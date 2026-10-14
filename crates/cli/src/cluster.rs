//! The commands of many runs: a coordinator, workers, batches of replicates or scenarios, calibrations.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Args;
use terraflux_dist::batch::{self, Param};
use terraflux_dist::client::{Client, TaskOf};
use terraflux_dist::coordinator::{self, Options};
use terraflux_dist::evaluate::Evaluation;
use terraflux_dist::run::{self, Batch, Place, Progress};
use terraflux_graph::Model;

/// Where a batch runs.
#[derive(Args, Clone)]
pub struct Where {
    /// The coordinator (`http://host:7700`); without one, a coordinator and `--workers` workers run here.
    #[arg(long)]
    coordinator: Option<String>,
    /// Workers on this machine when no coordinator is given (default: a quarter of the cores, at least 1).
    #[arg(long)]
    workers: Option<usize>,
}

pub fn default_cache() -> PathBuf {
    std::env::temp_dir().join("terraflux-cache")
}

/// Where a batch runs: the coordinator given, or workers started here.
fn place(w: &Where) -> Place {
    match &w.coordinator {
        Some(u) => Place::At(u.clone()),
        None => {
            let most = terraflux_ops::threads::most();
            let n = w.workers.unwrap_or((most / 4).max(1)).clamp(1, most);
            Place::Here { workers: n, cache: default_cache() }
        }
    }
}

fn progress(p: Progress) {
    match p {
        Progress::Workers(n) => eprintln!("{n} workers on this machine"),
        Progress::Sending => {}
        Progress::Submitted { job, total } => eprintln!("job {job}: {total} tasks"),
        Progress::Status(s) => {
            eprintln!("  {} of {} done, {} running, {} failed", s.done, s.total, s.running, s.failed)
        }
    }
}

pub async fn serve(listen: SocketAddr, timeout: u64) -> Result<()> {
    let (addr, _service, handle) =
        coordinator::serve(listen, Options { timeout: Duration::from_secs(timeout), attempts: 3 }).await?;
    eprintln!(
        "coordinator listening on {addr}: start workers with `terraflux worker --coordinator http://<this host>:{}`",
        addr.port()
    );
    tokio::select! {
        _ = handle => {}
        _ = tokio::signal::ctrl_c() => eprintln!("stopped"),
    }
    Ok(())
}

pub async fn worker(url: &str, name: Option<String>, cache: Option<PathBuf>) -> Result<()> {
    let name = name.unwrap_or_else(|| {
        std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "worker".into())
    });
    let cache = cache.unwrap_or_else(default_cache);
    eprintln!("worker {name}: tasks from {url}, data kept in {}", cache.display());
    let stop = Arc::new(AtomicBool::new(false));
    terraflux_dist::worker::run(url, terraflux_dist::worker::Options::new(&name, cache), stop).await?;
    Ok(())
}

pub async fn workers(url: &str) -> Result<()> {
    let mut c = Client::connect(url).await?;
    for w in c.workers().await? {
        println!(
            "{}\t{}\t{} threads\t{} tasks done\t{}\tseen {:.0} s ago",
            w.id,
            w.name,
            w.threads,
            w.done,
            if w.task.is_empty() { "idle".to_string() } else { format!("running {}", w.task) },
            w.seen_seconds_ago
        );
    }
    Ok(())
}

/// Run a batch of tasks of a model and write each task's files in `out/<label>/`.
pub async fn submit(model_path: &Path, tasks: Vec<TaskOf>, out: &Path, w: &Where) -> Result<()> {
    let model = Model::load_expanded(model_path).with_context(|| format!("reading {}", model_path.display()))?;
    let base = model_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = model.name.clone().unwrap_or_else(|| "batch".into());
    let b = Batch { name, model, base, tasks, files: true, evaluation: None };
    let o = run::run(&b, &place(w), &AtomicBool::new(false), &mut progress).await?;
    run::write_results(out, &o.results)?;
    eprintln!("wrote {}", out.display());
    if o.status.failed > 0 {
        bail!(
            "{} tasks failed:
  {}",
            o.status.failed,
            o.status.errors.join(
                "
  "
            )
        );
    }
    Ok(())
}

