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
}
impl Pool {
    const fn new(limit: usize) -> Self {
        Self {
            limit,
            state: Mutex::new(State {
                used: 0,
                peak: 0,
                waits: 0,
                reduced: 0,
            }),
            wake: Condvar::new(),
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
#[cfg(feature = "experimental-builder-budget")]
static POSTING_POOL: Pool = Pool::new(2 * 1024 * 1024);
#[cfg(feature = "experimental-builder-budget")]
pub(crate) fn reserve_postings() -> Result<Lease<'static>> {
    POSTING_POOL.reserve(1024 * 1024, 256 * 1024)
}
#[cfg(feature = "experimental-builder-budget")]
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
