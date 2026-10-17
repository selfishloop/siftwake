use fdb::{Database, Document, Error, Guard, Key, Mutation, Options, Query, Range};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fdb-disk-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn index(&self) -> PathBuf {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.extension().is_some_and(|s| s == "idx")
                    && !p
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .ends_with(".records04.idx")
            })
            .max()
            .unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn doc(id: &str, text: &str) -> Document {
    Document {
        key: Key::new("shared", "reports", id),
        title: "Rapor".into(),
        text: text.into(),
        fields: BTreeMap::from([("day".into(), json!(5)), ("category".into(), json!("tech"))]),
        links: vec![Key::new("shared", "funds", "ABC")],
    }
}
fn put(d: Document) -> Mutation {
    Mutation::Put {
        document: d,
        guard: Guard::Any,
    }
}

#[test]
fn checkpoint_evicts_content_and_postings_from_resident_state() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(doc("a", &"teknoloji ".repeat(100_000)))])
        .unwrap();
    assert!(db.stats().unwrap().resident_text_bytes > 1_000_000);
    db.compact().unwrap();
    let stats = db.stats().unwrap();
    assert_eq!(stats.resident_documents, 0);
    assert_eq!(stats.resident_text_bytes, 0);
    assert_eq!(stats.delta_keys, 0);
    assert!(stats.disk_posting_entries > 0);
    assert_eq!(
        db.get(&doc("a", "").key)
            .unwrap()
            .unwrap()
            .document
            .text
            .len(),
        1_000_000
    );
    drop(db);
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().resident_text_bytes, 0);
    let mut q = Query::new("shared");
    q.text = "TEKNOLOJİ".into();
    assert_eq!(db.search(&q).unwrap().total, 1);
}

#[test]
fn disk_delta_merge_masks_stale_results_and_preserves_page_order() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![
        put(doc("a", "eski teknoloji")),
        put(doc("c", "eski teknoloji")),
        put(doc("e", "eski teknoloji")),
    ])
    .unwrap();
    db.compact().unwrap();
    db.commit(vec![
        put(doc("b", "yeni teknoloji")),
        put(doc("c", "yeni altın")),
        Mutation::Delete {
            key: doc("e", "").key,
            guard: Guard::Version { version: 1 },
        },
    ])
    .unwrap();
    let mut q = Query::new("shared");
    q.text = "teknoloji".into();
    q.limit = 1;
    let page = db.search(&q).unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.hits[0].document.key.id, "a");
    q.offset = 1;
    assert_eq!(db.search(&q).unwrap().hits[0].document.key.id, "b");
    q.offset = usize::MAX;
    assert!(db.search(&q).unwrap().hits.is_empty());
    drop(db);
    let mut db = Database::open(&temp.0).unwrap();
    q.offset = 0;
    assert_eq!(db.search(&q).unwrap().total, 2);
    db.compact().unwrap();
    assert_eq!(db.search(&q).unwrap().total, 2);
    assert!(db.get(&doc("e", "").key).unwrap().is_none());
}

#[test]
fn deleted_disk_fields_and_links_never_leak_through_old_postings() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(doc("a", "eski"))]).unwrap();
    db.compact().unwrap();
    let mut replacement = doc("a", "yeni");
    replacement.fields.clear();
    replacement.links.clear();
    db.commit(vec![put(replacement)]).unwrap();
    let mut q = Query::new("shared");
    q.exact.insert("category".into(), json!("tech"));
    assert_eq!(db.search(&q).unwrap().total, 0);
    q.exact.clear();
    q.ranges.push(Range {
        field: "day".into(),
        min: 0,
        max: 10,
    });
    assert_eq!(db.search(&q).unwrap().total, 0);
    q.ranges.clear();
    q.linked_to = Some(Key::new("shared", "funds", "ABC"));
    assert_eq!(db.search(&q).unwrap().total, 0);
}

#[test]
fn missing_corrupt_and_wrong_generation_segments_rebuild_from_log() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(doc("a", "teknoloji"))]).unwrap();
    db.compact().unwrap();
    drop(db);
    let target = temp.index();
    let good = fs::read(&target).unwrap();
    fs::remove_file(&target).unwrap();
    let db = Database::open(&temp.0).unwrap();
    assert!(db.get(&doc("a", "").key).unwrap().is_some());
    drop(db);
    for position in [0, 8, 27, 28, good.len() / 2, good.len() - 1] {
        let mut bad = good.clone();
        bad[position] ^= 1;
        fs::write(&target, bad).unwrap();
        let db = Database::open(&temp.0).unwrap();
        assert_eq!(
            db.get(&doc("a", "").key).unwrap().unwrap().document.text,
            "teknoloji"
        );
        drop(db);
    }
    let donor = Temp::new();
    let mut other = Database::open(&donor.0).unwrap();
    other
        .commit(vec![put(doc("other", "başka kaynak"))])
        .unwrap();
    other.flush().unwrap();
    drop(other);
    let old = fs::read(donor.index()).unwrap();
    fs::write(&target, old).unwrap();
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 1);
    assert_eq!(db.stats().unwrap().resident_documents, 0);
}

