//! FDB v1: file magic, then independently checksummed atomic transaction frames.
use crate::model::StoredDocument;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

pub(crate) const MAGIC: &[u8; 8] = b"FDBLOG01";
pub(crate) const MAGIC_V3: &[u8; 8] = b"FDBLOG03";
pub(crate) const MAX_FRAME: usize = 64 * 1024 * 1024;
const HEADER: usize = 24;
const FOOTER: &[u8; 8] = b"FDBDONE1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Batch {
    pub checkpoint: bool,
    pub changes: Vec<Change>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Change {
    Put { stored: StoredDocument },
    Delete { key: crate::model::Key },
}

// CRC32 IEEE; framing and integrity are implemented here, not delegated to a DB.
const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut n = 0;
    while n < 256 {
        let mut value = n as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(value & 1));
            bit += 1;
        }
        table[n] = value;
        n += 1;
    }
    table
}
const CRC_TABLE: [u32; 256] = crc_table();
const fn crc_slices() -> [[u32; 256]; 8] {
    let mut tables = [[0; 256]; 8];
    tables[0] = CRC_TABLE;
    let mut row = 1;
    while row < 8 {
        let mut n = 0;
        while n < 256 {
            let value = tables[row - 1][n];
            tables[row][n] = (value >> 8) ^ CRC_TABLE[(value & 255) as usize];
            n += 1;
        }
        row += 1;
    }
    tables
}
const CRC_SLICES: [[u32; 256]; 8] = crc_slices();

pub(crate) fn crc_extend(state: u32, bytes: &[u8]) -> u32 {
    let (chunks, tail) = bytes.as_chunks::<8>();
    let mut state = state;
    for chunk in chunks {
        let word = state ^ u32::from_le_bytes(chunk[..4].try_into().unwrap());
        state = CRC_SLICES[7][(word & 255) as usize]
            ^ CRC_SLICES[6][((word >> 8) & 255) as usize]
            ^ CRC_SLICES[5][((word >> 16) & 255) as usize]
            ^ CRC_SLICES[4][(word >> 24) as usize]
            ^ CRC_SLICES[3][chunk[4] as usize]
            ^ CRC_SLICES[2][chunk[5] as usize]
            ^ CRC_SLICES[1][chunk[6] as usize]
            ^ CRC_SLICES[0][chunk[7] as usize];
    }
    tail.iter().fold(state, |s, b| {
        (s >> 8) ^ CRC_TABLE[((s ^ *b as u32) & 255) as usize]
    })
}

pub(crate) fn crc(bytes: &[u8]) -> u32 {
    !crc_extend(!0, bytes)
}

pub(crate) fn prefix_crc(file: &mut File, end: u64) -> Result<u32> {
    file.seek(SeekFrom::Start(0))?;
    let mut input = file.take(end);
    let mut buffer = [0; 65536];
    let mut state = !0;
    let mut size = 0u64;
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        state = crc_extend(state, &buffer[..n]);
    }
    if size != end {
        return Err(Error::Corrupt("truncated checkpoint".into()));
    }
    Ok(!state)
}

