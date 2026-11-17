#[allow(dead_code)]
#[path = "../../bench_support/churn.rs"]
mod churn;
mod tantivy_arm;

use churn::{Case, Corpus, Result, Row, cpu_ms, emit, key, memory};
use serde_json::json;
use siftwake::{Database, Document, Guard, MatchTotal, Mutation, Query, Snapshot};
use std::{
    env, fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use tantivy::{IndexReader, Searcher};
use tantivy_arm::Fields;

const N: usize = 50_000;
const BATCH: usize = 500;
const FIRST_QUERY: &str = "olduğu";
const LIMIT: usize = 50;
const MAX_SLOT_BYTES: u64 = 1_500 * 1024 * 1024;

#[derive(PartialEq, Debug)]
struct Page {
    total: usize,
    hits: Vec<Document>,
}

#[derive(Clone, Copy, PartialEq)]
enum Arm {
    Fdb,
    Tantivy,
}

impl Arm {
    fn name(self) -> &'static str {
        match self {
            Self::Fdb => "fdb",
            Self::Tantivy => "tantivy",
        }
    }
}

enum ReadView {
    Fdb(Snapshot),
    Tantivy(Fields, Searcher),
}

fn query(text: &str, offset: usize, limit: usize) -> Query {
    let mut query = Query::new("shared");
    query.collection = Some("texts".to_owned());
    query.text = text.to_owned();
    query.offset = offset;
    query.limit = limit;
    query
}

impl ReadView {
    fn open(arm: Arm, path: &Path) -> Result<(Self, Option<Database>)> {
        match arm {
            Arm::Fdb => {
                let mut db = Database::open(path)?;
                let view = db.snapshot_reader()?.acquire()?;
                Ok((Self::Fdb(view), Some(db)))
            }
            Arm::Tantivy => {
                let (_, fields, reader) = tantivy_arm::open_index(path)?;
                Ok((Self::Tantivy(fields, reader.searcher()), None))
            }
        }
    }

    fn search(&self, text: &str, offset: usize, limit: usize, capped: bool) -> Result<Page> {
        match self {
            Self::Fdb(view) => {
                let query = query(text, offset, limit);
                let (total, hits) = if capped {
                    let page = view.search_first_hybrid(&query, 200)?;
                    let total = match page.total {
                        MatchTotal::Exact(n) | MatchTotal::AtLeast(n) => n.min(offset + limit),
                    };
                    (total, page.hits)
                } else {
                    let page = view.search(&query)?;
                    (page.total, page.hits)
                };
                Ok(Page {
                    total,
                    hits: hits.into_iter().map(|doc| doc.document).collect(),
                })
            }
            Self::Tantivy(fields, searcher) => {
                tantivy_arm::search(searcher, *fields, text, offset, limit, capped)
            }
        }
    }

    fn segments(&self) -> Option<usize> {
        match self {
            Self::Fdb(_) => None,
            Self::Tantivy(_, searcher) => Some(searcher.segment_readers().len()),
        }
    }
}

fn corpus() -> Result<Corpus> {
    let corpus = Corpus::open()?;
    assert_eq!(corpus.index.entries.len(), N);
    assert_eq!(corpus.index.cases.len(), 41);
    assert_eq!(corpus.index.cases[0].text, FIRST_QUERY);
    Ok(corpus)
}

