//! Running a model in the background: the engine in a thread, its progress, maps and warnings sent to the
//! application as they come, and a way to stop it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use terraflux_exec::{Engine, Observer, Results};
use terraflux_graph::Model;
use terraflux_ops::Value;

/// What a run reports.
pub enum Message {
    /// A node started.
    Node(String),
    /// A container finished a step, with the values it collected.
    Step {
        at: String,
        step: usize,
        of: usize,
        values: BTreeMap<String, Value>,
    },
    Warning(String),
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
        let _ = self.tx.send(Message::Step { at: at.to_string(), step, of, values: values.clone() });
        (self.repaint)();
    }

    fn warning(&mut self, at: &str, message: &str) {
        let _ = self.tx.send(Message::Warning(format!("{at}: {message}")));
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
    /// Read the inputs (relative to `base`) and run the model in a thread; `repaint` wakes the application.
    pub fn start(model: Model, base: PathBuf, repaint: Box<dyn Fn() + Send>) -> Run {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread = std::thread::spawn(move || {
            let done_tx = tx.clone();
            let result = (|| {
                let inputs = terraflux_exec::load_inputs(&model, &base, &BTreeMap::new()).map_err(|e| e.to_string())?;
                let mut obs = Sending { tx, stop: s, repaint };
                let r = Engine::default().run(&model, &inputs, &mut obs).map_err(|e| e.to_string());
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
