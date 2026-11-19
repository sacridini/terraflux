//! The class tree (object-based analysis, as InterIMAGE's): what there is in the scene, said first (Scene > Urban >
//! Buildings, Roads; Vegetation; Water), each class found by a method of its own inside the region its mother class
//! found. The tree is no structure of the model file: it is a *view* over ordinary nodes, by this convention.
//!
//! - A class is a `control.group` with the settings `class` (the name), `parent_class` (its mother's name; none for
//!   a root) and `colour` (`#rrggbb`). Its body reads `@region`, the region of its mother (a map of 1 and no value;
//!   the number 1, every cell, for a root), and the data the method needs (`@image`, `@objects`, `@samples`,
//!   `@ndsm`: the group's inputs, wired to the model's own data), and gives the output `membership`: a map, 0 to 1,
//!   of its class, with no value outside `@region`.
//! - The classes with one mother (the siblings, the roots among themselves) are decided by one `objects.resolve`:
//!   `members_of_<mother>` (`stack.of`, a layer per sibling) feeds `resolve_of_<mother>` (`members` and `resolve`
//!   for the roots). Each sibling's win is `region_<class>` (`raster.calc`: the class number of the resolve's
//!   `class` map, 1 and no value), the region its own children's groups read.
//!
//! [`read`] finds the classes of a scope of a model (and, for a model that uses `objects.rules` instead, the
//! hierarchy of its `classes` setting); [`add_class`] makes a class's branch, with a recipe for its body, and puts it
//! in its siblings' decision: the whole edit made on the model it is given, for the caller to keep as one step of
//! undo.

use std::collections::BTreeMap;

use terraflux_graph::{Body, GROUP, Input, Model, Node, Source, Type};
use terraflux_ops::{Outputs, Registry, Value};

/// The setting of a class's group that names it, and its mother's and its colour.
pub const CLASS: &str = "class";
pub const PARENT: &str = "parent_class";
pub const COLOUR: &str = "colour";

/// A class of the tree, as the view shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub colour: Option<[u8; 3]>,
    /// Its mother (an index of the list [`read`] gives), none for a root.
    pub parent: Option<usize>,
    pub depth: usize,
    /// Its group (or the `objects.rules` node it is a class of).
    pub node: String,
    /// A class of `objects.rules`, only read.
    pub rules: bool,
    /// How it is found, in a few words.
    pub method: String,
    /// The rule that gives its membership, when a calculator holds it.
    pub rule: Option<String>,
    /// The objects it was trained on, and the area found, once the model has run.
    pub samples: Option<usize>,
    pub area: Option<f64>,
}

/// The name of a class as a layer's name (and in the ids of its nodes): letters, digits, `_` and `-`.
pub fn slug(name: &str) -> String {
    let mut s: String =
        name.trim().chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert_str(0, "c_");
    }
    s
}

fn class_id(name: &str) -> String {
    format!("class_{}", slug(name))
}

fn region_id(name: &str) -> String {
    format!("region_{}", slug(name))
}

/// The ids of the stack and of the decision of the classes with a mother (none: the roots).
fn members_id(mother: Option<&str>) -> String {
    mother.map_or("members".to_string(), |m| format!("members_of_{}", slug(m)))
}

fn resolve_id(mother: Option<&str>) -> String {
    mother.map_or("resolve".to_string(), |m| format!("resolve_of_{}", slug(m)))
}

fn text<'a>(n: &'a Node, key: &str) -> Option<&'a str> {
    n.params.get(key).and_then(|v| v.as_str()).filter(|t| !t.trim().is_empty())
}

/// The classes of a scope: the groups that name a class.
fn groups(nodes: &[Node]) -> impl Iterator<Item = &Node> {
    nodes.iter().filter(|n| n.op == GROUP && text(n, CLASS).is_some())
}

