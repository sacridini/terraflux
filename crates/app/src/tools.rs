//! The tools an assistant builds and runs a model with: those of `terraflux mcp` (on a model's file) and of the
//! assistant in the application ([`crate::chat`], on the model open there). The templates and the operators
//! listed and described, nodes added, connected and set (each change checked, a wrong one refused with the
//! checker's reason and the model left as it was), the model checked, run, and its results looked at.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use terraflux_exec::{Engine, Observer, load_inputs, write_outputs};
use terraflux_graph::{Input, Model, Node, OpSignature, Output, PortSpec, Ref, Severity, check, valid_id};
use terraflux_ops::Registry;

use crate::{editor, editor_view};

pub type Reply = std::result::Result<String, String>;

/// What a node is and reads (told to every assistant).
macro_rules! nodes {
    () => {
        "- A node runs one operator (`op`) and gives outputs (its ports). An input of a node reads `node.port` \
(another node's output), `$name` (an input of the model), `@name` (inside a container: a value carried along a \
loop, `@step`, a group's input) or is a literal value. Settings are the operator's options."
    };
}

/// Containers (told to every assistant).
macro_rules! containers {
    () => {
        "- Containers (control.repeat: a loop of steps; control.group) hold a body of nodes: give `scope` (the \
container's id; `outer/inner` for one in another's body) to edit inside it, and edit_container for what the body \
carries, collects or gives."
    };
}

/// What the assistant is told of Terraflux when it connects to `terraflux mcp`.
pub const INSTRUCTIONS: &str = concat!(
    "Terraflux builds and runs models of land use and land cover change, landscape ecology, connectivity and \
conservation planning. A model is a file (.toml or .json) holding a graph of operators; build and change it with \
these tools, not by writing the file.

",
    nodes!(),
    "
- Start from list_templates: a template is a small model that runs as it is (its data drawn by neutral landscape \
models, to be replaced by raster.read nodes of the user's files); new_model with `template` copies one. \
list_operators (with a query) and describe_operator give the operators and their exact ports and settings.
- Every edit is checked: a connection of the wrong type, a port the operator does not have, a cycle, is refused \
with the reason and the file is left as it was. The reply to an edit lists the problems the model still has \
(a model being built has some: inputs not connected yet).
- Files: a raster.read node reads a raster (setting `path`) and gives `raster`; give absolute paths (they are \
stored relative to the model's folder). The model's outputs (set_output) are what run_model writes; \
raster.write and table.write nodes write files too.
- check_model checks the model and its data; run_model runs it and writes its outputs; inspect_file summarises a \
raster (size, classes, statistics) or shows a table.
- open_in_app opens the model in Terraflux's application. The application follows the file: what is edited after \
appears there at once, and the user can undo it (ctrl+Z).
",
    containers!()
);

/// What the assistant in the application is told (the model open there, and the node selected, follow it).
pub const APP_INSTRUCTIONS: &str = concat!(
    "You are the assistant inside Terraflux, an application that builds and runs models of land use and land \
cover change, landscape ecology, connectivity and conservation planning. A model is a graph of operators. Your \
tools read and change the model open in the application: the user sees each change on the canvas at once and can \
undo it (ctrl+Z). Answer in the user's language and briefly; say what you changed and why.

",
    nodes!(),
    "
- Look an operator up (describe_operator, or list_operators with a query) before using it: never guess its ports \
or settings. To build a new model, start from list_templates (new_model with `template` puts one in place of the \
model open, only when the user wants a new model).
- Every edit is checked: a connection of the wrong type, a port the operator does not have, a cycle, is refused \
with the reason and the model is left as it was; read the reason and correct the call. The reply to an edit lists \
the problems the model still has (a model being built has some: inputs not connected yet).
- show_model gives the model as text; check_model checks it and its data; run_model runs it in the application \
(its maps shown there) and summarises what its outputs gave; inspect_file summarises a raster or shows a table.
- Files: a raster.read node reads a raster (setting `path`); give absolute paths.
- Give each node you add a `note`: why it is there.
- Explain the model from its operators' documentation (below, and describe_operator), its nodes' notes and settings, not from memory: an operator's name may mean something else here.
",
    containers!()
);

/// The tools that change a model.
pub const EDITS: [&str; 9] = [
    "set_input",
    "remove_input",
    "add_node",
    "edit_node",
    "remove_node",
    "edit_container",
    "set_output",
    "remove_output",
    "arrange",
];

/// The tools given to a small model (fewer to choose from, a shorter request): to explain a model, change its
/// settings, run it and check it.
pub const FEW: [&str; 7] =
    ["list_operators", "describe_operator", "show_model", "edit_node", "check_model", "run_model", "inspect_file"];

/// The tools, with what they take (as the Model Context Protocol has them: `name`, `description`,
/// `inputSchema`): those of `terraflux mcp`, or (`app`) those of the assistant in the application, which edit the
/// model open there (no `model` to give, no open_in_app; new_model in place of the model open, run_model in the
/// application).
pub fn catalogue(app: bool) -> Vec<Value> {
    let mut tools = all();
    if !app {
        return tools;
    }
    tools.retain(|t| t["name"] != "open_in_app");
    for t in &mut tools {
        if let Some(p) = t["inputSchema"]["properties"].as_object_mut() {
            p.remove("model");
        }
        if let Some(r) = t["inputSchema"]["required"].as_array_mut() {
            r.retain(|x| x != "model");
        }
        match t["name"].as_str() {
            Some("new_model") => {
                t["description"] = "Put a new model in place of the one open in the application: empty, or a copy of a template (the user can undo it). Refused when the model open has nodes, unless `overwrite`.".into();
            }
            Some("run_model") => {
                t["description"] = "Run the model in the application (its maps shown there): how long it took, the nodes that failed, the warnings, and what each of the model's outputs (else its last nodes) gave.".into();
                t["inputSchema"]["properties"] = json!({});
            }
            Some("show_model") => {
                t["description"] = "The model open in the application, as text (TOML), and the problems it has.".into()
            }
            _ => {}
        }
    }
    tools
}

/// The tools as a request to a chat model asks for them (the OpenAI API's `tools`).
pub fn functions(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": { "name": t["name"], "description": t["description"], "parameters": t["inputSchema"] },
            })
        })
        .collect()
}

