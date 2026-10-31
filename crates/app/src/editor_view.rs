//! The node editor's canvas: the nodes of a scope with their ports (inputs on the left, outputs on the right),
//! what the scope may read on the left (the model's inputs, a container's values), the connections. Drag a
//! node to move it, drag its right edge to make it wider (never narrower than its texts need; a double click on
//! the edge: as wide as they need), drag from an output to an input to connect them, click a node to edit it in
//! the side panel, double-click a container to edit its body (another node: its map in a tab of its own), click
//! an output to see it. Drag a connection off an input to take it away (onto another input: moved there), or
//! right-click the input; right-click a node for its menu (Run on a folder...). Drag the background to select the nodes in a box (shift or ctrl and a click add a node
//! or take it away, ctrl+A selects all, Esc none): dragging one of them moves them all. Drag with the middle or the right
//! button to move the view, the wheel zooms around the pointer, Home fits the graph. Each node shows how it did
//! in the last run: running, done (or taken from the cache), failed; a node with problems (an input it needs not
//! given, the run's error) has a mark that tells them on hover; a check (`check.assert`) a badge of how it fared
//! (passed, warned, failed, not run), a container the worst of its checks'; a node whose result is frozen a ❄ at its
//! top left. A box that reads or writes a file names it at
//! its bottom; the boxes of data (the model's inputs, what reads, writes, stacks or plots) may have a header in the
//! colour of the type they give (a map's blue, vectors' pink...), an option of the settings.
//!
//! A box shows the inputs its node needs and those given; its optional others are folded in a row ("+ 9
//! optional") that a click unfolds. A connection let go on a node rather than on an input goes to the free input it
//! fits, or to the one chosen in a list when several fit. Zoomed out, the ports' names give way and the nodes' names
//! stay readable (never smaller than a size that can be read, cut to their box).
//!
//! Built for graphs of many thousands of nodes:
//! - the scene (where every box, port and connection is, a grid of the boxes to find what is under the
//!   pointer) is made only when the graph changes, in time proportional to its nodes and connections;
//! - the boxes, ports and connections are drawn by the GPU (`canvas_gpu`), from instances uploaded when the
//!   scene changes: moving and zooming the view costs nothing;
//! - texts are drawn by egui, only for the boxes in view and when the zoom makes them readable.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use eframe::egui_wgpu;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use terraflux_graph::{Model, Node, Ref, Source, Type};
use terraflux_ops::Registry;

use crate::canvas_gpu::{Canvas, EdgeInstance, RectInstance, Shapes};
use crate::editor;

/// The narrowest a box is: wider when its texts need it (a long name, an input's and an output's names side by
/// side on a row), or when it was made wider by dragging its right edge.
pub const WIDTH: f32 = 190.0;
const HEAD: f32 = 38.0;
const ROW: f32 = 18.0;
const PORT: f32 = 5.0;
/// The row at the bottom of a box that names the file it reads or writes.
const FOOT: f32 = 20.0;
/// How long the halo of a node the assistant reads or changes lasts.
const PULSE: std::time::Duration = std::time::Duration::from_millis(2500);
/// The side of a cell of the grid that finds the boxes under a point, graph units.
const CELL: f32 = 256.0;
/// The space between a box's edge and a port's name, and between the names of an input and an output on a row.
const PAD: f32 = 9.0;
const GAP: f32 = 16.0;
/// What the place of a new input says.
const NEW_INPUT: &str = "+ new input";
/// The zoom from which the ports' names and the operators are written (below it, the nodes' names alone, in the
/// middle of their boxes).
const PORTS_FROM: f32 = 0.6;
/// The zoom under which the canvas frames and names the model's stages (and writes its groups' notes).
const STAGES_UNDER: f32 = 0.35;
/// The smallest size a node's name is written at, and the narrowest box on the screen it is written in.
const NAME_MIN: f32 = 11.0;
const NAME_ROOM: f32 = 40.0;

/// The fonts of a box's texts at zoom 1: its name; its operator and its ports' names; its file.
fn name_font() -> FontId {
    FontId::proportional(14.0)
}

fn port_font() -> FontId {
    FontId::monospace(11.0)
}

fn file_font() -> FontId {
    FontId::monospace(10.5)
}

/// A text's width at zoom 1 without the fonts at hand: Hack's advance for a monospace font (exact for its
/// letters), a little more than Ubuntu's mean for a proportional one.
pub fn estimate(text: &str, font: &FontId) -> f32 {
    let per = if font.family == egui::FontFamily::Monospace { 0.6021 } else { 0.58 };
    text.chars().count() as f32 * per * font.size
}

/// The width a box needs for its texts: its name (the light of its state after it), its operator, and on every
/// row the name of an input and that of the output across from it, a gap between them; never under [`WIDTH`],
/// rounded up to 10.
fn fitted(
    name: &str,
    op: &str,
    inputs: &[&str],
    outputs: &[&str],
    measure: &mut dyn FnMut(&str, &FontId) -> f32,
) -> f32 {
    let mut w = 8.0 + measure(name, &name_font()) + 24.0;
    if !op.is_empty() {
        w = w.max(8.0 + measure(op, &port_font()) + 8.0);
    }
    for k in 0..inputs.len().max(outputs.len()) {
        let mut side = |s: Option<&&str>| s.filter(|s| !s.is_empty()).map_or(0.0, |s| measure(s, &port_font()));
        let (l, r) = (side(inputs.get(k)), side(outputs.get(k)));
        let gap = if l > 0.0 && r > 0.0 { GAP } else { 0.0 };
        w = w.max(PAD + l + gap + r + PAD);
    }
    ((w / 10.0).ceil() * 10.0).max(WIDTH)
}

/// The names on a node's rows, its optional inputs folded: its inputs (the place of a new one, for an operator
/// that takes any; the row of the folded ones) and its outputs.
fn port_names(n: &Node, r: &Registry) -> (Vec<String>, Vec<String>) {
    let (shown, folded) = shown_inputs(n, r, false);
    let mut inputs: Vec<String> =
        shown.into_iter().map(|p| if p.name.is_empty() { NEW_INPUT.to_string() } else { p.name }).collect();
    if !folded.is_empty() {
        inputs.push(fold_label(folded.len(), false));
    }
    (inputs, editor::output_ports(n, r).into_iter().map(|(p, _)| p).collect())
}

/// A node's inputs as its box shows them: those it needs, those given (connected or a value), its operator's
/// essential ones, the optional others when `unfolded`, and the place of a new one (an operator that takes any);
/// the optional ones not shown (and the settings of a node of the user's: its values, set in the Node pane).
fn shown_inputs(n: &Node, r: &Registry, unfolded: bool) -> (Vec<Port>, Vec<(String, Type)>) {
    let mut shown = Vec::new();
    let mut folded = Vec::new();
    let essentials = terraflux_ops::essentials::essentials(&n.op).unwrap_or_default();
    for (name, ty, required) in editor::input_ports(n, r) {
        let connected = n.inputs.contains_key(&name);
        // (a node of the user's: a value it is given is a setting, in the Node pane, unless it must still be given)
        let setting = crate::my_nodes::is_setting(n, &name, ty) && !(required && !connected);
        if !setting && (required || connected || essentials.contains(&name.as_str())) || unfolded {
            shown.push(Port { connected, name, ty, required });
        } else {
            folded.push((name, ty));
        }
    }
    // (a node of the user's takes the inputs it declares, no others)
    if let Some(ty) = r.sig(&n.op).and_then(|s| s.extra).filter(|_| n.uses.is_none()) {
        shown.push(Port { name: String::new(), ty, required: false, connected: false });
    }
    (shown, folded)
}

/// What a node's box names under its id: its operator, or the node of the user's library it is (`ndvi@3`).
pub fn op_label(n: &Node) -> String {
    n.uses.as_ref().map_or_else(|| n.op.clone(), |u| u.label())
}

/// The words of the row that folds a node's optional inputs: how many there are, or that a click folds them.
fn fold_label(folded: usize, unfolded: bool) -> String {
    if unfolded { "- fewer".to_string() } else { format!("+ {folded} optional") }
}

/// The rows of a node's box, its optional inputs folded (for laying nodes out).
fn rows_of(n: &Node, r: &Registry) -> usize {
    let (shown, folded) = shown_inputs(n, r, false);
    (shown.len() + usize::from(!folded.is_empty())).max(editor::output_ports(n, r).len()).max(1)
}

/// The width of a node's box (its texts measured without the fonts at hand: for laying nodes out), the width
/// it was given when wider.
pub fn node_width(n: &Node, r: &Registry) -> f32 {
    let (inputs, outputs) = port_names(n, r);
    let (i, o): (Vec<&str>, Vec<&str>) =
        (inputs.iter().map(String::as_str).collect(), outputs.iter().map(String::as_str).collect());
    let w = fitted(&n.id, &op_label(n), &i, &o, &mut estimate);
    n.width.map_or(w, |u| u.max(w))
}

/// The operators by name, for the widths of nodes laid out where no registry is at hand.
pub fn standard() -> &'static Registry {
    static R: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    R.get_or_init(Registry::standard)
}

/// The colour of a type's ports and connections, the website's graphs' own (a map blue, a stack violet, a table
/// gold, a number green, features pink, agents ochre); `None` for the others (drawn in the theme's grey). On a
/// light theme a shade darker, to stand out on white.
pub fn type_colour(t: Type, dark: bool) -> Option<Color32> {
    let (r, g, b) = match t {
        Type::Map | Type::Raster => (0x5f, 0xa8, 0xe0),
        Type::Stack | Type::Series => (0xa0, 0x8a, 0xe6),
        Type::Table => (0xe3, 0xbd, 0x62),
        Type::Number | Type::Integer | Type::Bool => (0x9c, 0xcf, 0x7c),
        Type::Vector => (0xe2, 0x8a, 0xa8),
        Type::Agents => (0xd3, 0x9b, 0x6a),
        _ => return None,
    };
    let c = Color32::from_rgb(r, g, b);
    Some(if dark { c } else { c.lerp_to_gamma(Color32::BLACK, 0.3) })
}

/// How a node did in the last run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Running,
    /// Its outputs are kept: run now, or `cached` (nothing it depends on changed).
    Done {
        cached: bool,
    },
    Failed,
}

