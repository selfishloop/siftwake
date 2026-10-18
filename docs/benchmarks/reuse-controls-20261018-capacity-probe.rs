//! Read-only diagnostic against the archived prototype library. No timing claim.
use fdb::{Database, Query, StoredDocument};

fn main() -> fdb::Result<()> {
    let db = Database::open("data/benchmark-work/fdb")?;
    assert_eq!(db.stats()?.documents, 10000);
    assert_eq!(db.pending_source_checks()?, 0);
    for (text, limit) in [("quixote", 50), ("holmes", 50), ("the", 200)] {
        let mut q = Query::new("shared");
        q.collection = Some("texts".into());
        q.text = text.into();
        q.limit = limit;
        let off = db.search_reuse_probe(&q, false)?;
        let on = db.search_reuse_probe(&q, true)?;
        assert_eq!((off.total, &off.hits), (on.total, &on.hits));
        println!(
            "{{\"text\":\"{text}\",\"limit\":{limit},\"hits\":{},\"off_capacity\":{},\"on_capacity\":{},\"stored_document_bytes\":{},\"extra_vec_bytes\":{},\"same_full_results\":true}}",
            off.hits.len(),
            off.hits.capacity(),
            on.hits.capacity(),
            std::mem::size_of::<StoredDocument>(),
            (on.hits.capacity() as i64 - off.hits.capacity() as i64)
                * std::mem::size_of::<StoredDocument>() as i64,
        );
    }
    Ok(())
}
