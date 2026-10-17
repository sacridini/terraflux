//! The carbon of land change over a series of maps (the IPCC's stock-difference method): every cell's stock at
//! every date from its class (a stock per class, or a curve of regrowth from the date the class appeared there), and
//! the emissions and removals between the dates, by period and by transition (`carbon.emissions`).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::change_analysis::Series;
use crate::value::{Key, class_label};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Emissions));
}

/// How the stock of a class grows from the date it appears in a cell.
#[derive(Debug, Clone, Deserialize)]
pub struct Regrowth {
    /// The stock it tends to, tC/ha.
    pub max: f64,
    /// Chapman-Richards's rate (1/year), or the linear growth (tC/ha/year).
    pub rate: f64,
    /// Chapman-Richards's exponent (default 1).
    #[serde(default = "one")]
    pub shape: f64,
    /// chapman_richards (default) or linear (the rate every year up to the max).
    #[serde(default = "chapman")]
    pub form: String,
}

fn one() -> f64 {
    1.0
}

fn chapman() -> String {
    "chapman_richards".into()
}

impl Regrowth {
    /// The stock at an age, years.
    pub fn stock(&self, age: f64) -> f64 {
        if self.form == "linear" {
            (self.rate * age).min(self.max)
        } else {
            self.max * (1.0 - (-self.rate * age).exp()).powf(self.shape)
        }
    }
}

/// The stocks of every cell at every date, tC/ha: its class's stock, or its regrowth from the date the class
/// appeared in it (the stock of a class of regrowth already there at the first date is its `stock` entry, else its
/// curve's max). NaN where a map has no class.
pub fn stocks(
    maps: &[Arc<Vec<f64>>],
    times: &[f64],
    stock: &BTreeMap<u64, f64>,
    regrowth: &BTreeMap<u64, Regrowth>,
) -> std::result::Result<Vec<Vec<f64>>, String> {
    let key = |c: f64| c.to_bits();
    let n = maps[0].len();
    let mut out = vec![vec![f64::NAN; n]; maps.len()];
    for i in 0..n {
        // the date the current class appeared in the cell (none: it was there at the first date)
        let mut born: Option<f64> = None;
        for t in 0..maps.len() {
            let c = maps[t][i];
            if c.is_nan() {
                born = None;
                continue;
            }
            if t > 0 && maps[t - 1][i] != c {
                born = Some(times[t]);
            }
            out[t][i] = match (regrowth.get(&key(c)), born) {
                (Some(r), Some(b)) => r.stock(times[t] - b),
                (Some(r), None) => stock.get(&key(c)).copied().unwrap_or(r.max),
                (None, _) => *stock
                    .get(&key(c))
                    .ok_or_else(|| format!("the class {} has no stock (setting stock) nor regrowth", class_label(c)))?,
            };
        }
    }
    Ok(out)
}

/// A number per class from a setting {class: value}.
fn per_class<T: serde::de::DeserializeOwned>(args: &Args, name: &str) -> Result<BTreeMap<u64, T>> {
    let given: Option<BTreeMap<String, T>> = args.opt_param(name)?;
    let mut out = BTreeMap::new();
    for (k, v) in given.unwrap_or_default() {
        let c: f64 =
            k.trim().parse().map_err(|_| Error::msg(format!("node {:?}: {name}: {k:?} is not a class", args.node)))?;
        out.insert(c.to_bits(), v);
    }
    Ok(out)
}

struct Emissions;

