//! Batch processing (landschaft's batch module, GuidosToolbox's File -> Batch Process): an operator run on many
//! rasters in parallel, one raster per worker, its outputs written next to each other with the input's name.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::{Band, DataType, Raster};

use crate::files::{Rasters, as_port};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Run));
}

const RASTER_SUFFIXES: [&str; 2] = ["tif", "tiff"];

/// The files of a directory (its GeoTIFFs, sorted), a glob pattern or a path, or a list of them, without
/// duplicates (landschaft's expand_sources).
pub fn expand_sources(items: &[String], recursive: bool) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for item in items {
        let p = Path::new(item);
        if p.is_dir() {
            let mut found = Vec::new();
            let mut stack = vec![p.to_path_buf()];
            while let Some(dir) = stack.pop() {
                for e in std::fs::read_dir(&dir).map_err(|e| Error::msg(format!("{}: {e}", dir.display())))? {
                    let path = e.map_err(|e| Error::msg(e.to_string()))?.path();
                    if path.is_dir() {
                        if recursive {
                            stack.push(path);
                        }
                    } else if path
                        .extension()
                        .is_some_and(|x| RASTER_SUFFIXES.contains(&x.to_string_lossy().to_lowercase().as_str()))
                    {
                        found.push(path);
                    }
                }
            }
            found.sort();
            paths.extend(found);
        } else if item.contains(['*', '?', '[']) {
            let mut found = glob(item)?;
            found.sort();
            paths.extend(found);
        } else {
            paths.push(p.to_path_buf());
        }
    }
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    Ok(paths)
}

/// The files matching a pattern of `*`, `?` and `[...]` in its last component.
fn glob(pattern: &str) -> Result<Vec<PathBuf>> {
    let p = Path::new(pattern);
    let dir = p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = p.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let mut re = String::from("^");
    for c in name.chars() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '[' | ']' => re.push(c),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push('$');
    let re = regex::Regex::new(&re).map_err(|e| Error::msg(format!("{pattern}: {e}")))?;
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).map_err(|e| Error::msg(format!("{}: {e}", dir.display())))? {
        let path = e.map_err(|e| Error::msg(e.to_string()))?.path();
        if path.is_file() && path.file_name().is_some_and(|n| re.is_match(&n.to_string_lossy())) {
            out.push(path);
        }
    }
    Ok(out)
}

/// The outcome of one input.
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub input: PathBuf,
    pub outputs: Vec<PathBuf>,
    pub seconds: f64,
    pub error: Option<String>,
    /// Done before (its JSON found, and no overwrite): not run again, its files those of the earlier run.
    pub skipped: bool,
}

fn json_number(x: f64) -> serde_json::Value {
    if x.is_finite() { serde_json::json!(x) } else { serde_json::Value::Null }
}

/// Write a value next to `base`: maps as GeoTIFF, tables as CSV; numbers, texts and flags into `values`.
fn put(
    value: &Value,
    base: &Path,
    suffix: &str,
    outputs: &mut Vec<PathBuf>,
    values: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let name = |ext: &str| {
        let stem = base.file_name().unwrap().to_string_lossy();
        base.with_file_name(if suffix.is_empty() { format!("{stem}{ext}") } else { format!("{stem}_{suffix}{ext}") })
    };
    let key = if suffix.is_empty() { "value".to_string() } else { suffix.to_string() };
    match value {
        Value::Map(m) => {
            let (dtype, nodata) = DataType::fitting(&[&m.data], None);
            let r = Raster {
                grid: (*m.grid).clone(),
                bands: vec![Band { name: None, data: m.data.to_vec() }],
                dtype,
                nodata,
            };
            let path = name(".tif");
            crate::prepare::write_with_crs(&r, &path)?;
            outputs.push(path);
        }
        Value::Stack(s) => {
            let refs: Vec<&[f64]> = s.layers.iter().map(|l| l.as_slice()).collect();
            let (dtype, nodata) = DataType::fitting(&refs, None);
            let bands = s
                .keys
                .iter()
                .zip(&s.layers)
                .map(|(k, l)| Band { name: Some(k.to_string()), data: l.to_vec() })
                .collect();
            let r = Raster { grid: (*s.grid).clone(), bands, dtype, nodata };
            let path = name(".tif");
            crate::prepare::write_with_crs(&r, &path)?;
            outputs.push(path);
        }
        Value::Table(t) => {
            let path = name(".csv");
            std::fs::write(&path, t.to_csv()).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
            outputs.push(path);
        }
        Value::Text(t) => {
            let path = name(".txt");
            std::fs::write(&path, t).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
            outputs.push(path);
        }
        Value::Number(x) => {
            values.insert(key, json_number(*x));
        }
        Value::Integer(x) => {
            values.insert(key, serde_json::json!(x));
        }
        Value::Bool(x) => {
            values.insert(key, serde_json::json!(x));
        }
        Value::Json(j) => {
            values.insert(key, j.clone());
        }
        _ => {}
    }
    Ok(())
}

