//! Running a model in the background: the engine in a thread, its progress, maps and warnings sent to the
//! application as they come, and a way to stop it. The outputs of the nodes are kept in a cache shared with
//! the application (which shows them), so that a run after an edit only runs what changed. A whole run of a model
//! saved writes its outputs and the manifest of the run (`output/manifest.json`), as `terraflux run` does.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use terraflux_exec::{Cache, Engine, Error, Observer, Results};
use terraflux_graph::Model;
use terraflux_ops::Value;

/// What a run reports.
pub enum Message {
    /// A node started.
    Node(String),
    /// A container finished a step, with the values it collected and the cells of each class of its map
    /// (counted in the run's thread).
    Step {
        at: String,
        step: usize,
        of: usize,
        values: BTreeMap<String, Value>,
        counts: Option<Vec<(f64, usize)>>,
    },
    Warning(String),
    /// The files of a folder that failed in a chain of nodes (its first node), each with why.
    Failures {
        at: String,
        failed: Vec<(String, String)>,
    },
    /// How far a node running is, as its operator tells it.
    Progress {
        at: String,
        progress: terraflux_ops::Progress,
    },
    /// A node ran: its time and about how much memory what it gave holds.
    Timed {
        at: String,
        seconds: f64,
        bytes: usize,
    },
    /// A top-level node is done (its outputs in the cache), run or `cached`.
    Finished {
        at: String,
        cached: bool,
    },
    /// What the nodes of a loop's body gave at a step: the step wanted ([`Gate::want_step`]), or every step when
    /// the run goes step by step (the step it is paused after shown).
    Body {
        at: String,
        step: usize,
        results: BTreeMap<String, terraflux_ops::Outputs>,
    },
    /// The node that stopped the run.
    Failed(String),
    /// The outputs and the manifest written (the manifest's file), or why not.
    Recorded(Result<PathBuf, String>),
    /// The run ended.
    Done(Result<Results, String>),
}

/// Where a whole run writes what it gave: the model's file (as saved: the manifest hashes it) and the folder of
/// its outputs.
pub struct Record {
    pub model_file: PathBuf,
    pub out: PathBuf,
}

impl Record {
    /// The model's outputs (`set_output`) written into the folder, and the manifest of the run with them.
    fn write(&self, model: &Model, base: &Path, results: &Results) -> Result<PathBuf, String> {
        let files = terraflux_exec::write_outputs(model, results, &self.out).map_err(|e| e.to_string())?;
        let written: Vec<(String, PathBuf)> = model.outputs.keys().cloned().zip(files).collect();
        let manifest = terraflux_exec::testing::manifest(
            &self.model_file,
            model,
            &std::path::absolute(base).unwrap_or_else(|_| base.to_path_buf()),
            &BTreeMap::new(),
            None,
            &[],
            &written,
        )
        .map_err(|e| e.to_string())?;
        let path = self.out.join("manifest.json");
        manifest.save(&path).map_err(|e| e.to_string())?;
        Ok(path)
    }
}

/// A run step by step: when `stepping`, the run pauses after each step of a loop until the next is let go (or it
/// goes on to its end, or is stopped); and before each node with a breakpoint (by its place: `node`, `loop/node` in
/// a body, at every step), or before the next node when asked. Kept by the session from run to run.
#[derive(Default)]
pub struct Gate {
    pub stepping: AtomicBool,
    /// The places of the nodes the run pauses before.
    breakpoints: Mutex<std::collections::BTreeSet<String>>,
    /// Pause before the next node that starts (Next node, from a pause).
    next_node: AtomicBool,
    /// Paused, and the pauses let go not taken yet.
    state: Mutex<(bool, u64)>,
    /// Where it is paused: before a node (its place), or none (after a loop's step).
    at: Mutex<Option<String>>,
    cv: Condvar,
    /// The step of a loop (by its place) whose body's results the run sends ([`Message::Body`]), to be shown.
    wanted: Mutex<BTreeMap<String, usize>>,
}

impl Gate {
    /// Whether the run waits (after a step, or before a node).
    pub fn paused(&self) -> bool {
        self.state.lock().unwrap().0
    }

