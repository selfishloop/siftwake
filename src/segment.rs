//! Immutable, derived disk index. Full contents and posting lists stay on disk.
//! The transaction log is authoritative; missing/corrupt segments are rebuilt.
use crate::format::{crc, prefix_crc};
use crate::index::{fold, grams};
use crate::{Error, Key, Query, Result, StoredDocument};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "experimental-rkyv-payload")]
mod payload_bench;

pub(crate) mod block_check;
#[cfg(feature = "bounded-engine")]
mod checked_page;
mod checksum;
#[cfg(feature = "bounded-engine")]
mod compact;
#[cfg(feature = "bounded-engine")]
mod covering;
#[cfg(feature = "bounded-engine")]
mod short_ascii;
#[cfg(feature = "bounded-engine")]
pub(crate) use checked_page::CheckedPage;
#[cfg(feature = "bounded-engine")]
pub(crate) use covering::SourceLease;
#[cfg(feature = "bounded-engine")]
mod posting_spool;
#[cfg(feature = "experimental-streaming-open")]
mod term_pages;
#[cfg(feature = "bounded-engine")]
type PostingOutput = posting_spool::Postings;
#[cfg(not(feature = "bounded-engine"))]
type PostingOutput = Vec<(Term, Posting)>;
#[cfg(feature = "experimental-merge-postings")]
fn posting_output(stem: &Path) -> Result<PostingOutput> {
    #[cfg(feature = "bounded-engine")]
    {
        PostingOutput::new(stem)
    }
    #[cfg(not(feature = "bounded-engine"))]
    {
        let _ = stem;
        Ok(Vec::new())
    }
}
#[cfg(feature = "experimental-merge-postings")]
fn push_posting(output: &mut PostingOutput, term: Term, posting: Posting) -> Result<()> {
    #[cfg(feature = "bounded-engine")]
    {
        output.push(term, posting)
    }
    #[cfg(not(feature = "bounded-engine"))]
    {
        output.push((term, posting));
        Ok(())
    }
}

const MAGIC: &[u8; 8] = b"FDBSEG04";
const LEGACY_MAGIC: &[u8; 8] = b"FDBSEG03";
const FOOTER: &[u8; 8] = b"FDBEND04";
const LEGACY_FOOTER: &[u8; 8] = b"FDBEND03";
const HEADER_SIZE: u64 = 28;
const FOOTER_SIZE: u64 = 32;
const MAX_DIRECTORY: u64 = 256 * 1024 * 1024;
#[derive(Default)]
struct QueryTextScratch {
    payload: Vec<u8>,
    #[cfg(feature = "experimental-paged-metadata")]
    record_cursor: paged::TextCursor,
    #[cfg(feature = "experimental-grouped-read")]
    read_window_enabled: bool,
    #[cfg(feature = "experimental-grouped-read")]
    read_base: u64,
    #[cfg(feature = "experimental-grouped-read")]
    read_len: usize,
    #[cfg(feature = "experimental-grouped-read")]
    window_used: usize,
    #[cfg(feature = "experimental-grouped-read")]
    window_prefetched: bool,
    #[cfg(feature = "experimental-grouped-read")]
    read_limit: Option<u64>,
    #[cfg(feature = "experimental-grouped-read")]
    previous_end: Option<u64>,
    #[cfg(feature = "experimental-grouped-read")]
    adjacent: u8,
    #[cfg(feature = "diagnostic-timing")]
    reads: crate::timing::PayloadReads,
    title: String,
    text: String,
    window: String,
    found: Vec<bool>,
    #[cfg(feature = "bounded-engine")]
    raw_probes: u8,
    #[cfg(feature = "bounded-engine")]
    raw_hits: u8,
}
impl QueryTextScratch {
    fn matches(
        &mut self,
        document: &crate::Document,
        terms: &[String],
        cache: &mut BTreeMap<String, String>,
        cached_bytes: &mut usize,
    ) -> bool {
        self.matches_text(&document.title, &document.text, terms, cache, cached_bytes)
    }
    fn matches_text(
        &mut self,
        title: &str,
        text: &str,
        terms: &[String],
        cache: &mut BTreeMap<String, String>,
        cached_bytes: &mut usize,
    ) -> bool {
        #[cfg(feature = "diagnostic-timing")]
        let _verify = crate::timing::span("query.verify_text");
        // Keep at most 128 KiB per scratch buffer between candidates. Large
        // documents may need larger temporary outputs, released on the next use.
        for buffer in [&mut self.title, &mut self.text] {
            if buffer.capacity() > 128 * 1024 {
                *buffer = String::new();
            }
        }
        {
            #[cfg(feature = "diagnostic-timing")]
            let _fold = crate::timing::span("query.fold.title");
            crate::index::fold_into(title, &mut self.title);
        }
        if terms.iter().all(|term| self.title.contains(term)) {
            return true;
        }
        #[cfg(feature = "bounded-engine")]
        if terms.iter().all(|term| short_ascii::eligible(term)) {
            #[cfg(feature = "diagnostic-timing")]
            let _scan = crate::timing::span("query.scan.raw_short");
            return terms
                .iter()
                .all(|term| self.title.contains(term) || short_ascii::contains(text, term));
        }
        #[cfg(feature = "bounded-engine")]
        if self.raw_probes < 8
            && text.len() > 1024
            && terms.iter().all(|term| {
                self.title.contains(term)
                    || (term.len() >= 3
                        && term
                            .bytes()
                            .all(|b| b.is_ascii() && !b.is_ascii_uppercase()))
            })
        {
            // These ASCII spellings fold exactly to the lowercase query bytes.
            // This proves only positive matches; absence still takes the normal
            // Unicode/Turkish verifier. Short terms avoid a second broad scan.
            // The caller has already checked payload CRC, schema and identity.
            let proven = {
                #[cfg(feature = "diagnostic-timing")]
                let _probe = crate::timing::span("query.raw_ascii_probe");
                terms
                    .iter()
                    .all(|term| self.title.contains(term) || ascii_case_proven(text, term))
            };
            // Speculation pays only when it proves most candidates. A low
            // success rate stops extra scans for the remainder of this run's
            // query; later changes in spelling/precision also stop it.
            self.raw_probes += 1;
            self.raw_hits += u8::from(proven);
            if self.raw_probes == 8 && self.raw_hits >= 6 {
                self.raw_probes = 0;
                self.raw_hits = 0;
            }
            if proven {
                #[cfg(feature = "diagnostic-timing")]
                let _proof = crate::timing::span("query.raw_ascii_proven");
                return true;
            }
        }
        if let Some(text) = cache.get(text) {
            return terms
                .iter()
                .all(|term| self.title.contains(term) || text.contains(term));
        }
        if text.len() > 1024 {
            return self.matches_chunks(text, terms);
        }
        {
            #[cfg(feature = "diagnostic-timing")]
            let _fold = crate::timing::span("query.fold.body");
            crate::index::fold_into(text, &mut self.text);
        }
        if text.len() + self.text.len() <= 256 * 1024 - *cached_bytes {
            *cached_bytes += text.len() + self.text.len();
            cache.insert(text.to_owned(), self.text.clone());
        }
        terms
            .iter()
            .all(|term| self.title.contains(term) || self.text.contains(term))
    }

