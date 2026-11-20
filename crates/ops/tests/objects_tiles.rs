//! `segment.multiresolution` by tiles: the tiles segmented in parallel and the objects stitched at their borders.

use std::collections::VecDeque;
use std::sync::Arc;

use terraflux_ops::objects::Objects;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Stack, Value};
use terraflux_raster::Grid;

fn multires(image: Value, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    segment(
        if params.get("threshold").is_some() { "segment.region_growing" } else { "segment.multiresolution" },
        image,
        params,
    )
}

fn segment(op: &str, image: Value, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: [("image".to_string(), image)].into_iter().collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*s >> 11) as f64) / (1u64 << 53) as f64
}

/// A cheap normal-ish noise (sum of 4 uniforms, standard deviation about `sd`).
fn gauss(s: &mut u64, sd: f64) -> f64 {
    ((lcg(s) + lcg(s) + lcg(s) + lcg(s)) - 2.0) * sd * 3f64.sqrt()
}

fn map(w: usize, h: usize, data: Vec<f64>) -> Value {
    Value::Map(Map::new(Arc::new(Grid::new(w, h)), data))
}

/// Bands of `level(row, column)` plus noise.
fn image(w: usize, h: usize, bands: usize, sd: f64, seed: u64, level: impl Fn(usize, usize) -> f64) -> Value {
    let mut s = seed;
    let keys = (0..bands).map(|k| terraflux_ops::Key::Name(format!("b{}", k + 1))).collect();
    let layers = (0..bands)
        .map(|b| Arc::new((0..w * h).map(|i| level(i / w, i % w) + b as f64 + gauss(&mut s, sd)).collect()))
        .collect();
    Value::Stack(Stack { grid: Arc::new(Grid::new(w, h)), keys, layers })
}

/// Each object is one connected set of cells (4 neighbours).
fn connected(o: &Objects) {
    let (w, h) = (o.grid.width, o.grid.height);
    let mut seen = vec![false; w * h];
    let mut comps = vec![0usize; o.count];
    for start in 0..w * h {
        let l = o.labels[start];
        if l == 0 || seen[start] {
            continue;
        }
        comps[l as usize - 1] += 1;
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(i) = queue.pop_front() {
            let (r, c) = (i / w, i % w);
            let mut go = |j: usize| {
                if !seen[j] && o.labels[j] == l {
                    seen[j] = true;
                    queue.push_back(j);
                }
            };
            if r > 0 {
                go(i - w);
            }
            if r + 1 < h {
                go(i + w);
            }
            if c > 0 {
                go(i - 1);
            }
            if c + 1 < w {
                go(i + 1);
            }
        }
    }
    assert!(comps.iter().all(|&c| c == 1), "an object in parts");
}

/// Blocks of `side` x `side` cells in five levels, none next to one of its own level, noise of `sd`.
fn blocks(w: usize, h: usize, side: usize, sd: f64) -> Value {
    image(w, h, 2, sd, 4, |r, c| 100.0 * (((r / side) + 2 * (c / side)) % 5) as f64)
}

#[test]
fn tiled_on_a_homogeneous_image_is_the_untiled_result() {
    // two areas apart (a column of no data), constant values
    let (w, h) = (101, 70);
    let data: Vec<f64> = (0..w * h).map(|i| if i % w == 50 { f64::NAN } else { 7.0 }).collect();
    // (with shape 0 every cost is 0, every cell's best neighbour a tie: the merging is slow, hundreds of passes)
    for (shape, scale) in [(0.1, 20), (0.3, 300)] {
        let p = |tile: i64| serde_json::json!({ "scale": scale, "shape": shape, "tile": tile, "margin": 8 });
        let whole = objects_of(&multires(map(w, h, data.clone()), p(0)).unwrap());
        assert_eq!(whole.count, 2);
        for tile in [16, 32, 64] {
            let tiled = objects_of(&multires(map(w, h, data.clone()), p(tile)).unwrap());
            assert_eq!(tiled.count, 2, "tile {tile} shape {shape}");
            assert_eq!(tiled.labels, whole.labels, "tile {tile} shape {shape}");
        }
    }
}

