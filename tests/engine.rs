use fdb::{Database, Document, Error, Guard, Key, Mutation, Query, Range};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "experimental-wal-sync-control")]
#[test]
#[ignore = "subprocess helper; invoked by buffered_wal_process_kill"]
fn buffered_wal_child() {
    let Some(path) = std::env::var_os("FDB_BUFFERED_TEST_DIR") else {
        return;
    };
    let mut db = Database::open(path).unwrap();
    db.set_wal_sync(false).unwrap();
    db.commit(vec![put(doc("kill", "buffered process kill"))])
        .unwrap();
    println!("FDB_BUFFERED_READY");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

#[cfg(feature = "experimental-wal-sync-control")]
#[test]
fn buffered_wal_process_kill() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    let temp = Temp::new();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "buffered_wal_child", "--ignored", "--nocapture"])
        .env("FDB_BUFFERED_TEST_DIR", &temp.0)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        if output.read_line(&mut line).unwrap() == 0 {
            let _ = child.wait();
            panic!("buffered child exited before acknowledgement");
        }
        if line.trim() == "FDB_BUFFERED_READY" {
            break;
        }
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let db = Database::open(&temp.0).unwrap();
    assert_eq!(
        db.get(&doc("kill", "").key).unwrap().unwrap().document.text,
        "buffered process kill"
    );
    // Killing a process leaves the OS cache alive; this is not power-loss safety.
}

#[cfg(feature = "experimental-wal-sync-control")]
#[test]
fn buffered_wal_keeps_atomic_frames_and_rejects_corruption() {
    let temp = Temp::new();
    let mut db = Database::open(&temp.0).unwrap();
    db.commit(vec![put(doc("old", "synced"))]).unwrap();
    let previous_end = db.stats().unwrap().log_bytes as usize;
    db.set_wal_sync(false).unwrap();
    db.commit(vec![put(doc("a", "buffered")), put(doc("b", "buffered"))])
        .unwrap();
    drop(db);
    let log = temp.log();
    let complete = fs::read(&log).unwrap();
    for cut in previous_end..complete.len() {
        fs::write(&log, &complete[..cut]).unwrap();
        let db = Database::open(&temp.0).unwrap();
        assert_eq!(db.stats().unwrap().documents, 1, "cut={cut}");
        assert!(db.get(&doc("old", "").key).unwrap().is_some());
        assert!(db.get(&doc("a", "").key).unwrap().is_none());
        assert!(db.get(&doc("b", "").key).unwrap().is_none());
    }
    fs::write(&log, &complete).unwrap();
    let mut db = Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 3);
    db.set_wal_sync(false).unwrap();
    db.commit(vec![put(doc("c", "pending"))]).unwrap();
    db.set_wal_sync(true).unwrap();
    db.commit(vec![put(doc("d", "synced again"))]).unwrap();
    drop(db);
    assert_eq!(
        Database::open(&temp.0).unwrap().stats().unwrap().documents,
        5
    );
    let mut damaged = complete;
    damaged[previous_end + 24] ^= 1;
    fs::write(log, damaged).unwrap();
    assert!(matches!(Database::open(&temp.0), Err(Error::Corrupt(_))));
}

#[cfg(feature = "experimental-wal-sync-control")]
#[test]
fn synced_and_buffered_commits_produce_identical_wal_bytes() {
    let left = Temp::new();
    let right = Temp::new();
    for (temp, sync) in [(&left, true), (&right, false)] {
        let mut db = Database::open(&temp.0).unwrap();
        db.set_wal_sync(sync).unwrap();
        db.commit(vec![put(doc("a", "Şirket 東京 \\\"\n"))])
            .unwrap();
        db.commit(vec![put(doc("b", "second"))]).unwrap();
        drop(db);
    }
    assert_eq!(
        fs::read(left.log()).unwrap(),
        fs::read(right.log()).unwrap()
    );
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "fdb-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn log(&self) -> PathBuf {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|s| s == "fdb"))
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
        title: "Fon raporu".into(),
        text: text.into(),
        fields: BTreeMap::new(),
        links: vec![],
    }
}
fn put(document: Document) -> Mutation {
    Mutation::Put {
        document,
        guard: Guard::Any,
    }
}