pub(crate) fn encode(sequence: u64, batch: &Batch) -> Result<Vec<u8>> {
    #[cfg(feature = "diagnostic-memory")]
    let _origin = crate::memory::Scope::new(5);
    #[cfg(feature = "diagnostic-timing")]
    let _encode = crate::timing::span("frame.encode_json_crc");
    let mut bytes = vec![0; HEADER];
    let mut sink = crate::bounded::Sink::new(&mut bytes, MAX_FRAME);
    let encoded = serde_json::to_writer(&mut sink, batch);
    if sink.exceeded {
        return Err(Error::Invalid("transaction exceeds 64 MiB".into()));
    }
    if let Some(error) = sink.failure.take() {
        return Err(Error::Io(error));
    }
    encoded?;
    let len = sink.written;
    bytes[..4].copy_from_slice(b"TXN1");
    bytes[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    bytes[8..16].copy_from_slice(&sequence.to_le_bytes());
    let checksum = crc(&bytes[HEADER..]);
    bytes[16..20].copy_from_slice(&checksum.to_le_bytes());
    let checksum = crc(&bytes[..20]);
    bytes[20..24].copy_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(FOOTER);
    Ok(bytes)
}

pub(crate) fn append(file: &mut File, bytes: &[u8]) -> Result<()> {
    append_with_sync(file, bytes, true)
}

pub(crate) fn append_with_sync(file: &mut File, bytes: &[u8], sync: bool) -> Result<()> {
    file.seek(SeekFrom::End(0))?;
    #[cfg(feature = "diagnostic-timing")]
    let write = crate::timing::span("wal.write");
    file.write_all(bytes)?;
    #[cfg(feature = "diagnostic-timing")]
    drop(write);
    if sync {
        #[cfg(feature = "diagnostic-timing")]
        let _sync = crate::timing::span("wal.sync");
        file.sync_all()?;
    }
    Ok(())
}

#[cfg(feature = "experimental-wal-delta")]
pub(crate) fn encode_located(
    sequence: u64,
    batch: &Batch,
) -> Result<(Vec<u8>, Vec<Option<crate::wal_delta::Location>>)> {
    #[cfg(feature = "diagnostic-memory")]
    let _origin = crate::memory::Scope::new(5);
    let mut bytes = vec![0; HEADER];
    let mut locations = Vec::with_capacity(batch.changes.len());
    let mut sink = crate::bounded::Sink::new(&mut bytes, MAX_FRAME);
    let encoded = (|| -> Result<()> {
        sink.write_all(if batch.checkpoint {
            b"{\"checkpoint\":true,\"changes\":["
        } else {
            b"{\"checkpoint\":false,\"changes\":["
        })?;
        for (n, change) in batch.changes.iter().enumerate() {
            if n != 0 {
                sink.write_all(b",")?;
            }
            if let Change::Put { stored } = change {
                sink.write_all(b"{\"kind\":\"put\",\"stored\":")?;
                let start = sink.written;
                serde_json::to_writer(&mut sink, stored)?;
                locations.push(Some(crate::wal_delta::Location {
                    offset: (HEADER + start) as u64,
                    len: (sink.written - start) as u32,
                    checksum: 0,
                }));
                sink.write_all(b"}")?;
            } else {
                serde_json::to_writer(&mut sink, change)?;
                locations.push(None);
            }
        }
        sink.write_all(b"]}")?;
        Ok(())
    })();
    if sink.exceeded {
        return Err(Error::Invalid("transaction exceeds 64 MiB".into()));
    }
    if let Some(error) = sink.failure.take() {
        return Err(Error::Io(error));
    }
    encoded?;
    let len = sink.written;
    for location in locations.iter_mut().flatten() {
        let start = location.offset as usize;
        location.checksum = crc(&bytes[start..start + location.len as usize]);
    }
    bytes[..4].copy_from_slice(b"TXN1");
    bytes[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    bytes[8..16].copy_from_slice(&sequence.to_le_bytes());
    let checksum = crc(&bytes[HEADER..]);
    bytes[16..20].copy_from_slice(&checksum.to_le_bytes());
    let checksum = crc(&bytes[..20]);
    bytes[20..24].copy_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(FOOTER);
    Ok((bytes, locations))
}

#[cfg(feature = "experimental-wal-delta")]
pub(crate) fn replay_located(
    file: &mut File,
    checkpoint_end: u64,
    mut apply: impl FnMut(u64, Batch, Vec<Option<crate::wal_delta::Location>>) -> Result<()>,
) -> Result<u64> {
    replay_impl(
        file,
        checkpoint_end,
        None,
        &mut |seq, batch, payload, start| {
            let locations = crate::wal_delta::locations(payload, start, &batch)?;
            apply(seq, batch, locations)
        },
    )
}

/// Only unpublished immutable run sources; transaction WAL retains atomic encode.
#[cfg(feature = "experimental-write-buffer-budget")]
pub(crate) fn append_checkpoint_changes(
    file: &mut File,
    sequence: u64,
    changes: impl Iterator<Item = Result<Change>>,
) -> Result<()> {
    use std::io::BufWriter;
    struct Payload<'a> {
        file: &'a mut File,
        count: usize,
        state: u32,
    }
    impl Write for Payload<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_FRAME - self.count {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "checkpoint exceeds 64 MiB",
                ));
            }
            let n = self.file.write(bytes)?;
            self.state = crc_extend(self.state, &bytes[..n]);
            self.count += n;
            Ok(n)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.file.flush()
        }
    }
    let mut changes = changes.peekable();
    loop {
        // Acquire once before allocating, release before any builder reservation.
        let lease = crate::write_budget::reserve(64 * 1024, 8 * 1024)?;
        let start = file.seek(SeekFrom::End(0))?;
        file.write_all(&[0; HEADER])?;
        let (len, checksum) = {
            let mut writer = BufWriter::with_capacity(
                lease.bytes,
                Payload {
                    file,
                    count: 0,
                    state: !0,
                },
            );
            writer.write_all(b"{\"checkpoint\":true,\"changes\":[")?;
            let mut first = true;
            while let Some(next) = changes.peek() {
                if !first && let Ok(Change::Put { stored }) = next {
                    // Valid large JSON fields can exceed the 8 MiB target. Avoid
                    // combining them into a frame larger than the existing cap.
                    let fields = &stored.document.fields;
                    let preflight = fields.values().any(|v| v.is_array() || v.is_object())
                        || fields
                            .values()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::len)
                            .sum::<usize>()
                            > 4 * 1024 * 1024;
                    if preflight {
                        let mut counted = crate::bounded::Sink::new(std::io::sink(), MAX_FRAME);
                        let encoded = serde_json::to_writer(&mut counted, next.as_ref().unwrap());
                        if counted.exceeded {
                            return Err(Error::Invalid("checkpoint exceeds 64 MiB".into()));
                        }
                        encoded?;
                        if writer.get_ref().count + writer.buffer().len() + 1 + counted.written + 2
                            > MAX_FRAME
                        {
                            break;
                        }
                    }
                }
                if !first {
                    writer.write_all(b",")?;
                }
                first = false;
                let change = changes.next().unwrap()?;
                serde_json::to_writer(&mut writer, &change)?;
                drop(change);
                if writer.get_ref().count + writer.buffer().len() >= 8 * 1024 * 1024 {
                    break;
                }
            }
            writer.write_all(b"]}")?;
            writer.flush()?;
            (writer.get_ref().count, !writer.get_ref().state)
        };
        let end = file.stream_position()?;
        file.write_all(FOOTER)?;
        let mut header = [0u8; HEADER];
        header[..4].copy_from_slice(b"TXN1");
        header[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        header[8..16].copy_from_slice(&sequence.to_le_bytes());
        header[16..20].copy_from_slice(&checksum.to_le_bytes());
        let header_crc = crc(&header[..20]);
        header[20..24].copy_from_slice(&header_crc.to_le_bytes());
        file.seek(SeekFrom::Start(start))?;
        file.write_all(&header)?;
        file.seek(SeekFrom::Start(end + FOOTER.len() as u64))?;
        file.sync_all()?;
        drop(lease);
        if changes.peek().is_none() {
            break;
        }
    }
    Ok(())
}

