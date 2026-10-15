//! `terraflux`: run, check and create land change models. Without a command (a double click), or with only a
//! model file (dropped on it, "open with"), it opens the application.

mod cluster;

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
        /// Print only errors.
        #[arg(short, long)]
        quiet: bool,
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
        Command::App { model } => app(model)?,
        Command::Run { model, out, set, quiet, tiled, tile, radius, disk, coordinator, code, shared_disk } => {
            let mut m = Model::load_expanded(&model).with_context(|| format!("reading {}", model.display()))?;
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
            // `name=value` replaces an input; `node.setting=value` a node's setting (as a scenario's)
            let mut overrides = BTreeMap::new();
            let mut settings = BTreeMap::new();
            for s in &set {
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
            let t = Instant::now();
            let inputs = load_inputs(&m, &dir, &overrides)?;
            if !quiet {
                eprintln!("{}: inputs read ({:.1} s)", m.name.as_deref().unwrap_or("model"), t.elapsed().as_secs_f64());
            }
            let mut progress = Progress { quiet, start: t };
            let results = engine.run(&m, &inputs, &mut progress)?;
            let out = out.unwrap_or_else(|| dir.join("output"));
            let files = write_outputs(&m, &results, &out)?;
            if !quiet {
                for f in files {
                    eprintln!("wrote {}", f.display());
                }
                eprintln!("done in {:.1} s", t.elapsed().as_secs_f64());
            }
        }
        Command::Check { model } => {
            let m = Model::load_expanded(&model)?;
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
