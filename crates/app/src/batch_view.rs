//! Running the model many times, from the application: replicates (other seeds: how much of a result is
//! chance), scenarios (named variants of the model, in a table), calibration (settings tried in ranges, each
//! scored against an observed map) and sensitivity analysis (one at a time, Morris, Sobol: which settings matter
//! to an output). The runs go to workers on this machine (the same code as on many), each in a
//! folder of its own; the results are a table, the runs as one series in the model, and a calibration's best
//! settings applied to the model in a click.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
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
use terraflux_dist::sens_maps::{self, Made};
use terraflux_dist::sensitivity::{self as sens, Design, Indices, Method};
use terraflux_exec::journal::{self, Entry, Journal, Status};
use terraflux_graph::{Input, Model, Node, Source, Type};
use terraflux_ops::{Registry, Value};

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
    Sensitivity,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Replicates => "replicates",
            Kind::Scenarios => "scenarios",
            Kind::Calibration => "calibration",
            Kind::Sensitivity => "sensitivity",
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
    /// Whether it is a number (a factor of a sensitivity analysis may be), and a whole one.
    pub numeric: bool,
    pub integer: bool,
    /// What the operator takes when the model gives none, if its description says (`default 4`).
    pub default: Option<f64>,
}

impl Knob {
    /// A setting the model gives.
    fn given(key: String, value: Json, about: String) -> Knob {
        let (numeric, integer) = (value.is_number(), value.is_i64() || value.is_u64());
        Knob { key, value, about, numeric, integer, default: None }
    }

    /// A setting the model leaves to the operator's default.
    fn declared(key: String, ty: Type, doc: &str) -> Knob {
        Knob {
            key,
            value: Json::Null,
            about: doc.to_string(),
            numeric: matches!(ty, Type::Number | Type::Integer),
            integer: ty == Type::Integer,
            default: doc_default(doc),
        }
    }
}