#[cfg(any(not(feature = "experimental-wal-delta"), test))]
pub(crate) fn replay(
    file: &mut File,
    checkpoint_end: u64,
    mut apply: impl FnMut(u64, Batch) -> Result<()>,
) -> Result<u64> {
    replay_impl(file, checkpoint_end, None, &mut |seq, batch, _, _| {
        apply(seq, batch)
    })
}

#[cfg(not(feature = "experimental-streaming-open"))]
pub(crate) fn replay_prefix(
    file: &mut File,
    end: u64,
    mut apply: impl FnMut(u64, Batch) -> Result<()>,
) -> Result<u64> {
    replay_impl(file, end, Some(end), &mut |seq, batch, _, _| {
        apply(seq, batch)
    })
}

/// Immutable checkpoints only. CRC/footer are verified before emitting changes.
/// None starts a frame, including an empty checkpoint. Errors prevent publication.
#[cfg(feature = "experimental-streaming-open")]
pub(crate) fn replay_prefix_changes(
    file: &mut File,
    end: u64,
    mut emit: impl FnMut(u64, Option<Change>) -> Result<()>,
) -> Result<u64> {
    use std::io::BufReader;
    file.seek(SeekFrom::Start(0))?;
    if end < 8 || file.metadata()?.len() < end {
        return Err(Error::Corrupt("truncated checkpoint".into()));
    }
    let mut magic = [0; 8];
    file.read_exact(&mut magic)?;
    if &magic != MAGIC && &magic != MAGIC_V3 {
        return Err(Error::Corrupt("unsupported file format".into()));
    }
    let mut cursor = 8u64;
    let mut sequence = None;
    let mut buffer = [0; 64 * 1024];
    while cursor < end {
        if end - cursor < HEADER as u64 {
            return Err(Error::Corrupt("short checkpoint header".into()));
        }
        let mut header = [0; HEADER];
        file.read_exact(&mut header)?;
        if &header[..4] != b"TXN1"
            || crc(&header[..20]) != u32::from_le_bytes(header[20..24].try_into().unwrap())
        {
            return Err(Error::Corrupt("checkpoint header checksum".into()));
        }
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let seq = u64::from_le_bytes(header[8..16].try_into().unwrap());
        if len > MAX_FRAME
            || (HEADER + len + 8) as u64 > end - cursor
            || sequence.is_some_and(|previous| previous != seq)
        {
            return Err(Error::Corrupt("checkpoint frame bounds/sequence".into()));
        }
        let start = cursor + HEADER as u64;
        let mut remaining = len;
        let mut state = !0;
        while remaining != 0 {
            let count = remaining.min(buffer.len());
            file.read_exact(&mut buffer[..count])?;
            state = crc_extend(state, &buffer[..count]);
            remaining -= count;
        }
        let mut footer = [0; 8];
        file.read_exact(&mut footer)?;
        if &footer != FOOTER || !state != u32::from_le_bytes(header[16..20].try_into().unwrap()) {
            return Err(Error::Corrupt("checkpoint payload checksum/footer".into()));
        }
        file.seek(SeekFrom::Start(start))?;
        emit(seq, None)?;
        let mut input = crate::json_stream::Input {
            reader: BufReader::with_capacity(64 * 1024, (&mut *file).take(len as u64)),
            scratch: Vec::new(),
            limit: MAX_FRAME,
        };
        input.expect(b'{')?;
        let (mut flag_seen, mut changes_seen) = (false, false);
        loop {
            let field: String = input.value()?;
            input.expect(b':')?;
            match field.as_str() {
                "checkpoint" if !flag_seen => {
                    flag_seen = true;
                    if !input.value::<bool>()? {
                        return Err(Error::Corrupt(
                            "normal transaction inside immutable source".into(),
                        ));
                    }
                }
                "changes" if !changes_seen => {
                    changes_seen = true;
                    input.array(|change| emit(seq, Some(change)))?;
                }
                _ => return Err(Error::Corrupt("unknown/duplicate checkpoint field".into())),
            }
            input.whitespace()?;
            match input.byte()? {
                b'}' => break,
                b',' => {}
                _ => return Err(Error::Corrupt("checkpoint object delimiter".into())),
            }
        }
        input.whitespace()?;
        if !flag_seen || !changes_seen || input.peek()?.is_some() {
            return Err(Error::Corrupt("incomplete checkpoint schema".into()));
        }
        drop(input);
        cursor += (HEADER + len + 8) as u64;
        file.seek(SeekFrom::Start(cursor))?;
        sequence = Some(seq);
    }
    sequence.ok_or_else(|| Error::Corrupt("missing checkpoint".into()))
}

