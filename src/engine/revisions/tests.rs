use super::*;
use crate::reclaim::tests::{Temp, doc};

fn put(id: usize, revision: &str) -> Mutation {
    let mut document = doc(&format!("{id:04}"));
    document.text = format!("{revision} {}", "payload ".repeat(256));
    Mutation::Put {
        document,
        guard: Guard::Any,
    }
}

fn seed(t: &Temp) -> Database {
    let mut db = Database::open(&t.0).unwrap();
    db.commit((0..80).map(|id| put(id, "oldrevision")).collect())
        .unwrap();
    db.flush().unwrap();
    db.commit(
        (0..40)
            .map(|id| {
                if id < 20 {
                    put(id, "newrevision")
                } else {
                    Mutation::Delete {
                        key: doc(&format!("{id:04}")).key,
                        guard: Guard::Any,
                    }
                }
            })
            .collect(),
    )
    .unwrap();
    db.flush().unwrap();
    db
}

#[test]
fn rewrite_preserves_barriers_pins_pending_delta_and_reused_postings() {
    let t = Temp::new();
    let mut db = seed(&t);
    let old = db.runs[0].clone();
    let path = old.source.path(&t.0, "fdb");
    let before = db.segment_liveness().unwrap();
    assert_eq!(before[0].dead_records, 40);
    assert!(before[0].dead_payload_bytes >= 64 * 1024);
    assert!(db.start_revision_reclamation().unwrap());
    // Query the published view while its replacement is being constructed.
    let mut query = Query::new("shared");
    query.text = "newrevision".into();
    assert_eq!(db.search(&query).unwrap().total, 20);
    query.text = "oldrevision".into();
    assert_eq!(db.search(&query).unwrap().total, 40);
    // A later unflushed update must win even if worker completion races it.
    db.commit(vec![put(79, "latestrevision")]).unwrap();
    let sequence = db.sequence;
    db.wait_for_compaction().unwrap();
    // Responsive work may have been cancelled by that writer. Complete its
    // retry while the new delta is still resident, then test pin retirement.
    if db.runs.iter().any(|r| r.source.path(&t.0, "fdb") == path) {
        assert!(db.start_revision_reclamation().unwrap());
        db.wait_for_compaction().unwrap();
    }
    assert_eq!(db.sequence, sequence);
    #[cfg(all(windows, feature = "bounded-engine"))]
    {
        let rewritten = db
            .runs
            .iter()
            .find(|r| r.source.id != old.source.id && r.source.sequence == old.source.sequence)
            .unwrap();
        assert!(
            OpenOptions::new()
                .write(true)
                .open(rewritten.source.path(&t.0, "fdb"))
                .is_err(),
            "validated output must retain an immutable source lease"
        );
    }
    assert_eq!(db.count, 60);
    assert!(path.exists());
    assert!(
        old.index
            .get(&doc("0000").key)
            .unwrap()
            .unwrap()
            .document
            .text
            .starts_with("oldrevision")
    );
    for id in 20..40 {
        assert!(db.get(&doc(&format!("{id:04}")).key).unwrap().is_none());
    }
    assert!(
        db.get(&doc("0079").key)
            .unwrap()
            .unwrap()
            .document
            .text
            .starts_with("latestrevision")
    );
    db.check_integrity().unwrap();
    drop(old);
    db.reclaim_obsolete().unwrap();
    assert!(!path.exists());
    drop(db);
    let db = Database::open(&t.0).unwrap();
    assert_eq!(db.search(&query).unwrap().total, 39);
    db.check_integrity().unwrap();
}

#[test]
fn fully_dead_sources_retire_without_an_output_and_respect_reader_pin() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    db.commit((0..80).map(|id| put(id, "original")).collect())
        .unwrap();
    db.flush().unwrap();
    let pinned = db.runs[0].clone();
    let old = pinned.source.path(&t.0, "fdb");
    db.commit((0..80).map(|id| put(id, "replacement")).collect())
        .unwrap();
    db.flush().unwrap();
    let sequence = db.sequence;
    assert_eq!(db.segment_liveness().unwrap()[0].live_records, 0);
    assert!(db.start_revision_reclamation().unwrap());
    let output = db.job.as_ref().unwrap().output_id;
    assert!(db.wait_for_compaction().unwrap());
    assert!(!t.0.join(format!("run-{output:016x}.fdb")).exists());
    assert_eq!(db.runs.len(), 1);
    assert_eq!(db.sequence, sequence);
    assert!(old.exists());
    assert!(
        pinned
            .index
            .get(&doc("0000").key)
            .unwrap()
            .unwrap()
            .document
            .text
            .starts_with("original")
    );
    drop(pinned);
    db.reclaim_obsolete().unwrap();
    assert!(!old.exists());
    db.check_integrity().unwrap();
    drop(db);
    assert!(
        Database::open(&t.0)
            .unwrap()
            .get(&doc("0000").key)
            .unwrap()
            .unwrap()
            .document
            .text
            .starts_with("replacement")
    );
}

