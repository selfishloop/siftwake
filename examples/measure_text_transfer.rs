//! Read-only, resident first-page text transfer probe; no database writes.
#[path = "../tools/bench_support/memory_corpus.rs"]
#[allow(dead_code)]
mod input;
use serde_json::json;
use std::{hint::black_box, time::Instant};

fn main() -> input::Result<()> {
    let mut source = input::Input::open()?;
    let texts: Vec<_> = (0..200)
        .map(|_| source.read().map(|s| s.document.text))
        .collect::<Result<_, _>>()?;
    let bytes: usize = texts.iter().map(String::len).sum();
    let mut reused: Vec<_> = texts.iter().map(|s| s.to_owned()).collect();
    for iteration in 0..65 {
        // Alternate order. Allocate vector backing outside each bracket.
        let mut allocated = Vec::with_capacity(texts.len());
        let mut fresh = Vec::with_capacity(texts.len());
        let mut row = json!({"phase":"text_transfer","iteration":iteration,"warmup":iteration<5,"documents":texts.len(),"text_bytes":bytes});
        for mode in if iteration % 2 == 0 {
            [0, 1, 2]
        } else {
            [2, 1, 0]
        } {
            match mode {
                0 => {
                    let start = Instant::now();
                    for text in &texts {
                        fresh.push(black_box(text.as_str()).to_owned());
                    }
                    row["fresh_alloc_and_copy_ms"] = json!(start.elapsed().as_secs_f64() * 1000.);
                    assert_eq!(fresh, texts);
                }
                1 => {
                    let start = Instant::now();
                    for text in &texts {
                        allocated.push(String::with_capacity(black_box(text.len())));
                    }
                    row["reserve_only_ms"] = json!(start.elapsed().as_secs_f64() * 1000.);
                    let start = Instant::now();
                    for (out, text) in allocated.iter_mut().zip(&texts) {
                        out.push_str(black_box(text));
                    }
                    row["initial_copy_after_reserve_ms"] =
                        json!(start.elapsed().as_secs_f64() * 1000.);
                    assert_eq!(allocated, texts);
                }
                _ => {
                    let start = Instant::now();
                    for (out, text) in reused.iter_mut().zip(&texts) {
                        out.clear();
                        out.push_str(black_box(text));
                    }
                    row["reused_clear_and_copy_ms"] = json!(start.elapsed().as_secs_f64() * 1000.);
                    assert_eq!(reused, texts);
                }
            }
        }
        black_box((&fresh, &allocated, &reused));
        println!("{row}");
    }
    println!(
        "{}",
        json!({"phase":"limits","cache":"resident source texts; warmed allocator/OS; no eviction","includes_schema_crc_io":false,"reserve_excludes_most_first_touch":true,"initial_copy_includes_first_touch":true,"not_pure_memcpy_attribution":true,"memory_not_measured":true,"new_corpus_or_database":false})
    );
    Ok(())
}
