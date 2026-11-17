//! Oracle-free process-memory probe; full payload equality is checked by Matrix.
use crate::input;
use serde_json::json;
use siftwake::{Key, StoredDocument};
use std::{thread, time::Duration};

pub fn measure<G, S>(mut get: G, mut search: S) -> input::Result<()>
where
    G: FnMut(&Key) -> input::Result<Option<StoredDocument>>,
    S: FnMut(&str, usize, usize) -> input::Result<(usize, Vec<StoredDocument>)>,
{
    input::phase("query_memory_idle");
    thread::sleep(Duration::from_millis(300));
    // The fixed existing first10k corpus. This is benchmark validation, not
    // an engine word optimization. No corpus or expected documents are loaded.
    let cases = [
        ("the", 0, 50, 10000usize),
        ("quixote", 0, 50, 56),
        ("he", 0, 50, 10000),
        ("quixote whale", 0, 50, 44),
        ("zx", 0, 50, 0),
        ("the zx", 0, 50, 0),
        ("the", 9950, 50, 10000),
        ("the", 0, 200, 10000),
    ];
    for round in 0..3 {
        for (text, offset, limit, expected) in cases {
            input::phase("query_memory_search");
            let (total, hits) = search(text, offset, limit)?;
            assert_eq!(total, expected);
            assert_eq!(hits.len(), expected.saturating_sub(offset).min(limit));
            let mut previous = None;
            for hit in &hits {
                assert_eq!(hit.document.key.scope, "shared");
                assert_eq!(hit.document.key.collection, "texts");
                let id: usize = hit.document.key.id.parse()?;
                assert!(previous.is_none_or(|before| before < id));
                assert_eq!(hit.version, (id / 500 + 1) as u64);
                previous = Some(id);
            }
            println!(
                "{}",
                json!({"phase":"query_memory_result","round":round,"text":text,"offset":offset,"limit":limit,"total":total,"hits":hits.len()})
            );
        }
        input::phase("query_memory_get");
        for n in 0..256 {
            let id = n * 9999 / 255;
            let key = Key::new("shared", "texts", &format!("{id:010}"));
            let doc = get(&key)?.ok_or("missing probe key")?;
            assert_eq!(doc.document.key, key);
            assert_eq!(doc.version, (id / 500 + 1) as u64);
        }
        assert!(get(&Key::new("shared", "texts", "9999999999"))?.is_none());
    }
    input::phase("query_memory_done");
    thread::sleep(Duration::from_millis(300));
    println!(
        "{}",
        json!({"phase":"query_memory_verified","rounds":3,"searches":24,"gets":771,"oracle_loaded":false,"full_payload_equality_in_separate_matrix":true,"performance_not_measured":true})
    );
    Ok(())
}
