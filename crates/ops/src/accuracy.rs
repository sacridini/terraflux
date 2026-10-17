//! The accuracy and the area of a map from a stratified random sample, the good practices of Olofsson et al. (2014,
//! Remote Sensing of Environment 148, 42-57): `accuracy.sample_size` gives the size of the sample and its
//! allocation to the strata, `accuracy.sample` draws the points, and `accuracy.assess` takes them back labelled with
//! their reference class: the error matrix in shares of area, the user's, producer's and overall accuracies and the
//! area of each class, with their standard errors and confidence intervals, as R's mapaccuracy computes them
//! (`olofsson` when the strata are the map's classes, `stehman2014` for strata of their own).

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::signatures::NumpyRng;
use crate::value::class_label;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(SampleSize));
    r.add(Arc::new(Sample));
    r.add(Arc::new(Assess));
}

// -- the strata ---------------------------------------------------------------------------------------------------

/// The classes of a map (increasing) and the cells of each.
pub fn class_cells(map: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let classes = terraflux_ls_sys::ffi::map_classes(map);
    let mut cells = vec![0usize; classes.len()];
    for &v in map {
        if !v.is_nan() {
            cells[classes.partition_point(|&c| c < v)] += 1;
        }
    }
    (classes, cells)
}

/// A number per class: one for all, or `{class: value}` (`default` for the others).
fn per_class(v: Option<&serde_json::Value>, classes: &[f64], default: f64, what: &str) -> Result<Vec<f64>> {
    match v {
        None | Some(serde_json::Value::Null) => Ok(vec![default; classes.len()]),
        Some(serde_json::Value::Number(n)) => Ok(vec![n.as_f64().unwrap_or(f64::NAN); classes.len()]),
        Some(serde_json::Value::Object(o)) => {
            let mut out = vec![default; classes.len()];
            for (k, x) in o {
                let c: f64 = k.trim().parse().map_err(|_| Error::msg(format!("{what}: {k:?} is not a class")))?;
                let i = classes
                    .iter()
                    .position(|&v| v == c)
                    .ok_or_else(|| Error::msg(format!("{what}: the map has no class {k}")))?;
                out[i] = x.as_f64().ok_or_else(|| Error::msg(format!("{what}: {k}: not a number")))?;
            }
            Ok(out)
        }
        Some(other) => Err(Error::msg(format!("{what}: a number or {{class: number}}, not {other}"))),
    }
}

// -- the size of the sample ---------------------------------------------------------------------------------------

/// Olofsson's equation 13: the sample that gives the overall accuracy a standard error `se`, from the strata's
/// shares of the map and the standard deviations √(U (1 − U)) of their expected user's accuracies.
pub fn sample_size(weights: &[f64], sd: &[f64], se: f64) -> usize {
    let s: f64 = weights.iter().zip(sd).map(|(w, s)| w * s).sum();
    ((s / se) * (s / se)).ceil() as usize
}

/// `n` shared in proportion to `weights` without going over `caps`, in whole numbers adding up to `n` (the
/// largest remainders take the cells left, the first on ties).
pub fn apportion(n: usize, weights: &[f64], caps: &[usize]) -> Vec<usize> {
    let k = weights.len();
    let mut target = vec![0.0; k];
    let mut capped = vec![false; k];
    loop {
        let fixed: usize = (0..k).filter(|&i| capped[i]).map(|i| caps[i]).sum();
        let left = n.saturating_sub(fixed) as f64;
        let w: f64 = (0..k).filter(|&i| !capped[i]).map(|i| weights[i]).sum();
        let mut again = false;
        for i in 0..k {
            if capped[i] {
                target[i] = caps[i] as f64;
            } else {
                target[i] = if w > 0.0 { left * weights[i] / w } else { 0.0 };
                if target[i] > caps[i] as f64 {
                    capped[i] = true;
                    again = true;
                }
            }
        }
        if !again {
            break;
        }
    }
    let mut out: Vec<usize> = target.iter().map(|t| t.floor() as usize).collect();
    let mut left = n.saturating_sub(out.iter().sum());
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&a, &b| (target[b] - target[b].floor()).total_cmp(&(target[a] - target[a].floor())).then(a.cmp(&b)));
    while left > 0 {
        let before = left;
        for &i in &order {
            if left > 0 && out[i] < caps[i] {
                out[i] += 1;
                left -= 1;
            }
        }
        if left == before {
            break;
        }
    }
    out
}

/// How a sample is shared among the strata.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Allocation {
    /// In proportion to the strata's areas.
    Proportional,
    /// The same in every stratum.
    Equal,
    /// Neyman's: in proportion to the areas times the standard deviations.
    Neyman,
    /// At least this many in every stratum (or all of its cells), the rest in proportion to the areas.
    Minimum(usize),
}

