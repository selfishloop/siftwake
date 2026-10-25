//! Default native SEG05 cache and explicit layout conversion. Sources are unchanged.
//! Directory integers are little endian; strings are u32-length UTF-8. No
//! serializer ABI or Rust enum layout is persisted. Flag 1 addresses FDBLOG04.
use super::*;
use std::io::{BufReader, Read};

pub(super) const MAGIC: &[u8; 8] = b"FDBSEG05";
pub(super) const FOOTER: &[u8; 8] = b"FDBEND05";
const DIRECTORY: &[u8; 8] = b"FDBDIC01";

fn bad() -> Error {
    Error::Corrupt("SEG05 directory bounds/schema/order".into())
}
fn string(output: &mut impl Write, value: &str) -> Result<()> {
    let len = u32::try_from(value.len()).map_err(|_| bad())?;
    output.write_all(&len.to_le_bytes())?;
    output.write_all(value.as_bytes())?;
    Ok(())
}
fn key(output: &mut impl Write, value: &Key) -> Result<()> {
    for part in [&value.scope, &value.collection, &value.id] {
        string(output, part)?;
    }
    Ok(())
}
pub(super) fn write_directory_header(
    output: &mut impl Write,
    references: bool,
    records: usize,
    postings: usize,
) -> Result<()> {
    let records = u32::try_from(records).map_err(|_| bad())?;
    let postings = u32::try_from(postings).map_err(|_| bad())?;
    output.write_all(DIRECTORY)?;
    output.write_all(&[u8::from(references)])?;
    output.write_all(&records.to_le_bytes())?;
    output.write_all(&postings.to_le_bytes())?;
    Ok(())
}
pub(super) fn write_record(output: &mut impl Write, r: &Record) -> Result<()> {
    key(output, &r.key)?;
    output.write_all(&r.offset.to_le_bytes())?;
    output.write_all(&r.len.to_le_bytes())?;
    output.write_all(&r.checksum.to_le_bytes())?;
    output.write_all(&r.version.to_le_bytes())?;
    output.write_all(&[u8::from(r.deleted)])?;
    Ok(())
}
pub(super) fn write_posting(output: &mut impl Write, term: &Term, p: Posting) -> Result<()> {
    let (tag, parts): (u8, Vec<&str>) = match term {
        Term::Scope(s) => (0, vec![s]),
        Term::Collection(s, c) => (1, vec![s, c]),
        Term::Exact(s, c, f, v) => (2, vec![s, c, f, v]),
        Term::Numeric(s, c, f, _) => (3, vec![s, c, f]),
        Term::Gram(s, g) => (4, vec![s, g]),
        Term::Link(k) => (5, vec![&k.scope, &k.collection, &k.id]),
    };
    output.write_all(&[tag])?;
    for part in parts {
        string(output, part)?;
    }
    if let Term::Numeric(_, _, _, value) = term {
        output.write_all(&value.to_le_bytes())?;
    }
    output.write_all(&p.offset.to_le_bytes())?;
    output.write_all(&p.count.to_le_bytes())?;
    Ok(())
}

