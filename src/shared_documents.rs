//! Resident payload sharing. Public documents retain their exact owned API shape.
use crate::{Document, Key, Result, StoredDocument, format};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};

const BUDGET: usize = 128 * 1024;
const ENTRIES: usize = 256;
type Fields = BTreeMap<String, Value>;
#[derive(Clone)]
enum Body {
    Owned(String),
    Shared(Arc<str>),
}
#[derive(Clone)]
enum FieldSet {
    Owned(Fields),
    Shared(Arc<Fields>),
}
#[derive(Clone)]
pub(crate) struct Row {
    pub version: u64,
    pub serialized_len: usize,
    title: String,
    body: Body,
    fields: FieldSet,
    links: Vec<Key>,
}
impl Row {
    pub fn to_stored(&self, key: &Key) -> StoredDocument {
        StoredDocument {
            version: self.version,
            document: Document {
                key: key.clone(),
                title: self.title.clone(),
                text: match &self.body {
                    Body::Owned(text) => text.clone(),
                    Body::Shared(text) => text.as_ref().to_owned(),
                },
                fields: match &self.fields {
                    FieldSet::Owned(fields) => fields.clone(),
                    FieldSet::Shared(fields) => fields.as_ref().clone(),
                },
                links: self.links.clone(),
            },
        }
    }
    pub fn into_stored(self, key: Key) -> StoredDocument {
        StoredDocument {
            version: self.version,
            document: Document {
                key,
                title: self.title,
                text: match self.body {
                    Body::Owned(text) => text,
                    Body::Shared(text) => text.as_ref().to_owned(),
                },
                fields: match self.fields {
                    FieldSet::Owned(fields) => fields,
                    FieldSet::Shared(fields) => fields.as_ref().clone(),
                },
                links: self.links,
            },
        }
    }
    pub fn exclusive_text_bytes(&self) -> usize {
        self.title.len()
            + match &self.body {
                Body::Owned(text) => text.len(),
                Body::Shared(_) => 0,
            }
    }
}
struct FieldEntry {
    canonical: Vec<u8>,
    fields: Arc<Fields>,
}
#[derive(Default)]
pub(crate) struct Cache {
    bodies: BTreeMap<u32, Vec<Arc<str>>>,
    fields: BTreeMap<u32, Vec<FieldEntry>>,
    body_budget: usize,
    field_budget: usize,
    body_entries: usize,
    field_entries: usize,
    pub text_bytes: usize,
}
impl Cache {
    fn body(&mut self, text: String) -> Body {
        if text.is_empty() || text.len() > 4096 {
            return Body::Owned(text);
        }
        let hash = format::crc(text.as_bytes());
        self.body_hashed(text, hash)
    }
    fn body_hashed(&mut self, text: String, hash: u32) -> Body {
        if let Some(hit) = self
            .bodies
            .get(&hash)
            .and_then(|entries| entries.iter().find(|value| value.as_ref() == text))
        {
            return Body::Shared(hit.clone());
        }
        let weight = text.len() + 128;
        if self.body_entries == ENTRIES || weight > BUDGET - self.body_budget {
            return Body::Owned(text);
        }
        let shared: Arc<str> = Arc::from(text);
        self.body_budget += weight;
        self.body_entries += 1;
        self.text_bytes += shared.len();
        self.bodies.entry(hash).or_default().push(shared.clone());
        Body::Shared(shared)
    }
    fn fields(&mut self, fields: Fields) -> Result<FieldSet> {
        if fields.is_empty()
            || fields.len() > 16
            || fields.iter().any(|(key, value)| {
                value.is_array()
                    || value.is_object()
                    || key.len() + value.as_str().map_or(0, str::len) > 2048
            })
        {
            return Ok(FieldSet::Owned(fields));
        }
        let minimum = fields
            .iter()
            .map(|(key, value)| key.len() + value.as_str().map_or(0, str::len))
            .sum::<usize>()
            * 2
            + fields.len() * 160
            + 256;
        if minimum > BUDGET {
            return Ok(FieldSet::Owned(fields));
        }
        let canonical = serde_json::to_vec(&fields)?;
        let hash = format::crc(&canonical);
        Ok(self.fields_hashed(fields, canonical, hash))
    }
    fn fields_hashed(&mut self, fields: Fields, canonical: Vec<u8>, hash: u32) -> FieldSet {
        if let Some(hit) = self
            .fields
            .get(&hash)
            .and_then(|entries| entries.iter().find(|entry| entry.canonical == canonical))
        {
            return FieldSet::Shared(hit.fields.clone());
        }
        let weight = canonical.len() * 2 + fields.len() * 160 + 256;
        if self.field_entries == ENTRIES || weight > BUDGET - self.field_budget {
            return FieldSet::Owned(fields);
        }
        let fields = Arc::new(fields);
        self.fields.entry(hash).or_default().push(FieldEntry {
            canonical,
            fields: fields.clone(),
        });
        self.field_budget += weight;
        self.field_entries += 1;
        FieldSet::Shared(fields)
    }
    pub fn store(&mut self, stored: StoredDocument, serialized_len: usize) -> Result<(Key, Row)> {
        let Document {
            key,
            title,
            text,
            fields,
            links,
        } = stored.document;
        Ok((
            key,
            Row {
                version: stored.version,
                serialized_len,
                title,
                body: self.body(text),
                fields: self.fields(fields)?,
                links,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stored(text: &str, n: u64) -> StoredDocument {
        StoredDocument {
            version: n,
            document: Document {
                key: Key::new("shared", "collection", "ü"),
                title: format!("şirket {n}"),
                text: text.into(),
                fields: [
                    ("scalar".into(), serde_json::json!(n % 3)),
                    ("source".into(), serde_json::json!("same")),
                ]
                .into(),
                links: vec![Key::new("shared", "collection", "linked")],
            },
        }
    }
    #[test]
    fn exact_payloads_survive_sharing_updates_and_cache_release() {
        let mut cache = Cache::default();
        let mut rows = Vec::new();
        for n in 1..=1000 {
            let expected = stored("ŞİRKET \"ü\"\\\n🦀", n);
            let size = serde_json::to_vec(&expected).unwrap().len();
            let (key, row) = cache.store(expected.clone(), size).unwrap();
            assert_eq!(row.to_stored(&key), expected);
            let mut returned = row.to_stored(&key);
            returned.document.text.clear();
            returned.document.fields.clear();
            returned.document.links.clear();
            assert_eq!(row.to_stored(&key), expected);
            assert_eq!(row.serialized_len, size);
            rows.push((key, row, expected));
        }
        assert_eq!(cache.body_entries, 1);
        assert_eq!(cache.field_entries, 3);
        drop(cache);
        for (key, row, expected) in rows {
            assert_eq!(row.into_stored(key), expected);
        }
    }
    #[test]
    fn collisions_limits_and_uncached_complex_payloads_preserve_values() {
        let mut cache = Cache::default();
        for text in ["a", "b", "a"] {
            let body = cache.body_hashed(text.into(), 42);
            match body {
                Body::Shared(value) => assert_eq!(value.as_ref(), text),
                _ => panic!("small body not cached"),
            }
        }
        assert_eq!(cache.body_entries, 2);
        for value in [1, 2, 1] {
            let fields: Fields = [("v".into(), serde_json::json!(value))].into();
            let bytes = serde_json::to_vec(&fields).unwrap();
            match cache.fields_hashed(fields.clone(), bytes, 42) {
                FieldSet::Shared(actual) => assert_eq!(actual.as_ref(), &fields),
                _ => panic!("small fields not cached"),
            }
        }
        assert_eq!(cache.field_entries, 2);
        for n in 0..1000 {
            let mut expected = stored(&format!("body-{n}"), n);
            expected
                .document
                .fields
                .insert("unique".into(), serde_json::json!(n));
            let size = serde_json::to_vec(&expected).unwrap().len();
            let (key, row) = cache.store(expected.clone(), size).unwrap();
            assert_eq!(row.to_stored(&key), expected);
        }
        assert!(cache.body_entries <= ENTRIES && cache.field_entries <= ENTRIES);
        assert!(cache.body_budget <= BUDGET && cache.field_budget <= BUDGET);
        let mut expected = stored(&"large unique".repeat(1000), 1);
        expected
            .document
            .fields
            .insert("nested".into(), serde_json::json!({"array":[1,{"a":"ü"}]}));
        let (key, row) = cache.store(expected.clone(), 123).unwrap();
        assert!(matches!(row.body, Body::Owned(_)));
        assert!(matches!(row.fields, FieldSet::Owned(_)));
        assert_eq!(row.into_stored(key), expected);
    }
}
