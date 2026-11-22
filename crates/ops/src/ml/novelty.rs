//! What a fitted model has seen, and how far new records lie from it. `ml.fit` keeps a summary of each predictor of
//! the training (the extremes, the quantiles 1, 25, 50, 75 and 99, the mean and the deviation) and the training rows
//! themselves (a seeded sample of 10 000 when there are more); `ml.predict` and `objects.classify` measure the new
//! records against them with Elith's MESS or the MOP, flag those outside (`novel`), say which predictors weigh most
//! (ExDet's most influential covariate) and tell when the whole scene is brighter or darker than the training's.
//! Models saved before have none of it (`training` is absent) and give no novelty.

use serde::{Deserialize, Serialize};

use crate::Ctx;
use crate::sdm::extrapolation::{self, Frame, Mop};
use crate::sdm::points::quantile7;
use crate::signatures::NumpyRng;

/// The most training rows a model keeps (a seeded sample when the training had more).
pub const MOST_ROWS: usize = 10_000;

/// How far above the training's own share of rows outside the others' range (see [`Training::baseline`]) the share of
/// the new records outside must be, in points, for a warning (and it must be more than twice it as well): a few
/// objects beyond the extremes of a small sample are the rule.
pub const EXCESS: f64 = 0.10;

/// The shift of a predictor's median, in interquartile ranges of the training, from which a scene is called another.
pub const SHIFT: f64 = 0.75;

/// A predictor of the training.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub q01: f64,
    pub q25: f64,
    pub q50: f64,
    pub q75: f64,
    pub q99: f64,
    pub mean: f64,
    pub sd: f64,
}

/// The training of a model: its predictors' summary and its rows (a column per predictor, in the model's order).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Training {
    /// The rows of the training (complete ones).
    pub rows: usize,
    /// Whether `sample` holds a draw of them and not all.
    #[serde(default)]
    pub sampled: bool,
    pub summary: Vec<Summary>,
    #[serde(default)]
    pub sample: Vec<Vec<f64>>,
    /// The share of the training rows that lie outside the range of the others (each row against the rest): what
    /// "outside the training" is for the training itself, which a scene is told against. None for a model saved
    /// before it was kept: no warning is made for it.
    #[serde(default)]
    pub baseline: Option<f64>,
}

impl Training {
    /// The training of the complete rows `x`: the summary of every column and the rows (a draw of `MOST_ROWS` of them
    /// with numpy's generator seeded `seed` when there are more, in their order).
    pub fn of(x: &Frame, seed: u64) -> Training {
        let rows = x.rows();
        let summary = x.names.iter().zip(&x.columns).map(|(n, c)| summarise(n, c)).collect();
        let (sampled, sample) = if rows <= MOST_ROWS {
            (false, x.columns.clone())
        } else {
            let mut pick = NumpyRng::new(seed).choice_distinct(rows, MOST_ROWS);
            pick.sort_unstable();
            (true, x.columns.iter().map(|c| pick.iter().map(|&k| c[k]).collect()).collect())
        };
        let baseline = leave_one_out(&sample);
        Training { rows, sampled, summary, sample, baseline }
    }

    /// The predictors, in the model's order.
    pub fn names(&self) -> Vec<String> {
        self.summary.iter().map(|s| s.name.clone()).collect()
    }

    /// The rows kept, as a frame.
    pub fn reference(&self) -> Frame {
        Frame { names: self.names(), columns: self.sample.clone() }
    }
}

/// The share of the rows that are outside the range of the others in some column (a MESS below 0 and a MOP beyond
/// the ranges both come to that, row by row): a row is outside when it is the one smallest or the one largest value of
/// a column. None with fewer than three rows.
fn leave_one_out(columns: &[Vec<f64>]) -> Option<f64> {
    let n = columns.first()?.len();
    if n < 3 {
        return None;
    }
    let mut outside = vec![false; n];
    for c in columns {
        let finite = || c.iter().enumerate().filter(|(_, v)| v.is_finite());
        let (Some(lo), Some(hi)) =
            (finite().map(|(_, &v)| v).min_by(f64::total_cmp), finite().map(|(_, &v)| v).max_by(f64::total_cmp))
        else {
            continue;
        };
        if lo == hi {
            continue;
        }
        let at_lo: Vec<usize> = finite().filter(|(_, v)| **v == lo).map(|(i, _)| i).collect();
        let at_hi: Vec<usize> = finite().filter(|(_, v)| **v == hi).map(|(i, _)| i).collect();
        if at_lo.len() == 1 {
            outside[at_lo[0]] = true;
        }
        if at_hi.len() == 1 {
            outside[at_hi[0]] = true;
        }
    }
    Some(outside.iter().filter(|&&o| o).count() as f64 / n as f64)
}

