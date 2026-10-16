use crate::model::{Key, StoredDocument};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(crate) fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            match c {
                'I' | 'İ' | 'ı' => 'i',
                'Ç' | 'ç' => 'c',
                'Ğ' | 'ğ' => 'g',
                'Ö' | 'ö' => 'o',
                'Ş' | 'ş' => 's',
                'Ü' | 'ü' => 'u',
                _ => c,
            }
            .to_lowercase()
        })
        .collect()
}

// Find the first non-ASCII byte eight bytes at a time without unaligned or
// unchecked loads. The returned prefix always ends at a UTF-8 boundary.
fn ascii_prefix_word(bytes: &[u8]) -> usize {
    let (words, tail) = bytes.as_chunks::<8>();
    for (n, word) in words.iter().enumerate() {
        let high = u64::from_le_bytes(*word) & 0x8080_8080_8080_8080;
        if high != 0 {
            return n * 8 + high.trailing_zeros() as usize / 8;
        }
    }
    words.len() * 8
        + tail
            .iter()
            .position(|byte| !byte.is_ascii())
            .unwrap_or(tail.len())
}

fn ascii_prefix(bytes: &[u8]) -> usize {
    #[cfg(feature = "experimental-builder-budget")]
    {
        // Check four words together so a long ASCII run needs one branch per
        // 32 bytes. Once a block is mixed, locate its first high byte exactly.
        // All loads are safe, copied arrays; no alignment/CPU ISA assumption.
        let (blocks, tail) = bytes.as_chunks::<32>();
        for (n, block) in blocks.iter().enumerate() {
            let words = block.as_chunks::<8>().0;
            let combined = u64::from_le_bytes(words[0])
                | u64::from_le_bytes(words[1])
                | u64::from_le_bytes(words[2])
                | u64::from_le_bytes(words[3]);
            if combined & 0x8080_8080_8080_8080 != 0 {
                return n * 32 + ascii_prefix_word(block);
            }
        }
        blocks.len() * 32 + ascii_prefix_word(tail)
    }
    #[cfg(not(feature = "experimental-builder-budget"))]
    ascii_prefix_word(bytes)
}

/// Query scratch normalizer; identical to fold, without per-character Unicode
/// lowering for ASCII and without replacing the output allocation each time.
pub(crate) fn fold_into(text: &str, output: &mut String) {
    output.clear();
    if text.is_ascii() {
        output.push_str(text);
        output.make_ascii_lowercase();
        return;
    }
    output.reserve(text.len());
    #[cfg(feature = "experimental-builder-budget")]
    fold_changed_runs(text, output);
    #[cfg(not(feature = "experimental-builder-budget"))]
    {
        let mut offset = 0;
        while offset < text.len() {
            // A Unicode punctuation mark must not force the adjacent ASCII run
            // through char decoding and one String::push per byte.
            let remaining = &text[offset..];
            let ascii_len = ascii_prefix(remaining.as_bytes());
            if ascii_len != 0 {
                let start = output.len();
                output.push_str(&remaining[..ascii_len]);
                output[start..].make_ascii_lowercase();
                offset += ascii_len;
            }
            if offset == text.len() {
                break;
            }
            let original = text[offset..].chars().next().unwrap();
            let mapped = match original {
                'İ' | 'ı' => 'i',
                'Ç' | 'ç' => 'c',
                'Ğ' | 'ğ' => 'g',
                'Ö' | 'ö' => 'o',
                'Ş' | 'ş' => 's',
                'Ü' | 'ü' => 'u',
                _ => original,
            };
            output.extend(mapped.to_lowercase());
            offset += original.len_utf8();
        }
    }
}