#[cfg(feature = "bounded-engine")]
#[test]
fn checked_page_preserves_global_order_visibility_filters_and_fallback_pages() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let target = Key::new("shared", "targets", "one");
    // Small unit fixture; no benchmark corpus or persistent work slot changes.
    for batch in 0..3 {
        let mut mutations = Vec::new();
        for n in 0..8 {
            let id = n * 3 + batch;
            let mut d = doc(&format!("{id:03}"), "İSTANBUL needle 東京 🦀");
            d.key.collection = if id % 2 == 0 { "a" } else { "b" }.into();
            d.fields.insert("kind".into(), json!(id % 3));
            d.fields.insert("rank".into(), json!(id));
            d.fields
                .insert("nested".into(), json!([null, {"ok": true}]));
            d.links.push(target.clone());
            mutations.push(put(d));
        }
        db.commit(mutations).unwrap();
        db.flush().unwrap();
    }
    let mut changed = db
        .get(&Key::new("shared", "a", "000"))
        .unwrap()
        .unwrap()
        .document;
    changed.text = "new needle İstanbul".into();
    db.commit(vec![
        put(changed),
        Mutation::Delete {
            key: Key::new("shared", "b", "001"),
            guard: Guard::Any,
        },
        put(doc("delta", "needle İstanbul")),
    ])
    .unwrap();
    let mut reused_page = fdb::SearchPage {
        total: 0,
        sequence: 0,
        hits: Vec::new(),
    };
    for text in ["needle", "needle istanbul", "zzzz", "nee", ""] {
        for filtered in [false, true] {
            let mut q = Query::new("shared");
            q.text = text.into();
            if filtered {
                q.exact.insert("kind".into(), json!(2));
                q.ranges.push(Range {
                    field: "rank".into(),
                    min: 2,
                    max: 20,
                });
                q.linked_to = Some(target.clone());
            }
            for (offset, limit) in [(0, 1), (0, 5), (0, 50), (0, 51), (1, 5), (5000, 5)] {
                q.offset = offset;
                q.limit = limit;
                let actual = db.search(&q).unwrap();
                db.search_into(&q, &mut reused_page).unwrap();
                assert_eq!(reused_page.total, actual.total);
                assert_eq!(reused_page.sequence, actual.sequence);
                assert_eq!(reused_page.hits, actual.hits);
                let reference = db.search_direct_slots(&q).unwrap();
                assert_eq!(
                    actual.total, reference.total,
                    "{text}/{filtered}/{offset}/{limit}"
                );
                assert_eq!(actual.sequence, reference.sequence);
                assert_eq!(
                    actual.hits, reference.hits,
                    "{text}/{filtered}/{offset}/{limit}"
                );
                for stored in actual.hits {
                    assert_eq!(db.get(&stored.document.key).unwrap(), Some(stored));
                }
            }
        }
    }
    let mut invalid = Query::new("shared");
    db.search_into(&invalid, &mut reused_page).unwrap();
    assert!(!reused_page.hits.is_empty());
    invalid.limit = 0;
    assert!(db.search_into(&invalid, &mut reused_page).is_err());
    assert_eq!(
        (
            reused_page.total,
            reused_page.sequence,
            reused_page.hits.len(),
            reused_page.hits.capacity()
        ),
        (0, 0, 0, 0)
    );
}

#[test]
fn durable_batch_update_delete_compaction() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    assert_eq!(
        db.commit(vec![put(doc("1", "eski")), put(doc("2", "diğer"))])
            .unwrap(),
        1
    );
    db.commit(vec![
        put(doc("1", "yeni")),
        Mutation::Delete {
            key: doc("2", "").key,
            guard: Guard::Version { version: 1 },
        },
    ])
    .unwrap();
    let previous_size = db.stats().unwrap().log_bytes;
    db.compact().unwrap();
    assert!(db.stats().unwrap().log_bytes < previous_size);
    drop(db);
    let mut db = Database::open(&tmp.0).unwrap();
    assert_eq!(
        db.get(&doc("1", "").key).unwrap().unwrap().document.text,
        "yeni"
    );
    assert!(db.get(&doc("2", "").key).unwrap().is_none());
    assert_eq!(db.commit(vec![put(doc("3", "son"))]).unwrap(), 3);
    drop(db);
    assert_eq!(
        Database::open(&tmp.0).unwrap().stats().unwrap().documents,
        2
    );
}