impl Builder {
    /// Posting reuse: fully checked record boundaries without term extraction.
    pub(crate) fn from_native_records(
        target: &Path,
        file: &mut File,
        end: u64,
        checksum: u32,
    ) -> Result<(Self, u64)> {
        Self::from_native(target, file, end, checksum, false)
    }
    /// Build references only from checked native entry boundaries. Nothing is
    /// published until the visitor has checked the complete source/frame CRCs.
    pub(crate) fn from_native(
        target: &Path,
        file: &mut File,
        end: u64,
        checksum: u32,
        index_terms: bool,
    ) -> Result<(Self, u64)> {
        let mut builder = None;
        let (sequence, count) =
            crate::format::visit_native_prefix(file, end, checksum, |offset, seq, tag, bytes| {
                if builder.is_none() {
                    builder = Some(Self::new_layout(target, seq, end, checksum, true)?);
                }
                let b = builder.as_mut().unwrap();
                if tag == 0 {
                    let view = crate::payload::checked(bytes)?;
                    let (scope, collection, id) = view.key_parts();
                    let key = Key::new(scope, collection, id);
                    if b.last_record_key().is_some_and(|last| last >= &key) {
                        return Err(Error::Corrupt(
                            "checkpoint keys must be strictly sorted".into(),
                        ));
                    }
                    let id = u32::try_from(b.record_count()).map_err(|_| {
                        Error::Invalid("segment exceeds u32 record capacity".into())
                    })?;
                    b.push_record(Record {
                        key,
                        offset,
                        len: u32::try_from(bytes.len()).map_err(|_| bad())?,
                        checksum: crc(bytes),
                        version: view.version(),
                        deleted: false,
                    })?;
                    // Posting reuse needs only key/version/address metadata,
                    // so it never materializes title/text/fields/links here.
                    if index_terms {
                        b.index_document(&view.to_document()?, id)?;
                    }
                } else if let crate::format::Change::Delete { key } =
                    crate::binary_batch::decode_entry(tag, bytes)?
                {
                    b.add_delete(&key)?;
                }
                Ok(())
            })?;
        let builder = match builder {
            Some(builder) => builder,
            None => Self::new_layout(target, sequence, end, checksum, true)?,
        };
        if builder.record_count() != count {
            return Err(bad());
        }
        Ok((builder, sequence))
    }
}
struct Input<R> {
    reader: R,
    position: u64,
    end: u64,
}
impl<R: Read> Input<R> {
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N]> {
        if N as u64 > self.end - self.position {
            return Err(bad());
        }
        let mut bytes = [0; N];
        self.reader.read_exact(&mut bytes).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                bad()
            } else {
                Error::Io(e)
            }
        })?;
        self.position += N as u64;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.fixed::<1>()?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
    fn string(&mut self, max: u64) -> Result<String> {
        let len = self.u32()? as u64;
        if len > max || len > self.end - self.position {
            return Err(bad());
        }
        let mut bytes = vec![0; len as usize];
        self.reader.read_exact(&mut bytes).map_err(|_| bad())?;
        self.position += len;
        String::from_utf8(bytes).map_err(|_| bad())
    }
    fn key(&mut self) -> Result<Key> {
        Ok(Key {
            scope: self.string(1024)?,
            collection: self.string(1024)?,
            id: self.string(1024)?,
        })
    }
    fn record(&mut self) -> Result<Record> {
        let key = self.key()?;
        let offset = self.u64()?;
        let len = self.u32()?;
        let checksum = self.u32()?;
        let version = self.u64()?;
        let deleted = match self.byte()? {
            0 => false,
            1 => true,
            _ => return Err(bad()),
        };
        Ok(Record {
            key,
            offset,
            len,
            checksum,
            version,
            deleted,
        })
    }
    fn posting(&mut self) -> Result<(Term, Posting)> {
        let term = match self.byte()? {
            0 => Term::Scope(self.string(1024)?),
            1 => Term::Collection(self.string(1024)?, self.string(1024)?),
            2 => Term::Exact(
                self.string(1024)?,
                self.string(1024)?,
                self.string(256)?,
                self.string(MAX_DIRECTORY)?,
            ),
            3 => Term::Numeric(
                self.string(1024)?,
                self.string(1024)?,
                self.string(256)?,
                i64::from_le_bytes(self.fixed()?),
            ),
            4 => Term::Gram(self.string(1024)?, self.string(MAX_DIRECTORY)?),
            5 => Term::Link(self.key()?),
            _ => return Err(bad()),
        };
        Ok((
            term,
            Posting {
                offset: self.u64()?,
                count: self.u32()?,
            },
        ))
    }
}
pub(super) fn flags(file: &File, offset: u64) -> Result<bool> {
    let mut bytes = [0; 9];
    read_at(file, offset, &mut bytes)?;
    if &bytes[..8] != DIRECTORY || bytes[8] > 1 {
        return Err(bad());
    }
    Ok(bytes[8] == 1)
}
pub(super) fn read(
    file: &mut File,
    offset: u64,
    len: u64,
    sequence: u64,
    mut accept: impl FnMut(&Record, &mut streaming::Parsed) -> Result<()>,
) -> Result<streaming::Parsed> {
    #[cfg(feature = "diagnostic-timing")]
    let _span = crate::timing::span("open.index_binary_directory_parse");
    let refs = flags(file, offset)?;
    let mut header = [0; HEADER_SIZE as usize];
    read_at(file, 0, &mut header)?;
    let source_end = u64::from_le_bytes(header[16..24].try_into().unwrap());
    file.seek(SeekFrom::Start(offset))?;
    let mut input = Input {
        reader: BufReader::with_capacity(64 * 1024, file.take(len)),
        position: 0,
        end: len,
    };
    if &input.fixed::<8>()? != DIRECTORY || input.byte()? != u8::from(refs) {
        return Err(bad());
    }
    let count = input.u32()? as usize;
    let terms = input.u32()? as usize;
    if count as u64 > len / 40 || terms as u64 > len / 17 {
        return Err(bad());
    }
    let mut parsed = streaming::empty();
    let mut cursor = HEADER_SIZE;
    let mut source_cursor = 42;
    let mut previous = None;
    for _ in 0..count {
        let r = input.record()?;
        crate::validate_key(&r.key).map_err(|_| bad())?;
        if r.deleted != (r.len == 0)
            || r.len > 4 * 1024 * 1024 + 128
            || r.version == 0
            || r.version > sequence
            || previous.as_ref().is_some_and(|key| key >= &r.key)
        {
            return Err(bad());
        }
        if refs {
            if r.deleted {
                if r.offset != 0 || r.checksum != 0 {
                    return Err(bad());
                }
            } else {
                if r.offset < source_cursor {
                    return Err(bad());
                }
                source_cursor = r
                    .offset
                    .checked_add(r.len as u64)
                    .filter(|end| *end <= source_end)
                    .ok_or_else(bad)?;
            }
        } else {
            if r.offset != cursor {
                return Err(bad());
            }
            cursor = cursor.checked_add(r.len as u64).ok_or_else(bad)?;
        }
        if !r.deleted {
            parsed.live += 1;
        }
        accept(&r, &mut parsed)?;
        previous = Some(r.key);
    }
    let mut previous = None;
    for _ in 0..terms {
        let start = input.position;
        let (term, p) = input.posting()?;
        if p.count == 0
            || p.count as usize > count
            || p.offset != cursor
            || previous.as_ref().is_some_and(|t| t >= &term)
        {
            return Err(bad());
        }
        cursor = cursor.checked_add(p.count as u64 * 4).ok_or_else(bad)?;
        if cursor > offset {
            return Err(bad());
        }
        parsed.entries += p.count as u64;
        parsed.universal.observe(&term, p.count, parsed.live);
        if let Term::Collection(s, c) = &term {
            parsed.collections.push((s.clone(), c.clone()));
        }
        parsed.bloom.add(&term);
        term_pages::push_fence(
            &mut parsed.pages,
            &term,
            offset + start,
            offset + input.position,
            p.count,
        )?;
        previous = Some(term);
    }
    if input.position != len || cursor != offset {
        return Err(bad());
    }
    parsed.records.shrink_to_fit();
    parsed.ids.shrink_to_fit();
    Ok(parsed)
}
pub(super) fn decode_page(bytes: &[u8], count: usize) -> Result<Vec<(Term, Posting)>> {
    let mut input = Input {
        reader: bytes,
        position: 0,
        end: bytes.len() as u64,
    };
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        result.push(input.posting()?);
    }
    if input.position != input.end {
        return Err(bad());
    }
    Ok(result)
}

