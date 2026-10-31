//! Query-local cooperative cancellation. One sticky token per UI request.
use crate::{Error, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone, Debug, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
    progress: Option<Arc<AtomicU64>>,
}
impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
    /// Diagnostic witness only; normal tokens avoid the shared progress counter.
    pub fn observed() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            progress: Some(Arc::new(AtomicU64::new(0))),
        }
    }
    pub fn candidates_started(&self) -> Option<u64> {
        self.progress.as_ref().map(|p| p.load(Ordering::Relaxed))
    }
    pub(crate) fn candidate(&self) {
        if let Some(p) = &self.progress {
            p.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub(crate) fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}
pub(crate) fn check(token: Option<&CancelToken>) -> Result<()> {
    if let Some(t) = token {
        t.check()?;
    }
    Ok(())
}
