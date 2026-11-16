//! STAC catalogues (`data.stac`): the scenes of a collection over a grid, found by the catalogue's search
//! (`POST /search`, the pages followed), each asset read straight from the server (only the part the grid covers),
//! warped onto the grid and the scenes combined cell by cell (median, mean, min, max, or the first with a value).
//! The library's `download.stac` is the reference: the same search, order of the scenes, composite, no-data
//! handling, resampling and grid.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::{DataType, Grid, Raster};

use crate::files::Rasters;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Stac));
}

/// How the scenes are combined.
pub const COMPOSITES: [&str; 5] = ["median", "mean", "min", "max", "first"];

/// How the files are made readable: as they are, or with the Planetary Computer's tokens.
pub const SIGNS: [&str; 2] = ["none", "planetary_computer"];

/// What the catalogue is asked over HTTP: the search (POST) and the pages it links to (GET, or POST).
trait Http {
    fn post(&mut self, url: &str, body: &Json) -> std::result::Result<Json, String>;
    fn get(&mut self, url: &str) -> std::result::Result<Json, String>;
}

/// The web, through ureq.
struct Web(ureq::Agent);

impl Web {
    fn new() -> Web {
        use ureq::tls::{RootCerts, TlsConfig};
        Web(ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_global(Some(Duration::from_secs(120)))
            .http_status_as_error(false)
            .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
            .user_agent(format!("Terraflux/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .into())
    }

    fn answer(
        url: &str,
        r: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> std::result::Result<Json, String> {
        let mut r = match r {
            Ok(r) => r,
            Err(e) => {
                return Err(match e {
                    ureq::Error::HostNotFound | ureq::Error::ConnectionFailed | ureq::Error::Timeout(_) => {
                        format!("{url}: cannot be reached ({e}): is the computer online, and the address right?")
                    }
                    e => format!("{url}: {e}"),
                });
            }
        };
        let status = r.status().as_u16();
        let bytes = r.body_mut().with_config().limit(256 << 20).read_to_vec().map_err(|e| format!("{url}: {e}"))?;
        if !(200..300).contains(&status) {
            let why: String = String::from_utf8_lossy(&bytes).chars().take(500).collect();
            return Err(format!("{url}: HTTP {status}: {why}"));
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("{url}: not JSON: {e}"))
    }
}

impl Http for Web {
    fn post(&mut self, url: &str, body: &Json) -> std::result::Result<Json, String> {
        let bytes = serde_json::to_vec(body).map_err(|e| e.to_string())?;
        let r = self.0.post(url).header("Content-Type", "application/json").send(&bytes[..]);
        Web::answer(url, r)
    }

    fn get(&mut self, url: &str) -> std::result::Result<Json, String> {
        Web::answer(url, self.0.get(url).call())
    }
}

/// STAC's dates (RFC 3339) from dates without a time: a day, or an interval of days (`..` open).
pub fn rfc3339(datetime: &str) -> String {
    fn day(d: &str) -> bool {
        let b = d.as_bytes();
        b.len() == 10
            && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
    }
    let one = |d: &str, end: bool| -> String {
        let d = d.trim();
        if day(d) { format!("{d}{}", if end { "T23:59:59Z" } else { "T00:00:00Z" }) } else { d.to_string() }
    };
    if let Some((a, b)) = datetime.split_once('/') {
        return format!("{}/{}", one(a, false), one(b, true));
    }
    if day(datetime.trim()) {
        return format!("{}/{}", one(datetime, false), one(datetime, true));
    }
    datetime.to_string()
}

/// The body of the search: the collection over a box of longitudes and latitudes (and the dates, and a query on
/// the items' properties), a page of at most 100 items.
pub fn search_body(
    collection: &str,
    lon: [f64; 2],
    lat: [f64; 2],
    datetime: Option<&str>,
    query: Option<&Json>,
    limit: usize,
) -> Json {
    let mut body = json!({
        "collections": [collection],
        "bbox": [lon[0], lat[0], lon[1], lat[1]],
        "limit": limit.min(100),
    });
    if let Some(d) = datetime.filter(|d| !d.is_empty()) {
        body["datetime"] = Json::String(rfc3339(d));
    }
    if let Some(q) = query.filter(|q| !q.is_null() && q.as_object().is_none_or(|o| !o.is_empty())) {
        body["query"] = q.clone();
    }
    body
}

/// Python's truthiness of a JSON value (a link's `merge`).
fn truthy(v: &Json) -> bool {
    match v {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Json::String(s) => !s.is_empty(),
        Json::Array(a) => !a.is_empty(),
        Json::Object(o) => !o.is_empty(),
    }
}

/// The items of a search, the pages followed (the `next` link: a GET, or a POST with its body, merged with the
/// search's when it says so) until `limit` items or no more pages.
fn search(http: &mut dyn Http, url: &str, body: &Json, limit: usize) -> std::result::Result<Vec<Json>, String> {
    let mut items: Vec<Json> = Vec::new();
    let mut page = http.post(&format!("{}/search", url.trim_end_matches('/')), body)?;
    loop {
        if let Some(f) = page["features"].as_array() {
            items.extend(f.iter().cloned());
        }
        let next = page["links"].as_array().and_then(|l| l.iter().find(|x| x["rel"] == "next")).cloned();
        let Some(next) = next.filter(|_| items.len() < limit) else { break };
        let href = next["href"].as_str().ok_or("the catalogue's next page has no href")?;
        page = if next["method"].as_str().unwrap_or("GET").eq_ignore_ascii_case("POST") {
            let own = next.get("body").cloned();
            let sent = if truthy(&next["merge"]) {
                let mut b = body.clone();
                if let (Some(b), Some(Json::Object(o))) = (b.as_object_mut(), &own) {
                    b.extend(o.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
                b
            } else {
                own.unwrap_or_else(|| body.clone())
            };
            http.post(href, &sent)?
        } else {
            http.get(href)?
        };
    }
    items.truncate(limit);
    Ok(items)
}

/// The scenes (a cell each, NaN without value) combined cell by cell. `median`, `mean`, `min` and `max` take the
/// scenes with values at a cell (NaN when none has); `first` the first scene with a value. As numpy gives them:
/// with `single` (values, float32 in the library) the sums and means are float32's.
pub fn combine(stack: &[Vec<f64>], composite: &str, single: bool) -> Vec<f64> {
    let n = stack.first().map_or(0, Vec::len);
    let f = |v: f64| if single { f64::from(v as f32) } else { v };
    if composite == "first" {
        let mut a = stack[0].clone();
        for s in &stack[1..] {
            for (x, y) in a.iter_mut().zip(s) {
                if x.is_nan() {
                    *x = *y;
                }
            }
        }
        return a;
    }
    let mut vals: Vec<f64> = Vec::with_capacity(stack.len());
    (0..n)
        .map(|i| {
            vals.clear();
            vals.extend(stack.iter().map(|s| s[i]).filter(|v| !v.is_nan()));
            if vals.is_empty() {
                return f64::NAN;
            }
            match composite {
                "min" => vals.iter().copied().fold(f64::INFINITY, f64::min),
                "max" => vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                "mean" => {
                    // numpy: the sum in order, in the data's type, over the count (in double)
                    let sum = vals.iter().fold(0.0, |t, v| f(t + v));
                    f(sum / vals.len() as f64)
                }
                _ => {
                    vals.sort_by(|a, b| a.total_cmp(b));
                    let m = vals.len() / 2;
                    if vals.len() % 2 == 1 { vals[m] } else { f((vals[m - 1] + vals[m]) / 2.0) }
                }
            }
        })
        .collect()
}

/// The resampling of gdalwarp for a setting (the library's names too: `nearest`, `cubic_spline`), `auto` by the
/// data: the most common cell for classes, bilinear for values.
pub fn method(resampling: &str, classes: bool) -> std::result::Result<&'static str, String> {
    Ok(match resampling {
        "auto" if classes => "mode",
        "auto" | "bilinear" => "bilinear",
        "near" | "nearest" => "near",
        "cubic" => "cubic",
        "cubicspline" | "cubic_spline" => "cubicspline",
        "lanczos" => "lanczos",
        "average" => "average",
        "mode" => "mode",
        "max" => "max",
        "min" => "min",
        "med" => "med",
        "q1" => "q1",
        "q3" => "q3",
        "sum" => "sum",
        "rms" => "rms",
        r => return Err(format!("resampling: auto, near, bilinear, cubic, average, mode... not {r:?}")),
    })
}

/// An asset's file name: its name with the characters other than letters, digits and `_.-` as `_`.
pub fn file_stem(asset: &str) -> String {
    asset.chars().map(|c| if c.is_ascii_alphanumeric() || "_.-".contains(c) { c } else { '_' }).collect()
}

/// An asset's address made readable: the Planetary Computer's files (those on a blob storage of Azure) signed with
/// a token of their container, any other left as it is.
fn sign_href(
    href: &str,
    sign: &str,
    tokens: &mut Vec<(String, String)>,
    token_of: &dyn Fn(&str) -> std::result::Result<String, String>,
) -> std::result::Result<String, String> {
    let blob = href.strip_prefix("https://").is_some_and(|r| {
        r.split('/').next().is_some_and(|host| host.ends_with(".blob.core.windows.net")) && !r.contains('?')
    });
    if sign == "planetary_computer" && blob {
        crate::data::sas_sign(href, tokens, token_of)
    } else {
        Ok(href.to_string())
    }
}

/// The cache's folder of a request: a hash of the catalogue, collection, items, assets, composite, grid and
/// resampling.
fn folder(rq: &Request, ids: &[&str], g: &Grid) -> PathBuf {
    let key = format!(
        "{}|{}|{ids:?}|{:?}|{}|{}|{:?}{}{}{:?}|{}",
        rq.url,
        rq.collection,
        rq.assets,
        rq.composite,
        rq.classes,
        g.transform,
        g.width,
        g.height,
        crate::prepare::crs_of(g),
        rq.resampling
    );
    let hash: String = Sha256::digest(key.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect();
    crate::sdm::refdata::cache_dir().join("data").join(hash)
}

/// A layer written as a GeoTIFF (deflated, NaN for no data), through a file made beside it.
fn write_layer(path: &Path, g: &Grid, data: Vec<f64>, classes: bool) -> std::result::Result<(), String> {
    let dtype = if classes { DataType::F64 } else { DataType::F32 };
    let raster = Raster::single(g.clone(), data, dtype, Some(f64::NAN));
    let plain = path.with_extension("tmp.tif");
    terraflux_raster::tiff::write(&raster, &plain, &Default::default())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(wkt) = crate::prepare::crs_of(g).and_then(|c| terraflux_gdal::crs_wkt(&c).ok()) {
        terraflux_gdal::set_crs(&plain, &wkt)?;
    }
    let tmp = path.with_extension("part.tif");
    let args: Vec<String> =
        ["-ot", if classes { "Float64" } else { "Float32" }, "-co", "COMPRESS=DEFLATE"].map(String::from).to_vec();
    let r = terraflux_gdal::translate(&plain, &tmp, &args);
    let _ = std::fs::remove_file(&plain);
    r.map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The assets asked for: a name, or a list.
fn assets_of(args: &Args) -> Result<Vec<String>> {
    let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
    let v: Vec<String> = match args.params.get("assets") {
        Some(Json::String(s)) => vec![s.clone()],
        Some(Json::Null) | None => vec![],
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| fail(format!("assets: {e}")))?,
    };
    if v.is_empty() {
        return Err(fail("assets: the names of the assets to read (\"red\", \"nir\"...), one or a list".into()));
    }
    Ok(v)
}

/// What a node asks of the catalogue: its settings read and checked.
struct Request {
    url: String,
    collection: String,
    assets: Vec<String>,
    datetime: Option<String>,
    query: Option<Json>,
    limit: usize,
    composite: String,
    classes: bool,
    resampling: String,
    sign: String,
}

fn request(args: &Args) -> Result<Request> {
    let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
    let url: String = args.param("url", String::new())?;
    let collection: String = args.param("collection", String::new())?;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(fail("url: the catalogue's API, e.g. https://earth-search.aws.element84.com/v1".into()));
    }
    if collection.is_empty() {
        return Err(fail("collection: the collection's name, e.g. sentinel-2-l2a".into()));
    }
    let composite: String = args.param("composite", "median".to_string())?;
    if !COMPOSITES.contains(&composite.as_str()) {
        return Err(fail(format!("composite: median, mean, min, max or first, not {composite:?}")));
    }
    let sign: String = args.param("sign", "none".to_string())?;
    if !SIGNS.contains(&sign.as_str()) {
        return Err(fail(format!("sign: none or planetary_computer, not {sign:?}")));
    }
    let classes: bool = args.param("classes", false)?;
    let resampling: String = args.param("resampling", "auto".to_string())?;
    method(&resampling, classes).map_err(fail)?;
    let query = match args.opt_param::<String>("query")? {
        Some(q) if !q.trim().is_empty() => {
            let q: Json = serde_json::from_str(&q)
                .map_err(|e| fail(format!("query: JSON, e.g. {{\"eo:cloud_cover\": {{\"lt\": 20}}}}: {e}")))?;
            if !q.is_object() {
                return Err(fail("query: a JSON object, e.g. {\"eo:cloud_cover\": {\"lt\": 20}}".into()));
            }
            Some(q)
        }
        _ => None,
    };
    let limit: i64 = args.param("limit", 50)?;
    if limit < 1 {
        return Err(fail("limit: the most items, 1 or more".into()));
    }
    Ok(Request {
        url,
        collection,
        assets: assets_of(args)?,
        datetime: args.opt_param("datetime")?,
        query,
        limit: limit as usize,
        composite,
        classes,
        resampling,
        sign,
    })
}

struct Stac;

impl Operator for Stac {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "data.stac",
            "The scenes of a STAC catalogue over a grid, the data of Earth Search, the Planetary Computer and the \
             like (Sentinel-2, Landsat, elevation, land cover...): the catalogue's search gives the items of a \
             collection over the grid's box (and the dates, and a query on their properties: eo:cloud_cover below \
             20); every asset asked for of every item is read from the server (only the part the grid covers), \
             warped onto the grid, and the items combined cell by cell (the median of the cells with values, the \
             mean, the minimum, the maximum, or the first item with a value). A layer per asset, kept in the \
             cache (Terraflux's, TERRAFLUX_CACHE), made once. The values are the files' (Sentinel-2's \
             reflectances as integers: scale and offset are the catalogue's raster:bands). The Planetary \
             Computer's files need a token: sign = planetary_computer asks it, without a key.",
            Access::Global,
        )
        .input(PortSpec::optional("like", Type::Raster, "the map whose grid to fill (else extent and cellsize)"))
        .output(PortSpec::new("raster", Type::Raster, "a layer per asset on the grid, a file each"))
        .param(PortSpec::new(
            "url",
            Type::Text,
            "the catalogue's API, e.g. https://earth-search.aws.element84.com/v1 or \
             https://planetarycomputer.microsoft.com/api/stac/v1",
        ))
        .param(PortSpec::new("collection", Type::Text, "the collection, e.g. sentinel-2-l2a"))
        .param(PortSpec::new(
            "assets",
            Type::Any,
            "the assets' names (red, nir, SCL...), one or a list: a layer each, named by the asset",
        ))
        .param(PortSpec::optional(
            "datetime",
            Type::Text,
            "a day (2024-06-01) or an interval (2024-06-01/2024-08-31, .. open), or STAC's own RFC 3339 times",
        ))
        .param(PortSpec::optional(
            "query",
            Type::Text,
            "a query on the items' properties, as JSON: {\"eo:cloud_cover\": {\"lt\": 20}}",
        ))
        .param(PortSpec::optional("limit", Type::Integer, "the most items (default 50)"))
        .param(
            PortSpec::optional("composite", Type::Text, "how the items are combined (default median)")
                .choices(COMPOSITES),
        )
        .param(PortSpec::optional(
            "classes",
            Type::Bool,
            "the assets are classes (auto resampling: the most common cell)",
        ))
        .param(PortSpec::optional(
            "resampling",
            Type::Text,
            "auto (default: bilinear, the most common cell for classes), near, bilinear, cubic, average, mode... \
             (gdalwarp's)",
        ))
        .param(
            PortSpec::optional("sign", Type::Text, "none (default) or planetary_computer: the files' tokens")
                .choices(SIGNS),
        )
        .param(PortSpec::optional("extent", Type::Any, "without like: [west, south, east, north], degrees"))
        .param(PortSpec::optional("cellsize", Type::Number, "with extent: the cells' side, degrees"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        crate::prepare::need_gdal(args.node)?;
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let rq = request(args)?;
        if args.get("like").is_none() && args.params.get("extent").is_none_or(Json::is_null) {
            return Err(fail("connect a map whose grid to fill (like), or give extent and cellsize".into()));
        }
        let g = crate::data::grid_of(args)?;
        let (lon, lat) = crate::data::lonlat_box(&g)?;
        let body = search_body(&rq.collection, lon, lat, rq.datetime.as_deref(), rq.query.as_ref(), rq.limit);
        if !ctx.progress(None, format!("searching {}", rq.url)) {
            return Err(fail("stopped".into()));
        }
        let items = search(&mut Web::new(), &rq.url, &body, rq.limit)
            .map_err(|e| fail(format!("the STAC catalogue {}: {e}", rq.url)))?;
        if items.is_empty() {
            return Err(fail(format!("the catalogue has no item of {:?} over the grid", rq.collection)));
        }
        let ids: Vec<&str> = items.iter().map(|it| it["id"].as_str().unwrap_or("")).collect();
        let dir = folder(&rq, &ids, &g);
        std::fs::create_dir_all(&dir).map_err(|e| fail(format!("{}: {e}", dir.display())))?;
        let _ = std::fs::write(dir.join("items.json"), json!({"items": ids}).to_string());
        let (how, ot) =
            (method(&rq.resampling, rq.classes).map_err(fail)?, if rq.classes { "Float64" } else { "Float32" });
        // (the servers' files: a few tries, and no listing of the folder of each file)
        for (k, v) in [
            ("GDAL_DISABLE_READDIR_ON_OPEN", "EMPTY_DIR"),
            ("GDAL_HTTP_MAX_RETRY", "3"),
            ("GDAL_HTTP_RETRY_DELAY", "2"),
        ] {
            if terraflux_gdal::config(k).is_none() {
                let _ = terraflux_gdal::set_config(k, Some(v));
            }
        }
        let threads = crate::threads::for_this_thread();
        let t = g.transform;
        let (xmin, xmax) = (t[0], t[0] + t[1] * g.width as f64);
        let (ymax, ymin) = (t[3], t[3] + t[5] * g.height as f64);
        let mut tokens: Vec<(String, String)> = Vec::new();
        let mut files = Vec::new();
        for (a, asset) in rq.assets.iter().enumerate() {
            let out = dir.join(format!("{}.tif", file_stem(asset)));
            files.push(out.clone());
            if out.is_file() {
                continue;
            }
            let mut scenes: Vec<Vec<f64>> = Vec::new();
            for (k, it) in items.iter().enumerate() {
                let Some(href) = it["assets"][asset.as_str()]["href"].as_str() else { continue };
                let frac = (a as f64 + k as f64 / items.len() as f64) / rq.assets.len() as f64;
                if !ctx.progress(Some(frac), format!("reading {asset} of {}", ids[k])) {
                    return Err(fail("stopped".into()));
                }
                let href = sign_href(href, &rq.sign, &mut tokens, &crate::data::sas_token)
                    .map_err(|e| fail(format!("{asset} of {}: {e}", ids[k])))?;
                let path = if href.starts_with("http://") || href.starts_with("https://") {
                    format!("/vsicurl/{href}")
                } else {
                    href
                };
                let tmp = dir.join(format!(".{}-{}-{k}.tif", file_stem(asset), std::process::id()));
                // (the first band; no overview of the file: the library's reproject reads the full resolution)
                let mut w: Vec<String> =
                    ["-r", how, "-of", "GTiff", "-srcband", "1", "-ovr", "NONE", "-ot", ot, "-dstnodata", "nan"]
                        .map(String::from)
                        .to_vec();
                w.extend([
                    "-te".into(),
                    xmin.to_string(),
                    ymin.min(ymax).to_string(),
                    xmax.to_string(),
                    ymax.max(ymin).to_string(),
                    "-ts".into(),
                    g.width.to_string(),
                    g.height.to_string(),
                    "-wo".into(),
                    format!("NUM_THREADS={threads}"),
                ]);
                if let Some(c) = crate::prepare::crs_of(&g) {
                    w.extend(["-t_srs".into(), c]);
                }
                let read = terraflux_gdal::warp(&[Path::new(&path)], &tmp, &w).and_then(|()| {
                    let d = terraflux_gdal::Dataset::open(&tmp)?;
                    d.read(0, 0, 0, g.width, g.height, g.width, g.height)
                });
                let _ = std::fs::remove_file(&tmp);
                scenes.push(read.map_err(|e| fail(format!("{asset} of {}: {e}", ids[k])))?);
            }
            if scenes.is_empty() {
                return Err(fail(format!("no item has the asset {asset:?}")));
            }
            let data = combine(&scenes, &rq.composite, !rq.classes);
            write_layer(&out, &g, data, rq.classes).map_err(fail)?;
        }
        // used now; the downloads of other grids held under the cache's size, the least recently used removed
        crate::cache::touch(&dir);
        if let Some(all) = dir.parent() {
            crate::cache::prune(all, crate::cache::limit(), &[&dir]);
        }
        let name =
            if files.len() == 1 { files[0].to_string_lossy().into_owned() } else { dir.to_string_lossy().into_owned() };
        let r = Rasters::open_files_checked(&name, files, None, None, ctx.check())?;
        Ok(outputs([("raster", Value::Raster(Arc::new(r)))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A catalogue that answers from a list of pages, and remembers what it was asked.
    struct Fake {
        pages: Vec<Json>,
        asked: Vec<(String, String, Option<Json>)>,
    }

    impl Http for Fake {
        fn post(&mut self, url: &str, body: &Json) -> std::result::Result<Json, String> {
            self.asked.push(("POST".into(), url.into(), Some(body.clone())));
            Ok(self.pages.remove(0))
        }

        fn get(&mut self, url: &str) -> std::result::Result<Json, String> {
            self.asked.push(("GET".into(), url.into(), None));
            Ok(self.pages.remove(0))
        }
    }

    /// A page of a search as Earth Search answers it (recorded, cut down to two fields of its assets).
    const PAGE: &str = r#"{
        "type": "FeatureCollection",
        "features": [
            {"type": "Feature", "id": "S2B_31TCJ_20240607_0_L2A", "collection": "sentinel-2-l2a",
             "properties": {"datetime": "2024-06-07T10:50:00Z", "eo:cloud_cover": 3.2},
             "assets": {"red": {"href": "https://sentinel-cogs.s3.us-west-2.amazonaws.com/a/B04.tif"},
                        "nir": {"href": "https://sentinel-cogs.s3.us-west-2.amazonaws.com/a/B08.tif"}}},
            {"type": "Feature", "id": "S2A_31TCJ_20240602_0_L2A", "collection": "sentinel-2-l2a",
             "properties": {"datetime": "2024-06-02T10:50:00Z", "eo:cloud_cover": 11.0},
             "assets": {"red": {"href": "https://sentinel-cogs.s3.us-west-2.amazonaws.com/b/B04.tif"}}}
        ],
        "links": [{"rel": "next", "method": "POST", "href": "https://es.test/v1/search", "merge": true,
                   "body": {"next": "token2"}}]
    }"#;

    fn page(ids: &[&str], next: Option<Json>) -> Json {
        let features: Vec<Json> = ids.iter().map(|i| json!({"id": i})).collect();
        let links: Vec<Json> = next.into_iter().collect();
        json!({"features": features, "links": links})
    }

    #[test]
    fn the_body_of_the_search() {
        let q = json!({"eo:cloud_cover": {"lt": 20}});
        let b = search_body("sentinel-2-l2a", [1.0, 1.5], [41.0, 41.25], Some("2024-06-01/2024-08-31"), Some(&q), 250);
        assert_eq!(
            b,
            json!({
                "collections": ["sentinel-2-l2a"],
                "bbox": [1.0, 41.0, 1.5, 41.25],
                "limit": 100,
                "datetime": "2024-06-01T00:00:00Z/2024-08-31T23:59:59Z",
                "query": {"eo:cloud_cover": {"lt": 20}},
            })
        );
        // without dates and a query (or empty ones): neither key
        let b = search_body("c", [0.0, 1.0], [2.0, 3.0], Some(""), Some(&json!({})), 7);
        assert_eq!(b, json!({"collections": ["c"], "bbox": [0.0, 2.0, 1.0, 3.0], "limit": 7}));
    }

    #[test]
    fn dates_in_rfc_3339() {
        assert_eq!(rfc3339("2024-06-01"), "2024-06-01T00:00:00Z/2024-06-01T23:59:59Z");
        assert_eq!(rfc3339("2024-06-01/2024-08-31"), "2024-06-01T00:00:00Z/2024-08-31T23:59:59Z");
        assert_eq!(rfc3339("2024-06-01/.."), "2024-06-01T00:00:00Z/..");
        assert_eq!(rfc3339("../2024-08-31"), "../2024-08-31T23:59:59Z");
        assert_eq!(rfc3339("2024-06-01T10:00:00Z/2024-06-02T10:00:00Z"), "2024-06-01T10:00:00Z/2024-06-02T10:00:00Z");
        assert_eq!(rfc3339("2024-06-01T10:00:00Z"), "2024-06-01T10:00:00Z");
    }

    #[test]
    fn a_recorded_page_is_read_in_the_catalogues_order() {
        let mut http = Fake { pages: vec![serde_json::from_str(PAGE).unwrap()], asked: vec![] };
        // the limit reached by the first page: its next link is not followed
        let items = search(&mut http, "https://es.test/v1/", &json!({"limit": 2}), 2).unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["S2B_31TCJ_20240607_0_L2A", "S2A_31TCJ_20240602_0_L2A"]);
        assert_eq!(http.asked.len(), 1);
        assert_eq!(http.asked[0].1, "https://es.test/v1/search");
        assert_eq!(items[0]["assets"]["nir"]["href"], "https://sentinel-cogs.s3.us-west-2.amazonaws.com/a/B08.tif");
    }

    #[test]
    fn the_pages_are_followed_up_to_the_limit() {
        let body = json!({"collections": ["c"], "limit": 2});
        let get = json!({"rel": "next", "href": "https://es.test/v1/search?token=2"});
        let post =
            json!({"rel": "next", "method": "POST", "href": "https://es.test/v1/search", "body": {"token": "3"}});
        let merge = json!({"rel": "next", "method": "post", "href": "https://es.test/v1/search", "merge": true,
                           "body": {"token": "4", "limit": 1}});
        let mut http = Fake {
            pages: vec![
                page(&["a", "b"], Some(get)),
                page(&["c", "d"], Some(post)),
                page(&["e"], Some(merge)),
                page(&["f", "g"], None),
            ],
            asked: vec![],
        };
        let items = search(&mut http, "https://es.test/v1", &body, 6).unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["a", "b", "c", "d", "e", "f"]);
        // the first page POSTed; its link a GET; then a POST with its own body; then one merged with the search's
        assert_eq!(http.asked[0].0, "POST");
        assert_eq!((http.asked[1].0.as_str(), http.asked[1].1.as_str()), ("GET", "https://es.test/v1/search?token=2"));
        assert_eq!(http.asked[2].2, Some(json!({"token": "3"})));
        assert_eq!(http.asked[3].2, Some(json!({"collections": ["c"], "limit": 1, "token": "4"})));
        // and no more pages: all there are
        let mut http = Fake { pages: vec![page(&["a"], None)], asked: vec![] };
        assert_eq!(search(&mut http, "https://es.test/v1", &body, 50).unwrap().len(), 1);
    }

    #[test]
    fn the_files_of_the_planetary_computer_are_signed() {
        let asked = std::cell::RefCell::new(Vec::new());
        let token_of = |key: &str| -> std::result::Result<String, String> {
            asked.borrow_mut().push(key.to_string());
            Ok(format!("sv=fake&sig={}", key.replace('/', "-")))
        };
        let mut tokens = Vec::new();
        let a = "https://sentinel2l2a01.blob.core.windows.net/sentinel2-l2/x/B04.tif";
        let b = "https://sentinel2l2a01.blob.core.windows.net/sentinel2-l2/y/B08.tif";
        let s = sign_href(a, "planetary_computer", &mut tokens, &token_of).unwrap();
        assert_eq!(s, format!("{a}?sv=fake&sig=sentinel2l2a01-sentinel2-l2"));
        assert!(
            sign_href(b, "planetary_computer", &mut tokens, &token_of)
                .unwrap()
                .ends_with("sig=sentinel2l2a01-sentinel2-l2")
        );
        // one token a container
        assert_eq!(*asked.borrow(), ["sentinel2l2a01/sentinel2-l2"]);
        // not signed: asked not to, not on their storage, or already with a token
        assert_eq!(sign_href(b, "none", &mut tokens, &token_of).unwrap(), b);
        let other = "https://sentinel-cogs.s3.us-west-2.amazonaws.com/a/B04.tif";
        assert_eq!(sign_href(other, "planetary_computer", &mut tokens, &token_of).unwrap(), other);
        let signed = format!("{b}?sig=x");
        assert_eq!(sign_href(&signed, "planetary_computer", &mut tokens, &token_of).unwrap(), signed);
        assert_eq!(asked.borrow().len(), 1);
    }

    #[test]
    fn the_scenes_are_combined_as_numpy_does() {
        let n = f64::NAN;
        let stack = vec![vec![1.0, n, 4.0, n, 5.0], vec![3.0, 2.0, n, n, 1.0], vec![2.0, 6.0, 8.0, n, 9.0]];
        let same = |a: Vec<f64>, b: &[f64]| {
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(b) {
                assert!(x == y || (x.is_nan() && y.is_nan()), "{a:?} != {b:?}");
            }
        };
        // the cells with values only; none: NaN; an even count: the mean of the two in the middle
        same(combine(&stack, "median", true), &[2.0, 4.0, 6.0, n, 5.0]);
        same(combine(&stack, "min", true), &[1.0, 2.0, 4.0, n, 1.0]);
        same(combine(&stack, "max", true), &[3.0, 6.0, 8.0, n, 9.0]);
        same(combine(&stack, "mean", true), &[2.0, 4.0, 6.0, n, 5.0]);
        // the first with a value, in the catalogue's order
        same(combine(&stack, "first", true), &[1.0, 2.0, 4.0, n, 5.0]);
        // float32 sums: 0.1 + 0.2 + 0.3 in the single type is not the double's
        let s = vec![vec![0.1f32 as f64], vec![0.2f32 as f64], vec![0.3f32 as f64]];
        assert_eq!(combine(&s, "mean", true)[0], f64::from((0.1f32 + 0.2f32 + 0.3f32) / 3.0));
        assert_eq!(combine(&s, "median", true)[0], f64::from(0.2f32));
        // classes in double
        let c = vec![vec![1.0, 2.0], vec![2.0, 2.0], vec![2.0, n]];
        same(combine(&c, "median", false), &[2.0, 2.0]);
    }

    /// A tiny window of Sentinel-2 over Barcelona from Earth Search: `cargo test -p terraflux-ops --lib stac:: --
    /// --ignored`.
    #[test]
    #[ignore = "searches Earth Search and reads Sentinel-2 from AWS"]
    fn earth_search_gives_sentinel_2() {
        let reg = Registry::standard();
        let params = json!({
            "url": "https://earth-search.aws.element84.com/v1", "collection": "sentinel-2-l2a",
            "assets": ["red", "nir"], "datetime": "2024-07-01/2024-07-20", "query": "{\"eo:cloud_cover\": {\"lt\": 30}}",
            "limit": 3, "extent": [2.10, 41.38, 2.12, 41.40], "cellsize": 0.0005,
        });
        let params = params.as_object().cloned().unwrap();
        let args = Args { node: "n", inputs: Default::default(), params: &params };
        let out = reg.get("data.stac").unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{e}"));
        let Value::Raster(r) = &out["raster"] else { panic!() };
        let s = r.stack().unwrap();
        assert_eq!(s.keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(), ["red", "nir"]);
        for l in &s.layers {
            let v: Vec<f64> = l.iter().copied().filter(|x| x.is_finite()).collect();
            assert_eq!(l.len(), 40 * 40);
            // the reflectances as integers: a city's red a few hundreds to a few thousands
            assert!(v.len() > 1000, "{} cells with values", v.len());
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            assert!((100.0..6000.0).contains(&mean), "{mean}");
        }
        // (where the layers are, to compare them with the library's)
        println!("cache: {}", crate::sdm::refdata::cache_dir().join("data").display());
    }

    #[test]
    fn the_settings_are_checked() {
        let params = |v: Json| v.as_object().cloned().unwrap();
        let ok = params(json!({"url": "https://es.test/v1", "collection": "c", "assets": "red"}));
        let run =
            |p: &serde_json::Map<String, Json>| request(&Args { node: "n", inputs: Default::default(), params: p });
        let r = run(&ok).unwrap();
        assert_eq!(
            (r.assets.clone(), r.composite.as_str(), r.limit, r.sign.as_str()),
            (vec!["red".to_string()], "median", 50, "none")
        );
        let with = |k: &str, v: Json| {
            let mut p = ok.clone();
            p.insert(k.into(), v);
            p
        };
        for (k, v, says) in [
            ("composite", json!("mode"), "composite"),
            ("sign", json!("azure"), "sign"),
            ("resampling", json!("sharp"), "resampling"),
            ("query", json!("{not json"), "query"),
            ("query", json!("[1]"), "query"),
            ("limit", json!(0), "limit"),
            ("url", json!("ftp://x"), "url"),
            ("assets", json!([]), "assets"),
        ] {
            let e = run(&with(k, v.clone())).err().unwrap_or_else(|| panic!("{k} {v}")).to_string();
            assert!(e.contains(says), "{e}");
        }
        assert_eq!(run(&with("assets", json!(["red", "nir"]))).unwrap().assets, ["red", "nir"]);
        assert_eq!(file_stem("B04 (10m)/x"), "B04__10m__x");
        assert_eq!(method("auto", true), Ok("mode"));
        assert_eq!(method("auto", false), Ok("bilinear"));
        assert_eq!(method("cubic_spline", false), Ok("cubicspline"));
    }
}
