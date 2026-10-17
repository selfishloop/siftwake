//! Exact raw-text search for short needles whose letters cannot come from
//! non-ASCII scalar folding. Unlike the speculative long-needle path, absence
//! is conclusive. Payload CRC/schema/identity must already have been checked.

// Turkish mappings contribute c/g/i/o/s/u, Kelvin sign contributes k. The
// exhaustive scalar test guards this set when Rust's Unicode tables change.
const UNICODE_ASCII: &[u8] = b"cgikosu";

pub(super) fn eligible(term: &str) -> bool {
    (1..=2).contains(&term.len())
        && term
            .bytes()
            .all(|b| b.is_ascii_lowercase() && !UNICODE_ASCII.contains(&b))
}

// Keep the four spelling scans out of the general verifier's hot code.
#[inline(never)]
pub(super) fn contains(text: &str, term: &str) -> bool {
    debug_assert!(eligible(term));
    // Enumerate all 2^n ASCII case spellings, including lower/UPPER mixed
    // pairs. Never normalize/copy the body or allocate a result cache.
    let mut spelling = [0u8; 2];
    for mask in 0..1 << term.len() {
        for (n, byte) in term.bytes().enumerate() {
            spelling[n] = if mask & (1 << n) == 0 {
                byte
            } else {
                byte.to_ascii_uppercase()
            };
        }
        let needle = std::str::from_utf8(&spelling[..term.len()]).unwrap();
        if text.contains(needle) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::fold;

    #[test]
    fn every_non_ascii_scalar_folds_without_eligible_ascii_letters() {
        let mut bytes = [0; 4];
        let mut contributions = [false; 128];
        for scalar in (128..=0x10ffff).filter_map(char::from_u32) {
            for byte in fold(scalar.encode_utf8(&mut bytes)).bytes() {
                if byte.is_ascii() {
                    contributions[byte as usize] = true;
                }
            }
        }
        for (byte, present) in contributions.into_iter().enumerate() {
            if present {
                assert!(UNICODE_ASCII.contains(&(byte as u8)), "unhandled {byte}");
            }
        }
        // Canonical folding never removes a scalar, so an eligible ASCII
        // substring cannot jump across an intervening non-ASCII character.
        for scalar in (128..=0x10ffff).filter_map(char::from_u32) {
            assert!(!fold(scalar.encode_utf8(&mut bytes)).is_empty());
        }
    }

    #[test]
    fn all_short_spellings_and_unicode_boundaries_match_canonical_fold() {
        for a in b'a'..=b'z' {
            for b in b'a'..=b'z' {
                let pair = [a, b];
                for len in 1..=2 {
                    let needle = std::str::from_utf8(&pair[..len]).unwrap();
                    if !eligible(needle) {
                        continue;
                    }
                    for mask in 0..1 << len {
                        let mut spelling = pair;
                        for (n, byte) in spelling[..len].iter_mut().enumerate() {
                            if mask & (1 << n) != 0 {
                                byte.make_ascii_uppercase();
                            }
                        }
                        let spelling = std::str::from_utf8(&spelling[..len]).unwrap();
                        for padding in [0, 7, 1023, 4095] {
                            for text in [
                                format!("{}🦀東京İK{spelling}Ş", " ".repeat(padding)),
                                format!("{}İŞIKK", " ".repeat(padding)),
                                format!("{}{}İ{}", " ".repeat(padding), a as char, b as char),
                            ] {
                                assert_eq!(contains(&text, needle), fold(&text).contains(needle));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ambiguous_unicode_letters_and_unsupported_needles_use_fallback() {
        for needle in ["", "i", "ki", "si", "co", "gu", "zxz", "ZX", "🦀", "12"] {
            assert!(!eligible(needle));
        }
        for (text, needle) in [("İı", "i"), ("K", "k"), ("ŞÇĞÖÜ", "s"), ("İŞ", "is")] {
            assert!(fold(text).contains(needle));
            assert!(!eligible(needle));
        }
    }
}
