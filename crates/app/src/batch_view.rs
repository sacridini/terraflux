//! Running the model many times, from the application: replicates (other seeds: how much of a result is
//! chance), scenarios (named variants of the model, in a table) and calibration (settings tried in ranges, each
//! scored against an observed map). The runs go to workers on this machine (the same code as on many), each in a
//! folder of its own; the results are a table, the runs as one series in the model, and a calibration's best
//! settings applied to the model in a click.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{RichText, Ui};
use egui_plot::{Legend, Plot, Points};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use terraflux_dist::batch::{self, Param, Scored};
use terraflux_dist::client::TaskOf;
use terraflux_dist::evaluate::Evaluation;
use terraflux_dist::proto::JobStatus;
use terraflux_dist::run::{Batch, Event, Handle, Place, Progress, Start};
use terraflux_graph::{Input, Model, Node, Source, Type};
use terraflux_ops::Registry;

use crate::app::{Session, accent_button};
use crate::computers::Computers;
use crate::theme;

/// What a batch does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Kind {
    #[default]
    Replicates,
    Scenarios,
    Calibration,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Replicates => "replicates",
            Kind::Scenarios => "scenarios",
            Kind::Calibration => "calibration",
        }
    }
}

/// What a calibration maximises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Score {
    #[default]
    Similarity,
    FigureOfMerit,
    Overall,
}

/// A setting a batch may change: its key (`$input`, `node.setting`, `container/node.setting`), its value in the
/// model (null when the node takes its default), and what it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Knob {
    pub key: String,
    pub value: Json,
    pub about: String,
}

/// The settings of a model a batch may change: its inputs of values, its nodes' literal inputs and settings
/// (those of numbers, yes or no, and text), in the bodies of containers too.
pub fn knobs(m: &Model, r: &Registry) -> Vec<Knob> {
    let mut out = Vec::new();
    for (name, i) in &m.inputs {
        if let Input::Value { value, .. } = i
            && !value.is_object()
            && !value.is_array()
        {
            out.push(Knob { key: format!("${name}"), value: value.clone(), about: "an input of the model".into() });
        }
    }
    fn walk(nodes: &[Node], prefix: &str, r: &Registry, out: &mut Vec<Knob>) {
        for n in nodes {
            let at = format!("{prefix}{}", n.id);
            let sig = r.sig(&n.op);
            let scalar = |v: &Json| !v.is_object() && !v.is_array();
            for (port, src) in &n.inputs {
                if let Source::Literal(v) = src
                    && scalar(v)
                {
                    out.push(Knob {
                        key: format!("{at}.{port}"),
                        value: v.clone(),
                        about: format!("{} (input)", n.op),
                    });
                }
            }
            let mut seen = BTreeSet::new();
            for (k, v) in &n.params {
                if scalar(v) && !(k == "path" && terraflux_graph::FILE_OPS.contains(&n.op.as_str())) {
                    seen.insert(k.clone());
                    out.push(Knob { key: format!("{at}.{k}"), value: v.clone(), about: n.op.clone() });
                }
            }
            if let Some(sig) = sig {
                for p in &sig.params {
                    if !seen.contains(&p.name) && matches!(p.ty, Type::Number | Type::Integer | Type::Bool) {
                        out.push(Knob { key: format!("{at}.{}", p.name), value: Json::Null, about: p.doc.clone() });
                    }
                }
                for p in &sig.inputs {
                    if !n.inputs.contains_key(&p.name) && matches!(p.ty, Type::Number | Type::Integer | Type::Bool) {
                        out.push(Knob { key: format!("{at}.{}", p.name), value: Json::Null, about: p.doc.clone() });
                    }
                }
            }
            if let Some(b) = &n.body {
                walk(&b.nodes, &format!("{at}/"), r, out);
            }
        }
    }
    walk(&m.nodes, "", r, &mut out);
    out
}

/// Whether a setting is a seed.
fn is_seed(key: &str) -> bool {
    let last = key.rsplit(['.', '$']).next().unwrap_or(key).to_lowercase();
    last == "seed" || last.ends_with("_seed")
}

/// The seeds a batch changes by default: the model's inputs of seeds, else the seeds its nodes are given as
/// inputs (a simulation's), else every seed.
fn default_seeds(knobs: &[Knob]) -> BTreeSet<String> {
    let seeds: Vec<&Knob> = knobs.iter().filter(|k| is_seed(&k.key)).collect();
    let inputs: BTreeSet<String> = seeds.iter().filter(|k| k.key.starts_with('$')).map(|k| k.key.clone()).collect();
    if !inputs.is_empty() {
        return inputs;
    }
    let given: BTreeSet<String> =
        seeds.iter().filter(|k| k.about.ends_with("(input)")).map(|k| k.key.clone()).collect();
    if !given.is_empty() {
        return given;
    }
    seeds.iter().map(|k| k.key.clone()).collect()
}

/// The outputs of the top-level nodes a run may keep (maps, stacks, series, tables...): `node.port`.
fn outputs(m: &Model, r: &Registry) -> Vec<(String, Type)> {
    let mut out = Vec::new();
    for n in &m.nodes {
        if n.op == "view.plot" || crate::editor::output_ports(n, r).is_empty() {
            continue;
        }
        for (port, ty) in crate::editor::output_ports(n, r) {
            if matches!(
                ty,
                Type::Map | Type::Stack | Type::Series | Type::Table | Type::Any | Type::Raster | Type::Vector
            ) {
                out.push((format!("{}.{port}", n.id), ty));
            }
        }
    }
    out
}

/// The nodes no other node reads: where a model ends.
fn sinks(m: &Model) -> Vec<String> {
    let mut read = BTreeSet::new();
    fn refs(nodes: &[Node], read: &mut BTreeSet<String>) {
        for n in nodes {
            for s in n.inputs.values() {
                if let Source::Ref(r) = s
                    && let Some((node, _)) = r.split_once('.')
                {
                    read.insert(node.trim_start_matches('@').to_string());
                }
            }
            if let Some(b) = &n.body {
                refs(&b.nodes, read);
                for v in b.outputs.values().chain(b.carry.values()).chain(b.collect.values()) {
                    if let Some((node, _)) = v.split_once('.') {
                        read.insert(node.to_string());
                    }
                }
            }
        }
    }
    refs(&m.nodes, &mut read);
    for o in m.outputs.values() {
        if let Some((node, _)) = o.from.split_once('.') {
            read.insert(node.to_string());
        }
    }
    m.nodes.iter().filter(|n| !read.contains(&n.id) && n.op != "view.plot").map(|n| n.id.clone()).collect()
}

