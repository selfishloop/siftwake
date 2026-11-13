use super::*;
use crate::{Database, Document, Guard, Key, Mutation};
use std::path::PathBuf;

pub(crate) struct Temp(pub PathBuf);
impl Temp {
    pub(crate) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fdb-reclaim-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("fdb-reclaim-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}
pub(crate) fn doc(id: &str) -> Document {
    Document {
        key: Key::new("shared", "texts", id),
        title: "東京 İSTANBUL".into(),
        text: format!("actual source {id}"),
        fields: Default::default(),
        links: vec![],
    }
}
pub(crate) fn put(id: &str) -> Mutation {
    Mutation::Put {
        document: doc(id),
        guard: Guard::Any,
    }
}
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<_> = fs::read_dir(root)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().unwrap() != "LOCK")
        .map(|p| {
            let bytes = fs::read(&p).unwrap();
            (p, bytes)
        })
        .collect();
    files.sort();
    files
}
fn seed(t: &Temp) -> Database {
    let mut db = Database::open(&t.0).unwrap();
    for id in ["a", "b"] {
        db.commit(vec![put(id)]).unwrap();
        db.flush().unwrap();
    }
    db
}
#[test]
fn strict_open_removes_only_exact_scratch_names_and_refuses_cleanup_on_corrupt_root() {
    let t = Temp::new();
    let db = seed(&t);
    drop(db);
    let removed = [
        "input-123-0.tmp",
        "run-0000000000000001.build-123-456-0",
        "run-0000000000000001.build-123-456-0-records",
        "generation-0000000000000001.build-123-456-0-postings",
        "run-0000000000000001.build-123-456-0.spill-2",
        "run-0000000000000001.build-123-456-0.spill-stage-3",
    ];
    let preserved = [
        "input-12x-0.tmp",
        "input-1-2-3.tmp",
        "input-1-2.tmp.notes",
        "run-1.build-123-456-0",
        "run-0000000000000001.build-123-456-0.notes",
        "run-0000000000000001.build-123-456-0-records.notes",
        "notes.txt",
    ];
    for name in removed.into_iter().chain(preserved) {
        fs::write(t.0.join(name), b"scratch").unwrap();
    }
    let root = t.0.join("generation-0000000000000005.ready");
    let good = fs::read(&root).unwrap();
    fs::write(&root, b"corrupt").unwrap();
    assert!(Database::open(&t.0).is_err());
    for name in removed {
        assert!(t.0.join(name).exists());
    }
    fs::write(root, good).unwrap();
    let db = Database::open(&t.0).unwrap();
    for name in removed {
        assert!(!t.0.join(name).exists(), "{name}");
    }
    for name in preserved {
        assert_eq!(fs::read(t.0.join(name)).unwrap(), b"scratch", "{name}");
    }
    assert_eq!(db.stats().unwrap().documents, 2);
}

