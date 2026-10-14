//! GDAL for Terraflux, loaded when the program runs (no GDAL is needed to build Terraflux): every raster format
//! GDAL reads, read in windows at any resolution (its overviews used), and GDAL's warping (reprojecting,
//! resampling, clipping) and translating.
//!
//! GDAL is found, in order:
//! - in `TERRAFLUX_GDAL` (a folder, or the library itself);
//! - in the folder `gdal` next to the program (the installers bring it there, with its data), or in macOS's
//!   application bundle, `Contents/Frameworks/gdal` (its data in `Contents/Resources`);
//! - next to the program, in the `PATH`, and where GDAL is usually installed.
//!
//! Its data (GDAL's and PROJ's) and its plugins are looked for next to the library. Without GDAL, Terraflux
//! reads and writes GeoTIFF by itself; [`gdal`] says why GDAL could not be had.

use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use terraflux_raster::vector::{Column, Geometry, Vector};
use terraflux_raster::{Band, DataType, GeoTags, Grid, Raster};

pub type Result<T> = std::result::Result<T, String>;

type H = *mut c_void;

/// The functions of GDAL's C API that Terraflux uses.
#[allow(non_snake_case)]
struct Api {
    GDALAllRegister: unsafe extern "C" fn(),
    GDALVersionInfo: unsafe extern "C" fn(*const c_char) -> *const c_char,
    GDALOpenEx: unsafe extern "C" fn(
        *const c_char,
        c_uint,
        *const *const c_char,
        *const *const c_char,
        *const *const c_char,
    ) -> H,
    GDALClose: unsafe extern "C" fn(H) -> c_int,
    GDALGetRasterXSize: unsafe extern "C" fn(H) -> c_int,
    GDALGetRasterYSize: unsafe extern "C" fn(H) -> c_int,
    GDALGetRasterCount: unsafe extern "C" fn(H) -> c_int,
    GDALGetGeoTransform: unsafe extern "C" fn(H, *mut f64) -> c_int,
    GDALGetProjectionRef: unsafe extern "C" fn(H) -> *const c_char,
    GDALSetProjection: unsafe extern "C" fn(H, *const c_char) -> c_int,
    GDALGetRasterBand: unsafe extern "C" fn(H, c_int) -> H,
    GDALGetRasterDataType: unsafe extern "C" fn(H) -> c_int,
    GDALGetRasterNoDataValue: unsafe extern "C" fn(H, *mut c_int) -> f64,
    GDALGetDescription: unsafe extern "C" fn(H) -> *const c_char,
    GDALGetOverviewCount: unsafe extern "C" fn(H) -> c_int,
    GDALGetDatasetDriver: unsafe extern "C" fn(H) -> H,
    GDALGetDriverShortName: unsafe extern "C" fn(H) -> *const c_char,
    GDALGetDriverCount: unsafe extern "C" fn() -> c_int,
    GDALRasterIO: unsafe extern "C" fn(
        H,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *mut c_void,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
    ) -> c_int,
    GDALWarpAppOptionsNew: unsafe extern "C" fn(*mut *mut c_char, *mut c_void) -> H,
    GDALWarpAppOptionsFree: unsafe extern "C" fn(H),
    GDALWarp: unsafe extern "C" fn(*const c_char, H, c_int, *mut H, H, *mut c_int) -> H,
    GDALTranslateOptionsNew: unsafe extern "C" fn(*mut *mut c_char, *mut c_void) -> H,
    GDALTranslateOptionsFree: unsafe extern "C" fn(H),
    GDALTranslate: unsafe extern "C" fn(*const c_char, H, H, *mut c_int) -> H,
    GDALDEMProcessingOptionsNew: unsafe extern "C" fn(*mut *mut c_char, *mut c_void) -> H,
    GDALDEMProcessingOptionsFree: unsafe extern "C" fn(H),
    GDALDEMProcessing: unsafe extern "C" fn(*const c_char, H, *const c_char, *const c_char, H, *mut c_int) -> H,
    GDALGetDriverByName: unsafe extern "C" fn(*const c_char) -> H,
    GDALCreate: unsafe extern "C" fn(H, *const c_char, c_int, c_int, c_int, c_int, *mut *mut c_char) -> H,
    GDALSetGeoTransform: unsafe extern "C" fn(H, *const f64) -> c_int,
    GDALSetRasterNoDataValue: unsafe extern "C" fn(H, f64) -> c_int,
    GDALComputeProximity: unsafe extern "C" fn(H, H, *mut *mut c_char, *const c_void, *mut c_void) -> c_int,
    GDALFillNodata:
        unsafe extern "C" fn(H, H, f64, c_int, c_int, *mut *mut c_char, *const c_void, *mut c_void) -> c_int,
    GDALDatasetGetLayerCount: unsafe extern "C" fn(H) -> c_int,
    GDALDatasetGetLayer: unsafe extern "C" fn(H, c_int) -> H,
    GDALDatasetGetLayerByName: unsafe extern "C" fn(H, *const c_char) -> H,
    GDALDatasetCreateLayer: unsafe extern "C" fn(H, *const c_char, H, c_int, *mut *mut c_char) -> H,
    OGR_L_GetName: unsafe extern "C" fn(H) -> *const c_char,
    OGR_L_ResetReading: unsafe extern "C" fn(H),
    OGR_L_GetNextFeature: unsafe extern "C" fn(H) -> H,
    OGR_L_GetLayerDefn: unsafe extern "C" fn(H) -> H,
    OGR_L_GetSpatialRef: unsafe extern "C" fn(H) -> H,
    OGR_L_CreateField: unsafe extern "C" fn(H, H, c_int) -> c_int,
    OGR_L_CreateFeature: unsafe extern "C" fn(H, H) -> c_int,
    OGR_FD_GetFieldCount: unsafe extern "C" fn(H) -> c_int,
    OGR_FD_GetFieldDefn: unsafe extern "C" fn(H, c_int) -> H,
    OGR_Fld_GetNameRef: unsafe extern "C" fn(H) -> *const c_char,
    OGR_Fld_GetType: unsafe extern "C" fn(H) -> c_int,
    OGR_Fld_Create: unsafe extern "C" fn(*const c_char, c_int) -> H,
    OGR_Fld_Destroy: unsafe extern "C" fn(H),
    OGR_F_Create: unsafe extern "C" fn(H) -> H,
    OGR_F_Destroy: unsafe extern "C" fn(H),
    OGR_F_IsFieldSetAndNotNull: unsafe extern "C" fn(H, c_int) -> c_int,
    OGR_F_GetFieldAsDouble: unsafe extern "C" fn(H, c_int) -> f64,
    OGR_F_GetFieldAsString: unsafe extern "C" fn(H, c_int) -> *const c_char,
    OGR_F_SetFieldDouble: unsafe extern "C" fn(H, c_int, f64),
    OGR_F_SetFieldString: unsafe extern "C" fn(H, c_int, *const c_char),
    OGR_F_GetGeometryRef: unsafe extern "C" fn(H) -> H,
    OGR_F_SetGeometryDirectly: unsafe extern "C" fn(H, H) -> c_int,
    OGR_G_WkbSize: unsafe extern "C" fn(H) -> c_int,
    OGR_G_ExportToIsoWkb: unsafe extern "C" fn(H, c_int, *mut u8) -> c_int,
    OGR_G_CreateFromWkb: unsafe extern "C" fn(*const u8, H, *mut H, c_int) -> c_int,
    OGR_G_DestroyGeometry: unsafe extern "C" fn(H),
    OGR_G_Transform: unsafe extern "C" fn(H, H) -> c_int,
    OGR_G_AssignSpatialReference: unsafe extern "C" fn(H, H),
    OGR_G_Buffer: unsafe extern "C" fn(H, f64, c_int) -> H,
    OGR_G_Intersection: unsafe extern "C" fn(H, H) -> H,
    OGR_G_UnionCascaded: unsafe extern "C" fn(H) -> H,
    /// GDAL 3.7 and later
    OGR_G_UnaryUnion: Option<unsafe extern "C" fn(H) -> H>,
    OGR_G_IsEmpty: unsafe extern "C" fn(H) -> c_int,
    OGR_G_Distance: unsafe extern "C" fn(H, H) -> f64,
    OGR_G_CreateGeometry: unsafe extern "C" fn(c_int) -> H,
    OGR_G_Centroid: unsafe extern "C" fn(H, H) -> c_int,
    OGR_G_GetX: unsafe extern "C" fn(H, c_int) -> f64,
    OGR_G_GetY: unsafe extern "C" fn(H, c_int) -> f64,
    OGR_G_PointOnSurface: unsafe extern "C" fn(H) -> H,
    OGR_G_Within: unsafe extern "C" fn(H, H) -> c_int,
    OGR_G_Intersects: unsafe extern "C" fn(H, H) -> c_int,
    OGR_G_Difference: unsafe extern "C" fn(H, H) -> H,
    OGR_G_MakeValid: unsafe extern "C" fn(H) -> H,
    OGR_G_ConvexHull: unsafe extern "C" fn(H) -> H,
    /// GDAL 3.9 and later
    OGR_G_GeodesicArea: Option<unsafe extern "C" fn(H) -> f64>,
    OSRNewSpatialReference: unsafe extern "C" fn(*const c_char) -> H,
    OSRDestroySpatialReference: unsafe extern "C" fn(H),
    OSRSetFromUserInput: unsafe extern "C" fn(H, *const c_char) -> c_int,
    OSRExportToWkt: unsafe extern "C" fn(H, *mut *mut c_char) -> c_int,
    OSRSetAxisMappingStrategy: unsafe extern "C" fn(H, c_int),
    OSRIsSame: unsafe extern "C" fn(H, H) -> c_int,
    OSRIsGeographic: unsafe extern "C" fn(H) -> c_int,
    OCTNewCoordinateTransformation: unsafe extern "C" fn(H, H) -> H,
    OCTDestroyCoordinateTransformation: unsafe extern "C" fn(H),
    OCTTransformEx: unsafe extern "C" fn(H, c_int, *mut f64, *mut f64, *mut f64, *mut c_int) -> c_int,
    CPLHTTPFetch: unsafe extern "C" fn(*const c_char, *const *const c_char) -> *mut HttpResult,
    CPLHTTPDestroyResult: unsafe extern "C" fn(*mut HttpResult),
    VSIFileFromMemBuffer: unsafe extern "C" fn(*const c_char, *mut u8, u64, c_int) -> H,
    VSIFCloseL: unsafe extern "C" fn(H) -> c_int,
    VSIFOpenL: unsafe extern "C" fn(*const c_char, *const c_char) -> H,
    VSIFReadL: unsafe extern "C" fn(*mut c_void, usize, usize, H) -> usize,
    VSIReadDir: unsafe extern "C" fn(*const c_char) -> *mut *mut c_char,
    CSLDestroy: unsafe extern "C" fn(*mut *mut c_char),
    VSIUnlink: unsafe extern "C" fn(*const c_char) -> c_int,
    GDALGetRasterColorTable: unsafe extern "C" fn(H) -> H,
    GDALGetColorEntryCount: unsafe extern "C" fn(H) -> c_int,
    GDALGetColorEntry: unsafe extern "C" fn(H, c_int) -> *const [i16; 4],
    VSIFree: unsafe extern "C" fn(*mut c_void),
    GDALRasterizeGeometries: unsafe extern "C" fn(
        H,
        c_int,
        *const c_int,
        c_int,
        *const H,
        *const c_void,
        *mut c_void,
        *const f64,
        *mut *mut c_char,
        *const c_void,
        *mut c_void,
    ) -> c_int,
    CPLGetLastErrorMsg: unsafe extern "C" fn() -> *const c_char,
    CPLErrorReset: unsafe extern "C" fn(),
    CPLSetConfigOption: unsafe extern "C" fn(*const c_char, *const c_char),
    CPLSetErrorHandler: unsafe extern "C" fn(*const c_void) -> *const c_void,
    CPLQuietErrorHandler: *const c_void,
    OSRSetPROJSearchPaths: unsafe extern "C" fn(*const *const c_char),
}

