//! The features of a vector layer read from a file, edited on the map: their vertices dragged, a vertex added in
//! the middle of an edge or taken away, a feature added or deleted, their attributes typed (a number stays a
//! number, a text a text). The features stay in their own map coordinates (f64, as read: what is not touched is
//! written as it was; a ring with holes, a multipart feature are kept), carried to the shown map's cells only to
//! be drawn and picked. Written as a new GeoPackage beside the file read (never over it), read into the model by a
//! `vector.read` node, as the features drawn are (see [`crate::draw`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Color32, Pos2, Response, Stroke, Ui, Vec2};
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

/// The kinds of vertices lists a geometry has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Points,
    Line,
    Ring,
}

/// A vertex of a feature: the feature, the list of vertices of it (a line, a ring, the points of a multipoint; in
/// a multipolygon the rings of every polygon one after the other), the vertex of the list.
pub type Vtx = (usize, usize, usize);

/// What is under the pointer: a vertex, or the middle of an edge (after its vertex).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Vertex(Vtx),
    Middle(Vtx),
}

/// How many lists of vertices a geometry has.
fn lists(g: &Geometry) -> usize {
    match g {
        Geometry::Empty => 0,
        Geometry::Point(_) | Geometry::MultiPoint(_) | Geometry::LineString(_) => 1,
        Geometry::Polygon(r) | Geometry::MultiLineString(r) => r.len(),
        Geometry::MultiPolygon(m) => m.iter().map(Vec::len).sum(),
    }
}

fn shape(g: &Geometry) -> Shape {
    match g {
        Geometry::Point(_) | Geometry::MultiPoint(_) | Geometry::Empty => Shape::Points,
        Geometry::LineString(_) | Geometry::MultiLineString(_) => Shape::Line,
        Geometry::Polygon(_) | Geometry::MultiPolygon(_) => Shape::Ring,
    }
}

/// The vertices of list `k` of a geometry.
fn list(g: &Geometry, k: usize) -> Option<&[Xy]> {
    match g {
        Geometry::Empty => None,
        Geometry::Point(p) => (k == 0).then(|| std::slice::from_ref(p)),
        Geometry::LineString(l) | Geometry::MultiPoint(l) => (k == 0).then_some(&l[..]),
        Geometry::Polygon(r) | Geometry::MultiLineString(r) => r.get(k).map(|v| &v[..]),
        Geometry::MultiPolygon(m) => {
            let mut k = k;
            for p in m {
                if k < p.len() {
                    return Some(&p[k]);
                }
                k -= p.len();
            }
            None
        }
    }
}

/// The vertices of list `k`, to move (a point is a list of one).
fn list_mut(g: &mut Geometry, k: usize) -> Option<&mut [Xy]> {
    match g {
        Geometry::Point(p) => (k == 0).then(|| std::slice::from_mut(p)),
        _ => vec_mut(g, k).map(|v| &mut v[..]),
    }
}

/// The vertices of list `k` as a vector that can grow and shrink (none for a single point).
fn vec_mut(g: &mut Geometry, k: usize) -> Option<&mut Vec<Xy>> {
    match g {
        Geometry::Empty | Geometry::Point(_) => None,
        Geometry::LineString(l) | Geometry::MultiPoint(l) => (k == 0).then_some(l),
        Geometry::Polygon(r) | Geometry::MultiLineString(r) => r.get_mut(k),
        Geometry::MultiPolygon(m) => {
            let mut k = k;
            for p in m {
                if k < p.len() {
                    return p.get_mut(k);
                }
                k -= p.len();
            }
            None
        }
    }
}

/// A list of vertices taken away from a geometry (a polygon's outer ring takes the polygon): true when nothing is
/// left of the geometry.
fn drop_list(g: &mut Geometry, k: usize) -> bool {
    match g {
        Geometry::Polygon(r) => {
            if k == 0 {
                return true;
            }
            r.remove(k);
            false
        }
        Geometry::MultiLineString(r) => {
            r.remove(k);
            r.is_empty()
        }
        Geometry::MultiPolygon(m) => {
            let mut k = k;
            for i in 0..m.len() {
                if k < m[i].len() {
                    if k == 0 {
                        m.remove(i);
                    } else {
                        m[i].remove(k);
                    }
                    break;
                }
                k -= m[i].len();
            }
            m.is_empty()
        }
        _ => true,
    }
}

/// The vertices of a ring or line that can be moved (a ring's closing vertex is its first).
fn movable(s: Shape, n: usize) -> usize {
    if s == Shape::Ring && n > 1 { n - 1 } else { n }
}