    /// The node the run is paused before (a breakpoint, or Next node), if it is.
    pub fn paused_at(&self) -> Option<String> {
        // (the place read and let go before the state is: `hold` takes the state, then the place)
        let at = self.at.lock().unwrap().clone();
        at.filter(|_| self.paused())
    }

    /// Whether the run pauses before the node at a place (`node`, `container/node`).
    pub fn breaks_at(&self, place: &str) -> bool {
        self.breakpoints.lock().unwrap().contains(place)
    }

    /// Put a breakpoint before the node at a place, or take it away.
    pub fn set_breakpoint(&self, place: &str, on: bool) {
        let mut b = self.breakpoints.lock().unwrap();
        if on {
            b.insert(place.to_string());
        } else {
            b.remove(place);
        }
    }

    /// Keep what the body of the loop at a place gives at a step (none: its last, as the loop gives it).
    pub fn want_step(&self, place: &str, step: Option<usize>) {
        let mut w = self.wanted.lock().unwrap();
        match step {
            Some(k) => w.insert(place.to_string(), k),
            None => w.remove(place),
        };
    }

    /// The step of the loop at a place whose body is kept, if one is.
    pub fn wanted_step(&self, place: &str) -> Option<usize> {
        self.wanted.lock().unwrap().get(place).copied()
    }

    /// The places of the breakpoints.
    pub fn breakpoints(&self) -> Vec<String> {
        self.breakpoints.lock().unwrap().iter().cloned().collect()
    }

    /// Let the run go on to its next pause (the next step, stepping; the next breakpoint).
    pub fn next(&self) {
        self.state.lock().unwrap().1 += 1;
        self.cv.notify_all();
    }

    /// Let the run go on to the next node that starts, and pause before it.
    pub fn next_node(&self) {
        self.next_node.store(true, Ordering::Relaxed);
        self.next();
    }

    /// Go on without pausing after the steps (the breakpoints still pause it).
    pub fn resume(&self) {
        self.stepping.store(false, Ordering::Relaxed);
        self.next_node.store(false, Ordering::Relaxed);
        let mut st = self.state.lock().unwrap();
        if st.0 {
            st.1 += 1;
        }
        self.cv.notify_all();
    }

    /// Wake a run waiting (under the lock, not to be missed between its checks and its wait).
    fn wake(&self) {
        let _st = self.state.lock().unwrap();
        self.cv.notify_all();
    }

    /// Paused no more, no step let go (a new run).
    fn reset(&self) {
        *self.state.lock().unwrap() = (false, 0);
        *self.at.lock().unwrap() = None;
        self.next_node.store(false, Ordering::Relaxed);
    }

    /// After a step: wait until the next is let go, the run goes on to its end or is stopped.
    fn wait(&self, stop: &AtomicBool, repaint: &dyn Fn()) {
        if self.stepping.load(Ordering::Relaxed) {
            self.hold(None, stop, repaint);
        }
    }

    /// Before a node starts: wait there when it has a breakpoint, or Next node asked for it.
    fn before(&self, at: &str, stop: &AtomicBool, repaint: &dyn Fn()) {
        if self.breaks_at(at) || self.next_node.swap(false, Ordering::Relaxed) {
            self.hold(Some(at), stop, repaint);
        }
    }

    /// Paused (before a node, or after a step) until let go or stopped.
    fn hold(&self, at: Option<&str>, stop: &AtomicBool, repaint: &dyn Fn()) {
        let mut st = self.state.lock().unwrap();
        *self.at.lock().unwrap() = at.map(String::from);
        st.0 = true;
        repaint();
        while st.1 == 0 && !stop.load(Ordering::Relaxed) {
            st = self.cv.wait(st).unwrap();
        }
        st.1 = st.1.saturating_sub(1);
        st.0 = false;
        *self.at.lock().unwrap() = None;
    }
}

struct Sending {
    tx: Sender<Message>,
    stop: Arc<AtomicBool>,
    gate: Arc<Gate>,
    repaint: Box<dyn Fn() + Send>,
    /// When progress was sent last (no more than ten times a second).
    told: Option<std::time::Instant>,
}

