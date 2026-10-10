//! The tiled engine on Sibuyan: its fits are the exact engine's to the bit, its simulations the same with any
//! tiling (in memory or on disk), meet the demand, and match the exact engine's statistically.

use std::path::PathBuf;
use std::sync::Arc;

use terraflux_engine::dinamica::Settings;
use terraflux_engine::run::{Fitted, fit_dinamica, simulate};
use terraflux_engine::tiles::{FileSource, Source, Tiling};
use terraflux_ops::woe::Settings as WoeSettings;
use terraflux_ops::{Key, Map, Stack};
use terraflux_raster::tiff::Window;

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
}

struct Sibuyan {
    t0: FileSource,
    t1: FileSource,
    drivers: Vec<(String, FileSource)>,
}

fn sibuyan() -> Sibuyan {
    let lu = data("sibuyan_landuse.tif");
    Sibuyan {
        t0: FileSource::open(&lu, Some("1997"), None).unwrap(),
        t1: FileSource::open(&lu, Some("2011"), None).unwrap(),
        drivers: FileSource::bands(&data("sibuyan_drivers.tif"), None).unwrap(),
    }
}

fn whole(s: &dyn Source) -> Vec<f64> {
    let g = s.grid();
    s.read(Window { x0: 0, y0: 0, width: g.width, height: g.height }).unwrap()
}

fn woe_settings() -> WoeSettings {
    serde_json::from_value(serde_json::json!({"increment": 250.0})).unwrap()
}

fn fit(s: &Sibuyan, tile: usize) -> Fitted {
    let g = s.t0.grid();
    let tiling = Tiling::new(g.width, g.height, tile);
    let drivers: Vec<(String, &dyn Source)> = s.drivers.iter().map(|(n, d)| (n.clone(), d as &dyn Source)).collect();
    fit_dinamica(&s.t0, &s.t1, &drivers, &tiling, 14, &woe_settings(), None, 1.5).unwrap()
}

#[test]
fn fits_are_the_exact_engines() {
    let s = sibuyan();
    let g = Arc::new(s.t0.grid().clone());
    let t0 = Map::new(g.clone(), whole(&s.t0));
    let t1 = Map::new(g.clone(), whole(&s.t1));
    let stack = Stack {
        grid: g,
        keys: s.drivers.iter().map(|(n, _)| Key::Name(n.clone())).collect(),
        layers: s.drivers.iter().map(|(_, d)| Arc::new(whole(d))).collect(),
    };
    let markov = terraflux_ops::markov::fit(&t0, &t1, 14).unwrap();
    let woe = terraflux_ops::woe::fit(&t0, &t1, &stack, &woe_settings(), Some(markov.transitions())).unwrap();
    for tile in [17, 50, 1000] {
        let f = fit(&s, tile);
        assert_eq!(f.markov, markov, "tile {tile}");
        assert_eq!(f.woe, woe, "tile {tile}");
    }
    // the patches measured on one tile are the exact engine's
    let one = fit(&s, 1000).patches;
    let exact = terraflux_ops::dinamica::patch_params(&t0, &t1, &markov.transitions(), None, 1.5).unwrap();
    assert_eq!(one, exact);
}

/// The maps of `steps` steps with a tiling (in memory, or on disk in `dir`).
fn run(
    s: &Sibuyan,
    fitted: &Fitted,
    tile: usize,
    steps: usize,
    seed: u64,
    dir: Option<PathBuf>,
) -> (Vec<Vec<f64>>, Vec<Vec<terraflux_engine::dinamica::Done>>) {
    let g = s.t0.grid();
    let tiling = Tiling::new(g.width, g.height, tile);
    let drivers: Vec<(String, &dyn Source)> = s.drivers.iter().map(|(n, d)| (n.clone(), d as &dyn Source)).collect();
    let mut maps = Vec::new();
    let mut done = Vec::new();
    let settings = Settings { radius: 8, ..Default::default() };
    simulate(&tiling, &s.t0, &drivers, fitted, steps, seed, &settings, dir, |_, land, d| {
        maps.push(whole(land));
        done.push(d.to_vec());
        Ok(())
    })
    .unwrap();
    (maps, done)
}

