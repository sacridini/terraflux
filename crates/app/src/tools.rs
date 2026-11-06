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
use terraflux_graph::{Input, Model, Node, Output, PortSpec, Ref, Severity, check, valid_id};
use terraflux_ops::Registry;
use terraflux_ops::{algebra_tools, vector_algebra};
use terraflux_raster::vector::Column;

use crate::{editor, editor_view};

pub type Reply = std::result::Result<String, String>;

/// What a node is and reads (told to every assistant).
macro_rules! nodes {
    () => {
        "- A node runs one operator (`op`) and gives outputs (its ports). An input of a node reads `node.port` \
(another node's output), `$name` (an input of the model), `@name` (inside a container: a value carried along a \
loop, `@step`, a group's input) or is a literal value. Settings are the operator's options: a value, or a value read when the model runs (`\"$name\"`, `\"@step\"`, or `{\"from\": \"node.port\"}` for a node's number or text): a threshold that changes with the loop's step, a submodel's setting given as its input."
    };
}

/// The calculators' scripts (told to every assistant).
macro_rules! calc {
    () => {
        "- write_calc writes a calculator's script: of maps (raster.calc), of features' attributes (vector.calc: \
`out name = value` for each attribute, texts compared as uso == \"forest\", the geometry as geometry.area) or of the \
features kept (vector.filter: the script ends with the condition); checked, and tried on a sample of its inputs' \
cells or features, before anything changes; read what each output gives in its reply. explain_calc takes a script \
apart: what each input is and what each name holds on a sample. Use them, not add_node or edit_node, for a \
calculator's script."
    };
}

/// Containers (told to every assistant).
macro_rules! containers {
    () => {
        "- Containers (control.repeat: a loop of steps; control.each: the body once per element of a collection, @item, @index and @name inside, what it collects gathered; control.switch: a group per branch in its body, the \
one named as its case runs; control.group) hold a body of nodes: give `scope` (the \
container's id; `outer/inner` for one in another's body) to edit inside it, and edit_container for what the body \
carries, collects or gives.
- A model is read in stages: once it is built, put its top level in a group per stage (group_by_stage: data, \
preparation, model, validation, results, each with a note; or group_nodes for groups of your own, each with a \
note of what it is for). The application then shows a block per stage, and the model's document a section per \
group.
- The values a user is to change (a rate, a threshold, a demand) go in the model's form of parameters \
(set_parameter: a label, a unit, a range and a help each), and named sets of them in scenarios (set_scenario).
- Why a cell of a result has its value: trace_cell gives the chain of values that made it (expressions with the \
values in place, the cells around, what a model weighs, a loop step by step). Explain a result from it, not from \
what the model should do.
- When a run fails, its answer says which node failed, its message and what it was given side by side (each \
input's grid: size, cells, coordinate system) with what likely does not fit: read them before changing anything, \
look at the inputs it names (inspect_file, describe_operator), and put right what does not fit (raster.match for \
maps of different grids, raster.reproject for other coordinate systems), then run again."
    };
}

/// What the assistant is told of Terraflux when it connects to `terraflux mcp`: short, since clients cut long
/// instructions (Claude Code's at about 4000 characters); the details are in the guides, read on demand
/// ([`crate::guide`], the tool `guide`).
pub const INSTRUCTIONS: &str = "Terraflux builds and runs models of land use and land cover change, landscape \
ecology, connectivity and conservation planning. A model is a file (.toml or .json) holding a graph of operators; \
build and change it with these tools, not by writing the file.

- Read the guide of the field first (guide with `topic`): land-change, connectivity, landscape-metrics, \
conservation-planning, species-distribution, data-preparation give the method step by step, with its operators \
and templates; nodes, calc, containers, record and results tell how a model is built.
- Start from a template (list_templates with a `query`; new_model with `template`): a small model that runs as it \
is, its data drawn by neutral landscape models (nlm.*), to be replaced by raster.read nodes of the user's files \
(absolute paths).
- Operators: list_operators with a query (English or Portuguese) finds them; describe_operator gives the exact \
ports and settings, advice, and what the operator usually reads and gives to. Never guess a port or a setting.
- A node's input reads `node.port`, `$name` (a model's input) or `@name` (inside a container); a setting is a \
value or `\"$name\"`. Every edit is checked: a wrong one is refused with the reason and the file left as it was; \
the reply lists the problems left.
- Calculators (raster.calc, vector.calc, vector.filter) are written with write_calc, in Python's syntax: one \
statement a line, `out name = value` for each output (guide calc). Containers (control.repeat, control.each, \
control.switch, control.group) hold a body: edit inside one with `scope` (guide containers).
- Record each choice: a node's `note` and `why` (and `refs`, `assumptions`, `alternatives`), the model's \
`request`, `summary` and `author` (set_model_info).
- check_model checks the model and its data and advises on the method: follow its warnings. run_model runs it \
and writes its outputs; a failed run says what does not fit: read it before changing anything. inspect_file looks \
at a file, trace_cell explains a cell, methods drafts a paper's methods section, export_plot writes a Plot as a \
PNG or a GeoTIFF.
- open_in_app opens the model in Terraflux's application, which follows the file (the user can undo each edit).
";

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
- guide gives the method of each field step by step, with its operators and templates (land-change, \
connectivity, landscape-metrics, conservation-planning, species-distribution, data-preparation): read the field's \
before building or reviewing a model of it. check_model advises on the method too: follow its warnings.
- Every edit is checked: a connection of the wrong type, a port the operator does not have, a cycle, is refused \
with the reason and the model is left as it was; read the reason and correct the call. The reply to an edit lists \
the problems the model still has (a model being built has some: inputs not connected yet).
- show_model gives the model as text; check_model checks it and its data; run_model runs it in the application \
(its maps shown there) and summarises what its outputs gave; inspect_file summarises a raster or shows a table; \
methods drafts the methods section of a paper from the model, with the papers to cite (`language`: en or pt).
- Files: a raster.read node reads a raster (setting `path`); give absolute paths.
",
    calc!(),
    "
- The model is the record of its choices: give each node you add a `note` (what it is for) and a `why` (why \
this operator and these settings), and `refs`, `assumptions` and `alternatives` when there are. When you have \
built or changed the model, give it a `summary` of what was asked and done (set_model_info).
- Explain the model from its operators' documentation (below, and describe_operator), its nodes' notes and settings, not from memory: an operator's name may mean something else here.
",
    containers!()
);

/// The tools that change a model.
pub const EDITS: [&str; 24] = [
    "set_model_info",
    "set_input",
    "remove_input",
    "add_node",
    "edit_node",
    "remove_node",
    "edit_container",
    "set_output",
    "remove_output",
    "set_plot",
    "remove_plot",
    "set_note",
    "remove_note",
    "locate_files",
    "arrange",
    "group_nodes",
    "ungroup",
    "group_by_stage",
    "set_parameter",
    "remove_parameter",
    "set_scenario",
    "remove_scenario",
    "write_calc",
    "save_my_node",
];

/// The tools given to a small model (fewer to choose from, a shorter request): to explain a model, change its
/// settings, run it and check it, write and explain a calculator's script.
pub const FEW: [&str; 9] = [
    "list_operators",
    "describe_operator",
    "show_model",
    "edit_node",
    "check_model",
    "run_model",
    "inspect_file",
    "write_calc",
    "explain_calc",
];

/// The tools, with what they take (as the Model Context Protocol has them: `name`, `description`,
/// `inputSchema`): those of `terraflux mcp`, or (`app`) those of the assistant in the application, which edit the
/// model open there (no `model` to give, no open_in_app; new_model in place of the model open, run_model in the
/// application).
pub fn catalogue(app: bool) -> Vec<Value> {
    let mut tools = all();
    if !app {
        return tools;
    }
    // (export_plot runs the model on its own: the application's assistant exports from the Viewer)
    tools.retain(|t| t["name"] != "open_in_app" && t["name"] != "export_plot");
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
            Some("sensitivity") => {
                t["description"] = "Set up a sensitivity analysis of the model open: the factors (with their ranges) and the settings are put in Many runs > Sensitivity, and the user presses Run (it runs there, with the workers of the application, and writes its charts, tables and methods.md). Morris first to screen many factors, then Sobol on the few that matter; replicates (3 to 5) for a model that draws at random. The reply says the runs it will take.".into();
                if let Some(p) = t["inputSchema"]["properties"].as_object_mut() {
                    p.remove("out");
                    p.remove("confirm");
                }
            }
            _ => {}
        }
    }
    tools
}

/// The schema of what a tool answers as data (MCP's `outputSchema`, 2025-06-18), for those that give it: the
/// check's problems and advice, a run's files and warnings.
pub fn output_schema(name: &str) -> Option<Value> {
    let list = json!({ "type": "array", "items": { "type": "string" } });
    match name {
        "check_model" => Some(json!({
            "type": "object",
            "properties": {
                "runs": { "type": "boolean", "description": "whether the model can run (no errors)" },
                "errors": list, "warnings": list,
                "advice": { "type": "array", "description": "advice on the method and the record", "items": { "type": "string" } },
            },
            "required": ["runs", "errors", "warnings", "advice"],
        })),
        "run_model" => Some(json!({
            "type": "object",
            "properties": {
                "ran": { "type": "boolean" },
                "seconds": { "type": "number" },
                "files": { "type": "array", "description": "the files written", "items": { "type": "string" } },
                "warnings": list,
                "advice": list,
                "error": { "type": "string", "description": "why it did not run" },
            },
            "required": ["ran"],
        })),
        _ => None,
    }
}

/// What a tool answered, as the data of its [`output_schema`] (none for the tools without one).
pub fn structured(name: &str, text: &str, error: bool) -> Option<Value> {
    let lines = || text.lines().map(str::trim);
    let after = |prefix: &str| -> Vec<String> {
        lines().filter_map(|l| l.strip_prefix(prefix)).map(|l| l.trim().to_string()).collect()
    };
    match name {
        "check_model" => {
            let runs = !error && text.starts_with("The model can run.");
            Some(json!({
                "runs": runs,
                "errors": after("error:"),
                "warnings": after("warning:"),
                "advice": after("advice:"),
            }))
        }
        "run_model" => {
            if error {
                return Some(json!({ "ran": false, "error": text }));
            }
            let seconds =
                text.strip_prefix("Ran in ").and_then(|r| r.split(' ').next()).and_then(|s| s.parse::<f64>().ok());
            // the lists after their headings, a "- " line each
            let mut files = Vec::new();
            let mut warnings = Vec::new();
            let mut advice = Vec::new();
            let mut list = 0;
            for l in lines() {
                if l.contains("Wrote (") {
                    list = 1;
                } else if l == "Warnings:" {
                    list = 2;
                } else if l.starts_with("On the method") {
                    list = 3;
                } else if let Some(item) = l.strip_prefix("- ") {
                    match list {
                        1 => files.push(item.to_string()),
                        2 => warnings.push(item.to_string()),
                        3 => advice.push(item.to_string()),
                        _ => {}
                    }
                } else {
                    list = 0;
                }
            }
            let mut v = json!({ "ran": true, "files": files, "warnings": warnings, "advice": advice });
            if let Some(s) = seconds {
                v["seconds"] = json!(s);
            }
            Some(v)
        }
        _ => None,
    }
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
        "set_model_info" => {
            let set = model_info(m, a);
            Ok(if set.is_empty() {
                "nothing given to set".into()
            } else {
                format!("set the model's {}", set.join(", "))
            })
        }
        "set_input" => set_input(m, a, dir),
        "remove_input" => {
            let name = text(a, "name")?.trim_start_matches('$');
            editor::remove_input(m, name)?;
            Ok(format!("removed the input ${name} (what read it reads nothing now)"))
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
        "set_plot" => set_plot(m, r, a),
        "remove_plot" => {
            let id = text(a, "id")?;
            let before = m.plots.len();
            m.plots.retain(|p| p.id != id);
            if m.plots.len() == before {
                return Err(format!("the model has no plot {id:?}"));
            }
            Ok(format!("removed the plot {id}"))
        }
        "set_note" => set_note(m, a),
        "locate_files" => {
            let folders: Vec<std::path::PathBuf> = a
                .get("folders")
                .and_then(Value::as_array)
                .map(|v| v.iter().filter_map(Value::as_str).map(|f| dir.join(f)).collect())
                .unwrap_or_default();
            if let Some(gone) = folders.iter().find(|f| !f.is_dir()) {
                return Err(format!("no folder {}", gone.display()));
            }
            let done = crate::relink::relink_model(m, dir, &folders);
            Ok(done.words(dir))
        }
        "remove_note" => {
            let id = text(a, "id")?;
            let before = m.notes.len();
            m.notes.retain(|n| n.id != id);
            if m.notes.len() == before {
                return Err(format!("the model has no note {id:?}"));
            }
            Ok(format!("removed the note {id}"))
        }
        "remove_output" => {
            let name = text(a, "name")?;
            m.outputs.remove(name).ok_or_else(|| format!("the model has no output {name:?}"))?;
            Ok(format!("removed the output {name}"))
        }
        "arrange" => {
            arrange(&mut m.nodes, r);
            Ok("the nodes put in tidy columns".into())
        }
        "group_nodes" => {
            let ids: Vec<String> = a
                .get("nodes")
                .and_then(Value::as_array)
                .ok_or("give `nodes`: the ids of the nodes to group")?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let id = text(a, "id")?;
            editor::group_nodes(m, r, &scope(a), &ids, id, opt_text(a, "note"))?;
            if let Some(why) = opt_text(a, "why") {
                editor::set_why(m, &scope(a), id, why)?;
            }
            let outs = m.scope(&scope(a)).and_then(|n| n.iter().find(|n| n.id == id)).and_then(|g| g.body.as_ref());
            let outs: Vec<String> =
                outs.map(|b| b.outputs.keys().map(|k| format!("{id}.{k}")).collect()).unwrap_or_default();
            Ok(format!(
                "grouped {} into {id}; what read them from outside reads {}",
                ids.join(", "),
                if outs.is_empty() { "nothing".to_string() } else { outs.join(", ") }
            ))
        }
        "ungroup" => {
            let id = text(a, "node")?;
            editor::ungroup(m, r, &scope(a), id)?;
            Ok(format!("took the nodes of {id} out of it"))
        }
        "group_by_stage" => {
            let made = crate::document::group_by_stage(m, r)?;
            Ok(format!("grouped the top level by stage: {}", made.join(", ")))
        }
        "set_parameter" => set_parameter(m, r, a),
        "remove_parameter" => {
            let key = text(a, "key")?;
            let before = m.parameters.len();
            m.parameters.retain(|p| p.key != key);
            if m.parameters.len() == before {
                return Err(format!("no parameter {key:?}"));
            }
            Ok(format!("took {key} out of the parameters (the model keeps its value)"))
        }
        "set_scenario" => set_scenario(m, a),
        // (the inputs' values not known: rasters read from their files)
        "write_calc" => write_calc(m, r, dir, a, &|_| None),
        // (the model's types known from its graph and the headers of its files only: it has not run here)
        "save_my_node" => {
            let mut s = crate::app::Session::from_model(m.clone())?;
            let done = save_my_node(&mut s, r, a)?;
            *m = s.source;
            Ok(done)
        }
        "remove_scenario" => {
            let name = text(a, "name")?;
            let before = m.scenarios.len();
            m.scenarios.retain(|s| s.name != name);
            if m.scenarios.len() == before {
                return Err(format!("no scenario {name:?}"));
            }
            Ok(format!("removed the scenario {name}"))
        }
        _ => Err(format!("no tool {name:?} changes a model")),
    }
}

/// Add a parameter to the model's form, or change one: what it changes (`$input`, `node.setting`), its label,
/// unit, range, step, choices and help; `value` sets its value too.
fn set_parameter(m: &mut Model, r: &Registry, a: &Value) -> Reply {
    let key = text(a, "key")?.to_string();
    let mut p = match m.parameters.iter().find(|p| p.key == key) {
        Some(p) => p.clone(),
        None => crate::params_view::parameter(m, r, &key),
    };
    if let Some(t) = opt_text(a, "label") {
        p.label = Some(t.to_string());
    }
    if let Some(t) = opt_text(a, "unit") {
        p.unit = Some(t.to_string());
    }
    if let Some(t) = opt_text(a, "help") {
        p.help = Some(t.to_string());
    }
    for (name, slot) in [("min", &mut p.min), ("max", &mut p.max), ("step", &mut p.step)] {
        if let Some(x) = a.get(name).and_then(Value::as_f64) {
            *slot = Some(x);
        }
    }
    if let Some(c) = a.get("choices").and_then(Value::as_array) {
        p.choices = c.clone();
    }
    let mut c = m.clone();
    match c.parameters.iter_mut().find(|x| x.key == key) {
        Some(x) => *x = p,
        None => c.parameters.push(p),
    }
    if let Some(v) = a.get("value") {
        c.apply(&[(key.clone(), v.clone())].into()).map_err(|e| e.to_string())?;
    }
    if let Some(i) = check(&c, r).into_iter().find(|i| i.at == format!("parameters.{key}")) {
        return Err(i.message);
    }
    *m = c;
    let now = m.setting(&key).map_or("the operator's default".to_string(), Value::to_string);
    Ok(format!("the parameter {key} is in the form (now {now})"))
}

/// Keep a scenario: a name and the values it gives (by key; none given: the parameters' values now).
fn set_scenario(m: &mut Model, a: &Value) -> Reply {
    let name = text(a, "name")?.to_string();
    let values: BTreeMap<String, Value> = match a.get("values").and_then(Value::as_object) {
        Some(o) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        None => m.parameters.iter().filter_map(|p| Some((p.key.clone(), m.setting(&p.key)?.clone()))).collect(),
    };
    if values.is_empty() {
        return Err("give `values` (key -> value), or set_parameter first".into());
    }
    // (the values must apply)
    m.clone().apply(&values).map_err(|e| e.to_string())?;
    let note = opt_text(a, "note").map(String::from);
    let at = m.scenarios.iter().position(|s| s.name == name);
    let s = terraflux_graph::Scenario { name: name.clone(), note, values };
    match at {
        Some(k) => m.scenarios[k] = s,
        None => m.scenarios.push(s),
    }
    Ok(format!("the scenario {name} is kept (run_model with `scenario` runs it)"))
}