/// GuidosToolbox's MSPA naming: <name>_<FGconn>_<EdgeWidth>_<Transition>_<Intext>.tif.
fn mspa_name(src: &Path, params: &serde_json::Map<String, serde_json::Value>) -> String {
    let num =
        |k: &str, d: i64| params.get(k).and_then(|v| v.as_i64().or_else(|| v.as_bool().map(i64::from))).unwrap_or(d);
    format!(
        "{}_{}_{}_{}_{}.tif",
        src.file_stem().unwrap().to_string_lossy(),
        num("connectivity", 8),
        num("edge_width", 1),
        num("transition", 1),
        num("internal", 1)
    )
}

/// Run an operator on every input (`port` takes the raster) and write its outputs to `out_dir`.
#[allow(clippy::too_many_arguments)]
pub fn batch(
    registry: &Registry,
    operator: &str,
    inputs: &[PathBuf],
    port: Option<&str>,
    out_dir: &Path,
    params: &serde_json::Map<String, serde_json::Value>,
    workers: usize,
    overwrite: bool,
) -> Result<Vec<BatchResult>> {
    let op = registry.get(operator).ok_or_else(|| Error::msg(format!("unknown operator {operator:?}")))?;
    let sig = op.signature();
    let port = match port {
        Some(p) => sig
            .inputs
            .iter()
            .find(|i| i.name == p)
            .ok_or_else(|| Error::msg(format!("{operator} has no input {p:?}")))?,
        None => sig
            .inputs
            .iter()
            .find(|i| matches!(i.ty, Type::Map | Type::Raster | Type::Stack))
            .ok_or_else(|| Error::msg(format!("{operator} takes no raster")))?,
    };
    let required: Vec<&str> =
        sig.inputs.iter().filter(|i| i.required && i.name != port.name).map(|i| i.name.as_str()).collect();
    if !required.is_empty() {
        return Err(Error::msg(format!("{operator} needs other inputs too: {}", required.join(", "))));
    }
    std::fs::create_dir_all(out_dir).map_err(|e| Error::msg(format!("{}: {e}", out_dir.display())))?;
    let tool = operator.replace('.', "_");
    let single = sig.outputs.len() == 1;
    let process = |src: &PathBuf| -> BatchResult {
        let start = Instant::now();
        let run = || -> Result<(Vec<PathBuf>, bool)> {
            let base = out_dir.join(format!("{}_{tool}", src.file_stem().unwrap_or_default().to_string_lossy()));
            let meta_path = base.with_file_name(format!("{}.json", base.file_name().unwrap().to_string_lossy()));
            if meta_path.exists() && !overwrite {
                let meta: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap_or_default())
                        .unwrap_or(serde_json::Value::Null);
                let mut files: Vec<PathBuf> = meta["outputs"]
                    .as_array()
                    .map_or(Vec::new(), |a| a.iter().filter_map(|p| p.as_str().map(PathBuf::from)).collect());
                files.push(meta_path);
                return Ok((files, true));
            }
            let raster = Rasters::open(&src.to_string_lossy(), None, None)?;
            let value = as_port(Value::Raster(Arc::new(raster)), port.ty)?;
            let args = Args { node: "batch", inputs: [(port.name.clone(), value)].into_iter().collect(), params };
            let out = op.run(&mut Ctx::default(), &args)?;
            let (mut outputs, mut values) = (Vec::new(), serde_json::Map::new());
            if operator == "landscape.mspa" {
                if let Some(Value::Map(m)) = out.get("map") {
                    let path = out_dir.join(mspa_name(src, params));
                    let (dtype, nodata) = DataType::fitting(&[&m.data], None);
                    let r = Raster {
                        grid: (*m.grid).clone(),
                        bands: vec![Band { name: None, data: m.data.to_vec() }],
                        dtype,
                        nodata,
                    };
                    crate::prepare::write_with_crs(&r, &path)?;
                    outputs.push(path);
                }
            } else {
                for spec in &sig.outputs {
                    if let Some(v) = out.get(&spec.name) {
                        put(v, &base, if single { "" } else { &spec.name }, &mut outputs, &mut values)?;
                    }
                }
            }
            let meta = serde_json::json!({
                "tool": operator,
                "input": src.to_string_lossy(),
                "params": params,
                "outputs": outputs.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
                "values": values,
                "seconds": (start.elapsed().as_secs_f64() * 1000.0).round() / 1000.0,
                "terraflux": env!("CARGO_PKG_VERSION"),
            });
            std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap())
                .map_err(|e| Error::msg(format!("{}: {e}", meta_path.display())))?;
            outputs.push(meta_path);
            Ok((outputs, false))
        };
        match run() {
            Ok((outputs, skipped)) => BatchResult {
                input: src.clone(),
                outputs,
                seconds: start.elapsed().as_secs_f64(),
                error: None,
                skipped,
            },
            Err(e) => BatchResult {
                input: src.clone(),
                outputs: Vec::new(),
                seconds: start.elapsed().as_secs_f64(),
                error: Some(e.to_string()),
                skipped: false,
            },
        }
    };
    let pool =
        rayon::ThreadPoolBuilder::new().num_threads(workers.max(1)).build().map_err(|e| Error::msg(e.to_string()))?;
    Ok(pool.install(|| inputs.par_iter().map(process).collect()))
}

