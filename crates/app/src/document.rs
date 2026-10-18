//! The model as a document: the same graph read as the methods section of a paper (`docs/DOCUMENT_VIEW.md`). The
//! top-level nodes in five stages (data, preparation, model, validation, results), each stage in the order of the
//! dependencies with the ties broken by where the nodes are on the canvas (left to right, then top to bottom: the
//! order the author laid them out in); a step each, with its note, why, settings in words, inputs, expression and
//! record; a container's body as steps within it; the plots as the figures of the steps they show. Built from the
//! model and the operators' signatures alone: the Document pane draws it with the values of the last run, and the
//! report writes it ([`crate::report`]).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value as Json;
use terraflux_graph::{Input, Model, Node, OpSignature, Ref, Source, Type};
use terraflux_ops::Registry;

use crate::node_view::{default_of, label_of};

/// A stage of a model, in the order a document tells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// What is read or drawn: nodes that take nothing from other nodes.
    Data,
    /// What is done to the data before the model.
    Preparation,
    /// Fits, simulations, loops, optimisations, and what lies between them.
    Model,
    /// The model checked: agreement, accuracy, importance, evaluations.
    Validation,
    /// What comes of the model: maps made from it, the plots, the files written.
    Results,
}

impl Stage {
    pub const ALL: [Stage; 5] = [Stage::Data, Stage::Preparation, Stage::Model, Stage::Validation, Stage::Results];

    pub fn title(self) -> &'static str {
        match self {
            Stage::Data => "Data",
            Stage::Preparation => "Preparation",
            Stage::Model => "Model",
            Stage::Validation => "Validation",
            Stage::Results => "Results",
        }
    }
}

/// How much room a step takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// A row of the data's table.
    Data,
    /// A line: the plumbing (stacks, generators) without a why.
    Line,
    /// A card: its note, why, inputs, settings and record.
    Card,
}

/// Where an input comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum From {
    /// An output of a node (in the same scope).
    Node { node: String, port: String },
    /// An input of the model (`$name`).
    Input(String),
    /// A value carried along a loop, or a group's input (`@name`).
    Carried(String),
    /// A value written in the model.
    Value(String),
}

impl From {
    fn of(s: &Source) -> From {
        match s {
            Source::Ref(r) => match Ref::parse(r) {
                Ok(Ref::Port { node, port }) => From::Node { node, port },
                Ok(Ref::Input(n)) => From::Input(n),
                Ok(Ref::Carried(n)) => From::Carried(n),
                Err(_) => From::Value(r.clone()),
            },
            Source::Literal(v) => From::Value(words(v)),
        }
    }

    /// The source in a few words: a node (its port too unless it is the node's only output, `only` says which),
    /// `$input`, `@carried`, the value.
    pub fn text(&self, only: impl Fn(&str) -> Option<String>) -> String {
        match self {
            From::Node { node, port } => {
                if only(node).as_deref() == Some(port.as_str()) {
                    node.clone()
                } else {
                    format!("{node}.{port}")
                }
            }
            From::Input(n) => format!("${n}"),
            From::Carried(n) => format!("@{n}"),
            From::Value(v) => v.clone(),
        }
    }

    /// The node it comes from.
    pub fn node(&self) -> Option<&str> {
        match self {
            From::Node { node, .. } => Some(node),
            _ => None,
        }
    }
}

/// An input of a step: its port, what the port is (the operator's words), where it comes from.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub port: String,
    pub doc: String,
    pub from: From,
}

/// A setting given to a step, in words.
#[derive(Debug, Clone, PartialEq)]
pub struct Setting {
    pub name: String,
    /// What it is: the operator's words for it when they are short, else its name in words.
    pub label: String,
    /// The operator's whole description of it.
    pub doc: String,
    /// Its value in words.
    pub value: String,
    /// The value it has when not given, as its description says, when the value given differs from it.
    pub default: Option<String>,
    /// Whether it is a single value (a number, a text, a yes or no), edited in place.
    pub plain: bool,
    /// The values it takes, when they are a few (a yes or no among them).
    pub choices: Vec<String>,
}