#[test]
fn tiled_blocks_across_the_tile_borders_stay_whole() {
    // blocks of 27 cells: their borders (27, 54, 81, 108) are not the tiles' (64, 128)
    let (w, h, side) = (135, 135, 27);
    let img = blocks(w, h, side, 1.0);
    let p = |tile: i64| serde_json::json!({ "scale": 60, "shape": 0.0, "tile": tile, "margin": 16 });
    // (the merging leaves a few stray fragments of a few cells here and there, untiled too: so a block is whole when
    // 70% of its cells are in one object (a cut at a tile border leaves at most 63%), and no two have the same one)
    let whole = |o: &Objects, what: &str| {
        let mut count = vec![vec![0u32; o.count + 1]; 25];
        for (i, &l) in o.labels.iter().enumerate() {
            count[(i / w / side) * 5 + (i % w) / side][l as usize] += 1;
        }
        let mains: Vec<usize> = count
            .iter()
            .enumerate()
            .map(|(b, c)| {
                let (main, n) = c.iter().enumerate().max_by_key(|&(_, &n)| n).unwrap();
                assert!(*n as f64 >= 0.7 * (side * side) as f64, "{what}: block {b} cut, {n} cells in its main object");
                main
            })
            .collect();
        let mut ids = mains.clone();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 25, "{what}: two blocks in one object");
    };
    whole(&objects_of(&multires(img.clone(), p(0)).unwrap()), "untiled");
    for tile in [64, 40] {
        let o = objects_of(&multires(img.clone(), p(tile)).unwrap());
        connected(&o);
        whole(&o, &format!("tile {tile}"));
    }
}

#[test]
fn tiled_is_deterministic_whatever_the_threads() {
    let img = image(150, 120, 3, 4.0, 11, |r, c| 20.0 * (((r / 23) + 2 * (c / 31)) % 4) as f64);
    let p = serde_json::json!({ "scale": 25, "seed": 5, "tile": 48, "margin": 12 });
    let a = objects_of(&multires(img.clone(), p.clone()).unwrap());
    let b = objects_of(&multires(img.clone(), p.clone()).unwrap());
    assert_eq!(a.labels, b.labels);
    for threads in [1, 3] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let o = pool.install(|| objects_of(&multires(img.clone(), p.clone()).unwrap()));
        assert_eq!(o.labels, a.labels, "{threads} threads");
        assert_eq!(o.cells, a.cells);
    }
    let other =
        objects_of(&multires(img, serde_json::json!({ "scale": 25, "seed": 6, "tile": 48, "margin": 12 })).unwrap());
    connected(&a);
    connected(&other);
}

#[test]
fn tiled_objects_are_connected_and_the_table_matches() {
    // noise strong enough that the objects are small and irregular; some cells without a value
    let mut img = image(90, 77, 2, 6.0, 3, |r, c| 15.0 * ((r / 19 + c / 13) % 3) as f64);
    if let Value::Stack(s) = &mut img {
        for l in s.layers.iter_mut() {
            let v = Arc::make_mut(l);
            for i in (0..v.len()).step_by(37) {
                v[i] = f64::NAN;
            }
        }
    }
    let out = multires(img, serde_json::json!({ "scale": 30, "tile": 32, "margin": 10 })).unwrap();
    let o = objects_of(&out);
    connected(&o);
    let Value::Table(t) = &out["table"] else { panic!("table") };
    assert_eq!(t.rows.len(), o.count);
    assert_eq!(t.columns, ["id", "cells", "area", "mean_b1", "mean_b2", "sd_b1", "sd_b2"]);
    let cells: f64 = t.rows.iter().map(|r| r[1]).sum();
    assert_eq!(cells as usize, o.labels.iter().filter(|&&l| l != 0).count());
}

#[test]
fn no_tile_on_a_small_image_is_the_old_result() {
    let img = image(80, 60, 2, 5.0, 8, |r, c| 25.0 * ((r / 20 + c / 20) % 3) as f64);
    let base = objects_of(&multires(img.clone(), serde_json::json!({ "scale": 18, "seed": 2 })).unwrap());
    // automatic (0), and a tile as big as the image, are the whole image at once
    for tile in [0i64, 80, 1000] {
        let o =
            objects_of(&multires(img.clone(), serde_json::json!({ "scale": 18, "seed": 2, "tile": tile })).unwrap());
        assert_eq!(o.labels, base.labels, "tile {tile}");
    }
    for bad in [-1i64, 3] {
        assert!(multires(img.clone(), serde_json::json!({ "scale": 18, "tile": bad })).is_err());
    }
    assert!(multires(img, serde_json::json!({ "scale": 18, "tile": 32, "margin": -2 })).is_err());
}

