//! The operators of Terraflux's model graph.
//!
//! Each operator implements [`Operator`]: its [`OpSignature`] (the ports and
//! settings it takes and gives, which the graph is checked against) and
//! `run`, from the values of its inputs to the values of its outputs. The
//! [`Registry`] holds them by name.
//!
//! Data comes in and goes out through operators too (`raster.read`, `raster.write`), and landschaft's
//! landscape analyses (GuidosToolbox's `landscape.mspa`, `fragmentation.fad` and the others) run on the same maps.
//!
//! The land change operators reproduce landschaft's `ls.simulate` step by
//! step, calling the same C++ kernels (through `terraflux-ls-sys`), so that
//! a model gives the same maps here as in Python with the same seed.

pub mod accuracy;
pub mod actions;
pub mod actions_raster;
pub mod agents;
pub mod algebra;
pub mod batch;
pub mod bio;
pub mod carbon;
pub mod change;
pub mod change_analysis;
pub mod circuit;
pub mod circuit_ini;
pub mod clumondo;
pub mod conn;
pub mod cost;
pub mod crafty;
pub mod data;
pub mod demand;
pub mod differ;
pub mod dinamica;
pub mod dynamic;
pub mod essentials;
pub mod files;
pub mod forest_code;
pub mod fitted;
pub mod fragmentation;
pub mod futures;
pub mod futures_tools;
pub mod glm;
pub mod gtb_palettes;
pub mod gtn;
pub mod gurobi;
pub mod h3;
pub mod hydro;
pub mod landforms;
pub mod landscape;
pub mod legend;
pub mod lsm_catalog;
pub mod lulcc;
pub mod map_validation;
pub mod markov;
pub mod metrics;
pub mod milp;
pub mod ml;
pub mod molusce;
pub mod mspa_tiled;
pub mod nlm;
pub mod npmath;
pub mod planner;
pub mod plugin;
pub mod plus;
pub mod prepare;
pub mod presets;
pub mod prioritize;
pub mod prioritize_tools;
pub mod redd;
pub mod restoration;
pub mod restoration_plan;
pub mod roads;
pub mod sampling;
pub mod sdm;
pub mod services;
pub mod signatures;
pub mod surface;
pub mod swat;
pub mod swat_tables;
pub mod tables;
pub mod uncertainty;
pub mod validate;
pub mod value;
pub mod vector;
pub mod vector_tiled;
pub mod view;
pub mod woe;
mod ziggurat;
pub mod zonation;
pub mod zonation_project;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use terraflux_graph::{Access, Catalog, OpSignature, PortSpec, Type};

pub use terraflux_ls_sys::threads;
pub use value::{Demand, GlmModel, Key, Map, Markov, PatchParams, Stack, Table, Value};

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);

impl Error {
    pub fn msg(m: impl Into<String>) -> Self {
        Error(m.into())
    }
}

impl From<terraflux_ls_sys::Error> for Error {
    fn from(e: terraflux_ls_sys::Error) -> Self {
        Error(e.what().to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// How far an operator is, as it tells it: the share done (none when it cannot say) and what it is doing.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub fraction: Option<f64>,
    pub text: String,
}

/// What an operator is told while it runs: given how far it is (or nothing, only asked), whether to go on.
pub type Watch<'a> = &'a mut dyn FnMut(Option<&Progress>) -> bool;

/// What an operator may report while it runs, and how it learns that the run was stopped.
#[derive(Default)]
pub struct Ctx<'a> {
    /// Warnings (e.g. an allocation that could not meet its demand).
    pub warnings: Vec<String>,
    watch: Option<Watch<'a>>,
    stopped: bool,
}

impl<'a> Ctx<'a> {
    /// A context whose progress `watch` is told (the engine's, which shows it and says when the run is stopped).
    pub fn watched(watch: Watch<'a>) -> Self {
        Ctx { warnings: Vec::new(), watch: Some(watch), stopped: false }
    }