/// The first line of a script that gives the membership (or the first line), without its `out membership =`.
fn rule_of(expression: &str) -> Option<String> {
    let line = expression
        .lines()
        .map(str::trim)
        .find(|l| l.contains("membership"))
        .or_else(|| expression.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')))?;
    let line = line.strip_prefix("out").map_or(line, str::trim);
    let line = line.strip_prefix("membership").map_or(line, str::trim);
    Some(line.strip_prefix('=').map_or(line, str::trim).to_string())
}

/// How a branch finds its class, and the rule when a calculator says it.
fn method_of(body: &Body) -> (String, Option<String>) {
    let calc = body.nodes.iter().find(|n| matches!(n.op.as_str(), "raster.calc" | "objects.calc"));
    let script = calc.and_then(|n| text(n, "expression")).map(str::to_lowercase).unwrap_or_default();
    let rule = calc.and_then(|n| text(n, "expression")).and_then(rule_of);
    if body.nodes.iter().any(|n| n.op == "objects.samples") {
        return ("by samples".into(), rule);
    }
    if script.contains("ndsm") {
        return ("by height (nDSM)".into(), rule);
    }
    if script.contains("ndwi") {
        return ("by NDWI".into(), rule);
    }
    if script.contains("ndvi") {
        return ("by NDVI".into(), rule);
    }
    if calc.is_some() {
        return ("by a calculation".into(), rule);
    }
    if body.nodes.is_empty() {
        return ("no method yet".into(), None);
    }
    let ops: Vec<&str> = body.nodes.iter().map(|n| n.op.as_str()).collect();
    (format!("by {}", ops.join(", ")), None)
}

/// The classes of a scope as a tree, in the order of the model, each mother before its children: the groups with a
/// `class` setting; when there are none, the hierarchy of the `classes` of an `objects.rules` node (only read).
pub fn read(nodes: &[Node]) -> Vec<Entry> {
    let found: Vec<&Node> = groups(nodes).collect();
    if found.is_empty() {
        return read_rules(nodes);
    }
    let names: Vec<&str> = found.iter().filter_map(|n| text(n, CLASS)).collect();
    // (a mother that is not a class of the scope: the class is a root)
    let mother: Vec<Option<usize>> =
        found.iter().map(|n| text(n, PARENT).and_then(|p| names.iter().position(|x| *x == p))).collect();
    let entry = |k: usize, depth: usize, parent: Option<usize>| {
        let n = found[k];
        let (method, rule) = n.body.as_ref().map_or(("no method yet".to_string(), None), method_of);
        Entry {
            name: names[k].to_string(),
            colour: text(n, COLOUR).and_then(terraflux_ops::legend::rgb),
            parent,
            depth,
            node: n.id.clone(),
            rules: false,
            method,
            rule,
            samples: None,
            area: None,
        }
    };
    // the roots, each one's children after it (depth first)
    fn walk(
        of: Option<usize>,
        at: Option<usize>,
        depth: usize,
        mother: &[Option<usize>],
        done: &mut [bool],
        out: &mut Vec<Entry>,
        entry: &dyn Fn(usize, usize, Option<usize>) -> Entry,
    ) {
        for k in 0..mother.len() {
            if !done[k] && mother[k] == of {
                done[k] = true;
                out.push(entry(k, depth, at));
                let here = out.len() - 1;
                walk(Some(k), Some(here), depth + 1, mother, done, out, entry);
            }
        }
    }
    let mut done = vec![false; found.len()];
    let mut out = Vec::new();
    walk(None, None, 0, &mother, &mut done, &mut out, &entry);
    // (a cycle of mothers, none of them a root: shown as roots)
    for (k, seen) in done.iter().enumerate() {
        if !seen {
            out.push(entry(k, 0, None));
        }
    }
    out
}

/// The hierarchy of the `classes` of the first `objects.rules` node of a scope that has one.
fn read_rules(nodes: &[Node]) -> Vec<Entry> {
    for n in nodes.iter().filter(|n| n.op == "objects.rules") {
        let Some(v) = n.params.get("classes") else { continue };
        let Ok(tree) = terraflux_ops::objects::tree(v) else { continue };
        let depth = |mut k: usize| {
            let mut d = 0;
            while let Some(p) = tree[k].parent {
                d += 1;
                k = p;
                if d > tree.len() {
                    break;
                }
            }
            d
        };
        // (each mother before its children)
        let mut order: Vec<usize> = Vec::new();
        fn push(tree: &[terraflux_ops::objects::RuleClass], of: Option<usize>, order: &mut Vec<usize>) {
            for k in (0..tree.len()).filter(|&k| tree[k].parent == of) {
                order.push(k);
                push(tree, Some(k), order);
            }
        }
        push(&tree, None, &mut order);
        return order
            .iter()
            .map(|&k| Entry {
                name: tree[k].name.clone(),
                colour: tree[k].colour,
                parent: tree[k].parent.and_then(|p| order.iter().position(|&o| o == p)),
                depth: depth(k),
                node: n.id.clone(),
                rules: true,
                method: if tree[k].rule.is_some() { "by rule".into() } else { "as its mother".into() },
                rule: tree[k].rule.clone(),
                samples: None,
                area: None,
            })
            .collect();
    }
    Vec::new()
}

/// What a run found, put in the entries: the area of each class (from its siblings' decision) and the objects it was
/// trained on (from its body's `objects.samples`).
pub fn with_results(entries: &mut [Entry], nodes: &[Node], outputs: &BTreeMap<String, Outputs>) {
    for k in 0..entries.len() {
        if entries[k].rules {
            continue;
        }
        let mother = entries[k].parent.map(|p| entries[p].name.clone());
        let resolve = resolve_id(mother.as_deref());
        let name = slug(&entries[k].name);
        if let Some(Value::Table(t)) = outputs.get(&resolve).and_then(|o| o.get("table")) {
            let (c, a) = (t.columns.iter().position(|c| c == "class"), t.columns.iter().position(|c| c == "area"));
            if let (Some(c), Some(a)) = (c, a) {
                entries[k].area =
                    Some(t.rows.iter().filter(|r| t.label("class", r[c]) == Some(name.as_str())).map(|r| r[a]).sum());
            }
        }
        let group = groups(nodes).find(|n| n.id == entries[k].node);
        let sampler =
            group.and_then(|g| g.body.as_ref()).and_then(|b| b.nodes.iter().find(|n| n.op == "objects.samples"));
        if let Some(s) = sampler
            && let Some(Value::Table(t)) =
                outputs.get(&entries[k].node).and_then(|o| o.get(&format!("{}.counts", s.id)))
            && let Some(c) = t.columns.iter().position(|c| c == "objects")
        {
            entries[k].samples = Some(t.rows.iter().map(|r| r[c]).sum::<f64>() as usize);
        }
    }
}

// -- adding a class ---------------------------------------------------------------------------------------------

/// A ready body for a class: what it reads and how it finds the class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipe {
    /// Water: the normalised difference of the green and the near infrared bands.
    Water,
    /// Vegetation: the normalised difference of the near infrared and the red bands.
    Vegetation,
    /// Buildings: the height above the ground (an nDSM).
    Buildings,
    /// Any class: a forest fitted on the objects the user's samples point to.
    Samples,
}

