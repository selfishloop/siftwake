//! Disposable VFY01 checksum cache; SEG04 and authoritative sources stay unchanged.
use super::{Error, File, Path, Result, read_at};
use crate::format::{crc, crc_combine};
use std::io::Write;
use std::sync::Mutex;

const MAGIC: &[u8; 8] = b"FDBVFY01";
const BLOCK: u64 = 256 * 1024;
const HEADER: usize = 32;
const MAX_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) struct Checks {
    end: u64,
    crcs: Vec<u32>,
    checked: Mutex<Vec<bool>>,
}
impl Checks {
    pub(super) fn load(path: &Path, end: u64, expected: u32) -> Result<Self> {
        let path = path.with_extension("vfy");
        Self::load_file(&path, end, expected)
    }
    pub(crate) fn load_file(path: &Path, end: u64, expected: u32) -> Result<Self> {
        let mut input = File::open(path)?;
        let size = input.metadata()?.len();
        if !(HEADER as u64 + 4..=MAX_BYTES).contains(&size) {
            return Err(Error::Corrupt("block-check cache length".into()));
        }
        use std::io::Read;
        let mut bytes = Vec::with_capacity(size as usize);
        (&mut input).take(size + 1).read_to_end(&mut bytes)?;
        if bytes.len() != size as usize
            || &bytes[..8] != MAGIC
            || u64::from_le_bytes(bytes[8..16].try_into().unwrap()) != end
            || u64::from_le_bytes(bytes[16..24].try_into().unwrap()) != BLOCK
            || u32::from_le_bytes(bytes[24..28].try_into().unwrap()) != expected
        {
            return Err(Error::Corrupt("block-check cache binding/version".into()));
        }
        let count = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
        if count as u64 != end.div_ceil(BLOCK)
            || HEADER + count * 4 + 4 != bytes.len()
            || crc(&bytes[..bytes.len() - 4])
                != u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap())
        {
            return Err(Error::Corrupt(
                "block-check cache table checksum/bounds".into(),
            ));
        }
        let crcs: Vec<_> = bytes[HEADER..bytes.len() - 4]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        let mut root = 0;
        for (n, &value) in crcs.iter().enumerate() {
            root = crc_combine(root, value, (end - n as u64 * BLOCK).min(BLOCK));
        }
        if root != expected {
            return Err(Error::Corrupt("block-check root mismatch".into()));
        }
        Ok(Self {
            end,
            checked: Mutex::new(vec![false; count]),
            crcs,
        })
    }

    /// Check entire touched blocks once per session before exposing their bytes.
    pub(crate) fn verify(&self, file: &File, offset: u64, len: usize) -> Result<()> {
        let end = offset
            .checked_add(len as u64)
            .filter(|end| *end <= self.end)
            .ok_or_else(|| Error::Corrupt("checked index read bounds".into()))?;
        if len == 0 {
            return Ok(());
        }
        let mut checked = self
            .checked
            .lock()
            .map_err(|_| Error::Corrupt("block-check mutex poisoned".into()))?;
        let first = offset / BLOCK;
        let last = (end - 1) / BLOCK;
        let mut scratch = Vec::new();
        for n in first..=last {
            let n = n as usize;
            if checked[n] {
                continue;
            }
            let start = n as u64 * BLOCK;
            scratch.resize((self.end - start).min(BLOCK) as usize, 0);
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("index.verify_block");
            read_at(file, start, &mut scratch)?;
            if crc(&scratch) != self.crcs[n] {
                return Err(Error::Corrupt("index block checksum mismatch".into()));
            }
            checked[n] = true;
        }
        Ok(())
    }

    pub(super) fn prepare(path: &Path, file: &File, end: u64, expected: u32) -> Result<()> {
        Self::prepare_file(&path.with_extension("vfy"), file, end, expected)
    }
    pub(crate) fn prepare_file(target: &Path, file: &File, end: u64, expected: u32) -> Result<()> {
        // Preparation is explicit. Verify all bytes before publishing derived metadata.
        let count = end.div_ceil(BLOCK);
        if count > (MAX_BYTES - HEADER as u64 - 4) / 4 {
            return Err(Error::Invalid("block-check cache capacity".into()));
        }
        let mut bytes = Vec::with_capacity(HEADER + count as usize * 4 + 4);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&end.to_le_bytes());
        bytes.extend_from_slice(&BLOCK.to_le_bytes());
        bytes.extend_from_slice(&expected.to_le_bytes());
        bytes.extend_from_slice(&(count as u32).to_le_bytes());
        let mut scratch = vec![0; BLOCK as usize];
        let mut root = 0;
        for n in 0..count {
            let len = (end - n * BLOCK).min(BLOCK) as usize;
            read_at(file, n * BLOCK, &mut scratch[..len])?;
            let value = crc(&scratch[..len]);
            root = crc_combine(root, value, len as u64);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        if root != expected {
            return Err(Error::Corrupt(
                "segment checksum mismatch during cache preparation".into(),
            ));
        }
        bytes.extend_from_slice(&crc(&bytes).to_le_bytes());
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temporary = target.with_extension(format!(
            "vfy-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let result = (|| {
            let mut output = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            output.write_all(&bytes)?;
            output.sync_all()?;
            drop(output);
            #[cfg(test)]
            if let Ok(marker) = std::env::var("FDB_TEST_VFY_STAGE_MARKER") {
                std::fs::write(marker, b"staged")?;
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
            std::fs::rename(&temporary, target)?;
            Ok(())
        })();
        let _ = std::fs::remove_file(temporary);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fdb-vfy-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> std::path::PathBuf {
            self.0.join("test.idx")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lazy_blocks_check_boundaries_corruption_cuts_and_table_root() {
        let temp = Temp::new();
        let bytes: Vec<_> = (0..BLOCK * 2 + 17)
            .map(|n| (n.wrapping_mul(71) % 256) as u8)
            .collect();
        let path = temp.path();
        std::fs::write(&path, &bytes).unwrap();
        let file = File::open(&path).unwrap();
        let expected = crc(&bytes);
        Checks::prepare(&path, &file, bytes.len() as u64, expected).unwrap();
        let table = std::fs::read(path.with_extension("vfy")).unwrap();
        let checks = Checks::load(&path, bytes.len() as u64, expected).unwrap();
        checks.verify(&file, BLOCK - 1, 2).unwrap();
        assert_eq!(*checks.checked.lock().unwrap(), vec![true, true, false]);
        assert!(checks.verify(&file, bytes.len() as u64, 1).is_err());
        let mut corrupt = bytes.clone();
        corrupt[BLOCK as usize * 2 + 1] ^= 1;
        std::fs::write(&path, &corrupt).unwrap();
        assert!(matches!(
            checks.verify(&file, BLOCK * 2, 2),
            Err(Error::Corrupt(_))
        ));
        assert!(!checks.checked.lock().unwrap()[2]);
        std::fs::write(&path, &bytes[..BLOCK as usize * 2]).unwrap();
        assert!(checks.verify(&file, BLOCK * 2, 2).is_err());
        std::fs::write(&path, &bytes).unwrap();
        for cut in 0..table.len() {
            std::fs::write(path.with_extension("vfy"), &table[..cut]).unwrap();
            assert!(Checks::load(&path, bytes.len() as u64, expected).is_err());
        }
        for n in 0..table.len() {
            let mut corrupt = table.clone();
            corrupt[n] ^= 1;
            std::fs::write(path.with_extension("vfy"), &corrupt).unwrap();
            assert!(Checks::load(&path, bytes.len() as u64, expected).is_err());
        }
        let mut corrupt = table.clone();
        corrupt[HEADER] ^= 1;
        let end = corrupt.len() - 4;
        let checksum = crc(&corrupt[..end]);
        corrupt[end..].copy_from_slice(&checksum.to_le_bytes());
        std::fs::write(path.with_extension("vfy"), &corrupt).unwrap();
        assert!(
            matches!(Checks::load(&path, bytes.len() as u64, expected), Err(Error::Corrupt(message)) if message == "block-check root mismatch")
        );
        std::fs::write(path.with_extension("vfy"), table).unwrap();
        assert!(Checks::load(&path, bytes.len() as u64 + 1, expected).is_err());
    }

    #[test]
    #[ignore = "process helper"]
    fn prepare_child() {
        let path = std::path::PathBuf::from(std::env::var("FDB_TEST_VFY_SOURCE").unwrap());
        let bytes = std::fs::read(&path).unwrap();
        Checks::prepare(
            &path,
            &File::open(&path).unwrap(),
            bytes.len() as u64,
            crc(&bytes),
        )
        .unwrap();
    }

    #[test]
    fn process_kill_before_publication_keeps_previous_checksum_cache() {
        let temp = Temp::new();
        let path = temp.path();
        let bytes = b"original immutable test bytes";
        std::fs::write(&path, bytes).unwrap();
        Checks::prepare(
            &path,
            &File::open(&path).unwrap(),
            bytes.len() as u64,
            crc(bytes),
        )
        .unwrap();
        let prior = std::fs::read(path.with_extension("vfy")).unwrap();
        let marker = temp.0.join("staged");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "segment::block_check::tests::prepare_child",
                "--ignored",
            ])
            .env("FDB_TEST_VFY_SOURCE", &path)
            .env("FDB_TEST_VFY_STAGE_MARKER", &marker)
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
        assert_eq!(std::fs::read(path.with_extension("vfy")).unwrap(), prior);
        Checks::load(&path, bytes.len() as u64, crc(bytes))
            .unwrap()
            .verify(&File::open(&path).unwrap(), 0, bytes.len())
            .unwrap();
    }
}
