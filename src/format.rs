//! FDBLOG04: binary payloads in TXN2 frames; legacy JSON replay retained for fixtures.
//! file magic, then independently checksummed atomic transaction frames.
use crate::model::StoredDocument;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

pub(crate) const MAGIC: &[u8; 8] = b"FDBLOG01";
pub(crate) const MAGIC_V4: &[u8; 8] = b"FDBLOG04";
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
    // Preserve the raw (unfinalized) state contract. crc32fast accepts and
    // returns finalized IEEE CRCs; complement on both sides, including for
    // chained/nonzero initial states. Tiny frame headers stay on our tables.
    if bytes.len() >= 128 {
        let mut hasher = crc32fast::Hasher::new_with_initial(!state);
        hasher.update(bytes);
        return !hasher.finalize();
    }
    crc_extend_tables(state, bytes)
}

fn crc_extend_tables(state: u32, bytes: &[u8]) -> u32 {
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

const fn crc_transform(matrix: &[u32; 32], mut value: u32) -> u32 {
    let mut result = 0;
    let mut bit = 0;
    while value != 0 {
        if value & 1 != 0 {
            result ^= matrix[bit];
        }
        value >>= 1;
        bit += 1;
    }
    result
}

// IEEE zero-byte shift operators, then powers of two bytes. No heap allocation.
const fn crc_shift_powers() -> [[u32; 32]; 64] {
    let mut powers = [[0; 32]; 64];
    let mut bit = 0;
    while bit < 32 {
        let mut value = 1u32 << bit;
        let mut step = 0;
        while step < 8 {
            value = (value >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(value & 1));
            step += 1;
        }
        powers[0][bit] = value;
        bit += 1;
    }
    let mut power = 1;
    while power < 64 {
        let mut bit = 0;
        while bit < 32 {
            powers[power][bit] = crc_transform(&powers[power - 1], powers[power - 1][bit]);
            bit += 1;
        }
        power += 1;
    }
    powers
}
static CRC_SHIFT_POWERS: [[u32; 32]; 64] = crc_shift_powers();

/// CRC(A || B), using already computed, finalized IEEE CRCs and B's byte length.
pub(crate) fn crc_combine(mut left: u32, right: u32, mut right_len: u64) -> u32 {
    let mut bit = 0;
    while right_len != 0 {
        if right_len & 1 != 0 {
            left = crc_transform(&CRC_SHIFT_POWERS[bit], left);
        }
        right_len >>= 1;
        bit += 1;
    }
    left ^ right
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
    let _encode = crate::timing::span("frame.encode_binary_crc");
    // Include the footer so finalization cannot grow/copy the entire payload.
    let payload_len = crate::binary_batch::encoded_len(batch)?;
    let mut bytes = Vec::with_capacity(HEADER + payload_len + FOOTER.len());
    bytes.resize(HEADER, 0);
    crate::binary_batch::encode_into(batch, &mut bytes)?;
    finish_frame(sequence, bytes)
}

fn finish_frame(sequence: u64, mut bytes: Vec<u8>) -> Result<Vec<u8>> {
    let len = bytes.len() - HEADER;
    bytes[..4].copy_from_slice(b"TXN2");
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
    let bytes = encode(sequence, batch)?;
    let locations = crate::wal_delta::locations(
        &bytes[HEADER..bytes.len() - FOOTER.len()],
        HEADER as u64,
        batch,
    )?;
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
        None,
        false,
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
    let mut changes = changes.peekable();
    loop {
        let lease = crate::write_budget::reserve(64 * 1024, 8 * 1024)?;
        let start = file.seek(SeekFrom::End(0))?;
        file.write_all(&[0; HEADER])?;
        let mut len = 5usize;
        let mut count = 0u32;
        {
            let mut writer = BufWriter::with_capacity(lease.bytes, &mut *file);
            writer.write_all(&[1, 0, 0, 0, 0])?;
            while let Some(next) = changes.peek() {
                let entry_len = match next {
                    Ok(Change::Put { stored }) => {
                        5 + crate::payload::encoded_len(&stored.document)?
                    }
                    Ok(Change::Delete { key }) => {
                        17 + key.scope.len() + key.collection.len() + key.id.len()
                    }
                    Err(_) => {
                        changes.next().unwrap()?;
                        unreachable!();
                    }
                };
                if entry_len > MAX_FRAME - 5 {
                    return Err(Error::Invalid("checkpoint exceeds 64 MiB".into()));
                }
                if count != 0 && len + entry_len > 8 * 1024 * 1024 {
                    break;
                }
                let entry = crate::binary_batch::entry(&changes.next().unwrap()?)?;
                writer.write_all(&entry)?;
                len += entry.len();
                count += 1;
            }
            writer.flush()?;
        }
        // Count is known only after streaming. Patch it, then checksum in 64 KiB blocks.
        file.seek(SeekFrom::Start(start + HEADER as u64 + 1))?;
        file.write_all(&count.to_le_bytes())?;
        file.seek(SeekFrom::Start(start + HEADER as u64))?;
        let mut state = !0;
        let mut remaining = len;
        let mut buffer = [0; 64 * 1024];
        while remaining != 0 {
            let n = remaining.min(buffer.len());
            file.read_exact(&mut buffer[..n])?;
            state = crc_extend(state, &buffer[..n]);
            remaining -= n;
        }
        file.write_all(FOOTER)?;
        let mut header = [0; HEADER];
        header[..4].copy_from_slice(b"TXN2");
        header[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        header[8..16].copy_from_slice(&sequence.to_le_bytes());
        header[16..20].copy_from_slice(&(!state).to_le_bytes());
        let checksum = crc(&header[..20]);
        header[20..24].copy_from_slice(&checksum.to_le_bytes());
        file.seek(SeekFrom::Start(start))?;
        file.write_all(&header)?;
        file.seek(SeekFrom::Start(start + (HEADER + len + 8) as u64))?;
        file.sync_all()?;
        drop(lease);
        if changes.peek().is_none() {
            return Ok(());
        }
    }
}

#[cfg(any(not(feature = "experimental-wal-delta"), test))]
pub(crate) fn replay(
    file: &mut File,
    checkpoint_end: u64,
    mut apply: impl FnMut(u64, Batch) -> Result<()>,
) -> Result<u64> {
    replay_impl(
        file,
        checkpoint_end,
        None,
        None,
        false,
        &mut |seq, batch, _, _| apply(seq, batch),
    )
}

#[cfg(all(
    not(feature = "experimental-streaming-open"),
    feature = "experimental-merge-postings"
))]
pub(crate) fn replay_prefix(
    file: &mut File,
    end: u64,
    mut apply: impl FnMut(u64, Batch) -> Result<()>,
) -> Result<u64> {
    replay_impl(
        file,
        end,
        Some(end),
        None,
        false,
        &mut |seq, batch, _, _| apply(seq, batch),
    )
}

