//! Measurement only: original first50k donors, bounded runtime revisions, no corpus file.
use super::{
    churn::{self, Change, Corpus, Model, Result, Row},
    snapshot_common::{Client, Factory, View, Writer},
};
use serde_json::json;
use std::time::Instant;

fn eligible(c: &Corpus) -> Vec<Vec<usize>> {
    [(2048, 4095), (4096, 16383), (16384, 65535), (65536, 200000)]
        .into_iter()
        .map(|(lo, hi)| {
            c.index
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| (lo..=hi).contains(&e.text_bytes))
                .map(|(id, _)| id)
                .collect()
        })
        .collect()
}
fn perform<W: Writer, C: Client>(
    db: &mut W,
    c: &mut Corpus,
    m: &mut Model,
    group: &[Change],
    kind: &str,
    client: &mut C,
) -> Result<()> {
    // Prove the outgoing revision really exists before measuring its removal.
    for x in group {
        if let Some(old) = &x.old {
            assert!(db.get(x.id)?.is_some());
            if old.revision != 0 {
                assert_eq!(
                    db.search(&churn::marker(x.id, old.revision), 0, 50)?.total,
                    1
                );
            }
        }
    }
    db.reset_timing();
    db.begin_write_profile()?;
    let start = Instant::now();
    let seq = db.commit(c, group, true)?;
    let commit_ms = start.elapsed().as_secs_f64() * 1000.;
    let timing = db.take_timing();
    let trace = db.finish_write_profile();
    churn::apply(m, group, seq);
    let (view, admission) = client.acquire()?;
    assert_eq!(view.sequence(), seq);
    let changed = group.iter().find(|x| x.new.is_some()).unwrap_or(&group[0]);
    let (revision, total) = changed
        .new
        .as_ref()
        .map(|r| (r.revision, 1))
        .unwrap_or_else(|| (changed.old.as_ref().unwrap().revision, 0));
    let mut probe = fdb::Query::new("shared");
    probe.collection = Some("texts".into());
    probe.text = churn::marker(changed.id, revision);
    let visible = view.search(&probe)?.0;
    assert_eq!(visible.sequence, seq);
    assert_eq!(visible.total, total);
    let visible_ms = start.elapsed().as_secs_f64() * 1000.;
    for x in group {
        let expected = x
            .new
            .as_ref()
            .map(|r| {
                c.document(x.id, r).map(|document| fdb::StoredDocument {
                    document,
                    version: seq,
                })
            })
            .transpose()?;
        assert_eq!(view.get(x.id)?, expected);
        for (revision, total) in x
            .old
            .as_ref()
            .filter(|r| r.revision != 0)
            .map(|r| (r.revision, 0))
            .into_iter()
            .chain(x.new.as_ref().map(|r| (r.revision, 1)))
        {
            let mut q = fdb::Query::new("shared");
            q.collection = Some("texts".into());
            q.text = churn::marker(x.id, revision);
            assert_eq!(view.search(&q)?.0.total, total);
        }
    }
    assert!(view.release()?.cleanup_error.is_none());
    for x in group {
        churn::verify_change(db, c, m, x)?;
    }
    let verified_visibility_ms = start.elapsed().as_secs_f64() * 1000.;
    let lengths: Vec<_> = group
        .iter()
        .filter_map(|x| x.new.as_ref())
        .map(|r| c.index.entries[r.body].text_bytes)
        .collect();
    let old_lengths: Vec<_> = group
        .iter()
        .filter_map(|x| x.old.as_ref())
        .map(|r| c.index.entries[r.body].text_bytes)
        .collect();
    churn::emit(
        json!({"phase":"write50_commit","kind":kind,"mutations":group.len(),"sequence":seq,"commit_ms":commit_ms,"visibility_ms":visible_ms,"verified_visibility_ms":verified_visibility_ms,"admission":admission,"donor_text_bytes":lengths,"outgoing_donor_bytes":old_lengths,"timing":timing,"write_trace":trace,"memory":churn::memory(),"oracle":true}),
    );
    Ok(())
}
pub fn run<W: Writer>(db: &mut W, state: &str, mode: &str) -> Result<()> {
    assert_eq!(std::env::var("FDB_SCALE_DOCUMENTS")?, "50000");
    if mode == "write50-hold-setup" {
        let mut client = db.factory()?.open()?;
        let (view, _) = client.acquire()?;
        churn::emit(
            json!({"phase":"setup_reader_ready","sequence":view.sequence(),"purpose":"short SQLite setup read transaction prevents checkpoint; no long-snapshot benchmark"}),
        );
        loop {
            std::thread::park();
        }
    }
    let mut c = Corpus::open()?;
    let mut m = Model::load(state)?;
    if mode == "write50-dirty-part" {
        let part: usize = std::env::var("FDB_DIRTY_PART")?.parse()?;
        assert!(part < 5);
        let mut ids: Vec<_> = eligible(&c).into_iter().flatten().collect();
        ids.sort_unstable();
        ids.truncate(500);
        let changes: Vec<_> = ids[part * 100..(part + 1) * 100]
            .iter()
            .map(|&id| {
                let old = m.rows[&id].clone();
                assert_eq!(old.revision, 0);
                Change {
                    id,
                    old: Some(old),
                    new: Some(Row {
                        body: id,
                        revision: 1,
                        version: 0,
                    }),
                    kind: "dirty500_setup".into(),
                }
            })
            .collect();
        let at = Instant::now();
        let seq = db.commit(&mut c, &changes, true)?;
        churn::apply(&mut m, &changes, seq);
        m.save(state)?;
        let write_ms = at.elapsed().as_secs_f64() * 1000.;
        churn::verify_all(db, &mut c, &m)?;
        churn::emit(
            json!({"phase":"dirty_part","part":part,"mutations":100,"write_ms":write_ms,"sequence":seq,"stats":db.stats()?,"donor_text_bytes":changes.iter().map(|x|c.index.entries[x.id].text_bytes).collect::<Vec<_>>(),"oracle":true}),
        );
        return Ok(());
    }
    if mode == "write50-open" {
        std::thread::sleep(std::time::Duration::from_secs(2));
        churn::emit(json!({"phase":"write50_idle","memory":churn::memory(),"stats":db.stats()?}));
        churn::verify_all(db, &mut c, &m)?;
        churn::emit(json!({"phase":"write50_verified","oracle":true}));
        return Ok(());
    }
    db.maintain()?;
    let pools = eligible(&c);
    assert!(pools.iter().all(|p| !p.is_empty()));
    let live: Vec<_> = m
        .rows
        .keys()
        .copied()
        .filter(|id| m.rows[id].revision != 0)
        .collect();
    assert!(
        live.is_empty(),
        "stage must begin with a fresh, audited baseline"
    );
    if mode == "write50-single" {
        // Seed only the selected outgoing records with a real revision token.
        // This is setup outside latency, and makes old-text disappearance observable.
        let chosen: Vec<_> = pools
            .iter()
            .flat_map(|p| p.iter().take(256))
            .copied()
            .collect();
        for ids in chosen.chunks(20) {
            let setup: Vec<_> = ids
                .iter()
                .map(|&id| Change {
                    id,
                    old: Some(m.rows[&id].clone()),
                    new: Some(Row {
                        body: id,
                        revision: 1,
                        version: 0,
                    }),
                    kind: "setup".into(),
                })
                .collect();
            let seq = db.commit(&mut c, &setup, true)?;
            churn::apply(&mut m, &setup, seq);
        }
        db.maintain()?;
        let mut client = db.factory()?.open()?;
        for at in 0..256 {
            for kind in ["update", "delete", "insert", "mtime_delete_insert"] {
                let band = at % 4;
                let pool = &pools[band];
                let donor = if at == 3 {
                    *pool
                        .iter()
                        .max_by_key(|&&id| c.index.entries[id].text_bytes)
                        .unwrap()
                } else {
                    pool[(at / 4 + 17) % pool.len()]
                };
                let ids: Vec<_> = m
                    .rows
                    .iter()
                    .filter(|(_, r)| r.revision != 0 && r.revision < 2)
                    .map(|(&id, _)| id)
                    .collect();
                let mut group = vec![];
                if kind != "insert" {
                    let id = ids[at % ids.len()];
                    let old = m.rows[&id].clone();
                    group.push(Change {
                        id,
                        old: Some(old),
                        new: if kind == "update" {
                            Some(Row {
                                body: donor,
                                revision: 2,
                                version: 0,
                            })
                        } else {
                            None
                        },
                        kind: kind.into(),
                    });
                }
                if kind == "insert" || kind == "mtime_delete_insert" {
                    let id = m.next;
                    m.next += 1;
                    group.push(Change {
                        id,
                        old: None,
                        new: Some(Row {
                            body: donor,
                            revision: 2,
                            version: 0,
                        }),
                        kind: kind.into(),
                    });
                }
                perform(db, &mut c, &mut m, &group, kind, &mut client)?;
            }
        }
    } else if mode == "write50-dirty" {
        let ids: Vec<_> = pools.iter().flatten().take(500).copied().collect();
        for id in ids {
            let old = m.rows[&id].clone();
            let group = [Change {
                id,
                old: Some(old),
                new: Some(Row {
                    body: id,
                    revision: 1,
                    version: 0,
                }),
                kind: "dirty".into(),
            }];
            let seq = db.commit(&mut c, &group, true)?;
            churn::apply(&mut m, &group, seq);
        }
    } else {
        return Err("unknown write50 mode".into());
    }
    m.save(state)?;
    churn::verify_all(db, &mut c, &m)?;
    churn::emit(
        json!({"phase":"write50_finished","mode":mode,"memory":churn::memory(),"stats":db.stats()?,"oracle":true}),
    );
    Ok(())
}