impl Recipe {
    pub const ALL: [Recipe; 4] = [Recipe::Water, Recipe::Vegetation, Recipe::Buildings, Recipe::Samples];

    pub fn label(self) -> &'static str {
        match self {
            Recipe::Water => "water by NDWI",
            Recipe::Vegetation => "vegetation by NDVI",
            Recipe::Buildings => "buildings by height",
            Recipe::Samples => "by samples",
        }
    }

    /// What the recipe does, to put in the group's note.
    pub fn about(self) -> &'static str {
        match self {
            Recipe::Water => {
                "Water by NDWI: the membership rises from 0 at an NDWI of 0 to 1 at 0.3 (the bands named green and nir)."
            }
            Recipe::Vegetation => {
                "Vegetation by NDVI: the membership rises from 0 at an NDVI of 0.2 to 1 at 0.5 (the bands named red and \
                 nir)."
            }
            Recipe::Buildings => {
                "Buildings by height: the membership rises from 0 at 2 m above the ground to 1 at 5 m (the nDSM, the \
                 height of the surface above the ground; add the shape of the objects to tell them from trees)."
            }
            Recipe::Samples => {
                "By samples: the objects the samples point to (a vector of points or polygons whose `class` attribute \
                 is the class's name, any other value for what it is not) train a random forest on the objects' \
                 attributes; the membership is 1 for the objects it gives this class."
            }
        }
    }
}

/// What a recipe's body reads besides `@region`: the names of the group's inputs.
fn needs(recipe: Recipe) -> &'static [&'static str] {
    match recipe {
        Recipe::Water | Recipe::Vegetation => &["image"],
        Recipe::Buildings => &["ndsm"],
        Recipe::Samples => &["image", "objects", "samples"],
    }
}

fn calc(id: &str, inputs: &[(&str, &str)], expression: &str) -> Node {
    let mut n = Node::new(id, "raster.calc").param("expression", expression);
    for (port, from) in inputs {
        n = n.input(port, from);
    }
    n
}

/// The nodes of a recipe's body for the class `name`, and the reference of its membership.
fn body_of(recipe: Recipe, name: &str) -> Body {
    let nodes = match recipe {
        Recipe::Water => vec![calc(
            "m",
            &[("image", "@image"), ("region", "@region")],
            "ndwi = (image.green - image.nir) / (image.green + image.nir)\nout membership = clamp(ndwi / 0.3, 0, 1) * region",
        )],
        Recipe::Vegetation => vec![calc(
            "m",
            &[("image", "@image"), ("region", "@region")],
            "ndvi = (image.nir - image.red) / (image.nir + image.red)\n\
             out membership = clamp((ndvi - 0.2) / 0.3, 0, 1) * region",
        )],
        Recipe::Buildings => vec![calc(
            "m",
            &[("ndsm", "@ndsm"), ("region", "@region")],
            "out membership = clamp((ndsm - 2) / 3, 0, 1) * region",
        )],
        Recipe::Samples => vec![
            Node::new("feat", "objects.features").input("objects", "@objects").input("image", "@image"),
            Node::new("train", "objects.samples").input("objects", "feat.objects").input("samples", "@samples"),
            Node::new("fit", "ml.fit").input("table", "train.table").param("column", "class"),
            Node::new("cls", "objects.classify")
                .input("objects", "feat.objects")
                .input("model", "fit.model")
                .input("samples", "train.table"),
            Node::new("mark", "objects.calc")
                .input("objects", "cls.objects")
                .param("expression", format!("out membership = 1 if cls == \"{name}\" else 0")),
            Node::new("map", "objects.to_map").input("objects", "mark.objects").param("what", "membership"),
            calc("m", &[("map", "map.map"), ("region", "@region")], "out membership = map * region"),
        ],
    };
    Body { nodes, outputs: [("membership".to_string(), "m.membership".to_string())].into(), ..Default::default() }
}

