//! Habitat-graph nodes and distances from vector features (landschaft's node_table and distance_table on a
//! GeoDataFrame): the polygons' areas as GDAL computes them, their centroids as landschaft takes them (the
//! largest part's, or GEOS's point on the surface when it falls outside), and the distances between centroids,
//! between edges (GEOS's) or least-cost and commute times over a resistance. GEOS through GDAL.

use terraflux_gdal::Geos;
use terraflux_ls_sys::ffi;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::raster::{self, AREA_UNITS, DISTANCE_UNITS, DistanceOptions, PairTable, factor};

/// The polygons of a geometry (none for other kinds).
fn polygons(g: &Geometry) -> Vec<&Vec<Vec<[f64; 2]>>> {
    match g {
        Geometry::Polygon(p) => vec![p],
        Geometry::MultiPolygon(ps) => ps.iter().collect(),
        _ => Vec::new(),
    }
}

/// The areas of polygons as GDAL computes them (OGR get_Area, sf's st_area): 0 for other geometries.
pub fn gdal_areas(geoms: &[Geometry]) -> Result<Vec<f64>, String> {
    let (mut x, mut y) = (Vec::new(), Vec::new());
    let (mut ring_start, mut poly_start, mut geom_start) = (vec![0i64], vec![0i64], vec![0i64]);
    for g in geoms {
        for p in polygons(g) {
            for ring in p {
                for c in ring {
                    x.push(c[0]);
                    y.push(c[1]);
                }
                ring_start.push(x.len() as i64);
            }
            poly_start.push(ring_start.len() as i64 - 1);
        }
        geom_start.push(poly_start.len() as i64 - 1);
    }
    ffi::conn_gdal_areas(&x, &y, &ring_start, &poly_start, &geom_start).map_err(|e| e.to_string())
}

/// The shoelace area of a polygon (its exterior less its holes), to rank the parts of a multipolygon.
fn part_area(p: &[Vec<[f64; 2]>]) -> f64 {
    let ring = |r: &Vec<[f64; 2]>| -> f64 {
        let mut s = 0.0;
        for k in 1..r.len() {
            s += r[k - 1][0] * r[k][1] - r[k][0] * r[k - 1][1];
        }
        (s / 2.0).abs()
    };
    p.iter().enumerate().map(|(k, r)| if k == 0 { ring(r) } else { -ring(r) }).sum()
}

/// landschaft's vector_centroids: the centroid of each geometry (of its largest part when it has several),
/// GEOS's point on the surface when that falls outside it.
pub fn centroids(geoms: &[Geometry]) -> Result<(Vec<f64>, Vec<f64>), String> {
    let largest: Vec<Geometry> = geoms
        .iter()
        .map(|g| match g {
            Geometry::MultiPolygon(ps) if ps.len() > 1 => {
                let mut best = 0;
                for k in 1..ps.len() {
                    if part_area(&ps[k]) > part_area(&ps[best]) {
                        best = k;
                    }
                }
                Geometry::Polygon(ps[best].clone())
            }
            g => g.clone(),
        })
        .collect();
    let parts = Geos::new(&largest)?;
    let whole = Geos::new(geoms)?;
    let (mut x, mut y) = (Vec::with_capacity(geoms.len()), Vec::with_capacity(geoms.len()));
    for k in 0..geoms.len() {
        let mut p = parts.centroid(k)?;
        if !whole.contains_point(k, p)? {
            p = whole.point_on_surface(k)?;
        }
        x.push(p[0]);
        y.push(p[1]);
    }
    Ok((x, y))
}

/// The nodes' ids: a numeric column, or 1, 2...
pub fn ids_of(v: &Vector, id: Option<&str>) -> Result<Vec<f64>, String> {
    if v.is_empty() {
        return Err("a habitat graph needs at least one node".into());
    }
    match id {
        None => Ok((1..=v.len()).map(|i| i as f64).collect()),
        Some(name) => match v.attributes.iter().find(|(n, _)| n == name) {
            Some((_, c)) => {
                let ids = c.numbers();
                if ids.iter().any(|i| i.is_nan()) {
                    return Err(format!("the ids of {name:?} must be numbers"));
                }
                Ok(ids)
            }
            None => Err(format!("no column {name:?} in the nodes")),
        },
    }
}