    pub fn warn(&mut self, m: impl Into<String>) {
        self.warnings.push(m.into());
    }

    /// Say how far it is (shown with the node); false when the run was stopped: the operator should return (an
    /// error is fine: the run reports it stopped).
    pub fn progress(&mut self, fraction: Option<f64>, text: impl Into<String>) -> bool {
        let p = Progress { fraction: fraction.map(|f| f.clamp(0.0, 1.0)), text: text.into() };
        self.tell(Some(&p))
    }

    /// Whether the run was stopped (cheap: a long loop may ask at every turn).
    pub fn stopped(&mut self) -> bool {
        !self.tell(None)
    }

    /// Whether the run was stopped while this operator ran.
    pub fn was_stopped(&self) -> bool {
        self.stopped
    }

    fn tell(&mut self, p: Option<&Progress>) -> bool {
        if !self.stopped
            && let Some(w) = self.watch.as_mut()
            && !w(p)
        {
            self.stopped = true;
        }
        !self.stopped
    }
}

/// The inputs and settings of one run of an operator.
pub struct Args<'a> {
    pub node: &'a str,
    pub inputs: BTreeMap<String, Value>,
    pub params: &'a serde_json::Map<String, serde_json::Value>,
}

impl Args<'_> {
    fn missing(&self, port: &str) -> Error {
        Error::msg(format!("node {:?}: no input {port:?}", self.node))
    }

    fn wrong(&self, port: &str, want: &str, got: &Value) -> Error {
        Error::msg(format!("node {:?}: the input {port:?} is a {:?}, not a {want}", self.node, got.ty()))
    }

    pub fn get(&self, port: &str) -> Option<&Value> {
        self.inputs.get(port)
    }

    pub fn map(&self, port: &str) -> Result<&Map> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Map(m) => Ok(m),
            v => Err(self.wrong(port, "map", v)),
        }
    }

    pub fn opt_map(&self, port: &str) -> Result<Option<&Map>> {
        match self.get(port) {
            None => Ok(None),
            Some(Value::Map(m)) => Ok(Some(m)),
            Some(v) => Err(self.wrong(port, "map", v)),
        }
    }

    pub fn stack(&self, port: &str) -> Result<&Stack> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Stack(s) => Ok(s),
            v => Err(self.wrong(port, "stack", v)),
        }
    }

    pub fn opt_stack(&self, port: &str) -> Result<Option<&Stack>> {
        match self.get(port) {
            None => Ok(None),
            Some(Value::Stack(s)) => Ok(Some(s)),
            Some(v) => Err(self.wrong(port, "stack", v)),
        }
    }

    pub fn number(&self, port: &str) -> Result<f64> {
        let v = self.get(port).ok_or_else(|| self.missing(port))?;
        v.as_number().ok_or_else(|| self.wrong(port, "number", v))
    }

    pub fn integer(&self, port: &str) -> Result<i64> {
        let x = self.number(port)?;
        if x.fract() != 0.0 {
            return Err(Error::msg(format!("node {:?}: {port:?} must be a whole number, not {x}", self.node)));
        }
        Ok(x as i64)
    }

    pub fn markov(&self, port: &str) -> Result<&Arc<Markov>> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Markov(m) => Ok(m),
            v => Err(self.wrong(port, "Markov chain", v)),
        }
    }

    pub fn demand(&self, port: &str) -> Result<&Arc<Demand>> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Demand(m) => Ok(m),
            v => Err(self.wrong(port, "demand", v)),
        }
    }

    pub fn regional(&self, port: &str) -> Result<&Arc<value::Regional>> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Regional(m) => Ok(m),
            v => Err(self.wrong(port, "regional demand", v)),
        }
    }

    pub fn glm(&self, port: &str) -> Result<&Arc<GlmModel>> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Glm(m) => Ok(m),
            v => Err(self.wrong(port, "logistic regression", v)),
        }
    }

    pub fn woe(&self, port: &str) -> Result<&Arc<woe::WoeModel>> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Woe(m) => Ok(m),
            v => Err(self.wrong(port, "weights of evidence", v)),
        }
    }

    pub fn patches(&self, port: &str) -> Result<&Arc<PatchParams>> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Patches(m) => Ok(m),
            v => Err(self.wrong(port, "patch settings", v)),
        }
    }

    pub fn rng(&self, port: &str) -> Result<&value::Rng> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::Rng(r) => Ok(r),
            v => Err(self.wrong(port, "random generator", v)),
        }
    }

    pub fn numpy_rng(&self, port: &str) -> Result<&value::NumpyRng> {
        match self.get(port).ok_or_else(|| self.missing(port))? {
            Value::NumpyRng(r) => Ok(r),
            v => Err(self.wrong(port, "numpy generator (rng.numpy)", v)),
        }
    }

    /// A setting read into a type (its default when absent).
    pub fn param<T: DeserializeOwned>(&self, name: &str, default: T) -> Result<T> {
        match self.params.get(name) {
            None => Ok(default),
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: the setting {name:?}: {e}", self.node))),
        }
    }

    pub fn opt_param<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        match self.params.get(name) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(v) => serde_json::from_value(v.clone())
                .map(Some)
                .map_err(|e| Error::msg(format!("node {:?}: the setting {name:?}: {e}", self.node))),
        }
    }
}

