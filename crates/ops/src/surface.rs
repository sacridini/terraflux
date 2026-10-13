//! Surface metrics of continuous rasters (ISO 25178, as the R package geodiv computes them) through landschaft's
//! C++, geodiv's bits: amplitude (sa, sq, ssk, sku, sv, sph, smean), hybrid (sdq, sdq6, sdr), local features (sds,
//! ssc, s10z), the bearing area curve (sbi, sci, svi, sk, spk, svk) and spatial (sal, str, std, stdi, srw, srwi,
//! shw, sfd): of the whole surface (`surface.metrics`) or in a moving window around every cell
//! (`surface.window`). And the terrain of a surface (spatialEco's and terra's, through the same C++): slope and
//! aspect, TPI, TRI, VRM, SRR, dissection, curvatures, spherical standard deviation, HSP, TRASP, HLI; its trend
//! (geodiv's fitted planes) and the surface without it; its peaks and valleys, bearing area curve and areal
//! autocorrelation.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Metrics));
    r.add(Arc::new(Window));
    for op in TERRAIN {
        r.add(Arc::new(Terrain { op }));
    }
}

/// The surface metrics, in the order of landschaft's (and of its C++'s ids), with their groups and meanings.
pub const METRICS: [(&str, &str, &str); 27] = [
    ("sa", "amplitude", "average roughness"),
    ("sq", "amplitude", "root mean square roughness"),
    ("ssk", "amplitude", "skewness (adjusted)"),
    ("sku", "amplitude", "excess kurtosis"),
    ("sv", "amplitude", "maximum valley depth"),
    ("sph", "amplitude", "maximum peak height"),
    ("smean", "amplitude", "mean peak height"),
    ("sdq", "hybrid", "root mean square slope"),
    ("sdq6", "hybrid", "area root mean square slope"),
    ("sdr", "hybrid", "developed interfacial area ratio"),
    ("sds", "features", "density of summits"),
    ("ssc", "features", "mean summit curvature"),
    ("s10z", "features", "ten-point height"),
    ("sbi", "bearing", "surface bearing index"),
    ("sci", "bearing", "core fluid retention index"),
    ("svi", "bearing", "valley fluid retention index"),
    ("sk", "bearing", "core roughness depth"),
    ("spk", "bearing", "reduced peak height"),
    ("svk", "bearing", "reduced valley depth"),
    ("sal", "spatial", "autocorrelation length (fastest decay to 0.2)"),
    ("str", "spatial", "texture aspect ratio (at 0.2)"),
    ("std", "spatial", "texture direction (degrees)"),
    ("stdi", "spatial", "texture direction index"),
    ("srw", "spatial", "dominant radial wavelength"),
    ("srwi", "spatial", "radial wavelength index"),
    ("shw", "spatial", "half-power wavelength"),
    ("sfd", "spatial", "fractal dimension"),
];

/// The metrics a node asks for (`metrics`: a name or a list; default every one): their ids.
fn chosen(args: &Args) -> Result<Vec<i32>> {
    let names: Vec<String> = match args.params.get("metrics") {
        None | Some(serde_json::Value::Null) => METRICS.iter().map(|m| m.0.to_string()).collect(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| Error::msg(format!("node {:?}: metrics: {e} (a name or a list)", args.node)))?,
    };
    names
        .iter()
        .map(|n| {
            METRICS.iter().position(|m| m.0 == n.trim().to_lowercase()).map(|k| k as i32).ok_or_else(|| {
                Error::msg(format!(
                    "node {:?}: unknown surface metric {n:?} (they are: {})",
                    args.node,
                    METRICS.iter().map(|m| m.0).collect::<Vec<_>>().join(" ")
                ))
            })
        })
        .collect()
}

/// A grid's extent as geodiv takes it (xmin, xmax, ymin, ymax) and whether its coordinates are longitudes and
/// latitudes (landschaft's _crs_lonlat).
pub fn extent_of(g: &Grid) -> ([f64; 4], bool) {
    let t = g.transform;
    let (x0, x1) = (t[0], t[0] + g.width as f64 * t[1]);
    let (y0, y1) = (t[3], t[3] + g.height as f64 * t[5]);
    let lonlat = crate::prepare::crs_of(g)
        .is_some_and(|c| c.contains("longlat") || terraflux_gdal::is_geographic(&c).unwrap_or(false));
    ([x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1)], lonlat)
}

