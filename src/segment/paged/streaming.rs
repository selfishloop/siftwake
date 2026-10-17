//! Compare independently validated segment records with the unchanged MET04 pages.
use super::*;

struct Validator {
    paged: PagedRecords,
    root_offset: u64,
    cursor: u64,
    slot: usize,
    root: usize,
    leaves: Vec<Fence>,
    leaf: usize,
    page: Vec<u8>,
}
impl Validator {
    fn open(path: &Path, binding: (u64, u32, u64)) -> Result<Self> {
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
        {
            return Err(corrupt("binding/version mismatch"));
        }
        let count = u32::from_le_bytes(header[28..32].try_into().unwrap()) as usize;
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
        if roots.len() != count.div_ceil(LEAF * FANOUT) {
            return Err(corrupt("root count"));
        }
        Ok(Self {
            paged: PagedRecords {
                file,
                bytes: size,
                roots,
                count,
                owner: OWNER.fetch_add(1, Ordering::Relaxed),
                deleted: Vec::new(),
                last: None,
            },
            root_offset: offset,
            cursor: HEADER,
            slot: 0,
            root: 0,
            leaves: Vec::new(),
            leaf: 0,
            page: Vec::new(),
        })
    }
    fn page_bounds(&self, fence: &Fence) -> Result<()> {
        if fence.offset < HEADER
            || fence
                .offset
                .checked_add(fence.len as u64)
                .is_none_or(|end| end > self.root_offset)
        {
            return Err(corrupt("page bounds"));
        }
        Ok(())
    }
    fn push(&mut self, expected: &Record) -> Result<()> {
        if self.slot >= self.paged.count {
            return Err(corrupt("record count"));
        }
        let local = self.slot % LEAF;
        if local == 0 {
            if self.slot.is_multiple_of(LEAF * FANOUT) {
                let root = self
                    .paged
                    .roots
                    .get(self.root)
                    .ok_or_else(|| corrupt("missing root"))?;
                self.page_bounds(root)?;
                self.leaves = read_directory(&self.paged.file, root)?;
                self.leaf = 0;
                if root.slot as usize != self.slot
                    || self.leaves.is_empty()
                    || self.leaves.len()
                        != FANOUT.min((self.paged.count - self.slot).div_ceil(LEAF))
                    || root.key != self.leaves[0].key
                {
                    return Err(corrupt("root identity"));
                }
            }
            let leaf = &self.leaves[self.leaf];
            self.page_bounds(leaf)?;
            if leaf.offset != self.cursor || leaf.slot as usize != self.slot {
                return Err(corrupt("leaf order"));
            }
            // Release the old owned leaf before allocating the next page.
            self.page = Vec::new();
            self.page = read_page(&self.paged.file, leaf)?;
            if leaf_count(&self.page)? != LEAF.min(self.paged.count - self.slot) {
                return Err(corrupt("leaf count"));
            }
        }
        let actual = numeric_metadata(&self.page, local)?;
        if actual.offset != expected.offset
            || actual.len != expected.len
            || actual.version != expected.version
            || actual.checksum != expected.checksum
            || actual.deleted != expected.deleted
            || compare_leaf_key(&self.page, local, &expected.key)? != std::cmp::Ordering::Equal
            || (local == 0 && expected.key != self.leaves[self.leaf].key)
        {
            return Err(corrupt("record identity"));
        }
        if self.slot.is_multiple_of(64) {
            self.paged.deleted.push(0);
        }
        if actual.deleted {
            self.paged.deleted[self.slot / 64] |= 1 << (self.slot % 64);
        }
        if self.slot + 1 == self.paged.count {
            self.paged.last = Some(expected.key.clone());
        }
        self.slot += 1;
        if self.slot.is_multiple_of(LEAF) || self.slot == self.paged.count {
            self.cursor += self.leaves[self.leaf].len as u64;
            self.leaf += 1;
            if self.leaf == self.leaves.len() {
                let root = &self.paged.roots[self.root];
                if root.offset != self.cursor {
                    return Err(corrupt("directory order"));
                }
                self.cursor += root.len as u64;
                self.root += 1;
            }
        }
        Ok(())
    }
    fn finish(self) -> Result<PagedRecords> {
        if self.slot != self.paged.count
            || self.root != self.paged.roots.len()
            || self.cursor != self.root_offset
        {
            return Err(corrupt("noncontiguous layout/count"));
        }
        Ok(self.paged)
    }
}

