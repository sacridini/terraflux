//! The user's own nodes in the application: the library (`nodes` in the user's application data), a group or a
//! calculator saved in it (its ports typed and the layers its inputs need found from what it is given and what its
//! scripts read, all of it shown to change before it is saved), its nodes offered beside the operators, and a node
//! of it in a model told when the library has a newer version, when the node was changed since, and when what it is
//! given lacks a layer it reads.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use egui::{RichText, Ui, Vec2};
use terraflux_graph::user_node::{Library, read_inputs};
use terraflux_graph::{GROUP, Node, Ref, Source, Type, UserInput, UserNode, UserOutput, Uses};
use terraflux_ops::{Registry, Value, algebra_tools};

use crate::app::Session;

/// Where the library is: `TERRAFLUX_NODES`, else `nodes` in the user's application data.
pub fn dir() -> PathBuf {
    std::env::var_os("TERRAFLUX_NODES").map(PathBuf::from).unwrap_or_else(|| crate::settings::data_dir().join("nodes"))
}

/// The user's own library.
pub fn library() -> Library {
    Library::new(dir())
}

/// A library of nodes: the user's own, or one shared with others (a folder of the settings').
#[derive(Debug, Clone)]
pub struct Shelf {
    pub lib: Library,
    /// What it is called: "My nodes", or its folder's name.
    pub name: String,
    pub mine: bool,
}

/// The libraries: the user's own first, then the shared ones of the settings.
pub fn libraries() -> Vec<Shelf> {
    let mut out = vec![Shelf { lib: library(), name: "My nodes".into(), mine: true }];
    for d in crate::settings::get().node_libraries {
        let name = d.file_name().map_or_else(|| d.display().to_string(), |n| n.to_string_lossy().into_owned());
        out.push(Shelf { lib: Library::new(d), name, mine: false });
    }
    out
}

/// The library a node's name is in (the first that has it: the user's own before the shared), else the user's.
pub fn library_of(name: &str) -> Shelf {
    let all = libraries();
    let at = all.iter().position(|l| !l.lib.versions(name).is_empty()).unwrap_or(0);
    all.into_iter().nth(at).expect("the user's library")
}

/// The last version of a node of the libraries (none: the last there is).
pub fn get(name: &str, version: Option<u32>) -> Result<UserNode, String> {
    library_of(name).lib.get(name, version).map_err(|e| e.to_string())
}

/// The nodes of the libraries as read, with the library each is in (none: to be read).
type Shelved = Mutex<Option<Vec<(usize, UserNode)>>>;

fn cache() -> &'static Shelved {
    static C: OnceLock<Shelved> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// The last version of each node of the libraries, with the library it is in (its place in [`libraries`]): a
/// name in two of them, the first's (read once, and again after [`changed`]).
pub fn shelved() -> Vec<(usize, UserNode)> {
    cache()
        .lock()
        .unwrap()
        .get_or_insert_with(|| {
            let mut out: Vec<(usize, UserNode)> = Vec::new();
            for (k, l) in libraries().iter().enumerate() {
                for u in l.lib.list() {
                    if !out.iter().any(|(_, v)| v.name == u.name) {
                        out.push((k, u));
                    }
                }
            }
            out
        })
        .clone()
}

/// The last version of each node of the libraries.
pub fn nodes() -> Vec<UserNode> {
    shelved().into_iter().map(|(_, u)| u).collect()
}

/// The libraries changed: read them again.
pub fn changed() {
    *cache().lock().unwrap() = None;
}

/// What the assistant and the quick search call a node of the libraries as an operator: `my:name`.
pub const PREFIX: &str = "my:";

/// The node of the libraries an operator's name is (`my:name`, `my:name@2`): its name and version.
pub fn named(op: &str) -> Option<(&str, Option<u32>)> {
    let rest = op.strip_prefix(PREFIX)?;
    Some(match rest.rsplit_once('@') {
        Some((name, v)) if v.parse::<u32>().is_ok() => (name, v.parse().ok()),
        _ => (rest, None),
    })
}