/// The share of cells whose "is on an object's border" (a neighbour to the right or below in another object) is the
/// same in both.
fn boundary_agreement(a: &Objects, b: &Objects) -> f64 {
    let (w, h) = (a.grid.width, a.grid.height);
    let edge = |o: &Objects, i: usize| {
        let (r, c) = (i / w, i % w);
        (c + 1 < w && o.labels[i + 1] != o.labels[i]) || (r + 1 < h && o.labels[i + w] != o.labels[i])
    };
    (0..w * h).filter(|&i| edge(a, i) == edge(b, i)).count() as f64 / (w * h) as f64
}

/// 4000 x 4000 cells, 4 bands, blocks and noise: untiled on one thread against tiled on three (cargo test --release
/// ... --ignored --nocapture).
#[test]
#[ignore]
fn tiled_timing_on_sixteen_million_cells() {
    let (w, h) = (4000, 4000);
    let img = image(w, h, 4, 0.02, 1, |r, c| 0.05 + 0.1 * (((r / 50) + 3 * (c / 50)) % 7) as f64);
    let p = |tile: i64| serde_json::json!({ "scale": 20, "tile": tile, "margin": 0, "max_passes": 100 });
    let one = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let t = std::time::Instant::now();
    let whole = one.install(|| objects_of(&multires(img.clone(), p(w as i64 + 1)).unwrap()));
    let t_whole = t.elapsed();
    println!("untiled, 1 thread: {} objects in {t_whole:.2?}", whole.count);
    let three = rayon::ThreadPoolBuilder::new().num_threads(3).build().unwrap();
    let t = std::time::Instant::now();
    let tiled = three.install(|| objects_of(&multires(img.clone(), p(1024)).unwrap()));
    let t_tiled = t.elapsed();
    println!("tiles of 1024, 3 threads: {} objects in {t_tiled:.2?}", tiled.count);
    println!(
        "speedup {:.2}, boundary agreement {:.4}",
        t_whole.as_secs_f64() / t_tiled.as_secs_f64(),
        boundary_agreement(&whole, &tiled)
    );
}

/// Voronoi areas (one site per ~`area` cells) with a level each, a texture and noise of `sd`: objects of every size.
fn voronoi(w: usize, h: usize, bands: usize, area: usize, sd: f64, seed: u64) -> Value {
    let mut s = seed;
    let sites: Vec<(f64, f64, f64)> = (0..(w * h / area).max(4))
        .map(|_| (lcg(&mut s) * h as f64, lcg(&mut s) * w as f64, lcg(&mut s) * 200.0))
        .collect();
    let class: Vec<usize> = (0..w * h)
        .map(|i| {
            let d = |p: &(f64, f64, f64)| (p.0 - (i / w) as f64).powi(2) + (p.1 - (i % w) as f64).powi(2);
            (0..sites.len()).min_by(|&a, &b| d(&sites[a]).total_cmp(&d(&sites[b]))).unwrap()
        })
        .collect();
    let keys = (0..bands).map(|k| terraflux_ops::Key::Name(format!("b{}", k + 1))).collect();
    let layers = (0..bands)
        .map(|b| {
            Arc::new(
                (0..w * h)
                    .map(|i| {
                        let (r, c) = ((i / w) as f64, (i % w) as f64);
                        let tex = 6.0 * ((r * 0.35 + b as f64).sin() * (c * 0.27).cos());
                        sites[class[i]].2 * (1.0 + 0.1 * b as f64) + tex + gauss(&mut s, sd)
                    })
                    .collect(),
            )
        })
        .collect();
    Value::Stack(Stack { grid: Arc::new(Grid::new(w, h)), keys, layers })
}

/// A smooth field (waves of a few dozen cells) with noise of `sd`, as a photograph: no flat areas to merge.
fn smooth(w: usize, h: usize, bands: usize, sd: f64, seed: u64) -> Value {
    image(w, h, bands, sd, seed, |r, c| {
        let (r, c) = (r as f64, c as f64);
        60.0 * ((r * 0.031 + 1.0).sin() * (c * 0.043 + 2.0).cos()
            + 0.5 * ((r + c) * 0.09).sin()
            + 0.3 * (c * 0.21).cos())
    })
}