/// The trace of a cell ([`crate::trace`]): what a node gives at a place (`row` and `col` of its map, or `x` and
/// `y` in its coordinates) and the chain of values that made it, from the values of a run.
pub fn trace_cell(
    m: &Model,
    r: &Registry,
    values: crate::trace::Values,
    kept: &crate::trace::Kept,
    a: &Value,
) -> Reply {
    let (node, port, grid, cell) = trace_place(m, r, values, a)?;
    let t = crate::trace::trace(m, r, values, kept, &node, &port, &grid, cell);
    Ok(format!(
        "{}

{}",
        t.sentence(),
        t.text()
    ))
}

/// The node, port, grid and cell (row, column) a trace is asked of.
type TracePlace = (String, String, Arc<terraflux_raster::Grid>, (usize, usize));

/// The node, port, grid and cell a trace is asked of ([`trace_cell`]'s arguments).
fn trace_place(
    m: &Model,
    r: &Registry,
    values: crate::trace::Values,
    a: &Value,
) -> std::result::Result<TracePlace, String> {
    let node = text(a, "node")?;
    let n = m.node(node).ok_or_else(|| format!("no node {node:?} at the top level"))?;
    // (none given: its first output that is a series of maps, a loop's, else a map)
    let port = match opt_text(a, "port") {
        Some(p) => p.to_string(),
        None => {
            let maps: Vec<(String, bool)> = crate::editor::output_ports(n, r)
                .into_iter()
                .filter_map(|(p, _)| {
                    let v = values(node, &p)?;
                    crate::trace::grid_of(&v)?;
                    Some((p, matches!(v, terraflux_ops::Value::Series(_))))
                })
                .collect();
            maps.iter()
                .find(|(_, series)| *series)
                .or(maps.first())
                .map(|(p, _)| p.clone())
                .or_else(|| crate::builder::first_output(n, r))
                .ok_or_else(|| format!("{node} gives nothing to trace"))?
        }
    };
    let v = values(node, &port).ok_or_else(|| format!("{node}.{port} has not run: run the model first"))?;
    let grid = crate::trace::grid_of(&v).ok_or_else(|| format!("{node}.{port} is not a map: no cell to trace"))?;
    let cell = match (a.get("row").and_then(Value::as_u64), a.get("col").and_then(Value::as_u64)) {
        (Some(row), Some(col)) if (row as usize) < grid.height && (col as usize) < grid.width => {
            (row as usize, col as usize)
        }
        (Some(_), Some(_)) => return Err(format!("the map has {} rows and {} columns", grid.height, grid.width)),
        _ => {
            let (x, y) = (a.get("x").and_then(Value::as_f64), a.get("y").and_then(Value::as_f64));
            let (Some(x), Some(y)) = (x, y) else { return Err("give row and col, or x and y".into()) };
            crate::trace::cell_of(&grid, x, y).ok_or("the place is outside the map")?
        }
    };
    Ok((node.to_string(), port, grid, cell))
}

/// [`trace_cell`] of a model's file (`terraflux mcp`): the model run up to the node first; with `inside_loops`,
/// run again keeping its loops' values at the cell.
pub fn trace_cell_run(engine: &Engine, source: &Model, dir: &Path, a: &Value, watcher: &mut Watcher) -> Reply {
    let node = text(a, "node")?;
    let mut m = source.clone();
    terraflux_graph::submodel::expand(&mut m, dir).map_err(|e| e.to_string())?;
    let inputs = load_inputs(&m, dir, &BTreeMap::new()).map_err(|e| format!("the inputs: {e}"))?;
    let results = engine.run_with(&m, &inputs, watcher, None, Some(node)).map_err(|e| e.to_string())?;
    let values = |n: &str, p: &str| results.values.get(n)?.get(p).cloned();
    let mut kept = crate::trace::Kept::new();
    if a.get("inside_loops").and_then(Value::as_bool) == Some(true) {
        let (_, _, grid, (row, col)) = trace_place(source, &engine.registry, &values, a)?;
        let (x, y) = crate::trace::centre(&grid, row, col);
        kept = crate::trace::keep_loops(source, dir, x, y)?;
    }
    trace_cell(source, &engine.registry, &values, &kept, a)
}

/// The values known of a model, by what reads them (`node.port` at the top level, `$input`): those of a run.
pub type Known<'a> = &'a dyn Fn(&Ref) -> Option<terraflux_ops::Value>;

/// The cells a calculator's script is tried on, at most (as the Calculator pane's preview).
const CALC_SAMPLE: usize = 160_000;

/// What each input of a calculator is, and the values known: a number given, a value of a run ([`Known`]: the
/// outputs of nodes at the top level only, `top`), else a raster read from a file (its header and, for the
/// preview, its cells sampled from the file: nothing runs).
fn calc_inputs(
    nodes: &[Node],
    top: bool,
    dir: &Path,
    n: &Node,
    known: Known,
) -> (algebra_tools::Inputs, BTreeMap<String, terraflux_ops::Value>) {
    use terraflux_ops::Value as V;
    let (mut inputs, mut values) = (algebra_tools::Inputs::new(), BTreeMap::new());
    for (port, src) in &n.inputs {
        let value = match src {
            terraflux_graph::Source::Literal(v) => v.as_f64().map(V::Number),
            terraflux_graph::Source::Ref(s) => Ref::parse(s).ok().and_then(|r| {
                let run = top || !matches!(r, Ref::Port { .. });
                run.then(|| known(&r)).flatten().or_else(|| {
                    let Ref::Port { node, .. } = &r else { return None };
                    let read = nodes.iter().find(|n| n.id == *node && n.op == "raster.read")?;
                    let file = read.params.get("path")?.as_str().filter(|p| !terraflux_graph::is_sent(p))?;
                    let band = read.params.get("band").map(|b| b.as_str().map_or(b.to_string(), String::from));
                    let at = dir.join(file).to_string_lossy().into_owned();
                    let r = terraflux_ops::files::Rasters::open(&at, band.as_deref(), None).ok()?;
                    Some(V::Raster(Arc::new(r.picking(band.is_some()))))
                })
            }),
        };
        inputs.insert(port.clone(), value.as_ref().map_or(algebra_tools::Input::Unknown, algebra_tools::describe));
        if let Some(v) = value {
            values.insert(port.clone(), v);
        }
    }
    (inputs, values)
}

/// A map's cells in words: its classes and their shares (whole numbers, a dozen at most), else its least, mean and
/// largest values; the cells without a value.
fn cells_text(cells: &[f64]) -> String {
    let (least, mean, largest, missing) = algebra_tools::summary(cells);
    let mut classes = BTreeMap::new();
    let whole = cells.iter().filter(|x| !x.is_nan()).all(|x| x.fract() == 0.0 && x.abs() < 1e9);
    if whole {
        for x in cells.iter().filter(|x| !x.is_nan()) {
            *classes.entry(*x as i64).or_insert(0usize) += 1;
        }
    }
    let with = cells.len() - missing;
    let mut t = if with == 0 {
        "no cell has a value".to_string()
    } else if whole && classes.len() <= 12 {
        let shares: Vec<String> =
            classes.iter().map(|(c, k)| format!("{c} ({:.1} %)", 100.0 * *k as f64 / with as f64)).collect();
        format!("classes {}", shares.join(", "))
    } else {
        format!("from {least:.6} to {largest:.6}, mean {mean:.6}")
    };
    if missing > 0 && with > 0 {
        t.push_str(&format!("; {missing} of {} cells without a value", cells.len()));
    }
    t
}

/// What an output of map algebra gave, in words.
fn out_text(o: &terraflux_ops::algebra::Out) -> String {
    use terraflux_ops::algebra::Out;
    match o {
        Out::Number(x) => format!("the number {x}"),
        Out::Map(cells) => format!("a map: {}", cells_text(cells)),
        Out::Stack(layers) => {
            let names: Vec<String> = layers.iter().take(8).map(|(k, _)| k.to_string()).collect();
            let all: Vec<f64> = layers.iter().flat_map(|(_, c)| c.iter().copied()).collect();
            let more = if layers.len() > 8 { ", ..." } else { "" };
            format!("a stack of {} layers ({}{more}): {}", layers.len(), names.join(", "), cells_text(&all))
        }
    }
}

/// What an input of map algebra is, in words.
fn input_text(i: &algebra_tools::Input) -> String {
    use algebra_tools::Input;
    match i {
        Input::Map => "a map".into(),
        Input::Number(Some(x)) => format!("the number {x}"),
        Input::Number(None) => "a number".into(),
        Input::Stack(Some(layers)) => {
            let names: Vec<&str> = layers.iter().map(|(name, _)| name.as_str()).collect();
            format!("a stack of {} layers, read as {}", layers.len(), names.join(", "))
        }
        Input::Stack(None) => "a stack".into(),
        Input::Unknown => "not known yet (what it reads has not run)".into(),
    }
}

/// What a script gave on a sample: each output in words (by name), and the sample.
type Sampled = std::result::Result<(Vec<(String, String)>, String), String>;

/// A script run on a sample of the cells of its inputs, with each name it gives a value an output too: what
/// each one holds (by name), and the sample. None when an input it reads is not known.
fn calc_sample(script: &str, values: &BTreeMap<String, terraflux_ops::Value>, every_name: bool) -> Option<Sampled> {
    if algebra_tools::reads(script).iter().any(|name| !values.contains_key(name)) {
        return None;
    }
    let mut text = script.to_string();
    if every_name {
        let outs = terraflux_graph::declared_outputs(script);
        for name in algebra_tools::names_given(script) {
            if !outs.contains(&name) {
                text.push_str(&format!("\nout {name}"));
            }
        }
    }
    Some(match algebra_tools::preview(&text, values, CALC_SAMPLE) {
        Ok(p) => {
            let sample = if p.step > 1 {
                format!("one cell in {} ({} x {} cells)", p.step * p.step, p.width, p.height)
            } else {
                format!("every cell ({} x {})", p.width, p.height)
            };
            Ok((p.outputs.iter().map(|(name, o)| (name.clone(), out_text(o))).collect(), sample))
        }
        Err(e) => Err(e.0),
    })
}

/// Write the script of a map calculator (`raster.calc`): a new node of the id given, or the one of that id
/// rewritten (its inputs connected as given). The script is checked first (its language, every name an input or
/// given a value before, the layers a stack has) and run on a sample of its inputs' cells: refused with the
/// reason, the model as it was, when it fails; else written, and what each output gives told.
pub fn write_calc(m: &mut Model, r: &Registry, dir: &Path, a: &Value, known: Known) -> Reply {
    let id = text(a, "node")?;
    let script = text(a, "script")?;
    let path = scope(a);
    let nodes = m.scope(&path).ok_or_else(|| format!("no container {}", path.join("/")))?;
    let found = nodes.iter().find(|n| n.id == id);
    let op = match (found, opt_text(a, "op")) {
        (Some(n), _) if !CALCULATORS.contains(&n.op.as_str()) => {
            return Err(format!(
                "{id} is a {}, not a calculator ({}): give another id for a new one",
                n.op,
                CALCULATORS.join(", ")
            ));
        }
        (Some(n), Some(op)) if op != n.op => return Err(format!("{id} is a {}, not a {op}", n.op)),
        (Some(n), _) => n.op.clone(),
        (None, Some(op)) if !CALCULATORS.contains(&op) => {
            return Err(format!("op {op:?}: a calculator is one of {}", CALCULATORS.join(", ")));
        }
        (None, op) => op.unwrap_or("raster.calc").to_string(),
    };
    let exists = found.is_some();
    let features = op != "raster.calc";
    let refused = "the script was not written (the model is as it was)";
    let mut args = a.clone();
    let o = args.as_object_mut().ok_or("the arguments are not an object")?;
    o.remove("script");
    o.remove("op");
    if features {
        let s = terraflux_ops::algebra::parse_script(script).map_err(|e| format!("{refused}: {}", e.0))?;
        match (op.as_str(), s.outputs().is_empty()) {
            ("vector.calc", true) => {
                return Err(format!(
                    "{refused}: a vector.calc's script gives each attribute with out (out density = pop / \
                     geometry.area)"
                ));
            }
            ("vector.filter", false) => {
                return Err(format!(
                    "{refused}: a vector.filter's script ends with the condition of the features kept, without out"
                ));
            }
            _ => {}
        }
    }
    // (a vector.calc's attributes named by its out lines, not by its column)
    let settings = if op == "vector.calc" {
        json!({ "expression": script, "column": null })
    } else {
        json!({ "expression": script })
    };
    o.insert("settings".into(), settings);
    let mut next = m.clone();
    let done = if exists {
        edit_node(&mut next, r, &args)?
    } else {
        o.insert("id".into(), json!(id));
        o.insert("op".into(), json!(op));
        add_node(&mut next, r, &args)?
    };
    let nodes = next.scope(&path).expect("the container");
    let n = nodes.iter().find(|n| n.id == id).expect("the node written");
    let (inputs, values) = calc_inputs(nodes, path.is_empty(), dir, n, known);
    if features {
        let values = with_texts(n, values);
        let tried = match features_sample(script, n, &values, false) {
            Some(Ok((given, sample))) if op == "vector.filter" => {
                let Column::Numbers(keep) = &given[0].1 else {
                    return Err(format!(
                        "{refused}: its condition gives texts: a feature is kept where it is true (uso == \"forest\")"
                    ));
                };
                format!("\nOn a sample of {sample}, {}", kept_text(keep))
            }
            Some(Ok((given, sample))) => {
                let old = match values.get("vector") {
                    Some(terraflux_ops::Value::Vector(v)) => v.attributes.iter().map(|(a, _)| a.clone()).collect(),
                    _ => Vec::new(),
                };
                let outs: Vec<String> = given
                    .iter()
                    .map(|(name, c)| {
                        let replaced = if old.contains(name) { " (replacing the features' attribute)" } else { "" };
                        format!("- {name}{replaced}: {}", column_text(c))
                    })
                    .collect();
                format!("\nIts attributes, on a sample of {sample}:\n{}", outs.join("\n"))
            }
            Some(Err(e)) => return Err(format!("{refused}: on a sample of its features it fails: {e}")),
            None => "\nNo preview: the features, or an input it reads, have not run yet (run the model, then \
                     explain_calc)."
                .into(),
        };
        *m = next;
        return Ok(format!("{done}{tried}"));
    }
    if let Some(p) = algebra_tools::problem(script, &inputs) {
        let names: Vec<String> = inputs.iter().map(|(name, i)| format!("{name} ({})", input_text(i))).collect();
        return Err(format!(
            "{refused}: line {}: {}\nIts inputs: {}",
            p.line,
            p.message,
            if names.is_empty() { "none (give `inputs`)".into() } else { names.join(", ") }
        ));
    }
    let tried = match calc_sample(script, &values, false) {
        Some(Ok((outs, sample))) => {
            let outs: Vec<String> = outs.iter().map(|(name, t)| format!("- {name}: {t}")).collect();
            format!("\nIts outputs, on a sample of {sample}:\n{}", outs.join("\n"))
        }
        Some(Err(e)) => return Err(format!("{refused}: on a sample of its inputs' cells it fails: {e}")),
        None => "\nNo preview: an input it reads has not run yet (run the model, then explain_calc).".into(),
    };
    *m = next;
    Ok(format!("{done}{tried}"))
}

/// A map calculator's script explained by what it does: each input (what it reads and what it is), each line with
/// the names it reads, and what each name given a value holds on a sample of the cells.
pub fn explain_calc(m: &Model, dir: &Path, a: &Value, known: Known) -> Reply {
    let id = text(a, "node")?;
    let path = scope(a);
    let nodes = m.scope(&path).ok_or_else(|| format!("no container {}", path.join("/")))?;
    let n = nodes.iter().find(|n| n.id == id).ok_or_else(|| format!("no node {id:?}"))?;
    if !CALCULATORS.contains(&n.op.as_str()) {
        return Err(format!("{id} is a {}, not a calculator ({})", n.op, CALCULATORS.join(", ")));
    }
    let script = match n.params.get("expression") {
        Some(Value::String(t)) => t.clone(),
        _ => return Err(format!("{id} has no script")),
    };
    let (inputs, values) = calc_inputs(nodes, path.is_empty(), dir, n, known);
    if n.op != "raster.calc" {
        return Ok(explain_features(n, &script, &with_texts(n, values)));
    }
    let mut t = format!("The script of {id}:\n{script}\n\nIts inputs:");
    for (name, i) in &inputs {
        let from = match n.inputs.get(name) {
            Some(terraflux_graph::Source::Ref(s)) => s.clone(),
            Some(terraflux_graph::Source::Literal(v)) => v.to_string(),
            None => "nothing".into(),
        };
        t.push_str(&format!("\n- {name} <- {from}: {}", input_text(i)));
    }
    let layers = algebra_tools::layers_read(&script);
    if !layers.is_empty() {
        let read: Vec<String> = layers.iter().map(|(name, l)| format!("{name}: {}", l.join(", "))).collect();
        t.push_str(&format!("\nThe layers it reads: {}", read.join("; ")));
    }
    if let Some(p) = algebra_tools::problem(&script, &inputs) {
        t.push_str(&format!("\nIts error: line {}: {}", p.line, p.message));
        return Ok(t);
    }
    match calc_sample(&script, &values, true) {
        Some(Ok((outs, sample))) => {
            let declared = terraflux_graph::declared_outputs(&script);
            t.push_str(&format!("\nWhat each name holds at the end, on a sample of {sample}:"));
            for (name, what) in outs {
                let out = if declared.contains(&name) { " (an output)" } else { "" };
                t.push_str(&format!("\n- {name}{out}: {what}"));
            }
        }
        Some(Err(e)) => t.push_str(&format!("\nOn a sample of its inputs' cells it fails: {e}")),
        None => t.push_str("\nNo values: an input it reads has not run yet (run the model first)."),
    }
    Ok(t)
}

