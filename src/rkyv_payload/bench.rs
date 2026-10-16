//! Disposable benchmark transport, NOT an FDB source/cache/segment format.
//! Never called by Database. Both codecs share framing, CRC, I/O and identity.
use super::{checked, failure};
use crate::{Result, StoredDocument, format::crc};
use rkyv::util::AlignedVec;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"PLOADB01";
const MAX: usize = 4 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Json,
    Rkyv,
    Binary,
}
impl Codec {
    pub fn name(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Rkyv => "rkyv",
            Self::Binary => "binary",
        }
    }
    fn tag(self) -> u32 {
        match self {
            Self::Json => 1,
            Self::Rkyv => 2,
            Self::Binary => 3,
        }
    }
}
pub struct Writer {
    file: BufWriter<File>,
    codec: Codec,
    count: u32,
    written: u32,
}
impl Writer {
    pub fn create(path: &Path, codec: Codec, count: u32) -> Result<Self> {
        let mut file = BufWriter::with_capacity(
            1024 * 1024,
            OpenOptions::new().write(true).create_new(true).open(path)?,
        );
        let mut header = [0; 20];
        header[..8].copy_from_slice(MAGIC);
        header[8..12].copy_from_slice(&codec.tag().to_le_bytes());
        header[12..16].copy_from_slice(&count.to_le_bytes());
        let checksum = crc(&header[..16]);
        header[16..].copy_from_slice(&checksum.to_le_bytes());
        file.write_all(&header)?;
        Ok(Self {
            file,
            codec,
            count,
            written: 0,
        })
    }
    pub fn append(&mut self, stored: &StoredDocument) -> Result<()> {
        if self.written >= self.count {
            return Err(failure("too many benchmark records"));
        }
        let json;
        let binary;
        let offsets;
        let bytes: &[u8] = match self.codec {
            Codec::Json => {
                json = serde_json::to_vec(stored)?;
                &json
            }
            Codec::Rkyv => {
                binary = super::encode(stored)?;
                &binary
            }
            Codec::Binary => {
                offsets = super::binary::encode(stored)?;
                &offsets
            }
        };
        if bytes.len() > MAX {
            return Err(failure("benchmark payload too large"));
        }
        let mut header = [0; 16];
        header[..4].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        header[4..8].copy_from_slice(&crc(bytes).to_le_bytes());
        header[8..12].copy_from_slice(&self.written.to_le_bytes());
        let checksum = crc(&header[..12]);
        header[12..].copy_from_slice(&checksum.to_le_bytes());
        self.file.write_all(&header)?;
        self.file.write_all(bytes)?;
        self.written += 1;
        Ok(())
    }
    pub fn sync(&mut self) -> Result<()> {
        self.file.flush()?;
        self.file.get_ref().sync_all()?;
        Ok(())
    }
    pub fn finish(mut self) -> Result<()> {
        if self.written != self.count {
            return Err(failure("unfinished benchmark export"));
        }
        self.sync()
    }
}
#[derive(Clone, Copy)]
struct Entry {
    offset: u64,
    len: u32,
    checksum: u32,
}
pub struct Reader {
    file: BufReader<File>,
    position: u64,
    entries: Vec<Entry>,
    bytes: AlignedVec,
    pub codec: Codec,
    pub disk_bytes: u64,
}
impl Reader {
    pub fn open(path: &Path, codec: Codec) -> Result<Self> {
        let mut file = BufReader::with_capacity(1024 * 1024, File::open(path)?);
        let disk_bytes = file.get_ref().metadata()?.len();
        let mut header = [0; 20];
        file.read_exact(&mut header)?;
        if &header[..8] != MAGIC
            || u32::from_le_bytes(header[8..12].try_into().unwrap()) != codec.tag()
            || crc(&header[..16]) != u32::from_le_bytes(header[16..].try_into().unwrap())
        {
            return Err(failure("benchmark header"));
        }
        let count = u32::from_le_bytes(header[12..16].try_into().unwrap());
        if count as u64 > disk_bytes.saturating_sub(20) / 16 {
            return Err(failure("benchmark record count bounds"));
        }
        let mut entries = Vec::new();
        let mut bytes = AlignedVec::new();
        for id in 0..count {
            let mut header = [0; 16];
            file.read_exact(&mut header)?;
            if crc(&header[..12]) != u32::from_le_bytes(header[12..].try_into().unwrap())
                || u32::from_le_bytes(header[8..12].try_into().unwrap()) != id
            {
                return Err(failure("benchmark record header"));
            }
            let len = u32::from_le_bytes(header[..4].try_into().unwrap());
            let checksum = u32::from_le_bytes(header[4..8].try_into().unwrap());
            let offset = file.stream_position()?;
            if len == 0
                || len as usize > MAX
                || offset
                    .checked_add(len as u64)
                    .is_none_or(|end| end > disk_bytes)
            {
                return Err(failure("benchmark length bounds"));
            }
            bytes.resize(len as usize, 0);
            file.read_exact(&mut bytes)?;
            if crc(&bytes) != checksum {
                return Err(failure("benchmark payload CRC"));
            }
            match codec {
                Codec::Json => {
                    let d: StoredDocument = serde_json::from_slice(&bytes)?;
                    identity(
                        (
                            &d.document.key.scope,
                            &d.document.key.collection,
                            &d.document.key.id,
                        ),
                        d.version,
                        id,
                    )?;
                }
                Codec::Rkyv => {
                    let d = checked(&bytes)?;
                    identity(d.key_parts(), d.version(), id)?;
                }
                Codec::Binary => {
                    let d = super::binary::checked(&bytes)?;
                    identity(d.key_parts(), d.version(), id)?;
                }
            }
            entries.push(Entry {
                offset,
                len,
                checksum,
            });
        }
        if file.stream_position()? != disk_bytes {
            return Err(failure("benchmark trailing bytes"));
        }
        Ok(Self {
            file,
            position: disk_bytes,
            entries,
            bytes,
            codec,
            disk_bytes,
        })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    // CRC is separate from structural bytecheck/JSON parsing for attribution.
    pub fn bytes(&mut self, id: u32) -> Result<&[u8]> {
        let e = *self
            .entries
            .get(id as usize)
            .ok_or_else(|| failure("benchmark slot"))?;
        let delta = i64::try_from(e.offset as i128 - self.position as i128)
            .map_err(|_| failure("benchmark seek range"))?;
        self.file.seek_relative(delta)?;
        self.bytes.resize(e.len as usize, 0);
        self.file.read_exact(&mut self.bytes)?;
        self.position = e.offset + e.len as u64;
        Ok(&self.bytes)
    }
    pub fn checksum(&self, id: u32) -> Result<u32> {
        Ok(self
            .entries
            .get(id as usize)
            .ok_or_else(|| failure("benchmark slot"))?
            .checksum)
    }
    pub fn document(&mut self, id: u32) -> Result<StoredDocument> {
        let checksum = self.checksum(id)?;
        let codec = self.codec;
        let bytes = self.positional_bytes(id)?;
        verify_crc(bytes, checksum)?;
        match codec {
            Codec::Json => Ok(serde_json::from_slice(bytes)?),
            Codec::Rkyv => super::checked(bytes)?.to_document(),
            Codec::Binary => super::binary::checked(bytes)?.to_document(),
        }
    }
    // Engine overlay uses the same positional I/O as SEG03. Do not serve old
    // BufReader-prefetched bytes after the backing file has been corrupted.
    pub(crate) fn positional_bytes(&mut self, id: u32) -> Result<&[u8]> {
        let e = *self
            .entries
            .get(id as usize)
            .ok_or_else(|| failure("benchmark slot"))?;
        self.bytes.resize(e.len as usize, 0);
        crate::segment::read_at(self.file.get_ref(), e.offset, &mut self.bytes)?;
        Ok(&self.bytes)
    }
}
pub fn verify_crc(bytes: &[u8], checksum: u32) -> Result<()> {
    if crc(bytes) != checksum {
        return Err(failure("benchmark payload CRC"));
    }
    Ok(())
}
pub fn identity(key: (&str, &str, &str), version: u64, id: u32) -> Result<()> {
    if key != ("shared", "texts", format!("{id:010}").as_str()) || version != id as u64 / 500 + 1 {
        return Err(failure("benchmark key/version identity"));
    }
    Ok(())
}
