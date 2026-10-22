//! Native API/JSON attribution on the existing corpus's first 1k; no generator.
#[path = "../tools/bench_support/memory_corpus.rs"]
#[allow(dead_code)]
mod input;
use fdb::{Database, Guard, Mutation, Query};
use serde_json::json;
use std::{hint::black_box, path::Path, time::Instant};

fn main() -> input::Result<()> {
    let path = Path::new("data/benchmark-work/fdb");
    if path.exists() {
        return Err("Reset only the owned fixed FDB slot before this write probe".into());
    }
    let seed: usize = std::env::args().nth(1).ok_or("seed required")?.parse()?;
    let mut db = Database::open(path)?;
    let mut source = input::Input::open()?;
    for _ in 0..2 {
        let mutations = (0..500)
            .map(|_| {
                Ok(Mutation::Put {
                    document: source.read()?.document,
                    guard: Guard::Missing,
                })
            })
            .collect::<input::Result<Vec<_>>>()?;
        db.commit(mutations)?;
    }
    let stats = db.stats()?;
    assert_eq!(
        (stats.sequence, stats.documents, stats.delta_keys),
        (2, 1000, 1000)
    );
    let texts = [
        "the",
        "quixote",
        "he",
        "the man",
        "zx",
        "transubstantiation",
    ];
    let queries = texts.map(|text| {
        let mut query = Query::new("shared");
        query.collection = Some("texts".into());
        query.text = text.into();
        query.limit = 50;
        query
    });
    let mut checks = Vec::new();
    for query in &queries {
        let page = db.search(query)?;
        let encoded = serde_json::to_vec(&page)?;
        checks.push((page.total, encoded.len(), crc32fast::hash(&encoded)));
    }
    let mut search = vec![Vec::new(); queries.len()];
    let mut json = vec![Vec::new(); queries.len()];
    for round in 0..100 {
        let mut order: Vec<_> = (0..queries.len()).collect();
        if !(round + seed).is_multiple_of(2) {
            order.reverse();
        }
        for index in order {
            let start = Instant::now();
            let page = black_box(db.search(black_box(&queries[index]))?);
            search[index].push(start.elapsed().as_secs_f64() * 1000.);
            let start = Instant::now();
            let encoded = black_box(serde_json::to_vec(black_box(&page))?);
            json[index].push(start.elapsed().as_secs_f64() * 1000.);
            assert_eq!(page.sequence, 2);
            assert_eq!(
                (page.total, encoded.len(), crc32fast::hash(&encoded)),
                checks[index]
            );
        }
    }
    println!(
        "{}",
        json!({"phase":"native_boundary", "stats":stats,
        "seed":seed,"search_samples_ms":search,"json_samples_ms":json,
        "queries":texts,"checks":checks,"repeats":100,
        "limits":"Native search and serde_json::to_vec timed separately;not exact CLI serialization/IPC path;CRC checks and drops outside timing;no RAM claim"})
    );
    Ok(())
}
