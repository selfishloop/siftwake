//! Independent oracle for the explicitly supported synthetic workload.
//! Full payload/version/order checks happen outside timed query intervals.
use fdb::StoredDocument;

pub fn expected(
    kind: &str,
    count: usize,
    body: &str,
    changed: bool,
    offset: usize,
) -> (usize, Vec<StoredDocument>) {
    let selected = (0..count).filter(|id| {
        let mutation = changed && id.is_multiple_of(count / 100) && id / (count / 100) < 100;
        let deleted = mutation && (id / (count / 100)).is_multiple_of(2);
        !deleted
            && match kind {
                "narrow" => *id == count / 2,
                "range" => (2..=5).contains(&(id % 30)),
                "text" => !mutation,
                "exact" => true,
                _ => panic!("unknown oracle query"),
            }
    });
    let total = selected.clone().count();
    let hits = selected
        .skip(offset)
        .take(50)
        .map(|id| StoredDocument {
            document: super::corpus::document(
                id,
                if changed && id.is_multiple_of(count / 100) && id / (count / 100) < 100 {
                    "changed sentinel"
                } else {
                    body
                },
            ),
            version: if changed && id.is_multiple_of(count / 100) && id / (count / 100) < 100 {
                count.div_ceil(500) as u64 + 1
            } else {
                (id / 500 + 1) as u64
            },
        })
        .collect();
    (total, hits)
}

pub fn get(id: usize, body: &str) -> StoredDocument {
    StoredDocument {
        document: super::corpus::document(id, body),
        version: (id / 500 + 1) as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oracle_preserves_versions_pages_and_update_delete_semantics() {
        let (total, page) = expected("exact", 1000, "body", false, 950);
        assert_eq!(total, 1000);
        assert_eq!(page.len(), 50);
        assert_eq!(page[0], get(950, "body"));
        assert_eq!(page[49].version, 2);
        let (total, page) = expected("text", 1000, "body", true, 0);
        assert_eq!(total, 900);
        assert_eq!(page[0], get(1, "body"));
        let (total, page) = expected("exact", 1000, "body", true, 0);
        assert_eq!(total, 950);
        let changed = page
            .iter()
            .find(|d| d.document.key == super::super::corpus::key(10))
            .unwrap();
        assert_eq!(changed.document.text, "changed sentinel");
        assert!(expected("exact", 1000, "body", false, 1000).1.is_empty());
        assert_eq!(
            expected("range", 1000, "body", false, 0).0,
            (0..1000).filter(|n| (2..=5).contains(&(n % 30))).count()
        );
    }
}
