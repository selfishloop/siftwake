//! Actual Database::search on the long-passage corpus; no payload-only shortcut.
use serde::Deserialize;
use serde_json::json;
use siftwake::{Database, Document, Key, Query, StoredDocument};
use std::{collections::BTreeMap, fs, path::Path, time::Instant};
#[derive(Deserialize)]
struct Source {
    book_id: u32,
    title: String,
}
#[derive(Deserialize)]
struct Row {
    id: u32,
    book: usize,
    start: usize,
    len: usize,
}
#[derive(Deserialize)]
struct Corpus {
    sources: Vec<Source>,
    rows: Vec<Row>,
}
#[derive(Deserialize)]
struct Request {
    name: String,
    text: String,
    ids: Vec<usize>,
}
fn document(corpus: &Corpus, books: &[Vec<u8>], id: usize) -> StoredDocument {
    let row = &corpus.rows[id];
    assert_eq!(row.id as usize, id);
    let source = &corpus.sources[row.book];
    StoredDocument {
        version: id as u64 / 500 + 1,
        document: Document {
            key: Key::new("shared", "texts", &format!("{id:010}")),
            title: format!("Passage {id:010}"),
            text: String::from_utf8(books[row.book][row.start..row.start + row.len].to_vec())
                .unwrap(),
            fields: BTreeMap::from([
                ("source".into(), json!(source.title)),
                ("book_id".into(), json!(source.book_id)),
                ("offset".into(), json!(row.start)),
                ("bytes".into(), json!(row.len)),
                ("tags".into(), json!(["literature", "en"])),
                (
                    "attributes".into(),
                    json!({"verified":false,"score":0.125,"optional":null}),
                ),
            ]),
            links: if id.is_multiple_of(3) {
                vec![Key::new(
                    "shared",
                    "texts",
                    &format!("{:010}", id.saturating_sub(1)),
                )]
            } else {
                vec![]
            },
        },
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("existing benchmark DB required")?;
    let only = args.next();
    if args.next().is_some() {
        return Err("extra arguments".into());
    }
    let start = Instant::now();
    #[allow(unused_mut)]
    let mut db = Database::open(&path)?;
    let open_ms = start.elapsed().as_secs_f64() * 1000.;
    #[allow(unused_mut)]
    let mut overlay_bytes = 0u64;
    #[allow(unused_mut)]
    let mut attach_ms = 0.;
    let overlay = std::env::var("FDB_BENCH_PAYLOAD").unwrap_or_else(|_| "native".into());
    #[cfg(feature = "experimental-rkyv-payload")]
    if overlay != "native" {
        let codec = match overlay.as_str() {
            "json" => siftwake::rkyv_payload::bench::Codec::Json,
            "binary" => siftwake::rkyv_payload::bench::Codec::Binary,
            _ => return Err("invalid benchmark payload".into()),
        };
        let attach = Instant::now();
        overlay_bytes = db.attach_payload_benchmark(
            format!("data/rkyv-corpus-20261009/{}.payload-bench", codec.name()),
            codec,
        )?;
        attach_ms = attach.elapsed().as_secs_f64() * 1000.;
    }
    #[cfg(not(feature = "experimental-rkyv-payload"))]
    if overlay != "native" {
        return Err("overlay requires experimental-rkyv-payload".into());
    }
    let stats = db.stats()?;
    assert_eq!(stats.documents, 100000);
    assert_eq!(stats.resident_documents, 0);
    println!(
        "{}",
        json!({"phase":"open","open_ms":open_ms,"stats":stats,"api":"Database::search","overlay":overlay,"overlay_bytes":overlay_bytes,"attach_ms":attach_ms,"total_ready_ms":open_ms+attach_ms})
    );
    if only.as_deref() == Some("prepare") {
        return Ok(());
    }
    let root = Path::new("data/rkyv-corpus-20261009");
    let corpus: Corpus = serde_json::from_slice(&fs::read(root.join("corpus.json"))?)?;
    let books = corpus
        .sources
        .iter()
        .map(|s| fs::read(root.join(format!("body{}.txt", s.book_id))))
        .collect::<Result<Vec<_>, _>>()?;
    let requests: Vec<Request> =
        serde_json::from_slice(&fs::read(Path::new(&path).join("oracle.json"))?)?;
    if only.is_none() {
        let ids: Vec<_> = (0..100).map(|n| (n * 7919 + 137) % 100000).collect();
        let expected: Vec<_> = ids
            .iter()
            .map(|&id| document(&corpus, &books, id))
            .collect();
        let mut times = Vec::new();
        for repeat in 0..4 {
            let start = Instant::now();
            let found = expected
                .iter()
                .map(|d| db.get(&d.document.key))
                .collect::<siftwake::Result<Vec<_>>>()?;
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            assert_eq!(
                found,
                expected.iter().cloned().map(Some).collect::<Vec<_>>()
            );
            if repeat != 0 {
                times.push(elapsed);
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{}",
            json!({"phase":"get","requests":100,"samples_ms":times,"p50_ms":times[times.len()/2],"full_document_oracle_verified":true,"cache":"one untimed 100-get warmup; no forced OS eviction"})
        );
    }
    for request in requests {
        if only.as_deref().is_some_and(|name| name != request.name) {
            continue;
        }
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.text = request.text.clone();
        let mut offsets = vec![0, 50];
        if request.name == "common" {
            offsets.push(request.ids.len() - 50);
        }
        for (page_index, offset) in offsets.into_iter().enumerate() {
            q.offset = offset;
            let expected: Vec<_> = request
                .ids
                .iter()
                .skip(offset)
                .take(50)
                .map(|&id| document(&corpus, &books, id))
                .collect();
            let repeats = if request.name == "rare" { 10 } else { 1 };
            let mut times = Vec::new();
            for repeat in 0..repeats + usize::from(page_index == 0) {
                #[cfg(feature = "diagnostic-timing")]
                let _ = siftwake::timing::take();
                let start = Instant::now();
                let page = db.search(&q)?;
                let elapsed = start.elapsed().as_secs_f64() * 1000.;
                assert_eq!(page.total, request.ids.len(), "{}", request.name);
                assert_eq!(page.hits, expected, "{} offset={offset}", request.name);
                assert_eq!(page.sequence, 200);
                if page_index != 0 || repeat != 0 {
                    times.push(elapsed);
                }
                #[cfg(feature = "diagnostic-timing")]
                println!(
                    "{}",
                    json!({"phase":"profile","query":request.name,"offset":offset,"repeat":repeat,"elapsed_ms":elapsed,"counters":siftwake::timing::take(),"instrumented":true})
                );
            }
            times.sort_by(f64::total_cmp);
            println!(
                "{}",
                json!({"phase":"query","query":request.name,"text":request.text,"offset":offset,"total":request.ids.len(),"samples_ms":times,"p50_ms":times[times.len()/2],"full_page_oracle_verified":true,"api":"Database::search","cache":"warm OS cache; first page has one untimed query; later pages follow; no forced eviction","profiled":cfg!(feature="diagnostic-timing")})
            );
        }
    }
    Ok(())
}
