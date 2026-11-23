//! Operators on the point pattern of a layer (`vector.voronoi`, `vector.delaunay`, `vector.cluster`): GEOS's Voronoi
//! diagram and Delaunay triangulation through terraflux_gdal::geos, and a deterministic DBSCAN on the distance
//! between geometries, as the library's `tf.vector` gives them.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use terraflux_gdal::geos::{GeosIndex, Predicate, delaunay, intersect, union, voronoi};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector::{attribute_table, vector_of};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Voronoi));
    r.add(Arc::new(Delaunay));
    r.add(Arc::new(Cluster));
}

fn give(v: Vector) -> Outputs {
    let t = attribute_table(&v);
    outputs([("vector", Value::Vector(Arc::new(v))), ("table", Value::Table(t))])
}

fn err(node: &str) -> impl Fn(String) -> Error + '_ {
    move |m| Error::msg(format!("node {node:?}: {m}"))
}

/// Whether a geometry is something (not empty, not without a vertex).
fn present(g: &Geometry) -> bool {
    !matches!(g, Geometry::Empty) && g.bounds().is_some()
}

/// Features in the coordinate system of others (reprojected by GDAL when both are known and differ).
fn like(v: &Arc<Vector>, crs: &Option<String>) -> Result<Arc<Vector>> {
    match (&v.crs, crs) {
        (Some(a), Some(b)) if !terraflux_gdal::same_crs(a, b).unwrap_or(true) => {
            Ok(Arc::new(terraflux_gdal::reproject_vector(v, b).map_err(Error::msg)?))
        }
        _ => Ok(v.clone()),
    }
}

/// The key of a coordinate pair for "the same point" (0 and -0 are one).
fn key(p: [f64; 2]) -> (u64, u64) {
    ((p[0] + 0.0).to_bits(), (p[1] + 0.0).to_bits())
}

/// `vector.voronoi`.
struct Voronoi;

impl Operator for Voronoi {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.voronoi",
            "The Voronoi polygon of every point (GEOS's, as PostGIS's ST_VoronoiPolygons and QGIS's Voronoi \
             polygons): the area closer to that point than to any other, one polygon per point in the order of the \
             points, with their attributes. The extent is GEOS's own (the envelope of the points enlarged by a \
             margin, so the outer cells are finite); with `clip` (polygons, reprojected to the points') every cell \
             is cut by the union of its polygons, and a point whose cell is cut away keeps an empty one. Points at \
             the same place share one cell. Empty points, and the points of a set with a single distinct place, \
             get an empty geometry. `tolerance` is GEOS's snapping tolerance.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the points"))
        .input(PortSpec::optional("clip", Type::Vector, "polygons the cells are cut by"))
        .output(PortSpec::new("vector", Type::Vector, "the cells"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("tolerance", Type::Number, "GEOS's snapping tolerance (0)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let v = vector_of(args, "vector")?;
        let tolerance: f64 = args.param("tolerance", 0.0)?;
        if !(tolerance >= 0.0 && tolerance.is_finite()) {
            return Err(e("the tolerance is a number of 0 or more".into()));
        }
        let mut at: Vec<Option<[f64; 2]>> = Vec::with_capacity(v.len());
        for g in &v.geometries {
            at.push(match g {
                _ if !present(g) => None,
                Geometry::Point(p) => Some(*p),
                _ => return Err(e("voronoi needs points".into())),
            });
        }
        // the distinct places in the order of their first appearance, and the place of each point
        let mut rank: HashMap<(u64, u64), usize> = HashMap::new();
        let mut sites: Vec<[f64; 2]> = Vec::new();
        let place: Vec<Option<usize>> = at
            .iter()
            .map(|p| {
                p.map(|p| {
                    *rank.entry(key(p)).or_insert_with(|| {
                        sites.push(p);
                        sites.len() - 1
                    })
                })
            })
            .collect();
        let cells = if sites.len() >= 2 { voronoi(&sites, tolerance).map_err(&e)? } else { Vec::new() };
        let mut geometries: Vec<Geometry> =
            place.iter().map(|p| p.and_then(|k| cells.get(k).cloned()).unwrap_or(Geometry::Empty)).collect();
        if let Some(Value::Vector(c)) = args.get("clip") {
            let c = like(c, &v.crs)?;
            if c.geometries.iter().any(|g| present(g) && !g.is_polygonal()) {
                return Err(e("clip must hold polygons".into()));
            }
            let polys: Vec<Geometry> = c.geometries.iter().filter(|g| present(g)).cloned().collect();
            if polys.is_empty() {
                geometries.iter_mut().for_each(|g| *g = Geometry::Empty);
            } else {
                let region = union(&polys).map_err(&e)?;
                geometries = intersect(&geometries, &region).map_err(&e)?;
            }
        } else if args.get("clip").is_some() {
            return Err(e("clip must be features".into()));
        }
        Ok(give(Vector { geometries, attributes: v.attributes.clone(), crs: v.crs.clone() }))
    }
}

/// `vector.delaunay`.
struct Delaunay;

impl Operator for Delaunay {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.delaunay",
            "The Delaunay triangulation of all the vertices of the features (GEOS's, as PostGIS's \
             ST_DelaunayTriangles and QGIS's Delaunay triangulation): one polygon per triangle, or with `edges` \
             one line per edge, in the order GEOS gives them. The features have no attributes besides the geometry; \
             the coordinate system is the layer's. Fewer than three vertices, or all on a line, give no features. \
             `tolerance` is GEOS's snapping tolerance.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features whose vertices are triangulated"))
        .output(PortSpec::new("vector", Type::Vector, "the triangles or edges"))
        .output(PortSpec::new("table", Type::Table, "their attributes (none)"))
        .param(PortSpec::optional("edges", Type::Bool, "the edges as lines, not the triangles (false)"))
        .param(PortSpec::optional("tolerance", Type::Number, "GEOS's snapping tolerance (0)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = vector_of(args, "vector")?;
        let edges: bool = args.param("edges", false)?;
        let tolerance: f64 = args.param("tolerance", 0.0)?;
        let mut xy: Vec<[f64; 2]> = Vec::new();
        for g in &v.geometries {
            g.points(&mut |p| xy.push(p));
        }
        let geometries = delaunay(&xy, tolerance, edges).map_err(err(args.node))?;
        Ok(give(Vector { geometries, attributes: Vec::new(), crs: v.crs.clone() }))
    }
}

