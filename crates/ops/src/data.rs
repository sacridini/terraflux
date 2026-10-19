//! Data downloaded on demand (`data.download`): the climate of WorldClim 2.1 and CHELSA 2.1 (now and under the
//! CMIP6 scenarios), the Copernicus DEM, SoilGrids and MapBiomas, or any raster a URL names, read by GDAL straight
//! from the servers (only the part a grid needs: their files are cloud-optimised, zipped or virtual) and warped onto
//! the grid of a map of the model, a file per layer in the cache.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use crate::files::Rasters;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Download));
}

const SOURCES: [&str; 7] =
    ["worldclim", "worldclim_future", "chelsa", "chelsa_future", "copernicus_dem", "soilgrids", "mapbiomas"];

/// A layer to fetch: its name (the file's), the GDAL path of its source and whether it holds classes.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub path: String,
    pub classes: bool,
}

fn layer(name: String, path: String) -> Layer {
    Layer { name, path, classes: false }
}

fn curl(url: &str) -> String {
    format!("/vsicurl/{url}")
}

/// The numbers of a setting (one, or a list), else `all`.
fn numbers(args: &Args, name: &str, all: std::ops::RangeInclusive<u32>) -> Result<Vec<u32>> {
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok(all.collect()),
        Some(serde_json::Value::Number(n)) => Ok(vec![n.as_u64().unwrap_or(0) as u32]),
        Some(v) => {
            serde_json::from_value(v.clone()).map_err(|e| Error::msg(format!("node {:?}: {name}: {e}", args.node)))
        }
    }
}

fn text(args: &Args, name: &str, default: &str) -> Result<String> {
    args.param(name, default.to_string())
}

/// WorldClim 2.1's layers: the historical climate (1970-2000) of a variable, a zip of a file per layer.
pub fn worldclim(variable: &str, resolution: &str, layers: &[u32]) -> Vec<Layer> {
    let base = "https://geodata.ucdavis.edu/climate/worldclim/2_1/base";
    if variable == "elev" {
        let f = format!("wc2.1_{resolution}_elev");
        return vec![layer("elev".into(), format!("/vsizip/{}/{f}.tif", curl(&format!("{base}/{f}.zip"))))];
    }
    let zip = curl(&format!("{base}/wc2.1_{resolution}_{variable}.zip"));
    layers
        .iter()
        .map(|&i| {
            let file = if variable == "bio" {
                format!("wc2.1_{resolution}_bio_{i}")
            } else {
                format!("wc2.1_{resolution}_{variable}_{i:02}")
            };
            layer(format!("{variable}{i:02}"), format!("/vsizip/{zip}/{file}.tif"))
        })
        .collect()
}

/// WorldClim 2.1's CMIP6 futures: a file of a variable (bioc, tmin, tmax, prec) of a model, scenario and period,
/// a band per layer.
pub fn worldclim_future(
    variable: &str,
    resolution: &str,
    gcm: &str,
    ssp: &str,
    period: &str,
    layers: &[u32],
) -> Vec<Layer> {
    let file = format!(
        "https://geodata.ucdavis.edu/cmip6/{resolution}/{gcm}/{ssp}/wc2.1_{resolution}_{variable}_{gcm}_{ssp}_{period}.tif"
    );
    layers.iter().map(|&i| layer(format!("{variable}{i:02}"), format!("vrt://{}?bands={i}", curl(&file)))).collect()
}

/// CHELSA 2.1's bioclimatic variables, 1981-2010 or a CMIP6 future (a model, a scenario, a period).
pub fn chelsa(layers: &[u32], future: Option<(&str, &str, &str)>) -> Vec<Layer> {
    let base = "https://os.zhdk.cloud.switch.ch/chelsav2/GLOBAL/climatologies";
    layers
        .iter()
        .map(|&i| {
            let url = match future {
                None => format!("{base}/1981-2010/bio/CHELSA_bio{i}_1981-2010_V.2.1.tif"),
                Some((gcm, ssp, period)) => format!(
                    "{base}/{period}/{}/{ssp}/bio/CHELSA_bio{i}_{period}_{}_{ssp}_V.2.1.tif",
                    gcm.to_uppercase(),
                    gcm.to_lowercase()
                ),
            };
            layer(format!("bio{i:02}"), curl(&url))
        })
        .collect()
}

