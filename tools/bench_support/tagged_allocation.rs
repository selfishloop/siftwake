//! Diagnostic-only allocation headers preserve origin across moves/realloc/free.
//! Timings and RSS from this allocator are not normal engine performance.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
thread_local! { static ORIGIN: Cell<usize> = const { Cell::new(0) }; }
static LIVE: [AtomicUsize; 8] = [const { AtomicUsize::new(0) }; 8];
static PEAK: [AtomicUsize; 8] = [const { AtomicUsize::new(0) }; 8];
static TOTAL: AtomicUsize = AtomicUsize::new(0);
static TOTAL_PEAK: AtomicUsize = AtomicUsize::new(0);
#[derive(Clone, Copy)]
struct Header {
    owner: usize,
}
pub struct Tracking;
pub fn origin(next: usize) -> usize {
    ORIGIN.with(|v| v.replace(next.min(7)))
}
fn layout(original: Layout) -> Option<(Layout, usize)> {
    Layout::new::<Header>().extend(original).ok()
}
fn charge(owner: usize, bytes: usize) {
    let live = LIVE[owner].fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK[owner].fetch_max(live, Ordering::Relaxed);
    let total = TOTAL.fetch_add(bytes, Ordering::Relaxed) + bytes;
    TOTAL_PEAK.fetch_max(total, Ordering::Relaxed);
}
fn release(owner: usize, bytes: usize) {
    LIVE[owner].fetch_sub(bytes, Ordering::Relaxed);
    TOTAL.fetch_sub(bytes, Ordering::Relaxed);
}
unsafe fn allocate(original: Layout, owner: usize, zeroed: bool) -> *mut u8 {
    let Some((expanded, offset)) = layout(original) else {
        return std::ptr::null_mut();
    };
    // SAFETY: expanded is a valid Layout; System receives it unchanged.
    let base = unsafe {
        if zeroed {
            System.alloc_zeroed(expanded)
        } else {
            System.alloc(expanded)
        }
    };
    if base.is_null() {
        return base;
    }
    // SAFETY: extend leaves room for Header and aligns the user region to original.
    unsafe {
        base.cast::<Header>().write(Header { owner });
    }
    charge(owner, original.size());
    // SAFETY: offset is within the expanded allocation.
    unsafe { base.add(offset) }
}
unsafe fn header(user: *mut u8, original: Layout) -> (*mut u8, Layout, Header) {
    let (expanded, offset) = layout(original).unwrap();
    // SAFETY: all live user pointers were returned by allocate with this Layout.
    let base = unsafe { user.sub(offset) };
    // SAFETY: allocate initialized this Header at the aligned base.
    let value = unsafe { base.cast::<Header>().read() };
    (base, expanded, value)
}
// SAFETY: allocation expands System layouts, returns aligned user pointers, and
// restores original System base/layout for freeing; no pointer is otherwise owned.
unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, original: Layout) -> *mut u8 {
        // SAFETY: allocator caller supplies a valid Layout.
        unsafe { allocate(original, ORIGIN.with(Cell::get), false) }
    }
    unsafe fn alloc_zeroed(&self, original: Layout) -> *mut u8 {
        // SAFETY: same contract as alloc, including zeroing the user region.
        unsafe { allocate(original, ORIGIN.with(Cell::get), true) }
    }
    unsafe fn dealloc(&self, user: *mut u8, original: Layout) {
        // SAFETY: caller supplies a live pointer and its allocation Layout.
        let (base, expanded, value) = unsafe { header(user, original) };
        release(value.owner, original.size());
        // SAFETY: base/expanded match the original System allocation.
        unsafe {
            System.dealloc(base, expanded);
        }
    }
    unsafe fn realloc(&self, user: *mut u8, original: Layout, size: usize) -> *mut u8 {
        // SAFETY: caller supplies a live pointer and valid replacement size.
        let (_, _, value) = unsafe { header(user, original) };
        let Ok(next_layout) = Layout::from_size_align(size, original.align()) else {
            return std::ptr::null_mut();
        };
        // SAFETY: next_layout is valid. Preserve original owner, not current thread tag.
        let next = unsafe { allocate(next_layout, value.owner, false) };
        if !next.is_null() {
            // SAFETY: allocations are disjoint and both have at least this many bytes.
            unsafe {
                std::ptr::copy_nonoverlapping(user, next, size.min(original.size()));
                self.dealloc(user, original);
            }
        }
        next
    }
}
pub fn snapshot() -> ([usize; 8], [usize; 8], usize, usize) {
    (
        std::array::from_fn(|i| LIVE[i].load(Ordering::Relaxed)),
        std::array::from_fn(|i| PEAK[i].load(Ordering::Relaxed)),
        TOTAL.load(Ordering::Relaxed),
        TOTAL_PEAK.load(Ordering::Relaxed),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alignment_zeroing_reallocation_and_owner_survive_scope_change() {
        for align in [1, 8, 64, 4096] {
            let l = Layout::from_size_align(37, align).unwrap();
            let old = origin(4);
            // SAFETY: direct allocator calls use matching valid layouts/pointers.
            unsafe {
                let p = Tracking.alloc_zeroed(l);
                assert!(!p.is_null());
                assert_eq!(p as usize % align, 0);
                assert!((0..37).all(|i| *p.add(i) == 0));
                *p = 123;
                origin(6);
                let q = Tracking.realloc(p, l, 73);
                assert!(!q.is_null());
                assert_eq!(*q, 123);
                let next = Layout::from_size_align(73, align).unwrap();
                assert_eq!(header(q, next).2.owner, 4);
                Tracking.dealloc(q, next);
            }
            origin(old);
        }
    }
}
