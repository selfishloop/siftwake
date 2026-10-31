use super::*;
use crate::reclaim::tests::{Temp, doc};
fn put(id: usize, text: &str) -> Mutation {
    let mut document = doc(&format!("{id:04}"));
    document.text = format!("{text} {}", "payload ".repeat(300));
    Mutation::Put {
        document,
        guard: Guard::Any,
    }
}
fn q(text: &str) -> Query {
    let mut q = Query::new("shared");
    q.text = text.into();
    q
}
fn seeded(t: &Temp) -> Database {
    let mut db = Database::open(&t.0).unwrap();
    db.commit((0..80).map(|id| put(id, "old")).collect())
        .unwrap();
    db.flush().unwrap();
    db
}
#[test]
fn owned_delta_snapshots_are_atomic_and_stay_at_their_commit_boundary() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Snapshot>();
    send_sync::<SnapshotReader>();
    let t = Temp::new();
    let mut db = seeded(&t);
    let reader = db.snapshot_reader().unwrap();
    let old = reader.acquire().unwrap();
    db.commit(vec![
        put(0, "new"),
        Mutation::Delete {
            key: doc("0001").key,
            guard: Guard::Any,
        },
    ])
    .unwrap();
    let new = reader.acquire().unwrap();
    assert!(new.sequence() > old.sequence());
    assert_eq!(old.search(&q("old")).unwrap().total, 80);
    assert_eq!(new.search(&q("old")).unwrap().total, 78);
    assert_eq!(new.search(&q("new")).unwrap().total, 1);
    assert!(new.get(&doc("0001").key).unwrap().is_none());
    db.flush().unwrap();
    assert_eq!(new.search(&q("old")).unwrap().total, 78);
    assert_eq!(old.search(&q("old")).unwrap().total, 80);
    assert_eq!(new.search(&q("new")).unwrap().sequence, new.sequence());
}
#[cfg(feature = "experimental-paged-metadata")]
#[test]
fn lazy_sparse_mask_after_budget_miss_does_not_claim_wal_shadow_coverage() {
    let t = Temp::new();
    let mut db = seeded(&t);
    let reader = db.snapshot_reader().unwrap();
    db.commit(vec![put(0, "new")]).unwrap();
    let view = reader.acquire().unwrap();
    {
        // Simulate a snapshot run omitted by the bounded dense-mask cache.
        let mut cache = view.view().mask_cache.lock().unwrap();
        cache.entries.clear();
        cache.bytes = 0;
    }
    assert_eq!(view.search(&q("old")).unwrap().total, 79);
    assert_eq!(view.search(&q("old")).unwrap().total, 79);
    db.commit(vec![put(1, "new")]).unwrap();
    assert_eq!(
        reader.acquire().unwrap().search(&q("old")).unwrap().total,
        78
    );
    assert_eq!(view.search(&q("old")).unwrap().total, 79);
}
#[test]
fn four_readers_observe_whole_update_insert_delete_epochs() {
    fn verify(view: &Snapshot) {
        let sequence = view.sequence();
        let token = format!("epoch{sequence:010}");
        for id in 0..8 {
            let row = view.get(&doc(&format!("{id:04}")).key).unwrap();
            if id == 7 && sequence.is_multiple_of(2) {
                assert!(row.is_none());
            } else {
                let row = row.unwrap();
                assert_eq!(row.version, sequence);
                assert!(row.document.text.starts_with(&token));
            }
        }
        assert_eq!(
            view.document_count(),
            8 - usize::from(sequence.is_multiple_of(2))
        );
    }
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    db.commit((0..8).map(|id| put(id, "epoch0000000001")).collect())
        .unwrap();
    let reader = db.snapshot_reader().unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(5));
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let reader = reader.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                for sequence in 2u64..32 {
                    let old = reader.acquire().unwrap();
                    assert_eq!(old.sequence(), sequence - 1);
                    barrier.wait();
                    verify(&old);
                    barrier.wait();
                    verify(&old);
                    let current = reader.acquire().unwrap();
                    assert_eq!(current.sequence(), sequence);
                    verify(&current);
                }
            })
        })
        .collect();
    for sequence in 2u64..32 {
        barrier.wait();
        let mutations = (0..8)
            .map(|id| {
                if id == 7 && sequence.is_multiple_of(2) {
                    Mutation::Delete {
                        key: doc("0007").key,
                        guard: Guard::Any,
                    }
                } else {
                    put(id, &format!("epoch{sequence:010}"))
                }
            })
            .collect();
        assert_eq!(db.commit(mutations).unwrap(), sequence);
        barrier.wait();
    }
    for reader in readers {
        reader.join().unwrap();
    }
}
#[test]
fn last_reader_unpins_old_segment_and_active_wal_without_another_write() {
    let t = Temp::new();
    let mut db = seeded(&t);
    let path = db.runs[0].source.path(&t.0, "fdb");
    let wal = manifest::path(&t.0, db.active.id, "fdb");
    let reader = db.snapshot_reader().unwrap();
    let old = reader.acquire().unwrap();
    db.commit((0..80).map(|id| put(id, "new")).collect())
        .unwrap();
    db.flush().unwrap();
    assert!(db.start_revision_reclamation().unwrap());
    db.wait_for_compaction().unwrap();
    assert!(path.exists() && wal.exists());
    assert_eq!(old.search(&q("old")).unwrap().total, 80);
    assert_eq!(
        reader.acquire().unwrap().search(&q("new")).unwrap().total,
        80
    );
    let released = old.release();
    assert!(released.cleanup_error.is_none(), "{released:?}");
    assert!(!path.exists());
    assert!(!wal.exists());
}
#[test]
fn simultaneous_final_releases_report_cleanup_exactly_once() {
    let t = Temp::new();
    let mut db = seeded(&t);
    let path = db.runs[0].source.path(&t.0, "fdb");
    let reader = db.snapshot_reader().unwrap();
    let first = reader.acquire().unwrap();
    let second = first.clone();
    db.commit((0..80).map(|id| put(id, "new")).collect())
        .unwrap();
    db.flush().unwrap();
    assert!(db.start_revision_reclamation().unwrap());
    db.wait_for_compaction().unwrap();
    assert!(path.exists());
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let releases: Vec<_> = [first, second]
        .into_iter()
        .map(|snapshot| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                snapshot.release()
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = releases.into_iter().map(|r| r.join().unwrap()).collect();
    assert!(results.iter().all(|r| r.cleanup_error.is_none()));
    assert_eq!(results.iter().filter(|r| r.reclaimed_files != 0).count(), 1);
    assert!(!path.exists());
}
#[test]
fn snapshots_keep_lock_and_readability_after_writer_closes() {
    let t = Temp::new();
    let mut db = seeded(&t);
    let reader = db.snapshot_reader().unwrap();
    let snapshot = reader.acquire().unwrap();
    drop(db);
    assert!(matches!(Database::open(&t.0), Err(Error::Locked)));
    assert_eq!(snapshot.search(&q("old")).unwrap().total, 80);
    drop(reader);
    drop(snapshot);
    Database::open(&t.0).unwrap().check_integrity().unwrap();
}
#[test]
fn last_reader_cleanup_rejects_a_changed_authoritative_root() {
    let t = Temp::new();
    let mut db = seeded(&t);
    let path = db.runs[0].source.path(&t.0, "fdb");
    let reader = db.snapshot_reader().unwrap();
    let old = reader.acquire().unwrap();
    db.commit((0..80).map(|id| put(id, "new")).collect())
        .unwrap();
    db.flush().unwrap();
    assert!(db.start_revision_reclamation().unwrap());
    db.wait_for_compaction().unwrap();
    let root = manifest::path(&t.0, db.generation, "ready");
    let original = fs::read(&root).unwrap();
    let mut corrupt = original.clone();
    corrupt[0] ^= 1;
    fs::write(&root, corrupt).unwrap();
    let released = old.release();
    assert!(released.cleanup_error.is_some());
    assert!(path.exists());
    fs::write(root, original).unwrap();
    db.reclaim_obsolete().unwrap();
    assert!(!path.exists());
}
#[cfg(feature = "bounded-engine")]
#[test]
fn rejected_stream_does_not_publish_a_partial_view() {
    let t = Temp::new();
    let mut db = seeded(&t);
    let reader = db.snapshot_reader().unwrap();
    let before = reader.acquire().unwrap().sequence();
    let result = db.commit_streamed((0..80).map(|id| {
        if id == 40 {
            Err(Error::Invalid("rejected input".into()))
        } else {
            Ok(put(id, "new"))
        }
    }));
    assert!(result.is_err());
    let snapshot = reader.acquire().unwrap();
    assert_eq!(snapshot.sequence(), before);
    assert_eq!(snapshot.search(&q("old")).unwrap().total, 80);
    assert_eq!(snapshot.search(&q("new")).unwrap().total, 0);
    db.commit_streamed((0..80).map(|id| Ok(put(id, "new"))))
        .unwrap();
    assert_eq!(snapshot.search(&q("old")).unwrap().total, 80);
    assert_eq!(
        reader.acquire().unwrap().search(&q("new")).unwrap().total,
        80
    );
}
#[cfg(feature = "bounded-engine")]
#[test]
fn paused_stream_never_publishes_half_of_a_group() {
    let t = Temp::new();
    let mut db = seeded(&t);
    let reader = db.snapshot_reader().unwrap();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (go_tx, go_rx) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        let seq = db
            .commit_streamed((0..80).map(|id| {
                if id == 40 {
                    ready_tx.send(()).unwrap();
                    go_rx.recv().unwrap();
                }
                Ok(put(id, "new"))
            }))
            .unwrap();
        (db, seq)
    });
    ready_rx.recv().unwrap();
    let snapshot = reader.acquire().unwrap();
    assert_eq!(snapshot.search(&q("old")).unwrap().total, 80);
    assert_eq!(snapshot.search(&q("new")).unwrap().total, 0);
    go_tx.send(()).unwrap();
    let (_db, seq) = writer.join().unwrap();
    assert_eq!(snapshot.search(&q("old")).unwrap().total, 80);
    let current = reader.acquire().unwrap();
    assert_eq!(current.sequence(), seq);
    assert_eq!(current.search(&q("new")).unwrap().total, 80);
}
#[cfg(feature = "experimental-write-maintenance-slots")]
#[test]
fn writer_cancels_rewrite_while_snapshot_keeps_old_source_pinned() {
    let t = Temp::new();
    let mut db = seeded(&t);
    let path = db.runs[0].source.path(&t.0, "fdb");
    let reader = db.snapshot_reader().unwrap();
    let old = reader.acquire().unwrap();
    db.commit((0..40).map(|id| put(id, "new")).collect())
        .unwrap();
    db.flush().unwrap();
    let block = test_jobs::pause(&t.0);
    assert!(db.start_revision_reclamation().unwrap());
    block.wait_entered();
    let cancel = db.job.as_ref().unwrap().cancel.as_ref().unwrap().clone();
    let writer = std::thread::spawn(move || {
        db.commit(vec![put(79, "latest")]).unwrap();
        db
    });
    while !cancel.is_cancelled() {
        std::thread::yield_now();
    }
    assert_eq!(old.search(&q("old")).unwrap().total, 80);
    drop(block);
    let mut db = writer.join().unwrap();
    assert!(path.exists());
    assert_eq!(
        reader
            .acquire()
            .unwrap()
            .search(&q("latest"))
            .unwrap()
            .total,
        1
    );
    assert!(db.start_revision_reclamation().unwrap());
    db.wait_for_compaction().unwrap();
    assert!(path.exists());
    old.release();
    assert!(!path.exists());
}
