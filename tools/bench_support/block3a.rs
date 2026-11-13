//! Fixed-size diagnostic only: no reset, ingest, optimization or maintenance loop.
use super::{
    churn,
    key_pool::{Client, Factory, Result, Store, View},
};
use fdb::{Document, FirstPage, Guard, Key, MatchTotal, Mutation, Query, StoredDocument};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Seek, SeekFrom},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};
const COL: &str = "keybench-numeric";
const N: usize = 16;
#[derive(Deserialize)]
struct Entry {
    offset: u64,
    size: usize,
    crc: u32,
}
fn query(collection: &str, text: &str) -> Query {
    let mut q = Query::new("shared");
    q.collection = Some(collection.into());
    q.text = text.into();
    q.limit = 50;
    q
}
fn marker(slot: usize, revision: usize) -> String {
    format!("zqdiag{slot:04}r{revision:08}zq")
}
fn changed(template: &Document, slot: usize, revision: usize) -> Document {
    let mut doc = template.clone();
    doc.key = Key::new("shared", COL, &format!("{slot:010}"));
    doc.text.push(' ');
    doc.text.push_str(&marker(slot, revision));
    doc
}
fn check(page: &FirstPage, expected: &[StoredDocument], sequence: u64) {
    assert_eq!(page.sequence, sequence);
    assert_eq!(page.hits, expected);
    assert_eq!(
        page.total,
        if expected.len() < 50 {
            MatchTotal::Exact(expected.len())
        } else {
            MatchTotal::AtLeast(50)
        }
    );
}
fn audit<S: Store>(db: &mut S, model: &churn::Model, corpus: &mut churn::Corpus) -> Result<()> {
    let factory = db.factory()?;
    let mut client = factory.open()?;
    let view = client.acquire()?;
    for (&id, row) in &model.rows {
        assert_eq!(
            view.get(&churn::key(id))?,
            Some(StoredDocument {
                document: corpus.document(id, row)?,
                version: row.version
            })
        );
    }
    for &id in model.dead.keys() {
        assert!(view.get(&churn::key(id))?.is_none());
    }
    churn::emit(
        json!({"phase":"main_oracle","documents":model.rows.len(),"sequence":view.sequence(),"oracle":true}),
    );
    Ok(())
}
fn scan<S: Store>(db: &mut S, label: &str, expected: &[Vec<StoredDocument>]) -> Result<()> {
    let factory = db.factory()?;
    let mut client = factory.open()?;
    for (i, text) in ["zx", "the zx"].iter().enumerate() {
        // SQLite's diagnostic flag belongs to a read transaction; a fresh view
        // prevents the preceding query's sampled profile from leaking into N60.
        let view = client.acquire()?;
        let q = query("texts", text);
        for repeat in 0..63 {
            let profiled = repeat >= 61;
            let finish = profiled.then(|| view.begin_probe_diagnostic());
            let cpu = churn::cpu_ms();
            let at = Instant::now();
            let page = view.first(&q)?;
            let ms = at.elapsed().as_secs_f64() * 1000.;
            let cpu_ms = cpu.zip(churn::cpu_ms()).map(|(a, b)| b - a);
            let profile = finish.map(|f| f());
            check(&page, &expected[i], view.sequence());
            churn::emit(
                json!({"phase":"scan","cell":label,"text":text,"ordinal":repeat,"warmup":repeat==0,"profiled":profiled,"ms":ms,"cpu_ms":cpu_ms,"sequence":page.sequence,"returned":page.hits.len(),"profile":profile,"oracle":true}),
            );
        }
    }
    Ok(())
}
fn gap<S: Store>(
    db: &mut S,
    label: &str,
    templates: &[Document],
    live: &mut [StoredDocument],
    revision: &mut usize,
) -> Result<()> {
    let factory = db.factory()?;
    let mut main = factory.open()?;
    let pinned = main.acquire()?;
    let old = live.to_vec();
    let oldseq = pinned.sequence();
    let barrier = Arc::new(Barrier::new(5));
    let active = Arc::new(AtomicUsize::new(4));
    std::thread::scope(|scope| -> Result<()> {
        let mut threads = vec![];
        for reader in 0..4 {
            let factory = factory.clone();
            let barrier = barrier.clone();
            let active = active.clone();
            threads.push(scope.spawn(move || -> Result<()> {
                let mut client = factory.open()?;
                barrier.wait();
                for _ in 0..64 {
                    let view = client.acquire()?;
                    let text = ["zqdiag", "the", "zx", "qzxkvwnever"][reader];
                    let page = view.first(&query(COL, text))?;
                    let mut expected = vec![];
                    for slot in 0..N {
                        let row = view
                            .get(&Key::new("shared", COL, &format!("{slot:010}")))?
                            .ok_or("pool key missing")?;
                        assert!(row.version <= view.sequence());
                        let folded =
                            churn::fold(&format!("{}\n{}", row.document.title, row.document.text));
                        if text.split_whitespace().all(|term| folded.contains(term)) {
                            expected.push(row);
                        }
                    }
                    check(&page, &expected, view.sequence());
                }
                churn::emit(
                    json!({"phase":"load","cell":label,"reader":reader,"calls":64,"oracle":true}),
                );
                active.fetch_sub(1, Ordering::Release);
                Ok(())
            }));
        }
        barrier.wait();
        for ordinal in 0..64 {
            let active_readers = active.load(Ordering::Acquire);
            let slot = ordinal % N;
            *revision += 1;
            // Identical query bytes and bodies in every A/A/layout cell.
            let probe_revision = (ordinal / N) % 2 + 1;
            let oldmarker = live[slot]
                .document
                .text
                .split_whitespace()
                .last()
                .unwrap()
                .to_string();
            let doc = changed(&templates[slot], slot, probe_revision);
            let profiled = ordinal % 16 == 0;
            if profiled {
                db.begin_commit_diagnostic()?;
            }
            let began = Instant::now();
            let seq = db.commit(vec![Mutation::Put {
                document: doc.clone(),
                guard: Guard::Any,
            }])?;
            let commit_ns = began.elapsed().as_nanos() as u64;
            let acquire_at = Instant::now();
            let view = main.acquire()?;
            let acquire_ns = acquire_at.elapsed().as_nanos() as u64;
            let q = query(COL, &marker(slot, probe_revision));
            let finish = profiled.then(|| view.begin_probe_diagnostic());
            let query_start_ns = began.elapsed().as_nanos() as u64;
            let page = view.first(&q)?;
            let query_end_ns = began.elapsed().as_nanos() as u64;
            let diagnostic = finish.map(|f| f());
            live[slot] = StoredDocument {
                document: doc,
                version: seq,
            };
            check(&page, &live[slot..slot + 1], seq);
            assert_eq!(
                view.get(&live[slot].document.key)?,
                Some(live[slot].clone())
            );
            check(&view.first(&query(COL, &oldmarker))?, &[], seq);
            for row in &old {
                assert_eq!(pinned.get(&row.document.key)?, Some(row.clone()));
            }
            churn::emit(
                json!({"phase":"gap","cell":label,"ordinal":ordinal,"profiled":profiled,"sequence":seq,"active_readers":active_readers,"commit_ms":commit_ns as f64/1e6,"gap_ms":(query_end_ns-commit_ns) as f64/1e6,"acquire_ms":acquire_ns as f64/1e6,"query_ms":(query_end_ns-query_start_ns) as f64/1e6,"between_ms":(query_start_ns-commit_ns-acquire_ns) as f64/1e6,"profile":diagnostic,"commit_profile":if profiled {db.take_commit_diagnostic()} else {Value::Null},"oracle":true}),
            );
        }
        for t in threads {
            t.join().map_err(|_| "reader panicked")??;
        }
        Ok(())
    })?;
    for row in &old {
        assert_eq!(pinned.get(&row.document.key)?, Some(row.clone()));
    }
    churn::emit(json!({"phase":"old_snapshot","cell":label,"sequence":oldseq,"oracle":true}));
    Ok(())
}
pub fn run<S: Store>(db: &mut S, state: &str, donors: &str) -> Result<()> {
    let mut model = churn::Model::load(state)?;
    assert_eq!(model.rows.len(), 50000);
    assert_eq!(db.sequence()?, model.sequence);
    let mut corpus = churn::Corpus::open()?;
    let entries: Vec<Entry> = serde_json::from_slice(&std::fs::read(donors)?)?;
    assert_eq!(entries.len(), N);
    let mut file = std::fs::File::open(std::env::var("FDB_BENCH_CORPUS_PATH")?)?;
    let mut templates = vec![];
    for e in entries {
        file.seek(SeekFrom::Start(e.offset))?;
        let mut raw = vec![0; e.size];
        file.read_exact(&mut raw)?;
        assert_eq!(crc32fast::hash(&raw), e.crc);
        let stored: StoredDocument = serde_json::from_slice(&raw)?;
        assert!((2048..=4000).contains(&stored.document.text.len()));
        templates.push(stored.document);
    }
    assert!(
        db.keys(COL)?.is_empty(),
        "existing pool must be empty; do not overwrite"
    );
    let mut live: Vec<StoredDocument> = vec![];
    let mut expected = vec![];
    let mut revision = 0;
    let mut maintained = false;
    let mut cells = 0;
    churn::emit(
        json!({"phase":"ready","sequence":db.sequence()?,"layout":db.diagnostic_layout()?}),
    );
    for line in std::io::stdin().lock().lines() {
        let command = line?;
        let at = Instant::now();
        match command.as_str() {
            "audit" => audit(db, &model, &mut corpus)?,
            "setup" => {
                assert!(live.is_empty());
                let seq = db.commit(
                    templates
                        .iter()
                        .enumerate()
                        .map(|(slot, d)| {
                            Ok(Mutation::Put {
                                document: changed(d, slot, 2),
                                guard: Guard::Missing,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                )?;
                live = templates
                    .iter()
                    .enumerate()
                    .map(|(slot, d)| StoredDocument {
                        document: changed(d, slot, 2),
                        version: seq,
                    })
                    .collect();
                // Offline substring oracle from original main bodies, outside timers.
                let mut selected: BTreeMap<String, Vec<StoredDocument>> =
                    [("zx".into(), vec![]), ("the zx".into(), vec![])].into();
                for (&id, row) in &model.rows {
                    let doc = corpus.document(id, row)?;
                    let folded = churn::fold(&format!("{}\n{}", doc.title, doc.text));
                    for (text, hits) in &mut selected {
                        if hits.len() < 50
                            && text.split_whitespace().all(|term| folded.contains(term))
                        {
                            hits.push(StoredDocument {
                                document: doc.clone(),
                                version: row.version,
                            });
                        }
                    }
                }
                expected = ["zx", "the zx"]
                    .iter()
                    .map(|t| selected.remove(*t).unwrap())
                    .collect();
            }
            "L0-A" | "L0-Aprime" | "L1-A" | "L1-Aprime" => {
                cells += 1;
                assert!(cells <= 4 && live.len() == N);
                churn::emit(
                    json!({"phase":"layout_before","cell":command,"layout":db.diagnostic_layout()?}),
                );
                scan(db, &command, &expected)?;
                gap(db, &command, &templates, &mut live, &mut revision)?;
                churn::emit(
                    json!({"phase":"layout_after","cell":command,"layout":db.diagnostic_layout()?}),
                );
            }
            "maintain" => {
                assert!(!maintained);
                maintained = true;
                db.maintain()?;
            }
            "cleanup" => {
                assert_eq!(live.len(), N);
                db.commit(
                    live.iter()
                        .map(|d| Mutation::Delete {
                            key: d.document.key.clone(),
                            guard: Guard::Any,
                        })
                        .collect(),
                )?;
                assert!(db.keys(COL)?.is_empty());
                live.clear();
                model.sequence = db.sequence()?;
                model.save(state)?;
                churn::emit(json!({"phase":"clean","sequence":model.sequence,"oracle":true}));
            }
            "hold" => {
                assert!(live.is_empty());
                churn::emit(
                    json!({"phase":"kill_witness","sequence":db.sequence()?,"pool_empty":true}),
                );
                loop {
                    std::thread::park();
                }
            }
            "quit" => break,
            _ => return Err("unknown/duplicate measurement command".into()),
        }
        churn::emit(
            json!({"phase":"command_done","command":command,"elapsed_ms":at.elapsed().as_secs_f64()*1000.,"sequence":db.sequence()?,"memory":churn::memory(),"layout":db.diagnostic_layout()?}),
        );
    }
    assert!(
        live.is_empty(),
        "pool not cleaned: preserve state for explicit recovery"
    );
    Ok(())
}
