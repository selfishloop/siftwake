#[path = "../tools/bench_support/churn.rs"]
mod churn;
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
        let before = serde_json::to_value(self.0.stats()?)?;
        self.0.flush()?;
        let mut completed = usize::from(self.0.wait_for_compaction()?);
        while self.0.start_compaction()? {
            completed += usize::from(self.0.wait_for_compaction()?);
        }
        Ok(json!({"before":before,"after":self.0.stats()?,"merge_completions":completed}))
    }
    fn start_merge(&mut self) -> Result<bool> {
        Ok(self.0.start_compaction()?)
    }
    fn stats(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.0.stats()?)?)
    }
}
fn main() -> Result<()> {
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
    churn::emit(
        json!({"phase":"opened","engine":"fdb","mode":mode,"api":api,"open_ms":start.elapsed().as_secs_f64()*1000.,"strict":true,"memory":churn::memory(),"stats":engine.stats()?}),
    );
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