/// A change of a model by one of the [`EDITS`]: what was done (the model left as it may be half changed when it
/// fails: change a copy).
pub fn edit(name: &str, m: &mut Model, r: &Registry, dir: &Path, a: &Value) -> Reply {
    match name {
        "set_input" => set_input(m, a, dir),
        "remove_input" => {
            let name = text(a, "name")?.trim_start_matches('$');
            m.inputs.remove(name).ok_or_else(|| format!("the model has no input ${name}"))?;
            Ok(format!("removed the input ${name}"))
        }
        "add_node" => add_node(m, r, a),
        "edit_node" => edit_node(m, r, a),
        "remove_node" => {
            let id = text(a, "node")?;
            editor::remove_node(m, &scope(a), id)?;
            Ok(format!("removed {id} (what read it reads nothing now)"))
        }
        "edit_container" => edit_container(m, r, a),
        "set_output" => set_output(m, r, a),
        "remove_output" => {
            let name = text(a, "name")?;
            m.outputs.remove(name).ok_or_else(|| format!("the model has no output {name:?}"))?;
            Ok(format!("removed the output {name}"))
        }
        "arrange" => {
            arrange(&mut m.nodes, r);
            Ok("the nodes put in tidy columns".into())
        }
        _ => Err(format!("no tool {name:?} changes a model")),
    }
}

/// A template's model (by its name), brought up to date and arranged.
pub fn template_model(r: &Registry, name: &str) -> std::result::Result<Model, String> {
    let t = crate::templates::TEMPLATES
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(name.trim()))
        .ok_or_else(|| format!("no template {name:?} (list_templates)"))?;
    let mut m = t.model();
    m.upgrade();
    arrange(&mut m.nodes, r);
    Ok(m)
}

/// The model as text (TOML) and the problems it has.
pub fn show_model(m: &Model, dir: &Path, r: &Registry) -> Reply {
    Ok(format!("{}\n{}", m.to_toml().map_err(|e| e.to_string())?, problems(m, dir, r)))
}

/// The errors and warnings of a model (its submodels expanded, from the model's folder `dir`).
pub fn problems(m: &Model, dir: &Path, r: &Registry) -> String {
    let mut m = m.clone();
    if let Err(e) = terraflux_graph::submodel::expand(&mut m, dir) {
        return format!("The model's submodels do not read: {e}");
    }
    let issues = check(&m, r);
    if issues.is_empty() {
        return "The model has no problems.".into();
    }
    let mut s = String::from("Problems of the model:");
    for i in &issues {
        s.push_str(&format!("\n- {i}"));
    }
    s
}

/// The model and its data checked without running it.
pub fn check_model(r: &Registry, source: &Model, dir: &Path) -> Reply {
    let mut m = source.clone();
    terraflux_graph::submodel::expand(&mut m, dir).map_err(|e| e.to_string())?;
    let issues = check(&m, r);
    let mut lines: Vec<String> = issues.iter().map(|i| i.to_string()).collect();
    let mut errors = issues.iter().any(|i| i.severity == Severity::Error);
    match load_inputs(&m, dir, &BTreeMap::new()) {
        Err(e) => {
            lines.push(format!("error: the inputs: {e}"));
            errors = true;
        }
        Ok(inputs) => {
            for f in terraflux_exec::lint::lint(&m, &inputs) {
                errors |= f.level == terraflux_exec::lint::Level::Error;
                lines.push(f.to_string());
            }
        }
    }
    let verdict = if errors { "The model has errors: it does not run." } else { "The model can run." };
    Ok(if lines.is_empty() { verdict.into() } else { format!("{verdict}\n{}", lines.join("\n")) })
}

/// What a run keeps and tells: its warnings (50 at most), how it goes (to `tell`: each node, and an operator's
/// steps and progress no more than once a second), and whether it is to stop.
#[derive(Default)]
pub struct Watcher {
    pub warnings: Vec<String>,
    pub tell: Option<Box<dyn FnMut(String) + Send>>,
    pub stop: Option<Arc<AtomicBool>>,
    told: Option<Instant>,
}

impl Watcher {
    pub fn new(tell: Option<Box<dyn FnMut(String) + Send>>, stop: Option<Arc<AtomicBool>>) -> Watcher {
        Watcher { tell, stop, ..Default::default() }
    }

    fn say(&mut self, message: String, always: bool) {
        let Some(tell) = self.tell.as_mut() else { return };
        if !always && self.told.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
            return;
        }
        self.told = Some(Instant::now());
        tell(message);
    }
}

impl Observer for Watcher {
    fn node(&mut self, at: &str) {
        self.say(format!("running {at}"), true);
    }

