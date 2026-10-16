//! REF01 source addresses + VFY01 source blocks. Both are disposable caches.
use crate::{
    Error, Key, Result, StoredDocument, format,
    manifest::Source,
    segment::{DiskIndex, block_check::Checks, read_at},
};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

const MAGIC: &[u8; 8] = b"FDBREF01";
const HEADER: usize = 36;
const MAX: u64 = 64 * 1024 * 1024;
#[derive(Clone, Copy)]
struct Address {
    offset: u64,
    len: u32,
    checksum: u32,
}
pub(crate) struct Records {
    file: File,
    blocks: Checks,
    addresses: Vec<Address>,
}

fn header(source: &Source, index: &DiskIndex) -> Result<Vec<u8>> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&source.end.to_le_bytes());
    bytes.extend_from_slice(&source.checksum.to_le_bytes());
    bytes.extend_from_slice(&source.sequence.to_le_bytes());
    bytes.extend_from_slice(&index.checksum_binding()?.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(source.keys)
            .map_err(|_| Error::Invalid("source address capacity".into()))?
            .to_le_bytes(),
    );
    Ok(bytes)
}
impl Records {
    pub(crate) fn load(dir: &Path, source: &Source, index: &DiskIndex) -> Result<Self> {
        let mut input = File::open(source.path(dir, "rmp"))?;
        let size = input.metadata()?.len();
        if size > MAX || size != HEADER as u64 + source.keys as u64 * 16 + 4 {
            return Err(Error::Corrupt("source address table length".into()));
        }
        let mut bytes = Vec::with_capacity(size as usize);
        (&mut input).take(size + 1).read_to_end(&mut bytes)?;
        let binding = header(source, index)?;
        if bytes.len() != size as usize
            || bytes[..HEADER] != binding
            || format::crc(&bytes[..bytes.len() - 4])
                != u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap())
        {
            return Err(Error::Corrupt(
                "source address table binding/checksum".into(),
            ));
        }
        let mut addresses = Vec::with_capacity(source.keys);
        for (slot, entry) in bytes[HEADER..bytes.len() - 4]
            .as_chunks::<16>()
            .0
            .iter()
            .enumerate()
        {
            let row = Address {
                offset: u64::from_le_bytes(entry[..8].try_into().unwrap()),
                len: u32::from_le_bytes(entry[8..12].try_into().unwrap()),
                checksum: u32::from_le_bytes(entry[12..].try_into().unwrap()),
            };
            let (len, checksum, _, deleted) = index.payload_binding(slot as u32)?;
            if row.offset < 32
                || row.len == 0
                || row.len as usize > format::MAX_FRAME
                || row
                    .offset
                    .checked_add(row.len as u64)
                    .is_none_or(|end| end > source.end)
                || (!deleted && (row.len != len || row.checksum != checksum))
            {
                return Err(Error::Corrupt("source address bounds/index binding".into()));
            }
            addresses.push(row);
        }
        let file = File::open(source.path(dir, "fdb"))?;
        if file.metadata()?.len() != source.end {
            return Err(Error::Corrupt("immutable source length mismatch".into()));
        }
        let blocks = Checks::load_file(&source.path(dir, "svfy"), source.end, source.checksum)?;
        blocks.verify(&file, 0, 8)?;
        Ok(Self {
            file,
            blocks,
            addresses,
        })
    }
    pub(crate) fn read(&self, id: u32, key: &Key, version: u64) -> Result<StoredDocument> {
        #[cfg(feature = "diagnostic-timing")]
        let _read = crate::timing::span("source.record_get");
        let row = self
            .addresses
            .get(id as usize)
            .ok_or_else(|| Error::Corrupt("source address slot".into()))?;
        self.blocks
            .verify(&self.file, row.offset, row.len as usize)?;
        let mut bytes = vec![0; row.len as usize];
        read_at(&self.file, row.offset, &mut bytes)?;
        if format::crc(&bytes) != row.checksum {
            return Err(Error::Corrupt("source record checksum mismatch".into()));
        }
        let stored = crate::payload::decode(&bytes)?;
        crate::validate_doc(&stored.document).map_err(|e| Error::Corrupt(e.to_string()))?;
        if &stored.document.key != key || stored.version != version {
            return Err(Error::Corrupt("source record identity mismatch".into()));
        }
        Ok(stored)
    }

    pub(crate) fn prepare(dir: &Path, source: &Source, index: &DiskIndex) -> Result<()> {
        if source.keys > ((MAX as usize - HEADER - 4) / 16) {
            return Err(Error::Invalid("source address capacity".into()));
        }
        let mut file = File::open(source.path(dir, "fdb"))?;
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        if &magic != format::MAGIC_V4 {
            return Err(Error::Invalid("source addresses require FDBLOG04".into()));
        }
        let mut position = 8u64;
        let mut rows = vec![None; source.keys];
        let mut root = format::crc(&magic);
        while position < source.end {
            let mut frame = [0; 24];
            file.read_exact(&mut frame)?;
            let len = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
            if &frame[..4] != b"TXN2"
                || format::crc(&frame[..20]) != u32::from_le_bytes(frame[20..].try_into().unwrap())
                || u64::from_le_bytes(frame[8..16].try_into().unwrap()) != source.sequence
                || len > format::MAX_FRAME
                || position
                    .checked_add(32 + len as u64)
                    .is_none_or(|end| end > source.end)
            {
                return Err(Error::Corrupt("source address frame header".into()));
            }
            let mut payload = vec![0; len];
            file.read_exact(&mut payload)?;
            let mut footer = [0; 8];
            file.read_exact(&mut footer)?;
            let checksum = format::crc(&payload);
            if &footer != b"FDBDONE1"
                || checksum != u32::from_le_bytes(frame[16..20].try_into().unwrap())
            {
                return Err(Error::Corrupt("source address frame checksum".into()));
            }
            root = !format::crc_extend(!root, &frame);
            root = format::crc_combine(root, checksum, len as u64);
            root = !format::crc_extend(!root, &footer);
            let (checkpoint, entries) = crate::binary_batch::entries(&payload)?;
            if !checkpoint {
                return Err(Error::Corrupt("source address checkpoint flag".into()));
            }
            for (tag, relative, bytes) in entries {
                let (key, deleted) = match crate::binary_batch::decode_entry(tag, bytes)? {
                    format::Change::Put { stored } => {
                        crate::validate_doc(&stored.document)
                            .map_err(|e| Error::Corrupt(e.to_string()))?;
                        let version = index
                            .version(&stored.document.key)?
                            .ok_or_else(|| Error::Corrupt("source address missing key".into()))?;
                        if version != stored.version {
                            return Err(Error::Corrupt("source address version".into()));
                        }
                        (stored.document.key, false)
                    }
                    format::Change::Delete { key } => (key, true),
                };
                let slot = index
                    .find(&key)?
                    .ok_or_else(|| Error::Corrupt("source address missing key".into()))?;
                if index.deleted(slot) != deleted || rows[slot as usize].is_some() {
                    return Err(Error::Corrupt(
                        "source address duplicate/deleted key".into(),
                    ));
                }
                rows[slot as usize] = Some(Address {
                    offset: position + 24 + relative as u64,
                    len: bytes.len() as u32,
                    checksum: format::crc(bytes),
                });
            }
            position += 32 + len as u64;
        }
        if root != source.checksum || rows.iter().any(Option::is_none) {
            return Err(Error::Corrupt("source address root/count".into()));
        }
        Checks::prepare_file(
            &source.path(dir, "svfy"),
            &file,
            source.end,
            source.checksum,
        )?;
        let mut bytes = header(source, index)?;
        for row in rows.into_iter().flatten() {
            bytes.extend_from_slice(&row.offset.to_le_bytes());
            bytes.extend_from_slice(&row.len.to_le_bytes());
            bytes.extend_from_slice(&row.checksum.to_le_bytes());
        }
        bytes.extend_from_slice(&format::crc(&bytes).to_le_bytes());
        let target = source.path(dir, "rmp");
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temporary = target.with_extension(format!(
            "rmp-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let mut owned = false;
        let result = (|| {
            let mut output = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            owned = true;
            output.write_all(&bytes)?;
            output.sync_all()?;
            drop(output);
            #[cfg(test)]
            if let Ok(marker) = std::env::var("FDB_TEST_REF_STAGE_MARKER") {
                std::fs::write(marker, b"staged")?;
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
            std::fs::rename(&temporary, target)?;
            Ok(())
        })();
        if owned {
            let _ = std::fs::remove_file(temporary);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    #[ignore = "process helper"]
    fn prepare_child() {
        let root = std::env::var("FDB_TEST_REF_ROOT").unwrap();
        crate::Database::open(root)
            .unwrap()
            .prepare_record_reads()
            .unwrap();
    }
    #[test]
    fn interrupted_address_publication_preserves_prior_map_and_source() {
        let temp = Temp(std::env::temp_dir().join(format!(
                "fdb-ref-kill-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        let mut db = crate::Database::open(&temp.0).unwrap();
        let key = Key::new("shared", "docs", "a");
        let doc = crate::Document {
            key: key.clone(),
            title: "Title".into(),
            text: "actual source".into(),
            fields: Default::default(),
            links: vec![],
        };
        db.commit(vec![crate::Mutation::Put {
            document: doc.clone(),
            guard: crate::Guard::Any,
        }])
        .unwrap();
        db.compact().unwrap();
        db.prepare_record_reads().unwrap();
        drop(db);
        let map = std::fs::read_dir(&temp.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "rmp"))
            .unwrap();
        let prior = std::fs::read(&map).unwrap();
        let source = map.with_extension("fdb");
        let original = std::fs::read(&source).unwrap();
        let marker = temp.0.join("staged");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "source_records::tests::prepare_child",
                "--ignored",
            ])
            .env("FDB_TEST_REF_ROOT", &temp.0)
            .env("FDB_TEST_REF_STAGE_MARKER", &marker)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let start = std::time::Instant::now();
        while !marker.exists() && start.elapsed() < std::time::Duration::from_secs(20) {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let reached = marker.exists();
        let _ = child.kill();
        child.wait().unwrap();
        assert!(reached);
        assert_eq!(std::fs::read(map).unwrap(), prior);
        assert_eq!(std::fs::read(source).unwrap(), original);
        let db = crate::Database::open_record_reads(&temp.0).unwrap();
        assert_eq!(db.get(&key).unwrap().unwrap().document, doc);
        assert_eq!(db.pending_source_checks().unwrap(), 1);
    }
}