#[test]
#[cfg(feature = "bounded-engine")]
fn dirty_open_scavenges_before_recovery_flush_can_start_a_live_merge() {
    let t = Temp::new();
    let mut db = Database::open(&t.0).unwrap();
    let mut next = 0;
    for count in [1, 2, 4] {
        db.commit(
            (0..count)
                .map(|_| {
                    let mutation = put(&format!("{next:03}"));
                    next += 1;
                    mutation
                })
                .collect(),
        )
        .unwrap();
        db.flush().unwrap();
    }
    db.commit(vec![put("last")]).unwrap();
    drop(db);
    let scratch = t.0.join("run-0000000000000001.build-999-123-0-records");
    fs::write(&scratch, b"abandoned prior builder").unwrap();
    let mut db = Database::open_with_options(
        &t.0,
        crate::Options {
            max_delta_keys: 1,
            max_delta_bytes: 16 * 1024 * 1024,
        },
    )
    .unwrap();
    assert!(!scratch.exists());
    db.wait_for_compaction().unwrap();
    assert_eq!(db.stats().unwrap().documents, 8);
    assert!(db.get(&doc("last").key).unwrap().is_some());
    db.check_integrity().unwrap();
}
fn garbage(root: &Path) {
    for name in [
        "generation-0000000000000001.ready",
        "generation-0000000000000001.fdb",
        "generation-0000000000000001.idx",
        "generation-0000000000000003.ready",
        "generation-0000000000000003.fdb",
        "generation-0000000000000003.pending",
    ] {
        fs::write(root.join(name), b"obsolete").unwrap();
    }
}
#[test]
fn filename_allowlist_live_future_links_and_directories_are_preserved() {
    for invalid in [
        "LOCK",
        "run-1.fdb",
        "run-0000000000000000.fdb",
        "run-000000000000000A.fdb",
        "run-000000000000000a.ready",
        "generation-0000000000000001.fdb.notes",
        "generation-0000000000000001.build-1",
        "run-0000000000000001.payload-bench",
    ] {
        assert!(owned(invalid).is_none(), "{invalid}");
    }
    let t = Temp::new();
    let preserved = [
        "notes.txt",
        "run-0000000000000001.fdb.notes",
        "generation-000000000000000a.ready",
        "generation-000000000000000a.fdb",
        "run-000000000000000b.fdb",
        "run-0000000000000002.fdb",
        "run-0000000000000002.records04.idx",
    ];
    for name in preserved {
        fs::write(t.0.join(name), name).unwrap();
    }
    fs::create_dir(t.0.join("run-0000000000000003.fdb")).unwrap();
    let saved = snapshot(&t.0);
    // Symlink creation needs privileges on some Windows machines; Unix always
    // exercises the link exclusion, Windows also skips all reparse entries.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(t.0.join("notes.txt"), t.0.join("run-0000000000000004.fdb"))
            .unwrap();
    }
    for suffix in ["fdb", "idx", "records04.idx", "vfy", "svfy", "rmp"] {
        fs::write(
            t.0.join(format!("run-0000000000000001.{suffix}")),
            b"retired",
        )
        .unwrap();
    }
    let stats = collect(&t.0, 10, &BTreeSet::from([(Kind::Run, 2)])).unwrap();
    assert_eq!(stats.removed_files, 6);
    assert_eq!(stats.reclaimed_bytes, 42);
    for (p, b) in saved {
        assert_eq!(fs::read(p).unwrap(), b);
    }
    assert!(t.0.join("run-0000000000000003.fdb").is_dir());
    assert_eq!(
        collect(&t.0, 10, &BTreeSet::from([(Kind::Run, 2)])).unwrap(),
        ReclaimStats::default()
    );
}
#[test]
fn interruption_after_each_unlink_is_safe_and_retryable() {
    for stop in 1..=6 {
        let t = Temp::new();
        let mut db = seed(&t);
        drop(db);
        let saved = snapshot(&t.0);
        garbage(&t.0);
        let mut n = 0;
        assert!(
            collect_with(
                &t.0,
                5,
                &BTreeSet::from([(Kind::Legacy, 5), (Kind::Run, 2), (Kind::Run, 4)]),
                |phase, _| {
                    if phase == "marker" || phase == "file" {
                        n += 1;
                    }
                    if n == stop {
                        return Err(std::io::Error::other("interrupted cleanup").into());
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        db = Database::open(&t.0).unwrap();
        assert_eq!(db.get(&doc("a").key).unwrap().unwrap().document, doc("a"));
        db.check_integrity().unwrap();
        drop(db);
        assert_eq!(snapshot(&t.0), saved);
    }
}
#[test]
fn corrupt_latest_marker_or_source_prevents_any_reclamation() {
    for marker in [false, true] {
        let t = Temp::new();
        let db = seed(&t);
        drop(db);
        garbage(&t.0);
        let path = t.0.join(if marker {
            "generation-0000000000000005.ready"
        } else {
            "run-0000000000000002.fdb"
        });
        let good = fs::read(&path).unwrap();
        // Both a torn file and a single-bit change must fail before cleanup.
        for broken in [good[..good.len() - 1].to_vec(), {
            let mut b = good.clone();
            b[20] ^= 1;
            b
        }] {
            fs::write(&path, broken).unwrap();
            let before = snapshot(&t.0);
            assert!(Database::open(&t.0).is_err());
            assert_eq!(snapshot(&t.0), before);
        }
        fs::write(path, good).unwrap();
    }
}

#[test]
fn corrupt_active_wal_or_changed_root_rejects_explicit_reclaim_without_unlinks() {
    for change_root in [false, true] {
        let t = Temp::new();
        let mut db = seed(&t);
        garbage(&t.0);
        let path = t.0.join(if change_root {
            "generation-0000000000000005.ready"
        } else {
            "generation-0000000000000005.fdb"
        });
        let good = fs::read(&path).unwrap();
        let mut broken = good.clone();
        broken[20] ^= 1;
        fs::write(&path, broken).unwrap();
        let before = snapshot(&t.0);
        assert!(db.reclaim_obsolete().is_err());
        assert_eq!(snapshot(&t.0), before);
        fs::write(&path, &good).unwrap();
        assert_eq!(db.reclaim_obsolete().unwrap().removed_files, 6);
    }
}
#[test]
fn deferred_open_preserves_old_files_until_explicit_full_check() {
    let t = Temp::new();
    let db = seed(&t);
    drop(db);
    garbage(&t.0);
    let mut db = Database::open_deferred(&t.0).unwrap();
    assert!(t.0.join("generation-0000000000000001.fdb").exists());
    let stats = db.reclaim_obsolete().unwrap();
    assert_eq!(stats.removed_files, 6);
    assert_eq!(db.pending_source_checks().unwrap(), 0);
    assert_eq!(db.reclaim_obsolete().unwrap(), ReclaimStats::default());
}
#[test]
#[ignore = "subprocess helper for reclaim process kill"]
fn reclaim_child() {
    let path = std::env::var_os("FDB_TEST_RECLAIM_ROOT").unwrap();
    let mut db = Database::open_deferred(path).unwrap();
    db.set_revision_reclamation(false); // Isolate cleanup after normal publication.
    if std::env::var_os("FDB_TEST_RECLAIM_FLUSH").is_some() {
        db.commit(vec![put("c")]).unwrap();
        db.flush().unwrap();
    } else {
        db.reclaim_obsolete().unwrap();
    }
}
#[test]
fn kill_during_marker_and_data_phases_keeps_current_sources_byte_equal() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let t = Temp::new();
    let db = seed(&t);
    drop(db);
    let saved = snapshot(&t.0);
    for stage in ["marker", "markers_done", "file", "files_done"] {
        garbage(&t.0);
        let marker = t.0.join("kill-marker");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "reclaim::tests::reclaim_child", "--ignored"])
            .env("FDB_TEST_RECLAIM_ROOT", &t.0)
            .env("FDB_TEST_RECLAIM_STAGE", stage)
            .env("FDB_TEST_RECLAIM_MARKER", &marker)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !marker.exists() && start.elapsed() < Duration::from_secs(20) {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let reached = marker.exists();
        let _ = child.kill();
        child.wait().unwrap();
        assert!(reached, "{stage}");
        fs::remove_file(marker).unwrap();
        let db = Database::open(&t.0).unwrap();
        db.check_integrity().unwrap();
        assert_eq!(db.stats().unwrap().documents, 2);
        drop(db);
        assert_eq!(snapshot(&t.0), saved);
    }
}

#[test]
fn automatic_reclaim_kill_after_publication_preserves_newly_published_write() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    for stage in ["marker", "markers_done", "file", "files_done"] {
        let t = Temp::new();
        let db = seed(&t);
        drop(db);
        let marker = t.0.join("kill-marker");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "reclaim::tests::reclaim_child", "--ignored"])
            .env("FDB_TEST_RECLAIM_ROOT", &t.0)
            .env("FDB_TEST_RECLAIM_STAGE", stage)
            .env("FDB_TEST_RECLAIM_MARKER", &marker)
            .env("FDB_TEST_RECLAIM_FLUSH", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !marker.exists() && start.elapsed() < Duration::from_secs(20) {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let reached = marker.exists();
        let _ = child.kill();
        child.wait().unwrap();
        assert!(reached, "{stage}");
        // The cleanup hook is reached only after the new root was published.
        let generation = manifest::inventory(&t.0).unwrap().0.unwrap();
        assert_eq!(generation, 7);
        let manifest::Publication::Current(m) = manifest::read(&t.0, generation).unwrap() else {
            panic!("native publication")
        };
        let mut current: Vec<_> = m.runs.iter().map(|s| s.path(&t.0, "fdb")).collect();
        current.push(manifest::path(&t.0, m.active.id, "fdb"));
        current.push(manifest::path(&t.0, generation, "ready"));
        let saved: Vec<_> = current
            .into_iter()
            .map(|p| {
                let b = fs::read(&p).unwrap();
                (p, b)
            })
            .collect();
        let db = Database::open(&t.0).unwrap();
        assert_eq!(db.stats().unwrap().documents, 3);
        assert_eq!(db.get(&doc("c").key).unwrap().unwrap().version, 3);
        db.check_integrity().unwrap();
        for (path, bytes) in saved {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
    }
}
