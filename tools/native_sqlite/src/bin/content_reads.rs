#[path = "../../../bench_support/memory_corpus.rs"]
mod input;
#[path = "../../../bench_support/read_matrix.rs"]
mod matrix;
#[path = "../../../bench_support/content_reads.rs"]
mod reads;
use fdb::StoredDocument;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params, params_from_iter};
use serde_json::json;
use std::time::Instant;
fn main() -> input::Result<()> {
    let start = Instant::now();
    let db = Connection::open_with_flags(
        "data/benchmark-work/sqlite/sqlite.db",
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    db.execute_batch("PRAGMA cache_size=-2048; PRAGMA mmap_size=0; PRAGMA temp_store=FILE;")?;
    let open_ms = start.elapsed().as_secs_f64() * 1000.;
    assert_eq!(
        db.query_row("SELECT count(*) FROM docs", [], |r| r.get::<_, i64>(0))?,
        10000
    );
    println!(
        "{}",
        json!({"phase":"opened","engine":"sqlite","open_ms":open_ms,"sqlite_version":rusqlite::version(),"full_integrity_on_open":false,"cache_kib":2048,"mmap_bytes":0})
    );
    if let Some(seed) = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|args| args[0] == "--matrix")
        .map(|args| args[1].parse::<u64>())
        .transpose()?
    {
        return matrix::Matrix::load()?.measure(
            seed,
            |key| get(&db, key),
            |text, offset, limit| search(&db, text, offset, limit),
        );
    }
    let oracle = reads::Oracle::load()?;
    if std::env::args().any(|arg| arg == "--check-only") {
        for (n, text) in reads::QUERIES.iter().enumerate() {
            let (total, hits) = search(&db, text, 0, 50)?;
            assert_eq!(total, oracle.totals[n]);
            assert_eq!(hits, oracle.pages[n]);
        }
        println!(
            "{}",
            json!({"phase":"checked","queries":reads::QUERIES,"full_pages_and_versions_verified":true,"performance_not_measured":true})
        );
        return Ok(());
    }
    reads::measure(
        &oracle,
        |key| get(&db, key),
        |text| search(&db, text, 0, 50),
    )
}
fn get(db: &Connection, key: &fdb::Key) -> input::Result<Option<StoredDocument>> {
    let payload: Option<String> = db
        .query_row(
            "SELECT payload FROM docs WHERE scope=? AND collection=? AND id=?",
            params![key.scope, key.collection, key.id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(payload.map(|p| serde_json::from_str(&p)).transpose()?)
}
fn search(
    db: &Connection,
    text: &str,
    offset: usize,
    limit: usize,
) -> input::Result<(usize, Vec<StoredDocument>)> {
    let values: Vec<_> = text
        .split_whitespace()
        .map(|term| format!("%{}%", reads::fold(term)))
        .collect();
    let conditions = vec!["search.folded LIKE ?"; values.len()].join(" AND ");
    let from = format!(
        "search CROSS JOIN docs d ON d.rowid=search.rowid WHERE d.scope='shared' AND d.collection='texts' AND {conditions}"
    );
    let tx = db.unchecked_transaction()?;
    let total: i64 = tx.query_row(
        &format!("SELECT count(*) FROM {from}"),
        params_from_iter(&values),
        |r| r.get(0),
    )?;
    let sql = format!(
        "WITH selected AS MATERIALIZED (SELECT d.rowid AS rid,d.id AS id FROM {from} ORDER BY d.id LIMIT {limit} OFFSET {offset}) SELECT d.payload FROM selected s JOIN docs d ON d.rowid=s.rid ORDER BY s.id"
    );
    let hits = {
        let mut statement = tx.prepare(&sql)?;
        let payloads =
            statement.query_map(params_from_iter(&values), |row| row.get::<_, String>(0))?;
        let mut hits = Vec::new();
        for payload in payloads {
            hits.push(serde_json::from_str::<StoredDocument>(&payload?)?);
        }
        hits
    };
    tx.commit()?;
    Ok((usize::try_from(total)?, hits))
}
