//! Opt-in allocator instrumentation of writes and maintenance. Atomic overhead
//! makes these timings unsuitable for the release performance comparison.
#[path = "../tools/bench_support/allocation.rs"]
mod allocation;
#[path = "../tools/bench_support/corpus.rs"]
#[allow(dead_code)] // This diagnostic uses the generator, not timing quantiles.
mod corpus;
use serde_json::json;
use siftwake::{Database, Guard, Mutation};

#[global_allocator]
static ALLOCATOR: allocation::Tracking = allocation::Tracking;

fn report(phase: &str, records: usize, db: &Database) -> siftwake::Result<()> {
    let (live, peak, calls) = allocation::snapshot();
    println!(
        "{}",
        json!({"phase":phase,"records":records,"rust_requested_live_bytes":live,
        "rust_requested_cumulative_peak_bytes":peak,"allocation_calls":calls,
        "includes_rss":false,"includes_concurrent_worker":true,"timing_is_instrumented":true,
        "stats":db.stats()?})
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("profile_writes <new-directory> [count]")?;
    let count: usize = args
        .next()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(100_000);
    if std::path::Path::new(&path).exists() || count < 1000 {
        return Err("new directory and count >= 1000 required".into());
    }
    allocation::reset();
    let mut db = Database::open(path)?;
    let body = corpus::body(256);
    report("opened", 0, &db)?;
    for first in (0..count).step_by(500) {
        let changes = (first..(first + 500).min(count))
            .map(|id| Mutation::Put {
                document: corpus::document(id, &body),
                guard: Guard::Missing,
            })
            .collect();
        db.commit(changes)?;
        report("committed", (first + 500).min(count), &db)?;
    }
    report("before_final_maintenance", count, &db)?;
    db.flush()?;
    db.wait_for_compaction()?;
    while db.start_compaction()? {
        db.wait_for_compaction()?;
    }
    report("after_final_maintenance", count, &db)?;
    println!(
        "{}",
        json!({"phase":"index_layout","segments":db.index_memory_layout()?,"not_total_heap_or_rss":true})
    );
    Ok(())
}
