//! Requested Rust allocation diagnostics; timings with this allocator are invalid.
#[cfg(feature = "experimental-grouped-read")]
#[path = "../tools/bench_support/allocation.rs"]
mod allocation;
#[cfg(feature = "experimental-grouped-read")]
#[global_allocator]
static ALLOCATOR: allocation::Tracking = allocation::Tracking;

#[cfg(not(feature = "experimental-grouped-read"))]
fn main() {
    eprintln!("build with --features experimental-grouped-read");
    std::process::exit(1);
}

#[cfg(feature = "experimental-grouped-read")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use fdb::{Database, Query, Range};
    use serde_json::json;
    let path = std::env::args()
        .nth(1)
        .ok_or("profile_grouped_reads <healthy-db>")?;
    let db = Database::open(path)?;
    let mut exact = Query::new("shared");
    exact.collection = Some("contents".into());
    exact.limit = 50;
    exact.exact.insert("source".into(), json!("synthetic"));
    let mut range = exact.clone();
    range.ranges.push(Range {
        field: "day".into(),
        min: 2,
        max: 5,
    });
    for (name, query) in [("exact", exact), ("range", range)] {
        let expected = serde_json::to_value(db.search(&query)?)?;
        for mode in [0, 1, 2] {
            let before = allocation::reset();
            let page = match mode {
                0 => db.search(&query)?,
                1 => db.search_direct_slots(&query)?,
                _ => db.search_grouped_slots(&query)?,
            };
            let (live, peak, calls) = allocation::snapshot();
            println!(
                "{}",
                json!({"query":name,"mode":match mode {0=>"key",1=>"slot",_=>"grouped"},
                "additional_requested_peak_bytes":peak.saturating_sub(before),
                "requested_live_bytes_with_page":live,"requested_allocation_calls":calls,
                "includes_rss":false,"timing_is_instrumented":true})
            );
            assert_eq!(serde_json::to_value(page)?, expected);
        }
    }
    Ok(())
}
