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
