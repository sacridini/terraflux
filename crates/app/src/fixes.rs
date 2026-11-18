//! The problems with a safe correction, a click away in the Problems pane: maps of a node on different grids (a
//! `raster.match` put before the one that does not fit), a map read without its no-data value (the value set), an
//! input a node needs and has not (a file read, or a model's input connected). [`fix_for`] says what puts a problem
//! right, [`apply`] makes it on a copy of the model (refused when it does not apply or would bring an error), and
//! [`run`] makes it in the session as one step of undo. A problem without a safe correction has none.

use terraflux_exec::lint::{Finding, Level};
use terraflux_graph::{Input, Issue, Model, Node, Ref, Severity, Source, Type, check};
use terraflux_ops::Registry;

use crate::app::Session;
use crate::diagnose::Diagnosis;
use crate::editor;

pub type Result<T> = std::result::Result<T, String>;

/// A problem the pane shows.
pub enum Problem<'a> {
    /// The last run's failure, explained.
    Failure(&'a Diagnosis),
    /// A node not filled in (an input it needs not connected).
    Unfilled(&'a Issue),
    /// A finding in the data.
    Finding(&'a Finding),
}

/// What a no-data value is set on.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A model's input (`$name`).
    Input(String),
    /// A raster.read node, in a scope.
    Node { path: Vec<String>, id: String },
}

/// What a correction still asks of the person before it can be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Nothing,
    /// How to resample: near (classes) or bilinear (a continuous variable).
    Resampling,
    /// The no-data value.
    Number,
    /// A file to read.
    File,
}

/// A correction of a problem.
#[derive(Debug, Clone, PartialEq)]
pub enum Fix {
    /// A raster.match between `from` and the port that read it, on the grid of `like`.
    Match {
        path: Vec<String>,
        node: String,
        port: String,
        from: String,
        like: String,
        resampling: Option<&'static str>,
    },
    /// A no-data value set where there was none.
    Nodata { target: Target, value: Option<f64> },
    /// A node that reads a file, connected to an input left empty.
    Read { path: Vec<String>, node: String, port: String, op: &'static str, file: Option<String> },
    /// A model's input connected to an input left empty.
    Link { path: Vec<String>, node: String, port: String, to: String },
}

impl Fix {
    /// What is still to be asked.
    pub fn need(&self) -> Need {
        match self {
            Fix::Match { resampling: None, .. } => Need::Resampling,
            Fix::Nodata { value: None, .. } => Need::Number,
            Fix::Read { file: None, .. } => Need::File,
            _ => Need::Nothing,
        }
    }

    /// The correction with how to resample given.
    pub fn with_resampling(mut self, how: &'static str) -> Fix {
        if let Fix::Match { resampling, .. } = &mut self {
            *resampling = Some(how);
        }
        self
    }

    /// The correction with the no-data value given.
    pub fn with_number(mut self, v: f64) -> Fix {
        if let Fix::Nodata { value, .. } = &mut self {
            *value = Some(v);
        }
        self
    }

    /// The correction with the file given.
    pub fn with_file(mut self, f: &str) -> Fix {
        if let Fix::Read { file, .. } = &mut self {
            *file = Some(f.to_string());
        }
        self
    }

    /// Exactly what it changes, in words (the button's hint).
    pub fn words(&self) -> String {
        match self {
            Fix::Match { node, port, from, like, resampling, .. } => {
                let how = match resampling {
                    Some("near") => ", nearest neighbour: the values are classes",
                    Some(_) => ", bilinear: the values are continuous",
                    None => ", the method asked (near for classes, bilinear for a continuous variable)",
                };
                format!(
                    "adds a raster.match node between {from} and {node}.{port}: {from} put on the grid of {like}{how}"
                )
            }
            Fix::Nodata { target, value } => {
                let on = match target {
                    Target::Input(n) => format!("the input ${n}"),
                    Target::Node { id, .. } => format!("the node {id}"),
                };
                match value {
                    Some(v) => format!("sets the no-data value of {on} to {v}"),
                    None => format!("sets the no-data value of {on} to the number typed"),
                }
            }
            Fix::Read { node, port, op, file, .. } => match file {
                Some(f) => format!("adds a {op} node on {f} and connects it to {node}.{port}"),
                None => format!("asks for a file, adds a {op} node on it and connects it to {node}.{port}"),
            },
            Fix::Link { node, port, to, .. } => format!("connects {to} to {node}.{port}"),
        }
    }

