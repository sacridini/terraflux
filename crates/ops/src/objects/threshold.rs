//! `segment.threshold`: eCognition's multi-threshold segmentation. A band is cut at thresholds into classes of values,
//! given or found by multi-level Otsu, and each connected piece of a class is an object.
//!
//! Multi-level Otsu (Otsu 1979; the formulation of Liao, Chen & Chung 2001) puts the k - 1 thresholds where the variance
//! between the k classes is highest on a histogram of the band over its range. With the histogram's prefix sums, the
//! variance between classes is a sum over the classes of S² / N (S the sum of the levels in a class, N its cells), so
//! the best cuts come from a dynamic programme over the classes and the bin where each ends: O(k · bins²), the bins of
//! a row of the table in parallel. The histogram is made by integer counts, so every number is the same on any number
//! of threads.

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::Objects;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Threshold));
}

/// The class of a cell without a value.
const NONE: u16 = u16::MAX;

/// The cells of a band handled by a task.
const CHUNK: usize = 1 << 16;

/// The class of a value, from 0: how many thresholds are at or below it.
#[inline(always)]
fn class_of(v: f64, th: &[f64]) -> u16 {
    if th.len() <= 8 {
        let mut k = 0u16;
        for &t in th {
            k += (v >= t) as u16;
        }
        k
    } else {
        th.partition_point(|&t| t <= v) as u16
    }
}

/// The lowest and highest finite value of a band (none when it has no finite value).
fn range_of(v: &[f64]) -> Option<(f64, f64)> {
    let (lo, hi) = v
        .par_chunks(CHUNK)
        .map(|c| {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for &x in c {
                if x.is_finite() {
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
            }
            (lo, hi)
        })
        .reduce(|| (f64::INFINITY, f64::NEG_INFINITY), |a, b| (a.0.min(b.0), a.1.max(b.1)));
    (lo <= hi).then_some((lo, hi))
}

/// The histogram of a band's finite values in `bins` equal bins over `lo..=hi` (the highest value in the last bin).
fn histogram(v: &[f64], lo: f64, hi: f64, bins: usize) -> Vec<u64> {
    let scale = bins as f64 / (hi - lo);
    v.par_chunks(CHUNK)
        .fold(
            || vec![0u64; bins],
            |mut h, c| {
                for &x in c {
                    if x.is_finite() {
                        h[(((x - lo) * scale) as usize).min(bins - 1)] += 1;
                    }
                }
                h
            },
        )
        .reduce(
            || vec![0u64; bins],
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(&b) {
                    *x += y;
                }
                a
            },
        )
}

/// The cuts of multi-level Otsu on a histogram: the k - 1 bin edges (a cut at `e` puts bins below `e` in the class
/// before it), ascending, that make the variance between the k classes highest. Needs k <= the bins with cells. On a
/// tie the lowest cut wins, and a cut inside a run of empty bins moves to the middle of the run (every cut there is as
/// good), so the cuts are in half bins: the numbers are twice the edges.
fn otsu_cuts(hist: &[u64], k: usize) -> Vec<usize> {
    let n = hist.len();
    // prefix sums of the cells and of the levels (bin numbers): whole numbers, so exact
    let (mut p, mut s) = (vec![0.0f64; n + 1], vec![0.0f64; n + 1]);
    for i in 0..n {
        p[i + 1] = p[i] + hist[i] as f64;
        s[i + 1] = s[i] + i as f64 * hist[i] as f64;
    }
    // what the class of bins a..b adds to the variance between the classes (up to the total's constant)
    let gain = |a: usize, b: usize| -> f64 {
        let cells = p[b] - p[a];
        if cells > 0.0 {
            let d = s[b] - s[a];
            d * d / cells
        } else {
            0.0
        }
    };
    // best[b]: the most the classes so far give with the last ending at bin edge b
    let mut best: Vec<f64> = (0..=n).map(|b| if b >= 1 { gain(0, b) } else { f64::NEG_INFINITY }).collect();
    let mut from: Vec<Vec<u32>> = Vec::with_capacity(k);
    for c in 2..=k {
        let row = |b: usize| -> (f64, u32) {
            let (mut top, mut at) = (f64::NEG_INFINITY, 0u32);
            if b >= c {
                for (a, &before) in best.iter().enumerate().take(b).skip(c - 1) {
                    let g = before + gain(a, b);
                    if g > top {
                        (top, at) = (g, a as u32);
                    }
                }
            }
            (top, at)
        };
        let rows: Vec<(f64, u32)> =
            if n >= 512 { (0..=n).into_par_iter().map(row).collect() } else { (0..=n).map(row).collect() };
        best = rows.iter().map(|r| r.0).collect();
        from.push(rows.iter().map(|r| r.1).collect());
    }
    // back from the last edge
    let mut cuts = Vec::with_capacity(k - 1);
    let mut b = n;
    for row in from.iter().rev() {
        b = row[b] as usize;
        cuts.push(b);
    }
    cuts.reverse();
    cuts.iter()
        .map(|&e| {
            let mut l = e;
            while l > 0 && hist[l - 1] == 0 {
                l -= 1;
            }
            let mut r = e;
            while r < n && hist[r] == 0 {
                r += 1;
            }
            l + r
        })
        .collect()
}

