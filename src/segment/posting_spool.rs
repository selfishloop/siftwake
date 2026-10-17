//! Checked temporary dictionary entries; drains to JSON or SEG05.
use super::*;
use std::io::{BufReader, Read};
pub(super) struct Postings {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    count: usize,
}
impl Postings {
    pub fn new(stem: &Path) -> Result<Self> {
        let path = stem.with_extension(format!(
            "{}-postings",
            stem.extension().unwrap().to_string_lossy()
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self {
            path,
            writer: Some(BufWriter::with_capacity(8 * 1024, file)),
            count: 0,
        })
    }
    pub fn push(&mut self, term: Term, posting: Posting) -> Result<()> {
        let bytes = serde_json::to_vec(&(term, posting))?;
        if bytes.len() > MAX_DIRECTORY as usize {
            return Err(Error::Invalid("posting directory entry limit".into()));
        }
        let writer = self.writer.as_mut().unwrap();
        writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
        writer.write_all(&crc(&bytes).to_le_bytes())?;
        writer.write_all(&bytes)?;
        self.count += 1;
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
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn write_binary(&mut self, output: &mut impl Write) -> Result<()> {
        self.drain(|_, bytes| {
            let (term, posting) = serde_json::from_slice(bytes)?;
            super::compact::write_posting(output, &term, posting)
        })
    }
    fn drain(&mut self, mut accept: impl FnMut(usize, &[u8]) -> Result<()>) -> Result<()> {
        let mut writer = self.writer.take().unwrap();
        writer.flush()?;
        let mut file = writer.into_inner().map_err(|e| Error::Io(e.into_error()))?;
        file.seek(SeekFrom::Start(0))?;
        let mut reader = BufReader::with_capacity(8 * 1024, file);
        let mut bytes = Vec::new();
        for n in 0..self.count {
            let mut header = [0; 8];
            reader.read_exact(&mut header)?;
            let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
            if len == 0 || len > MAX_DIRECTORY as usize {
                return Err(Error::Corrupt("posting spool bounds".into()));
            }
            bytes.resize(len, 0);
            reader.read_exact(&mut bytes)?;
            if crc(&bytes) != u32::from_le_bytes(header[4..].try_into().unwrap()) {
                return Err(Error::Corrupt("posting spool checksum".into()));
            }
            accept(n, &bytes)?;
        }
        if reader.read(&mut [0])? != 0 {
            return Err(Error::Corrupt("posting spool trailing data".into()));
        }
        Ok(())
    }
}
impl Drop for Postings {
    fn drop(&mut self) {
        self.writer.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_directory_bytes_and_torn_or_corrupt_spool_fail_closed() {
        let stem = std::env::temp_dir().join(format!(
            "fdb-postings-spool-{}-{}.build",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let value = || {
            (
                Term::Exact(
                    "şirket".into(),
                    "東京".into(),
                    "field".into(),
                    "quote\"back\\🦀".into(),
                ),
                Posting {
                    offset: 28,
                    count: 1,
                },
            )
        };
        let mut spool = Postings::new(&stem).unwrap();
        let path = spool.path.clone();
        let (t, p) = value();
        spool.push(t, p).unwrap();
        spool.writer.as_mut().unwrap().flush().unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut json = Vec::new();
        spool.write_json(&mut json).unwrap();
        assert_eq!(json, serde_json::to_vec(&vec![value()]).unwrap());
        drop(spool);
        assert!(!path.exists());
        for cut in 0..original.len() {
            let mut spool = Postings::new(&stem).unwrap();
            let (t, p) = value();
            spool.push(t, p).unwrap();
            spool.writer.as_mut().unwrap().flush().unwrap();
            OpenOptions::new()
                .write(true)
                .open(&spool.path)
                .unwrap()
                .set_len(cut as u64)
                .unwrap();
            assert!(spool.write_json(&mut Vec::new()).is_err());
        }
        let mut spool = Postings::new(&stem).unwrap();
        let (t, p) = value();
        spool.push(t, p).unwrap();
        spool.writer.as_mut().unwrap().flush().unwrap();
        let mut bad = original;
        bad[13] ^= 1;
        std::fs::write(&spool.path, bad).unwrap();
        assert!(spool.write_json(&mut Vec::new()).is_err());
    }
}
