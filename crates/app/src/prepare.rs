//! Data put in shape in a click: what a node's rasters or vectors are next to the model's reference grid (the
//! grid every map of the model is put on: its coordinate system, cells and extent), and the steps that put them
//! on it or make drivers of them (a distance to features or to a class, the slope of a surface). Each step is a
//! node of an operator already there, with a note saying why it was chosen; nothing is done that cannot be seen
//! and changed in the graph.

use std::collections::BTreeMap;

use terraflux_graph::{Model, Node, Source};
use terraflux_ops::{Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry};

/// What the values of a raster are, which decides how it is resampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Classes (land use, soils): a cell takes the nearest cell's class, or the most common one when the cells
    /// grow; never a class made up between two.
    Classes,
    /// Measures (elevation, rainfall): interpolated, or averaged when the cells grow.
    Values,
    /// Amounts per cell (people, animals): summed, so the total stays the same.
    Counts,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Classes, Kind::Values, Kind::Counts];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Classes => "classes",
            Kind::Values => "values",
            Kind::Counts => "counts",
        }
    }

    /// gdalwarp's resampling for cells growing (coarser) or not.
    pub fn resampling(self, coarser: bool) -> &'static str {
        match (self, coarser) {
            (Kind::Classes, false) => "near",
            (Kind::Classes, true) => "mode",
            (Kind::Values, false) => "bilinear",
            (Kind::Values, true) => "average",
            (Kind::Counts, _) => "sum",
        }
    }

    /// Why that resampling, in a sentence.
    pub fn why(self, coarser: bool) -> &'static str {
        match (self, coarser) {
            (Kind::Classes, false) => {
                "classes: each cell takes the class of the nearest cell, so no class is made up between two"
            }
            (Kind::Classes, true) => "classes on larger cells: each takes the most common class of the cells it covers",
            (Kind::Values, false) => "values: interpolated between the four nearest cells (bilinear)",
            (Kind::Values, true) => "values on larger cells: the mean of the cells each covers",
            (Kind::Counts, _) => "counts: the amounts of the cells each covers are summed, so the total is kept",
        }
    }
}

/// What a node's output is, as the preparation sees it.
#[derive(Debug, Clone)]
pub enum Data {
    Raster {
        grid: Box<Grid>,
        /// Whole values only (an integer type): classes, likely.
        whole: bool,
        layers: usize,
        /// The classes of a map in memory (at most 64), when whole.
        classes: Vec<f64>,
    },
    Vector {
        crs: Option<String>,
        features: usize,
        polygons: bool,
        lines: bool,
        points: bool,
        /// The attributes of numbers.
        numbers: Vec<String>,
    },
}

impl Data {
    pub fn of(v: &Value) -> Option<Data> {
        Some(match v {
            Value::Raster(r) => Data::Raster {
                grid: Box::new(r.grid_of(r.chosen).clone()),
                whole: !r.dtype().is_float(),
                layers: r.layers.len(),
                classes: Vec::new(),
            },
            Value::Map(m) => {
                let classes = classes_of(&m.data);
                Data::Raster { grid: Box::new((*m.grid).clone()), whole: !classes.is_empty(), layers: 1, classes }
            }
            Value::Stack(s) => Data::Raster {
                grid: Box::new((*s.grid).clone()),
                whole: s.layers.first().is_some_and(|l| !classes_of(l).is_empty()),
                layers: s.layers.len(),
                classes: Vec::new(),
            },
            Value::Vector(v) => Data::Vector {
                crs: v.crs.clone(),
                features: v.len(),
                polygons: v.geometries.iter().any(Geometry::is_polygonal),
                lines: v.geometries.iter().any(|g| matches!(g, Geometry::LineString(_) | Geometry::MultiLineString(_))),
                points: v.geometries.iter().any(Geometry::is_point),
                numbers: v
                    .attributes
                    .iter()
                    .filter(|(_, c)| matches!(c, Column::Numbers(_)))
                    .map(|(n, _)| n.clone())
                    .collect(),
            },
            _ => return None,
        })
    }

    /// What its values most likely are.
    pub fn kind(&self) -> Kind {
        match self {
            Data::Raster { whole: true, .. } => Kind::Classes,
            _ => Kind::Values,
        }
    }

