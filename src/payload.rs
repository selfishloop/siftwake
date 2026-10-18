//! OFF1 typed offset payload used by FDBLOG04/TXN2 and FDBSEG04.
//! Safe borrowed UTF-8 views; typed metadata is validated without allocating.
use crate::{Document, Key, Result, StoredDocument};
use serde_json::{Number, Value};
use std::collections::BTreeMap;

const HEADER: usize = 44;
const MAGIC: &[u8; 4] = b"OFF1";

fn failure(message: impl std::fmt::Display) -> crate::Error {
    crate::Error::Corrupt(format!("binary payload: {message}"))
}

fn checked_utf8(bytes: &[u8]) -> Result<&str> {
    // Keep detailed std errors on invalid input; safe SIMD validation handles
    // long valid strings without copying or skipping any UTF-8 checks.
    if bytes.len() < 64 {
        return std::str::from_utf8(bytes).map_err(failure);
    }
    simdutf8::basic::from_utf8(bytes)
        .or_else(|_| std::str::from_utf8(bytes))
        .map_err(failure)
}

/// Legacy parsing is confined to replay/cache reads; new writes always use OFF1.
pub(crate) fn decode(bytes: &[u8]) -> Result<StoredDocument> {
    if bytes.starts_with(MAGIC) {
        checked(bytes)?.to_document()
    } else {
        serde_json::from_slice(bytes).map_err(failure)
    }
}

fn size(n: usize) -> Result<u32> {
    u32::try_from(n).map_err(|_| failure("binary length overflow"))
}
fn string(out: &mut Vec<u8>, s: &str) -> Result<()> {
    out.extend_from_slice(&size(s.len())?.to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    Ok(())
}
// Wire tags: null, false, true, signed, unsigned, float, string, list, map.
// Unsigned is an integer representation tag, preserving u64::MAX exactly.
fn value(out: &mut Vec<u8>, v: &Value, depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(failure("binary depth"));
    }
    match v {
        Value::Null => out.push(0),
        Value::Bool(v) => out.push(if *v { 2 } else { 1 }),
        Value::Number(n) => {
            if n.is_f64() {
                out.push(5);
                out.extend_from_slice(&n.as_f64().unwrap().to_le_bytes());
            } else if let Some(n) = n.as_u64() {
                out.push(4);
                out.extend_from_slice(&n.to_le_bytes());
            } else {
                out.push(3);
                out.extend_from_slice(&n.as_i64().unwrap().to_le_bytes());
            }
        }
        Value::String(s) => {
            out.push(6);
            string(out, s)?;
        }
        Value::Array(a) => {
            out.push(7);
            out.extend_from_slice(&size(a.len())?.to_le_bytes());
            for v in a {
                value(out, v, depth + 1)?;
            }
        }
        Value::Object(m) => {
            out.push(8);
            out.extend_from_slice(&size(m.len())?.to_le_bytes());
            for (k, v) in m {
                string(out, k)?;
                value(out, v, depth + 1)?;
            }
        }
    }
    Ok(())
}

/// Encoded size without materializing JSON or allocating a payload.
pub(crate) fn encoded_len(d: &Document) -> Result<usize> {
    fn node(v: &Value, depth: usize) -> Result<usize> {
        if depth > 128 {
            return Err(crate::Error::Invalid(
                "binary metadata depth exceeds 128".into(),
            ));
        }
        Ok(match v {
            Value::Null | Value::Bool(_) => 1,
            Value::Number(_) => 9,
            Value::String(s) => 5 + s.len(),
            Value::Array(a) => {
                let mut n = 5;
                for v in a {
                    n += node(v, depth + 1)?;
                }
                n
            }
            Value::Object(m) => {
                let mut n = 5;
                for (k, v) in m {
                    n += 4 + k.len() + node(v, depth + 1)?;
                }
                n
            }
        })
    }
    let mut n = HEADER
        + d.key.scope.len()
        + d.key.collection.len()
        + d.key.id.len()
        + d.title.len()
        + d.text.len()
        + 8;
    for (k, v) in &d.fields {
        n += 4 + k.len() + node(v, 0)?;
    }
    for k in &d.links {
        n += 12 + k.scope.len() + k.collection.len() + k.id.len();
    }
    Ok(n)
}
pub fn encode(stored: &StoredDocument) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(encoded_len(&stored.document)?);
    encode_into(stored, &mut out)?;
    Ok(out)
}

