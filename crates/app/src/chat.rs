//! The assistant: a chat with a model of the user's choosing (a university's server, one on this computer, a
//! commercial API: [`crate::llm`]) that reads, explains, changes and runs the model open, with the tools of
//! `terraflux mcp` ([`crate::tools`]) called in the application. The conversation goes on in a thread; the tools
//! that change the model, or run it, are done here, between frames, each change an edit of its own (ctrl+Z undoes
//! it) with the node it touched selected on the canvas.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use egui::{Color32, Key, Margin, RichText, Ui};
use serde_json::{Value, json};
use terraflux_graph::{Model, Severity};
use terraflux_ops::Registry;

use crate::app::Session;
use crate::editor_view::NodeStatus;
use crate::llm::{self, Delta, Endpoint, Provider};
use crate::theme;
use crate::tools::{self, Reply};

/// The rounds of tools in an answer, at most (a model going round in circles is stopped).
const ROUNDS: usize = 40;

/// The problems told to the assistant on their own one after another, at most (until the user writes).
const TOLD: usize = 3;

/// The model's text given whole in the system's message, up to this many characters (else its nodes, and
/// show_model for the rest).
const WHOLE: usize = 12_000;

/// What the pane shows of the conversation.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    User(String),
    Assistant { text: String, thinking: String },
    Tool { name: String, args: String, result: Option<(String, bool)> },
    Error(String),
}

/// The assistant: its conversation, what is being typed, the thread answering, the settings of its server.
#[derive(Default)]
pub struct Chat {
    /// The conversation as the API has it (the system's message apart: made again for each question).
    history: Vec<Value>,
    pub items: Vec<Item>,
    pub input: String,
    worker: Option<Worker>,
    /// A run asked for by the model, answered when it ends.
    run: Option<RunWait>,
    /// The settings of the server shown.
    pub setup: bool,
    key_text: String,
    listing: Option<Receiver<Result<Vec<String>, String>>>,
    listed: Vec<String>,
    list_error: Option<String>,
    testing: Option<Receiver<llm::Checks>>,
    tested: llm::Checks,
    note: Option<String>,
    /// What the model changed by the assistant is signed with: the assistant (its model and server), and the
    /// first question of the conversation (the request, when the model has none).
    stamp: Stamp,
    /// Not to tell the assistant of the problems that come up (by default it is told, in a conversation going on).
    pub quiet: bool,
    /// The problems told last, how many times one after another since the user wrote, the run whose failure was
    /// told, and whether an answer just ended (the model's errors told then).
    told: Option<String>,
    told_times: usize,
    told_run: Option<std::time::Instant>,
    answered: bool,
}

/// The assistant's mark on the model it changes: the author, the request.
#[derive(Default, Clone)]
struct Stamp {
    who: String,
    asked: String,
}

impl Stamp {
    /// The model signed: the assistant among its authors, the request it answers (when it has none).
    fn on(&self, m: &mut Model) {
        if !self.who.is_empty() {
            m.author = match m.author.take() {
                None => Some(self.who.clone()),
                Some(a) if a.contains(&self.who) => Some(a),
                Some(a) => Some(format!("{a}; {}", self.who)),
            };
        }
        if m.request.is_none() && !self.asked.is_empty() {
            m.request = Some(self.asked.clone());
        }
    }
}

/// The thread answering: what it sends, and the flag that stops it.
struct Worker {
    rx: Receiver<Event>,
    stop: Arc<AtomicBool>,
}

enum Event {
    Delta(Delta),
    Call {
        name: String,
        args: String,
    },
    Ask(Ask),
    Result {
        text: String,
        error: bool,
    },
    /// Messages added to the conversation.
    Turn(Value),
    Done,
    Failed(String),
}

/// A tool to be done here, on the model open: its name, arguments, and where to answer.
struct Ask {
    name: String,
    args: Value,
    reply: Sender<Answer>,
}

enum Answer {
    Done(Reply),
    /// The model open (its source) and its folder, for a tool that reads it in the thread.
    Model(Box<Model>, PathBuf),
}

struct RunWait {
    reply: Sender<Answer>,
    started: bool,
}

