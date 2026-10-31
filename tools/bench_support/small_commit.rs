//! Existing-corpus small writes and dirty500; no corpus generation.
use super::{
    churn,
    snapshot_common::{Client, Factory, View, Writer},
};
use churn::Result;
use serde_json::json;
use std::time::{Duration, Instant};
pub fn run<W: Writer>(engine: &mut W, state: &str, mode: &str) -> Result<()> {
    let mut corpus = churn::Corpus::open()?;
    let mut model = churn::Model::load(state)?;
    if mode == "small-open" {
        std::thread::sleep(Duration::from_secs(5));
        churn::emit(json!({"phase":"small_idle","memory":churn::memory(),"stats":engine.stats()?}));
        churn::verify_all(engine, &mut corpus, &model)?;
        churn::emit(json!({"phase":"small_verified","oracle":true}));
        return Ok(());
    }
    engine.maintain()?;
    let factory = engine.factory()?;
    let mut client = factory.open()?;
    let changes = churn::plan(
        &mut model,
        if mode == "small-dirty" {
            "update"
        } else {
            "mixed"
        },
        if mode == "small-dirty" {
            500
        } else if mode == "small-boundary" {
            std::env::var("FDB_SMALL_OPERATIONS")
                .ok()
                .map(|n| n.parse().expect("FDB_SMALL_OPERATIONS must be an integer"))
                .unwrap_or(768)
        } else {
            96
        },
        991,
    );
    let mut records = vec![];
    for group in changes.chunks(if mode == "small-dirty" { 20 } else { 1 }) {
        engine.reset_timing();
        let start = Instant::now();
        let seq = engine.commit(&mut corpus, group, true)?;
        let commit_ms = start.elapsed().as_secs_f64() * 1000.;
        let timing = engine.take_timing();
        churn::apply(&mut model, group, seq);
        let (view, admission) = client.acquire()?;
        assert_eq!(view.sequence(), seq);
        for c in group {
            let mut q = fdb::Query::new("shared");
            q.collection = Some("texts".into());
            if let Some(new) = &c.new {
                q.text = churn::marker(c.id, new.revision);
                assert_eq!(view.search(&q)?.0.total, 1);
            }
            if let Some(old) = &c.old {
                q.text = churn::marker(c.id, old.revision);
                assert_eq!(view.search(&q)?.0.total, 0);
            }
        }
        let visibility_ms = start.elapsed().as_secs_f64() * 1000.;
        let release = view.release()?;
        assert!(release.cleanup_error.is_none());
        records.push(json!({"kind":group[0].kind,"count":group.len(),"sequence":seq,"commit_ms":commit_ms,"write_to_verified_visibility_ms":visibility_ms,"admission":admission,"timing":timing,"oracle":true}));
    }
    model.save(state)?;
    churn::verify_all(engine, &mut corpus, &model)?;
    churn::emit(
        json!({"phase":mode,"records":records,"stats":engine.stats()?,"publication":engine.publication(),"memory":churn::memory(),"oracle":true}),
    );
    Ok(())
}
