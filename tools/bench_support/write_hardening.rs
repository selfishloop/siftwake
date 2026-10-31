//! Continuous20% churn, no corpus preparation and no per-commit oracle pauses.
use super::churn::{self, Backend, Change, Corpus, Model, Result, Row};
use serde_json::json;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub fn run(db: &mut impl Backend, state: &str, path: &str) -> Result<()> {
    run_inner(db, state, path, false)
}

pub fn idle_each_round(db: &mut impl Backend, state: &str, path: &str) -> Result<()> {
    run_inner(db, state, path, true)
}

fn run_inner(db: &mut impl Backend, state: &str, path: &str, every_round: bool) -> Result<()> {
    let mut corpus = Corpus::open()?;
    let mut model = Model::load(state)?;
    db.idle_notify()?;
    churn::verify_all(db, &mut corpus, &model)?;
    let mut retired = BTreeSet::new();
    churn::emit(
        json!({"phase":"write_churn_baseline","disk":churn::disk(path)?,"stats":db.stats()?,"liveness":db.liveness()?,"memory":churn::memory(),"policy":if every_round {"20261027: every round maintain_idle then2s; final45000retired/10000current marker oracle"} else {"historical: every3rounds sleep2s then maintenance"}}),
    );
    for round in 0..30 {
        let ids: Vec<_> = model.rows.keys().copied().collect();
        assert_eq!(ids.len(), 10000);
        let mut changes = Vec::new();
        for at in 0..1500 {
            let id = ids[(round * 683 + at) % 10000];
            let old = model.rows[&id].clone();
            let new = (at < 1000).then(|| Row {
                body: (old.body + 997 + round * 31) % 10000,
                revision: old.revision + 1,
                version: 0,
            });
            changes.push(Change {
                id,
                old: Some(old),
                new,
                kind: if at < 1000 { "update" } else { "delete" }.into(),
            });
        }
        for _ in 0..500 {
            let id = model.next;
            model.next += 1;
            changes.push(Change {
                id,
                old: None,
                new: Some(Row {
                    body: (id * 997) % 10000,
                    revision: 1,
                    version: 0,
                }),
                kind: "insert".into(),
            });
        }
        let mut commits = Vec::new();
        if every_round {
            for change in &changes {
                if let Some(old) = &change.old {
                    retired.insert((change.id, old.revision));
                }
            }
        }
        for group in changes.chunks(100) {
            let start = Instant::now();
            let seq = db.commit(&mut corpus, group, true)?;
            commits.push(start.elapsed().as_secs_f64() * 1000.);
            churn::apply(&mut model, group, seq);
        }
        let before = churn::disk(path)?;
        let live_before = db.liveness()?;
        let idle = every_round || (round + 1) % 3 == 0;
        let mut maintenance_ms = 0.;
        let mut idle_ms = 0.;
        let mut maintenance = serde_json::Value::Null;
        let mut post_maintenance = serde_json::Value::Null;
        if idle {
            if !every_round {
                let start = Instant::now();
                std::thread::sleep(Duration::from_secs(2));
                idle_ms = start.elapsed().as_secs_f64() * 1000.;
            }
            let start = Instant::now();
            maintenance = db.idle_notify()?;
            maintenance_ms = start.elapsed().as_secs_f64() * 1000.;
            post_maintenance = churn::disk(path)?;
            if every_round {
                let start = Instant::now();
                std::thread::sleep(Duration::from_secs(2));
                idle_ms = start.elapsed().as_secs_f64() * 1000.;
            }
        }
        model.save(state)?;
        churn::emit(
            json!({"phase":"write_churn_round","round":round+1,"commits_ms":commits,"before":before,"post_maintenance":post_maintenance,"after":churn::disk(path)?,"liveness_before":live_before,"liveness_after":db.liveness()?,"idle_ms":idle_ms,"maintenance_ms":maintenance_ms,"maintenance":maintenance,"stats":db.stats()?,"memory":churn::memory(),"oracle":"model ledger advanced; full oracle after continuous phase"}),
        );
    }
    churn::verify_all(db, &mut corpus, &model)?;
    if every_round {
        let start = Instant::now();
        for &(id, revision) in &retired {
            assert_eq!(
                db.search(&churn::marker(id, revision), 0, 1)?.total,
                0,
                "retired marker {id}/{revision}"
            );
        }
        for (&id, row) in &model.rows {
            let page = db.search(&churn::marker(id, row.revision), 0, 1)?;
            assert_eq!(page.total, 1, "current marker {id}/{}", row.revision);
            assert_eq!(page.hits[0].document.key, churn::key(id));
            assert_eq!(page.hits[0].version, row.version);
        }
        churn::emit(
            json!({"phase":"write_churn_marker_oracle","retired_absent":retired.len(),"current_present":model.rows.len(),"deleted_get_absent":model.dead.len(),"elapsed_ms":start.elapsed().as_secs_f64()*1000.,"oracle":true,"memory":churn::memory()}),
        );
    }
    churn::emit(churn::matrix(
        db,
        &mut corpus,
        &model,
        if every_round { 1 } else { 5 },
        "write_churn_final",
    )?);
    churn::emit(json!({"phase":"write_churn_verified","oracle":true,"power_loss_claim":false}));
    Ok(())
}
