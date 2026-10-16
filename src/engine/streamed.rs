//! Opt-in streamed transactions. Existing WAL/source/manifest codecs are retained.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};

const INPUT_ENTRY_LIMIT: usize = 256 * 1024;
const KEY_ALLOWANCE: usize = 1024 * 1024;
const HEADER: usize = 24;
static TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Stage {
    path: PathBuf,
    file: Option<File>,
    rows: BTreeMap<Key, (u64, usize, u32)>,
    key_bytes: usize,
    length: usize,
    index: Index,
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
        })
    }
    fn push(&mut self, change: Change) -> Result<()> {
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
        let bytes = crate::binary_batch::entry(&change)?;
        if let Change::Put { stored } = &change {
            self.index.add_builder(stored)?;
        }
        if self.index.builder_memory_bytes() > 4 * 1024 * 1024 {
            return Err(Error::Invalid(
                "streamed transaction index allowance".into(),
            ));
        }
        let start = (HEADER + self.length) as u64;
        self.file.as_mut().unwrap().write_all(&bytes)?;
        self.rows
            .insert(key.clone(), (start, bytes.len(), format::crc(&bytes)));
        self.length += bytes.len();
        self.key_bytes += cost;
        Ok(())
    }
    fn finish(&mut self, sequence: u64) -> Result<()> {
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
        wal.sync_all()?;
        Ok(())
    }
    fn changes(&self) -> Result<impl Iterator<Item = Result<Change>> + '_> {
        let file = self.file.as_ref().unwrap().try_clone()?;
        Ok(self
            .rows
            .iter()
            .map(move |(key, &(offset, len, checksum))| {
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
    /// Opt-in profile: <=256 KiB per encoded entry, <=1 MiB key admission,
    /// <=4 MiB estimated index admission (not an exact heap measurement);
    /// at most 10k mutations and the existing 64 MiB frame limit. A complete
    /// synced WAL frame precedes source/manifest publication. Each successful
    /// call leaves an empty delta, and drains the previous merge before intake.
    /// These workspace allowances do not cap caller allocations or process RSS.
    pub fn commit_streamed(
        &mut self,
        mutations: impl IntoIterator<Item = Result<Mutation>>,
    ) -> Result<u64> {
        self.healthy()?;
        self.ensure_sources()?;
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::Invalid("sequence overflow".into()))?;
        // Drain before reserving intake: a worker must not wait for a workspace
        // held by the foreground thread waiting for that same worker.
        self.wait_for_compaction()?;
        self.flush()?;
        self.wait_for_compaction()?;
        let intake = crate::write_budget::WorkingScope::streamed()?;
        let mut stage = Stage::new(&self.dir)?;
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
        // Intake is complete; only key/location and index-only metadata remain.
        // The same working scope includes the following source/builder phase.
        // Existing normal commits may have populated a delta. Publish that delta
        // first; the validated streamed transaction still has the next sequence.
        self.flush()?;
        self.wait_for_compaction()?;
        let result = (|| {
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