// Unchanged Unicode punctuation stays in the same copied span as its ASCII
// neighbours. Split output only where Unicode folding actually changes bytes.
#[cfg(feature = "experimental-builder-budget")]
fn fold_changed_runs(text: &str, output: &mut String) {
    let mut copied = 0;
    let mut offset = 0;
    while offset < text.len() {
        let remaining = &text[offset..];
        offset += ascii_prefix(remaining.as_bytes());
        if offset == text.len() {
            break;
        }
        let original = text[offset..].chars().next().unwrap();
        let mapped = match original {
            'İ' | 'ı' => 'i',
            'Ç' | 'ç' => 'c',
            'Ğ' | 'ğ' => 'g',
            'Ö' | 'ö' => 'o',
            'Ş' | 'ş' => 's',
            'Ü' | 'ü' => 'u',
            _ => original,
        };
        let mut lower = mapped.to_lowercase();
        let first = lower.next().unwrap();
        let second = lower.next();
        if first != original || second.is_some() {
            let start = output.len();
            output.push_str(&text[copied..offset]);
            output[start..].make_ascii_lowercase();
            output.push(first);
            output.extend(second);
            output.extend(lower);
            copied = offset + original.len_utf8();
        }
        offset += original.len_utf8();
    }
    let start = output.len();
    output.push_str(&text[copied..]);
    output[start..].make_ascii_lowercase();
}

