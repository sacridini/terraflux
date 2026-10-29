//! Step selection (amt's make_distribution, fit_distr, random_numbers, random_steps and extract_covariates): the
//! distributions of step lengths and turn angles, random steps from each observed step's start, and the values of
//! rasters at the ends of steps.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::amt::{is_steps, numbers};
use super::{BURST, codes, fail, give, is_lonlat, track_column, tracks_of};
use crate::sdm::points::{grid_cells, to_crs};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Make));
    r.add(Arc::new(Fit));
    r.add(Arc::new(RandomNumbers));
    r.add(Arc::new(RandomSteps));
    r.add(Arc::new(Covariates));
}

/// The parameters of each distribution and amt's defaults, in the order they are drawn with.
const DEFAULTS: [(&str, &[(&str, f64)]); 6] = [
    ("exp", &[("rate", 1.0)]),
    ("gamma", &[("shape", 1.0), ("scale", 1.0)]),
    ("hnorm", &[("sd", 1.0)]),
    ("lnorm", &[("meanlog", 0.0), ("sdlog", 1.0)]),
    ("unif", &[("min", -std::f64::consts::PI), ("max", std::f64::consts::PI)]),
    ("vonmises", &[("kappa", 1.0), ("mu", 0.0)]),
];

fn defaults(name: &str) -> Option<&'static [(&'static str, f64)]> {
    DEFAULTS.iter().find(|(n, _)| *n == name).map(|(_, p)| *p)
}

/// A distribution of step lengths or turn angles (amt's amt_distr): its name, its parameters by name and the
/// variance-covariance of the fitted ones (their names, row-major), when known.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    pub name: String,
    pub params: Vec<(String, f64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcov: Option<(Vec<String>, Vec<f64>)>,
}

impl Distribution {
    fn get(&self, p: &str) -> Option<f64> {
        self.params.iter().find(|(n, _)| n == p).map(|(_, v)| *v)
    }

    /// The parameters as amt's random_numbers draws with them (gamma: shape, scale; von Mises: kappa, mu).
    pub fn draw_params(&self) -> Vec<f64> {
        match self.name.as_str() {
            "gamma" => {
                let scale = self.get("scale").unwrap_or_else(|| 1.0 / self.get("rate").unwrap_or(1.0));
                vec![self.get("shape").unwrap_or(1.0), scale]
            }
            "vonmises" => vec![self.get("kappa").unwrap_or(1.0), self.get("mu").unwrap_or(0.0)],
            n => defaults(n).unwrap_or(&[]).iter().map(|(k, d)| self.get(k).unwrap_or(*d)).collect(),
        }
    }

    /// Its parameters as a table of one row.
    pub fn table(&self) -> Table {
        let mut t = Table::new(self.params.iter().map(|(n, _)| n.clone()).collect());
        t.rows.push(self.params.iter().map(|(_, v)| *v).collect());
        t
    }
}

/// amt's make_distribution: the parameters given, the others amt's defaults.
pub fn make(name: &str, given: &[(String, f64)]) -> std::result::Result<Distribution, String> {
    let names: Vec<&str> = DEFAULTS.iter().map(|(n, _)| *n).collect();
    let d = defaults(name).ok_or_else(|| format!("{name:?} is not implemented: one of {}", names.join(", ")))?;
    let has = |k: &str| given.iter().find(|(n, _)| n == k).map(|(_, v)| *v);
    let (known, params): (Vec<&str>, Vec<(String, f64)>) = if name == "gamma" && has("rate").is_some() {
        if has("scale").is_some() {
            return Err("gamma: scale or rate, not both".into());
        }
        (
            vec!["shape", "rate"],
            vec![("shape".into(), has("shape").unwrap_or(1.0)), ("rate".into(), has("rate").unwrap())],
        )
    } else {
        (d.iter().map(|(k, _)| *k).collect(), d.iter().map(|(k, v)| (k.to_string(), has(k).unwrap_or(*v))).collect())
    };
    let unknown: Vec<&str> = given.iter().map(|(n, _)| n.as_str()).filter(|n| !known.contains(n)).collect();
    if !unknown.is_empty() {
        let mut k = known.clone();
        k.sort();
        return Err(format!("parameters of {name}: {}; not {}", k.join(", "), unknown.join(", ")));
    }
    for (k, v) in &params {
        if ["rate", "sd", "sdlog", "kappa"].contains(&k.as_str()) && (v.is_nan() || *v < 0.0) {
            return Err(format!("{k}: 0 or more"));
        }
    }
    Ok(Distribution { name: name.to_string(), params, vcov: None })
}

