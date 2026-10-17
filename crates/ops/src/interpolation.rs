//! Interpolation of points onto a map, as R's gstat: the sample variogram of the points and the fit of a model to
//! it (`stats.variogram`: the classical estimator in distance classes; fit.variogram's iterated weighted least
//! squares, N/h² weights, Gauss-Newton steps of 20% at most), inverse distance weighting (`stats.idw`) and ordinary
//! kriging (`stats.kriging`: the prediction and its variance), with every point or the nearest.

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry};

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Variogram));
    r.add(Arc::new(Idw));
    r.add(Arc::new(Kriging));
}

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

// -- the variogram ----------------------------------------------------------------------------------------------------

/// A variogram's shape: spherical, exponential (its range the distance parameter, the practical range 3 times
/// it) or Gaussian (the practical range √3 times it), as gstat's Sph, Exp and Gau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Spherical,
    Exponential,
    Gaussian,
}

impl Shape {
    pub fn parse(s: &str) -> Option<Shape> {
        Some(match s {
            "spherical" | "sph" | "Sph" => Shape::Spherical,
            "exponential" | "exp" | "Exp" => Shape::Exponential,
            "gaussian" | "gau" | "Gau" => Shape::Gaussian,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Shape::Spherical => "spherical",
            Shape::Exponential => "exponential",
            Shape::Gaussian => "gaussian",
        }
    }

    /// The semivariance of a unit sill at a distance (gstat's fn_*).
    fn unit(self, h: f64, r: f64) -> f64 {
        if h == 0.0 {
            return 0.0;
        }
        match self {
            Shape::Spherical => {
                if h >= r {
                    return 1.0;
                }
                let hr = h / r;
                hr * (1.5 - 0.5 * hr * hr)
            }
            Shape::Exponential => 1.0 - (-h / r).exp(),
            Shape::Gaussian => {
                let hr = h / r;
                1.0 - (-(hr * hr)).exp()
            }
        }
    }

    /// Its derivative by the range (gstat's da_fn_*).
    fn d_range(self, h: f64, r: f64) -> f64 {
        match self {
            Shape::Spherical => {
                if h > r {
                    return 0.0;
                }
                let hr2 = h / (r * r);
                1.5 * hr2 * (-1.0 + h * hr2)
            }
            Shape::Exponential => {
                let hr = -h / r;
                (hr / r) * hr.exp()
            }
            Shape::Gaussian => {
                let hr = h / r;
                (-hr / r) * (-(hr * hr)).exp()
            }
        }
    }
}

/// A variogram model: a nugget and a structure of a shape, its partial sill and range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    pub shape: Shape,
    pub nugget: f64,
    pub psill: f64,
    pub range: f64,
}

impl Model {
    /// The semivariance at a distance (gstat's get_semivariance: the sills beyond a bounded model's range).
    pub fn gamma(&self, h: f64) -> f64 {
        if self.shape == Shape::Spherical && h > self.range.max(0.0) {
            return self.nugget + self.psill;
        }
        let nug = if h == 0.0 { 0.0 } else { 1.0 };
        self.nugget * nug + self.psill * self.shape.unit(h, self.range)
    }

    /// The covariance at a distance: the sills less the semivariance.
    pub fn covariance(&self, h: f64) -> f64 {
        self.nugget + self.psill - self.gamma(h)
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"model": self.shape.name(), "nugget": self.nugget, "psill": self.psill, "range": self.range})
    }

    pub fn from_json(v: &serde_json::Value) -> Option<Model> {
        Some(Model {
            shape: Shape::parse(v.get("model")?.as_str()?)?,
            nugget: v.get("nugget").and_then(|x| x.as_f64()).unwrap_or(0.0),
            psill: v.get("psill")?.as_f64()?,
            range: v.get("range")?.as_f64()?,
        })
    }
}

/// A sample variogram: per distance class with pairs, their number, mean distance and semivariance.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sample {
    pub np: Vec<f64>,
    pub dist: Vec<f64>,
    pub gamma: Vec<f64>,
}

