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

pub const SOURCES: [&str; 11] = [
    "worldclim",
    "worldclim_future",
    "chelsa",
    "chelsa_future",
    "copernicus_dem",
    "soilgrids",
    "mapbiomas",
    "hydrography90m",
    "esa_worldcover",
    "global_forest_change",
    "modis",
];

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

/// Hydrography90m's files on the IGB Berlin's server, by their path.
const H90M: &str = "https://public.igb-berlin.de/index.php/s/agciopgzXjWswF4/download?path=";

/// Hydrography90m's variables in tiles (hydrographr's download_tiles), by the folder that holds them.
pub const H90M_VARIABLES: [(&str, &[&str]); 6] = [
    ("r.watershed", &["sub_catchment", "segment", "basin", "outlet", "direction", "accumulation", "depression"]),
    ("r.stream.order", &["order_strahler", "order_shreve", "order_horton", "order_hack", "order_topo"]),
    ("r.stream.slope", &["slope_grad_dw_cel", "slope_elv_dw_cel", "slope_curv_max_dw_cel", "slope_curv_min_dw_cel"]),
    (
        "r.stream.distance",
        &[
            "stream_dist_up_near",
            "stream_dist_up_farth",
            "stream_dist_dw_near",
            "stream_dist_proximity",
            "outlet_dist_dw_basin",
            "outlet_dist_dw_scatch",
            "stream_diff_up_near",
            "stream_diff_up_farth",
            "stream_diff_dw_near",
            "outlet_diff_dw_basin",
            "outlet_diff_dw_scatch",
        ],
    ),
    (
        "r.stream.channel",
        &[
            "channel_grad_dw_seg",
            "channel_grad_up_seg",
            "channel_grad_up_cel",
            "channel_curv_cel",
            "channel_elv_dw_seg",
            "channel_elv_up_seg",
            "channel_elv_up_cel",
            "channel_elv_dw_cel",
            "channel_dist_dw_seg",
            "channel_dist_up_seg",
            "channel_dist_up_cel",
        ],
    ),
    ("flow.index", &["cti", "spi", "sti"]),
];

/// The 20° tiles of Hydrography90m and Environment90m over a box of longitudes and latitudes: hXXvYY, h00v00 the
/// one from 180° W and 85° N, their numbers stepping by 2 every 20°.
pub fn h90m_tiles(lon: [f64; 2], lat: [f64; 2]) -> Vec<String> {
    let span = |a: f64, b: f64, n: i32| -> (i32, i32) {
        ((a / 20.0).floor().clamp(0.0, (n - 1) as f64) as i32, ((b / 20.0).ceil() as i32 - 1).clamp(0, n - 1))
    };
    let (c0, c1) = span(lon[0] + 180.0, lon[1] + 180.0, 18);
    let (r0, r1) = span(85.0 - lat[1], 85.0 - lat[0], 7);
    let mut out = Vec::new();
    for r in r0..=r1 {
        for c in c0..=c1 {
            out.push(format!("h{:02}v{:02}", 2 * c, 2 * r));
        }
    }
    out
}

/// The paths of a Hydrography90m variable's tiles (whether they exist or not: the sea has none), and whether its
/// values are classes (ids, directions, orders).
pub fn hydrography90m(variable: &str, tiles: &[String]) -> std::result::Result<(Vec<String>, bool), String> {
    let folder = H90M_VARIABLES.iter().find(|(_, vs)| vs.contains(&variable)).map(|(f, _)| *f).ok_or_else(|| {
        let all: Vec<&str> = H90M_VARIABLES.iter().flat_map(|(_, v)| v.iter().copied()).collect();
        format!("hydrography90m: variable {variable:?} is not one of {}", all.join(", "))
    })?;
    let classes = folder == "r.stream.order"
        || ["sub_catchment", "segment", "basin", "outlet", "direction", "depression"].contains(&variable);
    let paths = tiles
        .iter()
        .map(|t| {
            curl(&format!("{H90M}{}", format!("/{folder}/{variable}_tiles20d/{variable}_{t}.tif").replace('/', "%2F")))
        })
        .collect();
    Ok((paths, classes))
}

