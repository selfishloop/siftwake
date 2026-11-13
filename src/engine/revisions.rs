//! Source-preserving reclamation. Only records shadowed by an immutable newer
//! source are removed; WAL/delta state never authorizes removing a barrier.
use super::*;

const MAP_LIMIT: usize = 1024 * 1024;
const SOURCE_LIMIT: u64 = 256 * 1024 * 1024;

#[cfg(test)]
mod tests;

impl Database {
    /// Enable or disable automatic stale-revision reclamation (enabled by default).
    /// Disabling cancels unpublished automatic work. Existing snapshots retain
    /// their view, and this setting is local to the open writer.
    pub fn set_revision_reclamation(&mut self, enabled: bool) {
        self.revision_reclamation = enabled;
        if !enabled {
            self.background_revision.cancel();
        }
    }
    /// Reference/candidate control for the measured prototype; no disk effect.
    pub fn set_revision_maintenance_responsive(&mut self, responsive: bool) {
        self.revision_responsive = responsive;
        if !responsive {
            self.background_revision.cancel();
        }
    }
    /// True only while a revision worker is still executing, not awaiting install.
    pub fn revision_reclamation_running(&self) -> bool {
        self.background_revision.running()
            || self
                .job
                .as_ref()
                .is_some_and(|j| j.revision && !j.handle.is_finished())
    }
    pub(super) fn preempt_revision_work(&mut self) -> Result<()> {
        if let Some(control) = self.job.as_ref().and_then(|j| j.cancel.as_ref()) {
            control.cancel();
            self.wait_for_merge()?;
        }
        Ok(())
    }
    pub(super) fn revision_masks(&self) -> Result<Vec<Vec<bool>>> {
        revision_masks(&self.runs)
    }

