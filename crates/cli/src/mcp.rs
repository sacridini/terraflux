//! `terraflux mcp`: Terraflux as a server of the Model Context Protocol (MCP), on the standard input and output.
//! An assistant (Claude Desktop, Claude Code, any client of MCP) builds a model through it as the node editor
//! does, with the tools of [`terraflux_app::tools`] (the same the assistant in the application uses) on a model's
//! file. The application, with the file open, shows every change as it is written ([`terraflux_app::watch`]).

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use terraflux_app::tools::{self, INSTRUCTIONS, Reply, Watcher, absolute, opt_text, text};
use terraflux_exec::Engine;
use terraflux_graph::Model;
use terraflux_ops::Registry;

/// The versions of the protocol spoken (the newest first): the client's, when it is one of them.
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

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
                    let tell = (!token.is_null()).then(|| progress(token, out.clone()));
                    let mut watcher = Watcher::new(tell, Some(stop));
                    let reply = server.answer(&m, &mut watcher);
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

/// The protocol's progress notifications of a run, to the client that gave a token.
fn progress(token: Value, out: Out) -> Box<dyn FnMut(String) + Send> {
    let mut sent = 0u64;
    Box::new(move |message| {
        sent += 1;
        let note = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": { "progressToken": token, "progress": sent, "message": message },
        });
        let _ = send(&out, &note);
    })
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
        self.answer(&m, &mut Watcher::default())
    }

    /// The answer to a request, a run telling `watcher` how it goes.
    fn answer(&self, m: &Value, watcher: &mut Watcher) -> Option<Value> {
        let id = m.get("id").cloned()?;
        let method = m.get("method").and_then(Value::as_str)?;
        let params = m.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => Ok(initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools::catalogue(false) })),
            "tools/call" => self.call(&params, watcher),
            "prompts/list" => Ok(json!({ "prompts": prompts() })),
            "prompts/get" => prompt(&params),
            "resources/list" => Ok(json!({ "resources": resources() })),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [{
                "uriTemplate": "terraflux://operator/{name}",
                "name": "operator",
                "title": "An operator's documentation",
                "description": "An operator's ports, settings, advice and the papers to cite (describe_operator), e.g. terraflux://operator/woe.fit.",
                "mimeType": "text/plain",
            }] })),
            "resources/read" => self.resource(&params),
            _ => Err((-32601, format!("no method {method:?}"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => failure(id, code, &message),
        })
    }

    fn call(&self, params: &Value, watcher: &mut Watcher) -> std::result::Result<Value, (i64, String)> {
        let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
        if !tools::catalogue(false).iter().any(|t| t["name"] == name) {
            return Err((-32602, format!("no tool {name:?}")));
        }
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        // (a failure of an operator ends the call, not the server)
        let (text, error) = match std::panic::catch_unwind(AssertUnwindSafe(|| self.tool(name, &args, watcher))) {
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

    /// A resource's contents: a guide, a template's file, an operator's documentation.
    fn resource(&self, params: &Value) -> std::result::Result<Value, (i64, String)> {
        let uri = params.get("uri").and_then(Value::as_str).unwrap_or_default();
        // (the protocol's code for a resource not found)
        let missing = || (-32002, format!("no resource {uri:?}"));
        let (kind, name) = uri.strip_prefix("terraflux://").and_then(|r| r.split_once('/')).ok_or_else(missing)?;
        let r = &self.engine.registry;
        let (mime, text) = match kind {
            "guide" => ("text/plain", terraflux_app::guide::guide(Some(name)).map_err(|_| missing())?),
            "template" => {
                let t =
                    terraflux_app::templates::TEMPLATES.iter().find(|t| slug(t.name) == name).ok_or_else(missing)?;
                let m = tools::template_model(r, t.name).map_err(|e| (-32603, e))?;
                ("application/toml", m.to_toml().map_err(|e| (-32603, e.to_string()))?)
            }
            "operator" => ("text/plain", tools::describe_operator(r, name).map_err(|_| missing())?),
            _ => return Err(missing()),
        };
        Ok(json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": text }] }))
    }

    fn tool(&self, name: &str, a: &Value, watcher: &mut Watcher) -> Reply {
        let r = &self.engine.registry;
        match name {
            "list_templates" => Ok(tools::find_templates(opt_text(a, "query"))),
            "guide" => terraflux_app::guide::guide(opt_text(a, "topic")),
            "list_operators" => Ok(tools::list_operators(r, opt_text(a, "query"))),
            "describe_operator" => tools::describe_operator(r, text(a, "name")?),
            "new_model" => new_model(r, a),
            "show_model" => {
                let path = model_path(a)?;
                tools::show_model(&load(&path)?, &folder(&path), r)
            }
            "check_model" => {
                let path = model_path(a)?;
                tools::check_model(r, &load(&path)?, &folder(&path))
            }
            "methods" => {
                let path = model_path(a)?;
                let file = path.file_name().map(|f| f.to_string_lossy().into_owned());
                tools::methods(&load(&path)?, r, a, file.as_deref())
            }
            "run_model" => {
                let path = model_path(a)?;
                tools::run_model(&self.engine, &load(&path)?, &folder(&path), a, watcher)
            }
            "trace_cell" => {
                let path = model_path(a)?;
                tools::trace_cell_run(&self.engine, &load(&path)?, &folder(&path), a, watcher)
            }
            "last_trace" => terraflux_app::trace::last()
                .ok_or_else(|| "no cell traced yet in the application (a right click on a cell of a map)".to_string()),
            "inspect_file" => tools::inspect_file(a),
            "open_in_app" => open_in_app(a),
            "explain_calc" => {
                let path = model_path(a)?;
                let (m, dir) = (load(&path)?, folder(&path));
                tools::explain_calc(&m, &dir, a, &tools::known_by_running(&self.engine, &m, &dir))
            }
            // (the values of the calculator's inputs: the model run up to what it reads)
            "write_calc" => change(a, r, |m, r, dir| {
                let known = tools::known_by_running(&self.engine, m, dir);
                tools::write_calc(m, r, dir, a, &known)
            }),
            edit if tools::EDITS.contains(&edit) => change(a, r, |m, r, dir| tools::edit(edit, m, r, dir, a)),
            _ => Err(format!("no tool {name:?}")),
        }
    }
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
    let version = VERSIONS.iter().find(|v| **v == asked).unwrap_or(&VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {}, "prompts": {}, "resources": {} },
        "serverInfo": { "name": "terraflux", "title": "Terraflux", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn model_path(a: &Value) -> std::result::Result<PathBuf, String> {
    let p = absolute(text(a, "model")?);
    if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml") || e.eq_ignore_ascii_case("json")) {
        return Err(format!("{}: a model is a .toml or .json file", p.display()));
    }
    Ok(p)
}

/// The folder of a model's file (its paths are relative to it).
fn folder(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
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
fn change(a: &Value, r: &Registry, change: impl FnOnce(&mut Model, &Registry, &Path) -> Reply) -> Reply {
    let path = model_path(a)?;
    let mut m = load(&path)?;
    let dir = folder(&path);
    let done = change(&mut m, r, &dir)?;
    save(&path, &mut m)?;
    Ok(format!("{done}\n{}", tools::problems(&m, &dir, r)))
}

fn new_model(r: &Registry, a: &Value) -> Reply {
    let path = model_path(a)?;
    if path.exists() && !a.get("overwrite").and_then(Value::as_bool).unwrap_or(false) {
        return Err(format!("{} exists: edit it, or give overwrite: true to replace it", path.display()));
    }
    let mut m = match opt_text(a, "template") {
        Some(name) => tools::template_model(r, name)?,
        None => Model::new(&path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
    };
    tools::model_info(&mut m, a);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    save(&path, &mut m)?;
    let nodes: Vec<String> = m.nodes.iter().map(|n| format!("{} ({})", n.id, n.op)).collect();
    let mut s = format!("wrote {}", path.display());
    if !nodes.is_empty() {
        s.push_str(&format!("; its nodes: {}", nodes.join(", ")));
    }
    if opt_text(a, "template").is_some() {
        s.push_str(&tools::template_next(&m));
    }
    Ok(format!("{s}\n{}", tools::problems(&m, &folder(&path), r)))
}

/// A template's name as the last part of its resource's address: small letters, digits and dashes.
fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in terraflux_app::search::fold(name).chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    s.trim_matches('-').to_string()
}

/// The resources: the guides and the templates (their files), read by the clients that show resources.
fn resources() -> Vec<Value> {
    let mut out: Vec<Value> = terraflux_app::guide::GUIDES
        .iter()
        .map(|g| {
            json!({
                "uri": format!("terraflux://guide/{}", g.name),
                "name": g.name,
                "title": g.title,
                "description": format!("Terraflux's guide: {}", g.title),
                "mimeType": "text/plain",
            })
        })
        .collect();
    out.extend(terraflux_app::templates::TEMPLATES.iter().map(|t| {
        json!({
            "uri": format!("terraflux://template/{}", slug(t.name)),
            "name": slug(t.name),
            "title": format!("Template: {}", t.name),
            "description": t.about.split_whitespace().collect::<Vec<_>>().join(" "),
            "mimeType": "application/toml",
        })
    }));
    out
}

/// The prompts: workflows a user starts by name, with their arguments.
fn prompts() -> Value {
    let arg = |name: &str, description: &str, required: bool| json!({ "name": name, "description": description, "required": required });
    let model = arg("model", "The model's file (.toml), an absolute path.", true);
    json!([
        {
            "name": "build_model",
            "title": "Build a model",
            "description": "Build a Terraflux model for a goal, from the guide of its field and a template, with your data: checked, run and recorded.",
            "arguments": [
                arg("goal", "What the model is to find out, e.g. where the forest will be lost by 2040.", true),
                arg("data", "The data: files or a folder (absolute paths), and what they are.", false),
                arg("model", "The model's file to write (.toml), an absolute path.", false),
            ],
        },
        {
            "name": "adapt_template",
            "title": "A template with your data",
            "description": "Copy a template and put your files in place of its drawn data.",
            "arguments": [
                arg("template", "The template's name (list_templates).", true),
                arg("data", "Your files (absolute paths) and what each is.", true),
                model.clone(),
            ],
        },
        {
            "name": "review_model",
            "title": "Review a model's method",
            "description": "Review a model as a paper's reviewer would, without changing it: what is sound, what is missing, what to change.",
            "arguments": [model.clone()],
        },
        {
            "name": "explain_model",
            "title": "Explain a model",
            "description": "Explain what a model does and why, stage by stage, from its record and its operators' documentation.",
            "arguments": [model, arg("language", "The language of the explanation (default: the user's).", false)],
        },
        {
            "name": "explain_cell",
            "title": "Explain the cell traced",
            "description": "Explain why the cell traced last in Terraflux's application (a right click on a cell of a map) has its value, from its trace.",
            "arguments": [],
        },
    ])
}

/// A prompt's messages, its arguments in place.
fn prompt(params: &Value) -> std::result::Result<Value, (i64, String)> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    if name == "explain_cell" {
        return explain_cell(params);
    }
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let get = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
    let need = |k: &str| get(k).ok_or((-32602, format!("the prompt {name} needs the argument {k}")));
    let (description, text) = match name {
        "build_model" => {
            let goal = need("goal")?;
            let data = get("data").map_or(String::new(), |d| format!("\nMy data: {d}"));
            let file = get("model").map_or(String::new(), |m| format!(" in the file {m}"));
            (
                "Build a model for a goal",
                format!(
                    "Build a Terraflux model{file} for this: {goal}{data}

1. Read the guide of its field (guide; without a topic it lists them) and follow its steps.
2. Find the template nearest to it (list_templates with a query) and copy it (new_model with `template`, my \
request in my words as `request`), or start from an empty model when none fits.
3. Look at each of my files (inspect_file) and put them on one grid (guide data-preparation), in place of the \
template's drawn data (raster.read nodes).
4. Look each operator up (describe_operator) before adding it; give each node a note and a why, and refs and \
assumptions where there are.
5. check_model: fix the errors, follow the warnings and the advice on the method, or tell me why not.
6. run_model, look at the outputs (inspect_file) and tell me what they show and how far to trust them.
7. Group the model by stage (group_by_stage), give it a summary (set_model_info) and open it in the application \
(open_in_app).
Ask me what the data cannot tell (what the classes are, the period, the targets) before choosing it yourself."
                ),
            )
        }
        "adapt_template" => {
            let (template, data, model) = (need("template")?, need("data")?, need("model")?);
            (
                "A template with the user's data",
                format!(
                    "Copy the Terraflux template {template:?} to {model} (new_model with `template`) and put my data in \
place of its drawn data (the nlm.* nodes): {data}

Look at each file first (inspect_file: its grid, coordinate system, classes) and at the guide of the template's \
field (guide); put the files on one grid (guide data-preparation) with raster.read nodes, keep the template's \
method, change its settings only where my data need it (say why in the node's why), then check_model (follow its \
advice), run_model and tell me what the outputs show."
                ),
            )
        }
        "review_model" => {
            let model = need("model")?;
            (
                "Review a model's method",
                format!(
                    "Review the method of the Terraflux model {model} as a reviewer of a paper would. Do not change it.

Read it (show_model) and check it (check_model: its warnings and advice on the method), read the guide of its \
field (guide) and look up the operators you are not sure of (describe_operator). Then tell me, the most serious \
first: what is sound; what is missing (a validation on data not used to calibrate, the uncertainty, the data on \
one grid in a projected coordinate system, choices without a note or a why); and what to change, with the \
operators that would do it. End with the methods drafted from it (methods) and what a reader would still miss."
                ),
            )
        }
        "explain_model" => {
            let model = need("model")?;
            let lang = get("language").map_or(String::new(), |l| format!(", in {l}"));
            (
                "Explain a model",
                format!(
                    "Explain the Terraflux model {model} to me{lang}: what it is for, its stages, what each node does and \
why, its inputs and outputs, and what it assumes. Read it (show_model) and take what each node does from its note \
and why and from its operator's documentation (describe_operator), not from memory: an operator's name may mean \
something else here. Do not change the model."
                ),
            )
        }
        _ => return Err((-32602, format!("no prompt {name:?}"))),
    };
    Ok(json!({
        "description": description,
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }],
    }))
}

/// The prompt `explain_cell`: the last trace made in the application, and the question.
fn explain_cell(params: &Value) -> std::result::Result<Value, (i64, String)> {
    if params.get("name").and_then(Value::as_str) != Some("explain_cell") {
        return Err((-32602, "no such prompt".into()));
    }
    let text = match terraflux_app::trace::last() {
        Some(t) => format!(
            "Explain to me, in words, why this cell of my model has its value: go through its trace, step by step, \
             from the result back to the data, saying what each step did to it; trace other cells or nodes with \
             trace_cell when it helps.\n\n{t}"
        ),
        None => "No cell has been traced yet in Terraflux's application: right-click a cell of a map there (Trace \
                 this cell), then ask again."
            .into(),
    };
    Ok(json!({
        "description": "Why the cell traced last has its value",
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }],
    }))
}

