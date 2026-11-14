use super::*;
use crate::reclaim::tests::{Temp, doc};

fn put(id: &str, text: &str) -> Mutation {
    let mut document = doc(id);
    document.text = text.into();
    Mutation::Put {
        document,
        guard: Guard::Any,
    }
}
fn tail(db: &mut Database) {
    db.commit_streamed([
        Ok(put("a", "newmarker東京")),
        Ok(Mutation::Delete {
            key: doc("b").key,
            guard: Guard::Any,
        }),
        Ok(put("c", "insertmarker")),
    ])
    .unwrap();
}
fn oracle(db: &Database) {
    assert_eq!(db.stats().unwrap().documents, 2);
    assert_eq!(
        db.get(&doc("a").key).unwrap().unwrap().document.text,
        "newmarker東京"
    );
    assert!(db.get(&doc("b").key).unwrap().is_none());
    for (text, expected) in [("oldmarker", 0), ("newmarker東京", 1), ("insertmarker", 1)] {
        let mut q = Query::new("shared");
        q.text = text.into();
        assert_eq!(db.search(&q).unwrap().total, expected, "{text}");
    }
}
fn seed(t: &Temp) -> Database {
    let mut db = Database::open(&t.0).unwrap();
    db.set_streamed_wal_layer(true).unwrap();
    db.commit_streamed([Ok(put("a", "oldmarker")), Ok(put("b", "oldmarker"))])
        .unwrap();
    db
}

#[test]
fn hundred_entry_streamed_epochs_use_wal_proofs_without_resident_payloads() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    db.set_streamed_wal_layer(true).unwrap();
    let reader = db.snapshot_reader().unwrap();
    for revision in ["oldmarker", "newmarker東京"] {
        let seq = db
            .commit_streamed((0..100).map(|n| Ok(put(&format!("{n:04}"), revision))))
            .unwrap();
        let view = reader.acquire().unwrap();
        assert_eq!(view.sequence(), seq);
        let mut q = Query::new("shared");
        q.text = revision.into();
        assert_eq!(view.search(&q).unwrap().total, 100);
        assert_eq!(db.stats().unwrap().resident_documents, 0);
        assert_eq!(db.stats().unwrap().resident_text_bytes, 0);
    }
    let mut q = Query::new("shared");
    q.text = "oldmarker".into();
    assert_eq!(reader.acquire().unwrap().search(&q).unwrap().total, 0);
    drop(reader);
    drop(db);
    assert_eq!(
        Database::open(&t.0).unwrap().stats().unwrap().documents,
        100
    );
}

#[test]
fn frozen_prefix_tail_relocation_and_last_snapshot_wal_pin() {
    let t = Temp::new();
    let mut db = seed(&t);
    let reader = db.snapshot_reader().unwrap();
    let old = reader.acquire().unwrap();
    let old_path = manifest::path(&t.0, db.active.id, "fdb");
    let block = test_jobs::pause(&t.0);
    assert!(db.start_light_flush().unwrap());
    block.wait_entered();
    tail(&mut db);
    let tail_view = reader.acquire().unwrap();
    assert!(db.light_flush.is_some());
    assert!(!db.light_flush_ready(false).unwrap());
    drop(block);
    assert!(db.light_flush_ready(true).unwrap());
    oracle(&db);
    assert!(old_path.exists());
    assert_eq!(
        old.get(&doc("a").key).unwrap().unwrap().document.text,
        "oldmarker"
    );
    assert!(tail_view.get(&doc("b").key).unwrap().is_none());
    assert_eq!(reader.acquire().unwrap().sequence(), db.sequence);
    drop(old);
    assert!(old_path.exists());
    assert!(tail_view.release().cleanup_error.is_none());
    assert!(!old_path.exists());
    drop(reader);
    db.check_integrity().unwrap();
    drop(db);
    oracle(&Database::open(&t.0).unwrap());
}

#[test]
fn wal_prefix_install_invalidates_writer_visibility_masks() {
    fn check(db: &Database, query: &Query, sequence: u64, total: usize, text: &str) {
        let expected = db.get(&doc("a").key).unwrap().unwrap();
        assert_eq!(expected.document.text, text);
        assert_eq!(expected.version, sequence);
        let exact = db.search(query).unwrap();
        assert_eq!(exact.total, total);
        assert_eq!(exact.sequence, sequence);
        let capped = db.search_first(query).unwrap();
        assert_eq!(capped.sequence, sequence);
        assert_eq!(capped.hits, exact.hits);
        assert_eq!(exact.hits.len(), total);
        assert_eq!(exact.hits[0], expected);
    }

    let t = Temp::new();
    let mut db = seed(&t);
    db.flush().unwrap();
    let mut query = Query::new("shared");
    query.text = "oldmarker".into();
    query.limit = 3;
    // Prime the base run's mask on the writer, independently of the reader.
    check(&db, &query, 1, 2, "oldmarker");
    let reader = db.snapshot_reader().unwrap();
    let old = reader.acquire().unwrap();
    db.commit_streamed([
        Ok(put("a", "oldmarker revised")),
        Ok(Mutation::Delete {
            key: doc("b").key,
            guard: Guard::Any,
        }),
    ])
    .unwrap();
    check(&db, &query, 2, 1, "oldmarker revised");
    assert!(db.start_light_flush().unwrap());
    assert!(db.light_flush_ready(true).unwrap());
    assert_eq!(db.runs.len(), 2);
    assert!(db.shadow.is_empty());
    check(&db, &query, 2, 1, "oldmarker revised");
    for (view, sequence, total, text) in [
        (reader.acquire().unwrap(), 2, 1, "oldmarker revised"),
        (old, 1, 2, "oldmarker"),
    ] {
        let exact = view.search(&query).unwrap();
        assert_eq!((exact.sequence, exact.total), (sequence, total));
        assert_eq!(view.search_first(&query).unwrap().hits, exact.hits);
        assert_eq!(exact.hits[0].document.text, text);
        assert_eq!(exact.hits[0].version, sequence);
    }
    drop((reader, db));
    let reopened = Database::open(&t.0).unwrap();
    reopened.check_integrity().unwrap();
    check(&reopened, &query, 2, 1, "oldmarker revised");
}

