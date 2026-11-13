//! `terraflux`: run, check and create land change models. Without a command (a double click), or with only a
//! model file (dropped on it, "open with"), it opens the application.

mod cluster;
mod mcp;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use terraflux_exec::journal::{Entry, Kind, Status};
use terraflux_exec::{Engine, Observer, load_inputs, write_outputs};
use terraflux_graph::{Input, Model, Severity, check};
use terraflux_ops::presets;

#[derive(Parser)]
#[command(name = "terraflux", version, about = "Land use and land cover change modelling")]
struct Cli {
    /// Threads to use (default and most: a quarter of the cores left free; never every core).
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
        /// Do not record the run in the model's journal (`.terraflux` beside the model, listed by the application's
        /// Runs pane).
        #[arg(long)]
        no_journal: bool,
        /// A check run, to see in seconds whether the model is right: one cell in every N, on each side, of the
        /// rasters it reads (the ones it draws are made N times smaller). The outputs are of the check, not the
        /// model's: metrics, distances and neighbourhoods change with the resolution.
        #[arg(long, default_value_t = 1, value_name = "N")]
        coarsen: u32,
        /// A check run on a part of the map: the rasters read are cut to this window (in the coordinate system of
        /// the model's first raster, else --window-crs), moved out to the cells. Without -o the outputs go to
        /// `<model> runs/check` next to the model, never over the real ones.
        #[arg(long, value_name = "XMIN,YMIN,XMAX,YMAX", allow_hyphen_values = true)]
        window: Option<String>,
        /// The coordinate system of --window (`EPSG:4326`, a code, or a WKT).
        #[arg(long, requires = "window", value_name = "CRS")]
        window_crs: Option<String>,
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
    /// Bring a Terraflux to the front: what a click on one of its notifications runs (`terraflux-front:<process>`).
    #[command(hide = true)]
    Front { uri: String },
    /// Find again the files a model reads that are not where it says (a model from another computer, a folder moved
    /// or renamed) and make it read them from there: each by the longest end of its old path found in a folder, else
    /// by its name, else by the change of folder of those found (Windows and Unix paths alike). The model's file is
    /// written again (relative paths), unless --dry-run.
    Relink {
        model: PathBuf,
        /// A folder to look in (repeat for more, in order; default: the model's folder, then the one above it).
        #[arg(long = "in", value_name = "FOLDER")]
        folders: Vec<PathBuf>,
        /// Say what would be found, without writing the model.
        #[arg(long)]
        dry_run: bool,
    },
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
        /// Run the frozen nodes too, instead of using their results kept next to the model.
        #[arg(long)]
        thaw: bool,
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
        /// Run the frozen nodes too, instead of using their results kept next to the model.
        #[arg(long)]
        thaw: bool,
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
        /// The language: en (default) or pt.
        #[arg(short, long, default_value = "en")]
        language: String,
    },
    /// Run a model and write one of its Plots (`[[plots]]`, kept with the model) as a picture of its map: the layers at
    /// their opacity, the features with their style and labels, as the Viewer draws it (no legend or decorations).
    Plot {
        model: PathBuf,
        /// The id of the plot.
        id: String,
        /// The picture: a .png, or a .tif (a GeoTIFF of red, green, blue and alpha on the map's grid, to open in QGIS).
        #[arg(short, long)]
        out: PathBuf,
        /// Pixels to a cell of the map: 1 (default), 2 or 4.
        #[arg(long, default_value_t = 1)]
        scale: u64,
        /// png or tif (default: by the extension of the file).
        #[arg(long)]
        format: Option<String>,
        /// Replace an input (`name=path` for a file, `name=value` (JSON) for a value) or a node's setting
        /// (`node.setting=value`); e.g. `--set seed=7`.
        #[arg(long = "set", value_name = "NAME=VALUE")]
        set: Vec<String>,
        /// Run a scenario of the model (its values, before any --set).
        #[arg(long)]
        scenario: Option<String>,
        /// Print only errors.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Draft the methods section of a paper from a model: the methods it uses with the papers to cite, the model
    /// step by step and the references; or the references as BibTeX or RIS.
    Methods {
        model: PathBuf,
        /// The language: en (default) or pt.
        #[arg(short, long, default_value = "en")]
        language: String,
        /// text (default), bibtex or ris.
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Write it to this file instead of printing it.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Write the model as a Python script of landschaft (each node a call of landschaft's function of the same
    /// method).
    Python {
        #[arg(required = true)]
        model: Vec<PathBuf>,
        /// Write it to this file instead of printing it (one model).
        #[arg(short, long, conflicts_with = "beside")]
        out: Option<PathBuf>,
        /// Write `<model>.py` beside each model (many models allowed).
        #[arg(long)]
        beside: bool,
        /// Fail when a node was left as a comment (no counterpart in landschaft).
        #[arg(long)]
        strict: bool,
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
        /// The threads of each task (default: a quarter of the cores left free).
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
fn reproduce(path: &Path, out: Option<PathBuf>, quiet: bool, thaw: bool, engine: &Engine) -> Result<()> {
    use terraflux_exec::testing::{Manifest, hash};
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mf: Manifest = serde_json::from_str(&text).with_context(|| format!("{}: not a manifest", path.display()))?;
    // the files from the manifest's folder first: the study's folder may have moved with it
    let here = folder_of(path);
    let model = mf.model.found(&here);
    if !model.is_file() {
        bail!("the model of the manifest is not found: neither {}", mf.model.places(&here));
    }
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
        let file = f.found(&here);
        if !file.exists() {
            bail!("the input {name} of the manifest is not found: neither {}", f.places(&here));
        }
        match hash(&file) {
            Ok(h) if h.sha256 == f.sha256 => {}
            Ok(_) => changed.push(format!("the input {name} ({}) changed", file.display())),
            Err(e) => changed.push(format!("the input {name} ({}): {e}", file.display())),
        }
        overrides.insert(name.clone(), file.to_string_lossy().into_owned());
    }
    for (at, f) in &mf.reads {
        let file = f.found(&here);
        match hash(&file) {
            Ok(h) if h.sha256 == f.sha256 => {}
            Ok(_) => changed.push(format!("the file {at} reads ({}) changed", file.display())),
            Err(_) => changed.push(format!("the file {at} read is not found: neither {}", f.places(&here))),
        }
    }
    // the frozen results: those of the run, unless told to run their nodes again
    let folder = terraflux_exec::frozen::folder(&model);
    if thaw {
        for n in m.nodes.iter_mut() {
            n.frozen = false;
        }
    } else {
        for (id, f) in &mf.frozen {
            let now = hash(&folder.join(id)).ok();
            if !m.nodes.iter().any(|n| n.frozen && &n.id == id) {
                changed.push(format!("{id} is no longer frozen: it runs"));
            } else if now.is_none() {
                changed.push(format!("the frozen result of {id} is gone ({}): it runs", folder.join(id).display()));
            } else if now.is_some_and(|h| h.sha256 != f.sha256) {
                changed.push(format!("the frozen result of {id} changed ({})", folder.join(id).display()));
            }
        }
        for n in m.nodes.iter().filter(|n| n.frozen && !mf.frozen.contains_key(&n.id)) {
            changed.push(format!("{} is frozen now, not then", n.id));
        }
    }
    for c in &changed {
        eprintln!("  note: {c}");
    }
    let dir = folder_of(&model);
    let inputs = load_inputs(&m, &dir, &overrides)?;
    let t = Instant::now();
    let results = run_frozen(engine, &model, &m, &inputs, &mut Progress { quiet: true, start: t }, true)?;
    let out = out.unwrap_or_else(|| here.join("reproduced"));
    let files = write_outputs(&m, &results, &out)?;
    let mut differ = 0;
    let mut tell = |same: bool, what: &str, file: &Path| {
        if !same {
            differ += 1;
        }
        if !same || !quiet {
            println!("{} {what} ({})", if same { "same   " } else { "DIFFERS" }, file.display());
        }
    };
    for (name, file) in m.outputs.keys().zip(&files) {
        let Some(then) = mf.outputs.get(name) else { continue };
        let now = hash(file)?;
        tell(now.sha256 == then.sha256 || same_features(file, &then.found(&here)), name, file);
    }
    // the files the nodes wrote, written again where they were (a GeoPackage records when: not compared)
    let written = terraflux_exec::testing::writes(&m, &dir);
    for (at, then) in &mf.writes {
        let Some(now) = written.get(at) else { continue };
        let file = PathBuf::from(&now.path);
        if file.extension().is_some_and(|e| e.eq_ignore_ascii_case("gpkg")) {
            if !quiet {
                println!("written {at} ({}): a GeoPackage, not compared", file.display());
            }
            continue;
        }
        tell(now.sha256 == then.sha256, at, &file);
    }
    if differ > 0 {
        bail!("{differ} of {} files differ from the run of the manifest", mf.outputs.len() + mf.writes.len());
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
fn test(paths: &[PathBuf], update: bool, quiet: bool, thaw: bool, engine: &Engine) -> Result<()> {
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
        let mut m = match Model::load_expanded(path) {
            Ok(m) => m,
            Err(e) => {
                fail(e.to_string());
                continue;
            }
        };
        // the frozen nodes' results used, as terraflux run uses them, unless told to run them again
        if thaw {
            m.nodes.iter_mut().for_each(|n| n.frozen = false);
        }
        let dir = folder_of(path);
        let t = Instant::now();
        let results = match load_inputs(&m, &dir, &BTreeMap::new()).map_err(|e| e.to_string()).and_then(|i| {
            run_frozen(engine, path, &m, &i, &mut Progress { quiet: true, start: t }, true).map_err(|e| e.to_string())
        }) {
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

/// The checks (check.*) of a model (in its containers too).
fn count_asserts(nodes: &[terraflux_graph::Node]) -> usize {
    nodes
        .iter()
        .map(|n| {
            usize::from(terraflux_ops::checks::is_check(&n.op)) + n.body.as_ref().map_or(0, |b| count_asserts(&b.nodes))
        })
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

/// `terraflux plot`: the model run (its frozen nodes used, `set` and `scenario` as in `run`) and its Plot `id` written
/// to `out` as the Viewer draws it; what was written and the layers drawn.
#[allow(clippy::too_many_arguments)]
fn plot(
    model: &Path,
    id: &str,
    out: &Path,
    scale: u64,
    format: Option<&str>,
    set: &[String],
    scenario: Option<&str>,
    quiet: bool,
    engine: &Engine,
) -> Result<String> {
    let scale = terraflux_app::plot_export::check_scale(scale).map_err(anyhow::Error::msg)?;
    let format = terraflux_app::plot_export::format_for(out, format).map_err(anyhow::Error::msg)?;
    let mut m = read_model(model)?;
    // (before the run, which can take long)
    terraflux_app::plot_export::find(&m, id).map_err(anyhow::Error::msg)?;
    if let Some(name) = scenario {
        m = m.with_scenario(name)?;
    }
    let overrides = apply_set(&mut m, set, engine)?;
    let dir = folder_of(model);
    let t = Instant::now();
    let inputs = load_inputs(&m, &dir, &overrides)?;
    let mut progress = Progress { quiet, start: t };
    let results = run_frozen(engine, model, &m, &inputs, &mut progress, quiet)?;
    let value = |from: &str| terraflux_app::plot_export::lookup(&inputs, &results, from);
    let made = terraflux_app::plot_export::panel_of(&m, id, &value).map_err(anyhow::Error::msg)?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    terraflux_app::plot_export::write(&made, scale, format, out).map_err(anyhow::Error::msg)
}

/// A run with the nodes frozen in the application: their results kept next to the model (`<model>.frozen/`), used
/// instead of running them (a node whose result cannot be read runs, with a note).
fn run_frozen(
    engine: &Engine,
    model_file: &Path,
    m: &Model,
    inputs: &BTreeMap<String, terraflux_ops::Value>,
    progress: &mut Progress,
    quiet: bool,
) -> Result<terraflux_exec::Results> {
    let frozen = terraflux_exec::frozen_places(m);
    if frozen.is_empty() {
        return Ok(engine.run(m, inputs, progress)?);
    }
    let folder = terraflux_exec::frozen::folder(model_file);
    let mut held = terraflux_exec::Cache::default();
    // (a node of a group's body by its place, group/node)
    for at in &frozen {
        match terraflux_exec::frozen::read(&folder, at) {
            Ok((outputs, when)) => {
                held.hold(at, Some(outputs), when);
                if !quiet {
                    eprintln!("{at}: its result frozen ({})", terraflux_exec::frozen::dir(&folder, at).display());
                }
            }
            Err(e) => eprintln!("  note: {at}: frozen, its result not read ({e}): it runs"),
        }
    }
    Ok(engine.run_with(m, inputs, progress, Some(&std::sync::Mutex::new(held)), None)?)
}

/// The folder a model's relative paths are read from: "." for a model named without one (`terraflux run m.toml`),
/// whose parent is the empty path.
/// The check run asked for (none: a normal run). The window `xmin,ymin,xmax,ymax` is in `crs` (`EPSG:4326`, a code,
/// a WKT), else in the coordinate system of the model's first raster (the files' own coordinates when none has one).
fn check_of(
    m: &Model,
    dir: &Path,
    coarsen: u32,
    window: Option<&str>,
    crs: Option<&str>,
) -> Result<Option<terraflux_ops::check::Check>> {
    if coarsen == 0 {
        bail!("--coarsen: 1 or more (one cell in every N, on each side)");
    }
    let window = match window {
        None => None,
        Some(w) => {
            let v: Vec<f64> = w
                .split(',')
                .map(|s| s.trim().parse::<f64>())
                .collect::<std::result::Result<_, _>>()
                .with_context(|| format!("--window {w}: give xmin,ymin,xmax,ymax"))?;
            let [x0, y0, x1, y1] = v[..] else { bail!("--window {w}: give xmin,ymin,xmax,ymax") };
            if !(x0 < x1 && y0 < y1) {
                bail!("--window {w}: the minimum must be less than the maximum");
            }
            Some([x0, y0, x1, y1])
        }
    };
    if window.is_none() && coarsen == 1 {
        return Ok(None);
    }
    let crs = match (window, crs) {
        (None, _) => None,
        (Some(_), Some(c)) if c.chars().all(|x| x.is_ascii_digit()) => Some(format!("EPSG:{c}")),
        (Some(_), Some(c)) => Some(c.to_string()),
        (Some(_), None) => first_crs(m, dir),
    };
    Ok(Some(terraflux_ops::check::Check::new(window.map(|w| (w, crs)), coarsen)))
}

/// The coordinate system of the first raster a model reads (an input of a map, then a `raster.read`), if one has it.
fn first_crs(m: &Model, dir: &Path) -> Option<String> {
    let inputs = m.inputs.values().filter_map(|i| match i {
        Input::Map { path, .. } | Input::Stack { path, .. } => Some(path.clone()),
        _ => None,
    });
    let nodes = m
        .nodes
        .iter()
        .filter(|n| n.op == "raster.read")
        .filter_map(|n| n.params.get("path").and_then(|p| p.as_str()).map(String::from));
    for p in inputs.chain(nodes) {
        let full = if Path::new(&p).is_absolute() { PathBuf::from(&p) } else { dir.join(&p) };
        if let Ok(r) = terraflux_ops::files::Rasters::open(&full.to_string_lossy(), None, None)
            && let Some(c) = terraflux_ops::prepare::crs_of(&r.grid)
        {
            return Some(c);
        }
    }
    None
}

/// `terraflux python`: each model as a landschaft script, printed, written to `out` or beside the model; the
/// summary of each to the error output. `strict`: an error when a node was left as a comment, after all are done.
fn python(
    models: &[PathBuf],
    out: Option<&Path>,
    beside: bool,
    strict: bool,
    r: &terraflux_ops::Registry,
) -> Result<()> {
    use terraflux_app::python;
    if models.len() > 1 && !beside {
        anyhow::bail!("several models: use --beside to write each one's script beside it");
    }
    if out.is_some() && beside {
        anyhow::bail!("--out and --beside do not go together");
    }
    let mut left = Vec::new();
    for model in models {
        let m = Model::load(model).with_context(|| format!("reading {}", model.display()))?;
        let prefix = if models.len() > 1 { format!("{}: ", model.display()) } else { String::new() };
        let s = if beside {
            let (p, s) = python::write_beside(&m, r, model).map_err(|e| anyhow::anyhow!(e))?;
            eprintln!("wrote {}", p.display());
            s
        } else {
            match out {
                Some(p) => {
                    let s = python::write_to(&m, r, model, p).map_err(|e| anyhow::anyhow!(e))?;
                    eprintln!("wrote {}", p.display());
                    s
                }
                None => {
                    let s = python::script(&m, r, Some(model));
                    print!("{}", s.text);
                    s
                }
            }
        };
        eprintln!("{prefix}{}", python::summary(&s));
        if !s.left.is_empty() {
            left.push(model.display().to_string());
        }
    }
    if strict && !left.is_empty() {
        anyhow::bail!("nodes left as comments in: {}", left.join(", "));
    }
    Ok(())
}

fn folder_of(model: &Path) -> PathBuf {
    match model.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
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
/// A run recorded in the model's journal (unless asked not to); a journal that cannot be written is said, not fatal.
fn journal(off: bool, quiet: bool, model: &Path, ran: &Model, entry: Entry) {
    if off {
        return;
    }
    if let Err(e) = terraflux_exec::journal::record_run(model, ran, entry)
        && !quiet
    {
        eprintln!("note: the run was not recorded in the journal: {e}");
    }
}

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

/// `terraflux sensitivity`, out of `main`: its future is large (the debug build keeps every command's locals in
/// `main`'s frame, and overflowed the main thread's stack).
fn sensitivity_command(a: &cluster::Sensitivity) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(cluster::sensitivity(a))
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
        Command::Sensitivity(a) => sensitivity_command(&a)?,
        Command::App { model } => app(model)?,
        command @ Command::Run { .. } => run_command(command, engine)?,
        Command::Reproduce { manifest, out, quiet, thaw } => reproduce(&manifest, out, quiet, thaw, &engine)?,
        Command::Test { models, update, quiet, thaw } => test(&models, update, quiet, thaw, &engine)?,
        Command::Report { model, out, run, language } => {
            let lang = terraflux_app::methods::Lang::parse(&language)
                .ok_or_else(|| anyhow::anyhow!("no language {language}: en or pt"))?;
            let source = Model::load(&model).with_context(|| format!("reading {}", model.display()))?;
            let dir = folder_of(&model);
            let expanded = Model::load_expanded(&model)?;
            let mut loaded = None;
            let results = if run {
                let inputs = load_inputs(&expanded, &dir, &BTreeMap::new())?;
                let mut progress = Progress { quiet: false, start: Instant::now() };
                let results = engine.run(&expanded, &inputs, &mut progress)?;
                loaded = Some(inputs);
                Some(results)
            } else {
                None
            };
            let doc = terraflux_app::document::Document::of_in(&source, &engine.registry, lang);
            let values = |n: &str, p: &str| results.as_ref()?.values.get(n)?.get(p).cloned();
            let first = |n: &str| terraflux_app::document::only_output(&source, &engine.registry, n);
            let out = out.unwrap_or_else(|| model.with_extension("html"));
            // the methods and the papers to cite, how the checks fared in the run
            let ran = |id: &str| results.as_ref().is_some_and(|r| r.values.contains_key(id));
            let warnings = results.as_ref().map(|r| r.warnings.clone()).unwrap_or_default();
            let checks = terraflux_app::checks::of_run(&source, &values, &ran, None, &warnings);
            let file = model.file_name().map(|f| f.to_string_lossy().into_owned());
            // the advice on the method, with the inputs when the model ran
            let advice = terraflux_app::advice::advise(&expanded, &engine.registry, &dir, loaded.as_ref());
            let extras = terraflux_app::report::Extras::of_in(&source, &engine.registry, file.as_deref(), checks, lang)
                .with_advice(advice);
            terraflux_app::report::write(&out, &doc, &values, &first, &extras)
                .with_context(|| format!("writing {}", out.display()))?;
            eprintln!("wrote {}", out.display());
        }
        Command::Plot { model, id, out, scale, format, set, scenario, quiet } => {
            let said = plot(&model, &id, &out, scale, format.as_deref(), &set, scenario.as_deref(), quiet, &engine)?;
            eprintln!("{said}");
        }
        Command::Methods { model, language, format, out } => {
            let source = Model::load(&model).with_context(|| format!("reading {}", model.display()))?;
            let file = model.file_name().map(|f| f.to_string_lossy().into_owned());
            let args = serde_json::json!({ "language": language, "format": format });
            let text = terraflux_app::tools::methods(&source, &engine.registry, &args, file.as_deref())
                .map_err(|e| anyhow::anyhow!(e))?;
            match out {
                Some(p) => {
                    std::fs::write(&p, &text).with_context(|| format!("writing {}", p.display()))?;
                    eprintln!("wrote {}", p.display());
                }
                None => print!("{text}"),
            }
        }
        Command::Python { model, out, beside, strict } => {
            python(&model, out.as_deref(), beside, strict, &engine.registry)?
        }
        Command::Check { model } => {
            let m = read_model(&model)?;
            let mut ok = report(&m, &engine);
            let dir = folder_of(&model);
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
        Command::Front { uri } => {
            hide_own_console();
            if let Some(pid) = terraflux_app::notify::front_of(&uri) {
                // (that Terraflux closed since: nothing to do)
                let _ = terraflux_app::handoff::front(pid);
            }
        }
        Command::Relink { model, folders, dry_run } => {
            if let Some(gone) = folders.iter().find(|f| !f.is_dir()) {
                bail!("--in {}: no such folder", gone.display());
            }
            let mut m = Model::load(&model)?;
            let dir = folder_of(&model);
            let done = terraflux_app::relink::relink_model(&mut m, &dir, &folders);
            println!("{}", done.words(&dir));
            if done.changed > 0 && !dry_run {
                m.save(&model)?;
                eprintln!("wrote {}", model.display());
            }
            if !done.still.is_empty() {
                std::process::exit(1);
            }
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

/// `terraflux run`: kept out of `main` so that its frame stays small (the debug build on Windows overflowed the
/// main thread's stack when every command's locals were in one frame).
#[inline(never)]
fn run_command(command: Command, engine: Engine) -> Result<()> {
    let Command::Run {
        model,
        out,
        set,
        scenario,
        quiet,
        profile,
        no_journal,
        coarsen,
        window,
        window_crs,
        tiled,
        tile,
        radius,
        disk,
        coordinator,
        code,
        shared_disk,
    } = command
    else {
        unreachable!("only a run comes here")
    };
    let mut m = read_model(&model)?;
    if let Some(name) = &scenario {
        m = m.with_scenario(name)?;
    }
    let dir = folder_of(&model);
    // a check run: the rasters cut and coarser, its files apart from the real ones
    let check = check_of(&m, &dir, coarsen, window.as_deref(), window_crs.as_deref())?
        .map(|c| c.writing_to(out.clone().unwrap_or_else(|| terraflux_exec::journal::check_dir(&model))));
    if check.is_some() && tiled {
        bail!("--tiled cannot be a check run (--coarsen, --window): it already reads in tiles");
    }
    let engine = match &check {
        Some(c) => Engine::checked(c.clone()),
        None => engine,
    };
    let kind = || check.clone().map_or(Kind::Run, Kind::Check);
    // (the folder of the check run, when not chosen, replaced: whatever the run before left in it goes)
    if let (Some(c), None) = (&check, &out)
        && let Some(o) = &c.out
    {
        let _ = std::fs::remove_dir_all(o);
    }
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
    let inputs = terraflux_exec::load_inputs_checked(&m, &dir, &overrides, check.as_ref())?;
    if !quiet {
        eprintln!("{}: inputs read ({:.1} s)", m.name.as_deref().unwrap_or("model"), t.elapsed().as_secs_f64());
        if let Some(c) = &check {
            eprintln!(
                "check run, {}: the results are of the part, not the model's (landscape metrics, distances and \
                 neighbourhoods change with the resolution and the border)",
                c.label()
            );
        }
    }
    let mut progress = Progress { quiet, start: t };
    // (a check run does not take the frozen results: their files are of the whole map)
    let ran = match &check {
        Some(_) => engine.run(&m, &inputs, &mut progress).map_err(anyhow::Error::from),
        None => run_frozen(&engine, &model, &m, &inputs, &mut progress, quiet),
    };
    let results = match ran {
        Ok(r) => r,
        Err(e) => {
            let mut entry = Entry::new(kind(), "cli", t.elapsed().as_secs_f64());
            entry.status = Status::Failed(e.to_string());
            entry.scenario = scenario.clone();
            journal(no_journal, quiet, &model, &m, entry);
            return Err(e);
        }
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
    let out = out.unwrap_or_else(|| check.as_ref().and_then(|c| c.out.clone()).unwrap_or_else(|| dir.join("output")));
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
    let manifest_file = out.join("manifest.json");
    let mut entry = Entry::new(kind(), "cli", t.elapsed().as_secs_f64()).with_manifest(&manifest, &manifest_file);
    entry.scenario = scenario.clone();
    manifest.save(&manifest_file)?;
    journal(no_journal, quiet, &model, &m, entry);
    if !quiet {
        for f in files {
            eprintln!("wrote {}", f.display());
        }
        eprintln!("wrote {} (terraflux reproduce runs it again)", out.join("manifest.json").display());
        eprintln!("done in {:.1} s", t.elapsed().as_secs_f64());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--coarsen` and `--window` make the check run (its window in the CRS given, else the first raster's); none of
    /// them is a normal run; a window that makes no sense is refused.
    #[test]
    fn the_check_run_is_asked_for_by_coarsen_and_window() {
        let dir = std::env::temp_dir().join(format!("terraflux-cli-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut grid = terraflux_raster::Grid::new(4, 4);
        grid.geo.wkt = None;
        terraflux_raster::Raster::single(grid, vec![1.0; 16], terraflux_raster::DataType::U8, None)
            .write(dir.join("a.tif"))
            .unwrap();
        let m = Model::from_toml(
            "format = \"terraflux/1\"
[[nodes]]
id = \"a\"
op = \"raster.read\"
params = { path = \"a.tif\" }
",
        )
        .unwrap();
        assert!(check_of(&m, &dir, 1, None, None).unwrap().is_none());
        let c = check_of(&m, &dir, 4, None, None).unwrap().unwrap();
        assert_eq!((c.coarsen, c.window.is_none()), (4, true));
        // no CRS in the model's rasters: the window is in the files' own coordinates
        let c = check_of(&m, &dir, 1, Some("1, 2,3.5,4"), None).unwrap().unwrap();
        assert_eq!((c.window, c.crs.clone()), (Some([1.0, 2.0, 3.5, 4.0]), None));
        // a code alone is an EPSG code
        let c = check_of(&m, &dir, 2, Some("-70,-10,-60,0"), Some("4326")).unwrap().unwrap();
        assert_eq!((c.window, c.crs.as_deref(), c.coarsen), (Some([-70.0, -10.0, -60.0, 0.0]), Some("EPSG:4326"), 2));
        for bad in ["1,2,3", "a,b,c,d", "3,0,1,4", "0,4,1,4"] {
            assert!(check_of(&m, &dir, 1, Some(bad), None).is_err(), "{bad}");
        }
        assert!(check_of(&m, &dir, 0, None, None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `terraflux python --beside` writes `<model>.py` beside each model, portable; `--strict` fails on nodes left.
    #[test]
    fn python_is_written_beside_the_models() {
        let dir = std::env::temp_dir().join(format!("terraflux-cli-python-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let r = terraflux_ops::Registry::standard();
        let name = terraflux_app::templates::TEMPLATES[0].name;
        let m = terraflux_app::tools::template_model(&r, name).unwrap();
        let (a, b) = (dir.join("a.toml"), dir.join("b.json"));
        m.save(&a).unwrap();
        m.save(&b).unwrap();
        python(&[a.clone(), b.clone()], None, true, false, &r).unwrap();
        let py = std::fs::read_to_string(dir.join("a.py")).unwrap();
        assert!(py.contains("import landschaft as ls") && py.contains("Path(__file__).parent"), "{py}");
        assert!(dir.join("b.py").exists());
        // several models without --beside, and --out with --beside: refused
        assert!(python(&[a.clone(), b.clone()], None, false, false, &r).is_err());
        assert!(python(std::slice::from_ref(&a), Some(&dir.join("x.py")), true, false, &r).is_err());
        // --out: one file
        python(std::slice::from_ref(&a), Some(&dir.join("x.py")), false, false, &r).unwrap();
        assert!(dir.join("x.py").exists());
        // --strict: fails only when a node was left as a comment
        let s = terraflux_app::python::script(&m, &r, None);
        assert_eq!(
            python(std::slice::from_ref(&a), Some(&dir.join("y.py")), false, true, &r).is_err(),
            !s.left.is_empty()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A run is recorded in the model's journal, unless asked not to.
    #[test]
    fn a_run_is_recorded_in_the_journal_unless_asked_not_to() {
        let dir = std::env::temp_dir().join(format!("terraflux-cli-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("m.toml");
        let m = Model::from_toml("format = \"terraflux/1\"\n[inputs.rate]\nkind = \"value\"\nvalue = 2\n").unwrap();
        m.save(&file).unwrap();
        let j = terraflux_exec::journal::Journal::of(&file).unwrap();
        journal(true, true, &file, &m, Entry::new(Kind::Run, "cli", 0.5));
        assert!(j.entries().is_empty());
        let mut e = Entry::new(Kind::Run, "cli", 0.5);
        e.scenario = Some("dry".into());
        journal(false, true, &file, &m, e);
        let all = j.entries();
        assert_eq!(all.len(), 1);
        assert_eq!((all[0].from.as_str(), all[0].scenario.as_deref()), ("cli", Some("dry")));
        assert_eq!(all[0].values["$rate"], serde_json::json!(2));
        assert_eq!(j.model(&all[0].model).unwrap().inputs.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

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
        reproduce(&manifest, None, true, false, &engine).unwrap();
        // tested: the files written, then checked; another seed fails
        test(std::slice::from_ref(&model), true, true, false, &engine).unwrap();
        test(std::slice::from_ref(&dir), false, true, false, &engine).unwrap();
        std::fs::write(&model, text(4)).unwrap();
        let e = test(std::slice::from_ref(&model), false, true, false, &engine).unwrap_err().to_string();
        assert!(e.contains("1 of 1 models failed"), "{e}");
        // and the run of another seed does not reproduce the manifest's
        assert!(reproduce(&manifest, None, true, false, &engine).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A study's folder moved with its manifest is reproduced from it: the model, the files its nodes read and wrote
    /// found from the manifest's folder, the frozen result used as the run used it (its node run again with --thaw).
    #[test]
    fn a_moved_study_with_a_frozen_node_is_reproduced() {
        let root = std::env::temp_dir().join(format!("terraflux-moved-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("a");
        std::fs::create_dir_all(&dir).unwrap();
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
        std::fs::copy(data, dir.join("lu.tif")).unwrap();
        let model = dir.join("m.toml");
        let text = |seed: u32, frozen: bool| {
            format!(
                "format = \"terraflux/1\"\n\n[[nodes]]\nid = \"lu\"\nop = \"raster.read\"\nparams = {{ path = \"lu.tif\" }}\n\n\
                 [[nodes]]\nid = \"land\"\nop = \"nlm.random\"\nfrozen = {frozen}\nparams = {{ rows = 30, columns = 20, seed = {seed} }}\n\n\
                 [[nodes]]\nid = \"forest\"\nop = \"raster.calc\"\ninputs = {{ x = \"lu.raster\" }}\nparams = {{ expression = \"x._2011 == 1\" }}\n\n\
                 [[nodes]]\nid = \"save\"\nop = \"raster.write\"\ninputs = {{ value = \"forest.map\" }}\nparams = {{ path = \"forest.tif\" }}\n\n\
                 [outputs.land]\nfrom = \"land.map\"\n"
            )
        };
        let engine = Engine::default();
        let start = || Progress { quiet: true, start: Instant::now() };
        // the result of seed 9 frozen; the model then says seed 3, its node frozen
        std::fs::write(&model, text(9, false)).unwrap();
        let m = Model::load_expanded(&model).unwrap();
        let results = engine.run(&m, &load_inputs(&m, &dir, &BTreeMap::new()).unwrap(), &mut start()).unwrap();
        terraflux_exec::frozen::freeze(&terraflux_exec::frozen::folder(&model), "land", &results.values["land"])
            .unwrap();
        std::fs::write(&model, text(3, true)).unwrap();
        let m = Model::load_expanded(&model).unwrap();
        let inputs = load_inputs(&m, &dir, &BTreeMap::new()).unwrap();
        let results = run_frozen(&engine, &model, &m, &inputs, &mut start(), true).unwrap();
        let out = dir.join("output");
        let files = write_outputs(&m, &results, &out).unwrap();
        let written: Vec<(String, PathBuf)> = m.outputs.keys().cloned().zip(files).collect();
        let mf = terraflux_exec::testing::manifest(&model, &m, &dir, &BTreeMap::new(), None, &[], &written).unwrap();
        assert!(mf.reads.contains_key("lu.path"), "{:?}", mf.reads.keys());
        assert!(mf.frozen.contains_key("land"));
        assert!(mf.writes.contains_key("save.path"), "{:?}", mf.writes.keys());
        mf.save(&out.join("manifest.json")).unwrap();
        // the folder moved (copied, its model taken out of the old one: the run holds its rasters open): found from
        // the manifest's
        fn copy(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for e in std::fs::read_dir(from).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    copy(&p, &to.join(e.file_name()));
                } else {
                    std::fs::copy(&p, to.join(e.file_name())).unwrap();
                }
            }
        }
        let moved = root.join("b");
        copy(&dir, &moved);
        std::fs::remove_file(&model).unwrap();
        let manifest = moved.join("output").join("manifest.json");
        reproduce(&manifest, None, true, false, &engine).unwrap();
        // the frozen node run again gives seed 3's, not the frozen result
        let e = reproduce(&manifest, None, true, true, &engine).unwrap_err().to_string();
        assert!(e.contains("1 of 2 files differ"), "{e}");
        // the model gone: said where it was looked for
        std::fs::rename(moved.join("m.toml"), moved.join("other.toml")).unwrap();
        let e = reproduce(&manifest, None, true, false, &engine).unwrap_err().to_string();
        assert!(e.contains("not found") && e.contains("m.toml"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A Plot of a model written as a PNG and as a GeoTIFF by its id, with a scenario's and a --set's values.
    #[test]
    fn a_plot_is_written_by_its_id() {
        let dir = std::env::temp_dir().join(format!("terraflux-plot-cmd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let model = dir.join("m.toml");
        std::fs::write(
            &model,
            "format = \"terraflux/1\"\n\n[inputs.cut]\nkind = \"value\"\nvalue = 0.5\n\n[[nodes]]\nid = \"land\"\n\
             op = \"nlm.random\"\nparams = { rows = 10, columns = 14, seed = 2 }\n\n[[nodes]]\nid = \"high\"\n\
             op = \"raster.calc\"\ninputs = { x = \"land.map\", k = \"$cut\" }\nparams = { expression = \"x > k\" }\n\n\
             [[plots]]\nid = \"cover\"\n\n[[plots.layers]]\nfrom = \"land.map\"\npalette = \"grey\"\n\n\
             [[plots.layers]]\nfrom = \"high.map\"\nopacity = 0.5\n",
        )
        .unwrap();
        let engine = Engine::default();
        let png = dir.join("out").join("cover.png");
        let said = plot(&model, "cover", &png, 2, None, &["cut=0.8".into()], None, true, &engine).unwrap();
        assert!(said.contains("28 x 20 px") && said.contains("high.map at 50%"), "{said}");
        // (the size is in the PNG's header)
        let head = std::fs::read(&png).unwrap();
        let side = |at: usize| u32::from_be_bytes(head[at..at + 4].try_into().unwrap());
        assert_eq!((side(16), side(20)), (28, 20));
        let tif = dir.join("cover.tif");
        plot(&model, "cover", &tif, 1, None, &[], None, true, &engine).unwrap();
        let r = terraflux_raster::tiff::read(&tif).unwrap();
        assert_eq!((r.bands.len(), r.grid.width, r.grid.height), (4, 14, 10));
        // refused before the run: a plot the model has not, a scale, a file that is no picture
        let e = plot(&model, "nope", &png, 1, None, &[], None, true, &engine).unwrap_err().to_string();
        assert!(e.contains("its plots: cover"), "{e}");
        assert!(plot(&model, "cover", &png, 3, None, &[], None, true, &engine).is_err());
        assert!(plot(&model, "cover", &dir.join("c.jpg"), 1, None, &[], None, true, &engine).is_err());
        assert!(plot(&model, "cover", &dir.join("c.jpg"), 1, Some("png"), &[], None, true, &engine).is_ok());
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
