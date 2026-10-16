//! TXN2 batch: checkpoint byte, change count, then tagged length-prefixed entries.
use crate::{
    Error, Key, Result,
    format::{Batch, Change, MAX_FRAME},
};

fn invalid() -> Error {
    Error::Corrupt("binary batch bounds/schema".into())
}

pub(crate) fn entry_len(change: &Change) -> Result<usize> {
    Ok(match change {
        Change::Put { stored } => 5 + crate::payload::encoded_len(&stored.document)?,
        Change::Delete { key } => 17 + key.scope.len() + key.collection.len() + key.id.len(),
    })
}

pub(crate) fn encoded_len(batch: &Batch) -> Result<usize> {
    u32::try_from(batch.changes.len()).map_err(|_| invalid())?;
    batch.changes.iter().try_fold(5usize, |len, change| {
        let next = len.checked_add(entry_len(change)?);
        match next {
            Some(next) if next <= MAX_FRAME => Ok(next),
            _ => Err(Error::Invalid("transaction exceeds 64 MiB".into())),
        }
    })
}

#[cfg(feature = "experimental-write-buffer-budget")]
pub(crate) fn entry(change: &Change) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(entry_len(change)?);
    write_entry(&mut bytes, change)?;
    Ok(bytes)
}

fn write_entry(out: &mut Vec<u8>, change: &Change) -> Result<()> {
    let len = entry_len(change)?;
    if len > MAX_FRAME - 5 {
        return Err(Error::Invalid("transaction exceeds 64 MiB".into()));
    }
    out.push(if matches!(change, Change::Put { .. }) {
        0
    } else {
        1
    });
    out.extend_from_slice(&((len - 5) as u32).to_le_bytes());
    match change {
        Change::Put { stored } => crate::payload::encode_into(stored, out)?,
        Change::Delete { key } => {
            for s in [&key.scope, &key.collection, &key.id] {
                out.extend_from_slice(
                    &u32::try_from(s.len()).map_err(|_| invalid())?.to_le_bytes(),
                );
                out.extend_from_slice(s.as_bytes());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn encode(batch: &Batch) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    encode_into(batch, &mut out)?;
    Ok(out)
}

pub(crate) fn encode_into(batch: &Batch, out: &mut Vec<u8>) -> Result<()> {
    let start = out.len();
    out.push(u8::from(batch.checkpoint));
    out.extend_from_slice(
        &u32::try_from(batch.changes.len())
            .map_err(|_| invalid())?
            .to_le_bytes(),
    );
    for change in &batch.changes {
        if entry_len(change)? > MAX_FRAME - (out.len() - start) {
            return Err(Error::Invalid("transaction exceeds 64 MiB".into()));
        }
        write_entry(out, change)?;
    }
    Ok(())
}

type Entry<'a> = (u8, usize, &'a [u8]);
pub(crate) fn entries(payload: &[u8]) -> Result<(bool, Vec<Entry<'_>>)> {
    if payload.len() < 5 || payload[0] > 1 {
        return Err(invalid());
    }
    let count = u32::from_le_bytes(payload[1..5].try_into().unwrap()) as usize;
    if count > (payload.len() - 5) / 17 {
        return Err(invalid());
    }
    let mut position = 5usize;
    let mut result = Vec::new();
    for _ in 0..count {
        let header = payload
            .get(position..position.checked_add(5).ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let tag = header[0];
        let len = u32::from_le_bytes(header[1..5].try_into().unwrap()) as usize;
        position += 5;
        let end = position.checked_add(len).ok_or_else(invalid)?;
        let bytes = payload.get(position..end).ok_or_else(invalid)?;
        if tag > 1 {
            return Err(invalid());
        }
        result.push((tag, position, bytes));
        position = end;
    }
    if position != payload.len() {
        return Err(invalid());
    }
    Ok((payload[0] != 0, result))
}

pub(crate) fn decode(payload: &[u8]) -> Result<Batch> {
    #[cfg(feature = "diagnostic-timing")]
    let _span = crate::timing::span("batch.decode_binary");
    let (checkpoint, entries) = entries(payload)?;
    let mut changes = Vec::new();
    for (tag, _, bytes) in entries {
        changes.push(decode_entry(tag, bytes)?);
    }
    Ok(Batch {
        checkpoint,
        changes,
    })
}

pub(crate) fn decode_entry(tag: u8, bytes: &[u8]) -> Result<Change> {
    Ok(if tag == 0 {
        Change::Put {
            stored: crate::payload::checked(bytes)?.to_document()?,
        }
    } else if tag == 1 {
        let mut position = 0usize;
        let mut parts = [""; 3];
        for part in &mut parts {
            let end = position.checked_add(4).ok_or_else(invalid)?;
            let len = u32::from_le_bytes(
                bytes
                    .get(position..end)
                    .ok_or_else(invalid)?
                    .try_into()
                    .unwrap(),
            ) as usize;
            position = end;
            let end = position.checked_add(len).ok_or_else(invalid)?;
            *part = std::str::from_utf8(bytes.get(position..end).ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
            position = end;
        }
        if position != bytes.len() {
            return Err(invalid());
        }
        Change::Delete {
            key: Key::new(parts[0], parts[1], parts[2]),
        }
    } else {
        return Err(invalid());
    })
}
