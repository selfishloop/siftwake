//! Allocation origin hooks for a separate diagnostic allocator, never RSS.
use std::sync::OnceLock;
pub const NAMES: [&str; 8] = [
    "other",
    "caller_documents",
    "delta_index",
    "delta_tables",
    "source_batch",
    "frame_encode",
    "builder",
    "packed_metadata",
];
static HOOK: OnceLock<fn(usize) -> usize> = OnceLock::new();
type Observer = fn(&'static str, u64);
static OBSERVER: OnceLock<Observer> = OnceLock::new();
/// Install before using the engine. A hook changes thread-local allocation origin.
pub fn install(hook: fn(usize) -> usize) {
    let _ = HOOK.set(hook);
}
/// Optional diagnostic stage snapshots. Absent from normal builds; callbacks
/// run outside allocation hooks and must not call back into the database.
pub fn install_observer(observer: Observer) {
    let _ = OBSERVER.set(observer);
}
pub(crate) fn observe(stage: &'static str, sequence: u64) {
    if let Some(observer) = OBSERVER.get() {
        observer(stage, sequence);
    }
}
pub struct Scope {
    old: usize,
    hook: Option<fn(usize) -> usize>,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Scope {
    pub fn new(origin: usize) -> Self {
        let hook = HOOK.get().copied();
        Self {
            old: hook.map_or(0, |set| set(origin)),
            _thread_bound: std::marker::PhantomData,
            hook,
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(set) = self.hook {
            set(self.old);
        }
    }
}
