//! The values that flow along the graph.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use terraflux_graph::Type;
use terraflux_raster::Grid;

use crate::{Error, Result};

/// A map: one value per cell of a grid, row by row (NaN missing).
#[derive(Debug, Clone)]
pub struct Map {
    pub grid: Arc<Grid>,
    pub data: Arc<Vec<f64>>,
}

impl Map {
    pub fn new(grid: Arc<Grid>, data: Vec<f64>) -> Self {
        Map { grid, data: Arc::new(data) }
    }

    pub fn shape(&self) -> (usize, usize) {
        (self.grid.height, self.grid.width)
    }

    /// The distinct values of the cells with one, increasing.
    pub fn classes(&self) -> Vec<f64> {
        terraflux_ls_sys::ffi::map_classes(&self.data)
    }
}

/// The key of a layer of a stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    /// A named variable (a driver).
    Name(String),
    /// A class (suitability of a class).
    Class(f64),
    /// A transition from one class to another (its probability).
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

/// Maps of the same grid by key.
#[derive(Debug, Clone)]
pub struct Stack {
    pub grid: Arc<Grid>,
    pub keys: Vec<Key>,
    pub layers: Vec<Arc<Vec<f64>>>,
}

impl Stack {
    pub fn get(&self, key: &Key) -> Option<&Arc<Vec<f64>>> {
        self.keys.iter().position(|k| k == key).map(|i| &self.layers[i])
    }

    pub fn by_name(&self, name: &str) -> Option<&Arc<Vec<f64>>> {
        self.get(&Key::Name(name.to_string()))
    }

    /// The names of the layers keyed by name.
    pub fn names(&self) -> Vec<String> {
        self.keys
            .iter()
            .filter_map(|k| match k {
                Key::Name(n) => Some(n.clone()),
                _ => None,
            })
            .collect()
    }
}

/// A class as landschaft names it: an integer when it is one.
pub fn class_label(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

/// Rows of named numbers (NaN for none).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<f64>>,
}

impl Table {
    pub fn new(columns: Vec<String>) -> Self {
        Table { columns, rows: Vec::new() }
    }

    pub fn column(&self, name: &str) -> Option<Vec<f64>> {
        let j = self.columns.iter().position(|c| c == name)?;
        Some(self.rows.iter().map(|r| r[j]).collect())
    }

    /// Append rows of other tables, their columns merged (NaN where a row has none).
    pub fn concat(parts: &[Table]) -> Table {
        let mut columns: Vec<String> = Vec::new();
        for p in parts {
            for c in &p.columns {
                if !columns.contains(c) {
                    columns.push(c.clone());
                }
            }
        }
        let mut out = Table::new(columns.clone());
        for p in parts {
            for r in &p.rows {
                out.rows.push(
                    columns
                        .iter()
                        .map(|c| p.columns.iter().position(|x| x == c).map(|j| r[j]).unwrap_or(f64::NAN))
                        .collect(),
                );
            }
        }
        out
    }

    /// Comma-separated values, a header first.
    pub fn to_csv(&self) -> String {
        let mut s = self.columns.join(",");
        s.push('\n');
        for r in &self.rows {
            let cells: Vec<String> =
                r.iter().map(|v| if v.is_nan() { String::new() } else { format!("{v}") }).collect();
            s.push_str(&cells.join(","));
            s.push('\n');
        }
        s
    }
}

/// A Markov chain of land change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Markov {
    pub classes: Vec<f64>,
    /// Transition probabilities of the period, k x k row-major (rows: the class at the start).
    pub single: Vec<f64>,
    /// Per time step: the principal `steps`-th root of `single`.
    pub multi: Vec<f64>,
    pub steps: usize,
}

impl Markov {
    pub fn k(&self) -> usize {
        self.classes.len()
    }

    pub fn rate(&self, from: f64, to: f64) -> Option<f64> {
        let i = self.classes.iter().position(|&c| c == from)?;
        let j = self.classes.iter().position(|&c| c == to)?;
        Some(self.multi[i * self.k() + j])
    }

