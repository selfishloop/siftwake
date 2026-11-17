//! Authorized repair only: guarded deletes of exactly 16 keys; no maintenance.
use super::churn::{self, Corpus, Model, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use siftwake::{Document, FirstPage, Key, MatchTotal, Query, StoredDocument};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
};
pub const COLLECTION: &str = "keybench-numeric";
pub trait Store {
    fn sequence(&mut self) -> Result<u64>;
    fn count(&mut self) -> Result<usize>;
    fn get(&mut self, key: &Key) -> Result<Option<StoredDocument>>;
    fn first(&mut self, query: &Query) -> Result<FirstPage>;
    fn delete_expected(&mut self, expected: &[StoredDocument]) -> Result<u64>;
}
#[derive(Deserialize)]
struct Entry {
    offset: u64,
    size: usize,
    crc: u32,
}
#[derive(Deserialize)]
struct Config {
    engine: String,
    state: String,
    donors: String,
    setup_sequence: u64,
    expected_versions: Option<Vec<u64>>,
    before_delete: String,
}
pub fn key(slot: usize) -> Key {
    assert!(slot < 16);
    Key::new("shared", COLLECTION, &format!("{slot:010}"))
}
fn document(template: &Document, slot: usize, revision: usize) -> Document {
    let mut out = template.clone();
    out.key = key(slot);
    out.text
        .push_str(&format!(" zqdiag{slot:04}r{revision:08}zq"));
    out
}
fn query(collection: &str, text: &str) -> Query {
    let mut q = Query::new("shared");
    q.collection = Some(collection.into());
    q.text = text.into();
    q.limit = 50;
    q
}
fn crc<T: serde::Serialize>(value: &T) -> Result<u32> {
    Ok(crc32fast::hash(&serde_json::to_vec(value)?))
}
fn pool_absent(db: &mut impl Store) -> Result<()> {
    for slot in 0..16 {
        assert!(db.get(&key(slot))?.is_none(), "deleted key resurfaced");
    }
    let p = db.first(&query(COLLECTION, "zqdiag"))?;
    assert!(p.hits.is_empty());
    assert_eq!(p.total, MatchTotal::Exact(0));
    Ok(())
}
fn delete(db: &mut impl Store, cfg: &Config) -> Result<()> {
    let mut model = Model::load(&cfg.state)?;
    assert_eq!(model.rows.len(), 50000);
    assert_eq!(db.count()?, 50016, "unexpected documents; do not delete");
    let sequence = db.sequence()?;
    let entries: Vec<Entry> = serde_json::from_slice(&fs::read(&cfg.donors)?)?;
    assert_eq!(entries.len(), 16);
    let mut corpus = File::open(std::env::var("FDB_BENCH_CORPUS_PATH")?)?;
    let mut observed = vec![];
    let mut revisions = vec![];
    for (slot, e) in entries.iter().enumerate() {
        corpus.seek(SeekFrom::Start(e.offset))?;
        let mut bytes = vec![0; e.size];
        corpus.read_exact(&mut bytes)?;
        assert_eq!(crc32fast::hash(&bytes), e.crc);
        let template: StoredDocument = serde_json::from_slice(&bytes)?;
        let actual = db.get(&key(slot))?.ok_or("expected repair key missing")?;
        let revision = if actual.document == document(&template.document, slot, 1) {
            1
        } else if actual.document == document(&template.document, slot, 2) {
            2
        } else {
            return Err(format!("unexpected content at slot {slot}; nothing deleted").into());
        };
        assert!(actual.version >= cfg.setup_sequence && actual.version <= sequence);
        if let Some(versions) = &cfg.expected_versions {
            assert_eq!(versions.len(), 16);
            assert_eq!(actual.version, versions[slot]);
        }
        revisions.push(revision);
        observed.push(actual);
    }
    let evidence = json!({"phase":"before_delete","engine":cfg.engine,"sequence":sequence,
      "setup_sequence":cfg.setup_sequence,"commits_since_setup":sequence.checked_sub(cfg.setup_sequence),
      "revisions":revisions,"records":observed,"all_16_get_content_version_verified":true});
    // Persist all GET evidence before any mutation; never overwrite a prior repair.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&cfg.before_delete)?;
    file.write_all(&serde_json::to_vec_pretty(&evidence)?)?;
    file.sync_all()?;
    churn::emit(evidence);
    let committed = db.delete_expected(&observed)?;
    assert_eq!(db.sequence()?, committed);
    assert_eq!(db.count()?, 50000);
    pool_absent(db)?;
    // Preserve every modeled row/version; only the global sequence advances.
    model.sequence = committed;
    model.save(&cfg.state)?;
    churn::emit(
        json!({"phase":"deleted","engine":cfg.engine,"keys":observed.iter().map(|d|&d.document.key).collect::<Vec<_>>(),"deleted":16,"sequence":committed,"pool_absent":true,"maintenance_calls":0,"model_rows_unchanged":true}),
    );
    Ok(())
}
fn verify(db: &mut impl Store, cfg: &Config) -> Result<()> {
    let model = Model::load(&cfg.state)?;
    assert_eq!(model.rows.len(), 50000);
    let sequence = db.sequence()?;
    assert_eq!(sequence, model.sequence);
    assert_eq!(db.count()?, 50000);
    pool_absent(db)?;
    let mut corpus = Corpus::open()?;
    assert_eq!(corpus.index.cases.len(), 41);
    assert!(
        corpus
            .index
            .cases
            .iter()
            .all(|c| c.offset == 0 && c.limit == 50)
    );
    let terms: Vec<Vec<String>> = corpus
        .index
        .cases
        .iter()
        .map(|c| c.text.split_whitespace().map(churn::fold).collect())
        .collect();
    let mut df = [0usize; 41];
    let mut first: Vec<Vec<usize>> = vec![vec![]; 41];
    for (&id, row) in &model.rows {
        let expected = StoredDocument {
            document: corpus.document(id, row)?,
            version: row.version,
        };
        assert_eq!(
            db.get(&churn::key(id))?,
            Some(expected.clone()),
            "main GET/version {id}"
        );
        let folded = churn::fold(&format!(
            "{}\n{}",
            expected.document.title, expected.document.text
        ));
        for (i, terms) in terms.iter().enumerate() {
            if terms.iter().all(|t| folded.contains(t)) {
                df[i] += 1;
                if first[i].len() < 50 {
                    first[i].push(id);
                }
            }
        }
    }
    for &id in model.dead.keys() {
        assert!(db.get(&churn::key(id))?.is_none());
    }
    churn::emit(
        json!({"phase":"main_get_oracle","engine":cfg.engine,"sequence":sequence,"documents":50000,"dead_keys_checked":model.dead.len(),"oracle":true}),
    );
    for i in 0..41 {
        let text = corpus.index.cases[i].text.clone();
        let page = db.first(&query("texts", &text))?;
        assert_eq!(page.sequence, sequence);
        let wanted: Vec<StoredDocument> = first[i]
            .iter()
            .map(|id| {
                Ok(StoredDocument {
                    document: corpus.document(*id, &model.rows[id])?,
                    version: model.rows[id].version,
                })
            })
            .collect::<Result<_>>()?;
        assert_eq!(page.hits, wanted, "case {i}: {text}");
        match page.total {
            MatchTotal::Exact(n) => assert_eq!(n, df[i]),
            MatchTotal::AtLeast(n) => assert!(n == 50 && df[i] >= n),
        }
        let wanted_crc = crc(&wanted)?;
        let actual_crc = crc(&page.hits)?;
        assert_eq!(actual_crc, wanted_crc);
        churn::emit(
            json!({"phase":"case_digest","case":i,"text":text,"offset":0,"limit":50,"oracle_df":df[i],"returned":page.hits.len(),"actual_crc32":actual_crc,"expected_crc32":wanted_crc,"hit_keys_versions":page.hits.iter().map(|d|json!([d.document.key.id,d.version])).collect::<Vec<Value>>(),"oracle":true}),
        );
    }
    assert_eq!(db.sequence()?, sequence);
    pool_absent(db)?;
    churn::emit(
        json!({"phase":"verified","engine":cfg.engine,"sequence":sequence,"main_gets":50000,"cases":41,"pool_absent":true,"oracle":true,"maintenance_calls":0}),
    );
    Ok(())
}
pub fn run(db: &mut impl Store, mode: &str, config: &str) -> Result<()> {
    let cfg: Config = serde_json::from_slice(&fs::read(config)?)?;
    match mode {
        "delete" => delete(db, &cfg),
        "verify" | "hold" => {
            verify(db, &cfg)?;
            if mode == "hold" {
                churn::emit(
                    json!({"phase":"kill_ready","engine":cfg.engine,"sequence":db.sequence()?,"oracle_completed":true}),
                );
                loop {
                    std::thread::park();
                }
            }
            Ok(())
        }
        _ => Err("repair mode must be delete, verify or hold".into()),
    }
}
