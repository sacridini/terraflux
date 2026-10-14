//! The geographic reference data of the cleaning of occurrence records (landschaft's reference_data):
//! CoordinateCleaner's tables (the centroids and capitals of countries and provinces, biodiversity institutions,
//! artificial hotspots; GPL-3, carried inside Terraflux, tools/make_sdm_refdata.py) and the Natural Earth layers
//! CoordinateCleaner uses (land, urban areas, countries; public domain), downloaded on first use and kept in the
//! cache, pinned by their SHA-256.

use std::io::Read;
use std::path::PathBuf;

use sha2::{Digest, Sha256};
use terraflux_raster::vector::{Column, Geometry, Vector};

/// A table of CoordinateCleaner's: its columns, the fields of every row (None missing).
pub struct RefTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

impl RefTable {
    fn index(&self, name: &str) -> usize {
        self.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no column {name}"))
    }

    pub fn numbers(&self, name: &str) -> Vec<f64> {
        let j = self.index(name);
        self.rows.iter().map(|r| r[j].as_deref().and_then(|s| s.parse().ok()).unwrap_or(f64::NAN)).collect()
    }

    pub fn texts(&self, name: &str) -> Vec<Option<String>> {
        let j = self.index(name);
        self.rows.iter().map(|r| r[j].clone()).collect()
    }

    /// The rows whose value in `column` is one `keep` takes.
    pub fn filter_by(&self, column: &str, keep: impl Fn(Option<&str>) -> bool) -> RefTable {
        let j = self.index(column);
        let rows = self.rows.iter().filter(|r| keep(r[j].as_deref())).cloned().collect();
        RefTable { columns: self.columns.clone(), rows }
    }

    /// Points at the columns `x` and `y`, with the other columns as attributes (lon/lat).
    pub fn points(&self, x: &str, y: &str) -> Vector {
        let (xs, ys) = (self.numbers(x), self.numbers(y));
        let geometries = xs
            .iter()
            .zip(&ys)
            .map(|(&a, &b)| if a.is_finite() && b.is_finite() { Geometry::Point([a, b]) } else { Geometry::Empty })
            .collect();
        let attributes = self
            .columns
            .iter()
            .filter(|c| *c != x && *c != y)
            .map(|c| {
                let t = self.texts(c);
                let col = if t.iter().flatten().all(|s| s.parse::<f64>().is_ok()) {
                    Column::Numbers(self.numbers(c))
                } else {
                    Column::Texts(t.into_iter().map(Option::unwrap_or_default).collect())
                };
                (c.clone(), col)
            })
            .collect();
        Vector { geometries, attributes, crs: Some("EPSG:4326".into()) }
    }
}

const COUNTRYREF: &[u8] = include_bytes!("../../data/sdm/countryref.csv.gz");
const INSTITUTIONS: &[u8] = include_bytes!("../../data/sdm/institutions.csv.gz");
const AOHI: &[u8] = include_bytes!("../../data/sdm/aohi.csv.gz");

/// The names of the reference data sets.
pub const NAMES: [&str; 8] =
    ["countryref", "institutions", "aohi", "land110", "land50", "land10", "urban50", "countries50"];

/// One of CoordinateCleaner's tables: countryref, institutions or aohi.
pub fn table(name: &str) -> Result<RefTable, String> {
    let bytes = match name {
        "countryref" => COUNTRYREF,
        "institutions" => INSTITUTIONS,
        "aohi" => AOHI,
        _ => return Err(format!("no reference table {name:?} (countryref, institutions, aohi)")),
    };
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes).read_to_string(&mut text).map_err(|e| format!("{name}: {e}"))?;
    let mut rows = crate::bio::basedata::read_csv(&text, ',', '"');
    if rows.is_empty() {
        return Err(format!("{name}: empty"));
    }
    let columns = rows.remove(0).into_iter().map(Option::unwrap_or_default).collect();
    Ok(RefTable { columns, rows })
}

const NE: &str = "https://naciscdn.org/naturalearth";