fn summarise(name: &str, c: &[f64]) -> Summary {
    let v: Vec<f64> = c.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        let n = f64::NAN;
        return Summary { name: name.into(), min: n, max: n, q01: n, q25: n, q50: n, q75: n, q99: n, mean: n, sd: n };
    }
    let q = quantile7(&v, &[0.0, 0.01, 0.25, 0.5, 0.75, 0.99, 1.0]);
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let sd = if v.len() > 1 {
        (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
    } else {
        0.0
    };
    Summary { name: name.into(), min: q[0], max: q[6], q01: q[1], q25: q[2], q50: q[3], q75: q[4], q99: q[5], mean, sd }
}

/// How novelty is measured: Elith's MESS (negative outside the training's ranges), the MOP (the mean distance to the
/// nearest training rows, and outside the ranges), or not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Mess,
    Mop,
    No,
}

impl Kind {
    pub fn parse(text: &str) -> Result<Kind, String> {
        match text {
            "mess" => Ok(Kind::Mess),
            "mop" => Ok(Kind::Mop),
            "none" => Ok(Kind::No),
            t => Err(format!("novelty: mess, mop or none, not {t:?}")),
        }
    }
}

/// The novelty of the records of a frame.
pub struct Found {
    /// The measure's column: `mess` or `mop`.
    pub measure: &'static str,
    pub values: Vec<f64>,
    /// 1 outside what the training saw, 0 inside, NaN where a predictor has no value.
    pub novel: Vec<f64>,
    /// The predictors that weigh most in the novel records, with how many of them each weighs on.
    pub weighs: Vec<(String, usize)>,
}

/// The novelty of the records of `target` (the model's predictors, in its order) against its training.
pub fn assess(t: &Training, target: &Frame, kind: Kind) -> Result<Option<Found>, String> {
    if kind == Kind::No || t.sample.is_empty() || t.sample.iter().any(|c| c.is_empty()) {
        return Ok(None);
    }
    let reference = t.reference().select(&target.names)?;
    let complete = |i: usize| target.columns.iter().all(|c| !c[i].is_nan());
    let (measure, values, novel) = match kind {
        Kind::Mess => {
            let f = extrapolation::mess(&reference, target, false)?;
            // (a row with a predictor missing has no novelty: MESS alone takes the predictors it has)
            let values: Vec<f64> =
                f.columns[0].iter().enumerate().map(|(i, &v)| if complete(i) { v } else { f64::NAN }).collect();
            let novel: Vec<f64> =
                values.iter().map(|&v| if v.is_nan() { f64::NAN } else { f64::from(v < 0.0) }).collect();
            ("mess", values, novel)
        }
        Kind::Mop => {
            let o = Mop {
                kind: "basic",
                distance: Some("euclidean"),
                r#where: "all",
                percentage: 1.0,
                scale: true,
                center: true,
                rescale: false,
            };
            let f = extrapolation::mop(&reference, target, &o)?;
            let novel: Vec<f64> = (0..target.rows())
                .map(|i| if complete(i) { f64::from(!f.columns[1][i].is_nan()) } else { f64::NAN })
                .collect();
            ("mop", f.columns[0].clone(), novel)
        }
        Kind::No => return Ok(None),
    };
    let weighs = heaviest(&reference, target, &novel);
    Ok(Some(Found { measure, values, novel, weighs }))
}

/// The predictors that weigh most in the novel rows: ExDet's most influential covariate of each (a sample of at most
/// 20 000 of them), or, when ExDet cannot be made (collinear predictors), how many are beyond each one's range.
fn heaviest(reference: &Frame, target: &Frame, novel: &[f64]) -> Vec<(String, usize)> {
    let rows: Vec<usize> = (0..novel.len()).filter(|&i| novel[i] == 1.0).collect();
    if rows.is_empty() {
        return Vec::new();
    }
    let step = rows.len().div_ceil(20_000);
    let rows: Vec<usize> = rows.into_iter().step_by(step).collect();
    let subset = Frame {
        names: target.names.clone(),
        columns: target.columns.iter().map(|c| rows.iter().map(|&i| c[i]).collect()).collect(),
    };
    let mut count = vec![0usize; target.names.len()];
    if let Ok(f) = extrapolation::exdet(reference, &subset) {
        for &m in &f.columns[1] {
            if m >= 1.0 && (m as usize) <= count.len() {
                count[m as usize - 1] += 1;
            }
        }
    }
    if count.iter().all(|&c| c == 0) {
        for (j, c) in subset.columns.iter().enumerate() {
            let (lo, hi) =
                reference.columns[j].iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
            count[j] = c.iter().filter(|&&x| x < lo || x > hi).count();
        }
    }
    let mut named: Vec<(String, usize)> = target.names.iter().cloned().zip(count).filter(|(_, c)| *c > 0).collect();
    named.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    named.truncate(3);
    named
}

