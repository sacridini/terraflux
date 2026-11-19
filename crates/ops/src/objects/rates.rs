//! The time a segmenter takes, estimated before it runs, from the rates of the benchmark battery
//! (`crates/ops/tests/objects_bench.rs`, run with `BENCH_RATES=1`, which writes `rates.json`).
//!
//! Per segmenter the file holds the seconds of a run of 1000² and of 2000² cells (4 bands) on 1 thread and on N. Time
//! is not linear in the cells (multiresolution: 4 times the cells, 8 times the seconds), so the exponent alpha comes
//! from the two sizes, and the gain of the threads from the two thread counts (Amdahl's law). The tiled variants
//! (`tile`) are modelled as tiles x the time of one tile (tile + 2 margins)² on a thread / the threads, plus the
//! stitch, which the tiled run of the battery gives. What the estimate ignores: the image's content (how much there is
//! to merge), the settings of a segmenter (region growing's `threshold` takes 3.7 s at 0.05 and 2.2 s at 0.02 on
//! 1000², but 17 s and 41 s on 2000²: no one factor fits; multiresolution's `scale` 20 against 50 changes the time by
//! 3%), the bands beyond a rough linear share, the objects given (`from`, `seeds`, `bounds`) and the machine (the
//! rates are one machine's: the caller corrects them by what it sees). Only `tile` and `margin` are read.

use std::sync::OnceLock;

use serde_json::{Map, Value};

const RATES: &str = include_str!("rates.json");

/// The cells above which multiresolution tiles by itself, and the side of its automatic tiles.
const AUTO_CELLS: f64 = 4_000_000.0;
const AUTO_TILE: usize = 1024;
/// The side of the tiles an alternative estimate is made for.
pub const SUGGESTED_TILE: usize = 512;

/// The segmenters of images the battery has rates for.
pub const SEGMENTERS: [&str; 12] = [
    "segment.multiresolution",
    "segment.slic",
    "segment.snic",
    "segment.felzenszwalb",
    "segment.watershed",
    "segment.meanshift",
    "segment.threshold",
    "segment.spectral_difference",
    "segment.region_growing",
    "segment.quickshift",
    "segment.chessboard",
    "segment.quadtree",
];

/// The seconds of a run of two sizes of image, on 1 thread and on N.
#[derive(Debug, Clone)]
struct Series {
    cells: [f64; 2],
    t1: [f64; 2],
    tn: [f64; 2],
}

impl Series {
    fn parse(v: &Value) -> Option<Series> {
        let two = |k: &str| -> Option<[f64; 2]> {
            let a = v.get(k)?.as_array()?;
            let (x, y) = (a.first()?.as_f64()?, a.get(1)?.as_f64()?);
            (x.is_finite() && y.is_finite() && x > 0.0 && y > 0.0).then_some([x, y])
        };
        Some(Series { cells: two("cells")?, t1: two("t1")?, tn: two("tn")? })
    }

    /// The exponent of the time against the cells, from the runs on one thread.
    fn alpha(&self) -> f64 {
        ((self.t1[1] / self.t1[0]).ln() / (self.cells[1] / self.cells[0]).ln()).clamp(0.5, 3.0)
    }

    /// The share of the time that the threads divide (Amdahl), from the speed-up at N threads.
    fn parallel(&self, n: f64) -> f64 {
        let speedup = self.t1[0] / self.tn[0];
        if n <= 1.0 || speedup <= 1.0 {
            return 0.0;
        }
        ((1.0 - 1.0 / speedup) / (1.0 - 1.0 / n)).clamp(0.0, 1.0)
    }

    /// The seconds of a run of `cells` on one thread.
    fn single(&self, cells: f64) -> f64 {
        self.t1[0] * (cells / self.cells[0]).powf(self.alpha())
    }
}

/// What the file says of a segmenter.
#[derive(Debug, Clone)]
struct Rate {
    whole: Series,
    /// The tiled run: its side, and the series of its runs.
    tiled: Option<(usize, Series)>,
}

