//! Bounded directory dispatch. SEG03/04 retain checked JSON entry framing;
//! the explicit SEG05 experiment uses the compact typed directory reader.
use super::*;
use std::io::{BufRead, BufReader, Read};

const ENTRY_LIMIT: usize = 256 * 1024;
fn corrupt(message: &str) -> Error {
    Error::Corrupt(format!("streaming directory: {message}"))
}

use crate::json_stream::Input;

#[cfg(feature = "bounded-engine")]
fn is_compact(file: &File) -> Result<bool> {
    let mut magic = [0; 8];
    read_at(file, 0, &mut magic)?;
    Ok(&magic == super::compact::MAGIC)
}

pub(super) struct Parsed {
    pub records: Vec<PackedRecord>,
    pub groups: Vec<(String, String)>,
    pub ids: String,
    pub postings: BTreeMap<Term, Posting>,
    pub pages: Vec<super::term_pages::Fence>,
    pub collections: Vec<(String, String)>,
    pub bloom: super::term_pages::Bloom,
    #[cfg(feature = "bounded-engine")]
    pub universal: super::covering::Universal,
    pub live: usize,
    pub entries: u64,
}

pub(super) fn empty() -> Parsed {
    Parsed {
        records: Vec::new(),
        groups: Vec::new(),
        ids: String::new(),
        postings: BTreeMap::new(),
        pages: Vec::new(),
        collections: Vec::new(),
        bloom: super::term_pages::Bloom::default(),
        #[cfg(feature = "bounded-engine")]
        universal: super::covering::Universal::default(),
        live: 0,
        entries: 0,
    }
}

