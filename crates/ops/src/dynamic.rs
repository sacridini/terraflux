//! Dynamic variables (landschaft's `distance_to`, `share_of`): drivers computed again from the simulated map
//! at every step, and the stacks they go into (`stack.set`, `stack.classes`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

fn classes_param(args: &Args) -> Result<Vec<f64>> {
    match args.params.get("classes") {
        Some(serde_json::Value::Number(n)) => Ok(vec![n.as_f64().unwrap()]),
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| Error::msg(format!("classes: {e}"))),
        None => Err(Error::msg(format!("node {:?}: give the classes", args.node))),
    }
}

/// The Euclidean distance from each cell to the nearest cell of some classes (exact, between cell centres),
/// in map units (`res` the side of a cell) or cells; the map's diagonal everywhere when no cell has them; NaN
/// where the map has no value.
pub fn distance_to(land: &Map, classes: &[f64], res: f64) -> Result<Map> {
    let (ny, nx) = land.shape();
    let origin: Vec<u8> = land.data.iter().map(|v| u8::from(classes.contains(v))).collect();
    let mut out = if origin.contains(&1) {
        ffi::distance_exact(&origin, ny as i64, nx as i64, res)?
    } else {
        vec![(ny as f64).hypot(nx as f64) * res; ny * nx]
    };
    for (o, v) in out.iter_mut().zip(land.data.iter()) {
        if v.is_nan() {
            *o = f64::NAN;
        }
    }
    Ok(Map::new(land.grid.clone(), out))
}

/// A transition of conductances on the map's grid (landschaft's `ls.cost.Transition`): the value of the
/// connection from each cell in each direction of gdistance's layout.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostLayout {
    /// 4, 8 or 16.
    pub directions: i32,
    /// Whether the transition keeps one value per pair of cells (the directions after (0, 0) only).
    pub symmetric: bool,
    /// The grid's extent (xmin, xmax, ymin, ymax) as the transition was made.
    pub extent: [f64; 4],
    /// Whether the columns wrap around (global longitude/latitude).
    #[serde(default)]
    pub wrap: bool,
}

/// The accumulated cost from the nearest cell of some classes over a transition of conductances (gdistance's
/// accCost; landschaft's `distance_to(..., cost=...)`): inf where no such cell can be reached, or everywhere
/// when the map has none; NaN where the map has no value.
pub fn cost_to(land: &Map, classes: &[f64], values: &[f64], layout: &CostLayout) -> Result<Map> {
    let (ny, nx) = land.shape();
    let sources: Vec<i64> =
        land.data.iter().enumerate().filter(|(_, v)| classes.contains(v)).map(|(i, _)| i as i64).collect();
    let mut out = if sources.is_empty() {
        vec![f64::INFINITY; ny * nx]
    } else {
        let [xmin, xmax, ymin, ymax] = layout.extent;
        ffi::cost_accumulated(
            values,
            ny as i64,
            nx as i64,
            xmin,
            xmax,
            ymin,
            ymax,
            layout.wrap,
            layout.directions,
            layout.symmetric,
            &sources,
        )?
    };
    for (o, v) in out.iter_mut().zip(land.data.iter()) {
        if v.is_nan() {
            *o = f64::NAN;
        }
    }
    Ok(Map::new(land.grid.clone(), out))
}

/// The share of the cells around each cell (weights of the window, the cells without a value left out) that
/// hold some classes; NaN where the map has no value.
pub fn share_of(land: &Map, classes: &[f64], window: &[Vec<f64>]) -> Result<Map> {
    let (ny, nx) = land.shape();
    let (wy, wx) = (window.len(), window.first().map_or(0, Vec::len));
    if wy % 2 == 0 || wx % 2 == 0 || window.iter().any(|r| r.len() != wx) {
        return Err(Error::msg("the window needs two odd sides"));
    }
    let presence: Vec<f64> = land
        .data
        .iter()
        .map(|v| if v.is_nan() { f64::NAN } else { f64::from(u8::from(classes.contains(v))) })
        .collect();
    let mut out = ffi::focal_mean(&presence, ny as i64, nx as i64, &window.concat(), wy as i32, wx as i32)?;
    for (o, v) in out.iter_mut().zip(land.data.iter()) {
        if v.is_nan() {
            *o = f64::NAN;
        }
    }
    Ok(Map::new(land.grid.clone(), out))
}

