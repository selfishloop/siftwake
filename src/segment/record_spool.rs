//! Checked temporary builder metadata; drains to JSON or a SEG05 directory.
use super::*;
use std::io::{BufReader, Read};
const LIMIT: usize = 256 * 1024;
pub(super) struct Records {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    pub count: usize,
    pub last: Option<Key>,
}
impl Records {
    pub fn new(stem: &Path) -> Result<Self> {
        let path = stem.with_extension(format!(
            "{}-records",
            stem.extension().unwrap().to_string_lossy()
        ));
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)?;
        Ok(Self {
            path,
            writer: Some(BufWriter::with_capacity(8 * 1024, file)),
            count: 0,
            last: None,
        })
    }
    pub fn push(&mut self, record: Record) -> Result<()> {
        let bytes = serde_json::to_vec(&record)?;
        if bytes.len() > LIMIT {
            return Err(Error::Invalid("builder record metadata limit".into()));
        }
        let writer = self.writer.as_mut().unwrap();
        writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
        writer.write_all(&crc(&bytes).to_le_bytes())?;
        writer.write_all(&bytes)?;
        self.count += 1;
        self.last = Some(record.key);
        Ok(())
    }
    pub fn write_json(&mut self, output: &mut impl Write) -> Result<()> {
        output.write_all(b"[")?;
        self.drain(|n, bytes| {
            if n != 0 {
                output.write_all(b",")?;
            }
            output.write_all(bytes)?;
            Ok(())
        })?;
        output.write_all(b"]")?;
        Ok(())
    }
    #[cfg(feature = "bounded-engine")]
    pub fn write_binary(&mut self, output: &mut impl Write) -> Result<()> {
        self.drain(|_, bytes| super::compact::write_record(output, &serde_json::from_slice(bytes)?))
    }
    fn drain(&mut self, mut accept: impl FnMut(usize, &[u8]) -> Result<()>) -> Result<()> {
        let mut writer = self.writer.take().unwrap();
        writer.flush()?;
        let mut file = writer.into_inner().map_err(|e| Error::Io(e.into_error()))?;
        file.seek(SeekFrom::Start(0))?;
        let mut input = BufReader::with_capacity(8 * 1024, file);
        let mut scratch = Vec::new();
        for n in 0..self.count {
            let mut header = [0; 8];
            input.read_exact(&mut header)?;
            let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
            if len == 0 || len > LIMIT {
                return Err(Error::Corrupt("builder spool bounds".into()));
            }
            if len > scratch.capacity() {
                scratch.reserve_exact(len - scratch.len());
            }
            scratch.resize(len, 0);
            input.read_exact(&mut scratch)?;
            if crc(&scratch) != u32::from_le_bytes(header[4..].try_into().unwrap()) {
                return Err(Error::Corrupt("builder spool checksum".into()));
            }
            accept(n, &scratch)?;
        }
        let mut extra = [0];
        if input.read(&mut extra)? != 0 {
            return Err(Error::Corrupt("builder spool trailing bytes".into()));
        }
        Ok(())
    }
}
impl Drop for Records {
    fn drop(&mut self) {
        self.writer.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stem() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fdb-spool-{}-{}.build",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn record(id: usize) -> Record {
        Record {
            key: Key::new("şirket", "東京", &format!("quote\"back\\🦀{id:05}")),
            offset: HEADER_SIZE + id as u64 * 7,
            len: 7,
            checksum: 23,
            version: 4,
            deleted: false,
        }
    }
    #[test]
    fn streamed_directory_is_byte_compatible_and_spool_is_removed() {
        let mut spool = Records::new(&stem()).unwrap();
        let path = spool.path.clone();
        let reference: Vec<_> = (0..200).map(record).collect();
        for item in (0..200).map(record) {
            spool.push(item).unwrap();
        }
        assert_eq!(spool.count, 200);
        let mut bytes = Vec::new();
        spool.write_json(&mut bytes).unwrap();
        assert_eq!(bytes, serde_json::to_vec(&reference).unwrap());
        drop(spool);
        assert!(!path.exists());
        let mut empty = Records::new(&stem()).unwrap();
        let mut bytes = Vec::new();
        empty.write_json(&mut bytes).unwrap();
        assert_eq!(bytes, b"[]");
    }
    #[test]
    fn every_torn_spool_and_payload_corruption_fails_without_source_mutation() {
        let mut first = Records::new(&stem()).unwrap();
        first.push(record(0)).unwrap();
        first.writer.as_mut().unwrap().flush().unwrap();
        let original = std::fs::read(&first.path).unwrap();
        drop(first);
        for cut in 0..original.len() {
            let mut spool = Records::new(&stem()).unwrap();
            spool.push(record(0)).unwrap();
            spool.writer.as_mut().unwrap().flush().unwrap();
            let file = OpenOptions::new().write(true).open(&spool.path).unwrap();
            file.set_len(cut as u64).unwrap();
            drop(file);
            assert!(spool.write_json(&mut Vec::new()).is_err(), "cut {cut}");
        }
        let mut spool = Records::new(&stem()).unwrap();
        spool.push(record(0)).unwrap();
        spool.writer.as_mut().unwrap().flush().unwrap();
        let mut corrupt = original;
        corrupt[12] ^= 1;
        std::fs::write(&spool.path, &corrupt).unwrap();
        assert!(spool.write_json(&mut Vec::new()).is_err());
    }
}
