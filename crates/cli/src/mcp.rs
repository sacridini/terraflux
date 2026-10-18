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

    fn tool(&self, name: &str, a: &Value, watcher: &mut Watcher) -> Reply {
        let r = &self.engine.registry;
        match name {
            "list_templates" => Ok(tools::list_templates()),
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
        "capabilities": { "tools": {} },
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
    Ok(format!("{s}\n{}", tools::problems(&m, &folder(&path), r)))
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
        assert!(t.contains("with the values here: if "), "{t}");
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
            json!({ "model": model, "op": "map.calc", "id": "sum", "inputs": { "a": "small.map", "b": "big.map" }, "settings": { "expression": "a + b" } }),
        );
        assert!(!e, "{t}");
        let (t, e) = call(&s, "run_model", json!({ "model": model }));
        assert!(e, "{t}");
        assert!(t.contains("The node sum (map.calc) failed") && t.contains("What it was given"), "{t}");
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