#[test]
fn automatic_maintenance_preserves_guards_and_does_not_run_for_rejected_batch() {
    let temp = Temp::new();
    let options = Options {
        max_delta_keys: 2,
        max_delta_bytes: 16 * 1024 * 1024,
    };
    let mut db = Database::open_with_options(&temp.0, options).unwrap();
    db.commit(vec![put(doc("a", "a")), put(doc("b", "b"))])
        .unwrap();
    let generation = db.stats().unwrap().generation;
    assert!(matches!(
        db.commit(vec![Mutation::Delete {
            key: doc("a", "").key,
            guard: Guard::Version { version: 9 }
        }]),
        Err(Error::Conflict(_))
    ));
    assert_eq!(db.stats().unwrap().generation, generation);
    assert_eq!(
        db.commit(vec![Mutation::Put {
            document: doc("a", "updated"),
            guard: Guard::Version { version: 1 }
        }])
        .unwrap(),
        2
    );
    assert!(db.stats().unwrap().generation > generation);
    assert_eq!(
        db.stats().unwrap().resident_documents,
        usize::from(!cfg!(feature = "experimental-wal-delta"))
    );
    assert_eq!(db.get(&doc("b", "").key).unwrap().unwrap().version, 1);
    let before = db.stats().unwrap().log_bytes;
    assert!(matches!(
        db.commit(vec![Mutation::Put {
            document: doc("b", "bad"),
            guard: Guard::Missing
        }]),
        Err(Error::Conflict(_))
    ));
    assert_eq!(db.stats().unwrap().log_bytes, before);
}

#[test]
fn numeric_groups_unicode_and_exact_filters_work_across_collections() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    let mut mutations = vec![];
    for n in 0..20 {
        let mut d = doc(&format!("{n:02}"), "東京 ŞİRKETİN 🦀 teknoloji");
        d.key.collection = if n % 2 == 0 {
            "reports".into()
        } else {
            "posts".into()
        };
        d.fields.insert("day".into(), json!(n - 10));
        mutations.push(put(d));
    }
    db.commit(mutations).unwrap();
    db.compact().unwrap();
    let mut q = Query::new("shared");
    q.text = "東京 🦀 sirketin".into();
    q.exact.insert("category".into(), json!("tech"));
    q.ranges.push(Range {
        field: "day".into(),
        min: -3,
        max: 4,
    });
    assert_eq!(db.search(&q).unwrap().total, 8);
    q.collection = Some("reports".into());
    assert_eq!(db.search(&q).unwrap().total, 4);
    q.ranges[0].min = i64::MIN;
    q.ranges[0].max = i64::MAX;
    assert_eq!(db.search(&q).unwrap().total, 10);
}

#[test]
fn runtime_document_corruption_is_reported_before_returning_payload() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(doc("a", "hello"))]).unwrap();
    db.compact().unwrap();
    // Keep coverage of runtime corruption in legacy index payload copies.
    // Native default payloads now live only in the checked, leased source.
    #[cfg(feature = "bounded-engine")]
    {
        db.repack_index_cache(fdb::IndexCacheLayout::JsonPayloadCopy)
            .unwrap();
        db = Database::open(&temp.0).unwrap();
    }
    let target = temp.index();
    let mut bytes = fs::read(&target).unwrap();
    bytes[30] ^= 1;
    fs::write(&target, bytes).unwrap();
    assert!(matches!(db.get(&doc("a", "").key), Err(Error::Corrupt(_))));
    let mut text_query = Query::new("shared");
    text_query.text = "hello".into();
    assert!(matches!(db.search(&text_query), Err(Error::Corrupt(_))));
    // A metadata-only query hydrates directly from the selected run/slot.
    // It must still check the payload CRC, even without text verification.
    assert!(matches!(
        db.search(&Query::new("shared")),
        Err(Error::Corrupt(_))
    ));
    #[cfg(feature = "experimental-direct-slot")]
    assert!(matches!(
        db.search_direct_slots(&Query::new("shared")),
        Err(Error::Corrupt(_))
    ));
    #[cfg(feature = "experimental-grouped-read")]
    assert!(matches!(
        db.search_grouped_slots(&Query::new("shared")),
        Err(Error::Corrupt(_))
    ));
}

