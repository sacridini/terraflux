//! Reproducing a run and testing a model: the manifest a run writes with its outputs (the version of Terraflux, the
//! model, the scenario and the values set, every input file's SHA-256 and every output's, the seeds), and the
//! comparison of a value with the file of what it should be (`terraflux test`).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use terraflux_graph::{Input, Model, Node};
use terraflux_ops::Value;

use crate::{Error, Result};

/// A file and its SHA-256.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hashed {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// What a run was: enough to run it again and to tell whether it gave the same.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub terraflux: String,
    /// When it ran (seconds since 1970, UTC).
    pub when: u64,
    pub platform: String,
    pub model: Hashed,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    /// The `--set` given (`name=value`, `node.setting=value`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set: Vec<String>,
    /// The inputs of files, hashed; the inputs of values, their values.
    pub files: BTreeMap<String, Hashed>,
    pub values: BTreeMap<String, serde_json::Value>,
    /// The settings named seed of the nodes (`node.seed`, `container/node.seed`).
    pub seeds: BTreeMap<String, serde_json::Value>,
    /// The outputs written, hashed (by the output's name).
    pub outputs: BTreeMap<String, Hashed>,
}

/// The SHA-256 of a file and its size (a folder: its files' in order, for a folder of rasters).
pub fn hash(path: &Path) -> Result<Hashed> {
    let mut h = Sha256::new();
    let mut bytes = 0u64;
    let mut files = vec![path.to_path_buf()];
    if path.is_dir() {
        files = std::fs::read_dir(path)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect();
        files.sort();
    }
    let mut buf = vec![0u8; 1 << 20];
    for f in files {
        let mut file = std::fs::File::open(&f)?;
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
            bytes += n as u64;
        }
    }
    let sha256 = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok(Hashed { path: path.to_string_lossy().into_owned(), sha256, bytes })
}

fn seeds(nodes: &[Node], prefix: &str, out: &mut BTreeMap<String, serde_json::Value>) {
    for n in nodes {
        let at = if prefix.is_empty() { n.id.clone() } else { format!("{prefix}/{}", n.id) };
        for (k, v) in &n.params {
            if k.contains("seed") {
                out.insert(format!("{at}.{k}"), v.clone());
            }
        }
        for (k, src) in &n.inputs {
            if k.contains("seed")
                && let terraflux_graph::Source::Literal(v) = src
            {
                out.insert(format!("{at}.{k}"), v.clone());
            }
        }
        if let Some(b) = &n.body {
            seeds(&b.nodes, &at, out);
        }
    }
}

/// The manifest of a run: the model file and the model as it ran (after its scenario and the values set), the
/// inputs' files found from `base` (or the paths `overrides` gives), and the outputs written.
pub fn manifest(
    model_file: &Path,
    model: &Model,
    base: &Path,
    overrides: &BTreeMap<String, String>,
    scenario: Option<&str>,
    set: &[String],
    outputs: &[(String, PathBuf)],
) -> Result<Manifest> {
    let mut files = BTreeMap::new();
    let mut values = BTreeMap::new();
    for (name, input) in &model.inputs {
        match input {
            Input::Map { path, .. } | Input::Stack { path, .. } => {
                let given = overrides.get(name).cloned().unwrap_or_else(|| path.clone());
                let p = if Path::new(&given).is_absolute() { PathBuf::from(&given) } else { base.join(&given) };
                files.insert(name.clone(), hash(&p)?);
            }
            Input::Value { value, .. } => {
                let v = match overrides.get(name) {
                    Some(t) => serde_json::from_str(t).unwrap_or_else(|_| serde_json::Value::String(t.clone())),
                    None => value.clone(),
                };
                values.insert(name.clone(), v);
            }
        }
    }
    let mut s = BTreeMap::new();
    seeds(&model.nodes, "", &mut s);
    let mut outs = BTreeMap::new();
    for (name, p) in outputs {
        outs.insert(name.clone(), hash(p)?);
    }
    Ok(Manifest {
        terraflux: env!("CARGO_PKG_VERSION").to_string(),
        when: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        model: hash(model_file)?,
        scenario: scenario.map(String::from),
        set: set.to_vec(),
        files,
        values,
        seeds: s,
        outputs: outs,
    })
}

/// A value read back from a file of what it should be: a raster (a map, or a stack of its bands), a table (CSV,
/// TSV), a JSON value, a text.
pub fn read_expected(path: &Path) -> Result<Value> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let fail = |e: String| Error::Model(format!("{}: {e}", path.display()));
    Ok(match ext.as_str() {
        "tif" | "tiff" => {
            let r = terraflux_ops::files::Rasters::open(&path.to_string_lossy(), None, None)
                .map_err(|e| fail(e.to_string()))?;
            if r.layers.len() > 1 {
                Value::Stack(r.stack().map_err(|e| fail(e.to_string()))?)
            } else {
                Value::Map(r.map().map_err(|e| fail(e.to_string()))?)
            }
        }
        "csv" | "tsv" => Value::Table(terraflux_ops::tables::read_table(path).map_err(|e| fail(e.to_string()))?.0),
        "json" => {
            let text = std::fs::read_to_string(path)?;
            Value::from_json(&serde_json::from_str(&text).map_err(|e| fail(e.to_string()))?)
        }
        _ => Value::Text(std::fs::read_to_string(path)?),
    })
}

