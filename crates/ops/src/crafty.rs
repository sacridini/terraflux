//! CRAFTY's competition for land (Murray-Rust et al. 2014; Brown et al. 2019): agent functional types (AFTs:
//! intensive farmers, foresters, conservationists...) produce services from the capitals of the cells they
//! manage; the demand for each service that the supply does not meet makes producing it worth more; each step,
//! managers whose competitiveness falls below their giving-up threshold abandon their cells, and cells are
//! taken by the AFT most competitive there when it beats the incumbent by the incumbent's giving-in threshold.
//!
//! - **Production:** `P[a, s](i) = production[a][s] x prod over capitals c of capital_c(i) ^ sensitivity[a][c]`.
//! - **Utility:** `u[s] = max(D[s] - S[s], 0) / D[s]`, the share of the demand still unmet.
//! - **Competitiveness:** `C[a](i) = sum over s of u[s] x P[a, s](i)`.
//! - **Giving up:** a managed cell whose competitiveness is below its AFT's `giving_up` is abandoned with
//!   probability `giving_up_rate`.
//! - **Taking over:** a share `search` of the cells is offered each step, in the order of random keys; the most
//!   competitive AFT takes an offered cell when the cell is unmanaged (and it is competitive at all) or when it
//!   beats the incumbent by the incumbent's `giving_in`. The supply and the utilities follow every change.
//!
//! Random numbers come from a counter (seed, step, cell): the same seed gives the same maps.

use std::sync::Arc;

use serde::Deserialize;
use std::collections::BTreeMap;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// An agent functional type.
#[derive(Debug, Clone, Deserialize)]
pub struct Aft {
    pub name: String,
    /// The land cover class of its cells (for the cover map).
    #[serde(default)]
    pub class: Option<f64>,
    /// The most it produces of each service on a cell of full capitals.
    pub production: BTreeMap<String, f64>,
    /// The exponent of each capital it depends on (missing: 0, not at all).
    #[serde(default)]
    pub sensitivity: BTreeMap<String, f64>,
    #[serde(default)]
    pub giving_up: f64,
    #[serde(default)]
    pub giving_in: f64,
}

/// A counter-based uniform number in [0, 1): SplitMix64 of the parts.
fn uniform(parts: &[u64]) -> f64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    for &p in parts {
        let mut z = h ^ p.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        h = z ^ (z >> 31);
    }
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// One step of CRAFTY's competition: the AFT of each cell after it (0 unmanaged, NaN outside), and the supply.
#[allow(clippy::too_many_arguments)]
pub fn step(
    land: &[f64],
    capitals: &[(&str, &[f64])],
    afts: &[Aft],
    services: &[String],
    demand: &[f64],
    search: f64,
    giving_up_rate: f64,
    seed: u64,
    step: u64,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let n = land.len();
    let k = afts.len();
    let ns = services.len();
    // each AFT's capital factor per cell
    let mut factor = vec![vec![0.0f64; n]; k];
    for (a, aft) in afts.iter().enumerate() {
        if let Some(c) = aft.sensitivity.keys().find(|c| !capitals.iter().any(|(name, _)| name == c)) {
            return Err(Error::msg(format!("the AFT {:?} depends on a capital {c:?} not given", aft.name)));
        }
        for i in 0..n {
            let mut f = 1.0;
            for (c, values) in capitals {
                let e = aft.sensitivity.get(*c).copied().unwrap_or(0.0);
                if e != 0.0 {
                    let x = values[i];
                    f *= if x.is_nan() || x <= 0.0 { 0.0 } else { x.min(1.0).powf(e) };
                }
            }
            factor[a][i] = f;
        }
    }
    let prod: Vec<Vec<f64>> =
        afts.iter().map(|a| services.iter().map(|s| a.production.get(s).copied().unwrap_or(0.0)).collect()).collect();
    let mut cur: Vec<f64> = land.to_vec();
    let aft_of =
        |v: f64| -> Option<usize> { (v >= 1.0 && v.fract() == 0.0 && (v as usize) <= k).then(|| v as usize - 1) };
    // the supply
    let mut supply = vec![0.0f64; ns];
    for i in 0..n {
        if let Some(a) = aft_of(cur[i]) {
            for s in 0..ns {
                supply[s] += prod[a][s] * factor[a][i];
            }
        }
    }
    let before = supply.clone();
    let utility = |supply: &[f64]| -> Vec<f64> {
        (0..ns).map(|s| if demand[s] > 0.0 { ((demand[s] - supply[s]) / demand[s]).max(0.0) } else { 0.0 }).collect()
    };
    let competitiveness =
        |u: &[f64], a: usize, i: usize| -> f64 { (0..ns).map(|s| u[s] * prod[a][s]).sum::<f64>() * factor[a][i] };
    // giving up
    let u = utility(&supply);
    for i in 0..n {
        if let Some(a) = aft_of(cur[i])
            && competitiveness(&u, a, i) < afts[a].giving_up
            && uniform(&[seed, step, i as u64, 1]) < giving_up_rate
        {
            cur[i] = 0.0;
            for s in 0..ns {
                supply[s] -= prod[a][s] * factor[a][i];
            }
        }
    }
    // the cells offered, in the order of their keys
    let mut offered: Vec<(f64, usize)> = (0..n)
        .filter(|&i| !cur[i].is_nan())
        .map(|i| (uniform(&[seed, step, i as u64, 2]), i))
        .filter(|(key, _)| *key < search)
        .collect();
    offered.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, i) in offered {
        let u = utility(&supply);
        let (best, cb) = (0..k)
            .map(|a| (a, competitiveness(&u, a, i)))
            .fold((usize::MAX, f64::NEG_INFINITY), |acc, x| if x.1 > acc.1 { x } else { acc });
        if best == usize::MAX || cb <= 0.0 {
            continue;
        }
        let take = match aft_of(cur[i]) {
            None => true,
            Some(a) if a == best => false,
            Some(a) => cb > competitiveness(&u, a, i) + afts[a].giving_in,
        };
        if take {
            if let Some(a) = aft_of(cur[i]) {
                for s in 0..ns {
                    supply[s] -= prod[a][s] * factor[a][i];
                }
            }
            cur[i] = (best + 1) as f64;
            for s in 0..ns {
                supply[s] += prod[best][s] * factor[best][i];
            }
        }
    }
    Ok((cur, before, supply))
}