struct DistanceTo;

impl Operator for DistanceTo {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "dynamic.distance_to",
            "The distance from each cell to the nearest cell of some classes (exact Euclidean distance between \
             cell centres), in map units or cells: a driver that moves with the simulated map.",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map"))
        .input(PortSpec::optional(
            "cost",
            Type::Stack,
            "a transition of conductances, a layer per direction (the accumulated cost instead of the distance)",
        ))
        .output(PortSpec::new("distance", Type::Map, "the distances (NaN where the map has no value)"))
        .param(PortSpec::new("classes", Type::Any, "a class or a list"))
        .param(
            PortSpec::optional("unit", Type::Text, "\"map\" (default; square cells) or \"cells\"")
                .choices(["map", "cells"]),
        )
        .param(PortSpec::optional(
            "layout",
            Type::Any,
            "the transition's { directions, symmetric, extent, wrap } (with `cost`)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let land = args.map("landscape")?;
        let classes = classes_param(args)?;
        if let Some(cost) = args.opt_stack("cost")? {
            let layout: CostLayout = args
                .opt_param("layout")?
                .ok_or_else(|| Error::msg(format!("node {:?}: a cost needs its layout", args.node)))?;
            if (cost.grid.height, cost.grid.width) != (land.grid.height, land.grid.width) {
                return Err(Error::msg("the cost transition differs in shape from the map"));
            }
            let values: Vec<f64> = cost.layers.iter().flat_map(|l| l.iter().copied()).collect();
            return Ok(outputs([("distance", Value::Map(cost_to(land, &classes, &values, &layout)?))]));
        }
        let unit: String = args.param("unit", "map".to_string())?;
        let res = match unit.as_str() {
            "cells" => 1.0,
            "map" => {
                let (rx, ry) = land.grid.resolution();
                if (rx - ry).abs() > 1e-9 * rx.abs().max(ry.abs()) {
                    return Err(Error::msg("distances in map units need square cells (unit = \"cells\")"));
                }
                rx
            }
            u => return Err(Error::msg(format!("unit must be \"map\" or \"cells\", not {u:?}"))),
        };
        Ok(outputs([("distance", Value::Map(distance_to(land, &classes, res)?))]))
    }
}

struct ShareOf;

impl Operator for ShareOf {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "dynamic.share_of",
            "The share of the cells around each cell that hold some classes (the mean of the window's weights x \
             presence over the cells with a value): a driver that moves with the simulated map.",
            Access::Halo(1),
        )
        .input(PortSpec::new("landscape", Type::Map, "the map"))
        .output(PortSpec::new("share", Type::Map, "the shares (NaN where the map has no value)"))
        .param(PortSpec::new("classes", Type::Any, "a class or a list"))
        .param(PortSpec::optional("window", Type::Any, "the side of a window of ones (3), or its weights"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let land = args.map("landscape")?;
        let classes = classes_param(args)?;
        let window: Vec<Vec<f64>> = match args.params.get("window") {
            None => vec![vec![1.0; 3]; 3],
            Some(serde_json::Value::Number(n)) => {
                let s = n.as_u64().ok_or_else(|| Error::msg("the window's side must be a whole number"))? as usize;
                vec![vec![1.0; s]; s]
            }
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| Error::msg(format!("window: {e}")))?,
        };
        Ok(outputs([("share", Value::Map(share_of(land, &classes, &window)?))]))
    }
}

struct Set;

impl Operator for Set {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "stack.set",
            "A stack with a map in the layer of a name (replacing the layer of that name, else added at the end): \
             a dynamic variable in place of the static driver of the same name.",
            Access::Local,
        )
        .input(PortSpec::new("stack", Type::Stack, "the stack"))
        .input(PortSpec::new("map", Type::Map, "the map"))
        .output(PortSpec::new("stack", Type::Stack, "the stack with the map"))
        .param(PortSpec::new("name", Type::Text, "the layer's name"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = args.stack("stack")?;
        let m = args.map("map")?;
        let name: String = args.param("name", String::new())?;
        if name.is_empty() {
            return Err(Error::msg(format!("node {:?}: give the layer's name", args.node)));
        }
        if m.data.len() != s.grid.cells() {
            return Err(Error::msg("the map differs in size from the stack"));
        }
        let mut out = s.clone();
        let key = Key::Name(name);
        match out.keys.iter().position(|k| *k == key) {
            Some(i) => out.layers[i] = m.data.clone(),
            None => {
                out.keys.push(key);
                out.layers.push(m.data.clone());
            }
        }
        Ok(outputs([("stack", Value::Stack(out))]))
    }
}

