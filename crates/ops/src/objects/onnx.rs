//! ONNX Runtime loaded at run time, as the GDAL is: the shared library is found and opened when an operator that
//! needs it runs (`segment.sam`, `objects.embed`), through its C API with a hand-written minimal FFI, so nothing
//! heavy is built or linked and the rest of Terraflux works without it. Only float32 tensors are carried (what SAM's
//! and the foundation models' exports take and give).
//!
//! The C API is `OrtApi`, a struct of function pointers whose order is its ABI. The order below is that of
//! `onnxruntime_c_api.h` of ONNX Runtime 1.17 (`ORT_API_VERSION` 17); a newer runtime keeps it for the version
//! asked in `GetApi` (functions are only appended), and an older one than 1.17 answers `GetApi(17)` with null, which
//! is refused with a message. The functions used are listed in [`idx`] with their position: the order must match the
//! header, so do not renumber them without checking it.

use std::ffi::{CStr, CString, OsString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{Error, Result};

/// The API version asked of the runtime (ONNX Runtime 1.17).
pub const ORT_API_VERSION: u32 = 17;

/// A float32 tensor: its shape and its values (row-major).
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    pub shape: Vec<i64>,
    pub data: Vec<f32>,
}

impl Tensor {
    pub fn new(shape: Vec<i64>, data: Vec<f32>) -> Tensor {
        debug_assert_eq!(shape.iter().product::<i64>() as usize, data.len());
        Tensor { shape, data }
    }
}

/// What the deep-learning operators need of a model: run it. A trait so that the parts that are plain Rust (tiling,
/// resolving the masks, averaging the patches) are tested with a fake.
pub trait Infer {
    /// The names of the model's inputs, in order.
    fn input_names(&self) -> Vec<String>;
    /// The names of the model's outputs, in order.
    fn output_names(&self) -> Vec<String>;
    /// Run it: the inputs by name, the outputs asked for by name (in that order).
    fn run(&self, inputs: &[(&str, &Tensor)], outputs: &[&str]) -> Result<Vec<Tensor>>;
}

/// How to install ONNX Runtime, for the messages.
const INSTALL: &str = "ONNX Runtime is needed for this operator and was not found. Install it with `pip install \
     onnxruntime` (the package ships onnxruntime.dll / libonnxruntime.so / libonnxruntime.dylib in its capi folder) \
     or download a release from https://github.com/microsoft/onnxruntime/releases, then set the environment \
     variable ORT_DYLIB_PATH to the full path of the shared library (or put it beside the executable or on the \
     library path). The rest of the objects operators works without it";

/// The file names of the shared library on this system.
fn library_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["onnxruntime.dll"]
    } else if cfg!(target_os = "macos") {
        &["libonnxruntime.dylib"]
    } else {
        &["libonnxruntime.so", "libonnxruntime.so.1"]
    }
}

/// Positions in `OrtApi` (onnxruntime_c_api.h, 1.17) of the functions used. The order must match the header.
mod idx {
    pub const GET_ERROR_MESSAGE: usize = 2;
    pub const CREATE_ENV: usize = 3;
    pub const CREATE_SESSION: usize = 7;
    pub const RUN: usize = 9;
    pub const CREATE_SESSION_OPTIONS: usize = 10;
    pub const SET_INTRA_OP_NUM_THREADS: usize = 24;
    pub const SESSION_GET_INPUT_COUNT: usize = 30;
    pub const SESSION_GET_OUTPUT_COUNT: usize = 31;
    pub const SESSION_GET_INPUT_NAME: usize = 36;
    pub const SESSION_GET_OUTPUT_NAME: usize = 37;
    pub const CREATE_TENSOR_WITH_DATA_AS_ORT_VALUE: usize = 49;
    pub const GET_TENSOR_MUTABLE_DATA: usize = 51;
    pub const GET_TENSOR_ELEMENT_TYPE: usize = 60;
    pub const GET_DIMENSIONS_COUNT: usize = 61;
    pub const GET_DIMENSIONS: usize = 62;
    pub const GET_TENSOR_SHAPE_ELEMENT_COUNT: usize = 64;
    pub const GET_TENSOR_TYPE_AND_SHAPE: usize = 65;
    pub const CREATE_CPU_MEMORY_INFO: usize = 69;
    pub const ALLOCATOR_FREE: usize = 76;
    pub const GET_ALLOCATOR_WITH_DEFAULT_OPTIONS: usize = 78;
    pub const RELEASE_ENV: usize = 92;
    pub const RELEASE_STATUS: usize = 93;
    pub const RELEASE_MEMORY_INFO: usize = 94;
    pub const RELEASE_SESSION: usize = 95;
    pub const RELEASE_VALUE: usize = 96;
    pub const RELEASE_TENSOR_TYPE_AND_SHAPE_INFO: usize = 99;
    pub const RELEASE_SESSION_OPTIONS: usize = 100;
    /// How many entries of the table are read.
    pub const COUNT: usize = 101;
}