pub(super) fn read(file: &mut File, offset: u64, len: u64, sequence: u64) -> Result<Parsed> {
    #[cfg(feature = "bounded-engine")]
    if is_compact(file)? {
        return super::compact::read(file, offset, len, sequence, pack);
    }
    file.seek(SeekFrom::Start(offset))?;
    let reader = BufReader::with_capacity(64 * 1024, file.take(len));
    parse_entries(reader, offset, sequence, true, pack)
}
#[cfg(test)]
fn parse(reader: impl BufRead, data_end: u64, sequence: u64) -> Result<Parsed> {
    parse_entries(reader, data_end, sequence, false, pack)
}
pub(super) fn pack(r: &Record, parsed: &mut Parsed) -> Result<()> {
    let new_group = parsed
        .groups
        .last()
        .is_none_or(|(s, c)| s != &r.key.scope || c != &r.key.collection);
    if new_group {
        parsed
            .groups
            .push((r.key.scope.clone(), r.key.collection.clone()));
    }
    let group = u32::try_from(parsed.groups.len() - 1).map_err(|_| corrupt("group capacity"))?;
    let id_offset = u32::try_from(parsed.ids.len()).map_err(|_| corrupt("ID pool capacity"))?;
    let id_len = r.key.id.len() as u32;
    parsed.ids.push_str(&r.key.id);
    parsed.records.push(PackedRecord {
        offset: r.offset,
        version: r.version,
        group,
        id_offset,
        id_len,
        len: r.len,
        checksum: r.checksum,
        deleted: r.deleted,
    });
    Ok(())
}
#[cfg(feature = "experimental-streaming-metadata")]
pub(super) fn read_with(
    file: &mut File,
    offset: u64,
    len: u64,
    sequence: u64,
    mut accept: impl FnMut(&Record) -> Result<()>,
) -> Result<Parsed> {
    #[cfg(feature = "bounded-engine")]
    if is_compact(file)? {
        return super::compact::read(file, offset, len, sequence, |r, _| accept(r));
    }
    file.seek(SeekFrom::Start(offset))?;
    let reader = BufReader::with_capacity(64 * 1024, file.take(len));
    parse_entries(reader, offset, sequence, true, |record, _| accept(record))
}
fn parse_entries(
    reader: impl BufRead,
    data_end: u64,
    sequence: u64,
    paged_terms: bool,
    mut accept: impl FnMut(&Record, &mut Parsed) -> Result<()>,
) -> Result<Parsed> {
    #[cfg(feature = "diagnostic-timing")]
    let _span = crate::timing::span("open.index_directory_parse");
    let mut input = Input {
        reader,
        scratch: Vec::new(),
        limit: ENTRY_LIMIT,
        position: 0,
    };
    let mut parsed = empty();
    let mut cursor = HEADER_SIZE;
    let mut record_count = 0usize;
    let mut previous = None;
    let mut postings = Vec::new();
    let mut previous_term = None;
    let mut previous_posting = None;
    let mut posting_end = None;
    let mut records_seen = false;
    let mut postings_seen = false;
    input.expect(b'{')?;
    loop {
        let field: String = input.value()?;
        input.expect(b':')?;
        match field.as_str() {
            "records" if !records_seen => {
                records_seen = true;
                input.array(|r: Record| {
                    crate::validate_key(&r.key).map_err(|e| corrupt(&e.to_string()))?;
                    if r.offset != cursor
                        || r.deleted != (r.len == 0)
                        || r.len > 4 * 1024 * 1024 + 128
                        || r.version == 0
                        || r.version > sequence
                        || previous.as_ref().is_some_and(|key| key >= &r.key)
                    {
                        return Err(corrupt("invalid record directory"));
                    }
                    cursor = cursor
                        .checked_add(r.len as u64)
                        .ok_or_else(|| corrupt("payload overflow"))?;
                    accept(&r, &mut parsed)?;
                    record_count = record_count
                        .checked_add(1)
                        .ok_or_else(|| corrupt("record count overflow"))?;
                    if !r.deleted {
                        parsed.live += 1;
                    }
                    previous = Some(r.key);
                    Ok(())
                })?;
            }
            "postings" if !postings_seen => {
                postings_seen = true;
                // Exact scalar values can exceed the metadata entry budget.
                // Preserve SEG03 compatibility up to its existing directory limit.
                input.limit = MAX_DIRECTORY as usize;
                input.expect(b'[')?;
                input.whitespace()?;
                if input.peek()? == Some(b']') {
                    input.byte()?;
                } else {
                    loop {
                        input.whitespace()?;
                        let start = input.position;
                        let (term, posting): (Term, Posting) = input.value()?;
                        if previous_term
                            .as_ref()
                            .is_some_and(|previous| previous >= &term)
                        {
                            return Err(corrupt("posting order"));
                        }
                        if previous_posting.is_some_and(|end| end != posting.offset)
                            || posting.count == 0
                        {
                            return Err(corrupt("posting bounds"));
                        }
                        let end = posting
                            .offset
                            .checked_add(posting.count as u64 * 4)
                            .ok_or_else(|| corrupt("posting overflow"))?;
                        posting_end.get_or_insert(posting.offset);
                        previous_posting = Some(end);
                        parsed.entries += posting.count as u64;
                        // Reversed legacy field order keeps the ordinary scan:
                        // its live count is not known yet, so no proof is made.
                        #[cfg(feature = "bounded-engine")]
                        if records_seen {
                            parsed.universal.observe(&term, posting.count, parsed.live);
                        }
                        if let Term::Collection(scope, collection) = &term {
                            parsed.collections.push((scope.clone(), collection.clone()));
                        }
                        if paged_terms {
                            parsed.bloom.add(&term);
                            super::term_pages::push_fence(
                                &mut parsed.pages,
                                &term,
                                data_end + start,
                                data_end + input.position,
                                posting.count,
                            )?;
                        } else {
                            postings.push((term.clone(), posting));
                        }
                        previous_term = Some(term);
                        input.whitespace()?;
                        match input.byte()? {
                            b']' => break,
                            b',' => {}
                            _ => return Err(corrupt("posting separator")),
                        }
                    }
                }
                input.limit = ENTRY_LIMIT;
            }
            _ => return Err(corrupt("unknown or duplicate directory field")),
        }
        input.whitespace()?;
        match input.byte()? {
            b'}' => break,
            b',' => {}
            _ => return Err(corrupt("object separator")),
        }
    }
    input.whitespace()?;
    if input.peek()?.is_some() || !records_seen || !postings_seen {
        return Err(corrupt("missing fields/trailing data"));
    }
    if posting_end.unwrap_or(cursor) != cursor
        || previous_posting.unwrap_or(cursor) != data_end
        || postings
            .iter()
            .any(|(_, p)| p.count as usize > record_count)
        || parsed
            .pages
            .iter()
            .any(|p| p.max_count as usize > record_count)
    {
        return Err(corrupt("noncontiguous data"));
    }
    parsed.records.shrink_to_fit();
    parsed.ids.shrink_to_fit();
    // Match the original bulk-built tree layout rather than sorted insertion.
    // The token dictionary is still resident; paging it is a separate stage.
    parsed.postings = postings.into_iter().collect();
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "bounded-engine")]
    #[test]
    fn universal_gram_uses_live_count_and_reversed_order_falls_back() {
        let directory = Directory {
            records: vec![
                Record {
                    key: Key::new("s", "c", "a"),
                    offset: HEADER_SIZE,
                    len: 5,
                    checksum: 1,
                    version: 1,
                    deleted: false,
                },
                Record {
                    key: Key::new("s", "c", "b"),
                    offset: HEADER_SIZE + 5,
                    len: 0,
                    checksum: 0,
                    version: 1,
                    deleted: true,
                },
            ],
            postings: vec![(
                Term::Gram("s".into(), "the".into()),
                Posting {
                    offset: HEADER_SIZE + 5,
                    count: 1,
                },
            )],
        };
        let bytes = serde_json::to_vec(&directory).unwrap();
        let parsed = parse(bytes.as_slice(), HEADER_SIZE + 9, 1).unwrap();
        assert_eq!(parsed.live, 1);
        let proof =
            super::super::covering::Covering::new(&std::fs::metadata("Cargo.toml").unwrap())
                .with_grams(parsed.universal);
        assert!(proof.short_covered("s", "he"));
        let reversed = format!(
            "{{\"postings\":{},\"records\":{}}}",
            serde_json::to_string(&directory.postings).unwrap(),
            serde_json::to_string(&directory.records).unwrap()
        );
        let parsed = parse(reversed.as_bytes(), HEADER_SIZE + 9, 1).unwrap();
        let proof =
            super::super::covering::Covering::new(&std::fs::metadata("Cargo.toml").unwrap())
                .with_grams(parsed.universal);
        assert!(!proof.short_covered("s", "he"));
    }
    #[test]
    fn framing_handles_escapes_boundaries_and_strict_schema() {
        let record = Record {
            key: Key::new("şirket", "東京", "quote\"back\\🦀"),
            offset: HEADER_SIZE,
            len: 5,
            checksum: 1,
            version: 1,
            deleted: false,
        };
        let directory = Directory {
            records: vec![record],
            postings: vec![(
                Term::Scope("şirket".into()),
                Posting {
                    offset: HEADER_SIZE + 5,
                    count: 1,
                },
            )],
        };
        let bytes = serde_json::to_vec(&directory).unwrap();
        for capacity in [1, 2, 7, 4096] {
            let parsed = parse(
                BufReader::with_capacity(capacity, bytes.as_slice()),
                HEADER_SIZE + 9,
                1,
            )
            .unwrap();
            assert_eq!(parsed.records.len(), 1);
            assert_eq!(parsed.ids, directory.records[0].key.id);
        }
        for cut in 0..bytes.len() {
            assert!(parse(&bytes[..cut], HEADER_SIZE + 9, 1).is_err());
        }
        for bad in [
            "{}",
            "{\"records\":[],\"records\":[]}",
            "{\"records\":[],\"postings\":[],}",
            "{\"records\":[],\"postings\":[]}x",
            "{\"extra\":[],\"records\":[],\"postings\":[]}",
        ] {
            assert!(parse(bad.as_bytes(), HEADER_SIZE, 1).is_err());
        }
        let reversed = br#"{"postings":[],"records":[]}"#;
        assert!(parse(reversed.as_slice(), HEADER_SIZE, 0).is_ok());
    }
    #[test]
    fn entry_budget_and_order_are_rejected_without_unbounded_scratch() {
        let large = format!("\"{}\"", "x".repeat(ENTRY_LIMIT));
        let mut input = Input {
            position: 0,
            reader: large.as_bytes(),
            scratch: Vec::new(),
            limit: ENTRY_LIMIT,
        };
        assert!(input.value::<String>().is_err());
        assert!(input.scratch.capacity() <= ENTRY_LIMIT);
        let record = |id| Record {
            key: Key::new("s", "c", id),
            offset: HEADER_SIZE,
            len: 0,
            checksum: 0,
            version: 1,
            deleted: true,
        };
        let directory = Directory {
            records: vec![record("z"), record("a")],
            postings: Vec::new(),
        };
        assert!(
            parse(
                serde_json::to_vec(&directory).unwrap().as_slice(),
                HEADER_SIZE,
                1
            )
            .is_err()
        );
    }
    #[test]
    fn large_exact_values_remain_compatible_with_existing_seg03() {
        let term = Term::Exact(
            "s".into(),
            "c".into(),
            "field".into(),
            "x".repeat(ENTRY_LIMIT + 1),
        );
        let directory = Directory {
            records: vec![Record {
                key: Key::new("s", "c", "id"),
                offset: HEADER_SIZE,
                len: 5,
                checksum: 1,
                version: 1,
                deleted: false,
            }],
            postings: vec![(
                term.clone(),
                Posting {
                    offset: HEADER_SIZE + 5,
                    count: 1,
                },
            )],
        };
        let bytes = serde_json::to_vec(&directory).unwrap();
        let parsed = parse(
            BufReader::with_capacity(7, bytes.as_slice()),
            HEADER_SIZE + 9,
            1,
        )
        .unwrap();
        assert!(parsed.postings.contains_key(&term));
    }
}
