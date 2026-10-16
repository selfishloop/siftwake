//! Sparse fences over the existing JSON directory; no new on-disk format.
use super::*;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

const PAGE_BYTES: usize = 16 * 1024;
const PAGE_TERMS: usize = 128;
const CACHE_BYTES: usize = 1024 * 1024;
// Hot descriptors share the existing allowance; no document/text cache is added.
const LOOKUP_BYTES: usize = 64 * 1024;
const PAGE_CACHE_BYTES: usize = CACHE_BYTES - LOOKUP_BYTES;
static OWNER: AtomicU64 = AtomicU64::new(1);
pub(super) struct Bloom {
    words: Vec<u64>,
}
impl Default for Bloom {
    fn default() -> Self {
        Self {
            words: vec![0; 8192],
        }
    }
}
impl Bloom {
    fn bits(term: &Term) -> [usize; 3] {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        term.hash(&mut h);
        let h = h.finish();
        [
            h as usize & 524287,
            h.rotate_left(21) as usize & 524287,
            h.rotate_left(42) as usize & 524287,
        ]
    }
    pub fn add(&mut self, term: &Term) {
        for bit in Self::bits(term) {
            self.words[bit / 64] |= 1u64 << (bit % 64);
        }
    }
    fn contains(&self, term: &Term) -> bool {
        Self::bits(term)
            .iter()
            .all(|&bit| self.words[bit / 64] & (1u64 << (bit % 64)) != 0)
    }
}