// Isolated witness of the old test pause's lock cycle. The timeout releases
// the artificial lock so this probe never leaves a deadlocked process behind.
#[test]
#[ignore = "isolated global pause/budget lock-cycle witness"]
fn global_source_pause_and_streamed_budget_cycle_witness() {
    let t = Temp::new();
    let other = Temp::new();
    let mut db = seed(&t);
    let mut fallback = Database::open(&other.0).unwrap();
    let block = maintenance_workspace().unwrap();
    eprintln!(
        "witness parent {:?} holds source workspace",
        std::thread::current().id()
    );
    let (other_tx, other_rx) = std::sync::mpsc::channel();
    let builder = std::thread::spawn(move || {
        let result = fallback.commit_streamed([Ok(put("other", "othermarker"))]);
        other_tx.send(result).unwrap();
        fallback
    });
    let began = std::time::Instant::now();
    while crate::write_budget::working_snapshot().in_use_bytes != 16 * 1024 * 1024
        && began.elapsed().as_secs() < 3
    {
        std::thread::yield_now();
    }
    let occupied = crate::write_budget::working_snapshot().in_use_bytes;
    let waits = crate::write_budget::working_snapshot().waits;
    let (tx, rx) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        eprintln!(
            "witness writer {:?} requests streamed budget",
            std::thread::current().id()
        );
        let result = db.commit_streamed([Ok(put("cycle", "cyclemarker"))]);
        tx.send(result).unwrap();
        db
    });
    let early = rx.recv_timeout(std::time::Duration::from_millis(100));
    let waited = early.is_err();
    let wait_count = crate::write_budget::working_snapshot().waits;
    eprintln!("witness cycle observed; parent releases source workspace");
    drop(block);
    early
        .or_else(|_| rx.recv_timeout(std::time::Duration::from_secs(3)))
        .unwrap()
        .unwrap();
    other_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap()
        .unwrap();
    let db = writer.join().unwrap();
    let fallback = builder.join().unwrap();
    assert_eq!(occupied, 16 * 1024 * 1024);
    assert!(waited && wait_count > waits);
    assert_eq!(db.stats().unwrap().documents, 3);
    assert_eq!(fallback.stats().unwrap().documents, 1);
    db.check_integrity().unwrap();
    fallback.check_integrity().unwrap();
}

#[test]
fn local_worker_pause_does_not_block_another_database_streamed_builder() {
    let t = Temp::new();
    let other = Temp::new();
    let mut db = seed(&t);
    let mut fallback = Database::open(&other.0).unwrap();
    let pause = test_jobs::pause(&t.0);
    assert!(db.start_light_flush().unwrap());
    pause.wait_entered();
    let (tx, rx) = std::sync::mpsc::channel();
    let builder = std::thread::spawn(move || {
        let result = fallback.commit_streamed([Ok(put("other", "othermarker"))]);
        tx.send(result).unwrap();
        fallback
    });
    let result = rx.recv_timeout(std::time::Duration::from_secs(10));
    // Unblock before assertions/joins, including the error/unwind path.
    drop(pause);
    let fallback = builder.join().unwrap();
    result.unwrap().unwrap();
    db.light_flush_ready(true).unwrap();
    assert_eq!(fallback.stats().unwrap().documents, 1);
    db.check_integrity().unwrap();
}

#[test]
#[ignore = "process helper for background WAL prefix publication"]
fn process_child() {
    let Some(path) = std::env::var_os("FDB_PREFIX_CHILD") else {
        return;
    };
    let t = Temp(path.into());
    let mut db = seed(&t);
    let stage = std::env::var("FDB_PREFIX_KILL").unwrap();
    let block = (stage == "build").then(|| test_jobs::pause(&t.0));
    db.start_light_flush().unwrap();
    if let Some(block) = &block {
        block.wait_entered();
    }
    tail(&mut db);
    if block.is_some() {
        kill_barrier("build");
    }
    db.light_flush_ready(true).unwrap();
    oracle(&db);
    kill_barrier("ack");
}

#[test]
fn process_kill_build_before_after_publish_and_ack_replays_one_consistent_tail() {
    for stage in ["build", "before_publish", "after_publish", "ack"] {
        let t = Temp::new();
        let marker = t.0.join("prefix-marker");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "engine::light_flush::tests::process_child",
                "--nocapture",
            ])
            .env("FDB_PREFIX_CHILD", &t.0)
            .env("FDB_PREFIX_KILL", stage)
            .env("FDB_PREFIX_MARKER", &marker)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let start = std::time::Instant::now();
        while !marker.exists()
            && start.elapsed().as_secs() < 15
            && child.try_wait().unwrap().is_none()
        {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let observed = marker.exists();
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            observed,
            "{stage}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let db = Database::open(&t.0).unwrap();
        oracle(&db);
        db.check_integrity().unwrap();
    }
}