/// The model in the application already open (its model replaced), else in a window of its own.
fn open_in_app(a: &Value) -> Reply {
    let path = model_path(a)?;
    load(&path)?;
    let why = match terraflux_app::handoff::open_in_running(&path) {
        Ok(pid) => {
            return Ok(format!(
                "opened {} in the Terraflux already open (process {pid}); it follows the file, so the edits made from \
                 now on appear there",
                path.display()
            ));
        }
        Err(why) => why,
    };
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
        "opened {} in a new window of the application ({why}); it follows the file, so the edits made from now on \
         appear there",
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
        // the prompt that explains the cell traced last in the application
        let prompts = s.message(json!({ "jsonrpc": "2.0", "id": 4, "method": "prompts/list" })).unwrap();
        assert!(prompts["result"]["prompts"].as_array().unwrap().iter().any(|p| p["name"] == "explain_cell"));
        let p = s
            .message(
                json!({ "jsonrpc": "2.0", "id": 5, "method": "prompts/get", "params": { "name": "explain_cell" } }),
            )
            .unwrap();
        let text = p["result"]["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("trace") || text.contains("traced"), "{text}");
        assert_eq!(init["result"]["capabilities"]["prompts"], json!({}));
        assert_eq!(init["result"]["capabilities"]["resources"], json!({}));
    }

    /// The instructions stay short enough for the clients that cut them (Claude Code at about 4000 characters),
    /// and name every guide, where the rest is.
    #[test]
    fn the_instructions_are_short_and_name_the_guides() {
        assert!(INSTRUCTIONS.chars().count() < 3200, "{} characters", INSTRUCTIONS.chars().count());
        for g in terraflux_app::guide::GUIDES {
            assert!(INSTRUCTIONS.contains(g.name), "the instructions do not name the guide {}", g.name);
        }
    }

    #[test]
    fn guides_prompts_and_resources() {
        let s = Server { engine: Engine::default() };
        let (t, e) = call(&s, "guide", json!({}));
        assert!(!e && t.contains("land-change") && t.contains("species-distribution"), "{t}");
        let (t, e) = call(&s, "guide", json!({ "topic": "land-change" }));
        assert!(!e && t.contains("validate.three_map") && t.contains("not used to calibrate"), "{t}");
        assert!(call(&s, "guide", json!({ "topic": "astrology" })).1);
        // the prompts: their arguments in place, one missing refused
        let get = |name: &str, args: Value| {
            s.message(json!({ "jsonrpc": "2.0", "id": 1, "method": "prompts/get", "params": { "name": name, "arguments": args } }))
                .unwrap()
        };
        let p = get("build_model", json!({ "goal": "where the forest is lost by 2040", "data": "C:/data/lulc" }));
        let text = p["result"]["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("where the forest is lost by 2040") && text.contains("C:/data/lulc"), "{text}");
        assert!(text.contains("guide") && text.contains("check_model"), "{text}");
        assert_eq!(get("review_model", json!({}))["error"]["code"], -32602);
        let p = get("review_model", json!({ "model": "C:/m.toml" }));
        assert!(p["result"]["messages"][0]["content"]["text"].as_str().unwrap().contains("Do not change it"));
        assert_eq!(get("nothing", json!({}))["error"]["code"], -32602);
        // the resources: the guides and the templates, an operator's documentation by its address
        let list = s.message(json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" })).unwrap();
        let uris: Vec<&str> =
            list["result"]["resources"].as_array().unwrap().iter().filter_map(|r| r["uri"].as_str()).collect();
        assert!(
            uris.contains(&"terraflux://guide/connectivity") && uris.contains(&"terraflux://template/dinamica-ego")
        );
        let read = |uri: &str| {
            s.message(json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/read", "params": { "uri": uri } }))
                .unwrap()
        };
        let r = read("terraflux://template/dinamica-ego");
        assert!(r["result"]["contents"][0]["text"].as_str().unwrap().contains("woe.fit"), "{r}");
        let r = read("terraflux://operator/woe.fit");
        assert!(r["result"]["contents"][0]["text"].as_str().unwrap().contains("Advice:"), "{r}");
        assert_eq!(read("terraflux://guide/nothing")["error"]["code"], -32002);
        assert_eq!(read("https://example.org")["error"]["code"], -32002);
        let t = s.message(json!({ "jsonrpc": "2.0", "id": 4, "method": "resources/templates/list" })).unwrap();
        assert_eq!(t["result"]["resourceTemplates"][0]["uriTemplate"], "terraflux://operator/{name}");
    }

    #[test]
    fn found_in_portuguese_and_advised_on_the_method() {
        let s = Server { engine: Engine::default() };
        let (t, _) = call(&s, "list_operators", json!({ "query": "densidade de bordas" }));
        assert!(t.contains("metrics."), "{t}");
        let (t, _) = call(&s, "list_operators", json!({ "query": "conectividade circuito" }));
        assert!(t.contains("circuit.omniscape") && !t.contains("No operator"), "{t}");
        let (t, _) = call(&s, "list_operators", json!({ "query": "circuitos" }));
        assert!(t.contains("circuit.pairwise"), "{t}");
        // none has all the words: those with the most
        let (t, _) = call(&s, "list_operators", json!({ "query": "omniscape zzzz" }));
        assert!(t.starts_with("No operator has all of") && t.contains("circuit.omniscape"), "{t}");
        let (t, _) = call(&s, "list_templates", json!({ "query": "validação de uma simulação" }));
        assert!(t.starts_with("Validating a simulation"), "{t}");
        // a land change model copied: what to do next, and advice on its method
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-a-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("d.toml").to_string_lossy().into_owned();
        let (t, e) = call(&s, "new_model", json!({ "model": model, "template": "Dinamica EGO" }));
        assert!(!e && t.contains("Next: its data are drawn") && t.contains("land_2000"), "{t}");
        let (t, e) = call(&s, "check_model", json!({ "model": model }));
        assert!(!e && t.contains("can run"), "{t}");
        assert!(t.contains("advice:") && t.contains("never compares") && t.contains("guide land-change"), "{t}");
        let (t, e) = call(&s, "describe_operator", json!({ "name": "woe.fit" }));
        assert!(!e && t.contains("Advice:") && t.contains("Its outputs are read by:"), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
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
            call(&s, "add_node", json!({ "model": model, "op": "raster.calc", "inputs": { "x": "nothing.map" } }));
        assert!(e, "{t}");
        assert_eq!(std::fs::read_to_string(&model).unwrap(), before);
        let (t, e) = call(
            &s,
            "add_node",
            json!({ "model": model, "op": "raster.calc", "id": "high", "inputs": { "x": "land.map" }, "settings": { "expression": "x > 0.5" } }),
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
        let (t, e) = call(&s, "describe_operator", json!({ "name": "raster.calc" }));
        assert!(!e && t.contains("Settings"), "{t}");
        let (t, _) = call(&s, "list_operators", json!({ "query": "markov" }));
        assert!(t.contains("markov.fit"), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_model_is_grouped_by_stage() {
        let s = Server { engine: Engine::default() };
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-g-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("g.toml").to_string_lossy().into_owned();
        let (t, e) = call(&s, "new_model", json!({ "model": model, "template": "CLUE-S" }));
        assert!(!e, "{t}");
        let (t, e) = call(&s, "group_by_stage", json!({ "model": model }));
        assert!(!e && t.contains("data, preparation, model"), "{t}");
        assert!(call(&s, "group_by_stage", json!({ "model": model })).1, "grouped already");
        let m = Model::load(&model).unwrap();
        assert!(m.node("model").is_some_and(|g| g.op == terraflux_graph::GROUP && g.note.is_some()));
        // a group taken apart, and made again by hand
        let (t, e) = call(&s, "ungroup", json!({ "model": model, "node": "data" }));
        assert!(!e, "{t}");
        let (t, e) = call(
            &s,
            "group_nodes",
            json!({ "model": model, "nodes": ["land_2000", "aptitude", "roughness"], "id": "inputs", "note": "The maps drawn." }),
        );
        assert!(!e && t.contains("inputs.land_2000"), "{t}");
        let (t, e) = call(&s, "check_model", json!({ "model": model }));
        assert!(!e && t.contains("can run"), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parameters_and_scenarios() {
        let s = Server { engine: Engine::default() };
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-p-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("p.toml").to_string_lossy().into_owned();
        let (t, e) = call(&s, "new_model", json!({ "model": model, "template": "Neutral landscapes" }));
        assert!(!e, "{t}");
        let (t, e) = call(
            &s,
            "set_parameter",
            json!({ "model": model, "key": "field.range", "unit": "cells", "min": 2, "max": 60, "step": 1 }),
        );
        assert!(!e && t.contains("now 20"), "{t}");
        assert!(call(&s, "set_parameter", json!({ "model": model, "key": "field.nope" })).1, "no such setting");
        assert!(call(&s, "set_parameter", json!({ "model": model, "key": "gone.range" })).1, "no such node");
        let m = Model::load(&model).unwrap();
        let p = &m.parameters[0];
        assert_eq!(p.label.as_deref(), Some("The range of the autocorrelation, in cells"));
        assert_eq!((p.min, p.max, p.unit.as_deref()), (Some(2.0), Some(60.0), Some("cells")));
        assert!(p.help.as_deref().is_some_and(|h| h.contains("autocorrelation")));
        let (t, e) =
            call(&s, "set_scenario", json!({ "model": model, "name": "coarse", "values": { "field.range": 50 } }));
        assert!(!e, "{t}");
        let (t, e) = call(&s, "set_scenario", json!({ "model": model, "name": "now", "note": "as it is" }));
        assert!(!e, "{t}");
        let m = Model::load(&model).unwrap();
        assert_eq!(m.scenarios[1].values["field.range"], 20, "the parameters' values now");
        let (t, e) = call(&s, "run_model", json!({ "model": model, "scenario": "coarse" }));
        assert!(!e, "{t}");
        assert!(call(&s, "run_model", json!({ "model": model, "scenario": "nope" })).1);
        let (t, e) = call(&s, "remove_scenario", json!({ "model": model, "name": "now" }));
        assert!(!e, "{t}");
        let (t, e) = call(&s, "remove_parameter", json!({ "model": model, "key": "field.range" }));
        assert!(!e, "{t}");
        let m = Model::load(&model).unwrap();
        assert!(m.parameters.is_empty() && m.scenarios.len() == 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_calculator_is_written_and_explained() {
        let s = Server { engine: Engine::default() };
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-calc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("c.toml").to_string_lossy().into_owned();
        assert!(!call(&s, "new_model", json!({ "model": model })).1);
        let land = json!({ "model": model, "op": "nlm.random", "id": "land",
            "settings": { "rows": 40, "columns": 60, "seed": 3 } });
        assert!(!call(&s, "add_node", land).1);
        let stack =
            json!({ "model": model, "op": "stack.of", "id": "s", "inputs": { "low": "land.map", "high": "land.map" } });
        assert!(!call(&s, "add_node", stack).1);
        // refused, the file as it was: a name that is no input, a layer the stack does not have
        let (t, e) = call(
            &s,
            "write_calc",
            json!({ "model": model, "node": "classes", "script": "c = 1\nc[elev > 0.5] = 2\nout c",
                "inputs": { "land": "land.map" } }),
        );
        assert!(e && t.contains("not written") && t.contains("elev") && t.contains("land (a map)"), "{t}");
        let (t, e) = call(
            &s,
            "write_calc",
            json!({ "model": model, "node": "classes", "script": "out x = s.middle", "inputs": { "s": "s.stack" } }),
        );
        assert!(e && t.contains("middle"), "{t}");
        assert!(!call(&s, "show_model", json!({ "model": model })).0.contains("classes"));
        // written, and what it gives told
        let (t, e) = call(
            &s,
            "write_calc",
            json!({ "model": model, "node": "classes",
                "script": "# two classes of the random map\nc = 1\nc[land > 0.5] = 2\nout c",
                "inputs": { "land": "land.map" }, "note": "the random map in two classes" }),
        );
        assert!(!e && t.contains("added classes (raster.calc)"), "{t}");
        assert!(t.contains("- c: a map: classes 1 (") && t.contains(", 2 ("), "{t}");
        // rewritten (an operator that is not a calculator is not)
        let (t, e) = call(
            &s,
            "write_calc",
            json!({ "model": model, "node": "classes", "script": "out high = s.high > 0.9", "inputs": { "s": "s.stack" } }),
        );
        assert!(!e && t.contains("changed classes") && t.contains("- high: a map: classes 0 ("), "{t}");
        assert!(call(&s, "write_calc", json!({ "model": model, "node": "land", "script": "out x = 1" })).1);
        let (t, e) = call(&s, "explain_calc", json!({ "model": model, "node": "classes" }));
        assert!(!e, "{t}");
        assert!(t.contains("- s <- s.stack: a stack of 2 layers, read as high, low"), "{t}");
        assert!(t.contains("The layers it reads: s: high"), "{t}");
        assert!(t.contains("- high (an output): a map: classes 0 ("), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cell_is_traced() {
        let s = Server { engine: Engine::default() };
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-tr-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("t.toml").to_string_lossy().into_owned();
        let (t, e) = call(&s, "new_model", json!({ "model": model, "template": "CLUE-S" }));
        assert!(!e, "{t}");
        let (t, e) = call(&s, "trace_cell", json!({ "model": model, "node": "simulation", "row": 40, "col": 60 }));
        assert!(!e, "{t}");
        assert!(t.starts_with("Here simulation.maps is "), "{t}");
        assert!(t.contains("with the values here: 2 if "), "{t}");
        assert!(!t.contains("in the loop,"), "{t}");
        let (t, e) = call(
            &s,
            "trace_cell",
            json!({ "model": model, "node": "simulation", "row": 40, "col": 60, "inside_loops": true }),
        );
        assert!(!e && t.contains("in the loop, allocate.landscape"), "{t}");
        assert!(call(&s, "trace_cell", json!({ "model": model, "node": "simulation", "row": 4000, "col": 0 })).1);
        assert!(call(&s, "trace_cell", json!({ "model": model, "node": "nope", "row": 1, "col": 1 })).1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failure_is_explained() {
        let s = Server { engine: Engine::default() };
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-f-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("f.toml").to_string_lossy().into_owned();
        let (t, e) = call(&s, "new_model", json!({ "model": model }));
        assert!(!e, "{t}");
        for (id, rows) in [("small", 10), ("big", 20)] {
            let (t, e) = call(
                &s,
                "add_node",
                json!({ "model": model, "op": "nlm.random", "id": id, "settings": { "rows": rows, "columns": 10 } }),
            );
            assert!(!e, "{t}");
        }
        let (t, e) = call(
            &s,
            "add_node",
            json!({ "model": model, "op": "raster.calc", "id": "sum", "inputs": { "a": "small.map", "b": "big.map" }, "settings": { "expression": "a + b" } }),
        );
        assert!(!e, "{t}");
        let (t, e) = call(&s, "run_model", json!({ "model": model }));
        assert!(e, "{t}");
        assert!(t.contains("The node sum (raster.calc) failed") && t.contains("What it was given"), "{t}");
        assert!(t.contains("10 x 20 cells") && t.contains("raster.match"), "{t}");
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
        assert!(!t.contains("advice"), "a template's nodes are not asked for a why: {t}");
        // its methods, as a paper's: in English, in Portuguese, the references as BibTeX
        let (t, e) = call(&s, "methods", json!({ "model": model }));
        assert!(!e && t.starts_with("Methods\n\nThe analysis was run in Terraflux") && t.contains("(t.toml)"), "{t}");
        let (t, e) = call(&s, "methods", json!({ "model": model, "language": "pt" }));
        assert!(!e && t.starts_with("Métodos\n\nA análise foi feita no Terraflux"), "{t}");
        let (t, e) = call(&s, "methods", json!({ "model": model, "format": "bibtex" }));
        assert!(!e && t.starts_with("@software{terraflux,"), "{t}");
        assert!(call(&s, "methods", json!({ "model": model, "language": "fr" })).1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_model_records_its_choices() {
        let s = Server { engine: Engine::default() };
        let dir = std::env::temp_dir().join(format!("terraflux-mcp-r-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let model = dir.join("r.toml").to_string_lossy().into_owned();
        let (t, e) = call(
            &s,
            "new_model",
            json!({ "model": model, "request": "where does the forest stay whole?", "author": "an agent" }),
        );
        assert!(!e, "{t}");
        // a node without its reasons: added, and asked for them; check_model advises
        let (t, e) = call(&s, "add_node", json!({ "model": model, "op": "nlm.random", "id": "land" }));
        assert!(!e && t.contains("a note and a why"), "{t}");
        let (t, _) = call(&s, "check_model", json!({ "model": model }));
        assert!(t.contains("can run") && t.contains("advice: nodes without a why") && t.contains("land"), "{t}");
        let (t, e) = call(
            &s,
            "edit_node",
            json!({
                "model": model, "node": "land", "note": "a landscape to try the model on",
                "why": "no data yet", "refs": ["Saura and Martinez-Millan (2000)"],
                "assumptions": "no structure\nuniform values", "alternatives": ["nlm.gaussian_field: too smooth"],
            }),
        );
        assert!(!e, "{t}");
        let (t, _) = call(&s, "check_model", json!({ "model": model }));
        assert!(!t.contains("advice"), "{t}");
        let (t, e) = call(&s, "set_model_info", json!({ "model": model, "summary": "a first try", "author": "" }));
        assert!(!e && t.contains("summary"), "{t}");
        let m = Model::load(&model).unwrap();
        let n = m.node("land").unwrap();
        assert_eq!(n.why.as_deref(), Some("no data yet"));
        assert_eq!(n.refs, ["Saura and Martinez-Millan (2000)"]);
        assert_eq!(n.assumptions, ["no structure", "uniform values"], "a text of a line each");
        assert_eq!(n.alternatives.len(), 1);
        assert_eq!(m.request.as_deref(), Some("where does the forest stay whole?"));
        assert_eq!((m.summary.as_deref(), m.author.as_deref()), (Some("a first try"), None), "empty: removed");
        // a list of the wrong kind: refused, the file as it was
        let (_, e) = call(&s, "edit_node", json!({ "model": model, "node": "land", "refs": [1, 2] }));
        assert!(e);
        assert_eq!(Model::load(&model).unwrap(), m);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
