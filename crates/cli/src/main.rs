//! `terraflux`: run, check and create land change models. Without a command (a double click), or with only a
//! model file (dropped on it, "open with"), it opens the application.

mod cluster;
mod mcp;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use terraflux_exec::{Engine, Observer, load_inputs, write_outputs};
use terraflux_graph::{Input, Model, Severity, check};
use terraflux_ops::presets;

#[derive(Parser)]
#[command(name = "terraflux", version, about = "Land use and land cover change modelling")]
struct Cli {
    /// Threads to use (default: all the cores but one; never every core).
    #[arg(long, global = true)]
    threads: Option<usize>,
    /// The command (none: the application).
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the application (with a model, if given).
    App { model: Option<PathBuf> },
    /// Run a model and write its outputs.
    Run {
        /// The model (.toml or .json).
        model: PathBuf,
        /// The folder of the outputs (default: `output` next to the model).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Replace an input (`name=path` for a file, `name=value` (JSON) for a value) or a node's setting
        /// (`node.setting=value`); e.g. `--set seed=7`, `--set patches.isometry=2`.
        #[arg(long = "set", value_name = "NAME=VALUE")]
        set: Vec<String>,
        /// Run a scenario of the model (its values, before any --set).
        #[arg(long)]
        scenario: Option<String>,
        /// Print only errors.
        #[arg(short, long)]
        quiet: bool,
        /// Print how long every node took and about how much memory what it gave holds, the slowest first.
        #[arg(long)]
        profile: bool,
        /// Run on tiles (maps larger than memory; Dinamica EGO's model): the same maps with any number of
        /// threads or machines (the tile size changes the patches a fit measures), statistically equivalent to
        /// the exact engine.
        #[arg(long)]
        tiled: bool,
        /// The side of the tiles (cells).
        #[arg(long, default_value_t = 1024)]
        tile: usize,
        /// The largest distance of a patch's cell from its seed (cells).
        #[arg(long, default_value_t = 32)]
        radius: usize,
        /// Keep the steps' layers in this folder instead of memory.
        #[arg(long)]
        disk: Option<PathBuf>,
        /// Run the tiles on a coordinator's workers (with --tiled and --disk): the maps and the layers' tiles go
        /// to them through the coordinator.
        #[arg(long)]
        coordinator: Option<String>,
        /// The coordinator's pairing code.
        #[arg(long)]
        code: Option<String>,
        /// The workers read the model's files and --disk on storage they share with this machine, at the same
        /// paths (only sums travel).
        #[arg(long)]
        shared_disk: bool,
    },
    /// Check a model and its data without running it.
    Check { model: PathBuf },
    /// Run again what a run's manifest (manifest.json, written with its outputs) records: the same model, scenario,
    /// values and files (told when one changed), and tell whether every output is the same, byte for byte (vectors:
    /// the same features).
    Reproduce {
        /// The manifest.json of the run.
        manifest: PathBuf,
        /// The folder of the outputs (default: `reproduced` next to the manifest).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Print only what differs.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Test models: run each (a folder: its models) and check what its [[tests]] say against the files of what they
    /// should be, and its check.assert nodes; `--update` writes those files from this run instead.
    Test {
        /// The models, or folders of models.
        #[arg(required = true)]
        models: Vec<PathBuf>,
        /// Write the files of what the outputs should be from this run.
        #[arg(long)]
        update: bool,
        /// Print only the failures.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Write a model as a document: its data, preparation, model, validation and results, step by step, with the
    /// note, why and settings of each (an HTML page, or a PDF by the extension).
    Report {
        model: PathBuf,
        /// The report's file, .html or .pdf (default: the model's name and .html, next to it).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Run the model first: the maps and tables of the steps in the report.
        #[arg(long)]
        run: bool,
    },
    /// List the operators (`--json`: their full signatures).
    Ops {
        #[arg(long)]
        json: bool,
    },
    /// Create a model from a classic land change model.
    #[command(subcommand)]
    New(Box<New>),
    /// Write a model in another format (by the extension: .toml or .json).
    Convert { from: PathBuf, to: PathBuf },
    /// A server of the Model Context Protocol on the standard input and output: an AI assistant (Claude, Gemini
    /// CLI, Codex...) builds, checks and runs models through it; the application, with the model open, shows its
    /// edits as they are made.
    Mcp,
    /// Run a coordinator: workers ask it for tasks, clients submit batches to it.
    Serve {
        /// The address to listen on (127.0.0.1: this machine only; 0.0.0.0:7700 for the network).
        #[arg(long, default_value = "127.0.0.1:7700")]
        listen: std::net::SocketAddr,
        /// Seconds without news from a worker before its task goes to another.
        #[arg(long, default_value_t = 60)]
        timeout: u64,
        /// Where the data and the jobs are kept (default: the user's application data); a coordinator started
        /// again on it goes on with its jobs.
        #[arg(long)]
        data: Option<PathBuf>,
        /// A pairing code: TLS, and only the computers given the code (`--code` alone makes one, printed); the
        /// coordinator says on the local network where it is, to be found by its code.
        #[arg(long, num_args = 0..=1, default_missing_value = "new")]
        code: Option<String>,
        /// Say nothing on the network (the workers given its address).
        #[arg(long)]
        no_beacons: bool,
    },
    /// Run a headless worker: tasks from a coordinator, until stopped.
    Worker {
        /// The coordinator (`host:7700`); with no coordinator, the one of --code found on the network.
        #[arg(long)]
        coordinator: Option<String>,
        /// The coordinator's pairing code (with no --coordinator: the coordinator of the code found on the
        /// network).
        #[arg(long)]
        code: Option<String>,
        #[arg(long)]
        name: Option<String>,
        /// Where the data of the tasks is kept.
        #[arg(long)]
        cache: Option<PathBuf>,
        /// Take tasks only after this many minutes without the keyboard or the mouse (Windows and macOS).
        #[arg(long)]
        idle: Option<f64>,
        /// The threads of each task (default: all the cores but one).
        #[arg(long)]
        threads: Option<usize>,
    },
    /// The workers of a coordinator and what they do.
    Workers {
        #[arg(long)]
        coordinator: Option<String>,
        /// The coordinator's pairing code (with no --coordinator: the coordinator of the code found on the
        /// network).
        #[arg(long)]
        code: Option<String>,
    },
    /// The jobs of a coordinator, done or not.
    Jobs {
        #[arg(long)]
        coordinator: Option<String>,
        /// The coordinator's pairing code (with no --coordinator: the coordinator of the code found on the
        /// network).
        #[arg(long)]
        code: Option<String>,
    },
    /// Follow a job of a coordinator to its end and write its files (those written already kept).
    Results {
        job: String,
        /// A cluster of `terraflux cluster add` (reached by SSH).
        #[arg(long)]
        on: Option<String>,
        #[arg(long)]
        coordinator: Option<String>,
        /// The coordinator's pairing code (with no --coordinator: the coordinator of the code found on the
        /// network).
        #[arg(long)]
        code: Option<String>,
        #[arg(short, long, default_value = "batch")]
        out: PathBuf,
    },
    /// Run a batch of a model: replicates (one seed each) or scenarios, here or on a coordinator's workers.
    Submit {
        model: PathBuf,
        /// Runs with the seeds `--first-seed`, `--first-seed + 1`...
        #[arg(long)]
        replicates: Option<usize>,
        #[arg(long, default_value_t = 1)]
        first_seed: i64,
        /// Scenarios: a TOML file of `[[scenario]] name = "..." set = { "$seed" = 2, "node.setting" = 1.2 }`.
        #[arg(long)]
        scenarios: Option<PathBuf>,
        /// What holds the seed: an input (`seed`), a node's (`simulation.seed`), or several, separated by commas.
        #[arg(long, default_value = "seed")]
        seed_input: String,
        /// The folder of the results: one folder per task.
        #[arg(short, long, default_value = "batch")]
        out: PathBuf,
        /// Only submit the batch to the coordinator and print its job (`terraflux results` fetches it later).
        #[arg(long)]
        detach: bool,
        #[command(flatten)]
        at: cluster::Where,
    },
    /// Calibrate settings of a model against an observed map, here or on a coordinator's workers.
    Calibrate(Box<cluster::Calibrate>),
    /// A global sensitivity analysis of a model's output to its settings (Morris or Sobol, as SALib), here or on
    /// a coordinator's workers.
    Sensitivity(Box<cluster::Sensitivity>),
    /// Clusters and servers reached by SSH: saved once, used by `submit --on`, `calibrate --on`, `results --on`.
    #[command(subcommand)]
    Cluster(cluster::ClusterCommand),
    /// The setup script (cloud-init user data) of a machine rented in the cloud: a server with Terraflux (then
    /// `terraflux cluster add`), or, with --worker, a machine working for a server's coordinator.
    CloudInit {
        /// A machine working for a coordinator (its address on the provider's network, --coordinator).
        #[arg(long)]
        worker: bool,
        /// The coordinator: `host:port`, or the host of a server saved (--on) whose port is taken.
        #[arg(long)]
        coordinator: Option<String>,
        /// The server saved whose coordinator the worker works for (its code and port).
        #[arg(long)]
        on: Option<String>,
        /// The coordinator's code (without --on).
        #[arg(long)]
        code: Option<String>,
        /// The threads of each task of the worker.
        #[arg(long)]
        threads: Option<usize>,
        /// Where the Linux package of Terraflux is downloaded from (default: this version's release).
        #[arg(long)]
        package: Option<String>,
    },
    /// Resample a raster to the grid of another (of the same coordinate reference system).
    Resample {
        input: PathBuf,
        /// The raster whose grid to take.
        #[arg(long)]
        like: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// auto (classes: nearest or mode; continuous: bilinear or average), nearest, mode, bilinear, average.
        #[arg(long, default_value = "auto")]
        method: String,
    },
}

#[derive(Subcommand)]
enum New {
    /// Dinamica EGO's classic model: Markov rates, weights of evidence, Expander and Patcher.
    Dinamica(DinamicaArgs),
    /// CLUE-S (lulcc's CluesModel): logistic regressions, a linear demand, CLUE allocation.
    Clue(LulccArgs),
    /// The ordered allocation (lulcc's OrderedModel): logistic regressions, a linear demand.
    Ordered(LulccArgs),
}

#[derive(Args)]
struct DinamicaArgs {
    /// The map at the start of the calibration period: `file.tif` or `file.tif:band` (a name or a 0-based index).
    #[arg(long)]
    t0: String,
    /// The map at its end.
    #[arg(long)]
    t1: String,
    /// The drivers: a raster with one band per variable.
    #[arg(long)]
    drivers: String,
    /// Time steps in the calibration period.
    #[arg(long)]
    steps: usize,
    /// Steps to simulate (default: `steps`).
    #[arg(long)]
    simulate: Option<usize>,
    /// The map the simulation starts from (default: t0).
    #[arg(long)]
    start: Option<String>,
    /// The weights' buffer increment (default: a hundredth of each variable's range).
    #[arg(long)]
    increment: Option<f64>,
    /// The Expander's share of every transition (default: measured on the observed change).
    #[arg(long)]
    expansion: Option<f64>,
    #[arg(long, default_value_t = 1.5)]
    isometry: f64,
    #[arg(long, default_value_t = 10.0)]
    prune_factor: f64,
    #[arg(long, default_value_t = 1)]
    seed: i64,
    /// The model file to write (.toml or .json).
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Args)]
struct LulccArgs {
    /// The observed maps, one band per date (the demand is interpolated from them).
    #[arg(long)]
    observed: String,
    /// The date of each band of `observed`, e.g. `0,6,14`.
    #[arg(long, value_delimiter = ',')]
    times: Vec<f64>,
    /// The drivers: a raster with one band per variable.
    #[arg(long)]
    drivers: String,
    /// The map the simulation starts from and the regressions are fitted on (default: the first observed band).
    #[arg(long)]
    start: Option<String>,
    /// The drivers of each class's regression as JSON, e.g. `{"1": ["slope"], "2": ["slope", "dist"]}`.
    #[arg(long)]
    formulas: Option<String>,
    /// CLUE: the elasticity of each class (one value, or a list in class order), e.g. `0.2,0.2,0.2`.
    #[arg(long, value_delimiter = ',')]
    elasticity: Vec<f64>,
    /// Ordered: the order the classes are served in, e.g. `2,1,3`.
    #[arg(long, value_delimiter = ',')]
    order: Vec<f64>,
    /// Ordered: take the most suitable cells instead of drawing them.
    #[arg(long)]
    deterministic: bool,
    /// The allocation's settings as JSON (CLUE: jitter, scale, max_iter, max_diff, mean_diff).
    #[arg(long)]
    settings: Option<String>,
    /// The conversion matrix as JSON, in class order, e.g. `[[1,1,0],[0,1,0],[1,1,1]]`.
    #[arg(long)]
    allowed: Option<String>,
    /// Neighbourhood rules as JSON, e.g. `{"rules": {"1": 0.3, "2": 0.2}}`.
    #[arg(long)]
    neighbourhood: Option<String>,
    /// A map of restricted cells (non-zero keep their class).
    #[arg(long)]
    restrictions: Option<String>,
    /// A map of the steps each cell has had its class.
    #[arg(long)]
    history: Option<String>,
    #[arg(long, default_value_t = 1)]
    seed: i64,
    #[arg(short, long)]
    output: PathBuf,
}

/// The values `--set` gives a model: `node.setting=value` set in the model (a setting its operator does not take
/// refused), `name=value` the inputs' values or files to read instead (returned).
fn apply_set(m: &mut Model, set: &[String], engine: &Engine) -> Result<BTreeMap<String, String>> {
    // `name=value` replaces an input; `node.setting=value` a node's setting (as a scenario's)
    let mut overrides = BTreeMap::new();
    let mut settings = BTreeMap::new();
    for s in set {
        let (k, v) = s.split_once('=').with_context(|| format!("--set {s}: give name=value"))?;
        if k.contains('.') {
            let v = serde_json::from_str(v).unwrap_or_else(|_| serde_json::Value::String(v.to_string()));
            settings.insert(k.to_string(), v);
        } else {
            overrides.insert(k.to_string(), v.to_string());
        }
    }
    // a setting the node's operator does not take is a mistake (a misspelt name), not a silent no-op
    for k in settings.keys() {
        let (path, name) = k.rsplit_once('.').context("--set node.setting=value")?;
        let op = m.node_mut(path).with_context(|| format!("--set {k}: no node {path:?}"))?.op.clone();
        if let Some(sig) = engine.registry.sig(&op)
            && !sig.container
            && sig.extra.is_none()
            && !sig.params.iter().any(|p| p.name == name)
            && sig.input_spec(name).is_none()
        {
            let known: Vec<&str> = sig.params.iter().map(|p| p.name.as_str()).collect();
            bail!("--set {k}: {op} has no setting {name:?} (its settings: {})", known.join(", "));
        }
    }
    if !settings.is_empty() {
        m.apply(&settings)?;
    }
    Ok(overrides)
}

/// `terraflux reproduce`: a run made again from its manifest, its outputs compared with the manifest's.
fn reproduce(path: &Path, out: Option<PathBuf>, quiet: bool, engine: &Engine) -> Result<()> {
    use terraflux_exec::testing::{Manifest, hash};
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mf: Manifest = serde_json::from_str(&text).with_context(|| format!("{}: not a manifest", path.display()))?;
    let model = PathBuf::from(&mf.model.path);
    let mut changed = Vec::new();
    if mf.terraflux != env!("CARGO_PKG_VERSION") {
        changed.push(format!("Terraflux is {} now, {} then", env!("CARGO_PKG_VERSION"), mf.terraflux));
    }
    if hash(&model)?.sha256 != mf.model.sha256 {
        changed.push(format!("the model {} changed", model.display()));
    }
    let mut m = Model::load_expanded(&model).with_context(|| format!("reading {}", model.display()))?;
    if let Some(name) = &mf.scenario {
        m = m.with_scenario(name)?;
    }
    let mut overrides = apply_set(&mut m, &mf.set, engine)?;
    for (name, f) in &mf.files {
        match hash(Path::new(&f.path)) {
            Ok(h) if h.sha256 == f.sha256 => {}
            Ok(_) => changed.push(format!("the input {name} ({}) changed", f.path)),
            Err(e) => changed.push(format!("the input {name} ({}): {e}", f.path)),
        }
        overrides.insert(name.clone(), f.path.clone());
    }
    for c in &changed {
        eprintln!("  note: {c}");
    }
    let dir = model.parent().map(Path::to_path_buf).unwrap_or_default();
    let inputs = load_inputs(&m, &dir, &overrides)?;
    let t = Instant::now();
    let results = engine.run(&m, &inputs, &mut Progress { quiet: true, start: t })?;
    let out = out.unwrap_or_else(|| path.parent().map(Path::to_path_buf).unwrap_or_default().join("reproduced"));
    let files = write_outputs(&m, &results, &out)?;
    let mut differ = 0;
    for (name, file) in m.outputs.keys().zip(&files) {
        let Some(then) = mf.outputs.get(name) else { continue };
        let now = hash(file)?;
        let same = now.sha256 == then.sha256 || same_features(file, Path::new(&then.path));
        if !same {
            differ += 1;
        }
        if !same || !quiet {
            println!("{} {name} ({})", if same { "same   " } else { "DIFFERS" }, file.display());
        }
    }
    if differ > 0 {
        bail!("{differ} of {} outputs differ from the run of the manifest", mf.outputs.len());
    }
    if !quiet {
        println!("reproduced: every output the same ({:.1} s)", t.elapsed().as_secs_f64());
    }
    Ok(())
}

/// Two files of vectors with the same features (a GeoPackage records when it was written).
fn same_features(a: &Path, b: &Path) -> bool {
    let vector = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gpkg"));
    vector(a)
        && vector(b)
        && matches!((terraflux_gdal::read_vector(a, None), terraflux_gdal::read_vector(b, None)), (Ok(x), Ok(y)) if x == y)
}

/// `terraflux test`: every model run, its check.assert nodes and its tests checked (or their files written with
/// `--update`).
fn test(paths: &[PathBuf], update: bool, quiet: bool, engine: &Engine) -> Result<()> {
    let mut models = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(p)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|f| f.extension().is_some_and(|e| e == "toml" || e == "json"))
                .collect();
            found.sort();
            models.extend(found);
        } else {
            models.push(p.clone());
        }
    }
    let (mut passed, mut failed) = (0, 0);
    for path in &models {
        let name = path.display().to_string();
        let mut fail = |what: String| {
            failed += 1;
            println!("FAIL {name}: {what}");
        };
        let m = match Model::load_expanded(path) {
            Ok(m) => m,
            Err(e) => {
                fail(e.to_string());
                continue;
            }
        };
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let t = Instant::now();
        let results = match load_inputs(&m, &dir, &BTreeMap::new())
            .map_err(|e| e.to_string())
            .and_then(|i| engine.run(&m, &i, &mut Progress { quiet: true, start: t }).map_err(|e| e.to_string()))
        {
            Ok(r) => r,
            // (a check.assert that fails stops the run: its message says what)
            Err(e) => {
                fail(e);
                continue;
            }
        };
        let checks = count_asserts(&m.nodes);
        let mut bad = Vec::new();
        for test in &m.tests {
            let Some(got) = results.get(&test.from) else {
                bad.push(format!("{} gave nothing", test.from));
                continue;
            };
            let file = dir.join(&test.expected);
            if update {
                if let Some(d) = file.parent() {
                    std::fs::create_dir_all(d)?;
                }
                terraflux_exec::write_value(got, &file, full_precision(got))?;
                continue;
            }
            match terraflux_exec::testing::read_expected(&file) {
                Ok(want) => {
                    if let Some(d) = terraflux_exec::testing::differs(got, &want, test.tolerance.unwrap_or(1e-9)) {
                        bad.push(format!("{}: {d}", test.from));
                    }
                }
                Err(e) => bad.push(format!("{}: {e} (terraflux test --update writes it)", test.from)),
            }
        }
        if bad.is_empty() {
            passed += 1;
            if !quiet {
                let what = if update { "written" } else { "ok" };
                println!(
                    "{what:<4} {name}: {} test(s), {checks} check(s) ({:.1} s)",
                    m.tests.len(),
                    t.elapsed().as_secs_f64()
                );
            }
        } else {
            fail(bad.join("; "));
        }
    }
    if failed > 0 {
        bail!("{failed} of {} models failed", models.len());
    }
    if !quiet {
        println!("{passed} model(s) passed");
    }
    Ok(())
}