struct Writer {
    file: Option<File>,
    temporary: PathBuf,
    target: PathBuf,
    records: Vec<Record>,
    leaves: Vec<Fence>,
    roots: Vec<Fence>,
    count: u32,
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.file.take();
        let _ = std::fs::remove_file(&self.temporary);
    }
}
impl Writer {
    fn new(target: PathBuf, binding: (u64, u32, u64)) -> Result<Self> {
        let temporary = target.with_extension(format!(
            "stream-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let mut result = Self {
            file: Some(file),
            temporary,
            target,
            records: Vec::with_capacity(LEAF),
            leaves: Vec::with_capacity(FANOUT),
            roots: Vec::new(),
            count: 0,
        };
        let file = result.file.as_mut().unwrap();
        file.write_all(MAGIC)?;
        file.write_all(&binding.0.to_le_bytes())?;
        file.write_all(&binding.1.to_le_bytes())?;
        file.write_all(&binding.2.to_le_bytes())?;
        file.write_all(&0u32.to_le_bytes())?;
        Ok(result)
    }
    fn push(&mut self, record: &Record) -> Result<()> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| corrupt("record count capacity"))?;
        self.records.push(Record {
            key: record.key.clone(),
            offset: record.offset,
            len: record.len,
            checksum: record.checksum,
            version: record.version,
            deleted: record.deleted,
        });
        if self.records.len() == LEAF {
            self.flush_leaf()?;
        }
        Ok(())
    }
    fn flush_leaf(&mut self) -> Result<()> {
        if self.records.is_empty() {
            return Ok(());
        }
        let bytes = encode_leaf(&self.records)?;
        let fence = write_page(
            self.file.as_mut().unwrap(),
            self.records[0].key.clone(),
            self.count - self.records.len() as u32,
            &bytes,
        )?;
        self.leaves.push(fence);
        self.records.clear();
        if self.leaves.len() == FANOUT {
            self.flush_directory()?;
        }
        Ok(())
    }
    fn flush_directory(&mut self) -> Result<()> {
        if self.leaves.is_empty() {
            return Ok(());
        }
        let records: Vec<_> = self
            .leaves
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
        let fence = write_page(
            self.file.as_mut().unwrap(),
            self.leaves[0].key.clone(),
            self.leaves[0].slot,
            &bytes,
        )?;
        self.roots.push(fence);
        self.leaves.clear();
        Ok(())
    }
    fn finish(mut self) -> Result<()> {
        self.flush_leaf()?;
        self.flush_directory()?;
        let file = self.file.as_mut().unwrap();
        let offset = file.stream_position()?;
        let mut bytes = Vec::new();
        serde_json::to_writer(
            crate::bounded::Sink::new(&mut bytes, 16 * 1024 * 1024),
            &self.roots,
        )?;
        file.write_all(&bytes)?;
        let end = file.stream_position()?;
        file.seek(SeekFrom::Start(28))?;
        file.write_all(&self.count.to_le_bytes())?;
        let checksum = prefix_crc(file, end)?;
        file.seek(SeekFrom::Start(end))?;
        let mut footer = END.to_vec();
        footer.extend_from_slice(&offset.to_le_bytes());
        footer.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        footer.extend_from_slice(&checksum.to_le_bytes());
        footer.extend_from_slice(&crc(&footer).to_le_bytes());
        file.write_all(&footer)?;
        file.sync_all()?;
        self.file.take();
        match std::fs::remove_file(&self.target) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        std::fs::rename(&self.temporary, &self.target)?;
        Ok(())
    }
}

