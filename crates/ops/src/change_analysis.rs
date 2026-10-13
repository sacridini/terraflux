//! The analysis of land change along a series of maps of classes, landschaft's `changes` (OpenLand's
//! `contingencyTable`, `intensityAnalysis` and `acc_changes`, diffeR's `crosstabm` and `diffTablej`): the
//! transitions between consecutive dates, the intensity analysis of Aldwaik & Pontius (2012) at the interval,
//! category and transition levels with their stationarity, the components of the difference (quantity, exchange
//! and shift; Pontius & Santacruz 2014), the number of changes of every cell, and the transitions worth modelling
//! (the largest, those that make most of the change). The same numbers as landschaft's (R's sums in 80 bits, as
//! landschaft's C++ makes them); text columns as numbers (a period by its first and last years; active or
//! targeted 1, dormant or avoided 0; a test passed 1).

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Crosstab));
    r.add(Arc::new(Transitions));
    r.add(Arc::new(Intensity));
    r.add(Arc::new(Components));
    r.add(Arc::new(Count));
    r.add(Arc::new(Suggest));
}

/// A series of maps of classes on one grid, their times, the classes found and a cell's area.
pub struct Series {
    pub maps: Vec<Arc<Vec<f64>>>,
    pub grid: Arc<terraflux_raster::Grid>,
    pub times: Vec<f64>,
    pub classes: Vec<f64>,
    pub cell_area: f64,
}

/// A time as Python writes it (a whole number without decimals).
fn time_text(t: f64) -> String {
    if t == t.trunc() && t.abs() < 1e15 { format!("{}", t as i64) } else { format!("{t}") }
}

