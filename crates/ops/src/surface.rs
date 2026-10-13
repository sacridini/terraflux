//! Surface metrics of continuous rasters (ISO 25178, as the R package geodiv computes them) through landschaft's
//! C++, geodiv's bits: amplitude (sa, sq, ssk, sku, sv, sph, smean), hybrid (sdq, sdq6, sdr), local features (sds,
//! ssc, s10z), the bearing area curve (sbi, sci, svi, sk, spk, svk) and spatial (sal, str, std, stdi, srw, srwi,
//! shw, sfd): of the whole surface (`surface.metrics`) or in a moving window around every cell
//! (`surface.window`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Metrics));
    r.add(Arc::new(Window));
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
