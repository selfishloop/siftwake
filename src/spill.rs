//! External posting sort: bounded batches, checksummed spill runs, streaming merge.
use crate::format::{crc, crc_extend};
use crate::segment::Term;
use crate::{Error, Result};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::fs::{File, OpenOptions};
use std::io::Seek;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
#[cfg(feature = "experimental-builder-budget")]
mod bounded;

pub(crate) struct Spills {
    stem: PathBuf,
    buffered: BTreeMap<Term, Vec<u32>>,
    paths: Vec<PathBuf>,
    #[cfg(feature = "experimental-builder-budget")]
    all_paths: Vec<PathBuf>,
    lengths: BTreeMap<PathBuf, u64>,
    estimated: usize,
    budget: usize,
}
impl Spills {
    pub fn new(stem: PathBuf, budget: usize) -> Self {
        Self {
            stem,
            buffered: BTreeMap::new(),
            paths: vec![],
            #[cfg(feature = "experimental-builder-budget")]
            all_paths: vec![],
            lengths: BTreeMap::new(),
            estimated: 0,
            budget,
        }
    }
    pub fn add(&mut self, term: Term, id: u32) -> Result<()> {
        use std::collections::btree_map::Entry;
        match self.buffered.entry(term) {
            Entry::Occupied(mut entry) => entry.get_mut().push(id),
            Entry::Vacant(entry) => {
                self.estimated += serde_json::to_vec(entry.key())?.len() + 128;
                entry.insert(vec![id]);
            }
        }
        self.estimated += 8;
        if self.estimated >= self.budget {
            self.flush()?;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        if self.buffered.is_empty() {
            return Ok(());
        }
        let mut name = self.stem.file_name().unwrap().to_os_string();
        name.push(format!(".spill-{}", self.paths.len()));
        let path = self.stem.with_file_name(name);
        let mut f = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?,
        );
        #[cfg(feature = "experimental-builder-budget")]
        self.all_paths.push(path.clone());
        self.paths.push(path);
        for (term, ids) in std::mem::take(&mut self.buffered) {
            let key = serde_json::to_vec(&term)?;
            let count = u32::try_from(ids.len())
                .map_err(|_| Error::Invalid("spill count overflow".into()))?;
            let mut state = !0;
            let mut block = [0; 4096];
            for chunk in ids.chunks(1024) {
                state = crc_extend(state, encode_ids(chunk, &mut block));
            }
            let mut header = (key.len() as u32).to_le_bytes().to_vec();
            header.extend_from_slice(&count.to_le_bytes());
            header.extend_from_slice(&crc(&key).to_le_bytes());
            header.extend_from_slice(&(!state).to_le_bytes());
            header.extend_from_slice(&crc(&header).to_le_bytes());
            f.write_all(&header)?;
            f.write_all(&key)?;
            for chunk in ids.chunks(1024) {
                f.write_all(encode_ids(chunk, &mut block))?;
            }
        }
        f.flush()?;
        self.lengths.insert(
            self.paths.last().unwrap().clone(),
            f.get_ref().metadata()?.len(),
        );
        self.estimated = 0;
        Ok(())
    }
    pub fn merge(
        &mut self,
        writer: &mut impl Write,
        offset: &mut u64,
    ) -> Result<Vec<(Term, u64, u32)>> {
        self.flush()?;
        #[cfg(feature = "experimental-builder-budget")]
        bounded::collapse(self)?;
        let mut readers: Vec<_> = self
            .paths
            .iter()
            .map(|p| self.cursor(p))
            .collect::<Result<_>>()?;
        let mut heap = BinaryHeap::new();
        for (n, reader) in readers.iter_mut().enumerate() {
            if let Some(term) = reader.advance()? {
                heap.push(Reverse((term, n)));
            }
        }
        let mut result = vec![];
        while let Some(Reverse((term, n))) = heap.pop() {
            let mut same = vec![n];
            while heap.peek().is_some_and(|Reverse((next, _))| next == &term) {
                let Reverse((_, n)) = heap.pop().unwrap();
                same.push(n);
            }
            same.sort_unstable();
            let start = *offset;
            let mut count = 0u32;
            let mut previous = None;
            for n in same {
                let reader = &mut readers[n];
                count = count
                    .checked_add(reader.remaining)
                    .ok_or_else(|| Error::Invalid("posting count overflow".into()))?;
                copy_ids(reader, &mut previous, |bytes| {
                    writer.write_all(bytes)?;
                    *offset += bytes.len() as u64;
                    Ok(())
                })?;
                if let Some(next) = reader.advance()? {
                    heap.push(Reverse((next, n)));
                }
            }
            result.push((term, start, count));
        }
        Ok(result)
    }
    fn cursor(&self, path: &PathBuf) -> Result<Cursor> {
        let expected = *self
            .lengths
            .get(path)
            .ok_or_else(|| Error::Corrupt("unregistered spill length".into()))?;
        Cursor::new(File::open(path)?, expected)
    }
}

