//! Running a model in the background: the engine in a thread, its progress, maps and warnings sent to the
//! application as they come, and a way to stop it. The outputs of the nodes are kept in a cache shared with
//! the application (which shows them), so that a run after an edit only runs what changed.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
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
    /// A top-level node is done (its outputs in the cache), run or `cached`.
    Finished {
        at: String,
        cached: bool,
    },
    /// The node that stopped the run.
    Failed(String),
    /// The run ended.
    Done(Result<Results, String>),
}

struct Sending {
    tx: Sender<Message>,
    stop: Arc<AtomicBool>,
    repaint: Box<dyn Fn() + Send>,
}

impl Observer for Sending {
    fn node(&mut self, at: &str) {
        let _ = self.tx.send(Message::Node(at.to_string()));
    }

    fn collected(&mut self, at: &str, step: usize, of: usize, values: &BTreeMap<String, Value>) {
        let counts = match values.get("maps") {
            Some(Value::Map(m)) => Some(crate::map_view::class_counts(&m.data)),
            _ => None,
        };
        let _ = self.tx.send(Message::Step { at: at.to_string(), step, of, values: values.clone(), counts });
        (self.repaint)();
    }

    fn warning(&mut self, at: &str, message: &str) {
        let _ = self.tx.send(Message::Warning(format!("{at}: {message}")));
    }

    fn done(&mut self, at: &str, cached: bool) {
        let _ = self.tx.send(Message::Finished { at: at.to_string(), cached });
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
    thread: Option<JoinHandle<()>>,
}

impl Run {
    /// Run the model in a thread on its inputs, read first (relative to `base`) when not given: every node, or
    /// what `target` needs; the outputs kept in `cache`. `repaint` wakes the application.
    pub fn start(
        model: Model,
        inputs: Option<BTreeMap<String, Value>>,
        base: PathBuf,
        cache: Arc<Mutex<Cache>>,
        target: Option<String>,
        repaint: Box<dyn Fn() + Send>,
    ) -> Run {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread = std::thread::spawn(move || {
            let done_tx = tx.clone();
            let result = (|| {
                let inputs = match inputs {
                    Some(i) => i,
                    None => terraflux_exec::load_inputs(&model, &base, &BTreeMap::new()).map_err(|e| e.to_string())?,
                };
                let mut obs = Sending { tx, stop: s, repaint };
                let r = Engine::default().run_with(&model, &inputs, &mut obs, Some(&cache), target.as_deref());
                if let Err(Error::Node { at, .. }) = &r {
                    let top = at.split(['/', '.']).next().unwrap_or(at).to_string();
                    let _ = obs.tx.send(Message::Failed(top));
                }
                let r = r.map_err(|e| e.to_string());
                (obs.repaint)();
                r
            })();
            let _ = done_tx.send(Message::Done(result));
        });
        Run { rx, stop, thread: Some(thread) }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
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
