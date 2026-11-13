//! Isolated key-cost pool in the existing owner database; never creates a corpus.
use fdb::{Document, FirstPage, Guard, Key, MatchTotal, Mutation, Query, StoredDocument};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    sync::{
        Arc, Barrier, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub type Result<T> = super::churn::Result<T>;
pub trait View {
    fn sequence(&self) -> u64;
    fn first(&self, q: &Query) -> Result<FirstPage>;
    fn get(&self, k: &Key) -> Result<Option<StoredDocument>>;
    fn begin_probe_diagnostic(&self) -> Box<dyn FnOnce() -> serde_json::Value> {
        Box::new(|| serde_json::Value::Null)
    }
}
pub trait Client {
    type View: View;
    fn acquire(&mut self) -> Result<Self::View>;
}
pub trait Factory: Clone + Send + Sync {
    type Client: Client;
    fn open(&self) -> Result<Self::Client>;
}
pub trait Store {
    type Factory: Factory;
    fn commit(&mut self, m: Vec<Mutation>) -> Result<u64>;
    fn factory(&mut self) -> Result<Self::Factory>;
    fn keys(&self, collection: &str) -> Result<Vec<Key>>;
    fn maintain(&mut self) -> Result<()>;
    fn sequence(&self) -> Result<u64>;
    fn diagnostic_layout(&self) -> Result<serde_json::Value> {
        Ok(json!({"sequence":self.sequence()?}))
    }
    fn begin_commit_diagnostic(&mut self) -> Result<()> {
        Ok(())
    }
    fn take_commit_diagnostic(&mut self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
#[derive(Clone, Deserialize)]
pub struct Entry {
    pub offset: u64,
    pub size: usize,
    pub crc: u32,
}
fn bodies(entries: &[Entry]) -> Result<Arc<Bodies>> {
    let mut corpus = File::open(std::env::var("FDB_BENCH_CORPUS_PATH")?)?;
    let mut docs = vec![];
    for e in entries {
        corpus.seek(SeekFrom::Start(e.offset))?;
        let mut raw = vec![0; e.size];
        corpus.read_exact(&mut raw)?;
        assert_eq!(crc32fast::hash(&raw), e.crc);
        let v: serde_json::Value = serde_json::from_slice(&raw)?;
        let mut doc: Document = serde_json::from_value(v["document"].clone())?;
        while doc.text.len() > 199_936 {
            doc.text.pop();
        }
        docs.push(doc);
    }
    Ok(Arc::new(Bodies {
        folded: docs
            .iter()
            .map(|d| super::churn::fold(&format!("{}\n{}", d.title, d.text)))
            .collect(),
        docs,
    }))
}
pub fn check_saved<S: Store>(db: &mut S, path: &str) -> Result<()> {
    #[derive(Deserialize)]
    struct State {
        variant: String,
        sequence: u64,
        rows: Vec<Row>,
        entries: Vec<Entry>,
    }
    let state: State = serde_json::from_slice(&std::fs::read(path)?)?;
    assert_eq!(db.sequence()?, state.sequence);
    let templates = bodies(&state.entries)?;
    let factory = db.factory()?;
    let mut client = factory.open()?;
    let view = client.acquire()?;
    for r in &state.rows {
        assert_eq!(
            view.get(&key(&state.variant, r))?,
            Some(StoredDocument {
                document: document(&templates, &state.variant, r),
                version: r.version
            })
        );
    }
    for text in TEXTS {
        verify(
            &view.first(&query(&state.variant, text))?,
            &state.rows,
            &templates,
            &state.variant,
            text,
        );
    }
    super::churn::emit(
        json!({"phase":"pool_verified","sequence":view.sequence(),"rows":state.rows.len(),"oracle":true}),
    );
    Ok(())
}
#[derive(Deserialize)]
pub struct Config {
    pub variant: String,
    pub entries: Vec<Entry>,
    pub output_state: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Row {
    pub slot: usize,
    pub revision: u64,
    pub identity: u64,
    pub mtime: u64,
    pub version: u64,
}
const BASE: u64 = 1_700_000_000_000_000;
const COUNT: usize = 257;
const TEXTS: [&str; 5] = [
    "fdbkeypoolmark",
    "olduğu",
    "üreticisi",
    "olduğu birlikte",
    "qzxkvwkeypoolabsent",
];
struct Bodies {
    docs: Vec<Document>,
    folded: Vec<String>,
}
impl std::ops::Deref for Bodies {
    type Target = [Document];
    fn deref(&self) -> &Self::Target {
        &self.docs
    }
}
pub fn collection(variant: &str) -> &'static str {
    match variant {
        "numeric" => "keybench-numeric",
        "frunner" => "keybench-frunner",
        _ => panic!("key variant"),
    }
}
pub fn path(slot: usize) -> String {
    let size = if slot == 256 {
        1000
    } else {
        [40, 80, 160, 300][(slot / 16) % 4]
    };
    let prefix = format!("C:\\pool\\ş\\{slot:04}\\");
    let tail = "\\file.txt";
    format!(
        "{prefix}{}{tail}",
        "a".repeat(size - prefix.len() - tail.len())
    )
}
pub fn key(variant: &str, row: &Row) -> Key {
    let id = if variant == "numeric" {
        format!("{:010}", row.identity)
    } else {
        format!("{:016x}|{}", !row.mtime, path(row.slot))
    };
    Key::new("shared", collection(variant), &id)
}
fn marker(row: &Row) -> String {
    format!("zqkeyp{:04}r{:08}zq", row.slot, row.revision)
}
fn document(templates: &[Document], variant: &str, row: &Row) -> Document {
    let mut d = templates[row.slot % 256].clone();
    d.key = key(variant, row);
    while d.text.len() > 199_936 {
        d.text.pop();
    }
    d.text.push_str(&format!(" fdbkeypoolmark {}", marker(row)));
    d
}
fn query(variant: &str, text: &str) -> Query {
    let mut q = Query::new("shared");
    q.collection = Some(collection(variant).into());
    q.text = text.into();
    q
}
type Versions = Vec<Vec<(u64, Option<Row>)>>;
struct History {
    versions: Versions,
    sequence: u64,
}
type Shared = Arc<(Mutex<History>, Condvar)>;
fn publish(h: &Shared, seq: u64, slot: usize, row: Option<Row>) {
    let (lock, cv) = &**h;
    let mut hist = lock.lock().unwrap();
    hist.versions[slot].push((seq, row));
    hist.sequence = seq;
    cv.notify_all();
}
fn at(h: &Shared, sequence: u64) -> Vec<Row> {
    let (lock, cv) = &**h;
    let mut hist = lock.lock().unwrap();
    while hist.sequence < sequence {
        hist = cv.wait(hist).unwrap();
    }
    hist.versions
        .iter()
        .filter_map(|vs| {
            let end = vs.partition_point(|(seq, _)| *seq <= sequence);
            end.checked_sub(1).and_then(|i| vs[i].1.clone())
        })
        .collect()
}
fn verify(page: &FirstPage, rows: &[Row], templates: &Bodies, variant: &str, text: &str) {
    let terms: Vec<_> = text.split_whitespace().map(super::churn::fold).collect();
    let mut matched: Vec<_> = rows
        .iter()
        .filter(|r| {
            let suffix = format!("fdbkeypoolmark {}", marker(r));
            terms
                .iter()
                .all(|t| templates.folded[r.slot % 256].contains(t) || suffix.contains(t))
        })
        .collect();
    matched.sort_by_key(|r| key(variant, r));
    let total = matched.len();
    matched.truncate(50);
    let expected: Vec<_> = matched
        .into_iter()
        .map(|r| StoredDocument {
            document: document(templates, variant, r),
            version: r.version,
        })
        .collect();
    assert_eq!(page.hits, expected);
    assert_eq!(
        page.total,
        if total >= 50 {
            MatchTotal::AtLeast(50)
        } else {
            MatchTotal::Exact(total)
        }
    );
}
#[allow(clippy::too_many_arguments)]
fn readers<F: Factory>(
    factory: &F,
    h: &Shared,
    templates: &Arc<Bodies>,
    variant: &str,
    count: usize,
    rounds: usize,
    active: Option<&AtomicBool>,
    barrier: Option<&Barrier>,
) -> Result<Vec<serde_json::Value>> {
    std::thread::scope(|scope| -> Result<_> {
        let handles: Vec<_> = (0..count).map(|_| {
            let f=factory.clone();let h=h.clone();let templates=templates.clone();
            scope.spawn(move || -> Result<Vec<serde_json::Value>> {
                let mut client=f.open()?; let mut records=vec![];
                if let Some(b)=barrier {b.wait();}
                let mut round=0;
                loop {
                    for (case,text) in TEXTS.iter().enumerate() {
                        let was_active=active.is_some_and(|a|a.load(Ordering::Acquire));
                        let began=Instant::now();let view=client.acquire()?;
                        let page=view.first(&query(variant,text))?;let seq=view.sequence();drop(view);
                        let engine_ms=began.elapsed().as_secs_f64()*1000.;
                        let oa=Instant::now();verify(&page,&at(&h,seq),&templates,variant,text);
                        records.push(json!({"case":case,"text":text,"engine_ms":engine_ms,"sequence":seq,"writer_active":was_active,"oracle_ms":oa.elapsed().as_secs_f64()*1000.,"oracle":true}));
                    }
                    round+=1;
                    if round>=rounds && !active.is_some_and(|a|a.load(Ordering::Acquire)) {break;}
                }
                Ok(records)
            })
        }).collect();
        handles
            .into_iter()
            .map(|t| Ok(json!(t.join().map_err(|_| "reader panic")??)))
            .collect()
    })
}
pub fn cleanup<S: Store>(db: &mut S) -> Result<()> {
    for col in [collection("numeric"), collection("frunner")] {
        for k in db.keys(col)? {
            assert_eq!(k.collection, col);
            db.commit(vec![Mutation::Delete {
                key: k,
                guard: Guard::Any,
            }])?;
        }
    }
    db.maintain()
}
pub fn run<S: Store>(db: &mut S, config: Config) -> Result<()> {
    // A failed oracle must terminate this isolated measurement process, rather
    // than leave scoped readers waiting for a writer that panicked.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        std::process::exit(101);
    }));
    let variant = &config.variant;
    collection(variant);
    assert_eq!(config.entries.len(), 256);
    cleanup(db)?;
    let templates = bodies(&config.entries)?;
    let h: Shared = Arc::new((
        Mutex::new(History {
            versions: vec![vec![]; COUNT],
            sequence: db.sequence()?,
        }),
        Condvar::new(),
    ));
    let mut current = vec![None; COUNT];
    let mut revs = [0u64; COUNT];
    let setup = Instant::now();
    for (slot, live) in current.iter_mut().enumerate() {
        let mut row = Row {
            slot,
            revision: 0,
            identity: slot as u64,
            mtime: BASE,
            version: 0,
        };
        row.version = db.commit(vec![Mutation::Put {
            document: document(&templates, variant, &row),
            guard: Guard::Any,
        }])?;
        publish(&h, row.version, slot, Some(row.clone()));
        *live = Some(row);
    }
    db.maintain()?;
    let factory = db.factory()?;
    super::churn::emit(
        json!({"phase":"pool_setup","variant":variant,"documents":COUNT,"elapsed_ms":setup.elapsed().as_secs_f64()*1000.,"sequence":db.sequence()?,"memory":super::churn::memory()}),
    );
    for (count, rounds) in [(1, 60), (4, 15)] {
        let records = readers(&factory, &h, &templates, variant, count, rounds, None, None)?;
        super::churn::emit(
            json!({"phase":if count==1{"idle1"}else{"idle4"},"records":records,"oracle":true}),
        );
    }
    let active = AtomicBool::new(true);
    let barrier = Barrier::new(5);
    let mut writer_ms = 0.;
    let writes = std::thread::scope(|scope| -> Result<Vec<serde_json::Value>> {
        let reader = scope.spawn(|| {
            readers(
                &factory,
                &h,
                &templates,
                variant,
                4,
                15,
                Some(&active),
                Some(&barrier),
            )
        });
        let result = (|| -> Result<Vec<serde_json::Value>> {
            let mut client = factory.open()?;
            let mut writes = vec![];
            barrier.wait();
            let writer_start = Instant::now();
            for cycle in 0..4 {
                for (slot, revision) in revs.iter_mut().enumerate().take(256) {
                    for kind in ["update", "delete", "insert", "mtime_delete_insert"] {
                        *revision += 1;
                        let counter = 1 + cycle * 1024 + slot * 4 + writes.len() % 4;
                        let old = current[slot].clone();
                        let row = if kind == "delete" {
                            None
                        } else {
                            let mut r = old.clone().unwrap_or(Row {
                                slot,
                                revision: 0,
                                identity: 0,
                                mtime: 0,
                                version: 0,
                            });
                            r.revision = *revision;
                            if kind != "update" {
                                r.identity = 500_000 + counter as u64;
                                r.mtime = BASE + COUNT as u64 + counter as u64;
                            }
                            Some(r)
                        };
                        let record =
                            perform(db, &mut client, &h, &templates, variant, old, row, kind)?;
                        current[slot] = record.0;
                        writes.push(record.1);
                    }
                }
            }
            // Exactly one measured edge transaction, with a pinned previous view.
            let old = current[256].clone().unwrap();
            let mut pin_client = factory.open()?;
            let pinned = pin_client.acquire()?;
            let mut new = old.clone();
            new.revision += 1;
            new.mtime = BASE + 1_000_000;
            new.identity = 1_000_000;
            let (row, record) = perform(
                db,
                &mut client,
                &h,
                &templates,
                variant,
                Some(old.clone()),
                Some(new),
                "edge1000_mtime",
            )?;
            assert_eq!(
                pinned.get(&key(variant, &old))?,
                Some(StoredDocument {
                    document: document(&templates, variant, &old),
                    version: old.version
                })
            );
            verify(
                &pinned.first(&query(variant, TEXTS[0]))?,
                &at(&h, pinned.sequence()),
                &templates,
                variant,
                TEXTS[0],
            );
            drop(pinned);
            current[256] = row;
            writes.push(record);
            writer_ms = writer_start.elapsed().as_secs_f64() * 1000.;
            Ok(writes)
        })();
        active.store(false, Ordering::Release);
        let reads = reader.join().map_err(|_| "reader panic")??;
        super::churn::emit(json!({"phase":"loaded","records":reads,"oracle":true}));
        result
    })?;
    let live: Vec<_> = current.into_iter().flatten().collect();
    let mut client = factory.open()?;
    let view = client.acquire()?;
    for row in &live {
        assert_eq!(
            view.get(&key(variant, row))?,
            Some(StoredDocument {
                document: document(&templates, variant, row),
                version: row.version
            })
        );
    }
    std::fs::write(
        &config.output_state,
        serde_json::to_vec(
            &json!({"variant":variant,"sequence":db.sequence()?,"rows":live,"entries":config.entries.iter().map(|e|json!({"offset":e.offset,"size":e.size,"crc":e.crc})).collect::<Vec<_>>()}),
        )?,
    )?;
    super::churn::emit(
        json!({"phase":"writes","records":writes,"writer_ms":writer_ms,"sequence":db.sequence()?,"memory":super::churn::memory(),"oracle":true}),
    );
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn perform<S: Store, C: Client>(
    db: &mut S,
    client: &mut C,
    h: &Shared,
    templates: &Bodies,
    variant: &str,
    old: Option<Row>,
    mut new: Option<Row>,
    kind: &str,
) -> Result<(Option<Row>, serde_json::Value)> {
    let slot = old.as_ref().or(new.as_ref()).unwrap().slot;
    let prepare = Instant::now();
    let mut changes = vec![];
    if let Some(o) = &old
        && new
            .as_ref()
            .is_none_or(|n| key(variant, n) != key(variant, o))
    {
        changes.push(Mutation::Delete {
            key: key(variant, o),
            guard: Guard::Any,
        });
    }
    if let Some(n) = &new {
        changes.push(Mutation::Put {
            document: document(templates, variant, n),
            guard: Guard::Any,
        });
    }
    let prepare_ms = prepare.elapsed().as_secs_f64() * 1000.;
    let mutations = changes.len();
    let stride: u64 = std::env::var("POOL_PROFILE_STRIDE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let diagnostic = std::env::var_os("POOL_VISIBILITY_PROFILE").is_some()
        && (db.sequence()? + 1).is_multiple_of(stride.max(1));
    if diagnostic {
        db.begin_commit_diagnostic()?;
    }
    let began = Instant::now();
    let seq = db.commit(changes)?;
    let commit_done_ns = began.elapsed().as_nanos() as u64;
    let commit_ms = commit_done_ns as f64 / 1_000_000.;
    if let Some(n) = &mut new {
        n.version = seq;
    }
    publish(h, seq, slot, new.clone());
    let history_done_ns = began.elapsed().as_nanos() as u64;
    let view = client.acquire()?;
    let acquire_done_ns = began.elapsed().as_nanos() as u64;
    assert_eq!(view.sequence(), seq);
    let probe = new.as_ref().or(old.as_ref()).unwrap();
    let probe_query = query(variant, &marker(probe));
    let query_prepare_done_ns = began.elapsed().as_nanos() as u64;
    let finish_probe = diagnostic.then(|| view.begin_probe_diagnostic());
    let diagnostic_begin_done_ns = began.elapsed().as_nanos() as u64;
    let page = view.first(&probe_query)?;
    let query_done_ns = began.elapsed().as_nanos() as u64;
    assert_eq!(page.hits.len(), usize::from(new.is_some()));
    drop(view);
    let release_done_ns = began.elapsed().as_nanos() as u64;
    let visibility_ms = release_done_ns as f64 / 1_000_000.;
    let diagnostic_output = if let Some(finish) = finish_probe {
        let at = Instant::now();
        let query = finish();
        let commit = db.take_commit_diagnostic();
        json!({"milestones_ns":{"commit_return":commit_done_ns,"history_done":history_done_ns,"acquire_done":acquire_done_ns,"probe_prepared":query_prepare_done_ns,"diagnostic_begin_done":diagnostic_begin_done_ns,"query_done":query_done_ns,"release_done":release_done_ns},"query":query,"commit":commit,"collection_after_visibility_ns":at.elapsed().as_nanos() as u64})
    } else {
        serde_json::Value::Null
    };
    let view = client.acquire()?;
    if let Some(n) = &new {
        assert_eq!(
            view.get(&key(variant, n))?,
            Some(StoredDocument {
                document: document(templates, variant, n),
                version: seq
            })
        );
        if variant == "frunner"
            && (kind == "insert" || kind == "mtime_delete_insert" || kind == "edge1000_mtime")
        {
            assert!(
                at(h, seq)
                    .iter()
                    .filter(|r| r.slot != slot)
                    .all(|r| key(variant, n) < key(variant, r)),
                "new mtime must be first"
            );
        }
    }
    if let Some(o) = &old {
        assert!(view.first(&query(variant, &marker(o)))?.hits.is_empty());
        if new
            .as_ref()
            .is_none_or(|n| key(variant, n) != key(variant, o))
        {
            assert!(view.get(&key(variant, o))?.is_none());
        }
    }
    let row = new.as_ref().or(old.as_ref()).unwrap();
    Ok((
        new.clone(),
        json!({"kind":kind,"slot":slot,"body_band":slot/64,"path_bytes":path(slot).len(),"key_bytes":key(variant,row).id.len(),"mutations":mutations,"sequence":seq,"prepare_ms":prepare_ms,"commit_ms":commit_ms,"visibility_ms":visibility_ms,"timeline_ns":{"commit_return":commit_done_ns,"history_done":history_done_ns,"acquire_done":acquire_done_ns,"probe_prepared":query_prepare_done_ns,"diagnostic_begin_done":diagnostic_begin_done_ns,"query_done":query_done_ns,"release_done":release_done_ns},"diagnostic":diagnostic_output,"oracle":true}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf8_paths_fit_existing_key_bounds_and_new_mtime_sorts_first() {
        for slot in 0..COUNT {
            let size = if slot == 256 {
                1000
            } else {
                [40, 80, 160, 300][(slot / 16) % 4]
            };
            assert_eq!(path(slot).len(), size);
            let old = Row {
                slot,
                revision: 0,
                identity: slot as u64,
                mtime: BASE,
                version: 1,
            };
            let new = Row {
                mtime: BASE + 1,
                ..old.clone()
            };
            assert_eq!(key("frunner", &old).id.len(), size + 17);
            assert!(key("frunner", &new) < key("frunner", &old));
            assert_eq!(key("numeric", &old).id.len(), 10);
        }
        assert_eq!(collection("numeric").len(), collection("frunner").len());
    }
    #[test]
    fn model_chooses_one_commit_epoch_and_retains_old_deleted_key() {
        let row = Row {
            slot: 0,
            revision: 0,
            identity: 0,
            mtime: BASE,
            version: 1,
        };
        let h = Arc::new((
            Mutex::new(History {
                versions: vec![vec![(1, Some(row.clone()))]],
                sequence: 1,
            }),
            Condvar::new(),
        ));
        publish(&h, 2, 0, None);
        let new = Row {
            mtime: BASE + 1,
            version: 3,
            ..row
        };
        publish(&h, 3, 0, Some(new));
        assert_eq!(at(&h, 1).len(), 1);
        assert!(at(&h, 2).is_empty());
        assert!(key("frunner", &at(&h, 3)[0]) < key("frunner", &at(&h, 1)[0]));
    }
}
