//! GEOS's own C API, for what GDAL does not pass on (the Voronoi diagram): the geos_c library next to GDAL's (the
//! one GDAL itself loads), else the system's, loaded when first needed.

use std::ffi::{CStr, c_char, c_double, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::Result;

mod index;
pub use index::{GeosIndex, Predicate};

type Ctx = *mut c_void;
type G = *mut c_void;
type Cs = *mut c_void;
type Bp = *mut c_void;

#[allow(non_snake_case)]
struct Api {
    GEOS_init_r: unsafe extern "C" fn() -> Ctx,
    GEOS_finish_r: unsafe extern "C" fn(Ctx),
    GEOSGeom_createPointFromXY_r: unsafe extern "C" fn(Ctx, c_double, c_double) -> G,
    GEOSGeom_createCollection_r: unsafe extern "C" fn(Ctx, c_int, *mut G, c_uint) -> G,
    GEOSVoronoiDiagram_r: unsafe extern "C" fn(Ctx, G, G, c_double, c_int) -> G,
    GEOSGetNumGeometries_r: unsafe extern "C" fn(Ctx, G) -> c_int,
    GEOSGetGeometryN_r: unsafe extern "C" fn(Ctx, G, c_int) -> G,
    GEOSContains_r: unsafe extern "C" fn(Ctx, G, G) -> c_char,
    GEOSGeom_destroy_r: unsafe extern "C" fn(Ctx, G),
    GEOSversion: unsafe extern "C" fn() -> *const c_char,
    GEOSCoordSeq_create_r: unsafe extern "C" fn(Ctx, c_uint, c_uint) -> Cs,
    GEOSCoordSeq_setXY_r: unsafe extern "C" fn(Ctx, Cs, c_uint, c_double, c_double) -> c_int,
    GEOSCoordSeq_getSize_r: unsafe extern "C" fn(Ctx, Cs, *mut c_uint) -> c_int,
    GEOSCoordSeq_getXY_r: unsafe extern "C" fn(Ctx, Cs, c_uint, *mut c_double, *mut c_double) -> c_int,
    GEOSGeom_createLinearRing_r: unsafe extern "C" fn(Ctx, Cs) -> G,
    GEOSGeom_createPolygon_r: unsafe extern "C" fn(Ctx, G, *mut G, c_uint) -> G,
    GEOSGeom_getCoordSeq_r: unsafe extern "C" fn(Ctx, G) -> Cs,
    GEOSUnaryUnion_r: unsafe extern "C" fn(Ctx, G) -> G,
    GEOSIntersection_r: unsafe extern "C" fn(Ctx, G, G) -> G,
    GEOSIntersects_r: unsafe extern "C" fn(Ctx, G, G) -> c_char,
    GEOSConvexHull_r: unsafe extern "C" fn(Ctx, G) -> G,
    GEOSBufferParams_create_r: unsafe extern "C" fn(Ctx) -> Bp,
    GEOSBufferParams_setQuadrantSegments_r: unsafe extern "C" fn(Ctx, Bp, c_int) -> c_int,
    GEOSBufferParams_destroy_r: unsafe extern "C" fn(Ctx, Bp),
    GEOSBufferWithParams_r: unsafe extern "C" fn(Ctx, G, Bp, c_double) -> G,
    GEOSGeomTypeId_r: unsafe extern "C" fn(Ctx, G) -> c_int,
    GEOSisEmpty_r: unsafe extern "C" fn(Ctx, G) -> c_char,
    GEOSGetExteriorRing_r: unsafe extern "C" fn(Ctx, G) -> G,
    GEOSGetNumInteriorRings_r: unsafe extern "C" fn(Ctx, G) -> c_int,
    GEOSGetInteriorRingN_r: unsafe extern "C" fn(Ctx, G, c_int) -> G,
    GEOSWKBReader_create_r: unsafe extern "C" fn(Ctx) -> G,
    GEOSWKBReader_read_r: unsafe extern "C" fn(Ctx, G, *const u8, usize) -> G,
    GEOSWKBReader_destroy_r: unsafe extern "C" fn(Ctx, G),
    GEOSSTRtree_create_r: unsafe extern "C" fn(Ctx, usize) -> G,
    GEOSSTRtree_insert_r: unsafe extern "C" fn(Ctx, G, G, *mut c_void),
    GEOSSTRtree_query_r: unsafe extern "C" fn(Ctx, G, G, TreeCallback, *mut c_void),
    GEOSSTRtree_nearest_generic_r:
        unsafe extern "C" fn(Ctx, G, *const c_void, G, NearCallback, *mut c_void) -> *const c_void,
    GEOSSTRtree_destroy_r: unsafe extern "C" fn(Ctx, G),
    GEOSPrepare_r: unsafe extern "C" fn(Ctx, G) -> G,
    GEOSPreparedGeom_destroy_r: unsafe extern "C" fn(Ctx, G),
    GEOSPreparedIntersects_r: PreparedTest,
    GEOSPreparedWithin_r: PreparedTest,
    GEOSPreparedContains_r: PreparedTest,
    GEOSPreparedContainsProperly_r: PreparedTest,
    GEOSPreparedTouches_r: PreparedTest,
    GEOSPreparedCrosses_r: PreparedTest,
    GEOSPreparedOverlaps_r: PreparedTest,
    GEOSPreparedCovers_r: PreparedTest,
    GEOSPreparedCoveredBy_r: PreparedTest,
    GEOSEquals_r: PreparedTest,
    GEOSDistance_r: unsafe extern "C" fn(Ctx, G, G, *mut c_double) -> c_int,
    /// GEOS 3.10 and later.
    GEOSPreparedDistanceWithin_r: Option<unsafe extern "C" fn(Ctx, G, G, c_double) -> c_char>,
}

pub(crate) type TreeCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);
pub(crate) type NearCallback = unsafe extern "C" fn(*const c_void, *const c_void, *mut c_double, *mut c_void) -> c_int;
pub(crate) type PreparedTest = unsafe extern "C" fn(Ctx, G, G) -> c_char;