fn copy(file: &File, output: &mut impl Write, start: u64, end: u64) -> Result<()> {
    if start > end {
        return Err(bad());
    }
    let mut buffer = vec![0; (end - start).min(1024 * 1024) as usize];
    let mut position = start;
    while position < end {
        let n = buffer.len().min((end - position) as usize);
        read_at(file, position, &mut buffer[..n])?;
        output.write_all(&buffer[..n])?;
        position += n as u64;
    }
    Ok(())
}

impl DiskIndex {
    /// Rewrite only a disposable cache. The caller must close/reopen its DB;
    /// the current index handle and query cache continue to refer to old bytes.
    pub(crate) fn repack(&self, path: &Path, layout: crate::IndexCacheLayout) -> Result<()> {
        use crate::IndexCacheLayout as Layout;
        let refs = layout == Layout::BinarySourceReferences;
        if !self.binary_payload {
            return Err(Error::Invalid("SEG05 requires native payloads".into()));
        }
        let records = u32::try_from(self.len())
            .map_err(|_| Error::Invalid("SEG05 record capacity".into()))?;
        self.verify_repack_source(path)?;
        let temporary = path.with_extension(format!(
            "repack-{}-{}.tmp",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            let mut writer = BufWriter::with_capacity(64 * 1024, file);
            let mut header = [0; HEADER_SIZE as usize];
            read_at(&self.file, 0, &mut header)?;
            header[..8].copy_from_slice(if layout == Layout::JsonPayloadCopy {
                super::MAGIC
            } else {
                MAGIC
            });
            writer.write_all(&header)?;
            let mut offset = HEADER_SIZE;
            if !refs {
                for id in 0..records {
                    let r = self.metadata(id)?;
                    copy(
                        self.payload_file()?,
                        &mut writer,
                        r.offset,
                        r.offset + r.len as u64,
                    )?;
                    offset += r.len as u64;
                }
            }
            let postings_start = offset;
            let mut terms = self.term_cursor();
            let mut count = 0u32;
            let mut old_start = None;
            let mut old_end = 0;
            while let Some((_, p)) = terms.next()? {
                old_start.get_or_insert(p.offset);
                old_end = p.offset + p.count as u64 * 4;
                count = count.checked_add(1).ok_or_else(bad)?;
            }
            if let Some(start) = old_start {
                copy(&self.file, &mut writer, start, old_end)?;
                offset += old_end - start;
            }
            let mut sink = crate::bounded::Sink::new(&mut writer, MAX_DIRECTORY as usize);
            if layout == Layout::JsonPayloadCopy {
                sink.write_all(b"{\"records\":[")?;
            } else {
                sink.write_all(DIRECTORY)?;
                sink.write_all(&[u8::from(refs)])?;
                sink.write_all(&records.to_le_bytes())?;
                sink.write_all(&count.to_le_bytes())?;
            }
            let mut slot = 0u32;
            let mut payload_offset = HEADER_SIZE;
            let mut source = File::open(path.with_extension("fdb"))?;
            let end = u64::from_le_bytes(header[16..24].try_into().unwrap());
            let checksum = u32::from_le_bytes(header[24..28].try_into().unwrap());
            let (seq, keys) = crate::format::visit_native_prefix(
                &mut source,
                end,
                checksum,
                |position, seq, tag, bytes| {
                    let mut r = self.record(slot)?;
                    if tag == 0 {
                        let view = crate::payload::checked(bytes)?;
                        if view.key_parts()
                            != (
                                r.key.scope.as_str(),
                                r.key.collection.as_str(),
                                r.key.id.as_str(),
                            )
                            || view.version() != r.version
                            || r.deleted
                            || r.len as usize != bytes.len()
                            || r.checksum != crc(bytes)
                        {
                            return Err(bad());
                        }
                    } else {
                        let crate::format::Change::Delete { key } =
                            crate::binary_batch::decode_entry(tag, bytes)?
                        else {
                            return Err(bad());
                        };
                        if key != r.key || !r.deleted || r.version != seq {
                            return Err(bad());
                        }
                    }
                    r.offset = if refs {
                        if r.deleted { 0 } else { position }
                    } else {
                        payload_offset
                    };
                    payload_offset += r.len as u64;
                    if layout == Layout::JsonPayloadCopy {
                        if slot != 0 {
                            sink.write_all(b",")?;
                        }
                        serde_json::to_writer(&mut sink, &r)?;
                    } else {
                        write_record(&mut sink, &r)?;
                    }
                    slot += 1;
                    Ok(())
                },
            )?;
            if seq != self.sequence || keys != self.len() || slot as usize != self.len() {
                return Err(bad());
            }
            if layout == Layout::JsonPayloadCopy {
                sink.write_all(b"],\"postings\":[")?;
            }
            let mut terms = self.term_cursor();
            let mut n = 0;
            while let Some((term, mut p)) = terms.next()? {
                p.offset = postings_start + p.offset - old_start.ok_or_else(bad)?;
                if layout == Layout::JsonPayloadCopy {
                    if n != 0 {
                        sink.write_all(b",")?;
                    }
                    serde_json::to_writer(&mut sink, &(term, p))?;
                } else {
                    write_posting(&mut sink, &term, p)?;
                }
                n += 1;
            }
            if layout == Layout::JsonPayloadCopy {
                sink.write_all(b"]}")?;
            }
            let len = sink.written as u64;
            drop(sink);
            writer.flush()?;
            let checksum = prefix_crc(writer.get_mut(), offset + len)?;
            writer.seek(SeekFrom::Start(offset + len))?;
            let mut footer = (if layout == Layout::JsonPayloadCopy {
                super::FOOTER
            } else {
                FOOTER
            })
            .to_vec();
            footer.extend_from_slice(&offset.to_le_bytes());
            footer.extend_from_slice(&len.to_le_bytes());
            footer.extend_from_slice(&checksum.to_le_bytes());
            footer.extend_from_slice(&crc(&footer).to_le_bytes());
            writer.write_all(&footer)?;
            writer.flush()?;
            writer.get_ref().sync_all()?;
            drop(writer);
            #[cfg(test)]
            publication_stage("prepared");
            // Independently validate emitted bytes/metadata before replacement.
            drop(DiskIndex::open(
                &temporary,
                Some((end, u32::from_le_bytes(header[24..28].try_into().unwrap()))),
            )?);
            // A crash between unlink and rename loses only a derived cache;
            // normal open rebuilds it from the unchanged checked source.
            std::fs::remove_file(path)?;
            #[cfg(test)]
            publication_stage("unlinked");
            std::fs::rename(&temporary, path)?;
            #[cfg(test)]
            publication_stage("published");
            Ok(())
        })();
        let _ = std::fs::remove_file(&temporary);
        let _ = std::fs::remove_file(temporary.with_extension("records04.idx"));
        result
    }
    fn verify_repack_source(&self, path: &Path) -> Result<()> {
        // Full source validation is also repeated while emitting references.
        // Only immutable native Run paths are accepted by the public caller.
        if self.covering.usable(&self.file)? {
            return Ok(());
        }
        let mut header = [0; HEADER_SIZE as usize];
        read_at(&self.file, 0, &mut header)?;
        let mut file = File::open(path.with_extension("fdb"))?;
        let (seq, count) = crate::format::check_native_prefix(
            &mut file,
            u64::from_le_bytes(header[16..24].try_into().unwrap()),
            u32::from_le_bytes(header[24..28].try_into().unwrap()),
        )?;
        if seq != self.sequence || count != self.len() {
            return Err(bad());
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn builder_stage(stage: &str) {
    if std::env::var("FDB_TEST_BUILDER_STAGE").ok().as_deref() == Some(stage) {
        let marker = std::env::var_os("FDB_TEST_BUILDER_MARKER").unwrap();
        std::fs::write(marker, stage).unwrap();
        loop {
            std::thread::park();
        }
    }
}

#[cfg(test)]
fn publication_stage(stage: &str) {
    if std::env::var("FDB_TEST_REPACK_STAGE").ok().as_deref() == Some(stage) {
        let marker = std::env::var_os("FDB_TEST_REPACK_MARKER").unwrap();
        std::fs::write(marker, stage).unwrap();
        loop {
            std::thread::park();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_builder_checks_final_crc_before_publication_and_handles_empty_checkpoint() {
        let root = std::env::temp_dir().join(format!(
            "fdb-native-builder-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("source.fdb");
        let target = root.join("source.idx");
        let mut bytes = crate::format::MAGIC_V4.to_vec();
        bytes.extend(
            crate::format::encode(
                7,
                &crate::format::Batch {
                    checkpoint: true,
                    changes: vec![crate::format::Change::Put {
                        stored: StoredDocument {
                            version: 3,
                            document: crate::Document {
                                key: Key::new("s", "c", "a"),
                                title: "東京".into(),
                                text: "actual source".into(),
                                fields: BTreeMap::new(),
                                links: vec![],
                            },
                        },
                    }],
                },
            )
            .unwrap(),
        );
        let expected = crc(&bytes);
        // Valid individual frames but an independently wrong source checksum
        // must drop the unpublished builder and leave the old target untouched.
        std::fs::write(&source, &bytes).unwrap();
        std::fs::write(&target, b"old derived cache").unwrap();
        assert!(
            Builder::from_native(
                &target,
                &mut File::open(&source).unwrap(),
                bytes.len() as u64,
                expected ^ 1,
                false
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"old derived cache");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        // No entry callback occurs in an empty checkpoint; its frame sequence
        // still initializes a valid SEG05 header/directory.
        let mut empty = crate::format::MAGIC_V4.to_vec();
        empty.extend(
            crate::format::encode(
                9,
                &crate::format::Batch {
                    checkpoint: true,
                    changes: vec![],
                },
            )
            .unwrap(),
        );
        std::fs::write(&source, &empty).unwrap();
        let (builder, seq) = Builder::from_native(
            &target,
            &mut File::open(&source).unwrap(),
            empty.len() as u64,
            crc(&empty),
            true,
        )
        .unwrap();
        assert_eq!(seq, 9);
        let index = builder.finish().unwrap();
        assert_eq!(index.len(), 0);
        assert!(index.source_references);
        drop(index);
        assert_eq!(&std::fs::read(&target).unwrap()[..8], MAGIC);
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            resolved
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("fdb-native-builder-")
        );
        std::fs::remove_dir_all(resolved).unwrap();
    }

    #[test]
    fn term_codec_roundtrips_all_tags_and_rejects_every_cut_and_invalid_utf8() {
        let values = vec![
            Term::Scope("şirket".into()),
            Term::Collection("şirket".into(), "東京".into()),
            Term::Exact("s".into(), "c".into(), "f".into(), "quote\"\\🦀".into()),
            Term::Numeric("s".into(), "c".into(), "n".into(), i64::MIN),
            Term::Numeric("s".into(), "c".into(), "n".into(), i64::MAX),
            Term::Gram("s".into(), "🦀ab".into()),
            Term::Link(Key::new("s", "c", "x")),
        ];
        let p = Posting {
            offset: 123,
            count: 456,
        };
        let mut bytes = Vec::new();
        for term in &values {
            write_posting(&mut bytes, term, p).unwrap();
        }
        assert_eq!(
            decode_page(&bytes, values.len()).unwrap(),
            values.into_iter().map(|t| (t, p)).collect::<Vec<_>>()
        );
        for cut in 0..bytes.len() {
            assert!(decode_page(&bytes[..cut], 7).is_err(), "cut {cut}");
        }
        let mut bad = Vec::new();
        write_posting(&mut bad, &Term::Scope("a".into()), p).unwrap();
        bad[5] = 255;
        assert!(decode_page(&bad, 1).is_err());
        bad[0] = 6;
        assert!(decode_page(&bad, 1).is_err());
        bad[0] = 0;
        bad[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_page(&bad, 1).is_err());
        assert!(decode_page(&bytes, 6).is_err());
    }
    #[test]
    #[ignore = "subprocess helper for publication kill tests"]
    fn repack_child() {
        let root = std::env::var_os("FDB_TEST_REPACK_ROOT").unwrap();
        crate::Database::open(root)
            .unwrap()
            .repack_index_cache(crate::IndexCacheLayout::BinarySourceReferences)
            .unwrap();
    }
    #[test]
    #[ignore = "subprocess helper for default builder kill tests"]
    fn rebuild_child() {
        let root = std::env::var_os("FDB_TEST_REPACK_ROOT").unwrap();
        crate::Database::open(root)
            .unwrap()
            .check_integrity()
            .unwrap();
    }
    #[test]
    fn process_kill_before_during_and_after_cache_replacement_preserves_sources() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};
        let nonce = TEMP.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("fdb-repack-kill-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let doc = crate::Document {
            key: Key::new("shared", "texts", "a"),
            title: "İSTANBUL".into(),
            text: "actual source".into(),
            fields: BTreeMap::new(),
            links: vec![],
        };
        let mut db = crate::Database::open(&root).unwrap();
        db.commit(vec![crate::Mutation::Put {
            document: doc.clone(),
            guard: crate::Guard::Any,
        }])
        .unwrap();
        db.flush().unwrap();
        drop(db);
        let snapshot = || -> Vec<_> {
            let mut files: Vec<_> = std::fs::read_dir(&root)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|s| s == "fdb" || s == "ready"))
                .map(|p| {
                    let bytes = std::fs::read(&p).unwrap();
                    (p, bytes)
                })
                .collect();
            files.sort();
            files
        };
        let before = snapshot();
        for operation in ["REPACK", "BUILDER"] {
            for stage in ["prepared", "unlinked", "published"] {
                if operation == "BUILDER" {
                    // Force the default writer, without any explicit conversion.
                    for entry in std::fs::read_dir(&root).unwrap().flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "idx")
                            && path
                                .file_stem()
                                .unwrap()
                                .to_string_lossy()
                                .starts_with("run-")
                            && !path.file_stem().unwrap().to_string_lossy().contains('.')
                        {
                            std::fs::remove_file(path).unwrap();
                        }
                    }
                }
                let marker = root.join(format!("stage-{operation}-{stage}"));
                let mut child = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        if operation == "BUILDER" {
                            "segment::compact::tests::rebuild_child"
                        } else {
                            "segment::compact::tests::repack_child"
                        },
                        "--ignored",
                    ])
                    .env("FDB_TEST_REPACK_ROOT", &root)
                    .env(format!("FDB_TEST_{operation}_STAGE"), stage)
                    .env(format!("FDB_TEST_{operation}_MARKER"), &marker)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap();
                let start = Instant::now();
                while !marker.exists() && start.elapsed() < Duration::from_secs(20) {
                    if child.try_wait().unwrap().is_some() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                let reached = marker.exists();
                let _ = child.kill();
                child.wait().unwrap();
                assert!(reached, "{stage}");
                assert_eq!(snapshot(), before);
                let db = crate::Database::open(&root).unwrap();
                assert_eq!(db.get(&doc.key).unwrap().unwrap().document, doc);
                db.check_integrity().unwrap();
                drop(db);
            }
        }
        let target = root.canonicalize().unwrap();
        assert!(
            target.starts_with(std::env::temp_dir().canonicalize().unwrap())
                && target
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("fdb-repack-kill-")
        );
        std::fs::remove_dir_all(target).unwrap();
        // Process kill keeps the OS cache alive; this is not a power-cut test.
    }
}
