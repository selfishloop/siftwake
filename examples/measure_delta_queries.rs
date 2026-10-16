//! Bounded long-text query comparison; reset only the owned fixed FDB workspace.
#[path = "../tools/bench_support/memory_corpus.rs"]
mod input;
use fdb::{Database, Guard, Mutation, Options, Query, StoredDocument};
use serde_json::json;
use std::time::Instant;

fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            match c {
                'I' | 'İ' | 'ı' => 'i',
                'Ç' | 'ç' => 'c',
                'Ğ' | 'ğ' => 'g',
                'Ö' | 'ö' => 'o',
                'Ş' | 'ş' => 's',
                'Ü' | 'ü' => 'u',
                _ => c,
            }
            .to_lowercase()
        })
        .collect()
}
fn commit(db: &mut Database, changes: Vec<Mutation>, streamed: bool) -> fdb::Result<u64> {
    if streamed {
        #[cfg(feature = "bounded-engine")]
        {
            return db.commit_streamed(changes.into_iter().map(Ok));
        }
        #[cfg(not(feature = "bounded-engine"))]
        {
            return Err(fdb::Error::Invalid(
                "streamed input requires experimental-builder-budget".into(),
            ));
        }
    }
    db.commit(changes)
}

fn queries(db: &Database, docs: &[StoredDocument], state: &str) -> input::Result<()> {
    let folded: Vec<_> = docs
        .iter()
        .map(|d| fold(&format!("{}\n{}", d.document.title, d.document.text)))
        .collect();
    for text in [
        "the",
        "quixote",
        "he",
        "the man",
        "istanbul",
        "updatedunique",
    ] {
        let terms: Vec<_> = text.split_whitespace().map(fold).collect();
        let mut expected: Vec<_> = docs
            .iter()
            .zip(&folded)
            .filter(|(_, body)| terms.iter().all(|t| body.contains(t)))
            .map(|(d, _)| d.clone())
            .collect();
        expected.sort_by(|a, b| a.document.key.cmp(&b.document.key));
        let mut q = Query::new(&docs[0].document.key.scope);
        q.collection = Some(docs[0].document.key.collection.clone());
        q.text = text.into();
        let mut samples = Vec::new();
        for turn in 0..11 {
            #[cfg(feature = "diagnostic-timing")]
            let _ = fdb::timing::take();
            let start = Instant::now();
            let page = db.search(&q)?;
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            #[cfg(feature = "diagnostic-timing")]
            if turn == 1 {
                println!(
                    "{}",
                    json!({"phase":"query_profile","state":state,"text":text,"elapsed_ms":elapsed,"counters":fdb::timing::take(),"instrumented":true,"nested_spans_do_not_sum":true})
                );
            }
            assert_eq!(page.total, expected.len(), "{state} {text}");
            assert_eq!(
                page.hits,
                expected.iter().take(50).cloned().collect::<Vec<_>>(),
                "{state} {text}"
            );
            if turn != 0 {
                samples.push(elapsed);
            }
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{}",
            json!({"phase":"query","state":state,"text":text,"total":expected.len(),"p50_ms":samples[5],"p95_ms":samples[9],"samples_ms":samples,"full_page_oracle":true,"cache":"one untimed query per text; no OS eviction","delta_bitmaps":cfg!(feature="experimental-delta-bitmaps")})
        );
    }
    Ok(())
}

fn main() -> input::Result<()> {
    let streamed = std::env::args().any(|arg| arg == "--streamed");
    let path = std::env::args()
        .nth(1)
        .ok_or("fresh database path required")?;
    if std::path::Path::new(&path).exists() {
        return Err("fresh database required".into());
    }
    input::phase("input");
    let mut source = input::Input::open()?;
    source.skip(0)?;
    let mut docs = (0..1000)
        .map(|_| source.read())
        .collect::<input::Result<Vec<_>>>()?;
    let options = Options {
        max_delta_keys: 2000,
        max_delta_bytes: 128 * 1024 * 1024,
    };
    let mut db = Database::open_with_options(&path, options.clone())?;
    commit(
        &mut db,
        docs.iter()
            .rev()
            .map(|s| Mutation::Put {
                document: s.document.clone(),
                guard: Guard::Missing,
            })
            .collect(),
        streamed,
    )?;
    for s in &mut docs {
        s.version = 1;
    }
    let stats = db.stats()?;
    assert_eq!(stats.documents, 1000);
    assert_eq!(stats.delta_keys, if streamed { 0 } else { 1000 });
    assert_eq!(
        stats.resident_documents,
        if streamed || cfg!(feature = "experimental-wal-delta") {
            0
        } else {
            1000
        }
    );
    queries(&db, &docs, "delta")?;
    let mut changes = Vec::new();
    for s in docs.iter_mut().take(40) {
        changes.push(Mutation::Delete {
            key: s.document.key.clone(),
            guard: Guard::Version { version: 1 },
        });
    }
    commit(&mut db, changes, streamed)?;
    let mut changes = Vec::new();
    for s in docs.iter_mut().take(40) {
        s.document.text = "İSTANBUL updatedunique".into();
        s.version = 3;
        changes.push(Mutation::Put {
            document: s.document.clone(),
            guard: Guard::Missing,
        });
    }
    commit(&mut db, changes, streamed)?;
    queries(&db, &docs, "reused_slots")?;
    drop(db);
    let db = Database::open_with_options(&path, options)?;
    queries(&db, &docs, "reopened_delta")?;
    println!(
        "{}",
        json!({"phase":"result","documents":1000,"text_bytes_initial":source.text_bytes,"crc_checked_input":true,"reverse_ingest_and_delete_reinsert_verified":true,"sequence":db.stats()?.sequence,"streamed_input":streamed,"physical_delta_keys":db.stats()?.delta_keys,"scope":"first 1000 long corpus records; normal input uses unflushed delta, streamed input uses published runs; see flags for both variants; no SQLite comparison or process RAM measurement"})
    );
    Ok(())
}
