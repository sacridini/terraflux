//! Connectivity on a grid of cells over a region (Makurhini's make_grid, MK_Connect_grid and MK_dECA_grid, as
//! landschaft ports them): hexagons (sf's st_make_grid) or squares over a region, ProtConn or PC per cell, and
//! the change of ECA per cell between times.

use terraflux_gdal::Geos;
use terraflux_ls_sys::ffi;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::graph::{Connections, HabitatGraph, Kind, comparison, eca_change};
use super::protconn::{INDICATORS, Settings, protconn};
use super::raster::{AREA_UNITS, factor};
use super::vector::{centroids, gdal_areas};

const QUAD_SEGS: i32 = 30;

/// R's seq(start, stop, by) as landschaft's _r_seq.
fn r_seq(start: f64, stop: f64, by: f64) -> Vec<f64> {
    let n = ((stop - start) / by + 1e-10) as i64;
    (0..=n).map(|k| (start + k as f64 * by).min(stop)).collect()
}

/// sf's st_make_grid hexagons (flat-topped, cellsize their width across flats) over a bounding box.
fn hexagons(xmin: f64, ymin: f64, xmax: f64, ymax: f64, cellsize: f64) -> Vec<Geometry> {
    let dx = cellsize / 3f64.sqrt();
    let dy = 3f64.sqrt() * dx / 2.0;
    let (xlim, ylim) = ((ymin, ymax), (xmin, xmax));
    let x0: Vec<f64> = r_seq(xlim.0 - dx, xlim.1 + 2.0 * dx, dx).into_iter().map(|v| v + 0.0).collect();
    let y0: Vec<f64> = r_seq(ylim.0 - 2.0 * dy, ylim.1 + 2.0 * dy, dy).into_iter().map(|v| v + 0.0).collect();
    let (nx, ny) = (x0.len(), y0.len());
    let len = nx * ny;
    let y: Vec<f64> = y0.iter().flat_map(|v| std::iter::repeat_n(*v, nx)).collect();
    let both: Vec<f64> = x0.iter().copied().chain(x0.iter().map(|v| v + dx / 2.0)).collect();
    let x: Vec<f64> = (0..len).map(|k| both[k % both.len()]).collect();
    let pattern: Vec<i64> = (1..2 * nx as i64).step_by(2).chain((2..=2 * nx as i64).step_by(2)).collect();
    let xi: Vec<i64> = (0..len).map(|k| pattern[k % pattern.len()]).collect();
    let yi: Vec<i64> = (1..=ny as i64).flat_map(|v| std::iter::repeat_n(v, nx)).collect();
    let xmax_i = *xi.iter().max().unwrap();
    let ymax_i = *yi.iter().max().unwrap();
    let offsets = [[-2, 0], [-1, -1], [1, -1], [2, 0], [1, 1], [-1, 1], [-2, 0]];
    let mut out = Vec::new();
    for k in 0..len {
        let keep = xi[k] >= 3 && xi[k] <= xmax_i - 2 && xi[k] % 3 == 0 && yi[k] > 1 && yi[k] < ymax_i;
        if !keep {
            continue;
        }
        let ring: Vec<[f64; 2]> = offsets
            .iter()
            .map(|o| {
                let (mx, my) = (xi[k] + o[0], yi[k] + o[1]);
                let idx = ((mx - 1).div_euclid(2) + (my - 1) * nx as i64) as usize;
                [y[idx], x[idx]]
            })
            .collect();
        out.push(Geometry::Polygon(vec![ring]));
    }
    out
}