/// A plot shown with the step whose output it shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Figure {
    /// The `view.plot` node.
    pub plot: String,
    /// What it shows: a node and its port.
    pub node: String,
    pub port: String,
    pub caption: String,
    /// Its number in the document (1, 2...).
    pub number: usize,
}

/// What a container carries, collects and gives, how many times it runs, and its body.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    /// How many steps (a loop's `times`).
    pub times: Option<From>,
    /// A carried value: its name, where it starts, what it takes at the end of every step.
    pub carry: Vec<(String, Option<From>, String)>,
    /// A collected series: its name and the reference collected.
    pub collect: Vec<(String, String)>,
    /// A group's output: its name and the reference giving it.
    pub outputs: Vec<(String, String)>,
    /// The reference that ends the loop when true.
    pub until: Option<String>,
    pub steps: Vec<Step>,
}

/// A step of the document: a node.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The containers it is in (empty at the top level).
    pub path: Vec<String>,
    pub id: String,
    pub op: String,
    pub stage: Stage,
    pub size: Size,
    pub note: Option<String>,
    pub why: Option<String>,
    pub refs: Vec<String>,
    pub assumptions: Vec<String>,
    pub alternatives: Vec<String>,
    pub inputs: Vec<Link>,
    pub settings: Vec<Setting>,
    /// Map algebra's expression (its inputs are its variables).
    pub expression: Option<String>,
    /// A file it reads or writes (`path`).
    pub file: Option<String>,
    pub figures: Vec<Figure>,
    /// The output whose value is shown with it (a map, rasters, a stack, a series or a table), and its type.
    pub shows: Option<(String, Type)>,
    pub body: Option<Body>,
}

/// A stage of the document and its steps.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub stage: Stage,
    /// The stage's title (the preparation is the analysis when there is no model).
    pub title: String,
    pub steps: Vec<Step>,
}

/// An input of the model.
#[derive(Debug, Clone, PartialEq)]
pub struct InputRow {
    pub name: String,
    /// "a map", "a stack", "a value".
    pub kind: &'static str,
    /// The file, or the value in words.
    pub value: String,
    pub note: Option<String>,
}

/// An output of the model.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputRow {
    pub name: String,
    pub from: String,
    pub file: String,
}

/// The model as a document.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub title: String,
    pub description: Option<String>,
    pub request: Option<String>,
    pub summary: Option<String>,
    pub author: Option<String>,
    pub inputs: Vec<InputRow>,
    pub sections: Vec<Section>,
    /// The plots of what is not a step (a model's input): shown with the results.
    pub figures: Vec<Figure>,
    pub outputs: Vec<OutputRow>,
}

/// What validates a model.
const VALIDATION: [&str; 4] = ["validate.", "accuracy.", "check.", "sensitivity.analyze"];
const VALIDATION_OPS: [&str; 7] = [
    "sdm.evaluate",
    "sdm.importance",
    "sdm.response_curves",
    "ml.importance",
    "ml.partial_dependence",
    "redd.assess",
    "metrics.correlation",
];

/// What shows or writes the results.
const RESULTS: [&str; 4] = ["view.", "raster.write", "vector.write", "table.write"];

/// The general tools: data read or drawn, map algebra, rasters and vectors handled, stacks and tables; prepare
/// the data or carry results, never the model itself.
const TOOLS: [&str; 12] = [
    "map.", "raster.", "vector.", "stack.", "table.", "dynamic.", "rng.", "prepare.", "data.", "nlm.", "series.",
    "legend.",
];

/// The operators of a field that prepare its model's data.
const PREPARING: [&str; 10] = [
    "sdm.prepare",
    "sdm.background",
    "sdm.clean_occurrences",
    "sdm.thin",
    "sdm.accessible_area",
    "sdm.select_predictors",
    "sdm.download_gbif",
    "sdm.area_of_habitat",
    "restoration.original_vegetation",
    "lulcc.history",
];

/// The plumbing: a line, unless it says why.
const PLUMBING: [&str; 3] = ["stack.", "rng.", "lulcc.history"];

fn is_validation(op: &str) -> bool {
    VALIDATION.iter().any(|p| op.starts_with(p)) || VALIDATION_OPS.contains(&op)
}

