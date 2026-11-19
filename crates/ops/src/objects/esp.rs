//! `segment.scale`: the scales worth using in a multiresolution segmentation, by the ESP of Drăguţ, Tiede & Levick
//! (2010) and its multi-band version ESP2 (Drăguţ, Csillik, Eisank & Tiede 2014).
//!
//! The image is segmented at a run of growing scales. Each level is made from the objects of the one below (the `from`
//! of `segment.multiresolution`), so the whole run costs about one segmentation. At each level the local variance LV
//! is the mean over the objects of their standard deviation (per band), and the rate of change ROC = (LV_L - LV_{L-1})
//! / LV_{L-1} x 100. A peak of the ROC (above both neighbours) marks a level where the objects stopped being parts
//! and became wholes: a scale to try. ESP2 takes the first three (fine, medium, coarse).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use super::multiresolution::{Cost, merge_level};
use crate::raster_stats::Layers;
use crate::{Args, Ctx, Error, Operator, Outputs, Result, Table, Value, outputs};

/// The levels of the run when `levels` is not given.
const LEVELS: usize = 30;
/// The side of the central window of the run when `window` is not given.
const WINDOW: usize = 512;
/// The peaks ESP2 picks.
const PEAKS: usize = 3;

/// A level of the run.
pub(crate) struct Row {
    pub scale: f64,
    pub objects: usize,
    /// Per band: the local variance (the mean over objects of their standard deviation) and its rate of change.
    pub lv: Vec<f64>,
    pub roc: Vec<f64>,
    pub roc_mean: f64,
}

/// What the run found.
pub(crate) struct Esp {
    pub rows: Vec<Row>,
    /// The levels (indices of `rows`) at the first peaks of the mean ROC.
    pub peaks: Vec<usize>,
}

impl Esp {
    /// The scale to use when only one is wanted: the first peak's, else the median of the scales tried.
    pub fn scale(&self) -> f64 {
        match self.peaks.first() {
            Some(&k) => self.rows[k].scale,
            None => self.rows[(self.rows.len() - 1) / 2].scale,
        }
    }
}

/// The central window of at most `window` x `window` cells (0: all), as a grid and its bands.
fn crop(l: &Layers, window: usize) -> (Arc<Grid>, Vec<Arc<Vec<f64>>>) {
    let (w, h) = (l.grid.width, l.grid.height);
    if window == 0 || (w <= window && h <= window) {
        return (l.grid.clone(), l.data.clone());
    }
    let (cw, ch) = (w.min(window), h.min(window));
    let (c0, r0) = ((w - cw) / 2, (h - ch) / 2);
    let data = l
        .data
        .iter()
        .map(|b| Arc::new((0..ch).flat_map(|r| b[(r0 + r) * w + c0..(r0 + r) * w + c0 + cw].iter().copied()).collect()))
        .collect();
    (Arc::new(Grid::new(cw, ch)), data)
}

/// The median over a band of the standard deviation in a 3 x 3 window (cells whose nine have a value); when that is 0
/// (flat areas the most), the mean of them; 0 when the band has none above 0.
fn local_sd(b: &[f64], w: usize, h: usize) -> f64 {
    let mut v: Vec<f64> = Vec::new();
    for r in 1..h.saturating_sub(1) {
        for c in 1..w.saturating_sub(1) {
            let (mut n, mut s, mut s2) = (0.0, 0.0, 0.0);
            for rr in r - 1..=r + 1 {
                for &x in &b[rr * w + c - 1..=rr * w + c + 1] {
                    n += 1.0;
                    s += x;
                    s2 += x * x;
                }
            }
            if s.is_finite() {
                let m = s / n;
                v.push((s2 / n - m * m).max(0.0).sqrt());
            }
        }
    }
    if v.is_empty() {
        return 0.0;
    }
    let mid = v.len() / 2;
    let median = *v.select_nth_unstable_by(mid, f64::total_cmp).1;
    if median > 0.0 { median } else { v.iter().sum::<f64>() / v.len() as f64 }
}

/// The scale ESP starts from when none is given: the square root of the weighted sum, over the bands, of their local
/// standard deviation (so scale squared is about what merging two cells of noise costs). An error for an image
/// with no variation.
fn default_start(data: &[Arc<Vec<f64>>], w: usize, h: usize, weights: &[f64]) -> Result<f64> {
    let total: f64 = data.iter().zip(weights).map(|(b, wt)| wt * local_sd(b, w, h)).sum();
    if total.is_finite() && total > 0.0 {
        Ok(total.sqrt())
    } else {
        Err(Error::msg("segment.scale: the image has no variation (or no 3 x 3 cells with values): give a start"))
    }
}

