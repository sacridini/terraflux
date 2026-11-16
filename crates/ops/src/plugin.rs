//! Plugins: operators written in any language that compiles to WebAssembly (Rust, C, Zig, AssemblyScript...),
//! run in a sandbox (no files, no network, a budget of instructions), so a model can send them to any worker.
//!
//! `plugin.wasm` has three interfaces (`params.interface`); the module may be binary (`.wasm`) or text (`.wat`),
//! and imports nothing. Inputs are `params.args` in order, else all by name; missing cells are NaN.
//!
//! `cell` (the default): the function (`params.function`, `cell`) is called for every cell with the node's inputs
//! as `f64` arguments (maps give their cell's value, numbers the same value everywhere) and returns the cell's
//! value (`f64`).
//!
//! ```wat
//! (module (func (export "cell") (param $land f64) (param $slope f64) (result f64)
//!   (if (result f64) (f64.lt (local.get $slope) (f64.const 10))
//!     (then (local.get $land)) (else (f64.const 0)))))
//! ```
//!
//! `window` (`params.radius` = r >= 1): the module exports `memory` and `alloc(bytes: i32) -> i32`, and the
//! function (`window`) `(ptr, side, n_inputs, row, col: i32) -> f64`. For every cell the host writes at `ptr` the
//! `side` x `side` window (`side` = 2r + 1) of each input, one block of `side * side` `f64` per input one after
//! the other, row by row, NaN outside the map (a number fills its block), and `row` and `col` are the cell's.
//!
//! `map`: the module exports `memory`, `alloc` and the function (`map`)
//! `(rows, cols, n_in, inputs, n_out, outputs: i32) -> i32` (ten `i32` when the node has tables: see below),
//! called once. `inputs` and `outputs` point at `n_in` and `n_out` pointers (`i32`) to `f64[rows * cols]` (a number
//! input fills its array); the host fills the outputs with NaN. It returns 0, or an error code. The outputs are
//! named by `params.outputs` (`map` when none). Tables: the function takes four more `i32`,
//! `(n_in_cols, in_cols, n_out_cols, out_cols)`, each column a descriptor of five `i32`
//! `(table, data, n_rows, name_ptr, name_len)`; the input tables' columns are given by the host
//! (`data` points at `f64[n_rows]`, the name is UTF-8), and the output tables' (`params.table_outputs`) are filled
//! by the module: it allocates the column, and writes `data` and `n_rows` in the descriptor.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, MAX_FUEL, OpSignature, PLUGIN, PortSpec, Type, plugin_problems};
use wasmi::Val;
use wasmi::core::ValType;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// Instructions a call may run (a function that loops forever stops).
const FUEL_PER_CELL: u64 = 1_000_000;

/// The most bytes of maps and tables the `map` interface hands a module (its memory has 32 bits).
const MAX_MEMORY: u64 = 3_500_000_000;

/// The most bytes of windows written for a call of the `window` interface.
const MAX_WINDOW: usize = 256 << 20;

/// A module read and compiled.
pub fn compile(path: &str) -> Result<(wasmi::Engine, wasmi::Module)> {
    let bytes = std::fs::read(path).map_err(|e| Error::msg(format!("the plugin {path}: {e}")))?;
    let wasm = if bytes.starts_with(b"\0asm") {
        bytes
    } else {
        wat::parse_bytes(&bytes).map_err(|e| Error::msg(format!("the plugin {path}: {e}")))?.into_owned()
    };
    let mut config = wasmi::Config::default();
    config.consume_fuel(true);
    let engine = wasmi::Engine::new(&config);
    let module = wasmi::Module::new(&engine, &wasm[..]).map_err(|e| Error::msg(format!("the plugin {path}: {e}")))?;
    Ok((engine, module))
}

/// Run `function` on chunks of cells: `args(i)` the arguments of cell `i`.
fn run_cells(
    engine: &wasmi::Engine,
    module: &wasmi::Module,
    function: &str,
    n: usize,
    arity: usize,
    fuel: u64,
    args: &(dyn Fn(usize, &mut Vec<f64>) + Sync),
) -> Result<Vec<f64>> {
    let chunk = 1 << 14;
    let parts: Vec<Vec<f64>> = (0..n.div_ceil(chunk))
        .into_par_iter()
        .map(|c| -> Result<Vec<f64>> {
            let mut store = wasmi::Store::new(engine, ());
            let linker = wasmi::Linker::<()>::new(engine);
            let instance = linker
                .instantiate_and_start(&mut store, module)
                .map_err(|e| Error::msg(format!("the plugin: {e} (it may import nothing)")))?;
            let f = instance
                .get_func(&store, function)
                .ok_or_else(|| Error::msg(format!("the plugin exports no function {function:?}")))?;
            let ty = f.ty(&store);
            if ty.params().len() != arity
                || ty.params().iter().any(|p| *p != wasmi::core::ValType::F64)
                || ty.results() != [wasmi::core::ValType::F64]
            {
                return Err(Error::msg(format!(
                    "the plugin's {function:?} must take {arity} f64 and give one f64 (it takes {:?}, gives {:?})",
                    ty.params(),
                    ty.results()
                )));
            }
            let (lo, hi) = (c * chunk, ((c + 1) * chunk).min(n));
            let mut vals = Vec::with_capacity(arity);
            let mut params = vec![wasmi::Val::F64(0.0.into()); arity];
            let mut result = [wasmi::Val::F64(0.0.into())];
            let mut out = Vec::with_capacity(hi - lo);
            store.set_fuel(fuel.saturating_mul((hi - lo) as u64)).map_err(|e| Error::msg(e.to_string()))?;
            for i in lo..hi {
                vals.clear();
                args(i, &mut vals);
                for (p, v) in params.iter_mut().zip(&vals) {
                    *p = wasmi::Val::F64((*v).into());
                }
                f.call(&mut store, &params, &mut result)
                    .map_err(|e| Error::msg(format!("the plugin's {function:?} at cell {i}: {}", explain(e))))?;
                out.push(match result[0] {
                    wasmi::Val::F64(x) => f64::from(x),
                    _ => f64::NAN,
                });
            }
            Ok(out)
        })
        .collect::<Result<_>>()?;
    Ok(parts.concat())
}