/// gstat's default cutoff: a third of the diagonal of the points' bounding box.
pub fn default_cutoff(xy: &[[f64; 2]]) -> f64 {
    let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for p in xy {
        (x0, x1, y0, y1) = (x0.min(p[0]), x1.max(p[0]), y0.min(p[1]), y1.max(p[1]));
    }
    ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt() / 3.0
}

/// The classical (Matheron's) sample variogram of points up to a cutoff in classes of a width, as gstat's
/// variogram(): the pairs at a distance (lower, upper] of a class, half the mean squared difference.
pub fn sample_variogram(xy: &[[f64; 2]], z: &[f64], cutoff: f64, width: f64) -> Sample {
    let n_est = (cutoff / width).ceil().max(1.0) as usize;
    let (mut nh, mut dist, mut gamma) = (vec![0usize; n_est], vec![0.0; n_est], vec![0.0; n_est]);
    for i in 0..xy.len() {
        for j in 0..i {
            let (dx, dy) = (xy[i][0] - xy[j][0], xy[i][1] - xy[j][1]);
            let d = (dx * dx + dy * dy).sqrt();
            if d > cutoff {
                continue;
            }
            let frac = d / width;
            let k = if d > 0.0 && frac == frac.floor() { frac as usize - 1 } else { frac.floor() as usize };
            let k = k.min(n_est - 1);
            gamma[k] += (z[i] - z[j]).powi(2);
            dist[k] += d;
            nh[k] += 1;
        }
    }
    let mut s = Sample::default();
    for k in (0..n_est).filter(|&k| nh[k] > 0) {
        s.np.push(nh[k] as f64);
        s.dist.push(dist[k] / nh[k] as f64);
        s.gamma.push(gamma[k] / (2.0 * nh[k] as f64));
    }
    s
}

/// A fit's model, its weighted sum of squared errors, and whether it was singular or did not converge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub model: Model,
    pub sserr: f64,
    pub singular: bool,
    pub converged: bool,
}

/// Cholesky's solution of a small symmetric system, none when it is not positive definite.
fn cholesky_solve(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let mut d = a[j * n + j];
        for k in 0..j {
            d -= l[j * n + k] * l[j * n + k];
        }
        if d <= 0.0 || d.is_nan() {
            return None;
        }
        let d = d.sqrt();
        l[j * n + j] = d;
        for i in j + 1..n {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            l[i * n + j] = s / d;
        }
    }
    let mut y = b.to_vec();
    for i in 0..n {
        for k in 0..i {
            y[i] -= l[i * n + k] * y[k];
        }
        y[i] /= l[i * n + i];
    }
    // (LAPACK's dpotrs: the back substitution subtracts the last unknowns first)
    for i in (0..n).rev() {
        for k in (i + 1..n).rev() {
            y[i] -= l[k * n + i] * y[k];
        }
        y[i] /= l[i * n + i];
    }
    Some(y)
}