impl Chat {
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }

    /// The provider chosen.
    pub fn provider() -> Option<Provider> {
        let s = crate::settings::get();
        s.provider.and_then(|k| s.providers.get(k).cloned())
    }

    /// Ask the model of the provider chosen (none chosen: its settings shown).
    pub fn send(&mut self, text: String, s: &Session, ctx: &egui::Context) {
        self.told_times = 0;
        match Chat::provider() {
            Some(p) => self.send_to(&p, text, s, ctx),
            None => self.setup = true,
        }
    }

    /// Ask a provider's model: the question and the conversation so far, the model open told in the system's
    /// message.
    pub fn send_to(&mut self, p: &Provider, text: String, s: &Session, ctx: &egui::Context) {
        let text = text.trim().to_string();
        if text.is_empty() || self.busy() {
            return;
        }
        if !self.history.iter().any(|m| m["role"] == "user") {
            self.stamp.asked = text.clone();
        }
        self.stamp.who = format!("Terraflux's assistant ({} on {})", p.model.trim(), p.name);
        self.items.push(Item::User(text.clone()));
        let question = json!({ "role": "user", "content": text });
        self.history.push(question);
        let system = format!("{}{}", tools::APP_INSTRUCTIONS, context(s));
        let mut offered = tools::catalogue(true);
        if p.few_tools {
            offered.retain(|t| tools::FEW.iter().any(|f| t["name"] == *f));
        }
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let job = Job {
            endpoint: Endpoint::of(p),
            system,
            messages: self.history.clone(),
            offered,
            tx,
            stop: stop.clone(),
            ctx: ctx.clone(),
        };
        std::thread::Builder::new()
            .name("terraflux-assistant".into())
            .spawn(move || job.work())
            .expect("a thread for the assistant");
        self.worker = Some(Worker { rx, stop });
    }

    /// Stop the answer: its thread let go (what it still sends is not taken), the conversation mended.
    pub fn stop(&mut self) {
        if let Some(w) = self.worker.take() {
            w.stop.store(true, Ordering::Relaxed);
            self.items.push(Item::Error("stopped".into()));
        }
        self.run = None;
        mend(&mut self.history);
    }

    /// A new conversation.
    pub fn clear(&mut self) {
        self.stop();
        self.history.clear();
        self.items.clear();
    }

    /// Once a frame: what the thread sent taken, the tools on the model done, the run asked for followed.
    pub fn poll(&mut self, s: &mut Session, r: &Registry, ctx: &egui::Context) {
        self.poll_setup();
        self.follow_run(s, ctx);
        let Some(w) = &self.worker else { return };
        let mut done = false;
        let mut events = Vec::new();
        loop {
            match w.rx.try_recv() {
                Ok(e) => events.push(e),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }
        for e in events {
            match e {
                Event::Delta(d) => {
                    if !matches!(self.items.last(), Some(Item::Assistant { .. })) {
                        self.items.push(Item::Assistant { text: String::new(), thinking: String::new() });
                    }
                    if let Some(Item::Assistant { text, thinking }) = self.items.last_mut() {
                        match d {
                            Delta::Text(t) => text.push_str(&t),
                            Delta::Thinking(t) => thinking.push_str(&t),
                        }
                    }
                }
                Event::Call { name, args } => self.items.push(Item::Tool { name, args, result: None }),
                Event::Result { text, error } => {
                    if let Some(Item::Tool { result, .. }) =
                        self.items.iter_mut().rev().find(|i| matches!(i, Item::Tool { result: None, .. }))
                    {
                        *result = Some((text, error));
                    }
                }
                Event::Ask(a) => self.ask(a, s, r, ctx),
                Event::Turn(m) => self.history.push(m),
                Event::Done => done = true,
                Event::Failed(e) => {
                    self.items.push(Item::Error(e));
                    done = true;
                }
            }
        }
        if done {
            self.worker = None;
            mend(&mut self.history);
            self.answered = true;
        }
        if self.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.tell_problems(s, ctx);
    }

    /// In a conversation going on, the problems that came up told to the assistant, for it to fix them: a run that
    /// failed (whoever started it), and the model's errors when an answer of its ends. Each told once, and not more
    /// than a few times one after another until the user writes.
    fn tell_problems(&mut self, s: &Session, ctx: &egui::Context) {
        if Chat::provider().is_none() {
            return;
        }
        if let Some(text) = self.problems_to_tell(s)
            && let Some(p) = Chat::provider()
        {
            self.send_to(&p, text, s, ctx);
        }
    }

    /// What to tell the assistant of the problems that came up ([`Chat::tell_problems`]), counted as told.
    fn problems_to_tell(&mut self, s: &Session) -> Option<String> {
        if self.quiet || self.busy() || self.run.is_some() || self.history.is_empty() || s.running() || s.loading() {
            return None;
        }
        let mut lines = Vec::new();
        if let Some(e) = &s.error
            && s.started.is_some()
            && s.started != self.told_run
        {
            self.told_run = s.started;
            lines.push(format!("The run failed: {e}"));
        }
        if std::mem::take(&mut self.answered) {
            let errors = problems(s);
            if !errors.is_empty() && !lines.iter().any(|l| errors.contains(l)) {
                lines.extend(errors);
            }
        }
        if lines.is_empty() {
            return None;
        }
        let text = format!(
            "(Told by the application, on its own.) Problems came up in the model:\n{}\nFix them, then check the model.",
            lines.join("\n")
        );
        if self.told.as_deref() == Some(text.as_str()) || self.told_times >= TOLD {
            return None;
        }
        self.told = Some(text.clone());
        self.told_times += 1;
        Some(text)
    }

    /// A tool done on the model open.
    fn ask(&mut self, a: Ask, s: &mut Session, r: &Registry, ctx: &egui::Context) {
        let answer = match a.name.as_str() {
            "show_model" | "check_model" => Answer::Model(Box::new(s.source.clone()), s.base()),
            "run_model" => {
                self.run = Some(RunWait { reply: a.reply, started: false });
                self.follow_run(s, ctx);
                return;
            }
            "new_model" => Answer::Done(new_model(s, r, &a.args, &self.stamp)),
            edit if tools::EDITS.contains(&edit) => Answer::Done(change(s, r, edit, &a.args, &self.stamp)),
            other => Answer::Done(Err(format!("no tool {other:?}"))),
        };
        let _ = a.reply.send(answer);
    }

    /// The run asked for: started when the model can run (its data read, another run ended), answered when it
    /// ends.
    fn follow_run(&mut self, s: &mut Session, ctx: &egui::Context) {
        let Some(w) = &mut self.run else { return };
        if !w.started {
            if s.loading() || s.running() {
                return;
            }
            if !s.can_run() {
                let w = self.run.take().expect("the run asked for");
                let _ = w.reply.send(Answer::Done(Err(cannot_run(s))));
                return;
            }
            let c = ctx.clone();
            s.start(Box::new(move || c.request_repaint()));
            w.started = true;
            return;
        }
        if s.running() {
            return;
        }
        let w = self.run.take().expect("the run asked for");
        // (the assistant hears of this run's failure in the answer to its tool)
        self.told_run = s.started;
        let _ = w.reply.send(Answer::Done(ran(s)));
    }

    fn poll_setup(&mut self) {
        if let Some(rx) = &self.listing
            && let Ok(found) = rx.try_recv()
        {
            (self.listed, self.list_error) = match found {
                Ok(l) => (l, None),
                Err(e) => (Vec::new(), Some(e)),
            };
            self.listing = None;
        }
        if let Some(rx) = &self.testing
            && let Ok(found) = rx.try_recv()
        {
            self.tested = found;
            self.testing = None;
        }
    }
}