/// Squares of side `width` over a bounding box grown by half a side.
fn squares(xmin: f64, ymin: f64, xmax: f64, ymax: f64, width: f64) -> Vec<Geometry> {
    let (xmin, xmax) = (xmin - width / 2.0, xmax + width / 2.0);
    let (ymin, ymax) = (ymin - width / 2.0, ymax + width / 2.0);
    let nc = 1.max(((xmax - xmin) / width).round_ties_even() as i64);
    let nr = 1.max(((ymax - ymin) / width).round_ties_even() as i64);
    let xmax = xmin + nc as f64 * width;
    let ymin = ymax - nr as f64 * width;
    let xres = (xmax - xmin) / nc as f64;
    let yres = (ymax - ymin) / nr as f64;
    let mut out = Vec::new();
    for r in 0..nr {
        for c in 0..nc {
            let cx = (c as f64 + 0.5) * xres + xmin;
            let cy = ymax - (r as f64 + 0.5) * yres;
            let (x0, x1, y0, y1) = (cx - xres / 2.0, cx + xres / 2.0, cy - yres / 2.0, cy + yres / 2.0);
            out.push(Geometry::Polygon(vec![vec![[x0, y1], [x1, y1], [x1, y0], [x0, y0], [x0, y1]]]));
        }
    }
    out
}

fn total_bounds(geoms: &[Geometry]) -> [f64; 4] {
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let mut add = |c: &[f64; 2]| {
        b[0] = b[0].min(c[0]);
        b[1] = b[1].min(c[1]);
        b[2] = b[2].max(c[0]);
        b[3] = b[3].max(c[1]);
    };
    for g in geoms {
        match g {
            Geometry::Empty => {}
            Geometry::Point(p) => add(p),
            Geometry::LineString(l) | Geometry::MultiPoint(l) => l.iter().for_each(&mut add),
            Geometry::Polygon(rs) | Geometry::MultiLineString(rs) => rs.iter().flatten().for_each(&mut add),
            Geometry::MultiPolygon(ps) => ps.iter().flatten().flatten().for_each(&mut add),
        }
    }
    b
}

fn is_empty(g: &Geometry) -> bool {
    match g {
        Geometry::Empty => true,
        Geometry::MultiPoint(v) => v.is_empty(),
        Geometry::MultiLineString(v) | Geometry::Polygon(v) => v.is_empty(),
        Geometry::MultiPolygon(v) => v.is_empty(),
        Geometry::LineString(v) => v.is_empty(),
        Geometry::Point(_) => false,
    }
}

fn explode(g: &Geometry) -> Vec<Geometry> {
    match g {
        Geometry::Polygon(p) if !p.is_empty() => vec![Geometry::Polygon(p.clone())],
        Geometry::MultiPolygon(ps) => {
            ps.iter().filter(|p| !p.is_empty()).map(|p| Geometry::Polygon(p.clone())).collect()
        }
        _ => Vec::new(),
    }
}

fn union_of(geoms: &[Geometry]) -> Result<Geometry, String> {
    let refs: Vec<&Geometry> =
        geoms.iter().filter(|g| matches!(g, Geometry::Polygon(_) | Geometry::MultiPolygon(_))).collect();
    terraflux_gdal::union(&refs)
}

/// The geometries that intersect a geometry.
fn intersecting(geoms: &[Geometry], other: &Geometry) -> Result<Vec<Geometry>, String> {
    let g = Geos::new(geoms)?;
    let mut out = Vec::new();
    for (k, geom) in geoms.iter().enumerate() {
        if g.intersects_with(k, other)? {
            out.push(geom.clone());
        }
    }
    Ok(out)
}

/// Makurhini's make_grid: hexagons (default) or squares of `cell_area` over the region that intersect it,
/// clipped to it, or only those whole within it (`grid_boundary`).
pub fn make_grid(
    region: &[Geometry],
    cell_area: f64,
    hexagonal: bool,
    clip: bool,
    grid_boundary: bool,
) -> Result<Vec<Geometry>, String> {
    if clip && grid_boundary {
        return Err("choose clip or grid_boundary, not both".into());
    }
    let union = union_of(region)?;
    let [xmin, ymin, xmax, ymax] = total_bounds(region);
    let cells = if hexagonal {
        hexagons(xmin, ymin, xmax, ymax, (2.0 * cell_area / 3f64.sqrt()).sqrt())
    } else {
        squares(xmin, ymin, xmax, ymax, cell_area.sqrt())
    };
    let cells = intersecting(&cells, &union)?;
    if clip {
        return Ok(terraflux_gdal::intersection(&cells, &union)?.into_iter().filter(|g| !is_empty(g)).collect());
    }
    if grid_boundary && !cells.is_empty() {
        let full = gdal_areas(&cells[..1])?[0].round_ties_even();
        let clean = terraflux_gdal::buffer(&[union], 0.0, 8)?.remove(0);
        let inside = terraflux_gdal::intersection(&cells, &clean)?;
        let areas = gdal_areas(&inside)?;
        return Ok(inside
            .into_iter()
            .zip(areas)
            .filter(|(_, a)| a.round_ties_even() >= full)
            .map(|(g, _)| g)
            .collect());
    }
    Ok(cells)
}

