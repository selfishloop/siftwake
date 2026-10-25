//! Source-preserving reclamation. Only records shadowed by an immutable newer
//! source are removed; WAL/delta state never authorizes removing a barrier.
use super::*;

const MAP_LIMIT: usize = 1024 * 1024;
const SOURCE_LIMIT: u64 = 256 * 1024 * 1024;

#[cfg(test)]
mod tests;

impl Database {
    /// Enable/disable prototype stale-revision rewrites during explicit maintenance.
    /// Disabled by default while the measured search-p95 gate remains open.
    /// Does not change the persistent format or interrupt an in-flight worker.
    pub fn set_revision_reclamation(&mut self, enabled: bool) {
        self.revision_reclamation = enabled;
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
                for (slot, live) in mask.into_iter().enumerate() {
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

    /// Schedule one immutable-segment rewrite when >=20% of its payload is
    /// shadowed. At most one existing maintenance worker, <=1 MiB ID map and
    /// <=256 MiB source input per job; existing builder/source leases still apply.
    /// Source rewriting is paced to 32 MiB/s; derived-index I/O is bounded by
    /// one segment but is not bandwidth-throttled. Scheduling scans metadata.
    /// The old source stays readable/pinned until normal manifest publication.
    pub fn start_revision_reclamation(&mut self) -> Result<bool> {
        self.healthy()?;
        self.ensure_sources()?;
        if self.job.is_some() {
            return Ok(false);
        }
        let stats = self.segment_liveness()?;
        let Some((selected, _)) = stats
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.records <= MAP_LIMIT / 4
                    && s.source_bytes <= SOURCE_LIMIT
                    && s.dead_payload_bytes >= 64 * 1024
                    && s.dead_payload_bytes.saturating_mul(5) >= s.payload_bytes
            })
            .max_by_key(|(_, s)| s.dead_payload_bytes)
        else {
            return Ok(false);
        };
        let mut mask = vec![0u64; self.runs[selected].index.len().div_ceil(64)];
        for newer in &self.runs[selected + 1..] {
            self.runs[selected]
                .index
                .hide_keys(&newer.index, &mut mask)?;
        }
        let input = self.runs[selected].clone();
        let descriptors = vec![input.source.clone()];
        let dir = self.dir.clone();
        let id = self.allocate()?;
        let handle = std::thread::spawn(move || {
            let mut file = File::open(input.source.path(&dir, "fdb"))?;
            if file.metadata()?.len() < input.source.end
                || (input.source.kind == Kind::Run && file.metadata()?.len() != input.source.end)
                || format::prefix_crc(&mut file, input.source.end)? != input.source.checksum
            {
                return Err(Error::Corrupt(
                    "reclamation source checksum mismatch".into(),
                ));
            }
            #[cfg(feature = "experimental-merge-postings")]
            input.index.verify_reuse()?;
            let keep = |slot: usize| mask[slot / 64] & (1 << (slot % 64)) == 0;
            #[cfg(feature = "experimental-merge-postings")]
            let maps = {
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
                    if copied >= next_pace {
                        if let Some(delay) = target.checked_sub(began.elapsed()) {
                            std::thread::sleep(delay);
                        }
                        next_pace = copied + 1024 * 1024;
                    }
                    Ok(change)
                });
            #[cfg(feature = "experimental-merge-postings")]
            {
                write_run_prepared(&dir, id, input.source.sequence, changes, |source| {
                    reused_builder(&dir, source)?.finish_reused(&[(&input.index, &maps)])?;
                    Ok(())
                })
            }
            #[cfg(not(feature = "experimental-merge-postings"))]
            {
                write_run(&dir, id, input.source.sequence, changes)
            }
        });
        self.job = Some(Job {
            inputs: descriptors,
            output_id: id,
            handle,
        });
        Ok(true)
    }
}