/// What a run keeps by default: the first map (else any output) of each node where the model ends, unless the
/// model has outputs or nodes that write files of their own.
fn default_keep(m: &Model, r: &Registry) -> BTreeSet<String> {
    let writes = |nodes: &[Node]| nodes.iter().any(|n| terraflux_dist::worker::WRITE_OPS.contains(&n.op.as_str()));
    if !m.outputs.is_empty() || writes(&m.nodes) {
        return BTreeSet::new();
    }
    let outs = outputs(m, r);
    sinks(m)
        .into_iter()
        .filter_map(|id| {
            let mine: Vec<&(String, Type)> = outs.iter().filter(|(k, _)| k.split_once('.').unwrap().0 == id).collect();
            mine.iter()
                .find(|(_, t)| matches!(t, Type::Map | Type::Series | Type::Stack | Type::Raster))
                .or(mine.first())
                .map(|(k, _)| k.clone())
        })
        .collect()
}

/// A setting a calibration varies, as typed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Range {
    pub key: String,
    pub from: String,
    pub to: String,
    pub points: String,
}

/// A batch running.
pub struct Running {
    handle: Handle,
    kind: Kind,
    out: PathBuf,
    started: Instant,
    total: usize,
    stage: String,
    status: Option<JobStatus>,
    calibration: Option<Plan>,
    /// What is kept on disk to take the batch up again, and when it was last kept.
    record: Record,
    kept: Instant,
}

/// A batch running, kept on disk while it runs: when the application stops before its end, it is taken up again
/// from there (the runs done are not run again).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub kind: Kind,
    /// The model's name.
    pub model: String,
    pub out: PathBuf,
    pub files: bool,
    /// Its job on the application's pool (see [`crate::computers`]), or on a cluster's.
    pub job: Option<String>,
    /// The cluster it runs on (none: the application's pool).
    #[serde(default)]
    pub on: Option<String>,
    pub total: usize,
    pub plan: Option<Plan>,
    /// When the application running it last said so (seconds since 1970): a batch of another window.
    pub alive: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Where the batch running is kept (the user's application data).
fn record_file() -> Option<PathBuf> {
    crate::settings::file().and_then(|f| f.parent().map(|d| d.join("batch.json")))
}

impl Record {
    fn keep(&mut self) {
        self.alive = now();
        if let Some(f) = record_file() {
            if let Some(d) = f.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(f, serde_json::to_string_pretty(self).unwrap_or_default());
        }
    }

    /// The batch let go: its record.
    fn drop_all(&self) {
        if let Some(f) = record_file() {
            let _ = std::fs::remove_file(f);
        }
    }

    /// Whether another window of the application runs it now.
    pub fn elsewhere(&self) -> bool {
        now().saturating_sub(self.alive) < 30
    }
}

/// The batch that stopped before its end, if any.
pub fn interrupted() -> Option<Record> {
    let r: Record = serde_json::from_str(&std::fs::read_to_string(record_file()?).ok()?).ok()?;
    r.job.is_some().then_some(r)
}

/// What a calibration needs to rank its runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    candidates: Vec<BTreeMap<String, Json>>,
    keys: Vec<String>,
    seeds: usize,
    labels: Vec<String>,
    metric: String,
}

/// A run of a batch done.
#[derive(Debug, Clone)]
pub struct Row {
    pub label: String,
    pub worker: String,
    pub seconds: f64,
    pub ok: bool,
    pub error: String,
}

/// A batch done.
pub struct Finished {
    pub kind: Kind,
    pub out: PathBuf,
    pub elapsed: Duration,
    pub rows: Vec<Row>,
    /// The files each run wrote.
    pub files: Vec<String>,
    pub failed: u32,
    pub errors: Vec<String>,
    /// A calibration's candidates, best first; the settings varied; the metric.
    pub ranked: Option<(Vec<Scored>, Vec<String>, String)>,
}

/// The batch pane's state (the application's, kept as models change).
pub struct BatchState {
    pub kind: Kind,
    pub runs: usize,
    pub first_seed: i64,
    /// The seeds changed (none: not chosen yet, the model's by default).
    pub seeds: Option<BTreeSet<String>>,
    /// The scenarios: the settings they change, and each one's name and values.
    pub columns: Vec<String>,
    pub scenarios: Vec<(String, Vec<String>)>,
    pub ranges: Vec<Range>,
    pub random: bool,
    pub draws: usize,
    pub seed_list: String,
    /// The output whose last map is scored, and what the simulation starts from (none: guessed).
    pub output: Option<String>,
    pub start: Option<String>,
    pub observed: Option<PathBuf>,
    pub bands: Vec<String>,
    pub band: usize,
    pub score: Score,
    pub window: i32,
    /// The outputs each run keeps (none: not chosen yet).
    pub keep: Option<BTreeSet<String>>,
    pub out: Option<PathBuf>,
    pub workers: usize,
    pub filter: String,
    pub running: Option<Running>,
    pub finished: Option<Finished>,
    pub error: Option<String>,
    /// A batch that stopped before its end (the application closed), to take up again.
    pub interrupted: Option<Record>,
    /// The model the choices were made for (they are let go when another opens).
    model_of: Option<String>,
    /// Where it runs: a cluster's name (none: this computer, and those helping it).
    pub on: Option<String>,
}

impl Default for BatchState {
    fn default() -> Self {
        BatchState {
            kind: Kind::Replicates,
            runs: 10,
            first_seed: 1,
            seeds: None,
            columns: Vec::new(),
            scenarios: Vec::new(),
            ranges: Vec::new(),
            random: false,
            draws: 20,
            seed_list: "1".into(),
            output: None,
            start: None,
            observed: None,
            bands: Vec::new(),
            band: 0,
            score: Score::Similarity,
            window: 5,
            keep: None,
            out: None,
            workers: default_workers(),
            filter: String::new(),
            running: None,
            finished: None,
            error: None,
            interrupted: None,
            model_of: None,
            on: None,
        }
    }
}

