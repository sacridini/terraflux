//! The node editor's canvas: the nodes of a scope with their ports (inputs on the left, outputs on the right),
//! what the scope may read on the left (the model's inputs, a container's values), the connections. Drag a
//! node to move it, drag from an output to an input to connect them, click a node to edit it in the side
//! panel, double-click a container to edit its body (another node: its map in a tab of its own), click an output
//! to see it. Drag a connection off an input to take it away (onto another input: moved there), or right-click
//! the input. Drag the background to select the nodes in a box (shift or ctrl and a click add a node or take it
//! away, ctrl+A selects all, Esc none): dragging one of them moves them all. Drag with the middle or the right
//! button to move the view, the wheel zooms around the pointer, Home fits the graph. Each node shows how it did
//! in the last run: running, done (or taken from the cache), failed. A box that reads or writes a file names it at
//! its bottom; the boxes of data (the model's inputs, what reads, writes, stacks or plots) may have a header in the
//! colour of the type they give (a map's blue, vectors' pink...), an option of the settings.
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

pub const WIDTH: f32 = 190.0;
const HEAD: f32 = 38.0;
const ROW: f32 = 18.0;
const PORT: f32 = 5.0;
/// The row at the bottom of a box that names the file it reads or writes.
const FOOT: f32 = 20.0;
/// The side of a cell of the grid that finds the boxes under a point, graph units.
const CELL: f32 = 256.0;
/// Where the column of what a scope reads begins.
const SOURCES_X: f32 = -(WIDTH + 80.0);
/// The zooms from which the nodes' names, then their ports' names and operators, are written.
const NAMES_FROM: f32 = 0.35;
const PORTS_FROM: f32 = 0.6;

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
}

impl EditorState {
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