/// The grid as features: OBJECTID 1, 2... and the cells.
pub fn grid_vector(cells: Vec<Geometry>, crs: Option<String>) -> Vector {
    let n = cells.len();
    Vector {
        geometries: cells,
        attributes: vec![("OBJECTID".into(), Column::Numbers((1..=n).map(|k| k as f64).collect()))],
        crs,
    }
}

/// The PC of a cell's dissolved protected areas (landschaft's _pc_cell): Protected_surface, LA, ECA,
/// ECA_Normalized, PC.
fn pc_cell(
    nodes: &[Geometry],
    la: f64,
    distance: f64,
    probability: f64,
    f: f64,
    single: bool,
) -> Result<[f64; 5], String> {
    if nodes.is_empty() {
        return Ok([0.0, la, f64::NAN, f64::NAN, f64::NAN]);
    }
    let areas: Vec<f64> = gdal_areas(nodes)?.into_iter().map(|a| a * f).collect();
    if areas.len() == 1 || single {
        let s = ffi::r_sum(&areas);
        let a = if la < s { la } else { s };
        let full = a >= la;
        return Ok([
            a,
            la,
            if full { la } else { a },
            if full { 100.0 } else { a * 100.0 / la },
            if full { 1.0 } else { a / (la * la) },
        ]);
    }
    let g = Geos::new(nodes)?;
    let n = nodes.len();
    let mut pairs = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            pairs.push([(i + 1) as f64, (j + 1) as f64, g.distance(i, j)]);
        }
    }
    let hg = HabitatGraph::new(areas.clone(), Connections::Pairs(pairs), None, Kind::Distance, None, false, None)?;
    let pc = vec!["PC".to_string()];
    let t = hg.overall(Some(&pc), &[Some(distance)], Some(probability), Some(0.0))?;
    let num = t.rows[0][t.columns.iter().position(|c| c == "PCnum").unwrap()];
    let eca = num.sqrt();
    Ok([ffi::r_sum(&areas), la, eca, eca * 100.0 / la, num / (la * la)])
}

/// The settings of connectivity_grid.
pub struct GridSettings<'a> {
    pub distance: f64,
    pub protconn_index: bool,
    pub probability: f64,
    pub transboundary: Option<f64>,
    pub area_unit: &'a str,
    pub pij_min: Option<f64>,
}

