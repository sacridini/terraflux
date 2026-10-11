# Terraflux

**Fast, scalable land use and land cover change modelling — from a laptop to the whole planet at 30 m.**

Terraflux is a native application for land change simulation with cellular automata, in the spirit of
Dinamica EGO: a visual model builder, guided data preparation and live maps, with an engine that runs
the same model on one machine or distributed across many headless workers.

> **Status:** 0.7 — the application (a wizard that checks and fixes the data, live maps, a node editor,
> charts and validation), the exact engine, the command line and batches on many machines (replicates,
> scenarios, calibration) run Dinamica EGO's classic model, CLUE-S, the ordered allocation and PLUS's CARS, with
> dynamic variables, bit for bit as landschaft's `ls.simulate` (and so as lulcc); models fitted in Python run
> here unchanged. Map algebra, submodels and WebAssembly plugins change a model's logic; agents (macro agents,
> parcel managers, CRAFTY's competition for land) act on the map in the same graph. A tiled engine runs
> Dinamica EGO's model on maps larger than memory, on one machine or many: the same maps whatever the number of
> workers. The plan is in [`docs/PLAN.md`](docs/PLAN.md) (Portuguese) and the changes in
> [`CHANGELOG.md`](CHANGELOG.md).

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
- **Agents.** An agent-based layer on top of the cellular automata, in the same graph.

## Technology

