#![cfg(feature = "bounded-engine")]
use fdb::{Database, Document, IndexCacheLayout as Layout, Key, Mutation, Query};
use std::{collections::BTreeMap, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("fdb-compact-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn index(&self) -> PathBuf {
        std::fs::read_dir(&self.0)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.extension().is_some_and(|e| e == "idx")
                    && p.file_stem().unwrap().to_string_lossy().starts_with("run-")
                    && !p.file_stem().unwrap().to_string_lossy().contains('.')
            })
            .unwrap()
    }
    fn seed(&self) {
        let mut db = Database::open(&self.0).unwrap();
        for (id, text) in [
            ("a", "İSTANBUL 🦀 quixote"),
            ("b", "other 東京"),
            ("c", "quixote whale"),
        ] {
            db.commit(vec![Mutation::Put {
                document: Document {
                    key: Key::new("shared", "texts", id),
                    title: "quote\" slash\\".into(),
                    text: text.into(),
                    fields: BTreeMap::from([
                        ("n".into(), serde_json::json!(-123)),
                        (
                            "nested".into(),
                            serde_json::json!([null,true,u64::MAX,{"x":"東京"}]),
                        ),
                    ]),
                    links: vec![Key::new("shared", "texts", "a")],
                },
                guard: fdb::Guard::Any,
            }])
            .unwrap();
        }
        db.flush().unwrap();
    }
    fn verify(&self) {
        let db = Database::open(&self.0).unwrap();
        assert_eq!(db.pending_source_checks().unwrap(), 0);
        let a = db.get(&Key::new("shared", "texts", "a")).unwrap().unwrap();
        assert_eq!(a.document.text, "İSTANBUL 🦀 quixote");
        assert_eq!(a.document.fields["nested"][2], serde_json::json!(u64::MAX));
        for (text, total) in [
            ("quixote", 2),
            ("he", 1),
            ("quixote whale", 1),
            ("istanbul", 1),
            ("missing", 0),
        ] {
            let mut q = Query::new("shared");
            q.collection = Some("texts".into());
            q.text = text.into();
            assert_eq!(db.search(&q).unwrap().total, total, "{text}");
        }
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.exact.insert("n".into(), serde_json::json!(-123));
        q.ranges.push(fdb::Range {
            field: "n".into(),
            min: -124,
            max: -122,
        });
        q.linked_to = Some(Key::new("shared", "texts", "a"));
        assert_eq!(db.search(&q).unwrap().total, 3);
        db.check_integrity().unwrap();
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let target = self.0.canonicalize().unwrap();
        assert!(
            target.starts_with(root)
                && target
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("fdb-compact-")
        );
        std::fs::remove_dir_all(target).unwrap();
    }
}
fn source_files(temp: &Temp) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<_> = std::fs::read_dir(&temp.0)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "fdb" || e == "ready"))
        .map(|p| {
            let b = std::fs::read(&p).unwrap();
            (p, b)
        })
        .collect();
    files.sort();
    files
}

fn assert_references(path: &std::path::Path) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[..8], b"FDBSEG05");
    let footer = bytes.len() - 32;
    assert_eq!(&bytes[footer..footer + 8], b"FDBEND05");
    let directory = u64::from_le_bytes(bytes[footer + 8..footer + 16].try_into().unwrap()) as usize;
    assert_eq!(&bytes[directory..directory + 8], b"FDBDIC01");
    assert_eq!(bytes[directory + 8], 1);
    // The entire data area consists of u32 posting IDs, no OFF1 payload copy.
    assert_eq!((directory - 28) % 4, 0);
    assert!(!bytes[28..directory].windows(4).any(|w| w == b"OFF1"));
}

