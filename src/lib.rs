//! Standalone FDB engine: transactional WAL and immutable indexed segments.
pub mod adapters;
mod bounded;
mod engine;
mod format;
mod index;
#[cfg(feature = "experimental-streaming-open")]
mod json_stream;
mod manifest;
#[cfg(feature = "diagnostic-memory")]
pub mod memory;
pub mod model;
mod query;
mod segment;
#[cfg(all(
    feature = "experimental-shared-documents",
    not(feature = "experimental-wal-delta")
))]
mod shared_documents;
mod spill;
#[cfg(feature = "diagnostic-timing")]
pub mod timing;
#[cfg(feature = "experimental-wal-delta")]
mod wal_delta;
#[cfg(feature = "experimental-write-buffer-budget")]
pub mod write_budget;
pub use engine::Database;
pub use model::*;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(String),
    Conflict(Key),
    Corrupt(String),
    Locked,
    Poisoned,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O: {e}"),
            Self::Json(e) => write!(f, "JSON: {e}"),
            Self::Invalid(e) => write!(f, "invalid request: {e}"),
            Self::Conflict(k) => {
                write!(f, "version conflict: {}/{}/{}", k.scope, k.collection, k.id)
            }
            Self::Corrupt(e) => write!(f, "corrupt database: {e}"),
            Self::Locked => write!(f, "database is already open in another engine"),
            Self::Poisoned => write!(
                f,
                "I/O failure: close and reopen database before continuing"
            ),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

#[derive(serde::Serialize)]
pub struct Stats {
    pub documents: usize,
    pub sequence: u64,
    pub generation: u64,
    pub log_bytes: u64,
    pub disk_index_bytes: u64,
    pub disk_posting_entries: u64,
    pub resident_documents: usize,
    pub resident_text_bytes: usize,
    pub delta_keys: usize,
    pub segments: usize,
    pub compaction_running: bool,
    pub source_bytes: u64,
}

/// Selected index allocation capacities for diagnostics. Not RSS or a total
/// heap estimate: BTreeMap nodes, allocator overhead, delta and transient work
/// are deliberately not included. Querying this does not change cache state.
#[derive(serde::Serialize)]
pub struct IndexMemoryLayout {
    pub source_id: u64,
    pub records: usize,
    pub record_vector_capacity_bytes: usize,
    pub id_pool_capacity_bytes: usize,
    pub group_string_capacity_bytes: usize,
    #[cfg(feature = "experimental-paged-metadata")]
    pub paged_root_capacity_bytes: usize,
    #[cfg(feature = "experimental-paged-metadata")]
    pub paged_deletion_capacity_bytes: usize,
    pub terms: usize,
    pub term_string_capacity_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct Options {
    /// Maintenance happens before the next validated write after this watermark.
    pub max_delta_keys: usize,
    /// Serialized live delta size, excluding the caller's batch and build memory.
    pub max_delta_bytes: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            max_delta_keys: 10_000,
            max_delta_bytes: 16 * 1024 * 1024,
        }
    }
}

fn validate_key(key: &Key) -> Result<()> {
    for s in [&key.scope, &key.collection, &key.id] {
        if s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control) {
            return Err(Error::Invalid(
                "key components must be 1..1024 bytes without controls".into(),
            ));
        }
    }
    Ok(())
}
fn validate_doc(doc: &Document) -> Result<()> {
    validate_key(&doc.key)?;
    if doc.title.len() + doc.text.len() > 2 * 1024 * 1024
        || doc.fields.len() > 256
        || doc.links.len() > 1024
    {
        return Err(Error::Invalid(
            "document exceeds text/field/link limits".into(),
        ));
    }
    if doc.fields.keys().any(|s| s.is_empty() || s.len() > 256) {
        return Err(Error::Invalid("invalid field name".into()));
    }
    for link in &doc.links {
        validate_key(link)?;
        if link.scope != doc.key.scope {
            return Err(Error::Invalid(
                "links must stay within the same scope".into(),
            ));
        }
    }
    if serde_json::to_vec(doc)?.len() > 4 * 1024 * 1024 {
        return Err(Error::Invalid("serialized document exceeds 4 MiB".into()));
    }
    Ok(())
}
