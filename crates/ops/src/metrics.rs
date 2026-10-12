//! Landscape metrics from landschaft's C++, the same values as the R package landscapemetrics (and FRAGSTATS'
//! extras): per patch (`metrics.patch`), per class (`metrics.class`), of the whole map (`metrics.landscape`) and
//! of the landscape in a moving window around each cell (`metrics.window`).
//!
//! A map's classes are its whole values (others are truncated, as terra::as.int); cells without a value are NA.
//! Areas and distances are in the units of the grid's cells (hectares and metres for a metric CRS, as
//! landscapemetrics).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::lsm_catalog::{METRICS, Metric};
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    for level in LEVELS {
        r.add(Arc::new(Metrics { level }));
    }
    r.add(Arc::new(Window));
}

/// The levels of the metrics, in landscapemetrics' order.
pub const LEVELS: [&str; 3] = ["patch", "class", "landscape"];

/// A row of the metrics, as landscapemetrics' calculate_lsm: the level, the class and patch where the metric
/// has one, the metric and its value.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub level: &'static str,
    pub class: Option<i32>,
    pub id: Option<i32>,
    pub metric: String,
    pub value: f64,
}

/// The metrics of a map and its patches.
pub struct Measured {
    pub rows: Vec<Row>,
    /// The map's classes, increasing.
    pub classes: Vec<i32>,
    /// Each cell's patch, 1.. (0 none), when asked for.
    pub labels: Vec<i32>,
    /// Each patch's class, when the labels were asked for.
    pub patch_class: Vec<i32>,
}

/// landscapemetrics' default options.
pub fn default_options() -> ffi::LsmOptions {
    ffi::LsmOptions {
        directions: 8,
        count_boundary: false,
        consider_boundary: false,
        edge_depth: 1,
        cell_center: false,
        classes_max: f64::NAN,
        neighbourhood: 4,
        ordered: true,
        base: "log2".into(),
        fragstats: false,
        search_radius: f64::NAN,
        connect_threshold: f64::NAN,
        boundary_contrast: 0.0,
        contrast: Vec::new(),
        similarity: Vec::new(),
    }
}

/// A map's classes as 32-bit integers (truncated) and its NA mask (empty when it has none).
fn classes_of(map: &Map) -> Result<(Vec<i32>, Vec<u8>)> {
    let mut na = Vec::new();
    let mut values = Vec::with_capacity(map.data.len());
    for (i, &v) in map.data.iter().enumerate() {
        if v.is_nan() {
            if na.is_empty() {
                na = vec![0u8; map.data.len()];
            }
            na[i] = 1;
            values.push(0);
        } else {
            let t = v.trunc();
            if t <= f64::from(i32::MIN) || t > f64::from(i32::MAX) {
                return Err(Error::msg(format!("the class {v} does not fit 32 bits")));
            }
            values.push(t as i32);
        }
    }
    Ok((values, na))
}

/// The cells' sides of a map's grid.
fn resolution(map: &Map) -> (f64, f64) {
    let t = map.grid.transform;
    (t[1].abs(), t[5].abs())
}

/// The metrics `specs` (level, metric) of a map, and its patches when `labels`.
pub fn measure(map: &Map, options: &ffi::LsmOptions, specs: &[(&'static str, &str)], labels: bool) -> Result<Measured> {
    let (values, na) = classes_of(map)?;
    let (ny, nx) = map.shape();
    let (resx, resy) = resolution(map);
    let levels: Vec<i32> = specs.iter().map(|(l, _)| LEVELS.iter().position(|x| x == l).unwrap() as i32).collect();
    let names: Vec<String> = specs.iter().map(|(_, m)| m.to_string()).collect();
    let t = ffi::lsm_table(&values, &na, ny as i64, nx as i64, resx, resy, options, &levels, &names, labels)?;
    let rows = (0..t.value.len())
        .map(|i| {
            let (level, metric) = specs[t.metric[i] as usize];
            Row {
                level,
                class: (t.cls_na[i] == 0).then_some(t.cls[i]),
                id: (t.id_na[i] == 0).then_some(t.id[i]),
                metric: metric.to_string(),
                value: t.value[i],
            }
        })
        .collect();
    Ok(Measured { rows, classes: t.class_values, labels: t.labels, patch_class: t.patch_class })
}

/// The metrics of a level chosen by a node's settings: those named in `metrics` (a name or a list), else every
/// one of the level (of a `type` when given, FRAGSTATS' extras too with `extras`).
pub fn chosen(args: &Args, level: &str) -> Result<Vec<&'static Metric>> {
    let of_level = METRICS.iter().filter(|m| m.level == level);
    let named: Option<Vec<String>> = match args.params.get("metrics") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(vec![s.clone()]),
        Some(v) => Some(
            serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: metrics: {e} (a name or a list)", args.node)))?,
        ),
    };
    if let Some(named) = named {
        return named
            .iter()
            .map(|n| {
                find(level, n).ok_or_else(|| {
                    Error::msg(format!(
                        "node {:?}: {n:?} is no {level} metric (they are: {})",
                        args.node,
                        METRICS
                            .iter()
                            .filter(|m| m.level == level && !m.extra)
                            .map(|m| m.metric)
                            .collect::<Vec<_>>()
                            .join(" ")
                    ))
                })
            })
            .collect();
    }
    let extras: bool = args.param("extras", false)?;
    let kind: Option<String> = args.opt_param("type")?;
    let kind = kind.map(|k| k.to_lowercase());
    let out: Vec<&Metric> = of_level
        .filter(|m| extras || !m.extra)
        .filter(|m| kind.as_ref().is_none_or(|k| m.kind.starts_with(k.as_str())))
        .collect();
    if out.is_empty() {
        return Err(Error::msg(format!("node {:?}: no {level} metric of the type {kind:?}", args.node)));
    }
    Ok(out)
}

