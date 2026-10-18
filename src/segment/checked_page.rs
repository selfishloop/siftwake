//! Query-local copies of bytes checked during contains verification. No source
//! address or checksum exemption survives the query or applies to a fresh read.
use super::*;

const PAYLOAD_BUDGET: usize = 2 * 1024 * 1024;

pub(crate) struct CheckedPage {
    limit: usize,
    bytes: usize,
    entries: BTreeMap<Key, (usize, u32, Box<[u8]>)>,
}

impl CheckedPage {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit: limit.min(50),
            bytes: 0,
            entries: BTreeMap::new(),
        }
    }

    // Called only with the very slice whose CRC, schema, UTF-8 and key/version
    // have just passed. Copies are private and never mutated after admission.
    pub(super) fn retain(
        &mut self,
        run: usize,
        id: u32,
        view: &crate::payload::View<'_>,
        bytes: &[u8],
    ) {
        if self.limit == 0 || bytes.len() > PAYLOAD_BUDGET {
            return;
        }
        let parts = view.key_parts();
        while let Some((last, _)) = self.entries.last_key_value() {
            if self.entries.len() < self.limit && self.bytes + bytes.len() <= PAYLOAD_BUDGET {
                break;
            }
            if parts
                >= (
                    last.scope.as_str(),
                    last.collection.as_str(),
                    last.id.as_str(),
                )
            {
                return;
            }
            let (_, (_, _, old)) = self.entries.pop_last().unwrap();
            self.bytes -= old.len();
        }
        #[cfg(feature = "diagnostic-timing")]
        let _copy = crate::timing::span("query.retain_checked_payload");
        let key = Key::new(parts.0, parts.1, parts.2);
        let len = bytes.len();
        if let Some((_, _, old)) = self.entries.insert(key, (run, id, bytes.into())) {
            self.bytes -= old.len();
        }
        self.bytes += len;
    }

    pub(crate) fn take(
        &mut self,
        key: &Key,
        run: usize,
        id: u32,
    ) -> Result<Option<StoredDocument>> {
        let Some((source, slot, bytes)) = self.entries.remove(key) else {
            return Ok(None);
        };
        self.bytes -= bytes.len();
        if source != run || slot != id {
            return Err(Error::Corrupt("checked page slot mismatch".into()));
        }
        #[cfg(feature = "diagnostic-timing")]
        let _reuse = crate::timing::span("payload.reuse_checked");
        // Recheck schema/UTF-8 while decoding the same immutable owned bytes.
        // No second file read or CRC is needed; fresh reads always use both.
        Ok(Some(crate::payload::checked(&bytes)?.to_document()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, len: usize) -> StoredDocument {
        StoredDocument {
            document: crate::Document {
                key: Key::new("shared", "texts", id),
                title: "İstanbul 東京".into(),
                text: "needle 🦀".repeat(len),
                fields: BTreeMap::from([(
                    "nested".into(),
                    serde_json::json!({"list": [null, true, -17, 1.25, "ışık"]}),
                )]),
                links: vec![Key::new("shared", "other", "target")],
            },
            version: 77,
        }
    }

    fn retain(page: &mut CheckedPage, run: usize, slot: u32, document: &StoredDocument) {
        let bytes = crate::payload::encode(document).unwrap();
        page.retain(run, slot, &crate::payload::checked(&bytes).unwrap(), &bytes);
    }

    #[test]
    fn page_uses_global_keys_and_preserves_full_document() {
        let mut page = CheckedPage::new(2);
        retain(&mut page, 0, 0, &record("z", 1));
        retain(&mut page, 1, 9, &record("c", 1));
        retain(&mut page, 2, 8, &record("a", 1));
        retain(&mut page, 3, 1, &record("b", 1));
        assert_eq!(page.entries.len(), 2);
        assert!(
            page.take(&record("z", 1).document.key, 0, 0)
                .unwrap()
                .is_none()
        );
        for (id, run, slot) in [("a", 2, 8), ("b", 3, 1)] {
            let expected = record(id, 1);
            assert_eq!(
                page.take(&expected.document.key, run, slot).unwrap(),
                Some(expected)
            );
        }
        assert_eq!(page.bytes, 0);
    }

    #[test]
    fn page_bounds_payload_bytes_and_slots_with_oversized_fallback() {
        let mut page = CheckedPage::new(1000);
        for id in 0..80 {
            retain(&mut page, 0, id, &record(&format!("{id:03}"), 8000));
            assert!(page.bytes <= PAYLOAD_BUDGET);
            assert!(page.entries.len() <= 50);
        }
        let oversized = record("!large", 200_000);
        retain(&mut page, 4, 0, &oversized);
        assert!(page.take(&oversized.document.key, 4, 0).unwrap().is_none());
        let mut small = CheckedPage::new(1000);
        for id in (0..80).rev() {
            retain(&mut small, 0, id, &record(&format!("{id:03}"), 1));
        }
        assert_eq!(small.entries.len(), 50);
        assert_eq!(small.entries.last_key_value().unwrap().0.id, "049");
    }

    #[test]
    fn retained_bytes_are_owned_and_slot_identity_is_enforced() {
        let expected = record("a", 1);
        let mut bytes = crate::payload::encode(&expected).unwrap();
        let mut page = CheckedPage::new(1);
        page.retain(2, 3, &crate::payload::checked(&bytes).unwrap(), &bytes);
        bytes.fill(0xff);
        assert_eq!(
            page.take(&expected.document.key, 2, 3).unwrap(),
            Some(expected.clone())
        );
        retain(&mut page, 2, 3, &expected);
        assert!(matches!(
            page.take(&expected.document.key, 4, 3),
            Err(Error::Corrupt(_))
        ));
    }
}
