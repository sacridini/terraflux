//! A window of an image segmented on its own: what the Viewer's live segmentation runs while a slider moves (the part
//! of the image in view, not the whole), and the ESP curve over the same window.
//!
//! The window is read every `step`-th cell when it has too many for the answer to come in a second; the objects then
//! live on a coarser grid placed where the window is (its transform says so), so they draw over the image as they are.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_raster::Grid;

use super::Objects;
use super::esp::{Settings, esp, tables};
use super::growing::ranges_of;
use super::multiresolution::{Cost, merge_level};
use super::superpixels::band_stats;
use super::zones::zones_of;
use crate::raster_stats::Layers;
use crate::{Args, Ctx, Error, Key, Map, Outputs, Registry, Result, Stack, Table, Value};

/// A window of an image: the cells from column `x0` and row `y0`, `width` by `height`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    pub x0: usize,
    pub y0: usize,
    pub width: usize,
    pub height: usize,
}

impl Window {
    pub fn cells(&self) -> usize {
        self.width * self.height
    }

    /// Every how many cells the window is read so that it has no more than `most` of them (1: all).
    pub fn step_for(&self, most: usize) -> usize {
        let mut s = 1;
        while self.width.div_ceil(s) * self.height.div_ceil(s) > most.max(1) {
            s += 1;
        }
        s
    }

    /// The grid of the window read every `step`-th cell, placed where the window is on `grid`.
    pub fn grid(&self, grid: &Grid, step: usize) -> Grid {
        let t = grid.transform;
        let s = step as f64;
        Grid {
            width: self.width.div_ceil(step),
            height: self.height.div_ceil(step),
            transform: [
                t[0] + self.x0 as f64 * t[1] + self.y0 as f64 * t[2],
                t[1] * s,
                t[2] * s,
                t[3] + self.x0 as f64 * t[4] + self.y0 as f64 * t[5],
                t[4] * s,
                t[5] * s,
            ],
            geo: grid.geo.clone(),
        }
    }
}

/// The settings of a segmentation (as `segment.multiresolution`'s).
#[derive(Debug, Clone)]
pub struct WindowSettings {
    pub scale: f64,
    pub shape: f64,
    pub compactness: f64,
    /// The weight of each band (empty: 1 for all).
    pub weights: Vec<f64>,
    pub mutual: bool,
    pub seed: u64,
}

impl Default for WindowSettings {
    fn default() -> Self {
        WindowSettings { scale: 1.0, shape: 0.1, compactness: 0.5, weights: Vec::new(), mutual: true, seed: 0 }
    }
}

impl WindowSettings {
    fn cost(&self, nb: usize) -> Result<Cost> {
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err(Error::msg(format!("scale {}: above 0", self.scale)));
        }
        if !(0.0..=0.9).contains(&self.shape) {
            return Err(Error::msg(format!("shape {}: 0 to 0.9", self.shape)));
        }
        if !(0.0..=1.0).contains(&self.compactness) {
            return Err(Error::msg(format!("compactness {}: 0 to 1", self.compactness)));
        }
        let weights = if self.weights.is_empty() { vec![1.0; nb] } else { self.weights.clone() };
        if weights.len() != nb {
            return Err(Error::msg(format!("weights: {nb} numbers, one for each band")));
        }
        Ok(Cost { colour: 1.0 - self.shape, compact: self.compactness, weights })
    }
}

/// The grid of a window and its bands.
type Bands = (Arc<Grid>, Vec<Arc<Vec<f64>>>);