    fn step(&mut self, at: &str, step: usize, of: usize) {
        self.say(format!("{at}: step {step} of {of}"), false);
    }

    fn progress(&mut self, at: &str, p: &terraflux_ops::Progress) {
        self.say(format!("{at}: {}", p.text), false);
    }

    fn warning(&mut self, at: &str, message: &str) {
        if self.warnings.len() < 50 {
            self.warnings.push(format!("{at}: {message}"));
        }
    }

    fn cancelled(&mut self) -> bool {
        self.stop.as_ref().is_some_and(|s| s.load(Ordering::Relaxed))
    }
}

/// Run a model (of the folder `dir`) and write its outputs (`out`, default: the folder `output` in `dir`; `set`:
/// inputs and settings for this run only).
pub fn run_model(engine: &Engine, source: &Model, dir: &Path, a: &Value, watcher: &mut Watcher) -> Reply {
    let mut m = source.clone();
    terraflux_graph::submodel::expand(&mut m, dir).map_err(|e| e.to_string())?;
    // `name`: an input replaced; `node.setting`: a node's setting (as a scenario's)
    let mut overrides = BTreeMap::new();
    let mut settings = BTreeMap::new();
    if let Some(set) = a.get("set").and_then(Value::as_object) {
        for (k, v) in set {
            if k.contains('.') {
                settings.insert(k.clone(), v.clone());
            } else {
                let v = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                overrides.insert(k.trim_start_matches('$').to_string(), v);
            }
        }
    }
    m.apply(&settings).map_err(|e| e.to_string())?;
    let errors: Vec<String> = check(&m, &engine.registry)
        .into_iter()
        .filter(|i| i.severity == Severity::Error)
        .map(|i| i.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(format!("The model has errors: it does not run.\n{}", errors.join("\n")));
    }
    let t = Instant::now();
    let inputs = load_inputs(&m, dir, &overrides).map_err(|e| format!("the inputs: {e}"))?;
    let results = engine.run(&m, &inputs, watcher).map_err(|e| e.to_string())?;
    let out = opt_text(a, "out").map(absolute).unwrap_or_else(|| dir.join("output"));
    let files = write_outputs(&m, &results, &out).map_err(|e| e.to_string())?;
    let mut s = format!("Ran in {:.1} s.", t.elapsed().as_secs_f64());
    if files.is_empty() {
        s.push_str(if m.outputs.is_empty() {
            " The model has no outputs (set_output): nothing written but what its nodes write themselves."
        } else {
            " Nothing written."
        });
    } else {
        s.push_str(" Wrote (inspect_file looks at them):");
        for f in &files {
            s.push_str(&format!("\n- {}", f.display()));
        }
    }
    if !watcher.warnings.is_empty() {
        s.push_str("\nWarnings:");
        for w in &watcher.warnings {
            s.push_str(&format!("\n- {w}"));
        }
    }
    Ok(s)
}

/// What a value an output gave is: a map's size and its classes or statistics, a table's columns and first rows,
/// a number...
pub fn summarise(v: &terraflux_ops::Value) -> String {
    use terraflux_ops::Value as V;
    match v {
        V::Number(x) => format!("{x}"),
        V::Integer(x) => format!("{x}"),
        V::Bool(x) => format!("{x}"),
        V::Text(t) => format!("{t:?}"),
        V::Json(j) => {
            let t = j.to_string();
            if t.chars().count() > 400 { format!("{}...", t.chars().take(400).collect::<String>()) } else { t }
        }
        V::Map(m) => format!("a map of {} x {} cells: {}", m.grid.width, m.grid.height, values_summary(&m.data)),
        V::Stack(s) => {
            let mut t = format!("a stack of {} layers of {} x {} cells", s.layers.len(), s.grid.width, s.grid.height);
            for (k, l) in s.keys.iter().zip(&s.layers).take(6) {
                t.push_str(&format!("\n  {k:?}: {}", values_summary(l)));
            }
            t
        }
        V::Series(items) => match items.last() {
            Some(last) => format!("a series of {} values; the last: {}", items.len(), summarise(last)),
            None => "an empty series".into(),
        },
        V::Table(t) => {
            let mut s = format!("a table of {} rows, columns {}", t.rows.len(), t.columns.join(", "));
            for row in t.rows.iter().take(8) {
                let cells: Vec<String> = row.iter().map(|x| format!("{x}")).collect();
                s.push_str(&format!("\n  {}", cells.join(", ")));
            }
            if t.rows.len() > 8 {
                s.push_str("\n  ...");
            }
            s
        }
        other => format!("a value of type {}", other.ty().name()),
    }
}

/// The cells of each class of a map's values (whole numbers, 30 classes at most), else their statistics; and the
/// cells without a value.
fn values_summary(data: &[f64]) -> String {
    let valid: Vec<f64> = data.iter().copied().filter(|v| !v.is_nan()).collect();
    let empty = data.len() - valid.len();
    if valid.is_empty() {
        return "no values".into();
    }
    let mut classes: BTreeMap<i64, usize> = BTreeMap::new();
    let integers = valid.iter().all(|v| v.fract() == 0.0);
    if integers {
        for v in &valid {
            *classes.entry(*v as i64).or_default() += 1;
            if classes.len() > 30 {
                break;
            }
        }
    }
    let mut s = if integers && classes.len() <= 30 {
        let parts: Vec<String> =
            classes.iter().map(|(c, n)| format!("{c}: {n} ({:.1}%)", 100.0 * *n as f64 / valid.len() as f64)).collect();
        format!("classes (cells): {}", parts.join(", "))
    } else {
        let min = valid.iter().copied().fold(f64::INFINITY, f64::min);
        let max = valid.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mean = valid.iter().sum::<f64>() / valid.len() as f64;
        let sd = (valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / valid.len() as f64).sqrt();
        format!("min {min:.6}, max {max:.6}, mean {mean:.6}, standard deviation {sd:.6}")
    };
    if empty > 0 {
        s.push_str(&format!("; {empty} cells without a value"));
    }
    s
}