/// cpl_http.h's CPLHTTPResult.
#[repr(C)]
struct HttpResult {
    status: c_int,
    content_type: *mut c_char,
    error: *mut c_char,
    len: c_int,
    alloc: c_int,
    data: *mut u8,
    headers: *mut *mut c_char,
    mime_parts: c_int,
    mime: *mut c_void,
}

/// GDAL loaded: its library, functions, version and folder.
pub struct Gdal {
    _lib: libloading::Library,
    api: Api,
    /// GDAL's version, e.g. "3.12.1".
    pub version: String,
    /// Where the library was found.
    pub library: PathBuf,
    /// The drivers registered.
    pub drivers: usize,
}

// the functions are plain C functions of a library kept loaded for the program's life
unsafe impl Send for Gdal {}
unsafe impl Sync for Gdal {}

/// The libraries that may be GDAL's in a folder.
fn libraries_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            if cfg!(windows) {
                // gdal.dll (GISInternals, conda), gdal312.dll (OSGeo4W); not its bindings
                n.starts_with("gdal") && n.ends_with(".dll") && n[4..n.len() - 4].chars().all(|c| c.is_ascii_digit())
            } else if cfg!(target_os = "macos") {
                n.starts_with("libgdal") && n.ends_with(".dylib")
            } else {
                n.starts_with("libgdal.so")
            }
        })
        .collect();
    v.sort();
    v
}

/// Where to look for GDAL, in order.
fn candidates() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("TERRAFLUX_GDAL").map(PathBuf::from) {
        if p.is_file() { files.push(p) } else { dirs.push(p) }
    }
    // the program's own folder, through a link to it (/usr/local/bin/terraflux to the application's)
    let exe = std::env::current_exe().ok().map(|e| if cfg!(windows) { e } else { e.canonicalize().unwrap_or(e) });
    if let Some(exe) = exe.and_then(|e| e.parent().map(Path::to_path_buf)) {
        dirs.push(exe.join("gdal"));
        dirs.push(exe.join("gdal").join("bin"));
        if cfg!(target_os = "macos") {
            // Terraflux.app/Contents/MacOS/terraflux: Contents/Frameworks/gdal
            dirs.push(exe.join("..").join("Frameworks").join("gdal"));
        }
        dirs.push(exe.clone());
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if cfg!(windows) {
        for d in
            ["C:\\Program Files\\GDAL", "C:\\OSGeo4W\\bin", "C:\\OSGeo4W64\\bin", "C:\\Program Files\\QGIS 3.40\\bin"]
        {
            dirs.push(PathBuf::from(d));
        }
    } else {
        for d in ["/usr/lib", "/usr/lib/x86_64-linux-gnu", "/usr/local/lib", "/opt/homebrew/lib", "/opt/local/lib"] {
            dirs.push(PathBuf::from(d));
        }
    }
    for d in dirs {
        files.extend(libraries_in(&d));
    }
    files
}

fn load_library(path: &Path) -> Result<libloading::Library> {
    #[cfg(windows)]
    {
        // its dependencies (PROJ, curl, ...) from its own folder
        const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;
        unsafe { libloading::os::windows::Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) }
            .map(Into::into)
            .map_err(|e| format!("{}: {e}", path.display()))
    }
    #[cfg(not(windows))]
    {
        unsafe { libloading::Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))
    }
}

fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap()
}

fn text(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
}