type Status = *mut c_void;
type Ptr = *mut c_void;

#[cfg(windows)]
type OrtChar = u16;
#[cfg(not(windows))]
type OrtChar = c_char;

#[repr(C)]
struct ApiBase {
    get_api: unsafe extern "system" fn(u32) -> *const *const c_void,
}

/// The functions of the C API that are used, typed.
struct Fns {
    get_error_message: unsafe extern "system" fn(Status) -> *const c_char,
    create_env: unsafe extern "system" fn(c_int, *const c_char, *mut Ptr) -> Status,
    create_session: unsafe extern "system" fn(Ptr, *const OrtChar, Ptr, *mut Ptr) -> Status,
    run: unsafe extern "system" fn(
        Ptr,
        Ptr,
        *const *const c_char,
        *const Ptr,
        usize,
        *const *const c_char,
        usize,
        *mut Ptr,
    ) -> Status,
    create_session_options: unsafe extern "system" fn(*mut Ptr) -> Status,
    set_intra_op_num_threads: unsafe extern "system" fn(Ptr, c_int) -> Status,
    session_get_count: [unsafe extern "system" fn(Ptr, *mut usize) -> Status; 2],
    session_get_name: [unsafe extern "system" fn(Ptr, usize, Ptr, *mut *mut c_char) -> Status; 2],
    create_tensor: unsafe extern "system" fn(Ptr, *mut c_void, usize, *const i64, usize, c_int, *mut Ptr) -> Status,
    get_tensor_mutable_data: unsafe extern "system" fn(Ptr, *mut *mut c_void) -> Status,
    get_tensor_element_type: unsafe extern "system" fn(Ptr, *mut c_int) -> Status,
    get_dimensions_count: unsafe extern "system" fn(Ptr, *mut usize) -> Status,
    get_dimensions: unsafe extern "system" fn(Ptr, *mut i64, usize) -> Status,
    get_element_count: unsafe extern "system" fn(Ptr, *mut usize) -> Status,
    get_type_and_shape: unsafe extern "system" fn(Ptr, *mut Ptr) -> Status,
    create_cpu_memory_info: unsafe extern "system" fn(c_int, c_int, *mut Ptr) -> Status,
    allocator_free: unsafe extern "system" fn(Ptr, *mut c_void) -> Status,
    get_allocator: unsafe extern "system" fn(*mut Ptr) -> Status,
    release_env: unsafe extern "system" fn(Ptr),
    release_status: unsafe extern "system" fn(Status),
    release_memory_info: unsafe extern "system" fn(Ptr),
    release_session: unsafe extern "system" fn(Ptr),
    release_value: unsafe extern "system" fn(Ptr),
    release_shape_info: unsafe extern "system" fn(Ptr),
    release_session_options: unsafe extern "system" fn(Ptr),
}

/// The loaded runtime: the library (kept open), its functions and an environment.
pub struct Ort {
    f: Fns,
    env: Ptr,
    mem: Ptr,
    /// Where it was loaded from.
    pub path: PathBuf,
    // (last: the library closes after the rest is released in `drop`)
    _lib: libloading::Library,
}

// The runtime's handles are thread-safe by its documentation (the environment, the memory info, the sessions).
unsafe impl Send for Ort {}
unsafe impl Sync for Ort {}

impl Drop for Ort {
    fn drop(&mut self) {
        unsafe {
            (self.f.release_memory_info)(self.mem);
            (self.f.release_env)(self.env);
        }
    }
}

/// The libraries to try, in order: the one `ORT_DYLIB_PATH` names (the only one when it is set), the file beside
/// the executable, then the name alone (the system's library path).
fn candidates(env: Option<&OsString>) -> Vec<PathBuf> {
    if let Some(p) = env.filter(|p| !p.is_empty()) {
        return vec![PathBuf::from(p)];
    }
    let mut v = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        v.extend(library_names().iter().map(|n| dir.join(n)).filter(|p| p.is_file()));
    }
    v.extend(library_names().iter().map(PathBuf::from));
    v
}

