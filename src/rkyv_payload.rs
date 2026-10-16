//! Isolated payload experiment. No engine/WAL/segment reader calls this module.
pub mod bench;
pub mod binary;
use crate::{Document, Error, Key, Result, StoredDocument};
use rkyv::{Archive, Serialize, rancor::Error as ArchiveError, util::AlignedVec};
use serde_json::{Number, Value};

#[derive(Archive, Serialize)]
struct PayloadKey {
    scope: String,
    collection: String,
    id: String,
}

// Flat postorder nodes avoid recursive bytecheck bounds and cyclic graphs.
// The i64 variant keeps an unsigned tag so JSON u64::MAX also round-trips
// exactly, without rounding through f64 or disguising a number as a string.
#[derive(Archive, Serialize)]
enum FieldValue {
    Null,
    Bool(bool),
    I64 { bits: i64, unsigned: bool },
    F64(f64),
    String(String),
    List(Vec<u32>),
    Map(Vec<(String, u32)>),
}

#[derive(Archive, Serialize)]
pub struct Payload {
    key: PayloadKey,
    title: String,
    text: String,
    fields: Vec<(String, u32)>,
    values: Vec<FieldValue>,
    links: Vec<PayloadKey>,
    version: u64,
}
fn failure(message: impl std::fmt::Display) -> Error {
    Error::Corrupt(format!("rkyv payload: {message}"))
}
fn key(value: &Key) -> PayloadKey {
    PayloadKey {
        scope: value.scope.clone(),
        collection: value.collection.clone(),
        id: value.id.clone(),
    }
}
fn push(value: &Value, nodes: &mut Vec<FieldValue>, depth: usize) -> Result<u32> {
    if depth > 128 {
        return Err(Error::Invalid("payload fields depth exceeds 128".into()));
    }
    let node = match value {
        Value::Null => FieldValue::Null,
        Value::Bool(v) => FieldValue::Bool(*v),
        Value::Number(v) => {
            if v.is_f64() {
                FieldValue::F64(v.as_f64().unwrap())
            } else if let Some(v) = v.as_u64() {
                FieldValue::I64 {
                    bits: v as i64,
                    unsigned: true,
                }
            } else {
                FieldValue::I64 {
                    bits: v.as_i64().unwrap(),
                    unsigned: false,
                }
            }
        }
        Value::String(v) => FieldValue::String(v.clone()),
        Value::Array(v) => FieldValue::List(
            v.iter()
                .map(|v| push(v, nodes, depth + 1))
                .collect::<Result<_>>()?,
        ),
        Value::Object(v) => FieldValue::Map(
            v.iter()
                .map(|(k, v)| Ok((k.clone(), push(v, nodes, depth + 1)?)))
                .collect::<Result<_>>()?,
        ),
    };
    let id =
        u32::try_from(nodes.len()).map_err(|_| Error::Invalid("too many field nodes".into()))?;
    nodes.push(node);
    Ok(id)
}
pub fn encode(stored: &StoredDocument) -> Result<AlignedVec> {
    let d = &stored.document;
    let mut values = Vec::new();
    let fields = d
        .fields
        .iter()
        .map(|(name, value)| Ok((name.clone(), push(value, &mut values, 0)?)))
        .collect::<Result<_>>()?;
    let payload = Payload {
        key: key(&d.key),
        title: d.title.clone(),
        text: d.text.clone(),
        fields,
        values,
        links: d.links.iter().map(key).collect(),
        version: stored.version,
    };
    rkyv::to_bytes::<ArchiveError>(&payload).map_err(failure)
}

