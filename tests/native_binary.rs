//! Native persistent binary format; no optional payload feature or overlay.
use fdb::{Database, Document, Error, Guard, Key, Mutation, Query};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fdb-native-binary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn document(id: &str) -> Document {
    Document {
        key: Key::new("shared", "texts", id),
        title: "İSTANBUL 東京 🦀".into(),
        text: "Şirket\n bilançosu \"ham\" \\ metin".into(),
        fields: BTreeMap::from([(
            "nested".into(),
            json!({"null":null,"bool":true,"signed":i64::MIN,"unsigned":u64::MAX,"float":-0.0,"list":[false,"東京",{"x":[1,2]}]}),
        )]),
        links: vec![Key::new("shared", "texts", "target")],
    }
}
fn put(d: Document) -> Mutation {
    Mutation::Put {
        document: d,
        guard: Guard::Any,
    }
}
fn immutable_source(path: &std::path::Path) -> PathBuf {
    fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "fdb") && p.with_extension("idx").exists())
        .unwrap()
}

#[test]
fn native_binary_roundtrip_versions_queries_and_cache_rebuild() {
    let temp = Temp::new();
    let expected = document("a");
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(expected.clone())]).unwrap();
    let before = db.get(&expected.key).unwrap().unwrap();
    db.compact().unwrap();
    let mut q = Query::new("shared");
    q.text = "istanbul bilançosu".into();
    assert_eq!(db.search(&q).unwrap().hits, vec![before.clone()]);
    drop(db);
    let source = immutable_source(&temp.0);
    let raw = fs::read(&source).unwrap();
    assert_eq!(&raw[..8], b"FDBLOG04");
    assert_eq!(&raw[8..12], b"TXN2");
    assert!(raw.windows(4).any(|w| w == b"OFF1"));
    let idx = source.with_extension("idx");
    assert_eq!(&fs::read(&idx).unwrap()[..8], b"FDBSEG04");
    assert!(!fs::read_dir(&temp.0).unwrap().any(|e| {
        e.unwrap()
            .path()
            .extension()
            .is_some_and(|s| s == "payload-bench")
    }));
    fs::remove_file(idx).unwrap();
    let mut db = Database::open(&temp.0).unwrap();
    assert_eq!(
        serde_json::to_vec(&db.get(&expected.key).unwrap().unwrap()).unwrap(),
        serde_json::to_vec(&before).unwrap()
    );
    assert_eq!(db.search(&q).unwrap().hits, vec![before]);
    let mut changed = expected.clone();
    changed.text = "yeni altın".into();
    db.commit(vec![put(changed.clone())]).unwrap();
    assert_eq!(db.search(&q).unwrap().total, 0);
    db.compact().unwrap();
    drop(db);
    let mut db = Database::open(&temp.0).unwrap();
    assert_eq!(db.get(&expected.key).unwrap().unwrap().document, changed);
    db.commit(vec![Mutation::Delete {
        key: expected.key.clone(),
        guard: Guard::Any,
    }])
    .unwrap();
    db.compact().unwrap();
    drop(db);
    assert!(
        Database::open(&temp.0)
            .unwrap()
            .get(&expected.key)
            .unwrap()
            .is_none()
    );
}

