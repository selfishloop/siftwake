//! Opt-in wall-time diagnostics. Nested/worker spans overlap; never sum them
//! as elapsed time or treat them as CPU attribution. Normal builds omit this.
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Counter {
    pub calls: u64,
    pub elapsed_ns: u64,
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
        let elapsed = self.start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        let mut values = counters().lock().expect("timing mutex poisoned");
        let value = values.entry(self.name).or_default();
        value.calls = value.calls.saturating_add(1);
        value.elapsed_ns = value.elapsed_ns.saturating_add(elapsed);
    }
}
/// Call after workers finish for a complete phase. Taking a snapshot while a
/// span is active records that span only in the next snapshot.
pub fn take() -> BTreeMap<&'static str, Counter> {
    std::mem::take(&mut *counters().lock().expect("timing mutex poisoned"))
}

/// One aggregate publication per long-text extraction, never per scalar.
#[cfg(feature = "experimental-builder-budget")]
#[derive(Default)]
pub(crate) struct GramWork {
    pub windows: u64,
    pub hits: u64,
    pub distinct: u64,
}
#[cfg(feature = "experimental-builder-budget")]
impl Drop for GramWork {
    fn drop(&mut self) {
        let mut values = counters().lock().expect("timing mutex poisoned");
        for (name, calls) in [
            ("index.gram_windows", self.windows),
            ("index.gram_cache_hits", self.hits),
            ("index.gram_tree_probes", self.windows - self.hits),
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
