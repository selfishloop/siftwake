#[cfg(feature = "bounded-engine")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use siftwake::{Database, IndexCacheLayout};
    let path = std::path::Path::new("data/benchmark-work/fdb");
    if !path.is_dir() {
        return Err("existing fixed 10k slot required".into());
    }
    let layout = match std::env::args().nth(1).as_deref() {
        Some("json") => IndexCacheLayout::JsonPayloadCopy,
        Some("binary") => IndexCacheLayout::BinaryPayloadCopy,
        Some("references") => IndexCacheLayout::BinarySourceReferences,
        _ => return Err("usage: repack_index_cache json|binary|references".into()),
    };
    let db = Database::open(path)?;
    let before = db.stats()?;
    if before.documents != 10_000 || before.delta_keys != 0 {
        return Err("expected immutable first 10k".into());
    }
    let start = std::time::Instant::now();
    db.repack_index_cache(layout)?;
    let db = Database::open(path)?;
    let after = db.stats()?;
    assert_eq!(after.documents, before.documents);
    assert_eq!(after.sequence, before.sequence);
    db.check_integrity()?;
    println!(
        "{}",
        serde_json::json!({"layout":format!("{layout:?}"),"elapsed_ms": start.elapsed().as_secs_f64()*1000.,"before":before,"after":after,"pending":db.pending_source_checks()?})
    );
    Ok(())
}
#[cfg(not(feature = "bounded-engine"))]
fn main() {
    eprintln!("requires bounded-engine");
}
