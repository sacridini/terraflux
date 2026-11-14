//! Where a reference to a node's output is, inside containers: `group.inner.port` (as the Viewer keeps it) or
//! `group/inner.port` (by its place), through the bodies of groups, loops and switches at any depth.

use crate::{Model, valid_id};

/// A node's output found in the model: the containers on the way (outermost first), the node and its port.
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    pub path: Vec<String>,
    pub node: String,
    pub port: String,
}

impl Located {
    /// Where a run keeps its value: the top-level node and port, or the top-level container's `inner.port`. A
    /// loop's body node means what its last step gave (`control.repeat` keeps it, `control.each` its last
    /// element's), as the Viewer shows it.
    pub fn key(&self) -> (String, String) {
        match self.path.split_first() {
            None => (self.node.clone(), self.port.clone()),
            Some((top, rest)) => {
                let inner: Vec<&str> = rest.iter().map(String::as_str).chain([&*self.node, &*self.port]).collect();
                (top.clone(), inner.join("."))
            }
        }
    }

    /// The key as one text, `node.port` (or `group.inner.port`).
    pub fn key_text(&self) -> String {
        let (n, p) = self.key();
        format!("{n}.{p}")
    }
}

/// Whether a reference is written as one that goes into a container (`g.inner.port`, `g/inner.port`), not a
/// plain `node.port`.
pub fn is_inner(from: &str) -> bool {
    from.contains('/') || from.matches('.').count() > 1
}

/// The top-level node and the port a Plot's `from` is kept at, read as text alone (the model not asked):
/// `node.port`, `g.inner.port` and `g/inner.port` as `("g", "inner.port")`, as [`Located::key`] gives. None for an
/// input or what is not a reference.
pub fn plot_port(from: &str) -> Option<(String, String)> {
    let tokens = tokens(from)?;
    Some((tokens[0].to_string(), tokens[1..].join(".")))
}

/// The ids of a reference (the containers, the node, the port), when each is valid and the port comes after the
/// last dot (`g/inner` has none).
fn tokens(from: &str) -> Option<Vec<&str>> {
    let tokens: Vec<&str> = from.split(['.', '/']).collect();
    let dot = from.rfind('.')?;
    (tokens.len() >= 2 && from.rfind('/').is_none_or(|slash| slash < dot) && tokens.iter().all(|t| valid_id(t)))
        .then_some(tokens)
}

/// The node and port a reference names, a node of the top level or inside containers: the container's node, then
/// its body's node (and so on) and the port. Why not, naming what is missing.
pub fn locate(m: &Model, from: &str) -> Result<Located, String> {
    let Some(tokens) = tokens(from) else {
        return Err(format!("{from:?} is not a reference (node.port, group.inner.port or group/inner.port)"));
    };
    let (port, ids) = tokens.split_last().expect("two tokens");
    let (node, containers) = ids.split_last().expect("one node");
    let mut nodes = &m.nodes;
    let mut path = Vec::new();
    for id in containers {
        let n = nodes.iter().find(|n| n.id == *id);
        let Some(n) = n else {
            return Err(match path.last() {
                None => format!("the model has no node {id:?}"),
                Some(c) => format!("{c:?} has no node {id:?}"),
            });
        };
        let Some(b) = &n.body else {
            return Err(format!("{id:?} ({}) is not a container: it has no nodes inside", n.op));
        };
        path.push(id.to_string());
        nodes = &b.nodes;
    }
    if !nodes.iter().any(|n| n.id == *node) {
        return Err(match path.last() {
            None => format!("the model has no node {node:?}"),
            Some(c) => format!("{c:?} has no node {node:?}"),
        });
    }
    Ok(Located { path, node: node.to_string(), port: port.to_string() })
}