impl Observer for Sending {
    fn node(&mut self, at: &str) {
        let _ = self.tx.send(Message::Node(at.to_string()));
        // a breakpoint: paused before it runs (what ran before it shown)
        self.gate.before(at, &self.stop, &*self.repaint);
    }

    fn collected(&mut self, at: &str, step: usize, of: usize, values: &BTreeMap<String, Value>) {
        let counts = match values.get("maps") {
            Some(Value::Map(m)) => Some(crate::map_view::class_counts(&m.data)),
            _ => None,
        };
        let _ = self.tx.send(Message::Step { at: at.to_string(), step, of, values: values.clone(), counts });
        (self.repaint)();
        // step by step: paused until the next step is let go (not after the last; steps from 1)
        if step < of {
            self.gate.wait(&self.stop, &*self.repaint);
        }
    }

    fn warning(&mut self, at: &str, message: &str) {
        let _ = self.tx.send(Message::Warning(format!("{at}: {message}")));
    }

    fn body(&mut self, at: &str, step: usize, results: &BTreeMap<String, terraflux_ops::Outputs>) {
        // (the values shared, not copied)
        if self.gate.wanted_step(at) == Some(step) || self.gate.stepping.load(Ordering::Relaxed) {
            let _ = self.tx.send(Message::Body { at: at.to_string(), step, results: results.clone() });
        }
    }

    fn failures(&mut self, at: &str, failed: &[(String, String)]) {
        let _ = self.tx.send(Message::Failures { at: at.to_string(), failed: failed.to_vec() });
    }

    fn timed(&mut self, at: &str, seconds: f64, bytes: usize) {
        let _ = self.tx.send(Message::Timed { at: at.to_string(), seconds, bytes });
    }

    fn done(&mut self, at: &str, cached: bool) {
        let _ = self.tx.send(Message::Finished { at: at.to_string(), cached });
        (self.repaint)();
    }