/// The workers on this machine by default: half its threads (the application keeps the rest).
pub fn default_workers() -> usize {
    (terraflux_ops::threads::most() / 2).max(1)
}

impl BatchState {
    /// The pane's state, with the batch that stopped before its end, if any.
    pub fn new() -> Self {
        BatchState { interrupted: interrupted(), ..Default::default() }
    }

    pub fn running(&self) -> bool {
        self.running.is_some()
    }

    /// What the batch running sent.
    pub fn poll(&mut self) {
        let Some(r) = &mut self.running else { return };
        let mut done = None;
        while let Ok(e) = r.handle.rx.try_recv() {
            match e {
                Event::Progress(Progress::Workers(n)) => r.stage = format!("{n} workers started"),
                Event::Progress(Progress::Sending) => r.stage = "sending the data to the workers".into(),
                Event::Progress(Progress::Cluster(what)) => r.stage = what,
                Event::Progress(Progress::Submitted { job, total }) => {
                    r.total = total;
                    r.stage = "running".into();
                    r.record.job = Some(job);
                    r.record.total = total;
                    r.record.keep();
                    r.kept = Instant::now();
                }
                Event::Progress(Progress::Status(s)) => r.status = Some(s),
                Event::Done(o) => done = Some(o),
            }
        }
        let Some(o) = done else {
            // still alive: another window does not take it up
            if r.record.job.is_some() && r.kept.elapsed() > Duration::from_secs(10) {
                r.record.keep();
                r.kept = Instant::now();
            }
            return;
        };
        let r = self.running.take().unwrap();
        r.record.drop_all();
        match o {
            Err(e) if e == terraflux_dist::run::STOPPED => self.error = None,
            Err(e) => self.error = Some(e),
            Ok(o) => {
                let rows = o
                    .results
                    .iter()
                    .map(|t| Row {
                        label: t.label.clone(),
                        worker: t.worker.clone(),
                        seconds: t.seconds,
                        ok: t.ok,
                        error: t.error.clone(),
                    })
                    .collect();
                let mut files: Vec<String> = Vec::new();
                for t in &o.results {
                    for f in &t.files {
                        if !files.contains(&f.name) {
                            files.push(f.name.clone());
                        }
                    }
                }
                let ranked = r.calibration.as_ref().and_then(|p| {
                    let metrics = batch::metrics_of(&p.labels, &o.results);
                    match batch::rank(&p.candidates, p.seeds, &metrics, &p.metric) {
                        Ok(ranked) => {
                            let _ = std::fs::create_dir_all(&r.out);
                            let _ = std::fs::write(
                                r.out.join("calibration.csv"),
                                batch::ranking_csv(&ranked, &p.keys, &p.metric),
                            );
                            if let Some(best) = ranked.first() {
                                let _ = std::fs::write(
                                    r.out.join("best.json"),
                                    serde_json::to_string_pretty(&best.settings).unwrap_or_default(),
                                );
                            }
                            Some((ranked, p.keys.clone(), p.metric.clone()))
                        }
                        Err(e) => {
                            self.error = Some(e.to_string());
                            None
                        }
                    }
                });
                self.finished = Some(Finished {
                    kind: r.kind,
                    out: r.out,
                    elapsed: r.started.elapsed(),
                    rows,
                    files,
                    failed: o.status.failed,
                    errors: o.status.errors,
                    ranked,
                });
            }
        }
    }

    /// The choices let go when another model is open.
    fn follow(&mut self, s: &Session) {
        let id = format!("{:?}{}", s.path, s.title());
        if self.model_of.is_none() {
            self.model_of = Some(id);
        } else if self.model_of.as_deref() != Some(id.as_str()) {
            *self = BatchState {
                kind: self.kind,
                runs: self.runs,
                workers: self.workers,
                running: self.running.take(),
                finished: self.finished.take(),
                interrupted: self.interrupted.take(),
                on: self.on.take(),
                model_of: Some(id),
                ..Default::default()
            };
        }
    }
}

/// A folder of results not used yet: `<model's folder>/<model> runs/<kind> <n>`.
fn next_out(s: &Session, kind: Kind) -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    let base = if s.path.is_some() { s.base() } else { home.unwrap_or_default() };
    let dir = base.join(format!("{} runs", s.title()));
    (1..).map(|n| dir.join(format!("{} {n}", kind.name()))).find(|p| !p.exists()).unwrap()
}

fn parse_value(text: &str) -> Json {
    let t = text.trim();
    serde_json::from_str(t).unwrap_or_else(|_| Json::String(t.to_string()))
}

fn shown(v: &Json) -> String {
    match v {
        Json::Null => String::new(),
        Json::String(t) => t.clone(),
        v => v.to_string(),
    }
}

