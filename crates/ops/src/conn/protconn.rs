//! Saura's ProtConn (Makurhini's MK_ProtConn, landschaft's protconn): how much of a region is protected and
//! connected. The protected areas are clipped to the region and dissolved into nodes (with the transboundary
//! ones around it, and the region's parts with `bound`); the indicators come from PC's numerator over the most
//! probable paths between them. GEOS through GDAL for the geometry; landschaft's C++ for the paths.

use terraflux_gdal::Geos;
use terraflux_ls_sys::ffi;
use terraflux_raster::vector::Geometry;

use super::graph::{Connections, HabitatGraph, Kind};
use super::raster::{AREA_UNITS, DistanceOptions, factor};
use super::vector::gdal_areas;

/// The segments of a quarter circle of landschaft's buffers.
const QUAD_SEGS: i32 = 30;

pub const INDICATORS: [&str; 20] = [
    "ECA",
    "PC",
    "LA",
    "Protected_surface",
    "Prot",
    "ProtConn",
    "ProtUnconn",
    "RelConn",
    "ProtUnconn_Design",
    "ProtConn_Bound",
    "Unprotected",
    "ProtConn_Prot",
    "ProtConn_Trans",
    "ProtConn_Unprot",
    "ProtConn_Within",
    "ProtConn_Contig",
    "ProtConn_Within_land",
    "ProtConn_Contig_land",
    "ProtConn_Unprot_land",
    "ProtConn_Trans_land",
];

fn at(name: &str) -> usize {
    INDICATORS.iter().position(|n| *n == name).unwrap()
}

fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// What a node is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeType {
    NonTransboundary,
    Transboundary,
    Region,
}

/// ProtConn's nodes: their kinds, attributes (areas; 0 transboundary and region) and geometries; the areas of
/// the clipped protected areas' parts, the clipped areas themselves, the region's area.
pub struct Nodes {
    pub types: Vec<NodeType>,
    pub attribute: Vec<f64>,
    pub geoms: Vec<Geometry>,
    pub pieces: Vec<f64>,
    pub clipped: Vec<(Geometry, f64)>,
    pub la: f64,
}

/// The polygons of a geometry, each its own.
fn explode(g: &Geometry) -> Vec<Geometry> {
    match g {
        Geometry::Polygon(p) if !p.is_empty() => vec![Geometry::Polygon(p.clone())],
        Geometry::MultiPolygon(ps) => {
            ps.iter().filter(|p| !p.is_empty()).map(|p| Geometry::Polygon(p.clone())).collect()
        }
        _ => Vec::new(),
    }
}

fn is_empty(g: &Geometry) -> bool {
    match g {
        Geometry::Empty => true,
        Geometry::MultiPoint(v) => v.is_empty(),
        Geometry::MultiLineString(v) => v.is_empty(),
        Geometry::MultiPolygon(v) => v.is_empty(),
        Geometry::LineString(v) => v.is_empty(),
        Geometry::Polygon(v) => v.is_empty(),
        Geometry::Point(_) => false,
    }
}

/// The union of the polygons of geometries, cleaned (GEOS's buffer of 0), in parts.
fn dissolve(geoms: &[Geometry]) -> Result<Vec<Geometry>, String> {
    let polys: Vec<&Geometry> =
        geoms.iter().filter(|g| matches!(g, Geometry::Polygon(_) | Geometry::MultiPolygon(_))).collect();
    if polys.is_empty() {
        return Ok(Vec::new());
    }
    let u = terraflux_gdal::union(&polys)?;
    let b = terraflux_gdal::buffer(&[u], 0.0, 8)?;
    Ok(explode(&b[0]))
}

