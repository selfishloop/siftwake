//! Fixed-slot Frunner disk and reopen measurement, with one short-snapshot reader.
use super::{
    churn::{self, Backend, Change, Corpus, Model, Result, Row},
    snapshot_common::{Client, Factory, View, Writer},
};
use fdb::{Query, ValidationStatus};
use serde_json::json;
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};

const DOCUMENTS: usize = 50_000;
const CHURN: usize = DOCUMENTS / 5;
const GROUP: usize = 100;

fn query(text: &str) -> Query {
    let mut q = Query::new("shared");
    q.collection = Some("texts".into());
    q.text = text.into();
    q.limit = 50;
    q
}

pub fn first(db: &impl Backend, start: Instant, open_ms: f64) -> Result<()> {
    let query_at = Instant::now();
    let page = db.query_variant("olduğu", "cap-hybrid")?;
    let first_ms = start.elapsed().as_secs_f64() * 1000.;
    let stats = db.stats()?;
    assert_eq!(stats["documents"], DOCUMENTS);
    assert_eq!(stats["sequence"], page.sequence);
    assert_eq!(page.validation, ValidationStatus::Verified);
    assert_eq!(page.hits.len(), 50);
    churn::emit(
        json!({"phase":"live_disk50_first","open_ms":open_ms,"first_ms":first_ms,
        "query_ms":first_ms-query_at.duration_since(start).as_secs_f64()*1000.,
        "sequence":page.sequence,"digest":crc32fast::hash(&serde_json::to_vec(&page.hits)?),
        "hits":page.hits.len(),"stats":stats,"memory":churn::memory()}),
    );
    Ok(())
}

fn changes(model: &mut Model, round: usize) -> Vec<Change> {
    let ids: Vec<_> = model.rows.keys().copied().collect();
    assert_eq!(ids.len(), DOCUMENTS);
    let mut changes = Vec::with_capacity(CHURN);
    for at in 0..CHURN * 3 / 4 {
        let id = ids[(round * 3413 + at) % DOCUMENTS];
        let old = model.rows[&id].clone();
        let new = (at < CHURN / 2).then(|| Row {
            body: (old.body + 997 + round * 31) % DOCUMENTS,
            revision: old.revision + 1,
            version: 0,
        });
        let kind = if new.is_some() { "update" } else { "delete" };
        changes.push(Change {
            id,
            old: Some(old),
            new,
            kind: kind.into(),
        });
    }
    for _ in 0..CHURN / 4 {
        let id = model.next;
        model.next += 1;
        changes.push(Change {
            id,
            old: None,
            new: Some(Row {
                body: (id * 997) % DOCUMENTS,
                revision: 1,
                version: 0,
            }),
            kind: "insert".into(),
        });
    }
    changes
}

fn marker_sample(db: &impl Backend, model: &Model) -> Result<serde_json::Value> {
    let current: Vec<_> = model
        .rows
        .iter()
        .filter(|(_, row)| row.revision > 0)
        .map(|(&id, row)| (id, row.revision))
        .collect();
    let superseded: Vec<_> = model
        .rows
        .iter()
        .flat_map(|(&id, row)| (1..row.revision).map(move |revision| (id, revision)))
        .collect();
    let deleted: Vec<_> = model
        .dead
        .iter()
        .flat_map(|(&id, row)| (1..=row.revision).map(move |revision| (id, revision)))
        .collect();
    let mut witnesses = Vec::new();
    for (kind, rows) in [
        ("current", &current),
        ("superseded", &superseded),
        ("deleted", &deleted),
    ] {
        let n = rows.len().min(8);
        for i in 0..n {
            let (id, revision) = rows[i * (rows.len() - 1) / n.saturating_sub(1).max(1)];
            let page = db.search(&churn::marker(id, revision), 0, 1)?;
            assert_eq!(
                page.total,
                usize::from(kind == "current"),
                "{kind} {id}/{revision}"
            );
            if kind == "current" {
                assert_eq!(page.hits[0].document.key, churn::key(id));
                assert_eq!(page.hits[0].version, model.rows[&id].version);
            }
            witnesses.push(json!({"kind":kind,"id":id,"revision":revision,"hits":page.total}));
        }
    }
    Ok(
        json!({"current_population":current.len(),"superseded_population":superseded.len(),
        "deleted_population":deleted.len(),"checked":witnesses}),
    )
}

