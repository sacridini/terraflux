//! A benchmark battery of the segmenters on fixed synthetic images (ignored: run it by hand, in release):
//! `cargo test -p terraflux-ops --release --test ops objects_bench -- --ignored --nocapture --test-threads=1`
//! It prints a table: segmenter, image, threads, seconds, objects. `BENCH_SKIP=meanshift,watershed` leaves some out,
//! `BENCH_ONLY=multiresolution` keeps only those, `BENCH_SIDES=1000` the sides of the images, `BENCH_THREADS=1`.

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

fn run(op: &str, img: &Value, params: serde_json::Value, threads: usize) -> (f64, usize) {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: [("image".to_string(), img.clone())].into_iter().collect(), params: &params };
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let t = Instant::now();
    let out = pool.install(|| r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap());
    let secs = t.elapsed().as_secs_f64();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    let o: &Arc<Objects> = o;
    (secs, o.count)
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
        ("segment.slic", "default", serde_json::json!({})),
        ("segment.snic", "default", serde_json::json!({})),
        ("segment.felzenszwalb", "default", serde_json::json!({})),
        ("segment.watershed", "default", serde_json::json!({})),
        ("segment.meanshift", "default", serde_json::json!({})),
    ];
    println!("\n{:<26} {:<16} {:<10} {:>7} {:>10} {:>9}", "segmenter", "variant", "image", "threads", "seconds", "objects");
    let list = |k: &str, d: &str| -> Vec<usize> {
        std::env::var(k).unwrap_or(d.into()).split(',').filter_map(|x| x.parse().ok()).collect()
    };
    let only = std::env::var("BENCH_ONLY").unwrap_or_default();
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
                let (mut secs, mut n) = (f64::MAX, 0);
                for _ in 0..reps {
                    let (s, k) = run(op, &img, params.clone(), threads);
                    (secs, n) = (secs.min(s), k);
                }
                println!("{op:<26} {variant:<16} {:<10} {threads:>7} {secs:>10.2} {n:>9}", format!("{side}x{side}x4"));
            }
        }
    }
}