    /// The name of its step of undo.
    pub fn name(&self) -> String {
        match self {
            Fix::Match { node, port, like, .. } => format!("Fix {node}.{port}: put it on the grid of {like}"),
            Fix::Nodata { target: Target::Input(n), .. } => format!("Fix: set the no-data value of ${n}"),
            Fix::Nodata { target: Target::Node { id, .. }, .. } => format!("Fix: set the no-data value of {id}"),
            Fix::Read { node, port, .. } => format!("Fix {node}.{port}: read a file"),
            Fix::Link { node, port, to, .. } => format!("Fix {node}.{port}: connect {to}"),
        }
    }
}

/// The corrections of a problem (none when it has no safe one): one per input that does not fit, one per way to
/// fill an input left empty.
pub fn fix_for(m: &Model, r: &Registry, p: &Problem) -> Vec<Fix> {
    match p {
        Problem::Failure(d) => of_failure(m, d),
        Problem::Unfilled(i) => of_unfilled(m, r, i),
        Problem::Finding(f) => of_finding(m, f).into_iter().collect(),
    }
}

/// The scope and the id of a node by its path (`loop/node`).
fn split(at: &str) -> (Vec<String>, String) {
    let mut path: Vec<String> = at.split('/').map(String::from).collect();
    let id = path.pop().unwrap_or_default();
    (path, id)
}

fn of_failure(m: &Model, d: &Diagnosis) -> Vec<Fix> {
    let (path, id) = split(&d.at);
    let Some(n) = m.scope(&path).and_then(|s| s.iter().find(|n| n.id == id)) else { return Vec::new() };
    let mut out = Vec::new();
    // maps that are not on one grid: each that differs from the first goes through a raster.match onto the first
    let reads = |port: &str| match n.inputs.get(port) {
        Some(Source::Ref(r)) => Some(r.clone()),
        _ => None,
    };
    let maps: Vec<_> = d
        .inputs
        .iter()
        .filter(|s| s.grid.is_some() && reads(&s.port).is_some_and(|r| r == s.from && Ref::parse(&r).is_ok()))
        .collect();
    if let Some(first) = maps.first() {
        let g = first.grid.as_ref().unwrap();
        for s in &maps[1..] {
            let x = s.grid.as_ref().unwrap();
            let same = x.crs == g.crs
                && (x.width, x.height) == (g.width, g.height)
                && x.cell == g.cell
                && x.origin == g.origin;
            if !same && s.from != first.from {
                let resampling = s.classes.map(|c| if c { "near" } else { "bilinear" });
                out.push(Fix::Match {
                    path: path.clone(),
                    node: id.clone(),
                    port: s.port.clone(),
                    from: s.from.clone(),
                    like: first.from.clone(),
                    resampling,
                });
            }
        }
    }
    // a raster read whose no-data value is what the failure is about
    let says = d.message.to_lowercase();
    if n.op == "raster.read"
        && !n.params.contains_key("nodata")
        && ["no-data", "nodata", "no data"].iter().any(|w| says.contains(w))
    {
        out.push(Fix::Nodata { target: Target::Node { path, id }, value: None });
    }
    out
}

fn of_unfilled(m: &Model, r: &Registry, i: &Issue) -> Vec<Fix> {
    if !i.unfilled {
        return Vec::new();
    }
    // `op needs the input "port" (what it is)`
    let Some(port) = i.message.split('"').nth(1) else { return Vec::new() };
    let (path, id) = split(&i.at);
    let Some(n) = m.scope(&path).and_then(|s| s.iter().find(|n| n.id == id)) else { return Vec::new() };
    if n.inputs.contains_key(port) || n.uses.is_some() {
        return Vec::new();
    }
    let Some(p) = r.sig(&n.op).and_then(|s| s.inputs.iter().find(|p| p.name == port).cloned()) else {
        return Vec::new();
    };
    let mut out: Vec<Fix> = editor::sources(m, r, &path)
        .into_iter()
        .filter(|(k, t)| k.starts_with('$') && *t != Type::Any && t.fits(p.ty))
        .take(3)
        .map(|(to, _)| Fix::Link { path: path.clone(), node: id.clone(), port: port.to_string(), to })
        .collect();
    let op = match p.ty {
        Type::Raster | Type::Map | Type::Stack => Some("raster.read"),
        Type::Vector => Some("vector.read"),
        Type::Table => Some("table.read"),
        _ => None,
    };
    if let Some(op) = op {
        out.push(Fix::Read { path, node: id, port: port.to_string(), op, file: None });
    }
    out
}

fn of_finding(m: &Model, f: &Finding) -> Option<Fix> {
    if f.level != Level::Error || !f.fix.as_deref().is_some_and(|x| x.contains("no-data value")) {
        return None;
    }
    let name = f.at.strip_prefix('$')?.split('/').next()?;
    match m.inputs.get(name)? {
        Input::Map { nodata: None, .. } | Input::Stack { nodata: None, .. } => {
            Some(Fix::Nodata { target: Target::Input(name.to_string()), value: None })
        }
        _ => None,
    }
}

fn node_mut<'a>(m: &'a mut Model, path: &[String], id: &str) -> Result<&'a mut Node> {
    let scope = m.scope_mut(path).ok_or_else(|| format!("no container {}", path.join("/")))?;
    scope.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("no node {id:?}"))
}