/// The settings of a run.
pub(crate) struct Settings {
    pub start: Option<f64>,
    pub step: Option<f64>,
    pub levels: usize,
    pub window: usize,
    pub seed: u64,
}

/// ESP over an image: the levels' LV and ROC and the peaks.
pub(crate) fn esp(l: &Layers, cost: &Cost, s: &Settings) -> Result<Esp> {
    let (grid, data) = crop(l, s.window);
    let (w, h) = (grid.width, grid.height);
    let start = match s.start {
        Some(x) => x,
        None => default_start(&data, w, h, &cost.weights)?,
    };
    let step = s.step.unwrap_or(start);
    let nb = data.len();
    let mut rows: Vec<Row> = Vec::new();
    let mut below = None;
    for k in 0..s.levels {
        let scale = start + k as f64 * step;
        let level = merge_level(&grid, &data, cost, scale, true, s.seed, 100, below.as_ref());
        let (mut sum, mut cnt) = (vec![0.0; nb], vec![0usize; nb]);
        for r in level.roots() {
            for (j, x) in level.sds(r).into_iter().enumerate() {
                if !x.is_nan() {
                    sum[j] += x;
                    cnt[j] += 1;
                }
            }
        }
        let lv: Vec<f64> = (0..nb).map(|j| if cnt[j] > 0 { sum[j] / cnt[j] as f64 } else { f64::NAN }).collect();
        let roc: Vec<f64> = match rows.last() {
            None => vec![f64::NAN; nb],
            Some(p) => {
                (0..nb).map(|j| if p.lv[j] > 0.0 { (lv[j] - p.lv[j]) / p.lv[j] * 100.0 } else { f64::NAN }).collect()
            }
        };
        let good: Vec<f64> = roc.iter().copied().filter(|x| x.is_finite()).collect();
        let roc_mean = if good.is_empty() { f64::NAN } else { good.iter().sum::<f64>() / good.len() as f64 };
        let objects = level.objects.count;
        rows.push(Row { scale, objects, lv, roc, roc_mean });
        // one object left: the next levels are the same
        if objects <= 1 {
            break;
        }
        below = Some(level.objects);
    }
    // the first peaks of the mean ROC: above the level before and the one after
    let peaks: Vec<usize> = (2..rows.len().saturating_sub(1))
        .filter(|&k| {
            let (a, b, c) = (rows[k - 1].roc_mean, rows[k].roc_mean, rows[k + 1].roc_mean);
            a.is_finite() && b.is_finite() && c.is_finite() && b > a && b > c
        })
        .take(PEAKS)
        .collect();
    Ok(Esp { rows, peaks })
}

/// The scale `segment.multiresolution` uses when none is given: ESP's first peak (the median scale tried when there is
/// none), with the default start, step, levels and window.
pub(crate) fn suggest_scale(l: &Layers, shape: f64, compactness: f64, weights: &[f64], seed: u64) -> Result<f64> {
    let cost = Cost { colour: 1.0 - shape, compact: compactness, weights: weights.to_vec() };
    let s = Settings { start: None, step: None, levels: LEVELS, window: WINDOW, seed };
    Ok(esp(l, &cost, &s)?.scale())
}

/// `segment.scale`.
pub struct Scale;

