use super::{BodyCache, fold, grams};

// A test-only intake switch. Both modes share one compiled search/JSON path;
// normal builds contain neither this atomic nor a runtime configuration knob.
#[cfg(feature = "bounded-engine")]
static INTAKE_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);

#[cfg(feature = "bounded-engine")]
pub(super) fn normalize(text: &str) -> String {
    match INTAKE_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        0 => return fold(text),
        2 => return super::fold_for_intake(text),
        1 => (),
        _ => unreachable!(),
    }
    let mut output = String::with_capacity(text.len());
    super::fold_into(text, &mut output);
    output
}

#[cfg(feature = "bounded-engine")]
#[test]
fn intake_capacity_matches_scalar_for_expansion_shrink_and_small_texts() {
    for len in [0, 1, 7, 8, 9, 31, 32, 33, 1024, 2057, 51197] {
        for unit in ["A", "İ", "Ⱥ", "🦀", "ASCII mixed—İȺ text"] {
            let text = unit.repeat(len);
            let expected = fold(&text);
            let actual = super::fold_for_intake(&text);
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
        }
    }
}

#[test]
fn body_cache_preserves_unicode_expansion_grams_and_cached_results() {
    let mut cache = BodyCache::default();
    for padding in [0, 31, 32, 1023, 1024, 1025, 4096] {
        for body in ["ASCII Plain TEXT", "İŞIK ıÇĞÖÜ 東京 🦀", "ȺΣẞK é—\u{1f88}"]
        {
            let text = format!("{}{body}{}", "A".repeat(padding), body.repeat(4));
            let expected = fold(&text);
            let expected_grams = grams(&expected);
            for _ in 0..2 {
                let terms = cache.get(&text);
                assert_eq!(&*terms.folded, expected);
                assert_eq!(terms.grams, expected_grams);
            }
        }
    }
}

// An explicit read-only probe, never run by the ordinary test suite. It consumes
// only the permitted first 10k records and keeps at most 200 source texts live.
#[cfg(feature = "bounded-engine")]
#[test]
#[ignore = "read-only normalization benchmark over the existing first 10k corpus"]
fn existing_corpus_normalization_probe() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs::File, hint::black_box, io::Read, time::Instant};

    let mut source = File::open("data/rkyv-corpus-20261009/json.payload-bench")?;
    let mut header = [0u8; 20];
    source.read_exact(&mut header)?;
    assert_eq!(&header[..8], b"PLOADB01");
    assert_eq!(crc32fast::hash(&header[..16]), word(&header[16..]));
    let run: usize = std::env::var("FDB_INTAKE_PROBE_RUN")
        .unwrap_or_else(|_| "0".into())
        .parse()?;
    let mut samples = [Vec::new(), Vec::new()];
    let mut capacity_bytes = [0usize; 2];
    let mut text_bytes = 0;
    for first in (0..10_000).step_by(200) {
        let mut texts = Vec::with_capacity(200);
        for id in first..first + 200 {
            let mut frame = [0u8; 16];
            source.read_exact(&mut frame)?;
            assert_eq!(crc32fast::hash(&frame[..12]), word(&frame[12..]));
            assert_eq!(word(&frame[8..12]) as usize, id);
            let len = word(&frame[..4]) as usize;
            assert!(len <= 4 * 1024 * 1024);
            let mut bytes = vec![0; len];
            source.read_exact(&mut bytes)?;
            assert_eq!(crc32fast::hash(&bytes), word(&frame[4..8]));
            let row: crate::model::StoredDocument = serde_json::from_slice(&bytes)?;
            assert_eq!(row.document.key.id, format!("{id:010}"));
            text_bytes += row.document.text.len();
            texts.push(row.document.text);
        }
        // Warm each mode once, then alternate order for four timed rounds.
        // Input CRC/JSON, Vec capacity and byte equality checks are not timed.
        for round in 0..5 {
            let mut outputs = [Vec::with_capacity(200), Vec::with_capacity(200)];
            let first_mode = (run + first / 200 + round) % 2;
            for mode in [first_mode, 1 - first_mode] {
                let start = Instant::now();
                for text in &texts {
                    let output = if mode == 0 {
                        fold(black_box(text))
                    } else {
                        let mut output = String::with_capacity(text.len());
                        super::fold_into(black_box(text), &mut output);
                        output
                    };
                    outputs[mode].push(black_box(output));
                }
                let elapsed = start.elapsed().as_secs_f64() * 1000.;
                if round != 0 {
                    samples[mode].push(elapsed);
                } else {
                    capacity_bytes[mode] +=
                        outputs[mode].iter().map(String::capacity).sum::<usize>();
                }
            }
            assert_eq!(outputs[0], outputs[1]);
        }
    }
    assert_eq!(text_bytes, 264_842_858);
    println!(
        "{}",
        serde_json::json!({"phase":"intake_fold_probe", "run":run,
            "documents":10000,"text_bytes":text_bytes,"group_size":200,
            "warmups_per_group":1,"timed_rounds_per_group":4,
            "scalar_ms":samples[0],"fast_ms":samples[1],
            "output_capacity_bytes_summed":capacity_bytes,
            "byte_equal":true,"not_process_ram":true})
    );
    Ok(())
}

#[cfg(feature = "bounded-engine")]
fn word(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().unwrap())
}