/// What the canvas reports of a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Select(String),
    /// Show an output of a node.
    Preview(String, String),
    /// A node's map in a tab of its own (a double click).
    Open(String),
    Enter(String),
    Move(String, [f32; 2]),
    /// A node's box given a width (dragged by its right edge), or none: as wide as its texts need (a double click
    /// on the edge).
    Resize(String, Option<f32>),
    Connect {
        reference: String,
        node: String,
        port: String,
    },
    /// A connection taken off an input.
    Disconnect {
        node: String,
        port: String,
    },
    /// A node run on every raster of a folder (chosen in a dialog): its menu's "Run on a folder...".
    RunOnFolder(String),
    /// An input of the model made a node (a `raster.read` of its file).
    InputToNode(String),
    /// A `raster.read` made an input of the model.
    NodeToInput(String),
    /// The box of something the scope reads taken away: a model's input removed, with what read it.
    RemoveSource(String),
}

/// What "Make it a node" does.
pub const MAKE_NODE_HELP: &str = "the input a raster.read node of its file, read by what read it: a node like the \
     others, which can read a folder";

/// What "Make it an input of the model" does.
pub const MAKE_INPUT_HELP: &str = "the raster.read an input of the model ($name), read by what read it: the file \
     the command line, Many runs and the scenarios change";

/// What the box of something a scope reads is, besides an input of the model.
pub fn source_help(key: &str) -> String {
    match key {
        "@step" => "@step: the loop's step (1, 2...)".into(),
        "@item" => "@item: the element of the collection the body runs on".into(),
        "@index" => "@index: the element's place in the collection (1, 2...)".into(),
        "@name" => "@name: the element's name".into(),
        k if k.starts_with('@') => {
            format!("{k}: a value the container gives its body (its input, or carried along its loop)")
        }
        k => format!("{k}: a node outside this body, read here"),
    }
}

/// Where the view looks: the screen offset of the graph's origin from the canvas's corner, and the points per
/// unit of the graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Camera {
    /// The camera that shows `bounds` in `rect` (no closer than 1:1).
    fn fitting(bounds: Rect, size: Vec2) -> Camera {
        let zoom = (size.x / bounds.width().max(1.0)).min(size.y / bounds.height().max(1.0)) * 0.92;
        let zoom = zoom.clamp(0.04, 1.0);
        Camera { pan: size / 2.0 - bounds.center().to_vec2() * zoom, zoom }
    }
}

/// What the pointer is doing on the canvas.
#[derive(Debug, Clone, PartialEq)]
enum Drag {
    /// A connection drawn from an output (its reference and where it starts, graph units).
    Connect(String, Pos2),
    /// The nodes selected moved: where the pointer started and each node's corner then (graph units).
    Move(Pos2, Vec<(String, Pos2)>),
    /// A node made wider or narrower by its right edge: its id, its left edge and the width its texts need.
    Resize(String, f32, f32),
    /// A box of selection from a corner (graph units).
    Select(Pos2),
}

/// The editor's state between frames.
#[derive(Default)]
pub struct EditorState {
    /// The scope edited: the containers' ids from the top.
    pub path: Vec<String>,
    drag: Option<Drag>,
    /// The search of the operators' library.
    pub search: String,
    /// The search of the operators from the keyboard ("/"), while it is open.
    pub quick: Option<crate::builder::Quick>,
    pub message: Option<String>,
    /// Where the view looks, by scope.
    cameras: HashMap<Vec<String>, Camera>,
    fit: bool,
    scene: Option<Scene>,
    generation: u64,
    /// Where the canvas was drawn last.
    rect: Option<Rect>,
    /// The nodes selected (of the scope shown), and the scope they are of.
    pub chosen: BTreeSet<String>,
    chosen_in: Vec<String>,
    /// The model shown as text instead of the canvas.
    pub text: bool,
    /// A node being renamed (ctrl+R): its id, the name typed, whether the field was given the focus.
    pub renaming: Option<(String, String, bool)>,
    /// The node running and its share done, as its operator tells it (a line along its bottom).
    pub progress: Option<(String, f32)>,
    /// The nodes whose optional inputs are unfolded, and a counter of their changes (the scene made again then).
    pub unfolded: HashSet<String>,
    folds: u64,
    /// A connection let go on a node that several of its free inputs fit: the list to choose one from.
    picking: Option<Pick>,
    /// The nodes marked (of the top level), in a colour each: those changed in a review ([`crate::review`]).
    pub marks: HashMap<String, Color32>,
    /// The nodes the assistant reads or changes now (their scope, their id, when): a halo that fades.
    pub pulse: Vec<(Vec<String>, String, std::time::Instant)>,
    /// A node being saved as a node of the user's library: the dialog's state.
    pub saving: Option<crate::my_nodes::Draft>,
    /// The box whose menu is open (right-clicked), and its operator (none for what a scope reads).
    menu: Option<(String, String)>,
}

/// The inputs of a node a connection may go to, offered where it was let go.
#[derive(Debug, Clone)]
struct Pick {
    reference: String,
    node: String,
    ports: Vec<(String, Type)>,
    at: Pos2,
}

impl EditorState {
    /// Unfold a node's optional inputs, or fold them.
    pub fn toggle_fold(&mut self, node: &str) {
        if !self.unfolded.remove(node) {
            self.unfolded.insert(node.to_string());
        }
        self.folds += 1;
    }

    /// Fit the graph in the view at the next frame.
    pub fn fit(&mut self) {
        self.fit = true;
    }

    /// Look at the scope shown from a camera: the graph's origin at `pan` from the canvas's corner, `zoom`
    /// points per unit.
    pub fn look(&mut self, pan: Vec2, zoom: f32) {
        self.cameras.insert(self.path.clone(), Camera { pan, zoom });
    }

    /// The zoom of the scope shown.
    pub fn zoom(&self) -> f32 {
        self.cameras.get(&self.path).map_or(1.0, |c| c.zoom)
    }

    /// The generation of the shapes the GPU was last given (no two editors share one).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The boxes of the scene shown (what the scope reads, then its nodes).
    pub fn boxes(&self) -> usize {
        self.scene.as_ref().map_or(0, |s| s.boxes.len())
    }

    fn on_screen(&self, p: Pos2) -> Option<Pos2> {
        let cam = self.cameras.get(&self.path)?;
        Some(self.rect?.min + cam.pan + p.to_vec2() * cam.zoom)
    }