/// The cell type that keeps every digit of a value's maps (f64), when they are not whole numbers (those are written
/// in the smallest type that holds them, the same values).
fn full_precision(v: &terraflux_ops::Value) -> Option<&'static str> {
    use terraflux_ops::Value;
    let fraction = |d: &[f64]| d.iter().any(|x| x.fract() != 0.0);
    let any = match v {
        Value::Map(m) => fraction(&m.data),
        Value::Stack(s) => s.layers.iter().any(|l| fraction(l)),
        Value::Series(items) => items.iter().any(|x| matches!(x, Value::Map(m) if fraction(&m.data))),
        _ => false,
    };
    any.then_some("f64")
}

/// The check.assert nodes of a model (in its containers too).
fn count_asserts(nodes: &[terraflux_graph::Node]) -> usize {
    nodes
        .iter()
        .map(|n| usize::from(n.op == "check.assert") + n.body.as_ref().map_or(0, |b| count_asserts(&b.nodes)))
        .sum()
}

/// Prints the progress of a run.
struct Progress {
    quiet: bool,
    start: Instant,
}

impl Observer for Progress {
    fn step(&mut self, at: &str, step: usize, of: usize) {
        if !self.quiet {
            eprintln!("  {at}: step {step}/{of} ({:.1} s)", self.start.elapsed().as_secs_f64());
        }
    }

