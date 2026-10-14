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
use terraflux_dist::secure::Code;
use terraflux_graph::Model;

/// Where a batch runs.
#[derive(Args, Clone)]
pub struct Where {
    /// The coordinator (`http://host:7700`); without one (and without --code), a coordinator and `--workers`
    /// workers run here.
    #[arg(long)]
    coordinator: Option<String>,
    /// The coordinator's pairing code (with no --coordinator: the coordinator of the code found on the network).
    #[arg(long)]
    code: Option<String>,
    /// A cluster or a server of `terraflux cluster add`: Terraflux started there for the batch, by SSH.
    #[arg(long)]
    on: Option<String>,
    /// Workers on this machine when no coordinator is given (default: a quarter of the cores, at least 1).
    #[arg(long)]
    workers: Option<usize>,
}

pub fn default_cache() -> PathBuf {
    std::env::temp_dir().join("terraflux-cache")
}

/// A pairing code given.
fn code_of(text: &Option<String>) -> Result<Option<Code>> {
    Ok(match text {
        Some(t) => Some(Code::parse(t)?),
        None => None,
    })
}

/// The coordinator given, or the one of the code found on the network.
async fn reach(coordinator: Option<String>, code: Option<&Code>) -> Result<String> {
    if let Some(c) = coordinator {
        return Ok(c);
    }
    let Some(code) = code else { bail!("give --coordinator, or --code to find it on the network") };
    eprintln!("looking for the coordinator of {code} on the network...");
    let found =
        terraflux_dist::discover::find(code, Duration::from_secs(5), terraflux_dist::discover::everywhere()).await?;
    let f = found.first().context("no coordinator of this code answered on the network: give --coordinator")?;
    eprintln!("found {} at {}", f.name, f.addr);
    Ok(f.addr.clone())
}

/// A cluster's profile, by its name.
pub fn profile(name: &str) -> Result<terraflux_dist::remote::Profile> {
    let all = terraflux_dist::remote::load(&terraflux_dist::remote::default_dir());
    let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
    all.iter().find(|p| p.name == name).cloned().with_context(|| {
        format!(
            "no cluster {name:?} (those of `terraflux cluster add`: {})",
            if names.is_empty() { "none".into() } else { names.join(", ") }
        )
    })
}

/// Where a batch runs: a cluster, the coordinator given (or found by its code), or workers started here.
async fn place(w: &Where) -> Result<Place> {
    if let Some(name) = &w.on {
        return Ok(Place::Cluster(Box::new(profile(name)?)));
    }
    let code = code_of(&w.code)?;
    if w.coordinator.is_some() || code.is_some() {
        let url = reach(w.coordinator.clone(), code.as_ref()).await?;
        return Ok(Place::At { url, code });
    }
    let most = terraflux_ops::threads::most();
    let n = w.workers.unwrap_or((most / 4).max(1)).clamp(1, most);
    Ok(Place::Here { workers: n, cache: default_cache(), state: None })
}

fn progress(p: Progress) {
    match p {
        Progress::Workers(n) => eprintln!("{n} workers on this machine"),
        Progress::Sending => {}
        Progress::Cluster(what) => eprintln!("{what}"),
        Progress::Submitted { job, total } => eprintln!("job {job}: {total} tasks"),
        Progress::Status(s) => {
            eprintln!("  {} of {} done, {} running, {} failed", s.done, s.total, s.running, s.failed)
        }
    }
}

/// Where a coordinator keeps its data and jobs by default (the user's application data).
pub fn default_data() -> PathBuf {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(|d| PathBuf::from(d).join("Terraflux"))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/terraflux")))
        .unwrap_or_else(std::env::temp_dir)
        .join("coordinator")
}

pub async fn serve(
    listen: SocketAddr,
    timeout: u64,
    data: Option<PathBuf>,
    code: Option<String>,
    beacons: bool,
) -> Result<()> {
    let dir = data.unwrap_or_else(default_data);
    let code = match code.as_deref() {
        Some("new") => Some(Code::generate()),
        Some(c) => Some(Code::parse(c)?),
        None => None,
    };
    let options =
        Options { timeout: Duration::from_secs(timeout), attempts: 3, dir: Some(dir.clone()), code: code.clone() };
    let (addr, service, handle) = coordinator::serve(listen, options).await?;
    eprintln!("coordinator {} listening on {addr}, its jobs kept in {}", coordinator::VERSION, dir.display());
    let _beacons = match &code {
        Some(c) => {
            eprintln!("its code: {c}\nstart workers with `terraflux worker --code {c}` (on this network)");
            (beacons && !listen.ip().is_loopback())
                .then(|| terraflux_dist::discover::announce(c, addr.port(), terraflux_dist::discover::PORT))
                .transpose()?
        }
        None => {
            eprintln!("start workers with `terraflux worker --coordinator http://<this host>:{}`", addr.port());
            None
        }
    };
    tokio::select! {
        _ = handle => {}
        _ = tokio::signal::ctrl_c() => eprintln!("stopped"),
    }
    service.close();
    Ok(())
}

