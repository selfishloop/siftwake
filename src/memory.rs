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
/// Install before using the engine. A hook changes thread-local allocation origin.
pub fn install(hook: fn(usize) -> usize) {
    let _ = HOOK.set(hook);
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