/// Makurhini's MK_Connect_grid (landschaft's connectivity_grid): per cell of the grid, ProtConn's indicators
/// (but ProtUnconn_Design and ProtConn_Bound) or the PC of its protected areas.
pub fn connectivity_grid(
    protected: &[Geometry],
    region: &[Geometry],
    grid: &Vector,
    s: &GridSettings,
) -> Result<Vector, String> {
    let f = factor(&AREA_UNITS, s.area_unit, "area")?;
    let union = union_of(region)?;
    let zone = match s.transboundary.filter(|t| *t != 0.0) {
        None => union,
        Some(t) => terraflux_gdal::buffer(&[union], t, QUAD_SEGS)?.remove(0),
    };
    let pre = intersecting(protected, &zone)?;
    let names: Vec<&str> = if s.protconn_index {
        INDICATORS.iter().copied().filter(|c| *c != "ProtUnconn_Design" && *c != "ProtConn_Bound").collect()
    } else {
        vec!["Protected_surface", "LA", "ECA", "ECA_Normalized", "PC"]
    };
    let mut cols: Vec<Vec<f64>> = vec![Vec::new(); names.len()];
    for cell in &grid.geometries {
        let la = gdal_areas(std::slice::from_ref(cell))?[0] * f;
        let row: Vec<f64> = if s.protconn_index {
            let settings = Settings {
                distances: &[s.distance],
                probability: Some(s.probability),
                transboundary: s.transboundary,
                transboundary_type: "nodes",
                bound: false,
                area_unit: s.area_unit,
                distance_type: "edge",
                cost: None,
                weight: 1.0,
                preselect: false,
                pij_min: s.pij_min,
            };
            let (rows, _) = protconn(&pre, std::slice::from_ref(cell), &settings, false)?;
            let full = &rows[0][1..];
            let mut row: Vec<f64> = INDICATORS
                .iter()
                .zip(full)
                .filter(|(c, _)| **c != "ProtUnconn_Design" && **c != "ProtConn_Bound")
                .map(|(_, v)| *v)
                .collect();
            if row[names.iter().position(|c| *c == "Protected_surface").unwrap()] == 0.0 {
                for c in ["ECA", "PC", "ProtConn", "ProtUnconn"] {
                    row[names.iter().position(|x| *x == c).unwrap()] = f64::NAN;
                }
            }
            row
        } else {
            let y1 = intersecting(&pre, cell)?;
            let clipped = terraflux_gdal::intersection(&y1, cell)?;
            let dissolved = if clipped.is_empty() {
                Vec::new()
            } else {
                let u = union_of(&clipped)?;
                explode(&terraflux_gdal::buffer(&[u], 0.0, 8)?.remove(0))
            };
            pc_cell(&dissolved, la, s.distance, s.probability, f, y1.len() == 1)?.to_vec()
        };
        for (c, v) in cols.iter_mut().zip(row) {
            c.push(v);
        }
    }
    let mut out = grid.clone();
    for (name, values) in names.iter().zip(cols) {
        out.attributes.push((name.to_string(), Column::Numbers(values)));
    }
    Ok(out)
}

/// A habitat graph of features as landschaft's from_vector by default (areas, centroid distances).
fn graph_of(geoms: &[Geometry], f: f64) -> Result<HabitatGraph, String> {
    let areas = gdal_areas(geoms)?;
    let attr: Vec<f64> = areas.iter().map(|a| a * f).collect();
    let ids: Vec<f64> = (1..=geoms.len()).map(|k| k as f64).collect();
    let (x, y) = centroids(geoms)?;
    let r = ffi::conn_point_distances(&x, &y, f64::INFINITY).map_err(|e| e.to_string())?;
    let pairs = (0..r.i.len()).map(|k| [ids[r.i[k] as usize], ids[r.j[k] as usize], r.distance[k]]).collect();
    let mut g = HabitatGraph::new(attr, Connections::Pairs(pairs), Some(ids), Kind::Distance, None, false, None)?;
    g.habitat_area = Some(ffi::r_sum(&areas) * f);
    Ok(g)
}

/// The settings of eca_change_grid.
pub struct ChangeSettings<'a> {
    pub distance: f64,
    pub metric: &'a str,
    pub probability: Option<f64>,
    pub pij_min: Option<f64>,
    pub names: &'a [String],
    pub area_unit: &'a str,
}