/// GEOS loaded: its functions and version.
pub struct Geos {
    _lib: libloading::Library,
    api: Api,
    /// GEOS's version, e.g. "3.13.1-CAPI-1.19.2".
    pub version: String,
    pub library: PathBuf,
}

unsafe impl Send for Geos {}
unsafe impl Sync for Geos {}

static GEOS: OnceLock<std::result::Result<Geos, String>> = OnceLock::new();

/// The names geos_c may have in a folder.
fn is_geos_c(name: &str) -> bool {
    let n = name.to_lowercase();
    if cfg!(windows) {
        n.starts_with("geos_c") && n.ends_with(".dll")
    } else if cfg!(target_os = "macos") {
        n.starts_with("libgeos_c") && n.ends_with(".dylib")
    } else {
        n.starts_with("libgeos_c.so")
    }
}

fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(g) = crate::gdal()
        && let Some(dir) = g.library.parent()
        && let Ok(rd) = std::fs::read_dir(dir)
    {
        let mut v: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().is_some_and(|n| is_geos_c(&n.to_string_lossy())))
            .collect();
        v.sort();
        out.extend(v);
    }
    let system: &[&str] = if cfg!(windows) {
        &["geos_c.dll"]
    } else if cfg!(target_os = "macos") {
        &["libgeos_c.dylib", "/opt/homebrew/lib/libgeos_c.dylib", "/usr/local/lib/libgeos_c.dylib"]
    } else {
        &["libgeos_c.so.1", "libgeos_c.so"]
    };
    out.extend(system.iter().map(PathBuf::from));
    out
}