impl Operator for Scale {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.scale",
            "The scales worth trying in segment.multiresolution (ESP of Drăguţ, Tiede & Levick 2010; ESP2 for \
             several bands, Drăguţ et al. 2014). The image is segmented at the scales start, start + step, ... (each \
             level made from the one below, so the run costs about one segmentation); at each the local variance LV \
             is the mean over the objects of their standard deviation, and its rate of change ROC = (LV - LV \
             before) / LV before x 100 (per band and the mean over the bands). A peak of the mean ROC, above both \
             neighbours, is a level where the objects went from parts to wholes: the first three peaks are \
             suggested (fine, medium, coarse). Without a start it is the square root of the sum over the bands of \
             weight x the median local standard deviation (3 x 3 cells), so scale squared is about what merging \
             two cells of noise costs. To stay fast it runs on the central window of at most `window` x `window` \
             cells; the scales hold for the whole image. Read the curve (the chart of the table) as well as the \
             peaks: ESP is a guide to scales, not an answer.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per level: scale, objects, lv_<band> and roc_<band> for each band, roc_mean, and suggested (1 at \
             the peaks)",
        ))
        .output(PortSpec::new("scales", Type::Table, "the suggested scales: level and scale (none if no peak)"))
        .output(PortSpec::new(
            "scale",
            Type::Number,
            "the first suggested scale (the median of those tried when there is no peak)",
        ))
        .param(PortSpec::optional(
            "start",
            Type::Number,
            "the first scale, above 0 (default: from the image's local standard deviation, see above)",
        ))
        .param(PortSpec::optional("step", Type::Number, "the increase of the scale between levels (default start)"))
        .param(PortSpec::optional("levels", Type::Integer, "how many scales to try, 3 or more (default 30)"))
        .param(PortSpec::optional("shape", Type::Number, "as in segment.multiresolution (default 0.1)"))
        .param(PortSpec::optional("compactness", Type::Number, "as in segment.multiresolution (default 0.5)"))
        .param(PortSpec::optional("weights", Type::Any, "a list, the weight of each band (default 1 for all)"))
        .param(PortSpec::optional(
            "window",
            Type::Integer,
            "the side in cells of the central window it runs on (default 512; 0 the whole image)",
        ))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of the order of the merges (default 0)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let l = crate::raster_stats::layers(args, "image")?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, false);
        let nb = l.data.len();
        let above0 = |name: &str| -> Result<Option<f64>> {
            match args.opt_param::<f64>(name)? {
                Some(x) if !x.is_finite() || x <= 0.0 => Err(bad(format!("{name} {x}: above 0"))),
                x => Ok(x),
            }
        };
        let (start, step) = (above0("start")?, above0("step")?);
        let levels: i64 = args.param("levels", LEVELS as i64)?;
        if !(3..=1000).contains(&levels) {
            return Err(bad(format!("levels {levels}: 3 to 1000")));
        }
        let shape: f64 = args.param("shape", 0.1)?;
        if !(0.0..=0.9).contains(&shape) {
            return Err(bad(format!("shape {shape}: 0 to 0.9")));
        }
        let compact: f64 = args.param("compactness", 0.5)?;
        if !(0.0..=1.0).contains(&compact) {
            return Err(bad(format!("compactness {compact}: 0 to 1")));
        }
        let window: i64 = args.param("window", WINDOW as i64)?;
        if window < 0 {
            return Err(bad(format!("window {window}: 0 or more cells")));
        }
        let seed: i64 = args.param("seed", 0)?;
        let weights: Vec<f64> = args.opt_param::<Vec<f64>>("weights")?.unwrap_or_else(|| vec![1.0; nb]);
        if weights.len() != nb || weights.iter().any(|x| !x.is_finite() || *x < 0.0) {
            return Err(bad(format!("weights: {nb} numbers, 0 or more (one for each band)")));
        }
        let cost = Cost { colour: 1.0 - shape, compact, weights };
        let set = Settings { start, step, levels: levels as usize, window: window as usize, seed: seed as u64 };
        let run = esp(&l, &cost, &set).map_err(|e| bad(e.to_string()))?;
        let names = super::band_names(&l.names);
        let mut columns: Vec<String> = ["scale", "objects"].map(String::from).to_vec();
        columns.extend(names.iter().map(|s| format!("lv_{s}")));
        columns.extend(names.iter().map(|s| format!("roc_{s}")));
        columns.push("roc_mean".into());
        columns.push("suggested".into());
        let mut t = Table::new(columns);
        t.rows = run
            .rows
            .iter()
            .enumerate()
            .map(|(k, r)| {
                let mut row = vec![r.scale, r.objects as f64];
                row.extend(&r.lv);
                row.extend(&r.roc);
                row.push(r.roc_mean);
                row.push(if run.peaks.contains(&k) { 1.0 } else { 0.0 });
                row
            })
            .collect();
        let mut sc = Table::new(["level", "scale"].map(String::from).to_vec());
        sc.rows = run.peaks.iter().map(|&k| vec![(k + 1) as f64, run.rows[k].scale]).collect();
        Ok(outputs([("table", Value::Table(t)), ("scales", Value::Table(sc)), ("scale", Value::Number(run.scale()))]))
    }
}
