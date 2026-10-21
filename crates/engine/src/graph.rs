//! Running a model's graph on tiles. The tiled engine runs Dinamica EGO's classic model: a Markov chain
//! (`markov.fit` or `markov.matrix`), weights of evidence (`woe.fit` or `woe.weights`), the Expander and
//! Patcher's settings (`patches.observe` or `patches.settings`) and a loop of `woe.probability` and
//! `dinamica.step`. Its maps are files read by windows; each step's map is written as a tiled GeoTIFF.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_graph::{Input, Model, Node, Source as Src};
use terraflux_ops::woe::Settings as WoeSettings;
use terraflux_ops::{Args, Ctx, Registry, Value};
use terraflux_raster::DataType;
use terraflux_raster::tiff::{TiledWriter, Window};

use crate::dinamica::{Done, Settings};
use crate::pass::Runner;
use crate::run::{Fitted, simulate};
use crate::tiles::{FileSource, Source, Tiling};
use crate::{Error, Result, fit};

/// How the tiled engine runs a model.
#[derive(Debug, Clone)]
pub struct Options {
    /// The side of the engine's tiles (cells).
    pub tile: usize,
    pub settings: Settings,
    /// Where the layers of the steps are kept (maps larger than memory); in memory when `None`.
    pub disk: Option<PathBuf>,
    /// What runs the passes over the tiles (other machines); this machine's threads when `None`. Other
    /// machines need the model's files and `disk` on storage they share, at the same paths.
    pub runner: Option<Arc<dyn Runner>>,
}

impl Default for Options {
    fn default() -> Self {
        Options { tile: 1024, settings: Settings::default(), disk: None, runner: None }
    }
}

fn err(m: impl Into<String>) -> Error {
    Error::Msg(m.into())
}

fn find<'a>(nodes: &'a [Node], ops: &[&str]) -> Option<&'a Node> {
    nodes.iter().find(|n| ops.contains(&n.op.as_str()))
}

/// The input a node's port reads (`$name`).
fn input_of(n: &Node, port: &str) -> Result<String> {
    match n.inputs.get(port) {
        Some(Src::Ref(r)) if r.starts_with('$') => Ok(r[1..].to_string()),
        _ => Err(err(format!("the tiled engine needs {}.{port} from an input of the model", n.id))),
    }
}

/// A literal input of a node, or the value of the input it reads.
fn number_of(model: &Model, n: &Node, port: &str) -> Result<f64> {
    match n.inputs.get(port) {
        Some(Src::Literal(v)) => v.as_f64().ok_or_else(|| err(format!("{}.{port} is not a number", n.id))),
        Some(Src::Ref(r)) if r.starts_with('$') => match model.inputs.get(&r[1..]) {
            Some(Input::Value { value, .. }) => value.as_f64().ok_or_else(|| err(format!("{r} is not a number"))),
            _ => Err(err(format!("{r} is not a value"))),
        },
        _ => Err(err(format!("the tiled engine needs {}.{port} as a number", n.id))),
    }
}

/// The value of a node that needs no input (the fitted pieces given in the model).
fn literal(n: &Node) -> Result<Value> {
    let r = Registry::standard();
    let op = r.get(&n.op).ok_or_else(|| err(format!("unknown operator {}", n.op)))?;
    let args = Args { node: &n.id, inputs: BTreeMap::new(), params: &n.params };
    let out = op.run(&mut Ctx::default(), &args)?;
    out.into_values().next().ok_or_else(|| err(format!("{} gave nothing", n.id)))
}

struct Sources {
    maps: BTreeMap<String, FileSource>,
    stacks: BTreeMap<String, Vec<(String, FileSource)>>,
}

fn sources(model: &Model, base: &Path) -> Result<Sources> {
    let mut s = Sources { maps: BTreeMap::new(), stacks: BTreeMap::new() };
    let path = |p: &str| if Path::new(p).is_absolute() { PathBuf::from(p) } else { base.join(p) };
    for (name, input) in &model.inputs {
        match input {
            Input::Map { path: p, band, nodata, .. } => {
                s.maps.insert(name.clone(), FileSource::open(&path(p), band.as_deref(), *nodata)?);
            }
            Input::Stack { path: p, bands, nodata, .. } => {
                s.stacks.insert(name.clone(), FileSource::some_bands(&path(p), *nodata, bands.as_deref())?);
            }
            Input::Value { .. } => {}
        }
    }
    Ok(s)
}

/// What the run wrote and did.
pub struct Report {
    pub files: Vec<PathBuf>,
    pub steps: Vec<Vec<Done>>,
    pub fitted: Fitted,
}