#[test]
fn intact_binary_cache_never_hides_cut_or_corrupt_published_source() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(document("a"))]).unwrap();
    db.compact().unwrap();
    drop(db);
    let path = immutable_source(&temp.0);
    let bytes = fs::read(&path).unwrap();
    let cache = fs::read(path.with_extension("idx")).unwrap();
    for cut in [0, 7, 8, 31, bytes.len() - 1] {
        fs::write(&path, &bytes[..cut]).unwrap();
        assert!(matches!(Database::open(&temp.0), Err(Error::Corrupt(_))));
        assert_eq!(fs::read(path.with_extension("idx")).unwrap(), cache);
    }
    for position in [0, 8, 28, 40, bytes.len() - 1] {
        let mut changed = bytes.clone();
        changed[position] ^= 1;
        fs::write(&path, changed).unwrap();
        assert!(matches!(Database::open(&temp.0), Err(Error::Corrupt(_))));
    }
    // A valid frame CRC must not bypass the independently published source CRC.
    // Change text, then repair both payload and header checksums, keeping .idx intact.
    let ieee_crc = |bytes: &[u8]| {
        let mut state = !0u32;
        for byte in bytes {
            state ^= *byte as u32;
            for _ in 0..8 {
                state = (state >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(state & 1));
            }
        }
        !state
    };
    let mut changed = bytes.clone();
    let position = changed
        .windows(5)
        .position(|part| part == b"bilan")
        .unwrap();
    let payload_len = u32::from_le_bytes(changed[12..16].try_into().unwrap()) as usize;
    assert!((32..32 + payload_len).contains(&position));
    changed[position] ^= 0x20;
    let payload_crc = ieee_crc(&changed[32..32 + payload_len]);
    changed[24..28].copy_from_slice(&payload_crc.to_le_bytes());
    let header_crc = ieee_crc(&changed[8..28]);
    changed[28..32].copy_from_slice(&header_crc.to_le_bytes());
    assert_ne!(ieee_crc(&changed), ieee_crc(&bytes));
    fs::write(&path, changed).unwrap();
    {
        let deferred = Database::open_deferred(&temp.0).unwrap();
        assert!(
            matches!(deferred.get(&Key::new("shared", "texts", "a")), Err(Error::Corrupt(message)) if message == "source checkpoint checksum mismatch")
        );
    }
    assert!(
        matches!(Database::open(&temp.0), Err(Error::Corrupt(message)) if message == "source checkpoint checksum mismatch")
    );
    assert_eq!(fs::read(path.with_extension("idx")).unwrap(), cache);
    // A provisional rebuild must not publish a cache for a bad authoritative source.
    fs::remove_file(path.with_extension("idx")).unwrap();
    assert!(
        matches!(Database::open(&temp.0), Err(Error::Corrupt(message)) if message == "source checkpoint checksum mismatch")
    );
    assert!(!path.with_extension("idx").exists());
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        Database::open(&temp.0).unwrap().stats().unwrap().documents,
        1
    );
}

