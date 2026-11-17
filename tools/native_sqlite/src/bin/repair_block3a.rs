//! Isolated SQLite cleanup only. No optimize/checkpoint or benchmark command.
#[allow(dead_code)]
#[path = "../../../bench_support/churn.rs"]
mod churn;
#[path = "../../../bench_support/block3a_repair.rs"]
mod repair;
use churn::Result;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use siftwake::{FirstPage, Key, MatchTotal, Query, StoredDocument, ValidationStatus};
struct Engine(Connection);
impl repair::Store for Engine {
    fn sequence(&mut self) -> Result<u64> {
        Ok(self
            .0
            .query_row("SELECT sequence FROM meta", [], |r| r.get::<_, i64>(0))? as u64)
    }
    fn count(&mut self) -> Result<usize> {
        Ok(self
            .0
            .query_row("SELECT count(*) FROM docs", [], |r| r.get::<_, i64>(0))?
            as usize)
    }
    fn get(&mut self, k: &Key) -> Result<Option<StoredDocument>> {
        let raw: Option<String> = self
            .0
            .prepare_cached("SELECT payload FROM docs WHERE scope=? AND collection=? AND id=?")?
            .query_row(params![k.scope, k.collection, k.id], |r| r.get(0))
            .optional()?;
        Ok(raw.map(|s| serde_json::from_str(&s)).transpose()?)
    }
    fn first(&mut self, q: &Query) -> Result<FirstPage> {
        let mut args = vec![q.scope.clone(), q.collection.clone().unwrap()];
        args.extend(
            q.text
                .split_whitespace()
                .map(|s| format!("%{}%", churn::fold(s))),
        );
        let condition = vec!["search.folded LIKE ?"; args.len() - 2].join(" AND ");
        let sql = format!(
            "WITH selected AS MATERIALIZED (SELECT d.rowid AS rid,d.id AS id FROM search CROSS JOIN docs d ON d.rowid=search.rowid WHERE d.scope=? AND d.collection=? AND {condition} ORDER BY d.id LIMIT 50) SELECT d.payload FROM selected s JOIN docs d ON d.rowid=s.rid ORDER BY s.id"
        );
        let hits: Vec<StoredDocument> = self
            .0
            .prepare_cached(&sql)?
            .query_map(params_from_iter(&args), |r| r.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect::<Result<_>>()?;
        Ok(FirstPage {
            sequence: self.sequence()?,
            total: if hits.len() == 50 {
                MatchTotal::AtLeast(50)
            } else {
                MatchTotal::Exact(hits.len())
            },
            hits,
            validation: ValidationStatus::Verified,
        })
    }
    fn delete_expected(&mut self, rows: &[StoredDocument]) -> Result<u64> {
        assert_eq!(rows.len(), 16);
        let sequence = self.sequence()? + 1;
        let tx = self.0.transaction()?;
        // Recheck every complete row inside the transaction, before the first DELETE.
        for (slot, row) in rows.iter().enumerate() {
            assert_eq!(row.document.key, repair::key(slot));
            let k = &row.document.key;
            let raw: String = tx.query_row(
                "SELECT payload FROM docs WHERE scope=? AND collection=? AND id=?",
                params![k.scope, k.collection, k.id],
                |r| r.get(0),
            )?;
            assert_eq!(serde_json::from_str::<StoredDocument>(&raw)?, *row);
        }
        for row in rows {
            let k = &row.document.key;
            assert_eq!(
                tx.execute(
                    "DELETE FROM docs WHERE scope=? AND collection=? AND id=?",
                    params![k.scope, k.collection, k.id]
                )?,
                1
            );
        }
        tx.execute("UPDATE meta SET sequence=?", [sequence as i64])?;
        tx.commit()?;
        Ok(sequence)
    }
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 2);
    let db = Connection::open("data/benchmark-work/sqlite/sqlite.db")?;
    db.execute_batch("PRAGMA cache_size=-2048; PRAGMA mmap_size=0; PRAGMA temp_store=FILE; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    db.set_prepared_statement_cache_capacity(64);
    repair::run(&mut Engine(db), &args[0], &args[1])
}