impl Ort {
    /// Find and load the runtime: `ORT_DYLIB_PATH`, else beside the executable, else the library path.
    pub fn load() -> Result<Arc<Ort>> {
        Ort::load_with(std::env::var_os("ORT_DYLIB_PATH").as_ref())
    }

    /// As [`Ort::load`], with the value of `ORT_DYLIB_PATH` given (none: not set).
    pub fn load_with(env: Option<&OsString>) -> Result<Arc<Ort>> {
        let mut tried: Vec<String> = Vec::new();
        for p in candidates(env) {
            // (a path that names a folder or a missing file: said as it is, with how to install)
            if env.is_some_and(|e| !e.is_empty()) && !p.is_file() {
                return Err(Error::msg(format!("ORT_DYLIB_PATH is {}, which is not a file. {INSTALL}", p.display())));
            }
            match unsafe { Ort::open(&p) } {
                Ok(o) => return Ok(Arc::new(o)),
                Err(e) => tried.push(format!("{}: {e}", p.display())),
            }
        }
        Err(Error::msg(format!("{INSTALL}. Tried {}", tried.join("; "))))
    }

    unsafe fn open(path: &Path) -> std::result::Result<Ort, String> {
        let lib = unsafe { libloading::Library::new(path) }.map_err(|e| e.to_string())?;
        let base: unsafe extern "system" fn() -> *const ApiBase =
            *unsafe { lib.get(b"OrtGetApiBase\0") }.map_err(|e| format!("no OrtGetApiBase ({e})"))?;
        let base = unsafe { base() };
        if base.is_null() {
            return Err("OrtGetApiBase gave nothing".into());
        }
        let api = unsafe { ((*base).get_api)(ORT_API_VERSION) };
        if api.is_null() {
            return Err(format!(
                "the runtime does not have the API version {ORT_API_VERSION} (ONNX Runtime 1.17 or newer)"
            ));
        }
        let at = |i: usize| -> *const c_void {
            debug_assert!(i < idx::COUNT);
            unsafe { *api.add(i) }
        };
        macro_rules! f {
            ($i:expr) => {
                unsafe { std::mem::transmute_copy(&at($i)) }
            };
        }
        let f = Fns {
            get_error_message: f!(idx::GET_ERROR_MESSAGE),
            create_env: f!(idx::CREATE_ENV),
            create_session: f!(idx::CREATE_SESSION),
            run: f!(idx::RUN),
            create_session_options: f!(idx::CREATE_SESSION_OPTIONS),
            set_intra_op_num_threads: f!(idx::SET_INTRA_OP_NUM_THREADS),
            session_get_count: [f!(idx::SESSION_GET_INPUT_COUNT), f!(idx::SESSION_GET_OUTPUT_COUNT)],
            session_get_name: [f!(idx::SESSION_GET_INPUT_NAME), f!(idx::SESSION_GET_OUTPUT_NAME)],
            create_tensor: f!(idx::CREATE_TENSOR_WITH_DATA_AS_ORT_VALUE),
            get_tensor_mutable_data: f!(idx::GET_TENSOR_MUTABLE_DATA),
            get_tensor_element_type: f!(idx::GET_TENSOR_ELEMENT_TYPE),
            get_dimensions_count: f!(idx::GET_DIMENSIONS_COUNT),
            get_dimensions: f!(idx::GET_DIMENSIONS),
            get_element_count: f!(idx::GET_TENSOR_SHAPE_ELEMENT_COUNT),
            get_type_and_shape: f!(idx::GET_TENSOR_TYPE_AND_SHAPE),
            create_cpu_memory_info: f!(idx::CREATE_CPU_MEMORY_INFO),
            allocator_free: f!(idx::ALLOCATOR_FREE),
            get_allocator: f!(idx::GET_ALLOCATOR_WITH_DEFAULT_OPTIONS),
            release_env: f!(idx::RELEASE_ENV),
            release_status: f!(idx::RELEASE_STATUS),
            release_memory_info: f!(idx::RELEASE_MEMORY_INFO),
            release_session: f!(idx::RELEASE_SESSION),
            release_value: f!(idx::RELEASE_VALUE),
            release_shape_info: f!(idx::RELEASE_TENSOR_TYPE_AND_SHAPE_INFO),
            release_session_options: f!(idx::RELEASE_SESSION_OPTIONS),
        };
        let mut o =
            Ort { f, env: std::ptr::null_mut(), mem: std::ptr::null_mut(), path: path.to_path_buf(), _lib: lib };
        let id = CString::new("terraflux").unwrap();
        // (warnings only: level 2)
        let st = unsafe { (o.f.create_env)(2, id.as_ptr(), &mut o.env) };
        o.check(st, "CreateEnv")?;
        // 1 = arena allocator, 0 = default memory type
        let st = unsafe { (o.f.create_cpu_memory_info)(1, 0, &mut o.mem) };
        o.check(st, "CreateCpuMemoryInfo")?;
        Ok(o)
    }

