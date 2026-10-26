//! Read-only probes on the current fixed slot; never prepares a corpus or DB.
use super::{Engine, churn};
#[cfg(feature = "diagnostic-timing")]
use churn::Backend;
use churn::Result;
#[cfg(feature = "diagnostic-timing")]
use serde_json::json;
#[cfg(feature = "diagnostic-timing")]
use std::time::Instant;

pub fn run(engine: &Engine, state: &str, mode: &str) -> Result<()> {
    let mut corpus = churn::Corpus::open()?;
    let model = churn::Model::load(state)?;
    if mode == "search-matrix" {
        churn::emit(churn::matrix(
            engine,
            &mut corpus,
            &model,
            30,
            "revision-search",
        )?);
        return Ok(());
    }
    #[cfg(not(feature = "diagnostic-timing"))]
    return Err("search-profile needs diagnostic-timing".into());
    #[cfg(feature = "diagnostic-timing")]
    {
        churn::verify_all(engine, &mut corpus, &model)?;
        churn::emit(
            json!({"phase":"search_profile_baseline","segments":engine.liveness()?,"stats":engine.stats()?}),
        );
        for (case_id, case) in corpus.index.cases.clone().iter().enumerate() {
            let page = engine.search(&case.text, case.offset, case.limit)?;
            churn::check_page(&mut corpus, &model, case, &page)?;
            for repeat in 0..5 {
                let _discard = fdb::timing::take();
                let at = Instant::now();
                let page = engine.search(&case.text, case.offset, case.limit)?;
                let elapsed_ms = at.elapsed().as_secs_f64() * 1000.;
                let counters = fdb::timing::take();
                churn::check_page(&mut corpus, &model, case, &page)?;
                churn::emit(
                    json!({"phase":"search_profile","case_id":case_id,"case":case,"repeat":repeat,"elapsed_ms":elapsed_ms,"counters":counters,"oracle":true}),
                );
            }
        }
        Ok(())
    }
}
