//! MaxNet (landschaft's port of the maxnet package, Phillips et al. 2017): Maxent as an infinitely weighted
//! logistic regression, glmnet's lasso path over linear, quadratic, product, hinge, threshold and categorical
//! features. landschaft's C++ for the fit and the predictions; their names as R writes them.

use serde::{Deserialize, Serialize};
use terraflux_ls_sys::ffi;

use super::extrapolation::Frame;

const KIND_LINEAR: i32 = 0;
const KIND_QUADRATIC: i32 = 1;
const KIND_HINGE: i32 = 2;
const KIND_THRESHOLD: i32 = 3;
const KIND_CATEGORICAL: i32 = 4;

/// The outputs of a MaxNet: link, exponential, cloglog or logistic.
pub const OUTPUTS: [&str; 4] = ["link", "exponential", "cloglog", "logistic"];

/// A fitted MaxNet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaxNet {
    /// The feature classes used (l, q, p, h, t).
    pub classes: String,
    pub cont_names: Vec<String>,
    pub cat_names: Vec<String>,
    /// Every factor's levels.
    pub levels: Vec<Vec<f64>>,
    /// The features with a nonzero coefficient: their names, kinds, variables and knots.
    pub names: Vec<String>,
    pub kind: Vec<i32>,
    pub var: Vec<i32>,
    pub var2: Vec<i32>,
    pub pk1: Vec<f64>,
    pub pk2: Vec<f64>,
    pub betas: Vec<f64>,
    pub feature_min: Vec<f64>,
    pub feature_max: Vec<f64>,
    /// The continuous variables' ranges in training (clamping).
    pub var_min: Vec<f64>,
    pub var_max: Vec<f64>,
    pub alpha: f64,
    pub entropy: f64,
    /// glmnet's lambdas.
    pub lambdas: Vec<f64>,
    /// The continuous variables' means over the presences, and every factor's most frequent level.
    pub sample_means: Vec<f64>,
    pub majority: Vec<f64>,
    pub n_passes: i64,
    pub n_added: i64,
    /// The output of predictions by default, and whether they clamp.
    pub output: String,
    pub clamp: bool,
}

/// The settings of a fit.
pub struct Settings<'a> {
    pub categorical: &'a [String],
    pub classes: &'a str,
    pub regmult: f64,
    pub add_samples_to_background: bool,
    pub output: &'a str,
    pub clamp: bool,
}

/// The continuous columns one after the other, and the factors' codes (0.., -1 missing, -2 unseen).
fn tables(frame: &Frame, cont: &[usize], cat: &[usize], levels: &[Vec<f64>]) -> (Vec<f64>, Vec<i32>) {
    let c: Vec<f64> = cont.iter().flat_map(|&j| frame.columns[j].iter().copied()).collect();
    let k: Vec<i32> = cat
        .iter()
        .zip(levels)
        .flat_map(|(&j, lv)| {
            frame.columns[j]
                .iter()
                .map(move |&v| if v.is_nan() { -1 } else { lv.iter().position(|l| *l == v).map_or(-2, |k| k as i32) })
        })
        .collect();
    (c, k)
}

/// The name of a level as landschaft writes it (R's as.character).
fn label(v: f64) -> String {
    ffi::sdm_r_as_character(v)
}

