//! A cluster reached by a shell: Terraflux checked there, its coordinator and workers started, a batch run
//! through it, everything stopped. The other side is this computer's own shell (Git's bash on Windows) with a
//! home of its own, and this build of the command.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use terraflux_dist::remote::{Cluster, Here, Profile, Scheduler};
use terraflux_dist::run::{self, Batch, Place, Start};
use terraflux_graph::{Model, Node};

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn shell_here() -> bool {
    std::process::Command::new("sh").arg("-c").arg("exit 0").status().is_ok_and(|s| s.success())
}

#[test]
fn a_batch_runs_on_a_cluster_started_for_it() {
    if !shell_here() {
        eprintln!("no POSIX shell here: skipped");
        return;
    }
    let home = tmp("remote-home");
    let program = env!("CARGO_BIN_EXE_terraflux").replace('\\', "/");
    let mut p = Profile::new("here", "", Scheduler::Host { workers: 2 });
    p.program = Some(program.clone());
    let cluster = Cluster::with_shell(p.clone(), Box::new(Here { home: home.clone() }));
    // Terraflux there, of this version
    let path = cluster.check().unwrap();
    assert_eq!(path, program);
    // another program: said what to install
    let mut wrong = p.clone();
    wrong.program = Some("/no/such/terraflux".into());
    let missing = Cluster::with_shell(wrong, Box::new(Here { home: home.clone() }));
    assert!(missing.program().is_ok(), "the PATH's terraflux, or ~/.terraflux/bin, may be found instead");
    // its coordinator and workers started, reached
    cluster.start(&path).unwrap();
    let tunnel = cluster.tunnel().unwrap();
    let url = format!("http://127.0.0.1:{}", tunnel.port);
    let mut m = Model::new("noise");
    m.nodes.push(Node::new("noise", "nlm.random").param("rows", 20).param("columns", 20).param("seed", 1));
    m.outputs.insert(
        "noise".into(),
        terraflux_graph::Output { from: "noise.map".into(), path: None, dtype: None, layer: None },
    );
    let b = Batch {
        name: "noise".into(),
        model: m,
        base: PathBuf::new(),
        tasks: terraflux_dist::batch::replicates(4, 1, &["noise.seed".to_string()]),
        files: true,
        evaluation: None,
    };
    let out = tmp("remote-out");
    let place = Place::At { url, code: Some(p.code().unwrap()) };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let r = rt.block_on(run::run(&Start::New(Box::new(b)), &place, Some(&out), &AtomicBool::new(false), &mut |_| {}));
    let r = r.unwrap();
    assert_eq!((r.status.done, r.status.failed), (4, 0), "{:?}", r.status.errors);
    assert!(out.join("seed_4").join("noise.tif").exists());
    let names: Vec<String> = r.results.iter().map(|t| t.worker.clone()).collect();
    assert!(!names.is_empty());
    // started again: the coordinator and its workers that run are kept
    cluster.start(&path).unwrap();
    let pids = std::fs::read_to_string(home.join(".terraflux").join(format!("workers-{}.pids", p.port))).unwrap();
    assert_eq!(pids.lines().count(), 2, "two workers, not four");
    // stopped: nobody answers there any more
    drop(tunnel);
    cluster.stop().unwrap();
    let t = Instant::now();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], p.port));
    while std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
        assert!(t.elapsed() < Duration::from_secs(20), "the coordinator stopped");
        std::thread::sleep(Duration::from_millis(200));
    }
}
