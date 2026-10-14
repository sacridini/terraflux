//! Clusters and servers reached by SSH, from a profile saved once: a batch runs there without logging in. The
//! batch's client checks Terraflux on the other side (the same version), starts a coordinator there (with a
//! pairing code of the profile's, its jobs kept in `~/.terraflux`), starts its workers (on the host itself, or on
//! the nodes of a Slurm allocation), reaches the coordinator through an SSH tunnel, and stops the workers and
//! the coordinator when the batch is done. A batch whose client went away goes on there, and is followed again
//! by its job.
//!
//! SSH is the system's own (`ssh`, OpenSSH: in Windows 10 and later, macOS and Linux), with the user's keys and
//! `~/.ssh/config`; it must log in without a password asked (a key, an agent). `TERRAFLUX_SSH` names another
//! program.

use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::secure::Code;
use crate::{Error, Result};

/// Where the workers run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Scheduler {
    /// On the host itself: so many workers, sharing its threads.
    Host { workers: usize },
    /// On the nodes of a Slurm allocation: `workers` per node, each with `cpus` / `workers` threads.
    Slurm {
        #[serde(default)]
        partition: Option<String>,
        #[serde(default)]
        account: Option<String>,
        /// The allocation's time (`04:00:00`).
        time: String,
        nodes: usize,
        /// The threads of each node used.
        cpus: usize,
        #[serde(default = "one")]
        workers: usize,
    },
}

fn one() -> usize {
    1
}

/// A cluster or a server, as saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    /// `user@host`, or an alias of `~/.ssh/config`.
    pub host: String,
    #[serde(default)]
    pub ssh_port: Option<u16>,
    /// Terraflux on the other side (default: `terraflux` on its PATH, else `~/.terraflux/bin/terraflux`).
    #[serde(default)]
    pub program: Option<String>,
    pub scheduler: Scheduler,
    /// The pairing code of its coordinator.
    pub code: String,
    /// The port of its coordinator there.
    pub port: u16,
}

impl Profile {
    /// A profile with a code and a port of its own.
    pub fn new(name: &str, host: &str, scheduler: Scheduler) -> Profile {
        let r = crate::hash(format!("{name}{host}{}", Code::generate()).as_bytes());
        let port = 20_000 + (u16::from_str_radix(&r[..4], 16).unwrap_or(0) % 30_000);
        Profile {
            name: name.into(),
            host: host.into(),
            ssh_port: None,
            program: None,
            scheduler,
            code: Code::generate().to_string(),
            port,
        }
    }