/// The bands of the window, read every `step`-th cell, and the grid they are on.
fn read(grid: &Grid, data: &[Arc<Vec<f64>>], win: Window, step: usize) -> Result<Bands> {
    let (w, h) = (grid.width, grid.height);
    if step == 0 || win.width == 0 || win.height == 0 || win.x0 + win.width > w || win.y0 + win.height > h {
        return Err(Error::msg(format!(
            "window {}x{} at ({}, {}) is not inside the {w}x{h} image (step {step})",
            win.width, win.height, win.x0, win.y0
        )));
    }
    let g = win.grid(grid, step);
    let bands = data
        .iter()
        .map(|b| {
            let mut v = Vec::with_capacity(g.width * g.height);
            for r in 0..g.height {
                let row = (win.y0 + r * step) * w + win.x0;
                v.extend((0..g.width).map(|c| b[row + c * step]));
            }
            Arc::new(v)
        })
        .collect();
    Ok((Arc::new(g), bands))
}

/// The multiresolution segmentation of a window of an image (`data`, a band each, on `grid`), read every `step`-th
/// cell: the objects on the window's grid, no table. With `step` 1 the cost is the whole image's on those cells, so
/// where the window is aligned with whole objects the objects are the same as in the whole image's segmentation.
pub fn segment_window(
    grid: &Grid,
    data: &[Arc<Vec<f64>>],
    win: Window,
    step: usize,
    p: &WindowSettings,
) -> Result<Objects> {
    segment_window_bounds(grid, data, win, step, p, None)
}

/// [`segment_window`] with the `bounds` of the node (a map, vectors or objects of the whole image) cut to the
/// window: no object crosses a zone.
pub fn segment_window_bounds(
    grid: &Grid,
    data: &[Arc<Vec<f64>>],
    win: Window,
    step: usize,
    p: &WindowSettings,
    bounds: Option<&Value>,
) -> Result<Objects> {
    let cost = p.cost(data.len())?;
    let (g, bands) = read(grid, data, win, step)?;
    let zone = match bounds {
        None => Vec::new(),
        Some(v) => zones_of(&cut("bounds", v, grid, win, step, &g)?, &g, "bounds")?,
    };
    Ok(merge_level(&g, &bands, &cost, p.scale, p.mutual, p.seed, 100, None, &zone).objects)
}

/// The table of ESP (as `segment.scale`'s `table`: scale, objects, lv_, roc_, roc_mean, suggested) over a window read
/// every `step`-th cell, over `levels` scales from the image's own start.
#[allow(clippy::too_many_arguments)]
pub fn esp_window(
    grid: &Grid,
    data: &[Arc<Vec<f64>>],
    names: &[String],
    win: Window,
    step: usize,
    levels: usize,
    p: &WindowSettings,
) -> Result<Table> {
    let cost = p.cost(data.len()).or_else(|_| WindowSettings { scale: 1.0, ..p.clone() }.cost(data.len()))?;
    let (g, bands) = read(grid, data, win, step)?;
    let names: Vec<String> = (0..bands.len()).map(|k| names.get(k).cloned().unwrap_or_default()).collect();
    let l = Layers { grid: g, names: names.clone(), data: bands };
    let set = Settings { start: None, step: None, levels: levels.max(3), window: 0, seed: p.seed };
    let run = esp(&l, &cost, &set)?;
    Ok(tables(&run, &names).0)
}

/// What the whole image gives a window of it, so that the window's objects are the whole image's: the mean and the
/// standard deviation of each band (the z-scores of the segmenters that standardise) and its range (region growing's
/// 0-1 scaling). Made once for an image.
#[derive(Debug, Clone)]
pub struct Whole {
    stats: Vec<(f64, f64)>,
    ranges: Vec<[f64; 2]>,
}

impl Whole {
    pub fn of(data: &[Arc<Vec<f64>>]) -> Whole {
        Whole { stats: band_stats(data), ranges: ranges_of(data) }
    }
}

/// The segmenters `segment_window_op` runs, and whether each standardises its bands (the others read the values).
const OPS: [(&str, bool); 10] = [
    ("segment.slic", true),
    ("segment.snic", true),
    ("segment.felzenszwalb", true),
    ("segment.watershed", true),
    ("segment.quickshift", true),
    ("segment.meanshift", true),
    ("segment.threshold", false),
    ("segment.spectral_difference", false),
    ("segment.region_growing", false),
    ("segment.multiresolution", false),
];

