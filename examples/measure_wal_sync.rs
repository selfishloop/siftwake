//! Isolate commit sync without flush/merge; run on/off in alternating processes.
#[cfg(feature = "experimental-wal-sync-control")]
#[path = "../tools/bench_support/corpus.rs"]
mod corpus;

#[cfg(not(feature = "experimental-wal-sync-control"))]
fn main() {
    eprintln!("build with experimental-wal-sync-control");
    std::process::exit(1);
}

#[cfg(feature = "experimental-wal-sync-control")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::json;
    use siftwake::{Database, Guard, Mutation, Options, StoredDocument};
    use std::time::Instant;
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("measure_wal_sync <new-dir> <on|off> <batch-size>")?;
    let mode = args.next().ok_or("on|off required")?;
    let batch_size: usize = args.next().ok_or("batch-size required")?.parse()?;
    if !["on", "off"].contains(&mode.as_str())
        || !(1..=5000).contains(&batch_size)
        || args.next().is_some()
    {
        return Err("invalid mode/batch arguments".into());
    }
    if std::path::Path::new(&path).exists() {
        return Err("new directory required".into());
    }
    let count: usize = 5000;
    let options = Options {
        max_delta_keys: count + 1,
        max_delta_bytes: 64 * 1024 * 1024,
    };
    let mut db = Database::open_with_options(&path, options)?;
    db.set_wal_sync(mode == "on")?;
    let body = corpus::body(256);
    let mut batches = Vec::new();
    let start = Instant::now();
    for first in (0..count).step_by(batch_size) {
        let mutations = (first..(first + batch_size).min(count))
            .map(|id| Mutation::Put {
                document: corpus::document(id, &body),
                guard: Guard::Missing,
            })
            .collect();
        let batch = Instant::now();
        db.commit(mutations)?;
        batches.push(batch.elapsed().as_secs_f64() * 1000.);
    }
    let ingest_ms = start.elapsed().as_secs_f64() * 1000.;
    let stats = db.stats()?;
    assert_eq!(stats.documents, count);
    assert_eq!(stats.segments, 0);
    assert_eq!(stats.delta_keys, count);
    drop(db);
    let db = Database::open(&path)?;
    // Every recovered byte/payload/version is verified outside measured ingest.
    for id in 0..count {
        assert_eq!(
            db.get(&corpus::key(id))?,
            Some(StoredDocument {
                document: corpus::document(id, &body),
                version: (id / batch_size + 1) as u64
            })
        );
    }
    assert_eq!(db.stats()?.documents, count);
    println!(
        "{}",
        json!({"count":count,"text_bytes":body.len(),"batch_size":batch_size,"wal_enabled":true,"wal_sync":mode,"ingest_ms":ingest_ms,"batches":corpus::samples(batches),"no_flush_or_merge_in_timer":true,"full_reopen_oracle_verified":true,"power_failure_tested":false})
    );
    Ok(())
}