pub type Outputs = BTreeMap<String, Value>;

/// An operator of the graph.
pub trait Operator: Send + Sync {
    fn signature(&self) -> OpSignature;
    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs>;
}

/// The operators by name, with their signatures made once.
#[derive(Clone)]
pub struct Registry {
    ops: BTreeMap<String, Arc<dyn Operator>>,
    sigs: BTreeMap<String, Arc<OpSignature>>,
}

impl Registry {
    pub fn empty() -> Self {
        Registry { ops: BTreeMap::new(), sigs: BTreeMap::new() }
    }

    /// Every operator of this crate.
    pub fn standard() -> Self {
        let mut r = Registry::empty();
        r.add(Arc::new(Repeat));
        r.add(Arc::new(Group));
        r.add(Arc::new(Submodel));
        r.add(Arc::new(RngNew));
        accuracy::register(&mut r);
        agents::register(&mut r);
        algebra::register(&mut r);
        change::register(&mut r);
        change_analysis::register(&mut r);
        differ::register(&mut r);
        prioritize_tools::register(&mut r);
        futures_tools::register(&mut r);
        map_validation::register(&mut r);
        batch::register(&mut r);
        bio::register(&mut r);
        carbon::register(&mut r);
        circuit::register(&mut r);
        circuit_ini::register(&mut r);
        uncertainty::register(&mut r);
        cost::register(&mut r);
        surface::register(&mut r);
        landforms::register(&mut r);
        roads::register(&mut r);
        swat::register(&mut r);
        hydro::register(&mut r);
        services::register(&mut r);
        legend::register(&mut r);
        clumondo::register(&mut r);
        crafty::register(&mut r);
        data::register(&mut r);
        markov::register(&mut r);
        metrics::register(&mut r);
        ml::register(&mut r);
        molusce::register(&mut r);
        nlm::register(&mut r);
        conn::register(&mut r);
        sdm::register(&mut r);
        demand::register(&mut r);
        glm::register(&mut r);
        woe::register(&mut r);
        lulcc::register(&mut r);
        dinamica::register(&mut r);
        dynamic::register(&mut r);
        files::register(&mut r);
        forest_code::register(&mut r);
        fitted::register(&mut r);
        fragmentation::register(&mut r);
        futures::register(&mut r);
        landscape::register(&mut r);
        mspa_tiled::register(&mut r);
        plus::register(&mut r);
        prepare::register(&mut r);
        prioritize::register(&mut r);
        zonation_project::register(&mut r);
        h3::register(&mut r);
        actions::register(&mut r);
        actions_raster::register(&mut r);
        restoration::register(&mut r);
        redd::register(&mut r);
        restoration_plan::register(&mut r);
        vector::register(&mut r);
        planner::register(&mut r);
        sampling::register(&mut r);
        signatures::register(&mut r);
        gtn::register(&mut r);
        plugin::register(&mut r);
        validate::register(&mut r);
        view::register(&mut r);
        tables::register(&mut r);
        r
    }

