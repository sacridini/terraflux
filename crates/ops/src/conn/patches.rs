//! The operators of habitat graphs from rasters of patches or vector features: their nodes, the distances
//! between them, the graph and per-node values back on the patches or the features.

use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::graph::{Connections, HabitatGraph, table};
use super::raster::{self, DistanceOptions, Patches};
use super::vector::{self, Attribute};
use super::{bad, graph_out, graph_settings, settings, the_graph};
use crate::{Args, Ctx, Map, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Nodes));
    r.add(Arc::new(Distances));
    r.add(Arc::new(FromRaster));
    r.add(Arc::new(ToMap));
    r.add(Arc::new(FromVector));
    r.add(Arc::new(ToVector));
    r.add(Arc::new(FocalNodes));
    r.add(Arc::new(ProtConn { regions: false }));
    r.add(Arc::new(ProtConn { regions: true }));
    r.add(Arc::new(MakeGrid));
    r.add(Arc::new(ConnectivityGrid));
    r.add(Arc::new(EcaChangeGrid));
}

/// The map of patches and how to read it.
fn patch_settings(sig: OpSignature) -> OpSignature {
    vector_settings(raster_settings(sig))
}

fn vector_settings(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::optional("vector", Type::Vector, "or the nodes as features (polygons)"))
        .param(PortSpec::optional("id", Type::Text, "the features' id column (default 1, 2...)"))
}

fn raster_settings(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::optional("map", Type::Map, "the patches: a patch id per cell, or classes with `habitat`"))
        .param(PortSpec::optional(
            "habitat",
            Type::Any,
            "the habitat classes (a number or several): their patches are the nodes",
        ))
        .param(
            PortSpec::optional("directions", Type::Integer, "the patches' neighbours with `habitat`: 8 (default) or 4")
                .choices([4, 8]),
        )
}

fn node_settings(sig: OpSignature) -> OpSignature {
    sig.param(PortSpec::optional(
        "attribute",
        Type::Any,
        "a value per patch, in the order of their ids (default: area)",
    ))
    .param(PortSpec::optional("weighted", Type::Bool, "the attribute times the area (default false)"))
    .param(PortSpec::optional(
        "area_unit",
        Type::Text,
        "m2 (default), km2, ha, Dam2, inch2, foot2, yard2 or mile2",
    ))
}

fn distance_settings(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::optional("resistance", Type::Map, "the resistance of least-cost and commute-time distances"))
        .param(
            PortSpec::optional(
                "distance",
                Type::Text,
                "centroid (default), edge, least-cost (gdistance's, over the resistance) or commute-time",
            )
            .choices(["centroid", "edge", "least-cost", "commute-time"]),
        )
        .param(PortSpec::optional("max_distance", Type::Number, "the pairs up to it (default all)"))
        .param(PortSpec::optional(
            "distance_unit",
            Type::Text,
            "of centroid and edge distances: m (default), km, inch, foot, yard or mile",
        ))
        .param(PortSpec::optional("cost_directions", Type::Integer, "of the costs: 8 (default) or 4").choices([4, 8]))
        .param(
            PortSpec::optional(
                "cost_function",
                Type::Text,
                "the conductance between cells: makurhini (default, 1 / the destination's resistance) or mean",
            )
            .choices(["makurhini", "mean"]),
        )
        .param(
            PortSpec::optional(
                "unreachable",
                Type::Text,
                "pairs the resistance does not join: euclidean (default, the straight distance) or drop",
            )
            .choices(["euclidean", "drop"]),
        )
}

fn patches_of(args: &Args) -> Result<Patches> {
    let map = args.map("map")?;
    let habitat: Option<Vec<f64>> = match args.params.get("habitat") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Array(_)) => Some(args.param("habitat", vec![])?),
        Some(_) => Some(vec![args.param("habitat", 0.0)?]),
    };
    raster::read_patches(map, habitat.as_deref(), args.param("directions", 8)?).map_err(|e| bad(args, e))
}

/// The nodes: patches of a raster or features.
enum Source {
    Raster(Patches),
    Vector(Arc<terraflux_raster::vector::Vector>),
}