/// The class a new branch is for.
#[derive(Debug, Clone)]
pub struct NewClass {
    pub name: String,
    pub colour: [u8; 3],
    /// Its mother's name (none: a root class).
    pub parent: Option<String>,
    pub recipe: Recipe,
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// The class numbers the decision of the siblings gives them: by the names of their layers, as `stack.of` orders them.
fn number_of(siblings: &[String], name: &str) -> usize {
    let mut slugs: Vec<String> = siblings.iter().map(|s| slug(s)).collect();
    slugs.sort();
    slugs.iter().position(|s| *s == slug(name)).map_or(0, |k| k + 1)
}

/// The expression of a class's region node: 1 where the decision gave it the cell, no value elsewhere.
fn region_expression(k: usize) -> String {
    format!("out region = 1 if won == {k} else None")
}

/// A reference to the first output of a type in a scope (its nodes, not the classes' own), or a model input of its
/// kind: `$name` or `node.port`.
fn source_of(m: &Model, nodes: &[Node], r: &Registry, ty: Type, prefer: &str) -> Option<String> {
    let kind = |i: &Input| matches!((ty, i), (Type::Stack, Input::Stack { .. }) | (Type::Map, Input::Map { .. }));
    if let Some((name, _)) = m.inputs.iter().find(|(k, i)| kind(i) && k.as_str() == prefer) {
        return Some(format!("${name}"));
    }
    for n in nodes.iter().filter(|n| !is_ours(n) && n.id == prefer) {
        if let Some((p, _)) = crate::editor::output_ports(n, r).into_iter().find(|(_, t)| *t == ty) {
            return Some(format!("{}.{p}", n.id));
        }
    }
    if let Some((name, _)) = m.inputs.iter().find(|(_, i)| kind(i)) {
        return Some(format!("${name}"));
    }
    for n in nodes.iter().filter(|n| !is_ours(n)) {
        let ports = crate::editor::output_ports(n, r);
        if let Some((p, _)) =
            ports.iter().find(|(p, t)| *t == ty && p == prefer).or_else(|| ports.iter().find(|(_, t)| *t == ty))
        {
            return Some(format!("{}.{p}", n.id));
        }
    }
    None
}

/// A node of the tree's own (a class's group, the stack and the decision of the siblings, a region): not a source of
/// the data a class reads.
fn is_ours(n: &Node) -> bool {
    n.op == GROUP
        || n.op.starts_with("control.")
        || n.id == "members"
        || n.id == "resolve"
        || ["members_of_", "resolve_of_", "region_"].iter().any(|p| n.id.starts_with(p))
}

fn unique(nodes: &[Node], id: &str) -> String {
    let mut k = 1;
    let mut at = id.to_string();
    while nodes.iter().any(|n| n.id == at) {
        k += 1;
        at = format!("{id}_{k}");
    }
    at
}

/// Check a new class before anything is made: a name that is not taken (nor gives the layer name of another), a
/// mother that is a class of the scope.
fn check_new(nodes: &[Node], c: &NewClass) -> Result<(), String> {
    let name = c.name.trim();
    if name.is_empty() {
        return Err("the class has no name".into());
    }
    if name.starts_with(['$', '@']) || name.contains(['"', '\\']) {
        return Err(format!("the name {name:?}: no $, @ at the start, no quotes or backslashes"));
    }
    for n in groups(nodes) {
        let other = text(n, CLASS).unwrap_or_default();
        if other == name {
            return Err(format!("there is a class {name:?} already"));
        }
        if slug(other) == slug(name) {
            return Err(format!("{name:?} and {other:?} give the same layer name ({})", slug(name)));
        }
    }
    if let Some(p) = &c.parent
        && !groups(nodes).any(|n| text(n, CLASS) == Some(p.as_str()))
    {
        return Err(format!("the mother {p:?} is not a class of the tree"));
    }
    Ok(())
}

/// Make the branch of a class in the scope at `path` (its group with the recipe's body), and put it in the decision of
/// its siblings (made with the first of them). The whole edit is made on `m` (the caller keeps it as one step of undo,
/// putting `m` back to undo it); on an error `m` is left as it was. What could not be wired is told (no image found...).
pub fn add_class(m: &mut Model, path: &[String], r: &Registry, c: &NewClass) -> Result<Vec<String>, String> {
    let before = m.clone();
    let told = add_inner(m, path, r, c);
    if told.is_err() {
        *m = before;
    }
    told
}

fn add_inner(m: &mut Model, path: &[String], r: &Registry, c: &NewClass) -> Result<Vec<String>, String> {
    let name = c.name.trim().to_string();
    let nodes = m.scope(path).ok_or("the scope is not there")?.clone();
    check_new(&nodes, c)?;
    let mut told = Vec::new();
    // the mother's region (the number 1, every cell, for a root)
    let mother = c.parent.clone();
    let mut new_nodes: Vec<Node> = Vec::new();
    let region: Source = match &mother {
        None => Source::Literal(1.into()),
        Some(p) => {
            let id = region_id(p);
            if !nodes.iter().any(|n| n.id == id) {
                // (its siblings' decision: the mother's mother's)
                let grand = groups(&nodes)
                    .find(|n| text(n, CLASS) == Some(p.as_str()))
                    .and_then(|n| text(n, PARENT))
                    .map(String::from);
                let resolve = resolve_id(grand.as_deref());
                if !nodes.iter().any(|n| n.id == resolve) {
                    return Err(format!("the decision of the class {p:?}'s siblings ({resolve}) is not in the scope"));
                }
                let siblings = siblings_of(&nodes, grand.as_deref());
                new_nodes.push(
                    Node::new(&id, "raster.calc")
                        .input("won", &format!("{resolve}.class"))
                        .param("expression", region_expression(number_of(&siblings, p)))
                        .note(&format!("Where {p} won: the region its own classes are looked for in.")),
                );
            }
            Source::Ref(format!("{id}.region"))
        }
    };
    // the group
    let gid = unique(&nodes, &class_id(&name));
    let mut g = Node::new(&gid, GROUP).param(CLASS, name.clone()).param(COLOUR, hex(c.colour));
    if let Some(p) = &mother {
        g = g.param(PARENT, p.clone());
    }
    g.note = Some(c.recipe.about().to_string());
    g.inputs.insert("region".into(), region);
    for need in needs(c.recipe) {
        let found = match *need {
            "image" => source_of(m, &nodes, r, Type::Stack, "image"),
            "objects" => source_of(m, &nodes, r, Type::Objects, "objects"),
            "samples" => source_of(m, &nodes, r, Type::Vector, "samples"),
            // (the nDSM: a model input of the name, else a node, else an input to fill)
            _ => source_of(m, &nodes, r, Type::Map, "ndsm").filter(|f| f.contains("ndsm")),
        };
        match found {
            Some(f) => {
                g.inputs.insert((*need).into(), Source::Ref(f));
            }
            None if *need == "ndsm" => {
                if !m.inputs.contains_key("ndsm") {
                    m.inputs.insert(
                        "ndsm".into(),
                        Input::Map {
                            path: "ndsm.tif".into(),
                            band: None,
                            nodata: None,
                            note: Some("the height of the surface above the ground, for the buildings' recipe".into()),
                            data: None,
                        },
                    );
                    told.push("the model's input ndsm reads ndsm.tif: point it at your height model".into());
                }
                g.inputs.insert("ndsm".into(), Source::Ref("$ndsm".into()));
            }
            None => told.push(format!("no {need} found in the model: connect the input {need} of {gid}")),
        }
    }
    g.body = Some(body_of(c.recipe, &name));
    new_nodes.push(g);
    // the siblings' decision: the stack of their memberships, and the arbitration
    let (members, resolve) = (members_id(mother.as_deref()), resolve_id(mother.as_deref()));
    let layer = slug(&name);
    let at = format!("{gid}.membership");
    match nodes.iter().position(|n| n.id == members) {
        Some(_) => {}
        None => {
            if nodes.iter().any(|n| n.id == resolve) {
                return Err(format!("a node named {resolve} is in the way"));
            }
            new_nodes.push(Node::new(&members, "stack.of").note("A layer for each sibling class: its membership."));
            let mut rs = Node::new(&resolve, "objects.resolve")
                .input("memberships", &format!("{members}.stack"))
                .note("Decides, cell by cell, between the sibling classes: the greatest membership wins.");
            if let Some(p) = &mother {
                rs = rs.input("region", &format!("{}.region", region_id(p)));
            }
            new_nodes.push(rs);
        }
    }
    let scope = m.scope_mut(path).ok_or("the scope is not there")?;
    scope.extend(new_nodes);
    let stack = scope.iter_mut().find(|n| n.id == members).ok_or("the stack of memberships is not there")?;
    if stack.op != "stack.of" {
        return Err(format!("the node {members} is not a stack.of"));
    }
    stack.inputs.insert(layer, Source::Ref(at));
    // the numbers of the siblings' regions: the new class may come before them
    let siblings = siblings_of(scope, mother.as_deref());
    for s in &siblings {
        let id = region_id(s);
        let k = number_of(&siblings, s);
        if let Some(n) = scope.iter_mut().find(|n| n.id == id && n.op == "raster.calc") {
            n.params.insert("expression".into(), region_expression(k).into());
        }
    }
    Ok(told)
}

/// The names of the classes of one mother (the roots: none).
fn siblings_of(nodes: &[Node], mother: Option<&str>) -> Vec<String> {
    groups(nodes).filter(|n| text(n, PARENT) == mother).filter_map(|n| text(n, CLASS)).map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use terraflux_exec::{Engine, Quiet};
    use terraflux_graph::{Severity, check};
    use terraflux_ops::Map;

    use super::*;

    fn registry() -> Registry {
        Registry::standard()
    }

    /// The scene: a classes map, its image (bands blue to swir2), a height model and the segments of the classes.
    fn scene() -> Model {
        let mut m = Model::new("classes");
        m.nodes.push(Node::new("land", "nlm.fbm").param("rows", 40).param("columns", 50).param("seed", 5));
        m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
        m.nodes.push(Node::new("img", "nlm.image").input("classes", "cls.classes"));
        // (heights of 0 to 8 m)
        m.nodes.push(Node::new("heights", "nlm.random").param("rows", 40).param("columns", 50).param("seed", 9));
        m.nodes.push(Node::new("ndsm", "raster.calc").input("x", "heights.map").param("expression", "x * 8"));
        m.nodes.push(Node::new("seg", "segment.from_map").input("map", "cls.classes"));
        m
    }

    fn new(name: &str, parent: Option<&str>, recipe: Recipe) -> NewClass {
        NewClass { name: name.into(), colour: [170, 51, 51], parent: parent.map(String::from), recipe }
    }

    fn errors(m: &Model) -> Vec<String> {
        check(m, &registry()).iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_string()).collect()
    }