pub fn checked(bytes: &[u8]) -> Result<&ArchivedPayload> {
    let payload = rkyv::access::<ArchivedPayload, ArchiveError>(bytes).map_err(failure)?;
    // Every child points backwards. This rules out cycles and forward references
    // before full decoding, and bounds the graph traversal without unsafe access.
    let mut references = vec![0u8; payload.values.len()];
    let mut depths = vec![0u8; payload.values.len()];
    for (id, value) in payload.values.iter().enumerate() {
        let valid_child = |child: &rkyv::primitive::ArchivedU32| (child.to_native() as usize) < id;
        match value {
            ArchivedFieldValue::List(values) if !values.iter().all(valid_child) => {
                return Err(failure("list child bounds/order"));
            }
            ArchivedFieldValue::Map(values) => {
                if !values.iter().all(|v| valid_child(&v.1))
                    || !values.windows(2).all(|v| v[0].0.as_str() < v[1].0.as_str())
                {
                    return Err(failure("map child bounds/order"));
                }
            }
            ArchivedFieldValue::F64(v) if !v.to_native().is_finite() => {
                return Err(failure("nonfinite field number"));
            }
            _ => {}
        }
        let mut mark = |child: u32| -> Result<()> {
            let child = child as usize;
            if child >= id || references[child] != 0 {
                return Err(failure("shared/invalid field child"));
            }
            references[child] = 1;
            let depth = depths[child]
                .checked_add(1)
                .ok_or_else(|| failure("field depth"))?;
            if depth > 128 {
                return Err(failure("field depth"));
            }
            depths[id] = depths[id].max(depth);
            Ok(())
        };
        match value {
            ArchivedFieldValue::List(v) => {
                for child in v.iter() {
                    mark(child.to_native())?;
                }
            }
            ArchivedFieldValue::Map(v) => {
                for pair in v.iter() {
                    mark(pair.1.to_native())?;
                }
            }
            _ => {}
        }
    }
    if !payload
        .fields
        .iter()
        .all(|v| (v.1.to_native() as usize) < payload.values.len())
        || !payload
            .fields
            .windows(2)
            .all(|v| v[0].0.as_str() < v[1].0.as_str())
    {
        return Err(failure("field roots/order"));
    }
    for root in payload.fields.iter() {
        let id = root.1.to_native() as usize;
        if references[id] != 0 {
            return Err(failure("shared field root"));
        }
        references[id] = 1;
    }
    if references.iter().any(|count| *count != 1) {
        return Err(failure("unreachable field node"));
    }
    Ok(payload)
}
impl ArchivedPayload {
    pub fn title(&self) -> &str {
        self.title.as_str()
    }
    pub fn text(&self) -> &str {
        self.text.as_str()
    }
    pub fn key_parts(&self) -> (&str, &str, &str) {
        (
            self.key.scope.as_str(),
            self.key.collection.as_str(),
            self.key.id.as_str(),
        )
    }
    pub fn version(&self) -> u64 {
        self.version.to_native()
    }
    pub fn to_document(&self) -> Result<StoredDocument> {
        let fields = self
            .fields
            .iter()
            .map(|v| Ok((v.0.as_str().to_owned(), self.value(v.1.to_native(), 0)?)))
            .collect::<Result<_>>()?;
        let owned_key = |k: &ArchivedPayloadKey| {
            Key::new(k.scope.as_str(), k.collection.as_str(), k.id.as_str())
        };
        Ok(StoredDocument {
            document: Document {
                key: owned_key(&self.key),
                title: self.title().into(),
                text: self.text().into(),
                fields,
                links: self.links.iter().map(owned_key).collect(),
            },
            version: self.version(),
        })
    }
    fn value(&self, id: u32, depth: usize) -> Result<Value> {
        if depth > 128 {
            return Err(failure("field depth"));
        }
        let v = self
            .values
            .get(id as usize)
            .ok_or_else(|| failure("field node bounds"))?;
        Ok(match v {
            ArchivedFieldValue::Null => Value::Null,
            ArchivedFieldValue::Bool(v) => Value::Bool(*v),
            ArchivedFieldValue::I64 { bits, unsigned } => Value::Number(if *unsigned {
                Number::from(bits.to_native() as u64)
            } else {
                Number::from(bits.to_native())
            }),
            ArchivedFieldValue::F64(v) => Value::Number(
                Number::from_f64(v.to_native()).ok_or_else(|| failure("nonfinite number"))?,
            ),
            ArchivedFieldValue::String(v) => Value::String(v.as_str().into()),
            ArchivedFieldValue::List(v) => Value::Array(
                v.iter()
                    .map(|id| self.value(id.to_native(), depth + 1))
                    .collect::<Result<_>>()?,
            ),
            ArchivedFieldValue::Map(v) => Value::Object(
                v.iter()
                    .map(|v| Ok((v.0.as_str().into(), self.value(v.1.to_native(), depth + 1)?)))
                    .collect::<Result<_>>()?,
            ),
        })
    }
}

pub fn decode(bytes: &[u8]) -> Result<StoredDocument> {
    checked(bytes)?.to_document()
}

/// The same normalization and AND substring semantics as the engine.
pub fn query_terms(text: &str) -> Vec<String> {
    text.split_whitespace().map(crate::index::fold).collect()
}
pub fn text_matches(title: &str, text: &str, terms: &[String]) -> bool {
    let title = crate::index::fold(title);
    if terms.iter().all(|term| title.contains(term)) {
        return true;
    }
    let text = crate::index::fold(text);
    terms
        .iter()
        .all(|term| title.contains(term) || text.contains(term))
}

/// Payload-only experiment: no engine path or persisted representation changes.
#[derive(Clone, Copy, Debug)]
pub enum FoldStrategy {
    Baseline,
    Ascii,
    Reuse,
}
impl FoldStrategy {
    pub fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Ascii => "ascii",
            Self::Reuse => "reuse",
        }
    }
}
fn fold_into(text: &str, output: &mut String) {
    crate::index::fold_into(text, output);
}