/// The tools of `terraflux mcp`, with what they take.
fn all() -> Vec<Value> {
    let model = json!({ "type": "string", "description": "The model's file (.toml or .json), best an absolute path." });
    let scope = json!({
        "type": "string",
        "description": "The container whose body is edited: its id (`outer/inner` for a container in another's body). None: the top level."
    });
    let inputs = json!({
        "type": "object",
        "description": "Port -> source: `node.port`, `$input` or `@carried` (a reference), any other value (a literal), null (disconnected).",
    });
    let settings = json!({
        "type": "object",
        "description": "Setting -> value (JSON: a number, a text, a list, an object); null removes it. See describe_operator.",
    });
    let read_only = json!({ "readOnlyHint": true });
    let tool = |name: &str, description: &str, properties: Value, required: &[&str], annotations: Option<&Value>| {
        let mut t = json!({
            "name": name,
            "description": description,
            "inputSchema": { "type": "object", "properties": properties, "required": required },
        });
        if let Some(a) = annotations {
            t["annotations"] = a.clone();
        }
        t
    };
    vec![
        tool(
            "list_templates",
            "The templates: small models that run as they are, by group, with what each shows. A good start: new_model with `template` copies one.",
            json!({}),
            &[],
            Some(&read_only),
        ),
        tool(
            "list_operators",
            "The operators. Without a query: their names by family. With a query (words all found in the name or the documentation): each one found with what it does.",
            json!({ "query": { "type": "string", "description": "Words to look for, e.g. \"weights of evidence\", \"patch area\"." } }),
            &[],
            Some(&read_only),
        ),
        tool(
            "describe_operator",
            "An operator's documentation: its inputs, outputs and settings (types, choices, which are optional), and a node of a template using it.",
            json!({ "name": { "type": "string", "description": "The operator, e.g. markov.fit." } }),
            &["name"],
            Some(&read_only),
        ),
        tool(
            "new_model",
            "Create a model file: empty, or a copy of a template. A file that exists is replaced only with overwrite.",
            json!({
                "model": model,
                "template": { "type": "string", "description": "A template's name (list_templates)." },
                "name": { "type": "string" },
                "description": { "type": "string" },
                "overwrite": { "type": "boolean" },
            }),
            &["model"],
            None,
        ),
        tool(
            "show_model",
            "The model as text (TOML) and the problems it has.",
            json!({ "model": model }),
            &["model"],
            Some(&read_only),
        ),
        tool(
            "set_input",
            "Add or replace an input of the model (`$name` in the nodes): a band of a raster (map), every band of a raster (stack), or a value. A raster.read node is often simpler for files.",
            json!({
                "model": model,
                "name": { "type": "string" },
                "kind": { "type": "string", "enum": ["map", "stack", "value"] },
                "path": { "type": "string", "description": "map, stack: the raster file." },
                "band": { "description": "map: the band, by name or 0-based index (default: the first)." },
                "bands": { "type": "array", "items": { "type": "string" }, "description": "stack: the bands taken (default: all)." },
                "nodata": { "type": "number" },
                "value": { "description": "value: the value (JSON)." },
                "note": { "type": "string" },
            }),
            &["model", "name", "kind"],
            None,
        ),
        tool(
            "remove_input",
            "Remove an input of the model.",
            json!({ "model": model, "name": { "type": "string" } }),
            &["model", "name"],
            None,
        ),
        tool(
            "add_node",
            "Add a node of an operator, its inputs connected and its settings given; its id (the operator's name with _ for the dot, unless `id` is given) and its outputs. Refused, the file unchanged, if a connection is wrong.",
            json!({
                "model": model,
                "op": { "type": "string" },
                "id": { "type": "string", "description": "The node's id: a letter, then letters, digits, _ or -." },
                "inputs": inputs,
                "settings": settings,
                "note": { "type": "string", "description": "Why the node is there (shown to whoever reads the model)." },
                "scope": scope,
            }),
            &["model", "op"],
            None,
        ),
        tool(
            "edit_node",
            "Change a node: connect or disconnect its inputs, change its settings or its note, rename it (the references to it follow).",
            json!({
                "model": model,
                "node": { "type": "string" },
                "inputs": inputs,
                "settings": settings,
                "note": { "type": "string", "description": "Empty: no note." },
                "rename": { "type": "string" },
                "scope": scope,
            }),
            &["model", "node"],
            None,
        ),
        tool(
            "remove_node",
            "Remove a node; what read it reads nothing.",
            json!({ "model": model, "node": { "type": "string" }, "scope": scope }),
            &["model", "node"],
            None,
        ),
        tool(
            "edit_container",
            "What a container's body gives: `carry` (a loop's value for the next step, read as @name; its first value is the container's input of the same name), `collect` (a loop's value of every step, as a series), `outputs` (a group's outputs), `until` (a loop's end: a yes-or-no value). Each: name -> a reference of the body (null removes it).",
            json!({
                "model": model,
                "container": { "type": "string", "description": "The container's id (`outer/inner` inside another)." },
                "carry": { "type": "object" },
                "collect": { "type": "object" },
                "outputs": { "type": "object" },
                "until": { "type": ["string", "null"] },
            }),
            &["model", "container"],
            None,
        ),
        tool(
            "set_output",
            "Add or replace an output of the model: a node's output written by run_model (a raster as GeoTIFF, a table as CSV, others as JSON).",
            json!({
                "model": model,
                "name": { "type": "string" },
                "from": { "type": "string", "description": "node.port, of a top-level node." },
                "path": { "type": "string", "description": "The file, relative to the output folder (default: the name and the usual extension)." },
                "dtype": { "type": "string", "description": "The cell type of rasters: u8, i16, i32, f32, f64..." },
                "layer": { "description": "One layer of a stack written alone (its name or 0-based number), e.g. `total` of prioritize.evaluate's ferrier." },
            }),
            &["model", "name", "from"],
            None,
        ),
        tool(
            "remove_output",
            "Remove an output of the model.",
            json!({ "model": model, "name": { "type": "string" } }),
            &["model", "name"],
            None,
        ),
        tool(
            "arrange",
            "Put the nodes of the model in tidy columns, by what they read, as the application's Arrange does.",
            json!({ "model": model }),
            &["model"],
            None,
        ),
        tool(
            "check_model",
            "Check the model and its data without running it: the errors and warnings of the graph, and what was found in the data.",
            json!({ "model": model }),
            &["model"],
            Some(&read_only),
        ),
        tool(
            "run_model",
            "Run the model and write its outputs (default: the folder `output` beside the model); the files written, the warnings, how long it took.",
            json!({
                "model": model,
                "out": { "type": "string", "description": "The folder of the outputs." },
                "set": { "type": "object", "description": "For this run only: input -> value or file (`seed`), node.setting -> value (`patches.isometry`)." },
            }),
            &["model"],
            None,
        ),
        tool(
            "inspect_file",
            "Look at a file: a raster's size, georeference and, band by band, its statistics or the cells of each class; the first lines of a table or text.",
            json!({ "path": { "type": "string" } }),
            &["path"],
            Some(&read_only),
        ),
        tool(
            "open_in_app",
            "Open the model in Terraflux's application, for the user to see it, run it and change it. The application follows the file: later edits appear there as they are made.",
            json!({ "model": model }),
            &["model"],
            None,
        ),
    ]
}

