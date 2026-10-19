//! Read-only first10k probe. At most200 input texts; no DB/corpus creation.
#[path = "../../bench_support/memory_corpus.rs"]
#[allow(dead_code)]
mod input;
mod text;
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;
use text::Text;

const MODES: [&str; 6] = [
    "raw_fresh",
    "lz4_utf8_fresh",
    "lz4_crc_utf8_fresh",
    "raw_reused",
    "lz4_utf8_reused",
    "lz4_crc_utf8_reused",
];
fn main() -> input::Result<()> {
    let run: usize = std::env::args()
        .nth(1)
        .ok_or("run seed required")?
        .parse()?;
    let mut source = input::Input::open()?;
    let mut total_raw = 0;
    let mut total_packed = 0;
    let mut compressed = 0;
    let mut encode_ns = 0;
    for first in (0..10_000).step_by(200) {
        let originals: Vec<_> = (0..200)
            .map(|_| source.read().map(|row| row.document.text))
            .collect::<input::Result<_>>()?;
        // Caller copy construction is outside encoding, like an owned commit.
        let intake = originals.clone();
        let raw_bytes = intake.iter().map(String::capacity).sum::<usize>()
            + intake.capacity() * std::mem::size_of::<String>();
        let start = Instant::now();
        let packed: Vec<_> = intake
            .into_iter()
            .map(Text::new)
            .collect::<Result<_, _>>()?;
        let encoded_ns = start.elapsed().as_nanos() as u64;
        encode_ns += encoded_ns;
        let packed_bytes = packed.iter().map(Text::heap_bytes).sum::<usize>()
            + packed.capacity() * std::mem::size_of::<Text>();
        total_raw += raw_bytes;
        total_packed += packed_bytes;
        compressed += packed.iter().filter(|row| row.compressed()).count();
        for (row, original) in packed.iter().zip(&originals) {
            let mut restored = String::new();
            row.copy_into(&mut restored, true)?;
            assert_eq!(&restored, original);
        }
        println!(
            "{}",
            json!({"phase":"group","first":first,"documents":200,"text_bytes":originals.iter().map(String::len).sum::<usize>(),"raw_retained_bytes":raw_bytes,"packed_retained_bytes":packed_bytes,"raw_inline_bytes":std::mem::size_of::<String>(),"packed_inline_bytes":std::mem::size_of::<Text>(),"compressed_records":packed.iter().filter(|r|r.compressed()).count(),"encode_crc_compact_ns":encoded_ns,"all_payloads_byte_equal":true,"not_process_memory":true})
        );
        for count in [1, 50, 200] {
            let offsets: Vec<_> = if count == 50 {
                vec![0, 50, 100, 150]
            } else if count == 1 {
                vec![(first / 200 * 37 + run * 11) % 200]
            } else {
                vec![0]
            };
            for offset in offsets {
                let raw = &originals[offset..offset + count];
                let encoded = &packed[offset..offset + count];
                let mut reuse: Vec<Vec<String>> = (0..3).map(|_| raw.to_vec()).collect();
                let mut timings: [Vec<u64>; 6] = std::array::from_fn(|_| Vec::with_capacity(8));
                for iteration in 0..10 {
                    let reverse = !(run + first / 200 + iteration).is_multiple_of(2);
                    for step in 0..6 {
                        let mode = if reverse { 5 - step } else { step };
                        let mut fresh = if mode < 3 {
                            Vec::with_capacity(count)
                        } else {
                            Vec::new()
                        };
                        let start = Instant::now();
                        if mode < 3 {
                            if mode == 0 {
                                for source in raw {
                                    fresh.push(black_box(source.as_str()).to_owned());
                                }
                            } else {
                                for source in encoded {
                                    let mut output = String::new();
                                    black_box(source).copy_into(&mut output, mode == 2)?;
                                    fresh.push(output);
                                }
                            }
                        } else {
                            let target = &mut reuse[mode - 3];
                            if mode == 3 {
                                for (out, source) in target.iter_mut().zip(raw) {
                                    out.clear();
                                    out.push_str(black_box(source.as_str()));
                                }
                            } else {
                                for (out, source) in target.iter_mut().zip(encoded) {
                                    black_box(source).copy_into(out, mode == 5)?;
                                }
                            }
                        }
                        let elapsed_ns = start.elapsed().as_nanos() as u64;
                        let result = if mode < 3 { &fresh } else { &reuse[mode - 3] };
                        assert_eq!(result, raw);
                        black_box(result);
                        if iteration >= 2 {
                            timings[mode].push(elapsed_ns);
                        }
                    }
                }
                println!(
                    "{}",
                    json!({"phase":"page","first":first+offset,"count":count,"bytes":raw.iter().map(String::len).sum::<usize>(),"modes":MODES.iter().zip(timings).map(|(mode,samples)|json!({"mode":mode,"samples_ns":samples})).collect::<Vec<_>>()})
                );
            }
        }
    }
    assert_eq!(source.text_bytes, 264_842_858);
    println!(
        "{}",
        json!({"phase":"result","run":run,"documents":10000,"text_bytes":source.text_bytes,"raw_retained_sum_bytes":total_raw,"packed_retained_sum_bytes":total_packed,"compressed_records":compressed,"encode_crc_compact_ns":encode_ns,"all_payloads_byte_equal":true,"maximum_input_group":200,"retained_sums_are_not_simultaneous_or_process_memory":true,"cache":"resident group;two warmups per mode/page;warmed allocator/OS;no eviction","includes":"materialization to owned UTF-8 String;fresh output allocation or reused caller buffer;optional CRC separately","excludes":"JSON corpus decode+CRC,input copying,fields,title,links,folded search text,normalization,posting lookup,WAL,maintenance,disk IO;no DB engine timing"})
    );
    Ok(())
}