#[test]
fn guard_failure_is_atomic_and_does_not_write() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    db.commit(vec![put(doc("1", "a"))]).unwrap();
    let before = db.stats().unwrap().log_bytes;
    let result = db.commit(vec![
        put(doc("2", "b")),
        Mutation::Delete {
            key: doc("1", "").key,
            guard: Guard::Version { version: 999 },
        },
    ]);
    assert!(matches!(result, Err(Error::Conflict(_))));
    assert!(db.get(&doc("2", "").key).unwrap().is_none());
    assert_eq!(db.stats().unwrap().log_bytes, before);
    assert!(
        db.commit(vec![Mutation::Put {
            document: doc("1", ""),
            guard: Guard::Missing
        }])
        .is_err()
    );
}

#[test]
fn combined_indexes_turkish_pagination_and_scope() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let target = Key::new("shared", "funds", "ABC");
    for id in 0..5 {
        let mut d = doc(&id.to_string(), "ŞİRKETİN teknoloji yatırımları");
        d.fields.insert("fund_code".into(), json!("ABC"));
        d.fields.insert("as_of".into(), json!(20261001 + id));
        d.links.push(target.clone());
        db.commit(vec![put(d)]).unwrap();
    }
    let mut private = doc("private", "şirketin teknoloji yatırımları");
    private.key.scope = "user:42".into();
    db.commit(vec![put(private)]).unwrap();
    let mut q = Query::new("shared");
    q.collection = Some("reports".into());
    q.text = "sirketin YATIRIM".into();
    q.exact.insert("fund_code".into(), json!("ABC"));
    q.ranges.push(Range {
        field: "as_of".into(),
        min: 20261002,
        max: 20261004,
    });
    q.linked_to = Some(target);
    q.limit = 2;
    let page = db.search(&q).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.hits.len(), 2);
    assert_eq!(page.hits[0].document.key.id, "1");
    q.offset = 2;
    assert_eq!(db.search(&q).unwrap().hits[0].document.key.id, "3");
    q.offset = 0;
    q.text = "不存在".into();
    assert_eq!(db.search(&q).unwrap().total, 0);
    assert_eq!(db.search(&Query::new("user:42")).unwrap().total, 1);
    db.compact().unwrap();
    drop(db);
    let db = Database::open(&tmp.0).unwrap();
    q.text = "sirketin YATIRIM".into();
    assert_eq!(db.search(&q).unwrap().total, 3);
    assert_eq!(db.search(&q).unwrap().hits[0].document.key.id, "1");
    assert_eq!(db.search(&Query::new("user:42")).unwrap().total, 1);
}

#[test]
fn stale_postings_removed_and_short_unicode_terms_work() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let mut d = doc("1", "東京 eski");
    d.fields.insert("day".into(), json!(1));
    d.links.push(Key::new("shared", "funds", "ABC"));
    db.commit(vec![put(d)]).unwrap();
    let mut q = Query::new("shared");
    q.text = "東京".into();
    assert_eq!(db.search(&q).unwrap().total, 1);
    db.commit(vec![put(doc("1", "yeni"))]).unwrap();
    assert_eq!(db.search(&q).unwrap().total, 0);
    q.text.clear();
    q.exact.insert("day".into(), json!(1));
    assert_eq!(db.search(&q).unwrap().total, 0);
    q.exact.clear();
    q.linked_to = Some(Key::new("shared", "funds", "ABC"));
    assert_eq!(db.search(&q).unwrap().total, 0);
    q.linked_to = None;
    q.ranges.push(Range {
        field: "day".into(),
        min: 0,
        max: 2,
    });
    assert_eq!(db.search(&q).unwrap().total, 0);
}

#[test]
fn exclusive_lock_released_on_drop() {
    let tmp = Temp::new();
    let db = Database::open(&tmp.0).unwrap();
    assert!(matches!(Database::open(&tmp.0), Err(Error::Locked)));
    drop(db);
    assert!(Database::open(&tmp.0).is_ok());
}