fn encode_ids<'a>(ids: &[u32], block: &'a mut [u8; 4096]) -> &'a [u8] {
    for (id, destination) in ids.iter().zip(block.as_chunks_mut::<4>().0) {
        destination.copy_from_slice(&id.to_le_bytes());
    }
    &block[..ids.len() * 4]
}

fn copy_ids(
    reader: &mut Cursor,
    previous: &mut Option<u32>,
    mut write: impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    let mut block = [0; 4096];
    while reader.remaining != 0 {
        let count = reader.remaining.min(1024);
        let bytes = &mut block[..count as usize * 4];
        reader.input.read_exact(bytes)?;
        reader.state = crc_extend(reader.state, bytes);
        for encoded in bytes.as_chunks::<4>().0 {
            let id = u32::from_le_bytes(*encoded);
            if previous.is_some_and(|old| old >= id) {
                return Err(Error::Corrupt("unordered spill IDs".into()));
            }
            *previous = Some(id);
        }
        write(bytes)?;
        reader.remaining -= count;
    }
    if !reader.state != reader.expected {
        return Err(Error::Corrupt("spill posting checksum mismatch".into()));
    }
    Ok(())
}
impl Drop for Spills {
    fn drop(&mut self) {
        #[cfg(feature = "experimental-builder-budget")]
        let paths = &self.all_paths;
        #[cfg(not(feature = "experimental-builder-budget"))]
        let paths = &self.paths;
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
    }
}
struct Cursor {
    input: BufReader<File>,
    remaining: u32,
    expected: u32,
    state: u32,
    previous: Option<Term>,
    end: u64,
}