    fn matches_chunks(&mut self, text: &str, terms: &[String]) -> bool {
        self.found.clear();
        self.found
            .extend(terms.iter().map(|term| self.title.contains(term)));
        self.window.clear();
        let overlap = terms
            .iter()
            .map(String::len)
            .max()
            .unwrap_or(0)
            .saturating_sub(1);
        let mut first = 0;
        while first < text.len() {
            // Preserve the small first probe for early matches. Once it fails,
            // amortize folding/window maintenance over larger bounded blocks.
            let step = if cfg!(feature = "bounded-engine") && first != 0 {
                4096
            } else {
                1024
            };
            let mut end = (first + step).min(text.len());
            while !text.is_char_boundary(end) {
                end += 1;
            }
            {
                #[cfg(feature = "diagnostic-timing")]
                let _fold = crate::timing::span("query.fold.body");
                crate::index::fold_into(&text[first..end], &mut self.text);
            }
            self.window.push_str(&self.text);
            {
                #[cfg(feature = "diagnostic-timing")]
                let _scan = crate::timing::span("query.scan.body");
                for (found, term) in self.found.iter_mut().zip(terms) {
                    if !*found {
                        *found = self.window.contains(term);
                    }
                }
            }
            if self.found.iter().all(|found| *found) {
                return true;
            }
            // Preserve enough normalized bytes to detect a term spanning blocks.
            // Round backwards to a UTF-8 boundary (Unicode lowercasing can expand).
            let mut discard = self.window.len().saturating_sub(overlap);
            while !self.window.is_char_boundary(discard) {
                discard -= 1;
            }
            self.window.drain(..discard);
            first = end;
        }
        false
    }
}
// Positive-only, allocation-free ASCII spellings. General mixed-case and
// Unicode spellings still use the canonical fold verifier. Cap only the
// speculative stack buffer, never the accepted query/document size.
#[cfg(feature = "bounded-engine")]
fn ascii_case_proven(text: &str, term: &str) -> bool {
    let mut end = text.len().min(64 * 1024);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let text = &text[..end];
    if text.contains(term) {
        return true;
    }
    if term.len() > 128 || term.is_empty() {
        return false;
    }
    let mut spelling = [0u8; 128];
    let spelling = &mut spelling[..term.len()];
    spelling.copy_from_slice(term.as_bytes());
    spelling[0].make_ascii_uppercase();
    if text.contains(std::str::from_utf8(spelling).unwrap()) {
        return true;
    }
    spelling.make_ascii_uppercase();
    text.contains(std::str::from_utf8(spelling).unwrap())
}
#[cfg(test)]
mod query_scratch_tests {
    use super::*;
    #[cfg(feature = "bounded-engine")]
    #[test]
    fn speculative_scan_budget_never_limits_match_semantics() {
        let mut scratch = QueryTextScratch::default();
        let mut cache = BTreeMap::new();
        let mut bytes = 0;
        // A failed probe budget must still allow later positives and Unicode
        // matches through the normal verifier, including a cross-budget token.
        for _ in 0..16 {
            assert!(!scratch.matches_text(
                "",
                &"x".repeat(4096),
                &["absent".into()],
                &mut cache,
                &mut bytes
            ));
        }
        for text in [
            format!("{}NeEdLe ŞİRKET", "x".repeat(65535)),
            format!("{}東京 Needle", "x".repeat(65535)),
            format!("{}Needle", "x".repeat(65533)),
        ] {
            for query in ["needle", "sirket", "東京 needle", "needle absent"] {
                let terms: Vec<_> = query.split_whitespace().map(fold).collect();
                let expected = terms.iter().all(|term| fold(&text).contains(term));
                assert_eq!(
                    scratch.matches_text("", &text, &terms, &mut cache, &mut bytes),
                    expected
                );
                let mut fresh = QueryTextScratch::default();
                assert_eq!(
                    fresh.matches_text("", &text, &terms, &mut cache, &mut bytes),
                    expected
                );
            }
        }
    }
    #[cfg(feature = "bounded-engine")]
    #[test]
    fn ascii_proof_and_unicode_fallback_match_folded_contains() {
        let mut scratch = QueryTextScratch::default();
        let mut cache = BTreeMap::new();
        let mut cached_bytes = 0;
        for first in [0, 1000, 1022, 1023, 1024, 2047] {
            for body in [
                "needle whale",
                "NEEDLE WHALE",
                "needLE WHale",
                "İĞNE ŞİRKET",
                "İneedleı WHALE",
                "東京🦀 needle\nwhale",
                "needlexxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxwhale",
                "missing",
            ] {
                let text = format!("{}{}{}", "x".repeat(first), body, "z".repeat(2200));
                for title in ["", "BAŞLIK NEEDLE", "ŞİRKET"] {
                    for query in [
                        "needle",
                        "needle whale",
                        "igne sirket",
                        "sirket whale",
                        "needle zx",
                        "whale needle whale",
                        "東京 needle",
                        "🦀 needle",
                        "ed",
                        "",
                        "i",
                        "xxxxx",
                        "needlex",
                        "needleı",
                    ] {
                        let terms: Vec<_> = query.split_whitespace().map(fold).collect();
                        let expected = terms
                            .iter()
                            .all(|term| fold(title).contains(term) || fold(&text).contains(term));
                        assert_eq!(
                            QueryTextScratch::default().matches_text(
                                title,
                                &text,
                                &terms,
                                &mut cache,
                                &mut cached_bytes
                            ),
                            expected,
                            "first={first} title={title:?} body={body:?} query={query:?}"
                        );
                    }
                }
            }
        }
        // A caller supplying an unnormalized uppercase needle cannot obtain a
        // false proof merely because the original body contains that spelling.
        assert!(!scratch.matches_text(
            "",
            &format!("{}UPPER", "x".repeat(2000)),
            &["UPPER".into()],
            &mut cache,
            &mut cached_bytes,
        ));
    }
    #[test]
    fn chunked_scan_matches_full_fold_at_utf8_and_term_boundaries() {
        let mut scratch = QueryTextScratch::default();
        for prefix in (1000..1040)
            .chain(4072..4120)
            .chain(5096..5144)
            .chain(8176..8216)
            .chain(9192..9240)
        {
            let body = format!(
                "{}İŞIK ȺΣ 🦀 QUİXOTE{}last",
                "x".repeat(prefix),
                "z".repeat(2200)
            );
            let title = "ŞARTNAME";
            for query in [
                "sartname isik",
                "ⱥσ 🦀",
                "quixote last",
                "isik missing",
                "last",
                "xisi",
                "🦀 quixote",
                "",
            ] {
                let terms: Vec<_> = query.split_whitespace().map(fold).collect();
                let expected = terms
                    .iter()
                    .all(|term| fold(title).contains(term) || fold(&body).contains(term));
                assert_eq!(
                    scratch.matches_text(title, &body, &terms, &mut BTreeMap::new(), &mut 0),
                    expected,
                    "prefix={prefix} query={query}"
                );
            }
        }
        let term = format!("{}a", "q".repeat(3000));
        for body in [format!("xx{term}zz"), format!("{}b", "q".repeat(3000))] {
            assert_eq!(
                scratch.matches_text(
                    "",
                    &body,
                    std::slice::from_ref(&term),
                    &mut BTreeMap::new(),
                    &mut 0
                ),
                body.contains(&term)
            );
        }
        assert!(scratch.window.capacity() <= 8192);
    }
    #[test]
    fn cached_body_never_reuses_title_and_oversize_text_does_not_escape_budget() {
        let mut scratch = QueryTextScratch::default();
        let mut cache = BTreeMap::new();
        let mut bytes = 0;
        let mut doc = crate::Document {
            key: Key::new("shared", "docs", "1"),
            title: "İLK".into(),
            text: "ŞİRKET 🦀".into(),
            fields: BTreeMap::new(),
            links: vec![],
        };
        let terms = vec!["ilk".into(), "sirket".into()];
        assert!(scratch.matches(&doc, &terms, &mut cache, &mut bytes));
        assert_eq!(cache.len(), 1);
        doc.title = "SON".into();
        assert!(!scratch.matches(&doc, &terms, &mut cache, &mut bytes));
        doc.text = "ABC".repeat(100000);
        assert!(scratch.matches(&doc, &["abc".into()], &mut cache, &mut bytes));
        assert_eq!(cache.len(), 1);
        assert!(bytes <= 256 * 1024);
        doc.text = "tiny".into();
        assert!(!scratch.matches(&doc, &["abc".into()], &mut cache, &mut bytes));
        assert!(scratch.text.capacity() <= 128 * 1024);
        assert!(scratch.matches(&doc, &[], &mut cache, &mut bytes));
    }
}
#[cfg(all(test, feature = "experimental-grouped-read"))]
mod grouped_tests;
#[cfg(feature = "experimental-paged-metadata")]
mod paged;
#[cfg(feature = "experimental-write-record-spool")]
mod record_spool;
#[cfg(feature = "experimental-merge-postings")]
mod reuse;
#[cfg(feature = "experimental-streaming-open")]
mod streaming;
#[cfg(all(test, feature = "experimental-grouped-read"))]
mod window_tests;
static TEMP: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "experimental-grouped-read")]
const READ_WINDOW: usize = 64 * 1024;
#[cfg(feature = "experimental-grouped-read")]
const READ_GAP: u64 = 4096;

#[cfg(feature = "experimental-grouped-read")]
#[derive(Default, Debug)]
pub(crate) struct BatchReadStats {
    pub requests: usize,
    pub requested_bytes: u64,
    pub oversized_documents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Term {
    Scope(String),
    Collection(String, String),
    Exact(String, String, String, String),
    Numeric(String, String, String, i64),
    Gram(String, String),
    Link(Key),
}

fn terms(stored: &StoredDocument, body: &crate::index::BodyTerms) -> Vec<Term> {
    let d = &stored.document;
    let k = &d.key;
    let mut result = vec![
        Term::Scope(k.scope.clone()),
        Term::Collection(k.scope.clone(), k.collection.clone()),
    ];
    for (field, value) in &d.fields {
        if value.is_array() || value.is_object() {
            continue;
        }
        result.push(Term::Exact(
            k.scope.clone(),
            k.collection.clone(),
            field.clone(),
            value.to_string(),
        ));
        if let Some(n) = value.as_i64() {
            result.push(Term::Numeric(
                k.scope.clone(),
                k.collection.clone(),
                field.clone(),
                n,
            ));
        }
    }
    let title = grams(&fold(&d.title));
    for gram in title.union(&body.grams) {
        result.push(Term::Gram(k.scope.clone(), gram.clone()));
    }
    for key in d.links.iter().collect::<BTreeSet<_>>() {
        result.push(Term::Link(key.clone()));
    }
    result
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    key: Key,
    offset: u64,
    len: u32,
    checksum: u32,
    version: u64,
    deleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Posting {
    offset: u64,
    count: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(any(not(feature = "experimental-write-record-spool"), test))]
struct Directory {
    records: Vec<Record>,
    postings: Vec<(Term, Posting)>,
}

pub(crate) struct Builder {
    #[cfg(feature = "bounded-engine")]
    source_references: bool,
    temporary: PathBuf,
    target: PathBuf,
    writer: Option<BufWriter<File>>,
    offset: u64,
    sequence: u64,
    #[cfg(not(feature = "experimental-write-record-spool"))]
    records: Vec<Record>,
    #[cfg(feature = "experimental-write-record-spool")]
    records: record_spool::Records,
    postings: crate::spill::Spills,
    body_cache: crate::index::BodyCache,
    // Leases are last: release reservations after owned buffers and spill files.
    #[cfg(feature = "experimental-write-buffer-budget")]
    _write_lease: crate::write_budget::Lease<'static>,
    #[cfg(feature = "bounded-engine")]
    _posting_lease: crate::write_budget::Lease<'static>,
    #[cfg(feature = "bounded-engine")]
    _working_lease: Option<crate::write_budget::Lease<'static>>,
}

impl Builder {
    pub fn new(
        target: &Path,
        sequence: u64,
        checkpoint_end: u64,
        checkpoint_crc: u32,
    ) -> Result<Self> {
        Self::new_layout(
            target,
            sequence,
            checkpoint_end,
            checkpoint_crc,
            #[cfg(feature = "bounded-engine")]
            false,
        )
    }

    fn new_layout(
        target: &Path,
        sequence: u64,
        checkpoint_end: u64,
        checkpoint_crc: u32,
        #[cfg(feature = "bounded-engine")] source_references: bool,
    ) -> Result<Self> {
        // Acquire posting quota before I/O quota: waiters must not hold all I/O
        // reservations while active builders need I/O to finish and release.
        #[cfg(feature = "bounded-engine")]
        let working_lease = crate::write_budget::builder_working()?;
        #[cfg(feature = "bounded-engine")]
        let posting_lease = crate::write_budget::reserve_postings()?;
        #[cfg(feature = "experimental-write-buffer-budget")]
        let write_lease = crate::write_budget::reserve(16 * 1024, 16 * 1024)?;
        let temporary = target.with_extension(format!(
            "build-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let mut writer = BufWriter::with_capacity(8 * 1024, file);
        #[cfg(feature = "bounded-engine")]
        let magic = if source_references {
            compact::MAGIC
        } else {
            MAGIC
        };
        #[cfg(not(feature = "bounded-engine"))]
        let magic = MAGIC;
        writer.write_all(magic)?;
        writer.write_all(&sequence.to_le_bytes())?;
        writer.write_all(&checkpoint_end.to_le_bytes())?;
        writer.write_all(&checkpoint_crc.to_le_bytes())?;
        Ok(Self {
            #[cfg(feature = "bounded-engine")]
            source_references,
            #[cfg(feature = "experimental-write-buffer-budget")]
            _write_lease: write_lease,
            #[cfg(feature = "bounded-engine")]
            postings: crate::spill::Spills::new(temporary.clone(), posting_lease.bytes),
            #[cfg(not(feature = "bounded-engine"))]
            postings: crate::spill::Spills::new(temporary.clone(), 4 * 1024 * 1024),
            #[cfg(feature = "bounded-engine")]
            _posting_lease: posting_lease,
            #[cfg(feature = "bounded-engine")]
            _working_lease: working_lease,
            #[cfg(feature = "experimental-write-record-spool")]
            records: record_spool::Records::new(&temporary)?,
            temporary,
            target: target.into(),
            writer: Some(writer),
            offset: HEADER_SIZE,
            sequence,
            #[cfg(not(feature = "experimental-write-record-spool"))]
            records: vec![],
            body_cache: crate::index::BodyCache::default(),
        })
    }

    fn record_count(&self) -> usize {
        #[cfg(feature = "experimental-write-record-spool")]
        {
            self.records.count
        }
        #[cfg(not(feature = "experimental-write-record-spool"))]
        {
            self.records.len()
        }
    }
    fn last_record_key(&self) -> Option<&Key> {
        #[cfg(feature = "experimental-write-record-spool")]
        {
            self.records.last.as_ref()
        }
        #[cfg(not(feature = "experimental-write-record-spool"))]
        {
            self.records.last().map(|r| &r.key)
        }
    }
    fn push_record(&mut self, record: Record) -> Result<()> {
        #[cfg(feature = "experimental-write-record-spool")]
        {
            self.records.push(record)
        }
        #[cfg(not(feature = "experimental-write-record-spool"))]
        {
            self.records.push(record);
            Ok(())
        }
    }
    pub fn add(&mut self, stored: &StoredDocument) -> Result<()> {
        self.add_document(stored, true)
    }

    #[cfg(all(
        feature = "experimental-merge-postings",
        any(not(feature = "bounded-engine"), test)
    ))]
    pub(crate) fn add_reused(&mut self, stored: &StoredDocument) -> Result<()> {
        self.add_document(stored, false)
    }

    fn add_document(&mut self, stored: &StoredDocument, index_terms: bool) -> Result<()> {
        #[cfg(feature = "bounded-engine")]
        if self.source_references {
            return Err(Error::Invalid(
                "source reference requires a validated entry".into(),
            ));
        }
        #[cfg(feature = "diagnostic-memory")]
        let _origin = crate::memory::Scope::new(6);
        #[cfg(feature = "diagnostic-timing")]
        let _add = crate::timing::span("builder.add");
        crate::validate_doc(&stored.document)?;
        if stored.version == 0 || stored.version > self.sequence {
            return Err(Error::Corrupt("invalid segment document version".into()));
        }
        if self
            .last_record_key()
            .is_some_and(|key| key >= &stored.document.key)
        {
            return Err(Error::Corrupt(
                "checkpoint keys must be strictly sorted".into(),
            ));
        }
        let id = u32::try_from(self.record_count())
            .map_err(|_| Error::Invalid("segment exceeds u32 record capacity".into()))?;
        let bytes = crate::payload::encode(stored)?;
        self.writer.as_mut().unwrap().write_all(&bytes)?;
        self.push_record(Record {
            key: stored.document.key.clone(),
            offset: self.offset,
            len: bytes.len() as u32,
            checksum: crc(&bytes),
            version: stored.version,
            deleted: false,
        })?;
        self.offset += bytes.len() as u64;
        if index_terms {
            self.index_document(stored, id)?;
        }
        Ok(())
    }

    fn index_document(&mut self, stored: &StoredDocument, id: u32) -> Result<()> {
        let extracted = {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("builder.extract_terms");
            let body = self.body_cache.get(&stored.document.text);
            terms(stored, &body)
        };
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("builder.insert_postings");
        for term in extracted {
            self.postings.add(term, id)?;
        }
        Ok(())
    }

    pub fn add_delete(&mut self, key: &Key) -> Result<()> {
        crate::validate_key(key)?;
        if self.last_record_key().is_some_and(|last| last >= key) {
            return Err(Error::Corrupt("unordered tombstone key".into()));
        }
        #[cfg(feature = "bounded-engine")]
        let offset = if self.source_references {
            0
        } else {
            self.offset
        };
        #[cfg(not(feature = "bounded-engine"))]
        let offset = self.offset;
        self.push_record(Record {
            key: key.clone(),
            offset,
            len: 0,
            checksum: 0,
            version: self.sequence,
            deleted: true,
        })?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<DiskIndex> {
        #[cfg(feature = "diagnostic-memory")]
        let _origin = crate::memory::Scope::new(6);
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("builder.finish");
        let writer = self.writer.as_mut().unwrap();
        #[cfg(not(feature = "bounded-engine"))]
        let postings: Vec<_> = self
            .postings
            .merge(writer, &mut self.offset)?
            .into_iter()
            .map(|(term, offset, count)| (term, Posting { offset, count }))
            .collect();
        #[cfg(feature = "bounded-engine")]
        let postings = {
            let mut postings = posting_output(&self.temporary)?;
            self.postings
                .merge_with(writer, &mut self.offset, |t, start, count| {
                    push_posting(
                        &mut postings,
                        t,
                        Posting {
                            offset: start,
                            count,
                        },
                    )
                })?;
            postings
        };
        self.finish_directory(postings)
    }

    #[cfg(feature = "experimental-merge-postings")]
    pub(crate) fn finish_reused(mut self, inputs: &[(&DiskIndex, &[u32])]) -> Result<DiskIndex> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("builder.finish_reused");
        #[cfg(not(feature = "bounded-engine"))]
        let postings = reuse::merge(inputs, self.writer.as_mut().unwrap(), &mut self.offset)?;
        #[cfg(feature = "bounded-engine")]
        let postings = {
            let mut postings = posting_output(&self.temporary)?;
            reuse::merge_with(
                inputs,
                self.writer.as_mut().unwrap(),
                &mut self.offset,
                |t, p| push_posting(&mut postings, t, p),
            )?;
            postings
        };
        self.finish_directory(postings)
    }

    #[cfg(feature = "experimental-merge-postings")]
    pub(crate) fn finish_delta(
        mut self,
        index: &crate::index::Index,
        records: &BTreeMap<Key, u32>,
    ) -> Result<DiskIndex> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("builder.finish_delta");
        if records.len() != self.record_count() {
            return Err(Error::Corrupt("delta checkpoint record count".into()));
        }
        let mut postings = posting_output(&self.temporary)?;
        let mut previous: Option<Term> = None;
        index.visit_terms(|term, posting| {
            let ids = index.disk_ids(posting, records)?;
            let count = u32::try_from(ids.len())
                .map_err(|_| Error::Invalid("delta posting too large".into()))?;
            if count == 0 {
                return Ok(());
            }
            if ids
                .last()
                .is_some_and(|id| *id as usize >= self.record_count())
            {
                return Err(Error::Corrupt("delta checkpoint posting bounds".into()));
            }
            if previous.as_ref().is_some_and(|previous| previous >= &term) {
                return Err(Error::Corrupt("delta checkpoint dictionary order".into()));
            }
            let start = self.offset;
            // Bounded I/O scratch; one term's remapped IDs, no global posting copy.
            for chunk in ids.chunks(2048) {
                let mut bytes = [0u8; 8192];
                for (slot, id) in chunk.iter().enumerate() {
                    bytes[slot * 4..slot * 4 + 4].copy_from_slice(&id.to_le_bytes());
                }
                let length = chunk.len() * 4;
                self.writer.as_mut().unwrap().write_all(&bytes[..length])?;
                self.offset += length as u64;
            }
            previous = Some(term.clone());
            push_posting(
                &mut postings,
                term,
                Posting {
                    offset: start,
                    count,
                },
            )?;
            Ok(())
        })?;
        self.finish_directory(postings)
    }