fn replay_impl(
    file: &mut File,
    checkpoint_end: u64,
    prefix: Option<u64>,
    apply: &mut impl FnMut(u64, Batch, &[u8], u64) -> Result<()>,
) -> Result<u64> {
    file.seek(SeekFrom::Start(0))?;
    let actual_size = file.metadata()?.len();
    let size = prefix.unwrap_or(actual_size);
    if actual_size < size || size < checkpoint_end {
        return Err(Error::Corrupt("published checkpoint truncated".into()));
    }
    let mut magic = [0; 8];
    file.read_exact(&mut magic)
        .map_err(|_| Error::Corrupt("truncated file header".into()))?;
    if &magic != MAGIC && &magic != MAGIC_V3 {
        return Err(Error::Corrupt("unsupported file format".into()));
    }
    let mut valid_end = 8u64;
    let mut sequence = 0;
    let mut first = true;
    while valid_end < size {
        if size - valid_end < HEADER as u64 {
            break;
        }
        let mut header = [0; HEADER];
        file.read_exact(&mut header)?;
        if &header[..4] != b"TXN1"
            || crc(&header[..20]) != u32::from_le_bytes(header[20..24].try_into().unwrap())
        {
            return Err(Error::Corrupt(format!(
                "invalid transaction header at {valid_end}"
            )));
        }
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        if len > MAX_FRAME {
            return Err(Error::Corrupt("oversized frame".into()));
        }
        if size - valid_end < (HEADER + len + 8) as u64 {
            break;
        }
        let seq = u64::from_le_bytes(header[8..16].try_into().unwrap());
        let mut payload = vec![0; len];
        file.read_exact(&mut payload)?;
        let mut footer = [0; 8];
        file.read_exact(&mut footer)?;
        if &footer != FOOTER
            || crc(&payload) != u32::from_le_bytes(header[16..20].try_into().unwrap())
        {
            return Err(Error::Corrupt(format!(
                "transaction checksum/footer mismatch at {valid_end}"
            )));
        }
        let batch: Batch =
            serde_json::from_slice(&payload).map_err(|e| Error::Corrupt(e.to_string()))?;
        if (batch.checkpoint && (valid_end >= checkpoint_end || (!first && seq != sequence)))
            || (!batch.checkpoint
                && (valid_end < checkpoint_end
                    || seq
                        != sequence
                            .checked_add(1)
                            .ok_or_else(|| Error::Corrupt("sequence overflow".into()))?))
        {
            return Err(Error::Corrupt("invalid transaction sequence".into()));
        }
        if first && !batch.checkpoint {
            return Err(Error::Corrupt("missing initial checkpoint".into()));
        }
        let checkpoint = batch.checkpoint;
        #[cfg(feature = "experimental-wal-delta")]
        {
            apply(seq, batch, &payload, valid_end + HEADER as u64)?;
            drop(payload);
        }
        #[cfg(not(feature = "experimental-wal-delta"))]
        {
            drop(payload);
            apply(seq, batch, &[], valid_end + HEADER as u64)?;
        }
        sequence = seq;
        first = false;
        valid_end += (HEADER + len + 8) as u64;
        if checkpoint && valid_end > checkpoint_end {
            return Err(Error::Corrupt("invalid checkpoint boundary".into()));
        }
    }
    if first || valid_end < checkpoint_end {
        return Err(Error::Corrupt("missing complete checkpoint".into()));
    }
    // An incomplete trailing frame is discarded. This cannot distinguish a torn
    // unacknowledged write from later external truncation of acknowledged data.
    if valid_end != size && prefix.is_some() {
        return Err(Error::Corrupt("incomplete immutable source".into()));
    }
    if valid_end != size {
        file.set_len(valid_end)?;
        file.sync_all()?;
    }
    file.seek(SeekFrom::End(0))?;
    Ok(sequence)
}