    /// Diagnostic scan of paged key/record metadata; no body decoding, no writes.
    /// A latest tombstone is retained even if an older source still contains a put.
    pub fn segment_liveness(&self) -> Result<Vec<crate::SegmentLiveness>> {
        self.healthy()?;
        self.ensure_sources()?;
        let masks = self.revision_masks()?;
        liveness_with(&self.runs, &masks)
    }
    /// Explicitly plan and schedule a revision rewrite or retire a shadowed group.
    /// Responsive selection favors >=50% dead payload, or >=20% when aggregate
    /// dead/live payload exceeds 20%. At most one worker, <=1 MiB ID map and
    /// <=256 MiB source input per job; existing builder/source leases still apply.
    /// Responsive copying is paced to 32 MiB/s while readers are active; idle
    /// copying is unpaced. Index I/O is not bandwidth-throttled. Metadata planning
    /// is foreground work. A writer cooperatively cancels unpublished work.
    /// Automatic reclamation instead plans off-thread. In both paths the old
    /// source stays readable and pinned until normal manifest publication.
    pub fn start_revision_reclamation(&mut self) -> Result<bool> {
        let _writer = crate::access::WriterScope::enter();
        #[cfg(feature = "diagnostic-timing")]
        let _plan = crate::timing::maintenance::span("planning_metadata");
        self.healthy()?;
        self.ensure_sources()?;
        if self.job.is_some() || self.background_revision.pending() {
            return Ok(false);
        }
        let responsive = self.revision_responsive;
        let Some(plan) = plan(&self.runs, responsive)? else {
            return Ok(false);
        };
        let inputs = self.runs[plan.start..plan.end].to_vec();
        let descriptors = inputs.iter().map(|r| r.source.clone()).collect();
        let dir = self.dir.clone();
        let id = self.allocate()?;
        let cancel = responsive
            .then(|| Arc::new(maintenance::Control::new(self.maintenance_activity.clone())));
        let worker_cancel = cancel.clone();
        let handle = std::thread::spawn(move || {
            #[cfg(test)]
            test_jobs::wait(&dir);
            rewrite(&dir, id, &inputs, plan, responsive, worker_cancel)
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

fn revision_masks(runs: &[Run]) -> Result<Vec<Vec<bool>>> {
    let mut visible: Vec<_> = runs.iter().map(|r| vec![false; r.index.len()]).collect();
    for row in MergeKeys::new(runs)? {
        maintenance::check()?;
        let (run, slot) = row?;
        visible[run][slot as usize] = true;
    }
    Ok(visible)
}

fn liveness_with(runs: &[Run], masks: &[Vec<bool>]) -> Result<Vec<crate::SegmentLiveness>> {
    runs.iter()
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
                maintenance::check()?;
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

pub(super) struct RevisionPlan {
    pub(super) start: usize,
    pub(super) end: usize,
    keep_mask: Vec<bool>,
    retire: bool,
}

pub(super) fn plan(runs: &[Run], responsive: bool) -> Result<Option<RevisionPlan>> {
    let mut visible = if responsive {
        revision_masks(runs)?
    } else {
        Vec::new()
    };
    // A newest tombstone is dispensable only when no older immutable
    // source contains its key. Pending WAL state never proves this.
    let mut barriers = vec![0usize; runs.len()];
    if responsive {
        for (n, mask) in visible.iter_mut().enumerate() {
            for (slot, keep) in mask.iter_mut().enumerate() {
                maintenance::check()?;
                if !*keep || !runs[n].index.deleted(slot as u32) {
                    continue;
                }
                let key = runs[n].index.key(slot as u32)?;
                let mut older = false;
                for run in &runs[..n] {
                    if run.index.find(&key)?.is_some() {
                        older = true;
                        break;
                    }
                }
                if !older {
                    *keep = false;
                    barriers[n] += 1;
                }
            }
        }
    }
    let stats = if responsive {
        liveness_with(runs, &visible)?
    } else {
        liveness_with(runs, &revision_masks(runs)?)?
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
        .filter(|(n, s)| {
            s.records <= MAP_LIMIT / 4
                && s.source_bytes <= SOURCE_LIMIT
                && ((responsive && s.live_records == 0)
                    || (barriers[*n] >= 128 && barriers[*n] * 5 >= s.records)
                    || (s.dead_payload_bytes >= 64 * 1024
                        && s.dead_payload_bytes.saturating_mul(5) >= s.payload_bytes
                        && (!responsive
                            || over_budget
                            || s.dead_payload_bytes.saturating_mul(2) >= s.payload_bytes)))
        })
        .max_by(|(a, x), (b, y)| {
            if responsive {
                // An all-dead prefix can retire together without any output.
                match (x.live_records == 0, y.live_records == 0) {
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
        return Ok(None);
    };
    let keep_mask = if responsive {
        visible[selected].clone()
    } else {
        let mut hidden = vec![0u64; runs[selected].index.len().div_ceil(64)];
        for newer in &runs[selected + 1..] {
            runs[selected].index.hide_keys(&newer.index, &mut hidden)?;
        }
        (0..runs[selected].index.len())
            .map(|slot| hidden[slot / 64] & (1 << (slot % 64)) == 0)
            .collect()
    };
    let retire = responsive && stats[selected].live_records == 0;
    let mut end = selected + 1;
    let mut bytes = stats[selected].source_bytes;
    if retire {
        while end < stats.len()
            && stats[end].live_records == 0
            && bytes + stats[end].source_bytes <= SOURCE_LIMIT
        {
            bytes += stats[end].source_bytes;
            end += 1;
        }
    }
    Ok(Some(RevisionPlan {
        start: selected,
        end,
        keep_mask,
        retire,
    }))
}

pub(super) fn rewrite(
    dir: &Path,
    id: u64,
    inputs: &[Run],
    plan: RevisionPlan,
    responsive: bool,
    cancel: Option<Arc<maintenance::Control>>,
) -> Result<JobOutput> {
    let RevisionPlan {
        keep_mask, retire, ..
    } = plan;
    let input = &inputs[0];
    let _writer = crate::access::WriterScope::enter();
    let _scope = maintenance::Scope::enter(cancel.clone());
    let result = (|| -> Result<JobOutput> {
        #[cfg(feature = "diagnostic-timing")]
        let _worker = crate::timing::maintenance::worker();
        #[cfg(feature = "diagnostic-timing")]
        let verify = crate::timing::maintenance::span("source_crc_verify");
        for run in inputs {
            maintenance::check()?;
            let mut file = File::open(run.source.path(dir, "fdb"))?;
            if file.metadata()?.len() < run.source.end
                || (run.source.kind == Kind::Run && file.metadata()?.len() != run.source.end)
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
        for run in inputs {
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
                    let stored = input
                        .index
                        .get(&input.index.key(slot)?)?
                        .ok_or_else(|| Error::Corrupt("missing reclamation payload".into()))?;
                    Change::Put { stored }
                };
                copied += crate::binary_batch::entry_len(&change)? as u64;
                let target =
                    std::time::Duration::from_secs_f64(copied as f64 / (32. * 1024. * 1024.));
                if copied >= next_pace
                    && (!responsive || cancel.as_ref().is_some_and(|c| c.readers_active()))
                {
                    if let Some(delay) = target.checked_sub(began.elapsed()) {
                        #[cfg(feature = "diagnostic-timing")]
                        let _pace = crate::timing::maintenance::span("pacing_wait");
                        if let Some(control) = &cancel {
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
                return write_run_ready(dir, id, input.source.sequence, changes, |source| {
                    #[cfg(feature = "diagnostic-timing")]
                    let _index = crate::timing::maintenance::span("index_rewrite_open");
                    // Pin BEFORE the final native CRC/schema validation;
                    // carry that same immutable handle into published reads.
                    let mut lease =
                        crate::segment::SourceLease::immutable(&source.path(dir, "fdb"))?;
                    let before = lease.metadata()?;
                    if before.len() != source.end {
                        return Err(Error::Corrupt("prepared source length".into()));
                    }
                    let (builder, sequence) = Builder::from_native_records(
                        &source.path(dir, "idx"),
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
                    index.enable_paging(&source.path(dir, "idx"))?;
                    index.bind_text_source(source.path(dir, "fdb"), lease)?;
                    let mut source = source.clone();
                    source.keys = index.len();
                    Ok(Some(Run {
                        source,
                        index: Arc::new(index),
                        deferred: None,
                        source_records: None,
                    }))
                })
                .map(JobOutput::Replace);
            }
            write_run_prepared(dir, id, input.source.sequence, changes, |source| {
                reused_builder(dir, source)?.finish_reused(&[(&input.index, &maps)])?;
                Ok(())
            })
            .map(JobOutput::Replace)
        }
        #[cfg(not(feature = "experimental-merge-postings"))]
        {
            write_run(dir, id, input.source.sequence, changes).map(JobOutput::Replace)
        }
    })();
    match result {
        Err(error) if maintenance::cancelled(&error, &cancel) => Ok(JobOutput::Cancelled),
        other => other,
    }
}