    fn progress(&mut self, at: &str, progress: &terraflux_ops::Progress) {
        if self.told.is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(100)) {
            return;
        }
        self.told = Some(std::time::Instant::now());
        let _ = self.tx.send(Message::Progress { at: at.to_string(), progress: progress.clone() });
        (self.repaint)();
    }

    fn cancelled(&mut self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// A run in progress.
pub struct Run {
    pub rx: Receiver<Message>,
    stop: Arc<AtomicBool>,
    gate: Arc<Gate>,
    thread: Option<JoinHandle<()>>,
}

impl Run {
    /// Run the model in a thread on its inputs, read first (relative to `base`) when not given: every node, or
    /// what `target` needs; the outputs kept in `cache`, and written with the run's manifest when `record` says
    /// where; paused after each step of a loop when `gate` says so. `repaint` wakes the application.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        model: Model,
        inputs: Option<BTreeMap<String, Value>>,
        base: PathBuf,
        cache: Arc<Mutex<Cache>>,
        target: Option<String>,
        record: Option<Record>,
        gate: Arc<Gate>,
        repaint: Box<dyn Fn() + Send>,
    ) -> Run {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        gate.reset();
        let g = gate.clone();
        let thread = std::thread::spawn(move || {
            let done_tx = tx.clone();
            let result = (|| {
                let inputs = match inputs {
                    Some(i) => i,
                    None => terraflux_exec::load_inputs(&model, &base, &BTreeMap::new()).map_err(|e| e.to_string())?,
                };
                let mut obs = Sending { tx, stop: s, gate: g, repaint, told: None };
                let r = Engine::default().run_with(&model, &inputs, &mut obs, Some(&cache), target.as_deref());
                if let Err(Error::Node { at, .. }) = &r {
                    let top = at.split(['/', '.']).next().unwrap_or(at).to_string();
                    let _ = obs.tx.send(Message::Failed(top));
                }
                let r = r.map_err(|e| e.to_string());
                if let (Ok(results), Some(record)) = (&r, &record)
                    && target.is_none()
                {
                    let _ = obs.tx.send(Message::Recorded(record.write(&model, &base, results)));
                }
                (obs.repaint)();
                r
            })();
            let _ = done_tx.send(Message::Done(result));
        });
        Run { rx, stop, gate, thread: Some(thread) }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        // (a run paused after a step wakes to stop)
        self.gate.wake();
    }

    /// Stop it and let it go without waiting: an operator that cannot stop in the middle (a long step of its own)
    /// ends in the background, its results not shown; the run's thread ends with it.
    pub fn detach(mut self) {
        self.stop();
        self.thread.take();
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        self.stop();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole run of a model saved writes its outputs and the run's manifest; a run up to a node does not.
    #[test]
    fn a_whole_run_writes_its_manifest() {
        let dir = std::env::temp_dir().join(format!("terraflux-app-record-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("m.toml");
        let text = [
            "format = \"terraflux/1\"",
            "[[nodes]]",
            "id = \"land\"",
            "op = \"nlm.random\"",
            "params = { rows = 10, columns = 8, seed = 4 }",
            "[outputs.land]",
            "from = \"land.map\"",
        ];
        std::fs::write(&file, text.join("\n")).unwrap();
        let model = Model::load_expanded(&file).unwrap();
        let run = |target: Option<String>| {
            let out = dir.join("output");
            let record = Some(Record { model_file: file.clone(), out });
            let cache = Arc::new(Mutex::new(Cache::default()));
            let gate = Arc::new(Gate::default());
            let r = Run::start(model.clone(), None, dir.clone(), cache, target, record, gate, Box::new(|| {}));
            let mut recorded = None;
            loop {
                match r.rx.recv().unwrap() {
                    Message::Recorded(x) => recorded = Some(x),
                    Message::Done(d) => {
                        d.unwrap();
                        break;
                    }
                    _ => {}
                }
            }
            recorded
        };
        assert!(run(Some("land".into())).is_none());
        let manifest = run(None).unwrap().unwrap();
        assert_eq!(manifest, dir.join("output").join("manifest.json"));
        let mf: terraflux_exec::testing::Manifest =
            serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
        assert_eq!(mf.seeds["land.seed"], serde_json::json!(4));
        assert!(Path::new(&mf.outputs["land"].path).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Step by step: the run pauses after each step of a loop (not after its last) until the next is let go, and
    /// goes on to its end when told.
    #[test]
    fn a_loop_runs_step_by_step() {
        use terraflux_graph::{Body, Node};
        let mut m = Model::new("steps");
        m.nodes.push(Node::new("loop", "control.repeat").literal("times", 3).body(Body {
            nodes: vec![Node::new("land", "nlm.random").param("rows", 4).param("columns", 5).param("seed", 1)],
            collect: [("maps".to_string(), "land.map".to_string())].into(),
            ..Default::default()
        }));
        let gate = Arc::new(Gate::default());
        gate.stepping.store(true, Ordering::Relaxed);
        let cache = Arc::new(Mutex::new(Cache::default()));
        let r = Run::start(m, None, PathBuf::new(), cache, None, None, gate.clone(), Box::new(|| {}));
        let steps = |r: &Run| {
            let mut seen = Vec::new();
            while let Ok(m) = r.rx.recv_timeout(std::time::Duration::from_secs(20)) {
                match m {
                    Message::Step { step, .. } => {
                        seen.push(step);
                        break;
                    }
                    Message::Done(d) => {
                        d.unwrap();
                        seen.push(0);
                        break;
                    }
                    _ => {}
                }
            }
            seen
        };
        let paused = || {
            let t = std::time::Instant::now();
            while !gate.paused() && t.elapsed() < std::time::Duration::from_secs(20) {
                std::thread::yield_now();
            }
            gate.paused()
        };
        assert_eq!(steps(&r), [1]);
        assert!(paused(), "after the first step");
        gate.next();
        assert_eq!(steps(&r), [2]);
        assert!(paused(), "after the second");
        gate.resume();
        assert_eq!(steps(&r), [3]);
        assert_eq!(steps(&r), [0], "to its end");
        assert!(!gate.paused());
    }
}