/// `crafty.allocate`.
pub struct Allocate;

impl Operator for Allocate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "crafty.allocate",
            "One step of CRAFTY's competition for land between agent functional types (AFTs): production of \
             services from the cells' capitals, the value of unmet demand, giving up and taking over. The land \
             map holds the AFT of each cell (1, 2... in the order of `afts`; 0 unmanaged).",
            Access::Global,
        )
        .input(PortSpec::new("land", Type::Map, "the AFT of each cell (0: unmanaged; missing: outside)"))
        .input(PortSpec::new("capitals", Type::Stack, "the capitals of the cells, 0 to 1, by name"))
        .input(PortSpec::new("demand", Type::Table, "the demand of each service (a column each), a row per step"))
        .input(PortSpec::new("step", Type::Integer, "the step (its row of the demand)"))
        .input(PortSpec::new("seed", Type::Integer, "the seed"))
        .param(PortSpec::new(
            "afts",
            Type::Any,
            "[{name, class, production: {service: p}, sensitivity: {capital: e}, giving_up, giving_in}, ...]",
        ))
        .param(PortSpec::optional(
            "services",
            Type::Any,
            "the services and the demand's column of each ({\"food\": \"food_sum\"}; default: every column)",
        ))
        .param(PortSpec::optional("search", Type::Number, "the share of cells offered each step (0.1)"))
        .param(PortSpec::optional(
            "giving_up_rate",
            Type::Number,
            "the chance a manager below its giving-up threshold leaves at a step (0.1)",
        ))
        .output(PortSpec::new("land", Type::Map, "the AFT of each cell after the step"))
        .output(PortSpec::new("cover", Type::Map, "the land cover class of each cell's AFT"))
        .output(PortSpec::new(
            "info",
            Type::Table,
            "a row per service (its column in the demand, from 0): demand, supply before and after the step",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let at = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let land = args.map("land")?;
        let caps = args.stack("capitals")?;
        let demand = match args.get("demand") {
            Some(Value::Table(t)) => t.clone(),
            _ => return Err(at("the demand is a table".into())),
        };
        let step = args.integer("step")?;
        let seed = args.integer("seed")? as u64;
        let afts: Vec<Aft> = args.opt_param("afts")?.ok_or_else(|| at("no afts".into()))?;
        if afts.is_empty() {
            return Err(at("no afts".into()));
        }
        let search: f64 = args.param("search", 0.1)?;
        let rate: f64 = args.param("giving_up_rate", 0.1)?;
        // the demand's row of the step (its last row after it: a demand of one row holds for every step)
        let all = demand
            .rows
            .get((step.max(1) as usize - 1).min(demand.rows.len().saturating_sub(1)))
            .ok_or_else(|| at("the demand has no rows".into()))?;
        let map: BTreeMap<String, String> = match args.opt_param::<BTreeMap<String, String>>("services")? {
            Some(m) => m,
            None => demand.columns.iter().map(|c| (c.clone(), c.clone())).collect(),
        };
        let services: Vec<String> = map.keys().cloned().collect();
        let row: Vec<f64> = map
            .values()
            .map(|c| {
                demand
                    .columns
                    .iter()
                    .position(|x| x == c)
                    .map(|j| all[j])
                    .ok_or_else(|| at(format!("the demand has no column {c:?}")))
            })
            .collect::<Result<_>>()?;
        let row = &row;
        let capitals: Vec<(&str, &[f64])> = caps
            .keys
            .iter()
            .zip(&caps.layers)
            .filter_map(|(k, l)| match k {
                crate::Key::Name(n) => Some((n.as_str(), l.as_slice())),
                _ => None,
            })
            .collect();
        if caps.grid.cells() != land.data.len() {
            return Err(at("the capitals are not on the grid of the land".into()));
        }
        let (cur, before, after) =
            step_checked(&land.data, &capitals, &afts, &services, row, search, rate, seed, step as u64)
                .map_err(|e| at(e.0))?;
        let cover = cur
            .iter()
            .map(|v| {
                if v.is_nan() || *v < 1.0 { *v } else { afts.get(*v as usize - 1).and_then(|a| a.class).unwrap_or(*v) }
            })
            .collect();
        let mut info = Table::new(vec!["service".into(), "demand".into(), "supply_before".into(), "supply".into()]);
        for (s, _) in services.iter().enumerate() {
            info.rows.push(vec![s as f64, row[s], before[s], after[s]]);
        }
        Ok(outputs([
            ("land", Value::Map(Map::new(land.grid.clone(), cur))),
            ("cover", Value::Map(Map::new(land.grid.clone(), cover))),
            ("info", Value::Table(info)),
        ]))
    }
}

