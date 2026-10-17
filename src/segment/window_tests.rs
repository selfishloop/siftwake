use super::*;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "fdb-window-{}-{}.idx",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        for path in [
            &self.0,
            &self.0.with_extension("vfy"),
            &self.0.with_extension("records04.idx"),
        ] {
            let _ = std::fs::remove_file(path);
        }
    }
}
fn build(temp: &Temp) -> DiskIndex {
    let mut builder = Builder::new(&temp.0, 7, 40, 0).unwrap();
    // Small bounded unit fixture; no benchmark corpus/database is created.
    for id in 0..64 {
        let text = format!("{} İstanbul 東京 🦀 needle {}", "x".repeat(8192), id);
        builder
            .add(&StoredDocument {
                document: crate::Document {
                    key: Key::new("shared", "texts", &format!("{id:04}")),
                    title: "başlık".into(),
                    text,
                    fields: Default::default(),
                    links: vec![],
                },
                version: 7,
            })
            .unwrap();
    }
    builder.finish().unwrap()
}
fn matches(index: &DiskIndex, scratch: &mut QueryTextScratch, id: u32) -> Result<bool> {
    index.binary_matches(
        id,
        scratch,
        &["needle".into(), "istanbul".into()],
        &mut BTreeMap::new(),
        &mut 0,
    )
}

#[test]
fn sparse_window_stops_read_ahead_but_dense_window_keeps_it() {
    let temp = Temp::new();
    let index = build(&temp);
    let mut sparse = QueryTextScratch {
        read_window_enabled: true,
        ..Default::default()
    };
    for id in [0, 1, 2, 3, 40, 41, 63] {
        assert!(matches(&index, &mut sparse, id).unwrap());
    }
    assert!(!sparse.read_window_enabled);
    let mut dense = QueryTextScratch {
        read_window_enabled: true,
        ..Default::default()
    };
    for id in 0..64 {
        assert!(matches(&index, &mut dense, id).unwrap());
    }
    assert!(dense.read_window_enabled);
}
#[test]
fn window_matches_exact_reads_across_boundaries_and_sparse_order() {
    let temp = Temp::new();
    let index = build(&temp);
    let mut window = QueryTextScratch {
        read_window_enabled: true,
        ..Default::default()
    };
    let mut exact = QueryTextScratch::default();
    for id in (0..64).chain([0, 63, 10, 40, 40, 11]) {
        assert_eq!(
            matches(&index, &mut window, id).unwrap(),
            matches(&index, &mut exact, id).unwrap()
        );
        assert!(window.payload.capacity() <= 256 * 1024);
    }
    let mut query = Query::new("shared");
    query.text = "needle istanbul".into();
    assert_eq!(
        index.matches(&query, |_| false).unwrap(),
        (0..64).collect::<Vec<_>>()
    );
    let mask = [0u64; 1];
    assert_eq!(
        index.prefix_matches(&query, &mask, 5).unwrap(),
        (64, (0..5).collect())
    );
}
#[test]
fn prefetched_corrupt_record_is_checked_before_it_matches() {
    let temp = Temp::new();
    let index = build(&temp);
    let target = index.metadata(4).unwrap();
    let mut file = OpenOptions::new().write(true).open(&temp.0).unwrap();
    file.seek(SeekFrom::Start(target.offset + 80)).unwrap();
    file.write_all(&[0xff]).unwrap();
    file.flush().unwrap();
    let mut scratch = QueryTextScratch {
        read_window_enabled: true,
        ..Default::default()
    };
    for id in 0..4 {
        assert!(matches(&index, &mut scratch, id).unwrap());
    }
    assert!(matches(&index, &mut scratch, 4).is_err());
    #[cfg(feature = "bounded-engine")]
    assert!(
        index
            .binary_matches(
                4,
                &mut scratch,
                &["zx".into()],
                &mut BTreeMap::new(),
                &mut 0
            )
            .is_err()
    );
}

#[cfg(feature = "bounded-engine")]
#[test]
fn raw_ascii_matches_and_absence_reject_payload_corruption_and_truncation() {
    for read_window_enabled in [false, true] {
        let temp = Temp::new();
        let index = build(&temp);
        let mut scratch = QueryTextScratch {
            read_window_enabled,
            ..Default::default()
        };
        for needle in ["needle", "ed", "zx"] {
            assert_eq!(
                index
                    .binary_matches(
                        0,
                        &mut scratch,
                        &[needle.into()],
                        &mut BTreeMap::new(),
                        &mut 0
                    )
                    .unwrap(),
                needle != "zx"
            );
        }
        // Corrupt bytes outside the matching substring. The raw positive proof
        // must never hide a bad record, even with a valid needle still present.
        let target = index.metadata(40).unwrap();
        let mut file = OpenOptions::new().write(true).open(&temp.0).unwrap();
        file.seek(SeekFrom::Start(target.offset + target.len as u64 - 1))
            .unwrap();
        file.write_all(&[0xff]).unwrap();
        file.flush().unwrap();
        for needle in ["needle", "ed", "zx"] {
            assert!(
                index
                    .binary_matches(
                        40,
                        &mut scratch,
                        &[needle.into()],
                        &mut BTreeMap::new(),
                        &mut 0
                    )
                    .is_err()
            );
        }
        file.set_len(target.offset + 70).unwrap();
        let mut scratch = QueryTextScratch {
            read_window_enabled,
            ..Default::default()
        };
        for needle in ["needle", "ed", "zx"] {
            assert!(
                index
                    .binary_matches(
                        40,
                        &mut scratch,
                        &[needle.into()],
                        &mut BTreeMap::new(),
                        &mut 0
                    )
                    .is_err()
            );
        }
    }
}

