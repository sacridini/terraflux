//! Circuitscape's and Omniscape's INI files (landschaft's circuitscape_ini, network_ini and omniscape_ini): a run
//! configured as Circuitscape.jl and Omniscape.jl read their files, the inputs read as they read them (ESRI ASCII
//! grids in double, GeoTIFFs, text lists), the outputs written as they name them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::{Band, DataType, Grid, Raster};

use crate::circuit::{
    Circuit, FocalPoints, OneToAllOptions, habitat_conductance, one_to_all, read_included_pairs, read_text,
};
use crate::files::Rasters;
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(CircuitscapeIni));
    r.add(Arc::new(OmniscapeIni));
}

const NODATA: f64 = -9999.0;
const TRUELIST: [&str; 3] = ["True", "true", "1"];
const FALSELIST: [&str; 3] = ["False", "false", "0"];

/// An INI file as Circuitscape's and Omniscape's parse_cfg read it: lines starting with `[` skipped, every other
/// split at its first `=` (the key right-stripped, the value stripped); a later key replaces an earlier one in
/// its place.
pub fn parse_ini(text: &str) -> Vec<(String, String)> {
    let mut cfg: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if line.starts_with('[') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        set(&mut cfg, k.trim_end(), v.trim());
    }
    cfg
}

fn set(cfg: &mut Vec<(String, String)>, k: &str, v: &str) {
    match cfg.iter_mut().find(|(key, _)| key == k) {
        Some(e) => e.1 = v.to_string(),
        None => cfg.push((k.to_string(), v.to_string())),
    }
}

fn get<'a>(cfg: &'a [(String, String)], k: &str) -> &'a str {
    cfg.iter().find(|(key, _)| key == k).map_or("", |(_, v)| v.as_str())
}

const CS_DEFAULTS: [(&str, &str); 43] = [
    ("version", "unknown"),
    ("connect_four_neighbors_only", "False"),
    ("connect_using_avg_resistances", "False"),
    ("use_polygons", "False"),
    ("polygon_file", "(Browse for a short-circuit region file)"),
    ("source_file", "(Browse for a current source file)"),
    ("ground_file", "(Browse for a ground point file)"),
    ("ground_file_is_resistances", "True"),
    ("use_unit_currents", "False"),
    ("use_direct_grounds", "False"),
    ("remove_src_or_gnd", "keepall"),
    ("use_mask", "False"),
    ("mask_file", "None"),
    ("data_type", "raster"),
    ("scenario", "not entered"),
    ("habitat_file", "(Browse for a resistance file)"),
    ("habitat_map_is_resistances", "True"),
    ("solver", "cg+amg"),
    ("parallelize", "False"),
    ("precision", "double"),
    ("use_variable_source_strengths", "False"),
    ("variable_source_file", "None"),
    ("use_included_pairs", "False"),
    ("included_pairs_file", "(Browse for a file with pairs to include or exclude)"),
    ("point_file", "(Browse for file with locations of focal points or regions)"),
    ("use_reclass_table", "False"),
    ("reclass_file", "(Browse for file with reclassification data)"),
    ("output_file", "(Choose a base name for output files)"),
    ("write_cur_maps", "False"),
    ("write_volt_maps", "False"),
    ("write_cum_cur_map_only", "False"),
    ("write_max_cur_maps", "False"),
    ("set_null_currents_to_nodata", "False"),
    ("set_null_voltages_to_nodata", "False"),
    ("set_focal_node_currents_to_zero", "False"),
    ("compress_grids", "False"),
    ("log_transform_maps", "False"),
    ("write_as_tif", "False"),
    // (the rest only exist for completeness of the list)
    ("low_memory_mode", "False"),
    ("preemptive_memory_release", "False"),
    ("print_timings", "False"),
    ("print_rusages", "False"),
    ("max_parallel", "0"),
];

fn unset(path: &str) -> bool {
    path.is_empty() || path == "None" || path.starts_with('(')
}

fn flag(cfg: &[(String, String)], k: &str) -> Result<bool> {
    let v = get(cfg, k);
    if TRUELIST.contains(&v) {
        Ok(true)
    } else if FALSELIST.contains(&v) {
        Ok(false)
    } else {
        Err(Error::msg(format!("{k} = \"{v}\": expected True or False")))
    }
}

/// Circuitscape reads paths from the working directory; failing that, next to the INI file.
fn resolve(name: &str, base: &Path) -> PathBuf {
    let p = Path::new(name);
    if p.is_absolute() || p.exists() {
        return p.to_path_buf();
    }
    let q = base.join(p);
    if q.exists() { q } else { p.to_path_buf() }
}

/// The grid of a habitat raster as Circuitscape reads it.
#[derive(Debug, Clone)]
pub struct RasterMeta {
    pub nrows: usize,
    pub ncols: usize,
    pub xllcorner: f64,
    pub yllcorner: f64,
    pub cellsize: f64,
    pub transform: [f64; 6],
    pub wkt: String,
}

impl RasterMeta {
    fn grid(&self) -> Grid {
        let mut g = Grid::new(self.ncols, self.nrows);
        g.transform = self.transform;
        if !self.wkt.is_empty() {
            g.geo.wkt = Some(self.wkt.clone());
        }
        g
    }
}

