use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Scope is mandatory: use `shared` for shared data and `user:<id>` for private data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub scope: String,
    pub collection: String,
    pub id: String,
}

impl Key {
    pub fn new(scope: &str, collection: &str, id: &str) -> Self {
        Self {
            scope: scope.into(),
            collection: collection.into(),
            id: id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub key: Key,
    #[serde(default)]
    pub title: String,
    /// Extracted UTF-8 text, not raw PDF/DOCX bytes.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub fields: BTreeMap<String, Value>,
    /// Same-scope references; targets may be ingested later.
    #[serde(default)]
    pub links: Vec<Key>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredDocument {
    pub document: Document,
    pub version: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Guard {
    #[default]
    Any,
    Missing,
    Version {
        version: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    Put {
        document: Document,
        #[serde(default)]
        guard: Guard,
    },
    Delete {
        key: Key,
        #[serde(default)]
        guard: Guard,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub field: String,
    pub min: i64,
    pub max: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub scope: String,
    #[serde(default)]
    pub collection: Option<String>,
    /// AND substring search over title and extracted text.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub exact: BTreeMap<String, Value>,
    #[serde(default)]
    pub ranges: Vec<Range>,
    /// Find documents pointing to this key.
    #[serde(default)]
    pub linked_to: Option<Key>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

impl Query {
    pub fn new(scope: &str) -> Self {
        Self {
            scope: scope.into(),
            collection: None,
            text: String::new(),
            exact: BTreeMap::new(),
            ranges: vec![],
            linked_to: None,
            offset: 0,
            limit: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPage {
    pub total: usize,
    pub sequence: u64,
    pub hits: Vec<StoredDocument>,
}

/// Exact when exhausted; otherwise a proved lower bound, never an estimate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchTotal {
    Exact(usize),
    AtLeast(usize),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Verified,
    Pending,
    RebuildRequired,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstPage {
    pub total: MatchTotal,
    pub sequence: u64,
    pub hits: Vec<StoredDocument>,
    pub validation: ValidationStatus,
}
/// Opt-in, query-local experiments. Ordinary search retains checked serial execution.
#[derive(Debug, Clone, Copy)]
pub struct SearchExecution {
    pub verification_workers: usize,
    /// Join only candidate ranges separated by at most this many bytes (0 disables).
    pub coalesce_gap_bytes: usize,
    /// Candidate CRC may be reused ONLY for validated, immutable, pinned native sources.
    /// Returned documents still receive their ordinary CRC/schema/identity checks.
    pub reuse_validated_source_crc: bool,
}
impl Default for SearchExecution {
    fn default() -> Self {
        Self {
            verification_workers: 1,
            coalesce_gap_bytes: 0,
            reuse_validated_source_crc: false,
        }
    }
}
/// Successfully unlinked engine-owned obsolete files. This is logical file
/// space, not a measurement of physical disk blocks or total process memory.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ReclaimStats {
    pub removed_files: usize,
    pub reclaimed_bytes: u64,
}