/// The metrics `ids` of a map's surface.
pub fn measure(map: &Map, ids: &[i32]) -> Result<Vec<f64>> {
    let (ny, nx) = map.shape();
    let (e, lonlat) = extent_of(&map.grid);
    Ok(ffi::surf_metrics(&map.data, nx as i32, ny as i32, &e, lonlat, ids)?)
}

struct Metrics;

impl Operator for Metrics {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "surface.metrics",
            "Surface metrics of a continuous raster (an elevation, a canopy height, an NDVI...), ISO 25178 as the R \
             package geodiv computes them, through landschaft's C++ (geodiv's bits): roughness and its \
             distribution (amplitude), slopes and the area developed (hybrid), summits (features), the bearing \
             area curve, the autocorrelation's lengths and directions and the fractal dimension (spatial). Usually \
             of a surface with its trend removed. A table of one row.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the surface (no value: missing)"))
        .output(PortSpec::new("table", Type::Table, "a column per metric"))
        .param(PortSpec::optional("metrics", Type::Any, "names (sa, sq, sku, sdr, sal...; default every one)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let ids = chosen(args)?;
        let v = measure(args.map("map")?, &ids).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let mut t = Table::new(ids.iter().map(|&k| METRICS[k as usize].0.to_string()).collect());
        t.rows.push(v);
        Ok(outputs([("table", Value::Table(t))]))
    }
}

struct Window;

impl Operator for Window {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "surface.window",
            "Surface metrics in a moving window around every cell (landschaft's surface.window_metrics through \
             its C++): the metrics of the window's surface at its centre, no value beyond the edges (and beyond \
             window / 2 cells in a circle). A map per metric, in a stack.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the surface"))
        .output(PortSpec::new("maps", Type::Stack, "a map per metric, by its name"))
        .param(PortSpec::optional("metrics", Type::Any, "names (default every one)"))
        .param(PortSpec::optional("window", Type::Integer, "the window's side in cells, odd, 3 or more (5)"))
        .param(PortSpec::optional("shape", Type::Text, "square (default) or circle").choices(["square", "circle"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let ids = chosen(args)?;
        let map = args.map("map")?;
        let size: i32 = args.param("window", 5)?;
        let shape: String = args.param("shape", "square".to_string())?;
        if shape != "square" && shape != "circle" {
            return Err(Error::msg(format!("node {:?}: shape is square or circle, not {shape:?}", args.node)));
        }
        let (ny, nx) = map.shape();
        let (e, lonlat) = extent_of(&map.grid);
        let out = ffi::surf_window(&map.data, nx as i32, ny as i32, &e, lonlat, size, shape == "circle", &ids)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let cells = nx * ny;
        let maps = Stack {
            grid: map.grid.clone(),
            keys: ids.iter().map(|&k| Key::Name(METRICS[k as usize].0.to_string())).collect(),
            layers: out.chunks(cells.max(1)).map(|c| Arc::new(c.to_vec())).collect(),
        };
        Ok(outputs([("maps", Value::Stack(maps))]))
    }
}

/// The terrain operators: their names.
const TERRAIN: [&str; 15] = [
    "terrain",
    "tpi",
    "tri",
    "vrm",
    "srr",
    "dissection",
    "curvature",
    "spherical_sd",
    "hsp",
    "trasp",
    "hli",
    "fit_plane",
    "peaks",
    "bearing",
    "aacf",
];

/// landschaft's _size: a window of odd cells, a number or [rows, columns].
fn size_of(args: &Args, name: &str, default: i32) -> Result<(i32, i32)> {
    let v = args.params.get(name).cloned().unwrap_or(serde_json::json!(default));
    let pair: Vec<f64> = match &v {
        serde_json::Value::Number(n) => vec![n.as_f64().unwrap(); 2],
        serde_json::Value::Array(a) => a.iter().filter_map(|x| x.as_f64()).collect(),
        _ => Vec::new(),
    };
    if pair.len() != 2 || pair.iter().any(|x| x.fract() != 0.0 || *x < 1.0 || (*x as i64) % 2 == 0) {
        return Err(Error::msg(format!("node {:?}: {name} must be odd numbers of cells, got {v}", args.node)));
    }
    Ok((pair[0] as i32, pair[1] as i32))
}

/// A rectangular window: rows, columns, every cell in.
fn rect((r, c): (i32, i32)) -> (i32, i32, Vec<u8>) {
    (r, c, vec![1; (r * c) as usize])
}

/// landschaft's _circle: the cells within a distance `d` (map units) of the centre.
fn circle(res: (f64, f64), d: f64) -> (i32, i32, Vec<u8>) {
    let (xr, yr) = res;
    let (nx, ny) = (1 + 2 * (d / xr).floor() as i32, 1 + 2 * (d / yr).floor() as i32);
    let (cx, cy) = (nx / 2, ny / 2);
    let mask = (0..ny)
        .flat_map(|y| (0..nx).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (dx, dy) = (f64::from(x - cx) * xr, f64::from(y - cy) * yr);
            u8::from((dx * dx + dy * dy).sqrt() <= d)
        })
        .collect();
    (ny, nx, mask)
}

