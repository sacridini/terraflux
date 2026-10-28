//! Terraflux's calculators for a web page: `raster.calc`, `vector.calc` and `vector.filter` built to WebAssembly
//! for the website's playgrounds. Their modules are terraflux-ops' own, as they are (`#[path]`): the map algebra
//! ([`algebra`]), its editor's help ([`algebra_tools`]: the parts of a script to colour, its first error, the names
//! to complete, a line computed as the console does) and the scripts over features ([`vector_algebra`]), over the
//! few types of terraflux-ops they use, made here. A page computes what the application computes, with the same
//! messages.
//!
//! One function, [`tf_call`]: a request in JSON, its answer in JSON (see `website/static/js/calc.js`). Positions
//! in a script are counted in UTF-16 units, as JavaScript counts them. A missing value is `null`, an infinite one
//! `"inf"` or `"-inf"`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};
use terraflux_graph::OpSignature;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

#[allow(dead_code)]
#[path = "../../ops/src/algebra.rs"]
pub mod algebra;
#[allow(dead_code)]
#[path = "../../ops/src/algebra_tools.rs"]
pub mod algebra_tools;
#[allow(dead_code)]
#[path = "../../ops/src/measures.rs"]
mod measures;
#[allow(dead_code)]
#[path = "../../ops/src/vector_algebra.rs"]
pub mod vector_algebra;

use algebra::{Out, Script, Stmt, parse_script};
use algebra_tools::{Input, Inputs, Part};

// ------------------------------------------------------------------------------------------- terraflux-ops' types

/// An error, told as terraflux-ops tells it.
#[derive(Debug, Clone)]
pub struct Error(pub String);

impl Error {
    pub fn msg(m: impl Into<String>) -> Self {
        Error(m.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The key of a layer of a stack.
#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Name(String),
    Class(f64),
    Transition(f64, f64),
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Name(n) => write!(f, "{n}"),
            Key::Class(c) => write!(f, "{}", class_label(*c)),
            Key::Transition(a, b) => write!(f, "{}_to_{}", class_label(*a), class_label(*b)),
        }
    }
}

pub fn class_label(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

#[derive(Debug, Clone)]
pub struct Map {
    pub grid: Arc<Grid>,
    pub data: Arc<Vec<f64>>,
}

impl Map {
    pub fn new(grid: Arc<Grid>, data: Vec<f64>) -> Self {
        Map { grid, data: Arc::new(data) }
    }
}

#[derive(Debug, Clone)]
pub struct Stack {
    pub grid: Arc<Grid>,
    pub keys: Vec<Key>,
    pub layers: Vec<Arc<Vec<f64>>>,
}

/// Rasters on disk: none on a web page.
pub mod files {
    use std::sync::Arc;

    use terraflux_raster::Grid;
    use terraflux_raster::tiff::Window;

    use crate::{Error, Result};

    #[derive(Debug)]
    pub struct Layer {
        pub name: String,
    }

    #[derive(Debug)]
    pub struct Rasters {
        pub grid: Arc<Grid>,
        pub layers: Vec<Layer>,
        pub chosen: usize,
        pub picked: bool,
        pub mixed: bool,
    }

    impl Rasters {
        pub fn read(&self, _layer: usize) -> Result<Vec<f64>> {
            Err(Error::msg("no rasters on disk here"))
        }

        pub fn sample(&self, _layer: usize, _window: Window, _step: usize) -> Result<Vec<f32>> {
            Err(Error::msg("no rasters on disk here"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Table;

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Integer(i64),
    Text(String),
    Map(Map),
    Stack(Stack),
    Table(Table),
    Vector(Arc<Vector>),
    Raster(Arc<files::Rasters>),
}

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(v) => Some(*v),
            Value::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }
}

pub struct Args<'a> {
    pub node: &'a str,
    pub inputs: BTreeMap<String, Value>,
    pub params: &'a serde_json::Map<String, Json>,
}

impl Args<'_> {
    pub fn opt_param<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        match self.params.get(name) {
            None | Some(Json::Null) => Ok(None),
            Some(v) => serde_json::from_value(v.clone())
                .map(Some)
                .map_err(|e| Error::msg(format!("node {:?}: the setting {name:?}: {e}", self.node))),
        }
    }
}

