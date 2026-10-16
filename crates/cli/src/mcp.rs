//! `terraflux mcp`: Terraflux as a server of the Model Context Protocol (MCP), on the standard input and output.
//! An assistant (Claude Desktop, Claude Code, any client of MCP) builds a model through it as the node editor
//! does: the templates and the operators listed and described, nodes added, connected and set (each change
//! checked, a wrong one refused with the checker's reason and the file left as it was), the model checked, run,
//! and its results looked at. The model is a file: the application, with it open, shows every change as it is
//! written ([`terraflux_app::watch`]).

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use terraflux_app::{editor, editor_view};
use terraflux_exec::{Engine, Observer, load_inputs, write_outputs};
use terraflux_graph::{Input, Model, Node, OpSignature, Output, PortSpec, Ref, Severity, check, valid_id};
use terraflux_ops::Registry;

/// The versions of the protocol spoken (the newest first): the client's, when it is one of them.
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What the assistant is told of Terraflux when it connects.
const INSTRUCTIONS: &str = "Terraflux builds and runs models of land use and land cover change, landscape \
ecology, connectivity and conservation planning. A model is a file (.toml or .json) holding a graph of operators; \
build and change it with these tools, not by writing the file.

- A node runs one operator (`op`) and gives outputs (its ports). An input of a node reads `node.port` (another \
node's output), `$name` (an input of the model), `@name` (inside a container: a value carried along a loop, \
`@step`, a group's input) or is a literal value. Settings are the operator's options.
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
- Containers (control.repeat: a loop of steps; control.group) hold a body of nodes: give `scope` (the \
container's id; `outer/inner` for one in another's body) to edit inside it, and edit_container for what the body \
carries, collects or gives.";

type Reply = std::result::Result<String, String>;

/// The server's output, shared by the runs that answer from their threads: a message per line.
type Out = Arc<Mutex<std::io::Stdout>>;

fn send(out: &Out, v: &Value) -> std::io::Result<()> {
    let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
    writeln!(o, "{v}")?;
    o.flush()
}

/// The server: the requests answered in their order, but run_model, which runs in a thread of its own (the other
/// tools answered meanwhile), tells the client how it goes (progress notifications, when it gave a token) and stops
/// when the client cancels it.
pub fn serve() -> anyhow::Result<()> {
    let server = Arc::new(Server { engine: Engine::default() });
    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let runs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>> = Arc::default();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            // (a batch, as the protocol's first versions allowed)
            Ok(Value::Array(all)) => {
                let r: Vec<Value> = all.into_iter().filter_map(|m| server.message(m)).collect();
                (!r.is_empty()).then_some(Value::Array(r))
            }
            // a run stopped by the client
            Ok(m) if m["method"] == "notifications/cancelled" => {
                if let Some(stop) = runs.lock().unwrap().get(&m["params"]["requestId"].to_string()) {
                    stop.store(true, Ordering::Relaxed);
                }
                None
            }
            Ok(m) if m["method"] == "tools/call" && m["params"]["name"] == "run_model" && !m["id"].is_null() => {
                let (server, out, runs) = (server.clone(), out.clone(), runs.clone());
                let key = m["id"].to_string();
                let stop = Arc::new(AtomicBool::new(false));
                runs.lock().unwrap().insert(key.clone(), stop.clone());
                std::thread::spawn(move || {
                    let token = m["params"]["_meta"]["progressToken"].clone();
                    let notify = (!token.is_null()).then(|| Notify { token, out: out.clone(), sent: 0, told: None });
                    let mut kept = Kept { warnings: Vec::new(), notify, stop: Some(stop) };
                    let reply = server.answer(&m, &mut kept);
                    runs.lock().unwrap().remove(&key);
                    if let Some(r) = reply {
                        let _ = send(&out, &r);
                    }
                });
                None
            }
            Ok(m) => server.message(m),
            Err(e) => Some(failure(Value::Null, -32700, &format!("not JSON: {e}"))),
        };
        if let Some(r) = reply {
            send(&out, &r)?;
        }
    }
    Ok(())
}