/// The conversation mended after an answer cut short: a call of a tool left without its result is answered as
/// not done (the API refuses a conversation with calls unanswered).
fn mend(history: &mut Vec<Value>) {
    let Some(k) = history.iter().rposition(|m| m["role"] == "assistant") else { return };
    let calls: Vec<String> = history[k]["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["id"].as_str().map(String::from))
        .collect();
    for id in calls {
        if !history[k..].iter().any(|m| m["role"] == "tool" && m["tool_call_id"] == id.as_str()) {
            history.push(
                json!({ "role": "tool", "tool_call_id": id, "content": "not done: the user stopped the answer" }),
            );
        }
    }
}

/// What the model is told of the model open: its name and file, its text (or its nodes, when it is long), its
/// problems, and the node selected.
fn context(s: &Session) -> String {
    let mut t = format!("\n\nThe model open in the application: {:?}", s.title());
    match &s.path {
        Some(p) => t.push_str(&format!(" (its file: {})", p.display())),
        None => t.push_str(" (not saved to a file yet)"),
    }
    if s.source.nodes.is_empty() && s.source.inputs.is_empty() {
        t.push_str(". It is empty.");
    } else {
        let toml = s.source.to_toml().unwrap_or_default();
        // (the places of the nodes on the canvas left out)
        let text: Vec<&str> = toml.lines().filter(|l| !l.starts_with("pos = ") && !l.starts_with("width = ")).collect();
        let text = text.join("\n");
        if text.len() <= WHOLE {
            t.push_str(&format!(". As text:\n```toml\n{text}\n```"));
        } else {
            let nodes: Vec<String> = s.source.nodes.iter().map(|n| format!("{} ({})", n.id, n.op)).collect();
            t.push_str(&format!(". Its nodes (show_model gives it whole): {}.", nodes.join(", ")));
        }
    }
    // what each operator of the model does (its documentation's first sentence)
    let mut ops: Vec<&str> = Vec::new();
    fn gather<'a>(nodes: &'a [terraflux_graph::Node], ops: &mut Vec<&'a str>) {
        for n in nodes {
            if !ops.contains(&n.op.as_str()) {
                ops.push(&n.op);
            }
            if let Some(b) = &n.body {
                gather(&b.nodes, ops);
            }
        }
    }
    gather(&s.source.nodes, &mut ops);
    let r = Registry::standard();
    let docs: Vec<String> = ops
        .iter()
        .take(40)
        .filter_map(|op| Some(format!("{op}: {}", tools::first_sentence(&r.sig(op)?.doc))))
        .collect();
    if !docs.is_empty() {
        t.push_str(&format!("\nIts operators:\n- {}", docs.join("\n- ")));
    }
    let problems: Vec<String> = s.issues.iter().take(12).map(|i| i.to_string()).collect();
    if !problems.is_empty() {
        t.push_str(&format!("\nIts problems:\n- {}", problems.join("\n- ")));
    }
    if let Some(id) = &s.selected
        && let Some(n) = s.source.nodes.iter().find(|n| n.id == *id)
    {
        t.push_str(&format!("\nThe node selected on the canvas: {id} ({}).", n.op));
    }
    t
}

/// A change of the model open by one of the [`tools::EDITS`]: made on a copy, taken when it worked (an edit of
/// its own), the node it touched selected.
fn change(s: &mut Session, r: &Registry, name: &str, a: &Value, stamp: &Stamp) -> Reply {
    let mut m = s.source.clone();
    let before: Vec<String> = m.nodes.iter().map(|n| n.id.clone()).collect();
    let done = tools::edit(name, &mut m, r, &s.base(), a)?;
    // (the author and request given by the model itself kept)
    if name != "set_model_info" || (a.get("author").is_none() && a.get("request").is_none()) {
        stamp.on(&mut m);
    }
    crate::review::begin(s, &stamp.who, false);
    s.source = m;
    crate::builder::forget_missing(s);
    s.edited();
    let touched = match name {
        "add_node" => s.source.nodes.iter().find(|n| !before.contains(&n.id)).map(|n| n.id.clone()),
        "edit_node" => tools::opt_text(a, "rename").or(tools::opt_text(a, "node")).map(String::from),
        _ => None,
    };
    if tools::opt_text(a, "scope").is_none()
        && let Some(id) = touched.filter(|id| s.source.nodes.iter().any(|n| n.id == *id))
    {
        crate::builder::select(s, r, &id);
    }
    Ok(format!("{done}\n{}", tools::problems(&s.source, &s.base(), r)))
}

