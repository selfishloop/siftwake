//! Separate matched-idle/load probes. Only the existing corpus supplies bodies.
use super::{Engine, churn};
use churn::{Backend, Result};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn files(path: &str) -> Result<BTreeSet<String>> {
    Ok(fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect())
}
fn prepare(
    engine: &mut Engine,
    corpus: &mut churn::Corpus,
    model: &mut churn::Model,
    state: &str,
    salt: usize,
) -> Result<()> {
    let changes = churn::plan(model, "mixed", 2000, salt);
    for chunk in changes.chunks(100) {
        let row = churn::measured_commit(engine, corpus, model, chunk, true)?;
        assert_eq!(row["ok"], true);
    }
    model.save(state)?;
    engine.0.wait_for_compaction()?;
    Ok(())
}
fn schedule(engine: &mut Engine, path: &str) -> Result<Value> {
    engine.0.wait_for_compaction()?;
    let before = files(path)?;
    let began = Instant::now();
    if !engine.0.start_revision_reclamation()? {
        return Ok(json!({"scheduled":false}));
    }
    let scheduled_ms = began.elapsed().as_secs_f64() * 1000.;
    let at = Instant::now();
    let mut observed = None;
    while engine.0.revision_reclamation_running() && at.elapsed() < Duration::from_secs(5) {
        for name in files(path)?
            .difference(&before)
            .filter(|n| n.starts_with("run-") && n.ends_with(".fdb"))
        {
            if let Ok(file) = fs::File::open(Path::new(path).join(name))
                && file.metadata()?.len() >= 256 * 1024
            {
                observed = Some(name.clone());
                break;
            }
        }
        if observed.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(
        json!({"scheduled":true,"schedule_ms":scheduled_ms,"wait_for_live_copy_ms":at.elapsed().as_secs_f64()*1000.,"source_copy_witness":observed,"active":engine.0.revision_reclamation_running()}),
    )
}
pub fn run(engine: &mut Engine, state: &str, path: &str) -> Result<()> {
    let mut corpus = churn::Corpus::open()?;
    let mut model = churn::Model::load(state)?;
    let mut preparations = 0;
    prepare(engine, &mut corpus, &mut model, state, 337)?;
    preparations += 1;
    let mut writes = Vec::new();
    let mut valid = 0;
    for attempt in 0..96 {
        let observation = schedule(engine, path)?;
        if observation["scheduled"] != true {
            prepare(engine, &mut corpus, &mut model, state, 337 + attempt)?;
            preparations += 1;
            continue;
        }
        let active =
            observation["active"] == true && observation["source_copy_witness"].is_string();
        let changes = churn::plan(&mut model, "update", 1, 991 + attempt);
        let measured = churn::measured_commit(engine, &mut corpus, &mut model, &changes, true)?;
        assert_eq!(measured["ok"], true);
        valid += usize::from(active);
        writes.push(json!({"attempt":attempt,"observation":observation,"result":measured}));
        model.save(state)?;
        if valid == 32 {
            break;
        }
    }
    churn::emit(
        json!({"phase":"cost_write_load","active_samples":valid,"preparations":preparations,"samples":writes,"oracle":true,"memory":churn::memory()}),
    );
    assert_eq!(valid, 32, "need 32 actual in-flight source-copy samples");
    let cases = corpus.index.cases.clone();
    let mut reads = Vec::new();
    for repeat in 0..10 {
        for (case_id, case) in cases.iter().enumerate() {
            let mut observed = false;
            for retry in 0..8 {
                engine.0.wait_for_compaction()?;
                // Equal warm-cache conditions: commits invalidate visibility
                // caches. The first query must not be used as the idle baseline.
                let warm = engine.search(&case.text, case.offset, case.limit)?;
                churn::check_page(&mut corpus, &model, case, &warm)?;
                let at = Instant::now();
                let idle = engine.search(&case.text, case.offset, case.limit)?;
                let idle_ms = at.elapsed().as_secs_f64() * 1000.;
                churn::check_page(&mut corpus, &model, case, &idle)?;
                let observation = schedule(engine, path)?;
                let active =
                    observation["active"] == true && observation["source_copy_witness"].is_string();
                let at = Instant::now();
                let page = engine.search(&case.text, case.offset, case.limit)?;
                let loaded_ms = at.elapsed().as_secs_f64() * 1000.;
                churn::check_page(&mut corpus, &model, case, &page)?;
                reads.push(json!({"case_id":case_id,"repeat":repeat,"retry":retry,"active":active,"idle_ms":idle_ms,"loaded_ms":loaded_ms,"observation":observation,"oracle":true}));
                // A real single update between pairs preempts the worker and
                // publishes/reclaims its abandoned files. Neither query timer
                // includes this writer. There is no foreground writer thread.
                let changes = churn::plan(&mut model, "update", 1, repeat * 101 + case_id);
                let row = churn::measured_commit(engine, &mut corpus, &mut model, &changes, true)?;
                assert_eq!(row["ok"], true);
                model.save(state)?;
                if active {
                    observed = true;
                    break;
                }
                prepare(
                    engine,
                    &mut corpus,
                    &mut model,
                    state,
                    repeat * 113 + case_id + retry,
                )?;
                preparations += 1;
            }
            assert!(observed, "missing active read sample case {case_id}");
        }
    }
    churn::verify_all(engine, &mut corpus, &model)?;
    churn::emit(
        json!({"phase":"cost_read_load","samples":reads,"preparations":preparations,"oracle":true,"memory":churn::memory()}),
    );
    Ok(())
}

pub fn kill(engine: &mut Engine, mode: &str, state: &str, path: &str) -> Result<()> {
    let mut corpus = churn::Corpus::open()?;
    let mut model = churn::Model::load(state)?;
    if mode.starts_with("kill-retire") {
        // Drain ordinary merge candidates first. Otherwise four similar small
        // inputs can automatically merge our two setup runs before the barrier,
        // leaving no entirely shadowed source to exercise retirement.
        engine.0.set_revision_reclamation(false);
        engine.0.wait_for_compaction()?;
        while engine.0.start_compaction()? {
            engine.0.wait_for_compaction()?;
        }
        let first = churn::plan(&mut model, "update", 100, 337);
        let seq = engine.commit(&mut corpus, &first, true)?;
        churn::apply(&mut model, &first, seq);
        let second: Vec<_> = first
            .iter()
            .map(|c| {
                let old = model.rows[&c.id].clone();
                let mut new = old.clone();
                new.body = (new.body + 997) % 10000;
                new.revision += 1;
                churn::Change {
                    id: c.id,
                    old: Some(old),
                    new: Some(new),
                    kind: "update".into(),
                }
            })
            .collect();
        fs::write(format!("{state}.pending"), serde_json::to_vec(&second)?)?;
        let seq = engine.commit(&mut corpus, &second, true)?;
        churn::apply(&mut model, &second, seq);
        model.save(state)?;
        engine.0.wait_for_compaction()?;
        let eligible: Vec<_> = engine
            .0
            .segment_liveness()?
            .into_iter()
            .filter(|s| {
                s.live_records == 0 && s.tombstones == 0 && s.dead_payload_bytes >= 64 * 1024
            })
            .collect();
        assert!(!eligible.is_empty());
        let before = engine.stats()?;
        assert!(engine.0.start_revision_reclamation()?);
        if mode == "kill-retire-after" {
            engine.0.wait_for_compaction()?;
        }
        churn::emit(
            json!({"phase":"retire_barrier","installed":mode=="kill-retire-after","eligible":eligible,"before":before,"after":engine.stats()?,"note":"worker never publishes; child parks before install or after completed install"}),
        );
        loop {
            std::thread::park();
        }
    }
    prepare(engine, &mut corpus, &mut model, state, 337)?;
    let mut observation = schedule(engine, path)?;
    for salt in 1..5 {
        if observation["active"] == true && observation["source_copy_witness"].is_string() {
            break;
        }
        prepare(engine, &mut corpus, &mut model, state, 337 + salt)?;
        observation = schedule(engine, path)?;
    }
    assert_eq!(observation["active"], true);
    let changes = churn::plan(&mut model, "mixed", 100, 991);
    fs::write(format!("{state}.pending"), serde_json::to_vec(&changes)?)?;
    let mut at = 0;
    engine.0.commit_streamed(changes.iter().map(|c| {
        if at==50 {churn::emit(json!({"phase":"cancel_iterator_barrier","old_sequence":model.sequence,"prior_worker":observation}));loop{std::thread::park();}}
        at+=1;churn::mutation(&mut corpus,c)
    }))?;
    Err("kill child unexpectedly continued".into())
}