    /// The node nearest a node in a direction (a unit vector, y down), among the nodes of the scope shown: within
    /// 60 degrees of it, the nearest by distance, sideways steps counting double; the first node (top left) when
    /// none is given.
    pub fn neighbour(&self, from: Option<&str>, dir: Vec2) -> Option<String> {
        let s = self.scene.as_ref()?;
        let nodes = || s.boxes.iter().filter(|b| !b.source);
        let Some(at) = from.and_then(|f| s.index.get(f)).map(|&i| s.boxes[i].rect.center()) else {
            return nodes()
                .min_by(|a, b| (a.rect.min.y + a.rect.min.x).total_cmp(&(b.rect.min.y + b.rect.min.x)))
                .map(|b| b.key.clone());
        };
        nodes()
            .filter(|b| Some(b.key.as_str()) != from)
            .filter_map(|b| {
                let d = b.rect.center() - at;
                let along = d.dot(dir);
                let across = (d - dir * along).length();
                (along > 0.0 && across <= along * 1.732).then(|| (along + 2.0 * across, b.key.clone()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, k)| k)
    }

    /// Where the canvas was drawn last.
    pub fn canvas(&self) -> Option<Rect> {
        self.rect
    }

    /// The point of the graph (graph units) at a point of the screen over the canvas.
    pub fn graph_at(&self, p: Pos2) -> Option<Pos2> {
        let rect = self.rect.filter(|r| r.contains(p))?;
        let cam = self.cameras.get(&self.path)?;
        Some(((p - rect.min - cam.pan) / cam.zoom).to_pos2())
    }

    /// Keep a node in view: the camera moved just enough that its box is on the canvas (with a margin).
    pub fn reveal(&mut self, node: &str) {
        let (Some(s), Some(rect)) = (self.scene.as_ref(), self.rect) else { return };
        let Some(&i) = s.index.get(node) else { return };
        let b = s.boxes[i].rect;
        let Some(cam) = self.cameras.get_mut(&self.path) else { return };
        let (min, max) =
            (rect.min + cam.pan + b.min.to_vec2() * cam.zoom, rect.min + cam.pan + b.max.to_vec2() * cam.zoom);
        let inner = rect.shrink(40.0);
        let mut shift = Vec2::ZERO;
        if min.x < inner.min.x {
            shift.x = inner.min.x - min.x;
        } else if max.x > inner.max.x {
            shift.x = (inner.max.x - max.x).max(inner.min.x - min.x);
        }
        if min.y < inner.min.y {
            shift.y = inner.min.y - min.y;
        } else if max.y > inner.max.y {
            shift.y = (inner.max.y - max.y).max(inner.min.y - min.y);
        }
        cam.pan += shift;
    }

    /// Where a node's header is on the screen (as last drawn).
    pub fn screen_of(&self, node: &str) -> Option<Pos2> {
        let s = self.scene.as_ref()?;
        let b = &s.boxes[*s.index.get(node)?];
        self.on_screen(b.head().center())
    }

    /// Where a node's box is on the screen (as last drawn).
    pub fn screen_rect_of(&self, node: &str) -> Option<Rect> {
        let s = self.scene.as_ref()?;
        let b = &s.boxes[*s.index.get(node)?];
        Some(Rect::from_min_max(self.on_screen(b.rect.min)?, self.on_screen(b.rect.max)?))
    }

    /// The width of a node's box (graph units) and the width its texts need.
    pub fn width_of(&self, node: &str) -> Option<(f32, f32)> {
        let s = self.scene.as_ref()?;
        let b = &s.boxes[*s.index.get(node)?];
        Some((b.rect.width(), b.fitted))
    }

    /// Where a node's output port is on the screen.
    pub fn screen_of_output(&self, node: &str, port: &str) -> Option<Pos2> {
        let s = self.scene.as_ref()?;
        let b = &s.boxes[*s.index.get(node)?];
        self.on_screen(b.output_at(b.outputs.iter().position(|(p, _)| p == port)?))
    }

    /// Where the row that folds or unfolds a node's optional inputs is on the screen.
    pub fn screen_of_fold(&self, node: &str) -> Option<Pos2> {
        let s = self.scene.as_ref()?;
        let b = &s.boxes[*s.index.get(node)?];
        self.on_screen(b.fold()?.center())
    }

    /// Where a node's input port is on the screen.
    pub fn screen_of_input(&self, node: &str, port: &str) -> Option<Pos2> {
        let s = self.scene.as_ref()?;
        let b = &s.boxes[*s.index.get(node)?];
        self.on_screen(b.input_at(b.inputs.iter().position(|p| p.name == port)?))
    }
}

/// An input port: its name, type, whether the operator needs it and whether something feeds it. A port of no
/// name is the place of a new input, at the bottom of a node whose operator takes any (`stack.of`'s maps).
struct Port {
    name: String,
    ty: Type,
    required: bool,
    connected: bool,
}

/// A box of the canvas: a node, or something its scope reads (on the left, one output).
struct BoxAt {
    key: String,
    op: String,
    /// What it names under its id ([`op_label`]).
    label: String,
    /// graph units
    rect: Rect,
    /// The width its texts need (it is never narrower).
    fitted: f32,
    inputs: Vec<Port>,
    outputs: Vec<(String, Type)>,
    source: bool,
    container: bool,
    note: Option<String>,
    /// The file it reads or writes (a node's `path`, a model input's), named in the row at its bottom.
    file: Option<String>,
    /// Its optional inputs not shown, and whether they are unfolded (shown, the row then folds them).
    folded: Vec<(String, Type)>,
    unfolded: bool,
    /// Turned off (its inputs passed on): drawn faint.
    bypass: bool,
    /// Its result frozen (kept on disk, used instead of running it): a mark at its top left.
    frozen: bool,
}

/// The name of a file short enough for a box's row: its name without its folder, cut in the middle (the end kept,
/// with the extension) when longer than `max` characters.
fn file_label(path: &str, max: usize) -> String {
    let name = path.rsplit(['/', '\\']).find(|p| !p.is_empty()).unwrap_or(path);
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max {
        return name.to_string();
    }
    let tail = (max / 3).max(1);
    let head = max.saturating_sub(tail + 1);
    chars[..head].iter().chain(['…'].iter()).chain(chars[chars.len() - tail..].iter()).collect()
}

impl BoxAt {
    /// What a connection from an output refers to.
    fn reference(&self, k: usize) -> String {
        if self.source { self.key.clone() } else { format!("{}.{}", self.key, self.outputs[k].0) }
    }

    fn input_at(&self, i: usize) -> Pos2 {
        Pos2::new(self.rect.left(), self.rect.top() + HEAD + (i as f32 + 0.5) * ROW)
    }

    fn output_at(&self, i: usize) -> Pos2 {
        if self.source {
            Pos2::new(self.rect.right(), self.rect.top() + HEAD)
        } else {
            Pos2::new(self.rect.right(), self.rect.top() + HEAD + (i as f32 + 0.5) * ROW)
        }
    }

    fn head(&self) -> Rect {
        Rect::from_min_size(self.rect.min, Vec2::new(self.rect.width(), HEAD))
    }

    /// The row that folds or unfolds its optional inputs, under its inputs, if it has optional inputs.
    fn fold(&self) -> Option<Rect> {
        if self.folded.is_empty() && !self.unfolded {
            return None;
        }
        let y = self.rect.top() + HEAD + self.inputs.len() as f32 * ROW;
        Some(Rect::from_min_size(Pos2::new(self.rect.left(), y), Vec2::new(self.rect.width() * 0.6, ROW)))
    }

    /// The row naming its file, if it has one.
    fn foot(&self) -> Option<Rect> {
        self.file.as_ref()?;
        Some(Rect::from_min_max(Pos2::new(self.rect.left(), self.rect.bottom() - FOOT), self.rect.max))
    }

    /// The type of what it gives first, for a box of data (its colour marks the box): a model's input, a node that
    /// reads or writes, makes a stack or a table of its inputs, or plots; none for the others (the functions).
    fn tint(&self) -> Option<Type> {
        let op = self.op.as_str();
        let data = self.source
            || op.ends_with(".read")
            || op.ends_with(".write")
            || op.starts_with("stack.")
            || op.starts_with("view.")
            || op == "table.of";
        if data { self.outputs.first().map(|(_, t)| *t) } else { None }
    }
}

/// What is under a point.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Output(usize, usize),
    Input(usize, usize),
    /// A node's right edge, which resizes it.
    Edge(usize),
    Head(usize),
    /// The row that folds or unfolds a node's optional inputs.
    Fold(usize),
    Body(usize),
}

/// A connection: from a box's output to a box's input.
struct EdgeAt {
    from: (usize, usize),
    to: (usize, usize),
}

/// The rectangles the GPU draws of a box: the box, its header's colour, the line over its file, its ports and the
/// light of its state.
fn box_rects(b: &BoxAt, state: Option<NodeStatus>, v: &egui::Visuals, colours: bool) -> Vec<RectInstance> {
    let mut rects = Vec::with_capacity(4 + b.inputs.len() + b.outputs.len());
    let stroke = v.widgets.noninteractive.bg_stroke.color;
    let fill = if b.source { v.faint_bg_color } else { v.widgets.inactive.bg_fill };
    // a node turned off: faint, its border the warning's colour
    let fill = if b.bypass { fill.gamma_multiply(0.45) } else { fill };
    let (border, width) = match state {
        Some(NodeStatus::Failed) => (v.error_fg_color, 1.5),
        _ if b.bypass => (v.warn_fg_color, 1.5),
        _ => (stroke, 1.0),
    };
    let radius = crate::theme::shape().node_radius;
    rects.push(RectInstance::new(b.rect.min.into(), b.rect.size().into(), radius, fill, border, width));
    // a node of data's header in the colour of the type it gives (a map's blue, vectors' pink...), when
    // asked: what flows from it at a glance
    if let Some(c) = b.tint().filter(|_| colours).and_then(|t| type_colour(t, v.dark_mode)) {
        let tint = fill.lerp_to_gamma(c, if v.dark_mode { 0.22 } else { 0.18 });
        let head = b.head().shrink(2.0);
        let r = radius - 1.0;
        rects.push(RectInstance::new(head.min.into(), head.size().into(), r, tint, Color32::TRANSPARENT, 0.0));
    }
    // the line over the row naming its file
    if let Some(f) = b.foot() {
        let at = [f.left() + 6.0, f.top()];
        rects.push(RectInstance::new(at, [f.width() - 12.0, 1.0], 0.0, stroke, Color32::TRANSPARENT, 0.0));
    }
    let dot = |c: Pos2, colour: Color32| {
        RectInstance::new([c.x - PORT, c.y - PORT], [2.0 * PORT; 2], PORT, colour, Color32::TRANSPARENT, 0.0)
    };
    for (i, p) in b.inputs.iter().enumerate() {
        let colour = if p.connected {
            type_colour(p.ty, v.dark_mode).unwrap_or(v.text_color())
        } else if p.required {
            v.warn_fg_color
        } else {
            v.weak_text_color()
        };
        rects.push(dot(b.input_at(i), colour));
    }
    for (i, (_, t)) in b.outputs.iter().enumerate() {
        rects.push(dot(b.output_at(i), type_colour(*t, v.dark_mode).unwrap_or(v.text_color())));
    }
    if let Some(s) = state {
        let colour = match s {
            NodeStatus::Running => crate::theme::accent(),
            NodeStatus::Done { cached: false } => Color32::from_rgb(120, 220, 110),
            NodeStatus::Done { cached: true } => Color32::from_rgb(80, 150, 75),
            NodeStatus::Failed => v.error_fg_color,
        };
        let c = b.rect.right_top() + Vec2::new(-10.0, 10.0);
        rects.push(RectInstance::new([c.x - 4.0, c.y - 4.0], [8.0; 2], 4.0, colour, Color32::TRANSPARENT, 0.0));
    }
    rects
}

/// A scope laid out: its boxes, a grid of them, and the shapes the GPU draws (each box's rectangles at
/// `ranges[box]`, the connections touching each box in `touching[box]`, to move a box alone).
struct Scene {
    key: (u64, Vec<String>, u64, &'static str, bool, u64),
    boxes: Vec<BoxAt>,
    index: HashMap<String, usize>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    bounds: Rect,
    shapes: Arc<Shapes>,
    ranges: Vec<std::ops::Range<usize>>,
    edges: Vec<EdgeAt>,
    touching: Vec<Vec<usize>>,
    /// The stage of each box of the top level, when it has no groups (the model's inputs are data): what the
    /// canvas frames when zoomed out.
    stage_of: Vec<Option<crate::document::Stage>>,
}

fn cell(p: Pos2) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

/// The cells of the grid a box is in (with its ports).
fn cells_of(r: Rect) -> impl Iterator<Item = (i32, i32)> {
    let r = r.expand(PORT + 4.0);
    let (a, z) = (cell(r.min), cell(r.max));
    (a.0..=z.0).flat_map(move |cx| (a.1..=z.1).map(move |cy| (cx, cy)))
}

/// The node ids of a scope that a node reads (its body's references to its own nodes left out).
fn reads(n: &Node, ids: &HashMap<&str, usize>) -> Vec<usize> {
    let own: HashSet<&str> =
        n.body.as_ref().map(|b| b.nodes.iter().map(|m| m.id.as_str()).collect()).unwrap_or_default();
    let mut out = Vec::new();
    for r in terraflux_graph::references(n) {
        if let Ok(Ref::Port { node, .. }) = Ref::parse(&r)
            && node != n.id
            && !own.contains(node.as_str())
            && let Some(&i) = ids.get(node.as_str())
        {
            out.push(i);
        }
    }
    out
}

/// The depth of every node of a scope: the longest chain of nodes it reads (a cycle counts once), in time
/// proportional to the nodes and connections.
fn depths(nodes: &[Node], deps: &[Vec<usize>]) -> Vec<usize> {
    let mut depth: Vec<Option<usize>> = vec![None; nodes.len()];
    let mut on_path = vec![false; nodes.len()];
    for start in 0..nodes.len() {
        if depth[start].is_some() {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        on_path[start] = true;
        while let Some(&mut (i, ref mut k)) = stack.last_mut() {
            if let Some(&d) = deps[i].get(*k) {
                *k += 1;
                if depth[d].is_none() && !on_path[d] {
                    on_path[d] = true;
                    stack.push((d, 0));
                }
            } else {
                let d = deps[i].iter().filter_map(|&d| depth[d]).max().map_or(0, |m| m + 1);
                depth[i] = Some(d);
                on_path[i] = false;
                stack.pop();
            }
        }
    }
    depth.into_iter().map(|d| d.unwrap_or(0)).collect()
}

/// Where the columns of depths begin: each after the widest box of the one before and a space.
fn columns_x(nodes: &[Node], depth: &[usize], r: &Registry, space: f32) -> Vec<f32> {
    let columns = depth.iter().max().map_or(0, |d| d + 1);
    let mut widest = vec![WIDTH; columns];
    for (n, &d) in nodes.iter().zip(depth) {
        widest[d] = widest[d].max(node_width(n, r));
    }
    let mut x = 0.0;
    widest
        .iter()
        .map(|w| {
            let at = x;
            x += w + space;
            at
        })
        .collect()
}

/// Where a scope's nodes go when they have no place yet: a column per depth (the longest chain of nodes they
/// read), as wide as its widest box, in time proportional to the nodes and connections.
pub fn auto_positions(nodes: &[Node]) -> HashMap<String, [f32; 2]> {
    let ids: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let deps: Vec<Vec<usize>> = nodes.iter().map(|n| reads(n, &ids)).collect();
    let depth = depths(nodes, &deps);
    let xs = columns_x(nodes, &depth, standard(), 60.0);
    let mut column: HashMap<usize, f32> = HashMap::new();
    let mut out = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        let d = depth[i];
        let y = column.entry(d).or_insert(0.0);
        out.insert(n.id.clone(), [xs[d], *y]);
        *y += HEAD + 6.0 * ROW + 20.0;
    }
    out
}

/// A tidy place for every node of a scope: a column per depth (the longest chain of nodes it reads), each
/// column ordered by where the nodes it reads are (the mean of their rows, twice over), so that connections
/// cross little; the boxes stacked by their heights.
pub fn arrange(nodes: &[Node], r: &Registry) -> HashMap<String, [f32; 2]> {
    let ids: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let deps: Vec<Vec<usize>> = nodes.iter().map(|n| reads(n, &ids)).collect();
    let depth = depths(nodes, &deps);
    let xs = columns_x(nodes, &depth, r, 90.0);
    let columns = xs.len();
    let mut column: Vec<Vec<usize>> = vec![Vec::new(); columns];
    // a first order: as they are on the canvas
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| {
        let (pa, pb) = (nodes[a].pos.unwrap_or([0.0, 0.0]), nodes[b].pos.unwrap_or([0.0, 0.0]));
        pa[1].total_cmp(&pb[1]).then(a.cmp(&b))
    });
    for i in order {
        column[depth[i]].push(i);
    }
    let mut rank = vec![0.0f32; nodes.len()];
    let set_ranks = |column: &Vec<Vec<usize>>, rank: &mut Vec<f32>| {
        for c in column {
            for (k, &i) in c.iter().enumerate() {
                rank[i] = k as f32;
            }
        }
    };
    set_ranks(&column, &mut rank);
    for _ in 0..2 {
        for c in column.iter_mut().skip(1) {
            let key = |i: usize| {
                let up: Vec<f32> = deps[i].iter().map(|&d| rank[d]).collect();
                if up.is_empty() { rank[i] } else { up.iter().sum::<f32>() / up.len() as f32 }
            };
            c.sort_by(|&a, &b| key(a).total_cmp(&key(b)));
            for (k, &i) in c.iter().enumerate() {
                rank[i] = k as f32;
            }
        }
    }
    let mut out = HashMap::new();
    for (d, c) in column.iter().enumerate() {
        let mut y = 0.0;
        for &i in c {
            let n = &nodes[i];
            let rows = rows_of(n, r);
            out.insert(n.id.clone(), [xs[d], y]);
            let foot = if n.params.get("path").and_then(|v| v.as_str()).is_some_and(|p| !p.trim().is_empty()) {
                FOOT
            } else {
                0.0
            };
            y += HEAD + rows as f32 * ROW + 6.0 + foot + 30.0;
        }
    }
    out
}

impl Scene {
    #[allow(clippy::too_many_arguments)]
    fn build(
        model: &Model,
        nodes: &[Node],
        r: &Registry,
        path: &[String],
        status: &BTreeStatus,
        v: &egui::Visuals,
        colours: bool,
        unfolded: &HashSet<String>,
        measure: &mut dyn FnMut(&str, &FontId) -> f32,
    ) -> Scene {
        let ids: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
        // what the scope reads: the model's inputs, its containers' values, and the nodes outside it its nodes
        // read
        let mut sources = editor::sources(model, r, path);
        let mut seen: HashSet<String> = sources.iter().map(|(s, _)| s.clone()).collect();
        for n in nodes {
            let own: HashSet<&str> =
                n.body.as_ref().map(|b| b.nodes.iter().map(|m| m.id.as_str()).collect()).unwrap_or_default();
            for reference in terraflux_graph::references(n) {
                if let Ok(Ref::Port { node, .. }) = Ref::parse(&reference)
                    && !ids.contains_key(node.as_str())
                    && !own.contains(node.as_str())
                    && seen.insert(reference.clone())
                {
                    sources.push((reference, Type::Any));
                }
            }
        }
        let mut boxes: Vec<BoxAt> = Vec::with_capacity(sources.len() + nodes.len());
        // a column as wide as the longest name of what the scope reads
        let sources_w = sources.iter().map(|(s, _)| fitted(s, "", &[], &[], measure)).fold(WIDTH, f32::max);
        let sources_x = -(sources_w + 80.0);
        let mut y = 0.0;
        for (s, t) in &sources {
            // a model's input of a raster file: the file named at its bottom
            let file = s.strip_prefix('$').and_then(|k| match model.inputs.get(k) {
                Some(terraflux_graph::Input::Map { path, .. } | terraflux_graph::Input::Stack { path, .. }) => {
                    Some(path.clone())
                }
                _ => None,
            });
            let foot = if file.is_some() { FOOT } else { 0.0 };
            // where it was moved, else in the column at the left
            let placed = model.places.get(&terraflux_graph::place_key(path, s)).map(|&[x, y]| Pos2::new(x, y));
            let rect =
                Rect::from_min_size(placed.unwrap_or(Pos2::new(sources_x, y)), Vec2::new(sources_w, HEAD + ROW + foot));
            boxes.push(BoxAt {
                key: s.clone(),
                op: String::new(),
                label: String::new(),
                rect,
                fitted: sources_w,
                inputs: Vec::new(),
                outputs: vec![(String::new(), *t)],
                source: true,
                container: false,
                note: None,
                file,
                folded: Vec::new(),
                unfolded: false,
                bypass: false,
                frozen: false,
            });
            if placed.is_none() {
                y += HEAD + ROW + foot + 12.0;
            }
        }
        let missing = nodes.iter().any(|n| n.pos.is_none());
        let auto = if missing { auto_positions(nodes) } else { HashMap::new() };
        for n in nodes {
            let p = n.pos.unwrap_or_else(|| auto[&n.id]);
            let open = unfolded.contains(&n.id);
            let (inputs, folded) = shown_inputs(n, r, open);
            // (unfolded, it had some to fold)
            let open = open && inputs.iter().any(|p| !p.required && !p.connected && !p.name.is_empty());
            let outputs = editor::output_ports(n, r);
            let fold = (!folded.is_empty() || open).then(|| fold_label(folded.len(), open));
            let rows = (inputs.len() + usize::from(fold.is_some())).max(outputs.len()).max(1);
            // as wide as its texts need, or as it was made
            let mut names: Vec<&str> =
                inputs.iter().map(|p| if p.name.is_empty() { NEW_INPUT } else { p.name.as_str() }).collect();
            names.extend(fold.as_deref());
            let outs: Vec<&str> = outputs.iter().map(|(p, _)| p.as_str()).collect();
            let label = op_label(n);
            let need = fitted(&n.id, &label, &names, &outs, measure);
            let width = n.width.map_or(need, |w| w.max(need));
            // the file it reads or writes
            let file = n.params.get("path").and_then(|v| v.as_str()).filter(|p| !p.trim().is_empty()).map(String::from);
            let foot = if file.is_some() { FOOT } else { 0.0 };
            let rect =
                Rect::from_min_size(Pos2::new(p[0], p[1]), Vec2::new(width, HEAD + rows as f32 * ROW + 6.0 + foot));
            boxes.push(BoxAt {
                key: n.id.clone(),
                op: n.op.clone(),
                label,
                rect,
                fitted: need,
                inputs,
                outputs,
                source: false,
                container: n.body.is_some(),
                note: [n.note.clone(), n.why.as_ref().map(|w| format!("Why: {w}"))]
                    .into_iter()
                    .flatten()
                    .reduce(|a, b| format!("{a}\n{b}")),
                file,
                folded,
                unfolded: open,
                bypass: n.bypass,
                frozen: n.frozen,
            });
        }
        let index: HashMap<String, usize> = boxes.iter().enumerate().map(|(i, b)| (b.key.clone(), i)).collect();
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let mut bounds = Rect::NOTHING;
        for (i, b) in boxes.iter().enumerate() {
            bounds = bounds.union(b.rect);
            for c in cells_of(b.rect) {
                grid.entry(c).or_default().push(i);
            }
        }
        if !bounds.is_positive() {
            bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0));
        }
        // the shapes: connections, then boxes, then ports and lights
        let mut edges = Vec::new();
        let mut edges_at = Vec::new();
        let mut touching: Vec<Vec<usize>> = vec![Vec::new(); boxes.len()];
        let line = v.weak_text_color();
        for (k, n) in nodes.iter().enumerate() {
            let b = &boxes[sources.len() + k];
            for (port, src) in &n.inputs {
                let Source::Ref(reference) = src else { continue };
                let Some(i) = b.inputs.iter().position(|p| p.name == *port) else { continue };
                let from = if let Some(&s) = index.get(reference.as_str()).filter(|&&s| boxes[s].source) {
                    Some((s, 0))
                } else if let Ok(Ref::Port { node, port: out }) = Ref::parse(reference) {
                    index
                        .get(node.as_str())
                        .and_then(|&s| boxes[s].outputs.iter().position(|(p, _)| *p == out).map(|o| (s, o)))
                } else {
                    None
                };
                if let Some((s, o)) = from {
                    let to_box = sources.len() + k;
                    touching[s].push(edges.len());
                    if s != to_box {
                        touching[to_box].push(edges.len());
                    }
                    edges_at.push(EdgeAt { from: (s, o), to: (to_box, i) });
                    let from = boxes[s].output_at(o);
                    let to = b.input_at(i);
                    // the type of what flows: the output's, else (an output of any type) the input's
                    let colour = type_colour(boxes[s].outputs[o].1, v.dark_mode)
                        .or_else(|| type_colour(b.inputs[i].ty, v.dark_mode))
                        .unwrap_or(line);
                    edges.push(EdgeInstance {
                        from: [from.x, from.y],
                        to: [to.x, to.y],
                        color: colour.to_array(),
                        width: 1.6,
                    });
                }
            }
        }
        // what a container's body reads around it (no input of the container gives it): a thinner line to the
        // container's side, after the connections (it is not one to take off, and follows the boxes once dropped)
        for (k, n) in nodes.iter().enumerate() {
            let Some(body) = &n.body else { continue };
            let own: HashSet<&str> = body.nodes.iter().map(|m| m.id.as_str()).collect();
            let given: HashSet<&String> =
                n.inputs.values().filter_map(|s| if let Source::Ref(r) = s { Some(r) } else { None }).collect();
            let mut seen = HashSet::new();
            for reference in terraflux_graph::references(n) {
                if given.contains(&reference) || !seen.insert(reference.clone()) {
                    continue;
                }
                let Ok(Ref::Port { node, port: out }) = Ref::parse(&reference) else { continue };
                if own.contains(node.as_str()) {
                    continue;
                }
                let Some((s, o)) = index
                    .get(node.as_str())
                    .and_then(|&s| boxes[s].outputs.iter().position(|(p, _)| *p == out).map(|o| (s, o)))
                else {
                    continue;
                };
                let b = &boxes[sources.len() + k];
                let from = boxes[s].output_at(o);
                let to = Pos2::new(b.rect.left(), b.rect.top() + HEAD / 2.0);
                edges.push(EdgeInstance {
                    from: [from.x, from.y],
                    to: [to.x, to.y],
                    color: line.gamma_multiply(0.55).to_array(),
                    width: 1.0,
                });
            }
        }
        let mut rects = Vec::with_capacity(boxes.len() * 6);
        let mut ranges = Vec::with_capacity(boxes.len());
        for b in &boxes {
            let first = rects.len();
            let state = if path.is_empty() { status.get(&b.key).copied() } else { None };
            rects.extend(box_rects(b, state, v, colours));
            ranges.push(first..rects.len());
        }
        let stage_of = if path.is_empty() && !nodes.iter().any(|n| n.op == terraflux_graph::GROUP) {
            let stages = crate::document::stages(nodes);
            boxes
                .iter()
                .map(|b| if b.source { Some(crate::document::Stage::Data) } else { stages.get(&b.key).copied() })
                .collect()
        } else {
            vec![None; boxes.len()]
        };
        Scene {
            key: (0, Vec::new(), 0, "", false, 0),
            boxes,
            index,
            grid,
            bounds,
            shapes: Arc::new(Shapes { generation: 0, rects, edges }),
            ranges,
            edges: edges_at,
            touching,
            stage_of,
        }
    }

    /// Move a box to a corner (graph units): its rectangles, its connections and its place in the grid, the rest
    /// as it is (a node dragged costs the connections it has, not the graph).
    fn move_box(&mut self, i: usize, min: Pos2, generation: u64) {
        let d = min - self.boxes[i].rect.min;
        if d == Vec2::ZERO {
            return;
        }
        self.reshape(i, self.boxes[i].rect.translate(d));
        let shapes = Arc::make_mut(&mut self.shapes);
        for r in &mut shapes.rects[self.ranges[i].clone()] {
            r.min[0] += d.x;
            r.min[1] += d.y;
        }
        self.follow(i, generation);
    }

    /// Give a box a width (graph units): its rectangles drawn again, its connections and its place in the grid
    /// following, the rest as it is.
    fn resize_box(&mut self, i: usize, width: f32, state: Option<NodeStatus>, v: &egui::Visuals, generation: u64) {
        let b = self.boxes[i].rect;
        if (b.width() - width).abs() < 0.01 {
            return;
        }
        self.reshape(i, Rect::from_min_size(b.min, Vec2::new(width, b.height())));
        let rects = box_rects(&self.boxes[i], state, v, self.key.4);
        let shapes = Arc::make_mut(&mut self.shapes);
        let range = self.ranges[i].clone();
        // (the same shapes of the same box: only their places and sizes change)
        if rects.len() == range.len() {
            shapes.rects[range].copy_from_slice(&rects);
        }
        self.follow(i, generation);
    }

    /// A box's new rectangle, and its place in the grid.
    fn reshape(&mut self, i: usize, rect: Rect) {
        for c in cells_of(self.boxes[i].rect) {
            if let Some(v) = self.grid.get_mut(&c) {
                v.retain(|&j| j != i);
            }
        }
        self.boxes[i].rect = rect;
        for c in cells_of(rect) {
            let v = self.grid.entry(c).or_default();
            // the grid keeps the boxes in their order (the last drawn on top)
            let at = v.partition_point(|&j| j < i);
            v.insert(at, i);
        }
        self.bounds = self.bounds.union(rect);
    }

    /// The connections of a box to where its ports are now.
    fn follow(&mut self, i: usize, generation: u64) {
        let shapes = Arc::make_mut(&mut self.shapes);
        for &e in &self.touching[i] {
            let EdgeAt { from, to } = self.edges[e];
            let (f, t) = (self.boxes[from.0].output_at(from.1), self.boxes[to.0].input_at(to.1));
            shapes.edges[e].from = [f.x, f.y];
            shapes.edges[e].to = [t.x, t.y];
        }
        shapes.generation = generation;
    }

    /// What is under a point (graph units), ports within `reach`; the boxes drawn last first.
    /// The output that feeds an input (box, input): its box and output.
    fn feeding(&self, i: usize, k: usize) -> Option<(usize, usize)> {
        self.edges.iter().find(|e| e.to == (i, k)).map(|e| e.from)
    }

    /// The type of what a reference gives (an output of a box of the scene), if known.
    fn output_type(&self, reference: &str) -> Option<Type> {
        let (i, k) = self.index.get(reference).map(|&i| (i, 0)).or_else(|| {
            let (node, port) = reference.split_once('.')?;
            let i = *self.index.get(node)?;
            Some((i, self.boxes[i].outputs.iter().position(|(p, _)| p == port)?))
        })?;
        self.boxes[i].outputs.get(k).map(|(_, t)| *t)
    }

    fn hit(&self, p: Pos2, reach: f32) -> Option<Hit> {
        let candidates = self.grid.get(&cell(p))?;
        for &i in candidates.iter().rev() {
            let b = &self.boxes[i];
            if let Some(o) = (0..b.outputs.len()).find(|&o| b.output_at(o).distance(p) <= reach) {
                return Some(Hit::Output(i, o));
            }
            if let Some(k) = (0..b.inputs.len()).find(|&k| b.input_at(k).distance(p) <= reach) {
                return Some(Hit::Input(i, k));
            }
        }
        for &i in candidates.iter().rev() {
            let b = &self.boxes[i];
            if !b.source
                && (p.x - b.rect.right()).abs() <= reach * 0.5
                && (b.rect.top()..=b.rect.bottom()).contains(&p.y)
            {
                return Some(Hit::Edge(i));
            }
            if b.head().contains(p) {
                return Some(Hit::Head(i));
            }
            if b.fold().is_some_and(|f| f.contains(p)) {
                return Some(Hit::Fold(i));
            }
            if b.rect.contains(p) {
                return Some(Hit::Body(i));
            }
        }
        None
    }

    /// The boxes within a rectangle (graph units).
    fn within(&self, view: Rect) -> Vec<usize> {
        let (a, z) = (cell(view.min), cell(view.max));
        let cells = (z.0 - a.0 + 1) as i64 * (z.1 - a.1 + 1) as i64;
        let mut out: Vec<usize> = if cells as usize > self.grid.len() {
            (0..self.boxes.len()).filter(|&i| self.boxes[i].rect.intersects(view)).collect()
        } else {
            let mut v = Vec::new();
            for cx in a.0..=z.0 {
                for cy in a.1..=z.1 {
                    if let Some(c) = self.grid.get(&(cx, cy)) {
                        v.extend(c.iter().copied().filter(|&i| self.boxes[i].rect.intersects(view)));
                    }
                }
            }
            v
        };
        out.sort_unstable();
        out.dedup();
        out
    }
}