/// Verify the authoritative whole-source CRC while replaying complete frames.
/// Callbacks are provisional: callers must not publish before this succeeds.
#[cfg(not(feature = "experimental-streaming-open"))]
pub(crate) fn replay_prefix_checked(
    file: &mut File,
    end: u64,
    expected: u32,
    mut apply: impl FnMut(u64, Batch) -> Result<()>,
) -> Result<u64> {
    replay_impl(
        file,
        end,
        Some(end),
        Some(expected),
        false,
        &mut |seq, batch, _, _| apply(seq, batch),
    )
}

/// Full native checkpoint integrity without allocating decoded documents.
/// Shares framing, sequence, truncation and whole-source CRC checks with replay.
#[cfg(not(feature = "experimental-streaming-open"))]
pub(crate) fn validate_native_prefix(
    file: &mut File,
    end: u64,
    expected: u32,
    mut validate: impl FnMut(u64, &[u8]) -> Result<()>,
) -> Result<u64> {
    replay_impl(
        file,
        end,
        Some(end),
        Some(expected),
        true,
        &mut |seq, _, payload, _| validate(seq, payload),
    )
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
    if &magic != MAGIC && &magic != MAGIC_V3 && &magic != MAGIC_V4 {
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
        if ((&magic == MAGIC_V4 && &header[..4] != b"TXN2")
            || (&header[..4] != b"TXN1" && &header[..4] != b"TXN2"))
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
        if &header[..4] == b"TXN2" {
            if len < 5 {
                return Err(Error::Corrupt("short binary checkpoint batch".into()));
            }
            let mut reader = BufReader::with_capacity(64 * 1024, (&mut *file).take(len as u64));
            let mut head = [0; 5];
            reader.read_exact(&mut head)?;
            let count = u32::from_le_bytes(head[1..5].try_into().unwrap()) as usize;
            if head[0] != 1 || count > (len - 5) / 17 {
                return Err(Error::Corrupt("binary checkpoint flag/count".into()));
            }
            emit(seq, None)?;
            let mut remaining = len - 5;
            let mut scratch = Vec::new();
            for _ in 0..count {
                if remaining < 5 {
                    return Err(Error::Corrupt("binary checkpoint entry header".into()));
                }
                let mut entry = [0; 5];
                reader.read_exact(&mut entry)?;
                remaining -= 5;
                let n = u32::from_le_bytes(entry[1..5].try_into().unwrap()) as usize;
                if n > remaining {
                    return Err(Error::Corrupt("binary checkpoint entry bounds".into()));
                }
                if scratch.capacity() > 256 * 1024 {
                    scratch = Vec::new();
                }
                scratch.resize(n, 0);
                reader.read_exact(&mut scratch)?;
                emit(
                    seq,
                    Some(crate::binary_batch::decode_entry(entry[0], &scratch)?),
                )?;
                remaining -= n;
            }
            if remaining != 0 {
                return Err(Error::Corrupt("binary checkpoint trailing bytes".into()));
            }
            drop(reader);
            cursor += (HEADER + len + 8) as u64;
            file.seek(SeekFrom::Start(cursor))?;
            sequence = Some(seq);
            continue;
        }
        emit(seq, None)?;
        let mut input = crate::json_stream::Input {
            position: 0,
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

/// A full active-log check without truncating an incomplete trailing frame.
pub(crate) fn check_log(
    file: &mut File,
    checkpoint_end: u64,
    mut apply: impl FnMut(u64, Batch) -> Result<()>,
) -> Result<u64> {
    let end = file.metadata()?.len();
    replay_impl(
        file,
        checkpoint_end,
        Some(end),
        None,
        false,
        &mut |seq, batch, _, _| apply(seq, batch),
    )
}

fn replay_impl(
    file: &mut File,
    checkpoint_end: u64,
    prefix: Option<u64>,
    expected_source_crc: Option<u32>,
    native_validation: bool,
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
    if &magic != MAGIC && &magic != MAGIC_V3 && &magic != MAGIC_V4 {
        return Err(Error::Corrupt("unsupported file format".into()));
    }
    if native_validation && &magic != MAGIC_V4 {
        return Err(Error::Corrupt("native validation requires FDBLOG04".into()));
    }
    let mut source_crc = crc(&magic);
    let mut valid_end = 8u64;
    let mut sequence = 0;
    let mut first = true;
    while valid_end < size {
        if size - valid_end < HEADER as u64 {
            break;
        }
        let mut header = [0; HEADER];
        file.read_exact(&mut header)?;
        if ((&magic == MAGIC_V4 && &header[..4] != b"TXN2")
            || (&header[..4] != b"TXN1" && &header[..4] != b"TXN2"))
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
        #[cfg(feature = "diagnostic-timing")]
        let read = crate::timing::span("source.frame_read_allocate");
        let mut payload = vec![0; len];
        file.read_exact(&mut payload)?;
        let mut footer = [0; 8];
        file.read_exact(&mut footer)?;
        #[cfg(feature = "diagnostic-timing")]
        drop(read);
        #[cfg(feature = "diagnostic-timing")]
        let checksum = crate::timing::span("source.frame_crc");
        let payload_crc = crc(&payload);
        #[cfg(feature = "diagnostic-timing")]
        drop(checksum);
        if &footer != FOOTER
            || payload_crc != u32::from_le_bytes(header[16..20].try_into().unwrap())
        {
            return Err(Error::Corrupt(format!(
                "transaction checksum/footer mismatch at {valid_end}"
            )));
        }
        if expected_source_crc.is_some() {
            #[cfg(feature = "diagnostic-timing")]
            let _combine = crate::timing::span("open.source_crc_combine");
            source_crc = !crc_extend(!source_crc, &header);
            source_crc = crc_combine(source_crc, payload_crc, len as u64);
            source_crc = !crc_extend(!source_crc, &footer);
        }
        let batch: Batch = if native_validation {
            if payload.len() < 5 || payload[0] > 1 {
                return Err(Error::Corrupt("binary checkpoint header".into()));
            }
            // Changes are validated by the borrowed callback after framing and
            // sequence checks. This mode never exposes an empty batch as data.
            Batch {
                checkpoint: payload[0] != 0,
                changes: Vec::new(),
            }
        } else if &header[..4] == b"TXN2" {
            crate::binary_batch::decode(&payload)?
        } else {
            serde_json::from_slice(&payload).map_err(|e| Error::Corrupt(e.to_string()))?
        };
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
        if native_validation {
            apply(seq, batch, &payload, valid_end + HEADER as u64)?;
        } else {
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
    if expected_source_crc.is_some_and(|expected| source_crc != expected) {
        return Err(Error::Corrupt("source checkpoint checksum mismatch".into()));
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
    #[cfg(not(feature = "experimental-streaming-open"))]
    #[test]
    fn native_validation_handles_large_small_growing_and_empty_frames() {
        let path = std::env::temp_dir().join(format!("fdb-reused-frame-{}", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut bytes = MAGIC_V4.to_vec();
        let mut expected = Vec::new();
        for (n, len) in [16384, 1, 32768, 7].into_iter().enumerate() {
            let stored = StoredDocument {
                document: crate::Document {
                    key: crate::Key::new("shared", "texts", &n.to_string()),
                    title: "東京".into(),
                    text: "é".repeat(len),
                    fields: Default::default(),
                    links: vec![],
                },
                version: 7,
            };
            expected.push(serde_json::to_value(&stored).unwrap());
            bytes.extend(
                encode(
                    7,
                    &Batch {
                        checkpoint: true,
                        changes: vec![Change::Put { stored }],
                    },
                )
                .unwrap(),
            );
        }
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
        file.write_all(&bytes).unwrap();
        let mut actual = Vec::new();
        let mut frame_sizes = Vec::new();
        validate_native_prefix(&mut file, bytes.len() as u64, crc(&bytes), |_, payload| {
            frame_sizes.push(payload.len());
            let (_, entries) = crate::binary_batch::entries(payload)?;
            for (tag, _, entry) in entries {
                assert_eq!(tag, 0);
                let view = crate::payload::checked(entry)?;
                view.validate_document(entry.len())?;
                actual.push(serde_json::to_value(view.to_document()?).unwrap());
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(actual, expected);
        assert!(frame_sizes[0] > frame_sizes[1]);
        assert!(frame_sizes[2] > frame_sizes[0]);
        assert!(frame_sizes[2] > frame_sizes[3]);
        assert_eq!(frame_sizes[4], 5);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn accelerated_ieee_matches_tables_for_offsets_lengths_states_and_chaining() {
        // Bounded unit bytes, not a document/benchmark corpus.
        let mut random = 0x12345678u32;
        let bytes: Vec<_> = (0..262_177)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                random as u8
            })
            .collect();
        for offset in 0..32 {
            for len in (0..260).chain([
                511, 1023, 1024, 1025, 4095, 4096, 4097, 65535, 65536, 262144,
            ]) {
                let input = &bytes[offset..offset + len];
                for state in [0, !0, 1, 0x12345678, 0xdeadbeef] {
                    let expected = crc_extend_tables(state, input);
                    assert_eq!(
                        crc_extend(state, input),
                        expected,
                        "offset={offset} len={len} state={state}"
                    );
                    for split in [0, len.min(127), len.min(128), len / 2, len] {
                        assert_eq!(
                            crc_extend(crc_extend(state, &input[..split]), &input[split..]),
                            expected
                        );
                    }
                }
            }
        }
    }
    #[cfg(not(feature = "experimental-streaming-open"))]
    #[test]
    fn borrowed_native_validation_matches_replay_on_cuts_and_corruption() {
        let path = std::env::temp_dir().join(format!("fdb-borrowed-frame-{}", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let batch = Batch {
            checkpoint: true,
            changes: vec![Change::Put {
                stored: StoredDocument {
                    document: crate::Document {
                        key: crate::Key::new("shared", "texts", "a"),
                        title: "İstanbul".into(),
                        text: "東京 🦀".into(),
                        fields: Default::default(),
                        links: vec![],
                    },
                    version: 7,
                },
            }],
        };
        file.write_all(MAGIC_V4).unwrap();
        append(&mut file, &encode(7, &batch).unwrap()).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut original = Vec::new();
        file.read_to_end(&mut original).unwrap();
        let expected = crc(&original);
        let verify = |file: &mut File, end| {
            validate_native_prefix(file, end, expected, |seq, bytes| {
                let (checkpoint, entries) = crate::binary_batch::entries(bytes)?;
                assert!(checkpoint);
                assert_eq!(seq, 7);
                for (tag, _, bytes) in entries {
                    assert_eq!(tag, 0);
                    let view = crate::payload::checked(bytes)?;
                    view.validate_document(bytes.len())?;
                    assert_eq!(view.version(), 7);
                }
                Ok(())
            })
        };
        assert_eq!(verify(&mut file, original.len() as u64).unwrap(), 7);
        for cut in 0..original.len() {
            assert!(verify(&mut file, cut as u64).is_err(), "cut {cut}");
        }
        for position in 0..original.len() {
            let mut bytes = original.clone();
            bytes[position] ^= 1;
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(&bytes).unwrap();
            assert!(
                verify(&mut file, bytes.len() as u64).is_err(),
                "byte {position}"
            );
        }
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
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
    fn binary_frame_has_versioned_header_and_expected_crc() {
        for count in [0, 1, 100] {
            let batch = Batch {
                checkpoint: true,
                changes: (0..count)
                    .map(|n| Change::Delete {
                        key: crate::Key::new("şirket", "東京", &format!("quote\"back\\🦀{n}")),
                    })
                    .collect(),
            };
            let payload = crate::binary_batch::encode(&batch).unwrap();
            let mut reference = b"TXN2".to_vec();
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
            for position in 8..bytes.len() {
                let mut damaged = bytes.clone();
                damaged[position] ^= 1;
                file.seek(SeekFrom::Start(0)).unwrap();
                file.write_all(&damaged).unwrap();
                assert!(replay(&mut file, bytes.len() as u64, |_, _| Ok(())).is_err());
            }
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(&bytes).unwrap();
            for cut in 8..bytes.len() {
                file.set_len(cut as u64).unwrap();
                assert!(replay(&mut file, bytes.len() as u64, |_, _| Ok(())).is_err());
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
            assert_eq!(
                crc_combine(
                    crc(&bytes[..cut]),
                    crc(&bytes[cut..]),
                    (bytes.len() - cut) as u64
                ),
                !reference
            );
        }
        assert_eq!(crc_combine(crc(&bytes), crc(b""), 0), crc(&bytes));
        // Powers beyond a u32 length, without allocating a multi-GB buffer.
        let mut checksum = crc(b"abc");
        let mut length = 3u64;
        for _ in 0..34 {
            let doubled = crc_combine(checksum, checksum, length);
            let tripled = crc_combine(doubled, checksum, length);
            assert_eq!(tripled, crc_combine(checksum, doubled, length * 2));
            checksum = doubled;
            length *= 2;
        }
    }

    #[cfg(not(feature = "experimental-streaming-open"))]
    #[test]
    fn whole_source_crc_is_checked_even_when_each_frame_is_valid() {
        let path = std::env::temp_dir().join(format!(
            "fdb-combined-crc-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut bytes = MAGIC_V4.to_vec();
        for key in ["bir", "İKİ", "東京"] {
            bytes.extend(
                encode(
                    19,
                    &Batch {
                        checkpoint: true,
                        changes: vec![Change::Delete {
                            key: crate::Key::new("scope", "docs", key),
                        }],
                    },
                )
                .unwrap(),
            );
        }
        file.write_all(&bytes).unwrap();
        let end = bytes.len() as u64;
        let mut seen = 0;
        assert_eq!(
            replay_prefix_checked(&mut file, end, crc(&bytes), |_, _| {
                seen += 1;
                Ok(())
            })
            .unwrap(),
            19
        );
        assert_eq!(seen, 3);
        assert!(matches!(
            replay_prefix_checked(&mut file, end, crc(&bytes) ^ 1, |_, _| Ok(())),
            Err(Error::Corrupt(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        // Published prefix excludes trailing bytes, just like the original CRC pass.
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(b"unpublished").unwrap();
        assert_eq!(
            replay_prefix_checked(&mut file, end, crc(&bytes), |_, _| Ok(())).unwrap(),
            19
        );
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
}