fn verify(db: &impl Backend, corpus: &mut Corpus, model: &Model, markers: bool) -> Result<()> {
    let stats = db.stats()?;
    assert_eq!(stats["sequence"], model.sequence);
    assert_eq!(stats["documents"], DOCUMENTS);
    let cases = corpus.index.cases.clone();
    assert!(
        cases
            .iter()
            .all(|case| case.limit == 50 && case.offset == 0)
    );
    let terms: Vec<Vec<_>> = cases
        .iter()
        .map(|case| case.text.split_whitespace().map(churn::fold).collect())
        .collect();
    let mut totals = vec![0; cases.len()];
    let mut keys = vec![Vec::new(); cases.len()];
    for (&id, row) in &model.rows {
        let document = corpus.document(id, row)?;
        let folded = churn::fold(&format!("{}\n{}", document.title, document.text));
        for (i, words) in terms.iter().enumerate() {
            if words.iter().all(|word| folded.contains(word)) {
                totals[i] += 1;
                if keys[i].len() < 50 {
                    keys[i].push(id);
                }
            }
        }
        assert_eq!(
            db.get(id)?,
            Some(fdb::StoredDocument {
                document,
                version: row.version
            }),
            "GET/version {id}"
        );
    }
    for &id in model.dead.keys() {
        assert!(db.get(id)?.is_none(), "deleted GET {id}");
    }
    churn::emit(
        json!({"phase":"live_disk50_get_oracle","documents":DOCUMENTS,
        "deleted_keys":model.dead.len(),"sequence":model.sequence}),
    );
    for (i, case) in cases.iter().enumerate() {
        let page = db.search(&case.text, 0, 50)?;
        assert_eq!(page.sequence, model.sequence);
        assert_eq!(page.total, totals[i], "case {i}: {}", case.text);
        assert_eq!(page.hits.len(), keys[i].len());
        for (actual, &id) in page.hits.iter().zip(&keys[i]) {
            let row = &model.rows[&id];
            assert_eq!(
                *actual,
                fdb::StoredDocument {
                    document: corpus.document(id, row)?,
                    version: row.version
                }
            );
        }
        churn::emit(
            json!({"phase":"live_disk50_case","case":i,"total":page.total,
            "digest":crc32fast::hash(&serde_json::to_vec(&page.hits)?),"oracle":true}),
        );
    }
    let marker_sample = if markers {
        marker_sample(db, model)?
    } else {
        serde_json::Value::Null
    };
    churn::emit(
        json!({"phase":"live_disk50_verified","documents":DOCUMENTS,"cases":41,
        "deleted_keys":model.dead.len(),"sequence":model.sequence,"marker_sample":marker_sample,
        "oracle":true}),
    );
    Ok(())
}

fn read_loop<F: Factory>(
    factory: F,
    queries: Vec<Query>,
    stop: &AtomicBool,
    phase: &AtomicUsize,
) -> Result<()> {
    let mut client = factory.open()?;
    let mut index = 0;
    let mut previous = 0;
    while !stop.load(Ordering::Acquire) {
        let q = &queries[index % queries.len()];
        let phase = phase.load(Ordering::Acquire);
        let started = Instant::now();
        let (view, admission) = client.acquire()?;
        let page = view.first(q)?;
        let wall_ms = started.elapsed().as_secs_f64() * 1000.;
        let sequence = view.sequence();
        assert!(sequence >= previous);
        assert_eq!(page.sequence, sequence);
        assert_eq!(page.validation, ValidationStatus::Verified);
        assert!(page.hits.len() <= q.limit);
        assert!(
            page.hits
                .windows(2)
                .all(|hits| hits[0].document.key < hits[1].document.key)
        );
        for hit in page.hits.first().into_iter().chain(page.hits.last()) {
            let id: usize = hit.document.key.id.parse()?;
            assert!(hit.version <= sequence);
            assert_eq!(
                view.get(id)?.as_ref(),
                Some(hit),
                "read-view sequence {sequence}, key {id}"
            );
        }
        let release = view.release()?;
        churn::emit(
            json!({"phase":"live_disk50_read","case":index % queries.len(),
            "sequence":sequence,"load_phase":phase,"wall_ms":wall_ms,
            "admission":admission,"release":release}),
        );
        previous = sequence;
        index += 1;
    }
    Ok(())
}