    fn tree_model() -> Model {
        let r = registry();
        let mut m = scene();
        for c in [
            new("Urban", None, Recipe::Water),
            new("Buildings", Some("Urban"), Recipe::Buildings),
            new("Roads", Some("Urban"), Recipe::Buildings),
            new("Vegetation", None, Recipe::Vegetation),
            new("Water", None, Recipe::Water),
        ] {
            let told = add_class(&mut m, &[], &r, &c).unwrap_or_else(|e| panic!("{}: {e}", c.name));
            assert!(told.is_empty(), "{}: {told:?}", c.name);
        }
        m
    }

    #[test]
    fn a_model_with_two_levels_is_read_back_as_the_tree() {
        let m = tree_model();
        assert_eq!(errors(&m), Vec::<String>::new());
        let t = read(&m.nodes);
        let seen: Vec<(&str, usize, Option<&str>)> =
            t.iter().map(|e| (e.name.as_str(), e.depth, e.parent.map(|p| t[p].name.as_str()))).collect();
        assert_eq!(
            seen,
            [
                ("Urban", 0, None),
                ("Buildings", 1, Some("Urban")),
                ("Roads", 1, Some("Urban")),
                ("Vegetation", 0, None),
                ("Water", 0, None)
            ]
        );
        assert_eq!(t[0].colour, Some([170, 51, 51]));
        assert_eq!(t[0].method, "by NDWI");
        assert_eq!(t[1].method, "by height (nDSM)");
        assert_eq!(t[3].method, "by NDVI");
        assert_eq!(t[3].rule.as_deref(), Some("clamp((ndvi - 0.2) / 0.3, 0, 1) * region"));
        // the group's settings are the convention
        let g = m.node("class_Buildings").unwrap();
        assert_eq!(g.op, GROUP);
        assert_eq!(g.params["class"], "Buildings");
        assert_eq!(g.params["parent_class"], "Urban");
        assert_eq!(g.params["colour"], "#aa3333");
        // the region of the children is the mother's win, the roots' is every cell
        assert_eq!(g.inputs["region"], Source::Ref("region_Urban.region".into()));
        assert_eq!(m.node("class_Urban").unwrap().inputs["region"], Source::Literal(1.into()));
        // one decision for the three roots, one for the two children
        let roots = m.node("members").unwrap();
        assert_eq!(roots.inputs.keys().collect::<Vec<_>>(), ["Urban", "Vegetation", "Water"]);
        assert_eq!(m.node("members_of_Urban").unwrap().inputs.keys().collect::<Vec<_>>(), ["Buildings", "Roads"]);
        assert_eq!(m.node("resolve_of_Urban").unwrap().inputs["region"], Source::Ref("region_Urban.region".into()));
        // Urban is the first of the roots' layers (by name), Vegetation the second
        assert_eq!(m.node("region_Urban").unwrap().params["expression"], "out region = 1 if won == 1 else None");
        // a model that uses objects.rules instead is read as its hierarchy
        let mut rules = scene();
        rules.nodes.push(Node::new("rules", "objects.rules").param(
            "classes",
            serde_json::json!([
                { "name": "Wet", "rule": "above(ndwi, 0, 0.2)" },
                { "name": "Lake", "parent": "Wet", "rule": "above(ndwi, 0.2, 0.4)", "colour": "#3366cc" },
                { "name": "Dry" }
            ]),
        ));
        let t = read(&rules.nodes);
        assert_eq!(
            t.iter().map(|e| (e.name.as_str(), e.depth, e.rules)).collect::<Vec<_>>(),
            [("Wet", 0, true), ("Lake", 1, true), ("Dry", 0, true)]
        );
        assert_eq!(t[1].colour, Some([0x33, 0x66, 0xcc]));
        assert_eq!(t[1].parent, Some(0));
        assert_eq!(t[2].method, "as its mother");
        assert_eq!(t[0].rule.as_deref(), Some("above(ndwi, 0, 0.2)"));
    }

