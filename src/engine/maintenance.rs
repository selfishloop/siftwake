//! Cooperative cancellation only for unpublished revision work. Normal merges
//! retain their existing behavior. A writer joins cancellation before admission.
use std::{
    cell::RefCell,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Default)]
pub(super) struct Activity {
    readers: AtomicUsize,
}
pub(super) struct Reader(Arc<Activity>);
impl Activity {
    pub fn read(self: &Arc<Self>) -> Reader {
        self.readers.fetch_add(1, Ordering::Relaxed);
        Reader(self.clone())
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.0.readers.fetch_sub(1, Ordering::Relaxed);
    }
}
pub(super) struct Control {
    cancelled: AtomicBool,
    signal: Mutex<()>,
    wake: Condvar,
    activity: Arc<Activity>,
}
impl Control {
    pub fn new(activity: Arc<Activity>) -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            signal: Mutex::new(()),
            wake: Condvar::new(),
            activity,
        }
    }
    pub fn cancel(&self) {
        let _guard = self.signal.lock().unwrap_or_else(|e| e.into_inner());
        self.cancelled.store(true, Ordering::Release);
        self.wake.notify_all();
    }
    pub fn readers_active(&self) -> bool {
        self.activity.readers.load(Ordering::Relaxed) != 0
    }
    pub fn wait(&self, time: Duration) -> crate::Result<()> {
        let guard = self.signal.lock().unwrap_or_else(|e| e.into_inner());
        if !self.cancelled.load(Ordering::Acquire) {
            let _result = self
                .wake
                .wait_timeout(guard, time)
                .unwrap_or_else(|e| e.into_inner());
        }
        self.check()
    }
    fn check(&self) -> crate::Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "unpublished revision work cancelled",
            )))
        } else {
            Ok(())
        }
    }
}
thread_local! { static CURRENT:RefCell<Option<Arc<Control>>>=const {RefCell::new(None)}; }
pub(super) struct Scope(Option<Arc<Control>>);
impl Scope {
    pub fn enter(control: Option<Arc<Control>>) -> Self {
        Self(CURRENT.with(|c| c.replace(control)))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|c| {
            c.replace(self.0.take());
        });
    }
}
pub(crate) fn check() -> crate::Result<()> {
    CURRENT.with(|c| c.borrow().as_ref().map_or(Ok(()), |c| c.check()))
}
pub(super) fn cancelled(result: &crate::Error, control: &Option<Arc<Control>>) -> bool {
    matches!(result,crate::Error::Io(e) if e.kind()==std::io::ErrorKind::Interrupted)
        && control
            .as_ref()
            .is_some_and(|c| c.cancelled.load(Ordering::Acquire))
}
