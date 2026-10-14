//! Occurrence records from GBIF's occurrence search API (landschaft's download_gbif): the species matched to
//! GBIF's backbone (a synonym gives the accepted name's records), the pages of 300 records asked a few at a time
//! (up to the API's 100 000), the Darwin Core terms kept, by GDAL's HTTP.

use serde_json::{Value as Json, json};
use terraflux_raster::vector::{Column, Geometry, Vector};

pub const API: &str = "https://api.gbif.org/v1";
/// The API's largest page.
const PAGE: usize = 300;
/// The search API's offset limit.
const MAX_RECORDS: usize = 100_000;

/// The Darwin Core terms kept (the search API's name, the column) and whether they are numbers.
const FIELDS: [(&str, &str, bool); 22] = [
    ("key", "gbifID", true),
    ("species", "species", false),
    ("scientificName", "scientificName", false),
    ("taxonKey", "taxonKey", true),
    ("decimalLongitude", "decimalLongitude", true),
    ("decimalLatitude", "decimalLatitude", true),
    ("coordinateUncertaintyInMeters", "coordinateUncertaintyInMeters", true),
    ("coordinatePrecision", "coordinatePrecision", true),
    ("countryCode", "countryCode", false),
    ("stateProvince", "stateProvince", false),
    ("locality", "locality", false),
    ("elevation", "elevation", true),
    ("year", "year", true),
    ("eventDate", "eventDate", false),
    ("basisOfRecord", "basisOfRecord", false),
    ("occurrenceStatus", "occurrenceStatus", false),
    ("institutionCode", "institutionCode", false),
    ("collectionCode", "collectionCode", false),
    ("catalogNumber", "catalogNumber", false),
    ("recordedBy", "recordedBy", false),
    ("datasetKey", "datasetKey", false),
    ("license", "license", false),
];

/// A request to the API: the path under it and the query's pairs, its JSON answer.
pub type Get<'a> = dyn Fn(&str, &[(String, String)]) -> Result<Json, String> + Sync + 'a;

/// Python's quote_plus: letters, digits and _.-~ kept, spaces as +, the rest %XX of their UTF-8 bytes.
fn quote_plus(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// GBIF's API through GDAL's HTTP.
pub fn http_get(path: &str, query: &[(String, String)]) -> Result<Json, String> {
    let q: Vec<String> = query.iter().map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v))).collect();
    let url = format!("{API}/{path}?{}", q.join("&"));
    let (status, body) =
        terraflux_gdal::fetch(&url, &["TIMEOUT=60", "MAX_RETRY=3", "RETRY_DELAY=2", "USERAGENT=Terraflux"])?;
    if status != 200 && status != 0 {
        return Err(format!("{url}: HTTP {status}"));
    }
    serde_json::from_slice(&body).map_err(|e| format!("{url}: {e}"))
}

/// The species: a scientific name or a taxon key.
pub enum Species {
    Name(String),
    Key(i64),
}

/// The query.
#[derive(Default)]
pub struct Query {
    pub country: Vec<String>,
    /// The bounding box of an area: x0, y0, x1, y1 (lon/lat).
    pub bbox: Option<[f64; 4]>,
    pub year: Option<String>,
    pub basis_of_record: Vec<String>,
    pub max_uncertainty: Option<f64>,
    pub limit: Option<usize>,
    pub has_geospatial_issue: bool,
    pub workers: usize,
}

/// Python's repr of a float (the WKT of the box as landschaft writes it).
fn py_float(v: f64) -> String {
    let s = format!("{v:?}");
    if s.contains('e') {
        // Python writes 1e-05 and 1e+20
        let (m, e) = s.split_once('e').unwrap();
        let e: i32 = e.parse().unwrap();
        format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        s
    }
}

fn taxon_key(species: &Species, get: &Get) -> Result<(i64, Json), String> {
    match species {
        Species::Key(k) => Ok((*k, json!({ "usageKey": k }))),
        Species::Name(name) => {
            let m = get("species/match", &[("name".into(), name.clone()), ("verbose".into(), "false".into())])?;
            let none = matches!(m.get("matchType").and_then(Json::as_str), None | Some("NONE"));
            let Some(usage) = m.get("usageKey").and_then(Json::as_i64).filter(|_| !none) else {
                return Err(format!("GBIF found no taxon named {name:?}"));
            };
            Ok((m.get("acceptedUsageKey").and_then(Json::as_i64).unwrap_or(usage), m))
        }
    }
}

