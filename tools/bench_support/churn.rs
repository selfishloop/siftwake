//! Measurement-only workload. No engine hooks, format changes or corpus generation.
use fdb::{Document, Guard, Key, Mutation, SearchPage, StoredDocument};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub const INDEX: &str = "docs/benchmarks/churn-10k-20261024-corpus-index.json";
pub fn emit(value: Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            match c {
                'I' | 'İ' | 'ı' => 'i',
                'Ş' | 'ş' => 's',
                'Ğ' | 'ğ' => 'g',
                'Ç' | 'ç' => 'c',
                'Ö' | 'ö' => 'o',
                'Ü' | 'ü' => 'u',
                x => x,
            }
            .to_lowercase()
        })
        .collect()
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Case {
    pub text: String,
    pub offset: usize,
    pub limit: usize,
    pub base: usize,
}
pub fn cases() -> Vec<Case> {
    let texts = [
        "quixote",
        "zeuglodon",
        "beagle",
        "mesopotamia",
        "transubstantiation",
        "harpoon",
        "ishmael",
        "invertebrata",
        "darcy",
        "elizabeth",
        "watson",
        "holmes",
        "frankenstein",
        "galapagos",
        "fossil",
        "geologist",
        "the",
        "man",
        "he",
        "zzxqnever",
        "zx",
        "the zx",
        "quixote whale",
        "holmes elementary",
        "fossil geologist",
        "whale harpoon",
        "QUİXOTE",
        "İSHMAEL",
    ];
    let mut out: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(base, text)| Case {
            text: (*text).into(),
            offset: 0,
            limit: 50,
            base,
        })
        .collect();
    out.push(Case {
        text: "zx".repeat(150),
        offset: 0,
        limit: 50,
        base: 28,
    });
    out.push(Case {
        text: "presumption".into(),
        offset: 0,
        limit: 50,
        base: 29,
    });
    for (base, offset, limit) in [
        (16, 0, 1),
        (16, 0, 200),
        (16, 5000, 50),
        (16, 9950, 50),
        (6, 130, 1),
        (6, 60, 200),
        (0, 28, 10),
        (0, 57, 50),
    ] {
        out.push(Case {
            text: out[base].text.clone(),
            offset,
            limit,
            base,
        });
    }
    out
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    offset: u64,
    size: usize,
    crc: u32,
    pub mask: u64,
    pub text_bytes: usize,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CorpusIndex {
    pub entries: Vec<Entry>,
    pub cases: Vec<Case>,
}
pub struct Corpus {
    pub index: CorpusIndex,
    file: File,
    original_bodies: bool,
}
#[allow(dead_code)] // Only the FDB executable prepares shared metadata.
fn word(b: &[u8]) -> u32 {
    u32::from_le_bytes(b.try_into().unwrap())
}
fn trimmed(text: &str) -> &str {
    let mut end = text.len().min(51_140);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
impl Corpus {
    #[allow(dead_code)]
    pub fn prepare() -> Result<()> {
        Self::catalog(10000, INDEX)
    }
    /// Metadata only; reads existing payloads, never generates document data.
    pub fn catalog(count: usize, destination: &str) -> Result<()> {
        if std::path::Path::new(destination).exists() {
            return Err("metadata destination already exists".into());
        }
        let mut file = File::open("data/rkyv-corpus-20261009/json.payload-bench")?;
        let mut header = [0; 20];
        file.read_exact(&mut header)?;
        assert_eq!(&header[..8], b"PLOADB01");
        assert_eq!(crc32fast::hash(&header[..16]), word(&header[16..]));
        assert!((1..=word(&header[12..16]) as usize).contains(&count));
        let cases = cases();
        let terms: Vec<Vec<String>> = cases[..30]
            .iter()
            .map(|c| c.text.split_whitespace().map(fold).collect())
            .collect();
        let mut entries = Vec::new();
        for id in 0..count {
            let mut h = [0; 16];
            file.read_exact(&mut h)?;
            assert_eq!(word(&h[8..12]) as usize, id);
            assert_eq!(crc32fast::hash(&h[..12]), word(&h[12..]));
            let size = word(&h[..4]) as usize;
            assert!(size < 4 * 1024 * 1024);
            let offset = file.stream_position()?;
            let mut bytes = vec![0; size];
            file.read_exact(&mut bytes)?;
            assert_eq!(crc32fast::hash(&bytes), word(&h[4..8]));
            let row: StoredDocument = serde_json::from_slice(&bytes)?;
            let title = fold(&row.document.title);
            let body = fold(trimmed(&row.document.text));
            let mut mask = 0;
            for (n, ts) in terms.iter().enumerate() {
                if ts.iter().all(|t| title.contains(t) || body.contains(t)) {
                    mask |= 1 << n;
                }
            }
            entries.push(Entry {
                offset,
                size,
                crc: word(&h[4..8]),
                mask,
                text_bytes: row.document.text.len(),
            });
        }
        fs::write(
            destination,
            serde_json::to_vec(&CorpusIndex { entries, cases })?,
        )?;
        emit(json!({"phase":"prepared","documents":count,"new_corpus":false,"metadata_only":true}));
        Ok(())
    }
    pub fn open() -> Result<Self> {
        let mut index: CorpusIndex = serde_json::from_slice(&fs::read(
            std::env::var("FDB_BENCH_CORPUS_INDEX").unwrap_or_else(|_| INDEX.into()),
        )?)?;
        if let Ok(count) = std::env::var("FDB_SCALE_DOCUMENTS") {
            let count = count.parse::<usize>()?;
            assert!(count <= index.entries.len());
            index.entries.truncate(count);
            index.entries.shrink_to_fit();
        }
        Ok(Self {
            index,
            file: File::open(
                std::env::var("FDB_BENCH_CORPUS_PATH")
                    .unwrap_or_else(|_| "data/rkyv-corpus-20261009/json.payload-bench".into()),
            )?,
            original_bodies: std::env::var_os("FDB_BENCH_ORIGINAL_BODIES").is_some(),
        })
    }
    pub fn document(&mut self, id: usize, row: &Row) -> Result<Document> {
        let e = &self.index.entries[row.body];
        self.file.seek(SeekFrom::Start(e.offset))?;
        let mut bytes = vec![0; e.size];
        self.file.read_exact(&mut bytes)?;
        assert_eq!(crc32fast::hash(&bytes), e.crc);
        let mut doc: StoredDocument = serde_json::from_slice(&bytes)?;
        doc.document.key = key(id);
        if self.original_bodies {
            if row.revision != 0 {
                if std::env::var_os("FDB_BENCH_50K_WRITE").is_some() {
                    let token = marker(id, row.revision);
                    let mut end = doc.document.text.len().min(200_000 - token.len() - 1);
                    while !doc.document.text.is_char_boundary(end) {
                        end -= 1;
                    }
                    doc.document.text.truncate(end);
                    doc.document.text.push(' ');
                    doc.document.text.push_str(&token);
                    assert!((2048..=200_000).contains(&doc.document.text.len()));
                    return Ok(doc.document);
                }
                return Err("provided-corpus stage is read/seed only; revision mutation needs a separate oracle".into());
            }
            return Ok(doc.document);
        }
        doc.document.text = format!(
            "{} {}",
            trimmed(&doc.document.text),
            marker(id, row.revision)
        );
        assert!((2048..=51200).contains(&doc.document.text.len()));
        Ok(doc.document)
    }
}
pub fn key(id: usize) -> Key {
    Key::new("shared", "texts", &format!("{id:010}"))
}
pub fn marker(id: usize, revision: u64) -> String {
    format!("zqv{id:010}r{revision:010}zq")
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Row {
    pub body: usize,
    pub revision: u64,
    pub version: u64,
}
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Model {
    pub rows: BTreeMap<usize, Row>,
    pub dead: BTreeMap<usize, Row>,
    pub next: usize,
    pub sequence: u64,
}
impl Model {
    pub fn save(&self, path: &str) -> Result<()> {
        fs::write(path, serde_json::to_vec(self)?)?;
        Ok(())
    }
    pub fn load(path: &str) -> Result<Self> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
    pub fn expected_ids(&self, corpus: &Corpus, case: &Case) -> Vec<usize> {
        self.rows
            .iter()
            .filter(|(_, r)| corpus.index.entries[r.body].mask & (1 << case.base) != 0)
            .map(|(id, _)| *id)
            .collect()
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Change {
    pub id: usize,
    pub old: Option<Row>,
    pub new: Option<Row>,
    pub kind: String,
}
pub trait Backend: Send {
    fn query_variant(&self, text: &str, variant: &str) -> Result<fdb::FirstPage> {
        if variant != "legacy" {
            return Err("query experiments unsupported".into());
        }
        let p = self.search(text, 0, 50)?;
        Ok(fdb::FirstPage {
            total: fdb::MatchTotal::Exact(p.total),
            sequence: p.sequence,
            hits: p.hits,
            validation: fdb::ValidationStatus::Verified,
        })
    }
    fn query_profile_reset(&self) {}
    fn query_profile_take(&self) -> Value {
        Value::Null
    }
    fn get(&self, id: usize) -> Result<Option<StoredDocument>>;
    fn search(&self, text: &str, offset: usize, limit: usize) -> Result<SearchPage>;
    fn commit(&mut self, corpus: &mut Corpus, changes: &[Change], streamed: bool) -> Result<u64>;
    fn maintain(&mut self) -> Result<Value>;
    fn idle_notify(&mut self) -> Result<Value> {
        self.maintain()
    }
    #[allow(dead_code)]
    fn start_merge(&mut self) -> Result<bool>;
    fn stats(&self) -> Result<Value>;
    fn liveness(&self) -> Result<Value> {
        Ok(Value::Null)
    }
}
#[allow(dead_code)] // SQLite consumes the same descriptors directly.
pub fn mutation(corpus: &mut Corpus, c: &Change) -> fdb::Result<Mutation> {
    match &c.new {
        Some(r) => Ok(Mutation::Put {
            document: corpus
                .document(c.id, r)
                .map_err(|e| fdb::Error::Invalid(e.to_string()))?,
            guard: Guard::Any,
        }),
        None => Ok(Mutation::Delete {
            key: key(c.id),
            guard: Guard::Any,
        }),
    }
}
pub fn apply(model: &mut Model, changes: &[Change], sequence: u64) {
    model.sequence = sequence;
    for c in changes {
        if let Some(mut row) = c.new.clone() {
            row.version = sequence;
            model.rows.insert(c.id, row);
            model.dead.remove(&c.id);
        } else if let Some(old) = model.rows.remove(&c.id) {
            model.dead.insert(c.id, old);
        }
    }
}
pub fn plan(model: &mut Model, kind: &str, n: usize, salt: usize) -> Vec<Change> {
    let ids: Vec<_> = model.rows.keys().copied().collect();
    let mut out = Vec::new();
    for at in 0..n {
        let op = if kind == "mixed" {
            match (at + salt) % 4 {
                0 | 1 => "update",
                2 => "delete",
                _ => "insert",
            }
        } else {
            kind
        };
        if op == "insert" {
            let id = model.next;
            model.next += 1;
            out.push(Change {
                id,
                old: None,
                new: Some(Row {
                    body: (id * 997 + salt) % 10000,
                    revision: 1,
                    version: 0,
                }),
                kind: op.into(),
            });
        } else {
            let id = ids[(salt * 101 + at) % ids.len()];
            let old = model.rows[&id].clone();
            let new = (op == "update").then(|| Row {
                body: (old.body + 997 + old.revision as usize * 13) % 10000,
                revision: old.revision + 1,
                version: 0,
            });
            out.push(Change {
                id,
                old: Some(old),
                new,
                kind: op.into(),
            });
        }
    }
    out
}
pub fn verify_change(
    db: &impl Backend,
    corpus: &mut Corpus,
    model: &Model,
    c: &Change,
) -> Result<()> {
    let actual = db.get(c.id)?;
    match model.rows.get(&c.id) {
        Some(row) => assert_eq!(
            actual,
            Some(StoredDocument {
                document: corpus.document(c.id, row)?,
                version: row.version
            })
        ),
        None => assert!(actual.is_none()),
    }
    if let Some(old) = &c.old {
        let page = db.search(&marker(c.id, old.revision), 0, 200)?;
        assert_eq!(page.total, 0, "old revision visible");
    }
    if let Some(row) = model.rows.get(&c.id) {
        let page = db.search(&marker(c.id, row.revision), 0, 200)?;
        assert_eq!(page.total, 1);
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].document.key, key(c.id));
    }
    Ok(())
}
pub fn verify_all(db: &impl Backend, corpus: &mut Corpus, model: &Model) -> Result<()> {
    for (&id, row) in &model.rows {
        assert_eq!(
            db.get(id)?,
            Some(StoredDocument {
                document: corpus.document(id, row)?,
                version: row.version
            })
        );
    }
    for &id in model.dead.keys() {
        assert!(db.get(id)?.is_none());
    }
    Ok(())
}
pub fn check_page(
    corpus: &mut Corpus,
    model: &Model,
    case: &Case,
    page: &SearchPage,
) -> Result<()> {
    let ids = model.expected_ids(corpus, case);
    assert_eq!(page.total, ids.len(), "total {}", case.text);
    let wanted: Vec<_> = ids.into_iter().skip(case.offset).take(case.limit).collect();
    assert_eq!(page.hits.len(), wanted.len());
    for (actual, id) in page.hits.iter().zip(wanted) {
        let row = &model.rows[&id];
        assert_eq!(
            *actual,
            StoredDocument {
                document: corpus.document(id, row)?,
                version: row.version
            }
        );
    }
    Ok(())
}
pub fn matrix(
    db: &impl Backend,
    corpus: &mut Corpus,
    model: &Model,
    repeats: usize,
    label: &str,
) -> Result<Value> {
    let cases = corpus.index.cases.clone();
    let mut records = Vec::new();
    for case in &cases {
        let page = db.search(&case.text, case.offset, case.limit)?;
        check_page(corpus, model, case, &page)?;
        let mut samples = Vec::new();
        for _ in 0..repeats {
            let at = Instant::now();
            let page = db.search(&case.text, case.offset, case.limit)?;
            samples.push(at.elapsed().as_secs_f64() * 1000.);
            check_page(corpus, model, case, &page)?;
        }
        records.push(json!({"case":case,"samples_ms":samples,"timing":summary(&samples)}));
    }
    Ok(json!({"phase":"matrix","label":label,"records":records,"oracle":true,"stats":db.stats()?}))
}
pub fn summary(values: &[f64]) -> Value {
    if values.is_empty() {
        return json!({"n":0});
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    json!({"n":v.len(),"p50_ms":v[(v.len()-1)/2],"p95_ms":v[(v.len()*95).div_ceil(100)-1],"max_ms":v[v.len()-1]})
}
pub fn disk(path: &str) -> Result<Value> {
    let mut retained = 0u64;
    let mut source = 0;
    let mut index = 0;
    let mut entries = Vec::new();
    for e in fs::read_dir(path)? {
        let e = e?;
        if e.file_type()?.is_file() {
            let n = e.file_name().to_string_lossy().into_owned();
            let bytes = e.metadata()?.len();
            retained += bytes;
            if n.ends_with(".fdb") {
                source += bytes;
            }
            if n.ends_with(".idx") {
                index += bytes;
            }
            entries.push(json!({"name":n,"bytes":bytes}));
        }
    }
    Ok(
        json!({"retained_bytes":retained,"all_source_bytes":source,"all_index_bytes":index,"files":entries}),
    )
}
static SAMPLE_PRIVATE: AtomicUsize = AtomicUsize::new(0);
static SAMPLE_WS: AtomicUsize = AtomicUsize::new(0);
pub fn start_memory_sampler() {
    std::thread::spawn(|| {
        loop {
            let v = memory_raw();
            SAMPLE_PRIVATE.fetch_max(
                v["private_bytes"].as_u64().unwrap_or(0) as usize,
                Ordering::Relaxed,
            );
            SAMPLE_WS.fetch_max(
                v["working_set_bytes"].as_u64().unwrap_or(0) as usize,
                Ordering::Relaxed,
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    });
}
fn reset_memory_sampler() {
    SAMPLE_PRIVATE.store(0, Ordering::Relaxed);
    SAMPLE_WS.store(0, Ordering::Relaxed);
}
pub fn memory() -> Value {
    let mut v = memory_raw();
    v["sampled_phase_peak_private_bytes"] = json!(SAMPLE_PRIVATE.load(Ordering::Relaxed));
    v["sampled_phase_peak_working_set_bytes"] = json!(SAMPLE_WS.load(Ordering::Relaxed));
    v["sample_interval_ms"] = json!(10);
    v
}
/// Process kernel+user CPU, all query workers included; Windows accounting can
/// be quantized. Aggregate repeats rather than interpreting a single zero.
pub fn cpu_ms() -> Option<f64> {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessTimes(
                p: *mut std::ffi::c_void,
                c: *mut u64,
                e: *mut u64,
                k: *mut u64,
                u: *mut u64,
            ) -> i32;
        }
        let (mut c, mut e, mut k, mut u) = (0, 0, 0, 0);
        if unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) } == 0 {
            return None;
        }
        Some((k + u) as f64 / 10_000.)
    }
    #[cfg(not(windows))]
    {
        None
    }
}
fn memory_raw() -> Value {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Counters {
            cb: u32,
            faults: u32,
            peak_ws: usize,
            ws: usize,
            quota_peak_paged: usize,
            quota_paged: usize,
            quota_peak_nonpaged: usize,
            quota_nonpaged: usize,
            pagefile: usize,
            peak_pagefile: usize,
            private: usize,
        }
        #[link(name = "psapi")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessMemoryInfo(
                process: *mut std::ffi::c_void,
                info: *mut Counters,
                cb: u32,
            ) -> i32;
        }
        let mut c: Counters = unsafe { std::mem::zeroed() };
        c.cb = std::mem::size_of::<Counters>() as u32;
        let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
        assert_ne!(ok, 0);
        json!({"private_bytes":c.private,"working_set_bytes":c.ws,"os_lifetime_peak_private_commit_bytes":c.peak_pagefile,"os_lifetime_peak_working_set_bytes":c.peak_ws})
    }
    #[cfg(not(windows))]
    {
        json!({"unavailable":true})
    }
}
pub fn measured_commit(
    db: &mut impl Backend,
    corpus: &mut Corpus,
    model: &mut Model,
    changes: &[Change],
    streamed: bool,
) -> Result<Value> {
    let at = Instant::now();
    let result = db.commit(corpus, changes, streamed);
    let commit = at.elapsed().as_secs_f64() * 1000.;
    match result {
        Ok(sequence) => {
            apply(model, changes, sequence);
            let acknowledged = Instant::now();
            let mut visibility = Vec::new();
            for c in changes {
                let began = Instant::now();
                verify_change(db, corpus, model, c)?;
                visibility.push(json!({"id":c.id,"kind":c.kind,"commit_to_probe_ms":acknowledged.elapsed().as_secs_f64()*1000.,"probe_ms":began.elapsed().as_secs_f64()*1000.}));
            }
            Ok(
                json!({"ok":true,"commit_ms":commit,"visible_end_to_end_ms":at.elapsed().as_secs_f64()*1000.,"visibility":visibility,"stats":db.stats()?,"memory":memory()}),
            )
        }
        Err(e) => {
            for c in changes {
                if let Some(old) = &c.old {
                    assert_eq!(db.get(c.id)?.as_ref().map(|r| r.version), Some(old.version));
                } else {
                    assert!(db.get(c.id)?.is_none());
                }
            }
            Ok(
                json!({"ok":false,"commit_ms":commit,"error":e.to_string(),"stats":db.stats()?,"memory":memory()}),
            )
        }
    }
}

