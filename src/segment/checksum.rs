//! Full index CRC with bounded exact reads; source/WAL checksum code is unchanged.
use crate::{Error, Result};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

#[cold]
#[inline(never)]
pub(super) fn check(file: &mut File, end: u64, expected: u32) -> Result<()> {
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0; end.min(1024 * 1024) as usize];
    let mut remaining = end;
    let mut state = !0;
    while remaining > 0 {
        crate::engine::maintenance::check()?;
        let n = remaining.min(buffer.len() as u64) as usize;
        {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("checksum.index_read");
            file.read_exact(&mut buffer[..n]).map_err(|e| {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    Error::Corrupt("truncated segment checksum prefix".into())
                } else {
                    e.into()
                }
            })?;
        }
        {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("checksum.index_compute");
            state = crate::format::crc_extend(state, &buffer[..n]);
        }
        remaining -= n as u64;
    }
    if !state != expected {
        return Err(Error::Corrupt("segment checksum mismatch".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_crc_reads_preserve_prefix_position_truncation_and_corruption() {
        struct Temp(std::path::PathBuf);
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp = Temp(
            std::env::temp_dir().join(format!("fdb-index-crc-{}-{nonce}", std::process::id())),
        );
        let bytes: Vec<u8> = (0..1024 * 1024 + 67)
            .map(|n| ((n * 71) % 256) as u8)
            .collect();
        std::fs::write(&temp.0, &bytes).unwrap();
        let mut file = File::open(&temp.0).unwrap();
        for end in [0, 1, 127, 65537, 1024 * 1024, bytes.len()] {
            check(&mut file, end as u64, crc32fast::hash(&bytes[..end])).unwrap();
            assert_eq!(file.stream_position().unwrap(), end as u64);
            assert!(matches!(
                check(&mut file, end as u64, crc32fast::hash(&bytes[..end]) ^ 1),
                Err(Error::Corrupt(_))
            ));
        }
        assert!(matches!(
            check(&mut file, bytes.len() as u64 + 1, 0),
            Err(Error::Corrupt(_))
        ));
        assert_eq!(std::fs::read(&temp.0).unwrap(), bytes);
    }
}