pub fn text<'a>(a: &'a Value, key: &str) -> std::result::Result<&'a str, String> {
    a.get(key).and_then(Value::as_str).ok_or_else(|| format!("give `{key}` (a text)"))
}

pub fn opt_text<'a>(a: &'a Value, key: &str) -> Option<&'a str> {
    a.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty())
}

pub fn absolute(p: &str) -> PathBuf {
    let p = PathBuf::from(p);
    if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) }
}

/// The path of containers' ids of `scope` (none: the top level).
fn scope(a: &Value) -> Vec<String> {
    opt_text(a, "scope").map(|s| s.split('/').filter(|x| !x.is_empty()).map(String::from).collect()).unwrap_or_default()
}

pub fn list_templates() -> String {
    let mut s = String::new();
    for g in crate::templates::GROUPS {
        s.push_str(&format!("{g}\n"));
        for t in crate::templates::TEMPLATES.iter().filter(|t| t.group == g) {
            s.push_str(&format!("  {}: {}\n", t.name, t.about));
        }
    }
    s
}

/// The first sentence of a documentation.
pub fn first_sentence(doc: &str) -> String {
    let end = doc.find(". ").map_or(doc.len(), |i| i + 1);
    let s = &doc[..end];
    if s.chars().count() > 220 { format!("{}...", s.chars().take(220).collect::<String>()) } else { s.to_string() }
}

pub fn list_operators(r: &Registry, query: Option<&str>) -> String {
    let mut sigs = r.signatures();
    sigs.sort_by(|a, b| a.name.cmp(&b.name));
    match query {
        None => {
            let mut families: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
            for s in &sigs {
                let (family, rest) = s.name.split_once('.').unwrap_or(("", &s.name));
                families.entry(family).or_default().push(rest);
            }
            let mut out = format!(
                "{} operators, by family (family.name); list_operators with a query finds them by what they do.\n",
                sigs.len()
            );
            for (f, names) in families {
                out.push_str(&format!("{f}: {}\n", names.join(", ")));
            }
            out
        }
        Some(q) => {
            let words: Vec<String> = q.to_lowercase().split_whitespace().map(String::from).collect();
            let found: Vec<&OpSignature> = sigs
                .iter()
                .filter(|s| {
                    let hay = format!("{} {}", s.name, s.doc).to_lowercase();
                    words.iter().all(|w| hay.contains(w.as_str()))
                })
                .collect();
            if found.is_empty() {
                return format!("No operator has all of {q:?}: try fewer or other words.");
            }
            let mut out = String::new();
            for s in found.iter().take(60) {
                out.push_str(&format!("{}: {}\n", s.name, first_sentence(&s.doc)));
            }
            if found.len() > 60 {
                out.push_str(&format!("... and {} more: give more words.\n", found.len() - 60));
            }
            out
        }
    }
}

fn port_line(p: &PortSpec) -> String {
    let mut t = format!("  {} ({}{})", p.name, p.ty.name(), if p.required { "" } else { ", optional" });
    if !p.choices.is_empty() {
        t.push_str(&format!(", one of: {}", p.choices.join(", ")));
    }
    if !p.doc.is_empty() {
        t.push_str(&format!(": {}", p.doc));
    }
    t
}

