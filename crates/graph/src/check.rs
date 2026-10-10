//! Checking a model before it runs: every operator known, every reference resolved, every connection of
//! compatible types, no cycles.

use std::collections::BTreeSet;

use crate::{Body, Input, Model, Node, OpSignature, Ref, Source, Type, dependencies, order, valid_id};

/// The operators a model may use (the engine's registry).
pub trait Catalog {
    fn signature(&self, op: &str) -> Option<OpSignature>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

/// A problem found in a model: where (a path of node ids) and what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub at: String,
    pub message: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        if self.at.is_empty() {
            write!(f, "{s}: {}", self.message)
        } else {
            write!(f, "{s}: {}: {}", self.at, self.message)
        }
    }
}

struct Scope<'a> {
    nodes: &'a [Node],
    /// the container whose body this scope is
    container: Option<&'a Node>,
}

struct Checker<'a> {
    model: &'a Model,
    catalog: &'a dyn Catalog,
    issues: Vec<Issue>,
}

impl<'a> Checker<'a> {
    fn error(&mut self, at: &str, message: String) {
        self.issues.push(Issue { severity: Severity::Error, at: at.to_string(), message });
    }

    fn warn(&mut self, at: &str, message: String) {
        self.issues.push(Issue { severity: Severity::Warning, at: at.to_string(), message });
    }

    fn input_type(&self, name: &str) -> Option<Type> {
        Some(match self.model.inputs.get(name)? {
            Input::Map { .. } => Type::Map,
            Input::Stack { .. } => Type::Stack,
            Input::Value { value, .. } => {
                if value.is_i64() || value.is_u64() {
                    Type::Integer
                } else if value.is_number() {
                    Type::Number
                } else if value.is_boolean() {
                    Type::Bool
                } else {
                    Type::Any
                }
            }
        })
    }