/// The Copernicus DEM's tiles (GLO-30 or GLO-90: a degree each) over a box of longitudes and latitudes.
pub fn copernicus_tiles(resolution: u32, lon: [f64; 2], lat: [f64; 2]) -> Vec<String> {
    let (code, bucket) = if resolution == 90 { (30, "copernicus-dem-90m") } else { (10, "copernicus-dem-30m") };
    let mut out = Vec::new();
    for la in (lat[0].floor() as i32)..(lat[1].ceil() as i32) {
        for lo in (lon[0].floor() as i32)..(lon[1].ceil() as i32) {
            let ns = if la < 0 { format!("S{:02}", -la) } else { format!("N{la:02}") };
            let ew = if lo < 0 { format!("W{:03}", -lo) } else { format!("E{lo:03}") };
            let name = format!("Copernicus_DSM_COG_{code}_{ns}_00_{ew}_00_DEM");
            out.push(curl(&format!("https://{bucket}.s3.amazonaws.com/{name}/{name}.tif")));
        }
    }
    out
}

/// SoilGrids' virtual rasters of a property at a depth (a statistic: mean, Q0.05, Q0.5, Q0.95, uncertainty).
pub fn soilgrids(property: &str, depths: &[String], stat: &str) -> Vec<Layer> {
    depths
        .iter()
        .map(|d| {
            let f = format!("{property}_{d}_{stat}");
            layer(f.clone(), curl(&format!("https://files.isric.org/soilgrids/latest/data/{property}/{f}.vrt")))
        })
        .collect()
}

/// MapBiomas Brazil's land use and cover of years (collections 9, 10 and 11).
pub fn mapbiomas(collection: u32, years: &[u32]) -> std::result::Result<Vec<Layer>, String> {
    let base = "https://storage.googleapis.com/mapbiomas-public/initiatives/brasil";
    years
        .iter()
        .map(|&y| {
            let url = match collection {
                9 => format!("{base}/collection_9/lclu/coverage/brasil_coverage_{y}.tif"),
                10 => format!("{base}/collection_10/lulc/coverage/brazil_coverage_{y}.tif"),
                11 => format!("{base}/collection11/lulc/coverage/brazil_coverage/brazil_coverage-col11_{y}.tif"),
                c => return Err(format!("collection: 9, 10 or 11, not {c}")),
            };
            Ok(Layer { name: y.to_string(), path: curl(&url), classes: true })
        })
        .collect()
}

/// The grid to fill: a map's (any raster input), or an extent of longitudes and latitudes at a cell size.
fn grid_of(args: &Args) -> Result<Grid> {
    let fail = |m: &str| Error::msg(format!("node {:?}: {m}", args.node));
    match args.get("like") {
        Some(Value::Map(m)) => Ok((*m.grid).clone()),
        Some(Value::Stack(s)) => Ok((*s.grid).clone()),
        Some(Value::Raster(r)) => Ok(r.grid_of(r.chosen).clone()),
        Some(v) => Err(fail(&format!("like: a raster, not a {:?}", v.ty()))),
        None => {
            let e: Option<[f64; 4]> = args.opt_param("extent")?;
            let [xmin, ymin, xmax, ymax] =
                e.ok_or_else(|| fail("connect a map whose grid to fill (like), or give extent and cellsize"))?;
            let res: f64 = args.param("cellsize", f64::NAN)?;
            if res.is_nan() || res <= 0.0 || xmax <= xmin || ymax <= ymin {
                return Err(fail("extent: [west, south, east, north]; cellsize: degrees, above 0"));
            }
            let (w, h) = (((xmax - xmin) / res).round() as usize, ((ymax - ymin) / res).round() as usize);
            let mut g = Grid::new(w.max(1), h.max(1));
            g.transform = [xmin, res, 0.0, ymax, 0.0, -res];
            g.geo.wkt = Some("EPSG:4326".into());
            Ok(g)
        }
    }
}

/// The longitudes and latitudes a grid spans (its edges sampled, in WGS 84).
fn lonlat_box(g: &Grid) -> Result<([f64; 2], [f64; 2])> {
    let t = g.transform;
    let (w, h) = (g.width as f64, g.height as f64);
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for k in 0..=20 {
        let f = k as f64 / 20.0;
        for (c, r) in [(f * w, 0.0), (f * w, h), (0.0, f * h), (w, f * h)] {
            xs.push(t[0] + c * t[1] + r * t[2]);
            ys.push(t[3] + c * t[4] + r * t[5]);
        }
    }
    if let Some(crs) = crate::prepare::crs_of(g) {
        let tr = terraflux_gdal::Transform::new(&crs, "EPSG:4326").map_err(Error::msg)?;
        tr.points(&mut xs, &mut ys);
    }
    let lo = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(([lo(&xs), hi(&xs)], [lo(&ys), hi(&ys)]))
}