/// The settings that count cells or are measured over them, as the reading step changes them: the setting, its
/// default (none: left alone when not given), the power of the step it is multiplied by (-1: a length in cells, -2: an
/// area in cells, 1: a gradient per cell, 2: a gradient per cell times a distance in cells) and whether it is a count.
const STEPPED: [(&str, &str, Option<f64>, i32, bool); 14] = [
    ("segment.slic", "size", Some(10.0), -1, true),
    ("segment.snic", "size", Some(10.0), -1, true),
    ("segment.felzenszwalb", "sigma", Some(0.8), -1, false),
    ("segment.felzenszwalb", "k", Some(1.0), -2, false),
    ("segment.felzenszwalb", "min_size", Some(20.0), -2, true),
    ("segment.watershed", "sigma", Some(1.0), -1, false),
    ("segment.watershed", "h", None, 1, false),
    ("segment.watershed", "compactness", None, 2, false),
    ("segment.quickshift", "kernel_size", Some(5.0), -1, false),
    ("segment.quickshift", "max_dist", Some(10.0), -1, false),
    ("segment.quickshift", "sigma", None, -1, false),
    ("segment.meanshift", "spatial_radius", Some(5.0), -1, false),
    ("segment.meanshift", "min_size", Some(20.0), -2, true),
    ("segment.region_growing", "minsize", Some(1.0), -2, true),
];

/// Whether `segment_window_op` runs the segmenter `op`.
pub fn window_op(op: &str) -> bool {
    OPS.iter().any(|(o, _)| *o == op)
}

type Params = serde_json::Map<String, serde_json::Value>;

/// The settings `p` of `op` for a window read every `step`-th cell: the ones in cells divided by the step (a count
/// stays a whole number of at least 1), so the objects have the size they have in the whole image.
fn stepped(op: &str, p: &mut Params, step: usize) {
    if step <= 1 {
        return;
    }
    let s = step as f64;
    for &(_, name, default, power, count) in STEPPED.iter().filter(|e| e.0 == op) {
        // (a lattice given by its count has no size to scale)
        if name == "size" && p.contains_key("count") {
            continue;
        }
        let Some(v) = p.get(name).and_then(|v| v.as_f64()).or(default) else { continue };
        let v = v * s.powi(power);
        let v = if count { serde_json::Value::from(v.round().max(1.0) as i64) } else { serde_json::Value::from(v) };
        p.insert(name.to_string(), v);
    }
}

/// The part of an input of the whole image (objects, a map or vectors on its grid) that lies in the window read every
/// `step`-th cell, on the window's grid `g`.
fn cut(name: &str, v: &Value, grid: &Grid, win: Window, step: usize, g: &Arc<Grid>) -> Result<Value> {
    let bad = |m: &str| Error::msg(format!("{name}: {m}"));
    let pick = |i: usize| (win.y0 + (i / g.width) * step) * grid.width + win.x0 + (i % g.width) * step;
    match v {
        Value::Map(m) if m.grid.same_as(grid) => {
            Ok(Value::Map(Map::new(g.clone(), (0..g.cells()).map(|i| m.data[pick(i)]).collect())))
        }
        Value::Objects(o) if o.grid.same_as(grid) => {
            let labels: Vec<u32> = (0..g.cells()).map(|i| o.labels[pick(i)]).collect();
            Ok(Value::Objects(Arc::new(Objects::from_fn(g.clone(), |i| labels[i], 0u32, 4))))
        }
        // (vectors are burnt on the whole grid, one number for each feature, and cut as a map)
        Value::Vector(_) => {
            let zone = zones_of(v, grid, name)?;
            let data = (0..g.cells()).map(|i| f64::from_bits(zone[pick(i)])).collect();
            Ok(Value::Map(Map::new(g.clone(), data)))
        }
        Value::Map(_) | Value::Objects(_) => Err(bad("not on the grid of the image")),
        _ => Err(bad("objects, a map or vectors, to be cut to the window")),
    }
}

