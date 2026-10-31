//! Comparator-only SQLite WAL reader transactions; never linked into the FDB engine.
use super::{
    Engine,
    churn::Result,
    search_connection, search_connection_mode,
    snapshot_common::{self, Client, Factory, View},
};
use fdb::{
    Query, SearchPage, SnapshotAdmission, SnapshotReadWait, SnapshotRelease, StoredDocument,
};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
thread_local! {static BUSY:Cell<(u64,u64)>=const{Cell::new((0,0))};}
fn busy(at: i32) -> bool {
    let begin = Instant::now();
    std::thread::sleep(Duration::from_millis(1));
    BUSY.with(|b| {
        let (n, ns) = b.get();
        b.set((n + 1, ns + begin.elapsed().as_nanos() as u64));
    });
    at < 5000
}
#[derive(Clone)]
pub struct SqlFactory;
pub struct SqlClient {
    db: Rc<Connection>,
}
pub struct SqlView {
    db: Rc<Connection>,
    seq: u64,
    active: bool,
}
impl Factory for SqlFactory {
    type Client = SqlClient;
    fn open(&self) -> Result<SqlClient> {
        let db = Connection::open("data/benchmark-work/sqlite/sqlite.db")?;
        db.execute_batch("PRAGMA query_only=ON;PRAGMA cache_size=-2048;PRAGMA mmap_size=0;PRAGMA temp_store=FILE;")?;
        db.set_prepared_statement_cache_capacity(64);
        db.busy_handler(Some(busy))?;
        Ok(SqlClient { db: Rc::new(db) })
    }
}
impl Client for SqlClient {
    type View = SqlView;
    fn acquire(&mut self) -> Result<(SqlView, SnapshotAdmission)> {
        let start = Instant::now();
        let before = BUSY.with(Cell::get);
        self.db.execute_batch("BEGIN")?;
        let seq = self
            .db
            .query_row("SELECT sequence FROM meta", [], |r| r.get::<_, i64>(0))?;
        let after = BUSY.with(Cell::get);
        Ok((
            SqlView {
                db: self.db.clone(),
                seq: seq as u64,
                active: true,
            },
            SnapshotAdmission {
                elapsed_ns: start.elapsed().as_nanos() as u64,
                contended: after.0 != before.0,
                wait_ns: after.1 - before.1,
            },
        ))
    }
}
impl View for SqlView {
    fn first(&self, q: &Query) -> Result<fdb::FirstPage> {
        let p = search_connection_mode(&self.db, self.seq, &q.text, q.offset, q.limit, true)?;
        Ok(fdb::FirstPage {
            total: if p.hits.len() == q.limit {
                fdb::MatchTotal::AtLeast(q.limit)
            } else {
                fdb::MatchTotal::Exact(p.hits.len())
            },
            sequence: self.seq,
            hits: p.hits,
            validation: fdb::ValidationStatus::Verified,
        })
    }
    fn sequence(&self) -> u64 {
        self.seq
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
    fn search(&self, q: &Query) -> Result<(SearchPage, SnapshotReadWait)> {
        let before = BUSY.with(Cell::get);
        let page = search_connection(&self.db, self.seq, &q.text, q.offset, q.limit)?;
        let after = BUSY.with(Cell::get);
        Ok((
            page,
            SnapshotReadWait {
                cache_wait_ns: after.1 - before.1,
                cache_contentions: after.0 - before.0,
                writer_wait_ns: after.1 - before.1,
                writer_contentions: after.0 - before.0,
                max_writer_wait_ns: after.1 - before.1,
            },
        ))
    }
    fn release(mut self) -> Result<SnapshotRelease> {
        let start = Instant::now();
        let before = BUSY.with(Cell::get);
        self.db.execute_batch("COMMIT")?;
        self.active = false;
        let after = BUSY.with(Cell::get);
        Ok(SnapshotRelease {
            writer_wait_ns: after.1 - before.1,
            cleanup_ns: start.elapsed().as_nanos() as u64,
            ..Default::default()
        })
    }
}
impl Drop for SqlView {
    fn drop(&mut self) {
        if self.active {
            let _ = self.db.execute_batch("ROLLBACK");
        }
    }
}
impl snapshot_common::Writer for Engine {
    type Factory = SqlFactory;
    fn factory(&mut self) -> Result<SqlFactory> {
        Ok(SqlFactory)
    }
    fn interlude(&mut self) -> Result<Value> {
        let start = Instant::now();
        self.db
            .execute("INSERT INTO search(search) VALUES('optimize')", [])?;
        let checkpoint: (i64, i64, i64) =
            self.db
                .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?;
        Ok(
            json!({"fts_optimize_ms":start.elapsed().as_secs_f64()*1000.,"passive_checkpoint":checkpoint,"revision_rewrite":"not applicable to SQLite"}),
        )
    }
    fn publication(&self) -> Value {
        json!({"kind":"SQLite WAL read transaction; meta sequence and rows from same BEGIN snapshot","busy_wait":"busy-handler retry time, source not individually classified"})
    }
}
