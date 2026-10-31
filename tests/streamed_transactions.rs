#![cfg(feature = "bounded-engine")]
use fdb::{Database, Document, Error, Guard, Key, Mutation, Query, Range};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "fdb-streamed-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn doc(id: usize) -> Document {
    Document {
        key: Key::new("şirket", "files", &format!("{id:05}")),
        title: "ŞİRKET 🦀".into(),
        text: format!("ortak teknoloji belge {id}"),
        fields: BTreeMap::from([("number".into(), json!(id))]),
        links: vec![Key::new("şirket", "links", "target")],
    }
}

#[test]
fn covered_grams_preserve_unicode_mixed_terms_visibility_and_numbered_pages() {
    let temp = Temp::new();
    let texts = [
        ("THE", "man 東京🦀 ababa"),
        ("İST", "man İstanbul ab"),
        ("none", "the aba xxx bab"),
        ("the", "woman 東京🦀"),
        ("th", "e man"),
        ("fields", "empty"),
    ];
    let mut expected: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(id, (title, text))| {
            let mut value = doc(id);
            value.title = (*title).into();
            value.text = (*text).into();
            value
                .fields
                .insert("unused_text".into(), json!("the 東京🦀"));
            fdb::StoredDocument {
                document: value,
                version: 1,
            }
        })
        .collect();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit_streamed(expected.iter().rev().map(|s| {
        Ok(Mutation::Put {
            document: s.document.clone(),
            guard: Guard::Missing,
        })
    }))
    .unwrap();
    fn check(db: &Database, rows: &[fdb::StoredDocument]) {
        fn fold(text: &str) -> String {
            text.chars()
                .flat_map(|c| match c {
                    'I' | 'İ' | 'ı' => 'i'.to_lowercase(),
                    _ => c.to_lowercase(),
                })
                .collect()
        }
        for text in [
            "the",
            "MAN THE",
            "東京🦀",
            "İST",
            "the istanbul",
            "ab",
            "abab",
            "abc",
        ] {
            let terms: Vec<_> = text.split_whitespace().map(fold).collect();
            let hits: Vec<_> = rows
                .iter()
                .filter(|s| {
                    let body = fold(&format!("{}\n{}", s.document.title, s.document.text));
                    terms.iter().all(|t| body.contains(t))
                })
                .cloned()
                .collect();
            for offset in [0, 1, 3, 5000] {
                let mut query = Query::new("şirket");
                query.text = text.into();
                query.limit = 2;
                query.offset = offset;
                let page = db.search(&query).unwrap();
                assert_eq!(page.total, hits.len(), "{text} offset{offset}");
                assert_eq!(
                    page.hits,
                    hits.iter()
                        .skip(offset)
                        .take(2)
                        .cloned()
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    check(&db, &expected);
    db.commit_streamed([Ok(Mutation::Delete {
        key: expected[0].document.key.clone(),
        guard: Guard::Version { version: 1 },
    })])
    .unwrap();
    expected.remove(0);
    expected[0].document.text = "the man 東京🦀".into();
    db.commit_streamed([Ok(Mutation::Put {
        document: expected[0].document.clone(),
        guard: Guard::Version { version: 1 },
    })])
    .unwrap();
    expected[0].version = 3;
    check(&db, &expected);
    drop(db);
    check(&Database::open_deferred(&temp.0).unwrap(), &expected);
}

#[test]
fn short_coverage_and_isolated_short_fallback_match_independent_oracle() {
    let temp = Temp::new();
    let mut rows: Vec<_> = (0..4)
        .map(|id| {
            let mut document = doc(id);
            document.title = if id == 0 { "THE 東京🦀" } else { "prefix" }.into();
            document.text = if id == 0 {
                "rare quixote"
            } else {
                "the 東京🦀 xyz"
            }
            .into();
            if id == 3 {
                document.key.collection = "other".into();
            }
            document.fields.insert("excluded".into(), json!("zx"));
            fdb::StoredDocument {
                document,
                version: 1,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.document.key.cmp(&b.document.key));
    let mut db = Database::open(&temp.0).unwrap();
    db.commit_streamed(rows.iter().rev().map(|row| {
        Ok(Mutation::Put {
            document: row.document.clone(),
            guard: Guard::Missing,
        })
    }))
    .unwrap();
    fn check(db: &Database, rows: &[fdb::StoredDocument]) {
        for text in [
            "HE",
            "th",
            "京🦀",
            "he xyz",
            "he quixote",
            "zx",
            "he zx",
            "h",
            "abab",
        ] {
            let terms: Vec<_> = text.split_whitespace().map(str::to_lowercase).collect();
            for scope in ["şirket", "missing"] {
                for collection in [None, Some("files"), Some("other")] {
                    let expected: Vec<_> = rows
                        .iter()
                        .filter(|row| {
                            let d = &row.document;
                            d.key.scope == scope
                                && collection.is_none_or(|c| d.key.collection == c)
                                && terms.iter().all(|t| {
                                    d.title.to_lowercase().contains(t)
                                        || d.text.to_lowercase().contains(t)
                                })
                        })
                        .cloned()
                        .collect();
                    for offset in [0, 1, 5000] {
                        let mut q = Query::new(scope);
                        q.text = text.into();
                        q.collection = collection.map(str::to_owned);
                        q.offset = offset;
                        q.limit = 2;
                        let page = db.search(&q).unwrap();
                        assert_eq!(page.total, expected.len(), "{text}/{scope}/{collection:?}");
                        assert_eq!(
                            page.hits,
                            expected
                                .iter()
                                .skip(offset)
                                .take(2)
                                .cloned()
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    }
    check(&db, &rows);
    // Isolated 'he' still matches, although no containing trigram proves it.
    rows[1].document.text = "he 東京🦀 isolated".into();
    db.commit_streamed([Ok(Mutation::Put {
        document: rows[1].document.clone(),
        guard: Guard::Version { version: 1 },
    })])
    .unwrap();
    rows[1].version = 2;
    check(&db, &rows);
    db.commit_streamed([Ok(Mutation::Delete {
        key: rows[0].document.key.clone(),
        guard: Guard::Version { version: 1 },
    })])
    .unwrap();
    rows.remove(0);
    check(&db, &rows);
    drop(db);
    check(&Database::open_record_reads(&temp.0).unwrap(), &rows);
}

#[test]
fn covered_query_never_accepts_corrupt_source_with_valid_index_and_address_caches() {
    use std::io::{Read, Seek, SeekFrom, Write};
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    let mut value = doc(0);
    value.text = "the man".into();
    db.commit_streamed([Ok(Mutation::Put {
        document: value,
        guard: Guard::Missing,
    })])
    .unwrap();
    db.flush().unwrap();
    db.prepare_record_reads().unwrap();
    drop(db);
    let mut sources: Vec<_> = std::fs::read_dir(&temp.0)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "fdb")
                && p.file_name().unwrap().to_string_lossy().starts_with("run-")
        })
        .collect();
    // Explicit flush publishes one run; generation WALs are not that source.
    assert_eq!(sources.len(), 1);
    let source = sources.pop().unwrap();
    let indexes: Vec<_> = std::fs::read_dir(&temp.0)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "idx"))
        .map(|p| {
            let bytes = std::fs::read(&p).unwrap();
            (p, bytes)
        })
        .collect();
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(source)
        .unwrap();
    file.seek(SeekFrom::Start(32)).unwrap();
    let mut byte = [0];
    file.read_exact(&mut byte).unwrap();
    byte[0] ^= 1;
    file.seek(SeekFrom::Start(32)).unwrap();
    file.write_all(&byte).unwrap();
    file.sync_all().unwrap();
    drop(file);
    let mut query = Query::new("şirket");
    query.text = "the man".into();
    assert!(Database::open(&temp.0).is_err());
    // Each reader owns the DB lock in turn; a lock error is not corruption proof.
    for record_reads in [false, true] {
        let opened = if record_reads {
            Database::open_record_reads(&temp.0)
        } else {
            Database::open_deferred(&temp.0)
        };
        match opened {
            Ok(db) => {
                for text in ["the man", "he"] {
                    query.text = text.into();
                    assert!(matches!(db.search(&query), Err(Error::Corrupt(_))));
                    assert!(matches!(db.search_first(&query), Err(Error::Corrupt(_))));
                    assert!(matches!(
                        db.search_with_execution(
                            &query,
                            fdb::SearchExecution {
                                verification_workers: 4,
                                coalesce_gap_bytes: 8192,
                                reuse_validated_source_crc: true
                            }
                        ),
                        Err(Error::Corrupt(_))
                    ));
                }
            }
            Err(error) => assert!(matches!(error, Error::Corrupt(_))),
        }
    }
    for (path, bytes) in indexes {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn covered_queries_reject_source_or_index_changed_after_successful_warm_search() {
    use std::io::{Seek, SeekFrom, Write};
    for extension in ["fdb", "idx"] {
        let temp = Temp::new();
        let mut db = Database::open(&temp.0).unwrap();
        let mut value = doc(0);
        value.text = "the man".into();
        db.commit_streamed([Ok(Mutation::Put {
            document: value,
            guard: Guard::Missing,
        })])
        .unwrap();
        db.flush().unwrap();
        let mut query = Query::new("şirket");
        query.text = "the".into();
        assert_eq!(db.search(&query).unwrap().total, 1);
        let source = std::fs::read_dir(&temp.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|p| {
                p.extension().is_some_and(|e| e == extension)
                    && p.file_name().unwrap().to_string_lossy().starts_with("run-")
            })
            .unwrap();
        #[cfg(windows)]
        if extension == "fdb" {
            assert!(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&source)
                    .is_err()
            );
            assert_eq!(db.search(&query).unwrap().total, 1);
            drop(db);
            let mut bytes = std::fs::read(&source).unwrap();
            bytes[32] ^= 0x40;
            std::fs::write(&source, bytes).unwrap();
            // A valid index/address cache must not hide offline corruption.
            assert!(matches!(Database::open(&temp.0), Err(Error::Corrupt(_))));
            continue;
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(source)
            .unwrap();
        file.seek(SeekFrom::Start(32)).unwrap();
        file.write_all(&[0xff]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        for text in ["the", "xyz", "he"] {
            query.text = text.into();
            assert!(
                matches!(db.search(&query), Err(Error::Corrupt(_))),
                "{extension}/{text}"
            );
        }
    }
}
#[cfg(windows)]
#[test]
fn immutable_source_lease_blocks_mutation_and_keeps_wal_writable() {
    let temp = Temp::new();
    let parent = temp.0.join("nested");
    let dir = parent.join("db");
    let mut db = Database::open(&dir).unwrap();
    db.commit_streamed([Ok(Mutation::Put {
        document: doc(0),
        guard: Guard::Missing,
    })])
    .unwrap();
    db.flush().unwrap();
    let source = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|p| {
            p.extension().is_some_and(|e| e == "fdb")
                && p.file_name().unwrap().to_string_lossy().starts_with("run-")
        })
        .unwrap();
    let original = std::fs::read(&source).unwrap();
    assert!(std::fs::File::open(&source).is_ok());
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(&source)
            .is_err()
    );
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&source)
            .is_err()
    );
    assert!(std::fs::remove_file(&source).is_err());
    assert!(std::fs::rename(&source, source.with_extension("moved")).is_err());
    assert!(std::fs::rename(&dir, parent.join("moved")).is_err());
    assert!(std::fs::rename(&parent, temp.0.join("moved")).is_err());
    // Active WAL remains writable while immutable runs are protected.
    db.commit(vec![Mutation::Put {
        document: doc(1),
        guard: Guard::Missing,
    }])
    .unwrap();
    assert_eq!(db.get(&doc(1).key).unwrap().unwrap().version, 2);
    assert_eq!(std::fs::read(&source).unwrap(), original);
    drop(db);
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap();
    // An existing writer cannot be grandfathered into a trusted source lease.
    assert!(matches!(Database::open(&dir), Err(Error::Io(_))));
    drop(writer);
    assert_eq!(Database::open(&dir).unwrap().stats().unwrap().documents, 2);
}
#[test]
fn atomic_streamed_input_filters_delete_update_reopen_and_normal_interop() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    assert_eq!(
        db.commit_streamed((0..180).rev().map(|id| Ok(Mutation::Put {
            document: doc(id),
            guard: Guard::Missing
        })))
        .unwrap(),
        1
    );
    assert_eq!(db.stats().unwrap().delta_keys, 0);
    assert_eq!(db.stats().unwrap().resident_documents, 0);
    for id in [0, 127, 179] {
        let s = db.get(&doc(id).key).unwrap().unwrap();
        assert_eq!(s.document, doc(id));
        assert_eq!(s.version, 1);
    }
    let mut q = Query::new("şirket");
    q.text = "sirket teknoloji".into();
    assert_eq!(db.search(&q).unwrap().total, 180);
    q.exact.insert("number".into(), json!(127));
    assert_eq!(db.search(&q).unwrap().total, 1);
    q.exact.clear();
    q.ranges.push(Range {
        field: "number".into(),
        min: 126,
        max: 129,
    });
    assert_eq!(db.search(&q).unwrap().total, 4);
    q.ranges.clear();
    q.linked_to = Some(Key::new("şirket", "links", "target"));
    assert_eq!(db.search(&q).unwrap().total, 180);
    let wal_before = db.stats().unwrap().log_bytes;
    let duplicate = db.commit_streamed([
        Ok(Mutation::Put {
            document: doc(999),
            guard: Guard::Missing,
        }),
        Ok(Mutation::Put {
            document: doc(999),
            guard: Guard::Missing,
        }),
    ]);
    assert!(matches!(duplicate, Err(Error::Invalid(_))));
    let rejected = db.commit_streamed([
        Ok(Mutation::Put {
            document: doc(999),
            guard: Guard::Missing,
        }),
        Err(Error::Invalid("input failed".into())),
    ]);
    assert!(rejected.is_err());
    let conflict = db.commit_streamed([Ok(Mutation::Delete {
        key: doc(0).key,
        guard: Guard::Version { version: 77 },
    })]);
    assert!(matches!(conflict, Err(Error::Conflict(_))));
    assert_eq!(db.stats().unwrap().log_bytes, wal_before);
    assert!(db.get(&doc(999).key).unwrap().is_none());
    db.commit(vec![Mutation::Put {
        document: doc(180),
        guard: Guard::Missing,
    }])
    .unwrap();
    let mut changed = doc(1);
    changed.text = "değişmiş İstanbul".into();
    assert_eq!(
        db.commit_streamed([
            Ok(Mutation::Delete {
                key: doc(0).key,
                guard: Guard::Version { version: 1 }
            }),
            Ok(Mutation::Put {
                document: changed.clone(),
                guard: Guard::Version { version: 1 }
            })
        ])
        .unwrap(),
        3
    );
    db.wait_for_compaction().unwrap();
    db.check_integrity().unwrap();
    drop(db);
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 180);
    assert!(db.get(&doc(0).key).unwrap().is_none());
    assert_eq!(db.get(&doc(1).key).unwrap().unwrap().document, changed);
    assert_eq!(db.get(&doc(1).key).unwrap().unwrap().version, 3);
    assert_eq!(db.get(&doc(180).key).unwrap().unwrap().version, 2);
    assert!(!std::fs::read_dir(&temp.0).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("input-")
    }));
}