    /// A status of the runtime as a result: null is fine, else the message (the status released).
    fn check(&self, st: Status, what: &str) -> std::result::Result<(), String> {
        if st.is_null() {
            return Ok(());
        }
        let msg = unsafe {
            let m = (self.f.get_error_message)(st);
            if m.is_null() { String::new() } else { CStr::from_ptr(m).to_string_lossy().into_owned() }
        };
        unsafe { (self.f.release_status)(st) };
        Err(format!("ONNX Runtime {what}: {msg}"))
    }

    /// A model loaded from a file, its session's intra-op threads within Terraflux's cap.
    pub fn session(self: &Arc<Self>, model: &Path) -> Result<Session> {
        if !model.is_file() {
            return Err(Error::msg(format!("the model {} is not a file", model.display())));
        }
        let e = |m: String| Error::msg(format!("{}: {m}", model.display()));
        let mut opts: Ptr = std::ptr::null_mut();
        let st = unsafe { (self.f.create_session_options)(&mut opts) };
        self.check(st, "CreateSessionOptions").map_err(e)?;
        let threads = terraflux_ls_sys::threads::most().max(1) as c_int;
        let st = unsafe { (self.f.set_intra_op_num_threads)(opts, threads) };
        let made = self.check(st, "SetIntraOpNumThreads").and_then(|_| {
            #[cfg(windows)]
            let path: Vec<OrtChar> = {
                use std::os::windows::ffi::OsStrExt;
                model.as_os_str().encode_wide().chain([0]).collect()
            };
            #[cfg(not(windows))]
            let path: Vec<OrtChar> = {
                use std::os::unix::ffi::OsStrExt;
                model.as_os_str().as_bytes().iter().map(|&b| b as c_char).chain([0]).collect()
            };
            let mut session: Ptr = std::ptr::null_mut();
            let st = unsafe { (self.f.create_session)(self.env, path.as_ptr(), opts, &mut session) };
            self.check(st, "CreateSession").map(|_| session)
        });
        unsafe { (self.f.release_session_options)(opts) };
        let session = made.map_err(e)?;
        let mut s = Session { ort: self.clone(), session, inputs: Vec::new(), outputs: Vec::new() };
        s.inputs = s.names(0).map_err(e)?;
        s.outputs = s.names(1).map_err(e)?;
        Ok(s)
    }
}

/// A model ready to run.
pub struct Session {
    ort: Arc<Ort>,
    session: Ptr,
    inputs: Vec<String>,
    outputs: Vec<String>,
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { (self.ort.f.release_session)(self.session) };
    }
}

/// An OrtValue released when it goes out of scope.
struct Value<'a>(&'a Ort, Ptr);

impl Drop for Value<'_> {
    fn drop(&mut self) {
        if !self.1.is_null() {
            unsafe { (self.0.f.release_value)(self.1) };
        }
    }
}

impl Session {
    /// The names of the inputs (`which` 0) or the outputs (1).
    fn names(&self, which: usize) -> std::result::Result<Vec<String>, String> {
        let f = &self.ort.f;
        let mut alloc: Ptr = std::ptr::null_mut();
        self.ort.check(unsafe { (f.get_allocator)(&mut alloc) }, "GetAllocatorWithDefaultOptions")?;
        let mut n = 0usize;
        self.ort.check(unsafe { (f.session_get_count[which])(self.session, &mut n) }, "SessionGetInputCount")?;
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            let mut p: *mut c_char = std::ptr::null_mut();
            self.ort.check(unsafe { (f.session_get_name[which])(self.session, k, alloc, &mut p) }, "SessionGetName")?;
            out.push(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned());
            let st = unsafe { (f.allocator_free)(alloc, p as *mut c_void) };
            self.ort.check(st, "AllocatorFree")?;
        }
        Ok(out)
    }
}

impl Infer for Session {
    fn input_names(&self) -> Vec<String> {
        self.inputs.clone()
    }

