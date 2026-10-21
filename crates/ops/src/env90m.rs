//! Environment90m (Garcia Marquez et al. 2025): the environment of every sub-catchment of Hydrography90m, in tables
//! per 20° tile and variable on the IGB Berlin's server, as hydrographr's download_env90m_tables: the climate
//! (CHELSA, observed and projected), the land cover (ESA CCI, by year), the soil (SoilGrids), the elevation (MERIT
//! DEM), the aridity and evapotranspiration (CGIAR) and the hydrography's own variables, each summarised over the
//! sub-catchment (min, max, range, mean, sd).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Environment));
}

/// Environment90m's files on the IGB Berlin's server, by their path.
const SERVER: &str = "https://public.igb-berlin.de/index.php/s/zw56kEd25NsQqcQ/download?path=";

/// The lists of Environment90m's files, by their theme.
const LISTS: [&str; 7] = ["observedclimate", "projectedclimate", "landcover", "soil", "meritdem", "cgiar", "hydro"];

/// The statistics a table has of each sub-catchment.
const STATS: [&str; 5] = ["min", "max", "range", "mean", "sd"];

fn url(path: &str) -> String {
    format!("{SERVER}{}", format!("/{path}").replace('/', "%2F"))
}

fn cache() -> PathBuf {
    crate::sdm::refdata::cache_dir().join("env90m")
}

/// A file of the server in the cache, downloaded once.
fn cached(path: &str) -> std::result::Result<PathBuf, String> {
    let out = cache().join(path);
    if out.is_file() {
        return Ok(out);
    }
    let u = url(path);
    let (status, bytes) = terraflux_gdal::fetch(&u, &["TIMEOUT=600", "MAX_RETRY=3", "RETRY_DELAY=2"])?;
    if status != 200 && status != 0 {
        return Err(format!("{u}: HTTP {status}"));
    }
    let dir = out.parent().unwrap();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".download-{}", std::process::id()));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &out).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(out)
}

/// A file's variable and tile, from its name (`<variable>_<tile>.zip`).
pub fn variable_and_tile(path: &str) -> Option<(String, String)> {
    let name = path.rsplit('/').next()?.strip_suffix(".zip")?;
    let (var, tile) = name.rsplit_once('_')?;
    let t = tile.as_bytes();
    (t.len() == 6
        && t[0] == b'h'
        && t[3] == b'v'
        && tile[1..3].parse::<u8>().is_ok()
        && tile[4..].parse::<u8>().is_ok())
    .then(|| (var.to_string(), tile.to_string()))
}

/// Where each variable's table of each tile is on the server, from the lists of its files (downloaded once).
fn catalogue() -> std::result::Result<HashMap<(String, String), String>, String> {
    let mut out = HashMap::new();
    for list in LISTS {
        let f = cached(&format!("_file_lists/env90m_{list}_paths_file_sizes.txt"))?;
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        for line in text.lines().skip(1) {
            let path = line.split(';').next().unwrap_or("");
            if let Some(key) = variable_and_tile(path) {
                out.insert(key, path.to_string());
            }
        }
    }
    Ok(out)
}

/// The rows of a table (`subcID` and statistics, by spaces; `na` missing) whose sub-catchments are wanted: by
/// sub-catchment, the values of the columns asked for.
pub fn read_table(
    r: impl BufRead,
    wanted: &HashSet<u64>,
    stats: &[String],
) -> std::result::Result<HashMap<u64, Vec<f64>>, String> {
    let mut lines = r.lines();
    let header = lines.next().ok_or("an empty table")?.map_err(|e| e.to_string())?;
    let names: Vec<&str> = header.split_whitespace().collect();
    let at: Vec<usize> = stats
        .iter()
        .map(|s| {
            names.iter().position(|n| n == s).ok_or_else(|| format!("no column {s:?} (it has {})", names.join(", ")))
        })
        .collect::<std::result::Result<_, _>>()?;
    let mut out = HashMap::new();
    for line in lines {
        let line = line.map_err(|e| e.to_string())?;
        let mut parts = line.split_whitespace();
        let Some(id) = parts.next().and_then(|x| x.parse::<f64>().ok()) else { continue };
        let id = id as u64;
        if !wanted.contains(&id) {
            continue;
        }
        let values: Vec<&str> = std::iter::once("").chain(parts).collect();
        out.insert(id, at.iter().map(|&j| values.get(j).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN)).collect());
    }
    Ok(out)
}

