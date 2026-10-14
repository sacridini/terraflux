//! Zonation 5's projects (landschaft's read_project and run_project): a settings file (`.z5`), its feature list
//! and link files read as landschaft reads them, with the switches of the z5 command line, and run by
//! prioritize.zonation; the calls of its post-processing file run on the ranking one after another.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value as Json, json};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::files::File;
use crate::prioritize::Zonation;
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(ZonationProject));
}

fn bad(m: impl std::fmt::Display) -> Error {
    Error::msg(m.to_string())
}

fn read_text(path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    Ok(text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text))
}

/// A line split into fields at whitespace, "double" or 'single' quotes keeping a field together (a shell's words).
fn fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut quote, mut any) = (None, false);
    for ch in line.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => cur.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                any = true;
            }
            None if ch.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            None => cur.push(ch),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A file's lines as fields, without # comments and empty lines.
fn rows(path: &Path) -> Result<Vec<Vec<String>>> {
    Ok(read_text(path)?
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(fields)
        .collect())
}

/// A value without the quotes around it (one optional quote at each end).
fn unquote(v: &str) -> String {
    let v = v.strip_prefix(['"', '\'']).unwrap_or(v);
    v.strip_suffix(['"', '\'']).unwrap_or(v).to_string()
}

/// The settings file: `key = value` lines, the keys in lower case (comments, sections and other lines skipped).
fn settings(path: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for line in read_text(path)?.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.insert(k.trim().to_lowercase(), unquote(v.trim()));
        }
    }
    Ok(out)
}

/// An INI file as Zonation reads it: the keys of a [section] as "[section] key", in lower case.
fn ini(path: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut section = String::new();
    for line in read_text(path)?.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = if section.is_empty() { k.trim().to_string() } else { format!("[{section}] {}", k.trim()) };
            out.insert(key.to_lowercase(), unquote(v.trim()));
        }
    }
    Ok(out)
}

fn resolve(name: &str, base: &Path) -> PathBuf {
    let p = Path::new(name);
    if p.is_absolute() { p.to_path_buf() } else { base.join(p) }
}

fn parent(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
}

/// A number as Python's float() reads it.
fn num(v: &str) -> Result<f64> {
    v.trim().parse::<f64>().map_err(|_| bad(format!("{v:?} is not a number")))
}

/// A whole number as int(float(v)).
fn int(v: &str) -> Result<i64> {
    Ok(num(v)? as i64)
}

/// The z5 command line's switches: the letters on, the analysis area (`--area=N`) and the mode (`--mode=...`);
/// -W turns on -w, -X turns on -x. Other words (the settings file, the output directory) are left out.
struct Flags {
    on: BTreeSet<char>,
    area: i64,
    mode: Option<String>,
}

fn flags(text: &str) -> Result<Flags> {
    let mut f = Flags { on: BTreeSet::new(), area: 0, mode: None };
    for w in fields(text) {
        if let Some(a) = w.strip_prefix("--area=") {
            f.area = a.parse().map_err(|_| bad(format!("flags: --area={a} is not a whole number")))?;
        } else if let Some(m) = w.strip_prefix("--mode=") {
            f.mode = Some(m.to_string());
        } else if w.starts_with("--") {
            continue;
        } else if let Some(letters) = w.strip_prefix('-') {
            f.on.extend(letters.chars());
        }
    }
    if f.on.contains(&'W') {
        f.on.insert('w');
    }
    if f.on.contains(&'X') {
        f.on.insert('x');
    }
    Ok(f)
}

/// A line of a connectivity link file: its id, the words before the kernel, the kernel and the words after it.
struct KernelLine {
    id: i64,
    head: Vec<String>,
    kernel: Json,
    tail: Vec<String>,
}

/// A connectivity link file: `id [options] kernel(parameters) [file]` per line.
fn kernel_links(path: &Path) -> Result<BTreeMap<i64, KernelLine>> {
    let re = regex::Regex::new(r"\s*([A-Za-z]+)\s*\(([^)]*)\)\s*").unwrap();
    let mut out = BTreeMap::new();
    for line in read_text(path)?.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let m = re.captures(line).ok_or_else(|| bad(format!("{}: no kernel in {line:?}", path.display())))?;
        let whole = m.get(0).unwrap();
        let head: Vec<String> = line[..whole.start()].split_whitespace().map(String::from).collect();
        let params: Vec<f64> = m[2].split(',').filter(|x| !x.trim().is_empty()).map(num).collect::<Result<Vec<_>>>()?;
        let Some(&distance) = params.first() else {
            return Err(bad(format!("{}: a kernel without its distance in {line:?}", path.display())));
        };
        let kernel =
            json!({"shape": m[1].to_lowercase(), "distance": distance, "cut": params.get(1).copied().unwrap_or(1.0)});
        let id = int(head.first().ok_or_else(|| bad(format!("{}: no id in {line:?}", path.display())))?)?;
        out.insert(id, KernelLine { id, head, kernel, tail: fields(&line[whole.end()..]) });
    }
    Ok(out)
}

