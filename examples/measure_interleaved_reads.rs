//! Short sequential blocks, preserving the database's exclusive-owner lock.
#[path = "../tools/bench_support/memory_corpus.rs"]
mod input;
#[path = "../tools/bench_support/read_matrix.rs"]
// Shared probes expose other entry points used by the ordinary matrix example.
#[allow(dead_code)]
mod matrix;
#[path = "../tools/bench_support/content_reads.rs"]
#[allow(dead_code)]
mod reads;
use fdb::{Database, Query};
use serde::Deserialize;
use serde_json::json;
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

fn main() -> input::Result<()> {
    let path = Path::new("data/benchmark-work/fdb");
    if !path.is_dir() {
        return Err("existing fixed FDB slot required".into());
    }
    let mut matrix = matrix::Matrix::load()?;
    let pages_only = std::env::args().any(|arg| arg == "--pages-only");
    if pages_only {
        matrix.cases.retain(|case| case.kind == "page");
    }
    println!(
        "{}",
        json!({"phase":"worker_ready","cases":matrix.cases.len(),"pages_only":pages_only,"no_database_handle_retained_between_blocks":true,"diagnostic_timing":cfg!(feature="diagnostic-timing")})
    );
    std::io::stdout().flush()?;
    for line in std::io::stdin().lock().lines() {
        let command: Block = serde_json::from_str(&line?)?;
        if !(1..=200).contains(&command.repeats) {
            return Err("block repeats must be 1..=200".into());
        }
        let start = Instant::now();
        let db = Database::open(path)?;
        let stats = db.stats()?;
        assert_eq!(
            (stats.documents, stats.delta_keys, stats.sequence),
            (10000, 0, 20)
        );
        println!(
            "{}",
            json!({"phase":"block_opened","id":command.id,"open_ms":start.elapsed().as_secs_f64()*1000.,"pending_source_checks":db.pending_source_checks()?,"outside_query_timing":true})
        );
        matrix.measure_rounds(
            command.seed,
            command.repeats,
            pages_only,
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
        )?;
        assert_eq!(db.pending_source_checks()?, 0);
        drop(db);
        println!(
            "{}",
            json!({"phase":"block_done","id":command.id,"database_released":true,"wall_ms_including_open_warmup_checks":start.elapsed().as_secs_f64()*1000.})
        );
        std::io::stdout().flush()?;
    }
    Ok(())
}
