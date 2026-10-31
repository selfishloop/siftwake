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
fn gates() -> &'static Mutex<BTreeMap<PathBuf, Arc<Gate>>> {
    static GATES: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Gate>>>> = OnceLock::new();
    GATES.get_or_init(Mutex::default)
}
pub(super) struct Pause {
    path: PathBuf,
    gate: Arc<Gate>,
}
pub(super) fn pause(path: &Path) -> Pause {
    let path = path.canonicalize().unwrap();
    let gate = Arc::new(Gate::default());
    assert!(
        gates()
            .lock()
            .unwrap()
            .insert(path.clone(), gate.clone())
            .is_none()
    );
    Pause { path, gate }
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
        gates().lock().unwrap().remove(&self.path);
        let mut state = self.gate.state.lock().unwrap();
        state.released = true;
        self.gate.wake.notify_all();
    }
}
pub(super) fn wait(path: &Path) {
    let path = path.canonicalize().unwrap();
    let gate = gates().lock().unwrap().get(&path).cloned();
    if let Some(gate) = gate {
        let mut state = gate.state.lock().unwrap();
        state.arrived = true;
        gate.wake.notify_all();
        while !state.released {
            state = gate.wake.wait(state).unwrap();
        }
    }
}
