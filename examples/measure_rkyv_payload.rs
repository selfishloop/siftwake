//! Codec-only experiment; no WAL/index/manifest change or application integration.
#[cfg(not(feature = "experimental-rkyv-payload"))]
fn main() {
    eprintln!("build with experimental-rkyv-payload");
    std::process::exit(1);
}

#[cfg(feature = "experimental-rkyv-payload")]
mod experiment {
    use fdb::{
        Document, Key, StoredDocument,
        rkyv_payload::{
            self as payload,
            bench::{Codec, Reader, Writer, identity, verify_crc},
        },
    };
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::{
        collections::BTreeMap,
        fs::{self, File},
        io::{Read, Seek, SeekFrom},
        path::Path,
        time::Instant,
    };
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    #[derive(Deserialize, Serialize)]
    struct Source {
        book_id: u32,
        title: String,
    }
    #[derive(Deserialize, Serialize)]
    struct Row {
        id: u32,
        book: usize,
        start: u64,
        len: usize,
    }
    #[derive(Deserialize, Serialize)]
    struct Request {
        name: String,
        text: String,
        candidate_ids: Vec<u32>,
        total: usize,
        page: Vec<Row>,
    }
    #[derive(Deserialize, Serialize)]
    struct Oracle {
        count: u32,
        sources: Vec<Source>,
        queries: Vec<Request>,
    }
    #[derive(Deserialize)]
    struct Corpus {
        count: u32,
        sources: Vec<Source>,
        queries: Vec<Request>,
        rows: Vec<Row>,
    }
    fn document(root: &Path, source: &Source, row: &Row) -> Result<StoredDocument> {
        let mut file = File::open(root.join(format!("body{}.txt", source.book_id)))?;
        file.seek(SeekFrom::Start(row.start))?;
        let mut bytes = vec![0; row.len];
        file.read_exact(&mut bytes)?;
        Ok(StoredDocument {
            version: row.id as u64 / 500 + 1,
            document: Document {
                key: Key::new("shared", "texts", &format!("{:010}", row.id)),
                title: format!("Passage {:010}", row.id),
                text: String::from_utf8(bytes)?,
                fields: BTreeMap::from([
                    ("source".into(), json!(source.title)),
                    ("book_id".into(), json!(source.book_id)),
                    ("offset".into(), json!(row.start)),
                    ("bytes".into(), json!(row.len)),
                    ("tags".into(), json!(["literature", "en"])),
                    (
                        "attributes".into(),
                        json!({"verified":false,"score":0.125,"optional":null}),
                    ),
                ]),
                links: if row.id.is_multiple_of(3) {
                    vec![Key::new(
                        "shared",
                        "texts",
                        &format!("{:010}", row.id.saturating_sub(1)),
                    )]
                } else {
                    vec![]
                },
            },
        })
    }
    fn generate(root: &Path) -> Result<()> {
        let corpus: Corpus = serde_json::from_slice(&fs::read(root.join("corpus.json"))?)?;
        assert_eq!(corpus.rows.len(), corpus.count as usize);
        let mut json_writer =
            Writer::create(&root.join("json.payload-bench"), Codec::Json, corpus.count)?;
        let mut rkyv_writer =
            Writer::create(&root.join("rkyv.payload-bench"), Codec::Rkyv, corpus.count)?;
        let (mut json_ns, mut rkyv_ns) = (0u128, 0u128);
        for row in &corpus.rows {
            let stored = document(root, &corpus.sources[row.book], row)?;
            let start = Instant::now();
            json_writer.append(&stored)?;
            json_ns += start.elapsed().as_nanos();
            let start = Instant::now();
            rkyv_writer.append(&stored)?;
            rkyv_ns += start.elapsed().as_nanos();
            if (row.id + 1).is_multiple_of(25_000) {
                eprintln!("encoded {} documents", row.id + 1);
            }
        }
        json_writer.finish()?;
        rkyv_writer.finish()?;
        fs::write(
            root.join("oracle.json"),
            serde_json::to_vec(&Oracle {
                count: corpus.count,
                sources: corpus.sources,
                queries: corpus.queries,
            })?,
        )?;
        println!(
            "{}",
            json!({"phase":"generation","count":corpus.count,"json_encode_write_ms":json_ns as f64/1e6,"rkyv_encode_write_ms":rkyv_ns as f64/1e6,"generation_timings_not_acceptance_gate":true})
        );
        Ok(())
    }
    #[derive(Default, Serialize)]
    struct Profile {
        io_ms: f64,
        crc_ms: f64,
        validation_or_json_decode_ms: f64,
        normalization_ms: f64,
        contains_ms: f64,
        page_decode_ms: f64,
    }
    fn span<T>(enabled: bool, ms: &mut f64, f: impl FnOnce() -> fdb::Result<T>) -> fdb::Result<T> {
        if !enabled {
            return f();
        }
        let start = Instant::now();
        let result = f();
        *ms += start.elapsed().as_secs_f64() * 1000.;
        result
    }
    fn matches(
        matcher: &mut payload::TextMatcher,
        title: &str,
        text: &str,
        terms: &[String],
        profile: bool,
        p: &mut Profile,
    ) -> fdb::Result<bool> {
        span(profile, &mut p.normalization_ms, || {
            matcher.prepare_title(title);
            Ok(())
        })?;
        if span(profile, &mut p.contains_ms, || {
            Ok(matcher.title_matches_all(terms))
        })? {
            return Ok(true);
        }
        span(profile, &mut p.normalization_ms, || {
            matcher.prepare_text(text);
            Ok(())
        })?;
        span(profile, &mut p.contains_ms, || {
            Ok(matcher.matches_all(terms))
        })
    }
    fn scan(
        reader: &mut Reader,
        query: &Request,
        profile: bool,
        strategy: payload::FoldStrategy,
    ) -> fdb::Result<(usize, Vec<StoredDocument>, Profile)> {
        let mut p = Profile::default();
        let terms = payload::query_terms(&query.text);
        let codec = reader.codec;
        let mut matcher = payload::TextMatcher::new(strategy);
        let (mut total, mut hits) = (0, Vec::new());
        for &id in &query.candidate_ids {
            let checksum = reader.checksum(id)?;
            let bytes = span(profile, &mut p.io_ms, || reader.bytes(id))?;
            span(profile, &mut p.crc_ms, || verify_crc(bytes, checksum))?;
            match codec {
                Codec::Json => {
                    let stored: StoredDocument =
                        span(profile, &mut p.validation_or_json_decode_ms, || {
                            let stored: StoredDocument = serde_json::from_slice(bytes)?;
                            identity(
                                (
                                    &stored.document.key.scope,
                                    &stored.document.key.collection,
                                    &stored.document.key.id,
                                ),
                                stored.version,
                                id,
                            )?;
                            Ok(stored)
                        })?;
                    let include = matches(
                        &mut matcher,
                        &stored.document.title,
                        &stored.document.text,
                        &terms,
                        profile,
                        &mut p,
                    )?;
                    if include {
                        total += 1;
                        if hits.len() < 50 {
                            hits.push(stored);
                        }
                    }
                }
                Codec::Rkyv => {
                    let stored = span(profile, &mut p.validation_or_json_decode_ms, || {
                        let stored = payload::checked(bytes)?;
                        identity(stored.key_parts(), stored.version(), id)?;
                        Ok(stored)
                    })?;
                    let include = matches(
                        &mut matcher,
                        stored.title(),
                        stored.text(),
                        &terms,
                        profile,
                        &mut p,
                    )?;
                    if include {
                        total += 1;
                        if hits.len() < 50 {
                            hits.push(span(profile, &mut p.page_decode_ms, || {
                                stored.to_document()
                            })?);
                        }
                    }
                }
                Codec::Binary => {
                    let stored = span(profile, &mut p.validation_or_json_decode_ms, || {
                        let stored = payload::binary::checked(bytes)?;
                        identity(stored.key_parts(), stored.version(), id)?;
                        Ok(stored)
                    })?;
                    let include = matches(
                        &mut matcher,
                        stored.title(),
                        stored.text(),
                        &terms,
                        profile,
                        &mut p,
                    )?;
                    if include {
                        total += 1;
                        if hits.len() < 50 {
                            hits.push(span(profile, &mut p.page_decode_ms, || {
                                stored.to_document()
                            })?);
                        }
                    }
                }
            }
        }
        Ok((total, hits, p))
    }
    pub fn run() -> Result<()> {
        let mut args = std::env::args().skip(1);
        let mode = args
            .next()
            .ok_or("generate|generate-binary|json|rkyv|binary|profile-{codec}")?;
        let root_arg = args.next().ok_or("corpus directory required")?;
        let strategy = match args.next().as_deref().unwrap_or("baseline") {
            "baseline" => payload::FoldStrategy::Baseline,
            "ascii" => payload::FoldStrategy::Ascii,
            "reuse" => payload::FoldStrategy::Reuse,
            _ => return Err("invalid fold strategy: baseline|ascii|reuse".into()),
        };
        if args.next().is_some() {
            return Err("extra arguments".into());
        }
        let root = Path::new(&root_arg);
        if mode == "generate-binary" {
            let mut reader = Reader::open(&root.join("json.payload-bench"), Codec::Json)?;
            let mut writer = Writer::create(
                &root.join("binary.payload-bench"),
                Codec::Binary,
                reader.len() as u32,
            )?;
            for id in 0..reader.len() as u32 {
                let checksum = reader.checksum(id)?;
                let source = reader.bytes(id)?;
                verify_crc(source, checksum)?;
                let stored: StoredDocument = serde_json::from_slice(source)?;
                let bytes = payload::binary::encode(&stored)?;
                assert_eq!(payload::binary::checked(&bytes)?.to_document()?, stored);
                writer.append(&stored)?;
                if (id + 1).is_multiple_of(25_000) {
                    eprintln!("binary round-trip verified {} documents", id + 1);
                }
            }
            writer.finish()?;
            println!("binary export round-trip verified");
            return Ok(());
        }
        if mode == "generate" {
            return generate(root);
        }
        let codec = match mode.as_str() {
            "json" | "profile-json" => Codec::Json,
            "rkyv" | "profile-rkyv" => Codec::Rkyv,
            "binary" | "profile-binary" => Codec::Binary,
            _ => return Err("invalid codec".into()),
        };
        let profiled = mode.starts_with("profile-");
        // Only the compact oracle/candidate lists are loaded in the measured process.
        let oracle: Oracle = serde_json::from_slice(&fs::read(root.join("oracle.json"))?)?;
        let start = Instant::now();
        let mut reader =
            Reader::open(&root.join(format!("{}.payload-bench", codec.name())), codec)?;
        let open_ms = start.elapsed().as_secs_f64() * 1000.;
        assert_eq!(reader.len(), oracle.count as usize);
        println!(
            "{}",
            json!({"phase":"open","codec":codec.name(),"open_ms":open_ms,"disk_bytes":reader.disk_bytes,"count":reader.len(),"validation":if codec==Codec::Json {"CRC + full JSON/schema/key/version"} else if codec==Codec::Rkyv {"CRC + bytecheck + graph/key/version"} else {"CRC + offsets + UTF-8 + typed metadata + key/version"},"scope":"payload-only; candidate/index construction excluded; no DB integration","profiled":profiled})
        );
        for query in &oracle.queries {
            let expected = query
                .page
                .iter()
                .map(|row| document(root, &oracle.sources[row.book], row))
                .collect::<Result<Vec<_>>>()?;
            let repeats = if profiled {
                1
            } else if query.name == "rare" {
                20
            } else {
                1
            };
            let mut times = Vec::new();
            for repeat in 0..=repeats {
                let start = Instant::now();
                let (total, hits, details) = scan(&mut reader, query, profiled, strategy)?;
                let elapsed = start.elapsed().as_secs_f64() * 1000.;
                assert_eq!(total, query.total, "{}", query.name);
                assert_eq!(hits, expected, "{}", query.name);
                if repeat > 0 {
                    times.push(elapsed);
                }
                if profiled {
                    println!(
                        "{}",
                        json!({"phase":"profile","codec":codec.name(),"strategy":strategy.name(),"query":query.name,"repeat":repeat,"elapsed_ms":elapsed,"details":details,"instrumented":true})
                    );
                }
            }
            times.sort_by(f64::total_cmp);
            println!(
                "{}",
                json!({"phase":"query","codec":codec.name(),"strategy":strategy.name(),"query":query.name,"text":query.text,"candidates":query.candidate_ids.len(),"total":query.total,"p50_ms":times[times.len()/2],"p95_ms":times[(times.len()*95/100).min(times.len()-1)],"samples_ms":times,"warmup":1,"full_page_oracle_verified":true,"cache":"OS cache warmed by full validation and untimed first query; no forced eviction; not cold I/O","profiled":profiled})
            );
        }
        Ok(())
    }
}
#[cfg(feature = "experimental-rkyv-payload")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    experiment::run()
}