/// The band chosen by `band` (a name, or a number from 1; the first by default).
fn pick_band(args: &Args, names: &[String]) -> Result<usize> {
    let bad = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
    let list = || names.join(", ");
    let by_number = |k: i64| -> Result<usize> {
        if k >= 1 && k as usize <= names.len() {
            Ok(k as usize - 1)
        } else {
            Err(bad(format!("band {k}: the image has {} bands, 1 to {} ({})", names.len(), names.len(), list())))
        }
    };
    match args.params.get("band") {
        None | Some(serde_json::Value::Null) => Ok(0),
        Some(serde_json::Value::Number(x)) => {
            match x.as_i64().or_else(|| x.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)) {
                Some(k) => by_number(k),
                None => Err(bad(format!("band {x}: a name or a whole number from 1"))),
            }
        }
        Some(serde_json::Value::String(s)) => match names.iter().position(|n| n == s) {
            Some(k) => Ok(k),
            None => match s.trim().parse::<i64>() {
                Ok(k) => by_number(k),
                Err(_) => Err(bad(format!("band {s:?}: no such band ({})", list()))),
            },
        },
        Some(v) => Err(bad(format!("band {v}: a name or a number from 1"))),
    }
}

/// `segment.threshold`.
struct Threshold;

impl Operator for Threshold {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.threshold",
            "Objects by multi-threshold segmentation (eCognition's): one band is cut at thresholds into classes of \
             values, and each connected piece of a class is an object. The thresholds are given (a list, ascending), \
             or found for a number of classes by multi-level Otsu (Otsu 1979; Liao, Chen & Chung 2001): the cuts of a \
             histogram of the band that make the variance between the classes highest, the way a bimodal image \
             splits at the gap between its two modes. Classes count from 1: class 1 is the values below the first \
             threshold, class i those from threshold i - 1 up to threshold i, the last class those from the last \
             threshold up. A cell without a value is no object. The thresholds used are an output, and with Otsu \
             noted in the run's warnings. The same result on every run, on any number of threads.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands: the band cut is the one of `band`"))
        .output(PortSpec::new("objects", Type::Objects, "the pieces of the classes as objects"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), class (from 1), mean_<band> and sd_<band> of the \
             band cut",
        ))
        .output(PortSpec::new("thresholds", Type::Table, "a row per threshold used: threshold"))
        .param(PortSpec::optional(
            "band",
            Type::Any,
            "the band to cut, by name or by number from 1 (default the first)",
        ))
        .param(PortSpec::optional(
            "thresholds",
            Type::Any,
            "a list of numbers, strictly ascending: the cuts between the classes (a number of them gives that many \
             plus one classes)",
        ))
        .param(PortSpec::optional(
            "classes",
            Type::Integer,
            "instead of thresholds: the number of classes, 2 to 8, whose thresholds multi-level Otsu finds (default \
             2 when there are no thresholds)",
        ))
        .param(PortSpec::optional(
            "bins",
            Type::Integer,
            "with Otsu: the bins of the histogram, 16 to 4096 (default 256); more bins place the thresholds finer, \
             the work grows with their square",
        ))
        .param(
            PortSpec::optional("neighbours", Type::Integer, "which cells touch: 4 (default, sharing an edge) or 8")
                .choices([4, 8]),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let given: Option<Vec<f64>> = match args.params.get("thresholds") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::Number(x)) => Some(vec![x.as_f64().unwrap_or(f64::NAN)]),
            Some(_) => args.opt_param::<Vec<f64>>("thresholds")?,
        };
        let classes = args.opt_param::<i64>("classes")?;
        if let Some(t) = &given {
            if classes.is_some() {
                return Err(bad("give thresholds or classes, not both".into()));
            }
            if t.is_empty() || t.len() >= NONE as usize {
                return Err(bad(format!("thresholds: from 1 to {} numbers", NONE - 1)));
            }
            if t.iter().any(|x| !x.is_finite()) || t.windows(2).any(|p| p[0] >= p[1]) {
                return Err(bad("thresholds: finite numbers, strictly ascending".into()));
            }
        }
        let k = classes.unwrap_or(2);
        if !(2..=8).contains(&k) {
            return Err(bad(format!("classes {k}: 2 to 8")));
        }
        let bins: i64 = args.param("bins", 256)?;
        if !(16..=4096).contains(&bins) {
            return Err(bad(format!("bins {bins}: 16 to 4096")));
        }
        let nbs: usize = args.param("neighbours", 4)?;
        if nbs != 4 && nbs != 8 {
            return Err(bad(format!("neighbours {nbs}: 4 or 8")));
        }
        let l = crate::raster_stats::layers(args, "image")?;
        let names = super::band_names(&l.names);
        let at = pick_band(args, &names)?;
        let data = &l.data[at];
        let th: Vec<f64> = match given {
            Some(t) => t,
            None => {
                let k = k as usize;
                let bins = bins as usize;
                let Some((lo, hi)) = range_of(data).filter(|(lo, hi)| lo < hi) else {
                    return Err(bad(format!("band {}: no range of values to find thresholds in", names[at])));
                };
                let hist = histogram(data, lo, hi, bins);
                if hist.iter().filter(|&&c| c > 0).count() < k {
                    return Err(bad(format!(
                        "band {}: fewer than {k} levels at {bins} bins, too few for {k} classes",
                        names[at]
                    )));
                }
                let width = (hi - lo) / bins as f64;
                let th: Vec<f64> = otsu_cuts(&hist, k).iter().map(|&c| lo + c as f64 * 0.5 * width).collect();
                let said: Vec<String> = th.iter().map(|t| format!("{t}")).collect();
                ctx.warn(format!(
                    "node {node:?}: multi-level Otsu cut band {} into {k} classes at {}",
                    names[at],
                    said.join(", ")
                ));
                th
            }
        };
        // the class of each cell, in parallel
        let n = data.len();
        let mut cls = vec![NONE; n];
        cls.par_chunks_mut(CHUNK).zip(data.par_chunks(CHUNK)).for_each(|(out, v)| {
            for (o, &x) in out.iter_mut().zip(v) {
                if !x.is_nan() {
                    *o = class_of(x, &th);
                }
            }
        });
        let o = Objects::from_fn(l.grid.clone(), |i| cls[i], NONE, nbs);
        // each object's class: the one of its first cell
        let mut class = vec![0u32; o.count];
        let mut left = o.count;
        for (i, &id) in o.labels.iter().enumerate() {
            if left == 0 {
                break;
            }
            if id != 0 && class[id as usize - 1] == 0 {
                class[id as usize - 1] = cls[i] as u32 + 1;
                left -= 1;
            }
        }
        let mut t = super::stats_table(&o, &names[at..=at], std::slice::from_ref(data));
        t.columns.insert(3, "class".into());
        for (row, &c) in t.rows.iter_mut().zip(&class) {
            row.insert(3, c as f64);
        }
        let o = o.with_table(t.clone())?;
        let mut cuts = Table::new(vec!["threshold".into()]);
        cuts.rows = th.iter().map(|&x| vec![x]).collect();
        Ok(outputs([
            ("objects", Value::Objects(Arc::new(o))),
            ("table", Value::Table(t)),
            ("thresholds", Value::Table(cuts)),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A histogram with cells at the given bins (bin, cells).
    fn hist(n: usize, at: &[(usize, u64)]) -> Vec<u64> {
        let mut h = vec![0u64; n];
        for &(b, c) in at {
            h[b] = c;
        }
        h
    }

    #[test]
    fn two_modes_split_in_the_middle_of_the_gap() {
        // modes at bins 10..=12 and 90..=92, a gap of bins 13..=89 between: the cut is at its middle
        let h = hist(100, &[(10, 5), (11, 9), (12, 5), (90, 5), (91, 9), (92, 5)]);
        // the gap runs from edge 13 to edge 90, its middle 51.5: 103 in half bins
        assert_eq!(otsu_cuts(&h, 2), vec![13 + 90]);
    }

    #[test]
    fn three_modes_give_two_cuts_between_them() {
        let h = hist(60, &[(5, 10), (6, 10), (30, 10), (31, 10), (55, 10), (56, 10)]);
        let c = otsu_cuts(&h, 3);
        assert_eq!(c.len(), 2);
        assert!(c[0] > 2 * 7 && c[0] < 2 * 30, "{c:?}");
        assert!(c[1] > 2 * 32 && c[1] < 2 * 55, "{c:?}");
    }

    #[test]
    fn the_class_of_a_value_counts_the_thresholds_at_or_below() {
        let few = [1.0, 2.0, 3.0];
        let many: Vec<f64> = (1..=20).map(|x| x as f64).collect();
        for (v, want) in [(0.5, 0), (1.0, 1), (1.5, 1), (3.0, 3), (9.0, 3)] {
            assert_eq!(class_of(v, &few), want, "{v}");
        }
        for (v, want) in [(0.5, 0), (1.0, 1), (10.5, 10), (20.0, 20), (99.0, 20)] {
            assert_eq!(class_of(v, &many), want, "{v}");
        }
    }
}
