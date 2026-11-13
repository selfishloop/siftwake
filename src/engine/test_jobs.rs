//! Test-only, per-database worker pause. Pause before claiming global resources.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

#[derive(Default)]
struct State {
    arrived: bool,
    released: bool,
}
#[derive(Default)]
struct Gate {
    state: Mutex<State>,
    wake: Condvar,
}
type Gates = Mutex<BTreeMap<PathBuf, Arc<Gate>>>;
fn gates(revision: bool) -> &'static Gates {
    static NORMAL: OnceLock<Gates> = OnceLock::new();
    static REVISION: OnceLock<Gates> = OnceLock::new();
    if revision { &REVISION } else { &NORMAL }.get_or_init(Mutex::default)
}
pub(super) struct Pause {
    path: PathBuf,
    gate: Arc<Gate>,
    revision: bool,
}
pub(super) fn pause(path: &Path) -> Pause {
    pause_for(path, false)
}
pub(super) fn pause_revision(path: &Path) -> Pause {
    pause_for(path, true)
}
fn pause_for(path: &Path, revision: bool) -> Pause {
    let path = path.canonicalize().unwrap();
    let gate = Arc::new(Gate::default());
    assert!(
        gates(revision)
            .lock()
            .unwrap()
            .insert(path.clone(), gate.clone())
            .is_none()
    );
    Pause {
        path,
        gate,
        revision,
    }
}
impl Pause {
    pub fn wait_entered(&self) {
        let state = self.gate.state.lock().unwrap();
        let (state, _) = self
            .gate
            .wake
            .wait_timeout_while(state, Duration::from_secs(10), |s| !s.arrived)
            .unwrap();
        assert!(state.arrived, "target worker did not reach its local pause");
    }
}
impl Drop for Pause {
    fn drop(&mut self) {
        gates(self.revision).lock().unwrap().remove(&self.path);
        let mut state = self.gate.state.lock().unwrap();
        state.released = true;
        self.gate.wake.notify_all();
    }
}
pub(super) fn wait(path: &Path) {
    wait_for(path, false);
}
pub(super) fn wait_revision(path: &Path) {
    wait_for(path, true);
}
fn wait_for(path: &Path, revision: bool) {
    let path = path.canonicalize().unwrap();
    let gate = gates(revision).lock().unwrap().get(&path).cloned();
    if let Some(gate) = gate {
        let mut state = gate.state.lock().unwrap();
        state.arrived = true;
        gate.wake.notify_all();
        while !state.released {
            state = gate.wake.wait(state).unwrap();
        }
    }
}