fn failure(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

struct Server {
    engine: Engine,
}

impl Server {
    /// The answer to a request; none to a notification (or to an answer of the client's).
    fn message(&self, m: Value) -> Option<Value> {
        self.answer(&m, &mut Kept::default())
    }

    /// The answer to a request, a run telling `kept` how it goes.
    fn answer(&self, m: &Value, kept: &mut Kept) -> Option<Value> {
        let id = m.get("id").cloned()?;
        let method = m.get("method").and_then(Value::as_str)?;
        let params = m.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => Ok(initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => self.call(&params, kept),
            _ => Err((-32601, format!("no method {method:?}"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => failure(id, code, &message),
        })
    }

    fn call(&self, params: &Value, kept: &mut Kept) -> std::result::Result<Value, (i64, String)> {
        let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
        if !tools().iter().any(|t| t["name"] == name) {
            return Err((-32602, format!("no tool {name:?}")));
        }
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        // (a failure of an operator ends the call, not the server)
        let (text, error) = match std::panic::catch_unwind(AssertUnwindSafe(|| self.tool(name, &args, kept))) {
            Ok(Ok(t)) => (t, false),
            Ok(Err(e)) => (e, true),
            Err(p) => {
                let why = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                (format!("Terraflux failed: {why}"), true)
            }
        };
        Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": error }))
    }

    fn tool(&self, name: &str, a: &Value, kept: &mut Kept) -> Reply {
        let r = &self.engine.registry;
        match name {
            "list_templates" => Ok(list_templates()),
            "list_operators" => Ok(list_operators(r, opt_text(a, "query"))),
            "describe_operator" => describe_operator(r, text(a, "name")?),
            "new_model" => new_model(r, a),
            "show_model" => {
                let path = model_path(a)?;
                let m = load(&path)?;
                Ok(format!("{}\n{}", m.to_toml().map_err(|e| e.to_string())?, problems(&m, &path, r)))
            }
            "set_input" => edit(a, r, |m, _, dir| set_input(m, a, dir)),
            "remove_input" => edit(a, r, |m, _, _| {
                let name = text(a, "name")?.trim_start_matches('$');
                m.inputs.remove(name).ok_or_else(|| format!("the model has no input ${name}"))?;
                Ok(format!("removed the input ${name}"))
            }),
            "add_node" => edit(a, r, |m, r, _| add_node(m, r, a)),
            "edit_node" => edit(a, r, |m, r, _| edit_node(m, r, a)),
            "remove_node" => edit(a, r, |m, _, _| {
                let id = text(a, "node")?;
                editor::remove_node(m, &scope(a), id)?;
                Ok(format!("removed {id} (what read it reads nothing now)"))
            }),
            "edit_container" => edit(a, r, |m, r, _| edit_container(m, r, a)),
            "set_output" => edit(a, r, |m, r, _| set_output(m, r, a)),
            "remove_output" => edit(a, r, |m, _, _| {
                let name = text(a, "name")?;
                m.outputs.remove(name).ok_or_else(|| format!("the model has no output {name:?}"))?;
                Ok(format!("removed the output {name}"))
            }),
            "arrange" => edit(a, r, |m, r, _| {
                arrange(&mut m.nodes, r);
                Ok("the nodes put in tidy columns".into())
            }),
            "check_model" => check_model(&self.engine, a),
            "run_model" => run_model(&self.engine, a, kept),
            "inspect_file" => inspect_file(a),
            "open_in_app" => open_in_app(a),
            _ => Err(format!("no tool {name:?}")),
        }
    }
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
    let version = VERSIONS.iter().find(|v| **v == asked).unwrap_or(&VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "terraflux", "title": "Terraflux", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

/// The tools, with what they take.
fn tools() -> Vec<Value> {
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

fn text<'a>(a: &'a Value, key: &str) -> std::result::Result<&'a str, String> {
    a.get(key).and_then(Value::as_str).ok_or_else(|| format!("give `{key}` (a text)"))
}

fn opt_text<'a>(a: &'a Value, key: &str) -> Option<&'a str> {
    a.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty())
}

fn absolute(p: &str) -> PathBuf {
    let p = PathBuf::from(p);
    if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) }
}

fn model_path(a: &Value) -> std::result::Result<PathBuf, String> {
    let p = absolute(text(a, "model")?);
    if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml") || e.eq_ignore_ascii_case("json")) {
        return Err(format!("{}: a model is a .toml or .json file", p.display()));
    }
    Ok(p)
}

/// The path of containers' ids of `scope` (none: the top level).
fn scope(a: &Value) -> Vec<String> {
    opt_text(a, "scope").map(|s| s.split('/').filter(|x| !x.is_empty()).map(String::from).collect()).unwrap_or_default()
}

