//! Opt-in first page and bounded exact-query experiments. No persistent state.
use super::*;
use crate::CancelToken;
use crate::segment::first::{Candidates, Verifier};
use crate::{FirstPage, MatchTotal, SearchExecution, ValidationStatus};
use std::sync::{Condvar, Mutex, TryLockError};

thread_local! {static REFERENCE_WAIT:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
#[cfg(test)]
thread_local! {static FALLBACK_STEP:std::cell::RefCell<Option<Box<dyn FnOnce()>>>=const{std::cell::RefCell::new(None)};}
#[cfg(test)]
thread_local! {static WAIT_STEP:std::cell::RefCell<Option<Box<dyn FnOnce()>>>=const{std::cell::RefCell::new(None)};}

static WORKERS: Mutex<usize> = Mutex::new(4);
static AVAILABLE: Condvar = Condvar::new();
#[cfg(test)]
pub(super) static EXCLUSIVE_PERMIT_TEST: Mutex<()> = Mutex::new(());
pub(super) struct Permits(pub(super) usize);
impl Permits {
    pub(super) fn try_acquire(
        requested: usize,
        token: Option<&CancelToken>,
    ) -> Result<Option<Self>> {
        crate::cancel::check(token)?;
        #[cfg(feature = "diagnostic-timing")]
        let _profile = crate::timing::span("first.permit_try");
        let mut free = match WORKERS.try_lock() {
            Ok(free) => free,
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Poisoned(_)) => return Err(Error::Poisoned),
        };
        crate::cancel::check(token)?;
        if *free == 0 {
            return Ok(None);
        }
        let n = requested.min(*free);
        *free -= n;
        #[cfg(feature = "diagnostic-timing")]
        crate::timing::count("first.permits_granted", n);
        Ok(Some(Self(n)))
    }
    pub(super) fn acquire(requested: usize) -> Result<Self> {
        Self::acquire_cancel(requested, None)
    }
    fn acquire_cancel(requested: usize, token: Option<&CancelToken>) -> Result<Self> {
        #[cfg(feature = "diagnostic-timing")]
        let _profile = crate::timing::span("first.permit_acquire");
        #[cfg(feature = "diagnostic-timing")]
        let mutex_wait = crate::timing::span("first.permit_mutex_wait");
        let mut free = WORKERS.lock().map_err(|_| Error::Poisoned)?;
        #[cfg(feature = "diagnostic-timing")]
        drop(mutex_wait);
        while *free == 0 {
            crate::cancel::check(token)?;
            #[cfg(test)]
            WAIT_STEP.with(|s| {
                if let Some(step) = s.borrow_mut().take() {
                    step();
                }
            });
            #[cfg(feature = "diagnostic-timing")]
            let _wait = crate::timing::span("first.permit_condvar_wait");
            free = if token.is_some() {
                AVAILABLE
                    .wait_timeout(free, std::time::Duration::from_millis(2))
                    .map_err(|_| Error::Poisoned)?
                    .0
            } else {
                AVAILABLE.wait(free).map_err(|_| Error::Poisoned)?
            };
        }
        crate::cancel::check(token)?;
        let n = requested.min(*free);
        #[cfg(feature = "diagnostic-timing")]
        crate::timing::count("first.permits_granted", n);
        *free -= n;
        Ok(Self(n))
    }
}
struct Frontier<'a> {
    cursors: Vec<Candidates<'a>>,
    heap: BinaryHeap<Reverse<(Key, usize, usize)>>,
    delta: Vec<Arc<Key>>,
    hits: Vec<StoredDocument>,
    verifier: Option<(usize, Verifier<'a>)>,
}
impl Drop for Permits {
    fn drop(&mut self) {
        if let Ok(mut free) = WORKERS.lock() {
            *free += self.0;
            AVAILABLE.notify_all();
        }
    }
}
fn validate(q: &Query, e: SearchExecution, first: bool) -> Result<()> {
    validate_key(&Key::new(
        &q.scope,
        q.collection.as_deref().unwrap_or("_"),
        "_",
    ))?;
    if !(1..=4).contains(&e.verification_workers) || e.coalesce_gap_bytes > 64 * 1024 {
        return Err(Error::Invalid(
            "query execution: workers1..4, coalescing gap<=64KiB".into(),
        ));
    }
    if q.limit == 0
        || q.limit > if first { 200 } else { 1000 }
        || q.text.len() > 4096
        || q.exact.len() > 256
        || q.ranges.len() > 32
        || q.offset.checked_add(q.limit).is_none_or(|n| n > 4096)
        || (first && q.offset != 0)
    {
        return Err(Error::Invalid("first/experimental query limits".into()));
    }
    if q.exact.values().any(|v| v.is_array() || v.is_object())
        || q.ranges.iter().any(|r| r.min > r.max)
    {
        return Err(Error::Invalid("invalid query filters".into()));
    }
    if let Some(k) = &q.linked_to {
        validate_key(k)?;
        if k.scope != q.scope {
            return Err(Error::Invalid("cross-scope link query".into()));
        }
    }
    Ok(())
}
impl Database {
    /// Calling-thread-only diagnostic rollback; never persisted. True restores
    /// blocking admission, false selects the default single try/serial frontier.
    /// Exact/serial ignore this scope; dropping it restores the previous policy.
    #[doc(hidden)]
    pub fn hybrid_wait_reference_for_diagnostics(enabled: bool) -> impl Drop {
        struct Restore(bool, std::marker::PhantomData<std::rc::Rc<()>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                REFERENCE_WAIT.with(|s| s.set(self.0));
            }
        }
        Restore(
            REFERENCE_WAIT.with(|s| s.replace(enabled)),
            std::marker::PhantomData,
        )
    }
    fn hybrid_finish(
        &self,
        q: &Query,
        front: Frontier<'_>,
        token: Option<&CancelToken>,
        permits: Option<Permits>,
    ) -> Result<FirstPage> {
        #[cfg(feature = "diagnostic-timing")]
        crate::timing::count("first.hybrid_handoffs", 1);
        let Frontier {
            cursors,
            heap,
            delta,
            mut hits,
            mut verifier,
        } = front;
        let mut pending = vec![None; self.runs.len()];
        let mut delta_start = delta.len();
        for Reverse((_, n, id)) in heap {
            if n == self.runs.len() {
                delta_start = id;
            } else {
                pending[n] = Some(id as u32);
            }
        }
        struct Task<'a> {
            n: usize,
            cursor: Candidates<'a>,
            first: u32,
            verifier: Option<Verifier<'a>>,
        }
        let mut tasks = std::collections::VecDeque::new();
        for (n, cursor) in cursors.into_iter().enumerate() {
            if let Some(first) = pending[n] {
                let cached = if verifier.as_ref().is_some_and(|(old, _)| *old == n) {
                    verifier.take().map(|(_, v)| v)
                } else {
                    None
                };
                tasks.push_back(Task {
                    n,
                    cursor,
                    first,
                    verifier: cached,
                });
            }
        }
        let local_limit = q.limit - hits.len();
        let workers = permits.as_ref().map_or(1, |p| p.0);
        let tasks = Mutex::new(tasks);
        let work = || -> Result<Vec<(usize, Vec<u32>)>> {
            let mut result = Vec::new();
            loop {
                crate::cancel::check(token)?;
                let task = tasks.lock().map_err(|_| Error::Poisoned)?.pop_front();
                let Some(mut task) = task else {
                    break;
                };
                let index = &self.runs[task.n].index;
                let mut verify = match task.verifier.take() {
                    Some(v) => v,
                    None => Verifier::new_cancellable(index, q, SearchExecution::default(), token)?,
                };
                let mut id = Some(task.first);
                let mut accepted = Vec::new();
                while let Some(current) = id {
                    crate::cancel::check(token)?;
                    if verify.one(current)? {
                        accepted.push(current);
                        if accepted.len() == local_limit {
                            break;
                        }
                    }
                    id = task.cursor.next()?;
                }
                result.push((task.n, accepted));
            }
            Ok(result)
        };
        let parts = std::thread::scope(|scope| -> Result<Vec<(usize, Vec<u32>)>> {
            let handles: Vec<_> = (1..workers)
                .map(|_| {
                    #[cfg(feature = "diagnostic-timing")]
                    let request = crate::timing::request::current();
                    scope.spawn(move || {
                        #[cfg(feature = "diagnostic-timing")]
                        let _profile = crate::timing::request::attach(request);
                        work()
                    })
                })
                .collect();
            let mut result = work();
            for h in handles {
                match h.join() {
                    Ok(Ok(rows)) => {
                        if let Ok(all) = &mut result {
                            all.extend(rows);
                        }
                    }
                    Ok(Err(e)) => result = Err(e),
                    Err(_) => result = Err(Error::Poisoned),
                }
            }
            result
        })?;
        drop(permits);
        crate::cancel::check(token)?;
        let mut lists = vec![Vec::new(); self.runs.len()];
        for (n, ids) in parts {
            lists[n] = ids;
        }
        let mut next = BinaryHeap::new();
        for (n, ids) in lists.iter().enumerate() {
            if let Some(&id) = ids.first() {
                next.push(Reverse((self.runs[n].index.key(id)?, n, 0)));
            }
        }
        if let Some(k) = delta.get(delta_start) {
            next.push(Reverse((k.as_ref().clone(), self.runs.len(), delta_start)));
        }
        while let Some(Reverse((key, n, slot))) = next.pop() {
            crate::cancel::check(token)?;
            hits.push(if n == self.runs.len() {
                self.get(&key)?
                    .ok_or_else(|| Error::Corrupt("missing hybrid delta".into()))?
            } else {
                self.runs[n].index.read_selected_document(lists[n][slot])?
            });
            if hits.len() == q.limit {
                break;
            }
            let slot = slot + 1;
            let key = if n == self.runs.len() {
                delta.get(slot).map(|k| Ok(k.as_ref().clone()))
            } else {
                lists[n].get(slot).map(|&id| self.runs[n].index.key(id))
            };
            if let Some(key) = key {
                next.push(Reverse((key?, n, slot)));
            }
        }
        crate::cancel::check(token)?;
        Ok(FirstPage {
            total: if hits.len() == q.limit {
                MatchTotal::AtLeast(q.limit)
            } else {
                MatchTotal::Exact(hits.len())
            },
            sequence: self.sequence,
            hits,
            validation: ValidationStatus::Verified,
        })
    }
    fn first_delta(
        &self,
        q: &Query,
        token: Option<&CancelToken>,
        cap: Option<usize>,
    ) -> Result<Vec<Arc<Key>>> {
        #[cfg(feature = "diagnostic-timing")]
        let _profile = crate::timing::span("first.delta_filter");
        let Some(token) = token else {
            #[cfg(feature = "bounded-engine")]
            if self.docs.is_empty() && cap.is_some() {
                return self.light_matches_prefix(q, cap);
            }
            let mut keys = self.delta_matches(q)?;
            if let Some(n) = cap {
                // Owned slots are not in public key order. Cut only after sort.
                keys.sort_unstable();
                keys.truncate(n);
            }
            return Ok(keys);
        };
        let terms: Vec<_> = q.text.split_whitespace().map(crate::index::fold).collect();
        let encoded = q
            .exact
            .iter()
            .map(|(f, v)| Ok((f, serde_json::to_string(v)?)))
            .collect::<Result<Vec<_>>>()?;
        let mut out = Vec::new();
        for key in &self.shadow {
            token.check()?;
            if key.scope != q.scope || q.collection.as_ref().is_some_and(|c| c != &key.collection) {
                continue;
            }
            let logical = own_delta_key(key);
            let Some(stored) = self.get(&logical)? else {
                continue;
            };
            let doc = &stored.document;
            // The existing owned index keys exact scalars by their JSON
            // representation; the light layer uses Value equality. Preserve
            // both paths, including their existing signed-zero distinction.
            let light = self.light.contains_key(&logical);
            let mismatch = if light {
                q.exact.iter().any(|(f, v)| doc.fields.get(f) != Some(v))
            } else {
                let mut mismatch = false;
                for (field, wanted) in &encoded {
                    match doc.fields.get(*field) {
                        Some(actual) => {
                            if serde_json::to_string(actual)? != *wanted {
                                mismatch = true;
                                break;
                            }
                        }
                        None => {
                            mismatch = true;
                            break;
                        }
                    }
                }
                mismatch
            };
            if mismatch
                || q.ranges.iter().any(|r| {
                    doc.fields
                        .get(&r.field)
                        .and_then(|v| v.as_i64())
                        .is_none_or(|v| v < r.min || v > r.max)
                })
                || q.linked_to.as_ref().is_some_and(|k| !doc.links.contains(k))
            {
                continue;
            }
            let title = crate::index::fold(&doc.title);
            let text = crate::index::fold(&doc.text);
            if terms.iter().all(|t| title.contains(t) || text.contains(t)) {
                out.push(Arc::new(logical));
                if cap.is_some_and(|n| out.len() == n) {
                    break;
                }
            }
        }
        token.check()?;
        Ok(out)
    }
    fn first_ready(&self, token: Option<&CancelToken>) -> Result<()> {
        crate::cancel::check(token)?;
        self.healthy()?;
        if token.is_some() && self.pending_source_checks()? != 0 {
            return Err(Error::Invalid(
                "cancellable search requires already verified sources".into(),
            ));
        }
        self.ensure_sources()?;
        crate::cancel::check(token)
    }
    /// Serial first200 verified candidates, then continue remaining work on <=4 workers.
    pub fn search_first_hybrid(&self, q: &Query, serial_candidates: usize) -> Result<FirstPage> {
        self.search_first_mode(q, SearchExecution::default(), Some(serial_candidates), None)
    }
    pub fn search_first_hybrid_cancellable(
        &self,
        q: &Query,
        serial_candidates: usize,
        token: &CancelToken,
    ) -> Result<FirstPage> {
        self.search_first_mode(
            q,
            SearchExecution::default(),
            Some(serial_candidates),
            Some(token),
        )
    }
    pub fn search_first_cancellable(
        &self,
        q: &Query,
        e: SearchExecution,
        token: &CancelToken,
    ) -> Result<FirstPage> {
        self.search_first_mode(q, e, None, Some(token))
    }
    fn first_mask(&self, n: usize) -> Result<Vec<u64>> {
        #[cfg(feature = "diagnostic-timing")]
        let _profile = crate::timing::span("first.visibility_mask");
        let run = &self.runs[n];
        let mut mask = vec![0; run.index.len().div_ceil(64)];
        let (cached, shadow_included) = {
            let cache = self
                .mask_cache
                .lock()
                .map_err(|_| Error::Corrupt("visibility cache poisoned".into()))?;
            (
                cache.apply(run.source.id, &mut mask),
                cache.includes_shadow(run.source.id),
            )
        };
        if !cached {
            for later in &self.runs[n + 1..] {
                run.index.hide_keys(&later.index, &mut mask)?;
            }
            self.mask_cache
                .lock()
                .map_err(|_| Error::Poisoned)?
                .remember(run.source.id, &mask);
        }
        for key in self.shadow.iter().filter(|_| !cached || !shadow_included) {
            if let Some(id) = run.index.find(key)? {
                mask[id as usize / 64] |= 1u64 << (id % 64);
            }
        }
        Ok(mask)
    }
    /// First N in key order, offset0 and limit1..200. No exact-total tail scan.
    pub fn search_first(&self, q: &Query) -> Result<FirstPage> {
        self.search_first_with_execution(q, SearchExecution::default())
    }
    pub fn search_first_with_execution(&self, q: &Query, e: SearchExecution) -> Result<FirstPage> {
        self.search_first_mode(q, e, None, None)
    }
    fn search_first_mode(
        &self,
        q: &Query,
        e: SearchExecution,
        mut hybrid: Option<usize>,
        token: Option<&CancelToken>,
    ) -> Result<FirstPage> {
        #[cfg(feature = "diagnostic-timing")]
        let _profile = crate::timing::span("first.total");
        validate(q, e, true)?;
        if hybrid.is_some_and(|n| n == 0 || n > 65536) {
            return Err(Error::Invalid("hybrid candidate budget1..65536".into()));
        }
        if hybrid.is_none() && (e.verification_workers > 1 || e.coalesce_gap_bytes != 0) {
            return self.first_parallel(q, e, token);
        }
        let _reader = self.maintenance_activity.read();
        self.first_ready(token)?;
        let delta = self.first_delta(q, token, Some(q.limit))?;
        let masks = (0..self.runs.len())
            .map(|n| {
                crate::cancel::check(token)?;
                self.first_mask(n)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut budget = 8 * 1024 * 1024;
        let mut cursors = self
            .runs
            .iter()
            .zip(&masks)
            .map(|(r, m)| Candidates::new_cancellable(&r.index, q, m, &mut budget, token))
            .collect::<Result<Vec<_>>>()?;
        for cursor in &mut cursors {
            cursor.prepare_pages(&mut budget);
        }
        let mut heap = BinaryHeap::new();
        for (n, c) in cursors.iter_mut().enumerate() {
            if let Some(id) = c.next()? {
                heap.push(Reverse((c.key(id)?, n, id as usize)));
            }
        }
        if let Some(k) = delta.first() {
            heap.push(Reverse((k.as_ref().clone(), self.runs.len(), 0)));
        }
        let mut current = None;
        let mut verifier = None;
        let mut hits = Vec::with_capacity(q.limit);
        let mut verified = 0;
        #[cfg(feature = "diagnostic-timing")]
        let mut _serial_tail = None;
        #[cfg(test)]
        let mut fell_back = false;
        // The queued next candidate stays in the frontier at handoff; no ID
        // from the serial prefix is enumerated or verified again.
        while !heap.is_empty() {
            crate::cancel::check(token)?;
            if hybrid.is_some_and(|n| verified >= n) {
                let sources = heap
                    .iter()
                    .filter(|Reverse((_, n, _))| *n < self.runs.len())
                    .count();
                #[cfg(not(feature = "diagnostic-timing"))]
                let ceiling = 4;
                #[cfg(feature = "diagnostic-timing")]
                let ceiling = crate::timing::request::hybrid_workers();
                let permits = if sources == 0 {
                    None
                } else if REFERENCE_WAIT.with(std::cell::Cell::get) {
                    Some(Permits::acquire_cancel(ceiling.min(sources), token)?)
                } else {
                    Permits::try_acquire(ceiling.min(sources), token)?
                };
                if sources == 0 || permits.is_some() {
                    return self.hybrid_finish(
                        q,
                        Frontier {
                            cursors,
                            heap,
                            delta,
                            hits,
                            verifier: current.zip(verifier),
                        },
                        token,
                        permits,
                    );
                }
                // Exactly one try. Continue this same serial frontier without
                // per-worker verifiers, buffers or tasks; never try again.
                hybrid = None;
                #[cfg(test)]
                {
                    fell_back = true;
                }
                #[cfg(feature = "diagnostic-timing")]
                {
                    crate::timing::count("first.hybrid_serial_fallbacks", 1);
                    _serial_tail = Some(crate::timing::span("first.hybrid_serial_tail"));
                }
                crate::cancel::check(token)?;
            }
            let Reverse((key, n, id)) = heap.pop().unwrap();
            let matched = if n == self.runs.len() {
                true
            } else {
                if current != Some(n) {
                    verifier = Some(Verifier::new_cancellable(&self.runs[n].index, q, e, token)?);
                    current = Some(n);
                }
                verified += 1;
                let matched = verifier.as_mut().unwrap().one(id as u32)?;
                #[cfg(test)]
                if fell_back {
                    FALLBACK_STEP.with(|s| {
                        if let Some(step) = s.borrow_mut().take() {
                            step();
                        }
                    });
                }
                matched
            };
            if matched {
                hits.push(if n == self.runs.len() {
                    self.get(&key)?
                        .ok_or_else(|| Error::Corrupt("missing delta hit".into()))?
                } else {
                    self.runs[n].index.read_selected_document(id as u32)?
                });
                if hits.len() == q.limit {
                    crate::cancel::check(token)?;
                    return Ok(FirstPage {
                        total: MatchTotal::AtLeast(q.limit),
                        sequence: self.sequence,
                        hits,
                        validation: ValidationStatus::Verified,
                    });
                }
            }
            if n == self.runs.len() {
                if let Some(k) = delta.get(id + 1) {
                    heap.push(Reverse((k.as_ref().clone(), n, id + 1)));
                }
            } else if let Some(next) = cursors[n].next()? {
                heap.push(Reverse((cursors[n].key(next)?, n, next as usize)));
            }
        }
        crate::cancel::check(token)?;
        Ok(FirstPage {
            total: MatchTotal::Exact(hits.len()),
            sequence: self.sequence,
            hits,
            validation: ValidationStatus::Verified,
        })
    }

    /// Explicit experiments; ordinary exact search is unchanged.
    pub fn search_with_execution(&self, q: &Query, e: SearchExecution) -> Result<SearchPage> {
        validate(q, e, false)?;
        let (total, hits) = self.execute_prefixes(q, e, None, None)?;
        Ok(SearchPage {
            total,
            sequence: self.sequence,
            hits,
        })
    }
    pub fn search_cancellable(
        &self,
        q: &Query,
        e: SearchExecution,
        token: &CancelToken,
    ) -> Result<SearchPage> {
        validate(q, e, false)?;
        let (total, hits) = self.execute_prefixes(q, e, None, Some(token))?;
        Ok(SearchPage {
            total,
            sequence: self.sequence,
            hits,
        })
    }
    fn first_parallel(
        &self,
        q: &Query,
        e: SearchExecution,
        token: Option<&CancelToken>,
    ) -> Result<FirstPage> {
        let (_, hits) = self.execute_prefixes(q, e, Some(q.limit), token)?;
        Ok(FirstPage {
            total: if hits.len() == q.limit {
                MatchTotal::AtLeast(q.limit)
            } else {
                MatchTotal::Exact(hits.len())
            },
            sequence: self.sequence,
            hits,
            validation: ValidationStatus::Verified,
        })
    }
    // Parallel alternative uses bounded per-source prefixes. It stops local
    // verification at N but can do more work than the serial global cursor.
    fn execute_prefixes(
        &self,
        q: &Query,
        e: SearchExecution,
        cap: Option<usize>,
        token: Option<&CancelToken>,
    ) -> Result<(usize, Vec<StoredDocument>)> {
        let _reader = self.maintenance_activity.read();
        self.first_ready(token)?;
        let permits = Permits::acquire_cancel(e.verification_workers, token)?;
        let delta = self.first_delta(q, token, cap)?;
        let masks = (0..self.runs.len())
            .map(|n| {
                crate::cancel::check(token)?;
                self.first_mask(n)
            })
            .collect::<Result<Vec<_>>>()?;
        let prefix = q.offset + q.limit;
        let work = |worker: usize| -> Result<Vec<(usize, usize, Vec<u32>)>> {
            let mut result = Vec::new();
            for n in (worker..self.runs.len()).step_by(permits.0) {
                let index = &self.runs[n].index;
                let mut budget = 512 * 1024;
                let mut cursor =
                    Candidates::new_cancellable(index, q, &masks[n], &mut budget, token)?;
                cursor.prepare_pages(&mut budget);
                let mut verifier = Verifier::new_cancellable(index, q, e, token)?;
                let mut count = 0;
                let mut ids = Vec::new();
                loop {
                    crate::cancel::check(token)?;
                    let mut batch = Vec::with_capacity(128);
                    while batch.len() < 128 {
                        if let Some(id) = cursor.next()? {
                            batch.push(id);
                        } else {
                            break;
                        }
                    }
                    if batch.is_empty() {
                        break;
                    }
                    for (&id, matched) in batch.iter().zip(verifier.batch(&batch)?) {
                        if matched {
                            count += 1;
                            if ids.len() < prefix {
                                ids.push(id);
                            }
                        }
                        if cap.is_some_and(|limit| count >= limit) {
                            break;
                        }
                    }
                    if cap.is_some_and(|limit| count >= limit) {
                        break;
                    }
                }
                result.push((n, count, ids));
            }
            Ok(result)
        };
        let partials = std::thread::scope(|scope| -> Result<Vec<(usize, usize, Vec<u32>)>> {
            let mut handles = Vec::new();
            for n in 1..permits.0 {
                let work = &work;
                handles.push(scope.spawn(move || work(n)));
            }
            let mut result = work(0)?;
            for h in handles {
                result.extend(h.join().map_err(|_| Error::Poisoned)??);
            }
            Ok(result)
        })?;
        let mut total = delta.len();
        let mut lists = vec![Vec::new(); self.runs.len()];
        for (n, count, ids) in partials {
            total += count;
            lists[n] = ids;
        }
        let mut heap = BinaryHeap::new();
        for (n, ids) in lists.iter().enumerate() {
            if let Some(id) = ids.first() {
                heap.push(Reverse((self.runs[n].index.key(*id)?, n, 0)));
            }
        }
        if let Some(k) = delta.first() {
            heap.push(Reverse((k.as_ref().clone(), self.runs.len(), 0)));
        }
        let mut hits = Vec::with_capacity(q.limit);
        let mut position = 0;
        while let Some(Reverse((key, n, slot))) = heap.pop() {
            crate::cancel::check(token)?;
            if position >= q.offset {
                hits.push(if n == self.runs.len() {
                    self.get(&key)?
                        .ok_or_else(|| Error::Corrupt("missing delta hit".into()))?
                } else {
                    self.runs[n].index.read_selected_document(lists[n][slot])?
                });
                if hits.len() == q.limit {
                    break;
                }
            }
            position += 1;
            let next = slot + 1;
            let key = if n == self.runs.len() {
                delta.get(next).map(|k| Ok(k.as_ref().clone()))
            } else {
                lists[n].get(next).map(|id| self.runs[n].index.key(*id))
            };
            if let Some(k) = key {
                heap.push(Reverse((k?, n, next)));
            }
        }
        crate::cancel::check(token)?;
        Ok((total, hits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, Guard};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "fdb-first-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn put(id: usize, text: &str) -> Mutation {
        let mut fields = BTreeMap::new();
        fields.insert("n".into(), serde_json::json!(id % 3));
        Mutation::Put {
            document: Document {
                key: Key::new("shared", "texts", &format!("{id:05}")),
                title: String::new(),
                text: text.into(),
                fields,
                links: vec![Key::new("shared", "targets", "anchor")],
            },
            guard: Guard::Any,
        }
    }
    fn compare(db: &Database, q: &Query) {
        let exact = db.search(q).unwrap();
        for n in [1, 10, 50, 200] {
            let mut query = q.clone();
            query.limit = n;
            let expected = db.search(&query).unwrap();
            for budget in [1, 20, 200] {
                let page = db.search_first_hybrid(&query, budget).unwrap();
                assert_eq!(page.hits, expected.hits);
                assert_eq!(page.sequence, expected.sequence);
                let token = CancelToken::new();
                assert_eq!(
                    db.search_first_hybrid_cancellable(&query, budget, &token)
                        .unwrap()
                        .hits,
                    expected.hits
                );
            }
            for e in [
                SearchExecution::default(),
                SearchExecution {
                    verification_workers: 2,
                    ..Default::default()
                },
                SearchExecution {
                    verification_workers: 4,
                    ..Default::default()
                },
                SearchExecution {
                    coalesce_gap_bytes: 8192,
                    ..Default::default()
                },
                SearchExecution {
                    verification_workers: 4,
                    coalesce_gap_bytes: 8192,
                    reuse_validated_source_crc: true,
                },
            ] {
                let first = db.search_first_with_execution(&query, e).unwrap();
                let token = CancelToken::new();
                assert_eq!(
                    db.search_first_cancellable(&query, e, &token).unwrap().hits,
                    expected.hits
                );
                assert_eq!(
                    db.search_cancellable(&query, e, &token).unwrap().total,
                    expected.total
                );
                assert_eq!(
                    serde_json::to_value(&first.hits).unwrap(),
                    serde_json::to_value(&expected.hits).unwrap()
                );
                assert_eq!(first.sequence, expected.sequence);
                assert_eq!(
                    first.total,
                    if expected.total >= n {
                        MatchTotal::AtLeast(n)
                    } else {
                        MatchTotal::Exact(expected.total)
                    }
                );
                let checked = db.search_with_execution(&query, e).unwrap();
                assert_eq!(
                    serde_json::to_value(checked).unwrap(),
                    serde_json::to_value(expected.clone()).unwrap()
                );
            }
        }
        assert_eq!(exact.sequence, db.sequence);
    }
    #[test]
    fn capped_exact_delta_ranges_links_and_snapshot_keep_one_epoch_and_key_order() {
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        for bucket in 0..3 {
            db.commit(
                (0..60)
                    .filter(|n| n % 3 == bucket)
                    .map(|n| put(n, "needle fox tail"))
                    .collect(),
            )
            .unwrap();
            db.flush().unwrap();
        }
        let reader = db.snapshot_reader().unwrap();
        let old = reader.acquire().unwrap();
        db.set_streamed_wal_layer(true).unwrap();
        db.commit_streamed([
            Ok(Mutation::Delete {
                key: Key::new("shared", "texts", "00000"),
                guard: Guard::Any,
            }),
            Ok(put(4, "new_only secret")),
            Ok(put(61, "needle fox tail")),
        ])
        .unwrap();
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.text = "needle fox".into();
        compare(&db, &q);
        q.ranges.push(crate::Range {
            field: "n".into(),
            min: 0,
            max: 1,
        });
        q.linked_to = Some(Key::new("shared", "targets", "anchor"));
        compare(&db, &q);
        q.ranges.clear();
        q.linked_to = None;
        q.text = "absent needle".into();
        compare(&db, &q);
        q.text = "needle fox".into();
        let before = old.search(&q).unwrap();
        assert_eq!(before.total, 60);
        assert_eq!(old.search_first(&q).unwrap().total, MatchTotal::AtLeast(50));
        db.maintain_idle().unwrap();
        assert_eq!(
            serde_json::to_value(old.search_first(&q).unwrap().hits).unwrap(),
            serde_json::to_value(before.hits).unwrap()
        );
        drop(old);
        drop(reader);
        drop(db);
        let db = Database::open(&t.0).unwrap();
        compare(&db, &q);
    }
    #[test]
    fn first_page_lower_bound_is_honest_at_exact_limit_and_rejects_pagination() {
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        db.commit(vec![put(1, "needle")]).unwrap();
        db.flush().unwrap();
        let mut q = Query::new("shared");
        q.text = "needle".into();
        q.limit = 1;
        assert_eq!(db.search_first(&q).unwrap().total, MatchTotal::AtLeast(1));
        q.offset = 1;
        assert!(db.search_first(&q).is_err());
        q.offset = 0;
        q.limit = 201;
        assert!(db.search_first(&q).is_err());
        q.limit = 50;
        assert!(
            db.search_with_execution(
                &q,
                SearchExecution {
                    verification_workers: 5,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn cancelled_request_is_sticky_not_poisoned_and_cancels_admission() {
        let _exclusive = EXCLUSIVE_PERMIT_TEST.lock().unwrap();
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        db.commit(vec![put(1, "needle")]).unwrap();
        db.flush().unwrap();
        let mut q = Query::new("shared");
        q.text = "needle".into();
        let token = CancelToken::new();
        let same = token.clone();
        same.cancel();
        assert!(token.is_cancelled());
        assert!(matches!(
            db.search_first_hybrid_cancellable(&q, 200, &token),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            db.search_cancellable(&q, SearchExecution::default(), &token),
            Err(Error::Cancelled)
        ));
        assert_eq!(
            db.search_first_hybrid_cancellable(&q, 200, &CancelToken::new())
                .unwrap()
                .hits
                .len(),
            1
        );
        // acquire() grants up to the available count. Concurrent tests may
        // already own permits; hold all four before asserting admission waits.
        let mut permits = Vec::new();
        let mut held = 0;
        while held < 4 {
            let lease = Permits::acquire(4 - held).unwrap();
            held += lease.0;
            permits.push(lease);
        }
        let token = CancelToken::new();
        let worker_token = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            let worker = s.spawn(|| {
                tx.send(()).unwrap();
                db.search_first_cancellable(
                    &q,
                    SearchExecution {
                        verification_workers: 4,
                        ..Default::default()
                    },
                    &worker_token,
                )
            });
            rx.recv().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
            token.cancel();
            assert!(matches!(worker.join().unwrap(), Err(Error::Cancelled)));
        });
        drop(permits);
        assert_eq!(db.search(&q).unwrap().total, 1);
        db.check_integrity().unwrap();
    }
    #[test]
    fn cancellation_keeps_each_existing_delta_exact_float_semantics() {
        for light in [false, true] {
            let t = Temp::new();
            let mut db = Database::open(&t.0).unwrap();
            let mut m = put(1, "needle");
            if let Mutation::Put { document, .. } = &mut m {
                document.fields.insert("f".into(), serde_json::json!(0.0));
            }
            if light {
                db.set_streamed_wal_layer(true).unwrap();
                db.commit_streamed([Ok(m)]).unwrap();
            } else {
                db.commit(vec![m]).unwrap();
            }
            for value in [serde_json::json!(0.0), serde_json::json!(-0.0)] {
                let mut q = Query::new("shared");
                q.text = "needle".into();
                q.exact.insert("f".into(), value);
                let expected = db.search(&q).unwrap();
                let actual = db
                    .search_cancellable(&q, SearchExecution::default(), &CancelToken::new())
                    .unwrap();
                assert_eq!(actual.total, expected.total, "light={light}");
                assert_eq!(actual.hits, expected.hits);
                for page in [
                    db.search_first(&q).unwrap(),
                    db.search_first_hybrid_cancellable(&q, 200, &CancelToken::new())
                        .unwrap(),
                ] {
                    assert_eq!(page.hits, expected.hits, "first signed zero light={light}");
                }
            }
        }
    }
    #[test]
    fn hybrid_default_and_nested_diagnostic_rollback_are_thread_local() {
        assert!(!REFERENCE_WAIT.with(std::cell::Cell::get));
        {
            let _reference = Database::hybrid_wait_reference_for_diagnostics(true);
            assert!(REFERENCE_WAIT.with(std::cell::Cell::get));
            std::thread::spawn(|| assert!(!REFERENCE_WAIT.with(std::cell::Cell::get)))
                .join()
                .unwrap();
            {
                let _default = Database::hybrid_wait_reference_for_diagnostics(false);
                assert!(!REFERENCE_WAIT.with(std::cell::Cell::get));
            }
            assert!(REFERENCE_WAIT.with(std::cell::Cell::get));
        }
        assert!(!REFERENCE_WAIT.with(std::cell::Cell::get));
    }
    #[test]
    fn diagnostic_rollback_still_waits_cancels_and_returns_the_same_page() {
        let _exclusive = EXCLUSIVE_PERMIT_TEST.lock().unwrap();
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        let text = "abcdefghijklmnop";
        db.commit(
            (0..700)
                .map(|id| {
                    put(
                        id,
                        if id < 620 {
                            "abc bcd cde def efg fgh ghi hij ijk jkl klm lmn mno nop"
                        } else {
                            text
                        },
                    )
                })
                .collect(),
        )
        .unwrap();
        db.flush().unwrap();
        let mut q = Query::new("shared");
        q.text = text.into();
        q.limit = 50;
        let expected = db.search_first_hybrid(&q, 200).unwrap();
        let mut leases = vec![];
        let mut held = 0;
        while held < 4 {
            let lease = Permits::acquire(4 - held).unwrap();
            held += lease.0;
            leases.push(lease);
        }
        let token = CancelToken::new();
        std::thread::scope(|scope| {
            let (entered, waiting) = std::sync::mpsc::channel();
            let token_ref = &token;
            let db_ref = &db;
            let q_ref = &q;
            let worker = scope.spawn(move || {
                let _reference = Database::hybrid_wait_reference_for_diagnostics(true);
                WAIT_STEP
                    .with(|s| *s.borrow_mut() = Some(Box::new(move || entered.send(()).unwrap())));
                db_ref.search_first_hybrid_cancellable(q_ref, 200, token_ref)
            });
            waiting
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            token.cancel();
            assert!(matches!(worker.join().unwrap(), Err(Error::Cancelled)));
        });
        std::thread::scope(|scope| {
            let (entered, waiting) = std::sync::mpsc::channel();
            let db_ref = &db;
            let q_ref = &q;
            let worker = scope.spawn(move || {
                let _reference = Database::hybrid_wait_reference_for_diagnostics(true);
                WAIT_STEP
                    .with(|s| *s.borrow_mut() = Some(Box::new(move || entered.send(()).unwrap())));
                db_ref.search_first_hybrid(q_ref, 200)
            });
            waiting
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            drop(leases);
            let got = worker.join().unwrap().unwrap();
            assert_eq!(
                (got.hits, got.total, got.sequence),
                (expected.hits, expected.total, expected.sequence)
            );
        });
        assert!(!REFERENCE_WAIT.with(std::cell::Cell::get));
        db.check_integrity().unwrap();
    }
    #[test]
    fn handoff_try_is_nonblocking_on_mutex_and_preserves_cancel() {
        let held = WORKERS.lock().unwrap();
        assert!(Permits::try_acquire(1, None).unwrap().is_none());
        let token = CancelToken::new();
        token.cancel();
        assert!(matches!(
            Permits::try_acquire(1, Some(&token)),
            Err(Error::Cancelled)
        ));
        drop(held);
    }
    #[test]
    fn exhausted_hybrid_continues_same_frontier_with_no_permit_or_new_worker() {
        let _exclusive = EXCLUSIVE_PERMIT_TEST.lock().unwrap();
        assert!(!REFERENCE_WAIT.with(std::cell::Cell::get));
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        let word = "abcdefghijklmnop";
        let false_positive = "abc bcd cde def efg fgh ghi hij ijk jkl klm lmn mno nop";
        db.commit(
            (0..700)
                .map(|n| put(n, if n < 620 { false_positive } else { word }))
                .collect(),
        )
        .unwrap();
        db.flush().unwrap();
        let reader = db.snapshot_reader().unwrap();
        let old = reader.acquire().unwrap();
        db.set_streamed_wal_layer(true).unwrap();
        db.commit_streamed([
            Ok(put(7, word)),
            Ok(Mutation::Delete {
                key: Key::new("shared", "texts", "00650"),
                guard: Guard::Any,
            }),
        ])
        .unwrap();
        let mut q = Query::new("shared");
        q.text = word.into();
        let mut expected = vec![];
        for limit in [50, 200] {
            q.limit = limit;
            expected.push(db.search_first(&q).unwrap());
        }
        let old_expected = old.search_first(&q).unwrap();
        let mut leases = vec![];
        let mut held = 0;
        while held < 4 {
            let p = Permits::acquire(4 - held).unwrap();
            held += p.0;
            leases.push(p);
        }
        for (limit, wanted) in [50, 200].into_iter().zip(expected) {
            q.limit = limit;
            let progressed = std::rc::Rc::new(std::cell::Cell::new(false));
            let check = progressed.clone();
            FALLBACK_STEP.with(|s| *s.borrow_mut() = Some(Box::new(move || check.set(true))));
            let got = db
                .search_first_hybrid_cancellable(&q, 200, &CancelToken::new())
                .unwrap();
            assert!(progressed.get());
            assert_eq!(
                (got.hits, got.total, got.sequence),
                (wanted.hits, wanted.total, wanted.sequence)
            );
            assert_eq!(*WORKERS.lock().unwrap(), 0);
        }
        let token = CancelToken::new();
        let cancel = token.clone();
        FALLBACK_STEP.with(|s| *s.borrow_mut() = Some(Box::new(move || cancel.cancel())));
        assert!(matches!(
            db.search_first_hybrid_cancellable(&q, 200, &token),
            Err(Error::Cancelled)
        ));
        assert_eq!(*WORKERS.lock().unwrap(), 0);
        let got = old.search_first_hybrid(&q, 200).unwrap();
        assert_eq!(
            (got.hits, got.total, got.sequence),
            (old_expected.hits, old_expected.total, old_expected.sequence)
        );
        drop(leases);
        db.check_integrity().unwrap();
    }
    #[test]
    fn delta_prefix_caps_only_first_pages_and_preserves_full_shadow_and_old_snapshot() {
        for light in [false, true] {
            let t = Temp::new();
            let mut db = Database::open(&t.0).unwrap();
            db.commit((0..160).map(|id| put(id, "needle oldonly")).collect())
                .unwrap();
            db.flush().unwrap();
            let reader = db.snapshot_reader().unwrap();
            let old = reader.acquire().unwrap();
            let mut changes: Vec<_> = (80..240)
                .rev()
                .map(|id| put(id, "needle freshonly"))
                .collect();
            changes.extend((0..10).map(|id| Mutation::Delete {
                key: Key::new("shared", "texts", &format!("{id:05}")),
                guard: Guard::Any,
            }));
            if light {
                db.set_streamed_wal_layer(true).unwrap();
                db.commit_streamed(changes.into_iter().map(Ok)).unwrap();
            } else {
                db.commit(changes).unwrap();
            }
            for text in ["needle", "oldonly", "freshonly", "absent"] {
                let mut q = Query::new("shared");
                q.text = text.into();
                compare(&db, &q);
                assert_eq!(
                    db.first_delta(&q, None, Some(50)).unwrap(),
                    db.first_delta(&q, None, None)
                        .unwrap()
                        .into_iter()
                        .take(50)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    db.search_first(&q).unwrap().sequence,
                    reader.acquire().unwrap().sequence()
                );
            }
            let mut q = Query::new("shared");
            q.text = "oldonly".into();
            assert_eq!(db.search(&q).unwrap().total, 70);
            assert_eq!(old.search(&q).unwrap().total, 160);
            assert!(matches!(
                db.search_first_hybrid_cancellable(&q, 200, &{
                    let t = CancelToken::new();
                    t.cancel();
                    t
                }),
                Err(Error::Cancelled)
            ));
        }
    }
    #[test]
    fn capped_selected_wal_payload_still_checks_crc() {
        use std::io::{Read, Seek, SeekFrom, Write};
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        db.set_streamed_wal_layer(true).unwrap();
        db.commit_streamed((0..80).map(|n| Ok(put(n, "needle"))))
            .unwrap();
        let location = db.light.values().next().unwrap().location;
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(manifest::path(&t.0, db.active.id, "fdb"))
            .unwrap();
        file.seek(SeekFrom::Start(location.offset)).unwrap();
        let mut byte = [0];
        file.read_exact(&mut byte).unwrap();
        byte[0] ^= 1;
        file.seek(SeekFrom::Start(location.offset)).unwrap();
        file.write_all(&byte).unwrap();
        file.sync_all().unwrap();
        let mut q = Query::new("shared");
        q.text = "needle".into();
        assert!(matches!(db.search_first(&q), Err(Error::Corrupt(_))));
        assert!(matches!(
            db.search_first_hybrid_cancellable(&q, 200, &CancelToken::new()),
            Err(Error::Corrupt(_))
        ));
    }
}