type BTreeStatus = std::collections::BTreeMap<String, NodeStatus>;

/// A generation of the shapes no canvas had before: the GPU keeps the shapes it was given last and takes new ones
/// when their generation changes, so every editor's (a new model's, a template's) must differ from the others'.
fn next_generation() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The name of a new input reading `reference`: the node it comes from (`elevation.map`: `elevation`), the model's
/// input or the carried value; `_2`, `_3`... after it when a node has that name already.
pub fn new_input(reference: &str, taken: &[&str]) -> String {
    let base = match Ref::parse(reference) {
        Ok(Ref::Port { node, .. }) => node,
        Ok(Ref::Input(n) | Ref::Carried(n)) => n,
        Err(_) => "input".to_string(),
    };
    let base: String = base.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    let base = if base.is_empty() { "input".to_string() } else { base };
    (1..)
        .map(|k| if k == 1 { base.clone() } else { format!("{base}_{k}") })
        .find(|n| !taken.contains(&n.as_str()))
        .unwrap()
}

/// Zoomed out (`frames`): a frame around the boxes of each stage of the top level, its name above it (a model
/// without groups). A group's note under its box, when its box is wide enough on the screen.
fn stages_and_notes(
    painter: &egui::Painter,
    scene: &Scene,
    screen: &dyn Fn(Pos2) -> Pos2,
    view: Rect,
    frames_shown: bool,
    v: &egui::Visuals,
) {
    let mut frames: Vec<(crate::document::Stage, Rect)> = Vec::new();
    for (b, st) in scene.boxes.iter().zip(&scene.stage_of) {
        let Some(st) = st else { continue };
        match frames.iter_mut().find(|(s, _)| s == st) {
            Some((_, r)) => *r = r.union(b.rect),
            None => frames.push((*st, b.rect)),
        }
    }
    frames.sort_by_key(|(s, _)| *s);
    for (k, (st, r)) in frames.iter().enumerate() {
        if !frames_shown || !r.intersects(view) {
            continue;
        }
        let colour = crate::palette::class_colour(k);
        let on = Rect::from_min_max(screen(r.min), screen(r.max)).expand(10.0);
        painter.rect_filled(on, 8.0, colour.gamma_multiply(0.06));
        painter.rect_stroke(on, 8.0, Stroke::new(1.5, colour.gamma_multiply(0.7)), egui::StrokeKind::Outside);
        painter.text(
            on.left_top() + Vec2::new(2.0, -4.0),
            Align2::LEFT_BOTTOM,
            st.title(),
            FontId::proportional(15.0),
            colour,
        );
    }
    for b in scene.boxes.iter().filter(|b| b.op == terraflux_graph::GROUP && b.rect.intersects(view)) {
        let Some(note) = &b.note else { continue };
        let on = Rect::from_min_max(screen(b.rect.min), screen(b.rect.max));
        if on.width() < 90.0 {
            continue;
        }
        let mut job =
            egui::text::LayoutJob::simple(note.clone(), FontId::proportional(11.5), v.weak_text_color(), on.width());
        job.wrap.max_rows = 3;
        let galley = painter.layout_job(job);
        painter.galley(on.left_bottom() + Vec2::new(2.0, 4.0), galley, v.weak_text_color());
    }
}

