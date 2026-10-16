//! Gurobi, when it is installed: its C library loaded when first needed (nothing of it is needed to build or run
//! Terraflux), the integer programs of the prioritizations solved by it as prioritizr's `add_gurobi_solver` does.
//! The library is found by `TERRAFLUX_GUROBI` (its file), `GUROBI_HOME` (Gurobi's installer sets it), the usual
//! folders of an installation, else the system's search path; a licence is needed as for any program using it.

use std::ffi::{CStr, CString, c_char, c_double, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use terraflux_ls_sys::ffi::SolveOut;

use crate::milp::Model;

type Env = *mut c_void;
type Grb = *mut c_void;

const INFINITY: f64 = 1e100;

#[allow(non_snake_case)]
struct Api {
    GRBversion: unsafe extern "system" fn(*mut c_int, *mut c_int, *mut c_int),
    GRBemptyenvinternal: unsafe extern "system" fn(*mut Env, c_int, c_int, c_int) -> c_int,
    GRBstartenv: unsafe extern "system" fn(Env) -> c_int,
    GRBfreeenv: unsafe extern "system" fn(Env),
    GRBgeterrormsg: unsafe extern "system" fn(Env) -> *const c_char,
    GRBsetintparam: unsafe extern "system" fn(Env, *const c_char, c_int) -> c_int,
    GRBsetdblparam: unsafe extern "system" fn(Env, *const c_char, c_double) -> c_int,
    GRBnewmodel: unsafe extern "system" fn(
        Env,
        *mut Grb,
        *const c_char,
        c_int,
        *mut c_double,
        *mut c_double,
        *mut c_double,
        *mut c_char,
        *mut *mut c_char,
    ) -> c_int,
    GRBXaddconstrs: unsafe extern "system" fn(
        Grb,
        c_int,
        usize,
        *mut usize,
        *mut c_int,
        *mut c_double,
        *mut c_char,
        *mut c_double,
        *mut *mut c_char,
    ) -> c_int,
    GRBsetintattr: unsafe extern "system" fn(Grb, *const c_char, c_int) -> c_int,
    GRBgetintattr: unsafe extern "system" fn(Grb, *const c_char, *mut c_int) -> c_int,
    GRBgetdblattr: unsafe extern "system" fn(Grb, *const c_char, *mut c_double) -> c_int,
    GRBgetdblattrarray: unsafe extern "system" fn(Grb, *const c_char, c_int, c_int, *mut c_double) -> c_int,
    GRBsetdblattrarray: unsafe extern "system" fn(Grb, *const c_char, c_int, c_int, *mut c_double) -> c_int,
    GRBgetenv: unsafe extern "system" fn(Grb) -> Env,
    GRBoptimize: unsafe extern "system" fn(Grb) -> c_int,
    GRBfreemodel: unsafe extern "system" fn(Grb) -> c_int,
    GRBsetcallbackfunc: unsafe extern "system" fn(Grb, Callback, *mut c_void) -> c_int,
    GRBcbget: unsafe extern "system" fn(*mut c_void, c_int, c_int, *mut c_void) -> c_int,
    GRBterminate: unsafe extern "system" fn(Grb),
}

type Callback = unsafe extern "system" fn(Grb, *mut c_void, c_int, *mut c_void) -> c_int;

/// Where Gurobi calls back from: its polling, presolve, simplex, MIP and barrier.
const CB_POLLING: c_int = 0;
const CB_MIP: c_int = 3;
const CB_MIP_OBJBST: c_int = 3000;
const CB_MIP_OBJBND: c_int = 3001;
const CB_RUNTIME: c_int = 6002;

/// What the callback holds: the watch, when it was told last, whether it stopped the solve.
struct Watched {
    watch: Box<dyn FnMut(f64, f64, f64, f64) -> bool>,
    told: std::time::Instant,
    cancelled: bool,
    cbget: unsafe extern "system" fn(*mut c_void, c_int, c_int, *mut c_void) -> c_int,
    terminate: unsafe extern "system" fn(Grb),
}

/// Gurobi's callback: the watch told how the solve goes a few times a second (the MIP's best objective, bound and
/// gap; NaN before it), and the solve terminated when it says to stop.
unsafe extern "system" fn callback(grb: Grb, cbdata: *mut c_void, at: c_int, usrdata: *mut c_void) -> c_int {
    let w = unsafe { &mut *(usrdata as *mut Watched) };
    if w.cancelled || w.told.elapsed() < std::time::Duration::from_millis(200) {
        return 0;
    }
    w.told = std::time::Instant::now();
    let get = |what: c_int| {
        let mut v = f64::NAN;
        let ok = unsafe { (w.cbget)(cbdata, at, what, &mut v as *mut f64 as *mut c_void) } == 0;
        if ok { v } else { f64::NAN }
    };
    let nan = f64::NAN;
    let (best, bound) = if at == CB_MIP { (get(CB_MIP_OBJBST), get(CB_MIP_OBJBND)) } else { (nan, nan) };
    // (no solution yet: Gurobi's infinity)
    let best = if best.abs() >= INFINITY { nan } else { best };
    let gap = if best.is_finite() && bound.is_finite() { (best - bound).abs() / best.abs().max(1e-10) } else { nan };
    let seconds = if at == CB_POLLING { nan } else { get(CB_RUNTIME) };
    let go_on = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (w.watch)(best, bound, gap, seconds)))
        .unwrap_or(false);
    if !go_on {
        w.cancelled = true;
        unsafe { (w.terminate)(grb) };
    }
    0
}