fn is_result(op: &str) -> bool {
    RESULTS.iter().any(|p| op.starts_with(p))
}

fn is_tool(op: &str) -> bool {
    TOOLS.iter().any(|p| op.starts_with(p)) || PREPARING.contains(&op)
}

fn is_plumbing(op: &str) -> bool {
    PLUMBING.iter().any(|p| op.starts_with(p))
}

/// Whether a node is part of a model (a fit, a simulation, an optimisation...): an operator of a field that is
/// not a tool, or a container holding one.
fn is_analysis(n: &Node) -> bool {
    match &n.body {
        Some(b) => b.nodes.iter().any(is_analysis),
        None => !(is_tool(&n.op) || is_validation(&n.op) || is_result(&n.op)),
    }
}

/// Whether a node is data: a tool (a reader, a neutral landscape...) that takes nothing from another node.
fn is_data(n: &Node) -> bool {
    n.body.is_none()
        && is_tool(&n.op)
        && !n.op.starts_with("rng.")
        && !PREPARING.contains(&n.op.as_str())
        && n.inputs.values().all(|s| !matches!(s, Source::Ref(r) if matches!(Ref::parse(r), Ok(Ref::Port { .. }))))
}

/// The nodes of a scope in the order a reader follows them: each after what it depends on, the ties broken by
/// where they are on the canvas (left to right, then top to bottom; where the canvas puts those without a place),
/// then by their order in the file.
pub fn reading_order(nodes: &[Node]) -> Vec<&Node> {
    let among = terraflux_graph::ids(nodes);
    let index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let deps: Vec<Vec<usize>> = nodes
        .iter()
        .map(|n| {
            terraflux_graph::dependencies_among(n, &among)
                .iter()
                .filter_map(|d| index.get(d.as_str()).copied())
                .collect()
        })
        .collect();
    let auto =
        if nodes.iter().any(|n| n.pos.is_none()) { crate::editor_view::auto_positions(nodes) } else { HashMap::new() };
    let key = |i: usize| {
        let [x, y] = nodes[i].pos.or_else(|| auto.get(&nodes[i].id).copied()).unwrap_or([f32::MAX, f32::MAX]);
        (x, y, i)
    };
    let mut done = vec![false; nodes.len()];
    let mut out = Vec::with_capacity(nodes.len());
    loop {
        let ready = (0..nodes.len())
            .filter(|&i| !done[i] && deps[i].iter().all(|&d| done[d] || d == i))
            .min_by(|&a, &b| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal));
        match ready {
            Some(i) => {
                done[i] = true;
                out.push(&nodes[i]);
            }
            None => break,
        }
    }
    // (a cycle: the rest as written)
    out.extend((0..nodes.len()).filter(|&i| !done[i]).map(|i| &nodes[i]));
    out
}

/// The stage of every top-level node: data, validation and results by their operators; a model's operators are
/// the model; a tool before the model is preparation, after it a result, between two of its nodes the model; and
/// no node in a stage before one it depends on (what takes a validation's sample validates too).
pub fn stages(nodes: &[Node]) -> HashMap<String, Stage> {
    let order = reading_order(nodes);
    let among = terraflux_graph::ids(nodes);
    let deps: HashMap<&str, Vec<String>> =
        nodes.iter().map(|n| (n.id.as_str(), terraflux_graph::dependencies_among(n, &among))).collect();
    let analysis: HashSet<&str> = nodes.iter().filter(|n| is_analysis(n)).map(|n| n.id.as_str()).collect();
    // after a node of the model
    let mut after: HashSet<String> = HashSet::new();
    for n in &order {
        if deps[n.id.as_str()].iter().any(|d| analysis.contains(d.as_str()) || after.contains(d)) {
            after.insert(n.id.clone());
        }
    }
    // before one
    let mut before: HashSet<String> = HashSet::new();
    for n in order.iter().rev() {
        let feeds = |m: &Node| {
            deps[m.id.as_str()].contains(&n.id) && (analysis.contains(m.id.as_str()) || before.contains(&m.id))
        };
        if nodes.iter().any(feeds) {
            before.insert(n.id.clone());
        }
    }
    let mut stages: HashMap<String, Stage> = nodes
        .iter()
        .map(|n| {
            let stage = if is_result(&n.op) {
                Stage::Results
            } else if is_validation(&n.op) {
                Stage::Validation
            } else if is_data(n) {
                Stage::Data
            } else if analysis.contains(n.id.as_str()) {
                Stage::Model
            } else {
                match (after.contains(&n.id), before.contains(&n.id)) {
                    (true, true) => Stage::Model,
                    (true, false) => Stage::Results,
                    _ => Stage::Preparation,
                }
            };
            (n.id.clone(), stage)
        })
        .collect();
    for n in &order {
        let latest = deps[n.id.as_str()].iter().filter_map(|d| stages.get(d).copied()).max();
        if let Some(l) = latest
            && l > stages[&n.id]
        {
            stages.insert(n.id.clone(), l);
        }
    }
    stages
}

