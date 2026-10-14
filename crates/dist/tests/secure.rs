//! Computers that work together from a code: a coordinator with a code speaks TLS and serves only those who
//! know the code; a computer that lends itself finds the coordinator of its code on the network and works for
//! it. Everything here stays on this computer (loopback), so no firewall is asked.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use terraflux_dist::client::Client;
use terraflux_dist::coordinator::{self, Options};
use terraflux_dist::pool::{Lender, Pool};
use terraflux_dist::run::{self, Batch, Place, Start};
use terraflux_dist::secure::Code;
use terraflux_graph::{Model, Node};

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A model of the builder: a neutral map by a node's seed.
fn batch(n: usize) -> Batch {
    let mut m = Model::new("noise");
    m.nodes.push(Node::new("noise", "nlm.random").param("rows", 30).param("columns", 40).param("seed", 1));
    m.outputs.insert("noise".into(), terraflux_graph::Output { from: "noise.map".into(), path: None, dtype: None });
    Batch {
        name: "noise".into(),
        model: m,
        base: PathBuf::new(),
        tasks: terraflux_dist::batch::replicates(n, 1, &["noise.seed".to_string()]),
        files: true,
        evaluation: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_who_knows_the_code_gets_in() {
    let code = Code::generate();
    let options = Options { code: Some(code.clone()), dir: Some(tmp("secure-coordinator")), ..Default::default() };
    let (addr, service, handle) = coordinator::serve(SocketAddr::from(([127, 0, 0, 1], 0)), options).await.unwrap();
    let url = format!("http://{addr}");
    // the code: in, and a batch runs on a worker of the code
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let mut o = terraflux_dist::worker::Options::new("w", tmp("secure-cache"));
    o.code = Some(code.clone());
    o.threads = 2;
    let (u, s) = (url.clone(), stop.clone());
    tokio::spawn(async move { terraflux_dist::worker::run(&u, o, s).await });
    let out = tmp("secure-out");
    let place = Place::At { url: url.clone(), code: Some(code.clone()) };
    let r = run::run(&Start::New(Box::new(batch(2))), &place, Some(&out), &AtomicBool::new(false), &mut |_| {})
        .await
        .unwrap();
    assert_eq!((r.status.done, r.status.failed), (2, 0), "{:?}", r.status.errors);
    assert!(out.join("seed_2").join("noise.tif").exists());
    // another code: the coordinator cannot prove it knows it, and nothing more is said
    let e = Client::connect_with(&url, Some(&Code::generate())).await.err().unwrap();
    assert!(e.to_string().contains("does not know this code"), "{e}");
    // no code: no TLS, no answer
    assert!(Client::connect(&url).await.is_err());
    // a coordinator without the code (another certificate, another proof) is refused by the client of the code
    let (other, _, h2) = coordinator::serve(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        Options { code: Some(Code::generate()), ..Default::default() },
    )
    .await
    .unwrap();
    assert!(Client::connect_with(&format!("http://{other}"), Some(&code)).await.is_err());
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    handle.abort();
    h2.abort();
    service.close();
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let t = Instant::now();
    while !done() {
        assert!(t.elapsed() < Duration::from_secs(60), "{what}: too long");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_lent_computer_finds_the_pool_of_its_code_and_works_for_it() {
    // the pool of an application, shared, without workers of its own: the runs are the lent computer's
    let code = Code::generate();
    let beacons = 47_000 + (std::process::id() % 1000) as u16;
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let mut pool = Pool::start(tmp("pool-dir"), tmp("pool-cache"), code.clone(), false, 0)
        .unwrap()
        .with_network(loopback, 0, beacons)
        .unwrap();
    pool.set_shared(true).unwrap();
    assert!(pool.shared());
    // a computer given another code never comes
    let stranger =
        Lender::start(Code::generate(), tmp("stranger-cache"), 1, None, SocketAddr::new(loopback, beacons)).unwrap();
    let lender = Lender::start(code.clone(), tmp("lender-cache"), 2, None, SocketAddr::new(loopback, beacons)).unwrap();
    until("the lent computer", || lender.status().coordinator.is_some());
    assert_eq!(pool.workers().len(), 1, "only the computer of the code");
    // a batch on the pool: done by the lent computer
    let out = tmp("pool-out");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let place = pool.place();
    let r = rt
        .block_on(run::run(&Start::New(Box::new(batch(3))), &place, Some(&out), &AtomicBool::new(false), &mut |_| {}))
        .unwrap();
    assert_eq!((r.status.done, r.status.failed), (3, 0), "{:?}", r.status.errors);
    assert!(r.results.iter().all(|t| pool.workers().iter().any(|w| w.id == t.worker)));
    until("the lent computer's count", || lender.status().done == 3);
    assert!(stranger.status().coordinator.is_none());
    // paused: it takes no more tasks (once the second its ask for one waits is over)
    lender.paused.store(true, std::sync::atomic::Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(1500));
    let job = rt.block_on(async {
        let mut c = Client::connect_with(&pool.url(), Some(&code)).await.unwrap();
        run::submit(&mut c, &batch(1), &mut |_| {}).await.unwrap()
    });
    std::thread::sleep(Duration::from_secs(3));
    let status = rt.block_on(async {
        let mut c = Client::connect_with(&pool.url(), Some(&code)).await.unwrap();
        c.status(&job).await.unwrap()
    });
    assert_eq!(status.done, 0, "paused");
    lender.paused.store(false, std::sync::atomic::Ordering::Relaxed);
    let r =
        rt.block_on(run::run(&Start::Resume(job), &place, Some(&out), &AtomicBool::new(false), &mut |_| {})).unwrap();
    assert_eq!(r.status.done, 1);
    // workers of the pool's own, then none again
    pool.set_workers(2);
    until("the pool's workers", || pool.workers().len() >= 3);
    drop(lender);
    drop(stranger);
}
