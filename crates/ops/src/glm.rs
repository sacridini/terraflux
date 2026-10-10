//! Logistic regression of each class on the drivers, as lulcc fits it (R's glm): `glm.fit`, `glm.predict`.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::{GlmFit, ffi};

use crate::markov::classes_of;
use crate::value::GlmClass;
use crate::{Args, Ctx, Error, GlmModel, Key, Map, Operator, Outputs, Registry, Result, Stack, Value, outputs};

/// The model matrix of some cells, column-major n x p (R's layout): the intercept, then each driver.
fn model_matrix(drivers: &[&[f64]], idx: &[usize]) -> Vec<f64> {
    let n = idx.len();
    let mut x = Vec::with_capacity(n * (drivers.len() + 1));
    x.extend(std::iter::repeat_n(1.0, n));
    for d in drivers {
        x.extend(idx.iter().map(|&i| d[i]));
    }
    x
}

fn driver<'a>(stack: &'a Stack, name: &str) -> Result<&'a [f64]> {
    stack
        .by_name(name)
        .map(|v| v.as_slice())
        .ok_or_else(|| Error::msg(format!("no driver named {name:?} (the drivers are {:?})", stack.names())))
}

/// Fit a model per class: presence of the class in the cells with a value in the landscape and in every
/// driver of the class's model (glm's na.omit). `formulas`: (class, drivers) in order; `None`: every class of
/// the map on every driver (by name).
pub fn fit(
    landscape: &Map,
    drivers: &Stack,
    formulas: Option<Vec<(f64, Vec<String>)>>,
    maxit: i32,
    epsilon: f64,
) -> Result<GlmModel> {
    let lv = &landscape.data;
    let formulas = match formulas {
        Some(f) => f,
        None => {
            let mut names = drivers.names();
            names.sort();
            classes_of(&[landscape]).into_iter().map(|c| (c, names.clone())).collect()
        }
    };
    let mut fits = Vec::new();
    for (cls, names) in formulas {
        let cols: Vec<&[f64]> = names.iter().map(|n| driver(drivers, n)).collect::<Result<_>>()?;
        let idx: Vec<usize> =
            (0..lv.len()).filter(|&i| !lv[i].is_nan() && cols.iter().all(|c| !c[i].is_nan())).collect();
        let x = model_matrix(&cols, &idx);
        let y: Vec<f64> = idx.iter().map(|&i| if lv[i] == cls { 1.0 } else { 0.0 }).collect();
        let f = ffi::glm_logit(&x, &y, idx.len() as i32, (names.len() + 1) as i32, maxit, epsilon)?;
        fits.push(GlmClass {
            class: cls,
            drivers: names,
            coefficients: f.coefficients,
            pivot: f.pivot,
            rank: f.rank,
            iterations: f.iterations,
            converged: f.converged,
            deviance: f.deviance,
            null_deviance: f.null_deviance,
            cells: idx.len(),
        });
    }
    Ok(GlmModel { fits })
}

/// The probability of each class in every cell (`predict(type = "response")`): NaN where the landscape (if
/// given) or a driver of the class's model is missing. Layers in the order of the fits.
pub fn predict(model: &GlmModel, drivers: &Stack, landscape: Option<&Map>) -> Result<Stack> {
    let n = drivers.grid.cells();
    let mut layers = Vec::new();
    let mut keys = Vec::new();
    for f in &model.fits {
        let cols: Vec<&[f64]> = f.drivers.iter().map(|d| driver(drivers, d)).collect::<Result<_>>()?;
        let idx: Vec<usize> = (0..n)
            .filter(|&i| landscape.is_none_or(|l| !l.data[i].is_nan()) && cols.iter().all(|c| !c[i].is_nan()))
            .collect();
        let x = model_matrix(&cols, &idx);
        let fit = GlmFit {
            coefficients: f.coefficients.clone(),
            pivot: f.pivot.clone(),
            rank: f.rank,
            iterations: f.iterations,
            converged: f.converged,
            boundary: false,
            deviance: f.deviance,
            null_deviance: f.null_deviance,
        };
        let p = ffi::glm_predict(&x, idx.len() as i64, (f.drivers.len() + 1) as i32, &fit)?;
        let mut out = vec![f64::NAN; n];
        for (k, &i) in idx.iter().enumerate() {
            out[i] = p[k];
        }
        keys.push(Key::Class(f.class));
        layers.push(Arc::new(out));
    }
    Ok(Stack { grid: drivers.grid.clone(), keys, layers })
}

struct Fit;

impl Operator for Fit {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "glm.fit",
            "Logistic regression of the presence of each class on the drivers (lulcc's glmModels, R's glm.fit \
             operation for operation), on the cells with a value in the map and in the class's drivers.",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map the presences come from"))
        .input(PortSpec::new("drivers", Type::Stack, "the explanatory variables"))
        .output(PortSpec::new("model", Type::Glm, "the fitted regressions"))
        .param(PortSpec::optional(
            "formulas",
            Type::Any,
            "{ class = [driver, ...] } (default: every class on every driver); the intercept is always in",
        ))
        .param(PortSpec::optional("maxit", Type::Integer, "glm.fit's iteration limit (25)"))
        .param(PortSpec::optional("epsilon", Type::Number, "glm.fit's convergence tolerance (1e-8)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let formulas: Option<BTreeMap<String, Vec<String>>> = args.opt_param("formulas")?;
        let formulas = formulas
            .map(|f| {
                let mut v: Vec<(f64, Vec<String>)> = f
                    .into_iter()
                    .map(|(k, d)| {
                        k.trim().parse::<f64>().map(|c| (c, d)).map_err(|_| Error::msg(format!("{k:?} is not a class")))
                    })
                    .collect::<Result<_>>()?;
                v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                Ok::<_, Error>(v)
            })
            .transpose()?;
        let m = fit(
            args.map("landscape")?,
            args.stack("drivers")?,
            formulas,
            args.param("maxit", 25)?,
            args.param("epsilon", 1e-8)?,
        )?;
        Ok(outputs([("model", Value::Glm(Arc::new(m)))]))
    }
}

struct Predict;

impl Operator for Predict {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "glm.predict",
            "The probability of each class in every cell from fitted logistic regressions (NaN where a driver, or \
             the landscape when given, is missing).",
            Access::Local,
        )
        .input(PortSpec::new("model", Type::Glm, "the fitted regressions"))
        .input(PortSpec::new("drivers", Type::Stack, "the explanatory variables (they may have changed)"))
        .input(PortSpec::optional("landscape", Type::Map, "a map whose missing cells stay missing"))
        .output(PortSpec::new("suitability", Type::Stack, "a layer per class"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = predict(args.glm("model")?, args.stack("drivers")?, args.opt_map("landscape")?)?;
        Ok(outputs([("suitability", Value::Stack(s))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Fit));
    r.add(Arc::new(Predict));
}