/// The sample of each stratum (their cells as caps); and whether the minima asked for more than `n`.
pub fn allocate(n: usize, weights: &[f64], sd: &[f64], cells: &[usize], how: Allocation) -> (Vec<usize>, bool) {
    let k = weights.len();
    match how {
        Allocation::Proportional => (apportion(n, weights, cells), false),
        Allocation::Equal => (apportion(n, &vec![1.0; k], cells), false),
        Allocation::Neyman => {
            let w: Vec<f64> = weights.iter().zip(sd).map(|(w, s)| w * s).collect();
            (apportion(n, &w, cells), false)
        }
        Allocation::Minimum(min) => {
            let floor: Vec<usize> = cells.iter().map(|&c| c.min(min)).collect();
            let mut fixed = vec![false; k];
            loop {
                let given: usize = (0..k).filter(|&i| fixed[i]).map(|i| floor[i]).sum();
                let left = n.saturating_sub(given) as f64;
                let w: f64 = (0..k).filter(|&i| !fixed[i]).map(|i| weights[i]).sum();
                let mut again = false;
                for i in 0..k {
                    if !fixed[i] && (w <= 0.0 || left * weights[i] / w < floor[i] as f64) {
                        fixed[i] = true;
                        again = true;
                    }
                }
                if !again {
                    break;
                }
            }
            let given: usize = (0..k).filter(|&i| fixed[i]).map(|i| floor[i]).sum();
            let rest: Vec<f64> = (0..k).map(|i| if fixed[i] { 0.0 } else { weights[i] }).collect();
            let caps: Vec<usize> = (0..k).map(|i| if fixed[i] { 0 } else { cells[i] }).collect();
            let shared = apportion(n.saturating_sub(given), &rest, &caps);
            ((0..k).map(|i| if fixed[i] { floor[i] } else { shared[i] }).collect(), given > n)
        }
    }
}

// -- the estimates ------------------------------------------------------------------------------------------------

/// What a sample says of a map: by class (in `classes`' order) the user's and producer's accuracies and the share
/// of the area, with their standard errors; the overall accuracy; the error matrix in shares of the area (rows: the
/// map's class, columns: the reference's; NaN where no sample fell); NaN where an estimate has no value (R's NA).
#[derive(Debug, Clone, PartialEq)]
pub struct Estimates {
    pub classes: Vec<f64>,
    pub overall: f64,
    pub overall_se: f64,
    pub user: Vec<f64>,
    pub user_se: Vec<f64>,
    pub producer: Vec<f64>,
    pub producer_se: Vec<f64>,
    pub area: Vec<f64>,
    pub area_se: Vec<f64>,
    pub matrix: Vec<Vec<f64>>,
    /// The cells the estimates stand for (the strata's).
    pub total: f64,
}

/// A sum without the NaNs (R's `na.rm = TRUE`).
fn sum_na(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().filter(|x| !x.is_nan()).sum()
}

/// R's `x^2`.
fn sq(x: f64) -> f64 {
    x * x
}

/// mapaccuracy's `olofsson(r, m, Nh)`: the strata are the map's classes, `cells` the cells of each of `classes`;
/// classes of the reference that are not in the map come after them, without cells.
pub fn olofsson(map: &[f64], reference: &[f64], classes: &[f64], cells: &[f64]) -> Result<Estimates> {
    let mut cls = classes.to_vec();
    let mut nhc = cells.to_vec();
    if let Some(m) = map.iter().find(|m| !classes.contains(m)) {
        return Err(Error::msg(format!("the map's class {} of a sample has no cells in the map", class_label(*m))));
    }
    for &r in reference {
        if !cls.contains(&r) {
            cls.push(r);
            nhc.push(0.0);
        }
    }
    let extra = classes.len();
    let q = cls.len();
    let pos = |v: f64| cls.iter().position(|&c| c == v).unwrap();
    let mut count = vec![vec![0.0; q]; q];
    for (&m, &r) in map.iter().zip(reference) {
        count[pos(m)][pos(r)] += 1.0;
    }
    let a: f64 = nhc.iter().sum();
    let nh: Vec<f64> = count.iter().map(|row| row.iter().sum()).collect();
    let wh: Vec<f64> = nhc.iter().map(|&n| n / a).collect();
    let mut props = vec![vec![0.0; q]; q];
    for i in 0..q {
        for j in 0..q {
            props[i][j] = if i >= extra && j >= extra { 0.0 } else { count[i][j] / nh[i] * wh[i] };
        }
    }
    let row = |i: usize| sum_na(props[i].iter().copied());
    let col = |j: usize| sum_na(props.iter().map(|r| r[j]));
    let overall = sum_na((0..q).map(|j| props[j][j]));
    let user: Vec<f64> = (0..q).map(|j| props[j][j] / row(j)).collect();
    let producer: Vec<f64> = (0..q).map(|j| props[j][j] / col(j)).collect();
    let area: Vec<f64> = (0..q).map(col).collect();
    let finite = |x: f64| if x.is_finite() { x } else { 0.0 };
    let mut var_oa = 0.0;
    for j in 0..q {
        var_oa += finite(sq(wh[j]) * user[j] * (1.0 - user[j]) / (nh[j] - 1.0));
    }
    let user_se: Vec<f64> = (0..q)
        .map(|j| {
            if user[j].is_nan() {
                return f64::NAN;
            }
            let v = user[j] * (1.0 - user[j]) / (nh[j] - 1.0);
            if v.is_nan() { 0.0 } else { v.sqrt() }
        })
        .collect();
    let n_j: Vec<f64> = (0..q).map(|j| sum_na((0..q).map(|i| nhc[i] / nh[i] * count[i][j]))).collect();
    let producer_se: Vec<f64> = (0..q)
        .map(|j| {
            let t1 = finite(sq(nhc[j]) * sq(1.0 - producer[j]) * user[j] * (1.0 - user[j]) / (nh[j] - 1.0));
            let t2 = sum_na(
                (0..q)
                    .filter(|&i| i != j)
                    .map(|i| sq(nhc[i]) * count[i][j] / nh[i] * (1.0 - count[i][j] / nh[i]) / (nh[i] - 1.0)),
            );
            let t2 = finite(t2);
            ((1.0 / sq(n_j[j])) * (t1 + sq(producer[j]) * t2)).sqrt()
        })
        .collect();
    let area_se: Vec<f64> = (0..q)
        .map(|j| {
            sum_na((0..q).map(|i| {
                let (b, c) = (count[i][j] / nh[i], 1.0 - count[i][j] / nh[i]);
                sq(wh[i]) * (b * c / (nh[i] - 1.0))
            }))
            .sqrt()
        })
        .collect();
    let matrix =
        (0..q).map(|i| (0..q).map(|j| if count[i][j] == 0.0 { f64::NAN } else { props[i][j] }).collect()).collect();
    Ok(Estimates {
        classes: cls,
        overall,
        overall_se: var_oa.sqrt(),
        user,
        user_se,
        producer,
        producer_se,
        area,
        area_se,
        matrix,
        total: a,
    })
}