pub struct Ctx;

pub type Outputs = BTreeMap<String, Value>;

pub trait Operator: Send + Sync {
    fn signature(&self) -> OpSignature;
    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs>;
}

pub struct Registry;

impl Registry {
    pub fn add(&mut self, _op: Arc<dyn Operator>) {}
}

mod threads {
    /// One thread on a web page.
    pub fn ensure() {}
}

// ---------------------------------------------------------------------------------------------------- the page's

/// The node's name in the messages of the operators (left out of them).
const NODE: &str = "calc";

fn bad(m: impl Into<String>) -> Error {
    Error::msg(m)
}

/// A byte of a text as UTF-16 units, and back.
fn units(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

fn byte_of(text: &str, unit: usize) -> usize {
    let mut n = 0;
    for (i, c) in text.char_indices() {
        if n >= unit {
            return i;
        }
        n += c.len_utf16();
    }
    text.len()
}

fn text_of<'a>(r: &'a Json, key: &str) -> Result<&'a str> {
    r.get(key).and_then(Json::as_str).ok_or_else(|| bad(format!("the request has no {key}")))
}

fn cells_of(c: &Json) -> Vec<f64> {
    c.as_array().map_or_else(Vec::new, |a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
}

/// A value in JSON: a number, `null` (missing), or `"inf"` and `"-inf"` (`1 / 0`, `log(0)`).
fn number_json(x: f64) -> Json {
    if x.is_finite() {
        json!(x)
    } else if x.is_nan() {
        Json::Null
    } else {
        json!(if x > 0.0 { "inf" } else { "-inf" })
    }
}

fn cells_json(c: &[f64]) -> Json {
    Json::Array(c.iter().map(|x| number_json(*x)).collect())
}

/// A message of an operator without its node's name.
fn told(e: Error) -> String {
    let lead = format!("node {NODE:?}: ");
    e.0.strip_prefix(&lead).map_or(e.0.clone(), str::to_string)
}

/// The maps, stacks, numbers and texts of a request on a grid: `{width, height, inputs: {name: {map: [...]} |
/// {stack: {layers: [...], cells: [[...], ...]}} | {number: x} | {text: "..."}}}`.
fn grid_inputs(r: &Json) -> Result<BTreeMap<String, Value>> {
    let w = r.get("width").and_then(Json::as_u64).unwrap_or(0) as usize;
    let h = r.get("height").and_then(Json::as_u64).unwrap_or(0) as usize;
    let grid = Arc::new(Grid::new(w, h));
    let mut out = BTreeMap::new();
    for (name, v) in r.get("inputs").and_then(Json::as_object).into_iter().flatten() {
        let value = if let Some(c) = v.get("map") {
            Value::Map(Map::new(grid.clone(), cells_of(c)))
        } else if let Some(s) = v.get("stack") {
            let keys = s
                .get("layers")
                .and_then(Json::as_array)
                .map_or_else(Vec::new, |a| a.iter().map(|k| Key::Name(k.as_str().unwrap_or("").into())).collect());
            let layers = s
                .get("cells")
                .and_then(Json::as_array)
                .map_or_else(Vec::new, |a| a.iter().map(|c| Arc::new(cells_of(c))).collect());
            Value::Stack(Stack { grid: grid.clone(), keys, layers })
        } else if let Some(x) = v.get("number") {
            Value::Number(x.as_f64().unwrap_or(f64::NAN))
        } else if let Some(t) = v.get("text") {
            Value::Text(t.as_str().unwrap_or("").into())
        } else {
            return Err(bad(format!("the input {name} is neither a map, a stack, a number nor a text")));
        };
        out.insert(name.clone(), value);
    }
    Ok(out)
}

fn out_json(name: &str, o: &Out) -> Json {
    match o {
        Out::Number(x) => json!({ "name": name, "number": number_json(*x) }),
        Out::Map(c) => json!({ "name": name, "map": cells_json(c) }),
        Out::Stack(l) => json!({
            "name": name,
            "stack": l.iter().map(|(k, c)| json!({ "key": k.to_string(), "cells": cells_json(c) })).collect::<Vec<_>>(),
        }),
    }
}

fn value_json(name: &str, v: &Value) -> Json {
    match v {
        Value::Map(m) => json!({ "name": name, "map": cells_json(&m.data) }),
        Value::Stack(s) => json!({
            "name": name,
            "stack": s
                .keys
                .iter()
                .zip(&s.layers)
                .map(|(k, c)| json!({ "key": k.to_string(), "cells": cells_json(c) }))
                .collect::<Vec<_>>(),
        }),
        v => json!({ "name": name, "number": number_json(v.as_number().unwrap_or(f64::NAN)) }),
    }
}

fn part_name(p: Part) -> &'static str {
    match p {
        Part::Number => "number",
        Part::Name => "name",
        Part::Input => "input",
        Part::Layer => "layer",
        Part::Function => "function",
        Part::Method => "method",
        Part::Word => "word",
        Part::Operator => "operator",
        Part::Comment => "comment",
    }
}