/// The tasks of the batch chosen, with what a calibration needs to rank them.
fn tasks(st: &BatchState, knobs: &[Knob]) -> Result<(Vec<TaskOf>, Option<Plan>), String> {
    let seeds: Vec<String> = st.seeds.clone().unwrap_or_else(|| default_seeds(knobs)).into_iter().collect();
    match st.kind {
        Kind::Replicates => {
            if seeds.is_empty() {
                return Err("the model has no seed to change: choose one below".into());
            }
            Ok((batch::replicates(st.runs, st.first_seed, &seeds), None))
        }
        Kind::Scenarios => {
            if st.scenarios.is_empty() {
                return Err("add a scenario".into());
            }
            let mut names = BTreeSet::new();
            let mut out = Vec::new();
            for (name, values) in &st.scenarios {
                let name = name.trim();
                if name.is_empty() || !names.insert(name.to_string()) {
                    return Err(format!("every scenario needs a name of its own ({name:?})"));
                }
                let set: BTreeMap<String, Json> = st
                    .columns
                    .iter()
                    .zip(values)
                    .filter(|(_, v)| !v.trim().is_empty())
                    .map(|(k, v)| (k.clone(), parse_value(v)))
                    .collect();
                out.push((name.to_string(), set));
            }
            Ok((out, None))
        }
        Kind::Calibration => {
            if st.ranges.is_empty() {
                return Err("add a setting to calibrate".into());
            }
            let mut params = Vec::new();
            for r in &st.ranges {
                let points = r.points.trim().parse::<usize>().unwrap_or(5);
                let mut p = Param::parse(&format!("{}={}:{}:{}", r.key, r.from.trim(), r.to.trim(), points), points)
                    .map_err(|e| format!("{}: {e}", r.key))?;
                // whole numbers or not as the model's value is (0 to 1 of a share: decimals)
                if let Some(v) = knobs.iter().find(|k| k.key == r.key).map(|k| &k.value).filter(|v| v.is_number()) {
                    p.integer = v.is_i64() || v.is_u64();
                }
                params.push(p);
            }
            let candidates = if st.random { batch::random(&params, st.draws.max(1), 1) } else { batch::grid(&params) };
            let list: Vec<i64> = st
                .seed_list
                .split([',', ' '])
                .filter(|t| !t.trim().is_empty())
                .map(|t| t.trim().parse::<i64>().map_err(|_| format!("{t:?} is not a seed")))
                .collect::<Result<_, _>>()?;
            let list = if list.is_empty() { vec![1] } else { list };
            let t = batch::calibration_tasks(&candidates, &list, &seeds);
            let metric = match st.score {
                Score::Similarity => format!("similarity:{}", st.window),
                Score::FigureOfMerit => "fom".into(),
                Score::Overall => "overall".into(),
            };
            let plan = Plan {
                labels: t.iter().map(|(l, _)| l.clone()).collect(),
                keys: params.iter().map(|p| p.key.clone()).collect(),
                candidates,
                seeds: list.len(),
                metric,
            };
            Ok((t, Some(plan)))
        }
    }
}

/// The output scored by default: a simulation's maps, else the first series or map of the nodes where the
/// model ends.
fn default_output(m: &Model, r: &Registry) -> Option<String> {
    let outs = outputs(m, r);
    if outs.iter().any(|(k, _)| k == "simulation.maps") {
        return Some("simulation.maps".into());
    }
    let ends = sinks(m);
    outs.iter()
        .filter(|(k, t)| ends.iter().any(|e| k.starts_with(&format!("{e}."))) && matches!(t, Type::Series | Type::Map))
        .chain(outs.iter().filter(|(_, t)| matches!(t, Type::Series)))
        .map(|(k, _)| k.clone())
        .next()
}

/// What a simulation starts from: the model's maps and the maps its nodes read.
fn starts(m: &Model) -> Vec<String> {
    let mut out: Vec<String> =
        m.inputs.iter().filter(|(_, i)| matches!(i, Input::Map { .. })).map(|(k, _)| k.clone()).collect();
    out.extend(m.nodes.iter().filter(|n| n.op == "raster.read").map(|n| format!("{}.raster", n.id)));
    out
}

fn default_start(m: &Model) -> Option<String> {
    let all = starts(m);
    ["t0", "start", "landuse", "initial"]
        .iter()
        .find(|k| all.iter().any(|a| a == *k))
        .map(|k| k.to_string())
        .or(all.into_iter().next())
}

/// Start the batch chosen on the model of the session, on the application's pool.
fn start(
    st: &mut BatchState,
    s: &Session,
    r: &Registry,
    computers: &mut Computers,
    ctx: &egui::Context,
) -> Result<(), String> {
    if s.issues.iter().any(|i| i.severity == terraflux_graph::Severity::Error) {
        return Err("the model has errors: fix them first (see the nodes in red)".into());
    }
    let knobs = knobs(&s.model, r);
    let (tasks, plan) = tasks(st, &knobs)?;
    if tasks.is_empty() {
        return Err("no run to do".into());
    }
    let mut model = s.model.clone();
    for key in st.keep.clone().unwrap_or_else(|| default_keep(&s.model, r)) {
        let name = key.replace(['.', '/'], "_");
        model.outputs.insert(name, terraflux_graph::Output { from: key, path: None, dtype: None, layer: None });
    }
    // every run must apply to the model
    for (label, o) in &tasks {
        model.clone().apply(o).map_err(|e| format!("{label}: {e}"))?;
    }
    let evaluation = match st.kind {
        Kind::Calibration => {
            let observed = st.observed.clone().ok_or("choose the observed map")?;
            let output =
                st.output.clone().or_else(|| default_output(&s.model, r)).ok_or("choose the simulated maps")?;
            let start = st
                .start
                .clone()
                .or_else(|| default_start(&s.model))
                .ok_or("choose the map the simulation starts from")?;
            Some(Evaluation {
                output,
                observed: observed.to_string_lossy().into_owned(),
                band: st.bands.get(st.band).cloned(),
                start,
                windows: vec![1, 3, 5, 7, 9, 11],
            })
        }
        _ => None,
    };
    let out = st.out.clone().unwrap_or_else(|| next_out(s, st.kind));
    let files = st.kind != Kind::Calibration;
    let total = tasks.len();
    let b =
        Batch { name: format!("{} {}", s.title(), st.kind.name()), model, base: s.base(), tasks, files, evaluation };
    let record =
        Record { kind: st.kind, model: s.title(), out, files, job: None, on: st.on.clone(), total, plan, alive: now() };
    let place = match &st.on {
        Some(name) => cluster_place(name)?,
        None => computers.pool(st.workers.clamp(1, total.max(1)))?.place(),
    };
    launch(st, Start::New(Box::new(b)), record, place, ctx);
    Ok(())
}

/// A cluster saved, as where a batch runs.
fn cluster_place(name: &str) -> Result<Place, String> {
    terraflux_dist::remote::load(&crate::settings::data_dir())
        .into_iter()
        .find(|p| p.name == name)
        .map(|p| Place::Cluster(Box::new(p)))
        .ok_or_else(|| format!("no cluster {name:?} any more (Computers)"))
}

/// Run a batch (or take one up again) on the application's pool or a cluster.
fn launch(st: &mut BatchState, start: Start, record: Record, place: Place, ctx: &egui::Context) {
    let ctx = ctx.clone();
    let out = record.files.then(|| record.out.clone());
    let handle = terraflux_dist::run::spawn(start, place, out, Box::new(move || ctx.request_repaint()));
    st.running = Some(Running {
        handle,
        kind: record.kind,
        out: record.out.clone(),
        started: Instant::now(),
        total: record.total,
        stage: "starting the workers".into(),
        status: None,
        calibration: record.plan.clone(),
        record,
        kept: Instant::now(),
    });
    st.interrupted = None;
    st.finished = None;
    st.error = None;
}