    fn warning(&mut self, at: &str, message: &str) {
        eprintln!("  warning: {at}: {message}");
    }
}

fn absolute(p: &str) -> Result<PathBuf> {
    let path = PathBuf::from(p);
    let abs = if path.is_absolute() { path } else { std::env::current_dir()?.join(path) };
    Ok(abs)
}

fn relative_to(path: &Path, dir: &Path) -> String {
    terraflux_graph::relative_path(path, dir)
}

/// `file.tif:band` -> (file, band); a Windows drive letter is not a band.
fn split_band(s: &str) -> (&str, Option<&str>) {
    match s.rfind(':') {
        Some(i) if i > 1 && !s[i + 1..].contains(['/', '\\']) => (&s[..i], Some(&s[i + 1..])),
        _ => (s, None),
    }
}

fn map_input(spec: &str, model_dir: &Path, note: &str) -> Result<Input> {
    let (file, band) = split_band(spec);
    Ok(Input::Map {
        path: relative_to(&absolute(file)?, model_dir),
        band: band.map(String::from),
        nodata: None,
        note: Some(note.to_string()),
    })
}

fn stack_input(spec: &str, model_dir: &Path, note: &str) -> Result<Input> {
    Ok(Input::Stack {
        path: relative_to(&absolute(spec)?, model_dir),
        bands: None,
        nodata: None,
        note: Some(note.to_string()),
    })
}