pub(crate) fn encode_into(stored: &StoredDocument, out: &mut Vec<u8>) -> Result<()> {
    let d = &stored.document;
    let start = out.len();
    out.resize(start + HEADER, 0);
    out[start..start + 4].copy_from_slice(MAGIC);
    out[start + 4..start + 12].copy_from_slice(&stored.version.to_le_bytes());
    let strings = [
        &d.key.scope,
        &d.key.collection,
        &d.key.id,
        &d.title,
        &d.text,
    ];
    for (i, s) in strings.into_iter().enumerate() {
        let offset = size(out.len() - start)?.to_le_bytes();
        out[start + 12 + i * 4..start + 16 + i * 4].copy_from_slice(&offset);
        out.extend_from_slice(s.as_bytes());
    }
    let offset = size(out.len() - start)?.to_le_bytes();
    out[start + 32..start + 36].copy_from_slice(&offset);
    out.extend_from_slice(&size(d.fields.len())?.to_le_bytes());
    for (k, v) in &d.fields {
        string(out, k)?;
        value(out, v, 0)?;
    }
    let offset = size(out.len() - start)?.to_le_bytes();
    out[start + 36..start + 40].copy_from_slice(&offset);
    out.extend_from_slice(&size(d.links.len())?.to_le_bytes());
    for k in &d.links {
        string(out, &k.scope)?;
        string(out, &k.collection)?;
        string(out, &k.id)?;
    }
    let offset = size(out.len() - start)?.to_le_bytes();
    out[start + 40..start + 44].copy_from_slice(&offset);
    Ok(())
}
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| failure("binary bounds"))?;
        let b = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| failure("binary bounds"))?;
        self.pos = end;
        Ok(b)
    }
    fn count(&mut self) -> Result<usize> {
        let n = u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize;
        // Each entry must consume at least one byte. Never reserve from untrusted counts.
        if n > self.bytes.len() - self.pos {
            return Err(failure("binary count"));
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<&'a str> {
        let n = self.count()?;
        checked_utf8(self.take(n)?)
    }
    fn node(&mut self, depth: usize, decode: bool) -> Result<Option<Value>> {
        if depth > 128 {
            return Err(failure("binary depth"));
        }
        let tag = self.take(1)?[0];
        let result = match tag {
            0 => decode.then_some(Value::Null),
            1 | 2 => decode.then_some(Value::Bool(tag == 2)),
            3..=5 => {
                let b: [u8; 8] = self.take(8)?.try_into().unwrap();
                let n = match tag {
                    3 => Number::from(i64::from_le_bytes(b)),
                    4 => Number::from(u64::from_le_bytes(b)),
                    _ => Number::from_f64(f64::from_le_bytes(b))
                        .ok_or_else(|| failure("binary nonfinite"))?,
                };
                decode.then_some(Value::Number(n))
            }
            6 => {
                let s = self.string()?;
                decode.then(|| Value::String(s.to_owned()))
            }
            7 => {
                let count = self.count()?;
                let mut a = Vec::new();
                for _ in 0..count {
                    if let Some(v) = self.node(depth + 1, decode)? {
                        a.push(v);
                    }
                }
                decode.then_some(Value::Array(a))
            }
            8 => {
                let count = self.count()?;
                let mut m = serde_json::Map::new();
                let mut previous = None;
                for _ in 0..count {
                    let k = self.string()?;
                    if previous.is_some_and(|p| p >= k) {
                        return Err(failure("binary map order"));
                    }
                    previous = Some(k);
                    if let Some(v) = self.node(depth + 1, decode)? {
                        m.insert(k.to_owned(), v);
                    }
                }
                decode.then_some(Value::Object(m))
            }
            _ => return Err(failure("binary value tag")),
        };
        Ok(result)
    }
    fn end(&self) -> Result<()> {
        if self.pos != self.bytes.len() {
            return Err(failure("binary trailing bytes"));
        }
        Ok(())
    }
}
pub struct View<'a> {
    strings: [&'a str; 5],
    version: u64,
    fields: &'a [u8],
    links: &'a [u8],
}
fn fields(bytes: &[u8], decode: bool) -> Result<BTreeMap<String, Value>> {
    let mut c = Cursor { bytes, pos: 0 };
    let count = c.count()?;
    let mut result = BTreeMap::new();
    let mut previous = None;
    for _ in 0..count {
        let k = c.string()?;
        if previous.is_some_and(|p| p >= k) {
            return Err(failure("binary field order"));
        }
        previous = Some(k);
        if let Some(v) = c.node(0, decode)? {
            result.insert(k.to_owned(), v);
        }
    }
    c.end()?;
    Ok(result)
}
fn links(bytes: &[u8], decode: bool) -> Result<Vec<Key>> {
    let mut c = Cursor { bytes, pos: 0 };
    let count = c.count()?;
    let mut result = Vec::new();
    for _ in 0..count {
        let (scope, collection, id) = (c.string()?, c.string()?, c.string()?);
        if decode {
            result.push(Key::new(scope, collection, id));
        }
    }
    c.end()?;
    Ok(result)
}
/// Structural/UTF-8/metadata validation without document or field allocation.
/// Caller must also verify the transport CRC on every read.
pub fn checked(bytes: &[u8]) -> Result<View<'_>> {
    #[cfg(feature = "diagnostic-timing")]
    let _schema = crate::timing::span("payload.schema_utf8");
    if bytes.len() < HEADER || &bytes[..4] != MAGIC {
        return Err(failure("binary header"));
    }
    let mut offsets = [0usize; 8];
    for (i, offset) in offsets.iter_mut().enumerate() {
        *offset = u32::from_le_bytes(bytes[12 + i * 4..16 + i * 4].try_into().unwrap()) as usize;
    }
    if offsets[0] != HEADER
        || offsets[7] != bytes.len()
        || !offsets.windows(2).all(|w| w[0] <= w[1])
    {
        return Err(failure("binary offsets"));
    }
    let mut strings = [""; 5];
    for i in 0..5 {
        strings[i] = checked_utf8(&bytes[offsets[i]..offsets[i + 1]])?;
    }
    let field_bytes = &bytes[offsets[5]..offsets[6]];
    let link_bytes = &bytes[offsets[6]..offsets[7]];
    fields(field_bytes, false)?;
    links(link_bytes, false)?;
    Ok(View {
        strings,
        version: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
        fields: field_bytes,
        links: link_bytes,
    })
}
impl View<'_> {
    /// The same document limits as validate_doc, without materializing strings
    /// or JSON values. checked() already guarantees canonical wire sizes.
    #[cold]
    #[inline(never)]
    pub(crate) fn validate_document(&self, encoded_len: usize) -> Result<()> {
        let (scope, collection, id) = self.key_parts();
        crate::validate_key_parts([scope, collection, id])?;
        let mut fields = Cursor {
            bytes: self.fields,
            pos: 0,
        };
        let field_count = fields.count()?;
        let mut links = Cursor {
            bytes: self.links,
            pos: 0,
        };
        let link_count = links.count()?;
        if self.title().len() + self.text().len() > 2 * 1024 * 1024
            || field_count > 256
            || link_count > 1024
        {
            return Err(crate::Error::Invalid(
                "document exceeds text/field/link limits".into(),
            ));
        }
        for _ in 0..field_count {
            let name = fields.string()?;
            if name.is_empty() || name.len() > 256 {
                return Err(crate::Error::Invalid("invalid field name".into()));
            }
            fields.node(0, false)?;
        }
        fields.end()?;
        for _ in 0..link_count {
            let parts = [links.string()?, links.string()?, links.string()?];
            crate::validate_key_parts(parts)?;
            if parts[0] != scope {
                return Err(crate::Error::Invalid(
                    "links must stay within the same scope".into(),
                ));
            }
        }
        links.end()?;
        if encoded_len > 4 * 1024 * 1024 {
            return Err(crate::Error::Invalid(
                "binary document exceeds 4 MiB".into(),
            ));
        }
        Ok(())
    }
    pub fn title(&self) -> &str {
        self.strings[3]
    }
    pub fn text(&self) -> &str {
        self.strings[4]
    }
    pub fn key_parts(&self) -> (&str, &str, &str) {
        (self.strings[0], self.strings[1], self.strings[2])
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn to_document(&self) -> Result<StoredDocument> {
        #[cfg(feature = "diagnostic-timing")]
        let _materialize = crate::timing::span("payload.materialize");
        let (scope, collection, id) = self.key_parts();
        Ok(StoredDocument {
            document: Document {
                key: {
                    #[cfg(feature = "diagnostic-timing")]
                    let _copy = crate::timing::span("payload.copy_key");
                    Key::new(scope, collection, id)
                },
                title: {
                    #[cfg(feature = "diagnostic-timing")]
                    let _copy = crate::timing::span("payload.copy_title");
                    self.title().to_owned()
                },
                text: {
                    #[cfg(feature = "diagnostic-timing")]
                    let _copy = crate::timing::span("payload.copy_text");
                    self.text().to_owned()
                },
                fields: {
                    #[cfg(feature = "diagnostic-timing")]
                    let _fields = crate::timing::span("payload.materialize_fields");
                    fields(self.fields, true)?
                },
                links: {
                    #[cfg(feature = "diagnostic-timing")]
                    let _links = crate::timing::span("payload.materialize_links");
                    links(self.links, true)?
                },
            },
            version: self.version,
        })
    }
}