pub struct TextMatcher {
    strategy: FoldStrategy,
    title: String,
    text: String,
}
impl TextMatcher {
    pub fn new(strategy: FoldStrategy) -> Self {
        Self {
            strategy,
            title: String::new(),
            text: String::new(),
        }
    }
    fn prepare(strategy: FoldStrategy, input: &str, buffer: &mut String) {
        match strategy {
            FoldStrategy::Baseline => *buffer = crate::index::fold(input),
            FoldStrategy::Ascii => {
                let mut fresh = String::new();
                fold_into(input, &mut fresh);
                *buffer = fresh;
            }
            FoldStrategy::Reuse => fold_into(input, buffer),
        }
    }
    pub fn prepare_title(&mut self, input: &str) {
        Self::prepare(self.strategy, input, &mut self.title);
    }
    pub fn prepare_text(&mut self, input: &str) {
        Self::prepare(self.strategy, input, &mut self.text);
    }
    pub fn title_matches_all(&self, terms: &[String]) -> bool {
        terms.iter().all(|term| self.title.contains(term))
    }
    pub fn matches_all(&self, terms: &[String]) -> bool {
        terms
            .iter()
            .all(|term| self.title.contains(term) || self.text.contains(term))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn experimental_folding_matches_every_unicode_scalar_and_reused_search() {
        let all: String = (0..=0x10ffff).filter_map(char::from_u32).collect();
        let expected = crate::index::fold(&all);
        let mut output = String::new();
        fold_into(&all, &mut output);
        assert_eq!(output, expected);
        for strategy in [
            FoldStrategy::Baseline,
            FoldStrategy::Ascii,
            FoldStrategy::Reuse,
        ] {
            let mut matcher = TextMatcher::new(strategy);
            for (title, text, query) in [
                ("ABC", "Nothing", "abc"),
                ("İSTANBUL", "ŞİRKET ÇĞÖÜ ı Σς ﬃ 🦀 東京", "sirket istanbul"),
                ("", "İ\u{307} Σ K ẞ", "i\u{307}"),
                ("the", "whale", "the whale"),
                ("a", "b", "ab"),
                ("", "", ""),
                ("SMALL", "Z", "missing"),
            ] {
                let terms = query_terms(query);
                matcher.prepare_title(title);
                let found = if matcher.title_matches_all(&terms) {
                    true
                } else {
                    matcher.prepare_text(text);
                    matcher.matches_all(&terms)
                };
                assert_eq!(found, text_matches(title, text, &terms));
            }
            matcher.prepare_text(&"UPPERCASE".repeat(6000));
            matcher.prepare_text("tiny");
            assert_eq!(matcher.text, "tiny");
        }
    }
    #[test]
    fn bytecheck_valid_but_malformed_field_graphs_are_rejected() {
        let cases = vec![
            vec![FieldValue::List(vec![0])],
            vec![
                FieldValue::String("leaf".into()),
                FieldValue::List(vec![0, 0]),
            ],
            vec![FieldValue::F64(f64::NAN)],
            vec![FieldValue::Null, FieldValue::Null],
            vec![
                FieldValue::Null,
                FieldValue::Bool(true),
                FieldValue::Map(vec![("z".into(), 0), ("a".into(), 1)]),
            ],
            {
                let mut v = vec![FieldValue::Null];
                for id in 0..130 {
                    v.push(FieldValue::List(vec![id]));
                }
                v
            },
        ];
        for values in cases {
            let root = values.len() as u32 - 1;
            let p = Payload {
                key: PayloadKey {
                    scope: "shared".into(),
                    collection: "texts".into(),
                    id: "0".into(),
                },
                title: String::new(),
                text: String::new(),
                fields: vec![("field".into(), root)],
                values,
                links: vec![],
                version: 1,
            };
            let bytes = rkyv::to_bytes::<ArchiveError>(&p).unwrap();
            assert!(rkyv::access::<ArchivedPayload, ArchiveError>(&bytes).is_ok());
            assert!(checked(&bytes).is_err());
        }
    }
    #[test]
    fn every_json_kind_number_boundary_unicode_and_links_round_trip() {
        let stored = StoredDocument {
            document: Document {
                key: Key::new("shared", "docs", "東京"),
                title: "ŞİRKET \"🦀\"\n".into(),
                text: "\\ escaped İstanbul".into(),
                fields: BTreeMap::from([(
                    "all".into(),
                    serde_json::json!([null,true,false,i64::MIN,i64::MAX,u64::MAX,0,-0.0,1.25,1e300,"null",[],{}, {"z":["🦀",null],"a":"İ"}]),
                )]),
                links: vec![Key::new("shared", "docs", "other")],
            },
            version: u64::MAX,
        };
        let bytes = encode(&stored).unwrap();
        let result = decode(&bytes).unwrap();
        assert_eq!(result, stored);
        assert_eq!(
            serde_json::to_vec(&result).unwrap(),
            serde_json::to_vec(&stored).unwrap()
        );
        assert_eq!(
            result.document.fields["all"][7].as_f64().unwrap().to_bits(),
            (-0.0f64).to_bits()
        );
    }
}
