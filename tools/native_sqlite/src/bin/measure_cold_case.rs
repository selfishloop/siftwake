#[allow(dead_code)]
#[path = "../../../bench_support/churn.rs"]
mod churn;
#[path = "../../../bench_support/cold_case.rs"]
mod cold_case;
use churn::Result;
use fdb::{FirstPage, Key, MatchTotal, Query, StoredDocument, ValidationStatus};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params, params_from_iter};

struct Reader {
    db: Connection,
    sequence: u64,
}

impl cold_case::Reader for Reader {
    fn get(&mut self, key: &Key) -> Result<Option<StoredDocument>> {
        let payload: Option<String> = self
            .db
            .prepare_cached("SELECT payload FROM docs WHERE scope=? AND collection=? AND id=?")?
            .query_row(params![key.scope, key.collection, key.id], |r| r.get(0))
            .optional()?;
        Ok(payload.map(|s| serde_json::from_str(&s)).transpose()?)
    }

    fn first(&mut self, q: &Query) -> Result<FirstPage> {
        let values: Vec<_> = q
            .text
            .split_whitespace()
            .map(|s| format!("%{}%", churn::fold(s)))
            .collect();
        let condition = vec!["search.folded LIKE ?"; values.len()].join(" AND ");
        let sql = format!(
            "WITH selected AS MATERIALIZED (SELECT d.rowid AS rid,d.id AS id FROM search CROSS JOIN docs d ON d.rowid=search.rowid WHERE d.scope='shared' AND d.collection='texts' AND {condition} ORDER BY d.id LIMIT 50 OFFSET 0) SELECT d.payload FROM selected s JOIN docs d ON d.rowid=s.rid ORDER BY s.id"
        );
        assert!(!self.db.is_autocommit());
        let hits = self
            .db
            .prepare_cached(&sql)?
            .query_map(params_from_iter(&values), |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect::<Result<Vec<_>>>()?;
        let total = if hits.len() == 50 {
            MatchTotal::AtLeast(50)
        } else {
            MatchTotal::Exact(hits.len())
        };
        Ok(FirstPage {
            sequence: self.sequence,
            total,
            hits,
            validation: ValidationStatus::Verified,
        })
    }
}

fn main() -> Result<()> {
    cold_case::run("sqlite", || {
        let db = Connection::open_with_flags(
            "data/benchmark-work/sqlite/sqlite.db",
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        db.execute_batch("PRAGMA query_only=ON; PRAGMA cache_size=-2048; PRAGMA mmap_size=0; PRAGMA temp_store=FILE; PRAGMA synchronous=FULL;")?;
        assert_eq!(
            db.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))?,
            "wal"
        );
        db.set_prepared_statement_cache_capacity(64);
        assert!(db.is_autocommit());
        db.execute_batch("BEGIN")?;
        let sequence =
            u64::try_from(db.query_row("SELECT sequence FROM meta", [], |r| r.get::<_, i64>(0))?)?;
        Ok(Reader { db, sequence })
    })
}
