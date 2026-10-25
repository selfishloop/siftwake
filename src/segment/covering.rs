//! Query coverage is authorized only after the engine has checked its source.
use super::*;
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;

// Acquire before validation, then retain the same handle with the index. Only
// immutable runs qualify: legacy sources can also be the writable active WAL.
pub(crate) struct SourceLease {
    file: File,
    pinned: bool,
}
impl SourceLease {
    pub(crate) fn immutable(path: &Path) -> Result<Self> {
        Self::open(path, true)
    }
    pub(crate) fn open(path: &Path, immutable: bool) -> Result<Self> {
        #[cfg(windows)]
        if immutable && plain_path(path)? {
            use std::os::windows::fs::OpenOptionsExt;
            let file = OpenOptions::new().read(true).share_mode(1).open(path)?;
            // Reparse paths retain the old metadata guard. Do not authorize a
            // path alias merely because its target has an immutable handle.
            let pinned = plain_path(path)?;
            return Ok(Self { file, pinned });
        }
        let _ = immutable;
        Ok(Self {
            file: File::open(path)?,
            pinned: false,
        })
    }
}
impl std::ops::Deref for SourceLease {
    type Target = File;
    fn deref(&self) -> &File {
        &self.file
    }
}
impl std::ops::DerefMut for SourceLease {
    fn deref_mut(&mut self) -> &mut File {
        &mut self.file
    }
}
#[cfg(windows)]
fn plain_path(path: &Path) -> Result<bool> {
    use std::os::windows::fs::MetadataExt;
    let path = std::path::absolute(path)?;
    if path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Ok(false);
    }
    for part in path.ancestors() {
        if std::fs::symlink_metadata(part)?.file_attributes() & 0x400 != 0 {
            return Ok(false);
        }
    }
    Ok(true)
}
struct Source {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
    // Windows share-read denies writers, truncation, deletion and rename for
    // this handle's lifetime. Arc<DiskIndex> also carries it into merge workers.
    pinned: Option<File>,
}
pub(super) struct Covering {
    authorized: AtomicBool,
    len: u64,
    modified: Option<SystemTime>,
    source: Option<Source>,
    universal: Universal,
}
impl Covering {
    pub fn new(metadata: &std::fs::Metadata) -> Self {
        Self {
            authorized: AtomicBool::new(false),
            len: metadata.len(),
            modified: metadata.modified().ok(),
            source: None,
            universal: Universal::default(),
        }
    }
    pub fn with_grams(mut self, universal: Universal) -> Self {
        self.universal = universal;
        self
    }
    pub fn short_covered(&self, scope: &str, term: &str) -> bool {
        self.universal.covers(scope, term)
    }
    pub fn root_bytes(&self) -> usize {
        self.universal.root_bytes()
    }
    pub fn bind_source(&mut self, path: PathBuf, lease: SourceLease) -> Result<()> {
        let metadata = lease.file.metadata()?;
        self.source = Some(Source {
            path,
            len: metadata.len(),
            modified: metadata.modified().ok(),
            pinned: lease.pinned.then_some(lease.file),
        });
        Ok(())
    }
    pub fn authorize(&self) {
        self.authorized.store(true, Ordering::Release);
    }
    pub fn usable(&self, file: &File) -> Result<bool> {
        #[cfg(feature = "diagnostic-timing")]
        let _guard = crate::timing::span("query.covering_guard");
        if !self.authorized.load(Ordering::Acquire) || self.modified.is_none() {
            return Ok(false);
        }
        let Some(source) = &self.source else {
            return Ok(false);
        };
        if source.pinned.is_none() {
            let Some(modified) = source.modified else {
                return Ok(false);
            };
            let metadata = {
                #[cfg(feature = "diagnostic-timing")]
                let _source = crate::timing::span("query.covering_source_metadata");
                std::fs::metadata(&source.path)?
            };
            let Ok(current) = metadata.modified() else {
                return Ok(false);
            };
            if metadata.len() != source.len || current != modified {
                return Err(Error::Corrupt(
                    "covered source changed since bound open".into(),
                ));
            }
        }
        let metadata = {
            #[cfg(feature = "diagnostic-timing")]
            let _index = crate::timing::span("query.covering_index_metadata");
            file.metadata()?
        };
        let Ok(modified) = metadata.modified() else {
            return Ok(false);
        };
        if metadata.len() != self.len || Some(modified) != self.modified {
            return Err(Error::Corrupt(
                "covered index changed since validated open".into(),
            ));
        }
        Ok(true)
    }
}

// Gram postings are exact substring evidence only for one complete trigram.
// Count Unicode scalars after folding; byte length and word boundaries differ.
pub(super) fn exact_gram(term: &str) -> bool {
    let mut chars = term.chars();
    (0..3).all(|_| chars.next().is_some_and(|c| !c.is_whitespace())) && chars.next().is_none()
}