/// The batch that stopped before its end: taken up again, or let go.
fn interrupted_ui(ui: &mut Ui, st: &mut BatchState, computers: &mut Computers) {
    let Some(r) = st.interrupted.clone() else { return };
    if st.running() {
        return;
    }
    egui::Frame::new().fill(ui.visuals().faint_bg_color).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        if r.elsewhere() {
            ui.label(format!("{} of {} runs in another window of Terraflux.", r.kind.name(), r.model));
            return;
        }
        ui.label(
            RichText::new(format!(
                "The {} of {} stopped before its end ({} runs, results in {}).",
                r.kind.name(),
                r.model,
                r.total,
                r.out.display()
            ))
            .strong(),
        );
        ui.horizontal(|ui| {
            if ui.add(accent_button("Go on", true)).on_hover_text("the runs done are not run again").clicked()
                && let Some(job) = r.job.clone()
            {
                let place = match &r.on {
                    Some(name) => cluster_place(name),
                    None => computers.pool(st.workers).map(|p| p.place()),
                };
                match place {
                    Ok(place) => launch(st, Start::Resume(job), r.clone(), place, ui.ctx()),
                    Err(e) => st.error = Some(e),
                }
            }
            if ui.button("Let it go").clicked() {
                if r.on.is_none()
                    && let (Some(job), Ok(p)) = (&r.job, computers.pool(0))
                {
                    p.forget(job);
                }
                r.drop_all();
                st.interrupted = None;
            }
        });
    });
}

/// A setting chosen from the model's, by a part of its name; whether it changed.
fn knob_picker(ui: &mut Ui, id: &str, knobs: &[Knob], current: &mut String, filter: &mut String, hint: &str) -> bool {
    let mut changed = false;
    let text = if current.is_empty() { hint.to_string() } else { current.clone() };
    egui::ComboBox::from_id_salt(id).selected_text(text).width(220.0).height(360.0).show_ui(ui, |ui| {
        ui.add(egui::TextEdit::singleline(filter).hint_text("filter").desired_width(200.0));
        let f = filter.to_lowercase();
        for k in knobs.iter().filter(|k| f.is_empty() || k.key.to_lowercase().contains(&f)) {
            let label = match &k.value {
                Json::Null => k.key.clone(),
                v => format!("{} = {}", k.key, shown(v)),
            };
            if ui.selectable_label(*current == k.key, label).on_hover_text(&k.about).clicked() {
                *current = k.key.clone();
                changed = true;
                ui.close();
            }
        }
    });
    changed
}

/// Open a folder in the system's file manager.
pub fn open_folder(p: &Path) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(p).spawn();
}

fn seeds_ui(ui: &mut Ui, st: &mut BatchState, knobs: &[Knob]) {
    let all: Vec<&Knob> = knobs.iter().filter(|k| is_seed(&k.key)).collect();
    let mut chosen = st.seeds.clone().unwrap_or_else(|| default_seeds(knobs));
    ui.horizontal_wrapped(|ui| {
        ui.label("Seeds changed:");
        if all.is_empty() {
            ui.label(RichText::new("the model has no seed").color(ui.visuals().warn_fg_color));
        }
        for k in all {
            let mut on = chosen.contains(&k.key);
            if ui.checkbox(&mut on, &k.key).on_hover_text(&k.about).changed() {
                if on {
                    chosen.insert(k.key.clone());
                } else {
                    chosen.remove(&k.key);
                }
                st.seeds = Some(chosen.clone());
            }
        }
    });
}

fn replicates_ui(ui: &mut Ui, st: &mut BatchState, knobs: &[Knob]) {
    ui.label(
        RichText::new(
            "The model run again with other seeds: what changes from run to run is chance, what stays is the model's.",
        )
        .weak(),
    );
    egui::Grid::new("replicates").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Runs");
        ui.add(egui::DragValue::new(&mut st.runs).range(1..=100_000));
        ui.end_row();
        ui.label("First seed");
        ui.add(egui::DragValue::new(&mut st.first_seed));
        ui.end_row();
    });
    seeds_ui(ui, st, knobs);
}

fn scenarios_ui(ui: &mut Ui, st: &mut BatchState, knobs: &[Knob]) {
    ui.label(RichText::new("Variants of the model, each run once: a row per scenario, a column per setting it changes (empty: as in the model).").weak());
    let value_of = |key: &str| knobs.iter().find(|k| k.key == key).map(|k| shown(&k.value)).unwrap_or_default();
    let mut drop_col = None;
    let mut drop_row = None;
    egui::ScrollArea::horizontal().id_salt("scenarios table").show(ui, |ui| {
        egui::Grid::new("scenarios").striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label(RichText::new("scenario").strong());
            for (c, key) in st.columns.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(key).strong().monospace());
                    if ui.small_button("x").on_hover_text("stop changing this setting").clicked() {
                        drop_col = Some(c);
                    }
                });
            }
            ui.end_row();
            for (i, (name, values)) in st.scenarios.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(name).desired_width(140.0));
                values.resize(st.columns.len(), String::new());
                for (c, v) in values.iter_mut().enumerate() {
                    let hint = value_of(&st.columns[c]);
                    ui.add(egui::TextEdit::singleline(v).desired_width(110.0).hint_text(hint));
                }
                if ui.small_button("x").on_hover_text("remove this scenario").clicked() {
                    drop_row = Some(i);
                }
                ui.end_row();
            }
        });
    });
    if let Some(c) = drop_col {
        st.columns.remove(c);
        for (_, v) in &mut st.scenarios {
            if c < v.len() {
                v.remove(c);
            }
        }
    }
    if let Some(i) = drop_row {
        st.scenarios.remove(i);
    }
    ui.horizontal(|ui| {
        if ui.button("+ Scenario").clicked() {
            let n = st.scenarios.len() + 1;
            let values = st.columns.iter().map(|c| value_of(c)).collect();
            st.scenarios.push((format!("scenario {n}"), values));
        }
        let mut key = String::new();
        if knob_picker(ui, "scenario column", knobs, &mut key, &mut st.filter, "+ Setting to change")
            && !st.columns.contains(&key)
        {
            let v = value_of(&key);
            st.columns.push(key);
            for (_, values) in &mut st.scenarios {
                values.push(v.clone());
            }
        }
    });
}