/// The number after `default ` in a setting's description.
fn doc_default(doc: &str) -> Option<f64> {
    let rest = doc.split("default ").nth(1)?;
    let n: String = rest.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-')).collect();
    n.trim_end_matches('.').parse().ok()
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
            out.push(Knob::given(format!("${name}"), value.clone(), "an input of the model".into()));
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
                    out.push(Knob::given(format!("{at}.{port}"), v.clone(), format!("{} (input)", n.op)));
                }
            }
            let mut seen = BTreeSet::new();
            for (k, v) in &n.params {
                if scalar(v) && !(k == "path" && terraflux_graph::FILE_OPS.contains(&n.op.as_str())) {
                    seen.insert(k.clone());
                    out.push(Knob::given(format!("{at}.{k}"), v.clone(), n.op.clone()));
                }
            }
            if let Some(sig) = sig {
                for p in &sig.params {
                    if !seen.contains(&p.name) && matches!(p.ty, Type::Number | Type::Integer | Type::Bool) {
                        out.push(Knob::declared(format!("{at}.{}", p.name), p.ty, &p.doc));
                    }
                }
                for p in &sig.inputs {
                    if !n.inputs.contains_key(&p.name) && matches!(p.ty, Type::Number | Type::Integer | Type::Bool) {
                        out.push(Knob::declared(format!("{at}.{}", p.name), p.ty, &p.doc));
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
    // (what a Plot shows is read: by the Viewer, not where the model ends)
    let plotted = m.plots.iter().flat_map(|p| {
        let layers = p.layers.iter().flat_map(|l| std::iter::once(&l.from).chain(&l.minus));
        let views = p.compare.iter().flat_map(|c| {
            let own = c.layers.iter().flatten().flat_map(|l| std::iter::once(&l.from).chain(&l.minus));
            std::iter::once(&c.with).chain(&c.also).chain(own)
        });
        layers.chain(views)
    });
    for from in m.outputs.values().map(|o| &o.from).chain(plotted) {
        if let Some((node, _)) = from.split_once('.') {
            read.insert(node.to_string());
        }
    }
    m.nodes.iter().filter(|n| !read.contains(&n.id)).map(|n| n.id.clone()).collect()
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
    /// What a sensitivity analysis needs to give its indices at the end.
    #[serde(default)]
    pub sensitivity: Option<SensitivityPlan>,
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

/// What a sensitivity analysis needs to give its indices: the design, whose runs gave the output measured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivityPlan {
    method: Method,
    design: Design,
    keys: Vec<String>,
    rows: Vec<Vec<f64>>,
    labels: Vec<String>,
    output: String,
    resamples: usize,
    /// The runs of each point (the seeds from the model seed up).
    #[serde(default = "one")]
    replicates: usize,
    /// The range of each factor, and the statistic of the output, for the methods text.
    #[serde(default)]
    bounds: Vec<(f64, f64)>,
    #[serde(default)]
    statistic: Option<String>,
    /// The maps of sensitivity to make at the end, cell by cell (none: only the numbers).
    #[serde(default)]
    maps: Option<MapsPlan>,
}

/// The maps of sensitivity asked for: the file each run kept and whether the runs' maps stay after the maps are made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapsPlan {
    file: String,
    keep_runs: bool,
}

/// The maps of sensitivity being made in a thread of their own, from the maps the runs kept.
pub struct Making {
    rx: std::sync::mpsc::Receiver<Result<Made, String>>,
    /// Thousandths of the rows done.
    done: Arc<AtomicU32>,
    plan: SensitivityPlan,
    out: PathBuf,
}

impl Making {
    /// The share done (0 to 1).
    pub fn share(&self) -> f32 {
        self.done.load(Ordering::Relaxed) as f32 / 1000.0
    }
}

/// Make the maps of sensitivity of a finished analysis (its runs' maps in `out/<label>/`) in a thread: the share
/// done goes to the `Making`, the maps (or why not) come through its channel.
fn make_maps(plan: &SensitivityPlan, out: &Path) -> Option<Making> {
    let m = plan.maps.clone()?;
    let (tx, rx) = std::sync::mpsc::channel();
    let done = Arc::new(AtomicU32::new(0));
    let (p, dir, share) = (plan.clone(), out.to_path_buf(), done.clone());
    std::thread::Builder::new()
        .name("terraflux-sensitivity-maps".into())
        .spawn(move || {
            let spec = sens_maps::Spec {
                method: p.method,
                keys: &p.keys,
                rows: &p.rows,
                design: &p.design,
                replicates: p.replicates,
                labels: &p.labels,
                runs: &dir,
                file: &m.file,
                out: &dir,
                keep_runs: m.keep_runs,
            };
            let made = sens_maps::make(&spec, &mut |f| share.store((f * 1000.0) as u32, Ordering::Relaxed));
            let _ = tx.send(made.map_err(|e| e.to_string()));
        })
        .ok()?;
    Some(Making { rx, done, plan: plan.clone(), out: out.to_path_buf() })
}

fn one() -> usize {
    1
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
    /// A sensitivity analysis's indices.
    pub sensitivity: Option<Indices>,
    /// The text of its `methods.md`.
    pub methods: Option<String>,
    /// Its maps of sensitivity, once made.
    pub maps: Option<Made>,
}

/// What a sensitivity analysis varies and measures.
pub struct SensitivityState {
    pub factors: Vec<Range>,
    pub method: Method,
    pub design: Design,
    pub resamples: usize,
    /// The runs of each point of the design, with seeds from the model seed up (a model with a seed).
    pub replicates: usize,
    /// The output measured (`node.port`; none: not chosen yet) and the statistic of a map or the column of a table.
    pub output: Option<String>,
    /// For a map, its kind: mean, sum, min, max, cells, share, area, changed or metric.
    pub statistic: String,
    /// The class (a number) of the kinds that take one, and the metric of `metric`.
    pub class: String,
    pub metric: String,
    /// What a single map's `changed` is against (none: guessed, as the calibration's).
    pub start: Option<String>,
    /// Every suggested factor shown, not only the first few.
    pub more: bool,
    /// Maps of sensitivity, cell by cell, of a map, series or stack measured: each run keeps its last map, and the
    /// maps are made when the analysis ends.
    pub maps: bool,
    /// The runs' maps left in place once the maps of sensitivity are made.
    pub keep_run_maps: bool,
}

impl Default for SensitivityState {
    fn default() -> Self {
        SensitivityState {
            factors: Vec::new(),
            method: Method::Morris,
            design: Design::new(Method::Morris),
            resamples: 100,
            replicates: 1,
            output: None,
            statistic: "mean".into(),
            class: String::new(),
            metric: "np".into(),
            start: None,
            more: false,
            maps: false,
            keep_run_maps: false,
        }
    }
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
    pub sens: SensitivityState,
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
    /// The maps of sensitivity of the analysis that ended, being made.
    pub making: Option<Making>,
    pub error: Option<String>,
    /// A batch that stopped before its end (the application closed), to take up again.
    pub interrupted: Option<Record>,
    /// The model the choices were made for (they are let go when another opens).
    model_of: Option<String>,
    /// Where it runs: a cluster's name (none: this computer, and those helping it).
    pub on: Option<String>,
    /// The outputs a comparison of the Parameters pane waits for from the batch running (`node.port`), and what
    /// each run gave of each when it ends (by output, then by scenario), for that pane.
    pub compare: Option<Vec<String>>,
    pub compared: Option<Vec<(String, crate::params_view::Compared)>>,
    /// The model's file and the model as written when the batch began, for the journal.
    journal_for: Option<(PathBuf, Model)>,
    /// A batch held back because its files may not fit the disk: the window asks.
    pub disk_ask: Option<crate::disk::Ask>,
    /// How many batches have been written to a journal (the Runs pane reads again when it changes).
    pub journaled: u64,
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
            sens: SensitivityState::default(),
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
            making: None,
            error: None,
            interrupted: None,
            model_of: None,
            on: None,
            compare: None,
            compared: None,
            journal_for: None,
            disk_ask: None,
            journaled: 0,
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

    /// The maps of sensitivity, once their thread has made them (or failed): put in the analysis's results, and
    /// `methods.md` written again with the sentence about them.
    fn poll_maps(&mut self) {
        let Some(m) = &self.making else { return };
        let made = match m.rx.try_recv() {
            Ok(made) => made,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("the thread stopped before the end".to_string()),
        };
        let m = self.making.take().unwrap();
        match made {
            Ok(made) => {
                if let Some(f) = &mut self.finished {
                    let p = &m.plan;
                    if let Some(i) = f.sensitivity.as_ref().filter(|i| p.bounds.len() == i.factors.len()) {
                        let run = crate::sens_methods::Run {
                            indices: i,
                            design: &p.design,
                            bounds: &p.bounds,
                            output: &p.output,
                            statistic: p.statistic.as_deref(),
                            resamples: p.resamples,
                            maps: true,
                            version: env!("CARGO_PKG_VERSION"),
                        };
                        f.methods = Some(crate::sens_methods::text(&run));
                        if let Err(e) = crate::sens_methods::write(&m.out, &run) {
                            self.error = Some(format!("methods.md was not written: {e}"));
                        }
                    }
                    f.maps = Some(made);
                }
            }
            Err(e) => {
                self.error = Some(format!("the maps of sensitivity were not made: {e} (the runs' maps are kept)"));
            }
        }
    }

    /// What the batch running sent.
    pub fn poll(&mut self) {
        self.poll_maps();
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
        let began = self.journal_for.take();
        if let Some(outputs) = self.compare.take() {
            let mut all = Vec::new();
            for output in outputs {
                let runs = match &o {
                    Err(e) => vec![("the runs".into(), Err(e.clone()))],
                    Ok(o) => {
                        let mut runs: Vec<(String, Result<Value, String>)> =
                            o.results.iter().map(|t| (t.label.clone(), compared_of(&r.out, t, &output))).collect();
                        // (in the scenarios' order, not the order they ended in)
                        runs.sort_by_key(|(l, _)| self.scenarios.iter().position(|(n, _)| n == l));
                        runs
                    }
                };
                all.push((output, runs));
            }
            self.compared = Some(all);
        }
        match o {
            Err(e) if e == terraflux_dist::run::STOPPED => {
                self.error = None;
                self.journal(began, &r, Status::Stopped, 0, 0, None);
            }
            Err(e) => {
                self.error = Some(e.clone());
                self.journal(began, &r, Status::Failed(e), 0, 0, None);
            }
            Ok(o) => {
                let rows: Vec<Row> = o
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
                let mut methods = None;
                let mut making = None;
                let sensitivity = r.record.sensitivity.as_ref().and_then(|p| {
                    let metrics = batch::metrics_of(&p.labels, &o.results);
                    let values: Vec<Option<f64>> = metrics.iter().map(|m| m.as_ref().map(|m| m.value)).collect();
                    let rep = sens::replicated(&values, p.replicates);
                    let y = rep.mean.clone();
                    let _ = sens::write_design(&r.out, &p.keys, &p.rows, &y, &p.output, Some(&rep));
                    if let Some(k) = y.iter().position(|v| v.is_nan()) {
                        self.error = Some(format!(
                            "run {} gave no {} ({} runs failed{}); the design is in {}",
                            k + 1,
                            p.output,
                            o.status.failed,
                            o.status.errors.first().map(|e| format!(": {e}")).unwrap_or_default(),
                            r.out.join("design.csv").display()
                        ));
                        return None;
                    }
                    match sens::analyze(p.method, &p.keys, &p.rows, &y, &p.design, p.resamples)
                        .map(|i| i.with_replicates(&rep))
                        .and_then(|i| sens::write_indices(&r.out, &i).map(|_| i))
                    {
                        Ok(i) => {
                            if let Err(e) = crate::sens_chart::write(&r.out, &i) {
                                self.error = Some(format!("the chart was not written: {e}"));
                            }
                            let run = crate::sens_methods::Run {
                                indices: &i,
                                design: &p.design,
                                bounds: &p.bounds,
                                output: &p.output,
                                statistic: p.statistic.as_deref(),
                                resamples: p.resamples,
                                maps: false,
                                version: env!("CARGO_PKG_VERSION"),
                            };
                            // (the maps of sensitivity come after, from the maps the runs kept)
                            making = make_maps(p, &r.out);
                            // (an analysis resumed from an older record has no ranges: no methods text)
                            if p.bounds.len() == i.factors.len() {
                                methods = Some(crate::sens_methods::text(&run));
                                if let Err(e) = crate::sens_methods::write(&r.out, &run) {
                                    self.error = Some(format!("methods.md was not written: {e}"));
                                }
                            }
                            Some(i)
                        }
                        Err(e) => {
                            self.error = Some(e.to_string());
                            None
                        }
                    }
                });
                let status = match (rows.len(), o.status.failed) {
                    (n, f) if n > 0 && f as usize >= n => Status::Failed(
                        o.status.errors.first().cloned().unwrap_or_else(|| format!("all {n} runs failed")),
                    ),
                    _ => Status::Done,
                };
                self.journal(began, &r, status, rows.len(), o.status.failed as usize, sensitivity.as_ref());
                self.finished = Some(Finished {
                    kind: r.kind,
                    out: r.out,
                    elapsed: r.started.elapsed(),
                    rows,
                    files,
                    failed: o.status.failed,
                    errors: o.status.errors,
                    ranked,
                    sensitivity,
                    methods,
                    maps: None,
                });
                self.making = making;
            }
        }
    }

    /// A batch that ended, written to its model's journal as one entry (its runs counted, a sensitivity analysis
    /// with its factors ranked); nothing for a batch taken up again, or of a model without a file.
    fn journal(
        &mut self,
        began: Option<(PathBuf, Model)>,
        r: &Running,
        status: Status,
        runs: usize,
        failed: usize,
        indices: Option<&Indices>,
    ) {
        let Some((file, model)) = began else { return };
        let Some(journal) = Journal::of(&file) else { return };
        let mut batch = journal::Batch {
            kind: r.kind.name().to_string(),
            out: r.out.clone(),
            runs: if runs > 0 { runs } else { r.total },
            failed,
            ..Default::default()
        };
        if let Some(i) = indices {
            ranked_factors(&mut batch, i);
        }
        let mut entry = Entry::new(journal::Kind::Batch(batch), "app", r.started.elapsed().as_secs_f64());
        entry.status = status;
        entry.values = journal::values_of(&model);
        if journal.record(entry, &model).is_ok() {
            self.journaled += 1;
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
                making: self.making.take(),
                interrupted: self.interrupted.take(),
                on: self.on.take(),
                compare: self.compare.take(),
                journal_for: self.journal_for.take(),
                journaled: self.journaled,
                model_of: Some(id),
                ..Default::default()
            };
        }
    }
}

/// A sensitivity analysis's method and factors, the most important first with the index they are ranked by, in its
/// batch's journal entry.
pub fn ranked_factors(batch: &mut journal::Batch, i: &Indices) {
    let by = i.ranked_by();
    let col = i.columns.iter().position(|c| c == by);
    batch.method = Some(i.method.name().to_string());
    batch.factors = i
        .order()
        .into_iter()
        .map(|k| journal::Factor {
            key: i.factors[k].clone(),
            by: by.to_string(),
            index: col.and_then(|c| i.rows[k].get(c).copied()).filter(|v| v.is_finite()),
        })
        .collect();
}