/// A new model in place of the one open (an edit of its own: ctrl+Z goes back).
fn new_model(s: &mut Session, r: &Registry, a: &Value, stamp: &Stamp) -> Reply {
    if !s.source.nodes.is_empty() && !a.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
        return Err(format!(
            "the model open has {} nodes: give overwrite: true to replace it (the user can undo it)",
            s.source.nodes.len()
        ));
    }
    let mut m = match tools::opt_text(a, "template") {
        Some(name) => tools::template_model(r, name)?,
        None => Model::new("Untitled"),
    };
    tools::model_info(&mut m, a);
    if m.name.is_none() {
        m.name = Some("Untitled".into());
    }
    stamp.on(&mut m);
    crate::review::begin(s, &stamp.who, false);
    let inputs = m.inputs != s.source.inputs;
    s.source = m;
    crate::builder::forget_missing(s);
    s.edited();
    if inputs {
        s.read_data();
    }
    let nodes: Vec<String> = s.source.nodes.iter().map(|n| format!("{} ({})", n.id, n.op)).collect();
    let mut t = "the model open is a new one".to_string();
    if !nodes.is_empty() {
        t.push_str(&format!("; its nodes: {}", nodes.join(", ")));
    }
    Ok(format!("{t}\n{}", tools::problems(&s.source, &s.base(), r)))
}

/// Why the model open cannot run.
/// The model's errors: of its graph, its data, the run.
fn problems(s: &Session) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(e) = &s.load_error {
        lines.push(format!("error: its data: {e}"));
    }
    lines.extend(s.issues.iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_string()));
    lines.extend(s.findings.iter().filter(|f| f.level == terraflux_exec::lint::Level::Error).map(|f| f.to_string()));
    lines
}

fn cannot_run(s: &Session) -> String {
    let mut lines = vec!["The model has errors: it does not run.".to_string()];
    lines.extend(problems(s));
    lines.join("\n")
}

/// What a run of the model open gave: how long it took, the nodes that failed, the warnings, what its outputs
/// (else its last nodes) gave.
fn ran(s: &Session) -> Reply {
    if let Some(e) = &s.error {
        return Err(format!("The run failed: {e}"));
    }
    let mut t = format!("Ran in {:.1} s.", s.elapsed.map_or(0.0, |d| d.as_secs_f64()));
    let failed: Vec<&String> = s.status.iter().filter(|(_, st)| **st == NodeStatus::Failed).map(|(n, _)| n).collect();
    if !failed.is_empty() {
        t.push_str(&format!(" Failed: {}.", failed.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")));
    }
    let mut shown: Vec<(String, String, String)> = s
        .model
        .outputs
        .iter()
        .filter_map(|(name, o)| {
            let (node, port) = o.from.split_once('.')?;
            Some((format!("the output {name} ({})", o.from), node.to_string(), port.to_string()))
        })
        .collect();
    if shown.is_empty() {
        // the nodes nothing reads
        let r = Registry::standard();
        for n in
            s.model.nodes.iter().filter(|n| {
                !s.model.nodes.iter().any(|o| terraflux_graph::dependencies(o, &s.model.nodes).contains(&n.id))
            })
        {
            for (port, _) in crate::editor::output_ports(n, &r).into_iter().take(3) {
                shown.push((format!("{}.{port}", n.id), n.id.clone(), port));
            }
        }
    }
    for (what, node, port) in shown.iter().take(12) {
        if let Some(v) = s.value_of(node, port) {
            t.push_str(&format!("\n- {what}: {}", tools::summarise(&v)));
        }
    }
    if !s.warnings.is_empty() {
        t.push_str("\nWarnings:");
        for w in s.warnings.iter().take(20) {
            t.push_str(&format!("\n- {w}"));
        }
    }
    Ok(t)
}

/// An answer worked out in its thread: the model asked, the tools it calls done (here, or by the application),
/// until it answers without calling one.
struct Job {
    endpoint: Endpoint,
    system: String,
    messages: Vec<Value>,
    offered: Vec<Value>,
    tx: Sender<Event>,
    stop: Arc<AtomicBool>,
    ctx: egui::Context,
}

impl Job {
    fn send(&self, e: Event) {
        let _ = self.tx.send(e);
        self.ctx.request_repaint();
    }

    fn work(mut self) {
        let functions = tools::functions(&self.offered);
        let registry = Registry::standard();
        for _ in 0..ROUNDS {
            let mut all = vec![json!({ "role": "system", "content": self.system })];
            all.extend(self.messages.iter().cloned());
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            let mut on = move |d: Delta| {
                let _ = tx.send(Event::Delta(d));
                ctx.request_repaint();
            };
            let answer = match llm::complete(&self.endpoint, &all, &functions, &mut on, &self.stop) {
                Ok(a) => a,
                Err(e) => return self.send(Event::Failed(e)),
            };
            let message = answer.message();
            self.messages.push(message.clone());
            self.send(Event::Turn(message));
            if answer.calls.is_empty() {
                return self.send(Event::Done);
            }
            for call in &answer.calls {
                if self.stop.load(Ordering::Relaxed) {
                    return;
                }
                self.send(Event::Call { name: call.name.clone(), args: call.arguments.clone() });
                let (text, error) = match self.tool(&registry, call) {
                    Ok(t) => (t, false),
                    Err(e) => (e, true),
                };
                self.send(Event::Result { text: text.clone(), error });
                let result = json!({ "role": "tool", "tool_call_id": call.id, "content": text });
                self.messages.push(result.clone());
                self.send(Event::Turn(result));
            }
        }
        self.send(Event::Failed(format!("stopped after {ROUNDS} rounds of tools")));
    }

    /// A tool called: done here when it only reads, else by the application.
    fn tool(&self, r: &Registry, call: &llm::Call) -> Reply {
        if !self.offered.iter().any(|t| t["name"] == call.name.as_str()) {
            return Err(format!("no tool {:?}", call.name));
        }
        let args: Value = if call.arguments.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&call.arguments).map_err(|e| format!("the arguments are not JSON: {e}"))?
        };
        let a = &args;
        // (a failure of an operator ends the call, not the assistant)
        let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match call.name.as_str() {
            "list_templates" => Ok(tools::list_templates()),
            "list_operators" => Ok(tools::list_operators(r, tools::opt_text(a, "query"))),
            "describe_operator" => tools::describe_operator(r, tools::text(a, "name")?),
            "inspect_file" => tools::inspect_file(a),
            name => match self.ask(name, a)? {
                Answer::Done(reply) => reply,
                Answer::Model(m, dir) => match name {
                    "show_model" => tools::show_model(&m, &dir, r),
                    _ => tools::check_model(r, &m, &dir),
                },
            },
        }));
        done.unwrap_or_else(|p| {
            let why = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            Err(format!("Terraflux failed: {why}"))
        })
    }

    /// A tool done by the application, waited for.
    fn ask(&self, name: &str, args: &Value) -> Result<Answer, String> {
        let (reply, answer) = channel();
        self.send(Event::Ask(Ask { name: name.into(), args: args.clone(), reply }));
        answer.recv().map_err(|_| "not done: the answer was stopped".to_string())
    }
}

