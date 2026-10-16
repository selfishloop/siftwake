//! Transient scalar keys only: persistent/query terms remain the same strings.
use std::collections::BTreeSet;

const SCALAR: u64 = (1 << 21) - 1;
const PREFIX: u64 = (1 << 42) - 1;
const CACHE_LEN: usize = 1024;

fn bucket(key: u64) -> usize {
    (key.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 54) as usize
}

#[inline(never)]
pub(super) fn extract(text: &str) -> BTreeSet<String> {
    // Three 21-bit scalars fit in 63 bits, so MAX cannot be a real key.
    // Store the full witness: collisions only cost a tree lookup, never a gram.
    let mut recent = [u64::MAX; CACHE_LEN];
    let mut unique = BTreeSet::new();
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
        unique.insert(key);
    }
    #[cfg(feature = "diagnostic-timing")]
    {
        work.distinct = unique.len() as u64;
    }
    // Numeric order is scalar-tuple order. String iteration/serialization order
    // is unchanged; no packed key or cache is written to disk or kept in Index.
    unique
        .into_iter()
        .map(|key| {
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
}