/// Gurobi loaded: its functions, version and file.
pub struct Gurobi {
    _lib: libloading::Library,
    api: Api,
    /// Its version, e.g. "12.0.3".
    pub version: String,
    pub library: PathBuf,
    major: c_int,
    minor: c_int,
    technical: c_int,
}

unsafe impl Send for Gurobi {}
unsafe impl Sync for Gurobi {}

static GUROBI: OnceLock<std::result::Result<Gurobi, String>> = OnceLock::new();
static LICENSED: OnceLock<std::result::Result<(), String>> = OnceLock::new();

/// Gurobi's library among the files of a folder: `gurobi120.dll`, `libgurobi120.so`, `libgurobi.12.0.3.dylib`...
/// (not its light, .NET or Java libraries).
fn is_gurobi(name: &str) -> bool {
    let n = name.to_lowercase();
    let version = |v: &str| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.');
    if cfg!(windows) {
        n.strip_prefix("gurobi").and_then(|r| r.strip_suffix(".dll")).is_some_and(version)
    } else if cfg!(target_os = "macos") {
        n.strip_prefix("libgurobi").and_then(|r| r.strip_suffix(".dylib")).is_some_and(version)
    } else {
        // libgurobi120.so, libgurobi.so.12.0.3
        n.strip_prefix("libgurobi")
            .is_some_and(|r| r.strip_suffix(".so").is_some_and(version) || r.strip_prefix(".so.").is_some_and(version))
    }
}

/// The library files in a folder, the newest version first.
fn in_folder(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| is_gurobi(&n.to_string_lossy())))
        .collect();
    v.sort();
    v.reverse();
    v
}