fn parts_json(text: &str, inputs: &Inputs) -> Json {
    Json::Array(
        algebra_tools::parts(text, inputs)
            .into_iter()
            .map(|(r, p)| json!([units(text, r.start), units(text, r.end), part_name(p)]))
            .collect(),
    )
}

fn complete_json(text: &str, cursor: usize, inputs: &Inputs, features: bool) -> Json {
    let (r, items) = algebra_tools::complete(text, byte_of(text, cursor), inputs);
    // (no neighbourhoods among features)
    let items: Vec<Json> = items
        .into_iter()
        .filter(|c| !(features && c.part == Part::Function && c.label.starts_with("nb")))
        .map(|c| json!({ "label": c.label, "text": c.text, "detail": c.detail, "part": part_name(c.part) }))
        .collect();
    json!({ "start": units(text, r.start), "end": units(text, r.end), "items": items })
}

/// Every name a script gives a value, at its end: the script with each `name = ...` made an output (its last line,
/// a value alone, named `(its last line)`).
fn every_name(script: &Script) -> Script {
    let mut s = script.clone();
    let n = s.statements.len();
    for (i, (_, st)) in s.statements.iter_mut().enumerate() {
        match st {
            Stmt::Set { out, .. } => *out = true,
            Stmt::Value(e) if i + 1 == n => {
                *st = Stmt::Set {
                    name: vector_algebra::LAST_LINE.into(),
                    layer: None,
                    mask: None,
                    value: e.clone(),
                    out: true,
                }
            }
            _ => {}
        }
    }
    s
}

/// `raster.calc` on the inputs: its outputs in the script's order (a script without `out`: `map`), and what every
/// name holds (`names: true`).
fn raster_run(r: &Json) -> Result<Json> {
    let text = text_of(r, "text")?;
    let inputs = grid_inputs(r)?;
    let mut params = serde_json::Map::new();
    params.insert("expression".into(), json!(text));
    let args = Args { node: NODE, inputs: inputs.clone(), params: &params };
    let mut got = algebra::Calc.run(&mut Ctx, &args).map_err(|e| bad(told(e)))?;
    let script = parse_script(text).map_err(|e| bad(e.0))?;
    let order = match script.outputs() {
        o if o.is_empty() => vec!["map".to_string()],
        o => o,
    };
    let outputs: Vec<Json> =
        order.iter().filter_map(|n| got.remove(n).map(|v| value_json(n, &v))).collect();
    let mut answer = json!({ "outputs": outputs });
    if r.get("names").and_then(Json::as_bool).unwrap_or(false) {
        let mut scope = algebra::Scope {
            width: r.get("width").and_then(Json::as_u64).unwrap_or(0) as usize,
            height: r.get("height").and_then(Json::as_u64).unwrap_or(0) as usize,
            maps: BTreeMap::new(),
            numbers: BTreeMap::new(),
            stacks: BTreeMap::new(),
        };
        for (name, v) in &inputs {
            match v {
                Value::Map(m) => {
                    scope.maps.insert(name.as_str(), m.data.as_slice());
                }
                Value::Stack(s) => {
                    let layers = s.keys.iter().cloned().zip(s.layers.iter().map(|l| l.as_slice())).collect();
                    scope.stacks.insert(name.as_str(), layers);
                }
                v => {
                    scope.numbers.insert(name.as_str(), v.as_number().unwrap_or(f64::NAN));
                }
            }
        }
        if let Ok(names) = every_name(&script).run(&scope) {
            answer["names"] = Json::Array(names.iter().map(|(n, o)| out_json(n, o)).collect());
        }
    }
    Ok(answer)
}