fn model_dir(output: &Path) -> Result<PathBuf> {
    let abs = absolute(&output.to_string_lossy())?;
    Ok(abs.parent().map(Path::to_path_buf).unwrap_or_default())
}

fn new_dinamica(a: &DinamicaArgs) -> Result<Model> {
    let dir = model_dir(&a.output)?;
    let mut m = Model::new("Dinamica EGO's classic model");
    m.description = Some("Markov rates, weights of evidence, Expander and Patcher, fitted on two maps.".into());
    m.inputs.insert("t0".into(), map_input(&a.t0, &dir, "the map at the start of the calibration period")?);
    m.inputs.insert("t1".into(), map_input(&a.t1, &dir, "the map at its end")?);
    m.inputs.insert("drivers".into(), stack_input(&a.drivers, &dir, "the explanatory variables")?);
    m.inputs
        .insert("seed".into(), Input::Value { value: a.seed.into(), note: Some("the seed of the simulation".into()) });
    let start = match &a.start {
        Some(s) => {
            m.inputs.insert("start".into(), map_input(s, &dir, "the map the simulation starts from")?);
            "start"
        }
        None => "t0",
    };
    presets::dinamica(
        &mut m,
        &presets::Dinamica {
            start: start.into(),
            steps: a.steps,
            simulate: a.simulate.unwrap_or(a.steps),
            increment: a.increment,
            expansion: a.expansion,
            isometry: a.isometry,
            prune_factor: a.prune_factor,
            ..Default::default()
        },
    );
    Ok(m)
}