/// A call's error with a hint when it ran out of fuel.
fn explain(e: impl Display) -> String {
    let m = e.to_string();
    if m.contains("fuel") { format!("{m} (the budget is `fuel` instructions per cell, up to {MAX_FUEL})") } else { m }
}

/// An instance of a module of the `window` and `map` interfaces: its memory, `alloc` and the function.
struct Guest {
    store: wasmi::Store<()>,
    mem: wasmi::Memory,
    alloc: wasmi::Func,
    f: wasmi::Func,
}

impl Guest {
    /// The module instantiated with `fuel`; `function` takes `params` and gives `result`.
    #[allow(clippy::too_many_arguments)]
    fn new(
        engine: &wasmi::Engine,
        module: &wasmi::Module,
        function: &str,
        params: &[ValType],
        result: ValType,
        fuel: u64,
        what: &str,
        help: &str,
    ) -> Result<Guest> {
        let mut store = wasmi::Store::new(engine, ());
        store.set_fuel(fuel).map_err(|e| Error::msg(e.to_string()))?;
        let linker = wasmi::Linker::<()>::new(engine);
        let instance = linker
            .instantiate_and_start(&mut store, module)
            .map_err(|e| Error::msg(format!("the plugin: {} (it may import nothing)", explain(e))))?;
        let needs = format!("the {what} interface needs `memory`, `alloc(bytes: i32) -> i32` and {function:?}");
        let f = instance
            .get_func(&store, function)
            .ok_or_else(|| Error::msg(format!("the plugin exports no function {function:?} ({needs})")))?;
        let ty = f.ty(&store);
        if ty.params() != params || ty.results() != [result] {
            let rt = format!("{result:?}").to_lowercase();
            return Err(Error::msg(format!(
                "the plugin's {function:?} must take {} i32 and give one {rt}{help} (it takes {:?}, gives {:?})",
                params.len(),
                ty.params(),
                ty.results()
            )));
        }
        let mem = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| Error::msg(format!("the plugin exports no memory ({needs})")))?;
        let alloc = instance
            .get_func(&store, "alloc")
            .ok_or_else(|| Error::msg(format!("the plugin exports no function \"alloc\" ({needs})")))?;
        let ty = alloc.ty(&store);
        if ty.params() != [ValType::I32] || ty.results() != [ValType::I32] {
            return Err(Error::msg(format!(
                "the plugin's \"alloc\" must take one i32 and give one i32 (it takes {:?}, gives {:?})",
                ty.params(),
                ty.results()
            )));
        }
        Ok(Guest { store, mem, alloc, f })
    }

    /// `n` bytes of the module's memory at `at`, if they are there.
    fn bytes(&mut self, at: usize, n: usize) -> Result<&mut [u8]> {
        let mem = self.mem.data_mut(&mut self.store);
        match at.checked_add(n) {
            Some(end) if end <= mem.len() => Ok(&mut mem[at..end]),
            _ => Err(Error::msg(format!("the plugin's memory has no bytes {at}..{at}+{n}"))),
        }
    }

    /// A block of `n` bytes from the module's `alloc`.
    fn alloc(&mut self, n: usize) -> Result<usize> {
        if n == 0 {
            return Ok(0);
        }
        let size = u32::try_from(n)
            .map_err(|_| Error::msg("a block of 4 GiB cannot be given to a module (its memory has 32 bits)"))?;
        let mut r = [Val::I32(0)];
        self.alloc
            .call(&mut self.store, &[Val::I32(size as i32)], &mut r)
            .map_err(|e| Error::msg(format!("the plugin's alloc: {}", explain(e))))?;
        let ptr = match r[0] {
            Val::I32(p) => p as u32 as usize,
            _ => 0,
        };
        self.bytes(ptr, n)
            .map_err(|_| Error::msg(format!("the plugin's alloc gave {ptr}, outside its memory ({n} bytes asked)")))?;
        Ok(ptr)
    }

    fn put_i32s(&mut self, at: usize, vals: &[i32]) -> Result<()> {
        let dst = self.bytes(at, vals.len() * 4)?;
        for (d, v) in dst.as_chunks_mut::<4>().0.iter_mut().zip(vals) {
            d.copy_from_slice(&v.to_le_bytes());
        }
        Ok(())
    }

    fn i32s(&mut self, at: usize, n: usize) -> Result<Vec<i32>> {
        Ok(self.bytes(at, n * 4)?.as_chunks::<4>().0.iter().map(|b| i32::from_le_bytes(*b)).collect())
    }

    fn f64s(&mut self, at: usize, n: usize) -> Result<Vec<f64>> {
        Ok(self.bytes(at, n * 8)?.as_chunks::<8>().0.iter().map(|b| f64::from_le_bytes(*b)).collect())
    }

    /// Call the function with `i32` arguments.
    fn call(&mut self, args: &[i32], result: &mut [Val; 1]) -> std::result::Result<(), wasmi::Error> {
        let args: Vec<Val> = args.iter().map(|a| Val::I32(*a)).collect();
        self.f.call(&mut self.store, &args, result)
    }
}

/// The pointer as the module's `i32`.
fn ptr(p: usize) -> i32 {
    p as u32 as i32
}