/// The layers of the node's source.
fn layers_of(args: &Args, g: &Grid) -> Result<Vec<Layer>> {
    let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
    let source: String = args.param("source", String::new())?;
    Ok(match source.as_str() {
        "worldclim" => {
            let v = text(args, "variable", "bio")?;
            let all = if v == "bio" { 1..=19 } else { 1..=12 };
            worldclim(&v, &text(args, "resolution", "10m")?, &numbers(args, "layers", all)?)
        }
        "worldclim_future" => worldclim_future(
            &text(args, "variable", "bioc")?,
            &text(args, "resolution", "10m")?,
            &text(args, "gcm", "MPI-ESM1-2-HR")?,
            &format!("ssp{}", text(args, "ssp", "245")?.trim_start_matches("ssp")),
            &text(args, "period", "2041-2060")?,
            &numbers(args, "layers", 1..=19)?,
        ),
        "chelsa" => chelsa(&numbers(args, "layers", 1..=19)?, None),
        "chelsa_future" => chelsa(
            &numbers(args, "layers", 1..=19)?,
            Some((
                &text(args, "gcm", "MPI-ESM1-2-HR")?,
                &format!("ssp{}", text(args, "ssp", "370")?.trim_start_matches("ssp")),
                &text(args, "period", "2041-2070")?,
            )),
        ),
        "copernicus_dem" => {
            let res: u32 = args.param("dem_resolution", 90)?;
            let (lon, lat) = lonlat_box(g)?;
            let tiles: Vec<String> = copernicus_tiles(res, lon, lat)
                .into_iter()
                .filter(|p| terraflux_gdal::Dataset::open(p).is_ok())
                .collect();
            if tiles.is_empty() {
                return Err(fail("the Copernicus DEM has no tile over the grid (all sea?)".into()));
            }
            // the tiles together, as a virtual mosaic of the sources
            vec![layer("elevation".into(), tiles.join("|"))]
        }
        "soilgrids" => {
            let depths: Vec<String> = match args.params.get("depth") {
                None => vec!["0-5cm".into()],
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                Some(v) => serde_json::from_value(v.clone()).map_err(|e| fail(format!("depth: {e}")))?,
            };
            soilgrids(&text(args, "property", "soc")?, &depths, &text(args, "stat", "mean")?)
        }
        "mapbiomas" => {
            let years = numbers(args, "years", 2023..=2023)?;
            mapbiomas(args.param("collection", 10)?, &years).map_err(fail)?
        }
        "" => {
            let urls: Vec<String> = match args.params.get("url") {
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                Some(v) => serde_json::from_value(v.clone()).map_err(|e| fail(format!("url: {e}")))?,
                None => return Err(fail(format!("source: one of {SOURCES:?}, or a url"))),
            };
            urls.iter()
                .enumerate()
                .map(|(k, u)| {
                    let path = if u.starts_with("http://") || u.starts_with("https://") { curl(u) } else { u.clone() };
                    layer(if urls.len() == 1 { "layer".into() } else { format!("layer{:02}", k + 1) }, path)
                })
                .collect()
        }
        s => return Err(fail(format!("source: {s:?} is not one of {SOURCES:?}"))),
    })
}

struct Download;