#[test]
fn the_same_maps_with_any_tiling() {
    let s = sibuyan();
    let fitted = fit(&s, 64);
    let (reference, _) = run(&s, &fitted, 1000, 3, 7, None);
    for tile in [16, 37, 64] {
        let (maps, _) = run(&s, &fitted, tile, 3, 7, None);
        for (k, (a, b)) in maps.iter().zip(&reference).enumerate() {
            let differ = a.iter().zip(b).filter(|(x, y)| x != y && !(x.is_nan() && y.is_nan())).count();
            assert_eq!(differ, 0, "tile {tile}, step {}: {differ} cells differ from one tile", k + 1);
        }
    }
    let dir = std::env::temp_dir().join(format!("terraflux-tiled-{}", std::process::id()));
    let (on_disk, _) = run(&s, &fitted, 37, 3, 7, Some(dir.clone()));
    assert!(
        on_disk.iter().zip(&reference).all(|(a, b)| a.iter().zip(b).all(|(x, y)| x == y || (x.is_nan() && y.is_nan())))
    );
    let _ = std::fs::remove_dir_all(dir);
    // another seed, another simulation
    let (other, _) = run(&s, &fitted, 64, 1, 8, None);
    assert_ne!(other[0], reference[0]);
}

#[test]
fn the_demand_is_met() {
    let s = sibuyan();
    let fitted = fit(&s, 64);
    let (_, done) = run(&s, &fitted, 40, 5, 3, None);
    for (k, step) in done.iter().enumerate() {
        for d in step {
            let made = d.expanded + d.patched;
            let slack = (0.02 * d.requested as f64).max(3.0);
            assert!((made - d.requested).abs() as f64 <= slack, "step {}: {d:?}", k + 1);
        }
    }
}

#[test]
fn like_the_exact_engine() {
    // fourteen steps from 1997: the changes against 2011 as close as the exact engine's (fuzzy similarity of
    // the changes in a 5 x 5 window, the mean of three seeds), and the same areas of each class
    let s = sibuyan();
    let fitted = fit(&s, 1000);
    let g = Arc::new(s.t0.grid().clone());
    let t0 = Map::new(g.clone(), whole(&s.t0));
    let t1 = Map::new(g.clone(), whole(&s.t1));
    let stack = Stack {
        grid: g.clone(),
        keys: s.drivers.iter().map(|(n, _)| Key::Name(n.clone())).collect(),
        layers: s.drivers.iter().map(|(_, d)| Arc::new(whole(d))).collect(),
    };
    let (mut sim_tiled, mut sim_exact) = (0.0, 0.0);
    let mut areas = (vec![0.0; 6], vec![0.0; 6]);
    for seed in [1, 2, 3] {
        let (maps, _) = run(&s, &fitted, 40, 14, seed, None);
        let last = Map::new(g.clone(), maps.last().unwrap().clone());
        let t = terraflux_ops::validate::similarity_of_differences(&t0, &t1, &last, &[5], true).unwrap();
        sim_tiled += t.rows[0][3] / 3.0;
        // the exact engine: the same fitted pieces, 14 steps of dinamica.step
        let mut land = t0.clone();
        for k in 1..=14 {
            let p = terraflux_ops::woe::probability(&fitted.woe, &land, &stack).unwrap();
            let mut ctx = terraflux_ops::Ctx::default();
            let (next, _) = terraflux_ops::dinamica::step(
                &mut ctx,
                &land,
                &p,
                &fitted.markov,
                &fitted.patches,
                k,
                seed as i64,
                10.0,
                (3, 3),
            )
            .unwrap();
            land = next;
        }
        let e = terraflux_ops::validate::similarity_of_differences(&t0, &t1, &land, &[5], true).unwrap();
        sim_exact += e.rows[0][3] / 3.0;
        for c in 1..=5 {
            areas.0[c] += last.data.iter().filter(|v| **v == c as f64).count() as f64 / 3.0;
            areas.1[c] += land.data.iter().filter(|v| **v == c as f64).count() as f64 / 3.0;
        }
    }
    eprintln!("fuzzy similarity of the changes (5 x 5): tiled {sim_tiled:.4}, exact {sim_exact:.4}");
    eprintln!("areas (cells): tiled {:?}, exact {:?}", &areas.0[1..], &areas.1[1..]);
    assert!((sim_tiled - sim_exact).abs() < 0.05, "tiled {sim_tiled:.4}, exact {sim_exact:.4}");
    for c in 1..=5 {
        let (a, b) = (areas.0[c], areas.1[c]);
        assert!((a - b).abs() <= 0.02 * b.max(50.0), "class {c}: tiled {a}, exact {b}");
    }
}
