//! Isolated SQLite comparator for phase-separated memory baselines.
#[path = "../../../bench_support/memory_corpus.rs"]
mod input;
use rusqlite::{Connection, OpenFlags, params};
use serde_json::json;
use siftwake::StoredDocument;
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            match c {
                'İ' | 'I' | 'ı' => 'i',
                'Ş' | 'ş' => 's',
                'Ğ' | 'ğ' => 'g',
                'Ç' | 'ç' => 'c',
                'Ö' | 'ö' => 'o',
                'Ü' | 'ü' => 'u',
                other => other,
            }
            .to_lowercase()
        })
        .collect()
}
fn main() -> input::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("idle|ingest|verify <db> <from> <to>".into());
    }
    let from: usize = args[2].parse()?;
    let to: usize = args[3].parse()?;
    if to != 10000 || from != 0 {
        return Err("invalid range".into());
    }
    let idle = args[0] == "idle";
    let verify = args[0] == "verify";
    let read_only = idle || verify;
    if !read_only && args[0] != "ingest" {
        return Err("unknown mode".into());
    }
    let root = Path::new(&args[1]);
    if from == 0 && !read_only {
        std::fs::create_dir(root)?;
    }
    input::phase("runtime_baseline");
    thread::sleep(Duration::from_secs(1));
    input::phase("opening");
    let start = Instant::now();
    let mut db = Connection::open_with_flags(
        root.join("sqlite.db"),
        if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        },
    )?;
    db.execute_batch("PRAGMA cache_size=-2048; PRAGMA mmap_size=0; PRAGMA temp_store=FILE;")?;
    if !read_only {
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
    }
    let open_ms = start.elapsed().as_secs_f64() * 1000.;
    if from == 0 && !read_only {
        db.execute_batch("CREATE TABLE docs(scope TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,folded TEXT NOT NULL,UNIQUE(scope,collection,id));
        CREATE VIRTUAL TABLE search USING fts5(folded,content='docs',content_rowid='rowid',tokenize='trigram',detail=none,columnsize=0);
        CREATE TRIGGER docs_ai AFTER INSERT ON docs BEGIN INSERT INTO search(rowid,folded) VALUES(new.rowid,new.folded); END;")?;
    }
    let count =
        usize::try_from(db.query_row("SELECT count(*) FROM docs", [], |r| r.get::<_, i64>(0))?)?;
    assert_eq!(count, if read_only { to } else { from });
    println!(
        "{}",
        json!({"phase":"opened","open_ms":open_ms,"documents":count,"sqlite_version":rusqlite::version(),"cache_kib":2048,"mmap_bytes":0,"full_integrity_on_open":false})
    );
    if idle {
        input::phase("idle_wait");
        thread::sleep(Duration::from_secs(5));
        input::phase("idle_ready");
        thread::sleep(Duration::from_secs(2));
        return Ok(());
    }
    if verify {
        let mut source = input::Input::open()?;
        let mut statement =
            db.prepare("SELECT payload FROM docs WHERE scope=? AND collection=? AND id=?")?;
        for _ in 0..to {
            let stored = source.read()?;
            let payload: String = statement.query_row(
                params![
                    stored.document.key.scope,
                    stored.document.key.collection,
                    stored.document.key.id
                ],
                |row| row.get(0),
            )?;
            assert_eq!(serde_json::from_str::<StoredDocument>(&payload)?, stored);
        }
        println!(
            "{}",
            json!({"phase":"verified","full_document_checks":to,"input_crc_verified":true,"outside_timing":true,"read_only":true})
        );
        return Ok(());
    }
    let mut source = input::Input::open()?;
    source.skip(from)?;
    input::phase("indexing");
    let start = Instant::now();
    for first in (from..to).step_by(500) {
        let tx = db.transaction()?;
        let mut expected = Vec::new();
        {
            let mut insert = tx.prepare("INSERT INTO docs VALUES(?,?,?,?,?)")?;
            for id in first..first + 500 {
                let stored = source.read()?;
                let payload = serde_json::to_string(&stored)?;
                let folded = fold(&format!(
                    "{}\n{}",
                    stored.document.title, stored.document.text
                ));
                insert.execute(params![
                    stored.document.key.scope,
                    stored.document.key.collection,
                    stored.document.key.id,
                    payload,
                    folded
                ])?;
                if id == first || id == first + 499 {
                    expected.push(stored);
                }
            }
        }
        tx.commit()?;
        for stored in expected {
            let payload: String = db.query_row(
                "SELECT payload FROM docs WHERE scope=? AND collection=? AND id=?",
                params![
                    stored.document.key.scope,
                    stored.document.key.collection,
                    stored.document.key.id
                ],
                |r| r.get(0),
            )?;
            assert_eq!(serde_json::from_str::<StoredDocument>(&payload)?, stored);
        }
        if (first + 500).is_multiple_of(10000) {
            println!(
                "{}",
                json!({"phase":"indexing","records":first+500,"elapsed_s":start.elapsed().as_secs_f64()})
            );
        }
    }
    let ingest_ms = start.elapsed().as_secs_f64() * 1000.;
    input::phase("maintenance");
    let start = Instant::now();
    db.execute("INSERT INTO search(search) VALUES('optimize')", [])?;
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    let maintenance_ms = start.elapsed().as_secs_f64() * 1000.;
    let count =
        usize::try_from(db.query_row("SELECT count(*) FROM docs", [], |r| r.get::<_, i64>(0))?)?;
    assert_eq!(count, to);
    println!(
        "{}",
        json!({"phase":"result","from":from,"to":to,"ingest_ms":ingest_ms,"maintenance_ms":maintenance_ms,"text_bytes":source.text_bytes,"documents":count,"sampled_full_document_checks":(to-from)/250,"all_input_crc_verified":true})
    );
    Ok(())
}