/// amt's fit_distr of values (the missing left out).
pub fn fit(x: &[f64], name: &str) -> std::result::Result<Distribution, String> {
    use terraflux_ls_sys::ffi;
    let v: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
    let e = |e: cxx::Exception| e.what().to_string();
    let d = |params: Vec<(&str, f64)>, vcov: Option<(Vec<String>, Vec<f64>)>| Distribution {
        name: name.to_string(),
        params: params.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        vcov,
    };
    Ok(match name {
        "gamma" => {
            let f = ffi::move_fit_gamma(&v).map_err(e)?;
            let vcov = (f.vcov.len() == 4).then(|| (vec!["scale".to_string(), "shape".to_string()], f.vcov.clone()));
            d(vec![("shape", f.shape), ("scale", f.scale)], vcov)
        }
        "exp" => d(vec![("rate", ffi::move_fit_exp(&v).map_err(e)?)], None),
        "lnorm" => {
            let p = ffi::move_fit_lnorm(&v).map_err(e)?;
            d(vec![("meanlog", p[0]), ("sdlog", p[1])], None)
        }
        "hnorm" => {
            if v.is_empty() {
                return Err("no values to fit".into());
            }
            let sq: Vec<f64> = v.iter().map(|x| x * x).collect();
            d(vec![("sd", (1.0 / v.len() as f64 * ffi::move_r_sum(&sq)).sqrt())], None)
        }
        "unif" => {
            if v.is_empty() {
                return Err("no values to fit".into());
            }
            let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            d(vec![("min", lo), ("max", hi)], None)
        }
        "vonmises" => {
            let f = ffi::move_fit_vonmises(&v).map_err(e)?;
            d(vec![("kappa", f[1]), ("mu", 0.0)], Some((vec!["kappa".to_string()], vec![f[3] * f[3]])))
        }
        _ => return Err(format!("{name:?} is not supported: exp, gamma, hnorm, lnorm, unif or vonmises")),
    })
}

fn distribution_out(d: Distribution) -> Outputs {
    let t = d.table();
    outputs([("distribution", Value::StepDistribution(Arc::new(d))), ("table", Value::Table(t))])
}

const PARAMS: [&str; 10] = ["shape", "scale", "rate", "meanlog", "sdlog", "sd", "min", "max", "kappa", "mu"];

/// `move.distribution`.
struct Make;

impl Operator for Make {
    fn signature(&self) -> OpSignature {
        let mut s = OpSignature::new(
            "move.distribution",
            "A distribution of step lengths or turn angles (amt's make_distribution): gamma (shape, and scale or \
             rate), exp (rate), lnorm (meanlog, sdlog), hnorm (sd) for step lengths; vonmises (kappa, mu 0) or \
             unif (min, max) for turn angles. The parameters not given take amt's defaults (gamma 1, 1; exp 1; \
             lnorm 0, 1; hnorm 1; unif -pi, pi; von Mises 1, 0).",
            Access::None,
        )
        .output(PortSpec::new("distribution", Type::StepDistribution, "the distribution"))
        .output(PortSpec::new("table", Type::Table, "its parameters"))
        .param(
            PortSpec::new("name", Type::Text, "gamma, exp, lnorm, hnorm, vonmises or unif")
                .choices(["gamma", "exp", "lnorm", "hnorm", "vonmises", "unif"]),
        );
        for p in PARAMS {
            s = s.param(PortSpec::optional(
                p,
                Type::Number,
                &format!("the parameter {p} (of the distributions that have it)"),
            ));
        }
        s
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let name: String = args.param("name", String::new())?;
        let mut given = Vec::new();
        for p in PARAMS {
            if let Some(v) = args.opt_param::<f64>(p)? {
                given.push((p.to_string(), v));
            }
        }
        Ok(distribution_out(make(&name, &given).map_err(|e| fail(args.node, e))?))
    }
}

