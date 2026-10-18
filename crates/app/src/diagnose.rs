//! Why a node failed: its error, and what it was given, side by side. For each of its inputs: where it comes from,
//! what it is, and for a map its grid (size, cell size, coordinate system, origin), so that what does not fit
//! shows (two maps of different grids, of different coordinate systems, an input that gave nothing); what does not
//! fit said in words, with what puts it right. The Problems pane shows it, the assistant is told it, `run_model`
//! answers with it.

use std::sync::Arc;

use terraflux_graph::{Model, Ref, Source};
use terraflux_ops::Value;
use terraflux_raster::Grid;

use crate::report::cell;

/// A map's grid in words.
#[derive(Debug, Clone, PartialEq)]
pub struct GridInfo {
    pub width: usize,
    pub height: usize,
    pub cell: (f64, f64),
    pub crs: String,
    pub origin: (f64, f64),
}

impl GridInfo {
    fn of(g: &Grid) -> GridInfo {
        let t = g.transform;
        GridInfo {
            width: g.width,
            height: g.height,
            cell: (t[1].abs(), t[5].abs()),
            crs: crate::prepare::crs_name(crate::prepare::crs_of(g).as_deref()),
            origin: (t[0], t[3]),
        }
    }

    /// The size in words: "160 x 160 cells".
    pub fn size(&self) -> String {
        format!("{} x {} cells", self.width, self.height)
    }

    pub fn cell_size(&self) -> String {
        if self.cell.0 == self.cell.1 {
            cell(self.cell.0)
        } else {
            format!("{} x {}", cell(self.cell.0), cell(self.cell.1))
        }
    }
}

/// An input of the node that failed: its port, where it comes from, what it is, its grid when it is maps.
#[derive(Debug, Clone, PartialEq)]
pub struct Side {
    pub port: String,
    pub from: String,
    /// The node it comes from, to select it.
    pub node: Option<String>,
    /// What it is ("a map", "a stack of 3 layers", "a table"), or that it gave nothing.
    pub kind: String,
    pub grid: Option<GridInfo>,
}

/// A node's failure explained.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnosis {
    /// Where it failed: the node's path (`loop/node` inside a container) and its operator.
    pub at: String,
    pub op: String,
    pub message: String,
    pub inputs: Vec<Side>,
    /// What does not fit, and what puts it right.
    pub hints: Vec<String>,
}

/// What a value is, in a few words.
fn kind(v: &Value) -> String {
    match v {
        Value::Map(_) => "a map".into(),
        Value::Stack(s) => format!("a stack of {} layers", s.layers.len()),
        Value::Raster(r) if r.layers.len() == 1 => "a raster on disk".into(),
        Value::Raster(r) => format!("rasters on disk ({} layers)", r.layers.len()),
        Value::Series(s) => format!("a series of {} values", s.len()),
        other => format!("a {}", other.ty().name()),
    }
}

/// The node and the message of a run's error (`node: message`, `loop/node.port: message`).
pub fn failed_at(error: &str) -> Option<(String, String)> {
    let (at, message) = error.split_once(": ")?;
    let at = at.trim().trim_matches('"');
    // (the node, without the port or the setting an error names)
    let node = at.split('.').next()?.to_string();
    terraflux_graph::valid_id(node.rsplit('/').next()?).then(|| (node, message.trim().to_string()))
}

/// The diagnosis of the node at a path (`node`, `loop/node`) that failed with a message, from the values of the run
/// (by node and port, as kept: a top-level node's, a group's `inner.port`).
pub fn diagnose(m: &Model, at: &str, message: &str, values: crate::trace::Values) -> Option<Diagnosis> {
    let path: Vec<String> = at.split('/').map(String::from).collect();
    let (scope, id) = path.split_at(path.len() - 1);
    let n = m.scope(scope)?.iter().find(|n| n.id == id[0])?;
    let mut inputs = Vec::new();
    for (port, src) in &n.inputs {
        let side = match src {
            Source::Literal(v) => Side {
                port: port.clone(),
                from: crate::document::words(v),
                node: None,
                kind: "a value written in the model".into(),
                grid: None,
            },
            Source::Ref(rf) => {
                let (node, value) = match Ref::parse(rf) {
                    Ok(Ref::Port { node, port: p }) => {
                        // a node of the top level (its value kept), else of a group (kept as `inner.port`)
                        let kept = if scope.is_empty() {
                            values(&node, &p)
                        } else {
                            let inner: Vec<&str> =
                                scope[1..].iter().map(String::as_str).chain([node.as_str(), p.as_str()]).collect();
                            values(&scope[0], &inner.join(".")).or_else(|| values(&node, &p))
                        };
                        (Some(node), kept)
                    }
                    _ => (None, None),
                };
                Side {
                    port: port.clone(),
                    from: rf.clone(),
                    node,
                    kind: value.as_ref().map_or_else(
                        || {
                            if rf.starts_with('$') || rf.starts_with('@') {
                                "an input".into()
                            } else {
                                "gave nothing (it did not run, or failed)".into()
                            }
                        },
                        kind,
                    ),
                    grid: value.as_ref().and_then(crate::trace::grid_of).map(|g: Arc<Grid>| GridInfo::of(&g)),
                }
            }
        };
        inputs.push(side);
    }
    let hints = hints(&inputs);
    Some(Diagnosis { at: at.to_string(), op: n.op.clone(), message: message.to_string(), inputs, hints })
}

