//! Versioned derived metadata sidecar. The validated SEG03 remains its source.
//! A process-wide cache is shared by foreground readers and merge workers.
use super::*;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

const MAGIC: &[u8; 8] = b"FDBMET04";
const END: &[u8; 8] = b"FDBMEND4";
const LEAF: usize = 64;
const FANOUT: usize = 32;
const BUDGET: usize = 256 * 1024;
const HEADER: u64 = 32;
#[cfg(feature = "experimental-streaming-metadata")]
mod streaming;
#[cfg(feature = "experimental-streaming-metadata")]
pub(super) use streaming::prepare;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fence {
    key: Key,
    offset: u64,
    len: u32,
    slot: u32,
}
struct CacheEntry {
    owner: u64,
    offset: u64,
    bytes: Vec<u8>,
}
#[derive(Default)]
struct Cache {
    entries: VecDeque<CacheEntry>,
    bytes: usize,
}
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
static OWNER: AtomicU64 = AtomicU64::new(1);
fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(Default::default)
}
fn corrupt(message: &str) -> Error {
    Error::Corrupt(format!("paged metadata: {message}"))
}

#[derive(Default)]
pub(super) struct TextCursor {
    leaf: Option<u32>,
    bytes: Vec<u8>,
    payload_offset: u64,
    payload: Vec<u8>,
    #[cfg(test)]
    requests: usize,
}

pub(super) struct PagedRecords {
    file: File,
    pub bytes: u64,
    roots: Vec<Fence>,
    count: usize,
    owner: u64,
    // Keeps the posting candidate loop free of per-candidate metadata I/O.
    // This bitmap is deliberately reported; it is not a constant-memory claim.
    deleted: Vec<u64>,
    last: Option<Key>,
}
impl Drop for PagedRecords {
    fn drop(&mut self) {
        if let Ok(mut cache) = cache().lock() {
            cache.entries.retain(|entry| entry.owner != self.owner);
            cache.bytes = cache
                .entries
                .iter()
                .map(|entry| entry.bytes.capacity())
                .sum();
        }
    }
}

impl PagedRecords {
    pub fn prepare(path: &Path, source: &DiskIndex) -> Result<Self> {
        let target = path.with_extension("records04.idx");
        let mut footer = [0; FOOTER_SIZE as usize];
        let segment_bytes = source.file.metadata()?.len();
        read_at(&source.file, segment_bytes - FOOTER_SIZE, &mut footer)?;
        let binding = (segment_bytes, crc(&footer), source.sequence);
        match Self::open(&target, binding, source) {
            Ok(paged) => return Ok(paged),
            Err(Error::Corrupt(_)) => {}
            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let temporary = target.with_extension(format!(
            "build-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(MAGIC)?;
            file.write_all(&binding.0.to_le_bytes())?;
            file.write_all(&binding.1.to_le_bytes())?;
            file.write_all(&binding.2.to_le_bytes())?;
            file.write_all(&(source.len() as u32).to_le_bytes())?;
            let mut roots = Vec::new();
            for group in (0..source.len()).step_by(LEAF * FANOUT) {
                let mut leaves = Vec::new();
                for start in (group..source.len().min(group + LEAF * FANOUT)).step_by(LEAF) {
                    let records = (start..source.len().min(start + LEAF))
                        .map(|id| source.record(id as u32))
                        .collect::<Result<Vec<_>>>()?;
                    let bytes = encode_leaf(&records)?;
                    leaves.push(write_page(
                        &mut file,
                        records[0].key.clone(),
                        start as u32,
                        &bytes,
                    )?);
                }
                let records: Vec<_> = leaves
                    .iter()
                    .map(|f| Record {
                        key: f.key.clone(),
                        offset: f.offset,
                        len: f.len,
                        checksum: f.slot,
                        version: 1,
                        deleted: false,
                    })
                    .collect();
                let bytes = encode_leaf(&records)?;
                roots.push(write_page(
                    &mut file,
                    leaves[0].key.clone(),
                    group as u32,
                    &bytes,
                )?);
            }
            let root_offset = file.stream_position()?;
            let bytes = serde_json::to_vec(&roots)?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err(corrupt("root exceeds 16 MiB"));
            }
            file.write_all(&bytes)?;
            let end = file.stream_position()?;
            let checksum = prefix_crc(&mut file, end)?;
            file.seek(SeekFrom::Start(end))?;
            let mut footer = END.to_vec();
            footer.extend_from_slice(&root_offset.to_le_bytes());
            footer.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            footer.extend_from_slice(&checksum.to_le_bytes());
            footer.extend_from_slice(&crc(&footer).to_le_bytes());
            file.write_all(&footer)?;
            file.sync_all()?;
            drop(file);
            // Derived data may disappear between remove and rename; open rebuilds it.
            match std::fs::remove_file(&target) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            std::fs::rename(&temporary, &target)?;
            Self::open(&target, binding, source)
        })();
        let _ = std::fs::remove_file(&temporary);
        result
    }

