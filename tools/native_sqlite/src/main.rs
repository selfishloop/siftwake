//! Native comparator only: same Rust corpus and payload types as FDB.
#[path = "../../bench_support/corpus.rs"]
mod corpus;
#[path = "../../bench_support/verify.rs"]
mod verify;
use fdb::StoredDocument;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Instant;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn report(value: Value) {
    println!("{value}");
}

fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            match c {
                'İ' | 'ı' | 'I' => 'i',
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

fn connect(path: &Path, cache_kib: usize) -> Result<Connection> {
    let db = Connection::open(path)?;
    db.execute_batch(&format!(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA cache_size=-{cache_kib};
         PRAGMA mmap_size=0; PRAGMA temp_store=FILE;"
    ))?;
    Ok(db)
}

fn query_impl(
    db: &mut Connection,
    kind: &str,
    expected: usize,
    repeats: usize,
    text: &str,
    offset: usize,
    oracle: Option<&[StoredDocument]>,
) -> Result<(Value, Vec<StoredDocument>)> {
    let (source, condition, values) = match kind {
        "text" => (
            "search CROSS JOIN docs d ON d.rowid=search.rowid",
            "search.folded LIKE ? AND d.scope=? AND d.collection=?",
            vec![
                format!("%{}%", fold(text)),
                "shared".into(),
                "contents".into(),
            ],
        ),
        "range" => (
            "docs d",
            "d.scope=? AND d.collection=? AND d.source=? AND d.day BETWEEN 2 AND 5",
            vec!["shared".into(), "contents".into(), "synthetic".into()],
        ),
        _ => (
            "docs d",
            "d.scope=? AND d.collection=? AND d.source=?",
            vec!["shared".into(), "contents".into(), "synthetic".into()],
        ),
    };
    let mut times = vec![];
    let mut last = vec![];
    for repeat in 0..=repeats {
        let start = Instant::now();
        let tx = db.transaction()?;
        let total: usize = tx.query_row(
            &format!("SELECT count(*) FROM {source} WHERE {condition}"),
            rusqlite::params_from_iter(&values),
            |row| row.get::<_, i64>(0),
        )? as usize;
        let mut hits = vec![];
        {
            let mut stmt = tx.prepare(&format!(
                "SELECT d.payload FROM {source} WHERE {condition} ORDER BY d.id LIMIT 50 OFFSET {offset}"))?;
            let mut rows = stmt.query(rusqlite::params_from_iter(&values))?;
            while let Some(row) = rows.next()? {
                let payload: String = row.get(0)?;
                hits.push(serde_json::from_str::<StoredDocument>(&payload)?);
            }
        }
        tx.commit()?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if repeat != 0 {
            times.push(elapsed);
        }
        assert_eq!(total, expected);
        assert_eq!(hits.len(), 50.min(total.saturating_sub(offset)));
        if let Some(oracle) = oracle {
            assert_eq!(hits, oracle);
        }
        last = hits;
    }
    Ok((corpus::samples(times), last))
}

#[cfg(test)]
fn query(
    db: &mut Connection,
    kind: &str,
    expected: usize,
    repeats: usize,
    text: &str,
    offset: usize,
) -> Result<(Value, Vec<StoredDocument>)> {
    query_impl(db, kind, expected, repeats, text, offset, None)
}
fn checked_query(
    db: &mut Connection,
    kind: &str,
    repeats: usize,
    text: &str,
    offset: usize,
    corpus: (usize, &str, bool),
) -> Result<(Value, Vec<StoredDocument>)> {
    let (count, body, changed) = corpus;
    let oracle_kind = if kind == "text" && text.starts_with("rapor-") {
        "narrow"
    } else {
        kind
    };
    let (total, hits) = verify::expected(oracle_kind, count, body, changed, offset);
    query_impl(db, kind, total, repeats, text, offset, Some(&hits))
}

fn lookup(db: &Connection, id: usize) -> Result<Option<StoredDocument>> {
    let payload: Option<String> = db
        .query_row(
            "SELECT payload FROM docs WHERE scope='shared' AND collection='contents' AND id=?",
            [format!("{id:010}")],
            |row| row.get(0),
        )
        .optional()?;
    payload
        .map(|p| serde_json::from_str(&p).map_err(Into::into))
        .transpose()
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .ok_or("native_sqlite <new-directory> [count] [text-bytes] [cache-kib]")?;
    let count: usize = args
        .next()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(1_000_000);
    let text_bytes: usize = args.next().map(|v| v.parse()).transpose()?.unwrap_or(256);
    let cache_kib: usize = args.next().map(|v| v.parse()).transpose()?.unwrap_or(2048);
    if args.next().is_some() || count < 1000 || text_bytes > 1024 * 1024 || cache_kib == 0 {
        return Err("invalid benchmark arguments".into());
    }
    let root = Path::new(&root);
    if root.exists() {
        return Err("new directory required".into());
    }
    std::fs::create_dir_all(root)?;
    let path = root.join("sqlite.db");
    let mut db = connect(&path, cache_kib)?;
    db.execute_batch(include_str!("../schema.sql"))?;
    let body = corpus::body(text_bytes);
    report(
        json!({"phase":"ingest","sqlite_version":rusqlite::version(),"count":count,
        "text_bytes":body.len(),"cache_kib":cache_kib,"mmap_bytes":0,"synchronous":"FULL",
        "journal_mode":"WAL","corpus":"shared Rust generator","payload":"FDB StoredDocument JSON"}),
    );
    let start = Instant::now();
    let mut batches = vec![];
    for first in (0..count).step_by(500) {
        let version = (first / 500 + 1) as u64;
        let rows = (first..(first + 500).min(count))
            .map(|id| {
                let stored = StoredDocument {
                    document: corpus::document(id, &body),
                    version,
                };
                let folded = fold(&format!(
                    "{}\n{}",
                    stored.document.title, stored.document.text
                ));
                Ok((id, serde_json::to_string(&stored)?, folded))
            })
            .collect::<Result<Vec<_>>>()?;
        let batch = Instant::now();
        let tx = db.transaction()?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO docs(scope,collection,id,source,day,payload,folded)
                VALUES('shared','contents',?,'synthetic',?,?,?)",
            )?;
            for (id, payload, folded) in rows {
                insert.execute(params![
                    format!("{id:010}"),
                    (id % 30) as i64,
                    payload,
                    folded
                ])?;
            }
        }
        tx.commit()?;
        batches.push(batch.elapsed().as_secs_f64() * 1000.0);
        if (first + 500).is_multiple_of(50_000) {
            report(
                json!({"phase":"ingest_progress","records":first+500,"elapsed_s":start.elapsed().as_secs_f64()}),
            );
        }
    }
    let ingest_ms = start.elapsed().as_secs_f64() * 1000.0;
    report(
        json!({"phase":"maintenance","ingest_ms":ingest_ms,"batches":corpus::samples(batches.clone())}),
    );
    let start = Instant::now();
    db.execute("INSERT INTO search(search) VALUES('optimize')", [])?;
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    let maintenance_ms = start.elapsed().as_secs_f64() * 1000.0;
    drop(db);
    let start = Instant::now();
    let mut db = connect(&path, cache_kib)?;
    let reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
    for id in [0, count / 2, count - 1] {
        assert_eq!(lookup(&db, id)?.unwrap(), verify::get(id, &body));
    }
    report(json!({"phase":"queries","reopen_ms":reopen_ms,"maintenance_ms":maintenance_ms}));
    let narrow = checked_query(
        &mut db,
        "text",
        30,
        &format!("rapor-{:010}", count / 2),
        0,
        (count, &body, false),
    )?
    .0;
    let broad = checked_query(&mut db, "exact", 30, "", 0, (count, &body, false))?.0;
    let second_page = checked_query(&mut db, "exact", 30, "", 50, (count, &body, false))?.0;
    let (last_page, hits) =
        checked_query(&mut db, "exact", 3, "", count - 50, (count, &body, false))?;
    assert_eq!(hits[0].document.key, corpus::key(count - 50));
    let range = checked_query(&mut db, "range", 30, "", 0, (count, &body, false))?.0;
    let mut get_times = Vec::new();
    for repeat in 0..=3 {
        let start = Instant::now();
        let found = (0..100)
            .map(|n| lookup(&db, n * (count / 100)))
            .collect::<Result<Vec<_>>>()?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if repeat != 0 {
            get_times.push(elapsed);
        }
        for (n, actual) in found.into_iter().enumerate() {
            assert_eq!(actual.unwrap(), verify::get(n * (count / 100), &body));
        }
    }
    let get_100 = corpus::samples(get_times);
    {
        let tx = db.transaction()?;
        for n in 0..100 {
            let id = n * (count / 100);
            if n % 2 == 0 {
                assert_eq!(
                    tx.execute(
                        "DELETE FROM docs WHERE scope='shared' AND collection='contents' AND id=?",
                        [format!("{id:010}")]
                    )?,
                    1
                );
            } else {
                let stored = StoredDocument {
                    document: corpus::document(id, "changed sentinel"),
                    version: count.div_ceil(500) as u64 + 1,
                };
                let folded = fold(&format!(
                    "{}\n{}",
                    stored.document.title, stored.document.text
                ));
                assert_eq!(tx.execute("UPDATE docs SET payload=?,folded=? WHERE scope='shared' AND collection='contents' AND id=?",
                    params![serde_json::to_string(&stored)?, folded, format!("{id:010}")])?, 1);
            }
        }
        tx.commit()?;
    }
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    drop(db);
    let start = Instant::now();
    let mut db = connect(&path, cache_kib)?;
    assert_eq!(
        db.query_row("SELECT count(*) FROM docs", [], |r| r.get::<_, i64>(0))?,
        (count - 50) as i64
    );
    let changed_reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
    for n in 0..100 {
        let stored = lookup(&db, n * (count / 100))?;
        if n % 2 == 0 {
            assert!(stored.is_none());
        } else {
            assert_eq!(stored.unwrap().document.text, "changed sentinel");
        }
    }
    let broad_text = checked_query(&mut db, "text", 3, "teknoloji", 0, (count, &body, true))?.0;
    let integrity_ms = if std::env::var("SQLITE_BENCH_SKIP_INTEGRITY").as_deref() == Ok("1") {
        None
    } else {
        report(json!({"phase":"integrity"}));
        let start = Instant::now();
        let check: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        assert_eq!(check, "ok");
        db.execute(
            "INSERT INTO search(search,rank) VALUES('integrity-check',1)",
            [],
        )?;
        Some(start.elapsed().as_secs_f64() * 1000.0)
    };
    report(
        json!({"phase":"result","count":count,"text_bytes":body.len(),"ingest_ms":ingest_ms,
        "maintenance_ms":maintenance_ms,"reopen_ms":reopen_ms,"changed_reopen_ms":changed_reopen_ms,
        "batches":corpus::samples(batches),"narrow":narrow,"broad":broad,"range":range,
        "broad_text":broad_text,"last_page":last_page,"integrity_ms":integrity_ms,
        "verified_updates":50,"verified_deletes":50,"documents":count-50,"second_page":second_page,"get_100":get_100,"query_verification":"full StoredDocument oracle; untimed warmup; checks outside timing"}),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_triggers_and_duplicate_batch_rollback_preserve_search_results() -> Result<()> {
        let mut db = Connection::open_in_memory()?;
        db.execute_batch(include_str!("../schema.sql"))?;
        let payload = serde_json::to_string(&StoredDocument {
            document: corpus::document(1, "İŞ GÜÇ"),
            version: 1,
        })?;
        let insert = "INSERT INTO docs VALUES('shared','contents',?,'synthetic',1,?,?)";
        db.execute(
            insert,
            params!["0000000001", payload, fold("rapor-0000000001\nİŞ GÜÇ")],
        )?;
        let tx = db.transaction()?;
        tx.execute(insert, params!["0000000002", payload, "teknoloji"])?;
        assert!(
            tx.execute(insert, params!["0000000001", payload, "duplicate"])
                .is_err()
        );
        tx.rollback()?;
        assert!(lookup(&db, 2)?.is_none());
        assert_eq!(fold("İıI Şş Ğğ Çç Öö Üü"), "iii ss gg cc oo uu");
        assert_eq!(query(&mut db, "text", 1, 1, "İŞ", 0)?.1.len(), 1);
        db.execute(
            "UPDATE docs SET folded='changed sentinel' WHERE id='0000000001'",
            [],
        )?;
        query(&mut db, "text", 0, 1, "İŞ", 0)?;
        db.execute("DELETE FROM docs WHERE id='0000000001'", [])?;
        query(&mut db, "text", 0, 1, "changed", 0)?;
        db.execute(
            "INSERT INTO search(search,rank) VALUES('integrity-check',1)",
            [],
        )?;
        Ok(())
    }
}
