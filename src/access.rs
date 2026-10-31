//! Volatile lock diagnostics for snapshot reads. No storage format or lock scope change.
use std::{
    cell::{Cell, RefCell},
    ops::{Deref, DerefMut},
    sync::{
        LockResult, Mutex, MutexGuard, PoisonError, TryLockError,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
thread_local! {static WRITER:Cell<bool>=const{Cell::new(false)};static READ:RefCell<Option<ReadWait>>=const{RefCell::new(None)};}
pub(crate) struct WriterScope(bool);
impl WriterScope {
    pub(crate) fn enter() -> Self {
        Self(WRITER.with(|w| w.replace(true)))
    }
}
impl Drop for WriterScope {
    fn drop(&mut self) {
        WRITER.with(|w| w.set(self.0));
    }
}
#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct ReadWait {
    pub cache_wait_ns: u64,
    pub cache_contentions: u64,
    pub writer_wait_ns: u64,
    pub writer_contentions: u64,
    pub max_writer_wait_ns: u64,
}
pub(crate) struct ReadScope {
    previous: Option<ReadWait>,
    active: bool,
}
impl ReadScope {
    pub(crate) fn enter() -> Self {
        Self {
            previous: READ.with(|r| r.replace(Some(ReadWait::default()))),
            active: true,
        }
    }
    pub(crate) fn finish(mut self) -> ReadWait {
        let current = READ.with(|r| r.replace(self.previous.take()));
        self.active = false;
        current.unwrap_or_default()
    }
}
impl Drop for ReadScope {
    fn drop(&mut self) {
        if self.active {
            READ.with(|r| {
                r.replace(self.previous.take());
            });
        }
    }
}
pub(crate) struct TrackedMutex<T> {
    inner: Mutex<T>,
    writer: AtomicBool,
}
impl<T: Default> Default for TrackedMutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
impl<T> TrackedMutex<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
            writer: AtomicBool::new(false),
        }
    }
    pub(crate) fn lock(&self) -> LockResult<TrackedGuard<'_, T>> {
        let result = match self.inner.try_lock() {
            Ok(g) => Ok(g),
            Err(TryLockError::Poisoned(e)) => Err(e),
            Err(TryLockError::WouldBlock) => {
                let reader = READ.with(|r| r.borrow().is_some());
                let at = reader.then(Instant::now);
                let writer = self.writer.load(Ordering::Acquire);
                #[cfg(feature = "diagnostic-timing")]
                let profile = crate::timing::span("lock.cache_wait");
                let g = self.inner.lock();
                #[cfg(feature = "diagnostic-timing")]
                drop(profile);
                if let Some(at) = at {
                    let ns = at.elapsed().as_nanos() as u64;
                    READ.with(|r| {
                        if let Some(stats) = r.borrow_mut().as_mut() {
                            stats.cache_wait_ns += ns;
                            stats.cache_contentions += 1;
                            if writer {
                                stats.writer_wait_ns += ns;
                                stats.writer_contentions += 1;
                                stats.max_writer_wait_ns = stats.max_writer_wait_ns.max(ns);
                            }
                        }
                    });
                }
                g
            }
        };
        self.writer.store(WRITER.with(Cell::get), Ordering::Release);
        result
            .map(|guard| TrackedGuard { guard, owner: self })
            .map_err(|e| {
                PoisonError::new(TrackedGuard {
                    guard: e.into_inner(),
                    owner: self,
                })
            })
    }
}
pub(crate) struct TrackedGuard<'a, T> {
    guard: MutexGuard<'a, T>,
    owner: &'a TrackedMutex<T>,
}
impl<T> Deref for TrackedGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}
impl<T> DerefMut for TrackedGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}
impl<T> Drop for TrackedGuard<'_, T> {
    fn drop(&mut self) {
        self.owner.writer.store(false, Ordering::Release);
    }
}
