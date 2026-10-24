//! Merge validated immutable posting streams, preserving the input codecs.
use super::*;
use std::cmp::Reverse;

impl DiskIndex {
    pub(crate) fn verify_reuse(&self) -> Result<()> {
        let size = self.file.metadata()?.len();
        if size < HEADER_SIZE + FOOTER_SIZE {
            return Err(Error::Corrupt("short reuse segment".into()));
        }
        let mut footer = [0; FOOTER_SIZE as usize];
        read_at(&self.file, size - FOOTER_SIZE, &mut footer)?;
        let expected_footer = if self.binary_payload {
            FOOTER
        } else {
            LEGACY_FOOTER
        };
        #[cfg(feature = "bounded-engine")]
        let expected_footer = if self.compact_directory {
            compact::FOOTER
        } else {
            expected_footer
        };
        if &footer[..8] != expected_footer
            || crc(&footer[..28]) != u32::from_le_bytes(footer[28..].try_into().unwrap())
        {
            return Err(Error::Corrupt("reuse segment footer".into()));
        }
        let mut buffer = [0; 8192];
        let mut offset = 0;
        let mut state = !0;
        while offset < size - FOOTER_SIZE {
            let n = ((size - FOOTER_SIZE - offset) as usize).min(buffer.len());
            read_at(&self.file, offset, &mut buffer[..n])?;
            state = crate::format::crc_extend(state, &buffer[..n]);
            offset += n as u64;
        }
        if !state != u32::from_le_bytes(footer[24..28].try_into().unwrap()) {
            return Err(Error::Corrupt("reuse segment checksum".into()));
        }
        Ok(())
    }
}

struct Cursor<'a> {
    index: &'a DiskIndex,
    map: &'a [u32],
    posting: Posting,
    consumed: u32,
    buffer: [u8; 8192],
    available: usize,
    position: usize,
    previous: Option<u32>,
    mapped_previous: Option<u32>,
}
impl Cursor<'_> {
    fn reset(&mut self, posting: Posting) {
        self.posting = posting;
        self.consumed = 0;
        self.available = 0;
        self.position = 0;
        self.previous = None;
        self.mapped_previous = None;
        // Old bytes are inaccessible until next() refills the current posting.
    }
    fn next(&mut self) -> Result<Option<u32>> {
        while self.consumed < self.posting.count {
            if self.position == self.available {
                self.available =
                    ((self.posting.count - self.consumed) as usize * 4).min(self.buffer.len());
                read_at(
                    &self.index.file,
                    self.posting.offset + self.consumed as u64 * 4,
                    &mut self.buffer[..self.available],
                )?;
                self.position = 0;
            }
            let id = u32::from_le_bytes(
                self.buffer[self.position..self.position + 4]
                    .try_into()
                    .unwrap(),
            );
            self.position += 4;
            self.consumed += 1;
            if self.previous.is_some_and(|old| old >= id) {
                return Err(Error::Corrupt("unordered reuse posting".into()));
            }
            self.previous = Some(id);
            let mapped = *self
                .map
                .get(id as usize)
                .ok_or_else(|| Error::Corrupt("reuse posting out of bounds".into()))?;
            if mapped == u32::MAX {
                continue;
            }
            if self.mapped_previous.is_some_and(|old| old >= mapped) {
                return Err(Error::Corrupt("unordered reuse mapping".into()));
            }
            self.mapped_previous = Some(mapped);
            return Ok(Some(mapped));
        }
        Ok(None)
    }
}

#[cfg(any(test, not(feature = "bounded-engine")))]
pub(super) fn merge(
    inputs: &[(&DiskIndex, &[u32])],
    writer: &mut impl Write,
    offset: &mut u64,
) -> Result<Vec<(Term, Posting)>> {
    let mut result = Vec::new();
    merge_with(inputs, writer, offset, |t, p| {
        result.push((t, p));
        Ok(())
    })?;
    Ok(result)
}
pub(super) fn merge_with(
    inputs: &[(&DiskIndex, &[u32])],
    writer: &mut impl Write,
    offset: &mut u64,
    emit: impl FnMut(Term, Posting) -> Result<()>,
) -> Result<()> {
    merge_workspace::<true>(inputs, writer, offset, emit)
}

