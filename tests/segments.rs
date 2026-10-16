use fdb::{Database, Document, Error, Guard, Key, Mutation, Options, Query, Range};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "experimental-merge-postings")]
#[test]
fn merge_reuse_rejects_runtime_source_or_index_corruption_before_publication() {
    for extension in ["fdb", "idx"] {
        let temp = Temp::new();
        let mut db = Database::open(&temp.0).unwrap();
        db.commit(vec![put(doc("a", "teknoloji", 1))]).unwrap();
        db.flush().unwrap();
        db.commit(vec![put(doc("b", "yatırım", 2))]).unwrap();
        db.flush().unwrap();
        let marker = temp.ready();
        let ready = fs::read(&marker).unwrap();
        let path = temp.source(0, extension);
        let mut bytes = fs::read(&path).unwrap();
        bytes[32] ^= 0x40;
        #[cfg(all(windows, feature = "bounded-engine"))]
        if extension == "fdb" {
            assert!(fs::write(&path, &bytes).is_err());
            // Existing source handles also remain valid for the merge worker.
            assert!(db.start_compaction().unwrap());
            db.wait_for_compaction().unwrap();
            assert_eq!(db.stats().unwrap().documents, 2);
            drop(db);
            // Corrupt the newly published live source after owners close.
            let live = temp.source(0, extension);
            let mut bytes = fs::read(&live).unwrap();
            bytes[32] ^= 0x40;
            fs::write(live, bytes).unwrap();
            assert!(matches!(Database::open(&temp.0), Err(Error::Corrupt(_))));
            continue;
        }
        fs::write(&path, &bytes).unwrap();
        assert!(db.start_compaction().unwrap());
        assert!(matches!(db.wait_for_compaction(), Err(Error::Corrupt(_))));
        assert_eq!(fs::read(&marker).unwrap(), ready);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        drop(db);
        if extension == "fdb" {
            assert!(matches!(Database::open(&temp.0), Err(Error::Corrupt(_))));
        } else {
            assert_eq!(
                Database::open(&temp.0).unwrap().stats().unwrap().documents,
                2
            );
        }
    }
}

#[cfg(feature = "experimental-streaming-open")]
#[test]
fn streaming_open_preserves_large_exact_fields_on_rebuild_and_reopen() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    let mut document = doc("large-field", "small text", 2);
    let value = json!("şirket\\\"".repeat(40_000));
    document.fields.insert("large".into(), value.clone());
    db.commit(vec![put(document.clone())]).unwrap();
    db.flush().unwrap();
    let mut query = Query::new("shared");
    query.exact.insert("large".into(), value);
    for rebuild in [false, true] {
        drop(db);
        if rebuild {
            for entry in fs::read_dir(&temp.0).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|extension| extension == "idx") {
                    fs::remove_file(path).unwrap();
                }
            }
        }
        db = Database::open(&temp.0).unwrap();
        assert_eq!(db.get(&document.key).unwrap().unwrap().document, document);
        let page = db.search(&query).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.hits[0].document, document);
    }
}