    fn open(path: &Path, binding: (u64, u32, u64), source: &DiskIndex) -> Result<Self> {
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();
        if size < HEADER + FOOTER_SIZE {
            return Err(corrupt("short file"));
        }
        let mut header = [0; HEADER as usize];
        read_at(&file, 0, &mut header)?;
        if &header[..8] != MAGIC
            || u64::from_le_bytes(header[8..16].try_into().unwrap()) != binding.0
            || u32::from_le_bytes(header[16..20].try_into().unwrap()) != binding.1
            || u64::from_le_bytes(header[20..28].try_into().unwrap()) != binding.2
            || u32::from_le_bytes(header[28..32].try_into().unwrap()) as usize != source.len()
        {
            return Err(corrupt("binding/version mismatch"));
        }
        let mut footer = [0; FOOTER_SIZE as usize];
        read_at(&file, size - FOOTER_SIZE, &mut footer)?;
        if &footer[..8] != END
            || crc(&footer[..28]) != u32::from_le_bytes(footer[28..32].try_into().unwrap())
        {
            return Err(corrupt("footer checksum"));
        }
        let offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());
        let len = u64::from_le_bytes(footer[16..24].try_into().unwrap());
        if offset < HEADER
            || len > 16 * 1024 * 1024
            || offset.checked_add(len) != Some(size - FOOTER_SIZE)
            || prefix_crc(&mut file, size - FOOTER_SIZE)?
                != u32::from_le_bytes(footer[24..28].try_into().unwrap())
        {
            return Err(corrupt("bounds/cache checksum"));
        }
        let mut bytes = vec![0; len as usize];
        read_at(&file, offset, &mut bytes)?;
        let roots: Vec<Fence> =
            serde_json::from_slice(&bytes).map_err(|e| corrupt(&e.to_string()))?;
        if roots.len() != source.len().div_ceil(LEAF * FANOUT) {
            return Err(corrupt("root count"));
        }
        let mut cursor = HEADER;
        let mut slot = 0;
        // Validate every slot against the independently validated SEG03 directory.
        // Source replay is still performed by open_source, regardless of this cache.
        for root in &roots {
            if root.offset < HEADER
                || root
                    .offset
                    .checked_add(root.len as u64)
                    .is_none_or(|end| end > offset)
            {
                return Err(corrupt("directory page bounds"));
            }
            let leaves = read_directory(&file, root)?;
            if root.slot as usize != slot
                || leaves.is_empty()
                || leaves.len() != FANOUT.min(source.len().saturating_sub(slot).div_ceil(LEAF))
                || root.key != leaves[0].key
            {
                return Err(corrupt("root identity"));
            }
            for leaf in &leaves {
                if leaf.offset != cursor || leaf.slot as usize != slot {
                    return Err(corrupt("leaf order"));
                }
                if leaf
                    .offset
                    .checked_add(leaf.len as u64)
                    .is_none_or(|end| end > offset)
                {
                    return Err(corrupt("leaf page bounds"));
                }
                let page = read_page(&file, leaf)?;
                let count = leaf_count(&page)?;
                if count != LEAF.min(source.len().saturating_sub(slot)) {
                    return Err(corrupt("leaf count"));
                }
                for local in 0..count {
                    let actual = decode_leaf(&page, local)?;
                    let expected = source.record(slot as u32)?;
                    if actual != expected || (local == 0 && actual.key != leaf.key) {
                        return Err(corrupt("record identity"));
                    }
                    slot += 1;
                }
                cursor += leaf.len as u64;
            }
            if root.offset != cursor {
                return Err(corrupt("directory order"));
            }
            cursor += root.len as u64;
        }
        if slot != source.len() || cursor != offset {
            return Err(corrupt("noncontiguous layout"));
        }
        let mut deleted = vec![0u64; source.len().div_ceil(64)];
        for id in 0..source.len() {
            if source.deleted(id as u32) {
                deleted[id / 64] |= 1 << (id % 64);
            }
        }
        let last = if source.len() == 0 {
            None
        } else {
            Some(source.key(source.len() as u32 - 1)?)
        };
        Ok(Self {
            file,
            bytes: size,
            roots,
            count: source.len(),
            owner: OWNER.fetch_add(1, Ordering::Relaxed),
            deleted,
            last,
        })
    }

    fn page<T>(&self, fence: &Fence, decode: impl FnOnce(&[u8]) -> Result<T>) -> Result<T> {
        let mut cache = cache().lock().map_err(|_| corrupt("cache lock poisoned"))?;
        if let Some(position) = cache
            .entries
            .iter()
            .position(|e| e.owner == self.owner && e.offset == fence.offset)
        {
            let entry = cache.entries.remove(position).unwrap();
            let result = decode(&entry.bytes);
            cache.entries.push_front(entry);
            return result;
        }
        // Evict before allocating the next buffer, so cache + pending read <= BUDGET.
        if fence.len as usize > BUDGET {
            return Err(corrupt("page exceeds budget"));
        }
        while cache.bytes + fence.len as usize > BUDGET {
            let entry = cache
                .entries
                .pop_back()
                .ok_or_else(|| corrupt("cache accounting"))?;
            cache.bytes -= entry.bytes.capacity();
        }
        let bytes = read_page(&self.file, fence)?;
        let result = decode(&bytes);
        cache.bytes += bytes.capacity();
        cache.entries.push_front(CacheEntry {
            owner: self.owner,
            offset: fence.offset,
            bytes,
        });
        result
    }
    pub fn record(&self, slot: u32) -> Result<Record> {
        if slot as usize >= self.count {
            return Err(corrupt("slot out of bounds"));
        }
        let root = &self.roots[slot as usize / (LEAF * FANOUT)];
        let leaf = self.page(root, |bytes| {
            Ok(as_fence(decode_leaf(
                bytes,
                (slot as usize % (LEAF * FANOUT)) / LEAF,
            )?))
        })?;
        self.page(&leaf, |bytes| decode_leaf(bytes, slot as usize % LEAF))
    }
    pub fn find(&self, key: &Key) -> Result<Option<u32>> {
        if self.roots.first().is_none_or(|first| key < &first.key)
            || self.last.as_ref().is_none_or(|last| key > last)
        {
            return Ok(None);
        }
        let root = &self.roots[self.roots.partition_point(|f| &f.key <= key) - 1];
        let leaf = self.page(root, |bytes| {
            let (mut lo, mut hi) = (0, leaf_count(bytes)?);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if compare_leaf_key(bytes, mid, key)? != std::cmp::Ordering::Greater {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            if lo == 0 {
                return Err(corrupt("directory fence"));
            }
            Ok(as_fence(decode_leaf(bytes, lo - 1)?))
        })?;
        self.page(&leaf, |bytes| {
            let (mut lo, mut hi) = (0, leaf_count(bytes)?);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                match compare_leaf_key(bytes, mid, key)? {
                    std::cmp::Ordering::Equal => return Ok(Some(leaf.slot + mid as u32)),
                    std::cmp::Ordering::Less => lo = mid + 1,
                    std::cmp::Ordering::Greater => hi = mid,
                }
            }
            Ok(None)
        })
    }
    fn query_leaf(&self, slot: u32, cursor: &mut TextCursor) -> Result<()> {
        if slot as usize >= self.count || self.deleted(slot) {
            return Err(corrupt("text slot"));
        }
        let leaf_id = slot / LEAF as u32;
        if cursor.leaf != Some(leaf_id) {
            cursor.bytes = Vec::new();
            cursor.leaf = None;
            let root = &self.roots[slot as usize / (LEAF * FANOUT)];
            let fence = self.page(root, |bytes| {
                Ok(as_fence(decode_leaf(
                    bytes,
                    (slot as usize % (LEAF * FANOUT)) / LEAF,
                )?))
            })?;
            cursor.bytes = self.page(&fence, |bytes| Ok(bytes.to_vec()))?;
            cursor.leaf = Some(leaf_id);
        }
        Ok(())
    }
    pub fn query_metadata(&self, slot: u32, cursor: &mut TextCursor) -> Result<PackedRecord> {
        self.query_leaf(slot, cursor)?;
        numeric_metadata(&cursor.bytes, slot as usize % LEAF)
    }
    pub fn query_identity(
        &self,
        slot: u32,
        cursor: &mut TextCursor,
        parts: (&str, &str, &str),
    ) -> Result<bool> {
        self.query_leaf(slot, cursor)?;
        Ok(compare_leaf_parts(
            &cursor.bytes,
            slot as usize % LEAF,
            [parts.0, parts.1, parts.2],
        )? == std::cmp::Ordering::Equal)
    }
    pub fn text_document(
        &self,
        slot: u32,
        cursor: &mut TextCursor,
        payload: &File,
    ) -> Result<StoredDocument> {
        self.query_leaf(slot, cursor)?;
        let local = slot as usize % LEAF;
        let record = numeric_metadata(&cursor.bytes, local)?;
        if record.deleted {
            return Err(corrupt("text tombstone"));
        }
        if record.len as usize > READ_WINDOW {
            let mut bytes = vec![0; record.len as usize];
            read_at(payload, record.offset, &mut bytes)?;
            #[cfg(test)]
            {
                cursor.requests += 1;
            }
            return decode_text_payload(&cursor.bytes, local, &record, &bytes);
        }
        let record_end = record
            .offset
            .checked_add(record.len as u64)
            .ok_or_else(|| corrupt("text payload overflow"))?;
        let window_end = cursor
            .payload_offset
            .checked_add(cursor.payload.len() as u64)
            .ok_or_else(|| corrupt("text window overflow"))?;
        if record.offset < cursor.payload_offset || record_end > window_end {
            let mut end = record_end;
            for next in local + 1..leaf_count(&cursor.bytes)? {
                let next = numeric_metadata(&cursor.bytes, next)?;
                let next_end = next
                    .offset
                    .checked_add(next.len as u64)
                    .ok_or_else(|| corrupt("text adjacent overflow"))?;
                if next.offset < end
                    || next.offset - end > READ_GAP
                    || next_end - record.offset > READ_WINDOW as u64
                {
                    break;
                }
                end = next_end;
            }
            let len = (end - record.offset) as usize;
            if cursor.payload.capacity() < len {
                cursor
                    .payload
                    .reserve_exact(len.saturating_sub(cursor.payload.len()));
            }
            cursor.payload.resize(len, 0);
            read_at(payload, record.offset, &mut cursor.payload)?;
            cursor.payload_offset = record.offset;
            #[cfg(test)]
            {
                cursor.requests += 1;
            }
        }
        let begin = (record.offset - cursor.payload_offset) as usize;
        let bytes = cursor
            .payload
            .get(begin..begin + record.len as usize)
            .ok_or_else(|| corrupt("text slice bounds"))?;
        decode_text_payload(&cursor.bytes, local, &record, bytes)
    }
    pub fn bounds(&self) -> Option<(&Key, &Key)> {
        Some((&self.roots.first()?.key, self.last.as_ref()?))
    }
    pub fn deleted(&self, id: u32) -> bool {
        self.deleted[id as usize / 64] & (1 << (id % 64)) != 0
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn capacities(&self) -> (usize, usize) {
        (
            self.roots.capacity() * std::mem::size_of::<Fence>()
                + self
                    .roots
                    .iter()
                    .map(|f| {
                        f.key.scope.capacity() + f.key.collection.capacity() + f.key.id.capacity()
                    })
                    .sum::<usize>()
                + self.last.as_ref().map_or(0, |k| {
                    k.scope.capacity() + k.collection.capacity() + k.id.capacity()
                }),
            self.deleted.capacity() * 8,
        )
    }
}