/// A Natural Earth layer: its address, the SHA-256 of the zip and the layer.
fn natural_earth_source(name: &str) -> Option<(String, &'static str, &'static str)> {
    let (path, sha, layer) = match name {
        "land110" => (
            "110m/physical/ne_110m_land.zip",
            "1926c621afd6ac67c3f36639bb1236134a48d82226dc675d3e3df53d02d2a3de",
            "ne_110m_land",
        ),
        "land50" => (
            "50m/physical/ne_50m_land.zip",
            "0b8e670cf80dce9cbebe2a193bc44ba5602758c22e1fa603980553646d7ff162",
            "ne_50m_land",
        ),
        "land10" => (
            "10m/physical/ne_10m_land.zip",
            "e547d749445eaa0964aba76738090ec88f5e63c4585122170f98c67a7ea922dc",
            "ne_10m_land",
        ),
        "urban50" => (
            "50m/cultural/ne_50m_urban_areas.zip",
            "69e916a46e663eefe8469cf4154bd34ff9486fb91e4a54762dbe85c1bbfb912b",
            "ne_50m_urban_areas",
        ),
        "countries50" => (
            "50m/cultural/ne_50m_admin_0_countries.zip",
            "5fed433373581fa648920435f937d95f2d3c0200e067409c6478dcdf1b853139",
            "ne_50m_admin_0_countries",
        ),
        _ => return None,
    };
    Some((format!("{NE}/{path}"), sha, layer))
}

/// The folder of the data downloaded on first use: `TERRAFLUX_CACHE`, else the platform's cache
/// (`%LOCALAPPDATA%\Terraflux\cache`, `~/Library/Caches/Terraflux`, `$XDG_CACHE_HOME/terraflux`).
pub fn cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("TERRAFLUX_CACHE") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Terraflux")
            .join("cache")
    } else if cfg!(target_os = "macos") {
        home.unwrap_or_else(std::env::temp_dir).join("Library").join("Caches").join("Terraflux")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".cache")))
            .unwrap_or_else(std::env::temp_dir)
            .join("terraflux")
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A file of the cache, downloaded first when missing or changed (its SHA-256 checked).
fn fetch(name: &str, url: &str, sha: &str) -> Result<PathBuf, String> {
    let path = cache_dir().join("sdm").join(name);
    if let Ok(bytes) = std::fs::read(&path)
        && sha256(&bytes) == sha
    {
        return Ok(path);
    }
    let (status, bytes) = terraflux_gdal::fetch(url, &["TIMEOUT=120", "MAX_RETRY=2", "RETRY_DELAY=2"])
        .map_err(|e| format!("{e} (without internet, copy the file to {})", path.display()))?;
    if status != 200 && status != 0 {
        return Err(format!("{url}: HTTP {status} (without internet, copy the file to {})", path.display()));
    }
    let got = sha256(&bytes);
    if got != sha {
        return Err(format!("{url}: checksum mismatch (got {got}, expected {sha})"));
    }
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".download-{}-{name}", std::process::id()));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// A Natural Earth layer (land110, land50, land10, urban50, countries50), its field names in lower case.
pub fn natural_earth(name: &str) -> Result<Vector, String> {
    let (url, sha, layer) = natural_earth_source(name)
        .ok_or_else(|| format!("no reference layer {name:?} (land110, land50, land10, urban50, countries50)"))?;
    let zip = fetch(url.rsplit('/').next().unwrap(), &url, sha)?;
    let mut v = terraflux_gdal::read_vector(format!("/vsizip/{}/{layer}.shp", zip.to_string_lossy()), None)?;
    for (n, _) in &mut v.attributes {
        *n = n.to_lowercase();
    }
    Ok(v)
}

/// A reference data set as features: the tables as points (countryref at the centroids), the layers as
/// polygons.
pub fn features(name: &str) -> Result<Vector, String> {
    match name {
        "countryref" => Ok(table(name)?.points("centroid.lon", "centroid.lat")),
        "institutions" | "aohi" => Ok(table(name)?.points("decimalLongitude", "decimalLatitude")),
        _ => natural_earth(name),
    }
}
