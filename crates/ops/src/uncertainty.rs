//! Propagation of uncertainty (landschaft's uncertainty module, the tools plangea uses in its restoration
//! plans): the standard deviation of a total of uncertain values and of a product of two, the delta method and
//! Monte Carlo over an expression of maps and uncertain parameters, and the normal confidence interval. The
//! sums, draws and quantiles are numpy's, to the bit.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::algebra::{Scope, parse};
use crate::change::numpy_sum;
use crate::signatures::NumpyRng;
use crate::ziggurat::{FI, KI, NOR_INV_R, NOR_R, WI};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Stack, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(SumSd));
    r.add(Arc::new(ProductSd));
    r.add(Arc::new(Interval));
    r.add(Arc::new(Delta));
    r.add(Arc::new(MonteCarlo));
}

/// numpy's `Generator.standard_normal` (the ziggurat of random_standard_normal).
pub fn standard_normal(rng: &mut NumpyRng) -> f64 {
    let next_double = |rng: &mut NumpyRng| (rng.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0);
    loop {
        let mut r = rng.next_u64();
        let idx = (r & 0xff) as usize;
        r >>= 8;
        let sign = r & 0x1;
        let rabs = (r >> 1) & 0x000f_ffff_ffff_ffff;
        let mut x = rabs as f64 * WI[idx];
        if sign & 0x1 != 0 {
            x = -x;
        }
        if rabs < KI[idx] {
            return x;
        }
        if idx == 0 {
            loop {
                let xx = -NOR_INV_R * (-next_double(rng)).ln_1p();
                let yy = -(-next_double(rng)).ln_1p();
                if yy + yy > xx * xx {
                    return if (rabs >> 8) & 0x1 != 0 { -(NOR_R + xx) } else { NOR_R + xx };
                }
            }
        } else if (FI[idx - 1] - FI[idx]) * next_double(rng) + FI[idx] < (-0.5 * x * x).exp() {
            return x;
        }
    }
}

fn polevl(x: f64, coef: &[f64]) -> f64 {
    let mut ans = coef[0];
    for c in &coef[1..] {
        ans = ans * x + c;
    }
    ans
}

fn p1evl(x: f64, coef: &[f64]) -> f64 {
    let mut ans = x + coef[0];
    for c in &coef[1..] {
        ans = ans * x + c;
    }
    ans
}

/// The inverse of the standard normal distribution (cephes' ndtri, scipy's norm.ppf).
#[allow(clippy::excessive_precision)]
pub fn ndtri(y0: f64) -> f64 {
    const P0: [f64; 5] = [
        -5.99633501014107895267E1,
        9.80010754185999661536E1,
        -5.66762857469070293439E1,
        1.39312609387279679503E1,
        -1.23916583867381258016E0,
    ];
    const Q0: [f64; 8] = [
        1.95448858338141759834E0,
        4.67627912898881538453E0,
        8.63602421390890590575E1,
        -2.25462687854119370527E2,
        2.00260212380060660359E2,
        -8.20372256168333339912E1,
        1.59056225126211695515E1,
        -1.18331621121330003142E0,
    ];
    const P1: [f64; 9] = [
        4.05544892305962419923E0,
        3.15251094599893866154E1,
        5.71628192246421288162E1,
        4.40805073893200834700E1,
        1.46849561928858024014E1,
        2.18663306850790267539E0,
        -1.40256079171354495875E-1,
        -3.50424626827848203418E-2,
        -8.57456785154685413611E-4,
    ];
    const Q1: [f64; 8] = [
        1.57799883256466749731E1,
        4.53907635128879210584E1,
        4.13172038254672030440E1,
        1.50425385692907503408E1,
        2.50464946208309415979E0,
        -1.42182922854787788574E-1,
        -3.80806407691578277194E-2,
        -9.33259480895457427372E-4,
    ];
    const P2: [f64; 9] = [
        3.23774891776946035970E0,
        6.91522889068984211695E0,
        3.93881025292474443415E0,
        1.33303460815807542389E0,
        2.01485389549179081538E-1,
        1.23716634817820021358E-2,
        3.01581553508235416007E-4,
        2.65806974686737550832E-6,
        6.23974539184983293730E-9,
    ];
    const Q2: [f64; 8] = [
        6.02427039364742014255E0,
        3.67983563856160859403E0,
        1.37702099489081330271E0,
        2.16236993594496635890E-1,
        1.34204006088543189037E-2,
        3.28014464682127739104E-4,
        2.89247864745380683936E-6,
        6.79019408009981274425E-9,
    ];
    const SQRT2PI: f64 = 2.506628274631000502415765284811045253007;
    if y0 == 0.0 {
        return f64::NEG_INFINITY;
    }
    if y0 == 1.0 {
        return f64::INFINITY;
    }
    if !(0.0..=1.0).contains(&y0) {
        return f64::NAN;
    }
    let mut code = true;
    let mut y = y0;
    if y > 1.0 - 0.13533528323661269189 {
        y = 1.0 - y;
        code = false;
    }
    if y > 0.13533528323661269189 {
        y -= 0.5;
        let y2 = y * y;
        let x = y + y * (y2 * polevl(y2, &P0) / p1evl(y2, &Q0));
        return x * SQRT2PI;
    }
    let x = (-2.0 * y.ln()).sqrt();
    let x0 = x - x.ln() / x;
    let z = 1.0 / x;
    let x1 = if x < 8.0 { z * polevl(z, &P1) / p1evl(z, &Q1) } else { z * polevl(z, &P2) / p1evl(z, &Q2) };
    let x = x0 - x1;
    if code { -x } else { x }
}