/// Circuitscape's _guess_file_type.
fn file_type(path: &Path) -> Result<&'static str> {
    let bytes = std::fs::read(path).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
    let gz = path.to_string_lossy().to_lowercase().ends_with("gz");
    let head: Vec<u8> = if gz {
        use std::io::Read;
        let mut v = Vec::new();
        let _ = flate2::read::GzDecoder::new(&bytes[..]).take(4096).read_to_end(&mut v);
        v
    } else {
        bytes.iter().take(4096).copied().collect()
    };
    if head.len() >= 4 && head[2..4] == [0x2a, 0x00] {
        return Ok("geotiff");
    }
    let line = String::from_utf8_lossy(&head);
    let line = line.lines().next().unwrap_or("");
    Ok(if line.starts_with("\u{93}NUMPY") || head.starts_with(b"\x93NUMPY") {
        "npy"
    } else if line.to_lowercase().starts_with("ncols") {
        "aagrid"
    } else if line.starts_with("min") {
        "pairs_aagrid"
    } else if line.starts_with("mode") {
        "pairs"
    } else {
        "list"
    })
}

const ASC_KEYS: [&str; 10] =
    ["ncols", "nrows", "xllcorner", "yllcorner", "xllcenter", "yllcenter", "cellsize", "dx", "dy", "nodata_value"];

/// read_asc: the header, then one row per line, parsed as double.
fn read_asc(path: &Path) -> Result<(Vec<f64>, RasterMeta)> {
    let text = read_text(&path.to_string_lossy())?;
    let mut header: HashMap<String, f64> = HashMap::new();
    let mut values = Vec::new();
    let mut rows = 0;
    for line in text.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.is_empty() {
            continue;
        }
        if rows == 0 && toks.len() == 2 && ASC_KEYS.contains(&toks[0].to_lowercase().as_str()) {
            header.insert(toks[0].to_lowercase(), parse_num(toks[1])?);
            continue;
        }
        for t in toks {
            values.push(parse_num(t)?);
        }
        rows += 1;
    }
    let h = |k: &str| header.get(k).copied().ok_or_else(|| Error::msg(format!("{}: no {k}", path.display())));
    let (nrows, ncols) = (h("nrows")? as usize, h("ncols")? as usize);
    if rows != nrows || values.len() != nrows * ncols {
        return Err(Error::msg(format!(
            "{}: the ASCII grid does not hold {nrows} rows of {ncols} values",
            path.display()
        )));
    }
    let dx = header.get("dx").or(header.get("cellsize")).copied().unwrap_or(f64::NAN);
    let dy = header.get("dy").copied().unwrap_or(dx);
    let xll = match header.get("xllcorner") {
        Some(v) => *v,
        None => h("xllcenter")? - dx / 2.0,
    };
    let yll = match header.get("yllcorner") {
        Some(v) => *v,
        None => h("yllcenter")? - dy / 2.0,
    };
    let nodata = header.get("nodata_value").copied().unwrap_or(NODATA);
    for v in values.iter_mut() {
        if *v == nodata || v.is_nan() {
            *v = NODATA;
        }
    }
    let s = path.to_string_lossy();
    let stem = s.strip_suffix(".gz").unwrap_or(&s);
    let prj = PathBuf::from(format!("{}.prj", &stem[..stem.len().saturating_sub(4)]));
    let wkt = std::fs::read_to_string(&prj).map(|t| t.trim().to_string()).unwrap_or_default();
    Ok((
        values,
        RasterMeta {
            nrows,
            ncols,
            xllcorner: xll,
            yllcorner: yll,
            cellsize: dx,
            transform: [xll, dx, 0.0, yll + nrows as f64 * dy, 0.0, -dy],
            wkt,
        },
    ))
}

/// Python's float() of a token.
fn parse_num(t: &str) -> Result<f64> {
    match t.to_lowercase().trim_start_matches(['+', '-']) {
        "inf" | "infinity" => Ok(if t.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY }),
        "nan" => Ok(f64::NAN),
        _ => t.parse().map_err(|_| Error::msg(format!("not a number: {t:?}"))),
    }
}

/// Circuitscape's read_raster: an ESRI ASCII grid (in double) or any GDAL raster, no data as -9999.
fn read_raster(path: &Path) -> Result<(Vec<f64>, RasterMeta)> {
    if !path.is_file() {
        return Err(Error::msg(format!("the file \"{}\" does not exist", path.display())));
    }
    let s = path.to_string_lossy().to_lowercase();
    if s.strip_suffix(".gz").unwrap_or(&s).ends_with(".asc") {
        return read_asc(path);
    }
    let name =
        if s.ends_with(".gz") { format!("/vsigzip/{}", path.display()) } else { path.to_string_lossy().into_owned() };
    let r = Rasters::open(&name, None, None)?;
    let m = r.map()?;
    let g = &m.grid;
    let t = g.transform;
    let values: Vec<f64> = m.data.iter().map(|v| if v.is_nan() { NODATA } else { *v }).collect();
    let wkt = crate::prepare::crs_of(g).unwrap_or_default();
    let nrows = g.height;
    Ok((
        values,
        RasterMeta {
            nrows,
            ncols: g.width,
            xllcorner: t[0],
            yllcorner: t[3] - nrows as f64 * t[1],
            cellsize: t[1],
            transform: t,
            wkt,
        },
    ))
}

/// A text list of numbers per line.
fn read_list(path: &Path, ncol: usize) -> Result<Vec<Vec<f64>>> {
    let text = read_text(&path.to_string_lossy())?;
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split_whitespace().map(parse_num).collect::<Result<_>>())
        .collect::<Result<_>>()?;
    if rows.iter().any(|r| r.len() != ncol) {
        return Err(Error::msg(format!("{} must have exactly {ncol} columns", path.display())));
    }
    Ok(rows)
}

/// 1-based row and column of (x, y) points: ceil(nrows - (y - yll) / cellsize), ceil((x - xll) / cellsize).
fn list_cells(points: &[Vec<f64>], meta: &RasterMeta) -> Vec<(i64, i64)> {
    points
        .iter()
        .map(|p| {
            (
                (meta.nrows as f64 - (p[2] - meta.yllcorner) / meta.cellsize).ceil() as i64,
                ((p[1] - meta.xllcorner) / meta.cellsize).ceil() as i64,
            )
        })
        .collect()
}