impl Operator for Emissions {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "carbon.emissions",
            "The carbon emitted and removed by land change over a series of maps (the IPCC's stock-difference \
             method): every cell's stock at every date from its class (a stock per class, tC/ha; or a curve of \
             regrowth, Chapman-Richards or linear, from the date the class appeared in the cell, for secondary \
             forest), a fall an emission, a rise a removal; by period, by transition, and every cell's net emission \
             over the series. Transitions happen at the later date of a period.",
            Access::Global,
        )
        .input(PortSpec::new("maps", Type::Raster, "the maps of classes, a layer per date"))
        .output(PortSpec::new(
            "periods",
            Type::Table,
            "a row per period: year_from, year_to, emissions, removals, net (t of CO2, or of C)",
        ))
        .output(PortSpec::new(
            "transitions",
            Type::Table,
            "a row per period and pair of classes with cells: year_from, year_to, from, to, area_ha, emissions, \
             removals",
        ))
        .output(PortSpec::new("stocks", Type::Stack, "the stock of every cell at every date, tC/ha"))
        .output(PortSpec::new("net", Type::Map, "every cell's net emission over the series, t per hectare"))
        .param(PortSpec::new("stock", Type::Any, "the stock of each class, tC/ha: {class: stock}"))
        .param(PortSpec::optional(
            "regrowth",
            Type::Any,
            "the classes that grow from the date they appear: {class: {max, rate, shape, form}} (tC/ha; \
             Chapman-Richards's max (1 - exp(-rate age))^shape, or linear: rate tC/ha a year up to max)",
        ))
        .param(PortSpec::optional("co2", Type::Bool, "in tonnes of CO2 (C × 44/12; default true), else of carbon"))
        .param(PortSpec::optional("times", Type::Any, "the dates of the maps (default: the layers' names)"))
        .param(PortSpec::optional(
            "cell_area",
            Type::Number,
            "a cell's area in m2 (default: the cells' sides)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let s = Series::of(args, "maps")?;
        let stock: BTreeMap<u64, f64> = per_class(args, "stock")?;
        let regrowth: BTreeMap<u64, Regrowth> = per_class(args, "regrowth")?;
        if let Some(r) = regrowth.values().find(|r| r.form != "linear" && r.form != "chapman_richards") {
            return Err(fail(format!("regrowth: form {:?} is not chapman_richards or linear", r.form)));
        }
        let factor = if args.param("co2", true)? { 44.0 / 12.0 } else { 1.0 };
        let st = stocks(&s.maps, &s.times, &stock, &regrowth).map_err(fail)?;
        let ha = s.cell_area / 10_000.0;
        let n = st[0].len();
        let mut periods =
            Table::new(["year_from", "year_to", "emissions", "removals", "net"].map(String::from).to_vec());
        let mut transitions = Table::new(
            ["year_from", "year_to", "from", "to", "area_ha", "emissions", "removals"].map(String::from).to_vec(),
        );
        let mut net = vec![f64::NAN; n];
        for t in 1..st.len() {
            let (mut e, mut r) = (0.0, 0.0);
            // (from, to) -> cells, emissions, removals
            let mut by: BTreeMap<(u64, u64), (f64, f64, f64)> = BTreeMap::new();
            for i in 0..n {
                let (a, b) = (st[t - 1][i], st[t][i]);
                if a.is_nan() || b.is_nan() {
                    continue;
                }
                let d = (a - b) * factor;
                let (de, dr) = if d > 0.0 { (d * ha, 0.0) } else { (0.0, -d * ha) };
                e += de;
                r += dr;
                net[i] = if net[i].is_nan() { d } else { net[i] + d };
                let k = (s.maps[t - 1][i].to_bits(), s.maps[t][i].to_bits());
                let row = by.entry(k).or_insert((0.0, 0.0, 0.0));
                row.0 += 1.0;
                row.1 += de;
                row.2 += dr;
            }
            let (t0, t1) = (s.times[t - 1], s.times[t]);
            periods.rows.push(vec![t0, t1, e, r, e - r]);
            let mut rows: Vec<_> = by.into_iter().collect();
            rows.sort_by(|a, b| {
                f64::from_bits(a.0.0)
                    .total_cmp(&f64::from_bits(b.0.0))
                    .then(f64::from_bits(a.0.1).total_cmp(&f64::from_bits(b.0.1)))
            });
            for ((from, to), (cells, de, dr)) in rows {
                transitions.rows.push(vec![t0, t1, f64::from_bits(from), f64::from_bits(to), cells * ha, de, dr]);
            }
        }
        let stack = Stack {
            grid: s.grid.clone(),
            keys: s.times.iter().map(|t| Key::Name(class_label(*t))).collect(),
            layers: st.into_iter().map(Arc::new).collect(),
        };
        Ok(outputs([
            ("periods", Value::Table(periods)),
            ("transitions", Value::Table(transitions)),
            ("stocks", Value::Stack(stack)),
            ("net", Value::Map(Map::new(s.grid.clone(), net))),
        ]))
    }
}