/// The calculators: of maps (`raster.calc`), of features' attributes (`vector.calc`) and of the features kept
/// (`vector.filter`).
const CALCULATORS: [&str; 3] = ["raster.calc", "vector.calc", "vector.filter"];

/// The features a calculator over features is tried on, at most.
const FEATURES_SAMPLE: usize = 5_000;

/// The values of a calculator's inputs with the texts given as literals (a calculator over features reads them).
fn with_texts(n: &Node, mut values: BTreeMap<String, terraflux_ops::Value>) -> BTreeMap<String, terraflux_ops::Value> {
    for (port, src) in &n.inputs {
        if let terraflux_graph::Source::Literal(Value::String(t)) = src {
            values.insert(port.clone(), terraflux_ops::Value::Text(t.clone()));
        }
    }
    values
}

/// A script over features (`vector.calc`, `vector.filter`) run on a sample of them (with `every_name`, what each
/// name it gives a value holds, [`vector_algebra::every_name`]): the attributes it gives by name, and the sample in
/// words. None when the features, or another input of the node, are not known.
fn features_sample(
    script: &str,
    n: &Node,
    values: &BTreeMap<String, terraflux_ops::Value>,
    every_name: bool,
) -> Option<std::result::Result<(Vec<(String, Column)>, String), String>> {
    let Some(terraflux_ops::Value::Vector(v)) = values.get("vector") else { return None };
    if n.inputs.keys().any(|port| !values.contains_key(port)) {
        return None;
    }
    let script = match terraflux_ops::algebra::parse_script(script) {
        Ok(s) => s,
        Err(e) => return Some(Err(e.0)),
    };
    let all = v.len();
    let step = all.div_ceil(FEATURES_SAMPLE).max(1);
    let picked;
    let sample = if step > 1 {
        picked = v.select(&(0..all).step_by(step).collect::<Vec<_>>());
        &picked
    } else {
        v.as_ref()
    };
    let given = if every_name {
        vector_algebra::every_name(sample, values, &script)
    } else {
        vector_algebra::run(sample, values, &script)
    };
    let what = if step > 1 {
        format!("one feature in {step} ({} of {all})", sample.len())
    } else {
        format!("every feature ({all})")
    };
    Some(given.map(|g| (g, what)).map_err(|e| e.0))
}

/// An attribute's values in words: numbers as a map's cells ([`cells_text`]), texts by their shares (a dozen
/// at most, the most frequent first); the features without a value.
fn column_text(c: &Column) -> String {
    let t = match c {
        Column::Numbers(x) => return format!("numbers: {}", cells_text(x).replace("cell", "feature")),
        Column::Texts(t) => t,
    };
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for x in t.iter().filter(|x| !x.is_empty()) {
        *counts.entry(x).or_default() += 1;
    }
    let with: usize = counts.values().sum();
    if with == 0 {
        return "no feature has a value".into();
    }
    let mut by: Vec<(&str, usize)> = counts.into_iter().collect();
    by.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let shares: Vec<String> =
        by.iter().take(12).map(|(x, k)| format!("{x:?} ({:.1} %)", 100.0 * *k as f64 / with as f64)).collect();
    let mut out = format!("texts {}", shares.join(", "));
    if by.len() > 12 {
        out.push_str(&format!(" and {} others", by.len() - 12));
    }
    if with < t.len() {
        out.push_str(&format!("; {} of {} features without a value", t.len() - with, t.len()));
    }
    out
}

/// What a condition over features keeps, in words.
fn kept_text(keep: &[f64]) -> String {
    let kept = keep.iter().filter(|x| !x.is_nan() && **x != 0.0).count();
    let share = if keep.is_empty() { 0.0 } else { 100.0 * kept as f64 / keep.len() as f64 };
    format!("it keeps {kept} of {} features ({share:.1} %)", keep.len())
}

/// A calculator over features (`vector.calc`, `vector.filter`) explained: each input (the features: how many and
/// their attributes; the others), its error if it has one, and what each name it gives a value holds at the end
/// on a sample of the features.
fn explain_features(n: &Node, script: &str, values: &BTreeMap<String, terraflux_ops::Value>) -> String {
    use terraflux_ops::Value as V;
    let mut t = format!("The script of {} ({}):\n{script}\n\nIts inputs:", n.id, n.op);
    for (port, src) in &n.inputs {
        let from = match src {
            terraflux_graph::Source::Ref(s) => s.clone(),
            terraflux_graph::Source::Literal(v) => v.to_string(),
        };
        let what = match values.get(port) {
            Some(V::Vector(v)) => {
                let attributes: Vec<String> = v
                    .attributes
                    .iter()
                    .map(|(a, c)| format!("{a} ({})", if matches!(c, Column::Texts(_)) { "texts" } else { "numbers" }))
                    .collect();
                format!(
                    "{} features, their attributes: {} (and geometry.area, geometry.perimeter, geometry.length, \
                     geometry.x, geometry.y, feature)",
                    v.len(),
                    if attributes.is_empty() { "none".into() } else { attributes.join(", ") }
                )
            }
            Some(V::Text(x)) => format!("the text {x:?}"),
            Some(v) => v.as_number().map_or_else(|| format!("a {:?}", v.ty()), |x| format!("the number {x}")),
            None => "not known yet (what it reads has not run)".into(),
        };
        t.push_str(&format!("\n- {port} <- {from}: {what}"));
    }
    match features_sample(script, n, values, true) {
        Some(Ok((given, sample))) => {
            let declared = terraflux_graph::declared_outputs(script);
            let column = n.params.get("column").and_then(Value::as_str).unwrap_or("");
            t.push_str(&format!("\nWhat each name holds at the end, on a sample of {sample}:"));
            for (name, c) in given {
                let line = match (name.as_str(), n.op.as_str(), &c) {
                    (vector_algebra::LAST_LINE, "vector.filter", Column::Numbers(keep)) => {
                        format!("the condition: {}", kept_text(keep))
                    }
                    (vector_algebra::LAST_LINE, "vector.calc", _) => {
                        format!("the attribute {column:?} given: {}", column_text(&c))
                    }
                    (vector_algebra::LAST_LINE, ..) => format!("its last line: {}", column_text(&c)),
                    _ if declared.contains(&name) => format!("{name} (an attribute given): {}", column_text(&c)),
                    _ => format!("{name}: {}", column_text(&c)),
                };
                t.push_str(&format!("\n- {line}"));
            }
        }
        Some(Err(e)) => t.push_str(&format!("\nIts error, on a sample of its features: {e}")),
        None => match terraflux_ops::algebra::parse_script(script) {
            Err(e) => t.push_str(&format!("\nIts error: {}", e.0)),
            Ok(_) => {
                t.push_str("\nNo values: the features, or an input it reads, have not run yet (run the model first).")
            }
        },
    }
    t
}

/// The values of a model's file known for its calculators (`terraflux mcp`): what a node gives, the model run up
/// to it when asked (what ran kept for the next), and the model's inputs.
pub fn known_by_running<'a>(
    engine: &'a Engine,
    source: &Model,
    dir: &Path,
) -> impl Fn(&Ref) -> Option<terraflux_ops::Value> + 'a {
    let mut m = source.clone();
    let expanded = terraflux_graph::submodel::expand(&mut m, dir).is_ok();
    let inputs = load_inputs(&m, dir, &BTreeMap::new()).ok();
    let cache = std::sync::Mutex::new(terraflux_exec::Cache::default());
    move |r: &Ref| match r {
        Ref::Input(name) => inputs.as_ref()?.get(name).cloned(),
        Ref::Port { node, port } if expanded => {
            if let Some(v) = cache.lock().unwrap().outputs(node).and_then(|o| o.get(port).cloned()) {
                return Some(v);
            }
            let mut watcher = Watcher::new(None, None);
            engine.run_with(&m, inputs.as_ref()?, &mut watcher, Some(&cache), Some(node)).ok()?;
            cache.lock().unwrap().outputs(node)?.get(port).cloned()
        }
        _ => None,
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

/// What to do next with a model copied from a template: its data drawn by neutral landscape models are to be
/// replaced by the user's files (none drawn: nothing to say).
pub fn template_next(m: &Model) -> String {
    let drawn: Vec<String> = m.nodes.iter().filter(|n| n.op.starts_with("nlm.")).map(|n| n.id.clone()).collect();
    if drawn.is_empty() {
        return String::new();
    }
    format!(
        "\nNext: its data are drawn ({}: neutral landscape models, and the maps made of them to stand for data, \
         such as a later date); for the user's data put a raster.read node (absolute `path`) in place of each and \
         connect what read them to it (inspect_file the files first; guide data-preparation puts them on one grid), \
         then check_model and run_model.",
        drawn.join(", ")
    )
}

/// The model as text (TOML) and the problems it has.
/// A note or frame of the canvas added or changed ([`terraflux_graph::Note`]).
fn set_note(m: &mut Model, a: &Value) -> Reply {
    let scope = scope(a);
    let nodes = m.scope(&scope).ok_or_else(|| format!("the model has no container {:?}", scope.join("/")))?;
    let framed: Option<Vec<String>> = match a.get("nodes") {
        None | Some(Value::Null) => None,
        Some(Value::Array(v)) => {
            let ids: Vec<String> = v.iter().filter_map(|x| x.as_str().map(String::from)).collect();
            if let Some(gone) = ids.iter().find(|id| !nodes.iter().any(|n| &n.id == *id)) {
                return Err(format!("no node {gone:?} in the scope to frame"));
            }
            if ids.is_empty() {
                return Err("nodes: the ids of the nodes it frames (none: leave it out for a note)".into());
            }
            Some(ids)
        }
        Some(_) => return Err("nodes: a list of node ids".into()),
    };
    let at = match a.get("at") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_array().map(|v| v.iter().filter_map(Value::as_f64).collect::<Vec<f64>>()).as_deref() {
            Some([x, y]) => Some([*x as f32, *y as f32]),
            _ => return Err("at: [x, y], two numbers".into()),
        },
    };
    let colour = opt_text(a, "colour").map(String::from);
    if let Some(c) = &colour
        && !terraflux_graph::NOTE_COLOURS.contains(&c.as_str())
    {
        return Err(format!("the colour {c:?}: one of {}", terraflux_graph::NOTE_COLOURS.join(", ")));
    }
    // (a note put nowhere: above the scope's nodes, at the left)
    let auto = crate::editor_view::auto_positions(nodes);
    let corner = nodes
        .iter()
        .filter_map(|n| n.pos.or_else(|| auto.get(&n.id).copied()))
        .reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1])])
        .map_or([0.0, -140.0], |p| [p[0], p[1] - 140.0]);
    let id = opt_text(a, "id").map(String::from);
    match id.as_ref().and_then(|id| m.notes.iter_mut().find(|n| &n.id == id)) {
        Some(n) => {
            if let Some(t) = opt_text(a, "text") {
                n.text = t.to_string();
            }
            if let Some(ids) = framed {
                n.nodes = ids;
                n.at = None;
            }
            if at.is_some() {
                n.at = at;
                n.nodes.clear();
            }
            if colour.is_some() {
                n.colour = colour;
            }
            n.scope = scope;
            Ok(format!("changed the {} {}", if n.is_frame() { "frame" } else { "note" }, n.id))
        }
        None => {
            let frame = framed.is_some();
            let id = id.unwrap_or_else(|| m.new_note_id(frame));
            if !terraflux_graph::valid_id(&id) {
                return Err(format!("{id:?} is not a valid id (a letter, then letters, digits, _ or -)"));
            }
            let at = if frame { None } else { at.or(Some(corner)) };
            let text = opt_text(a, "text").unwrap_or_default().to_string();
            let n =
                terraflux_graph::Note { id: id.clone(), scope, text, colour, at, nodes: framed.unwrap_or_default() };
            let what = if n.is_frame() { format!("a frame around {}", n.nodes.join(", ")) } else { "a note".into() };
            m.notes.push(n);
            Ok(format!("added {what} ({id}): on the canvas, outside the graph"))
        }
    }
}