/// A rows x cols matrix from the first rows and columns of a file (0 where it has none).
fn matrix(path: &Path, nrows: usize, ncols: usize) -> Result<Vec<Vec<f64>>> {
    let mut m = vec![vec![0.0; ncols]; nrows];
    for (r, vals) in rows(path)?.iter().take(nrows).enumerate() {
        for (c, v) in vals.iter().take(ncols).enumerate() {
            m[r][c] = num(v)?;
        }
    }
    Ok(m)
}

fn bit(v: &str) -> Result<bool> {
    Ok(int(v)? != 0)
}

/// A feature of the feature list: its file, the value extracted from it (`file.tif/3`) and its name.
#[derive(Debug, Clone)]
pub struct Feature {
    pub file: PathBuf,
    pub extract: Option<f64>,
    pub name: Option<String>,
}

/// A Zonation project read: its features, the maps of prioritize.zonation's inputs (their files), the maps its
/// settings name (a stack per input, the maps named by their files) and prioritize.zonation's settings.
#[derive(Debug, Default)]
pub struct Project {
    pub features: Vec<Feature>,
    pub maps: BTreeMap<String, PathBuf>,
    pub stacks: BTreeMap<String, Vec<PathBuf>>,
    pub params: serde_json::Map<String, Json>,
    pub post: Option<PathBuf>,
}

impl Project {
    fn map_of(&mut self, port: &str, path: PathBuf) -> Json {
        let key = path.to_string_lossy().into_owned();
        let list = self.stacks.entry(port.into()).or_default();
        if !list.contains(&path) {
            list.push(path);
        }
        Json::String(key)
    }
}

/// The impact scenarios of a project (-s, -e), their maps added to the project's impact maps.
fn impact_scenarios(
    st: &BTreeMap<String, String>,
    base: &Path,
    on: &BTreeSet<char>,
    ngroups: usize,
    p: &mut Project,
) -> Result<()> {
    let common =
        |ini: &BTreeMap<String, String>, at: &Path, p: &mut Project| -> Result<serde_json::Map<String, Json>> {
            let mut m = serde_json::Map::new();
            m.insert("name".into(), ini.get("project id").cloned().unwrap_or_else(|| "scenario".into()).into());
            let factor = ini.get("area conversion factor").filter(|v| !v.is_empty()).map_or(Ok(1.0), |v| num(v))?;
            m.insert("area_factor".into(), json!(if factor == 0.0 { 1.0 } else { factor }));
            if let Some(s) = ini.get("subareas raster file").filter(|v| !v.is_empty()) {
                m.insert("subareas".into(), p.map_of("impact_maps", resolve(s, at)));
            }
            let rasters =
                ini.get("output intermediate raster maps").filter(|v| !v.is_empty()).map_or(Ok(1), |v| int(v))?;
            m.insert("rasters".into(), json!(rasters != 0));
            Ok(m)
        };
    if on.contains(&'s')
        && let Some(f) = st.get("impact assessment scenario file").filter(|v| !v.is_empty())
    {
        let f = resolve(f, base);
        let at = parent(&f);
        let ini = ini(&f)?;
        let mut impacts = Vec::new();
        if let Some(b) = st.get("impact buffers file").filter(|v| !v.is_empty()) {
            for r in rows(&resolve(b, base))? {
                let vals: Vec<f64> = r.iter().map(|x| num(x)).collect::<Result<_>>()?;
                if vals.len() < 3 {
                    return Err(bad(format!("{b}: a line with {} values (group, source, footprint...)", vals.len())));
                }
                let buffers: Vec<Json> = (3..vals.len().saturating_sub(1))
                    .step_by(2)
                    .map(|i| json!([vals[i] as i64, vals[i + 1]]))
                    .collect();
                impacts.push(json!({"group": vals[0] as i64, "source": vals[1] as i64, "footprint": vals[2],
                                    "buffers": buffers}));
            }
        }
        let mut sc = common(&ini, &at, p)?;
        let src = ini
            .get("impact source raster file")
            .ok_or_else(|| bad(format!("{}: no 'impact source raster file'", f.display())))?;
        sc.insert("sources".into(), p.map_of("impact_maps", resolve(src, &at)));
        sc.insert("impacts".into(), impacts.into());
        p.params.insert("impact".into(), Json::Object(sc));
    }
    if on.contains(&'e')
        && let Some(f) = st.get("cumulative impact assessment scenario file").filter(|v| !v.is_empty())
    {
        let f = resolve(f, base);
        let at = parent(&f);
        let ini = ini(&f)?;
        let mut cs = common(&ini, &at, p)?;
        let mode = |key: &str| ini.get(key).filter(|v| !v.is_empty()).map_or(Ok(0), |v| int(v));
        let need = |key: &str| ini.get(key).ok_or_else(|| bad(format!("{}: no {key:?}", f.display())));
        let dmode = mode("analyze direct habitat loss")?;
        if dmode != 0 {
            cs.insert("direct".into(), json!(if dmode == 1 { "sum" } else { "product" }));
            let list = resolve(need("direct habitat loss raster list")?, &at);
            let mut direct = Vec::new();
            for r in rows(&list)?.iter().skip(1) {
                direct.push(p.map_of("impact_maps", resolve(&r[0], &parent(&list))));
            }
            let link = resolve(need("direct habitat loss link matrix")?, &at);
            cs.insert("direct_matrix".into(), json!(matrix(&link, ngroups, direct.len())?));
            cs.insert("direct_rasters".into(), direct.into());
        }
        let cmode = mode("analyze cumulative pressures")?;
        if cmode != 0 {
            cs.insert("cumulative".into(), json!(if cmode == 1 { "variant1" } else { "variant2" }));
            let list = resolve(need("cumulative pressures raster list")?, &at);
            let table = rows(&list)?;
            let head: Vec<String> =
                table.first().map(|h| h.iter().map(|x| x.to_lowercase()).collect()).unwrap_or_default();
            let mut pressures = Vec::new();
            for r in table.iter().skip(1) {
                let d: BTreeMap<&str, &str> =
                    head.iter().map(String::as_str).zip(r.iter().map(String::as_str)).collect();
                let file = d.get("filename").ok_or_else(|| bad(format!("{}: no 'filename' column", list.display())))?;
                pressures.push(json!({
                    "raster": p.map_of("impact_maps", resolve(file, &parent(&list))),
                    "normalize": d.get("normalize").map_or(Ok(false), |v| bit(v))?,
                    "offset": d.get("offset").map_or(Ok(0.0), |v| num(v))?,
                    "divisor": d.get("divisor").map_or(Ok(1.0), |v| num(v))?,
                }));
            }
            let aggregation = ["add", "multiply", "max"]
                .get(mode("individual pressure aggregation")? as usize)
                .ok_or_else(|| bad(format!("{}: individual pressure aggregation is 0, 1 or 2", f.display())))?;
            cs.insert("aggregation".into(), json!(aggregation));
            if cmode == 1 {
                let link = resolve(need("cumulative pressures link matrix")?, &at);
                cs.insert("cumulative_matrix".into(), json!(matrix(&link, ngroups, pressures.len())?));
            }
            cs.insert("pressures".into(), pressures.into());
            let re = regex::Regex::new(r"^\[cumulative pressure functions\] group (\d+)( pressure (\d+))?$").unwrap();
            let mut functions = Vec::new();
            for (key, val) in &ini {
                if let Some(m) = re.captures(key) {
                    let points: Vec<Json> = rows(&resolve(val, &at))?
                        .iter()
                        .map(|r| -> Result<Json> {
                            match r.as_slice() {
                                [a, b, ..] => Ok(json!([num(a)?, num(b)?])),
                                _ => Err(bad(format!("{val}: a point is two numbers"))),
                            }
                        })
                        .collect::<Result<_>>()?;
                    let pressure: Option<i64> = m.get(3).map(|x| x.as_str().parse().unwrap());
                    functions
                        .push(json!({"group": m[1].parse::<i64>().unwrap(), "pressure": pressure, "points": points}));
                }
            }
            cs.insert("functions".into(), functions.into());
        }
        p.params.insert("cumulative_impact".into(), Json::Object(cs));
    }
    Ok(())
}