#[test]
fn deferred_open_checks_source_before_reads_searches_and_writes() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    let expected = document("a");
    db.commit(vec![put(expected.clone())]).unwrap();
    db.compact().unwrap();
    drop(db);
    let source = immutable_source(&temp.0);
    let original = fs::read(&source).unwrap();
    let cache = fs::read(source.with_extension("idx")).unwrap();
    let mut q = Query::new("shared");
    q.text = "missing".into();
    let mut changed = original.clone();
    let position = changed
        .windows(5)
        .position(|part| part == b"bilan")
        .unwrap();
    changed[position] ^= 1;
    fs::write(&source, &changed).unwrap();
    let mut db = Database::open_deferred(&temp.0).unwrap();
    assert_eq!(db.pending_source_checks().unwrap(), 1);
    assert!(matches!(db.get(&expected.key), Err(Error::Corrupt(_))));
    assert!(matches!(
        db.get(&Key::new("shared", "texts", "missing")),
        Err(Error::Corrupt(_))
    ));
    assert!(matches!(db.search(&q), Err(Error::Corrupt(_))));
    assert!(matches!(db.check_integrity(), Err(Error::Corrupt(_))));
    let snapshot: BTreeMap<_, _> = fs::read_dir(&temp.0)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|path| path.file_name().is_some_and(|name| name != "LOCK"))
        .map(|path| (path.clone(), fs::read(path).unwrap()))
        .collect();
    assert!(matches!(
        db.commit(vec![put(document("b"))]),
        Err(Error::Corrupt(_))
    ));
    assert!(matches!(db.flush(), Err(Error::Corrupt(_))));
    assert!(matches!(db.start_compaction(), Err(Error::Corrupt(_))));
    for (path, bytes) in snapshot {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(db.pending_source_checks().unwrap(), 1);
    drop(db);
    assert_eq!(fs::read(source.with_extension("idx")).unwrap(), cache);

    fs::write(&source, &original).unwrap();
    let db = Database::open_deferred(&temp.0).unwrap();
    assert_eq!(db.get(&expected.key).unwrap().unwrap().document, expected);
    assert_eq!(db.pending_source_checks().unwrap(), 0);
    // Explicit full checks revisit previously verified sources.
    #[cfg(all(windows, feature = "bounded-engine"))]
    let db = {
        assert!(fs::write(&source, &changed).is_err());
        db.check_integrity().unwrap();
        drop(db);
        fs::write(&source, &changed).unwrap();
        Database::open_deferred(&temp.0).unwrap()
    };
    #[cfg(not(all(windows, feature = "bounded-engine")))]
    fs::write(&source, &changed).unwrap();
    assert!(matches!(db.check_integrity(), Err(Error::Corrupt(_))));
    drop(db);
    fs::write(&source, &original).unwrap();
    fs::remove_file(source.with_extension("idx")).unwrap();
    let db = Database::open_deferred(&temp.0).unwrap();
    assert_eq!(db.pending_source_checks().unwrap(), 0);
    db.check_integrity().unwrap();
    assert!(source.with_extension("idx").exists());
    assert_eq!(fs::read(&source).unwrap(), original);
    drop(db);
    let mut db = Database::open_deferred(&temp.0).unwrap();
    db.commit(vec![put(document("b"))]).unwrap();
    db.check_integrity().unwrap();
    db.flush().unwrap();
    db.check_integrity().unwrap();
}

#[test]
fn explicit_log_check_rejects_partial_tail_without_repairing_it() {
    use std::io::Write;
    let temp = Temp::new();
    let db = Database::open_deferred(&temp.0).unwrap();
    db.check_integrity().unwrap();
    let active = fs::read_dir(&temp.0)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "fdb"))
        .unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&active)
        .unwrap()
        .write_all(b"TXN2")
        .unwrap();
    let bytes = fs::read(&active).unwrap();
    assert!(matches!(db.check_integrity(), Err(Error::Corrupt(_))));
    assert_eq!(fs::read(active).unwrap(), bytes);
}

#[test]
fn block_cache_is_optional_and_does_not_hide_bad_sources_or_change_seg04() {
    let temp = Temp::new();
    let expected = document("a");
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(expected.clone())]).unwrap();
    db.compact().unwrap();
    let source = immutable_source(&temp.0);
    let idx = source.with_extension("idx");
    let before_source = fs::read(&source).unwrap();
    let before_index = fs::read(&idx).unwrap();
    db.prepare_open_checks().unwrap();
    drop(db);
    let cache = fs::read(source.with_extension("vfy")).unwrap();
    assert_eq!(&cache[..8], b"FDBVFY01");
    assert_eq!(fs::read(&source).unwrap(), before_source);
    assert_eq!(fs::read(&idx).unwrap(), before_index);
    for bytes in [&cache[..7], &cache[..cache.len() - 1], cache.as_slice()] {
        fs::write(source.with_extension("vfy"), bytes).unwrap();
        let db = Database::open_deferred(&temp.0).unwrap();
        assert_eq!(db.get(&expected.key).unwrap().unwrap().document, expected);
    }
    let mut corrupt = before_source.clone();
    let n = corrupt.windows(5).position(|w| w == b"bilan").unwrap();
    corrupt[n] ^= 1;
    fs::write(&source, &corrupt).unwrap();
    let db = Database::open_deferred(&temp.0).unwrap();
    assert!(matches!(db.get(&expected.key), Err(Error::Corrupt(_))));
    drop(db);
    fs::write(&source, &before_source).unwrap();
    // Full/old readers ignore VFY01 and still accept the unmodified SEG04.
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.get(&expected.key).unwrap().unwrap().document, expected);
    drop(db);
    fs::remove_file(source.with_extension("vfy")).unwrap();
    let db = Database::open_deferred(&temp.0).unwrap();
    assert_eq!(db.get(&expected.key).unwrap().unwrap().document, expected);
    assert_eq!(fs::read(&source).unwrap(), before_source);
    assert_eq!(fs::read(&idx).unwrap(), before_index);
}

