//! A map of another coordinate system drawn on the view's grid: each tile of the map is a mesh of 16 x 16 quads
//! whose vertices are the tile's cell corners carried to the view's cells (transformed once, kept), and the GPU
//! does the rest, the texture stretched across the quads. Nothing is warped on the processor: the workers make the
//! tiles of the map in its own cells, as for any layer. The pointer's cell is carried back the same way.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use egui::epaint::Vertex;
use egui::{Color32, Mesh, Pos2, Rect, TextureId, Vec2};
use terraflux_raster::Grid;

use crate::map_view::{Place, TILE};

/// A tile's quads along each side.
const STEPS: usize = 16;
/// The tiles' vertices kept.
const KEPT: usize = 2048;

/// The coordinate systems of a grid and of the one it is drawn on, when they differ and neither grid is rotated
/// (a map with no coordinate system, or a rotated grid, cannot be carried).
pub fn crs_pair(g: &Grid, base: &Grid) -> Option<(String, String)> {
    let (t, b) = (g.transform, base.transform);
    if t[2] != 0.0 || t[4] != 0.0 || b[2] != 0.0 || b[4] != 0.0 {
        return None;
    }
    let (x, y) = (crate::map_view::crs_of(g)?, crate::map_view::crs_of(base)?);
    (x != y && !terraflux_gdal::same_crs(&x, &y).unwrap_or(false)).then_some((x, y))
}

/// What tells a layer's grid on a view's grid apart from another: their cells, places and systems.
pub fn key(g: &Grid, base: &Grid) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut k = std::collections::hash_map::DefaultHasher::new();
    for grid in [g, base] {
        grid.transform.iter().for_each(|v| v.to_bits().hash(&mut k));
        (grid.width, grid.height).hash(&mut k);
        crate::map_view::crs_of(grid).hash(&mut k);
    }
    k.finish()
}

/// A tile's vertices as cells of the view.
type Vertices = Arc<Vec<[f32; 2]>>;

/// The corners of the view (as the bits of their cells) and the box of the layer's cells they stand for.
type Sight = ([u32; 4], Option<[f32; 4]>);

/// A layer's cells carried to the view's: the transformations between the two systems (made once), and the
/// vertices of the tiles seen.
pub struct Reproj {
    key: u64,
    src: [f64; 6],
    view: [f64; 6],
    to_view: terraflux_gdal::Transform,
    to_src: terraflux_gdal::Transform,
    /// View cells per cell of the layer, at the layer's middle.
    pub scale: f32,
    /// A tile's (STEPS + 1)^2 vertices as cells of the view (NaN where the view's system has no place for them).
    vertices: HashMap<(u32, usize, usize), Vertices>,
    /// The view's corners (cells of the view) last asked about, and the box of the layer's cells they stand for.
    sight: Option<Sight>,
}

impl Reproj {
    /// The layer `g` on the view's grid `base`, when the two systems are told and can be carried one to the other.
    pub fn new(g: &Grid, base: &Grid) -> Result<Reproj, String> {
        let (from, to) = crs_pair(g, base).ok_or("no coordinate systems to carry between")?;
        let (to_view, to_src) =
            (terraflux_gdal::Transform::new(&from, &to)?, terraflux_gdal::Transform::new(&to, &from)?);
        let mut re = Reproj {
            key: key(g, base),
            src: g.transform,
            view: base.transform,
            to_view,
            to_src,
            scale: 1.0,
            vertices: HashMap::new(),
            sight: None,
        };
        // the layer's middle cell and the next ones along its axes: the size of a cell in the view's
        let (cx, cy) = (g.width as f64 / 2.0, g.height as f64 / 2.0);
        let v = re.carry(&[[cx, cy], [cx + 1.0, cy], [cx, cy + 1.0]]);
        let d = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
        let s = (d(v[0], v[1]) + d(v[0], v[2])) / 2.0;
        if !s.is_finite() || s <= 0.0 {
            return Err("the map has no place in this map's coordinate system".into());
        }
        re.scale = s;
        Ok(re)
    }

    /// Whether it carries the layer's grid to the view's, as they are now.
    pub fn is_for(&self, g: &Grid, base: &Grid) -> bool {
        self.key == key(g, base)
    }

