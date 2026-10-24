//! Transient scalar keys only: persistent/query terms remain the same strings.
use std::collections::BTreeSet;

const SCALAR: u64 = (1 << 21) - 1;
const PREFIX: u64 = (1 << 42) - 1;
const CACHE_LEN: usize = 1024;
// Bound duplicate-key scratch independently of the input length. Larger bodies
// spill sorted, deduplicated chunks to the existing numeric tree.
const PENDING_LIMIT: usize = 8192;

fn bucket(key: u64) -> usize {
    (key.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 54) as usize
}

#[inline(never)]
pub(super) fn extract(text: &str) -> BTreeSet<String> {
    // Three 21-bit scalars fit in 63 bits, so MAX cannot be a real key.
    // Store the full witness: collisions only cost a tree lookup, never a gram.
    let mut recent = [u64::MAX; CACHE_LEN];
    let mut unique = BTreeSet::new();
    let mut pending = Vec::with_capacity(CACHE_LEN);
    let mut key = 0u64;
    let mut prefix = 0u8;
    #[cfg(feature = "diagnostic-timing")]
    let mut work = crate::timing::GramWork::default();
    for c in text.chars() {
        // A whitespace scalar breaks every crossing gram. Test it once rather
        // than retesting the preceding two scalars at each position.
        if c.is_whitespace() {
            key = 0;
            prefix = 0;
            continue;
        }
        key = ((key & PREFIX) << 21) | u64::from(c);
        if prefix < 2 {
            prefix += 1;
            continue;
        }
        #[cfg(feature = "diagnostic-timing")]
        {
            work.windows += 1;
        }
        let cached = &mut recent[bucket(key)];
        if *cached == key {
            #[cfg(feature = "diagnostic-timing")]
            {
                work.hits += 1;
            }
            continue;
        }
        *cached = key;
        pending.push(key);
        if pending.len() == PENDING_LIMIT {
            pending.sort_unstable();
            pending.dedup();
            #[cfg(feature = "diagnostic-timing")]
            {
                work.tree_probes += pending.len() as u64;
            }
            unique.extend(pending.drain(..));
        }
    }
    pending.sort_unstable();
    pending.dedup();
    if unique.is_empty() {
        #[cfg(feature = "diagnostic-timing")]
        {
            work.distinct = pending.len() as u64;
        }
        return strings(pending.into_iter());
    }
    #[cfg(feature = "diagnostic-timing")]
    {
        work.tree_probes += pending.len() as u64;
    }
    unique.extend(pending);
    #[cfg(feature = "diagnostic-timing")]
    {
        work.distinct = unique.len() as u64;
    }
    strings(unique.into_iter())
}

