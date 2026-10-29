//! A node's result frozen: its outputs written next to the model (`<model>.frozen/<node>/`, a file each and
//! `frozen.json`, what each is), read back in a later session and used instead of running the node while it is
//! frozen ([`Cache::hold`](crate::Cache::hold)). The same values: maps of whole numbers as they fit, the others in
//! float64; tables as CSV (every digit); vectors as GeoPackage; numbers, texts and fitted models as JSON.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_ops::{Key, Map, Outputs, Stack, Table, Value};
use terraflux_raster::Raster;

use crate::{Error, Result, write_value};

/// What a frozen output is and where.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Part {
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    /// A stack's keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keys: Vec<Key>,
    /// A number, a text, a structured value: kept in the index (a number as its text, NaN too).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value: Option<serde_json::Value>,
}

/// The index of a node's frozen outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Index {
    /// When it was frozen (seconds since 1970, UTC).
    when: u64,
    outputs: BTreeMap<String, Part>,
}

/// The folder of a model's frozen results: `<model>.frozen` next to its file.
pub fn folder(model_file: &Path) -> PathBuf {
    let stem = model_file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "model".into());
    model_file.with_file_name(format!("{stem}.frozen"))
}

/// When a node's result was frozen (seconds since 1970), if it is.
pub fn when(folder: &Path, node: &str) -> Option<u64> {
    let text = std::fs::read_to_string(folder.join(node).join("frozen.json")).ok()?;
    serde_json::from_str::<Index>(&text).ok().map(|i| i.when)
}

/// Forget a node's frozen result (its folder removed).
pub fn thaw(folder: &Path, node: &str) -> Result<()> {
    let dir = folder.join(node);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    // (the model's folder of frozen results gone with its last)
    if std::fs::read_dir(folder).is_ok_and(|mut d| d.next().is_none()) {
        let _ = std::fs::remove_dir(folder);
    }
    Ok(())
}

/// The cell type that keeps a map's values: as they fit when whole numbers, else float64.
fn dtype(layers: &[&[f64]]) -> Option<&'static str> {
    let whole = layers.iter().flat_map(|l| l.iter()).filter(|x| !x.is_nan()).all(|x| x.fract() == 0.0 && x.abs() < 2e9);
    (!whole).then_some("f64")
}

fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Rng(_) | Value::NumpyRng(_) => "a random generator",
        Value::Agents(_) => "agents",
        Value::BaseData(_) => "Biodiverse's data",
        Value::Tree(_) => "a tree",
        Value::Regional(_) => "a demand by region",
        Value::Series(_) => "a series (only of maps)",
        _ => "a value",
    }
}

/// Write a node's outputs into `folder/node` (what was there replaced); refused, with the reason, when one of them
/// cannot be kept (a random generator, agents, a series not of maps...).
pub fn freeze(folder: &Path, node: &str, outputs: &Outputs) -> Result<()> {
    let dir = folder.join(node);
    let tmp = folder.join(format!(".{node}.new"));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(&tmp)?;
    let result = (|| {
        let mut parts = BTreeMap::new();
        for (port, v) in outputs {
            let file = |p: PathBuf| p.file_name().map(|f| f.to_string_lossy().into_owned());
            let at = tmp.join(port);
            let part =
                |kind: &str, f: Option<String>| Part { kind: kind.into(), file: f, keys: Vec::new(), value: None };
            let inline = |kind: &str, value: serde_json::Value| Part {
                kind: kind.into(),
                file: None,
                keys: Vec::new(),
                value: Some(value),
            };
            let p = match v {
                Value::Map(m) => part("map", file(write_value(v, &at, dtype(&[&m.data]))?)),
                Value::Stack(s) => {
                    let layers: Vec<&[f64]> = s.layers.iter().map(|l| l.as_slice()).collect();
                    let mut p = part("stack", file(write_value(v, &at, dtype(&layers))?));
                    p.keys = s.keys.clone();
                    p
                }
                Value::Series(items) if !items.is_empty() && items.iter().all(|x| matches!(x, Value::Map(_))) => {
                    let layers: Vec<&[f64]> = items
                        .iter()
                        .filter_map(|x| if let Value::Map(m) = x { Some(m.data.as_slice()) } else { None })
                        .collect();
                    part("maps", file(write_value(v, &at, dtype(&layers))?))
                }
                Value::Raster(_) => part("raster", file(write_value(v, &at, None)?)),
                Value::Table(_) => part("table", file(write_value(v, &at, None)?)),
                Value::Vector(_) => part("vector", file(write_value(v, &at, None)?)),
                Value::Number(x) => inline("number", serde_json::Value::String(format!("{x}"))),
                Value::Integer(x) => inline("integer", serde_json::json!(x)),
                Value::Bool(x) => inline("bool", serde_json::json!(x)),
                Value::Text(x) => inline("text", serde_json::json!(x)),
                Value::Json(x) => inline("json", x.clone()),
                Value::Markov(_) => part("markov", file(write_value(v, &at, None)?)),
                Value::Demand(_) => part("demand", file(write_value(v, &at, None)?)),
                Value::Glm(_) => part("glm", file(write_value(v, &at, None)?)),
                Value::Woe(_) => part("woe", file(write_value(v, &at, None)?)),
                Value::Patches(_) => part("patches", file(write_value(v, &at, None)?)),
                Value::SdmModel(_) => part("sdm_model", file(write_value(v, &at, None)?)),
                Value::MlModel(_) => part("ml_model", file(write_value(v, &at, None)?)),
                Value::StepDistribution(_) => part("step_distribution", file(write_value(v, &at, None)?)),
                other => {
                    return Err(Error::Model(format!("its output {port} ({}) cannot be kept on disk", kind_of(other))));
                }
            };
            parts.insert(port.clone(), p);
        }
        let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let index = Index { when, outputs: parts };
        let text = serde_json::to_string_pretty(&index).map_err(|e| Error::Model(e.to_string()))?;
        std::fs::write(tmp.join("frozen.json"), text + "\n")?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::rename(&tmp, &dir)?;
    Ok(())
}

fn table_of_csv(text: &str) -> Result<Table> {
    let mut lines = text.lines();
    let columns: Vec<String> = lines.next().unwrap_or_default().split(',').map(String::from).collect();
    let mut t = Table::new(columns);
    for line in lines {
        let row = line
            .split(',')
            .map(|c| if c.is_empty() { Ok(f64::NAN) } else { c.parse::<f64>() })
            .collect::<std::result::Result<Vec<f64>, _>>()
            .map_err(|e| Error::Model(format!("a table: {e}")))?;
        t.rows.push(row);
    }
    Ok(t)
}

fn model<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Arc<T>> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text).map(Arc::new).map_err(|e| Error::Model(format!("{}: {e}", path.display())))
}