/// Circuitscape's read_polymap: a raster on the habitat's grid, no data as `nodata_as`.
fn read_polymap(path: &Path, integer: bool, nodata_as: Option<f64>) -> Result<Vec<f64>> {
    let (mut a, _) = read_raster(path)?;
    if integer && a.iter().any(|v| *v != v.round()) {
        return Err(Error::msg(format!("{} contains non-integer values", path.display())));
    }
    if let Some(n) = nodata_as {
        a.iter_mut().filter(|v| **v == NODATA).for_each(|v| *v = n);
    }
    Ok(a)
}

/// Circuitscape's read_point_map: a raster of ids or a text list of id x y; negative ids dropped, sorted by id
/// (stably).
fn read_point_map(path: &Path, meta: &RasterMeta) -> Result<FocalPoints> {
    let (nx, ny) = (meta.ncols, meta.nrows);
    let mut found: Vec<(i64, i64, f64)> = Vec::new();
    if file_type(path)? == "list" {
        let pts = read_list(path, 3)?;
        for (p, (i, j)) in pts.iter().zip(list_cells(&pts, meta)) {
            found.push((i, j, p[0]));
        }
    } else {
        let a = read_polymap(path, true, Some(0.0))?;
        for j in 0..nx {
            for i in 0..ny {
                let v = a[i * nx + j];
                if v != 0.0 {
                    found.push((i as i64 + 1, j as i64 + 1, v));
                }
            }
        }
    }
    found.retain(|f| f.2 >= 0.0);
    found.sort_by(|a, b| a.2.total_cmp(&b.2));
    if found.iter().any(|&(i, j, _)| i < 0 || j < 0 || i > ny as i64 || j > nx as i64) {
        return Err(Error::msg("At least one focal node location falls outside of habitat map"));
    }
    let mut ids: Vec<i64> = found.iter().map(|f| f.2 as i64).collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() < 2 {
        return Err(Error::msg("Less than two valid focal nodes found. Please check focal node location file."));
    }
    Ok(FocalPoints {
        rows: found.iter().map(|f| (f.0 - 1) as usize).collect(),
        cols: found.iter().map(|f| (f.1 - 1) as usize).collect(),
        ids: found.iter().map(|f| f.2 as i64).collect(),
    })
}

/// read_source_and_ground_maps: sources and grounds by cell (grounds as conductances).
fn source_and_ground(
    src_path: &Path,
    gnd_path: &Path,
    meta: &RasterMeta,
    ground_is_res: bool,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let n = meta.nrows * meta.ncols;
    let at = |i: i64, j: i64| -> Result<usize> {
        if i < 1 || j < 1 || i > meta.nrows as i64 || j > meta.ncols as i64 {
            return Err(Error::msg("a point falls outside of the habitat map"));
        }
        Ok((i - 1) as usize * meta.ncols + (j - 1) as usize)
    };
    let mut gnd = match file_type(gnd_path)? {
        "aagrid" | "geotiff" => read_polymap(gnd_path, false, None)?,
        "list" => {
            let pts = read_list(gnd_path, 3)?;
            let mut g = vec![NODATA; n];
            for (p, (i, j)) in pts.iter().zip(list_cells(&pts, meta)) {
                g[at(i, j)?] = p[0];
            }
            g
        }
        _ => return Err(Error::msg("Cannot recognise file type.")),
    };
    let src = match file_type(src_path)? {
        "aagrid" | "geotiff" => {
            let mut s = read_polymap(src_path, false, Some(0.0))?;
            s.iter_mut().filter(|v| **v == NODATA).for_each(|v| *v = 0.0);
            s
        }
        "list" => {
            let pts = read_list(src_path, 3)?;
            let mut s = vec![0.0; n];
            for (p, (i, j)) in pts.iter().zip(list_cells(&pts, meta)) {
                s[at(i, j)?] = p[0];
            }
            s
        }
        _ => return Err(Error::msg("Cannot recognize file type.")),
    };
    for v in gnd.iter_mut() {
        if *v == NODATA {
            *v = 0.0;
        } else if ground_is_res {
            *v = 1.0 / *v;
        }
    }
    Ok((src, gnd))
}

