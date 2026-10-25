use super::*;
use crate::reclaim::tests::{Temp, doc, put};
use std::sync::mpsc;
use std::time::Duration;

fn seed(t: &Temp) -> Database {
    let mut db = Database::open(&t.0).unwrap();
    for id in ["a", "b"] {
        db.commit(vec![put(id)]).unwrap();
        db.flush().unwrap();
    }
    db
}

#[test]
fn pinned_retired_reader_survives_merge_then_releases_all_owned_files() {
    let t = Temp::new();
    let mut db = seed(&t);
    let reader = db.runs[0].clone();
    let source = reader.source.path(&t.0, "fdb");
    assert!(db.start_compaction().unwrap());
    db.wait_for_compaction().unwrap();
    assert!(source.exists());
    assert_eq!(
        reader.index.get(&doc("a").key).unwrap().unwrap().document,
        doc("a")
    );
    assert_eq!(db.reclaim_obsolete().unwrap().removed_files, 0);
    drop(reader);
    assert!(db.reclaim_obsolete().unwrap().removed_files >= 2);
    assert!(!source.exists());
    assert!(!source.with_extension("idx").exists());
    db.check_integrity().unwrap();
}

#[test]
fn unpublished_worker_output_below_a_newer_root_is_pinned_until_install() {
    let t = Temp::new();
    let mut db = seed(&t);
    let id = db.allocate().unwrap();
    let sequence = db.sequence;
    let changes = vec![
        Change::Put {
            stored: db.get(&doc("a").key).unwrap().unwrap(),
        },
        Change::Put {
            stored: db.get(&doc("b").key).unwrap().unwrap(),
        },
    ];
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let root = t.0.clone();
    let handle = std::thread::spawn(move || {
        let run = write_run_prepared(&root, id, sequence, changes.into_iter().map(Ok), |_| Ok(()))?;
        ready_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(JobOutput::Replace(run))
    });
    db.job = Some(Job {
        inputs: db.descriptors(),
        output_id: id,
        handle,
        cancel: None,
        revision: false,
    });
    ready_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    db.commit(vec![put("c")]).unwrap();
    db.flush().unwrap();
    assert!(db.generation > id);
    let output = t.0.join(format!("run-{id:016x}.fdb"));
    let before = fs::read(&output).unwrap();
    db.reclaim_obsolete().unwrap();
    assert_eq!(fs::read(&output).unwrap(), before);
    assert!(output.with_extension("idx").exists());
    release_tx.send(()).unwrap();
    db.wait_for_compaction().unwrap();
    assert_eq!(db.stats().unwrap().documents, 3);
    db.check_integrity().unwrap();
    drop(db);
    let db = Database::open(&t.0).unwrap();
    for id in ["a", "b", "c"] {
        assert_eq!(db.get(&doc(id).key).unwrap().unwrap().document, doc(id));
    }
}

#[test]
fn drop_keeps_database_lock_until_worker_has_stopped() {
    let t = Temp::new();
    let mut db = seed(&t);
    let (release_tx, release_rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        release_rx.recv().unwrap();
        Err(Error::Invalid("test worker stopped".into()))
    });
    let id = db.allocate().unwrap();
    db.job = Some(Job {
        inputs: db.descriptors(),
        output_id: id,
        handle,
        cancel: None,
        revision: false,
    });
    let (start_tx, start_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let closing = std::thread::spawn(move || {
        start_tx.send(()).unwrap();
        drop(db);
        done_tx.send(()).unwrap();
    });
    start_rx.recv().unwrap();
    assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(matches!(Database::open(&t.0), Err(Error::Locked)));
    release_tx.send(()).unwrap();
    closing.join().unwrap();
    done_rx.recv().unwrap();
    Database::open(&t.0).unwrap().check_integrity().unwrap();
}

#[test]
fn future_orphan_id_is_not_reused_and_is_collected_after_next_publication() {
    let t = Temp::new();
    let db = seed(&t);
    let generation = db.generation;
    drop(db);
    let orphan = t.0.join(format!("run-{:016x}.fdb", generation + 4));
    fs::write(&orphan, b"unpublished").unwrap();
    let mut db = Database::open(&t.0).unwrap();
    assert!(orphan.exists());
    assert!(db.next_id > generation + 4);
    db.commit(vec![put("c")]).unwrap();
    db.flush().unwrap();
    assert!(!orphan.exists());
    assert!(db.generation > generation + 4);
    db.check_integrity().unwrap();
}

#[test]
fn active_wal_tail_updates_and_deletes_survive_reclamation() {
    let t = Temp::new();
    let mut db = seed(&t);
    db.commit(vec![
        put("c"),
        Mutation::Delete {
            key: doc("b").key,
            guard: Guard::Any,
        },
    ])
    .unwrap();
    let wal = db.active.id;
    let bytes = fs::read(manifest::path(&t.0, wal, "fdb")).unwrap();
    db.reclaim_obsolete().unwrap();
    assert_eq!(fs::read(manifest::path(&t.0, wal, "fdb")).unwrap(), bytes);
    drop(db);
    let db = Database::open(&t.0).unwrap();
    assert!(db.get(&doc("b").key).unwrap().is_none());
    assert_eq!(db.get(&doc("c").key).unwrap().unwrap().version, 3);
    assert_eq!(db.stats().unwrap().documents, 2);
}

#[cfg(windows)]
#[test]
fn locked_obsolete_file_can_be_retried_without_poisoning_writes() {
    use std::os::windows::fs::OpenOptionsExt;
    let t = Temp::new();
    let mut db = seed(&t);
    let old = t.0.join("generation-0000000000000001.fdb");
    fs::write(&old, b"old").unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&old)
        .unwrap();
    assert!(matches!(db.reclaim_obsolete(), Err(Error::Io(_))));
    db.commit(vec![put("c")]).unwrap();
    assert!(db.get(&doc("c").key).unwrap().is_some());
    drop(locked);
    assert!(db.reclaim_obsolete().unwrap().removed_files >= 1);
    assert!(!old.exists());
}