#[cfg(feature = "experimental-paged-metadata")]
#[test]
fn paged_metadata_deep_offsets_interleaved_runs_and_delta_match_model() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    let mut model = BTreeMap::new();
    for run in 0..3 {
        let mut mutations = Vec::new();
        for id in (run..7200).step_by(3) {
            let document = doc(&format!("{id:08}"), "içerik", id as i64);
            model.insert(document.key.clone(), document.clone());
            mutations.push(put(document));
        }
        db.commit(mutations).unwrap();
        db.flush().unwrap();
    }
    let mut mutations = Vec::new();
    for id in (0..7200).step_by(359) {
        let key = Key::new("shared", "reports", &format!("{id:08}"));
        model.remove(&key);
        mutations.push(Mutation::Delete {
            key,
            guard: Guard::Any,
        });
    }
    db.commit(mutations).unwrap();
    for reopen in [false, true] {
        if reopen {
            drop(db);
            db = Database::open(&temp.0).unwrap();
        }
        for offset in [0, 4090, 4096, 4300, 6900, 7200, usize::MAX] {
            let mut q = Query::new("shared");
            q.offset = offset;
            q.limit = 50;
            let page = db.search(&q).unwrap();
            assert_eq!(page.total, model.len());
            let expected: Vec<_> = model
                .values()
                .skip(offset)
                .take(50)
                .map(|doc| serde_json::to_value(doc).unwrap())
                .collect();
            let actual: Vec<_> = page
                .hits
                .iter()
                .map(|stored| serde_json::to_value(&stored.document).unwrap())
                .collect();
            assert_eq!(actual, expected, "offset {offset}, reopened {reopen}");
        }
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "fdb-segments-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn ready(&self) -> PathBuf {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|s| s == "ready"))
            .max()
            .unwrap()
    }
    fn manifest(&self) -> Value {
        let bytes = fs::read(self.ready()).unwrap();
        serde_json::from_slice(&bytes[20..bytes.len() - 4]).unwrap()
    }
    fn source(&self, position: usize, extension: &str) -> PathBuf {
        let m = self.manifest();
        let s = &m["runs"][position];
        let stem = if s["kind"] == "Run" {
            "run"
        } else {
            "generation"
        };
        self.0.join(format!(
            "{stem}-{:016x}.{extension}",
            s["id"].as_u64().unwrap()
        ))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn doc(id: &str, text: &str, day: i64) -> Document {
    Document {
        key: Key::new("shared", "reports", id),
        title: "belge".into(),
        text: text.into(),
        fields: BTreeMap::from([
            ("day".into(), json!(day)),
            (
                "tag".into(),
                json!(if day % 2 == 0 { "even" } else { "odd" }),
            ),
        ]),
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
fn bounded_first_page_merges_numeric_unions_and_newer_visibility_in_key_order() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    let mut expected = BTreeMap::new();
    for round in 0..3 {
        let mut changes = vec![];
        for n in (round * 80..(round + 1) * 80).rev() {
            let mut d = doc(&format!("{n:04}"), "teknoloji", n % 5);
            d.key.collection = format!("reports-{}", n % 2);
            expected.insert(d.key.clone(), d.clone());
            changes.push(put(d));
        }
        db.commit(changes).unwrap();
        db.flush().unwrap();
    }
    for (round, positions) in [(0, vec![1, 10, 89, 170]), (1, vec![5, 17, 91, 190])] {
        let mut changes = vec![];
        for n in positions {
            let key = Key::new("shared", &format!("reports-{}", n % 2), &format!("{n:04}"));
            if n % 2 == 0 {
                expected.remove(&key);
                changes.push(Mutation::Delete {
                    key,
                    guard: Guard::Any,
                });
            } else {
                let mut d = expected[&key].clone();
                d.fields.insert("day".into(), json!(3));
                d.text = "değişen teknoloji".into();
                expected.insert(key, d.clone());
                changes.push(put(d));
            }
        }
        db.commit(changes).unwrap();
        if round == 0 {
            db.flush().unwrap();
        }
    }
    let wanted: Vec<_> = expected
        .values()
        .filter(|d| (1..=3).contains(&d.fields["day"].as_i64().unwrap()))
        .cloned()
        .collect();
    let mut q = Query::new("shared");
    q.ranges.push(Range {
        field: "day".into(),
        min: 1,
        max: 3,
    });
    q.text = "teknoloji".into();
    q.limit = 7;
    for reopen in [false, true] {
        if reopen {
            drop(db);
            db = Database::open(&t.0).unwrap();
        }
        for offset in [0, 7, wanted.len() - 3, wanted.len() + 1] {
            q.offset = offset;
            let page = db.search(&q).unwrap();
            assert_eq!(page.total, wanted.len());
            #[cfg(feature = "experimental-grouped-read")]
            assert_eq!(
                serde_json::to_value(db.search_grouped_slots(&q).unwrap()).unwrap(),
                serde_json::to_value(&page).unwrap()
            );
            #[cfg(feature = "experimental-direct-slot")]
            assert_eq!(
                serde_json::to_value(db.search_direct_slots(&q).unwrap()).unwrap(),
                serde_json::to_value(&page).unwrap()
            );
            // Independent key lookup resolves newest versions; slot hydration
            // must return the same full StoredDocument across runs and delta.
            for hit in &page.hits {
                assert_eq!(db.get(&hit.document.key).unwrap().as_ref(), Some(hit));
            }
            let actual: Vec<_> = page.hits.into_iter().map(|s| s.document).collect();
            let end = (offset + q.limit).min(wanted.len());
            assert_eq!(actual, wanted[offset.min(wanted.len())..end]);
        }
    }
}

#[test]
fn full_filter_prefix_preserves_complete_hits_and_falls_back_for_hidden_slots() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    let mut expected = (0..12)
        .map(|id| {
            let mut value = doc(&format!("{id:04}"), "the man 東京🦀", 2);
            value.title = "THE".into();
            fdb::StoredDocument {
                document: value,
                version: 1,
            }
        })
        .collect::<Vec<_>>();
    db.commit(
        expected
            .iter()
            .rev()
            .map(|stored| put(stored.document.clone()))
            .collect(),
    )
    .unwrap();
    db.flush().unwrap();
    fn check(db: &Database, rows: &[fdb::StoredDocument]) {
        let mut query = Query::new("shared");
        query.collection = Some("reports".into());
        query.exact.insert("tag".into(), json!("even"));
        query.ranges.push(Range {
            field: "day".into(),
            min: 1,
            max: 3,
        });
        query.linked_to = Some(Key::new("shared", "funds", "ABC"));
        let wanted = rows
            .iter()
            .filter(|stored| stored.document.fields["day"] == json!(2))
            .cloned()
            .collect::<Vec<_>>();
        for text in ["", "THE", "he", "the man", "東京🦀"] {
            query.text = text.into();
            for limit in [1, 3] {
                query.limit = limit;
                for offset in [0, 1, 10, 12, 5000] {
                    query.offset = offset;
                    let page = db.search(&query).unwrap();
                    assert_eq!(page.total, wanted.len(), "{text}/{offset}/{limit}");
                    assert_eq!(
                        page.hits,
                        wanted
                            .iter()
                            .skip(offset)
                            .take(limit)
                            .cloned()
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
        query.offset = 0;
        for collection in ["missing", "reports"] {
            query.collection = Some(collection.into());
            query.text = "absent".into();
            let page = db.search(&query).unwrap();
            assert_eq!(page.total, 0);
            assert!(page.hits.is_empty());
        }
    }
    check(&db, &expected);
    db.commit(vec![
        Mutation::Delete {
            key: expected[0].document.key.clone(),
            guard: Guard::Version { version: 1 },
        },
        put(doc("0001", "replacement", 3)),
    ])
    .unwrap();
    expected.remove(0);
    expected[0] = db.get(&expected[0].document.key).unwrap().unwrap();
    check(&db, &expected);
    db.flush().unwrap();
    check(&db, &expected);
    drop(db);
    check(&Database::open(&temp.0).unwrap(), &expected);
}

#[test]
fn full_filter_count_excludes_hidden_tombstones_and_unflushed_deletes() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    let deleted = Key::new("shared", "reports", "never-present");
    let mut initial = ["a", "b", "c", "d"]
        .into_iter()
        .map(|id| put(doc(id, "teknoloji", 2)))
        .collect::<Vec<_>>();
    initial.push(Mutation::Delete {
        key: deleted.clone(),
        guard: Guard::Any,
    });
    db.commit(initial).unwrap();
    db.flush().unwrap();
    db.commit(vec![
        put(doc("a", "yeni teknoloji", 2)),
        Mutation::Delete {
            key: Key::new("shared", "reports", "b"),
            guard: Guard::Any,
        },
        Mutation::Delete {
            key: deleted,
            guard: Guard::Any,
        },
    ])
    .unwrap();
    db.flush().unwrap();
    db.commit(vec![Mutation::Delete {
        key: Key::new("shared", "reports", "c"),
        guard: Guard::Any,
    }])
    .unwrap();
    let mut q = Query::new("shared");
    q.collection = Some("reports".into());
    q.exact.insert("tag".into(), json!("even"));
    q.limit = 1;
    for reopen in [false, true] {
        if reopen {
            drop(db);
            db = Database::open(&t.0).unwrap();
        }
        q.offset = 0;
        let first = db.search(&q).unwrap();
        assert_eq!(first.total, 2);
        assert_eq!(first.hits[0].document.key.id, "a");
        assert_eq!(first.hits[0].document.text, "yeni teknoloji");
        q.offset = 1;
        let second = db.search(&q).unwrap();
        assert_eq!(second.total, first.total);
        assert_eq!(second.hits[0].document.key.id, "d");
    }
}

#[test]
fn bounded_prefix_boundary_and_offset_overflow_preserve_exact_pages() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    db.commit(
        (0..5005)
            .map(|id| put(doc(&format!("{id:05}"), "teknoloji", 2)))
            .collect(),
    )
    .unwrap();
    db.flush().unwrap();
    let mut q = Query::new("shared");
    q.limit = 50;
    for offset in [0, 4046, 4047, 4096, 5000, 5005, usize::MAX] {
        q.offset = offset;
        let page = db.search(&q).unwrap();
        assert_eq!(page.total, 5005);
        #[cfg(feature = "experimental-grouped-read")]
        assert_eq!(
            serde_json::to_value(db.search_grouped_slots(&q).unwrap()).unwrap(),
            serde_json::to_value(&page).unwrap()
        );
        #[cfg(feature = "experimental-direct-slot")]
        assert_eq!(
            serde_json::to_value(db.search_direct_slots(&q).unwrap()).unwrap(),
            serde_json::to_value(&page).unwrap()
        );
        let expected = (offset.min(5005)..offset.saturating_add(50).min(5005))
            .map(|id| format!("{id:05}"))
            .collect::<Vec<_>>();
        assert_eq!(
            page.hits
                .into_iter()
                .map(|s| s.document.key.id)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[cfg(feature = "experimental-grouped-read")]
#[test]
fn grouped_hydration_preserves_large_pages_delta_versions_and_oversized_documents() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    for round in 0..3 {
        let changes = (0..1103)
            .filter(|n| n % 3 == round)
            .map(|n| {
                let text = if n == 17 {
                    "teknoloji ".repeat(8000)
                } else {
                    "teknoloji".into()
                };
                put(doc(&format!("{n:05}"), &text, n % 5))
            })
            .collect();
        db.commit(changes).unwrap();
        db.flush().unwrap();
    }
    db.commit(vec![
        put(doc("00005", "delta teknoloji", 2)),
        put(doc("00007", "delta teknoloji", 3)),
        Mutation::Delete {
            key: Key::new("shared", "reports", "00009"),
            guard: Guard::Any,
        },
    ])
    .unwrap();
    let mut q = Query::new("shared");
    q.collection = Some("reports".into());
    q.limit = 1000;
    for reopen in [false, true] {
        if reopen {
            drop(db);
            db = Database::open(&temp.0).unwrap();
        }
        for offset in [0, 7, 1099, 4096, usize::MAX] {
            q.offset = offset;
            let expected = db.search(&q).unwrap();
            let grouped = db.search_grouped_slots(&q).unwrap();
            assert_eq!(
                serde_json::to_value(&grouped).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
            assert_eq!(grouped.total, 1102);
            for hit in &grouped.hits {
                assert_eq!(db.get(&hit.document.key).unwrap().as_ref(), Some(hit));
            }
        }
        q.offset = 0;
        q.ranges = vec![Range {
            field: "day".into(),
            min: 2,
            max: 3,
        }];
        assert_eq!(
            serde_json::to_value(db.search_grouped_slots(&q).unwrap()).unwrap(),
            serde_json::to_value(db.search(&q).unwrap()).unwrap()
        );
        q.ranges.clear();
    }
    // No disk selection: empty and delta-only pages also keep identical results.
    let mut d = doc("private", "yeni içerik", 1);
    d.key.scope = "user:1".into();
    d.links.clear();
    db.commit(vec![put(d)]).unwrap();
    for scope in ["user:1", "user:empty"] {
        let q = Query::new(scope);
        assert_eq!(
            serde_json::to_value(db.search_grouped_slots(&q).unwrap()).unwrap(),
            serde_json::to_value(db.search(&q).unwrap()).unwrap()
        );
    }
}

#[test]
fn flush_writes_only_delta_and_does_not_rewrite_existing_sources() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    db.commit(vec![put(doc("a", "teknoloji", 1))]).unwrap();
    db.flush().unwrap();
    let path = t.source(0, "fdb");
    let original = fs::read(&path).unwrap();
    db.commit(vec![put(doc("b", "altın", 2))]).unwrap();
    db.flush().unwrap();
    assert_eq!(db.stats().unwrap().segments, 2);
    assert_eq!(fs::read(path).unwrap(), original);
    let second = fs::read(t.source(1, "fdb")).unwrap();
    assert!(!String::from_utf8_lossy(&second).contains("teknoloji"));
    assert_eq!(db.stats().unwrap().documents, 2);
    drop(db);
    let db = Database::open(&t.0).unwrap();
    assert_eq!(db.search(&Query::new("shared")).unwrap().total, 2);
}

#[test]
fn tombstone_survives_merge_and_does_not_resurrect_old_document() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    db.commit(vec![
        put(doc("a", "eski teknoloji", 1)),
        put(doc("b", "kalıcı", 2)),
    ])
    .unwrap();
    db.flush().unwrap();
    db.commit(vec![Mutation::Delete {
        key: doc("a", "", 0).key,
        guard: Guard::Version { version: 1 },
    }])
    .unwrap();
    db.flush().unwrap();
    assert!(db.get(&doc("a", "", 0).key).unwrap().is_none());
    db.compact().unwrap();
    db.wait_for_compaction().unwrap();
    assert_eq!(db.stats().unwrap().segments, 1);
    drop(db);
    let mut db = Database::open(&t.0).unwrap();
    assert!(db.get(&doc("a", "", 0).key).unwrap().is_none());
    assert_eq!(db.stats().unwrap().documents, 1);
    db.commit(vec![Mutation::Put {
        document: doc("a", "yeni", 3),
        guard: Guard::Missing,
    }])
    .unwrap();
    assert_eq!(db.stats().unwrap().documents, 2);
}

#[test]
fn writes_during_background_merge_and_unflushed_wal_survive_publication() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    for n in 0..4 {
        db.commit(vec![put(doc(&n.to_string(), "teknoloji", n))])
            .unwrap();
        db.flush().unwrap();
    }
    assert!(db.stats().unwrap().compaction_running);
    let version = db.get(&doc("0", "", 0).key).unwrap().unwrap().version;
    db.commit(vec![
        Mutation::Put {
            document: doc("0", "latest gold", 100),
            guard: Guard::Version { version },
        },
        Mutation::Delete {
            key: doc("1", "", 0).key,
            guard: Guard::Any,
        },
        put(doc("new", "newest", 101)),
    ])
    .unwrap();
    db.wait_for_compaction().unwrap();
    assert_eq!(
        db.get(&doc("0", "", 0).key).unwrap().unwrap().document.text,
        "latest gold"
    );
    assert!(db.get(&doc("1", "", 0).key).unwrap().is_none());
    let seq = db.stats().unwrap().sequence;
    drop(db);
    let db = Database::open(&t.0).unwrap();
    assert_eq!(db.stats().unwrap().sequence, seq);
    assert_eq!(db.stats().unwrap().documents, 4);
    let mut q = Query::new("shared");
    q.text = "latest".into();
    assert_eq!(db.search(&q).unwrap().total, 1);
}

#[test]
fn older_referenced_source_corruption_is_rejected_even_with_valid_cache() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    for n in 0..2 {
        db.commit(vec![put(doc(&n.to_string(), "content", n))])
            .unwrap();
        db.flush().unwrap();
    }
    drop(db);
    let source = t.source(0, "fdb");
    let mut bytes = fs::read(&source).unwrap();
    bytes[40] ^= 1;
    fs::write(source, bytes).unwrap();
    assert!(matches!(Database::open(&t.0), Err(Error::Corrupt(_))));
}

#[test]
fn every_manifest_section_is_checksummed_and_no_fallback_occurs() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    db.commit(vec![put(doc("a", "value", 0))]).unwrap();
    db.flush().unwrap();
    drop(db);
    let path = t.ready();
    let original = fs::read(&path).unwrap();
    for position in [0, 8, 12, original.len() / 2, original.len() - 1] {
        let mut bytes = original.clone();
        bytes[position] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(Database::open(&t.0), Err(Error::Corrupt(_))));
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn whitespace_and_short_terms_preserve_substring_and_semantics() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    let mut a = doc("a", "teknoloji\u{2003}yatırımları", 0);
    a.title = "gold".into();
    let mut b = doc("b", "gold", 1);
    b.title = "technology".into();
    db.commit(vec![put(a), put(b)]).unwrap();
    for disk in [false, true] {
        if disk {
            db.flush().unwrap();
        }
        for (text, total) in [
            ("teknoloji yatırımları", 1),
            ("gold\ttechnology", 1),
            ("yatırımları gold", 1),
            ("te yat", 1),
            ("goldtech", 0),
        ] {
            let mut q = Query::new("shared");
            q.text = text.into();
            assert_eq!(db.search(&q).unwrap().total, total, "{disk}: {text}");
        }
    }
}

#[test]
fn repeated_content_cache_keeps_titles_and_large_uncached_text_independent() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    let mut a = doc("東京🦀", "shared technology", 0);
    a.title = "gold".into();
    let mut b = doc("éclair", "shared technology", 1);
    b.title = "silver".into();
    db.commit(vec![
        put(a),
        put(b),
        put(doc("large", &"unique ".repeat(50_000), 2)),
    ])
    .unwrap();
    db.flush().unwrap();
    for _ in 0..2 {
        let mut q = Query::new("shared");
        q.text = "gold technology".into();
        let page = db.search(&q).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.hits[0].document.key.id, "東京🦀");
        q.text = "unique".into();
        assert_eq!(db.search(&q).unwrap().total, 1);
        q.text = "belgetech".into();
        assert_eq!(db.search(&q).unwrap().total, 0);
        drop(db);
        db = Database::open(&t.0).unwrap();
    }
}