    fn output_names(&self) -> Vec<String> {
        self.outputs.clone()
    }

    fn run(&self, inputs: &[(&str, &Tensor)], outputs: &[&str]) -> Result<Vec<Tensor>> {
        let ort = &*self.ort;
        let f = &ort.f;
        let err = |m: String| Error::msg(m);
        let names: Vec<CString> =
            inputs.iter().map(|(n, _)| CString::new(*n).map_err(|e| err(e.to_string()))).collect::<Result<_>>()?;
        let out_names: Vec<CString> =
            outputs.iter().map(|n| CString::new(*n).map_err(|e| err(e.to_string()))).collect::<Result<_>>()?;
        // the inputs are views on the tensors' memory (the runtime does not write to inputs)
        let mut values: Vec<Value> = Vec::with_capacity(inputs.len());
        for (name, t) in inputs {
            let mut p: Ptr = std::ptr::null_mut();
            let st = unsafe {
                (f.create_tensor)(
                    ort.mem,
                    t.data.as_ptr() as *mut c_void,
                    t.data.len() * 4,
                    t.shape.as_ptr(),
                    t.shape.len(),
                    1, // ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT
                    &mut p,
                )
            };
            ort.check(st, &format!("CreateTensorWithDataAsOrtValue for {name}")).map_err(err)?;
            values.push(Value(ort, p));
        }
        let in_ptrs: Vec<*const c_char> = names.iter().map(|n| n.as_ptr()).collect();
        let in_vals: Vec<Ptr> = values.iter().map(|v| v.1).collect();
        let out_ptrs: Vec<*const c_char> = out_names.iter().map(|n| n.as_ptr()).collect();
        let mut out_vals: Vec<Ptr> = vec![std::ptr::null_mut(); outputs.len()];
        let st = unsafe {
            (f.run)(
                self.session,
                std::ptr::null_mut(),
                in_ptrs.as_ptr(),
                in_vals.as_ptr(),
                in_vals.len(),
                out_ptrs.as_ptr(),
                out_ptrs.len(),
                out_vals.as_mut_ptr(),
            )
        };
        // (released whatever came back, whatever happens next)
        let results: Vec<Value> = out_vals.into_iter().map(|p| Value(ort, p)).collect();
        ort.check(st, "Run").map_err(err)?;
        results
            .iter()
            .zip(outputs)
            .map(|(v, name)| self.read(v.1).map_err(|m| err(format!("output {name}: {m}"))))
            .collect()
    }
}

impl Session {
    /// A float tensor the runtime gave, copied.
    fn read(&self, v: Ptr) -> std::result::Result<Tensor, String> {
        let ort = &*self.ort;
        let f = &ort.f;
        let mut info: Ptr = std::ptr::null_mut();
        ort.check(unsafe { (f.get_type_and_shape)(v, &mut info) }, "GetTensorTypeAndShape")?;
        let r = (|| {
            let mut kind: c_int = 0;
            ort.check(unsafe { (f.get_tensor_element_type)(info, &mut kind) }, "GetTensorElementType")?;
            if kind != 1 {
                return Err(format!("the element type {kind} is not float32 (ONNX type 1)"));
            }
            let mut dims = 0usize;
            ort.check(unsafe { (f.get_dimensions_count)(info, &mut dims) }, "GetDimensionsCount")?;
            let mut shape = vec![0i64; dims];
            ort.check(unsafe { (f.get_dimensions)(info, shape.as_mut_ptr(), dims) }, "GetDimensions")?;
            let mut count = 0usize;
            ort.check(unsafe { (f.get_element_count)(info, &mut count) }, "GetTensorShapeElementCount")?;
            let mut p: *mut c_void = std::ptr::null_mut();
            ort.check(unsafe { (f.get_tensor_mutable_data)(v, &mut p) }, "GetTensorMutableData")?;
            let data = if count == 0 || p.is_null() {
                Vec::new()
            } else {
                unsafe { std::slice::from_raw_parts(p as *const f32, count) }.to_vec()
            };
            Ok(Tensor { shape, data })
        })();
        unsafe { (f.release_shape_info)(info) };
        r
    }
}

/// A model from a path given as a setting, loading the runtime first.
pub fn open(ort: &Arc<Ort>, node: &str, what: &str, path: &str) -> Result<Session> {
    ort.session(Path::new(path)).map_err(|e| Error::msg(format!("node {node:?}: {what}: {e}")))
}
