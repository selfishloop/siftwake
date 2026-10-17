//! Read-only I/O sizing probe over the live indexes in the existing fixed slot.
use serde::Deserialize;
use serde_json::json;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Instant,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
struct Manifest {
    runs: Vec<Source>,
}
#[derive(Deserialize)]
struct Source {
    kind: String,
    id: u64,
}
struct Index {
    path: PathBuf,
    end: u64,
    expected: u32,
    directory_bytes: u64,
}

fn indexes() -> Result<Vec<Index>> {
    let root = Path::new("data/benchmark-work/fdb");
    let path = std::fs::read_dir(root)?
        .filter_map(|e| {
            let path = e.ok()?.path();
            let name = path.file_name()?.to_str()?;
            let id = u64::from_str_radix(
                name.strip_prefix("generation-")?.strip_suffix(".ready")?,
                16,
            )
            .ok()?;
            Some((id, path))
        })
        .max_by_key(|(id, _)| *id)
        .ok_or("existing manifest required")?
        .1;
    let raw = std::fs::read(path)?;
    if raw.len() < 24
        || &raw[..8] != b"FDBMAN03"
        || crc32fast::hash(&raw[..raw.len() - 4])
            != u32::from_le_bytes(raw[raw.len() - 4..].try_into()?)
    {
        return Err("invalid manifest".into());
    }
    let len = u32::from_le_bytes(raw[16..20].try_into()?) as usize;
    if len.checked_add(24) != Some(raw.len()) {
        return Err("manifest length".into());
    }
    let manifest: Manifest = serde_json::from_slice(&raw[20..20 + len])?;
    let mut indexes = Vec::new();
    for source in manifest.runs {
        let stem = match source.kind.as_str() {
            "Run" => "run",
            "Legacy" => "generation",
            _ => return Err("unknown source kind".into()),
        };
        let path = root.join(format!("{stem}-{:016x}.idx", source.id));
        let mut file = File::open(&path)?;
        let size = file.metadata()?.len();
        if size < 60 {
            return Err("short index".into());
        }
        file.seek(SeekFrom::End(-32))?;
        let mut footer = [0; 32];
        file.read_exact(&mut footer)?;
        if !matches!(&footer[..8], b"FDBEND03" | b"FDBEND04")
            || crc32fast::hash(&footer[..28]) != u32::from_le_bytes(footer[28..32].try_into()?)
        {
            return Err("invalid index footer".into());
        }
        let directory = u64::from_le_bytes(footer[8..16].try_into()?);
        let directory_bytes = u64::from_le_bytes(footer[16..24].try_into()?);
        if directory.checked_add(directory_bytes) != Some(size - 32) {
            return Err("directory bounds".into());
        }
        indexes.push(Index {
            path,
            end: size - 32,
            expected: u32::from_le_bytes(footer[24..28].try_into()?),
            directory_bytes,
        });
    }
    Ok(indexes)
}

fn scan(indexes: &[Index], buffer_bytes: usize) -> Result<serde_json::Value> {
    let mut buffer = vec![0; buffer_bytes];
    let mut read_ms = 0.;
    let mut compute_ms = 0.;
    let mut calls = 0;
    for index in indexes {
        let mut file = File::open(&index.path)?.take(index.end);
        let mut crc = 0;
        let mut read = 0;
        loop {
            let start = Instant::now();
            let n = file.read(&mut buffer)?;
            read_ms += start.elapsed().as_secs_f64() * 1000.;
            if n == 0 {
                break;
            }
            calls += 1;
            read += n as u64;
            let start = Instant::now();
            let mut hasher = crc32fast::Hasher::new_with_initial(crc);
            hasher.update(&buffer[..n]);
            crc = hasher.finalize();
            compute_ms += start.elapsed().as_secs_f64() * 1000.;
        }
        if read != index.end || crc != index.expected {
            return Err("index checksum mismatch".into());
        }
    }
    Ok(
        json!({"buffer_bytes":buffer_bytes,"read_calls":calls,"read_ms":read_ms,"compute_ms":compute_ms,"scan_ms":read_ms+compute_ms,"all_crcs_verified":true}),
    )
}

fn main() -> Result<()> {
    let indexes = indexes()?;
    let sizes = [64 * 1024, 256 * 1024, 1024 * 1024];
    println!(
        "{}",
        json!({"phase":"checksum_design","indexes":indexes.len(),"bytes_per_scan":indexes.iter().map(|i|i.end).sum::<u64>(),"directory_bytes":indexes.iter().map(|i|i.directory_bytes).sum::<u64>(),"scope":"warm OS cache;prefix reads plus CRC only;not engine open, RAM or directory parsing;no mutations","buffer_sizes":sizes,"balanced_orders":6,"allocation_and_file_open_outside_timing":true})
    );
    for size in sizes {
        let _ = scan(&indexes, size)?;
    }
    for (round, order) in [
        [0, 1, 2],
        [2, 1, 0],
        [1, 0, 2],
        [2, 0, 1],
        [0, 2, 1],
        [1, 2, 0],
    ]
    .iter()
    .enumerate()
    {
        for &slot in order {
            let mut result = scan(&indexes, sizes[slot])?;
            result["phase"] = json!("checksum_scan");
            result["round"] = json!(round);
            println!("{result}");
        }
    }
    Ok(())
}