/// The assistant's pane: its server at the top (and its settings), the conversation, the changes to review, the
/// question typed. Whether the review of the changes is asked for.
pub fn pane(ui: &mut Ui, c: &mut Chat, s: &mut Session) -> bool {
    let fill = ui.visuals().panel_fill;
    let mut review = false;
    egui::Panel::top("assistant bar")
        .frame(egui::Frame::new().fill(fill).inner_margin(Margin::symmetric(8, 5)))
        .show(ui, |ui| bar(ui, c));
    egui::Panel::bottom("assistant input")
        .frame(egui::Frame::new().fill(fill).inner_margin(Margin::symmetric(8, 4)))
        .show(ui, |ui| {
            review = to_review(ui, s);
            input(ui, c, s);
        });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(fill).inner_margin(Margin::symmetric(8, 0))).show(
        ui,
        |ui| {
            if c.setup || Chat::provider().is_none() {
                egui::ScrollArea::vertical().id_salt("assistant setup").show(ui, |ui| setup(ui, c));
            } else {
                conversation(ui, c);
            }
        },
    );
    review
}

/// The changes made to the model and not reviewed yet: how many, to review, accept or reject. Whether the review
/// is asked for.
fn to_review(ui: &mut Ui, s: &mut Session) -> bool {
    let n = crate::review::changes(s).len();
    if n == 0 {
        return false;
    }
    let mut asked = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{n} change(s) to review")).color(crate::review::CHANGED));
        asked = ui.button("Review").on_hover_text("each change, before and after").clicked();
        if ui.button("Accept all").clicked() {
            crate::review::accept_all(s);
        }
        if ui.button("Reject all").on_hover_text("the model as it was (ctrl+Z goes back)").clicked() {
            crate::review::reject_all(s);
        }
    });
    ui.separator();
    asked
}

fn bar(ui: &mut Ui, c: &mut Chat) {
    ui.horizontal(|ui| {
        let st = crate::settings::get();
        let chosen = Chat::provider();
        let mut k = st.provider;
        egui::ComboBox::from_id_salt("assistant provider")
            .selected_text(chosen.as_ref().map_or("Choose a server...".to_string(), |p| p.name.clone()))
            .width(200.0)
            .show_ui(ui, |ui| {
                for (i, p) in st.providers.iter().enumerate() {
                    ui.selectable_value(&mut k, Some(i), &p.name);
                }
            });
        if k != st.provider {
            crate::settings::update(|s| s.provider = k);
            c.tested.clear();
            c.listed.clear();
            c.list_error = None;
            c.key_text.clear();
        }
        if let Some(p) = &chosen {
            ui.label(RichText::new(&p.model).weak().small());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.selectable_label(c.setup, "⚙").on_hover_text("the server's settings").clicked() {
                c.setup = !c.setup;
            }
            if ui.button("New").on_hover_text("a new conversation").clicked() {
                c.clear();
            }
            let mut tell = !c.quiet;
            if ui
                .checkbox(&mut tell, "fix problems")
                .on_hover_text("tell the assistant of the problems that come up (a run that fails, the model's errors after its answer) for it to fix them, in a conversation going on")
                .changed()
            {
                c.quiet = !tell;
            }
        });
    });
}