fn parse_json(what: &str, text: &Option<String>) -> Result<Option<serde_json::Value>> {
    text.as_ref().map(|t| serde_json::from_str(t).with_context(|| format!("--{what} is not valid JSON"))).transpose()
}

fn new_lulcc(a: &LulccArgs, ordered: bool) -> Result<Model> {
    let dir = model_dir(&a.output)?;
    let mut m = Model::new(if ordered { "Ordered allocation (lulcc)" } else { "CLUE-S (lulcc)" });
    m.inputs.insert("observed".into(), stack_input(&a.observed, &dir, "the observed maps, one band per date")?);
    m.inputs.insert("drivers".into(), stack_input(&a.drivers, &dir, "the explanatory variables")?);
    // by default the first observed band, by its name when it has one (its index otherwise)
    let start = a.start.clone().unwrap_or_else(|| {
        let first = terraflux_ops::files::Rasters::open(&a.observed, None, None)
            .ok()
            .and_then(|r| r.layers.first().map(|l| l.name.clone()))
            .unwrap_or_else(|| "0".into());
        format!("{}:{first}", a.observed)
    });
    m.inputs.insert("start".into(), map_input(&start, &dir, "the map the simulation starts from")?);
    m.inputs
        .insert("seed".into(), Input::Value { value: a.seed.into(), note: Some("set.seed() of R's generator".into()) });
    if let Some(r) = &a.restrictions {
        m.inputs.insert("restrictions".into(), map_input(r, &dir, "restricted cells keep their class")?);
    }
    if let Some(h) = &a.history {
        m.inputs.insert("history".into(), map_input(h, &dir, "the steps each cell has had its class")?);
    }
    let mut allocation = parse_json("settings", &a.settings)?.unwrap_or_else(|| serde_json::json!({}));
    let obj = allocation.as_object_mut().context("--settings must be a JSON object")?;
    if ordered {
        if a.order.is_empty() {
            bail!("give the order the classes are served in (--order)");
        }
        obj.insert("method".into(), "ordered".into());
        obj.insert("order".into(), serde_json::json!(a.order));
        obj.insert("stochastic".into(), (!a.deterministic).into());
    } else {
        obj.insert("method".into(), "clue".into());
    }
    let elasticity = match a.elasticity.len() {
        0 => None,
        1 => Some(serde_json::json!(a.elasticity[0])),
        _ => Some(serde_json::json!(a.elasticity)),
    };
    presets::lulcc(
        &mut m,
        &presets::Lulcc {
            observed: "observed".into(),
            drivers: "drivers".into(),
            seed: "seed".into(),
            start: "start".into(),
            restrictions: a.restrictions.as_ref().map(|_| "restrictions".into()),
            history: a.history.as_ref().map(|_| "history".into()),
            times: a.times.clone(),
            formulas: parse_json("formulas", &a.formulas)?,
            allocation,
            elasticity,
            allowed: parse_json("allowed", &a.allowed)?,
            neighbourhood: parse_json("neighbourhood", &a.neighbourhood)?,
        },
    );
    Ok(m)
}