/// An attribute: a numeric column of the features, or a value per feature.
pub enum Attribute<'a> {
    Column(&'a str),
    Values(&'a [f64]),
}

/// The nodes' ids and attributes: their area in the unit, an attribute, or both multiplied (`weighted`).
pub fn node_table(
    v: &Vector,
    id: Option<&str>,
    attribute: Option<Attribute>,
    weighted: bool,
    area_unit: &str,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let ids = ids_of(v, id)?;
    let f = factor(&AREA_UNITS, area_unit, "area")?;
    let values = match attribute {
        None => gdal_areas(&v.geometries)?.iter().map(|a| a * f).collect(),
        Some(a) => {
            let values: Vec<f64> = match a {
                Attribute::Column(name) => match v.attributes.iter().find(|(n, _)| n == name) {
                    Some((_, Column::Numbers(x))) => x.clone(),
                    Some((_, c)) => c.numbers(),
                    None => return Err(format!("no column {name:?} in the nodes")),
                },
                Attribute::Values(x) => x.to_vec(),
            };
            if values.len() != v.len() {
                return Err("attribute must have one value per node".into());
            }
            if weighted {
                let areas = gdal_areas(&v.geometries)?;
                values.iter().zip(&areas).map(|(x, a)| x * a * f).collect()
            } else {
                values
            }
        }
    };
    Ok((ids, values))
}

/// The distances between features: centroids, edges (GEOS's), least-cost or commute times.
pub fn distance_table(v: &Vector, id: Option<&str>, o: &DistanceOptions) -> Result<PairTable, String> {
    if !["centroid", "edge", "least-cost", "commute-time"].contains(&o.distance) {
        return Err(format!("distance must be centroid, edge, least-cost or commute-time, got {:?}", o.distance));
    }
    let ids = ids_of(v, id)?;
    if o.distance == "least-cost" || o.distance == "commute-time" {
        let (x, y) = centroids(&v.geometries)?;
        return raster::least_cost_table(&ids, &x, &y, o);
    }
    let f = factor(&DISTANCE_UNITS, o.distance_unit, "distance")?;
    let limit = o.max_distance.map_or(f64::INFINITY, |m| m / f * (1.0 + 1e-12));
    let (i, j, d): (Vec<usize>, Vec<usize>, Vec<f64>) = if o.distance == "centroid" {
        let (x, y) = centroids(&v.geometries)?;
        let r = ffi::conn_point_distances(&x, &y, limit).map_err(|e| e.to_string())?;
        (r.i.iter().map(|&k| k as usize).collect(), r.j.iter().map(|&k| k as usize).collect(), r.distance.to_vec())
    } else {
        let g = Geos::new(&v.geometries)?;
        let n = g.len();
        let (mut i, mut j, mut d) = (Vec::new(), Vec::new(), Vec::new());
        for a in 0..n {
            for b in a + 1..n {
                let dist = g.distance(a, b);
                if dist <= limit {
                    i.push(a);
                    j.push(b);
                    d.push(dist);
                }
            }
        }
        (i, j, d)
    };
    let mut out: PairTable = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..i.len() {
        let dd = d[k] * f;
        if o.max_distance.is_some_and(|m| dd.is_nan() || dd > m) {
            continue;
        }
        out.0.push(ids[i[k]]);
        out.1.push(ids[j[k]]);
        out.2.push(dd);
    }
    Ok(out)
}

/// A geometry's bounds: xmin, ymin, xmax, ymax.
fn bounds(g: &Geometry) -> [f64; 4] {
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let mut add = |c: &[f64; 2]| {
        b[0] = b[0].min(c[0]);
        b[1] = b[1].min(c[1]);
        b[2] = b[2].max(c[0]);
        b[3] = b[3].max(c[1]);
    };
    match g {
        Geometry::Empty => {}
        Geometry::Point(p) => add(p),
        Geometry::LineString(l) | Geometry::MultiPoint(l) => l.iter().for_each(&mut add),
        Geometry::Polygon(rs) | Geometry::MultiLineString(rs) => rs.iter().flatten().for_each(&mut add),
        Geometry::MultiPolygon(ps) => ps.iter().flatten().flatten().for_each(&mut add),
    }
    b
}

/// The settings of focal_nodes.
pub struct Focal<'a> {
    pub distances: &'a [f64],
    /// A buffer per distance.
    pub buffers: &'a [f64],
    /// IIC or PC.
    pub metric: &'a str,
    pub probability: Option<f64>,
    pub pij_min: Option<f64>,
    pub area_unit: &'a str,
    /// Pairs of nodes farther apart than this are not joined.
    pub threshold: Option<f64>,
}