    fn finish_directory(mut self, postings: PostingOutput) -> Result<DiskIndex> {
        #[cfg(feature = "bounded-engine")]
        let mut postings = postings;
        let writer = self.writer.as_mut().unwrap();
        let directory_offset = self.offset;
        #[cfg(not(feature = "experimental-write-record-spool"))]
        let directory = Directory {
            records: std::mem::take(&mut self.records),
            postings,
        };
        let mut sink = crate::bounded::Sink::new(&mut *writer, MAX_DIRECTORY as usize);
        #[cfg(not(feature = "experimental-write-record-spool"))]
        let encoded = serde_json::to_writer(&mut sink, &directory).map_err(Error::from);
        #[cfg(feature = "experimental-write-record-spool")]
        let encoded: Result<()> = (|| {
            #[cfg(feature = "bounded-engine")]
            if self.source_references {
                compact::write_directory_header(
                    &mut sink,
                    true,
                    self.records.count,
                    postings.count(),
                )?;
                self.records.write_binary(&mut sink)?;
                postings.write_binary(&mut sink)?;
                return Ok(());
            }
            sink.write_all(b"{\"records\":")?;
            self.records.write_json(&mut sink)?;
            sink.write_all(b",\"postings\":")?;
            #[cfg(not(feature = "bounded-engine"))]
            serde_json::to_writer(&mut sink, &postings)?;
            #[cfg(feature = "bounded-engine")]
            postings.write_json(&mut sink)?;
            sink.write_all(b"}")?;
            Ok(())
        })();
        if sink.exceeded {
            return Err(Error::Invalid("segment directory exceeds 256 MiB".into()));
        }
        if let Some(error) = sink.failure.take() {
            return Err(Error::Io(error));
        }
        encoded?;
        let directory_len = sink.written as u64;
        #[cfg(not(feature = "experimental-write-record-spool"))]
        drop(directory);
        #[cfg(feature = "experimental-write-record-spool")]
        drop(postings);
        writer.flush()?;
        let end = directory_offset + directory_len;
        let checksum = prefix_crc(writer.get_mut(), end)?;
        // prefix_crc changed the file position; flush already completed above.
        writer.get_mut().seek(SeekFrom::Start(end))?;
        #[cfg(feature = "bounded-engine")]
        let footer_magic = if self.source_references {
            compact::FOOTER
        } else {
            FOOTER
        };
        #[cfg(not(feature = "bounded-engine"))]
        let footer_magic = FOOTER;
        let mut footer = footer_magic.to_vec();
        footer.extend_from_slice(&directory_offset.to_le_bytes());
        footer.extend_from_slice(&directory_len.to_le_bytes());
        footer.extend_from_slice(&checksum.to_le_bytes());
        footer.extend_from_slice(&crc(&footer).to_le_bytes());
        writer.write_all(&footer)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
        self.writer.take();
        #[cfg(all(feature = "bounded-engine", test))]
        compact::builder_stage("prepared");
        let index = DiskIndex::open(&self.temporary, None)?;
        if self.target.exists() {
            std::fs::remove_file(&self.target)?;
        }
        #[cfg(all(feature = "bounded-engine", test))]
        compact::builder_stage("unlinked");
        std::fs::rename(&self.temporary, &self.target)?;
        #[cfg(all(feature = "bounded-engine", test))]
        compact::builder_stage("published");
        Ok(index)
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        self.writer.take();
        let _ = std::fs::remove_file(&self.temporary);
    }
}

#[derive(Clone, Copy)]
struct PackedRecord {
    offset: u64,
    version: u64,
    group: u32,
    id_offset: u32,
    id_len: u32,
    len: u32,
    checksum: u32,
    deleted: bool,
}

pub(crate) struct DiskIndex {
    #[cfg(feature = "bounded-engine")]
    compact_directory: bool,
    #[cfg(feature = "bounded-engine")]
    source_references: bool,
    #[cfg(feature = "bounded-engine")]
    payload_source: Option<File>,
    #[cfg(feature = "bounded-engine")]
    covering: covering::Covering,
    file: File,
    verification: Option<block_check::Checks>,
    binary_payload: bool,
    #[cfg(feature = "experimental-rkyv-payload")]
    payload_benchmark: Option<payload_bench::Overlay>,
    #[cfg(feature = "experimental-paged-metadata")]
    paged: Option<paged::PagedRecords>,
    records: Vec<PackedRecord>,
    groups: Vec<(String, String)>,
    ids: String,
    postings: BTreeMap<Term, Posting>,
    #[cfg(feature = "experimental-streaming-open")]
    term_pages: Option<term_pages::Pages>,
    pub sequence: u64,
    pub bytes: u64,
    pub posting_entries: u64,
    live_records: usize,
}

// Positional I/O leaves the shared file cursor untouched and avoids unsafe mmap.
pub(crate) fn read_at(file: &File, mut offset: u64, mut buffer: &mut [u8]) -> Result<()> {
    #[cfg(feature = "diagnostic-timing")]
    let _read = crate::timing::span("disk.read_at");
    while !buffer.is_empty() {
        #[cfg(windows)]
        let n = {
            use std::os::windows::fs::FileExt;
            file.seek_read(buffer, offset)?
        };
        #[cfg(unix)]
        let n = {
            use std::os::unix::fs::FileExt;
            file.read_at(buffer, offset)?
        };
        if n == 0 {
            return Err(Error::Corrupt("short segment read".into()));
        }
        offset += n as u64;
        buffer = &mut buffer[n..];
    }
    Ok(())
}

type KeyParts<'a> = (&'a str, &'a str, &'a str);