fn errors(m: &Model, r: &Registry) -> Vec<String> {
    check(m, r).into_iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_string()).collect()
}

/// The model after a correction, unless it brings an error the model did not have.
fn checked(before: &Model, after: Model, r: &Registry) -> Result<Model> {
    let old = errors(before, r);
    match errors(&after, r).into_iter().find(|e| !old.contains(e)) {
        Some(e) => Err(e),
        None => Ok(after),
    }
}

/// A number as a setting: whole when it is.
fn number(v: f64) -> serde_json::Value {
    if v.fract() == 0.0 && v.abs() < 1e15 { serde_json::json!(v as i64) } else { serde_json::json!(v) }
}

/// Where a node added for a node goes: to its left.
fn beside(n: &Node, down: f32) -> [f32; 2] {
    n.pos.map_or([0.0, 0.0], |p| [p[0] - 260.0, p[1] + down])
}

/// The model with a correction made, as a copy: the model given is not changed. Refused (with the reason) when the
/// correction asks for more (a resampling, a number, a file), does not apply any more (the input was changed since)
/// or would bring an error.
pub fn apply(m: &Model, r: &Registry, fix: &Fix) -> Result<Model> {
    let mut c = m.clone();
    match fix {
        Fix::Match { path, node, port, from, like, resampling } => {
            let how = resampling.ok_or("say how to resample: near for classes, bilinear for a continuous variable")?;
            let t = node_mut(&mut c, path, node)?;
            if t.inputs.get(port) != Some(&Source::Ref(from.clone())) {
                return Err(format!("{node}.{port} does not read {from} any more"));
            }
            let pos = beside(t, 140.0);
            let id = editor::add_node_named(&mut c, r, path, "raster.match", None, pos)?;
            let n = node_mut(&mut c, path, &id)?;
            n.inputs.insert("raster".into(), Source::Ref(from.clone()));
            n.inputs.insert("like".into(), Source::Ref(like.clone()));
            n.params.insert("resampling".into(), how.into());
            let t = node_mut(&mut c, path, node)?;
            t.inputs.insert(port.clone(), Source::Ref(format!("{id}.raster")));
            t.wires.remove(port);
        }
        Fix::Nodata { target, value } => {
            let v = value.ok_or("give the no-data value")?;
            if !v.is_finite() {
                return Err("the no-data value is a number".into());
            }
            match target {
                Target::Input(name) => match c.inputs.get_mut(name) {
                    Some(Input::Map { nodata, .. } | Input::Stack { nodata, .. }) if nodata.is_none() => {
                        *nodata = Some(v);
                    }
                    Some(Input::Map { .. } | Input::Stack { .. }) => {
                        return Err(format!("${name} has a no-data value"));
                    }
                    _ => return Err(format!("${name} is not a map")),
                },
                Target::Node { path, id } => {
                    let n = node_mut(&mut c, path, id)?;
                    if n.op != "raster.read" || n.params.contains_key("nodata") {
                        return Err(format!("{id} is not a raster.read without a no-data value"));
                    }
                    n.params.insert("nodata".into(), number(v));
                }
            }
        }
        Fix::Read { path, node, port, op, file } => {
            let file = file.as_deref().ok_or("choose the file to read")?;
            let t = node_mut(&mut c, path, node)?;
            if t.inputs.contains_key(port) {
                return Err(format!("{node}.{port} is connected already"));
            }
            let pos = beside(t, 0.0);
            let name = editor::id_of_file(file);
            let id = editor::add_node_named(&mut c, r, path, op, name.as_deref(), pos)?;
            node_mut(&mut c, path, &id)?.params.insert("path".into(), file.into());
            let out = op.trim_end_matches(".read").rsplit('.').next().unwrap_or("raster");
            let t = node_mut(&mut c, path, node)?;
            t.inputs.insert(port.clone(), Source::Ref(format!("{id}.{out}")));
            t.wires.remove(port);
        }
        Fix::Link { path, node, port, to } => {
            Ref::parse(to).map_err(|e| e.to_string())?;
            let t = node_mut(&mut c, path, node)?;
            if t.inputs.contains_key(port) {
                return Err(format!("{node}.{port} is connected already"));
            }
            t.inputs.insert(port.clone(), Source::Ref(to.clone()));
        }
    }
    checked(m, c, r)
}