/// A metric of a level by its abbreviation, or landscapemetrics' name of its function (lsm_c_ca as well as ca).
pub fn find(level: &str, name: &str) -> Option<&'static Metric> {
    let n = name.trim().to_lowercase();
    let short = n.strip_prefix("lsm_").and_then(|r| r.get(2..)).unwrap_or(&n);
    METRICS.iter().find(|m| m.level == level && m.metric == short)
}

/// The options of the metrics from a node's settings (landscapemetrics' defaults for those not given).
pub fn options(args: &Args) -> Result<ffi::LsmOptions> {
    let d = default_options();
    let triples = |name: &str| -> Result<Vec<f64>> {
        let t: Option<Vec<[f64; 3]>> = args.opt_param(name)?;
        Ok(t.unwrap_or_default().into_iter().flatten().collect())
    };
    let convention: String = args.param("convention", "landscapemetrics".to_string())?;
    if convention != "landscapemetrics" && convention != "fragstats" {
        return Err(Error::msg(format!(
            "node {:?}: the convention is landscapemetrics or fragstats, not {convention:?}",
            args.node
        )));
    }
    Ok(ffi::LsmOptions {
        directions: args.param("directions", d.directions)?,
        count_boundary: args.param("count_boundary", d.count_boundary)?,
        consider_boundary: args.param("consider_boundary", d.consider_boundary)?,
        edge_depth: args.param("edge_depth", d.edge_depth)?,
        cell_center: args.param("cell_center", d.cell_center)?,
        classes_max: args.opt_param("classes_max")?.unwrap_or(f64::NAN),
        neighbourhood: args.param("neighbourhood", d.neighbourhood)?,
        ordered: args.param("ordered", d.ordered)?,
        base: args.param("base", d.base)?,
        fragstats: convention == "fragstats",
        search_radius: args.opt_param("search_radius")?.unwrap_or(f64::NAN),
        connect_threshold: args.opt_param("connect_threshold")?.unwrap_or(f64::NAN),
        boundary_contrast: args.param("boundary_contrast", d.boundary_contrast)?,
        contrast: triples("contrast")?,
        similarity: triples("similarity")?,
    })
}

/// The settings every metric operator takes: the metrics chosen and their options.
pub(crate) fn settings(sig: OpSignature) -> OpSignature {
    sig.param(PortSpec::optional(
        "metrics",
        Type::Any,
        "the metrics (a name or a list: ca, pland, np...; default all of the level)",
    ))
    .param(
        PortSpec::optional(
            "type",
            Type::Text,
            "without metrics, those of a type: area and edge, shape, core area, aggregation, diversity, complexity, \
             contrast",
        )
        .choices(["area and edge", "shape", "core area", "aggregation", "diversity", "complexity", "contrast"]),
    )
    .param(PortSpec::optional("extras", Type::Bool, "without metrics, FRAGSTATS' that landscapemetrics lacks too"))
    .param(
        PortSpec::optional("directions", Type::Integer, "the patches' connectivity: 8 (default) or 4").choices([8, 4]),
    )
    .param(PortSpec::optional("count_boundary", Type::Bool, "the map's boundary counts as edge (default false)"))
    .param(PortSpec::optional("consider_boundary", Type::Bool, "cells at the boundary can be core (default false)"))
    .param(PortSpec::optional("edge_depth", Type::Integer, "core areas: the edge's depth in cells (default 1)"))
    .param(PortSpec::optional("cell_center", Type::Bool, "gyrate: the centroid on a cell of the patch"))
    .param(PortSpec::optional("classes_max", Type::Number, "rpr: the most classes there could be"))
    .param(
        PortSpec::optional("neighbourhood", Type::Integer, "complexity metrics: 4 (default) or 8 neighbours")
            .choices([4, 8]),
    )
    .param(PortSpec::optional("ordered", Type::Bool, "complexity metrics: ordered pairs (default true)"))
    .param(
        PortSpec::optional("base", Type::Text, "complexity metrics' logarithm: log2 (default), log or log10")
            .choices(["log2", "log", "log10"]),
    )
    .param(
        PortSpec::optional(
            "convention",
            Type::Text,
            "landscapemetrics (default) or fragstats (physical edge lengths, disjunct cores' sizes)",
        )
        .choices(["landscapemetrics", "fragstats"]),
    )
    .param(PortSpec::optional("search_radius", Type::Number, "prox, simi: the search radius (map units)"))
    .param(PortSpec::optional("connect_threshold", Type::Number, "connect: the distance of a connection"))
    .param(PortSpec::optional("boundary_contrast", Type::Number, "econ, cwed, teci: the weight of edges to NA"))
    .param(PortSpec::optional("contrast", Type::Any, "econ, cwed, teci: [class a, class b, weight] of each edge"))
    .param(PortSpec::optional("similarity", Type::Any, "simi: [class a, class b, weight] of each pair"))
}

