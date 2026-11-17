//! Fixed 50k case-0 benchmark. The model and full oracle stay out of the timed path.
use super::churn::{self, Corpus, Model, Result};
use serde::Deserialize;
use serde_json::json;
use siftwake::{FirstPage, Key, MatchTotal, Query, StoredDocument, ValidationStatus};
use std::{
    fs,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub trait Reader {
    fn get(&mut self, key: &Key) -> Result<Option<StoredDocument>>;
    fn first(&mut self, query: &Query) -> Result<FirstPage>;
}

#[derive(Deserialize)]
struct Expected {
    case: usize,
    text: String,
    expected_crc32: u32,
    oracle_df: usize,
    returned: usize,
}

fn check(page: &FirstPage, expected: &Expected, sequence: u64) -> Result<()> {
    assert_eq!(page.sequence, sequence);
    assert_eq!(page.validation, ValidationStatus::Verified);
    assert_eq!(page.hits.len(), expected.returned);
    match page.total {
        MatchTotal::Exact(n) => assert_eq!(n, expected.oracle_df),
        MatchTotal::AtLeast(n) => assert!(n == 50 && expected.oracle_df >= n),
    }
    assert_eq!(
        crc32fast::hash(&serde_json::to_vec(&page.hits)?),
        expected.expected_crc32,
        "case {} digest",
        expected.case
    );
    Ok(())
}

fn query(text: &str) -> Query {
    let mut q = Query::new("shared");
    q.collection = Some("texts".into());
    q.limit = 50;
    q.text = text.into();
    q
}

fn unix_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

pub fn run<D: Reader>(engine: &str, open: impl FnOnce() -> Result<D>) -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("MODE MODEL_PATH ORACLE_JSONL".into());
    }
    let mode = &args[0];
    let oracle = fs::read_to_string(&args[2])?;
    let expected: Vec<Expected> = oracle
        .lines()
        .filter(|line| line.contains("\"phase\":\"case_digest\""))
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    assert_eq!(expected.len(), 41);
    assert_eq!(expected[0].text, "olduğu");
    let sequence = match engine {
        "fdb" => 529967,
        "sqlite" => 1026311,
        _ => return Err("unknown engine".into()),
    };
    match mode.as_str() {
        "cold" | "warm" => {
            let q = query(&expected[0].text);
            let start_utc_ns = unix_ns();
            let start = Instant::now();
            let mut db = open()?;
            let open_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut times = Vec::with_capacity(150);
            let mut first_result = None;
            let count = if mode == "cold" { 30 } else { 150 };
            for index in 0..count {
                let at = Instant::now();
                let page = db.first(&q)?;
                let elapsed = at.elapsed().as_secs_f64() * 1000.;
                if index == 0 {
                    first_result = Some((start.elapsed().as_secs_f64() * 1000., unix_ns()));
                }
                times.push(elapsed);
                check(&page, &expected[0], sequence)?;
            }
            let (first_ms, first_utc_ns) = first_result.unwrap();
            println!(
                "{}",
                json!({"engine":engine,"mode":mode,"pid":std::process::id(),
                "open_start_unix_ns":start_utc_ns.to_string(),"first_result_unix_ns":first_utc_ns.to_string(),
                "open_ms":open_ms,"open_start_to_first_ms":first_ms,"query_ms":times,
                "sequence":sequence,"crc32":expected[0].expected_crc32,"oracle":true})
            );
        }
        "verify" | "hold" => {
            let model = Model::load(&args[1])?;
            assert_eq!(model.rows.len(), 50000);
            assert_eq!(model.sequence, sequence);
            let mut corpus = Corpus::open()?;
            let mut db = open()?;
            for (&id, row) in &model.rows {
                let wanted = StoredDocument {
                    document: corpus.document(id, row)?,
                    version: row.version,
                };
                assert_eq!(db.get(&churn::key(id))?, Some(wanted), "GET/version {id}");
            }
            for (i, case) in expected.iter().enumerate() {
                assert_eq!(case.case, i);
                assert_eq!(case.text, corpus.index.cases[i].text);
                check(&db.first(&query(&case.text))?, case, sequence)?;
            }
            println!(
                "{}",
                json!({"engine":engine,"mode":mode,"sequence":sequence,
                "gets":50000,"cases":41,"oracle":true,"kill_ready":mode=="hold"})
            );
            if mode == "hold" {
                use std::io::Write;
                std::io::stdout().flush()?;
                loop {
                    std::thread::park();
                }
            }
        }
        _ => return Err("mode must be cold, warm, verify or hold".into()),
    }
    Ok(())
}
