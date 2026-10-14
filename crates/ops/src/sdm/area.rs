//! The area accessible to a species (landschaft's accessible_area, the "M" of the BAM diagram): the land within
//! a distance of its records (geodesic circles on WGS84, merged), their convex hull or the regions that hold them
//! (widened by a distance), cut at the antimeridian and folded back into [-180, 180], within a mask. The circles
//! are GeographicLib's (its C library, as PROJ's geodesics); the unions and intersections GEOS's through GDAL.

use terraflux_ls_sys::ffi;
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use super::points::{Occurrences, to_crs};

/// The settings of accessible_area.
pub struct Settings<'a> {
    /// buffer, convex_hull or regions.
    pub method: &'a str,
    /// The radius in metres.
    pub distance: Option<f64>,
    pub regions: Option<&'a Vector>,
    pub within: Option<&'a Vector>,
    /// The vertices of each circle.
    pub resolution: usize,
}

/// Features in lon/lat.
fn lonlat(v: &Vector) -> Result<Vector, String> {
    match &v.crs {
        Some(c) if !terraflux_gdal::same_crs(c, "EPSG:4326")? => terraflux_gdal::reproject_vector(v, "EPSG:4326"),
        _ => Ok(v.clone()),
    }
}

/// The polygons of the points at `distance` metres of each point (pyproj's Geod.fwd at azimuths 0, 360/n, ...),
/// unwrapped around their centres.
fn geodesic_circles(pts: &[Xy], distance: f64, n: usize) -> Result<Vec<Geometry>, String> {
    let step = 360.0 / n as f64;
    let (mut lon, mut lat, mut az) = (Vec::new(), Vec::new(), Vec::new());
    for p in pts {
        for k in 0..n {
            lon.push(p[0]);
            lat.push(p[1]);
            az.push(k as f64 * step);
        }
    }
    let s = vec![distance; lon.len()];
    let xy = ffi::sdm_geod_direct(&lon, &lat, &az, &s).map_err(|e| e.to_string())?;
    Ok(pts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut ring: Vec<Xy> = (0..n)
                .map(|k| {
                    let j = i * n + k;
                    [p[0] + (xy[2 * j] - p[0] + 180.0).rem_euclid(360.0) - 180.0, xy[2 * j + 1]]
                })
                .collect();
            ring.push(ring[0]);
            Geometry::Polygon(vec![ring])
        })
        .collect())
}

/// The distinct points, sorted (numpy's unique by rows).
fn unique(mut pts: Vec<Xy>) -> Vec<Xy> {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup();
    pts
}

/// GEOS's Densifier: points added along each segment longer than `tol`, equally spaced.
fn densify(line: &[Xy], tol: f64, out: &mut Vec<Xy>) {
    for w in line.windows(2) {
        let (p0, p1) = (w[0], w[1]);
        out.push(p0);
        let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len <= tol {
            continue;
        }
        let count = (len / tol).ceil() as usize;
        let seg = len / count as f64;
        for j in 1..count {
            let f = (j as f64 * seg) / len;
            out.push([p0[0] + f * dx, p0[1] + f * dy]);
        }
    }
    if let Some(&last) = line.last() {
        out.push(last);
    }
}

/// The rings of polygons.
fn rings(g: &Geometry) -> Vec<&Vec<Xy>> {
    match g {
        Geometry::Polygon(r) => r.iter().collect(),
        Geometry::MultiPolygon(p) => p.iter().flatten().collect(),
        _ => Vec::new(),
    }
}

/// The area widened by `distance` metres: geodesic circles on the vertices of its outline (segments of at most
/// 0.25 degrees), merged with it.
fn widen(area: &Geometry, distance: f64, resolution: usize) -> Result<Geometry, String> {
    let mut pts = Vec::new();
    for r in rings(area) {
        densify(r, 0.25, &mut pts);
    }
    let circles = geodesic_circles(&unique(pts), distance, resolution)?;
    let mut all: Vec<&Geometry> = vec![area];
    all.extend(circles.iter());
    terraflux_gdal::union(&all)
}

fn shifted(g: &Geometry, dx: f64) -> Geometry {
    let shift = |r: &Vec<Xy>| r.iter().map(|p| [p[0] + dx, p[1]]).collect::<Vec<Xy>>();
    match g {
        Geometry::Polygon(r) => Geometry::Polygon(r.iter().map(shift).collect()),
        Geometry::MultiPolygon(p) => Geometry::MultiPolygon(p.iter().map(|r| r.iter().map(shift).collect()).collect()),
        g => g.clone(),
    }
}

fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
    Geometry::Polygon(vec![vec![[x1, y0], [x1, y1], [x0, y1], [x0, y0], [x1, y0]]])
}