/// Makurhini's MK_dECA_grid (landschaft's eca_change_grid): per cell and pair of consecutive times, the ECA of
/// each and their change (the habitat clipped to the cell, its graph per time).
pub fn eca_change_grid(times: &[&Vector], grid: &Vector, s: &ChangeSettings) -> Result<Vector, String> {
    if times.len() < 2 {
        return Err("give two or more times".into());
    }
    let f = factor(&AREA_UNITS, s.area_unit, "area")?;
    let metric = s.metric.to_uppercase();
    let probability = if metric == "PC" && s.probability.is_none() { Some(0.5) } else { s.probability };
    let ec = format!("EC({metric})");
    // (time pair, cell): the numbers and the readings
    let mut rows: Vec<(usize, usize, [f64; 7], String, String)> = Vec::new();
    for (k, cell) in grid.geometries.iter().enumerate() {
        let la = gdal_areas(std::slice::from_ref(cell))?[0] * f;
        let mut pieces: Vec<Option<Vec<Geometry>>> = Vec::new();
        for t in times {
            let near = intersecting(&t.geometries, cell)?;
            let cut = terraflux_gdal::intersection(&near, cell)?;
            let parts: Vec<Geometry> = cut.iter().flat_map(explode).collect();
            pieces.push(if parts.is_empty() { None } else { Some(parts) });
        }
        for i in 0..times.len() - 1 {
            let (a, b) = (&pieces[i], &pieces[i + 1]);
            let (nums, c, h): ([f64; 7], &str, &str) = match (a, b) {
                (Some(a), Some(b)) => {
                    let (ga, gb) = (graph_of(a, f)?, graph_of(b, f)?);
                    let (t, readings) =
                        eca_change(&[&ga, &gb], &metric, &[Some(s.distance)], la, probability, s.pij_min)?;
                    let col = |name: &str, r: usize| t.rows[r][t.columns.iter().position(|c| c == name).unwrap()];
                    (
                        [
                            col("ECA", 0),
                            col("ECA", 1),
                            col("ECA_LA", 0),
                            col("ECA_LA", 1),
                            col("dA", 1),
                            col("dECA", 1),
                            col("rECA", 1),
                        ],
                        readings[1].0,
                        readings[1].1,
                    )
                }
                (Some(a), None) => {
                    let eca_i = if a.len() > 2 {
                        let mut g = graph_of(a, f)?;
                        g.landscape_area = Some(la);
                        let m = vec![metric.clone()];
                        let t = g.overall(Some(&m), &[Some(s.distance)], probability, s.pij_min)?;
                        t.rows[0][t.columns.iter().position(|c| *c == ec).unwrap()]
                    } else {
                        let areas: Vec<f64> = gdal_areas(a)?.into_iter().map(|x| x * f).collect();
                        ffi::r_sum(&areas)
                    };
                    let (c, h) = comparison(-100.0, -100.0);
                    ([eca_i, 0.0, eca_i * 100.0 / la, 0.0, -100.0, -100.0, 1.0], c, h)
                }
                (None, Some(_)) => {
                    let (c, h) = comparison(100.0, 100.0);
                    ([0.0, la, 0.0, 100.0, 100.0, 100.0, 1.0], c, h)
                }
                (None, None) => ([f64::NAN; 7], "NA", "NA"),
            };
            rows.push((i, k, nums, c.to_string(), h.to_string()));
        }
    }
    rows.sort_by_key(|r| (r.0, r.1));
    let mut out = Vector { geometries: Vec::new(), attributes: Vec::new(), crs: grid.crs.clone() };
    for (name, col) in &grid.attributes {
        let pick = |r: &(usize, usize, [f64; 7], String, String)| r.1;
        let values = match col {
            Column::Numbers(v) => Column::Numbers(rows.iter().map(|r| v[pick(r)]).collect()),
            Column::Texts(v) => Column::Texts(rows.iter().map(|r| v[pick(r)].clone()).collect()),
        };
        out.attributes.push((name.clone(), values));
    }
    out.attributes.push((
        "time".into(),
        Column::Texts(rows.iter().map(|r| format!("{}.{}", s.names[r.0], s.names[r.0 + 1])).collect()),
    ));
    for (j, name) in ["ECA_i", "ECA_j", "ECAnorm_i", "ECAnorm_j", "dA", "dECA", "rECA"].iter().enumerate() {
        out.attributes.push((name.to_string(), Column::Numbers(rows.iter().map(|r| r.2[j]).collect())));
    }
    out.attributes.push(("comparison".into(), Column::Texts(rows.iter().map(|r| r.3.clone()).collect())));
    out.attributes.push(("change".into(), Column::Texts(rows.iter().map(|r| r.4.clone()).collect())));
    out.geometries = rows.iter().map(|r| grid.geometries[r.1].clone()).collect();
    Ok(out)
}