fn load(path: &Path) -> std::result::Result<Model, String> {
    if !path.exists() {
        return Err(format!("{}: no such model (new_model makes one)", path.display()));
    }
    Model::load(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Write a model, the paths of its files relative to its folder; written beside and renamed over the file, so
/// that a program following it never reads half a model.
fn save(path: &Path, m: &mut Model) -> std::result::Result<(), String> {
    let dir = path.parent().unwrap_or(Path::new(""));
    terraflux_graph::relative_paths(&mut m.nodes, dir);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{stem}.{}.{ext}", std::process::id()));
    m.save(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    // (on Windows a file being read cannot be replaced for that moment)
    let mut tries = 0;
    loop {
        match std::fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(_) if tries < 40 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(format!("{}: {e}", path.display()));
            }
        }
    }
}

/// A change of a model's file: read, changed, written back; nothing written when the change fails. What was done
/// and the problems the model has then.
fn edit(a: &Value, r: &Registry, change: impl FnOnce(&mut Model, &Registry, &Path) -> Reply) -> Reply {
    let path = model_path(a)?;
    let mut m = load(&path)?;
    let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
    let done = change(&mut m, r, &dir)?;
    save(&path, &mut m)?;
    Ok(format!("{done}\n{}", problems(&m, &path, r)))
}

/// The errors and warnings of a model (its submodels expanded).
fn problems(m: &Model, path: &Path, r: &Registry) -> String {
    let mut m = m.clone();
    if let Err(e) = terraflux_graph::submodel::expand(&mut m, path.parent().unwrap_or(Path::new(""))) {
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

fn list_templates() -> String {
    let mut s = String::new();
    for g in terraflux_app::templates::GROUPS {
        s.push_str(&format!("{g}\n"));
        for t in terraflux_app::templates::TEMPLATES.iter().filter(|t| t.group == g) {
            s.push_str(&format!("  {}: {}\n", t.name, t.about));
        }
    }
    s
}

/// The first sentence of a documentation.
fn first_sentence(doc: &str) -> String {
    let end = doc.find(". ").map_or(doc.len(), |i| i + 1);
    let s = &doc[..end];
    if s.chars().count() > 220 { format!("{}...", s.chars().take(220).collect::<String>()) } else { s.to_string() }
}

fn list_operators(r: &Registry, query: Option<&str>) -> String {
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

fn describe_operator(r: &Registry, name: &str) -> Reply {
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
    terraflux_app::templates::TEMPLATES.iter().find_map(|t| Some((t.name, find(&t.model().nodes, op)?)))
}

fn new_model(r: &Registry, a: &Value) -> Reply {
    let path = model_path(a)?;
    if path.exists() && !a.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
        return Err(format!("{} exists: edit it, or give overwrite: true to replace it", path.display()));
    }
    let mut m = match opt_text(a, "template") {
        Some(name) => {
            let t = terraflux_app::templates::TEMPLATES
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(name.trim()))
                .ok_or_else(|| format!("no template {name:?} (list_templates)"))?;
            let mut m = t.model();
            m.upgrade();
            arrange(&mut m.nodes, r);
            m
        }
        None => Model::new(&path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
    };
    if let Some(n) = opt_text(a, "name") {
        m.name = Some(n.to_string());
    }
    if let Some(d) = opt_text(a, "description") {
        m.description = Some(d.to_string());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    save(&path, &mut m)?;
    let nodes: Vec<String> = m.nodes.iter().map(|n| format!("{} ({})", n.id, n.op)).collect();
    let mut s = format!("wrote {}", path.display());
    if !nodes.is_empty() {
        s.push_str(&format!("; its nodes: {}", nodes.join(", ")));
    }
    Ok(format!("{s}\n{}", problems(&m, &path, r)))
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
fn arrange(nodes: &mut [Node], r: &Registry) {
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

fn check_model(engine: &Engine, a: &Value) -> Reply {
    let path = model_path(a)?;
    load(&path)?;
    let m = Model::load_expanded(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let issues = check(&m, &engine.registry);
    let mut lines: Vec<String> = issues.iter().map(|i| i.to_string()).collect();
    let mut errors = issues.iter().any(|i| i.severity == Severity::Error);
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    match load_inputs(&m, &dir, &BTreeMap::new()) {
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

/// What a run keeps and tells: its warnings (50 at most), its nodes and their progress as the protocol's progress
/// notifications (when the client gave a token), and whether the client cancelled it.
#[derive(Default)]
struct Kept {
    warnings: Vec<String>,
    notify: Option<Notify>,
    stop: Option<Arc<AtomicBool>>,
}

/// Progress notifications to the client: the token it gave, how many were sent, when the last.
struct Notify {
    token: Value,
    out: Out,
    sent: u64,
    told: Option<Instant>,
}

impl Kept {
    /// A progress notification (an operator's no more than once a second).
    fn tell(&mut self, message: String, always: bool) {
        let Some(n) = self.notify.as_mut() else { return };
        if !always && n.told.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
            return;
        }
        n.told = Some(Instant::now());
        n.sent += 1;
        let note = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": { "progressToken": n.token, "progress": n.sent, "message": message },
        });
        let _ = send(&n.out, &note);
    }
}

impl Observer for Kept {
    fn node(&mut self, at: &str) {
        self.tell(format!("running {at}"), true);
    }

    fn step(&mut self, at: &str, step: usize, of: usize) {
        self.tell(format!("{at}: step {step} of {of}"), false);
    }

    fn progress(&mut self, at: &str, p: &terraflux_ops::Progress) {
        self.tell(format!("{at}: {}", p.text), false);
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

fn run_model(engine: &Engine, a: &Value, kept: &mut Kept) -> Reply {
    let path = model_path(a)?;
    load(&path)?;
    let mut m = Model::load_expanded(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
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
    let inputs = load_inputs(&m, &dir, &overrides).map_err(|e| format!("the inputs: {e}"))?;
    let results = engine.run(&m, &inputs, kept).map_err(|e| e.to_string())?;
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
    if !kept.warnings.is_empty() {
        s.push_str("\nWarnings:");
        for w in &kept.warnings {
            s.push_str(&format!("\n- {w}"));
        }
    }
    Ok(s)
}

fn inspect_file(a: &Value) -> Reply {
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
        let valid: Vec<f64> = data.iter().copied().filter(|v| !v.is_nan()).collect();
        let empty = data.len() - valid.len();
        if valid.is_empty() {
            s.push_str("no values\n");
            continue;
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
        if integers && classes.len() <= 30 {
            let parts: Vec<String> = classes
                .iter()
                .map(|(c, n)| format!("{c}: {n} ({:.1}%)", 100.0 * *n as f64 / valid.len() as f64))
                .collect();
            s.push_str(&format!("classes (cells): {}", parts.join(", ")));
        } else {
            let min = valid.iter().copied().fold(f64::INFINITY, f64::min);
            let max = valid.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let mean = valid.iter().sum::<f64>() / valid.len() as f64;
            let sd = (valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / valid.len() as f64).sqrt();
            s.push_str(&format!("min {min:.6}, max {max:.6}, mean {mean:.6}, standard deviation {sd:.6}"));
        }
        if empty > 0 {
            s.push_str(&format!("; {empty} cells without a value"));
        }
        s.push('\n');
    }
    if r.layers.len() > 12 {
        s.push_str(&format!("... and {} bands more\n", r.layers.len() - 12));
    }
    Ok(s)
}

fn open_in_app(a: &Value) -> Reply {
    let path = model_path(a)?;
    load(&path)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut c = std::process::Command::new(exe);
    c.arg("app")
        .arg(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // without a console of its own, apart from this server's
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    c.spawn().map_err(|e| format!("the application did not start: {e}"))?;
    Ok(format!(
        "opened {} in the application; it follows the file, so the edits made from now on appear there",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(s: &Server, name: &str, args: Value) -> (String, bool) {
        let r = s.message(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": args } }))
            .unwrap();
        let res = &r["result"];
        (res["content"][0]["text"].as_str().unwrap().to_string(), res["isError"].as_bool().unwrap())
    }

    #[test]
    fn the_protocol_is_spoken() {
        let s = Server { engine: Engine::default() };
        let init = s
            .message(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }))
            .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert!(s.message(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
        let list = s.message(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() > 10);
        let none = s.message(json!({ "jsonrpc": "2.0", "id": 2, "method": "nothing" })).unwrap();
        assert_eq!(none["error"]["code"], -32601);
        let bad = s
            .message(json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "nope" } }))
            .unwrap();
        assert_eq!(bad["error"]["code"], -32602);
    }

    #[test]
    fn a_model_is_built_checked_and_run() {
        let s = Server { engine: Engine::default() };
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("m.toml").to_string_lossy().into_owned();
        let (t, e) = call(&s, "new_model", json!({ "model": model }));
        assert!(!e, "{t}");
        assert!(call(&s, "new_model", json!({ "model": model })).1, "an existing file is not replaced");
        let (t, e) = call(
            &s,
            "add_node",
            json!({ "model": model, "op": "nlm.random", "id": "land", "settings": { "rows": 20, "columns": 30, "seed": 3 } }),
        );
        assert!(!e, "{t}");
        assert!(t.contains("land.map"), "{t}");
        // a setting the operator does not have, a connection to nothing: refused, the file unchanged
        let before = std::fs::read_to_string(&model).unwrap();
        let (t, e) = call(&s, "add_node", json!({ "model": model, "op": "nlm.random", "settings": { "nope": 1 } }));
        assert!(e && t.contains("no setting"), "{t}");
        let (t, e) =
            call(&s, "add_node", json!({ "model": model, "op": "map.calc", "inputs": { "x": "nothing.map" } }));
        assert!(e, "{t}");
        assert_eq!(std::fs::read_to_string(&model).unwrap(), before);
        let (t, e) = call(
            &s,
            "add_node",
            json!({ "model": model, "op": "map.calc", "id": "high", "inputs": { "x": "land.map" }, "settings": { "expression": "x > 0.5" } }),
        );
        assert!(!e, "{t}");
        let (t, e) =
            call(&s, "set_output", json!({ "model": model, "name": "high", "from": "high.map", "dtype": "u8" }));
        assert!(!e, "{t}");
        assert!(call(&s, "set_output", json!({ "model": model, "name": "x", "from": "high.nope" })).1);
        let (t, e) = call(&s, "edit_node", json!({ "model": model, "node": "high", "rename": "above" }));
        assert!(!e, "{t}");
        let m = Model::load(&model).unwrap();
        assert_eq!(m.outputs["high"].from, "above.map", "the references follow a rename");
        // one layer of a stack written alone
        let (t, e) = call(
            &s,
            "add_node",
            json!({ "model": model, "op": "stack.of", "id": "both", "inputs": { "land": "land.map", "above": "above.map" } }),
        );
        assert!(!e, "{t}");
        let (t, e) = call(
            &s,
            "set_output",
            json!({ "model": model, "name": "only", "from": "both.stack", "layer": "above", "dtype": "u8" }),
        );
        assert!(!e && t.contains("the layer \"above\""), "{t}");
        let (t, e) = call(&s, "check_model", json!({ "model": model }));
        assert!(!e && t.contains("can run"), "{t}");
        let (t, e) = call(&s, "run_model", json!({ "model": model }));
        assert!(!e, "{t}");
        let out = dir.join("output").join("high.tif");
        assert!(out.exists(), "{t}");
        let (t, e) = call(&s, "inspect_file", json!({ "path": out.to_string_lossy() }));
        assert!(!e && t.contains("30 columns x 20 rows") && t.contains("classes"), "{t}");
        let only = dir.join("output").join("only.tif");
        let (t, e) = call(&s, "inspect_file", json!({ "path": only.to_string_lossy() }));
        assert!(!e && t.contains("1 band(s)") && t.contains("classes"), "{t}");
        let (t, e) = call(&s, "describe_operator", json!({ "name": "map.calc" }));
        assert!(!e && t.contains("Settings"), "{t}");
        let (t, _) = call(&s, "list_operators", json!({ "query": "markov" }));
        assert!(t.contains("markov.fit"), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_template_is_copied() {
        let s = Server { engine: Engine::default() };
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-t-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("t.toml").to_string_lossy().into_owned();
        let name = terraflux_app::templates::TEMPLATES[0].name;
        let (t, e) = call(&s, "new_model", json!({ "model": model, "template": name }));
        assert!(!e, "{t}");
        let m = Model::load(&model).unwrap();
        assert!(!m.nodes.is_empty() && m.nodes.iter().all(|n| n.pos.is_some()), "arranged");
        let (t, e) = call(&s, "check_model", json!({ "model": model }));
        assert!(!e && t.contains("can run"), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
