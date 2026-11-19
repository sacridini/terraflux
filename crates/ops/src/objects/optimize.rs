//! `segment.optimize`: unsupervised segmentation parameter optimisation (USPO), after Espindola, Câmara, Reis, Bins &
//! Monteiro (2006) and, for the F-measure, Johnson & Xie (2011).
//!
//! A segmenter is run once per value of one of its settings. Each result is scored by how alike the cells inside an
//! object are (the intra-object variance, weighted by the objects' cells, per band, then the mean over the bands: low
//! is good) and by how different the objects are from their neighbours (Moran's I of the objects' means over the
//! objects that touch, weight 1, per band, then the mean: low is good). Both are scaled over the runs to 0 (worst) to
//! 1 (best), (max - x) / (max - min), and joined: their sum (Espindola) or their F-measure (Johnson & Xie). The run with
//! the highest joined score wins.
//!
//! The runs are independent and go in parallel, as many at once as the memory allows. The winner depends on the
//! range of all the scores, so it is not known until the end; but it is never dominated (a run with both scores worse
//! than another's), so only the runs not dominated so far keep their objects.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::Objects;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// The most values (runs) of one call.
const MAX_VALUES: usize = 1000;
/// The most pieces the cells are cut into to be summed (each piece holds sums per object and band); a fixed number, so
/// the sums are the same at any number of threads.
const PIECES: usize = 16;

/// The setting `segment.optimize` varies when `parameter` is not given, for the segmenters that have a main one.
pub fn default_parameter(segmenter: &str) -> Option<&'static str> {
    Some(match segmenter {
        "segment.multiresolution" => "scale",
        "segment.region_growing" => "threshold",
        "segment.slic" | "segment.snic" => "size",
        "segment.felzenszwalb" => "k",
        "segment.watershed" => "h",
        "segment.quickshift" => "kernel_size",
        "segment.meanshift" => "range_radius",
        "segment.spectral_difference" => "max_difference",
        _ => return None,
    })
}