#[cfg(test)]
mod document_limit_tests {
    use super::*;

    #[test]
    fn utf8_validation_matches_std_including_boundaries_and_invalid_bytes() {
        fn compare(bytes: &[u8]) {
            match (std::str::from_utf8(bytes), checked_utf8(bytes)) {
                (Ok(expected), Ok(actual)) => assert_eq!(actual, expected),
                (Err(expected), Err(actual)) => {
                    assert_eq!(actual.to_string(), failure(expected).to_string())
                }
                _ => panic!("UTF-8 validators disagree"),
            }
        }
        for text in ["plain ASCII", "İstanbul ışık", "東京", "🦀", "\0\u{10ffff}"] {
            for offset in 0..32 {
                let mut bytes = vec![b'a'; offset];
                bytes.extend(text.repeat(80).as_bytes());
                for cut in 0..bytes.len() - offset {
                    compare(&bytes[offset..offset + cut]);
                }
                for position in offset..bytes.len() {
                    let original = bytes[position];
                    for bad in [0x80, 0xc0, 0xed, 0xf5, 0xff] {
                        bytes[position] = bad;
                        compare(&bytes[offset..]);
                    }
                    bytes[position] = original;
                }
            }
        }
        let mut state = 0x12345678u32;
        for length in 0..300 {
            let bytes: Vec<_> = (0..length)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    state as u8
                })
                .collect();
            compare(&bytes);
        }
    }

    #[test]
    fn borrowed_limits_match_owned_validation() {
        let valid = Document {
            key: Key::new("şirket", "texts", "東京"),
            title: "🦀".into(),
            text: "body".into(),
            fields: BTreeMap::from([(
                "nested".into(),
                serde_json::json!({"a":[null,true,i64::MIN,u64::MAX,-0.0,"東京"]}),
            )]),
            links: vec![Key::new("şirket", "texts", "target")],
        };
        let mut cases = vec![valid.clone()];
        for component in 0..3 {
            for invalid in ["".to_owned(), "a\n".to_owned(), "x".repeat(1025)] {
                let mut doc = valid.clone();
                match component {
                    0 => doc.key.scope = invalid,
                    1 => doc.key.collection = invalid,
                    _ => doc.key.id = invalid,
                }
                cases.push(doc);
            }
        }
        for length in [2 * 1024 * 1024 - valid.title.len(), 2 * 1024 * 1024] {
            let mut doc = valid.clone();
            doc.text = "x".repeat(length);
            cases.push(doc);
        }
        for name in ["".to_owned(), "x".repeat(256), "x".repeat(257)] {
            let mut doc = valid.clone();
            doc.fields = BTreeMap::from([(name, Value::Null)]);
            cases.push(doc);
        }
        for count in [256, 257] {
            let mut doc = valid.clone();
            doc.fields = (0..count)
                .map(|n| (format!("field{n}"), Value::Null))
                .collect();
            cases.push(doc);
        }
        for count in [1024, 1025] {
            let mut doc = valid.clone();
            doc.links = vec![Key::new("şirket", "texts", "target"); count];
            cases.push(doc);
        }
        for key in [
            Key::new("other", "texts", "target"),
            Key::new("şirket", "texts", "bad\0"),
        ] {
            let mut doc = valid.clone();
            doc.links = vec![key];
            cases.push(doc);
        }
        let mut doc = valid;
        doc.fields
            .insert("large".into(), Value::String("x".repeat(4 * 1024 * 1024)));
        cases.push(doc);
        for document in cases {
            let expected = crate::validate_doc(&document).is_ok();
            let bytes = encode(&StoredDocument {
                document,
                version: 1,
            })
            .unwrap();
            let actual = checked(&bytes)
                .unwrap()
                .validate_document(bytes.len())
                .is_ok();
            assert_eq!(actual, expected);
        }
    }
}