/// The operators whose names are like a name not found.
fn like(r: &Registry, name: &str) -> String {
    let key = name.rsplit('.').next().unwrap_or(name).to_lowercase();
    let near: Vec<String> = r
        .signatures()
        .into_iter()
        .map(|s| s.name)
        .filter(|n| !key.is_empty() && n.to_lowercase().contains(&key))
        .take(12)
        .collect();
    if near.is_empty() { String::new() } else { format!(" (like it: {})", near.join(", ")) }
}

pub fn describe_operator(r: &Registry, name: &str) -> Reply {
    let sig = r.sig(name).ok_or_else(|| format!("no operator {name:?}{}", like(r, name)))?;
    let mut s = format!("{}\n{}\n", sig.name, sig.doc);
    if sig.container {
        s.push_str(
            "\nA container: it holds a body of nodes (edit them with scope = this node's id; edit_container for what \
             the body carries, collects or gives).\n",
        );
    }
    for (title, ports) in [("Inputs", &sig.inputs), ("Outputs", &sig.outputs), ("Settings", &sig.params)] {
        if !ports.is_empty() {
            s.push_str(&format!("\n{title}:\n"));
            for p in ports.iter() {
                s.push_str(&port_line(p));
                s.push('\n');
            }
        }
    }
    if let Some(t) = sig.extra {
        s.push_str(&format!("\nIt also takes inputs of any other name, of type {}.\n", t.name()));
    }
    if let Some((template, node)) = example(name) {
        let mut m = Model::new("example");
        m.nodes.push(node);
        let toml = m.to_toml().unwrap_or_default();
        let body: Vec<&str> = toml.lines().filter(|l| !l.starts_with("format =") && !l.starts_with("name =")).collect();
        s.push_str(&format!("\nA node of it in the template {template:?}:\n{}\n", body.join("\n").trim()));
    }
    Ok(s)
}

/// A node of an operator in a template (the first found), and the template's name.
fn example(op: &str) -> Option<(&'static str, Node)> {
    fn find(nodes: &[Node], op: &str) -> Option<Node> {
        nodes.iter().find_map(|n| {
            if n.op == op {
                let mut c = n.clone();
                c.pos = None;
                return Some(c);
            }
            find(&n.body.as_ref()?.nodes, op)
        })
    }
    crate::templates::TEMPLATES.iter().find_map(|t| Some((t.name, find(&t.model().nodes, op)?)))
}

fn set_input(m: &mut Model, a: &Value, dir: &Path) -> Reply {
    let name = text(a, "name")?.trim_start_matches('$');
    if !valid_id(name) {
        return Err(format!("{name:?} is not a valid name (a letter, then letters, digits, _ or -)"));
    }
    let note = opt_text(a, "note").map(String::from);
    let nodata = a.get("nodata").and_then(Value::as_f64);
    let as_text = |v: &Value| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let mut warning = String::new();
    let mut file = || -> std::result::Result<String, String> {
        let p = absolute(text(a, "path")?);
        if !p.exists() {
            warning = format!(" (no file at {} yet)", p.display());
        }
        Ok(terraflux_graph::relative_path(&p, dir))
    };
    let input = match text(a, "kind")? {
        "map" => Input::Map { path: file()?, band: a.get("band").map(as_text), nodata, note },
        "stack" => Input::Stack {
            path: file()?,
            bands: a.get("bands").and_then(Value::as_array).map(|b| b.iter().map(as_text).collect()),
            nodata,
            note,
        },
        "value" => Input::Value { value: a.get("value").cloned().ok_or("give `value`")?, note },
        k => return Err(format!("{k:?}: the kind is map, stack or value")),
    };
    m.inputs.insert(name.to_string(), input);
    Ok(format!("the input ${name} set{warning}"))
}

/// Whether a text given for an input is a reference (`$input`, `@carried`, `node.port`), not a literal.
fn is_reference(s: &str) -> bool {
    Ref::parse(s).is_ok()
}

fn set_inputs(m: &mut Model, r: &Registry, path: &[String], id: &str, inputs: Option<&Value>) -> Reply {
    let Some(v) = inputs else { return Ok(String::new()) };
    let map = v.as_object().ok_or("`inputs` is an object: port -> source")?;
    for (port, src) in map {
        let done = match src {
            Value::Null => editor::disconnect(m, path, id, port),
            Value::String(s) if is_reference(s) => editor::connect(m, r, path, s, id, port),
            other => editor::set_literal(m, r, path, id, port, &other.to_string()),
        };
        done.map_err(|e| format!("{id}.{port}: {e}"))?;
    }
    Ok(String::new())
}

