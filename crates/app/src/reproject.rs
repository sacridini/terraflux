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

/// The coordinate systems of a grid and of the one it is drawn on, when they differ (a map with no coordinate
/// system cannot be carried). A rotated grid is no reason to refuse: it is carried through its full affine transform.
pub fn crs_pair(g: &Grid, base: &Grid) -> Option<(String, String)> {
    let (x, y) = (crate::map_view::crs_of(g)?, crate::map_view::crs_of(base)?);
    (x != y && !terraflux_gdal::same_crs(&x, &y).unwrap_or(false)).then_some((x, y))
}

/// A grid's world coordinates and its cells, by its full affine transform (rotated or not).
#[derive(Clone, Copy)]
struct Affine {
    t: [f64; 6],
}

impl Affine {
    /// The transform, when it can be inverted.
    fn new(t: [f64; 6]) -> Option<Affine> {
        let det = t[1] * t[5] - t[2] * t[4];
        (det.is_finite() && det != 0.0).then_some(Affine { t })
    }

    fn rotated(&self) -> bool {
        self.t[2] != 0.0 || self.t[4] != 0.0
    }

    /// A cell's world coordinates.
    fn world(&self, c: [f64; 2]) -> [f64; 2] {
        let t = self.t;
        [t[0] + c[0] * t[1] + c[1] * t[2], t[3] + c[0] * t[4] + c[1] * t[5]]
    }

    /// The cell of world coordinates (the plain division of an axis-aligned grid, as it was).
    fn cell(&self, x: f64, y: f64) -> [f64; 2] {
        let t = self.t;
        if !self.rotated() {
            return [(x - t[0]) / t[1], (y - t[3]) / t[5]];
        }
        let (dx, dy, det) = (x - t[0], y - t[3], t[1] * t[5] - t[2] * t[4]);
        [(t[5] * dx - t[2] * dy) / det, (t[1] * dy - t[4] * dx) / det]
    }

    /// Cells per degree (or unit) of x: how long a world's half (180 degrees) is in cells.
    fn half_world(&self) -> f32 {
        let t = self.t;
        let det = t[1] * t[5] - t[2] * t[4];
        (180.0 * (t[5] / det).hypot(t[4] / det)) as f32
    }
}