struct Classes;

impl Operator for Classes {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "stack.classes",
            "A stack whose layers are named by class (\"1\", \"2\"...) or transition (\"1_to_3\") as the suitability \
             of each class or the probability of each transition (e.g. computed elsewhere and read from a file).",
            Access::None,
        )
        .input(PortSpec::new("stack", Type::Stack, "layers named by class"))
        .output(PortSpec::new("stack", Type::Stack, "a layer per class"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = args.stack("stack")?;
        let keys = s
            .keys
            .iter()
            .map(|k| match k {
                Key::Name(n) => {
                    let bad = || Error::msg(format!("the layer {n:?} is not named by a class (or \"from_to_to\")"));
                    match n.split_once("_to_") {
                        Some((f, t)) => Ok(Key::Transition(
                            f.trim().parse().map_err(|_| bad())?,
                            t.trim().parse().map_err(|_| bad())?,
                        )),
                        None => n.trim().parse::<f64>().map(Key::Class).map_err(|_| bad()),
                    }
                }
                other => Ok(other.clone()),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(outputs([("stack", Value::Stack(Stack { grid: s.grid.clone(), keys, layers: s.layers.clone() }))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(DistanceTo));
    r.add(Arc::new(ShareOf));
    r.add(Arc::new(Set));
    r.add(Arc::new(Of));
    r.add(Arc::new(TableOf));
    r.add(Arc::new(Classes));
}

/// `table.of`: a table written in the settings.
struct TableOf;

impl Operator for TableOf {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "table.of",
            "A table written in the settings: its columns' names and its rows of numbers (null: no value), such as the small tables a planning problem takes.",
            Access::Local,
        )
        .output(PortSpec::new("table", Type::Table, "the table"))
        .param(PortSpec::new("columns", Type::Any, "the columns' names"))
        .param(PortSpec::new("rows", Type::Any, "the rows: [[number, ...], ...], as many numbers as columns"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let columns: Vec<String> = args.param("columns", Vec::new())?;
        let rows: Vec<Vec<Option<f64>>> = args.param("rows", Vec::new())?;
        if columns.is_empty() {
            return Err(Error::msg(format!("node {node:?}: the table has no columns")));
        }
        if let Some(k) = rows.iter().position(|r| r.len() != columns.len()) {
            return Err(Error::msg(format!(
                "node {node:?}: row {} has {} numbers for {} columns",
                k + 1,
                rows[k].len(),
                columns.len()
            )));
        }
        let mut t = Table::new(columns);
        t.rows = rows.into_iter().map(|r| r.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect()).collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}

/// `stack.of`: maps of a grid as one stack.
struct Of;

impl Operator for Of {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "stack.of",
            "Maps of one grid as a stack, a layer each named as its input (any names: the features of a \
             prioritization, the drivers of a model), in the order of their names.",
            Access::Local,
        )
        .extra(Type::Map)
        .output(PortSpec::new("stack", Type::Stack, "the maps' stack"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let mut grid = None;
        let (mut keys, mut layers) = (Vec::new(), Vec::new());
        for (name, v) in &args.inputs {
            let Value::Map(m) = v else {
                return Err(Error::msg(format!("node {:?}: {name} is no map", args.node)));
            };
            let g = grid.get_or_insert_with(|| m.grid.clone());
            if (g.width, g.height) != (m.grid.width, m.grid.height) {
                return Err(Error::msg(format!("node {:?}: {name} is not on the grid of the others", args.node)));
            }
            keys.push(Key::Name(name.clone()));
            layers.push(m.data.clone());
        }
        let grid = grid.ok_or_else(|| Error::msg(format!("node {:?}: no maps", args.node)))?;
        Ok(outputs([("stack", Value::Stack(Stack { grid, keys, layers }))]))
    }
}
