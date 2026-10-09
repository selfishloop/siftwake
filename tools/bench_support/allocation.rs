//! Opt-in benchmark instrumentation. Counts requested Rust heap bytes, not RSS,
//! allocator bookkeeping, stacks, OS cache or C allocations. Adds atomic overhead.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Tracking;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);

fn add(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
    CALLS.fetch_add(1, Ordering::Relaxed);
}

// SAFETY: every pointer/layout is passed unchanged to the system allocator.
// Accounting never dereferences pointers, allocates memory or calls user code.
unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: caller supplies the valid allocation layout.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: same layout contract as alloc; System provides zeroing.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: caller supplies the live pointer and its original layout.
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: caller supplies the live pointer/layout and valid new size.
        let next = unsafe { System.realloc(ptr, layout, size) };
        if !next.is_null() {
            if size >= layout.size() {
                add(size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - size, Ordering::Relaxed);
                CALLS.fetch_add(1, Ordering::Relaxed);
            }
        }
        next
    }
}

pub fn reset() -> usize {
    let live = LIVE.load(Ordering::Relaxed);
    PEAK.store(live, Ordering::Relaxed);
    CALLS.store(0, Ordering::Relaxed);
    live
}

pub fn snapshot() -> (usize, usize, usize) {
    (
        LIVE.load(Ordering::Relaxed),
        PEAK.load(Ordering::Relaxed),
        CALLS.load(Ordering::Relaxed),
    )
}