#[test]
#[ignore = "subprocess helper for streamed_process_kill"]
fn streamed_child() {
    use std::io::Write;
    let Some(path) = std::env::var_os("FDB_STREAMED_CHILD_DIR") else {
        return;
    };
    let mode = std::env::var("FDB_STREAMED_CHILD_MODE").unwrap();
    let mut db = Database::open(path).unwrap();
    db.commit_streamed((0..500).map(|id| {
        if id == 499 {
            println!("INTAKE_COMPLETE");
            std::io::stdout().flush().unwrap();
            if mode == "intake" {
                loop {
                    std::thread::park();
                }
            }
        }
        let mut d = doc(id);
        d.text = "şirket teknoloji örnek ".repeat(500);
        Ok(Mutation::Put {
            document: d,
            guard: Guard::Missing,
        })
    }))
    .unwrap();
    println!("STREAMED_ACK");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn streamed_process_kill_before_ack_during_wal_publication_and_after_ack() {
    use std::{
        io::{BufRead, BufReader, Read, Seek, SeekFrom},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    for mode in ["intake", "wal", "acked"] {
        let temp = Temp::new();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "streamed_child", "--nocapture"])
            .env("FDB_STREAMED_CHILD_DIR", &temp.0)
            .env("FDB_STREAMED_CHILD_MODE", mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = send.send(line);
            }
        });
        let wanted = if mode == "acked" {
            "STREAMED_ACK"
        } else {
            "INTAKE_COMPLETE"
        };
        loop {
            match receive.recv_timeout(Duration::from_secs(20)) {
                Ok(line) if line.contains(wanted) => break,
                Ok(_) => {}
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("child marker: {e}");
                }
            }
        }
        if mode == "wal" {
            let start = Instant::now();
            let path = temp.0.join("generation-0000000000000001.fdb");
            loop {
                let mut file = std::fs::File::open(&path).unwrap();
                let len = file.metadata().unwrap().len();
                let mut footer = [0; 8];
                if len > 45 {
                    file.seek(SeekFrom::End(-8)).unwrap();
                    file.read_exact(&mut footer).unwrap();
                    if &footer == b"FDBDONE1" {
                        break;
                    }
                }
                assert!(start.elapsed() < Duration::from_secs(20));
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        child.kill().unwrap();
        child.wait().unwrap();
        reader.join().unwrap();
        let db = Database::open(&temp.0).unwrap();
        assert_eq!(
            db.stats().unwrap().documents,
            if mode == "intake" { 0 } else { 500 }
        );
        for id in [0, 249, 499] {
            let result = db.get(&doc(id).key).unwrap();
            if mode == "intake" {
                assert!(result.is_none());
            } else {
                let stored = result.unwrap();
                assert_eq!(stored.version, 1);
                assert_eq!(stored.document.text, "şirket teknoloji örnek ".repeat(500));
            }
        }
        let mut q = Query::new("şirket");
        q.text = "teknoloji örnek".into();
        let exact = db.search(&q).unwrap();
        assert_eq!(db.search_first_hybrid(&q, 200).unwrap().hits, exact.hits);
        assert_eq!(
            db.search_first_hybrid_cancellable(&q, 200, &fdb::CancelToken::new())
                .unwrap()
                .hits,
            exact.hits
        );
        assert_eq!(exact.total, if mode == "intake" { 0 } else { 500 });
        for e in [
            fdb::SearchExecution::default(),
            fdb::SearchExecution {
                verification_workers: 4,
                coalesce_gap_bytes: 8192,
                reuse_validated_source_crc: true,
            },
        ] {
            let capped = db.search_first_with_execution(&q, e).unwrap();
            assert_eq!(capped.hits, exact.hits);
            assert_eq!(capped.sequence, exact.sequence);
            let checked = db.search_with_execution(&q, e).unwrap();
            assert_eq!(checked.hits, exact.hits);
            assert_eq!(checked.total, exact.total);
        }
        db.check_integrity().unwrap();
    }
}