struct Run;

impl Operator for Run {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "batch.run",
            "An operator run on many rasters in parallel (landschaft's batch, GuidosToolbox's Batch Process): each \
             worker reads one raster, runs the operator on it and writes what it gives to the output folder, named \
             after the input and the operator: maps <name>_<operator>[_<output>].tif, tables .csv, texts .txt, \
             and <name>_<operator>.json with the parameters, the files, the numbers and the time (an input whose \
             JSON exists is skipped unless overwrite). MSPA with GuidosToolbox's naming. A row per input: ok, \
             skipped (done before), seconds, files written; the errors in the report. A folder given to any node \
             that takes a map does the same, its maps seen and its settings in its pane (Make it a chain of nodes, \
             in the app).",
            Access::None,
        )
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per input: its number, ok (1/0), skipped (1/0: done before), seconds, files",
        ))
        .output(PortSpec::new("report", Type::Text, "each input and its outcome"))
        .param(PortSpec::new("operator", Type::Text, "the operator, e.g. landscape.mspa"))
        .param(PortSpec::new("inputs", Type::Any, "a folder, a glob pattern (tiles/*.tif) or a list of rasters"))
        .param(PortSpec::new("output_dir", Type::Text, "the folder for the results"))
        .param(PortSpec::optional("params", Type::Any, "the operator's parameters ({\"edge_width\": 2})"))
        .param(PortSpec::optional(
            "input",
            Type::Text,
            "the operator's input taking the raster (default: its first map)",
        ))
        .param(PortSpec::optional("workers", Type::Integer, "rasters at the same time (default: the threads)"))
        .param(PortSpec::optional("overwrite", Type::Bool, "recompute inputs already done"))
        .param(PortSpec::optional("recursive", Type::Bool, "the rasters of the folder's sub-folders too"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let operator: String = args.param("operator", String::new())?;
        let items: Vec<String> = match args.params.get("inputs") {
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            Some(serde_json::Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            _ => return Err(fail(Error::msg("inputs: a folder, a glob pattern or a list of rasters"))),
        };
        let inputs = expand_sources(&items, args.param("recursive", false)?).map_err(fail)?;
        let out_dir: String = args.param("output_dir", String::new())?;
        if out_dir.is_empty() {
            return Err(fail(Error::msg("give the output folder")));
        }
        let params = match args.params.get("params") {
            Some(serde_json::Value::Object(o)) => o.clone(),
            None | Some(serde_json::Value::Null) => serde_json::Map::new(),
            Some(_) => return Err(fail(Error::msg("params: an object of the operator's parameters"))),
        };
        let input = args.opt_param::<String>("input")?.filter(|s| !s.is_empty());
        let workers: i64 = args.param("workers", rayon::current_num_threads() as i64)?;
        let registry = Registry::standard();
        let results = batch(
            &registry,
            &operator,
            &inputs,
            input.as_deref(),
            Path::new(&out_dir),
            &params,
            workers.max(1) as usize,
            args.param("overwrite", false)?,
        )
        .map_err(fail)?;
        let mut table =
            Table::new(["input", "ok", "skipped", "seconds", "files"].iter().map(|s| s.to_string()).collect());
        let mut report = Vec::new();
        for (i, r) in results.iter().enumerate() {
            table.rows.push(vec![
                (i + 1) as f64,
                f64::from(u8::from(r.error.is_none())),
                f64::from(u8::from(r.skipped)),
                r.seconds,
                r.outputs.len() as f64,
            ]);
            report.push(match &r.error {
                None if r.skipped => format!(
                    "{}: skipped (done before; overwrite runs it again), {} files",
                    r.input.display(),
                    r.outputs.len()
                ),
                None => format!("{}: ok, {} files", r.input.display(), r.outputs.len()),
                Some(e) => format!("{}: {e}", r.input.display()),
            });
        }
        Ok([("table".to_string(), Value::Table(table)), ("report".to_string(), Value::Text(report.join("\n")))].into())
    }
}
