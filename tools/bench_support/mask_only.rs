//!100k setup without ingest/open/query/RAM timing; only the requested mask pair.
use super::churn::{self, Corpus, Result, Row};
use fdb::Database;
#[cfg(all(feature = "diagnostic-timing", feature = "bounded-engine"))]
use fdb::Key;

pub fn run(mode: &str, path: &str) -> Result<()> {
    if std::env::var("FDB_SCALE_DOCUMENTS")? != "100000" {
        return Err("mask-only requires100k".into());
    }
    if mode == "mask-scale-prepare" {
        let mut c = Corpus::open()?;
        let mut db = Database::open(path)?;
        db.set_streamed_wal_layer(true)?;
        for first in (0..100000).step_by(500) {
            db.commit_streamed((first..first + 500).map(|id| {
                let row = Row {
                    body: id,
                    revision: 0,
                    version: 0,
                };
                c.document(id, &row)
                    .map(|document| fdb::Mutation::Put {
                        document,
                        guard: fdb::Guard::Any,
                    })
                    .map_err(|e| fdb::Error::Invalid(e.to_string()))
            }))?;
            if (first + 500) % 10000 == 0 {
                churn::emit(
                    serde_json::json!({"phase":"mask_setup_progress","documents":first+500}),
                );
            }
        }
        db.maintain_idle()?;
        churn::emit(
            serde_json::json!({"phase":"mask_setup_complete","documents":100000,"not_an_ingest_benchmark":true}),
        );
        return Ok(());
    }
    if mode == "mask-scale-pair" {
        #[cfg(all(feature = "diagnostic-timing", feature = "bounded-engine"))]
        {
            let mut db = Database::open(path)?;
            let keys = (0..100)
                .map(|n| Key::new("shared", "texts", &format!("{n:010}")))
                .collect::<Vec<_>>();
            let pair = db.diagnostic_mask_pair(&keys)?;
            churn::emit(
                serde_json::json!({"phase":"mask_pair_100k","documents":100000,"sample":pair,"other_scale_measurements":false}),
            );
            return Ok(());
        }
        #[cfg(not(all(feature = "diagnostic-timing", feature = "bounded-engine")))]
        return Err("mask pair requires diagnostic bounded build".into());
    }
    Err("unknown mask-only mode".into())
}