/// A node's frozen outputs, read back, and when they were frozen.
pub fn read(folder: &Path, node: &str) -> Result<(Outputs, u64)> {
    let dir = folder.join(node);
    let text = std::fs::read_to_string(dir.join("frozen.json"))?;
    let index: Index = serde_json::from_str(&text).map_err(|e| Error::Model(format!("frozen.json: {e}")))?;
    let mut out = Outputs::new();
    for (port, p) in index.outputs {
        let path = || -> Result<PathBuf> {
            p.file.as_ref().map(|f| dir.join(f)).ok_or_else(|| Error::Model(format!("{port}: no file")))
        };
        let maps = || -> Result<(Arc<terraflux_raster::Grid>, Vec<Vec<f64>>)> {
            let r = Raster::read(path()?).map_err(|e| Error::Model(e.to_string()))?;
            Ok((Arc::new(r.grid.clone()), r.bands.iter().map(|b| b.masked(r.nodata)).collect()))
        };
        let v = match p.kind.as_str() {
            "map" => {
                let (grid, mut bands) = maps()?;
                Value::Map(Map::new(grid, bands.swap_remove(0)))
            }
            "stack" => {
                let (grid, bands) = maps()?;
                Value::Stack(Stack { grid, keys: p.keys.clone(), layers: bands.into_iter().map(Arc::new).collect() })
            }
            "maps" => {
                let (grid, bands) = maps()?;
                Value::Series(bands.into_iter().map(|b| Value::Map(Map::new(grid.clone(), b))).collect())
            }
            "raster" => {
                let r = terraflux_ops::files::Rasters::open(&path()?.to_string_lossy(), None, None)
                    .map_err(|e| Error::Model(e.to_string()))?;
                Value::Raster(Arc::new(r))
            }
            "table" => Value::Table(table_of_csv(&std::fs::read_to_string(path()?)?)?),
            "vector" => Value::Vector(Arc::new(terraflux_gdal::read_vector(path()?, None).map_err(Error::Model)?)),
            "number" => {
                let t = p.value.as_ref().and_then(|v| v.as_str()).unwrap_or("NaN");
                Value::Number(t.parse().map_err(|e| Error::Model(format!("{port}: {e}")))?)
            }
            "integer" => Value::Integer(p.value.as_ref().and_then(|v| v.as_i64()).unwrap_or_default()),
            "bool" => Value::Bool(p.value.as_ref().and_then(|v| v.as_bool()).unwrap_or_default()),
            "text" => Value::Text(p.value.as_ref().and_then(|v| v.as_str()).unwrap_or_default().to_string()),
            "json" => Value::Json(p.value.clone().unwrap_or_default()),
            "markov" => Value::Markov(model(&path()?)?),
            "demand" => Value::Demand(model(&path()?)?),
            "glm" => Value::Glm(model(&path()?)?),
            "woe" => Value::Woe(model(&path()?)?),
            "patches" => Value::Patches(model(&path()?)?),
            "sdm_model" => Value::SdmModel(model(&path()?)?),
            "ml_model" => Value::MlModel(model(&path()?)?),
            "step_distribution" => Value::StepDistribution(model(&path()?)?),
            k => return Err(Error::Model(format!("{port}: an output of an unknown kind {k:?}"))),
        };
        out.insert(port, v);
    }
    Ok((out, index.when))
}

