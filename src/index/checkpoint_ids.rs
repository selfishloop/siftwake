//! One checked RAM-slot to checkpoint-ID mapping for an entire flush.
//! Persistent IDs, term ordering and posting bytes retain their existing form.
use super::{Index, Key, Posting};
use crate::{Error, Result};
use std::collections::BTreeMap;

pub(crate) struct CheckpointIds {
    slots: Vec<u32>,
}

impl Index {
    pub(crate) fn checkpoint_ids(&self, records: &BTreeMap<Key, u32>) -> Result<CheckpointIds> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("delta.build_id_map");
        let mut slots = vec![u32::MAX; self.slots.len()];
        for (slot, stored) in self.slots.iter().enumerate() {
            let Some(stored) = stored else { continue };
            #[cfg(feature = "diagnostic-timing")]
            crate::timing::count("delta.checkpoint_key_lookups", 1);
            let id = *records
                .get(stored.key.as_ref())
                .ok_or_else(|| Error::Corrupt("delta posting absent from checkpoint".into()))?;
            if id as usize >= records.len() {
                return Err(Error::Corrupt("delta checkpoint posting bounds".into()));
            }
            slots[slot] = id;
        }
        Ok(CheckpointIds { slots })
    }
}

impl CheckpointIds {
    pub(crate) fn disk_ids(&self, posting: &Posting) -> Result<Vec<u32>> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("delta.remap_posting_ids");
        #[cfg(feature = "diagnostic-timing")]
        crate::timing::count("delta.remapped_entries", posting.len());
        let mut ids = Vec::with_capacity(posting.len());
        for slot in posting.iter() {
            let id = self
                .slots
                .get(slot as usize)
                .copied()
                .filter(|id| *id != u32::MAX)
                .ok_or_else(|| Error::Corrupt("invalid delta posting slot".into()))?;
            ids.push(id);
        }
        // RAM slots can be recycled in a different order from persistent IDs.
        ids.sort_unstable();
        if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Error::Corrupt("duplicate delta checkpoint posting".into()));
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, StoredDocument};

    fn document(id: usize) -> StoredDocument {
        StoredDocument {
            document: Document {
                key: Key::new("shared", "records", &format!("{id:04}")),
                title: "İstanbul 東京".into(),
                text: "şirket ortak metin".into(),
                fields: [("day".into(), serde_json::json!(id % 3))].into(),
                links: vec![Key::new("shared", "target", "a")],
            },
            version: 1,
        }
    }

    #[test]
    fn checkpoint_ids_match_key_lookup_after_deletes_recycling_and_unsorted_intake() {
        let mut index = Index::default();
        let mut live: BTreeMap<_, _> = (0..90).map(|id| (id, document(id))).collect();
        for stored in live.values().rev() {
            index.change(stored, true).unwrap();
        }
        for id in (0..90).step_by(3) {
            index.change(&live.remove(&id).unwrap(), false).unwrap();
        }
        for id in 90..110 {
            let stored = document(id);
            index.change(&stored, true).unwrap();
            live.insert(id, stored);
        }
        let records: BTreeMap<_, _> = live
            .values()
            .enumerate()
            .map(|(id, s)| (s.document.key.clone(), id as u32))
            .collect();
        let remap = index.checkpoint_ids(&records).unwrap();
        assert_eq!(remap.slots.len(), 90);
        index
            .visit_terms(|_, posting| {
                assert_eq!(remap.disk_ids(posting)?, index.disk_ids(posting, &records)?);
                Ok(())
            })
            .unwrap();
        // A tombstone belongs to checkpoint metadata without a live RAM slot.
        let mut with_tombstone = records.clone();
        with_tombstone.insert(document(200).document.key, records.len() as u32);
        assert!(index.checkpoint_ids(&with_tombstone).is_ok());
        let free = index.free[0];
        let mut invalid = Posting::default();
        invalid.insert(free);
        assert!(remap.disk_ids(&invalid).is_err());
        invalid.insert(99999);
        assert!(remap.disk_ids(&invalid).is_err());
        let mut missing = records.clone();
        missing.pop_first();
        assert!(index.checkpoint_ids(&missing).is_err());
        let mut bad_bounds = records;
        *bad_bounds.first_entry().unwrap().get_mut() = u32::MAX;
        assert!(index.checkpoint_ids(&bad_bounds).is_err());
    }

    #[test]
    fn checkpoint_ids_reject_duplicate_ids_and_release_empty_input() {
        let remap = Index::default().checkpoint_ids(&BTreeMap::new()).unwrap();
        assert!(remap.slots.is_empty());
        assert!(remap.disk_ids(&Posting::default()).unwrap().is_empty());
        let invalid = CheckpointIds { slots: vec![7, 7] };
        let mut posting = Posting::default();
        posting.insert(0);
        posting.insert(1);
        assert!(invalid.disk_ids(&posting).is_err());
        let mut outside = Posting::default();
        outside.insert(2);
        assert!(invalid.disk_ids(&outside).is_err());
    }
}
