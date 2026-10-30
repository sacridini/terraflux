//! How a simulated map matches the observed one (landschaft's `agreement` and `similarity_of_differences`):
//! `validate.agreement`, `validate.similarity`.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::markov::classes_of;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// Cell by cell agreement of a simulated map with the observed one, both from the same start.
#[derive(Debug, Clone, PartialEq)]
pub struct Agreement {
    /// Share of the cells of the same class.
    pub overall: f64,
    /// hits / (hits + misses + false alarms + wrong hits) (Pontius et al. 2008).
    pub figure_of_merit: f64,
    pub hits: i64,
    pub misses: i64,
    pub false_alarms: i64,
    /// Change simulated where it happened, to another class.
    pub wrong_hits: i64,
    /// Per class: simulated - observed cells.
    pub quantity: Vec<(f64, i64)>,
}

pub fn agreement(observed: &Map, simulated: &Map, start: &Map) -> Result<Agreement> {
    let n = observed.data.len();
    if simulated.data.len() != n || start.data.len() != n {
        return Err(Error::msg("the maps differ in size"));
    }
    let (mut hits, mut wrong, mut misses, mut false_alarms, mut same, mut count) = (0, 0, 0, 0, 0i64, 0i64);
    for i in 0..n {
        let (o, s, a) = (observed.data[i], simulated.data[i], start.data[i]);
        if o.is_nan() || s.is_nan() || a.is_nan() {
            continue;
        }
        count += 1;
        same += i64::from(o == s);
        let (oc, sc) = (o != a, s != a);
        match (oc, sc) {
            (true, true) if o == s => hits += 1,
            (true, true) => wrong += 1,
            (true, false) => misses += 1,
            (false, true) => false_alarms += 1,
            _ => {}
        }
    }
    let denom = hits + misses + false_alarms + wrong;
    let classes = classes_of(&[observed, simulated]);
    let quantity = classes
        .iter()
        .map(|&c| {
            let (mut sc, mut oc) = (0i64, 0i64);
            for i in 0..n {
                let (o, s, a) = (observed.data[i], simulated.data[i], start.data[i]);
                if o.is_nan() || s.is_nan() || a.is_nan() {
                    continue;
                }
                sc += i64::from(s == c);
                oc += i64::from(o == c);
            }
            (c, sc - oc)
        })
        .collect();
    Ok(Agreement {
        overall: if count > 0 { same as f64 / count as f64 } else { f64::NAN },
        figure_of_merit: if denom > 0 { hits as f64 / denom as f64 } else { f64::NAN },
        hits,
        misses,
        false_alarms,
        wrong_hits: wrong,
        quantity,
    })
}

/// The cells that changed between two maps, in their new class (NaN elsewhere).
pub fn differences(t0: &Map, t1: &Map) -> Vec<f64> {
    t0.data
        .iter()
        .zip(t1.data.iter())
        .map(|(&a, &b)| if a != b && !a.is_nan() && !b.is_nan() { b } else { f64::NAN })
        .collect()
}

/// Codes of the classes of two maps (their index among the classes of both; -1 without one).
fn codes(a: &[f64], b: &[f64]) -> (Vec<i32>, Vec<i32>) {
    let mut classes: Vec<f64> = a.iter().chain(b).copied().filter(|v| !v.is_nan()).collect();
    classes.sort_by(|x, y| x.partial_cmp(y).unwrap());
    classes.dedup();
    let code = |m: &[f64]| -> Vec<i32> {
        m.iter().map(|&v| if v.is_nan() { -1 } else { classes.partition_point(|&c| c < v) as i32 }).collect()
    };
    (code(a), code(b))
}

/// Fuzzy similarity of two categorical maps (Dinamica EGO's CalcReciprocalSimilarityMap): the means over the
/// cells of the first and of the second.
pub fn fuzzy_similarity(
    first: &[f64],
    second: &[f64],
    ny: usize,
    nx: usize,
    window: i32,
    exponential: bool,
) -> Result<(f64, f64)> {
    let (a, b) = codes(first, second);
    let r = ffi::similarity(&a, &b, ny as i64, nx as i64, window, exponential)?;
    Ok((r.first_mean, r.second_mean))
}

