//! Source-preserving reclamation. Only records shadowed by an immutable newer
//! source are removed; WAL/delta state never authorizes removing a barrier.
use super::*;

const MAP_LIMIT: usize = 1024 * 1024;
const SOURCE_LIMIT: u64 = 256 * 1024 * 1024;

#[cfg(test)]
mod tests;

impl Database {
    /// Enable/disable prototype stale-revision rewrites during explicit maintenance.
    /// Disabled by default while measured maintenance/search gates remain open.
    /// Does not change the persistent format or interrupt an in-flight worker.
    pub fn set_revision_reclamation(&mut self, enabled: bool) {
        self.revision_reclamation = enabled;
    }
    /// Reference/candidate control for the measured prototype; no disk effect.
    pub fn set_revision_maintenance_responsive(&mut self, responsive: bool) {
        self.revision_responsive = responsive;
    }
    /// True only while a revision worker is still executing, not awaiting install.
    pub fn revision_reclamation_running(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|j| j.revision && !j.handle.is_finished())
    }
    pub(super) fn preempt_revision_work(&mut self) -> Result<()> {
        if let Some(control) = self.job.as_ref().and_then(|j| j.cancel.as_ref()) {
            control.cancel();
            self.wait_for_compaction()?;
        }
        Ok(())
    }
    fn revision_masks(&self) -> Result<Vec<Vec<bool>>> {
        let mut visible: Vec<_> = self
            .runs
            .iter()
            .map(|r| vec![false; r.index.len()])
            .collect();
        for row in MergeKeys::new(&self.runs)? {
            let (run, slot) = row?;
            visible[run][slot as usize] = true;
        }
        Ok(visible)
    }

    /// Diagnostic scan of paged key/record metadata; no body decoding, no writes.
    /// A latest tombstone is retained even if an older source still contains a put.
    pub fn segment_liveness(&self) -> Result<Vec<crate::SegmentLiveness>> {
        self.healthy()?;
        self.ensure_sources()?;
        let masks = self.revision_masks()?;
        self.liveness_with(&masks)
    }
    fn liveness_with(&self, masks: &[Vec<bool>]) -> Result<Vec<crate::SegmentLiveness>> {
        self.runs
            .iter()
            .zip(masks)
            .map(|(run, mask)| {
                let mut out = crate::SegmentLiveness {
                    source_id: run.source.id,
                    source_bytes: run.source.end,
                    index_bytes: run.index.bytes,
                    records: mask.len(),
                    live_records: 0,
                    tombstones: 0,
                    payload_bytes: 0,
                    dead_payload_bytes: 0,
                    dead_records: 0,
                };
                for (slot, &live) in mask.iter().enumerate() {
                    let (len, _, _, deleted) = run.index.payload_binding(slot as u32)?;
                    out.payload_bytes += u64::from(len);
                    out.tombstones += usize::from(deleted);
                    out.live_records += usize::from(live);
                    out.dead_records += usize::from(!live);
                    if !live {
                        out.dead_payload_bytes += u64::from(len);
                    }
                }
                Ok(out)
            })
            .collect()
    }

    /// Schedule a revision rewrite or retire an entirely shadowed source group.
    /// Responsive selection favors >=50% dead payload, or >=20% when aggregate
    /// dead/live payload exceeds 20%. At most one worker, <=1 MiB ID map and
    /// <=256 MiB source input per job; existing builder/source leases still apply.
    /// Responsive copying is paced to 32 MiB/s while readers are active; idle
    /// copying is unpaced. Index I/O is not bandwidth-throttled. Metadata planning
    /// is foreground work. A writer cooperatively cancels unpublished work.
    /// The old source stays readable/pinned until normal manifest publication.
    pub fn start_revision_reclamation(&mut self) -> Result<bool> {
        #[cfg(feature = "diagnostic-timing")]
        let _plan = crate::timing::maintenance::span("planning_metadata");
        self.healthy()?;
        self.ensure_sources()?;
        if self.job.is_some() {
            return Ok(false);
        }
        let responsive = self.revision_responsive;
        let visible = if responsive {
            self.revision_masks()?
        } else {
            Vec::new()
        };
        let stats = if responsive {
            self.liveness_with(&visible)?
        } else {
            self.segment_liveness()?
        };
        let dead: u64 = stats.iter().map(|s| s.dead_payload_bytes).sum();
        let live: u64 = stats
            .iter()
            .map(|s| s.payload_bytes - s.dead_payload_bytes)
            .sum();
        let over_budget = dead.saturating_mul(5) > live;
        let Some((selected, _)) = stats
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.records <= MAP_LIMIT / 4
                    && s.source_bytes <= SOURCE_LIMIT
                    && s.dead_payload_bytes >= 64 * 1024
                    && s.dead_payload_bytes.saturating_mul(5) >= s.payload_bytes
                    && (!responsive
                        || over_budget
                        || s.dead_payload_bytes.saturating_mul(2) >= s.payload_bytes)
            })
            .max_by(|(a, x), (b, y)| {
                if responsive {
                    // An all-dead prefix can retire together without any output.
                    match (
                        x.live_records == 0 && x.tombstones == 0,
                        y.live_records == 0 && y.tombstones == 0,
                    ) {
                        (true, true) => b.cmp(a),
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        _ => (u128::from(x.dead_payload_bytes) * u128::from(y.payload_bytes))
                            .cmp(&(u128::from(y.dead_payload_bytes) * u128::from(x.payload_bytes)))
                            .then(x.dead_payload_bytes.cmp(&y.dead_payload_bytes)),
                    }
                } else {
                    x.dead_payload_bytes.cmp(&y.dead_payload_bytes)
                }
            })
        else {
            return Ok(false);
        };
        let keep_mask = if responsive {
            visible[selected].clone()
        } else {
            let mut hidden = vec![0u64; self.runs[selected].index.len().div_ceil(64)];
            for newer in &self.runs[selected + 1..] {
                self.runs[selected]
                    .index
                    .hide_keys(&newer.index, &mut hidden)?;
            }
            (0..self.runs[selected].index.len())
                .map(|slot| hidden[slot / 64] & (1 << (slot % 64)) == 0)
                .collect()
        };
        let retire =
            responsive && stats[selected].live_records == 0 && stats[selected].tombstones == 0;
        let mut end = selected + 1;
        let mut bytes = stats[selected].source_bytes;
        if retire {
            while end < stats.len()
                && stats[end].live_records == 0
                && stats[end].tombstones == 0
                && bytes + stats[end].source_bytes <= SOURCE_LIMIT
            {
                bytes += stats[end].source_bytes;
                end += 1;
            }
        }
        let inputs = self.runs[selected..end].to_vec();
        let input = self.runs[selected].clone();
        let descriptors = inputs.iter().map(|r| r.source.clone()).collect();
        let dir = self.dir.clone();
        let id = self.allocate()?;
        let cancel = responsive
            .then(|| Arc::new(maintenance::Control::new(self.maintenance_activity.clone())));
        let worker_cancel = cancel.clone();
        let handle = std::thread::spawn(move || {
            let _scope = maintenance::Scope::enter(worker_cancel.clone());
            let result = (|| -> Result<JobOutput> {
                #[cfg(feature = "diagnostic-timing")]
                let _worker = crate::timing::maintenance::worker();
                #[cfg(feature = "diagnostic-timing")]
                let verify = crate::timing::maintenance::span("source_crc_verify");
                for run in &inputs {
                    maintenance::check()?;
                    let mut file = File::open(run.source.path(&dir, "fdb"))?;
                    if file.metadata()?.len() < run.source.end
                        || (run.source.kind == Kind::Run
                            && file.metadata()?.len() != run.source.end)
                        || format::prefix_crc(&mut file, run.source.end)? != run.source.checksum
                    {
                        return Err(Error::Corrupt(
                            "reclamation source checksum mismatch".into(),
                        ));
                    }
                }
                #[cfg(feature = "diagnostic-timing")]
                drop(verify);
                #[cfg(feature = "experimental-merge-postings")]
                for run in &inputs {
                    run.index.verify_reuse()?;
                }
                maintenance::check()?;
                if retire {
                    return Ok(JobOutput::Retire);
                }
                let keep = |slot: usize| keep_mask[slot];
                #[cfg(feature = "experimental-merge-postings")]
                let maps = {
                    #[cfg(feature = "diagnostic-timing")]
                    let _maps = crate::timing::maintenance::span("id_map_build");
                    let mut map = vec![u32::MAX; input.index.len()];
                    let mut output = 0u32;
                    for (slot, mapped) in map.iter_mut().enumerate() {
                        if keep(slot) {
                            if !input.index.deleted(slot as u32) {
                                *mapped = output;
                            }
                            output += 1;
                        }
                    }
                    map
                };
                let began = std::time::Instant::now();
                let mut copied = 0u64;
                let mut next_pace = 1024 * 1024;
                let changes = (0..input.index.len())
                    .filter(|&slot| keep(slot))
                    .map(|slot| {
                        maintenance::check()?;
                        let slot = slot as u32;
                        let change = if input.index.deleted(slot) {
                            Change::Delete {
                                key: input.index.key(slot)?,
                            }
                        } else {
                            #[cfg(feature = "experimental-direct-slot")]
                            let stored = input.index.read_selected_document(slot)?;
                            #[cfg(not(feature = "experimental-direct-slot"))]
                            let stored =
                                input.index.get(&input.index.key(slot)?)?.ok_or_else(|| {
                                    Error::Corrupt("missing reclamation payload".into())
                                })?;
                            Change::Put { stored }
                        };
                        copied += crate::binary_batch::entry_len(&change)? as u64;
                        let target = std::time::Duration::from_secs_f64(
                            copied as f64 / (32. * 1024. * 1024.),
                        );
                        if copied >= next_pace
                            && (!responsive
                                || worker_cancel.as_ref().is_some_and(|c| c.readers_active()))
                        {
                            if let Some(delay) = target.checked_sub(began.elapsed()) {
                                #[cfg(feature = "diagnostic-timing")]
                                let _pace = crate::timing::maintenance::span("pacing_wait");
                                if let Some(control) = &worker_cancel {
                                    control.wait(delay)?;
                                } else {
                                    std::thread::sleep(delay);
                                }
                            }
                            next_pace = copied + 1024 * 1024;
                        }
                        Ok(change)
                    });
                #[cfg(feature = "experimental-merge-postings")]
                {
                    #[cfg(feature = "bounded-engine")]
                    if responsive {
                        return write_run_ready(
                            &dir,
                            id,
                            input.source.sequence,
                            changes,
                            |source| {
                                #[cfg(feature = "diagnostic-timing")]
                                let _index = crate::timing::maintenance::span("index_rewrite_open");
                                // Pin BEFORE the final native CRC/schema validation;
                                // carry that same immutable handle into published reads.
                                let mut lease = crate::segment::SourceLease::immutable(
                                    &source.path(&dir, "fdb"),
                                )?;
                                let before = lease.metadata()?;
                                if before.len() != source.end {
                                    return Err(Error::Corrupt("prepared source length".into()));
                                }
                                let (builder, sequence) = Builder::from_native_records(
                                    &source.path(&dir, "idx"),
                                    &mut lease,
                                    source.end,
                                    source.checksum,
                                )?;
                                if sequence != source.sequence {
                                    return Err(Error::Corrupt("prepared source sequence".into()));
                                }
                                let mut index = builder.finish_reused(&[(&input.index, &maps)])?;
                                let after = lease.metadata()?;
                                if before.len() != after.len()
                                    || before.modified().ok() != after.modified().ok()
                                    || index.sequence != source.sequence
                                {
                                    return Err(Error::Corrupt(
                                        "prepared source/index identity changed".into(),
                                    ));
                                }
                                index.enable_paging(&source.path(&dir, "idx"))?;
                                index.bind_text_source(source.path(&dir, "fdb"), lease)?;
                                let mut source = source.clone();
                                source.keys = index.len();
                                Ok(Some(Run {
                                    source,
                                    index: Arc::new(index),
                                    deferred: None,
                                    source_records: None,
                                }))
                            },
                        )
                        .map(JobOutput::Replace);
                    }
                    write_run_prepared(&dir, id, input.source.sequence, changes, |source| {
                        reused_builder(&dir, source)?.finish_reused(&[(&input.index, &maps)])?;
                        Ok(())
                    })
                    .map(JobOutput::Replace)
                }
                #[cfg(not(feature = "experimental-merge-postings"))]
                {
                    write_run(&dir, id, input.source.sequence, changes).map(JobOutput::Replace)
                }
            })();
            match result {
                Err(error) if maintenance::cancelled(&error, &worker_cancel) => {
                    Ok(JobOutput::Cancelled)
                }
                other => other,
            }
        });
        self.job = Some(Job {
            inputs: descriptors,
            output_id: id,
            handle,
            cancel,
            revision: true,
        });
        Ok(true)
    }
}