/// gstat's wls_fit of a model to a sample variogram (fit.method 7: weights N/h²): Gauss-Newton steps (the sills
/// and the range of those fitted), a step whose squared length passes 0.2 of the parameters' shrunk by that ratio, until the sum
/// of squares falls by less than 1e-5 of itself after a free step, or 200 iterations.
fn wls_fit(s: &Sample, mut m: Model, fit_nugget: bool, fit_psill: bool, fit_range: bool) -> Fit {
    // the parameters fitted: 0 the nugget, 1 the partial sill, 2 the range (gstat's order)
    let fitted: Vec<usize> =
        [(0, fit_nugget), (1, fit_psill), (2, fit_range)].iter().filter(|(_, f)| *f).map(|(k, _)| *k).collect();
    if fit_nugget && m.nugget == 0.0 {
        m.nugget = 1.0;
    }
    if fit_psill && m.psill == 0.0 {
        m.psill = 1.0;
    }
    let sserr = |m: &Model| -> f64 {
        (0..s.dist.len()).map(|i| (s.gamma[i] - m.gamma(s.dist[i])).powi(2) * (s.np[i] / (s.dist[i] * s.dist[i]))).sum()
    };
    let p = fitted.len();
    if p == 0 || s.dist.is_empty() {
        return Fit { model: m, sserr: sserr(&m), singular: s.dist.is_empty(), converged: true };
    }
    let w: Vec<f64> = (0..s.dist.len()).map(|i| (s.np[i] / (s.dist[i] * s.dist[i])).sqrt()).collect();
    let mut old = f64::MAX;
    for iter in 1..=200 {
        let get = |m: &Model, k: usize| [m.nugget, m.psill, m.range][k];
        let start: Vec<f64> = fitted.iter().map(|&k| get(&m, k)).collect();
        let (mut a, mut b) = (vec![0.0; p * p], vec![0.0; p]);
        if s.dist.len() < p {
            return Fit { model: m, sserr: sserr(&m), singular: true, converged: false };
        }
        for (i, &h) in s.dist.iter().enumerate() {
            let y = (s.gamma[i] - m.gamma(h)) * w[i];
            let x: Vec<f64> = fitted
                .iter()
                .map(|&k| {
                    w[i] * match k {
                        0 => {
                            if h == 0.0 {
                                0.0
                            } else {
                                1.0
                            }
                        }
                        1 => m.shape.unit(h, m.range),
                        _ => m.psill * m.shape.d_range(h, m.range),
                    }
                })
                .collect();
            for r in 0..p {
                b[r] += x[r] * y;
                for c in 0..p {
                    a[r * p + c] += x[r] * x[c];
                }
            }
        }
        let Some(mut beta) = cholesky_solve(&a, &b, p) else {
            return Fit { model: m, sserr: sserr(&m), singular: true, converged: false };
        };
        let mut bounded = false;
        if fit_range {
            // (gstat's v_norm2: the sum of the squares)
            let norm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>();
            let ratio = norm(&beta) / norm(&start);
            if ratio > 0.2 {
                beta.iter_mut().for_each(|v| *v *= 0.2 / ratio);
                bounded = true;
            }
        }
        for (j, &k) in fitted.iter().enumerate() {
            let v = start[j] + beta[j];
            match k {
                0 => m.nugget = v,
                1 => m.psill = v,
                _ => m.range = v,
            }
        }
        let e = sserr(&m);
        let mut step = old - e;
        if e > f64::EPSILON * 10.0 {
            step /= e;
        }
        old = e;
        if (0.0..1e-5).contains(&step) && !bounded {
            return Fit { model: m, sserr: e, singular: false, converged: true };
        }
        if iter == 200 {
            return Fit { model: m, sserr: e, singular: false, converged: false };
        }
    }
    unreachable!()
}

/// gstat's fit.variogram: the fit, and a negative sill fixed at zero and the fit made again from the initial range.
/// (Its derivative of the Gaussian by the range is gstat's, -h/r² exp(-(h/r)²).)
pub fn fit_variogram(s: &Sample, init: Model, fit_nugget: bool, fit_psill: bool, fit_range: bool) -> Fit {
    let f = wls_fit(s, init, fit_nugget, fit_psill, fit_range);
    if f.singular || (f.model.nugget >= 0.0 && f.model.psill >= 0.0) {
        return f;
    }
    let mut m = f.model;
    // (R's fit.sills = model$psill > 0: the positive sills fitted again)
    let (fit_nugget, fit_psill) = (m.nugget > 0.0, m.psill > 0.0);
    m.nugget = m.nugget.max(0.0);
    m.psill = m.psill.max(0.0);
    m.range = init.range;
    fit_variogram(s, m, fit_nugget, fit_psill, fit_range)
}

// -- the points -------------------------------------------------------------------------------------------------------