/// Dinamica EGO's validation: the fuzzy similarity of the simulated and the observed change in windows of
/// growing size. A row per window: window, simulated, observed, minimum, simulated_cells, observed_cells.
pub fn similarity_of_differences(
    start: &Map,
    observed: &Map,
    simulated: &Map,
    windows: &[i32],
    exponential: bool,
) -> Result<Table> {
    let (ny, nx) = start.shape();
    let d_obs = differences(start, observed);
    let d_sim = differences(start, simulated);
    let n_sim = d_sim.iter().filter(|v| !v.is_nan()).count() as f64;
    let n_obs = d_obs.iter().filter(|v| !v.is_nan()).count() as f64;
    let mut t = Table::new(
        ["window", "simulated", "observed", "minimum", "simulated_cells", "observed_cells"].map(String::from).to_vec(),
    );
    for &w in windows {
        let (s, o) = fuzzy_similarity(&d_sim, &d_obs, ny, nx, w, exponential)?;
        t.rows.push(vec![w as f64, s, o, s.min(o), n_sim, n_obs]);
    }
    Ok(t)
}

struct AgreementOp;

impl Operator for AgreementOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "validate.agreement",
            "How a simulated map matches the observed one, both from the same start: the share of cells of the \
             same class, the figure of merit (hits / (hits + misses + false alarms + wrong hits)) and its terms.",
            Access::Global,
        )
        .input(PortSpec::new("start", Type::Map, "the map both start from"))
        .input(PortSpec::new("observed", Type::Map, "the observed map"))
        .input(PortSpec::new("simulated", Type::Map, "the simulated map"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "overall, figure_of_merit, hits, misses, false_alarms, wrong_hits",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let a = agreement(args.map("observed")?, args.map("simulated")?, args.map("start")?)?;
        let t = Table {
            columns: ["overall", "figure_of_merit", "hits", "misses", "false_alarms", "wrong_hits"]
                .map(String::from)
                .to_vec(),
            rows: vec![vec![
                a.overall,
                a.figure_of_merit,
                a.hits as f64,
                a.misses as f64,
                a.false_alarms as f64,
                a.wrong_hits as f64,
            ]],
            ..Default::default()
        };
        Ok(outputs([("table", Value::Table(t))]))
    }
}

struct SimilarityOp;

impl Operator for SimilarityOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "validate.similarity",
            "Dinamica EGO's validation: the fuzzy similarity of the simulated and the observed change (the cells \
             that changed from the start, in their new class) in windows of growing size; the minimum of the two \
             directions is Dinamica's result.",
            Access::Global,
        )
        .input(PortSpec::new("start", Type::Map, "the map both start from"))
        .input(PortSpec::new("observed", Type::Map, "the observed map"))
        .input(PortSpec::new("simulated", Type::Map, "the simulated map"))
        .output(PortSpec::new("table", Type::Table, "a row per window"))
        .param(PortSpec::optional("windows", Type::Any, "the window sides, odd ([1, 3, 5, 7, 9, 11])"))
        .param(
            PortSpec::optional("decay", Type::Text, "\"exponential\" (default) or \"constant\"")
                .choices(["exponential", "constant"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let windows: Vec<i32> = args.param("windows", vec![1, 3, 5, 7, 9, 11])?;
        let decay: String = args.param("decay", "exponential".to_string())?;
        if decay != "exponential" && decay != "constant" {
            return Err(Error::msg("decay must be \"exponential\" or \"constant\""));
        }
        let t = similarity_of_differences(
            args.map("start")?,
            args.map("observed")?,
            args.map("simulated")?,
            &windows,
            decay == "exponential",
        )?;
        Ok(outputs([("table", Value::Table(t))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(AgreementOp));
    r.add(Arc::new(SimilarityOp));
}
