#[path = "../../../bench_support/block2_scale.rs"]
mod block2_scale;
#[path = "../../../bench_support/churn.rs"]
mod churn;
#[path = "../../../bench_support/small_commit.rs"]
mod small_commit;
#[path = "../../../bench_support/snapshot_common.rs"]
mod snapshot_common;
#[path = "../../../bench_support/sqlite_snapshot.rs"]
mod sqlite_snapshot;
#[path = "../../../bench_support/write50.rs"]
mod write50;
#[path = "../../../bench_support/write_hardening.rs"]
mod write_hardening;
use churn::{Backend, Change, Corpus, Result};
use fdb::{SearchPage, StoredDocument};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};
fn search_connection(
    db: &Connection,
    sequence: u64,
    text: &str,
    offset: usize,
    limit: usize,
) -> Result<SearchPage> {
    search_connection_mode(db, sequence, text, offset, limit, false)
}
fn search_connection_mode(
    db: &Connection,
    sequence: u64,
    text: &str,
    offset: usize,
    limit: usize,
    capped: bool,
) -> Result<SearchPage> {
    let values: Vec<_> = text
        .split_whitespace()
        .map(|t| format!("%{}%", churn::fold(t)))
        .collect();
    let conditions = vec!["search.folded LIKE ?"; values.len()].join(" AND ");
    let from = format!(
        "search CROSS JOIN docs d ON d.rowid=search.rowid WHERE d.scope='shared' AND d.collection='texts' AND {conditions}"
    );
    let total: i64 = if capped {
        0
    } else {
        db.prepare_cached(&format!("SELECT count(*) FROM {from}"))?
            .query_row(params_from_iter(&values), |r| r.get(0))?
    };
    let sql = format!(
        "WITH selected AS MATERIALIZED (SELECT d.rowid AS rid,d.id AS id FROM {from} ORDER BY d.id LIMIT {limit} OFFSET {offset}) SELECT d.payload FROM selected s JOIN docs d ON d.rowid=s.rid ORDER BY s.id"
    );
    let hits: Vec<StoredDocument> = {
        let mut statement = db.prepare_cached(&sql)?;
        let rows = statement.query_map(params_from_iter(&values), |r| r.get::<_, String>(0))?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(serde_json::from_str(&row?)?);
        }
        hits
    };
    Ok(SearchPage {
        total: if capped { hits.len() } else { total as usize },
        sequence,
        hits,
    })
}

