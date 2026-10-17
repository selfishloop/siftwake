//! Existing fixed 10k slot only; no corpus generation or database rebuild.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new("data/benchmark-work/fdb");
    if !path.is_dir() {
        return Err("existing fixed benchmark slot required".into());
    }
    // Defer automatic startup reclamation so this probe can report the explicit
    // operation. reclaim_obsolete itself completes full integrity validation.
    let mut db = fdb::Database::open_deferred(path)?;
    let before = db.stats()?;
    if before.documents != 10_000 || before.delta_keys != 0 {
        return Err("expected immutable first10000 benchmark documents".into());
    }
    let start = std::time::Instant::now();
    let reclaimed = db.reclaim_obsolete()?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
    let after = db.stats()?;
    assert_eq!(before.documents, after.documents);
    assert_eq!(before.sequence, after.sequence);
    assert_eq!(before.generation, after.generation);
    assert_eq!(db.pending_source_checks()?, 0);
    println!(
        "{}",
        serde_json::json!({"phase":"reclaimed","reclaimed":reclaimed,"validation_and_reclaim_ms":elapsed_ms,"before":before,"after":after,"pending_source_checks":0,"database":path,"scope":"explicit full validation plus unlink; warm OS cache; no corpus, writes, reset, DB copies or memory measurement"})
    );
    Ok(())
}