/// The points of a node's port (point features, or the first point of multipoints) with a value of an attribute,
/// in the grid's coordinate system when a grid is given; those without a value left out.
fn points_of(args: &Args, grid: Option<&Grid>) -> Result<(Vec<[f64; 2]>, Vec<f64>)> {
    let v = crate::vector::vector_of(args, "points")?;
    let v = match grid {
        Some(g) => crate::vector::on_grid(v, g)?,
        None => v.clone(),
    };
    let field: String = args.param("field", String::new())?;
    let values: Vec<f64> = if field.is_empty() {
        let numeric: Vec<&String> =
            v.attributes.iter().filter(|(_, c)| matches!(c, Column::Numbers(_))).map(|(n, _)| n).collect();
        match numeric.as_slice() {
            [one] => v.column(one).unwrap().numbers(),
            _ => return Err(fail(args, "field: the attribute of the values")),
        }
    } else {
        v.column(&field).ok_or_else(|| fail(args, format!("field: no attribute {field:?}")))?.numbers()
    };
    let (mut xy, mut z) = (Vec::new(), Vec::new());
    for (g, val) in v.geometries.iter().zip(values) {
        let p = match g {
            Geometry::Point(p) => *p,
            Geometry::MultiPoint(ps) if !ps.is_empty() => ps[0],
            _ => continue,
        };
        if !val.is_nan() {
            xy.push(p);
            z.push(val);
        }
    }
    if xy.len() < 2 {
        return Err(fail(args, "points: two with a value at least"));
    }
    Ok((xy, z))
}

/// The centres of the cells of a map with a value.
fn targets(m: &Map) -> Vec<(usize, [f64; 2])> {
    let t = m.grid.transform;
    let nx = m.grid.width;
    (0..m.data.len())
        .filter(|&i| !m.data[i].is_nan())
        .map(|i| {
            let (r, c) = ((i / nx) as f64, (i % nx) as f64);
            (i, [t[0] + (c + 0.5) * t[1] + (r + 0.5) * t[2], t[3] + (c + 0.5) * t[4] + (r + 0.5) * t[5]])
        })
        .collect()
}

/// The indices of the `nmax` points nearest a place (all of them, in order, without nmax).
fn nearest(xy: &[[f64; 2]], at: [f64; 2], nmax: Option<usize>) -> Vec<usize> {
    let d2 = |i: usize| (xy[i][0] - at[0]).powi(2) + (xy[i][1] - at[1]).powi(2);
    match nmax {
        Some(k) if k < xy.len() => {
            let mut idx: Vec<usize> = (0..xy.len()).collect();
            idx.sort_by(|a, b| d2(*a).total_cmp(&d2(*b)).then(a.cmp(b)));
            idx.truncate(k);
            idx
        }
        _ => (0..xy.len()).collect(),
    }
}

/// gstat's inverse distance weighting at a place: the values weighted by the distance to the power, the value of
/// a point at the place itself.
pub fn idw_at(xy: &[[f64; 2]], z: &[f64], at: [f64; 2], power: f64, nmax: Option<usize>) -> f64 {
    let sel = nearest(xy, at, nmax);
    if sel.len() == 1 {
        return z[sel[0]];
    }
    let (mut sw, mut sv) = (0.0, 0.0);
    for &j in &sel {
        let d2 = (xy[j][0] - at[0]).powi(2) + (xy[j][1] - at[1]).powi(2);
        if d2 == 0.0 {
            return z[j];
        }
        let w = if power == 2.0 { 1.0 / d2 } else { d2.powf(-0.5 * power) };
        sw += w;
        sv += w * z[j];
    }
    sv / sw
}

/// The lower triangle of Cholesky's factor of a symmetric matrix, none when it is not positive definite.
fn cholesky(a: &mut [f64], n: usize) -> Option<()> {
    for j in 0..n {
        let mut d = a[j * n + j];
        for k in 0..j {
            d -= a[j * n + k] * a[j * n + k];
        }
        if d <= 0.0 || d.is_nan() {
            return None;
        }
        let d = d.sqrt();
        a[j * n + j] = d;
        for i in j + 1..n {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= a[i * n + k] * a[j * n + k];
            }
            a[i * n + j] = s / d;
        }
    }
    Some(())
}

/// L y = b in place.
fn forward(l: &[f64], n: usize, y: &mut [f64]) {
    for i in 0..n {
        for k in 0..i {
            y[i] -= l[i * n + k] * y[k];
        }
        y[i] /= l[i * n + i];
    }
}

/// Lᵀ x = y in place.
fn backward(l: &[f64], n: usize, y: &mut [f64]) {
    for i in (0..n).rev() {
        for k in i + 1..n {
            y[i] -= l[k * n + i] * y[k];
        }
        y[i] /= l[i * n + i];
    }
}

