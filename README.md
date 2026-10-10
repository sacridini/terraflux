# Terraflux

**Fast, scalable land use and land cover change modelling — from a laptop to the whole planet at 30 m.**

Terraflux is a native application for land change simulation with cellular automata, in the spirit of
Dinamica EGO: a visual model builder, guided data preparation and live maps, with an engine that runs
the same model on one machine or distributed across many headless workers.

> **Status:** 0.1 — the model graph, the exact engine and the command line run Dinamica EGO's classic model,
> CLUE-S and the ordered allocation, bit for bit as landschaft's `ls.simulate` (and so as lulcc). The
> application, Python bridge and distributed engine come next; the plan is in [`docs/PLAN.md`](docs/PLAN.md)
> (Portuguese) and the changes in [`CHANGELOG.md`](CHANGELOG.md).

## Goals

- **One model, many front-ends.** A model is a typed graph of operators saved as versionable text. The
  guided wizard, the node editor, Python and the text file all read and write the same graph.
- **Easy without hiding anything.** Every automatic choice of the wizard becomes an explicit node in the
  graph, with its reason, so beginners accept sensible defaults and experts change anything.
- **Guided data preparation.** A reference grid per project; automatic harmonisation with the right
  resampling for each kind of data; class legends; observed changes; explanatory variables in one click;
  checks that catch problems before a run.
- **Any scale.** An exact mode, bit for bit with the reference tools on one machine, and a scalable mode
  over tiles: deterministic whatever the partitioning, up to global scenarios at 30 m.
- **Flexible logic.** Map algebra, reusable submodels and pluggable operators (Rust and WASM).
- **Agents later.** An agent-based layer on top of the cellular automata.

## Technology

- **Application:** Rust, with an [egui](https://github.com/emilk/egui) + wgpu interface.
- **Simulation kernels:** C++17/OpenMP from [landschaft](https://github.com/sacridini/landschaft), which
  reproduces lulcc, CLUMondo, r.futures, MOLUSCE, OpenLand and diffeR, plus Dinamica EGO's and PLUS's
  methods from their publications.
- **Distribution:** gRPC between a coordinator and workers, with Zarr and COG on shared storage.
- **Python:** through landschaft's `ls.simulate`.

## Quick start

```bash
# a model from Dinamica EGO's classic one, fitted on two maps (14 steps between them)
terraflux new dinamica --t0 landuse.tif:1997 --t1 landuse.tif:2011 --drivers drivers.tif --steps 14 -o model.toml
terraflux check model.toml          # every operator, reference and connection checked
terraflux run model.toml            # writes output/maps.tif (a band per step), steps.csv, weights.csv
terraflux run model.toml --set seed=7 --out other
terraflux ops                       # the operators and what they do
```

`terraflux new clue` and `terraflux new ordered` make lulcc's CLUE-S and ordered models (logistic regressions,
a demand interpolated from observed maps, conversion rules, elasticities, neighbourhoods, restrictions).

## The model file

A model is a graph of operators in TOML (or JSON): its inputs, its nodes and its outputs. Each node runs one
operator on inputs that come from the model's inputs (`$name`), from other nodes (`node.port`), from values
carried along a loop (`@name`, `@step`) or literal values; its `note` says why it is there. The models made by
`terraflux new` are written out node by node, so that anything they chose can be read and changed.

```toml
format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "landuse.tif"
band = "1997"

[[nodes]]
id = "markov"
op = "markov.fit"
note = "Transition rates per step: the 14th root of the matrix of the calibration period."
inputs = { t0 = "$t0", t1 = "$t1", steps = 14 }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 14, landscape = "$t0" }
[nodes.body]
carry = { landscape = "allocate.landscape" }
collect = { maps = "allocate.landscape" }
# [[nodes.body.nodes]] ... woe.probability, dinamica.step

[outputs.maps]
from = "simulation.maps"
```

## Building

Terraflux needs Rust (1.85 or later), CMake and a C++17 compiler (Visual Studio on Windows; on Windows the
CMake of the newest Visual Studio is used). Landschaft's C++ kernels come from the submodule:

```bash
git clone --recurse-submodules git@github.com:sacridini/terraflux.git
cd terraflux
cargo build --release          # target/release/terraflux
cargo test --release           # parity with landschaft included
```

On Windows keep the build folder's path short (`CARGO_TARGET_DIR`): MSBuild fails on paths over 260
characters. `LANDSCHAFT_DIR` points the build at another landschaft checkout, `LANDSCHAFT_EIGEN_DIR` at a local copy of
Eigen 3.4. `tools/make_reference.py` writes the reference outputs of the parity tests with landschaft.

| Crate | What it holds |
|---|---|
| `terraflux-graph` | the model graph: file format, references, signatures, checks |
| `terraflux-ops` | the operators (Markov chains, demands, logistic regression, weights of evidence, CLUE-S, ordered, Expander/Patcher) and the presets |
| `terraflux-exec` | the exact engine: inputs, graph run, outputs |
| `terraflux-raster` | grids, rasters, GeoTIFF reading and writing in pure Rust (BigTIFF, tiles, LZW, Deflate, predictors) |
| `terraflux-ls-sys` | the bridge to landschaft's C++ kernels |
| `terraflux-cli` | the `terraflux` command |

## License

GPL-3.0-or-later.
