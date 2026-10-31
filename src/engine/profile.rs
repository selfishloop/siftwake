//! Opt-in, bounded per-commit wall trace. Query/document loops are not traced.
//! Nested phases overlap; their durations must not be summed as exclusive time.
use super::*;
use std::{cell::RefCell, collections::VecDeque, sync::Mutex, time::Instant};

#[derive(Debug, serde::Serialize)]
pub struct WritePhase {
    pub name: &'static str,
    pub visible_sequence: u64,
    pub active_wal_id: u64,
    pub elapsed_ns: u64,
}
#[derive(Debug, serde::Serialize)]
pub struct WriteFrame {
    pub entry_sequence: u64,
    pub operation_sequence: u64,
    pub elapsed_ns: u64,
    pub phases: Vec<WritePhase>,
}
#[derive(Debug, serde::Serialize)]
pub struct WriteTrace {
    pub dropped_frames: u64,
    pub frames: Vec<WriteFrame>,
}
pub(super) struct Capture {
    capacity: usize,
    frames: VecDeque<WriteFrame>,
    dropped: u64,
}
pub(super) type CaptureHandle = Arc<Mutex<Capture>>;
struct Active {
    output: CaptureHandle,
    began: Instant,
    frame: WriteFrame,
}
thread_local! { static CURRENT: RefCell<Option<Active>> = const { RefCell::new(None) }; }
pub(super) struct Scope {
    enabled: bool,
    previous: Option<Active>,
}
impl Scope {
    pub(super) fn enter(handle: &Option<CaptureHandle>, entry_sequence: u64) -> Self {
        let Some(handle) = handle else {
            return Self {
                enabled: false,
                previous: None,
            };
        };
        let active = Active {
            output: handle.clone(),
            began: Instant::now(),
            frame: WriteFrame {
                entry_sequence,
                operation_sequence: entry_sequence.saturating_add(1),
                elapsed_ns: 0,
                phases: Vec::new(),
            },
        };
        Self {
            enabled: true,
            previous: CURRENT.with(|c| c.replace(Some(active))),
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }
        let mut active = CURRENT
            .with(|c| c.replace(self.previous.take()))
            .expect("write trace scope");
        active.frame.elapsed_ns = active.began.elapsed().as_nanos() as u64;
        let mut output = active.output.lock().unwrap_or_else(|e| e.into_inner());
        if output.frames.len() == output.capacity {
            output.frames.pop_front();
            output.dropped += 1;
        }
        output.frames.push_back(active.frame);
    }
}
pub(super) fn record(name: &'static str, sequence: u64, wal: u64, elapsed_ns: u64) {
    CURRENT.with(|c| {
        if let Some(active) = c.borrow_mut().as_mut() {
            active.frame.phases.push(WritePhase {
                name,
                visible_sequence: sequence,
                active_wal_id: wal,
                elapsed_ns,
            });
        }
    });
}
pub(super) struct Phase(Option<(Instant, &'static str, u64, u64)>);
pub(super) fn phase(enabled: bool, name: &'static str, sequence: u64, wal: u64) -> Phase {
    Phase(enabled.then(|| (Instant::now(), name, sequence, wal)))
}
impl Drop for Phase {
    fn drop(&mut self) {
        if let Some((start, name, seq, wal)) = self.0 {
            record(name, seq, wal, start.elapsed().as_nanos() as u64);
        }
    }
}

impl Database {
    /// Enable an in-memory wall trace, disabled by default. Records are keyed
    /// to the enclosing commit's prospective sequence, including publications
    /// of an earlier sequence during prefix install. No data format changes.
    /// Capacity bounds frames, not total process memory; nested phases overlap.
    pub fn start_write_profile(&mut self, capacity: usize) -> Result<()> {
        self.healthy()?;
        if !(1..=65536).contains(&capacity) {
            return Err(Error::Invalid(
                "write profile capacity must be1..65536".into(),
            ));
        }
        self.write_profile = Some(Arc::new(Mutex::new(Capture {
            capacity,
            frames: VecDeque::new(),
            dropped: 0,
        })));
        Ok(())
    }
    /// Take all captured frames and disable tracing. This handle does not pin
    /// a snapshot, source, worker or WAL; reader lifetime/GC is unaffected.
    pub fn take_write_profile(&mut self) -> Option<WriteTrace> {
        self.write_profile.take().map(|handle| {
            let mut capture = handle.lock().unwrap_or_else(|e| e.into_inner());
            WriteTrace {
                dropped_frames: capture.dropped,
                frames: capture.frames.drain(..).collect(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reclaim::tests::{Temp, doc};
    fn put(id: &str) -> Mutation {
        Mutation::Put {
            document: doc(id),
            guard: Guard::Any,
        }
    }

    #[test]
    fn bounded_trace_associates_old_and_new_publications_with_the_enclosing_commit() {
        let t = Temp::new();
        let mut db = Database::open(&t.0).unwrap();
        assert!(db.take_write_profile().is_none());
        assert!(db.start_write_profile(0).is_err());
        db.set_streamed_wal_layer(true).unwrap();
        let reader = db.snapshot_reader().unwrap();
        db.start_write_profile(2).unwrap();
        db.commit_streamed([Ok(put("a"))]).unwrap();
        let block = test_jobs::pause(&t.0);
        assert!(db.start_light_flush().unwrap());
        block.wait_entered();
        db.commit_streamed([Ok(put("a"))]).unwrap();
        drop(block);
        let began = Instant::now();
        while !db.light_flush.as_ref().unwrap().ready() {
            assert!(began.elapsed().as_secs() < 10);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(db.commit_streamed([Ok(put("b"))]).unwrap(), 3);
        let trace = db.take_write_profile().unwrap();
        assert_eq!(trace.dropped_frames, 1);
        assert_eq!(trace.frames.len(), 2);
        let frame = &trace.frames[1];
        assert_eq!(frame.entry_sequence, 2);
        assert_eq!(frame.operation_sequence, 3);
        let pubs: Vec<_> = frame
            .phases
            .iter()
            .filter(|p| p.name == "snapshot.prepare")
            .map(|p| p.visible_sequence)
            .collect();
        assert_eq!(pubs, vec![2, 3]);
        assert!(
            frame
                .phases
                .iter()
                .any(|p| p.name == "snapshot.wal_open" && p.visible_sequence == 2)
        );
        assert!(frame.phases.iter().any(|p| p.name == "prefix.install"));
        assert!(db.take_write_profile().is_none());
        assert_eq!(reader.acquire().unwrap().document_count(), 2);
        db.check_integrity().unwrap();
    }
}
