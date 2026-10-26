use crate::format::{self, Batch, Change};
use crate::index::Index;
use crate::manifest::{self, Active, Kind, Manifest, Publication, Source};
use crate::segment::{Builder, DiskIndex};
use crate::{
    Error, Guard, Key, Mutation, Options, Query, Result, SearchPage, Stats, StoredDocument,
    validate_doc, validate_key,
};
pub(crate) mod maintenance;
mod revisions;
#[cfg(feature = "bounded-engine")]
mod streamed;
use std::cmp::Reverse;
#[cfg(any(
    feature = "experimental-grouped-read",
    not(all(
        feature = "experimental-shared-documents",
        not(feature = "experimental-wal-delta")
    ))
))]
use std::collections::BTreeMap;
use std::collections::{BTreeSet, BinaryHeap};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

#[cfg(all(
    feature = "experimental-shared-documents",
    not(feature = "experimental-wal-delta")
))]
type DeltaKey = Arc<Key>;
#[cfg(not(all(
    feature = "experimental-shared-documents",
    not(feature = "experimental-wal-delta")
)))]
type DeltaKey = Key;

fn remember_delta_key(shadow: &mut BTreeSet<DeltaKey>, key: &Key) -> DeltaKey {
    #[cfg(all(
        feature = "experimental-shared-documents",
        not(feature = "experimental-wal-delta")
    ))]
    {
        if let Some(existing) = shadow.get(key) {
            return existing.clone();
        }
        let key = Arc::new(key.clone());
        shadow.insert(key.clone());
        key
    }
    #[cfg(not(all(
        feature = "experimental-shared-documents",
        not(feature = "experimental-wal-delta")
    )))]
    {
        shadow.insert(key.clone());
        key.clone()
    }
}

fn own_delta_key(key: &DeltaKey) -> Key {
    #[cfg(all(
        feature = "experimental-shared-documents",
        not(feature = "experimental-wal-delta")
    ))]
    {
        key.as_ref().clone()
    }
    #[cfg(not(all(
        feature = "experimental-shared-documents",
        not(feature = "experimental-wal-delta")
    )))]
    {
        key.clone()
    }
}

#[derive(Clone)]
struct Run {
    source: Source,
    index: Arc<DiskIndex>,
    deferred: Option<Arc<std::sync::Mutex<bool>>>,
    source_records: Option<Arc<crate::source_records::Records>>,
}
struct Job {
    inputs: Vec<Source>,
    output_id: u64,
    handle: JoinHandle<Result<JobOutput>>,
    cancel: Option<Arc<maintenance::Control>>,
    revision: bool,
}
enum JobOutput {
    Replace(Run),
    Retire,
    Cancelled,
}

#[cfg(feature = "experimental-write-maintenance-slots")]
fn maintenance_workspace() -> Result<std::sync::MutexGuard<'static, ()>> {
    #[cfg(feature = "diagnostic-timing")]
    let _wait = crate::timing::span("maintenance.source_workspace_wait");
    // One source-frame staging workspace across foreground and merge workers.
    static WORKSPACE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    WORKSPACE
        .lock()
        .map_err(|_| Error::Invalid("maintenance workspace poisoned".into()))
}

#[cfg(feature = "experimental-write-maintenance-slots")]
fn builder_workspace() -> Result<std::sync::MutexGuard<'static, ()>> {
    #[cfg(feature = "diagnostic-timing")]
    let _wait = crate::timing::span("maintenance.builder_workspace_wait");
    static WORKSPACE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    WORKSPACE
        .lock()
        .map_err(|_| Error::Invalid("builder workspace poisoned".into()))
}

#[cfg(feature = "experimental-write-merge-policy")]
fn select_merge(sources: &[Source], automatic: bool) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize, usize)> = None;
    for start in 0..sources.len().saturating_sub(1) {
        let (mut keys, mut bytes, mut min, mut max) = (0usize, 0u64, usize::MAX, 0usize);
        for (local, source) in sources[start..].iter().take(4).enumerate() {
            keys = keys.checked_add(source.keys)?;
            bytes = bytes.checked_add(source.end)?;
            min = min.min(source.keys.max(1));
            max = max.max(source.keys.max(1));
            if keys > 100_000 || bytes > 64 * 1024 * 1024 || max > min.saturating_mul(4) {
                break;
            }
            let count = local + 1;
            // Automatic maintenance waits for four similarly sized inputs;
            // explicit maintenance can drain remaining smaller groups.
            if count < 2 || (automatic && (count != 4 || max > min.saturating_mul(2))) {
                continue;
            }
            let end = start + count;
            if best.is_none_or(|(a, b, cost)| {
                (keys as u128) * ((b - a - 1) as u128) < (cost as u128) * ((count - 1) as u128)
            }) {
                best = Some((start, end, keys));
            }
        }
    }
    best.map(|(start, end, _)| (start, end))
}

#[cfg(all(test, feature = "experimental-write-merge-policy"))]
mod merge_selection_tests {
    use super::*;
    fn sources(sizes: &[usize]) -> Vec<Source> {
        sizes
            .iter()
            .enumerate()
            .map(|(id, &keys)| Source {
                kind: Kind::Run,
                id: id as u64,
                end: (keys * 100) as u64,
                checksum: 0,
                sequence: id as u64,
                keys,
            })
            .collect()
    }
    #[test]
    fn automatic_merge_avoids_rewriting_large_run_and_explicit_merge_drains() {
        let runs = sources(&[40_000, 10_000, 10_000, 10_000]);
        assert_eq!(select_merge(&runs, true), None);
        assert_eq!(select_merge(&runs, false), Some((1, 4)));
        assert_eq!(
            select_merge(&sources(&[40_000, 10_000, 10_000, 10_000, 10_000]), true),
            Some((1, 5))
        );
        assert_eq!(
            select_merge(&sources(&[40_000, 40_000, 20_000]), false),
            Some((0, 3))
        );
        assert_eq!(select_merge(&sources(&[70_000, 70_000]), false), None);
        assert_eq!(select_merge(&[], false), None);
        let mut huge = sources(&[1, 1]);
        huge[0].end = 64 * 1024 * 1024;
        assert_eq!(select_merge(&huge, false), None);
    }
}

#[cfg(feature = "experimental-paged-metadata")]
#[derive(Default)]
struct MaskCache {
    entries: Vec<(u64, Box<[u32]>)>,
    bytes: usize,
}
#[cfg(feature = "experimental-paged-metadata")]
impl MaskCache {
    fn apply(&self, source: u64, mask: &mut [u64]) -> bool {
        if let Some((_, ids)) = self.entries.iter().find(|(id, _)| *id == source) {
            for &id in ids.iter() {
                mask[id as usize / 64] |= 1 << (id % 64);
            }
            true
        } else {
            false
        }
    }
    fn remember(&mut self, source: u64, mask: &[u64]) {
        if self.entries.len() >= 256 || self.entries.iter().any(|(id, _)| *id == source) {
            return;
        }
        let count: usize = mask.iter().map(|word| word.count_ones() as usize).sum();
        if count > (64 * 1024 - self.bytes) / 4 {
            return;
        }
        let mut ids = Vec::with_capacity(count);
        for (word, &bits) in mask.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                ids.push((word * 64 + bits.trailing_zeros() as usize) as u32);
                bits &= bits - 1;
            }
        }
        self.bytes += count * 4;
        self.entries.push((source, ids.into_boxed_slice()));
    }
}

/// One exclusive owner. Background merge workers never publish manifests.
pub struct Database {
    dir: PathBuf,
    _lock: File,
    log: File,
    active: Active,
    generation: u64,
    next_id: u64,
    sequence: u64,
    count: usize,
    runs: Vec<Run>,
    retired: Vec<(Source, std::sync::Weak<DiskIndex>)>,
    #[cfg(not(any(
        feature = "experimental-wal-delta",
        feature = "experimental-shared-documents"
    )))]
    pub(crate) docs: BTreeMap<Key, StoredDocument>,
    #[cfg(feature = "experimental-wal-delta")]
    pub(crate) docs: BTreeMap<Key, crate::wal_delta::Row>,
    #[cfg(all(
        feature = "experimental-shared-documents",
        not(feature = "experimental-wal-delta")
    ))]
    pub(crate) docs: std::collections::HashMap<DeltaKey, crate::shared_documents::Row>,
    #[cfg(all(
        feature = "experimental-shared-documents",
        not(feature = "experimental-wal-delta")
    ))]
    doc_cache: crate::shared_documents::Cache,
    pub(crate) index: Index,
    shadow: BTreeSet<DeltaKey>,
    delta_bytes: usize,
    options: Options,
    job: Option<Job>,
    revision_reclamation: bool,
    revision_responsive: bool,
    query_visibility_pruning: bool,
    maintenance_activity: Arc<maintenance::Activity>,
    poisoned: bool,
    #[cfg(feature = "experimental-wal-sync-control")]
    sync_wal: bool,
    #[cfg(feature = "experimental-paged-metadata")]
    mask_cache: std::sync::Mutex<MaskCache>,
}