    /// Cells of the layer as cells of the view (NaN where they have no place there).
    pub fn carry(&self, cells: &[[f64; 2]]) -> Vec<[f32; 2]> {
        let t = self.src;
        let mut xs: Vec<f64> = cells.iter().map(|c| t[0] + c[0] * t[1]).collect();
        let mut ys: Vec<f64> = cells.iter().map(|c| t[3] + c[1] * t[5]).collect();
        self.to_view.points(&mut xs, &mut ys);
        let v = self.view;
        xs.iter().zip(&ys).map(|(x, y)| [((x - v[0]) / v[1]) as f32, ((y - v[3]) / v[5]) as f32]).collect()
    }

    /// Cells of the view as cells of the layer (NaN where they have no place there).
    pub fn to_source_many(&self, cells: &[[f64; 2]]) -> Vec<[f64; 2]> {
        let v = self.view;
        let mut xs: Vec<f64> = cells.iter().map(|c| v[0] + c[0] * v[1]).collect();
        let mut ys: Vec<f64> = cells.iter().map(|c| v[3] + c[1] * v[5]).collect();
        self.to_src.points(&mut xs, &mut ys);
        let t = self.src;
        xs.iter().zip(&ys).map(|(x, y)| [(x - t[0]) / t[1], (y - t[3]) / t[5]]).collect()
    }

    /// A cell of the view as a cell of the layer (none where it has no place there).
    pub fn to_source(&self, c: Vec2) -> Option<Vec2> {
        let p = self.to_source_many(&[[f64::from(c.x), f64::from(c.y)]])[0];
        (p[0].is_finite() && p[1].is_finite()).then(|| Vec2::new(p[0] as f32, p[1] as f32))
    }

    /// View cells per cell of the layer around the layer's cell `c` (the layer's middle's where `c` has no place).
    pub fn scale_at(&self, c: Vec2) -> f32 {
        let (x, y) = (f64::from(c.x), f64::from(c.y));
        let v = self.carry(&[[x, y], [x + 1.0, y], [x, y + 1.0]]);
        let d = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
        let s = (d(v[0], v[1]) + d(v[0], v[2])) / 2.0;
        if s.is_finite() && s > 0.0 { s } else { self.scale }
    }

    /// The box (`[x0, y0, x1, y1]`, cells of the view) a box of the layer's cells covers on the view: its edges
    /// walked and carried (the corners alone miss a curved edge); none when no point has a place there.
    pub fn box_on_view(&self, b: [f64; 4]) -> Option<[f32; 4]> {
        let n = 16;
        let mut cells = Vec::new();
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let (x, y) = (b[0] + (b[2] - b[0]) * t, b[1] + (b[3] - b[1]) * t);
            cells.extend([[x, b[1]], [x, b[3]], [b[0], y], [b[2], y]]);
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for p in self.carry(&cells).iter().filter(|p| p[0].is_finite() && p[1].is_finite()) {
            x0 = x0.min(p[0]);
            x1 = x1.max(p[0]);
            y0 = y0.min(p[1]);
            y1 = y1.max(p[1]);
        }
        (x1 >= x0).then_some([x0, y0, x1, y1])
    }