struct Engine {
    db: Connection,
    sequence: u64,
}
impl Backend for Engine {
    fn query_variant(&self, text: &str, variant: &str) -> Result<fdb::FirstPage> {
        let capped = variant.starts_with("cap");
        let tx = self.db.unchecked_transaction()?;
        let p = search_connection_mode(&tx, self.sequence, text, 0, 50, capped)?;
        tx.commit()?;
        Ok(fdb::FirstPage {
            total: if capped && p.hits.len() == 50 {
                fdb::MatchTotal::AtLeast(50)
            } else {
                fdb::MatchTotal::Exact(p.total)
            },
            sequence: p.sequence,
            hits: p.hits,
            validation: fdb::ValidationStatus::Verified,
        })
    }
    fn get(&self, id: usize) -> Result<Option<StoredDocument>> {
        let payload: Option<String> = self
            .db
            .prepare_cached(
                "SELECT payload FROM docs WHERE scope='shared' AND collection='texts' AND id=?",
            )?
            .query_row([format!("{id:010}")], |r| r.get(0))
            .optional()?;
        Ok(payload.map(|p| serde_json::from_str(&p)).transpose()?)
    }
    fn search(&self, text: &str, offset: usize, limit: usize) -> Result<SearchPage> {
        let tx = self.db.unchecked_transaction()?;
        let page = search_connection(&tx, self.sequence, text, offset, limit)?;
        tx.commit()?;
        Ok(page)
    }
    fn commit(&mut self, corpus: &mut Corpus, changes: &[Change], _streamed: bool) -> Result<u64> {
        let profile = std::env::var_os("FDB_SQL_WRITE_PROFILE").is_some();
        let mut donor_ns = 0u128;
        let mut fold_encode_ns = 0u128;
        let mut sql_fts_ns = 0u128;
        let sequence = self.sequence + 1;
        let tx = self.db.transaction()?;
        for c in changes {
            if let Some(row) = &c.new {
                let began = profile.then(Instant::now);
                let doc = corpus.document(c.id, row)?;
                if let Some(at) = began {
                    donor_ns += at.elapsed().as_nanos();
                }
                let began = profile.then(Instant::now);
                let folded = churn::fold(&format!("{}\n{}", doc.title, doc.text));
                let payload = serde_json::to_string(&StoredDocument {
                    document: doc,
                    version: sequence,
                })?;
                if let Some(at) = began {
                    fold_encode_ns += at.elapsed().as_nanos();
                }
                let began = profile.then(Instant::now);
                tx.prepare_cached("INSERT INTO docs(scope,collection,id,payload,folded) VALUES('shared','texts',?,?,?) ON CONFLICT(scope,collection,id) DO UPDATE SET payload=excluded.payload,folded=excluded.folded")?.execute(params![format!("{:010}",c.id),payload,folded])?;
                if let Some(at) = began {
                    sql_fts_ns += at.elapsed().as_nanos();
                }
            } else {
                tx.prepare_cached(
                    "DELETE FROM docs WHERE scope='shared' AND collection='texts' AND id=?",
                )?
                .execute([format!("{:010}", c.id)])?;
            }
        }
        tx.execute("UPDATE meta SET sequence=?", [sequence as i64])?;
        let began = profile.then(Instant::now);
        tx.commit()?;
        if let Some(at) = began {
            churn::emit(
                json!({"phase":"sqlite_write_profile","sequence":sequence,"mutations":changes.len(),"donor_ns":donor_ns,"fold_encode_ns":fold_encode_ns,"sql_fts_ns":sql_fts_ns,"tx_commit_sync_ns":at.elapsed().as_nanos()}),
            );
        }
        self.sequence = sequence;
        Ok(sequence)
    }
    fn maintain(&mut self) -> Result<Value> {
        let before = self.stats()?;
        self.db
            .execute("INSERT INTO search(search) VALUES('optimize')", [])?;
        self.db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(json!({"before":before,"after":self.stats()?,"merge_completions":null}))
    }
    fn start_merge(&mut self) -> Result<bool> {
        Ok(false)
    }
    fn stats(&self) -> Result<Value> {
        let count: i64 = self
            .db
            .query_row("SELECT count(*) FROM docs", [], |r| r.get(0))?;
        let pages: i64 = self.db.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let free: i64 = self
            .db
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let size: i64 = self.db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok(
            json!({"documents":count,"sequence":self.sequence,"delta_keys":null,"resident_documents":null,"compaction_running":false,"page_count":pages,"freelist_pages":free,"live_allocated_bytes":(pages-free)*size,"source_bytes":pages*size}),
        )
    }
}
fn main() -> Result<()> {
    churn::start_memory_sampler();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("content_churn MODE streamed|owned STATE.json".into());
    }
    let mode = &args[0];
    let path = "data/benchmark-work/sqlite";
    if (mode == "seed" || mode == "scale-seed") && Path::new(path).exists() {
        return Err("seed requires ownership-checked reset".into());
    }
    std::fs::create_dir_all(path)?;
    let cpu_start = churn::cpu_ms();
    let start = Instant::now();
    let db = Connection::open(format!("{path}/sqlite.db"))?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA cache_size=-2048; PRAGMA mmap_size=0; PRAGMA temp_store=FILE;")?;
    db.set_prepared_statement_cache_capacity(64);
    if mode == "seed" || mode == "scale-seed" {
        db.execute_batch("CREATE TABLE docs(scope TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,folded TEXT NOT NULL,UNIQUE(scope,collection,id));CREATE VIRTUAL TABLE search USING fts5(folded,content='docs',content_rowid='rowid',tokenize='trigram',detail=none,columnsize=0);CREATE TRIGGER docs_ai AFTER INSERT ON docs BEGIN INSERT INTO search(rowid,folded) VALUES(new.rowid,new.folded);END;CREATE TRIGGER docs_ad AFTER DELETE ON docs BEGIN INSERT INTO search(search,rowid,folded) VALUES('delete',old.rowid,old.folded);END;CREATE TRIGGER docs_au AFTER UPDATE ON docs BEGIN INSERT INTO search(search,rowid,folded) VALUES('delete',old.rowid,old.folded);INSERT INTO search(rowid,folded) VALUES(new.rowid,new.folded);END;CREATE TABLE meta(sequence INTEGER NOT NULL);INSERT INTO meta VALUES(0);")?;
    }
    let sequence = db.query_row("SELECT sequence FROM meta", [], |r| r.get::<_, i64>(0))? as u64;
    let mut engine = Engine { db, sequence };
    let session_ms = start.elapsed().as_secs_f64() * 1000.;
    churn::emit(
        json!({"phase":"opened","engine":"sqlite","mode":mode,"open_ms":start.elapsed().as_secs_f64()*1000.,"strict":false,"memory":churn::memory(),"stats":engine.stats()?}),
    );
    if mode.starts_with("write50-") {
        return write50::run(&mut engine, &args[2], mode);
    }
    if mode == "first-sqlite" {
        let text = std::env::var("FDB_FIRST_QUERY")?;
        let p = engine.query_variant(&text, "cap")?;
        let first_ms = start.elapsed().as_secs_f64() * 1000.;
        let cpu_ms = cpu_start.zip(churn::cpu_ms()).map(|(a, b)| b - a);
        churn::emit(
            json!({"phase":"first_result","mode":mode,"text":text,"session_ms":session_ms,"first_ms":first_ms,"full_validation_ms":null,"cpu_ms":cpu_ms,"memory":churn::memory(),"total":p.total,"returned":p.hits.len(),"validation_contract":"SQLite default integrity policy, no FDB strict source pass"}),
        );
        let mut corpus = Corpus::open()?;
        let model = churn::Model::load(&args[2])?;
        let c = corpus
            .index
            .cases
            .iter()
            .find(|c| c.text == text && c.offset == 0 && c.limit == 50)
            .unwrap()
            .clone();
        let exact = engine.search(&text, 0, 50)?;
        churn::check_page(&mut corpus, &model, &c, &exact)?;
        assert_eq!(p.hits, exact.hits);
        churn::emit(json!({"phase":"first_result_oracle","oracle":true}));
        return Ok(());
    }
    if mode.starts_with("scale-") {
        return block2_scale::run(&mut engine, mode, &args[2], path);
    }
    if mode == "snapshot-one" || mode == "snapshot-100" || mode.starts_with("snapshot-control-") {
        let mut engine = engine;
        return snapshot_common::run(&mut engine, &args[2], mode);
    }
    if mode.starts_with("small-") {
        let mut engine = engine;
        return small_commit::run(&mut engine, &args[2], mode);
    }
    if mode == "write-churn-30" {
        let mut engine = engine;
        return write_hardening::run(&mut engine, &args[2], path);
    }
    if mode == "write-churn-idle-30" {
        let mut engine = engine;
        return write_hardening::idle_each_round(&mut engine, &args[2], path);
    }
    churn::run(engine, mode, &args[1], &args[2], path)
}