struct Rates {
    threads: f64,
    by_op: Vec<(String, Rate)>,
}

fn rates() -> &'static Rates {
    static R: OnceLock<Rates> = OnceLock::new();
    R.get_or_init(|| {
        let v: Value = serde_json::from_str(RATES).unwrap_or(Value::Null);
        let threads = v.get("threads").and_then(Value::as_f64).unwrap_or(1.0).max(1.0);
        let mut by_op = Vec::new();
        if let Some(ops) = v.get("ops").and_then(Value::as_object) {
            for (op, r) in ops {
                let Some(whole) = Series::parse(r) else { continue };
                let tiled = r.get("tiled").and_then(|t| Some((t.get("tile")?.as_u64()? as usize, Series::parse(t)?)));
                by_op.push((op.clone(), Rate { whole, tiled }));
            }
        }
        Rates { threads, by_op }
    })
}

fn rate_of(op: &str) -> Option<&'static Rate> {
    rates().by_op.iter().find(|(o, _)| o == op).map(|(_, r)| r)
}

/// The exponent of the time against the cells of a segmenter, from the battery (for tests and for showing).
pub fn exponent(op: &str) -> Option<f64> {
    rate_of(op).map(|r| r.whole.alpha())
}

/// How long a segmenter takes, about.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    /// The seconds with the settings as they are.
    pub seconds: f64,
    /// The side of the tiles they make (0: the image whole, in one piece).
    pub tile: usize,
    /// The seconds with tiles of a side the operator takes, when `tile` is 0 and the operator has tiles.
    pub by_tiles: Option<(usize, f64)>,
}

fn number(params: &Map<String, Value>, k: &str) -> Option<f64> {
    params.get(k).and_then(Value::as_f64)
}

/// The margin of a tile: the given one, else 64 cells at most half the tile.
fn margin_of(tile: usize, margin: usize) -> f64 {
    (if margin > 0 { margin } else { (tile / 2).min(64) }) as f64
}

/// The tiles an image of `cells` cells is cut into by tiles of side `tile`, and the work of all of them on one thread:
/// each tile with its margins is a run of its own.
fn tile_work(whole: &Series, cells: f64, tile: usize, margin: usize) -> (f64, f64) {
    let tiles = (cells.sqrt().max(1.0) / tile as f64).ceil().powi(2).max(1.0);
    let side = tile as f64 + 2.0 * margin_of(tile, margin);
    (tiles, tiles * whole.single((side * side).min(cells)))
}

/// The seconds of a run of `cells` cells on `threads` threads, by tiles of side `tile` (0: whole).
fn seconds(r: &Rate, cells: f64, threads: f64, tile: usize, margin: usize) -> f64 {
    match (&r.tiled, tile) {
        (Some((t0, tiled)), t) if t > 0 => {
            // the tiled run of the battery, 2000² cells: the work of its tiles on one thread (w) that the threads
            // share, and what no thread shares (the stitch, s): t1 = w + s, tn = w / threads + s
            let (base, (tiles0, work0)) = (tiled.cells[1], tile_work(&r.whole, tiled.cells[1], *t0, 0));
            let shared = rates().threads.min(tiles0);
            let w = ((tiled.t1[1] - tiled.tn[1]) / (1.0 - 1.0 / shared).max(1e-9)).clamp(0.0, tiled.t1[1]);
            let (tiles, work) = tile_work(&r.whole, cells, t, margin);
            w * work / work0 / threads.min(tiles).max(1.0) + (tiled.t1[1] - w) * cells / base
        }
        _ => {
            let p = r.whole.parallel(rates().threads);
            r.whole.single(cells) * ((1.0 - p) + p / threads.max(1.0))
        }
    }
}