fn source_of(args: &Args) -> Result<Source> {
    match (args.get("map"), args.get("vector")) {
        (Some(_), None) => Ok(Source::Raster(patches_of(args)?)),
        (None, Some(Value::Vector(v))) => Ok(Source::Vector(v.clone())),
        (None, Some(_)) => Err(bad(args, "the vector input must be features")),
        _ => Err(bad(args, "connect a map of patches or a vector of features (one of them)")),
    }
}

fn node_table(args: &Args, s: &Source) -> Result<(Vec<f64>, Vec<f64>)> {
    let weighted = args.param("weighted", false)?;
    let unit: String = args.param("area_unit", "m2".into())?;
    match s {
        Source::Raster(p) => {
            let attribute: Option<Vec<f64>> = args.opt_param("attribute")?;
            p.node_table(attribute.as_deref(), weighted, &unit).map_err(|e| bad(args, e))
        }
        Source::Vector(v) => {
            let values: Vec<f64>;
            let attribute = match args.params.get("attribute") {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(c)) => Some(Attribute::Column(c)),
                Some(_) => {
                    values = args.param("attribute", vec![])?;
                    Some(Attribute::Values(&values))
                }
            };
            let id: Option<String> = args.opt_param("id")?;
            vector::node_table(v, id.as_deref(), attribute, weighted, &unit).map_err(|e| bad(args, e))
        }
    }
}

fn distance_table(args: &Args, s: &Source) -> Result<raster::PairTable> {
    let resistance: Option<&Map> = args.opt_map("resistance")?;
    let distance: String = args.param("distance", "centroid".into())?;
    let unit: String = args.param("distance_unit", "m".into())?;
    let function: String = args.param("cost_function", "makurhini".into())?;
    let unreachable: String = args.param("unreachable", "euclidean".into())?;
    let o = DistanceOptions {
        distance: &distance,
        max_distance: args.opt_param("max_distance")?,
        distance_unit: &unit,
        resistance,
        cost_directions: args.param("cost_directions", 8)?,
        cost_function: &function,
        unreachable: &unreachable,
    };
    match s {
        Source::Raster(p) => raster::distance_table(p, &o),
        Source::Vector(v) => {
            let id: Option<String> = args.opt_param("id")?;
            vector::distance_table(v, id.as_deref(), &o)
        }
    }
    .map_err(|e| bad(args, e))
}

struct Nodes;