/// What a run of a batch gave of an output (`node.port`, kept as `node_port`), read back from its file.
fn compared_of(out: &Path, t: &terraflux_dist::proto::TaskResult, output: &str) -> Result<Value, String> {
    if !t.ok {
        return Err(t.error.clone());
    }
    let name = output.replace(['.', '/'], "_");
    let file = t
        .files
        .iter()
        .find(|f| Path::new(&f.name).file_stem().is_some_and(|s| s.to_string_lossy() == name))
        .ok_or_else(|| format!("the run wrote no {name}"))?;
    terraflux_exec::frozen::read_written(&out.join(&t.label).join(&file.name)).map_err(|e| e.to_string())
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

/// The factors of a sensitivity analysis as parameters: their ranges typed, whole numbers where the setting is one.
fn factors(st: &BatchState, knobs: &[Knob]) -> Result<Vec<Param>, String> {
    if st.sens.factors.is_empty() {
        return Err("add a factor to vary".into());
    }
    let mut params = Vec::new();
    for r in &st.sens.factors {
        let (from, to) = (r.from.trim(), r.to.trim());
        if from.is_empty() || to.is_empty() {
            return Err(format!("{}: type its range (from and to)", r.key));
        }
        let mut p = Param::parse(&format!("{}={from}:{to}", r.key), 2).map_err(|e| format!("{}: {e}", r.key))?;
        if let Some(k) = knobs.iter().find(|k| k.key == r.key) {
            p.integer = k.integer;
        }
        if !p.from.is_finite() || !p.to.is_finite() || p.from >= p.to {
            return Err(format!("{}: its range must go from a value to a larger one", r.key));
        }
        params.push(p);
    }
    Ok(params)
}

/// The replicates at each point: the chosen ones where the model has a seed to change, else 1 (every run of a
/// point would be the same).
fn replicates_of(st: &BatchState, knobs: &[Knob]) -> usize {
    let has_seed = !st.seeds.clone().unwrap_or_else(|| default_seeds(knobs)).is_empty();
    if has_seed { st.sens.replicates.clamp(1, 50) } else { 1 }
}

/// The runs of a sensitivity analysis (the design's points times their replicates), without making them.
fn sensitivity_runs(st: &BatchState, knobs: &[Knob]) -> Result<usize, String> {
    let params = factors(st, knobs)?;
    if st.sens.method == Method::Sobol && !st.sens.design.n.is_power_of_two() {
        return Err("Sobol's base points must be a power of 2".into());
    }
    if st.sens.output.is_none() {
        return Err("choose the output to measure".into());
    }
    Ok(sens::total_runs(st.sens.method, params.len(), &st.sens.design, replicates_of(st, knobs)))
}

/// The time of a batch: the runs in rounds of `at_once`, each round as long as one run.
fn batch_time(runs: usize, one_run: f64, at_once: usize) -> f64 {
    runs.div_ceil(at_once.max(1)) as f64 * one_run
}

/// The time of one run from the nodes' times of the run before: their sum (it overstates a run whose nodes ran
/// side by side). None when no node has a time.
fn one_run_time(times: &BTreeMap<String, f64>) -> Option<f64> {
    let t: f64 = times.values().filter(|t| t.is_finite()).sum();
    (!times.is_empty() && t > 0.0).then_some(t)
}

/// A batch longer than this (seconds) gets a warning.
const LONG_BATCH: f64 = 7200.0;

/// The line under the run button: the runs, how long they take, and whether that is long.
/// `at_once` is the runs in parallel where they go (none: not known), `place` says where ("here" or "on name").
fn cost_text(head: &str, runs: usize, one: Option<f64>, at_once: Option<usize>, place: &str) -> (String, bool) {
    let n = if runs == 1 { "1 run".to_string() } else { format!("{runs} runs") };
    let Some(one) = one else {
        return (format!("{head}{n}; how long each takes is known once the model has run"), false);
    };
    let each = format!("a run took about {}", crate::eta::took(one));
    match at_once {
        Some(w) => {
            let total = batch_time(runs, one, w);
            (format!("{head}{n}, about {} {place} ({w} at once; {each})", crate::eta::took(total)), total > LONG_BATCH)
        }
        None => (format!("{head}{n} {place}; {each}, divided by the workers there"), one * runs as f64 > LONG_BATCH),
    }
}

/// Whether a kind of batch keeps the files of its runs (a calibration and a sensitivity analysis keep numbers only,
/// unless the analysis makes maps of sensitivity: see [`maps_of`]).
fn keeps_files(kind: Kind) -> bool {
    !matches!(kind, Kind::Calibration | Kind::Sensitivity)
}

/// Whether an output of a model is a map, series or stack: what has a last map to make maps of sensitivity from.
pub fn is_map_output(m: &Model, r: &Registry, output: &str) -> bool {
    measurable(m, r).iter().any(|(k, t)| k == output && matches!(t, Type::Map | Type::Series | Type::Stack))
}

/// The output whose last map each run keeps for the maps of sensitivity, when the analysis asks for them and the
/// output measured is a map, series or stack.
fn maps_of(st: &BatchState, s: &Session, r: &Registry) -> Option<String> {
    let out = st.sens.output.clone().or_else(|| default_measure(&s.model, r))?;
    (st.kind == Kind::Sensitivity && st.sens.maps && is_map_output(&s.model, r, &out)).then_some(out)
}

/// The bytes the files of a batch of `runs` runs may take: the runs' outputs, or the one map each run keeps for the
/// maps of sensitivity.
fn batch_bytes(st: &BatchState, s: &Session, r: &Registry, runs: usize) -> u64 {
    if keeps_files(st.kind) {
        s.run_bytes() * runs as u64
    } else if maps_of(st, s, r).is_some() {
        crate::disk::map_bytes(&s.model, &s.base()) * runs as u64
    } else {
        0
    }
}

/// The folder a batch writes to (its results' folder, else beside the model).
fn out_dir(st: &BatchState, s: &Session) -> PathBuf {
    st.out.clone().unwrap_or_else(|| s.base())
}

/// The cost line for the batch chosen, with its warning hint when long.
fn cost_ui(ui: &mut Ui, st: &BatchState, s: &Session, r: &Registry, knobs: &[Knob], runs: usize) {
    let mut times = s.times_before.clone();
    times.extend(s.timings.iter().map(|(at, (secs, _))| (at.clone(), *secs)));
    let one = one_run_time(&times);
    let (place, at_once) = match &st.on {
        None => ("here".to_string(), Some(st.workers.max(1))),
        Some(name) => {
            let w = terraflux_dist::remote::load(&crate::settings::data_dir())
                .into_iter()
                .find(|c| &c.name == name)
                .map(|c| match c.scheduler {
                    terraflux_dist::remote::Scheduler::Host { workers } => workers,
                    terraflux_dist::remote::Scheduler::Slurm { nodes, workers, .. } => nodes * workers,
                });
            (format!("on {name}"), w.map(|w| w.max(1)))
        }
    };
    let head = if st.kind == Kind::Sensitivity {
        let k = st.sens.factors.len();
        let f = if k == 1 { "factor" } else { "factors" };
        let r = replicates_of(st, knobs);
        let reps = if r > 1 { format!(", {r} replicates") } else { String::new() };
        match st.sens.method {
            Method::Oat => format!("One at a time of {k} {f}{reps}: "),
            Method::Morris => format!("Morris of {k} {f}, r={}{reps}: ", st.sens.design.n),
            Method::Sobol => format!("Sobol of {k} {f}, N={}{reps}: ", st.sens.design.n),
        }
    } else {
        String::new()
    };
    let (mut text, long) = cost_text(&head, runs, one, at_once, &place);
    // the files it keeps, against the disk's room
    let tight = crate::disk::cost_note(batch_bytes(st, s, r, runs), &out_dir(st, s), s.disk_space)
        .map(|(note, over)| {
            text.push_str(&note);
            over
        })
        .unwrap_or(false);
    if long || tight {
        let warn = ui.visuals().warn_fg_color;
        ui.label(RichText::new(text).color(warn));
        if long {
            let hint = if st.kind == Kind::Sensitivity {
                "That is long: fewer base points, or Morris first to keep only the factors that matter."
            } else {
                "That is long: fewer runs, or more computers (Computers)."
            };
            ui.label(RichText::new(hint).color(warn));
        }
        if tight {
            ui.label(RichText::new("That may not fit the disk: Run asks first.").color(warn));
        }
    } else {
        ui.label(RichText::new(text).weak());
    }
}

/// The tasks of the batch chosen, with what a calibration needs to rank them or a sensitivity analysis to analyse.
type Planned = (Vec<TaskOf>, Option<Plan>, Option<SensitivityPlan>);

fn tasks(st: &BatchState, knobs: &[Knob]) -> Result<Planned, String> {
    let seeds: Vec<String> = st.seeds.clone().unwrap_or_else(|| default_seeds(knobs)).into_iter().collect();
    match st.kind {
        Kind::Replicates => {
            if seeds.is_empty() {
                return Err("the model has no seed to change: choose one below".into());
            }
            Ok((batch::replicates(st.runs, st.first_seed, &seeds), None, None))
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
            Ok((out, None, None))
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
            Ok((t, Some(plan), None))
        }
        Kind::Sensitivity => {
            sensitivity_runs(st, knobs)?;
            let params = factors(st, knobs)?;
            let bounds: Vec<(f64, f64)> = params.iter().map(|p| (p.from, p.to)).collect();
            let rows = sens::rows(st.sens.method, &bounds, &st.sens.design).map_err(|e| e.to_string())?;
            let candidates = sens::candidates(&params, &rows);
            let r = replicates_of(st, knobs);
            let t = batch::calibration_tasks(&candidates, &sens::seeds(st.first_seed, r), &seeds);
            let plan = SensitivityPlan {
                replicates: r,
                method: st.sens.method,
                design: st.sens.design.clone(),
                keys: params.iter().map(|p| p.key.clone()).collect(),
                rows,
                labels: t.iter().map(|(l, _)| l.clone()).collect(),
                output: st.sens.output.clone().unwrap_or_default(),
                resamples: st.sens.resamples,
                bounds,
                statistic: None,
                maps: None,
            };
            Ok((t, None, Some(plan)))
        }
    }
}

/// The outputs a sensitivity analysis may measure: those of numbers, tables, maps, series and stacks.
fn measurable(m: &Model, r: &Registry) -> Vec<(String, Type)> {
    let mut out = Vec::new();
    for n in &m.nodes {
        for (port, ty) in crate::editor::output_ports(n, r) {
            if matches!(
                ty,
                Type::Number | Type::Integer | Type::Table | Type::Map | Type::Series | Type::Stack | Type::Any
            ) {
                out.push((format!("{}.{port}", n.id), ty));
            }
        }
    }
    out
}

/// The output measured by default: a number or a table where the model ends, else a map there, else the first.
pub fn default_measure(m: &Model, r: &Registry) -> Option<String> {
    let outs = measurable(m, r);
    let ends = sinks(m);
    let at_end = |k: &str| ends.iter().any(|e| k.starts_with(&format!("{e}.")));
    [Type::Number, Type::Integer, Type::Table, Type::Map, Type::Series, Type::Stack]
        .iter()
        .find_map(|t| outs.iter().find(|(k, ty)| ty == t && at_end(k)))
        .or_else(|| outs.first())
        .map(|(k, _)| k.clone())
}

/// What `Evaluation::statistic` takes for an output of a type: nothing for a number, a table's column (none: its
/// last), a map's statistic (see `terraflux_dist::evaluate::measure`) from its kind, class and metric.
fn statistic_of(ty: Option<Type>, sens: &SensitivityState) -> Result<Option<String>, String> {
    let t = sens.statistic.trim();
    Ok(match ty {
        Some(Type::Number | Type::Integer) => None,
        Some(Type::Table) => (!t.is_empty()).then(|| t.to_string()),
        _ => {
            let class = sens.class.trim();
            if matches!(t, "cells" | "share" | "area" | "metric") && class.parse::<f64>().is_err() {
                return Err(format!("{t}: the class must be a number (the class value of the map's cells)"));
            }
            Some(match t {
                "" => "mean".into(),
                "cells" | "share" | "area" => format!("{t}:{class}"),
                "metric" => format!("metric:{}:{class}", sens.metric),
                t => t.to_string(),
            })
        }
    })
}

/// The range of a factor when it is added: 20 % either side of its value (a whole number's, widened to whole
/// numbers); empty when there is no value to start from.
fn default_range(k: &Knob) -> (String, String) {
    let tidy = |x: f64| format!("{x:.6e}").parse::<f64>().unwrap_or(x);
    let Some(v) = k.value.as_f64().or(k.default).filter(|v| *v != 0.0 && v.is_finite()) else {
        return (String::new(), String::new());
    };
    let (a, b) = (v * 0.8, v * 1.2);
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    if k.integer {
        let (lo, mut hi) = (lo.floor(), hi.ceil());
        if hi <= lo {
            hi = lo + 1.0;
        }
        (shown(&Json::from(lo as i64)), shown(&Json::from(hi as i64)))
    } else {
        (shown(&Json::from(tidy(lo))), shown(&Json::from(tidy(hi))))
    }
}

/// A factor the model itself points to, with the range it would start in and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggested {
    pub key: String,
    pub from: String,
    pub to: String,
    pub why: String,
}