pub fn show_model(m: &Model, dir: &Path, r: &Registry) -> Reply {
    let mut text = format!("{}\n{}", m.to_toml().map_err(|e| e.to_string())?, problems(m, dir, r));
    // (the places kept, a line each, besides the [[bookmarks]] above)
    if !m.bookmarks.is_empty() {
        text.push_str("\nPlaces kept:");
        for p in &m.bookmarks {
            let slot = p.slot.map(|k| format!(", slot {k}")).unwrap_or_default();
            let what = match &p.view {
                terraflux_graph::PlaceView::Map { extent, crs, .. } => {
                    format!("Viewer, extent {extent:?} in {}", crs.as_deref().unwrap_or("the map's cells"))
                }
                terraflux_graph::PlaceView::Canvas { scope, x, y, zoom } => {
                    let at = if scope.is_empty() { "the top level" } else { scope.as_str() };
                    format!("canvas of {at}, at ({x:.0}, {y:.0}), zoom {zoom:.2}")
                }
            };
            text.push_str(&format!("\n- {}: {what}{slot}", p.name));
        }
    }
    Ok(text)
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

/// A model's methods section and the papers to cite (`file` the model's file name): as text (the paragraph, the
/// steps, the references), BibTeX or RIS (`format`), in English or Portuguese (`language`).
pub fn methods(m: &Model, r: &Registry, a: &Value, file: Option<&str>) -> Reply {
    use crate::methods::Lang;
    let lang = match opt_text(a, "language") {
        None => Lang::En,
        Some(t) => Lang::parse(t).ok_or_else(|| format!("the language {t:?}: en or pt"))?,
    };
    let version = env!("CARGO_PKG_VERSION");
    let me = crate::methods::of_in(m, r, version, file, lang);
    match opt_text(a, "format").unwrap_or("text") {
        "text" => Ok(me.text()),
        "bibtex" | "bib" => Ok(me.bibtex(version)),
        "ris" => Ok(me.ris(version)),
        f => Err(format!("the format {f:?}: text, bibtex or ris")),
    }
}

/// The model and its data checked without running it.
pub fn check_model(r: &Registry, source: &Model, dir: &Path) -> Reply {
    let mut m = source.clone();
    terraflux_graph::submodel::expand(&mut m, dir).map_err(|e| e.to_string())?;
    let issues = check(&m, r);
    let mut lines: Vec<String> = issues.iter().map(|i| i.to_string()).collect();
    let mut errors = issues.iter().any(|i| i.severity == Severity::Error);
    let loaded = load_inputs(&m, dir, &BTreeMap::new());
    match &loaded {
        Err(e) => {
            lines.push(format!("error: the inputs: {e}"));
            errors = true;
        }
        Ok(inputs) => {
            for f in terraflux_exec::lint::lint(&m, inputs) {
                errors |= f.level == terraflux_exec::lint::Level::Error;
                lines.push(f.to_string());
            }
        }
    }
    // the method's (what a reviewer would ask)
    lines.extend(crate::advice::advise(&m, r, dir, loaded.as_ref().ok()).iter().map(|a| a.to_string()));
    let (no_note, no_why) = unrecorded(source);
    for (what, ids) in [("a note (what it is for)", no_note), ("a why (why this operator and these settings)", no_why)]
    {
        if !ids.is_empty() {
            lines.push(format!("advice: nodes without {what}: {} (edit_node gives it)", ids.join(", ")));
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
/// inputs and settings for this run only). `file` is the model's file: the run is recorded in its journal.
pub fn run_model(
    engine: &Engine,
    source: &Model,
    file: Option<&Path>,
    dir: &Path,
    a: &Value,
    watcher: &mut Watcher,
) -> Reply {
    use terraflux_exec::journal::{Entry, Kind, Status, Written, record_run};
    let started = Instant::now();
    let scenario = opt_text(a, "scenario").map(String::from);
    let (m, inputs, results, seconds) = match run_for(engine, source, dir, a, watcher) {
        Ok(r) => r,
        Err(e) => {
            if let Some(f) = file {
                let mut entry = Entry::new(Kind::Run, "mcp", started.elapsed().as_secs_f64());
                entry.status = Status::Failed(e.clone());
                entry.scenario = scenario;
                entry.warnings = watcher.warnings.clone();
                let _ = record_run(f, source, entry);
            }
            return Err(e);
        }
    };
    let out = opt_text(a, "out").map(absolute).unwrap_or_else(|| dir.join("output"));
    let files = write_outputs(&m, &results, &out).map_err(|e| e.to_string())?;
    if let Some(f) = file {
        let mut entry = Entry::new(Kind::Run, "mcp", started.elapsed().as_secs_f64());
        entry.scenario = scenario;
        entry.warnings = watcher.warnings.clone();
        entry.outputs = m.outputs.keys().zip(&files).filter_map(|(n, p)| Written::of(n, p, None, None)).collect();
        let _ = record_run(f, &m, entry);
    }
    let mut s = format!("Ran in {seconds:.1} s.");
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
    s.push_str(&crate::advice::after_run(&m, &engine.registry, dir, Some(&inputs)));
    Ok(s)
}

/// Run a model for [`run_model`] and [`export_plot`] (`scenario` and `set` of `a`): the model as it ran, its
/// inputs, its results and the seconds it took.
fn run_for(
    engine: &Engine,
    source: &Model,
    dir: &Path,
    a: &Value,
    watcher: &mut Watcher,
) -> std::result::Result<(Model, BTreeMap<String, terraflux_ops::Value>, terraflux_exec::Results, f64), String> {
    let mut m = match opt_text(a, "scenario") {
        Some(name) => source.with_scenario(name).map_err(|e| e.to_string())?,
        None => source.clone(),
    };
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
    // (what ran before a failure kept, to explain it: what the node that failed was given)
    let cache = std::sync::Mutex::new(terraflux_exec::Cache::default());
    let results = engine.run_with(&m, &inputs, watcher, Some(&cache), None).map_err(|e| {
        let e = e.to_string();
        let c = cache.lock().unwrap();
        let values = |n: &str, p: &str| c.outputs(n)?.get(p).cloned();
        match crate::diagnose::failed_at(&e)
            .and_then(|(at, message)| crate::diagnose::diagnose(&m, &at, &message, &values))
        {
            Some(d) => format!(
                "The run failed.
{}",
                d.text()
            ),
            None => e,
        }
    })?;
    Ok((m, inputs, results, t.elapsed().as_secs_f64()))
}

/// Run a model and write one of its Plots (`id`) as a picture of the map (`file`: `.png` or `.tif`, a GeoTIFF of
/// red, green, blue and alpha on the map's grid; `scale`: pixels to a cell, 1, 2 or 4): as the Viewer draws it
/// from the Plot alone, without the application. What was written, the layers drawn and not drawn.
pub fn export_plot(engine: &Engine, source: &Model, dir: &Path, a: &Value, watcher: &mut Watcher) -> Reply {
    let id = text(a, "id")?;
    let file = absolute(text(a, "file")?);
    let scale = crate::plot_export::check_scale(a.get("scale").and_then(Value::as_u64).unwrap_or(1))?;
    let format = crate::plot_export::format_for(&file, opt_text(a, "format"))?;
    // (before the run, which can take long)
    crate::plot_export::find(source, id)?;
    let (m, inputs, results, seconds) = run_for(engine, source, dir, a, watcher)?;
    let value = |from: &str| crate::plot_export::lookup(&inputs, &results, from);
    let made = crate::plot_export::panel_of(&m, id, &value)?;
    if let Some(parent) = file.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let said = crate::plot_export::write(&made, scale, format, &file)?;
    Ok(format!("Ran in {seconds:.1} s. The plot {id}, {said}"))
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
        "description": "Setting -> value (JSON: a number, a text, a list, an object), or a value read when the model runs: \"$input\", \"@step\" or \"@carried\", {\"from\": \"node.port\"}; null removes it. See describe_operator.",
    });
    let read_only = json!({ "readOnlyHint": true });
    let list = |what: &str| json!({ "type": "array", "items": { "type": "string" }, "description": what });
    let record = json!({
        "why": { "type": "string", "description": "Why this operator and these settings (empty: none)." },
        "refs": list("Papers, methods, documentation the choice rests on (empty: none)."),
        "assumptions": list("What the choice assumes (empty: none)."),
        "alternatives": list("What was considered and set aside, and why (empty: none)."),
    });
    let about = json!({
        "request": { "type": "string", "description": "The user's request, in the user's words." },
        "summary": { "type": "string", "description": "A summary of the conversation that led to the model." },
        "author": { "type": "string", "description": "Who made it: a person, an agent (which), or both." },
    });
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
            "guide",
            "A guide, read before building or reviewing a model: the method of a field step by step with its operators and templates (land-change, connectivity, landscape-metrics, conservation-planning, species-distribution, data-preparation), or how a model is built (nodes, calc, containers, record, results). Without a topic: the guides listed.",
            json!({
                "topic": { "type": "string", "description": "The guide, e.g. land-change." },
                "language": { "type": "string", "enum": ["en", "pt"], "description": "en (default) or pt: the guides of the methods in Portuguese, for a user who writes in it." },
            }),
            &[],
            Some(&read_only),
        ),
        tool(
            "list_templates",
            "The templates: small models that run as they are, by group, with what each shows. With a query (English or Portuguese words): those that match it, the best first. A good start: new_model with `template` copies one.",
            json!({ "query": { "type": "string", "description": "What the model is to do, e.g. \"connectivity of a forest\", \"desmatamento\"." } }),
            &[],
            Some(&read_only),
        ),
        tool(
            "list_operators",
            "The operators. Without a query: their names by family. With a query (English or Portuguese words, found in the name or the documentation): those that have all of them, else those that have the most, each with what it does.",
            json!({ "query": { "type": "string", "description": "Words to look for, e.g. \"weights of evidence\", \"patch area\", \"conectividade\"." } }),
            &[],
            Some(&read_only),
        ),
        tool(
            "describe_operator",
            "An operator's documentation: its inputs, outputs and settings (types, choices, which are optional), a node of a template using it, advice on its use, the method and papers to cite, and the operators it usually reads and gives to.",
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
                "request": about["request"],
                "summary": about["summary"],
                "author": about["author"],
                "overwrite": { "type": "boolean" },
            }),
            &["model"],
            None,
        ),
        tool(
            "set_model_info",
            "Set what the model says of itself: its name, description, the user's request, a summary of the conversation that led to it, its author. Empty: removed.",
            json!({
                "model": model,
                "name": { "type": "string" },
                "description": { "type": "string" },
                "request": about["request"],
                "summary": about["summary"],
                "author": about["author"],
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
                "note": { "type": "string", "description": "What the node is for here, in a sentence (shown on the canvas)." },
                "why": record["why"],
                "refs": record["refs"],
                "assumptions": record["assumptions"],
                "alternatives": record["alternatives"],
                "scope": scope,
            }),
            &["model", "op"],
            None,
        ),
        tool(
            "edit_node",
            "Change a node: connect or disconnect its inputs, change its settings, its note or its record (why, refs, assumptions, alternatives), rename it (the references to it follow).",
            json!({
                "model": model,
                "node": { "type": "string" },
                "inputs": inputs,
                "settings": settings,
                "note": { "type": "string", "description": "Empty: no note." },
                "why": record["why"],
                "refs": record["refs"],
                "assumptions": record["assumptions"],
                "alternatives": record["alternatives"],
                "rename": { "type": "string" },
                "scope": scope,
            }),
            &["model", "node"],
            None,
        ),
        tool(
            "write_calc",
            "Write the script of a calculator: of maps (raster.calc, the default), of the attributes of vector features (vector.calc: `out name = value` for each attribute given or replaced) or of the features kept (vector.filter: the script's last line is the condition, no out); a new node of that id (`op`), or the one of that id rewritten. A script over features reads the attributes by their names, numbers and texts (uso == \"forest\", \"high\" if pop > 100 else \"low\", float(code) a text read as a number), the geometry's measures (geometry.area, geometry.perimeter, geometry.length, geometry.x, geometry.y), feature (1..), x.sum() and the other methods over all the features; connect the features to its input `vector`. Give `inputs` for the names the script reads (name -> `node.port` or `$input`; a stack's layers are read as `s.layer`). The script is checked before anything changes (its language, every name an input of the node or given a value before, the layers a stack has) and run on a sample of its inputs' cells: the reply tells what each output gives (a map's classes and their shares, or its least, mean and largest values), or why it was refused (the model as it was). Read the reply: a map of a single class, or no cell with a value, is seldom what was asked for.",
            json!({
                "model": model,
                "node": { "type": "string", "description": "The calculator's id: one that exists (rewritten) or a new one." },
                "op": { "type": "string", "enum": CALCULATORS, "description": "A new calculator's operator: raster.calc (the default), vector.calc or vector.filter." },
                "script": { "type": "string", "description": "The script: a statement a line, a comment saying what it does, `out name = value` (or `out name`) for each output." },
                "inputs": inputs,
                "note": { "type": "string", "description": "What the calculator is for." },
                "why": record["why"],
                "refs": record["refs"],
                "assumptions": record["assumptions"],
                "alternatives": record["alternatives"],
                "scope": scope,
            }),
            &["model", "node", "script"],
            None,
        ),
        tool(
            "save_my_node",
            "Save a group (control.group) or a calculator (raster.calc, made a group of itself) as a node of the user's libraries, to use in any model as `my:name` (add_node): the next version of its name, its inputs and outputs typed from what it is given and gave, the layers its scripts read, the literal values its inputs have its settings; the node in the model becomes that version. In `library` (a shared library's name as list_operators shows it, `lab` in `my:lab/name`; none: the user's own).",
            json!({
                "model": model,
                "node": { "type": "string", "description": "The group's or the calculator's id." },
                "name": { "type": "string", "description": "The node's name in the library (letters, digits, _ and -; none: the node's id)." },
                "description": { "type": "string", "description": "What it does, for whoever adds it (none: the node's note)." },
                "library": { "type": "string", "description": "The library: a shared one's name; none: the user's own." },
                "scope": scope,
            }),
            &["model", "node"],
            None,
        ),
        tool(
            "explain_calc",
            "A calculator's script (raster.calc, vector.calc, vector.filter) taken apart: each input (what it reads, and what it is: a map, a stack and its layers, a number; features, how many and their attributes), the layers it reads, its error if it has one, and what each name it gives a value holds at the end on a sample of the cells or features (classes and their shares, texts and their shares, or least, mean and largest values; the features a condition keeps). Explain a script, or find why it gives what it gives, from it.",
            json!({ "model": model, "node": { "type": "string" }, "scope": scope }),
            &["model", "node"],
            Some(&read_only),
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
            "set_plot",
            "Add or replace a Plot: a view of the results kept with the model (outside the graph), shown in the application's Viewer and as a figure of the Document and the report. Its layers, the lowest first: maps (each as opaque as asked), vector features over them, or a table (drawn as a chart). In place of the view.plot node.",
            json!({
                "model": model,
                "id": { "type": "string", "description": "The plot's id (a letter, then letters, digits, _ or -)." },
                "title": { "type": "string", "description": "Its title, the figure's caption (default: what it shows)." },
                "layers": {
                    "type": "array",
                    "description": "What it shows, the lowest first: node.port of top-level nodes (or $input), or objects { from, opacity (0 to 1), band (a stack's layer or a loop's step, 0-based), palette (viridis, cividis, blue, orange, terrain, grey, red, rank; \", logarithmic\" for a scale of logarithms; default: by what the map is), range ([least, most] of the colours of a map of values; default: its 2nd to 98th percentiles), hidden, minus (node.port: the layer is the difference, from minus it for values, the transitions from it to from for classes), minus_band (the band or step of minus, 0-based: two layers of one stack or series), selected (of vector features: the numbers of the features selected, from 1, as the attribute table shows them; the Viewer opens the table with them selected), style (of vector features: { colour (\"#rrggbbaa\"), by (an attribute: the colours of its values; a text always as categories), ramp (of a number attribute: as palette, without \", logarithmic\"; default viridis), breaks (continuous (default: a ramp along the values, a few whole numbers a class each), equal, quantiles, natural (Jenks)), classes (2 to 12 of equal, quantiles or natural breaks; default 5), categories (true: a number attribute as categories, a colour each value), size_by (a number attribute that sizes the points, the area as the value), size_min and size_max (their size in pixels across; default 3 and 18), symbol (circle (default), square, triangle, diamond, cross), fill (0 to 1, of polygons), width (pixels), labels (an attribute), label_size (points) }) }.",
                    "items": {},
                },
                "compare": { "description": "A map compared with the top map: node.port, or { with, how, also } (how: curtain, side by side (default) or difference; also: up to 2 more maps (node.port) side by side, 4 views in all that move together; band: the band of the stack or the step of the loop `with` is (from 0, default the first); also_bands: the same for each map of also (a number each, 0 or null for the first); layers: side by side, the layers each view draws instead of the Plot's, a list per view (first the view of `with`, then those of `also`) in the form of the Plot's layers but without palette, an empty list for a view that draws the Plot's layers)." },
                "chart": { "type": "object", "description": "A table's chart: { kind (bars, lines, points, boxes), x (a column), y (columns, comma-separated), by (the column of the groups) }." },
                "view": { "type": "object", "description": "The saved view, the frame the plot is drawn at: { extent: [west, south, east, north] } in the coordinates of its map (the map's coordinate system). The Viewer opens on it, and the figure, the export (export_plot, `terraflux plot`) and the Document show only that part of the map. A plot set again without `view` loses its frame; in the Viewer, *Keep this view in the plot* sets it from what is in view." },
                "place": { "type": "string", "description": "The name of a place of the Viewer's map kept in the model (show_model lists them under [[bookmarks]]): its extent becomes the plot's saved view (in the place's own coordinates, so it fits when the plot's map has the place's coordinate system). Ignored when `view` is given." },
                "note": { "type": "string", "description": "What it shows and why (the record)." },
                "why": { "type": "string" },
            }),
            &["model", "id", "layers"],
            None,
        ),
        tool(
            "remove_plot",
            "Remove a Plot of the model.",
            json!({ "model": model, "id": { "type": "string" } }),
            &["model", "id"],
            None,
        ),
        tool(
            "set_note",
            "Add or change a note of the canvas: a text put beside the nodes (for whoever reads the model), or a frame around nodes with a title (its text's first line), the frame following them. Outside the graph: never checked nor run. With the id of one, changes what is given; without, adds one.",
            json!({
                "model": model,
                "id": { "type": "string", "description": "The note's id (show_model lists them under [[notes]]); none: a new one (note1, frame1...)." },
                "text": { "type": "string", "description": "What it says; a frame's title is its first line." },
                "nodes": { "type": "array", "items": { "type": "string" }, "description": "The ids of the nodes it frames (of its scope): a frame. Not given: a note put somewhere." },
                "at": { "type": "array", "items": { "type": "number" }, "description": "Where a note is put, [x, y] in the canvas's units (default: above the scope's nodes, at the left)." },
                "colour": { "type": "string", "enum": terraflux_graph::NOTE_COLOURS },
                "scope": { "type": "string", "description": "The container it is in (its ids from the top joined by /); none: the top level." },
            }),
            &["model"],
            None,
        ),
        tool(
            "locate_files",
            "Find again the files the model reads that are not where it says (a model from another computer, a folder moved or renamed) and make the model read them from where they are: each by the longest end of its old path found in a folder, else by its name (one file of that name), else by the change of folder of those found. Windows and Unix paths alike. Says what was found, and what is still not found with the nodes that read it.",
            json!({
                "model": model,
                "folders": { "type": "array", "items": { "type": "string" }, "description": "The folders to look in, in order (absolute, or relative to the model's folder). Default: the model's folder, then the one above it." },
            }),
            &["model"],
            None,
        ),
        tool(
            "remove_note",
            "Remove a note or a frame of the canvas (a frame's nodes stay).",
            json!({ "model": model, "id": { "type": "string" } }),
            &["model", "id"],
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
            "group_nodes",
            "Put nodes of a scope into a new group (control.group): a block of the canvas, opened by itself, and a section of the model's document. What read them from outside reads the group's outputs (named after the nodes); refused when it would make a cycle.",
            json!({
                "model": model,
                "scope": scope,
                "nodes": { "type": "array", "items": { "type": "string" }, "description": "The ids of the nodes to group." },
                "id": { "type": "string", "description": "The group's id (a stage's name: data, preparation, model, validation, results...)." },
                "note": { "type": "string", "description": "What the group is for." },
                "why": { "type": "string", "description": "Why these nodes go together." },
            }),
            &["model", "nodes", "id", "note"],
            None,
        ),
        tool(
            "ungroup",
            "Take the nodes of a group out into its scope (what read the group's outputs reads them).",
            json!({ "model": model, "scope": scope, "node": { "type": "string", "description": "The group's id." } }),
            &["model", "node"],
            None,
        ),
        tool(
            "group_by_stage",
            "Put the model's top level in a group per stage (data, preparation, model, validation, results), each with a note, and lay it out again: plots, files written and random generators stay outside. Refused when the top level has groups already.",
            json!({ "model": model }),
            &["model"],
            None,
        ),
        tool(
            "methods",
            "Draft the methods section of a paper from the model: a paragraph naming the methods it uses with the papers to cite (those of the software Terraflux reproduces), the model step by step, and the references (APA); or only the references, as BibTeX or RIS. In English or Portuguese.",
            json!({
                "model": model,
                "language": { "type": "string", "enum": ["en", "pt"], "description": "en (default) or pt." },
                "format": { "type": "string", "enum": ["text", "bibtex", "ris"], "description": "text (default): the paragraph, the steps and the references; bibtex or ris: the references." },
            }),
            &["model"],
            Some(&read_only),
        ),
        tool(
            "check_model",
            "Check the model and its data without running it: the errors and warnings of the graph, what was found in the data, and advice on the method (a simulation not validated, or validated on the map it was calibrated on; distances and areas measured on maps in degrees; metrics of a continuous map...), each with what to do and the guide that tells more.",
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
                "scenario": { "type": "string", "description": "A scenario of the model (set_scenario) to run." },
            }),
            &["model"],
            None,
        ),
        tool(
            "sensitivity",
            "Run a sensitivity analysis of the model: which settings (factors) matter to an output. The design (one at a time, Morris's elementary effects, or Sobol's variance-based indices) is run on this computer's workers (at most 4), the output measured on each run, and the indices ranked: the factors most important first, the share of the output's variance that is chance (with `replicates`), whether the indices settled (a larger design if not), a methods paragraph with its references, and the files written (design.csv, indices.csv, convergence.csv, methods.md, the chart). Morris first to screen many factors (r x (k + 1) runs), then Sobol on the few that matter (N x (k + 2) runs); replicates (3 to 5) for a model that draws at random. Designs of more than 500 runs are refused unless `confirm` is true: the reply says the count. Record the analysis in the model's notes (set_note, set_model_info).",
            json!({
                "model": model,
                "factors": {
                    "type": "array",
                    "description": "The settings to vary, each a number of the model: {key, from, to}. `key` is `node.setting` or `$input` (`container/node.setting` inside a container); `from` and `to` the range (both or none: 20 % either side of the model's value).",
                    "items": {
                        "type": "object",
                        "properties": {
                            "key": { "type": "string" },
                            "from": { "type": "number" },
                            "to": { "type": "number" },
                        },
                        "required": ["key"],
                    },
                },
                "method": { "type": "string", "enum": ["oat", "morris", "sobol"], "description": "oat: one at a time (a tornado), morris: elementary effects (screening, default), sobol: S1 and ST." },
                "n": { "type": "integer", "description": "Morris's trajectories (default 10) or Sobol's base points, a power of 2 (default 64). Not for oat." },
                "levels": { "type": "integer", "description": "Morris's levels of the grid (default 4)." },
                "second_order": { "type": "boolean", "description": "Sobol: the second order indices too (2k + 2 runs per base point, not k + 2). Default false." },
                "replicates": { "type": "integer", "description": "Runs of each design point with other seeds, the output their mean (default 1; 3 to 5 for a stochastic model). Needs a seed in the model." },
                "output": { "type": "string", "description": "What is measured, `node.port`: a number, a table's column, or a map (default: where the model ends)." },
                "statistic": { "type": "string", "description": "Of a map: mean (default), sum, min, max, cells:<class>, share:<class>, area:<class>, changed, metric:<np|pland|ed|area_mn|lpi>:<class>; of a table: its column." },
                "maps": { "type": "boolean", "description": "Maps of sensitivity, cell by cell, of an output that is a map, series or stack: `sensitivity_maps.tif` with the mean and spread of each cell across the design and the index of each factor (OAT's swing, Morris's mu*, Sobol's S1 and ST) as bands. Each run keeps its last map until the maps are made. Default false." },
                "keep_run_maps": { "type": "boolean", "description": "With `maps`: leave each run's map in its folder (default: deleted once the maps are made)." },
                "out": { "type": "string", "description": "The folder of the results (default: `<model> runs/sensitivity N` beside the model)." },
                "confirm": { "type": "boolean", "description": "Run a design of more than 500 runs." },
            }),
            &["model", "factors"],
            None,
        ),
        tool(
            "export_plot",
            "Run the model and write one of its Plots (set_plot) as a picture of the map, as the Viewer draws it from the Plot alone: the layers at their opacity, under or over the top map, the features with their style and labels; hidden layers and differences are skipped (said in the reply). Each layer in its palette (else the one its producer asks for), the classes in the colours of the model's legend. A Plot that compares maps gives its map only. No legend or decorations. Says the file, its size, the cells to a pixel, the layers drawn and not drawn.",
            json!({
                "model": model,
                "id": { "type": "string", "description": "The plot's id." },
                "file": { "type": "string", "description": "The picture to write (absolute path): .png, or .tif for a GeoTIFF of red, green, blue and alpha on the map's grid, to open in QGIS." },
                "scale": { "type": "integer", "enum": [1, 2, 4], "description": "Pixels to a cell of the map (default 1)." },
                "format": { "type": "string", "enum": ["png", "tif"], "description": "Else by the file's extension." },
                "set": { "type": "object", "description": "For this run only: input -> value or file (`seed`), node.setting -> value." },
                "scenario": { "type": "string", "description": "A scenario of the model (set_scenario) to run." },
            }),
            &["model", "id", "file"],
            None,
        ),
        tool(
            "set_parameter",
            "Offer a value of the model in its form of parameters (the application's Parameters pane), to change a scenario without touching the graph: an input of a value (`$name`) or a node's setting or literal input (`node.setting`, `container/node.setting`), with its label, unit, range, step, choices and help; `value` sets it too. Called again, it changes what is given.",
            json!({
                "model": model,
                "key": { "type": "string", "description": "`$name` or `node.setting`." },
                "label": { "type": "string" },
                "unit": { "type": "string" },
                "min": { "type": "number" },
                "max": { "type": "number" },
                "step": { "type": "number" },
                "choices": { "type": "array", "description": "The values it takes, when they are a few." },
                "help": { "type": "string", "description": "What it is and how it acts on the model." },
                "value": { "description": "Its value now." },
            }),
            &["model", "key"],
            None,
        ),
        tool(
            "remove_parameter",
            "Take a value out of the model's form of parameters (the model keeps the value).",
            json!({ "model": model, "key": { "type": "string" } }),
            &["model", "key"],
            None,
        ),
        tool(
            "set_scenario",
            "Keep a scenario: a name, a note and the values it gives (key -> value, as set_parameter's keys; none: the parameters' values now). run_model with `scenario` runs it; the application compares scenarios side by side.",
            json!({
                "model": model,
                "name": { "type": "string" },
                "note": { "type": "string", "description": "What the scenario stands for." },
                "values": { "type": "object" },
            }),
            &["model", "name"],
            None,
        ),
        tool(
            "remove_scenario",
            "Forget a scenario of the model.",
            json!({ "model": model, "name": { "type": "string" } }),
            &["model", "name"],
            None,
        ),
        tool(
            "trace_cell",
            "Why a cell of a result has its value: what a node gives at a place and the chain of values that made it, back through the graph (map algebra's expressions with the values there, the cells around for a neighbourhood, the layers a model weighs, a loop's value at every step). Give row and col of the node's map, or x and y in its coordinates. Explain from it, not from memory.",
            json!({
                "model": model,
                "node": { "type": "string", "description": "A top-level node (a group: its output, traced into its body)." },
                "port": { "type": "string", "description": "Its output (default: its first map)." },
                "row": { "type": "integer" },
                "col": { "type": "integer" },
                "x": { "type": "number" },
                "y": { "type": "number" },
                "inside_loops": { "type": "boolean", "description": "Run the model again keeping its loops' values at the cell, step by step (slower)." },
            }),
            &["model", "node"],
            Some(&read_only),
        ),
        tool(
            "last_trace",
            "The trace of the cell the user traced last in the application (a right click on a cell of a map, Trace this cell): the model's file, then why the cell has its value, node by node. Explain it to the user, or trace more with trace_cell.",
            json!({}),
            &[],
            Some(&read_only),
        ),
        tool(
            "list_runs",
            "The runs of the model kept in its journal (the application, terraflux run and this server write it), newest first: when, how long, how it ended, what it was (the model, up to a node, a batch), the scenario, the values changed from the run before and the outputs with whether they are still as written. `limit` (default 20); with `id`, one run in full (every value, the outputs, the warnings, the batch's factors ranked).",
            json!({
                "model": model,
                "limit": { "type": "integer", "description": "How many runs, the newest first (default 20)." },
                "id": { "type": "integer", "description": "A run's id (from the list): that run in full." },
            }),
            &["model"],
            Some(&read_only),
        ),
        tool(
            "save_version",
            "Save the model as it is now as a named version, kept beside its file (in the .terraflux folder, with the journal of the runs): a point to go back to or to review against, with no git. Do it before a large change; the user sees the versions in the Runs pane's Versions tab and can restore one or review what changed since. A version of a model unchanged since the newest one is saved too.",
            json!({
                "model": model,
                "label": { "type": "string", "description": "The version's name, e.g. \"before changing the scenario\"." },
                "note": { "type": "string", "description": "Optional: why, in a sentence." },
            }),
            &["model", "label"],
            None,
        ),
        tool(
            "list_versions",
            "The named versions of the model (save_version, the application's Save a version...), newest first: id, label, when, note, where it was saved from and what changed from the version before (nodes added, removed and changed, other changes).",
            json!({ "model": model }),
            &["model"],
            Some(&read_only),
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
            "Open the model in Terraflux's application, for the user to see it, run it and change it: in the one already open (in a tab of its own, or its tab when open there already), else in a new window. The application follows the file: later edits appear there as they are made.",
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

/// A design of more runs than this is run only when the call says `confirm`.
pub const BIG_DESIGN: usize = 500;

/// What the tool `sensitivity` is asked (its arguments, checked).
#[derive(Debug, Clone, PartialEq)]
pub struct SensitivityAsk {
    /// The factors: the setting, and its range (none: 20 % either side of its value).
    pub factors: Vec<(String, Option<f64>, Option<f64>)>,
    pub method: terraflux_dist::sensitivity::Method,
    pub n: Option<usize>,
    pub levels: usize,
    pub second_order: bool,
    pub replicates: usize,
    pub output: Option<String>,
    pub statistic: Option<String>,
    pub out: Option<PathBuf>,
    pub confirm: bool,
    /// The maps of sensitivity, cell by cell, and whether the runs' maps stay.
    pub maps: bool,
    pub keep_run_maps: bool,
}

impl SensitivityAsk {
    /// The design of the call: `n` by the method, the levels, the second order, the seed 1.
    pub fn design(&self) -> terraflux_dist::sensitivity::Design {
        terraflux_dist::sensitivity::Design {
            n: self.n.unwrap_or(self.method.default_n()),
            levels: self.levels,
            second_order: self.second_order,
            seed: 1,
        }
    }
}

/// The arguments of `sensitivity`.
pub fn sensitivity_ask(a: &Value) -> std::result::Result<SensitivityAsk, String> {
    use terraflux_dist::sensitivity::Method;
    let number = |v: &Value, what: &str| -> std::result::Result<Option<f64>, String> {
        match v {
            Value::Null => Ok(None),
            v => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| format!("{what} must be a number")),
        }
    };
    let list = a.get("factors").and_then(Value::as_array).filter(|l| !l.is_empty());
    let list = list.ok_or("give the `factors`: a list of {key, from, to}, each a number setting of the model")?;
    let mut factors: Vec<(String, Option<f64>, Option<f64>)> = Vec::new();
    for f in list {
        let (key, from, to) = match f {
            Value::String(k) => (k.as_str(), None, None),
            Value::Object(_) => (
                f["key"].as_str().ok_or("a factor needs its `key` (`node.setting` or `$input`)")?,
                number(&f["from"], "`from`")?,
                number(&f["to"], "`to`")?,
            ),
            _ => return Err("each factor is {key, from, to}".into()),
        };
        let key = key.trim();
        if factors.iter().any(|(k, _, _)| k == key) {
            return Err(format!("{key}: given twice"));
        }
        factors.push((key.to_string(), from, to));
    }
    let method = match opt_text(a, "method") {
        Some(m) => Method::parse(m).map_err(|e| e.to_string())?,
        None => Method::Morris,
    };
    let count = |key: &str, default: usize| -> std::result::Result<usize, String> {
        match a.get(key) {
            None | Some(Value::Null) => Ok(default),
            Some(v) => v.as_u64().filter(|n| *n >= 1).map(|n| n as usize).ok_or(format!("`{key}` must be 1 or more")),
        }
    };
    let n = match a.get("n") {
        None | Some(Value::Null) => None,
        Some(_) => Some(count("n", 1)?),
    };
    Ok(SensitivityAsk {
        factors,
        method,
        n,
        levels: count("levels", 4)?,
        second_order: a.get("second_order").and_then(Value::as_bool).unwrap_or(false),
        replicates: count("replicates", 1)?,
        output: opt_text(a, "output").map(str::trim).filter(|s| !s.is_empty()).map(String::from),
        statistic: opt_text(a, "statistic").map(str::trim).filter(|s| !s.is_empty()).map(String::from),
        out: opt_text(a, "out").map(absolute),
        confirm: a.get("confirm").and_then(Value::as_bool).unwrap_or(false),
        maps: a.get("maps").and_then(Value::as_bool).unwrap_or(false),
        keep_run_maps: a.get("keep_run_maps").and_then(Value::as_bool).unwrap_or(false),
    })
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

/// The templates that match a query (in English or Portuguese), the best first; all of them, by group, without
/// one.
pub fn find_templates(query: Option<&str>) -> String {
    let words = query.map(crate::search::words).unwrap_or_default();
    if words.is_empty() {
        return list_templates();
    }
    let all: &[crate::templates::Template] = &crate::templates::TEMPLATES;
    let (found, every) =
        crate::search::rank(all, &words, |t| format!("{} {} {}", t.name, t.about, t.group), |t| t.name.to_string());
    if found.is_empty() {
        return format!("No template matches {:?}; the templates:\n{}", query.unwrap_or_default(), list_templates());
    }
    let mut s = if every {
        String::new()
    } else {
        format!("No template has all of {:?}; those nearest:\n", query.unwrap_or_default())
    };
    for t in found.iter().take(12) {
        s.push_str(&format!("{} ({}): {}\n", t.name, t.group, t.about));
    }
    s.push_str("new_model with `template` copies one; guide tells the method of its field.\n");
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
    // (the view.plot: a Plot now, set_plot)
    sigs.retain(|s| s.name != terraflux_graph::PLOT_OP);
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
            out.push_str(&mine_listed(None));
            out
        }
        Some(q) => {
            let words = crate::search::words(q);
            let (found, all) =
                crate::search::rank(&sigs, &words, |s| format!("{} {}", s.name, s.doc), |s| s.name.clone());
            let mine = mine_listed(Some(&words));
            if found.is_empty() && mine.is_empty() {
                return format!("No operator has any of {q:?}: try other words (or guide, the methods of each field).");
            }
            let mut out = mine;
            if !all && !found.is_empty() {
                out.push_str(&format!("No operator has all of {q:?}; those that have the most of its words:\n"));
            }
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

/// Save a group or a calculator of a model open (its values of the last run giving its types) as a node of the
/// user's libraries: what was saved, and where.
pub fn save_my_node(s: &mut crate::app::Session, r: &Registry, a: &Value) -> Reply {
    let id = text(a, "node")?;
    let path = scope(a);
    let mut d = crate::my_nodes::draft(s, r, &path, id)?;
    if let Some(name) = opt_text(a, "name") {
        d.name = name.trim().to_string();
    }
    if let Some(about) = opt_text(a, "description") {
        d.description = about.trim().to_string();
    }
    let all = crate::my_nodes::libraries();
    if let Some(lib) = opt_text(a, "library") {
        let wanted = lib.trim().to_lowercase();
        let shelf = all
            .iter()
            .find(|l| crate::my_nodes::slug(l) == wanted || l.name.to_lowercase() == wanted)
            .ok_or_else(|| {
                let known: Vec<String> = all.iter().map(crate::my_nodes::slug).collect();
                format!("no library {lib:?} (the libraries: {})", known.join(", "))
            })?;
        d.library = shelf.lib.dir.clone();
    }
    let at = all.iter().position(|l| l.lib.dir == d.library);
    let u = crate::my_nodes::save(s, r, &d)?;
    let lib = at.map_or_else(|| d.library.display().to_string(), |k| all[k].name.clone());
    let reached = crate::my_nodes::reached_as(at.unwrap_or(0), &u);
    let takes: Vec<String> = u.inputs.iter().map(|(k, i)| format!("{k}: {}", i.summary())).collect();
    let gives: Vec<String> = u.outputs.iter().map(|(k, o)| format!("{k}: {}", o.ty.name())).collect();
    Ok(format!(
        "saved {}@{} in {lib}: add it to any model as {}{reached} (takes {}; gives {}); {id} is now that version",
        u.name,
        u.version,
        crate::my_nodes::PREFIX,
        if takes.is_empty() { "nothing".into() } else { takes.join(", ") },
        if gives.is_empty() { "nothing".into() } else { gives.join(", ") },
    ))
}

/// The nodes of the user's libraries (those that have all the words, when given), a line each, for
/// list_operators: empty when there are none.
fn mine_listed(words: Option<&[String]>) -> String {
    let found: Vec<(String, terraflux_graph::UserNode)> = crate::my_nodes::reachable()
        .into_iter()
        .filter(|(_, u)| {
            let hay = format!("{} {}", u.name, u.description.clone().unwrap_or_default()).to_lowercase();
            words.is_none_or(|w| w.iter().all(|w| hay.contains(w.as_str())))
        })
        .collect();
    if found.is_empty() {
        return String::new();
    }
    let p = crate::my_nodes::PREFIX;
    let mut out = format!(
        "Nodes of the user's libraries (add_node with op = \"{p}name\"; describe_operator tells what they take):\n"
    );
    for (name, u) in found {
        let what = u.description.as_deref().map_or(String::new(), |d| format!(": {}", first_sentence(d)));
        out.push_str(&format!("{p}{name} (version {}){what}\n", u.version));
    }
    out
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
    if let Some((mine, version)) = crate::my_nodes::named(name) {
        return crate::my_nodes::get(mine, version).map(|u| crate::my_nodes::describe(&u));
    }
    let sig = r.sig(name).ok_or_else(|| format!("no operator {name:?}{}", like(r, name)))?;
    let mut s = format!("{}\n{}\n", sig.name, sig.doc);
    if sig.name == terraflux_graph::PLOT_OP {
        s.push_str(
            "\nRetired: a figure is a Plot now, kept with the model outside the graph (set_plot, remove_plot); the \
             view.plot nodes of older models are made Plots when they are read.\n",
        );
    }
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
    s.push_str(&crate::op_notes::notes(name, &sig.advice));
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
    // (a node of the user's: its settings are its inputs of a value)
    let declared: Vec<String> = m
        .scope(path)
        .and_then(|n| n.iter().find(|n| n.id == id))
        .and_then(|n| n.uses.as_ref())
        .map_or_else(Vec::new, |u| u.inputs.keys().cloned().collect());
    for (name, value) in map {
        // (a literal input given as a setting: set as the input it is)
        if sig.as_ref().is_some_and(|s| s.input_spec(name).is_some()) || declared.contains(name) {
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
    // a calculator's: what each output of its script is, from what its inputs are given
    let scripted = terraflux_graph::script_types(m, r, path, id).unwrap_or_default();
    let ports: Vec<String> = editor::output_ports(n, r)
        .into_iter()
        .map(|(p, t)| {
            let t = scripted.iter().find(|(o, _)| *o == p).map_or(t, |(_, t)| *t);
            format!("{id}.{p} ({})", t.name())
        })
        .collect();
    if ports.is_empty() { "no outputs yet".into() } else { format!("its outputs: {}", ports.join(", ")) }
}

fn add_node(m: &mut Model, r: &Registry, a: &Value) -> Reply {
    let op = text(a, "op")?;
    if op == terraflux_graph::PLOT_OP {
        return Err("view.plot is no longer a node: a figure is a Plot, kept with the model (set_plot: its layers, \
                    title, comparison, a table's chart)"
            .into());
    }
    let path = scope(a);
    let mut id = match crate::my_nodes::named(op) {
        // a node of the user's libraries
        Some((name, version)) => editor::add_user_node(m, &path, &crate::my_nodes::get(name, version)?, [0.0, 0.0])?,
        None if r.sig(op).is_none() => return Err(format!("no operator {op:?}{}", like(r, op))),
        None => editor::add_node(m, r, &path, op, [0.0, 0.0])?,
    };
    if let Some(want) = opt_text(a, "id") {
        editor::rename_node(m, &path, &id, want)?;
        id = want.to_string();
    }
    if let Some(note) = opt_text(a, "note") {
        editor::set_note(m, &path, &id, note)?;
    }
    set_record(m, &path, &id, a)?;
    set_settings(m, r, &path, &id, a.get("settings"))?;
    set_inputs(m, r, &path, &id, a.get("inputs"))?;
    place(m, &path, &id);
    let n = m.scope(&path).and_then(|n| n.iter().find(|n| n.id == id)).expect("the node added");
    let missing: Vec<&str> =
        [("a note", n.note.is_none()), ("a why", n.why.is_none())].into_iter().filter(|x| x.1).map(|x| x.0).collect();
    let ask = if missing.is_empty() {
        String::new()
    } else {
        format!(" Give it {} (edit_node): the model is the record of its choices.", missing.join(" and "))
    };
    Ok(format!("added {id} ({op}); {}.{ask}", outputs_of(m, r, &path, &id)))
}

/// A list of texts given: a list, or a text of a line each (null, empty: none).
fn texts(v: &Value, key: &str) -> std::result::Result<Vec<String>, String> {
    let all: Vec<String> = match v {
        Value::Null => Vec::new(),
        Value::String(s) => s.lines().map(String::from).collect(),
        Value::Array(items) => items
            .iter()
            .map(|x| match x {
                Value::String(s) => Ok(s.clone()),
                other => Err(format!("`{key}` is a list of texts, not {other}")),
            })
            .collect::<std::result::Result<_, _>>()?,
        other => return Err(format!("`{key}` is a list of texts, not {other}")),
    };
    Ok(all.into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
}

/// A node's record given (`why`, `refs`, `assumptions`, `alternatives`; empty: none).
fn set_record(m: &mut Model, path: &[String], id: &str, a: &Value) -> std::result::Result<(), String> {
    let n = m.scope_mut(path).and_then(|n| n.iter_mut().find(|n| n.id == id)).ok_or("no such node")?;
    if let Some(w) = a.get("why") {
        let w = w.as_str().ok_or("`why` is a text")?.trim();
        n.why = (!w.is_empty()).then(|| w.to_string());
    }
    for (key, list) in
        [("refs", &mut n.refs), ("assumptions", &mut n.assumptions), ("alternatives", &mut n.alternatives)]
    {
        if let Some(v) = a.get(key) {
            *list = texts(v, key)?;
        }
    }
    Ok(())
}

/// What the model says of itself, given (`name`, `description`, `request`, `summary`, `author`; empty: none):
/// the fields set.
pub fn model_info(m: &mut Model, a: &Value) -> Vec<&'static str> {
    let mut set = Vec::new();
    for (key, field) in [
        ("name", &mut m.name),
        ("description", &mut m.description),
        ("request", &mut m.request),
        ("summary", &mut m.summary),
        ("author", &mut m.author),
    ] {
        if let Some(t) = a.get(key).and_then(Value::as_str) {
            let t = t.trim();
            *field = (!t.is_empty()).then(|| t.to_string());
            set.push(key);
        }
    }
    set
}

/// The nodes (of every scope; `outer/inner` for one in a body) made here, not copied from a template, that do not
/// say what they are for (a note), and those that do not say why (a why).
pub fn unrecorded(m: &Model) -> (Vec<String>, Vec<String>) {
    use std::collections::HashSet;
    use std::sync::OnceLock;
    type Copied = HashSet<(String, String)>;
    static COPIED: OnceLock<Copied> = OnceLock::new();
    fn gather(nodes: &[Node], into: &mut Copied) {
        for n in nodes {
            into.insert((n.id.clone(), n.op.clone()));
            if let Some(b) = &n.body {
                gather(&b.nodes, into);
            }
        }
    }
    fn walk(nodes: &[Node], at: &str, copied: &Copied, no_note: &mut Vec<String>, no_why: &mut Vec<String>) {
        for n in nodes {
            let id = if at.is_empty() { n.id.clone() } else { format!("{at}/{}", n.id) };
            if !copied.contains(&(n.id.clone(), n.op.clone())) {
                if n.note.is_none() {
                    no_note.push(id.clone());
                }
                if n.why.is_none() {
                    no_why.push(id.clone());
                }
            }
            if let Some(b) = &n.body {
                walk(&b.nodes, &id, copied, no_note, no_why);
            }
        }
    }
    let copied = COPIED.get_or_init(|| {
        let mut all = HashSet::new();
        for t in crate::templates::TEMPLATES {
            gather(&t.model().nodes, &mut all);
        }
        all
    });
    let (mut no_note, mut no_why) = (Vec::new(), Vec::new());
    walk(&m.nodes, "", copied, &mut no_note, &mut no_why);
    (no_note, no_why)
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
    set_record(m, &path, &id, a)?;
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

/// A Plot added or replaced ([`terraflux_graph::Plot`]): its layers read outputs of top-level nodes (or inputs).
fn set_plot(m: &mut Model, r: &Registry, a: &Value) -> Reply {
    use terraflux_graph::{Plot, PlotChart, PlotCompare, PlotLayer, PlotView};
    let id = text(a, "id")?;
    if !valid_id(id) {
        return Err(format!("{id:?} is not a valid id (a letter, then letters, digits, _ or -)"));
    }
    // (a reference to an output of a top-level node, or an input)
    let checked = |from: &str| -> std::result::Result<String, String> {
        match Ref::parse(from) {
            Ok(Ref::Input(name)) if m.inputs.contains_key(&name) => Ok(from.to_string()),
            Ok(Ref::Input(name)) => Err(format!("the model has no input ${name}")),
            Ok(Ref::Port { node, port }) => {
                let n = m.node(&node).ok_or_else(|| format!("no node {node:?} at the top level"))?;
                let ports = editor::output_ports(n, r);
                if ports.iter().any(|(p, _)| *p == port) {
                    Ok(from.to_string())
                } else {
                    let names: Vec<&str> = ports.iter().map(|(p, _)| p.as_str()).collect();
                    Err(format!("{node} has no output {port:?} (its outputs: {})", names.join(", ")))
                }
            }
            _ => Err(format!("{from:?}: give node.port of a top-level node, or $input")),
        }
    };
    // (a layer: node.port, or an object; the layers of a view side by side have no palette: a map has one in all
    // the views)
    let layer_of = |l: &Value, view: bool| -> std::result::Result<PlotLayer, String> {
        Ok(match l {
            Value::String(from) => PlotLayer::new(&checked(from)?),
            Value::Object(o) => {
                let from = o.get("from").and_then(Value::as_str).ok_or("a layer as an object needs `from`")?;
                let opacity = o.get("opacity").and_then(Value::as_f64);
                if opacity.is_some_and(|x| !(0.0..=1.0).contains(&x)) {
                    return Err(format!("{from}: opacity is 0 to 1"));
                }
                let palette = o.get("palette").and_then(Value::as_str).filter(|p| !p.trim().is_empty());
                if view && palette.is_some() {
                    return Err(format!("{from}: a view's layer has no palette (the Plot's layer of it sets it)"));
                }
                if let Some(p) = palette
                    && crate::builder::palette_named(p).is_none()
                {
                    let names: Vec<&str> = crate::builder::RAMPS.iter().map(|(n, _)| *n).collect();
                    return Err(format!(
                        "{from}: no palette {p:?}; one of {} (\", logarithmic\" for a scale of logarithms)",
                        names.join("; ")
                    ));
                }
                // (the range of the colours: two numbers, the least first)
                let range = match o.get("range") {
                    None | Some(Value::Null) => None,
                    Some(Value::Array(v)) => match v.as_slice() {
                        [a, b] if a.as_f64().zip(b.as_f64()).is_some_and(|(a, b)| a < b) => {
                            Some([a.as_f64().unwrap_or(0.0), b.as_f64().unwrap_or(1.0)])
                        }
                        _ => return Err(format!("{from}: range is [least, most], two numbers, the least first")),
                    },
                    Some(other) => return Err(format!("{from}: range is [least, most], not {other}")),
                };
                let minus = match o.get("minus").and_then(Value::as_str).filter(|m| !m.is_empty()) {
                    Some(m) => Some(checked(m)?),
                    None => None,
                };
                if let Some(st) = o.get("style").filter(|s| !s.is_null()) {
                    // (the symbology of vectors: a word it does not know is refused, not drawn as the default)
                    let word = |k: &str| st.get(k).and_then(Value::as_str);
                    if let Some(b) = word("breaks").filter(|b| crate::vector_tiles::Breaks::named(b).is_none()) {
                        return Err(format!(
                            "{from}: style.breaks {b:?}; one of continuous, equal, quantiles, natural"
                        ));
                    }
                    if let Some(s) = word("symbol").filter(|s| crate::vector_tiles::Symbol::named(s).is_none()) {
                        return Err(format!(
                            "{from}: style.symbol {s:?}; one of circle, square, triangle, diamond, cross"
                        ));
                    }
                    if let Some(p) = word("ramp")
                        && !matches!(crate::builder::palette_named(p), Some(crate::builder::Palette::Chosen(..)))
                    {
                        let names: Vec<&str> = crate::builder::RAMPS.iter().map(|(n, _)| *n).collect();
                        return Err(format!("{from}: style.ramp {p:?}; one of {}", names.join("; ")));
                    }
                    if let Some(c) = st.get("classes").filter(|c| c.as_u64().is_none_or(|c| !(2..=12).contains(&c))) {
                        return Err(format!("{from}: style.classes is a whole number from 2 to 12, not {c}"));
                    }
                }
                // (the features selected: their numbers from 1)
                let selected = match o.get("selected") {
                    None | Some(Value::Null) => Vec::new(),
                    Some(Value::Array(v)) => v
                        .iter()
                        .map(|k| k.as_u64().filter(|k| *k > 0).map(|k| k as usize))
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| format!("{from}: selected is a list of feature numbers from 1"))?,
                    Some(other) => {
                        return Err(format!("{from}: selected is a list of feature numbers from 1, not {other}"));
                    }
                };
                PlotLayer {
                    from: checked(from)?,
                    opacity: opacity.filter(|x| *x != 1.0),
                    band: o.get("band").and_then(Value::as_u64).map(|b| b as usize),
                    // (the band of the map subtracted: only with one)
                    minus_band: o
                        .get("minus_band")
                        .and_then(Value::as_u64)
                        .map(|b| b as usize)
                        .filter(|_| minus.is_some()),
                    hidden: o.get("hidden").and_then(Value::as_bool).unwrap_or(false),
                    style: o.get("style").cloned(),
                    palette: palette.and_then(crate::builder::palette_named).and_then(crate::builder::palette_name),
                    range,
                    minus,
                    rgb: match o.get("rgb") {
                        None | Some(Value::Null) => None,
                        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| {
                            format!(
                                "{from}: rgb is {{ \"bands\": [r, g, b], \"red\": [least, most], \"gamma\": 1 }}: {e}"
                            )
                        })?),
                    },
                    selected,
                    blend: match o.get("blend") {
                        None | Some(Value::Null) => None,
                        Some(Value::String(b)) => match crate::blend::Blend::named(b) {
                            Some(crate::blend::Blend::Normal) => None,
                            Some(m) => Some(m.key().to_string()),
                            None => return Err(format!("{from}: blend is normal, multiply or screen, not {b:?}")),
                        },
                        Some(b) => return Err(format!("{from}: blend is normal, multiply or screen, not {b}")),
                    },
                    shade: match o.get("shade") {
                        None | Some(Value::Null) => None,
                        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| {
                            format!(
                                "{from}: shade is {{ \"azimuth\": 315, \"altitude\": 45, \"z\": 1, \"strength\": 0.6 }}: {e}"
                            )
                        })?),
                    },
                }
            }
            other => return Err(format!("a layer is node.port or an object with `from`, not {other}")),
        })
    };
    let list = a.get("layers").and_then(Value::as_array).ok_or("give `layers`: what it shows, the lowest first")?;
    let layers = list.iter().map(|l| layer_of(l, false)).collect::<std::result::Result<Vec<_>, _>>()?;
    if layers.is_empty() {
        return Err("give at least one layer".into());
    }
    let compare = match a.get("compare") {
        None | Some(Value::Null) => None,
        Some(Value::String(w)) if w.is_empty() => None,
        Some(Value::String(w)) => Some(PlotCompare { with: checked(w)?, ..Default::default() }),
        Some(Value::Object(o)) => {
            let with = o.get("with").and_then(Value::as_str).ok_or("compare needs `with`: node.port")?;
            let how = o.get("how").and_then(Value::as_str).map(str::to_string);
            if let Some(h) = &how
                && !["curtain", "side by side", "difference"].contains(&h.as_str())
            {
                return Err(format!("compare's how is curtain, side by side or difference, not {h:?}"));
            }
            let also = match o.get("also") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(a)) => {
                    let names: Option<Vec<&str>> = a.iter().map(Value::as_str).collect();
                    names
                        .ok_or("compare's also is a list of node.port")?
                        .into_iter()
                        .map(checked)
                        .collect::<Result<_, _>>()?
                }
                Some(other) => return Err(format!("compare's also is a list of node.port, not {other}")),
            };
            if !also.is_empty() && how.as_deref().is_some_and(|h| h != "side by side") {
                return Err("compare's also shows more maps side by side: how is side by side".into());
            }
            if also.len() > 2 {
                return Err(format!("compare's also has at most 2 maps (4 views in all), not {}", also.len()));
            }
            // (the layers of the views: a list each, the view of `with` first)
            let mut views: Vec<Vec<PlotLayer>> = Vec::new();
            match o.get("layers") {
                None | Some(Value::Null) => {}
                Some(Value::Array(lists)) => {
                    if lists.len() > also.len() + 1 {
                        return Err(format!(
                            "compare's layers has a list for each view (with, then also): at most {}, not {}",
                            also.len() + 1,
                            lists.len()
                        ));
                    }
                    for (v, list) in lists.iter().enumerate() {
                        let list = list.as_array().ok_or("compare's layers is a list of lists of layers")?;
                        let layers = list.iter().map(|l| layer_of(l, true));
                        views.push(
                            layers
                                .collect::<std::result::Result<_, _>>()
                                .map_err(|e| format!("compare.layers[{v}]: {e}"))?,
                        );
                    }
                    if how.as_deref().is_some_and(|h| h != "side by side") && views.iter().any(|v| !v.is_empty()) {
                        return Err("compare's layers are the layers of views side by side: how is side by side".into());
                    }
                    while views.last().is_some_and(|v| v.is_empty()) {
                        views.pop();
                    }
                }
                Some(other) => return Err(format!("compare's layers is a list of lists of layers, not {other}")),
            }
            // (the band of the stack or the step of the loop each map is: `with`'s, then one for each of `also`)
            let band = match o.get("band") {
                None | Some(Value::Null) => None,
                Some(b) => {
                    Some(b.as_u64().ok_or_else(|| format!("compare's band is a number from 0, not {b}"))? as usize)
                }
            };
            let mut also_bands: Vec<usize> = match o.get("also_bands") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|b| match b {
                        Value::Null => Ok(0),
                        b => b
                            .as_u64()
                            .map(|b| b as usize)
                            .ok_or_else(|| format!("compare's also_bands are numbers from 0 (or null), not {b}")),
                    })
                    .collect::<Result<_, _>>()?,
                Some(other) => return Err(format!("compare's also_bands is a list of numbers, not {other}")),
            };
            if also_bands.len() > also.len() {
                return Err(format!(
                    "compare's also_bands has a band for each map of also: at most {}, not {}",
                    also.len(),
                    also_bands.len()
                ));
            }
            while also_bands.last() == Some(&0) {
                also_bands.pop();
            }
            Some(PlotCompare { with: checked(with)?, how, also, band, also_bands, layers: views })
        }
        Some(other) => return Err(format!("compare is node.port or {{ with, how, also }}, not {other}")),
    };
    let chart = a.get("chart").and_then(Value::as_object).map(|c| {
        let s = |k: &str| c.get(k).and_then(Value::as_str).map(String::from);
        PlotChart { kind: s("kind"), x: s("x"), y: s("y"), by: s("by") }
    });
    let view = match a.get("view") {
        None | Some(Value::Null) => match opt_text(a, "place") {
            // the extent of a place of the Viewer's map kept in the model
            Some(name) => {
                let place = m.bookmarks_of(true).find(|p| p.name == name).ok_or_else(|| {
                    let known: Vec<&str> = m.bookmarks_of(true).map(|p| p.name.as_str()).collect();
                    format!("no place of the Viewer named {name:?} (the model's: {})", known.join(", "))
                })?;
                match &place.view {
                    terraflux_graph::PlaceView::Map { extent, .. } => {
                        let [w, s, e, n] = *extent;
                        Some(PlotView::new(w, s, e, n).ok_or("the place has no area")?)
                    }
                    _ => None,
                }
            }
            None => None,
        },
        Some(v) => {
            let list = v.get("extent").and_then(Value::as_array);
            let nums: Option<Vec<f64>> = list.and_then(|l| l.iter().map(Value::as_f64).collect());
            match nums.as_deref() {
                Some(&[w, s, e, n]) => Some(
                    PlotView::new(w, s, e, n).ok_or("view's extent needs an area: west < east and south < north")?,
                ),
                _ => return Err("view is { extent: [west, south, east, north] }, four numbers".into()),
            }
        }
    };
    let plot = Plot {
        id: id.to_string(),
        title: opt_text(a, "title").map(String::from),
        layers,
        compare,
        chart,
        note: opt_text(a, "note").map(String::from),
        why: opt_text(a, "why").map(String::from),
        view,
    };
    let shown: Vec<&str> = plot.layers.iter().map(|l| l.from.as_str()).collect();
    let mut said = format!("{} ({})", id, shown.join(", "));
    if plot.view.is_some() {
        said.push_str(", with a saved view");
    }
    match m.plots.iter_mut().find(|p| p.id == id) {
        Some(p) => {
            *p = plot;
            Ok(format!("replaced the plot {said}"))
        }
        None => {
            m.plots.push(plot);
            Ok(format!("added the plot {said}: in the Viewer's Plots, a figure of the Document and the report"))
        }
    }
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
            n.wires.clear();
        }
        if let Some(b) = &mut n.body {
            arrange(&mut b.nodes, r);
        }
    }
}