fn is_empty(g: &Geometry) -> bool {
    match g {
        Geometry::Empty => true,
        Geometry::Polygon(r) => r.is_empty(),
        Geometry::MultiPolygon(p) => p.is_empty(),
        _ => true,
    }
}

/// A lon/lat geometry cut at the antimeridian and folded back into [-180, 180].
fn wrap(g: &Geometry) -> Result<Geometry, String> {
    let world = rectangle(-180.0, -90.0, 180.0, 90.0);
    let mut parts = terraflux_gdal::intersection(std::slice::from_ref(g), &world)?;
    for shift in [-360.0, 360.0] {
        let out = terraflux_gdal::intersection(
            std::slice::from_ref(g),
            &rectangle(-180.0 - shift, -90.0, 180.0 - shift, 90.0),
        )?
        .remove(0);
        if !is_empty(&out) {
            parts.push(shifted(&out, shift));
        }
    }
    let refs: Vec<&Geometry> = parts.iter().filter(|p| !is_empty(p)).collect();
    terraflux_gdal::union(&refs)
}

/// Whether a point may lie in a geometry (its box holds it).
fn in_box(g: &Geometry, p: Xy) -> bool {
    let mut any = false;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for r in rings(g) {
        for q in r {
            any = true;
            (x0, y0, x1, y1) = (x0.min(q[0]), y0.min(q[1]), x1.max(q[0]), y1.max(q[1]));
        }
    }
    any && p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1
}

/// The accessible area: one (multi)polygon in lon/lat with the method and the distance.
pub fn accessible_area(occ: &Occurrences, s: &Settings, x: Option<&str>, y: Option<&str>) -> Result<Vector, String> {
    if !matches!(s.method, "buffer" | "convex_hull" | "regions") {
        return Err(format!("method must be buffer, convex_hull or regions, not {:?}", s.method));
    }
    if let Some(d) = s.distance
        && (d.is_nan() || d <= 0.0)
    {
        return Err("distance must be positive (metres)".into());
    }
    if s.resolution < 3 {
        return Err("resolution: 3 vertices or more".into());
    }
    let (mut lon, mut lat, crs) = occ.coordinates(x, y)?;
    to_crs(&mut lon, &mut lat, crs.as_deref(), Some("EPSG:4326"))?;
    let pts: Vec<Xy> =
        lon.iter().zip(&lat).filter(|(a, b)| a.is_finite() && b.is_finite()).map(|(&a, &b)| [a, b]).collect();
    if pts.is_empty() {
        return Err("no record with coordinates".into());
    }
    let area = match s.method {
        "buffer" => {
            let d = s.distance.ok_or("method buffer needs a distance (metres)")?;
            let circles = geodesic_circles(&unique(pts), d, s.resolution)?;
            terraflux_gdal::union(&circles.iter().collect::<Vec<_>>())?
        }
        method => {
            let area = if method == "convex_hull" {
                match terraflux_gdal::convex_hull(&Geometry::MultiPoint(pts))? {
                    g @ Geometry::Polygon(_) => g,
                    _ => {
                        return Err("the convex hull needs three records not on one line; use method buffer".into());
                    }
                }
            } else {
                let regions = lonlat(s.regions.ok_or("method regions needs regions (polygons)")?)?;
                let geos = terraflux_gdal::Geos::new(&regions.geometries)?;
                let mut hit = vec![false; regions.len()];
                for p in &pts {
                    for (k, g) in regions.geometries.iter().enumerate() {
                        if !hit[k] && in_box(g, *p) && geos.contains_point(k, *p)? {
                            hit[k] = true;
                        }
                    }
                }
                let chosen: Vec<&Geometry> =
                    (0..hit.len()).filter(|&k| hit[k]).map(|k| &regions.geometries[k]).collect();
                if chosen.is_empty() {
                    return Err("no record falls in any of the regions".into());
                }
                terraflux_gdal::union(&chosen)?
            };
            match s.distance {
                Some(d) => widen(&area, d, s.resolution)?,
                None => area,
            }
        }
    };
    let mut area = wrap(&terraflux_gdal::make_valid(&area)?)?;
    if let Some(w) = s.within {
        let w = lonlat(w)?;
        let clip = terraflux_gdal::union(&w.geometries.iter().collect::<Vec<_>>())?;
        area = terraflux_gdal::intersection(std::slice::from_ref(&area), &clip)?.remove(0);
    }
    if is_empty(&area) {
        return Err("the accessible area is empty".into());
    }
    Ok(Vector {
        geometries: vec![area],
        attributes: vec![
            ("method".into(), Column::Texts(vec![s.method.to_string()])),
            ("distance".into(), Column::Numbers(vec![s.distance.unwrap_or(f64::NAN)])),
        ],
        crs: Some("EPSG:4326".into()),
    })
}