/// The mean and the sample variances and covariance (n − 1) of two variables; NaN variances of fewer than two
/// values as 0 (mapaccuracy's `tvar[is.na(tvar)] <- 0`).
fn moments(y: &[f64], x: &[f64]) -> (f64, f64, f64, f64, f64) {
    let n = y.len() as f64;
    let my = y.iter().sum::<f64>() / n;
    let mx = x.iter().sum::<f64>() / n;
    if y.len() < 2 {
        return (my, mx, 0.0, 0.0, 0.0);
    }
    let vy = y.iter().map(|v| sq(v - my)).sum::<f64>() / (n - 1.0);
    let vx = x.iter().map(|v| sq(v - mx)).sum::<f64>() / (n - 1.0);
    let c = y.iter().zip(x).map(|(a, b)| (a - my) * (b - mx)).sum::<f64>() / (n - 1.0);
    (my, mx, vy, vx, c)
}

/// mapaccuracy's `stehman2014(s, r, m, Nh_strata)`: strata of their own (`strata` of each sample unit, `cells` the
/// cells of each of `strata_classes`); the strata without a sample unit are left out of the total. The classes are
/// those of the map and of the reference, increasing.
pub fn stehman(
    strata: &[f64],
    map: &[f64],
    reference: &[f64],
    strata_classes: &[f64],
    cells: &[f64],
) -> Result<Estimates> {
    if let Some(s) = strata.iter().find(|s| !strata_classes.contains(s)) {
        return Err(Error::msg(format!("the stratum {} of a sample has no cells", class_label(*s))));
    }
    // the strata sampled, in their order, with their units
    let kept: Vec<usize> = (0..strata_classes.len()).filter(|&h| strata.contains(&strata_classes[h])).collect();
    let units: Vec<Vec<usize>> =
        kept.iter().map(|&h| (0..strata.len()).filter(|&u| strata[u] == strata_classes[h]).collect()).collect();
    let big_n: Vec<f64> = kept.iter().map(|&h| cells[h]).collect();
    let small_n: Vec<f64> = units.iter().map(|u| u.len() as f64).collect();
    let total: f64 = big_n.iter().sum();
    let mut order: Vec<f64> = map.iter().chain(reference).copied().collect();
    order.sort_by(f64::total_cmp);
    order.dedup();
    let q = order.len();
    let incl: Vec<f64> = (0..strata.len())
        .map(|u| {
            let h = kept.iter().position(|&h| strata_classes[h] == strata[u]).unwrap();
            small_n[h] / big_n[h]
        })
        .collect();
    let eq2 = |y: &dyn Fn(usize) -> f64| (0..strata.len()).map(|u| y(u) / incl[u]).sum::<f64>() / total;
    let eq25 = |var: &[f64]| {
        (0..kept.len()).map(|h| sq(big_n[h]) * (1.0 - small_n[h] / big_n[h]) * var[h] / small_n[h]).sum::<f64>()
            * (1.0 / sq(total))
    };
    let eq28 = |r: f64, x: f64, vy: &[f64], vx: &[f64], cov: &[f64]| {
        (0..kept.len())
            .map(|h| {
                sq(big_n[h]) * (1.0 - small_n[h] / big_n[h]) * (vy[h] + sq(r) * vx[h] - 2.0 * r * cov[h]) / small_n[h]
            })
            .sum::<f64>()
            * (1.0 / sq(x))
    };
    // a variable's means, variances and covariances with another, stratum by stratum
    let by_stratum = |y: &dyn Fn(usize) -> f64, x: &dyn Fn(usize) -> f64| {
        let mut out = (vec![], vec![], vec![], vec![], vec![]);
        for u in &units {
            let ys: Vec<f64> = u.iter().map(|&i| y(i)).collect();
            let xs: Vec<f64> = u.iter().map(|&i| x(i)).collect();
            let (my, mx, vy, vx, c) = moments(&ys, &xs);
            out.0.push(my);
            out.1.push(mx);
            out.2.push(vy);
            out.3.push(vx);
            out.4.push(c);
        }
        out
    };
    let ind = |b: bool| f64::from(u8::from(b));
    let hit = |u: usize| ind(map[u] == reference[u]);
    let overall = eq2(&hit);
    let (_, _, v_hit, _, _) = by_stratum(&hit, &hit);
    let overall_se = eq25(&v_hit).sqrt();
    let mut area = Vec::with_capacity(q);
    let mut area_se = Vec::with_capacity(q);
    let mut user = Vec::with_capacity(q);
    let mut user_se = Vec::with_capacity(q);
    let mut producer = Vec::with_capacity(q);
    let mut producer_se = Vec::with_capacity(q);
    for &c in &order {
        let is_r = |u: usize| ind(reference[u] == c);
        area.push(eq2(&is_r));
        let (_, _, v, _, _) = by_stratum(&is_r, &is_r);
        area_se.push(eq25(&v).sqrt());
        let ratio = |y: &dyn Fn(usize) -> f64, x: &dyn Fn(usize) -> f64| {
            let (my, mx, vy, vx, cov) = by_stratum(y, x);
            let num: f64 = (0..kept.len()).map(|h| my[h] * big_n[h]).sum();
            let den: f64 = (0..kept.len()).map(|h| mx[h] * big_n[h]).sum();
            let r = num / den;
            (r, eq28(r, den, &vy, &vx, &cov).sqrt())
        };
        let (u, use_) = ratio(&|u| ind(map[u] == c && map[u] == reference[u]), &|u| ind(map[u] == c));
        user.push(u);
        user_se.push(use_);
        let (p, pse) = ratio(&|u| ind(reference[u] == c && reference[u] == map[u]), &|u| ind(reference[u] == c));
        producer.push(p);
        producer_se.push(pse);
    }
    let matrix = order
        .iter()
        .map(|&i| {
            order
                .iter()
                .map(|&j| {
                    let v = eq2(&|u| ind(map[u] == i && reference[u] == j));
                    if v == 0.0 { f64::NAN } else { v }
                })
                .collect()
        })
        .collect();
    Ok(Estimates {
        classes: order,
        overall,
        overall_se,
        user,
        user_se,
        producer,
        producer_se,
        area,
        area_se,
        matrix,
        total,
    })
}

