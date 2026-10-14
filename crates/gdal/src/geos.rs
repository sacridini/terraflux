//! GEOS's own C API, for what GDAL does not pass on (the Voronoi diagram): the geos_c library next to GDAL's (the
//! one GDAL itself loads), else the system's, loaded when first needed.

use std::ffi::{CStr, c_char, c_double, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::Result;

type Ctx = *mut c_void;
type G = *mut c_void;

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
}

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
