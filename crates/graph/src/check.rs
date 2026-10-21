//! Checking a model before it runs: every operator known, every reference resolved, every connection of
//! compatible types, no cycles.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use crate::{Body, Input, Model, Node, OpSignature, Ref, Source, Type, dependencies_among, ids, order, valid_id};

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
    /// the nodes by id
    index: Rc<HashMap<&'a str, &'a Node>>,
}

impl<'a> Scope<'a> {
    fn new(nodes: &'a [Node], container: Option<&'a Node>) -> Scope<'a> {
        Scope { nodes, container, index: Rc::new(nodes.iter().map(|n| (n.id.as_str(), n)).collect()) }
    }
}

struct Checker<'a> {
    model: &'a Model,
    catalog: &'a dyn Catalog,
    issues: Vec<Issue>,
    /// the signatures asked of the catalog, once each
    sigs: RefCell<HashMap<String, Option<Rc<OpSignature>>>>,
    /// whether each node runs once per layer ([`Checker::per_layer`]), found once, by the node's address
    layered: RefCell<HashMap<usize, Option<Type>>>,
}

impl<'a> Checker<'a> {
    fn signature(&self, op: &str) -> Option<Rc<OpSignature>> {
        if let Some(s) = self.sigs.borrow().get(op) {
            return s.clone();
        }
        let s = self.catalog.signature(op).map(Rc::new);
        self.sigs.borrow_mut().insert(op.to_string(), s.clone());
        s
    }

    fn error(&mut self, at: &str, message: String) {
        self.issues.push(Issue { severity: Severity::Error, at: at.to_string(), message });
    }

    fn warn(&mut self, at: &str, message: String) {
        self.issues.push(Issue { severity: Severity::Warning, at: at.to_string(), message });
    }