/// The standard normal's quantile (Wichura's AS 241, as R's qnorm).
#[allow(clippy::excessive_precision)]
pub fn qnorm(p: f64) -> f64 {
    let q = p - 0.5;
    if q.abs() <= 0.425 {
        let r = 0.180625 - q * q;
        return q
            * (((((((r * 2509.0809287301226727 + 33430.575583588128105) * r + 67265.770927008700853) * r
                + 45921.953931549871457)
                * r
                + 13731.693765509461125)
                * r
                + 1971.5909503065514427)
                * r
                + 133.14166789178437745)
                * r
                + 3.387132872796366608)
            / (((((((r * 5226.495278852545925 + 28729.085735721942674) * r + 39307.89580009271061) * r
                + 21213.794301586595867)
                * r
                + 5394.1960214247511077)
                * r
                + 687.1870074920579083)
                * r
                + 42.313330701600911252)
                * r
                + 1.0);
    }
    let mut r = if q < 0.0 { p } else { 1.0 - p };
    r = (-r.ln()).sqrt();
    let val = if r <= 5.0 {
        r -= 1.6;
        (((((((r * 7.7454501427834140764e-4 + 0.0227238449892691845833) * r + 0.24178072517745061177) * r
            + 1.27045825245236838258)
            * r
            + 3.64784832476320460504)
            * r
            + 5.7694972214606914055)
            * r
            + 4.6303378461565452959)
            * r
            + 1.42343711074968357734)
            / (((((((r * 1.05075007164441684324e-9 + 5.475938084995344946e-4) * r + 0.0151986665636164571966) * r
                + 0.14810397642748007459)
                * r
                + 0.68976733498510000455)
                * r
                + 1.6763848301838038494)
                * r
                + 2.05319162663775882187)
                * r
                + 1.0)
    } else {
        r -= 5.0;
        (((((((r * 2.01033439929228813265e-7 + 2.71155556874348757815e-5) * r + 0.0012426609473880784386) * r
            + 0.026532189526576123093)
            * r
            + 0.29656057182850489123)
            * r
            + 1.7848265399172913358)
            * r
            + 5.4637849111641143699)
            * r
            + 6.6579046435011037772)
            / (((((((r * 2.04426310338993978564e-15 + 1.4215117583164458887e-7) * r + 1.8463183175100546818e-5) * r
                + 7.868691311456132591e-4)
                * r
                + 0.0148753612908506148525)
                * r
                + 0.13692988092273580531)
                * r
                + 0.59983220655588793769)
                * r
                + 1.0)
    };
    if q < 0.0 { -val } else { val }
}