#[test]
fn deferred_index_rejects_an_unopened_payload_block_before_returning_document() {
    let temp = Temp::new();
    let mut expected = document("large");
    expected.text = "x".repeat(700 * 1024);
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(expected.clone())]).unwrap();
    db.compact().unwrap();
    db.prepare_open_checks().unwrap();
    drop(db);
    let source = immutable_source(&temp.0);
    let idx = source.with_extension("idx");
    let source_bytes = fs::read(&source).unwrap();
    let mut bytes = fs::read(&idx).unwrap();
    bytes[300 * 1024] ^= 1;
    fs::write(&idx, bytes).unwrap();
    let db = Database::open_deferred(&temp.0).unwrap();
    assert_eq!(db.pending_source_checks().unwrap(), 1);
    assert!(
        matches!(db.get(&expected.key), Err(Error::Corrupt(message)) if message == "index block checksum mismatch")
    );
    drop(db);
    // A strict reader can rebuild this disposable bad index from the valid source.
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.get(&expected.key).unwrap().unwrap().document, expected);
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
}

#[test]
fn source_record_addresses_check_actual_source_and_fall_back_for_bad_caches() {
    let temp = Temp::new();
    let mut large = document("z");
    large.text = "x".repeat(700 * 1024);
    let first = document("a");
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(first.clone()), put(large.clone())])
        .unwrap();
    db.compact().unwrap();
    db.prepare_open_checks().unwrap();
    db.prepare_record_reads().unwrap();
    drop(db);
    let source = immutable_source(&temp.0);
    let original = fs::read(&source).unwrap();
    let idx = fs::read(source.with_extension("idx")).unwrap();
    let map = fs::read(source.with_extension("rmp")).unwrap();
    let checks = fs::read(source.with_extension("svfy")).unwrap();
    assert_eq!(&map[..8], b"FDBREF01");
    let db = Database::open_record_reads(&temp.0).unwrap();
    assert_eq!(db.pending_source_checks().unwrap(), 1);
    assert_eq!(db.get(&first.key).unwrap().unwrap().document, first);
    assert_eq!(db.pending_source_checks().unwrap(), 1);
    assert_eq!(db.get(&large.key).unwrap().unwrap().document, large);
    db.check_integrity().unwrap();
    drop(db);
    let mut corrupt = original.clone();
    corrupt[300 * 1024] ^= 1;
    fs::write(&source, &corrupt).unwrap();
    let mut db = Database::open_record_reads(&temp.0).unwrap();
    // An untouched unrelated block is not claimed verified by a successful read.
    assert_eq!(db.get(&first.key).unwrap().unwrap().document, first);
    assert_eq!(db.pending_source_checks().unwrap(), 1);
    assert!(matches!(db.get(&large.key), Err(Error::Corrupt(_))));
    assert!(matches!(
        db.get(&Key::new("shared", "texts", "absent")),
        Err(Error::Corrupt(_))
    ));
    let mut q = Query::new("shared");
    q.text = "notfound".into();
    assert!(matches!(db.search(&q), Err(Error::Corrupt(_))));
    assert!(matches!(
        db.commit(vec![put(document("b"))]),
        Err(Error::Corrupt(_))
    ));
    drop(db);
    fs::write(&source, &original).unwrap();
    // Cache framing, CRC and source/index identity are all checked before use.
    for extension in ["rmp", "svfy"] {
        let full = if extension == "rmp" { &map } else { &checks };
        for cut in 0..full.len() {
            fs::write(source.with_extension(extension), &full[..cut]).unwrap();
            let db = Database::open_record_reads(&temp.0).unwrap();
            assert_eq!(db.get(&first.key).unwrap().unwrap().document, first);
            assert_eq!(db.pending_source_checks().unwrap(), 0);
        }
        for position in 0..full.len() {
            let mut changed = full.clone();
            changed[position] ^= 1;
            fs::write(source.with_extension(extension), changed).unwrap();
            let db = Database::open_record_reads(&temp.0).unwrap();
            assert_eq!(db.get(&first.key).unwrap().unwrap().document, first);
            assert_eq!(db.pending_source_checks().unwrap(), 0);
        }
        fs::write(source.with_extension(extension), full).unwrap();
    }
    // A CRC-valid but wrong byte address must not return index-cached payload.
    let mut wrong = map.clone();
    let offset = u64::from_le_bytes(wrong[36..44].try_into().unwrap());
    wrong[36..44].copy_from_slice(&(offset + 1).to_le_bytes());
    let end = wrong.len() - 4;
    let mut state = !0u32;
    for byte in &wrong[..end] {
        state ^= *byte as u32;
        for _ in 0..8 {
            state = (state >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(state & 1));
        }
    }
    wrong[end..].copy_from_slice(&(!state).to_le_bytes());
    fs::write(source.with_extension("rmp"), wrong).unwrap();
    let db = Database::open_record_reads(&temp.0).unwrap();
    assert!(matches!(db.get(&first.key), Err(Error::Corrupt(_))));
    drop(db);
    fs::remove_file(source.with_extension("rmp")).unwrap();
    let db = Database::open_record_reads(&temp.0).unwrap();
    assert_eq!(db.get(&first.key).unwrap().unwrap().document, first);
    drop(db);
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.get(&large.key).unwrap().unwrap().document, large);
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(fs::read(source.with_extension("idx")).unwrap(), idx);
}

