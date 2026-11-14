use super::*;
use crate::reclaim::tests::{Temp, doc};
use crate::{MatchTotal, ValidationStatus};

const KEYS: usize = 64;
const BASE_KEYS: usize = 48;
const SEEDS: u64 = 16;
const GROUPS: usize = 24;

#[derive(Clone, Copy, Debug)]
enum Edit {
    Put(usize),
    Delete(usize),
}

#[derive(Clone)]
struct Model {
    sequence: u64,
    rows: [Option<StoredDocument>; KEYS],
}

impl Model {
    fn new() -> Self {
        Self {
            sequence: 0,
            rows: std::array::from_fn(|_| None),
        }
    }

    fn commit(&mut self, db: &mut Database, edits: &[Edit], history: &mut Vec<Vec<Edit>>) {
        let sequence = self.sequence + 1;
        history.push(edits.to_vec());
        let mutations = edits.iter().map(|&edit| {
            Ok(match edit {
                Edit::Put(id) => {
                    let mut document = doc(&format!("{id:04}"));
                    let variant = if sequence.is_multiple_of(2) {
                        "middle"
                    } else {
                        "outside"
                    };
                    document.text = format!(
                        "common {variant} identity{id:04} {} {}",
                        marker(id, sequence),
                        if sequence == 1 {
                            "payload ".repeat(300)
                        } else {
                            String::new()
                        }
                    );
                    self.rows[id] = Some(StoredDocument {
                        document: document.clone(),
                        version: sequence,
                    });
                    Mutation::Put {
                        document,
                        guard: Guard::Any,
                    }
                }
                Edit::Delete(id) => {
                    self.rows[id] = None;
                    Mutation::Delete {
                        key: doc(&format!("{id:04}")).key,
                        guard: Guard::Any,
                    }
                }
            })
        });
        let actual = db
            .commit_streamed(mutations)
            .unwrap_or_else(|error| panic!("commit {sequence}, history {history:?}: {error:?}"));
        assert_eq!(actual, sequence, "{history:?}");
        self.sequence = sequence;
    }

