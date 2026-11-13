use super::*;
use crate::reclaim::tests::{Temp, doc};
use std::io::{Read, Seek, SeekFrom};

fn layout(dir: &Path) -> Vec<Source> {
    let sources = (1..=3)
        .map(|id| {
            let change = Change::Put {
                stored: StoredDocument {
                    document: doc("shared-key"),
                    version: id,
                },
            };
            write_run_prepared(dir, id, id, std::iter::once(Ok(change)), |_| Ok(()))
                .unwrap()
                .source
        })
        .collect::<Vec<_>>();
    let (_, active) = manifest::new_active(dir, 4, 3).unwrap();
    manifest::publish(
        dir,
        5,
        &Manifest {
            version: 3,
            runs: sources.clone(),
            active,
        },
    )
    .unwrap();
    sources
}

fn read_only_outcomes(dir: &Path, sources: &[Source]) -> Vec<Result<Run>> {
    load_runs_parallel(sources, 2, |source| {
        Ok(open_source_policy(dir, source, false, false))
    })
    .unwrap()
}

#[test]
fn parallel_strict_open_repairs_only_derived_cache_and_preserves_run_order() {
    let temp = Temp::new();
    let sources = layout(&temp.0);
    let missing = sources[0].path(&temp.0, "records04.idx");
    let corrupt = sources[1].path(&temp.0, "idx");
    fs::remove_file(&missing).unwrap();
    let mut bad_index = fs::read(&corrupt).unwrap();
    bad_index[0] ^= 1;
    fs::write(&corrupt, &bad_index).unwrap();

    let outcomes = read_only_outcomes(&temp.0, &sources);
    assert!(outcomes[0].is_err() && outcomes[1].is_err() && outcomes[2].is_ok());
    assert!(!missing.exists());
    assert_eq!(fs::read(&corrupt).unwrap(), bad_index);

    let runs = finish_strict_runs(&temp.0, sources.clone(), outcomes).unwrap();
    assert!(runs.iter().map(|run| &run.source).eq(&sources));
    assert!(missing.is_file());
    assert_ne!(fs::read(&corrupt).unwrap(), bad_index);
    drop(runs);

    let db = Database::open(&temp.0).unwrap();
    assert_eq!(db.stats().unwrap().documents, 1);
    assert_eq!(db.get(&doc("shared-key").key).unwrap().unwrap().version, 3);
    drop(db);

    let source = sources[1].path(&temp.0, "fdb");
    let index_before = fs::read(&corrupt).unwrap();
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(source)
        .unwrap();
    file.seek(SeekFrom::End(-1)).unwrap();
    let mut last = [0];
    file.read_exact(&mut last).unwrap();
    file.seek(SeekFrom::End(-1)).unwrap();
    file.write_all(&[last[0] ^ 1]).unwrap();
    drop(file);

    let outcomes = read_only_outcomes(&temp.0, &sources);
    assert!(matches!(outcomes[1], Err(Error::Corrupt(_))));
    assert!(matches!(
        finish_strict_runs(&temp.0, sources, outcomes),
        Err(Error::Corrupt(_))
    ));
    assert_eq!(fs::read(&corrupt).unwrap(), index_before);
    assert!(matches!(Database::open(&temp.0), Err(Error::Corrupt(_))));
}

#[test]
fn strict_open_uses_serial_path_when_no_worker_is_available() {
    let _exclusive = first::EXCLUSIVE_PERMIT_TEST.lock().unwrap();
    let temp = Temp::new();
    layout(&temp.0);
    let mut permits = Vec::new();
    let mut held = 0;
    while held < 4 {
        let acquired = first::Permits::acquire(4 - held).unwrap();
        held += acquired.0;
        permits.push(acquired);
    }
    let dir = &temp.0;
    let result = std::thread::scope(|scope| {
        let (tx, rx) = std::sync::mpsc::channel();
        scope.spawn(move || {
            tx.send(Database::open(dir).and_then(|db| db.stats()))
                .unwrap();
        });
        let result = rx.recv_timeout(std::time::Duration::from_secs(15));
        drop(permits);
        result
    });
    assert_eq!(result.unwrap().unwrap().documents, 1);
}
