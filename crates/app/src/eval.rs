//! The assistant evaluated on a fixed set of tasks: each asked of a server's model in a model of its own (a blank
//! one, or the one the task starts from), the assistant left to answer as in the application (its tools, the runs
//! it asks for), then what it made checked: the nodes, their settings, what the run gave, the answer. The same tasks
//! for every model, so models are compared on them; the last score of each kept with the settings and shown with
//! its server in the Assistant's settings.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use terraflux_graph::{Model, Node, Severity};
use terraflux_ops::{Registry, Value};

use crate::app::Session;
use crate::chat::{Chat, Item};
use crate::llm::Provider;

/// A task: its name, what is asked, the model it starts from (none: a blank one), and the check of what was made.
pub struct Task {
    pub name: &'static str,
    pub ask: &'static str,
    pub start: Option<fn() -> Model>,
    pub check: fn(&Session, &Chat, &Registry) -> Result<(), String>,
}

/// How long a task may take.
const PATIENCE: Duration = Duration::from_secs(600);

/// The tasks: drawing and running, a threshold checked cell by cell, a template changed, a model fixed, a node
/// explained without a change, metrics, a rename that keeps the connections, a loop.
pub const TASKS: &[Task] = &[
    Task {
        name: "a neutral landscape",
        ask: "Draw a random cluster neutral landscape of 50 x 50 cells with the seed 3, then run the model.",
        start: None,
        check: neutral,
    },
    Task {
        name: "a threshold",
        ask: "Make a Gaussian random field of 40 x 40 cells (seed 1) and, in a raster.calc node, mark with 1 the cells \
              above 0.6 and with 0 the others. Run the model.",
        start: None,
        check: threshold,
    },
    Task {
        name: "a template changed",
        ask: "Start from the template Neutral landscapes and set the autocorrelation range of its Gaussian field to \
              30 cells. Run the model.",
        start: None,
        check: template,
    },
    Task {
        name: "a model fixed",
        ask: "The model open does not run. Find why, fix it and run it.",
        start: Some(broken),
        check: fixed,
    },
    Task {
        name: "an explanation",
        ask: "In one or two sentences: what does the node field make? Do not change the model.",
        start: Some(neutral_landscapes),
        check: explained,
    },
    Task {
        name: "landscape metrics",
        ask: "Draw a random cluster landscape of 60 x 60 cells (seed 2) and compute its landscape metrics: the number \
              of patches and the edge density. Run the model.",
        start: None,
        check: metrics,
    },
    Task {
        name: "a rename",
        ask: "Rename the node a to habitat, keeping what reads it working.",
        start: Some(two_nodes),
        check: renamed,
    },
    Task {
        name: "a loop",
        ask: "Make a loop that runs 5 times and adds 2 to a number carried from step to step, starting at 0. Run it \
              and tell me the number at the end.",
        start: None,
        check: looped,
    },
];

fn broken() -> Model {
    let mut m = Model::new("broken");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 20).param("columns", 20).param("seed", 2));
    m.nodes.push(Node::new("high", "raster.calc").input("x", "land.mapp").param("expression", "x > 0.5"));
    m
}

fn neutral_landscapes() -> Model {
    crate::templates::TEMPLATES.iter().find(|t| t.name == "Neutral landscapes").expect("the template").model()
}

fn two_nodes() -> Model {
    let mut m = Model::new("two nodes");
    m.nodes.push(Node::new("a", "nlm.random").param("rows", 30).param("columns", 30).param("seed", 4));
    m.nodes.push(Node::new("b", "raster.calc").input("x", "a.map").param("expression", "x * 2"));
    m
}

/// Every node of the model, inside its containers too.
fn nodes(m: &Model) -> Vec<&Node> {
    fn add<'a>(ns: &'a [Node], out: &mut Vec<&'a Node>) {
        for n in ns {
            out.push(n);
            if let Some(b) = &n.body {
                add(&b.nodes, out);
            }
        }
    }
    let mut out = Vec::new();
    add(&m.nodes, &mut out);
    out
}

/// The top-level node of an operator.
fn node<'a>(s: &'a Session, op: &str) -> Result<&'a Node, String> {
    s.source.nodes.iter().find(|n| n.op == op).ok_or_else(|| format!("no {op} node"))
}

/// A setting as a number.
fn number(n: &Node, name: &str) -> Option<f64> {
    n.params.get(name)?.as_f64()
}

/// What a node gave in the last run (its first output, or `port`), the run having ended without an error.
fn gave(s: &Session, r: &Registry, n: &Node, port: Option<&str>) -> Result<Value, String> {
    if let Some(e) = &s.error {
        return Err(format!("the run failed: {e}"));
    }
    let port = match port {
        Some(p) => p.to_string(),
        None => crate::builder::first_output(n, r).ok_or_else(|| format!("{} gives nothing", n.id))?,
    };
    s.value_of(&n.id, &port).ok_or_else(|| format!("{}.{port} was not run", n.id))
}