/// Whether `word` is in `text` as a word of its own (both lower case): not inside a longer one.
fn has_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let edge = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    text.match_indices(word)
        .any(|(i, w)| edge(text[..i].chars().next_back()) && edge(text[i + w.len()..].chars().next()))
}

/// Whether an assumption speaks of a setting: by its name, or by its words (`_` as spaces).
fn mentions(assumption: &str, name: &str) -> bool {
    let (text, name) = (assumption.to_lowercase(), name.to_lowercase());
    has_word(&text, &name) || has_word(&text, &name.replace('_', " "))
}

/// The factors a model points to that are not taken yet: its parameters with a range (that range), then the
/// numeric settings its nodes' assumptions speak of (the node's own, and the model's inputs any node's
/// assumptions speak of), with the default range.
pub fn suggestions(m: &Model, knobs: &[Knob], taken: &[Range]) -> Vec<Suggested> {
    let mut out: Vec<Suggested> = Vec::new();
    let free = |out: &[Suggested], key: &str| !taken.iter().any(|t| t.key == key) && !out.iter().any(|o| o.key == key);
    for p in &m.parameters {
        if let (Some(lo), Some(hi)) = (p.min, p.max)
            && hi > lo
            && knobs.iter().any(|k| k.key == p.key && k.numeric)
            && free(&out, &p.key)
        {
            out.push(Suggested {
                key: p.key.clone(),
                from: shown(&Json::from(lo)),
                to: shown(&Json::from(hi)),
                why: "a parameter of the model, in its range".into(),
            });
        }
    }
    fn walk(nodes: &[Node], prefix: &str, all: &mut Vec<(String, String, String)>) {
        for n in nodes {
            let at = format!("{prefix}{}", n.id);
            for a in &n.assumptions {
                all.push((at.clone(), n.id.clone(), a.clone()));
            }
            if let Some(b) = &n.body {
                walk(&b.nodes, &format!("{at}/"), all);
            }
        }
    }
    // (the path of a node, its id, an assumption)
    let mut assumed = Vec::new();
    walk(&m.nodes, "", &mut assumed);
    for k in knobs.iter().filter(|k| k.numeric) {
        let found = if let Some(name) = k.key.strip_prefix('$') {
            assumed.iter().find(|(_, _, a)| mentions(a, name))
        } else {
            assumed.iter().find(|(at, _, a)| {
                k.key
                    .strip_prefix(&format!("{at}."))
                    .is_some_and(|name| !name.contains(['.', '/']) && mentions(a, name))
            })
        };
        if let Some((_, id, a)) = found
            && free(&out, &k.key)
        {
            let (from, to) = default_range(k);
            out.push(Suggested {
                key: k.key.clone(),
                from,
                to,
                why: format!("assumed, not measured: worth varying\n{id}: {a}"),
            });
        }
    }
    out
}

/// Take a factor in a sensitivity analysis (once), in `range` or else the start range of its setting, and show
/// the Sensitivity tab. False when the model has no such number to vary.
pub fn vary(st: &mut BatchState, knobs: &[Knob], key: &str, range: Option<(f64, f64)>) -> bool {
    let Some(k) = knobs.iter().find(|k| k.key == key && k.numeric) else { return false };
    st.kind = Kind::Sensitivity;
    if !st.sens.factors.iter().any(|x| x.key == key) {
        let (from, to) = match range {
            Some((lo, hi)) => (shown(&Json::from(lo)), shown(&Json::from(hi))),
            None => default_range(k),
        };
        st.sens.factors.push(Range { key: key.to_string(), from, to, points: String::new() });
    }
    true
}