/// Make a correction in the session, as one step of undo; the checks follow. The reason in the editor's message
/// when it is refused.
pub fn run(s: &mut Session, r: &Registry, fix: &Fix) -> bool {
    match apply(&s.source, r, fix) {
        Ok(m) => {
            s.source = m;
            s.history.name(&fix.name());
            s.edited();
            s.editor.message = None;
            true
        }
        Err(e) => {
            s.editor.message = Some(e);
            false
        }
    }
}

/// The dialog that picks the file for a read operator, with the filters of its kind.
pub fn pick_file(op: &str) -> Option<String> {
    let d = rfd::FileDialog::new().set_title("The file to read");
    match op {
        "vector.read" => d.add_filter("Vectors", &crate::builder::VECTOR_EXTENSIONS),
        "table.read" => d.add_filter("Tables", &["csv", "tsv", "txt", "json"]),
        _ => d.add_filter("Rasters", &terraflux_ops::files::RASTER_EXTENSIONS),
    }
    .add_filter("Every file", &["*"])
    .pick_file()
    .map(|p| p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnose::{GridInfo, Side};

    fn grid(w: usize, h: usize) -> GridInfo {
        GridInfo { width: w, height: h, cell: (1.0, 1.0), crs: "EPSG:4326".into(), origin: (0.0, 0.0) }
    }

    fn side(port: &str, from: &str, g: GridInfo, classes: Option<bool>) -> Side {
        Side {
            port: port.into(),
            from: from.into(),
            node: from.split('.').next().map(String::from),
            kind: "a map".into(),
            grid: Some(g),
            classes,
        }
    }

    fn maps() -> Model {
        Model::from_toml(
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
op = "raster.calc"
inputs = { a = "small.map", b = "big.map" }
params = { expression = "a + b" }
"#,
        )
        .unwrap()
    }

    fn failure(classes: Option<bool>) -> Diagnosis {
        Diagnosis {
            at: "sum".into(),
            op: "raster.calc".into(),
            message: "different grids".into(),
            inputs: vec![side("a", "small.map", grid(10, 10), classes), side("b", "big.map", grid(10, 20), classes)],
            hints: Vec::new(),
        }
    }

    #[test]
    fn maps_of_different_grids_go_through_a_raster_match() {
        let r = Registry::standard();
        let m = maps();
        let fixes = fix_for(&m, &r, &Problem::Failure(&failure(Some(false))));
        assert_eq!(fixes.len(), 1, "{fixes:?}");
        let Fix::Match { node, port, from, like, resampling, .. } = &fixes[0] else { panic!("{fixes:?}") };
        assert_eq!((node.as_str(), port.as_str(), from.as_str(), like.as_str()), ("sum", "b", "big.map", "small.map"));
        assert_eq!(*resampling, Some("bilinear"));
        assert_eq!(fixes[0].need(), Need::Nothing);
        assert!(fixes[0].words().contains("raster.match") && fixes[0].words().contains("small.map"));
        let after = apply(&m, &r, &fixes[0]).unwrap();
        let mt = after.node("raster_match").expect("the node added");
        assert_eq!(mt.op, "raster.match");
        assert_eq!(mt.inputs["raster"], Source::Ref("big.map".into()));
        assert_eq!(mt.inputs["like"], Source::Ref("small.map".into()));
        assert_eq!(mt.params["resampling"], "bilinear");
        assert_eq!(after.node("sum").unwrap().inputs["b"], Source::Ref("raster_match.raster".into()));
        assert_eq!(after.node("sum").unwrap().inputs["a"], Source::Ref("small.map".into()));
        // the model given is as it was, and no new error
        assert_eq!(m, maps());
        assert!(errors(&after, &r).is_empty(), "{:?}", errors(&after, &r));
        // classes: nearest neighbour
        let f = fix_for(&m, &r, &Problem::Failure(&failure(Some(true))));
        assert!(matches!(&f[0], Fix::Match { resampling: Some("near"), .. }));
    }

    #[test]
    fn a_match_asks_how_to_resample_when_the_type_is_not_known() {
        let r = Registry::standard();
        let m = maps();
        let f = fix_for(&m, &r, &Problem::Failure(&failure(None))).remove(0);
        assert_eq!(f.need(), Need::Resampling);
        assert!(apply(&m, &r, &f).is_err());
        let after = apply(&m, &r, &f.with_resampling("near")).unwrap();
        assert_eq!(after.node("raster_match").unwrap().params["resampling"], "near");
    }

    #[test]
    fn a_match_is_refused_when_it_does_not_apply() {
        let r = Registry::standard();
        let m = maps();
        // maps on one grid: nothing to fix
        let mut d = failure(Some(false));
        d.inputs[1].grid = Some(grid(10, 10));
        assert!(fix_for(&m, &r, &Problem::Failure(&d)).is_empty());
        // the input was changed since: refused
        let f = fix_for(&m, &r, &Problem::Failure(&failure(Some(false)))).remove(0);
        let after = apply(&m, &r, &f).unwrap();
        assert!(apply(&after, &r, &f).is_err());
        // a node that is not there
        let mut gone = failure(Some(false));
        gone.at = "nothing".into();
        assert!(fix_for(&m, &r, &Problem::Failure(&gone)).is_empty());
    }

    fn finding(at: &str) -> Finding {
        Finding {
            level: Level::Error,
            at: at.into(),
            message: "no cell has a class".into(),
            fix: Some("check its no-data value".into()),
            action: None,
        }
    }

    #[test]
    fn a_map_without_nodata_gets_the_value_typed() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        m.inputs.insert(
            "land".into(),
            Input::Map { path: "land.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        m.inputs.insert(
            "dem".into(),
            Input::Map { path: "dem.tif".into(), band: None, nodata: Some(-1.0), note: None, data: None },
        );
        let f = fix_for(&m, &r, &Problem::Finding(&finding("$land"))).remove(0);
        assert_eq!(f.need(), Need::Number);
        assert!(apply(&m, &r, &f).is_err());
        let after = apply(&m, &r, &f.clone().with_number(-9999.0)).unwrap();
        assert!(matches!(&after.inputs["land"], Input::Map { nodata: Some(v), .. } if *v == -9999.0));
        assert!(matches!(&m.inputs["land"], Input::Map { nodata: None, .. }));
        // a band of a stack names the input too; one that has a value, or none: no fix
        assert_eq!(fix_for(&m, &r, &Problem::Finding(&finding("$land/b1"))).len(), 1);
        assert!(fix_for(&m, &r, &Problem::Finding(&finding("$dem"))).is_empty());
        assert!(fix_for(&m, &r, &Problem::Finding(&finding("$other"))).is_empty());
        // set already, it is refused
        assert!(apply(&after, &r, &f.with_number(1.0)).is_err());
    }

    #[test]
    fn a_raster_read_without_nodata_gets_it_as_a_setting() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        m.nodes.push(Node::new("land", "raster.read").param("path", "land.tif"));
        let mut d = failure(None);
        d.at = "land".into();
        d.op = "raster.read".into();
        d.inputs.clear();
        d.message = "the file has no no-data value".into();
        let f = fix_for(&m, &r, &Problem::Failure(&d)).remove(0);
        let after = apply(&m, &r, &f.with_number(255.0)).unwrap();
        assert_eq!(after.node("land").unwrap().params["nodata"], 255);
        // another message: no fix
        d.message = "cannot open".into();
        assert!(fix_for(&m, &r, &Problem::Failure(&d)).is_empty());
    }

    fn unfilled(m: &Model, r: &Registry) -> Issue {
        check(m, r).into_iter().find(|i| i.unfilled).expect("a node not filled in")
    }

    #[test]
    fn an_input_left_empty_reads_a_file_or_a_model_input() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        m.inputs.insert(
            "land".into(),
            Input::Map { path: "land.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        m.nodes.push(Node::new("out", "raster.write").param("path", "out.tif"));
        let i = unfilled(&m, &r);
        let fixes = fix_for(&m, &r, &Problem::Unfilled(&i));
        assert_eq!(fixes.len(), 2, "{fixes:?}");
        // the model's input
        assert!(matches!(&fixes[0], Fix::Link { to, port, .. } if to == "$land" && port == "value"));
        let linked = apply(&m, &r, &fixes[0]).unwrap();
        assert_eq!(linked.node("out").unwrap().inputs["value"], Source::Ref("$land".into()));
        assert!(!check(&linked, &r).iter().any(|i| i.unfilled));
        // a file
        assert_eq!(fixes[1].need(), Need::File);
        assert!(apply(&m, &r, &fixes[1]).is_err());
        let read = apply(&m, &r, &fixes[1].clone().with_file("data/land use 1997.tif")).unwrap();
        let n = read.node("land_use_1997").expect("the reader");
        assert_eq!(n.op, "raster.read");
        assert_eq!(n.params["path"], "data/land use 1997.tif");
        assert_eq!(read.node("out").unwrap().inputs["value"], Source::Ref("land_use_1997.raster".into()));
        assert!(!check(&read, &r).iter().any(|i| i.unfilled));
        // connected since: refused
        assert!(apply(&linked, &r, &fixes[1].clone().with_file("x.tif")).is_err());
        // a vector or a table the port takes: their own readers
        let mut v = Model::new("t");
        v.nodes.push(Node::new("buf", "vector.buffer"));
        if let Some(i) = check(&v, &r).into_iter().find(|i| i.unfilled) {
            let f = fix_for(&v, &r, &Problem::Unfilled(&i));
            assert!(f.iter().all(|f| matches!(f, Fix::Read { op: "vector.read", .. })), "{f:?}");
        }
    }

    #[test]
    fn a_fix_is_one_step_of_undo() {
        let r = Registry::standard();
        let mut s = Session::blank();
        s.source = maps();
        s.edited();
        // (a frame ends a step)
        s.history.tick(&s.source, false);
        let before = s.source.clone();
        let f = fix_for(&s.source, &r, &Problem::Failure(&failure(Some(false)))).remove(0);
        assert!(run(&mut s, &r, &f));
        s.history.tick(&s.source, false);
        assert!(s.history.steps().0.iter().any(|n| n.starts_with("Fix sum.b")), "{:?}", s.history.steps());
        assert!(s.source.node("raster_match").is_some());
        assert!(s.model.node("raster_match").is_some(), "the checks follow");
        assert!(s.history.undo(&mut s.source));
        assert_eq!(s.source, before);
        // refused: the model as it was, the reason said
        assert!(!run(&mut s, &r, &Fix::Nodata { target: Target::Input("none".into()), value: Some(1.0) }));
        assert_eq!(s.source, before);
        assert!(s.editor.message.is_some());
    }
}
