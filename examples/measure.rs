use fdb::{Database, Guard, Mutation, Query, adapters};
use serde_json::json;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .ok_or("supply a new benchmark directory")?;
    let mut db = Database::open(&dir)?;
    if db.stats()?.documents != 0 {
        return Err("benchmark requires an empty database".into());
    }
    let started = Instant::now();
    for batch in 0..20 {
        let mut mutations = vec![];
        for n in batch * 500..(batch + 1) * 500 {
            let name = format!("teknoloji-raporu-{n:06}.txt");
            let doc = adapters::file_entry(
                "shared",
                "sample-volume",
                n + 10,
                5,
                &name,
                &format!("D:/Raporlar/{name}"),
                0,
            )?;
            mutations.push(Mutation::Put {
                document: doc,
                guard: Guard::Any,
            });
        }
        db.commit(mutations)?;
    }
    let ingest_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut query = Query::new("shared");
    query.collection = Some("files".into());
    query.text = "raporu-007777".into();
    let mut timings = vec![];
    for _ in 0..100 {
        let start = Instant::now();
        let result = db.search(&query)?;
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(result.total, 1);
    }
    timings.sort_by(f64::total_cmp);
    let start = Instant::now();
    db.compact()?;
    let compact_ms = start.elapsed().as_secs_f64() * 1000.0;
    let stats = db.stats()?;
    drop(db);
    let start = Instant::now();
    let db = Database::open(dir)?;
    let reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(db.stats()?.documents, 10_000);
    println!(
        "{}",
        json!({"synthetic_records":10000,"ingest_ms":ingest_ms,"query_p50_ms":timings[50],"query_p95_ms":timings[95],"compact_ms":compact_ms,"reopen_ms":reopen_ms,"active_log_bytes":stats.log_bytes})
    );
    Ok(())
}
