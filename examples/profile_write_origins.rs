//! Separate origin allocator; requested bytes, not RSS; timings not comparable.
#[path = "../tools/bench_support/tagged_allocation.rs"]
mod allocation;
#[path = "../tools/bench_support/corpus.rs"]
#[allow(dead_code)]
mod corpus;
#[global_allocator]
static ALLOCATOR: allocation::Tracking = allocation::Tracking;
#[cfg(feature = "diagnostic-memory")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::json;
    use siftwake::{Database, Guard, Mutation};
    siftwake::memory::install(allocation::origin);
    let path = std::env::args().nth(1).ok_or("new directory required")?;
    if std::path::Path::new(&path).exists() {
        return Err("new directory required".into());
    }
    let mut db = Database::open(path)?;
    let body = corpus::body(256);
    for first in (0..100_000).step_by(500) {
        let changes = {
            let _origin = siftwake::memory::Scope::new(1);
            (first..first + 500)
                .map(|id| Mutation::Put {
                    document: corpus::document(id, &body),
                    guard: Guard::Missing,
                })
                .collect()
        };
        db.commit(changes)?;
        let (live, peak, total, total_peak) = allocation::snapshot();
        println!(
            "{}",
            json!({"records":first+500,"origin_names":siftwake::memory::NAMES,"requested_live":live,"requested_peak":peak,"total_live":total,"total_peak":total_peak,"not_rss":true,"not_normal_timing":true})
        );
    }
    db.flush()?;
    db.wait_for_compaction()?;
    while db.start_compaction()? {
        db.wait_for_compaction()?;
    }
    let (live, peak, total, total_peak) = allocation::snapshot();
    println!(
        "{}",
        json!({"phase":"finished","origin_names":siftwake::memory::NAMES,"requested_live":live,"requested_peak":peak,"total_live":total,"total_peak":total_peak,"not_rss":true,"not_normal_timing":true})
    );
    #[cfg(feature = "experimental-write-buffer-budget")]
    println!(
        "{}",
        json!({"phase":"write_buffer_budget","reservation":siftwake::write_budget::snapshot(),"not_total_heap_or_rss":true})
    );
    #[cfg(feature = "bounded-engine")]
    println!(
        "{}",
        json!({"phase":"posting_budget","reservation":siftwake::write_budget::posting_snapshot(),"not_total_heap_or_rss":true})
    );
    Ok(())
}
#[cfg(not(feature = "diagnostic-memory"))]
fn main() {
    let _ = allocation::origin(0);
    let _ = allocation::snapshot();
    eprintln!("requires diagnostic-memory");
}