    #[test]
    fn adding_a_class_is_one_step_that_undo_takes_back() {
        let r = registry();
        let mut m = scene();
        add_class(&mut m, &[], &r, &new("Water", None, Recipe::Water)).unwrap();
        let mut history = crate::undo::History::new(&m);
        let before = m.clone();
        history.name("added the class Urban");
        // (the class and its first child: what Add a class... does twice before the step ends)
        add_class(&mut m, &[], &r, &new("Urban", None, Recipe::Water)).unwrap();
        add_class(&mut m, &[], &r, &new("Buildings", Some("Urban"), Recipe::Buildings)).unwrap();
        history.changed(&m);
        history.tick(&m, false);
        assert_eq!(m.node("members").unwrap().inputs.len(), 2);
        assert!(m.node("region_Urban").is_some() && m.node("resolve_of_Urban").is_some());
        assert_eq!(history.steps().0, ["added the class Urban"]);
        assert!(history.undo(&mut m));
        assert_eq!(m, before, "undo restores the model");
        assert_eq!(read(&m.nodes).len(), 1);
        assert!(history.redo(&mut m));
        assert_eq!(read(&m.nodes).len(), 3);
        // a refused class leaves the model as it was
        let kept = m.clone();
        for (c, why) in [
            (new("Urban", None, Recipe::Water), "already"),
            (new(" ", None, Recipe::Water), "no name"),
            (new("Roads", Some("Nowhere"), Recipe::Water), "not a class"),
            (new("$x", None, Recipe::Water), "$"),
        ] {
            let e = add_class(&mut m, &[], &r, &c).unwrap_err();
            assert!(e.contains(why), "{e}");
            assert_eq!(m, kept, "{why}");
        }
        // two names that are one layer's name
        add_class(&mut m, &[], &r, &new("Wa ter", None, Recipe::Water)).unwrap();
        assert!(m.node("class_Wa_ter").is_some());
        let e = add_class(&mut m, &[], &r, &new("Wa_ter", None, Recipe::Water)).unwrap_err();
        assert!(e.contains("layer name"), "{e}");
        // a new sibling that comes first by name renumbers the regions of the others
        add_class(&mut m, &[], &r, &new("Forest", Some("Water"), Recipe::Vegetation)).unwrap();
        assert_eq!(m.node("region_Water").unwrap().params["expression"], "out region = 1 if won == 3 else None");
        add_class(&mut m, &[], &r, &new("Air", None, Recipe::Water)).unwrap();
        assert_eq!(m.node("region_Water").unwrap().params["expression"], "out region = 1 if won == 4 else None");
        assert_eq!(errors(&m), Vec::<String>::new());
    }