fn write_page(file: &mut File, key: Key, slot: u32, bytes: &[u8]) -> Result<Fence> {
    if bytes.len() + 4 > BUDGET {
        return Err(corrupt("encoded page too large"));
    }
    let offset = file.stream_position()?;
    file.write_all(bytes)?;
    file.write_all(&crc(bytes).to_le_bytes())?;
    Ok(Fence {
        key,
        offset,
        len: (bytes.len() + 4) as u32,
        slot,
    })
}
fn read_page(file: &File, fence: &Fence) -> Result<Vec<u8>> {
    if fence.len < 4 || fence.len as usize > BUDGET {
        return Err(corrupt("page length"));
    }
    let mut bytes = vec![0; fence.len as usize];
    read_at(file, fence.offset, &mut bytes)?;
    let end = bytes.len() - 4;
    if crc(&bytes[..end]) != u32::from_le_bytes(bytes[end..].try_into().unwrap()) {
        return Err(corrupt("page checksum"));
    }
    bytes.truncate(end);
    Ok(bytes)
}
fn as_fence(record: Record) -> Fence {
    Fence {
        key: record.key,
        offset: record.offset,
        len: record.len,
        slot: record.checksum,
    }
}
fn read_directory(file: &File, fence: &Fence) -> Result<Vec<Fence>> {
    let bytes = read_page(file, fence)?;
    (0..leaf_count(&bytes)?)
        .map(|slot| {
            let record = decode_leaf(&bytes, slot)?;
            if record.version != 1 || record.deleted {
                return Err(corrupt("directory descriptor"));
            }
            Ok(as_fence(record))
        })
        .collect()
}
fn encode_leaf(records: &[Record]) -> Result<Vec<u8>> {
    let first = &records[0].key;
    let mut bytes = (records.len() as u32).to_le_bytes().to_vec();
    for part in [&first.scope, &first.collection, &first.id] {
        bytes.extend_from_slice(&(part.len() as u16).to_le_bytes());
        bytes.extend_from_slice(part.as_bytes());
    }
    let table = bytes.len();
    bytes.resize(table + records.len() * 4, 0);
    for (n, record) in records.iter().enumerate() {
        let offset = bytes.len() as u32;
        bytes[table + n * 4..table + n * 4 + 4].copy_from_slice(&offset.to_le_bytes());
        for (base, part) in [
            (&first.scope, &record.key.scope),
            (&first.collection, &record.key.collection),
            (&first.id, &record.key.id),
        ] {
            let prefix = base
                .bytes()
                .zip(part.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            bytes.extend_from_slice(&(prefix as u16).to_le_bytes());
            bytes.extend_from_slice(&((part.len() - prefix) as u16).to_le_bytes());
            bytes.extend_from_slice(&part.as_bytes()[prefix..]);
        }
        bytes.extend_from_slice(&record.offset.to_le_bytes());
        bytes.extend_from_slice(&record.version.to_le_bytes());
        bytes.extend_from_slice(&record.len.to_le_bytes());
        bytes.extend_from_slice(&record.checksum.to_le_bytes());
        bytes.push(u8::from(record.deleted));
    }
    Ok(bytes)
}
struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(len)
            .ok_or_else(|| corrupt("length overflow"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| corrupt("short page"))?;
        self.position = end;
        Ok(bytes)
    }
    fn u16(&mut self) -> Result<usize> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()) as usize)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}