/// The windows of `cols` around the cell (`row`, `col`) written at `dst`: a block per input, `side` x `side`
/// `f64` row by row, NaN outside the map.
fn fill_window(dst: &mut [u8], cols: &[Col], grid: &terraflux_raster::Grid, row: usize, col: usize, radius: usize) {
    let (w, h) = (grid.width, grid.height);
    let side = 2 * radius + 1;
    let mut out = dst.as_chunks_mut::<8>().0.iter_mut();
    for c in cols {
        for dr in 0..side {
            let y = (row + dr).wrapping_sub(radius);
            for dc in 0..side {
                let x = (col + dc).wrapping_sub(radius);
                let v = match c {
                    Col::One(v) => *v,
                    Col::Cells(m) if y < h && x < w => m[y * w + x],
                    Col::Cells(_) => f64::NAN,
                };
                out.next().unwrap().copy_from_slice(&v.to_le_bytes());
            }
        }
    }
}

/// The `window` interface: `function(ptr, side, n_inputs, row, col) -> f64` for every cell, in chunks of cells.
fn run_window(
    engine: &wasmi::Engine,
    module: &wasmi::Module,
    function: &str,
    grid: &terraflux_raster::Grid,
    cols: &[Col],
    radius: usize,
    fuel: u64,
) -> Result<Vec<f64>> {
    let side = 2 * radius + 1;
    let block = side * side * 8 * cols.len();
    if block > MAX_WINDOW {
        return Err(Error::msg(format!(
            "{} windows of {side} x {side} cells are {} MiB at each call: use a smaller radius or fewer inputs",
            cols.len(),
            block >> 20
        )));
    }
    let n = grid.cells();
    let chunk = 1 << 14;
    let parts: Vec<Vec<f64>> = (0..n.div_ceil(chunk))
        .into_par_iter()
        .map(|c| -> Result<Vec<f64>> {
            let (lo, hi) = (c * chunk, ((c + 1) * chunk).min(n));
            // (the budget of a cell grows with its window; a cell's call at least the allocation)
            let budget = fuel.saturating_mul((side * side) as u64).saturating_mul((hi - lo) as u64);
            let mut g = Guest::new(
                engine,
                module,
                function,
                &[ValType::I32; 5],
                ValType::F64,
                budget.saturating_add(FUEL_PER_CELL),
                "window",
                "",
            )?;
            let buf = g.alloc(block)?;
            let mut result = [Val::F64(0.0.into())];
            let mut out = Vec::with_capacity(hi - lo);
            for i in lo..hi {
                let (row, col) = (i / grid.width, i % grid.width);
                fill_window(g.bytes(buf, block)?, cols, grid, row, col, radius);
                g.call(&[ptr(buf), side as i32, cols.len() as i32, row as i32, col as i32], &mut result)
                    .map_err(|e| Error::msg(format!("the plugin's {function:?} at cell {i}: {}", explain(e))))?;
                out.push(match result[0] {
                    Val::F64(x) => f64::from(x),
                    _ => f64::NAN,
                });
            }
            Ok(out)
        })
        .collect::<Result<_>>()?;
    Ok(parts.concat())
}

/// What the `map` interface gives a module, and what it asks back.
struct MapJob<'a> {
    rows: usize,
    cols: usize,
    ins: &'a [Col],
    tables: &'a [&'a Table],
    /// The names of the output maps.
    maps: &'a [String],
    /// The output tables: name and columns.
    table_outs: &'a [(String, Vec<String>)],
}

/// What a `map` call gave: the maps and the tables, in the order of the job's names.
struct MapResult {
    maps: Vec<Vec<f64>>,
    tables: Vec<Table>,
}