#[derive(Args)]
pub struct Calibrate {
    /// The model (.toml or .json).
    pub model: PathBuf,
    /// A setting to vary: `node.setting=from:to` (or `from:to:points`), e.g. `patches.isometry=0.5:2.5`.
    #[arg(long = "param", required = true)]
    pub params: Vec<String>,
    /// `grid` (every combination) or `random` (a Latin hypercube of `--n` draws).
    #[arg(long, default_value = "grid")]
    pub method: String,
    /// Points of each grid range, or draws of a random search.
    #[arg(long, default_value_t = 5)]
    pub n: usize,
    /// The seeds each candidate runs with (its score is the mean).
    #[arg(long, value_delimiter = ',', default_value = "1")]
    pub seeds: Vec<i64>,
    /// The observed map at the end of the simulated period: `file.tif` or `file.tif:band`.
    #[arg(long)]
    pub observed: String,
    /// The input the simulation starts from.
    #[arg(long, default_value = "t0")]
    pub start: String,
    /// What to maximise: `fom` (figure of merit), `overall`, or `similarity:<window>`.
    #[arg(long, default_value = "fom")]
    pub metric: String,
    /// What holds the seed: an input (`seed`), a node's (`simulation.seed`), or several, separated by commas.
    #[arg(long, default_value = "seed")]
    pub seed_input: String,
    #[arg(short, long, default_value = "calibration")]
    pub out: PathBuf,
    #[command(flatten)]
    pub at: Where,
}

pub async fn calibrate(a: &Calibrate) -> Result<()> {
    let model = Model::load_expanded(&a.model).with_context(|| format!("reading {}", a.model.display()))?;
    let base = a.model.parent().map(Path::to_path_buf).unwrap_or_default();
    let params: Vec<Param> = a.params.iter().map(|p| Param::parse(p, a.n)).collect::<terraflux_dist::Result<_>>()?;
    let candidates = match a.method.as_str() {
        "grid" => batch::grid(&params),
        "random" => batch::random(&params, a.n, 1),
        m => bail!("unknown method {m:?} (grid or random)"),
    };
    // every candidate must apply to the model
    for c in &candidates {
        model.clone().apply(c)?;
    }
    let (file, band) = match a.observed.rsplit_once(':') {
        Some((f, b)) if !b.contains(['/', '\\']) && f.len() > 1 => (f.to_string(), Some(b.to_string())),
        _ => (a.observed.clone(), None),
    };
    let eval = Evaluation {
        output: "simulation.maps".into(),
        observed: std::path::absolute(&file)?.to_string_lossy().into_owned(),
        band,
        start: a.start.clone(),
        windows: vec![1, 3, 5, 7, 9, 11],
    };
    let tasks = batch::calibration_tasks(&candidates, &a.seeds, &batch::seed_keys(&a.seed_input));
    eprintln!("calibration: {} candidates x {} seeds", candidates.len(), a.seeds.len());
    let labels: Vec<String> = tasks.iter().map(|(l, _)| l.clone()).collect();
    let b =
        Batch { name: "calibration".into(), model: model.clone(), base, tasks, files: false, evaluation: Some(eval) };
    let o = run::run(&b, &place(&a.at), &AtomicBool::new(false), &mut progress).await?;
    let status = o.status;
    let metrics = batch::metrics_of(&labels, &o.results);
    let ranked = batch::rank(&candidates, a.seeds.len(), &metrics, &a.metric)?;
    std::fs::create_dir_all(&a.out)?;
    let keys: Vec<String> = params.iter().map(|p| p.key.clone()).collect();
    std::fs::write(a.out.join("calibration.csv"), batch::ranking_csv(&ranked, &keys, &a.metric))?;
    let best = &ranked[0];
    std::fs::write(a.out.join("best.json"), serde_json::to_string_pretty(&best.settings)?)?;
    let mut calibrated = model.clone();
    calibrated.apply(&best.settings)?;
    calibrated.description = Some(format!(
        "{} Calibrated: {} = {:.6} (mean over seeds {:?}).",
        model.description.clone().unwrap_or_default(),
        a.metric,
        best.score,
        a.seeds
    ));
    let ext = a.model.extension().and_then(|e| e.to_str()).unwrap_or("toml");
    let stem = a.model.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
    let out_model = a.model.with_file_name(format!("{stem}_calibrated.{ext}"));
    calibrated.save(&out_model)?;
    eprintln!("best: {} ({} = {:.6})", serde_json::to_string(&best.settings)?, a.metric, best.score);
    eprintln!("wrote {} and {}", a.out.join("calibration.csv").display(), out_model.display());
    if status.failed > 0 {
        eprintln!("{} runs failed:\n  {}", status.failed, status.errors.join("\n  "));
    }
    Ok(())
}