// -- the operators ------------------------------------------------------------------------------------------------

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

const ALLOCATIONS: [&str; 4] = ["minimum", "proportional", "equal", "neyman"];

struct SampleSize;

impl Operator for SampleSize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "accuracy.sample_size",
            "The size of a stratified random sample for the accuracy and the area of a map, and its share of each \
             stratum (Olofsson et al. 2014): n = (Σ W_i S_i / S(Ô))², W_i a stratum's share of the map, S_i = \
             √(U_i (1 − U_i)) from the user's accuracy expected of it, S(Ô) the standard error wanted of the overall \
             accuracy. Shared with a minimum in every stratum (the good practice when some classes are rare, such \
             as change: the rest in proportion to the areas), in proportion, equally or by Neyman's allocation.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the strata: the classified map, or a map of strata"))
        .output(PortSpec::new("table", Type::Table, "a row per stratum: stratum, cells, weight, user_accuracy, sd, n"))
        .output(PortSpec::new("n", Type::Integer, "the size of the sample"))
        .param(PortSpec::optional(
            "user_accuracy",
            Type::Any,
            "the user's accuracy expected of each stratum: a number for all (default 0.9), or {class: accuracy} \
             (the others 0.9); lower for the classes hard to map, such as change (Olofsson's 0.6 to 0.7)",
        ))
        .param(PortSpec::optional(
            "standard_error",
            Type::Number,
            "the standard error wanted of the overall accuracy (default 0.01)",
        ))
        .param(PortSpec::optional("n", Type::Integer, "the size of the sample, instead of the formula's"))
        .param(
            PortSpec::optional("allocation", Type::Text, "how the sample is shared among the strata (default minimum)")
                .choices(ALLOCATIONS),
        )
        .param(PortSpec::optional(
            "minimum",
            Type::Integer,
            "the sample of every stratum at least, or all its cells (allocation minimum; default 50; Olofsson \
             suggests 50 to 100)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let (classes, cells) = class_cells(&map.data);
        if classes.is_empty() {
            return Err(fail(args, "the map has no cells with a value"));
        }
        let total: usize = cells.iter().sum();
        let weights: Vec<f64> = cells.iter().map(|&c| c as f64 / total as f64).collect();
        let ua =
            per_class(args.params.get("user_accuracy"), &classes, 0.9, "user_accuracy").map_err(|e| fail(args, e))?;
        if let Some(u) = ua.iter().find(|u| !(0.0..=1.0).contains(*u)) {
            return Err(fail(args, format!("user_accuracy: {u} is not between 0 and 1")));
        }
        let sd: Vec<f64> = ua.iter().map(|u| (u * (1.0 - u)).sqrt()).collect();
        let se: f64 = args.param("standard_error", 0.01)?;
        let n = match args.opt_param::<usize>("n")? {
            Some(n) => n,
            None => {
                if se <= 0.0 {
                    return Err(fail(args, "standard_error must be above 0"));
                }
                sample_size(&weights, &sd, se)
            }
        };
        let allocation: String = args.param("allocation", "minimum".to_string())?;
        let how = match allocation.as_str() {
            "minimum" => Allocation::Minimum(args.param("minimum", 50)?),
            "proportional" => Allocation::Proportional,
            "equal" => Allocation::Equal,
            "neyman" => Allocation::Neyman,
            other => return Err(fail(args, format!("allocation: {other:?} is not one of {ALLOCATIONS:?}"))),
        };
        let n = n.min(total);
        let (sizes, over) = allocate(n, &weights, &sd, &cells, how);
        if over {
            ctx.warn(format!(
                "the minimum of every stratum asks for {} points, more than the {n} of the sample",
                sizes.iter().sum::<usize>()
            ));
        }
        let mut t = Table::new(["stratum", "cells", "weight", "user_accuracy", "sd", "n"].map(String::from).to_vec());
        for i in 0..classes.len() {
            t.rows.push(vec![classes[i], cells[i] as f64, weights[i], ua[i], sd[i], sizes[i] as f64]);
        }
        let n: usize = sizes.iter().sum();
        Ok(outputs([("table", Value::Table(t)), ("n", Value::Integer(n as i64))]))
    }
}

/// A cell's centre on a grid.
fn centre(g: &Grid, row: usize, col: usize) -> [f64; 2] {
    let t = &g.transform;
    let (c, r) = (col as f64 + 0.5, row as f64 + 0.5);
    [t[0] + c * t[1] + r * t[2], t[3] + c * t[4] + r * t[5]]
}