pub fn seed(db: &mut impl Backend, corpus: &mut Corpus, model: &mut Model) -> Result<()> {
    for first in (0..10000).step_by(500) {
        let changes: Vec<_> = (first..first + 500)
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
        let sequence = db.commit(corpus, &changes, true)?;
        apply(model, &changes, sequence);
    }
    model.next = 10000;
    db.maintain()?;
    verify_all(db, corpus, model)?;
    emit(json!({"phase":"seeded","documents":10000,"stats":db.stats()?,"memory":memory()}));
    Ok(())
}

pub fn run<B: Backend + 'static>(
    mut db: B,
    mode: &str,
    api: &str,
    state: &str,
    path: &str,
) -> Result<()> {
    let mut corpus = Corpus::open()?;
    let mut model = if mode == "seed" {
        Model::default()
    } else {
        Model::load(state)?
    };
    let streamed = api == "streamed";
    if mode == "seed" {
        seed(&mut db, &mut corpus, &mut model)?;
        model.save(state)?;
        return Ok(());
    }
    if mode == "idle" || mode == "open" {
        std::thread::sleep(Duration::from_secs(5));
        emit(
            json!({"phase":"idle","mode":mode,"memory":memory(),"stats":db.stats()?,"disk":disk(path)?}),
        );
        verify_all(&db, &mut corpus, &model)?;
        emit(json!({"phase":"verified","documents":model.rows.len(),"deleted":model.dead.len()}));
        return Ok(());
    }
    if mode == "verify" {
        verify_all(&db, &mut corpus, &model)?;
        emit(matrix(&db, &mut corpus, &model, 1, "verify")?);
        return Ok(());
    }
    if mode == "search-matrix" {
        emit(matrix(&db, &mut corpus, &model, 30, "revision-search")?);
        return Ok(());
    }
    if mode == "smoke" {
        let changes = plan(&mut model, "mixed", 4, 4);
        let result = measured_commit(&mut db, &mut corpus, &mut model, &changes, streamed)?;
        assert_eq!(result["ok"], true);
        emit(json!({"phase":"smoke_commit","result":result}));
        verify_all(&db, &mut corpus, &model)?;
        emit(matrix(&db, &mut corpus, &model, 1, "smoke")?);
        model.save(state)?;
        return Ok(());
    }
    if mode == "latency" || mode == "latency-single" {
        for &n in if mode == "latency-single" {
            &[1][..]
        } else {
            &[1, 10, 100, 1000][..]
        } {
            db.maintain()?;
            for kind in ["update", "delete", "insert", "mixed"] {
                let samples = match n {
                    1 => 32,
                    10 => 12,
                    100 => 6,
                    _ => 3,
                };
                let mut results = Vec::new();
                for sample in 0..samples {
                    let changes = plan(&mut model, kind, n, sample);
                    let row =
                        measured_commit(&mut db, &mut corpus, &mut model, &changes, streamed)?;
                    let ok = row["ok"] == true;
                    results.push(row);
                    if !ok {
                        break;
                    }
                    // Restore logical live count after homogeneous delete/insert.
                    if kind == "delete" || kind == "insert" {
                        let restore: Vec<_> = changes
                            .iter()
                            .map(|c| Change {
                                id: c.id,
                                old: c.new.clone(),
                                new: c.old.clone(),
                                kind: "restore".into(),
                            })
                            .collect();
                        let sequence = db.commit(&mut corpus, &restore, streamed)?;
                        apply(&mut model, &restore, sequence);
                    }
                }
                emit(
                    json!({"phase":"latency","api":api,"kind":kind,"batch":n,"samples":results,"memory":memory()}),
                );
                model.save(state)?;
            }
        }
        db.maintain()?;
        model.save(state)?;
        verify_all(&db, &mut corpus, &model)?;
        return Ok(());
    }
    if mode == "delta" {
        db.maintain()?;
        emit(matrix(&db, &mut corpus, &model, 5, "idle")?);
        let changes = plan(&mut model, "mixed", 100, 991);
        emit(
            json!({"phase":"delta_commit","api":api,"result":measured_commit(&mut db,&mut corpus,&mut model,&changes,streamed)?}),
        );
        model.save(state)?;
        emit(matrix(&db, &mut corpus, &model, 5, "active")?);
        return Ok(());
    }
    if mode == "churn" || mode == "revisions-30" || mode == "revisions-10" {
        db.maintain()?;
        emit(
            json!({"phase":"churn_baseline","disk":disk(path)?,"stats":db.stats()?,"memory":memory(),"liveness":db.liveness()?}),
        );
        for round in 0..if mode == "revisions-30" { 30 } else { 10 } {
            reset_memory_sampler();
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
            let at = Instant::now();
            let mut attempts = Vec::new();
            for chunk in changes.chunks(100) {
                attempts.push(measured_commit(
                    &mut db,
                    &mut corpus,
                    &mut model,
                    chunk,
                    streamed,
                )?);
            }
            let write_ms = at.elapsed().as_secs_f64() * 1000.;
            let before = disk(path)?;
            let liveness_before = db.liveness()?;
            let at = Instant::now();
            let maintenance = db.maintain()?;
            let maintenance_ms = at.elapsed().as_secs_f64() * 1000.;
            verify_all(&db, &mut corpus, &model)?;
            emit(
                json!({"phase":"churn_round","round":round+1,"attempts":attempts,"write_including_oracle_ms":write_ms,"maintenance_ms":maintenance_ms,"maintenance":maintenance,"before":before,"after":disk(path)?,"stats":db.stats()?,"memory":memory(),"full_oracle":true,"liveness_before":liveness_before,"liveness_after":db.liveness()?}),
            );
            model.save(state)?;
            if mode.starts_with("revisions-") && [9, 29].contains(&round) {
                emit(matrix(
                    &db,
                    &mut corpus,
                    &model,
                    30,
                    &format!("round{}", round + 1),
                )?);
            }
        }
        return Ok(());
    }
    if mode == "burst" {
        db.maintain()?;
        let changes = plan(&mut model, "update", 5000, 123);
        let result = measured_commit(&mut db, &mut corpus, &mut model, &changes, streamed)?;
        emit(json!({"phase":"burst_atomic","result":result}));
        if model.rows.get(&changes[0].id).unwrap().revision
            == changes[0].old.as_ref().unwrap().revision
        {
            let at = Instant::now();
            for chunk in changes.chunks(100) {
                let result = measured_commit(&mut db, &mut corpus, &mut model, chunk, streamed)?;
                assert_eq!(result["ok"], true);
            }
            emit(
                json!({"phase":"burst_bounded_fallback","batch":100,"not_atomic_5000":true,"elapsed_including_oracle_ms":at.elapsed().as_secs_f64()*1000.,"memory":memory()}),
            );
        }
        model.save(state)?;
        emit(matrix(&db, &mut corpus, &model, 5, "burst_unmaintained")?);
        let at = Instant::now();
        db.maintain()?;
        emit(
            json!({"phase":"burst_maintenance","elapsed_ms":at.elapsed().as_secs_f64()*1000.,"disk":disk(path)?,"memory":memory()}),
        );
        verify_all(&db, &mut corpus, &model)?;
        model.save(state)?;
        return Ok(());
    }
    if mode == "dirty" {
        db.maintain()?;
        let changes = plan(&mut model, "mixed", 500, 777);
        let result = measured_commit(&mut db, &mut corpus, &mut model, &changes, false)?;
        assert_eq!(result["ok"], true);
        model.save(state)?;
        emit(
            json!({"phase":"dirty_closed","stats":db.stats()?,"memory":memory(),"disk":disk(path)?}),
        );
        return Ok(());
    }
    if mode == "concurrent-one" || mode == "concurrent-100" {
        db.maintain()?;
        emit(matrix(&db, &mut corpus, &model, 5, "concurrent_idle")?);
        let batch = if mode == "concurrent-one" { 1 } else { 100 };
        let baseline_started = Instant::now();
        let mut baseline_commits = Vec::new();
        for sample in 0..32 {
            let changes = plan(&mut model, "update", batch, sample + 117);
            let start = Instant::now();
            let sequence = db.commit(&mut corpus, &changes, streamed)?;
            baseline_commits.push(start.elapsed().as_secs_f64() * 1000.);
            apply(&mut model, &changes, sequence);
        }
        let baseline_elapsed_ms = baseline_started.elapsed().as_secs_f64() * 1000.;
        db.maintain()?;
        emit(
            json!({"phase":"writer_baseline","api":api,"batch":batch,"commits":baseline_commits,"elapsed_ms":baseline_elapsed_ms,"throughput_ops_s":32.*batch as f64*1000./baseline_elapsed_ms}),
        );
        let db = Arc::new(Mutex::new((db, model)));
        let finished = Arc::new(AtomicBool::new(false));
        let writer_stopped = Arc::new(AtomicBool::new(false));
        let stopped = writer_stopped.clone();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let writer_barrier = barrier.clone();
        let worker = db.clone();
        let done = finished.clone();
        let writer = std::thread::spawn(move || -> Result<Value> {
            let mut source = Corpus::open()?;
            let mut local_model = worker.lock().unwrap().1.clone();
            let mut rows = Vec::new();
            let mut merge_observed = false;
            let mut drained_prior_jobs = 0;
            let mut prior_jobs_scheduled = 0;
            let began = Instant::now();
            let mut count = 0;
            writer_barrier.wait();
            while !done.load(Ordering::Acquire) || count < if batch == 1 { 100 } else { 10 } {
                let changes = plan(&mut local_model, "update", batch, count + 991);
                let at = Instant::now();
                let mut locked = worker.lock().unwrap();
                let wait_ms = at.elapsed().as_secs_f64() * 1000.;
                let (db, model) = &mut *locked;
                let before_stats = db.stats()?;
                let commit_started = Instant::now();
                let sequence = db.commit(&mut source, &changes, streamed)?;
                let commit_ms = commit_started.elapsed().as_secs_f64() * 1000.;
                apply(model, &changes, sequence);
                let stats = db.stats()?;
                if before_stats["compaction_running"] == true && streamed {
                    drained_prior_jobs += 1;
                }
                prior_jobs_scheduled += usize::from(stats["compaction_running"] == true);
                merge_observed |= stats["compaction_running"] == true;
                rows.push(json!({"wait_ms":wait_ms,"commit_ms":commit_ms,"sequence":sequence,"before":before_stats,"after":stats,"elapsed_ms":began.elapsed().as_secs_f64()*1000.}));
                count += 1;
                drop(locked);
                apply(&mut local_model, &changes, sequence);
                std::thread::yield_now();
                if count >= if batch == 1 { 4000 } else { 200 } {
                    break;
                }
            }
            stopped.store(true, Ordering::Release);
            let commits_elapsed_ms = began.elapsed().as_secs_f64() * 1000.;
            let wait = Instant::now();
            let mut locked = worker.lock().unwrap();
            let maintenance_wait_ms = wait.elapsed().as_secs_f64() * 1000.;
            let at = Instant::now();
            let merge = locked.0.maintain()?;
            Ok(
                json!({"commits":count,"mutations":count*batch,"commits_elapsed_ms":commits_elapsed_ms,"elapsed_ms":began.elapsed().as_secs_f64()*1000.,"rows":rows,"merge_observed_running":merge_observed,"previous_jobs_drained_by_streamed_commits":drained_prior_jobs,"running_job_observations_after_commits":prior_jobs_scheduled,"final_maintenance_wait_ms":maintenance_wait_ms,"final_maintenance_ms":at.elapsed().as_secs_f64()*1000.,"maintenance":merge}),
            )
        });
        let mut records = Vec::new();
        let cases = corpus.index.cases.clone();
        barrier.wait();
        for round in 0..10 {
            for (case_id, case) in cases.iter().enumerate() {
                let at = Instant::now();
                let locked = db.lock().unwrap();
                let wait_ms = at.elapsed().as_secs_f64() * 1000.;
                let (engine, model) = &*locked;
                let start = Instant::now();
                let page = engine.search(&case.text, case.offset, case.limit)?;
                let engine_ms = start.elapsed().as_secs_f64() * 1000.;
                let total_ms = at.elapsed().as_secs_f64() * 1000.;
                let stats = engine.stats()?;
                // Copy only expected result descriptors while admitted. Full
                // corpus payload verification runs after releasing the lock.
                let oracle_started = Instant::now();
                let ids = model.expected_ids(&corpus, case);
                let total = ids.len();
                let expected: Vec<_> = ids
                    .into_iter()
                    .skip(case.offset)
                    .take(case.limit)
                    .map(|id| (id, model.rows[&id].clone()))
                    .collect();
                let oracle_admission_ms = oracle_started.elapsed().as_secs_f64() * 1000.;
                records.push(json!({"case_id":case_id,"round":round,"wait_ms":wait_ms,"engine_ms":engine_ms,"total_ms":total_ms,"compaction_running":stats["compaction_running"],"writer_finished":writer_stopped.load(Ordering::Acquire),"oracle_admission_ms":oracle_admission_ms}));
                drop(locked);
                assert_eq!(page.total, total);
                assert_eq!(page.hits.len(), expected.len());
                for (actual, (id, row)) in page.hits.iter().zip(expected) {
                    assert_eq!(
                        *actual,
                        StoredDocument {
                            document: corpus.document(id, &row)?,
                            version: row.version
                        }
                    );
                }
                std::thread::yield_now();
            }
        }
        finished.store(true, Ordering::Release);
        let writes = writer.join().map_err(|_| "writer panic")??;
        let locked = db.lock().unwrap();
        locked.1.save(state)?;
        verify_all(&locked.0, &mut corpus, &locked.1)?;
        emit(
            json!({"phase":"concurrent","api":api,"batch":batch,"read_records":records,"write":writes,"stats":locked.0.stats()?,"memory":memory(),"shared_external_mutex":true,"oracle":true}),
        );
        return Ok(());
    }
    if mode == "kill-iterator" {
        let changes = plan(&mut model, "mixed", 100, 337);
        fs::write(format!("{state}.pending"), serde_json::to_vec(&changes)?)?;
        emit(json!({"phase":"kill_iterator_ready","model":state}));
        return Err("backend-specific iterator child required".into());
    }
    Err(format!("unknown mode {mode}").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workload_mixture_unique_keys_markers_and_sizes() {
        let mut model = Model {
            next: 10000,
            ..Model::default()
        };
        for id in 0..10000 {
            model.rows.insert(
                id,
                Row {
                    body: id,
                    revision: 0,
                    version: 20,
                },
            );
        }
        for size in [1, 10, 100, 1000, 5000] {
            let changes = plan(&mut model, "mixed", size, 3);
            let unique: std::collections::BTreeSet<_> = changes.iter().map(|c| c.id).collect();
            assert_eq!(unique.len(), size);
            if size % 4 == 0 {
                assert_eq!(
                    changes.iter().filter(|c| c.kind == "update").count(),
                    size / 2
                );
                assert_eq!(
                    changes.iter().filter(|c| c.kind == "delete").count(),
                    size / 4
                );
                assert_eq!(
                    changes.iter().filter(|c| c.kind == "insert").count(),
                    size / 4
                );
            }
        }
        assert!(!marker(1, 1).contains(&marker(1, 0)));
        assert!(!marker(11, 1).contains(&marker(1, 1)));
        assert_eq!(cases().len(), 38);
        assert_eq!(
            fold("IŞIK ışık isik ŞARTNAME şartname Sartname SARTNAME"),
            "isik isik isik sartname sartname sartname sartname"
        );
    }
}