#[test]
fn every_partial_transaction_cut_recovers_previous_commit() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    db.commit(vec![put(doc("old", "kalıcı"))]).unwrap();
    let old_end = db.stats().unwrap().log_bytes as usize;
    db.commit(vec![put(doc("new1", "bir")), put(doc("new2", "iki"))])
        .unwrap();
    drop(db);
    let log = tmp.log();
    let bytes = fs::read(&log).unwrap();
    for cut in old_end..bytes.len() {
        fs::write(&log, &bytes[..cut]).unwrap();
        let db = Database::open(&tmp.0).unwrap();
        assert_eq!(db.stats().unwrap().documents, 1, "cut={cut}");
        assert!(db.get(&doc("old", "").key).unwrap().is_some());
        assert_eq!(db.stats().unwrap().log_bytes, old_end as u64);
    }
    fs::write(&log, &bytes).unwrap();
    assert_eq!(
        Database::open(&tmp.0).unwrap().stats().unwrap().documents,
        3
    );
}

#[test]
fn every_single_byte_corruption_in_committed_frame_is_rejected() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let start = db.stats().unwrap().log_bytes as usize;
    db.commit(vec![put(doc("1", "test"))]).unwrap();
    drop(db);
    let log = tmp.log();
    let bytes = fs::read(&log).unwrap();
    for at in start..bytes.len() {
        let mut corrupt = bytes.clone();
        corrupt[at] ^= 1;
        fs::write(&log, &corrupt).unwrap();
        assert!(
            matches!(Database::open(&tmp.0), Err(Error::Corrupt(_))),
            "byte {at}"
        );
        assert_eq!(
            fs::read(&log).unwrap(),
            corrupt,
            "corruption must not be silently truncated"
        );
    }
}

#[test]
fn corrupt_latest_checkpoint_never_falls_back_to_stale_generation() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    db.commit(vec![put(doc("1", "v1"))]).unwrap();
    db.compact().unwrap();
    db.commit(vec![put(doc("2", "v2"))]).unwrap();
    drop(db);
    let latest = tmp.log();
    let mut bytes = fs::read(&latest).unwrap();
    bytes[40] ^= 1;
    fs::write(&latest, bytes).unwrap();
    assert!(matches!(Database::open(&tmp.0), Err(Error::Corrupt(_))));
}

#[test]
fn incomplete_unpublished_generation_is_ignored() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    db.commit(vec![put(doc("1", "a"))]).unwrap();
    drop(db);
    fs::write(tmp.0.join("generation-0000000000000002.fdb"), b"incomplete").unwrap();
    fs::write(
        tmp.0.join("generation-0000000000000002.pending"),
        b"partial marker",
    )
    .unwrap();
    let mut db = Database::open(&tmp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 1);
    assert!(db.compact().unwrap().generation > 2);
}

#[test]
fn truncated_published_checkpoint_and_missing_marker_are_errors() {
    let tmp = Temp::new();
    let db = Database::open(&tmp.0).unwrap();
    drop(db);
    fs::write(tmp.log(), b"FDBLOG01").unwrap();
    assert!(Database::open(&tmp.0).is_err());
    fs::remove_file(tmp.0.join("generation-0000000000000001.ready")).unwrap();
    assert!(Database::open(&tmp.0).is_err());
}

#[test]
fn invalid_batch_and_cross_scope_reference_do_not_write() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let before = db.stats().unwrap().log_bytes;
    assert!(
        db.commit(vec![put(doc("1", "a")), put(doc("1", "b"))])
            .is_err()
    );
    let mut d = doc("1", "a");
    d.links.push(Key::new("user:private", "funds", "ABC"));
    assert!(db.commit(vec![put(d)]).is_err());
    assert_eq!(db.stats().unwrap().log_bytes, before);
    let mut q = Query::new("shared");
    q.ranges.push(Range {
        field: "x".into(),
        min: 2,
        max: 1,
    });
    assert!(db.search(&q).is_err());
}