/// A surface's values and grid as the C++ takes them.
struct Surf<'a> {
    z: &'a [f64],
    nx: i32,
    ny: i32,
    e: [f64; 4],
    lonlat: bool,
}

impl Surf<'_> {
    fn of(m: &Map) -> Surf<'_> {
        let (ny, nx) = m.shape();
        let (e, lonlat) = extent_of(&m.grid);
        Surf { z: &m.data, nx: nx as i32, ny: ny as i32, e, lonlat }
    }

    fn res(&self) -> (f64, f64) {
        ((self.e[1] - self.e[0]) / f64::from(self.nx), (self.e[3] - self.e[2]) / f64::from(self.ny))
    }

    fn focal(&self, z: &[f64], w: &(i32, i32, Vec<u8>), stat: i32) -> Result<Vec<f64>> {
        Ok(ffi::surf_focal(z, self.nx, self.ny, &self.e, w.0, w.1, &w.2, stat)?)
    }

    fn slope_aspect(&self) -> Result<(Vec<f64>, Vec<f64>)> {
        Ok((
            ffi::surf_terrain(self.z, self.nx, self.ny, &self.e, self.lonlat, 8, false, false)?,
            ffi::surf_terrain(self.z, self.nx, self.ny, &self.e, self.lonlat, 8, false, true)?,
        ))
    }

    /// landschaft's _window: a circle of a radius in map units (two cells at least), or a rectangle of cells.
    fn window(&self, scale: f64, win: &str) -> Result<(i32, i32, Vec<u8>)> {
        match win {
            "circle" => {
                if scale < self.res().0 * 2.0 {
                    return Err(Error::msg("the scale of a circular window must be at least two cells"));
                }
                Ok(circle(self.res(), scale))
            }
            "rectangle" => {
                if scale.fract() != 0.0 || scale < 1.0 || (scale as i64) % 2 == 0 {
                    return Err(Error::msg(format!("scale must be odd numbers of cells, got {scale}")));
                }
                Ok(rect((scale as i32, scale as i32)))
            }
            w => Err(Error::msg(format!("win must be 'rectangle' or 'circle', got {w:?}"))),
        }
    }
}

const MEAN: i32 = 0;
const SUM: i32 = 1;
const MIN: i32 = 2;
const MAX: i32 = 3;
const CURVATURES: [&str; 5] = ["planform", "profile", "total", "mcnab", "bolstad"];

/// geodiv's (and landschaft's) least points of a polynomial of an order.
fn check_points(z: &[f64], order: i32) -> Result<()> {
    let npar = ((order + 1) * (order + 2) / 2) as usize;
    let n = z.iter().filter(|v| !v.is_nan()).count();
    if n < npar {
        return Err(Error::msg(format!(
            "a polynomial of order {order} needs at least {npar} cells with a value, got {n}"
        )));
    }
    Ok(())
}