/// Ordinary kriging of points with a model, ready for every place: the covariances among the points factored, the
/// mean's estimate (generalised least squares) and the residuals' weights.
struct Kriger {
    l: Vec<f64>,
    /// C⁻¹ 1
    ci1: Vec<f64>,
    /// 1ᵀ C⁻¹ 1
    s: f64,
    mean: f64,
    /// C⁻¹ (z - mean)
    r: Vec<f64>,
}

impl Kriger {
    fn new(xy: &[[f64; 2]], z: &[f64], m: &Model) -> Option<Kriger> {
        let n = xy.len();
        let mut l = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let d = ((xy[i][0] - xy[j][0]).powi(2) + (xy[i][1] - xy[j][1]).powi(2)).sqrt();
                l[i * n + j] = m.covariance(d);
            }
        }
        cholesky(&mut l, n)?;
        let mut ci1 = vec![1.0; n];
        forward(&l, n, &mut ci1);
        backward(&l, n, &mut ci1);
        let s: f64 = ci1.iter().sum();
        let mean = ci1.iter().zip(z).map(|(a, b)| a * b).sum::<f64>() / s;
        let mut r: Vec<f64> = z.iter().map(|v| v - mean).collect();
        forward(&l, n, &mut r);
        backward(&l, n, &mut r);
        Some(Kriger { l, ci1, s, mean, r })
    }

    /// The prediction and its variance at a place.
    fn at(&self, xy: &[[f64; 2]], m: &Model, p: [f64; 2]) -> (f64, f64) {
        let n = xy.len();
        let c0: Vec<f64> =
            xy.iter().map(|q| m.covariance(((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt())).collect();
        let pred = self.mean + c0.iter().zip(&self.r).map(|(a, b)| a * b).sum::<f64>();
        let mut y = c0.clone();
        forward(&self.l, n, &mut y);
        let cc: f64 = y.iter().map(|v| v * v).sum();
        let u = 1.0 - self.ci1.iter().zip(&c0).map(|(a, b)| a * b).sum::<f64>();
        (pred, m.covariance(0.0) - cc + u * u / self.s)
    }
}

/// Ordinary kriging at places: with every point (the system factored once) or the `nmax` nearest each.
pub fn krige(xy: &[[f64; 2]], z: &[f64], m: &Model, at: &[[f64; 2]], nmax: Option<usize>) -> Option<Vec<(f64, f64)>> {
    match nmax {
        Some(k) if k < xy.len() => at
            .par_iter()
            .map(|p| {
                let sel = nearest(xy, *p, Some(k));
                let (sxy, sz): (Vec<[f64; 2]>, Vec<f64>) = sel.iter().map(|&i| (xy[i], z[i])).unzip();
                Some(Kriger::new(&sxy, &sz, m)?.at(&sxy, m, *p))
            })
            .collect(),
        _ => {
            let k = Kriger::new(xy, z, m)?;
            Some(at.par_iter().map(|p| k.at(xy, m, *p)).collect())
        }
    }
}

// -- the operators ----------------------------------------------------------------------------------------------------

fn points_port(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new("points", Type::Vector, "the points, their values in an attribute"))
        .param(PortSpec::optional("field", Type::Text, "the attribute of the values (default the only number)"))
}

fn shape_of(args: &Args) -> Result<Option<Shape>> {
    let s: String = args.param("model", "spherical".to_string())?;
    if s == "none" {
        return Ok(None);
    }
    Shape::parse(&s)
        .map(Some)
        .ok_or_else(|| fail(args, format!("model: spherical, exponential, gaussian or none, not {s:?}")))
}

struct Variogram;