/// The factors of a sensitivity analysis asked for (the tool `sensitivity`) with their ranges: those given, else 20 %
/// either side of the setting's value. The error names the factor that is not a number of the model, or has no
/// value to start from.
pub fn asked_ranges(
    m: &Model,
    r: &Registry,
    factors: &[(String, Option<f64>, Option<f64>)],
) -> Result<Vec<(String, f64, f64)>, String> {
    let knobs = knobs(m, r);
    let mut out = Vec::new();
    for (key, from, to) in factors {
        let k = knobs.iter().find(|k| k.key == *key && k.numeric).ok_or_else(|| {
            format!("{key}: not a number setting of the model (`node.setting` or `$input`; show_model lists them)")
        })?;
        let (lo, hi) = match (from, to) {
            (Some(lo), Some(hi)) => (*lo, *hi),
            (None, None) => {
                let (a, b) = default_range(k);
                match (a.parse::<f64>(), b.parse::<f64>()) {
                    (Ok(a), Ok(b)) => (a, b),
                    _ => return Err(format!("{key}: the model gives it no value to start from: give `from` and `to`")),
                }
            }
            _ => return Err(format!("{key}: give both `from` and `to`, or neither")),
        };
        if lo.partial_cmp(&hi) != Some(std::cmp::Ordering::Less) {
            return Err(format!("{key}: its range must go from a value to a larger one"));
        }
        out.push((key.clone(), lo, hi));
    }
    Ok(out)
}

/// What a statistic asked for is, in the fields of the tab: its kind, the class and the metric.
fn set_statistic(sens: &mut SensitivityState, text: &str) {
    let mut parts = text.trim().split(':');
    let kind = parts.next().unwrap_or_default();
    let (second, third) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    match kind {
        "cells" | "share" | "area" => {
            sens.statistic = kind.into();
            sens.class = second.into();
        }
        "metric" => {
            sens.statistic = kind.into();
            sens.metric = second.into();
            sens.class = third.into();
        }
        // mean, sum, min, max, changed, or a table's column
        other => sens.statistic = other.into(),
    }
}

/// Put a sensitivity analysis asked for by the assistant in the Sensitivity tab (its factors in place of those
/// there, and its method, design, replicates, output and statistic), for the user to run.
pub fn set_asked(st: &mut BatchState, ranges: &[(String, f64, f64)], ask: &crate::tools::SensitivityAsk) {
    st.kind = Kind::Sensitivity;
    st.sens.factors = ranges
        .iter()
        .map(|(key, lo, hi)| Range {
            key: key.clone(),
            from: shown(&Json::from(*lo)),
            to: shown(&Json::from(*hi)),
            points: String::new(),
        })
        .collect();
    st.sens.method = ask.method;
    st.sens.design = Design { seed: st.sens.design.seed, ..ask.design() };
    st.sens.replicates = ask.replicates;
    if let Some(o) = &ask.output {
        st.sens.output = Some(o.clone());
    }
    if let Some(t) = &ask.statistic {
        set_statistic(&mut st.sens, t);
    }
    st.sens.maps = ask.maps;
    st.sens.keep_run_maps = ask.keep_run_maps;
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
pub fn start(
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
    let (tasks, plan, mut sensitivity) = tasks(st, &knobs)?;
    if tasks.is_empty() {
        return Err("no run to do".into());
    }
    let mut model = s.model.clone();
    // (a sensitivity analysis keeps nothing: only its numbers come back, unless it makes maps of sensitivity)
    let keep = if st.kind == Kind::Sensitivity {
        BTreeSet::new()
    } else {
        st.keep.clone().unwrap_or_else(|| default_keep(&s.model, r))
    };
    for key in keep {
        let name = key.replace(['.', '/'], "_");
        model.outputs.insert(name, terraflux_graph::Output { from: key, path: None, dtype: None, layer: None });
    }
    // the maps of sensitivity: each run keeps the last map of the output measured, and nothing else of the model's
    // outputs
    let maps = maps_of(st, s, r);
    if let Some(key) = &maps {
        sens_maps::keep_last_map(&mut model, key);
        if let Some(p) = sensitivity.as_mut() {
            p.maps = Some(MapsPlan { file: sens_maps::run_file(key), keep_runs: st.sens.keep_run_maps });
        }
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
                measure: None,
                statistic: None,
                windows: vec![1, 3, 5, 7, 9, 11],
            })
        }
        Kind::Sensitivity => {
            let output = sensitivity.as_ref().map(|p| p.output.clone()).unwrap_or_default();
            let ty = measurable(&s.model, r).into_iter().find(|(k, _)| *k == output).map(|(_, t)| t);
            let statistic = statistic_of(ty, &st.sens)?;
            if let Some(p) = sensitivity.as_mut() {
                p.statistic = statistic.clone();
            }
            // (a single map's `changed` is against the map the simulation starts from)
            let start = if statistic.as_deref() == Some("changed") && ty == Some(Type::Map) {
                st.sens
                    .start
                    .clone()
                    .or_else(|| default_start(&s.model))
                    .ok_or("choose the map the simulation starts from")?
            } else {
                String::new()
            };
            Some(Evaluation {
                measure: Some(output.clone()),
                statistic,
                output,
                observed: String::new(),
                band: None,
                start,
                windows: Vec::new(),
            })
        }
        _ => None,
    };
    let out = st.out.clone().unwrap_or_else(|| next_out(s, st.kind));
    let files = keeps_files(st.kind) || maps.is_some();
    let total = tasks.len();
    let b =
        Batch { name: format!("{} {}", s.title(), st.kind.name()), model, base: s.base(), tasks, files, evaluation };
    let record = Record {
        kind: st.kind,
        model: s.title(),
        out,
        files,
        job: None,
        on: st.on.clone(),
        total,
        plan,
        sensitivity,
        alive: now(),
    };
    let place = match &st.on {
        Some(name) => cluster_place(name)?,
        None => computers.pool(st.workers.clamp(1, total.max(1)))?.place(),
    };
    st.journal_for = s.path.clone().map(|p| (p, s.source.clone()));
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
    st.making = None;
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
pub(crate) fn knob_picker(
    ui: &mut Ui,
    id: &str,
    knobs: &[Knob],
    current: &mut String,
    filter: &mut String,
    hint: &str,
) -> bool {
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

/// The factors the model points to (see [`suggestions`]), a few at first, each with a button to take it.
fn suggested_ui(ui: &mut Ui, st: &mut BatchState, s: &Session, knobs: &[Knob]) {
    const FIRST: usize = 8;
    let list = suggestions(&s.source, knobs, &st.sens.factors);
    if list.is_empty() {
        return;
    }
    let mut take: Vec<Suggested> = Vec::new();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Suggested").strong()).on_hover_text(
            "the parameters of the model with a range, and the settings its nodes' assumptions speak of",
        );
        if ui.small_button("Add all").on_hover_text("take every one suggested, in the range shown").clicked() {
            take.extend(list.iter().cloned());
        }
    });
    let shown_n = if st.sens.more { list.len() } else { list.len().min(FIRST) };
    for sg in &list[..shown_n] {
        ui.horizontal(|ui| {
            if ui.small_button("+").on_hover_text(&sg.why).clicked() {
                take.push(sg.clone());
            }
            let range = if sg.from.is_empty() { String::new() } else { format!("  {} to {}", sg.from, sg.to) };
            ui.label(RichText::new(format!("{}{range}", sg.key)).monospace()).on_hover_text(&sg.why);
        });
    }
    if list.len() > shown_n && ui.small_button(format!("and {} more", list.len() - shown_n)).clicked() {
        st.sens.more = true;
    }
    for sg in take {
        if !st.sens.factors.iter().any(|x| x.key == sg.key) {
            st.sens.factors.push(Range { key: sg.key, from: sg.from, to: sg.to, points: String::new() });
        }
    }
}