/// How long `op` takes on an image of `cells` cells and `bands` bands (0: not told) with the settings `params`, on
/// `threads` threads; none for an operator the battery has no rate for. See the top of the file for what it ignores.
pub fn estimate(op: &str, params: &Map<String, Value>, cells: u64, bands: usize, threads: usize) -> Option<Estimate> {
    let r = rate_of(op)?;
    let (cells, threads) = (cells.max(1) as f64, threads.max(1) as f64);
    let mut factor = 1.0;
    if op != "segment.chessboard" {
        let b = if bands == 0 { 4.0 } else { bands as f64 };
        factor *= 0.3 + 0.7 * b / 4.0;
    }
    let margin = number(params, "margin").map_or(0, |m| m.max(0.0) as usize);
    let asked = number(params, "tile").map_or(0, |t| t.max(0.0) as usize);
    let tile = match (&r.tiled, asked) {
        (None, _) => 0,
        (Some(_), t) if t > 0 => t,
        (Some(_), _) if op == "segment.multiresolution" && cells > AUTO_CELLS => AUTO_TILE,
        _ => 0,
    };
    let secs = seconds(r, cells, threads, tile, margin) * factor;
    let by_tiles = (tile == 0 && r.tiled.is_some())
        .then(|| (SUGGESTED_TILE, seconds(r, cells, threads, SUGGESTED_TILE, margin) * factor));
    Some(Estimate { seconds: secs, tile, by_tiles })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Map<String, Value> {
        Map::new()
    }

    #[test]
    fn every_image_segmenter_has_a_rate() {
        for op in SEGMENTERS {
            let e = estimate(op, &none(), 1_000_000, 4, 8).unwrap_or_else(|| panic!("{op} has no rate"));
            assert!(e.seconds.is_finite() && e.seconds > 0.0, "{op}: {}", e.seconds);
        }
        assert!(estimate("segment.sam", &none(), 1_000_000, 4, 8).is_none());
        assert!(estimate("raster.calc", &none(), 1_000_000, 4, 8).is_none());
    }

    #[test]
    fn the_time_grows_like_the_cells_to_the_exponent() {
        for op in SEGMENTERS {
            let alpha = exponent(op).unwrap();
            let one = estimate(op, &none(), 1_000_000, 4, 1).unwrap().seconds;
            let four = estimate(op, &none(), 4_000_000, 4, 1).unwrap().seconds;
            // (a multiresolution of more than 4 million cells is tiled by itself: not on this curve)
            if op == "segment.multiresolution" && estimate(op, &none(), 4_000_001, 4, 1).unwrap().tile > 0 {
                continue;
            }
            let want = 4f64.powf(alpha);
            assert!(((four / one) / want - 1.0).abs() < 0.02, "{op}: {} against {want} (alpha {alpha})", four / one);
        }
    }

    #[test]
    fn threads_do_not_slow_it_and_tiles_come_with_a_second_figure() {
        let growing = "segment.region_growing";
        let params = none();
        let one = estimate(growing, &params, 4_000_000, 4, 1).unwrap();
        let many = estimate(growing, &params, 4_000_000, 4, 8).unwrap();
        assert!(many.seconds <= one.seconds + 1e-9);
        // without tiles it is one thread's work: the threads gain nothing
        assert!((many.seconds / one.seconds - 1.0).abs() < 0.2);
        let (side, tiled) = many.by_tiles.expect("the tiled figure");
        assert_eq!(side, SUGGESTED_TILE);
        assert!(tiled < many.seconds, "{tiled} against {}", many.seconds);
        let mut asked = none();
        asked.insert("tile".into(), 512.into());
        let now = estimate(growing, &asked, 4_000_000, 4, 8).unwrap();
        assert_eq!((now.tile, now.by_tiles), (512, None));
        assert!((now.seconds - tiled).abs() < 1e-9);
        // a segmenter with no tiles has no second figure
        assert!(estimate("segment.slic", &params, 4_000_000, 4, 8).unwrap().by_tiles.is_none());
    }
}
