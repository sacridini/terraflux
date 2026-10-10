//! The commands of many runs: a coordinator, workers, batches of replicates or scenarios, calibrations.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Args;
use terraflux_dist::Local;
use terraflux_dist::batch::{self, Param};
use terraflux_dist::client::{Client, TaskOf};
use terraflux_dist::coordinator::{self, Options};
use terraflux_dist::evaluate::{Evaluation, Metrics};
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

/// The coordinator's URL: the one given, or a local cluster started here.
async fn place(w: &Where) -> Result<(String, Option<Local>)> {
    match &w.coordinator {
        Some(u) => Ok((u.clone(), None)),
        None => {
            let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
            let n = w.workers.unwrap_or((cores / 4).max(1));
            let local = Local::start(n, default_cache(), Options::default()).await?;
            eprintln!("{n} workers on this machine");
            Ok((local.url.clone(), Some(local)))
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

fn progress(s: &terraflux_dist::proto::JobStatus) {
    eprintln!("  {} of {} done, {} running, {} failed", s.done, s.total, s.running, s.failed);
}

/// Run a batch of tasks of a model and write each task's files in `out/<label>/`.
pub async fn submit(model_path: &Path, tasks: Vec<TaskOf>, out: &Path, w: &Where) -> Result<()> {
    let model = Model::load(model_path).with_context(|| format!("reading {}", model_path.display()))?;
    let base = model_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let (url, local) = place(w).await?;
    let mut c = Client::connect(&url).await?;
    let bundled = c.bundle(&model, &base).await?;
    let name = model.name.clone().unwrap_or_else(|| "batch".into());
    let job = c.submit(&name, &bundled, &tasks, true, None).await?;
    eprintln!("job {job}: {} tasks", tasks.len());
    let status = c.wait(&job, progress).await?;
    std::fs::create_dir_all(out)?;
    let mut summary = String::from("task,worker,seconds,ok,error\n");
    for r in c.results(&job).await? {
        let dir = out.join(&r.label);
        std::fs::create_dir_all(&dir)?;
        for f in &r.files {
            std::fs::write(dir.join(&f.name), &f.data)?;
        }
        summary.push_str(&format!("{},{},{:.2},{},{:?}\n", r.label, r.worker, r.seconds, r.ok, r.error));
    }
    std::fs::write(out.join("tasks.csv"), summary)?;
    if let Some(l) = local {
        l.shutdown();
    }
    eprintln!("wrote {}", out.display());
    if status.failed > 0 {
        bail!("{} tasks failed:\n  {}", status.failed, status.errors.join("\n  "));
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
    /// The input holding the seed.
    #[arg(long, default_value = "seed")]
    pub seed_input: String,
    #[arg(short, long, default_value = "calibration")]
    pub out: PathBuf,
    #[command(flatten)]
    pub at: Where,
}

pub async fn calibrate(a: &Calibrate) -> Result<()> {
    let model = Model::load(&a.model).with_context(|| format!("reading {}", a.model.display()))?;
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
    let (url, local) = place(&a.at).await?;
    let mut c = Client::connect(&url).await?;
    let bundled = c.bundle(&model, &base).await?;
    let eval = Evaluation {
        output: "simulation.maps".into(),
        observed: c.upload(Path::new(&file)).await?,
        band,
        start: a.start.clone(),
        windows: vec![1, 3, 5, 7, 9, 11],
    };
    let tasks = batch::calibration_tasks(&candidates, &a.seeds, &a.seed_input);
    let job = c.submit("calibration", &bundled, &tasks, false, Some(&eval)).await?;
    eprintln!("calibration {job}: {} candidates x {} seeds", candidates.len(), a.seeds.len());
    let status = c.wait(&job, progress).await?;
    let results = c.results(&job).await?;
    let by_label: BTreeMap<String, Option<Metrics>> =
        results.iter().map(|r| (r.label.clone(), serde_json::from_str(&r.metrics_json).ok())).collect();
    let metrics: Vec<Option<Metrics>> = tasks.iter().map(|(l, _)| by_label.get(l).cloned().flatten()).collect();
    let ranked = batch::rank(&candidates, a.seeds.len(), &metrics, &a.metric)?;
    std::fs::create_dir_all(&a.out)?;
    let keys: Vec<String> = params.iter().map(|p| p.key.clone()).collect();
    let mut csv = format!("rank,candidate,{},{},runs\n", keys.join(","), a.metric);
    for (k, s) in ranked.iter().enumerate() {
        let vals: Vec<String> =
            keys.iter().map(|key| s.settings.get(key).map(|v| v.to_string()).unwrap_or_default()).collect();
        csv.push_str(&format!("{},{},{},{},{}\n", k + 1, s.candidate, vals.join(","), s.score, s.runs));
    }
    std::fs::write(a.out.join("calibration.csv"), csv)?;
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
    if let Some(l) = local {
        l.shutdown();
    }
    eprintln!("best: {} ({} = {:.6})", serde_json::to_string(&best.settings)?, a.metric, best.score);
    eprintln!("wrote {} and {}", a.out.join("calibration.csv").display(), out_model.display());
    if status.failed > 0 {
        eprintln!("{} runs failed:\n  {}", status.failed, status.errors.join("\n  "));
    }
    Ok(())
}