fn sensitivity_ui(ui: &mut Ui, st: &mut BatchState, s: &Session, r: &Registry, knobs: &[Knob]) {
    ui.label(
        RichText::new(
            "How much each setting matters to an output. One at a time (OAT) is the quick look: each factor at its \
             ends, the others in the middle of their ranges. Morris finds the factors that matter; Sobol splits the \
             output's variance among them. Morris first, then Sobol on the factors that mattered.",
        )
        .weak(),
    );
    ui.horizontal(|ui| {
        ui.label("Method");
        let before = st.sens.method;
        for (m, name, hover) in [
            (Method::Oat, "One at a time", "2 runs a factor and the base: the swing of the output over each range"),
            (Method::Morris, "Morris", "elementary effects: trajectories through the ranges, mu* ranks the factors"),
            (
                Method::Sobol,
                "Sobol",
                "the share of the output's variance due to each factor (S1) and with its interactions (ST)",
            ),
        ] {
            ui.radio_value(&mut st.sens.method, m, name).on_hover_text(hover);
        }
        if st.sens.method != before {
            st.sens.design.n = st.sens.method.default_n();
        }
    });
    let mut drop = None;
    egui::Grid::new("factors").num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
        for h in ["factor", "from", "to", ""] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for (i, rg) in st.sens.factors.iter_mut().enumerate() {
            ui.label(RichText::new(&rg.key).monospace());
            ui.add(egui::TextEdit::singleline(&mut rg.from).desired_width(70.0));
            ui.add(egui::TextEdit::singleline(&mut rg.to).desired_width(70.0));
            if ui.small_button("x").clicked() {
                drop = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = drop {
        st.sens.factors.remove(i);
    }
    suggested_ui(ui, st, s, knobs);
    // a number is a factor (not a yes or no, nor a text)
    let numeric: Vec<Knob> = knobs.iter().filter(|k| k.numeric).cloned().collect();
    let mut key = String::new();
    if knob_picker(ui, "sensitivity factor", &numeric, &mut key, &mut st.filter, "+ Factor to vary")
        && !st.sens.factors.iter().any(|x| x.key == key)
        && let Some(k) = numeric.iter().find(|k| k.key == key)
    {
        let (from, to) = default_range(k);
        st.sens.factors.push(Range { key, from, to, points: String::new() });
    }
    ui.separator();
    egui::Grid::new("sensitivity design").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        match st.sens.method {
            Method::Oat => {}
            Method::Morris => {
                ui.label("Trajectories");
                ui.add(egui::DragValue::new(&mut st.sens.design.n).range(2..=10_000))
                    .on_hover_text("each of factors + 1 runs; 10 to 20 is usual");
                ui.end_row();
                ui.label("Levels of the grid");
                ui.add(egui::DragValue::new(&mut st.sens.design.levels).range(2..=20));
                ui.end_row();
            }
            Method::Sobol => {
                ui.label("Base points");
                egui::ComboBox::from_id_salt("sobol base points")
                    .selected_text(st.sens.design.n.to_string())
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for e in 3..=14 {
                            ui.selectable_value(&mut st.sens.design.n, 1usize << e, (1usize << e).to_string());
                        }
                    })
                    .response
                    .on_hover_text("a power of 2; each takes factors + 2 runs (2 x factors + 2 with the second order)");
                ui.end_row();
                ui.label("Second order");
                ui.checkbox(&mut st.sens.design.second_order, "the pairs of factors too")
                    .on_hover_text("S2 in s2.csv; nearly doubles the runs");
                ui.end_row();
            }
        }
        if st.sens.method != Method::Oat {
            ui.label("Bootstraps");
            ui.add(egui::DragValue::new(&mut st.sens.resamples).range(10..=10_000))
                .on_hover_text("the resamples of the indices' intervals");
            ui.end_row();
            ui.label("Design seed");
            ui.add(egui::DragValue::new(&mut st.sens.design.seed));
            ui.end_row();
        }
        ui.label("Model seed");
        ui.add(egui::DragValue::new(&mut st.first_seed))
            .on_hover_text("every run takes this seed (with replicates: this one and the next ones)");
        ui.end_row();
        let has_seed = !st.seeds.clone().unwrap_or_else(|| default_seeds(knobs)).is_empty();
        ui.label("Replicates at each point");
        ui.add_enabled(has_seed, egui::DragValue::new(&mut st.sens.replicates).range(1..=50))
            .on_hover_text("runs of each point with seeds from the model seed up; the output is their mean");
        ui.end_row();
        ui.label("Output measured");
        let outs = measurable(&s.model, r);
        if st.sens.output.is_none() {
            st.sens.output = default_measure(&s.model, r);
        }
        let current = st.sens.output.clone().unwrap_or_default();
        egui::ComboBox::from_id_salt("sensitivity output").selected_text(&current).width(220.0).show_ui(ui, |ui| {
            for (k, t) in &outs {
                if ui.selectable_label(*k == current, k).on_hover_text(t.name()).clicked() {
                    st.sens.output = Some(k.clone());
                }
            }
        });
        ui.end_row();
        match outs.iter().find(|(k, _)| *k == current).map(|(_, t)| *t) {
            Some(Type::Number | Type::Integer) => {}
            Some(Type::Table) => {
                ui.label("Column");
                ui.add(egui::TextEdit::singleline(&mut st.sens.statistic).desired_width(120.0).hint_text("the last"))
                    .on_hover_text("the column of the table's first row (empty: the last)");
                ui.end_row();
            }
            ty => {
                const KINDS: [(&str, &str); 9] = [
                    ("mean", "the mean of the cells"),
                    ("sum", "the sum of the cells"),
                    ("min", "the smallest cell"),
                    ("max", "the largest cell"),
                    ("cells", "the number of cells of a class"),
                    ("share", "the fraction (0 to 1) of the cells with a value that are of a class"),
                    (
                        "area",
                        "the area of a class: hectares when the grid is projected in metres, else map units squared",
                    ),
                    ("changed", "the fraction of cells whose value changed between the first and last map"),
                    ("metric", "a landscape metric of a class"),
                ];
                if !KINDS.iter().any(|(k, _)| *k == st.sens.statistic) {
                    st.sens.statistic = "mean".into();
                }
                ui.label("Of the map");
                fn label(k: &str) -> &str {
                    match k {
                        "cells" => "cells of a class",
                        "share" => "share of a class",
                        "area" => "area of a class (ha or map units\u{b2})",
                        "changed" => "cells that changed",
                        "metric" => "landscape metric of a class",
                        k => k,
                    }
                }
                egui::ComboBox::from_id_salt("sensitivity statistic")
                    .selected_text(label(&st.sens.statistic))
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for (k, hint) in KINDS {
                            ui.selectable_value(&mut st.sens.statistic, k.to_string(), label(k)).on_hover_text(hint);
                        }
                    });
                ui.end_row();
                let kind = st.sens.statistic.clone();
                if kind == "metric" {
                    ui.label("Metric");
                    egui::ComboBox::from_id_salt("sensitivity metric").selected_text(&st.sens.metric).show_ui(
                        ui,
                        |ui| {
                            for (m, hint) in [
                                ("np", "number of patches"),
                                ("pland", "percentage of the landscape"),
                                ("ed", "edge density"),
                                ("area_mn", "mean patch area"),
                                ("lpi", "largest patch index"),
                            ] {
                                ui.selectable_value(&mut st.sens.metric, m.to_string(), m).on_hover_text(hint);
                            }
                        },
                    );
                    ui.end_row();
                }
                if matches!(kind.as_str(), "cells" | "share" | "area" | "metric") {
                    ui.label("Class");
                    ui.add(egui::TextEdit::singleline(&mut st.sens.class).desired_width(80.0).hint_text("a number"))
                        .on_hover_text("the value of the class in the map's cells");
                    ui.end_row();
                }
                if kind == "changed" && ty == Some(Type::Map) {
                    ui.label("Against");
                    let starts = starts(&s.model);
                    let cur = st.sens.start.clone().or_else(|| default_start(&s.model)).unwrap_or_default();
                    egui::ComboBox::from_id_salt("sensitivity start").selected_text(&cur).width(220.0).show_ui(
                        ui,
                        |ui| {
                            for k in &starts {
                                if ui.selectable_label(*k == cur, k).clicked() {
                                    st.sens.start = Some(k.clone());
                                }
                            }
                        },
                    );
                    ui.end_row();
                }
            }
        }
    });
    maps_ui(ui, st, s, r, knobs);
    let hint = if st.seeds.clone().unwrap_or_else(|| default_seeds(knobs)).is_empty() {
        "the model has no seed: every run of a point is the same"
    } else {
        "a simulation that draws at random: 3 to 5 replicates separate the factors' effect from chance"
    };
    ui.label(RichText::new(hint).weak());
    seeds_ui(ui, st, knobs);
}

