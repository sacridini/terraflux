//! The types of the values that flow along the graph and the signatures of the operators.

use serde::{Deserialize, Serialize};

/// The type of a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    /// Any value (the operator checks it).
    Any,
    Number,
    Integer,
    Bool,
    Text,
    /// A map: one value per cell of a grid (NaN missing).
    Map,
    /// Maps of the same grid by key: drivers by name, suitabilities by class, probabilities by transition.
    Stack,
    /// One value per step of a loop.
    Series,
    /// Rows of named numbers.
    Table,
    /// A Markov chain of land change (transition probabilities of a period and per step).
    Markov,
    /// Cells of each class at each step.
    Demand,
    /// Fitted logistic regressions (one per class).
    Glm,
    /// Fitted weights of evidence (one set per transition).
    Woe,
    /// The patch settings of Dinamica's Expander and Patcher, per transition.
    Patches,
    /// Decision rules of the allocation (conversions, elasticities, neighbourhoods).
    Rules,
    /// A random number generator carrying its state from step to step.
    Rng,
    /// A population of agents: attributes by name, a value per agent.
    Agents,
    /// Rasters on disk, read where and as finely as needed (a file's bands, or the files of a series): a map
    /// or a stack where one is needed.
    Raster,
}

impl Type {
    /// Whether a value of type `self` may feed a port of type `port` (numbers and whole numbers feed each
    /// other: an operator checks at run time that a number it needs whole is one).
    pub fn fits(self, port: Type) -> bool {
        let numeric = |t: Type| matches!(t, Type::Integer | Type::Number);
        // rasters on disk feed maps and stacks (read when the node runs); maps, stacks and series of maps feed
        // what takes rasters
        let raster = (self == Type::Raster && matches!(port, Type::Map | Type::Stack))
            || (port == Type::Raster && matches!(self, Type::Map | Type::Stack | Type::Series));
        self == port || self == Type::Any || port == Type::Any || (numeric(self) && numeric(port)) || raster
    }
}

/// How an operator reads its maps: what the engine needs to split it in tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    /// Each cell from the same cell of its inputs.
    Local,
    /// Each cell from the cells within a radius (the halo a tile needs from its neighbours).
    Halo(u32),
    /// Every cell together (reductions, fits, sorting the whole map).
    Global,
    /// No maps (tables, numbers, models).
    None,
}

/// A port of an operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub required: bool,
    pub doc: String,
}

impl PortSpec {
    pub fn new(name: &str, ty: Type, doc: &str) -> Self {
        PortSpec { name: name.to_string(), ty, required: true, doc: doc.to_string() }
    }

    pub fn optional(name: &str, ty: Type, doc: &str) -> Self {
        PortSpec { name: name.to_string(), ty, required: false, doc: doc.to_string() }
    }
}

/// What an operator takes and gives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpSignature {
    pub name: String,
    pub doc: String,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    /// The settings (`params`) it reads, with their documentation.
    pub params: Vec<PortSpec>,
    pub access: Access,
    /// Whether the node holds a body of nodes (and its outputs are the body's carried and collected values).
    pub container: bool,
    /// Inputs of any other name the operator takes, of this type (map algebra's variables).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Type>,
}

impl OpSignature {
    pub fn new(name: &str, doc: &str, access: Access) -> Self {
        OpSignature {
            name: name.to_string(),
            doc: doc.to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: Vec::new(),
            access,
            container: false,
            extra: None,
        }
    }

    pub fn input(mut self, p: PortSpec) -> Self {
        self.inputs.push(p);
        self
    }

    pub fn output(mut self, p: PortSpec) -> Self {
        self.outputs.push(p);
        self
    }

    pub fn param(mut self, p: PortSpec) -> Self {
        self.params.push(p);
        self
    }

    pub fn container(mut self) -> Self {
        self.container = true;
        self
    }

    /// The operator takes inputs of any other name, of type `ty`.
    pub fn extra(mut self, ty: Type) -> Self {
        self.extra = Some(ty);
        self
    }

    pub fn input_spec(&self, name: &str) -> Option<&PortSpec> {
        self.inputs.iter().find(|p| p.name == name)
    }

    pub fn output_spec(&self, name: &str) -> Option<&PortSpec> {
        self.outputs.iter().find(|p| p.name == name)
    }
}