/// Write a layer as a tiled GeoTIFF (classes: u8 or i32).
pub fn write_layer(land: &dyn Source, path: &Path) -> Result<()> {
    terraflux_ops::threads::ensure();
    let g = land.grid().clone();
    let tile = 256;
    // the type from the values of the first rows
    let sample = land.read(Window { x0: 0, y0: 0, width: g.width, height: g.height.min(64) })?;
    let small = sample.iter().all(|v| v.is_nan() || (*v >= 0.0 && *v <= 254.0 && v.fract() == 0.0));
    let (dtype, nodata) = if small { (DataType::U8, Some(255.0)) } else { (DataType::I32, Some(i32::MIN as f64)) };
    let mut w =
        TiledWriter::create(path, &g, vec![None], dtype, nodata, tile, Some(1)).map_err(|e| err(e.to_string()))?;
    for row in 0..g.height.div_ceil(tile) {
        for col in 0..g.width.div_ceil(tile) {
            let (x0, y0) = (col * tile, row * tile);
            let win =
                Window { x0: x0 as i64, y0: y0 as i64, width: tile.min(g.width - x0), height: tile.min(g.height - y0) };
            let v = land.read(win)?;
            w.write_tile(col, row, &[&v]).map_err(|e| err(e.to_string()))?;
        }
    }
    w.finish().map_err(|e| err(e.to_string()))
}

/// The model with the settings that read its inputs of values holding them (the tiled engine reads its settings
/// as they are written; a setting that reads another node or a loop's value is refused).
fn settled(model: &Model) -> Result<Model> {
    fn settle(nodes: &mut [Node], model: &Model) -> Result<()> {
        for n in nodes {
            for (k, v) in n.params.iter_mut() {
                let Some(r) = terraflux_graph::setting_ref(v) else { continue };
                *v = match r.strip_prefix('$').and_then(|name| model.inputs.get(name)) {
                    Some(Input::Value { value, .. }) => value.clone(),
                    _ => {
                        return Err(err(format!(
                            "the tiled engine takes {}.{k} = {r:?}: only the model's inputs of values",
                            n.id
                        )));
                    }
                };
            }
            if let Some(b) = &mut n.body {
                settle(&mut b.nodes, model)?;
            }
        }
        Ok(())
    }
    let mut m = model.clone();
    settle(&mut m.nodes, model)?;
    Ok(m)
}

