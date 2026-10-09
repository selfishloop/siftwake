#![cfg(feature = "experimental-wal-delta")]
use fdb::{Database, Document, Error, Guard, Key, Mutation, Query};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fdb-wal-delta-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn log(&self) -> PathBuf {
        fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "fdb"))
            .max()
            .unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if self.0.parent() == Some(std::env::temp_dir().as_path())
            && self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("fdb-wal-delta-")
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn document(id: &str) -> Document {
    Document {
        key: Key::new("shared", "documents", id),
        title: "ŞİRKET 🦀".into(),
        text: "teknoloji \"ü\"\\\n".into(),
        fields: [(
            "nested".into(),
            serde_json::json!({"large": [1,2,"\"ş\\",null]}),
        )]
        .into(),
        links: vec![Key::new("shared", "documents", "later")],
    }
}
fn put(document: Document) -> Mutation {
    Mutation::Put {
        document,
        guard: Guard::Any,
    }
}

#[test]
fn delta_references_preserve_documents_updates_search_recovery_and_flush() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let mut expected = document("1");
    db.commit(vec![put(expected.clone()), put(document("2"))])
        .unwrap();
    assert_eq!(db.stats().unwrap().resident_documents, 0);
    for version in 2..12 {
        expected
            .fields
            .insert("version".into(), serde_json::json!(version));
        db.commit(vec![Mutation::Put {
            document: expected.clone(),
            guard: Guard::Version {
                version: version - 1,
            },
        }])
        .unwrap();
        let got = db.get(&expected.key).unwrap().unwrap();
        assert_eq!(got.document, expected);
        assert_eq!(got.version, version);
    }
    db.commit(vec![Mutation::Delete {
        key: document("2").key,
        guard: Guard::Version { version: 1 },
    }])
    .unwrap();
    assert_eq!(db.stats().unwrap().delta_keys, 2);
    drop(db);
    let mut db = Database::open(&tmp.0).unwrap();
    assert_eq!(db.stats().unwrap().resident_documents, 0);
    assert_eq!(db.get(&expected.key).unwrap().unwrap().document, expected);
    assert!(db.get(&document("2").key).unwrap().is_none());
    let mut q = Query::new("shared");
    q.text = "teknoloji".into();
    let page = db.search(&q).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.hits[0].document, expected);
    db.flush().unwrap();
    drop(db);
    let db = Database::open(&tmp.0).unwrap();
    assert_eq!(db.get(&expected.key).unwrap().unwrap().document, expected);
}

#[test]
fn runtime_delta_corruption_is_not_hidden_by_resident_index_or_cached_result() {
    let tmp = Temp::new();
    let mut db = Database::open(&tmp.0).unwrap();
    let doc = document("selected");
    db.commit(vec![put(doc.clone())]).unwrap();
    assert_eq!(db.get(&doc.key).unwrap().unwrap().document, doc);
    let path = tmp.log();
    let mut bytes = fs::read(&path).unwrap();
    let marker = "teknoloji".as_bytes();
    let at = bytes
        .windows(marker.len())
        .rposition(|x| x == marker)
        .unwrap();
    bytes[at] = b'X';
    let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert!(matches!(db.get(&doc.key), Err(Error::Corrupt(_))));
    let mut q = Query::new("shared");
    q.text = "teknoloji".into();
    assert!(matches!(db.search(&q), Err(Error::Corrupt(_))));
    assert!(matches!(db.flush(), Err(Error::Corrupt(_))));
    drop(db);
    assert!(matches!(Database::open(&tmp.0), Err(Error::Corrupt(_))));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