/// numpy's quantile (the linear method) of sorted values.
fn quantile(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    // the linear method's virtual index: (n - 1) q
    let virtual_index = (n - 1) as f64 * q;
    let previous = virtual_index.floor();
    let gamma = virtual_index - previous;
    let clip = |i: f64| (i.max(0.0) as usize).min(n - 1);
    let (a, b) = (sorted[clip(previous)], sorted[clip(previous + 1.0)]);
    // _lerp: a + (b - a) t, or b - (b - a)(1 - t) from t = 0.5
    let diff = b - a;
    if gamma >= 0.5 { b - diff * (1.0 - gamma) } else { a + diff * gamma }
}

/// A port's values: a map's cells (with its grid) or a number.
enum Values {
    Map(Map),
    Number(f64),
}

impl Values {
    fn of(args: &Args, port: &str) -> Result<Option<Values>> {
        Ok(match args.get(port) {
            None => None,
            Some(Value::Map(m)) => Some(Values::Map(m.clone())),
            Some(v) => match v.as_number() {
                Some(x) => Some(Values::Number(x)),
                None => return Err(Error::msg(format!("node {:?}: {port}: a map or a number", args.node))),
            },
        })
    }

    fn at(&self, i: usize) -> f64 {
        match self {
            Values::Map(m) => m.data[i],
            Values::Number(x) => *x,
        }
    }

    fn len(&self) -> Option<usize> {
        match self {
            Values::Map(m) => Some(m.data.len()),
            Values::Number(_) => None,
        }
    }
}

fn method_of(args: &Args) -> Result<bool> {
    match args.param("method", "quadratic".to_string())?.as_str() {
        "quadratic" => Ok(true),
        "linear" => Ok(false),
        m => Err(Error::msg(format!("node {:?}: method must be 'quadratic' or 'linear', got {m:?}", args.node))),
    }
}

struct SumSd;

