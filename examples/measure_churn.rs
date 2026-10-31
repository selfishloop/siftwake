#[path = "../tools/bench_support/block2_scale.rs"]
mod block2_scale;
#[path = "../tools/bench_support/churn.rs"]
mod churn;
#[path = "../tools/bench_support/mask_only.rs"]
mod mask_only;
#[path = "../tools/bench_support/revision_cost.rs"]
mod revision_cost;
#[path = "../tools/bench_support/revision_search.rs"]
mod revision_search;
#[path = "../tools/bench_support/small_commit.rs"]
mod small_commit;
#[path = "../tools/bench_support/small_maintenance.rs"]
mod small_maintenance;
#[path = "../tools/bench_support/snapshot_common.rs"]
mod snapshot_common;
#[path = "../tools/bench_support/snapshot_load.rs"]
mod snapshot_load;
#[path = "../tools/bench_support/write50.rs"]
mod write50;
#[path = "../tools/bench_support/write_hardening.rs"]
mod write_hardening;
use churn::{Backend, Change, Corpus, Result};
use fdb::{Database, Query, SearchPage, StoredDocument};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};
struct Engine(Database);
fn long_cancel_probe(engine: &Engine, state: &str) -> Result<()> {
    let mut corpus = Corpus::open()?;
    let model = churn::Model::load(state)?;
    churn::verify_all(engine, &mut corpus, &model)?;
    let ids: Vec<usize> = std::env::var("FDB_CANCEL_LONG_IDS")?
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()?;
    for id in ids {
        let row = &model.rows[&id];
        let doc = corpus.document(id, row)?;
        let bytes = doc.text.len();
        assert_eq!(
            doc.fields.get("bytes").and_then(Value::as_u64),
            Some(bytes as u64)
        );
        assert_eq!(
            corpus
                .index
                .entries
                .iter()
                .filter(|e| e.text_bytes == bytes)
                .count(),
            1
        );
        // Exact numeric field selects one original long document. A two-scalar
        // substring has no trigram rejection and reaches payload verification.
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.text = "zx".into();
        q.exact.insert("bytes".into(), json!(bytes));
        let mut identity = q.clone();
        identity.text.clear();
        let only = engine.0.search(&identity)?;
        assert_eq!(only.total, 1);
        assert_eq!(only.hits[0].document.key, doc.key);
        let expected = engine.0.search(&q)?;
        for repeat in 0..40 {
            for variant in ["serial", "four", "hybrid", "exact-four"] {
                let token = fdb::CancelToken::observed();
                let (status, latency, progress) = std::thread::scope(|scope| -> Result<_> {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let q = &q;
                    let t = &token;
                    let worker = scope.spawn(move || {
                        let e = fdb::SearchExecution {
                            verification_workers: if variant == "serial" { 1 } else { 4 },
                            ..Default::default()
                        };
                        let result = if variant == "hybrid" {
                            engine
                                .0
                                .search_first_hybrid_cancellable(q, 200, t)
                                .map(|p| p.hits)
                        } else if variant == "exact-four" {
                            engine.0.search_cancellable(q, e, t).map(|p| p.hits)
                        } else {
                            engine.0.search_first_cancellable(q, e, t).map(|p| p.hits)
                        };
                        let _ = tx.send(());
                        result
                    });
                    let began = Instant::now();
                    let mut finished = false;
                    while token.candidates_started().unwrap() == 0 {
                        if rx.try_recv().is_ok() {
                            finished = true;
                            break;
                        }
                        if began.elapsed() > std::time::Duration::from_secs(10) {
                            token.cancel();
                            break;
                        }
                        std::thread::yield_now();
                    }
                    let progress = token.candidates_started().unwrap();
                    let at = Instant::now();
                    if !finished {
                        token.cancel();
                    }
                    let result = worker.join().map_err(|_| "long cancel worker panicked")?;
                    let elapsed = at.elapsed().as_secs_f64() * 1000.;
                    match result {
                        Err(fdb::Error::Cancelled) => {
                            assert_eq!(progress, 1);
                            Ok(("cancelled", Some(elapsed), progress))
                        }
                        Ok(hits) => {
                            assert_eq!(hits, expected.hits);
                            Ok((
                                if finished {
                                    "completed_before_cancel"
                                } else {
                                    "completed_at_cancel_race"
                                },
                                None,
                                progress,
                            ))
                        }
                        Err(e) => Err(e.into()),
                    }
                })?;
                churn::emit(
                    json!({"phase":"long_cancel","record_id":id,"text_bytes":bytes,"variant":variant,"repeat":repeat,"status":status,"cancel_to_query_threads_joined_ms":latency,"candidates_at_request":progress,"payload_identity":"exact bytes field uniquely selects this original document; zx cannot reject via a trigram","oracle":true,"cache":"warm verified source, synchronous read not force-aborted"}),
                );
            }
        }
    }
    engine.0.check_integrity()?;
    churn::emit(json!({"phase":"long_cancel_finished","healthy_after_cancel":true,"oracle":true}));
    Ok(())
}
fn cancel_probe(engine: &Engine, state: &str) -> Result<()> {
    let mut corpus = Corpus::open()?;
    let model = churn::Model::load(state)?;
    churn::verify_all(engine, &mut corpus, &model)?;
    let queries: Vec<String> = std::env::var("FDB_CANCEL_QUERIES")
        .ok()
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_else(|| vec!["zx".into(), "the zx".into(), "transubstantiation".into()]);
    for text in queries {
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.text = text.clone();
        let expected = engine.0.search(&q)?;
        for target in [32u64, 256] {
            for repeat in 0..10 {
                for variant in ["serial", "four", "hybrid", "exact-four"] {
                    let token = fdb::CancelToken::observed();
                    let began = Instant::now();
                    let cpu = churn::cpu_ms();
                    let (status, latency, progress) = std::thread::scope(|scope| -> Result<_> {
                        let (tx, rx) = std::sync::mpsc::channel();
                        let t = &token;
                        let q = &q;
                        let worker = scope.spawn(move || {
                            let e = fdb::SearchExecution {
                                verification_workers: if variant == "serial" { 1 } else { 4 },
                                ..Default::default()
                            };
                            let result = if variant == "hybrid" {
                                engine
                                    .0
                                    .search_first_hybrid_cancellable(q, 200, t)
                                    .map(|p| p.hits)
                            } else if variant == "exact-four" {
                                engine.0.search_cancellable(q, e, t).map(|p| p.hits)
                            } else {
                                engine.0.search_first_cancellable(q, e, t).map(|p| p.hits)
                            };
                            let _ = tx.send(());
                            result
                        });
                        let mut finished = false;
                        while token.candidates_started().unwrap() < target {
                            if rx.try_recv().is_ok() {
                                finished = true;
                                break;
                            }
                            if began.elapsed() > std::time::Duration::from_secs(10) {
                                token.cancel();
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_micros(100));
                        }
                        let progress = token.candidates_started().unwrap();
                        let cancelled_at = Instant::now();
                        if !finished {
                            token.cancel();
                        }
                        let result = worker.join().map_err(|_| "cancel probe query panicked")?;
                        let latency = cancelled_at.elapsed().as_secs_f64() * 1000.;
                        match result {
                            Err(fdb::Error::Cancelled) => {
                                Ok(("cancelled", Some(latency), progress))
                            }
                            Ok(hits) => {
                                assert_eq!(hits, expected.hits);
                                Ok((
                                    if finished {
                                        "completed_before_cancel"
                                    } else {
                                        "completed_at_cancel_race"
                                    },
                                    None,
                                    progress,
                                ))
                            }
                            Err(e) => Err(e.into()),
                        }
                    })?;
                    let cpu_ms = cpu.zip(churn::cpu_ms()).map(|(a, b)| b - a);
                    churn::emit(
                        json!({"phase":"cancel_probe","text":text,"variant":variant,"repeat":repeat,"target_candidates":target,"candidates_at_request":progress,"status":status,"cancel_to_all_threads_joined_ms":latency,"total_call_ms":began.elapsed().as_secs_f64()*1000.,"cpu_ms":cpu_ms,"oracle":true,"instrumentation":"observed token adds shared candidate-start counter; only cancellation probe uses it"}),
                    );
                }
            }
        }
    }
    engine.0.check_integrity()?;
    churn::emit(json!({"phase":"cancel_finished","healthy_after_cancel":true,"oracle":true}));
    Ok(())
}
#[derive(Clone)]
struct BenchFactory(fdb::SnapshotReader);
struct BenchClient(fdb::SnapshotReader);
struct BenchView(fdb::Snapshot);
impl snapshot_common::Factory for BenchFactory {
    type Client = BenchClient;
    fn open(&self) -> Result<Self::Client> {
        Ok(BenchClient(self.0.clone()))
    }
}
impl snapshot_common::Client for BenchClient {
    type View = BenchView;
    fn acquire(&mut self) -> Result<(Self::View, fdb::SnapshotAdmission)> {
        let (v, a) = self.0.acquire_timed()?;
        Ok((BenchView(v), a))
    }
}
impl snapshot_common::View for BenchView {
    fn sequence(&self) -> u64 {
        self.0.sequence()
    }
    fn get(&self, id: usize) -> Result<Option<StoredDocument>> {
        Ok(self.0.get(&churn::key(id))?)
    }
    fn search(&self, q: &Query) -> Result<(SearchPage, fdb::SnapshotReadWait)> {
        Ok(self.0.search_with_waits(q)?)
    }
    fn first(&self, q: &Query) -> Result<fdb::FirstPage> {
        let _reference = fdb::Database::hybrid_wait_reference_for_diagnostics(
            std::env::var_os("FDB_HYBRID_WAIT_REFERENCE").is_some(),
        );
        #[cfg(feature = "diagnostic-timing")]
        let _limit = fdb::timing::request::WorkerLimit::new(
            std::env::var("FDB_HYBRID_WORKERS")
                .ok()
                .map(|s| s.parse::<usize>())
                .transpose()?
                .unwrap_or(4),
        );
        Ok(self.0.search_first_hybrid(q, 200)?)
    }
    fn first_profiled(&self, q: &Query) -> Result<(fdb::FirstPage, Value)> {
        #[cfg(feature = "diagnostic-timing")]
        if std::env::var_os("FDB_QUERY_PROFILE").is_some() {
            let request = fdb::timing::request::Request::new();
            let page = self.first(q)?;
            return Ok((page, json!(request.finish())));
        }
        Ok((self.first(q)?, Value::Null))
    }
    fn release(self) -> Result<fdb::SnapshotRelease> {
        Ok(self.0.release())
    }
}
impl snapshot_common::Writer for Engine {
    fn begin_write_profile(&mut self) -> Result<()> {
        if std::env::var_os("FDB_SEQUENCE_PROFILE").is_some() {
            let capacity = std::env::var("FDB_WRITE_PROFILE_CAPACITY")
                .ok()
                .map(|s| s.parse::<usize>())
                .transpose()?
                .unwrap_or(8192);
            self.0.start_write_profile(capacity)?;
        }
        Ok(())
    }
    fn finish_write_profile(&mut self) -> Value {
        self.0
            .take_write_profile()
            .map_or(Value::Null, |trace| json!(trace))
    }
    fn reset_timing(&self) {
        #[cfg(feature = "diagnostic-timing")]
        let _ = fdb::timing::take();
    }
    fn take_timing(&self) -> Value {
        #[cfg(feature = "diagnostic-timing")]
        {
            json!(fdb::timing::take())
        }
        #[cfg(not(feature = "diagnostic-timing"))]
        {
            Value::Null
        }
    }
    type Factory = BenchFactory;
    fn factory(&mut self) -> Result<Self::Factory> {
        Ok(BenchFactory(self.0.snapshot_reader()?))
    }
    fn publication(&self) -> Value {
        self.0
            .snapshot_publication_stats()
            .map_or(Value::Null, |s| json!(s))
    }
    fn interlude(&mut self) -> Result<Value> {
        let mut normal = usize::from(self.0.wait_for_compaction()?);
        for _ in 0..32 {
            if !self.0.start_compaction()? {
                break;
            }
            normal += usize::from(self.0.wait_for_compaction()?);
        }
        let mut rewrites = 0;
        let mut retirements = 0;
        let mut jobs = vec![];
        for _ in 0..16 {
            let before = self.0.segment_liveness()?;
            if !self.0.start_revision_reclamation()? {
                break;
            }
            let running = self.0.revision_reclamation_running();
            let installed = self.0.wait_for_compaction()?;
            let after = self.0.segment_liveness()?;
            let rewrite = after
                .iter()
                .any(|s| !before.iter().any(|b| b.source_id == s.source_id));
            rewrites += usize::from(installed && rewrite);
            retirements += usize::from(installed && !rewrite);
            jobs.push(json!({"before":before,"after":after,"running":running,"installed":installed,"rewrite":rewrite}));
            if rewrites != 0 {
                break;
            }
        }
        Ok(
            json!({"normal_merges_completed":normal,"revision_rewrites_completed":rewrites,"retirements":retirements,"jobs":jobs}),
        )
    }
}
impl Backend for Engine {
    fn query_variant(&self, text: &str, variant: &str) -> Result<fdb::FirstPage> {
        let _reference = fdb::Database::hybrid_wait_reference_for_diagnostics(
            std::env::var_os("FDB_HYBRID_WAIT_REFERENCE").is_some(),
        );
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.text = text.into();
        if variant.starts_with("cap-hybrid") {
            return Ok(if variant.contains("cancel") {
                self.0
                    .search_first_hybrid_cancellable(&q, 200, &fdb::CancelToken::new())?
            } else {
                self.0.search_first_hybrid(&q, 200)?
            });
        }
        if variant == "legacy" {
            let p = self.0.search(&q)?;
            return Ok(fdb::FirstPage {
                total: fdb::MatchTotal::Exact(p.total),
                sequence: p.sequence,
                hits: p.hits,
                validation: fdb::ValidationStatus::Verified,
            });
        }
        let mut e = fdb::SearchExecution::default();
        if variant.contains("four") {
            e.verification_workers = 4;
        }
        if variant.contains("two") {
            e.verification_workers = 2;
        }
        if variant.contains("join") {
            e.coalesce_gap_bytes = 8192;
        }
        if variant.contains("crc") {
            e.reuse_validated_source_crc = true;
        }
        if variant.contains("cancel") {
            return Ok(self
                .0
                .search_first_cancellable(&q, e, &fdb::CancelToken::new())?);
        }
        if variant.starts_with("cap") {
            Ok(self.0.search_first_with_execution(&q, e)?)
        } else {
            let p = self.0.search_with_execution(&q, e)?;
            Ok(fdb::FirstPage {
                total: fdb::MatchTotal::Exact(p.total),
                sequence: p.sequence,
                hits: p.hits,
                validation: fdb::ValidationStatus::Verified,
            })
        }
    }
    fn query_profile_reset(&self) {
        #[cfg(feature = "diagnostic-timing")]
        let _discard = fdb::timing::take();
    }
    fn query_profile_take(&self) -> Value {
        #[cfg(feature = "diagnostic-timing")]
        return json!(fdb::timing::take());
        #[cfg(not(feature = "diagnostic-timing"))]
        Value::Null
    }
    fn idle_notify(&mut self) -> Result<Value> {
        Ok(json!(self.0.maintain_idle()?))
    }
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
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|s| s == "mask-scale-prepare" || s == "mask-scale-pair")
    {
        if args.len() != 3 {
            return Err("mask-only MODE streamed STATE-unused".into());
        }
        return mask_only::run(&args[0], "data/benchmark-work/fdb");
    }
    churn::start_memory_sampler();
    if args.first().is_some_and(|a| a == "scale-catalog") {
        if args.len() != 2 {
            return Err("scale-catalog DESTINATION.json".into());
        }
        return Corpus::catalog(block2_scale::count()?, &args[1]);
    }
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
    if (mode == "seed" || mode == "scale-seed") && Path::new(path).exists() {
        return Err("seed requires ownership-checked reset".into());
    }
    if mode == "first-background" || mode == "first-strict" {
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.text = std::env::var("FDB_FIRST_QUERY")?;
        let started = Instant::now();
        let cpu = churn::cpu_ms();
        let (page, db, full_ms, session_ms) = if mode == "first-background" {
            let mut session = fdb::OpeningDatabase::open_for_search(path)?;
            let session_ms = started.elapsed().as_secs_f64() * 1000.;
            churn::emit(
                json!({"phase":"session_return","session_ms":session_ms,"status":session.validation_status()}),
            );
            let page = session.search_first(&q)?;
            let full = session.validation_elapsed_ms().unwrap();
            (page, session.try_finish()?.unwrap(), full, session_ms)
        } else {
            let engine = Engine(Database::open(path)?);
            let session_ms = started.elapsed().as_secs_f64() * 1000.;
            churn::emit(
                json!({"phase":"session_return","session_ms":session_ms,"status":"verified"}),
            );
            let variant = std::env::var("FDB_FIRST_VARIANT").unwrap_or_else(|_| "cap-four".into());
            let page = engine.query_variant(&q.text, &variant)?;
            (page, engine.0, session_ms, session_ms)
        };
        let first_ms = started.elapsed().as_secs_f64() * 1000.;
        let cpu_ms = cpu.zip(churn::cpu_ms()).map(|(a, b)| b - a);
        churn::emit(
            json!({"phase":"first_result","mode":mode,"variant":if mode=="first-background" {"cap-four".to_owned()} else {std::env::var("FDB_FIRST_VARIANT").unwrap_or_else(|_|"cap-four".into())},"text":q.text,"session_ms":session_ms,"first_ms":first_ms,"full_validation_ms":full_ms,"after_session_wait_and_search_ms":first_ms-session_ms,"cpu_ms":cpu_ms,"returned":page.hits.len(),"total":page.total,"validation":page.validation,"memory":churn::memory()}),
        );
        #[cfg(feature = "diagnostic-timing")]
        churn::emit(json!({"phase":"background_open_profile","counters":fdb::timing::take()}));
        let mut corpus = Corpus::open()?;
        let model = churn::Model::load(state)?;
        let case = corpus
            .index
            .cases
            .iter()
            .find(|c| c.text == q.text && c.offset == 0 && c.limit == 50)
            .unwrap()
            .clone();
        let exact = db.search(&q)?;
        churn::check_page(&mut corpus, &model, &case, &exact)?;
        assert_eq!(page.hits, exact.hits);
        assert_eq!(page.sequence, exact.sequence);
        churn::verify_all(&Engine(db), &mut corpus, &model)?;
        churn::emit(
            json!({"phase":"first_result_oracle","oracle":true,"model_sequence":model.sequence}),
        );
        return Ok(());
    }
    let start = Instant::now();
    let mut engine = Engine(Database::open(path)?);
    let _mask_reference = Database::snapshot_mask_reference_for_diagnostics(
        std::env::var_os("FDB_SNAPSHOT_MASK_REFERENCE").is_some(),
    );
    engine.0.set_streamed_wal_layer(
        mode.starts_with("small-") || std::env::var_os("FDB_STREAMED_WAL_LAYER").is_some(),
    )?;
    let _snapshot_reader = if std::env::var_os("FDB_BENCH_SNAPSHOTS").is_some() {
        Some(engine.0.snapshot_reader()?)
    } else {
        None
    };
    let _pinned_snapshot = _snapshot_reader.as_ref().map(|r| r.acquire()).transpose()?;
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
    if mode.starts_with("write50-") {
        return write50::run(&mut engine, state, mode);
    }
    if mode == "scale-cancel-probe" {
        return cancel_probe(&engine, state);
    }
    if mode == "scale-long-cancel" {
        return long_cancel_probe(&engine, state);
    }
    if mode == "open-profile" {
        #[cfg(feature = "diagnostic-timing")]
        churn::emit(
            json!({"phase":"strict_open_profile","counters":fdb::timing::take(),"memory":churn::memory(),"stats":engine.stats()?}),
        );
        #[cfg(not(feature = "diagnostic-timing"))]
        return Err("open-profile needs diagnostic-timing".into());
        #[cfg(feature = "diagnostic-timing")]
        return Ok(());
    }
    if mode.starts_with("scale-") {
        return block2_scale::run(&mut engine, mode, state, path);
    }
    if mode == "liveness" {
        churn::emit(
            json!({"phase":"liveness","segments":engine.liveness()?,"stats":engine.stats()?}),
        );
        return Ok(());
    }
    if mode == "write-churn-30" {
        return write_hardening::run(&mut engine, state, path);
    }
    if mode == "write-churn-idle-30" {
        return write_hardening::idle_each_round(&mut engine, state, path);
    }
    if mode == "write-idle-check" {
        let mut corpus = Corpus::open()?;
        let model = churn::Model::load(state)?;
        churn::verify_all(&engine, &mut corpus, &model)?;
        let reader = engine.0.snapshot_reader()?;
        let old = reader.acquire()?;
        let before = churn::disk(path)?;
        engine.0.maintain_idle()?;
        let while_pinned = churn::disk(path)?;
        for (&id, row) in &model.rows {
            assert_eq!(
                old.get(&churn::key(id))?,
                Some(StoredDocument {
                    document: corpus.document(id, row)?,
                    version: row.version,
                })
            );
        }
        for &id in model.dead.keys() {
            assert!(old.get(&churn::key(id))?.is_none());
        }
        let released = old.release();
        assert!(released.cleanup_error.is_none());
        churn::verify_all(&engine, &mut corpus, &model)?;
        churn::emit(
            json!({"phase":"write_idle_checked","before":before,"while_pinned":while_pinned,"after_release":churn::disk(path)?,"release":released,"oracle":true,"stats":engine.stats()?}),
        );
        return Ok(());
    }
    if mode == "snapshot-pilot" {
        return snapshot_load::pilot(&mut engine, state);
    }
    if mode == "small-catchup" {
        return small_maintenance::run(&mut engine, state);
    }
    if mode == "small-chunks" {
        return small_maintenance::chunks(&mut engine, state);
    }
    if mode.starts_with("small-") {
        engine.0.set_revision_reclamation(false);
        return small_commit::run(&mut engine, state, mode);
    }
    if mode == "snapshot-one" || mode == "snapshot-100" || mode.starts_with("snapshot-control-") {
        engine.0.set_revision_reclamation(false);
        return snapshot_common::run(&mut engine, state, mode);
    }
    if mode == "search-profile" || mode == "search-target-profile" || mode == "search-matrix" {
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