/// The numbers of a value to compare, by layer: a map's cells, a stack's or a series of maps' layers, a table's
/// rows; none for others.
fn numbers(v: &Value) -> Option<Vec<(String, Vec<f64>)>> {
    Some(match v {
        Value::Map(m) => vec![(String::new(), m.data.to_vec())],
        Value::Stack(s) => s.keys.iter().zip(&s.layers).map(|(k, l)| (k.to_string(), l.to_vec())).collect(),
        Value::Series(items) => {
            let mut out = Vec::new();
            for (i, x) in items.iter().enumerate() {
                let Value::Map(m) = x else { return None };
                out.push(((i + 1).to_string(), m.data.to_vec()));
            }
            out
        }
        Value::Table(t) => t.rows.iter().enumerate().map(|(i, r)| ((i + 1).to_string(), r.clone())).collect(),
        Value::Number(x) => vec![(String::new(), vec![*x])],
        Value::Integer(x) => vec![(String::new(), vec![*x as f64])],
        _ => return None,
    })
}

/// Whether a value is what it should be, within a tolerance on its numbers: none when it is, else the first
/// difference.
pub fn differs(got: &Value, want: &Value, tolerance: f64) -> Option<String> {
    if let (Value::Table(a), Value::Table(b)) = (got, want)
        && a.columns != b.columns
    {
        return Some(format!("its columns are {:?}, not {:?}", a.columns, b.columns));
    }
    match (numbers(got), numbers(want)) {
        (Some(a), Some(b)) => {
            if a.len() != b.len() {
                return Some(format!("{} layers or rows, not {}", a.len(), b.len()));
            }
            for ((name, x), (_, y)) in a.iter().zip(&b) {
                if x.len() != y.len() {
                    return Some(format!("{name}: {} values, not {}", x.len(), y.len()));
                }
                let bad: Vec<usize> = (0..x.len())
                    .filter(|&i| !((x[i].is_nan() && y[i].is_nan()) || (x[i] - y[i]).abs() <= tolerance))
                    .collect();
                if let Some(&i) = bad.first() {
                    let place = if name.is_empty() { String::new() } else { format!("{name}: ") };
                    return Some(format!(
                        "{place}{} values differ, the first at {i}: {} instead of {}",
                        bad.len(),
                        x[i],
                        y[i]
                    ));
                }
            }
            None
        }
        _ => {
            let text = |v: &Value| match v {
                Value::Text(t) => Some(t.trim().to_string()),
                Value::Bool(b) => Some(b.to_string()),
                Value::Json(j) => Some(j.to_string()),
                _ => None,
            };
            match (text(got), text(want)) {
                (Some(a), Some(b)) if a == b => None,
                (Some(a), Some(b)) => Some(format!("{a:?} instead of {b:?}")),
                _ => Some(format!("a {:?} cannot be compared with a {:?}", got.ty(), want.ty())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terraflux_ops::{Map, Table};
    use terraflux_raster::Grid;

    #[test]
    fn values_are_compared_within_a_tolerance() {
        let m = |d: Vec<f64>| Value::Map(Map::new(Arc::new(Grid::new(d.len(), 1)), d));
        assert_eq!(differs(&m(vec![1.0, f64::NAN]), &m(vec![1.0 + 1e-12, f64::NAN]), 1e-9), None);
        assert!(
            differs(&m(vec![1.0, 2.0]), &m(vec![1.0, 2.5]), 1e-9).unwrap().contains("1 values differ, the first at 1")
        );
        let t = |c: &str| Value::Table(Table { columns: vec![c.into()], rows: vec![vec![1.0]] });
        assert!(differs(&t("a"), &t("b"), 0.0).unwrap().contains("columns"));
        assert_eq!(differs(&Value::Text("x\n".into()), &Value::Text("x".into()), 0.0), None);
        assert!(differs(&Value::Integer(3), &Value::Number(3.5), 0.1).is_some());
    }

    #[test]
    fn a_file_is_hashed() {
        let p = std::env::temp_dir().join(format!("terraflux-hash-{}.txt", std::process::id()));
        std::fs::write(&p, "abc").unwrap();
        let h = hash(&p).unwrap();
        assert_eq!(h.sha256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(h.bytes, 3);
        let _ = std::fs::remove_file(&p);
    }
}