impl Operator for SumSd {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "uncertainty.sum_sd",
            "The standard deviation of a total of uncertain values, the sum of w v (landschaft's sum_sd, plangea's \
             prop_quad and prop_lin): independent errors sqrt(sum (sigma w)^2), errors adding up sum |sigma w|, or \
             with a correlation rho of every pair sqrt((1 - rho) sum s^2 + rho (sum s)^2). Missing cells left out.",
            Access::Global,
        )
        .input(PortSpec::new("sd", Type::Map, "the values' standard deviations"))
        .input(PortSpec::optional("weights", Type::Any, "a map or a number (default 1)"))
        .output(PortSpec::new("sd", Type::Number, "the total's standard deviation"))
        .param(
            PortSpec::optional("method", Type::Text, "quadratic (default) or linear").choices(["quadratic", "linear"]),
        )
        .param(PortSpec::optional("correlation", Type::Number, "of every pair of errors, -1 to 1"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let sd = args.map("sd")?;
        let w = Values::of(args, "weights")?;
        let s: Vec<f64> = sd
            .data
            .iter()
            .enumerate()
            .map(|(i, v)| w.as_ref().map_or(*v, |w| v * w.at(i)))
            .filter(|v| !v.is_nan())
            .collect();
        let quadratic = method_of(args)?;
        let out = if let Some(rho) = args.opt_param::<f64>("correlation")? {
            if !(-1.0..=1.0).contains(&rho) {
                return Err(Error::msg(format!("node {:?}: correlation must be between -1 and 1", args.node)));
            }
            let total = numpy_sum(&s);
            let var = (1.0 - rho) * numpy_sum(&s.iter().map(|x| x * x).collect::<Vec<_>>()) + rho * total.powi(2);
            var.max(0.0).sqrt()
        } else if quadratic {
            numpy_sum(&s.iter().map(|x| x * x).collect::<Vec<_>>()).sqrt()
        } else {
            numpy_sum(&s.iter().map(|x| x.abs()).collect::<Vec<_>>())
        };
        Ok([("sd".to_string(), Value::Number(out))].into())
    }
}

struct ProductSd;

impl Operator for ProductSd {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "uncertainty.product_sd",
            "The standard deviation of the product of two independent uncertain values x v (landschaft's \
             product_sd): Goodman's exact variance x^2 sigma_v^2 + v^2 sigma_x^2 + sigma_x^2 sigma_v^2, cell by \
             cell, or of the sum of the products (total: quadratic or linear).",
            Access::Local,
        )
        .input(PortSpec::new("x", Type::Any, "a map or a number"))
        .input(PortSpec::new("sd_x", Type::Any, "its standard deviation"))
        .input(PortSpec::new("v", Type::Any, "a map or a number"))
        .input(PortSpec::new("sd_v", Type::Any, "its standard deviation"))
        .output(PortSpec::new("sd", Type::Any, "a map (or a number) of standard deviations, or the total's"))
        .param(PortSpec::optional("total", Type::Bool, "the standard deviation of the sum of the products"))
        .param(
            PortSpec::optional("method", Type::Text, "of the total: quadratic (default) or linear")
                .choices(["quadratic", "linear"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let get = |p: &str| -> Result<Values> {
            Values::of(args, p)?.ok_or_else(|| Error::msg(format!("node {:?}: connect {p}", args.node)))
        };
        let (x, sx, v, sv) = (get("x")?, get("sd_x")?, get("v")?, get("sd_v")?);
        let all = [&x, &sx, &v, &sv];
        let grid = all.iter().find_map(|a| if let Values::Map(m) = a { Some(m.grid.clone()) } else { None });
        let n = all.iter().find_map(|a| a.len()).unwrap_or(1);
        if all.iter().any(|a| a.len().is_some_and(|l| l != n)) {
            return Err(Error::msg(format!("node {:?}: the maps must have one shape", args.node)));
        }
        let var: Vec<f64> = (0..n)
            .map(|i| {
                let (xv, s_x, vv, s_v) = (x.at(i), sx.at(i), v.at(i), sv.at(i));
                xv * xv * s_v * s_v + vv * vv * s_x * s_x + s_x * s_x * s_v * s_v
            })
            .collect();
        let out = if args.param("total", false)? {
            if method_of(args)? {
                Value::Number(numpy_sum(&var.iter().copied().filter(|v| !v.is_nan()).collect::<Vec<_>>()).sqrt())
            } else {
                let lin: Vec<f64> = (0..n)
                    .map(|i| x.at(i).abs() * sv.at(i) + v.at(i).abs() * sx.at(i) + sx.at(i) * sv.at(i))
                    .filter(|v| !v.is_nan())
                    .collect();
                Value::Number(numpy_sum(&lin))
            }
        } else {
            let sd: Vec<f64> = var.iter().map(|v| v.sqrt()).collect();
            match grid {
                Some(g) => Value::Map(Map::new(g, sd)),
                None => Value::Number(sd[0]),
            }
        };
        Ok([("sd".to_string(), out)].into())
    }
}

struct Interval;

impl Operator for Interval {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "uncertainty.interval",
            "The normal confidence interval of a value: value -/+ z sd, z the normal quantile of the level \
             (landschaft's interval; scipy's norm.ppf, cephes' ndtri; plangea fixes z at 1.96).",
            Access::Local,
        )
        .input(PortSpec::new("value", Type::Any, "a map or a number"))
        .input(PortSpec::new("sd", Type::Any, "its standard deviation"))
        .output(PortSpec::new("low", Type::Any, "the lower bound"))
        .output(PortSpec::new("high", Type::Any, "the upper bound"))
        .param(PortSpec::optional("level", Type::Number, "the confidence level, 0 to 1 (0.95)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let level: f64 = args.param("level", 0.95)?;
        if !(level > 0.0 && level < 1.0) {
            return Err(Error::msg(format!("node {:?}: level must be between 0 and 1, got {level}", args.node)));
        }
        let z = ndtri(0.5 + level / 2.0);
        let get = |p: &str| -> Result<Values> {
            Values::of(args, p)?.ok_or_else(|| Error::msg(format!("node {:?}: connect {p}", args.node)))
        };
        let (value, sd) = (get("value")?, get("sd")?);
        let grid = [&value, &sd].iter().find_map(|a| if let Values::Map(m) = a { Some(m.grid.clone()) } else { None });
        let n = value.len().or(sd.len()).unwrap_or(1);
        let bound = |sign: f64| -> Value {
            let v: Vec<f64> = (0..n)
                .map(|i| if sign < 0.0 { value.at(i) - z * sd.at(i) } else { value.at(i) + z * sd.at(i) })
                .collect();
            match &grid {
                Some(g) => Value::Map(Map::new(g.clone(), v)),
                None => Value::Number(v[0]),
            }
        };
        Ok([("low".to_string(), bound(-1.0)), ("high".to_string(), bound(1.0))].into())
    }
}

/// The parameters of an expression: (name, value, standard deviation).
fn parameters(args: &Args) -> Result<Vec<(String, f64, f64)>> {
    let fail = || Error::msg(format!("node {:?}: parameters: [[\"name\", value, sd], ...]", args.node));
    let Some(serde_json::Value::Array(a)) = args.params.get("parameters") else { return Err(fail()) };
    a.iter()
        .map(|p| {
            let p = p.as_array().filter(|p| p.len() == 3).ok_or_else(fail)?;
            Ok((
                p[0].as_str().ok_or_else(fail)?.to_string(),
                p[1].as_f64().ok_or_else(fail)?,
                p[2].as_f64().ok_or_else(fail)?,
            ))
        })
        .collect()
}

/// An expression over the node's maps and numbers and some parameters: f(p), cell by cell.
struct Model<'a> {
    expr: crate::algebra::Expr,
    names: Vec<String>,
    maps: BTreeMap<&'a str, &'a [f64]>,
    numbers: BTreeMap<&'a str, f64>,
    grid: Option<Arc<terraflux_raster::Grid>>,
}

