//! Reproducible synthetic million-record benchmark. No application integration.
#[path = "../tools/bench_support/corpus.rs"]
mod corpus;
#[path = "../tools/bench_support/verify.rs"]
mod verify;
use corpus::{key, samples};
use serde_json::{Value, json};
use siftwake::{Database, Guard, Mutation, Options, Query, Range};
use std::time::Instant;

fn diagnostic(phase: &str) {
    #[cfg(feature = "diagnostic-timing")]
    println!(
        "{}",
        json!({"phase":"diagnostic_timing","window":phase,"counters":siftwake::timing::take(),"wall_spans_overlap":true,"not_cpu_time":true,"instrumented":true})
    );
    #[cfg(not(feature = "diagnostic-timing"))]
    let _ = phase;
}
fn report(value: Value) {
    println!("{value}");
}
fn query(
    db: &Database,
    q: &Query,
    expected: &(usize, Vec<siftwake::StoredDocument>),
    repeats: usize,
) -> siftwake::Result<Value> {
    let mut times = vec![];
    for repeat in 0..=repeats {
        let start = Instant::now();
        let page = db.search(q)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if repeat != 0 {
            times.push(elapsed);
        }
        assert_eq!(page.total, expected.0);
        assert_eq!(page.hits, expected.1);
    }
    diagnostic(&format!(
        "query_text_{}_offset_{}_ranges_{}",
        q.text,
        q.offset,
        q.ranges.len()
    ));
    Ok(samples(times))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .ok_or("measure_scale <new-dir> [count] [text-bytes] [delta-keys]")?;
    let count: usize = args
        .next()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(1_000_000);
    let text_bytes: usize = args.next().map(|v| v.parse()).transpose()?.unwrap_or(256);
    let delta_keys: usize = args
        .next()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(Options::default().max_delta_keys);
    let options = Options {
        max_delta_keys: delta_keys,
        ..Options::default()
    };
    if delta_keys == 0 {
        return Err("delta keys must be positive".into());
    }
    if count < 1000 || text_bytes > 1024 * 1024 {
        return Err("count >= 1000, text <= 1 MiB".into());
    }
    if std::path::Path::new(&dir).exists() {
        return Err("new directory required".into());
    }
    let mut db = Database::open_with_options(&dir, options.clone())?;
    let wal_sync = args.next().unwrap_or_else(|| "on".into());
    if !["on", "off"].contains(&wal_sync.as_str()) || args.next().is_some() {
        return Err("wal-sync must be on|off; no extra arguments".into());
    }
    #[cfg(feature = "experimental-wal-sync-control")]
    db.set_wal_sync(wal_sync == "on")?;
    #[cfg(not(feature = "experimental-wal-sync-control"))]
    if wal_sync == "off" {
        return Err("build with experimental-wal-sync-control".into());
    }
    if db.stats()?.documents != 0 {
        return Err("empty directory required".into());
    }
    let body = corpus::body(text_bytes);
    report(
        json!({"phase":"ingest","count":count,"text_bytes":body.len(),"wal_sync":wal_sync,"wal_enabled":true}),
    );
    diagnostic("initial_open");
    let start = Instant::now();
    let mut batches = vec![];
    for first in (0..count).step_by(500) {
        let changes = (first..(first + 500).min(count))
            .map(|id| Mutation::Put {
                document: corpus::document(id, &body),
                guard: Guard::Missing,
            })
            .collect();
        let batch = Instant::now();
        db.commit(changes)?;
        batches.push(batch.elapsed().as_secs_f64() * 1000.0);
        if (first + 500).is_multiple_of(50_000) {
            report(
                json!({"phase":"ingest_progress","records":first+500,"elapsed_s":start.elapsed().as_secs_f64(),"stats":db.stats()?}),
            );
        }
    }
    let ingest_ms = start.elapsed().as_secs_f64() * 1000.0;
    diagnostic("ingest_workers_may_overlap");
    report(json!({"phase":"maintenance","ingest_ms":ingest_ms,"batches":samples(batches)}));
    let start = Instant::now();
    db.flush()?;
    db.wait_for_compaction()?;
    while db.start_compaction()? {
        db.wait_for_compaction()?;
    }
    let maintenance_ms = start.elapsed().as_secs_f64() * 1000.0;
    diagnostic("final_maintenance_workers_finished");
    let stable = db.stats()?;
    assert_eq!(stable.resident_documents, 0);
    drop(db);
    report(json!({"phase":"reopen","maintenance_ms":maintenance_ms,"stable":stable}));
    let start = Instant::now();
    let mut db = Database::open_with_options(&dir, options.clone())?;
    let reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
    for id in [0, count / 2, count - 1] {
        assert_eq!(db.get(&key(id))?.unwrap(), verify::get(id, &body));
    }
    diagnostic("validated_reopen");
    report(json!({"phase":"queries","reopen_ms":reopen_ms}));
    let mut narrow = Query::new("shared");
    narrow.collection = Some("contents".into());
    narrow.text = format!("rapor-{:010}", count / 2);
    let narrow_ms = query(
        &db,
        &narrow,
        &verify::expected("narrow", count, &body, false, 0),
        30,
    )?;
    let mut broad = Query::new("shared");
    broad.collection = Some("contents".into());
    broad.exact.insert("source".into(), json!("synthetic"));
    let broad_ms = query(
        &db,
        &broad,
        &verify::expected("exact", count, &body, false, 0),
        30,
    )?;
    broad.offset = 50;
    let second_page = query(
        &db,
        &broad,
        &verify::expected("exact", count, &body, false, 50),
        30,
    )?;
    broad.offset = count - 50;
    let last_page = query(
        &db,
        &broad,
        &verify::expected("exact", count, &body, false, count - 50),
        3,
    )?;
    broad.offset = 0;
    broad.ranges = vec![Range {
        field: "day".into(),
        min: 2,
        max: 5,
    }];
    let range_ms = query(
        &db,
        &broad,
        &verify::expected("range", count, &body, false, 0),
        30,
    )?;
    let mut get_times = Vec::new();
    for repeat in 0..=3 {
        let start = Instant::now();
        let found = (0..100)
            .map(|n| db.get(&key(n * (count / 100))))
            .collect::<siftwake::Result<Vec<_>>>()?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if repeat != 0 {
            get_times.push(elapsed);
        }
        for (n, actual) in found.into_iter().enumerate() {
            assert_eq!(actual.unwrap(), verify::get(n * (count / 100), &body));
        }
    }
    let get_100 = samples(get_times);
    diagnostic("ordinary_queries");
    // Change records spread across every part of the corpus, including tombstones.
    let changes = (0..100)
        .map(|n| {
            let id = n * (count / 100);
            if n % 2 == 0 {
                Mutation::Delete {
                    key: key(id),
                    guard: Guard::Any,
                }
            } else {
                let mut d = db.get(&key(id)).unwrap().unwrap().document;
                d.text = "changed sentinel".into();
                Mutation::Put {
                    document: d,
                    guard: Guard::Any,
                }
            }
        })
        .collect();
    db.commit(changes)?;
    db.flush()?;
    db.wait_for_compaction()?;
    let sequence = db.stats()?.sequence;
    drop(db);
    let start = Instant::now();
    let db = Database::open_with_options(&dir, options.clone())?;
    let changed_reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
    for n in 0..100 {
        let stored = db.get(&key(n * (count / 100)))?;
        if n % 2 == 0 {
            assert!(stored.is_none());
        } else {
            assert_eq!(stored.unwrap().document.text, "changed sentinel");
        }
    }
    assert_eq!(db.stats()?.sequence, sequence);
    assert_eq!(db.stats()?.documents, count - 50);
    let mut text = Query::new("shared");
    text.collection = Some("contents".into());
    text.text = "teknoloji".into();
    let broad_text = query(
        &db,
        &text,
        &verify::expected("text", count, &body, true, 0),
        3,
    )?;
    diagnostic("updates_reopen_broad_text");
    report(
        json!({"phase":"result","delta_keys_watermark":delta_keys,"delta_bytes_watermark":options.max_delta_bytes,"count":count,"text_bytes":body.len(),"logical_text_bytes":count*body.len(),"ingest_ms":ingest_ms,"maintenance_ms":maintenance_ms,"reopen_ms":reopen_ms,"changed_reopen_ms":changed_reopen_ms,"narrow":narrow_ms,"broad":broad_ms,"range":range_ms,"broad_text":broad_text,"stats":db.stats()?,"verified_updates":50,"verified_deletes":50,"last_page":last_page,"second_page":second_page,"get_100":get_100,"query_verification":"full StoredDocument oracle; untimed warmup; checks outside timing"}),
    );
    Ok(())
}