/// Makurhini's MK_Focal_nodes (landschaft's focal_nodes): for every node and distance, the graph of the nodes
/// within its search buffer (their points on the surface, the landscape their bounding box) and the node's
/// importance in it: EC, the index, dI and its fractions, and IComp (the index times dI).
pub fn focal_nodes(v: &Vector, ids: &[f64], attribute: &[f64], o: &Focal) -> Result<crate::Table, String> {
    use super::graph::{Connections, HabitatGraph, Kind, table};
    if o.metric != "IIC" && o.metric != "PC" {
        return Err(format!("the metric is IIC or PC, got {:?}", o.metric));
    }
    let n = v.len();
    let f = factor(&AREA_UNITS, o.area_unit, "area")?;
    let probability = if o.metric == "PC" && o.probability.is_none() { Some(0.5) } else { o.probability };
    let geos = Geos::new(&v.geometries)?;
    let mut xy = Vec::with_capacity(n);
    for k in 0..n {
        xy.push(geos.point_on_surface(k)?);
    }
    let bounds: Vec<[f64; 4]> = v.geometries.iter().map(bounds).collect();
    // the nodes within each buffer of each node
    let mut around: Vec<(f64, Vec<Vec<usize>>)> = Vec::new();
    for &sb in o.buffers {
        if around.iter().any(|(b, _)| *b == sb) {
            continue;
        }
        let buffered = Geos::new(&terraflux_gdal::buffer(&v.geometries, sb, 30)?)?;
        let members = (0..n).map(|i| (0..n).filter(|&j| buffered.intersects_other(i, &geos, j)).collect()).collect();
        around.push((sb, members));
    }
    let widest = o.buffers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let of = |sb: f64| &around.iter().find(|(b, _)| *b == sb).unwrap().1;
    let m = o.metric;
    let names = [
        format!("EC({m})"),
        m.to_string(),
        format!("d{m}"),
        format!("d{m}intra"),
        format!("d{m}flux"),
        format!("d{m}connector"),
    ];
    let mut cols: Vec<(String, Vec<f64>)> = ["id", "distance", "attribute", "n_nodes"]
        .iter()
        .map(|c| c.to_string())
        .chain(names.iter().cloned())
        .chain(["IComp".to_string()])
        .map(|c| (c, Vec::new()))
        .collect();
    for (k, &d) in o.distances.iter().enumerate() {
        let sb = o.buffers[k];
        for i in 0..n {
            let mut members = of(sb)[i].clone();
            if sb != widest {
                let wide = &of(widest)[i];
                members.retain(|j| wide.contains(j));
            }
            let mut row = vec![ids[i], d, attribute[i], members.len() as f64];
            if members.len() <= 2 {
                row.extend([f64::NAN; 7]);
            } else {
                let mm = members.len();
                let (sx, sy): (Vec<f64>, Vec<f64>) = members.iter().map(|&j| (xy[j][0], xy[j][1])).unzip();
                let e = ffi::conn_point_distances(&sx, &sy, f64::INFINITY).map_err(|e| e.to_string())?;
                let mut dm = vec![0.0; mm * mm];
                for t in 0..e.i.len() {
                    let (a, b) = (e.i[t] as usize, e.j[t] as usize);
                    dm[a * mm + b] = e.distance[t];
                    dm[b * mm + a] = e.distance[t];
                }
                if let Some(t) = o.threshold {
                    dm.iter_mut().filter(|x| **x > t).for_each(|x| *x = f64::NAN);
                }
                let bb = members.iter().fold(
                    [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY],
                    |a, &j| {
                        let b = bounds[j];
                        [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
                    },
                );
                let (x0, y0, x1, y1) = (bb[0], bb[1], bb[2], bb[3]);
                let ring = vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]];
                let la = gdal_areas(&[Geometry::Polygon(vec![ring])])?[0] * f;
                let attr: Vec<f64> = members.iter().map(|&j| attribute[j]).collect();
                let g = HabitatGraph::new(attr, Connections::Matrix(dm), None, Kind::Distance, Some(la), false, None)?;
                let metric = vec![m.to_string()];
                let overall = g.overall(Some(&metric), &[Some(d)], probability, o.pij_min)?;
                let get =
                    |t: &crate::Table, r: usize, c: &str| t.rows[r][t.columns.iter().position(|x| x == c).unwrap()];
                let focal = members.iter().position(|&j| j == i).unwrap();
                let imp = g.node_importance(Some(&metric), &[Some(d)], probability, o.pij_min)?;
                let index = get(&overall, 0, m);
                let di = get(&imp, focal, &names[2]);
                row.push(get(&overall, 0, &names[0]));
                row.push(index);
                for c in &names[2..] {
                    row.push(get(&imp, focal, c));
                }
                row.push(index * di);
            }
            for (c, x) in cols.iter_mut().zip(row) {
                c.1.push(x);
            }
        }
    }
    Ok(table(cols))
}