fn open_source(dir: &Path, mut source: Source, legacy_unknown: bool) -> Result<Run> {
    #[cfg(feature = "diagnostic-timing")]
    let _open = crate::timing::maintenance::span("index_rewrite_open");
    #[cfg(feature = "bounded-engine")]
    let mut file =
        crate::segment::SourceLease::open(&source.path(dir, "fdb"), source.kind == Kind::Run)?;
    #[cfg(not(feature = "bounded-engine"))]
    let mut file = File::open(source.path(dir, "fdb"))?;
    if source.kind == Kind::Run && file.metadata()?.len() != source.end {
        return Err(Error::Corrupt("immutable source length mismatch".into()));
    }
    #[cfg(feature = "experimental-streaming-open")]
    let checked_sequence = if source.kind == Kind::Run && !legacy_unknown {
        let mut magic = [0; 8];
        crate::segment::read_at(&file, 0, &mut magic)?;
        if &magic == format::MAGIC_V4 {
            let (sequence, count) =
                format::check_native_prefix(&mut file, source.end, source.checksum)?;
            if sequence != source.sequence || count != source.keys {
                return Err(Error::Corrupt("manifest/source identity mismatch".into()));
            }
            Some(sequence)
        } else {
            None
        }
    } else {
        None
    };
    #[cfg(feature = "experimental-streaming-open")]
    let already_checked = checked_sequence.is_some();
    #[cfg(not(feature = "experimental-streaming-open"))]
    let already_checked = false;
    let checksum =
        if !already_checked && (legacy_unknown || cfg!(feature = "experimental-streaming-open")) {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("open.source_crc");
            #[cfg(feature = "diagnostic-timing")]
            let _stage = crate::timing::maintenance::span("source_crc_verify");
            let checksum = format::prefix_crc(&mut file, source.end)?;
            if !legacy_unknown && checksum != source.checksum {
                return Err(Error::Corrupt("source checkpoint checksum mismatch".into()));
            }
            checksum
        } else {
            source.checksum
        };
    source.checksum = checksum;
    let target = source.path(dir, "idx");
    let opened_index = {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("open.index");
        DiskIndex::open(&target, Some((source.end, checksum)))
    };
    let mut index = match opened_index {
        Ok(index) => Some(index),
        Err(Error::Corrupt(_)) => None,
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let mut builder = None;
    #[cfg(feature = "bounded-engine")]
    let rebuilt_sequence = if index.is_none() && source.kind == Kind::Run {
        let mut magic = [0; 8];
        crate::segment::read_at(&file, 0, &mut magic)?;
        if &magic == format::MAGIC_V4 {
            let (built, sequence) =
                Builder::from_native(&target, &mut file, source.end, checksum, true)?;
            builder = Some(built);
            Some(sequence)
        } else {
            None
        }
    } else {
        None
    };
    #[cfg(all(
        feature = "experimental-streaming-open",
        not(feature = "bounded-engine")
    ))]
    let rebuilt_sequence = None;
    #[cfg(feature = "diagnostic-timing")]
    let replay_start = crate::timing::span("open.source_replay");
    #[cfg(not(feature = "experimental-streaming-open"))]
    let sequence = format::replay_prefix_checked(&mut file, source.end, checksum, |seq, batch| {
        if !batch.checkpoint {
            return Err(Error::Corrupt(
                "normal transaction inside immutable source".into(),
            ));
        }
        if index.as_ref().is_some_and(|i| i.sequence != seq) {
            return Err(Error::Corrupt("source/index sequence mismatch".into()));
        }
        if index.is_none() && builder.is_none() {
            builder = Some(Builder::new(&target, seq, source.end, checksum)?);
        }
        for change in batch.changes {
            match change {
                Change::Put { stored } => {
                    validate_doc(&stored.document).map_err(|e| Error::Corrupt(e.to_string()))?;
                    if stored.version == 0 || stored.version > seq {
                        return Err(Error::Corrupt("invalid checkpoint version".into()));
                    }
                    if let Some(b) = &mut builder {
                        b.add(&stored)?;
                    }
                }
                Change::Delete { key } => {
                    if source.kind == Kind::Legacy {
                        return Err(Error::Corrupt("legacy checkpoint tombstone".into()));
                    }
                    validate_key(&key).map_err(|e| Error::Corrupt(e.to_string()))?;
                    if let Some(b) = &mut builder {
                        b.add_delete(&key)?;
                    }
                }
            }
        }
        Ok(())
    })?;
    #[cfg(feature = "experimental-streaming-open")]
    let sequence = if let Some(sequence) = checked_sequence
        .filter(|_| index.is_some())
        .or(rebuilt_sequence)
    {
        sequence
    } else {
        format::replay_prefix_changes(&mut file, source.end, |seq, change| {
            if index.as_ref().is_some_and(|i| i.sequence != seq) {
                return Err(Error::Corrupt("source/index sequence mismatch".into()));
            }
            if index.is_none() && builder.is_none() {
                builder = Some(Builder::new(&target, seq, source.end, checksum)?);
            }
            if let Some(change) = change {
                match change {
                    Change::Put { stored } => {
                        validate_doc(&stored.document)
                            .map_err(|e| Error::Corrupt(e.to_string()))?;
                        if stored.version == 0 || stored.version > seq {
                            return Err(Error::Corrupt("invalid checkpoint version".into()));
                        }
                        if let Some(b) = &mut builder {
                            b.add(&stored)?;
                        }
                    }
                    Change::Delete { key } => {
                        if source.kind == Kind::Legacy {
                            return Err(Error::Corrupt("legacy checkpoint tombstone".into()));
                        }
                        validate_key(&key).map_err(|e| Error::Corrupt(e.to_string()))?;
                        if let Some(b) = &mut builder {
                            b.add_delete(&key)?;
                        }
                    }
                }
            }
            Ok(())
        })?
    };
    #[cfg(feature = "diagnostic-timing")]
    drop(replay_start);
    if let Some(b) = builder {
        index = Some(b.finish()?);
    }
    let index = index.ok_or_else(|| Error::Corrupt("missing checkpoint index".into()))?;
    if index.sequence != sequence {
        return Err(Error::Corrupt("source/index sequence mismatch".into()));
    }
    if !legacy_unknown && (index.len() != source.keys || sequence != source.sequence) {
        return Err(Error::Corrupt("manifest/source identity mismatch".into()));
    }
    #[cfg(feature = "experimental-paged-metadata")]
    let index = {
        let mut index = index;
        index.enable_paging(&target)?;
        index
    };
    source.sequence = sequence;
    source.keys = index.len();
    #[cfg(feature = "bounded-engine")]
    let index = {
        let mut index = index;
        index.bind_text_source(source.path(dir, "fdb"), file)?;
        index
    };
    Ok(Run {
        source,
        index: Arc::new(index),
        deferred: None,
        source_records: None,
    })
}

#[cfg(all(test, not(feature = "experimental-streaming-open")))]
mod deferred_validation_tests {
    use super::*;

    #[test]
    fn borrowed_check_rejects_crc_valid_invalid_documents_and_manifest_identity() {
        let dir = std::env::temp_dir().join(format!("fdb-borrowed-limits-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let valid = StoredDocument {
            document: crate::Document {
                key: Key::new("shared", "texts", "a"),
                title: "title".into(),
                text: "text".into(),
                fields: Default::default(),
                links: vec![],
            },
            version: 7,
        };
        let mut cases = vec![(valid.clone(), true)];
        for version in [0, 8] {
            let mut stored = valid.clone();
            stored.version = version;
            cases.push((stored, false));
        }
        let mut stored = valid.clone();
        stored.document.key.id = "bad\n".into();
        cases.push((stored, false));
        let mut stored = valid.clone();
        stored
            .document
            .fields
            .insert("".into(), serde_json::Value::Null);
        cases.push((stored, false));
        let mut stored = valid;
        stored.document.links.push(Key::new("other", "texts", "b"));
        cases.push((stored, false));
        for (stored, expected) in cases {
            let batch = Batch {
                checkpoint: true,
                changes: vec![Change::Put { stored }],
            };
            let mut bytes = format::MAGIC_V4.to_vec();
            bytes.extend(format::encode(7, &batch).unwrap());
            let source = Source {
                kind: Kind::Run,
                id: 1,
                end: bytes.len() as u64,
                checksum: format::crc(&bytes),
                sequence: 7,
                keys: 1,
            };
            std::fs::write(source.path(&dir, "fdb"), bytes).unwrap();
            let mut file = File::open(source.path(&dir, "fdb")).unwrap();
            let owned = format::replay_prefix_checked(
                &mut file,
                source.end,
                source.checksum,
                |seq, batch| {
                    for change in batch.changes {
                        if let Change::Put { stored } = change {
                            validate_doc(&stored.document)?;
                            if stored.version == 0 || stored.version > seq {
                                return Err(Error::Corrupt("version".into()));
                            }
                        }
                    }
                    Ok(())
                },
            );
            assert_eq!(owned.is_ok(), expected);
            assert_eq!(verify_deferred_source(&dir, &source).is_ok(), expected);
            let mut wrong = source.clone();
            wrong.keys += 1;
            assert!(verify_deferred_source(&dir, &wrong).is_err());
            wrong = source;
            wrong.sequence += 1;
            assert!(verify_deferred_source(&dir, &wrong).is_err());
        }
        std::fs::remove_file(dir.join("run-0000000000000001.fdb")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}

fn verify_deferred_source(dir: &Path, source: &Source) -> Result<()> {
    #[cfg(feature = "diagnostic-timing")]
    let _verify = crate::timing::span("source.verify_total");
    let mut file = File::open(source.path(dir, "fdb"))?;
    if file.metadata()?.len() != source.end {
        return Err(Error::Corrupt("immutable source length mismatch".into()));
    }
    let mut count = 0usize;
    #[cfg(feature = "experimental-streaming-open")]
    let mut check_change = |seq: u64, change: Change| -> Result<()> {
        #[cfg(feature = "diagnostic-timing")]
        let _validate = crate::timing::span("source.validate_document");
        count = count
            .checked_add(1)
            .ok_or_else(|| Error::Corrupt("source record count overflow".into()))?;
        match change {
            Change::Put { stored } => {
                validate_doc(&stored.document).map_err(|e| Error::Corrupt(e.to_string()))?;
                if stored.version == 0 || stored.version > seq {
                    return Err(Error::Corrupt("invalid checkpoint version".into()));
                }
            }
            Change::Delete { key } => {
                validate_key(&key).map_err(|e| Error::Corrupt(e.to_string()))?
            }
        }
        Ok(())
    };
    #[cfg(not(feature = "experimental-streaming-open"))]
    let sequence =
        format::validate_native_prefix(&mut file, source.end, source.checksum, |seq, bytes| {
            #[cfg(feature = "diagnostic-timing")]
            let _validate = crate::timing::span("source.validate_borrowed_batch");
            let (_, entries) = crate::binary_batch::entries(bytes)?;
            for (tag, _, bytes) in entries {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| Error::Corrupt("source record count overflow".into()))?;
                if tag == 0 {
                    let view = crate::payload::checked(bytes)?;
                    view.validate_document(bytes.len())
                        .map_err(|e| Error::Corrupt(e.to_string()))?;
                    if view.version() == 0 || view.version() > seq {
                        return Err(Error::Corrupt("invalid checkpoint version".into()));
                    }
                } else if let Change::Delete { key } =
                    crate::binary_batch::decode_entry(tag, bytes)?
                {
                    validate_key(&key).map_err(|e| Error::Corrupt(e.to_string()))?;
                }
            }
            Ok(())
        })?;
    #[cfg(feature = "experimental-streaming-open")]
    let sequence = {
        if format::prefix_crc(&mut file, source.end)? != source.checksum {
            return Err(Error::Corrupt("source checkpoint checksum mismatch".into()));
        }
        format::replay_prefix_changes(&mut file, source.end, |seq, change| {
            if let Some(change) = change {
                check_change(seq, change)?;
            }
            Ok(())
        })?
    };
    if count != source.keys || sequence != source.sequence {
        return Err(Error::Corrupt("manifest/source identity mismatch".into()));
    }
    Ok(())
}

// Existing cache CRC/grammar is checked eagerly. Authoritative source validation
// is deferred only for current immutable runs; unsupported/missing cache falls back.
fn open_source_deferred(dir: &Path, source: Source) -> Result<Run> {
    if source.kind != Kind::Run {
        return open_source(dir, source, false);
    }
    #[cfg(feature = "bounded-engine")]
    let file = crate::segment::SourceLease::open(&source.path(dir, "fdb"), true)?;
    #[cfg(not(feature = "bounded-engine"))]
    let file = File::open(source.path(dir, "fdb"))?;
    if file.metadata()?.len() != source.end {
        return Err(Error::Corrupt("immutable source length mismatch".into()));
    }
    let mut magic = [0; 8];
    crate::segment::read_at(&file, 0, &mut magic)?;
    if &magic != format::MAGIC_V4 {
        return open_source(dir, source, false);
    }
    let index = match DiskIndex::open_deferred(
        &source.path(dir, "idx"),
        Some((source.end, source.checksum)),
    ) {
        Ok(index) => index,
        Err(Error::Corrupt(_)) => return open_source(dir, source, false),
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return open_source(dir, source, false);
        }
        Err(e) => return Err(e),
    };
    if index.len() != source.keys || index.sequence != source.sequence {
        return Err(Error::Corrupt("manifest/index identity mismatch".into()));
    }
    #[cfg(feature = "bounded-engine")]
    let index = {
        let mut index = index;
        index.bind_text_source(source.path(dir, "fdb"), file)?;
        index
    };
    Ok(Run {
        source,
        index: Arc::new(index),
        deferred: Some(Arc::new(std::sync::Mutex::new(false))),
        source_records: None,
    })
}

#[cfg(not(feature = "experimental-merge-postings"))]
fn write_run(
    dir: &Path,
    id: u64,
    sequence: u64,
    changes: impl Iterator<Item = Result<Change>>,
) -> Result<Run> {
    #[cfg(feature = "diagnostic-timing")]
    let _run_stage = crate::timing::maintenance::span("index_rewrite_open");
    write_run_prepared(dir, id, sequence, changes, |_| Ok(()))
}

#[cfg(feature = "experimental-merge-postings")]
fn reused_builder(dir: &Path, source: &Source) -> Result<Builder> {
    let mut file = File::open(source.path(dir, "fdb"))?;
    #[cfg(feature = "bounded-engine")]
    let (builder, found) = Builder::from_native(
        &source.path(dir, "idx"),
        &mut file,
        source.end,
        source.checksum,
        false,
    )?;
    #[cfg(not(feature = "bounded-engine"))]
    let mut builder = Builder::new(
        &source.path(dir, "idx"),
        source.sequence,
        source.end,
        source.checksum,
    )?;
    #[cfg(all(
        feature = "experimental-streaming-open",
        not(feature = "bounded-engine")
    ))]
    let found = format::replay_prefix_changes(&mut file, source.end, |seq, change| {
        if seq != source.sequence {
            return Err(Error::Corrupt("reused source sequence".into()));
        }
        match change {
            Some(Change::Put { stored }) => builder.add_reused(&stored)?,
            Some(Change::Delete { key }) => builder.add_delete(&key)?,
            None => {}
        }
        Ok(())
    })?;
    #[cfg(not(feature = "experimental-streaming-open"))]
    let found = format::replay_prefix(&mut file, source.end, |seq, batch| {
        if seq != source.sequence || !batch.checkpoint {
            return Err(Error::Corrupt("reused source sequence/checkpoint".into()));
        }
        for change in batch.changes {
            match change {
                Change::Put { stored } => builder.add_reused(&stored)?,
                Change::Delete { key } => builder.add_delete(&key)?,
            }
        }
        Ok(())
    })?;
    if found != source.sequence {
        return Err(Error::Corrupt("reused source sequence".into()));
    }
    Ok(builder)
}