/// A value in words: a text as it is, numbers as written, yes or no, a list's items and a table's keys and values
/// in a line.
pub fn words(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        Json::Bool(true) => "yes".into(),
        Json::Bool(false) => "no".into(),
        Json::Null => "none".into(),
        // (as people write numbers: 0.000005, not 5e-6)
        Json::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.to_string(),
            (None, Some(f)) => f.to_string(),
            _ => n.to_string(),
        },
        Json::Array(a) => a.iter().map(words).collect::<Vec<_>>().join(", "),
        Json::Object(o) => o
            .iter()
            .map(|(k, v)| match v {
                Json::Object(_) | Json::Array(_) => format!("{} ({})", label_of(k).to_lowercase(), words(v)),
                _ => format!("{} {}", label_of(k).to_lowercase(), words(v)),
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// What a setting is, in the operator's words when they are a short phrase ("the range of the autocorrelation, in
/// cells" for "the range of the autocorrelation, in cells (default 10)"), else its name in words.
pub fn setting_label(name: &str, doc: &str) -> String {
    let mut d = doc.to_string();
    if let Some(at) = d.find("(default") {
        let end = d[at..].find(')').map_or(d.len(), |e| at + e + 1);
        d.replace_range(at..end, "");
    }
    let d = d.trim().trim_end_matches(['.', ',', ';', ':']).trim();
    let phrase = !d.is_empty()
        && d.chars().count() <= 60
        && d.chars().next().is_some_and(|c| c.is_alphabetic())
        && !d.contains(['{', '[', '(', ':', ';']);
    if phrase { d.to_string() } else { label_of(name) }
}

fn setting(name: &str, v: &Json, spec: Option<&terraflux_graph::PortSpec>) -> Setting {
    let doc = spec.map(|p| p.doc.clone()).unwrap_or_default();
    let value = words(v);
    let default = default_of(&doc);
    let default = default.strip_prefix("default ").unwrap_or(&default).trim().to_string();
    let differs = !default.is_empty()
        && default != value
        && !(default == "true" && value == "yes")
        && !(default == "false" && value == "no");
    let mut choices = spec.map(|p| p.choices.clone()).unwrap_or_default();
    if spec.is_some_and(|p| p.ty == Type::Bool) && choices.is_empty() {
        choices = vec!["true".into(), "false".into()];
    }
    Setting {
        name: name.to_string(),
        label: setting_label(name, &doc),
        doc,
        value,
        default: differs.then_some(default),
        plain: matches!(v, Json::String(_) | Json::Number(_) | Json::Bool(_)),
        choices,
    }
}

/// The output of a node shown with it: its first map (rasters, a stack, a series), else its first table.
fn shown_output(n: &Node, sig: Option<&OpSignature>) -> Option<(String, Type)> {
    let mut outs: Vec<(String, Type)> =
        sig.map(|s| s.outputs.iter().map(|p| (p.name.clone(), p.ty)).collect()).unwrap_or_default();
    if let Some(b) = &n.body {
        outs.extend(b.collect.keys().map(|k| (k.clone(), Type::Series)));
        outs.extend(b.carry.keys().chain(b.outputs.keys()).map(|k| (k.clone(), Type::Any)));
    }
    let map = |t: Type| matches!(t, Type::Raster | Type::Map | Type::Stack | Type::Series);
    outs.iter().find(|(_, t)| map(*t)).or_else(|| outs.iter().find(|(_, t)| *t == Type::Table)).cloned()
}

fn step(n: &Node, path: &[String], stage: Stage, r: &Registry) -> Step {
    let sig = r.sig(&n.op).map(|s| s.as_ref());
    let carried: HashSet<&str> =
        n.body.as_ref().map(|b| b.carry.keys().map(String::as_str).collect()).unwrap_or_default();
    // the inputs in the operator's order, then the others (map algebra's variables, a container's)
    let mut ports: Vec<&String> = n.inputs.keys().collect();
    let rank = |p: &str| sig.and_then(|s| s.inputs.iter().position(|i| i.name == p)).unwrap_or(usize::MAX);
    ports.sort_by_key(|p| (rank(p), (*p).clone()));
    let inputs = ports
        .into_iter()
        .filter(|p| !carried.contains(p.as_str()) && !(n.op == "control.repeat" && p.as_str() == "times"))
        .map(|p| Link {
            port: p.clone(),
            doc: sig.and_then(|s| s.input_spec(p)).map(|i| i.doc.clone()).unwrap_or_default(),
            from: From::of(&n.inputs[p]),
        })
        .collect();
    let expression = (n.op == "map.calc").then(|| n.params.get("expression").map(words)).flatten();
    let file = n.params.get("path").filter(|_| terraflux_graph::FILE_OPS.contains(&n.op.as_str())).map(words);
    let mut names: Vec<&String> = n.params.keys().collect();
    let prank = |p: &str| sig.and_then(|s| s.params.iter().position(|i| i.name == p)).unwrap_or(usize::MAX);
    names.sort_by_key(|p| (prank(p), (*p).clone()));
    let settings = names
        .into_iter()
        .filter(|k| !(expression.is_some() && k.as_str() == "expression") && !(file.is_some() && k.as_str() == "path"))
        .map(|k| setting(k, &n.params[k], sig.and_then(|s| s.params.iter().find(|p| &p.name == k))))
        .collect();
    let size = if stage == Stage::Data && path.is_empty() {
        Size::Data
    } else if is_plumbing(&n.op) && n.why.is_none() && n.body.is_none() {
        Size::Line
    } else {
        Size::Card
    };
    let body = n.body.as_ref().map(|b| {
        let inner: Vec<String> = path.iter().cloned().chain([n.id.clone()]).collect();
        Body {
            times: n.inputs.get("times").filter(|_| n.op == "control.repeat").map(From::of),
            carry: b.carry.iter().map(|(k, each)| (k.clone(), n.inputs.get(k).map(From::of), each.clone())).collect(),
            collect: b.collect.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            outputs: b.outputs.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            until: b.until.clone(),
            steps: reading_order(&b.nodes).into_iter().map(|m| step(m, &inner, stage, r)).collect(),
        }
    });
    Step {
        path: path.to_vec(),
        id: n.id.clone(),
        op: n.op.clone(),
        stage,
        size,
        note: n.note.clone().filter(|t| !t.trim().is_empty()),
        why: n.why.clone().filter(|t| !t.trim().is_empty()),
        refs: n.refs.clone(),
        assumptions: n.assumptions.clone(),
        alternatives: n.alternatives.clone(),
        inputs,
        settings,
        expression,
        file,
        figures: Vec::new(),
        shows: if size == Size::Line { None } else { shown_output(n, sig) },
        body,
    }
}

/// What a plot shows: the node and port of its map, else its features, else its table.
fn plotted(n: &Node) -> Option<(String, String)> {
    ["value", "features", "table"].iter().find_map(|p| match n.inputs.get(*p)? {
        Source::Ref(r) => match Ref::parse(r).ok()? {
            Ref::Port { node, port } => Some((node, port)),
            _ => None,
        },
        Source::Literal(_) => None,
    })
}

impl Document {
    /// The document of a model.
    pub fn of(m: &Model, r: &Registry) -> Document {
        let stage = stages(&m.nodes);
        let plots: BTreeMap<&str, &Node> =
            m.nodes.iter().filter(|n| n.op == "view.plot").map(|n| (n.id.as_str(), n)).collect();
        let mut sections: Vec<Section> = Vec::new();
        for st in Stage::ALL {
            let steps: Vec<Step> = reading_order(&m.nodes)
                .into_iter()
                .filter(|n| stage[&n.id] == st && !plots.contains_key(n.id.as_str()))
                .map(|n| step(n, &[], st, r))
                .collect();
            if !steps.is_empty() {
                sections.push(Section { stage: st, title: st.title().to_string(), steps });
            }
        }
        // without a model, what is done to the data is the analysis
        if !sections.iter().any(|s| s.stage == Stage::Model)
            && let Some(p) = sections.iter_mut().find(|s| s.stage == Stage::Preparation)
        {
            p.title = "Analysis".into();
        }
        // the plots: the figures of the steps they show, numbered in the document's order
        let mut loose = Vec::new();
        for n in reading_order(&m.nodes) {
            let Some(plot) = plots.get(n.id.as_str()) else { continue };
            let Some((node, port)) = plotted(plot) else { continue };
            let caption = plot.params.get("title").map(words).unwrap_or_else(|| format!("{node}.{port}"));
            let f = Figure { plot: plot.id.clone(), node: node.clone(), port, caption, number: 0 };
            match sections.iter_mut().flat_map(|s| s.steps.iter_mut()).find(|s| s.id == node) {
                Some(s) => s.figures.push(f),
                None => loose.push(f),
            }
        }
        let mut number = 0;
        for s in sections.iter_mut().flat_map(|s| s.steps.iter_mut()) {
            for f in &mut s.figures {
                number += 1;
                f.number = number;
            }
            // (the output shown by a figure is not shown twice)
            if s.shows.as_ref().is_some_and(|(p, _)| s.figures.iter().any(|f| &f.port == p)) {
                s.shows = None;
            }
        }
        for f in &mut loose {
            number += 1;
            f.number = number;
        }
        Document {
            title: m.name.clone().unwrap_or_else(|| "Untitled".into()),
            description: m.description.clone(),
            request: m.request.clone(),
            summary: m.summary.clone(),
            author: m.author.clone(),
            inputs: m
                .inputs
                .iter()
                .map(|(k, i)| {
                    let (kind, value, note) = match i {
                        Input::Map { path, band, note, .. } => (
                            "a map",
                            match band {
                                Some(b) => format!("{path}, band {b}"),
                                None => path.clone(),
                            },
                            note,
                        ),
                        Input::Stack { path, note, .. } => ("a stack", path.clone(), note),
                        Input::Value { value, note } => ("a value", words(value), note),
                    };
                    InputRow { name: k.clone(), kind, value, note: note.clone() }
                })
                .collect(),
            sections,
            figures: loose,
            outputs: m
                .outputs
                .iter()
                .map(|(k, o)| OutputRow {
                    name: k.clone(),
                    from: o.from.clone(),
                    file: o.path.clone().unwrap_or_default(),
                })
                .collect(),
        }
    }

    /// Every step, its body's included, in the document's order.
    pub fn steps(&self) -> Vec<&Step> {
        fn walk<'a>(s: &'a Step, out: &mut Vec<&'a Step>) {
            out.push(s);
            for b in s.body.iter().flat_map(|b| &b.steps) {
                walk(b, out);
            }
        }
        let mut out = Vec::new();
        for s in self.sections.iter().flat_map(|s| &s.steps) {
            walk(s, &mut out);
        }
        out
    }

    /// The steps without a note or a why (what `check_model` advises to write).
    pub fn unexplained(&self) -> usize {
        self.steps().iter().filter(|s| s.size == Size::Card && (s.note.is_none() || s.why.is_none())).count()
    }
}

/// The output of a top-level node when it has only one: a reference to it names the node alone.
pub fn only_output(m: &Model, r: &Registry, node: &str) -> Option<String> {
    let n = m.nodes.iter().find(|n| n.id == node)?;
    match crate::editor::output_ports(n, r).as_slice() {
        [(p, _)] => Some(p.clone()),
        _ => None,
    }
}

/// The rows of the data's table: the data steps one after another of the same operator and note (and no why)
/// together, as the three maps of the same land use.
pub fn data_rows(steps: &[Step]) -> Vec<Vec<&Step>> {
    let mut rows: Vec<Vec<&Step>> = Vec::new();
    for s in steps {
        match rows.last_mut() {
            Some(row) if row[0].op == s.op && row[0].note == s.note && row[0].why.is_none() && s.why.is_none() => {
                row.push(s)
            }
            _ => rows.push(vec![s]),
        }
    }
    rows
}

/// The settings of a row of data steps: each name once, with the values of each step (one, when they agree).
pub fn row_settings(row: &[&Step]) -> Vec<(String, String)> {
    let mut names: Vec<&Setting> = Vec::new();
    for s in row {
        for st in &s.settings {
            if !names.iter().any(|n| n.name == st.name) {
                names.push(st);
            }
        }
    }
    names
        .into_iter()
        .map(|n| {
            let values: Vec<String> = row
                .iter()
                .map(|s| s.settings.iter().find(|t| t.name == n.name).map_or("-".to_string(), |t| t.value.clone()))
                .collect();
            let one = values.iter().all(|v| *v == values[0]);
            (n.label.clone(), if one { values[0].clone() } else { values.join(", ") })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(name: &str) -> Model {
        crate::templates::TEMPLATES.iter().find(|t| t.name == name).expect("a template").model()
    }

    fn ids(sec: &Section) -> Vec<&str> {
        sec.steps.iter().map(|s| s.id.as_str()).collect()
    }

    #[test]
    fn a_model_reads_in_stages() {
        let d = Document::of(&template("CLUE-S"), &Registry::standard());
        let stages: Vec<Stage> = d.sections.iter().map(|s| s.stage).collect();
        assert_eq!(stages, [Stage::Data, Stage::Preparation, Stage::Model]);
        assert_eq!(ids(&d.sections[0]), ["land_2000", "aptitude", "roughness"]);
        assert_eq!(ids(&d.sections[1]), ["rng", "land_2010", "drivers", "observed", "history"]);
        assert_eq!(ids(&d.sections[2]), ["regression", "suitability", "demand", "simulation"]);
        // the plot is the loop's figure, not a step
        let sim = d.sections[2].steps.iter().find(|s| s.id == "simulation").unwrap();
        assert_eq!(sim.figures.len(), 1);
        assert_eq!(sim.figures[0].caption, "Simulated land use");
        assert_eq!(sim.figures[0].number, 1);
        let body = sim.body.as_ref().unwrap();
        assert_eq!(body.steps[0].id, "allocate");
        assert_eq!(body.steps[0].path, ["simulation"]);
        assert_eq!(body.times, Some(From::Node { node: "demand".into(), port: "steps".into() }));
        assert!(body.carry.iter().any(|(k, start, each)| k == "landscape"
            && start == &Some(From::Node { node: "land_2010".into(), port: "map".into() })
            && each == "allocate.landscape"));
        // the plumbing in a line
        let drivers = d.sections[1].steps.iter().find(|s| s.id == "drivers").unwrap();
        assert_eq!(drivers.size, Size::Line);
        assert_eq!(d.sections[0].steps[0].size, Size::Data);
    }

    #[test]
    fn what_follows_the_model_is_a_result_and_evaluations_validate() {
        let d = Document::of(&template("A species in a warmer climate"), &Registry::standard());
        let of = |st: Stage| d.sections.iter().find(|s| s.stage == st).map(ids).unwrap_or_default();
        assert_eq!(of(Stage::Data), ["field_t", "field_r"]);
        assert!(of(Stage::Preparation).starts_with(&["temperature", "rainfall"]), "{:?}", of(Stage::Preparation));
        assert!(of(Stage::Preparation).contains(&"warmer"), "the data of the future: before the model");
        assert!(of(Stage::Preparation).contains(&"data"), "the preparation of an SDM is preparation");
        assert_eq!(of(Stage::Model), ["fit", "today", "tomorrow", "mess"]);
        assert_eq!(of(Stage::Validation), ["curves", "importance"]);
        assert_eq!(of(Stage::Results), ["shift"]);
        let figures: Vec<(usize, &str)> =
            d.steps().iter().flat_map(|s| s.figures.iter().map(|f| (f.number, f.node.as_str()))).collect();
        assert_eq!(figures, [(1, "today"), (2, "tomorrow"), (3, "mess"), (4, "shift")]);
        // map algebra: its expression, its variables the inputs
        let shift = d.steps().into_iter().find(|s| s.id == "shift").unwrap();
        assert_eq!(shift.expression.as_deref(), Some("b - a"));
        assert_eq!(shift.inputs.len(), 2);
        assert!(shift.settings.is_empty());
    }

    #[test]
    fn settings_in_words() {
        let r = Registry::standard();
        let d = Document::of(&template("CLUE-S"), &r);
        let aptitude = d.steps().into_iter().find(|s| s.id == "aptitude").unwrap();
        let range = aptitude.settings.iter().find(|s| s.name == "range").unwrap();
        assert_eq!(range.label, "the range of the autocorrelation, in cells");
        assert_eq!(range.value, "25");
        assert_eq!(range.default.as_deref(), Some("10"));
        assert!(range.plain);
        let allocate = d.steps().into_iter().find(|s| s.id == "allocate").unwrap();
        let alloc = allocate.settings.iter().find(|s| s.name == "allocation").unwrap();
        assert_eq!(alloc.label, "Allocation");
        assert_eq!(alloc.value, "max iter 5000, method clue, scale 0.000005");
        assert!(!alloc.plain);
        assert_eq!(words(&serde_json::json!([0.5, 0.2, 1.0])), "0.5, 0.2, 1");
        assert_eq!(setting_label("rows", "rows (default 256)"), "rows");
        assert_eq!(setting_label("allocation", "{ method = \"clue\" }"), "Allocation");
    }

    #[test]
    fn data_of_one_kind_share_a_row() {
        let d = Document::of(&template("Restoration plan (plangea)"), &Registry::standard());
        let data = &d.sections[0];
        let rows = data_rows(&data.steps);
        let ids: Vec<Vec<&str>> = rows.iter().map(|r| r.iter().map(|s| s.id.as_str()).collect()).collect();
        assert_eq!(ids, [vec!["field"], vec!["toucan", "jaguar", "frog"]]);
        let settings = row_settings(&rows[1]);
        let range = settings.iter().find(|(l, _)| l.starts_with("the range")).unwrap();
        assert_eq!(range.1, "15, 20, 6");
        let rows_setting = settings.iter().find(|(l, _)| l == "rows").unwrap();
        assert_eq!(rows_setting.1, "40");
    }

    #[test]
    fn every_template_makes_a_document() {
        let r = Registry::standard();
        for t in crate::templates::TEMPLATES.iter() {
            let m = t.model();
            let d = Document::of(&m, &r);
            let plots = m.nodes.iter().filter(|n| n.op == "view.plot").count();
            let steps = d.sections.iter().map(|s| s.steps.len()).sum::<usize>();
            assert_eq!(steps + plots, m.nodes.len(), "{}: every node once", t.name);
            // a step never before what it depends on in its stage
            for sec in &d.sections {
                let mut seen = HashSet::new();
                for s in &sec.steps {
                    for l in &s.inputs {
                        if let Some(n) = l.from.node()
                            && sec.steps.iter().any(|o| o.id == n)
                        {
                            assert!(seen.contains(n), "{}: {} before {n}", t.name, s.id);
                        }
                    }
                    seen.insert(s.id.as_str());
                }
            }
            // and the stages in the order of the dependencies, results and validation aside
            let at: HashMap<&str, Stage> = d.steps().iter().map(|s| (s.id.as_str(), s.stage)).collect();
            for s in d.steps() {
                if !s.path.is_empty() || matches!(s.stage, Stage::Results | Stage::Validation) {
                    continue;
                }
                for l in &s.inputs {
                    if let Some(n) = l.from.node()
                        && let Some(st) = at.get(n)
                    {
                        assert!(*st <= s.stage, "{}: {} ({:?}) takes {n} ({st:?})", t.name, s.id, s.stage);
                    }
                }
            }
        }
    }
}
