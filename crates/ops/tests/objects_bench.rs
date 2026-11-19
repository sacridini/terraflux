//! A benchmark battery of the segmenters on fixed synthetic images (ignored: run it by hand, in release):
//! `cargo test -p terraflux-ops --release --test ops objects_bench -- --ignored --nocapture --test-threads=1`
//! It prints a table: segmenter, image, threads, seconds, objects, and a hash of the labels (a change that must not alter the objects keeps it). `BENCH_SKIP=meanshift,watershed` leaves some out,
//! `BENCH_ONLY=multiresolution` keeps only those (`segment.threshold`, `segment.spectral_difference`: `from` names the segmenter that makes the objects first), `BENCH_SIDES=1000` the sides of the images, `BENCH_THREADS=1`.
//! `BENCH_RATES=1` also writes `crates/ops/src/objects/rates.json` (what `terraflux_ops::objects::estimate` reads): it needs
//! the default sides (1000 and 2000), `BENCH_THREADS` with 1 and N (the largest is N), and the segmenters of `RATED`;
//! `BENCH_SKIP=optimize` leaves out what the rates do not use.

use std::sync::Arc;
use std::time::Instant;

use terraflux_ops::objects::Objects;
use terraflux_ops::{Args, Ctx, Key, Registry, Stack, Value};
use terraflux_raster::Grid;

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*s >> 11) as f64) / (1u64 << 53) as f64
}

/// A class map (Voronoi cells of random points) with noise and a texture, `bands` bands, seeded.
fn image(w: usize, h: usize, bands: usize, seed: u64) -> Value {
    let mut s = seed;
    let sites: Vec<(f64, f64, f64)> = (0..(w * h / 12_000).max(8))
        .map(|_| (lcg(&mut s) * h as f64, lcg(&mut s) * w as f64, lcg(&mut s) * 200.0))
        .collect();
    let mut class = vec![0usize; w * h];
    for r in 0..h {
        for c in 0..w {
            let d = |p: &(f64, f64, f64)| (p.0 - r as f64).powi(2) + (p.1 - c as f64).powi(2);
            let mut best = 0;
            for (k, p) in sites.iter().enumerate() {
                if d(p) < d(&sites[best]) {
                    best = k;
                }
            }
            class[r * w + c] = best;
        }
    }
    let keys = (0..bands).map(|k| Key::Name(format!("b{}", k + 1))).collect();
    let layers = (0..bands)
        .map(|b| {
            Arc::new(
                (0..w * h)
                    .map(|i| {
                        let (r, c) = ((i / w) as f64, (i % w) as f64);
                        let tex = 6.0 * ((r * 0.35 + b as f64).sin() * (c * 0.27).cos());
                        let noise = (lcg(&mut s) + lcg(&mut s) + lcg(&mut s) + lcg(&mut s) - 2.0) * 10.0;
                        sites[class[i]].2 * (1.0 + 0.1 * b as f64) + tex + noise
                    })
                    .collect(),
            )
        })
        .collect();
    Value::Stack(Stack { grid: Arc::new(Grid::new(w, h)), keys, layers })
}

