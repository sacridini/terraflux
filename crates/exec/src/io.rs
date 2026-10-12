//! Reading a model's inputs and writing its outputs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_graph::{Input, Model};
use terraflux_ops::{Key, Map, Stack, Table, Value};
use terraflux_raster::{Band, DataType, Grid, Raster};

use crate::{Error, Result, Results};

fn resolve(base: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() { p.to_path_buf() } else { base.join(p) }
}

struct Files {
    base: PathBuf,
    cache: BTreeMap<PathBuf, (Arc<Raster>, Arc<Grid>)>,
}

impl Files {
    fn get(&mut self, path: &str) -> Result<(Arc<Raster>, Arc<Grid>)> {
        let p = resolve(&self.base, path);
        if let Some(r) = self.cache.get(&p) {
            return Ok(r.clone());
        }
        let r = Raster::read(&p).map_err(|e| Error::Model(format!("{}: {e}", p.display())))?;
        let g = Arc::new(r.grid.clone());
        let entry = (Arc::new(r), g);
        self.cache.insert(p, entry.clone());
        Ok(entry)
    }
}

fn band_index(r: &Raster, key: &str, path: &str) -> Result<usize> {
    if let Some(i) = r.bands.iter().position(|b| b.name.as_deref() == Some(key)) {
        return Ok(i);
    }
    match key.parse::<usize>() {
        Ok(i) if i < r.bands.len() => Ok(i),
        _ => Err(Error::Model(format!("{path} has no band {key:?} (its bands are {:?})", r.band_names()))),
    }
}

/// Read the inputs of a model; paths are relative to `base`. `overrides` replace the path of a file input
/// or, for a value input, give its value (JSON).
pub fn load_inputs(
    model: &Model,
    base: &Path,
    overrides: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Value>> {
    let mut files = Files { base: base.to_path_buf(), cache: BTreeMap::new() };
    let mut out = BTreeMap::new();
    for name in overrides.keys() {
        if !model.inputs.contains_key(name) {
            return Err(Error::Model(format!("the model has no input {name:?}")));
        }
    }
    for (name, input) in &model.inputs {
        let o = overrides.get(name);
        let v = match input {
            Input::Map { path, band, nodata, .. } => {
                let path = o.map(String::as_str).unwrap_or(path);
                let (r, grid) = files.get(path)?;
                let i = match band {
                    Some(b) => band_index(&r, b, path)?,
                    None => 0,
                };
                Value::Map(Map::new(grid, r.bands[i].masked(nodata.or(r.nodata))))
            }
            Input::Stack { path, bands, nodata, .. } => {
                let path = o.map(String::as_str).unwrap_or(path);
                let (r, grid) = files.get(path)?;
                let idx: Vec<usize> = match bands {
                    Some(b) => b.iter().map(|k| band_index(&r, k, path)).collect::<Result<_>>()?,
                    None => (0..r.bands.len()).collect(),
                };
                let names = r.band_names();
                Value::Stack(Stack {
                    grid,
                    keys: idx.iter().map(|&i| Key::Name(names[i].clone())).collect(),
                    layers: idx.iter().map(|&i| Arc::new(r.bands[i].masked(nodata.or(r.nodata)))).collect(),
                })
            }
            Input::Value { value, .. } => match o {
                Some(text) => {
                    let v: serde_json::Value =
                        serde_json::from_str(text).map_err(|e| Error::Model(format!("the value of ${name}: {e}")))?;
                    Value::from_json(&v)
                }
                None => Value::from_json(value),
            },
        };
        out.insert(name.clone(), v);
    }
    Ok(out)
}

/// The type a map is written in: the given one, else the smallest that holds its values.
fn choose_type(values: &[&[f64]], dtype: Option<&str>) -> Result<(DataType, Option<f64>)> {
    let wanted = match dtype {
        Some(s) => Some(
            DataType::parse(s)
                .ok_or_else(|| Error::Model(format!("unknown cell type {s:?} (u8, i16, i32, f32, f64...)")))?,
        ),
        None => None,
    };
    Ok(DataType::fitting(values, wanted))
}

fn write_maps(grid: &Grid, layers: Vec<(Option<String>, &[f64])>, path: &Path, dtype: Option<&str>) -> Result<PathBuf> {
    let refs: Vec<&[f64]> = layers.iter().map(|(_, d)| *d).collect();
    let (t, nodata) = choose_type(&refs, dtype)?;
    let r = Raster {
        grid: grid.clone(),
        bands: layers.into_iter().map(|(name, d)| Band { name, data: d.to_vec() }).collect(),
        dtype: t,
        nodata,
    };
    let path = path.with_extension("tif");
    terraflux_ops::prepare::write_with_crs(&r, &path).map_err(|e| Error::Model(e.to_string()))?;
    Ok(path)
}

fn write_text(path: &Path, ext: &str, text: &str) -> Result<PathBuf> {
    let path = path.with_extension(ext);
    std::fs::write(&path, text)?;
    Ok(path)
}

fn json<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string_pretty(v).map_err(|e| Error::Model(e.to_string()))
}