fn mid(a: Xy, b: Xy) -> Xy {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

/// Whether two layers are the same features (a number that is not there equals another).
pub fn same(a: &Vector, b: &Vector) -> bool {
    let col = |x: &Column, y: &Column| match (x, y) {
        (Column::Numbers(p), Column::Numbers(q)) => {
            p.len() == q.len() && p.iter().zip(q).all(|(a, b)| a == b || (a.is_nan() && b.is_nan()))
        }
        (Column::Texts(p), Column::Texts(q)) => p == q,
        _ => false,
    };
    a.geometries == b.geometries
        && a.crs == b.crs
        && a.attributes.len() == b.attributes.len()
        && a.attributes.iter().zip(&b.attributes).all(|((n, x), (m, y))| n == m && col(x, y))
}

/// The features of a layer being edited: what is done on them and can be taken back, the vertex dragged, the
/// feature being added.
pub struct Edit {
    /// The layer edited (its node and output).
    pub node: String,
    pub port: String,
    /// The file the features were read from (never written), and its layer.
    pub source: PathBuf,
    pub layer: Option<String>,
    /// The numeric fields of the file that are integers (written as such).
    pub integers: Vec<String>,
    /// The features as read, and as they are now.
    pub original: Arc<Vector>,
    pub data: Vector,
    /// Where it is written: the file's name, `_edited` and `.gpkg`, in its folder, until changed.
    pub target: String,
    /// A feature being added: its vertices so far (clicks on the map).
    pub adding: bool,
    pub current: Vec<Xy>,
    pub grabbed: Option<Vtx>,
    /// The feature selected (its row in the table).
    pub picked: Option<usize>,
    /// The visibility of the layer before it was edited (hidden while it is: this draws it).
    pub was_visible: bool,
    /// The question asked before the edits are let go.
    pub confirm_stop: bool,
    undo: Vec<Vector>,
    redo: Vec<Vector>,
}

/// Most vertices of a layer edited (more would be slow to draw and to pick on a frame).
pub const MOST: usize = 300_000;

/// How many edits can be taken back.
const DEPTH: usize = 100;

impl Edit {
    pub fn new(
        (node, port): (&str, &str),
        source: &Path,
        layer: Option<String>,
        original: Arc<Vector>,
        integers: Vec<String>,
    ) -> Result<Edit, String> {
        let mut n = 0;
        for g in &original.geometries {
            g.points(&mut |_| n += 1);
        }
        if n > MOST {
            return Err(format!("{n} vertices: too many to edit on the map (at most {MOST})"));
        }
        let stem = source.file_stem().map_or("layer".into(), |s| s.to_string_lossy().into_owned());
        let mut target = source.with_file_name(format!("{stem}_edited.gpkg"));
        let mut k = 2;
        while target.exists() {
            target = source.with_file_name(format!("{stem}_edited_{k}.gpkg"));
            k += 1;
        }
        Ok(Edit {
            node: node.to_string(),
            port: port.to_string(),
            source: source.to_path_buf(),
            layer,
            integers,
            data: (*original).clone(),
            original,
            target: target.to_string_lossy().into_owned(),
            adding: false,
            current: Vec::new(),
            grabbed: None,
            picked: None,
            was_visible: true,
            confirm_stop: false,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    /// Whether the features differ from those read.
    pub fn changed(&self) -> bool {
        !same(&self.data, &self.original)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The state kept before an edit (what a following undo goes back to).
    fn snap(&mut self) {
        if self.undo.len() >= DEPTH {
            self.undo.remove(0);
        }
        self.undo.push(self.data.clone());
        self.redo.clear();
    }

    /// The last edit taken back.
    pub fn undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.data, d));
            self.grabbed = None;
            self.picked = self.picked.filter(|&f| f < self.data.len());
        }
    }

    /// The last edit taken back, done again.
    pub fn redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.data, d));
            self.grabbed = None;
            self.picked = self.picked.filter(|&f| f < self.data.len());
        }
    }

    /// A vertex moved to `to` (a ring's first vertex moves its closing one with it).
    pub fn move_vertex(&mut self, at: Vtx, to: Xy) {
        self.snap();
        self.move_at(at, to);
    }

    fn move_at(&mut self, (f, k, v): Vtx, to: Xy) {
        let Some(g) = self.data.geometries.get_mut(f) else { return };
        let ring = shape(g) == Shape::Ring;
        let Some(l) = list_mut(g, k) else { return };
        if v >= l.len() {
            return;
        }
        l[v] = to;
        if ring && v == 0 {
            let last = l.len() - 1;
            l[last] = to;
        }
    }

    /// A vertex put in the middle of an edge (after vertex `after`), at `at`; its index.
    pub fn insert_vertex(&mut self, after: Vtx, at: Xy) -> Option<usize> {
        self.snap();
        self.insert_at(after, at)
    }

    fn insert_at(&mut self, (f, k, after): Vtx, at: Xy) -> Option<usize> {
        let g = self.data.geometries.get_mut(f)?;
        let l = vec_mut(g, k)?;
        (after < l.len()).then(|| {
            l.insert(after + 1, at);
            after + 1
        })
    }

    /// A vertex taken away; its line, ring or feature too when it would be too small (a line of two vertices, a
    /// ring of three, the only point of a feature).
    pub fn remove_vertex(&mut self, (f, k, v): Vtx) {
        let Some(g) = self.data.geometries.get(f) else { return };
        let Some(l) = list(g, k) else { return };
        let (s, n) = (shape(g), l.len());
        if v >= movable(s, n) {
            return;
        }
        self.snap();
        let few = match s {
            Shape::Points => n <= 1,
            Shape::Line => n <= 2,
            Shape::Ring => n <= 4,
        };
        if few {
            if drop_list(&mut self.data.geometries[f], k) {
                self.drop_feature(f);
            }
        } else if let Some(l) = vec_mut(&mut self.data.geometries[f], k) {
            l.remove(v);
            if s == Shape::Ring && v == 0 {
                let first = l[0];
                *l.last_mut().unwrap() = first;
            }
        }
        self.grabbed = None;
    }

    /// A feature taken away, with its attributes.
    pub fn remove_feature(&mut self, f: usize) {
        if f < self.data.len() {
            self.snap();
            self.drop_feature(f);
        }
    }

    fn drop_feature(&mut self, f: usize) {
        self.data.geometries.remove(f);
        for (_, c) in &mut self.data.attributes {
            match c {
                Column::Numbers(v) => {
                    v.remove(f);
                }
                Column::Texts(v) => {
                    v.remove(f);
                }
            }
        }
        self.grabbed = None;
        self.picked = match self.picked {
            Some(p) if p == f => None,
            Some(p) if p > f => Some(p - 1),
            p => p,
        };
    }

    /// The kind of features the layer has, for those added: its first geometry's shape, and whether it is a
    /// multi form (as one of them is).
    fn kind(&self) -> (Shape, bool) {
        let first = self.data.geometries.iter().find(|g| !matches!(g, Geometry::Empty));
        let multi =
            self.data.geometries.iter().any(|g| {
                matches!(g, Geometry::MultiPoint(_) | Geometry::MultiLineString(_) | Geometry::MultiPolygon(_))
            });
        (first.map_or(Shape::Points, shape), multi)
    }

    /// What a click adds, said in the panel.
    pub fn how(&self) -> &'static str {
        match self.kind().0 {
            Shape::Points => "a click: a point",
            Shape::Line => "a click a vertex; Enter or a double click ends the line",
            Shape::Ring => "a click a vertex; Enter, a double click or a click on the first vertex closes it",
        }
    }

    /// A feature added, its attributes empty (no value).
    pub fn add_feature(&mut self, vertices: &[Xy]) -> bool {
        let (s, multi) = self.kind();
        let mut v = vertices.to_vec();
        v.dedup();
        let g = match s {
            Shape::Points if !v.is_empty() => {
                if multi {
                    Geometry::MultiPoint(vec![v[0]])
                } else {
                    Geometry::Point(v[0])
                }
            }
            Shape::Line if v.len() >= 2 => {
                if multi {
                    Geometry::MultiLineString(vec![v])
                } else {
                    Geometry::LineString(v)
                }
            }
            Shape::Ring if v.len() >= 3 => {
                v.push(v[0]);
                if multi { Geometry::MultiPolygon(vec![vec![v]]) } else { Geometry::Polygon(vec![v]) }
            }
            _ => return false,
        };
        self.snap();
        self.data.geometries.push(g);
        for (_, c) in &mut self.data.attributes {
            match c {
                Column::Numbers(x) => x.push(f64::NAN),
                Column::Texts(x) => x.push(String::new()),
            }
        }
        self.picked = Some(self.data.len() - 1);
        true
    }

    /// A feature being added: a click (a point is added at once; a click on the first vertex closes a ring).
    pub fn click(&mut self, at: Xy, close: bool) {
        match self.kind().0 {
            Shape::Points => {
                self.add_feature(&[at]);
            }
            Shape::Ring if close && self.current.len() >= 3 => self.finish(),
            _ => {
                // (the second click of a double click: the same vertex again)
                if self.current.last() != Some(&at) {
                    self.current.push(at);
                }
            }
        }
    }

    /// The feature being added ended (a line of two vertices or more, a ring of three); else dropped.
    pub fn finish(&mut self) {
        let v = std::mem::take(&mut self.current);
        self.add_feature(&v);
    }

    /// The text of a value of an attribute (nothing for a number that is not there).
    pub fn text(&self, col: usize, f: usize) -> String {
        match self.data.attributes.get(col).map(|(_, c)| c) {
            Some(Column::Numbers(v)) => v.get(f).filter(|x| !x.is_nan()).map(|x| format!("{x}")).unwrap_or_default(),
            Some(Column::Texts(v)) => v.get(f).cloned().unwrap_or_default(),
            None => String::new(),
        }
    }

    /// A value of an attribute typed: a number for a numeric attribute (empty: none; what is not a number is
    /// refused), a text for a text one. Whether it was taken.
    pub fn set_value(&mut self, col: usize, f: usize, text: &str) -> bool {
        let t = text.trim();
        let ok = match self.data.attributes.get(col).map(|(_, c)| c) {
            Some(Column::Numbers(v)) => f < v.len() && (t.is_empty() || t.parse::<f64>().is_ok_and(f64::is_finite)),
            Some(Column::Texts(v)) => f < v.len(),
            None => false,
        };
        if !ok {
            return false;
        }
        match &mut self.data.attributes[col].1 {
            Column::Numbers(v) => v[f] = if t.is_empty() { f64::NAN } else { t.parse().unwrap_or(f64::NAN) },
            Column::Texts(v) => v[f] = text.to_string(),
        }
        true
    }

    /// The vertex, or the middle of an edge, within `reach` of `p` on the screen (vertices first).
    pub fn hit(&self, p: Pos2, reach: f32, screen: &dyn Fn(&Xy) -> Pos2) -> Option<Hit> {
        let near = |c: &Xy| screen(c).distance(p) <= reach;
        for f in (0..self.data.len()).rev() {
            let g = &self.data.geometries[f];
            for k in 0..lists(g) {
                let (Some(l), s) = (list(g, k), shape(g)) else { continue };
                if let Some(v) = (0..movable(s, l.len())).find(|&v| near(&l[v])) {
                    return Some(Hit::Vertex((f, k, v)));
                }
            }
        }
        for f in (0..self.data.len()).rev() {
            let g = &self.data.geometries[f];
            if shape(g) == Shape::Points {
                continue;
            }
            for k in 0..lists(g) {
                let Some(l) = list(g, k) else { continue };
                if let Some(v) = (0..l.len().saturating_sub(1)).find(|&v| near(&mid(l[v], l[v + 1]))) {
                    return Some(Hit::Middle((f, k, v)));
                }
            }
        }
        None
    }

    /// The features written as a GeoPackage at `path` (a layer named as the file), the fields as they were read.
    /// Never the file the features were read from.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let same = path == self.source
            || matches!((path.canonicalize(), self.source.canonicalize()), (Ok(a), Ok(b)) if a == b);
        if same {
            return Err("not over the file the features were read from: choose another name".into());
        }
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("gpkg")) {
            return Err("a GeoPackage (.gpkg)".into());
        }
        if self.data.is_empty() {
            return Err("no feature left".into());
        }
        if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let layer = path.file_stem().map_or("edited".into(), |s| s.to_string_lossy().into_owned());
        terraflux_gdal::write_vector_as(path, &layer, &self.data, &self.integers)
    }
}