    /// Why a key of a parameter or scenario (`$input`, `node.setting`, `container/node.setting`) names nothing the
    /// model has: none when it names an input of a value, or a setting or an input of a node's operator.
    fn unknown_key(&self, key: &str) -> Option<String> {
        if let Some(name) = key.strip_prefix('$') {
            return match self.model.inputs.get(name) {
                Some(Input::Value { .. }) => None,
                Some(_) => Some(format!("${name} is a file, not a value")),
                None => Some(format!("the model has no input ${name}")),
            };
        }
        let Some((path, name)) = key.rsplit_once('.') else {
            return Some(format!("{key:?}: give $input or node.setting"));
        };
        let mut nodes = &self.model.nodes;
        let mut node = None;
        for id in path.split('/') {
            let Some(n) = nodes.iter().find(|n| n.id == id) else {
                return Some(format!("the model has no node {path:?}"));
            };
            node = Some(n);
            nodes = match &n.body {
                Some(b) => &b.nodes,
                None => nodes,
            };
        }
        let n = node?;
        let sig = self.signature(&n.op)?;
        let known = sig.params.iter().any(|p| p.name == name)
            || sig.input_spec(name).is_some()
            || n.inputs.contains_key(name)
            || n.params.contains_key(name);
        (!known).then(|| format!("{} ({}) has no setting {name:?}", n.id, n.op))
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
                    if c.op == crate::GROUP || c.op == crate::SWITCH {
                        if let Some(s) = c.inputs.get(n).filter(|_| !(c.op == crate::SWITCH && n == "case")) {
                            return self.source_type(Some(s), outside);
                        }
                        continue;
                    }
                    if c.op == crate::EACH {
                        return match n.as_str() {
                            "item" => Ok(match self.source_type(c.inputs.get("over"), outside)? {
                                Type::Stack | Type::Raster | Type::Map => Type::Map,
                                t @ (Type::Table | Type::Vector) => t,
                                _ => Type::Any,
                            }),
                            "index" => Ok(Type::Integer),
                            "name" => Ok(Type::Text),
                            _ => match c.inputs.get(n) {
                                Some(s) => self.source_type(Some(s), outside),
                                None => Err(format!("@{n} is not an input of {:?} (@item, @index, @name)", c.id)),
                            },
                        };
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
                    if let Some(n) = s.index.get(node.as_str()) {
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
        let sig = self.signature(&n.op).ok_or_else(|| format!("unknown operator {:?}", n.op))?;
        if let Some(p) = sig.output_spec(port) {
            // run once per layer: its maps a stack (a series of a loop's), its tables one, its other values a series
            return Ok(match (self.per_layer(n, &sig, scopes), p.ty) {
                (None, t) | (Some(_), t @ Type::Table) => t,
                (Some(many), Type::Map) => many,
                (Some(_), _) => Type::Series,
            });
        }
        if sig.container
            && let Some(b) = &n.body
        {
            // a switch gives what its branches give (the first's types)
            if n.op == crate::SWITCH
                && let Some(first) = b.nodes.first()
                && first.body.as_ref().is_some_and(|fb| fb.outputs.contains_key(port))
            {
                let mut inner: Vec<Scope<'a>> = scopes
                    .iter()
                    .map(|s| Scope { nodes: s.nodes, container: s.container, index: s.index.clone() })
                    .collect();
                inner.push(Scope::new(&b.nodes, Some(n)));
                return self.port_type(first, port, &inner);
            }
            if let Some(r) = b.collect.get(port) {
                if n.op != crate::EACH {
                    return Ok(Type::Series);
                }
                // what each element gave, gathered: maps a stack, tables a table, the rest a series
                let mut inner: Vec<Scope<'a>> = scopes
                    .iter()
                    .map(|s| Scope { nodes: s.nodes, container: s.container, index: s.index.clone() })
                    .collect();
                inner.push(Scope::new(&b.nodes, Some(n)));
                let t = Ref::parse(r).map_err(|e| e.to_string()).and_then(|r| self.type_of(&r, &inner))?;
                return Ok(match t {
                    Type::Map => Type::Stack,
                    Type::Table => Type::Table,
                    _ => Type::Series,
                });
            }
            if b.carry.contains_key(port) {
                return self.source_type(n.inputs.get(port), scopes);
            }
            if let Some(r) = b.outputs.get(port) {
                let mut inner: Vec<Scope<'a>> = scopes
                    .iter()
                    .map(|s| Scope { nodes: s.nodes, container: s.container, index: s.index.clone() })
                    .collect();
                inner.push(Scope::new(&b.nodes, Some(n)));
                return Ref::parse(r).map_err(|e| e.to_string()).and_then(|r| self.type_of(&r, &inner));
            }
        }
        Err(format!("node {:?} ({}) has no output {port:?}", n.id, n.op))
    }

    /// Whether a node runs once per layer, and the type of the layers it is given (a stack, or a series when one
    /// of them is): a port of one map given a stack or a series of maps (rasters on disk are only known to be many
    /// bands when they are read).
    fn per_layer(&self, n: &'a Node, sig: &OpSignature, scopes: &[Scope<'a>]) -> Option<Type> {
        let key = n as *const Node as usize;
        if let Some(t) = self.layered.borrow().get(&key) {
            return *t;
        }
        let mut many = None;
        for (_, src) in n.inputs.iter().filter(|(p, _)| sig.per_layer(p)) {
            match self.source_type(Some(src), scopes) {
                Ok(Type::Series) => many = Some(Type::Series),
                Ok(Type::Stack) => many = many.or(Some(Type::Stack)),
                _ => {}
            }
        }
        self.layered.borrow_mut().insert(key, many);
        many
    }

    /// A reference checked against the type of its port; `one_map`: the port takes one map alone (its operator
    /// takes its maps together, or is a container), not a stack's layers one at a time.
    fn check_ref(&mut self, at: &str, text: &str, want: Type, scopes: &[Scope<'a>], one_map: bool) {
        match Ref::parse(text) {
            Err(e) => self.error(at, e.to_string()),
            Ok(r) => match self.type_of(&r, scopes) {
                Err(e) => self.error(at, e),
                Ok(t) if !t.fits(want) => {
                    self.error(at, format!("{text} gives a {t:?}, the port takes a {want:?}"));
                }
                Ok(t @ (Type::Stack | Type::Series)) if one_map && want == Type::Map => {
                    self.error(at, format!("{text} gives a {t:?}: the port takes one map, not its layers one by one"));
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
            let Some(sig) = self.signature(&n.op) else {
                self.error(&at, format!("unknown operator {:?}", n.op));
                continue;
            };
            for (port, src) in &n.inputs {
                let carried = sig.container
                    && (n.op == crate::GROUP || n.body.as_ref().is_some_and(|b| b.carry.contains_key(port)));
                let want = match sig.input_spec(port) {
                    Some(p) => p.ty,
                    None if carried => Type::Any,
                    None if sig.extra.is_some() && crate::valid_port(port) => sig.extra.unwrap(),
                    None => {
                        self.error(&at, format!("{} has no input {port:?}", n.op));
                        continue;
                    }
                };
                match src {
                    Source::Ref(r) => self.check_ref(&format!("{at}.{port}"), r, want, scopes, !sig.per_layer(port)),
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
            for (k, v) in &n.params {
                if !known.contains(k.as_str()) {
                    self.warn(&at, format!("{} does not read the setting {k:?}", n.op));
                }
                // a setting that reads a value: a number, a whole number, true or false, a text
                if let Some(r) = crate::setting_ref(v) {
                    let want = sig.params.iter().find(|p| p.name == *k).map_or(Type::Any, |p| p.ty);
                    let at = format!("{at}.{k}");
                    match Ref::parse(r).map_err(|e| e.to_string()).and_then(|x| self.type_of(&x, scopes)) {
                        Err(e) => self.error(&at, e),
                        Ok(t) if !matches!(t, Type::Number | Type::Integer | Type::Bool | Type::Text | Type::Any) => {
                            self.error(
                                &at,
                                format!("{r} gives a {t:?}: a setting takes a number, true or false, or a text"),
                            );
                        }
                        Ok(t) if !t.fits(want) => {
                            self.error(&at, format!("{r} gives a {t:?}, the setting takes a {want:?}"))
                        }
                        Ok(_) => {}
                    }
                }
            }
            match (&n.body, sig.container) {
                (Some(_), false) => self.error(&at, format!("{} is not a container: it takes no body", n.op)),
                (None, true) => self.error(&at, format!("{} needs a body of nodes", n.op)),
                (Some(b), true) => self.check_body(n, b, scopes, &at),
                (None, false) => {}
            }
        }
        let among = ids(nodes);
        if let Err(e) = order(nodes, |n| dependencies_among(n, &among)) {
            self.error(prefix, e.to_string());
        }
    }

    fn check_body(&mut self, n: &'a Node, b: &'a Body, scopes: &mut Vec<Scope<'a>>, at: &str) {
        if n.op == crate::SWITCH {
            if !b.carry.is_empty() || !b.collect.is_empty() || b.until.is_some() || !b.outputs.is_empty() {
                self.error(
                    at,
                    "control.switch gives its branches' outputs: it carries, collects and gives nothing of its own"
                        .into(),
                );
            }
            if b.nodes.is_empty() {
                self.error(at, "control.switch needs its branches: a group each in its body".into());
            }
            let outputs = |g: &Node| -> BTreeSet<String> {
                g.body.as_ref().map(|gb| gb.outputs.keys().cloned().collect()).unwrap_or_default()
            };
            for g in &b.nodes {
                if g.op != crate::GROUP {
                    self.error(
                        &format!("{at}/{}", g.id),
                        "a branch of control.switch is a group (control.group)".into(),
                    );
                } else if let Some(first) = b.nodes.first()
                    && outputs(g) != outputs(first)
                {
                    let names = |s: BTreeSet<String>| s.into_iter().collect::<Vec<_>>().join(", ");
                    self.error(
                        &format!("{at}/{}", g.id),
                        format!(
                            "every branch gives the same outputs: {} gives {}, {} gives {}",
                            g.id,
                            names(outputs(g)),
                            first.id,
                            names(outputs(first))
                        ),
                    );
                }
            }
        }
        if n.op == crate::EACH && (!b.carry.is_empty() || b.until.is_some() || !b.outputs.is_empty()) {
            self.error(
                at,
                "control.each collects what its elements give; it carries nothing, and has no until or outputs".into(),
            );
        }
        for name in b.carry.keys() {
            if !valid_id(name) || name == "step" {
                self.error(at, format!("{name:?} cannot be carried"));
            }
            if !n.inputs.contains_key(name) {
                self.error(at, format!("the carried value @{name} needs a first value: an input {name:?}"));
            }
        }
        scopes.push(Scope::new(&b.nodes, Some(n)));
        self.check_scope(scopes, at);
        for (name, r) in &b.carry {
            self.check_ref(&format!("{at}/carry.{name}"), r, Type::Any, scopes, false);
        }
        for (name, r) in &b.collect {
            self.check_ref(&format!("{at}/collect.{name}"), r, Type::Any, scopes, false);
        }
        for (name, r) in &b.outputs {
            self.check_ref(&format!("{at}/outputs.{name}"), r, Type::Any, scopes, false);
        }
        if let Some(r) = &b.until {
            self.check_ref(&format!("{at}/until"), r, Type::Bool, scopes, false);
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
    let mut c = Checker {
        model,
        catalog,
        issues: Vec::new(),
        sigs: RefCell::new(HashMap::new()),
        layered: RefCell::new(HashMap::new()),
    };
    for name in model.inputs.keys() {
        if !valid_id(name) {
            c.error("", format!("{name:?} is not a valid input name"));
        }
    }
    let mut scopes = vec![Scope::new(&model.nodes, None)];
    c.check_scope(&mut scopes, "");
    for (name, out) in &model.outputs {
        c.check_ref(&format!("outputs.{name}"), &out.from, Type::Any, &scopes, false);
    }
    // what the parameters and scenarios change: a warning when it is not in the model (a node renamed away)
    let keys =
        model.parameters.iter().map(|p| (format!("parameters.{}", p.key), &p.key)).chain(
            model.scenarios.iter().flat_map(|s| s.values.keys().map(move |k| (format!("scenarios.{}", s.name), k))),
        );
    for (at, key) in keys {
        if let Some(why) = c.unknown_key(key) {
            c.warn(&at, why);
        }
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
                "layers" => {
                    OpSignature::new("layers", "", Access::None).output(PortSpec::new("stack", Type::Stack, ""))
                }
                "classes" => OpSignature::new("classes", "", Access::Global)
                    .input(PortSpec::new("map", Type::Map, ""))
                    .output(PortSpec::new("map", Type::Map, ""))
                    .output(PortSpec::new("share", Type::Number, ""))
                    .output(PortSpec::new("table", Type::Table, "")),
                "takes_stack" => OpSignature::new("takes_stack", "", Access::Local)
                    .input(PortSpec::new("stack", Type::Stack, ""))
                    .input(PortSpec::optional("n", Type::Number, "")),
                "of" => OpSignature::new("of", "", Access::Local).extra(Type::Map).together(),
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

    /// A stack where a port takes one map: the node runs once per layer, its maps a stack, its tables a table,
    /// its other values a series; refused by an operator that takes its maps together.
    #[test]
    fn a_stack_runs_a_map_port_once_per_layer() {
        let m = model(
            r#"
[[nodes]]
id = "s"
op = "layers"
[[nodes]]
id = "c"
op = "classes"
inputs = { map = "s.stack" }
[[nodes]]
id = "again"
op = "classes"
inputs = { map = "c.map" }
[[nodes]]
id = "use"
op = "takes_stack"
inputs = { stack = "again.map", n = "c.share" }
"#,
        );
        let e = errors(&m);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("use.n: c.share gives a Series"), "{e:?}");
        let together = model(
            r#"
[[nodes]]
id = "s"
op = "layers"
[[nodes]]
id = "o"
op = "of"
inputs = { a = "s.stack" }
"#,
        );
        assert!(errors(&together).iter().any(|x| x.contains("takes one map")), "{:?}", errors(&together));
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
    fn settings_read_values() {
        // a setting reads the step, a value input, a node's number; not a map, nor what is not there
        let with = |k: &str| {
            let mut m = model(&GOOD.replace("params = { k = 1 }", &format!("params = {{ k = {k} }}")));
            m.inputs.insert("rate".into(), Input::Value { value: 0.5.into(), note: None });
            m.inputs.insert("flag".into(), Input::Value { value: true.into(), note: None });
            m.nodes.push(crate::Node::new("c", "classes").input("map", "$t0"));
            errors(&m)
        };
        for ok in ["\"@step\"", "\"$rate\"", "{ from = \"c.share\" }", "{ from = \"$rate\" }", "\"a.b\""] {
            assert!(with(ok).is_empty(), "{ok}: {:?}", with(ok));
        }
        assert!(with("\"$t0\"").iter().any(|x| x.contains("gives a Map: a setting takes")), "{:?}", with("\"$t0\""));
        assert!(with("{ from = \"c.table\" }").iter().any(|x| x.contains("gives a Table")));
        assert!(with("\"$flag\"").iter().any(|x| x.contains("the setting takes a Number")), "{:?}", with("\"$flag\""));
        assert!(with("\"$none\"").iter().any(|x| x.contains("no input named $none")));
        assert!(with("{ from = \"c.nothing\" }").iter().any(|x| x.contains("no output \"nothing\"")));
        assert!(with("{ from = \"nonsense\" }").iter().any(|x| x.contains("is not a reference")));
        // read before: the node that gives it runs first
        let mut m = model(GOOD);
        m.nodes.push(crate::Node::new("c", "classes").input("map", "$t0"));
        m.nodes[0].params.insert("k".into(), serde_json::json!({ "from": "c.share" }));
        assert_eq!(crate::dependencies(&m.nodes[0], &m.nodes), ["c"]);
    }

    #[test]
    fn unknown_settings_are_warnings() {
        let m = model(&GOOD.replace("params = { k = 1 }", "params = { kk = 1 }"));
        let all = check(&m, &Ops);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].severity, Severity::Warning);
    }

    #[test]
    fn parameters_name_what_the_model_has() {
        let mut m = model(GOOD);
        assert!(check(&m, &Ops).is_empty());
        // (a node of a loop's body)
        let step = "sim/s";
        m.inputs.insert("rate".into(), Input::Value { value: 0.5.into(), note: None });
        for key in ["$rate", &format!("{step}.k"), &format!("{step}.n")] {
            m.parameters.push(crate::Parameter { key: key.to_string(), ..Default::default() });
        }
        assert!(check(&m, &Ops).is_empty(), "{:?}", check(&m, &Ops));
        m.parameters.push(crate::Parameter { key: "$nothing".into(), ..Default::default() });
        m.parameters.push(crate::Parameter { key: format!("{step}.zz"), ..Default::default() });
        m.scenarios.push(crate::Scenario {
            name: "s".into(),
            note: None,
            values: [("gone.k".to_string(), serde_json::json!(1)), ("$t0".to_string(), serde_json::json!(1))].into(),
        });
        let all = check(&m, &Ops);
        assert_eq!(all.len(), 4, "{all:?}");
        assert!(all.iter().all(|i| i.severity == Severity::Warning));
    }
}
