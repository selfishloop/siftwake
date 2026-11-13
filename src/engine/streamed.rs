//! Streamed transactions in the default bounded-engine profile. Existing codecs are retained.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};

const INPUT_ENTRY_LIMIT: usize = 256 * 1024;
const KEY_ALLOWANCE: usize = 1024 * 1024;
const HEADER: usize = 24;
static TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Stage {
    path: PathBuf,
    file: Option<File>,
    rows: BTreeMap<Key, (u64, usize, u32, bool, u32)>,
    key_bytes: usize,
    length: usize,
    index: Index,
    filters: BTreeMap<Key, Arc<super::light::LightFilter>>,
    build_filters: bool,
}
impl Stage {
    fn new(dir: &Path) -> Result<Self> {
        let path = dir.join(format!(
            "input-{}-{}.tmp",
            std::process::id(),
            TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(&[0; HEADER + 5])?;
        Ok(Self {
            path,
            file: Some(file),
            rows: BTreeMap::new(),
            key_bytes: 0,
            length: 5,
            index: Index::default(),
            filters: BTreeMap::new(),
            build_filters: false,
        })
    }
    fn push(&mut self, change: Change) -> Result<()> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("stream.stage_entry");
        let key = match &change {
            Change::Put { stored } => &stored.document.key,
            Change::Delete { key } => key,
        };
        if self.rows.contains_key(key) {
            return Err(Error::Invalid("duplicate batch key".into()));
        }
        let length = crate::binary_batch::entry_len(&change)?;
        if length > INPUT_ENTRY_LIMIT || self.length + length > format::MAX_FRAME {
            return Err(Error::Invalid(
                "streamed transaction exceeds entry/frame allowance".into(),
            ));
        }
        // Key strings and a conservative map-node allowance. This is admission,
        // not an allocator/RSS measurement; caller-created input is outside it.
        let cost = key.scope.len() + key.collection.len() + key.id.len() + 512;
        if self.key_bytes + cost > KEY_ALLOWANCE {
            return Err(Error::Invalid("streamed transaction key allowance".into()));
        }
        let bytes = {
            #[cfg(feature = "diagnostic-timing")]
            let _encode = crate::timing::span("stream.encode_entry");
            crate::binary_batch::entry(&change)?
        };
        let deferred = if self.build_filters { 128 } else { 32 };
        if self.build_filters
            && self.rows.len() < deferred
            && let Change::Put { stored } = &change
        {
            self.filters.insert(
                key.clone(),
                super::light::LightFilter::new(&stored.document),
            );
        }
        // Small transactions need no inverted index until explicit flush.
        // When the small allowance is exceeded, build its earlier entries once.
        if self.rows.len() == deferred {
            let mut index = std::mem::take(&mut self.index);
            for previous in self.changes()? {
                if let Change::Put { stored } = previous? {
                    index.add_builder(&stored)?;
                }
            }
            self.index = index;
        }
        if self.rows.len() >= deferred
            && let Change::Put { stored } = &change
        {
            self.index.add_builder(stored)?;
        }
        if self.index.builder_memory_bytes() > 4 * 1024 * 1024 {
            return Err(Error::Invalid(
                "streamed transaction index allowance".into(),
            ));
        }
        let start = (HEADER + self.length) as u64;
        // Windows seek_read on a cloned staging handle may move its shared
        // cursor. Earlier-entry indexing must never change the append offset.
        self.file.as_mut().unwrap().seek(SeekFrom::Start(start))?;
        self.file.as_mut().unwrap().write_all(&bytes)?;
        self.rows.insert(
            key.clone(),
            (
                start,
                bytes.len(),
                format::crc(&bytes),
                matches!(change, Change::Delete { .. }),
                format::crc(&bytes[5..]),
            ),
        );
        self.length += bytes.len();
        self.key_bytes += cost;
        Ok(())
    }
    fn finish(&mut self, sequence: u64) -> Result<()> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("stream.finish_frame_crc");
        let file = self.file.as_mut().unwrap();
        file.seek(SeekFrom::Start(HEADER as u64))?;
        file.write_all(&[0])?;
        file.write_all(&(self.rows.len() as u32).to_le_bytes())?;
        file.seek(SeekFrom::Start(HEADER as u64))?;
        let mut block = [0; 64 * 1024];
        let mut remaining = self.length;
        let mut state = !0;
        while remaining != 0 {
            let n = remaining.min(block.len());
            file.read_exact(&mut block[..n])?;
            state = format::crc_extend(state, &block[..n]);
            remaining -= n;
        }
        file.write_all(b"FDBDONE1")?;
        let mut header = [0; HEADER];
        header[..4].copy_from_slice(b"TXN2");
        header[4..8].copy_from_slice(&(self.length as u32).to_le_bytes());
        header[8..16].copy_from_slice(&sequence.to_le_bytes());
        header[16..20].copy_from_slice(&(!state).to_le_bytes());
        let checksum = format::crc(&header[..20]);
        header[20..].copy_from_slice(&checksum.to_le_bytes());
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&header)?;
        Ok(())
    }
    fn append(&mut self, wal: &mut File) -> Result<()> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("stream.wal_copy_and_sync");
        let file = self.file.as_mut().unwrap();
        file.seek(SeekFrom::Start(0))?;
        wal.seek(SeekFrom::End(0))?;
        let mut bytes = [0; 64 * 1024];
        let mut remaining = HEADER + self.length + 8;
        while remaining != 0 {
            let n = remaining.min(bytes.len());
            file.read_exact(&mut bytes[..n])?;
            wal.write_all(&bytes[..n])?;
            remaining -= n;
        }
        {
            #[cfg(feature = "diagnostic-timing")]
            let _sync = crate::timing::span("stream.wal_sync");
            wal.sync_all()?;
        }
        Ok(())
    }
    fn changes(&self) -> Result<impl Iterator<Item = Result<Change>> + '_> {
        let file = self.file.as_ref().unwrap().try_clone()?;
        Ok(self
            .rows
            .iter()
            .map(move |(key, &(offset, len, checksum, _, _))| {
                #[cfg(feature = "diagnostic-timing")]
                let _span = crate::timing::span("stream.read_decode_entry");
                let mut bytes = vec![0; len];
                crate::segment::read_at(&file, offset, &mut bytes)?;
                if format::crc(&bytes) != checksum
                    || len < 5
                    || u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as usize != len - 5
                {
                    return Err(Error::Corrupt("staged entry bounds".into()));
                }
                let change = crate::binary_batch::decode_entry(bytes[0], &bytes[5..])?;
                let actual = match &change {
                    Change::Put { stored } => &stored.document.key,
                    Change::Delete { key } => key,
                };
                if actual != key {
                    return Err(Error::Corrupt("staged entry identity".into()));
                }
                Ok(change)
            }))
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Database {
    /// Stream one atomic transaction without collecting its documents in a Vec.
    /// Available in the default bounded-engine profile: <=256 KiB per encoded entry,
    /// <=1 MiB key admission,
    /// <=4 MiB estimated index admission (not an exact heap measurement);
    /// at most 10k mutations and the existing 64 MiB frame limit. A complete
    /// synced WAL frame is the commit boundary. With set_streamed_wal_layer(true),
    /// small <=128-entry/8MiB calls with no owned delta retain checked WAL
    /// addresses. A background prefix flush starts at256 keys/8MiB;2048 keys,
    ///64MiB WAL and Options watermarks bound backpressure. Larger calls drain
    /// the prefix worker and flush/build a run.
    /// These workspace allowances do not cap caller allocations or process RSS.
    pub fn commit_streamed(
        &mut self,
        mutations: impl IntoIterator<Item = Result<Mutation>>,
    ) -> Result<u64> {
        let _writer = crate::access::WriterScope::enter();
        let _profile = super::profile::Scope::enter(&self.write_profile, self.sequence);
        #[cfg(feature = "diagnostic-timing")]
        let _total = crate::timing::span("stream.commit_total");
        self.healthy()?;
        self.ensure_sources()?;
        {
            let _phase = super::profile::phase(
                self.write_profile.is_some(),
                "stream.prefix_poll",
                self.sequence,
                self.active.id,
            );
            self.light_flush_ready(false)?;
        }
        {
            let _phase = super::profile::phase(
                self.write_profile.is_some(),
                "stream.merge_poll",
                self.sequence,
                self.active.id,
            );
            self.poll_normal_compaction()?;
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::Invalid("sequence overflow".into()))?;
        // Six MiB intake can coexist with one eight MiB worker. Release it
        // before a fallback joins a worker or reserves the full builder scope.
        let stage_profile = super::profile::phase(
            self.write_profile.is_some(),
            "stream.stage_frame",
            self.sequence,
            self.active.id,
        );
        let intake = crate::write_budget::reserve_working(6 * 1024 * 1024)?;
        let mut stage = Stage::new(&self.dir)?;
        stage.build_filters = self.streamed_wal_layer;
        let mut count = self.count;
        for mutation in mutations {
            if stage.rows.len() == 10_000 {
                return Err(Error::Invalid(
                    "batch must contain 1..10000 mutations".into(),
                ));
            }
            let mutation = mutation?;
            let (key, guard) = match &mutation {
                Mutation::Put { document, guard } => {
                    validate_doc(document)?;
                    (&document.key, guard)
                }
                Mutation::Delete { key, guard } => {
                    validate_key(key)?;
                    (key, guard)
                }
            };
            let old = self.version(key)?;
            if !match guard {
                Guard::Any => true,
                Guard::Missing => old.is_none(),
                Guard::Version { version } => old == Some(*version),
            } {
                return Err(Error::Conflict(key.clone()));
            }
            match &mutation {
                Mutation::Put { .. } if old.is_none() => count += 1,
                Mutation::Delete { .. } if old.is_some() => count -= 1,
                _ => {}
            }
            stage.push(match mutation {
                Mutation::Put { document, .. } => Change::Put {
                    stored: StoredDocument {
                        document,
                        version: sequence,
                    },
                },
                Mutation::Delete { key, .. } => Change::Delete { key },
            })?;
        }
        if stage.rows.is_empty() {
            return Err(Error::Invalid(
                "batch must contain 1..10000 mutations".into(),
            ));
        }
        stage.finish(sequence)?;
        drop(stage_profile);
        self.finish_background_revision(false)?;
        if self.runs.len() > 1 {
            self.schedule_background_revision()?;
        }
        if self.streamed_wal_layer
            && stage.rows.len() <= 128
            && stage.length <= 8 * 1024 * 1024
            && self.docs.is_empty()
            && self.shadow.len() + stage.rows.len() <= 2048
            && self.log.metadata()?.len() + stage.length as u64 <= 64 * 1024 * 1024
            && !self.needs_flush()
        {
            // A synced existing-codec WAL frame is the commit boundary. No
            // manifest/index/worker drain is needed to expose this active layer.
            use std::io::Seek;
            let base = self.log.seek(SeekFrom::End(0))?;
            let wal_profile = super::profile::phase(
                self.write_profile.is_some(),
                "stream.wal_authoritative_sync",
                self.sequence,
                self.active.id,
            );
            if let Err(error) = stage.append(&mut self.log) {
                self.poisoned = true;
                return Err(error);
            }
            drop(wal_profile);
            if let Some(pending) = &mut self.light_flush {
                let _phase = super::profile::phase(
                    self.write_profile.is_some(),
                    "stream.wal_tail_sync",
                    self.sequence,
                    self.active.id,
                );
                let keys: Vec<_> = stage.rows.keys().cloned().collect();
                if let Err(error) = pending.append(|log| stage.append(log), keys.into_iter()) {
                    self.poisoned = true;
                    return Err(error);
                }
            }
            #[cfg(test)]
            if let Some(marker) = std::env::var_os("FDB_LIGHT_AFTER_SYNC") {
                std::fs::write(marker, b"synced-before-apply")?;
                loop {
                    std::thread::park();
                }
            }
            for (key, (offset, len, _, deleted, checksum)) in &stage.rows {
                remember_delta_key(&mut self.shadow, key);
                if let Some(old) = self.light.remove(key) {
                    self.delta_bytes -= old.location.len as usize;
                }
                if !deleted {
                    let location = crate::wal_delta::Location {
                        offset: base + offset + 5,
                        len: (len - 5) as u32,
                        checksum: *checksum,
                    };
                    self.delta_bytes += location.len as usize;
                    self.light.insert(
                        Arc::new(key.clone()),
                        super::light::LightRow {
                            wal: crate::wal_delta::Row {
                                location,
                                version: sequence,
                            },
                            filter: stage.filters.get(key).expect("small staged proof").clone(),
                        },
                    );
                }
            }
            self.sequence = sequence;
            self.count = count;
            drop(stage);
            drop(intake);
            {
                let _phase = super::profile::phase(
                    self.write_profile.is_some(),
                    "stream.publish_active_snapshot",
                    self.sequence,
                    self.active.id,
                );
                self.refresh_snapshots()?;
            }
            let _phase = super::profile::phase(
                self.write_profile.is_some(),
                "stream.prefix_start",
                self.sequence,
                self.active.id,
            );
            if (self.shadow.len() >= 256 || self.log.metadata()?.len() >= 8 * 1024 * 1024)
                && let Err(error) = self.start_light_flush()
            {
                self.poisoned = true;
                return Err(error);
            }
            return Ok(sequence);
        }
        drop(intake);
        {
            let _phase = super::profile::phase(
                self.write_profile.is_some(),
                "stream.worker_drain",
                self.sequence,
                self.active.id,
            );
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("stream.fallback_worker_drain");
            self.preempt_revision_work()?;
            self.wait_for_merge()?;
        }
        {
            let _phase = super::profile::phase(
                self.write_profile.is_some(),
                "stream.delta_flush",
                self.sequence,
                self.active.id,
            );
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("stream.fallback_delta_flush");
            self.flush()?;
        }
        {
            let _phase = super::profile::phase(
                self.write_profile.is_some(),
                "stream.post_flush_drain",
                self.sequence,
                self.active.id,
            );
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("stream.fallback_post_flush_drain");
            self.wait_for_merge()?;
        }
        let intake = crate::write_budget::WorkingScope::streamed()?;
        // Large/fallback transactions still reuse the staged inverted index.
        if stage.rows.len() <= if stage.build_filters { 128 } else { 32 } {
            let mut index = std::mem::take(&mut stage.index);
            for change in stage.changes()? {
                if let Change::Put { stored } = change? {
                    index.add_builder(&stored)?;
                }
            }
            stage.index = index;
        }
        // Intake is complete; only key/location and index-only metadata remain.
        // The same working scope includes the following source/builder phase.
        // Existing normal commits may have populated a delta. Publish that delta
        // first; the validated streamed transaction still has the next sequence.
        self.flush()?;
        self.wait_for_merge()?;
        let result = (|| {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("stream.fallback_transaction_publish");
            stage.append(&mut self.log)?;
            let id = self.allocate()?;
            let run = write_run_prepared(&self.dir, id, sequence, stage.changes()?, |source| {
                let records = stage
                    .rows
                    .keys()
                    .enumerate()
                    .map(|(id, key)| (key.clone(), id as u32))
                    .collect();
                reused_builder(&self.dir, source)?.finish_delta(&stage.index, &records)?;
                Ok(())
            })?;
            let id = self.allocate()?;
            let (log, active) = manifest::new_active(&self.dir, id, sequence)?;
            let mut runs = self.descriptors();
            runs.push(run.source.clone());
            self.publish(
                id,
                &Manifest {
                    version: 3,
                    runs,
                    active: active.clone(),
                },
            )?;
            self.log = log;
            self.active = active;
            self.runs.push(run);
            self.sequence = sequence;
            self.count = count;
            #[cfg(feature = "experimental-paged-metadata")]
            {
                *self
                    .mask_cache
                    .get_mut()
                    .map_err(|_| Error::Corrupt("visibility cache poisoned".into()))? =
                    MaskCache::default();
            }
            Ok(sequence)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        drop(stage);
        drop(intake);
        if result.is_ok() {
            self.refresh_snapshots()?;
            self.try_reclaim();
        }
        if result.is_ok() && self.runs.len() >= 4 {
            self.start_compaction_mode(true)?;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staged_frame_matches_existing_codec_torn_frames_are_atomic_and_entry_crc_is_checked() {
        let path = std::env::temp_dir().join(format!(
            "fdb-stage-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let stored = |id: &str| StoredDocument {
            version: 1,
            document: crate::Document {
                key: Key::new("shared", "texts", id),
                title: "ŞİRKET".into(),
                text: "東京 🦀".into(),
                fields: Default::default(),
                links: Vec::new(),
            },
        };
        let mut stage = Stage::new(&path).unwrap();
        stage
            .push(Change::Put {
                stored: stored("b"),
            })
            .unwrap();
        stage
            .push(Change::Put {
                stored: stored("a"),
            })
            .unwrap();
        stage.finish(1).unwrap();
        let bytes = std::fs::read(&stage.path).unwrap();
        assert_eq!(
            bytes,
            format::encode(
                1,
                &Batch {
                    checkpoint: false,
                    changes: vec![
                        Change::Put {
                            stored: stored("b")
                        },
                        Change::Put {
                            stored: stored("a")
                        }
                    ]
                }
            )
            .unwrap()
        );
        let source = path.join("cut.fdb");
        let mut prefix = format::MAGIC_V4.to_vec();
        prefix.extend(
            format::encode(
                0,
                &Batch {
                    checkpoint: true,
                    changes: Vec::new(),
                },
            )
            .unwrap(),
        );
        for cut in 0..=bytes.len() {
            let mut all = prefix.clone();
            all.extend(&bytes[..cut]);
            std::fs::write(&source, all).unwrap();
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&source)
                .unwrap();
            let mut found = 0;
            let seq = format::replay(&mut file, prefix.len() as u64, |_, batch| {
                found += batch.changes.len();
                Ok(())
            })
            .unwrap();
            assert_eq!(found, if cut == bytes.len() { 2 } else { 0 });
            assert_eq!(seq, if cut == bytes.len() { 1 } else { 0 });
        }
        let offset = stage.rows.values().next().unwrap().0;
        stage
            .file
            .as_mut()
            .unwrap()
            .seek(SeekFrom::Start(offset + 10))
            .unwrap();
        stage.file.as_mut().unwrap().write_all(b"!").unwrap();
        assert!(matches!(
            stage.changes().unwrap().next().unwrap(),
            Err(Error::Corrupt(_))
        ));
        drop(stage);
        std::fs::remove_file(source).unwrap();
        std::fs::remove_dir(path).unwrap();
    }
}