/// The settings of the server chosen: its address, model, key, and a test of it.
fn setup(ui: &mut Ui, c: &mut Chat) {
    egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
        theme::section(ui, "The assistant's server");
        ui.label(
            RichText::new(
                "Any server of OpenAI's chat completions API: a university's (as the HU Berlin's), one on this \
                 computer (Ollama, LM Studio), or a commercial provider's. The model needs to call tools.",
            )
            .weak(),
        );
        ui.add_space(6.0);
        let st = crate::settings::get();
        let Some(k) = st.provider.filter(|k| *k < st.providers.len()) else {
            ui.label("Choose a server above.");
            for (i, p) in st.providers.iter().enumerate() {
                if ui.button(&p.name).on_hover_text(&p.about).clicked() {
                    crate::settings::update(|s| s.provider = Some(i));
                }
            }
            return;
        };
        let before = st.providers[k].clone();
        let mut p = before.clone();
        egui::Grid::new("assistant server").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(f32::INFINITY));
            ui.end_row();
            ui.label("Address");
            ui.add(egui::TextEdit::singleline(&mut p.base_url).desired_width(f32::INFINITY))
                .on_hover_text("the API's address, up to its version: https://.../v1");
            ui.end_row();
            ui.label("Model");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut p.model).desired_width(180.0));
                if !c.listed.is_empty() {
                    egui::ComboBox::from_id_salt("assistant models").selected_text("found...").show_ui(ui, |ui| {
                        for m in &c.listed {
                            ui.selectable_value(&mut p.model, m.clone(), m);
                        }
                    });
                }
                if c.listing.is_some() {
                    ui.spinner();
                } else if ui.button("List").on_hover_text("the server's models").clicked() {
                    let (tx, rx) = channel();
                    let e = Endpoint::of(&p);
                    std::thread::spawn(move || {
                        let _ = tx.send(llm::models(&e));
                    });
                    c.listing = Some(rx);
                }
            });
            ui.end_row();
            ui.label("Key");
            ui.vertical(|ui| {
                ui.checkbox(&mut p.key, "the server asks for a key");
                if p.key {
                    ui.horizontal(|ui| {
                        let kept = llm::key(&p).is_some();
                        ui.add(
                            egui::TextEdit::singleline(&mut c.key_text)
                                .password(true)
                                .desired_width(180.0)
                                .hint_text(if kept { "kept (a new one replaces it)" } else { "the key" }),
                        );
                        if ui.add_enabled(!c.key_text.trim().is_empty(), egui::Button::new("Keep")).clicked() {
                            c.note = Some(match llm::set_key(&p, &c.key_text) {
                                Ok(()) => "the key is kept in the system's store of credentials".into(),
                                Err(e) => format!("the key was not kept: {e}"),
                            });
                            c.key_text.clear();
                        }
                        if kept && ui.button("Forget").clicked() {
                            c.note = llm::set_key(&p, "").err();
                        }
                    });
                }
            });
            ui.end_row();
            ui.label("");
            ui.vertical(|ui| {
                ui.checkbox(&mut p.thinking, "Think before answering")
                    .on_hover_text("Qwen 3 and other models served by vLLM: slower, often better");
                ui.checkbox(&mut p.few_tools, "Few tools")
                    .on_hover_text("only to explain, change settings, run and check: for small models");
            });
            ui.end_row();
        });
        if !p.about.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(&p.about).weak().small());
        }
        if p != before {
            crate::settings::update(|s| s.providers[k] = p.clone());
        }
        if let Some(n) = &c.note {
            ui.label(RichText::new(n).weak());
        }
        if let Some(e) = &c.list_error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if c.testing.is_some() {
                ui.spinner();
                ui.label("testing...");
            } else if ui
                .add(crate::app::accent_button("Test the connection", true))
                .on_hover_text("whether the server answers, has the model, and lets it call tools")
                .clicked()
            {
                let (tx, rx) = channel();
                let e = Endpoint::of(&p);
                std::thread::spawn(move || {
                    let _ = tx.send(llm::test(&e));
                });
                c.testing = Some(rx);
                c.tested.clear();
            }
            if ui.button("Done").clicked() {
                c.setup = false;
            }
        });
        for (ok, what) in &c.tested {
            let (mark, colour) =
                if *ok { ("✔", Color32::from_rgb(92, 196, 92)) } else { ("✖", ui.visuals().error_fg_color) };
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(colour, mark);
                ui.label(what);
            });
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Add a server").clicked() {
                crate::settings::update(|s| {
                    let mut new = llm::presets().pop().expect("the presets");
                    new.name = format!("Server {}", s.providers.len() + 1);
                    s.providers.push(new);
                    s.provider = Some(s.providers.len() - 1);
                });
            }
            if ui.button("Remove this one").clicked() {
                crate::settings::update(|s| {
                    s.providers.remove(k);
                    s.provider = None;
                });
            }
            if ui.button("The servers as they first were").clicked() {
                crate::settings::update(|s| {
                    s.providers = llm::presets();
                    s.provider = None;
                });
            }
        });
    });
}

/// The conversation, the last at the bottom.
fn conversation(ui: &mut Ui, c: &Chat) {
    egui::ScrollArea::vertical().id_salt("assistant conversation").stick_to_bottom(true).auto_shrink(false).show(
        ui,
        |ui| {
            ui.add_space(6.0);
            if c.items.is_empty() {
                ui.label(
                    RichText::new(
                        "Ask about the model open, or for a change of it: \"what does this model do?\", \"why this \
                         threshold?\", \"add a map of the distance to roads\", \"run it and tell me the area of \
                         each class\". Each change shows on the canvas at once, and ctrl+Z undoes it.",
                    )
                    .weak(),
                );
            }
            for (k, item) in c.items.iter().enumerate() {
                show_item(ui, k, item);
                ui.add_space(6.0);
            }
            if c.busy() && !matches!(c.items.last(), Some(Item::Assistant { text, .. }) if !text.is_empty()) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("working...").weak());
                });
            }
        },
    );
}