fn load(path: &Path) -> Result<Geos> {
    let lib = unsafe { libloading::Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))?;
    macro_rules! f {
        ($name:ident) => {
            *unsafe { lib.get(concat!(stringify!($name), "\0").as_bytes()) }
                .map_err(|e| format!("{}: no {}: {e}", path.display(), stringify!($name)))?
        };
    }
    macro_rules! opt {
        ($name:ident) => {
            unsafe { lib.get(concat!(stringify!($name), "\0").as_bytes()) }.ok().map(|s| *s)
        };
    }
    let api = Api {
        GEOS_init_r: f!(GEOS_init_r),
        GEOS_finish_r: f!(GEOS_finish_r),
        GEOSGeom_createPointFromXY_r: f!(GEOSGeom_createPointFromXY_r),
        GEOSGeom_createCollection_r: f!(GEOSGeom_createCollection_r),
        GEOSVoronoiDiagram_r: f!(GEOSVoronoiDiagram_r),
        GEOSGetNumGeometries_r: f!(GEOSGetNumGeometries_r),
        GEOSGetGeometryN_r: f!(GEOSGetGeometryN_r),
        GEOSContains_r: f!(GEOSContains_r),
        GEOSGeom_destroy_r: f!(GEOSGeom_destroy_r),
        GEOSversion: f!(GEOSversion),
        GEOSCoordSeq_create_r: f!(GEOSCoordSeq_create_r),
        GEOSCoordSeq_setXY_r: f!(GEOSCoordSeq_setXY_r),
        GEOSCoordSeq_getSize_r: f!(GEOSCoordSeq_getSize_r),
        GEOSCoordSeq_getXY_r: f!(GEOSCoordSeq_getXY_r),
        GEOSGeom_createLinearRing_r: f!(GEOSGeom_createLinearRing_r),
        GEOSGeom_createPolygon_r: f!(GEOSGeom_createPolygon_r),
        GEOSGeom_getCoordSeq_r: f!(GEOSGeom_getCoordSeq_r),
        GEOSUnaryUnion_r: f!(GEOSUnaryUnion_r),
        GEOSIntersection_r: f!(GEOSIntersection_r),
        GEOSIntersects_r: f!(GEOSIntersects_r),
        GEOSConvexHull_r: f!(GEOSConvexHull_r),
        GEOSBufferParams_create_r: f!(GEOSBufferParams_create_r),
        GEOSBufferParams_setQuadrantSegments_r: f!(GEOSBufferParams_setQuadrantSegments_r),
        GEOSBufferParams_destroy_r: f!(GEOSBufferParams_destroy_r),
        GEOSBufferWithParams_r: f!(GEOSBufferWithParams_r),
        GEOSGeomTypeId_r: f!(GEOSGeomTypeId_r),
        GEOSisEmpty_r: f!(GEOSisEmpty_r),
        GEOSGetExteriorRing_r: f!(GEOSGetExteriorRing_r),
        GEOSGetNumInteriorRings_r: f!(GEOSGetNumInteriorRings_r),
        GEOSGetInteriorRingN_r: f!(GEOSGetInteriorRingN_r),
        GEOSWKBReader_create_r: f!(GEOSWKBReader_create_r),
        GEOSWKBReader_read_r: f!(GEOSWKBReader_read_r),
        GEOSWKBReader_destroy_r: f!(GEOSWKBReader_destroy_r),
        GEOSSTRtree_create_r: f!(GEOSSTRtree_create_r),
        GEOSSTRtree_insert_r: f!(GEOSSTRtree_insert_r),
        GEOSSTRtree_query_r: f!(GEOSSTRtree_query_r),
        GEOSSTRtree_nearest_generic_r: f!(GEOSSTRtree_nearest_generic_r),
        GEOSSTRtree_destroy_r: f!(GEOSSTRtree_destroy_r),
        GEOSPrepare_r: f!(GEOSPrepare_r),
        GEOSPreparedGeom_destroy_r: f!(GEOSPreparedGeom_destroy_r),
        GEOSPreparedIntersects_r: f!(GEOSPreparedIntersects_r),
        GEOSPreparedWithin_r: f!(GEOSPreparedWithin_r),
        GEOSPreparedContains_r: f!(GEOSPreparedContains_r),
        GEOSPreparedContainsProperly_r: f!(GEOSPreparedContainsProperly_r),
        GEOSPreparedTouches_r: f!(GEOSPreparedTouches_r),
        GEOSPreparedCrosses_r: f!(GEOSPreparedCrosses_r),
        GEOSPreparedOverlaps_r: f!(GEOSPreparedOverlaps_r),
        GEOSPreparedCovers_r: f!(GEOSPreparedCovers_r),
        GEOSPreparedCoveredBy_r: f!(GEOSPreparedCoveredBy_r),
        GEOSEquals_r: f!(GEOSEquals_r),
        GEOSDistance_r: f!(GEOSDistance_r),
        GEOSPreparedDistanceWithin_r: opt!(GEOSPreparedDistanceWithin_r),
    };
    let v = unsafe { (api.GEOSversion)() };
    let version = if v.is_null() { String::new() } else { unsafe { CStr::from_ptr(v) }.to_string_lossy().into_owned() };
    Ok(Geos { _lib: lib, api, version, library: path.to_path_buf() })
}