    pub fn grid(&self) -> Option<&Grid> {
        match self {
            Data::Raster { grid, .. } => Some(grid),
            Data::Vector { .. } => None,
        }
    }
}

/// The classes of a map's values (whole numbers, at most 64 of them), increasing; none else.
fn classes_of(data: &[f64]) -> Vec<f64> {
    let mut seen = std::collections::BTreeSet::new();
    for &v in data.iter().filter(|v| !v.is_nan()) {
        if v.fract() != 0.0 || v.abs() > 1e15 {
            return Vec::new();
        }
        seen.insert(v as i64);
        if seen.len() > 64 {
            return Vec::new();
        }
    }
    seen.into_iter().map(|c| c as f64).collect()
}

/// A grid's coordinate system as GDAL takes it.
pub fn crs_of(g: &Grid) -> Option<String> {
    terraflux_ops::prepare::crs_of(g)
}

/// A coordinate system named shortly: its EPSG code, or the name in its WKT.
pub fn crs_name(crs: Option<&str>) -> String {
    let Some(c) = crs else { return "no coordinate system".into() };
    if let Some(code) = c.strip_prefix("EPSG:") {
        return format!("EPSG:{code}");
    }
    // the first quoted name of the WKT, and its authority at the end (ID["EPSG",4326] or AUTHORITY["EPSG","4326"])
    let name = c.split('"').nth(1).unwrap_or("a coordinate system").to_string();
    let tail = &c[c.len().saturating_sub(40)..];
    let code: String =
        tail.rsplit_once("\"EPSG\"").map(|(_, r)| r.chars().filter(char::is_ascii_digit).collect()).unwrap_or_default();
    if code.is_empty() { name } else { format!("{name} (EPSG:{code})") }
}

/// How a grid differs from the reference grid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Differences {
    /// Another coordinate system (or one of them unknown).
    pub crs: bool,
    /// Cells of another size.
    pub cells: bool,
    /// Cells of the same size not on the reference's lines.
    pub shifted: bool,
    /// Another extent.
    pub extent: bool,
    /// The cells grow (to the reference's): averaged rather than interpolated.
    pub coarser: bool,
}

impl Differences {
    pub fn any(&self) -> bool {
        self.crs || self.cells || self.shifted || self.extent
    }

    /// What differs, in words.
    pub fn describe(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.crs {
            out.push("another coordinate system");
        }
        if self.cells {
            out.push(if self.coarser { "smaller cells" } else { "larger cells" });
        }
        if self.shifted {
            out.push("cells shifted from the grid's");
        }
        if self.extent {
            out.push("another extent");
        }
        out
    }
}

/// How `g` differs from the reference grid.
pub fn compare(g: &Grid, reference: &Grid) -> Differences {
    let (a, b) = (crs_of(g), crs_of(reference));
    let crs = match (&a, &b) {
        (Some(a), Some(b)) => a != b && !terraflux_gdal::same_crs(a, b).unwrap_or(false),
        (None, None) => false,
        _ => true,
    };
    let (t, r) = (g.transform, reference.transform);
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-9 * y.abs().max(1.0);
    // cells of another size (in the same units: of another system they are resampled anyway)
    let cells = !crs && (!close(t[1].abs(), r[1].abs()) || !close(t[5].abs(), r[5].abs()));
    // the origin's distance from the reference's lines, in cells
    let off = |d: f64, side: f64| {
        let f = (d / side).rem_euclid(1.0);
        f.min(1.0 - f) > 1e-6
    };
    let shifted = !crs && !cells && (off(t[0] - r[0], r[1]) || off(t[3] - r[3], r[5]));
    let extent = crs
        || g.width as f64 * t[1].abs() != reference.width as f64 * r[1].abs()
        || g.height as f64 * t[5].abs() != reference.height as f64 * r[5].abs()
        || !close(t[0], r[0])
        || !close(t[3], r[3]);
    // the cell's area in the reference's units: the same units unless the systems differ (then by the extents)
    let coarser = if crs {
        let ext = |g: &Grid| g.width as f64 * g.height as f64;
        // more cells covering about the same place: finer cells, coarser on the reference
        ext(g) > ext(reference) * 1.5
    } else {
        t[1].abs() * t[5].abs() < r[1].abs() * r[5].abs()
    };
    Differences { crs, cells, shifted, extent, coarser }
}