/// The warning about the records outside the training: how many of them (`what`: objects, records, cells), and the
/// share of their `area` (the weight of each, none: all alike), what measured it and the predictors that weigh most.
/// Only when the share of the records or of the area is clearly above what the training itself gives (`baseline`: the
/// share of the training rows outside the range of the others): more than 10 points above it and more than twice it.
/// No warning for a model with no baseline.
pub fn warn(ctx: &mut Ctx, node: &str, what: &str, f: &Found, area: Option<&[f64]>, baseline: Option<f64>) {
    let Some(base) = baseline else { return };
    let valid: Vec<usize> = (0..f.novel.len()).filter(|&i| !f.novel[i].is_nan()).collect();
    let novel: Vec<usize> = valid.iter().copied().filter(|&i| f.novel[i] == 1.0).collect();
    if valid.is_empty() || novel.is_empty() {
        return;
    }
    let share = novel.len() as f64 / valid.len() as f64;
    let weight = |i: usize| area.map_or(1.0, |a| a[i]);
    let (inside, outside): (f64, f64) =
        (valid.iter().map(|&i| weight(i)).sum(), novel.iter().map(|&i| weight(i)).sum());
    let area_share = if inside > 0.0 { outside / inside } else { 0.0 };
    let clear = |s: f64| s > base + EXCESS && s > 2.0 * base;
    if !clear(share) && !clear(area_share) {
        return;
    }
    let how = if f.measure == "mess" { "MESS below 0" } else { "beyond the ranges of the training (MOP)" };
    let by = if area.is_some() { format!(" and {:.0} % of the area", 100.0 * area_share) } else { String::new() };
    let heavy = if f.weighs.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = f.weighs.iter().map(|(n, c)| format!("{n} ({c})")).collect();
        format!("; the predictors that weigh most: {}", list.join(", "))
    };
    ctx.warn(format!(
        "node {node:?}: {} of {} {what} ({:.0} %{by}) lie outside what the model saw in training ({how}), against {:.0} % of \
         the training samples themselves (each against the others): their classes are extrapolations{heavy}. Add \
         samples that cover them, or look at them: the `novel` column marks them (hatched in the Viewer)",
        novel.len(),
        valid.len(),
        100.0 * share,
        100.0 * base,
    ));
}

/// The median of the finite values.
fn median(v: &[f64]) -> f64 {
    let x: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if x.is_empty() { f64::NAN } else { quantile7(&x, &[0.5])[0] }
}

/// Whether an attribute is the level of a band (the mean or the median of its cells): what a change of light moves.
fn is_level(name: &str) -> bool {
    name.starts_with("mean_") || name.starts_with("median_")
}

/// Whether the predictors hold the levels of bands: the shift is then read in those alone (a brighter scene moves
/// the means; the shape and the texture of the objects do not follow it), else in all of them.
fn radiometric(summary: &[Summary]) -> bool {
    summary.iter().any(|s| is_level(&s.name))
}