#[allow(clippy::too_many_arguments)]
fn step_checked(
    land: &[f64],
    capitals: &[(&str, &[f64])],
    afts: &[Aft],
    services: &[String],
    demand: &[f64],
    search: f64,
    rate: f64,
    seed: u64,
    s: u64,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    if !(0.0..=1.0).contains(&search) || !(0.0..=1.0).contains(&rate) {
        return Err(Error::msg("search and giving_up_rate are shares, 0 to 1"));
    }
    step(land, capitals, afts, services, demand, search, rate, seed, s)
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Allocate));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 40 x 40 cells: fertility rises to the east, forest suitability to the north.
    fn capitals() -> (Vec<f64>, Vec<f64>) {
        let (w, h) = (40, 40);
        let mut fert = Vec::new();
        let mut wood = Vec::new();
        for y in 0..h {
            for x in 0..w {
                fert.push((x as f64 + 0.5) / w as f64);
                wood.push(1.0 - (y as f64 + 0.5) / h as f64);
            }
        }
        (fert, wood)
    }

    fn afts() -> Vec<Aft> {
        serde_json::from_value(serde_json::json!([
            {"name": "farmer", "class": 2, "production": {"food": 1.0}, "sensitivity": {"fertility": 2.0}, "giving_up": 0.05, "giving_in": 0.1},
            {"name": "forester", "class": 3, "production": {"timber": 1.0}, "sensitivity": {"wood": 1.0}, "giving_up": 0.05, "giving_in": 0.1}
        ]))
        .unwrap()
    }

    fn run(seed: u64, steps: u64) -> (Vec<f64>, Vec<f64>) {
        let (fert, wood) = capitals();
        let caps: Vec<(&str, &[f64])> = vec![("fertility", &fert), ("wood", &wood)];
        let services = vec!["food".to_string(), "timber".to_string()];
        let demand = [300.0, 200.0];
        let mut land = vec![0.0; 1600];
        let mut supply = vec![0.0; 2];
        for s in 1..=steps {
            let (l, _, after) = step(&land, &caps, &afts(), &services, &demand, 0.2, 0.2, seed, s).unwrap();
            land = l;
            supply = after;
        }
        (land, supply)
    }

    #[test]
    fn supply_meets_demand() {
        let (land, supply) = run(1, 40);
        for (s, d) in supply.iter().zip([300.0, 200.0]) {
            assert!((s - d).abs() / d < 0.1, "supply {supply:?}");
        }
        // farmers where it is fertile, foresters where the wood grows
        let mean = |aft: f64, f: &dyn Fn(usize) -> f64| {
            let cells: Vec<usize> = (0..1600).filter(|&i| land[i] == aft).collect();
            cells.iter().map(|&i| f(i)).sum::<f64>() / cells.len() as f64
        };
        assert!(mean(1.0, &|i| (i % 40) as f64) > mean(2.0, &|i| (i % 40) as f64), "farmers are east");
        assert!(mean(2.0, &|i| (i / 40) as f64) < mean(1.0, &|i| (i / 40) as f64), "foresters are north");
    }

    #[test]
    fn the_same_seed_gives_the_same_map() {
        assert_eq!(run(7, 10).0, run(7, 10).0);
        assert_ne!(run(7, 10).0, run(8, 10).0);
    }

    #[test]
    fn missing_capitals_are_refused() {
        let caps: Vec<(&str, &[f64])> = vec![];
        let e = step(&[0.0; 4], &caps, &afts(), &["food".into()], &[1.0], 0.5, 0.1, 1, 1).err().unwrap();
        assert!(e.0.contains("fertility"), "{}", e.0);
    }
}