/// A grid's cells (column, row) to map coordinates, and back, by its geotransform.
fn to_map(t: &[f64; 6], c: [f64; 2]) -> Xy {
    [t[0] + c[0] * t[1] + c[1] * t[2], t[3] + c[0] * t[4] + c[1] * t[5]]
}

fn to_cells(t: &[f64; 6], p: &Xy) -> [f64; 2] {
    let det = t[1] * t[5] - t[2] * t[4];
    let (dx, dy) = (p[0] - t[0], p[1] - t[3]);
    [(dx * t[5] - dy * t[2]) / det, (dy * t[1] - dx * t[4]) / det]
}

const INK: Color32 = Color32::from_rgb(255, 210, 0);

/// How near a vertex the pointer grabs it, in points.
const REACH: f32 = 7.0;

/// A vertex of a feature dragged (or a new one, from the middle of an edge): true while it is, the map not moved
/// then. Called before the map's own dragging.
pub fn drag(
    e: &mut Edit,
    ui: &Ui,
    response: &Response,
    t: [f64; 6],
    to_cell: impl Fn(Pos2) -> Vec2,
    to_screen: impl Fn(Vec2) -> Pos2,
) -> bool {
    let map = |p: Pos2| {
        let c = to_cell(p);
        to_map(&t, [f64::from(c.x), f64::from(c.y)])
    };
    let screen = |c: &Xy| {
        let c = to_cells(&t, c);
        to_screen(Vec2::new(c[0] as f32, c[1] as f32))
    };
    if response.drag_started()
        && e.current.is_empty()
        && let Some(p) = ui.input(|i| i.pointer.press_origin())
    {
        match e.hit(p, REACH, &screen) {
            Some(Hit::Vertex(v)) => {
                e.snap();
                e.grabbed = Some(v);
            }
            Some(Hit::Middle((f, k, v))) => {
                e.snap();
                let at = {
                    let l = list(&e.data.geometries[f], k).unwrap_or(&[]);
                    mid(l[v], l[v + 1])
                };
                e.grabbed = e.insert_at((f, k, v), at).map(|n| (f, k, n));
            }
            None => e.grabbed = None,
        }
    }
    let Some(v) = e.grabbed else { return false };
    if let Some(p) = response.interact_pointer_pos() {
        e.move_at(v, map(p));
    }
    if response.drag_stopped() || !response.dragged() {
        e.grabbed = None;
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    true
}

/// The clicks and keys over the map while editing, and the features drawn over it (the layer itself is hidden
/// while it is edited): `to_cell` and `to_screen` between the screen and the map's cells, `t` its geotransform.
pub fn interact(
    e: &mut Edit,
    ui: &Ui,
    response: &Response,
    t: [f64; 6],
    to_cell: impl Fn(Pos2) -> Vec2,
    to_screen: impl Fn(Vec2) -> Pos2,
) {
    let map = |p: Pos2| {
        let c = to_cell(p);
        to_map(&t, [f64::from(c.x), f64::from(c.y)])
    };
    let screen = |c: &Xy| {
        let c = to_cells(&t, c);
        to_screen(Vec2::new(c[0] as f32, c[1] as f32))
    };
    let on = |e: &Edit, p: Pos2| if e.current.is_empty() && !e.adding { e.hit(p, REACH, &screen) } else { None };
    if e.adding
        && response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        let close = e.current.first().is_some_and(|f| screen(f).distance(p) < 8.0);
        e.click(map(p), close);
    }
    if response.secondary_clicked()
        && let Some(p) = response.interact_pointer_pos()
        && let Some(Hit::Vertex(v)) = on(e, p)
    {
        e.remove_vertex(v);
    }
    if e.adding && response.double_clicked() {
        e.finish();
    }
    if response.hovered() || !e.current.is_empty() {
        ui.input_mut(|i| {
            if e.adding && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                e.finish();
            }
            if e.adding && i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                if e.current.is_empty() {
                    e.adding = false;
                }
                e.current.clear();
            }
            if e.adding && i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace) {
                e.current.pop();
            }
            if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) {
                e.redo();
            } else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
                e.undo();
            }
        });
    }
    let painter = ui.painter_at(response.rect);
    let view = response.rect.expand(20.0);
    let halo = Stroke::new(4.0, Color32::from_black_alpha(140));
    let line = Stroke::new(2.0, INK);
    let pick = Stroke::new(3.0, crate::theme::accent());
    let mut handles = 0;
    for (f, g) in e.data.geometries.iter().enumerate() {
        let selected = e.picked == Some(f);
        for k in 0..lists(g) {
            let (Some(l), s) = (list(g, k), shape(g)) else { continue };
            let pts: Vec<Pos2> = l.iter().map(&screen).collect();
            if !pts.iter().any(|p| view.contains(*p)) && !(pts.len() > 1 && view_crossed(&view, &pts)) {
                continue;
            }
            match s {
                Shape::Points => {
                    for p in &pts {
                        painter.circle(
                            *p,
                            5.0,
                            if selected { crate::theme::accent() } else { INK },
                            Stroke::new(1.5, Color32::BLACK),
                        );
                    }
                }
                Shape::Line | Shape::Ring => {
                    painter.add(egui::Shape::line(pts.clone(), halo));
                    painter.add(egui::Shape::line(pts.clone(), if selected { pick } else { line }));
                    if e.current.is_empty() && !e.adding && handles < 4000 {
                        for (v, p) in pts[..movable(s, pts.len())].iter().enumerate() {
                            if view.contains(*p) {
                                let big = e.grabbed == Some((f, k, v));
                                painter.circle(
                                    *p,
                                    if big { 5.0 } else { 3.5 },
                                    Color32::WHITE,
                                    Stroke::new(1.5, Color32::BLACK),
                                );
                                handles += 1;
                            }
                        }
                        for w in pts.windows(2) {
                            let m = w[0] + (w[1] - w[0]) / 2.0;
                            if view.contains(m) {
                                painter.circle(m, 2.5, Color32::from_black_alpha(90), Stroke::new(1.0, Color32::WHITE));
                            }
                        }
                    }
                }
            }
        }
    }
    if !e.current.is_empty() {
        let mut pts: Vec<Pos2> = e.current.iter().map(&screen).collect();
        if let Some(h) = response.hover_pos() {
            pts.push(h);
        }
        painter.add(egui::Shape::line(pts.clone(), halo));
        painter.add(egui::Shape::line(pts, Stroke::new(2.0, Color32::WHITE)));
        for v in &e.current {
            painter.circle(screen(v), 3.5, Color32::WHITE, Stroke::new(1.5, Color32::BLACK));
        }
    }
    if let Some(h) = response.hover_pos()
        && e.grabbed.is_none()
    {
        let grab = on(e, h).is_some();
        ui.ctx().set_cursor_icon(if e.adding {
            egui::CursorIcon::Crosshair
        } else if grab {
            egui::CursorIcon::Grab
        } else {
            egui::CursorIcon::Default
        });
    }
}

