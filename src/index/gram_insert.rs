//! Bounded merge of sorted document grams into the existing RAM dictionary.
use super::{BTreeMap, BTreeSet, Posting};
const BATCH: usize = 64;
const VISITS_PER_GRAM: usize = 8;

fn insert(posting: &mut Posting, id: u32, retain_text: bool, bytes: &mut usize) {
    let before = posting.heap_bytes();
    posting.insert(id);
    if !retain_text {
        let after = posting.heap_bytes();
        if after >= before {
            *bytes += after - before;
        } else {
            *bytes -= before - after;
        }
    }
}

fn lookup(
    map: &mut BTreeMap<String, Posting>,
    gram: &String,
    id: u32,
    retain_text: bool,
    bytes: &mut usize,
) {
    if let Some(posting) = map.get_mut(gram) {
        insert(posting, id, retain_text, bytes);
    } else {
        let mut posting = Posting::default();
        posting.insert(id);
        map.insert(gram.clone(), posting);
        if !retain_text {
            *bytes += 96 + gram.len();
        }
    }
}

pub(super) fn try_add(
    map: &mut BTreeMap<String, Posting>,
    title: &BTreeSet<String>,
    body: &BTreeSet<String>,
    id: u32,
    retain_text: bool,
    bytes: &mut usize,
) -> bool {
    let upper = title.len().saturating_add(body.len());
    if upper < BATCH || map.len() > upper.saturating_mul(16) {
        return false;
    }
    let mut grams = title.union(body);
    let mut batch = Vec::with_capacity(BATCH);
    let mut missing = Vec::with_capacity(BATCH);
    loop {
        batch.clear();
        batch.extend(grams.by_ref().take(BATCH));
        if batch.is_empty() {
            break;
        }
        missing.clear();
        let mut at = 0;
        // No chunk visits more than eight dictionary entries per input gram.
        // On exhaustion, the remaining grams use exact lookups after the range
        // is dropped. They must never be assumed absent from the dictionary.
        for (existing, posting) in map
            .range_mut::<str, _>((
                std::ops::Bound::Included(batch[0].as_str()),
                std::ops::Bound::Included(batch[batch.len() - 1].as_str()),
            ))
            .take(batch.len() * VISITS_PER_GRAM)
        {
            while at < batch.len() && batch[at] < existing {
                missing.push(batch[at]);
                at += 1;
            }
            if at < batch.len() && batch[at] == existing {
                insert(posting, id, retain_text, bytes);
                at += 1;
            }
            if at == batch.len() {
                break;
            }
        }
        // Missing keys and unvisited keys both go through exact lookup. This
        // also preserves the original allocation accounting for new terms.
        for gram in missing.iter().copied().chain(batch[at..].iter().copied()) {
            lookup(map, gram, id, retain_text, bytes);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scalar(
        map: &mut BTreeMap<String, Posting>,
        title: &BTreeSet<String>,
        body: &BTreeSet<String>,
        id: u32,
        retain: bool,
        bytes: &mut usize,
    ) {
        for gram in title.union(body) {
            lookup(map, gram, id, retain, bytes);
        }
    }
    fn equal(a: &BTreeMap<String, Posting>, b: &BTreeMap<String, Posting>) {
        assert_eq!(a.len(), b.len());
        for ((ak, av), (bk, bv)) in a.iter().zip(b) {
            assert_eq!(ak, bk);
            assert_eq!(av.iter().collect::<Vec<_>>(), bv.iter().collect::<Vec<_>>());
            assert_eq!(av.heap_bytes(), bv.heap_bytes());
        }
    }
    #[test]
    fn capped_ranges_missing_grams_duplicates_and_reused_ids_match_lookup() {
        for retain in [false, true] {
            let mut a = BTreeMap::new();
            let mut b = BTreeMap::new();
            let (mut ab, mut bb) = (0, 0);
            // A dense middle interval consumes the visit cap before later
            // existing and absent input keys. Overall size still passes guard.
            let body: BTreeSet<_> = (0..800).map(|n| format!("m{n:04}")).collect();
            scalar(&mut a, &BTreeSet::new(), &body, 9, retain, &mut ab);
            scalar(&mut b, &BTreeSet::new(), &body, 9, retain, &mut bb);
            let body: BTreeSet<_> = (0..63)
                .map(|n| format!("a{n:04}"))
                .chain(["m0000", "m0799", "z-new", "東京🦀", "\0xy"].map(str::to_owned))
                .collect();
            let title = ["m0799", "abc", "東京🦀"].map(str::to_owned).into();
            for id in [4, 4, 17, u32::MAX, 0, 9] {
                scalar(&mut a, &title, &body, id, retain, &mut ab);
                assert!(try_add(&mut b, &title, &body, id, retain, &mut bb));
                equal(&a, &b);
                assert_eq!(ab, bb);
            }
            // Sparse dictionaries and small inputs keep the original path.
            let small = ["abc".to_owned()].into();
            assert!(!try_add(&mut b, &small, &small, 1, retain, &mut bb));
            let huge: BTreeMap<_, _> = (0..2000)
                .map(|i| (format!("x{i}"), Posting::default()))
                .collect();
            b = huge;
            assert!(!try_add(&mut b, &title, &body, 1, retain, &mut bb));
        }
    }

    #[test]
    fn posting_representation_transitions_preserve_heap_accounting() {
        let body: BTreeSet<_> = (0..65).map(|n| format!("g{n:03}")).collect();
        // All title grams overlap, including the last gram in the second batch.
        let title = ["g000", "g063", "g064"].map(str::to_owned).into();
        for retain in [false, true] {
            let mut scalar_map = BTreeMap::new();
            let mut batched_map = BTreeMap::new();
            let (mut scalar_bytes, mut batched_bytes) = (0, 0);
            for id in (0..130).chain([64, 0, u32::MAX, u32::MAX]) {
                scalar(
                    &mut scalar_map,
                    &title,
                    &body,
                    id,
                    retain,
                    &mut scalar_bytes,
                );
                assert!(try_add(
                    &mut batched_map,
                    &title,
                    &body,
                    id,
                    retain,
                    &mut batched_bytes,
                ));
                equal(&scalar_map, &batched_map);
                assert_eq!(scalar_bytes, batched_bytes);
                if id == 64 {
                    assert!(matches!(batched_map["g064"], Posting::Dense { .. }));
                }
            }
            assert!(matches!(batched_map["g064"], Posting::Sparse(_)));
            if retain {
                assert_eq!(batched_bytes, 0);
            } else {
                let expected: usize = batched_map
                    .iter()
                    .map(|(gram, posting)| 96 + gram.len() + posting.heap_bytes())
                    .sum();
                assert_eq!(batched_bytes, expected);
            }
        }
    }

    #[test]
    #[ignore = "read-only posting insertion replay over the permitted existing first10k"]
    fn existing_corpus_posting_probe() -> Result<(), Box<dyn std::error::Error>> {
        use std::{
            fs::File,
            io::{BufReader, Read},
            time::Instant,
        };
        let word = |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap());
        let mut source =
            BufReader::new(File::open("data/rkyv-corpus-20261009/json.payload-bench")?);
        let mut header = [0; 20];
        source.read_exact(&mut header)?;
        assert_eq!(&header[..8], b"PLOADB01");
        assert_eq!(crc32fast::hash(&header[..16]), word(&header[16..]));
        let mut maps = [BTreeMap::new(), BTreeMap::new()];
        let mut bytes = [0, 0];
        let mut samples = [Vec::new(), Vec::new()];
        let mut text_bytes = 0;
        let mut merged = 0;
        for first in (0..10000).step_by(50) {
            if first % 500 == 0 {
                maps = [BTreeMap::new(), BTreeMap::new()];
                bytes = [0, 0];
            }
            let mut inputs = Vec::with_capacity(50);
            for id in first..first + 50 {
                let mut frame = [0; 16];
                source.read_exact(&mut frame)?;
                assert_eq!(crc32fast::hash(&frame[..12]), word(&frame[12..]));
                assert_eq!(word(&frame[8..12]) as usize, id);
                let size = word(&frame[..4]) as usize;
                assert!(size <= 4 * 1024 * 1024);
                let mut payload = vec![0; size];
                source.read_exact(&mut payload)?;
                assert_eq!(crc32fast::hash(&payload), word(&frame[4..8]));
                let doc: crate::StoredDocument = serde_json::from_slice(&payload)?;
                text_bytes += doc.document.text.len();
                inputs.push((
                    super::super::grams(&super::super::fold(&doc.document.title)),
                    super::super::packed_grams::extract(&super::super::fold_for_intake(
                        &doc.document.text,
                    )),
                ));
            }
            let order = (first / 50) % 2;
            for mode in [order, 1 - order] {
                let start = Instant::now();
                for (offset, (title, body)) in inputs.iter().enumerate() {
                    let id = ((first + offset) % 500) as u32;
                    if mode == 0
                        || !try_add(&mut maps[mode], title, body, id, false, &mut bytes[mode])
                    {
                        scalar(&mut maps[mode], title, body, id, false, &mut bytes[mode]);
                    } else {
                        merged += 1;
                    }
                }
                samples[mode].push(start.elapsed().as_nanos() as u64);
            }
            equal(&maps[0], &maps[1]);
            assert_eq!(bytes[0], bytes[1]);
        }
        assert_eq!(text_bytes, 264_842_858);
        println!(
            "{}",
            serde_json::json!({"phase":"posting_probe","documents":10000,
            "text_bytes":text_bytes,"chunk":64,"scan_cap_per_gram":8,
            "timed_block_documents":50,"reset_maps_every_documents":500,
            "lookup_ns":samples[0],"merge_ns":samples[1],"merged_documents":merged,
            "maps_and_accounting_equal":true,"read_only_existing_corpus":true,
            "scope":"RAM posting insertion only;CRC/JSON/fold/grams/equality/drop outside timing;not normal DB or OS RAM"})
        );
        Ok(())
    }
}
