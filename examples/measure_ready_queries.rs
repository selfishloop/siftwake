//! Read the same healthy benchmark database; exclude open and writes from timing.
#[path = "../tools/bench_support/corpus.rs"]
mod corpus;
#[path = "../tools/bench_support/verify.rs"]
mod verify;
use serde_json::{Value, json};
use siftwake::{Database, Query, Range, StoredDocument};
use std::time::Instant;
fn measured(
    db: &Database,
    q: &Query,
    expected: (usize, Vec<StoredDocument>),
    repeats: usize,
) -> siftwake::Result<Value> {
    let mut times = Vec::new();
    for repeat in 0..repeats + 10 {
        let start = Instant::now();
        let page = db.search(q)?;
        let ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(page.total, expected.0);
        assert_eq!(page.hits, expected.1);
        if repeat >= 10 {
            times.push(ms);
        }
    }
    Ok(corpus::samples(times))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("existing 100k benchmark database required")?;
    let db = Database::open(&path)?;
    assert_eq!(db.stats()?.documents, 99_950);
    let body = corpus::body(256);
    let count = 100_000;
    let mut exact = Query::new("shared");
    exact.collection = Some("contents".into());
    exact.exact.insert("source".into(), json!("synthetic"));
    let broad = measured(
        &db,
        &exact,
        verify::expected("exact", count, &body, true, 0),
        200,
    )?;
    exact.offset = 50;
    let second = measured(
        &db,
        &exact,
        verify::expected("exact", count, &body, true, 50),
        200,
    )?;
    exact.offset = count - 100;
    let last = measured(
        &db,
        &exact,
        verify::expected("exact", count, &body, true, count - 100),
        100,
    )?;
    exact.offset = 0;
    exact.ranges = vec![Range {
        field: "day".into(),
        min: 2,
        max: 5,
    }];
    let range = measured(
        &db,
        &exact,
        verify::expected("range", count, &body, true, 0),
        200,
    )?;
    let mut text = Query::new("shared");
    text.collection = Some("contents".into());
    text.text = "rapor-0000050001".into();
    let narrow = measured(&db, &text, (1, vec![verify::get(50_001, &body)]), 200)?;
    text.text = "teknoloji".into();
    let broad_text = measured(
        &db,
        &text,
        verify::expected("text", count, &body, true, 0),
        20,
    )?;
    let expected: Vec<_> = (0..100).map(|n| verify::get(n * 997 + 1, &body)).collect();
    let mut times = Vec::new();
    for repeat in 0..110 {
        let start = Instant::now();
        let actual = (0..100)
            .map(|n| db.get(&corpus::key(n * 997 + 1)))
            .collect::<siftwake::Result<Vec<_>>>()?;
        let ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(
            actual,
            expected.iter().cloned().map(Some).collect::<Vec<_>>()
        );
        if repeat >= 10 {
            times.push(ms);
        }
    }
    println!(
        "{}",
        json!({"database":path,"counts_verified":true,"open_excluded":true,"writes":false,"small_repeats":200,"last_repeats":100,"get_repeats":100,"text_repeats":20,"warmup":10,"narrow":narrow,"broad":broad,"second_page":second,"last_page":last,"range":range,"broad_text":broad_text,"get_100":corpus::samples(times)})
    );
    Ok(())
}
