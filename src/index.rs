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
        let folded = fold(text);
        #[cfg(feature = "experimental-shared-delta-text")]
        let folded: FoldedBody = folded.into();
        let value = BodyTerms {
            grams: grams(&folded),
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
    pub grams: BTreeMap<(String, String), Posting>,
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
        for gram in title_grams.union(&body.grams) {
            change(
                &mut self.grams,
                (key.scope.clone(), gram.clone()),
                &posting_key,
                add,
            );
        }
        if add {
            #[cfg(feature = "experimental-shared-delta-text")]
            let text = FoldedText {
                title,
                body: body.folded.clone(),
            };
            #[cfg(not(feature = "experimental-shared-delta-text"))]
            let text = format!("{title}\n{}", body.folded);
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
            .map(|g| self.grams.get(&(scope.to_owned(), g)))
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
            .chain(index.grams.values())
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