/// A segmenter of the registry (`segment.slic`, `snic`, `felzenszwalb`, `watershed`, `quickshift`, `meanshift`,
/// `threshold`, `spectral_difference` or `region_growing`) run on a window of an image read every `step`-th cell: the
/// objects on the window's grid. `params` are the node's settings for the whole image (the ones in cells are divided
/// by `step`); `extra` the inputs besides the image (`objects`, `markers`, `seeds`, `bounds`), of the whole image and
/// cut to the window here. For the window to give the whole image's objects, the segmenters that standardise get
/// the window already in the z-scores of the whole image (and `standardise` off), region growing the whole image's
/// ranges, and the threshold the thresholds of the whole image (found on the image read every few cells).
#[allow(clippy::too_many_arguments)]
pub fn segment_window_op(
    registry: &Registry,
    op: &str,
    params: &Params,
    grid: &Grid,
    data: &[Arc<Vec<f64>>],
    names: &[String],
    whole: &Whole,
    win: Window,
    step: usize,
    extra: &[(String, Value)],
) -> Result<Arc<Objects>> {
    let out = run_window(registry, op, params, grid, data, names, whole, win, step, extra)?;
    match out.get("objects") {
        Some(Value::Objects(o)) => Ok(o.clone()),
        _ => Err(Error::msg(format!("{op} gave no objects"))),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_window(
    registry: &Registry,
    op: &str,
    params: &Params,
    grid: &Grid,
    data: &[Arc<Vec<f64>>],
    names: &[String],
    whole: &Whole,
    win: Window,
    step: usize,
    extra: &[(String, Value)],
) -> Result<Outputs> {
    let Some(&(_, standardises)) = OPS.iter().find(|(o, _)| *o == op) else {
        return Err(Error::msg(format!("{op}: not a segmenter that runs on a window")));
    };
    let operator = registry.get(op).ok_or_else(|| Error::msg(format!("{op}: no such operator")))?;
    let (g, mut bands) = read(grid, data, win, step)?;
    let mut p = params.clone();
    stepped(op, &mut p, step);
    if standardises {
        bands = bands
            .iter()
            .zip(&whole.stats)
            .map(|(b, &(mean, sd))| {
                let scale = if sd > 0.0 { 1.0 / sd } else { 1.0 };
                Arc::new(b.iter().map(|&v| if v.is_nan() { v } else { (v - mean) * scale }).collect::<Vec<f64>>())
            })
            .collect();
        p.insert("standardise".into(), serde_json::Value::Bool(false));
    }
    if op == "segment.region_growing" && !p.contains_key("ranges") {
        p.insert("ranges".into(), serde_json::json!(whole.ranges));
    }
    let every = Window { x0: 0, y0: 0, width: grid.width, height: grid.height };
    if op == "segment.threshold" && !p.contains_key("thresholds") && win != every {
        // the classes' cuts are the whole image's, not the window's
        let out = run_window(registry, op, params, grid, data, names, whole, every, every.step_for(1_000_000), &[])?;
        let Some(Value::Table(t)) = out.get("thresholds") else {
            return Err(Error::msg("segment.threshold gave no thresholds"));
        };
        p.remove("classes");
        p.insert("thresholds".into(), serde_json::json!(t.rows.iter().map(|r| r[0]).collect::<Vec<f64>>()));
    }
    let keys = (0..bands.len())
        .map(|k| Key::Name(names.get(k).cloned().unwrap_or_else(|| format!("band{}", k + 1))))
        .collect();
    let mut inputs: BTreeMap<String, Value> =
        [("image".to_string(), Value::Stack(Stack { grid: g.clone(), keys, layers: bands }))].into();
    for (port, v) in extra {
        inputs.insert(port.clone(), cut(port, v, grid, win, step, &g)?);
    }
    let args = Args { node: "live", inputs, params: &p };
    operator.run(&mut Ctx::default(), &args)
}
