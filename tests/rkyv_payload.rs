#![cfg(feature = "experimental-rkyv-payload")]
use siftwake::{
    Document, Key, StoredDocument,
    rkyv_payload::{
        self as payload,
        bench::{Codec, Reader, Writer},
    },
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fdb-rkyv-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn stored(id: u32) -> StoredDocument {
    StoredDocument {
        document: Document {
            key: Key::new("shared", "texts", &format!("{id:010}")),
            title: "İSTANBUL 東京".into(),
            text: "Şirket bilançosu \"🦀\"\n içerik".into(),
            fields: BTreeMap::from([(
                "nested".into(),
                serde_json::json!({"list":[null,true,-0.0,u64::MAX,"😀"]}),
            )]),
            links: vec![Key::new("shared", "texts", "other")],
        },
        version: id as u64 / 500 + 1,
    }
}
#[test]
fn every_export_cut_byte_corruption_and_runtime_payload_error_fail_closed() {
    for codec in [Codec::Json, Codec::Rkyv, Codec::Binary] {
        let temp = Temp::new();
        let path = temp.0.join("scratch.payload-bench");
        let mut writer = Writer::create(&path, codec, 2).unwrap();
        writer.append(&stored(0)).unwrap();
        writer.append(&stored(1)).unwrap();
        writer.finish().unwrap();
        let complete = fs::read(&path).unwrap();
        for cut in 0..complete.len() {
            fs::write(&path, &complete[..cut]).unwrap();
            assert!(Reader::open(&path, codec).is_err(), "{codec:?} cut={cut}");
        }
        for offset in 0..complete.len() {
            let mut bytes = complete.clone();
            bytes[offset] ^= 1;
            fs::write(&path, bytes).unwrap();
            assert!(
                Reader::open(&path, codec).is_err(),
                "{codec:?} offset={offset}"
            );
        }
        fs::write(&path, &complete).unwrap();
        let mut reader = Reader::open(&path, codec).unwrap();
        // Mutating a selected payload after successful open is also rejected.
        let checksum = reader.checksum(1).unwrap();
        let bytes = reader.bytes(1).unwrap();
        let mut bad = bytes.to_vec();
        bad[0] ^= 1;
        assert!(payload::bench::verify_crc(&bad, checksum).is_err());
        assert_eq!(reader.len(), 2);
    }
}
#[test]
fn archive_cuts_misalignment_and_turkish_contains_keep_safe_access() {
    let bytes = payload::encode(&stored(0)).unwrap();
    for cut in 0..bytes.len() {
        assert!(payload::checked(&bytes[..cut]).is_err(), "cut={cut}");
    }
    let mut shifted = rkyv::util::AlignedVec::<16>::new();
    shifted.push(0);
    shifted.extend_from_slice(&bytes);
    assert!(payload::checked(&shifted[1..]).is_err());
    let archived = payload::checked(&bytes).unwrap();
    for query in ["istanbul", "şirket bilanço", "İÇ", "🦀"] {
        assert!(payload::text_matches(
            archived.title(),
            archived.text(),
            &payload::query_terms(query)
        ));
    }
    assert!(!payload::text_matches(
        archived.title(),
        archived.text(),
        &payload::query_terms("olmayan")
    ));
    assert_eq!(payload::decode(&bytes).unwrap(), stored(0));
}
#[test]
fn old_json_fixture_still_opens_with_rkyv_feature_enabled() {
    let temp = Temp::new();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1");
    for name in ["checkpoint.fdb", "checkpoint.ready"] {
        let extension = name.rsplit('.').next().unwrap();
        fs::copy(
            fixture.join(name),
            temp.0
                .join(format!("generation-0000000000000002.{extension}")),
        )
        .unwrap();
    }
    let db = siftwake::Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 4);
    let mut query = siftwake::Query::new("shared");
    query.text = "TEKNOLOJİ yatırımları".into();
    assert_eq!(db.search(&query).unwrap().total, 2);
    assert_eq!(
        fs::read(temp.0.join("generation-0000000000000002.fdb")).unwrap(),
        fs::read(fixture.join("checkpoint.fdb")).unwrap()
    );
}
#[test]
#[ignore = "child helper invoked by export_process_kill"]
fn export_child() {
    use std::io::Write;
    let Some(path) = std::env::var_os("FDB_RKYV_TEST_EXPORT") else {
        return;
    };
    let codec = match std::env::var("FDB_RKYV_TEST_CODEC").unwrap().as_str() {
        "json" => Codec::Json,
        "rkyv" => Codec::Rkyv,
        "binary" => Codec::Binary,
        _ => panic!("unknown test codec"),
    };
    let count: u32 = std::env::var("FDB_RKYV_TEST_COUNT")
        .unwrap()
        .parse()
        .unwrap();
    let mut writer = Writer::create(std::path::Path::new(&path), codec, count).unwrap();
    writer.append(&stored(0)).unwrap();
    writer.sync().unwrap();
    println!("FDB_EXPORT_READY");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn binary_roundtrip_borrowed_views_and_malformed_offsets() {
    use payload::binary;
    let mut doc = stored(0);
    doc.document.fields.insert(
        "numbers".into(),
        serde_json::json!([i64::MIN, i64::MAX, u64::MAX, -0.0, 1.25, [], {}, ""]),
    );
    let bytes = binary::encode(&doc).unwrap();
    let view = binary::checked(&bytes).unwrap();
    assert_eq!(view.to_document().unwrap(), doc);
    assert_eq!(
        serde_json::to_vec(&view.to_document().unwrap()).unwrap(),
        serde_json::to_vec(&doc).unwrap()
    );
    for query in ["istanbul", "şirket bilanço", "İÇ", "🦀"] {
        assert!(payload::text_matches(
            view.title(),
            view.text(),
            &payload::query_terms(query)
        ));
    }
    assert_eq!(view.key_parts(), ("shared", "texts", "0000000000"));
    let start = bytes.as_ptr() as usize;
    assert!((start..start + bytes.len()).contains(&(view.text().as_ptr() as usize)));
    for cut in 0..bytes.len() {
        assert!(binary::checked(&bytes[..cut]).is_err());
    }
    for offset in (12..44).step_by(4) {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(binary::checked(&bad).is_err());
    }
    let mut bad = bytes.clone();
    bad[44] = 0xff;
    assert!(binary::checked(&bad).is_err());
    let field_start = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
    let mut bad = bytes.clone();
    bad[field_start..field_start + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(binary::checked(&bad).is_err());
    // Metadata must be checked even when only text is requested.
    let mut empty = stored(0);
    empty.document.fields = BTreeMap::from([("x".into(), serde_json::json!(1.25))]);
    let mut bad = binary::encode(&empty).unwrap();
    let at = u32::from_le_bytes(bad[32..36].try_into().unwrap()) as usize + 4 + 4 + 1;
    bad[at] = 255;
    assert!(binary::checked(&bad).is_err());
    bad[at] = 5;
    bad[at + 1..at + 9].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(binary::checked(&bad).is_err());
    let mut deep = serde_json::Value::Null;
    for _ in 0..130 {
        deep = serde_json::json!([deep]);
    }
    empty.document.fields.insert("deep".into(), deep);
    assert!(binary::encode(&empty).is_err());
}

#[cfg(not(feature = "experimental-paged-metadata"))]
fn overlay_database(root: &std::path::Path) -> Vec<StoredDocument> {
    let docs: Vec<_> = (0..4)
        .map(|id| {
            let mut d = stored(id);
            d.document
                .fields
                .insert("marker".into(), serde_json::json!(true));
            d
        })
        .collect();
    let mut db = siftwake::Database::open(root).unwrap();
    db.commit(
        docs.iter()
            .map(|d| siftwake::Mutation::Put {
                document: d.document.clone(),
                guard: siftwake::Guard::Any,
            })
            .collect(),
    )
    .unwrap();
    db.flush().unwrap();
    docs
}

#[cfg(not(feature = "experimental-paged-metadata"))]
#[test]
fn real_engine_overlay_pages_filters_and_mismatched_cache_fail_closed() {
    for codec in [Codec::Json, Codec::Binary] {
        let temp = Temp::new();
        let root = temp.0.join("db");
        let docs = overlay_database(&root);
        let good = temp.0.join("good.payload-bench");
        let bad = temp.0.join("bad.payload-bench");
        for (path, wrong) in [(&good, false), (&bad, true)] {
            let mut writer = Writer::create(path, codec, 4).unwrap();
            for d in &docs {
                let mut d = d.clone();
                if wrong {
                    d.document
                        .text
                        .push_str(" changed despite valid CRC and identity");
                }
                writer.append(&d).unwrap();
            }
            writer.finish().unwrap();
        }
        let mut db = siftwake::Database::open(&root).unwrap();
        assert!(db.attach_payload_benchmark(&bad, codec).is_err());
        assert_eq!(db.get(&docs[0].document.key).unwrap().unwrap(), docs[0]);
        db.attach_payload_benchmark(&good, codec).unwrap();
        assert!(db.attach_payload_benchmark(&good, codec).is_err());
        let mut q = siftwake::Query::new("shared");
        q.collection = Some("texts".into());
        q.text = "istanbul şirket 🦀".into();
        q.limit = 2;
        q.offset = 2;
        let page = db.search(&q).unwrap();
        assert_eq!(page.total, 4);
        assert_eq!(page.hits, docs[2..]);
        q.exact.insert("marker".into(), serde_json::json!(true));
        assert_eq!(db.search(&q).unwrap().hits, docs[2..]);
        q.text.clear();
        assert_eq!(db.search(&q).unwrap().hits, docs[2..]);
        q.scope = "user:unrelated".into();
        assert_eq!(db.search(&q).unwrap().total, 0);
        // Alter an actually selected payload after attachment, not merely a copy.
        use std::io::{Seek, SeekFrom, Write};
        let mut file = fs::OpenOptions::new().write(true).open(&good).unwrap();
        file.seek(SeekFrom::Start(36)).unwrap();
        file.write_all(&[0xff]).unwrap();
        file.sync_all().unwrap();
        assert!(db.get(&docs[0].document.key).is_err());
        drop(db);
        // Overlay is never discovered automatically on normal reopen.
        assert_eq!(
            siftwake::Database::open(&root)
                .unwrap()
                .get(&docs[0].document.key)
                .unwrap()
                .unwrap(),
            docs[0]
        );
        let source = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "fdb") && p.with_extension("idx").exists())
            .unwrap();
        let mut bytes = fs::read(&source).unwrap();
        let at = bytes.len() - 1;
        bytes[at] ^= 1;
        fs::write(source, bytes).unwrap();
        assert!(siftwake::Database::open(&root).is_err());
    }
}

