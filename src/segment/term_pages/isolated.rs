//! Diagnostic-only A/K/D experiment. Production retains its reference path.
use super::*;
use std::cell::RefCell;
use std::sync::{Condvar, Mutex as SlotMutex};
use std::time::Duration;

pub(super) const PAGE_HITS: usize = 0;
pub(super) const PAGE_MISSES: usize = 1;
pub(super) const CAPACITY_PAGES: usize = 2;
pub(super) const OVERSIZED_PAGES: usize = 3;
pub(super) const DROP_PAGES: usize = 4;
pub(super) const LOOKUP_EVICTIONS: usize = 5;
pub(super) const LOOKUP_HITS: usize = 6;
pub(super) const LOOKUP_MISSES: usize = 7;
const RECHECK_HITS: usize = 8;
const FALLBACK: usize = 9;
const FLIGHT_PEAK: usize = 10;
const COMBINED_PEAK: usize = 11;
pub(super) const OWNER_CREATED: usize = 12;
pub(super) const OWNER_DROPPED: usize = 13;
const LEGACY_FALLBACK: usize = 14;
static COUNTERS: [AtomicU64; 15] = [const { AtomicU64::new(0) }; 15];
static INFLIGHT: AtomicU64 = AtomicU64::new(0);
pub(super) fn count(n: usize, value: usize) {
    COUNTERS[n].fetch_add(value as u64, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(offset: u64) -> (std::path::PathBuf, File, Pages) {
        let path = std::env::temp_dir().join(format!(
            "fdb-isolated-{}-{}.tmp",
            std::process::id(),
            OWNER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let mut bytes = super::super::super::compact::MAGIC.to_vec();
        let mut fences = Vec::new();
        let mut bloom = Bloom::default();
        for i in 0..400 {
            let term = Term::Gram("şirket".into(), format!("東京{i:06}"));
            let start = bytes.len() as u64;
            super::super::super::compact::write_posting(
                &mut bytes,
                &term,
                Posting { offset, count: 1 },
            )
            .unwrap();
            push_fence(&mut fences, &term, start, bytes.len() as u64, 1).unwrap();
            bloom.add(&term);
        }
        file.write_all(&bytes).unwrap();
        let pages = Pages::new(&file, fences, vec![], bloom).unwrap();
        (path, file, pages)
    }
    fn remove(path: std::path::PathBuf, file: File, pages: Pages) {
        drop(pages);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn a_bound_owner_concurrent_reads_crc_order_cancel_and_unwind() {
        let (path, file, pages) = fixture(4);
        let (otherpath, otherfile, other) = fixture(8);
        let pages = std::sync::Arc::new(pages);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let pages = &pages;
                scope.spawn(move || {
                    for page in 0..pages.fences.len() {
                        let values = pages.with_page_isolated(page, |v| v.to_vec()).unwrap();
                        assert!(values.iter().all(|v| v.1.offset == 4));
                        assert!(values.windows(2).all(|v| v[0].0 < v[1].0));
                    }
                });
            }
        });
        assert_eq!(other.with_page_isolated(0, |v| v[0].1.offset).unwrap(), 8);
        // A cache hit must not wait for the decoder slot.
        let held = DecodeSlot::acquire().unwrap();
        assert_eq!(other.with_page_isolated(0, |v| v[0].1.offset).unwrap(), 8);
        let token = crate::CancelToken::new();
        std::thread::scope(|s| {
            let t = token.clone();
            let thread = s.spawn(move || {
                let _scope = cancel_scope(Some(&t));
                DecodeSlot::acquire().map(drop)
            });
            token.cancel();
            assert!(matches!(thread.join().unwrap(), Err(Error::Cancelled)));
        });
        drop(held);
        // Use a new uncached owner: panic outside the cache guard releases both
        // reservation and slot without poisoning the metadata mutex.
        let (panicpath, panicfile, panicpages) = fixture(12);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = panicpages.with_page_isolated(0, |_| -> () {
                panic!("injected callback");
            });
        }));
        assert!(panic.is_err());
        assert_eq!(INFLIGHT.load(Ordering::Relaxed), 0);
        assert_eq!(
            panicpages.with_page_isolated(0, |v| v[0].1.offset).unwrap(),
            12
        );
        remove(panicpath, panicfile, panicpages);
        let (badpath, badfile, mut bad) = fixture(16);
        bad.fences[0].checksum ^= 1;
        assert!(matches!(
            bad.with_page_isolated(0, |_| ()),
            Err(Error::Corrupt(_))
        ));
        assert_eq!(INFLIGHT.load(Ordering::Relaxed), 0);
        drop(DecodeSlot::acquire().unwrap());
        remove(badpath, badfile, bad);
        assert!(COUNTERS[COMBINED_PEAK].load(Ordering::Relaxed) <= PAGE_CACHE_BYTES as u64);
        let pages = std::sync::Arc::try_unwrap(pages).ok().unwrap();
        remove(path, file, pages);
        remove(otherpath, otherfile, other);
    }
}
fn peak(n: usize, value: usize) {
    COUNTERS[n].fetch_max(value as u64, Ordering::Relaxed);
}
fn mode() -> &'static str {
    static MODE: OnceLock<String> = OnceLock::new();
    MODE.get_or_init(|| {
        let value = std::env::var("FDB_TERM_CACHE_ARM").unwrap_or_else(|_| "K".into());
        assert!(
            matches!(value.as_str(), "K" | "A" | "D"),
            "unknown term cache arm"
        );
        value
    })
}
pub(super) fn large_cache() -> bool {
    mode() == "D"
}
pub(crate) fn enabled() -> bool {
    mode() == "A"
}
pub(crate) fn diagnostics() -> serde_json::Value {
    let names = [
        "page_hits",
        "page_misses",
        "capacity_pages",
        "oversized_pages",
        "drop_pages",
        "lookup_evictions",
        "lookup_hits",
        "lookup_misses",
        "recheck_hits",
        "serial_fallback",
        "inflight_peak_bytes",
        "combined_peak_bytes",
        "owner_created",
        "owner_dropped",
        "legacy_fallback",
    ];
    let mut out = serde_json::Map::new();
    for (i, name) in names.into_iter().enumerate() {
        out.insert(name.into(), COUNTERS[i].load(Ordering::Relaxed).into());
    }
    out.insert("arm".into(), mode().into());
    out.insert(
        "inflight_current_bytes".into(),
        INFLIGHT.load(Ordering::Relaxed).into(),
    );
    out.insert("page_limit_bytes".into(), page_cache_bytes().into());
    out.insert("lookup_limit_bytes".into(), LOOKUP_BYTES.into());
    serde_json::Value::Object(out)
}