impl Gdal {
    fn load(path: &Path) -> Result<Gdal> {
        let lib = load_library(path)?;
        macro_rules! f {
            ($name:ident) => {
                *unsafe { lib.get(concat!(stringify!($name), "\0").as_bytes()) }
                    .map_err(|e| format!("{}: no {}: {e}", path.display(), stringify!($name)))?
            };
        }
        let quiet: libloading::Symbol<*const c_void> =
            unsafe { lib.get(b"CPLQuietErrorHandler\0") }.map_err(|e| format!("{}: {e}", path.display()))?;
        let api = Api {
            GDALAllRegister: f!(GDALAllRegister),
            GDALVersionInfo: f!(GDALVersionInfo),
            GDALOpenEx: f!(GDALOpenEx),
            GDALClose: f!(GDALClose),
            GDALGetRasterXSize: f!(GDALGetRasterXSize),
            GDALGetRasterYSize: f!(GDALGetRasterYSize),
            GDALGetRasterCount: f!(GDALGetRasterCount),
            GDALGetGeoTransform: f!(GDALGetGeoTransform),
            GDALGetProjectionRef: f!(GDALGetProjectionRef),
            GDALSetProjection: f!(GDALSetProjection),
            GDALGetRasterBand: f!(GDALGetRasterBand),
            GDALGetRasterDataType: f!(GDALGetRasterDataType),
            GDALGetRasterNoDataValue: f!(GDALGetRasterNoDataValue),
            GDALGetDescription: f!(GDALGetDescription),
            GDALGetOverviewCount: f!(GDALGetOverviewCount),
            GDALGetDatasetDriver: f!(GDALGetDatasetDriver),
            GDALGetDriverShortName: f!(GDALGetDriverShortName),
            GDALGetDriverCount: f!(GDALGetDriverCount),
            GDALRasterIO: f!(GDALRasterIO),
            GDALWarpAppOptionsNew: f!(GDALWarpAppOptionsNew),
            GDALWarpAppOptionsFree: f!(GDALWarpAppOptionsFree),
            GDALWarp: f!(GDALWarp),
            GDALTranslateOptionsNew: f!(GDALTranslateOptionsNew),
            GDALTranslateOptionsFree: f!(GDALTranslateOptionsFree),
            GDALTranslate: f!(GDALTranslate),
            GDALDEMProcessingOptionsNew: f!(GDALDEMProcessingOptionsNew),
            GDALDEMProcessingOptionsFree: f!(GDALDEMProcessingOptionsFree),
            GDALDEMProcessing: f!(GDALDEMProcessing),
            GDALGetDriverByName: f!(GDALGetDriverByName),
            GDALCreate: f!(GDALCreate),
            GDALSetGeoTransform: f!(GDALSetGeoTransform),
            GDALSetRasterNoDataValue: f!(GDALSetRasterNoDataValue),
            GDALComputeProximity: f!(GDALComputeProximity),
            GDALFillNodata: f!(GDALFillNodata),
            GDALDatasetGetLayerCount: f!(GDALDatasetGetLayerCount),
            GDALDatasetGetLayer: f!(GDALDatasetGetLayer),
            GDALDatasetGetLayerByName: f!(GDALDatasetGetLayerByName),
            GDALDatasetCreateLayer: f!(GDALDatasetCreateLayer),
            OGR_L_GetName: f!(OGR_L_GetName),
            OGR_L_ResetReading: f!(OGR_L_ResetReading),
            OGR_L_GetNextFeature: f!(OGR_L_GetNextFeature),
            OGR_L_GetLayerDefn: f!(OGR_L_GetLayerDefn),
            OGR_L_GetSpatialRef: f!(OGR_L_GetSpatialRef),
            OGR_L_CreateField: f!(OGR_L_CreateField),
            OGR_L_CreateFeature: f!(OGR_L_CreateFeature),
            OGR_FD_GetFieldCount: f!(OGR_FD_GetFieldCount),
            OGR_FD_GetFieldDefn: f!(OGR_FD_GetFieldDefn),
            OGR_Fld_GetNameRef: f!(OGR_Fld_GetNameRef),
            OGR_Fld_GetType: f!(OGR_Fld_GetType),
            OGR_Fld_Create: f!(OGR_Fld_Create),
            OGR_Fld_Destroy: f!(OGR_Fld_Destroy),
            OGR_F_Create: f!(OGR_F_Create),
            OGR_F_Destroy: f!(OGR_F_Destroy),
            OGR_F_IsFieldSetAndNotNull: f!(OGR_F_IsFieldSetAndNotNull),
            OGR_F_GetFieldAsDouble: f!(OGR_F_GetFieldAsDouble),
            OGR_F_GetFieldAsString: f!(OGR_F_GetFieldAsString),
            OGR_F_SetFieldDouble: f!(OGR_F_SetFieldDouble),
            OGR_F_SetFieldString: f!(OGR_F_SetFieldString),
            OGR_F_GetGeometryRef: f!(OGR_F_GetGeometryRef),
            OGR_F_SetGeometryDirectly: f!(OGR_F_SetGeometryDirectly),
            OGR_G_WkbSize: f!(OGR_G_WkbSize),
            OGR_G_ExportToIsoWkb: f!(OGR_G_ExportToIsoWkb),
            OGR_G_CreateFromWkb: f!(OGR_G_CreateFromWkb),
            OGR_G_DestroyGeometry: f!(OGR_G_DestroyGeometry),
            OGR_G_Transform: f!(OGR_G_Transform),
            OGR_G_AssignSpatialReference: f!(OGR_G_AssignSpatialReference),
            OGR_G_Buffer: f!(OGR_G_Buffer),
            OGR_G_Intersection: f!(OGR_G_Intersection),
            OGR_G_UnionCascaded: f!(OGR_G_UnionCascaded),
            OGR_G_UnaryUnion: unsafe { lib.get(b"OGR_G_UnaryUnion") }.ok().map(|s| *s),
            OGR_G_IsEmpty: f!(OGR_G_IsEmpty),
            OGR_G_Distance: f!(OGR_G_Distance),
            OGR_G_CreateGeometry: f!(OGR_G_CreateGeometry),
            OGR_G_Centroid: f!(OGR_G_Centroid),
            OGR_G_GetX: f!(OGR_G_GetX),
            OGR_G_GetY: f!(OGR_G_GetY),
            OGR_G_PointOnSurface: f!(OGR_G_PointOnSurface),
            OGR_G_Within: f!(OGR_G_Within),
            OGR_G_Intersects: f!(OGR_G_Intersects),
            OGR_G_Difference: f!(OGR_G_Difference),
            OGR_G_MakeValid: f!(OGR_G_MakeValid),
            OGR_G_ConvexHull: f!(OGR_G_ConvexHull),
            OGR_G_GeodesicArea: unsafe { lib.get(b"OGR_G_GeodesicArea ") }.ok().map(|s| *s),
            OSRNewSpatialReference: f!(OSRNewSpatialReference),
            OSRDestroySpatialReference: f!(OSRDestroySpatialReference),
            OSRSetFromUserInput: f!(OSRSetFromUserInput),
            OSRExportToWkt: f!(OSRExportToWkt),
            OSRSetAxisMappingStrategy: f!(OSRSetAxisMappingStrategy),
            OSRIsSame: f!(OSRIsSame),
            OSRIsGeographic: f!(OSRIsGeographic),
            OCTNewCoordinateTransformation: f!(OCTNewCoordinateTransformation),
            OCTDestroyCoordinateTransformation: f!(OCTDestroyCoordinateTransformation),
            OCTTransformEx: f!(OCTTransformEx),
            CPLHTTPFetch: f!(CPLHTTPFetch),
            CPLHTTPDestroyResult: f!(CPLHTTPDestroyResult),
            VSIFileFromMemBuffer: f!(VSIFileFromMemBuffer),
            VSIFCloseL: f!(VSIFCloseL),
            VSIFOpenL: f!(VSIFOpenL),
            VSIFReadL: f!(VSIFReadL),
            VSIReadDir: f!(VSIReadDir),
            CSLDestroy: f!(CSLDestroy),
            VSIUnlink: f!(VSIUnlink),
            GDALGetRasterColorTable: f!(GDALGetRasterColorTable),
            GDALGetColorEntryCount: f!(GDALGetColorEntryCount),
            GDALGetColorEntry: f!(GDALGetColorEntry),
            VSIFree: f!(VSIFree),
            GDALRasterizeGeometries: f!(GDALRasterizeGeometries),
            CPLGetLastErrorMsg: f!(CPLGetLastErrorMsg),
            CPLErrorReset: f!(CPLErrorReset),
            CPLSetConfigOption: f!(CPLSetConfigOption),
            CPLSetErrorHandler: f!(CPLSetErrorHandler),
            CPLQuietErrorHandler: *quiet,
            OSRSetPROJSearchPaths: f!(OSRSetPROJSearchPaths),
        };
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
        let first = |names: &[&str]| names.iter().map(|n| dir.join(n)).find(|p| p.is_dir());
        unsafe {
            // errors are read, not printed
            (api.CPLSetErrorHandler)(api.CPLQuietErrorHandler);
            // the last ones from macOS's application bundle's Contents/Frameworks/gdal
            if let Some(d) = first(&["gdal-data", "../share/gdal", "share/gdal", "data", "../../Resources/gdal-data"]) {
                (api.CPLSetConfigOption)(cstr("GDAL_DATA").as_ptr(), cstr(&d.to_string_lossy()).as_ptr());
            }
            if let Some(d) = first(&["gdalplugins", "../lib/gdalplugins", "gdal/plugins"]) {
                (api.CPLSetConfigOption)(cstr("GDAL_DRIVER_PATH").as_ptr(), cstr(&d.to_string_lossy()).as_ptr());
            }
            if let Some(d) =
                first(&["projlib", "proj-data", "../share/proj", "share/proj", "proj", "../../Resources/projlib"])
            {
                let p = cstr(&d.to_string_lossy());
                let paths = [p.as_ptr(), std::ptr::null()];
                (api.OSRSetPROJSearchPaths)(paths.as_ptr());
            }
            (api.CPLSetConfigOption)(cstr("GDAL_PAM_ENABLED").as_ptr(), cstr("NO").as_ptr());
            // no .properties files written next to the gzipped files read (/vsigzip/)
            (api.CPLSetConfigOption)(cstr("CPL_VSIL_GZIP_WRITE_PROPERTIES").as_ptr(), cstr("NO").as_ptr());
            // the web (map services): its certificates next to the library, and who asks
            if let Some(c) = ["curl-ca-bundle.crt", "../share/curl/curl-ca-bundle.crt"]
                .iter()
                .map(|n| dir.join(n))
                .find(|p| p.is_file())
            {
                (api.CPLSetConfigOption)(cstr("CURL_CA_BUNDLE").as_ptr(), cstr(&c.to_string_lossy()).as_ptr());
            }
            let agent = format!("Terraflux/{} (https://github.com/sacridini/terraflux)", env!("CARGO_PKG_VERSION"));
            (api.CPLSetConfigOption)(cstr("GDAL_HTTP_USERAGENT").as_ptr(), cstr(&agent).as_ptr());
            (api.GDALAllRegister)();
        }
        let version = text(unsafe { (api.GDALVersionInfo)(cstr("RELEASE_NAME").as_ptr()) });
        let drivers = unsafe { (api.GDALGetDriverCount)() } as usize;
        Ok(Gdal { _lib: lib, api, version, library: path.to_path_buf(), drivers })
    }

    fn last_error(&self, what: &str) -> String {
        let m = text(unsafe { (self.api.CPLGetLastErrorMsg)() });
        if m.is_empty() { what.to_string() } else { format!("{what}: {m}") }
    }

    fn open_handle(&self, path: &str, update: bool) -> Result<H> {
        // GDAL_OF_RASTER, and GDAL_OF_UPDATE to change it
        let flags: c_uint = 0x02 | if update { 0x01 } else { 0 };
        unsafe { (self.api.CPLErrorReset)() };
        let h = unsafe {
            (self.api.GDALOpenEx)(cstr(path).as_ptr(), flags, std::ptr::null(), std::ptr::null(), std::ptr::null())
        };
        if h.is_null() { Err(self.last_error(&format!("{path}: GDAL cannot open it"))) } else { Ok(h) }
    }
}

static GDAL: OnceLock<Result<Gdal>> = OnceLock::new();

/// GDAL, loaded the first time it is asked for; why it could not be, otherwise.
pub fn gdal() -> std::result::Result<&'static Gdal, &'static str> {
    GDAL.get_or_init(|| {
        let mut tried = Vec::new();
        for c in candidates() {
            match Gdal::load(&c) {
                Ok(g) => return Ok(g),
                Err(e) => tried.push(e),
            }
        }
        Err(if tried.is_empty() {
            "GDAL was not found (install it, or set TERRAFLUX_GDAL to its folder)".to_string()
        } else {
            format!("GDAL could not be loaded: {}", tried.join("; "))
        })
    })
    .as_ref()
    .map_err(String::as_str)
}

/// GDAL's data types, as Terraflux's.
fn data_type(t: c_int) -> DataType {
    match t {
        1 => DataType::U8,
        14 => DataType::I8,
        2 => DataType::U16,
        3 => DataType::I16,
        4 => DataType::U32,
        5 => DataType::I32,
        12 => DataType::U64,
        13 => DataType::I64,
        6 => DataType::F32,
        _ => DataType::F64,
    }
}

struct Handle(H);
unsafe impl Send for Handle {}

/// A raster file opened by GDAL: its header, and its cells read in windows; a handle per thread reading at once
/// (GDAL's datasets are not shared between threads).
pub struct Dataset {
    path: String,
    handles: Mutex<Vec<Handle>>,
    /// The raster without cells: grid (the CRS as WKT), bands' names, type, no-data value.
    pub meta: Raster,
    /// GDAL's driver (GTiff, netCDF, JP2OpenJPEG...) and the overviews of the first band.
    pub driver: String,
    pub overviews: usize,
}

impl Dataset {
    pub fn open(path: impl AsRef<Path>) -> Result<Dataset> {
        let g = gdal().map_err(String::from)?;
        let path = path.as_ref().to_string_lossy().into_owned();
        let h = g.open_handle(&path, false)?;
        let a = &g.api;
        let (w, ht, n) = unsafe { ((a.GDALGetRasterXSize)(h), (a.GDALGetRasterYSize)(h), (a.GDALGetRasterCount)(h)) };
        if n < 1 {
            unsafe { (a.GDALClose)(h) };
            return Err(format!("{path}: no band"));
        }
        let mut transform = [0.0, 1.0, 0.0, 0.0, 0.0, -1.0];
        unsafe { (a.GDALGetGeoTransform)(h, transform.as_mut_ptr()) };
        let wkt = text(unsafe { (a.GDALGetProjectionRef)(h) });
        let mut bands = Vec::new();
        let (mut dtype, mut nodata, mut overviews) = (DataType::F64, None, 0);
        for b in 1..=n {
            let band = unsafe { (a.GDALGetRasterBand)(h, b) };
            let name = text(unsafe { (a.GDALGetDescription)(band) });
            bands.push(Band { name: (!name.is_empty()).then_some(name), data: Vec::new() });
            if b == 1 {
                dtype = data_type(unsafe { (a.GDALGetRasterDataType)(band) });
                let mut ok: c_int = 0;
                let nd = unsafe { (a.GDALGetRasterNoDataValue)(band, &mut ok) };
                nodata = (ok != 0).then_some(nd);
                overviews = unsafe { (a.GDALGetOverviewCount)(band) } as usize;
            }
        }
        let driver = text(unsafe { (a.GDALGetDriverShortName)((a.GDALGetDatasetDriver)(h)) });
        let geo = GeoTags { wkt: (!wkt.is_empty()).then_some(wkt), ..Default::default() };
        let meta =
            Raster { grid: Grid { width: w as usize, height: ht as usize, transform, geo }, bands, dtype, nodata };
        Ok(Dataset { path, handles: Mutex::new(vec![Handle(h)]), meta, driver, overviews })
    }