    fn check(&self, view: View<'_>, probes: &[String], context: &str) {
        assert_eq!(view.sequence(), self.sequence, "{context}");
        assert_eq!(
            view.document_count(),
            self.rows.iter().flatten().count(),
            "{context}: document count"
        );
        for (id, expected) in self.rows.iter().enumerate() {
            let key = doc(&format!("{id:04}")).key;
            assert_eq!(
                view.get(&key)
                    .unwrap_or_else(|e| panic!("{context}: GET {id}: {e:?}"))
                    .as_ref(),
                expected.as_ref(),
                "{context}: GET {id}"
            );
        }
        for (text, offset, limit) in probes.iter().map(|probe| (probe.as_str(), 0, 5)).chain([
            ("common", 0, 5),
            ("common middle", 2, 6),
            ("oracleabsent", 0, 5),
        ]) {
            let mut q = Query::new("shared");
            q.collection = Some("texts".into());
            q.text = text.into();
            q.offset = offset;
            q.limit = limit;
            let matching: Vec<_> = self
                .rows
                .iter()
                .flatten()
                .filter(|row| {
                    text.split_whitespace()
                        .all(|word| row.document.text.contains(word))
                })
                .collect();
            let expected: Vec<_> = matching.iter().copied().skip(offset).take(limit).collect();
            let exact = view
                .exact(&q)
                .unwrap_or_else(|e| panic!("{context}: exact {text}: {e:?}"));
            assert_eq!(
                (exact.sequence, exact.total),
                (self.sequence, matching.len()),
                "{context}: exact total {text}"
            );
            assert_eq!(
                exact.hits.iter().collect::<Vec<_>>(),
                expected,
                "{context}: exact hits {text}"
            );
            if offset == 0 {
                for hybrid in [false, true] {
                    let page = view.first(&q, hybrid).unwrap_or_else(|e| {
                        panic!("{context}: capped {text}, hybrid={hybrid}: {e:?}")
                    });
                    assert_eq!(
                        page.sequence, self.sequence,
                        "{context}: capped sequence {text}"
                    );
                    assert_eq!(
                        page.validation,
                        ValidationStatus::Verified,
                        "{context}: capped validation {text}"
                    );
                    assert_eq!(
                        page.hits.iter().collect::<Vec<_>>(),
                        expected,
                        "{context}: capped hits {text}, hybrid={hybrid}"
                    );
                    let valid_total = match page.total {
                        MatchTotal::Exact(n) => n == matching.len(),
                        MatchTotal::AtLeast(n) => n == limit && matching.len() >= n,
                    };
                    assert!(
                        valid_total,
                        "{context}: capped total {text}, hybrid={hybrid}: {:?}",
                        page.total
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum View<'a> {
    Writer(&'a Database),
    Reader(&'a Snapshot),
}

impl View<'_> {
    fn sequence(self) -> u64 {
        match self {
            Self::Writer(db) => db.sequence,
            Self::Reader(snapshot) => snapshot.sequence(),
        }
    }

    fn document_count(self) -> usize {
        match self {
            Self::Writer(db) => db.count,
            Self::Reader(snapshot) => snapshot.document_count(),
        }
    }

    fn get(self, key: &Key) -> Result<Option<StoredDocument>> {
        match self {
            Self::Writer(db) => db.get(key),
            Self::Reader(snapshot) => snapshot.get(key),
        }
    }

    fn exact(self, q: &Query) -> Result<SearchPage> {
        match self {
            Self::Writer(db) => db.search(q),
            Self::Reader(snapshot) => snapshot.search(q),
        }
    }

    fn first(self, q: &Query, hybrid: bool) -> Result<crate::FirstPage> {
        match (self, hybrid) {
            (Self::Writer(db), false) => db.search_first(q),
            (Self::Writer(db), true) => db.search_first_hybrid(q, 2),
            (Self::Reader(snapshot), false) => snapshot.search_first(q),
            (Self::Reader(snapshot), true) => snapshot.search_first_hybrid(q, 2),
        }
    }
}

fn marker(id: usize, sequence: u64) -> String {
    format!("marker{id:04}version{sequence:04}")
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn random_edits(state: &mut u64) -> Vec<Edit> {
    let mut keys = BTreeSet::new();
    let mut edits = Vec::new();
    for _ in 0..(1 + next(state) % 4) {
        let key = (next(state) % KEYS as u64) as usize;
        if keys.insert(key) {
            edits.push(if next(state).is_multiple_of(4) {
                Edit::Delete(key)
            } else {
                Edit::Put(key)
            });
        }
    }
    edits
}

fn assert_live_files(db: &Database, context: &str) {
    assert!(
        manifest::path(&db.dir, db.generation, "ready").is_file(),
        "{context}"
    );
    assert!(
        manifest::path(&db.dir, db.active.id, "fdb").is_file(),
        "{context}"
    );
    for run in &db.runs {
        assert!(run.source.path(&db.dir, "fdb").is_file(), "{context}");
        assert!(run.source.path(&db.dir, "idx").is_file(), "{context}");
    }
}

#[test]
fn seeded_model_checks_writer_and_snapshots_across_publication_and_gc() {
    for seed in 1..=SEEDS {
        let temp = Temp::new();
        let mut db = Database::open(&temp.0).unwrap();
        db.set_streamed_wal_layer(true).unwrap();
        // Isolate explicitly witnessed events from automatic revision installs.
        db.set_revision_reclamation(false);
        let mut state = seed;
        let mut history = Vec::new();
        let mut model = Model::new();
        let mut probes = vec![
            marker(1, 1),
            marker(1, 2),
            "identity0029".into(),
            marker(0, 3),
            "identity0028".into(),
        ];
        model.commit(
            &mut db,
            &(0..BASE_KEYS).map(Edit::Put).collect::<Vec<_>>(),
            &mut history,
        );
        db.flush().unwrap();
        let reader = db.snapshot_reader().unwrap();
        let pinned = reader.acquire().unwrap();
        let pinned_model = model.clone();
        let original = db.runs[0].source.clone();
        let old_source = original.path(&db.dir, "fdb");
        let old_wal = manifest::path(&db.dir, db.active.id, "fdb");
        let check = |event,
                     db: &Database,
                     model: &Model,
                     history: &[Vec<Edit>],
                     probes: &[String],
                     old: Option<&Snapshot>| {
            let context = format!(
                "seed {seed}, {event}, generation {}, history {history:?}",
                db.generation
            );
            model.check(View::Writer(db), probes, &format!("{context}, writer"));
            let current = reader.acquire().unwrap();
            assert_eq!(current.generation(), db.generation, "{context}");
            model.check(
                View::Reader(&current),
                probes,
                &format!("{context}, current"),
            );
            if let Some(old) = old {
                pinned_model.check(View::Reader(old), probes, &format!("{context}, pinned"));
            }
            assert_live_files(db, &context);
        };
        check(
            "warm before writes",
            &db,
            &model,
            &history,
            &probes,
            Some(&pinned),
        );

        // The base stays partly live and has >64 KiB of superseded payload.
        let edits: Vec<_> = (0..28)
            .map(Edit::Put)
            .chain((28..32).map(Edit::Delete))
            .collect();
        model.commit(&mut db, &edits, &mut history);
        let block = test_jobs::pause(&db.dir);
        assert!(
            db.start_light_flush().unwrap(),
            "seed {seed}: prefix not scheduled"
        );
        block.wait_entered();
        model.commit(
            &mut db,
            &[Edit::Put(0), Edit::Put(28), Edit::Put(60)],
            &mut history,
        );
        let tail = reader.acquire().unwrap();
        let tail_model = model.clone();
        check(
            "before prefix",
            &db,
            &model,
            &history,
            &probes,
            Some(&pinned),
        );
        let before = db.generation;
        drop(block);
        assert!(
            db.light_flush_ready(true).unwrap(),
            "seed {seed}: prefix not installed"
        );
        assert!(
            db.generation > before && db.runs.len() == 2 && db.runs[0].source == original,
            "seed {seed}: prefix publication"
        );
        assert_ne!(
            old_wal,
            manifest::path(&db.dir, db.active.id, "fdb"),
            "seed {seed}"
        );
        assert_eq!(db.shadow.len(), 3, "seed {seed}: retained tail");
        check(
            "after prefix",
            &db,
            &model,
            &history,
            &probes,
            Some(&pinned),
        );
        tail_model.check(
            View::Reader(&tail),
            &probes,
            &format!("seed {seed}, pinned original WAL after prefix"),
        );

        db.set_revision_reclamation(true);
        check(
            "before reclaim rewrite",
            &db,
            &model,
            &history,
            &probes,
            Some(&pinned),
        );
        let before = db.generation;
        let suffix = db.runs[1].source.clone();
        assert!(
            db.schedule_background_revision().unwrap(),
            "seed {seed}: reclaim not scheduled"
        );
        assert!(
            db.finish_background_revision(true).unwrap(),
            "seed {seed}: reclaim not installed"
        );
        assert!(
            db.generation > before && db.runs.len() == 2 && db.runs[1].source == suffix,
            "seed {seed}: reclaim publication"
        );
        assert_ne!(db.runs[0].source.id, original.id, "seed {seed}");
        assert_eq!(
            db.runs[0].source.keys,
            BASE_KEYS - 32,
            "seed {seed}: expected rewrite"
        );
        check(
            "after reclaim rewrite",
            &db,
            &model,
            &history,
            &probes,
            Some(&pinned),
        );
        db.set_revision_reclamation(false);

        check(
            "before GC with pins",
            &db,
            &model,
            &history,
            &probes,
            Some(&pinned),
        );
        let before = db.generation;
        db.reclaim_obsolete().unwrap();
        assert_eq!(db.generation, before, "seed {seed}: GC changed manifest");
        assert!(
            old_source.is_file() && old_wal.is_file(),
            "seed {seed}: GC ignored a pin"
        );
        check(
            "after GC with pins",
            &db,
            &model,
            &history,
            &probes,
            Some(&pinned),
        );
        tail_model.check(
            View::Reader(&tail),
            &probes,
            &format!("seed {seed}, pinned tail during GC"),
        );
        assert!(pinned.release().cleanup_error.is_none(), "seed {seed}");
        assert!(
            old_source.is_file() && old_wal.is_file(),
            "seed {seed}: tail still pins both"
        );
        check(
            "before last pin release",
            &db,
            &model,
            &history,
            &probes,
            None,
        );
        tail_model.check(
            View::Reader(&tail),
            &probes,
            &format!("seed {seed}, before last pin release"),
        );
        let released = tail.release();
        assert!(
            released.cleanup_error.is_none() && released.reclaimed_files > 0,
            "seed {seed}: {released:?}"
        );
        assert!(
            !old_source.exists() && !old_wal.exists(),
            "seed {seed}: last release did not collect files"
        );
        assert_eq!(
            db.generation, before,
            "seed {seed}: last release changed manifest"
        );
        check(
            "after last pin release",
            &db,
            &model,
            &history,
            &probes,
            None,
        );

        let merge_pin = reader.acquire().unwrap();
        check("before merge", &db, &model, &history, &probes, None);
        let previous = db.descriptors();
        let before = db.generation;
        assert!(
            db.start_normal_compaction(false).unwrap(),
            "seed {seed}: merge not scheduled"
        );
        assert!(
            db.wait_for_merge().unwrap(),
            "seed {seed}: merge not installed"
        );
        assert!(
            db.generation > before && db.runs.len() < previous.len(),
            "seed {seed}: merge publication"
        );
        assert!(
            db.runs.iter().all(|run| !previous.contains(&run.source)),
            "seed {seed}: expected replacement run"
        );
        check("after merge", &db, &model, &history, &probes, None);
        model.check(
            View::Reader(&merge_pin),
            &probes,
            &format!("seed {seed}: pre-merge pin after merge"),
        );
        assert!(merge_pin.release().cleanup_error.is_none(), "seed {seed}");

        let random_pin = reader.acquire().unwrap();
        let random_pin_model = model.clone();
        for round in 0..GROUPS {
            let edits = random_edits(&mut state);
            let id = match edits[0] {
                Edit::Put(id) | Edit::Delete(id) => id,
            };
            probes.truncate(5);
            probes.push(format!("identity{id:04}"));
            if let Some(old) = &model.rows[id] {
                probes.push(marker(id, old.version));
            }
            model.commit(&mut db, &edits, &mut history);
            probes.push(marker(id, model.sequence));
            check("random commit", &db, &model, &history, &probes, None);
            if round == GROUPS / 2 || round + 1 == GROUPS {
                let context = format!("seed {seed}, round {round}, history {history:?}");
                random_pin_model.check(
                    View::Reader(&random_pin),
                    &probes,
                    &format!("{context}, before prefix"),
                );
                let before = db.generation;
                let run_count = db.runs.len();
                assert!(
                    db.start_light_flush().unwrap(),
                    "seed {seed}: random prefix not scheduled"
                );
                assert!(
                    db.light_flush_ready(true).unwrap(),
                    "seed {seed}: random prefix not installed"
                );
                assert!(
                    db.generation > before && db.runs.len() == run_count + 1,
                    "seed {seed}: random prefix publication"
                );
                check("random prefix", &db, &model, &history, &probes, None);
                random_pin_model.check(
                    View::Reader(&random_pin),
                    &probes,
                    &format!("{context}, after prefix"),
                );
            }
        }
        assert!(random_pin.release().cleanup_error.is_none(), "seed {seed}");
        drop((reader, db));
        let reopened = Database::open(&temp.0).unwrap();
        reopened.check_integrity().unwrap();
        model.check(
            View::Writer(&reopened),
            &probes,
            &format!("seed {seed}: strict reopen, history {history:?}"),
        );
    }
}