fn calibration_ui(ui: &mut Ui, st: &mut BatchState, s: &Session, r: &Registry, knobs: &[Knob]) {
    ui.label(RichText::new("Settings tried in ranges; every try run with each seed and scored against an observed map from the same start. The best settings can then go to the model.").weak());
    let mut drop = None;
    egui::Grid::new("ranges").num_columns(5).spacing([8.0, 4.0]).show(ui, |ui| {
        for h in ["setting", "from", "to", "values", ""] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for (i, rg) in st.ranges.iter_mut().enumerate() {
            ui.label(RichText::new(&rg.key).monospace());
            ui.add(egui::TextEdit::singleline(&mut rg.from).desired_width(70.0));
            ui.add(egui::TextEdit::singleline(&mut rg.to).desired_width(70.0));
            ui.add_enabled(!st.random, egui::TextEdit::singleline(&mut rg.points).desired_width(40.0));
            if ui.small_button("x").clicked() {
                drop = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = drop {
        st.ranges.remove(i);
    }
    let mut key = String::new();
    if knob_picker(ui, "calibration setting", knobs, &mut key, &mut st.filter, "+ Setting to calibrate")
        && !st.ranges.iter().any(|x| x.key == key)
    {
        let now = knobs.iter().find(|k| k.key == key).and_then(|k| k.value.as_f64());
        let (from, to) = match now {
            Some(v) if v != 0.0 => (shown(&Json::from(v / 2.0)), shown(&Json::from(v * 2.0))),
            _ => ("0".into(), "1".into()),
        };
        st.ranges.push(Range { key, from, to, points: "5".into() });
    }
    ui.horizontal(|ui| {
        ui.radio_value(&mut st.random, false, "every combination")
            .on_hover_text("a grid: the values of each range, every combination of them");
        ui.radio_value(&mut st.random, true, "random draws")
            .on_hover_text("a Latin hypercube: each range cut in as many strata as draws, each stratum drawn once");
        if st.random {
            ui.add(egui::DragValue::new(&mut st.draws).range(1..=100_000).suffix(" draws"));
        }
    });
    ui.horizontal(|ui| {
        ui.label("Seeds of each try");
        ui.add(egui::TextEdit::singleline(&mut st.seed_list).desired_width(120.0).hint_text("1, 2, 3"));
    });
    seeds_ui(ui, st, knobs);
    ui.separator();
    egui::Grid::new("scored").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Simulated maps");
        let outs = outputs(&s.model, r);
        let current = st.output.clone().or_else(|| default_output(&s.model, r)).unwrap_or_default();
        egui::ComboBox::from_id_salt("calibration output").selected_text(&current).width(220.0).show_ui(ui, |ui| {
            for (k, t) in outs.iter().filter(|(_, t)| matches!(t, Type::Series | Type::Map | Type::Stack | Type::Any)) {
                if ui
                    .selectable_label(*k == current, k)
                    .on_hover_text(format!("{}: its last map is scored", t.name()))
                    .clicked()
                {
                    st.output = Some(k.clone());
                }
            }
        });
        ui.end_row();
        ui.label("Starting from");
        let current = st.start.clone().or_else(|| default_start(&s.model)).unwrap_or_default();
        egui::ComboBox::from_id_salt("calibration start").selected_text(&current).width(220.0).show_ui(ui, |ui| {
            for k in starts(&s.model) {
                if ui.selectable_label(k == current, &k).clicked() {
                    st.start = Some(k);
                }
            }
        });
        ui.end_row();
        ui.label("Observed map");
        ui.horizontal(|ui| {
            if ui.button("Choose...").clicked()
                && let Some(p) =
                    rfd::FileDialog::new().add_filter("Rasters", &terraflux_ops::files::RASTER_EXTENSIONS).pick_file()
            {
                match terraflux_ops::files::File::open(&p) {
                    Ok(f) => {
                        st.bands = f.meta().band_names();
                        st.band = st.bands.len().saturating_sub(1);
                        st.observed = Some(p);
                    }
                    Err(e) => st.error = Some(e.to_string()),
                }
            }
            ui.label(st.observed.as_ref().map_or("none".into(), |p| p.display().to_string()));
        });
        ui.end_row();
        if st.bands.len() > 1 {
            ui.label("Its band");
            let bands = st.bands.clone();
            egui::ComboBox::from_id_salt("observed band").selected_text(&bands[st.band]).show_ui(ui, |ui| {
                for (i, b) in bands.iter().enumerate() {
                    ui.selectable_value(&mut st.band, i, b);
                }
            });
            ui.end_row();
        }
        ui.label("Score");
        ui.horizontal(|ui| {
            ui.radio_value(&mut st.score, Score::Similarity, "fuzzy similarity")
                .on_hover_text("Dinamica EGO's similarity of the changes in a window: near misses count");
            if st.score == Score::Similarity {
                egui::ComboBox::from_id_salt("window")
                    .selected_text(format!("{0} x {0}", st.window))
                    .width(70.0)
                    .show_ui(ui, |ui| {
                        for w in [1, 3, 5, 7, 9, 11] {
                            ui.selectable_value(&mut st.window, w, format!("{w} x {w}"));
                        }
                    });
            }
            ui.radio_value(&mut st.score, Score::FigureOfMerit, "figure of merit")
                .on_hover_text("hits / (hits + misses + false alarms + wrong hits), cell by cell");
            ui.radio_value(&mut st.score, Score::Overall, "same class")
                .on_hover_text("the share of cells of the same class");
        });
        ui.end_row();
    });
}

fn keep_ui(ui: &mut Ui, st: &mut BatchState, s: &Session, r: &Registry) {
    let outs = outputs(&s.model, r);
    let mut keep = st.keep.clone().unwrap_or_else(|| default_keep(&s.model, r));
    let writes: Vec<String> = s
        .model
        .nodes
        .iter()
        .filter(|n| terraflux_dist::worker::WRITE_OPS.contains(&n.op.as_str()))
        .map(|n| n.id.clone())
        .collect();
    ui.horizontal_wrapped(|ui| {
        ui.label("Each run keeps:");
        for (name, o) in &s.model.outputs {
            ui.label(RichText::new(name).monospace()).on_hover_text(format!("an output of the model ({})", o.from));
        }
        for w in &writes {
            ui.label(RichText::new(w).monospace()).on_hover_text("the file this node writes, in each run's folder");
        }
        for k in &keep {
            ui.label(RichText::new(k).monospace().color(theme::accent()));
        }
        ui.menu_button("+", |ui| {
            for (k, t) in &outs {
                let mut on = keep.contains(k);
                if ui.checkbox(&mut on, format!("{k}  ({})", t.name())).changed() {
                    if on {
                        keep.insert(k.clone());
                    } else {
                        keep.remove(k);
                    }
                    st.keep = Some(keep.clone());
                }
            }
        })
        .response
        .on_hover_text("the outputs of the nodes each run keeps");
    });
    if keep.is_empty() && writes.is_empty() && s.model.outputs.is_empty() {
        ui.label(RichText::new("nothing to keep: choose an output with +").color(ui.visuals().warn_fg_color));
    }
}

fn running_ui(ui: &mut Ui, r: &Running) {
    let (done, failed, running) =
        r.status.as_ref().map_or((0, 0, 0), |s| (s.done as usize, s.failed as usize, s.running as usize));
    let frac = if r.total == 0 { 0.0 } else { (done + failed) as f32 / r.total as f32 };
    let elapsed = r.started.elapsed();
    let left = if done + failed > 0 {
        let per = elapsed.as_secs_f64() / (done + failed) as f64;
        format!(", about {} left", duration(per * (r.total - done - failed) as f64))
    } else {
        String::new()
    };
    ui.add(
        egui::ProgressBar::new(frac)
            .fill(theme::accent())
            .text(RichText::new(format!("{} of {} runs", done + failed, r.total)).color(theme::on_accent())),
    );
    ui.label(
        RichText::new(format!(
            "{}: {running} running, {failed} failed, {} so far{left}",
            r.stage,
            duration(elapsed.as_secs_f64())
        ))
        .weak(),
    );
}

/// A duration as people say it.
fn duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.0} s")
    } else if secs < 3600.0 {
        format!("{:.0} min {:.0} s", (secs / 60.0).floor(), secs % 60.0)
    } else {
        format!("{:.0} h {:.0} min", (secs / 3600.0).floor(), (secs % 3600.0 / 60.0).floor())
    }
}