#[test]
fn source_record_maps_preserve_newer_versions_and_tombstones() {
    let temp = Temp::new();
    let a = document("a");
    let b = document("b");
    let c = document("c");
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(a.clone()), put(b.clone()), put(c.clone())])
        .unwrap();
    db.flush().unwrap();
    let mut updated = a.clone();
    updated.text = "updated source".into();
    db.commit(vec![
        put(updated.clone()),
        Mutation::Delete {
            key: b.key.clone(),
            guard: Guard::Any,
        },
    ])
    .unwrap();
    db.flush().unwrap();
    db.prepare_open_checks().unwrap();
    db.prepare_record_reads().unwrap();
    drop(db);
    let db = Database::open_record_reads(&temp.0).unwrap();
    let stored = db.get(&a.key).unwrap().unwrap();
    assert_eq!(stored.version, 2);
    assert_eq!(stored.document, updated);
    assert_eq!(db.get(&b.key).unwrap(), None);
    let stored = db.get(&c.key).unwrap().unwrap();
    assert_eq!(stored.version, 1);
    assert_eq!(stored.document, c);
    db.check_integrity().unwrap();
    assert_eq!(db.pending_source_checks().unwrap(), 0);
}

#[test]
fn binary_size_limit_and_depth_fail_before_append() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    let initial = db.stats().unwrap().log_bytes;
    let mut d = document("a");
    d.fields.insert("large".into(), json!(vec![0; 600_000]));
    assert!(matches!(db.commit(vec![put(d)]), Err(Error::Invalid(_))));
    let mut d = document("a");
    let mut v = json!(null);
    for _ in 0..130 {
        v = json!([v]);
    }
    d.fields.insert("deep".into(), v);
    assert!(matches!(db.commit(vec![put(d)]), Err(Error::Invalid(_))));
    assert_eq!(db.stats().unwrap().log_bytes, initial);
}