impl<'a> Model<'a> {
    fn new(args: &'a Args, names: Vec<String>) -> Result<Model<'a>> {
        let text: String =
            args.opt_param("expression")?.ok_or_else(|| Error::msg(format!("node {:?}: no expression", args.node)))?;
        let expr = parse(&text).map_err(|x| Error::msg(format!("node {:?}: the expression: {}", args.node, x.0)))?;
        let (mut maps, mut numbers, mut grid) = (BTreeMap::new(), BTreeMap::new(), None);
        for (name, v) in &args.inputs {
            match v {
                Value::Map(m) => {
                    if grid.as_ref().is_some_and(|g: &Arc<terraflux_raster::Grid>| g.cells() != m.grid.cells()) {
                        return Err(Error::msg(format!(
                            "node {:?}: {name} is not on the grid of the other maps",
                            args.node
                        )));
                    }
                    grid.get_or_insert_with(|| m.grid.clone());
                    maps.insert(name.as_str(), &m.data[..]);
                }
                v => {
                    let x = v
                        .as_number()
                        .ok_or_else(|| Error::msg(format!("node {:?}: {name}: a map or a number", args.node)))?;
                    numbers.insert(name.as_str(), x);
                }
            }
        }
        Ok(Model { expr, names, maps, numbers, grid })
    }

    fn eval(&self, p: &[f64]) -> Result<Vec<f64>> {
        let (w, h) = self.grid.as_ref().map_or((1, 1), |g| (g.width, g.height));
        let mut numbers = self.numbers.clone();
        for (n, v) in self.names.iter().zip(p) {
            numbers.insert(n.as_str(), *v);
        }
        let s = Scope { width: w, height: h, maps: self.maps.clone(), numbers, layers: BTreeMap::new() };
        self.expr.evaluate(&s).map_err(|x| Error::msg(x.0))
    }

    fn value(&self, v: Vec<f64>) -> Value {
        match &self.grid {
            Some(g) => Value::Map(Map::new(g.clone(), v)),
            None => Value::Number(v[0]),
        }
    }
}

struct Delta;

impl Operator for Delta {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "uncertainty.delta",
            "The uncertainty of an expression of the node's maps and numbers (map.calc's) from the uncertainty of \
             its parameters, by the delta method (landschaft's delta_method, the GUM's first order): the \
             derivatives by finite differences (central, or forward as plangea), the standard deviation \
             sqrt(sum (df/dp sigma)^2) (quadratic) or sum |df/dp sigma| (linear).",
            Access::Local,
        )
        .extra(Type::Any)
        .output(PortSpec::new("value", Type::Any, "the expression at the parameters (a map or a number)"))
        .output(PortSpec::new("sd", Type::Any, "its standard deviation"))
        .param(PortSpec::new("expression", Type::Text, "e.g. carbon * factor + base"))
        .param(PortSpec::new("parameters", Type::Any, "[[\"factor\", 0.47, 0.02], ...]: name, value, sd"))
        .param(
            PortSpec::optional("method", Type::Text, "quadratic (default) or linear").choices(["quadratic", "linear"]),
        )
        .param(PortSpec::optional("scheme", Type::Text, "central (default) or forward").choices(["central", "forward"]))
        .param(PortSpec::optional("step", Type::Any, "the finite-difference step (one, or one per parameter)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let params = parameters(args)?;
        let model = Model::new(args, params.iter().map(|p| p.0.clone()).collect())?;
        let p: Vec<f64> = params.iter().map(|q| q.1).collect();
        let sd: Vec<f64> = params.iter().map(|q| q.2).collect();
        let central = match args.param("scheme", "central".to_string())?.as_str() {
            "central" => true,
            "forward" => false,
            s => return Err(fail(Error::msg(format!("scheme must be 'central' or 'forward', got {s:?}")))),
        };
        let h: Vec<f64> = match args.params.get("step") {
            None | Some(serde_json::Value::Null) => {
                let eps = f64::EPSILON;
                let base = if central { eps.powf(1.0 / 3.0) } else { eps.powf(0.5) };
                p.iter().map(|x| base * x.abs().max(1.0)).collect()
            }
            Some(serde_json::Value::Number(n)) => vec![n.as_f64().unwrap(); p.len()],
            Some(_) => args.param::<Vec<f64>>("step", Vec::new())?,
        };
        if h.len() != p.len() {
            return Err(fail(Error::msg("one step per parameter")));
        }
        let base = model.eval(&p).map_err(fail)?;
        let m = base.len();
        let mut jac = vec![0.0; m * p.len()];
        for k in 0..p.len() {
            let mut up = p.clone();
            up[k] += h[k];
            let fu = model.eval(&up).map_err(fail)?;
            let col: Vec<f64> = if central {
                let mut dn = p.clone();
                dn[k] -= h[k];
                let fd = model.eval(&dn).map_err(fail)?;
                fu.iter().zip(&fd).map(|(a, b)| (a - b) / (2.0 * h[k])).collect()
            } else {
                fu.iter().zip(&base).map(|(a, b)| (a - b) / h[k]).collect()
            };
            for (i, c) in col.into_iter().enumerate() {
                jac[i * p.len() + k] = c;
            }
        }
        let quadratic = method_of(args)?;
        let out: Vec<f64> = jac
            .chunks(p.len().max(1))
            .map(|row| {
                let t: Vec<f64> = row.iter().zip(&sd).map(|(j, s)| j * s).collect();
                if quadratic {
                    numpy_sum(&t.iter().map(|x| x * x).collect::<Vec<_>>()).sqrt()
                } else {
                    numpy_sum(&t.iter().map(|x| x.abs()).collect::<Vec<_>>())
                }
            })
            .collect();
        Ok([("value".to_string(), model.value(base)), ("sd".to_string(), model.value(out))].into())
    }
}

struct MonteCarlo;

impl Operator for MonteCarlo {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "uncertainty.monte_carlo",
            "The uncertainty of an expression of the node's maps and numbers by drawing its parameters (landschaft's \
             monte_carlo): n draws from normal distributions with numpy's generator and seed, the expression \
             computed for each, their mean, standard deviation and quantiles (numpy's).",
            Access::Local,
        )
        .extra(Type::Any)
        .output(PortSpec::new("mean", Type::Any, "the mean over the draws (a map or a number)"))
        .output(PortSpec::new("sd", Type::Any, "the standard deviation over the draws"))
        .output(PortSpec::new("quantiles", Type::Any, "a layer (or a row) per quantile"))
        .param(PortSpec::new("expression", Type::Text, "e.g. carbon * factor + base"))
        .param(PortSpec::new("parameters", Type::Any, "[[\"factor\", 0.47, 0.02], ...]: name, mean, sd"))
        .param(PortSpec::optional("n", Type::Integer, "the draws (1000)"))
        .param(PortSpec::optional("seed", Type::Integer, "of numpy's generator (0)"))
        .param(PortSpec::optional("quantiles", Type::Any, "[0.025, 0.975] (default)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let params = parameters(args)?;
        let model = Model::new(args, params.iter().map(|p| p.0.clone()).collect())?;
        let n: i64 = args.param("n", 1000)?;
        if n < 2 {
            return Err(fail(Error::msg("at least 2 draws are needed")));
        }
        let n = n as usize;
        let seed: i64 = args.param("seed", 0)?;
        let qs: Vec<f64> = args.param("quantiles", vec![0.025, 0.975])?;
        let mut rng = NumpyRng::new(seed as u64);
        let k = params.len();
        let z: Vec<f64> = (0..n * k).map(|_| standard_normal(&mut rng)).collect();
        let mut out: Vec<Vec<f64>> = Vec::with_capacity(n);
        for i in 0..n {
            let draw: Vec<f64> = (0..k).map(|j| params[j].1 + z[i * k + j] * params[j].2).collect();
            out.push(model.eval(&draw).map_err(fail)?);
        }
        let m = out[0].len();
        // numpy's reductions along the draws: pairwise for one output (the axis of length 1 squeezed), else
        // in order, column by column
        let column_sums = |rows: &dyn Fn(usize) -> Vec<f64>| -> Vec<f64> {
            if m == 1 {
                return vec![numpy_sum(&(0..n).map(|i| rows(i)[0]).collect::<Vec<_>>())];
            }
            let mut sum = vec![0.0; m];
            for i in 0..n {
                for (s, v) in sum.iter_mut().zip(rows(i)) {
                    *s += v;
                }
            }
            sum
        };
        let sum = column_sums(&|i| out[i].clone());
        let mean: Vec<f64> = sum.iter().map(|s| s / n as f64).collect();
        let ss = column_sums(&|i| out[i].iter().zip(&mean).map(|(v, mu)| (v - mu) * (v - mu)).collect());
        let sd: Vec<f64> = ss.iter().map(|s| (s / (n - 1) as f64).sqrt()).collect();
        let quantiles: Vec<Vec<f64>> = {
            let mut cols: Vec<Vec<f64>> = (0..m).map(|j| out.iter().map(|r| r[j]).collect()).collect();
            for c in &mut cols {
                c.sort_by(f64::total_cmp);
            }
            qs.iter().map(|&q| cols.iter().map(|c| quantile(c, q)).collect()).collect()
        };
        let qv = match &model.grid {
            Some(g) => Value::Stack(Stack {
                grid: g.clone(),
                keys: qs.iter().map(|q| crate::Key::Name(format!("q{q}"))).collect(),
                layers: quantiles.into_iter().map(Arc::new).collect(),
            }),
            None => {
                let mut t = Table::new(vec!["quantile".into(), "value".into()]);
                t.rows = qs.iter().zip(&quantiles).map(|(q, v)| vec![*q, v[0]]).collect();
                Value::Table(t)
            }
        };
        Ok([
            ("mean".to_string(), model.value(mean)),
            ("sd".to_string(), model.value(sd)),
            ("quantiles".to_string(), qv),
        ]
        .into())
    }
}