#[test]
fn default_flush_rebuild_and_merge_write_source_references_without_conversion() {
    let t = Temp::new();
    t.seed();
    assert_references(&t.index());
    let source = source_files(&t);
    // A readable SEG04 remains usable without an implicit rewrite on open.
    Database::open(&t.0)
        .unwrap()
        .repack_index_cache(Layout::JsonPayloadCopy)
        .unwrap();
    let old = std::fs::read(t.index()).unwrap();
    t.verify();
    assert_eq!(std::fs::read(t.index()).unwrap(), old);
    // Missing old cache is rebuilt directly as SEG05.
    std::fs::remove_file(t.index()).unwrap();
    t.verify();
    assert_references(&t.index());
    assert_eq!(source_files(&t), source);
    let mut db = Database::open(&t.0).unwrap();
    db.commit(vec![Mutation::Delete {
        key: Key::new("shared", "texts", "b"),
        guard: fdb::Guard::Any,
    }])
    .unwrap();
    db.flush().unwrap();
    db.compact().unwrap();
    db.wait_for_compaction().unwrap();
    assert!(db.get(&Key::new("shared", "texts", "b")).unwrap().is_none());
    db.check_integrity().unwrap();
    drop(db);
    for entry in std::fs::read_dir(&t.0).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "idx")
            && path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .starts_with("run-")
            && !path.file_stem().unwrap().to_string_lossy().contains('.')
        {
            assert_references(&path);
        }
    }
}
#[test]
fn native_layout_roundtrip_query_get_maintenance_and_source_bytes() {
    let t = Temp::new();
    t.seed();
    let source = source_files(&t);
    for layout in [
        Layout::BinaryPayloadCopy,
        Layout::BinarySourceReferences,
        Layout::JsonPayloadCopy,
    ] {
        Database::open(&t.0)
            .unwrap()
            .repack_index_cache(layout)
            .unwrap();
        t.verify();
        // SEG05 also exercises the resident metadata fallback used by these
        // opt-in APIs; strict open above uses the independently checked pages.
        for record_reads in [false, true] {
            let db = if record_reads {
                Database::open_record_reads(&t.0).unwrap()
            } else {
                Database::open_deferred(&t.0).unwrap()
            };
            assert_eq!(
                db.get(&Key::new("shared", "texts", "a"))
                    .unwrap()
                    .unwrap()
                    .document
                    .text,
                "İSTANBUL 🦀 quixote"
            );
            let mut q = Query::new("shared");
            q.collection = Some("texts".into());
            q.text = "quixote".into();
            assert_eq!(db.search(&q).unwrap().total, 2);
            assert_eq!(db.pending_source_checks().unwrap(), 0);
        }
        assert_eq!(source_files(&t), source);
    }
    Database::open(&t.0)
        .unwrap()
        .repack_index_cache(Layout::BinarySourceReferences)
        .unwrap();
    let mut db = Database::open(&t.0).unwrap();
    db.commit(vec![Mutation::Delete {
        key: Key::new("shared", "texts", "b"),
        guard: fdb::Guard::Any,
    }])
    .unwrap();
    db.flush().unwrap();
    db.repack_index_cache(Layout::BinarySourceReferences)
        .unwrap();
    let mut db = Database::open(&t.0).unwrap();
    db.compact().unwrap();
    db.wait_for_compaction().unwrap();
    drop(db);
    let db = Database::open(&t.0).unwrap();
    assert!(db.get(&Key::new("shared", "texts", "b")).unwrap().is_none());
    db.check_integrity().unwrap();
}
#[test]
fn every_torn_or_single_byte_corrupt_cache_fails_and_rebuilds_from_source() {
    let t = Temp::new();
    t.seed();
    Database::open(&t.0)
        .unwrap()
        .repack_index_cache(Layout::BinarySourceReferences)
        .unwrap();
    let path = t.index();
    let original = std::fs::read(&path).unwrap();
    let source = source_files(&t);
    // A bounded three-document fixture, never the benchmark corpus.
    for cut in 0..original.len() {
        std::fs::write(&path, &original[..cut]).unwrap();
        let db = Database::open(&t.0).unwrap();
        assert!(db.get(&Key::new("shared", "texts", "a")).unwrap().is_some());
    }
    for at in 0..original.len() {
        let mut bytes = original.clone();
        bytes[at] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        let db = Database::open(&t.0).unwrap();
        assert!(db.get(&Key::new("shared", "texts", "c")).unwrap().is_some());
    }
    assert_eq!(source_files(&t), source);
}
#[test]
fn intact_seg05_cannot_hide_corrupt_authoritative_source() {
    let t = Temp::new();
    t.seed();
    Database::open(&t.0)
        .unwrap()
        .repack_index_cache(Layout::BinarySourceReferences)
        .unwrap();
    let path = t.index().with_extension("fdb");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[55] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert!(Database::open(&t.0).is_err());
}

#[test]
fn crc_valid_invalid_binary_flags_counts_and_addresses_cannot_hide_behind_metadata_cache() {
    let t = Temp::new();
    t.seed();
    Database::open(&t.0)
        .unwrap()
        .repack_index_cache(Layout::BinarySourceReferences)
        .unwrap();
    let path = t.index();
    let original = std::fs::read(&path).unwrap();
    let footer = original.len() - 32;
    let directory =
        u64::from_le_bytes(original[footer + 8..footer + 16].try_into().unwrap()) as usize;
    let mut record_offset = directory + 17;
    for _ in 0..3 {
        let len = u32::from_le_bytes(
            original[record_offset..record_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        record_offset += 4 + len;
    }
    for case in 0..3 {
        let mut bytes = original.clone();
        match case {
            0 => bytes[directory + 8] = 2,
            1 => bytes[directory + 9..directory + 13].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => bytes[record_offset..record_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes()),
        }
        let prefix = crc32fast::hash(&bytes[..footer]);
        bytes[footer + 24..footer + 28].copy_from_slice(&prefix.to_le_bytes());
        let checksum = crc32fast::hash(&bytes[footer..footer + 28]);
        bytes[footer + 28..].copy_from_slice(&checksum.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        t.verify();
    }
}