/// A node of the libraries in words, for the assistant: what it does, what it takes (its settings: the values it
/// has of its own) and gives.
pub fn describe(u: &UserNode) -> String {
    let mut s = format!("{PREFIX}{} (version {}): a node of the user's libraries", u.name, u.version);
    if let Some(d) = &u.description {
        s.push_str(&format!("\n{d}"));
    }
    let doc = |d: &Option<String>| d.as_ref().map_or(String::new(), |d| format!(": {d}"));
    for (title, settings) in [("Inputs", false), ("Settings", true)] {
        let ports: Vec<_> = u.inputs.iter().filter(|(_, i)| is_setting_type(i.ty) == settings).collect();
        if !ports.is_empty() {
            s.push_str(&format!("\n\n{title}:"));
            for (k, i) in ports {
                let need = if i.required() { "" } else { ", optional" };
                s.push_str(&format!("\n  {k} ({}{need}){}", i.summary(), doc(&i.doc)));
            }
        }
    }
    if !u.outputs.is_empty() {
        s.push_str("\n\nOutputs:");
        for (k, o) in &u.outputs {
            s.push_str(&format!("\n  {k} ({}){}", o.ty.name(), doc(&o.doc)));
        }
    }
    s.push_str(&format!(
        "\n\nAdd it with add_node, op = \"{PREFIX}{}\"; its inputs and settings are given as inputs (a value or \
         node.port).",
        u.name
    ));
    s
}

/// Whether an input of a node of the user's of this type is a setting (a value, set in the Node pane as an
/// operator's settings are) rather than data.
pub fn is_setting_type(ty: Type) -> bool {
    matches!(ty, Type::Number | Type::Integer | Type::Bool | Type::Text)
}

/// Whether an input of a node is a setting of a node of the user's: a value's, not fed by another node.
pub fn is_setting(n: &Node, port: &str, ty: Type) -> bool {
    n.op == GROUP && n.uses.is_some() && is_setting_type(ty) && !matches!(n.inputs.get(port), Some(Source::Ref(_)))
}

/// The types a port of a node of the user's is offered.
const TYPES: [Type; 11] = [
    Type::Map,
    Type::Stack,
    Type::Number,
    Type::Integer,
    Type::Bool,
    Type::Text,
    Type::Table,
    Type::Vector,
    Type::Series,
    Type::Raster,
    Type::Any,
];

