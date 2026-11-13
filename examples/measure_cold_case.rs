#[allow(dead_code)]
#[path = "../tools/bench_support/churn.rs"]
mod churn;
#[path = "../tools/bench_support/cold_case.rs"]
mod cold_case;
use churn::Result;
use fdb::{Database, FirstPage, Key, Query, StoredDocument};

impl cold_case::Reader for Database {
    fn get(&mut self, key: &Key) -> Result<Option<StoredDocument>> {
        Ok(Database::get(self, key)?)
    }

    fn first(&mut self, q: &Query) -> Result<FirstPage> {
        Ok(self.search_first_hybrid(q, 200)?)
    }
}

fn main() -> Result<()> {
    cold_case::run("fdb", || {
        let mut db = Database::open("data/benchmark-work/fdb")?;
        db.set_streamed_wal_layer(true)?;
        Ok(db)
    })
}