/// The neighbours of each feature (itself included) within `distance`, ascending.
fn neighbours(geoms: &[Geometry], distance: f64) -> std::result::Result<Vec<Vec<usize>>, String> {
    let index = GeosIndex::new(geoms)?;
    let mut out = vec![Vec::new(); geoms.len()];
    for (i, j) in index.pairs(geoms, Predicate::Dwithin, Some(distance))? {
        out[i].push(j);
    }
    Ok(out)
}

/// The cluster of each feature by DBSCAN, -1 for noise: features visited in index order; at an unvisited core
/// (at least `min_points` neighbours) a cluster starts and grows breadth first (FIFO, neighbours ascending); a
/// border feature joins the first cluster that reaches it. The clusters are numbered by their lowest member.
pub fn dbscan(neigh: &[Vec<usize>], min_points: usize) -> Vec<i64> {
    let n = neigh.len();
    let core: Vec<bool> = neigh.iter().map(|x| x.len() >= min_points).collect();
    let mut label = vec![-1i64; n];
    let mut next = 0;
    for i in 0..n {
        if label[i] != -1 || !core[i] {
            continue;
        }
        label[i] = next;
        let mut queue = VecDeque::from([i]);
        while let Some(k) = queue.pop_front() {
            for &j in &neigh[k] {
                if label[j] == -1 {
                    label[j] = next;
                    if core[j] {
                        queue.push_back(j);
                    }
                }
            }
        }
        next += 1;
    }
    // renumber by the lowest member of any kind
    let mut new: HashMap<i64, i64> = HashMap::new();
    label
        .iter()
        .map(|&c| {
            if c < 0 {
                return -1;
            }
            let k = new.len() as i64;
            *new.entry(c).or_insert(k)
        })
        .collect()
}

/// `vector.cluster`.
struct Cluster;

