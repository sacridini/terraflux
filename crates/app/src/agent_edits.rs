//! What an agent did to a model's file through `terraflux mcp`, told to the application that has the model open:
//! the step of its undo that takes the file again named by the tools that changed it ("the MCP: connect"), not
//! guessed from the difference. Kept in the user's application data (`agent-edits.json`), by the model's path, each
//! tool with when it was used; taken (and forgotten) when the application reads the file again, those older than ten
//! minutes left out.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const FILE: &str = "agent-edits.json";
/// How long a tool told is taken as the cause of a change of the file (seconds).
const FRESH: f64 = 600.0;

type Told = BTreeMap<String, Vec<(f64, String)>>;

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

fn key(path: &Path) -> String {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().into_owned()
}

fn read(dir: &Path) -> Told {
    std::fs::read_to_string(dir.join(FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write(dir: &Path, told: &Told) {
    let _ = std::fs::create_dir_all(dir);
    let tmp = dir.join(format!("{FILE}.{}", std::process::id()));
    if std::fs::write(&tmp, serde_json::to_string(told).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join(FILE));
    }
}

/// An agent's tool `tool` is changing the model of `path` (told before its file is written).
pub fn tell(path: &Path, tool: &str) {
    tell_in(&crate::settings::data_dir(), path, tool);
}

pub fn tell_in(dir: &Path, path: &Path, tool: &str) {
    let mut told = read(dir);
    let t = now();
    told.retain(|_, v| {
        v.retain(|(at, _)| t - at < FRESH);
        !v.is_empty()
    });
    told.entry(key(path)).or_default().push((t, tool.to_string()));
    write(dir, &told);
}

/// The tools told for the model of `path` since it was taken last (the recent ones, in order), forgotten.
pub fn take(path: &Path) -> Vec<String> {
    take_in(&crate::settings::data_dir(), path)
}

pub fn take_in(dir: &Path, path: &Path) -> Vec<String> {
    let mut told = read(dir);
    let Some(v) = told.remove(&key(path)) else { return Vec::new() };
    write(dir, &told);
    let t = now();
    v.into_iter().filter(|(at, _)| t - at < FRESH).map(|(_, tool)| tool).collect()
}

/// The name of the step of the tools told: "the MCP: connect", "the MCP: add_node, connect", "the MCP: add_node and
/// 4 more" (each tool once, in order); none without any.
pub fn step_name(tools: &[String]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for t in tools {
        if !seen.contains(&t.as_str()) {
            seen.push(t);
        }
    }
    let said = match seen.as_slice() {
        [] => return None,
        [a] => a.to_string(),
        [a, b] => format!("{a}, {b}"),
        [a, b, c] => format!("{a}, {b}, {c}"),
        [a, rest @ ..] => format!("{a} and {} more", rest.len()),
    };
    Some(format!("the MCP: {said}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_told_name_the_step() {
        let dir = std::env::temp_dir().join(format!("terraflux-agent-edits-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (a, b) = (dir.join("a.toml"), dir.join("b.toml"));
        tell_in(&dir, &a, "add_node");
        tell_in(&dir, &a, "connect");
        tell_in(&dir, &a, "connect");
        tell_in(&dir, &b, "set_parameter");
        let tools = take_in(&dir, &a);
        assert_eq!(tools, ["add_node", "connect", "connect"]);
        assert_eq!(step_name(&tools).as_deref(), Some("the MCP: add_node, connect"));
        assert!(take_in(&dir, &a).is_empty(), "forgotten once taken");
        assert_eq!(take_in(&dir, &b), ["set_parameter"], "each model its own");
        assert_eq!(step_name(&[]), None);
        let many: Vec<String> = ["a", "b", "c", "d", "e"].map(String::from).into();
        assert_eq!(step_name(&many).as_deref(), Some("the MCP: a and 4 more"));
        // an old one: not the cause of a change now
        let mut told = Told::new();
        told.insert(key(&a), vec![(now() - 2.0 * FRESH, "remove_node".into())]);
        write(&dir, &told);
        assert!(take_in(&dir, &a).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