/// Save a named version of the model (`label`, `note`): `open`, the model as the application has it on screen, else
/// the model's file as written. Kept beside the file, with the journal of the runs.
pub fn save_version(model_file: Option<&Path>, open: Option<&Model>, a: &Value, from: &str) -> Reply {
    use terraflux_exec::journal::Journal;
    let file = model_file.ok_or("the model has no file yet: save it first (versions are kept beside the file)")?;
    let journal = Journal::of(file).ok_or("the model has no file name")?;
    let label = a.get("label").and_then(Value::as_str).map(str::trim).filter(|l| !l.is_empty());
    let label = label.ok_or("`label` is the version's name (a text)")?;
    let note = a.get("note").and_then(Value::as_str);
    let loaded;
    let model = match open {
        Some(m) => m,
        None => {
            loaded = Model::load(file).map_err(|e| format!("{}: {e}", file.display()))?;
            &loaded
        }
    };
    let v = journal.save_version(label, note, model, from).map_err(|e| e.to_string())?;
    let n = journal.versions().len();
    Ok(format!(
        "Version \"{}\" saved (id {}, {n} version(s) of {}). The user sees it in the Runs pane's Versions tab and can restore it or review what changed since.",
        v.label,
        v.id,
        file.display()
    ))
}

/// The named versions of the model, newest first: each with when, its note, where it was saved from and what changed
/// from the version before. Read-only.
pub fn list_versions(model_file: Option<&Path>) -> Reply {
    use terraflux_exec::journal::Journal;
    let file = model_file.ok_or("the model has no file yet: save it first (versions are kept beside the file)")?;
    let journal = Journal::of(file).ok_or("the model has no file name")?;
    let rows = crate::versions_view::rows(&journal);
    if rows.is_empty() {
        return Ok(format!("No version saved yet for {} (save_version saves one).", file.display()));
    }
    let mut t = format!("Versions of {}, newest first ({}).\n", file.display(), rows.len());
    for row in rows.iter().rev() {
        let v = &row.version;
        let when = crate::eta::stamp(v.when).unwrap_or_else(|| v.when.to_string());
        t.push_str(&format!("- id {} | \"{}\" | {when} | from the {}\n", v.id, v.label, v.from));
        if let Some(n) = &v.note {
            t.push_str(&format!("  note: {n}\n"));
        }
        match (row.changes, row.kept) {
            (Some(c), _) => t.push_str(&format!("  changed from the version before: {}\n", c.text())),
            (None, false) => t.push_str("  (its snapshot is gone)\n"),
            (None, true) => t.push_str("  (the first version)\n"),
        }
    }
    Ok(t)
}