/// The choice of maps of sensitivity, cell by cell, under the output measured (when it is a map, series or stack).
fn maps_ui(ui: &mut Ui, st: &mut BatchState, s: &Session, r: &Registry, knobs: &[Knob]) {
    let Some(out) = st.sens.output.clone().filter(|o| is_map_output(&s.model, r, o)) else { return };
    ui.checkbox(&mut st.sens.maps, "Maps of sensitivity, cell by cell").on_hover_text(
        "the standard deviation of the output across the design and the index of each factor, in every cell, as \
         the bands of one GeoTIFF (sensitivity_maps.tif) for the Viewer",
    );
    if !st.sens.maps {
        return;
    }
    let runs = sensitivity_runs(st, knobs).unwrap_or(0);
    let bytes = crate::disk::map_bytes(&s.model, &s.base()) * runs as u64;
    let size = if bytes > 0 { format!(", about {} on disk", crate::disk::size_text(bytes)) } else { String::new() };
    let after = if st.sens.keep_run_maps { "and stay after" } else { "until the maps are made, then are deleted" };
    ui.label(RichText::new(format!("Each run keeps the last map of {out} ({runs} maps{size}) {after}.")).weak());
    ui.checkbox(&mut st.sens.keep_run_maps, "Keep each run's map")
        .on_hover_text("leave the runs' maps in their folders once the maps of sensitivity are made");
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
    theme::progress(
        ui,
        Some(frac),
        &format!("{} of {} runs", done + failed, r.total),
        (ui.available_width() - 120.0).max(80.0),
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
    let mut st_error = None;
    let mut show_maps = None;
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
            crate::theme::plot(Plot::new(("calibration", key)))
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
    } else if let Some(ind) = &f.sensitivity {
        if let Some(base) = ind.base {
            ui.label(format!("Output at the base (every factor in the middle of its range): {base:.6}"));
        }
        if let Some(c) = ind.chance {
            let text = format!(
                "Chance: {:.0} % of the output's variance comes from the seed, not the factors ({} replicates at each point)",
                c * 100.0,
                ind.replicates
            );
            if c > 0.5 {
                let warn = ui.visuals().warn_fg_color;
                ui.label(RichText::new(text).color(warn));
                ui.label(RichText::new("The factors explain little: more replicates or a larger range.").color(warn));
            } else {
                ui.label(text);
            }
        }
        if f.failed > 0 && ind.replicates > 1 {
            ui.label(format!("{} runs failed; their points are the mean of the replicates that ran.", f.failed));
        }
        crate::sens_chart::show(ui, ind);
        if ui
            .button("Save the chart...")
            .on_hover_text("as SVG or PNG; sensitivity.svg and sensitivity.png are in the folder already")
            .clicked()
            && let Some(p) = rfd::FileDialog::new()
                .set_title("Save the chart")
                .add_filter("SVG or PNG", &["svg", "png"])
                .set_file_name("sensitivity.svg")
                .save_file()
            && let Err(e) = crate::sens_chart::save(&p, ind)
        {
            st_error = Some(e);
        }
        if let Some(m) = &f.methods
            && ui
                .button("Copy the methods text")
                .on_hover_text("a paragraph for a methods section, with the references (methods.md is in the folder)")
                .clicked()
        {
            ui.ctx().copy_text(m.clone());
        }
        ui.label(RichText::new(format!("The factors that matter most first (by {}).", ind.ranked_by())).weak());
        egui::ScrollArea::horizontal().id_salt("indices").show(ui, |ui| {
            egui::Grid::new("indices").striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                ui.label(RichText::new("factor").strong());
                for c in &ind.columns {
                    ui.label(RichText::new(c).strong());
                }
                ui.end_row();
                for i in ind.order() {
                    ui.label(RichText::new(&ind.factors[i]).monospace());
                    for v in &ind.rows[i] {
                        ui.label(format!("{v:.4}"));
                    }
                    ui.end_row();
                }
            });
        });
        ui.label(
            RichText::new(
                "design.csv, indices.csv, convergence.csv, methods.md and the chart (sensitivity.svg and .png) are in \
                 the folder.",
            )
            .weak(),
        );
        if let Some(m) = &st.making {
            let share = m.share();
            theme::progress(
                ui,
                Some(share),
                &format!("making the maps of sensitivity: {:.0} %", share * 100.0),
                (ui.available_width() - 120.0).max(80.0),
            );
        }
        if let Some(m) = &f.maps {
            let name = m.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Maps of sensitivity: {name}"));
                if ui
                    .button("Show in the Viewer")
                    .on_hover_text(
                        "the bands as layers of the Viewer: the mean, the spread and the index of each factor",
                    )
                    .clicked()
                {
                    show_maps = Some(m.path.clone());
                }
            });
            ui.label(RichText::new(format!("Bands: {}", m.bands.join(", "))).weak());
            if m.missing > 0 {
                ui.label(format!(
                    "{} runs kept no map; their points are the mean of the replicates that did.",
                    m.missing
                ));
            }
        }
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
    if st_error.is_some() {
        st.error = st_error;
    }
    if let Some(p) = show_maps {
        crate::builder::drop_on_viewer(s, r, &[p]);
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

/// Whether a batch may start as far as the disk goes: one that keeps files and may not fit asks first
/// ([`BatchState::disk_ask`]); one that leaves little is told.
fn disk_ok(st: &mut BatchState, s: &mut Session, r: &Registry, runs: usize) -> bool {
    let bytes = batch_bytes(st, s, r, runs);
    if bytes == 0 || s.path.is_none() {
        return true;
    }
    match crate::disk::plan(bytes, &out_dir(st, s), s.disk_space) {
        crate::disk::Plan::Go => true,
        crate::disk::Plan::Warn(text) => {
            s.notice = Some((text, Instant::now()));
            true
        }
        crate::disk::Plan::Ask(text) => {
            st.disk_ask = Some(crate::disk::Ask { text });
            false
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
        let count = match st.kind {
            Kind::Sensitivity => sensitivity_runs(st, &knobs),
            _ => tasks(st, &knobs).map(|(t, ..)| t.len()),
        };
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!busy, |ui| {
                for (k, name) in [
                    (Kind::Replicates, "Replicates"),
                    (Kind::Scenarios, "Scenarios"),
                    (Kind::Calibration, "Calibration"),
                    (Kind::Sensitivity, "Sensitivity"),
                ] {
                    ui.selectable_value(&mut st.kind, k, name);
                }
            });
            ui.separator();
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
                if resp.clicked() && disk_ok(st, s, r, count.as_ref().map_or(0, |n| *n)) {
                    st.error = start(st, s, r, computers, ui.ctx()).err();
                }
            }
        });
        if st.running.is_none()
            && let Ok(n) = &count
        {
            cost_ui(ui, st, s, r, &knobs, *n);
        }
        match crate::disk::window(ui.ctx(), &mut st.disk_ask) {
            Some(crate::disk::Answer::Anyway) => st.error = start(st, s, r, computers, ui.ctx()).err(),
            Some(crate::disk::Answer::Clean) => s.open_clean(),
            _ => {}
        }
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
                    Kind::Sensitivity => sensitivity_ui(ui, st, s, r, &knobs),
                }
                ui.separator();
                if !matches!(st.kind, Kind::Calibration | Kind::Sensitivity) {
                    keep_ui(ui, st, s, r);
                }
                ui.horizontal(|ui| {
                    ui.label("Results in");
                    let shown = st.out.clone().unwrap_or_else(|| next_out(s, st.kind)).display().to_string();
                    // its buttons at the right, the folder in the room left (cut short, whole under the pointer)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if st.out.is_some()
                            && ui.small_button("x").on_hover_text("a new folder for every batch").clicked()
                        {
                            st.out = None;
                        }
                        if ui.small_button("Change...").clicked()
                            && let Some(p) = rfd::FileDialog::new().set_title("The folder of the results").pick_folder()
                        {
                            st.out = Some(p);
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(RichText::new(&shown).monospace().weak()).truncate())
                                .on_hover_text(&shown);
                        });
                    });
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
    fn a_batch_takes_rounds_of_runs_at_once() {
        assert_eq!(batch_time(5, 4.0, 2), 12.0);
        assert_eq!(batch_time(4, 4.0, 2), 8.0);
        assert_eq!(batch_time(1, 4.0, 8), 4.0);
        assert_eq!(batch_time(3, 4.0, 0), 12.0);
        assert_eq!(one_run_time(&BTreeMap::new()), None);
        assert_eq!(one_run_time(&[("a".to_string(), 1.5), ("b".to_string(), 2.5)].into()), Some(4.0));
        let (t, long) = cost_text("", 12, Some(60.0), Some(2), "here");
        assert!(t.starts_with("12 runs, about 6 min here (2 at once"), "{t}");
        assert!(!long);
        assert!(cost_text("", 12, Some(3600.0), Some(2), "here").1);
        let (t, long) = cost_text("", 3, None, Some(2), "here");
        assert!(t.contains("known once the model has run") && !long, "{t}");
    }

    #[test]
    fn a_maps_statistic_is_composed_from_its_kind_and_class() {
        let mut s = SensitivityState::default();
        assert_eq!(statistic_of(Some(Type::Map), &s), Ok(Some("mean".into())));
        assert_eq!(statistic_of(Some(Type::Number), &s), Ok(None));
        s.statistic = "share".into();
        assert!(statistic_of(Some(Type::Map), &s).is_err());
        s.class = " 3 ".into();
        assert_eq!(statistic_of(Some(Type::Series), &s), Ok(Some("share:3".into())));
        s.statistic = "metric".into();
        s.metric = "pland".into();
        assert_eq!(statistic_of(Some(Type::Map), &s), Ok(Some("metric:pland:3".into())));
        s.statistic = "changed".into();
        assert_eq!(statistic_of(Some(Type::Map), &s), Ok(Some("changed".into())));
    }

    #[test]
    fn a_plan_kept_before_the_maps_still_opens_and_keeps_them_after() {
        let old = r#"{"method":"oat","design":{"n":1,"levels":4,"second_order":false,"seed":1},"keys":["a.b"],
            "rows":[[1.0]],"labels":["x"],"output":"m.o","resamples":10}"#;
        let p: SensitivityPlan = serde_json::from_str(old).unwrap();
        assert!(p.maps.is_none());
        let p = SensitivityPlan { maps: Some(MapsPlan { file: "m_o.tif".into(), keep_runs: true }), ..p };
        let back: SensitivityPlan = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        let m = back.maps.unwrap();
        assert_eq!((m.file.as_str(), m.keep_runs), ("m_o.tif", true));
    }

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
    fn a_factor_starts_20_percent_either_side() {
        let k = |value: Json, integer: bool, default: Option<f64>| Knob {
            key: "a.b".into(),
            numeric: true,
            integer,
            about: String::new(),
            value,
            default,
        };
        assert_eq!(default_range(&k(Json::from(10.0), false, None)), ("8.0".into(), "12.0".into()));
        assert_eq!(default_range(&k(Json::from(0.5), false, None)), ("0.4".into(), "0.6".into()));
        assert_eq!(default_range(&k(Json::from(-10.0), false, None)), ("-12.0".into(), "-8.0".into()));
        assert_eq!(default_range(&k(Json::from(7), true, None)), ("5".into(), "9".into()));
        assert_eq!(default_range(&k(Json::from(1), true, None)), ("0".into(), "2".into()));
        assert_eq!(default_range(&k(Json::Null, false, Some(4.0))), ("3.2".into(), "4.8".into()));
        assert_eq!(default_range(&k(Json::Null, false, None)), (String::new(), String::new()));
        assert_eq!(default_range(&k(Json::from(0), true, None)), (String::new(), String::new()));
        assert_eq!(doc_default("the levels (default 4)"), Some(4.0));
        assert_eq!(doc_default("no value here"), None);
    }

    #[test]
    fn an_analysis_asked_for_by_the_assistant_is_put_in_the_tab() {
        let r = Registry::standard();
        let mut m = Model::new("m");
        m.inputs.insert("share".into(), Input::Value { value: 0.5.into(), note: None });
        m.nodes.push(Node::new("noise", "nlm.random").param("rows", 10).param("columns", 12));
        let some = |k: &str, a: Option<f64>, b: Option<f64>| (k.to_string(), a, b);
        // the range given, else 20 % either side (whole numbers widened)
        let got = asked_ranges(&m, &r, &[some("noise.rows", Some(4.0), Some(20.0)), some("$share", None, None)]);
        assert_eq!(got.unwrap(), [("noise.rows".to_string(), 4.0, 20.0), ("$share".to_string(), 0.4, 0.6)]);
        let err = |f: &[(String, Option<f64>, Option<f64>)]| asked_ranges(&m, &r, f).unwrap_err();
        assert!(err(&[some("noise.nope", None, None)]).contains("not a number setting"));
        assert!(err(&[some("noise.rows", Some(4.0), None)]).contains("both"));
        assert!(err(&[some("noise.rows", Some(9.0), Some(3.0))]).contains("larger"));
        // set in the tab: the factors in place of those there, and the settings
        let ask = crate::tools::sensitivity_ask(&serde_json::json!({
            "factors": ["noise.rows"], "method": "sobol", "n": 32, "second_order": true, "replicates": 3,
            "output": "noise.map", "statistic": "metric:np:2",
        }))
        .unwrap();
        let mut st = BatchState::default();
        st.sens.factors.push(Range { key: "old".into(), ..Default::default() });
        set_asked(&mut st, &[("noise.rows".into(), 4.0, 20.0)], &ask);
        assert_eq!(st.kind, Kind::Sensitivity);
        assert_eq!(st.sens.factors.len(), 1);
        assert_eq!((st.sens.factors[0].key.as_str(), st.sens.factors[0].from.as_str()), ("noise.rows", "4.0"));
        assert_eq!((st.sens.method, st.sens.design.n, st.sens.design.second_order), (Method::Sobol, 32, true));
        assert_eq!((st.sens.replicates, st.sens.output.as_deref()), (3, Some("noise.map")));
        assert_eq!(
            (st.sens.statistic.as_str(), st.sens.metric.as_str(), st.sens.class.as_str()),
            ("metric", "np", "2")
        );
        set_statistic(&mut st.sens, "share:3");
        assert_eq!((st.sens.statistic.as_str(), st.sens.class.as_str()), ("share", "3"));
        set_statistic(&mut st.sens, "changed");
        assert_eq!(st.sens.statistic, "changed");
    }

    #[test]
    fn the_model_suggests_its_ranged_parameters_and_assumed_settings() {
        let r = Registry::standard();
        let mut m = Model::new("m");
        m.inputs.insert("share".into(), Input::Value { value: 0.5.into(), note: None });
        let mut noise = Node::new("noise", "nlm.random").param("rows", 10).param("columns", 12);
        noise.assumptions = vec!["The grid is 10 ROWS tall, a guess".into(), "Nothing about the shape".into()];
        let mut other = Node::new("other", "nlm.random").param("rows", 8);
        other.assumptions = vec!["Drawn at random, with $share of the cells filled (assumed)".into()];
        m.nodes.push(noise);
        m.nodes.push(other);
        m.parameters.push(terraflux_graph::Parameter {
            key: "noise.columns".into(),
            min: Some(5.0),
            max: Some(20.0),
            ..Default::default()
        });
        m.parameters.push(terraflux_graph::Parameter {
            key: "other.rows".into(),
            min: Some(1.0),
            ..Default::default()
        });
        let k = knobs(&m, &r);
        let got = suggestions(&m, &k, &[]);
        let keys: Vec<&str> = got.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(keys, ["noise.columns", "$share", "noise.rows"], "ranged parameters first, then the assumed");
        assert_eq!((got[0].from.as_str(), got[0].to.as_str()), ("5.0", "20.0"));
        assert_eq!(got[0].why, "a parameter of the model, in its range");
        assert_eq!((got[1].from.as_str(), got[1].to.as_str()), ("0.4", "0.6"));
        assert!(got[2].why.contains("assumed, not measured") && got[2].why.contains("10 ROWS"), "{}", got[2].why);
        assert!(!keys.contains(&"other.rows"), "a parameter without both ends, and no assumption of its own");
        // what is taken is not suggested again
        let taken = [Range { key: "noise.rows".into(), ..Default::default() }];
        assert_eq!(suggestions(&m, &k, &taken).len(), 2);
        // a word inside another is not a mention
        assert!(
            mentions("the rows are many", "rows")
                && !mentions("arrows", "rows")
                && mentions("the Grid size", "grid_size")
        );
        // varying a setting takes it once, and shows the tab
        let mut st = BatchState::default();
        assert!(vary(&mut st, &k, "noise.rows", None) && vary(&mut st, &k, "noise.rows", None));
        assert_eq!(st.kind, Kind::Sensitivity);
        assert_eq!(st.sens.factors.len(), 1);
        assert_eq!((st.sens.factors[0].from.as_str(), st.sens.factors[0].to.as_str()), ("8", "12"));
        assert!(vary(&mut st, &k, "noise.columns", Some((5.0, 20.0))));
        assert_eq!(st.sens.factors[1].to, "20.0");
        assert!(!vary(&mut st, &k, "nowhere.x", None));
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
            sensitivity: None,
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
