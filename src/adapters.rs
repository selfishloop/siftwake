//! Explicit ingestion adapters; these do not scan disks or access Hanbase.
use crate::{Document, Error, Key, Result};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// One file-system name/link. Volume + full file reference avoids cross-volume
/// collisions; parent + name preserves multiple hard links to one file.
pub fn file_entry(
    scope: &str,
    volume: &str,
    file_id: u64,
    parent_id: u64,
    name: &str,
    path: &str,
    flags: u16,
) -> Result<Document> {
    if volume.is_empty() || name.is_empty() {
        return Err(Error::Invalid("volume and file name are required".into()));
    }
    let id = json!([volume, file_id.to_string(), parent_id.to_string(), name]).to_string();
    Ok(Document {
        key: Key::new(scope, "files", &id),
        title: name.into(),
        text: path.into(),
        fields: BTreeMap::from([
            ("source".into(), json!("sdb")),
            ("volume".into(), json!(volume)),
            ("file_id".into(), json!(file_id.to_string())),
            ("parent_id".into(), json!(parent_id.to_string())),
            ("path".into(), json!(path)),
            ("flags".into(), json!(flags)),
        ]),
        links: vec![],
    })
}

/// Idempotent Fundops record ingestion. Supply the source's natural key (e.g.
/// fund_code + date + provider) and an explicit access scope. No ID guessing.
pub fn fundops_record(
    scope: &str,
    collection: &str,
    id: &str,
    row: BTreeMap<String, Value>,
    extracted_text: &str,
) -> Result<Document> {
    let code = row
        .get("fund_code")
        .or_else(|| row.get("code"))
        .and_then(Value::as_str);
    let title = row
        .get("subject")
        .or_else(|| row.get("name"))
        .and_then(Value::as_str)
        .or(code)
        .unwrap_or(id)
        .to_owned();
    let links = if collection != "funds" {
        code.map(|code| vec![Key::new(scope, "funds", code)])
            .unwrap_or_default()
    } else {
        vec![]
    };
    let doc = Document {
        key: Key::new(scope, collection, id),
        title,
        text: extracted_text.into(),
        fields: row,
        links,
    };
    super::validate_doc(&doc)?;
    Ok(doc)
}
