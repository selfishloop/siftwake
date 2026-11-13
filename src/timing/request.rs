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
    /// At most 64 distinct owners plus owner 0 for overflow; no document keys.
    pub owners: BTreeMap<u64, BTreeMap<&'static str, super::Counter>>,
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
    owners: BTreeMap<u64, BTreeMap<&'static str, super::Counter>>,
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
pub(super) fn active() -> bool {
    LOCAL.with(|s| s.borrow().is_some())
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
                    owners: BTreeMap::new(),
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
        for (owner, counters) in local.owners {
            let owner = bounded_owner(&out.owners, owner);
            let target = out.owners.entry(owner).or_default();
            for (name, counter) in counters {
                let value = target.entry(name).or_default();
                value.calls = value.calls.saturating_add(counter.calls);
                value.elapsed_ns = value.elapsed_ns.saturating_add(counter.elapsed_ns);
            }
        }
    }
}
fn bounded_owner<T>(owners: &BTreeMap<u64, T>, owner: u64) -> u64 {
    if owners.contains_key(&owner)
        || owners
            .len()
            .saturating_sub(usize::from(owners.contains_key(&0)))
            < 64
    {
        owner
    } else {
        0
    }
}
/// Fixed engine call sites only. Detached/normal requests retain no owner data.
pub(crate) fn owner_record(owner: u64, name: &'static str, calls: u64, ns: u64) {
    LOCAL.with(|slot| {
        let mut local = slot.borrow_mut();
        let Some(local) = local.as_mut() else { return };
        let owner = bounded_owner(&local.owners, owner);
        let counter = local
            .owners
            .entry(owner)
            .or_default()
            .entry(name)
            .or_default();
        counter.calls = counter.calls.saturating_add(calls);
        counter.elapsed_ns = counter.elapsed_ns.saturating_add(ns);
    });
}
pub(crate) struct OwnerSpan(u64, &'static str, Option<Instant>);
pub(crate) fn owner_span(owner: u64, name: &'static str) -> OwnerSpan {
    OwnerSpan(owner, name, active().then(Instant::now))
}
impl Drop for OwnerSpan {
    fn drop(&mut self) {
        if let Some(at) = self.2 {
            owner_record(
                self.0,
                self.1,
                1,
                at.elapsed().as_nanos().min(u64::MAX as u128) as u64,
            );
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
    fn owner_diagnostics_are_bounded_and_request_isolated() {
        let request = Request::new();
        for owner in 1..=100 {
            owner_record(owner, "lookup", 1, 2);
        }
        let child = current();
        std::thread::spawn(move || {
            let _scope = attach(child);
            for owner in 101..=200 {
                owner_record(owner, "lookup", 1, 2);
            }
        })
        .join()
        .unwrap();
        let sample = request.finish();
        assert_eq!(sample.owners.len(), 65);
        assert_eq!(
            sample
                .owners
                .values()
                .map(|c| c["lookup"].calls)
                .sum::<u64>(),
            200
        );
        assert_eq!(sample.owners[&0]["lookup"].calls, 136);
        assert!(Request::new().finish().owners.is_empty());
    }
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