fn rounds(
    db: &mut impl Backend,
    corpus: &mut Corpus,
    model: &mut Model,
    paths: (&str, &str),
    phase: &AtomicUsize,
    background: bool,
    reader_ended: impl Fn() -> bool,
) -> Result<()> {
    let (state, path) = paths;
    let first: usize = std::env::var("FDB_LIVE_DISK_FIRST")?.parse()?;
    let end: usize = std::env::var("FDB_LIVE_DISK_END")?.parse()?;
    assert!(first < end && end <= 30);
    if first == 0 {
        churn::emit(
            json!({"phase":"live_disk50_baseline","disk":churn::disk(path)?,
            "stats":db.stats()?,"liveness":db.liveness()?}),
        );
    }
    for round in first..end {
        let changes = changes(model, round);
        let mut commits = Vec::with_capacity(CHURN / GROUP);
        phase.store((round + 1) * 10 + 1, Ordering::Release);
        let start = Instant::now();
        let (groups, remainder) = changes.as_chunks::<GROUP>();
        assert!(remainder.is_empty());
        for chunk in groups {
            if reader_ended() {
                return Err("snapshot reader ended before writer".into());
            }
            let at = Instant::now();
            let sequence = db.commit(corpus, chunk, true)?;
            commits.push(json!({"ms":at.elapsed().as_secs_f64()*1000.,"sequence":sequence}));
            churn::apply(model, chunk, sequence);
        }
        let write_ms = start.elapsed().as_secs_f64() * 1000.;
        let before = churn::disk(path)?;
        phase.store((round + 1) * 10 + 2, Ordering::Release);
        let at = Instant::now();
        let maintenance = if background {
            let until = at + Duration::from_secs(2);
            let mut advanced = 0;
            while let Some(remaining) = until.checked_duration_since(Instant::now()) {
                advanced += usize::from(db.poll_idle()?);
                std::thread::sleep(remaining.min(Duration::from_millis(5)));
            }
            json!({ "progress_calls": advanced })
        } else {
            db.idle_notify()?
        };
        let maintenance_ms = at.elapsed().as_secs_f64() * 1000.;
        phase.store((round + 1) * 10 + 3, Ordering::Release);
        let post_maintenance = churn::disk(path)?;
        if !background {
            std::thread::sleep(Duration::from_secs(2));
        }
        phase.store(0, Ordering::Release);
        model.save(state)?;
        churn::emit(
            json!({"phase":"live_disk50_round","round":round+1,"commits":commits,
            "write_ms":write_ms,"maintenance_ms":maintenance_ms,"maintenance":maintenance,
            "before":before,"post_maintenance":post_maintenance,"after":churn::disk(path)?,
            "stats":db.stats()?,"liveness":db.liveness()?,"memory":churn::memory()}),
        );
    }
    Ok(())
}

struct StopReader<'a>(&'a AtomicBool);
impl Drop for StopReader<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub fn run(db: &mut impl Writer, mode: &str, state: &str, path: &str) -> Result<()> {
    let mut corpus = Corpus::open()?;
    let mut model = Model::load(state)?;
    assert_eq!(corpus.index.entries.len(), DOCUMENTS);
    assert_eq!(corpus.index.cases.len(), 41);
    assert_eq!(model.rows.len(), DOCUMENTS);
    assert_eq!(db.stats()?["sequence"], model.sequence);
    match mode {
        "live-disk50-churn" | "live-disk50-default-churn" => {
            let background = mode == "live-disk50-default-churn";
            let factory = db.factory()?;
            let queries = corpus
                .index
                .cases
                .iter()
                .map(|case| query(&case.text))
                .collect();
            let stop = AtomicBool::new(false);
            let phase = AtomicUsize::new(0);
            std::thread::scope(|scope| -> Result<()> {
                let reader = scope.spawn(|| read_loop(factory, queries, &stop, &phase));
                let guard = StopReader(&stop);
                let written = rounds(
                    db,
                    &mut corpus,
                    &mut model,
                    (state, path),
                    &phase,
                    background,
                    || reader.is_finished(),
                );
                drop(guard);
                reader.join().map_err(|_| "snapshot reader panicked")??;
                written
            })?;
        }
        "live-disk50-verify" => verify(db, &mut corpus, &model, true)?,
        "live-disk50-kill" => {
            let group = churn::plan(&mut model, "mixed", 4, 4);
            let sequence = db.commit(&mut corpus, &group, true)?;
            churn::apply(&mut model, &group, sequence);
            model.save(state)?;
            for change in &group {
                churn::verify_change(db, &mut corpus, &model, change)?;
            }
            churn::emit(json!({"phase":"live_disk50_kill_ready","sequence":sequence,
                "committed":group.len(),"power_loss_claim":false}));
            loop {
                std::thread::park();
            }
        }
        _ => return Err("unknown 50k live disk mode".into()),
    }
    Ok(())
}
