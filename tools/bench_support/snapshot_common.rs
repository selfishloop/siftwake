//! Common fixed-corpus load/oracle for native FDB and the isolated SQLite reference.
use super::churn::{self, Backend, Change, Corpus, Model, Result, Row};
use serde_json::{Value, json};
use siftwake::{
    Query, SearchPage, SnapshotAdmission, SnapshotReadWait, SnapshotRelease, StoredDocument,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Barrier, OnceLock, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};
#[derive(Clone, Copy)]
struct ThreadSample {
    cpu_100ns: u64,
    cycles: u64,
}
impl ThreadSample {
    fn now() -> Option<Self> {
        #[cfg(windows)]
        {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn GetCurrentThread() -> *mut std::ffi::c_void;
                fn GetThreadTimes(
                    h: *mut std::ffi::c_void,
                    c: *mut u64,
                    e: *mut u64,
                    k: *mut u64,
                    u: *mut u64,
                ) -> i32;
                fn QueryThreadCycleTime(h: *mut std::ffi::c_void, cycles: *mut u64) -> i32;
            }
            let (mut c, mut e, mut k, mut u, mut cycles) = (0, 0, 0, 0, 0);
            // FILETIME is two DWORDs, represented here by an aligned eight-byte value.
            let ok = unsafe {
                let h = GetCurrentThread();
                GetThreadTimes(h, &mut c, &mut e, &mut k, &mut u) != 0
                    && QueryThreadCycleTime(h, &mut cycles) != 0
            };
            ok.then_some(Self {
                cpu_100ns: k + u,
                cycles,
            })
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
    fn elapsed(self) -> Value {
        Self::now().map_or(Value::Null, |end| json!({"thread_cpu_ms":end.cpu_100ns.saturating_sub(self.cpu_100ns) as f64/10000.,"thread_cycles":end.cycles.saturating_sub(self.cycles),"cpu_clock":"GetThreadTimes; coarse at short intervals; residual is I/O+wait+deschedule, not pure scheduler"}))
    }
}
pub trait View {
    fn sequence(&self) -> u64;
    fn get(&self, id: usize) -> Result<Option<StoredDocument>>;
    fn search(&self, q: &Query) -> Result<(SearchPage, SnapshotReadWait)>;
    fn first(&self, _q: &Query) -> Result<siftwake::FirstPage> {
        Err("first-page benchmark adapter unsupported".into())
    }
    fn first_profiled(&self, q: &Query) -> Result<(siftwake::FirstPage, Value)> {
        Ok((self.first(q)?, Value::Null))
    }
    fn release(self) -> Result<SnapshotRelease>;
}
pub trait Client {
    type View: View;
    fn acquire(&mut self) -> Result<(Self::View, SnapshotAdmission)>;
}
pub trait Factory: Clone + Send + Sync + 'static {
    type Client: Client;
    fn open(&self) -> Result<Self::Client>;
}
pub trait Writer: Backend {
    type Factory: Factory;
    fn factory(&mut self) -> Result<Self::Factory>;
    fn interlude(&mut self) -> Result<Value>;
    fn publication(&self) -> Value;
    fn reset_timing(&self) {}
    fn take_timing(&self) -> Value {
        Value::Null
    }
    fn begin_write_profile(&mut self) -> Result<()> {
        Ok(())
    }
    fn finish_write_profile(&mut self) -> Value {
        Value::Null
    }
}
impl View for siftwake::Snapshot {
    fn sequence(&self) -> u64 {
        self.sequence()
    }
    fn get(&self, id: usize) -> Result<Option<StoredDocument>> {
        Ok(self.get(&churn::key(id))?)
    }
    fn search(&self, q: &Query) -> Result<(SearchPage, SnapshotReadWait)> {
        Ok(self.search_with_waits(q)?)
    }
    fn release(self) -> Result<SnapshotRelease> {
        Ok(self.release())
    }
}
impl Client for siftwake::SnapshotReader {
    type View = siftwake::Snapshot;
    fn acquire(&mut self) -> Result<(Self::View, SnapshotAdmission)> {
        Ok(self.acquire_timed()?)
    }
}
impl Factory for siftwake::SnapshotReader {
    type Client = Self;
    fn open(&self) -> Result<Self> {
        Ok(self.clone())
    }
}
#[derive(Default)]
struct History {
    rows: BTreeMap<usize, Vec<(u64, Row)>>,
    groups: BTreeMap<u64, Vec<usize>>,
}
impl History {
    fn add(&mut self, seq: u64, changes: &[Change]) {
        let mut ids = vec![];
        for c in changes {
            let mut row = c.new.as_ref().unwrap().clone();
            row.version = seq;
            self.rows.entry(c.id).or_default().push((seq, row));
            ids.push(c.id);
        }
        self.groups.insert(seq, ids);
    }
    fn row(&self, base: &Model, id: usize, seq: u64) -> Row {
        if let Some(rows) = self.rows.get(&id) {
            let end = rows.partition_point(|(s, _)| *s <= seq);
            if end != 0 {
                return rows[end - 1].1.clone();
            }
        }
        base.rows[&id].clone()
    }
}
fn changes(model: &Model, ids: &[usize], batch: usize, at: usize) -> Vec<Change> {
    (0..batch)
        .map(|n| {
            let id = ids[(at * batch + n) % ids.len()];
            let old = model.rows[&id].clone();
            let mut new = old.clone();
            new.revision += 1;
            Change {
                id,
                old: Some(old),
                new: Some(new),
                kind: "update".into(),
            }
        })
        .collect()
}
struct Done(Arc<AtomicUsize>);
impl Drop for Done {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Release);
    }
}
type ReadResult = std::thread::JoinHandle<Result<Value>>;
#[derive(Clone)]
struct ReadConfig {
    count: usize,
    rounds: usize,
    active: Arc<AtomicBool>,
    barrier: Arc<Barrier>,
    done: Arc<AtomicUsize>,
    clock: Arc<OnceLock<Instant>>,
    active_rounds: Option<Arc<Vec<AtomicUsize>>>,
}
fn readers<F: Factory>(
    factory: F,
    base: Arc<Model>,
    history: Arc<RwLock<History>>,
    config: ReadConfig,
) -> Vec<ReadResult> {
    let ReadConfig {
        count,
        rounds,
        active,
        barrier,
        done,
        clock,
        active_rounds,
    } = config;
    (0..count).map(|thread|{
        let factory=factory.clone();let base=base.clone();let history=history.clone();let active=active.clone();let barrier=barrier.clone();let done=done.clone();let clock=clock.clone();let active_rounds=active_rounds.clone();
        std::thread::spawn(move||->Result<Value>{
            let _done=Done(done);let corpus=Corpus::open();let client=factory.open();barrier.wait();barrier.wait();let phase_at=*clock.get().unwrap();let mut corpus=corpus?;let mut client=client?;let cases=corpus.index.cases.clone();
            let expected:Vec<_>=cases.iter().map(|c|base.expected_ids(&corpus,c)).collect();let mut records=vec![];let mut checked_seq=None;
            let case_ids:Vec<usize>=std::env::var("FDB_SNAPSHOT_CASES").ok().map(|s|s.split(',').map(|v|v.parse::<usize>().expect("case ID")).collect()).unwrap_or_else(||(0..cases.len()).collect());
            assert!(!case_ids.is_empty() && case_ids.iter().all(|&n|n<cases.len()));
            let continuous=std::env::var_os("FDB_SNAPSHOT_FIXED_COMMITS").is_some()&&active.load(Ordering::Acquire);
            let mut round=0;
            while round<rounds || (continuous&&active.load(Ordering::Acquire)) {for position in 0..case_ids.len(){
                let case_id=case_ids[(position+thread*9+round*7)%case_ids.len()];let case=&cases[case_id];let in_write=active.load(Ordering::Acquire);
                let held=Instant::now();let started_ms=phase_at.elapsed().as_secs_f64()*1000.;let (view,admission)=client.acquire()?;let seq=view.sequence();let mut q=Query::new("shared");q.collection=Some("texts".into());q.text=case.text.clone();q.offset=case.offset;q.limit=case.limit;
                let first_mode=std::env::var_os("FDB_SNAPSHOT_FIRST_HYBRID").is_some();
                let start=Instant::now();
                let (page,waits,total_kind,query_profile)=if first_mode {
                    let (p,profile)=view.first_profiled(&q)?;let total=p.total;let numeric=match total {siftwake::MatchTotal::Exact(n)|siftwake::MatchTotal::AtLeast(n)=>n};
                    let mut waits=SnapshotReadWait::default();if let Some(counter)=profile.get("counters").and_then(|v|v.get("lock.cache_wait")){waits.cache_wait_ns=counter["elapsed_ns"].as_u64().unwrap_or(0);waits.cache_contentions=counter["calls"].as_u64().unwrap_or(0);}
                    (SearchPage{total:numeric,sequence:p.sequence,hits:p.hits},waits,total,profile)
                }else{let(p,w)=view.search(&q)?;let total=siftwake::MatchTotal::Exact(p.total);(p,w,total,Value::Null)};
                let search_ns=start.elapsed().as_nanos() as u64;assert_eq!(page.sequence,seq);
                let oracle=Instant::now();let waiting=Instant::now();let locked=history.read().unwrap();let oracle_lock_ns=waiting.elapsed().as_nanos() as u64;
                let expected_ids=&expected[case_id];let expected_page:Vec<_>=expected_ids.iter().skip(case.offset).take(case.limit).map(|&id|(id,locked.row(&base,id,seq))).collect();
                let group=if checked_seq!=Some(seq){locked.groups.range(..=seq).next_back().map(|(_,ids)|ids.iter().map(|&id|(id,locked.row(&base,id,seq))).collect::<Vec<_>>()).unwrap_or_default()}else{vec![]};drop(locked);
                match total_kind {siftwake::MatchTotal::Exact(n)=>assert_eq!(n,expected_ids.len()),siftwake::MatchTotal::AtLeast(n)=>assert!(n==case.limit && expected_ids.len()>=n)};assert_eq!(page.hits.len(),expected_page.len());
                for (actual,(id,row)) in page.hits.iter().zip(&expected_page){assert_eq!(*actual,StoredDocument{document:corpus.document(*id,row)?,version:row.version});}
                for (id,row) in &group {assert_eq!(view.get(*id)?,Some(StoredDocument{document:corpus.document(*id,row)?,version:row.version}));}
                if position==0 && let Some((id,row))=group.first() && row.revision!=0 {let mut old=Query::new("shared");old.collection=Some("texts".into());old.text=churn::marker(*id,row.revision-1);assert_eq!(view.search(&old)?.0.total,0);}
                checked_seq=Some(seq);let oracle_ns=oracle.elapsed().as_nanos() as u64;let release_at=Instant::now();let release=view.release()?;let release_ns=release_at.elapsed().as_nanos() as u64;assert!(release.cleanup_error.is_none(),"{release:?}");
                let writer_wait_ns=admission.wait_ns+waits.writer_wait_ns+release.writer_wait_ns;let max_writer_wait_ns=admission.wait_ns.max(waits.max_writer_wait_ns).max(release.max_writer_wait_ns);
                records.push(json!({"variant":if first_mode{"first_N50"}else{"exact_serial"},"query_profile":query_profile,"internal_wait_measured":!first_mode || query_profile.get("counters").is_some(),"internal_wait_scope":if query_profile.get("counters").is_some(){"request-local wall counters include term/metadata mutex acquisition + permit acquisition, including uncontended overhead; child counters merged; nested stages overlap"}else{"unavailable for first-page"},"match_total":total_kind,"thread":thread,"round":round,"case_id":case_id,"started_ms":started_ms,"finished_ms":phase_at.elapsed().as_secs_f64()*1000.,"max_writer_wait_ns":max_writer_wait_ns,"sequence":seq,"writer_active":in_write,"admission":admission,"search_ms":search_ns as f64/1e6,"cache_wait":waits,"cache_wait_scope":if first_mode{"diagnostic request lock.cache_wait measures actual blocking parent+workers; unprofiled first-page unavailable; writer ownership attribution unavailable"}else{"ReadScope"},"release":release,"release_ms":release_ns as f64/1e6,"engine_ms":(admission.elapsed_ns+search_ns+release_ns) as f64/1e6,"held_including_oracle_ms":held.elapsed().as_secs_f64()*1000.,"oracle_ms":oracle_ns as f64/1e6,"oracle_ledger_wait_ns":oracle_lock_ns,"writer_wait_ns":writer_wait_ns,"writer_waited":admission.contended||waits.writer_contentions!=0||release.writer_wait_ns!=0,"checked_group_records":group.len(),"oracle":true}));
            }round+=1;
            if active.load(Ordering::Acquire) && let Some(progress)=&active_rounds {progress[thread].store(round,Ordering::Release);}
            if std::env::var_os("FDB_SNAPSHOT_ESTIMATOR_PROGRESS").is_some() && round%5==0 {
                churn::emit(json!({"phase":"estimator_progress","thread":thread,"rounds":round,"readers":count,"writer_active":active.load(Ordering::Acquire),"elapsed_ms":phase_at.elapsed().as_secs_f64()*1000.}));
            }
            }
            Ok(json!({"thread":thread,"records":records,"oracle":true}))
        })
    }).collect()
}
fn idle<F: Factory>(
    factory: F,
    base: Arc<Model>,
    threads: usize,
    rounds: usize,
    label: &str,
) -> Result<()> {
    let barrier = Arc::new(Barrier::new(threads + 1));
    let clock = Arc::new(OnceLock::new());
    let workers = readers(
        factory,
        base,
        Arc::new(RwLock::new(History::default())),
        ReadConfig {
            count: threads,
            rounds,
            active: Arc::new(AtomicBool::new(false)),
            barrier: barrier.clone(),
            done: Arc::new(AtomicUsize::new(0)),
            clock: clock.clone(),
            active_rounds: None,
        },
    );
    barrier.wait();
    let at = Instant::now();
    clock.set(at).unwrap();
    barrier.wait();
    let results = workers
        .into_iter()
        .map(|w| w.join().map_err(|_| "reader panic")?)
        .collect::<Result<Vec<_>>>()?;
    churn::emit(
        json!({"phase":label,"readers":threads,"results":results,"elapsed_ms":at.elapsed().as_secs_f64()*1000.,"oracle":true}),
    );
    Ok(())
}
pub fn run<W: Writer>(engine: &mut W, state: &str, mode: &str) -> Result<()> {
    let mut corpus = Corpus::open()?;
    let mut model = Model::load(state)?;
    if std::env::var_os("FDB_SNAPSHOT_IDLE_ONLY").is_some() {
        churn::verify_all(engine, &mut corpus, &model)?;
        let factory = engine.factory()?;
        let threads = std::env::var("FDB_SNAPSHOT_IDLE_READERS")?.parse::<usize>()?;
        assert!(threads == 1 || threads == 4);
        let rounds = std::env::var("FDB_SNAPSHOT_ROUNDS")?.parse::<usize>()?;
        idle(
            factory,
            Arc::new(model),
            threads,
            rounds,
            "snapshot_idle_probe",
        )?;
        churn::emit(
            json!({"phase":"idle_probe_complete","memory":churn::memory(),"oracle":true,"writes":0}),
        );
        return Ok(());
    }
    let batch = if mode.ends_with("-100") { 100 } else { 1 };
    let control = mode.contains("control");
    let ids: Vec<_> = model
        .rows
        .keys()
        .copied()
        .filter(|id| {
            std::env::var_os("FDB_BENCH_50K_WRITE").is_none()
                || (2048..=199_960).contains(&corpus.index.entries[model.rows[id].body].text_bytes)
        })
        .take(1100)
        .collect();
    // Same body donor and byte length; only the unique revision token changes.
    // Thus all 38 case result-ID sets remain stable across writer epochs.
    for group in ids.chunks(100) {
        let changes = changes(&model, group, group.len(), 0);
        let seq = engine.commit(&mut corpus, &changes, true)?;
        churn::apply(&mut model, &changes, seq);
    }
    model.save(state)?;
    let fixed = std::env::var("FDB_SNAPSHOT_FIXED_COMMITS").ok().map(|s| {
        s.parse::<usize>()
            .expect("fixed commits must be a positive integer")
    });
    if fixed.is_some() {
        // Begin both arms with maintenance drained. Loading baseline queries
        // must not quietly give one arm extra worker catch-up time.
        engine.maintain()?;
    }
    churn::verify_all(engine, &mut corpus, &model)?;
    let factory = engine.factory()?;
    let base = Arc::new(model.clone());
    let rounds = std::env::var("FDB_SNAPSHOT_ROUNDS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(5);
    if !control {
        idle(
            factory.clone(),
            base.clone(),
            1,
            rounds * 4,
            "snapshot_idle_single",
        )?;
        idle(
            factory.clone(),
            base.clone(),
            4,
            rounds,
            "snapshot_idle_four",
        )?;
    }
    let history = Arc::new(RwLock::new(History::default()));
    let active = Arc::new(AtomicBool::new(!control));
    let done = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(if control { 1 } else { 5 }));
    let clock = Arc::new(OnceLock::new());
    // Measurement-only: keep real writer load alive until every reader has
    // completed this many full 41-case rounds. No post-writer samples substituted.
    let minimum_active_rounds = std::env::var("FDB_SNAPSHOT_MIN_ACTIVE_ROUNDS")
        .ok()
        .map(|s| s.parse::<usize>())
        .transpose()?;
    if let Some(minimum) = minimum_active_rounds {
        assert!(minimum > 0 && minimum == rounds && fixed.is_some());
    }
    let active_rounds = Arc::new((0..4).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
    let workers = if control {
        vec![]
    } else {
        readers(
            factory,
            base,
            history.clone(),
            ReadConfig {
                count: 4,
                rounds,
                active: active.clone(),
                barrier: barrier.clone(),
                done: done.clone(),
                clock: clock.clone(),
                active_rounds: minimum_active_rounds.map(|_| active_rounds.clone()),
            },
        )
    };
    let target = std::env::var("FDB_SNAPSHOT_CONTROL_COMMITS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0usize);
    let limit = std::env::var("FDB_SNAPSHOT_MAX_COMMITS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if batch == 1 { 65536 } else { 4096 });
    engine.begin_write_profile()?;
    let profile_cpu = std::env::var_os("FDB_THREAD_PROFILE").is_some();
    barrier.wait();
    let at = Instant::now();
    clock.set(at).unwrap();
    barrier.wait();
    let mut writes = vec![];
    let mut maintenance = Value::Null;
    let mut failure = None;
    while if let Some(target) = fixed {
        writes.len() < target
            || (!control
                && minimum_active_rounds.is_some_and(|minimum| {
                    active_rounds
                        .iter()
                        .any(|n| n.load(Ordering::Acquire) < minimum)
                }))
    } else if control {
        writes.len() < target
    } else {
        done.load(Ordering::Acquire) < 4 || writes.len() < 9
    } {
        if writes.len() >= limit {
            failure = Some("writer cap reached; coverage incomplete".to_string());
            break;
        }
        let group = changes(&model, &ids, batch, writes.len());
        let predicted = engine.stats()?["sequence"].as_u64().unwrap() + 1;
        history.write().unwrap().add(predicted, &group);
        let thread_start = profile_cpu.then(ThreadSample::now).flatten();
        let commit = Instant::now();
        match engine.commit(&mut corpus, &group, true) {
            Ok(seq) => {
                let ms = commit.elapsed().as_secs_f64() * 1000.;
                assert_eq!(seq, predicted);
                churn::apply(&mut model, &group, seq);
                writes.push(json!({"sequence":seq,"commit_ms":ms,"elapsed_ms":at.elapsed().as_secs_f64()*1000.,"cpu":thread_start.map(ThreadSample::elapsed)}));
            }
            Err(e) => {
                failure = Some(e.to_string());
                break;
            }
        }
        if writes.len() == 8 {
            let started = at.elapsed().as_secs_f64() * 1000.;
            let job = engine.interlude()?;
            maintenance =
                json!({"start_ms":started,"end_ms":at.elapsed().as_secs_f64()*1000.,"details":job});
        }
    }
    let elapsed_ms = at.elapsed().as_secs_f64() * 1000.;
    active.store(false, Ordering::Release);
    let write_profile = engine.finish_write_profile();
    model.save(state)?;
    let results = workers
        .into_iter()
        .map(|w| w.join().map_err(|_| "reader panic")?)
        .collect::<Result<Vec<_>>>()?;
    churn::verify_all(engine, &mut corpus, &model)?;
    churn::emit(
        json!({"phase":"snapshot_load","control":control,"batch":batch,"commits":writes.len(),"mutations":writes.len()*batch,"writer_elapsed_ms":elapsed_ms,"ops_s":writes.len() as f64*batch as f64*1000./elapsed_ms,"writes":writes,"results":results,"maintenance":maintenance,"publication":engine.publication(),"write_profile":write_profile,"memory":churn::memory(),"failure":failure,"oracle":true,"body_workload":"same existing donors, stable search sets, unique revision token changes; not arbitrary body replacement","latency":"admission+search+last-reader release, excludes oracle; full held duration also recorded"}),
    );
    if let Some(e) = failure {
        return Err(e.into());
    }
    Ok(())
}