impl Operator for Nodes {
    fn signature(&self) -> OpSignature {
        node_settings(patch_settings(
            OpSignature::new(
                "conn.nodes",
                "The nodes of a habitat graph from a raster of patches or from features (landschaft's \
                 node_table, Makurhini's nodesfile): each patch's id and attribute, its area by default (GDAL's \
                 area of the polygons).",
                Access::Global,
            )
            .output(PortSpec::new("table", Type::Table, "id and attribute")),
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = source_of(args)?;
        let (ids, values) = node_table(args, &s)?;
        Ok(outputs([("table", Value::Table(table(vec![("id".into(), ids), ("attribute".into(), values)])))]))
    }
}

struct Distances;

impl Operator for Distances {
    fn signature(&self) -> OpSignature {
        distance_settings(patch_settings(
            OpSignature::new(
                "conn.distances",
                "The distances between the patches of a raster or between features (landschaft's \
                 distance_table, Makurhini's distancefile): between their centroids (R's means of the cells' \
                 centres; the features' largest part's, or GEOS's point on the surface when it falls outside), \
                 their edges (the gap between their closest cells; GEOS's distance), least-cost over a \
                 resistance (gdistance's) or commute times; the straight distance for the pairs the resistance \
                 does not join.",
                Access::Global,
            )
            .output(PortSpec::new("table", Type::Table, "from, to and distance")),
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = source_of(args)?;
        let (from, to, d) = distance_table(args, &s)?;
        Ok(outputs([(
            "table",
            Value::Table(table(vec![("from".into(), from), ("to".into(), to), ("distance".into(), d)])),
        )]))
    }
}

struct FromRaster;

impl Operator for FromRaster {
    fn signature(&self) -> OpSignature {
        graph_settings(distance_settings(node_settings(patch_settings(
            OpSignature::new(
                "conn.graph_from_raster",
                "A habitat graph of the patches of a raster (landschaft's HabitatGraph.from_raster): the nodes' \
                 attributes and the distances between them (conn.nodes and conn.distances); its values map back \
                 onto the patches (conn.to_map).",
                Access::Global,
            )
            .output(PortSpec::new("graph", Type::HabitatGraph, "the habitat graph")),
        ))))
        .param(PortSpec::optional(
            "restoration",
            Type::Any,
            "per patch, in the order of their ids, 1 (existing) or 0 (to add)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = Source::Raster(patches_of(args)?);
        let mut g = graph_of(args, &s)?;
        let Source::Raster(p) = &s else { unreachable!() };
        let unit: String = args.param("area_unit", "m2".into())?;
        let (_, areas) = p.node_table(None, false, &unit).map_err(|e| bad(args, e))?;
        g.habitat_area = Some(terraflux_ls_sys::ffi::r_sum(&areas));
        g.patches = Some(Arc::new(p.node_raster().map_err(|e| bad(args, e))?));
        Ok(graph_out(g))
    }
}

struct ToMap;

impl Operator for ToMap {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "conn.to_map",
            "Per-node values on the patches of a graph made from a raster (landschaft's HabitatGraph.to_raster): \
             every cell of a node the value of its id in the table; missing elsewhere.",
            Access::Global,
        )
        .input(PortSpec::new("graph", Type::HabitatGraph, "a graph from conn.graph_from_raster"))
        .input(PortSpec::new("table", Type::Table, "a row per node: its id and values (conn.node_importance...)"))
        .output(PortSpec::new("map", Type::Map, "the values on the patches"))
        .param(PortSpec::new("column", Type::Text, "the column of the values"))
        .param(PortSpec::optional("id", Type::Text, "the column of the ids (default id)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        let Some(patches) = &g.patches else { return Err(bad(args, "the graph was not made from a raster")) };
        let Some(Value::Table(t)) = args.get("table") else { return Err(bad(args, "connect a table")) };
        let column: String = args.opt_param("column")?.ok_or_else(|| bad(args, "set the column"))?;
        let id: String = args.param("id", "id".into())?;
        let (Some(ids), Some(values)) = (t.column(&id), t.column(&column)) else {
            return Err(bad(args, format!("the table has no columns {id:?} and {column:?}")));
        };
        let mut by_id: HashMap<u64, f64> = HashMap::new();
        for (i, v) in ids.iter().zip(&values) {
            if by_id.insert(if *i == 0.0 { 0 } else { i.to_bits() }, *v).is_some() {
                return Err(bad(args, "the table has several rows per node (several distances): select one"));
            }
        }
        let per_node: Vec<f64> = g
            .ids
            .iter()
            .map(|i| by_id.get(&if *i == 0.0 { 0 } else { i.to_bits() }).copied().unwrap_or(f64::NAN))
            .collect();
        let data = patches.node.iter().map(|&k| if k >= 0 { per_node[k as usize] } else { f64::NAN }).collect();
        Ok(outputs([("map", Value::Map(Map::new(patches.grid.clone(), data)))]))
    }
}

/// A graph of the nodes and distances of a source.
fn graph_of(args: &Args, s: &Source) -> Result<HabitatGraph> {
    let (ids, values) = node_table(args, s)?;
    let (from, to, d) = distance_table(args, s)?;
    let (kind, conefor, area) = settings(args)?;
    let pairs = Connections::Pairs((0..from.len()).map(|k| [from[k], to[k], d[k]]).collect());
    let restoration: Option<Vec<f64>> = args.opt_param("restoration")?;
    HabitatGraph::new(values, pairs, Some(ids), kind, area, conefor, restoration).map_err(|e| bad(args, e))
}

struct FromVector;

impl Operator for FromVector {
    fn signature(&self) -> OpSignature {
        graph_settings(distance_settings(node_settings(vector_settings(
            OpSignature::new(
                "conn.graph_from_vector",
                "A habitat graph of vector features (landschaft's HabitatGraph.from_vector): the nodes' \
                 attributes and the distances between them (conn.nodes and conn.distances on the features); its \
                 values join back onto the features (conn.to_vector).",
                Access::Global,
            )
            .output(PortSpec::new("graph", Type::HabitatGraph, "the habitat graph")),
        ))))
        .param(PortSpec::optional("restoration", Type::Any, "per feature, 1 (existing) or 0 (to add)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let Some(Value::Vector(v)) = args.get("vector") else { return Err(bad(args, "connect the features")) };
        let s = Source::Vector(v.clone());
        let mut g = graph_of(args, &s)?;
        let unit: String = args.param("area_unit", "m2".into())?;
        let f = raster::factor(&raster::AREA_UNITS, &unit, "area").map_err(|e| bad(args, e))?;
        let areas = vector::gdal_areas(&v.geometries).map_err(|e| bad(args, e))?;
        g.habitat_area = Some(terraflux_ls_sys::ffi::r_sum(&areas) * f);
        g.features = Some(v.clone());
        Ok(graph_out(g))
    }
}

struct ToVector;

impl Operator for ToVector {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "conn.to_vector",
            "A table of nodes joined onto the features of a graph made from them (landschaft's \
             HabitatGraph.to_geodataframe): a feature per row of the table (its id's), with the table's columns.",
            Access::Global,
        )
        .input(PortSpec::new("graph", Type::HabitatGraph, "a graph from conn.graph_from_vector"))
        .input(PortSpec::new("table", Type::Table, "a row per node and distance: its id and values"))
        .output(PortSpec::new("vector", Type::Vector, "the features with the table's columns"))
        .param(PortSpec::optional("id", Type::Text, "the column of the ids (default id)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = the_graph(args)?;
        let Some(features) = &g.features else { return Err(bad(args, "the graph was not made from features")) };
        let Some(Value::Table(t)) = args.get("table") else { return Err(bad(args, "connect a table")) };
        let id: String = args.param("id", "id".into())?;
        let Some(ids) = t.column(&id) else { return Err(bad(args, format!("the table has no column {id:?}"))) };
        let by_id: HashMap<u64, usize> =
            g.ids.iter().enumerate().map(|(k, i)| (if *i == 0.0 { 0 } else { i.to_bits() }, k)).collect();
        let mut geometries = Vec::with_capacity(ids.len());
        for i in &ids {
            let k = by_id
                .get(&if *i == 0.0 { 0 } else { i.to_bits() })
                .ok_or_else(|| bad(args, format!("the id {i} is not a node of the graph")))?;
            geometries.push(features.geometries[*k].clone());
        }
        let attributes = t
            .columns
            .iter()
            .enumerate()
            .map(|(c, name)| {
                (name.clone(), terraflux_raster::vector::Column::Numbers(t.rows.iter().map(|r| r[c]).collect()))
            })
            .collect();
        let v = terraflux_raster::vector::Vector { geometries, attributes, crs: features.crs.clone() };
        Ok(outputs([("vector", Value::Vector(Arc::new(v)))]))
    }
}

/// A number or several.
fn numbers(args: &Args, name: &str) -> Result<Vec<f64>> {
    Ok(match args.params.get(name) {
        None | Some(serde_json::Value::Null) => return Err(bad(args, format!("set {name}"))),
        Some(serde_json::Value::Array(_)) => args.param(name, vec![])?,
        Some(_) => vec![args.param(name, 0.0)?],
    })
}

struct FocalNodes;

impl Operator for FocalNodes {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "conn.focal_nodes",
            "The importance of every node in its neighbourhood (Makurhini's MK_Focal_nodes, landschaft's \
             focal_nodes): the graph of the nodes within a search buffer of it (their points on the surface, \
             the landscape their bounding box) and, in it, EC and IIC or PC, the node's dI and its fractions, \
             and IComp (the index times dI). A row per node and distance.",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the nodes as features (polygons)"))
        .output(PortSpec::new("table", Type::Table, "the focal importances"))
        .param(PortSpec::new("distance", Type::Any, "the link distance (the distance of `probability`), or several"))
        .param(PortSpec::new("search_buffer", Type::Any, "the buffer around each node, or one per distance"))
        .param(PortSpec::optional("metric", Type::Text, "IIC (default) or PC").choices(["IIC", "PC"]))
        .param(PortSpec::optional("probability", Type::Number, "the probability at that distance (PC: 0.5)"))
        .param(PortSpec::optional("pij_min", Type::Number, "probabilities below it make no edge (default 0.01)"))
        .param(PortSpec::optional("id", Type::Text, "the features' id column (default 1, 2...)"))
        .param(PortSpec::optional("attribute", Type::Any, "a numeric column or a value per node (default: area)"))
        .param(PortSpec::optional("area_unit", Type::Text, "ha (default), m2, km2, Dam2, inch2, foot2, yard2 or mile2"))
        .param(PortSpec::optional("threshold", Type::Number, "nodes farther apart are not joined"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let Some(Value::Vector(v)) = args.get("vector") else { return Err(bad(args, "connect the features")) };
        let distances = numbers(args, "distance")?;
        let mut buffers = numbers(args, "search_buffer")?;
        if buffers.len() == 1 {
            buffers = vec![buffers[0]; distances.len()];
        }
        if buffers.len() != distances.len() {
            return Err(bad(args, "a search buffer, or one per distance"));
        }
        let unit: String = args.param("area_unit", "ha".into())?;
        let id: Option<String> = args.opt_param("id")?;
        let values: Vec<f64>;
        let attribute = match args.params.get("attribute") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(c)) => Some(Attribute::Column(c)),
            Some(_) => {
                values = args.param("attribute", vec![])?;
                Some(Attribute::Values(&values))
            }
        };
        let (ids, attr) = vector::node_table(v, id.as_deref(), attribute, false, &unit).map_err(|e| bad(args, e))?;
        let metric: String = args.param("metric", "IIC".into())?;
        let o = vector::Focal {
            distances: &distances,
            buffers: &buffers,
            metric: &metric,
            probability: args.opt_param("probability")?,
            pij_min: Some(args.param("pij_min", 0.01)?),
            area_unit: &unit,
            threshold: args.opt_param("threshold")?,
        };
        let t = vector::focal_nodes(v, &ids, &attr, &o).map_err(|e| bad(args, e))?;
        Ok(outputs([("table", Value::Table(t))]))
    }
}

struct ProtConn {
    /// One row per region of the regions given.
    regions: bool,
}

impl Operator for ProtConn {
    fn signature(&self) -> OpSignature {
        let sig = if self.regions {
            OpSignature::new(
                "conn.protconn_regions",
                "ProtConn for every region of several (landschaft's protconn_regions): conn.protconn's indicators \
                 with a row per region and distance.",
                Access::Global,
            )
            .input(PortSpec::new("regions", Type::Vector, "the regions, one feature each"))
            .param(PortSpec::optional("id", Type::Text, "the regions' id column (default 1, 2...)"))
        } else {
            OpSignature::new(
                "conn.protconn",
                "Saura's ProtConn (Makurhini's MK_ProtConn, landschaft's protconn): the share of a region \
                 protected and connected. The protected areas clipped to the region and dissolved are the nodes \
                 (and the transboundary ones within a distance of it; the region's parts with `bound`); PC's \
                 numerator over the most probable paths between them gives ECA, PC, Prot, ProtConn, ProtUnconn, \
                 RelConn and ProtConn's fractions. With `delta`, every protected area's importance.",
                Access::Global,
            )
            .input(PortSpec::new("region", Type::Vector, "the region (its features dissolved)"))
            .output(PortSpec::optional(
                "delta",
                Type::Vector,
                "with `delta`: the clipped protected areas, their dProt, dProtConn and varProtConn per distance",
            ))
            .param(PortSpec::optional(
                "delta",
                Type::Bool,
                "every protected area's importance too (default false)",
            ))
        };
        sig.input(PortSpec::new("protected", Type::Vector, "the protected areas (polygons)"))
            .input(PortSpec::optional("resistance", Type::Map, "the resistance of least-cost distances"))
            .output(PortSpec::new("table", Type::Table, "the indicators per distance"))
            .param(PortSpec::new("distance", Type::Any, "the median dispersal distance, or several"))
            .param(PortSpec::optional(
                "probability",
                Type::Number,
                "the probability at that distance (default 0.5; null: exp(-1))",
            ))
            .param(PortSpec::optional("transboundary", Type::Number, "the distance of the transboundary nodes"))
            .param(
                PortSpec::optional("transboundary_type", Type::Text, "around the nodes (default) or around the region")
                    .choices(["nodes", "region"]),
            )
            .param(PortSpec::optional("bound", Type::Bool, "the region's parts as nodes (ProtConn_Bound)"))
            .param(PortSpec::optional(
                "area_unit",
                Type::Text,
                "m2 (default), km2, ha, Dam2, inch2, foot2, yard2 or mile2",
            ))
            .param(
                PortSpec::optional("distance_type", Type::Text, "edge (default) or least-cost")
                    .choices(["edge", "least-cost"]),
            )
            .param(PortSpec::optional("transboundary_weight", Type::Number, "the weight of transboundary pairs (1)"))
            .param(
                PortSpec::optional("cost_directions", Type::Integer, "of least-cost: 8 (default) or 4").choices([4, 8]),
            )
            .param(
                PortSpec::optional("cost_function", Type::Text, "makurhini (default) or mean")
                    .choices(["makurhini", "mean"]),
            )
            .param(PortSpec::optional("preselect", Type::Bool, "keep only the protected areas near the region (true)"))
            .param(PortSpec::optional("pij_min", Type::Number, "probabilities below it make no edge (default 0)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        use super::protconn::{INDICATORS, Settings, protconn};
        let Some(Value::Vector(pas)) = args.get("protected") else {
            return Err(bad(args, "connect the protected areas"));
        };
        let distances = numbers(args, "distance")?;
        let probability = match args.params.get("probability") {
            None => Some(0.5),
            Some(serde_json::Value::Null) => None,
            Some(_) => Some(args.param("probability", 0.5)?),
        };
        let unit: String = args.param("area_unit", "m2".into())?;
        let ttype: String = args.param("transboundary_type", "nodes".into())?;
        let dtype: String = args.param("distance_type", "edge".into())?;
        let function: String = args.param("cost_function", "makurhini".into())?;
        let resistance = args.opt_map("resistance")?;
        let cost = resistance.map(|r| DistanceOptions {
            distance: "least-cost",
            max_distance: None,
            distance_unit: "m",
            resistance: Some(r),
            cost_directions: 8,
            cost_function: &function,
            unreachable: "euclidean",
        });
        let mut cost = cost;
        if let Some(c) = cost.as_mut() {
            c.cost_directions = args.param("cost_directions", 8)?;
        }
        let s = Settings {
            distances: &distances,
            probability,
            transboundary: args.opt_param("transboundary")?,
            transboundary_type: &ttype,
            bound: args.param("bound", false)?,
            area_unit: &unit,
            distance_type: &dtype,
            cost,
            weight: args.param("transboundary_weight", 1.0)?,
            preselect: args.param("preselect", true)?,
            pij_min: args.opt_param("pij_min")?,
        };
        let mut columns: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<f64>> = Vec::new();
        let mut delta_out = None;
        if self.regions {
            let Some(Value::Vector(regions)) = args.get("regions") else {
                return Err(bad(args, "connect the regions"));
            };
            let id: Option<String> = args.opt_param("id")?;
            let ids = vector::ids_of(regions, id.as_deref()).map_err(|e| bad(args, e))?;
            columns.push("region".into());
            for (k, g) in regions.geometries.iter().enumerate() {
                let (r, _) = protconn(&pas.geometries, std::slice::from_ref(g), &s, false).map_err(|e| bad(args, e))?;
                rows.extend(r.into_iter().map(|row| std::iter::once(ids[k]).chain(row).collect()));
            }
        } else {
            let Some(Value::Vector(region)) = args.get("region") else { return Err(bad(args, "connect the region")) };
            let (r, delta) = protconn(&pas.geometries, &region.geometries, &s, args.param("delta", false)?)
                .map_err(|e| bad(args, e))?;
            rows = r;
            if let Some(d) = delta {
                use terraflux_raster::vector::{Column, Vector};
                let col = |f: &dyn Fn(&super::protconn::DeltaRow) -> f64| Column::Numbers(d.iter().map(f).collect());
                let v = Vector {
                    geometries: d.iter().map(|x| x.0.clone()).collect(),
                    attributes: vec![
                        ("attribute".into(), col(&|x| x.1)),
                        ("distance".into(), col(&|x| x.2)),
                        ("dProt".into(), col(&|x| x.3)),
                        ("dProtConn".into(), col(&|x| x.4)),
                        ("varProtConn".into(), col(&|x| x.5)),
                    ],
                    crs: pas.crs.clone(),
                };
                delta_out = Some(Value::Vector(Arc::new(v)));
            }
        }
        columns.push("distance".into());
        columns.extend(INDICATORS.iter().map(|c| c.to_string()));
        let mut t = crate::Table::new(columns);
        t.rows = rows;
        let mut out = outputs([("table", Value::Table(t))]);
        if let Some(d) = delta_out {
            out.insert("delta".into(), d);
        }
        Ok(out)
    }
}

fn vector_input<'a>(args: &'a Args, port: &str) -> Result<&'a Arc<terraflux_raster::vector::Vector>> {
    match args.get(port) {
        Some(Value::Vector(v)) => Ok(v),
        _ => Err(bad(args, format!("connect the {port}"))),
    }
}

fn grid_cells(sig: OpSignature) -> OpSignature {
    sig.param(PortSpec::optional("cell_area", Type::Number, "the cells' area, in the region's units squared"))
        .param(PortSpec::optional("hexagonal", Type::Bool, "hexagons (default) or squares"))
}

/// The grid given, or one made over the region.
fn grid_of(args: &Args, region: &terraflux_raster::vector::Vector) -> Result<Arc<terraflux_raster::vector::Vector>> {
    if let Some(Value::Vector(g)) = args.get("grid") {
        return Ok(g.clone());
    }
    let area: f64 = args.opt_param("cell_area")?.ok_or_else(|| bad(args, "connect a grid or set cell_area"))?;
    let cells = super::grid::make_grid(&region.geometries, area, args.param("hexagonal", true)?, false, false)
        .map_err(|e| bad(args, e))?;
    Ok(Arc::new(super::grid::grid_vector(cells, region.crs.clone())))
}

struct MakeGrid;

impl Operator for MakeGrid {
    fn signature(&self) -> OpSignature {
        grid_cells(
            OpSignature::new(
                "conn.make_grid",
                "A grid of hexagons (sf's st_make_grid) or squares of a given area over a region (Makurhini's \
                 make_grid): the cells that meet the region, clipped to it or only those wholly inside.",
                Access::Global,
            )
            .input(PortSpec::new("region", Type::Vector, "the region"))
            .output(PortSpec::new("grid", Type::Vector, "the cells, OBJECTID 1, 2...")),
        )
        .param(PortSpec::optional("clip", Type::Bool, "the cells clipped to the region (default false)"))
        .param(PortSpec::optional("grid_boundary", Type::Bool, "only the cells wholly inside the region"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let region = vector_input(args, "region")?;
        let area: f64 = args.opt_param("cell_area")?.ok_or_else(|| bad(args, "set cell_area"))?;
        let cells = super::grid::make_grid(
            &region.geometries,
            area,
            args.param("hexagonal", true)?,
            args.param("clip", false)?,
            args.param("grid_boundary", false)?,
        )
        .map_err(|e| bad(args, e))?;
        let v = super::grid::grid_vector(cells, region.crs.clone());
        Ok(outputs([("grid", Value::Vector(Arc::new(v)))]))
    }
}

struct ConnectivityGrid;

impl Operator for ConnectivityGrid {
    fn signature(&self) -> OpSignature {
        grid_cells(
            OpSignature::new(
                "conn.connectivity_grid",
                "Connectivity per cell of a grid over a region (Makurhini's MK_Connect_grid, landschaft's \
                 connectivity_grid): ProtConn's indicators of the protected areas in each cell, or the PC of \
                 their dissolved parts (Protected_surface, LA, ECA, ECA_Normalized, PC).",
                Access::Global,
            )
            .input(PortSpec::new("protected", Type::Vector, "the protected areas (polygons)"))
            .input(PortSpec::new("region", Type::Vector, "the region"))
            .input(PortSpec::optional("grid", Type::Vector, "the cells (else made with cell_area)"))
            .output(PortSpec::new("grid", Type::Vector, "the cells with the indicators")),
        )
        .param(PortSpec::new("distance", Type::Number, "the median dispersal distance"))
        .param(PortSpec::optional("protconn_index", Type::Bool, "ProtConn's indicators (default true), else PC"))
        .param(PortSpec::optional("probability", Type::Number, "the probability at that distance (default 0.5)"))
        .param(PortSpec::optional("transboundary", Type::Number, "the distance of the transboundary nodes"))
        .param(PortSpec::optional("area_unit", Type::Text, "m2 (default), km2, ha..."))
        .param(PortSpec::optional("pij_min", Type::Number, "probabilities below it make no edge"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let pas = vector_input(args, "protected")?;
        let region = vector_input(args, "region")?;
        let grid = grid_of(args, region)?;
        let unit: String = args.param("area_unit", "m2".into())?;
        let s = super::grid::GridSettings {
            distance: args.opt_param("distance")?.ok_or_else(|| bad(args, "set the distance"))?,
            protconn_index: args.param("protconn_index", true)?,
            probability: args.param("probability", 0.5)?,
            transboundary: args.opt_param("transboundary")?,
            area_unit: &unit,
            pij_min: args.opt_param("pij_min")?,
        };
        let v =
            super::grid::connectivity_grid(&pas.geometries, &region.geometries, &grid, &s).map_err(|e| bad(args, e))?;
        Ok(outputs([("grid", Value::Vector(Arc::new(v)))]))
    }
}

struct EcaChangeGrid;

impl Operator for EcaChangeGrid {
    fn signature(&self) -> OpSignature {
        grid_cells(
            OpSignature::new(
                "conn.eca_change_grid",
                "The change of ECA per cell of a grid between consecutive times (Makurhini's MK_dECA_grid, \
                 landschaft's eca_change_grid): the habitat of each time clipped to the cell, its graph \
                 (areas, centroid distances), ECA of IIC or PC, their change and how it reads.",
                Access::Global,
            )
            .input(PortSpec::new("times", Type::Any, "the habitat at two or more times (a series of vectors)"))
            .input(PortSpec::new("region", Type::Vector, "the region"))
            .input(PortSpec::optional("grid", Type::Vector, "the cells (else made with cell_area)"))
            .output(PortSpec::new("grid", Type::Vector, "a cell per pair of times, with the changes")),
        )
        .param(PortSpec::new("distance", Type::Number, "the link distance"))
        .param(PortSpec::optional("metric", Type::Text, "IIC (default) or PC").choices(["IIC", "PC"]))
        .param(PortSpec::optional("probability", Type::Number, "the probability at that distance (PC: 0.5)"))
        .param(PortSpec::optional("pij_min", Type::Number, "probabilities below it make no edge"))
        .param(PortSpec::optional("names", Type::Any, "the times' names (default T1, T2...)"))
        .param(PortSpec::optional("area_unit", Type::Text, "m2 (default), km2, ha..."))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let times: Vec<&terraflux_raster::vector::Vector> = match args.get("times") {
            Some(Value::Series(items)) => items
                .iter()
                .map(|v| match v {
                    Value::Vector(v) => Ok(&**v),
                    _ => Err(bad(args, "the times must be vectors")),
                })
                .collect::<Result<_>>()?,
            _ => return Err(bad(args, "connect the times (a series of vectors)")),
        };
        let region = vector_input(args, "region")?;
        let grid = grid_of(args, region)?;
        let names: Vec<String> = args.param("names", (1..=times.len()).map(|k| format!("T{k}")).collect::<Vec<_>>())?;
        if names.len() != times.len() {
            return Err(bad(args, "a name per time"));
        }
        let metric: String = args.param("metric", "IIC".into())?;
        let unit: String = args.param("area_unit", "m2".into())?;
        let s = super::grid::ChangeSettings {
            distance: args.opt_param("distance")?.ok_or_else(|| bad(args, "set the distance"))?,
            metric: &metric,
            probability: args.opt_param("probability")?,
            pij_min: args.opt_param("pij_min")?,
            names: &names,
            area_unit: &unit,
        };
        let v = super::grid::eca_change_grid(&times, &grid, &s).map_err(|e| bad(args, e))?;
        Ok(outputs([("grid", Value::Vector(Arc::new(v)))]))
    }
}