struct Terrain {
    op: &'static str,
}

impl Operator for Terrain {
    fn signature(&self) -> OpSignature {
        let name = format!("surface.{}", self.op);
        let map_out = |sig: OpSignature, about: &str| sig.output(PortSpec::new("map", Type::Map, about));
        let surface = |sig: OpSignature| sig.input(PortSpec::new("map", Type::Map, "the surface (an elevation...)"));
        let sig = |doc: &str| OpSignature::new(&name, doc, Access::Global);
        match self.op {
            "terrain" => map_out(
                surface(sig("Slope or aspect of a surface as terra::terrain (Horn's 8 or 4 neighbours), through \
                             landschaft's C++; no value on the edge cells.")),
                "the slope or aspect",
            )
            .param(PortSpec::optional("v", Type::Text, "slope (default) or aspect").choices(["slope", "aspect"]))
            .param(
                PortSpec::optional("unit", Type::Text, "degrees (default) or radians").choices(["degrees", "radians"]),
            )
            .param(PortSpec::optional("neighbors", Type::Integer, "8 (default) or 4").choices([8, 4])),
            "tpi" => map_out(
                surface(sig("The topographic position index (spatialEco's tpi): each cell's value less the mean \
                             of a window around it (a rectangle of cells, or a circle of a radius in map units).")),
                "the TPI",
            )
            .param(PortSpec::optional("scale", Type::Number, "the window: odd cells, or a circle's radius (3)"))
            .param(
                PortSpec::optional("win", Type::Text, "rectangle (default) or circle").choices(["rectangle", "circle"]),
            )
            .param(PortSpec::optional(
                "normalize",
                Type::Bool,
                "divided by the window's standard deviation",
            )),
            "tri" => map_out(
                surface(sig("The terrain ruggedness index (spatialEco's tri) in an odd window of cells.")),
                "the TRI",
            )
            .param(PortSpec::optional("s", Type::Any, "the window: odd cells, or [rows, columns] (3)")),
            "vrm" => map_out(
                surface(sig("The vector ruggedness measure (spatialEco's vrm): 1 less the length of the summed \
                             unit normals of a window over its cells.")),
                "the VRM",
            )
            .param(PortSpec::optional("s", Type::Any, "the window: odd cells, or [rows, columns] (3)")),
            "srr" => map_out(
                surface(sig("The surface relief ratio (spatialEco's srr): (mean - min) / (max - min) in a window.")),
                "the SRR",
            )
            .param(PortSpec::optional("s", Type::Any, "the window: odd cells, or [rows, columns] (5)")),
            "dissection" => map_out(
                surface(sig("Martonne's dissection (spatialEco's dissection): (z - min) / (max - min) in a window.")),
                "the dissection",
            )
            .param(PortSpec::optional("s", Type::Any, "the window: odd cells, or [rows, columns] (5)")),
            "curvature" => map_out(
                surface(sig("A curvature of a surface (spatialEco's curvature): planform, profile, total, \
                             McNab's or Bolstad's.")),
                "the curvature",
            )
            .param(
                PortSpec::optional("type", Type::Text, "planform (default), profile, total, mcnab, bolstad")
                    .choices(CURVATURES),
            ),
            "spherical_sd" => map_out(
                surface(sig("The spherical standard deviation of the surface's normals in a window (spatialEco's \
                             spherical.sd), or their variance.")),
                "the spherical standard deviation",
            )
            .param(PortSpec::optional("d", Type::Any, "the window: odd cells, or [rows, columns] (3)"))
            .param(PortSpec::optional("variance", Type::Bool, "the variance instead")),
            "hsp" => map_out(
                surface(sig("The hierarchical slope position (spatialEco's hsp): the TPI summed over windows of \
                             growing scale.")),
                "the HSP",
            )
            .param(PortSpec::optional("min_scale", Type::Number, "the first scale (3)"))
            .param(PortSpec::optional("max_scale", Type::Number, "the last scale (27)"))
            .param(PortSpec::optional("inc", Type::Number, "the scales' step (4)"))
            .param(
                PortSpec::optional("win", Type::Text, "rectangle (default) or circle").choices(["rectangle", "circle"]),
            )
            .param(PortSpec::optional("normalize", Type::Bool, "normalised TPIs")),
            "trasp" => map_out(
                surface(sig("The transformed aspect (spatialEco's trasp): (1 - cos(aspect - 30°)) / 2, from 0 \
                             (north-northeast) to 1.")),
                "the TRASP",
            ),
            "hli" => OpSignature::new(
                &name,
                "The heat load index (McCune & Keon's equations, spatialEco's hli) from the aspect and the slope; the \
                 latitude that of the map's centre unless given.",
                Access::Global,
            )
            .input(PortSpec::new("aspect", Type::Map, "the aspect"))
            .input(PortSpec::new("slope", Type::Map, "the slope"))
            .output(PortSpec::new("map", Type::Map, "the HLI"))
            .param(PortSpec::optional("latitude", Type::Number, "degrees (default: the map's centre's)"))
            .param(PortSpec::optional("direct", Type::Bool, "the direct radiation's folding (180° instead of 225°)"))
            .param(PortSpec::optional("scaled", Type::Bool, "exp of the index"))
            .param(
                PortSpec::optional("units", Type::Text, "degrees (default) or radians").choices(["degrees", "radians"]),
            )
            .param(
                PortSpec::optional("hemisphere", Type::Text, "northern (default) or southern")
                    .choices(["northern", "southern"]),
            )
            .param(PortSpec::optional("force_hemisphere", Type::Bool, "the hemisphere as given (default true)"))
            .param(
                PortSpec::optional("equation", Type::Integer, "McCune & Keon's equation 1 (default), 2 or 3")
                    .choices([1, 2, 3]),
            ),
            "fit_plane" => surface(sig(
                "The trend of a surface: geodiv's least-squares polynomial of an order (0-6), or the best of order \
                 0-3 (the residuals' smallest absolute mean); and the surface without it (remove_plane).",
            ))
            .output(PortSpec::new("plane", Type::Map, "the trend fitted"))
            .output(PortSpec::new("residuals", Type::Map, "the surface less its trend"))
            .param(PortSpec::optional("order", Type::Any, "0 to 6, or \"best\" (default)")),
            "peaks" => surface(sig(
                "The peaks (or valleys) of a surface, geodiv's findpeaks: the cells higher (lower) than their 8 \
                 neighbours; a table of their centres, values, rows and columns.",
            ))
            .output(PortSpec::new("table", Type::Table, "x, y, value, row, col"))
            .param(PortSpec::optional("valleys", Type::Bool, "the valleys instead")),
            "bearing" => surface(sig(
                "The bearing area (Abbott-Firestone) curve of a surface, geodiv's bearing_area: the share of the \
                 surface at or above each height.",
            ))
            .output(PortSpec::new("table", Type::Table, "bearing, height")),
            _ => surface(sig(
                "The areal autocorrelation function of a surface, geodiv's aacf: 1 at zero lag in the centre; on the \
                 largest square without missing cells around the centre when there are some.",
            ))
            .output(PortSpec::new("map", Type::Map, "the autocorrelation at every lag")),
        }
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let one = |grid: &Arc<Grid>, v: Vec<f64>| outputs([("map", Value::Map(Map::new(grid.clone(), v)))]);
        if self.op == "hli" {
            return hli(args).map_err(fail);
        }
        let m = args.map("map")?;
        let s = Surf::of(m);
        let r = || -> Result<Outputs> {
            Ok(match self.op {
                "terrain" => {
                    let v: String = args.param("v", "slope".to_string())?;
                    let unit: String = args.param("unit", "degrees".to_string())?;
                    let neighbors: i32 = args.param("neighbors", 8)?;
                    if v != "slope" && v != "aspect" {
                        return Err(Error::msg(format!("v must be 'slope' or 'aspect', got {v:?}")));
                    }
                    if unit != "degrees" && unit != "radians" {
                        return Err(Error::msg(format!("unit must be 'degrees' or 'radians', got {unit:?}")));
                    }
                    one(
                        &m.grid,
                        ffi::surf_terrain(
                            s.z,
                            s.nx,
                            s.ny,
                            &s.e,
                            s.lonlat,
                            neighbors,
                            unit == "degrees",
                            v == "aspect",
                        )?,
                    )
                }
                "tpi" => {
                    let w = s.window(args.param("scale", 3.0)?, &args.param("win", "rectangle".to_string())?)?;
                    one(&m.grid, ffi::surf_tpi(s.z, s.nx, s.ny, &s.e, w.0, w.1, &w.2, args.param("normalize", false)?)?)
                }
                "tri" => {
                    let (r, c) = size_of(args, "s", 3)?;
                    one(&m.grid, ffi::surf_tri(s.z, s.nx, s.ny, &s.e, r, c)?)
                }
                "vrm" => {
                    let (r, c) = size_of(args, "s", 3)?;
                    let (slp, asp) = s.slope_aspect()?;
                    let sin_s: Vec<f64> = slp.iter().map(|v| v.sin()).collect();
                    let parts: [Vec<f64>; 3] = [
                        asp.iter().zip(&sin_s).map(|(a, ss)| a.sin() * ss).collect(),
                        asp.iter().zip(&sin_s).map(|(a, ss)| a.cos() * ss).collect(),
                        slp.iter().map(|v| v.cos()).collect(),
                    ];
                    let w = rect((r, c));
                    let sums: Vec<Vec<f64>> = parts.iter().map(|p| s.focal(p, &w, SUM)).collect::<Result<_>>()?;
                    let n = f64::from(r * c).round_ties_even();
                    let v = (0..sums[0].len())
                        .map(|k| {
                            let len = (sums[0][k].powi(2) + sums[1][k].powi(2) + sums[2][k].powi(2)).sqrt();
                            1.0 - len / n
                        })
                        .collect();
                    one(&m.grid, v)
                }
                "srr" | "dissection" => {
                    let w = rect(size_of(args, "s", 5)?);
                    let (lo, hi) = (s.focal(s.z, &w, MIN)?, s.focal(s.z, &w, MAX)?);
                    let top: Vec<f64> = if self.op == "srr" { s.focal(s.z, &w, MEAN)? } else { s.z.to_vec() };
                    one(&m.grid, (0..lo.len()).map(|k| (top[k] - lo[k]) / (hi[k] - lo[k])).collect())
                }
                "curvature" => {
                    let t: String = args.param("type", "planform".to_string())?;
                    let k = CURVATURES.iter().position(|c| *c == t).ok_or_else(|| {
                        Error::msg(format!("type must be one of {}, got {t:?}", CURVATURES.join(", ")))
                    })?;
                    one(&m.grid, ffi::surf_curvature(s.z, s.nx, s.ny, &s.e, k as i32)?)
                }
                "spherical_sd" => {
                    let w = rect(size_of(args, "d", 3)?);
                    let (slp, asp) = s.slope_aspect()?;
                    let sin_s: Vec<f64> = slp.iter().map(|v| v.sin()).collect();
                    let parts: [Vec<f64>; 3] = [
                        asp.iter().zip(&sin_s).map(|(a, ss)| a.cos() * ss).collect(),
                        asp.iter().zip(&sin_s).map(|(a, ss)| a.sin() * ss).collect(),
                        slp.iter().map(|v| v.cos()).collect(),
                    ];
                    let means: Vec<Vec<f64>> = parts.iter().map(|p| s.focal(p, &w, MEAN)).collect::<Result<_>>()?;
                    let variance: bool = args.param("variance", false)?;
                    let v = (0..means[0].len())
                        .map(|k| {
                            let v = 1.0 - (means[0][k].powi(2) + means[1][k].powi(2) + means[2][k].powi(2));
                            if variance { v } else { v.sqrt() }
                        })
                        .collect();
                    one(&m.grid, v)
                }
                "hsp" => {
                    let (lo, hi, inc): (f64, f64, f64) =
                        (args.param("min_scale", 3.0)?, args.param("max_scale", 27.0)?, args.param("inc", 4.0)?);
                    let win: String = args.param("win", "rectangle".to_string())?;
                    let mut scales = Vec::new();
                    let mut v = lo;
                    while v <= hi + 1e-9 * hi.abs() {
                        scales.push(v);
                        v += inc;
                    }
                    if scales.is_empty() {
                        return Err(Error::msg("no scale between min_scale and max_scale"));
                    }
                    let ws: Vec<(i32, i32, Vec<u8>)> =
                        scales.iter().map(|&k| s.window(k, &win)).collect::<Result<_>>()?;
                    let rows: Vec<i32> = ws.iter().map(|w| w.0).collect();
                    let cols: Vec<i32> = ws.iter().map(|w| w.1).collect();
                    let masks: Vec<u8> = ws.iter().flat_map(|w| w.2.iter().copied()).collect();
                    one(
                        &m.grid,
                        ffi::surf_hsp(s.z, s.nx, s.ny, &s.e, &rows, &cols, &masks, args.param("normalize", false)?)?,
                    )
                }
                "trasp" => {
                    let asp = ffi::surf_terrain(s.z, s.nx, s.ny, &s.e, s.lonlat, 8, true, true)?;
                    let rad = std::f64::consts::PI / 180.0;
                    one(&m.grid, asp.iter().map(|a| (1.0 - (rad * (a - 30.0)).cos()) / 2.0).collect())
                }
                "fit_plane" => {
                    let order = match args.params.get("order") {
                        None | Some(serde_json::Value::Null) => None,
                        Some(serde_json::Value::String(b)) if b == "best" => None,
                        Some(v) => {
                            let k = v.as_f64().unwrap_or(f64::NAN);
                            if k.fract() != 0.0 || !(0.0..=6.0).contains(&k) {
                                return Err(Error::msg(format!("order must be an integer from 0 to 6, got {v}")));
                            }
                            Some(k as i32)
                        }
                    };
                    check_points(s.z, order.unwrap_or(3))?;
                    let plane = match order {
                        Some(k) => ffi::surf_fit_plane(s.z, s.nx, s.ny, &s.e, k)?,
                        None => ffi::surf_best_fit_plane(s.z, s.nx, s.ny, &s.e)?,
                    };
                    let residuals: Vec<f64> = s.z.iter().zip(&plane.values).map(|(z, p)| z - p).collect();
                    outputs([
                        ("plane", Value::Map(Map::new(m.grid.clone(), plane.values))),
                        ("residuals", Value::Map(Map::new(m.grid.clone(), residuals))),
                    ])
                }
                "peaks" => {
                    let cells = ffi::surf_peaks(s.z, s.nx, s.ny, &s.e, args.param("valleys", false)?)?;
                    let nx = s.nx as i64;
                    let (dx, dy) = ((s.e[1] - s.e[0]) / f64::from(s.nx), (s.e[3] - s.e[2]) / f64::from(s.ny));
                    let mut t = Table::new(["x", "y", "value", "row", "col"].map(String::from).to_vec());
                    t.rows = cells
                        .iter()
                        .map(|&c| {
                            let (row, col) = (c / nx, c % nx);
                            vec![
                                s.e[0] + (col as f64 + 0.5) * dx,
                                s.e[3] - (row as f64 + 0.5) * dy,
                                s.z[c as usize],
                                row as f64,
                                col as f64,
                            ]
                        })
                        .collect();
                    outputs([("table", Value::Table(t))])
                }
                "bearing" => {
                    let b = ffi::surf_bearing(s.z, &[])?;
                    let mut t = Table::new(vec!["bearing".into(), "height".into()]);
                    t.rows = b.bearing.iter().zip(&b.height).map(|(a, h)| vec![*a, *h]).collect();
                    outputs([("table", Value::Table(t))])
                }
                _ => {
                    let a = ffi::surf_aacf(s.z, s.nx, s.ny, &s.e, s.lonlat)?;
                    let grid = if a.extent[..] == s.e[..] && a.nx == s.nx && a.ny == s.ny {
                        m.grid.clone()
                    } else {
                        let mut g = (*m.grid).clone();
                        (g.width, g.height) = (a.nx as usize, a.ny as usize);
                        g.transform = [
                            a.extent[0],
                            (a.extent[1] - a.extent[0]) / f64::from(a.nx),
                            0.0,
                            a.extent[3],
                            0.0,
                            -(a.extent[3] - a.extent[2]) / f64::from(a.ny),
                        ];
                        Arc::new(g)
                    };
                    one(&grid, a.values)
                }
            })
        };
        r().map_err(fail)
    }
}