impl Series {
    /// The maps of a node's port (a stack, a series of maps or rasters on disk, a layer per date), their times
    /// (`times`, else the layers' names when they are numbers, else 0, 1, 2...) and a cell's area (`cell_area`,
    /// else the product of the cells' sides).
    pub fn of(args: &Args, port: &str) -> Result<Series> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let v = args.get(port).ok_or_else(|| fail(format!("connect the maps ({port})")))?;
        Series::from_value(v, args.opt_param::<Vec<f64>>("times")?, args.opt_param::<f64>("cell_area")?)
            .map_err(|e| fail(format!("{port}: {e}")))
    }

    /// The series of a value (a stack, a series of maps, rasters on disk), its times and a cell's area given or
    /// not.
    pub fn from_value(v: &Value, times: Option<Vec<f64>>, cell_area: Option<f64>) -> Result<Series> {
        let fail = |m: String| Error::msg(m);
        let (maps, grid, names): (Vec<Arc<Vec<f64>>>, Arc<terraflux_raster::Grid>, Vec<String>) = match Some(v) {
            Some(Value::Stack(s)) => (s.layers.clone(), s.grid.clone(), s.keys.iter().map(|k| k.to_string()).collect()),
            Some(Value::Raster(r)) => {
                let st = r.stack()?;
                (st.layers, st.grid, r.layers.iter().map(|l| l.name.clone()).collect())
            }
            Some(Value::Series(items)) => {
                let ms: Vec<&Map> =
                    items.iter().filter_map(|v| if let Value::Map(m) = v { Some(m) } else { None }).collect();
                if ms.len() != items.len() || ms.is_empty() {
                    return Err(fail("a series of maps".into()));
                }
                (ms.iter().map(|m| m.data.clone()).collect(), ms[0].grid.clone(), Vec::new())
            }
            Some(v) => return Err(fail(format!("maps, a layer per date, not a {:?}", v.ty()))),
            None => unreachable!(),
        };
        if maps.len() < 2 {
            return Err(fail("two maps at least".into()));
        }
        let times: Vec<f64> = match times {
            Some(t) if t.len() == maps.len() => t,
            Some(t) => return Err(fail(format!("times: {} for {} maps", t.len(), maps.len()))),
            None => {
                let parsed: Option<Vec<f64>> = names.iter().map(|n| n.trim().parse::<f64>().ok()).collect();
                match parsed {
                    Some(t) if t.len() == maps.len() => t,
                    _ => (0..maps.len()).map(|i| i as f64).collect(),
                }
            }
        };
        let cell_area = match cell_area {
            Some(a) => a,
            None => (grid.transform[1] * grid.transform[5]).abs(),
        };
        let mut classes: Vec<f64> = maps.iter().flat_map(|m| ffi::map_classes(m)).collect();
        classes.sort_by(f64::total_cmp);
        classes.dedup();
        Ok(Series { maps, grid, times, classes, cell_area })
    }

    /// The cells of each class of map `i` that hold each class of map `j` (classes x classes, row-major).
    pub fn counts(&self, i: usize, j: usize) -> Result<Vec<i64>> {
        Ok(ffi::crosstab(&self.maps[i], &self.maps[j], &self.classes, &self.classes)?)
    }

    /// A period's name, as landschaft's: "t0-t1".
    fn period(&self, i: usize, j: usize) -> String {
        format!("{}-{}", time_text(self.times[i]), time_text(self.times[j]))
    }

    /// The transitions between the pairs of maps, a row per pair of classes with cells (OpenLand's
    /// `lulc_Multistep`).
    pub fn transitions(&self, pairs: &[(usize, usize)]) -> Result<Vec<Transition>> {
        let n = self.classes.len();
        let mut out = Vec::new();
        for &(i, j) in pairs {
            let c = self.counts(i, j)?;
            for r in 0..n {
                for s in 0..n {
                    let cells = c[r * n + s];
                    if cells != 0 {
                        out.push(Transition {
                            period: self.period(i, j),
                            from: self.classes[r],
                            to: self.classes[s],
                            km2: cells as f64 * self.cell_area / 1e6,
                            cells,
                            interval: self.times[j] - self.times[i],
                            year_from: self.times[i],
                            year_to: self.times[j],
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    /// The pairs of consecutive maps.
    pub fn steps(&self) -> Vec<(usize, usize)> {
        (0..self.maps.len() - 1).map(|k| (k, k + 1)).collect()
    }
}

/// A transition of a period: its classes, area and cells.
#[derive(Debug, Clone)]
pub struct Transition {
    pub period: String,
    pub from: f64,
    pub to: f64,
    pub km2: f64,
    pub cells: i64,
    pub interval: f64,
    pub year_from: f64,
    pub year_to: f64,
}

impl Transition {
    fn amount(&self, area: bool) -> f64 {
        if area { self.km2 } else { self.cells as f64 }
    }
}

/// A key of the groups of dplyr's group_by: text or a number, sorted as pandas sorts them.
#[derive(Debug, Clone, PartialEq)]
enum K {
    T(String),
    N(f64),
}

impl Eq for K {}

impl PartialOrd for K {
    fn partial_cmp(&self, o: &K) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for K {
    fn cmp(&self, o: &K) -> std::cmp::Ordering {
        match (self, o) {
            (K::T(a), K::T(b)) => a.cmp(b),
            (K::N(a), K::N(b)) => a.total_cmp(b),
            (K::T(_), K::N(_)) => std::cmp::Ordering::Less,
            (K::N(_), K::T(_)) => std::cmp::Ordering::Greater,
        }
    }
}

/// dplyr's group_by(keys) %>% summarise(sum(amount)): the groups sorted, each summed as R sums, in row order.
fn group_sum(rows: &[&Transition], keys: impl Fn(&Transition) -> Vec<K>, area: bool) -> BTreeMap<Vec<K>, f64> {
    let mut parts: BTreeMap<Vec<K>, Vec<f64>> = BTreeMap::new();
    for t in rows {
        parts.entry(keys(t)).or_default().push(t.amount(area));
    }
    parts.into_iter().map(|(k, v)| (k, ffi::r_sum(&v))).collect()
}

fn text(k: &K) -> &str {
    match k {
        K::T(s) => s,
        K::N(_) => "",
    }
}

fn number(k: &K) -> f64 {
    match k {
        K::N(v) => *v,
        K::T(_) => f64::NAN,
    }
}

/// The years a period's name gives.
fn years(p: &str, series: &Series) -> (f64, f64) {
    for i in 0..series.times.len() {
        for j in i + 1..series.times.len() {
            if series.period(i, j) == p {
                return (series.times[i], series.times[j]);
            }
        }
    }
    (f64::NAN, f64::NAN)
}

/// The intensity analysis: its tables.
pub struct IntensityTables {
    pub interval: Table,
    pub category_gain: Table,
    pub category_loss: Table,
    pub transition_gain: Table,
    pub transition_loss: Table,
    pub category_gain_stationarity: Table,
    pub category_loss_stationarity: Table,
    pub transition_gain_stationarity: Table,
    pub transition_loss_stationarity: Table,
}

/// The stationarity of rates along the periods (OpenLand's): for the rows whose rate is above their reference
/// (active, targeted) then below it (dormant, avoided), in how many periods each class is so, and whether in
/// every one.
fn stationarity(rows: &[(String, f64, f64, f64)], by: &str, count: &str) -> Table {
    let mut periods: Vec<&String> = rows.iter().map(|r| &r.0).collect();
    periods.sort();
    periods.dedup();
    let n = periods.len() as f64;
    let mut out = Table::new(vec![by.into(), count.into(), "N".into(), "above".into(), "test".into()]);
    for above in [true, false] {
        let mut counts: BTreeMap<K, usize> = BTreeMap::new();
        for (_, key, rate, reference) in rows {
            let hit = if above { rate > reference } else { rate < reference };
            if hit {
                *counts.entry(K::N(*key)).or_default() += 1;
            }
        }
        for (k, c) in counts {
            out.rows.push(vec![
                number(&k),
                c as f64,
                n,
                if above { 1.0 } else { 0.0 },
                if c as f64 == n { 1.0 } else { 0.0 },
            ]);
        }
    }
    out
}

/// The intensity analysis of the series' transitions between consecutive maps (OpenLand's `intensityAnalysis`):
/// the gains of class `gain` and the losses of class `loss` at the transition level; areas in km2 or cells.
pub fn intensity(series: &Series, gain: f64, loss: f64, area: bool) -> Result<IntensityTables> {
    let lulc = series.transitions(&series.steps())?;
    let unit = if area { "km2" } else { "pixel" };
    let all: Vec<&Transition> = lulc.iter().collect();
    // the total area: the first period's (by name), as OpenLand's totalArea
    let first = lulc.iter().map(|t| &t.period).min().cloned().unwrap_or_default();
    let ae = if area {
        ffi::r_sum(&lulc.iter().filter(|t| t.period == first).map(|t| t.km2).collect::<Vec<_>>())
    } else {
        lulc.iter().filter(|t| t.period == first).map(|t| t.cells).sum::<i64>() as f64
    };
    let changed: Vec<&Transition> = lulc.iter().filter(|t| t.from != t.to).collect();
    let p = |t: &Transition| K::T(t.period.clone());

    // interval level
    let eq1 = group_sum(&changed, |t| vec![p(t), K::N(t.interval)], area);
    let total_interval = series.times[series.times.len() - 1] - series.times[0];
    let u = ffi::r_sum(&changed.iter().map(|t| t.amount(area)).collect::<Vec<_>>()) / (total_interval * ae) * 100.0;
    let mut interval =
        Table::new(["year_from", "year_to", "PercentChange", "St", "U"].iter().map(|s| s.to_string()).collect());
    let mut st: BTreeMap<String, f64> = BTreeMap::new();
    for (k, intch) in &eq1 {
        let (period, iv) = (text(&k[0]).to_string(), number(&k[1]));
        let s = intch / (iv * ae) * 100.0;
        st.insert(period.clone(), s);
        let (a, b) = years(&period, series);
        interval.rows.push(vec![a, b, intch / ae * 100.0, s, u]);
    }

    // category level: the gains (to) or losses (from) of each class over its area
    let category = |to: bool, rate: &str, name: &str| -> (Table, Vec<(String, f64, f64, f64)>) {
        let side = |t: &Transition| K::N(if to { t.to } else { t.from });
        let num = group_sum(&changed, |t| vec![p(t), side(t), K::N(t.interval)], area);
        let den = group_sum(&all, |t| vec![p(t), side(t)], area);
        let mut out = Table::new(vec![
            "year_from".into(),
            "year_to".into(),
            if to { "To".into() } else { "From".into() },
            "Interval".into(),
            format!("{name}_{unit}"),
            rate.into(),
            "St".into(),
        ]);
        let mut rows = Vec::new();
        for (k, v) in &num {
            let period = text(&k[0]).to_string();
            let d = den.get(&vec![k[0].clone(), k[1].clone()]).copied().unwrap_or(f64::NAN);
            let iv = number(&k[2]);
            let r = v / (d * iv) * 100.0;
            let s = st.get(&period).copied().unwrap_or(f64::NAN);
            let (a, b) = years(&period, series);
            out.rows.push(vec![a, b, number(&k[1]), iv, *v, r, s]);
            rows.push((period, number(&k[1]), r, s));
        }
        (out, rows)
    };
    let (category_gain, cg) = category(true, "Gtj", "GG");
    let (category_loss, cl) = category(false, "Lti", "GL");

    // transition level: the gains of `gain` from each class, the losses of `loss` to each class
    let transition = |gains: bool| -> (Table, Vec<(String, f64, f64, f64)>) {
        let class = if gains { gain } else { loss };
        let mine: Vec<&Transition> =
            changed.iter().copied().filter(|t| if gains { t.to == class } else { t.from == class }).collect();
        let others: Vec<&Transition> =
            all.iter().copied().filter(|t| if gains { t.from != class } else { t.to != class }).collect();
        // (the other side: from for gains, to for losses)
        let other = |t: &Transition| K::N(if gains { t.from } else { t.to });
        let own = |t: &Transition| K::N(if gains { t.to } else { t.from });
        let num_a = group_sum(&mine, |t| vec![p(t), other(t), K::N(t.interval)], area);
        let den_a = group_sum(&others, |t| vec![p(t), other(t)], area);
        let num_b = group_sum(&mine, |t| vec![p(t), own(t), K::N(t.interval)], area);
        let den_b = group_sum(&others, |t| vec![p(t)], area);
        let (rate, uniform, amount) = if gains { ("Rtin", "Wtn", "T_i2n") } else { ("Qtmj", "Vtm", "T_m2j") };
        let mut out = Table::new(vec![
            "year_from".into(),
            "year_to".into(),
            if gains { "From".into() } else { "To".into() },
            if gains { "To".into() } else { "From".into() },
            "Interval".into(),
            format!("{amount}_{unit}"),
            rate.into(),
            uniform.into(),
        ]);
        let mut rows = Vec::new();
        for (k, v) in &num_a {
            let period = text(&k[0]).to_string();
            let iv = number(&k[2]);
            let d = den_a.get(&vec![k[0].clone(), k[1].clone()]).copied().unwrap_or(f64::NAN);
            let r = v / (iv * d) * 100.0;
            // the uniform intensity of the period (and interval): the class's change over the others' area
            let matches: Vec<(f64, f64)> = num_b
                .iter()
                .filter(|(kb, _)| text(&kb[0]) == period && number(&kb[2]) == iv)
                .map(|(kb, vb)| {
                    let db = den_b.get(&vec![kb[0].clone()]).copied().unwrap_or(f64::NAN);
                    (number(&kb[1]), vb / (iv * db) * 100.0)
                })
                .collect();
            let (a, b) = years(&period, series);
            if matches.is_empty() {
                out.rows.push(vec![a, b, number(&k[1]), f64::NAN, iv, *v, r, f64::NAN]);
                rows.push((period.clone(), number(&k[1]), r, f64::NAN));
            }
            for (c, w) in matches {
                out.rows.push(vec![a, b, number(&k[1]), c, iv, *v, r, w]);
                rows.push((period.clone(), number(&k[1]), r, w));
            }
        }
        (out, rows)
    };
    let (transition_gain, tg) = transition(true);
    let (transition_loss, tl) = transition(false);
    Ok(IntensityTables {
        interval,
        category_gain,
        category_loss,
        transition_gain,
        transition_loss,
        category_gain_stationarity: stationarity(&cg, "To", "Gain"),
        category_loss_stationarity: stationarity(&cl, "From", "Loss"),
        transition_gain_stationarity: stationarity(&tg, "From", "Loss"),
        transition_loss_stationarity: stationarity(&tl, "To", "Gain"),
    })
}

/// A square matrix of counts without its classes found in neither map (landschaft's crosstab): the classes kept
/// and the matrix (row-major).
pub fn dropped(counts: &[i64], classes: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = classes.len();
    let keep: Vec<usize> = (0..n)
        .filter(|&i| {
            (0..n).map(|j| counts[i * n + j]).sum::<i64>() > 0 || (0..n).map(|j| counts[j * n + i]).sum::<i64>() > 0
        })
        .collect();
    let m = keep.iter().flat_map(|&i| keep.iter().map(move |&j| counts[i * n + j] as f64)).collect();
    (keep.iter().map(|&i| classes[i]).collect(), m)
}

fn r_sums_rows(m: &[f64], n: usize) -> Vec<f64> {
    (0..n).map(|i| ffi::r_sum(&m[i * n..(i + 1) * n])).collect()
}

fn r_sums_cols(m: &[f64], n: usize) -> Vec<f64> {
    (0..n).map(|j| ffi::r_sum(&(0..n).map(|i| m[i * n + j]).collect::<Vec<_>>())).collect()
}

/// diffeR's components of each category of a square matrix (n x n, row-major): omission (gain), agreement
/// (persistence), commission (loss), quantity, exchange, shift.
pub fn components(m: &[f64], n: usize) -> Vec<[f64; 6]> {
    let (rows, cols) = (r_sums_rows(m, n), r_sums_cols(m, n));
    // exchange: twice the smaller of the two directions, the lower triangle's row and column sums
    let mut lt = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..i {
            lt[i * n + j] = 2.0 * m[i * n + j].min(m[j * n + i]);
        }
    }
    let exchange: Vec<f64> = r_sums_rows(&lt, n).iter().zip(r_sums_cols(&lt, n)).map(|(a, b)| a + b).collect();
    (0..n)
        .map(|k| {
            let diag = m[k * n + k];
            let omission = cols[k] - diag;
            let commission = rows[k] - diag;
            let agreement = rows[k] - commission;
            let quantity = (rows[k] - cols[k]).abs();
            let shift = (rows[k] + cols[k] - 2.0 * diag) - quantity - exchange[k];
            [omission, agreement, commission, quantity, exchange[k], shift]
        })
        .collect()
}

/// diffeR's diffTablej: the components of each category and the overall row (their sums, the quantity, exchange
/// and shift halved).
pub fn component_table(m: &[f64], classes: &[f64], change: bool) -> Table {
    let n = classes.len();
    let comp = components(m, n);
    let names: [&str; 3] =
        if change { ["Gain", "Persistence", "Loss"] } else { ["Omission", "Agreement", "Comission"] };
    let mut t = Table::new(
        std::iter::once("Category".to_string())
            .chain(names.iter().map(|s| s.to_string()))
            .chain(["Quantity", "Exchange", "Shift"].iter().map(|s| s.to_string()))
            .collect(),
    );
    for (k, c) in comp.iter().enumerate() {
        t.rows.push(std::iter::once(classes[k]).chain(c.iter().copied()).collect());
    }
    let mut overall: Vec<f64> = (0..6).map(|j| ffi::r_sum(&comp.iter().map(|c| c[j]).collect::<Vec<_>>())).collect();
    for v in &mut overall[3..] {
        *v /= 2.0;
    }
    t.rows.push(std::iter::once(f64::NAN).chain(overall).collect());
    t
}

/// diffeR's overall difference and its components: quantity, exchange, shift, allocation, overall.
pub fn overall(m: &[f64], n: usize) -> [f64; 5] {
    let (rows, cols) = (r_sums_rows(m, n), r_sums_cols(m, n));
    let by_column: Vec<f64> = (0..n).flat_map(|j| (0..n).map(move |i| m[i * n + j])).collect();
    let diag: Vec<f64> = (0..n).map(|k| m[k * n + k]).collect();
    let total = ffi::r_sum(&by_column) - ffi::r_sum(&diag);
    let quantity = ffi::r_sum(&rows.iter().zip(&cols).map(|(a, b)| (a - b).abs()).collect::<Vec<_>>()) / 2.0;
    let comp = components(m, n);
    let exchange = ffi::r_sum(&comp.iter().map(|c| c[4]).collect::<Vec<_>>()) / 2.0;
    let shift = ffi::r_sum(&comp.iter().map(|c| c[5]).collect::<Vec<_>>()) / 2.0;
    [quantity, exchange, shift, total - quantity, total]
}

/// How many times each cell changed along the maps (-1 where any map has no value), and the cells of each
/// count with their share (OpenLand's acc_changes).
pub fn change_count(maps: &[Arc<Vec<f64>>]) -> (Vec<f64>, Table) {
    let n = maps[0].len();
    let mut out = vec![0.0; n];
    for (i, o) in out.iter_mut().enumerate() {
        if maps.iter().any(|m| m[i].is_nan()) {
            *o = -1.0;
            continue;
        }
        *o = maps.windows(2).filter(|w| w[0][i] != w[1][i]).count() as f64;
    }
    let mut hist: BTreeMap<i64, i64> = BTreeMap::new();
    for &v in &out {
        if v >= 0.0 {
            *hist.entry(v as i64).or_default() += 1;
        }
    }
    let total: i64 = hist.values().sum();
    let mut t = Table::new(vec!["PxValue".into(), "Qt".into(), "Percent".into()]);
    for (v, c) in hist {
        t.rows.push(vec![v as f64, c as f64, c as f64 / total as f64 * 100.0]);
    }
    (out, t)
}

fn maps_port(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new("maps", Type::Raster, "the maps of classes, a layer per date (two at least)"))
        .param(PortSpec::optional(
            "times",
            Type::Any,
            "the dates of the maps (default: the layers' names, else 0, 1...)",
        ))
        .param(PortSpec::optional("cell_area", Type::Number, "a cell's area in m2 (default: the cells' sides)"))
}

struct Crosstab;

impl Operator for Crosstab {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "change.crosstab",
            "The transition matrix between two maps of classes (landschaft's crosstab, diffeR's crosstabm): the \
             cells of each class of the first (rows) that hold each class of the second (columns); classes found \
             in neither left out.",
            Access::Global,
        )
        .input(PortSpec::new("from", Type::Map, "the earlier (or compared) map"))
        .input(PortSpec::new("to", Type::Map, "the later (or reference) map"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per class of the first map: its class, then the cells to each",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (a, b) = (args.map("from")?, args.map("to")?);
        if a.data.len() != b.data.len() {
            return Err(Error::msg(format!("node {:?}: the maps differ in size", args.node)));
        }
        let mut classes: Vec<f64> = ffi::map_classes(&a.data).into_iter().chain(ffi::map_classes(&b.data)).collect();
        classes.sort_by(f64::total_cmp);
        classes.dedup();
        let counts = ffi::crosstab(&a.data, &b.data, &classes, &classes)?;
        let (kept, m) = dropped(&counts, &classes);
        Ok(outputs([("table", Value::Table(matrix_table(&kept, &m)))]))
    }
}

/// A square matrix as a table: a row per class, its class then a column per class.
fn matrix_table(classes: &[f64], m: &[f64]) -> Table {
    let n = classes.len();
    let mut t = Table::new(
        std::iter::once("from".to_string()).chain(classes.iter().map(|c| crate::value::class_label(*c))).collect(),
    );
    t.rows =
        (0..n).map(|i| std::iter::once(classes[i]).chain(m[i * n..(i + 1) * n].iter().copied()).collect()).collect();
    t
}

struct Transitions;

impl Operator for Transitions {
    fn signature(&self) -> OpSignature {
        maps_port(OpSignature::new(
            "change.transitions",
            "The transitions of a series of maps of classes, as landschaft's changes (OpenLand's \
             contingencyTable): between consecutive dates (and from the first to the last), a row per pair of \
             classes with cells: the years, the classes, the area in km2 (cells times a cell's area) and the \
             cells; and the cells of each class at each date.",
            Access::Global,
        ))
        .output(PortSpec::new("table", Type::Table, "year_from, year_to, from, to, km2, cells, interval"))
        .output(PortSpec::new("one_step", Type::Table, "the same, from the first date to the last"))
        .output(PortSpec::new("areas", Type::Table, "the cells of each class at each date"))
        .output(PortSpec::new("matrix", Type::Table, "the transition matrix from the first date to the last"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = Series::of(args, "maps")?;
        let as_table = |rows: Vec<Transition>| {
            let mut t = Table::new(
                ["year_from", "year_to", "from", "to", "km2", "cells", "interval"]
                    .iter()
                    .map(|c| c.to_string())
                    .collect(),
            );
            t.rows = rows
                .into_iter()
                .map(|r| vec![r.year_from, r.year_to, r.from, r.to, r.km2, r.cells as f64, r.interval])
                .collect();
            t
        };
        let last = s.maps.len() - 1;
        let table = as_table(s.transitions(&s.steps())?);
        let one = as_table(s.transitions(&[(0, last)])?);
        let n = s.classes.len();
        let mut areas = Table::new(
            std::iter::once("time".to_string())
                .chain(s.classes.iter().map(|c| crate::value::class_label(*c)))
                .collect(),
        );
        for (k, t) in s.times.iter().enumerate() {
            let c = s.counts(k, k)?;
            areas.rows.push(std::iter::once(*t).chain((0..n).map(|i| c[i * n + i] as f64)).collect());
        }
        let (kept, m) = dropped(&s.counts(0, last)?, &s.classes);
        Ok(outputs([
            ("table", Value::Table(table)),
            ("one_step", Value::Table(one)),
            ("areas", Value::Table(areas)),
            ("matrix", Value::Table(matrix_table(&kept, &m))),
        ]))
    }
}

struct Intensity;

impl Operator for Intensity {
    fn signature(&self) -> OpSignature {
        let mut sig = maps_port(OpSignature::new(
            "change.intensity",
            "The intensity analysis of land change (Aldwaik & Pontius 2012) as OpenLand's intensityAnalysis \
             through landschaft: at the interval level how fast the map changed in each period against a uniform \
             rate; at the category level each class's gains and losses against the period's rate (active or \
             dormant); at the transition level which classes the gains of one class (gain) take, and which take \
             the losses of another (loss), against uniform (targeted or avoided); with the stationarity of each \
             along the periods. Rates in % a year (a time unit); areas in km2 or cells.",
            Access::Global,
        ))
        .param(PortSpec::new("gain", Type::Number, "the class whose gains the transition level examines"))
        .param(PortSpec::new("loss", Type::Number, "the class whose losses the transition level examines"))
        .param(PortSpec::optional("area", Type::Bool, "areas in km2 (default) or in cells"));
        for (port, about) in [
            ("interval", "per period: PercentChange, St (the period's rate), U (the uniform rate)"),
            ("category_gain", "per period and class: its gross gain, Gtj, St"),
            ("category_loss", "per period and class: its gross loss, Lti, St"),
            ("transition_gain", "the gains of `gain` from each class: area, Rtin, Wtn (uniform)"),
            ("transition_loss", "the losses of `loss` to each class: area, Qtmj, Vtm (uniform)"),
            ("category_gain_stationarity", "per class: periods active (above 1) or dormant (0), every one (test)"),
            ("category_loss_stationarity", "per class: periods active (1) or dormant (0) losing"),
            ("transition_gain_stationarity", "per class: periods targeted (1) or avoided (0) by gain's gains"),
            ("transition_loss_stationarity", "per class: periods targeting (1) or avoiding (0) loss's losses"),
        ] {
            sig = sig.output(PortSpec::new(port, Type::Table, about));
        }
        sig
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = Series::of(args, "maps")?;
        let gain: f64 = args.param("gain", f64::NAN)?;
        let loss: f64 = args.param("loss", f64::NAN)?;
        if !s.classes.contains(&gain) || !s.classes.contains(&loss) {
            return Err(Error::msg(format!(
                "node {:?}: gain and loss are classes of the maps ({})",
                args.node,
                s.classes.iter().map(|c| crate::value::class_label(*c)).collect::<Vec<_>>().join(", ")
            )));
        }
        let t = intensity(&s, gain, loss, args.param("area", true)?)?;
        Ok(outputs([
            ("interval", Value::Table(t.interval)),
            ("category_gain", Value::Table(t.category_gain)),
            ("category_loss", Value::Table(t.category_loss)),
            ("transition_gain", Value::Table(t.transition_gain)),
            ("transition_loss", Value::Table(t.transition_loss)),
            ("category_gain_stationarity", Value::Table(t.category_gain_stationarity)),
            ("category_loss_stationarity", Value::Table(t.category_loss_stationarity)),
            ("transition_gain_stationarity", Value::Table(t.transition_gain_stationarity)),
            ("transition_loss_stationarity", Value::Table(t.transition_loss_stationarity)),
        ]))
    }
}

struct Components;

impl Operator for Components {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "change.components",
            "The components of the difference between two maps of classes (Pontius & Santacruz 2014, diffeR's \
             diffTablej and overallDiff through landschaft): of each class its gain (omission), persistence \
             (agreement) and loss (commission), and the difference split into quantity, exchange and shift; \
             overall too (the last row, class empty).",
            Access::Global,
        )
        .input(PortSpec::optional("from", Type::Map, "the earlier (or compared) map"))
        .input(PortSpec::optional("to", Type::Map, "the later (or reference) map"))
        .input(PortSpec::optional("maps", Type::Raster, "else a series of maps: from its first to its last"))
        .output(PortSpec::new("table", Type::Table, "per class and overall: the components"))
        .output(PortSpec::new("overall", Type::Table, "quantity, exchange, shift, allocation, overall (cells)"))
        .param(
            PortSpec::optional("analysis", Type::Text, "change (default: gain, persistence, loss) or error")
                .choices(["change", "error"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (a, b): (Arc<Vec<f64>>, Arc<Vec<f64>>) = match (args.opt_map("from")?, args.opt_map("to")?) {
            (Some(a), Some(b)) => (a.data.clone(), b.data.clone()),
            _ => {
                let s = Series::of(args, "maps")?;
                (s.maps[0].clone(), s.maps[s.maps.len() - 1].clone())
            }
        };
        if a.len() != b.len() {
            return Err(Error::msg(format!("node {:?}: the maps differ in size", args.node)));
        }
        let analysis: String = args.param("analysis", "change".to_string())?;
        let mut classes: Vec<f64> = ffi::map_classes(&a).into_iter().chain(ffi::map_classes(&b)).collect();
        classes.sort_by(f64::total_cmp);
        classes.dedup();
        let counts = ffi::crosstab(&a, &b, &classes, &classes)?;
        let (kept, m) = dropped(&counts, &classes);
        let table = component_table(&m, &kept, analysis != "error");
        let o = overall(&m, kept.len());
        let mut ov = Table::new(
            ["quantity", "exchange", "shift", "allocation", "overall"].iter().map(|c| c.to_string()).collect(),
        );
        ov.rows.push(o.to_vec());
        Ok(outputs([("table", Value::Table(table)), ("overall", Value::Table(ov))]))
    }
}

struct Count;

impl Operator for Count {
    fn signature(&self) -> OpSignature {
        maps_port(OpSignature::new(
            "change.count",
            "How many times each cell changed along a series of maps of classes (OpenLand's acc_changes through \
             landschaft): a map of the counts (none where any map has no value) and the cells of each count.",
            Access::Global,
        ))
        .output(PortSpec::new("map", Type::Map, "the changes of each cell"))
        .output(PortSpec::new("table", Type::Table, "PxValue (the count), Qt (cells), Percent"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = Series::of(args, "maps")?;
        let (counts, table) = change_count(&s.maps);
        let map = counts.into_iter().map(|v| if v < 0.0 { f64::NAN } else { v }).collect();
        Ok(outputs([("map", Value::Map(Map::new(s.grid.clone(), map))), ("table", Value::Table(table))]))
    }
}

/// The transitions worth modelling, from the first map to the last: each change (a class to another) with its
/// cells, its share of all the change and of its class's area; suggested (1) when its share of the change is at
/// least `least` %, the largest first.
pub fn suggest(s: &Series, least: f64) -> Result<Table> {
    let last = s.maps.len() - 1;
    let rows = s.transitions(&[(0, last)])?;
    let changed: Vec<&Transition> = rows.iter().filter(|t| t.from != t.to).collect();
    let all: f64 = changed.iter().map(|t| t.cells as f64).sum();
    let mut by_class: BTreeMap<K, i64> = BTreeMap::new();
    for t in &rows {
        *by_class.entry(K::N(t.from)).or_default() += t.cells;
    }
    let mut out = Table::new(
        ["from", "to", "cells", "km2", "of_change", "of_class", "suggested"].iter().map(|c| c.to_string()).collect(),
    );
    let mut sorted = changed.clone();
    sorted.sort_by(|a, b| b.cells.cmp(&a.cells).then(a.from.total_cmp(&b.from)).then(a.to.total_cmp(&b.to)));
    for t in sorted {
        let share = t.cells as f64 / all * 100.0;
        let of_class = t.cells as f64 / by_class[&K::N(t.from)] as f64 * 100.0;
        out.rows.push(vec![
            t.from,
            t.to,
            t.cells as f64,
            t.km2,
            share,
            of_class,
            if share >= least { 1.0 } else { 0.0 },
        ]);
    }
    Ok(out)
}

struct Suggest;

impl Operator for Suggest {
    fn signature(&self) -> OpSignature {
        maps_port(OpSignature::new(
            "change.suggest",
            "The transitions worth modelling, from the first map of a series to the last: every change of a class \
             to another with its cells, its share of all the change and of the class's area, the largest first; \
             suggested those that make at least `least` % of the change (the others are too small to fit a model \
             to).",
            Access::Global,
        ))
        .output(PortSpec::new("table", Type::Table, "from, to, cells, km2, of_change, of_class (%), suggested (1)"))
        .output(PortSpec::new("transitions", Type::Any, "the transitions suggested, [[from, to], ...]"))
        .param(PortSpec::optional(
            "least",
            Type::Number,
            "the least share of the change to suggest, % (default 1)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = Series::of(args, "maps")?;
        let t = suggest(&s, args.param("least", 1.0)?)?;
        let kept: Vec<serde_json::Value> =
            t.rows.iter().filter(|r| r[6] == 1.0).map(|r| serde_json::json!([r[0], r[1]])).collect();
        Ok(outputs([("table", Value::Table(t)), ("transitions", Value::Json(serde_json::Value::Array(kept)))]))
    }
}