fn original(corpus: &mut Corpus, id: usize) -> Result<Document> {
    corpus.document(
        id,
        &Row {
            body: id,
            revision: 0,
            version: 0,
        },
    )
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

fn size(path: &Path) -> Result<u64> {
    let mut bytes = 0;
    for file in fs::read_dir(path)? {
        let file = file?;
        let info = file.metadata()?;
        assert!(info.is_file(), "unexpected subdirectory");
        bytes += info.len();
    }
    assert!(bytes <= MAX_SLOT_BYTES, "benchmark slot exceeded 1500 MiB");
    Ok(bytes)
}

fn seed(arm: Arm, path: &Path) -> Result<()> {
    assert!(
        path.is_dir() && fs::read_dir(path)?.next().is_none(),
        "seed needs an empty owned slot"
    );
    let mut corpus = corpus()?;
    let mut commits = Vec::new();
    let cpu = cpu_ms().unwrap();
    let start = Instant::now();
    let (write_ms, segments) = match arm {
        Arm::Fdb => {
            let mut db = Database::open(path)?;
            for first in (0..N).step_by(BATCH) {
                let at = Instant::now();
                db.commit_streamed((first..first + BATCH).map(|id| {
                    original(&mut corpus, id)
                        .map(|document| Mutation::Put {
                            document,
                            guard: Guard::Missing,
                        })
                        .map_err(|e| siftwake::Error::Invalid(e.to_string()))
                }))?;
                commits.push(ms(at));
                if first % 5000 == 0 {
                    size(path)?;
                }
            }
            let write_ms = ms(start);
            let stats = db.maintain_idle()?;
            assert_eq!(stats.documents, N);
            (write_ms, stats.segments)
        }
        Arm::Tantivy => {
            let (index, fields) = tantivy_arm::create(path)?;
            let mut writer = index.writer_with_num_threads(1, tantivy_arm::WRITER_BYTES)?;
            for first in (0..N).step_by(BATCH) {
                let at = Instant::now();
                for id in first..first + BATCH {
                    let input = tantivy_arm::encode(fields, id, original(&mut corpus, id)?)?;
                    if let Err(error) = writer.add_document(input) {
                        let worker = writer.prepare_commit().err();
                        return Err(
                            format!("Tantivy add at key {id}: {error}; worker {worker:?}").into(),
                        );
                    }
                }
                writer.commit()?;
                commits.push(ms(at));
                if first % 5000 == 0 {
                    size(path)?;
                }
            }
            let write_ms = ms(start);
            writer.wait_merging_threads()?;
            (write_ms, index.searchable_segment_ids()?.len())
        }
    };
    emit(
        json!({"mode":"seed","arm":arm.name(),"documents":N,"segments":segments,
        "write_ms":write_ms,"total_ms":ms(start),"cpu_ms":cpu_ms().unwrap()-cpu,
        "commits_ms":commits,"bytes":size(path)?,"memory":memory()}),
    );
    Ok(())
}

fn checked(arm: Arm, path: &Path) -> Result<()> {
    let mut corpus = corpus()?;
    match arm {
        Arm::Fdb => {
            let db = Database::open(path)?;
            let stats = db.stats()?;
            assert_eq!(stats.documents, N);
            assert_eq!(stats.sequence, (N / BATCH) as u64);
            for id in 0..N {
                let actual = db.get(&key(id))?.ok_or("missing FDB doc")?;
                assert_eq!(actual.version, (id / BATCH + 1) as u64);
                assert!(
                    actual.document == original(&mut corpus, id)?,
                    "unexpected FDB payload {id}"
                );
            }
        }
        Arm::Tantivy => {
            let (index, fields, reader) = tantivy_arm::open_index(path)?;
            assert!(index.validate_checksum()?.is_empty());
            let searcher = reader.searcher();
            assert_eq!(searcher.num_docs(), N as u64);
            tantivy_arm::verify_all(&searcher, fields, |id| original(&mut corpus, id))?;
        }
    }
    emit(json!({"mode":"verify_all","arm":arm.name(),"documents":N,"oracle":true}));
    Ok(())
}

fn expected(corpus: &mut Corpus, case: &Case, capped: bool) -> Result<Page> {
    let (offset, limit) = if capped {
        (0, LIMIT)
    } else {
        (case.offset, case.limit)
    };
    let ids: Vec<_> = corpus
        .index
        .entries
        .iter()
        .enumerate()
        .filter(|(_, row)| row.mask & (1 << case.base) != 0)
        .map(|(id, _)| id)
        .collect();
    let hits = ids
        .iter()
        .skip(offset)
        .take(limit)
        .map(|id| original(corpus, *id))
        .collect::<Result<_>>()?;
    Ok(Page {
        total: if capped {
            ids.len().min(LIMIT)
        } else {
            ids.len()
        },
        hits,
    })
}

fn open_probe(arm: Arm, path: &Path) -> Result<()> {
    let cpu = cpu_ms().unwrap();
    let start = Instant::now();
    let (reader, _db) = ReadView::open(arm, path)?;
    let open_ms = ms(start);
    let page = reader.search(FIRST_QUERY, 0, LIMIT, true)?;
    let first_ms = ms(start);
    let cpu_ms = cpu_ms().unwrap() - cpu;
    let rss = memory();
    let mut corpus = corpus()?;
    let case = corpus.index.cases[0].clone();
    let expected = expected(&mut corpus, &case, true)?;
    assert!(page == expected, "opening probe mismatch");
    emit(
        json!({"mode":"open","arm":arm.name(),"open_ms":open_ms,"first_ms":first_ms,
        "cpu_ms":cpu_ms,"memory":rss,"hits":page.hits.len(),"segments":reader.segments()}),
    );
    Ok(())
}

fn idle(arm: Arm, path: &Path) -> Result<()> {
    let (reader, _db) = ReadView::open(arm, path)?;
    thread::sleep(Duration::from_secs(2));
    emit(json!({"mode":"idle","arm":arm.name(),"memory":memory(),"segments":reader.segments()}));
    drop(reader);
    Ok(())
}

fn matrix(arm: Arm, path: &Path, block: usize, samples: usize) -> Result<()> {
    let (reader, _db) = ReadView::open(arm, path)?;
    let mut corpus = corpus()?;
    let cases = corpus.index.cases.clone();
    let mut oracle = Vec::new();
    for capped in [false, true] {
        oracle.push(
            cases
                .iter()
                .map(|case| expected(&mut corpus, case, capped))
                .collect::<Result<Vec<_>>>()?,
        );
    }
    for case_id in 0..cases.len() {
        for capped in [false, true] {
            let case = &cases[case_id];
            let (offset, limit) = if capped {
                (0, LIMIT)
            } else {
                (case.offset, case.limit)
            };
            assert!(
                reader.search(&case.text, offset, limit, capped)?
                    == oracle[capped as usize][case_id],
                "warmup oracle {} {capped}",
                case.text
            );
        }
    }
    for round in 0..samples {
        let mut calls = Vec::with_capacity(cases.len() * 2);
        let cpu_start = cpu_ms().unwrap();
        let round_started = Instant::now();
        for n in 0..cases.len() {
            let case_id = (n * 17 + round * 13 + block * 19) % cases.len();
            let case = &cases[case_id];
            let capped_first = (round + block).is_multiple_of(2);
            for capped in [capped_first, !capped_first] {
                let (offset, limit) = if capped {
                    (0, LIMIT)
                } else {
                    (case.offset, case.limit)
                };
                let start = Instant::now();
                let page = reader.search(&case.text, offset, limit, capped)?;
                let latency = ms(start);
                assert!(
                    page == oracle[capped as usize][case_id],
                    "sample oracle {} {capped}",
                    case.text
                );
                calls.push(
                    json!({"case":case_id,"text":case.text,"offset":offset,"limit":limit,
                    "capped":capped,"ms":latency,"total":page.total,"hits":page.hits.len()}),
                );
            }
        }
        emit(
            json!({"mode":"matrix","arm":arm.name(),"block":block,"round":round,
            "wall_ms":ms(round_started),"cpu_ms":cpu_ms().unwrap()-cpu_start,"calls":calls}),
        );
    }
    Ok(())
}

// One writer lives on the stack per visibility run.
#[allow(clippy::large_enum_variant)]
enum Writer {
    Fdb(Database, siftwake::SnapshotReader),
    Tantivy(Fields, IndexReader, tantivy::IndexWriter),
}

impl Writer {
    fn open(arm: Arm, path: &Path) -> Result<Self> {
        match arm {
            Arm::Fdb => {
                let mut db = Database::open(path)?;
                let reader = db.snapshot_reader()?;
                Ok(Self::Fdb(db, reader))
            }
            Arm::Tantivy => {
                let (index, fields, reader) = tantivy_arm::open_index(path)?;
                let writer = index.writer_with_num_threads(1, tantivy_arm::WRITER_BYTES)?;
                Ok(Self::Tantivy(fields, reader, writer))
            }
        }
    }

    fn view(&self) -> Result<ReadView> {
        match self {
            Self::Fdb(_, reader) => Ok(ReadView::Fdb(reader.acquire()?)),
            Self::Tantivy(fields, reader, _) => Ok(ReadView::Tantivy(*fields, reader.searcher())),
        }
    }

    fn replace(&mut self, id: usize, document: Document, version: u64) -> Result<()> {
        match self {
            Self::Fdb(db, _) => {
                db.commit_streamed(std::iter::once(Ok(Mutation::Put {
                    document,
                    guard: Guard::Version { version },
                })))?;
            }
            Self::Tantivy(fields, _, writer) => {
                writer.delete_term(tantivy_arm::id_term(*fields, id));
                writer.add_document(tantivy_arm::encode(*fields, id, document)?)?;
                writer.commit()?;
            }
        }
        Ok(())
    }

    fn publish(&self) -> Result<()> {
        if let Self::Tantivy(_, reader, _) = self {
            reader.reload()?;
        }
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if let Self::Tantivy(_, _, writer) = self {
            writer.wait_merging_threads()?;
        }
        Ok(())
    }
}

fn visibility(arm: Arm, path: &Path, start_at: usize) -> Result<()> {
    let mut corpus = corpus()?;
    let selections: Vec<_> = [
        (2_048, 4_096),
        (4_096, 16_384),
        (16_384, 65_536),
        (65_536, 200_000),
    ]
    .into_iter()
    .flat_map(|(low, high)| {
        corpus
            .index
            .entries
            .iter()
            .enumerate()
            .filter(move |(_, entry)| (low..high).contains(&entry.text_bytes))
            .take(6)
            .map(|(id, _)| id)
    })
    .collect();
    assert_eq!(selections.len(), 24);
    let mut writer = Writer::open(arm, path)?;
    for (i, id) in selections.into_iter().enumerate().skip(start_at) {
        let marker = format!("fzdbtantivyvisibilityq{id:010}k");
        let mut document = original(&mut corpus, id)?;
        document.text.push(' ');
        document.text.push_str(&marker);
        let initial = writer.view()?;
        let before = initial.search(&marker, 0, LIMIT, true)?;
        if before.total != 0 {
            assert!(arm == Arm::Tantivy && start_at != 0 && i == start_at);
            assert_eq!(before.total, 1);
            assert!(before.hits == [document]);
            emit(
                json!({"mode":"visibility_recovered","arm":arm.name(),"sample":i,"id":id,
                "outcome":"already visible on reopen; no complete latency sample"}),
            );
            continue;
        }
        let old = if i == 0 {
            Some(initial)
        } else {
            drop(initial);
            None
        };
        let bytes = document.text.len();
        let mutation = document.clone();
        let start = Instant::now();
        writer.replace(id, mutation, (id / BATCH + 1) as u64)?;
        let commit_ms = ms(start);
        writer.publish()?;
        let ready_ms = ms(start);
        let page = writer.view()?.search(&marker, 0, LIMIT, true)?;
        let visible_ms = ms(start);
        assert_eq!(page.total, 1);
        assert!(page.hits == [document]);
        if let Some(old) = old {
            assert_eq!(old.search(&marker, 0, LIMIT, true)?.total, 0);
        }
        emit(
            json!({"mode":"visibility","arm":arm.name(),"sample":i,"id":id,"text_bytes":bytes,
            "commit_ms":commit_ms,"ready_ms":ready_ms,"visible_ms":visible_ms}),
        );
    }
    writer.finish()?;
    Ok(())
}

fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    assert!(
        (3..=5).contains(&args.len()),
        "expected arm mode path [block samples]"
    );
    let arm = match args[0].as_str() {
        "fdb" => Arm::Fdb,
        "tantivy" => Arm::Tantivy,
        _ => return Err("unknown arm".into()),
    };
    let path = Path::new(&args[2]);
    match args[1].as_str() {
        "seed" => seed(arm, path),
        "verify" => checked(arm, path),
        "open" => open_probe(arm, path),
        "idle" => idle(arm, path),
        "matrix" => matrix(arm, path, args[3].parse()?, args[4].parse()?),
        "visibility" => visibility(arm, path, args.get(3).map_or(Ok(0), |arg| arg.parse())?),
        _ => Err("unknown mode".into()),
    }
}