/// Whether a line through `pts` (all outside `view`) can cross it: its box meets the view's.
fn view_crossed(view: &egui::Rect, pts: &[Pos2]) -> bool {
    let b = egui::Rect::from_points(pts);
    b.intersects(*view)
}

/// What the panel asked for.
pub enum Ask {
    Save,
    Stop,
}

/// The panel of the edit under the Layers: its tools, the attributes of the features in a table, where it is
/// written, and its buttons.
pub fn panel(ui: &mut Ui, e: &mut Edit, name: &str, salt: &str) -> Option<Ask> {
    let mut ask = None;
    ui.label(egui::RichText::new(format!("Editing {name}")).strong());
    ui.label(
        egui::RichText::new(
            "drag a vertex to move it, the dot amid an edge to add one; a right click takes one away (Ctrl+Z undoes)",
        )
        .weak()
        .size(11.0),
    );
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{} features", e.data.len()));
        if ui.add_enabled(e.can_undo(), egui::Button::new("Undo")).clicked() {
            e.undo();
        }
        if ui.add_enabled(e.can_redo(), egui::Button::new("Redo")).clicked() {
            e.redo();
        }
        if ui.add(egui::Button::selectable(e.adding, "Add a feature")).on_hover_text(e.how()).clicked() {
            e.adding = !e.adding;
            e.current.clear();
        }
    });
    let n = e.data.len();
    let row = ui.spacing().interact_size.y + 2.0;
    let cols = e.data.attributes.len();
    if cols > 0 {
        egui::CollapsingHeader::new(format!("Attributes ({cols})"))
            .id_salt(("edit attributes", salt))
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::both().max_height(200.0).id_salt(("edit table", salt)).show_rows(
                    ui,
                    row,
                    n + 1,
                    |ui, range| {
                        for f in range {
                            ui.horizontal(|ui| {
                                // (the header, then a row a feature: its number selects it, × deletes it)
                                if f == 0 {
                                    ui.add_sized(
                                        [46.0, row - 2.0],
                                        egui::Label::new(egui::RichText::new("feature").weak()),
                                    );
                                    for (name, _) in &e.data.attributes {
                                        ui.add_sized(
                                            [84.0, row - 2.0],
                                            egui::Label::new(egui::RichText::new(name).weak()),
                                        );
                                    }
                                    return;
                                }
                                let f = f - 1;
                                let on = e.picked == Some(f);
                                if ui
                                    .add_sized([30.0, row - 2.0], egui::Button::selectable(on, format!("{}", f + 1)))
                                    .clicked()
                                {
                                    e.picked = (!on).then_some(f);
                                }
                                if ui.small_button("×").on_hover_text("this feature deleted").clicked() {
                                    e.remove_feature(f);
                                    return;
                                }
                                for c in 0..cols {
                                    let id = ui.id().with(("edit cell", salt, f, c));
                                    let mut text =
                                        ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| e.text(c, f));
                                    let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(80.0));
                                    if r.gained_focus() {
                                        e.snap();
                                    }
                                    if r.changed() {
                                        e.set_value(c, f, &text);
                                        ui.data_mut(|d| d.insert_temp(id, text));
                                    } else if r.lost_focus() {
                                        ui.data_mut(|d| d.remove::<String>(id));
                                    }
                                }
                            });
                        }
                    },
                );
            });
    }
    ui.horizontal(|ui| {
        ui.label("Save as");
        ui.add(egui::TextEdit::singleline(&mut e.target).desired_width(150.0));
        if ui.button("Browse...").clicked() {
            let p = PathBuf::from(&e.target);
            let mut d =
                rfd::FileDialog::new().set_title("Save the edited features").add_filter("GeoPackage", &["gpkg"]);
            if let Some(dir) = p.parent().filter(|d| d.is_dir()) {
                d = d.set_directory(dir);
            }
            if let Some(name) = p.file_name() {
                d = d.set_file_name(name.to_string_lossy());
            }
            if let Some(p) = d.save_file() {
                e.target = p.to_string_lossy().into_owned();
            }
        }
    });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!e.target.trim().is_empty(), egui::Button::new("Save the edited features"))
            .on_hover_text("written as a new GeoPackage, read by a vector.read node; the file read is not touched")
            .clicked()
        {
            ask = Some(Ask::Save);
        }
        if ui.button("Stop").on_hover_text("stop editing; what is not saved is lost").clicked() {
            if e.changed() {
                e.confirm_stop = true;
            } else {
                ask = Some(Ask::Stop);
            }
        }
    });
    if e.confirm_stop {
        ui.horizontal(|ui| {
            ui.label("The edits are lost.");
            if ui.button("Discard them").clicked() {
                ask = Some(Ask::Stop);
            }
            if ui.button("Keep editing").clicked() {
                e.confirm_stop = false;
            }
        });
    }
    ask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Vec<Xy> {
        vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s], [x, y]]
    }

    /// Features of every kind: a polygon with a hole, a multipolygon, a line, a point; a number, an integer one
    /// and a text each.
    fn layer() -> Vector {
        Vector {
            geometries: vec![
                Geometry::Polygon(vec![square(0.0, 0.0, 100.0), square(40.0, 40.0, 20.0)]),
                Geometry::MultiPolygon(vec![vec![square(200.0, 0.0, 10.0)], vec![square(300.0, 0.0, 10.0)]]),
                Geometry::LineString(vec![[0.0, 500.0], [10.0, 510.0], [20.0, 500.0]]),
                Geometry::Point([1.5, 2.5]),
            ],
            attributes: vec![
                ("area".into(), Column::Numbers(vec![12.5, 3.0, f64::NAN, 1e-7])),
                ("n".into(), Column::Numbers(vec![1.0, 2.0, 3.0, 4.0])),
                ("use".into(), Column::Texts(vec!["forest".into(), "pasture".into(), "".into(), "tree".into()])),
            ],
            crs: terraflux_gdal::crs_wkt("EPSG:32723").ok(),
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("terraflux-vector-edit-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    fn edit_of(v: Vector, source: &Path) -> Edit {
        Edit::new(("roads", "vector"), source, None, Arc::new(v), vec!["n".into()]).unwrap()
    }

    #[test]
    fn written_unedited_it_is_what_was_read() {
        let (src, out) = (tmp("plain.gpkg"), tmp("plain_edited.gpkg"));
        terraflux_gdal::write_vector_as(&src, "plain", &layer(), &["n".into()]).unwrap();
        let read = terraflux_gdal::read_vector(&src, None).unwrap();
        let ints = terraflux_gdal::integer_fields(&src, None).unwrap();
        assert_eq!(ints, ["n"]);
        let e = Edit::new(("roads", "vector"), &src, None, Arc::new(read.clone()), ints).unwrap();
        assert!(!e.changed());
        let before = std::fs::read(&src).unwrap();
        e.save(&out).unwrap();
        let back = terraflux_gdal::read_vector(&out, None).unwrap();
        // the hole, the two parts, the numbers and texts, the coordinate system
        assert_eq!(back.geometries, read.geometries);
        assert_eq!(back.attributes[1], ("n".to_string(), Column::Numbers(vec![1.0, 2.0, 3.0, 4.0])));
        assert_eq!(back.column("use"), read.column("use"));
        assert_eq!(back.column("area").map(|c| c.numbers()[3]), Some(1e-7));
        assert!(back.column("area").unwrap().numbers()[2].is_nan());
        assert_eq!(back.crs.is_some(), read.crs.is_some());
        assert!(terraflux_gdal::same_crs(back.crs.as_ref().unwrap(), read.crs.as_ref().unwrap()).unwrap());
        // the field n still an integer
        assert_eq!(terraflux_gdal::integer_fields(&out, None).unwrap(), ["n"]);
        // never over the file read
        assert!(e.save(&src).is_err());
        assert_eq!(std::fs::read(&src).unwrap(), before);
    }

    #[test]
    fn vertices_moved_added_and_taken_away_then_written() {
        let (src, out) = (tmp("edit.gpkg"), tmp("edit_edited.gpkg"));
        terraflux_gdal::write_vector_as(&src, "edit", &layer(), &["n".into()]).unwrap();
        let read = terraflux_gdal::read_vector(&src, None).unwrap();
        let mut e = edit_of(read.clone(), &src);
        // a corner of the hole moved: only it (the outer ring and the parts are as they were)
        e.move_vertex((0, 1, 2), [65.0, 65.0]);
        // the first vertex of the second part of the multipolygon (its list 3: the rings count on): the closing
        // vertex goes with it
        e.move_vertex((1, 1, 0), [301.0, 1.0]);
        // a vertex in the middle of the line's first edge
        assert_eq!(e.insert_vertex((2, 0, 0), [5.0, 505.0]), Some(1));
        // the point's feature deleted, its values with it
        e.remove_feature(3);
        assert!(e.changed());
        let Geometry::Polygon(r) = &e.data.geometries[0] else { panic!() };
        assert_eq!((r[0].clone(), r[1][2]), (square(0.0, 0.0, 100.0), [65.0, 65.0]));
        let Geometry::MultiPolygon(m) = &e.data.geometries[1] else { panic!() };
        assert_eq!((m[1][0][0], m[1][0][4]), ([301.0, 1.0], [301.0, 1.0]));
        assert_eq!(m[0][0], square(200.0, 0.0, 10.0));
        e.save(&out).unwrap();
        let back = terraflux_gdal::read_vector(&out, None).unwrap();
        assert_eq!(back.geometries, e.data.geometries);
        assert_eq!(back.len(), 3);
        assert_eq!(
            back.geometries[2],
            Geometry::LineString(vec![[0.0, 500.0], [5.0, 505.0], [10.0, 510.0], [20.0, 500.0]])
        );
        assert_eq!(back.column("use"), Some(&Column::Texts(vec!["forest".into(), "pasture".into(), "".into()])));
        assert_eq!(back.column("n"), Some(&Column::Numbers(vec![1.0, 2.0, 3.0])));
        // the file read is as it was
        assert!(same(&terraflux_gdal::read_vector(&src, None).unwrap(), &read));
        // everything undone: as read, and redone
        while e.can_undo() {
            e.undo();
        }
        assert!(!e.changed());
        assert!(same(&e.data, &read));
        while e.can_redo() {
            e.redo();
        }
        assert_eq!(e.data.geometries, back.geometries);
    }

    #[test]
    fn vertices_and_features_taken_away_by_size() {
        let src = tmp("small.gpkg");
        let mut e = edit_of(layer(), &src);
        // a vertex of the line: two are left; then the line, the whole feature, goes
        e.remove_vertex((2, 0, 1));
        assert_eq!(e.data.geometries[2], Geometry::LineString(vec![[0.0, 500.0], [20.0, 500.0]]));
        e.remove_vertex((2, 0, 0));
        assert_eq!(e.data.len(), 3);
        // the hole's: down to a triangle, then the hole goes and the polygon stays
        e.remove_vertex((0, 1, 0));
        let Geometry::Polygon(r) = &e.data.geometries[0] else { panic!() };
        assert_eq!((r.len(), r[1].len(), r[1][0], r[1][3]), (2, 4, [60.0, 40.0], [60.0, 40.0]));
        e.remove_vertex((0, 1, 1));
        let Geometry::Polygon(r) = &e.data.geometries[0] else { panic!() };
        assert_eq!(r.len(), 1);
        // a part of the multipolygon goes when too small (the other stays); the last one takes the feature
        e.remove_vertex((1, 0, 0));
        e.remove_vertex((1, 0, 0));
        let Geometry::MultiPolygon(m) = &e.data.geometries[1] else { panic!() };
        assert_eq!((m.len(), m[0][0].clone()), (1, square(300.0, 0.0, 10.0)));
        e.remove_vertex((1, 0, 0));
        e.remove_vertex((1, 0, 0));
        assert_eq!(e.data.len(), 2);
        // the attributes follow the features gone
        for (_, c) in &e.data.attributes {
            assert_eq!(c.len(), e.data.len());
        }
    }

    #[test]
    fn a_feature_added_with_its_attributes_empty_then_typed() {
        let src = tmp("add.gpkg");
        let mut e = edit_of(layer(), &src);
        // the layer's first geometry is a polygon: three clicks and a closing one make one (not a multi: one is)
        for v in [[400.0, 0.0], [410.0, 0.0], [410.0, 10.0], [400.0, 0.0]] {
            let close = e.current.first() == Some(&v);
            e.click(v, close);
        }
        assert_eq!(e.data.len(), 5);
        assert!(matches!(e.data.geometries[4], Geometry::MultiPolygon(_)));
        assert_eq!(e.text(0, 4), "");
        // numbers stay numbers: a text is refused for a number, the empty one is no value
        assert!(!e.set_value(0, 4, "abc"));
        assert!(e.set_value(0, 4, " 7.25 "));
        assert!(e.set_value(2, 4, "bush"));
        assert_eq!(e.data.column("area").unwrap().numbers()[4], 7.25);
        assert_eq!(e.text(2, 4), "bush");
        assert!(e.set_value(0, 4, ""));
        assert!(e.data.column("area").unwrap().numbers()[4].is_nan());
        // two vertices are no polygon
        e.click([1.0, 1.0], false);
        e.click([2.0, 2.0], false);
        e.finish();
        assert_eq!(e.data.len(), 5);
        // too many vertices to edit are refused
        let mut big = layer();
        big.geometries = vec![Geometry::LineString((0..MOST + 1).map(|i| [i as f64, 0.0]).collect())];
        big.attributes.clear();
        assert!(Edit::new(("a", "b"), &src, None, Arc::new(big), vec![]).is_err());
    }

    #[test]
    fn the_pointer_finds_vertices_and_edges_on_the_screen() {
        let src = tmp("hit.gpkg");
        let e = edit_of(layer(), &src);
        // (a point a unit of the map)
        let screen = |c: &Xy| Pos2::new(c[0] as f32, c[1] as f32);
        assert_eq!(e.hit(Pos2::new(100.5, 0.5), 2.0, &screen), Some(Hit::Vertex((0, 0, 1))));
        assert_eq!(e.hit(Pos2::new(50.0, 0.5), 2.0, &screen), Some(Hit::Middle((0, 0, 0))));
        assert_eq!(e.hit(Pos2::new(1.5, 2.5), 2.0, &screen), Some(Hit::Vertex((3, 0, 0))));
        assert_eq!(e.hit(Pos2::new(900.0, 900.0), 2.0, &screen), None);
        // a geotransform's cells and back
        let t = [500000.0, 30.0, 0.0, 9000000.0, 0.0, -30.0];
        let p = [500300.0, 8999400.0];
        assert_eq!(to_cells(&t, &p), [10.0, 20.0]);
        assert_eq!(to_map(&t, [10.0, 20.0]), p);
    }
}
