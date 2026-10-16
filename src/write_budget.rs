//! Shared reservation of controlled write buffers, not total heap or RSS.
use crate::{Error, Result};
use std::sync::{Condvar, Mutex};
#[derive(Clone, Copy, serde::Serialize)]
pub struct Snapshot {
    pub limit_bytes: usize,
    pub in_use_bytes: usize,
    pub peak_reserved_bytes: usize,
    pub waits: usize,
    pub reduced_leases: usize,
    pub baseline_reserved_bytes: usize,
}
struct State {
    used: usize,
    peak: usize,
    waits: usize,
    reduced: usize,
}
pub(crate) struct Pool {
    limit: usize,
    state: Mutex<State>,
    wake: Condvar,
    baseline: usize,
}
impl Pool {
    const fn new(limit: usize) -> Self {
        Self::with_baseline(limit, 0)
    }
    const fn with_baseline(limit: usize, baseline: usize) -> Self {
        Self {
            limit,
            state: Mutex::new(State {
                used: baseline,
                peak: baseline,
                waits: 0,
                reduced: 0,
            }),
            wake: Condvar::new(),
            baseline,
        }
    }
    pub(crate) fn reserve(&self, preferred: usize, minimum: usize) -> Result<Lease<'_>> {
        if minimum == 0 || preferred < minimum || preferred > self.limit {
            return Err(Error::Invalid("invalid write buffer reservation".into()));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| Error::Invalid("write budget poisoned".into()))?;
        while self.limit - state.used < minimum {
            state.waits += 1;
            state = self
                .wake
                .wait(state)
                .map_err(|_| Error::Invalid("write budget poisoned".into()))?;
        }
        let bytes = preferred.min(self.limit - state.used);
        state.reduced += usize::from(bytes < preferred);
        state.used += bytes;
        state.peak = state.peak.max(state.used);
        Ok(Lease { pool: self, bytes })
    }
}
pub(crate) struct Lease<'a> {
    pool: &'a Pool,
    pub bytes: usize,
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let mut state = self.pool.state.lock().unwrap_or_else(|e| e.into_inner());
        state.used -= self.bytes;
        self.pool.wake.notify_all();
    }
}
static POOL: Pool = Pool::new(256 * 1024);
// One admission pool for streamed input and builder/maintenance workspaces.
// Reservations are conservative workspace allowances, never process RSS.
#[cfg(feature = "bounded-engine")]
static WORKING_POOL: Pool = Pool::with_baseline(16 * 1024 * 1024, 2 * 1024 * 1024);
#[cfg(feature = "bounded-engine")]
pub(crate) fn reserve_working(bytes: usize) -> Result<Lease<'static>> {
    WORKING_POOL.reserve(bytes, bytes)
}
#[cfg(feature = "bounded-engine")]
thread_local! { static STREAM_WORKSPACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(feature = "bounded-engine")]
pub(crate) struct WorkingScope {
    _lease: Option<Lease<'static>>,
    owns_thread: bool,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}
#[cfg(feature = "bounded-engine")]
impl WorkingScope {
    pub(crate) fn streamed() -> Result<Self> {
        if STREAM_WORKSPACE.with(std::cell::Cell::get) {
            return Err(Error::Invalid(
                "nested streamed transactions are unsupported".into(),
            ));
        }
        let lease = reserve_working(14 * 1024 * 1024)?;
        STREAM_WORKSPACE.with(|state| {
            assert!(!state.replace(true));
        });
        Ok(Self {
            _lease: Some(lease),
            owns_thread: true,
            _thread_bound: std::marker::PhantomData,
        })
    }
    pub(crate) fn builder() -> Result<Self> {
        if STREAM_WORKSPACE.with(std::cell::Cell::get) {
            return Ok(Self {
                _lease: None,
                owns_thread: false,
                _thread_bound: std::marker::PhantomData,
            });
        }
        let lease = reserve_working(8 * 1024 * 1024)?;
        STREAM_WORKSPACE.with(|state| state.set(true));
        Ok(Self {
            _lease: Some(lease),
            owns_thread: true,
            _thread_bound: std::marker::PhantomData,
        })
    }
}
#[cfg(feature = "bounded-engine")]
impl Drop for WorkingScope {
    fn drop(&mut self) {
        if self.owns_thread {
            STREAM_WORKSPACE.with(|state| state.set(false));
        }
    }
}
#[cfg(feature = "bounded-engine")]
pub(crate) fn builder_working() -> Result<Option<Lease<'static>>> {
    // The streamed scope already allows 6 MiB intake/index plus 8 MiB builder.
    // Nested builders consume that allowance instead of reserving it twice.
    if STREAM_WORKSPACE.with(std::cell::Cell::get) {
        Ok(None)
    } else {
        reserve_working(8 * 1024 * 1024).map(Some)
    }
}
#[cfg(feature = "bounded-engine")]
pub fn working_snapshot() -> Snapshot {
    pool_snapshot(&WORKING_POOL)
}
#[cfg(feature = "bounded-engine")]
static POSTING_POOL: Pool = Pool::new(2 * 1024 * 1024);
#[cfg(feature = "bounded-engine")]
pub(crate) fn reserve_postings() -> Result<Lease<'static>> {
    POSTING_POOL.reserve(1024 * 1024, 256 * 1024)
}
#[cfg(feature = "bounded-engine")]
pub fn posting_snapshot() -> Snapshot {
    pool_snapshot(&POSTING_POOL)
}
pub(crate) fn reserve(preferred: usize, minimum: usize) -> Result<Lease<'static>> {
    POOL.reserve(preferred, minimum)
}
pub fn snapshot() -> Snapshot {
    pool_snapshot(&POOL)
}
fn pool_snapshot(pool: &Pool) -> Snapshot {
    let s = pool.state.lock().unwrap_or_else(|e| e.into_inner());
    Snapshot {
        baseline_reserved_bytes: pool.baseline,
        limit_bytes: pool.limit,
        in_use_bytes: s.used,
        peak_reserved_bytes: s.peak,
        waits: s.waits,
        reduced_leases: s.reduced,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reservations_shrink_wait_and_release_on_error() {
        let pool = std::sync::Arc::new(Pool::new(32));
        let first = pool.reserve(24, 8).unwrap();
        let second = pool.reserve(16, 8).unwrap();
        assert_eq!(second.bytes, 8);
        let p = pool.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let child = std::thread::spawn(move || {
            let lease = p.reserve(16, 16).unwrap();
            tx.send(lease.bytes).unwrap();
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(20))
                .is_err()
        );
        drop(first);
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
            16
        );
        child.join().unwrap();
        drop(second);
        assert_eq!(pool.state.lock().unwrap().used, 0);
        assert_eq!(pool.state.lock().unwrap().peak, 32);
        let failure: Result<()> = (|| {
            let _lease = pool.reserve(8, 8)?;
            Err(Error::Invalid("injected operation failure".into()))
        })();
        assert!(failure.is_err());
        assert_eq!(pool.state.lock().unwrap().used, 0);
        assert!(pool.reserve(33, 1).is_err());
        assert!(pool.reserve(1, 0).is_err());
    }
}