- **Application:** Rust, with an [egui](https://github.com/emilk/egui) + wgpu interface.
- **Simulation kernels:** C++17/OpenMP from [landschaft](https://github.com/sacridini/landschaft), which
  reproduces lulcc, CLUMondo, r.futures, MOLUSCE, OpenLand and diffeR, plus Dinamica EGO's and PLUS's
  methods from their publications.
- **Distribution:** gRPC between a coordinator and workers, with Zarr and COG on shared storage.
- **Python:** through landschaft's `ls.simulate`.

## The application

```bash
terraflux app                 # or: terraflux app model.toml
```

A double click on `terraflux.exe` (or `terraflux` without a command) opens the application too, and so does a
model file dropped on it or opened with it; started that way on Windows, it shows no console.

- **Panes you arrange:** the model, the operators, the node, the viewer, the simulation's maps, the areas, the
  validation and the text are tabs to drag, split, close (the View menu turns each on or off) or take to a
  window of their own on another screen (right click on a tab); the layout is kept between sessions. A double
  click on a node opens its map in a tab of its own: one per node, brought to the front when it is open already
  (a new one once it is closed).
- **Seeing without writing:** `view.plot` shows a map, a stack, a loop's maps or rasters in a tab of its own,
  refreshed at every run (read, reclassify, plot: no file). What a tab or the viewer shows is saved from it
  (Save output: a file only, or a file and a `raster.write` in the model), in the background.
- **Basemaps:** under any map shown, satellite imagery or a map of the world (Esri's imagery and topographic
  map, OpenStreetMap, OpenTopoMap, Carto), warped by GDAL onto the data's CRS, kept in a cache; the data over it
  shown or hidden, at the opacity chosen.
- **Editing many nodes:** drag on the canvas to select the nodes in a box (shift or ctrl and a click add or take
  one away, ctrl+A all), drag one to move them all, del deletes them; Arrange (ctrl+L) puts the nodes in tidy
  columns by what they read, ordered so that connections cross little.
- **Builder** (where the application opens): open a raster, click the operators of the library (by
  landschaft's subjects, with a search) and they come connected to the node selected; what the node selected gives is shown under the
  canvas (a map with its legend, the maps of a stack or of a loop's steps, a table, a value). A run stops at the
  node shown and takes what did not change from the cache, so after every change of a setting only what it
  touches runs again and its result shows at once (ctrl+enter runs to the node, F5 the whole model). Save a map
  as a raster, the model as a file.
- **From the keyboard:** `/` opens a search of the operators anywhere; type part of a name or of what it does,
  the arrows choose, enter adds it connected to the node selected (which the new node becomes, so a chain is
  built key after key), esc closes the search.
- **Templates:** models to start from, run as soon as they open: MSPA of a forest (a map drawn by a neutral
  landscape model, reclassified, MSPA, plotted), MSPA of your map, a forest cut from a surface by ranges, the
  fragmentation of a forest (FAD at five scales and FOS), its patterns and objects (SPA and Accounting), MSPA's
  edge width compared, the neutral landscape models side by side.
- **Reclassifying:** the rules of `raster.reclassify` as a table (the values, what they become), the classes of
  the map it reads with their share beside them (A rule per class fills it), or its range for a continuous map
  (4 equal ranges), and how to write values, ranges (`0..10`), every other value (`*`) and no data (`nan`).
- **Colours that fit the data,** as landschaft's plot: classes (whole values, few of them) a colour each, other
  values a ramp, centred on 0 in blue and red when they are on both sides of it; what GuidosToolbox gives (MSPA,
  SPA, Accounting, FAD and FOS) in its colours and names, a density's legend in its classes (rare to intact),
  through the plots that show them and in files named as GTB names them; elevations, a
  hillshade and NDVI by their ramps (by the names of their nodes, files and bands).
- **GuidosToolbox:** landschaft's port of GTB, the same bits as GTB's own outputs: MSPA (`landscape.mspa`), the
  simplified pattern analysis (`landscape.spa`, 2 to 6 classes), the objects by size (`landscape.accounting`),
  the foreground's density (`fragmentation.fad`, at five scales `fragmentation.fad_multiscale`) and the
  fragmentation at a fixed scale (`fragmentation.fos`, by FAD, FED or FAC, binary maps or % covers), contagion and
  clustering in a window, the spatial entropy, the landscape mosaic, GraySpatCon's metrics of a % cover,
  parcellation, contortion, the Euclidean distance to the foreground's edge, and the simple, morphological and
  FOS change between two dates, with GTB's reports as tables. They read a map coded as GTB's (0 missing, 1 background, 2 foreground) or any map with the
  foreground's classes given.
- **Landscape metrics:** landscapemetrics' metrics (and FRAGSTATS' extras) per patch, per class, of the
  landscape and in a moving window (`metrics.patch`, `metrics.class`, `metrics.landscape`, `metrics.window`), the
  same values as R's landscapemetrics; tables sorted, filtered and drawn as bars in the viewer, and the maps of a
  metric per patch or class.
- **Spatial prioritization:** Zonation 5's ranking (`prioritize.zonation`), the same rank maps as the z5 binary;
  Marxan 4's reserves (`prioritize.marxan`), the same runs as Marxan; ProxIRR's irreplaceability
  (`prioritize.proxirr`), R's proxirr's bits; prioritizr's integer programs (`prioritize.optimize`: min set, min
  shortfall, max utility, max features, phylogenetic diversity and more; boundary, connectivity and linear
  penalties, neighbour and contiguity constraints; planning units in maps or tables; portfolios), prioritizr's
  models bit for bit, solved by HiGHS; the evaluation of solutions (`prioritize.evaluate`) and the calibration of
  trade-offs (`prioritize.calibrate`).
- **Restoration planning (plangea):** ecological intactness of a human footprint and its gradient as cells are
  restored, the original vegetation of converted land, the area of habitat of species; plangea's bits.
- **Rasters of any size:** opening a raster reads its header only; the viewer reads the tiles in view at the
  screen's resolution (in threads, the coarser level shown meanwhile), so a raster of billions of cells shows
  in under half a second and zooms smoothly; a folder or a pattern (`forest/*.tif`) is one stack, a layer per
  band of each file (Open a folder as a stack), to go through.
  Operators that need a map in memory read it then.
- **GDAL:** every format GDAL reads, and by GDAL: `raster.reproject`, `raster.resample`, `raster.match`
  (another raster's grid), `raster.clip`, `raster.convert` (formats, cell types, COG), `raster.mosaic`,
  `raster.terrain` (slope, aspect, hillshade, TRI, TPI, roughness), `raster.fill` (gaps) and
  `distance.proximity` (the distance to some values, on the file whatever its size); `raster.reclassify` by
  Terraflux. A series may hold files of different grids (tiles to join). GDAL is loaded when the
  program runs (no GDAL is needed to build Terraflux): from the folder `gdal` next to the program (the
  installers bring it; `tools/bundle-gdal.ps1` copies one there), `TERRAFLUX_GDAL`, or an installed GDAL.
  Without it, GeoTIFF is read and written by Terraflux itself.
- **Large diagrams:** the canvas is drawn by the GPU (the boxes, ports and connections in a few instanced draws
  whatever their number) and laid out only when the graph changes; texts are written for the nodes in view
  when the zoom makes them readable. Drag the background (or with the middle button) to move, the wheel to zoom,
  Home to see the whole graph. With 10,000 nodes a frame takes about 0.1 ms, dragging a node 0.5 ms.
- **Wizard:** a wizard in three steps picks the model (Dinamica EGO's, CLUE-S, ordered), reads the land use
  maps and the drivers (the dates and steps from the band names), checks the data as soon as it is chosen
  (grids, classes, missing values, constant or collinear drivers) and offers to fix what it can (resampling a
  driver to the land use map's grid), then writes the model with a note on every choice.
- **The node editor:** connect ports (each connection checked), type values and settings, edit the bodies of
  loops and groups; the note of every node says why it is there.
- **Map:** the maps of the run as they come, step by step, and the maps of the inputs: a stack's one after the
  other (a slider, the arrow keys, play) with small images of all of them; zoom, pan, the class and coordinates
  under the pointer.
- **Charts:** the area of each class along the steps.
- **Validation:** the last map against an observed one: figure of merit, hits, misses and false alarms, and
  Dinamica EGO's fuzzy similarity of the changes in windows of growing size.

![A Dinamica model of Sibuyan after 14 steps](docs/images/map.png)

![The builder: a raster, MSPA on it and its result](docs/images/graph.png)

If the window stutters when it is moved or resized, how frames reach the screen can be changed:
`TERRAFLUX_PRESENT=novsync` (frames as soon as they are drawn), `TERRAFLUX_LATENCY=2` (frames queued; 1 by
default) and `WGPU_BACKEND=dx12`, `vulkan` or `gl` (the graphics interface).

## Quick start

```bash
# a model from Dinamica EGO's classic one, fitted on two maps (14 steps between them)
terraflux new dinamica --t0 landuse.tif:1997 --t1 landuse.tif:2011 --drivers drivers.tif --steps 14 -o model.toml
terraflux check model.toml          # every operator, reference and connection checked
terraflux run model.toml            # writes output/maps.tif (a band per step), steps.csv, weights.csv
terraflux run model.toml --set seed=7 --out other
terraflux ops                       # the operators and what they do
terraflux resample drivers.tif --like landuse.tif -o drivers_aligned.tif
```

`terraflux new clue` and `terraflux new ordered` make lulcc's CLUE-S and ordered models (logistic regressions,
a demand interpolated from observed maps, conversion rules, elasticities, neighbourhoods, restrictions).

## Changing the logic

```toml
[[nodes]]
id = "suitable"
op = "map.calc"                                      # map algebra, as Dinamica EGO's Calculate Map
inputs = { land = "$t0", drivers = "$drivers" }
params = { expression = "if land == 3 and drivers.dist_roads < 500 then nbmean(land == 2, 1) else 0" }

[[nodes]]
id = "forest"
op = "control.submodel"                              # another model file as one node
inputs = { lu = "$t0" }
params = { path = "lib/forest.toml" }

[[nodes]]
id = "keep"
op = "plugin.wasm"                                   # a function in WebAssembly (any language), sandboxed
inputs = { land = "$t0", slope = "suitable.map" }
params = { path = "plugins/keep.wasm" }
```

See [`docs/modelling.md`](docs/modelling.md): the node editor, map algebra, groups and submodels, plugins.

## Agents

Populations of agents carried along the loop with the map: macro agents giving the demand, parcel managers
changing the potential of their parcels (`agents.table`, `agents.parcels`, `agents.update` with rules
`attribute = expression`, `agents.map`, `agents.summary`), and CRAFTY's competition for land between agent
functional types (`crafty.allocate`). See [`docs/agents.md`](docs/agents.md).

## Many runs, many machines

```bash
terraflux submit model.toml --replicates 50 --out replicates            # the cores of this machine
terraflux calibrate model.toml --param patches.isometry=0.5:2.5 --observed landuse.tif:2011 --metric similarity:5

terraflux serve --listen 0.0.0.0:7700                                   # a coordinator
terraflux worker --coordinator http://coordinator-host:7700             # on every machine that works
terraflux submit model.toml --scenarios scenarios.toml --coordinator http://coordinator-host:7700
```

Workers only connect to the coordinator; data travels once, by hash; a task whose worker vanishes runs on
another. See [`docs/cluster.md`](docs/cluster.md) (and the Slurm template in `tools/slurm`).

## Maps larger than memory

```bash
terraflux run model.toml --tiled                          # tiles of 1024 cells, layers in memory
terraflux run model.toml --tiled --tile 2048 --disk layers  # the steps' layers kept on disk
terraflux run model.toml --tiled --threads 8               # threads (default: all the cores but one)
```

The tiled engine runs Dinamica EGO's model tile by tile. Its Markov chains and weights of evidence are the
exact engine's to the bit (they are sums over the tiles); the patches a fit measures are measured within tiles.
Its allocation draws random numbers by counter and finds the seeds of the patches from histograms summed over
the tiles, so with the same fitted pieces the same seed gives the same maps whatever the tiling and the number
of machines, statistically equivalent to the exact engine's (the same areas of change, a similar fuzzy
similarity). Patches grow within `--radius` cells of their seed.

On many machines, the tiles go to a coordinator's workers, the maps and the layers on storage they share:

```bash
terraflux run model.toml --tiled --disk /shared/layers --coordinator http://coordinator-host:7700
```

No command takes every core: Rust's tasks, landschaft's OpenMP kernels and the workers of a local cluster share
all the cores but one, unless `--threads` (or `TERRAFLUX_THREADS`) says otherwise.

## From Python

[landschaft](https://github.com/sacridini/landschaft) writes its fitted models as Terraflux graphs:

```python
import landschaft as ls

S = ls.simulate
model = S.presets.dinamica(lu[0], lu[1], drivers, steps=14)   # fitted in Python
model.save("dinamica.json", lu[0], drivers, seed=1)            # the graph and its data
maps = S.terraflux("dinamica.json")                            # run by Terraflux: the maps of model.run
model, inputs = S.Model.load("dinamica.json")                  # and back
```

## The model file

A model is a graph of operators in TOML (or JSON): its inputs, its nodes and its outputs. Each node runs one
operator on inputs that come from the model's inputs (`$name`), from other nodes (`node.port`), from values
carried along a loop (`@name`, `@step`) or literal values; its `note` says why it is there. The models made by
`terraflux new` are written out node by node, so that anything they chose can be read and changed.

Rasters can also come in and go out through nodes, as the builder writes them (paths relative to the model's
folder):

```toml
[[nodes]]
id = "read"
op = "raster.read"
params = { path = "forest.tif" }

[[nodes]]
id = "mspa"
op = "landscape.mspa"                 # GuidosToolbox's MSPA, the same bits
inputs = { map = "read.map" }
params = { foreground = 1, edge_width = 2 }

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "mspa.map" }
params = { path = "forest_mspa.tif" }
```

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
| `terraflux-graph` | the model graph: file format, references, signatures, checks, submodels |
| `terraflux-ops` | the operators (Markov chains, demands, logistic regression, weights of evidence, CLUE-S, ordered, Expander/Patcher, map algebra, WebAssembly plugins, agents, CRAFTY) and the presets |
| `terraflux-exec` | the exact engine: inputs, graph run, outputs; the checks of the data |
| `terraflux-engine` | the tiled engine: fits by reduction, counter-based random numbers, allocation by thresholds |
| `terraflux-raster` | grids, rasters, GeoTIFF reading and writing in pure Rust (BigTIFF, tiles, windows, LZW, Deflate, predictors), resampling |
| `terraflux-ls-sys` | the bridge to landschaft's C++ kernels |
| `terraflux-app` | the application (egui, wgpu): wizard, node editor, live maps, charts, validation |
| `terraflux-dist` | the coordinator, the workers and the batches (gRPC) |
| `terraflux-cli` | the `terraflux` command |

## License

GPL-3.0-or-later.