    /// The box of the layer's cells (`[x0, y0, x1, y1]`) under the view whose corners are the cells `a` and `b` of
    /// the view, a little wider than the samples found; none when none has a place. Kept while the corners stay.
    pub fn seen(&mut self, a: Vec2, b: Vec2) -> Option<[f32; 4]> {
        let key = [a.x, a.y, b.x, b.y].map(f32::to_bits);
        if let Some((k, s)) = &self.sight
            && *k == key
        {
            return *s;
        }
        let n = 8;
        let mut cells = Vec::new();
        for i in 0..=n {
            for j in 0..=n {
                let (u, v) = (j as f64 / n as f64, i as f64 / n as f64);
                cells.push([f64::from(a.x) + f64::from(b.x - a.x) * u, f64::from(a.y) + f64::from(b.y - a.y) * v]);
            }
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in self.to_source_many(&cells).iter().filter(|p| p[0].is_finite() && p[1].is_finite()) {
            x0 = x0.min(p[0]);
            x1 = x1.max(p[0]);
            y0 = y0.min(p[1]);
            y1 = y1.max(p[1]);
        }
        let seen = (x1 >= x0).then(|| {
            let (mx, my) = ((x1 - x0) * 0.02, (y1 - y0) * 0.02);
            [(x0 - mx) as f32, (y0 - my) as f32, (x1 + mx) as f32, (y1 + my) as f32]
        });
        self.sight = Some((key, seen));
        seen
    }

    /// A tile's vertices as cells of the view: its corners and edges carried there, once.
    pub fn vertices(&mut self, level: u32, tx: usize, ty: usize) -> Vertices {
        let k = (level, tx, ty);
        if let Some(v) = self.vertices.get(&k) {
            return v.clone();
        }
        let span = (TILE << level) as f64;
        let mut cells = Vec::with_capacity((STEPS + 1) * (STEPS + 1));
        for i in 0..=STEPS {
            for j in 0..=STEPS {
                cells
                    .push([(tx as f64 + j as f64 / STEPS as f64) * span, (ty as f64 + i as f64 / STEPS as f64) * span]);
            }
        }
        let v = Arc::new(self.carry(&cells));
        if self.vertices.len() > KEPT {
            self.vertices.clear();
        }
        self.vertices.insert(k, v.clone());
        v
    }
}

thread_local! {
    /// The ways to carry one grid to another that views side by side use, made once (the transformations are slow
    /// to make): by [`key`].
    static LINKS: std::cell::RefCell<HashMap<u64, Rc<Reproj>>> = std::cell::RefCell::new(HashMap::new());
}

/// How a view's cells are put on another view's (the first of those side by side): by their places when both are of
/// one coordinate system, else carried through the transformation between the two.
#[derive(Clone)]
pub enum Link {
    Place(Place),
    Warp(Rc<Reproj>),
}

impl From<Place> for Link {
    fn from(p: Place) -> Link {
        Link::Place(p)
    }
}

impl Link {
    /// The grid `g` on the grid `base`; none when it is rotated or its system cannot be carried to base's.
    pub fn between(g: &Grid, base: &Grid) -> Option<Link> {
        if let Some(p) = crate::map_view::place_on(g, base) {
            return Some(Link::Place(p));
        }
        let k = key(g, base);
        if let Some(re) = LINKS.with(|l| l.borrow().get(&k).cloned()) {
            return Some(Link::Warp(re));
        }
        let re = Rc::new(Reproj::new(g, base).ok()?);
        LINKS.with(|l| {
            let mut l = l.borrow_mut();
            if l.len() > 16 {
                l.clear();
            }
            l.insert(k, re.clone());
        });
        Some(Link::Warp(re))
    }

    /// A cell of the grid as a cell of the base (NaN where it has no place there).
    pub fn to_base(&self, c: Vec2) -> Vec2 {
        match self {
            Link::Place(p) => p.to_view(c),
            Link::Warp(re) => {
                let v = re.carry(&[[f64::from(c.x), f64::from(c.y)]])[0];
                Vec2::new(v[0], v[1])
            }
        }
    }

    /// A cell of the base as a cell of the grid (NaN where it has no place there).
    pub fn to_own(&self, c: Vec2) -> Vec2 {
        match self {
            Link::Place(p) => p.to_source(c),
            Link::Warp(re) => re.to_source(c).unwrap_or(Vec2::NAN),
        }
    }

