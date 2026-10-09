use crate::engine::Database;
use crate::index::{Posting, extend_posting, fold};
use crate::{Key, Query};
use std::borrow::Cow;
use std::sync::Arc;
impl Database {
    pub(crate) fn delta_matches(&self, query: &Query) -> Vec<Arc<Key>> {
        let empty = Posting::default();
        let mut postings: Vec<Cow<'_, Posting>> = vec![Cow::Borrowed(
            match &query.collection {
                Some(c) => self
                    .index
                    .collections
                    .get(&(query.scope.clone(), c.clone())),
                None => self.index.scopes.get(&query.scope),
            }
            .unwrap_or(&empty),
        )];
        for (field, value) in &query.exact {
            // Collection-bound filters use direct dictionary lookups.
            if let Some(c) = &query.collection {
                postings.push(Cow::Borrowed(
                    self.index
                        .exact
                        .get(&(
                            query.scope.clone(),
                            c.clone(),
                            field.clone(),
                            value.to_string(),
                        ))
                        .unwrap_or(&empty),
                ));
            } else {
                let mut p = Posting::default();
                for ((s, _, f, v), keys) in &self.index.exact {
                    if s == &query.scope && f == field && v == &value.to_string() {
                        extend_posting(&mut p, keys);
                    }
                }
                postings.push(Cow::Owned(p));
            }
        }
        for range in &query.ranges {
            let mut p = Posting::default();
            for ((s, c, f), values) in &self.index.numeric {
                if s == &query.scope
                    && query.collection.as_ref().is_none_or(|wanted| wanted == c)
                    && f == &range.field
                {
                    for (_, keys) in values.range(range.min..=range.max) {
                        extend_posting(&mut p, keys);
                    }
                }
            }
            postings.push(Cow::Owned(p));
        }
        if let Some(key) = &query.linked_to {
            postings.push(Cow::Borrowed(self.index.links.get(key).unwrap_or(&empty)));
        }
        let terms: Vec<String> = query.text.split_whitespace().map(fold).collect();
        for term in &terms {
            postings.extend(
                self.index
                    .text_postings(&query.scope, term)
                    .into_iter()
                    .map(|p| Cow::Borrowed(p.unwrap_or(&empty))),
            );
        }
        postings.sort_by_key(|p| p.len());
        let candidates = &postings[0];
        // Retain references to the index keys, not fresh copies of every match.
        // Only selected merge cursors need an owned public key.
        let keys: Vec<Arc<Key>> = candidates
            .iter()
            .filter(|key| {
                #[cfg(feature = "experimental-delta-bitmaps")]
                let id = key;
                #[cfg(not(feature = "experimental-delta-bitmaps"))]
                let id = *key;
                postings[1..].iter().all(|p| p.contains(id))
                    && terms.iter().all(|term| self.index.text_contains(id, term))
            })
            .map(|id| {
                #[cfg(feature = "experimental-delta-bitmaps")]
                let id = &id;
                self.index.posting_key(id)
            })
            .collect();
        #[cfg(feature = "experimental-compact-delta-postings")]
        {
            // Slot reuse and arbitrary ingest order cannot define public key order.
            let mut keys = keys;
            keys.sort_unstable();
            keys
        }
        #[cfg(not(feature = "experimental-compact-delta-postings"))]
        {
            keys
        }
    }
}
