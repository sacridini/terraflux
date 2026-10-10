//! Plugins: operators written in any language that compiles to WebAssembly (Rust, C, Zig, AssemblyScript...),
//! run in a sandbox (no files, no network, a budget of instructions), so a model can send them to any worker.
//!
//! `plugin.wasm` calls a function of a module (`params.function`, `cell` by default) for every cell: its
//! arguments are the node's inputs (in the order of `params.args`, else by name; maps give their cell's value,
//! numbers the same value everywhere), all `f64`, and it returns the cell's value (`f64`). Missing cells are
//! NaN, for the function to handle. The module may be binary (`.wasm`) or text (`.wat`).
//!
//! ```wat
//! (module (func (export "cell") (param $land f64) (param $slope f64) (result f64)
//!   (if (result f64) (f64.lt (local.get $slope) (f64.const 10))
//!     (then (local.get $land)) (else (f64.const 0)))))
//! ```

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PLUGIN, PortSpec, Type};

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value, outputs};

/// Instructions a call may run (a function that loops forever stops).
const FUEL_PER_CELL: u64 = 1_000_000;

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
            store.set_fuel(FUEL_PER_CELL * (hi - lo) as u64).map_err(|e| Error::msg(e.to_string()))?;
            for i in lo..hi {
                vals.clear();
                args(i, &mut vals);
                for (p, v) in params.iter_mut().zip(&vals) {
                    *p = wasmi::Val::F64((*v).into());
                }
                f.call(&mut store, &params, &mut result)
                    .map_err(|e| Error::msg(format!("the plugin's {function:?} at cell {i}: {e}")))?;
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

/// An argument: a map's cells, or one number for every cell.
enum Col {
    Cells(Arc<Vec<f64>>),
    One(f64),
}

/// `plugin.wasm`.
pub struct Wasm;

impl Operator for Wasm {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            PLUGIN,
            "A map computed cell by cell by a WebAssembly function (a plugin written in any language), in a \
             sandbox: its arguments are the node's inputs (maps or numbers, in the order of `args` or by name) as \
             f64, its result the cell's value.",
            Access::Local,
        )
        .extra(Type::Any)
        .param(PortSpec::new("path", Type::Text, "the module (.wasm or .wat), relative to the model's folder"))
        .param(PortSpec::optional("function", Type::Text, "the function called for every cell (default: cell)"))
        .param(PortSpec::optional("args", Type::Any, "the inputs in the order of the function's parameters"))
        .output(PortSpec::new("map", Type::Map, "the map computed"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let at = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let path: String = args.opt_param("path")?.ok_or_else(|| at("no path to the plugin".into()))?;
        let function: String = args.param("function", "cell".to_string())?;
        let names: Vec<String> = match args.opt_param::<Vec<String>>("args")? {
            Some(n) => n,
            None => args.inputs.keys().cloned().collect(),
        };
        let mut grid = None;
        let mut cols: Vec<Col> = Vec::new();
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
                v => cols
                    .push(Col::One(v.as_number().ok_or_else(|| at(format!("{name} is neither a map nor a number")))?)),
            }
        }
        let grid = grid.ok_or_else(|| at("a plugin needs a map among its inputs".into()))?;
        let (engine, module) = compile(&path).map_err(|e| at(e.0))?;
        let fill = |i: usize, out: &mut Vec<f64>| {
            for c in &cols {
                out.push(match c {
                    Col::Cells(m) => m[i],
                    Col::One(x) => *x,
                });
            }
        };
        let cells = run_cells(&engine, &module, &function, grid.cells(), cols.len(), &fill).map_err(|e| at(e.0))?;
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
}
