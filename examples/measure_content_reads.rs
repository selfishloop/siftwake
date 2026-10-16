#[cfg(feature = "diagnostic-timing")]
#[path = "../tools/bench_support/thread_cycles.rs"]
mod cycles;
#[path = "../tools/bench_support/memory_corpus.rs"]
mod input;
#[path = "../tools/bench_support/read_matrix.rs"]
mod matrix;
#[path = "../tools/bench_support/content_reads.rs"]
mod reads;
use fdb::{Database, Query};
use serde_json::json;
use std::{path::Path, time::Instant};
fn main() -> input::Result<()> {
    let path = "data/benchmark-work/fdb";
    if !Path::new(path).is_dir() {
        return Err("existing FDB slot required; this probe never creates a database".into());
    }
    let start = Instant::now();
    let db = if std::env::args().any(|arg| arg == "--record-reads") {
        Database::open_record_reads(path)?
    } else if std::env::args().any(|arg| arg == "--deferred") {
        Database::open_deferred(path)?
    } else {
        Database::open(path)?
    };
    let open_ms = start.elapsed().as_secs_f64() * 1000.;
    let stats = db.stats()?;
    assert_eq!(stats.documents, 10000);
    assert_eq!(stats.delta_keys, 0);
    println!(
        "{}",
        json!({"phase":"opened","engine":"fdb","open_ms":open_ms,"stats":stats,"source_validation_on_open":!std::env::args().any(|arg| arg == "--deferred" || arg == "--record-reads"),"pending_source_checks":db.pending_source_checks()?})
    );
    if std::env::args().any(|arg| arg == "--prepare-record-reads") {
        let start = Instant::now();
        db.prepare_record_reads()?;
        println!(
            "{}",
            json!({"phase":"record_reads_prepared","elapsed_ms":start.elapsed().as_secs_f64()*1000.})
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--prepare-open-checks") {
        let start = Instant::now();
        db.prepare_open_checks()?;
        println!(
            "{}",
            json!({"phase":"open_checks_prepared","elapsed_ms":start.elapsed().as_secs_f64()*1000.})
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--check-integrity") {
        let start = Instant::now();
        db.check_integrity()?;
        println!(
            "{}",
            json!({"phase":"integrity_checked","elapsed_ms":start.elapsed().as_secs_f64()*1000.,"pending_source_checks":db.pending_source_checks()?})
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--deferred-probe") {
        #[cfg(feature = "diagnostic-timing")]
        let _ = fdb::timing::take();
        let pending = db.pending_source_checks()?;
        let start = Instant::now();
        let doc = db
            .get(&fdb::Key::new("shared", "texts", "0000009999"))?
            .unwrap();
        let first_get_ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(doc.version, 20);
        let after_get = db.pending_source_checks()?;
        #[cfg(feature = "diagnostic-timing")]
        println!(
            "{}",
            json!({"phase":"first_get_profile","counters":fdb::timing::take(),"nested_spans_do_not_sum":true})
        );
        let mut query = Query::new("shared");
        query.collection = Some("texts".into());
        query.text = "quixote".into();
        let start = Instant::now();
        let page = db.search(&query)?;
        let first_search_ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(page.total, 56);
        assert_eq!(page.hits.len(), 50);
        assert_eq!(db.pending_source_checks()?, 0);
        #[cfg(feature = "diagnostic-timing")]
        println!(
            "{}",
            json!({"phase":"first_search_profile","counters":fdb::timing::take(),"nested_spans_do_not_sum":true})
        );
        let start = Instant::now();
        let repeated = db.search(&query)?;
        let warm_search_ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(page.hits, repeated.hits);
        println!(
            "{}",
            json!({"phase":"deferred_first_use","pending_on_open":pending,"pending_after_get":after_get,"first_get_ms":first_get_ms,"first_search_ms":first_search_ms,"warm_search_ms":warm_search_ms,"first_use_includes_segment_verification":true})
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--open-only") {
        #[cfg(feature = "diagnostic-timing")]
        println!(
            "{}",
            json!({"phase":"open_profile","counters":fdb::timing::take(),"instrumented":true,"nested_spans_do_not_sum":true})
        );
        return Ok(());
    }
    if let Some(seed) = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|args| args[0] == "--matrix")
        .map(|args| args[1].parse::<u64>())
        .transpose()?
    {
        let mut matrix = matrix::Matrix::load()?;
        if std::env::args().any(|arg| arg == "--matrix-check") {
            for case in &matrix.cases {
                let mut query = Query::new("shared");
                query.collection = Some("texts".into());
                query.text = case.text.clone();
                query.offset = case.offset;
                query.limit = case.limit;
                let page = db.search(&query)?;
                let expected: Vec<_> = case
                    .ids
                    .iter()
                    .map(|id| matrix.documents[id].clone())
                    .collect();
                assert_eq!(page.total, case.total, "{}", case.text);
                assert_eq!(page.hits, expected, "{}", case.text);
                assert_eq!(page.sequence, 20);
            }
            println!(
                "{}",
                json!({"phase":"matrix_checked","cases":matrix.cases.len(),"full_pages_and_versions_verified":true,"performance_not_measured":true})
            );
            return Ok(());
        }
        if std::env::args().any(|arg| arg == "--stress-only") {
            matrix.cases.retain(|case| {
                matches!(
                    case.kind,
                    "short-negative" | "negative-and" | "tail-token" | "long-negative"
                )
            });
        }
        if std::env::args().any(|arg| arg == "--pages-only") {
            matrix.cases.retain(|case| case.kind == "page");
        }
        if std::env::args().any(|arg| arg == "--oracle-only") {
            return Ok(());
        }
        return matrix.measure(
            seed,
            |key| Ok(db.get(key)?),
            |text, offset, limit| {
                let mut query = Query::new("shared");
                query.collection = Some("texts".into());
                query.text = text.into();
                query.offset = offset;
                query.limit = limit;
                let page = db.search(&query)?;
                assert_eq!(page.sequence, 20);
                Ok((page.total, page.hits))
            },
        );
    }
    #[cfg(feature = "diagnostic-timing")]
    if std::env::args().any(|arg| {
        matches!(
            arg.as_str(),
            "--profile-selectivity"
                | "--profile-regressions"
                | "--profile-covering"
                | "--profile-pages"
        )
    }) {
        if std::env::args().any(|arg| arg == "--profile-covering") {
            let pending = db.pending_source_checks()?;
            let _ = fdb::timing::take();
            let mut query = Query::new("shared");
            query.collection = Some("texts".into());
            query.text = "the".into();
            let start = Instant::now();
            let page = db.search(&query)?;
            let first_ms = start.elapsed().as_secs_f64() * 1000.;
            assert_eq!(page.total, 10000);
            assert_eq!(page.hits.len(), 50);
            assert_eq!(page.sequence, 20);
            assert_eq!(db.pending_source_checks()?, 0);
            println!(
                "{}",
                json!({"phase":"covered_first_use","text":"the","elapsed_ms":first_ms,"pending_before":pending,"pending_after":0,"counters":fdb::timing::take(),"instrumented":true,"no_os_cache_eviction":true,"includes_source_validation":true,"nested_spans_do_not_sum":true})
            );
        }
        let matrix = matrix::Matrix::load()?;
        let regressions = std::env::args().any(|arg| arg == "--profile-regressions");
        let covering = std::env::args().any(|arg| arg == "--profile-covering");
        let pages = std::env::args().any(|arg| arg == "--profile-pages");
        for case in matrix.cases.iter().filter(|case| {
            if pages {
                case.kind == "page"
            } else if covering {
                matches!(
                    case.text.as_str(),
                    "the" | "man" | "he" | "quixote" | "the zx"
                )
            } else if regressions {
                (case.offset != 60
                    && matches!(
                        case.text.as_str(),
                        "zeuglodon"
                            | "transubstantiation"
                            | "harpoon"
                            | "ishmael"
                            | "darcy"
                            | "watson"
                            | "geologist"
                            | "presumption"
                    ))
                    || case.kind == "long-negative"
                    || (case.text == "quixote" && case.offset == 57)
            } else {
                case.offset == 0
                    && matches!(
                        case.text.as_str(),
                        "beagle"
                            | "holmes elementary"
                            | "transubstantiation"
                            | "zx"
                            | "quixote"
                            | "the"
                    )
            }
        }) {
            let text = &case.text;
            let mut query = Query::new("shared");
            query.collection = Some("texts".into());
            query.text = text.into();
            query.limit = case.limit;
            query.offset = case.offset;
            let _ = db.search(&query)?;
            let _ = fdb::timing::take();
            for iteration in 0..if pages { 30 } else { 3 } {
                let cycle_start = cycles::read();
                let start = Instant::now();
                let page = db.search(&query)?;
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
                let thread_cycles = cycle_start
                    .zip(cycles::read())
                    .and_then(|(first, last)| last.checked_sub(first));
                let counters = fdb::timing::take();
                assert_eq!(page.total, case.total);
                let expected: Vec<_> = case
                    .ids
                    .iter()
                    .map(|id| matrix.documents[id].clone())
                    .collect();
                assert_eq!(page.hits, expected);
                println!(
                    "{}",
                    json!({"phase":"selectivity_profile","text":text,"offset":case.offset,"limit":case.limit,"iteration":iteration,"total":page.total,"elapsed_ms":elapsed_ms,"thread_cycles":thread_cycles,"counters":counters,"instrumented":true,"nested_spans_do_not_sum":true})
                );
            }
        }
        return Ok(());
    }
    let oracle = reads::Oracle::load()?;
    #[cfg(feature = "diagnostic-timing")]
    if std::env::args().any(|arg| arg == "--profile") {
        for (n, text) in reads::QUERIES.iter().enumerate() {
            let mut query = Query::new("shared");
            query.collection = Some("texts".into());
            query.text = (*text).into();
            let _ = db.search(&query)?;
            let _ = fdb::timing::take();
            for iteration in 0..3 {
                let page = db.search(&query)?;
                let counters = fdb::timing::take();
                assert_eq!(page.total, oracle.totals[n]);
                assert_eq!(page.hits, oracle.pages[n]);
                println!(
                    "{}",
                    json!({"phase":"query_profile","text":text,"iteration":iteration,"counters":counters,"instrumented":true,"nested_spans_do_not_sum":true})
                );
            }
        }
        return Ok(());
    }
    reads::measure(
        &oracle,
        |key| Ok(db.get(key)?),
        |text| {
            let mut query = Query::new("shared");
            query.collection = Some("texts".into());
            query.text = text.into();
            let page = db.search(&query)?;
            assert_eq!(page.sequence, 20);
            Ok((page.total, page.hits))
        },
    )
}
