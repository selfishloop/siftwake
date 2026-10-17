//! Full native checkpoint validation in one bounded pass, without owned documents.
use super::*;
use std::io::BufReader;

#[cold]
#[inline(never)]
pub(crate) fn check_native_prefix(
    file: &mut File,
    end: u64,
    expected: u32,
) -> Result<(u64, usize)> {
    #[cfg(feature = "diagnostic-timing")]
    let _span = crate::timing::span("open.source_native_check");
    file.seek(SeekFrom::Start(0))?;
    if end < 8 || file.metadata()?.len() < end {
        return Err(Error::Corrupt("truncated checkpoint".into()));
    }
    let mut reader = BufReader::with_capacity(64 * 1024, file.take(end));
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC_V4 {
        return Err(Error::Corrupt("native validation requires FDBLOG04".into()));
    }
    let mut source_crc = crc(&magic);
    let mut cursor = 8u64;
    let mut sequence = None;
    let mut records = 0usize;
    let mut scratch = Vec::new();
    while cursor < end {
        if end - cursor < HEADER as u64 {
            return Err(Error::Corrupt("short checkpoint header".into()));
        }
        let mut header = [0; HEADER];
        reader.read_exact(&mut header)?;
        if &header[..4] != b"TXN2"
            || crc(&header[..20]) != u32::from_le_bytes(header[20..24].try_into().unwrap())
        {
            return Err(Error::Corrupt("checkpoint header checksum".into()));
        }
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let seq = u64::from_le_bytes(header[8..16].try_into().unwrap());
        if !(5..=MAX_FRAME).contains(&len)
            || (HEADER + len + 8) as u64 > end - cursor
            || sequence.is_some_and(|previous| previous != seq)
        {
            return Err(Error::Corrupt("checkpoint frame bounds/sequence".into()));
        }
        let mut head = [0; 5];
        reader.read_exact(&mut head)?;
        let count = u32::from_le_bytes(head[1..5].try_into().unwrap()) as usize;
        if head[0] != 1 || count > (len - 5) / 17 {
            return Err(Error::Corrupt("binary checkpoint flag/count".into()));
        }
        let mut state = crc_extend(!0, &head);
        let mut remaining = len - 5;
        for _ in 0..count {
            if remaining < 5 {
                return Err(Error::Corrupt("binary checkpoint entry header".into()));
            }
            let mut entry = [0; 5];
            reader.read_exact(&mut entry)?;
            remaining -= 5;
            let n = u32::from_le_bytes(entry[1..5].try_into().unwrap()) as usize;
            let limit = match entry[0] {
                0 => 4 * 1024 * 1024,
                1 => 3 * (4 + 1024),
                _ => return Err(Error::Corrupt("binary checkpoint entry tag".into())),
            };
            if n > remaining || n > limit {
                return Err(Error::Corrupt("binary checkpoint entry bounds".into()));
            }
            if scratch.capacity() > 256 * 1024 {
                scratch = Vec::new();
            }
            scratch.resize(n, 0);
            reader.read_exact(&mut scratch)?;
            state = crc_extend(state, &entry);
            state = crc_extend(state, &scratch);
            if entry[0] == 0 {
                let view = crate::payload::checked(&scratch)?;
                view.validate_document(n)
                    .map_err(|e| Error::Corrupt(e.to_string()))?;
                if view.version() == 0 || view.version() > seq {
                    return Err(Error::Corrupt("invalid checkpoint version".into()));
                }
            } else if let Change::Delete { key } = crate::binary_batch::decode_entry(1, &scratch)? {
                crate::validate_key(&key).map_err(|e| Error::Corrupt(e.to_string()))?;
            }
            records = records
                .checked_add(1)
                .ok_or_else(|| Error::Corrupt("source record count overflow".into()))?;
            remaining -= n;
        }
        if remaining != 0 {
            return Err(Error::Corrupt("binary checkpoint trailing bytes".into()));
        }
        let mut footer = [0; 8];
        reader.read_exact(&mut footer)?;
        let payload_crc = !state;
        if &footer != FOOTER
            || payload_crc != u32::from_le_bytes(header[16..20].try_into().unwrap())
        {
            return Err(Error::Corrupt("checkpoint payload checksum/footer".into()));
        }
        source_crc = !crc_extend(!source_crc, &header);
        source_crc = crc_combine(source_crc, payload_crc, len as u64);
        source_crc = !crc_extend(!source_crc, &footer);
        cursor += (HEADER + len + 8) as u64;
        sequence = Some(seq);
    }
    if source_crc != expected {
        return Err(Error::Corrupt("source checkpoint checksum mismatch".into()));
    }
    Ok((
        sequence.ok_or_else(|| Error::Corrupt("missing checkpoint".into()))?,
        records,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn document() -> StoredDocument {
        StoredDocument {
            version: 7,
            document: crate::Document {
                key: crate::Key::new("şirket", "東京", "a"),
                title: "🦀 quoted \" title".into(),
                text: "İstanbul\nbody".into(),
                fields: std::collections::BTreeMap::from([(
                    "nested".into(),
                    serde_json::json!([null,true,i64::MIN,u64::MAX,-0.0,{"s":"東京"}]),
                )]),
                links: vec![crate::Key::new("şirket", "東京", "target")],
            },
        }
    }
    fn check(bytes: &[u8], expected: u32) -> Result<(u64, usize)> {
        let temp = Temp(std::env::temp_dir().join(format!(
            "fdb-native-check-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        )));
        std::fs::write(&temp.0, bytes)?;
        let mut file = File::open(&temp.0)?;
        check_native_prefix(&mut file, bytes.len() as u64, expected)
    }
    fn source(changes: Vec<Change>) -> Vec<u8> {
        let mut bytes = MAGIC_V4.to_vec();
        bytes.extend(
            encode(
                7,
                &Batch {
                    checkpoint: true,
                    changes,
                },
            )
            .unwrap(),
        );
        bytes
    }
    #[test]
    fn native_pass_checks_every_cut_byte_and_independent_source_crc() {
        let mut bytes = source(vec![Change::Put { stored: document() }]);
        bytes.extend(
            encode(
                7,
                &Batch {
                    checkpoint: true,
                    changes: vec![],
                },
            )
            .unwrap(),
        );
        bytes.extend(
            encode(
                7,
                &Batch {
                    checkpoint: true,
                    changes: vec![Change::Delete {
                        key: crate::Key::new("şirket", "東京", "b"),
                    }],
                },
            )
            .unwrap(),
        );
        assert_eq!(check(&bytes, crc(&bytes)).unwrap(), (7, 2));
        for cut in 0..bytes.len() {
            assert!(check(&bytes[..cut], crc(&bytes)).is_err(), "cut {cut}");
        }
        for at in 0..bytes.len() {
            let mut changed = bytes.clone();
            changed[at] ^= 1;
            assert!(check(&changed, crc(&changed)).is_err(), "byte {at}");
        }
        let mut changed = bytes.clone();
        let at = changed.windows(4).position(|s| s == b"body").unwrap();
        changed[at] = b'B';
        let len = u32::from_le_bytes(changed[12..16].try_into().unwrap()) as usize;
        let checksum = crc(&changed[32..32 + len]);
        changed[24..28].copy_from_slice(&checksum.to_le_bytes());
        let checksum = crc(&changed[8..28]);
        changed[28..32].copy_from_slice(&checksum.to_le_bytes());
        assert!(
            matches!(check(&changed, crc(&bytes)), Err(Error::Corrupt(message)) if message == "source checkpoint checksum mismatch")
        );
        assert_eq!(check(&changed, crc(&changed)).unwrap(), (7, 2));
    }
    #[test]
    fn valid_crcs_do_not_hide_invalid_schema_limits_keys_links_or_versions() {
        let valid = document();
        let mut cases = vec![valid.clone()];
        for version in [0, 8] {
            let mut doc = valid.clone();
            doc.version = version;
            cases.push(doc);
        }
        let mut doc = valid.clone();
        doc.document.key.id = "bad\n".into();
        cases.push(doc);
        let mut doc = valid.clone();
        doc.document
            .fields
            .insert("".into(), serde_json::Value::Null);
        cases.push(doc);
        let mut doc = valid.clone();
        doc.document
            .links
            .push(crate::Key::new("other", "東京", "target"));
        cases.push(doc);
        let mut doc = valid;
        doc.document.text = "x".repeat(2 * 1024 * 1024 + 1);
        cases.push(doc);
        for doc in cases {
            let expected =
                crate::validate_doc(&doc.document).is_ok() && doc.version > 0 && doc.version <= 7;
            let bytes = source(vec![Change::Put { stored: doc }]);
            assert_eq!(check(&bytes, crc(&bytes)).is_ok(), expected);
        }
    }
}