/// Draw the scope; what the user did. `revision` and `status_rev` change when the model and the nodes' states
/// do (the scene is made again then only).
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    model: &Model,
    r: &Registry,
    st: &mut EditorState,
    selected: Option<&str>,
    status: &BTreeStatus,
    revision: u64,
    status_rev: u64,
    shown: Option<(&str, &str)>,
    problems: &HashMap<String, Vec<(bool, String)>>,
    layers: &HashMap<String, usize>,
    checks: &HashMap<String, crate::checks::Verdict>,
) -> Vec<Action> {
    let mut actions = Vec::new();
    let Some(nodes) = model.scope(&st.path) else {
        st.path.clear();
        return actions;
    };
    // (built again when the model, the place in it, the nodes' states or the theme change)
    let colours = crate::settings::get().node_colours;
    let key = (revision, st.path.clone(), status_rev, crate::theme::current().name, colours, st.folds);
    if st.scene.as_ref().is_none_or(|s| s.key != key) {
        // the texts measured in the fonts they are written in
        let v = ui.visuals().clone();
        let mut scene = ui.ctx().fonts_mut(|f| {
            let mut measure = |s: &str, font: &FontId| s.chars().map(|c| f.glyph_width(font, c)).sum::<f32>();
            Scene::build(model, nodes, r, &st.path, status, &v, colours, &st.unfolded, &mut measure)
        });
        st.generation = next_generation();
        scene.key = key;
        Arc::get_mut(&mut scene.shapes).unwrap().generation = st.generation;
        st.scene = Some(scene);
    }
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    st.rect = Some(rect);
    // a selection is of a scope: another scope, none
    if st.chosen_in != st.path {
        st.chosen.clear();
        st.chosen_in = st.path.clone();
    }
    if let Some(s) = selected
        && st.chosen.is_empty()
        && nodes.iter().any(|n| n.id == s)
    {
        st.chosen.insert(s.to_string());
    }
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    let (all, none) =
        ui.input(|i| (i.modifiers.command && i.key_pressed(egui::Key::A), i.key_pressed(egui::Key::Escape)));
    if !typing && all && (resp.hovered() || resp.has_focus() || !st.chosen.is_empty()) {
        st.chosen = nodes.iter().map(|n| n.id.clone()).collect();
    }
    if !typing && none {
        st.chosen.clear();
    }
    let adding = ui.input(|i| i.modifiers.shift || i.modifiers.command);
    let scene = st.scene.as_ref().unwrap();
    let path = st.path.clone();
    if st.fit || !st.cameras.contains_key(&path) {
        st.cameras.insert(path.clone(), Camera::fitting(scene.bounds, rect.size()));
        st.fit = false;
    }
    let cam = st.cameras.get_mut(&path).unwrap();
    // the wheel zooms around the pointer
    if let Some(p) = resp.hover_pos() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = pinch * (scroll / 300.0).exp();
        if factor != 1.0 {
            let under = (p - rect.min - cam.pan) / cam.zoom;
            cam.zoom = (cam.zoom * factor).clamp(0.03, 3.0);
            cam.pan = p - rect.min - under * cam.zoom;
        }
    }
    let to_world = |p: Pos2, cam: &Camera| ((p - rect.min - cam.pan) / cam.zoom).to_pos2();
    let reach = |cam: &Camera| (PORT + 3.0).max(8.0 / cam.zoom);
    // a drag: a connection from an output, the nodes selected moved, a box of selection; the view with the
    // middle or the right button
    if resp.drag_started_by(egui::PointerButton::Primary)
        && let Some(o) = ui.input(|i| i.pointer.press_origin())
    {
        let w = to_world(o, cam);
        st.drag = Some(match scene.hit(w, reach(cam)) {
            Some(Hit::Output(i, k)) => {
                let b = &scene.boxes[i];
                Drag::Connect(b.reference(k), b.output_at(k))
            }
            // a connection taken off an input, carried by the pointer (dropped on an input: moved there)
            Some(Hit::Input(i, k)) if scene.boxes[i].inputs[k].connected => match scene.feeding(i, k) {
                Some((j, o)) => {
                    let b = &scene.boxes[i];
                    actions.push(Action::Disconnect { node: b.key.clone(), port: b.inputs[k].name.clone() });
                    Drag::Connect(scene.boxes[j].reference(o), scene.boxes[j].output_at(o))
                }
                None => Drag::Select(w),
            },
            // a node's right edge: its width
            Some(Hit::Edge(i)) => {
                let b = &scene.boxes[i];
                Drag::Resize(b.key.clone(), b.rect.left(), b.fitted)
            }
            // a node, or the box of something the scope reads: moved
            Some(Hit::Head(i) | Hit::Body(i) | Hit::Fold(i)) => {
                let key = &scene.boxes[i].key;
                if !st.chosen.contains(key) {
                    if !adding {
                        st.chosen.clear();
                    }
                    st.chosen.insert(key.clone());
                    actions.push(Action::Select(key.clone()));
                }
                let corners = st
                    .chosen
                    .iter()
                    .filter_map(|k| scene.index.get(k).map(|&j| (k.clone(), scene.boxes[j].rect.min)))
                    .collect();
                Drag::Move(w, corners)
            }
            _ => Drag::Select(w),
        });
    }
    if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
        cam.pan += resp.drag_delta();
    }
    let pointer = resp.hover_pos().or(ui.input(|i| i.pointer.interact_pos()));
    let mut moved = Vec::new();
    let mut resized = None;
    if let (Some(Drag::Resize(id, left, need)), Some(q)) = (&st.drag, pointer)
        && resp.dragged()
    {
        // never narrower than its texts need
        let w = (to_world(q, cam).x - left).max(*need).round();
        actions.push(Action::Resize(id.clone(), Some(w)));
        resized = scene.index.get(id).map(|&i| (i, w));
    }
    if let (Some(Drag::Move(from, corners)), Some(q)) = (&st.drag, pointer)
        && resp.dragged()
    {
        // the group keeps its shape, anywhere on the canvas
        let d = to_world(q, cam) - *from;
        for (id, c) in corners {
            let p = *c + d;
            actions.push(Action::Move(id.clone(), [p.x, p.y]));
            if let Some(&i) = scene.index.get(id) {
                moved.push((i, p));
            }
        }
    }
    let boxed = match (&st.drag, pointer) {
        (Some(Drag::Select(from)), Some(q)) => Some(Rect::from_two_pos(*from, to_world(q, cam))),
        _ => None,
    };
    if resp.drag_stopped() {
        if let (Some(Drag::Connect(reference, _)), Some(q)) = (&st.drag, pointer) {
            // on an input; on a node that takes any (its place of a new input, or anywhere on it): a new one,
            // named as what it reads
            let port = match scene.hit(to_world(q, cam), reach(cam)) {
                Some(Hit::Input(i, k)) if !scene.boxes[i].inputs[k].name.is_empty() => {
                    Some((i, scene.boxes[i].inputs[k].name.clone()))
                }
                Some(Hit::Input(i, _) | Hit::Head(i) | Hit::Body(i))
                    if scene.boxes[i].inputs.last().is_some_and(|p| p.name.is_empty()) =>
                {
                    let names: Vec<&str> = scene.boxes[i].inputs.iter().map(|p| p.name.as_str()).collect();
                    Some((i, new_input(reference, &names)))
                }
                // on a node: the free input of its (shown or folded) that what is dragged fits; a list when several do
                Some(Hit::Head(i) | Hit::Body(i) | Hit::Fold(i)) if !scene.boxes[i].source => {
                    let b = &scene.boxes[i];
                    let ty = scene.output_type(reference);
                    let free: Vec<(String, Type)> = b
                        .inputs
                        .iter()
                        .filter(|p| !p.connected && !p.name.is_empty())
                        .map(|p| (p.name.clone(), p.ty))
                        .chain(b.folded.iter().cloned())
                        .filter(|(_, t)| ty.is_none_or(|ty| ty.fits(*t)))
                        .collect();
                    match free.as_slice() {
                        [(port, _)] => Some((i, port.clone())),
                        [] => None,
                        _ => {
                            st.picking = Some(Pick {
                                reference: reference.clone(),
                                node: b.key.clone(),
                                ports: free,
                                at: pointer.unwrap_or_default(),
                            });
                            None
                        }
                    }
                }
                _ => None,
            };
            if let Some((i, port)) = port {
                actions.push(Action::Connect { reference: reference.clone(), node: scene.boxes[i].key.clone(), port });
            }
        }
        // the nodes in the box: the selection (added to it with shift or ctrl)
        if let Some(r) = boxed {
            if !adding {
                st.chosen.clear();
            }
            for i in scene.within(r) {
                if !scene.boxes[i].source && r.intersects(scene.boxes[i].rect) {
                    st.chosen.insert(scene.boxes[i].key.clone());
                }
            }
            if st.chosen.len() == 1 {
                actions.push(Action::Select(st.chosen.first().unwrap().clone()));
            }
        }
        st.drag = None;
    }
    // clicks
    let at = resp.interact_pointer_pos().map(|p| to_world(p, cam));
    if resp.clicked()
        && let Some(w) = at
    {
        match scene.hit(w, reach(cam)) {
            Some(Hit::Output(i, k)) if !scene.boxes[i].source => {
                actions.push(Action::Preview(scene.boxes[i].key.clone(), scene.boxes[i].outputs[k].0.clone()));
            }
            Some(Hit::Fold(i)) => {
                let key = scene.boxes[i].key.clone();
                if !st.unfolded.remove(&key) {
                    st.unfolded.insert(key);
                }
                st.folds += 1;
                ui.ctx().request_repaint();
            }
            Some(Hit::Head(i) | Hit::Body(i) | Hit::Input(i, _) | Hit::Edge(i)) => {
                let key = scene.boxes[i].key.clone();
                if adding {
                    // shift or ctrl: one more, or one less
                    if !st.chosen.remove(&key) {
                        st.chosen.insert(key.clone());
                        actions.push(Action::Select(key));
                    }
                } else {
                    st.chosen.clear();
                    st.chosen.insert(key.clone());
                    actions.push(Action::Select(key));
                }
            }
            None if !adding => st.chosen.clear(),
            _ => {}
        }
    }
    // a right click on a connected input takes its connection away
    if resp.secondary_clicked()
        && let Some(w) = at
        && let Some(Hit::Input(i, k)) = scene.hit(w, reach(cam))
        && scene.boxes[i].inputs[k].connected
        && !scene.boxes[i].source
    {
        let b = &scene.boxes[i];
        actions.push(Action::Disconnect { node: b.key.clone(), port: b.inputs[k].name.clone() });
    }
    // a right click on a node, or on the box of something the scope reads: its menu (elsewhere none)
    if resp.secondary_clicked() {
        st.menu = at.and_then(|w| match scene.hit(w, reach(cam)) {
            Some(Hit::Head(i) | Hit::Body(i)) => Some((scene.boxes[i].key.clone(), scene.boxes[i].op.clone())),
            _ => None,
        });
    }
    resp.context_menu(|ui| {
        let Some((key, op)) = st.menu.clone() else {
            ui.close();
            return;
        };
        let mut item = |ui: &mut Ui, text: &str, help: &str, action: Action| {
            if ui.button(text).on_hover_text(help).clicked() {
                actions.push(action);
                ui.close();
            }
        };
        if let Some(name) = key.strip_prefix('$') {
            item(ui, "Make it a node", MAKE_NODE_HELP, Action::InputToNode(name.to_string()));
            item(
                ui,
                "Delete the input",
                "the model's input taken away; what read it reads nothing",
                Action::RemoveSource(key.clone()),
            );
        } else if crate::editor::is_source(&key) {
            ui.label(egui::RichText::new(source_help(&key)).weak());
        } else {
            item(ui, "Run on a folder...", crate::node_view::FOLDER_HELP, Action::RunOnFolder(key.clone()));
            if op == "raster.read" && st.path.is_empty() {
                item(ui, "Make it an input of the model", MAKE_INPUT_HELP, Action::NodeToInput(key.clone()));
            }
        }
    });
    if resp.double_clicked()
        && let Some(w) = at
    {
        match scene.hit(w, reach(cam)) {
            // a double click on the right edge: as wide as the texts need
            Some(Hit::Edge(i)) => actions.push(Action::Resize(scene.boxes[i].key.clone(), None)),
            Some(Hit::Head(i) | Hit::Body(i)) if !scene.boxes[i].source => {
                let b = &scene.boxes[i];
                actions.push(if b.container { Action::Enter(b.key.clone()) } else { Action::Open(b.key.clone()) });
            }
            _ => {}
        }
    }
    // the pointer over a right edge, or dragging it: a cursor that says so
    let on_edge = matches!(st.drag, Some(Drag::Resize(..)))
        || (st.drag.is_none()
            && resp.hover_pos().is_some_and(|p| matches!(scene.hit(to_world(p, cam), reach(cam)), Some(Hit::Edge(_)))));
    if on_edge {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    let cam = *cam;
    // the nodes dragged move in the scene at once (the model's places follow through the actions)
    if !moved.is_empty() {
        st.generation = next_generation();
        let generation = st.generation;
        let scene = st.scene.as_mut().unwrap();
        for (i, p) in moved {
            scene.move_box(i, p, generation);
        }
    }
    // the node resized changes in the scene at once too
    if let Some((i, w)) = resized {
        st.generation = next_generation();
        let generation = st.generation;
        let state =
            if st.path.is_empty() { status.get(&st.scene.as_ref().unwrap().boxes[i].key).copied() } else { None };
        st.scene.as_mut().unwrap().resize_box(i, w, state, ui.visuals(), generation);
    }
    let scene = st.scene.as_ref().unwrap();
    let screen = |p: Pos2| rect.min + cam.pan + p.to_vec2() * cam.zoom;
    // the GPU draws the boxes, ports and connections
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        Canvas { shapes: scene.shapes.clone(), size: rect.size().into(), origin: cam.pan.into(), zoom: cam.zoom },
    ));
    let painter = ui.painter_at(rect);
    let v = ui.visuals().clone();
    // the weak texts on a box: between the weak and the plain text, to read on the box's lighter grey
    let faint = v.weak_text_color().lerp_to_gamma(v.text_color(), 0.5);
    // the texts of the boxes in view: the nodes' names never smaller than can be read (in the middle of their
    // boxes when the ports' names are not written), cut to their boxes; the operators and the ports' names when
    // the zoom makes them readable
    let view = Rect::from_min_max(to_world(rect.min, &cam), to_world(rect.max, &cam));
    let size = |s: f32| ((s * cam.zoom) * 2.0).round() / 2.0;
    let in_view = scene.within(view);
    let ports = cam.zoom >= PORTS_FROM;
    // (zoomed far out over thousands of nodes, the boxes alone)
    if in_view.len() <= 600 || ports {
        for &i in &in_view {
            let b = &scene.boxes[i];
            let on_screen = Rect::from_min_max(screen(b.rect.min), screen(b.rect.max));
            if on_screen.width() < NAME_ROOM {
                continue;
            }
            let font = FontId::proportional(size(14.0).max(NAME_MIN));
            let room = on_screen.width() - 16.0 * cam.zoom.max(0.5) - if ports { 14.0 * cam.zoom } else { 0.0 };
            let mut job = egui::text::LayoutJob::simple_singleline(b.key.clone(), font, v.strong_text_color());
            job.wrap = egui::text::TextWrapping {
                max_width: room.max(8.0),
                max_rows: 1,
                break_anywhere: true,
                overflow_character: Some('…'),
            };
            let galley = painter.layout_job(job);
            let at = if ports {
                screen(b.rect.min) + Vec2::new(8.0, 5.0) * cam.zoom
            } else {
                Pos2::new(on_screen.left() + 8.0 * cam.zoom.max(0.5), on_screen.center().y - galley.size().y / 2.0)
            };
            painter.with_clip_rect(on_screen.expand(2.0).intersect(rect)).galley(at, galley, v.strong_text_color());
            if let (Some(f), Some(file), true) = (b.foot(), &b.file, ports || cam.zoom >= 0.35) {
                painter.text(
                    screen(Pos2::new(f.left() + 8.0, f.center().y + 0.5)),
                    Align2::LEFT_CENTER,
                    file_label(file, ((f.width() - 16.0) / estimate("x", &file_font())).max(8.0) as usize),
                    FontId::monospace(size(10.5)),
                    faint,
                );
            }
            if !ports {
                continue;
            }
            let top = screen(b.rect.min);
            if !b.source {
                // a node of data's operator in the colour of the type it gives, when asked
                let colour =
                    b.tint().filter(|_| scene.key.4).and_then(|t| type_colour(t, v.dark_mode)).unwrap_or(faint);
                painter.text(
                    top + Vec2::new(8.0, 21.0) * cam.zoom,
                    Align2::LEFT_TOP,
                    &b.label,
                    FontId::monospace(size(11.0)),
                    colour,
                );
            }
            for (k, p) in b.inputs.iter().enumerate() {
                let (name, colour) =
                    if p.name.is_empty() { (NEW_INPUT, faint) } else { (p.name.as_str(), v.text_color()) };
                painter.text(
                    screen(b.input_at(k)) + Vec2::new(9.0, 0.0) * cam.zoom,
                    Align2::LEFT_CENTER,
                    name,
                    FontId::monospace(size(11.0)),
                    colour,
                );
            }
            if let Some(f) = b.fold() {
                painter.text(
                    screen(Pos2::new(f.left() + 9.0, f.center().y)),
                    Align2::LEFT_CENTER,
                    fold_label(b.folded.len(), b.unfolded),
                    FontId::monospace(size(10.5)),
                    faint,
                );
            }
            for (k, (name, _)) in b.outputs.iter().enumerate() {
                if !name.is_empty() {
                    painter.text(
                        screen(b.output_at(k)) - Vec2::new(9.0, 0.0) * cam.zoom,
                        Align2::RIGHT_CENTER,
                        name,
                        FontId::monospace(size(11.0)),
                        v.text_color(),
                    );
                }
            }
        }
    }
    // the marks of the nodes with problems: red for an error, amber for a warning, at the left of the light of
    // their state
    for &i in &in_view {
        let b = &scene.boxes[i];
        let Some(list) = problems.get(&b.key) else { continue };
        let error = list.iter().any(|(e, _)| *e);
        let c = screen(b.rect.right_top() + Vec2::new(-26.0, 10.0));
        let radius = (6.0 * cam.zoom).max(3.5);
        painter.circle_filled(c, radius, if error { v.error_fg_color } else { v.warn_fg_color });
        if radius >= 5.0 {
            painter.text(c, Align2::CENTER_CENTER, "!", FontId::proportional(radius * 1.6), Color32::BLACK);
        }
    }
    // the checks' badges (a container's: the worst of its checks), at the left of the marks of problems
    for &i in &in_view {
        let b = &scene.boxes[i];
        let Some(verdict) = checks.get(&b.key) else { continue };
        let left = if problems.contains_key(&b.key) { 42.0 } else { 26.0 };
        let c = screen(b.rect.right_top() + Vec2::new(-left, 10.0));
        let radius = (6.0 * cam.zoom).max(3.5);
        painter.circle_filled(c, radius, verdict.colour(&v));
        if radius >= 5.0 {
            painter.text(
                c,
                Align2::CENTER_CENTER,
                verdict.symbol(),
                FontId::proportional(radius * 1.4),
                Color32::BLACK,
            );
        }
    }
    // the nodes whose result is frozen: ❄ on their top left corner
    for &i in &in_view {
        let b = &scene.boxes[i];
        if !b.frozen {
            continue;
        }
        let c = screen(b.rect.left_top());
        let radius = (8.0 * cam.zoom).max(4.5);
        painter.circle(c, radius, Color32::from_rgb(170, 215, 245), Stroke::new(1.0, v.window_stroke().color));
        if radius >= 5.0 {
            painter.text(
                c,
                Align2::CENTER_CENTER,
                "❄",
                FontId::proportional(radius * 1.5),
                Color32::from_rgb(20, 70, 120),
            );
        }
    }
    // the nodes that ran once per layer: ×12, at the left of the marks of problems and checks
    if ports {
        for &i in &in_view {
            let b = &scene.boxes[i];
            let Some(n) = layers.get(&b.key) else { continue };
            let marks = problems.contains_key(&b.key) as u8 + checks.contains_key(&b.key) as u8;
            let left = 20.0 + 16.0 * marks as f32;
            painter.text(
                screen(b.rect.right_top() + Vec2::new(-left, 10.0)),
                Align2::RIGHT_CENTER,
                format!("×{n}"),
                FontId::monospace(size(11.0)),
                crate::theme::accent(),
            );
        }
    }
    // zoomed out, the model's stages (each framed and named); the groups' notes
    stages_and_notes(&painter, scene, &screen, view, cam.zoom < STAGES_UNDER, &v);
    // the nodes changed in a review going on, outlined in the colour of their change
    if st.path.is_empty() {
        for (k, colour) in &st.marks {
            if let Some(&i) = scene.index.get(k.as_str())
                && scene.boxes[i].rect.intersects(view)
            {
                let b = &scene.boxes[i];
                let r = Rect::from_min_max(screen(b.rect.min), screen(b.rect.max));
                painter.rect_stroke(r, 4.0 * cam.zoom, Stroke::new(2.5, *colour), egui::StrokeKind::Outside);
            }
        }
    }
    // the nodes the assistant reads or changes now: a halo that widens as it fades
    st.pulse.retain(|(_, _, at)| at.elapsed() < PULSE);
    for (path, k, at) in &st.pulse {
        if *path != st.path {
            continue;
        }
        if let Some(&i) = scene.index.get(k.as_str())
            && scene.boxes[i].rect.intersects(view)
        {
            let gone = at.elapsed().as_secs_f32() / PULSE.as_secs_f32();
            let b = &scene.boxes[i];
            let r = Rect::from_min_max(screen(b.rect.min), screen(b.rect.max)).expand(3.0 + 9.0 * gone);
            let colour = crate::theme::accent().gamma_multiply(1.0 - gone);
            painter.rect_stroke(r, 6.0 * cam.zoom, Stroke::new(3.0, colour), egui::StrokeKind::Outside);
        }
    }
    if !st.pulse.is_empty() {
        ui.ctx().request_repaint();
    }
    // the nodes selected (the one in the inspector more marked), the output shown, the nodes running (a light
    // that breathes)
    for k in st.chosen.iter().chain(selected.map(String::from).as_ref()) {
        if let Some(&i) = scene.index.get(k.as_str())
            && scene.boxes[i].rect.intersects(view)
        {
            let b = &scene.boxes[i];
            let r = Rect::from_min_max(screen(b.rect.min), screen(b.rect.max));
            let width = if selected == Some(k.as_str()) { 2.5 } else { 1.5 };
            painter.rect_stroke(
                r,
                4.0 * cam.zoom,
                Stroke::new(width, crate::theme::accent()),
                egui::StrokeKind::Inside,
            );
        }
    }
    if let Some(r) = boxed {
        let r = Rect::from_min_max(screen(r.min), screen(r.max));
        painter.rect_filled(r, 0.0, crate::theme::accent().gamma_multiply(0.08));
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, crate::theme::accent()), egui::StrokeKind::Inside);
    }
    if let Some((node, port)) = shown
        && let Some(&i) = scene.index.get(node)
        && let Some(k) = scene.boxes[i].outputs.iter().position(|(p, _)| p == port)
    {
        painter.circle_filled(screen(scene.boxes[i].output_at(k)), (PORT * cam.zoom).max(2.0), crate::theme::accent());
    }
    if st.path.is_empty() {
        let running: Vec<&String> = status.iter().filter(|(_, s)| **s == NodeStatus::Running).map(|(k, _)| k).collect();
        if !running.is_empty() {
            let t = ui.input(|i| i.time) as f32;
            ui.ctx().request_repaint();
            for id in running {
                if let Some(&i) = scene.index.get(id.as_str()) {
                    let c = screen(scene.boxes[i].rect.right_top() + Vec2::new(-10.0, 10.0));
                    let glow = crate::theme::accent().gamma_multiply(0.3 + 0.7 * (t * 6.0).sin().abs());
                    painter.circle_filled(c, (6.0 * cam.zoom).max(3.0), glow);
                }
            }
        }
        // its share done, a line along its bottom
        if let Some((id, f)) = &st.progress
            && let Some(&i) = scene.index.get(id.as_str())
        {
            let b = scene.boxes[i].rect;
            let (l, r, y) = (screen(b.left_bottom()), screen(b.right_bottom()), screen(b.left_bottom()).y);
            let h = (3.0 * cam.zoom).max(2.0);
            let full = egui::Rect::from_min_max(egui::pos2(l.x + 2.0, y - h - 2.0), egui::pos2(r.x - 2.0, y - 2.0));
            painter.rect_filled(full, h / 2.0, v.faint_bg_color);
            let done = egui::Rect::from_min_size(full.min, egui::vec2(full.width() * f.clamp(0.0, 1.0), h));
            painter.rect_filled(done, h / 2.0, crate::theme::accent());
        }
    }
    // a connection being drawn
    if let (Some(Drag::Connect(reference, from)), Some(q)) = (&st.drag, pointer) {
        let p0 = screen(*from);
        // in the colour of the output it comes from
        let of = scene.index.get(reference.as_str()).map(|&i| (i, 0)).or_else(|| {
            let (node, port) = reference.split_once('.')?;
            let i = *scene.index.get(node)?;
            Some((i, scene.boxes[i].outputs.iter().position(|(p, _)| p == port)?))
        });
        let colour =
            of.and_then(|(i, k)| type_colour(scene.boxes[i].outputs[k].1, v.dark_mode)).unwrap_or(v.selection.bg_fill);
        let dx = ((q.x - p0.x).abs() * 0.5).max(30.0 * cam.zoom);
        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
            [p0, p0 + Vec2::new(dx, 0.0), q - Vec2::new(dx, 0.0), q],
            false,
            Color32::TRANSPARENT,
            Stroke::new(2.0, colour),
        ));
    }
    // the note of the node under the pointer; how to take a connection off an input
    if st.drag.is_none()
        && let Some(p) = resp.hover_pos()
    {
        match scene.hit(to_world(p, &cam), reach(&cam)) {
            Some(Hit::Head(i)) => {
                let b = &scene.boxes[i];
                let mut text: Vec<String> =
                    problems.get(&b.key).into_iter().flatten().map(|(_, m)| format!("⚠ {m}")).collect();
                if let Some(c) = checks.get(&b.key) {
                    let what = if b.op == "check.assert" { "the check" } else { "the worst of its checks" };
                    text.push(format!("{} {what}: {}", c.symbol(), c.words()));
                }
                if b.frozen {
                    text.push(
                        "❄ its result frozen: kept on disk next to the model and used instead of running it (Freeze \
                         in the Node pane thaws it)"
                            .into(),
                    );
                }
                if let Some(n) = layers.get(&b.key) {
                    text.push(format!(
                        "×{n}: given {n} layers where it takes one map, it ran once per layer (its maps a stack, \
                         its tables one with a column layer)"
                    ));
                }
                text.extend(b.note.clone());
                if !text.is_empty() {
                    resp.clone().on_hover_text_at_pointer(text.join("\n"));
                }
            }
            Some(Hit::Fold(i)) => {
                let b = &scene.boxes[i];
                let text = if b.unfolded {
                    "fold the optional inputs not given".to_string()
                } else {
                    let names: Vec<&str> = b.folded.iter().map(|(n, _)| n.as_str()).collect();
                    format!("optional inputs: {} (a click shows them)", names.join(", "))
                };
                resp.clone().on_hover_text_at_pointer(text);
            }
            Some(Hit::Output(i, k)) => {
                let b = &scene.boxes[i];
                let (name, ty) = &b.outputs[k];
                let name = if name.is_empty() { b.key.as_str() } else { name.as_str() };
                let what = if b.source { "drag it to an input" } else { "a click shows it; drag it to an input" };
                resp.clone().on_hover_text_at_pointer(format!("{name}: {}\n{what}", ty.name()));
            }
            // the file's whole path
            Some(Hit::Body(i)) if scene.boxes[i].foot().is_some_and(|f| f.contains(to_world(p, &cam))) => {
                resp.clone().on_hover_text_at_pointer(scene.boxes[i].file.clone().unwrap_or_default());
            }
            Some(Hit::Input(i, k)) if !scene.boxes[i].inputs[k].name.is_empty() => {
                let p = &scene.boxes[i].inputs[k];
                let need = if p.required { "needed" } else { "optional" };
                let what = if p.connected {
                    "drag the connection away, or right-click, to disconnect it"
                } else {
                    "drop a connection here"
                };
                resp.clone().on_hover_text_at_pointer(format!("{}: {} ({need})\n{what}", p.name, p.ty.name()));
            }
            Some(Hit::Input(i, k)) if scene.boxes[i].inputs[k].name.is_empty() => {
                resp.clone().on_hover_text_at_pointer("drop a connection here (or on the node) for one more input");
            }
            _ => {}
        }
    }
    // a connection let go on a node several of whose inputs fit: the input chosen in a list
    if let Some(pick) = st.picking.clone() {
        let area = egui::Area::new(egui::Id::new("pick an input"))
            .order(egui::Order::Foreground)
            .fixed_pos(pick.at)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(format!("{} into {}'s", pick.reference, pick.node)).strong());
                    for (port, ty) in &pick.ports {
                        let text = egui::RichText::new(format!("{port}  ·  {}", ty.name())).monospace();
                        if ui.add(egui::Button::new(text).frame(false)).clicked() {
                            actions.push(Action::Connect {
                                reference: pick.reference.clone(),
                                node: pick.node.clone(),
                                port: port.clone(),
                            });
                            st.picking = None;
                        }
                    }
                });
            });
        let away = ui.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
        if away || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            st.picking = None;
        }
    }
    // the zoom, in the corner
    painter.text(
        rect.right_bottom() - Vec2::new(8.0, 6.0),
        Align2::RIGHT_BOTTOM,
        format!("{} nodes   {:.0}%", nodes.len(), cam.zoom * 100.0),
        FontId::proportional(11.0),
        v.weak_text_color(),
    );
    if ui.input(|i| i.key_pressed(egui::Key::Home)) && ui.ctx().memory(|m| m.focused().is_none()) {
        st.fit = true;
    }
    actions
}