/// ESA WorldCover's tiles (3 degrees, named by their south-west corner) over a box of longitudes and latitudes:
/// 2020 (v100) or 2021 (v200).
pub fn worldcover_tiles(year: u32, lon: [f64; 2], lat: [f64; 2]) -> std::result::Result<Vec<String>, String> {
    let version = match year {
        2020 => "v100",
        2021 => "v200",
        y => return Err(format!("esa_worldcover: 2020 or 2021, not {y}")),
    };
    let mut out = Vec::new();
    let from = |v: f64| (v / 3.0).floor() as i32 * 3;
    let mut la = from(lat[0]);
    while (la as f64) < lat[1] {
        let mut lo = from(lon[0]);
        while (lo as f64) < lon[1] {
            let ns = if la < 0 { format!("S{:02}", -la) } else { format!("N{la:02}") };
            let ew = if lo < 0 { format!("W{:03}", -lo) } else { format!("E{lo:03}") };
            out.push(curl(&format!(
                "https://esa-worldcover.s3.eu-central-1.amazonaws.com/{version}/{year}/map/\
                 ESA_WorldCover_10m_{year}_{version}_{ns}{ew}_Map.tif"
            )));
            lo += 3;
        }
        la += 3;
    }
    Ok(out)
}

/// The version of Hansen's Global Forest Change read.
pub const GFC: &str = "GFC-2025-v1.13";

/// Global Forest Change's tiles of a variable (10 degrees, named by their north-west corner) over a box of
/// longitudes and latitudes, and whether its values are classes.
pub fn gfc_tiles(variable: &str, lon: [f64; 2], lat: [f64; 2]) -> std::result::Result<(Vec<String>, bool), String> {
    let classes = match variable {
        "treecover2000" => false,
        "lossyear" | "gain" | "datamask" => true,
        v => return Err(format!("global_forest_change: treecover2000, lossyear, gain or datamask, not {v:?}")),
    };
    let mut out = Vec::new();
    for top in ((lat[0] / 10.0).floor() as i32 + 1)..=((lat[1] / 10.0).ceil() as i32) {
        let t = top * 10;
        for left in ((lon[0] / 10.0).floor() as i32)..((lon[1] / 10.0).ceil() as i32) {
            let l = left * 10;
            let ns = if t < 0 { format!("{:02}S", -t) } else { format!("{t:02}N") };
            let ew = if l < 0 { format!("{:03}W", -l) } else { format!("{l:03}E") };
            out.push(curl(&format!(
                "https://storage.googleapis.com/earthenginepartners-hansen/{GFC}/Hansen_{GFC}_{variable}_{ns}_{ew}.tif"
            )));
        }
    }
    Ok((out, classes))
}

/// MODIS on Microsoft's Planetary Computer: its STAC catalog, and the tokens that sign its files (anonymous).
const PLANETARY: &str = "https://planetarycomputer.microsoft.com/api";

/// A MODIS product on the Planetary Computer: the variable's name here, its collection, the prefix of its items
/// (Terra's, or both satellites' combined), its asset, and how many days back a date looks for the file that
/// covers it (the product's period, and some).
pub struct Modis {
    pub variable: &'static str,
    pub collection: &'static str,
    pub prefix: &'static str,
    pub asset: &'static str,
    pub days: i64,
}

/// The MODIS variables: the vegetation indices (MOD13Q1, 16-day composites at 250 m, × 0.0001), the land surface
/// temperature of the day and of the night (MOD11A2, 8-day at 1 km, kelvin × 0.02), the burned area (MCD64A1,
/// monthly at 500 m: the day of the year a cell burned, 0 unburned) and the yearly gross and net primary
/// production (MOD17A3HGF at 500 m, kg C/m² × 0.0001).
pub const MODIS: [Modis; 7] = [
    Modis { variable: "ndvi", collection: "modis-13Q1-061", prefix: "MOD13Q1", asset: "250m_16_days_NDVI", days: 40 },
    Modis { variable: "evi", collection: "modis-13Q1-061", prefix: "MOD13Q1", asset: "250m_16_days_EVI", days: 40 },
    Modis { variable: "lst_day", collection: "modis-11A2-061", prefix: "MOD11A2", asset: "LST_Day_1km", days: 24 },
    Modis { variable: "lst_night", collection: "modis-11A2-061", prefix: "MOD11A2", asset: "LST_Night_1km", days: 24 },
    Modis { variable: "burned", collection: "modis-64A1-061", prefix: "MCD64A1", asset: "Burn_Date", days: 62 },
    Modis { variable: "gpp", collection: "modis-17A3HGF-061", prefix: "MOD17A3HGF", asset: "Gpp_500m", days: 366 },
    Modis { variable: "npp", collection: "modis-17A3HGF-061", prefix: "MOD17A3HGF", asset: "Npp_500m", days: 366 },
];