/// The values of a column of features or of a table.
fn values_of(args: &Args, column: &str) -> Result<Vec<f64>> {
    let node = args.node;
    match args.get("values") {
        Some(Value::Vector(v)) => {
            v.column(column).map(Column::numbers).ok_or_else(|| fail(node, format!("no column {column:?}")))
        }
        Some(Value::Table(t)) => t.column(column).ok_or_else(|| fail(node, format!("no column {column:?}"))),
        Some(v) => Err(fail(node, format!("values: steps or a table, not a {}", v.ty().name()))),
        None => Err(fail(node, "connect the steps (values)")),
    }
}

/// `move.fit_distribution`.
struct Fit;

impl Operator for Fit {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.fit_distribution",
            "A distribution fitted to step lengths or turn angles (amt's fit_distr, the same numbers): gamma by \
             maximum likelihood with R's Nelder-Mead from the closed-form estimates (fitdistrplus' fitdist; steps of \
             length 0 take the smallest other length); exp the maximum likelihood rate 1 / mean(x) (amt fits it by \
             BFGS, which fails for rates under 0.001); lnorm by Nelder-Mead; hnorm sqrt(mean(x^2)); unif the \
             smallest and largest value; vonmises kappa by maximum likelihood (circular's mle.vonmises), mu 0. The \
             missing values are left out.",
            Access::None,
        )
        .input(PortSpec::new("values", Type::Any, "the steps (move.steps) or a table"))
        .output(PortSpec::new("distribution", Type::StepDistribution, "the distribution fitted"))
        .output(PortSpec::new("table", Type::Table, "its parameters"))
        .param(
            PortSpec::new("name", Type::Text, "gamma, exp, lnorm, hnorm, vonmises or unif")
                .choices(["gamma", "exp", "lnorm", "hnorm", "vonmises", "unif"]),
        )
        .param(PortSpec::optional(
            "column",
            Type::Text,
            "the column of the values (sl_ for gamma, exp, lnorm and hnorm; ta_ for vonmises and unif)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let name: String = args.param("name", String::new())?;
        let default = if name == "vonmises" || name == "unif" { "ta_" } else { "sl_" };
        let column: String = args.param("column", default.to_string())?;
        let x = values_of(args, &column)?;
        Ok(distribution_out(fit(&x, &name).map_err(|e| fail(args.node, e))?))
    }
}

fn distribution_of<'a>(args: &'a Args, port: &str) -> Result<Option<&'a Arc<Distribution>>> {
    match args.get(port) {
        None => Ok(None),
        Some(Value::StepDistribution(d)) => Ok(Some(d)),
        Some(v) => Err(fail(args.node, format!("{port}: a distribution (move.distribution), not a {}", v.ty().name()))),
    }
}

/// `move.random_numbers`.
struct RandomNumbers;

