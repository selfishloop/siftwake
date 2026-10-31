//! Small streamed transactions retain checked WAL addresses, never bodies or postings.
use super::*;
/// Volatile, exact ASCII1..3 membership plus a conservative byte-gram filter.
/// No payload/folded body retention, no persisted codec.
pub(super) struct LightFilter {
    short: Box<[u64]>,
    grams: Box<[u64]>,
}
impl LightFilter {
    pub(super) fn new(doc: &crate::Document) -> Arc<Self> {
        let mut folded = String::new();
        crate::index::fold_into(&format!("{}\n{}", doc.title, doc.text), &mut folded);
        let mut out = Self {
            short: vec![0; 18304usize.div_ceil(64)].into_boxed_slice(),
            grams: vec![0; 128].into_boxed_slice(),
        };
        let bytes = folded.as_bytes();
        for (i, &c) in bytes.iter().enumerate() {
            if c.is_ascii_lowercase() {
                let n = usize::from(c - b'a');
                out.short[n / 64] |= 1u64 << (n % 64);
                if i >= 1 && bytes[i - 1].is_ascii_lowercase() {
                    let n = 26 + usize::from(bytes[i - 1] - b'a') * 26 + n;
                    out.short[n / 64] |= 1u64 << (n % 64);
                    if i >= 2 && bytes[i - 2].is_ascii_lowercase() {
                        let n = 26
                            + 676
                            + usize::from(bytes[i - 2] - b'a') * 676
                            + usize::from(bytes[i - 1] - b'a') * 26
                            + usize::from(c - b'a');
                        out.short[n / 64] |= 1u64 << (n % 64);
                    }
                }
            }
            if i >= 1 {
                let n = Self::hash(&bytes[i - 1..=i]);
                out.grams[n / 64] |= 1u64 << (n % 64);
            }
            if i >= 2 {
                let n = Self::hash(&bytes[i - 2..=i]);
                out.grams[n / 64] |= 1u64 << (n % 64);
            }
        }
        Arc::new(out)
    }
    fn short_id(bytes: &[u8]) -> Option<usize> {
        if bytes.is_empty() || bytes.len() > 3 || !bytes.iter().all(u8::is_ascii_lowercase) {
            return None;
        }
        let base = match bytes.len() {
            1 => 0,
            2 => 26,
            _ => 26 + 676,
        };
        Some(base + bytes.iter().fold(0, |n, b| n * 26 + usize::from(b - b'a')))
    }
    fn hash(bytes: &[u8]) -> usize {
        bytes.iter().fold(bytes.len() as u32, |h, b| {
            h.wrapping_mul(16777619) ^ u32::from(*b)
        }) as usize
            % 8192
    }
    fn exact(&self, term: &str) -> Option<bool> {
        Self::short_id(term.as_bytes()).map(|id| self.short[id / 64] & (1u64 << (id % 64)) != 0)
    }
    fn might(&self, term: &str) -> bool {
        if let Some(exact) = self.exact(term) {
            return exact;
        }
        if term.len() < 2 {
            return true;
        }
        term.as_bytes().windows(term.len().min(3)).all(|g| {
            // Long ASCII words also benefit from exact trigram membership;
            // the Bloom fallback needlessly admitted absent rare trigrams.
            if let Some(id) = Self::short_id(g) {
                return self.short[id / 64] & (1u64 << (id % 64)) != 0;
            }
            let id = Self::hash(g);
            self.grams[id / 64] & (1u64 << (id % 64)) != 0
        })
    }
}
#[derive(Clone)]
pub(super) struct LightRow {
    pub(super) wal: crate::wal_delta::Row,
    pub(super) filter: Arc<LightFilter>,
}
impl std::ops::Deref for LightRow {
    type Target = crate::wal_delta::Row;
    fn deref(&self) -> &Self::Target {
        &self.wal
    }
}
#[derive(Debug, Default)]
pub struct ChunkedCommit {
    /// Always false. Earlier chunks survive a later input/commit failure.
    pub atomic: bool,
    pub sequences: Vec<u64>,
    /// Ordered acknowledged chunk lengths, paired with sequences.
    pub chunk_sizes: Vec<usize>,
    pub mutations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reclaim::tests::{Temp, doc};
    fn put(id: usize) -> Mutation {
        let mut document = doc(&format!("{id:05}"));
        document.text = "IŞIK ŞARTNAME東京🦀".into();
        document.fields.insert("n".into(), serde_json::json!(id));
        Mutation::Put {
            document,
            guard: Guard::Any,
        }
    }
    #[test]
    fn small_wal_writes_require_explicit_opt_in_and_can_be_disabled_safely() {
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        db.commit_streamed([Ok(put(0))]).unwrap();
        assert_eq!(db.stats().unwrap().delta_keys, 0);
        db.set_streamed_wal_layer(true).unwrap();
        db.commit_streamed([Ok(put(1))]).unwrap();
        assert_eq!(db.light.len(), 1);
        db.set_streamed_wal_layer(false).unwrap();
        assert!(db.get(&doc("00001").key).unwrap().is_some());
        db.commit_streamed([Ok(put(2))]).unwrap();
        assert!(db.light.is_empty());
        assert_eq!(db.stats().unwrap().documents, 3);
    }
    #[test]
    fn volatile_filter_never_rejects_a_real_substring_and_short_ascii_is_exact() {
        for text in [
            "IŞIK ŞARTNAME İstanbul東京🦀 ababa",
            "the man the zx",
            "Şİ abcéxyz\n東京",
            "",
        ] {
            let mut document = doc("filter");
            document.title = "İST".into();
            document.text = text.into();
            let filter = LightFilter::new(&document);
            let folded = crate::index::fold(&format!("{}\n{}", document.title, document.text));
            let boundaries: Vec<_> = folded
                .char_indices()
                .map(|(i, _)| i)
                .chain([folded.len()])
                .collect();
            for (n, &start) in boundaries.iter().enumerate() {
                for &end in boundaries.iter().skip(n + 1).take(8) {
                    assert!(filter.might(&folded[start..end]), "{}", &folded[start..end]);
                }
            }
            for n in 0..17576 {
                let bytes = [
                    b'a' + (n / 676) as u8,
                    b'a' + ((n / 26) % 26) as u8,
                    b'a' + (n % 26) as u8,
                ];
                let term = std::str::from_utf8(&bytes).unwrap();
                assert_eq!(filter.exact(term), Some(folded.contains(term)));
            }
        }
    }
    #[test]
    fn wal_layer_survives_reopen_flush_and_owned_overwrite_without_payload_retention() {
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        db.set_streamed_wal_layer(true).unwrap();
        let reader = db.snapshot_reader().unwrap();
        db.commit_streamed((0..20).map(|id| Ok(put(id)))).unwrap();
        assert_eq!(db.stats().unwrap().resident_text_bytes, 0);
        assert_eq!(db.stats().unwrap().resident_documents, 0);
        assert_eq!(db.light.len(), 20);
        let old = reader.acquire().unwrap();
        let mut q = Query::new("shared");
        q.text = "isik sartname".into();
        assert_eq!(old.search(&q).unwrap().total, 20);
        q.exact.insert("n".into(), serde_json::json!(7));
        q.ranges.push(crate::Range {
            field: "n".into(),
            min: 7,
            max: 7,
        });
        assert_eq!(old.search(&q).unwrap().total, 1);
        db.commit(vec![Mutation::Delete {
            key: doc("00007").key,
            guard: Guard::Any,
        }])
        .unwrap();
        assert_eq!(db.search(&q).unwrap().total, 0);
        assert_eq!(old.search(&q).unwrap().total, 1);
        db.flush().unwrap();
        drop(old);
        drop(reader);
        drop(db);
        let db = Database::open(&t.0).unwrap();
        q.exact.clear();
        q.ranges.clear();
        assert_eq!(db.search(&q).unwrap().total, 19);
    }
    #[test]
    fn chunks_report_partial_success_and_do_not_claim_atomicity() {
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        db.set_streamed_wal_layer(true).unwrap();
        let result = db
            .commit_streamed_chunks((0..80).map(|id| {
                if id == 65 {
                    Err(Error::Invalid("input interrupted".into()))
                } else {
                    Ok(put(id))
                }
            }))
            .unwrap_err();
        assert!(!result.committed.atomic);
        assert_eq!(result.committed.mutations, 64);
        assert_eq!(result.committed.sequences.len(), 2);
        assert_eq!(db.stats().unwrap().documents, 64);
        assert!(db.get(&doc("00064").key).unwrap().is_none());
        let done = db
            .commit_streamed_chunks((64..80).map(|id| Ok(put(id))))
            .unwrap();
        assert!(!done.atomic);
        assert_eq!(done.mutations, 16);
        drop(db);
        let db = Database::open(&t.0).unwrap();
        assert_eq!(db.stats().unwrap().documents, 80);
        assert_eq!(db.stats().unwrap().resident_text_bytes, 0);
    }
    #[cfg(feature = "experimental-write-maintenance-slots")]
    #[test]
    fn small_commit_does_not_drain_blocked_normal_worker() {
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        db.set_streamed_wal_layer(true).unwrap();
        for _ in 0..2 {
            db.commit(vec![put(0)]).unwrap();
            db.flush().unwrap();
        }
        let block = test_jobs::pause(&t.0);
        assert!(db.start_compaction().unwrap());
        block.wait_entered();
        let (tx, rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            let result = db.commit_streamed([Ok(put(1))]);
            tx.send(result).unwrap();
            db
        });
        let result = rx.recv_timeout(std::time::Duration::from_secs(3));
        drop(block);
        let mut db = writer.join().unwrap();
        result.unwrap().unwrap();
        assert_eq!(db.light.len(), 1);
        db.wait_for_compaction().unwrap();
        assert!(db.get(&doc("00001").key).unwrap().is_some());
    }
    #[test]
    #[ignore = "process helper for the small WAL commit boundary"]
    fn process_child() {
        let Some(path) = std::env::var_os("FDB_LIGHT_CHILD") else {
            return;
        };
        let mode = std::env::var("FDB_LIGHT_MODE").unwrap();
        let mut db = Database::open(path).unwrap();
        db.set_streamed_wal_layer(true).unwrap();
        let reader = db.snapshot_reader().unwrap();
        let old = reader.acquire().unwrap();
        db.commit_streamed((0..2).map(|id| {
            if id == 1 && mode == "intake" {
                std::fs::write(std::env::var_os("FDB_LIGHT_MARKER").unwrap(), b"intake").unwrap();
                loop {
                    std::thread::park();
                }
            }
            Ok(put(id))
        }))
        .unwrap();
        assert_eq!(old.document_count(), 0);
        assert_eq!(reader.acquire().unwrap().document_count(), 2);
        std::fs::write(
            std::env::var_os("FDB_LIGHT_MARKER").unwrap(),
            b"acknowledged",
        )
        .unwrap();
        loop {
            std::thread::park();
        }
    }
    #[test]
    fn process_kill_before_append_after_sync_and_after_snapshot_ack() {
        for mode in ["intake", "sync", "ack"] {
            let t = Temp::new();
            let marker = t.0.join("marker");
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--ignored",
                    "--exact",
                    "engine::light::tests::process_child",
                    "--nocapture",
                ])
                .env("FDB_LIGHT_CHILD", &t.0)
                .env("FDB_LIGHT_MODE", mode)
                .env("FDB_LIGHT_MARKER", &marker)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped());
            if mode == "sync" {
                command.env("FDB_LIGHT_AFTER_SYNC", &marker);
            }
            let mut child = command.spawn().unwrap();
            let began = std::time::Instant::now();
            while !marker.exists()
                && began.elapsed() < std::time::Duration::from_secs(10)
                && child.try_wait().unwrap().is_none()
            {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let observed = marker.exists();
            if child.try_wait().unwrap().is_none() {
                child.kill().unwrap();
            }
            let output = child.wait_with_output().unwrap();
            assert!(
                observed,
                "{mode}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let db = Database::open(&t.0).unwrap();
            let expected = if mode == "intake" { 0 } else { 2 };
            assert_eq!(db.stats().unwrap().documents, expected);
            let mut q = Query::new("shared");
            q.text = "isik sartname".into();
            assert_eq!(db.search(&q).unwrap().total, expected);
            for id in 0..2 {
                assert_eq!(
                    db.get(&doc(&format!("{id:05}")).key).unwrap().is_some(),
                    expected != 0
                );
            }
        }
    }
}
#[derive(Debug)]
pub struct ChunkedCommitError {
    pub committed: Box<ChunkedCommit>,
    pub error: Error,
}
impl std::fmt::Display for ChunkedCommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "non-atomic chunks: {} mutations committed before {}",
            self.committed.mutations, self.error
        )
    }
}
impl std::error::Error for ChunkedCommitError {}
impl Database {
    /// Opt-in small synced-WAL commits. Disabled by default while the measured
    /// threshold-flush visibility and concurrent-search gates remain open.
    /// Disabling affects future commits; pending WAL rows stay readable.
    pub fn set_streamed_wal_layer(&mut self, enabled: bool) -> Result<()> {
        self.healthy()?;
        if !enabled {
            self.light_flush_ready(true)?;
        }
        self.streamed_wal_layer = enabled;
        Ok(())
    }
    /// Non-atomic convenience: <=32 operations and <=512KiB encoded entries
    /// per chunk. Reports every acknowledged sequence on any later failure.
    /// Duplicate keys in different chunks are separate, sequential mutations.
    pub fn commit_streamed_chunks(
        &mut self,
        input: impl IntoIterator<Item = Result<Mutation>>,
    ) -> std::result::Result<ChunkedCommit, ChunkedCommitError> {
        let mut report = ChunkedCommit::default();
        let mut pending = Vec::new();
        let mut bytes = 0usize;
        let commit =
            |db: &mut Self, chunk: &mut Vec<Mutation>, report: &mut ChunkedCommit| -> Result<()> {
                if chunk.is_empty() {
                    return Ok(());
                }
                let count = chunk.len();
                let seq = db.commit_streamed(std::mem::take(chunk).into_iter().map(Ok))?;
                report.sequences.push(seq);
                report.chunk_sizes.push(count);
                report.mutations += count;
                Ok(())
            };
        let work = (|| -> Result<()> {
            for mutation in input {
                let mutation = mutation?;
                let len = match &mutation {
                    Mutation::Put { document, .. } => crate::payload::encoded_len(document)? + 5,
                    Mutation::Delete { key, .. } => {
                        17 + key.scope.len() + key.collection.len() + key.id.len()
                    }
                };
                if !pending.is_empty() && (pending.len() == 32 || bytes + len > 512 * 1024) {
                    commit(self, &mut pending, &mut report)?;
                    bytes = 0;
                }
                pending.push(mutation);
                bytes += len;
            }
            commit(self, &mut pending, &mut report)
        })();
        match work {
            Ok(()) => Ok(report),
            Err(error) => Err(ChunkedCommitError {
                committed: Box::new(report),
                error,
            }),
        }
    }
    pub(crate) fn light_matches(&self, query: &Query) -> Result<Vec<Arc<Key>>> {
        self.light_matches_prefix(query, None)
    }
    // BTreeMap visits full public keys in order. Only first-page callers cap;
    // the complete shadow set and exact query path remain independent.
    pub(crate) fn light_matches_prefix(
        &self,
        query: &Query,
        cap: Option<usize>,
    ) -> Result<Vec<Arc<Key>>> {
        let terms: Vec<_> = query
            .text
            .split_whitespace()
            .map(crate::index::fold)
            .collect();
        let mut out = Vec::new();
        let mut folded = String::new();
        for (key, row) in &self.light {
            if key.scope != query.scope
                || query
                    .collection
                    .as_ref()
                    .is_some_and(|c| c != &key.collection)
            {
                continue;
            }
            if terms.iter().any(|term| !row.filter.might(term)) {
                continue;
            }
            // Exact volatile membership was built from this synced immutable
            // frame's input. Like the owned RAM index, it needs no payload read
            // for a count. Any selected payload still passes Row::read CRC/ID.
            if query.exact.is_empty()
                && query.ranges.is_empty()
                && query.linked_to.is_none()
                && terms
                    .iter()
                    .all(|term| row.filter.exact(term) == Some(true))
            {
                out.push(key.clone());
                if cap.is_some_and(|n| out.len() == n) {
                    break;
                }
                continue;
            }
            let stored = row.read(&self.log, key)?;
            let doc = &stored.document;
            if query
                .exact
                .iter()
                .any(|(field, value)| doc.fields.get(field) != Some(value))
                || query.ranges.iter().any(|range| {
                    doc.fields
                        .get(&range.field)
                        .and_then(|v| v.as_i64())
                        .is_none_or(|n| n < range.min || n > range.max)
                })
                || query
                    .linked_to
                    .as_ref()
                    .is_some_and(|k| !doc.links.contains(k))
            {
                continue;
            }
            crate::index::fold_into(&format!("{}\n{}", doc.title, doc.text), &mut folded);
            if terms.iter().all(|term| folded.contains(term)) {
                out.push(key.clone());
                if cap.is_some_and(|n| out.len() == n) {
                    break;
                }
            }
        }
        Ok(out)
    }
}