struct Metrics {
    level: &'static str,
}

impl Operator for Metrics {
    fn signature(&self) -> OpSignature {
        let name = format!("metrics.{}", self.level);
        let sig = match self.level {
            "patch" => OpSignature::new(
                &name,
                "Landscape metrics of every patch (the cells of a class connected to one another), as \
                 landscapemetrics and FRAGSTATS: area, perimeter, shape, core, nearest neighbour... A table of a \
                 row per patch (its class and number, then the metrics), the map of the patches, and the map of \
                 one metric if asked for.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map of classes"))
            .output(PortSpec::new("table", Type::Table, "per patch: its class, its number and the metrics"))
            .output(PortSpec::new("patches", Type::Map, "each cell's patch, by number"))
            .output(PortSpec::new("map", Type::Map, "the metric shown (show) on each patch")),
            "class" => OpSignature::new(
                &name,
                "Landscape metrics of every class, as landscapemetrics and FRAGSTATS: area, share (pland), \
                 patches, edge density, aggregation, the patches' metrics summarised (mean, sd, cv...). A table \
                 of a row per class, and the map of one metric if asked for.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map of classes"))
            .output(PortSpec::new("table", Type::Table, "per class: the class and the metrics"))
            .output(PortSpec::new("map", Type::Map, "the metric shown (show) on each class")),
            _ => OpSignature::new(
                &name,
                "Landscape metrics of the whole map, as landscapemetrics and FRAGSTATS: diversity (shdi, sidi...), \
                 contagion, edge density, aggregation, the patches' metrics summarised... A table of one row.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map of classes"))
            .output(PortSpec::new("table", Type::Table, "the metrics")),
        };
        let sig = settings(sig);
        if self.level == "landscape" {
            sig
        } else {
            sig.param(PortSpec::optional("show", Type::Text, "a metric of the level to give as a map"))
        }
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let mut metrics = chosen(args, self.level)?;
        let show =
            match args.opt_param::<String>("show")? {
                Some(s) => Some(find(self.level, &s).ok_or_else(|| {
                    Error::msg(format!("node {:?}: show: {s:?} is no {} metric", args.node, self.level))
                })?),
                None => None,
            };
        // the metric shown is measured too
        if let Some(m) = show
            && !metrics.iter().any(|x| x.metric == m.metric)
        {
            metrics.push(m);
        }
        let show = show.map(|m| m.metric);
        let specs: Vec<(&'static str, &str)> = metrics.iter().map(|m| (self.level, m.metric)).collect();
        let patch = self.level == "patch";
        let m = measure(map, &options(args)?, &specs, patch)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let column = |name: &str| metrics.iter().position(|x| x.metric == name);
        match self.level {
            "patch" => {
                let n = m.patch_class.len();
                let mut columns = vec!["class".to_string(), "patch".to_string()];
                columns.extend(metrics.iter().map(|x| x.metric.to_string()));
                let mut rows: Vec<Vec<f64>> = (0..n)
                    .map(|i| {
                        let mut r = vec![f64::NAN; columns.len()];
                        r[0] = f64::from(m.classes[m.patch_class[i] as usize]);
                        r[1] = (i + 1) as f64;
                        r
                    })
                    .collect();
                for row in &m.rows {
                    if let (Some(id), Some(j)) = (row.id, column(&row.metric))
                        && (1..=n as i32).contains(&id)
                    {
                        rows[id as usize - 1][j + 2] = row.value;
                    }
                }
                let patches: Vec<f64> = m.labels.iter().map(|&l| if l > 0 { f64::from(l) } else { f64::NAN }).collect();
                let mut out = outputs([("patches", Value::Map(Map::new(map.grid.clone(), patches)))]);
                if let Some(j) = show.and_then(column) {
                    let painted =
                        m.labels.iter().map(|&l| if l > 0 { rows[l as usize - 1][j + 2] } else { f64::NAN }).collect();
                    out.insert("map".into(), Value::Map(Map::new(map.grid.clone(), painted)));
                }
                out.insert("table".into(), Value::Table(Table { columns, rows }));
                Ok(out)
            }
            "class" => {
                let mut columns = vec!["class".to_string()];
                columns.extend(metrics.iter().map(|x| x.metric.to_string()));
                let mut rows: Vec<Vec<f64>> = m
                    .classes
                    .iter()
                    .map(|&c| {
                        let mut r = vec![f64::NAN; columns.len()];
                        r[0] = f64::from(c);
                        r
                    })
                    .collect();
                for row in &m.rows {
                    let Some(j) = column(&row.metric) else { continue };
                    // a row without class (enn of a map whose classes have one patch each): every class's
                    match row.class.and_then(|c| m.classes.iter().position(|&x| x == c)) {
                        Some(i) => rows[i][j + 1] = row.value,
                        None if row.class.is_none() => rows.iter_mut().for_each(|r| r[j + 1] = row.value),
                        None => {}
                    }
                }
                let mut out = Outputs::new();
                if let Some(j) = show.and_then(column) {
                    let (values, _) = classes_of(map)?;
                    let painted = map
                        .data
                        .iter()
                        .zip(&values)
                        .map(|(v, c)| {
                            if v.is_nan() {
                                return f64::NAN;
                            }
                            m.classes.iter().position(|x| x == c).map_or(f64::NAN, |i| rows[i][j + 1])
                        })
                        .collect();
                    out.insert("map".into(), Value::Map(Map::new(map.grid.clone(), painted)));
                }
                out.insert("table".into(), Value::Table(Table { columns, rows }));
                Ok(out)
            }
            _ => {
                let columns: Vec<String> = metrics.iter().map(|x| x.metric.to_string()).collect();
                let mut row = vec![f64::NAN; columns.len()];
                for r in &m.rows {
                    if let Some(j) = column(&r.metric) {
                        row[j] = r.value;
                    }
                }
                Ok(outputs([("table", Value::Table(Table { columns, rows: vec![row] }))]))
            }
        }
    }
}