#[cfg(not(feature = "experimental-paged-metadata"))]
#[test]
#[ignore = "child helper invoked by overlay_engine_process_kill"]
fn overlay_engine_child() {
    use std::io::Write;
    let root = std::env::var_os("FDB_OVERLAY_TEST_DB").unwrap();
    let path = std::env::var_os("FDB_OVERLAY_TEST_PAYLOAD").unwrap();
    let mut db = siftwake::Database::open(root).unwrap();
    db.attach_payload_benchmark(path, Codec::Binary).unwrap();
    let mut q = siftwake::Query::new("shared");
    q.text = "şirket".into();
    assert_eq!(db.search(&q).unwrap().total, 4);
    println!("FDB_OVERLAY_READY");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

#[cfg(not(feature = "experimental-paged-metadata"))]
#[test]
fn overlay_engine_process_kill() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    let temp = Temp::new();
    let root = temp.0.join("db");
    let docs = overlay_database(&root);
    let path = temp.0.join("binary.payload-bench");
    let mut writer = Writer::create(&path, Codec::Binary, 4).unwrap();
    for d in &docs {
        writer.append(d).unwrap();
    }
    writer.finish().unwrap();
    let before: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e == "fdb" || e == "ready" || e == "idx")
        })
        .map(|p| {
            let b = fs::read(&p).unwrap();
            (p, b)
        })
        .collect();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "overlay_engine_child",
            "--ignored",
            "--nocapture",
        ])
        .env("FDB_OVERLAY_TEST_DB", &root)
        .env("FDB_OVERLAY_TEST_PAYLOAD", &path)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        if output.read_line(&mut line).unwrap() == 0 {
            let _ = child.wait();
            panic!("child exited before query");
        }
        if line.trim() == "FDB_OVERLAY_READY" {
            break;
        }
    }
    child.kill().unwrap();
    child.wait().unwrap();
    for (p, b) in before {
        assert_eq!(fs::read(p).unwrap(), b);
    }
    let db = siftwake::Database::open(&root).unwrap();
    assert_eq!(db.get(&docs[0].document.key).unwrap().unwrap(), docs[0]);
}
#[test]
fn export_process_kill() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    for codec in [Codec::Json, Codec::Rkyv, Codec::Binary] {
        for count in [1, 2] {
            let temp = Temp::new();
            let path = temp.0.join("killed.payload-bench");
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "export_child", "--ignored", "--nocapture"])
                .env("FDB_RKYV_TEST_EXPORT", &path)
                .env("FDB_RKYV_TEST_CODEC", codec.name())
                .env("FDB_RKYV_TEST_COUNT", count.to_string())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let mut output = BufReader::new(child.stdout.take().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                if output.read_line(&mut line).unwrap() == 0 {
                    let _ = child.wait();
                    panic!("child exited before sync");
                }
                if line.trim() == "FDB_EXPORT_READY" {
                    break;
                }
            }
            child.kill().unwrap();
            child.wait().unwrap();
            let result = Reader::open(&path, codec);
            if count == 1 {
                assert_eq!(result.unwrap().len(), 1);
            } else {
                assert!(result.is_err());
            }
        }
    }
}
