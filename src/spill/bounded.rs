//! Consecutive staged groups preserve ID order and bound open readers to 32.
use super::*;
use std::io::{Seek, SeekFrom};

pub(super) fn collapse(spills: &mut Spills) -> Result<()> {
    while spills.paths.len() > 32 {
        let inputs = spills.paths.clone();
        let mut outputs = Vec::new();
        for group in inputs.chunks(32) {
            let mut name = spills.stem.file_name().unwrap().to_os_string();
            name.push(format!(".spill-stage-{}", spills.all_paths.len()));
            let path = spills.stem.with_file_name(name);
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            // Register only files we created, before any fallible serialization.
            spills.all_paths.push(path.clone());
            let length = write_group(spills, group, file)?;
            spills.lengths.insert(path.clone(), length);
            outputs.push(path);
        }
        spills.paths = outputs;
    }
    Ok(())
}

fn write_group(spills: &Spills, paths: &[PathBuf], file: File) -> Result<u64> {
    let mut writer = BufWriter::new(file);
    let mut readers: Vec<_> = paths
        .iter()
        .map(|p| spills.cursor(p))
        .collect::<Result<_>>()?;
    let mut heap = BinaryHeap::new();
    for (n, reader) in readers.iter_mut().enumerate() {
        if let Some(term) = reader.advance()? {
            heap.push(Reverse((term, n)));
        }
    }
    let mut offset = 0u64;
    while let Some(Reverse((term, n))) = heap.pop() {
        let mut same = vec![n];
        while heap.peek().is_some_and(|Reverse((next, _))| next == &term) {
            same.push(heap.pop().unwrap().0.1);
        }
        same.sort_unstable();
        let key = serde_json::to_vec(&term)?;
        let key_len =
            u32::try_from(key.len()).map_err(|_| Error::Invalid("spill key overflow".into()))?;
        let start = offset;
        writer.write_all(&[0; 20])?;
        writer.write_all(&key)?;
        let mut count = 0u32;
        let mut state = !0;
        let mut previous = None;
        for n in same {
            let reader = &mut readers[n];
            count = count
                .checked_add(reader.remaining)
                .ok_or_else(|| Error::Invalid("spill count overflow".into()))?;
            copy_ids(reader, &mut previous, |bytes| {
                state = crc_extend(state, bytes);
                writer.write_all(bytes)?;
                Ok(())
            })?;
            if let Some(next) = reader.advance()? {
                heap.push(Reverse((next, n)));
            }
        }
        offset = offset
            .checked_add(20 + key.len() as u64 + u64::from(count) * 4)
            .ok_or_else(|| Error::Invalid("spill offset overflow".into()))?;
        let mut header = [0; 20];
        header[..4].copy_from_slice(&key_len.to_le_bytes());
        header[4..8].copy_from_slice(&count.to_le_bytes());
        header[8..12].copy_from_slice(&crc(&key).to_le_bytes());
        header[12..16].copy_from_slice(&(!state).to_le_bytes());
        let checksum = crc(&header[..16]);
        header[16..].copy_from_slice(&checksum.to_le_bytes());
        writer.seek(SeekFrom::Start(start))?;
        writer.write_all(&header)?;
        writer.seek(SeekFrom::Start(offset))?;
    }
    writer.flush()?;
    Ok(offset)
}
