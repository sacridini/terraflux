//! The Python counterparts of the operators of connectivity (conn, circuit, networks).

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[Py {
    op: "conn.centrality",
    call: "tf.conn.HabitatGraph.centrality",
    args: &[("graph", ""), ("distance", "distance"), ("probability", "probability"), ("binary", "binary")],
    defaults: &[],
    outs: &[("table", "")],
    kind: Kind::Method,
}];

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(_n: &Node, _var: &str, _sc: &Scope) -> Result<Option<Code>, String> {
    Ok(None)
}