fn report(model: &Model, engine: &Engine) -> bool {
    let issues = check(model, &engine.registry);
    for i in &issues {
        eprintln!("{i}");
    }
    !issues.iter().any(|i| i.severity == Severity::Error)
}

/// Open the application; started from the file explorer, without the console Windows opened for it.
/// The threads asked for: on the command line, else (without `TERRAFLUX_THREADS`) in the application's settings.
fn settings_threads(asked: Option<usize>) -> Option<usize> {
    asked.or_else(|| std::env::var_os("TERRAFLUX_THREADS").is_none().then(terraflux_app::settings::threads).flatten())
}

/// A model read to run or check, its submodels expanded; what of it was written by an older Terraflux and brought
/// up to date (map algebra in the old syntax) told on the standard error.
fn read_model(path: &Path) -> Result<Model> {
    let (mut m, notes) = Model::load_noting(path).with_context(|| format!("reading {}", path.display()))?;
    for n in &notes {
        eprintln!("note: {n}");
    }
    if !notes.is_empty() {
        eprintln!("note: the file keeps the old syntax until it is saved (terraflux convert MODEL MODEL rewrites it)");
    }
    terraflux_graph::submodel::expand(&mut m, path.parent().unwrap_or(Path::new("")))
        .with_context(|| format!("reading {}", path.display()))?;
    Ok(m)
}

fn app(model: Option<PathBuf>) -> Result<()> {
    hide_own_console();
    terraflux_app::run(model).map_err(|e| anyhow::anyhow!("the application: {e}"))
}

/// Free the console when no other program shares it (a double click opened it for this one; a terminal's stays).
#[cfg(windows)]
fn hide_own_console() {
    unsafe extern "system" {
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
        fn FreeConsole() -> i32;
    }
    let mut ids = [0u32; 2];
    // SAFETY: the list has room for the 2 ids asked for
    if unsafe { GetConsoleProcessList(ids.as_mut_ptr(), 2) } == 1 {
        // SAFETY: no handle of the console is kept
        unsafe { FreeConsole() };
    }
}

#[cfg(not(windows))]
fn hide_own_console() {}

/// A size in bytes, as people read it (KB, MB, GB).
fn size(bytes: usize) -> String {
    let b = bytes as f64;
    match bytes {
        0 => "-".into(),
        n if n < 1 << 10 => format!("{n} B"),
        n if n < 1 << 20 => format!("{:.1} KB", b / 1024.0),
        n if n < 1 << 30 => format!("{:.1} MB", b / 1048576.0),
        _ => format!("{:.2} GB", b / 1073741824.0),
    }
}

