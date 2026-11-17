//! Standalone measurement adapter; does not change the engine or main collection.
#[path = "../tools/bench_support/block3a.rs"]
mod block3a;
#[allow(dead_code)]
#[path = "../tools/bench_support/churn.rs"]
mod churn;
#[path = "../tools/bench_support/key_pool.rs"]
mod key_pool;
use key_pool::{Client, Factory, Result, Store, View};
use siftwake::{Database, Key, Mutation, Query, Snapshot, SnapshotReader, StoredDocument};
struct Engine(Database);
#[derive(Clone)]
struct Readers(SnapshotReader);
struct Reader(SnapshotReader);
impl Factory for Readers {
    type Client = Reader;
    fn open(&self) -> Result<Reader> {
        Ok(Reader(self.0.clone()))
    }
}
impl Client for Reader {
    type View = Snapshot;
    fn acquire(&mut self) -> Result<Snapshot> {
        Ok(self.0.acquire()?)
    }
}
impl View for Snapshot {
    fn sequence(&self) -> u64 {
        self.sequence()
    }
    fn first(&self, q: &Query) -> Result<siftwake::FirstPage> {
        #[cfg(feature = "diagnostic-timing")]
        let _isolated = (std::env::var_os("POOL_VISIBILITY_PROFILE").is_some()
            && std::env::var_os("FDB_TIMING_DISABLED").is_none()
            && siftwake::timing::request::current().is_none())
        .then(siftwake::timing::request::Request::new);
        Ok(self.search_first_hybrid(q, 200)?)
    }
    fn begin_probe_diagnostic(&self) -> Box<dyn FnOnce() -> serde_json::Value> {
        #[cfg(feature = "diagnostic-timing")]
        {
            let request = siftwake::timing::request::Request::new();
            Box::new(move || serde_json::to_value(request.finish()).unwrap())
        }
        #[cfg(not(feature = "diagnostic-timing"))]
        {
            Box::new(|| serde_json::Value::Null)
        }
    }
    fn get(&self, k: &Key) -> Result<Option<StoredDocument>> {
        Ok(self.get(k)?)
    }
}
impl Store for Engine {
    type Factory = Readers;
    fn commit(&mut self, m: Vec<Mutation>) -> Result<u64> {
        Ok(self.0.commit_streamed(m.into_iter().map(Ok))?)
    }
    fn factory(&mut self) -> Result<Readers> {
        Ok(Readers(self.0.snapshot_reader()?))
    }
    fn keys(&self, col: &str) -> Result<Vec<Key>> {
        let mut q = Query::new("shared");
        q.collection = Some(col.into());
        q.limit = 1000;
        let page = self.0.search(&q)?;
        assert!(page.total <= 1000, "unexpected pool size");
        Ok(page.hits.into_iter().map(|d| d.document.key).collect())
    }
    fn maintain(&mut self) -> Result<()> {
        self.0.maintain_idle()?;
        Ok(())
    }
    fn sequence(&self) -> Result<u64> {
        Ok(self.0.stats()?.sequence)
    }
    fn diagnostic_layout(&self) -> Result<serde_json::Value> {
        #[cfg(feature = "diagnostic-timing")]
        let owners = serde_json::json!(self.0.diagnostic_run_owners());
        #[cfg(not(feature = "diagnostic-timing"))]
        let owners = serde_json::Value::Null;
        Ok(
            serde_json::json!({"stats":self.0.stats()?,"runs":self.0.segment_liveness()?,"owners":owners}),
        )
    }
    fn begin_commit_diagnostic(&mut self) -> Result<()> {
        Ok(self.0.start_write_profile(1)?)
    }
    fn take_commit_diagnostic(&mut self) -> serde_json::Value {
        serde_json::to_value(self.0.take_write_profile()).unwrap()
    }
}
fn main() -> Result<()> {
    let result = run();
    #[cfg(feature = "diagnostic-timing")]
    churn::emit(
        serde_json::json!({"phase":"term_cache_diagnostics","counters":siftwake::term_cache_diagnostics()}),
    );
    result
}
fn run() -> Result<()> {
    churn::start_memory_sampler();
    assert!(std::env::var_os("FDB_HYBRID_WAIT_REFERENCE").is_none());
    let mut engine = Engine(Database::open("data/benchmark-work/fdb")?);
    engine.0.set_streamed_wal_layer(true)?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args[0] == "block3a" {
        return block3a::run(&mut engine, &args[1], &args[2]);
    }
    if args[0] == "inspect" {
        churn::emit(
            serde_json::json!({"phase":"inspect","sequence":engine.sequence()?,"numeric":engine.keys(key_pool::collection("numeric"))?.len(),"frunner":engine.keys(key_pool::collection("frunner"))?.len()}),
        );
        return Ok(());
    }
    if args[0] == "verify" || args[0] == "hold" {
        key_pool::check_saved(&mut engine, &args[1])?;
        if args[0] == "hold" {
            loop {
                std::thread::park();
            }
        }
        return Ok(());
    }
    if args[0] == "cleanup" {
        key_pool::cleanup(&mut engine)?;
        churn::emit(
            serde_json::json!({"phase":"cleanup","sequence":engine.sequence()?,"memory":churn::memory()}),
        );
        return Ok(());
    }
    let config = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    key_pool::run(&mut engine, config)
}
