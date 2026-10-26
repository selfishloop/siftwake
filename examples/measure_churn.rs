#[path = "../tools/bench_support/churn.rs"]
mod churn;
#[path = "../tools/bench_support/revision_cost.rs"]
mod revision_cost;
#[path = "../tools/bench_support/revision_search.rs"]
mod revision_search;
use churn::{Backend, Change, Corpus, Result};
use fdb::{Database, Query, SearchPage, StoredDocument};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};
struct Engine(Database);
impl Backend for Engine {
    fn get(&self, id: usize) -> Result<Option<StoredDocument>> {
        Ok(self.0.get(&churn::key(id))?)
    }
    fn search(&self, text: &str, offset: usize, limit: usize) -> Result<SearchPage> {
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.text = text.into();
        q.offset = offset;
        q.limit = limit;
        Ok(self.0.search(&q)?)
    }
    fn commit(&mut self, corpus: &mut Corpus, changes: &[Change], streamed: bool) -> Result<u64> {
        let mutations = changes.iter().map(|c| churn::mutation(corpus, c));
        Ok(if streamed {
            self.0.commit_streamed(mutations)?
        } else {
            self.0.commit(mutations.collect::<fdb::Result<Vec<_>>>()?)?
        })
    }
    fn maintain(&mut self) -> Result<Value> {
        #[cfg(feature = "diagnostic-timing")]
        let window = fdb::timing::maintenance::window();
        #[cfg(feature = "diagnostic-timing")]
        let _discard = fdb::timing::take();
        let before = serde_json::to_value(self.0.stats()?)?;
        self.0.flush()?;
        let mut completed = usize::from(self.0.wait_for_compaction()?);
        while self.0.start_compaction()? {
            completed += usize::from(self.0.wait_for_compaction()?);
        }
        let mut result =
            json!({"before":before,"after":self.0.stats()?,"merge_completions":completed});
        #[cfg(feature = "diagnostic-timing")]
        {
            result["cost_profile"] = window.finish();
            result["cost_timing_counters"] = serde_json::to_value(fdb::timing::take())?;
        }
        Ok(result.take())
    }
    fn start_merge(&mut self) -> Result<bool> {
        Ok(self.0.start_compaction()?)
    }
    fn stats(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.0.stats()?)?)
    }
    fn liveness(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.0.segment_liveness()?)?)
    }
}
fn main() -> Result<()> {
    #[cfg(feature = "diagnostic-timing")]
    fdb::timing::maintenance::enable();
    churn::start_memory_sampler();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "prepare") {
        return Corpus::prepare();
    }
    if args.len() != 3 {
        return Err("measure_churn MODE streamed|owned STATE.json".into());
    }
    let mode = &args[0];
    let api = &args[1];
    let state = &args[2];
    let path = "data/benchmark-work/fdb";
    if mode == "seed" && Path::new(path).exists() {
        return Err("seed requires ownership-checked reset".into());
    }
    let start = Instant::now();
    let mut engine = Engine(Database::open(path)?);
    // Benchmark policy A/B is explicit, independent of product defaults.
    engine
        .0
        .set_revision_reclamation(std::env::var_os("FDB_RECLAMATION_CONTROL").is_none());
    engine
        .0
        .set_revision_maintenance_responsive(std::env::var_os("FDB_COST_LEGACY").is_none());
    engine
        .0
        .set_query_visibility_pruning(std::env::var_os("FDB_QUERY_VISIBILITY_PRUNING").is_some());
    churn::emit(
        json!({"phase":"opened","engine":"fdb","mode":mode,"api":api,"open_ms":start.elapsed().as_secs_f64()*1000.,"strict":true,"memory":churn::memory(),"stats":engine.stats()?}),
    );
    if mode == "liveness" {
        churn::emit(
            json!({"phase":"liveness","segments":engine.liveness()?,"stats":engine.stats()?}),
        );
        return Ok(());
    }
    if mode == "search-profile" || mode == "search-matrix" {
        return revision_search::run(&engine, state, mode);
    }
    if mode == "cost-load" {
        return revision_cost::run(&mut engine, state, path);
    }
    if mode.starts_with("kill-retire") || mode == "kill-cancel" {
        return revision_cost::kill(&mut engine, mode, state, path);
    }
    if mode == "revision-reads" || mode == "revision-write" {
        let mut corpus = Corpus::open()?;
        let mut model = churn::Model::load(state)?;
        let changes = churn::plan(&mut model, "mixed", 2000, 337);
        for chunk in changes.chunks(100) {
            let result = churn::measured_commit(&mut engine, &mut corpus, &mut model, chunk, true)?;
            assert_eq!(result["ok"], true);
        }
        model.save(state)?;
        engine.0.wait_for_compaction()?;
        if mode == "revision-write" {
            let one = churn::plan(&mut model, "update", 1, 991);
            let baseline =
                churn::measured_commit(&mut engine, &mut corpus, &mut model, &one, true)?;
            engine.0.wait_for_compaction()?;
            let at = Instant::now();
            assert!(engine.0.start_revision_reclamation()?);
            let scheduled = at.elapsed().as_secs_f64() * 1000.;
            let one = churn::plan(&mut model, "update", 1, 993);
            let under_worker =
                churn::measured_commit(&mut engine, &mut corpus, &mut model, &one, true)?;
            model.save(state)?;
            churn::verify_all(&engine, &mut corpus, &model)?;
            churn::emit(
                json!({"phase":"rewrite_write","baseline":baseline,"schedule_ms":scheduled,"under_worker":under_worker,"oracle":true}),
            );
            return Ok(());
        }
        churn::emit(churn::matrix(
            &engine,
            &mut corpus,
            &model,
            30,
            "before_worker",
        )?);
        let at = Instant::now();
        assert!(
            engine.0.start_revision_reclamation()?,
            "need eligible stale segment"
        );
        churn::emit(
            json!({"phase":"rewrite_admitted","schedule_ms":at.elapsed().as_secs_f64()*1000.,"liveness":engine.liveness()?}),
        );
        let mut active = true;
        let mut samples = Vec::new();
        let mut completions = 0;
        let cases = corpus.index.cases.clone();
        for repeat in 0..30 {
            for (case_id, case) in cases.iter().enumerate() {
                let at = Instant::now();
                let page = engine.search(&case.text, case.offset, case.limit)?;
                let ms = at.elapsed().as_secs_f64() * 1000.;
                // Publication only between queries. Every page must match the
                // same unchanged model, on either side of worker installation.
                samples.push(json!({"case_id":case_id,"repeat":repeat,"ms":ms,"job_pending_at_start":active}));
                churn::check_page(&mut corpus, &model, case, &page)?;
                if active && engine.0.poll_compaction()? {
                    completions += 1;
                    active = engine.0.start_revision_reclamation()?;
                }
            }
        }
        if active {
            completions += usize::from(engine.0.wait_for_compaction()?);
        }
        churn::verify_all(&engine, &mut corpus, &model)?;
        churn::emit(
            json!({"phase":"rewrite_reads","samples":samples,"completions":completions,"oracle":true,"memory":churn::memory()}),
        );
        churn::emit(churn::matrix(
            &engine,
            &mut corpus,
            &model,
            30,
            "after_worker",
        )?);
        return Ok(());
    }
    if mode == "kill-revisions" {
        let mut corpus = Corpus::open()?;
        let mut model = churn::Model::load(state)?;
        let changes = churn::plan(&mut model, "mixed", 2000, 337);
        std::fs::write(format!("{state}.pending"), serde_json::to_vec(&changes)?)?;
        for chunk in changes.chunks(100) {
            let sequence = engine.commit(&mut corpus, chunk, true)?;
            churn::apply(&mut model, chunk, sequence);
            model.save(state)?;
        }
        engine.0.wait_for_compaction()?;
        churn::emit(json!({"phase":"kill_acknowledged","sequence":model.sequence,"mode":mode}));
        if !engine.0.start_revision_reclamation()? {
            return Err("kill coverage requires an eligible stale segment".into());
        }
        churn::emit(json!({"phase":"merge_scheduled","stats":engine.stats()?,"reclamation":true}));
        loop {
            std::thread::park();
        }
    }
    if mode.starts_with("kill-") {
        let mut corpus = Corpus::open()?;
        let mut model = churn::Model::load(state)?;
        let changes = churn::plan(&mut model, "mixed", 100, 337);
        std::fs::write(format!("{state}.pending"), serde_json::to_vec(&changes)?)?;
        if mode == "kill-iterator" {
            let mut at = 0;
            engine.0.commit_streamed(changes.iter().map(|c|{if at==50{churn::emit(json!({"phase":"kill_iterator_barrier","yielded":50,"old_sequence":model.sequence}));loop{std::thread::park();}}at+=1;churn::mutation(&mut corpus,c)}))?;
            return Err("kill child unexpectedly continued".into());
        }
        let sequence = engine.commit(&mut corpus, &changes, false)?;
        churn::apply(&mut model, &changes, sequence);
        model.save(state)?;
        churn::emit(json!({"phase":"kill_acknowledged","sequence":sequence,"mode":mode}));
        if mode == "kill-flush" {
            churn::emit(json!({"phase":"flush_start"}));
            engine.0.flush()?;
            churn::emit(json!({"phase":"flush_end"}));
        }
        if mode == "kill-merge" {
            engine.0.flush()?;
            engine.0.wait_for_compaction()?;
            if !engine.start_merge()? {
                let extra = churn::plan(&mut model, "mixed", 100, 381);
                let sequence = engine.commit(&mut corpus, &extra, true)?;
                churn::apply(&mut model, &extra, sequence);
                model.save(state)?;
                let _ = engine.start_merge()?;
            }
            churn::emit(json!({"phase":"merge_scheduled","stats":engine.stats()?}));
        }
        loop {
            std::thread::park();
        }
    }
    churn::run(engine, mode, api, state, path)
}