/// The inputs of a grid as the editor knows them.
fn grid_described(inputs: &BTreeMap<String, Value>) -> Inputs {
    inputs.iter().map(|(n, v)| (n.clone(), algebra_tools::describe(v))).collect()
}

// ------------------------------------------------------------------------------------------------------ features

fn ring(c: &Json) -> Vec<Xy> {
    c.as_array().map_or_else(Vec::new, |a| {
        a.iter()
            .map(|p| [p[0].as_f64().unwrap_or(f64::NAN), p[1].as_f64().unwrap_or(f64::NAN)])
            .collect()
    })
}

fn rings(c: &Json) -> Vec<Vec<Xy>> {
    c.as_array().map_or_else(Vec::new, |a| a.iter().map(ring).collect())
}

/// A GeoJSON geometry.
fn geometry(g: &Json) -> Geometry {
    let c = &g["coordinates"];
    match g["type"].as_str().unwrap_or("") {
        "Point" => Geometry::Point([c[0].as_f64().unwrap_or(f64::NAN), c[1].as_f64().unwrap_or(f64::NAN)]),
        "MultiPoint" => Geometry::MultiPoint(ring(c)),
        "LineString" => Geometry::LineString(ring(c)),
        "MultiLineString" => Geometry::MultiLineString(rings(c)),
        "Polygon" => Geometry::Polygon(rings(c)),
        "MultiPolygon" => {
            Geometry::MultiPolygon(c.as_array().map_or_else(Vec::new, |a| a.iter().map(rings).collect()))
        }
        _ => Geometry::Empty,
    }
}

