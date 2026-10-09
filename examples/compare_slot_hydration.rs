//! Alternate hydration paths on one open DB, excluding JSON/stdio timing.
#[cfg(not(feature = "experimental-direct-slot"))]
fn main() {
    eprintln!("build with --features experimental-direct-slot");
    std::process::exit(1);
}

#[cfg(feature = "experimental-direct-slot")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use fdb::{Database, Query, Range};
    use serde_json::json;
    use std::time::Instant;
    let path = std::env::args()
        .nth(1)
        .ok_or("compare_slot_hydration <healthy-db>")?;
    let db = Database::open(path)?;
    let grouped = std::env::args().any(|arg| arg == "--grouped");
    if grouped && !cfg!(feature = "experimental-grouped-read") {
        return Err("--grouped requires experimental-grouped-read".into());
    }
    let modes = if grouped { 3 } else { 2 };
    let rounds = if grouped { 6 } else { 5 };
    let skip_text = std::env::args().any(|arg| arg == "--skip-text");
    let mut exact = Query::new("shared");
    exact.collection = Some("contents".into());
    exact.limit = 50;
    exact.exact.insert("source".into(), json!("synthetic"));
    let mut range = exact.clone();
    range.ranges.push(Range {
        field: "day".into(),
        min: 2,
        max: 5,
    });
    let mut narrow = Query::new("shared");
    narrow.collection = Some("contents".into());
    narrow.limit = 50;
    narrow.text = "rapor-0000500001".into();
    let mut text = narrow.clone();
    text.text = "teknoloji".into();
    let mut second = exact.clone();
    second.offset = 50;
    let mut last = exact.clone();
    last.offset = 999900;
    for run in 0..rounds {
        for (name, q, repeats) in [
            ("exact", &exact, 30),
            ("range", &range, 30),
            ("narrow", &narrow, 30),
            ("second_page", &second, 30),
            ("last_page", &last, 3),
            ("text", &text, 1),
        ] {
            if skip_text && name == "text" {
                continue;
            }
            let expected = serde_json::to_value(db.search(q)?)?;
            assert_eq!(serde_json::to_value(db.search_direct_slots(q)?)?, expected);
            #[cfg(feature = "experimental-grouped-read")]
            if grouped {
                assert_eq!(serde_json::to_value(db.search_grouped_slots(q)?)?, expected);
            }
            let mut times = vec![vec![]; modes];
            for iteration in 0..repeats {
                let order: Vec<_> = if grouped {
                    let offset = (iteration + run) % modes;
                    (0..modes)
                        .map(|n| {
                            (offset
                                + if (iteration + run) % 2 == 0 {
                                    n
                                } else {
                                    modes - n
                                })
                                % modes
                        })
                        .collect()
                } else if (iteration + run) % 2 == 0 {
                    vec![0, 1]
                } else {
                    vec![1, 0]
                };
                for mode in order {
                    let start = Instant::now();
                    let page = match mode {
                        0 => db.search(q)?,
                        1 => db.search_direct_slots(q)?,
                        #[cfg(feature = "experimental-grouped-read")]
                        2 => db.search_grouped_slots(q)?,
                        _ => return Err("invalid measurement mode".into()),
                    };
                    times[mode].push(start.elapsed().as_secs_f64() * 1000.);
                    assert_eq!(serde_json::to_value(page)?, expected);
                }
            }
            for (mode, values) in times.iter_mut().enumerate() {
                values.sort_by(f64::total_cmp);
                println!(
                    "{}",
                    json!({"phase":"query","run":run,"query":name,
                    "engine":match mode {0=>"baseline",1=>"candidate",_=>"grouped"},
                    "total":expected["total"],"hits":expected["hits"].as_array().unwrap().len(),
                    "timing":{"p50_ms":values[values.len()/2],"p95_ms":values[(values.len()*95/100).min(values.len()-1)],
                        "max_ms":values[values.len()-1]},
                    "samples":repeats,"includes_stdio":false,"includes_json_serialization":false,
                    "cache":"same open DB; warm OS cache; alternating per request"})
                );
            }
        }
    }
    Ok(())
}