/// The cell (row-major index) holding a point, none outside the grid.
fn cell_at(g: &Grid, p: [f64; 2]) -> Option<usize> {
    let t = &g.transform;
    let col = ((p[0] - t[0]) / t[1]).floor();
    let row = ((p[1] - t[3]) / t[5]).floor();
    (col >= 0.0 && row >= 0.0 && (col as usize) < g.width && (row as usize) < g.height)
        .then(|| row as usize * g.width + col as usize)
}

struct Sample;

impl Operator for Sample {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "accuracy.sample",
            "A stratified random sample of a map's cells for its accuracy and area (Olofsson et al. 2014): in every \
             stratum its share of cells drawn without replacement, as numpy's default_rng(seed).choice of the \
             stratum's cells, stratum after stratum (increasing); the points at the cells' centres, by stratum and \
             cell. Give each point its reference class (the column reference, from imagery or the field) and take \
             them to accuracy.assess.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the strata: the classified map, or a map of strata"))
        .input(PortSpec::optional(
            "sizes",
            Type::Table,
            "the sample of each stratum: columns stratum and n (accuracy.sample_size's table)",
        ))
        .output(PortSpec::new(
            "points",
            Type::Vector,
            "the points: id, stratum, row, col, x, y and reference (empty, to be filled)",
        ))
        .output(PortSpec::new("cells", Type::Map, "the stratum of the cells drawn, missing elsewhere"))
        .param(PortSpec::optional(
            "n",
            Type::Any,
            "the sample of each stratum instead of sizes: a number for all, or {stratum: n}",
        ))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of numpy's generator (default 1)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let (classes, cells) = class_cells(&map.data);
        let sizes: Vec<f64> = match (args.get("sizes"), args.params.get("n")) {
            (Some(_), Some(_)) => return Err(fail(args, "the sizes or n, not both")),
            (Some(Value::Table(t)), None) => {
                let (Some(s), Some(n)) = (t.column("stratum"), t.column("n")) else {
                    return Err(fail(args, "sizes: a table with the columns stratum and n"));
                };
                let mut out = vec![0.0; classes.len()];
                for (s, n) in s.iter().zip(&n) {
                    let i = classes
                        .iter()
                        .position(|c| c == s)
                        .ok_or_else(|| fail(args, format!("sizes: the map has no stratum {}", class_label(*s))))?;
                    out[i] = *n;
                }
                out
            }
            (Some(v), None) => return Err(fail(args, format!("sizes: a table, not a {:?}", v.ty()))),
            (None, n @ Some(_)) => per_class(n, &classes, 0.0, "n").map_err(|e| fail(args, e))?,
            (None, None) => return Err(fail(args, "connect the sizes (accuracy.sample_size) or give n")),
        };
        let seed: u64 = args.param("seed", 1)?;
        let mut rng = NumpyRng::new(seed);
        let mut drawn: Vec<(usize, usize)> = Vec::new();
        for (k, &c) in classes.iter().enumerate() {
            let n = sizes[k];
            if n < 0.0 || n.fract() != 0.0 {
                return Err(fail(args, format!("stratum {}: {n} is not a number of points", class_label(c))));
            }
            let n = n as usize;
            if n > cells[k] {
                return Err(fail(
                    args,
                    format!("stratum {}: {n} points, but it has {} cells", class_label(c), cells[k]),
                ));
            }
            if n == 0 {
                ctx.warn(format!("stratum {}: no points", class_label(c)));
                continue;
            }
            let idx: Vec<usize> = map.data.iter().enumerate().filter(|(_, v)| **v == c).map(|(i, _)| i).collect();
            let mut pick: Vec<usize> = rng.choice_distinct(idx.len(), n).into_iter().map(|j| idx[j]).collect();
            pick.sort_unstable();
            drawn.extend(pick.into_iter().map(|i| (k, i)));
        }
        let g = &map.grid;
        let mut out = vec![f64::NAN; map.data.len()];
        let mut cols: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
        let mut geometries = Vec::with_capacity(drawn.len());
        for (id, &(k, i)) in drawn.iter().enumerate() {
            let (row, col) = (i / g.width, i % g.width);
            let p = centre(g, row, col);
            out[i] = classes[k];
            geometries.push(Geometry::Point(p));
            for (name, v) in [
                ("id", (id + 1) as f64),
                ("stratum", classes[k]),
                ("row", row as f64),
                ("col", col as f64),
                ("x", p[0]),
                ("y", p[1]),
                ("reference", f64::NAN),
            ] {
                cols.entry(name).or_default().push(v);
            }
        }
        let attributes = ["id", "stratum", "row", "col", "x", "y", "reference"]
            .iter()
            .map(|&n| (n.to_string(), Column::Numbers(cols.remove(n).unwrap_or_default())))
            .collect();
        let v = Vector { geometries, attributes, crs: crate::prepare::crs_of(g) };
        Ok(outputs([("points", Value::Vector(Arc::new(v))), ("cells", Value::Map(Map::new(map.grid.clone(), out)))]))
    }
}

/// The units of a sample: the map's class, the reference class and the stratum of each.
struct Units {
    map: Vec<f64>,
    reference: Vec<f64>,
    strata: Vec<f64>,
}

