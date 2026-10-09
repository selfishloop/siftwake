//! Fresh 10k unflushed delta: expose WAL hydration cost hidden by ready-run queries.
#[path = "../tools/bench_support/corpus.rs"]
mod corpus;
#[path = "../tools/bench_support/verify.rs"]
mod verify;
use fdb::{Database, Guard, Mutation, Options, Query, Range, StoredDocument};
use serde_json::{Value, json};
use std::time::Instant;
fn measured(
    db: &Database,
    q: &Query,
    expected: (usize, Vec<StoredDocument>),
    repeats: usize,
) -> fdb::Result<Value> {
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
// Independent corpus transformation; search totals/page predicates remain in verify.
fn decorate(mut stored: StoredDocument, mode: &str) -> StoredDocument {
    let id: usize = stored.document.key.id.parse().expect("corpus integer ID");
    if mode == "unique" || (mode == "mixed" && id.is_multiple_of(2)) {
        stored.document.text.push_str(&format!(" unique-{id:010}"));
        stored.document.fields.insert(
            "metadata".into(),
            json!({"file_id":id,"labels":["ş","archive"]}),
        );
    }
    stored
}
fn expected(
    kind: &str,
    count: usize,
    body: &str,
    offset: usize,
    mode: &str,
) -> (usize, Vec<StoredDocument>) {
    let (total, hits) = verify::expected(kind, count, body, false, offset);
    (
        total,
        hits.into_iter()
            .map(|stored| decorate(stored, mode))
            .collect(),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("new benchmark directory required")?;
    let mode = args.next().unwrap_or_else(|| "repeated".into());
    if !["repeated", "unique", "mixed"].contains(&mode.as_str()) {
        return Err("mode repeated|unique|mixed required".into());
    }
    if std::path::Path::new(&path).exists() {
        return Err("benchmark directory already exists".into());
    }
    let body = corpus::body(256);
    let count = 10_000;
    let mut db = Database::open_with_options(
        &path,
        Options {
            max_delta_keys: count + 1,
            max_delta_bytes: 64 * 1024 * 1024,
        },
    )?;
    for first in (0..count).step_by(500) {
        db.commit(
            (first..first + 500)
                .map(|id| Mutation::Put {
                    document: decorate(verify::get(id, &body), &mode).document,
                    guard: Guard::Any,
                })
                .collect(),
        )?;
    }
    assert_eq!(db.stats()?.documents, count);
    assert_eq!(db.stats()?.delta_keys, count);
    assert_eq!(db.stats()?.segments, 0);
    let mut exact = Query::new("shared");
    exact.collection = Some("contents".into());
    exact.exact.insert("source".into(), json!("synthetic"));
    let broad = measured(&db, &exact, expected("exact", count, &body, 0, &mode), 200)?;
    exact.offset = 50;
    let second = measured(&db, &exact, expected("exact", count, &body, 50, &mode), 200)?;
    exact.offset = count - 50;
    let last = measured(
        &db,
        &exact,
        expected("exact", count, &body, count - 50, &mode),
        100,
    )?;
    exact.offset = 0;
    exact.ranges = vec![Range {
        field: "day".into(),
        min: 2,
        max: 5,
    }];
    let range = measured(&db, &exact, expected("range", count, &body, 0, &mode), 200)?;
    let mut text = Query::new("shared");
    text.collection = Some("contents".into());
    text.text = "rapor-0000005000".into();
    let narrow = measured(
        &db,
        &text,
        (1, vec![decorate(verify::get(5_000, &body), &mode)]),
        200,
    )?;
    text.text = "teknoloji".into();
    let broad_text = measured(&db, &text, expected("text", count, &body, 0, &mode), 20)?;
    let expected: Vec<_> = (0..100)
        .map(|n| decorate(verify::get(n * 97 + 1, &body), &mode))
        .collect();
    let mut times = Vec::new();
    for repeat in 0..110 {
        let start = Instant::now();
        let actual = (0..100)
            .map(|n| db.get(&corpus::key(n * 97 + 1)))
            .collect::<fdb::Result<Vec<_>>>()?;
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
        json!({"database":path,"corpus_mode":mode,"counts_verified":true,"open_excluded":true,"writes_before_queries":true,"flushed":false,"count":count,"text_bytes":256,"stats":db.stats()?,"small_repeats":200,"last_repeats":100,"get_repeats":100,"text_repeats":20,"warmup":10,"narrow":narrow,"broad":broad,"second_page":second,"last_page":last,"range":range,"broad_text":broad_text,"get_100":corpus::samples(times)})
    );
    Ok(())
}