#[test]
#[ignore = "subprocess helper for mixed_writer_workspace_admission_does_not_deadlock"]
fn concurrent_streamed_child() {
    use std::io::Write;
    let Some(root) = std::env::var_os("FDB_STREAMED_CONCURRENT_DIR") else {
        return;
    };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let handles: Vec<_> = (0..3)
        .map(|worker| {
            let path = PathBuf::from(&root).join(worker.to_string());
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut db = Database::open(path).unwrap();
                barrier.wait();
                for _ in 0..3 {
                    let changes = (0..100).map(|id| Mutation::Put {
                        document: doc(id),
                        guard: Guard::Any,
                    });
                    if worker == 0 {
                        db.commit(changes.collect()).unwrap();
                        db.flush().unwrap();
                    } else {
                        db.commit_streamed(changes.map(Ok)).unwrap();
                    }
                }
                db.wait_for_compaction().unwrap();
                db.check_integrity().unwrap();
                assert_eq!(db.stats().unwrap().documents, 100);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    println!("CONCURRENT_DONE");
    std::io::stdout().flush().unwrap();
}

#[test]
fn mixed_writer_workspace_admission_does_not_deadlock() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        time::Duration,
    };
    let temp = Temp::new();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "concurrent_streamed_child",
            "--nocapture",
        ])
        .env("FDB_STREAMED_CONCURRENT_DIR", &temp.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let out = child.stdout.take().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = send.send(line);
        }
    });
    loop {
        match receive.recv_timeout(Duration::from_secs(25)) {
            Ok(line) if line.contains("CONCURRENT_DONE") => break,
            Ok(_) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("workspace admission hung: {e}");
            }
        }
    }
    assert!(child.wait().unwrap().success());
    reader.join().unwrap();
    for worker in 0..3 {
        let db = Database::open(temp.0.join(worker.to_string())).unwrap();
        assert_eq!(db.stats().unwrap().documents, 100);
        assert_eq!(db.get(&doc(99).key).unwrap().unwrap().version, 3);
    }
}