/// The sample of `accuracy.assess`: points over the map (their reference column), or a table.
fn units_of(ctx: &mut Ctx, args: &Args, map: &Map, strata: Option<&Map>) -> Result<Units> {
    let column: String = args.param("reference", "reference".to_string())?;
    let mut u = Units { map: vec![], reference: vec![], strata: vec![] };
    match (args.get("points"), args.get("samples")) {
        (Some(_), Some(_)) => Err(fail(args, "the points or the samples (a table), not both")),
        (Some(_), None) => {
            let v = crate::vector::on_grid(crate::vector::vector_of(args, "points")?, &map.grid)?;
            let g = &map.grid;
            if g.transform[2] != 0.0 || g.transform[4] != 0.0 {
                return Err(fail(args, "the map's grid is rotated"));
            }
            let refs = v
                .column(&column)
                .ok_or_else(|| {
                    let names: Vec<&str> = v.attributes.iter().map(|(n, _)| n.as_str()).collect();
                    fail(args, format!("the points have no column {column:?} (they have {names:?})"))
                })?
                .numbers();
            let (mut unlabelled, mut outside) = (0, 0);
            let mut k = 0;
            for geom in &v.geometries {
                let p = match geom {
                    Geometry::Point(p) => *p,
                    Geometry::Empty => {
                        k += 1;
                        continue;
                    }
                    _ => return Err(fail(args, "the sample's features must be points")),
                };
                let r = refs[k];
                k += 1;
                if r.is_nan() {
                    unlabelled += 1;
                    continue;
                }
                let Some(i) = cell_at(g, p) else {
                    outside += 1;
                    continue;
                };
                let (m, s) = (map.data[i], strata.map_or(map.data[i], |s| s.data[i]));
                if m.is_nan() || s.is_nan() {
                    outside += 1;
                    continue;
                }
                u.map.push(m);
                u.reference.push(r);
                u.strata.push(s);
            }
            if unlabelled > 0 {
                ctx.warn(format!("{unlabelled} points without a reference class left out"));
            }
            if outside > 0 {
                ctx.warn(format!("{outside} points outside the map or on its missing cells left out"));
            }
            Ok(u)
        }
        (None, Some(Value::Table(t))) => {
            let need =
                |name: &str| t.column(name).ok_or_else(|| fail(args, format!("the samples have no column {name:?}")));
            let m = need("map")?;
            let r = need(&column)?;
            let s = if strata.is_some() { need("stratum")? } else { m.clone() };
            for k in 0..m.len() {
                if !(m[k].is_nan() || r[k].is_nan() || s[k].is_nan()) {
                    u.map.push(m[k]);
                    u.reference.push(r[k]);
                    u.strata.push(s[k]);
                }
            }
            Ok(u)
        }
        (None, Some(v)) => Err(fail(args, format!("samples: a table, not a {:?}", v.ty()))),
        (None, None) => Err(fail(args, "connect the sample: points labelled with their reference class, or a table")),
    }
}

struct Assess;