/// The features of a request: `{features: {geometries: [GeoJSON geometries], attributes: [{name, numbers: [...]} |
/// {name, texts: [...]}]}}`; and its other inputs, `{inputs: {name: number | text}}`.
fn features(r: &Json) -> Result<(Vector, BTreeMap<String, Value>)> {
    let f = r.get("features").ok_or_else(|| bad("the request has no features"))?;
    let geometries = f.get("geometries").and_then(Json::as_array).map_or_else(Vec::new, |a| a.iter().map(geometry).collect());
    let mut attributes = Vec::new();
    for a in f.get("attributes").and_then(Json::as_array).into_iter().flatten() {
        let name = a["name"].as_str().unwrap_or("").to_string();
        let c = if let Some(t) = a.get("texts").and_then(Json::as_array) {
            Column::Texts(t.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
        } else {
            Column::Numbers(cells_of(&a["numbers"]))
        };
        attributes.push((name, c));
    }
    let mut inputs = BTreeMap::new();
    for (name, v) in r.get("inputs").and_then(Json::as_object).into_iter().flatten() {
        let v = match v {
            Json::String(t) => Value::Text(t.clone()),
            v => Value::Number(v.as_f64().unwrap_or(f64::NAN)),
        };
        inputs.insert(name.clone(), v);
    }
    Ok((Vector { geometries, attributes, crs: None }, inputs))
}

fn column_json(name: &str, c: &Column) -> Json {
    match c {
        Column::Numbers(x) => json!({ "name": name, "numbers": cells_json(x) }),
        Column::Texts(t) => json!({ "name": name, "texts": t }),
    }
}

/// The features' attributes and other inputs as the editor knows them: maps (a value per feature), the geometry's
/// measures as the layers of a stack, numbers.
fn features_described(v: &Vector, inputs: &BTreeMap<String, Value>) -> Inputs {
    let mut d: Inputs = v.attributes.iter().map(|(n, _)| (n.clone(), Input::Map)).collect();
    if v.column("feature").is_none() {
        d.insert("feature".into(), Input::Map);
    }
    let measures = measures::MEASURES.iter().map(|m| (m.to_string(), m.to_string())).collect();
    d.insert("geometry".into(), Input::Stack(Some(measures)));
    for (n, x) in inputs {
        d.insert(n.clone(), Input::Number(x.as_number()));
    }
    d
}

/// `vector.calc` (`op: "calc"`, with `column` for a script without `out`) or `vector.filter` (`op: "filter"`) on
/// the features: the attributes after it, or the features kept (their places); and what every name holds.
fn vector_run(r: &Json) -> Result<Json> {
    let text = text_of(r, "text")?;
    let (v, inputs) = features(r)?;
    let script = parse_script(text).map_err(|e| bad(format!("the expression: {}", e.0)))?;
    let mut answer = match r.get("op").and_then(Json::as_str).unwrap_or("calc") {
        "filter" => json!({ "kept": vector_algebra::kept(&v, &inputs, &script).map_err(|e| bad(e.0))? }),
        _ => {
            let column = r.get("column").and_then(Json::as_str).unwrap_or("");
            let after = vector_algebra::calculated(&v, &inputs, &script, column).map_err(|e| bad(e.0))?;
            let given: Vec<String> = match script.outputs() {
                o if o.is_empty() => vec![column.to_string()],
                o => o,
            };
            json!({
                "attributes": after.attributes.iter().map(|(n, c)| column_json(n, c)).collect::<Vec<_>>(),
                "given": given,
            })
        }
    };
    if r.get("names").and_then(Json::as_bool).unwrap_or(false)
        && let Ok(names) = vector_algebra::every_name(&v, &inputs, &script)
    {
        answer["names"] = Json::Array(names.iter().map(|(n, c)| column_json(n, c)).collect());
    }
    Ok(answer)
}

/// A line computed in the scope of a script over features, as the raster calculator's console computes one
/// (`algebra_tools::evaluate_line`): the script's names given their values first (the script left out when it has
/// an error), then the line's value (a statement: its name's), for every feature.
fn vector_line(r: &Json) -> Result<Json> {
    let (v, inputs) = features(r)?;
    let mut statements: Vec<(usize, Stmt)> = parse_script(text_of(r, "text")?)
        .map(|s| s.statements)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(l, st)| match st {
            Stmt::Set { name, layer, mask, value, .. } => Some((l, Stmt::Set { name, layer, mask, value, out: false })),
            Stmt::Out(_) | Stmt::Value(_) => None,
        })
        .collect();
    let asked = parse_script(text_of(r, "line")?)?;
    let n = asked.statements.len();
    for (k, (_, st)) in asked.statements.into_iter().enumerate() {
        let last = k + 1 == n;
        let at = usize::MAX;
        match st {
            Stmt::Value(e) if last => {
                statements.push((at, Stmt::Set { name: "_".into(), layer: None, mask: None, value: e, out: true }))
            }
            Stmt::Value(_) => {}
            Stmt::Set { name, layer, mask, value, .. } => {
                statements.push((at, Stmt::Set { name: name.clone(), layer, mask, value, out: false }));
                if last {
                    statements.push((at, Stmt::Out(name)));
                }
            }
            Stmt::Out(name) if last => statements.push((at, Stmt::Out(name))),
            Stmt::Out(_) => {}
        }
    }
    let script = Script { statements, lines: false };
    let (name, c) = vector_algebra::run(&v, &inputs, &script)?.pop().ok_or_else(|| bad("nothing to compute"))?;
    Ok(column_json(&name, &c))
}

