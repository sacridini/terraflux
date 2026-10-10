//! `terraflux`: run, check and create land change models.

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
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a model and write its outputs.
    Run {
        /// The model (.toml or .json).
        model: PathBuf,
        /// The folder of the outputs (default: `output` next to the model).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Replace an input: `name=path` for a file, `name=value` (JSON) for a value; e.g. `--set seed=7`.
        #[arg(long = "set", value_name = "NAME=VALUE")]
        set: Vec<String>,
        /// Print only errors.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Check a model without running it.
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

/// `path` relative to `dir` when it is inside it (or next to it), else absolute; with forward slashes.
fn relative_to(path: &Path, dir: &Path) -> String {
    let p: Vec<_> = path.components().collect();
    let d: Vec<_> = dir.components().collect();
    let same = |a: &std::path::Component, b: &std::path::Component| {
        if cfg!(windows) {
            a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase()
        } else {
            a == b
        }
    };
    let common = p.iter().zip(&d).take_while(|(a, b)| same(a, b)).count();
    let s = if common == 0 {
        path.to_string_lossy().into_owned()
    } else {
        let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), d.len() - common).collect();
        parts.extend(p[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
        parts.join("/")
    };
    s.replace('\\', "/")
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
    let start = a.start.clone().unwrap_or_else(|| format!("{}:0", a.observed));
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = Engine::default();
    match cli.command {
        Command::Run { model, out, set, quiet } => {
            let m = Model::load(&model).with_context(|| format!("reading {}", model.display()))?;
            let dir = model.parent().map(Path::to_path_buf).unwrap_or_default();
            let mut overrides = BTreeMap::new();
            for s in &set {
                let (k, v) = s.split_once('=').with_context(|| format!("--set {s}: give name=value"))?;
                overrides.insert(k.to_string(), v.to_string());
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
            let m = Model::load(&model)?;
            if report(&m, &engine) {
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
