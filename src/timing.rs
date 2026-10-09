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