// A full-live posting proves every substring of its gram in that scope.
// This is positive evidence only: isolated short text has no gram at all.
// Keep a bounded summary while parsing the existing directory, never full text.
#[derive(Default)]
pub(super) struct Universal {
    grams: Vec<(Box<str>, Box<str>)>,
    string_bytes: usize,
}
impl Universal {
    const ENTRIES: usize = 64;
    const BYTES: usize = 16 * 1024;
    pub fn observe(&mut self, term: &Term, count: u32, live: usize) {
        if live == 0 || count as usize != live || self.grams.len() == Self::ENTRIES {
            return;
        }
        let Term::Gram(scope, gram) = term else {
            return;
        };
        if !exact_gram(gram)
            || Self::ENTRIES * std::mem::size_of::<(Box<str>, Box<str>)>()
                + self.string_bytes
                + scope.len()
                + gram.len()
                > Self::BYTES
        {
            return;
        }
        if self.grams.is_empty() {
            self.grams.reserve_exact(Self::ENTRIES);
        }
        self.string_bytes += scope.len() + gram.len();
        self.grams
            .push((scope.as_str().into(), gram.as_str().into()));
    }
    fn covers(&self, scope: &str, term: &str) -> bool {
        let mut chars = term.chars();
        if !chars.next().is_some_and(|c| !c.is_whitespace())
            || !chars.next().is_some_and(|c| !c.is_whitespace())
            || chars.next().is_some()
        {
            return false;
        }
        self.grams
            .iter()
            .any(|(s, gram)| s.as_ref() == scope && gram.contains(term))
    }
    fn root_bytes(&self) -> usize {
        self.grams.capacity() * std::mem::size_of::<(Box<str>, Box<str>)>() + self.string_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::sync::Arc;
    #[cfg(windows)]
    #[test]
    fn immutable_handle_lives_until_last_arc_and_existing_writers_are_rejected() {
        struct Temp(PathBuf);
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let temp = Temp(std::env::temp_dir().join(format!(
            "fdb-source-lease-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()
        )));
        std::fs::write(&temp.0, b"source").unwrap();
        let lease = Arc::new(SourceLease::open(&temp.0, true).unwrap());
        assert!(lease.pinned);
        let worker = Arc::clone(&lease);
        drop(lease);
        assert!(OpenOptions::new().write(true).open(&temp.0).is_err());
        drop(worker);
        let writer = OpenOptions::new().write(true).open(&temp.0).unwrap();
        assert!(matches!(
            SourceLease::open(&temp.0, true),
            Err(Error::Io(_))
        ));
        drop(writer);
        let fallback = SourceLease::open(&temp.0, false).unwrap();
        assert!(!fallback.pinned);
        assert!(OpenOptions::new().write(true).open(&temp.0).is_ok());
    }
    #[cfg(windows)]
    #[test]
    fn junction_ancestor_keeps_metadata_guard_instead_of_trusting_path_alias() {
        struct Temp(PathBuf);
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let temp =
            Temp(std::env::temp_dir().join(format!("fdb-source-junction-{}", std::process::id())));
        let target = temp.0.join("target");
        let alias = temp.0.join("alias");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("source"), b"source").unwrap();
        let result = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&alias)
            .arg(&target)
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        let path = alias.join("source");
        let lease = SourceLease::open(&path, true).unwrap();
        assert!(!lease.pinned);
        assert!(OpenOptions::new().write(true).open(&path).is_ok());
    }
    #[test]
    fn complete_gram_is_unicode_scalar_based() {
        for text in ["the", "man", "東京🦀", "éçü", "a\u{0301}x"] {
            assert!(exact_gram(text));
            assert_eq!(grams(text), BTreeSet::from([text.to_owned()]));
        }
        for text in ["", "a", "he", "four", "a b", "a\nβ", " a "] {
            assert!(!exact_gram(text));
        }
    }
    #[test]
    fn short_proof_is_positive_scoped_complete_and_bounded() {
        let mut proof = Universal::default();
        for (gram, count) in [("the", 3), ("東京🦀", 3), ("xyz", 2), ("bad!", 3)] {
            proof.observe(&Term::Gram("s".into(), gram.into()), count, 3);
        }
        for term in ["he", "th", "東京", "京🦀"] {
            assert!(proof.covers("s", term));
            assert!(!proof.covers("other", term));
        }
        for term in ["xy", "ba", "zx", "h", "the", "", "h e"] {
            assert!(!proof.covers("s", term));
        }
        for n in 0..1000 {
            proof.observe(&Term::Gram("s".into(), format!("{n:03}")), 3, 3);
            assert!(proof.root_bytes() <= Universal::BYTES);
            assert!(proof.grams.len() <= Universal::ENTRIES);
        }
        let before = proof.root_bytes();
        proof.observe(&Term::Gram("s".into(), "end".into()), 3, 3);
        assert_eq!(proof.root_bytes(), before);
        let mut empty = Universal::default();
        empty.observe(
            &Term::Gram("s".repeat(Universal::BYTES), "the".into()),
            1,
            1,
        );
        empty.observe(&Term::Gram("s".into(), "the".into()), 0, 0);
        assert_eq!(empty.root_bytes(), 0);
    }
}