thread_local! { static CANCEL: RefCell<Option<crate::CancelToken>> = const { RefCell::new(None) }; }
pub(crate) struct CancelScope(Option<crate::CancelToken>);
pub(crate) fn cancel_scope(token: Option<&crate::CancelToken>) -> CancelScope {
    CancelScope(CANCEL.with(|c| c.replace(token.cloned())))
}
impl Drop for CancelScope {
    fn drop(&mut self) {
        CANCEL.with(|c| {
            c.replace(self.0.take());
        });
    }
}
fn check() -> Result<()> {
    CANCEL.with(|c| crate::cancel::check(c.borrow().as_ref()))
}

static SLOT: SlotMutex<bool> = SlotMutex::new(false);
static WAKE: Condvar = Condvar::new();
struct DecodeSlot;
impl DecodeSlot {
    fn acquire() -> Result<Self> {
        let _span = crate::timing::span("query.term_decode_slot_wait");
        check()?;
        let mut busy = SLOT.lock().map_err(|_| Error::Poisoned)?;
        while *busy {
            check()?;
            busy = WAKE
                .wait_timeout(busy, Duration::from_millis(10))
                .map_err(|_| Error::Poisoned)?
                .0;
        }
        check()?;
        *busy = true;
        Ok(Self)
    }
}
impl Drop for DecodeSlot {
    fn drop(&mut self) {
        *SLOT.lock().unwrap_or_else(|e| e.into_inner()) = false;
        WAKE.notify_all();
    }
}
struct Flight;
impl Flight {
    fn new(bytes: usize, combined: usize) -> Self {
        let previous = INFLIGHT.swap(bytes as u64, Ordering::Relaxed);
        debug_assert_eq!(previous, 0);
        peak(FLIGHT_PEAK, bytes);
        peak(COMBINED_PEAK, combined);
        Self
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        INFLIGHT.store(0, Ordering::Relaxed);
    }
}
// Fixed charge includes slot/condvar/counter state. Vacant deque capacity is
// accounted separately. This is a controlled-byte bound, not process RSS.
const FIXED: usize = 512;
fn resident(cache: &Cache) -> usize {
    cache.bytes + cache.entries.capacity() * std::mem::size_of::<Entry>() + FIXED
}
fn evict(cache: &mut Cache) -> bool {
    if let Some(entry) = cache.entries.pop_back() {
        cache.bytes -= entry.bytes;
        count(CAPACITY_PAGES, 1);
        true
    } else {
        false
    }
}