pub(in crate::segment) fn prepare(
    path: &Path,
    file: &mut File,
    size: u64,
    footer_crc: u32,
    sequence: u64,
    offset: u64,
    len: u64,
) -> Result<(PagedRecords, super::super::streaming::Parsed)> {
    #[cfg(feature = "diagnostic-timing")]
    let _span = crate::timing::span("open.index_metadata_prepare");
    let target = path.with_extension("records04.idx");
    let binding = (size, footer_crc, sequence);
    let validate = |file: &mut File| -> Result<_> {
        let mut validator = Validator::open(&target, binding)?;
        let parsed = super::super::streaming::read_with(file, offset, len, sequence, |record| {
            validator.push(record)
        })?;
        Ok((validator.finish()?, parsed))
    };
    match validate(file) {
        Ok(result) => return Ok(result),
        Err(Error::Corrupt(_)) => {}
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut writer = Writer::new(target.clone(), binding)?;
    let parsed = super::super::streaming::read_with(file, offset, len, sequence, |record| {
        writer.push(record)
    })?;
    drop(parsed);
    writer.finish()?;
    // Independently re-read SEG03 and the emitted pages before using any cache.
    validate(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fdb-stream-meta-{}-{}",
                std::process::id(),
                TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> PathBuf {
            self.0.join("test.idx")
        }
        fn build(&self, count: usize) -> DiskIndex {
            let mut builder = Builder::new(&self.path(), 2, 42, 77).unwrap();
            for id in 0..count {
                let key = Key::new("şirket", "東京", &format!("🦀{id:08}"));
                if id % 7 == 0 {
                    builder.add_delete(&key).unwrap();
                } else {
                    builder
                        .add(&StoredDocument {
                            version: 1,
                            document: crate::Document {
                                key,
                                title: String::new(),
                                text: String::new(),
                                fields: BTreeMap::new(),
                                links: Vec::new(),
                            },
                        })
                        .unwrap();
                }
            }
            builder.finish().unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let root = std::env::temp_dir().canonicalize().unwrap();
            let target = self.0.canonicalize().unwrap();
            assert!(
                target.starts_with(root)
                    && target
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("fdb-stream-meta-")
            );
            let _ = std::fs::remove_dir_all(target);
        }
    }
    fn refresh_crc(bytes: &mut [u8]) {
        let end = bytes.len() - FOOTER_SIZE as usize;
        let total = crc(&bytes[..end]);
        bytes[end + 24..end + 28].copy_from_slice(&total.to_le_bytes());
        let footer = crc(&bytes[end..end + 28]);
        bytes[end + 28..].copy_from_slice(&footer.to_le_bytes());
    }
    #[test]
    fn streamed_pages_are_byte_compatible_and_have_no_resident_record_table() {
        for count in [0, 1, 63, 64, 65, 2047, 2048, 2049] {
            let temp = Temp::new();
            let index = temp.build(count);
            assert_eq!(index.len(), count);
            assert!(index.paged.is_some());
            assert_eq!(index.records.capacity(), 0);
            assert_eq!(index.ids.capacity(), 0);
            let sidecar = temp.path().with_extension("records04.idx");
            let streamed = std::fs::read(&sidecar).unwrap();
            for id in 0..count {
                assert_eq!(index.deleted(id as u32), id % 7 == 0);
            }
            drop(index);
            let source = DiskIndex::open_with_mode(&temp.path(), None, false).unwrap();
            std::fs::remove_file(&sidecar).unwrap();
            let legacy = PagedRecords::prepare(&temp.path(), &source).unwrap();
            assert_eq!(std::fs::read(&sidecar).unwrap(), streamed);
            for id in [0, count / 2, count.saturating_sub(1)] {
                if id < count {
                    assert_eq!(
                        legacy.record(id as u32).unwrap(),
                        source.record(id as u32).unwrap()
                    );
                }
            }
        }
    }
    #[test]
    fn every_torn_sidecar_and_crc_valid_wrong_record_rebuild_from_seg03() {
        let temp = Temp::new();
        let index = temp.build(2);
        let sidecar = temp.path().with_extension("records04.idx");
        let original = std::fs::read(&sidecar).unwrap();
        let segment = std::fs::read(temp.path()).unwrap();
        let paged = index.paged.as_ref().unwrap();
        let leaf = read_directory(&paged.file, &paged.roots[0])
            .unwrap()
            .remove(0);
        let mut records: Vec<_> = (0..2).map(|id| index.record(id).unwrap()).collect();
        records[1].version = 2;
        let mut wrong_page = encode_leaf(&records).unwrap();
        wrong_page.extend_from_slice(&crc(&wrong_page).to_le_bytes());
        assert_eq!(wrong_page.len(), leaf.len as usize);
        drop(index);
        for cut in 0..original.len() {
            std::fs::write(&sidecar, &original[..cut]).unwrap();
            let reopened = DiskIndex::open(&temp.path(), None).unwrap();
            assert_eq!(reopened.len(), 2);
            assert_eq!(std::fs::read(&sidecar).unwrap(), original);
            drop(reopened);
        }
        let mut wrong = original.clone();
        wrong[leaf.offset as usize..leaf.offset as usize + wrong_page.len()]
            .copy_from_slice(&wrong_page);
        refresh_crc(&mut wrong);
        std::fs::write(&sidecar, wrong).unwrap();
        let reopened = DiskIndex::open(&temp.path(), None).unwrap();
        assert_eq!(reopened.record(1).unwrap().version, 1);
        assert_eq!(std::fs::read(&sidecar).unwrap(), original);
        assert_eq!(std::fs::read(temp.path()).unwrap(), segment);
        assert!(std::fs::read_dir(&temp.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .path()
                .extension()
                .unwrap()
                .to_string_lossy()
                .starts_with("stream-")
        }));
    }
    #[test]
    fn intact_sidecar_cannot_hide_invalid_seg03_record_directory() {
        let temp = Temp::new();
        drop(temp.build(2));
        let mut bytes = std::fs::read(temp.path()).unwrap();
        let footer = bytes.len() - FOOTER_SIZE as usize;
        let start = u64::from_le_bytes(bytes[footer + 8..footer + 16].try_into().unwrap()) as usize;
        let mut directory: Directory = serde_json::from_slice(&bytes[start..footer]).unwrap();
        directory.records[1].version = 0;
        let encoded = serde_json::to_vec(&directory).unwrap();
        assert_eq!(encoded.len(), footer - start);
        bytes[start..footer].copy_from_slice(&encoded);
        refresh_crc(&mut bytes);
        std::fs::write(temp.path(), &bytes).unwrap();
        assert!(matches!(
            DiskIndex::open(&temp.path(), None),
            Err(Error::Corrupt(_))
        ));
        assert_eq!(std::fs::read(temp.path()).unwrap(), bytes);
    }
}
