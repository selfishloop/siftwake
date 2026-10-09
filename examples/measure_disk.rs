//! Synthetic content benchmark; does not read either application project.
use fdb::{Database, Document, Guard, Key, Mutation, Query, Range};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Instant;

fn timing(
    db: &Database,
    query: &Query,
    expected: usize,
    repeats: usize,
) -> fdb::Result<serde_json::Value> {
    let mut samples = vec![];
    for _ in 0..repeats {
        let start = Instant::now();
        let page = db.search(query)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(page.total, expected);
    }
    samples.sort_by(f64::total_cmp);
    Ok(
        json!({"p50_ms":samples[samples.len()/2], "p95_ms":samples[(samples.len()*95/100).min(samples.len()-1)]}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .ok_or("usage: measure_disk <new-db-dir> [records] [text-bytes]")?;
    let count = args
        .next()
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(50_000);
    let text_size = args
        .next()
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(1024);
    if count < 100 || text_size > 1024 * 1024 {
        return Err("records >= 100 and text-bytes <= 1 MiB required".into());
    }
    let mut db = Database::open(&dir)?;
    if db.stats()?.documents != 0 {
        return Err("benchmark requires an empty database".into());
    }
    let mut body = "Şirketin teknoloji yatırımları. ".repeat(text_size.div_ceil(36) + 1);
    while body.len() > text_size {
        body.pop();
    }
    let started = Instant::now();
    for batch_start in (0..count).step_by(500) {
        let mutations = (batch_start..(batch_start + 500).min(count))
            .map(|id| {
                let document = Document {
                    key: Key::new("shared", "contents", &format!("{id:08}")),
                    title: format!("rapor-{id:08}"),
                    text: body.clone(),
                    fields: BTreeMap::from([
                        ("day".into(), json!(id % 30)),
                        ("source".into(), json!("synthetic")),
                    ]),
                    links: vec![],
                };
                Mutation::Put {
                    document,
                    guard: Guard::Any,
                }
            })
            .collect();
        db.commit(mutations)?;
    }
    let ingest_ms = started.elapsed().as_secs_f64() * 1000.0;
    let before = db.stats()?;
    let start = Instant::now();
    db.compact()?;
    db.wait_for_compaction()?;
    let after = db.stats()?;
    let compact_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(after.resident_documents, 0);
    assert_eq!(after.resident_text_bytes, 0);
    drop(db);
    let start = Instant::now();
    let db = Database::open(&dir)?;
    let reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(db.stats()?.resident_documents, 0);
    let mut narrow = Query::new("shared");
    narrow.collection = Some("contents".into());
    narrow.text = format!("rapor-{:08}", count / 2);
    let narrow_times = timing(&db, &narrow, 1, 30)?;
    let mut broad = Query::new("shared");
    broad.collection = Some("contents".into());
    broad.exact.insert("source".into(), json!("synthetic"));
    let broad_times = timing(&db, &broad, count, 5)?;
    let mut filtered = broad.clone();
    filtered.ranges = vec![Range {
        field: "day".into(),
        min: 2,
        max: 5,
    }];
    let expected = (0..count).filter(|n| (2..=5).contains(&(n % 30))).count();
    let filtered_times = timing(&db, &filtered, expected, 5)?;
    println!(
        "{}",
        json!({"records":count, "text_bytes_per_record":body.len(), "logical_text_bytes":body.len()*count,"ingest_ms":ingest_ms,"compact_ms":compact_ms,"reopen_ms":reopen_ms,"narrow_disk_query":narrow_times,"broad_disk_query":broad_times,"filtered_disk_query":filtered_times,"before_compact":before,"after_compact":after})
    );
    Ok(())
}