/// GEOS, or why it could not be had.
pub fn geos() -> std::result::Result<&'static Geos, &'static str> {
    GEOS.get_or_init(|| {
        let mut tried = Vec::new();
        for c in candidates() {
            match load(&c) {
                Ok(g) => return Ok(g),
                Err(e) => tried.push(e),
            }
        }
        Err(format!("GEOS could not be loaded: {}", tried.join("; ")))
    })
    .as_ref()
    .map_err(String::as_str)
}

impl Geos {
    /// GEOS's major and minor version.
    pub fn version_number(&self) -> (u32, u32) {
        let mut it = self.version.split(['.', '-']).map(|p| p.parse::<u32>().unwrap_or(0));
        (it.next().unwrap_or(0), it.next().unwrap_or(0))
    }

    /// The germ inside each cell of GEOS's Voronoi diagram of the points (as `sf::st_voronoi(sf::st_union(points))`
    /// gives it: the points sorted by x, then y, in a multipoint; shapely's `voronoi_polygons` with the tolerance),
    /// in the order GEOS lists the cells: the index of the point each cell contains.
    pub fn voronoi_cell_order(&self, x: &[f64], y: &[f64], tolerance: f64) -> Result<Vec<usize>> {
        let a = &self.api;
        let mut order: Vec<usize> = (0..x.len()).collect();
        order.sort_by(|&i, &j| x[i].total_cmp(&x[j]).then(y[i].total_cmp(&y[j])));
        unsafe {
            let ctx = (a.GEOS_init_r)();
            if ctx.is_null() {
                return Err("GEOS could not start".into());
            }
            let result = (|| -> Result<Vec<usize>> {
                let mut sorted: Vec<G> =
                    order.iter().map(|&i| (a.GEOSGeom_createPointFromXY_r)(ctx, x[i], y[i])).collect();
                if sorted.iter().any(|p| p.is_null()) {
                    return Err("GEOS could not make the points".into());
                }
                // GEOS_MULTIPOINT: the collection takes the points
                let multi = (a.GEOSGeom_createCollection_r)(ctx, 4, sorted.as_mut_ptr(), sorted.len() as c_uint);
                if multi.is_null() {
                    return Err("GEOS could not make the multipoint".into());
                }
                let cells = (a.GEOSVoronoiDiagram_r)(ctx, multi, std::ptr::null_mut(), tolerance, 0);
                (a.GEOSGeom_destroy_r)(ctx, multi);
                if cells.is_null() {
                    return Err("GEOS could not make the Voronoi diagram".into());
                }
                let points: Vec<G> = (0..x.len()).map(|i| (a.GEOSGeom_createPointFromXY_r)(ctx, x[i], y[i])).collect();
                let n = (a.GEOSGetNumGeometries_r)(ctx, cells).max(0) as usize;
                let mut germ = vec![usize::MAX; n];
                for (k, g) in germ.iter_mut().enumerate() {
                    let cell = (a.GEOSGetGeometryN_r)(ctx, cells, k as c_int);
                    // the last point inside, as shapely's tree query and its assignment
                    for (i, &p) in points.iter().enumerate() {
                        if (a.GEOSContains_r)(ctx, cell, p) == 1 {
                            *g = i;
                        }
                    }
                }
                points.into_iter().for_each(|p| (a.GEOSGeom_destroy_r)(ctx, p));
                (a.GEOSGeom_destroy_r)(ctx, cells);
                if n != x.len() || germ.contains(&usize::MAX) {
                    return Err("GEOS's Voronoi cells do not match the germs one to one".into());
                }
                Ok(germ)
            })();
            (a.GEOS_finish_r)(ctx);
            result
        }
    }
}

/// A ring's vertices (the last equal to the first).
pub type Ring = Vec<[f64; 2]>;
/// A polygon: its rings, the exterior first (none: an empty polygon).
pub type Poly = Vec<Ring>;

/// A GEOS context, finished when dropped.
struct Session<'a> {
    a: &'a Api,
    ctx: Ctx,
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        unsafe { (self.a.GEOS_finish_r)(self.ctx) }
    }
}