/// The share of pairs of neighbouring cells in different objects, across the tile borders (the lines `t`, `2t`, ...,
/// rows and columns) and along the other lines.
fn cut_share(o: &Objects, t: usize) -> (f64, f64) {
    let (w, h) = (o.grid.width, o.grid.height);
    let (mut b, mut nb, mut n, mut nn) = (0usize, 0usize, 0usize, 0usize);
    for r in 0..h {
        for c in 0..w {
            let i = r * w + c;
            if c + 1 < w {
                let cut = o.labels[i] != o.labels[i + 1];
                if (c + 1) % t == 0 {
                    (n, b) = (n + 1, b + cut as usize);
                } else {
                    (nn, nb) = (nn + 1, nb + cut as usize);
                }
            }
            if r + 1 < h {
                let cut = o.labels[i] != o.labels[i + w];
                if (r + 1) % t == 0 {
                    (n, b) = (n + 1, b + cut as usize);
                } else {
                    (nn, nb) = (nn + 1, nb + cut as usize);
                }
            }
        }
    }
    (b as f64 / n as f64, nb as f64 / nn as f64)
}

/// The boundaries (pairs of neighbouring cells in different objects) of `a` against those of `b`: F = 2PR/(P+R) of
/// the pairs, a boundary matching one of the other within one cell counting.
fn boundary_f(a: &Objects, b: &Objects) -> f64 {
    let (w, h) = (a.grid.width, a.grid.height);
    // a cell is on a boundary when a neighbour to the right or below is in another object
    let edge = |o: &Objects| -> Vec<bool> {
        (0..w * h)
            .map(|i| {
                let (r, c) = (i / w, i % w);
                (c + 1 < w && o.labels[i + 1] != o.labels[i]) || (r + 1 < h && o.labels[i + w] != o.labels[i])
            })
            .collect()
    };
    let (ea, eb) = (edge(a), edge(b));
    let near = |e: &[bool], i: usize| {
        let (r, c) = (i / w, i % w);
        (r.saturating_sub(1)..(r + 2).min(h)).any(|rr| (c.saturating_sub(1)..(c + 2).min(w)).any(|cc| e[rr * w + cc]))
    };
    let hit = |x: &[bool], y: &[bool]| (0..w * h).filter(|&i| x[i] && near(y, i)).count() as f64;
    let (p, r) = (
        hit(&ea, &eb) / ea.iter().filter(|&&x| x).count().max(1) as f64,
        hit(&eb, &ea) / eb.iter().filter(|&&x| x).count().max(1) as f64,
    );
    2.0 * p * r / (p + r)
}

/// No seam: the pairs of cells across a tile border are in different objects about as often as along any other line,
/// the objects are as many as the untiled run's (give or take what another seed makes of them), and their boundaries
/// match the untiled ones as well as another seed's do. Before the pieces at the borders were dissolved, 36-44 % of
/// the pairs across the borders were cut against 22-27 % elsewhere.
fn no_seam(op: &str, key: &str, value: f64) {
    let (w, t) = (192, 64);
    let img = smooth(w, w, 3, 4.0, 7);
    let p = |tile: i64, seed: i64| serde_json::json!({ (key): value, "tile": tile, "margin": 16, "seed": seed });
    let run = |tile: i64, seed: i64| objects_of(&segment(op, img.clone(), p(tile, seed)).unwrap());
    let (whole, other, tiled) = (run(0, 1), run(0, 2), run(t as i64, 1));
    connected(&tiled);
    let (border, elsewhere) = cut_share(&tiled, t);
    let (f_tiled, f_other) = (boundary_f(&whole, &tiled), boundary_f(&whole, &other));
    println!(
        "{op} {value}: {} objects (whole {}, seed 2 {}), cut {border:.3} on borders, {elsewhere:.3} elsewhere, F {f_tiled:.3} (seed 2 {f_other:.3})",
        tiled.count, whole.count, other.count
    );
    assert!(
        border < elsewhere * 1.2,
        "{op}: {border:.3} of the pairs across the borders cut, {elsewhere:.3} elsewhere"
    );
    let (lo, hi) = (whole.count.min(other.count) as f64, whole.count.max(other.count) as f64);
    assert!(
        tiled.count as f64 > 0.9 * lo && (tiled.count as f64) < 1.1 * hi,
        "{op}: {} objects of {}",
        tiled.count,
        whole.count
    );
    assert!(f_tiled > f_other - 0.03, "{op}: boundaries F {f_tiled:.3} against {f_other:.3} of another seed");
}