pub(super) struct Fence {
    pub first: Term,
    pub last: Term,
    pub offset: u64,
    pub len: usize,
    pub count: usize,
    pub max_count: u32,
    checksum: u32,
}
pub(super) fn push_fence(
    pages: &mut Vec<Fence>,
    term: &Term,
    start: u64,
    end: u64,
    count: u32,
) -> Result<()> {
    let len =
        usize::try_from(end - start).map_err(|_| Error::Corrupt("term page length".into()))?;
    if pages
        .last()
        .is_none_or(|p| p.count == PAGE_TERMS || end - p.offset > PAGE_BYTES as u64)
    {
        pages.push(Fence {
            first: term.clone(),
            last: term.clone(),
            offset: start,
            len,
            count: 1,
            max_count: count,
            checksum: 0,
        });
    } else {
        let p = pages.last_mut().unwrap();
        p.len = (end - p.offset) as usize;
        p.count += 1;
        p.max_count = p.max_count.max(count);
        p.last = term.clone();
    }
    Ok(())
}
struct Entry {
    owner: u64,
    page: usize,
    values: Vec<(Term, Posting)>,
    bytes: usize,
}
#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
    bytes: usize,
    lookups: BTreeMap<u64, BTreeMap<Term, Lookup>>,
    lookup_bytes: usize,
    clock: u64,
}
struct Lookup {
    posting: Option<Posting>,
    stamp: u64,
    bytes: usize,
}
impl Cache {
    fn lookup(&mut self, owner: u64, term: &Term) -> Option<Option<Posting>> {
        let entry = self.lookups.get_mut(&owner)?.get_mut(term)?;
        self.clock = self.clock.saturating_add(1);
        entry.stamp = self.clock;
        Some(entry.posting)
    }
    fn forget(&mut self, owner: u64) {
        if let Some(entries) = self.lookups.remove(&owner) {
            self.lookup_bytes -= 128 + entries.values().map(|e| e.bytes).sum::<usize>();
        }
    }
    fn remember(&mut self, owner: u64, term: &Term, posting: Option<Posting>) {
        // Account owned strings, descriptor and conservative BTree node overhead.
        let bytes = 96 + std::mem::size_of::<(Term, Lookup)>() + strings(term);
        if bytes + 128 > LOOKUP_BYTES {
            return;
        }
        if self.lookup(owner, term).is_some() {
            return;
        }
        while self.lookup_bytes
            + bytes
            + if self.lookups.contains_key(&owner) {
                0
            } else {
                128
            }
            > LOOKUP_BYTES
        {
            let oldest = self
                .lookups
                .iter()
                .flat_map(|(owner, entries)| {
                    entries.iter().map(move |(term, e)| (*owner, term, e.stamp))
                })
                .min_by_key(|(_, _, stamp)| *stamp)
                .map(|(owner, term, _)| (owner, term.clone()))
                .expect("nonempty lookup accounting");
            let entries = self.lookups.get_mut(&oldest.0).unwrap();
            self.lookup_bytes -= entries.remove(&oldest.1).unwrap().bytes;
            if entries.is_empty() {
                self.lookups.remove(&oldest.0);
                self.lookup_bytes -= 128;
            }
        }
        if !self.lookups.contains_key(&owner) {
            self.lookup_bytes += 128;
        }
        self.clock = self.clock.saturating_add(1);
        self.lookups.entry(owner).or_default().insert(
            term.clone(),
            Lookup {
                posting,
                stamp: self.clock,
                bytes,
            },
        );
        self.lookup_bytes += bytes;
    }
}
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}
fn strings(term: &Term) -> usize {
    match term {
        Term::Scope(s) => s.capacity(),
        Term::Collection(s, c) | Term::Gram(s, c) => s.capacity() + c.capacity(),
        Term::Exact(s, c, f, v) => s.capacity() + c.capacity() + f.capacity() + v.capacity(),
        Term::Numeric(s, c, f, _) => s.capacity() + c.capacity() + f.capacity(),
        Term::Link(k) => k.scope.capacity() + k.collection.capacity() + k.id.capacity(),
    }
}
pub(super) struct Pages {
    file: File,
    fences: Vec<Fence>,
    pub collections: Vec<(String, String)>,
    pub count: usize,
    owner: u64,
    bloom: Bloom,
}
impl Drop for Pages {
    fn drop(&mut self) {
        if let Ok(mut cache) = cache().lock() {
            cache.entries.retain(|e| e.owner != self.owner);
            cache.bytes = cache.entries.iter().map(|e| e.bytes).sum();
            cache.forget(self.owner);
        }
    }
}
impl Pages {
    pub fn new(
        file: &File,
        mut fences: Vec<Fence>,
        collections: Vec<(String, String)>,
        bloom: Bloom,
    ) -> Result<Self> {
        let mut buffer = [0; PAGE_BYTES];
        for fence in &mut fences {
            let mut consumed = 0;
            let mut state = !0;
            while consumed < fence.len {
                let n = (fence.len - consumed).min(buffer.len());
                read_at(file, fence.offset + consumed as u64, &mut buffer[..n])?;
                state = crate::format::crc_extend(state, &buffer[..n]);
                consumed += n;
            }
            fence.checksum = !state;
        }
        let count = fences.iter().map(|p| p.count).sum();
        fences.shrink_to_fit();
        Ok(Self {
            file: file.try_clone()?,
            fences,
            collections,
            count,
            owner: OWNER.fetch_add(1, Ordering::Relaxed),
            bloom,
        })
    }
    pub fn root_bytes(&self) -> usize {
        self.fences.capacity() * std::mem::size_of::<Fence>()
            + self
                .fences
                .iter()
                .map(|f| strings(&f.first) + strings(&f.last))
                .sum::<usize>()
            + self.bloom.words.capacity() * 8
            + self.collections.capacity() * std::mem::size_of::<(String, String)>()
            + self
                .collections
                .iter()
                .map(|(s, c)| s.capacity() + c.capacity())
                .sum::<usize>()
    }
    fn decode(&self, page: usize) -> Result<Vec<(Term, Posting)>> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("query.term_page_decode");
        let p = &self.fences[page];
        let mut raw = vec![0; p.len + 2];
        raw[0] = b'[';
        read_at(&self.file, p.offset, &mut raw[1..p.len + 1])?;
        if crc(&raw[1..p.len + 1]) != p.checksum {
            return Err(Error::Corrupt("term directory page checksum".into()));
        }
        raw[p.len + 1] = b']';
        let values: Vec<(Term, Posting)> =
            serde_json::from_slice(&raw).map_err(|e| Error::Corrupt(e.to_string()))?;
        if values.len() != p.count
            || values.first().is_none_or(|v| v.0 != p.first)
            || values.last().is_none_or(|v| v.0 != p.last)
            || values.windows(2).any(|v| v[0].0 >= v[1].0)
        {
            return Err(Error::Corrupt("term directory page identity/order".into()));
        }
        Ok(values)
    }
    fn with_page<T>(&self, page: usize, f: impl FnOnce(&[(Term, Posting)]) -> T) -> Result<T> {
        let mut cache = cache()
            .lock()
            .map_err(|_| Error::Corrupt("term cache poisoned".into()))?;
        // The common working pool earmarks cache/scratch allowance at creation.
        // Never wait for workspace admission while holding this cache mutex.
        if let Some(pos) = cache
            .entries
            .iter()
            .position(|e| e.owner == self.owner && e.page == page)
        {
            let entry = cache.entries.remove(pos).unwrap();
            let result = f(&entry.values);
            cache.entries.push_front(entry);
            return Ok(result);
        }
        // A page's decoded Term enum/String overhead is larger than its JSON.
        // Release old pages before decoding. Oversized legacy scalars are uncached.
        let estimate = self.fences[page].len.saturating_mul(2)
            + self.fences[page].count.next_power_of_two() * std::mem::size_of::<(Term, Posting)>();
        while cache.bytes.saturating_add(estimate) > PAGE_CACHE_BYTES && !cache.entries.is_empty() {
            cache.bytes -= cache.entries.pop_back().unwrap().bytes;
        }
        let values = self.decode(page)?;
        let bytes = values.capacity() * std::mem::size_of::<(Term, Posting)>()
            + values.iter().map(|(t, _)| strings(t)).sum::<usize>();
        let result = f(&values);
        if bytes <= PAGE_CACHE_BYTES {
            while cache.bytes + bytes > PAGE_CACHE_BYTES {
                cache.bytes -= cache.entries.pop_back().unwrap().bytes;
            }
            cache.bytes += bytes;
            cache.entries.push_front(Entry {
                owner: self.owner,
                page,
                values,
                bytes,
            });
        }
        Ok(result)
    }
    // A definite miss can reject an AND query before any directory page is read.
    pub fn may_contain(&self, term: &Term) -> bool {
        if !self.bloom.contains(term) {
            return false;
        }
        let end = self.fences.partition_point(|p| &p.first <= term);
        end != 0 && term <= &self.fences[end - 1].last
    }
    pub fn get(&self, term: &Term) -> Result<Option<Posting>> {
        if !self.bloom.contains(term) {
            return Ok(None);
        }
        if let Some(posting) = cache()
            .lock()
            .map_err(|_| Error::Corrupt("term cache poisoned".into()))?
            .lookup(self.owner, term)
        {
            return Ok(posting);
        }
        let end = self.fences.partition_point(|p| &p.first <= term);
        if end == 0 || term > &self.fences[end - 1].last {
            return Ok(None);
        }
        let posting = self.with_page(end - 1, |values| {
            values
                .binary_search_by(|v| v.0.cmp(term))
                .ok()
                .map(|i| values[i].1)
        })?;
        cache()
            .lock()
            .map_err(|_| Error::Corrupt("term cache poisoned".into()))?
            .remember(self.owner, term, posting);
        Ok(posting)
    }
    pub fn range(&self, min: &Term, max: &Term) -> Result<Vec<Posting>> {
        let start = self
            .fences
            .partition_point(|p| &p.first <= min)
            .saturating_sub(1);
        let mut result = Vec::new();
        for page in start..self.fences.len() {
            if &self.fences[page].first > max {
                break;
            }
            self.with_page(page, |values| {
                result.extend(
                    values
                        .iter()
                        .filter(|(t, _)| t >= min && t <= max)
                        .map(|(_, p)| *p),
                );
            })?;
        }
        Ok(result)
    }
    pub fn cursor(&self) -> Cursor<'_> {
        Cursor {
            pages: self,
            page: 0,
            values: Vec::new().into_iter(),
        }
    }
}
pub(super) struct Cursor<'a> {
    pages: &'a Pages,
    page: usize,
    values: std::vec::IntoIter<(Term, Posting)>,
}
impl Cursor<'_> {
    pub fn next(&mut self) -> Result<Option<(Term, Posting)>> {
        if let Some(value) = self.values.next() {
            return Ok(Some(value));
        }
        if self.page == self.pages.fences.len() {
            return Ok(None);
        }
        self.values = Vec::new().into_iter();
        self.values = self.pages.decode(self.page)?.into_iter();
        self.page += 1;
        Ok(self.values.next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hot_descriptors_are_bounded_and_bound_to_each_index_owner() {
        let mut cache = Cache::default();
        let term = Term::Gram("şirket".into(), "東京🦀".into());
        let a = Posting {
            offset: 4,
            count: 2,
        };
        let b = Posting {
            offset: 8,
            count: 1,
        };
        cache.remember(1, &term, Some(a));
        cache.remember(2, &term, Some(b));
        assert_eq!(cache.lookup(1, &term), Some(Some(a)));
        assert_eq!(cache.lookup(2, &term), Some(Some(b)));
        cache.forget(1);
        assert_eq!(cache.lookup(1, &term), None);
        assert_eq!(cache.lookup(2, &term), Some(Some(b)));
        for i in 0..2000 {
            let term = Term::Gram("şirket".into(), format!("東京🦀{i:08}"));
            cache.remember(3, &term, Some(a));
            assert!(cache.lookup_bytes <= LOOKUP_BYTES);
        }
        assert!(cache.lookups[&3].len() < 2000);
        cache.forget(2);
        cache.forget(3);
        assert_eq!(cache.lookup_bytes, 0);
        assert!(cache.lookups.is_empty());
    }
    #[test]
    fn sparse_lookup_ranges_cursor_cache_and_uncached_corruption() {
        let path = std::env::temp_dir().join(format!(
            "fdb-term-pages-{}-{}.tmp",
            std::process::id(),
            OWNER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let values: Vec<_> = (0..600)
            .map(|i| {
                (
                    Term::Exact(
                        "şirket".into(),
                        "東京".into(),
                        "quote\"back\\".into(),
                        format!("🦀{i:08}"),
                    ),
                    Posting {
                        offset: i * 4,
                        count: 1,
                    },
                )
            })
            .collect();
        let mut bytes = vec![b'['];
        let mut fences = Vec::new();
        for (i, (term, posting)) in values.iter().enumerate() {
            if i != 0 {
                bytes.push(b',');
            }
            let start = bytes.len();
            serde_json::to_writer(&mut bytes, &(term, posting)).unwrap();
            push_fence(
                &mut fences,
                term,
                start as u64,
                bytes.len() as u64,
                posting.count,
            )
            .unwrap();
        }
        bytes.push(b']');
        file.write_all(&bytes).unwrap();
        let mut bloom = Bloom::default();
        for (t, _) in &values {
            bloom.add(t);
        }
        let pages = Pages::new(&file, fences, Vec::new(), bloom).unwrap();
        assert!(pages.root_bytes() < 96 * 1024);
        for (term, posting) in &values {
            assert!(pages.may_contain(term));
            assert_eq!(pages.get(term).unwrap(), Some(*posting));
        }
        let missing = Term::Gram("şirket".into(), "eve".into());
        assert!(!pages.may_contain(&missing));
        assert_eq!(
            pages.range(&values[126].0, &values[257].0).unwrap(),
            values[126..=257]
                .iter()
                .map(|(_, p)| *p)
                .collect::<Vec<_>>()
        );
        let mut cursor = pages.cursor();
        let mut actual = Vec::new();
        while let Some(v) = cursor.next().unwrap() {
            actual.push(v);
        }
        assert_eq!(actual, values);
        {
            let cache = cache().lock().unwrap();
            assert!(cache.bytes <= PAGE_CACHE_BYTES);
            assert!(cache.lookup_bytes <= LOOKUP_BYTES);
            assert!(cache.bytes + cache.lookup_bytes <= CACHE_BYTES);
        }
        drop(cursor);
        drop(pages);
        for cut in [0, bytes.len() / 2, bytes.len() - 2] {
            file.set_len(cut as u64).unwrap();
            let fence = Fence {
                first: values[599].0.clone(),
                last: values[599].0.clone(),
                offset: (bytes.len() - 10) as u64,
                len: 9,
                count: 1,
                max_count: 1,
                checksum: 0,
            };
            assert!(Pages::new(&file, vec![fence], Vec::new(), Bloom::default()).is_err());
        }
        file.set_len(0).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&bytes).unwrap();
        let mut fences = Vec::new();
        let mut offset = 1usize;
        for (term, p) in &values {
            let encoded = serde_json::to_vec(&(term, p)).unwrap();
            push_fence(
                &mut fences,
                term,
                offset as u64,
                (offset + encoded.len()) as u64,
                1,
            )
            .unwrap();
            offset += encoded.len() + 1;
        }
        let mut bloom = Bloom::default();
        for (t, _) in &values {
            bloom.add(t);
        }
        let pages = Pages::new(&file, fences, Vec::new(), bloom).unwrap();
        file.seek(SeekFrom::Start(4)).unwrap();
        file.write_all(b"!").unwrap();
        assert!(matches!(pages.get(&values[0].0), Err(Error::Corrupt(_))));
        drop(pages);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
}
