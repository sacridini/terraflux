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
