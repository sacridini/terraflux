//! From QSWAT+'s tables to a SWAT+ project, as the SWAT+ Editor 4.0.2 writes it (landschaft's
//! `hydro/swat/_editor.py`, byte for byte but for the date of each file's first line): a port of the editor's GIS
//! import (`actions/import_gis.py`), of its observed weather import (`actions/import_weather.py`: stations at the
//! coordinates of the weather files, each object matched to the nearest one) and of its file writers
//! (`fileio/*.py`), for the projects QSWAT+ makes without landscapes, lakes, reservoirs or point sources. The SWAT+
//! Editor (github.com/swat-model/swatplus-editor) and its datasets database are public domain; the files the
//! editor writes the same for every project (parameter databases, decision tables, defaults) come from them
//! (landschaft's `data/swatplus_editor.zip`).

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, OnceLock};

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::swat::{Project, normalize};
use crate::swat_watershed::Watershed;
use crate::swat_weather::{
    Climate, Monthly, Record, WGN_COLUMNS, WgnStation, civil_from_days, climate, parse_date, parse_monthly, time_sim,
};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(FromTables));
}

const EDITOR_VERSION: &str = "4.0.2";

/// The editor's data as landschaft ships it (`tools/swat/make_editor_data.py`): the files it writes the same for
/// every project (`static/`) and its datasets database (`datasets.json`).
const EDITOR_ZIP: &[u8] =
    include_bytes!("../../../extern/landschaft/src/landschaft/hydro/swat/data/swatplus_editor.zip");

struct Editor {
    statics: Vec<(String, String)>,
    data: serde_json::Value,
}

/// The entries of a zip file (stored or deflated).
fn unzip(b: &[u8]) -> std::result::Result<Vec<(String, Vec<u8>)>, String> {
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]) as usize;
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
    let end = (0..b.len().saturating_sub(21)).rev().find(|&i| u32_at(i) == 0x0605_4b50).ok_or("not a zip file")?;
    let (count, mut at) = (u16_at(end + 10), u32_at(end + 16));
    let mut out = Vec::new();
    for _ in 0..count {
        if u32_at(at) != 0x0201_4b50 {
            return Err("bad zip directory".into());
        }
        let (method, csize) = (u16_at(at + 10), u32_at(at + 20));
        let (nlen, xlen, clen, local) = (u16_at(at + 28), u16_at(at + 30), u16_at(at + 32), u32_at(at + 42));
        let name = String::from_utf8_lossy(&b[at + 46..at + 46 + nlen]).into_owned();
        let data = local + 30 + u16_at(local + 26) + u16_at(local + 28);
        let raw = &b[data..data + csize];
        let body = match method {
            0 => raw.to_vec(),
            8 => {
                let mut v = Vec::new();
                flate2::read::DeflateDecoder::new(raw).read_to_end(&mut v).map_err(|e| e.to_string())?;
                v
            }
            m => return Err(format!("zip method {m} is not supported")),
        };
        out.push((name, body));
        at += 46 + nlen + xlen + clen;
    }
    Ok(out)
}

fn editor() -> Result<&'static Editor> {
    static EDITOR: OnceLock<std::result::Result<Editor, String>> = OnceLock::new();
    EDITOR
        .get_or_init(|| {
            let files = unzip(EDITOR_ZIP)?;
            let mut statics = Vec::new();
            let mut data = serde_json::Value::Null;
            for (n, b) in files {
                if let Some(s) = n.strip_prefix("static/") {
                    statics.push((s.to_string(), String::from_utf8(b).map_err(|e| e.to_string())?));
                } else if n == "datasets.json" {
                    data = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
                }
            }
            Ok(Editor { statics, data })
        })
        .as_ref()
        .map_err(|e| Error::msg(format!("the editor's data: {e}")))
}

// ------------------------------------------------------------------------------------- helpers/utils.py

fn rjust(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { s.to_string() } else { " ".repeat(w - n) + s }
}

fn ljust(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { s.to_string() } else { s.to_string() + &" ".repeat(w - n) }
}

/// The editor's `spad`: the text (spaces inside it made `_`) in a field, `null` for none or empty text.
fn spad(val: Option<&str>, pad: usize, right: bool, null: &str) -> String {
    let text = match val {
        None | Some("") => null.to_string(),
        Some(s) => s.trim().replace(' ', "_"),
    };
    (if right { rjust(&text, pad) } else { ljust(&text, pad) }) + "  "
}

fn sp(v: &str) -> String {
    spad(Some(v), 16, true, "null")
}

fn spl(v: &str) -> String {
    spad(Some(v), 16, false, "null")
}

fn spn(v: Option<&str>) -> String {
    spad(v, 16, true, "null")
}

fn spw(v: &str, pad: usize) -> String {
    spad(Some(v), pad, true, "null")
}

fn splw(v: &str, pad: usize) -> String {
    spad(Some(v), pad, false, "null")
}

/// A number as Python formats it with `dec` decimals.
fn fnum(v: f64, dec: usize) -> String {
    if v.is_nan() { "nan".to_string() } else { format!("{v:.dec$}") }
}

/// `npad` of a number: 5 decimals in 12 characters, `null` for none (NaN).
fn npad_full(v: f64, dec: usize, pad: usize, null: &str, nzmin: Option<f64>) -> String {
    if v.is_nan() {
        return spad(None, pad, true, null);
    }
    let v = match nzmin {
        Some(m) if v < m => m,
        _ => v,
    };
    spad(Some(&fnum(v, dec)), pad, true, null)
}

fn npad(v: f64) -> String {
    npad_full(v, 5, 12, "0", None)
}

/// `npad(v, nzmin=True)`: at least 0.00001.
fn npad_nz(v: f64) -> String {
    npad_full(v, 5, 12, "0", Some(0.00001))
}

/// The header of a column of numbers (`npad("slp")`).
fn hd(h: &str) -> String {
    spad(Some(h), 12, true, "0")
}

fn ipad(v: f64) -> String {
    npad_full(v, 0, 8, "0", None)
}

fn ipadw(v: f64, pad: usize) -> String {
    npad_full(v, 0, pad, "0", None)
}

fn ihd(h: &str) -> String {
    spad(Some(h), 8, true, "0")
}

/// `epad`: the number as Python's Decimal formats it with 4 decimals and an exponent.
fn epad(v: f64, nzmin: bool) -> String {
    if v.is_nan() {
        return spad(None, 12, true, "0");
    }
    let v = if nzmin && v == 0.0 { 0.00000001 } else { v };
    let text = if v == 0.0 {
        // (Decimal's zero is shifted by the precision)
        if v.is_sign_negative() { "-0.0000E+4".to_string() } else { "0.0000E+4".to_string() }
    } else {
        let s = format!("{v:.4e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let e: i32 = e.parse().unwrap_or(0);
        format!("{m}E{}{}", if e < 0 { '-' } else { '+' }, e.abs())
    };
    spad(Some(&text), 12, true, "0")
}

fn yn(v: bool) -> String {
    spad(Some(if v { "y" } else { "n" }), 12, true, "null")
}

/// Python's `repr` of a float.
pub fn py_repr(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let s = format!("{:e}", v.abs());
    let (m, e) = s.split_once('e').unwrap();
    let digits: String = m.chars().filter(|c| *c != '.').collect();
    let decpt: i32 = e.parse::<i32>().unwrap() + 1;
    let n = digits.len() as i32;
    let body = if -4 < decpt && decpt <= 16 {
        if decpt <= 0 {
            format!("0.{}{}", "0".repeat((-decpt) as usize), digits)
        } else if decpt >= n {
            format!("{}{}.0", digits, "0".repeat((decpt - n) as usize))
        } else {
            format!("{}.{}", &digits[..decpt as usize], &digits[decpt as usize..])
        }
    } else {
        let exp = decpt - 1;
        let mant = if n > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits.clone() };
        format!("{mant}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
    };
    if v < 0.0 { format!("-{body}") } else { body }
}

/// Python's `str` of a number from a table (a whole number without decimals).
fn py_num(v: f64) -> String {
    if v.is_nan() {
        "None".into()
    } else if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        py_repr(v)
    }
}

fn name_of(prefix: &str, i: i64, maxid: i64) -> String {
    format!("{prefix}{:0>w$}", i, w = maxid.to_string().len())
}

/// import_gis.get_slope_len (slope in percent).
fn slope_len(slope: f64) -> f64 {
    if slope < 1.0 {
        121.0
    } else if slope < 3.0 {
        90.0
    } else if slope < 5.0 {
        60.0
    } else if slope < 8.0 {
        30.0
    } else {
        10.0
    }
}

/// hydrology.Hydrology_hyd.get_perco_cn3_swf_latq_co: (perco, cn3_swf, latq_co).
fn hydrology(grp: &str, slope: f64) -> (f64, f64, f64) {
    let (mut leach, mut runoff) = ("low", "low");
    match grp {
        "A" => {
            leach = "high";
            runoff = if slope < 6.0 {
                "low"
            } else if slope <= 12.0 {
                "mod"
            } else {
                "high"
            };
        }
        "B" => {
            leach = if slope < 6.0 { "high" } else { "mod" };
            runoff = if slope < 4.0 {
                "low"
            } else if slope <= 6.0 {
                "mod"
            } else {
                "high"
            };
        }
        "C" => {
            leach = if slope < 12.0 { "mod" } else { "low" };
            runoff = if slope < 2.0 {
                "low"
            } else if slope <= 6.0 {
                "mod"
            } else {
                "high"
            };
        }
        "D" => {
            runoff = if slope < 2.0 {
                "low"
            } else if slope <= 4.0 {
                "mod"
            } else {
                "high"
            };
        }
        _ => {}
    }
    (
        match leach {
            "high" => 0.9,
            "mod" => 0.5,
            _ => 0.05,
        },
        match runoff {
            "high" => 0.0,
            "mod" => 0.3,
            _ => 0.95,
        },
        match runoff {
            "high" => 0.9,
            "mod" => 0.2,
            _ => 0.01,
        },
    )
}