impl Operator for Assess {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "accuracy.assess",
            "The accuracy and the area of a map's classes from a stratified random sample labelled with the \
             reference class (Olofsson et al. 2014): the error matrix in shares of the area, the user's, \
             producer's and overall accuracies, and each class's area estimated from the sample instead of counted \
             on the map, with standard errors and confidence intervals; as R's mapaccuracy: olofsson when the \
             strata are the map's classes, stehman2014 for strata of their own (a map of strata, or the finite \
             population correction).",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the classified map: its class at each point and its cells by class"))
        .input(PortSpec::optional(
            "points",
            Type::Vector,
            "the sample's points with their reference class (accuracy.sample's, labelled)",
        ))
        .input(PortSpec::optional(
            "samples",
            Type::Table,
            "the sample as a table instead: columns map and reference (and stratum, with strata)",
        ))
        .input(PortSpec::optional(
            "strata",
            Type::Map,
            "the strata of the sample when they are not the map's classes (e.g. an older map, or a buffer around \
             change)",
        ))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per class: class, map_cells, samples, user_accuracy, user_se, user_ci, producer_accuracy, \
             producer_se, producer_ci, area_share, area_share_se, area_cells, area_cells_ci, area_ha, area_ha_ci \
             (the _ci the half-widths of the confidence intervals)",
        ))
        .output(PortSpec::new(
            "overall",
            Type::Table,
            "overall_accuracy, overall_se, overall_ci, samples, cells, confidence",
        ))
        .output(PortSpec::new(
            "matrix",
            Type::Table,
            "the error matrix in shares of the area: a row per class of the map, a column per reference class",
        ))
        .output(PortSpec::new("counts", Type::Table, "the error matrix in sample units"))
        .param(PortSpec::optional("reference", Type::Text, "the column of the reference class (default reference)"))
        .param(
            PortSpec::optional(
                "estimator",
                Type::Text,
                "olofsson (the strata are the map's classes), stehman (strata of their own, the finite population \
                 correction); auto (default): stehman with strata, else olofsson",
            )
            .choices(["auto", "olofsson", "stehman"]),
        )
        .param(PortSpec::optional("confidence", Type::Number, "the level of the confidence intervals (default 0.95)"))
        .param(PortSpec::optional(
            "cell_area",
            Type::Number,
            "a cell's area in m2 (default: the cells' sides)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let strata = args.opt_map("strata")?;
        if let Some(s) = strata
            && s.data.len() != map.data.len()
        {
            return Err(fail(args, "the strata and the map differ in size"));
        }
        let estimator: String = args.param("estimator", "auto".to_string())?;
        let stehman_ = match estimator.as_str() {
            "auto" => strata.is_some(),
            "olofsson" if strata.is_some() => {
                return Err(fail(
                    args,
                    "olofsson takes the map's classes as strata: leave the strata out, or use stehman",
                ));
            }
            "olofsson" => false,
            "stehman" => true,
            other => return Err(fail(args, format!("estimator: {other:?} is not auto, olofsson or stehman"))),
        };
        let confidence: f64 = args.param("confidence", 0.95)?;
        if !(confidence > 0.0 && confidence < 1.0) {
            return Err(fail(args, "confidence must be between 0 and 1"));
        }
        let z = qnorm(0.5 + confidence / 2.0);
        let (res_x, res_y) = map.grid.resolution();
        let cell_area: f64 = args.param("cell_area", res_x * res_y)?;
        let u = units_of(ctx, args, map, strata)?;
        if u.map.is_empty() {
            return Err(fail(args, "no sample unit has a reference class"));
        }
        let (classes, cells) = class_cells(&map.data);
        let cells_f: Vec<f64> = cells.iter().map(|&c| c as f64).collect();
        let e = if stehman_ {
            let (sc, scells) = match strata {
                Some(s) => class_cells(&s.data),
                None => (classes.clone(), cells.clone()),
            };
            let scells: Vec<f64> = scells.iter().map(|&c| c as f64).collect();
            for (h, &s) in sc.iter().enumerate() {
                if u.strata.iter().filter(|&&x| x == s).count() == 1 {
                    ctx.warn(format!("the stratum {} has a single sample unit", class_label(sc[h])));
                }
            }
            stehman(&u.strata, &u.map, &u.reference, &sc, &scells)
        } else {
            olofsson(&u.map, &u.reference, &classes, &cells_f)
        }
        .map_err(|m| fail(args, m))?;
        // the rows by class, increasing
        let mut idx: Vec<usize> = (0..e.classes.len()).collect();
        idx.sort_by(|&a, &b| e.classes[a].total_cmp(&e.classes[b]));
        let mut t = Table::new(
            [
                "class",
                "map_cells",
                "samples",
                "user_accuracy",
                "user_se",
                "user_ci",
                "producer_accuracy",
                "producer_se",
                "producer_ci",
                "area_share",
                "area_share_se",
                "area_cells",
                "area_cells_ci",
                "area_ha",
                "area_ha_ci",
            ]
            .map(String::from)
            .to_vec(),
        );
        for &j in &idx {
            let c = e.classes[j];
            let map_cells = classes.iter().position(|&x| x == c).map_or(0.0, |i| cells_f[i]);
            let samples = u.map.iter().filter(|&&m| m == c).count() as f64;
            let (a, a_ci) = (e.area[j] * e.total, z * e.area_se[j] * e.total);
            t.rows.push(vec![
                c,
                map_cells,
                samples,
                e.user[j],
                e.user_se[j],
                z * e.user_se[j],
                e.producer[j],
                e.producer_se[j],
                z * e.producer_se[j],
                e.area[j],
                e.area_se[j],
                a,
                a_ci,
                a * cell_area / 10_000.0,
                a_ci * cell_area / 10_000.0,
            ]);
        }
        let mut overall = Table::new(
            ["overall_accuracy", "overall_se", "overall_ci", "samples", "cells", "confidence"]
                .map(String::from)
                .to_vec(),
        );
        overall.rows.push(vec![e.overall, e.overall_se, z * e.overall_se, u.map.len() as f64, e.total, confidence]);
        let header = std::iter::once("map".to_string())
            .chain(idx.iter().map(|&j| class_label(e.classes[j])))
            .collect::<Vec<_>>();
        let mut matrix = Table::new(header.clone());
        let mut counts = Table::new(header);
        for &i in &idx {
            let ci = e.classes[i];
            matrix.rows.push(std::iter::once(ci).chain(idx.iter().map(|&j| e.matrix[i][j])).collect());
            counts.rows.push(
                std::iter::once(ci)
                    .chain(idx.iter().map(|&j| {
                        let cj = e.classes[j];
                        u.map.iter().zip(&u.reference).filter(|&(&m, &r)| m == ci && r == cj).count() as f64
                    }))
                    .collect(),
            );
        }
        Ok(outputs([
            ("table", Value::Table(t)),
            ("overall", Value::Table(overall)),
            ("matrix", Value::Table(matrix)),
            ("counts", Value::Table(counts)),
        ]))
    }
}