impl Operator for Cluster {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.cluster",
            "The features with a column of cluster ids: DBSCAN on the distance between geometries (PostGIS's \
             ST_ClusterDBSCAN and ST_ClusterWithin, QGIS's DBSCAN clustering). The neighbours of a feature are \
             those within `distance` of it (inclusive, itself included), whatever their type; a feature with at \
             least `min_points` neighbours is a core, which starts a cluster that grows through the cores' \
             neighbours; a feature reached but not a core joins the first cluster that reaches it, and one in no \
             cluster is noise, -1. Clusters are numbered 0, 1, 2... by their lowest feature. `min_points` 1 is \
             ST_ClusterWithin: the connected groups, with no noise. Empty features are noise. The result is \
             deterministic. An existing column of that name is overwritten.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .output(PortSpec::new("vector", Type::Vector, "the features with the cluster column"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::new("distance", Type::Number, "the neighbourhood, in the layer's units"))
        .param(PortSpec::optional("min_points", Type::Number, "neighbours (itself included) of a core (1)"))
        .param(PortSpec::optional("column", Type::Text, "the name of the cluster column (cluster)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let v = vector_of(args, "vector")?;
        let distance: f64 = args
            .opt_param("distance")?
            .ok_or_else(|| Error::msg(format!("node {:?}: give the distance", args.node)))?;
        if !(distance >= 0.0 && distance.is_finite()) {
            return Err(e(format!("distance must be a finite number >= 0, not {distance}")));
        }
        let min_points: f64 = args.param("min_points", 1.0)?;
        if !(min_points >= 1.0 && min_points.fract() == 0.0 && min_points < 1e9) {
            return Err(e(format!("min_points must be an integer >= 1, not {min_points}")));
        }
        let column: String = args.param("column", "cluster".to_string())?;
        if column.is_empty() {
            return Err(e("the column needs a name".into()));
        }
        let neigh = neighbours(&v.geometries, distance).map_err(&e)?;
        let ids: Vec<f64> = dbscan(&neigh, min_points as usize).into_iter().map(|c| c as f64).collect();
        let mut attributes = v.attributes.clone();
        match attributes.iter_mut().find(|(n, _)| *n == column) {
            Some((_, c)) => *c = Column::Numbers(ids),
            None => attributes.push((column, Column::Numbers(ids))),
        }
        Ok(give(Vector { geometries: v.geometries.clone(), attributes, crs: v.crs.clone() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(op: &dyn Operator, v: Vector, params: serde_json::Value) -> Result<Outputs> {
        run_with(op, v, None, params)
    }

    fn run_with(op: &dyn Operator, v: Vector, clip: Option<Vector>, params: serde_json::Value) -> Result<Outputs> {
        let mut inputs: HashMap<String, Value> = HashMap::new();
        inputs.insert("vector".to_string(), Value::Vector(Arc::new(v)));
        if let Some(c) = clip {
            inputs.insert("clip".to_string(), Value::Vector(Arc::new(c)));
        }
        let args = Args { node: "n", inputs: inputs.into_iter().collect(), params: params.as_object().unwrap() };
        op.run(&mut Ctx::default(), &args)
    }

    fn vector(o: &Outputs) -> Arc<Vector> {
        let Value::Vector(v) = &o["vector"] else { panic!() };
        v.clone()
    }

    fn points(p: &[[f64; 2]]) -> Vector {
        Vector {
            geometries: p.iter().map(|&p| Geometry::Point(p)).collect(),
            attributes: vec![("id".into(), Column::Numbers((1..=p.len()).map(|i| i as f64).collect()))],
            crs: Some("EPSG:32633".into()),
        }
    }

    fn ready() -> bool {
        terraflux_gdal::geos::geos().is_ok()
    }

    #[test]
    fn dbscan_follows_the_documented_order() {
        // a chain 0-1-2 and a lone 3; min_points 2: 0 and 2 have two neighbours with themselves
        let neigh = vec![vec![0, 1], vec![0, 1, 2], vec![1, 2], vec![3]];
        assert_eq!(dbscan(&neigh, 1), vec![0, 0, 0, 1]);
        assert_eq!(dbscan(&neigh, 2), vec![0, 0, 0, -1]);
        // only 1 is a core: 0 and 2 are borders of its cluster
        assert_eq!(dbscan(&neigh, 3), vec![0, 0, 0, -1]);
        assert_eq!(dbscan(&neigh, 4), vec![-1, -1, -1, -1]);
        // a border point reached by two clusters joins the first: 2 is within 1 and 3, both cores
        let two = vec![vec![0, 1, 2], vec![0, 1, 2], vec![0, 1, 2, 3, 4], vec![2, 3, 4], vec![2, 3, 4]];
        let l = dbscan(&two, 3);
        assert!(l.iter().all(|&c| c == 0));
        // features with no neighbour at all (empty geometries) are noise
        assert_eq!(dbscan(&[vec![], vec![1]], 1), vec![-1, 0]);
    }

    #[test]
    fn cluster_groups_by_distance_and_overwrites_the_column() {
        if !ready() {
            return;
        }
        let mut v = points(&[[0.0, 0.0], [1.0, 0.0], [50.0, 50.0], [2.0, 0.0], [51.0, 50.0], [200.0, 0.0]]);
        v.geometries.push(Geometry::Empty);
        v.attributes[0].1 = Column::Numbers(vec![1.0; 7]);
        v.attributes.push(("cluster".into(), Column::Numbers(vec![9.0; 7])));
        let o = run(&Cluster, v.clone(), serde_json::json!({"distance": 1.0})).unwrap();
        let out = vector(&o);
        assert_eq!(out.attributes.len(), 2);
        assert_eq!(out.column("cluster").unwrap().numbers(), vec![0.0, 0.0, 1.0, 0.0, 1.0, 2.0, -1.0]);
        assert!(o.contains_key("table"));
        let o = run(&Cluster, v.clone(), serde_json::json!({"distance": 1.0, "min_points": 2, "column": "k"})).unwrap();
        let out = vector(&o);
        assert_eq!(out.attributes.len(), 3);
        assert_eq!(out.column("k").unwrap().numbers(), vec![0.0, 0.0, 1.0, 0.0, 1.0, -1.0, -1.0]);
        assert!(run(&Cluster, v.clone(), serde_json::json!({})).is_err());
        assert!(run(&Cluster, v.clone(), serde_json::json!({"distance": -1.0})).is_err());
        assert!(run(&Cluster, v, serde_json::json!({"distance": 1.0, "min_points": 0})).is_err());
    }

    #[test]
    fn voronoi_gives_one_cell_per_point_with_duplicates_sharing() {
        if !ready() {
            return;
        }
        let mut v = points(&[[0.0, 0.0], [10.0, 0.0], [0.0, 0.0], [5.0, 8.0]]);
        v.geometries.push(Geometry::Empty);
        v.attributes[0].1 = Column::Numbers(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let out = vector(&run(&Voronoi, v.clone(), serde_json::json!({})).unwrap());
        assert_eq!(out.len(), 5);
        assert_eq!(out.geometries[0], out.geometries[2]);
        assert_eq!(out.geometries[4], Geometry::Empty);
        assert!(matches!(out.geometries[1], Geometry::Polygon(_)));
        assert_eq!(out.column("id").unwrap().numbers(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        // a clip of the unit area around the origin: only the cell of (0, 0) and its twin survive there
        let clip = Vector {
            geometries: vec![Geometry::Polygon(vec![vec![
                [-1.0, -1.0],
                [1.0, -1.0],
                [1.0, 1.0],
                [-1.0, 1.0],
                [-1.0, -1.0],
            ]])],
            attributes: vec![],
            crs: Some("EPSG:32633".into()),
        };
        let cut = vector(&run_with(&Voronoi, v.clone(), Some(clip), serde_json::json!({})).unwrap());
        assert_eq!(cut.geometries[0].bounds(), Some([-1.0, -1.0, 1.0, 1.0]));
        assert_eq!(cut.geometries[1], Geometry::Empty);
        assert_eq!(cut.geometries[0], cut.geometries[2]);
        // one distinct place gives empties; anything but points is an error
        let one = vector(&run(&Voronoi, points(&[[1.0, 1.0], [1.0, 1.0]]), serde_json::json!({})).unwrap());
        assert!(one.geometries.iter().all(|g| *g == Geometry::Empty));
        let line = Vector { geometries: vec![Geometry::LineString(vec![[0.0, 0.0], [1.0, 1.0]])], ..Vector::default() };
        assert!(run(&Voronoi, line, serde_json::json!({})).is_err());
        assert!(run(&Voronoi, v, serde_json::json!({"tolerance": -1.0})).is_err());
    }

    #[test]
    fn delaunay_triangulates_the_vertices() {
        if !ready() {
            return;
        }
        let v = points(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [5.0, 5.0]]);
        let o = run(&Delaunay, v.clone(), serde_json::json!({})).unwrap();
        let t = vector(&o);
        assert_eq!(t.len(), 4);
        assert!(t.attributes.is_empty());
        assert_eq!(t.crs.as_deref(), Some("EPSG:32633"));
        let sq = Vector {
            geometries: vec![Geometry::Polygon(vec![vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]]])],
            ..Vector::default()
        };
        assert_eq!(vector(&run(&Delaunay, sq, serde_json::json!({})).unwrap()).len(), 2);
        let e = vector(&run(&Delaunay, v, serde_json::json!({"edges": true})).unwrap());
        assert!(e.len() >= 8);
        assert!(vector(&run(&Delaunay, points(&[[0.0, 0.0]]), serde_json::json!({})).unwrap()).is_empty());
    }
}
