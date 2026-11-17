//! Current-engine baseline for derived-profile targets; not a derived engine.
#[cfg(feature = "diagnostic-memory")]
#[path = "../tools/bench_support/tagged_allocation.rs"]
mod allocation;
#[path = "../tools/bench_support/memory_corpus.rs"]
mod input;
#[cfg(feature = "diagnostic-memory")]
#[global_allocator]
static ALLOCATOR: allocation::Tracking = allocation::Tracking;
use serde_json::json;
use siftwake::{Database, Guard, Mutation, Query};
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};
fn main() -> input::Result<()> {
    #[cfg(feature = "diagnostic-memory")]
    {
        siftwake::memory::install(allocation::origin);
        siftwake::memory::install_observer(allocation_checkpoint);
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 && !(args.len() == 5 && args[4] == "--streamed") {
        return Err("idle|ingest|verify <db> <from> <to>".into());
    }
    let streamed = args.len() == 5;
    #[cfg(not(feature = "bounded-engine"))]
    if streamed {
        return Err("streamed input requires the bounded-engine profile".into());
    }
    let from: usize = args[2].parse()?;
    let to: usize = args[3].parse()?;
    if to != 10000 || from != 0 {
        return Err("invalid range".into());
    }
    input::phase("runtime_baseline");
    thread::sleep(Duration::from_secs(1));
    input::phase("opening");
    let start = Instant::now();
    let mut db = Database::open(&args[1])?;
    let open_ms = start.elapsed().as_secs_f64() * 1000.;
    let stats = db.stats()?;
    assert_eq!(
        stats.documents,
        if args[0] == "idle" || args[0] == "verify" {
            to
        } else {
            from
        }
    );
    println!(
        "{}",
        json!({"phase":"opened","open_ms":open_ms,"stats":stats})
    );
    if args[0] == "idle" {
        input::phase("idle_wait");
        thread::sleep(Duration::from_secs(5));
        input::phase("idle_ready");
        thread::sleep(Duration::from_secs(2));
        return Ok(());
    }
    if args[0] == "verify" {
        return verify(&db, to);
    }
    if args[0] != "ingest" {
        return Err("unknown mode".into());
    }
    let mut source = input::Input::open()?;
    source.skip(from)?;
    input::phase("indexing");
    let start = Instant::now();
    for first in (from..to).step_by(500) {
        #[cfg(feature = "diagnostic-memory")]
        let caller_origin = siftwake::memory::Scope::new(1);
        let mut expected = Vec::new();
        let changes = (first..first + 500).map(|id| -> input::Result<Mutation> {
            let stored = {
                #[cfg(feature = "diagnostic-timing")]
                let _input = siftwake::timing::span("input.corpus_crc_json");
                source.read()?
            };
            if id == first || id == first + 499 {
                expected.push(stored.clone());
            }
            Ok(Mutation::Put {
                document: stored.document,
                guard: Guard::Missing,
            })
        });
        let sequence = if streamed {
            #[cfg(feature = "bounded-engine")]
            {
                db.commit_streamed(
                    changes.map(|r| r.map_err(|e| siftwake::Error::Invalid(e.to_string()))),
                )?
            }
            #[cfg(not(feature = "bounded-engine"))]
            {
                unreachable!()
            }
        } else {
            db.commit(changes.collect::<input::Result<Vec<_>>>()?)?
        };
        assert_eq!(sequence, first as u64 / 500 + 1);
        #[cfg(feature = "diagnostic-memory")]
        drop(caller_origin);
        for stored in expected {
            assert_eq!(db.get(&stored.document.key)?, Some(stored));
        }
        #[cfg(feature = "diagnostic-memory")]
        {
            let (live, peak, total, total_peak) = allocation::snapshot();
            println!(
                "{}",
                json!({"phase":"allocation","records":first+500,"origin_names":siftwake::memory::NAMES,"requested_live":live,"requested_peak":peak,"total_live":total,"total_peak":total_peak,"not_rss":true,"not_normal_timing":true})
            );
        }
        if (first + 500).is_multiple_of(10000) {
            println!(
                "{}",
                json!({"phase":"indexing","records":first+500,"elapsed_s":start.elapsed().as_secs_f64()})
            );
        }
    }
    let ingest_ms = start.elapsed().as_secs_f64() * 1000.;
    #[cfg(feature = "diagnostic-timing")]
    {
        siftwake::timing::mark_drain();
        println!("{}", json!({"phase":"before_drain","stats":db.stats()?}));
    }
    #[cfg(feature = "diagnostic-timing")]
    println!(
        "{}",
        json!({"phase":"profile_ingest","counters":siftwake::timing::take(),"instrumented":true,"overlapping_spans_do_not_sum":true})
    );
    input::phase("maintenance");
    let start = Instant::now();
    db.flush()?;
    db.wait_for_compaction()?;
    while db.start_compaction()? {
        db.wait_for_compaction()?;
    }
    let maintenance_ms = start.elapsed().as_secs_f64() * 1000.;
    #[cfg(feature = "experimental-write-buffer-budget")]
    println!(
        "{}",
        json!({"phase":"controlled_buffer_budget","write":siftwake::write_budget::snapshot(),"not_total_process_memory":true})
    );
    #[cfg(feature = "bounded-engine")]
    println!(
        "{}",
        json!({"phase":"controlled_posting_budget","posting":siftwake::write_budget::posting_snapshot(),"working":siftwake::write_budget::working_snapshot(),"streamed_input":streamed,"not_total_process_memory":true})
    );
    #[cfg(feature = "diagnostic-timing")]
    println!(
        "{}",
        json!({"phase":"profile_maintenance","counters":siftwake::timing::take(),"instrumented":true,"overlapping_spans_do_not_sum":true})
    );
    #[cfg(feature = "diagnostic-memory")]
    {
        let (live, peak, total, total_peak) = allocation::snapshot();
        println!(
            "{}",
            json!({"phase":"allocation_final","origin_names":siftwake::memory::NAMES,"requested_live":live,"requested_peak":peak,"total_live":total,"total_peak":total_peak,"not_rss":true,"not_normal_timing":true})
        );
    }
    let stats = db.stats()?;
    assert_eq!(stats.documents, to);
    assert_eq!(stats.resident_documents, 0);
    println!(
        "{}",
        json!({"phase":"result","from":from,"to":to,"ingest_ms":ingest_ms,"maintenance_ms":maintenance_ms,"text_bytes":source.text_bytes,"stats":stats,"sampled_full_document_checks":(to-from)/250,"all_input_crc_verified":true,"database":Path::new(&args[1])})
    );
    Ok(())
}

#[cfg(feature = "diagnostic-memory")]
fn allocation_checkpoint(stage: &'static str, sequence: u64) {
    let (live, peak, total, total_peak) = allocation::snapshot();
    println!(
        "{}",
        json!({"phase":"allocation_checkpoint","stage":stage,"sequence":sequence,
            "origin_names":siftwake::memory::NAMES,"requested_live":live,"requested_peak":peak,
            "total_live":total,"total_peak":total_peak,"not_rss":true,"not_normal_timing":true})
    );
}

// Independent scan oracle, outside all measured ingest/maintenance processes.
fn verify(db: &Database, count: usize) -> input::Result<()> {
    let queries = ["the", "quixote", "he", "the man"];
    let mut totals = [0usize; 4];
    let mut expected = vec![Vec::new(); 4];
    let mut source = input::Input::open()?;
    source.skip(0)?;
    let mut previous = None;
    for _ in 0..count {
        let stored = source.read()?;
        assert_eq!(stored.document.key.scope, "shared");
        assert_eq!(stored.document.key.collection, "texts");
        assert!(
            previous
                .as_ref()
                .is_none_or(|key| key < &stored.document.key)
        );
        previous = Some(stored.document.key.clone());
        assert_eq!(db.get(&stored.document.key)?, Some(stored.clone()));
        let body: String = format!("{}\n{}", stored.document.title, stored.document.text)
            .chars()
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
            .collect();
        for (n, text) in queries.iter().enumerate() {
            if text.split_whitespace().all(|term| body.contains(term)) {
                totals[n] += 1;
                if expected[n].len() < 50 {
                    expected[n].push(stored.clone());
                }
            }
        }
    }
    for (n, text) in queries.iter().enumerate() {
        let mut query = Query::new("shared");
        query.collection = Some("texts".into());
        query.text = (*text).into();
        let page = db.search(&query)?;
        assert_eq!(page.total, totals[n], "{text}");
        assert_eq!(page.hits, expected[n], "{text}");
        assert_eq!(page.sequence, 20);
    }
    println!(
        "{}",
        json!({"phase":"verified","full_document_checks":count,"queries":queries,"totals":totals,"first50_full_pages_verified":true,"input_crc_verified":true,"outside_timing":true})
    );
    Ok(())
}
