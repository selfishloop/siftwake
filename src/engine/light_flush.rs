//! Build an immutable WAL prefix off-thread. Tail frames are synced to both
//! old authoritative WAL and a standard replacement WAL until manifest install.
//! A kill before install replays the old WAL; after install it replays the tail.
use super::*;
use std::io::{Seek, SeekFrom};

pub(super) struct Pending {
    handle: JoinHandle<Result<Run>>,
    output: u64,
    active: Active,
    log: File,
    old_tail: u64,
    new_tail: u64,
    keys: BTreeSet<DeltaKey>,
}

impl Database {
    pub(super) fn start_light_flush(&mut self) -> Result<bool> {
        if !self.streamed_wal_layer
            || self.light_flush.is_some()
            || !self.docs.is_empty()
            || self.shadow.is_empty()
        {
            return Ok(false);
        }
        // Independent read handle: Windows positional reads can change the
        // shared cursor of try_clone(), which must never race WAL appends.
        let reader = {
            let _phase = profile::phase(
                self.write_profile.is_some(),
                "prefix.capture_old_wal_open",
                self.sequence,
                self.active.id,
            );
            File::open(manifest::path(&self.dir, self.active.id, "fdb"))?
        };
        let output = self.allocate()?;
        let id = self.allocate()?;
        let sequence = self.sequence;
        let (log, active) = {
            let _phase = profile::phase(
                self.write_profile.is_some(),
                "prefix.create_new_active",
                sequence,
                id,
            );
            manifest::new_active(&self.dir, id, sequence)?
        };
        let old_tail = self.log.metadata()?.len();
        let new_tail = log.metadata()?.len();
        let shadow = self.shadow.clone();
        let light = self.light.clone();
        let dir = self.dir.clone();
        let handle = std::thread::spawn(move || {
            #[cfg(test)]
            test_jobs::wait(&dir);
            let _writer = crate::access::WriterScope::enter();
            let _working = crate::write_budget::WorkingScope::builder()?;
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("light_flush.worker");
            // The index and source are generated from the same frozen keys,
            // checked locations and versions, one document at a time.
            let changes = || {
                shadow.iter().map(|key| {
                    let owned = own_delta_key(key);
                    Ok(match light.get(&owned) {
                        Some(row) => Change::Put {
                            stored: row.read(&reader, &owned)?,
                        },
                        None => Change::Delete { key: owned },
                    })
                })
            };
            let mut index = Index::default();
            for change in changes() {
                if let Change::Put { stored } = change? {
                    index.add_builder(&stored)?;
                }
            }
            write_run_prepared(&dir, output, sequence, changes(), |source| {
                let records = shadow
                    .iter()
                    .enumerate()
                    .map(|(slot, key)| (own_delta_key(key), slot as u32))
                    .collect();
                reused_builder(&dir, source)?.finish_delta(&index, &records)?;
                Ok(())
            })
        });
        self.light_flush = Some(Pending {
            handle,
            output,
            active,
            log,
            old_tail,
            new_tail,
            keys: BTreeSet::new(),
        });
        Ok(true)
    }

    pub(super) fn light_flush_ready(&mut self, wait: bool) -> Result<bool> {
        if !self
            .light_flush
            .as_ref()
            .is_some_and(|p| wait || p.handle.is_finished())
        {
            return Ok(false);
        }
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("light_flush.install");
        let pending = self.light_flush.take().unwrap();
        let _phase = profile::phase(
            self.write_profile.is_some(),
            "prefix.install",
            self.sequence,
            self.active.id,
        );
        let result = (|| {
            let run = {
                let _phase = profile::phase(
                    self.write_profile.is_some(),
                    "prefix.worker_join",
                    self.sequence,
                    self.active.id,
                );
                pending
                    .handle
                    .join()
                    .map_err(|_| Error::Invalid("WAL prefix worker panicked".into()))??
            };
            let mut runs = self.descriptors();
            runs.push(run.source.clone());
            let id = self.allocate()?;
            #[cfg(test)]
            kill_barrier("before_publish");
            self.publish(
                id,
                &Manifest {
                    version: 3,
                    runs,
                    active: pending.active.clone(),
                },
            )?;
            #[cfg(test)]
            kill_barrier("after_publish");
            // Only keys written since capture belong to the replacement WAL.
            // Freeze-before/after snapshot views own different maps/row offsets.
            self.light.retain(|key, row| {
                if !pending.keys.contains(key.as_ref()) {
                    return false;
                }
                row.wal.location.offset =
                    row.wal.location.offset - pending.old_tail + pending.new_tail;
                true
            });
            self.shadow = pending.keys;
            self.delta_bytes = self.light.values().map(|r| r.location.len as usize).sum();
            self.log = pending.log;
            self.active = pending.active;
            self.runs.push(run);
            self.refresh_snapshots()?;
            self.try_reclaim();
            if self.runs.len() >= 4 {
                self.start_compaction_mode(true)?;
            }
            Ok(true)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    pub(super) fn light_flush_live(&self, live: &mut BTreeSet<(Kind, u64)>) {
        if let Some(pending) = &self.light_flush {
            live.insert((Kind::Run, pending.output));
            live.insert((Kind::Legacy, pending.active.id));
        }
    }
}

#[cfg(test)]
fn kill_barrier(stage: &str) {
    if std::env::var("FDB_PREFIX_KILL").ok().as_deref() == Some(stage) {
        std::fs::write(std::env::var_os("FDB_PREFIX_MARKER").unwrap(), stage).unwrap();
        loop {
            std::thread::park();
        }
    }
}

#[cfg(test)]
mod tests;

impl Pending {
    #[cfg(test)]
    pub(super) fn ready(&self) -> bool {
        self.handle.is_finished()
    }
    pub(super) fn append(
        &mut self,
        mut copy: impl FnMut(&mut File) -> Result<()>,
        keys: impl Iterator<Item = Key>,
    ) -> Result<()> {
        self.log.seek(SeekFrom::End(0))?;
        copy(&mut self.log)?;
        for key in keys {
            remember_delta_key(&mut self.keys, &key);
        }
        Ok(())
    }
    pub(super) fn finish(self) {
        let _ = self.handle.join();
    }
}