/// landschaft's protconn_nodes.
pub fn protconn_nodes(
    protected: &[Geometry],
    region: &[Geometry],
    transboundary: Option<f64>,
    transboundary_type: &str,
    bound: bool,
    area_unit: &str,
    preselect: bool,
) -> Result<Nodes, String> {
    if transboundary_type != "nodes" && transboundary_type != "region" {
        return Err("transboundary_type must be nodes or region".into());
    }
    let f = factor(&AREA_UNITS, area_unit, "area")?;
    let tb = transboundary.filter(|t| *t != 0.0);
    let region_refs: Vec<&Geometry> = region.iter().collect();
    let region_geom = terraflux_gdal::make_valid(&terraflux_gdal::union(&region_refs)?)?;
    let la = ffi::r_sum(&gdal_areas(region)?) * f;
    let pas: Vec<Geometry> = protected.iter().map(terraflux_gdal::make_valid).collect::<Result<_, _>>()?;
    let y: Vec<Geometry> = if preselect {
        let zone = match tb {
            None => region_geom.clone(),
            Some(t) => terraflux_gdal::buffer(std::slice::from_ref(&region_geom), t, QUAD_SEGS)?.remove(0),
        };
        let g = Geos::new(&pas)?;
        let mut keep = Vec::new();
        for (k, pa) in pas.iter().enumerate() {
            if g.intersects_with(k, &zone)? {
                keep.push(pa.clone());
            }
        }
        keep
    } else {
        pas
    };
    let g = Geos::new(&y)?;
    let (mut y1, mut y2) = (Vec::new(), Vec::new());
    for (k, yk) in y.iter().enumerate() {
        if g.intersects_with(k, &region_geom)? { y1.push(yk.clone()) } else { y2.push(yk.clone()) }
    }
    let empty = Nodes { types: vec![], attribute: vec![], geoms: vec![], pieces: vec![], clipped: vec![], la };
    if y1.is_empty() {
        return Ok(empty);
    }
    let f1_all = terraflux_gdal::intersection(&y1, &region_geom)?;
    let keep: Vec<bool> = f1_all.iter().map(|g| !is_empty(g)).collect();
    let f1: Vec<Geometry> = f1_all.into_iter().filter(|g| !is_empty(g)).collect();
    let clipped: Vec<(Geometry, f64)> = f1.iter().cloned().zip(gdal_areas(&f1)?.into_iter().map(|a| a * f)).collect();
    let pieces: Vec<Geometry> = f1.iter().flat_map(explode).collect();
    let f2 = dissolve(&f1)?;
    let mut types = vec![NodeType::NonTransboundary; f2.len()];
    let mut attribute: Vec<f64> = gdal_areas(&f2)?.into_iter().map(|a| a * f).collect();
    let mut geoms = f2;
    if !y2.is_empty()
        && let Some(t) = tb
    {
        let kept: Vec<&Geometry> = y1.iter().zip(&keep).filter(|(_, k)| **k).map(|(g, _)| g).collect();
        let f3: Vec<Geometry> =
            kept.iter().map(|g| terraflux_gdal::difference(g, &region_geom)).collect::<Result<_, _>>()?;
        let mask = if transboundary_type == "nodes" {
            let all: Vec<&Geometry> = y1.iter().collect();
            terraflux_gdal::buffer(&[terraflux_gdal::union(&all)?], t, QUAD_SEGS)?.remove(0)
        } else {
            terraflux_gdal::buffer(std::slice::from_ref(&region_geom), t, QUAD_SEGS)?.remove(0)
        };
        let g2 = Geos::new(&y2)?;
        let mut near = Vec::new();
        for (k, yk) in y2.iter().enumerate() {
            if g2.intersects_with(k, &mask)? {
                near.push(yk.clone());
            }
        }
        let f4 = terraflux_gdal::intersection(&near, &mask)?;
        let parts: Vec<Geometry> = f3.into_iter().chain(f4).filter(|g| !is_empty(g)).collect();
        if !parts.is_empty() {
            let f5 = dissolve(&parts)?;
            types.extend(std::iter::repeat_n(NodeType::Transboundary, f5.len()));
            attribute.extend(std::iter::repeat_n(0.0, f5.len()));
            geoms.extend(f5);
            if bound {
                let rp = explode(&region_geom);
                types.extend(std::iter::repeat_n(NodeType::Region, rp.len()));
                attribute.extend(std::iter::repeat_n(0.0, rp.len()));
                geoms.extend(rp);
            }
        }
    }
    let pieces = gdal_areas(&pieces)?.into_iter().map(|a| a * f).collect();
    Ok(Nodes { types, attribute, geoms, pieces, clipped, la })
}

