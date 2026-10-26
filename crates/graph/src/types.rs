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
    /// A demand per region: a map of region ids and each region's demand (cells per class or a Markov chain).
    Regional,
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
    /// Vector features: geometries (points, lines, polygons) with attributes, in a coordinate system.
    Vector,
    /// Biodiverse's groups x labels sample counts, with the groups' coordinates.
    BaseData,
    /// A rooted phylogenetic tree.
    Tree,
    /// Values between labels (Biodiverse's label matrix).
    LabelMatrix,
    /// A habitat connectivity graph: the nodes' attributes and the distances, probabilities or links between
    /// them (Conefor's and Makurhini's).
    HabitatGraph,
    /// A fitted species distribution model (MaxNet).
    SdmModel,
    /// A fitted machine learning model (random forests).
    MlModel,
}

impl Type {
    /// The type's name for people: a map in memory and rasters on disk are both a raster.
    pub fn name(self) -> &'static str {
        match self {
            Type::Any => "any value",
            Type::Number => "number",
            Type::Integer => "whole number",
            Type::Bool => "yes or no",
            Type::Text => "text",
            Type::Map | Type::Raster => "raster",
            Type::Stack => "stack",
            Type::Series => "series",
            Type::Table => "table",
            Type::Markov => "Markov chain",
            Type::Demand => "demand",
            Type::Regional => "regional demand",
            Type::Glm => "regressions",
            Type::Woe => "weights of evidence",
            Type::Patches => "patch settings",
            Type::Rules => "allocation rules",
            Type::Rng => "random generator",
            Type::Agents => "agents",
            Type::Vector => "vector",
            Type::BaseData => "base data",
            Type::Tree => "tree",
            Type::LabelMatrix => "label matrix",
            Type::HabitatGraph => "habitat graph",
            Type::SdmModel => "distribution model",
            Type::MlModel => "fitted model",
        }
    }

    /// Whether a value of type `self` may feed a port of type `port` (numbers and whole numbers feed each
    /// other: an operator checks at run time that a number it needs whole is one).
    pub fn fits(self, port: Type) -> bool {
        let numeric = |t: Type| matches!(t, Type::Integer | Type::Number);
        // rasters on disk feed maps and stacks (read when the node runs); maps, stacks and series of maps feed
        // what takes rasters; stacks and series feed what takes a map (the node runs once per layer)
        let raster = (self == Type::Raster && matches!(port, Type::Map | Type::Stack))
            || (port == Type::Raster && matches!(self, Type::Map | Type::Stack | Type::Series))
            || (port == Type::Map && matches!(self, Type::Stack | Type::Series));
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
    /// The values a setting takes, when they are a few (offered in a list; the first is the default when the
    /// setting is optional and its doc says none).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
}

impl PortSpec {
    pub fn new(name: &str, ty: Type, doc: &str) -> Self {
        PortSpec { name: name.to_string(), ty, required: true, doc: doc.to_string(), choices: Vec::new() }
    }

    pub fn optional(name: &str, ty: Type, doc: &str) -> Self {
        PortSpec { name: name.to_string(), ty, required: false, doc: doc.to_string(), choices: Vec::new() }
    }

    /// The values the setting takes (texts or numbers, as written in a model).
    pub fn choices<S: ToString>(mut self, values: impl IntoIterator<Item = S>) -> Self {
        self.choices = values.into_iter().map(|v| v.to_string()).collect();
        self
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
    /// Whether its maps are taken together (a stack made of them): given many layers where it takes a map, it
    /// does not run once per layer ([`OpSignature::per_layer`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub together: bool,
    /// The setting whose `out` lines name the node's outputs, in place of `outputs` when it has any (map
    /// algebra's script).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs_from: Option<String>,
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
            together: false,
            outputs_from: None,
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

    /// Its maps are taken together: many layers where it takes a map are refused, not run one at a time.
    pub fn together(mut self) -> Self {
        self.together = true;
        self
    }

    /// The node's outputs are named by the `out` lines of the setting `param` (see [`declared_outputs`]).
    pub fn outputs_from(mut self, param: &str) -> Self {
        self.outputs_from = Some(param.to_string());
        self
    }

    /// The outputs of a node of this operator: those its script declares, or the operator's.
    pub fn outputs_of(&self, n: &crate::Node) -> Vec<PortSpec> {
        let declared = match &self.outputs_from {
            Some(p) => n.params.get(p).and_then(|v| v.as_str()).map(declared_outputs).unwrap_or_default(),
            None => Vec::new(),
        };
        if declared.is_empty() {
            return self.outputs.clone();
        }
        declared.iter().map(|o| PortSpec::new(o, Type::Any, "an output of the script (out)")).collect()
    }

    pub fn input_spec(&self, name: &str) -> Option<&PortSpec> {
        self.inputs.iter().find(|p| p.name == name)
    }

    /// The type of an input port: its own, or that of the inputs of any name.
    pub fn input_type(&self, name: &str) -> Option<Type> {
        self.input_spec(name).map(|p| p.ty).or(self.extra)
    }

    /// Whether the input takes one map and, given many layers (a stack, a series of maps, the bands of rasters on
    /// disk), the node runs once per layer: a map port of an operator that is not a container and does not take
    /// its maps together.
    pub fn per_layer(&self, name: &str) -> bool {
        !self.container && !self.together && self.input_type(name) == Some(Type::Map)
    }

    pub fn output_spec(&self, name: &str) -> Option<&PortSpec> {
        self.outputs.iter().find(|p| p.name == name)
    }
}

/// The outputs a map algebra script declares: the names after `out` at the start of its statements (a line, or
/// `;`; `#` begins a comment), in order.
pub fn declared_outputs(script: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in script.lines() {
        let line = line.split('#').next().unwrap_or("");
        for statement in line.split(';') {
            let Some(rest) = statement.trim_start().strip_prefix("out") else { continue };
            if !rest.starts_with(char::is_whitespace) {
                continue;
            }
            let name: String = rest.trim_start().chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit()) && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}