/// The records of the species with coordinates, as points (lon/lat) with the Darwin Core terms and their issues;
/// and the query, the taxon matched and the count.
pub fn download(species: &Species, q: &Query, get: &Get) -> Result<(Vector, Json), String> {
    let (key, matched) = taxon_key(species, get)?;
    let mut params: Vec<(String, String)> = vec![
        ("taxonKey".into(), key.to_string()),
        ("hasCoordinate".into(), "true".into()),
        ("hasGeospatialIssue".into(), q.has_geospatial_issue.to_string()),
    ];
    params.extend(q.country.iter().map(|c| ("country".to_string(), c.clone())));
    if let Some([x0, y0, x1, y1]) = q.bbox {
        let (x0, y0, x1, y1) = (py_float(x0), py_float(y0), py_float(x1), py_float(y1));
        params.push(("geometry".into(), format!("POLYGON(({x0} {y0},{x1} {y0},{x1} {y1},{x0} {y1},{x0} {y0}))")));
    }
    if let Some(y) = &q.year {
        params.push(("year".into(), y.clone()));
    }
    params.extend(q.basis_of_record.iter().map(|b| ("basisOfRecord".to_string(), b.clone())));
    params.push(("occurrenceStatus".into(), "PRESENT".into()));
    let page = |offset: usize| {
        let mut p = params.clone();
        p.push(("limit".into(), PAGE.to_string()));
        p.push(("offset".into(), offset.to_string()));
        get("occurrence/search", &p)
    };
    let first = page(0)?;
    let count = first.get("count").and_then(Json::as_u64).unwrap_or(0) as usize;
    let total = count.min(MAX_RECORDS).min(q.limit.unwrap_or(MAX_RECORDS));
    let offsets: Vec<usize> = (PAGE..total).step_by(PAGE).collect();
    let workers = q.workers.max(1);
    let mut pages = vec![first];
    for chunk in offsets.chunks(workers) {
        let got: Vec<Result<Json, String>> = std::thread::scope(|s| {
            let handles: Vec<_> = chunk.iter().map(|&o| s.spawn(move || page(o))).collect();
            handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a request failed".into()))).collect()
        });
        for g in got {
            pages.push(g?);
        }
    }
    let rows: Vec<&Json> = pages
        .iter()
        .flat_map(|p| p.get("results").and_then(Json::as_array).map(|r| r.iter()).into_iter().flatten())
        .take(total)
        .collect();
    let fetched = rows.len();
    let number = |v: Option<&Json>| match v {
        Some(Json::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Json::String(s)) => s.trim().parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    let text = |v: Option<&Json>| match v {
        None | Some(Json::Null) => String::new(),
        Some(Json::String(s)) => s.clone(),
        Some(v) => v.to_string(),
    };
    // the records kept: within the uncertainty, the first of each gbifID
    let mut seen = std::collections::HashSet::new();
    let kept: Vec<&Json> = rows
        .into_iter()
        .filter(|r| {
            let u = number(r.get("coordinateUncertaintyInMeters"));
            q.max_uncertainty.is_none_or(|m| u.is_nan() || u <= m)
        })
        .filter(|r| seen.insert(number(r.get("key")).to_bits()))
        .collect();
    let mut attributes: Vec<(String, Column)> = FIELDS
        .iter()
        .map(|&(k, col, num)| {
            let c = if num {
                Column::Numbers(kept.iter().map(|r| number(r.get(k))).collect())
            } else {
                Column::Texts(kept.iter().map(|r| text(r.get(k))).collect())
            };
            (col.to_string(), c)
        })
        .collect();
    let issues = kept
        .iter()
        .map(|r| {
            r.get("issues")
                .and_then(Json::as_array)
                .map(|a| a.iter().map(|v| text(Some(v))).collect::<Vec<_>>().join(";"))
                .unwrap_or_default()
        })
        .collect();
    attributes.push(("issue".into(), Column::Texts(issues)));
    let geometries = kept
        .iter()
        .map(|r| {
            let (x, y) = (number(r.get("decimalLongitude")), number(r.get("decimalLatitude")));
            if x.is_finite() && y.is_finite() { Geometry::Point([x, y]) } else { Geometry::Empty }
        })
        .collect();
    let query: serde_json::Map<String, Json> = params.iter().fold(serde_json::Map::new(), |mut m, (k, v)| {
        match m.get_mut(k) {
            Some(Json::Array(a)) => a.push(json!(v)),
            Some(old) => *old = json!([old.clone(), v]),
            None => {
                m.insert(k.clone(), json!(v));
            }
        }
        m
    });
    let info = json!({"taxonKey": key, "match": matched, "query": query, "count": count,
                      "truncated": count > fetched});
    Ok((Vector { geometries, attributes, crs: Some("EPSG:4326".into()) }, info))
}