/// The probabilities of the most probable paths between the nodes kept (row-major).
fn pstar(
    dist: &[f64],
    n: usize,
    keep: &[bool],
    distance: f64,
    probability: Option<f64>,
    pij_min: Option<f64>,
) -> Result<(usize, Vec<f64>), String> {
    let idx: Vec<usize> = (0..n).filter(|&k| keep[k]).collect();
    let m = idx.len();
    if m == 0 {
        return Ok((0, Vec::new()));
    }
    let (mut i, mut j, mut value) = (Vec::new(), Vec::new(), Vec::new());
    for a in 0..m {
        for b in a + 1..m {
            i.push(a as i32);
            j.push(b as i32);
            value.push(dist[idx[a] * n + idx[b]]);
        }
    }
    let probability = probability.unwrap_or((-1.0f64).exp());
    let cut = pij_min.unwrap_or(0.0);
    if cut.is_infinite() {
        let mut eye = vec![0.0; m * m];
        (0..m).for_each(|k| eye[k * m + k] = 1.0);
        return Ok((m, eye));
    }
    let d = ffi::conn_all_pairs(&i, &j, &value, m as i32, 0, distance, probability, cut, f64::INFINITY, false, false)
        .map_err(|e| e.to_string())?;
    Ok((m, d.iter().map(|x| (-x).exp()).collect()))
}

/// landschaft's protconn_indicators: the row of INDICATORS.
#[allow(clippy::too_many_arguments)]
pub fn indicators(
    types: &[NodeType],
    a: &[f64],
    dist: &[f64],
    distance: f64,
    la: f64,
    pieces: Option<&[f64]>,
    probability: Option<f64>,
    weight: f64,
    pij_min: Option<f64>,
) -> Result<Vec<f64>, String> {
    let n = a.len();
    let mut out = vec![f64::NAN; INDICATORS.len()];
    let non: Vec<bool> = types.iter().map(|t| *t == NodeType::NonTransboundary).collect();
    let tr: Vec<bool> = types.iter().map(|t| *t == NodeType::Transboundary).collect();
    let region: Vec<bool> = types.iter().map(|t| *t == NodeType::Region).collect();
    let area1: Vec<f64> = (0..n).filter(|&k| non[k]).map(|k| a[k]).collect();
    let pieces = pieces.map_or_else(|| area1.clone(), |p| p.to_vec());
    out[at("LA")] = la;
    if n == 0 || !non.contains(&true) {
        for (k, v) in [
            ("ECA", 0.0),
            ("PC", 0.0),
            ("Protected_surface", 0.0),
            ("Prot", 0.0),
            ("ProtConn", 0.0),
            ("ProtUnconn", 0.0),
            ("Unprotected", 100.0),
        ] {
            out[at(k)] = v;
        }
        return Ok(out);
    }
    let numerator = |keep: &[bool], weighted: &[bool]| -> Result<f64, String> {
        let (m, p) = pstar(dist, n, keep, distance, probability, pij_min)?;
        let idx: Vec<usize> = (0..n).filter(|&k| keep[k]).collect();
        let b: Vec<f64> = idx.iter().map(|&k| a[k]).collect();
        let special: Vec<bool> = idx.iter().map(|&k| weighted[k]).collect();
        let mut terms = Vec::with_capacity(m * m);
        // column-major, as numpy's ravel(order="F")
        for c in 0..m {
            for r in 0..m {
                let aiaj = if special[r] || special[c] { weight } else { b[r] * b[c] };
                terms.push(aiaj * p[r * m + c]);
            }
        }
        Ok(ffi::r_sum(&terms))
    };
    let graph: Vec<bool> = region.iter().map(|r| !r).collect();
    let pcnum = numerator(&graph, &tr)?;
    let eca = pcnum.sqrt();
    let protected = ffi::r_sum(&area1);
    out[at("ECA")] = eca;
    out[at("PC")] = pcnum / (la * la);
    out[at("Protected_surface")] = protected;
    let prot = 100.0 * (protected / la);
    let protconn = 100.0 * (eca / la);
    out[at("Prot")] = prot;
    out[at("ProtConn")] = protconn;
    out[at("ProtUnconn")] = prot - protconn;
    out[at("RelConn")] = 100.0 * (protconn / prot);
    out[at("Unprotected")] = 100.0 - prot;
    let sq: Vec<f64> = area1.iter().map(|x| x * x).collect();
    let prot_prot = 100.0 * (100.0 * (ffi::r_sum(&sq).sqrt() / la) / protconn);
    out[at("ProtConn_Prot")] = prot_prot;
    let trans = if !tr.contains(&true) {
        0.0
    } else {
        let ecan = numerator(&non, &vec![false; n])?.sqrt();
        100.0 * (100.0 * ((eca - ecan) / la) / protconn)
    };
    out[at("ProtConn_Trans")] = trans;
    out[at("ProtConn_Unprot")] = py_max(100.0 - prot_prot - trans, 0.0);
    let psq: Vec<f64> = pieces.iter().map(|x| x * x).collect();
    let within1 = ffi::r_sum(&psq).sqrt() / la * 100.0;
    let within2 = (protected / ffi::r_sum(&pieces)).sqrt();
    let within = py_min(100.0 * (within1 * within2 / protconn), 100.0);
    out[at("ProtConn_Within")] = within;
    out[at("ProtConn_Contig")] = py_max(100.0 - within, 0.0);
    for part in ["Within", "Contig", "Unprot", "Trans"] {
        out[at(&format!("ProtConn_{part}_land"))] = out[at(&format!("ProtConn_{part}"))] * (protconn / 100.0);
    }
    if region.contains(&true) {
        let others: Vec<bool> = non.iter().map(|x| !x).collect();
        let design = numerator(&vec![true; n], &others)?.sqrt();
        out[at("ProtUnconn_Design")] = 100.0 * (design / la) - 100.0 * (eca / la);
        out[at("ProtConn_Bound")] = prot - out[at("ProtUnconn_Design")];
    }
    Ok(out)
}