impl Operator for RandomNumbers {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.random_numbers",
            "Draws of a distribution of steps (amt's random_numbers), from R's generator after set.seed(seed): \
             amt's numbers. Turn angles of a von Mises distribution are wrapped to (-pi, pi].",
            Access::None,
        )
        .input(PortSpec::new("distribution", Type::StepDistribution, "the distribution"))
        .output(PortSpec::new("table", Type::Table, "the draws (value)"))
        .param(PortSpec::optional("n", Type::Integer, "how many (100)"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of R's generator (1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let d = distribution_of(args, "distribution")?.ok_or_else(|| fail(args.node, "connect the distribution"))?;
        let n: i64 = args.param("n", 100)?;
        let mut rng = terraflux_ls_sys::RRng::new(args.param("seed", 1)?);
        let x = terraflux_ls_sys::ffi::move_random_numbers(rng.pin_mut(), &d.name, &d.draw_params(), n)?;
        let mut t = Table::new(vec!["value".into()]);
        t.rows = x.into_iter().map(|v| vec![v]).collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}

/// The sample.int calls of fitting a gamma distribution to `sl` with fitdistrplus (keepdata = FALSE): fitdist
/// keeps min(100, n) - 2 values at random besides the smallest and the largest.
fn fit_draws(sl: &[f64]) -> Vec<(i64, i64)> {
    let mut v: Vec<f64> = sl.iter().copied().filter(|x| !x.is_nan()).collect();
    if v.contains(&0.0) && v.iter().any(|&x| x > 0.0) {
        let m = v.iter().copied().filter(|&x| x > 0.0).fold(f64::INFINITY, f64::min);
        v.iter_mut().filter(|x| **x == 0.0).for_each(|x| *x = m);
    }
    let n = v.len() as i64;
    let keep = n.min(100) - 2;
    if keep <= 0 {
        return vec![];
    }
    // numpy's argmin / argmax: the first
    let (mut imin, mut imax) = (0, 0);
    for (i, &x) in v.iter().enumerate() {
        if x < v[imin] {
            imin = i;
        }
        if x > v[imax] {
            imax = i;
        }
    }
    let rest: Vec<i64> = (0..v.len()).filter(|&i| i != imin && i != imax).map(|i| i as i64 + 1).collect();
    vec![(if rest.len() == 1 { rest[0] } else { rest.len() as i64 }, keep)]
}

/// `move.random_steps`.
struct RandomSteps;

impl Operator for RandomSteps {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.random_steps",
            "Each observed step with n_control random steps from its start (amt's random_steps, the same steps): \
             the data of a step selection function. For each step but the first of a track (or of a burst), the \
             observed step (case_ 1) and n_control steps from the same start (case_ 0) whose lengths and turn \
             angles are drawn from sl_distr and ta_distr (default: a gamma and a von Mises distribution fitted to \
             each track's steps), turned from the direction of the previous step; step_id_ names the stratum. As \
             amt, n_draws numbers are drawn from each distribution once per track and each random step takes one of \
             them at random. Bursts of fewer than 3 steps are left out. R's generator after set.seed(seed), the \
             tracks in turn. The steps must be in a projected coordinate system (move.aeqd_crs).",
            Access::None,
        )
        .input(PortSpec::new("steps", Type::Vector, "the steps (move.steps), projected"))
        .input(PortSpec::optional("sl_distr", Type::StepDistribution, "the distribution of the step lengths"))
        .input(PortSpec::optional("ta_distr", Type::StepDistribution, "the distribution of the turn angles"))
        .output(PortSpec::new("steps", Type::Vector, "the observed and random steps (case_, step_id_)"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .output(PortSpec::new(
            "distributions",
            Type::Vector,
            "the distributions of each track (sl_distr, ta_distr and their parameters), without geometries",
        ))
        .param(PortSpec::optional("n_control", Type::Integer, "random steps per observed step (10)"))
        .param(PortSpec::optional("n_draws", Type::Integer, "numbers drawn from each distribution per track (100000)"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of R's generator (1)"))
        .param(PortSpec::optional("track", Type::Text, "the column of the track (default: the usual names)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        use terraflux_ls_sys::ffi;
        let node = args.node;
        let steps = tracks_of(args, "steps")?;
        if !is_steps(steps) {
            return Err(fail(node, "steps: the steps of move.steps"));
        }
        let k = track_column(args, steps)?;
        let nc: i64 = args.param("n_control", 10)?;
        let n_draws: i64 = args.param("n_draws", 100_000)?;
        if nc < 1 || n_draws < 1 {
            return Err(fail(node, "n_control and n_draws: 1 or more"));
        }
        if is_lonlat(steps.crs.as_deref()).map_err(|e| fail(node, e))? {
            return Err(fail(
                node,
                "random steps are drawn in a projected coordinate system: project the tracks first (move.aeqd_crs)",
            ));
        }
        let num = |c: &str| numbers(steps, c).map_err(|e| fail(node, e));
        let (x1, y1, dir, sl_all, ta_all) = (num("x1_")?, num("y1_")?, num("direction_p")?, num("sl_")?, num("ta_")?);
        let burst = steps.column(BURST).map(Column::numbers);
        let code = codes(steps.column(&k).unwrap());
        let n = code.len();
        let bounds: Vec<usize> = (0..=n).filter(|&i| i == 0 || i == n || code[i] != code[i - 1]).collect::<Vec<_>>();
        let sl_given = distribution_of(args, "sl_distr")?;
        let ta_given = distribution_of(args, "ta_distr")?;
        let seed: i32 = args.param("seed", 1)?;
        let mut rng = terraflux_ls_sys::RRng::new(seed);
        let (mut short, mut rows_out, mut controls) = (0usize, Vec::new(), Vec::new());
        let mut distrs: Vec<(usize, Distribution, Distribution)> = Vec::new();
        let label = |c: &Column, i: usize| match c {
            Column::Texts(t) => format!("{:?}", t[i]),
            Column::Numbers(x) => crate::value::class_label(x[i]),
        };
        for w in bounds.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a == b {
                continue;
            }
            let tid = label(steps.column(&k).unwrap(), a);
            // the parts of the track: its bursts of 3 steps or more (in the order of their numbers), or all of it
            let parts: Vec<Vec<usize>> = match &burst {
                Some(bu) => {
                    // pandas' groupby: the bursts in order, a missing one left out
                    let mut ids: Vec<f64> = (a..b).map(|i| bu[i]).filter(|x| !x.is_nan()).collect();
                    ids.sort_by(|x, y| x.total_cmp(y));
                    ids.dedup();
                    let mut kept = Vec::new();
                    for id in ids {
                        let rows: Vec<usize> = (a..b).filter(|&i| bu[i] == id).collect();
                        if rows.len() >= 3 {
                            kept.push(rows);
                        } else {
                            short += 1;
                        }
                    }
                    if kept.is_empty() {
                        return Err(fail(node, format!("track {tid}: no burst with 3 steps or more")));
                    }
                    kept
                }
                None => {
                    if b - a < 2 {
                        return Err(fail(node, format!("track {tid}: random steps need 2 steps or more")));
                    }
                    vec![(a..b).collect()]
                }
            };
            let mut start = 1i64;
            let mut starts = Vec::new();
            for p in &parts {
                starts.push(start);
                start += p.len() as i64;
            }
            let e = |m: String| fail(node, format!("track {tid}: {m}"));
            let sl = match sl_given {
                Some(d) => (**d).clone(),
                None => fit(&sl_all[a..b], "gamma").map_err(e)?,
            };
            let ta = match ta_given {
                Some(d) => (**d).clone(),
                None => fit(&ta_all[a..b], "vonmises").map_err(e)?,
            };
            // the draws, as amt makes them: the subsample fitdistrplus keeps, the numbers of each distribution,
            // then for each step sample.int(n_draws, n_control, TRUE) of each
            if sl_given.is_none() {
                for (bn, size) in fit_draws(&sl_all[a..b]) {
                    ffi::rrng_sample_int(rng.pin_mut(), bn, size, false, &[])?;
                }
            }
            let rsl = ffi::move_random_numbers(rng.pin_mut(), &sl.name, &sl.draw_params(), n_draws)?;
            let rta = ffi::move_random_numbers(rng.pin_mut(), &ta.name, &ta.draw_params(), n_draws)?;
            let n_steps: usize = parts.iter().map(|p| p.len() - 1).sum();
            let (mut dsl, mut dta) = (Vec::with_capacity(n_steps * nc as usize), Vec::new());
            for _ in 0..n_steps {
                let is = ffi::rrng_sample_int(rng.pin_mut(), n_draws, nc, true, &[])?;
                let it = ffi::rrng_sample_int(rng.pin_mut(), n_draws, nc, true, &[])?;
                dsl.extend(is.iter().map(|&j| rsl[j as usize - 1]));
                dta.extend(it.iter().map(|&j| rta[j as usize - 1]));
            }
            let mut used = 0usize;
            for (p, &st) in parts.iter().zip(&starts) {
                let m = p.len();
                // the observed steps (all but the part's first) and their random steps
                let rep = |x: &[f64], off: usize| -> Vec<f64> {
                    (0..m - 1).flat_map(|i| std::iter::repeat_n(x[p[i + off]], nc as usize)).collect()
                };
                let s = &dsl[used * nc as usize..(used + m - 1) * nc as usize];
                let t = &dta[used * nc as usize..(used + m - 1) * nc as usize];
                used += m - 1;
                let c = ffi::move_amt_controls(
                    &rep(&x1, 1),
                    &rep(&y1, 1),
                    &rep(&x1, 0),
                    &rep(&y1, 0),
                    &rep(&dir, 0),
                    s,
                    t,
                )?;
                for i in 0..m - 1 {
                    let row = p[i + 1];
                    let id = (i as i64 + 2 + st) as f64;
                    rows_out.push((row, true, id, None));
                    for j in 0..nc as usize {
                        let q = i * nc as usize + j;
                        rows_out.push((row, false, id, Some(controls.len())));
                        controls.push([c.sl[q], c.dir[q], s[q], c.ta[q]]);
                    }
                }
            }
            distrs.push((a, sl, ta));
        }
        if short > 0 {
            ctx.warn(format!("node {node:?}: {short} bursts of fewer than 3 steps are left out"));
        }
        // the steps' columns but direction_p, the random steps' ends, lengths and turn angles, case_ and step_id_
        let rows: Vec<usize> = rows_out.iter().map(|r| r.0).collect();
        let mut out = steps.select(&rows);
        out.attributes.retain(|(n, _)| n != "direction_p");
        for (name, slot) in [("x2_", 0usize), ("y2_", 1), ("sl_", 2), ("ta_", 3)] {
            if let Some((_, Column::Numbers(x))) = out.attributes.iter_mut().find(|(n, _)| n == name) {
                for (r, row) in rows_out.iter().enumerate() {
                    if let Some(q) = row.3 {
                        x[r] = controls[q][slot];
                    }
                }
            }
        }
        out.attributes
            .push(("case_".into(), Column::Numbers(rows_out.iter().map(|r| f64::from(u8::from(r.1))).collect())));
        out.attributes.push(("step_id_".into(), Column::Numbers(rows_out.iter().map(|r| r.2).collect())));
        let col = |n: &str| out.column(n).map(Column::numbers).unwrap();
        let (ax, ay, bx, by) = (col("x1_"), col("y1_"), col("x2_"), col("y2_"));
        out.geometries = (0..out.len()).map(|i| Geometry::LineString(vec![[ax[i], ay[i]], [bx[i], by[i]]])).collect();
        // a feature per track: its distributions
        let firsts: Vec<usize> = distrs.iter().map(|d| d.0).collect();
        let mut attributes = vec![(k.clone(), super::take(steps.column(&k).unwrap(), &firsts))];
        for (which, pick) in [("sl", 0usize), ("ta", 1)] {
            let of = |d: &(usize, Distribution, Distribution)| if pick == 0 { d.1.clone() } else { d.2.clone() };
            attributes.push((format!("{which}_distr"), Column::Texts(distrs.iter().map(|d| of(d).name).collect())));
            let mut names: Vec<String> = Vec::new();
            for d in &distrs {
                for (p, _) in &of(d).params {
                    if !names.contains(p) {
                        names.push(p.clone());
                    }
                }
            }
            for p in names {
                attributes.push((
                    format!("{which}_{p}"),
                    Column::Numbers(distrs.iter().map(|d| of(d).get(&p).unwrap_or(f64::NAN)).collect()),
                ));
            }
        }
        let dv = Vector { geometries: vec![Geometry::Empty; distrs.len()], attributes, crs: None };
        let mut o = give("steps", out);
        o.insert("distributions".into(), Value::Vector(Arc::new(dv)));
        Ok(o)
    }
}

/// `move.extract_covariates`.
struct Covariates;

impl Operator for Covariates {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.extract_covariates",
            "The values of rasters at locations, or at the ends or starts of steps (amt's extract_covariates): the \
             value of the cell holding each point (terra's extract: a point on the edge between two cells takes the \
             cell to its right / below), none outside the raster or on no data. Connect the rasters as inputs of \
             any names (forest, elev...): a column each, named by its input (a stack's layers input_layer). The \
             points are taken to each raster's coordinate system.",
            Access::Global,
        )
        .input(PortSpec::new("tracks", Type::Vector, "tracks, their steps (move.steps) or random steps"))
        .output(PortSpec::new("tracks", Type::Vector, "the features with a column per raster (or layer)"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .param(
            PortSpec::optional(
                "where",
                Type::Text,
                "steps: at their end (default), start or both (columns suffixed _start and _end)",
            )
            .choices(["end", "start", "both"]),
        )
        .extra(Type::Any)
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        // (name, grid, values) of each layer
        let mut layers: Vec<(String, Arc<terraflux_raster::Grid>, Arc<Vec<f64>>)> = Vec::new();
        for (port, value) in &args.inputs {
            if port == "tracks" {
                continue;
            }
            let value = crate::files::as_port(value.clone(), Type::Any)?;
            match &value {
                Value::Map(m) => layers.push((port.clone(), m.grid.clone(), m.data.clone())),
                Value::Stack(s) if s.layers.len() == 1 => {
                    layers.push((port.clone(), s.grid.clone(), s.layers[0].clone()))
                }
                Value::Stack(s) => {
                    for (key, l) in s.keys.iter().zip(&s.layers) {
                        layers.push((format!("{port}_{key}"), s.grid.clone(), l.clone()));
                    }
                }
                other => {
                    return Err(fail(node, format!("{port}: a raster, not a {}", other.ty().name())));
                }
            }
        }
        if layers.is_empty() {
            return Err(fail(node, "connect the rasters (inputs of any names: forest, elev...)"));
        }
        let points: Vec<(Vec<f64>, Vec<f64>, &str)> = if is_steps(v) {
            let w: String = args.param("where", "end".to_string())?;
            if !["end", "start", "both"].contains(&w.as_str()) {
                return Err(fail(node, "where: end, start or both"));
            }
            let num = |c: &str| numbers(v, c).map_err(|e| fail(node, e));
            let mut p = Vec::new();
            if w == "start" || w == "both" {
                p.push((num("x1_")?, num("y1_")?, if w == "both" { "_start" } else { "" }));
            }
            if w == "end" || w == "both" {
                p.push((num("x2_")?, num("y2_")?, if w == "both" { "_end" } else { "" }));
            }
            p
        } else {
            let (mut x, mut y) = (Vec::with_capacity(v.len()), Vec::with_capacity(v.len()));
            for g in &v.geometries {
                match g {
                    Geometry::Point(p) => {
                        x.push(p[0]);
                        y.push(p[1]);
                    }
                    Geometry::Empty => {
                        x.push(f64::NAN);
                        y.push(f64::NAN);
                    }
                    _ => return Err(fail(node, "the values are taken at points")),
                }
            }
            vec![(x, y, "")]
        };
        let mut out = (**v).clone();
        for (xs, ys, suffix) in &points {
            for (name, grid, values) in &layers {
                let (mut px, mut py) = (xs.clone(), ys.clone());
                to_crs(&mut px, &mut py, v.crs.as_deref(), crate::prepare::crs_of(grid).as_deref())
                    .map_err(|e| Error::msg(format!("node {node:?}: {e}")))?;
                let cells = grid_cells(&px, &py, grid);
                let vals = cells.iter().map(|&c| if c >= 0 { values[c as usize] } else { f64::NAN }).collect();
                super::set_column(&mut out, &format!("{name}{suffix}"), Column::Numbers(vals));
            }
        }
        Ok(give("tracks", out))
    }
}
