#[path = "../../../bench_support/churn.rs"]
mod churn;
use churn::{Backend, Change, Corpus, Result};
use fdb::{SearchPage, StoredDocument};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};
struct Engine {
    db: Connection,
    sequence: u64,
}
impl Backend for Engine {
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
        let values: Vec<_> = text
            .split_whitespace()
            .map(|t| format!("%{}%", churn::fold(t)))
            .collect();
        let conditions = vec!["search.folded LIKE ?"; values.len()].join(" AND ");
        let from = format!(
            "search CROSS JOIN docs d ON d.rowid=search.rowid WHERE d.scope='shared' AND d.collection='texts' AND {conditions}"
        );
        let tx = self.db.unchecked_transaction()?;
        let total: i64 = tx
            .prepare_cached(&format!("SELECT count(*) FROM {from}"))?
            .query_row(params_from_iter(&values), |r| r.get(0))?;
        let sql = format!(
            "WITH selected AS MATERIALIZED (SELECT d.rowid AS rid,d.id AS id FROM {from} ORDER BY d.id LIMIT {limit} OFFSET {offset}) SELECT d.payload FROM selected s JOIN docs d ON d.rowid=s.rid ORDER BY s.id"
        );
        let hits = {
            let mut statement = tx.prepare_cached(&sql)?;
            let rows = statement.query_map(params_from_iter(&values), |r| r.get::<_, String>(0))?;
            let mut hits = Vec::new();
            for row in rows {
                hits.push(serde_json::from_str(&row?)?);
            }
            hits
        };
        tx.commit()?;
        Ok(SearchPage {
            total: total as usize,
            sequence: self.sequence,
            hits,
        })
    }
    fn commit(&mut self, corpus: &mut Corpus, changes: &[Change], _streamed: bool) -> Result<u64> {
        let sequence = self.sequence + 1;
        let tx = self.db.transaction()?;
        for c in changes {
            if let Some(row) = &c.new {
                let doc = corpus.document(c.id, row)?;
                let folded = churn::fold(&format!("{}\n{}", doc.title, doc.text));
                let payload = serde_json::to_string(&StoredDocument {
                    document: doc,
                    version: sequence,
                })?;
                tx.prepare_cached("INSERT INTO docs(scope,collection,id,payload,folded) VALUES('shared','texts',?,?,?) ON CONFLICT(scope,collection,id) DO UPDATE SET payload=excluded.payload,folded=excluded.folded")?.execute(params![format!("{:010}",c.id),payload,folded])?;
            } else {
                tx.prepare_cached(
                    "DELETE FROM docs WHERE scope='shared' AND collection='texts' AND id=?",
                )?
                .execute([format!("{:010}", c.id)])?;
            }
        }
        tx.execute("UPDATE meta SET sequence=?", [sequence as i64])?;
        tx.commit()?;
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
    if mode == "seed" && Path::new(path).exists() {
        return Err("seed requires ownership-checked reset".into());
    }
    std::fs::create_dir_all(path)?;
    let start = Instant::now();
    let db = Connection::open(format!("{path}/sqlite.db"))?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA cache_size=-2048; PRAGMA mmap_size=0; PRAGMA temp_store=FILE;")?;
    db.set_prepared_statement_cache_capacity(64);
    if mode == "seed" {
        db.execute_batch("CREATE TABLE docs(scope TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,folded TEXT NOT NULL,UNIQUE(scope,collection,id));CREATE VIRTUAL TABLE search USING fts5(folded,content='docs',content_rowid='rowid',tokenize='trigram',detail=none,columnsize=0);CREATE TRIGGER docs_ai AFTER INSERT ON docs BEGIN INSERT INTO search(rowid,folded) VALUES(new.rowid,new.folded);END;CREATE TRIGGER docs_ad AFTER DELETE ON docs BEGIN INSERT INTO search(search,rowid,folded) VALUES('delete',old.rowid,old.folded);END;CREATE TRIGGER docs_au AFTER UPDATE ON docs BEGIN INSERT INTO search(search,rowid,folded) VALUES('delete',old.rowid,old.folded);INSERT INTO search(rowid,folded) VALUES(new.rowid,new.folded);END;CREATE TABLE meta(sequence INTEGER NOT NULL);INSERT INTO meta VALUES(0);")?;
    }
    let sequence = db.query_row("SELECT sequence FROM meta", [], |r| r.get::<_, i64>(0))? as u64;
    let engine = Engine { db, sequence };
    churn::emit(
        json!({"phase":"opened","engine":"sqlite","mode":mode,"open_ms":start.elapsed().as_secs_f64()*1000.,"strict":false,"memory":churn::memory(),"stats":engine.stats()?}),
    );
    churn::run(engine, mode, &args[1], &args[2], path)
}