struct Environment;

impl Operator for Environment {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "data.environment90m",
            "The environment of every sub-catchment of a map of Hydrography90m's sub-catchments (data.download's \
             hydrography90m), from Environment90m (Garcia Marquez et al. 2025), as hydrographr's \
             download_env90m_tables: the climate (CHELSA's bio01 to bio19, observed 1981-2010 and projected), the \
             land cover (ESA CCI's share of each class by year), the soil (SoilGrids), the elevation (MERIT DEM), \
             the aridity and evapotranspiration (CGIAR) and the hydrography's variables, each summarised over the \
             sub-catchment, in the units of their sources (CHELSA's temperatures in tenths of a kelvin: x 0.1 - \
             273.15 for degrees). The tables of the tiles over the map downloaded once into the cache (some are \
             hundreds of MB), the rows of the map's sub-catchments kept.",
            Access::Global,
        )
        .input(PortSpec::new("sub_catchments", Type::Map, "the sub-catchment of each cell (Hydrography90m's ids)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per sub-catchment of the map: subcID, then <variable>_<statistic> for each variable and statistic",
        ))
        .output(PortSpec::new("map", Type::Map, "the column to_map on the cells of each sub-catchment"))
        .param(PortSpec::new(
            "variables",
            Type::Any,
            "the variables, a name or a list, as hydrographr's: bio01_1981-2010_observed, \
             bio12_2071-2100_gfdl-esm4_ssp370_v2_1, c10_2020 (land cover class 10 in 2020), clyppt (clay), elev, \
             garid (aridity), slope_grad_dw_cel...",
        ))
        .param(PortSpec::optional("stats", Type::Any, "min, max, range, mean (default), sd, or a list"))
        .param(PortSpec::optional("to_map", Type::Text, "the column of the map (the first after subcID)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: String| Error::msg(format!("node {:?}: {e}", args.node));
        crate::prepare::need_gdal(args.node)?;
        let subc = args.map("sub_catchments")?;
        let list = |name: &str, default: &[&str]| -> Result<Vec<String>> {
            Ok(match args.params.get(name) {
                None | Some(serde_json::Value::Null) => default.iter().map(|s| s.to_string()).collect(),
                Some(serde_json::Value::String(s)) => {
                    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
                }
                Some(v) => serde_json::from_value(v.clone()).map_err(|e| fail(format!("{name}: {e}")))?,
            })
        };
        let variables = list("variables", &[])?;
        if variables.is_empty() {
            return Err(fail("name the variables".into()));
        }
        let stats = list("stats", &["mean"])?;
        if let Some(s) = stats.iter().find(|s| !STATS.contains(&s.as_str())) {
            return Err(fail(format!("unknown statistic {s:?} (min, max, range, mean, sd)")));
        }
        let wanted: HashSet<u64> = subc.data.iter().filter(|v| !v.is_nan() && **v > 0.0).map(|v| *v as u64).collect();
        let (lon, lat) = crate::data::lonlat_box(&subc.grid)?;
        let tiles = crate::data::h90m_tiles(lon, lat);
        if !ctx.progress(Some(0.0), "the lists of Environment90m's files") {
            return Err(fail("stopped".into()));
        }
        let files = catalogue().map_err(fail)?;
        let mut rows: BTreeMap<u64, Vec<f64>> = wanted.iter().map(|&id| (id, Vec::new())).collect();
        let mut columns = vec!["subcID".to_string()];
        for (i, var) in variables.iter().enumerate() {
            let paths: Vec<&String> = tiles.iter().filter_map(|t| files.get(&(var.clone(), t.clone()))).collect();
            if paths.is_empty() {
                let any = files.keys().any(|(v, _)| v == var);
                return Err(fail(if any {
                    format!("{var}: Environment90m has no table over the map (all sea?)")
                } else {
                    format!(
                        "{var}: not one of Environment90m's variables (as hydrographr names them: bio01_1981-2010_observed, c10_2020, clyppt, elev...)"
                    )
                }));
            }
            let mut found: HashMap<u64, Vec<f64>> = HashMap::new();
            for (k, path) in paths.iter().enumerate() {
                let f = (i as f64 + k as f64 / paths.len() as f64) / variables.len() as f64;
                if !ctx.progress(Some(f), format!("downloading {path}")) {
                    return Err(fail("stopped".into()));
                }
                let zip = cached(path).map_err(fail)?;
                let name = path.rsplit('/').next().unwrap().replace(".zip", ".txt");
                let inner = format!("/vsizip/{}/{name}", zip.to_string_lossy());
                let file = terraflux_gdal::VsiFile::open(&inner).map_err(fail)?;
                let got = read_table(std::io::BufReader::new(file), &wanted, &stats)
                    .map_err(|e| fail(format!("{path}: {e}")))?;
                for (id, v) in got {
                    found.entry(id).or_insert(v);
                }
            }
            for (id, row) in rows.iter_mut() {
                row.extend(found.get(id).cloned().unwrap_or_else(|| vec![f64::NAN; stats.len()]));
            }
            columns.extend(stats.iter().map(|s| format!("{var}_{s}")));
        }
        let mut table = Table::new(columns);
        table.rows =
            rows.iter().map(|(id, v)| std::iter::once(*id as f64).chain(v.iter().copied()).collect()).collect();
        let to_map: String = args.param("to_map", table.columns[1].clone())?;
        let j = table
            .columns
            .iter()
            .position(|c| *c == to_map)
            .ok_or_else(|| fail(format!("to_map: no column {to_map:?}")))?;
        let value: HashMap<u64, f64> = rows.iter().map(|(id, v)| (*id, v[j - 1])).collect();
        let map: Vec<f64> =
            subc.data
                .iter()
                .map(|v| {
                    if v.is_nan() || *v <= 0.0 {
                        f64::NAN
                    } else {
                        value.get(&(*v as u64)).copied().unwrap_or(f64::NAN)
                    }
                })
                .collect();
        Ok(outputs([("table", Value::Table(table)), ("map", Value::Map(Map::new(subc.grid.clone(), map)))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_files_variable_and_tile() {
        assert_eq!(
            variable_and_tile("esa_cci_landcover_v2_1_1/c100/c100_1992_h00v02.zip"),
            Some(("c100_1992".into(), "h00v02".into()))
        );
        assert_eq!(
            variable_and_tile(
                "chelsa_bioclim_v2_1/2041-2070_projected/bio01/bio01_2041-2070_ipsl-cm6a-lr_ssp126_v2_1_h34v12.zip"
            ),
            Some(("bio01_2041-2070_ipsl-cm6a-lr_ssp126_v2_1".into(), "h34v12".into()))
        );
        assert_eq!(variable_and_tile("README.txt"), None);
    }

    #[test]
    fn the_rows_of_the_sub_catchments_wanted() {
        let text = "subcID min max range mean sd\n476322246 0 1 1 0.1667 0.37\n476322247 0 1 1 0.8 0.39\n476322248 na na na na na\n";
        let wanted: HashSet<u64> = [476322247, 476322248, 9].into_iter().collect();
        let got = read_table(text.as_bytes(), &wanted, &["mean".into(), "sd".into()]).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[&476322247], [0.8, 0.39]);
        assert!(got[&476322248].iter().all(|v| v.is_nan()));
        assert!(read_table(text.as_bytes(), &wanted, &["median".into()]).is_err());
    }
}