// ------------------------------------------------------------------------------------------------------- the call

/// A request: `{fn: ...}` and its arguments.
///
/// - `raster.parts`, `raster.problem`, `raster.complete` (`cursor`), `raster.run` (`names`), `raster.line`
///   (`line`: computed in the scope of the script `text`, as the console does): `text` and the grid's inputs;
/// - `vector.parts`, `vector.complete` (`cursor`), `vector.run` (`op`, `column`, `names`), `vector.line`
///   (`line`): `text`, the features and their other inputs.
///
/// The answer: `{ok: ...}` or `{error: "..."}`.
pub fn call(request: &str) -> String {
    let answer = (|| -> Result<Json> {
        let r: Json = serde_json::from_str(request).map_err(|e| bad(format!("the request: {e}")))?;
        let f = text_of(&r, "fn")?;
        match f {
            "raster.parts" => Ok(parts_json(text_of(&r, "text")?, &grid_described(&grid_inputs(&r)?))),
            "raster.problem" => {
                let text = text_of(&r, "text")?;
                Ok(match algebra_tools::problem(text, &grid_described(&grid_inputs(&r)?)) {
                    Some(p) => json!({
                        "start": units(text, p.at.start),
                        "end": units(text, p.at.end),
                        "line": p.line,
                        "message": p.message,
                    }),
                    None => Json::Null,
                })
            }
            "raster.complete" => {
                let cursor = r.get("cursor").and_then(Json::as_u64).unwrap_or(0) as usize;
                Ok(complete_json(text_of(&r, "text")?, cursor, &grid_described(&grid_inputs(&r)?), false))
            }
            "raster.run" => raster_run(&r),
            "raster.line" => {
                let (w, h, o) = algebra_tools::evaluate_line(text_of(&r, "text")?, text_of(&r, "line")?, &grid_inputs(&r)?)?;
                Ok(json!({ "width": w, "height": h, "value": out_json("_", &o) }))
            }
            "vector.parts" => {
                let (v, inputs) = features(&r)?;
                Ok(parts_json(text_of(&r, "text")?, &features_described(&v, &inputs)))
            }
            "vector.complete" => {
                let (v, inputs) = features(&r)?;
                let cursor = r.get("cursor").and_then(Json::as_u64).unwrap_or(0) as usize;
                Ok(complete_json(text_of(&r, "text")?, cursor, &features_described(&v, &inputs), true))
            }
            "vector.run" => vector_run(&r),
            "vector.line" => vector_line(&r),
            f => Err(bad(format!("no function {f}"))),
        }
    })();
    match answer {
        Ok(x) => json!({ "ok": x }).to_string(),
        Err(e) => json!({ "error": e.0 }).to_string(),
    }
}

/// The answer of the last call, kept for the page to read.
static ANSWER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Room for a request of `len` bytes, the page writing it there before [`tf_call`].
#[unsafe(no_mangle)]
pub extern "C" fn tf_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Gives back the room of a request.
///
/// # Safety
/// `ptr` and `len` as [`tf_alloc`] gave them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_free(ptr: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len.max(1)) });
}

/// Answers the request of `len` bytes at `ptr` (UTF-8 JSON, freed): the length of the answer, read at
/// [`tf_answer`].
///
/// # Safety
/// `ptr` and `len` as [`tf_alloc`] gave them, the request written there.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_call(ptr: *mut u8, len: usize) -> usize {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let answer = call(&String::from_utf8_lossy(&bytes));
    let mut a = ANSWER.lock().unwrap_or_else(|e| e.into_inner());
    *a = answer.into_bytes();
    a.len()
}

/// Where the last answer is.
#[unsafe(no_mangle)]
pub extern "C" fn tf_answer() -> *const u8 {
    ANSWER.lock().unwrap_or_else(|e| e.into_inner()).as_ptr()
}