impl Operator for Variogram {
    fn signature(&self) -> OpSignature {
        points_port(OpSignature::new(
            "stats.variogram",
            "The sample variogram of points and a model fitted to it, as R's gstat (variogram and fit.variogram, \
             to rounding): half the mean squared difference of the pairs of points in classes of distance up to a \
             cutoff; a nugget and a spherical, exponential or Gaussian structure fitted by weighted least squares \
             (the pairs over the squared distance as weights; a negative sill fixed at zero), for stats.kriging.",
            Access::None,
        ))
        .output(PortSpec::new("table", Type::Table, "a row per class with pairs: np, dist, gamma, model"))
        .output(PortSpec::new("model", Type::Any, "the model fitted: {model, nugget, psill, range}"))
        .param(PortSpec::optional("cutoff", Type::Number, "the largest distance (default a third of the diagonal)"))
        .param(PortSpec::optional("width", Type::Number, "the classes' width (default the cutoff / 15)"))
        .param(
            PortSpec::optional("model", Type::Text, "the structure: spherical (default), exponential, gaussian, none")
                .choices(["spherical", "exponential", "gaussian", "none"]),
        )
        .param(PortSpec::optional("psill", Type::Number, "the partial sill to start from (default the variance)"))
        .param(PortSpec::optional("range", Type::Number, "the range to start from (default the cutoff / 3)"))
        .param(PortSpec::optional("nugget", Type::Number, "the nugget to start from (default the variance / 10)"))
        .param(PortSpec::optional("fit_nugget", Type::Bool, "fit the nugget (default true), else keep it"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (xy, z) = points_of(args, None)?;
        let cutoff: f64 = args.opt_param("cutoff")?.unwrap_or_else(|| default_cutoff(&xy));
        let width: f64 = args.opt_param("width")?.unwrap_or(cutoff / 15.0);
        if !(cutoff > 0.0 && width > 0.0) {
            return Err(fail(args, "cutoff and width: above 0"));
        }
        let s = sample_variogram(&xy, &z, cutoff, width);
        let n = z.len() as f64;
        let mean = z.iter().sum::<f64>() / n;
        let var = z.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let mut t = Table::new(["np", "dist", "gamma", "model"].map(String::from).to_vec());
        let mut out = Vec::new();
        let fitted = match shape_of(args)? {
            Some(shape) => {
                let init = Model {
                    shape,
                    nugget: args.opt_param("nugget")?.unwrap_or(var / 10.0),
                    psill: args.opt_param("psill")?.unwrap_or(var),
                    range: args.opt_param("range")?.unwrap_or(cutoff / 3.0),
                };
                let f = fit_variogram(&s, init, args.param("fit_nugget", true)?, true, true);
                if f.singular {
                    return Err(fail(args, "the fit is singular: try other starting values, or another model"));
                }
                if !f.converged {
                    eprintln!("node {:?}: the variogram's fit did not converge in 200 iterations", args.node);
                }
                out.push(("model", Value::Json(f.model.to_json())));
                Some(f.model)
            }
            None => None,
        };
        for k in 0..s.dist.len() {
            let g = fitted.map_or(f64::NAN, |m| m.gamma(s.dist[k]));
            t.rows.push(vec![s.np[k], s.dist[k], s.gamma[k], g]);
        }
        out.push(("table", Value::Table(t)));
        Ok(out.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

/// The cells a map of the result has: the cells with a value of `like`.
fn like_of<'a>(args: &'a Args) -> Result<&'a Map> {
    args.map("like")
}

fn nmax_of(args: &Args) -> Result<Option<usize>> {
    match args.opt_param::<usize>("nmax")? {
        Some(0) => Err(fail(args, "nmax: 1 or more")),
        n => Ok(n),
    }
}

struct Idw;

impl Operator for Idw {
    fn signature(&self) -> OpSignature {
        points_port(OpSignature::new(
            "stats.idw",
            "Inverse distance weighting of points onto a map, as R's gstat (idw, to rounding): every cell the mean of \
             the points' values weighted by one over the distance to a power, with every point or the nearest; a \
             point's value at its own place.",
            Access::Global,
        ))
        .input(PortSpec::new("like", Type::Map, "the map whose cells to fill (those with a value)"))
        .output(PortSpec::new("map", Type::Map, "the values interpolated"))
        .param(PortSpec::optional("power", Type::Number, "the distance's power (default 2)"))
        .param(PortSpec::optional("nmax", Type::Integer, "the nearest points used (default all)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let like = like_of(args)?;
        let (xy, z) = points_of(args, Some(&like.grid))?;
        let power: f64 = args.param("power", 2.0)?;
        if power < 0.0 {
            return Err(fail(args, "power: 0 or more"));
        }
        let nmax = nmax_of(args)?;
        let cells = targets(like);
        let values: Vec<f64> = cells.par_iter().map(|(_, p)| idw_at(&xy, &z, *p, power, nmax)).collect();
        let mut out = vec![f64::NAN; like.data.len()];
        for ((i, _), v) in cells.iter().zip(values) {
            out[*i] = v;
        }
        Ok(outputs([("map", Value::Map(Map::new(like.grid.clone(), out)))]))
    }
}

struct Kriging;

impl Operator for Kriging {
    fn signature(&self) -> OpSignature {
        points_port(OpSignature::new(
            "stats.kriging",
            "Ordinary kriging of points onto a map, as R's gstat (krige, to rounding): every cell the best linear \
             unbiased prediction from the points' values under a variogram model (from stats.variogram, or given), \
             with every point or the nearest, and the prediction's variance.",
            Access::Global,
        ))
        .input(PortSpec::new("like", Type::Map, "the map whose cells to fill (those with a value)"))
        .input(PortSpec::optional("model", Type::Any, "the variogram model (stats.variogram's model)"))
        .output(PortSpec::new("prediction", Type::Map, "the values predicted"))
        .output(PortSpec::new("variance", Type::Map, "the prediction's variance"))
        .param(PortSpec::optional("nmax", Type::Integer, "the nearest points used (default all)"))
        .param(
            PortSpec::optional("shape", Type::Text, "without a model input: spherical, exponential or gaussian")
                .choices(["spherical", "exponential", "gaussian"]),
        )
        .param(PortSpec::optional("psill", Type::Number, "without a model input: the partial sill"))
        .param(PortSpec::optional("range", Type::Number, "without a model input: the range"))
        .param(PortSpec::optional("nugget", Type::Number, "without a model input: the nugget (default 0)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let like = like_of(args)?;
        let (xy, z) = points_of(args, Some(&like.grid))?;
        let model = match args.get("model") {
            Some(Value::Json(v)) => {
                Model::from_json(v).ok_or_else(|| fail(args, "model: {model, nugget, psill, range}"))?
            }
            Some(v) => return Err(fail(args, format!("model: stats.variogram's model, not a {:?}", v.ty()))),
            None => {
                let s: String = args.param("shape", "spherical".to_string())?;
                Model {
                    shape: Shape::parse(&s).ok_or_else(|| fail(args, format!("shape: not {s:?}")))?,
                    nugget: args.param("nugget", 0.0)?,
                    psill: args.opt_param("psill")?.ok_or_else(|| fail(args, "connect a model, or give its psill"))?,
                    range: args.opt_param("range")?.ok_or_else(|| fail(args, "connect a model, or give its range"))?,
                }
            }
        };
        if !(model.range > 0.0 && model.psill >= 0.0 && model.nugget >= 0.0 && model.psill + model.nugget > 0.0) {
            return Err(fail(args, "model: a range above 0 and sills of 0 or more, not all 0"));
        }
        let cells = targets(like);
        let places: Vec<[f64; 2]> = cells.iter().map(|(_, p)| *p).collect();
        let k = krige(&xy, &z, &model, &places, nmax_of(args)?).ok_or_else(|| {
            fail(
                args,
                "the covariances of the points are singular (points at one place, or no nugget among neighbours)",
            )
        })?;
        let (mut pred, mut var) = (vec![f64::NAN; like.data.len()], vec![f64::NAN; like.data.len()]);
        for ((i, _), (p, v)) in cells.iter().zip(k) {
            pred[*i] = p;
            // (0 at a point's own place, a rounding below it taken as 0)
            var[*i] = v.max(0.0);
        }
        Ok(outputs([
            ("prediction", Value::Map(Map::new(like.grid.clone(), pred))),
            ("variance", Value::Map(Map::new(like.grid.clone(), var))),
        ]))
    }
}