/// Where Gurobi's library may be, in the order tried.
fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(f) = std::env::var_os("TERRAFLUX_GUROBI") {
        out.push(PathBuf::from(f));
    }
    let lib = if cfg!(windows) { "bin" } else { "lib" };
    if let Some(home) = std::env::var_os("GUROBI_HOME") {
        out.extend(in_folder(&PathBuf::from(home).join(lib)));
    }
    // the installers' folders: C:\gurobi1203\win64, /opt/gurobi1203/linux64, /Library/gurobi1203/macos_universal2
    let (roots, platform): (&[&str], &[&str]) = if cfg!(windows) {
        (&["C:\\"], &["win64"])
    } else if cfg!(target_os = "macos") {
        (&["/Library"], &["macos_universal2"])
    } else {
        (&["/opt"], &["linux64", "armlinux64"])
    };
    let mut homes: Vec<PathBuf> = roots
        .iter()
        .filter_map(|r| std::fs::read_dir(r).ok())
        .flat_map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with("gurobi")))
        .collect();
    homes.sort();
    homes.reverse();
    for h in homes {
        for p in platform {
            out.extend(in_folder(&h.join(p).join(lib)));
        }
    }
    // the system's search path
    for v in ["130", "120", "110", "100"] {
        out.push(PathBuf::from(if cfg!(windows) {
            format!("gurobi{v}.dll")
        } else if cfg!(target_os = "macos") {
            format!("libgurobi{v}.dylib")
        } else {
            format!("libgurobi{v}.so")
        }));
    }
    out
}

fn load(path: &Path) -> std::result::Result<Gurobi, String> {
    let lib = unsafe { libloading::Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))?;
    macro_rules! f {
        ($name:ident) => {
            *unsafe { lib.get(concat!(stringify!($name), "\0").as_bytes()) }
                .map_err(|e| format!("{}: no {}: {e}", path.display(), stringify!($name)))?
        };
    }
    let api = Api {
        GRBversion: f!(GRBversion),
        GRBemptyenvinternal: f!(GRBemptyenvinternal),
        GRBstartenv: f!(GRBstartenv),
        GRBfreeenv: f!(GRBfreeenv),
        GRBgeterrormsg: f!(GRBgeterrormsg),
        GRBsetintparam: f!(GRBsetintparam),
        GRBsetdblparam: f!(GRBsetdblparam),
        GRBnewmodel: f!(GRBnewmodel),
        GRBXaddconstrs: f!(GRBXaddconstrs),
        GRBsetintattr: f!(GRBsetintattr),
        GRBgetintattr: f!(GRBgetintattr),
        GRBgetdblattr: f!(GRBgetdblattr),
        GRBgetdblattrarray: f!(GRBgetdblattrarray),
        GRBsetdblattrarray: f!(GRBsetdblattrarray),
        GRBgetenv: f!(GRBgetenv),
        GRBoptimize: f!(GRBoptimize),
        GRBfreemodel: f!(GRBfreemodel),
        GRBsetcallbackfunc: f!(GRBsetcallbackfunc),
        GRBcbget: f!(GRBcbget),
        GRBterminate: f!(GRBterminate),
    };
    let (mut major, mut minor, mut technical) = (0, 0, 0);
    unsafe { (api.GRBversion)(&mut major, &mut minor, &mut technical) };
    Ok(Gurobi {
        _lib: lib,
        api,
        version: format!("{major}.{minor}.{technical}"),
        library: path.to_path_buf(),
        major,
        minor,
        technical,
    })
}

/// Gurobi, or why it could not be had.
pub fn gurobi() -> std::result::Result<&'static Gurobi, &'static str> {
    GUROBI
        .get_or_init(|| {
            let mut tried = Vec::new();
            for c in candidates() {
                match load(&c) {
                    Ok(g) => return Ok(g),
                    Err(e) => tried.push(e),
                }
            }
            Err(format!(
                "Gurobi's library was not found (set GUROBI_HOME, or TERRAFLUX_GUROBI to the library's file): {}",
                tried.first().cloned().unwrap_or_default()
            ))
        })
        .as_ref()
        .map_err(String::as_str)
}

/// Whether Gurobi can solve here: its library found and a licence it accepts (checked once); else why not.
pub fn available() -> std::result::Result<&'static Gurobi, String> {
    let g = gurobi().map_err(String::from)?;
    LICENSED.get_or_init(|| g.env(0).map(|e| unsafe { (g.api.GRBfreeenv)(e) })).clone().map(|_| g)
}

fn c(s: &str) -> CString {
    CString::new(s).expect("a name without NUL")
}

impl Gurobi {
    /// The last error of an environment.
    fn error(&self, env: Env) -> String {
        let p = unsafe { (self.api.GRBgeterrormsg)(env) };
        if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
    }