/// A Zonation 5 project: the settings file (`.z5`) with the z5 command line's switches (`-w` weights, `-W` weight
/// groups, `-g` output groups, `-t` thresholds, `-c` conditions, `-r` retention, `-x`/`-X` cost tracked or in the
/// ranking, `-h` hierarchic mask, `-a` analysis area with `--area=N`, `-b` pre-processing maps, `-f` the features
/// as ranked, `-d`/`-i`/`-I`/`-m` connectivity, `-s`/`-e` impact scenarios, `-p` post-processing) and the marginal
/// loss rule (z5's `--mode`), as prioritize.zonation's inputs and settings.
pub fn read_project(settings_file: &Path, switches: &str, mode: Option<&str>) -> Result<Project> {
    let base = parent(settings_file);
    let st = settings(settings_file)?;
    let f = flags(switches)?;
    let on = &f.on;
    let mode = mode.map(String::from).or(f.mode.clone());
    let mut p = Project::default();
    let fl_name = st
        .get("feature list file")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| bad(format!("{}: no 'feature list file'", settings_file.display())))?;
    let fl = resolve(fl_name, &base);
    let table = rows(&fl)?;
    let header: Vec<String> = table.first().map(|h| h.iter().map(|x| x.to_lowercase()).collect()).unwrap_or_default();
    if !header.iter().any(|h| h == "filename") {
        return Err(bad(format!("{}: no 'filename' column", fl.display())));
    }
    let mut cols: BTreeMap<String, Vec<String>> = header.iter().map(|h| (h.clone(), Vec::new())).collect();
    for r in &table[1..] {
        if r.len() != header.len() {
            return Err(bad(format!("{}: a line with {} fields for {} columns", fl.display(), r.len(), header.len())));
        }
        for (h, v) in header.iter().zip(r) {
            cols.get_mut(h).unwrap().push(v.clone());
        }
    }
    let n = cols["filename"].len();
    for (i, name) in cols["filename"].iter().enumerate() {
        let mut file = resolve(name, &parent(&fl));
        let mut extract = None;
        // "file.tif/3": the cells of value 3
        if !file.exists()
            && let Some(par) = file.parent().filter(|x| x.is_file())
        {
            extract = Some(num(&file.file_name().unwrap_or_default().to_string_lossy())?);
            file = par.to_path_buf();
        }
        p.features.push(Feature { file, extract, name: cols.get("name").map(|c| c[i].clone()) });
    }
    let nums = |col: &str| -> Result<Option<Vec<f64>>> {
        cols.get(col).map(|c| c.iter().map(|v| num(v)).collect::<Result<Vec<_>>>()).transpose()
    };
    let ints = |col: &str| -> Result<Option<Vec<i64>>> {
        cols.get(col).map(|c| c.iter().map(|v| int(v)).collect::<Result<Vec<_>>>()).transpose()
    };
    let link = |key: &str| st.get(key).filter(|v| !v.is_empty()).map(|v| resolve(v, &base));
    let params = &mut p.params;
    if p.features.iter().any(|f| f.extract.is_some()) {
        params.insert("extract".into(), json!(p.features.iter().map(|f| f.extract).collect::<Vec<_>>()));
    }
    if let Some(m) = &mode {
        params.insert("rule".into(), json!(m));
    }
    if on.contains(&'w') {
        params.insert("weights".into(), json!(nums("weight")?.unwrap_or_else(|| vec![1.0; n])));
    }
    if on.contains(&'W') {
        params.insert("weight_groups".into(), json!(ints("wgrp")?));
        if let Some(g) = link("weight groups file") {
            let mut gw = serde_json::Map::new();
            for r in rows(&g)? {
                if let [id, w, ..] = r.as_slice() {
                    gw.insert(int(id)?.to_string(), json!(num(w)?));
                }
            }
            params.insert("group_weights".into(), Json::Object(gw));
        }
    }
    if on.contains(&'g')
        && let Some(g) = ints("group")?
    {
        params.insert("groups".into(), json!(g));
    }
    if on.contains(&'t')
        && let Some(t) = nums("threshold")?
    {
        params.insert("thresholds".into(), json!(t));
    }
    if let Some(z) = nums("abfz")? {
        params.insert("abf_z".into(), json!(z));
    }
    if let Some(zm) = st.get("zero mode").filter(|v| !v.is_empty()) {
        params.insert("zero_mode".into(), json!(zm.to_lowercase()));
    }
    if let Some(k) = ints("keep_zeros")? {
        params.insert("keep_zeros".into(), json!(k.iter().map(|v| u8::from(*v != 0)).collect::<Vec<_>>()));
    }
    if on.contains(&'c')
        && let (Some(file), Some(ids)) = (link("condition link file"), ints("condition")?)
    {
        let mut links = BTreeMap::new();
        for r in rows(&file)? {
            if let [id, renorm, path, ..] = r.as_slice() {
                links.insert(int(id)?, (bit(renorm)?, resolve(path, &parent(&file))));
            }
        }
        let mut cond = Vec::new();
        let mut renorm = Vec::new();
        for i in &ids {
            match links.get(i) {
                Some((r, path)) => {
                    cond.push(p.map_of("condition_maps", path.clone()));
                    renorm.push(*r);
                }
                None => {
                    cond.push(Json::Null);
                    renorm.push(false);
                }
            }
        }
        p.params.insert("condition".into(), cond.into());
        p.params.insert("renormalize".into(), json!(renorm));
    }
    if on.contains(&'r')
        && let (Some(file), Some(ids)) = (link("retention link file"), ints("retention")?)
    {
        let mut links = BTreeMap::new();
        for r in rows(&file)? {
            if let [id, kind, v, ..] = r.as_slice() {
                links.insert(int(id)?, (int(kind)?, v.clone()));
            }
        }
        let mut ret = Vec::new();
        for i in &ids {
            ret.push(match links.get(i) {
                None => Json::Null,
                Some((0, path)) => json!({"absolute": p.map_of("retention_maps", resolve(path, &parent(&file)))}),
                Some((1, path)) => p.map_of("retention_maps", resolve(path, &parent(&file))),
                Some((_, v)) => json!(num(v)?),
            });
        }
        p.params.insert("retention".into(), ret.into());
    }
    let sf_link = link("single-feature connectivity link file");
    if on.contains(&'d')
        && let (Some(file), Some(ids)) = (sf_link, ints("sf_conn")?)
    {
        let links = kernel_links(&file)?;
        let kernels: Vec<Json> = ids.iter().map(|i| links.get(i).map_or(Json::Null, |l| l.kernel.clone())).collect();
        let selfm: Vec<bool> = ids
            .iter()
            .map(|i| links.get(i).and_then(|l| l.head.get(1)).map_or(Ok(false), |v| bit(v)))
            .collect::<Result<_>>()?;
        p.params.insert("connectivity".into(), kernels.into());
        p.params.insert("self_multiply".into(), json!(selfm));
    }
    for (flag, col, key) in [('i', "ia_conn1", "interaction"), ('I', "ia_conn2", "interaction2")] {
        if on.contains(&flag)
            && let (Some(file), Some(ids)) = (link("interaction connectivity link file"), ints(col)?)
        {
            let links = kernel_links(&file)?;
            let mut list = Vec::new();
            for i in &ids {
                list.push(match links.get(i) {
                    None => Json::Null,
                    Some(l) => {
                        let (Some(pos), Some(beta), Some(norm), Some(res)) =
                            (l.head.get(1), l.head.get(2), l.head.get(3), l.tail.first())
                        else {
                            return Err(bad(format!(
                                "{}: interaction {}: id positive beta normalize kernel(...) resource",
                                file.display(),
                                l.id
                            )));
                        };
                        json!({"kernel": l.kernel, "resource": p.map_of("resources", resolve(res, &parent(&file))),
                               "positive": bit(pos)?, "beta": num(beta)?, "normalize": bit(norm)?})
                    }
                });
            }
            p.params.insert(key.into(), list.into());
        }
    }
    if on.contains(&'m')
        && let (Some(file), Some(ids)) = (link("matrix connectivity link file"), ints("matrix_conn")?)
    {
        let links = kernel_links(&file)?;
        let mut list = Vec::new();
        let mut matrices = serde_json::Map::new();
        for i in &ids {
            let Some(l) = links.get(i) else {
                list.push(Json::Null);
                continue;
            };
            let m = resolve(
                l.tail.first().ok_or_else(|| bad(format!("{}: matrix link {i} names no matrix", file.display())))?,
                &parent(&file),
            );
            let key = m.to_string_lossy().into_owned();
            list.push(json!({"kernel": l.kernel, "matrix": key}));
            if !matrices.contains_key(&key) {
                // a row per feature linked to the matrix
                let linked = ids
                    .iter()
                    .filter(|j| {
                        links.get(j).and_then(|x| x.tail.first()).is_some_and(|t| resolve(t, &parent(&file)) == m)
                    })
                    .count();
                let r: Vec<Vec<f64>> = rows(&m)?
                    .iter()
                    .take(linked)
                    .map(|r| r.iter().map(|v| num(v)).collect::<Result<_>>())
                    .collect::<Result<_>>()?;
                matrices.insert(key, json!(r));
            }
        }
        p.params.insert("matrix_connectivity".into(), list.into());
        p.params.insert("matrices".into(), Json::Object(matrices));
    }
    if (on.contains(&'s') || on.contains(&'e'))
        && let Some(groups) = ints("impact_grp")?
    {
        p.params.insert("impact_groups".into(), json!(groups));
        if let Some(bins) = ints("impact_bins")? {
            let mut file = BTreeMap::new();
            if let Some(b) = link("binning link file") {
                for r in rows(&b)? {
                    if let Some((id, limits)) = r.split_first() {
                        file.insert(int(id)?, limits.iter().map(|v| num(v)).collect::<Result<Vec<_>>>()?);
                    }
                }
            }
            let list: Vec<Json> = bins
                .iter()
                .map(|&b| match b {
                    b if b < 0 => Json::Null,
                    0 => json!(0),
                    b => file.get(&b).map_or(Json::Null, |l| json!(l)),
                })
                .collect();
            p.params.insert("impact_bins".into(), list.into());
        }
        let ngroups = groups.iter().filter(|g| **g >= 0).collect::<BTreeSet<_>>().len();
        impact_scenarios(&st, &base, on, ngroups, &mut p)?;
    }
    if on.contains(&'x')
        && let Some(c) = link("cost layer")
    {
        p.maps.insert("cost".into(), c);
        p.params.insert("cost_in_ranking".into(), json!(on.contains(&'X')));
    }
    if on.contains(&'h')
        && let Some(h) = link("hierarchic mask layer")
    {
        p.maps.insert("hierarchic_mask".into(), h);
    }
    if on.contains(&'a')
        && let Some(a) = link("analysis area mask layer")
    {
        p.maps.insert("analysis_area".into(), a);
        p.params.insert("area".into(), json!(f.area));
    }
    if let Some(s) = link("external solution")
        && mode.as_deref().and_then(|m| m.split_whitespace().next()).is_some_and(|m| m.eq_ignore_ascii_case("LOAD"))
    {
        p.maps.insert("solution".into(), s);
    }
    if on.contains(&'b') {
        p.params.insert("preprocessing".into(), json!(true));
    }
    if on.contains(&'p') {
        p.post = link("post-processing file");
    }
    if on.contains(&'f')
        && let Some(t) = ints("tr_out")?
    {
        p.params.insert("transformed".into(), json!(t.iter().map(|v| *v != 0).collect::<Vec<_>>()));
    }
    for (key, arg) in [
        ("max iterations", "max_iterations"),
        ("mean ranking error threshold", "ranking_error"),
        ("smoothing factor", "smoothing_factor"),
        ("curves output interval", "curves_interval"),
    ] {
        if let Some(v) = st.get(key).filter(|v| !v.is_empty()) {
            let x = num(v)?;
            p.params.insert(arg.into(), if arg == "max_iterations" { json!(x as i64) } else { json!(x) });
        }
    }
    if let Some(v) = st.get("stop on plateau").filter(|v| !v.is_empty()) {
        p.params.insert("stop_on_plateau".into(), json!(matches!(v.trim().to_lowercase().as_str(), "1" | "true")));
    }
    Ok(p)
}