pub fn fit(frame: &Frame, y: &[f64], s: &Settings) -> Result<MaxNet, String> {
    if !OUTPUTS.contains(&s.output) {
        return Err(format!("type must be one of {}", OUTPUTS.join(", ")));
    }
    let n = frame.rows();
    if y.len() != n {
        return Err(format!("y must have one value per row of X ({n})"));
    }
    if y.iter().any(|v| *v != 0.0 && *v != 1.0) {
        return Err("y must be 1 (presence) or 0 (background)".into());
    }
    let cat: Vec<usize> = (0..frame.names.len()).filter(|&j| s.categorical.contains(&frame.names[j])).collect();
    let cont: Vec<usize> = (0..frame.names.len()).filter(|j| !cat.contains(j)).collect();
    let levels: Vec<Vec<f64>> = cat
        .iter()
        .map(|&j| {
            let mut v: Vec<f64> = frame.columns[j].iter().copied().filter(|x| !x.is_nan()).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.dedup();
            v
        })
        .collect();
    let (c, k) = tables(frame, &cont, &cat, &levels);
    if c.iter().any(|v| v.is_nan()) || k.iter().any(|v| *v < 0) {
        return Err("MaxNet cannot fit rows with missing values: remove them first".into());
    }
    let n_levels: Vec<i32> = levels.iter().map(|l| l.len() as i32).collect();
    let m = ffi::sdm_maxnet_fit(&c, &k, n as i64, &n_levels, y, s.classes, s.regmult, s.add_samples_to_background)
        .map_err(|e| e.to_string())?;
    let cont_names: Vec<String> = cont.iter().map(|&j| frame.names[j].clone()).collect();
    let cat_names: Vec<String> = cat.iter().map(|&j| frame.names[j].clone()).collect();
    let plain = m.classes.contains('l');
    let all_names: Vec<String> = (0..m.kind.len())
        .map(|j| {
            let (a, b) = (m.var[j] as usize, m.var2[j] as usize);
            match m.kind[j] {
                KIND_LINEAR => cont_names[a].clone(),
                KIND_QUADRATIC => format!("I({}^2)", cont_names[a]),
                KIND_HINGE => format!("hinge({}):{}:{}", cont_names[a], m.s1[j], m.s2[j]),
                KIND_THRESHOLD => format!("thresholds({}):{}", cont_names[a], m.s1[j]),
                KIND_CATEGORICAL => format!("categorical({}):{}", cat_names[a], label(levels[a][b])),
                _ => {
                    let (first, second) = if !plain && (a, b) == (0, 1) { (b, a) } else { (a, b) };
                    format!("{}:{}", cont_names[first], cont_names[second])
                }
            }
        })
        .collect();
    let bi: Vec<usize> = m.beta_index.iter().map(|&k| k as usize).collect();
    let pick_i = |v: &[i32]| bi.iter().map(|&k| v[k]).collect::<Vec<_>>();
    let pick_f = |v: &[f64]| bi.iter().map(|&k| v[k]).collect::<Vec<_>>();
    Ok(MaxNet {
        classes: m.classes.to_string(),
        cont_names,
        cat_names,
        majority: m.majority.iter().zip(&levels).map(|(&k, lv)| lv[k as usize]).collect(),
        levels,
        names: bi.iter().map(|&k| all_names[k].clone()).collect(),
        kind: pick_i(&m.kind),
        var: pick_i(&m.var),
        var2: pick_i(&m.var2),
        pk1: pick_f(&m.pk1),
        pk2: pick_f(&m.pk2),
        betas: m.betas.to_vec(),
        feature_min: pick_f(&m.feature_min),
        feature_max: pick_f(&m.feature_max),
        var_min: m.var_min.to_vec(),
        var_max: m.var_max.to_vec(),
        alpha: m.alpha,
        entropy: m.entropy,
        lambdas: m.lambda.to_vec(),
        sample_means: m.sample_means.to_vec(),
        n_passes: m.n_passes,
        n_added: m.n_added,
        output: s.output.to_string(),
        clamp: s.clamp,
    })
}

impl MaxNet {
    /// The predictors the model needs.
    pub fn predictors(&self) -> Vec<String> {
        self.cont_names.iter().chain(&self.cat_names).cloned().collect()
    }

    /// The predictions at the rows of a table of the predictors (by name): link, exponential, cloglog or
    /// logistic, clamped to the training's ranges or not.
    pub fn predict(&self, frame: &Frame, output: Option<&str>, clamp: Option<bool>) -> Result<Vec<f64>, String> {
        let output = output.unwrap_or(&self.output);
        let o = OUTPUTS
            .iter()
            .position(|x| *x == output)
            .ok_or_else(|| format!("type must be one of {}", OUTPUTS.join(", ")))?;
        let find = |name: &String| {
            frame.names.iter().position(|x| x == name).ok_or_else(|| format!("the data lack the predictor {name:?}"))
        };
        let cont: Vec<usize> = self.cont_names.iter().map(find).collect::<Result<_, _>>()?;
        let cat: Vec<usize> = self.cat_names.iter().map(find).collect::<Result<_, _>>()?;
        let (c, k) = tables(frame, &cont, &cat, &self.levels);
        let n_levels: Vec<i32> = self.levels.iter().map(|l| l.len() as i32).collect();
        ffi::sdm_maxnet_predict(
            &self.kind,
            &self.var,
            &self.var2,
            &self.pk1,
            &self.pk2,
            &self.betas,
            &self.feature_min,
            &self.feature_max,
            &self.var_min,
            &self.var_max,
            self.alpha,
            self.entropy,
            &c,
            &k,
            frame.rows() as i64,
            &n_levels,
            clamp.unwrap_or(self.clamp),
            o as i32,
        )
        .map_err(|e| e.to_string())
    }
}
