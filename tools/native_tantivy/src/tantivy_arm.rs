use crate::Page;
use crate::churn::{self, Result};
use siftwake::{Document, Key};
use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::path::Path;
use tantivy::columnar::Column;
use tantivy::index::{IndexSettings, IndexSortByField, Order, SegmentReader};
use tantivy::query::{AllQuery, BooleanQuery, EnableScoring, Occur, Query, Scorer, TermQuery};
use tantivy::schema::{
    BytesOptions, FAST, Field, INDEXED, IndexRecordOption, STORED, Schema, TextFieldIndexing,
    TextOptions, Value,
};
use tantivy::tokenizer::NgramTokenizer;
use tantivy::{
    DocAddress, DocSet, Index, IndexReader, ReloadPolicy, Searcher, TERMINATED, TantivyDocument,
    Term,
};

const KEY: &str = "key";
pub const WRITER_BYTES: usize = 32_000_000;

#[derive(Clone, Copy)]
pub struct Fields {
    id: Field,
    title: Field,
    text: Field,
    attributes: Field,
    grams: Field,
}

type Attributes = (BTreeMap<String, serde_json::Value>, Vec<Key>);

fn register(index: &Index) -> Result<()> {
    index
        .tokenizers()
        .register("all_trigrams", NgramTokenizer::all_ngrams(3, 3)?);
    Ok(())
}

pub fn create(path: &Path) -> Result<(Index, Fields)> {
    let mut schema = Schema::builder();
    schema.add_u64_field(KEY, FAST | INDEXED);
    schema.add_text_field("title", STORED);
    schema.add_text_field("text", STORED);
    schema.add_bytes_field("attributes", BytesOptions::default().set_stored());
    schema.add_text_field(
        "grams",
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("all_trigrams")
                .set_fieldnorms(false)
                .set_index_option(IndexRecordOption::Basic),
        ),
    );
    let settings = IndexSettings {
        sort_by_field: Some(IndexSortByField {
            field: KEY.into(),
            order: Order::Asc,
        }),
        ..IndexSettings::default()
    };
    let index = Index::builder()
        .schema(schema.build())
        .settings(settings)
        .create_in_dir(path)?;
    register(&index)?;
    let fields = fields(&index)?;
    Ok((index, fields))
}

fn fields(index: &Index) -> Result<Fields> {
    let schema = index.schema();
    Ok(Fields {
        id: schema.get_field(KEY)?,
        title: schema.get_field("title")?,
        text: schema.get_field("text")?,
        attributes: schema.get_field("attributes")?,
        grams: schema.get_field("grams")?,
    })
}

pub fn open_index(path: &Path) -> Result<(Index, Fields, IndexReader)> {
    let index = Index::open_in_dir(path)?;
    register(&index)?;
    let fields = fields(&index)?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;
    Ok((index, fields, reader))
}

fn normalized(text: &str) -> Cow<'_, str> {
    if text.is_ascii() {
        if text.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(text.to_ascii_lowercase())
        } else {
            Cow::Borrowed(text)
        }
    } else {
        Cow::Owned(churn::fold(text))
    }
}

pub fn encode(fields: Fields, id: usize, doc: Document) -> Result<TantivyDocument> {
    assert_eq!(doc.key, churn::key(id));
    let mut out = TantivyDocument::new();
    out.add_u64(fields.id, id as u64);
    out.add_text(fields.grams, normalized(&doc.title).as_ref());
    out.add_text(fields.grams, normalized(&doc.text).as_ref());
    out.add_text(fields.title, &doc.title);
    out.add_text(fields.text, &doc.text);
    out.add_bytes(
        fields.attributes,
        &serde_json::to_vec(&(doc.fields, doc.links))?,
    );
    Ok(out)
}

pub fn id_term(fields: Fields, id: usize) -> Term {
    Term::from_field_u64(fields.id, id as u64)
}

struct Cursor<'a> {
    segment: &'a SegmentReader,
    matched: Box<dyn Scorer>,
    keys: Column<u64>,
}

impl Cursor<'_> {
    fn next_live(&mut self) -> Option<(u64, tantivy::DocId)> {
        while self.matched.doc() != TERMINATED {
            let doc = self.matched.doc();
            self.matched.advance();
            if !self.segment.is_deleted(doc) {
                return Some((self.keys.first(doc).expect("missing numeric key"), doc));
            }
        }
        None
    }
}

