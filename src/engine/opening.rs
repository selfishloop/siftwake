//! Opt-in background opening, including source checks, SEG05 parsing and index
//! CRC. This conservative prototype waits for ALL strict checks before results.
use super::*;
use crate::{FirstPage, SearchExecution, ValidationStatus};
use std::sync::{Condvar, Mutex};
use std::time::Instant;

#[derive(Debug)]
pub enum OpeningFailure {
    RebuildRequired(String),
    Query(Error),
    Consumed,
}
impl OpeningFailure {
    pub fn code(&self) -> &'static str {
        match self {
            Self::RebuildRequired(_) => "ERR_REBUILD",
            Self::Query(_) => "QUERY_ERROR",
            Self::Consumed => "INVALID_STATE",
        }
    }
}
impl std::fmt::Display for OpeningFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RebuildRequired(s) => write!(f, "ERR_REBUILD: {s}"),
            Self::Query(e) => write!(f, "{e}"),
            Self::Consumed => write!(f, "opening session already consumed"),
        }
    }
}
impl std::error::Error for OpeningFailure {}
enum Outcome {
    Pending,
    Ready(Box<Database>),
    Failed(String),
    Taken,
}
struct State {
    outcome: Outcome,
    elapsed_ms: Option<f64>,
}
type Shared = Arc<(Mutex<State>, Condvar)>;