struct Window;

impl Operator for Window {
    fn signature(&self) -> OpSignature {
        settings(
            OpSignature::new(
                "metrics.window",
                "Landscape-level metrics in a moving window, as landscapemetrics' window_lsm: for every cell, the \
                 metrics of the landscape the window centred on it covers (cells outside the map are NA). A map per \
                 metric, in a stack.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map of classes"))
            .output(PortSpec::new("maps", Type::Stack, "a map per metric, by its name")),
        )
        .param(PortSpec::optional("window", Type::Integer, "the window's side in cells, odd (default 5)"))
        .param(
            PortSpec::optional("shape", Type::Text, "square (default) or circle (the cells within side/2)")
                .choices(["square", "circle"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let metrics = chosen(args, "landscape")?;
        let side: i64 = args.param("window", 5)?;
        if side < 1 || side % 2 == 0 {
            return Err(Error::msg(format!("node {:?}: the window is an odd number of cells (got {side})", args.node)));
        }
        let shape: String = args.param("shape", "square".to_string())?;
        let r = (side / 2) as f64;
        let window: Vec<u8> = match shape.as_str() {
            "square" => vec![1; (side * side) as usize],
            "circle" => (0..side * side)
                .map(|i| {
                    let (y, x) = ((i / side) as f64 - r, (i % side) as f64 - r);
                    u8::from(x.hypot(y) <= r)
                })
                .collect(),
            s => return Err(Error::msg(format!("node {:?}: the shape is square or circle, not {s:?}", args.node))),
        };
        let (values, na) = classes_of(map)?;
        let (ny, nx) = map.shape();
        let (resx, resy) = resolution(map);
        let names: Vec<String> = metrics.iter().map(|m| m.metric.to_string()).collect();
        let out = ffi::lsm_window(
            &values,
            &na,
            ny as i64,
            nx as i64,
            resx,
            resy,
            &window,
            side,
            side,
            &names,
            &options(args)?,
        )?;
        let cells = ny * nx;
        let maps = Stack {
            grid: map.grid.clone(),
            keys: names.iter().map(|n| Key::Name(n.clone())).collect(),
            layers: out.chunks(cells.max(1)).map(|c| Arc::new(c.to_vec())).collect(),
        };
        Ok(outputs([("maps", Value::Stack(maps))]))
    }
}