/// spatialEco's hli (landschaft's), from the aspect and the slope.
fn hli(args: &Args) -> Result<Outputs> {
    let (a, sl) = (args.map("aspect")?, args.map("slope")?);
    if a.data.len() != sl.data.len() {
        return Err(Error::msg("aspect and slope must have the same shape"));
    }
    let units: String = args.param("units", "degrees".to_string())?;
    let equation: i32 = args.param("equation", 1)?;
    let mut hemisphere: String = args.param("hemisphere", "northern".to_string())?;
    if units != "degrees" && units != "radians" {
        return Err(Error::msg(format!("units must be 'degrees' or 'radians', got {units:?}")));
    }
    if !(1..=3).contains(&equation) {
        return Err(Error::msg("equation must be 1, 2 or 3"));
    }
    if hemisphere != "northern" && hemisphere != "southern" {
        return Err(Error::msg(format!("hemisphere must be 'northern' or 'southern', got {hemisphere:?}")));
    }
    let latitude = match args.opt_param::<f64>("latitude")? {
        Some(l) => l,
        None => {
            let (e, lonlat) = extent_of(&a.grid);
            let crs = crate::prepare::crs_of(&a.grid)
                .ok_or_else(|| Error::msg("hli needs the latitude, or a raster with a CRS"))?;
            if lonlat {
                (e[2] + e[3]) / 2.0
            } else {
                let t = terraflux_gdal::Transform::new(&crs, "EPSG:4326").map_err(Error::msg)?;
                let (mut xs, mut ys) = (vec![(e[0] + e[1]) / 2.0], vec![(e[2] + e[3]) / 2.0]);
                t.points(&mut xs, &mut ys);
                ys[0]
            }
        }
    };
    if !args.param("force_hemisphere", true)? {
        hemisphere = if latitude <= 0.0 { "southern".into() } else { "northern".into() };
    }
    let (direct, scaled): (bool, bool) = (args.param("direct", false)?, args.param("scaled", false)?);
    let pi = std::f64::consts::PI;
    let rad = pi / 180.0;
    let northern = hemisphere == "northern";
    let l = if northern { latitude * rad } else { latitude.abs() * rad };
    let (cl, sl_) = (l.cos(), l.sin());
    let v = a
        .data
        .iter()
        .zip(sl.data.iter())
        .map(|(&asp, &s)| {
            let asp = if units == "degrees" { asp * rad } else { asp };
            let s = if units == "degrees" { s * rad } else { s };
            let aa = match (northern, direct) {
                (true, true) => pi - (asp - pi).abs(),
                (true, false) => (pi - (asp - pi * 5.0 / 4.0).abs()).abs(),
                (false, true) => (asp - pi).abs(),
                (false, false) => (pi - (asp - pi * 7.0 / 4.0).abs()).abs(),
            };
            let (cs, ss, ca, sa) = (s.cos(), s.sin(), aa.cos(), aa.sin());
            let r = match equation {
                1 => -1.467 + 1.582 * cl * cs - 1.5 * ca * ss * sl_ - 0.262 * sl_ * ss + 0.607 * sa * ss,
                2 => -1.236 + 1.35 * cl * cs - 1.376 * ca * ss * sl_ - 0.331 * sl_ * ss + 0.375 * sa * ss,
                _ => 0.339 + 0.808 * cl * cs - 0.196 * sl_ * ss - 0.482 * ca * ss,
            };
            if scaled { r.exp() } else { r }
        })
        .collect();
    Ok(outputs([("map", Value::Map(Map::new(a.grid.clone(), v)))]))
}