/// The runs the model's journal has kept (`.terraflux` beside its file), newest first: each in a few lines (when, how
/// long, how it ended, what it was, the scenario, the values changed from the run before, the outputs and whether
/// they are still as written). `limit` of them (default 20); with `id`, that one in full. Read-only.
pub fn list_runs(model_file: Option<&Path>, a: &Value) -> Reply {
    use terraflux_exec::journal::{Journal, Kind, Status};
    let file = model_file.ok_or("the model has no file yet: save it first (the journal is kept beside the file)")?;
    let journal = Journal::of(file).ok_or("the model has no file name")?;
    let entries = journal.entries();
    if entries.is_empty() {
        return Ok(format!(
            "No run kept yet for {} (each run asked for is recorded beside the model).",
            file.display()
        ));
    }
    let when = |e: &terraflux_exec::journal::Entry| crate::eta::stamp(e.when).unwrap_or_else(|| e.when.to_string());
    let status = |e: &terraflux_exec::journal::Entry| match &e.status {
        Status::Done => "done".to_string(),
        Status::Stopped => "stopped".to_string(),
        Status::Failed(m) => format!("failed: {m}"),
    };
    let outputs = |e: &terraflux_exec::journal::Entry| -> String {
        e.outputs
            .iter()
            .map(|w| format!("{} ({}, {})", w.name, w.path, crate::runs_view::state_of(w)))
            .collect::<Vec<_>>()
            .join("; ")
    };
    if let Some(id) = a.get("id") {
        let id = id
            .as_u64()
            .or_else(|| id.as_str().and_then(|s| s.trim().parse().ok()))
            .ok_or("`id` is a run's id (a number)")?;
        let at = entries.iter().position(|e| e.id == id).ok_or_else(|| format!("no run {id} (list_runs)"))?;
        let e = &entries[at];
        let mut t = format!(
            "Run {} of {}\nwhen: {}\ntook: {:.1} s\nstatus: {}\nwhat: {}\nscenario: {}\nterraflux: {} (from the {})\n",
            e.id,
            file.display(),
            when(e),
            e.seconds,
            status(e),
            crate::runs_view::kind_text(&e.kind),
            e.scenario.as_deref().unwrap_or("none"),
            e.terraflux,
            e.from
        );
        t.push_str(&format!(
            "model: {} ({})\n",
            e.model,
            if journal.model(&e.model).is_some() { "kept" } else { "snapshot gone" }
        ));
        if let Some(m) = &e.manifest {
            t.push_str(&format!("manifest: {}\n", m.display()));
        }
        t.push_str("values:\n");
        for (k, v) in &e.values {
            t.push_str(&format!("  {k} = {v}\n"));
        }
        if let Some(before) = at.checked_sub(1).map(|p| &entries[p]) {
            let c = crate::runs_view::changes(&before.values, &e.values);
            t.push_str(&format!(
                "changed from the run before: {}\n",
                if c.is_empty() { "none".into() } else { c.join(", ") }
            ));
        }
        t.push_str("outputs:\n");
        for w in &e.outputs {
            t.push_str(&format!("  {}: {} ({} bytes, {})\n", w.name, w.path, w.size, crate::runs_view::state_of(w)));
        }
        if !e.warnings.is_empty() {
            t.push_str(&format!("warnings:\n  {}\n", e.warnings.join("\n  ")));
        }
        if let Kind::Batch(b) = &e.kind {
            t.push_str(&format!("batch: {} runs, {} failed, results in {}\n", b.runs, b.failed, b.out.display()));
            for f in &b.factors {
                match f.index {
                    Some(i) => t.push_str(&format!("  factor {}: {} {i:.3}\n", f.key, f.by)),
                    None => t.push_str(&format!("  factor {}\n", f.key)),
                }
            }
        }
        return Ok(t);
    }
    let limit = a.get("limit").and_then(Value::as_u64).map_or(20, |n| n.max(1) as usize);
    let mut t = format!(
        "Runs of {}, newest first ({} of {}); list_runs with `id` tells one in full.\n",
        file.display(),
        limit.min(entries.len()),
        entries.len()
    );
    for (at, e) in entries.iter().enumerate().rev().take(limit) {
        t.push_str(&format!(
            "- id {} | {} | took {} | {} | {} | scenario: {}\n",
            e.id,
            when(e),
            crate::eta::took(e.seconds),
            status(e),
            crate::runs_view::kind_text(&e.kind),
            e.scenario.as_deref().unwrap_or("none")
        ));
        match at.checked_sub(1).map(|p| &entries[p]) {
            Some(before) => {
                let c = crate::runs_view::changes(&before.values, &e.values);
                t.push_str(&format!(
                    "  values changed from the run before: {}\n",
                    if c.is_empty() { "none".into() } else { c.join(", ") }
                ));
                if before.model != e.model {
                    t.push_str("  the model changed from the run before\n");
                }
            }
            None => t.push_str("  (the first run kept)\n"),
        }
        if !e.outputs.is_empty() {
            t.push_str(&format!("  outputs: {}\n", outputs(e)));
        }
    }
    Ok(t)
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

/// The classes of a map of classes (a band of whole numbers with 2 to 24 values, no data apart), sorted; none when
/// the file is not one (or cannot be read).
pub fn classes_of(p: &Path, band: Option<usize>) -> Option<Vec<i64>> {
    let r = terraflux_ops::files::Rasters::open(&p.to_string_lossy(), None, None).ok()?;
    let data = r.read(band.unwrap_or(0)).ok()?;
    let mut seen = std::collections::BTreeSet::new();
    for v in data.iter().filter(|v| v.is_finite()) {
        if v.fract() != 0.0 {
            return None;
        }
        seen.insert(*v as i64);
        if seen.len() > 24 {
            return None;
        }
    }
    (seen.len() >= 2).then(|| seen.into_iter().collect())
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
    s.push_str(&grid_notes(g));
    Ok(s)
}

/// What a raster's grid asks for before it is used: a coordinate system of degrees, or none at all.
fn grid_notes(g: &terraflux_raster::Grid) -> String {
    let crs = terraflux_ops::prepare::crs_of(g);
    let geographic =
        g.geo.is_geographic().or_else(|| crs.as_deref().and_then(|c| terraflux_gdal::is_geographic(c).ok()));
    match (geographic, &crs) {
        (Some(true), _) => {
            "note: its coordinates are longitudes and latitudes: distances, areas and landscape metrics \
                            need a projected coordinate system (raster.reproject first; guide data-preparation)\n"
                .into()
        }
        (_, None) if g.geo.geokeys.is_none() => "note: no coordinate system is known: give it (raster.match's \
                                                 source_crs) before putting it with other maps\n"
            .into(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod plot_tests {
    use serde_json::json;
    use terraflux_graph::{Model, Node, PlotLayer};

    fn model() -> Model {
        let mut m = Model::new("plots");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 10).param("cols", 10));
        m.nodes.push(Node::new("forest", "raster.reclassify").input("map", "land.map").param("rules", "1 -> 1"));
        m
    }

    /// A Plot's saved view set by set_plot: four numbers with an area, kept in the file, lost when set again without.
    #[test]
    fn a_plot_keeps_its_view() {
        let r = terraflux_ops::Registry::standard();
        let dir = std::path::Path::new(".");
        let mut m = model();
        let a = json!({ "id": "cover", "layers": ["land.map"], "view": { "extent": [10.0, 20.0, 30, 40] } });
        let said = super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert!(said.contains("saved view"), "{said}");
        assert_eq!(m.plot("cover").unwrap().view, terraflux_graph::PlotView::new(10.0, 20.0, 30.0, 40.0));
        let shown = super::show_model(&m, dir, &r).unwrap();
        assert!(shown.contains("extent"), "{shown}");
        for bad in [json!({ "extent": [1, 2, 3] }), json!({ "extent": [5, 0, 5, 9] }), json!({ "extent": "all" })] {
            let a = json!({ "id": "cover", "layers": ["land.map"], "view": bad });
            assert!(super::edit("set_plot", &mut m, &r, dir, &a).is_err(), "{bad}");
        }
        let a = json!({ "id": "cover", "layers": ["land.map"] });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert!(m.plot("cover").unwrap().view.is_none());
    }

    /// A place of the Viewer's map names the plot's saved view; show_model lists the places; an unknown place, or one
    /// of the canvas, is refused.
    #[test]
    fn a_plot_takes_the_view_of_a_place() {
        use terraflux_graph::{Place, PlaceView};
        let r = terraflux_ops::Registry::standard();
        let dir = std::path::Path::new(".");
        let mut m = model();
        let map = PlaceView::Map { extent: [1.0, 2.0, 3.0, 4.0], crs: Some("EPSG:4326".into()), layers: None };
        m.save_bookmark(Place { name: "Lake".into(), slot: Some(2), view: map });
        m.save_bookmark(Place {
            name: "Inputs".into(),
            slot: None,
            view: PlaceView::Canvas { scope: String::new(), x: 1.0, y: 2.0, zoom: 1.0 },
        });
        let a = json!({ "id": "cover", "layers": ["land.map"], "place": "Lake" });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert_eq!(m.plot("cover").unwrap().view, terraflux_graph::PlotView::new(1.0, 2.0, 3.0, 4.0));
        // (an explicit view wins)
        let a = json!({ "id": "cover", "layers": ["land.map"], "place": "Lake", "view": { "extent": [5, 6, 7, 8] } });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert_eq!(m.plot("cover").unwrap().view, terraflux_graph::PlotView::new(5.0, 6.0, 7.0, 8.0));
        for bad in ["Nowhere", "Inputs"] {
            let a = json!({ "id": "cover", "layers": ["land.map"], "place": bad });
            let e = super::edit("set_plot", &mut m, &r, dir, &a).unwrap_err();
            assert!(e.contains("Lake"), "{e}");
        }
        let shown = super::show_model(&m, dir, &r).unwrap();
        assert!(shown.contains("[[bookmarks]]") && shown.contains("- Lake: Viewer, extent"), "{shown}");
        assert!(shown.contains("slot 2") && shown.contains("- Inputs: canvas of the top level"), "{shown}");
    }

    /// The symbology of a vector layer in set_plot: its style kept as given (and read by the Viewer), a word the Viewer
    /// does not know refused.
    #[test]
    fn a_plot_keeps_the_symbology_of_its_vectors() {
        let r = terraflux_ops::Registry::standard();
        let dir = std::path::Path::new(".");
        let mut m = model();
        let style = json!({
            "by": "area", "ramp": "cividis", "breaks": "quantiles", "classes": 4,
            "size_by": "pop", "size_min": 4, "size_max": 20, "symbol": "triangle",
        });
        let a = json!({ "id": "v", "layers": ["land.map", { "from": "land.map", "style": style }] });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        let kept = m.plot("v").unwrap().layers[1].style.clone().expect("the style kept");
        let s = crate::vector_tiles::Style::from_json(&kept);
        assert_eq!(
            (s.ramp, s.breaks, s.classes, s.symbol, s.size_by.as_deref()),
            (
                crate::palette::Ramp::Cividis,
                crate::vector_tiles::Breaks::Quantiles,
                4,
                crate::vector_tiles::Symbol::Triangle,
                Some("pop")
            )
        );
        for bad in [
            json!({ "breaks": "wavy" }),
            json!({ "symbol": "star" }),
            json!({ "ramp": "rainbow" }),
            json!({ "classes": 40 }),
        ] {
            let a = json!({ "id": "v", "layers": [{ "from": "land.map", "style": bad }] });
            assert!(super::edit("set_plot", &mut m, &r, dir, &a).is_err(), "{bad}");
        }
    }

    /// The features selected of a layer in set_plot: their numbers from 1 kept in the Plot; none by default; a zero or
    /// a word refused.
    #[test]
    fn a_plot_keeps_the_selected_features() {
        let r = terraflux_ops::Registry::standard();
        let dir = std::path::Path::new(".");
        let mut m = model();
        let a = json!({ "id": "s", "layers": ["land.map", { "from": "land.map", "selected": [3, 1, 2] }] });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        let p = m.plot("s").unwrap();
        assert!(p.layers[0].selected.is_empty());
        assert_eq!(p.layers[1].selected, [3, 1, 2]);
        let a = json!({ "id": "s", "layers": [{ "from": "land.map" }] });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert!(m.plot("s").unwrap().layers[0].selected.is_empty());
        for bad in [json!([0]), json!(["a"]), json!(-1), json!([1.5])] {
            let a = json!({ "id": "s", "layers": [{ "from": "land.map", "selected": bad }] });
            assert!(super::edit("set_plot", &mut m, &r, dir, &a).is_err(), "{bad}");
        }
    }

    /// A Plot added and replaced by set_plot, its references checked; removed by remove_plot; no view.plot node.
    #[test]
    fn plots_are_set_and_removed() {
        let r = terraflux_ops::Registry::standard();
        let dir = std::path::Path::new(".");
        let mut m = model();
        let a = json!({
            "id": "cover",
            "title": "The forest over the land",
            "layers": ["land.map", { "from": "forest.map", "opacity": 0.5 }],
            "compare": { "with": "land.map", "how": "curtain" },
        });
        let said = super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert!(said.contains("added the plot cover"), "{said}");
        let p = m.plot("cover").unwrap();
        assert_eq!(
            p.layers,
            [PlotLayer::new("land.map"), PlotLayer { opacity: Some(0.5), ..PlotLayer::new("forest.map") }]
        );
        assert_eq!(p.compare.as_ref().unwrap().how.as_deref(), Some("curtain"));
        assert!(terraflux_graph::check(&m, &r).iter().all(|i| !i.at.starts_with("plots.")));
        // replaced
        let said =
            super::edit("set_plot", &mut m, &r, dir, &json!({ "id": "cover", "layers": ["forest.map"] })).unwrap();
        assert!(said.starts_with("replaced"), "{said}");
        assert_eq!(m.plots.len(), 1);
        assert!(m.plot("cover").unwrap().compare.is_none());
        // more maps side by side: four views
        let a = json!({
            "id": "cover",
            "layers": ["land.map"],
            "compare": { "with": "forest.map", "also": ["land.map", "forest.map"] },
        });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert_eq!(m.plot("cover").unwrap().compare.as_ref().unwrap().also, ["land.map", "forest.map"]);
        // ... each map a band of its stack (trailing nulls not kept)
        let a = json!({
            "id": "cover",
            "layers": ["land.map"],
            "compare": { "with": "forest.map", "band": 3, "also": ["land.map", "forest.map"], "also_bands": [null, 1] },
        });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        let c = m.plot("cover").unwrap().compare.clone().unwrap();
        assert_eq!((c.band, c.also_bands), (Some(3), vec![0, 1]));
        let a = json!({
            "id": "cover",
            "layers": ["land.map"],
            "compare": { "with": "forest.map", "also": ["land.map", "forest.map"], "also_bands": [2, null] },
        });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert_eq!(m.plot("cover").unwrap().compare.as_ref().unwrap().also_bands, [2]);
        for bad in [
            json!({ "with": "forest.map", "band": "first" }),
            json!({ "with": "forest.map", "also": ["land.map"], "also_bands": [1, 2] }),
            json!({ "with": "forest.map", "also": ["land.map"], "also_bands": [-1] }),
        ] {
            let a = json!({ "id": "cover", "layers": ["land.map"], "compare": bad });
            assert!(super::edit("set_plot", &mut m, &r, dir, &a).is_err(), "{a}");
        }
        // ... each view with layers of its own (an empty list: the Plot's layers)
        let a = json!({
            "id": "cover",
            "layers": ["land.map"],
            "compare": { "with": "forest.map", "also": ["land.map"], "layers": [
                ["forest.map", { "from": "land.map", "opacity": 0.5, "minus": "forest.map" }],
                [],
            ] },
        });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        let c = m.plot("cover").unwrap().compare.clone().unwrap();
        assert_eq!(c.layers.len(), 1, "the empty list at the end is not kept");
        assert_eq!(c.layers[0][0], PlotLayer::new("forest.map"));
        assert_eq!((c.layers[0][1].opacity, c.layers[0][1].minus.as_deref()), (Some(0.5), Some("forest.map")));
        assert!(terraflux_graph::check(&m, &r).iter().all(|i| !i.at.starts_with("plots.")));
        // a layer that is a difference
        let a = json!({ "id": "cover", "layers": ["land.map", { "from": "forest.map", "minus": "land.map" }] });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert_eq!(m.plot("cover").unwrap().layers[1].minus.as_deref(), Some("land.map"));
        // ... of two layers of one stack or series (the band of the map subtracted kept with it only)
        let a = json!({ "id": "cover", "layers": [
            { "from": "forest.map", "band": 4, "minus": "forest.map", "minus_band": 0 },
            { "from": "land.map", "minus_band": 2 },
        ] });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        let l = &m.plot("cover").unwrap().layers;
        assert_eq!((l[0].band, l[0].minus_band), (Some(4), Some(0)));
        assert_eq!((l[1].minus.as_deref(), l[1].minus_band), (None, None));
        let a = json!({ "id": "cover", "layers": ["land.map", { "from": "forest.map", "minus": "land.map" }] });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        assert!(terraflux_graph::check(&m, &r).iter().all(|i| !i.at.starts_with("plots.")));
        // a palette by its name in the Viewer, or its first word
        let a = json!({ "id": "cover", "layers": [{ "from": "forest.map", "palette": "Cividis, log" }] });
        super::edit("set_plot", &mut m, &r, dir, &a).unwrap();
        let kept = m.plot("cover").unwrap().layers[0].palette.clone();
        assert_eq!(kept.as_deref(), Some("cividis (for colour-blind readers), logarithmic"));
        // what is not there, refused
        for bad in [
            json!({ "id": "x", "layers": ["gone.map"] }),
            json!({ "id": "x", "layers": ["land.nothing"] }),
            json!({ "id": "x", "layers": [] }),
            json!({ "id": "x", "layers": ["land.map"], "compare": { "with": "land.map", "how": "sideways" } }),
            json!({ "id": "1x", "layers": ["land.map"] }),
            json!({ "id": "x", "layers": ["land.map"], "compare": { "with": "land.map", "also": ["gone.map"] } }),
            json!({ "id": "x", "layers": ["land.map"], "compare": { "with": "land.map", "how": "curtain", "also": ["land.map"] } }),
            json!({ "id": "x", "layers": ["land.map"], "compare": { "with": "land.map", "also": ["land.map", "land.map", "land.map"] } }),
            json!({ "id": "x", "layers": ["land.map"], "compare": { "with": "land.map", "layers": [["gone.map"]] } }),
            json!({ "id": "x", "layers": ["land.map"], "compare": { "with": "land.map", "layers": [[], ["land.map"]] } }),
            json!({ "id": "x", "layers": ["land.map"], "compare": { "with": "land.map", "layers": [["land.map"]], "how": "curtain" } }),
            json!({ "id": "x", "layers": ["land.map"], "compare": { "with": "land.map", "layers": [[{ "from": "land.map", "palette": "viridis" }]] } }),
            json!({ "id": "x", "layers": [{ "from": "land.map", "palette": "rainbow" }] }),
            json!({ "id": "x", "layers": [{ "from": "land.map", "minus": "gone.map" }] }),
        ] {
            assert!(super::edit("set_plot", &mut m, &r, dir, &bad).is_err(), "{bad}");
        }
        // a view.plot node: a Plot instead
        let e = super::edit("add_node", &mut m, &r, dir, &json!({ "op": "view.plot" })).unwrap_err();
        assert!(e.contains("set_plot"), "{e}");
        assert!(!super::list_operators(&r, None).contains("view.plot"));
        // removed
        super::edit("remove_plot", &mut m, &r, dir, &json!({ "id": "cover" })).unwrap();
        assert!(m.plots.is_empty());
        assert!(super::edit("remove_plot", &mut m, &r, dir, &json!({ "id": "cover" })).is_err());
    }
    /// A Plot exported by its id: the model run, the picture written as PNG and as GeoTIFF on the map's grid.
    #[test]
    fn a_plot_is_exported_by_its_id() {
        let dir = std::env::temp_dir().join(format!("terraflux-export-plot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut m = Model::new("exported");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 8).param("columns", 12).param("seed", 3));
        m.nodes.push(Node::new("high", "raster.calc").input("x", "land.map").param("expression", "x > 0.5"));
        let r = terraflux_ops::Registry::standard();
        let a = json!({
            "id": "cover",
            "layers": [{ "from": "land.map", "palette": "grey" }, { "from": "high.map", "opacity": 0.5 }],
            "compare": { "with": "land.map" },
        });
        super::edit("set_plot", &mut m, &r, &dir, &a).unwrap();
        let engine = terraflux_exec::Engine::default();
        let mut w = super::Watcher::new(None, None);
        let png = dir.join("cover.png");
        let a = json!({ "id": "cover", "file": png.to_string_lossy(), "scale": 2 });
        let said = super::export_plot(&engine, &m, &dir, &a, &mut w).unwrap();
        // (the Plot compares side by side: the PNG has the two views, the map and the one compared, apart by the gutter)
        let g = crate::export_view::GUTTER as u32;
        assert!(said.contains(&format!("{} x 16 px", 24 + g + 24)) && said.contains("2 pixels to a cell"), "{said}");
        assert!(said.contains("land.map") && said.contains("high.map at 50%"), "{said}");
        assert!(said.contains("2 views side by side") && said.contains("GeoTIFF only the map"), "{said}");
        let back = tiny_skia::Pixmap::load_png(&png).unwrap();
        assert_eq!((back.width(), back.height()), (24 + g + 24, 16));
        // a GeoTIFF of four bands on the map's grid, its cells half as large
        let tif = dir.join("cover.tif");
        let a = json!({ "id": "cover", "file": tif.to_string_lossy() });
        let said = super::export_plot(&engine, &m, &dir, &a, &mut w).unwrap();
        assert!(
            said.contains("12 x 8 px") && said.contains("1 pixel to a cell") && said.contains("one grid"),
            "{said}"
        );
        let t = terraflux_raster::tiff::read(&tif).unwrap();
        assert_eq!(t.bands.len(), 4);
        assert_eq!((t.grid.width, t.grid.height), (12, 8));
        // refused: another scale, another extension, a plot the model has not
        for (bad, why) in [
            (json!({ "id": "cover", "file": png.to_string_lossy(), "scale": 3 }), "scale"),
            (json!({ "id": "cover", "file": dir.join("c.jpg").to_string_lossy() }), "not a .png"),
            (json!({ "id": "nope", "file": png.to_string_lossy() }), "no plot"),
        ] {
            let e = super::export_plot(&engine, &m, &dir, &bad, &mut w).unwrap_err();
            assert!(e.contains(why), "{e}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A note put by an agent (above the nodes when not placed), a frame around nodes, changed, refused when it
    /// frames a node not there or has a colour not known, removed.
    #[test]
    fn notes_and_frames_are_set_and_removed() {
        let r = terraflux_ops::Registry::standard();
        let dir = std::path::Path::new(".");
        let mut m = model();
        let ids: Vec<String> = m.nodes.iter().map(|n| n.id.clone()).collect();
        let said = super::edit("set_note", &mut m, &r, dir, &json!({ "text": "Start here" })).unwrap();
        assert!(said.contains("note1"), "{said}");
        assert!(m.notes[0].at.is_some() && !m.notes[0].is_frame());
        let a = json!({ "text": "Inputs", "nodes": [ids[0]], "colour": "blue" });
        super::edit("set_note", &mut m, &r, dir, &a).unwrap();
        assert_eq!(m.notes[1].id, "frame1");
        assert_eq!(m.notes[1].nodes, [ids[0].clone()]);
        super::edit("set_note", &mut m, &r, dir, &json!({ "id": "frame1", "text": "The inputs" })).unwrap();
        assert_eq!((m.notes[1].title(), m.notes[1].colour.as_deref()), ("The inputs", Some("blue")));
        assert!(super::edit("set_note", &mut m, &r, dir, &json!({ "nodes": ["gone"] })).is_err());
        assert!(super::edit("set_note", &mut m, &r, dir, &json!({ "colour": "pink" })).is_err());
        assert!(terraflux_graph::check(&m, &r).iter().all(|i| !i.at.contains("note")), "never checked");
        assert!(super::show_model(&m, dir, &r).unwrap().contains("[[notes]]"));
        super::edit("remove_note", &mut m, &r, dir, &json!({ "id": "note1" })).unwrap();
        assert_eq!(m.notes.len(), 1);
        assert!(super::edit("remove_note", &mut m, &r, dir, &json!({ "id": "note1" })).is_err());
    }

    /// A file not where the model says found by an agent in a folder given (relative to the model's), the model
    /// reading it from there; a folder not there refused.
    #[test]
    fn files_not_found_are_located_by_an_agent() {
        let r = terraflux_ops::Registry::standard();
        let dir = std::env::temp_dir().join(format!("terraflux-locate-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("moved")).unwrap();
        std::fs::write(dir.join("moved").join("dem.tif"), b"x").unwrap();
        let mut m = Model::new("m");
        m.nodes.push(terraflux_graph::Node::new("dem", "raster.read").param("path", "/old/place/dem.tif"));
        let said = super::edit("locate_files", &mut m, &r, &dir, &json!({ "folders": ["moved"] })).unwrap();
        assert!(said.contains("1 of 1"), "{said}");
        assert_eq!(m.nodes[0].params["path"], json!("moved/dem.tif"));
        assert!(super::edit("locate_files", &mut m, &r, &dir, &json!({})).unwrap().contains("every file"));
        assert!(super::edit("locate_files", &mut m, &r, &dir, &json!({ "folders": ["nowhere"] })).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod sensitivity_tests {
    use serde_json::json;
    use terraflux_dist::sensitivity::Method;

    use terraflux_graph::Model;

    use super::{catalogue, list_runs, sensitivity_ask};

    #[test]
    fn the_arguments_of_a_sensitivity_analysis_are_read_and_checked() {
        let a = sensitivity_ask(&json!({
            "factors": [{ "key": "a.b", "from": 1, "to": 3 }, "$c", { "key": "d.e", "from": 2 }],
        }))
        .unwrap();
        assert_eq!(a.factors[0], ("a.b".to_string(), Some(1.0), Some(3.0)));
        assert_eq!(a.factors[1], ("$c".to_string(), None, None));
        assert_eq!(a.factors[2], ("d.e".to_string(), Some(2.0), None));
        // Morris, 10 trajectories on 4 levels, one replicate, by default
        assert_eq!(
            (a.method, a.n, a.levels, a.replicates, a.second_order, a.confirm),
            (Method::Morris, None, 4, 1, false, false)
        );
        assert_eq!(a.design().n, 10);
        let s = sensitivity_ask(&json!({ "factors": ["a.b"], "method": "Sobol", "n": 128, "confirm": true })).unwrap();
        assert_eq!((s.method, s.design().n, s.confirm), (Method::Sobol, 128, true));
        for bad in [
            json!({}),
            json!({ "factors": [] }),
            json!({ "factors": [3] }),
            json!({ "factors": [{ "from": 1 }] }),
            json!({ "factors": ["a.b", "a.b"] }),
            json!({ "factors": ["a.b"], "method": "fast" }),
            json!({ "factors": ["a.b"], "replicates": 0 }),
            json!({ "factors": [{ "key": "a.b", "from": "low" }] }),
        ] {
            assert!(sensitivity_ask(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_journal_is_listed_newest_first_and_one_run_in_full() {
        use terraflux_exec::journal::{Entry, Journal, Kind, Status, Written};
        let d = std::env::temp_dir().join(format!("terraflux-list-runs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let file = d.join("m.toml");
        assert!(list_runs(None, &json!({})).is_err());
        assert!(list_runs(Some(&file), &json!({})).unwrap().starts_with("No run kept yet"));
        let model = Model::from_toml("format = \"terraflux/1\"").unwrap();
        let out = d.join("land.tif");
        std::fs::write(&out, "one").unwrap();
        let j = Journal::of(&file).unwrap();
        for (k, rate) in [1, 2, 3].into_iter().enumerate() {
            let mut e = Entry::new(Kind::Run, "app", 1.0);
            e.id = 1_000 + k as u64;
            e.values = [("$rate".to_string(), json!(rate))].into();
            if k == 0 {
                e.outputs = vec![Written::of("land", &out, None, None).unwrap()];
            }
            if k == 2 {
                e.status = Status::Failed("no data".into());
                e.scenario = Some("dry".into());
            }
            j.record(e, &model).unwrap();
        }
        std::fs::write(&out, "another one").unwrap();
        let all = list_runs(Some(&file), &json!({})).unwrap();
        assert!(all.contains("(3 of 3)"), "{all}");
        assert!(all.find("id 1002").unwrap() < all.find("id 1001").unwrap());
        assert!(all.find("id 1001").unwrap() < all.find("id 1000").unwrap());
        assert!(all.contains("failed: no data") && all.contains("scenario: dry"), "{all}");
        assert!(all.contains("$rate 2 -> 3") && all.contains("(the first run kept)"), "{all}");
        assert!(all.contains("land.tif, overwritten since"), "{all}");
        let two = list_runs(Some(&file), &json!({ "limit": 2 })).unwrap();
        assert!(two.contains("(2 of 3)") && !two.contains("id 1000"), "{two}");
        let one = list_runs(Some(&file), &json!({ "id": 1002 })).unwrap();
        assert!(one.contains("status: failed: no data") && one.contains("$rate = 3"), "{one}");
        assert!(one.contains("changed from the run before: $rate 2 -> 3") && one.contains("(kept)"), "{one}");
        assert!(list_runs(Some(&file), &json!({ "id": 5 })).unwrap_err().contains("no run 5"));
        // (the assistant in the application names no model: the open one)
        let app = catalogue(true).into_iter().find(|t| t["name"] == "list_runs").unwrap();
        assert!(app["inputSchema"]["properties"].get("model").is_none());
        assert!(catalogue(false).iter().any(|t| t["name"] == "list_runs"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn versions_are_saved_and_listed_newest_first() {
        use super::{list_versions, save_version};
        let d = std::env::temp_dir().join(format!("terraflux-versions-tool-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let file = d.join("m.toml");
        assert!(list_versions(None).is_err() && save_version(None, None, &json!({ "label": "a" }), "mcp").is_err());
        assert!(list_versions(Some(&file)).unwrap().starts_with("No version saved yet"));
        let mut model = Model::from_toml("format = \"terraflux/1\"").unwrap();
        model.save(&file).unwrap();
        // (from the file as written, when no model is on screen)
        let said = save_version(Some(&file), None, &json!({ "label": "before", "note": "the first" }), "mcp").unwrap();
        assert!(said.contains("\"before\" saved"), "{said}");
        assert!(save_version(Some(&file), None, &json!({}), "mcp").unwrap_err().contains("label"));
        assert!(save_version(Some(&file), None, &json!({ "label": "  " }), "mcp").is_err());
        // (the model on screen, not the file)
        model.nodes.push(terraflux_graph::Node::new("land", "nlm.random"));
        save_version(Some(&file), Some(&model), &json!({ "label": "after" }), "mcp").unwrap();
        let t = list_versions(Some(&file)).unwrap();
        assert!(t.contains("(2)") && t.find("\"after\"").unwrap() < t.find("\"before\"").unwrap(), "{t}");
        assert!(t.contains("note: the first") && t.contains("(the first version)"), "{t}");
        assert!(t.contains("changed from the version before: 1 node added"), "{t}");
        let app = catalogue(true).into_iter().find(|t| t["name"] == "save_version").unwrap();
        assert!(app["inputSchema"]["properties"].get("model").is_none());
        assert_eq!(app["inputSchema"]["required"], json!(["label"]));
        assert!(catalogue(false).iter().any(|t| t["name"] == "list_versions"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_assistant_in_the_application_sets_the_analysis_up_instead_of_running_it() {
        let find = |all: &[serde_json::Value]| all.iter().find(|t| t["name"] == "sensitivity").cloned().unwrap();
        let mcp = find(&catalogue(false));
        let app = find(&catalogue(true));
        assert!(mcp["inputSchema"]["properties"].get("model").is_some());
        assert!(mcp["inputSchema"]["properties"].get("confirm").is_some());
        let p = &app["inputSchema"]["properties"];
        assert!(p.get("model").is_none() && p.get("out").is_none() && p.get("confirm").is_none());
        assert!(p.get("factors").is_some() && app["description"].as_str().unwrap().contains("the user presses Run"));
        assert_eq!(app["inputSchema"]["required"], json!(["factors"]));
    }
}
