//! Opt-in exclusive wall intervals. Percentages describe the observed critical
//! timeline, not CPU time; a foreground join is attributed to the active worker.
use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
static ENABLED: AtomicBool = AtomicBool::new(false);
#[derive(Clone, serde::Serialize)]
pub struct Interval {
    pub stage: &'static str,
    pub worker: bool,
    pub start_ns: u64,
    pub end_ns: u64,
}
struct Local {
    stack: Vec<&'static str>,
    worker: bool,
    since: u64,
}
thread_local! { static LOCAL:RefCell<Local>=const { RefCell::new(Local{stack:Vec::new(),worker:false,since:0}) }; }
fn events() -> &'static Mutex<Vec<Interval>> {
    static E: OnceLock<Mutex<Vec<Interval>>> = OnceLock::new();
    E.get_or_init(Default::default)
}
fn now() -> u64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_nanos() as u64
}
fn close(local: &mut Local, at: u64) {
    if let Some(&stage) = local.stack.last() {
        events().lock().unwrap().push(Interval {
            stage,
            worker: local.worker,
            start_ns: local.since,
            end_ns: at,
        });
    }
    local.since = at;
}
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}
pub struct Span {
    enabled: bool,
    previous_worker: Option<bool>,
}
pub fn span(stage: &'static str) -> Span {
    let enabled = ENABLED.load(Ordering::Relaxed);
    if enabled {
        LOCAL.with(|l| {
            let mut l = l.borrow_mut();
            close(&mut l, now());
            l.stack.push(stage);
        });
    }
    Span {
        enabled,
        previous_worker: None,
    }
}
pub fn worker() -> Span {
    let mut guard = span("worker_other");
    if guard.enabled {
        guard.previous_worker = Some(LOCAL.with(|l| {
            let mut l = l.borrow_mut();
            let old = l.worker;
            l.worker = true;
            old
        }));
    }
    guard
}
impl Drop for Span {
    fn drop(&mut self) {
        if self.enabled {
            LOCAL.with(|l| {
                let mut l = l.borrow_mut();
                close(&mut l, now());
                l.stack.pop();
                if let Some(old) = self.previous_worker {
                    l.worker = old;
                }
            });
        }
    }
}
pub struct Window(u64);
pub fn window() -> Window {
    Window(now())
}
impl Window {
    pub fn finish(self) -> serde_json::Value {
        let end = now();
        let selected: Vec<_> = events()
            .lock()
            .unwrap()
            .drain(..)
            .filter_map(|mut e| {
                e.start_ns = e.start_ns.max(self.0);
                e.end_ns = e.end_ns.min(end);
                (e.end_ns > e.start_ns).then_some(e)
            })
            .collect();
        let mut boundaries = vec![self.0, end];
        for e in &selected {
            boundaries.extend([e.start_ns, e.end_ns]);
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        let mut critical = BTreeMap::<&str, u64>::new();
        let mut worker = BTreeMap::<&str, u64>::new();
        for e in &selected {
            if e.worker {
                *worker.entry(e.stage).or_default() += e.end_ns - e.start_ns;
            }
        }
        for times in boundaries.windows(2) {
            let covers = |e: &&Interval| e.start_ns <= times[0] && e.end_ns >= times[1];
            let fg = selected
                .iter()
                .filter(covers)
                .find(|e| !e.worker && !matches!(e.stage, "join_wait" | "workspace_wait"));
            let active = fg.or_else(|| selected.iter().filter(covers).find(|e| e.worker));
            *critical
                .entry(active.map_or("other", |e| e.stage))
                .or_default() += times[1] - times[0];
        }
        serde_json::json!({"window_ns":end-self.0,"critical_ns":critical,"worker_exclusive_ns":worker,"intervals":selected,"analysis_ns":now()-end,"method":"foreground non-wait stage, otherwise active worker, otherwise other; wall not CPU; nested stages exclusive"})
    }
}