pub async fn worker(
    coordinator: Option<String>,
    code: Option<String>,
    name: Option<String>,
    cache: Option<PathBuf>,
    idle: Option<f64>,
    threads: Option<usize>,
) -> Result<()> {
    let code = code_of(&code)?;
    let name = name.unwrap_or_else(|| {
        std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "worker".into())
    });
    let cache = cache.unwrap_or_else(default_cache);
    let mut o = terraflux_dist::worker::Options::new(&name, cache.clone());
    if let Some(t) = threads {
        o.threads = t.max(1);
    }
    if let Some(m) = idle {
        if terraflux_dist::idle::seconds().is_none() {
            eprintln!("worker {name}: this system does not tell when its user is away: --idle is let go");
        } else {
            eprintln!("worker {name}: tasks only after {m} minutes without the keyboard or the mouse");
            o.idle = Some(Duration::from_secs_f64(m * 60.0));
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    match (coordinator, code) {
        (Some(url), code) => {
            eprintln!("worker {name}: tasks from {url}, data kept in {}", cache.display());
            o.code = code;
            terraflux_dist::worker::run(&url, o, stop).await?;
        }
        (None, Some(code)) => {
            eprintln!(
                "worker {name}: tasks from the coordinator of {code} on this network, data kept in {}",
                cache.display()
            );
            terraflux_dist::worker::run_found(code, o, stop, terraflux_dist::discover::everywhere()).await?;
        }
        (None, None) => bail!("give --coordinator, or --code to find it on the network"),
    }
    Ok(())
}

/// A client of the coordinator given, or of the one of the code found on the network.
async fn client(coordinator: Option<String>, code: Option<String>) -> Result<Client> {
    let code = code_of(&code)?;
    let url = reach(coordinator, code.as_ref()).await?;
    Ok(Client::connect_with(&url, code.as_ref()).await?)
}

pub async fn workers(coordinator: Option<String>, code: Option<String>) -> Result<()> {
    let mut c = client(coordinator, code).await?;
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

/// The jobs of a coordinator, done or not.
pub async fn jobs(coordinator: Option<String>, code: Option<String>) -> Result<()> {
    let mut c = client(coordinator, code).await?;
    for j in c.jobs().await? {
        println!(
            "{}\t{}\t{} of {} done\t{} running\t{} failed",
            j.id,
            if j.name.is_empty() { &j.kind } else { &j.name },
            j.done,
            j.total,
            j.running,
            j.failed
        );
    }
    Ok(())
}

fn failed(s: &terraflux_dist::proto::JobStatus) -> Result<()> {
    if s.failed > 0 {
        bail!("{} tasks failed:\n  {}", s.failed, s.errors.join("\n  "));
    }
    Ok(())
}

/// Follow a job of a coordinator to its end, its files written in `out/<label>/` (those there already kept).
pub async fn results(
    coordinator: Option<String>,
    code: Option<String>,
    on: Option<String>,
    job: &str,
    out: &Path,
) -> Result<()> {
    let place = match on {
        Some(name) => Place::Cluster(Box::new(profile(&name)?)),
        None => {
            let code = code_of(&code)?;
            Place::At { url: reach(coordinator, code.as_ref()).await?, code }
        }
    };
    let start = run::Start::Resume(job.to_string());
    let o = run::run(&start, &place, Some(out), &AtomicBool::new(false), &mut progress).await?;
    eprintln!("wrote {}", out.display());
    failed(&o.status)
}

/// Run a batch of tasks of a model and write each task's files in `out/<label>/`; with `detach`, only submit it
/// (to a coordinator) and say its job.
pub async fn submit(model_path: &Path, tasks: Vec<TaskOf>, out: &Path, w: &Where, detach: bool) -> Result<()> {
    let model = Model::load_expanded(model_path).with_context(|| format!("reading {}", model_path.display()))?;
    let base = model_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = model.name.clone().unwrap_or_else(|| "batch".into());
    let b = Batch { name, model, base, tasks, files: true, evaluation: None };
    let place = place(w).await?;
    if detach {
        let Place::At { url, code } = place else { bail!("--detach needs --coordinator or --code") };
        let mut c = Client::connect_with(&url, code.as_ref()).await?;
        let job = run::submit(&mut c, &b, &mut progress).await?;
        println!("{job}");
        let with = code.map(|c| format!(" --code {c}")).unwrap_or_default();
        eprintln!("its results later: terraflux results {job} --coordinator {url}{with} --out {}", out.display());
        return Ok(());
    }
    let start = run::Start::New(Box::new(b));
    let o = run::run(&start, &place, Some(out), &AtomicBool::new(false), &mut progress).await?;
    eprintln!("wrote {}", out.display());
    failed(&o.status)
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
    let start = run::Start::New(Box::new(b));
    let o = run::run(&start, &place(&a.at).await?, None, &AtomicBool::new(false), &mut progress).await?;
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

/// The commands of the clusters and servers reached by SSH.
#[derive(clap::Subcommand)]
pub enum ClusterCommand {
    /// Save a cluster or a server: `--workers N` (on the host itself) or `--slurm` (on the nodes of an
    /// allocation: --nodes, --cpus, --time, --partition, --account, --per-node).
    Add {
        name: String,
        /// `user@host`, or an alias of ~/.ssh/config.
        #[arg(long)]
        host: String,
        #[arg(long)]
        ssh_port: Option<u16>,
        /// Terraflux there (default: `terraflux` on its PATH, else ~/.terraflux/bin/terraflux).
        #[arg(long)]
        program: Option<String>,
        /// Workers on the host itself.
        #[arg(long, conflicts_with = "slurm")]
        workers: Option<usize>,
        /// Workers on the nodes of a Slurm allocation.
        #[arg(long)]
        slurm: bool,
        #[arg(long, default_value_t = 1)]
        nodes: usize,
        /// The threads of each node used.
        #[arg(long, default_value_t = 8)]
        cpus: usize,
        /// Workers per node.
        #[arg(long, default_value_t = 1)]
        per_node: usize,
        #[arg(long, default_value = "04:00:00")]
        time: String,
        #[arg(long)]
        partition: Option<String>,
        #[arg(long)]
        account: Option<String>,
    },
    /// The clusters and servers saved.
    List,
    /// Check that a cluster is reached and has Terraflux of this version.
    Test { name: String },
    /// Stop Terraflux's coordinator and workers on a cluster (its jobs kept).
    Stop { name: String },
    /// Forget a cluster.
    Remove { name: String },
}

pub fn cluster(c: ClusterCommand) -> Result<()> {
    use terraflux_dist::remote::{self, Cluster, Profile, Scheduler};
    let dir = remote::default_dir();
    let mut all = remote::load(&dir);
    match c {
        ClusterCommand::Add {
            name,
            host,
            ssh_port,
            program,
            workers,
            slurm,
            nodes,
            cpus,
            per_node,
            time,
            partition,
            account,
        } => {
            let scheduler = if slurm {
                Scheduler::Slurm { partition, account, time, nodes, cpus, workers: per_node }
            } else {
                Scheduler::Host { workers: workers.unwrap_or(1) }
            };
            let mut p = Profile::new(&name, &host, scheduler);
            p.ssh_port = ssh_port;
            p.program = program;
            all.retain(|x| x.name != name);
            all.push(p);
            remote::save(&dir, &all)?;
            eprintln!(
                "saved {name} in {}: `terraflux cluster test {name}` checks it",
                dir.join("clusters.toml").display()
            );
        }
        ClusterCommand::List => {
            for p in &all {
                let how = match &p.scheduler {
                    Scheduler::Host { workers } => format!("{workers} workers on the host"),
                    Scheduler::Slurm { nodes, cpus, time, partition, .. } => format!(
                        "Slurm: {nodes} nodes x {cpus} threads, {time}{}",
                        partition.as_ref().map(|p| format!(", partition {p}")).unwrap_or_default()
                    ),
                };
                println!("{}\t{}\t{how}", p.name, p.host);
            }
        }
        ClusterCommand::Test { name } => {
            let c = Cluster::new(profile(&name)?);
            let (path, version) = c.program()?;
            println!("{}: Terraflux {version} ({path})", c.profile.host);
            c.check()?;
        }
        ClusterCommand::Stop { name } => {
            Cluster::new(profile(&name)?).stop()?;
            eprintln!("stopped");
        }
        ClusterCommand::Remove { name } => {
            let before = all.len();
            all.retain(|x| x.name != name);
            if all.len() == before {
                bail!("no cluster {name:?}");
            }
            remote::save(&dir, &all)?;
        }
    }
    Ok(())
}

/// The cloud-init user data of a server, or of a worker for a coordinator.
pub fn cloud_init(
    worker: bool,
    coordinator: Option<String>,
    on: Option<String>,
    code: Option<String>,
    threads: Option<usize>,
    package: Option<String>,
) -> Result<String> {
    use terraflux_dist::cloud::{Role, package_url, user_data};
    let url = package.unwrap_or_else(|| package_url(coordinator::VERSION));
    let role = if worker {
        let at =
            coordinator.context("--worker needs --coordinator (the server's address on the provider's network)")?;
        let (at, code) = match on {
            Some(name) => {
                let p = profile(&name)?;
                let at = if at.contains(':') { at } else { format!("{at}:{}", p.port) };
                (at, p.code)
            }
            None => (at, Code::parse(&code.context("give --on <server> or --code")?)?.to_string()),
        };
        if !at.contains(':') {
            bail!("give the coordinator's port: --coordinator host:port (or --on <server>)");
        }
        Role::Worker { coordinator: at, code, threads }
    } else {
        Role::Server
    };
    Ok(user_data(&role, &url))
}
