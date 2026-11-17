//! Optional caller-result reuse against normal search, same existing corpus.
#[cfg(feature = "diagnostic-memory")]
#[path = "../tools/bench_support/allocation.rs"]
mod allocation;
#[cfg(feature = "diagnostic-memory")]
#[global_allocator]
static ALLOCATOR: allocation::Tracking = allocation::Tracking;
#[path = "../tools/bench_support/memory_corpus.rs"]
mod input;
#[path = "../tools/bench_support/read_matrix.rs"]
#[allow(dead_code)]
mod matrix;
#[path = "../tools/bench_support/content_reads.rs"]
#[allow(dead_code)]
mod reads;
use serde::Deserialize;
use serde_json::json;
use siftwake::{Database, Query, SearchPage};
use std::{
    io::{BufRead, Write},
    path::Path,
    time::Instant,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    id: u64,
    seed: u64,
    repeats: usize,
}
fn empty() -> SearchPage {
    SearchPage {
        total: 0,
        sequence: 0,
        hits: Vec::new(),
    }
}
fn run(db: &Database, query: &Query, page: &mut SearchPage, reuse: bool) -> input::Result<()> {
    #[cfg(feature = "bounded-engine")]
    if reuse {
        db.search_into(query, page)?;
        return Ok(());
    }
    assert!(!reuse, "reuse requires bounded-engine");
    // Both contracts consume the previous page inside the timed call.
    page.hits.clear();
    *page = db.search(query)?;
    Ok(())
}
fn query(text: &str, offset: usize, limit: usize) -> Query {
    let mut q = Query::new("shared");
    q.collection = Some("texts".into());
    q.text = text.into();
    q.offset = offset;
    q.limit = limit;
    q
}
fn check(matrix: &matrix::Matrix, case: &matrix::Case, page: &SearchPage) {
    assert_eq!(page.total, case.total);
    assert_eq!(page.sequence, 20);
    assert_eq!(page.hits.len(), case.ids.len());
    for (stored, id) in page.hits.iter().zip(&case.ids) {
        assert_eq!(stored, &matrix.documents[id]);
    }
}
fn shuffle(items: &mut [usize], state: &mut u64) {
    for n in (1..items.len()).rev() {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        items.swap(n, *state as usize % (n + 1));
    }
}
fn memory_probe(db: &Database, reuse: bool) -> input::Result<()> {
    let mut page = empty();
    for round in 0..3 {
        for (text, offset, limit, total) in [
            ("the", 0, 50, 10000usize),
            ("quixote", 0, 50, 56),
            ("he", 0, 50, 10000),
            ("quixote whale", 0, 50, 44),
            ("zx", 0, 50, 0),
            ("the zx", 0, 50, 0),
            ("the", 9950, 50, 10000),
            ("the", 0, 200, 10000),
        ] {
            run(db, &query(text, offset, limit), &mut page, reuse)?;
            assert_eq!(page.total, total);
            assert_eq!(page.sequence, 20);
            assert_eq!(page.hits.len(), total.saturating_sub(offset).min(limit));
            let mut previous = None;
            for s in &page.hits {
                let id: usize = s.document.key.id.parse()?;
                assert_eq!(
                    (&*s.document.key.scope, &*s.document.key.collection),
                    ("shared", "texts")
                );
                assert!(previous.is_none_or(|n| n < id));
                previous = Some(id);
                assert_eq!(s.version, (id / 500 + 1) as u64);
            }
            println!(
                "{}",
                json!({"phase":"query_memory_result","round":round,"text":text,"offset":offset,"limit":limit,"hits":page.hits.len()})
            );
        }
        // Match the ordinary oracle-free probe's lifetime and key reads.
        for n in 0..256 {
            let id = n * 9999 / 255;
            let key = siftwake::Key::new("shared", "texts", &format!("{id:010}"));
            let got = db.get(&key)?.ok_or("missing key")?;
            assert_eq!(got.document.key, key);
            assert_eq!(got.version, (id / 500 + 1) as u64);
        }
        assert!(
            db.get(&siftwake::Key::new("shared", "texts", "9999999999"))?
                .is_none()
        );
    }
    drop(page);
    println!(
        "{}",
        json!({"phase":"query_memory_verified","rounds":3,"searches":24,"gets":771,"oracle_loaded":false,"performance_not_measured":true,"full_payload_equality_in_separate_matrix":true})
    );
    Ok(())
}
fn main() -> input::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let reuse = args.iter().any(|s| s == "--reuse-results");
    let fresh = args.iter().any(|s| s == "--fresh-results");
    let path = Path::new("data/benchmark-work/fdb");
    if !path.is_dir() {
        return Err("existing fixed slot required".into());
    }
    if args.iter().any(|s| s == "--memory-probe") {
        let now = Instant::now();
        let db = Database::open(path)?;
        assert_eq!((db.stats()?.documents, db.stats()?.sequence), (10000, 20));
        println!(
            "{}",
            json!({"phase":"opened","engine":"fdb","open_ms":now.elapsed().as_secs_f64()*1000.,"pending_source_checks":db.pending_source_checks()?,"reuse":reuse})
        );
        return memory_probe(&db, reuse);
    }
    let mut matrix = matrix::Matrix::load()?;
    if args.iter().any(|s| s == "--retention-probe") {
        let db = Database::open(path)?;
        let mut page = empty();
        for (step, n) in [31, 31, 33, 31, 30, 37].into_iter().enumerate() {
            let c = &matrix.cases[n];
            let q = query(&c.text, c.offset, c.limit);
            #[cfg(feature = "diagnostic-timing")]
            let _ = siftwake::timing::take();
            #[cfg(feature = "diagnostic-memory")]
            let before = allocation::reset();
            run(&db, &q, &mut page, reuse)?;
            #[cfg(feature = "diagnostic-memory")]
            let (live, peak, calls) = allocation::snapshot();
            #[allow(unused_mut)]
            let mut row = json!({"phase":"retention","reuse":reuse,"step":step,"case_index":n,"text":c.text,"offset":c.offset,"limit":c.limit,"hits":page.hits.len(),"text_bytes":page.hits.iter().map(|s|s.document.text.len()).sum::<usize>(),"text_capacity":page.hits.iter().map(|s|s.document.text.capacity()).sum::<usize>(),"result_capacity":page.hits.capacity(),"full_document_checks":true});
            #[cfg(feature = "diagnostic-memory")]
            {
                row["live_requested_delta_bytes"] = json!(live as i64 - before as i64);
                row["additional_requested_peak_bytes"] = json!(peak.saturating_sub(before));
                row["allocation_calls"] = json!(calls);
            }
            #[cfg(feature = "diagnostic-timing")]
            {
                row["spans"] = json!(siftwake::timing::take());
            }
            check(&matrix, c, &page);
            println!("{row}");
        }
        return Ok(());
    }
    let pages_only = args.iter().any(|s| s == "--pages-only");
    let case_indices = if let Some(at) = args.iter().position(|s| s == "--case-indices") {
        let selected: Vec<usize> = args
            .get(at + 1)
            .ok_or("missing indices")?
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        let unique: std::collections::BTreeSet<_> = selected.iter().copied().collect();
        if selected.is_empty()
            || selected.len() != unique.len()
            || selected.iter().any(|n| *n >= 38)
        {
            return Err("bad indices".into());
        }
        matrix.cases = matrix
            .cases
            .into_iter()
            .enumerate()
            .filter_map(|(n, c)| unique.contains(&n).then_some(c))
            .collect();
        Some(selected)
    } else {
        None
    };
    if pages_only {
        matrix.cases.retain(|c| c.kind == "page");
    }
    println!(
        "{}",
        json!({"phase":"worker_ready","cases":matrix.cases.len(),"pages_only":pages_only,"case_indices":case_indices,"diagnostic_timing":cfg!(feature="diagnostic-timing"),"diagnostic_memory":cfg!(feature="diagnostic-memory"),"reuse_results":reuse,"previous_result_release_inside_timing":true})
    );
    std::io::stdout().flush()?;
    for line in std::io::stdin().lock().lines() {
        let command: Block = serde_json::from_str(&line?)?;
        if !(1..=200).contains(&command.repeats) {
            return Err("bad repeats".into());
        }
        let db = Database::open(path)?;
        assert_eq!(
            (
                db.stats()?.documents,
                db.stats()?.delta_keys,
                db.stats()?.sequence
            ),
            (10000, 0, 20)
        );
        let mut page = empty();
        for case in &matrix.cases {
            run(
                &db,
                &query(&case.text, case.offset, case.limit),
                &mut page,
                reuse,
            )?;
            check(&matrix, case, &page);
        }
        let mut state = command.seed + 1;
        let mut order: Vec<_> = (0..matrix.cases.len()).collect();
        let mut samples = vec![Vec::new(); matrix.cases.len()];
        for _ in 0..command.repeats {
            shuffle(&mut order, &mut state);
            for &n in &order {
                let c = &matrix.cases[n];
                if fresh {
                    page = empty();
                }
                let now = Instant::now();
                run(&db, &query(&c.text, c.offset, c.limit), &mut page, reuse)?;
                samples[n].push(now.elapsed().as_secs_f64() * 1000.);
                check(&matrix, c, &page);
            }
        }
        println!(
            "{}",
            json!({"phase":"matrix_reads","seed":command.seed,"repeats":command.repeats,"queries":matrix.cases.iter().enumerate().map(|(n,c)|json!({"text":c.text,"kind":c.kind,"offset":c.offset,"limit":c.limit,"total":c.total,"late_matches":c.late_matches,"timing":reads::summary(&samples[n])})).collect::<Vec<_>>(),"full_document_checks":true,"cache":"warm OS cache; each block full source validation and untimed warmup; no eviction","includes_previous_result_release":true,"includes_get_interleaving":false})
        );
        drop(page);
        assert_eq!(db.pending_source_checks()?, 0);
        drop(db);
        println!(
            "{}",
            json!({"phase":"block_done","id":command.id,"database_released":true})
        );
        std::io::stdout().flush()?;
    }
    Ok(())
}