#[test]
fn delta_byte_watermark_bounds_resident_long_content_after_next_write() {
    let temp = Temp::new();
    let mut db = Database::open_with_options(
        &temp.0,
        Options {
            max_delta_keys: 1000,
            max_delta_bytes: 4096,
        },
    )
    .unwrap();
    db.commit(vec![put(doc("a", &"metin ".repeat(1000)))])
        .unwrap();
    assert_eq!(
        db.stats().unwrap().resident_documents,
        usize::from(!cfg!(feature = "experimental-wal-delta"))
    );
    db.commit(vec![put(doc("b", "next"))]).unwrap();
    assert_eq!(
        db.stats().unwrap().resident_documents,
        usize::from(!cfg!(feature = "experimental-wal-delta"))
    );
    assert_eq!(db.stats().unwrap().documents, 2);
    drop(db);
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(
        db.stats().unwrap().resident_documents,
        usize::from(!cfg!(feature = "experimental-wal-delta"))
    );
}

#[test]
fn postings_cross_block_boundaries_without_losing_page_entries() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(
        (0..2500)
            .map(|n| put(doc(&format!("{n:05}"), "ortak teknoloji")))
            .collect(),
    )
    .unwrap();
    db.compact().unwrap();
    let mut q = Query::new("shared");
    q.text = "teknoloji".into();
    q.offset = 1020;
    q.limit = 10;
    let page = db.search(&q).unwrap();
    assert_eq!(page.total, 2500);
    assert_eq!(page.hits[0].document.key.id, "01020");
    assert_eq!(page.hits[9].document.key.id, "01029");
    q.offset = 2499;
    assert_eq!(db.search(&q).unwrap().hits.len(), 1);
}

#[test]
fn v01_checkpoint_fixture_opens_without_rewriting_authoritative_bytes() {
    let temp = Temp::new();
    let original = include_bytes!("fixtures/v1/checkpoint.fdb");
    let log = temp.0.join("generation-0000000000000002.fdb");
    fs::write(&log, original).unwrap();
    fs::write(
        temp.0.join("generation-0000000000000002.ready"),
        include_bytes!("fixtures/v1/checkpoint.ready"),
    )
    .unwrap();
    let mut db = Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 4);
    assert_eq!(db.stats().unwrap().resident_documents, 0);
    assert_eq!(fs::read(&log).unwrap(), original);
    let mut q = Query::new("shared");
    q.text = "TEKNOLOJİ yatırımları".into();
    q.linked_to = Some(Key::new("shared", "funds", "ABC"));
    assert_eq!(db.search(&q).unwrap().total, 2);
    assert_eq!(
        db.commit(vec![put(doc("new-v02", "yeni sürüm"))]).unwrap(),
        2
    );
    db.compact().unwrap();
    db.wait_for_compaction().unwrap();
    assert!(fs::read(&log).unwrap().starts_with(original));
    drop(db);
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 5);
    assert_eq!(db.search(&q).unwrap().total, 2);
}

#[test]
fn killing_compaction_before_publication_preserves_acknowledged_data() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let temp = Temp::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fdb"))
        .arg(&temp.0)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mutations: Vec<_> = (0..8)
        .map(|n| put(doc(&n.to_string(), &"x".repeat(500_000))))
        .collect();
    let request = json!({"op":"commit", "mutations":mutations});
    let input = child.stdin.as_mut().unwrap();
    writeln!(input, "{request}").unwrap();
    input.flush().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&line).unwrap()["ok"],
        true
    );
    let source = temp.0.join("generation-0000000000000001.fdb");
    let original = fs::read(&source).unwrap();
    let input = child.stdin.as_mut().unwrap();
    writeln!(input, "{{\"op\":\"compact\"}}").unwrap();
    input.flush().unwrap();
    let pending_log = temp.0.join("run-0000000000000002.fdb");
    let started = Instant::now();
    while !pending_log.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "compaction did not start"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!temp.0.join("generation-0000000000000003.ready").exists());
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 8);
    assert_eq!(db.stats().unwrap().sequence, 1);
    assert_eq!(fs::read(source).unwrap(), original);
    for id in 0..8 {
        assert_eq!(
            db.get(&doc(&id.to_string(), "").key)
                .unwrap()
                .unwrap()
                .document
                .text
                .len(),
            500_000
        );
    }
}