    /// The transitions with a positive rate per step, rows then columns (as Dinamica lists them).
    pub fn transitions(&self) -> Vec<(f64, f64)> {
        let k = self.k();
        let mut out = Vec::new();
        for i in 0..k {
            for j in 0..k {
                if i != j && self.multi[i * k + j] > 0.0 {
                    out.push((self.classes[i], self.classes[j]));
                }
            }
        }
        out
    }
}

/// Cells of each class at each step (row 0: the start).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Demand {
    pub classes: Vec<f64>,
    pub times: Vec<f64>,
    pub table: Vec<Vec<f64>>,
}

impl Demand {
    pub fn steps(&self) -> usize {
        self.table.len().saturating_sub(1)
    }

    /// The demand of each of `classes` at a step (0 for a class without one).
    pub fn at(&self, step: usize, classes: &[f64]) -> Result<Vec<f64>> {
        let row = self.table.get(step).ok_or_else(|| Error::msg(format!("the demand has no step {step}")))?;
        Ok(classes.iter().map(|c| self.classes.iter().position(|x| x == c).map(|j| row[j]).unwrap_or(0.0)).collect())
    }
}

/// The demand of one region: cells per class and step, or a Markov chain (the rates of the Expander and Patcher,
/// or projected from the region's cells by CARS).
#[derive(Debug, Clone)]
pub enum RegionDemand {
    Demand(Arc<Demand>),
    Markov(Arc<Markov>),
}

impl RegionDemand {
    /// The classes the demand names.
    pub fn classes(&self) -> &[f64] {
        match self {
            RegionDemand::Demand(d) => &d.classes,
            RegionDemand::Markov(m) => &m.classes,
        }
    }

    pub fn steps(&self) -> usize {
        match self {
            RegionDemand::Demand(d) => d.steps(),
            RegionDemand::Markov(m) => m.steps,
        }
    }
}

/// A demand per region (landschaft's `Regional`): the cells of the regions keep their own demand; cells of no
/// region keep their class.
#[derive(Debug, Clone)]
pub struct Regional {
    /// The region ids, sorted.
    pub ids: Vec<f64>,
    /// The region of each cell: its position in `ids`, -1 outside them.
    pub index: Vec<i32>,
    pub parts: Vec<RegionDemand>,
}

impl Regional {
    /// The classes of all the regions' demands, sorted.
    pub fn classes(&self) -> Vec<f64> {
        let mut all: Vec<f64> = self.parts.iter().flat_map(|p| p.classes().iter().copied()).collect();
        all.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all.dedup();
        all
    }

    /// The steps of the demands (all the same).
    pub fn steps(&self) -> usize {
        self.parts.first().map_or(0, RegionDemand::steps)
    }

    /// The area demand of region `k` (an error for a Markov chain).
    pub fn demand(&self, k: usize) -> Result<&Arc<Demand>> {
        match &self.parts[k] {
            RegionDemand::Demand(d) => Ok(d),
            RegionDemand::Markov(_) => Err(Error::msg(format!(
                "region {}: this allocation takes the cells of each class at each step, not a Markov chain",
                class_label(self.ids[k])
            ))),
        }
    }

    /// The Markov chain of region `k` (an error for an area demand).
    pub fn markov(&self, k: usize) -> Result<&Arc<Markov>> {
        match &self.parts[k] {
            RegionDemand::Markov(m) => Ok(m),
            RegionDemand::Demand(_) => Err(Error::msg(format!(
                "region {}: this allocation takes a Markov chain, not cells per class",
                class_label(self.ids[k])
            ))),
        }
    }
}

/// A logistic regression fitted for one class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlmClass {
    pub class: f64,
    pub drivers: Vec<String>,
    /// NaN for an aliased driver (R's NA; null in JSON).
    #[serde(deserialize_with = "nullable_vec")]
    pub coefficients: Vec<f64>,
    pub pivot: Vec<i32>,
    pub rank: i32,
    #[serde(default)]
    pub iterations: i32,
    #[serde(default = "yes")]
    pub converged: bool,
    #[serde(default = "nan", deserialize_with = "nullable")]
    pub deviance: f64,
    #[serde(default = "nan", deserialize_with = "nullable")]
    pub null_deviance: f64,
    #[serde(default)]
    pub cells: usize,
}

