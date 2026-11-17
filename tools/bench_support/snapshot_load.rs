//! Snapshot publication/reader benchmark; all bodies use the existing corpus.
use super::{Engine, churn};
use churn::{Backend, Result};
use serde_json::json;
use std::time::Instant;
pub fn pilot(engine: &mut Engine, state: &str) -> Result<()> {
    let mut corpus = churn::Corpus::open()?;
    let mut model = churn::Model::load(state)?;
    let ids: Vec<_> = model.rows.keys().take(32).copied().collect();
    for enabled in [false, true] {
        let start = Instant::now();
        let reader = if enabled {
            Some(engine.0.snapshot_reader()?)
        } else {
            None
        };
        let initial_ms = start.elapsed().as_secs_f64() * 1000.;
        let mut rows = vec![];
        for id in &ids {
            let old = model.rows[id].clone();
            let mut new = old.clone();
            new.revision += 1;
            let changes = vec![churn::Change {
                id: *id,
                old: Some(old),
                new: Some(new),
                kind: "update".into(),
            }];
            let at = Instant::now();
            let seq = engine.commit(&mut corpus, &changes, true)?;
            let commit_ms = at.elapsed().as_secs_f64() * 1000.;
            churn::apply(&mut model, &changes, seq);
            if let Some(reader) = &reader {
                let (snapshot, a) = reader.acquire_timed()?;
                assert_eq!(snapshot.sequence(), seq);
                assert_eq!(
                    snapshot.get(&churn::key(*id))?,
                    Some(siftwake::StoredDocument {
                        document: corpus.document(*id, &model.rows[id])?,
                        version: seq
                    })
                );
                let released = snapshot.release();
                assert!(released.cleanup_error.is_none());
                rows.push(json!({"commit_ms":commit_ms,"admission":a,"release":released}));
            } else {
                rows.push(json!({"commit_ms":commit_ms}));
            }
        }
        model.save(state)?;
        churn::emit(
            json!({"phase":"snapshot_pilot","enabled":enabled,"initial_ms":initial_ms,"rows":rows,"publication":reader.map(|r|r.publication_stats()),"memory":churn::memory(),"oracle":true,"limitation":"one calibration order, not an acceptance A/B"}),
        );
    }
    Ok(())
}