/// The `map` interface: `function(rows, cols, n_in, inputs, n_out, outputs[, n_in_cols, in_cols, n_out_cols,
/// out_cols]) -> i32` called once.
fn run_map(
    engine: &wasmi::Engine,
    module: &wasmi::Module,
    function: &str,
    job: &MapJob,
    fuel: u64,
) -> Result<MapResult> {
    let cells = job.rows * job.cols;
    let map_bytes = cells * 8;
    let tables = !job.tables.is_empty() || !job.table_outs.is_empty();
    let table_bytes: usize = job.tables.iter().map(|t| t.rows.len() * t.columns.len() * 8).sum();
    let total = ((job.ins.len() + job.maps.len()) * map_bytes + table_bytes) as u64;
    if total > MAX_MEMORY {
        return Err(Error::msg(format!(
            "the maps and tables of the map interface take {:.1} GB, more than a module's 32-bit memory holds ({:.1} GB \
             at most): use the \"cell\" or \"window\" interface for maps this large",
            total as f64 / 1e9,
            MAX_MEMORY as f64 / 1e9
        )));
    }
    let arity = if tables { 10 } else { 6 };
    let help = if tables {
        " (the tables add n_in_cols, in_cols, n_out_cols and out_cols)"
    } else {
        " (the error code; 0 is ok)"
    };
    let budget = fuel.saturating_mul(cells.max(1) as u64).saturating_add(FUEL_PER_CELL);
    let mut g = Guest::new(engine, module, function, &[ValType::I32; 10][..arity], ValType::I32, budget, "map", help)?;
    // the input maps
    let mut in_ptrs = Vec::new();
    for c in job.ins {
        let p = g.alloc(map_bytes)?;
        for (k, d) in g.bytes(p, map_bytes)?.as_chunks_mut::<8>().0.iter_mut().enumerate() {
            d.copy_from_slice(&c.at(k).to_le_bytes());
        }
        in_ptrs.push(ptr(p));
    }
    // the output maps, NaN
    let mut out_ptrs = Vec::new();
    for _ in job.maps {
        let p = g.alloc(map_bytes)?;
        for d in g.bytes(p, map_bytes)?.as_chunks_mut::<8>().0.iter_mut() {
            d.copy_from_slice(&f64::NAN.to_le_bytes());
        }
        out_ptrs.push(ptr(p));
    }
    let ins = g.alloc(in_ptrs.len() * 4)?;
    g.put_i32s(ins, &in_ptrs)?;
    let outs = g.alloc(out_ptrs.len() * 4)?;
    g.put_i32s(outs, &out_ptrs)?;
    let mut args =
        vec![job.rows as i32, job.cols as i32, in_ptrs.len() as i32, ptr(ins), out_ptrs.len() as i32, ptr(outs)];
    // the columns of the input tables (their data, their names), and the descriptors of the output tables' (the
    // module fills `data` and `n_rows`)
    fn text(g: &mut Guest, s: &str) -> Result<i32> {
        let p = g.alloc(s.len())?;
        g.bytes(p, s.len())?.copy_from_slice(s.as_bytes());
        Ok(ptr(p))
    }
    let mut table_out_at = (0, 0);
    if tables {
        let mut cols_in: Vec<i32> = Vec::new();
        for (ti, t) in job.tables.iter().enumerate() {
            let n = t.rows.len();
            for (j, name) in t.columns.iter().enumerate() {
                let data = g.alloc(n * 8)?;
                for (r, d) in t.rows.iter().zip(g.bytes(data, n * 8)?.as_chunks_mut::<8>().0.iter_mut()) {
                    d.copy_from_slice(&r.get(j).copied().unwrap_or(f64::NAN).to_le_bytes());
                }
                let np = text(&mut g, name)?;
                cols_in.extend([ti as i32, ptr(data), n as i32, np, name.len() as i32]);
            }
        }
        let mut cols_out: Vec<i32> = Vec::new();
        for (ti, (_, columns)) in job.table_outs.iter().enumerate() {
            for name in columns {
                let np = text(&mut g, name)?;
                cols_out.extend([ti as i32, 0, 0, np, name.len() as i32]);
            }
        }
        let a = g.alloc(cols_in.len() * 4)?;
        g.put_i32s(a, &cols_in)?;
        let b = g.alloc(cols_out.len() * 4)?;
        g.put_i32s(b, &cols_out)?;
        table_out_at = (b, cols_out.len() / 5);
        args.extend([(cols_in.len() / 5) as i32, ptr(a), table_out_at.1 as i32, ptr(b)]);
    }
    let mut result = [Val::I32(0)];
    g.call(&args, &mut result).map_err(|e| Error::msg(format!("the plugin's {function:?}: {}", explain(e))))?;
    match result[0] {
        Val::I32(0) => {}
        Val::I32(code) => {
            return Err(Error::msg(format!("the plugin's {function:?} returned the error code {code} (0 is ok)")));
        }
        _ => return Err(Error::msg(format!("the plugin's {function:?} gave no code"))),
    }
    let maps = out_ptrs.iter().map(|p| g.f64s(*p as u32 as usize, cells)).collect::<Result<Vec<_>>>()?;
    // the output tables: the descriptors the module filled, one column after the other
    let desc = g.i32s(table_out_at.0, table_out_at.1 * 5)?;
    let mut tables_out = Vec::new();
    let mut d = desc.as_chunks::<5>().0.iter();
    for (name, columns) in job.table_outs {
        let mut data: Vec<Vec<f64>> = Vec::new();
        let mut n_rows: Option<usize> = None;
        for column in columns {
            let c = d.next().unwrap();
            let (at, n) = (c[1] as u32 as usize, c[2] as u32 as usize);
            if n_rows.is_some_and(|m| m != n) {
                return Err(Error::msg(format!(
                    "the output table {name:?}: its columns have different numbers of rows ({} and {n} at {column:?})",
                    n_rows.unwrap_or(0)
                )));
            }
            n_rows = Some(n);
            data.push(
                g.f64s(at, n).map_err(|e| Error::msg(format!("the output table {name:?}, {column:?}: {}", e.0)))?,
            );
        }
        let n = n_rows.unwrap_or(0);
        let mut t = Table::new(columns.clone());
        t.rows = (0..n).map(|r| data.iter().map(|c| c[r]).collect()).collect();
        tables_out.push(t);
    }
    Ok(MapResult { maps, tables: tables_out })
}

/// An argument: a map's cells, or one number for every cell.
enum Col {
    Cells(Arc<Vec<f64>>),
    One(f64),
}

impl Col {
    fn at(&self, i: usize) -> f64 {
        match self {
            Col::Cells(m) => m[i],
            Col::One(x) => *x,
        }
    }
}

/// `plugin.wasm`.
pub struct Wasm;