/// import_weather.weather_sta_name.
fn sta_name(lat: f64, lon: f64) -> String {
    format!(
        "s{}{}{}{}",
        (lat * 1000.0).round_ties_even().abs() as i64,
        if lat >= 0.0 { "n" } else { "s" },
        (lon * 1000.0).round_ties_even().abs() as i64,
        if lon >= 0.0 { "e" } else { "w" }
    )
}

/// import_weather.update_closest_lat_lon: the first of the nearest (squared degrees).
fn nearest(lat: f64, lon: f64, cands: &[(f64, f64)]) -> usize {
    let (mut best, mut bi) = (f64::INFINITY, 0);
    let mut first = true;
    for (i, (la, lo)) in cands.iter().enumerate() {
        let d = (lat - la) * (lat - la) + (lon - lo) * (lon - lo);
        if first || d < best {
            best = d;
            bi = i;
            first = false;
        }
    }
    bi
}

/// BaseFileModel.write_ele_ids2: element numbers with `-` for runs.
fn ele_ids(positions: &[i64]) -> String {
    let (mut out, mut last, mut last_app, mut wrote) = (Vec::new(), 0i64, 0i64, false);
    for &p in positions {
        if last == 0 {
            out.push(ipad(p as f64));
            wrote = true;
            last_app = p;
        } else if p > last + 1 {
            if last_app != last {
                out.push(ipad(-last as f64));
            }
            out.push(ipad(p as f64));
            last_app = p;
            wrote = true;
        } else {
            wrote = false;
        }
        last = p;
    }
    if !wrote && !positions.is_empty() {
        out.push(ipad(-last as f64));
    }
    ipad(out.len() as f64) + &out.concat()
}

// ------------------------------------------------------------------------------------------- the tables

fn table<'a>(w: &'a Watershed, name: &str) -> Result<&'a Table> {
    w.table(name).ok_or_else(|| Error::msg(format!("the watershed has no table {name}")))
}

fn column(t: &Table, table: &str, c: &str) -> Result<usize> {
    t.columns.iter().position(|x| x == c).ok_or_else(|| Error::msg(format!("{table} has no column {c:?}")))
}

fn nums(t: &Table, table: &str, c: &str) -> Result<Vec<f64>> {
    let j = column(t, table, c)?;
    Ok(t.rows.iter().map(|r| r[j]).collect())
}

fn texts(t: &Table, table: &str, c: &str) -> Result<Vec<Option<String>>> {
    let j = column(t, table, c)?;
    Ok(t.rows.iter().map(|r| (!r[j].is_nan()).then(|| t.cell(j, r[j]))).collect())
}

/// The rows' order by a column of whole numbers (pandas' sort_values).
fn order_by(ids: &[f64]) -> Vec<usize> {
    let mut o: Vec<usize> = (0..ids.len()).collect();
    o.sort_by(|a, b| ids[*a].partial_cmp(&ids[*b]).unwrap_or(std::cmp::Ordering::Equal));
    o
}

struct Lsu {
    id: i64,
    slope: f64,
    elev: f64,
    lat: f64,
    lon: f64,
    area: f64,
}

struct Chan {
    id: i64,
    strahler: String,
    wid2: f64,
    dep2: f64,
    slo2: f64,
    len2: f64,
    midlat: f64,
    midlon: f64,
    areac: f64,
}

struct Hru {
    id: i64,
    lsu: i64,
    slope: f64,
    landuse: Option<String>,
    soil: Option<String>,
    elev: f64,
    lat: f64,
    lon: f64,
    arslp: f64,
    arlsu: f64,
}

struct Aqu {
    id: i64,
    lat: f64,
    lon: f64,
    area: f64,
    elev: f64,
}

struct Route {
    sourcecat: String,
    sourceid: i64,
    sinkcat: Option<String>,
    sinkid: i64,
    percent: f64,
    hyd_typ: Option<String>,
}

struct Soil {
    id: f64,
    name: String,
    hyd_grp: String,
    dp_tot: f64,
    anion_excl: f64,
    perc_crk: f64,
    texture: Option<String>,
}

struct Layer {
    soil_id: f64,
    layer_num: f64,
    v: [f64; 14],
}

fn lsus_of(t: &Table) -> Result<Vec<Lsu>> {
    let n = "gis_lsus";
    let (id, slope, elev, lat, lon, area) = (
        nums(t, n, "id")?,
        nums(t, n, "slope")?,
        nums(t, n, "elev")?,
        nums(t, n, "lat")?,
        nums(t, n, "lon")?,
        nums(t, n, "area")?,
    );
    Ok(order_by(&id)
        .into_iter()
        .map(|i| Lsu { id: id[i] as i64, slope: slope[i], elev: elev[i], lat: lat[i], lon: lon[i], area: area[i] })
        .collect())
}

fn chans_of(t: &Table) -> Result<Vec<Chan>> {
    let n = "gis_channels";
    let id = nums(t, n, "id")?;
    let (st, wid2, dep2, slo2, len2) =
        (nums(t, n, "strahler")?, nums(t, n, "wid2")?, nums(t, n, "dep2")?, nums(t, n, "slo2")?, nums(t, n, "len2")?);
    let (midlat, midlon, areac) = (nums(t, n, "midlat")?, nums(t, n, "midlon")?, nums(t, n, "areac")?);
    Ok(order_by(&id)
        .into_iter()
        .map(|i| Chan {
            id: id[i] as i64,
            strahler: py_num(st[i]),
            wid2: wid2[i],
            dep2: dep2[i],
            slo2: slo2[i],
            len2: len2[i],
            midlat: midlat[i],
            midlon: midlon[i],
            areac: areac[i],
        })
        .collect())
}

fn hrus_of(t: &Table) -> Result<Vec<Hru>> {
    let n = "gis_hrus";
    let (id, lsu, slope) = (nums(t, n, "id")?, nums(t, n, "lsu")?, nums(t, n, "slope")?);
    let (landuse, soil) = (texts(t, n, "landuse")?, texts(t, n, "soil")?);
    let (elev, lat, lon) = (nums(t, n, "elev")?, nums(t, n, "lat")?, nums(t, n, "lon")?);
    let (arslp, arlsu) = (nums(t, n, "arslp")?, nums(t, n, "arlsu")?);
    Ok((0..id.len())
        .map(|i| Hru {
            id: id[i] as i64,
            lsu: lsu[i] as i64,
            slope: slope[i],
            landuse: landuse[i].clone(),
            soil: soil[i].clone(),
            elev: elev[i],
            lat: lat[i],
            lon: lon[i],
            arslp: arslp[i],
            arlsu: arlsu[i],
        })
        .collect())
}

fn aqus_of(t: &Table, n: &str) -> Result<Vec<Aqu>> {
    let id = nums(t, n, "id")?;
    let (lat, lon, area, elev) = (nums(t, n, "lat")?, nums(t, n, "lon")?, nums(t, n, "area")?, nums(t, n, "elev")?);
    Ok(order_by(&id)
        .into_iter()
        .map(|i| Aqu { id: id[i] as i64, lat: lat[i], lon: lon[i], area: area[i], elev: elev[i] })
        .collect())
}

fn routing_of(t: &Table) -> Result<Vec<Route>> {
    let n = "gis_routing";
    let (src, sink) = (texts(t, n, "sourcecat")?, texts(t, n, "sinkcat")?);
    let (sid, kid, pc) = (nums(t, n, "sourceid")?, nums(t, n, "sinkid")?, nums(t, n, "percent")?);
    let hyd = texts(t, n, "hyd_typ")?;
    Ok((0..src.len())
        .map(|i| Route {
            sourcecat: src[i].clone().unwrap_or_default(),
            sourceid: sid[i] as i64,
            sinkcat: sink[i].clone(),
            sinkid: kid[i] as i64,
            percent: pc[i],
            hyd_typ: hyd[i].clone(),
        })
        .collect())
}

const LAYER_COLUMNS: [&str; 14] =
    ["dp", "bd", "awc", "soil_k", "carbon", "clay", "silt", "sand", "rock", "alb", "usle_k", "ec", "caco3", "ph"];

fn soils_of(t: &Table) -> Result<Vec<Soil>> {
    let n = "soils_sol";
    let (id, name, grp) = (nums(t, n, "id")?, texts(t, n, "name")?, texts(t, n, "hyd_grp")?);
    let (dp_tot, anion, crk) = (nums(t, n, "dp_tot")?, nums(t, n, "anion_excl")?, nums(t, n, "perc_crk")?);
    let texture = texts(t, n, "texture")?;
    Ok(order_by(&id)
        .into_iter()
        .map(|i| Soil {
            id: id[i],
            name: name[i].clone().unwrap_or_default(),
            hyd_grp: grp[i].clone().unwrap_or_default(),
            dp_tot: dp_tot[i],
            anion_excl: anion[i],
            perc_crk: crk[i],
            texture: texture[i].clone(),
        })
        .collect())
}

