//! Opt-in wall-time diagnostics. Nested/worker spans overlap; never sum them
//! as elapsed time or treat them as CPU attribution. Normal builds omit this.
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
pub mod maintenance;

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Counter {
    pub calls: u64,
    pub elapsed_ns: u64,
    pub before_drain_ns: u64,
    pub after_drain_ns: u64,
    pub started_before_drain: u64,
}
fn drain_boundary() -> &'static Mutex<Option<Instant>> {
    static BOUNDARY: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    BOUNDARY.get_or_init(Mutex::default)
}
/// A single ingest/drain boundary for an isolated diagnostic process. Completed
/// spans may arrive in a later snapshot; their wall portions remain explicit.
pub fn mark_drain() {
    let mut boundary = drain_boundary().lock().expect("timing boundary poisoned");
    assert!(boundary.is_none(), "drain boundary already marked");
    *boundary = Some(Instant::now());
}
fn counters() -> &'static Mutex<BTreeMap<&'static str, Counter>> {
    static VALUES: OnceLock<Mutex<BTreeMap<&'static str, Counter>>> = OnceLock::new();
    VALUES.get_or_init(Mutex::default)
}
pub struct Span {
    name: &'static str,
    start: Instant,
}
pub fn span(name: &'static str) -> Span {
    Span {
        name,
        start: Instant::now(),
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let end = Instant::now();
        let elapsed = end
            .duration_since(self.start)
            .as_nanos()
            .min(u64::MAX as u128) as u64;
        let boundary = *drain_boundary().lock().expect("timing boundary poisoned");
        let before = boundary.map_or(elapsed, |boundary| {
            boundary
                .min(end)
                .saturating_duration_since(self.start)
                .as_nanos()
                .min(u64::MAX as u128) as u64
        });
        let mut values = counters().lock().expect("timing mutex poisoned");
        let value = values.entry(self.name).or_default();
        value.calls = value.calls.saturating_add(1);
        value.elapsed_ns = value.elapsed_ns.saturating_add(elapsed);
        value.before_drain_ns = value.before_drain_ns.saturating_add(before);
        value.after_drain_ns = value.after_drain_ns.saturating_add(elapsed - before);
        value.started_before_drain +=
            u64::from(boundary.is_none_or(|boundary| self.start < boundary));
    }
}
/// Call after workers finish for a complete phase. Taking a snapshot while a
/// span is active records that span only in the next snapshot.
pub fn take() -> BTreeMap<&'static str, Counter> {
    std::mem::take(&mut *counters().lock().expect("timing mutex poisoned"))
}

pub(crate) fn count(name: &'static str, calls: usize) {
    let mut values = counters().lock().expect("timing mutex poisoned");
    let value = values.entry(name).or_default();
    value.calls = value.calls.saturating_add(calls as u64);
}

/// Sparse samples of the existing per-document gram loop. These are wall time
/// samples, not CPU attribution or an extrapolated whole-loop duration.
#[cfg(feature = "bounded-engine")]
#[derive(Default)]
pub(crate) struct GramInsertWork {
    pub grams: u64,
    pub samples: u64,
    pub lookup_ns: u64,
    pub insert_ns: u64,
    pub tiny: u64,
    pub sparse: u64,
    pub dense: u64,
    pub missing: u64,
    pub append: u64,
}
#[cfg(feature = "bounded-engine")]
impl Drop for GramInsertWork {
    fn drop(&mut self) {
        let mut values = counters().lock().expect("timing mutex poisoned");
        for (name, calls, elapsed_ns) in [
            ("index.gram_insert_inputs", self.grams, 0),
            ("index.sample_gram_lookup", self.samples, self.lookup_ns),
            (
                "index.sample_posting_insert",
                self.samples - self.missing,
                self.insert_ns,
            ),
            ("index.sample_posting_tiny", self.tiny, 0),
            ("index.sample_posting_sparse", self.sparse, 0),
            ("index.sample_posting_dense", self.dense, 0),
            ("index.sample_posting_missing", self.missing, 0),
            ("index.sample_posting_append", self.append, 0),
        ] {
            let value = values.entry(name).or_default();
            value.calls = value.calls.saturating_add(calls);
            value.elapsed_ns = value.elapsed_ns.saturating_add(elapsed_ns);
        }
    }
}

/// One aggregate publication per long-text extraction, never per scalar.
#[cfg(feature = "bounded-engine")]
#[derive(Default)]
pub(crate) struct GramWork {
    pub windows: u64,
    pub hits: u64,
    pub distinct: u64,
    pub tree_probes: u64,
}
#[cfg(feature = "bounded-engine")]
impl Drop for GramWork {
    fn drop(&mut self) {
        let mut values = counters().lock().expect("timing mutex poisoned");
        for (name, calls) in [
            ("index.gram_windows", self.windows),
            ("index.gram_cache_hits", self.hits),
            ("index.gram_cache_misses", self.windows - self.hits),
            ("index.gram_tree_probes", self.tree_probes),
            ("index.distinct_grams", self.distinct),
        ] {
            let value = values.entry(name).or_default();
            value.calls = value.calls.saturating_add(calls);
        }
    }
}

/// Per-segment query totals, published once to avoid a mutex per candidate.
#[derive(Default)]
pub(crate) struct PayloadReads {
    pub requests: u64,
    pub hits: u64,
    pub read_bytes: u64,
    pub requested_bytes: u64,
}
impl Drop for PayloadReads {
    fn drop(&mut self) {
        let mut values = counters().lock().expect("timing mutex poisoned");
        for (name, count) in [
            ("query.payload_read_requests", self.requests),
            ("query.payload_window_hits", self.hits),
            ("query.payload_read_bytes", self.read_bytes),
            ("query.payload_requested_bytes", self.requested_bytes),
        ] {
            let value = values.entry(name).or_default();
            value.calls = value.calls.saturating_add(count);
        }
    }
}

#[derive(Default)]
pub(crate) struct QueryWork {
    pub visible: u64,
    pub covered: u64,
    pub candidates: u64,
    pub accepted: u64,
    pub intersection_ns: u64,
}
impl Drop for QueryWork {
    fn drop(&mut self) {
        let mut values = counters().lock().expect("timing mutex poisoned");
        for (name, calls, elapsed_ns) in [
            ("query.visible_seed_ids", self.visible, 0),
            ("query.covered_candidates", self.covered, 0),
            ("query.payload_candidates", self.candidates, 0),
            ("query.accepted_ids", self.accepted, 0),
            (
                "query.posting_intersection",
                self.visible,
                self.intersection_ns,
            ),
        ] {
            let value = values.entry(name).or_default();
            value.calls = value.calls.saturating_add(calls);
            value.elapsed_ns = value.elapsed_ns.saturating_add(elapsed_ns);
        }
    }
}