#[cfg(test)]
mod crc_tests {
    use super::*;
    #[cfg(feature = "experimental-streaming-open")]
    #[test]
    fn streaming_checkpoint_checks_crc_before_emitting_and_rejects_schema_errors() {
        use std::fs::OpenOptions;
        let path = std::env::temp_dir().join(format!(
            "fdb-checkpoint-stream-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let batch = Batch {
            checkpoint: true,
            changes: vec![Change::Delete {
                key: crate::Key::new("şirket", "東京", "quote\"back\\🦀"),
            }],
        };
        let frame = encode(19, &batch).unwrap();
        let bytes = [
            MAGIC_V3.as_slice(),
            frame.as_slice(),
            encode(
                19,
                &Batch {
                    checkpoint: true,
                    changes: vec![],
                },
            )
            .unwrap()
            .as_slice(),
        ]
        .concat();
        file.write_all(&bytes).unwrap();
        let mut seen = Vec::new();
        assert_eq!(
            replay_prefix_changes(&mut file, bytes.len() as u64, |seq, change| {
                seen.push((seq, change.map(|c| serde_json::to_value(c).unwrap())));
                Ok(())
            })
            .unwrap(),
            19
        );
        assert_eq!(seen.len(), 3);
        assert!(seen[0].1.is_none() && seen[2].1.is_none());
        assert_eq!(
            seen[1].1.as_ref().unwrap(),
            &serde_json::to_value(&batch.changes[0]).unwrap()
        );
        for cut in 8..8 + frame.len() {
            let mut emitted = 0;
            assert!(
                replay_prefix_changes(&mut file, cut as u64, |_, _| {
                    emitted += 1;
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(emitted, 0);
        }
        for position in 8..8 + frame.len() {
            let mut corrupt = bytes.clone();
            corrupt[position] ^= 1;
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(&corrupt).unwrap();
            let mut emitted = 0;
            assert!(
                replay_prefix_changes(&mut file, bytes.len() as u64, |_, _| {
                    emitted += 1;
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(emitted, 0);
        }
        for payload in [
            r#"{"changes":[],"checkpoint":true}"#,
            r#"{"changes":[],"checkpoint":false}"#,
            r#"{"changes":[],"changes":[],"checkpoint":true}"#,
            r#"{"checkpoint":true}"#,
            r#"{"changes":[],"checkpoint":true,}"#,
            r#"{"changes":[],"checkpoint":true}null"#,
        ] {
            let payload = payload.as_bytes();
            let mut raw = b"TXN1".to_vec();
            raw.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            raw.extend_from_slice(&19u64.to_le_bytes());
            raw.extend_from_slice(&crc(payload).to_le_bytes());
            raw.extend_from_slice(&crc(&raw).to_le_bytes());
            raw.extend_from_slice(payload);
            raw.extend_from_slice(FOOTER);
            file.set_len(0).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(MAGIC_V3).unwrap();
            file.write_all(&raw).unwrap();
            let result = replay_prefix_changes(&mut file, 8 + raw.len() as u64, |_, _| Ok(()));
            assert_eq!(
                result.is_ok(),
                payload == br#"{"changes":[],"checkpoint":true}"#
            );
        }
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn single_buffer_frame_is_byte_compatible_with_v1_and_v3() {
        for count in [0, 1, 100] {
            let batch = Batch {
                checkpoint: true,
                changes: (0..count)
                    .map(|n| Change::Delete {
                        key: crate::Key::new("şirket", "東京", &format!("quote\"back\\🦀{n}")),
                    })
                    .collect(),
            };
            let payload = serde_json::to_vec(&batch).unwrap();
            let mut reference = b"TXN1".to_vec();
            reference.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            reference.extend_from_slice(&19u64.to_le_bytes());
            reference.extend_from_slice(&crc(&payload).to_le_bytes());
            reference.extend_from_slice(&crc(&reference).to_le_bytes());
            reference.extend_from_slice(&payload);
            reference.extend_from_slice(FOOTER);
            assert_eq!(encode(19, &batch).unwrap(), reference);
        }
    }
    #[cfg(feature = "experimental-write-buffer-budget")]
    #[test]
    fn streamed_checkpoint_is_byte_compatible_and_every_cut_is_rejected() {
        use std::fs::OpenOptions;
        let path = std::env::temp_dir().join(format!(
            "fdb-stream-frame-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for count in [0, 3] {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            file.write_all(MAGIC_V3).unwrap();
            let batch = Batch {
                checkpoint: true,
                changes: (0..count)
                    .map(|i| Change::Delete {
                        key: crate::Key::new("şirket", "東京", &format!("quote\"\\🦀{i}")),
                    })
                    .collect(),
            };
            let expected = encode(7, &batch).unwrap();
            append_checkpoint_changes(&mut file, 7, batch.changes.into_iter().map(Ok)).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(&bytes[8..], expected);
            for cut in 8..bytes.len() {
                file.set_len(cut as u64).unwrap();
                assert!(
                    replay_prefix_changes(&mut file, bytes.len() as u64, |_, _| Ok(())).is_err()
                );
                file.seek(SeekFrom::Start(0)).unwrap();
                file.write_all(&bytes).unwrap();
            }
        }
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(feature = "experimental-write-buffer-budget")]
    #[test]
    fn large_fields_start_another_frame_before_crossing_existing_limit() {
        use std::fs::OpenOptions;
        let path = std::env::temp_dir().join(format!(
            "fdb-stream-large-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.write_all(MAGIC_V3).unwrap();
        let changes = [6usize, 61].into_iter().enumerate().map(|(id, mb)| {
            Ok(Change::Put {
                stored: StoredDocument {
                    version: 7,
                    document: crate::Document {
                        key: crate::Key::new("test", "files", &id.to_string()),
                        title: String::new(),
                        text: String::new(),
                        fields: std::collections::BTreeMap::from([(
                            "large".into(),
                            serde_json::Value::String("x".repeat(mb * 1024 * 1024)),
                        )]),
                        links: vec![],
                    },
                },
            })
        });
        append_checkpoint_changes(&mut file, 7, changes).unwrap();
        let mut frames = 0;
        let end = file.metadata().unwrap().len();
        replay(&mut file, end, |seq, batch| {
            assert_eq!(seq, 7);
            assert!(batch.checkpoint);
            assert_eq!(batch.changes.len(), 1);
            frames += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(frames, 2);
        assert_eq!(file.metadata().unwrap().len(), end);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn ieee_crc_and_streaming_boundaries_remain_compatible() {
        assert_eq!(crc(b"123456789"), 0xcbf43926);
        assert_eq!(crc(b""), 0);
        let bytes: Vec<_> = (0..4099).map(|n| ((n * 71) % 256) as u8).collect();
        let mut reference = !0u32;
        for &b in &bytes {
            reference ^= b as u32;
            for _ in 0..8 {
                reference = (reference >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(reference & 1));
            }
        }
        assert_eq!(crc(&bytes), !reference);
        for cut in (0..bytes.len()).step_by(7) {
            assert_eq!(
                !crc_extend(crc_extend(!0, &bytes[..cut]), &bytes[cut..]),
                !reference
            );
        }
    }
}
