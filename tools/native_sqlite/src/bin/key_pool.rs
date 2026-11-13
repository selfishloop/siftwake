//! Isolated comparator, using the existing tables and unchanged FTS5 settings.
#[path = "../../../bench_support/block3a.rs"]
mod block3a;
#[allow(dead_code)]
#[path = "../../../bench_support/churn.rs"]
mod churn;
#[path = "../../../bench_support/key_pool.rs"]
mod key_pool;
use fdb::{FirstPage, Key, MatchTotal, Mutation, Query, StoredDocument, ValidationStatus};
use key_pool::{Client, Factory, Result, Store, View};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};
thread_local! {static PLAN_TAKEN:Cell<bool>=const{Cell::new(false)};}
const DB: &str = "data/benchmark-work/sqlite/sqlite.db";
struct Engine(Connection);
#[derive(Clone)]
struct Readers;
struct Reader(Rc<Connection>);
struct SqlView {
    db: Rc<Connection>,
    seq: u64,
    diagnostic: Cell<bool>,
    profile: Rc<RefCell<serde_json::Value>>,
}
fn connect(read: bool) -> Result<Connection> {
    let db = Connection::open(DB)?;
    db.execute_batch("PRAGMA cache_size=-2048;PRAGMA mmap_size=0;PRAGMA temp_store=FILE;")?;
    if read {
        db.execute_batch("PRAGMA query_only=ON;")?;
    } else {
        db.execute_batch("PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;")?;
    }
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    db.set_prepared_statement_cache_capacity(64);
    Ok(db)
}
impl Factory for Readers {
    type Client = Reader;
    fn open(&self) -> Result<Reader> {
        Ok(Reader(Rc::new(connect(true)?)))
    }
}
impl Client for Reader {
    type View = SqlView;
    fn acquire(&mut self) -> Result<SqlView> {
        self.0.execute_batch("BEGIN")?;
        let seq = self
            .0
            .query_row("SELECT sequence FROM meta", [], |r| r.get::<_, i64>(0))?
            as u64;
        Ok(SqlView {
            db: self.0.clone(),
            seq,
            diagnostic: Cell::new(false),
            profile: Rc::new(RefCell::new(serde_json::Value::Null)),
        })
    }
}
impl Drop for SqlView {
    fn drop(&mut self) {
        self.db.execute_batch("ROLLBACK").unwrap();
    }
}
impl View for SqlView {
    fn begin_probe_diagnostic(&self) -> Box<dyn FnOnce() -> serde_json::Value> {
        self.diagnostic.set(true);
        let profile = self.profile.clone();
        let db = self.db.clone();
        let plan = !PLAN_TAKEN.with(|p| p.replace(true));
        Box::new(move || {
            let mut result = std::mem::take(&mut *profile.borrow_mut());
            if plan {
                let sql = result["sql"].as_str().unwrap();
                let args: Vec<_> = result["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                let mut st = db.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
                let details: Vec<String> = st
                    .query_map(params_from_iter(&args), |r| r.get(3))
                    .unwrap()
                    .collect::<std::result::Result<_, _>>()
                    .unwrap();
                result["explain_after_visibility"] = serde_json::json!(details);
            }
            result
        })
    }
    fn sequence(&self) -> u64 {
        self.seq
    }
    fn get(&self, k: &Key) -> Result<Option<StoredDocument>> {
        let raw: Option<String> = self
            .db
            .prepare_cached("SELECT payload FROM docs WHERE scope=? AND collection=? AND id=?")?
            .query_row(params![k.scope, k.collection, k.id], |r| r.get(0))
            .optional()?;
        Ok(raw.map(|p| serde_json::from_str(&p)).transpose()?)
    }
    fn first(&self, q: &Query) -> Result<FirstPage> {
        let trace = self.diagnostic.get();
        let start = trace.then(Instant::now);
        let mut args = vec![q.scope.clone(), q.collection.clone().unwrap()];
        let terms: Vec<_> = q
            .text
            .split_whitespace()
            .map(|t| format!("%{}%", churn::fold(t)))
            .collect();
        let cond = vec!["search.folded LIKE ?"; terms.len()].join(" AND ");
        args.extend(terms);
        let sql = format!(
            "WITH selected AS MATERIALIZED (SELECT d.rowid AS rid,d.id AS id FROM search CROSS JOIN docs d ON d.rowid=search.rowid WHERE d.scope=? AND d.collection=? AND {cond} ORDER BY d.id LIMIT 50) SELECT d.payload FROM selected s JOIN docs d ON d.rowid=s.rid ORDER BY s.id"
        );
        let built_ns = start.map(|t| t.elapsed().as_nanos() as u64);
        let mut statement = self.db.prepare_cached(&sql)?;
        let prepared_ns = start.map(|t| t.elapsed().as_nanos() as u64);
        let mut raw = statement.query_map(params_from_iter(&args), |r| r.get::<_, String>(0))?;
        let mut hits = vec![];
        let (mut step_ns, mut decode_ns) = (0u64, 0u64);
        loop {
            let began = trace.then(Instant::now);
            let row = raw.next();
            if let Some(t) = began {
                step_ns += t.elapsed().as_nanos() as u64;
            }
            let Some(p) = row else {
                break;
            };
            let payload = p?;
            let began = trace.then(Instant::now);
            hits.push(serde_json::from_str(&payload)?);
            if let Some(t) = began {
                decode_ns += t.elapsed().as_nanos() as u64;
            }
        }
        if let Some(t) = start {
            *self.profile.borrow_mut() = serde_json::json!({"sql_build_ns":built_ns.unwrap(),"statement_prepare_ns":prepared_ns.unwrap()-built_ns.unwrap(),"step_and_payload_copy_ns":step_ns,"json_decode_ns":decode_ns,"total_ns":t.elapsed().as_nanos() as u64,"returned":hits.len(),"sql":sql,"args":args});
        }
        Ok(FirstPage {
            total: if hits.len() == 50 {
                MatchTotal::AtLeast(50)
            } else {
                MatchTotal::Exact(hits.len())
            },
            hits,
            sequence: self.seq,
            validation: ValidationStatus::Verified,
        })
    }
}
impl Store for Engine {
    type Factory = Readers;
    fn factory(&mut self) -> Result<Readers> {
        Ok(Readers)
    }
    fn commit(&mut self, changes: Vec<Mutation>) -> Result<u64> {
        let seq = self.sequence()? + 1;
        let tx = self.0.transaction()?;
        for m in changes {
            match m {
                Mutation::Delete { key: k, .. } => {
                    tx.prepare_cached("DELETE FROM docs WHERE scope=? AND collection=? AND id=?")?
                        .execute(params![k.scope, k.collection, k.id])?;
                }
                Mutation::Put { document: d, .. } => {
                    let folded = churn::fold(&format!("{}\n{}", d.title, d.text));
                    let raw = serde_json::to_string(&StoredDocument {
                        document: d.clone(),
                        version: seq,
                    })?;
                    tx.prepare_cached("INSERT INTO docs(scope,collection,id,payload,folded) VALUES(?,?,?,?,?) ON CONFLICT(scope,collection,id) DO UPDATE SET payload=excluded.payload,folded=excluded.folded")?.execute(params![d.key.scope,d.key.collection,d.key.id,raw,folded])?;
                }
            }
        }
        tx.execute("UPDATE meta SET sequence=?", [seq as i64])?;
        tx.commit()?;
        Ok(seq)
    }
    fn keys(&self, col: &str) -> Result<Vec<Key>> {
        let mut st = self
            .0
            .prepare("SELECT id FROM docs WHERE scope='shared' AND collection=?")?;
        Ok(st
            .query_map([col], |r| {
                Ok(Key::new("shared", col, &r.get::<_, String>(0)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }
    fn maintain(&mut self) -> Result<()> {
        self.0
            .execute("INSERT INTO search(search) VALUES('optimize')", [])?;
        self.0.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
    fn sequence(&self) -> Result<u64> {
        Ok(self
            .0
            .query_row("SELECT sequence FROM meta", [], |r| r.get::<_, i64>(0))? as u64)
    }
}
fn main() -> Result<()> {
    churn::start_memory_sampler();
    let mut engine = Engine(connect(false)?);
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args[0] == "block3a" {
        return block3a::run(&mut engine, &args[1], &args[2]);
    }
    if args[0] == "inspect" {
        churn::emit(
            serde_json::json!({"phase":"inspect","sequence":engine.sequence()?,"numeric":engine.keys(key_pool::collection("numeric"))?.len(),"frunner":engine.keys(key_pool::collection("frunner"))?.len()}),
        );
        return Ok(());
    }
    if args[0] == "verify" || args[0] == "hold" {
        key_pool::check_saved(&mut engine, &args[1])?;
        if args[0] == "hold" {
            loop {
                std::thread::park();
            }
        }
        return Ok(());
    }
    if args[0] == "cleanup" {
        key_pool::cleanup(&mut engine)?;
        churn::emit(
            serde_json::json!({"phase":"cleanup","sequence":engine.sequence()?,"memory":churn::memory()}),
        );
        return Ok(());
    }
    let config = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    key_pool::run(&mut engine, config)
}