/// C++'s std::to_chars of a double: the shortest round trip, fixed or scientific, whichever is shorter (fixed on
/// a tie); scientific as printf's (an exponent of two digits at least).
pub fn to_chars(v: f64) -> String {
    if v == 0.0 {
        return if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    if !v.is_finite() {
        return if v.is_nan() {
            "nan".into()
        } else if v > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    let e = format!("{:e}", v.abs());
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let sign = if v < 0.0 { "-" } else { "" };
    let sci = format!("{sign}{mant}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    let nd = digits.len() as i32;
    let fixed = if exp >= nd - 1 {
        format!("{sign}{digits}{}", "0".repeat((exp - nd + 1) as usize))
    } else if exp >= 0 {
        let (a, b) = digits.split_at((exp + 1) as usize);
        format!("{sign}{a}.{b}")
    } else {
        format!("{sign}0.{}{digits}", "0".repeat((-exp - 1) as usize))
    };
    if fixed.len() <= sci.len() { fixed } else { sci }
}

/// A grid written as Circuitscape writes it: an ESRI ASCII grid (and its .prj) or a GeoTIFF, no data -9999.
fn write_grid(prefix: &str, values: &[f64], meta: &RasterMeta, tif: bool) -> Result<PathBuf> {
    let out: Vec<f64> = values.iter().map(|v| if v.is_nan() { NODATA } else { *v }).collect();
    if tif {
        let path = PathBuf::from(format!("{prefix}.tif"));
        let r = Raster {
            grid: meta.grid(),
            bands: vec![Band { name: None, data: out }],
            dtype: DataType::F64,
            nodata: Some(NODATA),
        };
        crate::prepare::write_with_crs(&r, &path)?;
        return Ok(path);
    }
    let py = crate::bio::names::py_float;
    let mut text = format!(
        "ncols         {}\nnrows         {}\nxllcorner     {}\nyllcorner     {}\ncellsize      {}\nNODATA_value  -9999\n",
        meta.ncols,
        meta.nrows,
        py(meta.xllcorner),
        py(meta.yllcorner),
        py(meta.cellsize)
    );
    for row in out.chunks(meta.ncols) {
        text.push_str(&row.iter().map(|v| to_chars(*v)).collect::<Vec<_>>().join(" "));
        text.push('\n');
    }
    let path = PathBuf::from(format!("{prefix}.asc"));
    std::fs::write(&path, text).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
    if !meta.wkt.is_empty() {
        let _ = std::fs::write(format!("{prefix}.prj"), &meta.wkt);
    }
    Ok(path)
}

fn write_matrix(path: &str, rows: &[Vec<f64>], sep: &str) -> Result<()> {
    let py = crate::bio::names::py_float;
    let text: String = rows.iter().map(|r| r.iter().map(|v| py(*v)).collect::<Vec<_>>().join(sep) + "\n").collect();
    std::fs::write(path, text).map_err(|e| Error::msg(format!("{path}: {e}")))
}

/// The results of a Circuitscape run on rasters.
#[derive(Default)]
pub struct CsResult {
    /// Advanced: the voltages and currents of every cell (0 without a node).
    pub voltage: Option<Vec<f64>>,
    pub current: Option<Vec<f64>>,
    /// Pairwise: the matrix with the ids in the first row and column; one-to-all: id, value rows.
    pub resistances: Option<Vec<Vec<f64>>>,
    pub cum: Option<Vec<f64>>,
    pub max: Option<Vec<f64>>,
    /// Per pair or point: voltmap_<i>_<j>, curmap_<i>_<j> (or _<i>).
    pub maps: Vec<(String, Vec<f64>)>,
    pub meta: Option<RasterMeta>,
}

/// Run a Circuitscape INI file on rasters (landschaft's circuitscape_ini): `write` writes the outputs as
/// Circuitscape; `batch` the one-to-all problems solved together.
pub fn circuitscape_ini(path: &Path, write: bool, batch: usize) -> Result<CsResult> {
    let base = path.canonicalize().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
    let mut cfg: Vec<(String, String)> =
        CS_DEFAULTS[..38].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    for (k, v) in parse_ini(&read_text(&path.to_string_lossy())?) {
        set(&mut cfg, &k, &v);
    }
    let data_type = get(&cfg, "data_type");
    if data_type == "network" || data_type == "Network" {
        return Err(Error::msg("data_type = network: use circuit.network_advanced or circuit.network_pairwise"));
    }
    if data_type != "raster" && data_type != "Raster" {
        return Err(Error::msg(format!("data_type = \"{data_type}\": expected raster or network")));
    }
    let scenario = match get(&cfg, "scenario") {
        "pairwise" | "Pairwise" => "pairwise",
        "advanced" | "Advanced" => "advanced",
        "one-to-all" | "one_to_all" => "one-to-all",
        "all-to-one" | "all_to_one" => "all-to-one",
        s => {
            return Err(Error::msg(format!(
                "scenario = \"{s}\" is not one of advanced, all-to-one, one-to-all, pairwise"
            )));
        }
    };
    for k in ["low_memory_mode", "preemptive_memory_release"] {
        if TRUELIST.contains(&get(&cfg, k)) {
            return Err(Error::msg(format!("{k} is not implemented in Circuitscape.jl; remove it or set it to False")));
        }
    }
    let need = |k: &str| -> Result<PathBuf> {
        let v = get(&cfg, k);
        if unset(v) {
            return Err(Error::msg(format!("{k} is not set")));
        }
        let p = resolve(v, &base);
        if !p.is_file() {
            return Err(Error::msg(format!("{k} = \"{v}\" does not exist")));
        }
        Ok(p)
    };
    let (cells, meta) = read_raster(&need("habitat_file")?)?;
    let reclass: Option<Vec<(f64, f64)>> = if flag(&cfg, "use_reclass_table")? {
        Some(read_list(&need("reclass_file")?, 2)?.into_iter().map(|r| (r[0], r[1])).collect())
    } else {
        None
    };
    let mut g = habitat_conductance(&cells, flag(&cfg, "habitat_map_is_resistances")?, reclass.as_deref())?;
    let poly: Vec<i64> = if flag(&cfg, "use_polygons")? {
        read_polymap(&need("polygon_file")?, true, Some(0.0))?.into_iter().map(|v| v as i64).collect()
    } else {
        Vec::new()
    };
    if flag(&cfg, "use_mask")? {
        let m = read_polymap(&need("mask_file")?, false, Some(0.0))?;
        for (x, k) in g.iter_mut().zip(&m) {
            *x *= if *k > 0.0 { 1.0 } else { 0.0 };
        }
        if g.iter().sum::<f64>() == 0.0 {
            return Err(Error::msg("Mask file deleted everything!"));
        }
    }
    let four = flag(&cfg, "connect_four_neighbors_only")?;
    let avg = flag(&cfg, "connect_using_avg_resistances")?;
    let policy = match get(&cfg, "remove_src_or_gnd") {
        "keepall" | "not entered" => 0,
        "rmvsrc" => 1,
        "rmvgnd" => 2,
        "rmvall" => 3,
        p => {
            return Err(Error::msg(format!(
                "remove_src_or_gnd must be one of keepall, rmvsrc, rmvgnd, rmvall, not {p:?}"
            )));
        }
    };
    let (nx, ny) = (meta.ncols, meta.nrows);
    let out_file = get(&cfg, "output_file").to_string();
    let prefix = || -> Result<String> {
        if unset(&out_file) {
            return Err(Error::msg("output_file is not set"));
        }
        if let Some(d) = Path::new(&out_file).parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d).map_err(|e| Error::msg(format!("{}: {e}", d.display())))?;
        }
        Ok(out_file.split(".out").next().unwrap_or("").to_string())
    };
    let tif = flag(&cfg, "write_as_tif")?;
    let mut result = CsResult { meta: Some(meta.clone()), ..Default::default() };
    match scenario {
        "advanced" => {
            let (mut src, mut gnd) = source_and_ground(
                &need("source_file")?,
                &need("ground_file")?,
                &meta,
                flag(&cfg, "ground_file_is_resistances")?,
            )?;
            if flag(&cfg, "use_unit_currents")? {
                src.iter_mut().filter(|v| **v != 0.0).for_each(|v| *v = 1.0);
            }
            if flag(&cfg, "use_direct_grounds")? {
                gnd.iter_mut().filter(|v| **v != 0.0).for_each(|v| *v = f64::INFINITY);
            }
            let r = ffi::circuit_cs_advanced(&g, &poly, nx as i64, ny as i64, &src, &gnd, avg, four, policy)?;
            let by_cell =
                |v: &[f64]| r.nodemap.iter().map(|&n| if n >= 0 { v[n as usize] } else { 0.0 }).collect::<Vec<f64>>();
            result.voltage = Some(by_cell(&r.voltage));
            result.current = Some(by_cell(&r.current));
        }
        "pairwise" => {
            let mut points = read_point_map(&need("point_file")?, &meta)?;
            let pairs = if flag(&cfg, "use_included_pairs")? {
                Some(read_included_pairs(&read_text(&need("included_pairs_file")?.to_string_lossy())?, false)?)
            } else {
                None
            };
            let excluded = match &pairs {
                Some(p) => {
                    if p.mode == "include" {
                        points.retain(&p.ids);
                    }
                    p.excluded()
                }
                None => Default::default(),
            };
            let (volt, cur, cum_only) =
                (flag(&cfg, "write_volt_maps")?, flag(&cfg, "write_cur_maps")?, flag(&cfg, "write_cum_cur_map_only")?);
            let (cum, max) = (cur || cum_only, flag(&cfg, "write_max_cur_maps")?);
            let flags = [
                u8::from(cum),
                u8::from(max),
                u8::from(flag(&cfg, "set_focal_node_currents_to_zero")?),
                u8::from(flag(&cfg, "set_null_currents_to_nodata")?),
                u8::from(flag(&cfg, "set_null_voltages_to_nodata")?),
                u8::from(flag(&cfg, "log_transform_maps")?),
                u8::from(volt),
                u8::from(cur && !cum_only),
            ];
            let regions = points.has_regions();
            let ids = if regions { points.unique() } else { points.ids.clone() };
            let n = ids.len();
            let ex: Vec<u8> = (0..n * n).map(|k| u8::from(excluded.contains(&(ids[k / n], ids[k % n])))).collect();
            let cells: Vec<i64> = points.rows.iter().zip(&points.cols).map(|(r, c)| (r * nx + c) as i64).collect();
            let r = ffi::circuit_pairwise(
                &g,
                &poly,
                nx as i64,
                ny as i64,
                &cells,
                if regions { &points.ids } else { &[] },
                &ex,
                avg,
                four,
                &flags,
            )?;
            let mut res = vec![vec![0.0; n + 1]; n + 1];
            for (k, id) in ids.iter().enumerate() {
                res[0][k + 1] = *id as f64;
                res[k + 1][0] = *id as f64;
                res[k + 1][1..].copy_from_slice(&r.resistances[k * n..(k + 1) * n]);
            }
            result.resistances = Some(res);
            result.cum = cum.then(|| r.cum.clone());
            result.max = max.then(|| r.max.clone());
            let cellsn = nx * ny;
            let (mut vk, mut ck) = (0, 0);
            for k in 0..r.pair_ci.len() {
                let name = format!("_{}_{}", ids[r.pair_ci[k] as usize], ids[r.pair_cj[k] as usize]);
                if volt {
                    result.maps.push((format!("voltmap{name}"), r.pair_volt[vk..vk + cellsn].to_vec()));
                    vk += cellsn;
                }
                if cur && !cum_only {
                    result.maps.push((format!("curmap{name}"), r.pair_cur[ck..ck + cellsn].to_vec()));
                    ck += cellsn;
                }
            }
        }
        _ => {
            let points = read_point_map(&need("point_file")?, &meta)?;
            let pairs = if flag(&cfg, "use_included_pairs")? {
                Some(read_included_pairs(&read_text(&need("included_pairs_file")?.to_string_lossy())?, false)?)
            } else {
                None
            };
            let strengths = if flag(&cfg, "use_variable_source_strengths")? {
                let mut t = read_list(&need("variable_source_file")?, 2)?;
                if t.iter().map(|r| r[0]).fold(f64::INFINITY, f64::min) == 0.0 {
                    t.iter_mut().for_each(|r| r[0] += 1.0);
                }
                Some(t.into_iter().map(|r| (r[0], r[1])).collect())
            } else {
                None
            };
            let cum_only = flag(&cfg, "write_cum_cur_map_only")?;
            let cur = flag(&cfg, "write_cur_maps")? || cum_only;
            let o = OneToAllOptions {
                one_to_all: scenario == "one-to-all",
                strengths,
                pairs,
                cum: cur,
                max: flag(&cfg, "write_max_cur_maps")?,
                focal_zero: flag(&cfg, "set_focal_node_currents_to_zero")?,
                volt_maps: flag(&cfg, "write_volt_maps")?,
                cur_maps: cur,
                avg_resistances: avg,
                four_neighbors: four,
                batch,
            };
            let mut c = Circuit { g: g.clone(), poly: poly.clone(), points, nx, ny };
            let r = one_to_all(&mut c, &o)?;
            result.resistances = Some(r.ids.iter().zip(&r.values).map(|(i, v)| vec![*i as f64, *v]).collect());
            result.cum = o.cum.then_some(r.cum);
            result.max = o.max.then_some(r.max);
            for (n, v, cmap) in r.maps {
                if let Some(v) = v {
                    result.maps.push((format!("voltmap_{n}"), v));
                }
                if let Some(cmap) = cmap {
                    result.maps.push((format!("curmap_{n}"), cmap));
                }
            }
        }
    }
    if write {
        let prefix = prefix()?;
        let text: String = cfg.iter().map(|(k, v)| format!("{k} = {v}\n")).collect();
        std::fs::write(&out_file, text).map_err(|e| Error::msg(format!("{out_file}: {e}")))?;
        if scenario == "advanced" {
            if flag(&cfg, "write_volt_maps")? {
                write_grid(&format!("{prefix}_voltmap"), result.voltage.as_ref().unwrap(), &meta, tif)?;
            }
            if flag(&cfg, "write_cur_maps")? || flag(&cfg, "write_cum_cur_map_only")? {
                write_grid(&format!("{prefix}_curmap"), result.current.as_ref().unwrap(), &meta, tif)?;
            }
        } else {
            for (name, v) in &result.maps {
                write_grid(&format!("{prefix}_{name}"), v, &meta, tif)?;
            }
            if let Some(c) = &result.cum {
                write_grid(&format!("{prefix}_cum_curmap"), c, &meta, tif)?;
            }
            if let Some(m) = &result.max {
                write_grid(&format!("{prefix}_max_curmap"), m, &meta, tif)?;
            }
            if scenario == "pairwise" {
                let r = result.resistances.as_ref().unwrap();
                write_matrix(&format!("{prefix}_resistances.out"), r, " ")?;
                let fp: Vec<f64> = r[1..].iter().map(|row| row[0]).collect();
                let mut three = Vec::new();
                for i in 0..fp.len() {
                    for j in i + 1..fp.len() {
                        three.push(vec![fp[i], fp[j], r[j + 1][i + 1]]);
                    }
                }
                write_matrix(&format!("{prefix}_resistances_3columns.out"), &three, " ")?;
            }
        }
    }
    Ok(result)
}

struct CircuitscapeIni;

impl Operator for CircuitscapeIni {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "circuit.circuitscape_ini",
            "Run a Circuitscape INI file on rasters, as Circuitscape.compute(path) does (landschaft's \
             circuitscape_ini): advanced, pairwise, one-to-all and all-to-one, with its polygons, mask, \
             reclassification, included pairs and variable source strengths; the inputs read as Circuitscape \
             reads them (ASCII grids in double, GeoTIFFs, text lists; paths from the working directory or next to \
             the INI), the outputs written as it names them next to output_file (ASCII grids, or GeoTIFF with \
             write_as_tif). solver, precision and parallelize have no effect: every solve is exact in double.",
            Access::None,
        )
        .output(PortSpec::new(
            "resistances",
            Type::Table,
            "pairwise: a row per point and its resistances; one-to-all: id, value",
        ))
        .output(PortSpec::new("voltage", Type::Map, "advanced: the voltages"))
        .output(PortSpec::new("current", Type::Map, "advanced: the currents"))
        .output(PortSpec::new("cum", Type::Map, "the cumulative current"))
        .output(PortSpec::new("max", Type::Map, "the largest current"))
        .output(PortSpec::new("maps", Type::Stack, "the pairs' (or points') voltage and current maps"))
        .param(PortSpec::new("path", Type::Text, "the INI file"))
        .param(PortSpec::optional("write_outputs", Type::Bool, "write the output files as Circuitscape (default true)"))
        .param(PortSpec::optional(
            "batch",
            Type::Integer,
            "one-to-all problems solved together (default 4 x the threads)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let path: String = args.param("path", String::new())?;
        let threads = crate::threads::ensure();
        let batch = args.opt_param::<usize>("batch")?.unwrap_or(4 * threads).max(1);
        let r = circuitscape_ini(Path::new(&path), args.param("write_outputs", true)?, batch).map_err(fail)?;
        let grid = Arc::new(r.meta.as_ref().unwrap().grid());
        let map = |v: Vec<f64>| Value::Map(Map::new(grid.clone(), v));
        let mut out = Outputs::new();
        if let Some(rows) = r.resistances {
            let pairwise = rows.first().is_some_and(|r| r.len() > 2)
                || rows.first().is_some_and(|r| r[0] == 0.0 && rows.len() > 1 && r.len() == rows.len());
            let mut t = if pairwise {
                Table::new(
                    std::iter::once("id".to_string()).chain(rows[0][1..].iter().map(|v| v.to_string())).collect(),
                )
            } else {
                Table::new(vec!["id".into(), "value".into()])
            };
            t.rows = if pairwise { rows[1..].to_vec() } else { rows };
            out.insert("resistances".into(), Value::Table(t));
        }
        for (k, v) in [("voltage", r.voltage), ("current", r.current), ("cum", r.cum), ("max", r.max)] {
            if let Some(v) = v {
                out.insert(k.into(), map(v));
            }
        }
        if !r.maps.is_empty() {
            out.insert(
                "maps".into(),
                Value::Stack(Stack {
                    grid: grid.clone(),
                    keys: r.maps.iter().map(|(k, _)| Key::Name(k.clone())).collect(),
                    layers: r.maps.into_iter().map(|(_, v)| Arc::new(v)).collect(),
                }),
            );
        }
        Ok(out)
    }
}

const OMNI_TRUE: [&str; 3] = ["true", "True", "TRUE"];
const OMNI_DEFAULTS: [(&str, &str); 38] = [
    ("resistance_file", ""),
    ("resistance_is_conductance", "false"),
    ("source_file", ""),
    ("parallelize", "true"),
    ("parallel_batch_size", "10"),
    ("block_size", "1"),
    ("buffer", "0"),
    ("source_threshold", "0"),
    ("source_from_resistance", "false"),
    ("r_cutoff", "Inf"),
    ("precision", "double"),
    ("connect_four_neighbors_only", "false"),
    ("solver", "cg+amg"),
    ("calc_flow_potential", "false"),
    ("calc_normalized_current", "false"),
    ("correct_artifacts", "true"),
    ("write_raw_currmap", "true"),
    ("write_as_tif", "true"),
    ("mask_nodata", "true"),
    ("suppress_cs_messages", "true"),
    ("conditional", "false"),
    ("n_conditions", "1"),
    ("compare_to_future", "none"),
    ("condition1_file", ""),
    ("condition2_file", ""),
    ("condition1_future_file", ""),
    ("condition2_future_file", ""),
    ("comparison1", "within"),
    ("comparison2", "within"),
    ("condition1_lower", "0"),
    ("condition2_lower", "0"),
    ("condition1_upper", "0"),
    ("condition2_upper", "0"),
    ("reclassify_resistance", "false"),
    ("reclass_table", ""),
    ("write_reclassified_resistance", "false"),
    ("allow_different_projections", "false"),
    ("project_name", ""),
];

/// Julia's parse(Float64, ...): Inf, -Inf and NaN too.
fn julia_float(v: &str) -> Result<f64> {
    parse_num(&v.replace("Inf", "inf"))
}

/// The project folder of an Omniscape run: the name, or name_1, name_2... when it exists.
fn project_dir(name: &str) -> Result<PathBuf> {
    let base = PathBuf::from(name);
    let dir = if !base.is_dir() {
        base
    } else {
        let mut k = 1;
        while Path::new(&format!("{name}_{k}")).is_dir() {
            k += 1;
        }
        PathBuf::from(format!("{name}_{k}"))
    };
    std::fs::create_dir_all(&dir).map_err(|e| Error::msg(format!("{}: {e}", dir.display())))?;
    Ok(dir)
}

struct OmniscapeIni;

impl Operator for OmniscapeIni {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "circuit.omniscape_ini",
            "Run an Omniscape INI file, as run_omniscape(path) does (landschaft's omniscape_ini): every option of \
             Omniscape 0.7 with its defaults (radius, resistance_file and project_name required), paths from the \
             working directory or next to the INI; the outputs written to project_name (_1, _2... when it exists) \
             as Omniscape writes them: config.ini, cum_currmap, flow_potential, normalized_cum_currmap, \
             classified_resistance (GeoTIFF or ASCII grids, no data -9999).",
            Access::None,
        )
        .output(PortSpec::new("current", Type::Map, "the cumulative current (cum_currmap)"))
        .output(PortSpec::optional("flow_potential", Type::Map, "with calc_flow_potential"))
        .output(PortSpec::optional("normalized", Type::Map, "with calc_normalized_current"))
        .output(PortSpec::optional("classified_resistance", Type::Map, "with reclassify_resistance"))
        .param(PortSpec::new("path", Type::Text, "the INI file"))
        .param(PortSpec::optional(
            "write_outputs",
            Type::Bool,
            "write the outputs to the project folder (default true)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let path: String = args.param("path", String::new())?;
        let path = Path::new(&path);
        let base = path.canonicalize().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
        let user = parse_ini(&read_text(&path.to_string_lossy()).map_err(fail)?);
        let missing: Vec<&str> = ["resistance_file", "radius", "project_name"]
            .into_iter()
            .filter(|k| !user.iter().any(|(u, _)| u == k))
            .collect();
        if !missing.is_empty() {
            return Err(fail(Error::msg(format!(
                "The following arguments are missing from your .ini with no defaults: {}",
                missing.join(" ")
            ))));
        }
        let mut cfg: Vec<(String, String)> =
            OMNI_DEFAULTS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        for (k, v) in &user {
            set(&mut cfg, k, v);
        }
        let flag = |k: &str| OMNI_TRUE.contains(&get(&cfg, k));
        let single = matches!(get(&cfg, "precision"), "single" | "Single" | "SINGLE");
        let read = |k: &str| -> Result<Map> {
            let p = resolve(get(&cfg, k), &base);
            if !p.exists() {
                return Err(Error::msg(format!("the file \"{}\" does not exist", p.display())));
            }
            let mut m = Rasters::open(&p.to_string_lossy(), None, None)?.map()?;
            if single {
                m = Map::new(m.grid.clone(), m.data.iter().map(|v| *v as f32 as f64).collect());
            }
            Ok(m)
        };
        let resistance = read("resistance_file").map_err(fail)?;
        let mut inputs: Vec<(String, Value)> = vec![("resistance".into(), Value::Map(resistance.clone()))];
        let num = |k: &str| julia_float(get(&cfg, k));
        let mut params = serde_json::Map::new();
        params.insert("radius".into(), serde_json::json!(num("radius").map_err(fail)?));
        for (k, p) in [
            ("block_size", "block_size"),
            ("buffer", "buffer"),
            ("source_threshold", "source_threshold"),
            ("r_cutoff", "r_cutoff"),
        ] {
            let v = num(k).map_err(fail)?;
            // (an infinite cutoff is the operator's default)
            if v.is_finite() {
                params.insert(p.into(), serde_json::json!(v));
            }
        }
        for (k, p) in [
            ("resistance_is_conductance", "resistance_is_conductance"),
            ("connect_four_neighbors_only", "four_neighbors"),
            ("calc_flow_potential", "flow_potential"),
            ("calc_normalized_current", "normalized_current"),
            ("correct_artifacts", "correct_artifacts"),
            ("mask_nodata", "mask_nodata"),
        ] {
            params.insert(p.into(), serde_json::json!(flag(k)));
        }
        params.insert("precision".into(), serde_json::json!(if single { "single" } else { "double" }));
        let reclass: Option<Vec<(f64, f64)>> = if flag("reclassify_resistance") {
            // Omniscape's reclass table: two columns, "missing" (quoted or not) no data
            let text = read_text(&resolve(get(&cfg, "reclass_table"), &base).to_string_lossy()).map_err(fail)?;
            let mut rows = Vec::new();
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() != 2 {
                    return Err(fail(Error::msg(format!("reclass table lines need two columns: {:?}", line.trim()))));
                }
                let v = |p: &str| if p.trim_matches('"') == "missing" { Ok(f64::NAN) } else { parse_num(p) };
                rows.push((v(parts[0]).map_err(fail)?, v(parts[1]).map_err(fail)?));
            }
            Some(rows)
        } else {
            None
        };
        if let Some(t) = &reclass {
            params.insert(
                "reclass_table".into(),
                serde_json::json!(
                    t.iter()
                        .map(|(a, b)| serde_json::json!([
                            a,
                            if b.is_nan() { serde_json::Value::Null } else { serde_json::json!(b) }
                        ]))
                        .collect::<Vec<_>>()
                ),
            );
        }
        if !flag("source_from_resistance") {
            if get(&cfg, "source_file").is_empty() {
                return Err(fail(Error::msg(
                    "You did not provide a source raster file path. Set source_from_resistance to true in your config \
                     if you want to generate source strength from the resistance layer.",
                )));
            }
            inputs.push(("source".into(), Value::Map(read("source_file").map_err(fail)?)));
        }
        if flag("conditional") {
            let compare = get(&cfg, "compare_to_future").to_lowercase();
            let n_cond = num("n_conditions").map_err(fail)?.round() as i64;
            inputs.push(("condition1".into(), Value::Map(read("condition1_file").map_err(fail)?)));
            if compare == "1" || compare == "both" {
                inputs.push(("condition1_future".into(), Value::Map(read("condition1_future_file").map_err(fail)?)));
            }
            if n_cond == 2 {
                inputs.push(("condition2".into(), Value::Map(read("condition2_file").map_err(fail)?)));
                if compare == "2" || compare == "both" {
                    inputs
                        .push(("condition2_future".into(), Value::Map(read("condition2_future_file").map_err(fail)?)));
                }
            } else if n_cond != 1 {
                return Err(fail(Error::msg(format!(
                    "n_conditions must be one of ['1', '2']. Got {}.",
                    get(&cfg, "n_conditions")
                ))));
            }
            params.insert("compare_to_future".into(), serde_json::json!(compare));
            for k in ["comparison1", "comparison2"] {
                params.insert(k.into(), serde_json::json!(get(&cfg, k)));
            }
            for k in ["condition1_lower", "condition1_upper", "condition2_lower", "condition2_upper"] {
                params.insert(k.into(), serde_json::json!(num(k).map_err(fail)?));
            }
        }
        let registry = Registry::standard();
        let op = registry.get("circuit.omniscape").ok_or_else(|| fail(Error::msg("no circuit.omniscape")))?;
        let a = Args { node: args.node, inputs: inputs.into_iter().collect(), params: &params };
        let mut out = op.run(ctx, &a)?;
        if let Some(t) = &reclass {
            // every row in turn, matched against the original values
            let mut classified = resistance.data.to_vec();
            for (old, new) in t {
                for (c, v) in classified.iter_mut().zip(resistance.data.iter()) {
                    if v == old {
                        *c = *new;
                    }
                }
            }
            let classified: Vec<f64> =
                classified.into_iter().map(|v| if single { v as f32 as f64 } else { v }).collect();
            out.insert("classified_resistance".into(), Value::Map(Map::new(resistance.grid.clone(), classified)));
        }
        if args.param("write_outputs", true)? {
            let dir = project_dir(get(&cfg, "project_name")).map_err(fail)?;
            let text: String =
                user.iter().filter(|(k, _)| !k.is_empty()).map(|(k, v)| format!("{k} = {v}\n")).collect();
            std::fs::write(dir.join("config.ini"), text).map_err(|e| fail(Error::msg(e.to_string())))?;
            let tif = flag("write_as_tif");
            let t = resistance.grid.transform;
            let meta = RasterMeta {
                nrows: resistance.grid.height,
                ncols: resistance.grid.width,
                xllcorner: t[0],
                yllcorner: t[3] + resistance.grid.height as f64 * t[5],
                cellsize: t[1],
                transform: t,
                wkt: crate::prepare::crs_of(&resistance.grid).unwrap_or_default(),
            };
            let write = |name: &str, key: &str| -> Result<()> {
                if let Some(Value::Map(m)) = out.get(key) {
                    let v: Vec<f64> = m.data.iter().map(|x| if single { *x as f32 as f64 } else { *x }).collect();
                    write_grid(&dir.join(name).to_string_lossy(), &v, &meta, tif)?;
                }
                Ok(())
            };
            if reclass.is_some() && flag("write_reclassified_resistance") {
                write("classified_resistance", "classified_resistance").map_err(fail)?;
            }
            if flag("write_raw_currmap") {
                write("cum_currmap", "current").map_err(fail)?;
            }
            if flag("calc_flow_potential") {
                write("flow_potential", "flow_potential").map_err(fail)?;
            }
            if flag("calc_normalized_current") {
                write("normalized_cum_currmap", "normalized").map_err(fail)?;
            }
        }
        Ok(out)
    }
}
