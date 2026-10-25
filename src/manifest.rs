use crate::format::{Batch, MAGIC_V4, append, crc, encode};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    Legacy,
    Run,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Source {
    pub kind: Kind,
    pub id: u64,
    pub end: u64,
    pub checksum: u32,
    pub sequence: u64,
    pub keys: usize,
}
impl Source {
    pub fn path(&self, dir: &Path, extension: &str) -> PathBuf {
        let stem = match self.kind {
            Kind::Legacy => "generation",
            Kind::Run => "run",
        };
        dir.join(format!("{stem}-{:016x}.{extension}", self.id))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Active {
    pub id: u64,
    pub end: u64,
    pub sequence: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub version: u32,
    pub runs: Vec<Source>,
    pub active: Active,
}

pub(crate) enum Publication {
    Legacy { end: u64 },
    Current(Manifest),
}

pub(crate) fn path(dir: &Path, id: u64, extension: &str) -> PathBuf {
    dir.join(format!("generation-{id:016x}.{extension}"))
}

pub(crate) fn inventory(dir: &Path) -> Result<(Option<u64>, u64)> {
    let mut latest = None;
    let mut maximum = 0;
    for e in std::fs::read_dir(dir)? {
        let name = e?.file_name();
        let name = name.to_string_lossy();
        let Some((stem, suffix)) = name.split_once('.') else {
            continue;
        };
        if let Some(n) = stem
            .strip_prefix("generation-")
            .or_else(|| stem.strip_prefix("run-"))
            && let Ok(id) = u64::from_str_radix(n, 16)
        {
            maximum = maximum.max(id);
            if stem.starts_with("generation-") && suffix == "ready" {
                latest = Some(latest.map_or(id, |n: u64| n.max(id)));
            }
        }
    }
    Ok((latest, maximum))
}

pub(crate) fn read(dir: &Path, id: u64) -> Result<Publication> {
    let mut bytes = vec![];
    File::open(path(dir, id, "ready"))?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() == 28 && &bytes[..8] == b"FDBREADY" {
        if crc(&bytes[..24]) != u32::from_le_bytes(bytes[24..28].try_into().unwrap())
            || u64::from_le_bytes(bytes[8..16].try_into().unwrap()) != id
        {
            return Err(Error::Corrupt("invalid legacy marker".into()));
        }
        let end = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        if end < 40 {
            return Err(Error::Corrupt("invalid legacy checkpoint length".into()));
        }
        return Ok(Publication::Legacy { end });
    }
    if bytes.len() < 24 || bytes.len() > 16 * 1024 * 1024 || &bytes[..8] != b"FDBMAN03" {
        return Err(Error::Corrupt("invalid manifest header".into()));
    }
    let len = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    if u64::from_le_bytes(bytes[8..16].try_into().unwrap()) != id
        || len.checked_add(24) != Some(bytes.len())
        || crc(&bytes[..bytes.len() - 4])
            != u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap())
    {
        return Err(Error::Corrupt("manifest checksum/length mismatch".into()));
    }
    let m: Manifest =
        serde_json::from_slice(&bytes[20..20 + len]).map_err(|e| Error::Corrupt(e.to_string()))?;
    if m.version != 3 || m.active.id == 0 || m.active.end < 40 || m.active.id > id {
        return Err(Error::Corrupt("invalid manifest active source".into()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for s in &m.runs {
        if s.id == 0
            || s.id > id
            || s.end < 40
            || s.sequence > m.active.sequence
            || !seen.insert((s.kind as u8, s.id))
            || (s.kind == Kind::Legacy && s.id == m.active.id)
        {
            return Err(Error::Corrupt("invalid manifest source list".into()));
        }
    }
    Ok(Publication::Current(m))
}

#[cfg(unix)]
pub(crate) fn sync_dir(dir: &Path) -> Result<()> {
    File::open(dir)?.sync_all()?;
    Ok(())
}
#[cfg(not(unix))]
pub(crate) fn sync_dir(_dir: &Path) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
fn publish_ready(pending: &Path, ready: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let wide = |path: &Path| -> Result<Vec<u16>> {
        let parent = path
            .parent()
            .ok_or_else(|| Error::Invalid("publication parent".into()))?;
        let name = path
            .file_name()
            .ok_or_else(|| Error::Invalid("publication name".into()))?;
        // canonicalize supplies extended-length Windows paths, as std::fs
        // already does for long paths. Only the not-yet-existing leaf is joined.
        let absolute = parent.canonicalize()?.join(name);
        let mut value: Vec<_> = absolute.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(Error::Invalid("NUL in publication path".into()));
        }
        value.push(0);
        Ok(value)
    };
    let from = wide(pending)?;
    let to = wide(ready)?;
    // No replacement/cross-volume copy: IDs are unique. Request the completed
    // namespace move before obsolete sources can be reclaimed (WRITE_THROUGH).
    // SAFETY: both pointers reference NUL-terminated buffers alive for this call.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 8) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn publish_ready(pending: &Path, ready: &Path) -> Result<()> {
    std::fs::rename(pending, ready)?;
    Ok(())
}

pub(crate) fn publish(dir: &Path, id: u64, manifest: &Manifest) -> Result<()> {
    #[cfg(feature = "diagnostic-timing")]
    let _publish = crate::timing::maintenance::span("fsync_publish");
    let payload = serde_json::to_vec(manifest)?;
    if payload.len() > 16 * 1024 * 1024 - 24 {
        return Err(Error::Invalid("manifest exceeds 16 MiB".into()));
    }
    let mut bytes = b"FDBMAN03".to_vec();
    bytes.extend_from_slice(&id.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&crc(&bytes).to_le_bytes());
    let pending = path(dir, id, "pending");
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)?;
    f.write_all(&bytes)?;
    f.sync_all()?;
    drop(f);
    publish_ready(&pending, &path(dir, id, "ready"))?;
    sync_dir(dir)
}

pub(crate) fn new_active(dir: &Path, id: u64, sequence: u64) -> Result<(File, Active)> {
    let mut log = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path(dir, id, "fdb"))?;
    log.write_all(MAGIC_V4)?;
    append(
        &mut log,
        &encode(
            sequence,
            &Batch {
                checkpoint: true,
                changes: vec![],
            },
        )?,
    )?;
    let end = log.metadata()?.len();
    Ok((log, Active { id, end, sequence }))
}