/// FNV-1a over the labels as u32 (and the count): equal hashes mean the same objects.
fn hash(o: &Objects) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in (o.count as u32).to_le_bytes().into_iter().chain((0..o.grid.cells()).flat_map(|i| o.labels[i].to_le_bytes())) {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

fn run(op: &str, img: &Value, params: serde_json::Value, threads: usize) -> (f64, usize, u64) {
    let r = Registry::standard();
    let mut params = params.as_object().cloned().unwrap_or_default();
    let mut inputs: std::collections::BTreeMap<String, Value> = [("image".to_string(), img.clone())].into_iter().collect();
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    // `from`: the objects a merging segmenter starts from, made first by another segmenter (not timed)
    if let Some(serde_json::Value::String(from)) = params.remove("from") {
        let (p, size) = (serde_json::Map::new(), serde_json::json!({ "size": 4 }).as_object().cloned().unwrap());
        let p = if from == "chessboard" { size } else { p };
        let first = Args { node: "n", inputs: inputs.clone(), params: &p };
        let out = pool.install(|| r.get(&format!("segment.{from}")).unwrap().run(&mut Ctx::default(), &first).unwrap());
        inputs.insert("objects".to_string(), out["objects"].clone());
    }
    let args = Args { node: "n", inputs, params: &params };
    let t = Instant::now();
    let out = pool.install(|| r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap());
    let secs = t.elapsed().as_secs_f64();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    let o: &Arc<Objects> = o;
    (secs, o.count, hash(o))
}

/// The variants the rates are made of: segmenter, the variant of the whole image, the tiled one.
type Rated = (&'static str, &'static str, Option<&'static str>);

const RATED: [Rated; 12] = [
    // (no setting scales the rate: multiresolution's scale changes it by 3%, region growing's threshold the other way
    // at 1000 and at 2000)
    ("segment.multiresolution", "scale50", Some("scale50-tile512")),
    ("segment.slic", "default", None),
    ("segment.snic", "default", None),
    ("segment.felzenszwalb", "default", None),
    ("segment.watershed", "default", None),
    ("segment.meanshift", "default", None),
    ("segment.threshold", "otsu-3", None),
    ("segment.spectral_difference", "slic-d8", None),
    ("segment.region_growing", "t0.05", Some("t0.05-tile512")),
    ("segment.quickshift", "default", None),
    ("segment.chessboard", "default", None),
    ("segment.quadtree", "default", None),
];

type Seen = Vec<(String, String, usize, usize, f64)>;

/// The rates file from the seconds seen: `{ cells, t1, tn }` for the whole image and for the tiled run.
fn write_rates(seen: &Seen, n: usize) {
    let at = |op: &str, variant: &str, side: usize, threads: usize| -> Option<f64> {
        seen.iter().find(|r| r.0 == op && r.1 == variant && r.2 == side && r.3 == threads).map(|r| r.4)
    };
    let series = |op: &str, variant: &str| -> Option<serde_json::Value> {
        let t = |side, threads| at(op, variant, side, threads);
        Some(serde_json::json!({
            "cells": [1_000_000, 4_000_000],
            "t1": [t(1000, 1)?, t(2000, 1)?],
            "tn": [t(1000, n)?, t(2000, n)?],
        }))
    };
    let mut ops = serde_json::Map::new();
    for (op, variant, tiled) in RATED {
        let Some(mut entry) = series(op, variant) else {
            println!("rates: {op} {variant} is missing a run, left out");
            continue;
        };
        if let Some(t) = tiled.and_then(|v| series(op, v)) {
            let mut t = t;
            t["tile"] = 512.into();
            entry["tiled"] = t;
        }
        ops.insert(op.to_string(), entry);
    }
    let doc = serde_json::json!({
        "note": "written by BENCH_RATES=1 on crates/ops/tests/objects_bench.rs: seconds of 1000x1000 and 2000x2000 cells, 4 bands, on 1 thread and on `threads`",
        "threads": n,
        "ops": ops,
    });
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/objects/rates.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "
").unwrap();
    println!("rates written to {path}");
}

#[test]
#[ignore]
fn objects_bench() {
    let skip = std::env::var("BENCH_SKIP").unwrap_or_default();
    let cases: Vec<(&str, &str, serde_json::Value)> = vec![
        ("segment.multiresolution", "scale20", serde_json::json!({ "scale": 20, "shape": 0.1, "compactness": 0.5 })),
        ("segment.multiresolution", "scale50", serde_json::json!({ "scale": 50, "shape": 0.1, "compactness": 0.5 })),
        (
            "segment.multiresolution",
            "scale50-tile512",
            serde_json::json!({ "scale": 50, "shape": 0.1, "compactness": 0.5, "tile": 512 }),
        ),
        ("segment.chessboard", "default", serde_json::json!({})),
        ("segment.quadtree", "default", serde_json::json!({ "threshold": 20 })),
        ("segment.slic", "default", serde_json::json!({})),
        ("segment.snic", "default", serde_json::json!({})),
        ("segment.felzenszwalb", "default", serde_json::json!({})),
        ("segment.watershed", "default", serde_json::json!({})),
        ("segment.meanshift", "default", serde_json::json!({})),
        ("segment.threshold", "otsu-3", serde_json::json!({ "classes": 3 })),
        ("segment.threshold", "otsu-5-1024", serde_json::json!({ "classes": 5, "bins": 1024 })),
        ("segment.threshold", "given-4", serde_json::json!({ "thresholds": [40, 80, 120, 160] })),
        ("segment.spectral_difference", "chess4-d8", serde_json::json!({ "from": "chessboard", "max_difference": 8 })),
        ("segment.spectral_difference", "slic-d8", serde_json::json!({ "from": "slic", "max_difference": 8 })),
        ("segment.region_growing", "t0.02", serde_json::json!({ "threshold": 0.02 })),
        ("segment.region_growing", "t0.05-min20", serde_json::json!({ "threshold": 0.05, "minsize": 20 })),
        ("segment.region_growing", "t0.05-n8-manh", serde_json::json!({ "threshold": 0.05, "neighbours": 8, "similarity": "manhattan" })),
        ("segment.region_growing", "t0.05", serde_json::json!({ "threshold": 0.05 })),
        ("segment.region_growing", "t0.05-tile512", serde_json::json!({ "threshold": 0.05, "tile": 512 })),
        ("segment.region_growing", "t0.05-min20-tile512", serde_json::json!({ "threshold": 0.05, "minsize": 20, "tile": 512 })),
        ("segment.quickshift", "default", serde_json::json!({ "ratio": 10 })),
        ("segment.quickshift", "k3-d8", serde_json::json!({ "ratio": 10, "kernel_size": 3, "max_dist": 8 })),
        ("segment.optimize", "ms-5-scales", serde_json::json!({ "values": [10, 20, 30, 40, 50] })),
    ];
    println!("\n{:<26} {:<16} {:<10} {:>7} {:>10} {:>9} {:>18}", "segmenter", "variant", "image", "threads", "seconds", "objects", "hash");
    let list = |k: &str, d: &str| -> Vec<usize> {
        std::env::var(k).unwrap_or(d.into()).split(',').filter_map(|x| x.parse().ok()).collect()
    };
    let only = std::env::var("BENCH_ONLY").unwrap_or_default();
    let mut seen: Seen = Vec::new();
    for side in list("BENCH_SIDES", "1000,2000") {
        let img = image(side, side, 4, 7);
        for (op, variant, params) in &cases {
            if !op.contains(&only) {
                continue;
            }
            if skip.split(',').any(|s| !s.is_empty() && op.ends_with(s)) {
                continue;
            }
            for threads in list("BENCH_THREADS", "1,8") {
                let reps = list("BENCH_REPEAT", "1")[0];
                let (mut secs, mut n, mut hs) = (f64::MAX, 0, 0);
                for _ in 0..reps {
                    let (s, k, x) = run(op, &img, params.clone(), threads);
                    (secs, n, hs) = (secs.min(s), k, x);
                }
                println!("{op:<26} {variant:<16} {:<10} {threads:>7} {secs:>10.2} {n:>9} {hs:>18x}", format!("{side}x{side}x4"));
                seen.push((op.to_string(), variant.to_string(), side, threads, secs));
            }
        }
    }
    if std::env::var("BENCH_RATES").is_ok_and(|v| v == "1") {
        write_rates(&seen, list("BENCH_THREADS", "1,8").into_iter().max().unwrap_or(1));
    }
}