#[test]
fn tiled_multiresolution_has_no_seam() {
    no_seam("segment.multiresolution", "scale", 16.0);
}

#[test]
fn tiled_region_growing_has_no_seam() {
    no_seam("segment.region_growing", "threshold", 0.03);
}

#[test]
#[ignore]
fn seam_report() {
    let (w, h, t) = (512, 512, 128);
    let img = if std::env::var("SEAM_IMAGE").is_ok_and(|v| v == "smooth") {
        smooth(w, h, 4, 4.0, 7)
    } else {
        voronoi(w, h, 4, 3000, 5.0, 7)
    };
    let scales: Vec<f64> =
        std::env::var("SEAM_SCALES").unwrap_or("10,20,40".into()).split(',').map(|x| x.parse().unwrap()).collect();
    let key = if std::env::var("SEAM_OP").is_ok_and(|v| v == "growing") { "threshold" } else { "scale" };
    for scale in scales {
        let p = |tile: i64, margin: i64| serde_json::json!({ (key): scale, "tile": tile, "margin": margin, "seed": 1 });
        let whole = objects_of(&multires(img.clone(), p(0, 0)).unwrap());
        let other = objects_of(&multires(img.clone(), serde_json::json!({ (key): scale, "seed": 2 })).unwrap());
        let (_, wn) = cut_share(&whole, t);
        println!(
            "scale {scale}: whole {} objects (seed 2: {}), cut on the tile lines {:.3} (others {wn:.3})",
            whole.count,
            other.count,
            cut_share(&whole, t).0
        );
        for margin in [0, 16] {
            let t0 = std::time::Instant::now();
            let o = objects_of(&multires(img.clone(), p(t as i64, margin)).unwrap());
            let secs = t0.elapsed().as_secs_f64();
            let (b, n) = cut_share(&o, t);
            println!(
                "  margin {margin}: {} objects, cut {b:.3} on borders, {n:.3} elsewhere, F vs whole {:.3} (seed 2 vs whole {:.3}), {secs:.2} s",
                o.count,
                boundary_f(&whole, &o),
                boundary_f(&whole, &other)
            );
        }
    }
}

fn peak_mb() -> f64 {
    super::objects_big::peak_bytes().map_or(0.0, |b| b as f64 / 1e6)
}

/// The seam and the time on a big image: SEAM_SIDE (2000), SEAM_THREADS (1), SEAM_SCALE (16), tiles of 512, and the
/// peak memory of the process (so run one case a time).
#[test]
#[ignore]
fn seam_bench() {
    let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let (side, threads, scale) = (env("SEAM_SIDE", 2000), env("SEAM_THREADS", 1), env("SEAM_SCALE", 16));
    let tile = env("SEAM_TILE", 512);
    let img = smooth(side, side, 4, 4.0, 7);
    println!("image built, peak {:.0} MB", peak_mb());
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let run = |tile: i64| {
        let p = serde_json::json!({ "scale": scale, "tile": tile, "seed": 1 });
        let t0 = std::time::Instant::now();
        let o = pool.install(|| objects_of(&multires(img.clone(), p).unwrap()));
        (o, t0.elapsed().as_secs_f64())
    };
    let (tiled, secs) = run(tile as i64);
    let (b, n) = cut_share(&tiled, tile);
    println!(
        "tiled {tile}, {threads} threads: {} objects, cut {b:.3} on borders, {n:.3} elsewhere, {secs:.2} s, peak {:.0} MB",
        tiled.count,
        peak_mb()
    );
    if std::env::var_os("SEAM_WHOLE").is_some() {
        let (whole, secs) = run(0);
        println!(
            "whole: {} objects, {secs:.2} s, F {:.3}, peak {:.0} MB",
            whole.count,
            boundary_f(&whole, &tiled),
            peak_mb()
        );
    }
}
