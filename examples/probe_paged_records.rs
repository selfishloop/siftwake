//! Experimental metadata paging only; not a production cache format or DB reader.
//! Uses a healthy SEG03 benchmark after normal Database validation. Output goes
//! into a new directory, never into the database. Timings exclude source loading.
use fdb::{Database, Key};
use serde::Deserialize;
use serde_json::json;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;

const ROWS: usize = 64;
const CACHE_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Record {
    key: Key,
    offset: u64,
    len: u32,
    checksum: u32,
    version: u64,
    deleted: bool,
}
#[derive(Deserialize)]
struct Directory {
    records: Vec<Record>,
}
struct Fence {
    first: Key,
    offset: u64,
    len: usize,
    checksum: u32,
}
struct Paged {
    file: File,
    fences: Vec<Fence>,
}
struct Cache {
    pages: VecDeque<((usize, usize), Vec<u8>)>,
    bytes: usize,
    reads: usize,
}
fn checksum(bytes: &[u8]) -> u32 {
    // FNV for this disposable experiment, not the production format's CRC.
    bytes.iter().fold(2166136261, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(16777619)
    })
}
fn encode(rows: &[Record]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = vec![0; 4 + (rows.len() + 1) * 4];
    bytes[..4].copy_from_slice(&u32::try_from(rows.len())?.to_le_bytes());
    for (index, row) in rows.iter().enumerate() {
        let offset = u32::try_from(bytes.len())?.to_le_bytes();
        bytes[4 + index * 4..8 + index * 4].copy_from_slice(&offset);
        for text in [&row.key.scope, &row.key.collection, &row.key.id] {
            bytes.extend_from_slice(&u32::try_from(text.len())?.to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
        bytes.extend_from_slice(&row.offset.to_le_bytes());
        bytes.extend_from_slice(&row.version.to_le_bytes());
        bytes.extend_from_slice(&row.len.to_le_bytes());
        bytes.extend_from_slice(&row.checksum.to_le_bytes());
        bytes.push(u8::from(row.deleted));
    }
    let end = u32::try_from(bytes.len())?.to_le_bytes();
    bytes[4 + rows.len() * 4..8 + rows.len() * 4].copy_from_slice(&end);
    Ok(bytes)
}
fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    if bytes.len() < len {
        return Err("truncated experimental page".into());
    }
    let (result, rest) = bytes.split_at(len);
    *bytes = rest;
    Ok(result)
}
fn string<'a>(bytes: &mut &'a [u8]) -> Result<&'a str, Box<dyn std::error::Error>> {
    let len = u32::from_le_bytes(take(bytes, 4)?.try_into()?) as usize;
    Ok(std::str::from_utf8(take(bytes, len)?)?)
}
fn scan(mut bytes: &[u8], key: &Key) -> ProbeResult<Option<Metadata>> {
    let all = bytes;
    let count = u32::from_le_bytes(take(&mut bytes, 4)?.try_into()?) as usize;
    if count > ROWS {
        return Err("invalid experimental page row count".into());
    }
    let offsets = take(&mut bytes, (count + 1) * 4)?;
    let at = |index: usize| -> ProbeResult<usize> {
        Ok(u32::from_le_bytes(offsets[index * 4..index * 4 + 4].try_into()?) as usize)
    };
    if at(0)? != 4 + (count + 1) * 4 || at(count)? != all.len() {
        return Err("invalid experimental page bounds".into());
    }
    let (mut lo, mut hi) = (0, count);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let mut row = all
            .get(at(mid)?..at(mid + 1)?)
            .ok_or("invalid row bounds")?;
        let scope = string(&mut row)?;
        let collection = string(&mut row)?;
        let id = string(&mut row)?;
        let metadata = take(&mut row, 25)?;
        if !row.is_empty() {
            return Err("extra experimental row bytes".into());
        }
        if metadata[24] > 1 {
            return Err("invalid experimental deletion flag".into());
        }
        let candidate = (scope, collection, id);
        let wanted = (key.scope.as_str(), key.collection.as_str(), key.id.as_str());
        match candidate.cmp(&wanted) {
            std::cmp::Ordering::Equal => {
                return Ok(Some((
                    u64::from_le_bytes(metadata[..8].try_into()?),
                    u64::from_le_bytes(metadata[8..16].try_into()?),
                    u32::from_le_bytes(metadata[16..20].try_into()?),
                    u32::from_le_bytes(metadata[20..24].try_into()?),
                    metadata[24] == 1,
                )));
            }
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Less => lo = mid + 1,
        }
    }
    Ok(None)
}
type Metadata = (u64, u64, u32, u32, bool);
type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;
impl Paged {
    fn build(path: &Path, rows: &[Record]) -> ProbeResult<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        let mut fences = Vec::with_capacity(rows.len().div_ceil(ROWS));
        let mut offset = 0;
        for chunk in rows.chunks(ROWS) {
            let bytes = encode(chunk)?;
            file.write_all(&bytes)?;
            fences.push(Fence {
                first: chunk[0].key.clone(),
                offset,
                len: bytes.len(),
                checksum: checksum(&bytes),
            });
            offset += bytes.len() as u64;
        }
        file.sync_all()?;
        Ok(Self { file, fences })
    }
    fn find(&mut self, run: usize, key: &Key, cache: &mut Cache) -> ProbeResult<Option<Metadata>> {
        let next = self.fences.partition_point(|f| f.first <= *key);
        if next == 0 {
            return Ok(None);
        }
        let page = next - 1;
        let identity = (run, page);
        if let Some(index) = cache.pages.iter().position(|(id, _)| *id == identity) {
            let item = cache.pages.remove(index).ok_or("missing cached page")?;
            cache.pages.push_front(item);
            return scan(&cache.pages[0].1, key);
        }
        let fence = &self.fences[page];
        let mut bytes = vec![0; fence.len];
        self.file.seek(SeekFrom::Start(fence.offset))?;
        self.file.read_exact(&mut bytes)?;
        cache.reads += 1;
        if checksum(&bytes) != fence.checksum {
            return Err("experimental page checksum mismatch".into());
        }
        let result = scan(&bytes, key)?;
        if bytes.len() <= CACHE_BYTES {
            while cache.bytes + bytes.len() > CACHE_BYTES {
                let (_, old) = cache.pages.pop_back().ok_or("cache accounting error")?;
                cache.bytes -= old.len();
            }
            cache.bytes += bytes.len();
            cache.pages.push_front((identity, bytes));
        }
        Ok(result)
    }
    fn fence_capacity(&self) -> usize {
        self.fences.capacity() * std::mem::size_of::<Fence>()
            + self
                .fences
                .iter()
                .map(|f| {
                    f.first.scope.capacity() + f.first.collection.capacity() + f.first.id.capacity()
                })
                .sum::<usize>()
    }
}
fn metadata(row: &Record) -> Metadata {
    (row.offset, row.version, row.len, row.checksum, row.deleted)
}
fn main() -> ProbeResult<()> {
    let args: Vec<_> = std::env::args().collect();
    let input = args
        .get(1)
        .ok_or("probe_paged_records <healthy-db> <new-output-dir>")?;
    let output = Path::new(args.get(2).ok_or("missing new output directory")?);
    std::fs::create_dir(output)?;
    let db = Database::open(input)?;
    let layouts = db.index_memory_layout()?;
    drop(db);
    let mut resident = Vec::new();
    let mut paged = Vec::new();
    for layout in &layouts {
        let mut file =
            File::open(Path::new(input).join(format!("run-{:016x}.idx", layout.source_id)))?;
        let mut footer = [0; 32];
        file.seek(SeekFrom::End(-32))?;
        file.read_exact(&mut footer)?;
        if &footer[..8] != b"FDBEND03" {
            return Err("requires validated SEG03 run".into());
        }
        let offset = u64::from_le_bytes(footer[8..16].try_into()?);
        let len = u64::from_le_bytes(footer[16..24].try_into()?);
        let mut bytes = vec![0; usize::try_from(len)?];
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(&mut bytes)?;
        let directory: Directory = serde_json::from_slice(&bytes)?;
        if directory.records.len() != layout.records {
            return Err("source changed during probe".into());
        }
        paged.push(Paged::build(
            &output.join(format!("{}.probe", layout.source_id)),
            &directory.records,
        )?);
        resident.push(directory.records);
    }
    let mut cache = Cache {
        pages: VecDeque::new(),
        bytes: 0,
        reads: 0,
    };
    let fences: usize = paged.iter().map(Paged::fence_capacity).sum();
    println!(
        "{}",
        json!({"phase":"layout", "records":resident.iter().map(Vec::len).sum::<usize>(),
        "runs":paged.len(), "original_selected_record_id_capacity_bytes":layouts.iter().map(|s|s.record_vector_capacity_bytes+s.id_pool_capacity_bytes).sum::<usize>(),
        "prototype_fence_capacity_bytes":fences, "shared_page_payload_budget_bytes":CACHE_BYTES,
        "not_total_heap_or_rss":true,"fences_still_grow_linearly":true})
    );
    // Deterministic arbitrary-order samples, then a locality-heavy first-page sample.
    for locality in [false, true] {
        let probes: Vec<_> = (0..20000usize)
            .map(|i| {
                let run = i % resident.len();
                let slot = if locality {
                    (i / resident.len()) % 50.min(resident[run].len())
                } else {
                    (i.wrapping_mul(2654435761) / resident.len()) % resident[run].len()
                };
                (run, &resident[run][slot].key)
            })
            .collect();
        // Warm OS cache from conversion; clear only the prototype cache.
        cache.pages.clear();
        cache.bytes = 0;
        cache.reads = 0;
        let expected: Vec<_> = probes
            .iter()
            .map(|(run, key)| {
                let index = resident[*run]
                    .binary_search_by(|r| r.key.cmp(key))
                    .expect("sample key");
                metadata(&resident[*run][index])
            })
            .collect();
        let start = Instant::now();
        for ((run, key), expected) in probes.iter().zip(&expected) {
            let rows = std::hint::black_box(&resident[*run]);
            let index = rows
                .binary_search_by(|r| r.key.cmp(key))
                .expect("sample key");
            assert_eq!(metadata(&rows[index]), *expected);
        }
        let resident_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        for ((run, key), expected) in probes.iter().zip(&expected) {
            assert_eq!(paged[*run].find(*run, key, &mut cache)?, Some(*expected));
        }
        let paged_ms = start.elapsed().as_secs_f64() * 1000.;
        println!(
            "{}",
            json!({"phase":"lookup","workload":if locality {"first_50_per_run"}else{"scattered_existing_keys"},
            "lookups":probes.len(),"resident_ms":resident_ms,"paged_ms":paged_ms,"page_reads":cache.reads,
            "cached_page_payload_bytes":cache.bytes,"warm_os_cache":true,"end_to_end_query":false})
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_boundaries_tombstones_shared_budget_and_io_failures() {
        let path =
            std::env::temp_dir().join(format!("fdb-paging-probe-{}.bin", std::process::id()));
        let rows: Vec<_> = (0..6000)
            .map(|i| Record {
                key: Key::new("shared", "contents", &format!("{i:08}")),
                offset: i,
                version: 7,
                len: if i % 5 == 0 { 0 } else { 42 },
                checksum: 17,
                deleted: i % 5 == 0,
            })
            .collect();
        let mut paged = Paged::build(&path, &rows).unwrap();
        let mut cache = Cache {
            pages: VecDeque::new(),
            bytes: 0,
            reads: 0,
        };
        for (id, row) in rows.iter().enumerate() {
            assert_eq!(
                paged.find(0, &row.key, &mut cache).unwrap(),
                Some(metadata(row))
            );
            assert!(cache.bytes <= CACHE_BYTES);
            assert_eq!(
                cache.bytes,
                cache.pages.iter().map(|(_, b)| b.len()).sum::<usize>()
            );
            if id == 64 {
                assert_eq!(cache.reads, 2);
            }
        }
        assert!(cache.reads > cache.pages.len());
        assert_eq!(
            paged.find(0, &Key::new("a", "b", "c"), &mut cache).unwrap(),
            None
        );
        assert_eq!(
            paged.find(0, &Key::new("z", "b", "c"), &mut cache).unwrap(),
            None
        );
        cache.pages.clear();
        cache.bytes = 0;
        paged.file.seek(SeekFrom::Start(0)).unwrap();
        paged.file.write_all(&[255]).unwrap();
        assert!(paged.find(0, &rows[0].key, &mut cache).is_err());
        paged.file.set_len(0).unwrap();
        assert!(paged.find(0, &rows[128].key, &mut cache).is_err());
        drop(paged);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn unicode_keys_metadata_and_malformed_pages() {
        let row = Record {
            key: Key::new("user:ş", "文書", "😀-ı"),
            offset: 123,
            version: 9,
            len: 42,
            checksum: 17,
            deleted: false,
        };
        let bytes = encode(std::slice::from_ref(&row)).unwrap();
        assert_eq!(scan(&bytes, &row.key).unwrap(), Some(metadata(&row)));
        assert_eq!(
            scan(&bytes, &Key::new("user:ş", "文書", "missing")).unwrap(),
            None
        );
        assert!(scan(&bytes[..bytes.len() - 1], &row.key).is_err());
        let mut invalid = bytes.clone();
        invalid[4] = 255;
        assert!(scan(&invalid, &row.key).is_err());
        assert_ne!(checksum(&bytes), checksum(&invalid));
    }
}