/// What does not fit among the inputs, in words, with what puts it right.
fn hints(inputs: &[Side]) -> Vec<String> {
    let mut out = Vec::new();
    let maps: Vec<(&str, &GridInfo)> =
        inputs.iter().filter_map(|s| Some((s.port.as_str(), s.grid.as_ref()?))).collect();
    for s in inputs.iter().filter(|s| s.kind.starts_with("gave nothing")) {
        out.push(format!("{} ({}) gave nothing: it did not run, or failed first; look at it first", s.port, s.from));
    }
    if let Some((_, g)) = maps.first() {
        let list = |f: &dyn Fn(&GridInfo) -> String| {
            maps.iter().map(|(p, x)| format!("{p} {}", f(x))).collect::<Vec<_>>().join(", ")
        };
        if maps.iter().any(|(_, x)| x.crs != g.crs) {
            out.push(format!(
                "the maps are of different coordinate systems ({}): put them on one with raster.reproject, or the \
                 Prepare pane's steps (to the reference grid)",
                list(&|x| x.crs.clone())
            ));
        }
        if maps.iter().any(|(_, x)| (x.width, x.height) != (g.width, g.height)) {
            out.push(format!(
                "the maps are of different sizes ({}): an operator of cell by cell maps needs one grid; put them on \
                 one with raster.match (or the Prepare pane's \"to the reference grid\")",
                list(&|x| x.size())
            ));
        } else if maps.iter().any(|(_, x)| x.cell != g.cell) {
            out.push(format!(
                "the maps' cells are of different sizes ({}): raster.match puts them on one grid",
                list(&|x| x.cell_size())
            ));
        } else if maps.iter().any(|(_, x)| x.origin != g.origin) {
            out.push(format!(
                "the maps are of the same size but not in the same place (origins {}): raster.match aligns them",
                list(&|x| format!("{}, {}", cell(x.origin.0), cell(x.origin.1)))
            ));
        }
    }
    out
}

impl Diagnosis {
    /// The diagnosis in a few lines of words (for the assistant, a copy).
    pub fn text(&self) -> String {
        let mut out = format!("The node {} ({}) failed: {}\n", self.at, self.op, self.message);
        if !self.inputs.is_empty() {
            out.push_str("What it was given:\n");
            for s in &self.inputs {
                let grid = s.grid.as_ref().map_or(String::new(), |g| {
                    format!(
                        ": {}, cells of {}, {}, origin {}, {}",
                        g.size(),
                        g.cell_size(),
                        g.crs,
                        cell(g.origin.0),
                        cell(g.origin.1)
                    )
                });
                out.push_str(&format!("- {} <- {} = {}{grid}\n", s.port, s.from, s.kind));
            }
        }
        for h in &self.hints {
            out.push_str(&format!("Likely: {h}.\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_of_different_grids_are_told() {
        let m = terraflux_graph::Model::from_toml(
            r#"
format = "terraflux/1"
[[nodes]]
id = "small"
op = "nlm.random"
params = { rows = 10, columns = 10, seed = 1 }
[[nodes]]
id = "big"
op = "nlm.random"
params = { rows = 20, columns = 10, seed = 2 }
[[nodes]]
id = "sum"
op = "map.calc"
inputs = { a = "small.map", b = "big.map" }
params = { expression = "a + b" }
"#,
        )
        .unwrap();
        let Err(e) = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet) else {
            panic!("maps of different grids added")
        };
        let (at, message) = failed_at(&e.to_string()).unwrap_or_else(|| panic!("{e}"));
        assert_eq!(at, "sum");
        let cache = std::sync::Mutex::new(terraflux_exec::Cache::default());
        let _ = terraflux_exec::Engine::default().run_with(
            &m,
            &Default::default(),
            &mut terraflux_exec::Quiet,
            Some(&cache),
            None,
        );
        let c = cache.lock().unwrap();
        let values = |n: &str, p: &str| c.outputs(n)?.get(p).cloned();
        let d = diagnose(&m, &at, &message, &values).unwrap();
        assert_eq!(d.inputs.len(), 2);
        assert_eq!(d.inputs[0].grid.as_ref().unwrap().size(), "10 x 10 cells");
        assert_eq!(d.inputs[1].grid.as_ref().unwrap().size(), "10 x 20 cells");
        assert!(d.hints.iter().any(|h| h.contains("different sizes") && h.contains("raster.match")), "{:?}", d.hints);
        let t = d.text();
        assert!(t.contains("The node sum (map.calc) failed"), "{t}");
        assert!(t.contains("b <- big.map = a map: 10 x 20 cells"), "{t}");
        // an input that gave nothing
        let none = |_: &str, _: &str| None;
        let d = diagnose(&m, "sum", "x", &none).unwrap();
        assert!(d.hints.iter().any(|h| h.contains("gave nothing")));
        assert_eq!(failed_at("loop/step.land: no grid"), Some(("loop/step".into(), "no grid".into())));
    }
}
