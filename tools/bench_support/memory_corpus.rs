//! Bounded streaming input shared by memory probes; no books/oracle resident.
use serde_json::json;
use siftwake::StoredDocument;
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn phase(name: &str) {
    println!("{}", json!({"phase":name}));
}
pub fn crc(bytes: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut v = i as u32;
        for _ in 0..8 {
            v = (v >> 1) ^ (0xedb88320u32 & 0u32.wrapping_sub(v & 1));
        }
        *slot = v;
    }
    !bytes.iter().fold(!0u32, |v, b| {
        table[((v ^ u32::from(*b)) & 255) as usize] ^ (v >> 8)
    })
}

pub struct Input {
    file: BufReader<File>,
    next: usize,
    bytes: Vec<u8>,
    pub text_bytes: u64,
}
impl Input {
    pub fn open() -> Result<Self> {
        let mut file = BufReader::with_capacity(
            65536,
            File::open(Path::new("data/rkyv-corpus-20261009/json.payload-bench"))?,
        );
        let mut h = [0u8; 20];
        file.read_exact(&mut h)?;
        assert_eq!(&h[..8], b"PLOADB01");
        assert_eq!(u32::from_le_bytes(h[8..12].try_into()?), 1);
        assert_eq!(u32::from_le_bytes(h[12..16].try_into()?), 100000);
        assert_eq!(crc(&h[..16]), u32::from_le_bytes(h[16..].try_into()?));
        Ok(Self {
            file,
            next: 0,
            bytes: Vec::new(),
            text_bytes: 0,
        })
    }
    pub fn read(&mut self) -> Result<StoredDocument> {
        let mut h = [0u8; 16];
        self.file.read_exact(&mut h)?;
        assert_eq!(crc(&h[..12]), u32::from_le_bytes(h[12..].try_into()?));
        assert_eq!(u32::from_le_bytes(h[8..12].try_into()?) as usize, self.next);
        let length = u32::from_le_bytes(h[..4].try_into()?) as usize;
        assert!(length <= 4 * 1024 * 1024);
        self.bytes.resize(length, 0);
        self.file.read_exact(&mut self.bytes)?;
        assert_eq!(crc(&self.bytes), u32::from_le_bytes(h[4..8].try_into()?));
        let stored: StoredDocument = serde_json::from_slice(&self.bytes)?;
        assert_eq!(stored.document.key.id, format!("{:010}", self.next));
        assert_eq!(stored.version, self.next as u64 / 500 + 1);
        self.text_bytes += stored.document.text.len() as u64;
        self.next += 1;
        Ok(stored)
    }
    pub fn skip(&mut self, count: usize) -> Result<()> {
        for _ in 0..count {
            let _ = self.read()?;
        }
        self.text_bytes = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn corpus_crc_uses_standard_ieee_vector() {
        assert_eq!(super::crc(b"123456789"), 0xcbf43926);
        assert_eq!(super::crc(b""), 0);
        assert_ne!(super::crc(b"123456788"), 0xcbf43926);
    }
}
