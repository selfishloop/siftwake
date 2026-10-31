//! Unlink only obsolete engine-owned files after a checked publication.
//! No source rewriting, recursion, guessed names, or fallback to older roots.
use crate::{
    ReclaimStats, Result,
    manifest::{self, Kind},
};
use std::{collections::BTreeSet, fs, path::Path};

fn owned(name: &str) -> Option<(Kind, u64, &str)> {
    let (kind, tail) = if let Some(tail) = name.strip_prefix("generation-") {
        (Kind::Legacy, tail)
    } else {
        (Kind::Run, name.strip_prefix("run-")?)
    };
    let (digits, suffix) = tail.split_once('.')?;
    if digits.len() != 16
        || !digits
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let id = u64::from_str_radix(digits, 16).ok().filter(|id| *id != 0)?;
    match suffix {
        "fdb" | "idx" | "records04.idx" | "vfy" | "svfy" | "rmp" => {}
        "ready" | "pending" if kind == Kind::Legacy => {}
        _ => return None,
    }
    Some((kind, id, suffix))
}

fn decimal(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}
fn temporary(name: &str) -> bool {
    if let Some(input) = name
        .strip_prefix("input-")
        .and_then(|s| s.strip_suffix(".tmp"))
    {
        return input
            .split_once('-')
            .is_some_and(|(pid, n)| decimal(pid) && decimal(n));
    }
    let Some((stem, suffix)) = name.split_once('.') else {
        return false;
    };
    // Reuse the exact lowercase engine source stem allowlist, never a prefix
    // glob that could eat user notes or a real .fdb/.idx/.ready file.
    if owned(&format!("{stem}.fdb")).is_none() {
        return false;
    }
    let Some(build) = suffix.strip_prefix("build-") else {
        return false;
    };
    let (build, spill) = build
        .split_once('.')
        .map_or((build, None), |(b, s)| (b, Some(s)));
    let build = build
        .strip_suffix("-records")
        .or_else(|| build.strip_suffix("-postings"))
        .unwrap_or(build);
    let parts: Vec<_> = build.split('-').collect();
    if parts.len() != 3 || !parts.iter().all(|s| decimal(s)) {
        return false;
    }
    spill.is_none_or(|s| {
        s.strip_prefix("spill-stage-")
            .or_else(|| s.strip_prefix("spill-"))
            .is_some_and(decimal)
    })
}

/// Call only during fully validated strict open under the exclusive DB lock,
/// before any worker/intake exists. A surviving snapshot holds that same lock,
/// so this cannot remove another live handle's scratch files.
pub(crate) fn collect_open_temporaries(dir: &Path) -> Result<ReclaimStats> {
    let mut stats = ReclaimStats::default();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_name().to_str().is_some_and(temporary) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        fs::remove_file(entry.path())?;
        stats.removed_files += 1;
        stats.reclaimed_bytes += metadata.len();
    }
    Ok(stats)
}

pub(crate) fn collect(
    dir: &Path,
    generation: u64,
    live: &BTreeSet<(Kind, u64)>,
) -> Result<ReclaimStats> {
    collect_with(dir, generation, live, |stage, _| {
        #[cfg(test)]
        if std::env::var("FDB_TEST_RECLAIM_STAGE").ok().as_deref() == Some(stage) {
            let marker = std::env::var_os("FDB_TEST_RECLAIM_MARKER").unwrap();
            fs::write(marker, stage)?;
            loop {
                std::thread::park();
            }
        }
        let _ = stage;
        Ok(())
    })
}

fn collect_with(
    dir: &Path,
    generation: u64,
    live: &BTreeSet<(Kind, u64)>,
    mut after: impl FnMut(&str, &ReclaimStats) -> Result<()>,
) -> Result<ReclaimStats> {
    let root = dir.canonicalize()?;
    let mut stats = ReclaimStats::default();
    // Retire old roots first. A failed/interrupted cleanup always leaves the
    // latest checked root in place, never a surviving old root selecting a
    // partially deleted generation. Two directory passes bound scratch memory.
    for markers in [true, false] {
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some((kind, id, suffix)) = owned(name) else {
                continue;
            };
            if id >= generation || (suffix == "ready") != markers {
                continue;
            }
            if !markers && live.contains(&(kind, id)) {
                continue;
            }
            let path = root.join(name);
            debug_assert_eq!(path.parent(), Some(root.as_path()));
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                continue;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    continue;
                }
            }
            fs::remove_file(&path)?;
            stats.removed_files += 1;
            stats.reclaimed_bytes += metadata.len();
            after(if markers { "marker" } else { "file" }, &stats)?;
        }
        manifest::sync_dir(&root)?;
        after(
            if markers {
                "markers_done"
            } else {
                "files_done"
            },
            &stats,
        )?;
    }
    // Unpublished IDs at/above the current root are left until the next
    // successful publication. This preserves the allocator high-water mark
    // across a close/reopen without a new durable format/counter.
    Ok(stats)
}

#[cfg(test)]
pub(crate) mod tests;