fn write_run_prepared(
    dir: &Path,
    id: u64,
    sequence: u64,
    changes: impl Iterator<Item = Result<Change>>,
    prepare: impl FnOnce(&Source) -> Result<()>,
) -> Result<Run> {
    write_run_ready(dir, id, sequence, changes, |source| {
        prepare(source)?;
        Ok(None)
    })
}

fn write_run_ready(
    dir: &Path,
    id: u64,
    sequence: u64,
    changes: impl Iterator<Item = Result<Change>>,
    prepare: impl FnOnce(&Source) -> Result<Option<Run>>,
) -> Result<Run> {
    #[cfg(feature = "diagnostic-timing")]
    let _run_stage = crate::timing::maintenance::span("index_rewrite_open");
    #[cfg(feature = "experimental-write-maintenance-slots")]
    let _workspace = maintenance_workspace()?;
    #[cfg(feature = "diagnostic-timing")]
    let _run = crate::timing::span("run.write_source_rebuild");
    let mut source = Source {
        kind: Kind::Run,
        id,
        end: 0,
        checksum: 0,
        sequence,
        keys: 0,
    };
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(source.path(dir, "fdb"))?;
    file.write_all(format::MAGIC_V4)?;
    #[cfg(feature = "diagnostic-timing")]
    let copy = crate::timing::maintenance::span("source_copy_encode");
    #[cfg(feature = "diagnostic-memory")]
    let _origin = crate::memory::Scope::new(4);
    #[cfg(feature = "experimental-write-buffer-budget")]
    format::append_checkpoint_changes(&mut file, sequence, changes)?;
    #[cfg(not(feature = "experimental-write-buffer-budget"))]
    {
        let mut batch = vec![];
        let mut size = 0;
        for change in changes {
            let change = change?;
            size += crate::binary_batch::entry_len(&change)?;
            batch.push(change);
            if size >= 8 * 1024 * 1024 {
                format::append(
                    &mut file,
                    &format::encode(
                        sequence,
                        &Batch {
                            checkpoint: true,
                            changes: std::mem::take(&mut batch),
                        },
                    )?,
                )?;
                size = 0;
            }
        }
        format::append(
            &mut file,
            &format::encode(
                sequence,
                &Batch {
                    checkpoint: true,
                    changes: batch,
                },
            )?,
        )?;
    }
    #[cfg(feature = "diagnostic-timing")]
    drop(copy);
    source.end = file.metadata()?.len();
    source.checksum = {
        #[cfg(feature = "diagnostic-timing")]
        let _crc = crate::timing::maintenance::span("source_crc_verify");
        format::prefix_crc(&mut file, source.end)?
    };
    // Finish the writer lifetime before binding the immutable source fingerprint.
    #[cfg(feature = "bounded-engine")]
    drop(file);
    #[cfg(feature = "experimental-write-maintenance-slots")]
    drop(_workspace);
    // The large source Batch/encode buffers have been dropped before waiting.
    // A single source writer and a single builder may overlap, never two of either.
    #[cfg(feature = "bounded-engine")]
    let _builder_working = crate::write_budget::WorkingScope::builder()?;
    #[cfg(feature = "experimental-write-maintenance-slots")]
    let _builder_workspace = builder_workspace()?;
    // Authoritative source is complete before its derived index is constructed.
    if let Some(run) = prepare(&source)? {
        return Ok(run);
    }
    open_source(dir, source, true)
}

struct MergeKeys<'a> {
    runs: &'a [Run],
    heap: BinaryHeap<Reverse<(Key, usize, u32)>>,
}
impl<'a> MergeKeys<'a> {
    fn new(runs: &'a [Run]) -> Result<Self> {
        let mut heap = BinaryHeap::new();
        for (n, run) in runs.iter().enumerate() {
            if run.index.len() != 0 {
                heap.push(Reverse((run.index.key(0)?, n, 0)));
            }
        }
        Ok(Self { runs, heap })
    }
}
impl Iterator for MergeKeys<'_> {
    type Item = Result<(usize, u32)>;
    fn next(&mut self) -> Option<Self::Item> {
        let Reverse((key, n, id)) = self.heap.pop()?;
        let mut same = vec![(n, id)];
        while self
            .heap
            .peek()
            .is_some_and(|Reverse((next, _, _))| next == &key)
        {
            let Reverse((_, n, id)) = self.heap.pop().unwrap();
            same.push((n, id));
        }
        let selected = *same.iter().max_by_key(|(n, _)| n).unwrap();
        for (n, id) in same {
            let next = id + 1;
            if (next as usize) < self.runs[n].index.len() {
                let key = match self.runs[n].index.key(next) {
                    Ok(key) => key,
                    Err(e) => {
                        self.heap.clear();
                        return Some(Err(e));
                    }
                };
                self.heap.push(Reverse((key, n, next)));
            }
        }
        Some(Ok(selected))
    }
}

