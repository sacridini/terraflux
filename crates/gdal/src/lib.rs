//! GDAL for Terraflux, loaded when the program runs (no GDAL is needed to build Terraflux): every raster format
//! GDAL reads, read in windows at any resolution (its overviews used), and GDAL's warping (reprojecting,
//! resampling, clipping) and translating.
//!
//! GDAL is found, in order:
//! - in `TERRAFLUX_GDAL` (a folder, or the library itself);
//! - in the folder `gdal` next to the program (the installers bring it there, with its data);
//! - next to the program, in the `PATH`, and where GDAL is usually installed.
//!
//! Its data (GDAL's and PROJ's) and its plugins are looked for next to the library. Without GDAL, Terraflux
//! reads and writes GeoTIFF by itself; [`gdal`] says why GDAL could not be had.

use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

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
    CPLGetLastErrorMsg: unsafe extern "C" fn() -> *const c_char,
    CPLErrorReset: unsafe extern "C" fn(),
    CPLSetConfigOption: unsafe extern "C" fn(*const c_char, *const c_char),
    CPLSetErrorHandler: unsafe extern "C" fn(*const c_void) -> *const c_void,
    CPLQuietErrorHandler: *const c_void,
    OSRSetPROJSearchPaths: unsafe extern "C" fn(*const *const c_char),
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
    if let Some(exe) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        dirs.push(exe.join("gdal"));
        dirs.push(exe.join("gdal").join("bin"));
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
        for d in ["/usr/lib", "/usr/lib/x86_64-linux-gnu", "/usr/local/lib", "/opt/homebrew/lib"] {
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
            if let Some(d) = first(&["gdal-data", "../share/gdal", "share/gdal", "data"]) {
                (api.CPLSetConfigOption)(cstr("GDAL_DATA").as_ptr(), cstr(&d.to_string_lossy()).as_ptr());
            }
            if let Some(d) = first(&["gdalplugins", "../lib/gdalplugins", "gdal/plugins"]) {
                (api.CPLSetConfigOption)(cstr("GDAL_DRIVER_PATH").as_ptr(), cstr(&d.to_string_lossy()).as_ptr());
            }
            if let Some(d) = first(&["projlib", "proj-data", "../share/proj", "share/proj", "proj"]) {
                let p = cstr(&d.to_string_lossy());
                let paths = [p.as_ptr(), std::ptr::null()];
                (api.OSRSetPROJSearchPaths)(paths.as_ptr());
            }
            (api.CPLSetConfigOption)(cstr("GDAL_PAM_ENABLED").as_ptr(), cstr("NO").as_ptr());
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
