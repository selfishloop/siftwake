//! Diagnostic-only request-local counters. Child threads explicitly inherit the
//! request. Each thread records locally and merges once, outside the hot loop.
//! Nested wall spans overlap; CPU and non-CPU residual are not scheduler attribution.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Default, serde::Serialize)]
pub struct Sample {
    pub counters: BTreeMap<&'static str, super::Counter>,
    pub threads: Vec<ThreadSample>,
}
#[derive(serde::Serialize)]
pub struct ThreadSample {
    pub thread_id: String,
    pub attached_wall_ns: u64,
    pub cpu_100ns: Option<u64>,
    pub cycles: Option<u64>,
}
#[derive(Clone)]
pub struct Handle(Arc<Mutex<Sample>>);
struct Local {
    handle: Handle,
    counters: BTreeMap<&'static str, super::Counter>,
    start: Instant,
    cpu: Option<(u64, u64)>,
}
thread_local! {
    static LOCAL: RefCell<Option<Local>> = const { RefCell::new(None) };
    static HYBRID: Cell<usize> = const { Cell::new(4) };
}
pub struct WorkerLimit(usize);
impl WorkerLimit {
    /// A diagnostic thread-local ceiling only; process-wide four permits remain.
    pub fn new(n: usize) -> Self {
        assert!((1..=4).contains(&n));
        Self(HYBRID.with(|s| s.replace(n)))
    }
}
impl Drop for WorkerLimit {
    fn drop(&mut self) {
        HYBRID.with(|s| s.set(self.0));
    }
}
pub(crate) fn hybrid_workers() -> usize {
    HYBRID.with(Cell::get)
}
pub fn current() -> Option<Handle> {
    LOCAL.with(|s| s.borrow().as_ref().map(|l| l.handle.clone()))
}
pub struct Scope(Option<Local>, bool);
pub fn attach(handle: Option<Handle>) -> Scope {
    match handle {
        Some(handle) => Scope(
            LOCAL.with(|s| {
                s.replace(Some(Local {
                    handle,
                    counters: BTreeMap::new(),
                    start: Instant::now(),
                    cpu: cpu(),
                }))
            }),
            true,
        ),
        None => Scope(None, false),
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if !self.1 {
            return;
        }
        let local = LOCAL
            .with(|s| s.replace(self.0.take()))
            .expect("request diagnostic context");
        let end = cpu();
        let mut out = local.handle.0.lock().unwrap_or_else(|e| e.into_inner());
        out.threads.push(ThreadSample {
            thread_id: format!("{:?}", std::thread::current().id()),
            attached_wall_ns: local.start.elapsed().as_nanos() as u64,
            cpu_100ns: local.cpu.zip(end).map(|(a, b)| b.0.saturating_sub(a.0)),
            cycles: local.cpu.zip(end).map(|(a, b)| b.1.saturating_sub(a.1)),
        });
        for (name, c) in local.counters {
            let v = out.counters.entry(name).or_default();
            v.calls += c.calls;
            v.elapsed_ns += c.elapsed_ns;
        }
    }
}
pub struct Request {
    handle: Handle,
    scope: Scope,
}
impl Default for Request {
    fn default() -> Self {
        Self::new()
    }
}
impl Request {
    pub fn new() -> Self {
        let handle = Handle(Arc::default());
        let scope = attach(Some(handle.clone()));
        Self { handle, scope }
    }
    pub fn finish(self) -> Sample {
        let Self { handle, scope } = self;
        drop(scope);
        std::mem::take(&mut *handle.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}
pub(super) fn record(name: &'static str, calls: u64, ns: u64) -> bool {
    LOCAL.with(|s| {
        let mut local = s.borrow_mut();
        let Some(local) = local.as_mut() else {
            return false;
        };
        let v = local.counters.entry(name).or_default();
        v.calls = v.calls.saturating_add(calls);
        v.elapsed_ns = v.elapsed_ns.saturating_add(ns);
        true
    })
}
fn cpu() -> Option<(u64, u64)> {
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
            fn QueryThreadCycleTime(h: *mut std::ffi::c_void, n: *mut u64) -> i32;
        }
        let (mut c, mut e, mut k, mut u, mut n) = (0, 0, 0, 0, 0);
        // FILETIME layout is eight bytes, and these variables are eight-byte aligned.
        let ok = unsafe {
            let h = GetCurrentThread();
            GetThreadTimes(h, &mut c, &mut e, &mut k, &mut u) != 0
                && QueryThreadCycleTime(h, &mut n) != 0
        };
        ok.then_some((k + u, n))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child_request_counts_are_joined_without_cross_request_leaks() {
        let request = Request::new();
        assert!(record("parent", 1, 7));
        let child = current();
        std::thread::spawn(move || {
            let _scope = attach(child);
            assert!(record("child", 2, 11));
            let unrelated = Request::new();
            assert!(record("unrelated", 3, 13));
            let other = unrelated.finish();
            assert_eq!(other.counters.len(), 1);
            assert_eq!(other.counters["unrelated"].calls, 3);
            assert!(record("child", 1, 2));
        })
        .join()
        .unwrap();
        let got = request.finish();
        assert_eq!(got.threads.len(), 2);
        assert_eq!(got.counters.len(), 2);
        assert_eq!(got.counters["parent"].elapsed_ns, 7);
        assert_eq!(got.counters["child"].calls, 3);
        assert_eq!(got.counters["child"].elapsed_ns, 13);
        assert!(current().is_none());
        assert_eq!(hybrid_workers(), 4);
        {
            let _limit = WorkerLimit::new(2);
            assert_eq!(hybrid_workers(), 2);
        }
        assert_eq!(hybrid_workers(), 4);
    }
}