impl Operator for Wasm {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            PLUGIN,
            "A map computed by a WebAssembly function (a plugin written in any language), in a sandbox: cell by \
             cell (its arguments are the node's inputs, maps or numbers, in the order of `args` or by name, as f64; \
             its result the cell's value), from the window around each cell (`window`), or from the whole maps and \
             tables at once, with several outputs (`map`).",
            Access::Local,
        )
        .advice(
            "The interface `cell` calls the function once per cell; `window` gives it the (2r+1) x (2r+1) cells \
             around each (the module exports `memory` and `alloc`); `map` calls it once with every input and \
             output as an array in its memory, and takes tables too. The module imports nothing and every call has \
             a budget of instructions (`fuel` per cell).",
        )
        .extra(Type::Any)
        .param(PortSpec::new("path", Type::Text, "the module (.wasm or .wat), relative to the model's folder"))
        .param(
            PortSpec::optional(
                "interface",
                Type::Text,
                "how the module is called: `cell` (the default), `window` or `map`",
            )
            .choices(["cell", "window", "map"]),
        )
        .param(PortSpec::optional(
            "function",
            Type::Text,
            "the function called (default: the interface's name: cell, window or map)",
        ))
        .param(PortSpec::optional("args", Type::Any, "the inputs in the order of the function's parameters"))
        .param(PortSpec::optional("radius", Type::Integer, "window: the radius r of the window, 1 to 255 cells"))
        .param(PortSpec::optional("outputs", Type::Any, "map: the names of the output maps, a list (default: map)"))
        .param(PortSpec::optional(
            "table_outputs",
            Type::Any,
            "map: the output tables, `{name = [columns]}`, filled by the module",
        ))
        .param(PortSpec::optional(
            "fuel",
            Type::Integer,
            "the instructions a call may run per cell (default 1000000, up to 100000000)",
        ))
        .output(PortSpec::new("map", Type::Map, "the map computed"))
        .outputs_listed("outputs", Type::Map)
        .outputs_listed("table_outputs", Type::Table)
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let at = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        if let Some((k, why)) = plugin_problems(args.params).first() {
            return Err(at(format!("the setting {k}: {why}")));
        }
        let path: String = args.opt_param("path")?.ok_or_else(|| at("no path to the plugin".into()))?;
        let interface: String = args.param("interface", "cell".to_string())?;
        let function: String = args.param("function", interface.clone())?;
        let fuel: u64 = args.param("fuel", FUEL_PER_CELL)?;
        let names: Vec<String> = match args.opt_param::<Vec<String>>("args")? {
            Some(n) => n,
            None => args.inputs.keys().cloned().collect(),
        };
        let mut grid = None;
        let mut cols: Vec<Col> = Vec::new();
        let mut tables: Vec<&Table> = Vec::new();
        for name in &names {
            match args.get(name).ok_or_else(|| at(format!("no input {name:?}")))? {
                Value::Map(m) => {
                    if let Some(g) = &grid
                        && g != &m.grid
                    {
                        return Err(at(format!("{name} is not on the grid of the other maps")));
                    }
                    grid = Some(m.grid.clone());
                    cols.push(Col::Cells(m.data.clone()));
                }
                Value::Table(t) if interface == "map" => tables.push(t),
                v => {
                    let what = if interface == "map" { "a map, a table nor a number" } else { "a map nor a number" };
                    cols.push(Col::One(v.as_number().ok_or_else(|| at(format!("{name} is neither {what}")))?))
                }
            }
        }
        let (engine, module) = compile(&path).map_err(|e| at(e.0))?;
        if interface == "map" {
            let table_outs: BTreeMap<String, Vec<String>> = args.param("table_outputs", BTreeMap::new())?;
            let table_outs: Vec<(String, Vec<String>)> = table_outs.into_iter().collect();
            let maps: Vec<String> = match args.opt_param("outputs")? {
                Some(m) => m,
                None if table_outs.is_empty() => vec!["map".to_string()],
                None => Vec::new(),
            };
            if grid.is_none() && !maps.is_empty() {
                return Err(at("a plugin that gives maps needs a map among its inputs".into()));
            }
            let (rows, cols_n) = grid.as_ref().map_or((0, 0), |g| (g.height, g.width));
            let job = MapJob { rows, cols: cols_n, ins: &cols, tables: &tables, maps: &maps, table_outs: &table_outs };
            let done = run_map(&engine, &module, &function, &job, fuel).map_err(|e| at(e.0))?;
            let mut out = Outputs::new();
            for (name, data) in maps.iter().zip(done.maps) {
                let grid = grid.clone().expect("a grid when there are maps");
                out.insert(name.clone(), Value::Map(Map::new(grid, data)));
            }
            for ((name, _), t) in table_outs.iter().zip(done.tables) {
                out.insert(name.clone(), Value::Table(t));
            }
            return Ok(out);
        }
        let grid = grid.ok_or_else(|| at("a plugin needs a map among its inputs".into()))?;
        let cells = if interface == "window" {
            let radius: usize = args.param("radius", 0)?;
            run_window(&engine, &module, &function, &grid, &cols, radius, fuel).map_err(|e| at(e.0))?
        } else {
            let fill = |i: usize, out: &mut Vec<f64>| out.extend(cols.iter().map(|c| c.at(i)));
            run_cells(&engine, &module, &function, grid.cells(), cols.len(), fuel, &fill).map_err(|e| at(e.0))?
        };
        Ok(outputs([("map", Value::Map(Map::new(grid, cells)))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Wasm));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn write(name: &str, text: &str) -> String {
        let p = std::env::temp_dir().join(format!("terraflux-plugin-{}-{name}", std::process::id()));
        std::fs::write(&p, text).unwrap();
        p.to_string_lossy().into_owned()
    }

    fn run(path: &str, params: serde_json::Value) -> Result<Vec<f64>> {
        let g = Arc::new(terraflux_raster::Grid::new(3, 1));
        let inputs: BTreeMap<String, Value> = [
            ("land".to_string(), Value::Map(Map::new(g.clone(), vec![1.0, 2.0, f64::NAN]))),
            ("slope".to_string(), Value::Map(Map::new(g, vec![5.0, 20.0, 5.0]))),
            ("k".to_string(), Value::Number(10.0)),
        ]
        .into();
        let mut p = params.as_object().unwrap().clone();
        p.insert("path".into(), path.into());
        let a = Args { node: "p", inputs, params: &p };
        let out = Wasm.run(&mut Ctx::default(), &a)?;
        match &out["map"] {
            Value::Map(m) => Ok(m.data.to_vec()),
            _ => unreachable!(),
        }
    }

    #[test]
    fn a_function_per_cell() {
        let p = write(
            "keep.wat",
            r#"(module (func (export "keep") (param $land f64) (param $slope f64) (param $k f64) (result f64)
                 (if (result f64) (f64.lt (local.get $slope) (local.get $k))
                   (then (local.get $land)) (else (f64.const 0)))))"#,
        );
        let v = run(&p, serde_json::json!({"function": "keep", "args": ["land", "slope", "k"]})).unwrap();
        assert_eq!(v[..2], [1.0, 0.0]);
        assert!(v[2].is_nan());
    }

    #[test]
    fn plugins_are_sandboxed_and_checked() {
        // a loop forever runs out of fuel
        let p = write(
            "forever.wat",
            r#"(module (func (export "cell") (param f64) (result f64) (loop $l (br $l)) (f64.const 0)))"#,
        );
        let e = run(&p, serde_json::json!({"args": ["land"]})).err().unwrap().0;
        assert!(e.contains("fuel") || e.contains("cell 0"), "{e}");
        // the arity must match
        let p = write("one.wat", r#"(module (func (export "cell") (param f64) (result f64) (local.get 0)))"#);
        let e = run(&p, serde_json::json!({})).err().unwrap().0;
        assert!(e.contains("must take 3 f64"), "{e}");
        // no imports: the module cannot reach the host
        let p = write(
            "import.wat",
            r#"(module (import "env" "open" (func $open (result f64))) (func (export "cell") (param f64) (result f64) (call $open)))"#,
        );
        let e = run(&p, serde_json::json!({"args": ["land"]})).err().unwrap().0;
        assert!(e.contains("import"), "{e}");
        assert!(run("nowhere.wasm", serde_json::json!({})).is_err());
    }

    /// A module with a memory and a bump allocator, and `body` after them.
    fn module(body: &str) -> String {
        format!(
            r#"(module
              (memory (export "memory") 2)
              (global $top (mut i32) (i32.const 8))
              (func $alloc (export "alloc") (param $n i32) (result i32) (local $p i32)
                (local.set $p (global.get $top))
                (global.set $top (i32.add (local.get $p) (local.get $n)))
                (local.get $p))
              (func $at (param $base i32) (param $i i32) (result i32)
                (i32.add (local.get $base) (i32.shl (local.get $i) (i32.const 3))))
              {body})"#
        )
    }

    fn call(path: &str, params: serde_json::Value, inputs: Vec<(&str, Value)>) -> Result<Outputs> {
        let inputs: BTreeMap<String, Value> = inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let mut p = params.as_object().unwrap().clone();
        p.insert("path".into(), path.into());
        Wasm.run(&mut Ctx::default(), &Args { node: "p", inputs, params: &p })
    }

    fn map_of(w: usize, h: usize, cells: Vec<f64>) -> Value {
        Value::Map(Map::new(Arc::new(terraflux_raster::Grid::new(w, h)), cells))
    }

    fn cells(v: &Value) -> Vec<f64> {
        match v {
            Value::Map(m) => m.data.to_vec(),
            _ => panic!("not a map"),
        }
    }

    /// The mean of the 3 x 3 cells around (r, c) that are in the map and not NaN.
    fn mean3(v: &[f64], w: usize, h: usize, r: usize, c: usize) -> f64 {
        let (mut s, mut n) = (0.0, 0);
        for y in r.saturating_sub(1)..(r + 2).min(h) {
            for x in c.saturating_sub(1)..(c + 2).min(w) {
                if !v[y * w + x].is_nan() {
                    s += v[y * w + x];
                    n += 1;
                }
            }
        }
        s / n as f64
    }

    const WINDOW_MEAN: &str = r#"
      (func (export "window") (param $ptr i32) (param $side i32) (param $n i32) (param $row i32) (param $col i32)
            (result f64)
        (local $i i32) (local $cnt i32) (local $sum f64) (local $x f64)
        (block $done (loop $next
          (br_if $done (i32.ge_u (local.get $i) (i32.mul (local.get $side) (local.get $side))))
          (local.set $x (f64.load (call $at (local.get $ptr) (local.get $i))))
          (if (f64.eq (local.get $x) (local.get $x)) (then
            (local.set $sum (f64.add (local.get $sum) (local.get $x)))
            (local.set $cnt (i32.add (local.get $cnt) (i32.const 1)))))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (br $next)))
        (f64.div (local.get $sum) (f64.convert_i32_s (local.get $cnt))))
      ;; the cell's own coordinates and the number of inputs, and the centre of the second input's window
      (func (export "where") (param $ptr i32) (param $side i32) (param $n i32) (param $row i32) (param $col i32)
            (result f64)
        (f64.add
          (f64.add (f64.mul (f64.convert_i32_s (local.get $row)) (f64.const 1000))
                   (f64.mul (f64.convert_i32_s (local.get $col)) (f64.const 10)))
          (f64.add (f64.convert_i32_s (local.get $n))
            (f64.load (call $at (local.get $ptr)
              (i32.add (i32.mul (local.get $side) (local.get $side))
                       (i32.div_u (i32.mul (local.get $side) (local.get $side)) (i32.const 2))))))))"#;

    #[test]
    fn a_window_of_neighbours_with_edges_and_missing_cells() {
        let (w, h) = (5, 4);
        let mut v: Vec<f64> = (0..w * h).map(|i| (i * i % 7) as f64).collect();
        v[7] = f64::NAN;
        v[0] = f64::NAN;
        let p = write("window.wat", &module(WINDOW_MEAN));
        let given = || vec![("a", map_of(w, h, v.clone()))];
        let out = call(&p, serde_json::json!({"interface": "window", "radius": 1, "args": ["a"]}), given()).unwrap();
        let got = cells(&out["map"]);
        for r in 0..h {
            for c in 0..w {
                let (a, b) = (got[r * w + c], mean3(&v, w, h, r, c));
                assert!(a == b || (a.is_nan() && b.is_nan()), "({r}, {c}): {a} against {b}");
            }
        }
        // the radius is checked, and a window needs the module's memory and alloc
        let e = call(&p, serde_json::json!({"interface": "window", "radius": 0}), given()).err().unwrap().0;
        assert!(e.contains("radius"), "{e}");
        let e = call(&p, serde_json::json!({"interface": "window"}), given()).err().unwrap().0;
        assert!(e.contains("radius"), "{e}");
        // the cell's row and column, the inputs and a number filling its window (radius 2: 25 cells)
        let inputs = vec![("a", map_of(w, h, v.clone())), ("k", Value::Number(0.5))];
        let params = serde_json::json!({"interface": "window", "radius": 2, "function": "where", "args": ["a", "k"]});
        let got = cells(&call(&p, params, inputs).unwrap()["map"]);
        assert_eq!(got[2 * w + 3], 2000.0 + 30.0 + 2.0 + 0.5);
        assert_eq!(got[0], 2.5);
    }

    #[test]
    fn a_map_with_two_outputs() {
        let both = module(
            r#"(func (export "map") (param $rows i32) (param $cols i32) (param $nin i32) (param $in i32)
                     (param $nout i32) (param $out i32) (result i32)
                 (local $i i32) (local $n i32) (local $a i32) (local $b i32) (local $s i32) (local $d i32)
                 (local.set $a (i32.load (local.get $in)))
                 (local.set $b (i32.load offset=4 (local.get $in)))
                 (local.set $s (i32.load (local.get $out)))
                 (local.set $d (i32.load offset=4 (local.get $out)))
                 (local.set $n (i32.mul (local.get $rows) (local.get $cols)))
                 (block $done (loop $next
                   (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
                   (f64.store (call $at (local.get $s) (local.get $i))
                     (f64.add (f64.load (call $at (local.get $a) (local.get $i)))
                              (f64.load (call $at (local.get $b) (local.get $i)))))
                   (f64.store (call $at (local.get $d) (local.get $i))
                     (f64.sub (f64.load (call $at (local.get $a) (local.get $i)))
                              (f64.load (call $at (local.get $b) (local.get $i)))))
                   (local.set $i (i32.add (local.get $i) (i32.const 1)))
                   (br $next)))
                 (i32.const 0))"#,
        );
        let p = write("both.wat", &both);
        let given =
            || vec![("land", map_of(3, 1, vec![1.0, 2.0, f64::NAN])), ("slope", map_of(3, 1, vec![5.0, 20.0, 5.0]))];
        let params = serde_json::json!({"interface": "map", "outputs": ["sum", "diff"], "args": ["land", "slope"]});
        let out = call(&p, params, given()).unwrap();
        assert_eq!(out.keys().collect::<Vec<_>>(), ["diff", "sum"]);
        let (s, d) = (cells(&out["sum"]), cells(&out["diff"]));
        assert_eq!(s[..2], [6.0, 22.0]);
        assert_eq!(d[..2], [-4.0, -18.0]);
        assert!(s[2].is_nan() && d[2].is_nan());
        // a number is an array filled with it
        let given = vec![("land", map_of(3, 1, vec![1.0, 2.0, 3.0])), ("slope", Value::Number(10.0))];
        let params = serde_json::json!({"interface": "map", "outputs": ["a", "b"], "args": ["land", "slope"]});
        assert_eq!(cells(&call(&p, params, given).unwrap()["a"]), [11.0, 12.0, 13.0]);
    }

    #[test]
    fn a_map_that_fails_says_its_code() {
        let p = write(
            "fail.wat",
            &module(r#"(func (export "map") (param i32 i32 i32 i32 i32 i32) (result i32) (i32.const 7))"#),
        );
        let given = vec![("a", map_of(2, 1, vec![1.0, 2.0]))];
        let e = call(&p, serde_json::json!({"interface": "map"}), given).err().unwrap().0;
        assert!(e.contains("error code 7"), "{e}");
        // a function that does not fit the interface says what it takes
        let p = write("shape.wat", &module(r#"(func (export "map") (param f64) (result i32) (i32.const 0))"#));
        let given = vec![("a", map_of(2, 1, vec![1.0, 2.0]))];
        let e = call(&p, serde_json::json!({"interface": "map"}), given).err().unwrap().0;
        assert!(e.contains("must take 6 i32"), "{e}");
    }

    #[test]
    fn the_fuel_stops_a_map_and_a_window() {
        let given = || vec![("a", map_of(3, 1, vec![1.0, 2.0, 3.0]))];
        let p = write(
            "forever-map.wat",
            &module(
                r#"(func (export "map") (param i32 i32 i32 i32 i32 i32) (result i32) (loop $l (br $l)) (i32.const 0))"#,
            ),
        );
        let e = call(&p, serde_json::json!({"interface": "map", "fuel": 1000}), given()).err().unwrap().0;
        assert!(e.contains("fuel"), "{e}");
        let p = write(
            "forever-window.wat",
            &module(
                r#"(func (export "window") (param i32 i32 i32 i32 i32) (result f64) (loop $l (br $l)) (f64.const 0))"#,
            ),
        );
        let e =
            call(&p, serde_json::json!({"interface": "window", "radius": 1, "fuel": 1000}), given()).err().unwrap().0;
        assert!(e.contains("fuel"), "{e}");
    }

    #[test]
    fn a_module_without_memory_or_alloc_is_told_so() {
        let given = || vec![("a", map_of(2, 1, vec![1.0, 2.0]))];
        let map = r#"(func (export "map") (param i32 i32 i32 i32 i32 i32) (result i32) (i32.const 0))"#;
        let p = write("nomem.wat", &format!("(module {map})"));
        let e = call(&p, serde_json::json!({"interface": "map"}), given()).err().unwrap().0;
        assert!(e.contains("exports no memory") && e.contains("alloc"), "{e}");
        let p = write("noalloc.wat", &format!(r#"(module (memory (export "memory") 1) {map})"#));
        let e = call(&p, serde_json::json!({"interface": "map"}), given()).err().unwrap().0;
        assert!(e.contains("no function \"alloc\""), "{e}");
        let window = r#"(func (export "window") (param i32 i32 i32 i32 i32) (result f64) (f64.const 0))"#;
        let p = write("nowin.wat", &format!("(module {window})"));
        let e = call(&p, serde_json::json!({"interface": "window", "radius": 1}), given()).err().unwrap().0;
        assert!(e.contains("exports no memory"), "{e}");
        // an interface the model does not have
        let e = call(&p, serde_json::json!({"interface": "tiles"}), given()).err().unwrap().0;
        assert!(e.contains("interface"), "{e}");
    }

    #[test]
    fn a_table_goes_in_as_columns_and_comes_out_as_columns() {
        // the sum of y on every cell of `s`, and a table `dbl` with the column x2 = 2 x
        let body = r#"(func (export "map") (param $rows i32) (param $cols i32) (param $nin i32) (param $in i32)
                 (param $nout i32) (param $out i32) (param $nci i32) (param $ci i32) (param $nco i32) (param $co i32)
                 (result i32)
               (local $n i32) (local $i i32) (local $x i32) (local $y i32) (local $s i32) (local $m i32)
               (local $sum f64)
               ;; the second column's name is "y" (its first byte is 121), and there are two columns
               (if (i32.ne (local.get $nci) (i32.const 2)) (then (return (i32.const 8))))
               (if (i32.ne (i32.load8_u (i32.load offset=32 (local.get $ci))) (i32.const 121))
                 (then (return (i32.const 9))))
               (local.set $x (i32.load offset=4 (local.get $ci)))
               (local.set $y (i32.load offset=24 (local.get $ci)))
               (local.set $n (i32.load offset=8 (local.get $ci)))
               (block $d1 (loop $l1
                 (br_if $d1 (i32.ge_u (local.get $i) (local.get $n)))
                 (local.set $sum (f64.add (local.get $sum) (f64.load (call $at (local.get $y) (local.get $i)))))
                 (local.set $i (i32.add (local.get $i) (i32.const 1)))
                 (br $l1)))
               (local.set $s (i32.load (local.get $out)))
               (local.set $i (i32.const 0))
               (block $d2 (loop $l2
                 (br_if $d2 (i32.ge_u (local.get $i) (i32.mul (local.get $rows) (local.get $cols))))
                 (f64.store (call $at (local.get $s) (local.get $i)) (local.get $sum))
                 (local.set $i (i32.add (local.get $i) (i32.const 1)))
                 (br $l2)))
               (local.set $m (call $alloc (i32.shl (local.get $n) (i32.const 3))))
               (local.set $i (i32.const 0))
               (block $d3 (loop $l3
                 (br_if $d3 (i32.ge_u (local.get $i) (local.get $n)))
                 (f64.store (call $at (local.get $m) (local.get $i))
                   (f64.mul (f64.load (call $at (local.get $x) (local.get $i))) (f64.const 2)))
                 (local.set $i (i32.add (local.get $i) (i32.const 1)))
                 (br $l3)))
               (i32.store offset=4 (local.get $co) (local.get $m))
               (i32.store offset=8 (local.get $co) (local.get $n))
               (i32.const 0))"#;
        let p = write("table.wat", &module(body));
        let mut t = Table::new(vec!["x".into(), "y".into()]);
        t.rows = vec![vec![1.0, 10.0], vec![2.0, 20.0], vec![3.0, 30.0]];
        let given = || vec![("land", map_of(2, 1, vec![1.0, 2.0])), ("t", Value::Table(t.clone()))];
        let params = serde_json::json!({"interface": "map", "args": ["land", "t"], "outputs": ["s"],
            "table_outputs": {"dbl": ["x2"]}});
        let out = call(&p, params, given()).unwrap();
        assert_eq!(cells(&out["s"]), [60.0, 60.0]);
        match &out["dbl"] {
            Value::Table(t) => {
                assert_eq!(t.columns, ["x2"]);
                assert_eq!(t.rows, [[2.0], [4.0], [6.0]]);
            }
            _ => panic!("not a table"),
        }
        // the cell interface takes no table, and a table asks for the function with the tables' arguments
        let e = call(&p, serde_json::json!({"args": ["land", "t"]}), given()).err().unwrap().0;
        assert!(e.contains("neither a map nor a number"), "{e}");
        let six = write(
            "six.wat",
            &module(r#"(func (export "map") (param i32 i32 i32 i32 i32 i32) (result i32) (i32.const 0))"#),
        );
        let e = call(&six, serde_json::json!({"interface": "map", "args": ["land", "t"]}), given()).err().unwrap().0;
        assert!(e.contains("must take 10 i32"), "{e}");
    }

    #[test]
    fn the_modules_of_the_tutorial_run() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../website/figures/building-models/plugins/");
        let (w, h) = (4, 3);
        let v: Vec<f64> = (0..w * h).map(|i| (i * 5 % 11) as f64).collect();
        let params = serde_json::json!({"interface": "window", "radius": 1});
        let out = call(&format!("{dir}window_mean.wat"), params, vec![("a", map_of(w, h, v.clone()))]).unwrap();
        let got = cells(&out["map"]);
        assert!((0..w * h).all(|i| (got[i] - mean3(&v, w, h, i / w, i % w)).abs() < 1e-12), "{got:?}");
        let given = vec![("a", map_of(w, h, v.clone())), ("b", Value::Number(1.0))];
        let params = serde_json::json!({"interface": "map", "args": ["a", "b"], "outputs": ["sum", "diff"]});
        let out = call(&format!("{dir}sum_diff.wat"), params, given).unwrap();
        assert_eq!(cells(&out["sum"]), v.iter().map(|x| x + 1.0).collect::<Vec<_>>());
        assert_eq!(cells(&out["diff"]), v.iter().map(|x| x - 1.0).collect::<Vec<_>>());
    }

    #[test]
    fn maps_too_large_for_32_bits_are_refused_with_a_way_out() {
        let p = write(
            "big.wat",
            &module(r#"(func (export "map") (param i32 i32 i32 i32 i32 i32) (result i32) (i32.const 0))"#),
        );
        let (engine, m) = compile(&p).unwrap();
        let (maps, ins) = (["a".to_string()], [Col::One(1.0)]);
        let job = MapJob { rows: 40_000, cols: 40_000, ins: &ins, tables: &[], maps: &maps, table_outs: &[] };
        let e = run_map(&engine, &m, "map", &job, FUEL_PER_CELL).err().unwrap().0;
        assert!(e.contains("32-bit") && e.contains("\"cell\" or \"window\""), "{e}");
    }
}