/// The type of a value as a port takes it: rasters on disk a stack when they are many bands (none chosen), else a map.
fn kind(v: &Value) -> Type {
    match v {
        Value::Raster(r) if r.layers.len() > 1 && !r.picked => Type::Stack,
        Value::Raster(_) => Type::Map,
        v => v.ty(),
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

/// The layers the scripts of a body read of each of its inputs (`@img` given to a calculator as `s`, and `s.nir`
/// read: `nir` of `img`), the bodies of its containers too.
pub fn bands_read(nodes: &[Node]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in nodes {
        if n.op == "raster.calc"
            && let Some(script) = n.params.get("expression").and_then(|v| v.as_str())
        {
            let read = algebra_tools::layers_read(script);
            for (var, src) in &n.inputs {
                if let (Source::Ref(r), Some(layers)) = (src, read.get(var))
                    && let Some(name) = r.strip_prefix('@')
                {
                    let v = out.entry(name.to_string()).or_default();
                    v.extend(layers.iter().filter(|l| !v.contains(l)).cloned().collect::<Vec<_>>());
                }
            }
        }
        if let Some(b) = &n.body {
            for (k, v) in bands_read(&b.nodes) {
                let o = out.entry(k).or_default();
                o.extend(v.into_iter().filter(|l| !o.contains(l)).collect::<Vec<_>>());
            }
        }
    }
    out
}

/// What a group takes and gives, as a node of the user's: its inputs (those it is given, those its body reads),
/// typed by what they are given (`given`), a value its own default, the layers its scripts read (a stack then);
/// its outputs typed by what they gave (`made`). What a node of the library declared already is kept where it
/// still holds.
pub fn infer(g: &Node, given: &dyn Fn(&Source) -> Type, made: &dyn Fn(&str) -> Type) -> Uses {
    let body = g.body.clone().unwrap_or_default();
    let mut names: Vec<String> = g.inputs.keys().cloned().collect();
    names.extend(read_inputs(&body).into_iter().filter(|k| !g.inputs.contains_key(k)));
    let bands = bands_read(&body.nodes);
    let before = g.uses.as_ref();
    let mut inputs = BTreeMap::new();
    for k in names {
        let mut i = UserInput::new(Type::Any);
        i.bands = bands.get(&k).cloned().unwrap_or_default();
        match g.inputs.get(&k) {
            Some(Source::Literal(v)) => {
                i.ty = literal_type(v);
                i.default = Some(v.clone());
            }
            Some(src) => i.ty = given(src),
            None => {}
        }
        if !i.bands.is_empty() {
            i.ty = Type::Stack;
        }
        if let Some(b) = before.and_then(|u| u.inputs.get(&k)) {
            i.doc = b.doc.clone();
            if i.ty == Type::Any {
                i.ty = b.ty;
            }
        }
        inputs.insert(k, i);
    }
    let outputs = body
        .outputs
        .keys()
        .map(|k| {
            let mut o = UserOutput::new(made(k));
            if let Some(b) = before.and_then(|u| u.outputs.get(k)) {
                o.doc = b.doc.clone();
                if o.ty == Type::Any {
                    o.ty = b.ty;
                }
            }
            (k.clone(), o)
        })
        .collect();
    Uses {
        name: before.map_or_else(|| g.id.clone(), |u| u.name.clone()),
        version: before.map_or(0, |u| u.version),
        description: before.and_then(|u| u.description.clone()).or_else(|| g.note.clone()),
        inputs,
        outputs,
    }
}

/// The value a source of a scope gives, when it is known: from the last run, the model's inputs read.
fn known(s: &Session, path: &[String], src: &Source) -> Option<Value> {
    match src {
        Source::Literal(v) => Some(Value::from_json(v)),
        Source::Ref(r) => match Ref::parse(r).ok()? {
            // (the outputs of the nodes in a container are not kept)
            Ref::Port { node, port } if path.is_empty() => s.value_of(&node, &port),
            Ref::Input(name) => s.inputs.as_ref()?.get(&name).cloned(),
            _ => None,
        },
    }
}

/// The rasters read by their headers: by file and band, what they are (none: not read).
type Headers = Mutex<BTreeMap<(PathBuf, String), Option<Value>>>;

/// What rasters a node reads are, by their headers (read once): many bands (none chosen) a stack.
fn header(s: &Session, path: &[String], src: &Source) -> Option<Value> {
    static HEADERS: OnceLock<Headers> = OnceLock::new();
    let Source::Ref(r) = src else { return None };
    let file = match Ref::parse(r).ok()? {
        Ref::Port { node, .. } => {
            let n = s.source.scope(path)?.iter().find(|n| n.id == node)?;
            let p = n.params.get("path").and_then(|p| p.as_str()).filter(|_| n.op == "raster.read")?;
            let band = n.params.get("band").map(|b| b.as_str().map_or(b.to_string(), String::from));
            (s.base().join(p), band)
        }
        Ref::Input(name) => match s.source.inputs.get(&name)? {
            terraflux_graph::Input::Stack { path, .. } => (s.base().join(path), None),
            terraflux_graph::Input::Map { path, band, .. } => {
                (s.base().join(path), Some(band.clone().unwrap_or_default()))
            }
            terraflux_graph::Input::Value { .. } => return None,
        },
        Ref::Carried(_) => return None,
    };
    let key = (file.0.clone(), file.1.clone().unwrap_or_default());
    let mut h = HEADERS.get_or_init(|| Mutex::new(BTreeMap::new())).lock().unwrap();
    h.entry(key)
        .or_insert_with(|| {
            let band = file.1.as_deref().filter(|b| !b.is_empty());
            let r = terraflux_ops::files::Rasters::open(&file.0.to_string_lossy(), band, None).ok()?;
            Some(Value::Raster(Arc::new(r.picking(file.1.is_some()))))
        })
        .clone()
}

/// The type a source of a scope gives: of its value when known, else of what gives it.
fn type_given(s: &Session, r: &Registry, path: &[String], src: &Source) -> Type {
    if let Some(v) = known(s, path, src).or_else(|| header(s, path, src)) {
        return kind(&v);
    }
    let Source::Ref(text) = src else { return Type::Any };
    match Ref::parse(text) {
        Ok(Ref::Port { node, port }) => s
            .source
            .scope(path)
            .and_then(|v| v.iter().find(|n| n.id == node))
            .and_then(|n| crate::editor::output_ports(n, r).into_iter().find(|(p, _)| *p == port))
            .map_or(Type::Any, |(_, t)| if t == Type::Raster { Type::Map } else { t }),
        Ok(_) => crate::editor::sources(&s.source, r, path)
            .into_iter()
            .find(|(k, _)| k == text)
            .map_or(Type::Any, |(_, t)| t),
        Err(_) => Type::Any,
    }
}

/// Why what a node of the user's is given lacks layers it reads, by input (what is given known from the last run
/// or the headers of the rasters it reads).
pub fn lacking(s: &Session, path: &[String], n: &Node) -> Vec<(String, String)> {
    let Some(u) = &n.uses else { return Vec::new() };
    let mut out = Vec::new();
    for (k, i) in u.inputs.iter().filter(|(_, i)| !i.bands.is_empty()) {
        let Some(src) = n.inputs.get(k) else { continue };
        if let Some(v) = known(s, path, src).or_else(|| header(s, path, src))
            && let Some(why) = algebra_tools::lacks_layers(&v, &i.bands)
        {
            out.push((k.clone(), why));
        }
    }
    out
}

/// A port of the node being saved, as the dialog edits it.
#[derive(Debug, Clone, PartialEq)]
pub struct Port {
    pub name: String,
    pub ty: Type,
    /// The layers (an input's), one after another with commas.
    pub bands: String,
    /// Its value of its own (an input's; empty: none, it must be given).
    pub default: String,
    pub doc: String,
}

/// A group or a calculator being saved as a node of the user's (the dialog's state).
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub path: Vec<String>,
    pub id: String,
    /// A calculator, made a group of itself when saved.
    pub calc: bool,
    pub name: String,
    pub description: String,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    /// The folder of the library it is saved in.
    pub library: PathBuf,
    pub error: Option<String>,
}

impl Draft {
    /// What the node takes and gives, as the dialog has it.
    pub fn uses(&self) -> Result<Uses, String> {
        let mut inputs = BTreeMap::new();
        for p in &self.inputs {
            let bands: Vec<String> =
                p.bands.split([',', ' ']).map(str::trim).filter(|b| !b.is_empty()).map(String::from).collect();
            let default = (!p.default.trim().is_empty()).then(|| crate::editor::value_of(&p.default));
            if let Some(b) = bands.iter().find(|b| !b.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
                return Err(format!("{}: {b:?} is not a layer's name as a script reads it", p.name));
            }
            let doc = (!p.doc.trim().is_empty()).then(|| p.doc.trim().to_string());
            inputs.insert(p.name.clone(), UserInput { ty: p.ty, bands, default, doc });
        }
        let outputs = self
            .outputs
            .iter()
            .map(|p| {
                let doc = (!p.doc.trim().is_empty()).then(|| p.doc.trim().to_string());
                (p.name.clone(), UserOutput { ty: p.ty, doc })
            })
            .collect();
        let description = self.description.trim();
        Ok(Uses {
            name: self.name.trim().to_string(),
            version: 0,
            description: (!description.is_empty()).then(|| description.to_string()),
            inputs,
            outputs,
        })
    }
}

/// The group the node is or becomes (a calculator made a group of itself), in a copy of the model.
fn as_group(s: &Session, r: &Registry, path: &[String], id: &str) -> Result<(terraflux_graph::Model, Node), String> {
    let mut m = s.source.clone();
    let n = m.scope(path).and_then(|v| v.iter().find(|n| n.id == id)).cloned().ok_or(format!("no node {id:?}"))?;
    if n.op == "raster.calc" {
        crate::editor::calc_to_group(&mut m, r, path, id)?;
    } else if n.op != GROUP {
        return Err(format!("{id} is a {}: a group or a calculator (raster.calc) is saved as a node", n.op));
    }
    let g = m.scope(path).and_then(|v| v.iter().find(|n| n.id == id)).cloned().ok_or(format!("no node {id:?}"))?;
    Ok((m, g))
}

/// The dialog's first state for a node of a scope: what it takes and gives found ([`infer`]).
pub fn draft(s: &Session, r: &Registry, path: &[String], id: &str) -> Result<Draft, String> {
    let calc = s.source.scope(path).and_then(|v| v.iter().find(|n| n.id == id)).is_some_and(|n| n.op == "raster.calc");
    let (_, g) = as_group(s, r, path, id)?;
    let given = |src: &Source| type_given(s, r, path, src);
    // what it gave: the calculator's or the group's outputs in the last run
    let made = |port: &str| {
        let v = if path.is_empty() { s.value_of(id, port) } else { None };
        v.map_or(Type::Any, |v| kind(&v))
    };
    let u = infer(&g, &given, &made);
    let port =
        |name: &String, ty: Type, bands: &[String], default: Option<&serde_json::Value>, doc: &Option<String>| Port {
            name: name.clone(),
            ty,
            bands: bands.join(", "),
            default: default.map(|v| v.to_string()).unwrap_or_default(),
            doc: doc.clone().unwrap_or_default(),
        };
    Ok(Draft {
        path: path.to_vec(),
        id: id.to_string(),
        calc,
        name: u.name.clone(),
        description: u.description.clone().unwrap_or_default(),
        inputs: u.inputs.iter().map(|(k, i)| port(k, i.ty, &i.bands, i.default.as_ref(), &i.doc)).collect(),
        outputs: u.outputs.iter().map(|(k, o)| port(k, o.ty, &[], None, &o.doc)).collect(),
        library: library_of(&u.name).lib.dir,
        error: None,
    })
}

/// Save the node of a draft in the library, as the next version of its name, and make the node of the model that
/// version (a calculator a group of itself first): the version saved. Nothing changes when the model would break
/// (a type that does not fit what the node is given or what reads it).
pub fn save(s: &mut Session, r: &Registry, d: &Draft) -> Result<UserNode, String> {
    let mut uses = d.uses()?;
    let (mut m, g) = as_group(s, r, &d.path, &d.id)?;
    crate::editor::set_uses(&mut m, r, &d.path, &d.id, Some(uses.clone()))?;
    let lib = Library::new(&d.library);
    let saved =
        lib.save(&UserNode::new(uses.clone(), g.body.clone().unwrap_or_default())).map_err(|e| e.to_string())?;
    uses.version = saved.version;
    crate::editor::set_uses(&mut m, r, &d.path, &d.id, Some(uses))?;
    s.source = m;
    changed();
    s.edited();
    Ok(saved)
}

/// Add a node of the library next to the node selected, its first input it needs fed by what the node selected
/// gives first that fits; it becomes the node selected.
pub fn add(s: &mut Session, r: &Registry, u: &UserNode) -> Result<String, String> {
    let path = s.editor.path.clone();
    let nodes = s.source.scope(&path).cloned().unwrap_or_default();
    let from = s.selected.as_ref().and_then(|id| nodes.iter().find(|n| &n.id == id)).cloned();
    let pos = crate::builder::beside(&nodes, from.as_ref().map(|n| n.id.as_str()));
    let id = crate::editor::add_user_node(&mut s.source, &path, u, pos)?;
    if let Some(f) = &from {
        crate::builder::feed(s, r, &path, &id, f);
    }
    s.edited();
    crate::builder::select(s, r, &id);
    Ok(id)
}

/// The dialog of a node being saved, over the panes, while there is one.
pub fn dialog(ctx: &egui::Context, s: &mut Session, r: &Registry) {
    let Some(mut d) = s.editor.saving.take() else { return };
    let mut open = true;
    let mut cancel = false;
    let mut done = false;
    let versions = Library::new(&d.library).versions(d.name.trim());
    let shelves = libraries();
    egui::Window::new("Save as my node")
        .id(egui::Id::new("save my node"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "A node of your library, used as one operator in any model: the model keeps a copy of the \
                     version it uses, so a new version changes it only when you update it.",
                )
                .size(11.5)
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(4.0);
            egui::Grid::new("my node head").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("Name");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(180.0));
                    let note = match versions.last() {
                        Some(v) => format!("version {} of {} in the library", v + 1, d.name.trim()),
                        None => "a new node: version 1".into(),
                    };
                    ui.label(RichText::new(note).size(11.0).color(ui.visuals().weak_text_color()));
                });
                ui.end_row();
                if shelves.len() > 1 {
                    ui.label("Library");
                    let shown = shelves.iter().find(|l| l.lib.dir == d.library).map_or("?", |l| l.name.as_str());
                    egui::ComboBox::from_id_salt("my node library").selected_text(shown).show_ui(ui, |ui| {
                        for l in &shelves {
                            let what =
                                if l.mine { "yours".to_string() } else { format!("shared: {}", l.lib.dir.display()) };
                            ui.selectable_value(&mut d.library, l.lib.dir.clone(), &l.name).on_hover_text(what);
                        }
                    });
                    ui.end_row();
                }
                ui.label("What it does");
                ui.add(egui::TextEdit::multiline(&mut d.description).desired_rows(2).desired_width(f32::INFINITY));
                ui.end_row();
            });
            ui.separator();
            ui.label(RichText::new("Inputs").strong());
            if d.inputs.is_empty() {
                ui.label(RichText::new("none").weak());
            }
            egui::Grid::new("my node inputs").num_columns(5).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
                if !d.inputs.is_empty() {
                    for h in ["name", "type", "layers it reads", "its value", "what it is"] {
                        ui.label(RichText::new(h).size(11.0).weak());
                    }
                    ui.end_row();
                }
                for (k, p) in d.inputs.iter_mut().enumerate() {
                    ui.label(RichText::new(&p.name).monospace());
                    type_box(ui, ("in", k), &mut p.ty);
                    // (sized: a text field in a grid is otherwise no wider than its column's header)
                    let h = ui.spacing().interact_size.y;
                    ui.add_sized([150.0, h], egui::TextEdit::singleline(&mut p.bands).hint_text("-"))
                        .on_hover_text("the layers a stack given it must have, by the names a script reads them");
                    ui.add_sized([70.0, h], egui::TextEdit::singleline(&mut p.default).hint_text("needed"))
                        .on_hover_text("its value when none is given (empty: it must be given)");
                    ui.add(egui::TextEdit::singleline(&mut p.doc).desired_width(150.0));
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new("Outputs").strong());
            egui::Grid::new("my node outputs").num_columns(3).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
                for (k, p) in d.outputs.iter_mut().enumerate() {
                    ui.label(RichText::new(&p.name).monospace());
                    type_box(ui, ("out", k), &mut p.ty);
                    ui.add(egui::TextEdit::singleline(&mut p.doc).desired_width(240.0).hint_text("what it is"));
                    ui.end_row();
                }
            });
            if let Some(e) = &d.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add(crate::app::accent_button("Save", true)).clicked() {
                    done = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                ui.label(
                    RichText::new(format!("in {}", d.library.display()))
                        .size(10.5)
                        .color(ui.visuals().weak_text_color()),
                );
            });
        });
    if done {
        match save(s, r, &d) {
            Ok(u) => {
                let shelf = shelves.iter().find(|l| l.lib.dir == d.library).map_or("the library", |l| l.name.as_str());
                s.editor.message = Some(format!("saved {} in {shelf}", u.label()));
                return;
            }
            Err(e) => d.error = Some(e),
        }
    }
    if open && !cancel {
        s.editor.saving = Some(d);
    }
}