    /// A started environment, silent, using `threads` (0: Gurobi's choice, every core).
    fn env(&self, threads: usize) -> std::result::Result<Env, String> {
        let mut env: Env = std::ptr::null_mut();
        let e = unsafe { (self.api.GRBemptyenvinternal)(&mut env, self.major, self.minor, self.technical) };
        if e != 0 || env.is_null() {
            return Err(format!("Gurobi's environment: error {e}"));
        }
        let fail = |what: &str| {
            let m = format!("Gurobi {what}: {}", self.error(env));
            unsafe { (self.api.GRBfreeenv)(env) };
            m
        };
        unsafe {
            (self.api.GRBsetintparam)(env, c("OutputFlag").as_ptr(), 0);
            if threads > 0 {
                (self.api.GRBsetintparam)(env, c("Threads").as_ptr(), threads as c_int);
            }
        }
        if unsafe { (self.api.GRBstartenv)(env) } != 0 {
            return Err(fail("could not start (its licence?)"));
        }
        Ok(env)
    }
}

/// A model solved by Gurobi as prioritizr's `add_gurobi_solver(gap, time_limit, threads)` solves it (`relaxation`:
/// every column continuous, the LP; `start`: a first solution, else none): its status, solution, objective, bound
/// and gap, as HiGHS's.
pub fn solve(
    m: &Model,
    gap: f64,
    time_limit: f64,
    relaxation: bool,
    start: &[f64],
    threads: usize,
) -> std::result::Result<SolveOut, String> {
    let g = available()?;
    let api = &g.api;
    let n = m.obj.len();
    // (as landschaft: every core but one unless told)
    let threads = if threads > 0 { threads } else { terraflux_ls_sys::ffi::max_threads().max(1) as usize };
    let env = g.env(threads)?;
    let mut grb: Grb = std::ptr::null_mut();
    let finite = |x: f64| x.clamp(-INFINITY, INFINITY);
    let mut obj = m.obj.clone();
    let mut lb: Vec<f64> = m.lb.iter().map(|&x| finite(x)).collect();
    let mut ub: Vec<f64> = m.ub.iter().map(|&x| finite(x)).collect();
    // (the LP: every column continuous)
    let mut vtype: Vec<c_char> = m.vtype.iter().map(|&t| (if relaxation { b'C' } else { t as u8 }) as c_char).collect();
    let out = (|| {
        let check =
            |e: c_int, what: &str| if e == 0 { Ok(()) } else { Err(format!("Gurobi {what}: {}", g.error(env))) };
        check(
            unsafe {
                (api.GRBnewmodel)(
                    env,
                    &mut grb,
                    c("terraflux").as_ptr(),
                    n as c_int,
                    obj.as_mut_ptr(),
                    lb.as_mut_ptr(),
                    ub.as_mut_ptr(),
                    vtype.as_mut_ptr(),
                    std::ptr::null_mut(),
                )
            },
            "model",
        )?;
        let mut beg: Vec<usize> = m.a.start[..m.a.nrow].to_vec();
        let mut ind: Vec<c_int> = m.a.index.iter().map(|&j| j as c_int).collect();
        let mut val = m.a.value.clone();
        let mut sense: Vec<c_char> = m
            .sense
            .iter()
            .map(|s| match s.as_str() {
                "<=" => b'<',
                ">=" => b'>',
                _ => b'=',
            } as c_char)
            .collect();
        let mut rhs: Vec<f64> = m.rhs.iter().map(|&x| finite(x)).collect();
        if m.a.nrow > 0 {
            check(
                unsafe {
                    (api.GRBXaddconstrs)(
                        grb,
                        m.a.nrow as c_int,
                        val.len(),
                        beg.as_mut_ptr(),
                        ind.as_mut_ptr(),
                        val.as_mut_ptr(),
                        sense.as_mut_ptr(),
                        rhs.as_mut_ptr(),
                        std::ptr::null_mut(),
                    )
                },
                "constraints",
            )?;
        }
        unsafe {
            check((api.GRBsetintattr)(grb, c("ModelSense").as_ptr(), if m.maximize { -1 } else { 1 }), "sense")?;
            let menv = (api.GRBgetenv)(grb);
            check((api.GRBsetdblparam)(menv, c("MIPGap").as_ptr(), gap), "MIPGap")?;
            if time_limit.is_finite() {
                check((api.GRBsetdblparam)(menv, c("TimeLimit").as_ptr(), time_limit), "TimeLimit")?;
            }
            if start.len() == n {
                let mut s = start.to_vec();
                check((api.GRBsetdblattrarray)(grb, c("Start").as_ptr(), 0, n as c_int, s.as_mut_ptr()), "Start")?;
            }
        }
        // this thread's watch told how it goes, and stopping it
        let mut watched = crate::milp::current_watch().map(|watch| Watched {
            watch,
            // (asked at once the first time)
            told: std::time::Instant::now() - std::time::Duration::from_millis(200),
            cancelled: false,
            cbget: api.GRBcbget,
            terminate: api.GRBterminate,
        });
        if let Some(w) = watched.as_mut() {
            let data = w as *mut Watched as *mut c_void;
            check(unsafe { (api.GRBsetcallbackfunc)(grb, callback, data) }, "callback")?;
        }
        check(unsafe { (api.GRBoptimize)(grb) }, "optimize")?;
        let cancelled = watched.as_ref().is_some_and(|w| w.cancelled);
        let int = |name: &str| {
            let mut v = 0;
            (unsafe { (api.GRBgetintattr)(grb, c(name).as_ptr(), &mut v) } == 0).then_some(v)
        };
        let dbl = |name: &str| {
            let mut v = f64::NAN;
            (unsafe { (api.GRBgetdblattr)(grb, c(name).as_ptr(), &mut v) } == 0).then_some(v)
        };
        let status = match int("Status").unwrap_or(0) {
            2 => "optimal".to_string(),
            3 => "infeasible".into(),
            4 => "infeasible_or_unbounded".into(),
            5 => "unbounded".into(),
            9 => "time_limit".into(),
            11 if cancelled => "cancelled".into(),
            11 => "error: interrupted".into(),
            13 => "suboptimal".into(),
            s => format!("error: Gurobi's status {s}"),
        };
        let has_solution = int("SolCount").unwrap_or(0) > 0;
        let mut x = vec![f64::NAN; n];
        let (mut objective, mut bound, mut gap_out) = (f64::NAN, f64::NAN, f64::NAN);
        if has_solution {
            check(unsafe { (api.GRBgetdblattrarray)(grb, c("X").as_ptr(), 0, n as c_int, x.as_mut_ptr()) }, "X")?;
            objective = dbl("ObjVal").unwrap_or(f64::NAN);
            let mip = int("IsMIP").unwrap_or(0) == 1;
            bound = if mip { dbl("ObjBound").unwrap_or(objective) } else { objective };
            gap_out = if mip { dbl("MIPGap").unwrap_or(0.0) } else { 0.0 };
        }
        Ok(SolveOut { status, has_solution, x, objective, bound, gap: gap_out })
    })();
    unsafe {
        if !grb.is_null() {
            (api.GRBfreemodel)(grb);
        }
        (api.GRBfreeenv)(env);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_is_told_from_its_neighbours() {
        if cfg!(windows) {
            assert!(is_gurobi("gurobi120.dll") && is_gurobi("Gurobi110.dll"));
            assert!(!is_gurobi("gurobi120_light.dll") && !is_gurobi("Gurobi120.NET.dll"));
            assert!(!is_gurobi("GurobiJni120.dll"));
        } else if !cfg!(target_os = "macos") {
            assert!(is_gurobi("libgurobi120.so") && is_gurobi("libgurobi.so.12.0.3"));
            assert!(!is_gurobi("libgurobi_light.so"));
        }
    }
}
