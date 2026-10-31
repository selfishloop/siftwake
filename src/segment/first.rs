//! Resumable sorted posting intersection and bounded candidate verification.
use super::*;
use crate::SearchExecution;
use std::cmp::Reverse;

struct Seed {
    posting: Posting,
    slot: u32,
    previous: Option<u32>,
    bytes: Vec<u8>,
    at: usize,
}
impl Seed {
    fn next(&mut self, index: &DiskIndex) -> Result<Option<u32>> {
        if self.at == self.bytes.len() {
            if self.slot == self.posting.count {
                return Ok(None);
            }
            let n = (self.posting.count - self.slot).min(1024) as usize;
            self.bytes.resize(n * 4, 0);
            index.read_at(
                self.posting.offset + u64::from(self.slot) * 4,
                &mut self.bytes,
            )?;
            self.slot += n as u32;
            self.at = 0;
        }
        let id = u32::from_le_bytes(self.bytes[self.at..self.at + 4].try_into().unwrap());
        self.at += 4;
        if id as usize >= index.len() || self.previous.is_some_and(|old| old >= id) {
            return Err(Error::Corrupt("invalid sorted first-page posting".into()));
        }
        self.previous = Some(id);
        Ok(Some(id))
    }
}
pub(crate) struct Candidates<'a> {
    cancel: Option<&'a crate::CancelToken>,
    index: &'a DiskIndex,
    mask: &'a [u64],
    seeds: Vec<Seed>,
    heap: BinaryHeap<Reverse<(u32, usize)>>,
    groups: Vec<Vec<Posting>>,
    cached: Vec<Option<Vec<u32>>>,
    posting_pages: posting_pages::Pages,
    previous: Option<u32>,
    pub(crate) keys: KeyCursor,
}
impl<'a> Candidates<'a> {
    pub(crate) fn new_cancellable(
        index: &'a DiskIndex,
        q: &Query,
        mask: &'a [u64],
        budget: &mut usize,
        cancel: Option<&'a crate::CancelToken>,
    ) -> Result<Self> {
        #[cfg(feature = "diagnostic-timing")]
        let _profile = crate::timing::span("first.posting_setup");
        crate::cancel::check(cancel)?;
        let terms = DiskIndex::query_terms(q);
        let mut groups = vec![match &q.collection {
            Some(c) => index.one(Term::Collection(q.scope.clone(), c.clone()))?,
            None => index.one(Term::Scope(q.scope.clone()))?,
        }];
        let collections = index.collections(q);
        for (field, value) in &q.exact {
            crate::cancel::check(cancel)?;
            let mut group = Vec::new();
            for c in &collections {
                group.extend(index.one(Term::Exact(
                    q.scope.clone(),
                    c.to_string(),
                    field.clone(),
                    value.to_string(),
                ))?);
            }
            groups.push(group);
        }
        for r in &q.ranges {
            let mut group = Vec::new();
            for c in &collections {
                group.extend(index.posting_range(
                    Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.min),
                    Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.max),
                )?);
            }
            groups.push(group);
        }
        if let Some(key) = &q.linked_to {
            groups.push(index.one(Term::Link(key.clone()))?);
        }
        for term in &terms {
            crate::cancel::check(cancel)?;
            for gram in grams(term) {
                groups.push(index.one(Term::Gram(q.scope.clone(), gram))?);
            }
        }
        if index.required_gram_absent(q, &terms) || groups.iter().any(Vec::is_empty) {
            groups = vec![vec![]];
        }
        groups.sort_by_key(|g| g.iter().map(|p| u64::from(p.count)).sum::<u64>());
        let smallest = groups.remove(0);
        if smallest.len().saturating_mul(4096) > *budget {
            return Err(Error::Invalid(
                "first-page posting workspace allowance".into(),
            ));
        }
        *budget -= smallest.len() * 4096;
        let mut seeds = Vec::new();
        let mut heap = BinaryHeap::new();
        for p in smallest {
            let mut seed = Seed {
                posting: p,
                slot: 0,
                previous: None,
                bytes: Vec::new(),
                at: 0,
            };
            if let Some(id) = seed.next(index)? {
                heap.push(Reverse((id, seeds.len())));
            }
            seeds.push(seed);
        }
        let mut cached = Vec::new();
        for group in &groups {
            let n = group.iter().map(|p| p.count as usize).sum::<usize>();
            if n <= 4096 && n * 4 <= *budget && n < index.len() {
                cached.push(Some(index.read_small_group(group)?));
                *budget -= n * 4;
            } else {
                cached.push(None);
            }
        }
        Ok(Self {
            cancel,
            index,
            mask,
            seeds,
            heap,
            groups,
            cached,
            posting_pages: posting_pages::Pages::empty(),
            previous: None,
            keys: KeyCursor::default(),
        })
    }
    // Optional pages never take admission space from a later mandatory seed.
    pub(crate) fn prepare_pages(&mut self, budget: &mut usize) {
        self.posting_pages =
            posting_pages::Pages::new(&self.groups, budget, self.index.live_records);
    }
    pub(crate) fn next(&mut self) -> Result<Option<u32>> {
        #[cfg(feature = "diagnostic-timing")]
        let _profile = crate::timing::span("first.posting_intersection");
        crate::cancel::check(self.cancel)?;
        let mut scanned = 0usize;
        while let Some(Reverse((id, n))) = self.heap.pop() {
            scanned += 1;
            if scanned.is_multiple_of(64) {
                crate::cancel::check(self.cancel)?;
            }
            if let Some(next) = self.seeds[n].next(self.index)? {
                self.heap.push(Reverse((next, n)));
            }
            if self.previous == Some(id) {
                continue;
            }
            self.previous = Some(id);
            #[cfg(feature = "diagnostic-timing")]
            crate::timing::count("first.seed_ids", 1);
            if self.index.deleted(id) || self.mask[id as usize / 64] & (1u64 << (id % 64)) != 0 {
                continue;
            }
            let mut included = true;
            for (g, cache) in self.groups.iter().zip(&self.cached) {
                if !if let Some(ids) = cache {
                    ids.binary_search(&id).is_ok()
                } else {
                    self.posting_pages.contains_group(self.index, g, id)?
                } {
                    included = false;
                    break;
                }
            }
            if included {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }
    pub(crate) fn key(&mut self, id: u32) -> Result<Key> {
        self.index.snapshot_key(id, &mut self.keys)
    }
}

pub(crate) struct Verifier<'a> {
    cancel: Option<&'a crate::CancelToken>,
    index: &'a DiskIndex,
    terms: Vec<String>,
    scratch: QueryTextScratch,
    folded: BTreeMap<String, String>,
    folded_bytes: usize,
    execution: SearchExecution,
}
impl<'a> Verifier<'a> {
    pub(crate) fn new_cancellable(
        index: &'a DiskIndex,
        q: &Query,
        execution: SearchExecution,
        cancel: Option<&'a crate::CancelToken>,
    ) -> Result<Self> {
        let terms = DiskIndex::query_terms(q);
        let terms = index.verification_terms(q, &terms)?.into_owned();
        let scratch = QueryTextScratch {
            reuse_crc: execution.reuse_validated_source_crc
                && index.source_references
                && index.covering.permits_crc_reuse(&index.file)?,
            ..QueryTextScratch::default()
        };
        Ok(Self {
            cancel,
            index,
            terms,
            scratch,
            folded: BTreeMap::new(),
            folded_bytes: 0,
            execution,
        })
    }
    pub(crate) fn one(&mut self, id: u32) -> Result<bool> {
        crate::cancel::check(self.cancel)?;
        if let Some(t) = self.cancel {
            t.candidate();
        }
        if self.terms.is_empty() {
            return Ok(true);
        }
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("first.candidate_verify");
        #[cfg(feature = "experimental-rkyv-payload")]
        if let Some(result) = self.index.benchmark_matches(
            id,
            &mut self.scratch,
            &self.terms,
            &mut self.folded,
            &mut self.folded_bytes,
        )? {
            return Ok(result);
        }
        if self.index.binary_payload {
            self.index.binary_matches_with_capture(
                id,
                &mut self.scratch,
                &self.terms,
                &mut self.folded,
                &mut self.folded_bytes,
                &mut |_, _, _| {},
            )
        } else {
            let doc = self.index.read_document(id)?;
            Ok(self.scratch.matches(
                &doc.document,
                &self.terms,
                &mut self.folded,
                &mut self.folded_bytes,
            ))
        }
    }
    pub(crate) fn batch(&mut self, ids: &[u32]) -> Result<Vec<bool>> {
        let mut matched = Vec::with_capacity(ids.len());
        if self.execution.coalesce_gap_bytes == 0
            || !self.index.binary_payload
            || self.terms.is_empty()
        {
            for &id in ids {
                matched.push(self.one(id)?);
            }
            return Ok(matched);
        }
        let mut at = 0;
        while at < ids.len() {
            let first = self.index.query_metadata(ids[at], &mut self.scratch)?;
            let base = first.offset;
            let mut end = base
                .checked_add(u64::from(first.len))
                .ok_or_else(|| Error::Corrupt("coalesced payload bounds".into()))?;
            let mut stop = at + 1;
            while stop < ids.len() {
                let r = self.index.query_metadata(ids[stop], &mut self.scratch)?;
                let next_end = r
                    .offset
                    .checked_add(u64::from(r.len))
                    .ok_or_else(|| Error::Corrupt("coalesced payload bounds".into()))?;
                if r.offset < end
                    || r.offset - end > self.execution.coalesce_gap_bytes as u64
                    || next_end - base > 256 * 1024
                {
                    break;
                }
                end = next_end;
                stop += 1;
            }
            if stop == at + 1 {
                self.scratch.read_window_enabled = false;
                matched.push(self.one(ids[at])?);
            } else {
                let len = usize::try_from(end - base)
                    .map_err(|_| Error::Corrupt("coalesced payload size".into()))?;
                self.scratch.payload.resize(len, 0);
                self.index
                    .read_payload_at(base, &mut self.scratch.payload)?;
                self.scratch.read_window_enabled = true;
                self.scratch.read_base = base;
                self.scratch.read_len = len;
                self.scratch.window_prefetched = false;
                self.scratch.window_used = 0;
                #[cfg(feature = "diagnostic-timing")]
                {
                    self.scratch.reads.requests += 1;
                    self.scratch.reads.read_bytes += len as u64;
                }
                for &id in &ids[at..stop] {
                    matched.push(self.one(id)?);
                }
            }
            at = stop;
        }
        self.scratch.read_window_enabled = false;
        Ok(matched)
    }
}
