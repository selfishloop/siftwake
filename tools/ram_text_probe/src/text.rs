//! Volatile text prototype only; no engine or persistent format uses this.
use lz4_flex::block;

const MAX_TEXT: usize = 2 * 1024 * 1024;
enum Body {
    Raw(String),
    Lz4(Box<[u8]>),
}
pub struct Text {
    body: Body,
    length: u32,
    checksum: u32,
}
type Result<T> = std::result::Result<T, &'static str>;
impl Text {
    pub fn new(text: String) -> Result<Self> {
        if text.len() > MAX_TEXT {
            return Err("input text limit");
        }
        let length = text.len() as u32;
        let checksum = crc32fast::hash(text.as_bytes());
        let body = if text.len() >= 1024 {
            let encoded = block::compress(text.as_bytes());
            // Savings must exceed the extra metadata; no dictionary trained on
            // this corpus, no inter-document sharing, no word-specific paths.
            if encoded.len().saturating_add(64) < text.len() {
                Body::Lz4(encoded.into_boxed_slice())
            } else {
                Body::Raw(text)
            }
        } else {
            Body::Raw(text)
        };
        Ok(Self {
            body,
            length,
            checksum,
        })
    }
    pub fn compressed(&self) -> bool {
        matches!(self.body, Body::Lz4(_))
    }
    pub fn heap_bytes(&self) -> usize {
        match &self.body {
            Body::Raw(text) => text.capacity(),
            Body::Lz4(bytes) => bytes.len(),
        }
    }
    pub fn copy_into(&self, output: &mut String, check_crc: bool) -> Result<()> {
        // An error consumes old output and leaves no partial/unchecked string.
        let mut previous = std::mem::take(output);
        if self.length as usize > MAX_TEXT {
            return Err("decoded text limit");
        }
        match &self.body {
            Body::Raw(text) => {
                if text.len() != self.length as usize {
                    return Err("raw text length");
                }
                if check_crc && crc32fast::hash(text.as_bytes()) != self.checksum {
                    return Err("raw text checksum");
                }
                // Input is already a valid String, just like the control.
                previous.clear();
                previous.push_str(text);
                *output = previous;
            }
            Body::Lz4(encoded) => {
                let mut bytes = previous.into_bytes();
                bytes.resize(self.length as usize, 0);
                let written =
                    block::decompress_into(encoded, &mut bytes).map_err(|_| "lz4 bounds/schema")?;
                if written != self.length as usize {
                    return Err("decoded text length");
                }
                if check_crc && crc32fast::hash(&bytes) != self.checksum {
                    return Err("decoded text checksum");
                }
                // Includes UTF-8 validation and transfers the same allocation.
                *output = String::from_utf8(bytes).map_err(|_| "decoded text utf8")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_empty_and_small_raw_fallback_roundtrip_without_changing_input() {
        for original in ["", "İŞIK ıÇĞÖÜ 東京 🦀", "ȺΣẞK é—\u{1f88}"] {
            let stored = Text::new(original.to_owned()).unwrap();
            assert!(!stored.compressed());
            for checked in [false, true] {
                let mut result = "old value".repeat(100);
                stored.copy_into(&mut result, checked).unwrap();
                assert_eq!(result, original);
            }
        }
    }
    #[test]
    fn compressed_text_reuses_exact_output_and_handles_length_changes() {
        let original = "İŞIK 東京 🦀 Başlık\n".repeat(4000);
        let stored = Text::new(original.clone()).unwrap();
        assert!(stored.compressed());
        let mut result = String::with_capacity(original.len());
        for checked in [true, false, true] {
            result.clear();
            let allocation = result.as_ptr();
            stored.copy_into(&mut result, checked).unwrap();
            assert_eq!(result, original);
            assert_eq!(result.as_ptr(), allocation);
            Text::new("short".into())
                .unwrap()
                .copy_into(&mut result, checked)
                .unwrap();
            assert_eq!(result, "short");
        }
    }
    #[test]
    fn truncation_corruption_bounds_and_utf8_fail_with_empty_output() {
        let original = "İstanbul 東京 🦀 Boundary".repeat(80);
        let stored = Text::new(original.clone()).unwrap();
        let Body::Lz4(encoded) = stored.body else {
            panic!("expected compressed body");
        };
        for cut in 0..encoded.len() {
            let bad = Text {
                body: Body::Lz4(encoded[..cut].into()),
                length: stored.length,
                checksum: stored.checksum,
            };
            let mut result = "old result".to_owned();
            assert!(bad.copy_into(&mut result, true).is_err(), "cut={cut}");
            assert!(result.is_empty());
        }
        for index in 0..encoded.len() {
            let mut corrupt = encoded.to_vec();
            corrupt[index] ^= 1;
            let bad = Text {
                body: Body::Lz4(corrupt.into()),
                length: stored.length,
                checksum: stored.checksum,
            };
            let mut result = "old result".to_owned();
            // LZ4 may have unused token bits. Such a change is admissible only
            // if the checked decoded text is still byte-identical.
            if bad.copy_into(&mut result, true).is_ok() {
                assert_eq!(result, original, "byte={index}");
            } else {
                assert!(result.is_empty());
            }
        }
        for length in [0, stored.length - 1, stored.length + 1, MAX_TEXT as u32 + 1] {
            let bad = Text {
                body: Body::Lz4(encoded.clone()),
                length,
                checksum: stored.checksum,
            };
            let mut result = "old result".into();
            assert!(bad.copy_into(&mut result, true).is_err());
            assert!(result.is_empty());
        }
        let bad = Text {
            body: Body::Lz4(block::compress(&[255; 2048]).into()),
            length: 2048,
            checksum: crc32fast::hash(&[255; 2048]),
        };
        let mut result = "old".into();
        assert_eq!(bad.copy_into(&mut result, true), Err("decoded text utf8"));
        assert!(result.is_empty());
    }
    #[test]
    fn oversized_input_and_raw_corruption_do_not_escape_validation() {
        assert!(Text::new("a".repeat(MAX_TEXT + 1)).is_err());
        let mut stored = Text::new("original".into()).unwrap();
        if let Body::Raw(raw) = &mut stored.body {
            *raw = "modified".into();
        }
        let mut result = "old".into();
        assert!(stored.copy_into(&mut result, true).is_err());
        assert!(result.is_empty());
    }
}
