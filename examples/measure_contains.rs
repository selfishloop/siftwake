//! Read-only contains matrix for the completed 100k native benchmark corpus.
#[path = "../tools/bench_support/corpus.rs"]
mod corpus;
#[path = "../tools/bench_support/verify.rs"]
mod verify;
use serde_json::json;
use siftwake::{Database, Query, Range};
use std::time::Instant;

// Independent normalization oracle, rather than the engine's index helpers.
fn normalized(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| *c != '\u{307}')
        .map(|c| match c {
            'ı' => 'i',
            'ş' => 's',
            'ğ' => 'g',
            'ç' => 'c',
            'ö' => 'o',
            'ü' => 'u',
            c => c,
        })
        .collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("healthy completed 100k DB required")?;
    let only = std::env::args().nth(2);
    let repeats = std::env::args()
        .nth(3)
        .map(|n| n.parse::<usize>())
        .transpose()?
        .unwrap_or(20);
    if repeats == 0 {
        return Err("positive repeats required".into());
    }
    let db = Database::open(&path)?;
    assert_eq!(db.stats()?.documents, 99_950);
    let body = corpus::body(256);
    let all = verify::expected("exact", 100_000, &body, true, 0);
    assert_eq!(all.0, 99_950);
    // Build the independent expected corpus outside every timed interval.
    let mut documents = Vec::new();
    for id in 0usize..100_000 {
        let changed = id.is_multiple_of(1000) && id / 1000 < 100;
        if changed && (id / 1000).is_multiple_of(2) {
            continue;
        }
        let mut stored = verify::get(id, if changed { "changed sentinel" } else { &body });
        if changed {
            stored.version = 201;
        }
        documents.push(stored);
    }
    let folded: Vec<_> = documents
        .iter()
        .map(|d| (normalized(&d.document.title), normalized(&d.document.text)))
        .collect();
    for (name, text, range, offset) in [
        ("rare_title", "rapor-0000050001", false, 0),
        ("common_body", "teknoloji", false, 0),
        ("two_terms", "teknoloji bilanço", false, 0),
        ("short_two_chars", "Şİ", false, 0),
        ("missing", "bulunmayan-xyz", false, 0),
        ("false_gram_candidate", "000000000000", false, 0),
        ("common_with_range", "teknoloji", true, 0),
        ("common_second_page", "teknoloji", false, 50),
        ("common_last_page", "teknoloji", false, 99_850),
    ] {
        if only.as_deref().is_some_and(|filter| filter != name) {
            continue;
        }
        let terms: Vec<_> = text.split_whitespace().map(normalized).collect();
        let selected: Vec<_> = documents
            .iter()
            .zip(&folded)
            .filter(|(d, (title, body))| {
                (!range || (2..=5).contains(&d.document.fields["day"].as_u64().unwrap()))
                    && terms
                        .iter()
                        .all(|term| title.contains(term) || body.contains(term))
            })
            .map(|(d, _)| d.clone())
            .collect();
        let expected: Vec<_> = selected.iter().skip(offset).take(50).cloned().collect();
        let mut q = Query::new("shared");
        q.collection = Some("contents".into());
        q.text = text.into();
        q.offset = offset;
        if range {
            q.ranges.push(Range {
                field: "day".into(),
                min: 2,
                max: 5,
            });
        }
        let mut times = Vec::new();
        let mut first_ms = 0.;
        #[cfg(feature = "diagnostic-timing")]
        let _ = siftwake::timing::take();
        for repeat in 0..=repeats {
            let start = Instant::now();
            let page = db.search(&q)?;
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            assert_eq!(page.total, selected.len(), "{name}");
            assert_eq!(page.hits, expected, "{name}");
            if repeat == 0 {
                first_ms = elapsed;
            } else {
                times.push(elapsed);
            }
        }
        println!(
            "{}",
            json!({"query": name, "text": text, "offset": offset, "total": selected.len(), "first_query_ms": first_ms, "warm": corpus::samples(times), "repeats": repeats, "oracle": "full ordered StoredDocument and exact total", "cache": "warm OS cache; first_query is not cold disk", "open_excluded": true})
        );
        #[cfg(feature = "diagnostic-timing")]
        println!(
            "{}",
            json!({"query": name, "instrumented": true, "counters": siftwake::timing::take(), "nested_spans_overlap": true})
        );
    }
    Ok(())
}
