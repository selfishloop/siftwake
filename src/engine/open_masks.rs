use super::{DeltaKey, MaskCache, Run};
use crate::{Key, Result};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

pub(super) fn build(runs: &[Run], shadow: &BTreeSet<DeltaKey>) -> Result<(MaskCache, usize)> {
    let mut cache = MaskCache::default();
    let mut visible = 0;
    for (run, mut mask) in runs.iter().zip(sweep(runs)?) {
        visible += finish(run, &mut mask, shadow, &mut cache)?;
    }
    Ok((cache, visible))
}

fn finish(
    run: &Run,
    mask: &mut [u64],
    shadow: &BTreeSet<DeltaKey>,
    cache: &mut MaskCache,
) -> Result<usize> {
    cache.remember(run.source.id, mask);
    for key in shadow {
        if let Some(slot) = run.index.find(key)? {
            hide(mask, slot);
        }
    }
    run.index.visible_count(mask)
}

fn hide(mask: &mut [u64], slot: u32) {
    mask[slot as usize / 64] |= 1 << (slot % 64);
}

fn sweep(runs: &[Run]) -> Result<Vec<Vec<u64>>> {
    let mut masks: Vec<_> = runs
        .iter()
        .map(|run| vec![0; run.index.len().div_ceil(64)])
        .collect();
    let mut ranges: Vec<_> = runs
        .iter()
        .enumerate()
        .filter_map(|(n, run)| run.index.bounds().map(|(low, high)| (low, high, n)))
        .collect();
    ranges.sort_unstable();

    // A disjoint range has no revision to hide in another run. Avoid reading
    // its slots, as the pairwise implementation did on clean layouts.
    let mut heap = BinaryHeap::with_capacity(ranges.len());
    let mut ranges = ranges.into_iter().peekable();
    let mut previous_high = None;
    while let Some((low, high, n)) = ranges.next() {
        if previous_high.is_some_and(|end| end >= low)
            || ranges
                .peek()
                .is_some_and(|&(next_low, _, _)| next_low <= high)
        {
            heap.push(Reverse((runs[n].index.key(0)?, n, 0u32)));
        }
        previous_high = Some(previous_high.map_or(high, |end| high.max(end)));
    }

    // Equal keys arrive in manifest order, oldest first. Every successor,
    // including a tombstone, hides its predecessor for that key.
    let mut previous: Option<(Key, usize, u32)> = None;
    while let Some(Reverse((key, n, slot))) = heap.pop() {
        if let Some((prev_key, prev_run, prev_slot)) = previous
            && prev_key == key
        {
            hide(&mut masks[prev_run], prev_slot);
        }
        let next = slot + 1;
        let index = &runs[n].index;
        if (next as usize) < index.len() {
            heap.push(Reverse((index.key(next)?, n, next)));
        }
        previous = Some((key, n, slot));
    }
    Ok(masks)
}

#[cfg(test)]
mod tests;
