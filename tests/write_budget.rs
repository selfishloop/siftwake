#![cfg(feature = "experimental-write-buffer-budget")]
use fdb::{Database, Document, Guard, Key, Mutation};
#[test]
fn concurrent_databases_stream_sources_and_release_shared_buffer_reservations() {
    let root = std::env::temp_dir().join(format!(
        "fdb-shared-writers-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|worker| {
            let path = root.join(worker.to_string());
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let body = "şirket teknoloji ".repeat(500);
                let mut db = Database::open(&path).unwrap();
                let changes = (0..128)
                    .map(|id| Mutation::Put {
                        document: Document {
                            key: Key::new("test", "files", &format!("{id:05}")),
                            title: format!("rapor {id}"),
                            text: body.clone(),
                            fields: Default::default(),
                            links: vec![],
                        },
                        guard: Guard::Missing,
                    })
                    .collect();
                db.commit(changes).unwrap();
                barrier.wait();
                db.flush().unwrap();
                drop(db);
                let db = Database::open(&path).unwrap();
                assert_eq!(db.stats().unwrap().documents, 128);
                for id in [0, 64, 127] {
                    let stored = db
                        .get(&Key::new("test", "files", &format!("{id:05}")))
                        .unwrap()
                        .unwrap();
                    assert_eq!(stored.document.text, body);
                    assert_eq!(stored.version, 1);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let budget = fdb::write_budget::snapshot();
    assert_eq!(budget.in_use_bytes, 0);
    assert!(budget.peak_reserved_bytes <= budget.limit_bytes);
    #[cfg(feature = "experimental-builder-budget")]
    {
        let postings = fdb::write_budget::posting_snapshot();
        assert_eq!(postings.in_use_bytes, 0);
        assert!(postings.peak_reserved_bytes <= postings.limit_bytes);
        assert_eq!(postings.limit_bytes, 2 * 1024 * 1024);
    }
    // Every path is created beneath this unique test directory.
    let resolved = root.canonicalize().unwrap();
    let temp = std::env::temp_dir().canonicalize().unwrap();
    assert!(resolved.starts_with(&temp));
    assert_eq!(resolved.file_name(), root.file_name());
    std::fs::remove_dir_all(resolved).unwrap();
}