impl Session<'_> {
    /// A ring as a GEOS linear ring.
    unsafe fn ring(&self, r: &Ring) -> Result<G> {
        let a = self.a;
        let cs = unsafe { (a.GEOSCoordSeq_create_r)(self.ctx, r.len() as c_uint, 2) };
        if cs.is_null() {
            return Err("GEOS could not make a coordinate sequence".into());
        }
        for (i, p) in r.iter().enumerate() {
            if unsafe { (a.GEOSCoordSeq_setXY_r)(self.ctx, cs, i as c_uint, p[0], p[1]) } == 0 {
                return Err("GEOS could not set a coordinate".into());
            }
        }
        let g = unsafe { (a.GEOSGeom_createLinearRing_r)(self.ctx, cs) };
        if g.is_null() {
            Err("GEOS could not make a ring (it needs 4 vertices, the last the first)".into())
        } else {
            Ok(g)
        }
    }

    /// A polygon (None: an empty one, left out).
    unsafe fn polygon(&self, p: &Poly) -> Result<Option<G>> {
        let a = self.a;
        let Some((shell, holes)) = p.split_first() else { return Ok(None) };
        let shell = unsafe { self.ring(shell) }?;
        let mut made: Vec<G> = Vec::with_capacity(holes.len());
        for h in holes {
            made.push(unsafe { self.ring(h) }?);
        }
        let g = unsafe { (a.GEOSGeom_createPolygon_r)(self.ctx, shell, made.as_mut_ptr(), made.len() as c_uint) };
        if g.is_null() { Err("GEOS could not make a polygon".into()) } else { Ok(Some(g)) }
    }

    /// The polygons as one geometry: a collection of the type (6 multipolygon, 7 geometry collection), or for a
    /// multipolygon of one polygon that polygon.
    unsafe fn shape(&self, polys: &[Poly], kind: c_int) -> Result<G> {
        let a = self.a;
        let mut made: Vec<G> = Vec::with_capacity(polys.len());
        for p in polys {
            if let Some(g) = unsafe { self.polygon(p) }? {
                made.push(g);
            }
        }
        if kind == 6 && made.len() == 1 {
            return Ok(made[0]);
        }
        let g = unsafe { (a.GEOSGeom_createCollection_r)(self.ctx, kind, made.as_mut_ptr(), made.len() as c_uint) };
        if g.is_null() { Err("GEOS could not make a collection".into()) } else { Ok(g) }
    }

    unsafe fn coords(&self, ring: G) -> Result<Ring> {
        let a = self.a;
        let cs = unsafe { (a.GEOSGeom_getCoordSeq_r)(self.ctx, ring) };
        let mut n: c_uint = 0;
        if cs.is_null() || unsafe { (a.GEOSCoordSeq_getSize_r)(self.ctx, cs, &mut n) } == 0 {
            return Err("GEOS could not read a ring".into());
        }
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            let (mut x, mut y) = (0.0, 0.0);
            if unsafe { (a.GEOSCoordSeq_getXY_r)(self.ctx, cs, i, &mut x, &mut y) } == 0 {
                return Err("GEOS could not read a vertex".into());
            }
            out.push([x, y]);
        }
        Ok(out)
    }

    /// The polygons of a geometry in GEOS's order (the points and lines of a collection left out).
    unsafe fn read(&self, g: G, out: &mut Vec<Poly>) -> Result<()> {
        let a = self.a;
        match unsafe { (a.GEOSGeomTypeId_r)(self.ctx, g) } {
            3 => {
                if unsafe { (a.GEOSisEmpty_r)(self.ctx, g) } == 1 {
                    return Ok(());
                }
                let mut rings = vec![unsafe { self.coords((a.GEOSGetExteriorRing_r)(self.ctx, g)) }?];
                for k in 0..unsafe { (a.GEOSGetNumInteriorRings_r)(self.ctx, g) }.max(0) {
                    rings.push(unsafe { self.coords((a.GEOSGetInteriorRingN_r)(self.ctx, g, k)) }?);
                }
                out.push(rings);
            }
            6 | 7 => {
                for k in 0..unsafe { (a.GEOSGetNumGeometries_r)(self.ctx, g) }.max(0) {
                    unsafe { self.read((a.GEOSGetGeometryN_r)(self.ctx, g, k), out) }?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    unsafe fn destroy(&self, g: G) {
        unsafe { (self.a.GEOSGeom_destroy_r)(self.ctx, g) }
    }
}

impl Geos {
    fn session(&self) -> Result<Session<'_>> {
        let ctx = unsafe { (self.api.GEOS_init_r)() };
        if ctx.is_null() { Err("GEOS could not start".into()) } else { Ok(Session { a: &self.api, ctx }) }
    }

    /// The convex hull of each set of points (indices into `xy`) buffered by `width` with `quad_segs` segments per
    /// quarter circle (shapely's `buffer(convex_hull(MultiPoint(...)), width, quad_segs=...)`): their polygons.
    pub fn hull_buffers(
        &self,
        xy: &[[f64; 2]],
        sets: &[Vec<usize>],
        width: f64,
        quad_segs: i32,
    ) -> Result<Vec<Vec<Poly>>> {
        let s = self.session()?;
        let a = s.a;
        unsafe {
            let params = (a.GEOSBufferParams_create_r)(s.ctx);
            if params.is_null() {
                return Err("GEOS could not make the buffer's parameters".into());
            }
            (a.GEOSBufferParams_setQuadrantSegments_r)(s.ctx, params, quad_segs);
            let result = (|| -> Result<Vec<Vec<Poly>>> {
                let mut out = Vec::with_capacity(sets.len());
                for set in sets {
                    let mut pts: Vec<G> = Vec::with_capacity(set.len());
                    for &i in set {
                        pts.push((a.GEOSGeom_createPointFromXY_r)(s.ctx, xy[i][0], xy[i][1]));
                    }
                    if pts.iter().any(|p| p.is_null()) {
                        return Err("GEOS could not make the points".into());
                    }
                    let multi = (a.GEOSGeom_createCollection_r)(s.ctx, 4, pts.as_mut_ptr(), pts.len() as c_uint);
                    if multi.is_null() {
                        return Err("GEOS could not make the multipoint".into());
                    }
                    let hull = (a.GEOSConvexHull_r)(s.ctx, multi);
                    s.destroy(multi);
                    if hull.is_null() {
                        return Err("GEOS could not make the convex hull".into());
                    }
                    let buffered = (a.GEOSBufferWithParams_r)(s.ctx, hull, params, width);
                    s.destroy(hull);
                    if buffered.is_null() {
                        return Err("GEOS could not buffer a hull".into());
                    }
                    let mut polys = Vec::new();
                    let r = s.read(buffered, &mut polys);
                    s.destroy(buffered);
                    r?;
                    out.push(polys);
                }
                Ok(out)
            })();
            (a.GEOSBufferParams_destroy_r)(s.ctx, params);
            result
        }
    }

    /// shapely's `union_all` of polygons: GEOS's unary union of their collection, as polygons.
    pub fn union_all(&self, polys: &[Poly]) -> Result<Vec<Poly>> {
        let s = self.session()?;
        unsafe {
            let collection = s.shape(polys, 7)?;
            let u = (s.a.GEOSUnaryUnion_r)(s.ctx, collection);
            s.destroy(collection);
            if u.is_null() {
                return Err("GEOS could not make the union".into());
            }
            let mut out = Vec::new();
            let r = s.read(u, &mut out);
            s.destroy(u);
            r.map(|()| out)
        }
    }

    /// The polygons where two shapes (a polygon, or the polygons of a multipolygon) intersect; none when they do
    /// not intersect at all.
    pub fn intersection(&self, a: &[Poly], b: &[Poly]) -> Result<Option<Vec<Poly>>> {
        let s = self.session()?;
        unsafe {
            let (ga, gb) = (s.shape(a, 6)?, s.shape(b, 6)?);
            let result = (|| -> Result<Option<Vec<Poly>>> {
                match (s.a.GEOSIntersects_r)(s.ctx, ga, gb) {
                    0 => return Ok(None),
                    1 => {}
                    _ => return Err("GEOS could not test the intersection".into()),
                }
                let g = (s.a.GEOSIntersection_r)(s.ctx, ga, gb);
                if g.is_null() {
                    return Err("GEOS could not intersect the polygons".into());
                }
                let mut out = Vec::new();
                let r = s.read(g, &mut out);
                s.destroy(g);
                r.map(|()| Some(out))
            })();
            s.destroy(ga);
            s.destroy(gb);
            result
        }
    }
}