fn layers_of(t: &Table) -> Result<Vec<Layer>> {
    let n = "soils_sol_layer";
    let (sid, num) = (nums(t, n, "soil_id")?, nums(t, n, "layer_num")?);
    let cols: Vec<Vec<f64>> = LAYER_COLUMNS.iter().map(|c| nums(t, n, c)).collect::<Result<_>>()?;
    Ok((0..sid.len())
        .map(|i| {
            let mut v = [0.0; 14];
            for (k, c) in cols.iter().enumerate() {
                v[k] = c[i];
            }
            Layer { soil_id: sid[i], layer_num: num[i], v }
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------- the project

struct Topo {
    name: String,
    slp: f64,
    slp_len: f64,
    lat_len: f64,
}

struct Rtu {
    name: String,
    topo: String,
    field: String,
    gis_id: f64,
    elev: f64,
    lat: f64,
    lon: f64,
    area: f64,
}

struct Cha {
    name: String,
    hyd: String,
    order: String,
    wd: f64,
    dp: f64,
    slp: f64,
    len: f64,
    gis_id: f64,
    lat: f64,
    lon: f64,
    area: f64,
}

struct HruData {
    name: String,
    topo: String,
    hydro: String,
    soil: String,
    lu_mgt: Option<String>,
}

struct EleRow {
    name: String,
    rtu: i64,
    obj_id: i64,
    frac: f64,
}

struct LsEle {
    name: String,
    obj_typ_no: i64,
    bsn_frac: f64,
    sub_frac: f64,
    rtu: i64,
}

struct Aquifer {
    name: String,
    deep: bool,
    gis_id: f64,
    lat: f64,
    lon: f64,
    area: f64,
    elev: f64,
}

/// A land use of landuse.lum (None: `null`).
#[derive(Clone, Default)]
struct Lum {
    name: String,
    plnt_com: Option<String>,
    mgt: Option<String>,
    cn2: Option<String>,
    cons_prac: Option<String>,
    urban: Option<String>,
    urb_ro: Option<String>,
    ov_mann: Option<String>,
}

struct PlantIni {
    plnt_name: String,
    lc_status: bool,
    v: [f64; 6],
}

struct PCom {
    name: String,
    rot_yr_ini: f64,
    plants: Vec<PlantIni>,
}

struct Mgt {
    name: String,
    d_table: String,
    plant1: String,
}

fn jstr(v: &serde_json::Value) -> Option<String> {
    v.as_str().map(String::from)
}

/// The land uses (by the land use of the HRUs, lower case), their plant communities and their rotations.
type Landuses = (Vec<(String, Lum)>, Vec<PCom>, Vec<Mgt>);

/// import_gis.insert_landuse: a land use and plant community per land use of the HRUs.
fn landuse(hrus: &[Hru], data: &serde_json::Value) -> Result<Landuses> {
    let mut lus: Vec<String> = Vec::new();
    for h in hrus {
        if let Some(l) = &h.landuse
            && l != "NULL"
            && !lus.contains(&l.to_lowercase())
        {
            lus.push(l.to_lowercase());
        }
    }
    let (mut lums, mut pcoms, mut mgts): Landuses = (Vec::new(), Vec::new(), Vec::new());
    let defaults = Lum {
        cn2: jstr(&data["cntable"]["5"]),
        cons_prac: jstr(&data["cons_prac"]["1"]),
        ov_mann: jstr(&data["ovn_table"]["2"]),
        ..Default::default()
    };
    for lu in lus {
        let plant = &data["plants"][&lu];
        let (lum_name, comm_name) = (format!("{lu}_lum"), format!("{lu}_comm"));
        if plant.is_null() {
            let u = &data["urban"][&lu];
            if u.is_null() {
                return Err(Error::msg(format!(
                    "{lu} does not exist in plants_plt or urban_urb, but is used as land use in your GIS HRUs."
                )));
            }
            lums.push((
                lu.clone(),
                Lum {
                    name: lum_name,
                    cn2: jstr(&data["cntable"]["49"]),
                    cons_prac: defaults.cons_prac.clone(),
                    urban: jstr(u),
                    urb_ro: Some("buildup_washoff".into()),
                    ov_mann: jstr(&data["ovn_table"]["18"]),
                    ..Default::default()
                },
            ));
            continue;
        }
        let pname = plant["name"].as_str().unwrap_or_default().to_string();
        let ptyp = plant["typ"].as_str().unwrap_or_default();
        let ds_pi = &data["plant_ini"][&comm_name];
        if !ds_pi.is_null() {
            let plants = ds_pi["plants"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|p| PlantIni {
                            plnt_name: pname.clone(),
                            lc_status: p["lc_status"].as_f64().unwrap_or(0.0) != 0.0,
                            v: ["lai_init", "bm_init", "phu_init", "plnt_pop", "yrs_init", "rsd_init"]
                                .map(|k| p[k].as_f64().unwrap_or(0.0)),
                        })
                        .collect()
                })
                .unwrap_or_default();
            pcoms.push(PCom {
                name: jstr(&ds_pi["name"]).unwrap_or_default(),
                rot_yr_ini: ds_pi["rot_yr_ini"].as_f64().unwrap_or(1.0),
                plants,
            });
            let mut mgt = None;
            if ptyp.starts_with("warm_annual") || ptyp.starts_with("cold_annual") {
                let table = if ptyp.starts_with("warm_annual") { "pl_hv_summer1" } else { "pl_hv_winter1" };
                let m = format!("{pname}_rot");
                if !mgts.iter().any(|x: &Mgt| x.name == m) {
                    mgts.push(Mgt { name: m.clone(), d_table: table.into(), plant1: pname.clone() });
                }
                mgt = Some(m);
            }
            let ds_m = &data["landuse_lum"][&lum_name];
            let lum = if !ds_m.is_null() {
                Lum {
                    name: jstr(&ds_m["name"]).unwrap_or_default(),
                    plnt_com: jstr(&ds_pi["name"]),
                    mgt,
                    cn2: jstr(&ds_m["cn2"]),
                    cons_prac: jstr(&ds_m["cons_prac"]),
                    ov_mann: jstr(&ds_m["ov_mann"]),
                    ..Default::default()
                }
            } else {
                Lum { name: lum_name, plnt_com: jstr(&ds_pi["name"]), mgt, ..defaults.clone() }
            };
            lums.push((lu, lum));
        } else {
            pcoms.push(PCom {
                name: comm_name.clone(),
                rot_yr_ini: 1.0,
                plants: vec![PlantIni { plnt_name: pname, lc_status: false, v: [0.0, 0.0, 0.0, 0.0, 0.0, 10000.0] }],
            });
            lums.push((lu, Lum { name: lum_name, plnt_com: Some(comm_name), ..defaults.clone() }));
        }
    }
    Ok((lums, pcoms, mgts))
}

type Out = (&'static str, i64, Option<String>, f64);

fn con_type(cat: &str) -> Option<&'static str> {
    match cat {
        "CH" => Some("sdc"),
        "AQU" | "DAQ" => Some("aqu"),
        "LSU" => Some("ru"),
        _ => None,
    }
}

/// import_gis.get_connections (no point sources, reservoirs or lte): the outputs of each object.
fn connections(
    routing: &[Route],
    cats: &[&str],
    src_ids: &HashMap<i64, i64>,
    ids: &[(&str, &HashMap<i64, i64>)],
) -> Result<HashMap<i64, Vec<Out>>> {
    let id_of = |cat: &str, k: i64| -> Result<i64> {
        ids.iter()
            .find(|(c, _)| *c == cat)
            .and_then(|(_, m)| m.get(&k).copied())
            .ok_or_else(|| Error::msg(format!("the routing names the {cat} {k}, which the tables do not have")))
    };
    let mut rows: Vec<&Route> = routing
        .iter()
        .filter(|r| cats.contains(&r.sourcecat.as_str()) && r.percent > 0.0 && r.sinkcat.as_deref() != Some("X"))
        .collect();
    rows.sort_by_key(|r| r.sourceid);
    let mut pt_src: HashMap<i64, &Route> = HashMap::new();
    for r in routing.iter().filter(|r| r.sourcecat == "PT") {
        pt_src.insert(r.sourceid, r);
    }
    let mut ch_to_pt: HashMap<i64, &Route> = HashMap::new();
    for r in routing.iter().filter(|r| r.sourcecat == "CH" && r.sinkcat.as_deref() == Some("PT")) {
        ch_to_pt.insert(r.sinkid, r);
    }
    let mut out: HashMap<i64, Vec<Out>> = HashMap::new();
    for r in rows {
        let from = *src_ids.get(&r.sourceid).ok_or_else(|| {
            Error::msg(format!("the routing names the {} {}, which the tables do not have", r.sourcecat, r.sourceid))
        })?;
        if r.sinkcat.as_deref() == Some("PT") && cats.contains(&"AQU") {
            let Some(src) = ch_to_pt.get(&r.sinkid) else { continue };
            let typ = con_type(&src.sourcecat)
                .ok_or_else(|| Error::msg(format!("routing: {} cannot be a source here", src.sourcecat)))?;
            out.entry(from).or_default().push((
                typ,
                id_of(&src.sourcecat, src.sourceid)?,
                src.hyd_typ.clone(),
                src.percent / 100.0,
            ));
            continue;
        }
        let mut con = Some(r);
        let mut hops = 0;
        while let Some(c) = con {
            if c.sinkcat.as_deref() != Some("PT") {
                break;
            }
            hops += 1;
            if hops > routing.len() {
                return Err(Error::msg("the routing's point sources form a loop"));
            }
            con = pt_src.get(&c.sinkid).copied();
        }
        if let Some(c) = con
            && let Some(cat) = c.sinkcat.as_deref().filter(|s| con_type(s).is_some())
        {
            out.entry(from).or_default().push((
                con_type(cat).unwrap(),
                id_of(cat, c.sinkid)?,
                c.hyd_typ.clone(),
                c.percent / 100.0,
            ));
        }
    }
    Ok(out)
}

struct Con {
    name: String,
    gis_id: f64,
    area: f64,
    lat: f64,
    lon: f64,
    elev: f64,
}

/// fileio/connect.write_con_table.
fn con_table(meta: &str, elem: &str, cons: &[Con], outs: &[Vec<Out>], wst: &dyn Fn(f64, f64) -> String) -> String {
    let has_out = outs.iter().any(|o| !o.is_empty());
    let mut head = ihd("id")
        + &spl("name")
        + &ihd("gis_id")
        + &hd("area")
        + &hd("lat")
        + &hd("lon")
        + &hd("elev")
        + &ihd(elem)
        + &sp("wst")
        + &ihd("cst")
        + &ihd("ovfl")
        + &ihd("rule")
        + &ihd("out_tot");
    if has_out {
        head += &(spw("obj_typ", 12) + &ihd("obj_id") + &spw("hyd_typ", 12) + &hd("frac"));
    }
    let mut text = format!("{meta}{head}\n");
    for (i, (c, o)) in cons.iter().zip(outs).enumerate() {
        let i = (i + 1) as f64;
        let mut s = ipad(i)
            + &spl(&c.name)
            + &ipad(c.gis_id)
            + &npad_nz(c.area)
            + &npad(c.lat)
            + &npad(c.lon)
            + &npad(c.elev)
            + &ipad(i)
            + &sp(&wst(c.lat, c.lon))
            + &ipad(0.0)
            + &ipad(0.0)
            + &ipad(0.0)
            + &ipad(o.len() as f64);
        for (t, oid, hyd, frac) in o {
            s += &(spw(t, 12) + &ipad(*oid as f64) + &spad(hyd.as_deref(), 12, true, "null") + &npad(*frac));
        }
        text += &(s + "\n");
    }
    text
}

/// fileio/channel.Hyd_sed_lte_cha (import_gis.insert_channels_lte values).
fn hyd_sed(meta: &str, chas: &[Cha]) -> String {
    let cols = [
        "order",
        "wd",
        "dp",
        "slp",
        "len",
        "mann",
        "k",
        "erod_fact",
        "cov_fact",
        "sinu",
        "eq_slp",
        "d50",
        "clay",
        "carbon",
        "dry_bd",
        "side_slp",
        "bankfull_flo",
        "fps",
        "fpn",
        "n_conc",
        "p_conc",
        "p_bio",
    ];
    let fixed = |k: &str| match k {
        "mann" => 0.05,
        "k" => 1.0,
        "erod_fact" => 0.01,
        "cov_fact" => 0.005,
        "sinu" => 1.05,
        "eq_slp" => 0.001,
        "d50" => 12.0,
        "clay" => 50.0,
        "carbon" => 0.04,
        "dry_bd" => 1.0,
        "side_slp" => 0.5,
        "bankfull_flo" => 0.5,
        "fps" => 0.00001,
        "fpn" => 0.1,
        _ => 0.0,
    };
    let head = spl("name") + &sp("order") + &cols[1..].iter().map(|c| hd(c)).collect::<String>() + "description";
    let mut text = format!("{meta}{head}\n");
    for c in chas {
        let mut s = spl(&c.hyd) + &sp(&c.order);
        for k in &cols[1..] {
            s += &match *k {
                "wd" => npad_full(c.wd, 5, 12, "0", Some(0.001)),
                "dp" => npad_full(c.dp, 5, 12, "0", Some(0.001)),
                "slp" => npad_full(c.slp, 5, 12, "0", Some(0.00001)),
                "len" => npad_full(c.len, 5, 12, "0", Some(0.00001)),
                k => npad(fixed(k)),
            };
        }
        text += &(s + "\n");
    }
    text
}

fn landuse_text(meta: &str, rows: &[&Lum]) -> String {
    let cols = [
        "cal_group",
        "plnt_com",
        "mgt",
        "cn2",
        "cons_prac",
        "urban",
        "urb_ro",
        "ov_mann",
        "tile",
        "sep",
        "vfs",
        "grww",
        "bmp",
    ];
    let w = |c: &str| if c == "mgt" { 41 } else { 16 };
    let head = splw("name", 20) + &cols.iter().map(|c| spw(c, w(c))).collect::<String>();
    let mut text = format!("{meta}{head}\n");
    for r in rows {
        let get = |c: &str| match c {
            "plnt_com" => r.plnt_com.as_deref(),
            "mgt" => r.mgt.as_deref(),
            "cn2" => r.cn2.as_deref(),
            "cons_prac" => r.cons_prac.as_deref(),
            "urban" => r.urban.as_deref(),
            "urb_ro" => r.urb_ro.as_deref(),
            "ov_mann" => r.ov_mann.as_deref(),
            _ => None,
        };
        text +=
            &(splw(&r.name, 20) + &cols.iter().map(|c| spad(get(c), w(c), true, "null")).collect::<String>() + "\n");
    }
    text
}

fn plant_ini(meta: &str, pcoms: &[PCom]) -> String {
    let nums = ["lai_init", "bm_init", "phu_init", "plnt_pop", "yrs_init", "rsd_init"];
    let head = spl("pcom_name")
        + &ihd("plt_cnt")
        + &ihd("rot_yr_ini")
        + &sp("plt_name")
        + &spw("lc_status", 12)
        + &nums.iter().map(|c| hd(c)).collect::<String>();
    let mut text = format!("{meta}{head}\n");
    for p in pcoms {
        text += &(spl(&p.name) + &ipad(p.plants.len() as f64) + &ipad(p.rot_yr_ini) + "\n");
        for it in &p.plants {
            text += &(spad(Some(""), 16, true, "")
                + &spad(Some(""), 8, true, "")
                + &sp(&it.plnt_name)
                + &yn(it.lc_status)
                + &it.v.iter().map(|x| npad(*x)).collect::<String>()
                + "\n");
        }
    }
    text
}

fn management(meta: &str, mgts: &[Mgt]) -> String {
    let head = splw("name", 41)
        + &spw("numb_ops", 8)
        + &spw("numb_auto", 9)
        + &sp("op_typ")
        + &ihd("mon")
        + &ihd("day")
        + &hd("hu_sch")
        + &sp("op_data1")
        + &sp("op_data2")
        + &hd("op_data3");
    let mut text = format!("{meta}{head}\n");
    for m in mgts {
        text += &(splw(&m.name, 25) + &ipad(0.0) + &ipadw(1.0, 9) + "\n");
        text += &(spw(" ", 46) + &sp(&m.d_table) + &spw(&m.plant1, 5) + "\n");
    }
    text
}

fn soils_text(meta: &str, soils: &[Soil], layers: &[Layer]) -> String {
    let head = splw("name", 25)
        + &spw("nly", 8)
        + &sp("hyd_grp")
        + &hd("dp_tot")
        + &hd("anion_excl")
        + &hd("perc_crk")
        + &splw("texture", 25)
        + &LAYER_COLUMNS.iter().map(|c| hd(c)).collect::<String>();
    let mut text = format!("{meta}{head}\n");
    for s in soils {
        let mut ls: Vec<&Layer> = layers.iter().filter(|x| x.soil_id == s.id).collect();
        ls.sort_by(|a, b| a.layer_num.partial_cmp(&b.layer_num).unwrap_or(std::cmp::Ordering::Equal));
        text += &(splw(&s.name, 25)
            + &ipad(ls.len() as f64)
            + &sp(&s.hyd_grp)
            + &npad(s.dp_tot)
            + &npad(s.anion_excl)
            + &npad(s.perc_crk)
            + &spad(s.texture.as_deref(), 25, false, "null")
            + "\n");
        for x in ls {
            text +=
                &(spw(" ", 122) + &x.v.iter().map(|v| npad_full(*v, 5, 12, "0.0", None)).collect::<String>() + "\n");
        }
    }
    text
}

fn wgn_text(meta: &str, wgns: &[WgnStation]) -> String {
    let mut text = meta.to_string();
    let mut sorted: Vec<&WgnStation> = wgns.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for w in sorted {
        text += &(splw(&w.name, 25) + &npad(w.lat) + &npad(w.lon) + &npad(w.elev) + &ipad(w.rain_yrs as f64) + "\n");
        text += &(WGN_COLUMNS.iter().map(|c| hd(c)).collect::<String>() + "\n");
        for (_, m) in &w.months {
            text += &(m.iter().map(|x| npad(*x)).collect::<String>() + "\n");
        }
    }
    text
}

const CIO: &[(&str, &[Option<&str>])] = &[
    ("simulation", &[Some("time.sim"), Some("print.prt"), None, Some("object.cnt"), None]),
    ("basin", &[Some("codes.bsn"), Some("parameters.bsn"), None]),
    (
        "climate",
        &[
            Some("weather-sta.cli"),
            Some("weather-wgn.cli"),
            Some("pet.cli"),
            Some("pcp.cli"),
            Some("tmp.cli"),
            Some("slr.cli"),
            Some("hmd.cli"),
            Some("wnd.cli"),
            None,
        ],
    ),
    (
        "connect",
        &[
            Some("hru.con"),
            None,
            Some("rout_unit.con"),
            None,
            Some("aquifer.con"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("chandeg.con"),
        ],
    ),
    (
        "channel",
        &[
            Some("initial.cha"),
            None,
            None,
            None,
            Some("nutrients.cha"),
            Some("channel-lte.cha"),
            Some("hyd-sed-lte.cha"),
            None,
        ],
    ),
    ("reservoir", &[None; 8]),
    ("routing_unit", &[Some("rout_unit.def"), Some("rout_unit.ele"), Some("rout_unit.rtu"), None]),
    ("hru", &[Some("hru-data.hru"), None]),
    ("exco", &[None; 6]),
    ("recall", &[None]),
    ("dr", &[None; 6]),
    ("aquifer", &[Some("initial.aqu"), Some("aquifer.aqu")]),
    ("herd", &[None; 3]),
    ("water_rights", &[None; 3]),
    ("link", &[None; 2]),
    ("hydrology", &[Some("hydrology.hyd"), Some("topography.hyd"), Some("field.fld")]),
    (
        "structural",
        &[
            Some("tiledrain.str"),
            Some("septic.str"),
            Some("filterstrip.str"),
            Some("grassedww.str"),
            Some("bmpuser.str"),
        ],
    ),
    (
        "hru_parm_db",
        &[
            Some("plants.plt"),
            Some("fertilizer.frt"),
            Some("tillage.til"),
            Some("pesticide.pes"),
            None,
            None,
            None,
            Some("urban.urb"),
            Some("septic.sep"),
            Some("snow.sno"),
        ],
    ),
    (
        "ops",
        &[
            Some("harv.ops"),
            Some("graze.ops"),
            Some("irr.ops"),
            Some("chem_app.ops"),
            Some("fire.ops"),
            Some("sweep.ops"),
        ],
    ),
    (
        "lum",
        &[
            Some("landuse.lum"),
            Some("management.sch"),
            Some("cntable.lum"),
            Some("cons_practice.lum"),
            Some("ovn_table.lum"),
        ],
    ),
    ("chg", &[Some("cal_parms.cal"), None, None, None, None, None, None, None, None]),
    (
        "init",
        &[
            Some("plant.ini"),
            Some("soil_plant.ini"),
            Some("om_water.ini"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
    ),
    ("soils", &[Some("soils.sol"), Some("nutrients.sol"), None]),
    ("decision_table", &[Some("lum.dtl"), Some("res_rel.dtl"), None, None]),
    (
        "regions",
        &[
            Some("ls_unit.ele"),
            Some("ls_unit.def"),
            None,
            None,
            None,
            None,
            None,
            None,
            Some("aqu_catunit.ele"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
    ),
    ("pcp_path", &[None]),
    ("tmp_path", &[None]),
    ("slr_path", &[None]),
    ("hmd_path", &[None]),
    ("wnd_path", &[None]),
    ("out_path", &[None]),
];

/// fileio/config.File_cio: the files of each class, null where the project has none.
fn file_cio(meta: &str, present: &dyn Fn(&str) -> bool) -> String {
    let mut text = meta.to_string();
    for (cls, names) in CIO {
        text += &spl(cls);
        for n in *names {
            text += &spad(n.filter(|n| present(n)), 16, false, "null");
        }
        text += "\n";
    }
    text
}

/// aquifer.Aquifer_aqu.get_default_shallow / get_default_deep.
fn aquifer_defaults(deep: bool) -> [f64; 15] {
    if deep {
        [0.0, 100.0, 20.0, 0.0, 0.0, 0.5, 50.0, 1.0, 0.01, 0.0, 0.0, 0.03, 30.0, 0.0, 0.0]
    } else {
        [0.05, 10.0, 3.0, 0.0, 0.0, 0.5, 50.0, 1.0, 0.05, 0.02, 0.05, 0.05, 30.0, 3.0, 5.0]
    }
}

/// A value of `codes` as Python's `str` gives it.
fn code_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => i.to_string(),
            None => py_repr(n.as_f64().unwrap_or(f64::NAN)),
        },
        serde_json::Value::Null => "None".into(),
        other => other.to_string(),
    }
}

fn set_codes(text: &str, codes: &[(String, serde_json::Value)]) -> Result<String> {
    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    if lines.len() < 3 {
        return Err(Error::msg("codes.bsn is too short"));
    }
    let head: Vec<String> = lines[1].split_whitespace().map(String::from).collect();
    let mut vals: Vec<String> = lines[2].split_whitespace().map(String::from).collect();
    for (k, v) in codes {
        let i = head.iter().position(|h| h == k).ok_or_else(|| Error::msg(format!("codes.bsn has no column '{k}'")))?;
        vals[i] = code_text(v);
    }
    lines[2] =
        vals.iter().enumerate().map(|(i, v)| spw(v, if i < 2 || head[i] == "atmo_dep" { 16 } else { 8 })).collect();
    Ok(lines.join("\n"))
}

/// The date and time (UTC) as the files' first lines have them.
fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let (y, m, d) = civil_from_days(secs.div_euclid(86400));
    let t = secs.rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", t / 3600, t % 3600 / 60)
}

struct Sta {
    name: String,
    lat: f64,
    lon: f64,
    /// hmd, pcp, slr, tmp, wnd, pet
    files: [Option<String>; 6],
    wgn: String,
}

/// A SWAT+ project from the watershed's tables and the weather, as the SWAT+ Editor makes it (landschaft's
/// `from_tables`): `start` and `end` (days) default to the days all the weather files cover, `codes` change columns
/// of codes.bsn.
pub fn from_tables(
    w: &Arc<Watershed>,
    climate: &Climate,
    start: Option<i64>,
    end: Option<i64>,
    codes: &[(String, serde_json::Value)],
    name: &str,
) -> Result<Project> {
    let ed = editor()?;
    let stamp = stamp();
    let meta = |f: &str| {
        format!("{f}: written by Terraflux for SWAT+ 62 (SWAT+ Editor {EDITOR_VERSION} formats) on {stamp}\n")
    };
    let mut files: Vec<(String, String)> = Vec::new();
    let put =
        |files: &mut Vec<(String, String)>, name: &str, text: String| match files.iter_mut().find(|(n, _)| n == name) {
            Some(e) => e.1 = text,
            None => files.push((name.to_string(), text)),
        };
    for (f, text) in &ed.statics {
        put(&mut files, f, meta(f) + text);
    }
    if !codes.is_empty() {
        let at = files
            .iter()
            .position(|(n, _)| n == "codes.bsn")
            .ok_or_else(|| Error::msg("the editor's data has no codes.bsn"))?;
        files[at].1 = set_codes(&files[at].1, codes)?;
    }

    let lsus = lsus_of(table(w, "gis_lsus")?)?;
    let chans = chans_of(table(w, "gis_channels")?)?;
    let hrus = hrus_of(table(w, "gis_hrus")?)?;
    let aqus = aqus_of(table(w, "gis_aquifers")?, "gis_aquifers")?;
    let deeps = aqus_of(table(w, "gis_deep_aquifers")?, "gis_deep_aquifers")?;
    let subs_area = nums(table(w, "gis_subbasins")?, "gis_subbasins", "area")?;
    let routing = routing_of(table(w, "gis_routing")?)?;
    let soils = soils_of(table(w, "soils_sol")?)?;
    let layers = layers_of(table(w, "soils_sol_layer")?)?;

    // insert_routing_units
    let empty = |what: &str| Error::msg(format!("{what} is empty"));
    let cnt = lsus.iter().map(|r| r.id).max().ok_or_else(|| empty("gis_lsus"))?;
    let mut gis_to_rtu: HashMap<i64, i64> = HashMap::new();
    let (mut topo_sub, mut fields, mut rtus): (Vec<Topo>, Vec<String>, Vec<Rtu>) = (Vec::new(), Vec::new(), Vec::new());
    for (i, r) in lsus.iter().enumerate() {
        gis_to_rtu.insert(r.id, i as i64 + 1);
        let sl = slope_len(r.slope / 100.0);
        topo_sub.push(Topo { name: name_of("toportu", r.id, cnt), slp: r.slope / 100.0, slp_len: sl, lat_len: sl });
        fields.push(name_of("fld", r.id, cnt));
        rtus.push(Rtu {
            name: name_of("rtu", r.id, cnt),
            topo: topo_sub[i].name.clone(),
            field: fields[i].clone(),
            gis_id: r.id as f64,
            elev: r.elev,
            lat: r.lat,
            lon: r.lon,
            area: r.area,
        });
    }
    // insert_channels_lte
    let cnt = chans.iter().map(|r| r.id).max().ok_or_else(|| empty("gis_channels"))?;
    let mut gis_to_cha: HashMap<i64, i64> = HashMap::new();
    let mut chas: Vec<Cha> = Vec::new();
    for (i, r) in chans.iter().enumerate() {
        gis_to_cha.insert(r.id, i as i64 + 1);
        let cname = name_of("cha", r.id, cnt);
        chas.push(Cha {
            hyd: format!("hyd{cname}"),
            name: cname,
            order: r.strahler.clone(),
            wd: r.wid2,
            dp: r.dep2,
            slp: r.slo2 / 100.0,
            len: r.len2 / 1000.0,
            gis_id: r.id as f64,
            lat: r.midlat,
            lon: r.midlon,
            area: r.areac,
        });
    }
    // insert_hrus (and insert_landuse)
    let (lum_rows, pcoms, mgts) = landuse(&hrus, &ed.data)?;
    let bsn_area = crate::swat_weather::np_sum(&subs_area);
    let cnt = hrus.iter().map(|r| r.id).max().ok_or_else(|| empty("gis_hrus"))?;
    let mut hyds: Vec<(String, [f64; 14])> = Vec::new();
    let (mut topo_hru, mut hru_data, mut hru_cons): (Vec<Topo>, Vec<HruData>, Vec<Con>) =
        (Vec::new(), Vec::new(), Vec::new());
    let (mut ru_ele, mut ls_ele): (Vec<EleRow>, Vec<LsEle>) = (Vec::new(), Vec::new());
    for (i, r) in hrus.iter().enumerate() {
        let hname = name_of("hru", r.id, cnt);
        let sname = r.soil.as_deref().unwrap_or("");
        // (a later soil of the same name wins, as in a dict)
        let soil = soils
            .iter()
            .rev()
            .find(|s| s.name == sname)
            .ok_or_else(|| Error::msg(format!("soil \"{sname}\" of the HRUs is not in soils_sol")))?;
        let (perco, cn3_swf, latq_co) = hydrology(&soil.hyd_grp, r.slope / 100.0);
        hyds.push((
            name_of("hyd", r.id, cnt),
            [0.0, 0.0, 1.0, 0.95, 0.5, 0.0, 0.0, cn3_swf, 0.2, perco, 0.0, 0.0, 1.0, latq_co],
        ));
        let sl = slope_len(r.slope);
        topo_hru.push(Topo { name: name_of("topohru", r.id, cnt), slp: r.slope / 100.0, slp_len: sl, lat_len: sl });
        let lu = r.landuse.as_ref().map(|l| l.to_lowercase());
        if let Some(l) = &lu
            && ["wehb", "wetf", "wetl", "wetn", "wewo", "watr", "playa", "wetw", "wetm"].contains(&l.as_str())
        {
            return Err(Error::msg(format!(
                "land use '{}' is a wetland in SWAT+, which the port does not simulate; give the water cells \
                 another land use",
                r.landuse.as_deref().unwrap_or("")
            )));
        }
        let lu_mgt = match &lu {
            None => None,
            Some(l) => Some(
                lum_rows
                    .iter()
                    .find(|(k, _)| k == l)
                    .map(|(_, m)| m.name.clone())
                    .ok_or_else(|| Error::msg(format!("land use {l:?} has no land use in the project")))?,
            ),
        };
        hru_data.push(HruData {
            name: hname.clone(),
            topo: topo_hru[i].name.clone(),
            hydro: hyds[i].0.clone(),
            soil: soil.name.clone(),
            lu_mgt,
        });
        hru_cons.push(Con {
            name: hname.clone(),
            gis_id: r.id as f64,
            area: r.arslp,
            lat: r.lat,
            lon: r.lon,
            elev: r.elev,
        });
        let rtu = *gis_to_rtu.get(&r.lsu).ok_or_else(|| {
            Error::msg(format!("the HRU {} is in the landscape unit {}, which gis_lsus does not have", r.id, r.lsu))
        })?;
        ru_ele.push(EleRow { name: hname.clone(), rtu, obj_id: i as i64 + 1, frac: r.arslp / r.arlsu });
        ls_ele.push(LsEle {
            name: hname,
            obj_typ_no: i as i64 + 1,
            bsn_frac: r.arslp / bsn_area,
            sub_frac: r.arslp / r.arlsu,
            rtu,
        });
    }
    // insert_aquifers
    let cnt = aqus.iter().map(|r| r.id).max().ok_or_else(|| empty("gis_aquifers"))?;
    let (mut gis_to_aqu, mut gis_to_deep): (HashMap<i64, i64>, HashMap<i64, i64>) = (HashMap::new(), HashMap::new());
    let mut aquifers: Vec<Aquifer> = Vec::new();
    for (i, r) in aqus.iter().enumerate() {
        gis_to_aqu.insert(r.id, i as i64 + 1);
        aquifers.push(Aquifer {
            name: name_of("aqu", r.id, cnt),
            deep: false,
            gis_id: r.id as f64,
            lat: r.lat,
            lon: r.lon,
            area: r.area,
            elev: r.elev,
        });
    }
    for r in &deeps {
        gis_to_deep.insert(r.id, aquifers.len() as i64 + 1);
        aquifers.push(Aquifer {
            name: name_of("aqu_deep", r.id, cnt),
            deep: true,
            gis_id: r.id as f64,
            lat: r.lat,
            lon: r.lon,
            area: r.area,
            elev: r.elev,
        });
    }
    // insert_connections
    let ids: [(&str, &HashMap<i64, i64>); 4] =
        [("CH", &gis_to_cha), ("AQU", &gis_to_aqu), ("DAQ", &gis_to_deep), ("LSU", &gis_to_rtu)];
    let rtu_out = connections(&routing, &["LSU"], &gis_to_rtu, &ids)?;
    let cha_out = connections(&routing, &["CH"], &gis_to_cha, &ids)?;
    let aqu_out = connections(&routing, &["AQU"], &gis_to_aqu, &ids)?;

    // the weather: files, stations at their coordinates, nearest files, generator and objects
    let (wfiles, records): (Vec<(String, String)>, Vec<Record>) = climate.files()?;
    for (n, t) in wfiles {
        put(&mut files, &n, t);
    }
    let mut coords: Vec<(f64, f64)> = records.iter().map(|r| (r.lat, r.lon)).collect();
    coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    coords.dedup();
    let wgns = climate.wgn_records()?;
    let wgn_ll: Vec<(f64, f64)> = wgns.iter().map(|w| (w.lat, w.lon)).collect();
    if wgns.is_empty() {
        return Err(Error::msg("the weather generator has no station"));
    }
    let mut stations: Vec<Sta> = Vec::new();
    for (la, lo) in coords {
        let mut sf: [Option<String>; 6] = Default::default();
        for (k, kind) in ["hmd", "pcp", "slr", "tmp", "wnd", "pet"].iter().enumerate() {
            let cands: Vec<&Record> = records.iter().filter(|r| r.ext == *kind).collect();
            if !cands.is_empty() {
                let ll: Vec<(f64, f64)> = cands.iter().map(|r| (r.lat, r.lon)).collect();
                sf[k] = Some(cands[nearest(la, lo, &ll)].fname.clone());
            }
        }
        stations.push(Sta {
            name: sta_name(la, lo),
            lat: la,
            lon: lo,
            files: sf,
            wgn: wgns[nearest(la, lo, &wgn_ll)].name.clone(),
        });
    }
    let sta_ll: Vec<(f64, f64)> = stations.iter().map(|s| (s.lat, s.lon)).collect();
    if stations.is_empty() {
        return Err(Error::msg("the project has no weather station"));
    }
    let wst = |la: f64, lo: f64| stations[nearest(la, lo, &sta_ll)].name.clone();

    // time.sim
    let (s0, e0) = if start.is_none() || end.is_none() { climate.period(&records)? } else { (0, 0) };
    let t = time_sim(start.unwrap_or(s0), end.unwrap_or(e0));

    // ---------------------------------------------------------------- the files
    put(
        &mut files,
        "time.sim",
        meta("time.sim")
            + &["day_start", "yrc_start", "day_end", "yrc_end", "step"].iter().map(|h| ihd(h)).collect::<String>()
            + "\n"
            + &t.iter().map(|v| ipad(*v as f64)).collect::<String>()
            + &ipad(0.0)
            + "\n",
    );
    let topo_cols = ["slp", "slp_len", "lat_len", "dist_cha", "depos"];
    put(
        &mut files,
        "topography.hyd",
        meta("topography.hyd")
            + &spl("name")
            + &topo_cols.iter().map(|h| hd(h)).collect::<String>()
            + "\n"
            + &topo_hru
                .iter()
                .chain(&topo_sub)
                .map(|r| {
                    spl(&r.name) + &npad(r.slp) + &npad(r.slp_len) + &npad(r.lat_len) + &npad(121.0) + &npad(0.0) + "\n"
                })
                .collect::<String>(),
    );
    let hcols = [
        "lat_ttime",
        "lat_sed",
        "can_max",
        "esco",
        "epco",
        "orgn_enrich",
        "orgp_enrich",
        "cn3_swf",
        "bio_mix",
        "perco",
        "lat_orgn",
        "lat_orgp",
        "pet_co",
        "latq_co",
    ];
    put(
        &mut files,
        "hydrology.hyd",
        meta("hydrology.hyd")
            + &spl("name")
            + &hcols.iter().map(|h| hd(h)).collect::<String>()
            + "\n"
            + &hyds
                .iter()
                .map(|(n, v)| spl(n) + &v.iter().map(|x| npad(*x)).collect::<String>() + "\n")
                .collect::<String>(),
    );
    put(
        &mut files,
        "field.fld",
        meta("field.fld")
            + &spl("name")
            + &["len", "wd", "ang"].iter().map(|h| hd(h)).collect::<String>()
            + "\n"
            + &fields.iter().map(|n| spl(n) + &npad(500.0) + &npad(100.0) + &npad(30.0) + "\n").collect::<String>(),
    );
    let hru_cols = ["topo", "hydro", "soil", "lu_mgt", "soil_plant_init", "surf_stor", "snow", "field"];
    put(
        &mut files,
        "hru-data.hru",
        meta("hru-data.hru")
            + &ihd("id")
            + &spl("name")
            + &hru_cols.iter().map(|h| sp(h)).collect::<String>()
            + "\n"
            + &hru_data
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    ipad(i as f64 + 1.0)
                        + &spl(&r.name)
                        + &sp(&r.topo)
                        + &sp(&r.hydro)
                        + &sp(&r.soil)
                        + &spn(r.lu_mgt.as_deref())
                        + &sp("soilplant1")
                        + &spn(None)
                        + &sp("snow001")
                        + &spn(None)
                        + "\n"
                })
                .collect::<String>(),
    );
    put(&mut files, "hru.con", con_table(&meta("hru.con"), "hru", &hru_cons, &vec![Vec::new(); hru_cons.len()], &wst));
    let rtu_cons: Vec<Con> = rtus
        .iter()
        .map(|r| Con { name: r.name.clone(), gis_id: r.gis_id, area: r.area, lat: r.lat, lon: r.lon, elev: r.elev })
        .collect();
    let outs_of = |m: &HashMap<i64, Vec<Out>>, n: usize| -> Vec<Vec<Out>> {
        (1..=n as i64).map(|i| m.get(&i).cloned().unwrap_or_default()).collect()
    };
    put(
        &mut files,
        "rout_unit.con",
        con_table(&meta("rout_unit.con"), "rtu", &rtu_cons, &outs_of(&rtu_out, rtus.len()), &wst),
    );
    let cha_cons: Vec<Con> = chas
        .iter()
        .map(|c| Con { name: c.name.clone(), gis_id: c.gis_id, area: c.area, lat: c.lat, lon: c.lon, elev: f64::NAN })
        .collect();
    put(
        &mut files,
        "chandeg.con",
        con_table(&meta("chandeg.con"), "lcha", &cha_cons, &outs_of(&cha_out, chas.len()), &wst),
    );
    let aqu_cons: Vec<Con> = aquifers
        .iter()
        .map(|a| Con { name: a.name.clone(), gis_id: a.gis_id, area: a.area, lat: a.lat, lon: a.lon, elev: a.elev })
        .collect();
    put(
        &mut files,
        "aquifer.con",
        con_table(&meta("aquifer.con"), "aqu", &aqu_cons, &outs_of(&aqu_out, aquifers.len()), &wst),
    );
    put(
        &mut files,
        "rout_unit.rtu",
        meta("rout_unit.rtu")
            + &ihd("id")
            + &["name", "define", "dlr", "topo", "field"].iter().map(|h| sp(h)).collect::<String>()
            + "\n"
            + &rtus
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    ipad(i as f64 + 1.0)
                        + &sp(&r.name)
                        + &sp(&r.name)
                        + &spn(None)
                        + &sp(&r.topo)
                        + &sp(&r.field)
                        + "\n"
                })
                .collect::<String>(),
    );
    put(
        &mut files,
        "rout_unit.ele",
        meta("rout_unit.ele")
            + &ihd("id")
            + &spl("name")
            + &spw("obj_typ", 12)
            + &ihd("obj_id")
            + &hd("frac")
            + &sp("dlr")
            + "\n"
            + &ru_ele
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    ipad(i as f64 + 1.0)
                        + &spl(&r.name)
                        + &spw("hru", 12)
                        + &ipad(r.obj_id as f64)
                        + &epad(r.frac, false)
                        + &spad(None, 16, true, "0")
                        + "\n"
                })
                .collect::<String>(),
    );
    put(
        &mut files,
        "rout_unit.def",
        meta("rout_unit.def")
            + &ihd("id")
            + &sp("name")
            + &ihd("elem_tot")
            + &ihd("elements")
            + "\n"
            + &rtus
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let k = i as i64 + 1;
                    let pos: Vec<i64> = ru_ele.iter().filter(|e| e.rtu == k).map(|e| e.obj_id).collect();
                    ipad(k as f64) + &sp(&r.name) + &ele_ids(&pos) + "\n"
                })
                .collect::<String>(),
    );
    let ls_defs: Vec<(i64, &Rtu)> = rtus
        .iter()
        .enumerate()
        .map(|(i, r)| (i as i64 + 1, r))
        .filter(|(i, _)| ru_ele.iter().any(|e| e.rtu == *i))
        .collect();
    put(
        &mut files,
        "ls_unit.def",
        meta("ls_unit.def")
            + &format!("{}\n", ls_defs.len())
            + &ihd("id")
            + &sp("name")
            + &hd("area")
            + &ihd("elem_tot")
            + &ihd("elements")
            + "\n"
            + &ls_defs
                .iter()
                .enumerate()
                .map(|(k, (i, r))| {
                    let pos: Vec<i64> = ls_ele.iter().filter(|e| e.rtu == *i).map(|e| e.obj_typ_no).collect();
                    ipad(k as f64 + 1.0) + &sp(&r.name) + &npad(r.area) + &ele_ids(&pos) + "\n"
                })
                .collect::<String>(),
    );
    put(
        &mut files,
        "ls_unit.ele",
        meta("ls_unit.ele")
            + &ihd("id")
            + &spl("name")
            + &spw("obj_typ", 12)
            + &ihd("obj_typ_no")
            + &hd("bsn_frac")
            + &hd("sub_frac")
            + &hd("reg_frac")
            + "\n"
            + &ls_ele
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    ipad(i as f64 + 1.0)
                        + &spl(&r.name)
                        + &spw("hru", 12)
                        + &ipadw(r.obj_typ_no as f64, 10)
                        + &epad(r.bsn_frac, true)
                        + &npad(r.sub_frac)
                        + &npad(0.0)
                        + "\n"
                })
                .collect::<String>(),
    );
    // (Python's builtin sum adds in order)
    let tot_area = rtus.iter().fold(0.0, |a, r| a + r.area);
    put(
        &mut files,
        "aqu_catunit.ele",
        meta("aqu_catunit.ele")
            + &ihd("id")
            + &spl("name")
            + &spw("obj_typ", 12)
            + &spad(Some("obj_typ_no"), 10, true, "0")
            + &hd("bsn_frac")
            + &hd("sub_frac")
            + &hd("reg_frac")
            + "\n"
            + &aquifers
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let k = i as f64 + 1.0;
                    ipad(k)
                        + &spl(&r.name)
                        + &spw("aqu", 12)
                        + &ipadw(k, 10)
                        + &npad_nz(r.area / tot_area)
                        + &npad(0.0)
                        + &npad(0.0)
                        + "\n"
                })
                .collect::<String>(),
    );
    let acols = [
        "gw_flo",
        "dep_bot",
        "dep_wt",
        "no3_n",
        "sol_p",
        "carbon",
        "flo_dist",
        "bf_max",
        "alpha_bf",
        "revap",
        "rchg_dp",
        "spec_yld",
        "hl_no3n",
        "flo_min",
        "revap_min",
    ];
    put(
        &mut files,
        "aquifer.aqu",
        meta("aquifer.aqu")
            + &ihd("id")
            + &spl("name")
            + &sp("init")
            + &acols.iter().map(|h| hd(h)).collect::<String>()
            + "\n"
            + &aquifers
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    ipad(i as f64 + 1.0)
                        + &spl(&r.name)
                        + &sp("initaqu1")
                        + &aquifer_defaults(r.deep).iter().map(|v| npad(*v)).collect::<String>()
                        + "\n"
                })
                .collect::<String>(),
    );
    put(
        &mut files,
        "channel-lte.cha",
        meta("channel-lte.cha")
            + &ihd("id")
            + &spl("name")
            + &["cha_ini", "cha_hyd", "cha_sed", "cha_nut"].iter().map(|h| sp(h)).collect::<String>()
            + "\n"
            + &chas
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    ipad(i as f64 + 1.0)
                        + &spl(&r.name)
                        + &sp("initcha1")
                        + &sp(&r.hyd)
                        + &spn(None)
                        + &sp("nutcha1")
                        + "\n"
                })
                .collect::<String>(),
    );
    put(&mut files, "hyd-sed-lte.cha", hyd_sed(&meta("hyd-sed-lte.cha"), &chas));
    let lum_list: Vec<&Lum> = lum_rows.iter().map(|(_, m)| m).collect();
    put(&mut files, "landuse.lum", landuse_text(&meta("landuse.lum"), &lum_list));
    put(&mut files, "plant.ini", plant_ini(&meta("plant.ini"), &pcoms));
    if !mgts.is_empty() {
        put(&mut files, "management.sch", management(&meta("management.sch"), &mgts));
    }
    put(&mut files, "soils.sol", soils_text(&meta("soils.sol"), &soils, &layers));
    let mut sorted: Vec<&Sta> = stations.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    put(
        &mut files,
        "weather-sta.cli",
        meta("weather-sta.cli")
            + &spl("name")
            + &sp("wgn")
            + &["pcp", "tmp", "slr", "hmd", "wnd", "pet"].iter().map(|h| spw(h, 25)).collect::<String>()
            + &sp("atmo_dep")
            + "\n"
            + &sorted
                .iter()
                .map(|s| {
                    // (files: hmd, pcp, slr, tmp, wnd, pet)
                    let f = |k: usize| spad(s.files[k].as_deref(), 25, true, "sim");
                    spl(&s.name)
                        + &sp(&s.wgn)
                        + &f(1)
                        + &f(3)
                        + &f(2)
                        + &f(0)
                        + &f(4)
                        + &spad(s.files[5].as_deref(), 25, true, "null")
                        + &spn(None)
                        + "\n"
                })
                .collect::<String>(),
    );
    put(&mut files, "weather-wgn.cli", wgn_text(&meta("weather-wgn.cli"), &wgns));
    let counts: [(&str, usize); 18] = [
        ("obj", hru_cons.len() + rtus.len() + aquifers.len() + chas.len()),
        ("hru", hru_cons.len()),
        ("lhru", 0),
        ("rtu", rtus.len()),
        ("gwfl", 0),
        ("aqu", aquifers.len()),
        ("cha", 0),
        ("res", 0),
        ("rec", 0),
        ("exco", 0),
        ("dlr", 0),
        ("can", 0),
        ("pmp", 0),
        ("out", 0),
        ("lcha", chas.len()),
        ("aqu2d", 0),
        ("hrd", 0),
        ("wro", 0),
    ];
    let ls_area = ls_defs.iter().fold(0.0, |a, (_, r)| a + r.area);
    let proj_name: String = name.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
    put(
        &mut files,
        "object.cnt",
        meta("object.cnt")
            + &spl("name")
            + &spw("ls_area", 12)
            + &spw("tot_area", 12)
            + &counts.iter().map(|(k, _)| ihd(k)).collect::<String>()
            + "\n"
            + &spad(Some(&proj_name), 16, false, "null")
            + &npad(ls_area)
            + &npad(tot_area)
            + &counts.iter().map(|(_, v)| ipad(*v as f64)).collect::<String>()
            + "\n",
    );
    let present: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    put(&mut files, "file.cio", file_cio(&meta("file.cio"), &|n| present.iter().any(|p| p == n)));
    Ok(Project {
        files: files.into_iter().map(|(n, t)| (n, normalize(t.as_bytes()))).collect(),
        watershed: Some(w.clone()),
    })
}