fn set_settings(m: &mut Model, r: &Registry, path: &[String], id: &str, settings: Option<&Value>) -> Reply {
    let Some(v) = settings else { return Ok(String::new()) };
    let map = v.as_object().ok_or("`settings` is an object: setting -> value")?;
    let op = m.scope(path).and_then(|n| n.iter().find(|n| n.id == id)).map(|n| n.op.clone()).ok_or("no such node")?;
    let sig = r.sig(&op).cloned();
    for (name, value) in map {
        // (a literal input given as a setting: set as the input it is)
        if sig.as_ref().is_some_and(|s| s.input_spec(name).is_some()) {
            set_inputs(m, r, path, id, Some(&json!({ name: value })))?;
            continue;
        }
        let n = m.scope_mut(path).and_then(|n| n.iter_mut().find(|n| n.id == id)).ok_or("no such node")?;
        if let Some(sig) = &sig
            && !sig.container
            && !sig.params.iter().any(|p| &p.name == name)
            && !n.params.contains_key(name)
        {
            let known: Vec<&str> = sig.params.iter().map(|p| p.name.as_str()).collect();
            return Err(format!("{op} has no setting {name:?} (its settings: {})", known.join(", ")));
        }
        if value.is_null() {
            n.params.remove(name);
        } else {
            n.params.insert(name.clone(), value.clone());
        }
    }
    Ok(String::new())
}

/// A place on the canvas for a node added: none when no node of its scope has one (the application lays them out
/// by itself), else to the right of what it reads, out of the way of the others.
fn place(m: &mut Model, path: &[String], id: &str) {
    let Some(nodes) = m.scope(path) else { return };
    let others: Vec<&Node> = nodes.iter().filter(|n| n.id != id).collect();
    let pos = if others.iter().all(|n| n.pos.is_none()) {
        None
    } else {
        let auto = editor_view::auto_positions(nodes);
        let at = |n: &Node| n.pos.unwrap_or(auto[&n.id]);
        let node = nodes.iter().find(|n| n.id == id).expect("the node added");
        let width = |n: &Node| editor_view::node_width(n, editor_view::standard());
        // what it reads: where each is, and its right edge
        let deps: Vec<[f32; 2]> = terraflux_graph::dependencies(node, nodes)
            .iter()
            .filter_map(|d| others.iter().find(|n| &n.id == d))
            .map(|n| [at(n)[0] + width(n), at(n)[1]])
            .collect();
        let mut p = if deps.is_empty() {
            let x = others.iter().map(|n| at(n)[0]).fold(f32::INFINITY, f32::min);
            let y = others.iter().map(|n| at(n)[1]).fold(f32::NEG_INFINITY, f32::max);
            [x, y + 160.0]
        } else {
            let x = deps.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
            let y = deps.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
            [x + 60.0, y]
        };
        let own = width(node);
        // out of the way of the boxes there (theirs or its own width apart, as each is left or right of it)
        while others.iter().any(|n| {
            let q = at(n);
            let apart = if q[0] < p[0] { width(n) } else { own };
            (q[0] - p[0]).abs() < apart + 20.0 && (q[1] - p[1]).abs() < 110.0
        }) {
            p[1] += 130.0;
        }
        Some(p)
    };
    if let Some(n) = m.scope_mut(path).and_then(|n| n.iter_mut().find(|n| n.id == id)) {
        n.pos = pos;
    }
}

/// The output ports of a node, as a text.
fn outputs_of(m: &Model, r: &Registry, path: &[String], id: &str) -> String {
    let Some(n) = m.scope(path).and_then(|n| n.iter().find(|n| n.id == id)) else { return String::new() };
    let ports: Vec<String> =
        editor::output_ports(n, r).into_iter().map(|(p, t)| format!("{id}.{p} ({})", t.name())).collect();
    if ports.is_empty() { "no outputs yet".into() } else { format!("its outputs: {}", ports.join(", ")) }
}

fn add_node(m: &mut Model, r: &Registry, a: &Value) -> Reply {
    let op = text(a, "op")?;
    let path = scope(a);
    if r.sig(op).is_none() {
        return Err(format!("no operator {op:?}{}", like(r, op)));
    }
    let mut id = editor::add_node(m, r, &path, op, [0.0, 0.0])?;
    if let Some(want) = opt_text(a, "id") {
        editor::rename_node(m, &path, &id, want)?;
        id = want.to_string();
    }
    if let Some(note) = opt_text(a, "note") {
        editor::set_note(m, &path, &id, note)?;
    }
    set_settings(m, r, &path, &id, a.get("settings"))?;
    set_inputs(m, r, &path, &id, a.get("inputs"))?;
    place(m, &path, &id);
    Ok(format!("added {id} ({op}); {}", outputs_of(m, r, &path, &id)))
}

fn edit_node(m: &mut Model, r: &Registry, a: &Value) -> Reply {
    let mut id = text(a, "node")?.to_string();
    let path = scope(a);
    if m.scope(&path).is_none_or(|n| !n.iter().any(|n| n.id == id)) {
        return Err(format!(
            "no node {id:?} in {}",
            if path.is_empty() { "the top level".into() } else { path.join("/") }
        ));
    }
    set_settings(m, r, &path, &id, a.get("settings"))?;
    set_inputs(m, r, &path, &id, a.get("inputs"))?;
    if let Some(note) = a.get("note").and_then(Value::as_str) {
        editor::set_note(m, &path, &id, note)?;
    }
    if let Some(new) = opt_text(a, "rename") {
        editor::rename_node(m, &path, &id, new)?;
        id = new.to_string();
    }
    Ok(format!("changed {id}; {}", outputs_of(m, r, &path, &id)))
}