fn candidate_query(terms: &[String], fields: Fields) -> Box<dyn Query> {
    let mut grams = BTreeSet::new();
    for term in terms {
        let chars: Vec<_> = term.chars().collect();
        grams.extend(
            chars
                .windows(3)
                .map(|chars| chars.iter().collect::<String>()),
        );
    }
    if grams.is_empty() {
        return Box::new(AllQuery);
    }
    Box::new(BooleanQuery::new(
        grams
            .into_iter()
            .map(|gram| {
                let term = Term::from_field_text(fields.grams, &gram);
                let query: Box<dyn Query> =
                    Box::new(TermQuery::new(term, IndexRecordOption::Basic));
                (Occur::Must, query)
            })
            .collect(),
    ))
}

fn materialize(
    fields: Fields,
    id: usize,
    stored: &TantivyDocument,
    title: &str,
    text: &str,
) -> Result<Document> {
    let bytes = stored
        .get_first(fields.attributes)
        .and_then(|value| value.as_bytes())
        .ok_or("missing attributes")?;
    let (fields, links): Attributes = serde_json::from_slice(bytes)?;
    Ok(Document {
        key: churn::key(id),
        title: title.into(),
        text: text.into(),
        fields,
        links,
    })
}

fn text_values(doc: &TantivyDocument, fields: Fields) -> Result<(&str, &str)> {
    let read = |field| doc.get_first(field).and_then(|value| value.as_str());
    Ok((
        read(fields.title).ok_or("missing title")?,
        read(fields.text).ok_or("missing text")?,
    ))
}

pub fn search(
    searcher: &Searcher,
    fields: Fields,
    text: &str,
    offset: usize,
    limit: usize,
    capped: bool,
) -> Result<Page> {
    let terms: Vec<_> = text.split_whitespace().map(churn::fold).collect();
    let query = candidate_query(&terms, fields);
    let weight = query.weight(EnableScoring::disabled_from_searcher(searcher))?;
    let mut cursors = Vec::new();
    let mut heap = BinaryHeap::new();
    for (i, segment) in searcher.segment_readers().iter().enumerate() {
        let mut cursor = Cursor {
            segment,
            matched: weight.scorer(segment, 1.0)?,
            keys: segment.fast_fields().u64(KEY)?,
        };
        if let Some((id, doc)) = cursor.next_live() {
            heap.push(Reverse((id, i, doc)));
        }
        cursors.push(cursor);
    }
    let mut total = 0;
    let mut hits = Vec::new();
    let end = offset + limit;
    while let Some(Reverse((id, i, doc))) = heap.pop() {
        let stored: TantivyDocument = searcher.doc(DocAddress::new(i as u32, doc))?;
        let (title, text) = text_values(&stored, fields)?;
        let title_folded = normalized(title);
        let text_folded = normalized(text);
        if terms
            .iter()
            .all(|term| title_folded.contains(term) || text_folded.contains(term))
        {
            if (offset..end).contains(&total) {
                hits.push(materialize(fields, id as usize, &stored, title, text)?);
            }
            total += 1;
            if capped && total == end {
                break;
            }
        }
        if let Some((id, doc)) = cursors[i].next_live() {
            heap.push(Reverse((id, i, doc)));
        }
    }
    Ok(Page { total, hits })
}

pub fn verify_all(
    searcher: &Searcher,
    fields: Fields,
    mut expected: impl FnMut(usize) -> Result<Document>,
) -> Result<()> {
    let mut keys = BTreeSet::new();
    for (i, segment) in searcher.segment_readers().iter().enumerate() {
        let column = segment.fast_fields().u64(KEY)?;
        for doc in segment.doc_ids_alive() {
            let id = column.first(doc).ok_or("missing numeric key")? as usize;
            assert!(keys.insert(id), "duplicate visible key {id}");
            let stored: TantivyDocument = searcher.doc(DocAddress::new(i as u32, doc))?;
            let (title, text) = text_values(&stored, fields)?;
            assert!(materialize(fields, id, &stored, title, text)? == expected(id)?);
        }
    }
    assert!(keys.iter().copied().eq(0..keys.len()));
    assert_eq!(keys.len(), searcher.num_docs() as usize);
    Ok(())
}