#[cfg(test)]
mod scratch_fold_tests {
    #[test]
    fn unchanged_unicode_runs_keep_ascii_lowering_and_changed_scalar_boundaries() {
        let mut scratch = String::new();
        for punctuation in ["“”,—…", "🦀東京", "éjà", "\u{0307}\u{0345}", "", "€£§"]
        {
            for changed in ["İŞIK", "ȺΣẞK", "ÇĞÖÜı", "\u{1f88}", "A", ""] {
                for prefix in [0, 1, 7, 8, 31, 32, 33, 1023] {
                    let text = format!(
                        "{}{punctuation}ABC{punctuation}{changed}XYZ{punctuation}{changed}",
                        "A".repeat(prefix)
                    );
                    super::fold_into(&text, &mut scratch);
                    assert_eq!(scratch, super::fold(&text));
                }
            }
        }
    }
    #[test]
    fn wide_ascii_prefix_handles_each_lane_and_unaligned_tails() {
        for len in [31, 32, 33, 63, 64, 65, 95, 96, 97, 127, 128, 129] {
            for position in 0..=len {
                for high in [128, 195, 255] {
                    let mut bytes = vec![b'a'; len + 32];
                    if position < len {
                        bytes[32 + position] = high;
                    }
                    for alignment in 0..32 {
                        let slice = &bytes[alignment..32 + len];
                        assert_eq!(
                            super::ascii_prefix(slice),
                            slice
                                .iter()
                                .position(|b| !b.is_ascii())
                                .unwrap_or(slice.len())
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn ascii_prefix_matches_byte_scan_at_every_alignment_and_boundary() {
        for len in 0..80 {
            for position in 0..=len {
                for high in 128..=255u8 {
                    let mut bytes = vec![b'a'; len + 8];
                    if position < len {
                        bytes[8 + position] = high;
                    }
                    for alignment in 0..8 {
                        let slice = &bytes[alignment..8 + len];
                        assert_eq!(
                            super::ascii_prefix(slice),
                            slice
                                .iter()
                                .position(|byte| !byte.is_ascii())
                                .unwrap_or(slice.len())
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn query_scratch_fold_matches_every_unicode_scalar_and_reuse() {
        let all: String = (0..=0x10ffff).filter_map(char::from_u32).collect();
        let mut scratch = String::new();
        super::fold_into(&all, &mut scratch);
        assert_eq!(scratch, super::fold(&all));
        for text in ["ASCII I XYZ", "", "İŞ ÇĞÖÜ ı Σς K ẞ 🦀", "tiny"] {
            super::fold_into(text, &mut scratch);
            assert_eq!(scratch, super::fold(text));
        }
    }
}

pub(crate) fn grams(text: &str) -> BTreeSet<String> {
    let mut chars = text.chars();
    let (Some(mut a), Some(mut b)) = (chars.next(), chars.next()) else {
        return BTreeSet::new();
    };
    let mut windows = BTreeSet::new();
    for c in chars {
        // Queries split on Unicode whitespace, so these grams are never used.
        if !a.is_whitespace() && !b.is_whitespace() && !c.is_whitespace() {
            windows.insert([a, b, c]);
        }
        a = b;
        b = c;
    }
    windows
        .into_iter()
        .map(|w| w.into_iter().collect())
        .collect()
}

#[cfg(feature = "experimental-builder-budget")]
mod packed_grams;

#[cfg(feature = "experimental-compact-delta-postings")]
pub(crate) type PostingKey = u32;
#[cfg(not(feature = "experimental-compact-delta-postings"))]
pub(crate) type PostingKey = Arc<Key>;
#[cfg(not(feature = "experimental-delta-bitmaps"))]
pub(crate) type Posting = BTreeSet<PostingKey>;
#[cfg(feature = "experimental-delta-bitmaps")]
mod posting;
#[cfg(feature = "experimental-delta-bitmaps")]
pub(crate) use posting::Posting;
pub(crate) fn extend_posting(target: &mut Posting, source: &Posting) {
    #[cfg(not(feature = "experimental-delta-bitmaps"))]
    target.extend(source.iter().cloned());
    #[cfg(feature = "experimental-delta-bitmaps")]
    target.extend(source.iter());
}
type FieldKey = (String, String, String, String);
type NumericKey = (String, String, String);

#[cfg(feature = "experimental-shared-delta-text")]
type FoldedBody = Arc<str>;
#[cfg(not(feature = "experimental-shared-delta-text"))]
type FoldedBody = String;

#[cfg(feature = "experimental-shared-delta-text")]
pub(crate) struct FoldedText {
    title: String,
    body: Arc<str>,
}
#[cfg(feature = "experimental-shared-delta-text")]
impl FoldedText {
    pub fn contains(&self, term: &str) -> bool {
        // Query terms cannot contain whitespace, so none can span the separator.
        self.title.contains(term) || self.body.contains(term)
    }
}
#[cfg(not(feature = "experimental-shared-delta-text"))]
type FoldedText = String;

#[derive(Clone)]
pub(crate) struct BodyTerms {
    pub folded: FoldedBody,
    pub grams: BTreeSet<String>,
}
#[derive(Default)]
pub(crate) struct BodyCache {
    entries: BTreeMap<String, BodyTerms>,
    bytes: usize,
}
impl BodyCache {
    pub fn get(&mut self, text: &str) -> Cow<'_, BodyTerms> {
        if self.entries.contains_key(text) {
            return Cow::Borrowed(self.entries.get(text).unwrap());
        }
        let folded = {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("body.fold");
            fold(text)
        };
        #[cfg(feature = "experimental-shared-delta-text")]
        let folded: FoldedBody = folded.into();
        let body_grams = {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("body.grams");
            #[cfg(feature = "experimental-builder-budget")]
            if folded.len() >= 1024 {
                packed_grams::extract(&folded)
            } else {
                grams(&folded)
            }
            #[cfg(not(feature = "experimental-builder-budget"))]
            grams(&folded)
        };
        let value = BodyTerms {
            grams: body_grams,
            folded,
        };
        let cost = text.len()
            + value.folded.len()
            + value.grams.iter().map(|g| g.len() + 64).sum::<usize>()
            + 128;
        if cost <= 256 * 1024 - self.bytes {
            self.bytes += cost;
            self.entries.insert(text.to_owned(), value);
            Cow::Borrowed(self.entries.get(text).unwrap())
        } else {
            Cow::Owned(value)
        }
    }
}

#[cfg(feature = "experimental-compact-delta-postings")]
struct Slot {
    key: Arc<Key>,
    text: FoldedText,
}

#[derive(Default)]
pub(crate) struct Index {
    pub scopes: BTreeMap<String, Posting>,
    pub collections: BTreeMap<(String, String), Posting>,
    pub exact: BTreeMap<FieldKey, Posting>,
    pub numeric: BTreeMap<NumericKey, BTreeMap<i64, Posting>>,
    // Scope is owned once per dictionary, not once per gram or update probe.
    pub grams: BTreeMap<String, BTreeMap<String, Posting>>,
    pub links: BTreeMap<Key, Posting>,
    #[cfg(not(feature = "experimental-compact-delta-postings"))]
    pub folded: BTreeMap<Arc<Key>, FoldedText>,
    #[cfg(feature = "experimental-compact-delta-postings")]
    key_ids: BTreeMap<Arc<Key>, u32>,
    #[cfg(feature = "experimental-compact-delta-postings")]
    slots: Vec<Option<Slot>>,
    #[cfg(feature = "experimental-compact-delta-postings")]
    free: Vec<u32>,
    body_cache: BodyCache,
    #[cfg(feature = "experimental-builder-budget")]
    builder_gram_bytes: usize,
}

fn change<K: Ord>(map: &mut BTreeMap<K, Posting>, index_key: K, key: &PostingKey, add: bool) {
    if add {
        #[cfg(feature = "experimental-compact-delta-postings")]
        let key = *key;
        #[cfg(not(feature = "experimental-compact-delta-postings"))]
        let key = key.clone();
        map.entry(index_key).or_default().insert(key);
    } else if let Some(p) = map.get_mut(&index_key) {
        p.remove(key);
        if p.is_empty() {
            map.remove(&index_key);
        }
    }
}

impl Index {
    // Export the already-built delta dictionary without scanning document text.
    #[cfg(feature = "experimental-merge-postings")]
    pub(crate) fn visit_terms(
        &self,
        mut emit: impl FnMut(crate::segment::Term, &Posting) -> crate::Result<()>,
    ) -> crate::Result<()> {
        use crate::segment::Term;
        for (scope, posting) in &self.scopes {
            emit(Term::Scope(scope.clone()), posting)?;
        }
        for ((scope, collection), posting) in &self.collections {
            emit(Term::Collection(scope.clone(), collection.clone()), posting)?;
        }
        for ((scope, collection, field, value), posting) in &self.exact {
            emit(
                Term::Exact(
                    scope.clone(),
                    collection.clone(),
                    field.clone(),
                    value.clone(),
                ),
                posting,
            )?;
        }
        for ((scope, collection, field), values) in &self.numeric {
            for (value, posting) in values {
                emit(
                    Term::Numeric(scope.clone(), collection.clone(), field.clone(), *value),
                    posting,
                )?;
            }
        }
        for (scope, grams) in &self.grams {
            for (gram, posting) in grams {
                emit(Term::Gram(scope.clone(), gram.clone()), posting)?;
            }
        }
        for (key, posting) in &self.links {
            emit(Term::Link(key.clone()), posting)?;
        }
        Ok(())
    }

    #[cfg(feature = "experimental-merge-postings")]
    pub(crate) fn disk_ids(
        &self,
        posting: &Posting,
        records: &BTreeMap<Key, u32>,
    ) -> crate::Result<Vec<u32>> {
        let mut ids = Vec::with_capacity(posting.len());
        let mut add = |id: &PostingKey| -> crate::Result<()> {
            #[cfg(not(feature = "experimental-compact-delta-postings"))]
            let key = id.as_ref();
            #[cfg(feature = "experimental-compact-delta-postings")]
            let key = self
                .slots
                .get(*id as usize)
                .and_then(Option::as_ref)
                .ok_or_else(|| crate::Error::Corrupt("invalid delta posting slot".into()))?
                .key
                .as_ref();
            ids.push(*records.get(key).ok_or_else(|| {
                crate::Error::Corrupt("delta posting absent from checkpoint".into())
            })?);
            Ok(())
        };
        #[cfg(feature = "experimental-delta-bitmaps")]
        for id in posting.iter() {
            add(&id)?;
        }
        #[cfg(not(feature = "experimental-delta-bitmaps"))]
        for id in posting {
            add(id)?;
        }
        // Recycled RAM slot IDs have no persistent/key-order meaning.
        ids.sort_unstable();
        if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(crate::Error::Corrupt(
                "duplicate delta checkpoint posting".into(),
            ));
        }
        Ok(ids)
    }

    #[cfg(feature = "experimental-compact-delta-postings")]
    pub fn check_capacity(&self, additions: usize) -> crate::Result<()> {
        let required = self
            .slots
            .len()
            .checked_add(additions.saturating_sub(self.free.len()));
        if required.is_none_or(|count| count as u128 > u32::MAX as u128 + 1) {
            return Err(crate::Error::Invalid(
                "delta posting ID capacity exceeded".into(),
            ));
        }
        Ok(())
    }

    pub fn change(&mut self, stored: &StoredDocument, add: bool) -> crate::Result<()> {
        self.change_with_key(stored, add, None)
    }

    #[cfg(all(
        feature = "experimental-shared-documents",
        not(feature = "experimental-wal-delta")
    ))]
    pub fn change_shared(
        &mut self,
        stored: &StoredDocument,
        add: bool,
        key: Arc<Key>,
    ) -> crate::Result<()> {
        if key.as_ref() != &stored.document.key {
            return Err(crate::Error::Corrupt("shared delta key mismatch".into()));
        }
        self.change_with_key(stored, add, Some(key))
    }

    fn change_with_key(
        &mut self,
        stored: &StoredDocument,
        add: bool,
        provided: Option<Arc<Key>>,
    ) -> crate::Result<()> {
        self.change_impl(stored, add, provided, true)
    }
    #[cfg(feature = "experimental-builder-budget")]
    pub(crate) fn add_builder(&mut self, stored: &StoredDocument) -> crate::Result<()> {
        self.change_impl(stored, true, None, false)
    }
    fn change_impl(
        &mut self,
        stored: &StoredDocument,
        add: bool,
        provided: Option<Arc<Key>>,
        retain_text: bool,
    ) -> crate::Result<()> {
        let doc = &stored.document;
        #[cfg(not(feature = "experimental-compact-delta-postings"))]
        let shared = provided.unwrap_or_else(|| Arc::new(doc.key.clone()));
        #[cfg(not(feature = "experimental-compact-delta-postings"))]
        let posting_key = shared.clone();
        #[cfg(feature = "experimental-compact-delta-postings")]
        let (shared, posting_key) = if add {
            if self.key_ids.contains_key(&doc.key) {
                return Err(crate::Error::Corrupt("duplicate delta index key".into()));
            }
            self.check_capacity(1)?;
            let id = match self.free.pop() {
                Some(id) => id,
                None => {
                    let id = u32::try_from(self.slots.len()).map_err(|_| {
                        crate::Error::Invalid("delta posting ID capacity exceeded".into())
                    })?;
                    self.slots.push(None);
                    id
                }
            };
            let key = provided.unwrap_or_else(|| Arc::new(doc.key.clone()));
            self.key_ids.insert(key.clone(), id);
            (key, id)
        } else {
            let (key, id) = self
                .key_ids
                .get_key_value(&doc.key)
                .ok_or_else(|| crate::Error::Corrupt("missing delta index key".into()))?;
            (key.clone(), *id)
        };
        let key = &shared;
        change(&mut self.scopes, key.scope.clone(), &posting_key, add);
        change(
            &mut self.collections,
            (key.scope.clone(), key.collection.clone()),
            &posting_key,
            add,
        );
        for (name, value) in &doc.fields {
            if value.is_array() || value.is_object() {
                continue;
            }
            change(
                &mut self.exact,
                (
                    key.scope.clone(),
                    key.collection.clone(),
                    name.clone(),
                    value.to_string(),
                ),
                &posting_key,
                add,
            );
            if let Some(n) = value.as_i64() {
                let nk = (key.scope.clone(), key.collection.clone(), name.clone());
                if add {
                    change(self.numeric.entry(nk).or_default(), n, &posting_key, true);
                } else if let Some(values) = self.numeric.get_mut(&nk) {
                    change(values, n, &posting_key, false);
                    if values.is_empty() {
                        self.numeric.remove(&nk);
                    }
                }
            }
        }
        for target in &doc.links {
            change(&mut self.links, target.clone(), &posting_key, add);
        }
        let title = fold(&doc.title);
        let title_grams = grams(&title);
        let body = self.body_cache.get(&doc.text);
        #[cfg(feature = "experimental-builder-budget")]
        if !retain_text && !self.grams.contains_key(&key.scope) {
            self.builder_gram_bytes += 96 + key.scope.len();
        }
        let scope_grams = if add {
            Some(self.grams.entry(key.scope.clone()).or_default())
        } else {
            self.grams.get_mut(&key.scope)
        };
        if let Some(scope_grams) = scope_grams {
            for gram in title_grams.union(&body.grams) {
                if add {
                    if let Some(posting) = scope_grams.get_mut(gram) {
                        #[cfg(feature = "experimental-builder-budget")]
                        let before = posting.heap_bytes();
                        #[cfg(feature = "experimental-compact-delta-postings")]
                        posting.insert(posting_key);
                        #[cfg(not(feature = "experimental-compact-delta-postings"))]
                        posting.insert(posting_key.clone());
                        #[cfg(feature = "experimental-builder-budget")]
                        if !retain_text {
                            let after = posting.heap_bytes();
                            if after >= before {
                                self.builder_gram_bytes += after - before;
                            } else {
                                self.builder_gram_bytes -= before - after;
                            }
                        }
                    } else {
                        change(scope_grams, gram.clone(), &posting_key, true);
                        #[cfg(feature = "experimental-builder-budget")]
                        if !retain_text {
                            self.builder_gram_bytes += 96 + gram.len();
                        }
                    }
                } else if let Some(posting) = scope_grams.get_mut(gram) {
                    posting.remove(&posting_key);
                    if posting.is_empty() {
                        scope_grams.remove(gram);
                    }
                }
            }
        }
        if !add && self.grams.get(&key.scope).is_some_and(BTreeMap::is_empty) {
            self.grams.remove(&key.scope);
        }
        if add {
            #[cfg(feature = "experimental-shared-delta-text")]
            let text = FoldedText {
                title: if retain_text { title } else { String::new() },
                body: if retain_text {
                    body.folded.clone()
                } else {
                    Arc::from("")
                },
            };
            #[cfg(not(feature = "experimental-shared-delta-text"))]
            let text = if retain_text {
                format!("{title}\n{}", body.folded)
            } else {
                String::new()
            };
            #[cfg(not(feature = "experimental-compact-delta-postings"))]
            self.folded.insert(key.clone(), text);
            #[cfg(feature = "experimental-compact-delta-postings")]
            {
                self.slots[posting_key as usize] = Some(Slot { key: shared, text });
            }
        } else {
            #[cfg(not(feature = "experimental-compact-delta-postings"))]
            self.folded.remove(key);
            #[cfg(feature = "experimental-compact-delta-postings")]
            {
                self.key_ids.remove(key);
                self.slots[posting_key as usize] = None;
                self.free.push(posting_key);
            }
        }
        Ok(())
    }
    #[cfg(feature = "experimental-builder-budget")]
    pub(crate) fn builder_memory_bytes(&self) -> usize {
        let post = |p: &Posting| p.heap_bytes();
        let key = |k: &Key| k.scope.capacity() + k.collection.capacity() + k.id.capacity();
        self.body_cache.bytes
            + self.builder_gram_bytes
            + self
                .scopes
                .iter()
                .map(|(s, p)| 96 + s.capacity() + post(p))
                .sum::<usize>()
            + self
                .collections
                .iter()
                .map(|((s, c), p)| 120 + s.capacity() + c.capacity() + post(p))
                .sum::<usize>()
            + self
                .exact
                .iter()
                .map(|((s, c, f, v), p)| {
                    176 + s.capacity() + c.capacity() + f.capacity() + v.capacity() + post(p)
                })
                .sum::<usize>()
            + self
                .numeric
                .iter()
                .map(|((s, c, f), values)| {
                    144 + s.capacity()
                        + c.capacity()
                        + f.capacity()
                        + values.values().map(|p| 64 + post(p)).sum::<usize>()
                })
                .sum::<usize>()
            + self
                .links
                .iter()
                .map(|(k, p)| 144 + key(k) + post(p))
                .sum::<usize>()
            + self.key_ids.keys().map(|k| 144 + key(k)).sum::<usize>()
            + self.slots.capacity() * std::mem::size_of::<Option<Slot>>()
    }

    fn texts(&self) -> impl Iterator<Item = &FoldedText> {
        #[cfg(not(feature = "experimental-compact-delta-postings"))]
        {
            self.folded.values()
        }
        #[cfg(feature = "experimental-compact-delta-postings")]
        {
            self.slots
                .iter()
                .filter_map(|slot| slot.as_ref().map(|slot| &slot.text))
        }
    }
    pub fn posting_key(&self, id: &PostingKey) -> Arc<Key> {
        #[cfg(not(feature = "experimental-compact-delta-postings"))]
        {
            id.clone()
        }
        #[cfg(feature = "experimental-compact-delta-postings")]
        {
            self.slots[*id as usize]
                .as_ref()
                .expect("live posting slot")
                .key
                .clone()
        }
    }
    pub fn text_contains(&self, id: &PostingKey, term: &str) -> bool {
        #[cfg(not(feature = "experimental-compact-delta-postings"))]
        {
            self.folded[id].contains(term)
        }
        #[cfg(feature = "experimental-compact-delta-postings")]
        {
            self.slots[*id as usize]
                .as_ref()
                .expect("live posting slot")
                .text
                .contains(term)
        }
    }
    #[cfg(all(test, feature = "experimental-shared-delta-text"))]
    fn text_for_key(&self, key: &Key) -> &FoldedText {
        #[cfg(not(feature = "experimental-compact-delta-postings"))]
        {
            &self.folded[key]
        }
        #[cfg(feature = "experimental-compact-delta-postings")]
        {
            &self.slots[self.key_ids[key] as usize]
                .as_ref()
                .unwrap()
                .text
        }
    }
    pub fn folded_bytes(&self) -> usize {
        #[cfg(not(feature = "experimental-shared-delta-text"))]
        {
            self.texts().map(String::len).sum()
        }
        #[cfg(feature = "experimental-shared-delta-text")]
        {
            let mut bodies = std::collections::HashSet::new();
            let mut bytes = self
                .texts()
                .map(|text| {
                    let body_bytes = if bodies.insert(Arc::as_ptr(&text.body) as *const u8) {
                        text.body.len()
                    } else {
                        0
                    };
                    text.title.len() + body_bytes
                })
                .sum::<usize>();
            // Cached bodies may outlive the last delta record referencing them.
            for body in self.body_cache.entries.values() {
                if bodies.insert(Arc::as_ptr(&body.folded) as *const u8) {
                    bytes += body.folded.len();
                }
            }
            bytes
        }
    }

    pub fn text_postings(&self, scope: &str, text: &str) -> Vec<Option<&Posting>> {
        grams(text)
            .into_iter()
            .map(|g| self.grams.get(scope).and_then(|grams| grams.get(&g)))
            .collect()
    }
}

#[cfg(all(
    test,
    feature = "experimental-shared-documents",
    not(feature = "experimental-wal-delta")
))]
mod key_sharing_tests {
    use super::*;
    #[test]
    fn shared_identity_survives_indexing_and_wrong_key_rejects_before_mutation() {
        let stored = StoredDocument {
            version: 7,
            document: crate::Document {
                key: Key::new("ortak", "belgeler", "ş-ü"),
                title: "Şirket".into(),
                text: "teknoloji".into(),
                fields: [("day".into(), serde_json::json!(5))].into(),
                links: vec![Key::new("ortak", "belgeler", "other")],
            },
        };
        let key = Arc::new(stored.document.key.clone());
        let mut index = Index::default();
        index.change_shared(&stored, true, key.clone()).unwrap();
        let id = index.scopes["ortak"].iter().next().unwrap();
        #[cfg(not(feature = "experimental-delta-bitmaps"))]
        let id = id.clone();
        let retained = index.posting_key(&id);
        assert!(Arc::ptr_eq(&retained, &key));
        let wrong = Arc::new(Key::new("ortak", "belgeler", "wrong"));
        assert!(index.change_shared(&stored, true, wrong.clone()).is_err());
        assert!(index.change_shared(&stored, false, wrong).is_err());
        assert_eq!(index.scopes["ortak"].len(), 1);
        assert!(index.text_contains(&id, "teknoloji"));
        index.change_shared(&stored, false, key).unwrap();
        assert!(index.scopes.is_empty() && index.grams.is_empty() && index.links.is_empty());
    }
}

#[cfg(all(test, feature = "experimental-shared-delta-text"))]
mod shared_tests {
    use super::*;
    use crate::model::Document;
    fn record(id: &str, title: &str, text: &str) -> StoredDocument {
        StoredDocument {
            version: 1,
            document: Document {
                key: Key::new("shared", "reports", id),
                title: title.into(),
                text: text.into(),
                fields: BTreeMap::new(),
                links: vec![],
            },
        }
    }
    #[test]
    fn shared_bodies_preserve_titles_unicode_and_removal() {
        let mut index = Index::default();
        let a = record("a", "İLK", "Şirket büyüyor");
        let b = record("b", "SON", "Şirket büyüyor");
        index.change(&a, true).unwrap();
        index.change(&b, true).unwrap();
        let left = index.text_for_key(&a.document.key);
        let right = index.text_for_key(&b.document.key);
        assert!(Arc::ptr_eq(&left.body, &right.body));
        assert!(left.contains("ilk") && !right.contains("ilk"));
        assert!(left.contains("sirket") && right.contains("buyuyor"));
        assert_eq!(
            index.folded_bytes(),
            "ilk".len() + "son".len() + fold(&a.document.text).len()
        );
        index.change(&a, false).unwrap();
        assert_eq!(index.scopes["shared"].len(), 1);
        let changed = record("a", "Yeni", "Farklı metin");
        index.change(&changed, true).unwrap();
        assert!(!index.text_for_key(&changed.document.key).contains("sirket"));
        assert!(index.text_for_key(&b.document.key).contains("sirket"));
    }
    #[test]
    fn uncached_large_distinct_bodies_are_owned_and_searchable() {
        let mut index = Index::default();
        let a = record("a", "", &"a".repeat(300_000));
        let b = record("b", "", &"b".repeat(300_000));
        index.change(&a, true).unwrap();
        index.change(&b, true).unwrap();
        assert!(index.body_cache.entries.is_empty());
        assert_eq!(index.folded_bytes(), 600_000);
        assert!(index.text_for_key(&a.document.key).contains("aaa"));
        assert!(!index.text_for_key(&b.document.key).contains("aaa"));
        index.change(&a, false).unwrap();
        assert_eq!(index.folded_bytes(), 300_000);
    }
}

#[cfg(all(test, feature = "experimental-compact-delta-postings"))]
mod compact_tests {
    use super::*;
    use crate::model::Document;
    fn doc(id: usize, text: &str) -> StoredDocument {
        StoredDocument {
            version: 1,
            document: Document {
                key: Key::new("shared", "records", &id.to_string()),
                title: id.to_string(),
                text: text.into(),
                fields: [("day".into(), serde_json::json!(id % 3))].into(),
                links: vec![Key::new("shared", "target", "a")],
            },
        }
    }
    fn live_postings(index: &Index) {
        let lists = index
            .scopes
            .values()
            .chain(index.collections.values())
            .chain(index.exact.values())
            .chain(index.grams.values().flat_map(BTreeMap::values))
            .chain(index.links.values())
            .chain(index.numeric.values().flat_map(|values| values.values()));
        for list in lists {
            #[cfg(not(feature = "experimental-delta-bitmaps"))]
            let ids = list.iter().copied();
            #[cfg(feature = "experimental-delta-bitmaps")]
            let ids = list.iter();
            for id in ids {
                let slot = index.slots[id as usize].as_ref().unwrap();
                assert_eq!(index.key_ids[&slot.key], id);
            }
        }
    }
    #[test]
    fn slots_recycle_without_stale_terms_or_growth_across_updates() {
        let mut index = Index::default();
        let mut docs: Vec<_> = (0..50).map(|id| doc(id, "old body")).collect();
        for stored in docs.iter().rev() {
            index.change(stored, true).unwrap();
        }
        for turn in 0..1000 {
            let id = turn % docs.len();
            index.change(&docs[id], false).unwrap();
            live_postings(&index);
            docs[id] = doc(
                id,
                if turn % 2 == 0 {
                    "new body"
                } else {
                    "other body"
                },
            );
            index.change(&docs[id], true).unwrap();
            assert_eq!(index.slots.len(), 50);
            assert!(index.free.is_empty());
        }
        assert!(index.text_postings("shared", "old")[0].is_none());
        live_postings(&index);
        for stored in &docs {
            index.change(stored, false).unwrap();
        }
        assert!(index.slots.iter().all(Option::is_none));
        assert_eq!(index.free.len(), 50);
        assert!(index.key_ids.is_empty());
        assert!(index.scopes.is_empty() && index.grams.is_empty() && index.links.is_empty());
    }
    #[test]
    fn capacity_and_duplicate_checks_leave_index_unchanged() {
        let mut index = Index::default();
        let stored = doc(0, "test");
        index.change(&stored, true).unwrap();
        assert!(index.check_capacity(usize::MAX).is_err());
        assert!(index.change(&stored, true).is_err());
        assert_eq!(index.slots.len(), 1);
        assert_eq!(index.scopes["shared"].len(), 1);
        index.change(&stored, false).unwrap();
        assert!(index.change(&stored, false).is_err());
        assert_eq!(index.free.len(), 1);
        live_postings(&index);
    }
}
