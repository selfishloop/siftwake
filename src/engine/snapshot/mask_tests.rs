use super::*;
use crate::reclaim::tests::{Temp, doc};

fn change(id: usize, version: u64, deleted: bool) -> Change {
    let key = doc(&format!("{id:04}")).key;
    if deleted {
        Change::Delete { key }
    } else {
        let mut document = doc(&format!("{id:04}"));
        document.text = format!("version{version} id{id}");
        Change::Put {
            stored: StoredDocument { document, version },
        }
    }
}
fn layout(t: &Temp, base: usize) -> Database {
    let mut db = Database::open(&t.0).unwrap();
    let groups = vec![
        (0..base).map(|n| change(n, 1, false)).collect::<Vec<_>>(),
        vec![
            change(0, 2, false),
            change(1, 2, false),
            change(2, 2, true),
            change(70, 2, false),
        ],
        vec![
            change(0, 3, false),
            change(1, 3, true),
            change(71, 3, false),
        ],
        vec![
            change(0, 4, false),
            change(5, 4, false),
            change(71, 4, true),
            change(72, 4, false),
        ],
    ];
    let mut latest = BTreeMap::new();
    for (n, group) in groups.into_iter().enumerate() {
        for c in &group {
            match c {
                Change::Put { stored } => {
                    latest.insert(stored.document.key.clone(), true);
                }
                Change::Delete { key } => {
                    latest.insert(key.clone(), false);
                }
            }
        }
        let id = db.allocate().unwrap();
        let run = write_run_prepared(
            &t.0,
            id,
            n as u64 + 1,
            group.into_iter().map(Ok),
            |_| Ok(()),
        )
        .unwrap();
        db.runs.push(run);
    }
    db.sequence = 4;
    db.count = latest.values().filter(|&&v| v).count();
    let id = db.allocate().unwrap();
    let (log, active) = manifest::new_active(&t.0, id, 4).unwrap();
    db.log = log;
    db.active = active;
    let id = db.allocate().unwrap();
    db.publish(
        id,
        &Manifest {
            version: 3,
            runs: db.descriptors(),
            active: db.active.clone(),
        },
    )
    .unwrap();
    db
}
fn install_window(db: &mut Database, start: usize, end: usize) {
    let inputs = db.runs[start..end].to_vec();
    let sources = inputs.iter().map(|r| r.source.clone()).collect();
    let id = db.allocate().unwrap();
    let seq = inputs.last().unwrap().source.sequence;
    let dir = db.dir.clone();
    let handle = std::thread::spawn(move || {
        let changes = MergeKeys::new(&inputs)?.map(|item| {
            let (n, slot) = item?;
            let index = &inputs[n].index;
            Ok(if index.deleted(slot) {
                Change::Delete {
                    key: index.key(slot)?,
                }
            } else {
                Change::Put {
                    stored: index.get(&index.key(slot)?)?.unwrap(),
                }
            })
        });
        write_run_prepared(&dir, id, seq, changes, |_| Ok(())).map(JobOutput::Replace)
    });
    assert!(
        db.install(Job {
            inputs: sources,
            output_id: id,
            handle,
            cancel: None,
            revision: false
        })
        .unwrap()
    );
}
fn verify_window(start: usize, end: usize, base: usize, shadow: bool) {
    let t = Temp::new();
    let mut db = layout(&t, base);
    let reader = db.snapshot_reader().unwrap();
    if shadow {
        let mut document = doc("0001");
        document.text = "active latest".into();
        db.commit(vec![
            Mutation::Put {
                document,
                guard: Guard::Any,
            },
            Mutation::Delete {
                key: doc("0070").key,
                guard: Guard::Any,
            },
        ])
        .unwrap();
    }
    let old = reader.acquire().unwrap();
    let seq = old.sequence();
    let queries = ["", "version", "active", "id0", "id1"].map(|text| {
        let mut q = Query::new("shared");
        q.text = text.into();
        q
    });
    let pages = queries
        .iter()
        .map(|q| old.search(q).unwrap())
        .collect::<Vec<_>>();
    let before = (0..80)
        .map(|n| old.get(&doc(&format!("{n:04}")).key).unwrap())
        .collect::<Vec<_>>();
    let retired = db.runs[start..end]
        .iter()
        .map(|r| r.source.path(&t.0, "fdb"))
        .collect::<Vec<_>>();
    let counters = reader.publication_stats();
    install_window(&mut db, start, end);
    let after = reader.publication_stats();
    assert_eq!(after.mask_full_rebuilds, counters.mask_full_rebuilds);
    assert_eq!(
        after.mask_incremental_publications,
        counters.mask_incremental_publications + 1
    );
    let current = reader.acquire().unwrap();
    assert_eq!(current.sequence(), seq);
    for (q, wanted) in queries.iter().zip(pages) {
        for got in [current.search(q).unwrap(), old.search(q).unwrap()] {
            assert_eq!(
                (got.total, got.sequence, &got.hits),
                (wanted.total, wanted.sequence, &wanted.hits)
            );
        }
    }
    assert_eq!(
        current.view().full_snapshot_masks().unwrap(),
        db.full_snapshot_masks().unwrap()
    );
    for (n, wanted) in before.iter().enumerate() {
        let key = doc(&format!("{n:04}")).key;
        assert_eq!(&current.get(&key).unwrap(), wanted);
        assert_eq!(&old.get(&key).unwrap(), wanted);
    }
    assert!(retired.iter().all(|p| p.exists()));
    old.release();
    assert!(retired.iter().all(|p| !p.exists()));
    drop(current);
    drop(reader);
    drop(db);
    Database::open(&t.0).unwrap().check_integrity().unwrap();
}
#[test]
fn replacement_masks_cover_tombstones_suffix_duplicates_shadow_and_pins() {
    verify_window(1, 3, 64, true);
}
#[test]
fn replacement_masks_allow_base_window_and_unmerged_suffix() {
    verify_window(0, 2, 8, false);
}
#[test]
fn replacement_masks_preserve_tombstone_with_no_newer_suffix() {
    verify_window(1, 4, 64, false);
}
#[test]
fn replacement_masks_fall_back_when_previous_dense_cache_is_incomplete() {
    let t = Temp::new();
    let mut db = layout(&t, 64);
    let reader = db.snapshot_reader().unwrap();
    let old = reader.acquire().unwrap();
    {
        let mut c = old.view().mask_cache.lock().unwrap();
        c.entries.clear();
        c.bytes = 0;
    }
    let before = reader.publication_stats();
    install_window(&mut db, 1, 3);
    assert_eq!(
        reader.publication_stats().mask_full_rebuilds,
        before.mask_full_rebuilds + 1
    );
    let current = reader.acquire().unwrap();
    for id in 0..80 {
        let key = doc(&format!("{id:04}")).key;
        assert_eq!(old.get(&key).unwrap(), current.get(&key).unwrap());
    }
}
