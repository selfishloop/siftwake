#[cfg(feature = "diagnostic-memory")]
#[path = "../tools/bench_support/allocation.rs"]
mod allocation;
#[cfg(feature = "diagnostic-timing")]
#[path = "../tools/bench_support/thread_cycles.rs"]
mod cycles;
#[cfg(feature = "diagnostic-memory")]
#[global_allocator]
static ALLOCATOR: allocation::Tracking = allocation::Tracking;
#[path = "../tools/bench_support/memory_corpus.rs"]
mod input;
#[path = "../tools/bench_support/read_matrix.rs"]
mod matrix;
#[path = "../tools/bench_support/read_probe.rs"]
mod read_probe;
#[path = "../tools/bench_support/content_reads.rs"]
mod reads;
use serde_json::json;
use siftwake::{Database, Query};
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
    #[cfg(feature = "diagnostic-memory")]
    if std::env::args().any(|arg| arg == "--allocation-matrix") {
        if cfg!(feature = "diagnostic-timing") {
            return Err("run allocation and timing diagnostics separately".into());
        }
        let matrix = matrix::Matrix::load()?;
        for case in &matrix.cases {
            let mut query = Query::new("shared");
            query.collection = Some("texts".into());
            query.text = case.text.clone();
            query.offset = case.offset;
            query.limit = case.limit;
            let expected: Vec<_> = case
                .ids
                .iter()
                .map(|id| matrix.documents[id].clone())
                .collect();
            assert_eq!(db.search(&query)?.hits, expected);
            for iteration in 0..3 {
                let before = allocation::reset();
                let page = db.search(&query)?;
                let (live, peak, calls) = allocation::snapshot();
                assert_eq!(page.total, case.total);
                assert_eq!(page.hits, expected);
                assert_eq!(page.sequence, 20);
                println!(
                    "{}",
                    json!({"phase":"query_allocation","text":case.text,"offset":case.offset,"limit":case.limit,"iteration":iteration,"hits":page.hits.len(),"capacity":page.hits.capacity(),"stored_document_bytes":std::mem::size_of::<siftwake::StoredDocument>(),"additional_requested_peak_bytes":peak.saturating_sub(before),"live_requested_delta_bytes":live as i64-before as i64,"allocation_calls":calls,"includes_os_memory":false,"timings_are_invalid":true,"full_document_and_version_checks":true})
                );
            }
        }
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--memory-probe") {
        return read_probe::measure(
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
        let _ = siftwake::timing::take();
        let pending = db.pending_source_checks()?;
        let start = Instant::now();
        let doc = db
            .get(&siftwake::Key::new("shared", "texts", "0000009999"))?
            .unwrap();
        let first_get_ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(doc.version, 20);
        let after_get = db.pending_source_checks()?;
        #[cfg(feature = "diagnostic-timing")]
        println!(
            "{}",
            json!({"phase":"first_get_profile","counters":siftwake::timing::take(),"nested_spans_do_not_sum":true})
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
            json!({"phase":"first_search_profile","counters":siftwake::timing::take(),"nested_spans_do_not_sum":true})
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
            json!({"phase":"open_profile","counters":siftwake::timing::take(),"instrumented":true,"nested_spans_do_not_sum":true})
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
            "--profile-matrix"
                | "--profile-selectivity"
                | "--profile-regressions"
                | "--profile-covering"
                | "--profile-pages"
        )
    }) {
        if std::env::args().any(|arg| arg == "--profile-covering") {
            let pending = db.pending_source_checks()?;
            let _ = siftwake::timing::take();
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
                json!({"phase":"covered_first_use","text":"the","elapsed_ms":first_ms,"pending_before":pending,"pending_after":0,"counters":siftwake::timing::take(),"instrumented":true,"no_os_cache_eviction":true,"includes_source_validation":true,"nested_spans_do_not_sum":true})
            );
        }
        let matrix = matrix::Matrix::load()?;
        let regressions = std::env::args().any(|arg| arg == "--profile-regressions");
        let covering = std::env::args().any(|arg| arg == "--profile-covering");
        let pages = std::env::args().any(|arg| arg == "--profile-pages");
        let all = std::env::args().any(|arg| arg == "--profile-matrix");
        for case in matrix.cases.iter().filter(|case| {
            if all {
                true
            } else if pages {
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
            let _ = siftwake::timing::take();
            for iteration in 0..if pages { 30 } else { 3 } {
                let cycle_start = cycles::read();
                let start = Instant::now();
                let page = db.search(&query)?;
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
                let thread_cycles = cycle_start
                    .zip(cycles::read())
                    .and_then(|(first, last)| last.checked_sub(first));
                let counters = siftwake::timing::take();
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
            let _ = siftwake::timing::take();
            for iteration in 0..3 {
                let page = db.search(&query)?;
                let counters = siftwake::timing::take();
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