    /// The file.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Run with a handle of this thread's own (one kept, or opened).
    fn with<R>(&self, f: impl FnOnce(&Gdal, H) -> Result<R>) -> Result<R> {
        let g = gdal().map_err(String::from)?;
        let h = match self.handles.lock().unwrap().pop() {
            Some(h) => h.0,
            None => g.open_handle(&self.path, false)?,
        };
        let r = f(g, h);
        self.handles.lock().unwrap().push(Handle(h));
        r
    }

    /// The cells of a window of a band (0-based) resampled to `buf_w` x `buf_h` values (GDAL's nearest, from its
    /// overviews when the window is reduced), row by row, as f64; no-data values as they are.
    #[allow(clippy::too_many_arguments)] // GDAL's RasterIO: a window and the size it is read at
    pub fn read(
        &self,
        band: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        buf_w: usize,
        buf_h: usize,
    ) -> Result<Vec<f64>> {
        let mut out = vec![0f64; buf_w * buf_h];
        if w == 0 || h == 0 || buf_w == 0 || buf_h == 0 {
            return Ok(out);
        }
        self.with(|g, ds| {
            let a = &g.api;
            let b = unsafe { (a.GDALGetRasterBand)(ds, band as c_int + 1) };
            if b.is_null() {
                return Err(format!("{}: no band {band}", self.path));
            }
            unsafe { (a.CPLErrorReset)() };
            // GF_Read, GDT_Float64, packed
            let e = unsafe {
                (a.GDALRasterIO)(
                    b,
                    0,
                    x as c_int,
                    y as c_int,
                    w as c_int,
                    h as c_int,
                    out.as_mut_ptr() as *mut c_void,
                    buf_w as c_int,
                    buf_h as c_int,
                    7,
                    0,
                    0,
                )
            };
            if e >= 3 { Err(g.last_error(&format!("{}: reading", self.path))) } else { Ok(()) }
        })?;
        Ok(out)
    }
}

impl Drop for Dataset {
    fn drop(&mut self) {
        if let Ok(g) = gdal() {
            for h in self.handles.get_mut().unwrap().drain(..) {
                unsafe { (g.api.GDALClose)(h.0) };
            }
        }
    }
}

/// An argument list for GDAL's utilities (argv, null-terminated), kept alive while used.
struct Argv {
    _strings: Vec<CString>,
    ptrs: Vec<*mut c_char>,
}

impl Argv {
    fn new(args: &[String]) -> Argv {
        let strings: Vec<CString> = args.iter().map(|a| cstr(a)).collect();
        let mut ptrs: Vec<*mut c_char> = strings.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        ptrs.push(std::ptr::null_mut());
        Argv { _strings: strings, ptrs }
    }
}

/// `gdalwarp`: rasters warped into a new file (`args` as on the command line, without the files: `-t_srs
/// EPSG:4326 -tr 30 30 -r mode`...).
pub fn warp(sources: &[&Path], dest: &Path, args: &[String]) -> Result<()> {
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let mut srcs = Vec::new();
    for s in sources {
        match g.open_handle(&s.to_string_lossy(), false) {
            Ok(h) => srcs.push(h),
            Err(e) => {
                for h in srcs {
                    unsafe { (a.GDALClose)(h) };
                }
                return Err(e);
            }
        }
    }
    let mut argv = Argv::new(args);
    let opts = unsafe { (a.GDALWarpAppOptionsNew)(argv.ptrs.as_mut_ptr(), std::ptr::null_mut()) };
    let r = if opts.is_null() {
        Err(g.last_error("gdalwarp: the options"))
    } else {
        unsafe { (a.CPLErrorReset)() };
        let mut usage: c_int = 0;
        let out = unsafe {
            (a.GDALWarp)(
                cstr(&dest.to_string_lossy()).as_ptr(),
                std::ptr::null_mut(),
                srcs.len() as c_int,
                srcs.as_mut_ptr(),
                opts,
                &mut usage,
            )
        };
        unsafe { (a.GDALWarpAppOptionsFree)(opts) };
        if out.is_null() {
            Err(g.last_error("gdalwarp"))
        } else {
            unsafe { (a.GDALClose)(out) };
            Ok(())
        }
    };
    for h in srcs {
        unsafe { (a.GDALClose)(h) };
    }
    r
}

/// `gdal_translate`: a raster copied into a new file (`args` as on the command line, without the files:
/// `-projwin ...`, `-of COG`, `-ot Byte`...).
pub fn translate(source: &Path, dest: &Path, args: &[String]) -> Result<()> {
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let src = g.open_handle(&source.to_string_lossy(), false)?;
    let mut argv = Argv::new(args);
    let opts = unsafe { (a.GDALTranslateOptionsNew)(argv.ptrs.as_mut_ptr(), std::ptr::null_mut()) };
    let r = if opts.is_null() {
        Err(g.last_error("gdal_translate: the options"))
    } else {
        unsafe { (a.CPLErrorReset)() };
        let mut usage: c_int = 0;
        let out = unsafe { (a.GDALTranslate)(cstr(&dest.to_string_lossy()).as_ptr(), src, opts, &mut usage) };
        unsafe { (a.GDALTranslateOptionsFree)(opts) };
        if out.is_null() {
            Err(g.last_error("gdal_translate"))
        } else {
            unsafe { (a.GDALClose)(out) };
            Ok(())
        }
    };
    unsafe { (a.GDALClose)(src) };
    r
}

/// Give a file the coordinate reference system of a WKT (a GeoTIFF written without GeoTIFF keys).
pub fn set_crs(path: &Path, wkt: &str) -> Result<()> {
    let g = gdal().map_err(String::from)?;
    let h = g.open_handle(&path.to_string_lossy(), true)?;
    let e = unsafe { (g.api.GDALSetProjection)(h, cstr(wkt).as_ptr()) };
    unsafe { (g.api.GDALClose)(h) };
    if e >= 3 { Err(g.last_error("setting the CRS")) } else { Ok(()) }
}

/// `gdaldem`: a measure of a surface (`slope`, `aspect`, `hillshade`, `TRI`, `TPI`, `roughness`) into a new file
/// (`args` as on the command line: `-p` for slope in percent, `-s 111120` for degrees...).
pub fn dem(source: &Path, dest: &Path, measure: &str, args: &[String]) -> Result<()> {
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let src = g.open_handle(&source.to_string_lossy(), false)?;
    let mut argv = Argv::new(args);
    let opts = unsafe { (a.GDALDEMProcessingOptionsNew)(argv.ptrs.as_mut_ptr(), std::ptr::null_mut()) };
    let r = if opts.is_null() {
        Err(g.last_error("gdaldem: the options"))
    } else {
        unsafe { (a.CPLErrorReset)() };
        let mut usage: c_int = 0;
        let out = unsafe {
            (a.GDALDEMProcessing)(
                cstr(&dest.to_string_lossy()).as_ptr(),
                src,
                cstr(measure).as_ptr(),
                std::ptr::null(),
                opts,
                &mut usage,
            )
        };
        unsafe { (a.GDALDEMProcessingOptionsFree)(opts) };
        if out.is_null() {
            Err(g.last_error("gdaldem"))
        } else {
            unsafe { (a.GDALClose)(out) };
            Ok(())
        }
    };
    unsafe { (a.GDALClose)(src) };
    r
}

/// A new GeoTIFF of one band of Float32 on the grid of a dataset (deflated, tiled), its no-data value given.
fn create_like(g: &Gdal, like: H, dest: &Path, nodata: f64) -> Result<H> {
    let a = &g.api;
    let driver = unsafe { (a.GDALGetDriverByName)(cstr("GTiff").as_ptr()) };
    if driver.is_null() {
        return Err("GDAL has no GTiff driver".into());
    }
    let (w, h) = unsafe { ((a.GDALGetRasterXSize)(like), (a.GDALGetRasterYSize)(like)) };
    let options: Vec<String> =
        ["COMPRESS=DEFLATE", "TILED=YES", "BIGTIFF=IF_SAFER"].iter().map(|s| s.to_string()).collect();
    let mut argv = Argv::new(&options);
    let out =
        unsafe { (a.GDALCreate)(driver, cstr(&dest.to_string_lossy()).as_ptr(), w, h, 1, 6, argv.ptrs.as_mut_ptr()) };
    if out.is_null() {
        return Err(g.last_error(&format!("{}: GDAL cannot create it", dest.display())));
    }
    let mut t = [0.0; 6];
    unsafe {
        (a.GDALGetGeoTransform)(like, t.as_mut_ptr());
        (a.GDALSetGeoTransform)(out, t.as_ptr());
        (a.GDALSetProjection)(out, (a.GDALGetProjectionRef)(like));
        (a.GDALSetRasterNoDataValue)((a.GDALGetRasterBand)(out, 1), nodata);
    }
    Ok(out)
}