fn show_item(ui: &mut Ui, k: usize, item: &Item) {
    match item {
        Item::User(t) => {
            egui::Frame::new().fill(theme::well()).corner_radius(6.0).inner_margin(Margin::symmetric(8, 6)).show(
                ui,
                |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(t);
                },
            );
        }
        Item::Assistant { text, thinking } => {
            if !thinking.trim().is_empty() {
                egui::CollapsingHeader::new(RichText::new("thinking").weak().small())
                    .id_salt(("assistant thinking", k))
                    .show(ui, |ui| ui.label(RichText::new(thinking.trim()).weak().small()));
            }
            text_with_code(ui, text);
        }
        Item::Tool { name, args, result } => {
            let (colour, mark) = match result {
                None => (ui.visuals().weak_text_color(), "…"),
                Some((_, false)) => (theme::accent(), "⚙"),
                Some((_, true)) => (ui.visuals().error_fg_color, "✖"),
            };
            let short: String = args.chars().take(90).collect();
            let more = if args.chars().count() > 90 { "…" } else { "" };
            egui::CollapsingHeader::new(
                RichText::new(format!("{mark} {name} {short}{more}")).color(colour).small().monospace(),
            )
            .id_salt(("assistant tool", k))
            .show(ui, |ui| {
                ui.label(RichText::new(args).small().monospace());
                if let Some((t, _)) = result {
                    ui.separator();
                    ui.label(RichText::new(t).small().monospace());
                }
            });
        }
        Item::Error(e) => {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
    }
}

/// A text with its blocks of code (between ```) in a fixed font.
fn text_with_code(ui: &mut Ui, text: &str) {
    for (k, part) in text.split("```").enumerate() {
        if k % 2 == 1 {
            // (the block's language, on its first line, left out)
            let code =
                part.split_once('\n').map_or(part, |(first, rest)| if first.contains(' ') { part } else { rest });
            egui::Frame::new().fill(theme::well()).corner_radius(4.0).inner_margin(Margin::same(6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(code.trim_end()).monospace().small());
            });
        } else {
            markdown(ui, part.trim());
        }
    }
}

/// A text written in Markdown, its usual parts shown: headings, lists, **strong** and `code`.
fn markdown(ui: &mut Ui, text: &str) {
    let mut blank = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            if !blank {
                ui.add_space(4.0);
            }
            blank = true;
            continue;
        }
        blank = false;
        let indent = (line.len() - trimmed.len()) as f32 * 4.0;
        let heading = trimmed.starts_with('#');
        let (lead, rest) = if heading {
            ("", trimmed.trim_start_matches('#').trim_start())
        } else if let Some(r) = trimmed.strip_prefix("* ").or(trimmed.strip_prefix("- ")) {
            ("•  ", r)
        } else {
            ("", trimmed)
        };
        let mut job = egui::text::LayoutJob::default();
        let style = ui.style().clone();
        let mut piece = |t: &str, strong: bool, code: bool| {
            let mut rt = RichText::new(t);
            if strong || heading {
                rt = rt.strong();
            }
            if code {
                rt = rt.monospace().background_color(theme::well());
            }
            rt.append_to(&mut job, &style, egui::FontSelection::Default, egui::Align::Center);
        };
        piece(lead, false, false);
        for (k, chunk) in rest.split('`').enumerate() {
            if k % 2 == 1 {
                piece(chunk, false, true);
                continue;
            }
            for (j, part) in chunk.split("**").enumerate() {
                if !part.is_empty() {
                    piece(part, j % 2 == 1, false);
                }
            }
        }
        job.wrap.max_width = ui.available_width() - indent;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.add(egui::Label::new(job).wrap());
        });
        if heading {
            ui.add_space(2.0);
        }
    }
}

