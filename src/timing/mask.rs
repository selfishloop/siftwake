//! One explicit mask-only comparison, never enabled by ordinary measurements.
use std::cell::{Cell, RefCell};
#[derive(serde::Serialize)]
pub struct Sample {
    pub reference_ns: u64,
    pub incremental_ns: u64,
    pub output_records: usize,
    pub bitwise_equal: bool,
}
thread_local! {
    static REQUESTED: Cell<bool> = const {Cell::new(false)};
    static RESULT: RefCell<Option<Sample>> = const {RefCell::new(None)};
}
pub fn begin() {
    assert!(RESULT.with(|r| r.borrow().is_none()));
    REQUESTED.with(|r| r.set(true));
}
pub(crate) fn requested() -> bool {
    REQUESTED.with(Cell::get)
}
pub(crate) fn record(sample: Sample) {
    assert!(REQUESTED.with(|r| r.replace(false)));
    RESULT.with(|r| *r.borrow_mut() = Some(sample));
}
pub fn take() -> Option<Sample> {
    REQUESTED.with(|r| r.set(false));
    RESULT.with(|r| r.borrow_mut().take())
}