    pub fn code(&self) -> Result<Code> {
        Code::parse(&self.code)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Profiles {
    #[serde(default, rename = "cluster")]
    clusters: Vec<Profile>,
}

/// The profiles saved (`clusters.toml` in a folder: the user's application data).
pub fn load(dir: &std::path::Path) -> Vec<Profile> {
    std::fs::read_to_string(dir.join("clusters.toml"))
        .ok()
        .and_then(|t| toml::from_str::<Profiles>(&t).ok())
        .map(|p| p.clusters)
        .unwrap_or_default()
}

pub fn save(dir: &std::path::Path, profiles: &[Profile]) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = toml::to_string_pretty(&Profiles { clusters: profiles.to_vec() })
        .map_err(|e| Error::Other(format!("the clusters' file: {e}")))?;
    std::fs::write(dir.join("clusters.toml"), text)?;
    Ok(())
}

/// A tunnel to the other side's coordinator: the local port, and what keeps it open.
pub struct Tunnel {
    pub port: u16,
    child: Option<Child>,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        if let Some(c) = &mut self.child {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// How scripts reach the other side.
pub trait Shell: Send + Sync {
    /// Run a POSIX shell script there: its output, or its error.
    fn run(&self, script: &str) -> Result<String>;
    /// A tunnel to a port there.
    fn tunnel(&self, port: u16) -> Result<Tunnel>;
}

/// SSH.
pub struct Ssh {
    pub host: String,
    pub port: Option<u16>,
}

fn ssh_program() -> String {
    std::env::var("TERRAFLUX_SSH").unwrap_or_else(|_| "ssh".into())
}

impl Ssh {
    fn command(&self) -> Command {
        let mut c = Command::new(ssh_program());
        c.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=20", "-o", "ServerAliveInterval=30"]);
        if let Some(p) = self.port {
            c.args(["-p", &p.to_string()]);
        }
        c
    }
}

fn output(mut c: Command, script: &str, what: &str) -> Result<String> {
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| Error::Other(format!("{what}: {e}")))?;
    child.stdin.take().expect("stdin").write_all(script.as_bytes())?;
    let o = child.wait_with_output()?;
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
        return Err(Error::Other(format!("{what}: {}", if err.is_empty() { o.status.to_string() } else { err })));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// A free port of this computer.
fn free_port() -> Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// Wait until a port of this computer takes connections.
fn reachable(port: u16, child: &mut Option<Child>) -> Result<()> {
    let t = Instant::now();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    while TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_err() {
        if let Some(c) = child
            && let Ok(Some(s)) = c.try_wait()
        {
            return Err(Error::Other(format!("the tunnel stopped ({s})")));
        }
        if t.elapsed() > Duration::from_secs(30) {
            return Err(Error::Other("the tunnel did not open".into()));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

impl Shell for Ssh {
    fn run(&self, script: &str) -> Result<String> {
        let mut c = self.command();
        c.args([self.host.as_str(), "sh", "-s"]);
        output(c, script, &format!("ssh {}", self.host))
    }

    fn tunnel(&self, port: u16) -> Result<Tunnel> {
        let local = free_port()?;
        let mut c = self.command();
        c.args(["-N", "-o", "ExitOnForwardFailure=yes", "-L", &format!("127.0.0.1:{local}:127.0.0.1:{port}")]);
        c.arg(&self.host).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = Some(c.spawn().map_err(|e| Error::Other(format!("ssh {}: {e}", self.host)))?);
        reachable(local, &mut child)?;
        Ok(Tunnel { port: local, child })
    }
}

/// This computer's own shell as the other side, its home in a folder of its own (tests).
pub struct Here {
    pub home: PathBuf,
}

impl Shell for Here {
    fn run(&self, script: &str) -> Result<String> {
        // sh: on Windows, Git's (a "bash" there may be WSL's, in System32)
        let mut c = Command::new("sh");
        c.arg("-s").env("HOME", &self.home);
        output(c, script, "the shell")
    }

    fn tunnel(&self, port: u16) -> Result<Tunnel> {
        reachable(port, &mut None)?;
        Ok(Tunnel { port, child: None })
    }
}

/// A word for a shell script, quoted.
fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The script that starts the workers of a Slurm allocation.
pub fn sbatch(p: &Profile, program: &str, login: &str) -> String {
    let Scheduler::Slurm { partition, account, time, nodes, cpus, workers } = &p.scheduler else {
        return String::new();
    };
    let per = (cpus / workers.max(&1)).max(1);
    let mut s = String::from("#!/bin/bash\n#SBATCH --job-name=terraflux-workers\n");
    s += &format!("#SBATCH --nodes={nodes}\n#SBATCH --ntasks-per-node={workers}\n#SBATCH --cpus-per-task={per}\n");
    s += &format!("#SBATCH --time={time}\n#SBATCH --output=slurm-terraflux-%j.log\n");
    if let Some(x) = partition.as_deref().filter(|x| !x.is_empty()) {
        s += &format!("#SBATCH --partition={x}\n");
    }
    if let Some(x) = account.as_deref().filter(|x| !x.is_empty()) {
        s += &format!("#SBATCH --account={x}\n");
    }
    s += "export OMP_NUM_THREADS=\"$SLURM_CPUS_PER_TASK\"\n";
    s += &format!(
        "srun --kill-on-bad-exit=0 bash -c '{} worker --coordinator {login}:{} --code {} --name \"$(hostname)-$SLURM_PROCID\" \
         --threads \"$SLURM_CPUS_PER_TASK\" --cache \"${{TMPDIR:-/tmp}}/terraflux-cache-$SLURM_JOB_ID-$SLURM_PROCID\"'\n",
        program.replace('\'', ""),
        p.port,
        p.code
    );
    s
}

/// A cluster or server reached: what is done there.
pub struct Cluster {
    pub profile: Profile,
    shell: Box<dyn Shell>,
}

/// The folder of Terraflux's files on the other side.
const DIR: &str = "$HOME/.terraflux";

impl Cluster {
    pub fn new(profile: Profile) -> Cluster {
        let shell = Box::new(Ssh { host: profile.host.clone(), port: profile.ssh_port });
        Cluster { profile, shell }
    }

    /// Reached by another shell (tests).
    pub fn with_shell(profile: Profile, shell: Box<dyn Shell>) -> Cluster {
        Cluster { profile, shell }
    }

    /// Terraflux there: its path, and its version.
    pub fn program(&self) -> Result<(String, String)> {
        let given = self.profile.program.clone().unwrap_or_default();
        let script = format!(
            "for p in {} terraflux \"{DIR}/bin/terraflux\"; do\n  [ -z \"$p\" ] && continue\n  if command -v \"$p\" >/dev/null \
             2>&1; then echo \"$p\"; \"$p\" --version; exit 0; fi\ndone\nexit 3\n",
            q(&given)
        );
        let out = self.shell.run(&script).map_err(|e| {
            Error::Other(format!(
                "{e}\nno Terraflux on {}: install Terraflux {} there (in its PATH, or in ~/.terraflux/bin), or give its \
                 path in the profile",
                self.profile.host,
                crate::coordinator::VERSION
            ))
        })?;
        let mut lines = out.lines();
        let path = lines.next().unwrap_or_default().trim().to_string();
        let version = lines.next().unwrap_or_default().trim().trim_start_matches("terraflux").trim().to_string();
        Ok((path, version))
    }

    /// Check Terraflux there: its path (of this version).
    pub fn check(&self) -> Result<String> {
        let (path, version) = self.program()?;
        if version != crate::coordinator::VERSION {
            return Err(Error::Other(format!(
                "{} has Terraflux {version}, this computer {}: install the same version on both",
                self.profile.host,
                crate::coordinator::VERSION
            )));
        }
        Ok(path)
    }

    /// Start the coordinator there (unless it runs) and its workers: the host name the workers reach it by.
    pub fn start(&self, program: &str) -> Result<String> {
        let (port, code, p) = (self.profile.port, &self.profile.code, q(program));
        let mut s = format!(
            "set -e\nmkdir -p \"{DIR}\"\ncd \"{DIR}\"\nif [ -f serve-{port}.pid ] && kill -0 \"$(cat serve-{port}.pid)\" 2>/dev/null; then :; \
             else\n  nohup {p} serve --listen 0.0.0.0:{port} --code {code} --no-beacons --data \"{DIR}/coordinator-{port}\" \
             > serve-{port}.log 2>&1 &\n  echo $! > serve-{port}.pid\nfi\nhostname\n"
        );
        match &self.profile.scheduler {
            Scheduler::Host { workers } => {
                let n = (*workers).max(1);
                s += &format!(
                    "if [ ! -s workers-{port}.pids ]; then\n  T=$(( $(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null || echo {n}) \
                     / {n} ))\n  [ \"$T\" -lt 1 ] && T=1\n  for i in $(seq 1 {n}); do\n    nohup {p} worker --coordinator 127.0.0.1:{port} \
                     --code {code} --name \"$(hostname)-$i\" --threads \"$T\" --cache \"{DIR}/cache\" > worker-{port}-$i.log 2>&1 &\n    \
                     echo $! >> workers-{port}.pids\n  done\nfi\n"
                );
            }
            Scheduler::Slurm { .. } => {
                // the login node's name put in the script where the workers reach it
                s += &format!(
                    "if [ ! -s slurm-{port}.job ] || ! squeue -h -j \"$(cat slurm-{port}.job)\" >/dev/null 2>&1; then\n  cat > \
                     workers-{port}.sbatch <<'TERRAFLUX'\n{}TERRAFLUX\n  sed -i \"s/@LOGIN@/$(hostname)/\" workers-{port}.sbatch\n  \
                     sbatch --parsable workers-{port}.sbatch | cut -d';' -f1 > slurm-{port}.job\nfi\n",
                    sbatch(&self.profile, program, "@LOGIN@")
                );
            }
        }
        let out = self.shell.run(&s)?;
        Ok(out.lines().next().unwrap_or_default().trim().to_string())
    }

    /// A tunnel to the coordinator there, once it answers.
    pub fn tunnel(&self) -> Result<Tunnel> {
        self.shell.tunnel(self.profile.port)
    }

    /// Stop the workers and the coordinator there (its jobs kept).
    pub fn stop(&self) -> Result<()> {
        let port = self.profile.port;
        let s = format!(
            "cd \"{DIR}\" 2>/dev/null || exit 0\nif [ -s slurm-{port}.job ]; then scancel \"$(cat slurm-{port}.job)\" 2>/dev/null || true; rm -f \
             slurm-{port}.job; fi\nif [ -s workers-{port}.pids ]; then kill $(cat workers-{port}.pids) 2>/dev/null || true; rm -f \
             workers-{port}.pids; fi\nif [ -s serve-{port}.pid ]; then kill \"$(cat serve-{port}.pid)\" 2>/dev/null || true; rm -f \
             serve-{port}.pid; fi\n"
        );
        self.shell.run(&s)?;
        Ok(())
    }
}

/// A batch's way to a cluster: Terraflux checked there, the coordinator and workers started, the tunnel open.
pub struct Opened {
    pub cluster: Cluster,
    pub tunnel: Tunnel,
}

impl Opened {
    pub fn open(cluster: Cluster) -> Result<Opened> {
        let program = cluster.check()?;
        cluster.start(&program)?;
        let tunnel = cluster.tunnel()?;
        Ok(Opened { cluster, tunnel })
    }

    /// The coordinator, through the tunnel.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.tunnel.port)
    }
}

/// The folder the profiles are kept in by default (the user's application data).
pub fn default_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(|d| PathBuf::from(d).join("Terraflux"))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/Terraflux")))
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_are_kept_and_slurm_scripts_written() {
        let d = std::env::temp_dir().join(format!("terraflux-profiles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let mut p = Profile::new(
            "hpc",
            "me@login.hpc.edu",
            Scheduler::Slurm {
                partition: Some("short".into()),
                account: None,
                time: "02:00:00".into(),
                nodes: 4,
                cpus: 32,
                workers: 2,
            },
        );
        p.ssh_port = Some(2222);
        assert!(Code::parse(&p.code).is_ok() && p.port >= 20_000);
        save(&d, std::slice::from_ref(&p)).unwrap();
        assert_eq!(load(&d), vec![p.clone()]);
        let s = sbatch(&p, "/opt/terraflux/terraflux", "login1");
        for want in [
            "#SBATCH --nodes=4",
            "#SBATCH --ntasks-per-node=2",
            "#SBATCH --cpus-per-task=16",
            "#SBATCH --time=02:00:00",
            "#SBATCH --partition=short",
            "--coordinator login1:",
            &format!("--code {}", p.code),
        ] {
            assert!(s.contains(want), "{want}\n{s}");
        }
        assert!(!s.contains("--account"));
        assert_eq!(q("it's"), r"'it'\''s'");
        let _ = std::fs::remove_dir_all(&d);
    }
}