impl Pages {
    // SEG05 creates exactly count tuples, and all copied string bytes are
    // disjoint subsets of the encoded page. Include raw AND decoded capacity.
    // Legacy serde allocation cannot be justified by this bound.
    fn isolated_bound(&self, page: usize) -> Option<usize> {
        if self.binary {
            let p = &self.fences[page];
            return p.len.checked_mul(2)?.checked_add(2)?.checked_add(
                p.count
                    .checked_mul(std::mem::size_of::<(Term, Posting)>())?,
            );
        }
        None
    }
    fn isolated_hit<T, F: FnOnce(&[(Term, Posting)]) -> T>(
        &self,
        cache: &mut Cache,
        page: usize,
        f: F,
    ) -> std::result::Result<T, F> {
        if let Some(pos) = cache
            .entries
            .iter()
            .position(|e| e.owner == self.owner && e.page == page)
        {
            count(PAGE_HITS, 1);
            crate::timing::count("query.term_page_hits", 1);
            let entry = cache.entries.remove(pos).unwrap();
            let out = f(&entry.values);
            cache.entries.push_front(entry);
            Ok(out)
        } else {
            Err(f)
        }
    }
    pub(super) fn with_page_isolated<T>(
        &self,
        page: usize,
        f: impl FnOnce(&[(Term, Posting)]) -> T,
    ) -> Result<T> {
        check()?;
        let f = {
            let mut cache = query_cache_lock()?;
            match self.isolated_hit(&mut cache, page, f) {
                Ok(out) => return Ok(out),
                Err(f) => f,
            }
        };
        count(PAGE_MISSES, 1);
        crate::timing::count("query.term_page_misses", 1);
        let _slot = DecodeSlot::acquire()?;
        let f = {
            let mut cache = query_cache_lock()?;
            match self.isolated_hit(&mut cache, page, f) {
                Ok(out) => {
                    count(RECHECK_HITS, 1);
                    return Ok(out);
                }
                Err(f) => f,
            }
        };
        let bound = self.isolated_bound(page);
        if bound.is_none_or(|n| {
            n.saturating_add(FIXED + std::mem::size_of::<Entry>()) > PAGE_CACHE_BYTES
        }) {
            count(FALLBACK, 1);
            if bound.is_none() {
                count(LEGACY_FALLBACK, 1);
            } else {
                count(OVERSIZED_PAGES, 1);
            }
            let mut cache = query_cache_lock()?;
            while evict(&mut cache) {}
            check()?;
            let values = self.decode(page)?;
            check()?;
            // The legacy/oversized exception stays serialized and uncached.
            // It is not represented as a newly hard-bounded normal-page path.
            return Ok(f(&values));
        }
        let bound = bound.unwrap();
        let flight = {
            let mut cache = query_cache_lock()?;
            // Allow old+new deque storage during reserve_exact, before allocating.
            if cache.entries.len() == cache.entries.capacity() {
                let metadata = (cache.entries.len() + 1) * std::mem::size_of::<Entry>();
                while resident(&cache)
                    .saturating_add(bound)
                    .saturating_add(metadata)
                    > PAGE_CACHE_BYTES
                {
                    if !evict(&mut cache) {
                        break;
                    }
                }
                cache.entries.reserve_exact(1);
            }
            while resident(&cache).saturating_add(bound) > PAGE_CACHE_BYTES {
                if !evict(&mut cache) {
                    cache.entries.shrink_to_fit();
                    if resident(&cache).saturating_add(bound) > PAGE_CACHE_BYTES {
                        return Err(Error::Invalid(
                            "term decode reservation exceeds allowance".into(),
                        ));
                    }
                }
            }
            Flight::new(bound, resident(&cache) + bound)
        };
        check()?;
        let values = self.decode(page)?;
        check()?;
        let bytes = values.capacity() * std::mem::size_of::<(Term, Posting)>()
            + values.iter().map(|(t, _)| strings(t)).sum::<usize>();
        if bytes > bound {
            return Err(Error::Corrupt("term decode bound".into()));
        }
        let result = f(&values);
        let mut cache = query_cache_lock()?;
        // Raw has been freed by decode(); transfer, don't double-charge values.
        drop(flight);
        while resident(&cache) + bytes > PAGE_CACHE_BYTES {
            if !evict(&mut cache) {
                break;
            }
        }
        if cache.entries.len() == cache.entries.capacity() {
            cache.entries.reserve_exact(1);
        }
        if resident(&cache) + bytes <= PAGE_CACHE_BYTES {
            cache.bytes += bytes;
            cache.entries.push_front(Entry {
                owner: self.owner,
                page,
                values,
                bytes,
            });
            peak(COMBINED_PEAK, resident(&cache));
        }
        Ok(result)
    }
}
