//! Isolated long-passage SQLite comparator. Never part of the FDB runtime.
#[path = "../../../bench_support/passages.rs"]
mod passages;
use passages::{Corpus, Request, document};
use rusqlite::{Connection, OpenFlags, params};
use serde_json::json;
use siftwake::StoredDocument;
use std::{fs, io::Read, path::Path, time::Instant};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SCHEMA: &str = "CREATE TABLE meta(sequence INTEGER NOT NULL);
INSERT INTO meta VALUES(200);
CREATE TABLE docs(scope TEXT NOT NULL, collection TEXT NOT NULL, id TEXT NOT NULL,
payload TEXT NOT NULL, folded TEXT NOT NULL, UNIQUE(scope,collection,id));
CREATE VIRTUAL TABLE search USING fts5(folded,content='docs',content_rowid='rowid',
tokenize='trigram',detail=none,columnsize=0);
CREATE TRIGGER docs_ai AFTER INSERT ON docs BEGIN
INSERT INTO search(rowid,folded) VALUES(new.rowid,new.folded); END;";
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            match c {
                'İ' | 'ı' => 'i',
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
fn connect(path: &Path, write: bool) -> Result<Connection> {
    let flags = if write {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let db = Connection::open_with_flags(path, flags)?;
    if write {
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
    }
    db.execute_batch("PRAGMA cache_size=-2048; PRAGMA mmap_size=0; PRAGMA temp_store=FILE;")?;
    Ok(db)
}
fn sql(text: &str, offset: usize) -> Result<(String, String, Vec<String>)> {
    let terms: Vec<_> = text.split_whitespace().map(fold).collect();
    if terms.is_empty() || terms.iter().any(|s| s.contains(['%', '_', '\\'])) {
        return Err(
            "Only nonempty literal benchmark terms without LIKE wildcards supported".into(),
        );
    }
    let conditions = vec!["search.folded LIKE ?"; terms.len()].join(" AND ");
    let from = format!(
        "search CROSS JOIN docs d ON d.rowid=search.rowid WHERE d.scope='shared' AND d.collection='texts' AND {conditions}"
    );
    let count = format!("SELECT count(*) FROM {from}");
    // Sort keys/rowids only; hydrate exactly the same 50 complete payloads as FDB.
    let page = format!(
        "WITH selected AS MATERIALIZED (SELECT d.rowid AS rid,d.id AS id FROM {from} ORDER BY d.id LIMIT 50 OFFSET {offset}) SELECT d.payload FROM selected s JOIN docs d ON d.rowid=s.rid ORDER BY s.id"
    );
    Ok((
        count,
        page,
        terms.into_iter().map(|s| format!("%{s}%")).collect(),
    ))
}
fn search(
    db: &mut Connection,
    count: &str,
    page: &str,
    values: &[String],
) -> Result<(usize, Vec<StoredDocument>, u64)> {
    let tx = db.transaction()?;
    let total = usize::try_from(
        tx.query_row(count, rusqlite::params_from_iter(values), |r| {
            r.get::<_, i64>(0)
        })?,
    )?;
    let sequence =
        u64::try_from(tx.query_row("SELECT sequence FROM meta", [], |r| r.get::<_, i64>(0))?)?;
    let mut hits = Vec::new();
    {
        let mut stmt = tx.prepare(page)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(values))?;
        while let Some(row) = rows.next()? {
            let payload: String = row.get(0)?;
            hits.push(serde_json::from_str(&payload)?);
        }
    }
    tx.commit()?;
    Ok((total, hits, sequence))
}
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .ok_or("prepare|query|integrity <SQLite directory> <FDB oracle directory>")?;
    let root = args.next().ok_or("SQLite directory missing")?;
    let oracle = args.next().ok_or("FDB oracle directory missing")?;
    if args.next().is_some() {
        return Err("extra arguments".into());
    }
    let path = Path::new(&root).join("sqlite.db");
    if mode == "integrity" {
        let db = connect(&path, true)?;
        let start = Instant::now();
        let check: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        assert_eq!(check, "ok");
        db.execute(
            "INSERT INTO search(search,rank) VALUES('integrity-check',1)",
            [],
        )?;
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        println!(
            "{}",
            json!({"phase":"integrity","ok":true,"elapsed_ms":start.elapsed().as_secs_f64()*1000.})
        );
        return Ok(());
    }
    let start = Instant::now();
    if mode == "prepare" {
        fs::create_dir(&root)?;
    } else if mode != "query" {
        return Err("unknown mode".into());
    }
    let mut db = connect(&path, mode == "prepare")?;
    let open_ms = start.elapsed().as_secs_f64() * 1000.;
    if mode == "query" {
        println!(
            "{}",
            json!({"phase":"open","open_ms":open_ms,"api":"SQLite transaction count + key page + full JSON decode","sqlite_version":rusqlite::version(),"full_integrity_on_open":false})
        );
    }
    let corpus_root = Path::new("data/rkyv-corpus-20261009");
    let corpus: Corpus = serde_json::from_slice(&fs::read(corpus_root.join("corpus.json"))?)?;
    let books = corpus
        .sources
        .iter()
        .map(|s| fs::read(corpus_root.join(format!("body{}.txt", s.book_id))))
        .collect::<std::io::Result<Vec<_>>>()?;
    assert_eq!(corpus.rows.len(), 100000);
    if mode == "prepare" {
        let start = Instant::now();
        let mut canonical = fs::File::open(corpus_root.join("json.payload-bench"))?;
        let mut header = [0u8; 20];
        canonical.read_exact(&mut header)?;
        assert_eq!(&header[..8], b"PLOADB01");
        assert_eq!(u32::from_le_bytes(header[12..16].try_into()?), 100000);
        db.execute_batch(SCHEMA)?;
        for first in (0..100000).step_by(500) {
            let tx = db.transaction()?;
            {
                let mut insert = tx.prepare("INSERT INTO docs VALUES('shared','texts',?,?,?)")?;
                for id in first..first + 500 {
                    let stored = document(&corpus, &books, id);
                    let payload = serde_json::to_string(&stored)?;
                    let mut record = [0u8; 16];
                    canonical.read_exact(&mut record)?;
                    assert_eq!(u32::from_le_bytes(record[8..12].try_into()?) as usize, id);
                    let mut original =
                        vec![0; u32::from_le_bytes(record[..4].try_into()?) as usize];
                    canonical.read_exact(&mut original)?;
                    assert_eq!(payload.as_bytes(), original, "corpus payload id={id}");
                    let folded = fold(&format!(
                        "{}\n{}",
                        stored.document.title, stored.document.text
                    ));
                    insert.execute(params![stored.document.key.id, payload, folded])?;
                }
            }
            tx.commit()?;
            if (first + 500) % 10000 == 0 {
                println!(
                    "{}",
                    json!({"phase":"prepare_progress","records":first+500,"elapsed_s":start.elapsed().as_secs_f64()})
                );
            }
        }
        assert_eq!(canonical.read(&mut [0u8; 1])?, 0);
        let ingest_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        db.execute("INSERT INTO search(search) VALUES('optimize')", [])?;
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        println!(
            "{}",
            json!({"phase":"prepare_result","ingest_ms":ingest_ms,"maintenance_ms":start.elapsed().as_secs_f64()*1000.,"documents":100000,"sqlite_version":rusqlite::version(),"query_only_comparison":true})
        );
        return Ok(());
    }
    let ids: Vec<_> = (0..100).map(|n| (n * 7919 + 137) % 100000).collect();
    let expected: Vec<_> = ids
        .iter()
        .map(|&id| document(&corpus, &books, id))
        .collect();
    let mut times = Vec::new();
    for repeat in 0..4 {
        let start = Instant::now();
        let mut found = Vec::new();
        for item in &expected {
            let key = &item.document.key;
            let payload: String = db.query_row(
                "SELECT payload FROM docs WHERE scope=? AND collection=? AND id=?",
                params![key.scope, key.collection, key.id],
                |r| r.get(0),
            )?;
            found.push(serde_json::from_str::<StoredDocument>(&payload)?);
        }
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(found, expected);
        if repeat != 0 {
            times.push(elapsed);
        }
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{}",
        json!({"phase":"get","requests":100,"samples_ms":times,"p50_ms":times[times.len()/2],"full_document_oracle_verified":true,"cache":"one untimed 100-get warmup; no forced OS eviction"})
    );
    let requests: Vec<Request> =
        serde_json::from_slice(&fs::read(Path::new(&oracle).join("oracle.json"))?)?;
    for request in requests {
        let mut offsets = vec![0, 50];
        if request.name == "common" {
            offsets.push(request.ids.len() - 50);
        }
        for (page_index, offset) in offsets.into_iter().enumerate() {
            let expected: Vec<_> = request
                .ids
                .iter()
                .skip(offset)
                .take(50)
                .map(|&id| document(&corpus, &books, id))
                .collect();
            let (count, page, values) = sql(&request.text, offset)?;
            let mut plan = Vec::new();
            {
                let mut stmt = db.prepare(&format!("EXPLAIN QUERY PLAN {page}"))?;
                let rows = stmt.query_map(rusqlite::params_from_iter(&values), |r| {
                    r.get::<_, String>(3)
                })?;
                for row in rows {
                    plan.push(row?);
                }
            }
            let repeats = if request.name == "rare" { 10 } else { 1 };
            let mut times = Vec::new();
            for repeat in 0..repeats + usize::from(page_index == 0) {
                let start = Instant::now();
                let (total, hits, sequence) = search(&mut db, &count, &page, &values)?;
                let elapsed = start.elapsed().as_secs_f64() * 1000.;
                assert_eq!(total, request.ids.len());
                assert_eq!(hits, expected);
                assert_eq!(sequence, 200);
                if page_index != 0 || repeat != 0 {
                    times.push(elapsed);
                }
            }
            times.sort_by(f64::total_cmp);
            println!(
                "{}",
                json!({"phase":"query","query":request.name,"text":request.text,"offset":offset,"total":request.ids.len(),"p50_ms":times[times.len()/2],"samples_ms":times,"full_page_oracle_verified":true,"profiled":false,"api":"SQLite transaction count + key page + full JSON decode","plan":plan,"cache":"warm protocol: one untimed first page; subsequent pages follow; no forced eviction"})
            );
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trigram_and_short_queries_keep_literal_and_page_semantics() -> Result<()> {
        let mut db = Connection::open_in_memory()?;
        db.execute_batch(SCHEMA)?;
        for (id, title, body) in [
            (0, "İŞ", "Whale of sea"),
            (1, "other", "of the sea"),
            (2, "whale", "the sea"),
        ] {
            let stored = StoredDocument {
                version: 1,
                document: siftwake::Document {
                    key: siftwake::Key::new("shared", "texts", &format!("{id:010}")),
                    title: title.into(),
                    text: body.into(),
                    fields: Default::default(),
                    links: vec![],
                },
            };
            db.execute(
                "INSERT INTO docs VALUES('shared','texts',?,?,?)",
                params![
                    stored.document.key.id,
                    serde_json::to_string(&stored)?,
                    fold(&format!("{title}\n{body}"))
                ],
            )?;
        }
        for (text, expected) in [
            ("of", 2),
            ("the whale", 1),
            ("İŞ", 1),
            ("sea", 3),
            ("absent", 0),
        ] {
            let (count, page, values) = sql(text, 0)?;
            let (total, hits, sequence) = search(&mut db, &count, &page, &values)?;
            assert_eq!(total, expected);
            assert_eq!(hits.len(), expected);
            assert_eq!(sequence, 200);
        }
        assert!(sql("%", 0).is_err());
        assert!(sql("", 0).is_err());
        let (count, page, values) = sql("sea", 1)?;
        let (total, hits, _) = search(&mut db, &count, &page, &values)?;
        assert_eq!(total, 3);
        assert_eq!(hits[0].document.key.id, "0000000001");
        Ok(())
    }
}
