//! Volatile publication only. Sources and codecs remain owned by the writer.
use super::*;
use std::sync::{
    Mutex, MutexGuard, RwLock, TryLockError, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Instant;
#[cfg(feature = "experimental-paged-metadata")]
thread_local! {static REFERENCE_MASKS:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}

#[derive(Clone)]
pub struct SnapshotReader {
    context: Arc<Context>,
}
#[derive(Clone)]
pub struct Snapshot {
    data: Arc<Data>,
}
#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct SnapshotAdmission {
    pub contended: bool,
    pub wait_ns: u64,
    pub elapsed_ns: u64,
}
#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct SnapshotRelease {
    pub writer_wait_ns: u64,
    pub max_writer_wait_ns: u64,
    pub cache_wait: crate::SnapshotReadWait,
    pub cleanup_ns: u64,
    pub reclaimed_files: usize,
    pub cleanup_error: Option<String>,
}
#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct SnapshotPublication {
    pub publications: u64,
    pub prepare_ns: u64,
    pub max_prepare_ns: u64,
    pub pointer_ns: u64,
    pub old_view_release_ns: u64,
    pub mask_full_rebuilds: u64,
    pub mask_incremental_publications: u64,
    pub mask_keys_scanned: u64,
    pub mask_prepare_ns: u64,
    pub wal_reader_open_ns: u64,
    pub state_clone_ns: u64,
}
#[derive(Default)]
struct Counters {
    publications: AtomicU64,
    prepare: AtomicU64,
    max_prepare: AtomicU64,
    pointer: AtomicU64,
    old_release: AtomicU64,
    mask_full: AtomicU64,
    mask_incremental: AtomicU64,
    mask_keys: AtomicU64,
    mask_ns: AtomicU64,
    wal_ns: AtomicU64,
    clone_ns: AtomicU64,
}
pub(super) struct Context {
    current: RwLock<Option<Arc<Data>>>,
    pub(super) gc: Arc<Reclaimer>,
    counters: Counters,
}
struct Pins {
    sources: Vec<(Kind, u64)>,
}
struct Data {
    view: Option<Database>,
    pins: Option<Arc<Pins>>,
    gc: Arc<Reclaimer>,
}
#[derive(Default)]
struct GcState {
    generation: u64,
    root: Vec<u8>,
    live: BTreeSet<(Kind, u64)>,
    retired: Vec<(Source, Weak<DiskIndex>)>,
    pins: Vec<Weak<Pins>>,
    pending: bool,
}
pub(super) struct Reclaimer {
    dir: PathBuf,
    _lock: File,
    state: Mutex<GcState>,
    writer_active: AtomicBool,
}
pub(super) struct WriterGuard<'a> {
    _guard: MutexGuard<'a, GcState>,
    gc: &'a Reclaimer,
}
impl Drop for WriterGuard<'_> {
    fn drop(&mut self) {
        self.gc.writer_active.store(false, Ordering::Release);
    }
}
impl Reclaimer {
    pub(super) fn writer_guard(&self) -> Result<WriterGuard<'_>> {
        let guard = self
            .state
            .lock()
            .map_err(|_| Error::Corrupt("snapshot GC mutex poisoned".into()))?;
        self.writer_active.store(true, Ordering::Release);
        Ok(WriterGuard {
            _guard: guard,
            gc: self,
        })
    }
    fn perform(&self, state: &mut GcState) -> Result<crate::ReclaimStats> {
        if state.generation == 0 || manifest::inventory(&self.dir)?.0 != Some(state.generation) {
            return Ok(Default::default());
        }
        if fs::read(manifest::path(&self.dir, state.generation, "ready"))? != state.root {
            return Err(Error::Corrupt(
                "snapshot reclaim publication changed".into(),
            ));
        }
        let mut live = state.live.clone();
        state.retired.retain(|(_, w)| w.strong_count() != 0);
        live.extend(state.retired.iter().map(|(s, _)| (s.kind, s.id)));
        state.pins.retain(|p| p.strong_count() != 0);
        for weak in &state.pins {
            if let Some(pin) = weak.upgrade() {
                live.extend(pin.sources.iter().copied());
            }
        }
        state.pending = live.iter().any(|id| !state.live.contains(id));
        crate::reclaim::collect(&self.dir, state.generation, &live)
    }
    pub(super) fn collect_writer(
        &self,
        generation: u64,
        live: BTreeSet<(Kind, u64)>,
        retired: Vec<(Source, Weak<DiskIndex>)>,
    ) -> Result<crate::ReclaimStats> {
        let mut guard = self.writer_guard()?;
        guard._guard.generation = generation;
        guard._guard.root = fs::read(manifest::path(&self.dir, generation, "ready"))?;
        guard._guard.live = live;
        guard._guard.retired = retired;
        self.perform(&mut guard._guard)
    }
    fn release(&self) -> SnapshotRelease {
        let start = Instant::now();
        let mut report = SnapshotRelease::default();
        let guard = match self.state.try_lock() {
            Ok(g) => Ok(g),
            Err(TryLockError::WouldBlock) => {
                let writer = self.writer_active.load(Ordering::Acquire);
                let waited = Instant::now();
                let g = self.state.lock();
                if writer {
                    report.writer_wait_ns = waited.elapsed().as_nanos() as u64;
                    report.max_writer_wait_ns = report.writer_wait_ns;
                }
                g
            }
            Err(TryLockError::Poisoned(e)) => Err(e),
        };
        match guard {
            Ok(mut state) => {
                if state.pending {
                    match self.perform(&mut state) {
                        Ok(s) => report.reclaimed_files = s.removed_files,
                        Err(e) => report.cleanup_error = Some(e.to_string()),
                    }
                }
            }
            Err(_) => report.cleanup_error = Some("snapshot GC mutex poisoned".into()),
        }
        report.cleanup_ns = start.elapsed().as_nanos() as u64;
        report
    }
}
impl Data {
    fn release(&mut self) -> SnapshotRelease {
        if self.view.is_none() {
            return Default::default();
        }
        drop(self.view.take());
        drop(self.pins.take());
        self.gc.release()
    }
}
impl Drop for Data {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
impl SnapshotReader {
    pub fn publication_stats(&self) -> SnapshotPublication {
        let c = &self.context.counters;
        SnapshotPublication {
            publications: c.publications.load(Ordering::Relaxed),
            prepare_ns: c.prepare.load(Ordering::Relaxed),
            max_prepare_ns: c.max_prepare.load(Ordering::Relaxed),
            pointer_ns: c.pointer.load(Ordering::Relaxed),
            old_view_release_ns: c.old_release.load(Ordering::Relaxed),
            mask_full_rebuilds: c.mask_full.load(Ordering::Relaxed),
            mask_incremental_publications: c.mask_incremental.load(Ordering::Relaxed),
            mask_keys_scanned: c.mask_keys.load(Ordering::Relaxed),
            mask_prepare_ns: c.mask_ns.load(Ordering::Relaxed),
            wal_reader_open_ns: c.wal_ns.load(Ordering::Relaxed),
            state_clone_ns: c.clone_ns.load(Ordering::Relaxed),
        }
    }
    pub fn acquire(&self) -> Result<Snapshot> {
        self.acquire_timed().map(|(s, _)| s)
    }
    /// Includes pointer-lock contention only; query/release timing is separate.
    pub fn acquire_timed(&self) -> Result<(Snapshot, SnapshotAdmission)> {
        let start = Instant::now();
        let mut admission = SnapshotAdmission::default();
        let guard = match self.context.current.try_read() {
            Ok(g) => g,
            Err(TryLockError::WouldBlock) => {
                admission.contended = true;
                let at = Instant::now();
                let g = self
                    .context
                    .current
                    .read()
                    .map_err(|_| Error::Corrupt("snapshot publication poisoned".into()))?;
                admission.wait_ns = at.elapsed().as_nanos() as u64;
                g
            }
            Err(_) => return Err(Error::Corrupt("snapshot publication poisoned".into())),
        };
        let data = guard
            .as_ref()
            .ok_or_else(|| Error::Invalid("snapshot has not been published".into()))?
            .clone();
        drop(guard);
        admission.elapsed_ns = start.elapsed().as_nanos() as u64;
        Ok((Snapshot { data }, admission))
    }
}
impl Snapshot {
    fn view(&self) -> &Database {
        self.data.view.as_ref().unwrap()
    }
    pub fn sequence(&self) -> u64 {
        self.view().sequence
    }
    pub fn generation(&self) -> u64 {
        self.view().generation
    }
    pub fn document_count(&self) -> usize {
        self.view().count
    }
    pub fn get(&self, key: &Key) -> Result<Option<StoredDocument>> {
        self.view().get(key)
    }
    pub fn search(&self, query: &Query) -> Result<SearchPage> {
        self.search_with_waits(query).map(|(page, _)| page)
    }
    #[cfg(feature = "bounded-engine")]
    pub fn search_first(&self, query: &Query) -> Result<crate::FirstPage> {
        self.view().search_first(query)
    }
    #[cfg(feature = "bounded-engine")]
    pub fn search_first_hybrid(&self, q: &Query, budget: usize) -> Result<crate::FirstPage> {
        self.view().search_first_hybrid(q, budget)
    }
    #[cfg(feature = "bounded-engine")]
    pub fn search_first_cancellable(
        &self,
        q: &Query,
        e: crate::SearchExecution,
        token: &crate::CancelToken,
    ) -> Result<crate::FirstPage> {
        self.view().search_first_cancellable(q, e, token)
    }
    #[cfg(feature = "bounded-engine")]
    pub fn search_first_hybrid_cancellable(
        &self,
        q: &Query,
        budget: usize,
        token: &crate::CancelToken,
    ) -> Result<crate::FirstPage> {
        self.view()
            .search_first_hybrid_cancellable(q, budget, token)
    }
    #[cfg(feature = "bounded-engine")]
    pub fn search_cancellable(
        &self,
        q: &Query,
        e: crate::SearchExecution,
        token: &crate::CancelToken,
    ) -> Result<SearchPage> {
        self.view().search_cancellable(q, e, token)
    }
    #[cfg(feature = "bounded-engine")]
    pub fn search_first_with_execution(
        &self,
        query: &Query,
        execution: crate::SearchExecution,
    ) -> Result<crate::FirstPage> {
        self.view().search_first_with_execution(query, execution)
    }
    #[cfg(feature = "bounded-engine")]
    pub fn search_with_execution(
        &self,
        query: &Query,
        execution: crate::SearchExecution,
    ) -> Result<SearchPage> {
        self.view().search_with_execution(query, execution)
    }
    pub fn search_with_waits(
        &self,
        query: &Query,
    ) -> Result<(SearchPage, crate::SnapshotReadWait)> {
        let tracking = crate::access::ReadScope::enter();
        let page = self.view().search(query)?;
        Ok((page, tracking.finish()))
    }
    #[cfg(feature = "bounded-engine")]
    pub fn search_into(&self, query: &Query, page: &mut SearchPage) -> Result<()> {
        self.view().search_into(query, page)
    }
    /// Ordinary Drop has identical cleanup semantics. Explicit release exposes
    /// the last-reader GC wait and I/O, so benchmarks cannot hide that latency.
    pub fn release(self) -> SnapshotRelease {
        let tracking = crate::access::ReadScope::enter();
        // into_inner atomically decrements the reference count. With try_unwrap,
        // two concurrent unsuccessful attempts could both drop their Arc and
        // one perform final-reader GC in the unreported Err branch.
        let mut report = match Arc::into_inner(self.data) {
            Some(mut data) => data.release(),
            None => SnapshotRelease::default(),
        };
        report.cache_wait = tracking.finish();
        report.writer_wait_ns += report.cache_wait.writer_wait_ns;
        report.max_writer_wait_ns = report
            .max_writer_wait_ns
            .max(report.cache_wait.max_writer_wait_ns);
        report
    }
}
impl Database {
    /// Volatile, calling-thread-only benchmark control. No persisted setting;
    /// ordinary callers keep replacement-mask reuse enabled.
    #[cfg(feature = "experimental-paged-metadata")]
    #[doc(hidden)]
    pub fn snapshot_mask_reference_for_diagnostics(enabled: bool) -> impl Drop {
        struct Restore(bool, std::marker::PhantomData<std::rc::Rc<()>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                REFERENCE_MASKS.with(|s| s.set(self.0));
            }
        }
        Restore(
            REFERENCE_MASKS.with(|s| s.replace(enabled)),
            std::marker::PhantomData,
        )
    }
    #[cfg(feature = "experimental-paged-metadata")]
    fn hide_snapshot_shadow<'a>(
        &self,
        masks: &mut [Vec<u64>],
        keys: impl Iterator<Item = &'a DeltaKey>,
        start: usize,
    ) -> Result<()> {
        for key in keys {
            for (run, mask) in self.runs.iter().zip(masks.iter_mut()).skip(start) {
                if let Some(slot) = run.index.find(key)? {
                    mask[slot as usize / 64] |= 1u64 << (slot % 64);
                }
            }
        }
        Ok(())
    }
    pub fn snapshot_publication_stats(&self) -> Option<SnapshotPublication> {
        self.snapshot_context
            .as_ref()
            .map(|c| SnapshotReader { context: c.clone() }.publication_stats())
    }
    #[cfg(feature = "experimental-paged-metadata")]
    fn snapshot_masks(&self) -> Result<Vec<Vec<u64>>> {
        // Unchanged immutable prefixes keep their resolved visibility. Only
        // appended runs can hide more keys; active-WAL shadowing is separate.
        if let Some(context) = &self.snapshot_context {
            let previous = context
                .current
                .read()
                .map_err(|_| Error::Corrupt("snapshot publication poisoned".into()))?
                .clone();
            if let Some(previous) = previous {
                let view = previous.view.as_ref().unwrap();
                if self.runs.len() >= view.runs.len()
                    && self
                        .runs
                        .iter()
                        .zip(&view.runs)
                        .all(|(a, b)| a.source == b.source)
                {
                    let cache = view
                        .mask_cache
                        .lock()
                        .map_err(|_| Error::Corrupt("visibility cache poisoned".into()))?;
                    let mut masks: Vec<_> = self
                        .runs
                        .iter()
                        .map(|r| vec![0u64; r.index.len().div_ceil(64)])
                        .collect();
                    let complete = view.runs.iter().zip(&mut masks).all(|(r, m)| {
                        cache.includes_shadow(r.source.id) && cache.apply(r.source.id, m)
                    });
                    drop(cache);
                    if complete {
                        let mut scanned = 0u64;
                        for n in view.runs.len()..self.runs.len() {
                            let mut cursor = crate::segment::KeyCursor::default();
                            for id in 0..self.runs[n].index.len() {
                                let key =
                                    self.runs[n].index.snapshot_key(id as u32, &mut cursor)?;
                                scanned += 1;
                                for (run, mask) in self.runs[..n].iter().zip(&mut masks[..n]) {
                                    if let Some(slot) = run.index.find(&key)? {
                                        mask[slot as usize / 64] |= 1u64 << (slot % 64);
                                    }
                                }
                            }
                        }
                        self.hide_snapshot_shadow(
                            &mut masks,
                            self.shadow.difference(&view.shadow),
                            0,
                        )?;
                        self.hide_snapshot_shadow(
                            &mut masks,
                            self.shadow.intersection(&view.shadow),
                            view.runs.len(),
                        )?;
                        context
                            .counters
                            .mask_incremental
                            .fetch_add(1, Ordering::Relaxed);
                        context
                            .counters
                            .mask_keys
                            .fetch_add(scanned, Ordering::Relaxed);
                        return Ok(masks);
                    }
                }
                #[cfg(feature = "diagnostic-timing")]
                let reference = if crate::timing::mask::requested() {
                    let at = Instant::now();
                    let masks = self.full_snapshot_masks()?;
                    Some((masks, at.elapsed().as_nanos() as u64))
                } else {
                    None
                };
                #[cfg(feature = "diagnostic-timing")]
                let incremental_at = Instant::now();
                if let Some(masks) = self.replaced_snapshot_masks(view)? {
                    #[cfg(feature = "diagnostic-timing")]
                    if let Some((full, ns)) = reference {
                        let elapsed = incremental_at.elapsed().as_nanos() as u64;
                        assert_eq!(masks, full, "sampled replacement mask reference");
                        crate::timing::mask::record(crate::timing::mask::Sample {
                            reference_ns: ns,
                            incremental_ns: elapsed,
                            output_records: self.runs.iter().map(|r| r.index.len()).sum(),
                            bitwise_equal: true,
                        });
                    }
                    #[cfg(any(test, debug_assertions))]
                    assert_eq!(
                        masks,
                        self.full_snapshot_masks()?,
                        "replacement masks must match complete rebuild bit-for-bit"
                    );
                    return Ok(masks);
                }
            }
            context.counters.mask_full.fetch_add(1, Ordering::Relaxed);
            context.counters.mask_keys.fetch_add(
                self.runs.iter().map(|r| r.index.len() as u64).sum(),
                Ordering::Relaxed,
            );
        }
        self.full_snapshot_masks()
    }
    /// Reuse unchanged sides of one contiguous, union-preserving replacement.
    /// Keys (including tombstones) and output order are checked, never inferred
    /// from a record count. Rewrites that discard keys use the full reference.
    #[cfg(feature = "experimental-paged-metadata")]
    fn replaced_snapshot_masks(&self, previous: &Database) -> Result<Option<Vec<Vec<u64>>>> {
        if REFERENCE_MASKS.with(std::cell::Cell::get) {
            return Ok(None);
        }
        if self.shadow != previous.shadow {
            return Ok(None);
        }
        let prefix = self
            .runs
            .iter()
            .zip(&previous.runs)
            .take_while(|(a, b)| a.source == b.source)
            .count();
        let suffix = self.runs[prefix..]
            .iter()
            .rev()
            .zip(previous.runs[prefix..].iter().rev())
            .take_while(|(a, b)| a.source == b.source)
            .count();
        let old_end = previous.runs.len() - suffix;
        let new_end = self.runs.len() - suffix;
        if new_end != prefix + 1 || old_end <= prefix || old_end - prefix > 4 {
            return Ok(None);
        }
        let cache = previous
            .mask_cache
            .lock()
            .map_err(|_| Error::Corrupt("visibility cache poisoned".into()))?;
        let mut old: Vec<_> = previous
            .runs
            .iter()
            .map(|r| vec![0u64; r.index.len().div_ceil(64)])
            .collect();
        if !previous
            .runs
            .iter()
            .zip(&mut old)
            .all(|(r, m)| cache.includes_shadow(r.source.id) && cache.apply(r.source.id, m))
        {
            return Ok(None);
        }
        drop(cache);
        let output = &self.runs[prefix].index;
        let mut mask = vec![0u64; output.len().div_ceil(64)];
        let mut cursor = crate::segment::KeyCursor::default();
        let mut count = 0usize;
        for item in MergeKeys::new(&previous.runs[prefix..old_end])? {
            let (input, slot) = item?;
            if count == output.len()
                || output.snapshot_key(count as u32, &mut cursor)?
                    != previous.runs[prefix + input].index.key(slot)?
            {
                return Ok(None);
            }
            if old[prefix + input][slot as usize / 64] & (1u64 << (slot % 64)) != 0 {
                mask[count / 64] |= 1u64 << (count % 64);
            }
            count += 1;
        }
        if count != output.len() {
            return Ok(None);
        }
        let scanned = previous.runs[prefix..old_end]
            .iter()
            .map(|r| r.index.len() as u64)
            .sum::<u64>()
            + count as u64;
        let mut masks = Vec::with_capacity(self.runs.len());
        masks.extend(old.drain(..prefix));
        masks.push(mask);
        masks.extend(old.drain(old_end - prefix..));
        if let Some(context) = &self.snapshot_context {
            context
                .counters
                .mask_incremental
                .fetch_add(1, Ordering::Relaxed);
            context
                .counters
                .mask_keys
                .fetch_add(scanned, Ordering::Relaxed);
        }
        Ok(Some(masks))
    }
    #[cfg(feature = "experimental-paged-metadata")]
    fn full_snapshot_masks(&self) -> Result<Vec<Vec<u64>>> {
        let mut cursors: Vec<_> = std::iter::repeat_with(crate::segment::KeyCursor::default)
            .take(self.runs.len())
            .collect();
        let mut masks: Vec<_> = self
            .runs
            .iter()
            .map(|r| {
                let len = r.index.len();
                let mut bits = vec![u64::MAX; len.div_ceil(64)];
                if len % 64 != 0 {
                    *bits.last_mut().unwrap() = (1u64 << (len % 64)) - 1;
                }
                bits
            })
            .collect();
        let mut heap = BinaryHeap::new();
        for (n, run) in self.runs.iter().enumerate() {
            if run.index.len() != 0 {
                heap.push(Reverse((
                    run.index.snapshot_key(0, &mut cursors[n])?,
                    n,
                    0u32,
                )));
            }
        }
        while let Some(Reverse((key, n, id))) = heap.pop() {
            let mut same = vec![(n, id)];
            while heap
                .peek()
                .is_some_and(|Reverse((next, _, _))| next == &key)
            {
                let Reverse((_, n, id)) = heap.pop().unwrap();
                same.push((n, id));
            }
            let &(selected, slot) = same.iter().max_by_key(|(n, _)| n).unwrap();
            masks[selected][slot as usize / 64] &= !(1u64 << (slot % 64));
            for (n, id) in same {
                let next = id + 1;
                if (next as usize) < self.runs[n].index.len() {
                    heap.push(Reverse((
                        self.runs[n].index.snapshot_key(next, &mut cursors[n])?,
                        n,
                        next,
                    )));
                }
            }
        }
        self.hide_snapshot_shadow(&mut masks, self.shadow.iter(), 0)?;
        Ok(masks)
    }
    /// Enable volatile snapshot publication. Existing APIs remain exclusive to
    /// the writer; readers use a separately cloned SnapshotReader.
    pub fn snapshot_reader(&mut self) -> Result<SnapshotReader> {
        self.healthy()?;
        self.ensure_sources()?;
        if self.snapshot_context.is_none() {
            let gc = Arc::new(Reclaimer {
                dir: self.dir.clone(),
                _lock: self._lock.try_clone()?,
                state: Mutex::default(),
                writer_active: AtomicBool::new(false),
            });
            self.snapshot_context = Some(Arc::new(Context {
                current: RwLock::new(None),
                gc,
                counters: Counters::default(),
            }));
            if let Err(e) = self
                .refresh_snapshots()
                .and_then(|_| self.collect_obsolete().map(|_| ()))
            {
                self.snapshot_context = None;
                return Err(e);
            }
        }
        Ok(SnapshotReader {
            context: self.snapshot_context.as_ref().unwrap().clone(),
        })
    }
    fn snapshot_view(&self) -> Result<Database> {
        let _writer = crate::access::WriterScope::enter();
        self.healthy()?;
        self.ensure_sources()?;
        let phase = Instant::now();
        #[cfg(feature = "experimental-paged-metadata")]
        let mask_cache = {
            let masks = self.snapshot_masks()?;
            let mut cache = MaskCache::default();
            for (run, mask) in self.runs.iter().zip(masks) {
                cache.remember_dense(run.source.id, mask);
            }
            cache.shadow_included = true;
            cache
        };
        let mask_ns = phase.elapsed().as_nanos() as u64;
        #[cfg(feature = "bounded-engine")]
        if self.write_profile.is_some() {
            profile::record("snapshot.mask", self.sequence, self.active.id, mask_ns);
        }
        let phase = Instant::now();
        // Keep a READ-ONLY WAL handle independent from the writer's handle.
        // Duplicating the previous read handle avoids a pathname reopen on
        // every commit. All snapshot WAL reads use explicit offsets; no reader
        // relies on its file cursor or aliases the writer's append cursor.
        let previous = self
            .snapshot_context
            .as_ref()
            .map(|context| {
                context
                    .current
                    .read()
                    .map(|g| g.clone())
                    .map_err(|_| Error::Corrupt("snapshot publication poisoned".into()))
            })
            .transpose()?
            .flatten();
        let log = match previous
            .as_ref()
            .and_then(|p| p.view.as_ref())
            .filter(|v| v.active.id == self.active.id)
        {
            Some(view) => {
                #[cfg(feature = "bounded-engine")]
                let _phase = profile::phase(
                    self.write_profile.is_some(),
                    "snapshot.wal_clone",
                    self.sequence,
                    self.active.id,
                );
                view.log.try_clone()?
            }
            None => {
                #[cfg(feature = "bounded-engine")]
                let _phase = profile::phase(
                    self.write_profile.is_some(),
                    "snapshot.wal_open",
                    self.sequence,
                    self.active.id,
                );
                File::open(manifest::path(&self.dir, self.active.id, "fdb"))?
            }
        };
        let wal_ns = phase.elapsed().as_nanos() as u64;
        #[cfg(feature = "bounded-engine")]
        if self.write_profile.is_some() {
            profile::record(
                "snapshot.wal_prepare",
                self.sequence,
                self.active.id,
                wal_ns,
            );
        }
        let phase = Instant::now();
        let view = Database {
            dir: self.dir.clone(),
            _lock: self._lock.try_clone()?,
            log,
            active: self.active.clone(),
            generation: self.generation,
            next_id: self.next_id,
            sequence: self.sequence,
            count: self.count,
            runs: self.runs.clone(),
            retired: Vec::new(),
            docs: self.docs.clone(),
            #[cfg(all(
                feature = "experimental-shared-documents",
                not(feature = "experimental-wal-delta")
            ))]
            doc_cache: crate::shared_documents::Cache::default(),
            index: self.index.snapshot_clone(),
            shadow: self.shadow.clone(),
            delta_bytes: self.delta_bytes,
            options: self.options.clone(),
            job: None,
            background_revision: background_revision::State::default(),
            revision_reclamation: false,
            revision_responsive: self.revision_responsive,
            query_visibility_pruning: self.query_visibility_pruning,
            maintenance_activity: self.maintenance_activity.clone(),
            poisoned: false,
            #[cfg(feature = "experimental-wal-sync-control")]
            sync_wal: true,
            #[cfg(feature = "experimental-paged-metadata")]
            mask_cache: Mutex::new(mask_cache),
            snapshot_context: None,
            #[cfg(feature = "bounded-engine")]
            light: self.light.clone(),
            #[cfg(feature = "bounded-engine")]
            streamed_wal_layer: self.streamed_wal_layer,
            #[cfg(feature = "bounded-engine")]
            light_flush: None,
            #[cfg(feature = "bounded-engine")]
            write_profile: None,
        };
        #[cfg(feature = "bounded-engine")]
        if self.write_profile.is_some() {
            profile::record(
                "snapshot.state_clone",
                self.sequence,
                self.active.id,
                phase.elapsed().as_nanos() as u64,
            );
        }
        if let Some(context) = &self.snapshot_context {
            context
                .counters
                .mask_ns
                .fetch_add(mask_ns, Ordering::Relaxed);
            context.counters.wal_ns.fetch_add(wal_ns, Ordering::Relaxed);
            context
                .counters
                .clone_ns
                .fetch_add(phase.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
        Ok(view)
    }
    pub(super) fn refresh_snapshots(&mut self) -> Result<()> {
        let Some(context) = self.snapshot_context.clone() else {
            return Ok(());
        };
        let begin = Instant::now();
        let view = match self.snapshot_view() {
            Ok(v) => v,
            Err(e) => {
                self.poisoned = true;
                return Err(e);
            }
        };
        let mut sources: Vec<_> = view
            .runs
            .iter()
            .map(|r| (r.source.kind, r.source.id))
            .collect();
        sources.push((Kind::Legacy, view.active.id));
        let pins = Arc::new(Pins { sources });
        {
            let mut state = context
                .gc
                .state
                .lock()
                .map_err(|_| Error::Corrupt("snapshot GC mutex poisoned".into()))?;
            state.pins.retain(|p| p.strong_count() != 0);
            state.pins.push(Arc::downgrade(&pins));
        }
        let data = Arc::new(Data {
            view: Some(view),
            pins: Some(pins),
            gc: context.gc.clone(),
        });
        let preparation = begin.elapsed().as_nanos() as u64;
        #[cfg(feature = "bounded-engine")]
        if self.write_profile.is_some() {
            profile::record(
                "snapshot.prepare",
                self.sequence,
                self.active.id,
                preparation,
            );
        }
        let at = Instant::now();
        let old = context
            .current
            .write()
            .map_err(|_| Error::Corrupt("snapshot publication poisoned".into()))?
            .replace(data);
        let pointer = at.elapsed().as_nanos() as u64;
        let release = Instant::now();
        drop(old);
        #[cfg(feature = "bounded-engine")]
        if self.write_profile.is_some() {
            profile::record("snapshot.pointer", self.sequence, self.active.id, pointer);
            profile::record(
                "snapshot.old_release",
                self.sequence,
                self.active.id,
                release.elapsed().as_nanos() as u64,
            );
        }
        context
            .counters
            .prepare
            .fetch_add(preparation, Ordering::Relaxed);
        context
            .counters
            .max_prepare
            .fetch_max(preparation, Ordering::Relaxed);
        context
            .counters
            .pointer
            .fetch_add(pointer, Ordering::Relaxed);
        context
            .counters
            .old_release
            .fetch_add(release.elapsed().as_nanos() as u64, Ordering::Relaxed);
        context
            .counters
            .publications
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(all(test, feature = "experimental-paged-metadata"))]
mod mask_tests;
#[cfg(all(feature = "diagnostic-timing", feature = "bounded-engine"))]
mod probe;
#[cfg(test)]
mod tests;