/// A step of the preparation: an operator already there, its inputs and settings.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Onto the reference grid: raster.match (reprojected, resampled, cut and aligned at once).
    Match { kind: Kind, coarser: bool },
    /// Reprojected to a coordinate system: raster.reproject.
    Reproject { crs: String, kind: Kind },
    /// The slope of a surface: raster.terrain.
    Slope,
    /// The distance to cells of some classes: dynamic.distance_to.
    DistanceToClass { classes: Vec<f64> },
    /// The distance to features, on the reference grid: vector.distance.
    DistanceToFeatures,
    /// Features burnt into the reference grid: vector.rasterize.
    Rasterize { field: Option<String> },
    /// The statistics of the reference grid's map in every polygon: vector.zonal.
    Zonal,
}

impl Step {
    pub fn op(&self) -> &'static str {
        match self {
            Step::Match { .. } => "raster.match",
            Step::Reproject { .. } => "raster.reproject",
            Step::Slope => "raster.terrain",
            Step::DistanceToClass { .. } => "dynamic.distance_to",
            Step::DistanceToFeatures => "vector.distance",
            Step::Rasterize { .. } => "vector.rasterize",
            Step::Zonal => "vector.zonal",
        }
    }

    /// Whether it needs the reference grid.
    pub fn needs_reference(&self) -> bool {
        matches!(self, Step::Match { .. } | Step::DistanceToFeatures | Step::Rasterize { .. } | Step::Zonal)
    }

    /// What it does, as a button says it.
    pub fn title(&self) -> String {
        match self {
            Step::Match { .. } => "Put on the reference grid".into(),
            Step::Reproject { crs, .. } => format!("Reproject to {}", crs_name(Some(crs))),
            Step::Slope => "Slope of this surface".into(),
            Step::DistanceToClass { classes } => format!("Distance to class {}", list(classes)),
            Step::DistanceToFeatures => "Distance to these features".into(),
            Step::Rasterize { field: Some(f) } => format!("Burn into the grid ({f})"),
            Step::Rasterize { field: None } => "Burn into the grid".into(),
            Step::Zonal => "Statistics of the reference map in each polygon".into(),
        }
    }

    /// Why, the note the node keeps.
    pub fn why(&self, differences: &[&str]) -> String {
        match self {
            Step::Match { kind, coarser } => {
                let what =
                    if differences.is_empty() { String::new() } else { format!(" ({})", differences.join(", ")) };
                format!(
                    "Put on the reference grid{what}: reprojected, resampled, cut and aligned in one go. Resampling \
                     {}: {}.",
                    kind.resampling(*coarser),
                    kind.why(*coarser)
                )
            }
            Step::Reproject { crs, kind } => {
                format!("Reprojected to {}, its cells resampled by the data: {}.", crs_name(Some(crs)), kind.why(false))
            }
            Step::Slope => "The slope of the surface (degrees, gdaldem), a driver of where change happens.".into(),
            Step::DistanceToClass { classes } => format!(
                "The distance from every cell to the nearest cell of class {} (map units): a driver; when the map is \
                 the simulated one, it moves with it.",
                list(classes)
            ),
            Step::DistanceToFeatures => "The distance from every cell of the reference grid to the nearest feature \
                                         (map units), a driver (roads, rivers, towns...). The features are \
                                         reprojected to the grid first."
                .into(),
            Step::Rasterize { field } => match field {
                Some(f) => format!("The features' {f} burnt into the reference grid (0 elsewhere)."),
                None => "The features burnt into the reference grid: 1 where they are, 0 elsewhere.".into(),
            },
            Step::Zonal => "The statistics of the reference map's cells in each polygon.".into(),
        }
    }
}

fn list(classes: &[f64]) -> String {
    classes.iter().map(|c| format!("{c}")).collect::<Vec<_>>().join(", ")
}