/// The indicators of a single node of `area`.
pub fn single_node(area: f64, la: f64) -> Vec<f64> {
    let mut out = vec![f64::NAN; INDICATORS.len()];
    let full = area >= la;
    let pct = 100.0 * (area / la);
    for (k, v) in [
        ("ECA", if full { la } else { area }),
        ("PC", if full { 1.0 } else { area / (la * la) }),
        ("LA", la),
        ("Protected_surface", area),
        ("Prot", py_min(pct, 100.0)),
        ("Unprotected", py_max(100.0 - pct, 0.0)),
        ("ProtConn", py_min(pct, 100.0)),
        ("ProtUnconn", 0.0),
        ("ProtUnconn_Design", 0.0),
        ("ProtConn_Bound", py_min(pct, 100.0)),
        ("ProtConn_Prot", 100.0),
        ("ProtConn_Within", 100.0),
    ] {
        out[at(k)] = v;
    }
    out
}

/// The distances between the nodes (n x n): GEOS's between edges, or least-cost between their centroids; 0
/// between a region and the nodes it holds.
pub fn node_distances(
    types: &[NodeType],
    geoms: &[Geometry],
    distance_type: &str,
    cost: Option<&DistanceOptions>,
) -> Result<Vec<f64>, String> {
    let n = geoms.len();
    let mut m = vec![0.0; n * n];
    match distance_type {
        "edge" => {
            let g = Geos::new(geoms)?;
            for i in 0..n {
                for j in i + 1..n {
                    let d = g.distance(i, j);
                    m[i * n + j] = d;
                    m[j * n + i] = d;
                }
            }
        }
        "least-cost" => {
            let o = cost.ok_or("least-cost distances need a resistance")?;
            let ids: Vec<f64> = (1..=n).map(|k| k as f64).collect();
            let (x, y) = super::vector::centroids(geoms)?;
            let (from, to, d) = super::raster::least_cost_table(&ids, &x, &y, o)?;
            m = vec![f64::NAN; n * n];
            for k in 0..from.len() {
                let (i, j) = (from[k] as usize - 1, to[k] as usize - 1);
                m[i * n + j] = d[k];
                m[j * n + i] = d[k];
            }
            (0..n).for_each(|k| m[k * n + k] = 0.0);
        }
        t => return Err(format!("ProtConn distances are edge or least-cost, got {t:?}")),
    }
    let region: Vec<usize> = (0..n).filter(|&k| types[k] == NodeType::Region).collect();
    if region.len() == 1 {
        let r = region[0];
        for o in (0..n).filter(|&k| types[k] != NodeType::NonTransboundary) {
            m[r * n + o] = 0.0;
            m[o * n + r] = 0.0;
        }
    } else if region.len() > 1 {
        let g = Geos::new(geoms)?;
        for &r in &region {
            for o in (0..n).filter(|&k| types[k] == NodeType::NonTransboundary) {
                if g.intersects(o, r) {
                    m[r * n + o] = 0.0;
                    m[o * n + r] = 0.0;
                }
            }
        }
    }
    Ok(m)
}

/// ProtConn's settings.
pub struct Settings<'a> {
    pub distances: &'a [f64],
    pub probability: Option<f64>,
    pub transboundary: Option<f64>,
    pub transboundary_type: &'a str,
    pub bound: bool,
    pub area_unit: &'a str,
    pub distance_type: &'a str,
    pub cost: Option<DistanceOptions<'a>>,
    pub weight: f64,
    pub preselect: bool,
    pub pij_min: Option<f64>,
}