    /// Cells of the base per cell of the grid, around the grid's cell `c`.
    pub fn scale_at(&self, c: Vec2) -> f32 {
        match self {
            Link::Place(p) => p.scale.x.abs().max(1e-12),
            Link::Warp(re) => re.scale_at(c).max(1e-12),
        }
    }
}

/// A tile's mesh: the part `uv` of its texture across the quads whose four corners have a place, `screen` the
/// screen point of a cell of the view.
pub fn mesh(vertices: &[[f32; 2]], texture: TextureId, uv: Rect, tint: Color32, screen: impl Fn(Vec2) -> Pos2) -> Mesh {
    let mut m = Mesh::with_texture(texture);
    let side = STEPS + 1;
    let valid: Vec<bool> = vertices.iter().map(|p| p[0].is_finite() && p[1].is_finite()).collect();
    for (k, p) in vertices.iter().enumerate() {
        let (i, j) = (k / side, k % side);
        let pos = if valid[k] { screen(Vec2::new(p[0], p[1])) } else { Pos2::ZERO };
        let at = Pos2::new(
            uv.min.x + uv.width() * j as f32 / STEPS as f32,
            uv.min.y + uv.height() * i as f32 / STEPS as f32,
        );
        m.vertices.push(Vertex { pos, uv: at, color: tint });
    }
    for i in 0..STEPS {
        for j in 0..STEPS {
            let a = i * side + j;
            let (b, c, d) = (a + 1, a + side, a + side + 1);
            if [a, b, c, d].iter().all(|&q| valid[q]) {
                m.indices.extend([a as u32, b as u32, d as u32, a as u32, d as u32, c as u32]);
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10 m cells in UTM 51N (EPSG:32651), about Sibuyan.
    fn utm() -> Grid {
        let mut g = Grid::new(400, 300);
        g.transform = [450_000.0, 10.0, 0.0, 1_300_000.0, 0.0, -10.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32651]);
        g
    }

    /// Cells of 0.0001 degrees in WGS 84, around the same place.
    fn geographic() -> Grid {
        let mut g = Grid::new(1000, 800);
        g.transform = [122.5, 0.0001, 0.0, 11.8, 0.0, -0.0001];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        g
    }

    #[test]
    fn a_cell_of_a_grid_is_carried_to_another_and_back() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let (utm, geo) = (utm(), geographic());
        assert!(crs_pair(&utm, &utm).is_none(), "the same system needs nothing");
        assert!(crs_pair(&utm, &geo).is_some());
        let re = Reproj::new(&utm, &geo).expect("a transformation");
        // (10 m is about 0.00009 degrees)
        assert!((0.5..1.5).contains(&re.scale), "a cell of the layer is about a cell of the view: {}", re.scale);
        let back = Reproj::new(&geo, &utm).expect("the other way");
        for cell in [[0.0, 0.0], [200.0, 150.0], [399.0, 299.0], [37.5, 211.25]] {
            let there = re.carry(&[cell])[0];
            assert!(there[0].is_finite() && there[1].is_finite(), "{cell:?} has a place");
            // (a fraction of a cell, f32 apart)
            let again = back.carry(&[[f64::from(there[0]), f64::from(there[1])]])[0];
            assert!((again[0] as f64 - cell[0]).abs() < 0.05, "{cell:?} -> {there:?} -> {again:?}");
            assert!((again[1] as f64 - cell[1]).abs() < 0.05, "{cell:?} -> {there:?} -> {again:?}");
            // the view's cell given back by the inverse
            let c = re.to_source(Vec2::new(there[0], there[1])).expect("back");
            assert!((c.x as f64 - cell[0]).abs() < 0.05 && (c.y as f64 - cell[1]).abs() < 0.05, "{cell:?} -> {c:?}");
        }
        // the layer's first corner (450 000 E, 1 300 000 N) falls within the geographic grid
        let v = re.carry(&[[0.0, 0.0]])[0];
        assert!(v[0] > 0.0 && v[0] < 1000.0 && v[1] > 0.0 && v[1] < 800.0, "inside the geographic grid: {v:?}");
    }

    #[test]
    fn the_box_seen_and_a_tiles_vertices() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let (utm, geo) = (utm(), geographic());
        let mut re = Reproj::new(&utm, &geo).expect("a transformation");
        // the whole geographic grid is over the layer's cells, the layer's box within a little of its own
        let b = re.seen(Vec2::ZERO, Vec2::new(1000.0, 800.0)).expect("a box");
        assert!(b[0] < b[2] && b[1] < b[3]);
        assert!(re.seen(Vec2::ZERO, Vec2::new(1000.0, 800.0)) == Some(b), "kept");
        // the vertices of the first tile: the corners of its cells, in a lattice that follows them
        let v = re.vertices(0, 0, 0);
        assert_eq!(v.len(), (STEPS + 1) * (STEPS + 1));
        let corner = re.carry(&[[0.0, 0.0], [TILE as f64, TILE as f64]]);
        assert_eq!(v[0], corner[0]);
        assert_eq!(v[v.len() - 1], corner[1]);
        assert!(Arc::ptr_eq(&v, &re.vertices(0, 0, 0)), "made once");
        // the layer's own box (400 x 300 cells of 10 m) on the view: about 0.04 x 0.027 degrees, 360 x 270 cells of it
        let b = re.box_on_view([0.0, 0.0, 400.0, 300.0]).expect("a box on the view");
        let (w, h) = (b[2] - b[0], b[3] - b[1]);
        assert!((350.0..420.0).contains(&w) && (260.0..330.0).contains(&h), "{b:?}");
        // a mesh of it: every quad has a place
        let m =
            mesh(&v, TextureId::default(), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE, |c| {
                Pos2::new(c.x, c.y)
            });
        assert_eq!(m.vertices.len(), v.len());
        assert_eq!(m.indices.len(), STEPS * STEPS * 6);
    }
}
