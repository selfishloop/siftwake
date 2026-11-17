//! Same oracle document reconstruction as measure_engine_fold.
use serde::Deserialize;
use serde_json::json;
use siftwake::{Document, Key, StoredDocument};
use std::collections::BTreeMap;
#[derive(Deserialize)]
pub(crate) struct Source {
    pub(crate) book_id: u32,
    pub(crate) title: String,
}
#[derive(Deserialize)]
pub(crate) struct Row {
    pub(crate) id: u32,
    pub(crate) book: usize,
    pub(crate) start: usize,
    pub(crate) len: usize,
}
#[derive(Deserialize)]
pub(crate) struct Corpus {
    pub(crate) sources: Vec<Source>,
    pub(crate) rows: Vec<Row>,
}
#[derive(Deserialize)]
pub(crate) struct Request {
    pub(crate) name: String,
    pub(crate) text: String,
    pub(crate) ids: Vec<usize>,
}
pub(crate) fn document(corpus: &Corpus, books: &[Vec<u8>], id: usize) -> StoredDocument {
    let row = &corpus.rows[id];
    assert_eq!(row.id as usize, id);
    let source = &corpus.sources[row.book];
    StoredDocument {
        version: id as u64 / 500 + 1,
        document: Document {
            key: Key::new("shared", "texts", &format!("{id:010}")),
            title: format!("Passage {id:010}"),
            text: String::from_utf8(books[row.book][row.start..row.start + row.len].to_vec())
                .unwrap(),
            fields: BTreeMap::from([
                ("source".into(), json!(source.title)),
                ("book_id".into(), json!(source.book_id)),
                ("offset".into(), json!(row.start)),
                ("bytes".into(), json!(row.len)),
                ("tags".into(), json!(["literature", "en"])),
                (
                    "attributes".into(),
                    json!({"verified":false,"score":0.125,"optional":null}),
                ),
            ]),
            links: if id.is_multiple_of(3) {
                vec![Key::new(
                    "shared",
                    "texts",
                    &format!("{:010}", id.saturating_sub(1)),
                )]
            } else {
                vec![]
            },
        },
    }
}