fn strings(keys: impl Iterator<Item = u64>) -> BTreeSet<String> {
    // Numeric order is scalar-tuple order. String iteration/serialization order
    // is unchanged; no packed key or cache is written to disk or kept in Index.
    keys.map(|key| {
        [42, 21, 0]
            .into_iter()
            .map(|shift| char::from_u32(((key >> shift) & SCALAR) as u32).unwrap())
            .collect()
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pre-change extractor, retained only as an exact same-binary probe control.
    #[inline(never)]
    fn tree_extract(text: &str) -> BTreeSet<String> {
        let mut recent = [u64::MAX; CACHE_LEN];
        let mut unique = BTreeSet::new();
        let mut key = 0u64;
        let mut prefix = 0u8;
        for c in text.chars() {
            if c.is_whitespace() {
                key = 0;
                prefix = 0;
                continue;
            }
            key = ((key & PREFIX) << 21) | u64::from(c);
            if prefix < 2 {
                prefix += 1;
                continue;
            }
            let cached = &mut recent[bucket(key)];
            if *cached != key {
                *cached = key;
                unique.insert(key);
            }
        }
        strings(unique.into_iter())
    }

    fn reference(text: &str) -> BTreeSet<String> {
        text.chars()
            .collect::<Vec<_>>()
            .windows(3)
            .filter(|chars| chars.iter().all(|c| !c.is_whitespace()))
            .map(|chars| chars.iter().collect())
            .collect()
    }

    #[test]
    fn packed_grams_preserve_unicode_whitespace_nulls_and_sorted_strings() {
        for text in [
            "",
            "a",
            "ab",
            "abc",
            "aaaaaa",
            "a\0\0\0b",
            "İŞIK İstanbul ışık",
            "東京🦀ⱥΣ\u{301}ǅ",
            "\u{10ffff}\u{10ffff}\u{10ffff}",
            "abc\tdef\nghi\rjkl\u{b}mno\u{c}pqr\u{85}stu\u{a0}vwx\u{1680}yz0",
            "abc\u{2000}def\u{2007}ghi\u{2028}jkl\u{2029}mno\u{202f}pqr\u{205f}stu\u{3000}vwx",
            "ab\u{200b}cd\u{feff}ef",
        ] {
            assert_eq!(extract(text), reference(text), "{text:?}");
            let padded = format!("{}{text}", " ".repeat(1024));
            assert_eq!(super::super::grams(&padded), reference(&padded));
            let mut body_cache = super::super::BodyCache::default();
            assert_eq!(
                body_cache.get(&padded).grams,
                reference(&super::super::fold(&padded))
            );
        }
        let mut state = 0x8742_35ab_18cd_6729u64;
        let mut text = String::new();
        for round in 0..48 {
            text.clear();
            for _ in 0..(round * 83 + 3) {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let c = match state % 5 {
                    0 => ' ',
                    1 => '\u{a0}',
                    2 => char::from_u32(33 + (state % 90) as u32).unwrap(),
                    _ => char::from_u32((state % 0x110000) as u32).unwrap_or('\0'),
                };
                text.push(c);
            }
            assert_eq!(extract(&text), reference(&text));
        }
    }

    #[test]
    fn hash_collision_eviction_never_drops_a_distinct_or_repeated_gram() {
        let mut slots = [None; CACHE_LEN];
        let mut pair = None;
        for c in 33u32..1024 {
            let key = (u64::from('x') << 42) | (u64::from('y') << 21) | u64::from(c);
            let slot = &mut slots[bucket(key)];
            if let Some(old) = slot {
                pair = Some((*old, c));
                break;
            }
            *slot = Some(c);
        }
        let (a, b) = pair.expect("expected direct-map collision");
        let text = format!(
            "xy{} xy{} ",
            char::from_u32(a).unwrap(),
            char::from_u32(b).unwrap()
        )
        .repeat(1000);
        assert_eq!(extract(&text), reference(&text));
    }

    #[test]
    fn bounded_pending_spills_preserve_unique_and_repeated_unicode_grams() {
        let scalars: String = (0x10000..0x14000).filter_map(char::from_u32).collect();
        for text in [
            scalars.clone(),
            format!("{scalars}\0\0\0{scalars}\u{a0}{scalars}\u{10ffff}\u{10ffff}\u{10ffff}"),
        ] {
            let expected = reference(&text);
            assert!(expected.len() > PENDING_LIMIT);
            assert_eq!(extract(&text), expected);
            assert_eq!(extract(&text), tree_extract(&text));
        }
    }

    #[test]
    #[ignore = "read-only gram extraction probe over the permitted existing first10k"]
    fn existing_corpus_gram_probe() -> Result<(), Box<dyn std::error::Error>> {
        use std::{fs::File, hint::black_box, io::Read, time::Instant};

        fn word(bytes: &[u8]) -> u32 {
            u32::from_le_bytes(bytes.try_into().unwrap())
        }
        fn misses(text: &str) -> usize {
            let mut recent = [u64::MAX; CACHE_LEN];
            let mut key = 0u64;
            let mut prefix = 0u8;
            let mut count = 0;
            for c in text.chars() {
                if c.is_whitespace() {
                    key = 0;
                    prefix = 0;
                    continue;
                }
                key = ((key & PREFIX) << 21) | u64::from(c);
                if prefix < 2 {
                    prefix += 1;
                    continue;
                }
                let cached = &mut recent[bucket(key)];
                if *cached != key {
                    *cached = key;
                    count += 1;
                }
            }
            count
        }

        let run: usize = std::env::var("FDB_GRAM_PROBE_RUN")
            .unwrap_or_else(|_| "0".into())
            .parse()?;
        let mut source = File::open("data/rkyv-corpus-20261009/json.payload-bench")?;
        let mut header = [0u8; 20];
        source.read_exact(&mut header)?;
        assert_eq!(&header[..8], b"PLOADB01");
        assert_eq!(crc32fast::hash(&header[..16]), word(&header[16..]));
        let mut samples = [Vec::new(), Vec::new()];
        let mut text_bytes = 0;
        let mut max_misses = 0;
        let mut max_distinct = 0;
        let mut spill_documents = 0;
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
                let folded = super::super::fold_for_intake(&row.document.text);
                let count = misses(&folded);
                max_misses = max_misses.max(count);
                spill_documents += usize::from(count >= PENDING_LIMIT);
                texts.push(folded);
            }
            // Warmup and full output equality are outside the timed extraction.
            for text in &texts {
                let old = tree_extract(text);
                max_distinct = max_distinct.max(old.len());
                assert_eq!(extract(text), old);
            }
            for round in 0..2 {
                let first_mode = (run + first / 200 + round) % 2;
                for mode in [first_mode, 1 - first_mode] {
                    let start = Instant::now();
                    for text in &texts {
                        let output = if mode == 0 {
                            tree_extract(black_box(text))
                        } else {
                            extract(black_box(text))
                        };
                        black_box(output);
                    }
                    samples[mode].push(start.elapsed().as_nanos() as u64);
                }
            }
        }
        assert_eq!(text_bytes, 264_842_858);
        println!(
            "{}",
            serde_json::json!({"phase":"gram_probe","run":run,"documents":10000,
                "text_bytes":text_bytes,"group_size":200,"timed_rounds_per_group":2,
                "tree_ns":samples[0],"bounded_sort_ns":samples[1],
                "max_cache_misses_per_document":max_misses,"max_distinct_per_document":max_distinct,
                "spill_documents":spill_documents,"pending_key_limit":PENDING_LIMIT,
                "pending_element_bytes_bound":PENDING_LIMIT * std::mem::size_of::<u64>(),
                "exact_grams_equal":true,"not_process_ram":true,
                "scope":"folded body gram extraction+returned String set+drop; no fold/input CRC/JSON/DB"})
        );
        Ok(())
    }
}
