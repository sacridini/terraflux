//! Ecosystem services: InVEST's models (natcap.invest 3.20) as landschaft reimplements them (`ls.services`), through
//! its C++ and with its arithmetic: carbon storage and sequestration, habitat quality (degradation, quality,
//! rarity), annual water yield (with consumption and hydropower), and the sediment delivery ratio with its index of
//! connectivity. The maps are InVEST's float32 (or float64) bits; numpy's float32 exp, sin and cos, which numpy
//! computes with its own approximations, through [`crate::npmath`].

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::hydro::MFD_BLOCK;
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, npmath, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Carbon));
    r.add(Arc::new(Habitat));
    r.add(Arc::new(Water));
    r.add(Arc::new(Sdr));
    r.add(Arc::new(ndr::Ndr));
    r.add(Arc::new(pollination::Pollination));
    r.add(Arc::new(swy::SeasonalWaterYield));
}

mod ndr;
mod pollination;
mod swy;

const POOLS: [&str; 4] = ["c_above", "c_below", "c_soil", "c_dead"];
/// kWh produced by a cubic metre falling one metre (InVEST's constant).
const KWH_PER_M3_M: f64 = 0.00272;

fn threads() -> i32 {
    crate::threads::current() as i32
}

/// A land use map: its integer codes, which cells have one, its grid (landschaft's LandUse).
struct LandUse<'a> {
    map: &'a Map,
    codes: Vec<i64>,
    valid: Vec<u8>,
    what: &'static str,
}

impl<'a> LandUse<'a> {
    fn new(map: &'a Map, what: &'static str) -> Result<Self> {
        let mut codes = Vec::with_capacity(map.data.len());
        let mut valid = Vec::with_capacity(map.data.len());
        for v in map.data.iter() {
            if v.is_nan() {
                codes.push(0);
                valid.push(0);
            } else {
                if v.fract() != 0.0 {
                    return Err(Error::msg(format!("{what} must hold integer class codes")));
                }
                codes.push(*v as i64);
                valid.push(1);
            }
        }
        Ok(LandUse { map, codes, valid, what })
    }

    /// Cell width and height in metres: a projected map, as InVEST requires.
    fn pixel(&self) -> Result<(f64, f64)> {
        if crate::surface::extent_of(&self.map.grid).1 {
            return Err(Error::msg(format!(
                "{} is in longitude / latitude: give it projected, in metres (as InVEST)",
                self.what
            )));
        }
        let t = self.map.grid.transform;
        Ok((t[1].abs(), t[5].abs()))
    }

    /// The row of `codes` (sorted) of every cell; -1 where the map has no value.
    fn classes(&self, codes: &[i64]) -> Result<Vec<i32>> {
        self.classes_of(codes, &self.valid)
    }

    fn classes_of(&self, codes: &[i64], valid: &[u8]) -> Result<Vec<i32>> {
        let r = ffi::serv_classify(&self.codes, valid, codes, threads())?;
        if r.first >= 0 {
            let mut missing: Vec<i64> =
                (0..self.codes.len()).filter(|&i| valid[i] == 1 && r.classes[i] == -2).map(|i| self.codes[i]).collect();
            missing.sort_unstable();
            missing.dedup();
            return Err(Error::msg(format!("{} has class(es) {missing:?} that the table lacks", self.what)));
        }
        Ok(r.classes)
    }

    fn same_grid(&self, other: &LandUse) -> Result<()> {
        if self.map.shape() != other.map.shape() {
            return Err(Error::msg(format!(
                "{} has {:?} cells, {} {:?}: give them on the same grid",
                other.what,
                other.map.shape(),
                self.what,
                self.map.shape()
            )));
        }
        Ok(())
    }
}

/// A table by land use code (landschaft's lookup_table): the codes sorted and the columns (names not
/// case-sensitive), a vector per column.
fn lookup(t: &Table, columns: &[&str], what: &str, index: &str) -> Result<(Vec<i64>, Vec<Vec<f64>>)> {
    let lower: Vec<String> = t.columns.iter().map(|c| c.trim().to_lowercase()).collect();
    let col = |name: &str| lower.iter().position(|c| *c == name.to_lowercase());
    let key = col(index).ok_or_else(|| Error::msg(format!("{what} lacks the column {index:?}")))?;
    let missing: Vec<&str> = columns.iter().copied().filter(|c| col(c).is_none()).collect();
    if !missing.is_empty() {
        return Err(Error::msg(format!("{what} lacks the column(s) {missing:?}")));
    }
    let mut rows: Vec<(i64, &Vec<f64>)> = Vec::with_capacity(t.rows.len());
    for r in &t.rows {
        let k = r[key];
        if k.is_nan() || k.fract() != 0.0 {
            return Err(Error::msg(format!("{what} has a {index} that is not a whole number")));
        }
        rows.push((k as i64, r));
    }
    rows.sort_by_key(|r| r.0);
    if rows.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err(Error::msg(format!("{what} lists a class more than once")));
    }
    let mut out = Vec::with_capacity(columns.len());
    for c in columns {
        let j = col(c).unwrap();
        let v: Vec<f64> = rows.iter().map(|r| r.1[j]).collect();
        if v.iter().any(|x| x.is_nan()) {
            return Err(Error::msg(format!("{what} has empty values")));
        }
        out.push(v);
    }
    Ok((rows.iter().map(|r| r.0).collect(), out))
}

fn table_of<'a>(args: &'a Args, port: &str) -> Result<&'a Table> {
    match args.get(port) {
        Some(Value::Table(t)) => Ok(t),
        Some(v) => Err(Error::msg(format!("node {:?}: {port} is a {:?}, not a table", args.node, v.ty()))),
        None => Err(Error::msg(format!("node {:?}: connect the table {port}", args.node))),
    }
}

fn opt_table<'a>(args: &'a Args, port: &str) -> Result<Option<&'a Table>> {
    if args.get(port).is_none() { Ok(None) } else { table_of(args, port).map(Some) }
}

fn f32_map(grid: &Arc<Grid>, v: &[f32]) -> Value {
    Value::Map(Map::new(grid.clone(), v.iter().map(|&x| f64::from(x)).collect()))
}

fn f64_map(grid: &Arc<Grid>, v: Vec<f64>) -> Value {
    Value::Map(Map::new(grid.clone(), v))
}

/// The polygons of features in the grid's coordinate system.
fn polygons(v: &Arc<Vector>, grid: &Grid) -> Result<Arc<Vector>> {
    let v = crate::vector::on_grid(v, grid)?;
    if v.geometries.iter().any(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
        return Err(Error::msg("the watersheds are polygons"));
    }
    Ok(v)
}

/// Statistics (landschaft's codes: 0 count, 2 sum) of maps in every polygon (the cells whose centre is inside):
/// (polygons, maps, stats).
fn zonal(v: &Vector, grid: &Grid, maps: &[&[f64]], stats: &[i32]) -> Result<Vec<f64>> {
    let geoms: Vec<&Geometry> = v.geometries.iter().collect();
    let runs = crate::vector::runs(&geoms, grid, false)?;
    let values: Vec<f64> = maps.iter().flat_map(|m| m.iter().copied()).collect();
    Ok(ffi::io_zonal_stats(
        &values,
        maps.len() as i32,
        grid.height as i32,
        grid.width as i32,
        &runs,
        stats,
        &vec![0.0; stats.len()],
    )?)
}