fn yes() -> bool {
    true
}

fn nan() -> f64 {
    f64::NAN
}

/// A number, null for NaN (JSON has no NaN).
fn nullable<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
}

fn nullable_vec<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<f64>, D::Error> {
    Ok(Vec::<Option<f64>>::deserialize(d)?.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect())
}

/// Logistic regressions of the presence of each class (lulcc's glmModels).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlmModel {
    pub fits: Vec<GlmClass>,
}

/// The settings of Dinamica's Expander and Patcher for one transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchTransition {
    pub from: f64,
    pub to: f64,
    /// Share of the changes made by the Expander.
    pub expansion: f64,
    /// Mean, variance (cells) and isometry of the patches of the Expander.
    pub expander: [f64; 3],
    /// ... and of the Patcher.
    pub patcher: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchParams {
    pub transitions: Vec<PatchTransition>,
}

impl PatchParams {
    pub fn get(&self, from: f64, to: f64) -> Option<&PatchTransition> {
        self.transitions.iter().find(|t| t.from == from && t.to == to)
    }
}

/// A random number generator shared along a loop (R's, as lulcc's).
#[derive(Clone)]
pub struct Rng(pub Arc<Mutex<terraflux_ls_sys::RRng>>);

impl std::fmt::Debug for Rng {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rng")
    }
}

/// numpy's legacy generator shared along a loop (MOLUSCE's).
#[derive(Clone)]
pub struct NumpyRng(pub Arc<Mutex<terraflux_ls_sys::NumpyRng>>);

impl std::fmt::Debug for NumpyRng {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NumpyRng")
    }
}

/// A value of the graph.
#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Integer(i64),
    Bool(bool),
    Text(String),
    /// Any structured value of the model file.
    Json(serde_json::Value),
    Map(Map),
    Stack(Stack),
    Series(Vec<Value>),
    Table(Table),
    Markov(Arc<Markov>),
    Demand(Arc<Demand>),
    Regional(Arc<Regional>),
    Glm(Arc<GlmModel>),
    Woe(Arc<crate::woe::WoeModel>),
    Patches(Arc<PatchParams>),
    Rng(Rng),
    /// numpy's generator (a random generator too).
    NumpyRng(NumpyRng),
    Agents(Arc<crate::agents::Agents>),
    /// Rasters on disk, read when needed.
    Raster(Arc<crate::files::Rasters>),
    /// Vector features.
    Vector(Arc<terraflux_raster::vector::Vector>),
    /// Biodiverse's groups x labels sample counts.
    BaseData(Arc<crate::bio::BaseData>),
    /// A rooted phylogenetic tree.
    Tree(Arc<crate::bio::tree::Tree>),
}

impl Value {
    pub fn ty(&self) -> Type {
        match self {
            Value::Number(_) => Type::Number,
            Value::Integer(_) => Type::Integer,
            Value::Bool(_) => Type::Bool,
            Value::Text(_) => Type::Text,
            Value::Json(_) => Type::Any,
            Value::Map(_) => Type::Map,
            Value::Stack(_) => Type::Stack,
            Value::Series(_) => Type::Series,
            Value::Table(_) => Type::Table,
            Value::Markov(_) => Type::Markov,
            Value::Demand(_) => Type::Demand,
            Value::Regional(_) => Type::Regional,
            Value::Glm(_) => Type::Glm,
            Value::Woe(_) => Type::Woe,
            Value::Patches(_) => Type::Patches,
            Value::Rng(_) | Value::NumpyRng(_) => Type::Rng,
            Value::Agents(_) => Type::Agents,
            Value::Raster(_) => Type::Raster,
            Value::Vector(_) => Type::Vector,
            Value::BaseData(_) => Type::BaseData,
            Value::Tree(_) => Type::Tree,
        }
    }

    /// A literal of the model file.
    pub fn from_json(v: &serde_json::Value) -> Value {
        match v {
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) if n.is_i64() => Value::Integer(n.as_i64().unwrap()),
            serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(f64::NAN)),
            serde_json::Value::String(s) => Value::Text(s.clone()),
            other => Value::Json(other.clone()),
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(v) => Some(*v),
            Value::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }
}