    /// A model run, its outputs.
    fn run(m: &Model) -> BTreeMap<String, Outputs> {
        let e = Engine { registry: registry() };
        e.run(m, &BTreeMap::new(), &mut Quiet).unwrap_or_else(|e| panic!("{e}")).values
    }

    fn map(o: &BTreeMap<String, Outputs>, node: &str, port: &str) -> Map {
        let Some(Value::Map(m)) = o.get(node).and_then(|n| n.get(port)) else { panic!("{node}.{port} is no map") };
        m.clone()
    }

    #[test]
    fn a_child_finds_nothing_outside_its_mother() {
        let r = registry();
        let mut m = scene();
        add_class(&mut m, &[], &r, &new("Wet", None, Recipe::Water)).unwrap();
        // the child looks for water too: inside the mother's region it has the mother's membership, outside none
        add_class(&mut m, &[], &r, &new("Deep", Some("Wet"), Recipe::Water)).unwrap();
        add_class(&mut m, &[], &r, &new("Green", None, Recipe::Vegetation)).unwrap();
        assert_eq!(errors(&m), Vec::<String>::new());
        let out = run(&m);
        let region = map(&out, "region_Wet", "region");
        let wet = map(&out, "class_Wet", "membership");
        let deep = map(&out, "class_Deep", "membership");
        let inside = region.data.iter().filter(|v| !v.is_nan()).count();
        assert!(inside > 0 && inside < region.data.len(), "the mother's region has {inside} cells");
        let mut kept = 0;
        for i in 0..region.data.len() {
            if region.data[i].is_nan() {
                assert!(deep.data[i].is_nan() || deep.data[i] == 0.0, "cell {i}: {} outside the mother", deep.data[i]);
            } else {
                // inside: the mother's own membership (it won there, so it is at least the least that wins)
                assert!(deep.data[i] > 0.0, "cell {i}");
                assert_eq!(deep.data[i], wet.data[i]);
                kept += 1;
            }
        }
        assert_eq!(kept, inside);
        // what the mother won is where it was the greatest of the roots, not more
        for i in 0..region.data.len() {
            assert!(!region.data[i].is_nan() || wet.data[i].is_nan() || wet.data[i] >= 0.0);
        }
        // the areas found, as the view shows them
        let mut t = read(&m.nodes);
        with_results(&mut t, &m.nodes, &out);
        let area = |n: &str| t.iter().find(|e| e.name == n).unwrap().area.unwrap();
        assert!(area("Wet") > 0.0);
        assert!(area("Deep") > 0.0 && area("Deep") <= area("Wet") + 1e-9);
        assert!(
            (area("Deep") - area("Wet")).abs() < 1e-9,
            "the child wins all the mother's cells it has a membership in"
        );
    }