fn main() -> Result<()> {
    // a model file alone: the application with it
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if let [one] = args.as_slice() {
        let p = PathBuf::from(one);
        if p.is_file()
            && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml") || e.eq_ignore_ascii_case("json"))
        {
            terraflux_ops::threads::init(settings_threads(None));
            return app(Some(p));
        }
    }
    let cli = Cli::parse();
    // the application's settings give the threads when neither the command nor the environment does
    let threads = if cli.command.is_none() { settings_threads(cli.threads) } else { cli.threads };
    terraflux_ops::threads::init(threads);
    let Some(command) = cli.command else {
        return app(None);
    };
    let engine = Engine::default();
    let rt = || tokio::runtime::Builder::new_multi_thread().enable_all().build();
    match command {
        Command::Serve { listen, timeout, data, code, no_beacons } => {
            rt()?.block_on(cluster::serve(listen, timeout, data, code, !no_beacons))?
        }
        Command::Worker { coordinator, code, name, cache, idle, threads } => {
            rt()?.block_on(cluster::worker(coordinator, code, name, cache, idle, threads))?
        }
        Command::Cluster(c) => cluster::cluster(c)?,
        Command::CloudInit { worker, coordinator, on, code, threads, package } => {
            print!("{}", cluster::cloud_init(worker, coordinator, on, code, threads, package)?)
        }
        Command::Workers { coordinator, code } => rt()?.block_on(cluster::workers(coordinator, code))?,
        Command::Jobs { coordinator, code } => rt()?.block_on(cluster::jobs(coordinator, code))?,
        Command::Results { job, on, coordinator, code, out } => {
            rt()?.block_on(cluster::results(coordinator, code, on, &job, &out))?
        }
        Command::Submit { model, replicates, first_seed, scenarios, seed_input, out, detach, at } => {
            let tasks = match (replicates, scenarios) {
                (Some(n), None) => {
                    terraflux_dist::batch::replicates(n, first_seed, &terraflux_dist::batch::seed_keys(&seed_input))
                }
                (None, Some(f)) => terraflux_dist::batch::scenarios(&std::fs::read_to_string(&f)?)?,
                _ => bail!("give --replicates or --scenarios"),
            };
            rt()?.block_on(cluster::submit(&model, tasks, &out, &at, detach))?;
        }
        Command::Calibrate(a) => rt()?.block_on(cluster::calibrate(&a))?,
        Command::Sensitivity(a) => rt()?.block_on(cluster::sensitivity(&a))?,
        Command::App { model } => app(model)?,
        Command::Run {
            model,
            out,
            set,
            scenario,
            quiet,
            profile,
            tiled,
            tile,
            radius,
            disk,
            coordinator,
            code,
            shared_disk,
        } => {
            let mut m = read_model(&model)?;
            if let Some(name) = &scenario {
                m = m.with_scenario(name)?;
            }
            let dir = model.parent().map(Path::to_path_buf).unwrap_or_default();
            if tiled {
                let mut o = BTreeMap::new();
                for s in &set {
                    let (k, v) = s.split_once('=').with_context(|| format!("--set {s}: give name=value"))?;
                    let v = serde_json::from_str(v).unwrap_or_else(|_| serde_json::Value::String(v.to_string()));
                    o.insert(if k.contains('.') { k.to_string() } else { format!("${k}") }, v);
                }
                m.apply(&o)?;
                let t = Instant::now();
                let opt = terraflux_engine::graph::Options {
                    tile,
                    settings: terraflux_engine::dinamica::Settings { radius, ..Default::default() },
                    disk,
                    runner: match &coordinator {
                        Some(url) => {
                            let code = code.as_deref().map(terraflux_dist::secure::Code::parse).transpose()?;
                            let mut r = terraflux_dist::tiles::Remote::connect_with(url, code.as_ref())?;
                            r.shared = shared_disk;
                            Some(std::sync::Arc::new(r))
                        }
                        None => None,
                    },
                };
                let out = out.unwrap_or_else(|| dir.join("output"));
                let report = terraflux_engine::graph::run(&m, &dir, &out, &opt, |s| {
                    if !quiet {
                        eprintln!("  {s} ({:.1} s)", t.elapsed().as_secs_f64());
                    }
                })?;
                if !quiet {
                    for f in &report.files {
                        eprintln!("wrote {}", f.display());
                    }
                    eprintln!("done in {:.1} s", t.elapsed().as_secs_f64());
                }
                return Ok(());
            }
            let overrides = apply_set(&mut m, &set, &engine)?;
            let t = Instant::now();
            let inputs = load_inputs(&m, &dir, &overrides)?;
            if !quiet {
                eprintln!("{}: inputs read ({:.1} s)", m.name.as_deref().unwrap_or("model"), t.elapsed().as_secs_f64());
            }
            let mut progress = Progress { quiet, start: t };
            // the nodes frozen in the application: their results kept next to the model, used instead of running them
            let folder = terraflux_exec::frozen::folder(&model);
            let mut held = terraflux_exec::Cache::default();
            for n in m.nodes.iter().filter(|n| n.frozen) {
                match terraflux_exec::frozen::read(&folder, &n.id) {
                    Ok((outputs, when)) => {
                        held.hold(&n.id, Some(outputs), when);
                        if !quiet {
                            eprintln!("{}: its result frozen ({})", n.id, folder.join(&n.id).display());
                        }
                    }
                    Err(e) => eprintln!("  note: {}: frozen, its result not read ({e}): it runs", n.id),
                }
            }
            let results = if m.nodes.iter().any(|n| n.frozen) {
                engine.run_with(&m, &inputs, &mut progress, Some(&std::sync::Mutex::new(held)), None)?
            } else {
                engine.run(&m, &inputs, &mut progress)?
            };
            if profile {
                let mut t = results.timings.clone();
                t.sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
                let total: f64 = results.timings.iter().filter(|t| !t.at.contains('/')).map(|t| t.seconds).sum();
                println!("{:>10}  {:>6}  {:>10}  node", "seconds", "share", "gave");
                for x in &t {
                    let share = if total > 0.0 { 100.0 * x.seconds / total } else { 0.0 };
                    println!("{:>10.3}  {share:>5.1}%  {:>10}  {}", x.seconds, size(x.bytes), x.at);
                }
            }
            let out = out.unwrap_or_else(|| dir.join("output"));
            let files = write_outputs(&m, &results, &out)?;
            // what the run was, to run it again and tell whether it gives the same
            let written: Vec<(String, PathBuf)> = m.outputs.keys().cloned().zip(files.iter().cloned()).collect();
            let manifest = terraflux_exec::testing::manifest(
                &std::path::absolute(&model)?,
                &m,
                &std::path::absolute(&dir)?,
                &overrides,
                scenario.as_deref(),
                &set,
                &written,
            )?;
            std::fs::write(out.join("manifest.json"), serde_json::to_string_pretty(&manifest)? + "\n")?;
            if !quiet {
                for f in files {
                    eprintln!("wrote {}", f.display());
                }
                eprintln!("wrote {} (terraflux reproduce runs it again)", out.join("manifest.json").display());
                eprintln!("done in {:.1} s", t.elapsed().as_secs_f64());
            }
        }
        Command::Reproduce { manifest, out, quiet } => reproduce(&manifest, out, quiet, &engine)?,
        Command::Test { models, update, quiet } => test(&models, update, quiet, &engine)?,
        Command::Report { model, out, run } => {
            let source = Model::load(&model).with_context(|| format!("reading {}", model.display()))?;
            let dir = model.parent().map(Path::to_path_buf).unwrap_or_default();
            let results = if run {
                let m = Model::load_expanded(&model)?;
                let inputs = load_inputs(&m, &dir, &BTreeMap::new())?;
                let mut progress = Progress { quiet: false, start: Instant::now() };
                Some(engine.run(&m, &inputs, &mut progress)?)
            } else {
                None
            };
            let doc = terraflux_app::document::Document::of(&source, &engine.registry);
            let values = |n: &str, p: &str| results.as_ref()?.values.get(n)?.get(p).cloned();
            let first = |n: &str| terraflux_app::document::only_output(&source, &engine.registry, n);
            let out = out.unwrap_or_else(|| model.with_extension("html"));
            terraflux_app::report::write(&out, &doc, &values, &first)
                .with_context(|| format!("writing {}", out.display()))?;
            eprintln!("wrote {}", out.display());
        }
        Command::Check { model } => {
            let m = read_model(&model)?;
            let mut ok = report(&m, &engine);
            let dir = model.parent().map(Path::to_path_buf).unwrap_or_default();
            match load_inputs(&m, &dir, &BTreeMap::new()) {
                Err(e) => {
                    eprintln!("error: the inputs: {e}");
                    ok = false;
                }
                Ok(inputs) => {
                    for f in terraflux_exec::lint::lint(&m, &inputs) {
                        eprintln!("{f}");
                        ok &= f.level != terraflux_exec::lint::Level::Error;
                    }
                }
            }
            if ok {
                eprintln!("{}: no errors", model.display());
            } else {
                std::process::exit(1);
            }
        }
        Command::Ops { json } => {
            let sigs = engine.registry.signatures();
            if json {
                println!("{}", serde_json::to_string_pretty(&sigs)?);
            } else {
                for s in sigs {
                    println!("{}\n    {}\n", s.name, s.doc);
                }
            }
        }
        Command::New(n) => {
            let (m, out) = match &*n {
                New::Dinamica(a) => (new_dinamica(a)?, &a.output),
                New::Clue(a) => (new_lulcc(a, false)?, &a.output),
                New::Ordered(a) => (new_lulcc(a, true)?, &a.output),
            };
            if !report(&m, &engine) {
                bail!("the model made has errors");
            }
            m.save(out)?;
            eprintln!("wrote {}", out.display());
        }
        Command::Convert { from, to } => {
            Model::load(&from)?.save(&to)?;
        }
        Command::Mcp => mcp::serve()?,
        Command::Resample { input, like, output, method } => {
            use terraflux_raster::resample::{Method, resample};
            let m = Method::parse(&method).with_context(|| format!("unknown method {method:?}"))?;
            let grid = terraflux_raster::Raster::read(&like)?.grid;
            let (r, used) = resample(&terraflux_raster::Raster::read(&input)?, &grid, m)?;
            r.write(&output)?;
            eprintln!("wrote {} ({used:?})", output.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run reproduced from its manifest, and a model tested against the files of what it should give.
    #[test]
    fn a_run_is_reproduced_and_a_model_tested() {
        let dir = std::env::temp_dir().join(format!("terraflux-repro-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let model = dir.join("m.toml");
        let text = |seed: u32| {
            format!(
                "format = \"terraflux/1\"\n\n[inputs.k]\nkind = \"value\"\nvalue = 0.5\n\n[[nodes]]\nid = \"land\"\n\
                 op = \"nlm.random\"\nparams = {{ rows = 30, columns = 20, seed = {seed} }}\n\n[[nodes]]\nid = \"high\"\n\
                 op = \"raster.calc\"\ninputs = {{ x = \"land.map\", k = \"$k\" }}\nparams = {{ expression = \"x * k\" }}\n\n\
                 [[nodes]]\nid = \"ok\"\nop = \"check.assert\"\ninputs = {{ m = \"high.map\" }}\n\
                 params = {{ expression = \"m <= 0.5\" }}\n\n[outputs.high]\nfrom = \"high.map\"\n\n[[tests]]\n\
                 from = \"high.map\"\nexpected = \"expected/high.tif\"\n"
            )
        };
        std::fs::write(&model, text(3)).unwrap();
        let engine = Engine::default();
        // a run and its manifest
        let m = Model::load_expanded(&model).unwrap();
        let inputs = load_inputs(&m, &dir, &BTreeMap::new()).unwrap();
        let results = engine.run(&m, &inputs, &mut terraflux_exec::Quiet).unwrap();
        let out = dir.join("out");
        let files = write_outputs(&m, &results, &out).unwrap();
        let written: Vec<(String, PathBuf)> = m.outputs.keys().cloned().zip(files).collect();
        let mf = terraflux_exec::testing::manifest(&model, &m, &dir, &BTreeMap::new(), None, &[], &written).unwrap();
        assert_eq!(mf.values["k"], serde_json::json!(0.5));
        assert_eq!(mf.seeds["land.seed"], serde_json::json!(3));
        let manifest = out.join("manifest.json");
        std::fs::write(&manifest, serde_json::to_string(&mf).unwrap()).unwrap();
        reproduce(&manifest, None, true, &engine).unwrap();
        // tested: the files written, then checked; another seed fails
        test(std::slice::from_ref(&model), true, true, &engine).unwrap();
        test(std::slice::from_ref(&dir), false, true, &engine).unwrap();
        std::fs::write(&model, text(4)).unwrap();
        let e = test(std::slice::from_ref(&model), false, true, &engine).unwrap_err().to_string();
        assert!(e.contains("1 of 1 models failed"), "{e}");
        // and the run of another seed does not reproduce the manifest's
        assert!(reproduce(&manifest, None, true, &engine).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bands_and_drive_letters() {
        assert_eq!(split_band("lu.tif:1997"), ("lu.tif", Some("1997")));
        assert_eq!(split_band("C:\\data\\lu.tif"), ("C:\\data\\lu.tif", None));
        assert_eq!(split_band("C:\\data\\lu.tif:0"), ("C:\\data\\lu.tif", Some("0")));
        assert_eq!(split_band("lu.tif"), ("lu.tif", None));
    }

    #[test]
    fn relative_paths() {
        let dir = Path::new("/a/b");
        assert_eq!(relative_to(Path::new("/a/b/c.tif"), dir), "c.tif");
        assert_eq!(relative_to(Path::new("/a/d/c.tif"), dir), "../d/c.tif");
    }
}