/// A parameter of a post-processing call.
#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Text(String),
    Bool(bool),
    Number(f64),
}

/// The calls of a post-processing file: `name(param, ...)` with numbers, true/false and "strings"; # starts a
/// comment on a line without quotes before it.
pub fn parse_calls(text: &str) -> Vec<(String, Vec<CallArg>)> {
    let body: Vec<&str> = text
        .lines()
        .map(|line| {
            let head = line.split('#').next().unwrap_or("");
            if head.contains('"') { line } else { head }
        })
        .collect();
    let body = body.join("\n");
    let call = regex::Regex::new(r"([A-Za-z][A-Za-z0-9_]+)\s*\(([^)]*)\)").unwrap();
    let token = regex::Regex::new(r#""[^"]*"|[^,\s][^,]*"#).unwrap();
    let int = regex::Regex::new(r"^[+-]?\d+$").unwrap();
    let mut calls = Vec::new();
    for m in call.captures_iter(&body) {
        let mut params = Vec::new();
        for t in token.find_iter(&m[2]) {
            let t = t.as_str().trim();
            if t.is_empty() {
                continue;
            }
            params.push(if let Some(s) = t.strip_prefix('"') {
                CallArg::Text(s.strip_suffix('"').unwrap_or(s).to_string())
            } else if t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("false") {
                CallArg::Bool(t.eq_ignore_ascii_case("true"))
            } else if int.is_match(t) {
                CallArg::Number(t.parse::<i64>().map_or(f64::NAN, |v| v as f64))
            } else {
                CallArg::Number(t.parse().unwrap_or(f64::NAN))
            });
        }
        calls.push((m[1].to_string(), params));
    }
    calls
}

/// A raster file read: its bands (no-data values as NaN), its grid and no-data value and the height of its blocks.
struct Bands {
    bands: Vec<Vec<f64>>,
    grid: terraflux_raster::Grid,
    nodata: Option<f64>,
    block_rows: usize,
}

fn read_file(path: &Path) -> Result<Bands> {
    let f = File::open(path)?;
    let meta = f.meta();
    let nodata = meta.nodata.filter(|v| !v.is_nan());
    let mut bands = Vec::new();
    for b in 0..meta.bands.len() {
        let mut v = f.read_band(b)?;
        if let Some(nd) = nodata {
            v.iter_mut().filter(|x| **x == nd).for_each(|x| *x = f64::NAN);
        }
        bands.push(v);
    }
    Ok(Bands { bands, grid: meta.grid.clone(), nodata, block_rows: f.block_rows() })
}

/// A map of a project on the features' grid.
fn read_map(path: &Path, grid: &Arc<terraflux_raster::Grid>) -> Result<Map> {
    let Bands { mut bands, grid: g, .. } = read_file(path)?;
    if (g.height, g.width) != (grid.height, grid.width) {
        return Err(bad(format!(
            "{}: {} x {} cells, not the features' {} x {}",
            path.display(),
            g.height,
            g.width,
            grid.height,
            grid.width
        )));
    }
    Ok(Map::new(grid.clone(), bands.swap_remove(0)))
}

/// A project's inputs of prioritize.zonation and its settings with a value per feature for each band of the
/// features' files (a file of several bands: the settings of its line for each).
fn inputs(p: &Project) -> Result<(BTreeMap<String, Value>, serde_json::Map<String, Json>)> {
    let mut keys = Vec::new();
    let mut layers = Vec::new();
    let mut nodata = Vec::new();
    let mut copies = Vec::new();
    let mut grid: Option<Arc<terraflux_raster::Grid>> = None;
    let mut block_rows = usize::MAX;
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for f in &p.features {
        let Bands { bands, grid: g, nodata: nd, block_rows: rows } = read_file(&f.file)?;
        match &grid {
            None => grid = Some(Arc::new(g)),
            Some(first) if (g.height, g.width) != (first.height, first.width) => {
                return Err(bad(format!("{}: not on the grid of the other features", f.file.display())));
            }
            Some(_) => {}
        }
        block_rows = block_rows.min(rows);
        copies.push(bands.len());
        let stem = f.file.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let several = bands.len() > 1;
        for (b, v) in bands.into_iter().enumerate() {
            let mut key = f.name.clone().unwrap_or_else(|| {
                let mut k = stem.clone();
                if several {
                    k += &format!(".{}", b + 1);
                }
                if let Some(x) = f.extract {
                    k += &format!("/{x}");
                }
                k
            });
            let count = seen.entry(key.clone()).or_insert(0);
            *count += 1;
            if *count > 1 {
                key += &format!(" ({count})");
            }
            keys.push(Key::Name(key));
            layers.push(Arc::new(v));
            nodata.push(nd.map_or(Json::Null, |v| json!(v as f32)));
        }
    }
    let grid = grid.ok_or_else(|| bad("the feature list has no features"))?;
    let mut params = p.params.clone();
    // the settings per feature, repeated for the bands of a file
    if copies.iter().any(|c| *c != 1) {
        for v in params.values_mut() {
            if let Json::Array(a) = v
                && a.len() == copies.len()
            {
                *a = a.iter().zip(&copies).flat_map(|(x, c)| std::iter::repeat_n(x.clone(), *c)).collect();
            }
        }
    }
    if nodata.iter().any(|v| !v.is_null()) {
        params.insert("nodata".into(), nodata.into());
    }
    params.insert("block_rows".into(), json!(block_rows.min(grid.height)));
    let mut ins = BTreeMap::new();
    ins.insert("features".to_string(), Value::Stack(Stack { grid: grid.clone(), keys, layers }));
    for (port, path) in &p.maps {
        ins.insert(port.clone(), Value::Map(read_map(path, &grid)?));
    }
    for (port, paths) in &p.stacks {
        let mut s = Stack { grid: grid.clone(), keys: Vec::new(), layers: Vec::new() };
        for path in paths {
            s.keys.push(Key::Name(path.to_string_lossy().into_owned()));
            s.layers.push(read_map(path, &grid)?.data);
        }
        ins.insert(port.clone(), Value::Stack(s));
    }
    Ok((ins, params))
}

/// The outputs of prioritize.zonation's post-processing that a project's calls give, with the call's number.
const POST_TABLES: [&str; 4] = ["area_statistics", "coverage", "comparison", "histogram"];

pub struct ZonationProject;

impl Operator for ZonationProject {
    fn signature(&self) -> OpSignature {
        let zonation = Zonation.signature();
        let post =
            ["area_statistics", "coverage", "area_mask", "comparison", "comparison_map", "histogram", "flexibility"];
        let mut sig = OpSignature::new(
            "prioritize.zonation_project",
            "A Zonation 5 project run as the z5 binary runs it (landschaft's run_project): the settings file (.z5), \
             its feature list and link files (conditions, retention, connectivity, weight groups, impact scenarios, \
             binning) read with the switches of the z5 command line, the rasters they name read from disk; the \
             ranking of prioritize.zonation, and with -p the calls of the post-processing file one after another.",
            Access::Global,
        )
        .param(PortSpec::new("path", Type::Text, "the project's settings file (.z5)"))
        .param(PortSpec::optional(
            "flags",
            Type::Text,
            "the z5 switches: -w weights, -W weight groups, -g output groups, -t thresholds, -c conditions, \
             -r retention, -x/-X cost (tracked / in the ranking), -h hierarchic mask, -a analysis area \
             (--area=N), -b pre-processing maps, -f the features as ranked, -d/-i/-I/-m connectivity, -s/-e \
             impact scenarios, -p post-processing (e.g. \"-wg -X\")",
        ))
        .param(PortSpec::optional(
            "mode",
            Type::Text,
            "the marginal loss rule, z5's --mode (CAZ2 by default; \"ABF 0.25\"...); a --mode= in flags too",
        ))
        .param(PortSpec::optional("bits", Type::Integer, "8 (z5, the default) or 16 (z5_16bit)").choices([8, 16]));
        sig.outputs = zonation.outputs.into_iter().filter(|o| !post.contains(&o.name.as_str())).collect();
        sig.outputs.extend([
            PortSpec::optional(
                "area_statistics",
                Type::Table,
                "-p: the statistics of areas of every call (area_statistics_external, _rank_range, \
                 _separate_areas), the call's number first",
            ),
            PortSpec::optional("coverage", Type::Table, "-p: per call, set and area: the share of every feature in it"),
            PortSpec::optional("comparison", Type::Table, "-p: per solution_comparison call: the cells in both..."),
            PortSpec::optional("histogram", Type::Table, "-p: per histogram call, set and bin: the count of features"),
            PortSpec::optional(
                "post_maps",
                Type::Stack,
                "-p: the calls' maps named as Zonation's files (area_statistics_rank_range.mask.<id>, \
                 solution_comparison.mask.<id>, flexibilitymap.<id>.up...)",
            ),
        ]);
        sig
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let fail = |e: Error| Error::msg(format!("node {node:?}: {e}"));
        let path: String = args.param("path", String::new())?;
        if path.is_empty() {
            return Err(fail(bad("path: the project's settings file (.z5)")));
        }
        let switches: String = args.param("flags", String::new())?;
        let mode: Option<String> = args.opt_param("mode")?;
        let project = read_project(Path::new(&path), &switches, mode.as_deref()).map_err(fail)?;
        let (ins, mut params) = inputs(&project).map_err(fail)?;
        if let Some(b) = args.params.get("bits").filter(|v| !v.is_null()) {
            params.insert("bits".into(), b.clone());
        }
        let zargs = Args { node, inputs: ins, params: &params };
        let (mut out, ranked) = Zonation.rank(&zargs)?;
        let Some(pp) = &project.post else { return Ok(out) };
        let at = parent(pp);
        let grid = ranked_grid(&out);
        let mut maps = Stack { grid: grid.clone(), keys: Vec::new(), layers: Vec::new() };
        let mut tables: BTreeMap<&str, Table> = BTreeMap::new();
        for (i, (name, call)) in parse_calls(&read_text(pp).map_err(fail)?).iter().enumerate() {
            let number = (i + 1) as f64;
            let num_at = |k: usize| -> Result<f64> {
                match call.get(k) {
                    Some(CallArg::Number(x)) => Ok(*x),
                    _ => Err(fail(bad(format!("{name}: parameter {} is a number", k + 1)))),
                }
            };
            let flag_at = |k: usize| match call.get(k) {
                Some(CallArg::Bool(b)) => *b,
                Some(CallArg::Number(x)) => *x != 0.0,
                _ => false,
            };
            let text_at = |k: usize| match call.get(k) {
                Some(CallArg::Text(t)) => t.clone(),
                _ => String::new(),
            };
            let id_of = |k: usize| {
                let id = text_at(k);
                if id.is_empty() { String::new() } else { format!(".{id}") }
            };
            let file_at = |k: usize| -> Result<Map> {
                let t = text_at(k);
                if t.is_empty() {
                    return Err(fail(bad(format!("{name}: parameter {} is a file", k + 1))));
                }
                read_map(&resolve(&t, &at), &grid).map_err(fail)
            };
            let mut cp = params.clone();
            let mut cins = zargs.inputs.clone();
            let mut mask_name = None;
            let mut flex = Vec::new();
            match name.as_str() {
                "area_statistics_external" => {
                    cp.insert("area_statistics".into(), json!("areas"));
                    cins.insert("areas".into(), Value::Map(file_at(0)?));
                }
                "area_statistics_rank_range" => {
                    cp.insert("area_statistics".into(), json!([num_at(0)?, num_at(1)?]));
                    mask_name = Some(format!("area_statistics_rank_range.mask{}", id_of(2)));
                }
                "area_statistics_separate_areas" => {
                    cp.insert("area_statistics".into(), json!([num_at(0)?, num_at(1)?, num_at(2)?]));
                    mask_name = Some(format!("area_statistics_separate_areas.mask{}", id_of(3)));
                }
                "solution_comparison" => {
                    cp.insert("comparison".into(), json!([num_at(0)?, num_at(1)?, num_at(3)?, num_at(4)?]));
                    cins.insert("compare".into(), Value::Map(file_at(2)?));
                    mask_name = Some(format!("solution_comparison.mask{}", id_of(5)));
                }
                "histogram" => {
                    cp.insert("histogram".into(), json!([num_at(0)?, num_at(1)?, num_at(2)? as i64]));
                }
                "flexibility_map" => {
                    cp.insert("flexibility".into(), json!(num_at(0)?));
                    let id = id_of(4);
                    for (key, suffix, on) in
                        [("up", ".up", flag_at(2)), ("down", ".down", flag_at(3)), ("both", "", flag_at(1))]
                    {
                        if on {
                            flex.push((key, format!("flexibilitymap{id}{suffix}")));
                        }
                    }
                }
                other => return Err(fail(bad(format!("{}: unknown post-processing call {other:?}", pp.display())))),
            }
            let cargs = Args { node, inputs: cins, params: &cp };
            let mut o = Outputs::new();
            Zonation.post(&cargs, &ranked, &mut o)?;
            for key in POST_TABLES {
                if let Some(Value::Table(t)) = o.remove(key) {
                    let all = tables.entry(key).or_insert_with(|| {
                        Table::new(std::iter::once("call".to_string()).chain(t.columns.iter().cloned()).collect())
                    });
                    all.rows.extend(t.rows.into_iter().map(|r| std::iter::once(number).chain(r).collect()));
                }
            }
            if let Some(name) = mask_name
                && let Some(Value::Map(m)) = o.get("area_mask").or_else(|| o.get("comparison_map"))
            {
                maps.keys.push(Key::Name(name));
                maps.layers.push(m.data.clone());
            }
            if let Some(Value::Stack(s)) = o.get("flexibility") {
                for (key, name) in flex {
                    let at = s.keys.iter().position(|k| k.to_string() == key).unwrap();
                    maps.keys.push(Key::Name(name));
                    maps.layers.push(s.layers[at].clone());
                }
            }
        }
        for (k, t) in tables {
            out.insert(k.into(), Value::Table(t));
        }
        if !maps.layers.is_empty() {
            out.insert("post_maps".into(), Value::Stack(maps));
        }
        Ok(out)
    }
}

fn ranked_grid(out: &Outputs) -> Arc<terraflux_raster::Grid> {
    match &out["rank"] {
        Value::Map(m) => m.grid.clone(),
        _ => unreachable!("the rank is a map"),
    }
}