/// The scores of a segmentation: the intra-object variance (the objects' cells weighting) and Moran's I of the
/// objects' means, each the mean over the bands that have them (NaN when none: no object with a value, fewer than two
/// objects, objects that touch none, or means all equal).
pub(crate) fn score(o: &Objects, data: &[Arc<Vec<f64>>]) -> (f64, f64) {
    let nb = data.len();
    let n = o.count;
    if n == 0 || nb == 0 {
        return (f64::NAN, f64::NAN);
    }
    // a band's values are taken from its mean, so the sums of squares keep their digits
    let shift: Vec<f64> = data
        .par_iter()
        .map(|b| {
            let (s, k) = b.iter().filter(|x| x.is_finite()).fold((0.0, 0usize), |(s, k), x| (s + x, k + 1));
            if k > 0 { s / k as f64 } else { 0.0 }
        })
        .collect();
    // per object and band: the cells with a value, their sum and sum of squares, summed in pieces of the cells
    let cells = o.grid.cells();
    let width = n * nb * 3;
    let pieces = (cells / (2 * width).max(1)).clamp(1, PIECES);
    let span = cells.div_ceil(pieces);
    let mut parts: Vec<Vec<f64>> = (0..pieces)
        .into_par_iter()
        .map(|p| {
            let mut acc = vec![0.0; width];
            for i in p * span..((p + 1) * span).min(cells) {
                let l = o.labels[i] as usize;
                if l == 0 {
                    continue;
                }
                let base = (l - 1) * nb * 3;
                for (b, band) in data.iter().enumerate() {
                    let x = band[i];
                    if x.is_finite() {
                        let x = x - shift[b];
                        let a = &mut acc[base + b * 3..base + b * 3 + 3];
                        a[0] += 1.0;
                        a[1] += x;
                        a[2] += x * x;
                    }
                }
            }
            acc
        })
        .collect();
    let mut sums = parts.remove(0);
    // the pieces added in their order, objects in parallel
    sums.par_chunks_mut(3 * nb * 64).enumerate().for_each(|(k, chunk)| {
        for part in &parts {
            for (j, x) in chunk.iter_mut().enumerate() {
                *x += part[k * 3 * nb * 64 + j];
            }
        }
    });
    let bands: Vec<(f64, f64)> = (0..nb)
        .into_par_iter()
        .map(|b| {
            let at = |k: usize| &sums[k * nb * 3 + b * 3..k * nb * 3 + b * 3 + 3];
            let (mut total, mut ss) = (0.0, 0.0);
            // the mean of each object with a value, NaN for the others
            let mean: Vec<f64> = (0..n)
                .map(|k| {
                    let a = at(k);
                    if a[0] > 0.0 {
                        total += a[0];
                        ss += (a[2] - a[1] * a[1] / a[0]).max(0.0);
                        a[1] / a[0]
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            let variance = if total > 0.0 { ss / total } else { f64::NAN };
            let valid = mean.iter().filter(|m| !m.is_nan()).count();
            let grand = mean.iter().filter(|m| !m.is_nan()).sum::<f64>() / valid as f64;
            let den: f64 = mean.iter().filter(|m| !m.is_nan()).map(|m| (m - grand) * (m - grand)).sum();
            let (mut num, mut w) = (0.0, 0.0);
            for (k, nb_of) in o.adjacency.iter().enumerate() {
                if mean[k].is_nan() {
                    continue;
                }
                for &(j, _) in nb_of {
                    let m = mean[j as usize - 1];
                    if !m.is_nan() && j as usize - 1 != k {
                        num += (mean[k] - grand) * (m - grand);
                        w += 1.0;
                    }
                }
            }
            let moran = if valid >= 2 && w > 0.0 && den > 0.0 { valid as f64 / w * num / den } else { f64::NAN };
            (variance, moran)
        })
        .collect();
    let mean_of = |f: &dyn Fn(&(f64, f64)) -> f64| {
        let good: Vec<f64> = bands.iter().map(f).filter(|x| x.is_finite()).collect();
        if good.is_empty() { f64::NAN } else { good.iter().sum::<f64>() / good.len() as f64 }
    };
    (mean_of(&|b| b.0), mean_of(&|b| b.1))
}

/// A score scaled over the runs to 1 (the lowest) to 0 (the highest): (max - x) / (max - min); 0 when they are all
/// the same, NaN for a NaN.
pub fn scaled(x: &[f64]) -> Vec<f64> {
    let good = || x.iter().copied().filter(|v| v.is_finite());
    let lo = good().fold(f64::INFINITY, f64::min);
    let hi = good().fold(f64::NEG_INFINITY, f64::max);
    x.iter()
        .map(|&v| {
            if !v.is_finite() {
                f64::NAN
            } else if hi > lo {
                (hi - v) / (hi - lo)
            } else {
                0.0
            }
        })
        .collect()
}

/// The objective of each run from its variance and Moran's I: the sum of the two scaled scores, or with `alpha` their
/// F-measure (1 + a²) v i / (a² i + v), as i.segment.uspo: a above 1 weighs the variance (the homogeneity) more than
/// Moran's I.
fn objective(variance: &[f64], moran: &[f64], alpha: Option<f64>) -> Vec<f64> {
    let (v, m) = (scaled(variance), scaled(moran));
    v.iter()
        .zip(&m)
        .map(|(&v, &m)| match alpha {
            None => v + m,
            Some(a) => {
                let den = a * a * m + v;
                if den > 0.0 {
                    (1.0 + a * a) * v * m / den
                } else if v.is_nan() || m.is_nan() {
                    f64::NAN
                } else {
                    0.0
                }
            }
        })
        .collect()
}

/// A run that no other beats on both scores: its place among the values, its scores and what it made.
struct Kept {
    at: usize,
    variance: f64,
    moran: f64,
    out: Outputs,
}

/// Whether `a` is no better than `b`: not lower in the variance or in Moran's I, and either worse in one of them or
/// later among the values.
fn beaten(a: &Kept, b: &Kept) -> bool {
    b.variance <= a.variance && b.moran <= a.moran && (b.variance < a.variance || b.moran < a.moran || b.at < a.at)
}

/// What a run gives: the scores of the objects it made, the objects' outputs and the notes it made.
struct Run {
    variance: f64,
    moran: f64,
    objects: usize,
    notes: Vec<String>,
}

/// A number as the setting of an operator: a whole one is an integer (the settings that are counts read no other).
fn number(x: f64) -> serde_json::Value {
    if x.fract() == 0.0 && x.abs() < 9e15 { serde_json::Value::from(x as i64) } else { serde_json::Value::from(x) }
}

/// `segment.optimize`.
struct Optimize;

impl Operator for Optimize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.optimize",
            "The best value of a segmenter's setting, by unsupervised parameter optimisation (USPO of Espindola et \
             al. 2006, as GRASS's i.segment.uspo; the F-measure of Johnson & Xie 2011). The segmenter runs once for \
             each value, in parallel; each result is scored by its intra-object variance (the mean over the bands \
             of the variance inside the objects, weighted by their cells: low when the objects are homogeneous) and \
             by Moran's I of the objects' means over the objects that touch (per band, the mean: low when the \
             neighbours differ). Both are scaled over the runs from 1 (lowest) to 0 (highest) and the objective \
             is their sum, or with objective f their F-measure (1 + alpha²) v i / (alpha² i + v). The value with \
             the highest objective wins: too fine a segmentation is not different from its neighbours, too coarse \
             a one is not homogeneous. Its objects are the result. Read the table of scores as well as the winner: \
             it is a guide, as segment.scale is. Of the segmenter's other settings, `params` holds those that stay \
             fixed.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour (given to every run)"))
        .output(PortSpec::new("objects", Type::Objects, "the objects of the best run"))
        .output(PortSpec::new("table", Type::Table, "the table of those objects, as the segmenter gives it"))
        .output(PortSpec::new(
            "scores",
            Type::Table,
            "a row per value: value, objects, variance, moran, objective (NaN where a run has no score, e.g. one \
             object)",
        ))
        .output(PortSpec::new("value", Type::Number, "the best value of the setting"))
        .param(PortSpec::optional(
            "segmenter",
            Type::Text,
            "the operator, a segment.* that takes an image (default segment.multiresolution)",
        ))
        .param(PortSpec::optional(
            "parameter",
            Type::Text,
            "the setting varied (default: scale for multiresolution, threshold for region_growing, size for slic and \
             snic, k for felzenszwalb, h for watershed, kernel_size for quickshift, range_radius for meanshift, \
             max_difference for spectral_difference; else needed)",
        ))
        .param(PortSpec::optional("values", Type::Any, "a list of the values to try (or from, to and step)"))
        .param(PortSpec::optional("from", Type::Number, "the first value, with to and step"))
        .param(PortSpec::optional("to", Type::Number, "the last value, with from and step"))
        .param(PortSpec::optional("step", Type::Number, "the increase between values, above 0"))
        .param(PortSpec::optional(
            "params",
            Type::Any,
            "an object of the segmenter's other settings, fixed ({\"shape\": 0.3})",
        ))
        .param(
            PortSpec::optional(
                "objective",
                Type::Text,
                "sum (default: the scaled variance plus the scaled Moran's I, Espindola) or f (their F-measure, \
                 Johnson & Xie)",
            )
            .choices(["sum", "f"]),
        )
        .param(PortSpec::optional(
            "alpha",
            Type::Number,
            "with objective f, the weight of the variance's score against Moran's, above 0 (default 1: equal)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let segmenter: String = args.param("segmenter", "segment.multiresolution".to_string())?;
        let registry = Registry::standard();
        let (Some(op), Some(sig)) = (registry.get(&segmenter), registry.sig(&segmenter)) else {
            return Err(bad(format!("segmenter {segmenter:?}: no such operator")));
        };
        if !segmenter.starts_with("segment.")
            || segmenter == "segment.optimize"
            || !sig.inputs.iter().any(|p| p.name == "image")
            || !sig.outputs.iter().any(|p| p.name == "objects")
        {
            return Err(bad(format!("segmenter {segmenter:?}: a segment.* that takes an image and gives objects")));
        }
        if let Some(p) = sig.inputs.iter().find(|p| p.required && p.name != "image") {
            return Err(bad(format!("segmenter {segmenter:?} needs the input {:?} too: not run here", p.name)));
        }
        let parameter = match args.opt_param::<String>("parameter")?.filter(|p| !p.is_empty()) {
            Some(p) => p,
            None => default_parameter(&segmenter)
                .ok_or_else(|| bad(format!("parameter: the setting to vary (no default for {segmenter})")))?
                .to_string(),
        };
        if !sig.params.iter().any(|p| p.name == parameter) {
            let names: Vec<&str> = sig.params.iter().map(|p| p.name.as_str()).collect();
            return Err(bad(format!("parameter {parameter:?}: {segmenter} has the settings {}", names.join(", "))));
        }
        let fixed = match args.params.get("params") {
            Some(serde_json::Value::Object(o)) => o.clone(),
            None | Some(serde_json::Value::Null) => serde_json::Map::new(),
            Some(_) => return Err(bad("params: an object of the segmenter's settings".into())),
        };
        if fixed.contains_key(&parameter) {
            return Err(bad(format!("params: {parameter:?} is the setting varied, it is not fixed too")));
        }
        let values = values_of(args).map_err(bad)?;
        let alpha = match args.param("objective", "sum".to_string())?.as_str() {
            "sum" => None,
            "f" => {
                let a: f64 = args.param("alpha", 1.0)?;
                if !a.is_finite() || a <= 0.0 {
                    return Err(bad(format!("alpha {a}: above 0")));
                }
                Some(a)
            }
            o => return Err(bad(format!("objective {o:?}: sum or f"))),
        };
        let image = args.get("image").ok_or_else(|| bad("no input \"image\"".into()))?;
        let l = super::big::layers(args, "image")?;
        let cells = l.grid.cells() as u64;
        // as many runs at once as the memory of the runs allows, as the tiles of segment.multiresolution
        let per = cells.max(1) * (l.data.len() as u64 * 40 + 200);
        let fit = (super::big::TILES_BYTES.load(std::sync::atomic::Ordering::Relaxed) / per) as usize;
        let at_once = fit.clamp(1, rayon::current_num_threads()).min(values.len());
        let kept: Mutex<Vec<Kept>> = Mutex::new(Vec::new());
        let mut rows: Vec<Run> = Vec::new();
        let mut notes: BTreeSet<String> = BTreeSet::new();
        let one = |k: usize| -> Result<Run> {
            let mut params = fixed.clone();
            params.insert(parameter.clone(), number(values[k]));
            let inner = Args { node, inputs: [("image".to_string(), image.clone())].into(), params: &params };
            let mut c = Ctx::default();
            let out = op
                .run(&mut c, &inner)
                .map_err(|e| Error::msg(format!("node {node:?}: {segmenter} with {parameter} {}: {e}", values[k])))?;
            let Some(Value::Objects(o)) = out.get("objects") else {
                return Err(bad(format!("{segmenter} gave no objects")));
            };
            let table = out.get("table").cloned().unwrap_or_else(|| Value::Table(o.table.clone()));
            let (variance, moran) = score(o, &l.data);
            let objects = o.count;
            if variance.is_finite() && moran.is_finite() {
                let out = outputs([("objects", Value::Objects(o.clone())), ("table", table)]);
                let mine = Kept { at: k, variance, moran, out };
                let mut front = kept.lock().unwrap();
                if !front.iter().any(|f| beaten(&mine, f)) {
                    front.retain(|f| !beaten(f, &mine));
                    front.push(mine);
                }
            }
            Ok(Run { variance, moran, objects, notes: c.warnings })
        };
        for wave in (0..values.len()).collect::<Vec<_>>().chunks(at_once) {
            if ctx.stopped() {
                return Err(bad("stopped".into()));
            }
            ctx.progress(Some(rows.len() as f64 / values.len() as f64), format!("{} of {}", rows.len(), values.len()));
            let done: Vec<Result<Run>> = wave.par_iter().with_max_len(1).map(|&k| one(k)).collect();
            for r in done {
                let r = r?;
                notes.extend(r.notes.iter().cloned());
                rows.push(r);
            }
        }
        for n in notes {
            ctx.warn(n);
        }
        let variance: Vec<f64> = rows.iter().map(|r| r.variance).collect();
        let moran: Vec<f64> = rows.iter().map(|r| r.moran).collect();
        let obj = objective(&variance, &moran, alpha);
        let mut front = kept.into_inner().unwrap();
        // the best of those kept (the highest objective, the earliest value when tied)
        front.sort_by_key(|f| f.at);
        let best = front.iter().fold(None::<&Kept>, |b, f| match b {
            Some(b) if obj[b.at] >= obj[f.at] => Some(b),
            _ => Some(f),
        });
        let Some(best) = best else {
            return Err(bad(format!(
                "no value gave a score ({} runs with fewer than two objects, or objects that touch none or are all \
                 alike)",
                rows.len()
            )));
        };
        let mut scores = Table::new(["value", "objects", "variance", "moran", "objective"].map(String::from).to_vec());
        scores.rows = (0..rows.len())
            .map(|k| vec![values[k], rows[k].objects as f64, rows[k].variance, rows[k].moran, obj[k]])
            .collect();
        ctx.warn(format!(
            "node {node:?}: {segmenter} with {parameter} {} scored the highest of {} values (objective {}, {} objects)",
            values[best.at],
            rows.len(),
            obj[best.at],
            rows[best.at].objects
        ));
        let mut out = best.out.clone();
        out.insert("scores".into(), Value::Table(scores));
        out.insert("value".into(), Value::Number(values[best.at]));
        Ok(out)
    }
}

/// The values to try: a list, or from, to and step.
fn values_of(args: &Args) -> std::result::Result<Vec<f64>, String> {
    let list = match args.params.get("values") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => Some(serde_json::from_value::<Vec<f64>>(v.clone()).map_err(|_| "values: a list of numbers")?),
    };
    let get = |n: &str| args.opt_param::<f64>(n).map_err(|e| e.to_string());
    let (from, to, step) = (get("from")?, get("to")?, get("step")?);
    if list.is_some() && (from.is_some() || to.is_some() || step.is_some()) {
        return Err("values, or from, to and step, not both".into());
    }
    let values = match (list, from, to, step) {
        (Some(v), ..) => v,
        (None, Some(a), Some(b), Some(s)) => {
            if !a.is_finite() || !b.is_finite() || !s.is_finite() || s <= 0.0 || b < a {
                return Err(format!("from {a}, to {b}, step {s}: finite, from up to to, step above 0"));
            }
            let n = ((b - a) / s + 1e-9).floor();
            if n + 1.0 > MAX_VALUES as f64 {
                return Err(format!("from, to and step make over {MAX_VALUES} values"));
            }
            (0..=n as usize).map(|k| a + k as f64 * s).collect()
        }
        _ => return Err("the values: a list (values) or from, to and step".into()),
    };
    if values.is_empty() || values.len() > MAX_VALUES || values.iter().any(|x| !x.is_finite()) {
        return Err(format!("values: 1 to {MAX_VALUES} finite numbers"));
    }
    Ok(values)
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Optimize));
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_raster::Grid;

    /// 4 x 1 cells in two objects of two cells, values 1 3 | 5 9, and a second band 0 0 | 2 4.
    fn tiny() -> (Objects, Vec<Arc<Vec<f64>>>) {
        let o = Objects::from_regions(Arc::new(Grid::new(4, 1)), &[0, 0, 1, 1], 4);
        (o, vec![Arc::new(vec![1.0, 3.0, 5.0, 9.0]), Arc::new(vec![0.0, 0.0, 2.0, 4.0])])
    }

    #[test]
    fn score_is_the_weighted_variance_and_morans_i() {
        let (o, data) = tiny();
        let (v, m) = score(&o, &data);
        // band 1: variances 1 and 4 over two cells each: 2.5; band 2: 0 and 1: 0.5; the mean 1.5
        assert!((v - 1.5).abs() < 1e-12, "{v}");
        // two objects that touch: Moran's I is -1 in each band
        assert!((m + 1.0).abs() < 1e-12, "{m}");
    }

    #[test]
    fn moran_of_a_line_of_three_objects() {
        let o = Objects::from_regions(Arc::new(Grid::new(3, 1)), &[0, 1, 2], 4);
        let data = vec![Arc::new(vec![1.0, 2.0, 4.0])];
        let (v, m) = score(&o, &data);
        assert_eq!(v, 0.0);
        // means 1 2 4, grand 7/3, deviations -4/3 -1/3 5/3: sum over the 4 ordered pairs (1,2) and (2,3) twice
        // = 2 (4/9 - 5/9) = -2/9; denominator 16/9 + 1/9 + 25/9 = 42/9; N / W = 3 / 4
        let want = 3.0 / 4.0 * (-2.0 / 9.0) / (42.0 / 9.0);
        assert!((m - want).abs() < 1e-12, "{m} {want}");
    }

    #[test]
    fn one_object_has_no_moran_and_empty_cells_are_left_out() {
        let o = Objects::from_regions(Arc::new(Grid::new(2, 1)), &[0, 0], 4);
        let (v, m) = score(&o, &[Arc::new(vec![1.0, 3.0])]);
        assert_eq!(v, 1.0);
        assert!(m.is_nan());
        let (v, _) = score(&o, &[Arc::new(vec![1.0, f64::NAN]), Arc::new(vec![2.0, 6.0])]);
        // the first band has one cell (variance 0), the second two (4): the mean 2
        assert!((v - 2.0).abs() < 1e-12, "{v}");
    }

    #[test]
    fn scaling_and_the_objectives() {
        assert_eq!(scaled(&[1.0, 3.0, 2.0]), [1.0, 0.0, 0.5]);
        assert_eq!(scaled(&[2.0, 2.0]), [0.0, 0.0]);
        assert!(scaled(&[1.0, f64::NAN, 3.0])[1].is_nan());
        let (v, m) = ([1.0, 2.0, 3.0], [3.0, 1.0, 2.0]);
        assert_eq!(objective(&v, &m, None), [1.0, 1.5, 0.5]);
        // v = 1, .5, 0 and m = 0, 1, .5: F = 2 v m / (v + m)
        let f = objective(&v, &m, Some(1.0));
        assert_eq!(f, [0.0, 2.0 * 0.5 / 1.5, 0.0]);
    }
}