/// Run a Dinamica model on tiles; its outputs written in `out`.
pub fn run(model: &Model, base: &Path, out: &Path, opt: &Options, mut progress: impl FnMut(&str)) -> Result<Report> {
    terraflux_ops::threads::ensure();
    let model = &settled(model)?;
    let nodes = &model.nodes;
    let unsupported = |what: &str| err(format!("the tiled engine runs Dinamica EGO's classic model: {what}"));
    let sim = find(nodes, &["control.repeat"]).ok_or_else(|| unsupported("no loop"))?;
    let body = sim.body.as_ref().ok_or_else(|| unsupported("no loop"))?;
    let step_node = find(&body.nodes, &["dinamica.step"]).ok_or_else(|| unsupported("no dinamica.step in the loop"))?;
    let prob_node =
        find(&body.nodes, &["woe.probability"]).ok_or_else(|| unsupported("no woe.probability in the loop"))?;
    if body.nodes.len() != 2 {
        return Err(unsupported("only woe.probability and dinamica.step in the loop (no dynamic variables yet)"));
    }
    let src = sources(model, base)?;
    let start_name = input_of(sim, "landscape")?;
    let start = src.maps.get(&start_name).ok_or_else(|| err(format!("${start_name} is not a map")))?;
    let g = start.grid().clone();
    let drivers_name = input_of(prob_node, "drivers")?;
    let drivers = src.stacks.get(&drivers_name).ok_or_else(|| err(format!("${drivers_name} is not a stack")))?;
    let drv: Vec<(String, &dyn Source)> = drivers.iter().map(|(n, d)| (n.clone(), d as &dyn Source)).collect();
    for (name, m) in
        src.maps.iter().map(|(k, v)| (k.as_str(), v as &dyn Source)).chain(drv.iter().map(|(k, v)| (k.as_str(), *v)))
    {
        let mg = m.grid();
        if mg.width != g.width || mg.height != g.height || mg.transform != g.transform {
            return Err(err(format!("{name} is not on the grid of ${start_name}: resample it first")));
        }
    }
    let mut tiling = Tiling::new(g.width, g.height, opt.tile);
    if let Some(r) = &opt.runner {
        if opt.disk.is_none() {
            return Err(err("tiles run elsewhere need the layers on disk (--disk)"));
        }
        tiling = tiling.with_runner(r.clone());
    }
    let map = |n: &Node, port: &str| -> Result<&FileSource> {
        let name = input_of(n, port)?;
        src.maps.get(&name).ok_or_else(|| err(format!("${name} is not a map")))
    };
    // the fitted pieces
    let mk = find(nodes, &["markov.fit", "markov.matrix"]).ok_or_else(|| unsupported("no Markov chain"))?;
    progress("fitting the Markov chain");
    let markov = if mk.op == "markov.fit" {
        fit::markov(map(mk, "t0")?, map(mk, "t1")?, &tiling, number_of(model, mk, "steps")? as usize)?
    } else {
        match literal(mk)? {
            Value::Markov(m) => (*m).clone(),
            _ => return Err(unsupported("markov.matrix")),
        }
    };
    let wn = find(nodes, &["woe.fit", "woe.weights"]).ok_or_else(|| unsupported("no weights of evidence"))?;
    progress("fitting the weights of evidence");
    let woe = if wn.op == "woe.fit" {
        let s: WoeSettings =
            serde_json::from_value(serde_json::Value::Object(wn.params.clone())).map_err(|e| err(e.to_string()))?;
        let tr = wn.inputs.contains_key("markov").then(|| markov.transitions());
        let wd =
            src.stacks.get(&input_of(wn, "drivers")?).ok_or_else(|| err("the weights' drivers are not a stack"))?;
        let wdrv: Vec<(String, &dyn Source)> = wd.iter().map(|(n, d)| (n.clone(), d as &dyn Source)).collect();
        fit::woe(map(wn, "t0")?, map(wn, "t1")?, &wdrv, &tiling, &s, tr)?
    } else {
        match literal(wn)? {
            Value::Woe(w) => (*w).clone(),
            _ => return Err(unsupported("woe.weights")),
        }
    };
    let pn = find(nodes, &["patches.observe", "patches.settings"]).ok_or_else(|| unsupported("no patch settings"))?;
    progress("measuring the patches");
    let patches = if pn.op == "patches.observe" {
        let expansion = pn.params.get("expansion").and_then(|v| v.as_f64());
        let isometry = pn.params.get("isometry").and_then(|v| v.as_f64()).unwrap_or(1.5);
        fit::patches(map(pn, "t0")?, map(pn, "t1")?, &tiling, &markov.transitions(), expansion, isometry)?
    } else {
        match literal(pn)? {
            Value::Patches(p) => (*p).clone(),
            _ => return Err(unsupported("patches.settings")),
        }
    };
    let fitted = Fitted { markov, woe, patches };
    // the loop
    let times = number_of(model, sim, "times")? as usize;
    let seed = number_of(model, step_node, "seed")? as i64 as u64;
    let mut settings = opt.settings.clone();
    if let Some(p) = step_node.params.get("prune_factor").and_then(|v| v.as_f64()) {
        settings.prune = p;
    }
    if let Some(w) = step_node.params.get("window")
        && w != &serde_json::json!([3, 3])
    {
        return Err(unsupported("the tiled Expander and Patcher take a 3 x 3 window"));
    }
    std::fs::create_dir_all(out)?;
    let maps_out = model.outputs.iter().find(|(_, o)| o.from == "simulation.maps").map(|(k, _)| out.join(k));
    if let Some(d) = &maps_out {
        std::fs::create_dir_all(d)?;
    }
    let mut files = Vec::new();
    let mut steps = Vec::new();
    progress(&format!("simulating {times} steps"));
    simulate(&tiling, start, &drv, &fitted, times, seed, &settings, opt.disk.clone(), |k, land, done| {
        progress(&format!("step {k} of {times}"));
        if let Some(d) = &maps_out {
            let f = d.join(format!("step_{k:03}.tif"));
            write_layer(land, &f)?;
            files.push(f);
        }
        steps.push(done.to_vec());
        Ok(())
    })?;
    // the tables
    for (name, o) in &model.outputs {
        if o.from == "simulation.info" {
            let trs = crate::dinamica::transitions(&fitted.markov, &fitted.patches)?;
            let mut csv = String::from(
                "step,from,to,requested,expanded,patched,expander_patches,patcher_patches,expander_rounds,patcher_rounds\n",
            );
            for (k, st) in steps.iter().enumerate() {
                for (t, d) in trs.iter().zip(st) {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{}\n",
                        k + 1,
                        t.from,
                        t.to,
                        d.requested,
                        d.expanded,
                        d.patched,
                        d.expander_patches,
                        d.patcher_patches,
                        d.expander_rounds,
                        d.patcher_rounds
                    ));
                }
            }
            let f = out.join(format!("{name}.csv"));
            std::fs::write(&f, csv)?;
            files.push(f);
        } else if o.from.ends_with(".table") {
            let (t, vars) = fitted.woe.table();
            let f = out.join(format!("{name}.csv"));
            std::fs::write(&f, format!("{}# variables: {}\n", t.to_csv(), vars.join(", ")))?;
            files.push(f);
        }
    }
    Ok(Report { files, steps, fitted })
}