fn edit_container(m: &mut Model, r: &Registry, a: &Value) -> Reply {
    let at = text(a, "container")?;
    let path: Vec<String> = at.split('/').filter(|x| !x.is_empty()).map(String::from).collect();
    if m.node_mut(at).is_none_or(|n| n.body.is_none()) {
        return Err(format!("no container {at:?}"));
    }
    for which in ["carry", "collect", "outputs"] {
        let Some(map) = a.get(which).and_then(Value::as_object) else { continue };
        for (name, v) in map {
            match v {
                Value::Null => {
                    let b = m.node_mut(at).and_then(|n| n.body.as_mut()).expect("the container");
                    match which {
                        "carry" => b.carry.remove(name),
                        "collect" => b.collect.remove(name),
                        _ => b.outputs.remove(name),
                    };
                }
                Value::String(reference) => editor::set_body_map(m, r, &path, which, name, reference)
                    .map_err(|e| format!("{which}.{name}: {e}"))?,
                _ => return Err(format!("{which}.{name}: give a reference of the body, or null")),
            }
        }
    }
    if let Some(u) = a.get("until") {
        let until = match u {
            Value::Null => None,
            Value::String(s) => {
                Ref::parse(s).map_err(|e| e.to_string())?;
                Some(s.clone())
            }
            _ => return Err("until: a reference of the body, or null".into()),
        };
        m.node_mut(at).and_then(|n| n.body.as_mut()).expect("the container").until = until;
    }
    Ok(format!("changed the container {at}; {}", outputs_of(m, r, &path[..path.len() - 1], &path[path.len() - 1])))
}

fn set_output(m: &mut Model, r: &Registry, a: &Value) -> Reply {
    let name = text(a, "name")?;
    if !valid_id(name) {
        return Err(format!("{name:?} is not a valid name (a letter, then letters, digits, _ or -)"));
    }
    let from = text(a, "from")?;
    let Ok(Ref::Port { node, port }) = Ref::parse(from) else {
        return Err(format!("{from:?}: give node.port, of a top-level node"));
    };
    let n = m.node(&node).ok_or_else(|| format!("no node {node:?} at the top level"))?;
    let ports = editor::output_ports(n, r);
    if !ports.iter().any(|(p, _)| *p == port) {
        let names: Vec<&str> = ports.iter().map(|(p, _)| p.as_str()).collect();
        return Err(format!("{node} has no output {port:?} (its outputs: {})", names.join(", ")));
    }
    let dtype = opt_text(a, "dtype").map(String::from);
    if let Some(d) = &dtype
        && terraflux_raster::DataType::parse(d).is_none()
    {
        return Err(format!("{d:?} is not a cell type (u8, i16, u16, i32, u32, f32, f64)"));
    }
    let layer = match a.get("layer") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(v) => Some(v.to_string()),
    };
    let what = match &layer {
        Some(l) => format!("the layer {l:?} of {from}"),
        None => from.to_string(),
    };
    m.outputs.insert(
        name.to_string(),
        Output { from: from.to_string(), path: opt_text(a, "path").map(String::from), dtype, layer },
    );
    Ok(format!("the output {name} writes {what}"))
}

/// Every scope's nodes in tidy columns.
pub fn arrange(nodes: &mut [Node], r: &Registry) {
    let places = editor_view::arrange(nodes, r);
    for n in nodes.iter_mut() {
        if let Some(p) = places.get(&n.id) {
            n.pos = Some(*p);
        }
        if let Some(b) = &mut n.body {
            arrange(&mut b.nodes, r);
        }
    }
}

pub fn inspect_file(a: &Value) -> Reply {
    let p = absolute(text(a, "path")?);
    if !p.exists() {
        return Err(format!("{}: no such file", p.display()));
    }
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let texts = ["csv", "tsv", "txt", "json", "toml", "md", "geojson", "log", "yaml", "yml"];
    if p.is_file() && texts.contains(&ext.as_str()) {
        return text_head(&p);
    }
    match raster_summary(&p) {
        Ok(s) => Ok(s),
        Err(e) => Err(format!("{}: not a raster Terraflux reads ({e})", p.display())),
    }
}

/// The first lines of a text file.
fn text_head(p: &Path) -> Reply {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let lines: Vec<&str> = text.lines().collect();
    let mut shown = String::new();
    for l in lines.iter().take(200) {
        if shown.len() + l.len() > 20_000 {
            break;
        }
        shown.push_str(l);
        shown.push('\n');
    }
    let more = lines.len().saturating_sub(shown.lines().count());
    if more > 0 {
        shown.push_str(&format!("... and {more} lines more\n"));
    }
    Ok(format!("{} ({} lines)\n{shown}", p.display(), lines.len()))
}

/// A raster's size, georeference and, for each band, its statistics or the cells of each class.
fn raster_summary(p: &Path) -> Reply {
    let r = terraflux_ops::files::Rasters::open(&p.to_string_lossy(), None, None).map_err(|e| e.to_string())?;
    let g = &r.grid;
    let (dx, dy) = g.resolution();
    let mut s = format!(
        "{}: {} columns x {} rows, cells of {dx} x {dy}, upper-left corner ({}, {}){}; {} band(s)\n",
        p.display(),
        g.width,
        g.height,
        g.transform[0],
        g.transform[3],
        g.geo.epsg().map(|e| format!(", EPSG:{e}")).unwrap_or_default(),
        r.layers.len()
    );
    for (k, layer) in r.layers.iter().enumerate().take(12) {
        s.push_str(&format!("band {k} ({}): ", layer.name));
        let data = match r.read(k) {
            Ok(d) => d,
            Err(e) => {
                s.push_str(&format!("not read: {e}\n"));
                continue;
            }
        };
        s.push_str(&values_summary(&data));
        s.push('\n');
    }
    if r.layers.len() > 12 {
        s.push_str(&format!("... and {} bands more\n", r.layers.len() - 12));
    }
    Ok(s)
}
