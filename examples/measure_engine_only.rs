//! Isolated engine process: bounded stdin document, no corpus/model/oracle/history.
#[cfg(all(windows, feature = "bounded-engine"))]
use fdb::{Database, Guard, Mutation};
#[cfg(all(windows, feature = "bounded-engine"))]
use std::io::{BufRead, BufReader};
#[cfg(all(windows, feature = "bounded-engine"))]
fn memory() -> serde_json::Value {
    #[repr(C)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak_ws: usize,
        ws: usize,
        peak_paged: usize,
        paged: usize,
        peak_nonpaged: usize,
        nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(p: *mut std::ffi::c_void, c: *mut Counters, n: u32) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
    }
    let mut c: Counters = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<Counters>() as u32;
    assert_ne!(
        unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) },
        0
    );
    serde_json::json!({"private_bytes":c.private,"working_set_bytes":c.ws,"peak_private_commit_bytes":c.peak_pagefile,"peak_working_set_bytes":c.peak_ws})
}
#[cfg(all(windows, feature = "bounded-engine"))]
fn read_mutation(input: &mut impl BufRead) -> fdb::Result<Mutation> {
    let mut line = String::new();
    input.read_line(&mut line)?;
    if line.len() > 1024 * 1024 {
        return Err(fdb::Error::Invalid("stdin record bound".into()));
    }
    Ok(Mutation::Put {
        document: serde_json::from_str(&line)?,
        guard: Guard::Any,
    })
}
#[cfg(all(windows, feature = "bounded-engine"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1).ok_or("ingest or idle")?;
    let _reference = Database::snapshot_mask_reference_for_diagnostics(
        std::env::var_os("FDB_SNAPSHOT_MASK_REFERENCE").is_some(),
    );
    let mut db = Database::open("data/benchmark-work/fdb")?;
    db.set_streamed_wal_layer(true)?;
    let reader = db.snapshot_reader()?;
    let mut query_counts = Vec::new();
    if mode == "ingest" {
        assert_eq!(db.stats()?.documents, 0);
        let mut input = BufReader::with_capacity(8192, std::io::stdin().lock());
        for _ in 0..100 {
            db.commit_streamed((0..500).map(|_| read_mutation(&mut input)))?;
        }
        db.maintain_idle()?;
    } else if mode == "loaded" {
        use std::sync::{
            Arc, Barrier,
            atomic::{AtomicBool, Ordering},
        };
        let texts: Vec<String> =
            serde_json::from_slice(&std::fs::read(std::env::var("FDB_RAM_QUERY_LIST")?)?)?;
        assert_eq!(texts.len(), 41);
        let queries: Arc<Vec<_>> = Arc::new(
            texts
                .into_iter()
                .map(|text| {
                    let mut q = fdb::Query::new("shared");
                    q.collection = Some("texts".into());
                    q.text = text;
                    q
                })
                .collect(),
        );
        let active = Arc::new(AtomicBool::new(true));
        let barrier = Arc::new(Barrier::new(5));
        let reference = std::env::var_os("FDB_HYBRID_WAIT_REFERENCE").is_some();
        std::thread::scope(|scope| -> fdb::Result<()> {
            let handles: Vec<_> = (0..4)
                .map(|id| {
                    let reader = reader.clone();
                    let queries = queries.clone();
                    let active = active.clone();
                    let barrier = barrier.clone();
                    scope.spawn(move || -> fdb::Result<usize> {
                        let _reference = Database::hybrid_wait_reference_for_diagnostics(reference);
                        barrier.wait();
                        let mut n = 0;
                        while active.load(Ordering::Acquire) {
                            let snapshot = reader.acquire()?;
                            let page =
                                snapshot.search_first_hybrid(&queries[(n + id * 9) % 41], 200)?;
                            let _ = std::hint::black_box(&page);
                            drop(page);
                            if let Some(error) = snapshot.release().cleanup_error {
                                return Err(fdb::Error::Corrupt(error));
                            }
                            n += 1;
                        }
                        Ok(n)
                    })
                })
                .collect();
            barrier.wait();
            let result = (|| -> fdb::Result<()> {
                let mut input = BufReader::with_capacity(8192, std::io::stdin().lock());
                for n in 0..16384 {
                    db.commit_streamed(std::iter::once_with(|| read_mutation(&mut input)))?;
                    if n == 8191 {
                        db.maintain_idle()?;
                    }
                }
                Ok(())
            })();
            active.store(false, Ordering::Release);
            for handle in handles {
                let count = handle.join().map_err(|_| fdb::Error::Poisoned)??;
                assert!(count > 0);
                query_counts.push(count);
            }
            result
        })?;
        db.maintain_idle()?;
    } else if mode != "idle" {
        return Err("unknown mode".into());
    }
    assert_eq!(db.stats()?.documents, 50000);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let measured = memory();
    println!(
        "{}",
        serde_json::json!({"phase":mode,"memory":measured,"documents":50000,"snapshot_reader":true,"query_counts":query_counts,"writer_mutations":if mode=="loaded"{16384}else if mode=="ingest"{50000}else{0},"caller_input":"at most one bounded JSON document +8KiB stdin buffer; runtime included, no subtraction; loaded has41 small query definitions","excluded":"corpus/catalog,model,oracle,history,profiling all outside process"})
    );
    Ok(())
}
#[cfg(not(all(windows, feature = "bounded-engine")))]
fn main() {
    eprintln!("engine-only process RAM probe requires Windows bounded-engine");
}