#[test]
fn randomized_operations_match_reference_across_reopens() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let mut reference = BTreeMap::new();
    let mut seed = 12345u64;
    for step in 0..240 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = ((seed >> 32) % 32).to_string();
        if seed.is_multiple_of(4) {
            db.commit(vec![Mutation::Delete {
                key: doc(&id, "").key,
                guard: Guard::Any,
            }])
            .unwrap();
            reference.remove(&id);
        } else {
            let text = if seed.is_multiple_of(2) {
                "yeşil teknoloji"
            } else {
                "altın yatırım"
            };
            db.commit(vec![put(doc(&id, text))]).unwrap();
            reference.insert(id, text.to_string());
        }
        if step % 40 == 0 {
            db.compact().unwrap();
            drop(db);
            db = Database::open(&tmp.0).unwrap();
        }
        for term in ["teknoloji", "yatırım", "al", "y", "şil"] {
            let mut q = Query::new("shared");
            q.text = term.into();
            let actual: Vec<_> = db
                .search(&q)
                .unwrap()
                .hits
                .iter()
                .map(|h| h.document.key.id.clone())
                .collect();
            let expected: Vec<_> = reference
                .iter()
                .filter(|(_, text)| text.contains(term))
                .map(|(id, _)| id.clone())
                .collect();
            assert_eq!(actual, expected, "step={step}, term={term}");
        }
    }
}

#[test]
fn complete_garbage_header_is_not_a_torn_write() {
    let tmp = Temp::new();
    let db = Database::open(&tmp.0).unwrap();
    drop(db);
    OpenOptions::new()
        .append(true)
        .open(tmp.log())
        .unwrap()
        .write_all(&[0; 24])
        .unwrap();
    assert!(matches!(Database::open(&tmp.0), Err(Error::Corrupt(_))));
}

#[test]
fn adapters_preserve_volume_hardlinks_and_fund_identity() {
    use fdb::adapters::*;
    let a = file_entry("shared", "C", u64::MAX, 5, "a", "C:/a", 0).unwrap();
    let b = file_entry("shared", "D", u64::MAX, 5, "a", "D:/a", 0).unwrap();
    let c = file_entry("shared", "C", u64::MAX, 6, "a", "C:/b/a", 0).unwrap();
    assert_ne!(a.key, b.key);
    assert_ne!(a.key, c.key);
    let report = fundops_record(
        "shared",
        "kap_reports",
        "1",
        BTreeMap::from([("fund_code".into(), json!("ABC"))]),
        "rapor metni",
    )
    .unwrap();
    assert_eq!(report.links, vec![Key::new("shared", "funds", "ABC")]);
}

#[test]
fn acknowledged_commit_survives_process_kill() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    let tmp = Temp::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fdb"))
        .arg(&tmp.0)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let request =
        json!({"op":"commit", "mutations":[{"op":"put", "document":doc("1", "kalıcı içerik")}]});
    let input = child.stdin.as_mut().unwrap();
    writeln!(input, "{request}").unwrap();
    input.flush().unwrap();
    let mut response = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut response)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&response).unwrap()["ok"],
        true
    );
    child.kill().unwrap();
    child.wait().unwrap();
    let db = Database::open(&tmp.0).unwrap();
    assert_eq!(
        db.get(&doc("1", "").key).unwrap().unwrap().document.text,
        "kalıcı içerik"
    );
}

#[test]
fn multi_frame_checkpoint_preserves_versions_and_can_accept_writes() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let text = "abc ".repeat(250_000);
    for id in 0..10 {
        db.commit(vec![put(doc(&id.to_string(), &text))]).unwrap();
    }
    db.compact().unwrap();
    drop(db);
    let mut db = Database::open(&tmp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 10);
    assert_eq!(db.get(&doc("0", "").key).unwrap().unwrap().version, 1);
    assert_eq!(db.commit(vec![put(doc("after", "yeni"))]).unwrap(), 11);
    drop(db);
    assert_eq!(
        Database::open(&tmp.0).unwrap().stats().unwrap().documents,
        11
    );
}

#[test]
fn empty_checkpoint_after_deleting_all_documents_reopens() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    db.commit(vec![put(doc("1", "x"))]).unwrap();
    db.commit(vec![Mutation::Delete {
        key: doc("1", "").key,
        guard: Guard::Any,
    }])
    .unwrap();
    db.compact().unwrap();
    drop(db);
    let mut db = Database::open(&tmp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 0);
    assert_eq!(db.commit(vec![put(doc("2", "x"))]).unwrap(), 3);
}