/// The cells whose centre lies in a polygon.
fn inside(v: &Vector, grid: &Grid) -> Result<Vec<bool>> {
    let geoms: Vec<&Geometry> = v.geometries.iter().collect();
    let runs = crate::vector::runs(&geoms, grid, false)?;
    let mut out = vec![false; grid.cells()];
    for r in 0..runs.y.len() {
        let row = runs.y[r] as usize * grid.width;
        for x in runs.x0[r]..runs.x1[r] {
            out[row + x as usize] = true;
        }
    }
    Ok(out)
}

/// A numeric attribute of features, by its name (not case-sensitive).
fn attribute(v: &Vector, name: &str, what: &str) -> Result<Vec<f64>> {
    for (n, c) in &v.attributes {
        if n.to_lowercase() == name {
            return match c {
                Column::Numbers(x) => Ok(x.clone()),
                Column::Texts(_) => Err(Error::msg(format!("the {what}' {name} are not numbers"))),
            };
        }
    }
    Err(Error::msg(format!("the {what} need a {name:?} attribute")))
}

/// A polygon's planar area (the shoelace over its rings, the holes taken off).
fn area(g: &Geometry) -> f64 {
    let ring = |r: &Vec<[f64; 2]>| -> f64 {
        let n = r.len();
        if n < 3 {
            return 0.0;
        }
        let x0 = r[0][0];
        let mut s = 0.0;
        for i in 1..n - 1 {
            s += (r[i][0] - x0) * (r[i + 1][1] - r[i - 1][1]);
        }
        (s / 2.0).abs()
    };
    let poly = |p: &Vec<Vec<[f64; 2]>>| -> f64 {
        p.iter().enumerate().map(|(k, r)| if k == 0 { ring(r) } else { -ring(r) }).sum()
    };
    match g {
        Geometry::Polygon(p) => poly(p),
        Geometry::MultiPolygon(m) => m.iter().map(poly).sum(),
        _ => 0.0,
    }
}

// -- carbon ---------------------------------------------------------------------------------------------------

/// The factor turning a change in carbon into its net present value (InVEST's _calculate_valuation_constant).
pub fn valuation_constant(
    price: f64,
    discount_rate: f64,
    rate_change: f64,
    year: i64,
    future_year: i64,
) -> Result<f64> {
    let n = future_year - year;
    if n <= 0 {
        return Err(Error::msg(format!("the future year ({future_year}) must come after the baseline year ({year})")));
    }
    let ratio = 1.0 / ((1.0 + discount_rate / 100.0) * (1.0 + rate_change / 100.0));
    let mut constant = price / n as f64;
    if ratio == 1.0 {
        constant *= n as f64;
    } else {
        constant *= (1.0 - ratio.powf(n as f64)) / (1.0 - ratio);
    }
    Ok(constant)
}

struct Carbon;