/// Clean MAN03/native sources with existing caches only. Pending sessions have
/// no write API. `try_finish` transfers the SAME lock, leases and Database.
pub struct OpeningDatabase {
    shared: Shared,
    worker: Option<JoinHandle<()>>,
}
impl OpeningDatabase {
    pub fn open_for_search(dir: impl AsRef<Path>) -> Result<Self> {
        let started = Instant::now();
        let dir = dir.as_ref().canonicalize()?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("LOCK"))?;
        lock.try_lock().map_err(|e| match e {
            std::fs::TryLockError::WouldBlock => Error::Locked,
            std::fs::TryLockError::Error(e) => Error::Io(e),
        })?;
        let (latest, _) = manifest::inventory(&dir)?;
        let publication = manifest::read(
            &dir,
            latest.ok_or_else(|| {
                Error::Invalid("background opening requires existing MAN03".into())
            })?,
        )?;
        let Publication::Current(m) = publication else {
            return Err(Error::Invalid("legacy source requires strict open".into()));
        };
        if m.active.end != 45 || fs::metadata(manifest::path(&dir, m.active.id, "fdb"))?.len() != 45
        {
            return Err(Error::Invalid(
                "background opening prototype requires a clean active WAL".into(),
            ));
        }
        let mut pins = Vec::new();
        for source in &m.runs {
            if source.kind != Kind::Run {
                return Err(Error::Invalid(
                    "background opening requires native immutable runs".into(),
                ));
            }
            let file = crate::segment::SourceLease::immutable(&source.path(&dir, "fdb"))?;
            let mut magic = [0; 8];
            crate::segment::read_at(&file, 0, &mut magic)?;
            if &magic != format::MAGIC_V4 || file.metadata()?.len() != source.end {
                return Err(Error::Corrupt("background source header/length".into()));
            }
            pins.push(file);
        }
        let shared = Arc::new((
            Mutex::new(State {
                outcome: Outcome::Pending,
                elapsed_ms: None,
            }),
            Condvar::new(),
        ));
        let worker_state = shared.clone();
        let worker = std::thread::Builder::new()
            .name("fdb-open".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let permits = super::first::Permits::acquire(4)?;
                    let db = Database::open_policy(
                        &dir,
                        Options::default(),
                        false,
                        false,
                        permits.0,
                        Some(lock),
                    );
                    drop(permits);
                    db
                }));
                drop(pins);
                let outcome = match result {
                    Ok(Ok(db)) => Outcome::Ready(Box::new(db)),
                    Ok(Err(e)) => Outcome::Failed(e.to_string()),
                    Err(_) => Outcome::Failed("background validation panicked".into()),
                };
                let (state, ready) = &*worker_state;
                if let Ok(mut state) = state.lock() {
                    state.outcome = outcome;
                    state.elapsed_ms = Some(started.elapsed().as_secs_f64() * 1000.);
                    ready.notify_all();
                }
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }
    pub fn validation_status(&self) -> ValidationStatus {
        match self.shared.0.lock() {
            Ok(s) => match s.outcome {
                Outcome::Pending => ValidationStatus::Pending,
                Outcome::Ready(_) | Outcome::Taken => ValidationStatus::Verified,
                Outcome::Failed(_) => ValidationStatus::RebuildRequired,
            },
            Err(_) => ValidationStatus::RebuildRequired,
        }
    }
    pub fn validation_elapsed_ms(&self) -> Option<f64> {
        self.shared.0.lock().ok().and_then(|s| s.elapsed_ms)
    }
    pub fn search_first(&self, q: &Query) -> std::result::Result<FirstPage, OpeningFailure> {
        self.search_first_with_execution(
            q,
            SearchExecution {
                verification_workers: 4,
                ..Default::default()
            },
        )
    }
    pub fn search_first_with_execution(
        &self,
        q: &Query,
        e: SearchExecution,
    ) -> std::result::Result<FirstPage, OpeningFailure> {
        let (mutex, ready) = &*self.shared;
        let mut state = mutex
            .lock()
            .map_err(|_| OpeningFailure::RebuildRequired("opening mutex poisoned".into()))?;
        while matches!(state.outcome, Outcome::Pending) {
            state = ready
                .wait(state)
                .map_err(|_| OpeningFailure::RebuildRequired("opening wait poisoned".into()))?;
        }
        let result = match &state.outcome {
            Outcome::Ready(db) => db.search_first_with_execution(q, e),
            Outcome::Failed(s) => return Err(OpeningFailure::RebuildRequired(s.clone())),
            Outcome::Taken => return Err(OpeningFailure::Consumed),
            Outcome::Pending => unreachable!(),
        };
        match result {
            Ok(p) => Ok(p),
            Err(e @ (Error::Corrupt(_) | Error::Io(_) | Error::Poisoned)) => {
                let text = e.to_string();
                state.outcome = Outcome::Failed(text.clone());
                Err(OpeningFailure::RebuildRequired(text))
            }
            Err(e) => Err(OpeningFailure::Query(e)),
        }
    }
    pub fn try_finish(&mut self) -> std::result::Result<Option<Database>, OpeningFailure> {
        let mut state = self
            .shared
            .0
            .lock()
            .map_err(|_| OpeningFailure::RebuildRequired("opening mutex poisoned".into()))?;
        match &state.outcome {
            Outcome::Pending => Ok(None),
            Outcome::Failed(s) => Err(OpeningFailure::RebuildRequired(s.clone())),
            Outcome::Taken => Err(OpeningFailure::Consumed),
            Outcome::Ready(_) => {
                let Outcome::Ready(db) = std::mem::replace(&mut state.outcome, Outcome::Taken)
                else {
                    unreachable!()
                };
                Ok(Some(*db))
            }
        }
    }
}
impl Drop for OpeningDatabase {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "fdb-opening-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn seed(&self) {
            let mut db = Database::open(&self.0).unwrap();
            for i in 0..3 {
                db.commit(vec![Mutation::Put {
                    document: crate::Document {
                        key: Key::new("s", "c", &i.to_string()),
                        title: String::new(),
                        text: "needle company".into(),
                        fields: Default::default(),
                        links: vec![],
                    },
                    guard: Guard::Any,
                }])
                .unwrap();
                db.flush().unwrap();
            }
            db.maintain_idle().unwrap();
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn query() -> Query {
        let mut q = Query::new("s");
        q.text = "needle".into();
        q
    }
    #[test]
    fn background_checked_open_transfers_same_lock_and_preserves_search_snapshot() {
        let t = Temp::new();
        t.seed();
        let mut session = OpeningDatabase::open_for_search(&t.0).unwrap();
        assert!(matches!(Database::open(&t.0), Err(Error::Locked)));
        let page = session.search_first(&query()).unwrap();
        assert_eq!(page.hits.len(), 3);
        assert_eq!(page.validation, ValidationStatus::Verified);
        assert_eq!(session.validation_status(), ValidationStatus::Verified);
        assert!(session.validation_elapsed_ms().is_some());
        let mut db = session.try_finish().unwrap().unwrap();
        assert_eq!(db.search(&query()).unwrap().hits, page.hits);
        let reader = db.snapshot_reader().unwrap();
        assert_eq!(
            reader
                .acquire()
                .unwrap()
                .search_first(&query())
                .unwrap()
                .hits,
            page.hits
        );
        assert!(matches!(
            session.try_finish(),
            Err(OpeningFailure::Consumed)
        ));
        drop(db);
        drop(reader);
        drop(session);
        assert!(Database::open(&t.0).is_ok());
    }
    #[test]
    fn background_source_cuts_single_byte_and_cache_failures_never_repair_source() {
        let t = Temp::new();
        t.seed();
        let source = fs::read_dir(&t.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.extension().is_some_and(|x| x == "fdb")
                    && p.file_name().unwrap().to_string_lossy().starts_with("run-")
            })
            .unwrap();
        let original = fs::read(&source).unwrap();
        for cut in [0, 7, original.len() - 1] {
            fs::write(&source, &original[..cut]).unwrap();
            assert!(OpeningDatabase::open_for_search(&t.0).is_err());
            assert_eq!(fs::read(&source).unwrap(), original[..cut]);
        }
        let mut bad = original.clone();
        bad[32] ^= 1;
        fs::write(&source, &bad).unwrap();
        let session = OpeningDatabase::open_for_search(&t.0).unwrap();
        assert_eq!(
            session.search_first(&query()).unwrap_err().code(),
            "ERR_REBUILD"
        );
        assert_eq!(
            session.validation_status(),
            ValidationStatus::RebuildRequired
        );
        assert_eq!(
            session.search_first(&query()).unwrap_err().code(),
            "ERR_REBUILD"
        );
        drop(session);
        assert_eq!(fs::read(&source).unwrap(), bad);
        fs::write(&source, &original).unwrap();
        for path in [
            source.with_extension("idx"),
            source.with_extension("records04.idx"),
        ] {
            let bytes = fs::read(&path).unwrap();
            fs::remove_file(&path).unwrap();
            let session = OpeningDatabase::open_for_search(&t.0).unwrap();
            assert_eq!(
                session.search_first(&query()).unwrap_err().code(),
                "ERR_REBUILD"
            );
            drop(session);
            assert!(!path.exists());
            assert_eq!(fs::read(&source).unwrap(), original);
            fs::write(&path, bytes).unwrap();
        }
        let session = OpeningDatabase::open_for_search(&t.0).unwrap();
        let mut invalid = query();
        invalid.offset = 1;
        assert_eq!(
            session.search_first(&invalid).unwrap_err().code(),
            "QUERY_ERROR"
        );
        assert_eq!(session.validation_status(), ValidationStatus::Verified);
        assert_eq!(session.search_first(&query()).unwrap().hits.len(), 3);
    }
}
