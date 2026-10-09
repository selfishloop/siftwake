//! Immutable, derived disk index. Full contents and posting lists stay on disk.
//! The transaction log is authoritative; missing/corrupt segments are rebuilt.
use crate::format::{crc, prefix_crc};
use crate::index::{fold, grams};
use crate::{Error, Key, Query, Result, StoredDocument};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: &[u8; 8] = b"FDBSEG03";
const FOOTER: &[u8; 8] = b"FDBEND03";
const HEADER_SIZE: u64 = 28;
const FOOTER_SIZE: u64 = 32;
const MAX_DIRECTORY: u64 = 256 * 1024 * 1024;
#[cfg(all(test, feature = "experimental-grouped-read"))]
mod grouped_tests;
#[cfg(feature = "experimental-paged-metadata")]
mod paged;
#[cfg(feature = "experimental-write-record-spool")]
mod record_spool;
#[cfg(feature = "experimental-merge-postings")]
mod reuse;
#[cfg(feature = "experimental-streaming-open")]
mod streaming;
static TEMP: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "experimental-grouped-read")]
const READ_WINDOW: usize = 64 * 1024;
#[cfg(feature = "experimental-grouped-read")]
const READ_GAP: u64 = 4096;

#[cfg(feature = "experimental-grouped-read")]
#[derive(Default, Debug)]
pub(crate) struct BatchReadStats {
    pub requests: usize,
    pub requested_bytes: u64,
    pub oversized_documents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Term {
    Scope(String),
    Collection(String, String),
    Exact(String, String, String, String),
    Numeric(String, String, String, i64),
    Gram(String, String),
    Link(Key),
}

fn terms(stored: &StoredDocument, body: &crate::index::BodyTerms) -> Vec<Term> {
    let d = &stored.document;
    let k = &d.key;
    let mut result = vec![
        Term::Scope(k.scope.clone()),
        Term::Collection(k.scope.clone(), k.collection.clone()),
    ];
    for (field, value) in &d.fields {
        if value.is_array() || value.is_object() {
            continue;
        }
        result.push(Term::Exact(
            k.scope.clone(),
            k.collection.clone(),
            field.clone(),
            value.to_string(),
        ));
        if let Some(n) = value.as_i64() {
            result.push(Term::Numeric(
                k.scope.clone(),
                k.collection.clone(),
                field.clone(),
                n,
            ));
        }
    }
    let title = grams(&fold(&d.title));
    for gram in title.union(&body.grams) {
        result.push(Term::Gram(k.scope.clone(), gram.clone()));
    }
    for key in d.links.iter().collect::<BTreeSet<_>>() {
        result.push(Term::Link(key.clone()));
    }
    result
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    key: Key,
    offset: u64,
    len: u32,
    checksum: u32,
    version: u64,
    deleted: bool,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Posting {
    offset: u64,
    count: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(any(not(feature = "experimental-write-record-spool"), test))]
struct Directory {
    records: Vec<Record>,
    postings: Vec<(Term, Posting)>,
}

pub(crate) struct Builder {
    temporary: PathBuf,
    target: PathBuf,
    writer: Option<BufWriter<File>>,
    offset: u64,
    sequence: u64,
    #[cfg(not(feature = "experimental-write-record-spool"))]
    records: Vec<Record>,
    #[cfg(feature = "experimental-write-record-spool")]
    records: record_spool::Records,
    postings: crate::spill::Spills,
    body_cache: crate::index::BodyCache,
    // Leases are last: release reservations after owned buffers and spill files.
    #[cfg(feature = "experimental-write-buffer-budget")]
    _write_lease: crate::write_budget::Lease<'static>,
    #[cfg(feature = "experimental-builder-budget")]
    _posting_lease: crate::write_budget::Lease<'static>,
}

impl Builder {
    pub fn new(
        target: &Path,
        sequence: u64,
        checkpoint_end: u64,
        checkpoint_crc: u32,
    ) -> Result<Self> {
        // Acquire posting quota before I/O quota: waiters must not hold all I/O
        // reservations while active builders need I/O to finish and release.
        #[cfg(feature = "experimental-builder-budget")]
        let posting_lease = crate::write_budget::reserve_postings()?;
        #[cfg(feature = "experimental-write-buffer-budget")]
        let write_lease = crate::write_budget::reserve(16 * 1024, 16 * 1024)?;
        let temporary = target.with_extension(format!(
            "build-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let mut writer = BufWriter::with_capacity(8 * 1024, file);
        writer.write_all(MAGIC)?;
        writer.write_all(&sequence.to_le_bytes())?;
        writer.write_all(&checkpoint_end.to_le_bytes())?;
        writer.write_all(&checkpoint_crc.to_le_bytes())?;
        Ok(Self {
            #[cfg(feature = "experimental-write-buffer-budget")]
            _write_lease: write_lease,
            #[cfg(feature = "experimental-builder-budget")]
            postings: crate::spill::Spills::new(temporary.clone(), posting_lease.bytes),
            #[cfg(not(feature = "experimental-builder-budget"))]
            postings: crate::spill::Spills::new(temporary.clone(), 4 * 1024 * 1024),
            #[cfg(feature = "experimental-builder-budget")]
            _posting_lease: posting_lease,
            #[cfg(feature = "experimental-write-record-spool")]
            records: record_spool::Records::new(&temporary)?,
            temporary,
            target: target.into(),
            writer: Some(writer),
            offset: HEADER_SIZE,
            sequence,
            #[cfg(not(feature = "experimental-write-record-spool"))]
            records: vec![],
            body_cache: crate::index::BodyCache::default(),
        })
    }

    fn record_count(&self) -> usize {
        #[cfg(feature = "experimental-write-record-spool")]
        {
            self.records.count
        }
        #[cfg(not(feature = "experimental-write-record-spool"))]
        {
            self.records.len()
        }
    }
    fn last_record_key(&self) -> Option<&Key> {
        #[cfg(feature = "experimental-write-record-spool")]
        {
            self.records.last.as_ref()
        }
        #[cfg(not(feature = "experimental-write-record-spool"))]
        {
            self.records.last().map(|r| &r.key)
        }
    }
    fn push_record(&mut self, record: Record) -> Result<()> {
        #[cfg(feature = "experimental-write-record-spool")]
        {
            self.records.push(record)
        }
        #[cfg(not(feature = "experimental-write-record-spool"))]
        {
            self.records.push(record);
            Ok(())
        }
    }
    pub fn add(&mut self, stored: &StoredDocument) -> Result<()> {
        self.add_document(stored, true)
    }

    #[cfg(feature = "experimental-merge-postings")]
    pub(crate) fn add_reused(&mut self, stored: &StoredDocument) -> Result<()> {
        self.add_document(stored, false)
    }

    fn add_document(&mut self, stored: &StoredDocument, index_terms: bool) -> Result<()> {
        #[cfg(feature = "diagnostic-memory")]
        let _origin = crate::memory::Scope::new(6);
        #[cfg(feature = "diagnostic-timing")]
        let _add = crate::timing::span("builder.add");
        crate::validate_doc(&stored.document)?;
        if stored.version == 0 || stored.version > self.sequence {
            return Err(Error::Corrupt("invalid segment document version".into()));
        }
        if self
            .last_record_key()
            .is_some_and(|key| key >= &stored.document.key)
        {
            return Err(Error::Corrupt(
                "checkpoint keys must be strictly sorted".into(),
            ));
        }
        let id = u32::try_from(self.record_count())
            .map_err(|_| Error::Invalid("segment exceeds u32 record capacity".into()))?;
        let bytes = serde_json::to_vec(stored)?;
        self.writer.as_mut().unwrap().write_all(&bytes)?;
        self.push_record(Record {
            key: stored.document.key.clone(),
            offset: self.offset,
            len: bytes.len() as u32,
            checksum: crc(&bytes),
            version: stored.version,
            deleted: false,
        })?;
        self.offset += bytes.len() as u64;
        if index_terms {
            let body = self.body_cache.get(&stored.document.text);
            for term in terms(stored, &body) {
                self.postings.add(term, id)?;
            }
        }
        Ok(())
    }

    pub fn add_delete(&mut self, key: &Key) -> Result<()> {
        crate::validate_key(key)?;
        if self.last_record_key().is_some_and(|last| last >= key) {
            return Err(Error::Corrupt("unordered tombstone key".into()));
        }
        self.push_record(Record {
            key: key.clone(),
            offset: self.offset,
            len: 0,
            checksum: 0,
            version: self.sequence,
            deleted: true,
        })?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<DiskIndex> {
        #[cfg(feature = "diagnostic-memory")]
        let _origin = crate::memory::Scope::new(6);
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("builder.finish");
        let writer = self.writer.as_mut().unwrap();
        let postings: Vec<_> = self
            .postings
            .merge(writer, &mut self.offset)?
            .into_iter()
            .map(|(term, offset, count)| (term, Posting { offset, count }))
            .collect();
        self.finish_directory(postings)
    }

    #[cfg(feature = "experimental-merge-postings")]
    pub(crate) fn finish_reused(mut self, inputs: &[(&DiskIndex, &[u32])]) -> Result<DiskIndex> {
        let postings = reuse::merge(inputs, self.writer.as_mut().unwrap(), &mut self.offset)?;
        self.finish_directory(postings)
    }

    fn finish_directory(mut self, postings: Vec<(Term, Posting)>) -> Result<DiskIndex> {
        let writer = self.writer.as_mut().unwrap();
        let directory_offset = self.offset;
        #[cfg(not(feature = "experimental-write-record-spool"))]
        let directory = Directory {
            records: std::mem::take(&mut self.records),
            postings,
        };
        let mut sink = crate::bounded::Sink::new(&mut *writer, MAX_DIRECTORY as usize);
        #[cfg(not(feature = "experimental-write-record-spool"))]
        let encoded = serde_json::to_writer(&mut sink, &directory).map_err(Error::from);
        #[cfg(feature = "experimental-write-record-spool")]
        let encoded: Result<()> = (|| {
            sink.write_all(b"{\"records\":")?;
            self.records.write_json(&mut sink)?;
            sink.write_all(b",\"postings\":")?;
            serde_json::to_writer(&mut sink, &postings)?;
            sink.write_all(b"}")?;
            Ok(())
        })();
        if sink.exceeded {
            return Err(Error::Invalid("segment directory exceeds 256 MiB".into()));
        }
        if let Some(error) = sink.failure.take() {
            return Err(Error::Io(error));
        }
        encoded?;
        let directory_len = sink.written as u64;
        #[cfg(not(feature = "experimental-write-record-spool"))]
        drop(directory);
        #[cfg(feature = "experimental-write-record-spool")]
        drop(postings);
        writer.flush()?;
        let end = directory_offset + directory_len;
        let checksum = prefix_crc(writer.get_mut(), end)?;
        // prefix_crc changed the file position; flush already completed above.
        writer.get_mut().seek(SeekFrom::Start(end))?;
        let mut footer = FOOTER.to_vec();
        footer.extend_from_slice(&directory_offset.to_le_bytes());
        footer.extend_from_slice(&directory_len.to_le_bytes());
        footer.extend_from_slice(&checksum.to_le_bytes());
        footer.extend_from_slice(&crc(&footer).to_le_bytes());
        writer.write_all(&footer)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
        self.writer.take();
        let index = DiskIndex::open(&self.temporary, None)?;
        if self.target.exists() {
            std::fs::remove_file(&self.target)?;
        }
        std::fs::rename(&self.temporary, &self.target)?;
        Ok(index)
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        self.writer.take();
        let _ = std::fs::remove_file(&self.temporary);
    }
}

#[derive(Clone, Copy)]
struct PackedRecord {
    offset: u64,
    version: u64,
    group: u32,
    id_offset: u32,
    id_len: u32,
    len: u32,
    checksum: u32,
    deleted: bool,
}

pub(crate) struct DiskIndex {
    file: File,
    #[cfg(feature = "experimental-paged-metadata")]
    paged: Option<paged::PagedRecords>,
    records: Vec<PackedRecord>,
    groups: Vec<(String, String)>,
    ids: String,
    postings: BTreeMap<Term, Posting>,
    pub sequence: u64,
    pub bytes: u64,
    pub posting_entries: u64,
    live_records: usize,
}

// Positional I/O leaves the shared file cursor untouched and avoids unsafe mmap.
pub(crate) fn read_at(file: &File, mut offset: u64, mut buffer: &mut [u8]) -> Result<()> {
    #[cfg(feature = "diagnostic-timing")]
    let _read = crate::timing::span("disk.read_at");
    while !buffer.is_empty() {
        #[cfg(windows)]
        let n = {
            use std::os::windows::fs::FileExt;
            file.seek_read(buffer, offset)?
        };
        #[cfg(unix)]
        let n = {
            use std::os::unix::fs::FileExt;
            file.read_at(buffer, offset)?
        };
        if n == 0 {
            return Err(Error::Corrupt("short segment read".into()));
        }
        offset += n as u64;
        buffer = &mut buffer[n..];
    }
    Ok(())
}

type KeyParts<'a> = (&'a str, &'a str, &'a str);

impl DiskIndex {
    pub(crate) fn memory_layout(&self, source_id: u64) -> crate::IndexMemoryLayout {
        let key_bytes =
            |key: &Key| key.scope.capacity() + key.collection.capacity() + key.id.capacity();
        let term_string_capacity_bytes = self
            .postings
            .keys()
            .map(|term| match term {
                Term::Scope(s) => s.capacity(),
                Term::Collection(s, c) | Term::Gram(s, c) => s.capacity() + c.capacity(),
                Term::Exact(s, c, f, v) => {
                    s.capacity() + c.capacity() + f.capacity() + v.capacity()
                }
                Term::Numeric(s, c, f, _) => s.capacity() + c.capacity() + f.capacity(),
                Term::Link(key) => key_bytes(key),
            })
            .sum();
        crate::IndexMemoryLayout {
            source_id,
            records: self.len(),
            record_vector_capacity_bytes: self.records.capacity()
                * std::mem::size_of::<PackedRecord>(),
            id_pool_capacity_bytes: self.ids.capacity(),
            group_string_capacity_bytes: self
                .groups
                .iter()
                .map(|(s, c)| s.capacity() + c.capacity())
                .sum(),
            #[cfg(feature = "experimental-paged-metadata")]
            paged_root_capacity_bytes: self.paged.as_ref().map_or(0, |p| p.capacities().0),
            #[cfg(feature = "experimental-paged-metadata")]
            paged_deletion_capacity_bytes: self.paged.as_ref().map_or(0, |p| p.capacities().1),
            terms: self.postings.len(),
            term_string_capacity_bytes,
        }
    }
    pub fn open(path: &Path, binding: Option<(u64, u32)>) -> Result<Self> {
        Self::open_with_mode(
            path,
            binding,
            #[cfg(feature = "experimental-streaming-metadata")]
            true,
        )
    }
    fn open_with_mode(
        path: &Path,
        binding: Option<(u64, u32)>,
        #[cfg(feature = "experimental-streaming-metadata")] streamed: bool,
    ) -> Result<Self> {
        #[cfg(feature = "diagnostic-memory")]
        let _origin = crate::memory::Scope::new(7);
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("segment.open");
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();
        if size < HEADER_SIZE + FOOTER_SIZE {
            return Err(Error::Corrupt("truncated segment".into()));
        }
        let mut header = [0; HEADER_SIZE as usize];
        read_at(&file, 0, &mut header)?;
        if &header[..8] != MAGIC {
            return Err(Error::Corrupt("unsupported segment format".into()));
        }
        let sequence = u64::from_le_bytes(header[8..16].try_into().unwrap());
        let end = u64::from_le_bytes(header[16..24].try_into().unwrap());
        let log_crc = u32::from_le_bytes(header[24..28].try_into().unwrap());
        if binding.is_some_and(|expected| expected != (end, log_crc)) {
            return Err(Error::Corrupt("segment/checkpoint binding mismatch".into()));
        }
        let mut footer = [0; FOOTER_SIZE as usize];
        read_at(&file, size - FOOTER_SIZE, &mut footer)?;
        if &footer[..8] != FOOTER
            || crc(&footer[..28]) != u32::from_le_bytes(footer[28..32].try_into().unwrap())
        {
            return Err(Error::Corrupt("invalid segment footer".into()));
        }
        let offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());
        let len = u64::from_le_bytes(footer[16..24].try_into().unwrap());
        if offset < HEADER_SIZE
            || len > MAX_DIRECTORY
            || offset.checked_add(len) != Some(size - FOOTER_SIZE)
        {
            return Err(Error::Corrupt("invalid segment directory bounds".into()));
        }
        if prefix_crc(&mut file, size - FOOTER_SIZE)?
            != u32::from_le_bytes(footer[24..28].try_into().unwrap())
        {
            return Err(Error::Corrupt("segment checksum mismatch".into()));
        }
        #[cfg(feature = "experimental-streaming-metadata")]
        if streamed {
            let (paged, parsed) =
                paged::prepare(path, &mut file, size, crc(&footer), sequence, offset, len)?;
            let bytes = size
                .checked_add(paged.bytes)
                .ok_or_else(|| Error::Corrupt("index byte count overflow".into()))?;
            return Ok(Self {
                file,
                paged: Some(paged),
                records: Vec::new(),
                groups: Vec::new(),
                ids: String::new(),
                postings: parsed.postings,
                sequence,
                bytes,
                posting_entries: parsed.entries,
                live_records: parsed.live,
            });
        }
        #[cfg(feature = "experimental-streaming-open")]
        {
            let parsed = streaming::read(&mut file, offset, len, sequence)?;
            Ok(Self {
                file,
                paged: None,
                records: parsed.records,
                groups: parsed.groups,
                ids: parsed.ids,
                postings: parsed.postings,
                sequence,
                bytes: size,
                posting_entries: parsed.entries,
                live_records: parsed.live,
            })
        }
        #[cfg(not(feature = "experimental-streaming-open"))]
        {
            let mut bytes = vec![0; len as usize];
            read_at(&file, offset, &mut bytes)?;
            let directory: Directory =
                serde_json::from_slice(&bytes).map_err(|e| Error::Corrupt(e.to_string()))?;
            drop(bytes);
            let mut cursor = HEADER_SIZE;
            let mut previous_key = None;
            let mut id_bytes = 0usize;
            for record in &directory.records {
                crate::validate_key(&record.key).map_err(|e| Error::Corrupt(e.to_string()))?;
                if record.offset != cursor
                    || (record.deleted != (record.len == 0))
                    || record.len > 4 * 1024 * 1024 + 128
                    || record.version == 0
                    || record.version > sequence
                    || previous_key.is_some_and(|key: &Key| key >= &record.key)
                {
                    return Err(Error::Corrupt("invalid segment record directory".into()));
                }
                cursor = cursor
                    .checked_add(record.len as u64)
                    .ok_or_else(|| Error::Corrupt("segment offset overflow".into()))?;
                previous_key = Some(&record.key);
                id_bytes = id_bytes
                    .checked_add(record.key.id.len())
                    .ok_or_else(|| Error::Corrupt("key pool length overflow".into()))?;
            }
            let mut previous_term = None;
            let mut entries = 0u64;
            for (term, p) in &directory.postings {
                if p.offset != cursor
                    || p.count == 0
                    || p.count as usize > directory.records.len()
                    || previous_term.is_some_and(|t: &Term| t >= term)
                {
                    return Err(Error::Corrupt("invalid segment posting directory".into()));
                }
                cursor = cursor
                    .checked_add(p.count as u64 * 4)
                    .ok_or_else(|| Error::Corrupt("segment posting overflow".into()))?;
                entries += p.count as u64;
                previous_term = Some(term);
            }
            if cursor != offset {
                return Err(Error::Corrupt("noncontiguous segment data".into()));
            }
            let live_records = directory.records.iter().filter(|r| !r.deleted).count();
            let mut groups = vec![];
            let mut group_ids = BTreeMap::new();
            let mut ids = String::with_capacity(id_bytes);
            let mut records = Vec::with_capacity(directory.records.len());
            for r in directory.records {
                let pair = (r.key.scope, r.key.collection);
                let group = if let Some(&id) = group_ids.get(&pair) {
                    id
                } else {
                    let id = groups.len() as u32;
                    group_ids.insert(pair.clone(), id);
                    groups.push(pair);
                    id
                };
                let id_offset = u32::try_from(ids.len())
                    .map_err(|_| Error::Invalid("key pool exceeds u32 offsets".into()))?;
                let id_len = r.key.id.len() as u32;
                ids.push_str(&r.key.id);
                records.push(PackedRecord {
                    offset: r.offset,
                    version: r.version,
                    group,
                    id_offset,
                    id_len,
                    len: r.len,
                    checksum: r.checksum,
                    deleted: r.deleted,
                });
            }
            Ok(Self {
                file,
                #[cfg(feature = "experimental-paged-metadata")]
                paged: None,
                records,
                groups,
                ids,
                postings: directory.postings.into_iter().collect(),
                sequence,
                bytes: size,
                posting_entries: entries,
                live_records,
            })
        }
    }

    #[cfg(feature = "experimental-paged-metadata")]
    pub fn enable_paging(&mut self, path: &Path) -> Result<()> {
        if self.paged.is_some() {
            return Ok(());
        }
        let paged = paged::PagedRecords::prepare(path, self)?;
        self.bytes = self
            .bytes
            .checked_add(paged.bytes)
            .ok_or_else(|| Error::Corrupt("index byte count overflow".into()))?;
        self.paged = Some(paged);
        self.records = Vec::new();
        self.ids = String::new();
        self.groups = Vec::new();
        Ok(())
    }
    pub fn len(&self) -> usize {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.len();
        }
        self.records.len()
    }
    fn key_parts(&self, id: u32) -> (&str, &str, &str) {
        let r = &self.records[id as usize];
        let (scope, collection) = &self.groups[r.group as usize];
        (
            scope,
            collection,
            &self.ids[r.id_offset as usize..(r.id_offset + r.id_len) as usize],
        )
    }
    fn metadata(&self, id: u32) -> Result<PackedRecord> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            let r = paged.record(id)?;
            return Ok(PackedRecord {
                offset: r.offset,
                version: r.version,
                len: r.len,
                checksum: r.checksum,
                deleted: r.deleted,
                group: 0,
                id_offset: 0,
                id_len: 0,
            });
        }
        self.records
            .get(id as usize)
            .copied()
            .ok_or_else(|| Error::Corrupt("invalid metadata slot".into()))
    }
    #[cfg(feature = "experimental-paged-metadata")]
    fn record(&self, id: u32) -> Result<Record> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.record(id);
        }
        let r = self
            .records
            .get(id as usize)
            .ok_or_else(|| Error::Corrupt("invalid metadata slot".into()))?;
        Ok(Record {
            key: self.key(id)?,
            offset: r.offset,
            len: r.len,
            checksum: r.checksum,
            version: r.version,
            deleted: r.deleted,
        })
    }
    pub fn key(&self, id: u32) -> Result<Key> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return Ok(paged.record(id)?.key);
        }
        if id as usize >= self.records.len() {
            return Err(Error::Corrupt("invalid key slot".into()));
        }
        let (scope, collection, id) = self.key_parts(id);
        Ok(Key::new(scope, collection, id))
    }
    pub fn version(&self, key: &Key) -> Result<Option<u64>> {
        match self.find(key)? {
            Some(id) if !self.deleted(id) => Ok(Some(self.metadata(id)?.version)),
            _ => Ok(None),
        }
    }
    pub fn contains_key(&self, key: &Key) -> Result<bool> {
        Ok(self.find(key)?.is_some())
    }
    pub fn deleted(&self, id: u32) -> bool {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.deleted(id);
        }
        self.records[id as usize].deleted
    }
    pub fn find(&self, key: &Key) -> Result<Option<u32>> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.find(key);
        }
        Ok(self.find_parts((key.scope.as_str(), key.collection.as_str(), key.id.as_str())))
    }
    fn find_parts(&self, key: (&str, &str, &str)) -> Option<u32> {
        if self.records.is_empty()
            || key < self.key_parts(0)
            || key > self.key_parts(self.len() as u32 - 1)
        {
            return None;
        }
        let (mut lo, mut hi) = (0, self.len() as u32);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.key_parts(mid).cmp(&key) {
                std::cmp::Ordering::Equal => return Some(mid),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        None
    }
    fn bounds(&self) -> Option<(KeyParts<'_>, KeyParts<'_>)> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.bounds().map(|(a, b)| {
                (
                    (a.scope.as_str(), a.collection.as_str(), a.id.as_str()),
                    (b.scope.as_str(), b.collection.as_str(), b.id.as_str()),
                )
            });
        }
        if self.len() == 0 {
            None
        } else {
            Some((self.key_parts(0), self.key_parts(self.len() as u32 - 1)))
        }
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        match (self.bounds(), other.bounds()) {
            (Some((a, b)), Some((c, d))) => a <= d && c <= b,
            _ => false,
        }
    }
    pub fn hide_keys(&self, newer: &Self, mask: &mut [u64]) -> Result<()> {
        if !self.overlaps(newer) {
            return Ok(());
        }
        for n in 0..newer.len() as u32 {
            if let Some(id) = self.find(&newer.key(n)?)? {
                mask[id as usize / 64] |= 1 << (id % 64);
            }
        }
        Ok(())
    }
    pub fn get(&self, key: &Key) -> Result<Option<StoredDocument>> {
        self.find(key)?
            .filter(|id| !self.deleted(*id))
            .map(|id| self.read_document(id))
            .transpose()
    }

    /// Hydrate a slot already selected by the query's visibility mask.
    /// Payload CRC and key/version identity checks also apply to this path.
    #[cfg(feature = "experimental-direct-slot")]
    pub(crate) fn read_selected_document(&self, id: u32) -> Result<StoredDocument> {
        if id as usize >= self.len() || self.deleted(id) {
            return Err(Error::Corrupt(
                "cannot hydrate invalid/tombstone slot".into(),
            ));
        }
        self.read_document(id)
    }

    #[cfg(feature = "experimental-paged-metadata")]
    fn read_text_document(
        &self,
        id: u32,
        cursor: &mut paged::TextCursor,
    ) -> Result<StoredDocument> {
        if let Some(paged) = &self.paged {
            paged.text_document(id, cursor, &self.file)
        } else {
            self.read_document(id)
        }
    }
    fn read_document(&self, id: u32) -> Result<StoredDocument> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("payload.read_document");
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            let record = paged.record(id)?;
            let mut bytes = vec![0; record.len as usize];
            read_at(&self.file, record.offset, &mut bytes)?;
            if crc(&bytes) != record.checksum {
                return Err(Error::Corrupt("segment document checksum mismatch".into()));
            }
            let stored: StoredDocument =
                serde_json::from_slice(&bytes).map_err(|e| Error::Corrupt(e.to_string()))?;
            if stored.document.key != record.key || stored.version != record.version {
                return Err(Error::Corrupt("segment document identity mismatch".into()));
            }
            return Ok(stored);
        }
        let r = self.metadata(id)?;
        let mut bytes = vec![0; r.len as usize];
        read_at(&self.file, r.offset, &mut bytes)?;
        self.decode_document(id, &bytes)
    }

    fn decode_document(&self, id: u32, bytes: &[u8]) -> Result<StoredDocument> {
        #[cfg(feature = "diagnostic-timing")]
        let _decode = crate::timing::span("payload.decode_crc_identity");
        let r = self.metadata(id)?;
        if bytes.len() != r.len as usize || crc(bytes) != r.checksum {
            return Err(Error::Corrupt("segment document checksum mismatch".into()));
        }
        let stored: StoredDocument =
            serde_json::from_slice(bytes).map_err(|e| Error::Corrupt(e.to_string()))?;
        let key = &stored.document.key;
        #[cfg(feature = "experimental-paged-metadata")]
        let identity = if let Some(paged) = &self.paged {
            key == &paged.record(id)?.key
        } else {
            (key.scope.as_str(), key.collection.as_str(), key.id.as_str()) == self.key_parts(id)
        };
        #[cfg(not(feature = "experimental-paged-metadata"))]
        let identity =
            (key.scope.as_str(), key.collection.as_str(), key.id.as_str()) == self.key_parts(id);
        if !identity || stored.version != r.version {
            return Err(Error::Corrupt("segment document identity mismatch".into()));
        }
        Ok(stored)
    }

    /// Sorted visible slots, tagged with their final position in the global page.
    /// Nearby payloads share one read window; only selected payloads are decoded.
    #[cfg(feature = "experimental-grouped-read")]
    pub(crate) fn read_selected_batch(
        &self,
        slots: &[(u32, usize)],
        scratch: &mut Vec<u8>,
        output: &mut Vec<(usize, StoredDocument)>,
    ) -> Result<BatchReadStats> {
        let mut previous = None;
        for &(id, _) in slots {
            if id as usize >= self.len() || self.deleted(id) || previous.is_some_and(|p| p >= id) {
                return Err(Error::Corrupt("unordered or deleted selected slot".into()));
            }
            previous = Some(id);
        }
        let mut stats = BatchReadStats::default();
        let mut start = 0;
        while start < slots.len() {
            let (id, position) = slots[start];
            let first = self.metadata(id)?;
            if first.len as usize > READ_WINDOW {
                output.push((position, self.read_selected_document(id)?));
                stats.requests += 1;
                stats.requested_bytes += first.len as u64;
                stats.oversized_documents += 1;
                start += 1;
                continue;
            }
            let offset = first.offset;
            let mut end = offset
                .checked_add(first.len as u64)
                .ok_or_else(|| Error::Corrupt("selected payload end overflow".into()))?;
            let mut next = start + 1;
            while next < slots.len() {
                let r = self.metadata(slots[next].0)?;
                let record_end = r
                    .offset
                    .checked_add(r.len as u64)
                    .ok_or_else(|| Error::Corrupt("selected payload end overflow".into()))?;
                if r.offset < end
                    || r.offset - end > READ_GAP
                    || record_end - offset > READ_WINDOW as u64
                {
                    break;
                }
                end = record_end;
                next += 1;
            }
            let len = usize::try_from(end - offset)
                .map_err(|_| Error::Corrupt("selected read length overflow".into()))?;
            if scratch.capacity() < len {
                scratch.reserve_exact(len.saturating_sub(scratch.len()));
            }
            scratch.resize(len, 0);
            read_at(&self.file, offset, scratch)?;
            stats.requests += 1;
            stats.requested_bytes += len as u64;
            for &(id, position) in &slots[start..next] {
                let r = self.metadata(id)?;
                let begin = (r.offset - offset) as usize;
                let bytes = scratch
                    .get(begin..begin + r.len as usize)
                    .ok_or_else(|| Error::Corrupt("invalid grouped payload bounds".into()))?;
                output.push((position, self.decode_document(id, bytes)?));
            }
            start = next;
        }
        Ok(stats)
    }

    fn one(&self, term: Term) -> Vec<Posting> {
        self.postings.get(&term).copied().into_iter().collect()
    }
    fn id_at(&self, p: Posting, slot: u32) -> Result<u32> {
        let mut bytes = [0; 4];
        read_at(&self.file, p.offset + slot as u64 * 4, &mut bytes)?;
        let id = u32::from_le_bytes(bytes);
        if id as usize >= self.len() {
            return Err(Error::Corrupt("posting record id out of range".into()));
        }
        Ok(id)
    }
    fn contains(&self, p: Posting, id: u32) -> Result<bool> {
        // A sorted, unique posting covering all record IDs needs no disk probe.
        if p.count as usize == self.live_records {
            return Ok(true);
        }
        let (mut lo, mut hi) = (0, p.count);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.id_at(p, mid)?.cmp(&id) {
                std::cmp::Ordering::Equal => return Ok(true),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        Ok(false)
    }
    fn group_contains(&self, group: &[Posting], id: u32) -> Result<bool> {
        for &p in group {
            if self.contains(p, id)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn read_small_group(&self, group: &[Posting]) -> Result<Vec<u32>> {
        let mut ids = vec![];
        for &p in group {
            let mut bytes = vec![0; p.count as usize * 4];
            read_at(&self.file, p.offset, &mut bytes)?;
            let mut previous = None;
            for chunk in bytes.as_chunks::<4>().0 {
                let id = u32::from_le_bytes(*chunk);
                if id as usize >= self.len() || previous.is_some_and(|old| old >= id) {
                    return Err(Error::Corrupt("invalid small posting list".into()));
                }
                previous = Some(id);
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    // Preserve the original local Vec/block loop for offset pages. Keeping
    // this path separate avoids collector overhead on the legacy fallback.
    pub fn matches(&self, q: &Query, hidden: impl Fn(u32) -> bool) -> Result<Vec<u32>> {
        let mut groups = vec![match &q.collection {
            Some(c) => self.one(Term::Collection(q.scope.clone(), c.clone())),
            None => self.one(Term::Scope(q.scope.clone())),
        }];
        let collections: BTreeSet<&str> = if let Some(c) = &q.collection {
            BTreeSet::from([c.as_str()])
        } else {
            self.postings
                .keys()
                .filter_map(|t| match t {
                    Term::Collection(s, c) if s == &q.scope => Some(c.as_str()),
                    _ => None,
                })
                .collect()
        };
        for (field, value) in &q.exact {
            let mut group = vec![];
            for c in &collections {
                group.extend(self.one(Term::Exact(
                    q.scope.clone(),
                    c.to_string(),
                    field.clone(),
                    value.to_string(),
                )));
            }
            groups.push(group);
        }
        for r in &q.ranges {
            let mut group = vec![];
            for c in &collections {
                let min = Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.min);
                let max = Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.max);
                group.extend(self.postings.range(min..=max).map(|(_, p)| *p));
            }
            groups.push(group);
        }
        if let Some(key) = &q.linked_to {
            groups.push(self.one(Term::Link(key.clone())));
        }
        let terms: Vec<String> = q.text.split_whitespace().map(fold).collect();
        for term in &terms {
            for gram in grams(term) {
                groups.push(self.one(Term::Gram(q.scope.clone(), gram)));
            }
        }
        groups.sort_by_key(|g| g.iter().map(|p| p.count as u64).sum::<u64>());
        if groups[0].is_empty() {
            return Ok(vec![]);
        }
        // Query-local cache avoids repeated positional probes for small lists.
        // It is capped at 256 KiB and is released when this search returns.
        let mut cache_budget = 256 * 1024usize;
        let mut cached = Vec::with_capacity(groups.len() - 1);
        for group in &groups[1..] {
            let count = group.iter().map(|p| p.count as usize).sum::<usize>();
            if count <= 4096 && count * 4 <= cache_budget && count < self.len() {
                cached.push(Some(self.read_small_group(group)?));
                cache_budget -= count * 4;
            } else {
                cached.push(None);
            }
        }
        // Query-local reuse of identical content; bounded to 256 KiB. Titles
        // remain independently verified so cross-record text cannot leak.
        let mut folded_content = BTreeMap::<String, String>::new();
        let mut folded_bytes = 0usize;
        #[cfg(feature = "experimental-paged-metadata")]
        let mut text_cursor = paged::TextCursor::default();
        // Stream the smallest posting group in blocks; no full list hydration.
        let mut matched = vec![];
        let mut buffer = [0; 4096];
        for &p in &groups[0] {
            let mut slot = 0;
            let mut previous = None;
            while slot < p.count {
                let count = (p.count - slot).min((buffer.len() / 4) as u32);
                read_at(
                    &self.file,
                    p.offset + slot as u64 * 4,
                    &mut buffer[..count as usize * 4],
                )?;
                for bytes in buffer[..count as usize * 4].as_chunks::<4>().0 {
                    let id = u32::from_le_bytes(*bytes);
                    if id as usize >= self.len() || previous.is_some_and(|old| old >= id) {
                        return Err(Error::Corrupt("invalid sorted posting list".into()));
                    }
                    previous = Some(id);
                    if hidden(id) || self.deleted(id) {
                        continue;
                    }
                    let mut include = true;
                    for (group, cached) in groups[1..].iter().zip(&cached) {
                        let found = if let Some(ids) = cached {
                            ids.binary_search(&id).is_ok()
                        } else {
                            self.group_contains(group, id)?
                        };
                        if !found {
                            include = false;
                            break;
                        }
                    }
                    if include && !terms.is_empty() {
                        #[cfg(feature = "experimental-paged-metadata")]
                        let d = self.read_text_document(id, &mut text_cursor)?.document;
                        #[cfg(not(feature = "experimental-paged-metadata"))]
                        let d = self.read_document(id)?.document;
                        let title = fold(&d.title);
                        if !terms.iter().all(|term| title.contains(term)) {
                            let text = if let Some(text) = folded_content.get(&d.text) {
                                text.as_str()
                            } else {
                                let text = fold(&d.text);
                                if d.text.len() + text.len() <= 256 * 1024 - folded_bytes {
                                    folded_bytes += d.text.len() + text.len();
                                    folded_content.insert(d.text.clone(), text);
                                    folded_content.get(&d.text).unwrap().as_str()
                                } else {
                                    include = terms
                                        .iter()
                                        .all(|term| title.contains(term) || text.contains(term));
                                    if include {
                                        matched.push(id);
                                    }
                                    continue;
                                }
                            };
                            include = terms
                                .iter()
                                .all(|term| title.contains(term) || text.contains(term));
                        }
                    }
                    if include {
                        matched.push(id);
                    }
                }
                slot += count;
            }
        }
        matched.sort_unstable();
        matched.dedup();
        Ok(matched)
    }

    pub fn visible_count(&self, mask: &[u64]) -> Result<usize> {
        let mut hidden_live = 0;
        for (word, &bits) in mask.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let id = word * 64 + bits.trailing_zeros() as usize;
                if id < self.len() && !self.deleted(id as u32) {
                    hidden_live += 1;
                }
                bits &= bits - 1;
            }
        }
        self.live_records
            .checked_sub(hidden_live)
            .ok_or_else(|| Error::Corrupt("invalid live visibility count".into()))
    }

    /// Exact total and a bounded prefix of IDs in key order. A per-run prefix
    /// of offset + limit contains every candidate for that global page.
    pub fn prefix_matches(
        &self,
        q: &Query,
        mask: &[u64],
        limit: usize,
    ) -> Result<(usize, Vec<u32>)> {
        let visible = self.visible_count(mask)?;
        let hidden = |id: u32| mask[id as usize / 64] & (1u64 << (id % 64)) != 0;
        let mut ids = BinaryHeap::with_capacity(limit.min(self.len()));
        let mut group = None;
        let mut capture = true;
        // Within a union, collection/scalar-value postings are disjoint by
        // construction. IDs in each posting are increasing; after an ID exceeds
        // our largest retained ID the rest of that posting cannot improve it.
        let total = self.visit_matches::<true>(q, hidden, Some(visible), limit, |id, n| {
            if group != Some(n) {
                group = Some(n);
                capture = true;
            }
            if !capture {
                return;
            }
            if ids.len() < limit {
                ids.push(id);
            } else if id < *ids.peek().unwrap() {
                *ids.peek_mut().unwrap() = id;
            } else {
                capture = false;
            }
        })?;
        Ok((total, ids.into_sorted_vec()))
    }

    fn visit_matches<const COUNT: bool>(
        &self,
        q: &Query,
        hidden: impl Fn(u32) -> bool,
        visible_total: Option<usize>,
        capture_limit: usize,
        mut emit: impl FnMut(u32, usize),
    ) -> Result<usize> {
        let mut groups = vec![match &q.collection {
            Some(c) => self.one(Term::Collection(q.scope.clone(), c.clone())),
            None => self.one(Term::Scope(q.scope.clone())),
        }];
        let collections: BTreeSet<&str> = if let Some(c) = &q.collection {
            BTreeSet::from([c.as_str()])
        } else {
            self.postings
                .keys()
                .filter_map(|t| match t {
                    Term::Collection(s, c) if s == &q.scope => Some(c.as_str()),
                    _ => None,
                })
                .collect()
        };
        for (field, value) in &q.exact {
            let mut group = vec![];
            for c in &collections {
                group.extend(self.one(Term::Exact(
                    q.scope.clone(),
                    c.to_string(),
                    field.clone(),
                    value.to_string(),
                )));
            }
            groups.push(group);
        }
        for r in &q.ranges {
            let mut group = vec![];
            for c in &collections {
                let min = Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.min);
                let max = Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.max);
                group.extend(self.postings.range(min..=max).map(|(_, p)| *p));
            }
            groups.push(group);
        }
        if let Some(key) = &q.linked_to {
            groups.push(self.one(Term::Link(key.clone())));
        }
        let terms: Vec<String> = q.text.split_whitespace().map(fold).collect();
        for term in &terms {
            for gram in grams(term) {
                groups.push(self.one(Term::Gram(q.scope.clone(), gram)));
            }
        }
        groups.sort_by_key(|g| g.iter().map(|p| p.count as u64).sum::<u64>());
        if groups[0].is_empty() {
            return Ok(0);
        }
        // As with contains(), a posting covering every live record is a proven
        // full filter for a valid derived index. Visibility is counted from the
        // mask, excluding tombstones, so no stale or deleted row enters total.
        // Text still requires candidate verification and cannot use this path.
        let fast_total = visible_total.filter(|_| {
            terms.is_empty()
                && groups
                    .iter()
                    .all(|g| g.iter().any(|p| p.count as usize == self.live_records))
        });
        if fast_total == Some(0) {
            return Ok(0);
        }
        // Query-local cache avoids repeated positional probes for small lists.
        // It is capped at 256 KiB and is released when this search returns.
        let mut cache_budget = 256 * 1024usize;
        let mut cached = Vec::with_capacity(groups.len() - 1);
        for group in &groups[1..] {
            let count = group.iter().map(|p| p.count as usize).sum::<usize>();
            if count <= 4096 && count * 4 <= cache_budget && count < self.len() {
                cached.push(Some(self.read_small_group(group)?));
                cache_budget -= count * 4;
            } else {
                cached.push(None);
            }
        }
        // Query-local reuse of identical content; bounded to 256 KiB. Titles
        // remain independently verified so cross-record text cannot leak.
        let mut folded_content = BTreeMap::<String, String>::new();
        let mut folded_bytes = 0usize;
        #[cfg(feature = "experimental-paged-metadata")]
        let mut text_cursor = paged::TextCursor::default();
        // Stream the smallest posting group in blocks; no full list hydration.
        let mut total = 0;
        let mut buffer = [0; 4096];
        for (n, &p) in groups[0].iter().enumerate() {
            let mut slot = 0;
            let mut previous = None;
            while slot < p.count {
                let count = (p.count - slot).min((buffer.len() / 4) as u32);
                read_at(
                    &self.file,
                    p.offset + slot as u64 * 4,
                    &mut buffer[..count as usize * 4],
                )?;
                for bytes in buffer[..count as usize * 4].as_chunks::<4>().0 {
                    let id = u32::from_le_bytes(*bytes);
                    if id as usize >= self.len() || previous.is_some_and(|old| old >= id) {
                        return Err(Error::Corrupt("invalid sorted posting list".into()));
                    }
                    previous = Some(id);
                    if hidden(id) || self.deleted(id) {
                        continue;
                    }
                    let mut include = true;
                    for (group, cached) in groups[1..].iter().zip(&cached) {
                        let found = if let Some(ids) = cached {
                            ids.binary_search(&id).is_ok()
                        } else {
                            self.group_contains(group, id)?
                        };
                        if !found {
                            include = false;
                            break;
                        }
                    }
                    if include && !terms.is_empty() {
                        #[cfg(feature = "experimental-paged-metadata")]
                        let d = self.read_text_document(id, &mut text_cursor)?.document;
                        #[cfg(not(feature = "experimental-paged-metadata"))]
                        let d = self.read_document(id)?.document;
                        let title = fold(&d.title);
                        if !terms.iter().all(|term| title.contains(term)) {
                            let text = if let Some(text) = folded_content.get(&d.text) {
                                text.as_str()
                            } else {
                                let text = fold(&d.text);
                                if d.text.len() + text.len() <= 256 * 1024 - folded_bytes {
                                    folded_bytes += d.text.len() + text.len();
                                    folded_content.insert(d.text.clone(), text);
                                    folded_content.get(&d.text).unwrap().as_str()
                                } else {
                                    include = terms
                                        .iter()
                                        .all(|term| title.contains(term) || text.contains(term));
                                    if include {
                                        if COUNT {
                                            total += 1;
                                        }
                                        emit(id, n);
                                    }
                                    continue;
                                }
                            };
                            include = terms
                                .iter()
                                .all(|term| title.contains(term) || text.contains(term));
                        }
                    }
                    if include {
                        if COUNT {
                            total += 1;
                        }
                        emit(id, n);
                        if let Some(count) = fast_total
                            && total >= capture_limit
                        {
                            return Ok(count);
                        }
                    }
                }
                slot += count;
            }
        }
        Ok(fast_total.unwrap_or(total))
    }
}