    /// Where a node's output port is on the screen.
    pub fn screen_of_output(&self, node: &str, port: &str) -> Option<Pos2> {
        let s = self.scene.as_ref()?;
        let b = &s.boxes[*s.index.get(node)?];
        self.on_screen(b.output_at(b.outputs.iter().position(|(p, _)| p == port)?))
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
    /// graph units
    rect: Rect,
    inputs: Vec<Port>,
    outputs: Vec<(String, Type)>,
    source: bool,
    container: bool,
    note: Option<String>,
    /// The file it reads or writes (a node's `path`, a model input's), named in the row at its bottom.
    file: Option<String>,
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
    Head(usize),
    Body(usize),
}

/// A connection: from a box's output to a box's input.
struct EdgeAt {
    from: (usize, usize),
    to: (usize, usize),
}

/// A scope laid out: its boxes, a grid of them, and the shapes the GPU draws (each box's rectangles at
/// `ranges[box]`, the connections touching each box in `touching[box]`, to move a box alone).
struct Scene {
    key: (u64, Vec<String>, u64, &'static str, bool),
    boxes: Vec<BoxAt>,
    index: HashMap<String, usize>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    bounds: Rect,
    shapes: Arc<Shapes>,
    ranges: Vec<std::ops::Range<usize>>,
    edges: Vec<EdgeAt>,
    touching: Vec<Vec<usize>>,
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

/// Where a scope's nodes go when they have no place yet: a column per depth (the longest chain of nodes they
/// read), in time proportional to the nodes and connections.
pub fn auto_positions(nodes: &[Node]) -> HashMap<String, [f32; 2]> {
    let ids: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let deps: Vec<Vec<usize>> = nodes.iter().map(|n| reads(n, &ids)).collect();
    // depths by a walk from each node (a cycle counts once)
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
    let mut column: HashMap<usize, f32> = HashMap::new();
    let mut out = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        let d = depth[i].unwrap_or(0);
        let y = column.entry(d).or_insert(0.0);
        out.insert(n.id.clone(), [d as f32 * (WIDTH + 60.0), *y]);
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
    let first = auto_positions(nodes);
    let depth: Vec<usize> = nodes.iter().map(|n| (first[&n.id][0] / (WIDTH + 60.0)).round() as usize).collect();
    let columns = depth.iter().max().map_or(0, |d| d + 1);
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
            let rows = editor::input_ports(n, r).len().max(editor::output_ports(n, r).len()).max(1);
            out.insert(n.id.clone(), [d as f32 * (WIDTH + 90.0), y]);
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
    fn build(
        model: &Model,
        nodes: &[Node],
        r: &Registry,
        path: &[String],
        status: &BTreeStatus,
        v: &egui::Visuals,
        colours: bool,
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
            let rect = Rect::from_min_size(Pos2::new(SOURCES_X, y), Vec2::new(WIDTH, HEAD + ROW + foot));
            boxes.push(BoxAt {
                key: s.clone(),
                op: String::new(),
                rect,
                inputs: Vec::new(),
                outputs: vec![(String::new(), *t)],
                source: true,
                container: false,
                note: None,
                file,
            });
            y += HEAD + ROW + foot + 12.0;
        }
        let missing = nodes.iter().any(|n| n.pos.is_none());
        let auto = if missing { auto_positions(nodes) } else { HashMap::new() };
        for n in nodes {
            let p = n.pos.unwrap_or_else(|| auto[&n.id]);
            let mut inputs: Vec<Port> = editor::input_ports(n, r)
                .into_iter()
                .map(|(name, ty, required)| Port { connected: n.inputs.contains_key(&name), name, ty, required })
                .collect();
            if let Some(ty) = r.sig(&n.op).and_then(|s| s.extra) {
                inputs.push(Port { name: String::new(), ty, required: false, connected: false });
            }
            let outputs = editor::output_ports(n, r);
            let rows = inputs.len().max(outputs.len()).max(1);
            // the file it reads or writes
            let file = n.params.get("path").and_then(|v| v.as_str()).filter(|p| !p.trim().is_empty()).map(String::from);
            let foot = if file.is_some() { FOOT } else { 0.0 };
            let rect =
                Rect::from_min_size(Pos2::new(p[0], p[1]), Vec2::new(WIDTH, HEAD + rows as f32 * ROW + 6.0 + foot));
            boxes.push(BoxAt {
                key: n.id.clone(),
                op: n.op.clone(),
                rect,
                inputs,
                outputs,
                source: false,
                container: n.body.is_some(),
                note: n.note.clone(),
                file,
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
        let mut rects = Vec::with_capacity(boxes.len() * 6);
        let mut ranges = Vec::with_capacity(boxes.len());
        let stroke = v.widgets.noninteractive.bg_stroke.color;
        for b in &boxes {
            let first = rects.len();
            let state = if path.is_empty() { status.get(&b.key).copied() } else { None };
            let fill = if b.source { v.faint_bg_color } else { v.widgets.inactive.bg_fill };
            let (border, width) =
                if state == Some(NodeStatus::Failed) { (v.error_fg_color, 1.5) } else { (stroke, 1.0) };
            rects.push(RectInstance::new(b.rect.min.into(), b.rect.size().into(), 4.0, fill, border, width));
            // a node of data's header in the colour of the type it gives (a map's blue, vectors' pink...), when
            // asked: what flows from it at a glance
            if let Some(c) = b.tint().filter(|_| colours).and_then(|t| type_colour(t, v.dark_mode)) {
                let tint = fill.lerp_to_gamma(c, if v.dark_mode { 0.22 } else { 0.18 });
                let head = b.head().shrink(2.0);
                rects.push(RectInstance::new(
                    head.min.into(),
                    head.size().into(),
                    3.0,
                    tint,
                    Color32::TRANSPARENT,
                    0.0,
                ));
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
            ranges.push(first..rects.len());
        }
        Scene {
            key: (0, Vec::new(), 0, "", false),
            boxes,
            index,
            grid,
            bounds,
            shapes: Arc::new(Shapes { generation: 0, rects, edges }),
            ranges,
            edges: edges_at,
            touching,
        }
    }

    /// Move a box to a corner (graph units): its rectangles, its connections and its place in the grid, the rest
    /// as it is (a node dragged costs the connections it has, not the graph).
    fn move_box(&mut self, i: usize, min: Pos2, generation: u64) {
        let d = min - self.boxes[i].rect.min;
        if d == Vec2::ZERO {
            return;
        }
        for c in cells_of(self.boxes[i].rect) {
            if let Some(v) = self.grid.get_mut(&c) {
                v.retain(|&j| j != i);
            }
        }
        self.boxes[i].rect = self.boxes[i].rect.translate(d);
        for c in cells_of(self.boxes[i].rect) {
            let v = self.grid.entry(c).or_default();
            // the grid keeps the boxes in their order (the last drawn on top)
            let at = v.partition_point(|&j| j < i);
            v.insert(at, i);
        }
        self.bounds = self.bounds.union(self.boxes[i].rect);
        let shapes = Arc::make_mut(&mut self.shapes);
        for r in &mut shapes.rects[self.ranges[i].clone()] {
            r.min[0] += d.x;
            r.min[1] += d.y;
        }
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
            if b.head().contains(p) {
                return Some(Hit::Head(i));
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
) -> Vec<Action> {
    let mut actions = Vec::new();
    let Some(nodes) = model.scope(&st.path) else {
        st.path.clear();
        return actions;
    };
    // (built again when the model, the place in it, the nodes' states or the theme change)
    let colours = crate::settings::get().node_colours;
    let key = (revision, st.path.clone(), status_rev, crate::theme::current().name, colours);
    if st.scene.as_ref().is_none_or(|s| s.key != key) {
        let mut scene = Scene::build(model, nodes, r, &st.path, status, ui.visuals(), colours);
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
            Some(Hit::Head(i) | Hit::Body(i)) if !scene.boxes[i].source => {
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
            Some(Hit::Head(i) | Hit::Body(i) | Hit::Input(i, _)) if !scene.boxes[i].source => {
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
    if resp.double_clicked()
        && let Some(w) = at
        && let Some(Hit::Head(i) | Hit::Body(i)) = scene.hit(w, reach(cam))
        && !scene.boxes[i].source
    {
        let b = &scene.boxes[i];
        actions.push(if b.container { Action::Enter(b.key.clone()) } else { Action::Open(b.key.clone()) });
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
    let scene = st.scene.as_ref().unwrap();
    let screen = |p: Pos2| rect.min + cam.pan + p.to_vec2() * cam.zoom;
    // the GPU draws the boxes, ports and connections
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        Canvas { shapes: scene.shapes.clone(), size: rect.size().into(), origin: cam.pan.into(), zoom: cam.zoom },
    ));
    let painter = ui.painter_at(rect);
    let v = ui.visuals().clone();
    // the texts of the boxes in view, when they can be read
    let view = Rect::from_min_max(to_world(rect.min, &cam), to_world(rect.max, &cam));
    if cam.zoom >= NAMES_FROM {
        let size = |s: f32| ((s * cam.zoom) * 2.0).round() / 2.0;
        for i in scene.within(view) {
            let b = &scene.boxes[i];
            let top = screen(b.rect.min);
            painter.text(
                top + Vec2::new(8.0, 5.0) * cam.zoom,
                Align2::LEFT_TOP,
                &b.key,
                FontId::proportional(size(14.0)),
                v.strong_text_color(),
            );
            if let (Some(f), Some(file)) = (b.foot(), &b.file) {
                painter.text(
                    screen(Pos2::new(f.left() + 8.0, f.center().y + 0.5)),
                    Align2::LEFT_CENTER,
                    file_label(file, 27),
                    FontId::monospace(size(10.5)),
                    v.weak_text_color(),
                );
            }
            if cam.zoom < PORTS_FROM {
                continue;
            }
            if !b.source {
                // a node of data's operator in the colour of the type it gives, when asked
                let colour = b
                    .tint()
                    .filter(|_| scene.key.4)
                    .and_then(|t| type_colour(t, v.dark_mode))
                    .unwrap_or(v.weak_text_color());
                painter.text(
                    top + Vec2::new(8.0, 21.0) * cam.zoom,
                    Align2::LEFT_TOP,
                    &b.op,
                    FontId::monospace(size(11.0)),
                    colour,
                );
            }
            for (k, p) in b.inputs.iter().enumerate() {
                let (name, colour) = if p.name.is_empty() {
                    ("+ new input", v.weak_text_color())
                } else {
                    (p.name.as_str(), v.text_color())
                };
                painter.text(
                    screen(b.input_at(k)) + Vec2::new(9.0, 0.0) * cam.zoom,
                    Align2::LEFT_CENTER,
                    name,
                    FontId::monospace(size(11.0)),
                    colour,
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
    // the nodes selected (the one in the inspector more marked), the output shown, the nodes running (a light
    // that breathes)
    for k in st.chosen.iter().chain(selected.map(String::from).as_ref()) {
        if let Some(&i) = scene.index.get(k.as_str())
            && scene.boxes[i].rect.intersects(view)
        {
            let b = &scene.boxes[i];
            let r = Rect::from_min_max(screen(b.rect.min), screen(b.rect.max));
            let width = if selected == Some(k.as_str()) { 2.5 } else { 1.5 };
            painter.rect_stroke(r, 4.0 * cam.zoom, Stroke::new(width, v.selection.bg_fill), egui::StrokeKind::Inside);
        }
    }
    if let Some(r) = boxed {
        let r = Rect::from_min_max(screen(r.min), screen(r.max));
        painter.rect_filled(r, 0.0, v.selection.bg_fill.gamma_multiply(0.08));
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, v.selection.bg_fill), egui::StrokeKind::Inside);
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
                if let Some(note) = &scene.boxes[i].note {
                    resp.clone().on_hover_text_at_pointer(note);
                }
            }
            // the file's whole path
            Some(Hit::Body(i)) if scene.boxes[i].foot().is_some_and(|f| f.contains(to_world(p, &cam))) => {
                resp.clone().on_hover_text_at_pointer(scene.boxes[i].file.clone().unwrap_or_default());
            }
            Some(Hit::Input(i, k)) if scene.boxes[i].inputs[k].connected => {
                resp.clone().on_hover_text_at_pointer("drag the connection away, or right-click, to disconnect it");
            }
            Some(Hit::Input(i, k)) if scene.boxes[i].inputs[k].name.is_empty() => {
                resp.clone().on_hover_text_at_pointer("drop a connection here (or on the node) for one more input");
            }
            _ => {}
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