#[test]
fn manifest_copied_under_another_generation_is_rejected() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    db.commit(vec![put(doc("a", "value", 0))]).unwrap();
    db.flush().unwrap();
    drop(db);
    fs::copy(t.ready(), t.0.join("generation-000000000000ffff.ready")).unwrap();
    assert!(matches!(Database::open(&t.0), Err(Error::Corrupt(_))));
}

#[test]
fn random_multi_segment_operations_match_reference_with_fields_and_links() {
    let t = Temp::new();
    let mut db = Database::open_with_options(
        &t.0,
        Options {
            max_delta_keys: 3,
            max_delta_bytes: 4096,
        },
    )
    .unwrap();
    let mut model = BTreeMap::new();
    let mut seed = 987654u64;
    for step in 0..250 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let key = format!("{:02}", (seed >> 32) % 30);
        if seed.is_multiple_of(5) {
            db.commit(vec![Mutation::Delete {
                key: doc(&key, "", 0).key,
                guard: Guard::Any,
            }])
            .unwrap();
            model.remove(&key);
        } else {
            let d = doc(
                &key,
                if seed.is_multiple_of(2) {
                    "technology"
                } else {
                    "gold"
                },
                (step % 9) - 4,
            );
            db.commit(vec![put(d.clone())]).unwrap();
            model.insert(key, d);
        }
        if step % 13 == 0 {
            db.compact().unwrap();
            db.wait_for_compaction().unwrap();
        }
        if step % 31 == 0 {
            drop(db);
            db = Database::open_with_options(
                &t.0,
                Options {
                    max_delta_keys: 3,
                    max_delta_bytes: 4096,
                },
            )
            .unwrap();
        }
        for text in ["", "technology", "gold"] {
            let mut q = Query::new("shared");
            q.text = text.into();
            q.ranges.push(Range {
                field: "day".into(),
                min: -2,
                max: 3,
            });
            q.linked_to = Some(Key::new("shared", "funds", "ABC"));
            let actual: Vec<_> = db
                .search(&q)
                .unwrap()
                .hits
                .into_iter()
                .map(|s| s.document.key.id)
                .collect();
            let expected: Vec<_> = model
                .iter()
                .filter(|(_, d)| {
                    d.text.contains(text) && (-2..=3).contains(&d.fields["day"].as_i64().unwrap())
                })
                .map(|(key, _)| key.clone())
                .collect();
            assert_eq!(actual, expected, "step {step}");
        }
        assert_eq!(db.stats().unwrap().documents, model.len());
    }
}

#[test]
fn process_kill_after_scheduling_merge_recovers_acknowledged_sources() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    let t = Temp::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fdb"))
        .arg(&t.0)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut request = |value: Value| {
        let input = child.stdin.as_mut().unwrap();
        writeln!(input, "{value}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["ok"], true);
        response
    };
    for n in 0..2 {
        request(
            json!({"op":"commit", "mutations":[put(doc(&n.to_string(), &"x".repeat(100_000), n))]}),
        );
        request(json!({"op":"flush"}));
    }
    let before = t.ready();
    let response = request(json!({"op":"compact"}));
    assert_eq!(response["result"]["compaction_running"], true);
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(t.ready(), before);
    #[cfg(all(windows, feature = "bounded-engine"))]
    for position in 0..2 {
        // A killed process releases the source lease without cleanup code.
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(t.source(position, "fdb"))
                .is_ok()
        );
    }
    let db = Database::open(&t.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 2);
    assert_eq!(db.stats().unwrap().sequence, 2);
    for n in 0..2 {
        assert_eq!(
            db.get(&doc(&n.to_string(), "", 0).key)
                .unwrap()
                .unwrap()
                .document
                .text
                .len(),
            100_000
        );
    }
}
