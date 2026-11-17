//! Shared by the FDB and isolated native SQLite benchmarks.
use serde_json::{Value, json};
use siftwake::{Document, Key};
use std::collections::BTreeMap;

pub fn key(id: usize) -> Key {
    Key::new("shared", "contents", &format!("{id:010}"))
}

pub fn document(id: usize, body: &str) -> Document {
    Document {
        key: key(id),
        title: format!("rapor-{id:010}"),
        text: body.into(),
        fields: BTreeMap::from([
            ("day".into(), json!(id % 30)),
            ("source".into(), json!("synthetic")),
        ]),
        links: vec![],
    }
}

pub fn body(bytes: usize) -> String {
    let mut text = "Şirketin teknoloji yatırımları ve bilanço raporu. ".repeat(bytes / 20 + 1);
    while text.len() > bytes {
        text.pop();
    }
    text
}

pub fn samples(mut values: Vec<f64>) -> Value {
    values.sort_by(f64::total_cmp);
    json!({"p50_ms":values[values.len()/2],"p95_ms":values[(values.len()*95/100).min(values.len()-1)],"max_ms":values[values.len()-1]})
}
