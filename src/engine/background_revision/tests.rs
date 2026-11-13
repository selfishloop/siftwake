use super::*;
use crate::reclaim::tests::{Temp, doc};
use std::time::{Duration, Instant};

fn put(id: usize, marker: &str) -> Mutation {
    let mut document = doc(&format!("{id:04}"));
    document.text = format!("{marker} {}", "payload ".repeat(256));
    Mutation::Put {
        document,
        guard: Guard::Any,
    }
}

fn prepared(t: &Temp) -> Database {
    let mut db = Database::open(&t.0).unwrap();
    db.commit((0..160).map(|n| put(n, "before")).collect())
        .unwrap();
    db.flush().unwrap();
    let mut changes: Vec<_> = (0..80).map(|n| put(n, "after")).collect();
    changes.push(Mutation::Delete {
        key: doc("0080").key,
        guard: Guard::Any,
    });
    db.commit(changes).unwrap();
    db.flush().unwrap();
    db
}

fn wait_ready(db: &Database) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while db.background_revision.running() {
        assert!(Instant::now() < deadline, "revision worker did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(db.background_revision.pending());
}

#[test]
fn default_write_schedules_revision_and_never_joins_a_paused_worker() {
    let t = Temp::new();
    let mut db = prepared(&t);
    let reader = db.snapshot_reader().unwrap();
    let old = reader.acquire().unwrap();
    let old_path = db.runs[0].source.path(&t.0, "fdb");
    let pause = test_jobs::pause_revision(&t.0);
    let sequence = db.commit(vec![put(0, "newest")]).unwrap();
    pause.wait_entered();
    assert!(db.revision_reclamation_running());
    let (send, receive) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        #[cfg(feature = "bounded-engine")]
        let result = db.commit_streamed((200..329).map(|id| Ok(put(id, "appended"))));
        #[cfg(not(feature = "bounded-engine"))]
        let result = db.commit(vec![put(200, "appended")]);
        send.send(result).unwrap();
        db
    });
    let written = receive.recv_timeout(Duration::from_secs(10));
    drop(pause);
    let mut db = writer.join().unwrap();
    let written = written
        .expect("foreground waited for the paused revision")
        .unwrap();
    assert_eq!(reader.acquire().unwrap().sequence(), written);
    db.maintain_idle().unwrap();
    assert_eq!(db.get(&doc("0000").key).unwrap().unwrap().version, sequence);
    assert!(db.get(&doc("0080").key).unwrap().is_none());
    assert!(old_path.is_file());
    assert!(
        old.get(&doc("0000").key)
            .unwrap()
            .unwrap()
            .document
            .text
            .starts_with("after")
    );
    drop(old);
    drop(reader);
    db.check_integrity().unwrap();
}

#[test]
fn finished_revision_is_published_only_for_the_captured_prefix() {
    for replace in [false, true] {
        let t = Temp::new();
        let mut db = prepared(&t);
        assert!(db.poll_maintenance().unwrap());
        wait_ready(&db);
        if replace {
            assert!(db.start_normal_compaction(false).unwrap());
            assert!(db.wait_for_merge().unwrap());
        }
        let before = db.descriptors();
        assert_eq!(db.finish_background_revision(false).unwrap(), !replace);
        assert_eq!(db.descriptors() == before, replace);
        assert!(db.poll_maintenance().unwrap());
        db.check_integrity().unwrap();
    }
}

#[test]
fn disabling_default_reclaim_discards_even_a_finished_rewrite() {
    let t = Temp::new();
    let mut db = prepared(&t);
    assert!(db.poll_maintenance().unwrap());
    wait_ready(&db);
    let before = db.descriptors();
    db.set_revision_reclamation(false);
    assert!(!db.wait_for_compaction().unwrap());
    assert_eq!(db.descriptors(), before);
    db.commit(vec![put(200, "disabled")]).unwrap();
    assert!(!db.background_revision.pending());
    db.set_revision_reclamation(true);
    assert!(db.poll_maintenance().unwrap()); // deferred file collection
    assert!(db.poll_maintenance().unwrap()); // new plan
    assert!(db.wait_for_compaction().unwrap());
}

#[test]
#[ignore = "subprocess for unpublished background output recovery"]
fn unpublished_output_child() {
    let dir = PathBuf::from(std::env::var_os("FDB_BG_CRASH_TEST_DIR").unwrap());
    let mut db = Database::open(&dir).unwrap();
    assert!(db.poll_maintenance().unwrap());
    wait_ready(&db);
    let pending = db.background_revision.job.as_ref().unwrap();
    let name = format!("run-{:016x}.fdb", pending.id);
    assert!(dir.join(&name).is_file());
    std::fs::write(dir.join("bg-crash-ready"), name).unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn kill_before_revision_publication_keeps_source_and_cleans_output() {
    let t = Temp::new();
    let db = prepared(&t);
    let sequence = db.sequence;
    let before = db.descriptors();
    drop(db);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "engine::background_revision::tests::unpublished_output_child",
        ])
        .env("FDB_BG_CRASH_TEST_DIR", &t.0)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let marker = t.0.join("bg-crash-ready");
    let filename = loop {
        if let Ok(name) = std::fs::read_to_string(&marker)
            && !name.is_empty()
        {
            break Some(name);
        }
        if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
            break None;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
    }
    assert!(!child.wait().unwrap().success());
    let output = t.0.join(filename.expect("unpublished output witness"));
    assert!(output.is_file());
    let mut db = Database::open(&t.0).unwrap();
    assert_eq!(db.sequence, sequence);
    assert_eq!(db.descriptors(), before);
    assert!(db.get(&doc("0080").key).unwrap().is_none());
    db.maintain_idle().unwrap();
    assert!(!output.exists());
    db.check_integrity().unwrap();
}
