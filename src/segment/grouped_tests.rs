use super::*;
use crate::Document;

struct TempIndex(std::path::PathBuf);
impl TempIndex {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "fdb-grouped-{}-{}-{}.idx", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            TEMP.fetch_add(1, Ordering::Relaxed))))
    }
    fn build(&self, docs: &[StoredDocument], delete: bool) -> DiskIndex {
        let mut builder = Builder::new(&self.0, 1, 40, 0).unwrap();
        for doc in docs {
            builder.add(doc).unwrap();
        }
        if delete {
            builder
                .add_delete(&Key::new("shared", "contents", "99999999"))
                .unwrap();
        }
        let index = builder.finish().unwrap();
        #[cfg(feature = "experimental-streaming-metadata")]
        {
            // These fault injections mutate the resident metadata reference.
            drop(index);
            DiskIndex::open_with_mode(&self.0, None, false).unwrap()
        }
        #[cfg(not(feature = "experimental-streaming-metadata"))]
        {
            index
        }
    }
    fn replace(&self, offset: u64, bytes: &[u8]) {
        let mut file = OpenOptions::new().write(true).open(&self.0).unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.write_all(bytes).unwrap();
        file.flush().unwrap();
    }
}
impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        #[cfg(feature = "experimental-streaming-metadata")]
        let _ = std::fs::remove_file(self.0.with_extension("records04.idx"));
    }
}
fn doc(id: usize, text: String) -> StoredDocument {
    StoredDocument {
        version: 1,
        document: Document {
            key: Key::new("shared", "contents", &format!("{id:08}")),
            title: "Şirket 文書 😀".into(),
            text,
            fields: BTreeMap::new(),
            links: vec![],
        },
    }
}

#[test]
fn adjacent_and_gapped_slots_share_one_window_with_independent_payload_checks() {
    let temp = TempIndex::new();
    let docs: Vec<_> = (0..20)
        .map(|i| doc(i, "şirket teknoloji 😀".into()))
        .collect();
    let index = temp.build(&docs, false);
    let mut scratch = vec![];
    let mut output = vec![];
    let stats = index
        .read_selected_batch(&[(0, 9), (2, 4), (5, 2)], &mut scratch, &mut output)
        .unwrap();
    assert_eq!(stats.requests, 1);
    assert_eq!(
        stats.requested_bytes,
        index.records[5].offset + index.records[5].len as u64 - index.records[0].offset
    );
    assert_eq!(
        output,
        vec![
            (9, docs[0].clone()),
            (4, docs[2].clone()),
            (2, docs[5].clone())
        ]
    );
    assert!(scratch.capacity() <= READ_WINDOW);

    // An unselected payload in the read window is not parsed or returned.
    temp.replace(index.records[1].offset, &[255]);
    output.clear();
    index
        .read_selected_batch(&[(0, 0), (2, 1)], &mut scratch, &mut output)
        .unwrap();
    assert_eq!(output, vec![(0, docs[0].clone()), (1, docs[2].clone())]);
    temp.replace(index.records[2].offset, &[255]);
    assert!(matches!(
        index.read_selected_batch(&[(0, 0), (2, 1)], &mut scratch, &mut output),
        Err(Error::Corrupt(_))
    ));
}

#[test]
fn window_budget_gaps_and_oversized_payload_fallback() {
    let temp = TempIndex::new();
    let sizes = [20, 70 * 1024, 20, 70 * 1024, 33 * 1024, 33 * 1024];
    let docs: Vec<_> = sizes
        .into_iter()
        .enumerate()
        .map(|(i, n)| doc(i, "x".repeat(n)))
        .collect();
    let index = temp.build(&docs, false);
    let mut scratch = vec![];
    let mut output = vec![];
    let stats = index
        .read_selected_batch(
            &[(0, 0), (2, 1), (3, 2), (4, 3), (5, 4)],
            &mut scratch,
            &mut output,
        )
        .unwrap();
    assert_eq!(stats.requests, 5);
    assert_eq!(stats.oversized_documents, 1);
    assert_eq!(
        stats.requested_bytes,
        [0, 2, 3, 4, 5]
            .into_iter()
            .map(|i| index.records[i].len as u64)
            .sum::<u64>()
    );
    assert!(scratch.capacity() <= READ_WINDOW);
    assert_eq!(
        output.into_iter().map(|(_, d)| d).collect::<Vec<_>>(),
        [0, 2, 3, 4, 5]
            .into_iter()
            .map(|i| docs[i].clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn invalid_slots_tombstone_short_read_and_empty_batch() {
    let temp = TempIndex::new();
    let docs = [doc(0, "hello".into()), doc(1, "world".into())];
    let index = temp.build(&docs, true);
    let mut scratch = vec![];
    let mut output = vec![];
    assert_eq!(
        index
            .read_selected_batch(&[], &mut scratch, &mut output)
            .unwrap()
            .requests,
        0
    );
    assert_eq!(scratch.capacity(), 0);
    for slots in [
        vec![(99, 0)],
        vec![(0, 0), (0, 1)],
        vec![(1, 0), (0, 1)],
        vec![(2, 0)],
    ] {
        assert!(matches!(
            index.read_selected_batch(&slots, &mut scratch, &mut output),
            Err(Error::Corrupt(_))
        ));
        assert!(output.is_empty());
    }
    OpenOptions::new()
        .write(true)
        .open(&temp.0)
        .unwrap()
        .set_len(0)
        .unwrap();
    assert!(matches!(
        index.read_selected_batch(&[(0, 0)], &mut scratch, &mut output),
        Err(Error::Corrupt(_))
    ));
}

#[test]
fn valid_crc_still_requires_matching_key_and_version() {
    for change_key in [false, true] {
        let temp = TempIndex::new();
        let expected = doc(0, "hello".into());
        let mut index = temp.build(std::slice::from_ref(&expected), false);
        let mut wrong = expected.clone();
        if change_key {
            wrong.document.key.id = "00000001".into();
        } else {
            wrong.version = 2;
        }
        let bytes = crate::payload::encode(&wrong).unwrap();
        assert_eq!(bytes.len(), index.records[0].len as usize);
        index.records[0].checksum = crc(&bytes);
        temp.replace(index.records[0].offset, &bytes);
        assert!(matches!(
            index.read_selected_batch(&[(0, 0)], &mut vec![], &mut vec![]),
            Err(Error::Corrupt(_))
        ));
    }
}