/// The model without errors.
fn sound(s: &Session, r: &Registry) -> Result<(), String> {
    let errors: Vec<String> = terraflux_graph::check(&s.source, r)
        .into_iter()
        .filter(|i| i.severity == Severity::Error)
        .map(|i| i.to_string())
        .collect();
    if errors.is_empty() { Ok(()) } else { Err(format!("the model has errors: {}", errors.join("; "))) }
}

/// The assistant's last answer.
fn answer(c: &Chat) -> String {
    c.items
        .iter()
        .rev()
        .find_map(|i| match i {
            Item::Assistant { text, .. } if !text.trim().is_empty() => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn neutral(s: &Session, _: &Chat, r: &Registry) -> Result<(), String> {
    let n = node(s, "nlm.random_cluster")?;
    let size = (number(n, "rows"), number(n, "columns"), number(n, "seed"));
    if size != (Some(50.0), Some(50.0), Some(3.0)) {
        return Err(format!("rows, columns and seed {size:?}, not 50, 50 and 3"));
    }
    match gave(s, r, n, Some("map"))? {
        Value::Map(m) if m.data.len() == 2500 => Ok(()),
        _ => Err("its map is not of 50 x 50 cells".into()),
    }
}

fn threshold(s: &Session, _: &Chat, r: &Registry) -> Result<(), String> {
    let field = node(s, "nlm.gaussian_field")?;
    let Value::Map(f) = gave(s, r, field, Some("map"))? else { return Err("the field is not a map".into()) };
    if f.data.len() != 1600 {
        return Err(format!("the field has {} cells, not 40 x 40", f.data.len()));
    }
    let calcs: Vec<&Node> = nodes(&s.source).into_iter().filter(|n| n.op == "raster.calc").collect();
    let marked = calcs
        .iter()
        .find_map(|n| match s.value_of(&n.id, "map") {
            Some(Value::Map(m)) if m.data.len() == f.data.len() => Some(m),
            _ => None,
        })
        .ok_or("no raster.calc gave a map of the field's cells")?;
    let wrong = f
        .data
        .iter()
        .zip(marked.data.iter())
        .filter(|(v, m)| v.is_finite() && **m != if **v > 0.6 { 1.0 } else { 0.0 })
        .count();
    if wrong > 0 { Err(format!("{wrong} cells marked otherwise than field > 0.6")) } else { Ok(()) }
}

fn template(s: &Session, _: &Chat, r: &Registry) -> Result<(), String> {
    for op in ["nlm.random_cluster", "nlm.fbm"] {
        node(s, op).map_err(|_| format!("the template's {op} is gone"))?;
    }
    let field = node(s, "nlm.gaussian_field")?;
    if number(field, "range") != Some(30.0) {
        return Err(format!("the range is {:?}, not 30", field.params.get("range")));
    }
    gave(s, r, field, Some("map")).map(|_| ())
}

fn fixed(s: &Session, _: &Chat, r: &Registry) -> Result<(), String> {
    sound(s, r)?;
    let high = s.source.nodes.iter().find(|n| n.op == "raster.calc").ok_or("the raster.calc is gone")?;
    gave(s, r, high, None).map(|_| ())
}

fn explained(s: &Session, c: &Chat, _: &Registry) -> Result<(), String> {
    if s.source.nodes != neutral_landscapes().nodes {
        return Err("the model was changed".into());
    }
    let a = answer(c).to_lowercase();
    if a.contains("gaussian") || a.contains("autocorrelat") || a.contains("smooth") {
        Ok(())
    } else {
        Err(format!("the answer does not say it is a Gaussian field: {:?}", answer(c)))
    }
}

fn metrics(s: &Session, _: &Chat, r: &Registry) -> Result<(), String> {
    let cluster = node(s, "nlm.random_cluster")?;
    if (number(cluster, "rows"), number(cluster, "columns")) != (Some(60.0), Some(60.0)) {
        return Err("the landscape is not of 60 x 60 cells".into());
    }
    let m = s
        .source
        .nodes
        .iter()
        .find(|n| {
            n.op.starts_with("metrics.")
                && terraflux_graph::references(n).iter().any(|x| x.starts_with(&format!("{}.", cluster.id)))
        })
        .ok_or("no metrics node reads the landscape")?;
    match gave(s, r, m, None)? {
        Value::Table(t) if !t.rows.is_empty() => Ok(()),
        Value::Table(_) => Err("the metrics' table is empty".into()),
        _ => Ok(()),
    }
}

fn renamed(s: &Session, _: &Chat, r: &Registry) -> Result<(), String> {
    if s.source.nodes.iter().any(|n| n.id == "a") {
        return Err("a node a is still there".into());
    }
    let h = s.source.nodes.iter().find(|n| n.id == "habitat").ok_or("no node habitat")?;
    if h.op != "nlm.random" {
        return Err(format!("habitat is a {}, not the nlm.random", h.op));
    }
    let b = s.source.nodes.iter().find(|n| n.id == "b").ok_or("b is gone")?;
    if !terraflux_graph::references(b).iter().any(|x| x == "habitat.map") {
        return Err("b does not read habitat.map".into());
    }
    sound(s, r)
}

fn looped(s: &Session, c: &Chat, _: &Registry) -> Result<(), String> {
    let l = s.source.nodes.iter().find(|n| n.op == "control.repeat").ok_or("no loop (control.repeat)")?;
    if let Some(e) = &s.error {
        return Err(format!("the run failed: {e}"));
    }
    let carried: Vec<&String> = l.body.as_ref().map(|b| b.carry.keys().collect()).unwrap_or_default();
    let ten = carried.iter().any(|k| s.value_of(&l.id, k).and_then(|v| v.as_number()) == Some(10.0));
    if !ten {
        return Err("no number carried by the loop ends at 10".into());
    }
    if answer(c).contains("10") { Ok(()) } else { Err("the answer does not tell 10".into()) }
}

/// How a task went.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Outcome {
    pub task: String,
    /// Why it failed (none: passed).
    pub failed: Option<String>,
    pub seconds: f64,
    /// The tools the assistant called.
    pub tools: usize,
}

impl Outcome {
    pub fn passed(&self) -> bool {
        self.failed.is_none()
    }
}

/// Do a task with a server's model, in a session of its own (the problems that come up not told: the assistant
/// alone), until it answers and the runs it asked for end.
pub fn one(p: &Provider, r: &Registry, t: &Task, stop: &AtomicBool) -> Outcome {
    let ctx = egui::Context::default();
    let started = Instant::now();
    let outcome = |failed: Option<String>, c: &Chat| Outcome {
        task: t.name.to_string(),
        failed,
        seconds: started.elapsed().as_secs_f64(),
        tools: c.items.iter().filter(|i| matches!(i, Item::Tool { .. })).count(),
    };
    let mut c = Chat::default();
    c.quiet = true;
    let mut s = match t.start.map(|f| Session::from_model(f())) {
        None => Session::blank(),
        Some(Ok(s)) => s,
        Some(Err(e)) => return outcome(Some(format!("the task's model: {e}")), &c),
    };
    c.send_to(p, t.ask.to_string(), &s, &ctx);
    loop {
        s.poll();
        c.poll(&mut s, r, &ctx);
        if !c.busy() && !s.running() && !s.loading() {
            break;
        }
        if stop.load(Ordering::Relaxed) {
            c.stop();
            return outcome(Some("stopped".into()), &c);
        }
        if started.elapsed() > PATIENCE {
            c.stop();
            return outcome(Some(format!("no answer in {} minutes", PATIENCE.as_secs() / 60)), &c);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let failed = match c.items.last() {
        Some(Item::Error(e)) => Some(format!("the assistant failed: {e}")),
        _ => (t.check)(&s, &c, r).err(),
    };
    outcome(failed, &c)
}

/// What an evaluation running sends.
pub enum Event {
    /// A task begun (its index in [`TASKS`]).
    Began(usize),
    Ended(usize, Outcome),
    Done,
}

/// Do every task with a server's model, one after another, telling how each goes.
pub fn run(p: Provider, tx: Sender<Event>, stop: Arc<AtomicBool>, ctx: egui::Context) {
    let r = Registry::standard();
    let mut done = Vec::new();
    for (k, t) in TASKS.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let _ = tx.send(Event::Began(k));
        ctx.request_repaint();
        let o = one(&p, &r, t, &stop);
        done.push(o.clone());
        let _ = tx.send(Event::Ended(k, o));
        ctx.request_repaint();
    }
    if done.len() == TASKS.len() && !stop.load(Ordering::Relaxed) {
        keep(&p, &done);
    }
    let _ = tx.send(Event::Done);
    ctx.request_repaint();
}

/// The last evaluation of a model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Score {
    /// When (YYYY-MM-DD).
    pub date: String,
    pub outcomes: Vec<Outcome>,
}

impl Score {
    pub fn passed(&self) -> usize {
        self.outcomes.iter().filter(|o| o.passed()).count()
    }
}

/// The model evaluated, by its server and name.
pub fn key_of(p: &Provider) -> String {
    format!("{} on {}", p.model.trim(), p.base_url.trim())
}

fn file() -> std::path::PathBuf {
    crate::settings::data_dir().join("assistant-evaluations.json")
}

/// The last evaluation of each model.
pub fn kept() -> BTreeMap<String, Score> {
    std::fs::read_to_string(file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Keep an evaluation as its model's last.
pub fn keep(p: &Provider, outcomes: &[Outcome]) {
    let mut all = kept();
    let date = chrono_date();
    all.insert(key_of(p), Score { date, outcomes: outcomes.to_vec() });
    let _ = std::fs::create_dir_all(crate::settings::data_dir());
    let _ = std::fs::write(file(), serde_json::to_string_pretty(&all).unwrap_or_default());
}

/// Today, as YYYY-MM-DD (UTC).
fn chrono_date() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400);
    // (the civil date of a day count, Howard Hinnant's algorithm)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A session with a model as an assistant would leave it, run.
    fn ran(m: Model) -> Session {
        let mut s = Session::from_model(m).unwrap();
        s.start(Box::new(|| {}));
        let t = Instant::now();
        while s.running() || s.loading() {
            assert!(t.elapsed().as_secs() < 120);
            s.poll();
            std::thread::sleep(Duration::from_millis(10));
        }
        s
    }

    fn said(text: &str) -> Chat {
        let mut c = Chat::default();
        c.items = vec![Item::Assistant { text: text.into(), thinking: String::new() }];
        c
    }

    #[test]
    fn the_tasks_check_what_was_made() {
        let r = Registry::standard();
        let none = Chat::default();
        // a threshold made right passes, a wrong one fails cell by cell
        let mut m = Model::new("t");
        m.nodes.push(Node::new("field", "nlm.gaussian_field").param("rows", 40).param("columns", 40).param("seed", 1));
        m.nodes.push(Node::new("mark", "raster.calc").input("x", "field.map").param("expression", "x > 0.6"));
        assert_eq!(threshold(&ran(m.clone()), &none, &r), Ok(()));
        m.nodes[1].params.insert("expression".into(), "x > 0.5".into());
        let e = threshold(&ran(m), &none, &r).unwrap_err();
        assert!(e.contains("cells marked otherwise"), "{e}");
        // a blank model fails each task with its reason
        let blank = Session::blank();
        for t in TASKS {
            assert!((t.check)(&blank, &none, &r).is_err(), "{}", t.name);
        }
        // the model fixed and run
        let mut m = broken();
        m.nodes[1].inputs.insert("x".into(), terraflux_graph::Source::Ref("land.map".into()));
        assert_eq!(fixed(&ran(m), &none, &r), Ok(()));
        // an explanation that leaves the model as it was
        let s = Session::from_model(neutral_landscapes()).unwrap();
        assert_eq!(explained(&s, &said("It makes a Gaussian random field, a smooth surface."), &r), Ok(()));
        assert!(explained(&s, &said("It draws classes."), &r).is_err());
        // a rename that kept b reading it
        let mut m = two_nodes();
        m.nodes[0].id = "habitat".into();
        m.nodes[1].inputs.insert("x".into(), terraflux_graph::Source::Ref("habitat.map".into()));
        assert_eq!(renamed(&Session::from_model(m).unwrap(), &none, &r), Ok(()));
        // the loop: 5 steps of + 2 from 0
        let mut m = Model::new("l");
        m.nodes.push(Node::new("loop", "control.repeat").literal("times", 5).literal("total", 0).body(
            terraflux_graph::Body {
                nodes: vec![Node::new("add", "value.calc").input("x", "@total").param("expression", "x + 2")],
                carry: [("total".to_string(), "add.value".to_string())].into(),
                ..Default::default()
            },
        ));
        let s = ran(m);
        assert_eq!(looped(&s, &said("The number at the end is 10."), &r), Ok(()), "{:?}", s.error);
    }

    #[test]
    fn scores_are_kept_by_model() {
        let o = |failed: Option<&str>| Outcome {
            task: "x".into(),
            failed: failed.map(String::from),
            seconds: 1.0,
            tools: 2,
        };
        let s = Score { date: chrono_date(), outcomes: vec![o(None), o(Some("no loop")), o(None)] };
        assert_eq!(s.passed(), 2);
        assert_eq!(s.date.len(), 10);
        assert!(s.date.starts_with("20"), "{}", s.date);
        let p = crate::llm::presets().into_iter().next().unwrap();
        assert!(key_of(&p).contains(" on http"), "{}", key_of(&p));
    }

    /// The tasks with the HU Berlin's model (on its network: run by hand).
    #[test]
    #[ignore]
    fn hu_berlin_does_the_tasks() {
        let r = Registry::standard();
        let p = crate::llm::presets().into_iter().next().unwrap();
        let stop = AtomicBool::new(false);
        for t in TASKS {
            let o = one(&p, &r, t, &stop);
            println!("{}: {:?} ({:.0} s, {} tools)", o.task, o.failed, o.seconds, o.tools);
        }
    }
}
