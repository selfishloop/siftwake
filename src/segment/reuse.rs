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

#[cfg(any(test, not(feature = "experimental-builder-budget")))]
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
    mut emit: impl FnMut(Term, Posting) -> Result<()>,
) -> Result<()> {
    let mut iterators: Vec<_> = inputs
        .iter()
        .map(|(index, _)| index.term_cursor())
        .collect();
    let mut terms = BinaryHeap::new();
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
        let mut cursors: Vec<_> = same
            .iter()
            .map(|(n, posting)| Cursor {
                index: inputs[*n].0,
                map: inputs[*n].1,
                posting: *posting,
                consumed: 0,
                buffer: [0; 8192],
                available: 0,
                position: 0,
                previous: None,
                mapped_previous: None,
            })
            .collect();
        let mut ids = BinaryHeap::new();
        for (n, cursor) in cursors.iter_mut().enumerate() {
            if let Some(id) = cursor.next()? {
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
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
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