impl DiskIndex {
    #[cfg(feature = "experimental-merge-postings")]
    fn term_cursor(&self) -> TermCursor<'_> {
        #[cfg(feature = "experimental-streaming-open")]
        if let Some(pages) = &self.term_pages {
            return TermCursor::Pages(pages.cursor());
        }
        TermCursor::Memory(self.postings.iter())
    }
    pub(crate) fn memory_layout(&self, source_id: u64) -> crate::IndexMemoryLayout {
        let key_bytes =
            |key: &Key| key.scope.capacity() + key.collection.capacity() + key.id.capacity();
        let term_string_capacity_bytes = self
            .postings
            .keys()
            .map(|term| match term {
                Term::Scope(s) => s.capacity(),
                Term::Collection(s, c) | Term::Gram(s, c) => s.capacity() + c.capacity(),
                Term::Exact(s, c, f, v) => {
                    s.capacity() + c.capacity() + f.capacity() + v.capacity()
                }
                Term::Numeric(s, c, f, _) => s.capacity() + c.capacity() + f.capacity(),
                Term::Link(key) => key_bytes(key),
            })
            .sum();
        crate::IndexMemoryLayout {
            source_id,
            records: self.len(),
            record_vector_capacity_bytes: self.records.capacity()
                * std::mem::size_of::<PackedRecord>(),
            id_pool_capacity_bytes: self.ids.capacity(),
            group_string_capacity_bytes: self
                .groups
                .iter()
                .map(|(s, c)| s.capacity() + c.capacity())
                .sum(),
            #[cfg(feature = "experimental-paged-metadata")]
            paged_root_capacity_bytes: self.paged.as_ref().map_or(0, |p| p.capacities().0),
            #[cfg(feature = "experimental-paged-metadata")]
            paged_deletion_capacity_bytes: self.paged.as_ref().map_or(0, |p| p.capacities().1),
            terms: {
                #[cfg(feature = "experimental-streaming-open")]
                {
                    self.term_pages
                        .as_ref()
                        .map_or(self.postings.len(), |p| p.count)
                }
                #[cfg(not(feature = "experimental-streaming-open"))]
                {
                    self.postings.len()
                }
            },
            #[cfg(feature = "experimental-streaming-open")]
            term_page_root_capacity_bytes: self.term_pages.as_ref().map_or(0, |p| p.root_bytes())
                + {
                    #[cfg(feature = "bounded-engine")]
                    {
                        self.covering.root_bytes()
                    }
                    #[cfg(not(feature = "bounded-engine"))]
                    {
                        0
                    }
                },
            term_string_capacity_bytes,
        }
    }
    pub fn open(path: &Path, binding: Option<(u64, u32)>) -> Result<Self> {
        Self::open_with_mode(
            path,
            binding,
            #[cfg(feature = "experimental-streaming-metadata")]
            true,
        )
    }
    fn open_with_mode(
        path: &Path,
        binding: Option<(u64, u32)>,
        #[cfg(feature = "experimental-streaming-metadata")] streamed: bool,
    ) -> Result<Self> {
        Self::open_checked(
            path,
            binding,
            false,
            #[cfg(feature = "experimental-streaming-metadata")]
            streamed,
        )
    }
    pub(crate) fn open_deferred(path: &Path, binding: Option<(u64, u32)>) -> Result<Self> {
        Self::open_checked(
            path,
            binding,
            true,
            #[cfg(feature = "experimental-streaming-metadata")]
            false,
        )
    }
    fn open_checked(
        path: &Path,
        binding: Option<(u64, u32)>,
        blocks: bool,
        #[cfg(feature = "experimental-streaming-metadata")] streamed: bool,
    ) -> Result<Self> {
        #[cfg(feature = "diagnostic-memory")]
        let _origin = crate::memory::Scope::new(7);
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("segment.open");
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let size = metadata.len();
        #[cfg(feature = "bounded-engine")]
        let covering = covering::Covering::new(&metadata);
        if size < HEADER_SIZE + FOOTER_SIZE {
            return Err(Error::Corrupt("truncated segment".into()));
        }
        let mut header = [0; HEADER_SIZE as usize];
        read_at(&file, 0, &mut header)?;
        #[cfg(feature = "bounded-engine")]
        let compact_directory = &header[..8] == compact::MAGIC;
        #[cfg(not(feature = "bounded-engine"))]
        let compact_directory = false;
        if &header[..8] != MAGIC && &header[..8] != LEGACY_MAGIC && !compact_directory {
            return Err(Error::Corrupt("unsupported segment format".into()));
        }
        let binary_payload = &header[..8] == MAGIC || compact_directory;
        let sequence = u64::from_le_bytes(header[8..16].try_into().unwrap());
        let end = u64::from_le_bytes(header[16..24].try_into().unwrap());
        let log_crc = u32::from_le_bytes(header[24..28].try_into().unwrap());
        if binding.is_some_and(|expected| expected != (end, log_crc)) {
            return Err(Error::Corrupt("segment/checkpoint binding mismatch".into()));
        }
        let mut footer = [0; FOOTER_SIZE as usize];
        read_at(&file, size - FOOTER_SIZE, &mut footer)?;
        let footer_magic = if binary_payload {
            FOOTER
        } else {
            LEGACY_FOOTER
        };
        #[cfg(feature = "bounded-engine")]
        let footer_magic = if compact_directory {
            compact::FOOTER
        } else {
            footer_magic
        };
        if &footer[..8] != footer_magic
            || crc(&footer[..28]) != u32::from_le_bytes(footer[28..32].try_into().unwrap())
        {
            return Err(Error::Corrupt("invalid segment footer".into()));
        }
        let offset = u64::from_le_bytes(footer[8..16].try_into().unwrap());
        let len = u64::from_le_bytes(footer[16..24].try_into().unwrap());
        if offset < HEADER_SIZE
            || len > MAX_DIRECTORY
            || offset.checked_add(len) != Some(size - FOOTER_SIZE)
        {
            return Err(Error::Corrupt("invalid segment directory bounds".into()));
        }
        let expected = u32::from_le_bytes(footer[24..28].try_into().unwrap());
        #[cfg(feature = "bounded-engine")]
        let source_references = compact_directory && compact::flags(&file, offset)?;
        let verification = if blocks {
            block_check::Checks::load(path, size - FOOTER_SIZE, expected).ok()
        } else {
            None
        };
        if let Some(checks) = &verification {
            checks.verify(&file, 0, HEADER_SIZE as usize)?;
            checks.verify(&file, offset, len as usize)?;
        } else {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("open.index_crc");
            checksum::check(&mut file, size - FOOTER_SIZE, expected)?;
        }
        #[cfg(feature = "experimental-streaming-metadata")]
        if streamed {
            let (paged, parsed) =
                paged::prepare(path, &mut file, size, crc(&footer), sequence, offset, len)?;
            let bytes = size
                .checked_add(paged.bytes)
                .ok_or_else(|| Error::Corrupt("index byte count overflow".into()))?;
            let term_pages = Some(term_pages::Pages::new(
                &file,
                parsed.pages,
                parsed.collections,
                parsed.bloom,
            )?);
            return Ok(Self {
                #[cfg(feature = "bounded-engine")]
                compact_directory,
                #[cfg(feature = "bounded-engine")]
                source_references,
                #[cfg(feature = "bounded-engine")]
                payload_source: None,
                #[cfg(feature = "bounded-engine")]
                covering: covering.with_grams(parsed.universal),
                file,
                verification,
                binary_payload,
                #[cfg(feature = "experimental-rkyv-payload")]
                payload_benchmark: None,
                paged: Some(paged),
                records: Vec::new(),
                groups: Vec::new(),
                ids: String::new(),
                term_pages,
                postings: parsed.postings,
                sequence,
                bytes,
                posting_entries: parsed.entries,
                live_records: parsed.live,
            });
        }
        #[cfg(feature = "experimental-streaming-open")]
        {
            let parsed = streaming::read(&mut file, offset, len, sequence)?;
            let term_pages = Some(term_pages::Pages::new(
                &file,
                parsed.pages,
                parsed.collections,
                parsed.bloom,
            )?);
            Ok(Self {
                #[cfg(feature = "bounded-engine")]
                compact_directory,
                #[cfg(feature = "bounded-engine")]
                source_references,
                #[cfg(feature = "bounded-engine")]
                payload_source: None,
                #[cfg(feature = "bounded-engine")]
                covering: covering.with_grams(parsed.universal),
                file,
                verification,
                binary_payload,
                #[cfg(feature = "experimental-rkyv-payload")]
                payload_benchmark: None,
                paged: None,
                records: parsed.records,
                groups: parsed.groups,
                ids: parsed.ids,
                term_pages,
                postings: parsed.postings,
                sequence,
                bytes: size,
                posting_entries: parsed.entries,
                live_records: parsed.live,
            })
        }
        #[cfg(not(feature = "experimental-streaming-open"))]
        {
            let mut bytes = vec![0; len as usize];
            read_at(&file, offset, &mut bytes)?;
            let directory: Directory =
                serde_json::from_slice(&bytes).map_err(|e| Error::Corrupt(e.to_string()))?;
            drop(bytes);
            let mut cursor = HEADER_SIZE;
            let mut previous_key = None;
            let mut id_bytes = 0usize;
            for record in &directory.records {
                crate::validate_key(&record.key).map_err(|e| Error::Corrupt(e.to_string()))?;
                if record.offset != cursor
                    || (record.deleted != (record.len == 0))
                    || record.len > 4 * 1024 * 1024 + 128
                    || record.version == 0
                    || record.version > sequence
                    || previous_key.is_some_and(|key: &Key| key >= &record.key)
                {
                    return Err(Error::Corrupt("invalid segment record directory".into()));
                }
                cursor = cursor
                    .checked_add(record.len as u64)
                    .ok_or_else(|| Error::Corrupt("segment offset overflow".into()))?;
                previous_key = Some(&record.key);
                id_bytes = id_bytes
                    .checked_add(record.key.id.len())
                    .ok_or_else(|| Error::Corrupt("key pool length overflow".into()))?;
            }
            let mut previous_term = None;
            let mut entries = 0u64;
            for (term, p) in &directory.postings {
                if p.offset != cursor
                    || p.count == 0
                    || p.count as usize > directory.records.len()
                    || previous_term.is_some_and(|t: &Term| t >= term)
                {
                    return Err(Error::Corrupt("invalid segment posting directory".into()));
                }
                cursor = cursor
                    .checked_add(p.count as u64 * 4)
                    .ok_or_else(|| Error::Corrupt("segment posting overflow".into()))?;
                entries += p.count as u64;
                previous_term = Some(term);
            }
            if cursor != offset {
                return Err(Error::Corrupt("noncontiguous segment data".into()));
            }
            let live_records = directory.records.iter().filter(|r| !r.deleted).count();
            let mut groups = vec![];
            let mut group_ids = BTreeMap::new();
            let mut ids = String::with_capacity(id_bytes);
            let mut records = Vec::with_capacity(directory.records.len());
            for r in directory.records {
                let pair = (r.key.scope, r.key.collection);
                let group = if let Some(&id) = group_ids.get(&pair) {
                    id
                } else {
                    let id = groups.len() as u32;
                    group_ids.insert(pair.clone(), id);
                    groups.push(pair);
                    id
                };
                let id_offset = u32::try_from(ids.len())
                    .map_err(|_| Error::Invalid("key pool exceeds u32 offsets".into()))?;
                let id_len = r.key.id.len() as u32;
                ids.push_str(&r.key.id);
                records.push(PackedRecord {
                    offset: r.offset,
                    version: r.version,
                    group,
                    id_offset,
                    id_len,
                    len: r.len,
                    checksum: r.checksum,
                    deleted: r.deleted,
                });
            }
            Ok(Self {
                file,
                verification,
                binary_payload,
                #[cfg(feature = "experimental-rkyv-payload")]
                payload_benchmark: None,
                #[cfg(feature = "experimental-paged-metadata")]
                paged: None,
                records,
                groups,
                ids,
                postings: directory.postings.into_iter().collect(),
                sequence,
                bytes: size,
                posting_entries: entries,
                live_records,
            })
        }
    }

    #[cfg(feature = "experimental-paged-metadata")]
    pub fn enable_paging(&mut self, path: &Path) -> Result<()> {
        if self.paged.is_some() {
            return Ok(());
        }
        let paged = paged::PagedRecords::prepare(path, self)?;
        self.bytes = self
            .bytes
            .checked_add(paged.bytes)
            .ok_or_else(|| Error::Corrupt("index byte count overflow".into()))?;
        self.paged = Some(paged);
        self.records = Vec::new();
        self.ids = String::new();
        self.groups = Vec::new();
        Ok(())
    }
    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<()> {
        if let Some(checks) = &self.verification {
            checks.verify(&self.file, offset, buffer.len())?;
        }
        read_at(&self.file, offset, buffer)
    }
    #[cfg(feature = "experimental-grouped-read")]
    fn payload_file(&self) -> Result<&File> {
        #[cfg(feature = "bounded-engine")]
        if self.source_references {
            return self
                .payload_source
                .as_ref()
                .ok_or_else(|| Error::Corrupt("unbound SEG05 source".into()));
        }
        Ok(&self.file)
    }
    fn read_payload_at(&self, offset: u64, buffer: &mut [u8]) -> Result<()> {
        #[cfg(feature = "bounded-engine")]
        if self.source_references {
            return read_at(self.payload_file()?, offset, buffer);
        }
        self.read_at(offset, buffer)
    }
    pub(crate) fn prepare_open_checks(&self, path: &Path) -> Result<()> {
        let size = self.file.metadata()?.len();
        let mut footer = [0; FOOTER_SIZE as usize];
        read_at(&self.file, size - FOOTER_SIZE, &mut footer)?;
        if crc(&footer[..28]) != u32::from_le_bytes(footer[28..].try_into().unwrap()) {
            return Err(Error::Corrupt("invalid segment footer".into()));
        }
        block_check::Checks::prepare(
            path,
            &self.file,
            size - FOOTER_SIZE,
            u32::from_le_bytes(footer[24..28].try_into().unwrap()),
        )
    }
    pub fn len(&self) -> usize {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.len();
        }
        self.records.len()
    }
    fn key_parts(&self, id: u32) -> (&str, &str, &str) {
        let r = &self.records[id as usize];
        let (scope, collection) = &self.groups[r.group as usize];
        (
            scope,
            collection,
            &self.ids[r.id_offset as usize..(r.id_offset + r.id_len) as usize],
        )
    }
    fn metadata(&self, id: u32) -> Result<PackedRecord> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("query.record_metadata");
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            let r = paged.record(id)?;
            return Ok(PackedRecord {
                offset: r.offset,
                version: r.version,
                len: r.len,
                checksum: r.checksum,
                deleted: r.deleted,
                group: 0,
                id_offset: 0,
                id_len: 0,
            });
        }
        self.records
            .get(id as usize)
            .copied()
            .ok_or_else(|| Error::Corrupt("invalid metadata slot".into()))
    }

    pub(crate) fn payload_binding(&self, id: u32) -> Result<(u32, u32, u64, bool)> {
        let row = self.metadata(id)?;
        Ok((row.len, row.checksum, row.version, row.deleted))
    }
    pub(crate) fn checksum_binding(&self) -> Result<u32> {
        let size = self.file.metadata()?.len();
        let mut footer = [0; FOOTER_SIZE as usize];
        read_at(&self.file, size - FOOTER_SIZE, &mut footer)?;
        if crc(&footer[..28]) != u32::from_le_bytes(footer[28..].try_into().unwrap()) {
            return Err(Error::Corrupt("invalid segment footer".into()));
        }
        Ok(u32::from_le_bytes(footer[24..28].try_into().unwrap()))
    }
    #[cfg(feature = "experimental-paged-metadata")]
    fn record(&self, id: u32) -> Result<Record> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.record(id);
        }
        let r = self
            .records
            .get(id as usize)
            .ok_or_else(|| Error::Corrupt("invalid metadata slot".into()))?;
        Ok(Record {
            key: self.key(id)?,
            offset: r.offset,
            len: r.len,
            checksum: r.checksum,
            version: r.version,
            deleted: r.deleted,
        })
    }
    pub fn key(&self, id: u32) -> Result<Key> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return Ok(paged.record(id)?.key);
        }
        if id as usize >= self.records.len() {
            return Err(Error::Corrupt("invalid key slot".into()));
        }
        let (scope, collection, id) = self.key_parts(id);
        Ok(Key::new(scope, collection, id))
    }
    pub fn version(&self, key: &Key) -> Result<Option<u64>> {
        match self.find(key)? {
            Some(id) if !self.deleted(id) => Ok(Some(self.metadata(id)?.version)),
            _ => Ok(None),
        }
    }
    pub fn contains_key(&self, key: &Key) -> Result<bool> {
        Ok(self.find(key)?.is_some())
    }
    pub fn deleted(&self, id: u32) -> bool {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.deleted(id);
        }
        self.records[id as usize].deleted
    }
    pub fn find(&self, key: &Key) -> Result<Option<u32>> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.find(key);
        }
        Ok(self.find_parts((key.scope.as_str(), key.collection.as_str(), key.id.as_str())))
    }
    fn find_parts(&self, key: (&str, &str, &str)) -> Option<u32> {
        if self.records.is_empty()
            || key < self.key_parts(0)
            || key > self.key_parts(self.len() as u32 - 1)
        {
            return None;
        }
        let (mut lo, mut hi) = (0, self.len() as u32);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.key_parts(mid).cmp(&key) {
                std::cmp::Ordering::Equal => return Some(mid),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        None
    }
    fn bounds(&self) -> Option<(KeyParts<'_>, KeyParts<'_>)> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.bounds().map(|(a, b)| {
                (
                    (a.scope.as_str(), a.collection.as_str(), a.id.as_str()),
                    (b.scope.as_str(), b.collection.as_str(), b.id.as_str()),
                )
            });
        }
        if self.len() == 0 {
            None
        } else {
            Some((self.key_parts(0), self.key_parts(self.len() as u32 - 1)))
        }
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        match (self.bounds(), other.bounds()) {
            (Some((a, b)), Some((c, d))) => a <= d && c <= b,
            _ => false,
        }
    }
    pub fn hide_keys(&self, newer: &Self, mask: &mut [u64]) -> Result<()> {
        if !self.overlaps(newer) {
            return Ok(());
        }
        for n in 0..newer.len() as u32 {
            if let Some(id) = self.find(&newer.key(n)?)? {
                mask[id as usize / 64] |= 1 << (id % 64);
            }
        }
        Ok(())
    }
    pub fn get(&self, key: &Key) -> Result<Option<StoredDocument>> {
        self.find(key)?
            .filter(|id| !self.deleted(*id))
            .map(|id| self.read_document(id))
            .transpose()
    }

    /// Hydrate a slot already selected by the query's visibility mask.
    /// Payload CRC and key/version identity checks also apply to this path.
    #[cfg(feature = "experimental-direct-slot")]
    pub(crate) fn read_selected_document(&self, id: u32) -> Result<StoredDocument> {
        if id as usize >= self.len() || self.deleted(id) {
            return Err(Error::Corrupt(
                "cannot hydrate invalid/tombstone slot".into(),
            ));
        }
        self.read_document(id)
    }

    #[cfg(feature = "experimental-paged-metadata")]
    fn read_text_document(
        &self,
        id: u32,
        cursor: &mut paged::TextCursor,
    ) -> Result<StoredDocument> {
        if let Some(paged) = &self.paged {
            paged.text_document(id, cursor, self.payload_file()?)
        } else {
            self.read_document(id)
        }
    }
    fn read_document(&self, id: u32) -> Result<StoredDocument> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("payload.read_document");
        #[cfg(feature = "experimental-rkyv-payload")]
        if let Some(d) = self.benchmark_document(id)? {
            return Ok(d);
        }
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            let record = paged.record(id)?;
            let mut bytes = vec![0; record.len as usize];
            self.read_payload_at(record.offset, &mut bytes)?;
            if crc(&bytes) != record.checksum {
                return Err(Error::Corrupt("segment document checksum mismatch".into()));
            }
            let stored = self.decode_payload(&bytes)?;
            if stored.document.key != record.key || stored.version != record.version {
                return Err(Error::Corrupt("segment document identity mismatch".into()));
            }
            return Ok(stored);
        }
        let r = self.metadata(id)?;
        let mut bytes = vec![0; r.len as usize];
        self.read_payload_at(r.offset, &mut bytes)?;
        self.decode_document(id, &bytes)
    }

    fn decode_payload(&self, bytes: &[u8]) -> Result<StoredDocument> {
        if self.binary_payload {
            crate::payload::checked(bytes)?.to_document()
        } else {
            crate::payload::decode(bytes)
        }
    }

    fn binary_matches(
        &self,
        id: u32,
        scratch: &mut QueryTextScratch,
        terms: &[String],
        cache: &mut BTreeMap<String, String>,
        cached_bytes: &mut usize,
    ) -> Result<bool> {
        self.binary_matches_with_capture(id, scratch, terms, cache, cached_bytes, &mut |_, _, _| {})
    }

    fn binary_matches_with_capture(
        &self,
        id: u32,
        scratch: &mut QueryTextScratch,
        terms: &[String],
        cache: &mut BTreeMap<String, String>,
        cached_bytes: &mut usize,
        capture: &mut impl FnMut(u32, &crate::payload::View<'_>, &[u8]),
    ) -> Result<bool> {
        #[cfg(feature = "experimental-grouped-read")]
        if scratch.read_window_enabled {
            return self.binary_window_matches(id, scratch, terms, cache, cached_bytes, capture);
        }
        self.binary_exact_matches(id, scratch, terms, cache, cached_bytes, capture)
    }

    fn query_metadata(&self, id: u32, scratch: &mut QueryTextScratch) -> Result<PackedRecord> {
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            #[cfg(feature = "diagnostic-timing")]
            let _span = crate::timing::span("query.record_metadata");
            return paged.query_metadata(id, &mut scratch.record_cursor);
        }
        #[cfg(not(feature = "experimental-paged-metadata"))]
        let _ = scratch;
        self.metadata(id)
    }

    fn query_identity(
        &self,
        id: u32,
        scratch: &mut QueryTextScratch,
        parts: (&str, &str, &str),
    ) -> Result<bool> {
        #[cfg(feature = "diagnostic-timing")]
        let _span = crate::timing::span("query.record_identity");
        #[cfg(feature = "experimental-paged-metadata")]
        if let Some(paged) = &self.paged {
            return paged.query_identity(id, &mut scratch.record_cursor, parts);
        }
        #[cfg(not(feature = "experimental-paged-metadata"))]
        let _ = scratch;
        Ok(self.key_parts(id) == parts)
    }

    // Keep sparse/narrow verification on the original individual-read loop.
    #[inline(always)]
    fn binary_exact_matches(
        &self,
        id: u32,
        scratch: &mut QueryTextScratch,
        terms: &[String],
        cache: &mut BTreeMap<String, String>,
        cached_bytes: &mut usize,
        capture: &mut impl FnMut(u32, &crate::payload::View<'_>, &[u8]),
    ) -> Result<bool> {
        let r = self.query_metadata(id, scratch)?;
        scratch.payload.resize(r.len as usize, 0);
        {
            #[cfg(feature = "diagnostic-timing")]
            let _read = crate::timing::span("query.payload_read");
            self.read_payload_at(r.offset, &mut scratch.payload)?;
        }
        #[cfg(feature = "diagnostic-timing")]
        {
            scratch.reads.requests += 1;
            scratch.reads.read_bytes += r.len as u64;
            scratch.reads.requested_bytes += r.len as u64;
        }
        {
            #[cfg(feature = "diagnostic-timing")]
            let _crc = crate::timing::span("query.payload_crc");
            if crc(&scratch.payload) != r.checksum {
                return Err(Error::Corrupt("segment document checksum mismatch".into()));
            }
        }
        let bytes = std::mem::take(&mut scratch.payload);
        let result = (|| {
            let view = {
                #[cfg(feature = "diagnostic-timing")]
                let _validation = crate::timing::span("query.payload_validation");
                crate::payload::checked(&bytes)?
            };
            if !self.query_identity(id, scratch, view.key_parts())? || view.version() != r.version {
                return Err(Error::Corrupt("segment document identity mismatch".into()));
            }
            let matched =
                scratch.matches_text(view.title(), view.text(), terms, cache, cached_bytes);
            if matched {
                capture(id, &view, &bytes);
            }
            Ok(matched)
        })();
        scratch.payload = bytes;
        result
    }

    #[cfg(feature = "experimental-grouped-read")]
    #[inline(never)]
    fn binary_window_matches(
        &self,
        id: u32,
        scratch: &mut QueryTextScratch,
        terms: &[String],
        cache: &mut BTreeMap<String, String>,
        cached_bytes: &mut usize,
        capture: &mut impl FnMut(u32, &crate::payload::View<'_>, &[u8]),
    ) -> Result<bool> {
        let r = self.query_metadata(id, scratch)?;
        const WINDOW: usize = 256 * 1024;
        let read_limit = if let Some(limit) = scratch.read_limit {
            limit
        } else {
            #[cfg(feature = "bounded-engine")]
            let footer = if self.source_references {
                0
            } else {
                FOOTER_SIZE
            };
            #[cfg(not(feature = "bounded-engine"))]
            let footer = FOOTER_SIZE;
            let limit = self
                .payload_file()?
                .metadata()?
                .len()
                .checked_sub(footer)
                .ok_or_else(|| Error::Corrupt("segment payload read bounds".into()))?;
            scratch.read_limit = Some(limit);
            limit
        };
        let end = r
            .offset
            .checked_add(r.len as u64)
            .filter(|end| *end <= read_limit)
            .ok_or_else(|| Error::Corrupt("segment payload read bounds".into()))?;
        let adjacent = scratch.previous_end.is_some_and(|previous| {
            if previous == r.offset {
                return true;
            }
            #[cfg(feature = "bounded-engine")]
            if self.source_references {
                // FDBLOG04: 5-byte entry header, or footer8 + frame24 +
                // batch5 + entry5 at a checkpoint-frame boundary. Payloads
                // retain per-record CRC/schema/identity checks after slicing.
                return matches!(r.offset.checked_sub(previous), Some(5) | Some(42));
            }
            false
        });
        scratch.adjacent = if adjacent {
            scratch.adjacent.saturating_add(1)
        } else {
            0
        };
        scratch.previous_end = Some(end);
        let covered = scratch.read_len != 0
            && r.offset >= scratch.read_base
            && end <= scratch.read_base + scratch.read_len as u64;
        // Judge a completed window, not the freshly prefetched bytes. If less
        // than three quarters was useful, keep the rest of this query exact.
        // This bounds wasted read-ahead without penalizing every dense seed.
        if !covered && scratch.window_prefetched && scratch.window_used * 4 < scratch.read_len * 3 {
            scratch.read_window_enabled = false;
            scratch.read_len = 0;
            return self.binary_exact_matches(id, scratch, terms, cache, cached_bytes, capture);
        }
        #[cfg(feature = "diagnostic-timing")]
        {
            scratch.reads.requested_bytes += r.len as u64;
        }
        {
            #[cfg(feature = "diagnostic-timing")]
            let _read = crate::timing::span("query.payload_read");
            if !covered {
                // A broad seed alone is insufficient: require three physically
                // adjacent requested records before reading ahead. Sparse and
                // narrow queries keep exact individual reads.
                let length = if scratch.read_window_enabled
                    && scratch.adjacent >= 2
                    && r.len as usize <= WINDOW
                {
                    (read_limit - r.offset).min(WINDOW as u64) as usize
                } else {
                    r.len as usize
                };
                scratch.payload.resize(length, 0);
                scratch.read_len = 0;
                self.read_payload_at(r.offset, &mut scratch.payload)?;
                scratch.read_base = r.offset;
                scratch.read_len = length;
                scratch.window_used = 0;
                scratch.window_prefetched = length > r.len as usize;
                #[cfg(feature = "diagnostic-timing")]
                {
                    scratch.reads.requests += 1;
                    scratch.reads.read_bytes += length as u64;
                }
            } else {
                #[cfg(feature = "diagnostic-timing")]
                {
                    scratch.reads.hits += 1;
                }
            }
        }
        scratch.window_used = scratch
            .window_used
            .saturating_add(r.len as usize)
            .min(scratch.read_len);
        let start = (r.offset - scratch.read_base) as usize;
        let payload_range = start..start + r.len as usize;
        {
            #[cfg(feature = "diagnostic-timing")]
            let _crc = crate::timing::span("query.payload_crc");
            if crc(&scratch.payload[payload_range.clone()]) != r.checksum {
                return Err(Error::Corrupt("segment document checksum mismatch".into()));
            }
        }
        // Take the reusable buffer so title/text can be borrowed while mutating fold scratch.
        let bytes = std::mem::take(&mut scratch.payload);
        let result = (|| {
            let view = {
                #[cfg(feature = "diagnostic-timing")]
                let _validation = crate::timing::span("query.payload_validation");
                crate::payload::checked(&bytes[payload_range.clone()])?
            };
            if !self.query_identity(id, scratch, view.key_parts())? || view.version() != r.version {
                return Err(Error::Corrupt("segment document identity mismatch".into()));
            }
            let matched =
                scratch.matches_text(view.title(), view.text(), terms, cache, cached_bytes);
            if matched {
                capture(id, &view, &bytes[payload_range]);
            }
            Ok(matched)
        })();
        scratch.payload = bytes;
        result
    }

    #[cfg(feature = "experimental-grouped-read")]
    fn broad_payload_seed(&self, group: &[Posting]) -> bool {
        #[cfg(feature = "experimental-rkyv-payload")]
        if self.payload_benchmark.is_some() {
            return false;
        }
        self.live_records >= 64
            && group.iter().map(|p| p.count as u64).sum::<u64>()
                >= self.live_records.div_ceil(2) as u64
    }

    fn decode_document(&self, id: u32, bytes: &[u8]) -> Result<StoredDocument> {
        #[cfg(feature = "diagnostic-timing")]
        let _decode = crate::timing::span("payload.decode_crc_identity");
        let r = self.metadata(id)?;
        if bytes.len() != r.len as usize || crc(bytes) != r.checksum {
            return Err(Error::Corrupt("segment document checksum mismatch".into()));
        }
        let stored = self.decode_payload(bytes)?;
        let key = &stored.document.key;
        #[cfg(feature = "experimental-paged-metadata")]
        let identity = if let Some(paged) = &self.paged {
            key == &paged.record(id)?.key
        } else {
            (key.scope.as_str(), key.collection.as_str(), key.id.as_str()) == self.key_parts(id)
        };
        #[cfg(not(feature = "experimental-paged-metadata"))]
        let identity =
            (key.scope.as_str(), key.collection.as_str(), key.id.as_str()) == self.key_parts(id);
        if !identity || stored.version != r.version {
            return Err(Error::Corrupt("segment document identity mismatch".into()));
        }
        Ok(stored)
    }

    /// Sorted visible slots, tagged with their final position in the global page.
    /// Nearby payloads share one read window; only selected payloads are decoded.
    #[cfg(feature = "experimental-grouped-read")]
    pub(crate) fn read_selected_batch(
        &self,
        slots: &[(u32, usize)],
        scratch: &mut Vec<u8>,
        output: &mut Vec<(usize, StoredDocument)>,
    ) -> Result<BatchReadStats> {
        let mut previous = None;
        for &(id, _) in slots {
            if id as usize >= self.len() || self.deleted(id) || previous.is_some_and(|p| p >= id) {
                return Err(Error::Corrupt("unordered or deleted selected slot".into()));
            }
            previous = Some(id);
        }
        let mut stats = BatchReadStats::default();
        // Reuse one checked metadata leaf while hydrating a sorted page. The
        // payload still gets its own CRC, schema and key/version checks.
        let mut metadata = QueryTextScratch::default();
        let mut start = 0;
        while start < slots.len() {
            let (id, position) = slots[start];
            let first = self.query_metadata(id, &mut metadata)?;
            if first.len as usize > READ_WINDOW {
                output.push((position, self.read_selected_document(id)?));
                stats.requests += 1;
                stats.requested_bytes += first.len as u64;
                stats.oversized_documents += 1;
                start += 1;
                continue;
            }
            let offset = first.offset;
            let mut end = offset
                .checked_add(first.len as u64)
                .ok_or_else(|| Error::Corrupt("selected payload end overflow".into()))?;
            let mut next = start + 1;
            while next < slots.len() {
                let r = self.query_metadata(slots[next].0, &mut metadata)?;
                let record_end = r
                    .offset
                    .checked_add(r.len as u64)
                    .ok_or_else(|| Error::Corrupt("selected payload end overflow".into()))?;
                if r.offset < end
                    || r.offset - end > READ_GAP
                    || record_end - offset > READ_WINDOW as u64
                {
                    break;
                }
                end = record_end;
                next += 1;
            }
            let len = usize::try_from(end - offset)
                .map_err(|_| Error::Corrupt("selected read length overflow".into()))?;
            if scratch.capacity() < len {
                scratch.reserve_exact(len.saturating_sub(scratch.len()));
            }
            scratch.resize(len, 0);
            {
                #[cfg(feature = "diagnostic-timing")]
                let _read = crate::timing::span("payload.result_read");
                self.read_payload_at(offset, scratch)?;
            }
            stats.requests += 1;
            stats.requested_bytes += len as u64;
            for &(id, position) in &slots[start..next] {
                let r = self.query_metadata(id, &mut metadata)?;
                let begin = (r.offset - offset) as usize;
                let bytes = scratch
                    .get(begin..begin + r.len as usize)
                    .ok_or_else(|| Error::Corrupt("invalid grouped payload bounds".into()))?;
                output.push((
                    position,
                    self.decode_selected(id, &r, bytes, &mut metadata)?,
                ));
            }
            start = next;
        }
        Ok(stats)
    }

    #[cfg(feature = "experimental-grouped-read")]
    fn decode_selected(
        &self,
        id: u32,
        record: &PackedRecord,
        bytes: &[u8],
        scratch: &mut QueryTextScratch,
    ) -> Result<StoredDocument> {
        #[cfg(feature = "diagnostic-timing")]
        let _decode = crate::timing::span("payload.decode_crc_identity");
        {
            #[cfg(feature = "diagnostic-timing")]
            let _crc = crate::timing::span("payload.result_crc");
            if bytes.len() != record.len as usize || crc(bytes) != record.checksum {
                return Err(Error::Corrupt("segment document checksum mismatch".into()));
            }
        }
        let stored = self.decode_payload(bytes)?;
        let key = &stored.document.key;
        {
            #[cfg(feature = "diagnostic-timing")]
            let _identity = crate::timing::span("payload.result_identity");
            if stored.version != record.version
                || !self.query_identity(id, scratch, (&key.scope, &key.collection, &key.id))?
            {
                return Err(Error::Corrupt("segment document identity mismatch".into()));
            }
        }
        Ok(stored)
    }

    fn one(&self, term: Term) -> Result<Vec<Posting>> {
        #[cfg(feature = "experimental-streaming-open")]
        if let Some(pages) = &self.term_pages {
            return Ok(pages.get(&term)?.into_iter().collect());
        }
        Ok(self.postings.get(&term).copied().into_iter().collect())
    }
    fn posting_range(&self, min: Term, max: Term) -> Result<Vec<Posting>> {
        #[cfg(feature = "experimental-streaming-open")]
        if let Some(pages) = &self.term_pages {
            return pages.range(&min, &max);
        }
        Ok(self.postings.range(min..=max).map(|(_, p)| *p).collect())
    }
    fn collections(&self, q: &Query) -> BTreeSet<&str> {
        // Returned references belong to this index; query collections are handled
        // by the caller via a separate owned set below.
        #[cfg(feature = "experimental-streaming-open")]
        if let Some(pages) = &self.term_pages {
            return pages
                .collections
                .iter()
                .filter(|(s, c)| {
                    s == &q.scope && q.collection.as_ref().is_none_or(|wanted| wanted == c)
                })
                .map(|(_, c)| c.as_str())
                .collect();
        }
        self.postings
            .keys()
            .filter_map(|t| match t {
                Term::Collection(s, c)
                    if s == &q.scope && q.collection.as_ref().is_none_or(|wanted| wanted == c) =>
                {
                    Some(c.as_str())
                }
                _ => None,
            })
            .collect()
    }
    fn id_at(&self, p: Posting, slot: u32) -> Result<u32> {
        #[cfg(feature = "diagnostic-timing")]
        let _probe = crate::timing::span("query.posting_probe");
        let mut bytes = [0; 4];
        self.read_at(p.offset + slot as u64 * 4, &mut bytes)?;
        let id = u32::from_le_bytes(bytes);
        if id as usize >= self.len() {
            return Err(Error::Corrupt("posting record id out of range".into()));
        }
        Ok(id)
    }
    fn contains(&self, p: Posting, id: u32) -> Result<bool> {
        // A sorted, unique posting covering all record IDs needs no disk probe.
        if p.count as usize == self.live_records {
            return Ok(true);
        }
        let (mut lo, mut hi) = (0, p.count);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.id_at(p, mid)?.cmp(&id) {
                std::cmp::Ordering::Equal => return Ok(true),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        Ok(false)
    }
    fn group_contains(&self, group: &[Posting], id: u32) -> Result<bool> {
        for &p in group {
            if self.contains(p, id)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn read_small_group(&self, group: &[Posting]) -> Result<Vec<u32>> {
        #[cfg(feature = "diagnostic-timing")]
        let _read = crate::timing::span("query.posting_cache_read");
        let mut ids = vec![];
        for &p in group {
            let mut bytes = vec![0; p.count as usize * 4];
            self.read_at(p.offset, &mut bytes)?;
            let mut previous = None;
            for chunk in bytes.as_chunks::<4>().0 {
                let id = u32::from_le_bytes(*chunk);
                if id as usize >= self.len() || previous.is_some_and(|old| old >= id) {
                    return Err(Error::Corrupt("invalid small posting list".into()));
                }
                previous = Some(id);
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    // Preserve the original local Vec/block loop for offset pages. Keeping
    // this path separate avoids collector overhead on the legacy fallback.
    fn query_terms(q: &Query) -> Vec<String> {
        q.text
            .split_whitespace()
            .map(|term| {
                let mut normalized = String::new();
                crate::index::fold_into(term, &mut normalized);
                normalized
            })
            .collect()
    }
    #[cfg(feature = "bounded-engine")]
    pub(crate) fn authorize_text_covering(&self) {
        self.covering.authorize();
    }
    #[cfg(feature = "bounded-engine")]
    pub(crate) fn bind_text_source(&mut self, path: PathBuf, lease: SourceLease) -> Result<()> {
        if self.source_references {
            self.payload_source = Some(lease.try_clone()?);
        }
        self.covering.bind_source(path, lease)
    }
    fn verification_terms<'a>(
        &self,
        q: &Query,
        terms: &'a [String],
    ) -> Result<std::borrow::Cow<'a, [String]>> {
        #[cfg(not(feature = "bounded-engine"))]
        let _ = q;
        #[cfg(feature = "bounded-engine")]
        if self.binary_payload
            && terms
                .iter()
                .any(|t| covering::exact_gram(t) || self.covering.short_covered(&q.scope, t))
            && self.covering.usable(&self.file)?
        {
            #[cfg(feature = "experimental-rkyv-payload")]
            if self.payload_benchmark.is_some() {
                return Ok(std::borrow::Cow::Borrowed(terms));
            }
            return Ok(std::borrow::Cow::Owned(
                terms
                    .iter()
                    .filter(|t| {
                        !covering::exact_gram(t) && !self.covering.short_covered(&q.scope, t)
                    })
                    .cloned()
                    .collect(),
            ));
        }
        Ok(std::borrow::Cow::Borrowed(terms))
    }
    #[cfg(feature = "experimental-streaming-open")]
    fn required_gram_absent(&self, q: &Query, terms: &[String]) -> bool {
        self.term_pages.as_ref().is_some_and(|pages| {
            terms.iter().any(|term| {
                grams(term)
                    .into_iter()
                    .any(|gram| !pages.may_contain(&Term::Gram(q.scope.clone(), gram)))
            })
        })
    }
    pub fn matches(&self, q: &Query, hidden: impl Fn(u32) -> bool) -> Result<Vec<u32>> {
        let terms = Self::query_terms(q);
        let verification_terms = self.verification_terms(q, &terms)?;
        #[cfg(feature = "experimental-streaming-open")]
        if self.required_gram_absent(q, &terms) {
            return Ok(Vec::new());
        }
        let mut groups = vec![match &q.collection {
            Some(c) => self.one(Term::Collection(q.scope.clone(), c.clone()))?,
            None => self.one(Term::Scope(q.scope.clone()))?,
        }];
        #[cfg(feature = "experimental-streaming-open")]
        if self.term_pages.is_some() && groups[0].is_empty() {
            return Ok(Vec::new());
        }
        let collections = self.collections(q);
        for (field, value) in &q.exact {
            let mut group = vec![];
            for c in &collections {
                group.extend(self.one(Term::Exact(
                    q.scope.clone(),
                    c.to_string(),
                    field.clone(),
                    value.to_string(),
                ))?);
            }
            groups.push(group);
        }
        for r in &q.ranges {
            let mut group = vec![];
            for c in &collections {
                let min = Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.min);
                let max = Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.max);
                group.extend(self.posting_range(min, max)?);
            }
            groups.push(group);
        }
        if let Some(key) = &q.linked_to {
            groups.push(self.one(Term::Link(key.clone()))?);
        }
        for term in &terms {
            for gram in grams(term) {
                let posting = self.one(Term::Gram(q.scope.clone(), gram))?;
                #[cfg(feature = "experimental-streaming-open")]
                if self.term_pages.is_some() && posting.is_empty() {
                    return Ok(Vec::new());
                }
                groups.push(posting);
            }
        }
        groups.sort_by_key(|g| g.iter().map(|p| p.count as u64).sum::<u64>());
        if groups[0].is_empty() {
            return Ok(vec![]);
        }
        // Query-local cache avoids repeated positional probes for small lists.
        // It is capped at 256 KiB and is released when this search returns.
        #[cfg(feature = "diagnostic-timing")]
        let mut work = crate::timing::QueryWork::default();
        let mut cache_budget = 256 * 1024usize;
        let mut cached = Vec::with_capacity(groups.len() - 1);
        for group in &groups[1..] {
            let count = group.iter().map(|p| p.count as usize).sum::<usize>();
            if count <= 4096 && count * 4 <= cache_budget && count < self.len() {
                cached.push(Some(self.read_small_group(group)?));
                cache_budget -= count * 4;
            } else {
                cached.push(None);
            }
        }
        // Query-local reuse of identical content; bounded to 256 KiB. Titles
        // remain independently verified so cross-record text cannot leak.
        let mut folded_content = BTreeMap::<String, String>::new();
        let mut folded_bytes = 0usize;
        #[cfg(feature = "experimental-grouped-read")]
        let mut text_scratch = QueryTextScratch {
            read_window_enabled: self.broad_payload_seed(&groups[0]),
            ..Default::default()
        };
        #[cfg(not(feature = "experimental-grouped-read"))]
        let mut text_scratch = QueryTextScratch::default();
        #[cfg(feature = "experimental-paged-metadata")]
        let mut text_cursor = paged::TextCursor::default();
        // Stream the smallest posting group in blocks; no full list hydration.
        let mut matched = vec![];
        let mut buffer = [0; 4096];
        for &p in &groups[0] {
            let mut slot = 0;
            let mut previous = None;
            while slot < p.count {
                let count = (p.count - slot).min((buffer.len() / 4) as u32);
                self.read_at(
                    p.offset + slot as u64 * 4,
                    &mut buffer[..count as usize * 4],
                )?;
                for bytes in buffer[..count as usize * 4].as_chunks::<4>().0 {
                    let id = u32::from_le_bytes(*bytes);
                    if id as usize >= self.len() || previous.is_some_and(|old| old >= id) {
                        return Err(Error::Corrupt("invalid sorted posting list".into()));
                    }
                    previous = Some(id);
                    if hidden(id) || self.deleted(id) {
                        continue;
                    }
                    #[cfg(feature = "diagnostic-timing")]
                    let intersection_start = std::time::Instant::now();
                    let mut include = true;
                    for (group, cached) in groups[1..].iter().zip(&cached) {
                        let found = if let Some(ids) = cached {
                            ids.binary_search(&id).is_ok()
                        } else {
                            self.group_contains(group, id)?
                        };
                        if !found {
                            include = false;
                            break;
                        }
                    }
                    #[cfg(feature = "diagnostic-timing")]
                    {
                        work.visible += 1;
                        work.intersection_ns += intersection_start.elapsed().as_nanos() as u64;
                    }
                    if include && verification_terms.len() != terms.len() {
                        #[cfg(feature = "diagnostic-timing")]
                        {
                            work.covered += 1;
                        }
                    }
                    if include && !verification_terms.is_empty() {
                        #[cfg(feature = "diagnostic-timing")]
                        {
                            work.candidates += 1;
                        }
                        #[cfg(feature = "experimental-rkyv-payload")]
                        let overlay_match = self.benchmark_matches(
                            id,
                            &mut text_scratch,
                            &verification_terms,
                            &mut folded_content,
                            &mut folded_bytes,
                        )?;
                        #[cfg(not(feature = "experimental-rkyv-payload"))]
                        let overlay_match: Option<bool> = None;
                        if let Some(result) = overlay_match {
                            include = result;
                        } else if self.binary_payload {
                            include = self.binary_matches(
                                id,
                                &mut text_scratch,
                                &verification_terms,
                                &mut folded_content,
                                &mut folded_bytes,
                            )?;
                        } else {
                            #[cfg(feature = "experimental-paged-metadata")]
                            let d = self.read_text_document(id, &mut text_cursor)?.document;
                            #[cfg(not(feature = "experimental-paged-metadata"))]
                            let d = self.read_document(id)?.document;
                            include = text_scratch.matches(
                                &d,
                                &verification_terms,
                                &mut folded_content,
                                &mut folded_bytes,
                            );
                        }
                    }
                    if include {
                        #[cfg(feature = "diagnostic-timing")]
                        {
                            work.accepted += 1;
                        }
                        matched.push(id);
                    }
                }
                slot += count;
            }
        }
        matched.sort_unstable();
        matched.dedup();
        Ok(matched)
    }

    pub fn visible_count(&self, mask: &[u64]) -> Result<usize> {
        let mut hidden_live = 0;
        for (word, &bits) in mask.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let id = word * 64 + bits.trailing_zeros() as usize;
                if id < self.len() && !self.deleted(id as u32) {
                    hidden_live += 1;
                }
                bits &= bits - 1;
            }
        }
        self.live_records
            .checked_sub(hidden_live)
            .ok_or_else(|| Error::Corrupt("invalid live visibility count".into()))
    }

    /// Exact total and a bounded prefix of IDs in key order. A per-run prefix
    /// of offset + limit contains every candidate for that global page.
    pub fn prefix_matches(
        &self,
        q: &Query,
        mask: &[u64],
        limit: usize,
    ) -> Result<(usize, Vec<u32>)> {
        self.prefix_matches_with_capture(q, mask, limit, &mut |_, _, _| {})
    }

    #[cfg(feature = "bounded-engine")]
    pub(crate) fn prefix_matches_checked(
        &self,
        q: &Query,
        mask: &[u64],
        limit: usize,
        run: usize,
        page: &mut CheckedPage,
    ) -> Result<(usize, Vec<u32>)> {
        self.prefix_matches_with_capture(q, mask, limit, &mut |id, view, bytes| {
            page.retain(run, id, view, bytes);
        })
    }

    fn prefix_matches_with_capture(
        &self,
        q: &Query,
        mask: &[u64],
        limit: usize,
        capture: &mut impl FnMut(u32, &crate::payload::View<'_>, &[u8]),
    ) -> Result<(usize, Vec<u32>)> {
        let visible = self.visible_count(mask)?;
        let hidden = |id: u32| mask[id as usize / 64] & (1u64 << (id % 64)) != 0;
        let mut ids = BinaryHeap::with_capacity(limit.min(self.len()));
        let mut group = None;
        let mut capture_ids = true;
        // Within a union, collection/scalar-value postings are disjoint by
        // construction. IDs in each posting are increasing; after an ID exceeds
        // our largest retained ID the rest of that posting cannot improve it.
        let total = self.visit_matches_with_capture::<true>(
            q,
            hidden,
            Some(visible),
            limit,
            |id, n| {
                if group != Some(n) {
                    group = Some(n);
                    capture_ids = true;
                }
                if !capture_ids {
                    return;
                }
                if ids.len() < limit {
                    ids.push(id);
                } else if id < *ids.peek().unwrap() {
                    *ids.peek_mut().unwrap() = id;
                } else {
                    capture_ids = false;
                }
            },
            capture,
        )?;
        Ok((total, ids.into_sorted_vec()))
    }

    fn visit_matches_with_capture<const COUNT: bool>(
        &self,
        q: &Query,
        hidden: impl Fn(u32) -> bool,
        visible_total: Option<usize>,
        capture_limit: usize,
        mut emit: impl FnMut(u32, usize),
        capture: &mut impl FnMut(u32, &crate::payload::View<'_>, &[u8]),
    ) -> Result<usize> {
        let terms = Self::query_terms(q);
        let verification_terms = self.verification_terms(q, &terms)?;
        #[cfg(feature = "experimental-streaming-open")]
        if self.required_gram_absent(q, &terms) {
            return Ok(0);
        }
        let mut groups = vec![match &q.collection {
            Some(c) => self.one(Term::Collection(q.scope.clone(), c.clone()))?,
            None => self.one(Term::Scope(q.scope.clone()))?,
        }];
        #[cfg(feature = "experimental-streaming-open")]
        if self.term_pages.is_some() && groups[0].is_empty() {
            return Ok(0);
        }
        let collections = self.collections(q);
        for (field, value) in &q.exact {
            let mut group = vec![];
            for c in &collections {
                group.extend(self.one(Term::Exact(
                    q.scope.clone(),
                    c.to_string(),
                    field.clone(),
                    value.to_string(),
                ))?);
            }
            groups.push(group);
        }
        for r in &q.ranges {
            let mut group = vec![];
            for c in &collections {
                let min = Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.min);
                let max = Term::Numeric(q.scope.clone(), c.to_string(), r.field.clone(), r.max);
                group.extend(self.posting_range(min, max)?);
            }
            groups.push(group);
        }
        if let Some(key) = &q.linked_to {
            groups.push(self.one(Term::Link(key.clone()))?);
        }
        for term in &terms {
            for gram in grams(term) {
                let posting = self.one(Term::Gram(q.scope.clone(), gram))?;
                #[cfg(feature = "experimental-streaming-open")]
                if self.term_pages.is_some() && posting.is_empty() {
                    return Ok(0);
                }
                groups.push(posting);
            }
        }
        groups.sort_by_key(|g| g.iter().map(|p| p.count as u64).sum::<u64>());
        if groups[0].is_empty() {
            return Ok(0);
        }
        // As with contains(), a posting covering every live record is a proven
        // full filter for a valid derived index. Visibility is counted from the
        // mask, excluding tombstones, so no stale or deleted row enters total.
        // Complete trigrams may use this only after source authorization;
        // other text still requires candidate verification.
        let fast_total = visible_total.filter(|_| {
            verification_terms.is_empty()
                && groups
                    .iter()
                    .all(|g| g.iter().any(|p| p.count as usize == self.live_records))
        });
        if fast_total == Some(0) {
            return Ok(0);
        }
        // Query-local cache avoids repeated positional probes for small lists.
        // It is capped at 256 KiB and is released when this search returns.
        #[cfg(feature = "diagnostic-timing")]
        let mut work = crate::timing::QueryWork::default();
        let mut cache_budget = 256 * 1024usize;
        let mut cached = Vec::with_capacity(groups.len() - 1);
        for group in &groups[1..] {
            let count = group.iter().map(|p| p.count as usize).sum::<usize>();
            if count <= 4096 && count * 4 <= cache_budget && count < self.len() {
                cached.push(Some(self.read_small_group(group)?));
                cache_budget -= count * 4;
            } else {
                cached.push(None);
            }
        }
        // Query-local reuse of identical content; bounded to 256 KiB. Titles
        // remain independently verified so cross-record text cannot leak.
        let mut folded_content = BTreeMap::<String, String>::new();
        let mut folded_bytes = 0usize;
        #[cfg(feature = "experimental-grouped-read")]
        let mut text_scratch = QueryTextScratch {
            read_window_enabled: self.broad_payload_seed(&groups[0]),
            ..Default::default()
        };
        #[cfg(not(feature = "experimental-grouped-read"))]
        let mut text_scratch = QueryTextScratch::default();
        #[cfg(feature = "experimental-paged-metadata")]
        let mut text_cursor = paged::TextCursor::default();
        // Stream the smallest posting group in blocks; no full list hydration.
        let mut total = 0;
        let mut buffer = [0; 4096];
        for (n, &p) in groups[0].iter().enumerate() {
            let mut slot = 0;
            let mut previous = None;
            while slot < p.count {
                let count = (p.count - slot).min((buffer.len() / 4) as u32);
                self.read_at(
                    p.offset + slot as u64 * 4,
                    &mut buffer[..count as usize * 4],
                )?;
                for bytes in buffer[..count as usize * 4].as_chunks::<4>().0 {
                    let id = u32::from_le_bytes(*bytes);
                    if id as usize >= self.len() || previous.is_some_and(|old| old >= id) {
                        return Err(Error::Corrupt("invalid sorted posting list".into()));
                    }
                    previous = Some(id);
                    if hidden(id) || self.deleted(id) {
                        continue;
                    }
                    #[cfg(feature = "diagnostic-timing")]
                    let intersection_start = std::time::Instant::now();
                    let mut include = true;
                    for (group, cached) in groups[1..].iter().zip(&cached) {
                        let found = if let Some(ids) = cached {
                            ids.binary_search(&id).is_ok()
                        } else {
                            self.group_contains(group, id)?
                        };
                        if !found {
                            include = false;
                            break;
                        }
                    }
                    #[cfg(feature = "diagnostic-timing")]
                    {
                        work.visible += 1;
                        work.intersection_ns += intersection_start.elapsed().as_nanos() as u64;
                    }
                    if include && verification_terms.len() != terms.len() {
                        #[cfg(feature = "diagnostic-timing")]
                        {
                            work.covered += 1;
                        }
                    }
                    if include && !verification_terms.is_empty() {
                        #[cfg(feature = "diagnostic-timing")]
                        {
                            work.candidates += 1;
                        }
                        #[cfg(feature = "experimental-rkyv-payload")]
                        let overlay_match = self.benchmark_matches(
                            id,
                            &mut text_scratch,
                            &verification_terms,
                            &mut folded_content,
                            &mut folded_bytes,
                        )?;
                        #[cfg(not(feature = "experimental-rkyv-payload"))]
                        let overlay_match: Option<bool> = None;
                        if let Some(result) = overlay_match {
                            include = result;
                        } else if self.binary_payload {
                            include = self.binary_matches_with_capture(
                                id,
                                &mut text_scratch,
                                &verification_terms,
                                &mut folded_content,
                                &mut folded_bytes,
                                capture,
                            )?;
                        } else {
                            #[cfg(feature = "experimental-paged-metadata")]
                            let d = self.read_text_document(id, &mut text_cursor)?.document;
                            #[cfg(not(feature = "experimental-paged-metadata"))]
                            let d = self.read_document(id)?.document;
                            include = text_scratch.matches(
                                &d,
                                &verification_terms,
                                &mut folded_content,
                                &mut folded_bytes,
                            );
                        }
                    }
                    if include {
                        #[cfg(feature = "diagnostic-timing")]
                        {
                            work.accepted += 1;
                        }
                        if COUNT {
                            total += 1;
                        }
                        emit(id, n);
                        if let Some(count) = fast_total
                            && total >= capture_limit
                        {
                            return Ok(count);
                        }
                    }
                }
                slot += count;
            }
        }
        Ok(fast_total.unwrap_or(total))
    }
}

#[cfg(feature = "experimental-merge-postings")]
enum TermCursor<'a> {
    Memory(std::collections::btree_map::Iter<'a, Term, Posting>),
    #[cfg(feature = "experimental-streaming-open")]
    Pages(term_pages::Cursor<'a>),
}
#[cfg(feature = "experimental-merge-postings")]
impl TermCursor<'_> {
    fn next(&mut self) -> Result<Option<(Term, Posting)>> {
        match self {
            Self::Memory(values) => Ok(values.next().map(|(t, p)| (t.clone(), *p))),
            #[cfg(feature = "experimental-streaming-open")]
            Self::Pages(values) => values.next(),
        }
    }
}