fn type_box(ui: &mut Ui, salt: impl std::hash::Hash + std::fmt::Debug, ty: &mut Type) {
    egui::ComboBox::from_id_salt(salt).width(80.0).selected_text(ty.name()).show_ui(ui, |ui| {
        for t in TYPES {
            ui.selectable_value(ty, t, t.name());
        }
    });
}

/// Begin saving a node of the scope shown (a group or a calculator) as a node of the user's: the dialog.
pub fn begin(s: &mut Session, r: &Registry, id: &str) {
    let path = s.editor.path.clone();
    match draft(s, r, &path, id) {
        Ok(d) => s.editor.saving = Some(d),
        Err(e) => s.editor.message = Some(e),
    }
}

/// The library's nodes in the operators' pane (those a search finds, when there is one): a click adds one; a right
/// click takes it out of the library.
pub fn list(ui: &mut Ui, s: &mut Session, r: &Registry, query: &str) {
    let shelves = libraries();
    let all = shelved();
    let query = query.trim().to_lowercase();
    let found: Vec<&(usize, UserNode)> = all
        .iter()
        .filter(|(_, u)| {
            query.is_empty()
                || u.name.to_lowercase().contains(&query)
                || u.description.as_deref().is_some_and(|d| d.to_lowercase().contains(&query))
        })
        .collect();
    if found.is_empty() && !query.is_empty() {
        return;
    }
    let mut chosen = None;
    let mut removed = None;
    // the user's library, then each shared one (shown when it has nodes)
    for (k, shelf) in shelves.iter().enumerate() {
        let here: Vec<&UserNode> = found.iter().filter(|(at, _)| *at == k).map(|(_, u)| u).collect();
        if !shelf.mine && here.is_empty() {
            continue;
        }
        crate::theme::section(ui, &shelf.name);
        if !shelf.mine {
            ui.label(RichText::new(format!("shared · {}", shelf.lib.dir.display())).size(10.5).weak());
        } else if !all.iter().any(|(at, _)| *at == 0) {
            ui.label(
                RichText::new(
                    "Select a group or a calculator, then Save as my node (in its pane) to use it in any model.",
                )
                .size(11.0)
                .color(ui.visuals().weak_text_color()),
            );
        }
        shelf_nodes(ui, &here, &mut chosen, &mut removed, shelf);
    }
    if let Some(u) = chosen
        && let Err(e) = add(s, r, &u)
    {
        s.editor.message = Some(e);
    }
    if let Some((name, shelf)) = removed {
        if let Err(e) = shelf.lib.remove(&name) {
            s.editor.message = Some(e.to_string());
        }
        changed();
    }
    ui.add_space(4.0);
}