fn finished_ui(ui: &mut Ui, st: &mut BatchState, s: &mut Session, r: &Registry) {
    let Some(f) = &st.finished else { return };
    theme::section(ui, &format!("{} done", f.kind.name()));
    ui.label(format!(
        "{} runs in {}{}",
        f.rows.len(),
        duration(f.elapsed.as_secs_f64()),
        if f.failed > 0 { format!(", {} failed", f.failed) } else { String::new() }
    ));
    for e in f.errors.iter().take(5) {
        ui.colored_label(ui.visuals().error_fg_color, e);
    }
    let out = f.out.clone();
    ui.horizontal(|ui| {
        ui.label(RichText::new(out.display().to_string()).monospace().weak());
        if ui.button("Open the folder").clicked() {
            open_folder(&out);
        }
    });
    let mut add_series = None;
    let mut apply = None;
    if let Some((ranked, keys, metric)) = &f.ranked {
        if let Some(best) = ranked.first() {
            ui.label(RichText::new(format!("Best: {} = {:.4}", metric, best.score)).strong());
            for (k, v) in &best.settings {
                ui.label(RichText::new(format!("{k} = {v}")).monospace());
            }
            if ui.add(accent_button("Apply the best settings to the model", true)).clicked() {
                apply = Some(best.settings.clone());
            }
        }
        for key in keys {
            let pts: Vec<[f64; 2]> = ranked
                .iter()
                .filter_map(|c| Some([c.settings.get(key)?.as_f64()?, c.score]))
                .filter(|p| p[1].is_finite())
                .collect();
            Plot::new(("calibration", key))
                .height(160.0)
                .legend(Legend::default())
                .x_axis_label(key.as_str())
                .y_axis_label(metric.as_str())
                .show(ui, |p| p.points(Points::new(key.as_str(), pts).color(theme::accent()).radius(3.0)));
        }
        egui::ScrollArea::vertical().id_salt("ranking").max_height(220.0).show(ui, |ui| {
            egui::Grid::new("ranking").striped(true).show(ui, |ui| {
                ui.label(RichText::new("rank").strong());
                for k in keys {
                    ui.label(RichText::new(k).strong());
                }
                ui.label(RichText::new(metric).strong());
                ui.end_row();
                for (i, c) in ranked.iter().enumerate() {
                    ui.label((i + 1).to_string());
                    for k in keys {
                        ui.label(c.settings.get(k).map(shown).unwrap_or_default());
                    }
                    ui.label(format!("{:.4}", c.score));
                    ui.end_row();
                }
            });
        });
    } else {
        for name in f.files.iter().filter(|n| n.ends_with(".tif")) {
            if ui
                .button(format!("Show the runs of {name} in the model"))
                .on_hover_text("a node that reads this file of every run, as one series (a run per step)")
                .clicked()
            {
                add_series = Some(out.join("*").join(name));
            }
        }
        egui::ScrollArea::vertical().id_salt("runs").max_height(260.0).show(ui, |ui| {
            egui::Grid::new("runs").striped(true).show(ui, |ui| {
                for h in ["run", "worker", "seconds", ""] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for row in &f.rows {
                    ui.label(&row.label);
                    ui.label(RichText::new(&row.worker).weak());
                    ui.label(format!("{:.1}", row.seconds));
                    if row.ok {
                        ui.label("");
                    } else {
                        ui.colored_label(ui.visuals().error_fg_color, &row.error);
                    }
                    ui.end_row();
                }
            });
        });
    }
    if let Some(p) = add_series
        && let Err(e) = crate::builder::add(s, r, "raster.read", &[("path", p.to_string_lossy().into_owned().into())])
    {
        st.error = Some(e);
    }
    if let Some(settings) = apply {
        match s.source.apply(&settings) {
            Ok(()) => {
                s.edited();
                s.notice = Some(("the best settings are in the model".into(), Instant::now()));
            }
            Err(e) => st.error = Some(e.to_string()),
        }
    }
}