/// Write a value to `path` (its extension set from the kind of value): maps and series of maps as GeoTIFF,
/// tables as CSV, vectors as GeoPackage (or the vector format of the path's extension), fitted models and numbers
/// as JSON.
pub fn write_value(value: &Value, path: &Path, dtype: Option<&str>) -> Result<PathBuf> {
    match value {
        Value::Map(m) => write_maps(&m.grid, vec![(None, &m.data)], path, dtype),
        Value::Stack(s) => write_maps(
            &s.grid,
            s.keys.iter().zip(&s.layers).map(|(k, l)| (Some(k.to_string()), l.as_slice())).collect(),
            path,
            dtype,
        ),
        Value::Series(items) if items.iter().all(|v| matches!(v, Value::Map(_))) && !items.is_empty() => {
            let Value::Map(first) = &items[0] else { unreachable!() };
            let layers = items
                .iter()
                .enumerate()
                .map(|(i, v)| match v {
                    Value::Map(m) => (Some(format!("{}", i + 1)), m.data.as_slice()),
                    _ => unreachable!(),
                })
                .collect();
            write_maps(&first.grid, layers, path, dtype)
        }
        Value::Series(items) if items.iter().all(|v| matches!(v, Value::Table(_))) => {
            let parts: Vec<Table> =
                items.iter().map(|v| if let Value::Table(t) = v { t.clone() } else { unreachable!() }).collect();
            write_text(path, "csv", &Table::concat(&parts).to_csv())
        }
        Value::Table(t) => write_text(path, "csv", &t.to_csv()),
        Value::Markov(m) => write_text(path, "json", &json(&**m)?),
        Value::Demand(d) => write_text(path, "json", &json(&**d)?),
        Value::Glm(g) => write_text(path, "json", &json(&**g)?),
        Value::Woe(w) => write_text(path, "json", &json(&**w)?),
        Value::Patches(p) => write_text(path, "json", &json(&**p)?),
        Value::Number(x) => write_text(path, "json", &json(x)?),
        Value::Integer(x) => write_text(path, "json", &json(x)?),
        Value::Bool(x) => write_text(path, "json", &json(x)?),
        Value::Text(x) => write_text(path, "json", &json(x)?),
        Value::Json(x) => write_text(path, "json", &json(x)?),
        Value::Series(_) => Err(Error::Model("a series of mixed values cannot be written".into())),
        Value::Rng(_) => Err(Error::Model("a random generator cannot be written".into())),
        Value::Agents(a) => write_text(path, "csv", &a.to_table().to_csv()),
        Value::Vector(v) => {
            // GeoPackage, unless the path names another vector format
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let vector = ["gpkg", "shp", "geojson", "json", "fgb", "kml", "gml"].contains(&ext.as_str());
            let path = if vector { path.to_path_buf() } else { path.with_extension("gpkg") };
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir)?;
            }
            let layer = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "features".into());
            terraflux_gdal::write_vector(&path, &layer, v).map_err(Error::Model)?;
            Ok(path)
        }
        Value::Raster(r) => {
            let path = path.with_extension("tif");
            let wanted = match dtype {
                Some(s) => Some(DataType::parse(s).ok_or_else(|| Error::Model(format!("unknown cell type {s:?}")))?),
                None => None,
            };
            terraflux_ops::files::copy(r, &path, wanted).map_err(|e| Error::Model(e.to_string()))?;
            Ok(path)
        }
    }
}

/// Write the outputs of a model into a folder; returns the files written.
pub fn write_outputs(model: &Model, results: &Results, dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut files = Vec::new();
    for (name, out) in &model.outputs {
        let v = results
            .get(&out.from)
            .ok_or_else(|| Error::Model(format!("the output {name:?}: {} has no value", out.from)))?;
        let path = dir.join(out.path.as_deref().unwrap_or(name));
        files.push(write_value(v, &path, out.dtype.as_deref())?);
    }
    Ok(files)
}
