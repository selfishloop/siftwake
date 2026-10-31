//! Query-local bounded pages for posting membership. No persisted state.
use super::{DiskIndex, Posting};
use crate::{Error, Result};
use std::collections::BTreeMap;

const IDS: u32 = 1024;
const BYTES: usize = (IDS as usize + 2) * 4;
// Includes boundary IDs, page-map and bounded progress-hint bookkeeping.
const COST: usize = BYTES + 384;
const LIMIT: usize = 128 * 1024;
struct Page {
    posting: Posting,
    base: u32,
    begin: u32,
    used: u64,
    bytes: Box<[u8; BYTES]>,
}
#[derive(Clone, Copy)]
struct Hint {
    count: u32,
    target: u32,
    slot: u32,
    used: u64,
}
pub(super) struct Pages {
    capacity: usize,
    clock: u64,
    pages: Vec<Page>,
    slots: BTreeMap<(u64, u32), usize>,
    hints: BTreeMap<u64, Hint>,
}
impl Pages {
    pub(super) fn empty() -> Self {
        Self {
            capacity: 0,
            clock: 0,
            pages: Vec::new(),
            slots: BTreeMap::new(),
            hints: BTreeMap::new(),
        }
    }
    pub(super) fn new(groups: &[Vec<Posting>], budget: &mut usize, records: usize) -> Self {
        let needed = groups.iter().any(|g| {
            let n = g.iter().map(|p| p.count as usize).sum::<usize>();
            (n > 4096 || n.saturating_mul(4) > *budget)
                && g.iter().any(|p| p.count as usize != records)
        });
        let capacity = if needed {
            (*budget).min(LIMIT) / COST
        } else {
            0
        };
        *budget -= capacity * COST;
        Self {
            capacity,
            clock: 0,
            pages: Vec::new(),
            slots: BTreeMap::new(),
            hints: BTreeMap::new(),
        }
    }
    pub(super) fn contains_group(
        &mut self,
        index: &DiskIndex,
        group: &[Posting],
        id: u32,
    ) -> Result<bool> {
        if self.capacity == 0 {
            return index.group_contains(group, id);
        }
        for &p in group {
            if p.count as usize == index.live_records {
                return Ok(true);
            }
            if self.contains(p, id, index.len(), |at, bytes| index.read_at(at, bytes))? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn contains(
        &mut self,
        p: Posting,
        target: u32,
        records: usize,
        mut read: impl FnMut(u64, &mut [u8]) -> Result<()>,
    ) -> Result<bool> {
        if self.capacity == 0 {
            return Err(Error::Corrupt("posting cursor without allowance".into()));
        }
        let hint = self.hints.get(&p.offset).copied();
        if hint.is_some_and(|h| h.count != p.count) {
            return Err(Error::Corrupt("posting cursor identity mismatch".into()));
        }
        let slot = if let Some(h) = hint.filter(|h| h.target <= target) {
            let mut slot = h.slot;
            while slot < p.count {
                self.id_at(p, slot, records, &mut read)?;
                let base = slot / IDS * IDS;
                let page = &self.pages[self.slots[&(p.offset, base)]];
                let end = p.count.min(base.saturating_add(IDS));
                let (mut lo, mut hi) = (slot, end);
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    let at = (mid - page.begin) as usize * 4;
                    let id = u32::from_le_bytes(page.bytes[at..at + 4].try_into().unwrap());
                    if id < target {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                slot = lo;
                if slot < end {
                    break;
                }
                #[cfg(feature = "diagnostic-timing")]
                crate::timing::count("query.posting_cursor_page_advances", 1);
            }
            slot
        } else {
            // New/evicted hint, or a new seed list whose IDs moved backwards.
            // Keep a bounded binary fallback; no scan from zero on every reset.
            let (mut lo, mut hi) = (0, p.count);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if self.id_at(p, mid, records, &mut read)? < target {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            lo
        };
        let found = slot < p.count && self.id_at(p, slot, records, &mut read)? == target;
        if !self.hints.contains_key(&p.offset) && self.hints.len() == self.capacity {
            let old = *self.hints.iter().min_by_key(|(_, h)| h.used).unwrap().0;
            self.hints.remove(&old);
        }
        self.hints.insert(
            p.offset,
            Hint {
                count: p.count,
                target,
                slot,
                used: self.clock,
            },
        );
        Ok(found)
    }
    fn id_at(
        &mut self,
        p: Posting,
        slot: u32,
        records: usize,
        read: impl FnOnce(u64, &mut [u8]) -> Result<()>,
    ) -> Result<u32> {
        if slot >= p.count || self.capacity == 0 {
            return Err(Error::Corrupt("posting page slot bounds".into()));
        }
        self.clock = self.clock.saturating_add(1);
        let base = slot / IDS * IDS;
        let key = (p.offset, base);
        let n = if let Some(&n) = self.slots.get(&key) {
            #[cfg(feature = "diagnostic-timing")]
            crate::timing::count("query.posting_page_hits", 1);
            if self.pages[n].posting != p {
                return Err(Error::Corrupt("posting page identity mismatch".into()));
            }
            n
        } else {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("query.posting_page_read");
            let mut page = if self.pages.len() == self.capacity {
                let n = self
                    .pages
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, p)| p.used)
                    .unwrap()
                    .0;
                let old = self.pages.swap_remove(n);
                self.slots.remove(&(old.posting.offset, old.base));
                if n < self.pages.len() {
                    let moved = &self.pages[n];
                    self.slots.insert((moved.posting.offset, moved.base), n);
                }
                old
            } else {
                Page {
                    posting: p,
                    base,
                    begin: 0,
                    used: 0,
                    bytes: Box::new([0; BYTES]),
                }
            };
            // Read both adjacent IDs too: a page boundary cannot conceal a
            // duplicate/inversion. Full source/index checks still run upstream.
            let begin = base.saturating_sub(1);
            let end = p.count.min(base.saturating_add(IDS + 1));
            let bytes = (end - begin) as usize * 4;
            let at = p
                .offset
                .checked_add(u64::from(begin) * 4)
                .ok_or_else(|| Error::Corrupt("posting page offset overflow".into()))?;
            read(at, &mut page.bytes[..bytes])?;
            let mut previous = None;
            for word in page.bytes[..bytes].as_chunks::<4>().0 {
                let id = u32::from_le_bytes(*word);
                if id as usize >= records || previous.is_some_and(|old| old >= id) {
                    return Err(Error::Corrupt("invalid sorted posting page".into()));
                }
                previous = Some(id);
            }
            page.posting = p;
            page.base = base;
            page.begin = begin;
            let n = self.pages.len();
            self.pages.push(page);
            self.slots.insert(key, n);
            n
        };
        let page = &mut self.pages[n];
        page.used = self.clock;
        let at = (slot - page.begin) as usize * 4;
        Ok(u32::from_le_bytes(
            page.bytes[at..at + 4].try_into().unwrap(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Posting, Vec<u8>) {
        let p = Posting {
            offset: 64,
            count: 10_000,
        };
        let mut bytes = vec![0; 64];
        for id in 0..p.count {
            bytes.extend_from_slice(&(id * 3).to_le_bytes());
        }
        (p, bytes)
    }
    fn get(cache: &mut Pages, p: Posting, bytes: &[u8], slot: u32) -> Result<u32> {
        cache.id_at(p, slot, 30_000, |at, out| {
            let data = bytes
                .get(at as usize..at as usize + out.len())
                .ok_or_else(|| Error::Corrupt("cut posting".into()))?;
            out.copy_from_slice(data);
            Ok(())
        })
    }
    #[test]
    fn bounded_eviction_and_reverse_page_visits_preserve_ids() {
        let (p, bytes) = fixture();
        let mut budget = COST * 2;
        let mut cache = Pages::new(&[vec![p]], &mut budget, 30_000);
        assert_eq!(budget, 0);
        for slot in [0, 1023, 1024, 9999, 5000, 1, 6000, 2048, 2047] {
            assert_eq!(get(&mut cache, p, &bytes, slot).unwrap(), slot * 3);
            assert!(cache.pages.len() <= 2 && cache.slots.len() == cache.pages.len());
        }
    }
    #[test]
    fn advancing_membership_and_backwards_reset_match_binary_oracle() {
        let (p, bytes) = fixture();
        let mut budget = COST * 2;
        let mut cache = Pages::new(&[vec![p]], &mut budget, 30_000);
        let targets = (0..30_003).step_by(13).chain((0..30_000).rev().step_by(97));
        for target in targets {
            let found = cache
                .contains(p, target, 30_000, |at, out| {
                    out.copy_from_slice(&bytes[at as usize..at as usize + out.len()]);
                    Ok(())
                })
                .unwrap();
            assert_eq!(found, target < 30_000 && target % 3 == 0, "target {target}");
            assert!(cache.pages.len() <= 2 && cache.hints.len() <= 2);
        }
    }
    #[test]
    fn cut_order_boundary_and_out_of_range_fail_without_caching_bad_page() {
        let (p, original) = fixture();
        for (slot, value) in [(100, 0), (1024, 3069), (9999, 30_000)] {
            let mut bytes = original.clone();
            let at = 64 + slot * 4;
            bytes[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
            let mut budget = COST;
            let mut cache = Pages::new(&[vec![p]], &mut budget, 30_000);
            let visit = if slot == 1024 { 1023 } else { slot as u32 };
            assert!(get(&mut cache, p, &bytes, visit).is_err());
            assert!(cache.pages.is_empty());
        }
        let mut budget = COST;
        let mut cache = Pages::new(&[vec![p]], &mut budget, 30_000);
        assert!(get(&mut cache, p, &original[..original.len() - 1], 9999).is_err());
        assert!(get(&mut cache, p, &original, p.count).is_err());
        assert_eq!(get(&mut cache, p, &original, 9999).unwrap(), 29997);
    }
}