    /// The type of a resolved reference, or why it does not resolve.
    fn type_of(&self, r: &Ref, scopes: &[Scope<'a>]) -> Result<Type, String> {
        match r {
            Ref::Input(n) => self.input_type(n).ok_or_else(|| format!("no input named ${n}")),
            Ref::Carried(n) => {
                // the innermost container that has it: a group's inputs, a loop's step and carried values
                let mut any = false;
                for (i, sc) in scopes.iter().enumerate().rev() {
                    let Some(c) = sc.container else { continue };
                    any = true;
                    let outside = &scopes[..i];
                    if c.op == crate::GROUP {
                        if let Some(s) = c.inputs.get(n) {
                            return self.source_type(Some(s), outside);
                        }
                        continue;
                    }
                    if n == "step" {
                        return Ok(Type::Integer);
                    }
                    if c.body.as_ref().is_some_and(|b| b.carry.contains_key(n)) {
                        return self.source_type(c.inputs.get(n), outside);
                    }
                    return Err(format!("@{n} is not carried by {:?}", c.id));
                }
                if any {
                    Err(format!("@{n} is not an input of the enclosing groups"))
                } else {
                    Err(format!("@{n} outside a container"))
                }
            }
            Ref::Port { node, port } => {
                for (depth, s) in scopes.iter().enumerate().rev() {
                    if let Some(n) = s.nodes.iter().find(|x| &x.id == node) {
                        return self.port_type(n, port, &scopes[..=depth]);
                    }
                }
                Err(format!("no node named {node:?}"))
            }
        }
    }

    fn source_type(&self, s: Option<&Source>, scopes: &[Scope<'a>]) -> Result<Type, String> {
        match s {
            None => Ok(Type::Any),
            Some(Source::Literal(v)) => Ok(literal_type(v)),
            Some(Source::Ref(r)) => match Ref::parse(r) {
                Ok(r) => self.type_of(&r, scopes),
                Err(e) => Err(e.to_string()),
            },
        }
    }

    fn port_type(&self, n: &'a Node, port: &str, scopes: &[Scope<'a>]) -> Result<Type, String> {
        let sig = self.catalog.signature(&n.op).ok_or_else(|| format!("unknown operator {:?}", n.op))?;
        if let Some(p) = sig.output_spec(port) {
            return Ok(p.ty);
        }
        if sig.container
            && let Some(b) = &n.body
        {
            if b.collect.contains_key(port) {
                return Ok(Type::Series);
            }
            if b.carry.contains_key(port) {
                return self.source_type(n.inputs.get(port), scopes);
            }
            if let Some(r) = b.outputs.get(port) {
                let mut inner: Vec<Scope<'a>> =
                    scopes.iter().map(|s| Scope { nodes: s.nodes, container: s.container }).collect();
                inner.push(Scope { nodes: &b.nodes, container: Some(n) });
                return Ref::parse(r).map_err(|e| e.to_string()).and_then(|r| self.type_of(&r, &inner));
            }
        }
        Err(format!("node {:?} ({}) has no output {port:?}", n.id, n.op))
    }

    fn check_ref(&mut self, at: &str, text: &str, want: Type, scopes: &[Scope<'a>]) {
        match Ref::parse(text) {
            Err(e) => self.error(at, e.to_string()),
            Ok(r) => match self.type_of(&r, scopes) {
                Err(e) => self.error(at, e),
                Ok(t) if !t.fits(want) => {
                    self.error(at, format!("{text} gives a {t:?}, the port takes a {want:?}"));
                }
                Ok(_) => {}
            },
        }
    }

    fn check_scope(&mut self, scopes: &mut Vec<Scope<'a>>, prefix: &str) {
        let nodes = scopes.last().unwrap().nodes;
        let mut seen = BTreeSet::new();
        for n in nodes {
            let at = if prefix.is_empty() { n.id.clone() } else { format!("{prefix}/{}", n.id) };
            if !valid_id(&n.id) {
                self.error(&at, format!("{:?} is not a valid node id", n.id));
            }
            if !seen.insert(n.id.as_str()) {
                self.error(&at, format!("two nodes are named {:?}", n.id));
            }
            let Some(sig) = self.catalog.signature(&n.op) else {
                self.error(&at, format!("unknown operator {:?}", n.op));
                continue;
            };
            for (port, src) in &n.inputs {
                let carried = sig.container
                    && (n.op == crate::GROUP || n.body.as_ref().is_some_and(|b| b.carry.contains_key(port)));
                let want = match sig.input_spec(port) {
                    Some(p) => p.ty,
                    None if carried => Type::Any,
                    None if sig.extra.is_some() && valid_id(port) => sig.extra.unwrap(),
                    None => {
                        self.error(&at, format!("{} has no input {port:?}", n.op));
                        continue;
                    }
                };
                match src {
                    Source::Ref(r) => self.check_ref(&format!("{at}.{port}"), r, want, scopes),
                    Source::Literal(v) => {
                        let t = literal_type(v);
                        if !t.fits(want) {
                            self.error(&format!("{at}.{port}"), format!("a literal {t:?}, the port takes a {want:?}"));
                        }
                    }
                }
            }
            for p in sig.inputs.iter().filter(|p| p.required) {
                if !n.inputs.contains_key(&p.name) {
                    self.error(&at, format!("{} needs the input {:?} ({})", n.op, p.name, p.doc));
                }
            }
            let known: BTreeSet<&str> = sig.params.iter().map(|p| p.name.as_str()).collect();
            for k in n.params.keys() {
                if !known.contains(k.as_str()) {
                    self.warn(&at, format!("{} does not read the setting {k:?}", n.op));
                }
            }
            match (&n.body, sig.container) {
                (Some(_), false) => self.error(&at, format!("{} is not a container: it takes no body", n.op)),
                (None, true) => self.error(&at, format!("{} needs a body of nodes", n.op)),
                (Some(b), true) => self.check_body(n, b, scopes, &at),
                (None, false) => {}
            }
        }
        if let Err(e) = order(nodes, |n| dependencies(n, nodes)) {
            self.error(prefix, e.to_string());
        }
    }

    fn check_body(&mut self, n: &'a Node, b: &'a Body, scopes: &mut Vec<Scope<'a>>, at: &str) {
        for name in b.carry.keys() {
            if !valid_id(name) || name == "step" {
                self.error(at, format!("{name:?} cannot be carried"));
            }
            if !n.inputs.contains_key(name) {
                self.error(at, format!("the carried value @{name} needs a first value: an input {name:?}"));
            }
        }
        scopes.push(Scope { nodes: &b.nodes, container: Some(n) });
        self.check_scope(scopes, at);
        for (name, r) in &b.carry {
            self.check_ref(&format!("{at}/carry.{name}"), r, Type::Any, scopes);
        }
        for (name, r) in &b.collect {
            self.check_ref(&format!("{at}/collect.{name}"), r, Type::Any, scopes);
        }
        for (name, r) in &b.outputs {
            self.check_ref(&format!("{at}/outputs.{name}"), r, Type::Any, scopes);
        }
        scopes.pop();
    }
}

fn literal_type(v: &serde_json::Value) -> Type {
    if v.is_i64() || v.is_u64() {
        Type::Integer
    } else if v.is_number() {
        Type::Number
    } else if v.is_boolean() {
        Type::Bool
    } else if v.is_string() {
        Type::Text
    } else {
        Type::Any
    }
}

/// The problems of a model: errors stop it from running, warnings do not.
pub fn check(model: &Model, catalog: &dyn Catalog) -> Vec<Issue> {
    let mut c = Checker { model, catalog, issues: Vec::new() };
    for name in model.inputs.keys() {
        if !valid_id(name) {
            c.error("", format!("{name:?} is not a valid input name"));
        }
    }
    let mut scopes = vec![Scope { nodes: &model.nodes, container: None }];
    c.check_scope(&mut scopes, "");
    for (name, out) in &model.outputs {
        c.check_ref(&format!("outputs.{name}"), &out.from, Type::Any, &scopes);
    }
    let mut issues = c.issues;
    issues.sort_by_key(|i| std::cmp::Reverse(i.severity));
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Access, PortSpec};

    struct Ops;

    impl Catalog for Ops {
        fn signature(&self, op: &str) -> Option<OpSignature> {
            Some(match op {
                "fit" => OpSignature::new("fit", "", Access::Global)
                    .input(PortSpec::new("map", Type::Map, "a map"))
                    .output(PortSpec::new("model", Type::Woe, "")),
                "step" => OpSignature::new("step", "", Access::Halo(1))
                    .input(PortSpec::new("land", Type::Map, ""))
                    .input(PortSpec::new("model", Type::Woe, ""))
                    .input(PortSpec::optional("n", Type::Number, ""))
                    .output(PortSpec::new("land", Type::Map, ""))
                    .param(PortSpec::new("k", Type::Number, "")),
                "control.repeat" => OpSignature::new("control.repeat", "", Access::None)
                    .input(PortSpec::new("times", Type::Integer, ""))
                    .container(),
                _ => return None,
            })
        }
    }

    fn model(text: &str) -> Model {
        Model::from_toml(&format!("format = \"terraflux/1\"\n{text}")).unwrap()
    }

    fn errors(m: &Model) -> Vec<String> {
        check(m, &Ops).into_iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_string()).collect()
    }