/// The nodes of a library in the operators' pane: the one clicked, the one taken out of it.
fn shelf_nodes(
    ui: &mut Ui,
    found: &[&UserNode],
    chosen: &mut Option<UserNode>,
    removed: &mut Option<(String, Shelf)>,
    shelf: &Shelf,
) {
    for u in found {
        let resp = ui.add(
            egui::Button::selectable(false, RichText::new(u.label()).monospace())
                .min_size(Vec2::new(ui.available_width(), 20.0)),
        );
        resp.context_menu(|ui| {
            if ui.button(format!("Take {} out of {} (every version)", u.name, shelf.name)).clicked() {
                *removed = Some((u.name.clone(), shelf.clone()));
                ui.close();
            }
            let others = if shelf.mine { "models using it" } else { "models using it, yours and others'," };
            ui.label(RichText::new(format!("{others} keep their copies")).size(10.5).weak());
        });
        let mut hover = u.description.clone().unwrap_or_else(|| format!("a node of {}", shelf.name));
        for (k, i) in &u.inputs {
            hover.push_str(&format!("\n{k}: {}", i.summary()));
        }
        for (k, o) in &u.outputs {
            hover.push_str(&format!("\n→ {k}: {}", o.ty.name()));
        }
        if resp.on_hover_text(hover).clicked() {
            *chosen = Some((*u).clone());
        }
    }
}