impl Cursor {
    fn new(file: File, expected: u64) -> Result<Self> {
        let end = file.metadata()?.len();
        if end == 0 || expected != end {
            return Err(Error::Corrupt("empty or truncated spill".into()));
        }
        Ok(Self {
            input: BufReader::new(file),
            remaining: 0,
            expected: 0,
            state: !0,
            previous: None,
            end,
        })
    }
    fn advance(&mut self) -> Result<Option<Term>> {
        let mut h = [0; 20];
        if self.input.read(&mut h[..1])? == 0 {
            if self.input.stream_position()? != self.end {
                return Err(Error::Corrupt("spill shortened while reading".into()));
            }
            return Ok(None);
        }
        self.input.read_exact(&mut h[1..])?;
        if crc(&h[..16]) != u32::from_le_bytes(h[16..].try_into().unwrap()) {
            return Err(Error::Corrupt("spill header checksum mismatch".into()));
        }
        let len = u32::from_le_bytes(h[..4].try_into().unwrap()) as usize;
        self.remaining = u32::from_le_bytes(h[4..8].try_into().unwrap());
        if len > 8 * 1024 * 1024 || self.remaining == 0 {
            return Err(Error::Corrupt("invalid spill header".into()));
        }
        let mut key = vec![0; len];
        self.input.read_exact(&mut key)?;
        if crc(&key) != u32::from_le_bytes(h[8..12].try_into().unwrap()) {
            return Err(Error::Corrupt("spill term checksum mismatch".into()));
        }
        let term: Term = serde_json::from_slice(&key).map_err(|e| Error::Corrupt(e.to_string()))?;
        if self.previous.as_ref().is_some_and(|old| old >= &term) {
            return Err(Error::Corrupt("unordered spill terms".into()));
        }
        self.previous = Some(term.clone());
        self.expected = u32::from_le_bytes(h[12..16].try_into().unwrap());
        self.state = !0;
        Ok(Some(term))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spills() -> Spills {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Spills::new(
            std::env::temp_dir().join(format!(
                "fdb-spill-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            )),
            1,
        )
    }
    #[test]
    fn spilled_groups_merge_in_term_and_id_order_and_are_removed() {
        let mut s = spills();
        for id in 0..20 {
            s.add(Term::Scope("b".into()), id).unwrap();
            s.add(Term::Scope("a".into()), id).unwrap();
        }
        assert_eq!(s.paths.len(), 40);
        let paths = s.paths.clone();
        let mut bytes = vec![];
        let mut offset = 0;
        let terms = s.merge(&mut bytes, &mut offset).unwrap();
        assert_eq!(
            terms,
            vec![
                (Term::Scope("a".into()), 0, 20),
                (Term::Scope("b".into()), 80, 20)
            ]
        );
        assert_eq!(offset, 160);
        for group in bytes.as_chunks::<80>().0 {
            let ids: Vec<_> = group
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| u32::from_le_bytes(*b))
                .collect();
            assert_eq!(ids, (0..20).collect::<Vec<_>>());
        }
        drop(s);
        assert!(paths.iter().all(|p| !p.exists()));
    }
    #[test]
    fn corrupt_spill_header_term_and_postings_fail_closed() {
        for position in [0, 20, 30] {
            let mut s = spills();
            s.add(Term::Scope("shared".into()), 7).unwrap();
            let path = &s.paths[0];
            let mut bytes = std::fs::read(path).unwrap();
            let position = if position == 30 {
                bytes.len() - 1
            } else {
                position
            };
            bytes[position] ^= 1;
            std::fs::write(path, bytes).unwrap();
            assert!(matches!(
                s.merge(&mut vec![], &mut 0),
                Err(Error::Corrupt(_))
            ));
        }
    }

    #[test]
    fn empty_and_complete_term_boundary_truncation_are_rejected() {
        let mut s = spills();
        s.budget = 1024;
        s.add(Term::Scope("a".into()), 0).unwrap();
        s.add(Term::Scope("b".into()), 0).unwrap();
        s.flush().unwrap();
        let path = s.paths[0].clone();
        let original = std::fs::read(&path).unwrap();
        let boundary = 20 + serde_json::to_vec(&Term::Scope("a".into())).unwrap().len() + 4;
        for cut in [0, boundary] {
            std::fs::write(&path, &original[..cut]).unwrap();
            assert!(matches!(
                s.merge(&mut vec![], &mut 0),
                Err(Error::Corrupt(_))
            ));
        }
    }

    #[test]
    fn chunk_boundaries_preserve_bytes_and_reject_duplicate_ids() {
        for duplicate in [false, true] {
            let mut s = spills();
            s.budget = 1024 * 1024;
            for id in 0..2051 {
                s.add(Term::Scope("blocks".into()), id).unwrap();
            }
            if duplicate {
                s.add(Term::Scope("blocks".into()), 2050).unwrap();
            }
            let mut bytes = vec![];
            let mut offset = 0;
            let result = s.merge(&mut bytes, &mut offset);
            if duplicate {
                assert!(matches!(result, Err(Error::Corrupt(_))));
            } else {
                assert_eq!(
                    result.unwrap(),
                    vec![(Term::Scope("blocks".into()), 0, 2051)]
                );
                assert_eq!(
                    bytes,
                    (0..2051u32).flat_map(u32::to_le_bytes).collect::<Vec<_>>()
                );
                assert_eq!(offset, 2051 * 4);
            }
        }
    }

    #[cfg(feature = "experimental-builder-budget")]
    #[test]
    fn multiple_stages_preserve_all_ids_and_release_every_owned_file() {
        let mut s = spills();
        for id in 0..1057 {
            s.add(Term::Scope("same".into()), id).unwrap();
        }
        let mut bytes = vec![];
        let mut offset = 0;
        assert_eq!(
            s.merge(&mut bytes, &mut offset).unwrap(),
            vec![(Term::Scope("same".into()), 0, 1057)]
        );
        assert_eq!(
            bytes,
            (0..1057u32).flat_map(u32::to_le_bytes).collect::<Vec<_>>()
        );
        assert!(s.paths.len() <= 32 && s.all_paths.len() > 1057 + 32);
        let paths = s.all_paths.clone();
        drop(s);
        assert!(paths.iter().all(|p| !p.exists()));
    }

    #[cfg(feature = "experimental-builder-budget")]
    #[test]
    fn every_staged_cut_and_crc_corruption_fail_closed_and_cleanup() {
        let mut s = spills();
        for id in 0..40 {
            s.add(Term::Scope("same".into()), id).unwrap();
        }
        bounded::collapse(&mut s).unwrap();
        let path = s.paths[0].clone();
        let original = std::fs::read(&path).unwrap();
        for cut in 0..original.len() {
            std::fs::write(&path, &original[..cut]).unwrap();
            // A zero-byte file must not silently disappear from a staged merge.
            assert!(s.merge(&mut vec![], &mut 0).is_err(), "cut {cut}");
        }
        for position in [0, 20, original.len() - 1] {
            let mut bytes = original.clone();
            bytes[position] ^= 1;
            std::fs::write(&path, &bytes).unwrap();
            assert!(s.merge(&mut vec![], &mut 0).is_err());
        }
        let paths = s.all_paths.clone();
        drop(s);
        assert!(paths.iter().all(|p| !p.exists()));
    }
}
