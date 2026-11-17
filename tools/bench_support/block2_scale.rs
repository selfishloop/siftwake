//! Scale measurements only. Original bodies come from the existing audited pool.
use super::churn::{self, Backend, Change, Corpus, Model, Result, Row};
use serde_json::json;
use std::time::{Duration, Instant};

pub fn count() -> Result<usize> {
    let count = std::env::var("FDB_SCALE_DOCUMENTS")?.parse()?;
    if ![10_000, 50_000, 100_000].contains(&count) {
        return Err("scale requires first 10000, 50000 or 100000 existing records".into());
    }
    Ok(count)
}

pub fn run(db: &mut impl Backend, mode: &str, state: &str, path: &str) -> Result<()> {
    let count = count()?;
    if mode == "scale-idle" {
        // Measure the engine in a fresh process before loading oracle/catalog RAM.
        std::thread::sleep(Duration::from_secs(2));
        churn::emit(
            json!({"phase":"scale_idle","documents":count,"memory":churn::memory(),"stats":db.stats()?,"disk":churn::disk(path)?,"oracle_loaded":false}),
        );
        return Ok(());
    }
    let mut corpus = Corpus::open()?;
    assert_eq!(corpus.index.entries.len(), count);
    if mode == "scale-seed" {
        let mut model = Model::default();
        let at = Instant::now();
        let mut commits = vec![];
        for first in (0..count).step_by(500) {
            let changes: Vec<_> = (first..(first + 500).min(count))
                .map(|id| Change {
                    id,
                    old: None,
                    new: Some(Row {
                        body: id,
                        revision: 0,
                        version: 0,
                    }),
                    kind: "seed".into(),
                })
                .collect();
            if std::env::var_os("FDB_SEED_PHASE_PROFILE").is_some() {
                db.query_profile_reset();
            }
            let started = Instant::now();
            let seq = db.commit(&mut corpus, &changes, true)?;
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
            commits.push(elapsed_ms);
            if std::env::var_os("FDB_SEED_PHASE_PROFILE").is_some() {
                churn::emit(
                    json!({"phase":"seed_commit_profile","first":first,"sequence":seq,"elapsed_ms":elapsed_ms,"counters":db.query_profile_take()}),
                );
            }
            churn::apply(&mut model, &changes, seq);
            if (first + 500) % 5000 == 0 {
                churn::emit(
                    json!({"phase":"scale_progress","documents":first+500,"elapsed_ms":at.elapsed().as_secs_f64()*1000.,"memory":churn::memory()}),
                );
            }
        }
        model.next = count;
        let commit_total_ms = at.elapsed().as_secs_f64() * 1000.;
        let started = Instant::now();
        let maintenance = db.maintain()?;
        let maintenance_ms = started.elapsed().as_secs_f64() * 1000.;
        let cleanup = if std::env::var_os("FDB_SEED_IDLE_CLEANUP").is_some() {
            let at = Instant::now();
            let result = db.idle_notify()?;
            json!({"elapsed_ms":at.elapsed().as_secs_f64()*1000.,"result":result})
        } else {
            serde_json::Value::Null
        };
        let ingest_ms = at.elapsed().as_secs_f64() * 1000.;
        churn::emit(
            json!({"phase":"scale_ingested","documents":count,"ingest_including_maintenance_ms":ingest_ms,"commit_total_ms":commit_total_ms,"maintenance_ms":maintenance_ms,"idle_cleanup":cleanup,"commits_ms":commits,"memory":churn::memory(),"stats":db.stats()?,"disk":churn::disk(path)?,"maintenance":maintenance,"api":"streamed","harness":"catalog and model descriptors included; no resident corpus bodies"}),
        );
        model.save(state)?;
        churn::verify_all(db, &mut corpus, &model)?;
        churn::emit(json!({"phase":"scale_verified","documents":count,"oracle":true}));
        return Ok(());
    }
    let mut model = Model::load(state)?;
    let expected_rows = match std::env::var("FDB_DIAGNOSTIC_EXPECTED_ROWS").ok() {
        Some(value) if mode == "scale-query-experiments" => {
            if db.query_profile_take().is_null() {
                return Err("partial-row override requires diagnostic counters".into());
            }
            value.parse::<usize>()?
        }
        _ => count,
    };
    assert!(expected_rows <= count);
    assert_eq!(model.rows.len(), expected_rows);
    if mode == "scale-query-cpu-blocks" {
        churn::verify_all(db, &mut corpus, &model)?;
        let ids: Vec<usize> = std::env::var("FDB_QUERY_CASES")?
            .split(',')
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?;
        let variants: Vec<String> = std::env::var("FDB_QUERY_VARIANTS")?
            .split(',')
            .map(str::to_owned)
            .collect();
        let repeats = std::env::var("FDB_CPU_BLOCK_REPEATS")
            .unwrap_or("100".into())
            .parse::<usize>()?;
        for id in ids {
            let case = &corpus.index.cases[id];
            let expected = db.search(&case.text, 0, 50)?;
            for block in 0..5 {
                for n in 0..variants.len() {
                    let variant = &variants[(n + block) % variants.len()];
                    let cpu = churn::cpu_ms();
                    let at = Instant::now();
                    let mut last = None;
                    for _ in 0..repeats {
                        last = Some(db.query_variant(&case.text, variant)?);
                    }
                    let wall_ms = at.elapsed().as_secs_f64() * 1000.;
                    let cpu_ms = cpu.zip(churn::cpu_ms()).map(|(a, b)| b - a);
                    assert_eq!(last.unwrap().hits, expected.hits);
                    churn::emit(
                        json!({"phase":"cpu_block","case_id":id,"text":case.text,"block":block,"variant":variant,"queries":repeats,"cpu_ms":cpu_ms,"wall_ms":wall_ms,"oracle":true,"includes":"query return + replacing/dropping prior owned result, loop overhead; no model oracle in CPU interval"}),
                    );
                }
            }
        }
        return Ok(());
    }
    if mode == "scale-first-oracle" {
        churn::verify_all(db, &mut corpus, &model)?;
        for (id, case) in corpus.index.cases.clone().iter().enumerate() {
            let mut c = case.clone();
            c.offset = 0;
            c.limit = 50;
            let exact = db.search(&c.text, 0, 50)?;
            churn::check_page(&mut corpus, &model, &c, &exact)?;
            for variant in [
                "cap",
                "cap-four",
                "cap-hybrid",
                "cap-hybrid-cancel",
                "cap-four-join-crc",
                "exact-four",
            ] {
                let p = db.query_variant(&c.text, variant)?;
                assert_eq!(p.hits, exact.hits);
                assert_eq!(p.sequence, exact.sequence);
                match p.total {
                    siftwake::MatchTotal::Exact(n) => assert_eq!(n, exact.total),
                    siftwake::MatchTotal::AtLeast(n) => assert!(n == 50 && exact.total >= n),
                }
            }
            churn::emit(
                json!({"phase":"first_oracle","case_id":id,"text":c.text,"exact_total":exact.total,"oracle":true,"limit":50,"offset":0}),
            );
        }
        return Ok(());
    }
    if mode == "scale-query-experiments" {
        churn::verify_all(db, &mut corpus, &model)?;
        let all = corpus.index.cases.clone();
        let selected: Vec<usize> = std::env::var("FDB_QUERY_CASES")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_else(|| vec![5, 29, 8, 4, 23]);
        let variants = std::env::var("FDB_QUERY_VARIANTS")?;
        let variants: Vec<_> = variants.split(',').collect();
        let repeats = std::env::var("FDB_SCALE_QUERY_REPEATS")
            .unwrap_or("10".into())
            .parse::<usize>()?;
        let mut expected = Vec::new();
        for &i in &selected {
            let c = &all[i];
            let p = db.search(&c.text, 0, 50)?;
            churn::check_page(&mut corpus, &model, c, &p)?;
            expected.push(p);
        }
        for repeat in 0..=repeats {
            for pos in 0..selected.len() {
                let case_pos = (pos + repeat) % selected.len();
                let id = selected[case_pos];
                let c = &all[id];
                for v in 0..variants.len() {
                    let variant = variants[(v + repeat + pos) % variants.len()];
                    db.query_profile_reset();
                    let cpu = churn::cpu_ms();
                    let at = Instant::now();
                    let p = db.query_variant(&c.text, variant)?;
                    let elapsed_ms = at.elapsed().as_secs_f64() * 1000.;
                    let cpu_ms = cpu.zip(churn::cpu_ms()).map(|(a, b)| b - a);
                    let counters = db.query_profile_take();
                    let exact = &expected[case_pos];
                    assert_eq!(p.sequence, exact.sequence);
                    assert_eq!(p.hits, exact.hits, "{variant} {}", c.text);
                    match p.total {
                        siftwake::MatchTotal::Exact(n) => assert_eq!(n, exact.total),
                        siftwake::MatchTotal::AtLeast(n) => assert!(n == 50 && exact.total >= n),
                    }
                    if repeat != 0 {
                        churn::emit(
                            json!({"phase":"query_experiment","repeat":repeat-1,"case_id":id,"text":c.text,"variant":variant,"elapsed_ms":elapsed_ms,"cpu_ms":cpu_ms,"total":p.total,"returned":p.hits.len(),"counters":counters,"oracle":true}),
                        );
                    }
                }
            }
        }
        churn::emit(
            json!({"phase":"query_experiments_finished","memory":churn::memory(),"stats":db.stats()?,"oracle":true}),
        );
        return Ok(());
    }
    if mode == "scale-align-reference" {
        if path != "data/benchmark-work/sqlite" {
            return Err("reference alignment is restricted to the fixed SQLite slot".into());
        }
        let target = Model::load(&std::env::var("FDB_SCALE_TARGET_MODEL")?)?;
        if target.rows.len() != count || target.sequence <= model.sequence {
            return Err(
                "alignment requires the same live count and a newer target sequence".into(),
            );
        }
        let mut plans = Vec::new();
        for seq in model.sequence + 1..=target.sequence {
            let mut changes: Vec<_> = target
                .rows
                .iter()
                .filter(|(_, row)| row.version == seq)
                .map(|(&id, row)| Change {
                    id,
                    old: model.rows.get(&id).cloned(),
                    new: Some(row.clone()),
                    kind: "reference_align".into(),
                })
                .collect();
            if seq == target.sequence {
                changes.extend(
                    model
                        .rows
                        .iter()
                        .filter(|(id, _)| !target.rows.contains_key(id))
                        .map(|(&id, row)| Change {
                            id,
                            old: Some(row.clone()),
                            new: None,
                            kind: "reference_align_delete".into(),
                        }),
                );
            }
            if changes.is_empty() {
                return Err("alignment would require an invented empty commit".into());
            }
            plans.push((seq, changes));
        }
        for (expected, changes) in plans {
            let seq = db.commit(&mut corpus, &changes, true)?;
            assert_eq!(seq, expected);
            churn::apply(&mut model, &changes, seq);
            model.save(state)?;
        }
        assert_eq!(model.rows, target.rows);
        assert_eq!(model.sequence, target.sequence);
        model = target;
        model.save(state)?;
        db.maintain()?;
        churn::verify_all(db, &mut corpus, &model)?;
        churn::emit(
            json!({"phase":"reference_aligned","documents":count,"sequence":model.sequence,"oracle":true,"source":"existing FDB post-recovery ledger; same payload and version","stats":db.stats()?}),
        );
        return Ok(());
    }
    if mode == "scale-target-matrix" {
        churn::verify_all(db, &mut corpus, &model)?;
        let all = corpus.index.cases.clone();
        let selected = [5, 29, 8, 4, 23];
        let repeats = std::env::var("FDB_SCALE_QUERY_REPEATS")
            .ok()
            .map(|s| s.parse::<usize>())
            .transpose()?
            .unwrap_or(10);
        for &id in &selected {
            let c = &all[id];
            let page = db.search(&c.text, c.offset, c.limit)?;
            churn::check_page(&mut corpus, &model, c, &page)?;
        }
        for repeat in 0..repeats {
            for position in 0..selected.len() {
                let case_id = selected[(position + repeat) % selected.len()];
                let case = &all[case_id];
                db.query_profile_reset();
                let at = Instant::now();
                let page = db.search(&case.text, case.offset, case.limit)?;
                let elapsed_ms = at.elapsed().as_secs_f64() * 1000.;
                let counters = db.query_profile_take();
                churn::check_page(&mut corpus, &model, case, &page)?;
                churn::emit(
                    json!({"phase":"target_query","case_id":case_id,"case":case,"repeat":repeat,"elapsed_ms":elapsed_ms,"total":page.total,"returned":page.hits.len(),"counters":counters,"oracle":true}),
                );
            }
        }
        churn::emit(
            json!({"phase":"target_matrix_finished","oracle":true,"memory":churn::memory(),"stats":db.stats()?}),
        );
        return Ok(());
    }
    if mode == "scale-matrix" {
        churn::verify_all(db, &mut corpus, &model)?;
        let repeats = std::env::var("FDB_SCALE_QUERY_REPEATS")
            .ok()
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(10);
        churn::emit(churn::matrix(
            db,
            &mut corpus,
            &model,
            repeats,
            "scale_warm",
        )?);
        churn::emit(json!({"phase":"scale_query_memory","memory":churn::memory(),"oracle":true}));
        return Ok(());
    }
    Err("unknown scale mode".into())
}