#[test]
fn cancellation_never_publishes_or_discards_acknowledged_revisions() {
    let t = Temp::new();
    let mut db = seed(&t);
    let generation = db.generation;
    #[cfg(feature = "experimental-write-maintenance-slots")]
    let block = maintenance_workspace().unwrap();
    assert!(db.start_revision_reclamation().unwrap());
    db.job.as_ref().unwrap().cancel.as_ref().unwrap().cancel();
    #[cfg(feature = "experimental-write-maintenance-slots")]
    drop(block);
    let installed = db.wait_for_compaction().unwrap();
    #[cfg(feature = "experimental-write-maintenance-slots")]
    assert!(!installed);
    if !installed {
        assert_eq!(db.generation, generation);
    }
    assert!(
        db.get(&doc("0000").key)
            .unwrap()
            .unwrap()
            .document
            .text
            .starts_with("newrevision")
    );
    assert!(db.get(&doc("0020").key).unwrap().is_none());
    #[cfg(feature = "bounded-engine")]
    db.commit_streamed([Ok(put(79, "aftercancel"))]).unwrap();
    #[cfg(not(feature = "bounded-engine"))]
    db.commit(vec![put(79, "aftercancel")]).unwrap();
    db.check_integrity().unwrap();
    drop(db);
    assert!(
        Database::open(&t.0)
            .unwrap()
            .get(&doc("0079").key)
            .unwrap()
            .unwrap()
            .document
            .text
            .starts_with("aftercancel")
    );
}

#[test]
fn query_visibility_pruning_keeps_updates_deletes_and_empty_segment_totals() {
    let t = Temp::new();
    let mut db = seed(&t);
    db.set_query_visibility_pruning(true);
    for (text, total) in [
        ("oldrevision", 40),
        ("newrevision", 20),
        ("oldrevision payload", 40),
        ("newrevision payload", 20),
        ("oldrevision newrevision", 0),
        ("payload", 60),
    ] {
        let mut query = Query::new("shared");
        query.text = text.into();
        let page = db.search(&query).unwrap();
        assert_eq!(page.total, total, "{text}");
        assert!(
            page.hits
                .iter()
                .all(|d| !(20..40).contains(&d.document.key.id.parse::<usize>().unwrap()))
        );
    }
    // Every prior segment is now masked, including its tombstones. The new
    // complete revision must be the sole visible result set, before and after
    // flush. This exercises the empty-run shortcut as well as cached AND lists.
    db.commit((0..80).map(|id| put(id, "finalrevision")).collect())
        .unwrap();
    for flush in [false, true] {
        if flush {
            db.flush().unwrap();
        }
        for (text, total) in [
            ("oldrevision", 0),
            ("newrevision", 0),
            ("finalrevision payload", 80),
        ] {
            let mut q = Query::new("shared");
            q.text = text.into();
            assert_eq!(db.search(&q).unwrap().total, total);
        }
    }
    db.check_integrity().unwrap();
}

#[test]
fn reopen_rejects_truncated_and_single_byte_corrupt_source_with_good_cache() {
    use std::io::{Read, Seek, SeekFrom, Write};
    for truncate in [false, true] {
        let t = Temp::new();
        let db = seed(&t);
        let path = db.runs[0].source.path(&t.0, "fdb");
        drop(db); // immutable Windows source leases deny concurrent mutation
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        if truncate {
            file.set_len(40).unwrap();
        } else {
            file.seek(SeekFrom::Start(60)).unwrap();
            let mut b = [0];
            file.read_exact(&mut b).unwrap();
            b[0] ^= 1;
            file.seek(SeekFrom::Start(60)).unwrap();
            file.write_all(&b).unwrap();
        }
        file.sync_all().unwrap();
        drop(file);
        assert!(Database::open(&t.0).is_err());
    }
}

#[test]
fn worker_rechecks_source_binding_before_publishing_replacement() {
    let t = Temp::new();
    let mut db = seed(&t);
    let generation = db.generation;
    // No live source can be mutated through its Windows immutable lease. An
    // incorrect worker binding still must fail before publication.
    db.runs[0].source.checksum ^= 1;
    assert!(db.start_revision_reclamation().unwrap());
    assert!(db.wait_for_compaction().is_err());
    assert_eq!(db.generation, generation);
    drop(db);
    Database::open(&t.0).unwrap().check_integrity().unwrap();
}