impl Database {
    /// Diagnostic capacities only; does not include total process memory.
    pub fn index_memory_layout(&self) -> Result<Vec<crate::IndexMemoryLayout>> {
        self.healthy()?;
        Ok(self
            .runs
            .iter()
            .map(|run| run.index.memory_layout(run.source.id))
            .collect())
    }
    /// Explicit, disposable single-segment benchmark overlay; never enabled by open.
    #[cfg(feature = "experimental-rkyv-payload")]
    pub fn attach_payload_benchmark(
        &mut self,
        path: impl AsRef<Path>,
        codec: crate::rkyv_payload::bench::Codec,
    ) -> Result<u64> {
        self.healthy()?;
        if self.runs.len() != 1
            || !self.docs.is_empty()
            || !self.shadow.is_empty()
            || self.job.is_some()
        {
            return Err(Error::Invalid(
                "payload benchmark requires one immutable segment and empty delta".into(),
            ));
        }
        let index = Arc::get_mut(&mut self.runs[0].index).ok_or_else(|| {
            Error::Invalid("payload benchmark requires exclusive segment ownership".into())
        })?;
        index.attach_payload_benchmark(path.as_ref(), codec)
    }
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(dir, Options::default())
    }
    /// Opt-in segment-level deferred source validation. Indexes and active WAL
    /// are checked at open; immutable sources are checked before their first use.
    /// Opening successfully does not imply a full integrity check has completed.
    pub fn open_deferred(dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_mode(dir, Options::default(), true, false)
    }
    pub fn open_with_options(dir: impl AsRef<Path>, options: Options) -> Result<Self> {
        Self::open_mode(dir, options, false, false)
    }
    /// Opt-in checked source-record reads. Search, negative lookups and writes
    /// retain complete source verification. Missing/stale address caches fall back.
    pub fn open_record_reads(dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_mode(dir, Options::default(), true, true)
    }
    fn open_mode(
        dir: impl AsRef<Path>,
        options: Options,
        deferred: bool,
        record_reads: bool,
    ) -> Result<Self> {
        #[cfg(feature = "diagnostic-timing")]
        let _open = crate::timing::span("open.total");
        if options.max_delta_keys == 0 || options.max_delta_bytes == 0 {
            return Err(Error::Invalid("delta watermarks must be positive".into()));
        }
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        // Keep all publication/reclamation paths under the same resolved root
        // for which the database lock is acquired.
        let dir = dir.canonicalize()?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("LOCK"))?;
        lock.try_lock().map_err(|e| match e {
            std::fs::TryLockError::WouldBlock => Error::Locked,
            std::fs::TryLockError::Error(e) => Error::Io(e),
        })?;
        let (latest, maximum) = manifest::inventory(&dir)?;
        let mut next_id = maximum
            .checked_add(1)
            .ok_or_else(|| Error::Invalid("generation overflow".into()))?;
        let (generation, publication) = if let Some(id) = latest {
            (id, manifest::read(&dir, id)?)
        } else {
            if maximum != 0 {
                return Err(Error::Corrupt(
                    "source files exist without publication marker".into(),
                ));
            }
            let id = next_id;
            next_id += 1;
            let (_, active) = manifest::new_active(&dir, id, 0)?;
            let m = Manifest {
                version: 3,
                runs: vec![],
                active,
            };
            manifest::publish(&dir, id, &m)?;
            (id, Publication::Current(m))
        };
        let (runs, active, legacy) = match publication {
            Publication::Legacy { end } => {
                let run = open_source(
                    &dir,
                    Source {
                        kind: Kind::Legacy,
                        id: generation,
                        end,
                        checksum: 0,
                        sequence: 0,
                        keys: 0,
                    },
                    true,
                )?;
                let active = Active {
                    id: generation,
                    end,
                    sequence: run.source.sequence,
                };
                (vec![run], active, true)
            }
            Publication::Current(m) => {
                let runs = m
                    .runs
                    .into_iter()
                    .map(|s| {
                        if deferred {
                            let mut run = open_source_deferred(&dir, s)?;
                            if record_reads && run.deferred.is_some() {
                                run.source_records = crate::source_records::Records::load(
                                    &dir,
                                    &run.source,
                                    &run.index,
                                )
                                .ok()
                                .map(Arc::new);
                            }
                            Ok(run)
                        } else {
                            open_source(&dir, s, false)
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                (runs, m.active, false)
            }
        };
        let mut log = OpenOptions::new()
            .read(true)
            .write(true)
            .open(manifest::path(&dir, active.id, "fdb"))?;
        #[cfg(not(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        )))]
        let mut docs = BTreeMap::new();
        #[cfg(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        ))]
        let mut docs = std::collections::HashMap::new();
        #[cfg(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        ))]
        let mut doc_cache = crate::shared_documents::Cache::default();
        let mut shadow = BTreeSet::new();
        #[cfg(not(feature = "experimental-wal-delta"))]
        let sequence = format::replay(&mut log, active.end, |seq, batch| {
            if batch.checkpoint {
                if seq != active.sequence || (!legacy && !batch.changes.is_empty()) {
                    return Err(Error::Corrupt("invalid active WAL checkpoint".into()));
                }
                return Ok(());
            }
            for change in batch.changes {
                match change {
                    Change::Put { stored } => {
                        validate_doc(&stored.document)
                            .map_err(|e| Error::Corrupt(e.to_string()))?;
                        if stored.version != seq {
                            return Err(Error::Corrupt("invalid WAL document version".into()));
                        }
                        let shared_key = remember_delta_key(&mut shadow, &stored.document.key);
                        #[cfg(not(any(
                            feature = "experimental-wal-delta",
                            feature = "experimental-shared-documents"
                        )))]
                        docs.insert(shared_key, stored);
                        #[cfg(all(
                            feature = "experimental-shared-documents",
                            not(feature = "experimental-wal-delta")
                        ))]
                        {
                            let size = crate::payload::encoded_len(&stored.document)?;
                            let (_, row) = doc_cache.store(stored, size)?;
                            docs.insert(shared_key, row);
                        }
                    }
                    Change::Delete { key } => {
                        validate_key(&key).map_err(|e| Error::Corrupt(e.to_string()))?;
                        remember_delta_key(&mut shadow, &key);
                        docs.remove(&key);
                    }
                }
            }
            Ok(())
        })?;
        #[cfg(feature = "experimental-wal-delta")]
        let sequence = format::replay_located(&mut log, active.end, |seq, batch, locations| {
            if batch.checkpoint {
                if seq != active.sequence || (!legacy && !batch.changes.is_empty()) {
                    return Err(Error::Corrupt("invalid active WAL checkpoint".into()));
                }
                return Ok(());
            }
            for (change, location) in batch.changes.into_iter().zip(locations) {
                match change {
                    Change::Put { stored } => {
                        validate_doc(&stored.document)
                            .map_err(|e| Error::Corrupt(e.to_string()))?;
                        if stored.version != seq {
                            return Err(Error::Corrupt("invalid WAL document version".into()));
                        }
                        let shared_key = remember_delta_key(&mut shadow, &stored.document.key);
                        docs.insert(
                            shared_key,
                            crate::wal_delta::Row {
                                version: stored.version,
                                location: location.ok_or_else(|| {
                                    Error::Corrupt("missing delta location".into())
                                })?,
                            },
                        );
                    }
                    Change::Delete { key } => {
                        validate_key(&key).map_err(|e| Error::Corrupt(e.to_string()))?;
                        remember_delta_key(&mut shadow, &key);
                        docs.remove(&key);
                    }
                }
            }
            Ok(())
        })?;
        let mut count = docs.len();
        #[cfg(feature = "experimental-paged-metadata")]
        let mut mask_cache = MaskCache::default();
        #[cfg(not(feature = "experimental-paged-metadata"))]
        for item in MergeKeys::new(&runs)? {
            let (n, id) = item?;
            if !runs[n].index.deleted(id)
                && (shadow.is_empty() || !shadow.contains(&runs[n].index.key(id)?))
            {
                count += 1;
            }
        }
        #[cfg(feature = "experimental-paged-metadata")]
        for (n, run) in runs.iter().enumerate() {
            let mut mask = vec![0u64; run.index.len().div_ceil(64)];
            for newer in &runs[n + 1..] {
                run.index.hide_keys(&newer.index, &mut mask)?;
            }
            mask_cache.remember(run.source.id, &mask);
            for key in &shadow {
                if let Some(id) = run.index.find(key)? {
                    mask[id as usize / 64] |= 1 << (id % 64);
                }
            }
            count += run.index.visible_count(&mask)?;
        }
        let mut index = Index::default();
        let mut delta_bytes = 0;
        #[cfg(not(any(
            feature = "experimental-wal-delta",
            feature = "experimental-shared-documents"
        )))]
        for stored in docs.values() {
            index.change(stored, true)?;
            delta_bytes += crate::payload::encoded_len(&stored.document)?;
        }
        #[cfg(feature = "experimental-wal-delta")]
        for (key, row) in &docs {
            let stored = row.read(&log, key)?;
            index.change(&stored, true)?;
            delta_bytes += row.location.len as usize;
        }
        #[cfg(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        ))]
        for (key, row) in &docs {
            let stored = row.to_stored(key);
            index.change_shared(&stored, true, key.clone())?;
            delta_bytes += row.serialized_len;
        }
        let mut db = Self {
            dir,
            _lock: lock,
            log,
            active,
            generation,
            next_id,
            sequence,
            count,
            runs,
            retired: Vec::new(),
            docs,
            #[cfg(all(
                feature = "experimental-shared-documents",
                not(feature = "experimental-wal-delta")
            ))]
            doc_cache,
            index,
            shadow,
            delta_bytes,
            options,
            job: None,
            // The measured prototype retains its disk win, but its read-p95
            // acceptance gate is still open. Explicit opt-in until resolved.
            revision_reclamation: false,
            revision_responsive: true,
            query_visibility_pruning: false,
            maintenance_activity: Arc::default(),
            poisoned: false,
            #[cfg(feature = "experimental-wal-sync-control")]
            sync_wal: true,
            #[cfg(feature = "experimental-paged-metadata")]
            mask_cache: std::sync::Mutex::new(mask_cache),
        };
        if db.needs_flush() {
            db.flush()?;
        }
        if !deferred {
            db.try_reclaim();
        }
        Ok(db)
    }
    fn ensure_run(&self, run: &Run) -> Result<()> {
        if let Some(state) = &run.deferred {
            let mut checked = state
                .lock()
                .map_err(|_| Error::Corrupt("source verification mutex poisoned".into()))?;
            if !*checked {
                verify_deferred_source(&self.dir, &run.source)?;
                *checked = true;
            }
        }
        #[cfg(feature = "bounded-engine")]
        run.index.authorize_text_covering();
        Ok(())
    }
    fn ensure_sources(&self) -> Result<()> {
        for run in &self.runs {
            self.ensure_run(run)?;
        }
        Ok(())
    }
    /// Explicitly prepare disposable VFY01 index block checks after full integrity verification.
    /// No source/index payloads or publication markers are changed.
    pub fn prepare_open_checks(&self) -> Result<()> {
        self.check_integrity()?;
        for run in &self.runs {
            run.index
                .prepare_open_checks(&run.source.path(&self.dir, "idx"))?;
        }
        Ok(())
    }
    /// Explicitly prepare REF01 address maps and VFY01 source block checks.
    pub fn prepare_record_reads(&self) -> Result<()> {
        self.check_integrity()?;
        for run in &self.runs {
            crate::source_records::Records::prepare(&self.dir, &run.source, &run.index)?;
        }
        Ok(())
    }
    /// Explicit SEG05 cache experiment, consuming this handle before callers
    /// reopen it. Normal source writes and index builders still emit SEG04.
    #[cfg(feature = "bounded-engine")]
    pub fn repack_index_cache(self, layout: crate::IndexCacheLayout) -> Result<()> {
        self.healthy()?;
        if self.job.is_some()
            || !self.shadow.is_empty()
            || self.runs.iter().any(|r| r.source.kind != Kind::Run)
        {
            return Err(Error::Invalid(
                "repack requires idle native immutable runs".into(),
            ));
        }
        self.ensure_sources()?;
        for run in &self.runs {
            run.index
                .repack(&run.source.path(&self.dir, "idx"), layout)?;
        }
        Ok(())
    }
    /// Number of immutable sources still awaiting their first full verification.
    /// Stats/count are provisional until these sources have been verified.
    pub fn pending_source_checks(&self) -> Result<usize> {
        self.healthy()?;
        let mut count = 0;
        for run in &self.runs {
            if let Some(state) = &run.deferred {
                count +=
                    usize::from(!*state.lock().map_err(|_| {
                        Error::Corrupt("source verification mutex poisoned".into())
                    })?);
            }
        }
        Ok(count)
    }
    /// Explicit full integrity pass, including sources verified earlier this session.
    /// Corrupt/missing derived indexes may be rebuilt; source bytes are never repaired.
    pub fn check_integrity(&self) -> Result<()> {
        self.healthy()?;
        for run in &self.runs {
            let _verified = open_source(&self.dir, run.source.clone(), false)?;
            if let Some(state) = &run.deferred {
                *state
                    .lock()
                    .map_err(|_| Error::Corrupt("source verification mutex poisoned".into()))? =
                    true;
            }
        }
        let mut log = self.log.try_clone()?;
        let sequence = format::check_log(&mut log, self.active.end, |seq, batch| {
            for change in batch.changes {
                match change {
                    Change::Put { stored } => {
                        validate_doc(&stored.document)
                            .map_err(|e| Error::Corrupt(e.to_string()))?;
                        if stored.version == 0 || stored.version > seq {
                            return Err(Error::Corrupt("invalid WAL version".into()));
                        }
                    }
                    Change::Delete { key } => {
                        validate_key(&key).map_err(|e| Error::Corrupt(e.to_string()))?
                    }
                }
            }
            Ok(())
        })?;
        if sequence != self.sequence {
            return Err(Error::Corrupt("WAL sequence mismatch".into()));
        }
        Ok(())
    }
    fn healthy(&self) -> Result<()> {
        if self.poisoned {
            Err(Error::Poisoned)
        } else {
            Ok(())
        }
    }
    fn allocate(&mut self) -> Result<u64> {
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or_else(|| Error::Invalid("generation overflow".into()))?;
        Ok(id)
    }
    /// Experimental commit-sync control. WAL bytes and framing remain enabled.
    /// With sync disabled, acknowledged writes can be lost on system/power failure.
    /// Reopening restores synced mode. Re-enabling sync first syncs pending WAL
    /// bytes; checkpoint and manifest publication are always synced.
    #[cfg(feature = "experimental-wal-sync-control")]
    pub fn set_wal_sync(&mut self, enabled: bool) -> Result<()> {
        self.healthy()?;
        if enabled
            && !self.sync_wal
            && let Err(error) = self.log.sync_all()
        {
            self.poisoned = true;
            return Err(error.into());
        }
        self.sync_wal = enabled;
        Ok(())
    }
    fn version(&self, key: &Key) -> Result<Option<u64>> {
        if let Some(s) = self.docs.get(key) {
            return Ok(Some(s.version));
        }
        if self.shadow.contains(key) {
            return Ok(None);
        }
        for run in self.runs.iter().rev() {
            self.ensure_run(run)?;
            if run.index.contains_key(key)? {
                return run.index.version(key);
            }
        }
        Ok(None)
    }
    pub fn get(&self, key: &Key) -> Result<Option<StoredDocument>> {
        let _reader = self.maintenance_activity.read();
        self.healthy()?;
        validate_key(key)?;
        if let Some(s) = self.docs.get(key) {
            #[cfg(not(any(
                feature = "experimental-wal-delta",
                feature = "experimental-shared-documents"
            )))]
            return Ok(Some(s.clone()));
            #[cfg(all(
                feature = "experimental-shared-documents",
                not(feature = "experimental-wal-delta")
            ))]
            return Ok(Some(s.to_stored(key)));
            #[cfg(feature = "experimental-wal-delta")]
            return s.read(&self.log, key).map(Some);
        }
        if self.shadow.contains(key) {
            return Ok(None);
        }
        for run in self.runs.iter().rev() {
            if let Some(records) = &run.source_records
                && let Some(slot) = run.index.find(key)?
            {
                let (_, _, version, deleted) = run.index.payload_binding(slot)?;
                if !deleted {
                    return records.read(slot, key, version).map(Some);
                }
            }
            self.ensure_run(run)?;
            if run.index.contains_key(key)? {
                return run.index.get(key);
            }
        }
        Ok(None)
    }
    fn needs_flush(&self) -> bool {
        self.shadow.len() >= self.options.max_delta_keys
            || self.delta_bytes >= self.options.max_delta_bytes
    }
    pub fn commit(&mut self, mutations: Vec<Mutation>) -> Result<u64> {
        self.preempt_revision_work()?;
        #[cfg(feature = "diagnostic-timing")]
        let _total = crate::timing::span("commit.total");
        #[cfg(feature = "diagnostic-timing")]
        let validation = crate::timing::span("commit.validate_guard");
        self.healthy()?;
        self.ensure_sources()?;
        if mutations.is_empty() || mutations.len() > 10_000 {
            return Err(Error::Invalid(
                "batch must contain 1..10000 mutations".into(),
            ));
        }
        let seq = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::Invalid("sequence overflow".into()))?;
        #[cfg(feature = "diagnostic-memory")]
        crate::memory::observe("commit.input", seq);
        let mut seen = BTreeSet::new();
        let mut changes = vec![];
        let mut count = self.count;
        for mutation in mutations {
            let (key, guard) = match &mutation {
                Mutation::Put { document, guard } => {
                    validate_doc(document)?;
                    (&document.key, guard)
                }
                Mutation::Delete { key, guard } => {
                    validate_key(key)?;
                    (key, guard)
                }
            };
            if !seen.insert(key.clone()) {
                return Err(Error::Invalid("duplicate batch key".into()));
            }
            let old = self.version(key)?;
            if !match guard {
                Guard::Any => true,
                Guard::Missing => old.is_none(),
                Guard::Version { version } => old == Some(*version),
            } {
                return Err(Error::Conflict(key.clone()));
            }
            match &mutation {
                Mutation::Put { .. } if old.is_none() => count += 1,
                Mutation::Delete { .. } if old.is_some() => count -= 1,
                _ => {}
            }
            changes.push(match mutation {
                Mutation::Put { document, .. } => Change::Put {
                    stored: StoredDocument {
                        document,
                        version: seq,
                    },
                },
                Mutation::Delete { key, .. } => Change::Delete { key },
            });
        }
        let batch = Batch {
            checkpoint: false,
            changes,
        };
        #[cfg(feature = "diagnostic-memory")]
        crate::memory::observe("commit.validated", seq);
        #[cfg(feature = "diagnostic-timing")]
        drop(validation);
        #[cfg(not(feature = "experimental-wal-delta"))]
        let bytes = if self.needs_flush() {
            // Validate the frame limit before maintenance, but do not keep a
            // full new WAL frame alive while flushing the previous delta.
            crate::binary_batch::encoded_len(&batch)?;
            None
        } else {
            Some(format::encode(seq, &batch)?)
        };
        #[cfg(feature = "experimental-wal-delta")]
        let encoded = if self.needs_flush() {
            crate::binary_batch::encoded_len(&batch)?;
            None
        } else {
            Some(format::encode_located(seq, &batch)?)
        };
        #[cfg(feature = "diagnostic-timing")]
        let maintenance = crate::timing::span("commit.foreground_maintenance");
        self.poll_compaction()?;
        if self.needs_flush() {
            #[cfg(feature = "diagnostic-memory")]
            crate::memory::observe("commit.before_flush", seq);
            self.flush()?;
            #[cfg(feature = "diagnostic-memory")]
            crate::memory::observe("commit.after_flush", seq);
        }
        #[cfg(not(feature = "experimental-wal-delta"))]
        let bytes = match bytes {
            Some(bytes) => bytes,
            None => format::encode(seq, &batch)?,
        };
        #[cfg(feature = "experimental-wal-delta")]
        let (bytes, mut locations) = match encoded {
            Some(encoded) => encoded,
            None => format::encode_located(seq, &batch)?,
        };
        #[cfg(feature = "diagnostic-memory")]
        crate::memory::observe("commit.frame_ready", seq);
        #[cfg(feature = "diagnostic-timing")]
        drop(maintenance);
        #[cfg(feature = "diagnostic-timing")]
        let accounting = crate::timing::span("commit.delta_size_json");
        let mut delta_bytes = self.delta_bytes;
        #[cfg(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        ))]
        let mut row_sizes = Vec::with_capacity(batch.changes.len());
        #[cfg(feature = "experimental-wal-delta")]
        let mut accounting_locations = locations.iter();
        for change in &batch.changes {
            #[cfg(all(
                feature = "experimental-shared-documents",
                not(feature = "experimental-wal-delta")
            ))]
            let row_size = {
                let size = match change {
                    Change::Put { stored } => crate::payload::encoded_len(&stored.document)?,
                    Change::Delete { .. } => 0,
                };
                row_sizes.push(size);
                size
            };
            #[cfg(feature = "experimental-wal-delta")]
            let location = accounting_locations
                .next()
                .expect("encoded change location");
            let key = match change {
                Change::Put { stored } => &stored.document.key,
                Change::Delete { key } => key,
            };
            if let Some(old) = self.docs.get(key) {
                #[cfg(not(any(
                    feature = "experimental-wal-delta",
                    feature = "experimental-shared-documents"
                )))]
                {
                    delta_bytes -= crate::payload::encoded_len(&old.document)?;
                }
                #[cfg(all(
                    feature = "experimental-shared-documents",
                    not(feature = "experimental-wal-delta")
                ))]
                {
                    delta_bytes -= old.serialized_len;
                }
                #[cfg(feature = "experimental-wal-delta")]
                {
                    delta_bytes -= old.location.len as usize;
                }
            }
            if let Change::Put { stored } = change {
                #[cfg(not(any(
                    feature = "experimental-wal-delta",
                    feature = "experimental-shared-documents"
                )))]
                {
                    delta_bytes += crate::payload::encoded_len(&stored.document)?;
                }
                #[cfg(all(
                    feature = "experimental-shared-documents",
                    not(feature = "experimental-wal-delta")
                ))]
                {
                    let _ = stored;
                    delta_bytes += row_size;
                }
                #[cfg(feature = "experimental-wal-delta")]
                {
                    let _ = stored;
                    delta_bytes += location.expect("encoded put location").len as usize;
                }
            }
        }
        #[cfg(feature = "diagnostic-timing")]
        drop(accounting);
        #[cfg(feature = "experimental-compact-delta-postings")]
        self.index.check_capacity(batch.changes.len())?;
        #[cfg(feature = "experimental-wal-delta")]
        {
            let start = self.log.metadata()?.len();
            for location in locations.iter_mut().flatten() {
                location.offset = location
                    .offset
                    .checked_add(start)
                    .ok_or_else(|| Error::Invalid("WAL offset overflow".into()))?;
            }
        }
        #[cfg(feature = "experimental-wal-sync-control")]
        let appended = format::append_with_sync(&mut self.log, &bytes, self.sync_wal);
        #[cfg(not(feature = "experimental-wal-sync-control"))]
        let appended = format::append(&mut self.log, &bytes);
        if let Err(e) = appended {
            self.poisoned = true;
            return Err(e);
        }
        // Durability has completed. The owned changes/locations remain; the
        // encoded frame is no longer needed during delta indexing.
        drop(bytes);
        #[cfg(feature = "diagnostic-memory")]
        crate::memory::observe("commit.frame_dropped", seq);
        #[cfg(feature = "diagnostic-timing")]
        let _apply = crate::timing::span("commit.apply_delta_index");
        #[cfg(feature = "experimental-wal-delta")]
        let mut applied_locations = locations.into_iter();
        #[cfg(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        ))]
        let mut row_sizes = row_sizes.into_iter();
        for change in batch.changes {
            #[cfg(all(
                feature = "experimental-shared-documents",
                not(feature = "experimental-wal-delta")
            ))]
            let row_size = row_sizes.next().expect("encoded change size");
            #[cfg(feature = "experimental-wal-delta")]
            let location = applied_locations.next().expect("encoded change location");
            let key = match &change {
                Change::Put { stored } => &stored.document.key,
                Change::Delete { key } => key,
            };
            #[cfg(feature = "diagnostic-memory")]
            let _origin = crate::memory::Scope::new(3);
            let shared_key = remember_delta_key(&mut self.shadow, key);
            #[cfg(all(
                feature = "experimental-shared-documents",
                not(feature = "experimental-wal-delta")
            ))]
            let old = self
                .docs
                .remove_entry(key)
                .map(|(key, row)| row.into_stored(Arc::unwrap_or_clone(key)));
            #[cfg(not(all(
                feature = "experimental-shared-documents",
                not(feature = "experimental-wal-delta")
            )))]
            let old = self.docs.remove(key);
            if let Some(old) = old {
                #[cfg(feature = "experimental-wal-delta")]
                let old = match old.read(&self.log, key) {
                    Ok(old) => old,
                    Err(error) => {
                        self.poisoned = true;
                        return Err(error);
                    }
                };
                #[cfg(feature = "diagnostic-memory")]
                let _index_origin = crate::memory::Scope::new(2);
                if let Err(error) = self.index.change(&old, false) {
                    self.poisoned = true;
                    return Err(error);
                }
            }
            if let Change::Put { stored } = change {
                {
                    #[cfg(feature = "diagnostic-memory")]
                    let _index_origin = crate::memory::Scope::new(2);
                    #[cfg(all(
                        feature = "experimental-shared-documents",
                        not(feature = "experimental-wal-delta")
                    ))]
                    let indexed = self.index.change_shared(&stored, true, shared_key.clone());
                    #[cfg(not(all(
                        feature = "experimental-shared-documents",
                        not(feature = "experimental-wal-delta")
                    )))]
                    let indexed = self.index.change(&stored, true);
                    if let Err(error) = indexed {
                        self.poisoned = true;
                        return Err(error);
                    }
                }
                #[cfg(not(any(
                    feature = "experimental-wal-delta",
                    feature = "experimental-shared-documents"
                )))]
                self.docs.insert(shared_key, stored);
                #[cfg(all(
                    feature = "experimental-shared-documents",
                    not(feature = "experimental-wal-delta")
                ))]
                {
                    let (_, row) = match self.doc_cache.store(stored, row_size) {
                        Ok(row) => row,
                        Err(error) => {
                            self.poisoned = true;
                            return Err(error);
                        }
                    };
                    self.docs.insert(shared_key, row);
                }
                #[cfg(feature = "experimental-wal-delta")]
                self.docs.insert(
                    shared_key,
                    crate::wal_delta::Row {
                        version: stored.version,
                        location: location.expect("encoded put location"),
                    },
                );
            }
        }
        self.count = count;
        self.delta_bytes = delta_bytes;
        self.sequence = seq;
        #[cfg(feature = "diagnostic-memory")]
        crate::memory::observe("commit.applied", seq);
        Ok(seq)
    }

    /// Opt-in query-only experiment. No source/index mutation or format change.
    pub fn set_query_visibility_pruning(&mut self, enabled: bool) {
        self.query_visibility_pruning = enabled;
    }
    pub fn search(&self, query: &Query) -> Result<SearchPage> {
        #[cfg(feature = "experimental-paged-metadata")]
        {
            self.search_grouped_slots(query)
        }
        #[cfg(not(feature = "experimental-paged-metadata"))]
        {
            self.search_impl(query, 0)
        }
    }

    /// Replace a caller-owned page, reusing eligible binary result text buffers.
    /// Old results are consumed even on error; an error leaves an empty page.
    /// No off-page document cache survives the call. Oversized/legacy/direct
    /// reads retain their normal checked decoding path. Empty buffers, single
    /// hits and checked-payload capture queries use ordinary search; text
    /// reuse requires an exact capacity match, retaining no text slack.
    #[cfg(feature = "bounded-engine")]
    pub fn search_into(&self, query: &Query, page: &mut SearchPage) -> Result<()> {
        let mut previous = std::mem::take(&mut page.hits);
        page.total = 0;
        page.sequence = 0;
        let hydration = if query
            .offset
            .checked_add(query.limit)
            .is_some_and(|n| n <= 4096)
        {
            2
        } else {
            1
        };
        if previous.is_empty()
            || query.limit <= 1
            || hydration != 2
            || Self::captures_checked_page(query, hydration)
        {
            previous.clear();
            *page = self.search(query)?;
            return Ok(());
        }
        previous.truncate(query.limit.min(1000));
        // Release old keys/metadata/links before filtering the next query.
        // Only text allocations are useful to hydration; no old document cache.
        let mut texts = Vec::with_capacity(previous.len());
        for stored in previous {
            texts.push(stored.document.text);
        }
        let mut pool = texts.into_iter();
        *page = self.search_with_pool::<true>(query, hydration, &mut pool)?;
        Ok(())
    }

    /// Opt-in run/slot hydration with identical query semantics.
    #[cfg(feature = "experimental-direct-slot")]
    pub fn search_direct_slots(&self, query: &Query) -> Result<SearchPage> {
        self.search_impl(query, 1)
    }

    /// Opt-in bounded, grouped payload reads from visible run/slot selections.
    #[cfg(feature = "experimental-grouped-read")]
    pub fn search_grouped_slots(&self, query: &Query) -> Result<SearchPage> {
        // Keep the measured deep-offset regression out of the grouped path.
        // Ranked cursors will replace the full-ID fallback in a later stage.
        if query
            .offset
            .checked_add(query.limit)
            .is_some_and(|n| n <= 4096)
        {
            self.search_impl(query, 2)
        } else {
            self.search_direct_slots(query)
        }
    }

    fn search_impl(&self, query: &Query, hydration: u8) -> Result<SearchPage> {
        self.search_with_pool::<false>(query, hydration, &mut Vec::new().into_iter())
    }

    fn search_with_pool<const REUSE: bool>(
        &self,
        query: &Query,
        hydration: u8,
        pool: &mut std::vec::IntoIter<String>,
    ) -> Result<SearchPage> {
        #[cfg(feature = "bounded-engine")]
        if !REUSE && Self::captures_checked_page(query, hydration) {
            return self.search_impl_mode::<true, REUSE>(query, hydration, pool);
        }
        self.search_impl_mode::<false, REUSE>(query, hydration, pool)
    }

    #[cfg(feature = "bounded-engine")]
    fn captures_checked_page(query: &Query, hydration: u8) -> bool {
        query.offset == 0
            && query.limit <= 50
            && hydration == 2
            && query.text.len() <= 4096
            && query
                .text
                .split_whitespace()
                .any(|term| term.chars().nth(3).is_some())
    }

    fn search_impl_mode<const CAPTURE: bool, const REUSE: bool>(
        &self,
        query: &Query,
        _hydration: u8,
        pool: &mut std::vec::IntoIter<String>,
    ) -> Result<SearchPage> {
        let _reader = self.maintenance_activity.read();
        #[cfg(feature = "diagnostic-timing")]
        let _search = crate::timing::span("search.total");
        #[cfg(feature = "diagnostic-timing")]
        let candidates = crate::timing::span("search.filter_visibility_text");
        self.healthy()?;
        self.ensure_sources()?;
        validate_key(&Key::new(
            &query.scope,
            query.collection.as_deref().unwrap_or("_"),
            "_",
        ))?;
        if query.limit == 0
            || query.limit > 1000
            || query.text.len() > 4096
            || query.exact.len() > 256
            || query.ranges.len() > 32
        {
            return Err(Error::Invalid("query limits exceeded".into()));
        }
        if query.exact.values().any(|v| v.is_array() || v.is_object())
            || query.ranges.iter().any(|r| r.min > r.max)
        {
            return Err(Error::Invalid("invalid query filters".into()));
        }
        if let Some(k) = &query.linked_to {
            validate_key(k)?;
            if k.scope != query.scope {
                return Err(Error::Invalid("linked_to scope mismatch".into()));
            }
        }
        let delta = self.delta_matches(query);
        let mut lists = vec![];
        let mut total = delta.len();
        // Keep ordinary numbered pages bounded. Very large offsets retain the
        // original path until ranked disk cursors replace that fallback.
        let prefix_limit = query.offset.checked_add(query.limit).filter(|n| *n <= 4096);
        #[cfg(feature = "bounded-engine")]
        let mut checked_page = crate::segment::CheckedPage::new(query.limit);
        #[cfg(feature = "bounded-engine")]
        let reuse_checked = CAPTURE;
        #[cfg(feature = "bounded-engine")]
        let mut fallback_positions = Vec::new();
        for (n, run) in self.runs.iter().enumerate() {
            #[cfg(feature = "diagnostic-timing")]
            let visibility = crate::timing::span("query.visibility_mask");
            let mut mask = vec![0u64; run.index.len().div_ceil(64)];
            #[cfg(feature = "experimental-paged-metadata")]
            let cached = self
                .mask_cache
                .lock()
                .map_err(|_| Error::Corrupt("visibility cache poisoned".into()))?
                .apply(run.source.id, &mut mask);
            #[cfg(not(feature = "experimental-paged-metadata"))]
            let cached = false;
            if !cached {
                for later in &self.runs[n + 1..] {
                    run.index.hide_keys(&later.index, &mut mask)?;
                }
                #[cfg(feature = "experimental-paged-metadata")]
                self.mask_cache
                    .lock()
                    .map_err(|_| Error::Corrupt("visibility cache poisoned".into()))?
                    .remember(run.source.id, &mask);
            }
            for key in &self.shadow {
                if let Some(id) = run.index.find(key)? {
                    mask[id as usize / 64] |= 1 << (id % 64);
                }
            }
            let hidden = |id: u32| mask[id as usize / 64] & (1u64 << (id % 64)) != 0;
            #[cfg(feature = "diagnostic-timing")]
            drop(visibility);
            let ids = if let Some(limit) = prefix_limit {
                #[cfg(feature = "bounded-engine")]
                let (count, ids) = if reuse_checked {
                    run.index.prefix_matches_checked(
                        query,
                        &mask,
                        limit,
                        n,
                        &mut checked_page,
                        self.query_visibility_pruning,
                    )?
                } else {
                    run.index
                        .prefix_matches(query, &mask, limit, self.query_visibility_pruning)?
                };
                #[cfg(not(feature = "bounded-engine"))]
                let (count, ids) =
                    run.index
                        .prefix_matches(query, &mask, limit, self.query_visibility_pruning)?;
                total += count;
                ids
            } else {
                let ids = run
                    .index
                    .matches(query, hidden, self.query_visibility_pruning)?;
                total += ids.len();
                ids
            };
            lists.push(ids);
        }
        #[cfg(feature = "diagnostic-timing")]
        drop(candidates);
        #[cfg(feature = "diagnostic-timing")]
        let _page = crate::timing::span("search.merge_page_hydration");
        let mut heap: BinaryHeap<Reverse<(Key, usize, usize)>> = BinaryHeap::new();
        for (n, ids) in lists.iter().enumerate() {
            if let Some(id) = ids.first() {
                heap.push(Reverse((self.runs[n].index.key(*id)?, n, 0)));
            }
        }
        if let Some(key) = delta.first() {
            heap.push(Reverse((key.as_ref().clone(), self.runs.len(), 0)));
        }
        let mut hits = vec![];
        #[cfg(feature = "experimental-grouped-read")]
        let mut selected = Vec::new();
        let grouped = cfg!(feature = "experimental-grouped-read") && _hydration == 2;
        #[cfg(feature = "bounded-engine")]
        let mut reused = if CAPTURE {
            // The exact total is already known. Keep the page buffer bounded
            // without geometric growth surviving the final in-place collect.
            Vec::with_capacity(query.limit.min(total))
        } else {
            Vec::new()
        };
        let mut page_size = 0;
        let mut position = 0;
        while let Some(Reverse((key, n, slot))) = heap.pop() {
            #[cfg(feature = "experimental-paged-metadata")]
            if position < query.offset {
                // Lists are already sorted and visibility-resolved. Skip the
                // range preceding the next competing run in logarithmic reads.
                let available = if n == self.runs.len() {
                    delta.len()
                } else {
                    lists[n].len()
                };
                let end = available.min(slot.saturating_add(query.offset - position));
                let next_key = |index: usize| -> Result<Key> {
                    if n == self.runs.len() {
                        Ok(delta[index].as_ref().clone())
                    } else {
                        self.runs[n].index.key(lists[n][index])
                    }
                };
                let mut lo = slot;
                let mut hi = end;
                if let Some(Reverse((boundary, _, _))) = heap.peek() {
                    while lo < hi {
                        let mid = lo + (hi - lo) / 2;
                        if next_key(mid)? < *boundary {
                            lo = mid + 1;
                        } else {
                            hi = mid;
                        }
                    }
                } else {
                    lo = end;
                }
                if lo > slot {
                    position += lo - slot;
                    if lo < available {
                        heap.push(Reverse((next_key(lo)?, n, lo)));
                    }
                    continue;
                }
            }

            if position >= query.offset {
                #[cfg(feature = "experimental-grouped-read")]
                if grouped {
                    #[cfg(feature = "bounded-engine")]
                    let retained = if reuse_checked && n < self.runs.len() {
                        checked_page.take(&key, n, lists[n][slot])?
                    } else {
                        None
                    };
                    #[cfg(feature = "bounded-engine")]
                    if let Some(stored) = retained {
                        reused.push((page_size, stored));
                    } else {
                        selected.push((n, slot));
                        if CAPTURE {
                            fallback_positions.push(page_size);
                        }
                    }
                    #[cfg(not(feature = "bounded-engine"))]
                    selected.push((n, slot));
                }
                if !grouped {
                    // Visibility was resolved while producing each run's ID list.
                    // The opt-in experiment keeps that run/slot for hydration.
                    let by_key = || {
                        self.get(&key)?
                            .ok_or_else(|| Error::Corrupt("missing search hit".into()))
                    };
                    #[cfg(feature = "experimental-direct-slot")]
                    let hit = if _hydration == 1 {
                        if n == self.runs.len() {
                            self.get(&key)?
                                .ok_or_else(|| Error::Corrupt("missing delta search hit".into()))?
                        } else {
                            self.runs[n].index.read_selected_document(lists[n][slot])?
                        }
                    } else {
                        by_key()?
                    };
                    #[cfg(not(feature = "experimental-direct-slot"))]
                    let hit = by_key()?;
                    hits.push(hit);
                }
                page_size += 1;
                if page_size == query.limit {
                    break;
                }
            }
            position += 1;
            let next = slot + 1;
            let key = if n == self.runs.len() {
                delta.get(next).map(|key| key.as_ref().clone())
            } else {
                lists[n]
                    .get(next)
                    .map(|id| self.runs[n].index.key(*id))
                    .transpose()?
            };
            if let Some(key) = key {
                heap.push(Reverse((key, n, next)));
            }
        }
        // Any remaining copies are outside the selected page (e.g. displaced
        // by delta hits). Release them before allocating fallback documents.
        #[cfg(feature = "bounded-engine")]
        drop(checked_page);
        #[cfg(feature = "experimental-grouped-read")]
        if grouped {
            let mut by_run: BTreeMap<usize, Vec<(u32, usize)>> = BTreeMap::new();
            #[cfg(feature = "bounded-engine")]
            let mut hydrated = if CAPTURE {
                reused
            } else {
                Vec::with_capacity(selected.len())
            };
            #[cfg(not(feature = "bounded-engine"))]
            let mut hydrated = Vec::with_capacity(selected.len());
            if CAPTURE {
                hydrated.reserve_exact(selected.len());
            }
            for (ordinal, (run, slot)) in selected.into_iter().enumerate() {
                #[cfg(feature = "bounded-engine")]
                let position = if CAPTURE {
                    fallback_positions[ordinal]
                } else {
                    ordinal
                };
                #[cfg(not(feature = "bounded-engine"))]
                let position = ordinal;
                if run == self.runs.len() {
                    let key = &delta[slot];
                    let stored = self
                        .get(key)?
                        .ok_or_else(|| Error::Corrupt("missing delta search hit".into()))?;
                    hydrated.push((position, stored));
                } else {
                    by_run
                        .entry(run)
                        .or_default()
                        .push((lists[run][slot], position));
                }
            }
            // One bounded window is reused across all selected runs.
            let mut scratch = Vec::new();
            for (run, slots) in by_run {
                if REUSE {
                    self.runs[run].index.read_selected_batch_with_pool::<true>(
                        &slots,
                        &mut scratch,
                        &mut hydrated,
                        pool,
                    )?;
                } else {
                    self.runs[run].index.read_selected_batch(
                        &slots,
                        &mut scratch,
                        &mut hydrated,
                    )?;
                }
            }
            hydrated.sort_unstable_by_key(|(position, _)| *position);
            hits = hydrated.into_iter().map(|(_, stored)| stored).collect();
        }
        // Non-grouped feature profiles do not consume the previous page.
        let _ = pool;
        Ok(SearchPage {
            total,
            sequence: self.sequence,
            hits,
        })
    }

    fn descriptors(&self) -> Vec<Source> {
        self.runs.iter().map(|r| r.source.clone()).collect()
    }
    /// Reclaim obsolete engine-owned files after full integrity validation.
    /// Current sources/WAL, pinned readers, and in-flight merge files survive.
    /// An I/O error can leave a partially completed, safely retryable cleanup.
    pub fn reclaim_obsolete(&mut self) -> Result<crate::ReclaimStats> {
        self.check_integrity()?;
        self.collect_obsolete()
    }

    fn collect_obsolete(&mut self) -> Result<crate::ReclaimStats> {
        self.healthy()?;
        if manifest::inventory(&self.dir)?.0 != Some(self.generation) {
            return Err(Error::Corrupt("reclaim latest publication changed".into()));
        }
        // Re-read the authoritative root. A corrupt/stale manifest must never
        // turn the in-memory descriptor list into authority for deletion.
        match manifest::read(&self.dir, self.generation)? {
            Publication::Current(m) if m.active == self.active && m.runs == self.descriptors() => {}
            Publication::Legacy { end }
                if self.active.id == self.generation
                    && self.active.end == end
                    && self.runs.len() == 1
                    && self.runs[0].source.kind == Kind::Legacy
                    && self.runs[0].source.id == self.generation
                    && self.runs[0].source.end == end => {}
            _ => {
                return Err(Error::Corrupt(
                    "reclaim publication identity mismatch".into(),
                ));
            }
        }
        let mut live: BTreeSet<_> = self
            .runs
            .iter()
            .map(|r| (r.source.kind, r.source.id))
            .collect();
        live.insert((Kind::Legacy, self.active.id));
        self.retired
            .retain(|(_, reader)| reader.strong_count() != 0);
        live.extend(
            self.retired
                .iter()
                .map(|(source, _)| (source.kind, source.id)),
        );
        if let Some(job) = &self.job {
            live.insert((Kind::Run, job.output_id));
            live.extend(job.inputs.iter().map(|source| (source.kind, source.id)));
        }
        crate::reclaim::collect(&self.dir, self.generation, &live)
    }

    fn try_reclaim(&mut self) {
        // Publication already succeeded: cleanup failure must not turn an
        // acknowledged write into an ambiguous error or poison the database.
        // Explicit reclaim reports errors; later publication/open retries.
        let _ = self.collect_obsolete();
    }
    fn publish(&mut self, id: u64, m: &Manifest) -> Result<()> {
        if let Err(e) = manifest::publish(&self.dir, id, m) {
            self.poisoned = true;
            return Err(e);
        }
        self.generation = id;
        Ok(())
    }
    /// Write only the resident delta and retain all prior immutable sources.
    pub fn flush(&mut self) -> Result<Stats> {
        #[cfg(feature = "diagnostic-timing")]
        let _flush = crate::timing::span("flush.total");
        self.healthy()?;
        self.ensure_sources()?;
        self.poll_compaction()?;
        if self.shadow.is_empty() {
            self.try_reclaim();
            return self.stats();
        }
        let id = self.allocate()?;
        let changes = self.shadow.iter().map(|key| {
            Ok(match self.get(key)? {
                Some(stored) => Change::Put { stored },
                None => Change::Delete {
                    key: own_delta_key(key),
                },
            })
        });
        #[cfg(not(feature = "experimental-merge-postings"))]
        let run = write_run(&self.dir, id, self.sequence, changes)?;
        #[cfg(feature = "experimental-merge-postings")]
        let run = write_run_prepared(&self.dir, id, self.sequence, changes, |source| {
            let records = self
                .shadow
                .iter()
                .enumerate()
                .map(|(slot, key)| {
                    let id = u32::try_from(slot)
                        .map_err(|_| Error::Invalid("delta checkpoint exceeds u32".into()))?;
                    Ok((own_delta_key(key), id))
                })
                .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
            reused_builder(&self.dir, source)?.finish_delta(&self.index, &records)?;
            Ok(())
        })?;
        let id = self.allocate()?;
        let (log, active) = manifest::new_active(&self.dir, id, self.sequence)?;
        let mut runs = self.descriptors();
        runs.push(run.source.clone());
        self.publish(
            id,
            &Manifest {
                version: 3,
                runs,
                active: active.clone(),
            },
        )?;
        self.log = log;
        self.active = active;
        self.runs.push(run);
        #[cfg(feature = "experimental-paged-metadata")]
        {
            *self
                .mask_cache
                .get_mut()
                .map_err(|_| Error::Corrupt("visibility cache poisoned".into()))? =
                MaskCache::default();
        }
        #[cfg(not(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        )))]
        self.docs.clear();
        #[cfg(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        ))]
        {
            self.docs = std::collections::HashMap::new();
        }
        #[cfg(all(
            feature = "experimental-shared-documents",
            not(feature = "experimental-wal-delta")
        ))]
        {
            self.doc_cache = crate::shared_documents::Cache::default();
        }
        self.shadow.clear();
        self.index = Index::default();
        self.delta_bytes = 0;
        self.try_reclaim();
        if self.runs.len() >= 4 {
            self.start_compaction_mode(true)?;
        }
        self.stats()
    }
    /// Flush, then schedule at most one bounded merge; does not wait for it.
    pub fn compact(&mut self) -> Result<Stats> {
        self.flush()?;
        self.start_compaction()?;
        self.stats()
    }

    pub fn start_compaction(&mut self) -> Result<bool> {
        self.start_compaction_mode(false)
    }
    fn start_compaction_mode(&mut self, _automatic: bool) -> Result<bool> {
        #[cfg(feature = "diagnostic-timing")]
        let _plan = crate::timing::maintenance::span("planning_metadata");
        self.healthy()?;
        self.ensure_sources()?;
        if self.job.is_some() {
            return Ok(false);
        }
        #[cfg(feature = "experimental-write-merge-policy")]
        let selected = select_merge(&self.descriptors(), _automatic);
        #[cfg(not(feature = "experimental-write-merge-policy"))]
        let selected = {
            let mut selected = None;
            for start in 0..self.runs.len().saturating_sub(1) {
                let mut keys = 0;
                let mut bytes = 0;
                let mut min = usize::MAX;
                let mut max = 0;
                let mut end = start;
                for run in self.runs[start..].iter().take(4) {
                    keys += run.source.keys;
                    bytes += run.source.end;
                    min = min.min(run.source.keys.max(1));
                    max = max.max(run.source.keys.max(1));
                    if keys > 100_000 || bytes > 64 * 1024 * 1024 || max > min * 4 {
                        break;
                    }
                    end += 1;
                }
                if end - start >= 2 {
                    selected = Some((start, end));
                    break;
                }
            }
            selected
        };
        let Some((start, end)) = selected else {
            // Size-tier merging alone cannot reclaim revisions in large or
            // dissimilar runs. Explicit maintenance also drains one stale run.
            return if _automatic || !self.revision_reclamation {
                Ok(false)
            } else {
                self.start_revision_reclamation()
            };
        };
        let inputs = self.runs[start..end].to_vec();
        #[cfg(feature = "diagnostic-timing")]
        {
            crate::timing::count("merge.scheduled_jobs", 1);
            crate::timing::count(
                "merge.scheduled_input_records",
                inputs.iter().map(|r| r.source.keys).sum(),
            );
        }
        let descriptors = inputs.iter().map(|r| r.source.clone()).collect();
        let dir = self.dir.clone();
        let id = self.allocate()?;
        let handle = std::thread::spawn(move || {
            #[cfg(feature = "diagnostic-timing")]
            let _worker = crate::timing::maintenance::worker();
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("merge.worker");
            let sequence = inputs.last().unwrap().source.sequence;
            #[cfg(feature = "experimental-merge-postings")]
            let maps = {
                #[cfg(feature = "diagnostic-timing")]
                let verify = crate::timing::span("merge.verify_sources_indexes");
                for run in &inputs {
                    let mut file = File::open(run.source.path(&dir, "fdb"))?;
                    let actual = file.metadata()?.len();
                    if actual < run.source.end
                        || (run.source.kind == Kind::Run && actual != run.source.end)
                        || format::prefix_crc(&mut file, run.source.end)? != run.source.checksum
                    {
                        return Err(Error::Corrupt("merge source checksum mismatch".into()));
                    }
                    run.index.verify_reuse()?;
                }
                #[cfg(feature = "diagnostic-timing")]
                drop(verify);
                #[cfg(feature = "diagnostic-timing")]
                let _maps = crate::timing::span("merge.build_id_maps");
                let mut maps: Vec<_> = inputs
                    .iter()
                    .map(|run| vec![u32::MAX; run.index.len()])
                    .collect();
                for (output, item) in MergeKeys::new(&inputs)?.enumerate() {
                    let (n, slot) = item?;
                    let output = u32::try_from(output)
                        .ok()
                        .filter(|id| *id != u32::MAX)
                        .ok_or_else(|| Error::Invalid("merge ID capacity".into()))?;
                    if !inputs[n].index.deleted(slot) {
                        maps[n][slot as usize] = output;
                    }
                }
                maps
            };
            let changes = MergeKeys::new(&inputs)?.map(|item| {
                let (n, slot) = item?;
                let index = &inputs[n].index;
                if index.deleted(slot) {
                    Ok(Change::Delete {
                        key: index.key(slot)?,
                    })
                } else {
                    #[cfg(feature = "experimental-write-merge-slot")]
                    let stored = index.read_selected_document(slot)?;
                    #[cfg(not(feature = "experimental-write-merge-slot"))]
                    let stored = index
                        .get(&index.key(slot)?)?
                        .ok_or_else(|| Error::Corrupt("missing merge payload".into()))?;
                    Ok(Change::Put { stored })
                }
            });
            let result = {
                #[cfg(not(feature = "experimental-merge-postings"))]
                {
                    write_run(&dir, id, sequence, changes)
                }
                #[cfg(feature = "experimental-merge-postings")]
                {
                    write_run_prepared(&dir, id, sequence, changes, |source| {
                        let builder = reused_builder(&dir, source)?;
                        let paired: Vec<_> = inputs
                            .iter()
                            .zip(&maps)
                            .map(|(run, map)| (run.index.as_ref(), map.as_slice()))
                            .collect();
                        builder.finish_reused(&paired)?;
                        Ok(())
                    })
                }
            };
            result.map(JobOutput::Replace)
        });
        self.job = Some(Job {
            inputs: descriptors,
            output_id: id,
            handle,
            cancel: None,
            revision: false,
        });
        Ok(true)
    }
    fn install(&mut self, job: Job) -> Result<bool> {
        #[cfg(feature = "diagnostic-timing")]
        let _install = crate::timing::maintenance::span("publication_reclaim");
        #[cfg(feature = "diagnostic-timing")]
        let join_stage = crate::timing::maintenance::span("join_wait");
        #[cfg(feature = "diagnostic-timing")]
        let join = crate::timing::span("merge.join_wait");
        let output = job
            .handle
            .join()
            .map_err(|_| Error::Invalid("compaction worker panicked".into()))??;
        #[cfg(feature = "diagnostic-timing")]
        drop(join);
        #[cfg(feature = "diagnostic-timing")]
        drop(join_stage);
        if matches!(output, JobOutput::Cancelled) {
            return Ok(false);
        }
        let Some(start) = self
            .runs
            .windows(job.inputs.len())
            .position(|window| window.iter().map(|r| &r.source).eq(job.inputs.iter()))
        else {
            return Err(Error::Corrupt("compaction inputs changed".into()));
        };
        let replacement = match output {
            JobOutput::Replace(run) => vec![run],
            JobOutput::Retire => vec![],
            JobOutput::Cancelled => unreachable!(),
        };
        let mut descriptors = self.descriptors();
        descriptors.splice(
            start..start + job.inputs.len(),
            replacement.iter().map(|r| r.source.clone()),
        );
        let id = self.allocate()?;
        self.publish(
            id,
            &Manifest {
                version: 3,
                runs: descriptors,
                active: self.active.clone(),
            },
        )?;
        for old in self
            .runs
            .splice(start..start + job.inputs.len(), replacement)
        {
            self.retired.push((old.source, Arc::downgrade(&old.index)));
        }
        #[cfg(feature = "experimental-paged-metadata")]
        {
            *self
                .mask_cache
                .get_mut()
                .map_err(|_| Error::Corrupt("visibility cache poisoned".into()))? =
                MaskCache::default();
        }
        self.try_reclaim();
        Ok(true)
    }
    pub fn poll_compaction(&mut self) -> Result<bool> {
        self.healthy()?;
        self.ensure_sources()?;
        if self.job.as_ref().is_some_and(|j| j.handle.is_finished()) {
            let job = self.job.take().unwrap();
            return self.install(job);
        }
        Ok(false)
    }
    pub fn wait_for_compaction(&mut self) -> Result<bool> {
        self.healthy()?;
        if let Some(job) = self.job.take() {
            self.install(job)
        } else {
            Ok(false)
        }
    }
    pub fn stats(&self) -> Result<Stats> {
        self.healthy()?;
        Ok(Stats {
            documents: self.count,
            sequence: self.sequence,
            generation: self.generation,
            log_bytes: self.log.metadata()?.len(),
            disk_index_bytes: self.runs.iter().map(|r| r.index.bytes).sum(),
            disk_posting_entries: self.runs.iter().map(|r| r.index.posting_entries).sum(),
            #[cfg(not(feature = "experimental-wal-delta"))]
            resident_documents: self.docs.len(),
            #[cfg(feature = "experimental-wal-delta")]
            resident_documents: 0,
            #[cfg(not(any(
                feature = "experimental-wal-delta",
                feature = "experimental-shared-documents"
            )))]
            resident_text_bytes: self
                .docs
                .values()
                .map(|s| s.document.title.len() + s.document.text.len())
                .sum::<usize>()
                + self.index.folded_bytes(),
            #[cfg(all(
                feature = "experimental-shared-documents",
                not(feature = "experimental-wal-delta")
            ))]
            resident_text_bytes: self
                .docs
                .values()
                .map(|row| row.exclusive_text_bytes())
                .sum::<usize>()
                + self.doc_cache.text_bytes
                + self.index.folded_bytes(),
            #[cfg(feature = "experimental-wal-delta")]
            resident_text_bytes: self.index.folded_bytes(),
            delta_keys: self.shadow.len(),
            segments: self.runs.len(),
            compaction_running: self.job.is_some(),
            source_bytes: self.runs.iter().map(|r| r.source.end).sum::<u64>()
                + self.log.metadata()?.len(),
        })
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            let _ = job.handle.join();
        }
    }
}

#[cfg(test)]
mod reclaim_tests;
