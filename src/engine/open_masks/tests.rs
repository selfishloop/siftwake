use super::*;
use crate::StoredDocument;
use crate::engine::{remember_delta_key, write_run_prepared};
use crate::format::Change;
use crate::reclaim::tests::{Temp, doc};

fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn key_id(id: u64) -> String {
    format!("{id:04}")
}

fn run_keys(n: u64, state: &mut u64) -> BTreeSet<u64> {
    match n {
        // Three dense runs repeat the same keys across two 64-slot boundaries.
        1..=3 => (0..130).collect(),
        4 => BTreeSet::new(),
        5 => BTreeSet::from([64]),
        // Include a disjoint range and a range that touches an endpoint.
        6 => (200..270).collect(),
        7 => BTreeSet::from([269, 300]),
        _ => (0..random(state) % 48)
            .map(|_| random(state) % 350)
            .collect(),
    }
}

fn reference_mask(runs: &[Run], n: usize) -> Vec<u64> {
    let index = &runs[n].index;
    let mut mask = vec![0; index.len().div_ceil(64)];
    for newer in &runs[n + 1..] {
        index.hide_keys(&newer.index, &mut mask).unwrap();
    }
    mask
}

fn compare(
    runs: &[Run],
    masks: &[Vec<u64>],
    reference: &[Vec<u64>],
    shadow: &BTreeSet<DeltaKey>,
    seed: u64,
) -> (MaskCache, usize) {
    let mut reference_cache = MaskCache::default();
    let mut sweep_cache = MaskCache::default();
    let mut count = 0;
    for (n, (run, mut mask)) in runs.iter().zip(masks.iter().cloned()).enumerate() {
        let context = (seed, runs.len(), n, shadow.len());
        let mut expected = reference[n].clone();
        assert_eq!(mask, expected, "base bitmap {context:?}");

        reference_cache.remember(run.source.id, &expected);
        for key in shadow {
            if let Some(slot) = run.index.find(key).unwrap() {
                expected[slot as usize / 64] |= 1 << (slot % 64);
            }
        }
        let visible = finish(run, &mut mask, shadow, &mut sweep_cache).unwrap();
        assert_eq!(mask, expected, "WAL shadow bitmap {context:?}");
        assert_eq!(
            visible,
            run.index.visible_count(&expected).unwrap(),
            "visible count {context:?}"
        );
        count += visible;
    }
    assert_eq!(sweep_cache, reference_cache);
    (reference_cache, count)
}

#[test]
fn sorted_sweep_matches_pairwise_visibility_for_one_to_sixty_runs() {
    for seed in [0xe3ab_9c41_74d2_1703, 0x9df5_6ba1_4ee2_1d77] {
        let mut state = seed;
        let temp = Temp::new();
        let runs: Vec<_> = (1..=60)
            .map(|n| {
                let changes: Vec<_> = run_keys(n, &mut state)
                    .into_iter()
                    .map(|id| {
                        let document = doc(&key_id(id));
                        let deleted = (n == 3 && id == 63)
                            || (n == 5 && id == 64)
                            || (n > 7 && random(&mut state).is_multiple_of(5));
                        Ok(if deleted {
                            Change::Delete { key: document.key }
                        } else {
                            Change::Put {
                                stored: StoredDocument {
                                    document,
                                    version: n,
                                },
                            }
                        })
                    })
                    .collect();
                write_run_prepared(&temp.0, n, n, changes.into_iter(), |_| Ok(())).unwrap()
            })
            .collect();

        assert!(runs[0].index.memory_layout(1).paged_root_capacity_bytes > 0);
        assert_eq!(runs[3].index.len(), 0);
        assert_eq!(runs[4].index.len(), 1);
        assert!(runs[2].index.deleted(63));
        assert!(runs[4].index.deleted(0));

        let mut shadow = BTreeSet::new();
        for id in [0, 62, 63, 64, 127, 128, 129, 269, 300, 999] {
            remember_delta_key(&mut shadow, &doc(&key_id(id)).key);
        }
        for _ in 0..20 {
            let id = random(&mut state) % 350;
            remember_delta_key(&mut shadow, &doc(&key_id(id)).key);
        }
        let mut reference: Vec<Vec<u64>> = Vec::new();
        for (n, newer) in runs.iter().enumerate() {
            // Appending the new run adds exactly one hide_keys call to each
            // older mask. Reuse the existing pairwise prefix for the oracle.
            for (older, mask) in runs[..n].iter().zip(&mut reference) {
                older.index.hide_keys(&newer.index, mask).unwrap();
            }
            reference.push(vec![0; newer.index.len().div_ceil(64)]);
            let prefix = &runs[..=n];
            let masks = sweep(prefix).unwrap();
            compare(prefix, &masks, &reference, &BTreeSet::new(), seed);
            compare(prefix, &masks, &reference, &shadow, seed);
        }
        for subset in [&runs[3..4], &runs[5..7], &runs[..]] {
            let reference: Vec<_> = (0..subset.len())
                .map(|n| reference_mask(subset, n))
                .collect();
            let masks = sweep(subset).unwrap();
            let expected = compare(subset, &masks, &reference, &shadow, seed);
            assert_eq!(build(subset, &shadow).unwrap(), expected);
        }
    }
}