/// Longitudes (degrees) moved by whole turns to the nearest to `centre`: a point carried across +-180 degrees comes
/// back as the other side of the world, and this puts it on the side the grid is on. Not for polar grids, where a
/// longitude means little.
fn unwrap(xs: &mut [f64], centre: f64) {
    for x in xs.iter_mut().filter(|x| x.is_finite()) {
        *x -= 360.0 * ((*x - centre) / 360.0).round();
    }
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

/// A layer's cells carried to the view's: the transformations between the two systems (made once, none when they
/// are the same), and the vertices of the tiles seen.
pub struct Reproj {
    key: u64,
    src: Affine,
    view: Affine,
    /// The layer's system to the view's; none when the same (a rotated grid of the view's own system).
    to_view: Option<terraflux_gdal::Transform>,
    to_src: Option<terraflux_gdal::Transform>,
    /// The longitude (degrees) at the view's middle, when its system is geographic: points carried there are moved
    /// by 360 degrees to the nearest to it (the antimeridian).
    view_lon: Option<f64>,
    /// The same for the layer, for the cells of the view carried back to it.
    src_lon: Option<f64>,
    /// View cells per cell of the layer, at the layer's middle.
    pub scale: f32,
    /// Half a world (180 degrees) in cells of the view, when it is geographic (infinite when not): a quad whose
    /// corners are farther apart than that jumps over the antimeridian and is not drawn.
    pub half: f32,
    /// A tile's (STEPS + 1)^2 vertices as cells of the view (NaN where the view's system has no place for them).
    vertices: HashMap<(u32, usize, usize), Vertices>,
    /// The view's corners (cells of the view) last asked about, and the box of the layer's cells they stand for.
    sight: Option<Sight>,
}

/// The longitude at the middle of a grid, when its system is geographic.
fn middle_lon(g: &Grid, crs: Option<&String>) -> Option<f64> {
    let crs = crs?;
    terraflux_gdal::is_geographic(crs).unwrap_or(false).then(|| {
        let a = Affine { t: g.transform };
        a.world([g.width as f64 / 2.0, g.height as f64 / 2.0])[0]
    })
}

impl Reproj {
    /// The layer `g` on the view's grid `base`, when the two can be carried one to the other: of two systems told
    /// (through GDAL's transformation when they differ, directly when they are the same) or of none told (rotated
    /// grids), their grids rotated or not.
    pub fn new(g: &Grid, base: &Grid) -> Result<Reproj, String> {
        let (cg, cb) = (crate::map_view::crs_of(g), crate::map_view::crs_of(base));
        let (to_view, to_src) = match (&cg, &cb) {
            (Some(x), Some(y)) if x != y && !terraflux_gdal::same_crs(x, y).unwrap_or(false) => {
                (Some(terraflux_gdal::Transform::new(x, y)?), Some(terraflux_gdal::Transform::new(y, x)?))
            }
            (Some(_), None) | (None, Some(_)) => return Err("no coordinate systems to carry between".into()),
            // the same system, or none told for either: the cells' places in the world are the same ones
            _ => (None, None),
        };
        let (src, view) = (Affine::new(g.transform), Affine::new(base.transform));
        let (Some(src), Some(view)) = (src, view) else { return Err("a grid with no cells' size".into()) };
        let mut re = Reproj {
            key: key(g, base),
            src,
            view,
            to_view,
            to_src,
            view_lon: middle_lon(base, cb.as_ref()),
            src_lon: middle_lon(g, cg.as_ref()),
            scale: 1.0,
            half: f32::INFINITY,
            vertices: HashMap::new(),
            sight: None,
        };
        if re.view_lon.is_some() {
            re.half = view.half_world();
        }
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

    /// Cells of the layer as cells of the view (NaN where they have no place there): by the layer's transform, the
    /// systems' transformation, the longitudes unwrapped to the view's middle (if geographic), the view's inverse.
    pub fn carry(&self, cells: &[[f64; 2]]) -> Vec<[f32; 2]> {
        let (mut xs, mut ys): (Vec<f64>, Vec<f64>) = cells
            .iter()
            .map(|&c| {
                let w = self.src.world(c);
                (w[0], w[1])
            })
            .unzip();
        if let Some(t) = &self.to_view {
            t.points(&mut xs, &mut ys);
        }
        if let Some(lon) = self.view_lon {
            unwrap(&mut xs, lon);
        }
        xs.iter().zip(&ys).map(|(&x, &y)| self.view.cell(x, y).map(|v| v as f32)).collect()
    }

    /// Cells of the view as cells of the layer (NaN where they have no place there); the longitudes brought to the
    /// layer's middle (a layer across the antimeridian is looked up at +-360 degrees).
    pub fn to_source_many(&self, cells: &[[f64; 2]]) -> Vec<[f64; 2]> {
        let (mut xs, mut ys): (Vec<f64>, Vec<f64>) = cells
            .iter()
            .map(|&c| {
                let w = self.view.world(c);
                (w[0], w[1])
            })
            .unzip();
        if let Some(t) = &self.to_src {
            t.points(&mut xs, &mut ys);
        }
        if let Some(lon) = self.src_lon {
            unwrap(&mut xs, lon);
        }
        xs.iter().zip(&ys).map(|(&x, &y)| self.src.cell(x, y)).collect()
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
    /// The grid `g` on the grid `base` (rotated or not); none when its system cannot be carried to base's.
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

/// A tile's mesh: the part `uv` of its texture across the quads whose four corners have a place and are not farther
/// apart than `half` cells (a quad that jumps across the antimeridian: [`Reproj::half`]), `screen` the screen point
/// of a cell of the view.
pub fn mesh(
    vertices: &[[f32; 2]],
    texture: TextureId,
    uv: Rect,
    tint: Color32,
    half: f32,
    screen: impl Fn(Vec2) -> Pos2,
) -> Mesh {
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
            if [a, b, c, d].iter().all(|&q| valid[q])
                && !jumps(&[vertices[a], vertices[b], vertices[c], vertices[d]], half)
            {
                m.indices.extend([a as u32, b as u32, d as u32, a as u32, d as u32, c as u32]);
            }
        }
    }
    m
}

/// Whether a quad's corners are farther apart (the diagonal of their box) than `half`.
fn jumps(q: &[[f32; 2]; 4], half: f32) -> bool {
    let span = |k: usize| {
        let (lo, hi) = q.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), p| (l.min(p[k]), h.max(p[k])));
        hi - lo
    };
    span(0).hypot(span(1)) > half
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
        let m = mesh(
            &v,
            TextureId::default(),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
            f32::INFINITY,
            |c| Pos2::new(c.x, c.y),
        );
        assert_eq!(m.vertices.len(), v.len());
        assert_eq!(m.indices.len(), STEPS * STEPS * 6);
    }

    /// A grid in degrees: `w` x `h` cells of 0.1 degrees from the longitude `x0`.
    fn degrees(x0: f64, w: usize, h: usize) -> Grid {
        let mut g = Grid::new(w, h);
        g.transform = [x0, 0.1, 0.0, 10.0, 0.0, -0.1];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        g
    }

    /// The largest step between a tile's neighbouring vertices (cells of the view), and how many have a place.
    fn steps(v: &[[f32; 2]]) -> (f32, usize) {
        let side = STEPS + 1;
        let (mut most, mut n) = (0.0f32, 0);
        for (k, p) in v.iter().enumerate() {
            n += usize::from(p[0].is_finite() && p[1].is_finite());
            for q in [k + 1, k + side].into_iter().filter(|&q| q < v.len() && (q != k + 1 || (k + 1) % side != 0)) {
                let d = (v[q][0] - p[0]).hypot(v[q][1] - p[1]);
                if d.is_finite() {
                    most = most.max(d);
                }
            }
        }
        (most, n)
    }

    fn whole() -> Rect {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
    }

    #[test]
    fn a_rotated_grid_is_carried_to_an_axis_aligned_one_and_back() {
        // the same system told (no transformation is made, so no GDAL is needed), a layer turned by 30 degrees
        let view = utm();
        let (c, s) = (30f64.to_radians().cos() * 10.0, 30f64.to_radians().sin() * 10.0);
        let mut layer = Grid::new(100, 80);
        layer.transform = [450_100.0, c, s, 1_299_900.0, s, -c];
        layer.geo.geokeys = view.geo.geokeys.clone();
        assert!(crs_pair(&layer, &view).is_none(), "the same system");
        assert!(crate::map_view::place_on(&layer, &view).is_none(), "no plain place for a rotated grid");
        assert!(matches!(Link::between(&layer, &view), Some(Link::Warp(_))));
        let re = Reproj::new(&layer, &view).expect("a rotated grid is carried");
        assert!(re.to_view.is_none() && re.to_src.is_none(), "no transformation of systems for one system");
        assert!(re.half.is_infinite(), "no antimeridian in metres");
        assert!((re.scale - 1.0).abs() < 1e-3, "cells of 10 m on cells of 10 m: {}", re.scale);
        // the layer's cell corner (3, 4): its world place by the layer's transform, then the view's cell
        let (x, y) = (450_100.0 + 3.0 * c + 4.0 * s, 1_299_900.0 + 3.0 * s - 4.0 * c);
        let (vx, vy) = ((x - 450_000.0) / 10.0, (y - 1_300_000.0) / -10.0);
        let there = re.carry(&[[3.0, 4.0], [0.0, 0.0]]);
        assert!((f64::from(there[0][0]) - vx).abs() < 1e-3 && (f64::from(there[0][1]) - vy).abs() < 1e-3, "{there:?}");
        assert!((there[1][0] - 10.0).abs() < 1e-3 && (there[1][1] - 10.0).abs() < 1e-3, "{there:?}");
        // and back, by the inverse of the layer's full transform
        let back = re.to_source_many(&[[vx, vy]])[0];
        assert!((back[0] - 3.0).abs() < 1e-6 && (back[1] - 4.0).abs() < 1e-6, "{back:?}");
        let c = re.to_source(Vec2::new(vx as f32, vy as f32)).expect("a place");
        assert!((c.x - 3.0).abs() < 1e-2 && (c.y - 4.0).abs() < 1e-2, "{c:?}");
        // the view the other way round (the shown grid rotated, the layer not) is the inverse
        let inv = Reproj::new(&view, &layer).expect("the other way");
        let again = inv.carry(&[[vx, vy]])[0];
        assert!((f64::from(again[0]) - 3.0).abs() < 1e-3 && (f64::from(again[1]) - 4.0).abs() < 1e-3, "{again:?}");
        // the box of the whole layer on the view is wider than the layer's cells
        let b = re.box_on_view([0.0, 0.0, 100.0, 80.0]).expect("a box");
        assert!(b[2] - b[0] > 100.0 && b[3] - b[1] > 80.0, "{b:?}");
        // a map with a system and one with none cannot be carried to each other
        let mut bare = layer.clone();
        bare.geo.geokeys = None;
        assert!(Reproj::new(&bare, &view).is_err());
        assert!(Reproj::new(&bare, &bare).is_ok(), "none told for either: the same places");
    }

    #[test]
    fn a_layer_across_the_antimeridian_is_carried_without_a_jump() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        // 170 E to 170 W (in 190 degrees' notation, or -180 to -160, or -190 to -150) on a view of 160 to 200
        let view = degrees(160.0, 400, 100);
        for layer in [degrees(170.0, 200, 100), degrees(-180.0, 200, 100), degrees(-190.0, 400, 100)] {
            let mut re = Reproj::new(&layer, &view).expect("the same system");
            assert!(re.to_view.is_none(), "one system: GDAL is not called");
            assert!((re.half - 1800.0).abs() < 1.0, "half a world in cells of 0.1 degrees: {}", re.half);
            // every tile of the layer: vertices continuous and every quad drawn
            for tx in 0..layer.width.div_ceil(TILE) {
                let v = re.vertices(0, tx, 0);
                let (most, n) = steps(&v);
                assert!(most < 20.0, "a tile's vertices are one step apart, not a world: {most}");
                assert_eq!(n, v.len());
                let m = mesh(&v, TextureId::default(), whole(), Color32::WHITE, re.half, |c| Pos2::new(c.x, c.y));
                assert_eq!(m.indices.len(), STEPS * STEPS * 6);
            }
        }
        // the cells at -175 degrees are at 185 on a view at 180, and the view's cell 250 (185 degrees) is the layer's
        let layer = degrees(-180.0, 200, 100);
        let re = Reproj::new(&layer, &view).expect("a transformation");
        let there = re.carry(&[[50.0, 0.0]])[0];
        assert!((there[0] - 250.0).abs() < 1e-3, "{there:?}");
        let back = re.to_source_many(&[[250.0, 0.0]])[0];
        assert!((back[0] - 50.0).abs() < 1e-6, "{back:?}");
        // a layer of the whole world: the quads over the cut (0 degrees, far from the view) are dropped
        let world = degrees(-180.0, 3600, 100);
        let mut re = Reproj::new(&world, &view).expect("a transformation");
        let v = re.vertices(0, 7, 0);
        let m = mesh(&v, TextureId::default(), whole(), Color32::WHITE, re.half, |c| Pos2::new(c.x, c.y));
        assert!(m.indices.len() < STEPS * STEPS * 6 && !m.indices.is_empty(), "the quads over 0 degrees are dropped");
        for t in m.indices.chunks(3) {
            let p: Vec<Pos2> = t.iter().map(|&i| m.vertices[i as usize].pos).collect();
            assert!((p[0] - p[1]).length() < 100.0 && (p[0] - p[2]).length() < 100.0, "{p:?}");
        }
    }

    #[test]
    fn a_layer_across_the_antimeridian_on_a_projected_view() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        // PDC Mercator (EPSG:3832) is centred on 150 E: 170 E to 170 W is one stretch of it
        let mut view = Grid::new(800, 800);
        view.transform = [2_000_000.0, 10_000.0, 0.0, 6_000_000.0, 0.0, -10_000.0];
        view.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 3832]);
        let layer = degrees(170.0, 200, 100);
        let mut re = Reproj::new(&layer, &view).expect("a transformation");
        assert!(re.half.is_infinite());
        for tx in 0..layer.width.div_ceil(TILE) {
            let v = re.vertices(0, tx, 0);
            let (most, n) = steps(&v);
            assert!(n > 0 && most < 60.0, "continuous across 180 degrees: {most} ({n} vertices)");
        }
        // and the view's cells are looked up in the layer at 190, not -170
        let c = re.to_source_many(&[[400.0, 400.0]]);
        assert!(c[0][0].is_finite());
    }
}
