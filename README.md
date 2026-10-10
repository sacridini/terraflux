# Terraflux

**Fast, scalable land use and land cover change modelling — from a laptop to the whole planet at 30 m.**

Terraflux is a native application for land change simulation with cellular automata, in the spirit of
Dinamica EGO: a visual model builder, guided data preparation and live maps, with an engine that runs
the same model on one machine or distributed across many headless workers.

> **Status:** planning. There is no code yet; the design is in [`docs/PLAN.md`](docs/PLAN.md) (Portuguese).

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

## License

GPL-3.0-or-later.