impl Operator for Download {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "data.download",
            "Data downloaded on demand and warped onto a grid (gdalwarp straight from the servers: only the part \
             the grid covers is read): WorldClim 2.1 (the 1970-2000 climate; CMIP6 futures), CHELSA 2.1 \
             (1981-2010; CMIP6 futures), the Copernicus DEM (30 or 90 m), SoilGrids (a property at depths) and \
             MapBiomas Brazil (land use and cover by year), or any raster a url names; a file per layer kept in \
             the cache (Terraflux's, TERRAFLUX_CACHE), downloaded once. Classes are resampled by the most common \
             cell, values bilinearly. The values are the servers' (CHELSA's scaled integers: × 0.1, and - 273.15 \
             for temperatures; SoilGrids' in its mapped units).",
            Access::Global,
        )
        .input(PortSpec::optional("like", Type::Raster, "the map whose grid to fill (else extent and cellsize)"))
        .output(PortSpec::new("raster", Type::Raster, "the layers on the grid, a file each"))
        .param(PortSpec::optional("source", Type::Text, "the data (or url instead)").choices(SOURCES))
        .param(PortSpec::optional(
            "variable",
            Type::Text,
            "worldclim: bio (default), tavg, tmin, tmax, prec, srad, wind, vapr, elev; worldclim_future: bioc \
             (default), tmin, tmax, prec",
        ))
        .param(PortSpec::optional("layers", Type::Any, "the variables' numbers (bio 1-19, months 1-12; default all)"))
        .param(PortSpec::optional(
            "resolution",
            Type::Text,
            "worldclim and worldclim_future: 10m (default), 5m, 2.5m, 30s",
        ))
        .param(PortSpec::optional("gcm", Type::Text, "the climate model of a future (default MPI-ESM1-2-HR)"))
        .param(PortSpec::optional(
            "ssp",
            Type::Text,
            "the scenario: 126, 245 (WorldClim's default), 370 (CHELSA's), 585",
        ))
        .param(PortSpec::optional(
            "period",
            Type::Text,
            "a future's years (WorldClim 2021-2040, 2041-2060 (default), 2061-2080, 2081-2100; CHELSA 2011-2040, \
             2041-2070 (default), 2071-2100)",
        ))
        .param(PortSpec::optional("dem_resolution", Type::Integer, "copernicus_dem: 30 or 90 m (default)"))
        .param(PortSpec::optional(
            "property",
            Type::Text,
            "soilgrids: soc (default), phh2o, clay, sand, silt, bdod, cec, cfvo, nitrogen, ocd",
        ))
        .param(PortSpec::optional("depth", Type::Any, "soilgrids: 0-5cm (default), 5-15cm, ... 100-200cm, or a list"))
        .param(PortSpec::optional("stat", Type::Text, "soilgrids: mean (default), Q0.05, Q0.5, Q0.95, uncertainty"))
        .param(PortSpec::optional("collection", Type::Integer, "mapbiomas: 9, 10 (default) or 11"))
        .param(PortSpec::optional("years", Type::Any, "mapbiomas: a year or a list (default 2023)"))
        .param(PortSpec::optional("url", Type::Any, "any raster GDAL reads (a URL, a path), or a list"))
        .param(PortSpec::optional("extent", Type::Any, "without like: [west, south, east, north], degrees"))
        .param(PortSpec::optional("cellsize", Type::Number, "with extent: the cells' side, degrees"))
        .param(PortSpec::optional(
            "resampling",
            Type::Text,
            "auto (default), near, bilinear, cubic, average, mode... (gdalwarp's)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        crate::prepare::need_gdal(args.node)?;
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let g = grid_of(args)?;
        let layers = layers_of(args, &g)?;
        let resampling: String = args.param("resampling", "auto".to_string())?;
        // the cache: a folder per source and grid
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for l in &layers {
            l.path.hash(&mut h);
        }
        format!("{:?}{}{}{:?}{resampling}", g.transform, g.width, g.height, crate::prepare::crs_of(&g)).hash(&mut h);
        let dir = crate::sdm::refdata::cache_dir().join("data").join(format!("{:016x}", h.finish()));
        std::fs::create_dir_all(&dir).map_err(|e| fail(format!("{}: {e}", dir.display())))?;
        let t = g.transform;
        let (xmin, xmax) = (t[0], t[0] + t[1] * g.width as f64);
        let (ymax, ymin) = (t[3], t[3] + t[5] * g.height as f64);
        for (k, l) in layers.iter().enumerate() {
            let out = dir.join(format!("{}.tif", l.name));
            if out.is_file() {
                continue;
            }
            if !ctx.progress(Some(k as f64 / layers.len() as f64), format!("downloading {}", l.name)) {
                return Err(fail("stopped".into()));
            }
            let method = match resampling.as_str() {
                "auto" if l.classes => "mode",
                "auto" => "bilinear",
                m => m,
            };
            let mut a: Vec<String> = ["-r", method, "-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES"]
                .iter()
                .map(|s| s.to_string())
                .collect();
            a.extend([
                "-te".into(),
                xmin.to_string(),
                ymin.min(ymax).to_string(),
                xmax.to_string(),
                ymax.max(ymin).to_string(),
                "-ts".into(),
                g.width.to_string(),
                g.height.to_string(),
            ]);
            if let Some(c) = crate::prepare::crs_of(&g) {
                a.extend(["-t_srs".into(), c]);
            }
            let sources: Vec<PathBuf> = l.path.split('|').map(PathBuf::from).collect();
            let refs: Vec<&Path> = sources.iter().map(PathBuf::as_path).collect();
            let tmp = dir.join(format!(".{}-{}.tif", l.name, std::process::id()));
            if let Err(e) = terraflux_gdal::warp(&refs, &tmp, &a) {
                let _ = std::fs::remove_file(&tmp);
                return Err(fail(format!("{}: {e}", l.name)));
            }
            std::fs::rename(&tmp, &out).map_err(|e| fail(format!("{}: {e}", out.display())))?;
        }
        // used now; the downloads of other grids held under the cache's size, the least recently used removed
        crate::cache::touch(&dir);
        if let Some(all) = dir.parent() {
            crate::cache::prune(all, crate::cache::limit(), &[&dir]);
        }
        let path = if layers.len() == 1 { dir.join(format!("{}.tif", layers[0].name)) } else { dir };
        Ok(outputs([("raster", Value::Raster(Arc::new(Rasters::open(&path.to_string_lossy(), None, None)?)))]))
    }
}
