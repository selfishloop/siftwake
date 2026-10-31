//! Explicit idle catch-up after responsive rewrite cancellation; same donor corpus.
use super::{Engine, churn};
use churn::{Backend, Result};
use serde_json::json;
use std::time::{Duration, Instant};
pub fn run(engine: &mut Engine, state: &str) -> Result<()> {
    let mut corpus = churn::Corpus::open()?;
    let mut model = churn::Model::load(state)?;
    engine.0.set_revision_reclamation(false);
    engine.maintain()?;
    let setup = churn::plan(&mut model, "update", 1100, 991);
    for chunk in setup.chunks(100) {
        let seq = engine.commit(&mut corpus, chunk, true)?;
        churn::apply(&mut model, chunk, seq);
    }
    engine.0.wait_for_compaction()?;
    let before = engine.0.segment_liveness()?;
    let mut rounds = vec![];
    for salt in 0..3 {
        if !engine.0.start_revision_reclamation()? {
            break;
        }
        let began = Instant::now();
        while engine.0.revision_reclamation_running() && began.elapsed() < Duration::from_millis(50)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        let running = engine.0.revision_reclamation_running();
        let group = churn::plan(&mut model, "update", 100, 1001 + salt * 100);
        let at = Instant::now();
        let seq = engine.commit(&mut corpus, &group, true)?;
        churn::apply(&mut model, &group, seq);
        rounds.push(json!({"rewrite_running_at_write":running,"write_ms":at.elapsed().as_secs_f64()*1000.,"sequence":seq}));
        engine.0.wait_for_compaction()?;
    }
    model.save(state)?;
    churn::verify_all(engine, &mut corpus, &model)?;
    let dirty = engine.0.segment_liveness()?;
    std::thread::sleep(Duration::from_secs(1));
    let idle = Instant::now();
    engine.0.set_revision_reclamation(true);
    let maintenance = engine.maintain()?;
    let idle_ms = idle.elapsed().as_secs_f64() * 1000.;
    churn::verify_all(engine, &mut corpus, &model)?;
    model.save(state)?;
    churn::emit(
        json!({"phase":"small_catchup","before":before,"after_writes":dirty,"after_idle":engine.0.segment_liveness()?,"writes":rounds,"idle_delay_ms":1000,"idle_maintenance_ms":idle_ms,"maintenance":maintenance,"memory":churn::memory(),"oracle":true,"scope":"explicit maintenance after one idle second; no autonomous scheduling/time bound;100-op writes can preempt revision work;small WAL calls do not drain it"}),
    );
    Ok(())
}

pub fn chunks(engine: &mut Engine, state: &str) -> Result<()> {
    let mut corpus = churn::Corpus::open()?;
    let mut model = churn::Model::load(state)?;
    engine.0.set_revision_reclamation(false);
    engine.maintain()?;
    let changes = churn::plan(&mut model, "update", 5000, 991);
    let before = engine.stats()?;
    let at = Instant::now();
    let atomic = engine.commit(&mut corpus, &changes, true);
    let atomic_ms = at.elapsed().as_secs_f64() * 1000.;
    assert!(
        atomic.is_err(),
        "This existing-corpus5k workload must exercise admission rejection"
    );
    assert_eq!(engine.stats()?["sequence"], before["sequence"]);
    let error = atomic.unwrap_err().to_string();
    churn::verify_all(engine, &mut corpus, &model)?;
    let at = Instant::now();
    let report = engine.0.commit_streamed_chunks(
        changes
            .iter()
            .map(|change| churn::mutation(&mut corpus, change)),
    )?;
    let chunked_ms = at.elapsed().as_secs_f64() * 1000.;
    assert!(!report.atomic);
    assert_eq!(report.mutations, 5000);
    let mut start = 0;
    for (&seq, &count) in report.sequences.iter().zip(&report.chunk_sizes) {
        churn::apply(&mut model, &changes[start..start + count], seq);
        start += count;
    }
    assert_eq!(start, 5000);
    model.save(state)?;
    churn::verify_all(engine, &mut corpus, &model)?;
    churn::emit(
        json!({"phase":"small_chunks","atomic_rejected":true,"atomic_error":error,"atomic_ms":atomic_ms,"helper_atomic":report.atomic,"helper_chunks":report.sequences.len(),"chunk_sizes":report.chunk_sizes,"chunked_ms":chunked_ms,"memory":churn::memory(),"stats":engine.stats()?,"oracle":true}),
    );
    Ok(())
}
