use crate::{
    Document, Key, StoredDocument,
    binary_batch::*,
    format::{Batch, Change},
};
use std::collections::BTreeMap;

#[test]
fn binary_batch_prefix_offsets_sizes_and_every_cut() {
    let stored = StoredDocument {
        version: 7,
        document: Document {
            key: Key::new("shared", "docs", "🦀"),
            title: "İSTANBUL".into(),
            text: "東京\n\\\"".into(),
            fields: BTreeMap::from([(
                "v".into(),
                serde_json::json!([null,true,i64::MIN,u64::MAX,-0.0,{"x":"ü"}]),
            )]),
            links: vec![Key::new("shared", "docs", "target")],
        },
    };
    let batch = Batch {
        checkpoint: true,
        changes: vec![
            Change::Put {
                stored: stored.clone(),
            },
            Change::Delete {
                key: stored.document.key.clone(),
            },
        ],
    };
    let bytes = encode(&batch).unwrap();
    assert_eq!(
        serde_json::to_vec(&decode(&bytes).unwrap()).unwrap(),
        serde_json::to_vec(&batch).unwrap()
    );
    assert_eq!(
        crate::payload::encoded_len(&stored.document).unwrap(),
        crate::payload::encode(&stored).unwrap().len()
    );
    let mut prefixed = vec![123; 24];
    encode_into(&batch, &mut prefixed).unwrap();
    assert_eq!(&prefixed[24..], bytes);
    for cut in 0..bytes.len() {
        assert!(decode(&bytes[..cut]).is_err(), "cut {cut}");
    }
    for position in [0, 1, 5, 6] {
        let mut bad = bytes.clone();
        bad[position] = 255;
        assert!(decode(&bad).is_err(), "schema at {position}");
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert!(decode(&bad).is_err());
    // Valid outer CRC cannot make invalid OFF1 offsets acceptable to the batch decoder.
    let mut bad = bytes.clone();
    bad[10 + 12..10 + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode(&bad).is_err());
}
