//! Existing DB, read-only queries. Does not commit/flush/compact. Normal open
//! may perform FDB's cache rebuild/torn WAL recovery; use a healthy benchmark DB.
#[path = "../tools/bench_support/allocation.rs"]
mod allocation;
use fdb::{Database, Query, Range};
use serde_json::json;

#[global_allocator]
static ALLOCATOR: allocation::Tracking = allocation::Tracking;

fn report(phase: &str, before: usize) {
    let (live, peak, calls) = allocation::snapshot();
    println!(
        "{}",
        json!({"phase":phase,"rust_requested_live_bytes":live,
        "rust_requested_peak_bytes":peak,"phase_additional_peak_bytes":peak.saturating_sub(before),
        "allocation_calls":calls,"includes_rss":false,"timing_is_instrumented":true})
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("profile_memory <healthy-benchmark-db>")?;
    let before = allocation::reset();
    let db = Database::open(path)?;
    report("open", before);
    println!(
        "{}",
        json!({"phase":"index_layout","not_total_heap_or_rss":true,
        "segments":db.index_memory_layout()?})
    );
    let mut exact = Query::new("shared");
    exact.collection = Some("contents".into());
    exact.exact.insert("source".into(), json!("synthetic"));
    let mut ranged = exact.clone();
    ranged.ranges.push(Range {
        field: "day".into(),
        min: 2,
        max: 5,
    });
    let mut text = Query::new("shared");
    text.collection = Some("contents".into());
    text.text = "teknoloji".into();
    for (name, query) in [
        ("exact_first_page", &exact),
        ("range_first_page", &ranged),
        ("text_first_page", &text),
    ] {
        let before = allocation::reset();
        let page = db.search(query)?;
        report(name, before);
        println!(
            "{}",
            json!({"phase":"verified_page","query":name,"total":page.total,
            "hits":page.hits.len(),"sequence":page.sequence})
        );
    }
    exact.offset = 5000;
    let before = allocation::reset();
    let page = db.search(&exact)?;
    report("exact_large_offset_legacy_path", before);
    println!(
        "{}",
        json!({"phase":"verified_page","query":"exact_large_offset_legacy_path",
        "total":page.total,"hits":page.hits.len(),"sequence":page.sequence})
    );
    drop(page);
    let before = allocation::reset();
    drop(db);
    report("closed", before);
    Ok(())
}
