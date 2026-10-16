//! Bounded independent oracle; existing corpus only, no DB/corpus writes.
use crate::{input, reads};
use fdb::{Key, StoredDocument};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

const REPEATS: usize = 20;
pub struct Case {
    pub text: String,
    pub offset: usize,
    pub limit: usize,
    pub kind: &'static str,
    pub ids: Vec<usize>,
    pub total: usize,
    pub late_matches: usize,
}
pub struct Matrix {
    pub cases: Vec<Case>,
    pub documents: BTreeMap<usize, StoredDocument>,
    pub gets: Vec<usize>,
}
fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
fn shuffle<T>(items: &mut [T], state: &mut u64) {
    for n in (1..items.len()).rev() {
        let selected = random(state) as usize % (n + 1);
        items.swap(n, selected);
    }
}
fn stats(samples: &[f64]) -> serde_json::Value {
    let mut value = reads::summary(samples);
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    value["p99_ms"] = json!(sorted[(sorted.len() * 99).div_ceil(100) - 1]);
    value
}
impl Matrix {
    pub fn load() -> input::Result<Self> {
        input::phase("matrix_oracle_loading_untimed");
        let specs = [
            ("quixote", "word"),
            ("zeuglodon", "word"),
            ("beagle", "word"),
            ("mesopotamia", "word"),
            ("transubstantiation", "word"),
            ("harpoon", "word"),
            ("ishmael", "word"),
            ("invertebrata", "word"),
            ("darcy", "word"),
            ("elizabeth", "word"),
            ("watson", "word"),
            ("holmes", "word"),
            ("frankenstein", "word"),
            ("galapagos", "word"),
            ("fossil", "word"),
            ("geologist", "word"),
            ("the", "common"),
            ("man", "common"),
            ("he", "short"),
            ("zzxqnever", "absent"),
            ("zx", "short-negative"),
            ("the zx", "negative-and"),
            ("quixote whale", "and"),
            ("holmes elementary", "and"),
            ("fossil geologist", "and"),
            ("whale harpoon", "and"),
            ("QUİXOTE", "fold"),
            ("İSHMAEL", "fold"),
        ];
        let mut texts: Vec<_> = specs
            .iter()
            .map(|(text, kind)| (text.to_string(), *kind))
            .collect();
        texts.push(("zx".repeat(150), "long-negative"));
        let mut preview = input::Input::open()?;
        let first = preview.read()?;
        let normalized = reads::fold(&first.document.text);
        let late = normalized
            .split_whitespace()
            .rev()
            .map(|word| word.trim_matches(|c: char| !c.is_alphabetic()))
            .find(|word| word.len() >= 10)
            .ok_or("no late probe token in first source document")?
            .to_string();
        texts.push((late, "tail-token"));
        let terms: Vec<Vec<String>> = texts
            .iter()
            .map(|(text, _)| text.split_whitespace().map(reads::fold).collect())
            .collect();
        let mut matches = vec![Vec::new(); texts.len()];
        let mut late_counts = vec![0; texts.len()];
        let mut source = input::Input::open()?;
        let mut lengths = Vec::with_capacity(10000);
        for id in 0..10000 {
            let stored = source.read()?;
            assert_eq!(
                stored.document.key,
                Key::new("shared", "texts", &format!("{id:010}"))
            );
            lengths.push(stored.document.text.len());
            let title = reads::fold(&stored.document.title);
            let body = reads::fold(&stored.document.text);
            for (n, terms) in terms.iter().enumerate() {
                if terms
                    .iter()
                    .all(|term| title.contains(term) || body.contains(term))
                {
                    matches[n].push(id);
                    if terms.iter().any(|term| {
                        !title.contains(term)
                            && body.find(term).is_some_and(|at| at >= body.len() * 3 / 4)
                    }) {
                        late_counts[n] += 1;
                    }
                }
            }
        }
        let mut cases: Vec<_> = texts
            .iter()
            .enumerate()
            .map(|(n, (text, kind))| Case {
                text: text.clone(),
                offset: 0,
                limit: 50,
                kind,
                ids: matches[n].iter().take(50).copied().collect(),
                total: matches[n].len(),
                late_matches: late_counts[n],
            })
            .collect();
        let medium = matches
            .iter()
            .position(|ids| (100..=1000).contains(&ids.len()))
            .ok_or("no medium query")?;
        let common = texts.iter().position(|(text, _)| text == "the").unwrap();
        for (n, offset, limit) in [
            (common, 0, 1),
            (common, 0, 200),
            (common, matches[common].len() / 2, 50),
            (common, matches[common].len().saturating_sub(50), 50),
            (medium, matches[medium].len() / 2, 1),
            (medium, matches[medium].len().saturating_sub(200), 200),
            (0, matches[0].len() / 2, 10),
            (0, matches[0].len() + 1, 50),
        ] {
            cases.push(Case {
                text: texts[n].0.clone(),
                offset,
                limit,
                kind: "page",
                ids: matches[n]
                    .iter()
                    .skip(offset)
                    .take(limit)
                    .copied()
                    .collect(),
                total: matches[n].len(),
                late_matches: late_counts[n],
            });
        }
        let mut seed = 0x0046_4442_1234_u64;
        let mut keys = BTreeSet::from([0, 9999]);
        for (bin_min, bin_max) in [
            (0, 4096),
            (4096, 16384),
            (16384, 32768),
            (32768, usize::MAX),
        ] {
            let id = lengths
                .iter()
                .position(|len| *len >= bin_min && *len < bin_max)
                .ok_or("empty size bin")?;
            keys.insert(id);
        }
        while keys.len() < 256 {
            keys.insert(random(&mut seed) as usize % 10000);
        }
        let gets: Vec<_> = keys.iter().copied().collect();
        for case in &cases {
            keys.extend(&case.ids);
        }
        let mut documents = BTreeMap::new();
        let mut source = input::Input::open()?;
        for id in 0..10000 {
            let stored = source.read()?;
            if keys.contains(&id) {
                documents.insert(id, stored);
            }
        }
        println!(
            "{}",
            json!({"phase":"matrix_oracle","text_bytes":source.text_bytes,"records":10000,"retained_documents":documents.len(),"queries":cases.iter().map(|c| json!({"text":c.text,"kind":c.kind,"offset":c.offset,"limit":c.limit,"total":c.total,"late_matches":c.late_matches})).collect::<Vec<_>>(),"get_keys":gets,"source_crc_verified":true,"outside_timing":true})
        );
        Ok(Self {
            cases,
            documents,
            gets,
        })
    }
    pub fn measure<G, S>(&self, seed: u64, mut get: G, mut search: S) -> input::Result<()>
    where
        G: FnMut(&Key) -> input::Result<Option<StoredDocument>>,
        S: FnMut(&str, usize, usize) -> input::Result<(usize, Vec<StoredDocument>)>,
    {
        let pages_only = std::env::args().any(|arg| arg == "--pages-only");
        let repeats = if pages_only { 200 } else { REPEATS };
        self.measure_rounds(seed, repeats, pages_only, &mut get, &mut search)
    }
    pub fn measure_rounds<G, S>(
        &self,
        seed: u64,
        repeats: usize,
        pages_only: bool,
        mut get: G,
        mut search: S,
    ) -> input::Result<()>
    where
        G: FnMut(&Key) -> input::Result<Option<StoredDocument>>,
        S: FnMut(&str, usize, usize) -> input::Result<(usize, Vec<StoredDocument>)>,
    {
        let check = |case: &Case, total, hits: &[StoredDocument]| {
            assert_eq!(total, case.total, "{}", case.text);
            assert_eq!(hits.len(), case.ids.len(), "{}", case.text);
            for (hit, id) in hits.iter().zip(&case.ids) {
                assert_eq!(hit, &self.documents[id]);
            }
        };
        for case in &self.cases {
            let (total, hits) = search(&case.text, case.offset, case.limit)?;
            check(case, total, &hits);
        }
        for id in &self.gets {
            assert_eq!(
                get(&self.documents[id].document.key)?.as_ref(),
                Some(&self.documents[id])
            );
        }
        let missing = [
            Key::new("shared", "texts", "9999999999"),
            Key::new("missing-scope", "texts", "0000000000"),
            Key::new("shared", "missing-collection", "0000000000"),
        ];
        for key in &missing {
            assert!(get(key)?.is_none());
        }
        let mut random_seed = seed + 1;
        let mut query_order: Vec<_> = (0..self.cases.len()).collect();
        let mut key_order = self.gets.clone();
        let mut samples = vec![Vec::new(); self.cases.len()];
        let mut get_samples: [Vec<f64>; 5] = std::array::from_fn(|_| Vec::new());
        for round in 0..repeats {
            shuffle(&mut query_order, &mut random_seed);
            for &n in &query_order {
                let case = &self.cases[n];
                // Pagination is an extra probe: five samples per process,
                // versus twenty for each of the thirty primary queries.
                if case.kind == "page" && !pages_only && round % 4 != 0 {
                    continue;
                }
                let start = Instant::now();
                let (total, hits) = search(&case.text, case.offset, case.limit)?;
                samples[n].push(start.elapsed().as_secs_f64() * 1000.);
                check(case, total, &hits);
            }
            shuffle(&mut key_order, &mut random_seed);
            for id in &key_order {
                let expected = &self.documents[id];
                let bin = match expected.document.text.len() {
                    0..4096 => 0,
                    4096..16384 => 1,
                    16384..32768 => 2,
                    _ => 3,
                };
                let start = Instant::now();
                let actual = get(&expected.document.key)?;
                get_samples[bin].push(start.elapsed().as_secs_f64() * 1000.);
                assert_eq!(actual.as_ref(), Some(expected));
            }
            for key in &missing {
                let start = Instant::now();
                let actual = get(key)?;
                get_samples[4].push(start.elapsed().as_secs_f64() * 1000.);
                assert!(actual.is_none());
            }
        }
        println!(
            "{}",
            json!({"phase":"matrix_reads","seed":seed,"repeats":repeats,"pages_only":pages_only,"queries":self.cases.iter().enumerate().map(|(n,c)|json!({"text":c.text,"kind":c.kind,"offset":c.offset,"limit":c.limit,"total":c.total,"late_matches":c.late_matches,"timing":stats(&samples[n])})).collect::<Vec<_>>(),"gets":get_samples.iter().enumerate().map(|(n,s)|json!({"bin":n,"timing":stats(s)})).collect::<Vec<_>>(),"full_document_checks":true,"cache":"warm OS cache; per-case/key untimed warmup; no eviction","memory_not_measured":true})
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shuffle_is_reproducible_permutation() {
        let mut a: Vec<_> = (0..100).collect();
        let mut b = a.clone();
        super::shuffle(&mut a, &mut 7);
        super::shuffle(&mut b, &mut 7);
        assert_eq!(a, b);
        a.sort();
        assert_eq!(a, (0..100).collect::<Vec<_>>());
    }
}