/// The warning for a scene whose predictors are all higher, or all lower, than the training's (a brighter or a darker
/// scene, another date or sensor): at least half of the predictors, and one, with the median beyond [`SHIFT`]
/// interquartile ranges of the training's, on one side. Points to raster.match_histogram.
pub fn warn_shift(ctx: &mut Ctx, node: &str, t: &Training, target: &Frame) {
    let mut up = Vec::new();
    let mut down = Vec::new();
    let radio = radiometric(&t.summary);
    for s in t.summary.iter().filter(|s| !radio || is_level(&s.name)) {
        let Some(j) = target.names.iter().position(|n| *n == s.name) else { continue };
        let iqr = if s.q75 > s.q25 { s.q75 - s.q25 } else { s.sd };
        let m = median(&target.columns[j]);
        if iqr.is_nan() || iqr <= 0.0 || m.is_nan() {
            continue;
        }
        let d = (m - s.q50) / iqr;
        if d > SHIFT {
            up.push((s, m, d));
        } else if d < -SHIFT {
            down.push((s, m, d));
        }
    }
    let p =
        if radiometric(&t.summary) { t.summary.iter().filter(|s| is_level(&s.name)).count() } else { t.summary.len() };
    let (side, moved) = if up.len() >= down.len() { ("higher (brighter)", up) } else { ("lower (darker)", down) };
    if moved.is_empty() || 2 * moved.len() < p {
        return;
    }
    let list: Vec<String> = moved
        .iter()
        .take(3)
        .map(|(s, m, d)| format!("{}: median {m:.3} against {:.3}, {:.1} IQR", s.name, s.q50, d.abs()))
        .collect();
    ctx.warn(format!(
        "node {node:?}: the scene's attributes are {side} than those the model was trained on, {} of {p} by more than {SHIFT} \
         interquartile ranges ({}): another date, sensor or processing. The classes will follow the shift, not the \
         ground: bring the scene to the training image first with raster.match_histogram (the training scene as the \
         reference), and make the objects and their attributes again",
        moved.len(),
        list.join("; "),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(a: &[f64], b: &[f64]) -> Frame {
        Frame { names: vec!["a".into(), "b".into()], columns: vec![a.to_vec(), b.to_vec()] }
    }

    #[test]
    fn the_summary_is_the_quantiles_and_the_moments_of_each_predictor() {
        let v: Vec<f64> = (0..101).map(f64::from).collect();
        let t = Training::of(&frame(&v, &v.iter().map(|x| x * 2.0).collect::<Vec<_>>()), 1);
        let a = &t.summary[0];
        assert_eq!(
            (a.min, a.max, a.q01, a.q25, a.q50, a.q75, a.q99, a.mean),
            (0.0, 100.0, 1.0, 25.0, 50.0, 75.0, 99.0, 50.0)
        );
        assert!((a.sd - 858.5f64.sqrt()).abs() < 1e-12);
        assert_eq!(t.summary[1].max, 200.0);
        assert!(!t.sampled && t.sample[0].len() == 101);
    }

    #[test]
    fn the_baseline_is_the_share_of_rows_outside_the_range_of_the_others() {
        // each column's one smallest and one largest value: 2 of 10 rows for one column, the same two for a second
        let v: Vec<f64> = (0..10).map(f64::from).collect();
        let t = Training::of(&frame(&v, &v), 1);
        assert_eq!(t.baseline, Some(0.2));
        // other rows are the extremes of the second column: four rows outside
        let w = [5.0, 9.0, 0.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0];
        assert_eq!(Training::of(&frame(&v, &w), 1).baseline, Some(0.4));
        // a value that two rows share is inside the others' range: nobody is outside
        let tie = [1.0, 1.0, 2.0, 3.0, 3.0, 2.0];
        assert_eq!(Training::of(&frame(&tie, &tie), 1).baseline, Some(0.0));
        assert_eq!(Training::of(&frame(&[1.0, 2.0], &[1.0, 2.0]), 1).baseline, None, "two rows say nothing");
    }

    #[test]
    fn a_large_training_keeps_a_seeded_sample() {
        let v: Vec<f64> = (0..30_000).map(f64::from).collect();
        let t = Training::of(&frame(&v, &v), 3);
        assert!(t.sampled && t.rows == 30_000 && t.sample[0].len() == MOST_ROWS);
        assert_eq!(t.sample[0], Training::of(&frame(&v, &v), 3).sample[0], "the same seed, the same rows");
        assert!(t.sample[0].windows(2).all(|w| w[0] < w[1]), "in their order");
        assert_eq!(t.summary[0].max, 29_999.0, "the summary is of all the rows");
    }

    #[test]
    fn mess_flags_what_is_outside_the_range() {
        let v: Vec<f64> = (0..50).map(f64::from).collect();
        let t = Training::of(&frame(&v, &v), 1);
        let target = frame(&[10.0, 60.0, f64::NAN], &[10.0, 10.0, 10.0]);
        let f = assess(&t, &target, Kind::Mess).unwrap().unwrap();
        assert_eq!(f.measure, "mess");
        assert!(f.values[0] > 0.0 && f.values[1] < 0.0 && f.values[2].is_nan() == f.novel[2].is_nan());
        assert_eq!(f.novel[0], 0.0);
        assert_eq!(f.novel[1], 1.0);
        assert_eq!(f.weighs.first().map(|w| w.0.as_str()), Some("a"));
    }

    #[test]
    fn a_model_without_a_training_gives_nothing() {
        let t = Training { rows: 0, sampled: false, summary: Vec::new(), sample: Vec::new(), baseline: None };
        assert!(assess(&t, &frame(&[1.0], &[1.0]), Kind::Mess).unwrap().is_none());
        assert!(Kind::parse("other").is_err());
    }
}