fn merge_workspace<const REUSE: bool>(
    inputs: &[(&DiskIndex, &[u32])],
    writer: &mut impl Write,
    offset: &mut u64,
    mut emit: impl FnMut(Term, Posting) -> Result<()>,
) -> Result<()> {
    #[cfg(feature = "diagnostic-timing")]
    let _merge = crate::timing::span("merge.posting_streams");
    #[cfg(feature = "diagnostic-timing")]
    let (mut input_entries, mut output_entries, mut output_terms) = (0usize, 0usize, 0usize);
    let mut iterators: Vec<_> = inputs
        .iter()
        .map(|(index, _)| index.term_cursor())
        .collect();
    let mut terms = BinaryHeap::new();
    let make_cursor = |n: usize, posting| Cursor {
        index: inputs[n].0,
        map: inputs[n].1,
        posting,
        consumed: 0,
        buffer: [0; 8192],
        available: 0,
        position: 0,
        previous: None,
        mapped_previous: None,
    };
    // One bounded buffer per input stream for this merge only. The selected
    // streams, maps and validation rules stay identical across terms.
    let mut cursors: Vec<_> = if REUSE {
        (0..inputs.len())
            .map(|n| {
                make_cursor(
                    n,
                    Posting {
                        offset: 0,
                        count: 0,
                    },
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    for (n, iterator) in iterators.iter_mut().enumerate() {
        if let Some((term, posting)) = iterator.next()? {
            terms.push(Reverse((term, n, posting.offset, posting.count)));
        }
    }
    while let Some(Reverse((term, n, start, count))) = terms.pop() {
        let mut same = vec![(
            n,
            Posting {
                offset: start,
                count,
            },
        )];
        while terms
            .peek()
            .is_some_and(|Reverse((next, _, _, _))| *next == term)
        {
            let Reverse((_, n, start, count)) = terms.pop().unwrap();
            same.push((
                n,
                Posting {
                    offset: start,
                    count,
                },
            ));
        }
        if REUSE {
            for (n, posting) in &same {
                cursors[*n].reset(*posting);
            }
        } else {
            cursors = same.iter().map(|(n, p)| make_cursor(*n, *p)).collect();
        }
        #[cfg(feature = "diagnostic-timing")]
        {
            input_entries += same.iter().map(|(_, p)| p.count as usize).sum::<usize>();
        }
        let mut ids = BinaryHeap::new();
        for (local, (input, _)) in same.iter().enumerate() {
            let n = if REUSE { *input } else { local };
            if let Some(id) = cursors[n].next()? {
                ids.push(Reverse((id, n)));
            }
        }
        let start = *offset;
        let mut count = 0u32;
        let mut previous = None;
        while let Some(Reverse((id, n))) = ids.pop() {
            if previous == Some(id) {
                return Err(Error::Corrupt("duplicate reuse destination".into()));
            }
            writer.write_all(&id.to_le_bytes())?;
            *offset += 4;
            count = count
                .checked_add(1)
                .ok_or_else(|| Error::Invalid("reuse posting count overflow".into()))?;
            previous = Some(id);
            if let Some(next) = cursors[n].next()? {
                ids.push(Reverse((next, n)));
            }
        }
        if count != 0 {
            #[cfg(feature = "diagnostic-timing")]
            {
                output_entries += count as usize;
                output_terms += 1;
            }
            emit(
                term.clone(),
                Posting {
                    offset: start,
                    count,
                },
            )?;
        }
        for (n, _) in same {
            if let Some((next, posting)) = iterators[n].next()? {
                terms.push(Reverse((next, n, posting.offset, posting.count)));
            }
        }
        if !REUSE {
            // Match the reference loop's end-of-term allocation lifetime.
            cursors = Vec::new();
        }
    }
    #[cfg(feature = "diagnostic-timing")]
    {
        crate::timing::count("merge.posting_input_entries", input_entries);
        crate::timing::count("merge.posting_output_entries", output_entries);
        crate::timing::count("merge.posting_output_terms", output_terms);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    type DirectoryEntries = Vec<(Term, Posting)>;
    fn merged<const REUSE: bool>(
        inputs: &[(&DiskIndex, &[u32])],
    ) -> Result<(Vec<u8>, DirectoryEntries)> {
        let mut bytes = Vec::new();
        let mut directory = Vec::new();
        merge_workspace::<REUSE>(inputs, &mut bytes, &mut 0, |t, p| {
            directory.push((t, p));
            Ok(())
        })?;
        Ok((bytes, directory))
    }

    #[test]
    fn cursor_reset_discards_partial_buffers_and_validation_state() {
        let dir = Temp(std::env::temp_dir().join(format!(
            "fdb-reuse-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&dir.0).unwrap();
        let path = dir.0.join("cursor.idx");
        let index = Builder::new(&path, 1, 0, 0).unwrap().finish().unwrap();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let start = file.metadata().unwrap().len();
        for id in 0u32..2052 {
            file.write_all(&id.to_le_bytes()).unwrap();
        }
        let small = start + 2052 * 4;
        for id in [3u32, 7, 7, 3, 7, 8, 2052] {
            file.write_all(&id.to_le_bytes()).unwrap();
        }
        file.flush().unwrap();
        let mut map: Vec<u32> = (10..2062).collect();
        map[3] = u32::MAX;
        map[8] = 5;
        let mut cursor = Cursor {
            index: &index,
            map: &map,
            posting: Posting {
                offset: start,
                count: 2052,
            },
            consumed: 0,
            buffer: [0; 8192],
            available: 0,
            position: 0,
            previous: None,
            mapped_previous: None,
        };
        assert_eq!(cursor.next().unwrap(), Some(10));
        // A smaller posting follows an unfinished 8192-byte window.
        cursor.reset(Posting {
            offset: small,
            count: 2,
        });
        assert_eq!(cursor.next().unwrap(), Some(17));
        assert_eq!(cursor.next().unwrap(), None);
        cursor.reset(Posting {
            offset: small,
            count: 0,
        });
        assert_eq!(cursor.next().unwrap(), None);
        // Input order, destination order, map bounds and truncated reads still fail.
        for (offset, count) in [
            (small + 8, 2),
            (small + 16, 2),
            (small + 24, 1),
            (small + 24, 2),
        ] {
            cursor.reset(Posting { offset, count });
            let result = (|| -> Result<()> {
                while cursor.next()?.is_some() {}
                Ok(())
            })();
            assert!(result.is_err(), "offset {offset}, count {count}");
        }
        // A reset after each error clears both previous-ID checks.
        cursor.reset(Posting {
            offset: start,
            count: 3,
        });
        assert_eq!(
            (
                cursor.next().unwrap(),
                cursor.next().unwrap(),
                cursor.next().unwrap(),
                cursor.next().unwrap()
            ),
            (Some(10), Some(11), Some(12), None)
        );
        // Refill past the 2048-ID boundary, with a fresh monotone mapping.
        let identity: Vec<_> = (0..2052).collect();
        cursor.map = &identity;
        cursor.reset(Posting {
            offset: start,
            count: 2052,
        });
        for id in 0..2052 {
            assert_eq!(cursor.next().unwrap(), Some(id));
        }
        assert_eq!(cursor.next().unwrap(), None);
    }

    #[test]
    #[ignore = "read-only probe of the existing fixed 10k indexes, never generates a corpus"]
    fn posting_workspace_existing_10k_probe() {
        use std::time::Instant;
        struct Sink {
            hash: crc32fast::Hasher,
        }
        impl Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.hash.update(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        fn run<const REUSE: bool>(
            inputs: &[(&DiskIndex, &[u32])],
        ) -> (u128, u64, u32, DirectoryEntries) {
            let mut sink = Sink {
                hash: crc32fast::Hasher::new(),
            };
            let mut offset = 0;
            let mut directory = Vec::new();
            let start = Instant::now();
            merge_workspace::<REUSE>(inputs, &mut sink, &mut offset, |t, p| {
                directory.push((t, p));
                Ok(())
            })
            .unwrap();
            let ns = start.elapsed().as_nanos();
            (ns, offset, sink.hash.finalize(), directory)
        }
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/benchmark-work/fdb");
        let mut paths: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| {
                p.extension().is_some_and(|e| e == "idx")
                    && !p
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .contains(".records")
            })
            .collect();
        paths.sort();
        assert_eq!(paths.len(), 5, "requires completed current 10k DB");
        let indexes: Vec<_> = paths
            .iter()
            .map(|p| {
                let i = DiskIndex::open(p, None).unwrap();
                i.verify_reuse().unwrap();
                i
            })
            .collect();
        let mut keys = BTreeMap::new();
        for index in &indexes {
            for id in 0..index.len() as u32 {
                assert!(keys.insert(index.key(id).unwrap(), 0u32).is_none());
            }
        }
        assert_eq!(keys.len(), 10000);
        for (id, slot) in keys.values_mut().enumerate() {
            *slot = id as u32;
        }
        let maps: Vec<Vec<_>> = indexes
            .iter()
            .map(|index| {
                (0..index.len() as u32)
                    .map(|id| keys[&index.key(id).unwrap()])
                    .collect()
            })
            .collect();
        let inputs: Vec<_> = indexes
            .iter()
            .zip(&maps)
            .map(|(i, m)| (i, m.as_slice()))
            .collect();
        let cursor_initializations: usize = indexes
            .iter()
            .map(|index| {
                let mut cursor = index.term_cursor();
                let mut count = 0;
                while cursor.next().unwrap().is_some() {
                    count += 1;
                }
                count
            })
            .sum();
        for block in 0..4 {
            let (before, after) = if block % 2 == 0 {
                (run::<false>(&inputs), run::<true>(&inputs))
            } else {
                let a = run::<true>(&inputs);
                (run::<false>(&inputs), a)
            };
            assert_eq!(
                (&before.1, &before.2, &before.3),
                (&after.1, &after.2, &after.3)
            );
            println!(
                "{}",
                serde_json::json!({"block":block,"allocated_ns":before.0,"reused_ns":after.0,"bytes":before.1,"terms":before.3.len(),"input_streams":inputs.len(),"cursor_buffer_bytes_per_stream":8192,"reference_cursor_initializations":cursor_initializations,"reused_cursor_initializations":inputs.len(),"initializations_are_code_counts_not_measured_memory_transfers":true,"read_only":true,"not_database_ingest_or_drain":true})
            );
        }
    }
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            if self.0.parent() == Some(std::env::temp_dir().as_path())
                && self
                    .0
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("fdb-reuse-")
            {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
    fn doc(id: &str, version: u64) -> StoredDocument {
        StoredDocument {
            version,
            document: Document {
                key: Key::new("shared", "çizelge", id),
                title: format!("Şirket {id}"),
                text: format!("teknoloji yatırımları {version}"),
                fields: [
                    ("day".into(), serde_json::json!(version)),
                    ("scalar".into(), serde_json::json!(true)),
                    ("object".into(), serde_json::json!({"skip":1})),
                ]
                .into(),
                links: vec![
                    Key::new("shared", "hedef", "ü"),
                    Key::new("shared", "hedef", "ü"),
                ],
            },
        }
    }
    #[test]
    fn legacy_footer_is_verified_with_its_own_codec_and_rejects_corruption() {
        let dir = Temp(std::env::temp_dir().join(format!(
            "fdb-reuse-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&dir.0).unwrap();
        let path = dir.0.join("legacy.idx");
        let mut builder = Builder::new(&path, 1, 0, 0).unwrap();
        builder
            .add_delete(&Key::new("shared", "docs", "deleted"))
            .unwrap();
        drop(builder.finish().unwrap());
        // A tombstone-only SEG03 has no JSON payloads; its directory is identical.
        let mut bytes = std::fs::read(&path).unwrap();
        let footer_start = bytes.len() - FOOTER_SIZE as usize;
        bytes[..8].copy_from_slice(LEGACY_MAGIC);
        bytes[footer_start..footer_start + 8].copy_from_slice(LEGACY_FOOTER);
        let checksum = crc(&bytes[..footer_start]);
        bytes[footer_start + 24..footer_start + 28].copy_from_slice(&checksum.to_le_bytes());
        let checksum = crc(&bytes[footer_start..footer_start + 28]);
        bytes[footer_start + 28..].copy_from_slice(&checksum.to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();
        let index = DiskIndex::open(&path, None).unwrap();
        assert!(!index.binary_payload);
        index.verify_reuse().unwrap();
        // Verify against an already-open index, including footer/global CRC cuts.
        for cut in 0..bytes.len() {
            std::fs::write(&path, &bytes[..cut]).unwrap();
            assert!(index.verify_reuse().is_err(), "cut {cut}");
        }
        for position in [0, HEADER_SIZE as usize, footer_start, bytes.len() - 1] {
            let mut corrupt = bytes.clone();
            corrupt[position] ^= 1;
            std::fs::write(&path, corrupt).unwrap();
            assert!(index.verify_reuse().is_err(), "corruption {position}");
        }
    }
    #[test]
    fn reused_postings_are_byte_equal_to_independent_reindex_and_reject_bad_maps_and_crc() {
        let dir = Temp(std::env::temp_dir().join(format!(
            "fdb-reuse-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&dir.0).unwrap();
        let mut first = Builder::new(&dir.0.join("first.idx"), 1, 0, 0).unwrap();
        for id in ["a", "b", "c"] {
            first.add(&doc(id, 1)).unwrap();
        }
        let first = first.finish().unwrap();
        let mut second = Builder::new(&dir.0.join("second.idx"), 2, 0, 0).unwrap();
        second.add_delete(&doc("b", 2).document.key).unwrap();
        for id in ["c", "d"] {
            second.add(&doc(id, 2)).unwrap();
        }
        let second = second.finish().unwrap();
        first.verify_reuse().unwrap();
        second.verify_reuse().unwrap();
        let first_map = [0, u32::MAX, u32::MAX];
        let second_map = [u32::MAX, 2, 3];
        let paired = [
            (&first, first_map.as_slice()),
            (&second, second_map.as_slice()),
        ];
        assert_eq!(
            merged::<false>(&paired).unwrap(),
            merged::<true>(&paired).unwrap()
        );
        assert_eq!(merged::<false>(&[]).unwrap(), merged::<true>(&[]).unwrap());
        let all_deleted = [u32::MAX; 3];
        assert_eq!(
            merged::<false>(&[(&first, &all_deleted)]).unwrap(),
            merged::<true>(&[(&first, &all_deleted)]).unwrap()
        );
        assert!(merged::<false>(&[(&first, &[])]).is_err());
        assert!(merged::<true>(&[(&first, &[])]).is_err());
        // Duplicate destination across independent streams remains an error.
        let duplicate = [0, 1, 2];
        assert!(merged::<false>(&[(&first, &duplicate), (&first, &duplicate)]).is_err());
        assert!(merged::<true>(&[(&first, &duplicate), (&first, &duplicate)]).is_err());
        for reuse in [false, true] {
            let name = if reuse { "reused.idx" } else { "reference.idx" };
            let mut builder = Builder::new(&dir.0.join(name), 2, 0, 0).unwrap();
            for (id, version) in [("a", 1), ("b", 2), ("c", 2), ("d", 2)] {
                if id == "b" {
                    builder.add_delete(&doc(id, version).document.key).unwrap();
                } else if reuse {
                    builder.add_reused(&doc(id, version)).unwrap();
                } else {
                    builder.add(&doc(id, version)).unwrap();
                }
            }
            if reuse {
                builder.finish_reused(&paired).unwrap();
            } else {
                builder.finish().unwrap();
            }
        }
        assert_eq!(
            std::fs::read(dir.0.join("reference.idx")).unwrap(),
            std::fs::read(dir.0.join("reused.idx")).unwrap()
        );
        let bad_map = [u32::MAX, 0, 3];
        assert!(
            merge(
                &[(&first, &first_map), (&second, &bad_map)],
                &mut Vec::new(),
                &mut 0
            )
            .is_err()
        );
        assert!(merge(&[(&first, &[])], &mut Vec::new(), &mut 0).is_err());
        let mut file = OpenOptions::new()
            .write(true)
            .open(dir.0.join("first.idx"))
            .unwrap();
        file.seek(SeekFrom::Start(HEADER_SIZE)).unwrap();
        file.write_all(b"!").unwrap();
        file.sync_all().unwrap();
        assert!(matches!(first.verify_reuse(), Err(Error::Corrupt(_))));
    }

    #[test]
    fn delta_export_matches_reindex_after_slot_reuse_updates_and_tombstones() {
        let dir = Temp(std::env::temp_dir().join(format!(
            "fdb-reuse-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&dir.0).unwrap();
        let mut index = crate::index::Index::default();
        let mut live = BTreeMap::new();
        for id in ["d", "b", "a", "c"] {
            let mut stored = doc(id, 1);
            stored.document.fields.extend([
                ("negative".into(), serde_json::json!(-17)),
                ("null".into(), serde_json::Value::Null),
                ("float".into(), serde_json::json!(1.25)),
                ("list".into(), serde_json::json!([1, "x"])),
            ]);
            index.change(&stored, true).unwrap();
            live.insert(stored.document.key.clone(), stored);
        }
        // Dense posting representation, mixed scope/collection dictionaries.
        for n in 0..130 {
            let mut stored = doc(&format!("k{n:03}"), 1);
            if n % 3 == 0 {
                stored.document.key.collection = "başka".into();
            }
            if n % 5 == 0 {
                stored.document.key.scope = "private".into();
                for target in &mut stored.document.links {
                    target.scope = "private".into();
                }
            }
            index.change(&stored, true).unwrap();
            live.insert(stored.document.key.clone(), stored);
        }
        for id in ["b", "a"] {
            let old = live.remove(&doc(id, 1).document.key).unwrap();
            index.change(&old, false).unwrap();
        }
        for id in ["z", "a"] {
            let stored = doc(id, 2);
            index.change(&stored, true).unwrap();
            live.insert(stored.document.key.clone(), stored);
        }
        let keys: BTreeSet<_> = live
            .keys()
            .cloned()
            .chain([doc("b", 2).document.key])
            .collect();
        let records: BTreeMap<_, _> = keys
            .iter()
            .enumerate()
            .map(|(slot, key)| (key.clone(), slot as u32))
            .collect();
        for reused in [false, true] {
            let name = if reused {
                "delta.idx"
            } else {
                "reference-delta.idx"
            };
            let mut builder = Builder::new(&dir.0.join(name), 2, 0, 0).unwrap();
            for key in &keys {
                if let Some(stored) = live.get(key) {
                    if reused {
                        builder.add_reused(stored).unwrap();
                    } else {
                        builder.add(stored).unwrap();
                    }
                } else {
                    builder.add_delete(key).unwrap();
                }
            }
            if reused {
                builder.finish_delta(&index, &records).unwrap();
            } else {
                builder.finish().unwrap();
            }
        }
        assert_eq!(
            std::fs::read(dir.0.join("delta.idx")).unwrap(),
            std::fs::read(dir.0.join("reference-delta.idx")).unwrap()
        );
        // Missing/nonmatching shadow mapping is rejected, without publishing an index.
        let mut bad = records.clone();
        bad.remove(&doc("a", 2).document.key);
        bad.insert(doc("wrong", 2).document.key, 0);
        let mut builder = Builder::new(&dir.0.join("bad.idx"), 2, 0, 0).unwrap();
        for key in &keys {
            if let Some(stored) = live.get(key) {
                builder.add_reused(stored).unwrap();
            } else {
                builder.add_delete(key).unwrap();
            }
        }
        assert!(builder.finish_delta(&index, &bad).is_err());
        assert!(!dir.0.join("bad.idx").exists());
    }
}
