//! Merge validated immutable posting streams, preserving the SEG03 layout.
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
        if &footer[..8] != FOOTER
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

pub(super) fn merge(
    inputs: &[(&DiskIndex, &[u32])],
    writer: &mut impl Write,
    offset: &mut u64,
) -> Result<Vec<(Term, Posting)>> {
    let mut iterators: Vec<_> = inputs
        .iter()
        .map(|(index, _)| index.postings.iter())
        .collect();
    let mut terms = BinaryHeap::new();
    for (n, iterator) in iterators.iter_mut().enumerate() {
        if let Some((term, posting)) = iterator.next() {
            terms.push(Reverse((term, n, posting.offset, posting.count)));
        }
    }
    let mut result = Vec::new();
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
            result.push((
                term.clone(),
                Posting {
                    offset: start,
                    count,
                },
            ));
        }
        for (n, _) in same {
            if let Some((next, posting)) = iterators[n].next() {
                terms.push(Reverse((next, n, posting.offset, posting.count)));
            }
        }
    }
    Ok(result)
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
}