/// A value [`write_value`] wrote (a run's output, a batch's), read back by its file's kind: rasters of one band a
/// map, of more a series of maps (a step a band); CSV a table; JSON a number, else the JSON; a text.
pub fn read_written(path: &Path) -> Result<Value> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    Ok(match ext.as_str() {
        "tif" | "tiff" => {
            let r = Raster::read(path).map_err(|e| Error::Model(e.to_string()))?;
            let grid = Arc::new(r.grid.clone());
            let mut bands: Vec<Vec<f64>> = r.bands.iter().map(|b| b.masked(r.nodata)).collect();
            if bands.len() == 1 {
                Value::Map(Map::new(grid, bands.swap_remove(0)))
            } else {
                Value::Series(bands.into_iter().map(|b| Value::Map(Map::new(grid.clone(), b))).collect())
            }
        }
        "csv" => Value::Table(table_of_csv(&std::fs::read_to_string(path)?)?),
        "json" => {
            let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)
                .map_err(|e| Error::Model(format!("{}: {e}", path.display())))?;
            j.as_f64().map(Value::Number).unwrap_or(Value::Json(j))
        }
        _ => Value::Text(std::fs::read_to_string(path)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node's outputs frozen and read back are the same values; thawed, gone.
    #[test]
    fn outputs_are_frozen_and_read_back() {
        let dir = std::env::temp_dir().join(format!("terraflux-frozen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let folder = folder(&dir.join("model.toml"));
        assert_eq!(folder, dir.join("model.frozen"));
        let mut g = terraflux_raster::Grid::new(3, 2);
        g.transform = [0.0, 10.0, 0.0, 20.0, 0.0, -10.0];
        let grid = Arc::new(g);
        let classes = Map::new(grid.clone(), vec![1.0, 2.0, f64::NAN, 3.0, 1.0, 2.0]);
        let fine = Map::new(grid.clone(), vec![0.1, 1.0 / 3.0, f64::NAN, 1e-300, -2.5, 7.0]);
        let mut table = Table::new(vec!["a".into(), "b".into()]);
        table.rows = vec![vec![1.0 / 7.0, f64::NAN], vec![-3.0, 1e20]];
        let outputs: Outputs = [
            ("classes".to_string(), Value::Map(classes.clone())),
            ("fine".into(), Value::Map(fine.clone())),
            (
                "stack".into(),
                Value::Stack(Stack {
                    grid: grid.clone(),
                    keys: vec![Key::Class(1.0), Key::Transition(1.0, 2.0)],
                    layers: vec![classes.data.clone(), fine.data.clone()],
                }),
            ),
            ("table".into(), Value::Table(table.clone())),
            ("x".into(), Value::Number(f64::NAN)),
            ("k".into(), Value::Integer(-4)),
            ("said".into(), Value::Text("it held".into())),
        ]
        .into_iter()
        .collect();
        freeze(&folder, "n", &outputs).unwrap();
        assert!(when(&folder, "n").is_some());
        let (back, _) = read(&folder, "n").unwrap();
        let same = |a: &[f64], b: &[f64]| {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()))
        };
        let Value::Map(m) = &back["classes"] else { panic!() };
        assert!(same(&m.data, &classes.data));
        assert_eq!(m.grid.transform, grid.transform);
        let Value::Map(m) = &back["fine"] else { panic!() };
        assert!(same(&m.data, &fine.data), "{:?}", m.data);
        let Value::Stack(s) = &back["stack"] else { panic!() };
        assert_eq!(s.keys, vec![Key::Class(1.0), Key::Transition(1.0, 2.0)]);
        assert!(same(&s.layers[1], &fine.data));
        let Value::Table(t) = &back["table"] else { panic!() };
        assert_eq!(t.columns, table.columns);
        assert!(same(&t.rows.concat(), &table.rows.concat()));
        assert!(matches!(back["x"], Value::Number(x) if x.is_nan()));
        assert!(matches!(back["k"], Value::Integer(-4)));
        assert!(matches!(&back["said"], Value::Text(t) if t == "it held"));
        // a series of numbers cannot be kept: refused, nothing left behind
        let mixed: Outputs = [("s".to_string(), Value::Series(vec![Value::Number(1.0)]))].into_iter().collect();
        assert!(freeze(&folder, "r", &mixed).is_err());
        assert!(!folder.join("r").exists() && !folder.join(".r.new").exists());
        thaw(&folder, "n").unwrap();
        assert!(!folder.exists());
        // the files a run writes, read back by their kind
        let written = |name: &str, v: &Value| {
            let p = dir.join(name);
            write_value(v, &p, None).unwrap();
            read_written(&p).unwrap()
        };
        let Value::Map(m) = written("one.tif", &Value::Map(classes.clone())) else { panic!("a map") };
        assert_eq!(m.data[0], 1.0);
        let steps = Value::Series(vec![Value::Map(classes.clone()), Value::Map(fine.clone())]);
        assert!(matches!(written("steps.tif", &steps), Value::Series(s) if s.len() == 2));
        assert!(matches!(written("t.csv", &Value::Table(table.clone())), Value::Table(t) if t.rows.len() == 2));
        assert!(matches!(written("x.json", &Value::Number(2.5)), Value::Number(x) if x == 2.5));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
