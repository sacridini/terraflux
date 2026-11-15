//! The Python counterparts of the operators of prioritization and restoration (prioritize, restoration).

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[];

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(_n: &Node, _var: &str, _sc: &Scope) -> Result<Option<Code>, String> {
    Ok(None)
}