/// `gdal_proximity`: the distance from every cell to the nearest cell of some values of a band (1-based), into a
/// new GeoTIFF (`options` as GDALComputeProximity's: `VALUES=1,2`, `DISTUNITS=GEO`, `MAXDIST=5000`...).
pub fn proximity(source: &Path, band: usize, dest: &Path, options: &[String]) -> Result<()> {
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let src = g.open_handle(&source.to_string_lossy(), false)?;
    let r = (|| {
        let out = create_like(g, src, dest, -1.0)?;
        let mut opts = Argv::new(options);
        unsafe { (a.CPLErrorReset)() };
        let e = unsafe {
            (a.GDALComputeProximity)(
                (a.GDALGetRasterBand)(src, band as c_int),
                (a.GDALGetRasterBand)(out, 1),
                opts.ptrs.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        unsafe { (a.GDALClose)(out) };
        if e >= 3 { Err(g.last_error("gdal_proximity")) } else { Ok(()) }
    })();
    unsafe { (a.GDALClose)(src) };
    r
}

/// `gdal_fillnodata`: the cells without a value filled from those around them (up to `max_distance` cells,
/// smoothed `smoothing` times), in a copy of the raster.
pub fn fill_nodata(source: &Path, dest: &Path, max_distance: f64, smoothing: usize) -> Result<()> {
    translate(source, dest, &["-co".into(), "COMPRESS=DEFLATE".into(), "-co".into(), "TILED=YES".into()])?;
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let h = g.open_handle(&dest.to_string_lossy(), true)?;
    let n = unsafe { (a.GDALGetRasterCount)(h) };
    let mut r = Ok(());
    for b in 1..=n {
        unsafe { (a.CPLErrorReset)() };
        let e = unsafe {
            (a.GDALFillNodata)(
                (a.GDALGetRasterBand)(h, b),
                std::ptr::null_mut(),
                max_distance,
                0,
                smoothing as c_int,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        if e >= 3 {
            r = Err(g.last_error("gdal_fillnodata"));
            break;
        }
    }
    unsafe { (a.GDALClose)(h) };
    r
}

/// A service of web map tiles (XYZ, Web Mercator), as GDAL's WMS driver opens it: `url` with `${z}`, `${x}`,
/// `${y}`; its tiles kept in `cache`.
pub fn tile_service(url: &str, levels: u32, cache: &Path) -> String {
    let e = 20037508.342789244;
    format!(
        "<GDAL_WMS><Service name=\"TMS\"><ServerUrl>{}</ServerUrl></Service><DataWindow><UpperLeftX>-{e}</UpperLeftX>         <UpperLeftY>{e}</UpperLeftY><LowerRightX>{e}</LowerRightX><LowerRightY>-{e}</LowerRightY><TileLevel>{levels}</TileLevel>         <TileCountX>1</TileCountX><TileCountY>1</TileCountY><YOrigin>top</YOrigin></DataWindow>         <Projection>EPSG:3857</Projection><BlockSizeX>256</BlockSizeX><BlockSizeY>256</BlockSizeY>         <BandsCount>3</BandsCount><MaxConnections>4</MaxConnections><Timeout>20</Timeout>         <Cache><Path>{}</Path></Cache><ZeroBlockHttpCodes>204,404</ZeroBlockHttpCodes></GDAL_WMS>",
        url.replace('&', "&amp;"),
        cache.to_string_lossy().replace('&', "&amp;")
    )
}

/// A part of a map service (or of any raster GDAL opens) warped onto an extent `[xmin, ymin, xmax, ymax]` of a
/// CRS, `width` x `height` pixels, as RGBA (transparent where the service has nothing).
pub fn warp_image(source: &str, crs: &str, extent: [f64; 4], width: usize, height: usize) -> Result<Vec<[u8; 4]>> {
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let mut src = g.open_handle(source, false)?;
    let mut args: Vec<String> =
        ["-of", "MEM", "-t_srs", crs, "-r", "bilinear", "-dstalpha", "-te"].iter().map(|s| s.to_string()).collect();
    args.extend(extent.iter().map(|v| v.to_string()));
    args.extend(["-ts".to_string(), width.to_string(), height.to_string()]);
    let mut argv = Argv::new(&args);
    let opts = unsafe { (a.GDALWarpAppOptionsNew)(argv.ptrs.as_mut_ptr(), std::ptr::null_mut()) };
    if opts.is_null() {
        unsafe { (a.GDALClose)(src) };
        return Err(g.last_error("warping the map: the options"));
    }
    unsafe { (a.CPLErrorReset)() };
    let mut usage: c_int = 0;
    let out = unsafe { (a.GDALWarp)(cstr("").as_ptr(), std::ptr::null_mut(), 1, &mut src, opts, &mut usage) };
    unsafe { (a.GDALWarpAppOptionsFree)(opts) };
    unsafe { (a.GDALClose)(src) };
    if out.is_null() {
        return Err(g.last_error("warping the map"));
    }
    let n = unsafe { (a.GDALGetRasterCount)(out) } as usize;
    let mut px = vec![[0u8, 0, 0, 255]; width * height];
    let mut band = vec![0u8; width * height];
    let mut r = Ok(());
    for b in 0..n.min(4) {
        let h = unsafe { (a.GDALGetRasterBand)(out, b as c_int + 1) };
        let e = unsafe {
            (a.GDALRasterIO)(
                h,
                0,
                0,
                0,
                width as c_int,
                height as c_int,
                band.as_mut_ptr() as *mut c_void,
                width as c_int,
                height as c_int,
                1,
                0,
                0,
            )
        };
        if e >= 3 {
            r = Err(g.last_error("reading the map"));
            break;
        }
        // red, green, blue and the alpha GDAL added (a grey map: its band as the three colours)
        let k = if n == 2 && b == 1 { 3 } else { b };
        for (p, v) in px.iter_mut().zip(&band) {
            if n == 2 && b == 0 {
                p[0] = *v;
                p[1] = *v;
                p[2] = *v;
            } else {
                p[k] = *v;
            }
        }
    }
    unsafe { (a.GDALClose)(out) };
    r.map(|_| px)
}

// ---------------------------------------------------------------------------------------------------- vectors

impl Gdal {
    /// A spatial reference from WKT (or anything OGR takes: EPSG:4326...), its axes in x, y order.
    fn srs(&self, wkt: &str) -> Result<H> {
        let h = unsafe { (self.api.OSRNewSpatialReference)(std::ptr::null()) };
        if h.is_null() {
            return Err("OGR: no spatial reference".into());
        }
        unsafe { (self.api.CPLErrorReset)() };
        if unsafe { (self.api.OSRSetFromUserInput)(h, cstr(wkt).as_ptr()) } != 0 {
            unsafe { (self.api.OSRDestroySpatialReference)(h) };
            return Err(self.last_error("OGR cannot read the coordinate system"));
        }
        // OAMS_TRADITIONAL_GIS_ORDER: x (easting, longitude) first
        unsafe { (self.api.OSRSetAxisMappingStrategy)(h, 0) };
        Ok(h)
    }

    fn wkt_of(&self, srs: H) -> Option<String> {
        if srs.is_null() {
            return None;
        }
        let mut p: *mut c_char = std::ptr::null_mut();
        unsafe { (self.api.OSRExportToWkt)(srs, &mut p) };
        let w = text(p);
        unsafe { (self.api.VSIFree)(p as *mut c_void) };
        (!w.is_empty()).then_some(w)
    }
}

/// The layers of a vector file.
pub fn vector_layers(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let g = gdal().map_err(String::from)?;
    let path = path.as_ref().to_string_lossy().into_owned();
    let a = &g.api;
    unsafe { (a.CPLErrorReset)() };
    // GDAL_OF_VECTOR
    let ds =
        unsafe { (a.GDALOpenEx)(cstr(&path).as_ptr(), 0x04, std::ptr::null(), std::ptr::null(), std::ptr::null()) };
    if ds.is_null() {
        return Err(g.last_error(&format!("{path}: GDAL cannot open it as vectors")));
    }
    let n = unsafe { (a.GDALDatasetGetLayerCount)(ds) };
    let names = (0..n).map(|i| text(unsafe { (a.OGR_L_GetName)((a.GDALDatasetGetLayer)(ds, i)) })).collect();
    unsafe { (a.GDALClose)(ds) };
    Ok(names)
}

/// The features of a layer of a vector file (the first without a name): every geometry (in 2-D), the attributes
/// (numbers for OGR's integers and reals, texts for the others), the coordinate system.
pub fn read_vector(path: impl AsRef<Path>, layer: Option<&str>) -> Result<Vector> {
    let g = gdal().map_err(String::from)?;
    let path = path.as_ref().to_string_lossy().into_owned();
    let a = &g.api;
    unsafe { (a.CPLErrorReset)() };
    let ds =
        unsafe { (a.GDALOpenEx)(cstr(&path).as_ptr(), 0x04, std::ptr::null(), std::ptr::null(), std::ptr::null()) };
    if ds.is_null() {
        return Err(g.last_error(&format!("{path}: GDAL cannot open it as vectors")));
    }
    let close = |r: Result<Vector>| {
        unsafe { (a.GDALClose)(ds) };
        r
    };
    let lyr = match layer {
        Some(name) => unsafe { (a.GDALDatasetGetLayerByName)(ds, cstr(name).as_ptr()) },
        None => unsafe { (a.GDALDatasetGetLayer)(ds, 0) },
    };
    if lyr.is_null() {
        return close(Err(format!("{path}: no layer {}", layer.unwrap_or("(the file has none)"))));
    }
    let defn = unsafe { (a.OGR_L_GetLayerDefn)(lyr) };
    let nf = unsafe { (a.OGR_FD_GetFieldCount)(defn) };
    // OFTInteger 0, OFTReal 2, OFTInteger64 12: numbers
    let fields: Vec<(String, bool)> = (0..nf)
        .map(|i| {
            let fd = unsafe { (a.OGR_FD_GetFieldDefn)(defn, i) };
            let t = unsafe { (a.OGR_Fld_GetType)(fd) };
            (text(unsafe { (a.OGR_Fld_GetNameRef)(fd) }), matches!(t, 0 | 2 | 12))
        })
        .collect();
    let mut numbers: Vec<Vec<f64>> = fields.iter().map(|_| Vec::new()).collect();
    let mut texts: Vec<Vec<String>> = fields.iter().map(|_| Vec::new()).collect();
    let mut geometries = Vec::new();
    unsafe { (a.OGR_L_ResetReading)(lyr) };
    loop {
        let f = unsafe { (a.OGR_L_GetNextFeature)(lyr) };
        if f.is_null() {
            break;
        }
        for (i, (_, num)) in fields.iter().enumerate() {
            let set = unsafe { (a.OGR_F_IsFieldSetAndNotNull)(f, i as c_int) } != 0;
            if *num {
                numbers[i].push(if set { unsafe { (a.OGR_F_GetFieldAsDouble)(f, i as c_int) } } else { f64::NAN });
            } else {
                texts[i].push(if set {
                    text(unsafe { (a.OGR_F_GetFieldAsString)(f, i as c_int) })
                } else {
                    String::new()
                });
            }
        }
        let gh = unsafe { (a.OGR_F_GetGeometryRef)(f) };
        let geom = if gh.is_null() {
            Ok(Geometry::Empty)
        } else {
            let n = unsafe { (a.OGR_G_WkbSize)(gh) } as usize;
            let mut buf = vec![0u8; n];
            // wkbNDR
            unsafe { (a.OGR_G_ExportToIsoWkb)(gh, 1, buf.as_mut_ptr()) };
            Geometry::from_wkb(&buf)
        };
        unsafe { (a.OGR_F_Destroy)(f) };
        match geom {
            Ok(g) => geometries.push(g),
            Err(e) => return close(Err(format!("{path}: {e}"))),
        }
    }
    let crs = g.wkt_of(unsafe { (a.OGR_L_GetSpatialRef)(lyr) });
    let attributes = fields
        .into_iter()
        .zip(numbers.into_iter().zip(texts))
        .map(|((name, num), (n, t))| (name, if num { Column::Numbers(n) } else { Column::Texts(t) }))
        .collect();
    close(Ok(Vector { geometries, attributes, crs }))
}

/// The driver of a vector file by its extension.
fn vector_driver(path: &Path) -> Result<&'static str> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    Ok(match ext.as_str() {
        "gpkg" => "GPKG",
        "shp" => "ESRI Shapefile",
        "geojson" | "json" => "GeoJSON",
        "fgb" => "FlatGeobuf",
        "kml" => "KML",
        "gml" => "GML",
        "csv" => "CSV",
        e => {
            return Err(format!(
                "{}: no vector format for .{e} (gpkg, shp, geojson, fgb, kml, gml, csv)",
                path.display()
            ));
        }
    })
}

/// Features written to a vector file (its format by the extension; replaced if it exists), a layer named
/// `layer`.
pub fn write_vector(path: impl AsRef<Path>, layer: &str, v: &Vector) -> Result<()> {
    let g = gdal().map_err(String::from)?;
    let path = path.as_ref();
    let a = &g.api;
    let driver = unsafe { (a.GDALGetDriverByName)(cstr(vector_driver(path)?).as_ptr()) };
    if driver.is_null() {
        return Err(format!("{}: GDAL has no driver for it", path.display()));
    }
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let name = path.to_string_lossy().into_owned();
    unsafe { (a.CPLErrorReset)() };
    let ds = unsafe { (a.GDALCreate)(driver, cstr(&name).as_ptr(), 0, 0, 0, 0, std::ptr::null_mut()) };
    if ds.is_null() {
        return Err(g.last_error(&format!("{name}: GDAL cannot create it")));
    }
    let result = (|| -> Result<()> {
        let srs = match &v.crs {
            Some(w) => g.srs(w)?,
            None => std::ptr::null_mut(),
        };
        // the geometry type: of the first geometry, else unknown (0)
        let gtype = match v.geometries.iter().find(|g| !matches!(g, Geometry::Empty)) {
            Some(Geometry::Point(_)) => 1,
            Some(Geometry::LineString(_)) => 2,
            Some(Geometry::Polygon(_)) if v.geometries.iter().all(|g| !matches!(g, Geometry::MultiPolygon(_))) => 3,
            Some(Geometry::MultiPoint(_)) => 4,
            Some(Geometry::MultiLineString(_)) => 5,
            Some(Geometry::Polygon(_) | Geometry::MultiPolygon(_)) => 6,
            _ => 0,
        };
        let lyr = unsafe { (a.GDALDatasetCreateLayer)(ds, cstr(layer).as_ptr(), srs, gtype, std::ptr::null_mut()) };
        if !srs.is_null() {
            unsafe { (a.OSRDestroySpatialReference)(srs) };
        }
        if lyr.is_null() {
            return Err(g.last_error(&format!("{name}: no layer created")));
        }
        for (n, c) in &v.attributes {
            // OFTReal 2, OFTString 4
            let fd =
                unsafe { (a.OGR_Fld_Create)(cstr(n).as_ptr(), if matches!(c, Column::Numbers(_)) { 2 } else { 4 }) };
            let e = unsafe { (a.OGR_L_CreateField)(lyr, fd, 1) };
            unsafe { (a.OGR_Fld_Destroy)(fd) };
            if e != 0 {
                return Err(g.last_error(&format!("{name}: the field {n} cannot be made")));
            }
        }
        let defn = unsafe { (a.OGR_L_GetLayerDefn)(lyr) };
        for (k, geom) in v.geometries.iter().enumerate() {
            let f = unsafe { (a.OGR_F_Create)(defn) };
            for (i, (_, c)) in v.attributes.iter().enumerate() {
                match c {
                    Column::Numbers(x) if x[k].is_finite() => unsafe { (a.OGR_F_SetFieldDouble)(f, i as c_int, x[k]) },
                    Column::Texts(t) if !t[k].is_empty() => unsafe {
                        (a.OGR_F_SetFieldString)(f, i as c_int, cstr(&t[k]).as_ptr())
                    },
                    _ => {}
                }
            }
            if !matches!(geom, Geometry::Empty) {
                let wkb = geom.to_wkb();
                let mut gh: H = std::ptr::null_mut();
                let e =
                    unsafe { (a.OGR_G_CreateFromWkb)(wkb.as_ptr(), std::ptr::null_mut(), &mut gh, wkb.len() as c_int) };
                if e != 0 || gh.is_null() {
                    unsafe { (a.OGR_F_Destroy)(f) };
                    return Err(format!("{name}: feature {k}: OGR cannot read its geometry"));
                }
                unsafe { (a.OGR_F_SetGeometryDirectly)(f, gh) };
            }
            let e = unsafe { (a.OGR_L_CreateFeature)(lyr, f) };
            unsafe { (a.OGR_F_Destroy)(f) };
            if e != 0 {
                return Err(g.last_error(&format!("{name}: feature {k} cannot be written")));
            }
        }
        Ok(())
    })();
    unsafe { (a.GDALClose)(ds) };
    result
}

/// The features in another coordinate system (WKT, EPSG:...): their geometries transformed by PROJ.
pub fn reproject_vector(v: &Vector, to: &str) -> Result<Vector> {
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let from = v.crs.as_deref().ok_or("the features have no coordinate system")?;
    let (src, dst) = (g.srs(from)?, g.srs(to)?);
    let same = unsafe { (a.OSRIsSame)(src, dst) } != 0;
    let out = (|| -> Result<Vector> {
        let mut out = v.clone();
        out.crs = g.wkt_of(dst);
        if same {
            return Ok(out);
        }
        let ct = unsafe { (a.OCTNewCoordinateTransformation)(src, dst) };
        if ct.is_null() {
            return Err(g.last_error("no transformation between the coordinate systems"));
        }
        let mut result = Ok(());
        for geom in out.geometries.iter_mut().filter(|g| !matches!(g, Geometry::Empty)) {
            let wkb = geom.to_wkb();
            let mut gh: H = std::ptr::null_mut();
            let e = unsafe { (a.OGR_G_CreateFromWkb)(wkb.as_ptr(), std::ptr::null_mut(), &mut gh, wkb.len() as c_int) };
            if e != 0 || gh.is_null() {
                result = Err("OGR cannot read a geometry".to_string());
                break;
            }
            let e = unsafe { (a.OGR_G_Transform)(gh, ct) };
            let n = unsafe { (a.OGR_G_WkbSize)(gh) } as usize;
            let mut buf = vec![0u8; n];
            unsafe { (a.OGR_G_ExportToIsoWkb)(gh, 1, buf.as_mut_ptr()) };
            unsafe { (a.OGR_G_DestroyGeometry)(gh) };
            if e != 0 {
                result = Err(g.last_error("a geometry cannot be transformed"));
                break;
            }
            *geom = Geometry::from_wkb(&buf)?;
        }
        unsafe { (a.OCTDestroyCoordinateTransformation)(ct) };
        result.map(|_| out)
    })();
    unsafe {
        (a.OSRDestroySpatialReference)(src);
        (a.OSRDestroySpatialReference)(dst);
    }
    out
}

/// A geometry as OGR holds it (to be destroyed).
fn ogr_geometry(g: &Gdal, geom: &Geometry) -> Result<H> {
    let wkb = geom.to_wkb();
    let mut h: H = std::ptr::null_mut();
    let e = unsafe { (g.api.OGR_G_CreateFromWkb)(wkb.as_ptr(), std::ptr::null_mut(), &mut h, wkb.len() as c_int) };
    if e != 0 || h.is_null() {
        return Err("OGR cannot read a geometry".into());
    }
    Ok(h)
}

/// The geometry of an OGR handle, which is destroyed (an empty one for none).
fn from_ogr(g: &Gdal, h: H, what: &str) -> Result<Geometry> {
    if h.is_null() {
        return Err(g.last_error(&format!("GEOS: no {what}")));
    }
    let out = if unsafe { (g.api.OGR_G_IsEmpty)(h) } != 0 {
        Ok(Geometry::Empty)
    } else {
        let n = unsafe { (g.api.OGR_G_WkbSize)(h) } as usize;
        let mut buf = vec![0u8; n];
        unsafe { (g.api.OGR_G_ExportToIsoWkb)(h, 1, buf.as_mut_ptr()) };
        Geometry::from_wkb(&buf)
    };
    unsafe { (g.api.OGR_G_DestroyGeometry)(h) };
    out
}

/// The buffers of geometries (GEOS through GDAL): `distance` around each (in their units; inside polygons when
/// negative), a quarter of a circle in `segments` segments (16 as shapely's).
pub fn buffer(geoms: &[Geometry], distance: f64, segments: i32) -> Result<Vec<Geometry>> {
    let g = gdal().map_err(String::from)?;
    geoms
        .iter()
        .map(|geom| {
            if matches!(geom, Geometry::Empty) {
                return Ok(Geometry::Empty);
            }
            let h = ogr_geometry(g, geom)?;
            let b = unsafe { (g.api.OGR_G_Buffer)(h, distance, segments) };
            unsafe { (g.api.OGR_G_DestroyGeometry)(h) };
            from_ogr(g, b, "buffer")
        })
        .collect()
}

/// Every geometry's intersection with a mask (GEOS through GDAL); empty where they do not meet.
pub fn intersection(geoms: &[Geometry], mask: &Geometry) -> Result<Vec<Geometry>> {
    let g = gdal().map_err(String::from)?;
    let m = ogr_geometry(g, mask)?;
    let out = geoms
        .iter()
        .map(|geom| {
            if matches!(geom, Geometry::Empty) {
                return Ok(Geometry::Empty);
            }
            let h = ogr_geometry(g, geom)?;
            let i = unsafe { (g.api.OGR_G_Intersection)(h, m) };
            unsafe { (g.api.OGR_G_DestroyGeometry)(h) };
            from_ogr(g, i, "intersection")
        })
        .collect();
    unsafe { (g.api.OGR_G_DestroyGeometry)(m) };
    out
}

/// The union of geometries of one kind (GEOS's unary union through GDAL; its cascaded union of polygons before
/// GDAL 3.7).
pub fn union(geoms: &[&Geometry]) -> Result<Geometry> {
    let g = gdal().map_err(String::from)?;
    let (mut pts, mut lines, mut polys) = (Vec::new(), Vec::new(), Vec::new());
    for geom in geoms {
        match geom {
            Geometry::Empty => {}
            Geometry::Point(p) => pts.push(*p),
            Geometry::MultiPoint(v) => pts.extend(v.iter().copied()),
            Geometry::LineString(l) => lines.push(l.clone()),
            Geometry::MultiLineString(v) => lines.extend(v.iter().cloned()),
            Geometry::Polygon(r) => polys.push(r.clone()),
            Geometry::MultiPolygon(v) => polys.extend(v.iter().cloned()),
        }
    }
    let kinds = usize::from(!pts.is_empty()) + usize::from(!lines.is_empty()) + usize::from(!polys.is_empty());
    if kinds > 1 {
        return Err("a union of geometries of one kind (points, lines or polygons)".into());
    }
    let all = if !polys.is_empty() {
        Geometry::MultiPolygon(polys)
    } else if !lines.is_empty() {
        Geometry::MultiLineString(lines)
    } else if !pts.is_empty() {
        Geometry::MultiPoint(pts)
    } else {
        return Ok(Geometry::Empty);
    };
    let polygonal = matches!(all, Geometry::MultiPolygon(_));
    let h = ogr_geometry(g, &all)?;
    let u = match g.api.OGR_G_UnaryUnion {
        Some(f) => unsafe { f(h) },
        None if polygonal => unsafe { (g.api.OGR_G_UnionCascaded)(h) },
        None => {
            unsafe { (g.api.OGR_G_DestroyGeometry)(h) };
            return Err("the union of lines or points needs GDAL 3.7 or later".into());
        }
    };
    unsafe { (g.api.OGR_G_DestroyGeometry)(h) };
    from_ogr(g, u, "union")
}

/// GEOS's operations on geometries through GDAL, the geometries held by OGR while it runs (destroyed after).
pub struct Geos {
    g: &'static Gdal,
    handles: Vec<H>,
}

impl Geos {
    /// The geometries, read once by OGR.
    pub fn new(geoms: &[Geometry]) -> Result<Geos> {
        let g = gdal().map_err(String::from)?;
        let mut out = Geos { g, handles: Vec::with_capacity(geoms.len()) };
        for geom in geoms {
            let h = if matches!(geom, Geometry::Empty) {
                ogr_geometry(g, &Geometry::MultiPolygon(Vec::new()))?
            } else {
                ogr_geometry(g, geom)?
            };
            out.handles.push(h);
        }
        Ok(out)
    }

    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// The distance between geometries i and j (GEOS's, 0 when they meet).
    pub fn distance(&self, i: usize, j: usize) -> f64 {
        unsafe { (self.g.api.OGR_G_Distance)(self.handles[i], self.handles[j]) }
    }

    /// The centroid of geometry i (GEOS's).
    pub fn centroid(&self, i: usize) -> Result<[f64; 2]> {
        let a = &self.g.api;
        let p = unsafe { (a.OGR_G_CreateGeometry)(1) };
        let e = unsafe { (a.OGR_G_Centroid)(self.handles[i], p) };
        let xy = unsafe { [(a.OGR_G_GetX)(p, 0), (a.OGR_G_GetY)(p, 0)] };
        unsafe { (a.OGR_G_DestroyGeometry)(p) };
        if e != 0 { Err(self.g.last_error("GEOS: no centroid")) } else { Ok(xy) }
    }

    /// A point on the surface of geometry i (GEOS's interior point).
    pub fn point_on_surface(&self, i: usize) -> Result<[f64; 2]> {
        let a = &self.g.api;
        let p = unsafe { (a.OGR_G_PointOnSurface)(self.handles[i]) };
        if p.is_null() {
            return Err(self.g.last_error("GEOS: no point on the surface"));
        }
        let xy = unsafe { [(a.OGR_G_GetX)(p, 0), (a.OGR_G_GetY)(p, 0)] };
        unsafe { (a.OGR_G_DestroyGeometry)(p) };
        Ok(xy)
    }

    /// Whether the point lies within geometry i (GEOS's predicate).
    pub fn contains_point(&self, i: usize, xy: [f64; 2]) -> Result<bool> {
        let p = ogr_geometry(self.g, &Geometry::Point(xy))?;
        let r = unsafe { (self.g.api.OGR_G_Within)(p, self.handles[i]) } != 0;
        unsafe { (self.g.api.OGR_G_DestroyGeometry)(p) };
        Ok(r)
    }

    /// Whether geometries i and j intersect.
    pub fn intersects(&self, i: usize, j: usize) -> bool {
        unsafe { (self.g.api.OGR_G_Intersects)(self.handles[i], self.handles[j]) != 0 }
    }

    /// Whether geometry i intersects geometry j of another set.
    pub fn intersects_other(&self, i: usize, other: &Geos, j: usize) -> bool {
        unsafe { (self.g.api.OGR_G_Intersects)(self.handles[i], other.handles[j]) != 0 }
    }

    /// Whether geometry i intersects another geometry.
    pub fn intersects_with(&self, i: usize, other: &Geometry) -> Result<bool> {
        let h = ogr_geometry(self.g, other)?;
        let r = unsafe { (self.g.api.OGR_G_Intersects)(self.handles[i], h) } != 0;
        unsafe { (self.g.api.OGR_G_DestroyGeometry)(h) };
        Ok(r)
    }
}

impl Drop for Geos {
    fn drop(&mut self) {
        for &h in &self.handles {
            unsafe { (self.g.api.OGR_G_DestroyGeometry)(h) };
        }
    }
}

/// Geometry a less geometry b (GEOS's difference through GDAL).
pub fn difference(a: &Geometry, b: &Geometry) -> Result<Geometry> {
    let g = gdal().map_err(String::from)?;
    if matches!(a, Geometry::Empty) {
        return Ok(Geometry::Empty);
    }
    if matches!(b, Geometry::Empty) {
        return Ok(a.clone());
    }
    let (ha, hb) = (ogr_geometry(g, a)?, ogr_geometry(g, b)?);
    let d = unsafe { (g.api.OGR_G_Difference)(ha, hb) };
    unsafe {
        (g.api.OGR_G_DestroyGeometry)(ha);
        (g.api.OGR_G_DestroyGeometry)(hb);
    }
    from_ogr(g, d, "difference")
}

/// A valid geometry (GEOS's MakeValid through GDAL, its linework method as shapely's make_valid).
pub fn make_valid(a: &Geometry) -> Result<Geometry> {
    let g = gdal().map_err(String::from)?;
    if matches!(a, Geometry::Empty) {
        return Ok(Geometry::Empty);
    }
    let h = ogr_geometry(g, a)?;
    let v = unsafe { (g.api.OGR_G_MakeValid)(h) };
    unsafe { (g.api.OGR_G_DestroyGeometry)(h) };
    from_ogr(g, v, "valid geometry")
}

/// The convex hull of a geometry (GEOS's through GDAL).
pub fn convex_hull(a: &Geometry) -> Result<Geometry> {
    let g = gdal().map_err(String::from)?;
    if matches!(a, Geometry::Empty) {
        return Ok(Geometry::Empty);
    }
    let h = ogr_geometry(g, a)?;
    let v = unsafe { (g.api.OGR_G_ConvexHull)(h) };
    unsafe { (g.api.OGR_G_DestroyGeometry)(h) };
    from_ogr(g, v, "convex hull")
}

/// The geodesic areas (square metres) of polygons on the WGS84 ellipsoid, their coordinates taken to EPSG:4326 by
/// PROJ: GeographicLib's areas, as pyproj's Geod and terra's expanse give them (GDAL 3.9 or later). Holes count
/// against their polygon; other geometries have no area.
pub fn geodesic_areas(v: &Vector) -> Result<Vec<f64>> {
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let area = a.OGR_G_GeodesicArea.ok_or("geodesic areas need GDAL 3.9 or later")?;
    let ll = reproject_vector(v, "EPSG:4326")?;
    let srs = g.srs("EPSG:4326")?;
    let mut out = Vec::with_capacity(ll.len());
    let mut result = Ok(());
    for geom in &ll.geometries {
        if !matches!(geom, Geometry::Polygon(_) | Geometry::MultiPolygon(_)) {
            out.push(0.0);
            continue;
        }
        let wkb = geom.to_wkb();
        let mut gh: H = std::ptr::null_mut();
        let e = unsafe { (a.OGR_G_CreateFromWkb)(wkb.as_ptr(), std::ptr::null_mut(), &mut gh, wkb.len() as c_int) };
        if e != 0 || gh.is_null() {
            result = Err("OGR cannot read a geometry".to_string());
            break;
        }
        unsafe { (a.OGR_G_AssignSpatialReference)(gh, srs) };
        unsafe { (a.CPLErrorReset)() };
        let s = unsafe { area(gh) };
        unsafe { (a.OGR_G_DestroyGeometry)(gh) };
        if s < 0.0 {
            result = Err(g.last_error("OGR cannot measure a geodesic area"));
            break;
        }
        out.push(s);
    }
    unsafe { (a.OSRDestroySpatialReference)(srs) };
    result.map(|_| out)
}

/// Whether two coordinate systems are the same (by OGR).
pub fn same_crs(a_wkt: &str, b_wkt: &str) -> Result<bool> {
    let g = gdal().map_err(String::from)?;
    let (x, y) = (g.srs(a_wkt)?, g.srs(b_wkt)?);
    let same = unsafe { (g.api.OSRIsSame)(x, y) } != 0;
    unsafe {
        (g.api.OSRDestroySpatialReference)(x);
        (g.api.OSRDestroySpatialReference)(y);
    }
    Ok(same)
}

/// Geometries burnt into a grid (width x height, GDAL's geotransform) by GDAL's rasterizer, value k for geometry
/// k, the later ones over the earlier (or added up); cells touched by a line or a polygon's edge with
/// `all_touched`, else the cells whose centre it takes (polygons) or the line's cells (Bresenham's).
#[allow(clippy::too_many_arguments)]
pub fn rasterize(
    out: &mut [f64],
    width: usize,
    height: usize,
    transform: [f64; 6],
    geometries: &[Geometry],
    values: &[f64],
    all_touched: bool,
    add: bool,
) -> Result<()> {
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    if out.len() != width * height || geometries.len() != values.len() {
        return Err("rasterize: the grid or the values differ in size".into());
    }
    let driver = unsafe { (a.GDALGetDriverByName)(cstr("MEM").as_ptr()) };
    unsafe { (a.CPLErrorReset)() };
    // GDT_Float64
    let ds = unsafe {
        (a.GDALCreate)(driver, cstr("").as_ptr(), width as c_int, height as c_int, 1, 7, std::ptr::null_mut())
    };
    if ds.is_null() {
        return Err(g.last_error("rasterize: no grid in memory"));
    }
    let mut geoms: Vec<H> = Vec::new();
    let result = (|| -> Result<()> {
        unsafe { (a.GDALSetGeoTransform)(ds, transform.as_ptr()) };
        let band = unsafe { (a.GDALGetRasterBand)(ds, 1) };
        let io = |rw: c_int, buf: *mut f64| unsafe {
            (a.GDALRasterIO)(
                band,
                rw,
                0,
                0,
                width as c_int,
                height as c_int,
                buf as *mut c_void,
                width as c_int,
                height as c_int,
                7,
                0,
                0,
            )
        };
        if io(1, out.as_mut_ptr()) != 0 {
            return Err(g.last_error("rasterize: the grid cannot be written"));
        }
        let mut burn = Vec::new();
        for (geom, &v) in geometries.iter().zip(values) {
            if matches!(geom, Geometry::Empty) {
                continue;
            }
            let wkb = geom.to_wkb();
            let mut h: H = std::ptr::null_mut();
            if unsafe { (a.OGR_G_CreateFromWkb)(wkb.as_ptr(), std::ptr::null_mut(), &mut h, wkb.len() as c_int) } != 0 {
                return Err("rasterize: OGR cannot read a geometry".into());
            }
            geoms.push(h);
            burn.push(v);
        }
        let mut opts = Vec::new();
        if all_touched {
            opts.push("ALL_TOUCHED=TRUE".to_string());
        }
        if add {
            opts.push("MERGE_ALG=ADD".to_string());
        }
        let mut opts = Argv::new(&opts);
        let bands = [1 as c_int];
        let e = unsafe {
            (a.GDALRasterizeGeometries)(
                ds,
                1,
                bands.as_ptr(),
                geoms.len() as c_int,
                geoms.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                burn.as_ptr(),
                opts.ptrs.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        if e >= 3 {
            return Err(g.last_error("rasterize"));
        }
        if io(0, out.as_mut_ptr()) != 0 {
            return Err(g.last_error("rasterize: the grid cannot be read"));
        }
        Ok(())
    })();
    for h in geoms {
        unsafe { (a.OGR_G_DestroyGeometry)(h) };
    }
    unsafe { (a.GDALClose)(ds) };
    result
}

/// A transformation of coordinates between two systems (x, easting or longitude, first), kept to transform many
/// points.
pub struct Transform {
    ct: H,
}

// a transformation is used by one thread at a time (it is moved, not shared)
unsafe impl Send for Transform {}

impl Transform {
    /// From one system to another (EPSG codes, WKT, PROJ strings).
    pub fn new(from: &str, to: &str) -> Result<Transform> {
        let g = gdal().map_err(String::from)?;
        let (src, dst) = (g.srs(from)?, g.srs(to)?);
        let ct = unsafe { (g.api.OCTNewCoordinateTransformation)(src, dst) };
        unsafe {
            (g.api.OSRDestroySpatialReference)(src);
            (g.api.OSRDestroySpatialReference)(dst);
        }
        if ct.is_null() {
            return Err(g.last_error("no transformation between the coordinate systems"));
        }
        Ok(Transform { ct })
    }

    /// The points transformed in place; those that cannot be become NaN.
    pub fn points(&self, xs: &mut [f64], ys: &mut [f64]) {
        let Ok(g) = gdal() else { return };
        let n = xs.len().min(ys.len());
        let mut ok = vec![0 as c_int; n];
        unsafe {
            (g.api.OCTTransformEx)(
                self.ct,
                n as c_int,
                xs.as_mut_ptr(),
                ys.as_mut_ptr(),
                std::ptr::null_mut(),
                ok.as_mut_ptr(),
            )
        };
        for i in 0..n {
            if ok[i] == 0 || !xs[i].is_finite() || !ys[i].is_finite() {
                xs[i] = f64::NAN;
                ys[i] = f64::NAN;
            }
        }
    }
}

impl Drop for Transform {
    fn drop(&mut self) {
        if let Ok(g) = gdal() {
            unsafe { (g.api.OCTDestroyCoordinateTransformation)(self.ct) };
        }
    }
}

/// The body of a web address fetched by GDAL's HTTP (curl): its status (200...) and bytes.
pub fn fetch(url: &str, options: &[&str]) -> Result<(i32, Vec<u8>)> {
    let g = gdal().map_err(String::from)?;
    let opts: Vec<CString> = options.iter().map(|o| cstr(o)).collect();
    let mut ptrs: Vec<*const c_char> = opts.iter().map(|o| o.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    let r = unsafe { (g.api.CPLHTTPFetch)(cstr(url).as_ptr(), ptrs.as_ptr()) };
    if r.is_null() {
        return Err(format!("{url}: no answer"));
    }
    let res = unsafe { &*r };
    let out = if !res.error.is_null() && res.data.is_null() {
        Err(format!("{url}: {}", text(res.error)))
    } else {
        let data = if res.data.is_null() || res.len <= 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(res.data, res.len as usize) }.to_vec()
        };
        Ok((res.status, data))
    };
    unsafe { (g.api.CPLHTTPDestroyResult)(r) };
    out
}

/// A file read through GDAL's virtual file systems (`/vsizip/`, `/vsicurl/`...), a block at a time.
pub struct VsiFile {
    g: &'static Gdal,
    h: H,
}

impl VsiFile {
    pub fn open(path: &str) -> Result<VsiFile> {
        let g = gdal().map_err(String::from)?;
        unsafe { (g.api.CPLErrorReset)() };
        let h = unsafe { (g.api.VSIFOpenL)(cstr(path).as_ptr(), cstr("rb").as_ptr()) };
        if h.is_null() {
            return Err(g.last_error(&format!("{path}: GDAL cannot open it")));
        }
        Ok(VsiFile { g, h })
    }

    /// Up to `buf.len()` bytes; 0 at the end.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        unsafe { (self.g.api.VSIFReadL)(buf.as_mut_ptr() as *mut c_void, 1, buf.len(), self.h) }
    }
}

impl std::io::Read for VsiFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(VsiFile::read(self, buf))
    }
}