    const GOOD: &str = r#"
[inputs.t0]
kind = "map"
path = "lu.tif"

[[nodes]]
id = "woe"
op = "fit"
inputs = { map = "$t0" }

[[nodes]]
id = "sim"
op = "control.repeat"
inputs = { times = 3, land = "$t0" }
[nodes.body]
carry = { land = "s.land" }
collect = { maps = "s.land" }
[[nodes.body.nodes]]
id = "s"
op = "step"
inputs = { land = "@land", model = "woe.model", n = "@step" }
params = { k = 1 }

[outputs.maps]
from = "sim.maps"
"#;

    #[test]
    fn a_good_model_has_no_issue() {
        assert!(check(&model(GOOD), &Ops).is_empty(), "{:?}", check(&model(GOOD), &Ops));
    }

    #[test]
    fn broken_references_are_found() {
        let m = model(&GOOD.replace("\"woe.model\"", "\"wo.model\"").replace("\"$t0\" }", "\"$t1\" }"));
        let e = errors(&m);
        assert!(e.iter().any(|x| x.contains("no node named \"wo\"")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("no input named $t1")), "{e:?}");
    }

    #[test]
    fn types_must_fit() {
        let m = model(&GOOD.replace("model = \"woe.model\"", "model = \"$t0\""));
        let e = errors(&m);
        assert!(e.iter().any(|x| x.contains("gives a Map, the port takes a Woe")), "{e:?}");
    }

    #[test]
    fn unknown_ports_ops_and_missing_inputs() {
        let m = model(&GOOD.replace("op = \"fit\"", "op = \"fitt\"").replace("n = \"@step\"", "q = 1"));
        let e = errors(&m);
        assert!(e.iter().any(|x| x.contains("unknown operator \"fitt\"")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("has no input \"q\"")), "{e:?}");
        let m = model(&GOOD.replace("inputs = { map = \"$t0\" }", ""));
        assert!(errors(&m).iter().any(|x| x.contains("needs the input \"map\"")));
    }

    #[test]
    fn carried_values_need_a_first_value() {
        let m = model(&GOOD.replace("inputs = { times = 3, land = \"$t0\" }", "inputs = { times = 3 }"));
        assert!(errors(&m).iter().any(|x| x.contains("needs a first value")));
    }

    #[test]
    fn unknown_settings_are_warnings() {
        let m = model(&GOOD.replace("params = { k = 1 }", "params = { kk = 1 }"));
        let all = check(&m, &Ops);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].severity, Severity::Warning);
    }
}
