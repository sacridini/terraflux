//! What an edit of the model makes out of date: the nodes whose own definition changed, those that read them (all
//! the way down) and those taken away, never what only draws (position, width, wires' reroutes) or tells (notes).

use std::collections::{BTreeSet, HashSet};
use terraflux_graph::{Model, Node};

/// What an edit invalidates, by the places of the statuses (`node`, `group/inner`).
#[derive(Debug, Default)]
pub struct Stale {
    /// The nodes whose status goes.
    pub keys: BTreeSet<String>,
    /// The places whose whole body's statuses go too (`group/`).
    pub trees: BTreeSet<String>,
}

impl Stale {
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Whether a status at `at` goes.
    pub fn hits(&self, at: &str) -> bool {
        self.keys.contains(at) || self.trees.iter().any(|t| at.starts_with(t.as_str()))
    }
}

/// The node as what it computes: no body's nodes (judged by themselves), no drawing, no record.
fn shallow(n: &Node) -> Node {
    let mut n = n.unrecorded();
    n.pos = None;
    n.wires.clear();
    n.width = None;
    n.shown.clear();
    n.hidden.clear();
    if let Some(b) = &mut n.body {
        b.nodes.clear();
    }
    n
}

/// The nodes of `new` out of date against `old`, the model's inputs `changed` among them.
pub fn between(old: &Model, new: &Model) -> Stale {
    let changed: HashSet<&str> = old
        .inputs
        .keys()
        .chain(new.inputs.keys())
        .filter(|k| old.inputs.get(*k) != new.inputs.get(*k))
        .map(String::as_str)
        .collect();
    let mut out = Stale::default();
    scope(&old.nodes, &new.nodes, "", &changed, &mut out);
    out
}

fn reads_changed(n: &Node, changed: &HashSet<&str>) -> bool {
    !changed.is_empty()
        && terraflux_graph::references(n)
            .iter()
            .any(|r| r.strip_prefix('$').is_some_and(|r| changed.contains(r.split(['.', '/']).next().unwrap_or(r))))
}

/// The ids of a scope's nodes that are out of date; their places are added to `out`.
fn scope(old: &[Node], new: &[Node], prefix: &str, changed: &HashSet<&str>, out: &mut Stale) -> HashSet<String> {
    let mut hit: HashSet<String> = HashSet::new();
    // (the nodes taken away count, for those that read them)
    for o in old {
        if !new.iter().any(|n| n.id == o.id) {
            hit.insert(o.id.clone());
            out.keys.insert(format!("{prefix}{}", o.id));
            out.trees.insert(format!("{prefix}{}/", o.id));
        }
    }
    let mut among: HashSet<&str> = new.iter().map(|n| n.id.as_str()).collect();
    among.extend(old.iter().map(|n| n.id.as_str()));
    for n in new {
        let key = format!("{prefix}{}", n.id);
        let o = old.iter().find(|o| o.id == n.id);
        let own = o.is_none_or(|o| shallow(o) != shallow(n)) || reads_changed(n, changed);
        if own {
            hit.insert(n.id.clone());
            out.keys.insert(key.clone());
            out.trees.insert(format!("{key}/"));
            continue;
        }
        if let (Some(ob), Some(nb)) = (o.and_then(|o| o.body.as_ref()), n.body.as_ref()) {
            let inner = scope(&ob.nodes, &nb.nodes, &format!("{key}/"), changed, out);
            if !inner.is_empty() {
                hit.insert(n.id.clone());
                out.keys.insert(key);
            }
        }
    }
    // what reads a node out of date, all the way down
    loop {
        let mut more = false;
        for n in new {
            if hit.contains(&n.id) {
                continue;
            }
            if terraflux_graph::dependencies_among(n, &among).iter().any(|d| hit.contains(d)) {
                let key = format!("{prefix}{}", n.id);
                hit.insert(n.id.clone());
                out.keys.insert(key.clone());
                out.trees.insert(format!("{key}/"));
                more = true;
            }
        }
        if !more {
            return hit;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Body;

    fn model(nodes: Vec<Node>) -> Model {
        let mut m = Model::new("m");
        m.nodes = nodes;
        m
    }

    #[test]
    fn only_what_an_edit_reaches_is_out_of_date() {
        let a = Node::new("a", "nlm.random").param("seed", 1);
        let b = Node::new("b", "x.op").input("map", "a.raster");
        let c = Node::new("c", "nlm.random");
        let old = model(vec![a.clone(), b.clone(), c.clone()]);
        let mut new = old.clone();
        new.nodes[0].params.insert("seed".into(), 2.into());
        let s = between(&old, &new);
        assert!(s.hits("a") && s.hits("b") && !s.hits("c"));
        // drawing and notes: nothing
        let mut new = old.clone();
        new.nodes[0].pos = Some([5.0, 5.0]);
        new.nodes[1].note = Some("why".into());
        assert!(between(&old, &new).is_empty());
        // inside a body
        let g = Node::new("g", "control.group").body(Body {
            nodes: vec![
                Node::new("x", "nlm.random"),
                Node::new("y", "x.op").input("map", "x.raster"),
                Node::new("z", "nlm.random"),
            ],
            ..Default::default()
        });
        let d = Node::new("d", "x.op").input("map", "g.out");
        let old = model(vec![g, d, c]);
        let mut new = old.clone();
        new.nodes[0].body.as_mut().unwrap().nodes[0].params.insert("seed".into(), 3.into());
        let s = between(&old, &new);
        assert!(s.hits("g") && s.hits("g/x") && s.hits("g/y") && s.hits("d"));
        assert!(!s.hits("g/z") && !s.hits("c"));
    }
}