impl Drop for VsiFile {
    fn drop(&mut self) {
        unsafe { (self.g.api.VSIFCloseL)(self.h) };
    }
}

/// The names in a folder of GDAL's virtual file systems (a zip's members: `/vsizip/file.zip`).
pub fn read_dir(path: &str) -> Result<Vec<String>> {
    let g = gdal().map_err(String::from)?;
    let list = unsafe { (g.api.VSIReadDir)(cstr(path).as_ptr()) };
    let mut out = Vec::new();
    if !list.is_null() {
        let mut k = 0;
        loop {
            let p = unsafe { *list.add(k) };
            if p.is_null() {
                break;
            }
            out.push(text(p));
            k += 1;
        }
        unsafe { (g.api.CSLDestroy)(list) };
    }
    Ok(out)
}

/// An image file in memory (PNG, JPEG...) decoded by GDAL: its width, height and RGBA pixels.
pub fn decode_image(bytes: &[u8]) -> Result<(usize, usize, Vec<[u8; 4]>)> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let g = gdal().map_err(String::from)?;
    let a = &g.api;
    let name = format!("/vsimem/terraflux-image-{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let mut data = bytes.to_vec();
    let f = unsafe { (a.VSIFileFromMemBuffer)(cstr(&name).as_ptr(), data.as_mut_ptr(), data.len() as u64, 0) };
    if f.is_null() {
        return Err("the image cannot be put in memory".into());
    }
    unsafe { (a.VSIFCloseL)(f) };
    let out = (|| -> Result<(usize, usize, Vec<[u8; 4]>)> {
        let ds = g.open_handle(&name, false)?;
        let (w, h) = unsafe { ((a.GDALGetRasterXSize)(ds) as usize, (a.GDALGetRasterYSize)(ds) as usize) };
        let n = unsafe { (a.GDALGetRasterCount)(ds) } as usize;
        let mut px = vec![[0u8, 0, 0, 255]; w * h];
        let mut band = vec![0u8; w * h];
        let mut r = Ok(());
        for b in 0..n.min(4) {
            let hb = unsafe { (a.GDALGetRasterBand)(ds, b as c_int + 1) };
            let e = unsafe {
                (a.GDALRasterIO)(
                    hb,
                    0,
                    0,
                    0,
                    w as c_int,
                    h as c_int,
                    band.as_mut_ptr() as *mut c_void,
                    w as c_int,
                    h as c_int,
                    1,
                    0,
                    0,
                )
            };
            if e >= 3 {
                r = Err(g.last_error("decoding the image"));
                break;
            }
            // a palette: each value its colour
            let table = if n == 1 { unsafe { (a.GDALGetRasterColorTable)(hb) } } else { std::ptr::null_mut() };
            if !table.is_null() {
                let k = unsafe { (a.GDALGetColorEntryCount)(table) }.max(0) as usize;
                let colours: Vec<[u8; 4]> = (0..k)
                    .map(|i| {
                        let e = unsafe { &*(a.GDALGetColorEntry)(table, i as c_int) };
                        [e[0] as u8, e[1] as u8, e[2] as u8, e[3] as u8]
                    })
                    .collect();
                for (p, v) in px.iter_mut().zip(&band) {
                    *p = colours.get(*v as usize).copied().unwrap_or([0, 0, 0, 0]);
                }
                continue;
            }
            for (p, v) in px.iter_mut().zip(&band) {
                match (n, b) {
                    // grey (and alpha)
                    (1 | 2, 0) => {
                        p[0] = *v;
                        p[1] = *v;
                        p[2] = *v;
                    }
                    (2, 1) => p[3] = *v,
                    _ => p[b] = *v,
                }
            }
        }
        unsafe { (a.GDALClose)(ds) };
        r.map(|_| (w, h, px))
    })();
    unsafe { (a.VSIUnlink)(cstr(&name).as_ptr()) };
    drop(data);
    out
}

/// Whether a coordinate system is geographic (longitude and latitude in degrees).
pub fn is_geographic(crs: &str) -> Result<bool> {
    let g = gdal().map_err(String::from)?;
    let h = g.srs(crs)?;
    let geo = unsafe { (g.api.OSRIsGeographic)(h) } != 0;
    unsafe { (g.api.OSRDestroySpatialReference)(h) };
    Ok(geo)
}