#[cfg(feature = "bounded-engine")]
#[test]
#[ignore = "same-binary post-write attribution;requires the owned fixed FDB slot reset"]
fn existing_corpus_postwrite_control() -> Result<(), Box<dyn std::error::Error>> {
    use crate::{Database, Guard, Mutation, Query, StoredDocument};
    use serde_json::{Value, json};
    use std::{
        fs::File,
        hint::black_box,
        io::{BufReader, Read, Write},
        path::Path,
        sync::atomic::Ordering,
        time::Instant,
    };
    let mode = std::env::var("FDB_INTAKE_MODE")?;
    INTAKE_MODE.store(
        match mode.as_str() {
            "scalar" => 0,
            "fast" => 1,
            "rounded" => 2,
            _ => panic!("invalid intake mode"),
        },
        Ordering::Relaxed,
    );
    let seed: usize = std::env::var("FDB_INTAKE_SEED")?.parse()?;
    let path = Path::new("data/benchmark-work/fdb");
    assert!(!path.exists(), "Reset only the owned fixed FDB slot first");
    let mut source = BufReader::with_capacity(
        65536,
        File::open("data/rkyv-corpus-20261009/json.payload-bench")?,
    );
    let mut header = [0; 20];
    source.read_exact(&mut header)?;
    assert_eq!(&header[..8], b"PLOADB01");
    assert_eq!(crc32fast::hash(&header[..16]), word(&header[16..]));
    let mut db = Database::open(path)?;
    for batch in 0..2 {
        let mut changes = Vec::with_capacity(500);
        for id in batch * 500..(batch + 1) * 500 {
            let mut frame = [0; 16];
            source.read_exact(&mut frame)?;
            assert_eq!(crc32fast::hash(&frame[..12]), word(&frame[12..]));
            assert_eq!(word(&frame[8..12]) as usize, id);
            let len = word(&frame[..4]) as usize;
            assert!(len <= 4 * 1024 * 1024);
            let mut bytes = vec![0; len];
            source.read_exact(&mut bytes)?;
            assert_eq!(crc32fast::hash(&bytes), word(&frame[4..8]));
            let row: StoredDocument = serde_json::from_slice(&bytes)?;
            assert_eq!(row.document.key.id, format!("{id:010}"));
            assert_eq!(row.version, batch as u64 + 1);
            changes.push(Mutation::Put {
                document: row.document,
                guard: Guard::Missing,
            });
        }
        assert_eq!(db.commit(changes)?, batch as u64 + 1);
    }
    let stats = db.stats()?;
    assert_eq!(
        (stats.sequence, stats.documents, stats.delta_keys),
        (2, 1000, 1000)
    );
    let texts = [
        "the",
        "quixote",
        "he",
        "the man",
        "zx",
        "transubstantiation",
    ];
    let queries = texts.map(|text| {
        let mut query = Query::new("shared");
        query.collection = Some("texts".into());
        query.text = text.into();
        query.limit = 50;
        query
    });
    let stages = ["search", "page_to_value", "response_value", "format_to_vec"];
    let mut samples = vec![vec![Vec::new(); queries.len()]; stages.len()];
    let mut checks = vec![Value::Null; queries.len()];
    for round in 0..=200 {
        let mut order: Vec<_> = (0..queries.len()).collect();
        if !(round + seed).is_multiple_of(2) {
            order.reverse();
        }
        for index in order {
            let start = Instant::now();
            let page = black_box(db.search(black_box(&queries[index]))?);
            let search_ms = start.elapsed().as_secs_f64() * 1000.;
            let start = Instant::now();
            let value = black_box(serde_json::to_value(page)?);
            let value_ms = start.elapsed().as_secs_f64() * 1000.;
            let start = Instant::now();
            let response = black_box(json!({"ok":true,"result":value}));
            // CLI's match-arm result is dropped before formatting the response.
            drop(value);
            let response_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut encoded = Vec::new();
            let start = Instant::now();
            writeln!(&mut encoded, "{response}")?;
            black_box(&encoded);
            let format_ms = start.elapsed().as_secs_f64() * 1000.;
            assert_eq!(response["result"]["sequence"], 2);
            let current = json!({
                "total":response["result"]["total"],
                "keys_versions":response["result"]["hits"].as_array().unwrap().iter()
                    .map(|r| json!([r["document"]["key"],r["version"]])).collect::<Vec<_>>(),
                "encoded_bytes":encoded.len(),"encoded_crc":crc32fast::hash(&encoded)
            });
            if round == 0 {
                checks[index] = current;
            } else {
                assert_eq!(current, checks[index]);
                for (stage, elapsed) in [search_ms, value_ms, response_ms, format_ms]
                    .into_iter()
                    .enumerate()
                {
                    samples[stage][index].push(elapsed);
                }
            }
        }
    }
    println!(
        "{}",
        json!({"phase":"intake_causal", "mode":mode,"seed":seed,
            "stats":stats,"queries":texts,"stages":stages,"samples_ms":samples,
            "checks":checks,"repeats":200,"test_only_intake_switch":true,
            "limits":"Same release test binary/search path;normal speed must be measured separately;Value/Display pipeline matches CLI preparation but output goes to fresh Vec,not pipe;CRC/checks/drops outside stage timing;no RAM claim"})
    );
    Ok(())
}