/// The MODIS product of a variable.
pub fn modis_of(variable: &str) -> std::result::Result<&'static Modis, String> {
    MODIS.iter().find(|m| m.variable == variable).ok_or_else(|| {
        let known: Vec<&str> = MODIS.iter().map(|m| m.variable).collect();
        format!("modis: one of {}, not {variable:?}", known.join(", "))
    })
}

/// The asset of a MODIS variable.
pub fn modis_asset(variable: &str) -> std::result::Result<&'static str, String> {
    modis_of(variable).map(|m| m.asset)
}

/// Days since 1970-01-01 of a date (proleptic Gregorian), and back.
fn days_of(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn date_of(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
}

/// A date written YYYY-MM-DD, as days since 1970.
pub fn parse_date(t: &str) -> std::result::Result<i64, String> {
    let p: Vec<i64> = t
        .trim()
        .split('-')
        .map(|x| x.parse::<i64>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| format!("{t:?}: a date as YYYY-MM-DD"))?;
    match p[..] {
        [y, m, d] if (1..=12).contains(&m) && (1..=31).contains(&d) => Ok(days_of(y, m, d)),
        _ => Err(format!("{t:?}: a date as YYYY-MM-DD")),
    }
}

/// The files of the MODIS product of an asset at a date over a grid: its latest file (a 16-day or 8-day
/// composite, a month, a year) begun on the date or before (its tiles together), signed.
fn modis_sources(asset: &str, date: &str, g: &Grid) -> std::result::Result<Vec<String>, String> {
    let product = MODIS.iter().find(|m| m.asset == asset).ok_or_else(|| format!("modis: no asset {asset:?}"))?;
    let day = parse_date(date)?;
    let (y0, m0, d0) = date_of(day - product.days);
    let (lon, lat) = lonlat_box(g).map_err(|e| e.to_string())?;
    let get = |url: &str| -> std::result::Result<serde_json::Value, String> {
        let (status, body) =
            terraflux_gdal::fetch(url, &["TIMEOUT=60", "MAX_RETRY=3", "RETRY_DELAY=2", "USERAGENT=Terraflux"])?;
        if status != 200 && status != 0 {
            return Err(format!("{url}: HTTP {status}"));
        }
        serde_json::from_slice(&body).map_err(|e| format!("{url}: {e}"))
    };
    let collection = product.collection;
    let url = format!(
        "{PLANETARY}/stac/v1/search?collections={collection}&bbox={},{},{},{}&datetime={y0:04}-{m0:02}-{d0:02}T00:00:00Z/\
         {date}T23:59:59Z&limit=200",
        lon[0], lat[0], lon[1], lat[1]
    );
    let found = get(&url)?;
    let items: Vec<&serde_json::Value> = found["features"]
        .as_array()
        .map(|a| a.iter().filter(|f| f["id"].as_str().is_some_and(|i| i.starts_with(product.prefix))).collect())
        .unwrap_or_default();
    let start = |f: &serde_json::Value| f["properties"]["start_datetime"].as_str().unwrap_or("").to_string();
    let last = items.iter().map(|f| start(f)).filter(|s| s.as_str() <= date || s.starts_with(date)).max();
    let Some(last) = last else {
        return Err(format!("modis: no {} over the grid in the {} days up to {date}", product.prefix, product.days));
    };
    // signed by a token of their storage account and container (https://ACCOUNT.blob.core.windows.net/CONTAINER/)
    let hrefs: Vec<&str> =
        items.iter().filter(|f| start(f) == last).filter_map(|f| f["assets"][asset]["href"].as_str()).collect();
    let mut tokens: Vec<(String, String)> = Vec::new();
    let mut out = Vec::new();
    for h in hrefs {
        let rest = h.strip_prefix("https://").ok_or_else(|| format!("modis: {h}: not on https"))?;
        let (host, path) = rest.split_once('/').ok_or_else(|| format!("modis: {h}: no container"))?;
        let account = host.split('.').next().unwrap_or(host);
        let container = path.split('/').next().unwrap_or("");
        let key = format!("{account}/{container}");
        let token = match tokens.iter().find(|(k, _)| *k == key) {
            Some((_, t)) => t.clone(),
            None => {
                let t = get(&format!("{PLANETARY}/sas/v1/token/{key}"))?["token"]
                    .as_str()
                    .ok_or("modis: the Planetary Computer gave no token")?
                    .to_string();
                tokens.push((key, t.clone()));
                t
            }
        };
        out.push(curl(&format!("{h}?{token}")));
    }
    Ok(out)
}

/// The files a layer is read from, found when it is warped: a MODIS composite looked up, the tiles there (the
/// sea has none), else its path.
fn sources_of(l: &Layer, g: &Grid) -> std::result::Result<Vec<String>, String> {
    if let Some(key) = l.path.strip_prefix("modis:") {
        let (asset, date) = key.split_once(':').ok_or("modis: asset:date")?;
        return modis_sources(asset, date, g);
    }
    let all: Vec<String> = l.path.split('|').map(String::from).collect();
    if all.len() == 1 {
        return Ok(all);
    }
    let there: Vec<String> = all.into_iter().filter(|p| terraflux_gdal::Dataset::open(p).is_ok()).collect();
    if there.is_empty() {
        return Err(format!("{}: no tile over the grid (all sea?)", l.name));
    }
    Ok(there)
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
            let area = area_of(args)?;
            let (e, res) = match (e, &area) {
                (Some(e), _) => (e, args.param("cellsize", f64::NAN)?),
                (None, Some(a)) => {
                    // the area's bounds in longitudes and latitudes, widened to whole cells
                    let b = lonlat_bounds(a)?;
                    let side = (b[2] - b[0]).max(b[3] - b[1]);
                    let res: f64 = args.param("cellsize", side / AREA_CELLS)?;
                    if res.is_nan() || res <= 0.0 {
                        return Err(fail("cellsize: degrees, above 0"));
                    }
                    let snap = |v: f64, up: bool| if up { (v / res).ceil() * res } else { (v / res).floor() * res };
                    ([snap(b[0], false), snap(b[1], false), snap(b[2], true), snap(b[3], true)], res)
                }
                (None, None) => {
                    return Err(fail(
                        "connect a map whose grid to fill (like) or an area, or give extent and cellsize",
                    ));
                }
            };
            let [xmin, ymin, xmax, ymax] = e;
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

/// Without a cell size, the cells across an area's longer side.
const AREA_CELLS: f64 = 500.0;

/// The area to cut the data to: its polygons (none: the whole grid).
fn area_of(args: &Args) -> Result<Option<Arc<terraflux_raster::vector::Vector>>> {
    let v = match args.get("area") {
        None => return Ok(None),
        Some(_) => crate::vector::vector_of(args, "area")?,
    };
    let keep: Vec<usize> = (0..v.len()).filter(|&i| v.geometries[i].is_polygonal()).collect();
    if keep.is_empty() {
        return Err(Error::msg(format!("node {:?}: area: polygons (an area drawn, a region), none given", args.node)));
    }
    Ok(Some(if keep.len() == v.len() { v.clone() } else { Arc::new(v.select(&keep)) }))
}

/// An area's bounds in longitudes and latitudes (its points reprojected to WGS 84).
fn lonlat_bounds(v: &terraflux_raster::vector::Vector) -> Result<[f64; 4]> {
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for g in &v.geometries {
        g.points(&mut |p| {
            xs.push(p[0]);
            ys.push(p[1]);
        });
    }
    if let Some(crs) = &v.crs {
        let tr = terraflux_gdal::Transform::new(crs, "EPSG:4326").map_err(Error::msg)?;
        tr.points(&mut xs, &mut ys);
    }
    let lo = |v: &[f64]| v.iter().copied().filter(|x| x.is_finite()).fold(f64::INFINITY, f64::min);
    let hi = |v: &[f64]| v.iter().copied().filter(|x| x.is_finite()).fold(f64::NEG_INFINITY, f64::max);
    let b = [lo(&xs), lo(&ys), hi(&xs), hi(&ys)];
    if b.iter().all(|x| x.is_finite()) && b[2] > b[0] && b[3] > b[1] {
        Ok(b)
    } else {
        Err(Error::msg("area: its bounds span no longitudes or latitudes"))
    }
}

/// The longitudes and latitudes a grid spans (its edges sampled, in WGS 84).
pub(crate) fn lonlat_box(g: &Grid) -> Result<([f64; 2], [f64; 2])> {
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
            // the tiles together, as a virtual mosaic of the sources (those there, when warped)
            vec![layer("elevation".into(), copernicus_tiles(res, lon, lat).join("|"))]
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
        "hydrography90m" => {
            let v = text(args, "variable", "sub_catchment")?;
            let (lon, lat) = lonlat_box(g)?;
            let (paths, classes) = hydrography90m(&v, &h90m_tiles(lon, lat)).map_err(fail)?;
            vec![Layer { name: v, path: paths.join("|"), classes }]
        }
        "esa_worldcover" => {
            let (lon, lat) = lonlat_box(g)?;
            numbers(args, "years", 2021..=2021)?
                .into_iter()
                .map(|y| {
                    let tiles = worldcover_tiles(y, lon, lat).map_err(fail)?;
                    Ok(Layer { name: format!("worldcover{y}"), path: tiles.join("|"), classes: true })
                })
                .collect::<Result<Vec<Layer>>>()?
        }
        "global_forest_change" => {
            let v = text(args, "variable", "treecover2000")?;
            let (lon, lat) = lonlat_box(g)?;
            let (tiles, classes) = gfc_tiles(&v, lon, lat).map_err(fail)?;
            vec![Layer { name: v, path: tiles.join("|"), classes }]
        }
        "modis" => {
            let v = text(args, "variable", "ndvi")?;
            let asset = modis_asset(&v).map_err(fail)?;
            let dates: Vec<String> = match args.params.get("date") {
                None | Some(serde_json::Value::Null) => vec!["2024-07-01".into()],
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                Some(d) => serde_json::from_value(d.clone()).map_err(|e| fail(format!("date: {e}")))?,
            };
            dates
                .iter()
                .map(|d| {
                    parse_date(d).map_err(fail)?;
                    Ok(layer(format!("{v}_{}", d.trim()), format!("modis:{asset}:{}", d.trim())))
                })
                .collect::<Result<Vec<Layer>>>()?
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

/// What a node fetches, found without the network: the cache's folder for it (a folder per source, grid and
/// area), the grid, the layers (a file each in the folder once fetched), the area and the resampling.
pub struct Plan {
    pub dir: PathBuf,
    pub grid: Grid,
    pub layers: Vec<Layer>,
    pub area: Option<Arc<terraflux_raster::vector::Vector>>,
    pub resampling: String,
}

impl Plan {
    /// The layers already in the cache.
    pub fn cached(&self) -> Vec<&Layer> {
        self.layers.iter().filter(|l| self.dir.join(format!("{}.tif", l.name)).is_file()).collect()
    }
}

/// What the node's settings and inputs fetch, and where in the cache.
pub fn plan(args: &Args) -> Result<Plan> {
    let grid = grid_of(args)?;
    let layers = layers_of(args, &grid)?;
    let resampling: String = args.param("resampling", "auto".to_string())?;
    let area = area_of(args)?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for l in &layers {
        l.path.hash(&mut h);
    }
    let g = &grid;
    format!("{:?}{}{}{:?}{resampling}", g.transform, g.width, g.height, crate::prepare::crs_of(g)).hash(&mut h);
    if let Some(a) = &area {
        format!("{:?}{:?}", a.geometries, a.crs).hash(&mut h);
    }
    let dir = crate::sdm::refdata::cache_dir().join("data").join(format!("{:016x}", h.finish()));
    Ok(Plan { dir, grid, layers, area, resampling })
}

struct Download;

impl Operator for Download {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "data.download",
            "Data downloaded on demand and warped onto a grid (gdalwarp straight from the servers: only the part \
             the grid covers is read): WorldClim 2.1 (the 1970-2000 climate; CMIP6 futures), CHELSA 2.1 \
             (1981-2010; CMIP6 futures), the Copernicus DEM (30 or 90 m), SoilGrids (a property at depths) and \
             MapBiomas Brazil (land use and cover by year), Hydrography90m (the river network of the world at 90 m: \
             its sub-catchments, segments, basins, flow directions and accumulation, stream orders, slopes and \
             distances), ESA WorldCover (land cover at 10 m, 2020 and 2021), Hansen's Global Forest Change \
             (tree cover in 2000, the year of loss, gain, at 30 m), MODIS from Microsoft's Planetary Computer \
             (the vegetation indices NDVI and EVI, 16-day at 250 m; the land surface temperature of the day and \
             the night, 8-day at 1 km; the burned area, monthly at 500 m; the yearly gross and net primary \
             production at 500 m), or any raster a url names; a file \
             per layer kept in the cache (Terraflux's, TERRAFLUX_CACHE), downloaded once. Classes are resampled \
             by the most common cell, values bilinearly. The values are the servers' (CHELSA's scaled integers: \
             × 0.1, and - 273.15 for temperatures; SoilGrids' in its mapped units; MODIS' indices and production \
             × 0.0001, its temperatures × 0.02 in kelvin, its burned area the day of the year, 0 unburned).",
            Access::Global,
        )
        .input(PortSpec::optional("like", Type::Raster, "the map whose grid to fill (else extent and cellsize)"))
        .input(PortSpec::optional(
            "area",
            Type::Vector,
            "polygons to cut the data to (an area drawn): no data outside them; without like and extent, the grid              spans them",
        ))
        .output(PortSpec::new("raster", Type::Raster, "the layers on the grid, a file each"))
        .param(PortSpec::optional("source", Type::Text, "the data (or url instead)").choices(SOURCES))
        .param(PortSpec::optional(
            "variable",
            Type::Text,
            "worldclim: bio (default), tavg, tmin, tmax, prec, srad, wind, vapr, elev; worldclim_future: bioc \
             (default), tmin, tmax, prec; hydrography90m: sub_catchment (default), segment, basin, outlet, direction, \
             accumulation, order_strahler... (hydrographr's names); global_forest_change: treecover2000 (default), \
             lossyear, gain, datamask; modis: ndvi (default), evi, lst_day, lst_night, burned, gpp, npp",
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
        .param(PortSpec::optional(
            "years",
            Type::Any,
            "mapbiomas: a year or a list (default 2023); esa_worldcover: 2020 or 2021 (default), or both",
        ))
        .param(PortSpec::optional(
            "date",
            Type::Any,
            "modis: a date, YYYY-MM-DD (the composite, month or year begun on it or last before it; default \
             2024-07-01), or a list",
        ))
        .param(PortSpec::optional("url", Type::Any, "any raster GDAL reads (a URL, a path), or a list"))
        .param(PortSpec::optional("extent", Type::Any, "without like: [west, south, east, north], degrees"))
        .param(PortSpec::optional(
            "cellsize",
            Type::Number,
            "with extent or an area: the cells' side, degrees (an area's default: its longer side in 500 cells)",
        ))
        .param(PortSpec::optional(
            "resampling",
            Type::Text,
            "auto (default), near, bilinear, cubic, average, mode... (gdalwarp's)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        crate::prepare::need_gdal(args.node)?;
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let Plan { dir, grid: g, layers, area, resampling } = plan(args)?;
        std::fs::create_dir_all(&dir).map_err(|e| fail(format!("{}: {e}", dir.display())))?;
        // the area as a file, gdalwarp's cutline (reprojected by it to the data's coordinates)
        let cutline = match &area {
            Some(a) => {
                let file = dir.join("area.gpkg");
                if !file.is_file() {
                    let only = terraflux_raster::vector::Vector { attributes: Vec::new(), ..(**a).clone() };
                    terraflux_gdal::write_vector(&file, "area", &only).map_err(|e| fail(format!("area: {e}")))?;
                }
                Some(file)
            }
            None => None,
        };
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
            let sources: Vec<PathBuf> = sources_of(l, &g).map_err(fail)?.into_iter().map(PathBuf::from).collect();
            if let Some(file) = &cutline {
                a.extend(["-cutline".into(), file.to_string_lossy().into_owned(), "-cl".into(), "area".into()]);
                // outside the area no data: the data's own no-data value, else NaN (values) or 0 (classes)
                let own = terraflux_gdal::Dataset::open(&sources[0]).ok().and_then(|d| d.meta.nodata);
                if own.is_none() {
                    a.extend(["-dstnodata".into(), if l.classes { "0" } else { "nan" }.into()]);
                    if !l.classes {
                        a.extend(["-ot".into(), "Float32".into()]);
                    }
                }
            }
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
