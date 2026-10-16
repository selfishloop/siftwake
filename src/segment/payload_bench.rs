//! Explicit benchmark-only overlay. Original sources and SEG03 remain intact.
use super::*;
use crate::rkyv_payload::{
    bench::{Codec, Reader, verify_crc},
    binary,
};
use std::sync::Mutex;

pub(super) struct Overlay(pub Mutex<Reader>);
impl DiskIndex {
    pub(crate) fn attach_payload_benchmark(&mut self, path: &Path, codec: Codec) -> Result<u64> {
        if self.payload_benchmark.is_some() || codec == Codec::Rkyv {
            return Err(Error::Invalid(
                "benchmark overlay already attached or unsupported codec".into(),
            ));
        }
        #[cfg(feature = "experimental-paged-metadata")]
        if self.paged.is_some() {
            return Err(Error::Invalid(
                "payload benchmark requires unpacked metadata".into(),
            ));
        }
        let mut reader = Reader::open(path, codec)?;
        if reader.len() != self.len() {
            return Err(Error::Corrupt("overlay/segment count mismatch".into()));
        }
        // Do not accept a cache just because its identities/CRC are plausible.
        // Compare every full document against the already validated source index.
        for id in 0..self.len() as u32 {
            if self.deleted(id) || reader.document(id)? != self.read_document(id)? {
                return Err(Error::Corrupt("overlay/segment document mismatch".into()));
            }
        }
        let bytes = reader.disk_bytes;
        self.payload_benchmark = Some(Overlay(Mutex::new(reader)));
        Ok(bytes)
    }
    fn benchmark_reader(&self) -> Result<Option<std::sync::MutexGuard<'_, Reader>>> {
        self.payload_benchmark
            .as_ref()
            .map(|o| {
                o.0.lock()
                    .map_err(|_| Error::Corrupt("overlay lock poisoned".into()))
            })
            .transpose()
    }
    pub(super) fn benchmark_matches(
        &self,
        id: u32,
        scratch: &mut QueryTextScratch,
        terms: &[String],
        cache: &mut BTreeMap<String, String>,
        cached_bytes: &mut usize,
    ) -> Result<Option<bool>> {
        let Some(mut reader) = self.benchmark_reader()? else {
            return Ok(None);
        };
        let checksum = reader.checksum(id)?;
        let codec = reader.codec;
        #[cfg(feature = "diagnostic-timing")]
        let io = crate::timing::span("overlay.io");
        let bytes = reader.positional_bytes(id)?;
        #[cfg(feature = "diagnostic-timing")]
        drop(io);
        {
            #[cfg(feature = "diagnostic-timing")]
            let _crc = crate::timing::span("overlay.crc");
            verify_crc(bytes, checksum)?;
        }
        let r = self.metadata(id)?;
        match codec {
            Codec::Json => {
                #[cfg(feature = "diagnostic-timing")]
                let validation = crate::timing::span("overlay.validation");
                let d: StoredDocument = serde_json::from_slice(bytes)?;
                if (
                    d.document.key.scope.as_str(),
                    d.document.key.collection.as_str(),
                    d.document.key.id.as_str(),
                ) != self.key_parts(id)
                    || d.version != r.version
                {
                    return Err(Error::Corrupt("overlay identity mismatch".into()));
                }
                #[cfg(feature = "diagnostic-timing")]
                drop(validation);
                Ok(Some(scratch.matches(
                    &d.document,
                    terms,
                    cache,
                    cached_bytes,
                )))
            }
            Codec::Binary => {
                #[cfg(feature = "diagnostic-timing")]
                let validation = crate::timing::span("overlay.validation");
                let d = binary::checked(bytes)?;
                if d.key_parts() != self.key_parts(id) || d.version() != r.version {
                    return Err(Error::Corrupt("overlay identity mismatch".into()));
                }
                #[cfg(feature = "diagnostic-timing")]
                drop(validation);
                Ok(Some(scratch.matches_text(
                    d.title(),
                    d.text(),
                    terms,
                    cache,
                    cached_bytes,
                )))
            }
            Codec::Rkyv => Err(Error::Invalid("unsupported overlay codec".into())),
        }
    }
    pub(super) fn benchmark_document(&self, id: u32) -> Result<Option<StoredDocument>> {
        let Some(mut reader) = self.benchmark_reader()? else {
            return Ok(None);
        };
        let d = reader.document(id)?;
        if d.document.key != self.key(id)? || d.version != self.metadata(id)?.version {
            return Err(Error::Corrupt("overlay identity mismatch".into()));
        }
        Ok(Some(d))
    }
}