fn leaf_count(bytes: &[u8]) -> Result<usize> {
    let count = Cursor { bytes, position: 0 }.u32()? as usize;
    if count == 0 || count > LEAF {
        return Err(corrupt("record count"));
    }
    Ok(count)
}
fn decode_text_payload(
    leaf: &[u8],
    local: usize,
    record: &PackedRecord,
    bytes: &[u8],
) -> Result<StoredDocument> {
    #[cfg(feature = "diagnostic-timing")]
    let _decode = crate::timing::span("text.decode_crc_identity");
    if bytes.len() != record.len as usize || crc(bytes) != record.checksum {
        return Err(corrupt("text payload checksum"));
    }
    #[cfg(feature = "diagnostic-timing")]
    let json = crate::timing::span("text.json_deserialize");
    let stored = crate::payload::decode(bytes)?;
    #[cfg(feature = "diagnostic-timing")]
    drop(json);
    if stored.version != record.version
        || compare_leaf_key(leaf, local, &stored.document.key)? != std::cmp::Ordering::Equal
    {
        return Err(corrupt("text payload identity"));
    }
    Ok(stored)
}
fn numeric_metadata(bytes: &[u8], local: usize) -> Result<PackedRecord> {
    let count = leaf_count(bytes)?;
    if local >= count {
        return Err(corrupt("numeric slot"));
    }
    let mut cursor = Cursor { bytes, position: 4 };
    let mut lengths = [0; 3];
    for len in &mut lengths {
        *len = cursor.u16()?;
        if *len == 0 || *len > 1024 {
            return Err(corrupt("numeric base length"));
        }
        cursor.take(*len)?;
    }
    let table = cursor.position;
    cursor.position += local * 4;
    let offset = cursor.u32()? as usize;
    if offset < table + count * 4 {
        return Err(corrupt("numeric offset"));
    }
    let end = if local + 1 < count {
        cursor.u32()? as usize
    } else {
        bytes.len()
    };
    if end <= offset || end > bytes.len() {
        return Err(corrupt("numeric bounds"));
    }
    cursor.position = offset;
    for len in lengths {
        let prefix = cursor.u16()?;
        let suffix = cursor.u16()?;
        if prefix > len || prefix + suffix > 1024 {
            return Err(corrupt("numeric prefix"));
        }
        cursor.take(suffix)?;
    }
    let record = PackedRecord {
        offset: cursor.u64()?,
        version: cursor.u64()?,
        len: cursor.u32()?,
        checksum: cursor.u32()?,
        deleted: match cursor.take(1)?[0] {
            0 => false,
            1 => true,
            _ => return Err(corrupt("numeric deletion")),
        },
        group: 0,
        id_offset: 0,
        id_len: 0,
    };
    if cursor.position != end
        || record.len > 4 * 1024 * 1024 + 128
        || record.deleted != (record.len == 0)
        || record.version == 0
    {
        return Err(corrupt("numeric record size"));
    }
    Ok(record)
}
fn compare_leaf_key(bytes: &[u8], local: usize, key: &Key) -> Result<std::cmp::Ordering> {
    compare_leaf_parts(bytes, local, [&key.scope, &key.collection, &key.id])
}
fn compare_leaf_parts(bytes: &[u8], local: usize, parts: [&str; 3]) -> Result<std::cmp::Ordering> {
    let count = leaf_count(bytes)?;
    if local >= count {
        return Err(corrupt("local comparison slot"));
    }
    let mut cursor = Cursor { bytes, position: 4 };
    let mut bases: [&[u8]; 3] = [&[]; 3];
    for base in &mut bases {
        let len = cursor.u16()?;
        if len == 0 || len > 1024 {
            return Err(corrupt("comparison key length"));
        }
        *base = cursor.take(len)?;
    }
    let table = cursor.position;
    cursor.position += local * 4;
    let offset = cursor.u32()? as usize;
    if offset < table + count * 4 {
        return Err(corrupt("comparison offset"));
    }
    cursor.position = offset;
    for (base, query) in bases.into_iter().zip(parts) {
        let prefix = cursor.u16()?;
        let suffix = cursor.u16()?;
        if prefix > base.len() || prefix + suffix > 1024 {
            return Err(corrupt("comparison prefix"));
        }
        let suffix = cursor.take(suffix)?;
        let order = base[..prefix]
            .iter()
            .chain(suffix)
            .copied()
            .cmp(query.bytes());
        if order != std::cmp::Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(std::cmp::Ordering::Equal)
}
fn decode_leaf(bytes: &[u8], local: usize) -> Result<Record> {
    let count = leaf_count(bytes)?;
    if local >= count {
        return Err(corrupt("local slot"));
    }
    let mut cursor = Cursor { bytes, position: 4 };
    let mut bases: [&[u8]; 3] = [&[]; 3];
    for base in &mut bases {
        let len = cursor.u16()?;
        if len == 0 || len > 1024 {
            return Err(corrupt("base key length"));
        }
        *base = cursor.take(len)?;
    }
    let table = cursor.position;
    cursor.position += local * 4;
    let offset = cursor.u32()? as usize;
    if offset < table + count * 4 {
        return Err(corrupt("record offset"));
    }
    let end = if local + 1 < count {
        cursor.u32()? as usize
    } else {
        bytes.len()
    };
    if end <= offset || end > bytes.len() {
        return Err(corrupt("record bounds"));
    }
    cursor.position = offset;
    let mut parts: [String; 3] = Default::default();
    for (base, target) in bases.into_iter().zip(&mut parts) {
        let prefix = cursor.u16()?;
        let suffix = cursor.u16()?;
        if prefix > base.len() || prefix + suffix > 1024 {
            return Err(corrupt("key prefix"));
        }
        let mut part = Vec::with_capacity(prefix + suffix);
        part.extend_from_slice(&base[..prefix]);
        part.extend_from_slice(cursor.take(suffix)?);
        *target = String::from_utf8(part).map_err(|_| corrupt("key UTF-8"))?;
    }
    let deleted;
    let [scope, collection, id] = parts;
    let record = Record {
        key: Key {
            scope,
            collection,
            id,
        },
        offset: cursor.u64()?,
        version: cursor.u64()?,
        len: cursor.u32()?,
        checksum: cursor.u32()?,
        deleted: {
            deleted = cursor.take(1)?[0];
            deleted == 1
        },
    };
    if deleted > 1 || cursor.position != end {
        return Err(corrupt("record layout"));
    }
    crate::validate_key(&record.key).map_err(|e| corrupt(&e.to_string()))?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fdb-meta-{}-{}",
                std::process::id(),
                TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn index(&self, count: usize, long: bool) -> DiskIndex {
            let mut builder = Builder::new(&self.0.join("test.idx"), 2, 42, 77).unwrap();
            for id in 0..count {
                let key = if long {
                    Key::new(
                        &"\"".repeat(1024),
                        &"\"".repeat(1024),
                        &format!("{}{:08}", "\"".repeat(1016), id),
                    )
                } else {
                    Key::new("shared", "contents", &format!("{id:08}"))
                };
                builder
                    .add(&StoredDocument {
                        version: 1,
                        document: crate::Document {
                            key,
                            title: "".into(),
                            text: "".into(),
                            fields: BTreeMap::new(),
                            links: vec![],
                        },
                    })
                    .unwrap();
            }
            let index = builder.finish().unwrap();
            #[cfg(feature = "experimental-streaming-metadata")]
            {
                // Legacy codec tests require an independent resident reference.
                drop(index);
                DiskIndex::open_with_mode(&self.0.join("test.idx"), None, false).unwrap()
            }
            #[cfg(not(feature = "experimental-streaming-metadata"))]
            {
                index
            }
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn two_level_slots_lookup_eviction_and_shared_budget() {
        let temp = Temp::new();
        let source = temp.index(2100, false);
        let paged = PagedRecords::prepare(&temp.0.join("test.idx"), &source).unwrap();
        let mut text_cursor = TextCursor::default();
        for slot in [0, 63, 64, 2047, 2048, 2099] {
            assert_eq!(paged.record(slot).unwrap(), source.record(slot).unwrap());
            let key = source.key(slot).unwrap();
            let metadata = paged.query_metadata(slot, &mut text_cursor).unwrap();
            let expected = source.metadata(slot).unwrap();
            assert_eq!(
                (
                    metadata.offset,
                    metadata.len,
                    metadata.checksum,
                    metadata.version
                ),
                (
                    expected.offset,
                    expected.len,
                    expected.checksum,
                    expected.version
                )
            );
            assert!(
                paged
                    .query_identity(
                        slot,
                        &mut text_cursor,
                        (&key.scope, &key.collection, &key.id)
                    )
                    .unwrap()
            );
            assert!(
                !paged
                    .query_identity(
                        slot,
                        &mut text_cursor,
                        (&key.scope, &key.collection, "wrong-id")
                    )
                    .unwrap()
            );
            assert_eq!(
                serde_json::to_value(
                    paged
                        .text_document(slot, &mut text_cursor, &source.file)
                        .unwrap()
                )
                .unwrap(),
                serde_json::to_value(source.read_document(slot).unwrap()).unwrap()
            );
            assert!(text_cursor.bytes.capacity() <= BUDGET);
            assert_eq!(paged.find(&source.key(slot).unwrap()).unwrap(), Some(slot));
        }
        assert!(text_cursor.payload.capacity() <= READ_WINDOW);
        assert_eq!(text_cursor.requests, 4);
        assert_eq!(
            paged
                .find(&Key::new("shared", "contents", "00000000x"))
                .unwrap(),
            None
        );
        assert!(matches!(paged.record(2100), Err(Error::Corrupt(_))));
        let second = PagedRecords::prepare(&temp.0.join("test.idx"), &source).unwrap();
        for slot in (0..2100).step_by(64) {
            second.record(slot).unwrap();
            paged.record(slot).unwrap();
        }
        let cache = cache().lock().unwrap();
        assert!(cache.bytes <= BUDGET);
        assert_eq!(
            cache.bytes,
            cache
                .entries
                .iter()
                .map(|e| e.bytes.capacity())
                .sum::<usize>()
        );
    }
    #[test]
    fn corruption_truncation_binding_rebuild_and_runtime_error() {
        let temp = Temp::new();
        let source = temp.index(130, false);
        let path = temp.0.join("test.idx");
        let sidecar = path.with_extension("records04.idx");
        let paged = PagedRecords::prepare(&path, &source).unwrap();
        drop(paged);
        let original = std::fs::read(&sidecar).unwrap();
        for cut in [0, 7, 31, original.len() / 2, original.len() - 1] {
            std::fs::write(&sidecar, &original[..cut]).unwrap();
            let rebuilt = PagedRecords::prepare(&path, &source).unwrap();
            assert_eq!(rebuilt.record(129).unwrap(), source.record(129).unwrap());
            drop(rebuilt);
        }
        for position in [0, 8, 16, 20, 32, original.len() - 1] {
            let mut bytes = original.clone();
            bytes[position] ^= 1;
            std::fs::write(&sidecar, bytes).unwrap();
            let rebuilt = PagedRecords::prepare(&path, &source).unwrap();
            assert_eq!(rebuilt.record(64).unwrap(), source.record(64).unwrap());
            drop(rebuilt);
        }
        let paged = PagedRecords::prepare(&path, &source).unwrap();
        OpenOptions::new()
            .write(true)
            .open(&sidecar)
            .unwrap()
            .set_len(0)
            .unwrap();
        assert!(matches!(paged.record(64), Err(Error::Corrupt(_))));
    }
    #[test]
    fn text_windows_check_selected_crc_and_identity_and_handle_oversized_payloads() {
        let temp = Temp::new();
        let path = temp.0.join("test.idx");
        let mut builder = Builder::new(&path, 2, 42, 77).unwrap();
        for id in 0..4 {
            builder
                .add(&StoredDocument {
                    version: 1,
                    document: crate::Document {
                        key: Key::new("shared", "contents", &format!("{id:08}")),
                        title: "Şirket 東京 🦀".into(),
                        text: if id == 2 {
                            "x".repeat(80_000)
                        } else {
                            "içerik".into()
                        },
                        fields: BTreeMap::new(),
                        links: vec![],
                    },
                })
                .unwrap();
        }
        let source = builder.finish().unwrap();
        let paged = PagedRecords::prepare(&path, &source).unwrap();
        let mut cursor = TextCursor::default();
        for slot in [0, 2, 3] {
            assert_eq!(
                serde_json::to_value(
                    paged
                        .text_document(slot, &mut cursor, &source.file)
                        .unwrap()
                )
                .unwrap(),
                serde_json::to_value(source.read_document(slot).unwrap()).unwrap()
            );
        }
        assert!(cursor.payload.capacity() <= READ_WINDOW);
        let mut bytes = read_page(
            &paged.file,
            &read_directory(&paged.file, &paged.roots[0]).unwrap()[0],
        )
        .unwrap();
        let record = numeric_metadata(&bytes, 0).unwrap();
        let mut payload = vec![0; record.len as usize];
        read_at(&source.file, record.offset, &mut payload).unwrap();
        // CRC-valid key/version changes still fail identity verification.
        let mut value = crate::payload::decode(&payload).unwrap();
        value.document.key.id = "00000009".into();
        let changed = crate::payload::encode(&value).unwrap();
        let mut metadata = record;
        metadata.len = changed.len() as u32;
        metadata.checksum = crc(&changed);
        assert!(decode_text_payload(&bytes, 0, &metadata, &changed).is_err());
        value.document.key.id = "00000000".into();
        value.version = 2;
        let changed = crate::payload::encode(&value).unwrap();
        metadata.len = changed.len() as u32;
        metadata.checksum = crc(&changed);
        assert!(decode_text_payload(&bytes, 0, &metadata, &changed).is_err());
        payload[0] ^= 1;
        assert!(decode_text_payload(&bytes, 0, &record, &payload).is_err());
        bytes.truncate(4);
        assert!(numeric_metadata(&bytes, 0).is_err());
        let unselected = source.record(1).unwrap();
        let mut file = OpenOptions::new().write(true).open(&path).unwrap();
        file.seek(SeekFrom::Start(unselected.offset)).unwrap();
        file.write_all(b"!").unwrap();
        file.flush().unwrap();
        let mut fresh = TextCursor::default();
        // Slot 1 is inside slot 0's read window, but is not parsed until selected.
        paged.text_document(0, &mut fresh, &source.file).unwrap();
        assert!(matches!(
            paged.text_document(1, &mut fresh, &source.file),
            Err(Error::Corrupt(_))
        ));
    }
    #[test]
    fn maximal_keys_unicode_prefixes_empty_and_malformed_leaf() {
        let temp = Temp::new();
        let source = temp.index(66, true);
        let paged = PagedRecords::prepare(&temp.0.join("test.idx"), &source).unwrap();
        assert_eq!(paged.find(&source.key(65).unwrap()).unwrap(), Some(65));
        let records: Vec<_> = ["é", "ê", "東京🦀"]
            .into_iter()
            .enumerate()
            .map(|(n, id)| Record {
                key: Key::new("şirket", "belgeler", id),
                offset: n as u64,
                len: 1,
                checksum: 0,
                version: 1,
                deleted: false,
            })
            .collect();
        let bytes = encode_leaf(&records).unwrap();
        for (n, record) in records.iter().enumerate() {
            assert_eq!(&decode_leaf(&bytes, n).unwrap(), record);
            for other in &records {
                assert_eq!(
                    compare_leaf_key(&bytes, n, &other.key).unwrap(),
                    record.key.cmp(&other.key)
                );
            }
        }
        for cut in 0..bytes.len() {
            assert!(decode_leaf(&bytes[..cut], 2).is_err());
        }
        let mut invalid = bytes;
        invalid[..4].copy_from_slice(&65u32.to_le_bytes());
        assert!(decode_leaf(&invalid, 0).is_err());
        let empty_temp = Temp::new();
        let empty = empty_temp.index(0, false);
        let empty = PagedRecords::prepare(&empty_temp.0.join("test.idx"), &empty).unwrap();
        assert_eq!(empty.find(&Key::new("s", "c", "x")).unwrap(), None);
        assert!(empty.record(0).is_err());
    }
}