#[test]
fn large_document_keeps_exact_read_and_record_validation() {
    let temp = Temp::new();
    let mut builder = Builder::new(&temp.0, 7, 40, 0).unwrap();
    builder
        .add(&StoredDocument {
            document: crate::Document {
                key: Key::new("shared", "texts", "large"),
                title: "title".into(),
                text: format!("needle İstanbul{}", "x".repeat(300 * 1024)),
                fields: Default::default(),
                links: vec![],
            },
            version: 7,
        })
        .unwrap();
    let index = builder.finish().unwrap();
    let mut scratch = QueryTextScratch {
        read_window_enabled: true,
        ..Default::default()
    };
    assert!(matches(&index, &mut scratch, 0).unwrap());
    assert_eq!(scratch.read_len, index.metadata(0).unwrap().len as usize);
    assert!(scratch.read_len > 256 * 1024);
}
#[test]
fn read_window_never_uses_a_valid_vfy_to_hide_corrupt_index_blocks_or_cuts() {
    let temp = Temp::new();
    let index = build(&temp);
    index.prepare_open_checks(&temp.0).unwrap();
    drop(index);
    let index = DiskIndex::open_deferred(&temp.0, Some((40, 0))).unwrap();
    let target = index.metadata(40).unwrap();
    let mut file = OpenOptions::new().write(true).open(&temp.0).unwrap();
    file.seek(SeekFrom::Start(target.offset + 80)).unwrap();
    file.write_all(&[0xff]).unwrap();
    file.flush().unwrap();
    let mut scratch = QueryTextScratch {
        read_window_enabled: true,
        ..Default::default()
    };
    for id in 0..2 {
        assert!(matches(&index, &mut scratch, id).unwrap());
    }
    assert!(matches(&index, &mut scratch, 2).is_err());
    file.set_len(target.offset).unwrap();
    let mut exact = QueryTextScratch::default();
    assert!(matches(&index, &mut exact, 60).is_err());
}

#[cfg(feature = "bounded-engine")]
#[test]
fn covering_needs_source_authorization_and_rejects_modified_unselected_payload() {
    let temp = Temp::new();
    let mut index = build(&temp);
    let terms = vec!["nee".to_owned(), "istanbul".to_owned()];
    let mut query = Query::new("shared");
    assert_eq!(
        index.verification_terms(&query, &terms).unwrap().as_ref(),
        terms
    );
    // Stand-in source for this isolated index test; engine tests bind real .fdb.
    index
        .bind_text_source(temp.0.clone(), SourceLease::open(&temp.0, false).unwrap())
        .unwrap();
    index.authorize_text_covering();
    assert_eq!(
        index.verification_terms(&query, &terms).unwrap().as_ref(),
        ["istanbul"]
    );
    assert_eq!(
        index
            .verification_terms(&query, &["he".to_owned()])
            .unwrap()
            .as_ref(),
        ["he"]
    );
    assert!(
        index
            .verification_terms(&query, &["ne".into()])
            .unwrap()
            .is_empty()
    );
    query.scope = "other".into();
    assert_eq!(
        index
            .verification_terms(&query, &["ne".into()])
            .unwrap()
            .as_ref(),
        ["ne"]
    );
    query.scope = "shared".into();
    query.text = "nee".into();
    assert_eq!(
        index.prefix_matches(&query, &[0], 1).unwrap(),
        (64, vec![0])
    );
    let target = index.metadata(63).unwrap();
    let mut file = OpenOptions::new().write(true).open(&temp.0).unwrap();
    file.seek(SeekFrom::Start(target.offset + 80)).unwrap();
    file.write_all(&[0xff]).unwrap();
    file.flush().unwrap();
    drop(file);
    assert!(matches!(
        index.prefix_matches(&query, &[0], 1),
        Err(Error::Corrupt(_))
    ));
    query.text = "ne".into();
    assert!(matches!(
        index.prefix_matches(&query, &[0], 1),
        Err(Error::Corrupt(_))
    ));
}