    /// The samples as a table of points (one per object of the segments, at its first cell) of this scene's classes.
    fn with_samples(m: &mut Model) {
        let out = run(m);
        let truth = map(&out, "cls", "classes");
        let Some(Value::Objects(o)) = out.get("seg").and_then(|n| n.get("objects")) else { panic!("no objects") };
        let g = truth.grid.clone();
        let mut seen = vec![false; o.count];
        let mut rows = Vec::new();
        for (i, &l) in o.labels.iter().enumerate() {
            let l = l as usize;
            if l == 0 || seen[l - 1] || truth.data[i].is_nan() {
                continue;
            }
            seen[l - 1] = true;
            let (row, col) = (i / g.width, i % g.width);
            let x = g.transform[0] + (col as f64 + 0.5) * g.transform[1];
            let y = g.transform[3] + (row as f64 + 0.5) * g.transform[5];
            rows.push(serde_json::json!([x, y, if truth.data[i] == 1.0 { "Water" } else { "Other" }]));
        }
        m.nodes.push(Node::new("samples_table", "table.of").param("columns", ["x", "y", "class"]).param("rows", rows));
        m.nodes.push(Node::new("samples", "vector.from_table").input("table", "samples_table.table"));
    }

    #[test]
    fn each_recipe_passes_the_check_and_runs() {
        let r = registry();
        let mut base = scene();
        with_samples(&mut base);
        for recipe in Recipe::ALL {
            let mut m = base.clone();
            let told = add_class(&mut m, &[], &r, &new("Water", None, recipe)).unwrap();
            assert!(told.is_empty(), "{recipe:?}: {told:?}");
            assert_eq!(errors(&m), Vec::<String>::new(), "{recipe:?}");
            let out = run(&m);
            let mem = map(&out, "class_Water", "membership");
            assert!(mem.data.iter().all(|v| v.is_nan() || (0.0..=1.0).contains(v)), "{recipe:?}");
            assert!(mem.data.iter().any(|&v| v > 0.0), "{recipe:?}: no cell has a membership");
            let Some(Value::Table(t)) = out.get("resolve").and_then(|n| n.get("table")) else { panic!("resolve") };
            assert!(!t.rows.is_empty(), "{recipe:?}: the class won nothing");
            // the samples' count and the area, as the view shows them
            let mut tree = read(&m.nodes);
            with_results(&mut tree, &m.nodes, &out);
            assert_eq!(tree[0].method, if recipe == Recipe::Samples { "by samples" } else { tree[0].method.as_str() });
            assert!(tree[0].area.unwrap() > 0.0, "{recipe:?}");
            assert_eq!(tree[0].samples.is_some(), recipe == Recipe::Samples, "{recipe:?}");
        }
        // the height recipe with no nDSM in the model adds the input to fill
        let mut bare = Model::new("bare");
        bare.nodes.push(Node::new("land", "nlm.random").param("rows", 5).param("columns", 5));
        let told = add_class(&mut bare, &[], &r, &new("Buildings", None, Recipe::Buildings)).unwrap();
        assert!(bare.inputs.contains_key("ndsm") && told.iter().any(|t| t.contains("ndsm")), "{told:?}");
        // and one with no image says so
        let told = add_class(&mut bare, &[], &r, &new("Water", None, Recipe::Water)).unwrap();
        assert!(told.iter().any(|t| t.contains("no image")), "{told:?}");
    }
}