/// The batch pane.
pub fn pane(ui: &mut Ui, st: &mut BatchState, s: &mut Session, r: &Registry, computers: &mut Computers) {
    st.follow(s);
    st.poll();
    let knobs = knobs(&s.model, r);
    let busy = st.running();
    // what the batch is, and its button and progress, always in view
    egui::Frame::new().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        interrupted_ui(ui, st, computers);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!busy, |ui| {
                for (k, name) in [
                    (Kind::Replicates, "Replicates"),
                    (Kind::Scenarios, "Scenarios"),
                    (Kind::Calibration, "Calibration"),
                ] {
                    ui.selectable_value(&mut st.kind, k, name);
                }
            });
            ui.separator();
            let count = tasks(st, &knobs).map(|(t, _)| t.len());
            if let Some(run) = &st.running {
                if ui.button("Stop").clicked() {
                    run.handle.stop();
                }
            } else {
                let label = match &count {
                    Ok(n) => format!("Run {n} {}", if *n == 1 { "time" } else { "times" }),
                    Err(_) => "Run".into(),
                };
                let ok = count.is_ok() && !s.loading();
                let resp = ui.add_enabled(ok, accent_button(&label, ok));
                if let Err(e) = &count {
                    ui.label(RichText::new(e).color(ui.visuals().warn_fg_color));
                }
                if resp.clicked()
                    && let Err(e) = start(st, s, r, computers, ui.ctx())
                {
                    st.error = Some(e);
                }
            }
        });
        if let Some(run) = &st.running {
            running_ui(ui, run);
            let helpers = computers.helpers().len();
            if helpers > 0 {
                ui.label(
                    RichText::new(format!(
                        "{helpers} other {} helping",
                        if helpers == 1 { "computer is" } else { "computers are" }
                    ))
                    .color(theme::accent()),
                );
            }
        }
        if let Some(e) = &st.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("batch pane").show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.add_enabled_ui(!busy, |ui| {
                match st.kind {
                    Kind::Replicates => replicates_ui(ui, st, &knobs),
                    Kind::Scenarios => scenarios_ui(ui, st, &knobs),
                    Kind::Calibration => calibration_ui(ui, st, s, r, &knobs),
                }
                ui.separator();
                if st.kind != Kind::Calibration {
                    keep_ui(ui, st, s, r);
                }
                ui.horizontal(|ui| {
                    ui.label("Results in");
                    let shown = st.out.clone().unwrap_or_else(|| next_out(s, st.kind));
                    ui.label(RichText::new(shown.display().to_string()).monospace().weak());
                    if ui.small_button("Change...").clicked()
                        && let Some(p) = rfd::FileDialog::new().set_title("The folder of the results").pick_folder()
                    {
                        st.out = Some(p);
                    }
                    if st.out.is_some() && ui.small_button("x").on_hover_text("a new folder for every batch").clicked()
                    {
                        st.out = None;
                    }
                });
                let clusters = terraflux_dist::remote::load(&crate::settings::data_dir());
                if !clusters.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label("Run on");
                        let shown = st.on.clone().unwrap_or_else(|| "this computer".into());
                        egui::ComboBox::from_id_salt("run on").selected_text(shown).show_ui(ui, |ui| {
                            ui.selectable_value(&mut st.on, None, "this computer")
                                .on_hover_text("and the computers helping it (Computers)");
                            for c in &clusters {
                                ui.selectable_value(&mut st.on, Some(c.name.clone()), &c.name).on_hover_text(&c.host);
                            }
                        });
                    });
                }
                if st.on.is_none() {
                    ui.horizontal(|ui| {
                        let most = terraflux_ops::threads::most();
                        ui.label("Runs at once on this computer");
                        ui.add(egui::Slider::new(&mut st.workers, 1..=most.max(1)))
                            .on_hover_text("each run gets an equal share of the computer's threads");
                    });
                }
            });
            ui.add_space(6.0);
            finished_ui(ui, st, s, r);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_and_settings_of_a_model() {
        let r = Registry::standard();
        let mut m = Model::new("m");
        m.inputs.insert("seed".into(), Input::Value { value: 3.into(), note: None });
        m.nodes.push(Node::new("noise", "nlm.random").param("seed", 1).param("rows", 10));
        let k = knobs(&m, &r);
        assert!(k.iter().any(|x| x.key == "$seed" && x.value == 3));
        assert!(k.iter().any(|x| x.key == "noise.rows" && x.value == 10));
        assert!(k.iter().any(|x| x.key == "noise.columns" && x.value.is_null()), "the settings left at their defaults");
        assert_eq!(default_seeds(&k), ["$seed".to_string()].into(), "the model's seed first");
        m.inputs.clear();
        assert_eq!(default_seeds(&knobs(&m, &r)), ["noise.seed".to_string()].into());
        assert_eq!(sinks(&m), ["noise"]);
        assert_eq!(default_keep(&m, &r), ["noise.map".to_string()].into());
    }

    #[test]
    fn a_batch_is_kept_and_found_again() {
        let dir = std::env::temp_dir().join(format!("terraflux-record-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        unsafe { std::env::set_var("TERRAFLUX_SETTINGS", dir.join("settings.json")) };
        assert!(interrupted().is_none());
        let mut r = Record {
            kind: Kind::Calibration,
            model: "m".into(),
            out: dir.join("out"),
            files: false,
            job: None,
            on: None,
            total: 4,
            plan: Some(Plan {
                candidates: vec![[("a.b".to_string(), Json::from(1.5))].into()],
                keys: vec!["a.b".into()],
                seeds: 4,
                labels: (0..4).map(|k| format!("candidate_0_seed_{k}")).collect(),
                metric: "fom".into(),
            }),
            alive: 0,
        };
        r.keep();
        assert!(interrupted().is_none(), "not submitted yet: nothing to take up");
        r.job = Some("job-1".into());
        r.keep();
        let back = interrupted().unwrap();
        assert_eq!(back.plan.as_ref().unwrap().candidates[0]["a.b"], 1.5);
        assert!(back.elsewhere(), "kept a moment ago: a window runs it");
        r.drop_all();
        assert!(interrupted().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
