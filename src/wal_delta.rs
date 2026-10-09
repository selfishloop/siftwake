//! Volatile references to validated documents in the active transaction log.
//! No new persistent representation; every read checks the original JSON bytes.
use crate::{Error, Key, Result, StoredDocument, format};
use std::fs::File;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Location {
    pub offset: u64,
    pub len: u32,
    pub checksum: u32,
}

pub(crate) struct Row {
    pub location: Location,
    pub version: u64,
}
impl Row {
    pub fn read(&self, file: &File, key: &Key) -> Result<StoredDocument> {
        if self.location.len as usize > format::MAX_FRAME {
            return Err(Error::Corrupt("delta document bounds".into()));
        }
        let mut bytes = vec![0; self.location.len as usize];
        crate::segment::read_at(file, self.location.offset, &mut bytes)?;
        if format::crc(&bytes) != self.location.checksum {
            return Err(Error::Corrupt("delta document checksum mismatch".into()));
        }
        let stored: StoredDocument =
            serde_json::from_slice(&bytes).map_err(|error| Error::Corrupt(error.to_string()))?;
        if &stored.document.key != key || stored.version != self.version {
            return Err(Error::Corrupt("delta document identity mismatch".into()));
        }
        Ok(stored)
    }
}

/// Called only after the full frame/schema/checksum/sequence validation.
pub(crate) fn locations(
    payload: &[u8],
    start: u64,
    batch: &format::Batch,
) -> Result<Vec<Option<Location>>> {
    use serde::Deserialize;
    use serde_json::value::RawValue;
    #[derive(Deserialize)]
    struct RawBatch<'a> {
        #[serde(borrow)]
        changes: Vec<&'a RawValue>,
    }
    #[derive(Deserialize)]
    struct RawPut<'a> {
        #[serde(borrow)]
        stored: &'a RawValue,
    }
    let raw: RawBatch<'_> =
        serde_json::from_slice(payload).map_err(|error| Error::Corrupt(error.to_string()))?;
    if raw.changes.len() != batch.changes.len() {
        return Err(Error::Corrupt("delta change count mismatch".into()));
    }
    raw.changes
        .iter()
        .zip(&batch.changes)
        .map(|(raw, change)| {
            if matches!(change, format::Change::Delete { .. }) {
                return Ok(None);
            }
            let put: RawPut<'_> = serde_json::from_str(raw.get())
                .map_err(|error| Error::Corrupt(error.to_string()))?;
            let bytes = put.stored.get().as_bytes();
            let offset = (bytes.as_ptr() as usize)
                .checked_sub(payload.as_ptr() as usize)
                .ok_or_else(|| Error::Corrupt("delta location underflow".into()))?;
            if offset
                .checked_add(bytes.len())
                .is_none_or(|end| end > payload.len())
            {
                return Err(Error::Corrupt("delta location bounds".into()));
            }
            Ok(Some(Location {
                offset: start
                    .checked_add(offset as u64)
                    .ok_or_else(|| Error::Corrupt("delta location overflow".into()))?,
                len: bytes.len() as u32,
                checksum: format::crc(bytes),
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Document,
        format::{Batch, Change},
    };

    fn batch(checkpoint: bool) -> Batch {
        Batch {
            checkpoint,
            changes: vec![
                Change::Put {
                    stored: StoredDocument {
                        document: Document {
                            key: Key::new("user:ş", "çizelge", "\\\"ü"),
                            title: "\"başlık\n\\".into(),
                            text: "ŞİRKET 🦀".into(),
                            fields: [("json".into(), serde_json::json!({"nested":[1,"ü",null]}))]
                                .into(),
                            links: vec![Key::new("user:ş", "çizelge", "2")],
                        },
                        version: 7,
                    },
                },
                Change::Delete {
                    key: Key::new("shared", "c", "deleted"),
                },
            ],
        }
    }

    #[test]
    fn located_encoder_matches_existing_bytes_and_borrowed_replay_offsets() {
        for checkpoint in [false, true] {
            let batch = batch(checkpoint);
            let (bytes, written) = format::encode_located(7, &batch).unwrap();
            assert_eq!(bytes, format::encode(7, &batch).unwrap());
            let recovered = locations(&bytes[24..bytes.len() - 8], 24, &batch).unwrap();
            for (a, b) in written.iter().zip(recovered) {
                assert_eq!(
                    a.map(|x| (x.offset, x.len, x.checksum)),
                    b.map(|x| (x.offset, x.len, x.checksum))
                );
            }
        }
    }

    #[test]
    fn raw_offsets_accept_whitespace_and_reordered_existing_schema() {
        let batch = batch(false);
        let stored = match &batch.changes[0] {
            Change::Put { stored } => stored,
            _ => unreachable!(),
        };
        let payload = format!(
            " {{ \"changes\": [ {{\"stored\": {}, \"kind\":\"put\"}} ], \"checkpoint\":false }} ",
            serde_json::to_string_pretty(stored).unwrap()
        );
        let batch: Batch = serde_json::from_str(&payload).unwrap();
        let loc = locations(payload.as_bytes(), 99, &batch).unwrap()[0].unwrap();
        let start = (loc.offset - 99) as usize;
        let bytes = &payload.as_bytes()[start..start + loc.len as usize];
        assert_eq!(format::crc(bytes), loc.checksum);
        assert_eq!(
            serde_json::from_slice::<StoredDocument>(bytes).unwrap(),
            *stored
        );
    }

    #[test]
    fn valid_crc_still_requires_identity_and_complete_bytes() {
        let batch = batch(false);
        let stored = match &batch.changes[0] {
            Change::Put { stored } => stored,
            _ => unreachable!(),
        };
        let bytes = serde_json::to_vec(stored).unwrap();
        let path = std::env::temp_dir().join(format!("fdb-wal-row-{}.tmp", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut &file, &bytes).unwrap();
        let mut row = Row {
            version: stored.version,
            location: Location {
                offset: 0,
                len: bytes.len() as u32,
                checksum: format::crc(&bytes),
            },
        };
        assert_eq!(row.read(&file, &stored.document.key).unwrap(), *stored);
        assert!(matches!(
            row.read(&file, &Key::new("shared", "other", "x")),
            Err(Error::Corrupt(_))
        ));
        row.version += 1;
        assert!(matches!(
            row.read(&file, &stored.document.key),
            Err(Error::Corrupt(_))
        ));
        row.version = stored.version;
        file.set_len(bytes.len() as u64 - 1).unwrap();
        assert!(matches!(
            row.read(&file, &stored.document.key),
            Err(Error::Corrupt(_))
        ));
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
}