/// The steps a node's data suggests (the first the one most needed), next to the reference grid if there is
/// one.
pub fn suggestions(data: &Data, kind: Kind, reference: Option<&Grid>) -> Vec<Step> {
    let mut out = Vec::new();
    match data {
        Data::Raster { grid, classes, .. } => {
            if let Some(r) = reference {
                let d = compare(grid, r);
                if d.any() {
                    out.push(Step::Match { kind, coarser: d.coarser });
                }
            }
            if kind == Kind::Values {
                out.push(Step::Slope);
            }
            if kind == Kind::Classes && !classes.is_empty() {
                out.push(Step::DistanceToClass { classes: vec![classes[0]] });
            }
        }
        Data::Vector { polygons, .. } => {
            if reference.is_some() {
                out.push(Step::DistanceToFeatures);
                out.push(Step::Rasterize { field: None });
                if *polygons {
                    out.push(Step::Zonal);
                }
            }
        }
    }
    out
}

/// The reference grid of a model: the output most used as the grid others are put on (`like` of raster.match,
/// vector.distance, vector.rasterize), if any.
pub fn reference_of(m: &Model) -> Option<(String, String)> {
    let mut count: BTreeMap<(String, String), usize> = BTreeMap::new();
    for n in &m.nodes {
        if !matches!(n.op.as_str(), "raster.match" | "vector.distance" | "vector.rasterize") {
            continue;
        }
        if let Some(Source::Ref(r)) = n.inputs.get("like")
            && let Some((node, port)) = r.split_once('.')
        {
            *count.entry((node.to_string(), port.to_string())).or_default() += 1;
        }
    }
    count.into_iter().max_by_key(|(_, c)| *c).map(|(k, _)| k)
}

/// Add a step's node to the top level of a model, its data `from` (node, port), on the reference grid `reference`
/// (node, port) when it needs one, at `pos`; the new node's id.
pub fn apply(
    m: &mut Model,
    r: &Registry,
    step: &Step,
    from: (&str, &str),
    reference: Option<(&str, &str)>,
    differences: &[&str],
    pos: [f32; 2],
) -> Result<String, String> {
    let top: &[String] = &[];
    if step.needs_reference() && reference.is_none() {
        return Err("choose the reference grid first".into());
    }
    let id = crate::editor::add_node(m, r, top, step.op(), pos)?;
    let data = format!("{}.{}", from.0, from.1);
    let grid = reference.map(|(n, p)| format!("{n}.{p}"));
    let set = |m: &mut Model, k: &str, v: &str| crate::editor::set_param(m, top, &id, k, v);
    let connect = |m: &mut Model, reference: &str, port: &str| crate::editor::connect(m, r, top, reference, &id, port);
    match step {
        Step::Match { kind, coarser } => {
            connect(m, &data, "raster")?;
            connect(m, grid.as_deref().unwrap(), "like")?;
            set(m, "resampling", kind.resampling(*coarser))?;
        }
        Step::Reproject { crs, kind } => {
            connect(m, &data, "raster")?;
            set(m, "crs", crs)?;
            set(m, "resampling", kind.resampling(false))?;
        }
        Step::Slope => {
            connect(m, &data, "raster")?;
            set(m, "measure", "slope")?;
        }
        Step::DistanceToClass { classes } => {
            connect(m, &data, "landscape")?;
            set(m, "classes", &serde_json::to_string(classes).unwrap())?;
        }
        Step::DistanceToFeatures | Step::Rasterize { .. } => {
            connect(m, &data, "vector")?;
            connect(m, grid.as_deref().unwrap(), "like")?;
            if let Step::Rasterize { field: Some(f) } = step {
                set(m, "field", f)?;
            }
        }
        Step::Zonal => {
            connect(m, &data, "vector")?;
            connect(m, grid.as_deref().unwrap(), "map")?;
            set(m, "stats", r#"["count", "mean", "min", "max", "std"]"#)?;
        }
    }
    if let Some(n) = m.nodes.iter_mut().find(|n| n.id == id) {
        n.note = Some(step.why(differences));
    }
    Ok(id)
}

/// The top-level nodes whose first output is rasters read from files (raster.read), but `except`.
pub fn read_rasters<'a>(m: &'a Model, except: &str) -> Vec<&'a Node> {
    m.nodes.iter().filter(|n| n.op == "raster.read" && n.id != except).collect()
}