    pub fn add(&mut self, op: Arc<dyn Operator>) {
        let sig = op.signature();
        self.ops.insert(sig.name.clone(), op);
        self.sigs.insert(sig.name.clone(), Arc::new(sig));
    }

    /// The signature of an operator (made when it was added).
    pub fn sig(&self, name: &str) -> Option<&Arc<OpSignature>> {
        self.sigs.get(name)
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Operator>> {
        self.ops.get(name)
    }

    pub fn signatures(&self) -> Vec<OpSignature> {
        self.sigs.values().map(|s| (**s).clone()).collect()
    }
}

impl Catalog for Registry {
    fn signature(&self, op: &str) -> Option<OpSignature> {
        self.sigs.get(op).map(|s| (**s).clone())
    }
}

/// `control.repeat`: the engine runs its body; the operator only declares it.
pub struct Repeat;

impl Operator for Repeat {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "control.repeat",
            "Run the body once per step (1..times): `@step` is the step, each carried value starts as the input of \
             its name and takes the value of its reference at the end of every step, each collected reference \
             gives a series with its value at every step. The outputs are the carried values after the last step \
             and the collected series.",
            Access::None,
        )
        .input(PortSpec::new("times", Type::Integer, "how many steps"))
        .container()
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        Err(Error::msg(format!("node {:?}: control.repeat is run by the engine", args.node)))
    }
}

/// `control.group`: the engine runs its body once; the operator only declares it.
pub struct Group;

impl Operator for Group {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            terraflux_graph::GROUP,
            "Run the body once: its inputs (any names) are @name inside, its outputs the references of body.outputs. Submodels become groups.",
            Access::None,
        )
        .extra(Type::Any)
        .container()
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        Err(Error::msg(format!("node {:?}: control.group is run by the engine", args.node)))
    }
}

/// `control.submodel`: another model file, expanded into a group before a run.
pub struct Submodel;

impl Operator for Submodel {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            terraflux_graph::SUBMODEL,
            "Another model used as one node: its inputs are the node's inputs of the same names (its value inputs keep their values when not given; its files become inputs of this model), its outputs the node's. Expanded into a control.group before the model runs.",
            Access::None,
        )
        .extra(Type::Any)
        .param(PortSpec::new("path", Type::Text, "the model file, relative to this model's folder"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        Err(Error::msg(format!("node {:?}: expand the submodels before running the model", args.node)))
    }
}

/// `rng.r`: R's generator after `set.seed(seed)`.
pub struct RngNew;

impl Operator for RngNew {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "rng.r",
            "R's random number generator after set.seed(seed) (Mersenne-Twister), as lulcc and landschaft use it: \
             carry it along a loop so that every step draws after the previous one.",
            Access::None,
        )
        .input(PortSpec::new("seed", Type::Integer, "the seed"))
        .output(PortSpec::new("rng", Type::Rng, "the generator"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let seed = args.integer("seed")?;
        let seed = i32::try_from(seed).map_err(|_| Error::msg(format!("the seed {seed} does not fit 32 bits")))?;
        let rng = value::Rng(Arc::new(std::sync::Mutex::new(terraflux_ls_sys::RRng::new(seed))));
        Ok([("rng".to_string(), Value::Rng(rng))].into())
    }
}

/// Outputs from (name, value) pairs.
pub(crate) fn outputs<const N: usize>(pairs: [(&str, Value); N]) -> Outputs {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}