// ---------------------------------------------------------------------------------------------- the operator

struct FromTables;

fn port_table<'a>(args: &'a Args, port: &str) -> Result<Option<&'a Table>> {
    match args.get(port) {
        None => Ok(None),
        Some(Value::Table(t)) => Ok(Some(t)),
        Some(v) => Err(Error::msg(format!("{port} is a {:?}, not a table", v.ty()))),
    }
}

/// A date setting: text (2001-01-31, 20010131) or a number (20010131).
fn date_setting(args: &Args, name: &str) -> Result<Option<i64>> {
    let Some(v) = args.opt_param::<serde_json::Value>(name)? else { return Ok(None) };
    let text = match &v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    parse_date(&text)
        .map(Some)
        .ok_or_else(|| Error::msg(format!("the setting {name:?}: {text:?} is not a date (2001-01-31)")))
}

impl Operator for FromTables {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "swat.from_tables",
            "A SWAT+ project from a watershed (swat.watershed: QSWAT+'s tables) and the weather, as the SWAT+ Editor \
             4.0.2 makes it (landschaft's from_tables, file for file): the TxtInOut files (hru-data.hru, the \
             connections, routing units, aquifers, SWAT-DEG channels, land uses and their plant communities and \
             crop rotations, soils, weather stations at the coordinates of the weather files with each object \
             matched to the nearest station, the weather generator, time.sim and file.cio) as a value to run \
             (swat.run) or to change (swat.set); it keeps the watershed for the maps of the results. The weather is \
             tables: the stations (name, lat, lon, elev) and a daily series each of precipitation, maximum and \
             minimum temperature and, if there are, solar radiation, humidity and wind (one row per day: date, as \
             2001-01-31 or 20010131, or year and day; a column per station; empty for a missing day). The weather \
             generator's statistics are computed from them (swat.wgn) unless given in wgn. Wetland land uses are \
             refused (the port does not simulate them). The first line of each file carries the date it was written \
             (UTC).",
            Access::None,
        )
        .input(PortSpec::new("watershed", Type::SwatWatershed, "the watershed (swat.watershed)"))
        .input(PortSpec::new("stations", Type::Table, "the weather stations: name, lat, lon (degrees), elev (m)"))
        .input(PortSpec::new("pcp", Type::Table, "daily precipitation (mm)"))
        .input(PortSpec::new("tmax", Type::Table, "daily maximum temperature (deg C)"))
        .input(PortSpec::new("tmin", Type::Table, "daily minimum temperature (deg C)"))
        .input(PortSpec::optional("slr", Type::Table, "daily solar radiation (MJ/m2; SWAT+ generates it when absent)"))
        .input(PortSpec::optional("hmd", Type::Table, "daily relative humidity (fraction)"))
        .input(PortSpec::optional("wnd", Type::Table, "daily wind speed (m/s)"))
        .input(PortSpec::optional(
            "wgn",
            Type::Table,
            "the weather generator statistics as swat.wgn gives them (e.g. from SWAT+'s WGN database), instead of \
             computing them from the series",
        ))
        .output(PortSpec::new("project", Type::SwatProject, "the project"))
        .param(PortSpec::optional(
            "start",
            Type::Text,
            "the first day simulated, 2001-01-31 (default: the latest first day of the weather files)",
        ))
        .param(PortSpec::optional(
            "end",
            Type::Text,
            "the last day simulated (default: the earliest last day of the weather files)",
        ))
        .param(PortSpec::optional(
            "codes",
            Type::Any,
            "columns of codes.bsn to change, {\"pet\": 2} for Hargreaves, {\"rte_cha\": 1} for Muskingum",
        ))
        .param(PortSpec::optional("name", Type::Text, "the project's name (object.cnt; default landschaft)"))
        .param(PortSpec::optional(
            "monthly",
            Type::Any,
            "of the generator's statistics, the values not derived from daily data, by column (as swat.wgn): \
             pcp_hhr, the largest half-hour rainfall of each month, at least",
        ))
    }

    fn run(&self, _: &mut Ctx, args: &Args) -> Result<Outputs> {
        (|| -> Result<Outputs> {
            let Some(Value::SwatWatershed(w)) = args.get("watershed") else {
                return Err(Error::msg("connect the watershed (swat.watershed)"));
            };
            let need = |p: &str| port_table(args, p)?.ok_or_else(|| Error::msg(format!("connect the table {p}")));
            let stations = need("stations")?;
            let series = [
                ("pcp", Some(need("pcp")?)),
                ("tmax", Some(need("tmax")?)),
                ("tmin", Some(need("tmin")?)),
                ("slr", port_table(args, "slr")?),
                ("hmd", port_table(args, "hmd")?),
                ("wnd", port_table(args, "wnd")?),
            ];
            let monthly: Vec<(String, Monthly)> = parse_monthly(args.opt_param("monthly")?)?;
            let wx = climate(stations, &series, port_table(args, "wgn")?, &monthly)?;
            let codes: Vec<(String, serde_json::Value)> = args
                .opt_param::<serde_json::Map<String, serde_json::Value>>("codes")?
                .map(|m| m.into_iter().collect())
                .unwrap_or_default();
            let name: String = args.param("name", "landschaft".to_string())?;
            let project = from_tables(w, &wx, date_setting(args, "start")?, date_setting(args, "end")?, &codes, &name)?;
            let mut o = Outputs::new();
            o.insert("project".into(), Value::SwatProject(Arc::new(project)));
            Ok(o)
        })()
        .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_pythons() {
        assert_eq!(epad(1.03125, false).trim(), "1.0312E+0");
        assert_eq!(epad(0.28125, false).trim(), "2.8125E-1");
        assert_eq!(epad(0.0, false).trim(), "0.0000E+4");
        assert_eq!(epad(2.5e-9, false).trim(), "2.5000E-9");
        assert_eq!(epad(123456.5, false).trim(), "1.2346E+5");
        assert_eq!(py_repr(1e-5), "1e-05");
        assert_eq!(py_repr(2.0), "2.0");
        assert_eq!(py_repr(0.1), "0.1");
        assert_eq!(py_repr(1e16), "1e+16");
        assert_eq!(py_repr(123456789012345.0), "123456789012345.0");
        assert_eq!(py_repr(-2.5), "-2.5");
        assert_eq!(npad(0.123456).trim(), "0.12346");
        let want: String = ["3", "1", "-3", "7"].iter().map(|s| format!("{s:>8}  ")).collect();
        assert_eq!(ele_ids(&[1, 2, 3, 7]), want);
    }

    #[test]
    fn the_editors_data_is_there() {
        let e = editor().unwrap();
        assert!(e.statics.iter().any(|(n, _)| n == "codes.bsn"));
        assert!(e.data["plants"]["frsd"]["name"].is_string());
        assert_eq!(crate::swat_weather::days_from_civil(1970, 1, 1), 0);
    }
}