/// A row per distance: the distance and INDICATORS.
pub type Rows = Vec<Vec<f64>>;

/// A clipped protected area's importance at a distance: its geometry, area, the distance, dProt, dProtConn and
/// varProtConn.
pub type DeltaRow = (Geometry, f64, f64, f64, f64, f64);

/// Each clipped protected area's importance per distance.
pub type Delta = Vec<DeltaRow>;

/// landschaft's protconn: the indicators per distance, and with `delta` every protected area's importance.
pub fn protconn(
    protected: &[Geometry],
    region: &[Geometry],
    s: &Settings,
    delta: bool,
) -> Result<(Rows, Option<Delta>), String> {
    let prep =
        protconn_nodes(protected, region, s.transboundary, s.transboundary_type, s.bound, s.area_unit, s.preselect)?;
    let dist = if prep.geoms.is_empty() {
        Vec::new()
    } else {
        node_distances(&prep.types, &prep.geoms, s.distance_type, s.cost.as_ref())?
    };
    let non_areas: Vec<f64> = (0..prep.types.len())
        .filter(|&k| prep.types[k] == NodeType::NonTransboundary)
        .map(|k| prep.attribute[k])
        .collect();
    let single = non_areas.len() == 1 || prep.clipped.len() == 1;
    let mut rows = Vec::new();
    for &d in s.distances {
        let mut row = vec![d];
        if single {
            row.extend(single_node(py_min(ffi::r_sum(&non_areas), prep.la), prep.la));
        } else {
            row.extend(indicators(
                &prep.types,
                &prep.attribute,
                &dist,
                d,
                prep.la,
                Some(&prep.pieces),
                s.probability,
                s.weight,
                s.pij_min,
            )?);
        }
        rows.push(row);
    }
    if !delta {
        return Ok((rows, None));
    }
    Ok((rows, Some(protconn_delta(&prep, s)?)))
}

fn protconn_delta(prep: &Nodes, s: &Settings) -> Result<Delta, String> {
    let trans: Vec<usize> = (0..prep.types.len()).filter(|&k| prep.types[k] == NodeType::Transboundary).collect();
    let nc = prep.clipped.len();
    let mut types = vec![NodeType::NonTransboundary; nc];
    types.extend(std::iter::repeat_n(NodeType::Transboundary, trans.len()));
    let mut a: Vec<f64> = prep.clipped.iter().map(|c| c.1).collect();
    a.extend(std::iter::repeat_n(0.0, trans.len()));
    let geoms: Vec<Geometry> =
        prep.clipped.iter().map(|c| c.0.clone()).chain(trans.iter().map(|&k| prep.geoms[k].clone())).collect();
    let n = a.len();
    let dist = node_distances(&types, &geoms, s.distance_type, s.cost.as_ref())?;
    let total = ffi::r_sum(&a);
    let pc = vec!["PC".to_string()];
    let mut out = Vec::new();
    for &d in s.distances {
        let mut pairs = Vec::new();
        for i in 0..n {
            for j in i + 1..n {
                pairs.push([(i + 1) as f64, (j + 1) as f64, dist[i * n + j]]);
            }
        }
        let g = HabitatGraph::new(a.clone(), Connections::Pairs(pairs), None, Kind::Distance, None, false, None)?;
        let p_imp = Some(s.probability.unwrap_or(0.5));
        let imp = g.node_importance(Some(&pc), &[Some(d)], p_imp, Some(0.0))?;
        let col = imp.columns.iter().position(|c| c == "dPC").unwrap();
        let p_all = Some(match s.probability {
            Some(p) if p != 0.0 => p,
            _ => 0.5,
        });
        let overall = g.overall(Some(&pc), &[Some(d)], p_all, Some(0.0))?;
        let num = overall.rows[0][overall.columns.iter().position(|c| c == "PCnum").unwrap()];
        let protconn1 = 100.0 * num.sqrt() / prep.la;
        for (k, clipped) in prep.clipped.iter().enumerate() {
            let x = 1.0 - imp.rows[k][col] / 100.0;
            let ratio = (if x < 0.0 { 0.0 } else { x }).sqrt();
            out.push((
                clipped.0.clone(),
                clipped.1,
                d,
                100.0 * a[k] / total,
                100.0 * (1.0 - ratio),
                protconn1 * (1.0 - ratio),
            ));
        }
    }
    Ok(out)
}