/// The question typed (enter sends it, shift+enter goes to a new line), and the buttons to send or stop.
fn input(ui: &mut Ui, c: &mut Chat, s: &mut Session) {
    ui.add_space(4.0);
    let id = ui.make_persistent_id("assistant question");
    let typing = ui.memory(|m| m.has_focus(id));
    let mut send = typing && !c.busy() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
    ui.add(
        egui::TextEdit::multiline(&mut c.input)
            .id(id)
            .desired_rows(3)
            .desired_width(f32::INFINITY)
            .hint_text("Ask the assistant (enter sends, shift+enter: a new line)"),
    );
    ui.horizontal(|ui| {
        if c.busy() {
            if ui.button("Stop").clicked() {
                c.stop();
            }
        } else {
            send |= ui.add(crate::app::accent_button("Send", true)).clicked();
            if let Some(id) = s.selected.clone()
                && ui
                    .add(crate::app::accent_button(&format!("Explain {id}"), true))
                    .on_hover_text("what the node selected does, and why")
                    .clicked()
            {
                c.input =
                    format!("Explain the node {id}: what it does, why it is there, what it reads and what it gives.");
                send = true;
            }
        }
        if let Some(p) = Chat::provider() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(&p.name).weak().small());
            });
        }
    });
    ui.add_space(2.0);
    if send && !c.input.trim().is_empty() {
        let text = std::mem::take(&mut c.input);
        c.send(text, s, ui.ctx());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conversation_cut_short_is_mended() {
        let mut h = vec![
            json!({ "role": "user", "content": "go" }),
            json!({ "role": "assistant", "content": "", "tool_calls": [
                { "id": "a", "type": "function", "function": { "name": "show_model", "arguments": "{}" } },
                { "id": "b", "type": "function", "function": { "name": "run_model", "arguments": "{}" } },
            ] }),
            json!({ "role": "tool", "tool_call_id": "a", "content": "the model" }),
        ];
        mend(&mut h);
        assert_eq!(h.len(), 4);
        assert_eq!(h[3]["tool_call_id"], "b");
        mend(&mut h);
        assert_eq!(h.len(), 4, "mended once");
    }

    #[test]
    fn the_problems_that_come_up_are_told() {
        let mut c = Chat::default();
        let mut s = Session::blank();
        while s.loading() {
            s.poll();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        s.error = Some("node x: no input".into());
        s.started = Some(std::time::Instant::now());
        assert!(c.problems_to_tell(&s).is_none(), "no conversation going on");
        c.history.push(json!({ "role": "user", "content": "hi" }));
        let t = c.problems_to_tell(&s).unwrap();
        assert!(t.contains("The run failed: node x: no input"), "{t}");
        assert!(c.problems_to_tell(&s).is_none(), "a failure told once");
        // an answer ended, the model in error
        s.issues.push(terraflux_graph::Issue { severity: Severity::Error, at: "a".into(), message: "needs b".into() });
        c.answered = true;
        assert!(c.problems_to_tell(&s).unwrap().contains("needs b"));
        c.answered = true;
        assert!(c.problems_to_tell(&s).is_none(), "the same problems not told again");
        s.issues[0].message = "needs c".into();
        c.answered = true;
        assert!(c.problems_to_tell(&s).is_some());
        // three times one after another: no more until the user writes
        s.issues[0].message = "needs d".into();
        c.answered = true;
        assert!(c.problems_to_tell(&s).is_none());
        c.told_times = 0;
        c.quiet = true;
        c.answered = true;
        assert!(c.problems_to_tell(&s).is_none(), "not told when asked not to");
    }

    #[test]
    fn the_tools_change_the_model_open() {
        let r = Registry::standard();
        let mut s = Session::blank();
        let stamp = Stamp { who: "the assistant (qwen)".into(), asked: "a landscape, please".into() };
        let added = change(
            &mut s,
            &r,
            "add_node",
            &json!({ "op": "nlm.random", "id": "land", "settings": { "rows": 10, "columns": 10 }, "why": "no data" }),
            &stamp,
        )
        .unwrap();
        assert!(added.contains("land.map"), "{added}");
        assert!(s.history.can_undo() && s.dirty);
        assert_eq!(s.selected.as_deref(), Some("land"), "the node added is selected");
        assert_eq!(s.source.node("land").unwrap().why.as_deref(), Some("no data"));
        // signed: the assistant the author, the first question the request
        assert_eq!(s.source.author.as_deref(), Some("the assistant (qwen)"));
        assert_eq!(s.source.request.as_deref(), Some("a landscape, please"));
        s.source.author = Some("Ana".into());
        change(&mut s, &r, "edit_node", &json!({ "node": "land", "note": "the land" }), &stamp).unwrap();
        assert_eq!(s.source.author.as_deref(), Some("Ana; the assistant (qwen)"), "both");
        // a wrong change: refused, the model as it was
        let before = s.source.clone();
        let wrong = json!({ "op": "map.calc", "inputs": { "x": "nothing.map" } });
        assert!(change(&mut s, &r, "add_node", &wrong, &stamp).is_err());
        assert_eq!(s.source, before);
        // a new model only when asked to replace the one open
        assert!(new_model(&mut s, &r, &json!({}), &stamp).is_err());
        let t = crate::templates::TEMPLATES[0].name;
        new_model(&mut s, &r, &json!({ "template": t, "overwrite": true }), &stamp).unwrap();
        assert!(!s.source.nodes.is_empty() && s.source.node("land").is_none());
        let told = context(&s);
        assert!(told.contains("```toml") && !told.contains("pos = "), "{told}");
    }

    #[test]
    fn the_tools_offered_in_the_application_edit_the_model_open() {
        let all = tools::catalogue(true);
        assert!(all.iter().all(|t| t["inputSchema"]["properties"].get("model").is_none()));
        assert!(!all.iter().any(|t| t["name"] == "open_in_app"));
        for f in tools::FEW {
            assert!(all.iter().any(|t| t["name"] == f), "{f}");
        }
        assert_eq!(tools::functions(&all)[0]["type"], "function");
    }

    /// The Humboldt-Universität's model builds a small model and runs it, when this computer reaches it (its
    /// network or VPN): `cargo test -p terraflux-app hu_berlin -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn hu_berlin_builds_and_runs() {
        let r = Registry::standard();
        let ctx = egui::Context::default();
        let p = llm::presets().into_iter().next().unwrap();
        let mut s = Session::blank();
        let mut c = Chat::default();
        c.send_to(
            &p,
            "Build a model: a random neutral landscape of 60 x 40 cells (seed 7), and a map.calc node marking the \
             cells above 0.5 with 1, else 0. Then run it and tell me the share of the cells marked."
                .into(),
            &s,
            &ctx,
        );
        let t = std::time::Instant::now();
        while c.busy() {
            assert!(t.elapsed().as_secs() < 300, "no answer in 5 minutes");
            s.poll();
            c.poll(&mut s, &r, &ctx);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        for i in &c.items {
            println!("{i:?}\n");
        }
        assert!(s.source.nodes.iter().any(|n| n.op.starts_with("nlm.")), "a landscape drawn");
        assert!(s.source.nodes.iter().any(|n| n.op == "map.calc"), "a map.calc");
        let ran = c
            .items
            .iter()
            .any(|i| matches!(i, Item::Tool { name, result: Some((_, false)), .. } if name == "run_model"));
        assert!(ran, "the model was run");
        assert!(matches!(c.items.last(), Some(Item::Assistant { text, .. }) if !text.is_empty()));
    }
}