/// What the node pane tells of a node of the user's: its name and version, what it does, what its inputs need
/// (and what they are given that lacks a layer), and against the library: a newer version to update to, the
/// changes made here to save as a new version, a version not in the library to add to it; and to make it a plain
/// group. For a group or a calculator that is not one: to save it as one.
pub fn pane(ui: &mut Ui, s: &mut Session, r: &Registry, path: &[String], n: &Node) -> Option<Result<(), String>> {
    let mut change = None;
    let Some(u) = n.uses.as_ref().filter(|_| n.op == GROUP) else {
        if (n.op == GROUP || n.op == "raster.calc")
            && ui
                .button("Save as my node...")
                .on_hover_text("a node of your library, to use in any model as one operator")
                .clicked()
        {
            begin(s, r, &n.id);
        }
        return None;
    };
    ui.add_space(2.0);
    let shelf = library_of(&u.name);
    let whose = if shelf.mine { "a node of yours".to_string() } else { format!("a node of {} (shared)", shelf.name) };
    ui.label(RichText::new(format!("{} · {whose}", u.label())).strong())
        .on_hover_text(shelf.lib.dir.display().to_string());
    if let Some(d) = &u.description {
        ui.label(d);
    }
    for (k, i) in &u.inputs {
        ui.label(RichText::new(format!("{k}: {}", i.summary())).size(11.5).color(ui.visuals().weak_text_color()))
            .on_hover_text(i.doc.clone().unwrap_or_default());
    }
    for (k, why) in lacking(s, path, n) {
        ui.colored_label(ui.visuals().warn_fg_color, format!("{k}: {why}"));
    }
    let lib = shelf.lib;
    let last = lib.versions(&u.name).last().copied();
    let mine = lib.get(&u.name, Some(u.version)).ok().filter(|v| v.label() == u.label());
    let edited = mine.as_ref().is_none_or(|v| v.differs(n));
    ui.horizontal_wrapped(|ui| {
        if let Some(v) = last.filter(|v| *v > u.version)
            && !edited
            && ui
                .button(format!("Update to {}@{v}", u.name))
                .on_hover_text(
                    "its body and ports become that version's; what it is given is kept where it still takes it",
                )
                .clicked()
        {
            change = Some(lib.get(&u.name, Some(v)).map_err(|e| e.to_string()).and_then(|newer| {
                crate::editor::update_user_node(&mut s.source, r, path, &n.id, &newer).map(|gone| {
                    if !gone.is_empty() {
                        let text = format!("{} takes no {} any more: taken away", newer.label(), gone.join(", "));
                        s.notice = Some((text, std::time::Instant::now()));
                    }
                })
            }));
        }
        if edited {
            let what = if mine.is_some() { "changed here since" } else { "not in the library:" };
            ui.label(RichText::new(format!("{what} {}", u.label())).size(11.5).color(ui.visuals().warn_fg_color));
            if ui.button("Save as a new version...").clicked() {
                begin(s, r, &n.id);
            }
        } else if ui.button("Edit and save a new version...").on_hover_text("its ports and what it does").clicked() {
            begin(s, r, &n.id);
        }
        if ui
            .button("Make it a plain group")
            .on_hover_text("it stays as it is, no longer a node of your library")
            .clicked()
        {
            change = Some(crate::editor::set_uses(&mut s.source, r, path, &n.id, None));
        }
    });
    change
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Body;

    #[test]
    fn what_a_group_takes_and_gives_is_found() {
        let g = Node::new("veg", GROUP)
            .input("img", "$s2")
            .literal("k", 0.5)
            .body(Body {
                nodes: vec![
                    Node::new("calc", "raster.calc")
                        .input("s", "@img")
                        .input("k", "@k")
                        .param("expression", "x = s.nir\nout ndvi = (x - s.red) / (x + s.red) * k"),
                    Node::new("more", "raster.calc")
                        .input("s", "@img")
                        .input("t", "@extra")
                        .param("expression", "out b = s.swir"),
                ],
                outputs: [("ndvi".to_string(), "calc.ndvi".to_string()), ("b".to_string(), "more.b".to_string())]
                    .into(),
                ..Default::default()
            })
            .note("the index");
        let u = infer(&g, &|_| Type::Raster, &|p| if p == "ndvi" { Type::Map } else { Type::Any });
        assert_eq!(u.name, "veg");
        assert_eq!(u.description.as_deref(), Some("the index"));
        assert_eq!(u.inputs["img"].ty, Type::Stack);
        assert_eq!(u.inputs["img"].bands, ["nir", "red", "swir"]);
        assert_eq!((u.inputs["k"].ty, u.inputs["k"].default.clone()), (Type::Number, Some(0.5.into())));
        // read inside and not given: an input still, of a type not known
        assert_eq!(u.inputs["extra"].ty, Type::Any);
        assert_eq!(u.outputs["ndvi"].ty, Type::Map);
        assert_eq!(u.outputs["b"].ty, Type::Any);
        // a node of the library again: what it declared kept where nothing better is known
        let mut again = g.clone();
        let mut declared = u.clone();
        declared.outputs.get_mut("b").unwrap().ty = Type::Map;
        declared.outputs.get_mut("b").unwrap().doc = Some("the short infrared".into());
        declared.version = 2;
        again.uses = Some(declared);
        let u2 = infer(&again, &|_| Type::Any, &|_| Type::Any);
        assert_eq!((u2.outputs["b"].ty, u2.outputs["b"].doc.as_deref()), (Type::Map, Some("the short infrared")));
        assert_eq!(u2.version, 2);
    }

    #[test]
    fn a_node_of_the_libraries_named_as_an_operator() {
        assert_eq!(named("my:ndvi"), Some(("ndvi", None)));
        assert_eq!(named("my:ndvi@2"), Some(("ndvi", Some(2))));
        assert_eq!(named("my:a@b"), Some(("a@b", None)));
        assert_eq!(named("raster.calc"), None);
        // its values are its settings, its data its inputs (unless fed by another node)
        let mut g = Node::new("veg", GROUP).literal("k", 2).input("img", "s.stack").input("t", "x.value");
        let mut u = Uses {
            name: "veg".into(),
            version: 1,
            description: None,
            inputs: Default::default(),
            outputs: Default::default(),
        };
        for (k, ty) in [("k", Type::Number), ("img", Type::Stack), ("t", Type::Text)] {
            u.inputs.insert(k.into(), UserInput::new(ty));
        }
        g.uses = Some(u);
        assert!(is_setting(&g, "k", Type::Number));
        assert!(!is_setting(&g, "img", Type::Stack));
        assert!(!is_setting(&g, "t", Type::Text), "fed by another node: an input");
        assert!(!is_setting(&Node::new("calc", "raster.calc").literal("k", 2), "k", Type::Number));
    }

    #[test]
    fn a_draft_reads_as_what_the_node_declares() {
        let d = Draft {
            path: Vec::new(),
            id: "veg".into(),
            calc: false,
            name: " ndvi ".into(),
            description: "".into(),
            inputs: vec![
                Port {
                    name: "img".into(),
                    ty: Type::Stack,
                    bands: "nir, red".into(),
                    default: "".into(),
                    doc: "".into(),
                },
                Port {
                    name: "k".into(),
                    ty: Type::Number,
                    bands: "".into(),
                    default: "0.5".into(),
                    doc: "a factor".into(),
                },
            ],
            outputs: vec![Port {
                name: "ndvi".into(),
                ty: Type::Map,
                bands: "".into(),
                default: "".into(),
                doc: "".into(),
            }],
            library: PathBuf::new(),
            error: None,
        };
        let u = d.uses().unwrap();
        assert_eq!(u.name, "ndvi");
        assert_eq!(u.description, None);
        assert_eq!(u.inputs["img"].bands, ["nir", "red"]);
        assert_eq!(u.inputs["k"].default, Some(0.5.into()));
        assert_eq!(u.inputs["k"].doc.as_deref(), Some("a factor"));
        let mut bad = d.clone();
        bad.inputs[0].bands = "nir, B-8".into();
        assert!(bad.uses().is_err());
    }
}