impl Operator for Carbon {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "services.carbon",
            "Carbon storage of a landscape and its change between two land use maps: InVEST's carbon model \
             (natcap.invest 3.20) bit for bit, through landschaft. Each class holds a density (t C / ha) in four \
             pools (above and below ground biomass, soil, dead matter); the storage is their sum, the \
             sequestration the future minus the baseline, valued with a price.",
            Access::Global,
        )
        .input(PortSpec::new("lulc", Type::Map, "the baseline land use (class codes; projected, metres)"))
        .input(PortSpec::new("pools", Type::Table, "by class: lucode, c_above, c_below, c_soil, c_dead (Mg C / ha)"))
        .input(PortSpec::optional("future", Type::Map, "the alternate (future) land use, on the same grid"))
        .output(PortSpec::new("storage", Type::Map, "the baseline's carbon, t / ha (float32)"))
        .output(PortSpec::new("pools", Type::Stack, "each pool's map (c_above, c_below, c_soil, c_dead)"))
        .output(PortSpec::new("storage_future", Type::Map, "the future's carbon (with future)"))
        .output(PortSpec::new("pools_future", Type::Stack, "the future's pools (with future)"))
        .output(PortSpec::new("change", Type::Map, "sequestration: future minus baseline (with future)"))
        .output(PortSpec::new("npv", Type::Map, "the change's net present value per ha (with a price)"))
        .output(PortSpec::new("totals", Type::Table, "tons of carbon: baseline, alternate, change; npv in currency"))
        .param(PortSpec::optional("price", Type::Number, "of a ton of carbon (valuation of the change)"))
        .param(PortSpec::optional("discount_rate", Type::Number, "per year, percent"))
        .param(PortSpec::optional("rate_change", Type::Number, "of the price per year, percent (0)"))
        .param(PortSpec::optional("year", Type::Integer, "of the baseline map"))
        .param(PortSpec::optional("future_year", Type::Integer, "of the future map"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        carbon(args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

fn carbon(args: &Args) -> Result<Outputs> {
    let lu = LandUse::new(args.map("lulc")?, "the land use map")?;
    let (codes, cols) = lookup(table_of(args, "pools")?, &POOLS, "the carbon pools", "lucode")?;
    let pools32: Vec<f32> = (0..codes.len()).flat_map(|i| cols.iter().map(move |c| c[i] as f32)).collect();
    let (dx, dy) = lu.pixel()?;
    let area_ha = dx * dy / 10000.0;
    let grid = &lu.map.grid;
    let n = lu.codes.len();
    let maps =
        |l: &LandUse| -> Result<Vec<f32>> { Ok(ffi::serv_carbon_pools(&l.classes(&codes)?, &pools32, threads())?) };
    let stack = |all: &[f32]| -> Value {
        Value::Stack(Stack {
            grid: grid.clone(),
            keys: POOLS.iter().map(|p| Key::Name(p.to_string())).collect(),
            layers: (0..4).map(|p| Arc::new(all[p * n..(p + 1) * n].iter().map(|&x| f64::from(x)).collect())).collect(),
        })
    };
    let base = maps(&lu)?;
    let storage = &base[4 * n..];
    let mut out = Outputs::new();
    out.insert("storage".into(), f32_map(grid, storage));
    out.insert("pools".into(), stack(&base));
    let mut totals = vec![("baseline", ffi::serv_total(storage, threads()) * area_ha)];
    let price: Option<f64> = args.opt_param("price")?;
    match args.opt_map("future")? {
        None if price.is_some() => return Err(Error::msg("the valuation values a change: give the future map")),
        None => {}
        Some(f) => {
            let fu = LandUse::new(f, "the future land use map")?;
            lu.same_grid(&fu)?;
            let fut = maps(&fu)?;
            let fstorage = &fut[4 * n..];
            let change: Vec<f32> = fstorage.iter().zip(storage).map(|(f, b)| f - b).collect();
            out.insert("storage_future".into(), f32_map(grid, fstorage));
            out.insert("pools_future".into(), stack(&fut));
            totals.push(("alternate", ffi::serv_total(fstorage, threads()) * area_ha));
            totals.push(("change", ffi::serv_total(&change, threads()) * area_ha));
            if let Some(price) = price {
                let (Some(d), Some(y0), Some(y1)) =
                    (args.opt_param::<f64>("discount_rate")?, args.opt_param("year")?, args.opt_param("future_year")?)
                else {
                    return Err(Error::msg("the valuation needs price, discount_rate, year and future_year"));
                };
                let k = valuation_constant(price, d, args.param("rate_change", 0.0)?, y0, y1)? as f32;
                let npv: Vec<f32> = change.iter().map(|c| c * k).collect();
                totals.push(("npv", ffi::serv_total(&npv, threads()) * area_ha));
                out.insert("npv".into(), f32_map(grid, &npv));
            }
            out.insert("change".into(), f32_map(grid, &change));
        }
    }
    let mut t = Table::new(totals.iter().map(|(k, _)| k.to_string()).collect());
    t.rows.push(totals.iter().map(|(_, v)| *v).collect());
    out.insert("totals".into(), Value::Table(t));
    Ok(out)
}

// -- habitat quality --------------------------------------------------------------------------------------------

struct Threat {
    name: String,
    max_dist: f64,
    weight: f64,
    exponential: bool,
}

/// A table of text (the first row its header), the separator found by itself.
fn text_table(text: &str) -> Vec<Vec<String>> {
    let first = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t'].into_iter().max_by_key(|c| first.matches(*c).count()).unwrap_or(',');
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(sep).map(|v| v.trim().trim_matches('"').to_string()).collect())
        .collect()
}

/// The threats: a list of {threat, max_dist, weight, decay}, or a file of them (InVEST's threats table).
fn threats_of(args: &Args) -> Result<Vec<Threat>> {
    let v =
        args.params.get("threats").ok_or_else(|| Error::msg("give the threats (threat, max_dist, weight, decay)"))?;
    let rows: Vec<serde_json::Map<String, serde_json::Value>> = match v {
        serde_json::Value::String(path) => {
            let text = std::fs::read_to_string(path.trim()).map_err(|e| Error::msg(format!("{path}: {e}")))?;
            let t = text_table(&text);
            let head: Vec<String> = t.first().map(|h| h.iter().map(|c| c.to_lowercase()).collect()).unwrap_or_default();
            t.iter()
                .skip(1)
                .map(|r| head.iter().cloned().zip(r.iter().map(|x| serde_json::Value::String(x.clone()))).collect())
                .collect()
        }
        serde_json::Value::Array(a) => a
            .iter()
            .map(|r| {
                r.as_object()
                    .map(|o| o.iter().map(|(k, v)| (k.trim().to_lowercase(), v.clone())).collect())
                    .ok_or_else(|| Error::msg("each threat is {threat, max_dist, weight, decay}"))
            })
            .collect::<Result<_>>()?,
        _ => return Err(Error::msg("the threats are a list of {threat, max_dist, weight, decay} or a file")),
    };
    let text = |r: &serde_json::Map<String, serde_json::Value>, k: &str| -> Result<String> {
        match r.get(k) {
            Some(serde_json::Value::String(s)) => Ok(s.trim().to_string()),
            Some(v) if !v.is_null() => Ok(v.to_string()),
            _ => Err(Error::msg(format!("the threats table lacks the column {k:?}"))),
        }
    };
    let num = |r: &serde_json::Map<String, serde_json::Value>, k: &str| -> Result<f64> {
        match r.get(k) {
            Some(serde_json::Value::Number(n)) => Ok(n.as_f64().unwrap()),
            Some(serde_json::Value::String(s)) => {
                s.trim().parse().map_err(|_| Error::msg(format!("the threats' {k} {s:?} is not a number")))
            }
            _ => Err(Error::msg(format!("the threats table lacks the column {k:?}"))),
        }
    };
    let mut out = Vec::new();
    for r in &rows {
        let decay = text(r, "decay")?.to_lowercase();
        if decay != "linear" && decay != "exponential" {
            return Err(Error::msg("decay is 'linear' or 'exponential'"));
        }
        out.push(Threat {
            name: text(r, "threat")?,
            max_dist: num(r, "max_dist")?,
            weight: num(r, "weight")?,
            exponential: decay == "exponential",
        });
    }
    let mut names: Vec<&str> = out.iter().map(|t| t.name.as_str()).collect();
    names.sort_unstable();
    if names.windows(2).any(|w| w[0] == w[1]) {
        return Err(Error::msg("the threats table lists a threat more than once"));
    }
    if out.iter().any(|t| t.max_dist <= 0.0) {
        return Err(Error::msg("max_dist must be positive"));
    }
    if out.iter().any(|t| !(0.0..=1.0).contains(&t.weight)) {
        return Err(Error::msg("the weights are ratios from 0 to 1"));
    }
    Ok(out)
}

/// The access to threats, float32 (1 where not given): a map, or polygons with an `access` attribute.
fn access_of(args: &Args, lu: &LandUse) -> Result<Vec<f32>> {
    match args.get("access") {
        None => Ok(Vec::new()),
        Some(Value::Vector(v)) => {
            let v = crate::vector::on_grid(v, &lu.map.grid)?;
            let values = attribute(&v, "access", "access polygons")?;
            let mut out = vec![f64::NAN; lu.map.data.len()];
            crate::vector::burn(&mut out, &lu.map.grid, &v, &values, false, false)?;
            Ok(out.iter().map(|&a| if a.is_nan() { 1.0 } else { a as f32 }).collect())
        }
        Some(_) => {
            let m = args.map("access")?;
            if m.shape() != lu.map.shape() {
                return Err(Error::msg(format!(
                    "the access raster has {:?} cells, the land use map {:?}",
                    m.shape(),
                    lu.map.shape()
                )));
            }
            Ok(m.data.iter().map(|&a| a as f32).collect())
        }
    }
}

/// The threat maps of a scenario by name, as regions (1 where the threat is: a value other than 0).
fn regions(stack: &Stack, threats: &[Threat], lu: &LandUse, scenario: &str) -> Result<Vec<Vec<u8>>> {
    if stack.grid.width != lu.map.grid.width || stack.grid.height != lu.map.grid.height {
        return Err(Error::msg(format!("the {scenario} threat maps differ in shape from the land use map")));
    }
    threats
        .iter()
        .map(|t| {
            let v = stack
                .by_name(&t.name)
                .ok_or_else(|| Error::msg(format!("no {scenario} map of the threat {:?}", t.name)))?;
            if v.iter().any(|x| !x.is_nan() && !(0.0..=1.0).contains(x)) {
                return Err(Error::msg(format!("the {scenario} map of {:?} must hold values from 0 to 1", t.name)));
            }
            Ok(v.iter().map(|x| u8::from(!x.is_nan() && *x != 0.0)).collect())
        })
        .collect()
}

struct Habitat;

impl Operator for Habitat {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "services.habitat_quality",
            "Habitat degradation, quality and rarity: InVEST's habitat quality model (natcap.invest 3.20) bit for \
             bit, through landschaft. Each threat degrades the habitat around it, decaying linearly or \
             exponentially with the distance to its nearest cell up to its maximum distance, weighed by its \
             weight, the sensitivity of the cell's class and the access; quality h (1 - D^2.5 / (D^2.5 + k^2.5)). \
             With a base map, the rarity of each class.",
            Access::Global,
        )
        .input(PortSpec::new("lulc", Type::Map, "the current land use (class codes; projected, metres)"))
        .input(PortSpec::new(
            "sensitivity",
            Type::Table,
            "by class: lucode, habitat (0 to 1) and a column per threat (its sensitivity, 0 to 1)",
        ))
        .input(PortSpec::new("threat_maps", Type::Stack, "the current map of each threat, by its name (0 to 1)"))
        .input(PortSpec::optional("future", Type::Map, "a future land use"))
        .input(PortSpec::optional("future_threat_maps", Type::Stack, "the future's threat maps"))
        .input(PortSpec::optional("base", Type::Map, "a base land use, for the rarity"))
        .input(PortSpec::optional("base_threat_maps", Type::Stack, "the base's threat maps (optional)"))
        .input(PortSpec::optional(
            "access",
            Type::Any,
            "accessibility to threats, 0 to 1: a raster, or polygons with an access attribute (1 outside)",
        ))
        .output(PortSpec::new("quality", Type::Map, "the current habitat quality"))
        .output(PortSpec::new("degradation", Type::Map, "the current total degradation"))
        .output(PortSpec::new("threats", Type::Stack, "the current degradation of each threat"))
        .output(PortSpec::new("quality_future", Type::Map, "with a future"))
        .output(PortSpec::new("degradation_future", Type::Map, "with a future"))
        .output(PortSpec::new("quality_base", Type::Map, "with the base's threat maps"))
        .output(PortSpec::new("degradation_base", Type::Map, "with the base's threat maps"))
        .output(PortSpec::new("rarity", Type::Map, "the rarity of the current classes (with a base)"))
        .output(PortSpec::new("rarity_future", Type::Map, "of the future classes (with a base and a future)"))
        .output(PortSpec::new("rarity_table", Type::Table, "scenario (0 current, 1 future), lucode, rarity"))
        .param(PortSpec::new(
            "threats",
            Type::Any,
            "a list of {threat, max_dist (metres), weight (0 to 1), decay (linear or exponential)}, or InVEST's \
             threats table (a file)",
        ))
        .param(PortSpec::new("half_saturation", Type::Number, "k, often half the highest degradation"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        habitat(args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

fn habitat(args: &Args) -> Result<Outputs> {
    let lu = LandUse::new(args.map("lulc")?, "the land use map")?;
    let threats = threats_of(args)?;
    let weight_sum = crate::change::numpy_sum(&threats.iter().map(|t| t.weight).collect::<Vec<_>>());
    if weight_sum.is_nan() || weight_sum <= 0.0 {
        return Err(Error::msg("the weights sum to zero"));
    }
    let k: f64 = args.opt_param("half_saturation")?.ok_or_else(|| Error::msg("give the half_saturation"))?;
    if k.is_nan() || k <= 0.0 {
        return Err(Error::msg("half_saturation must be positive"));
    }
    let names: Vec<&str> = threats.iter().map(|t| t.name.as_str()).collect();
    let columns: Vec<&str> = std::iter::once("habitat").chain(names.iter().copied()).collect();
    let (codes, table) = lookup(table_of(args, "sensitivity")?, &columns, "the sensitivity table", "lucode")?;
    let to32 = |v: &[f64]| -> Vec<f32> { v.iter().map(|&x| x as f32).collect() };
    let habitat = to32(&table[0]);
    let px = lu.pixel()?.0;
    let ksq = k.powf(2.5);
    let access = access_of(args, &lu)?;
    let grid = lu.map.grid.clone();
    let mut out = Outputs::new();

    let future = args.opt_map("future")?.map(|m| LandUse::new(m, "the future land use map")).transpose()?;
    if let Some(f) = &future {
        lu.same_grid(f)?;
        if args.opt_stack("future_threat_maps")?.is_none() {
            return Err(Error::msg("give the threat maps of the future (future_threat_maps)"));
        }
    }
    let base = args.opt_map("base")?.map(|m| LandUse::new(m, "the base land use map")).transpose()?;
    if let Some(b) = &base {
        lu.same_grid(b)?;
    } else if args.opt_stack("base_threat_maps")?.is_some() {
        return Err(Error::msg("base_threat_maps without the base map"));
    }
    let mut lands: Vec<(&str, &LandUse, Option<&Stack>)> = vec![("current", &lu, Some(args.stack("threat_maps")?))];
    if let Some(f) = &future {
        lands.push(("future", f, args.opt_stack("future_threat_maps")?));
    }
    if let Some(b) = &base {
        lands.push(("base", b, args.opt_stack("base_threat_maps")?));
    }
    for (sc, land, maps) in &lands {
        let Some(maps) = maps else { continue };
        let cls = land.classes(&codes)?;
        let regions = regions(maps, &threats, land, sc)?;
        let n = cls.len();
        let mut total = vec![0f32; n];
        let mut per = Vec::new();
        for (t, region) in threats.iter().zip(&regions) {
            let dist = ffi::serv_distance_transform(region, grid.height as i32, grid.width as i32, threads())?;
            let w = t.weight / weight_sum;
            let max_px = t.max_dist / px;
            let given: Vec<f32> = if t.exponential {
                // InVEST's numpy expression, numpy's float32 exp
                let m = max_px as f32;
                dist.iter().map(|&d| if d > m { 0.0 } else { npmath::exp((-d * 2.99) / m) }).collect()
            } else {
                Vec::new()
            };
            let sens = to32(&table[1 + per.len()]);
            per.push(ffi::serv_threat_degradation(
                &dist,
                &given,
                &cls,
                &sens,
                &access,
                max_px,
                t.exponential,
                w,
                &mut total,
                threads(),
            )?);
        }
        let quality = ffi::serv_quality(&total, &cls, &habitat, ksq, threads())?;
        let suffix = if *sc == "current" { String::new() } else { format!("_{sc}") };
        out.insert(format!("quality{suffix}"), f32_map(&grid, &quality));
        out.insert(format!("degradation{suffix}"), f32_map(&grid, &total));
        if *sc == "current" {
            out.insert(
                "threats".into(),
                Value::Stack(Stack {
                    grid: grid.clone(),
                    keys: threats.iter().map(|t| Key::Name(t.name.clone())).collect(),
                    layers: per.iter().map(|d| Arc::new(d.iter().map(|&x| f64::from(x)).collect())).collect(),
                }),
            );
        }
    }
    if let Some(b) = &base {
        let counts = |land: &LandUse, keep: &dyn Fn(usize) -> bool| -> std::collections::BTreeMap<i64, i64> {
            let mut c = std::collections::BTreeMap::new();
            for i in 0..land.codes.len() {
                if keep(i) {
                    *c.entry(land.codes[i]).or_insert(0) += 1;
                }
            }
            c
        };
        let base_counts = counts(b, &|i| b.valid[i] == 1);
        let area = px * lu.pixel()?.1;
        let mut t = Table::new(vec!["scenario".into(), "lucode".into(), "rarity".into()]);
        for (s, land) in [(0.0, Some(&lu)), (1.0, future.as_ref())] {
            let Some(land) = land else { continue };
            let keep = |i: usize| land.valid[i] == 1 && b.valid[i] == 1;
            let index: std::collections::BTreeMap<i64, f64> = counts(land, &keep)
                .into_iter()
                .map(|(code, n)| match base_counts.get(&code) {
                    Some(&nb) => {
                        let num = n as f64 * area;
                        let den = nb as f64 * area;
                        (code, 1.0 - (num / (den + num)))
                    }
                    None => (code, 0.0),
                })
                .collect();
            let r: Vec<f64> = (0..land.codes.len())
                .map(|i| if keep(i) { f64::from(index[&land.codes[i]] as f32) } else { f64::NAN })
                .collect();
            out.insert(if s == 0.0 { "rarity".into() } else { "rarity_future".into() }, f64_map(&grid, r));
            t.rows.extend(index.iter().map(|(c, v)| vec![s, *c as f64, *v]));
        }
        out.insert("rarity_table".into(), Value::Table(t));
    }
    Ok(out)
}

// -- annual water yield -----------------------------------------------------------------------------------------

struct Water;

impl Operator for Water {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "services.water_yield",
            "Annual water yield, consumption and hydropower: InVEST's annual water yield model (natcap.invest \
             3.20) bit for bit, through landschaft. The water balance of each cell on the Budyko curve (Fu and \
             Zhang): AET / P from the potential evapotranspiration Kc ET0, the plant available water and the \
             seasonality Z; the yield P - AET. With watersheds, their means and volumes; with a demand table, \
             the consumption and the realized supply; with a valuation table, the hydropower energy and value.",
            Access::Global,
        )
        .input(PortSpec::new("lulc", Type::Map, "the land use (class codes; projected, metres)"))
        .input(PortSpec::new("biophysical", Type::Table, "by class: lucode, lulc_veg (1 or 0), root_depth (mm), kc"))
        .input(PortSpec::new("precipitation", Type::Map, "annual precipitation, mm"))
        .input(PortSpec::new("eto", Type::Map, "annual reference evapotranspiration, mm"))
        .input(PortSpec::new("depth", Type::Map, "depth to the root restricting layer, mm"))
        .input(PortSpec::new("pawc", Type::Map, "plant available water content, 0 to 1"))
        .input(PortSpec::optional("watersheds", Type::Vector, "polygons with a ws_id"))
        .input(PortSpec::optional("subwatersheds", Type::Vector, "polygons with a subws_id"))
        .input(PortSpec::optional("demand", Type::Table, "by class: lucode, demand (m³ per year and cell)"))
        .input(PortSpec::optional(
            "valuation",
            Type::Table,
            "by watershed: ws_id, efficiency, fraction, height, kw_price, cost, time_span, discount",
        ))
        .output(PortSpec::new("wyield", Type::Map, "water yield, mm"))
        .output(PortSpec::new("aet", Type::Map, "actual evapotranspiration, mm"))
        .output(PortSpec::new("pet", Type::Map, "potential evapotranspiration, mm"))
        .output(PortSpec::new("fractp", Type::Map, "AET / P"))
        .output(PortSpec::new("demand_map", Type::Map, "consumptive use per cell, m³ / year (with demand)"))
        .output(PortSpec::new("watershed_table", Type::Table, "by watershed: means, volumes, supply, hydropower"))
        .output(PortSpec::new("subwatershed_table", Type::Table, "by subwatershed"))
        .param(PortSpec::new("seasonality", Type::Number, "Z, about 1 to 30"))
        .param(
            PortSpec::optional(
                "precision",
                Type::Text,
                "InVEST's arithmetic: float32 (its float32 rasters), float64, or auto (default: float64 when a \
                 raster has a value float32 cannot hold)",
            )
            .choices(["auto", "float32", "float64"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        water(args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

/// Whether numpy would compute with a raster in float64: by the setting, or (auto) when it has a value float32
/// cannot hold.
fn wide(precision: &str, v: &[f64]) -> bool {
    match precision {
        "float64" => true,
        "float32" => false,
        _ => v.iter().any(|x| !x.is_nan() && f64::from(*x as f32) != *x),
    }
}

fn water(args: &Args) -> Result<Outputs> {
    let lu = LandUse::new(args.map("lulc")?, "the land use map")?;
    let (codes, bio) =
        lookup(table_of(args, "biophysical")?, &["lulc_veg", "root_depth", "kc"], "the biophysical table", "lucode")?;
    if bio[0].iter().any(|v| *v != 0.0 && *v != 1.0) {
        return Err(Error::msg("lulc_veg is 1 (vegetation) or 0"));
    }
    let kc: Vec<f32> = bio[2].iter().map(|&x| x as f32).collect();
    let root: Vec<f32> = bio[0].iter().zip(&bio[1]).map(|(v, r)| if *v == 1.0 { *r as f32 } else { 1.0 }).collect();
    let veg: Vec<f32> = bio[0].iter().map(|&x| x as f32).collect();
    let z: f64 = args.opt_param("seasonality")?.ok_or_else(|| Error::msg("give the seasonality (Z)"))?;
    if z.is_nan() || z <= 0.0 {
        return Err(Error::msg("seasonality (Z) must be positive"));
    }
    let precision: String = args.param("precision", "auto".to_string())?;
    let cls = lu.classes(&codes)?;
    let raster = |port: &str, what: &str| -> Result<&[f64]> {
        let m = args.map(port)?;
        if m.shape() != lu.map.shape() {
            return Err(Error::msg(format!("{what} has {:?} cells, the land use map {:?}", m.shape(), lu.map.shape())));
        }
        Ok(&m.data)
    };
    let e = raster("eto", "the reference evapotranspiration")?;
    let p = raster("precipitation", "the precipitation")?;
    let s = raster("depth", "the soil depth")?;
    let w = raster("pawc", "the plant available water content")?;
    let any_wide = [e, p, s, w].iter().any(|v| wide(&precision, v));
    let fractp = ffi::serv_water_fraction(&cls, &kc, &root, &veg, e, p, s, w, z, any_wide, threads())?;
    let n = cls.len();
    let yields = ffi::serv_water_yield(&fractp, p, wide(&precision, p), threads())?;
    let (wyield, aet) = yields.split_at(n);
    let pet = ffi::serv_potential_et(&cls, &kc, e, wide(&precision, e), threads())?;
    let grid = lu.map.grid.clone();
    let mut out = Outputs::new();
    out.insert("wyield".into(), f64_map(&grid, wyield.to_vec()));
    out.insert("aet".into(), f64_map(&grid, aet.to_vec()));
    out.insert("pet".into(), f64_map(&grid, pet.clone()));
    out.insert("fractp".into(), f32_map(&grid, &fractp));
    let mut demand = None;
    if let Some(t) = opt_table(args, "demand")? {
        let (dcodes, dt) = lookup(t, &["demand"], "the demand table", "lucode")?;
        let dcls = lu.classes(&dcodes)?;
        let d: Vec<f64> =
            dcls.iter().map(|&c| if c >= 0 { f64::from(dt[0][c as usize] as f32) } else { f64::NAN }).collect();
        out.insert("demand_map".into(), f64_map(&grid, d.clone()));
        demand = Some(d);
    }
    let valuation = opt_table(args, "valuation")?;
    if valuation.is_some() && demand.is_none() {
        return Err(Error::msg("the valuation values the realized supply: give the demand table too"));
    }
    let sheds = args.get("watersheds").map(|_| crate::vector::vector_of(args, "watersheds")).transpose()?;
    if sheds.is_none() && (args.get("subwatersheds").is_some() || valuation.is_some()) {
        return Err(Error::msg("give the watersheds"));
    }
    if let Some(v) = sheds {
        let (dx, dy) = lu.pixel()?;
        // the precipitation's own cells, rain or not
        let mut maps: Vec<(&str, &[f64])> =
            vec![("precip_mn", p), ("PET_mn", &pet), ("AET_mn", aet), ("wyield_mn", wyield)];
        if let Some(d) = &demand {
            maps.push(("demand", d));
        }
        out.insert("watershed_table".into(), Value::Table(shed_table(v, "ws_id", &maps, &grid, dx * dy, valuation)?));
        if args.get("subwatersheds").is_some() {
            let sub = crate::vector::vector_of(args, "subwatersheds")?;
            out.insert(
                "subwatershed_table".into(),
                Value::Table(shed_table(sub, "subws_id", &maps, &grid, dx * dy, None)?),
            );
        }
    }
    Ok(out)
}

/// InVEST's per-watershed table: means of the maps, volumes, realized supply, hydropower.
fn shed_table(
    v: &Arc<Vector>,
    id: &str,
    maps: &[(&str, &[f64])],
    grid: &Grid,
    cell_area: f64,
    valuation: Option<&Table>,
) -> Result<Table> {
    let what = if id == "subws_id" { "subwatersheds" } else { "watersheds" };
    let v = polygons(v, grid)?;
    let ids = attribute(&v, id, what)?;
    let stats = zonal(&v, grid, &maps.iter().map(|m| m.1).collect::<Vec<_>>(), &[2, 0])?;
    let nm = maps.len();
    let mut cols: Vec<String> = vec![id.to_string()];
    let mut rows: Vec<Vec<f64>> = ids.iter().map(|&i| vec![i]).collect();
    let has_demand = maps.iter().any(|m| m.0 == "demand");
    for (z, row) in rows.iter_mut().enumerate() {
        let geom_area = area(&v.geometries[z]);
        let mut wyield_mn = f64::NAN;
        let mut wyield_vol = f64::NAN;
        let (mut consum_vol, mut consum_mn) = (f64::NAN, f64::NAN);
        for (k, (name, _)) in maps.iter().enumerate() {
            let s = stats[(z * nm + k) * 2];
            let c = stats[(z * nm + k) * 2 + 1];
            let mean = if c > 0.0 { s / c } else { f64::NAN };
            if *name == "demand" {
                consum_vol = if c > 0.0 { s } else { f64::NAN };
                consum_mn = mean;
            } else {
                row.push(mean);
                if *name == "wyield_mn" {
                    wyield_mn = mean;
                    wyield_vol = mean * geom_area / 1000.0;
                    row.push(wyield_vol);
                }
            }
        }
        if has_demand {
            let rsupply = wyield_vol - consum_vol;
            row.extend([consum_vol, consum_mn, rsupply, wyield_mn * cell_area / 1000.0 - consum_mn]);
        }
    }
    for (name, _) in maps {
        if *name != "demand" {
            cols.push(name.to_string());
            if *name == "wyield_mn" {
                cols.push("wyield_vol".into());
            }
        }
    }
    if has_demand {
        cols.extend(["consum_vol", "consum_mn", "rsupply_vl", "rsupply_mn"].map(String::from));
    }
    if let Some(val) = valuation {
        let need = ["efficiency", "fraction", "height", "kw_price", "cost", "time_span", "discount"];
        let (wids, vcols) = lookup(val, &need, "the valuation table", "ws_id")?;
        let rsupply_col = cols.iter().position(|c| c == "rsupply_vl").unwrap();
        for row in &mut rows {
            let ws = row[0] as i64;
            let j = wids
                .binary_search(&ws)
                .map_err(|_| Error::msg(format!("the valuation table lacks the watershed {ws}")))?;
            let rs = row[rsupply_col];
            if !rs.is_finite() {
                row.extend([f64::NAN, f64::NAN]);
                continue;
            }
            let g = |k: usize| vcols[k][j];
            let e = g(0) * g(1) * g(2) * rs * KWH_PER_M3_M;
            let ratio = 1.0 / (1.0 + g(6) / 100.0);
            let years = g(5);
            let dsum = if ratio != 1.0 { (1.0 - ratio.powf(years)) / (1.0 - ratio) } else { years };
            row.extend([e, (g(3) * e - g(4)) * dsum]);
        }
        cols.extend(["hp_energy", "hp_val"].map(String::from));
    }
    let mut t = Table::new(cols);
    t.rows = rows;
    Ok(t)
}

// -- sediment delivery ratio ------------------------------------------------------------------------------------

/// The routing of the SDR chain: pygeoprocessing's MFD (packed) or D8 directions and their accumulation.
struct Routing {
    mfd: Vec<i32>,
    d8: Vec<u8>,
    acc: Vec<f64>,
    ny: i32,
    nx: i32,
}

impl Routing {
    fn new(filled: &[f64], ny: i32, nx: i32, method: &str) -> Result<Routing> {
        Ok(if method == "mfd" {
            let d = ffi::hydro_mfd_directions(filled, ny, nx, MFD_BLOCK)?;
            let acc = ffi::hydro_mfd_packed_accumulation(&d, &[], ny, nx)?;
            Routing { mfd: d, d8: Vec::new(), acc, ny, nx }
        } else {
            let d = ffi::hydro_pgp_d8(filled, ny, nx, MFD_BLOCK)?;
            let acc = ffi::hydro_pgp_d8_accumulation(&d, &[], ny, nx)?;
            Routing { mfd: Vec::new(), d8: d, acc, ny, nx }
        })
    }

    fn accumulate(&self, w: &[f64]) -> Result<Vec<f64>> {
        Ok(if self.d8.is_empty() {
            ffi::hydro_mfd_packed_accumulation(&self.mfd, w, self.ny, self.nx)?
        } else {
            ffi::hydro_pgp_d8_accumulation(&self.d8, w, self.ny, self.nx)?
        })
    }

    fn distance(&self, channel: &[u8], w: &[f64]) -> Result<Vec<f64>> {
        Ok(if self.d8.is_empty() {
            ffi::hydro_mfd_distance_to_channel(&self.mfd, channel, w, self.ny, self.nx)?
        } else {
            ffi::hydro_pgp_d8_distance_to_channel(&self.d8, channel, w, self.ny, self.nx)?
        })
    }

    fn missing(&self, i: usize) -> bool {
        if self.d8.is_empty() { self.mfd[i] == 0 } else { self.d8[i] == 128 }
    }

    /// The streams (1, 0, 255 missing): traced as InVEST from the mfd accumulation, or above the threshold.
    fn streams(&self, threshold: f64) -> Result<Vec<u8>> {
        Ok(if self.d8.is_empty() {
            ffi::hydro_mfd_streams(&self.acc, &self.mfd, self.ny, self.nx, threshold, 0.7, MFD_BLOCK)?
        } else {
            self.acc.iter().map(|a| if a.is_nan() { 255 } else { u8::from(*a > threshold) }).collect()
        })
    }
}

/// InVEST's threshold_slope_op: m/m from 0.005 to 1, float32.
fn thresholds(slope: &[f32]) -> Vec<f32> {
    slope
        .iter()
        .map(|&s| {
            if s.is_nan() {
                return f32::NAN;
            }
            (s / 100.0).clamp(0.005, 1.0)
        })
        .collect()
}

/// The weights from 0.001 (InVEST's), float32.
fn weight_floor(w: &[f32]) -> Vec<f32> {
    w.iter()
        .map(|&a| {
            if a.is_nan() {
                f32::NAN
            } else if a < 0.001 {
                0.001
            } else {
                a
            }
        })
        .collect()
}

/// W bar, S bar, D_up, the inverse WS weight, D_dn and the IC (float32), as InVEST.
struct Connectivity {
    d_dn: Vec<f64>,
    ic: Vec<f32>,
}

fn connectivity(w_thr: &[f32], s_thr: &[f32], r: &Routing, channel: &[u8], cell_area: f64) -> Result<Connectivity> {
    let wide = |v: &[f32]| -> Vec<f64> { v.iter().map(|&x| f64::from(x)).collect() };
    let w_acc = r.accumulate(&wide(w_thr))?;
    let s_acc = r.accumulate(&wide(s_thr))?;
    let n = r.acc.len();
    let mut d_up = vec![f64::NAN; n];
    for i in 0..n {
        let f = r.acc[i];
        let (wa, sa) = (w_acc[i], s_acc[i]);
        if wa.is_nan() || sa.is_nan() || f.is_nan() {
            continue;
        }
        let (wb, sb) = (wa / f, sa / f);
        if wb.is_nan() || sb.is_nan() {
            continue;
        }
        d_up[i] = wb * sb * (f * cell_area).sqrt();
    }
    let ws_inv: Vec<f64> = w_thr
        .iter()
        .zip(s_thr)
        .map(|(w, s)| if w.is_nan() || s.is_nan() { f64::NAN } else { f64::from(1.0 / (w * s)) })
        .collect();
    let d_dn = r.distance(channel, &ws_inv)?;
    let ic = d_up
        .iter()
        .zip(&d_dn)
        .map(
            |(u, d)| {
                if u.is_nan() || d.is_nan() || *d == 0.0 || *u == 0.0 { f32::NAN } else { (u / d).log10() as f32 }
            },
        )
        .collect();
    Ok(Connectivity { d_dn, ic })
}

/// InVEST's ls_factor_function, as it is written (float32 slope, float64 accumulation).
fn ls_factor(p: f32, acc: f64, cell_size: f64, cell_area: f64, l_max: f64) -> f64 {
    let ca = ((acc - 1.0) * cell_area).sqrt();
    let rad = (p / 100.0).atan();
    let (sin, cos) = (npmath::sin(rad), npmath::cos(rad));
    let aspect_length = sin.abs() + cos.abs();
    let slope_factor = if p < 9.0 { 10.8 * sin + 0.03 } else { 16.8 * sin - 0.5 };
    let m: f32 = if f64::from(p) > 9.0 {
        let beta = (sin / 0.0896) / (3.0 * npmath::powf(sin, 0.8) + 0.56);
        beta / (1.0 + beta)
    } else {
        let bins = [1.0, 3.5, 5.0, 9.0];
        let k = bins.iter().filter(|&&b| b < f64::from(p)).count();
        [0.2, 0.3, 0.4, 0.5][k] as f32
    };
    let m1 = f64::from(m + 1.0);
    let num = (ca + cell_area).powf(m1) - ca.powf(m1);
    let den = ((cell_size as f32).powf(m + 2.0) * aspect_length.powf(m)) * 22.13f32.powf(m);
    let lf = num / f64::from(den);
    (if lf > l_max { l_max } else { lf }) * f64::from(slope_factor)
}

struct Sdr;

impl Operator for Sdr {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "services.sdr",
            "Sediment export, deposition and retention: InVEST's sediment delivery ratio model (natcap.invest \
             3.20), through landschaft. The soil loss of the RUSLE (R K LS C P, the LS of Desmet & Govers from the \
             flow accumulation) and the share that reaches a stream, SDR = SDR_max / (1 + exp((IC_0 - IC) / k)) of \
             the index of connectivity (Borselli et al. 2008); the DEM filled and routed by pygeoprocessing's \
             algorithms, as InVEST.",
            Access::Global,
        )
        .input(PortSpec::new("dem", Type::Map, "elevations (projected, metres)"))
        .input(PortSpec::new("erosivity", Type::Map, "rainfall erosivity R (MJ mm / ha h yr)"))
        .input(PortSpec::new("erodibility", Type::Map, "soil erodibility K (t ha h / ha MJ mm)"))
        .input(PortSpec::new("lulc", Type::Map, "land use (class codes)"))
        .input(PortSpec::new("biophysical", Type::Table, "by class: lucode, usle_c, usle_p"))
        .input(PortSpec::new("watersheds", Type::Vector, "the watershed polygons (the model runs inside them)"))
        .input(PortSpec::optional("drainage", Type::Map, "other cells that drain to the streams (1)"))
        .output(PortSpec::new("usle", Type::Map, "soil loss, t / ha / yr"))
        .output(PortSpec::new("sed_export", Type::Map, "the soil loss that reaches the streams"))
        .output(PortSpec::new("sed_deposition", Type::Map, "sediment from upslope the cell traps"))
        .output(PortSpec::new("avoided_export", Type::Map, "total retention"))
        .output(PortSpec::new("avoided_erosion", Type::Map, "RKLS - USLE"))
        .output(PortSpec::new("rkls", Type::Map, "the soil loss of bare soil"))
        .output(PortSpec::new("sdr", Type::Map, "the sediment delivery ratio"))
        .output(PortSpec::new("ic", Type::Map, "the index of connectivity"))
        .output(PortSpec::new("stream", Type::Map, "the streams (1)"))
        .output(PortSpec::new("drains_to_stream", Type::Map, "1 where the flow reaches a stream"))
        .output(PortSpec::new(
            "watershed_table",
            Type::Table,
            "by watershed: usle_tot, sed_export, sed_dep, avoid_exp, avoid_eros (t / yr)",
        ))
        .param(PortSpec::new("threshold", Type::Number, "the flow accumulation (cells) of a stream"))
        .param(PortSpec::optional("k", Type::Number, "Borselli's k (2)"))
        .param(PortSpec::optional("ic_0", Type::Number, "Borselli's IC_0 (0.5)"))
        .param(PortSpec::optional("sdr_max", Type::Number, "the largest SDR (0.8)"))
        .param(PortSpec::optional("l_max", Type::Number, "the largest L factor (122)"))
        .param(PortSpec::optional("method", Type::Text, "the routing: mfd (default) or d8").choices(["mfd", "d8"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        sdr(args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

fn sdr(args: &Args) -> Result<Outputs> {
    let method: String = args.param("method", "mfd".to_string())?.to_lowercase();
    if method != "mfd" && method != "d8" {
        return Err(Error::msg("method is 'mfd' or 'd8'"));
    }
    let threshold: f64 = args.opt_param("threshold")?.ok_or_else(|| Error::msg("give the threshold"))?;
    if threshold.is_nan() || threshold <= 0.0 {
        return Err(Error::msg("threshold must be positive"));
    }
    let (k, ic_0, sdr_max, l_max): (f64, f64, f64, f64) =
        (args.param("k", 2.0)?, args.param("ic_0", 0.5)?, args.param("sdr_max", 0.8)?, args.param("l_max", 122.0)?);
    let lu = LandUse::new(args.map("lulc")?, "the land use map")?;
    let grid = lu.map.grid.clone();
    let (ny, nx) = (grid.height as i32, grid.width as i32);
    let n = grid.cells();
    let (codes, bio) =
        lookup(table_of(args, "biophysical")?, &["usle_c", "usle_p"], "the biophysical table", "lucode")?;
    let mut ports = vec!["dem", "erosivity", "erodibility"];
    if args.get("drainage").is_some() {
        ports.push("drainage");
    }
    let mut values = Vec::new();
    for p in &ports {
        let m = args.map(p)?;
        if m.shape() != lu.map.shape() {
            return Err(Error::msg(format!(
                "the {p} raster has {:?} cells, the land use map {:?}",
                m.shape(),
                lu.map.shape()
            )));
        }
        values.push(&m.data);
    }
    let sheds = polygons(crate::vector::vector_of(args, "watersheds")?, &grid)?;
    let within = inside(&sheds, &grid)?;
    let valid: Vec<bool> =
        (0..n).map(|i| within[i] && lu.valid[i] == 1 && values.iter().all(|v| !v[i].is_nan())).collect();
    let masked: Vec<Vec<f64>> =
        values.iter().map(|v| (0..n).map(|i| if valid[i] { v[i] } else { f64::NAN }).collect()).collect();
    let t = grid.transform;
    let (xcell, ycell) = (t[1], t[5]);
    lu.pixel()?;
    let (cell_size, cell_area) = (xcell.abs(), xcell.abs().powi(2));
    let filled = ffi::hydro_fill(&masked[0], ny, nx, false)?;
    let slope = ffi::serv_pgp_slope(&filled, ny, nx, xcell, ycell, threads())?;
    let slope_thr = thresholds(&slope);
    let r = Routing::new(&filled, ny, nx, &method)?;
    let stream = r.streams(threshold)?;
    let ls: Vec<f64> = (0..n)
        .map(|i| {
            if slope[i].is_nan() || r.acc[i].is_nan() {
                f64::NAN
            } else {
                ls_factor(slope[i], r.acc[i], cell_size, cell_area, l_max)
            }
        })
        .collect();
    let channel: Vec<u8> = match masked.get(3) {
        None => stream.clone(),
        Some(dr) => (0..n)
            .map(|i| if stream[i] != 255 && !dr[i].is_nan() { if dr[i] == 1.0 { 1 } else { stream[i] } } else { 255 })
            .collect(),
    };
    let valid8: Vec<u8> = valid.iter().map(|&v| u8::from(v)).collect();
    let index = lu.classes_of(&codes, &valid8)?;
    let c32: Vec<f32> = bio[0].iter().map(|&c| c as f32).collect();
    let cp32: Vec<f32> = bio[0].iter().zip(&bio[1]).map(|(c, p)| (c * p) as f32).collect();
    let pick =
        |v: &[f32]| -> Vec<f32> { index.iter().map(|&c| if c >= 0 { v[c as usize] } else { f32::NAN }).collect() };
    let (w, cp) = (pick(&c32), pick(&cp32));
    let w_thr = weight_floor(&w);
    let (eros, erod) = (&masked[1], &masked[2]);
    let rkls: Vec<f32> =
        (0..n)
            .map(|i| {
                if valid[i] && !ls[i].is_nan() && channel[i] == 0 {
                    (ls[i] * eros[i] * erod[i]) as f32
                } else {
                    f32::NAN
                }
            })
            .collect();
    let usle: Vec<f32> = rkls.iter().zip(&cp).map(|(r, c)| r * c).collect();
    let con = connectivity(&w_thr, &slope_thr, &r, &channel, cell_area)?;
    let (ic0, kk, smax) = (ic_0 as f32, k as f32, sdr_max as f32);
    let sdr_f: Vec<f32> = (0..n)
        .map(|i| {
            if channel[i] == 1 {
                1.0
            } else if con.ic[i].is_nan() {
                f32::NAN
            } else {
                smax / (1.0 + npmath::exp((ic0 - con.ic[i]) / kk))
            }
        })
        .collect();
    let sed_export: Vec<f32> = usle.iter().zip(&sdr_f).map(|(u, s)| u * s).collect();
    let e_prime: Vec<f32> = (0..n)
        .map(|i| {
            if channel[i] == 1 {
                0.0
            } else if usle[i].is_nan() || sdr_f[i].is_nan() {
                f32::NAN
            } else {
                usle[i] * (1.0 - sdr_f[i])
            }
        })
        .collect();
    let dep_flux = ffi::serv_sediment_deposition(&r.mfd, &r.d8, &e_prime, &sdr_f, ny, nx, MFD_BLOCK)?;
    let dep = &dep_flux[..n];
    let avoided_erosion: Vec<f32> = rkls.iter().zip(&usle).map(|(r, u)| r - u).collect();
    let avoided_export: Vec<f32> = (0..n).map(|i| avoided_erosion[i] * sdr_f[i] + dep[i]).collect();
    let drains: Vec<f64> =
        (0..n).map(|i| if r.missing(i) { 255.0 } else { f64::from(u8::from(!con.d_dn[i].is_nan())) }).collect();
    let ha = (xcell * ycell).abs() / 10000.0;
    let wide = |v: &[f32]| -> Vec<f64> { v.iter().map(|&x| f64::from(x)).collect() };
    let report = [wide(&usle), wide(&sed_export), wide(dep), wide(&avoided_export), wide(&avoided_erosion)];
    let sums = zonal(&sheds, &grid, &report.iter().map(|v| v.as_slice()).collect::<Vec<_>>(), &[2])?;
    let ids =
        attribute(&sheds, "ws_id", "watersheds").unwrap_or_else(|_| (1..=sheds.len()).map(|i| i as f64).collect());
    let mut table = Table::new(
        ["ws_id", "usle_tot", "sed_export", "sed_dep", "avoid_exp", "avoid_eros"].map(String::from).to_vec(),
    );
    for (z, id) in ids.iter().enumerate() {
        table.rows.push(std::iter::once(*id).chain((0..5).map(|k| sums[z * 5 + k] * ha)).collect());
    }
    let mut out = Outputs::new();
    for (name, v) in [
        ("usle", &usle),
        ("sed_export", &sed_export),
        ("avoided_export", &avoided_export),
        ("avoided_erosion", &avoided_erosion),
        ("rkls", &rkls),
        ("sdr", &sdr_f),
        ("ic", &con.ic),
    ] {
        out.insert(name.into(), f32_map(&grid, v));
    }
    out.insert("sed_deposition".into(), f32_map(&grid, dep));
    out.insert("stream".into(), f64_map(&grid, channel.iter().map(|&c| f64::from(c)).collect()));
    out.insert("drains_to_stream".into(), f64_map(&grid, drains));
    out.insert("watershed_table".into(), Value::Table(table));
    Ok(out)
}

/// The index of connectivity (`hydro.connectivity_index`): landschaft's ls.hydro.connectivity_index.
pub fn connectivity_index(args: &Args) -> Result<Outputs> {
    let m = args.map("dem")?;
    let weight = args.map("weight")?;
    if weight.shape() != m.shape() {
        return Err(Error::msg("the weight differs in shape from the DEM"));
    }
    let method: String = args.param("method", "mfd".to_string())?.to_lowercase();
    if method != "mfd" && method != "d8" {
        return Err(Error::msg("method is 'mfd' or 'd8'"));
    }
    let (ny, nx) = (m.grid.height as i32, m.grid.width as i32);
    let t = m.grid.transform;
    let (xcell, ycell) = (t[1], t[5]);
    let filled = ffi::hydro_fill(&m.data, ny, nx, false)?;
    let slope = ffi::serv_pgp_slope(&filled, ny, nx, xcell, ycell, threads())?;
    let r = Routing::new(&filled, ny, nx, &method)?;
    let channel: Vec<u8> = match args.opt_map("streams")? {
        Some(s) => s.data.iter().map(|v| if v.is_nan() { 255 } else { u8::from(*v == 1.0) }).collect(),
        None => match args.opt_param::<f64>("threshold")? {
            Some(th) => r.streams(th)?,
            None => return Err(Error::msg("give the streams or a threshold")),
        },
    };
    let w32: Vec<f32> = weight.data.iter().map(|&x| x as f32).collect();
    let con = connectivity(&weight_floor(&w32), &thresholds(&slope), &r, &channel, xcell.abs().powi(2))?;
    Ok(outputs([("map", f32_map(&m.grid, &con.ic))]))
}
