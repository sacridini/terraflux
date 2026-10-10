# Changelog

All notable changes to Terraflux are documented here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 the model format and the API may still change between
minor versions.

## [Unreleased]

## [0.5.0] - 2026-10-10

### Added
- The tiled engine (crate `terraflux-engine`, `terraflux run --tiled`): Dinamica EGO's model on maps larger than
  memory, cut in tiles read, computed and written one at a time, in parallel, with the steps' layers in memory
  or on disk (`--disk`). With the same fitted pieces, the same seed gives the same maps whatever the tiling
  (`--tile`); the patches a fit measures depend on the tiles (a patch across a tile's edge counts as two).
  - Fits by reduction over the tiles: the Markov chain, the weights of evidence and the patches' statistics
    are the exact engine's to the bit.
  - Random numbers by counter: every draw is a function of the seed, the step, the transition and the cell.
  - Allocation by thresholds: the Expander's and the Patcher's seeds are the cells whose random keys pass
    thresholds found from histograms summed over the tiles; patches grow within a radius (`--radius`) of their
    seed, every tile with a halo of twice that radius; cells claimed by several patches go to the highest key;
    the thresholds are adjusted in rounds until the changes meet the demand. Statistically equivalent to the
    exact engine (the same areas, a similar fuzzy similarity), not the same draws.
  - Few passes over the map per phase: the thresholds of every transition are found in the same passes; the
    largest keys are collected once, so each round's seeds are a selection; the changes past the demand are
    cut by their exact scores.
  - Each window of a file of drivers is decoded once for all its bands; readers are kept open per thread.
  - Layers on disk keep the tiles read lately decoded in memory (`TERRAFLUX_CACHE_MB`, 2048 by default).
  - `TERRAFLUX_TRACE` prints the time of each pass of a step.
  - 56.6 million cells, 10 transitions, 3 steps on 4 threads: 318 s (43 s of fits, about 92 s a step).
- The tiled engine on many machines (`terraflux run --tiled --disk <shared> --coordinator <url>`): every pass
  over the tiles is a job of the coordinator, its tasks runs of tiles, the workers reading the maps and writing
  the layers on shared storage; the maps are those of the same run on one machine (`terraflux_engine::pass`:
  passes as data, `terraflux_dist::tiles::Remote`). The coordinator waits for jobs (`Wait`) and forgets
  them (`Forget`).
- GeoTIFF: windowed reading (`Reader`) and tiled writing in any order (`TiledWriter`), for maps larger than
  memory.
- `woe.probability` computes the buckets of a variable once for the transitions from a class (they share
  their layout): about twice as fast, the same values.
- `--threads` on every command. Parallel work never takes every core: by default all the cores but one (or
  `TERRAFLUX_THREADS`), for Rust's tasks, landschaft's OpenMP kernels and the workers of a local cluster.

## [0.4.0] - 2026-10-10

### Added
- Batches on many machines (crate `terraflux-dist`, gRPC): `terraflux serve` (a coordinator), `terraflux worker`
  (a headless worker: it only connects to the coordinator), `terraflux workers`, `terraflux submit` (replicates or
  scenarios: a folder of outputs per task) and `terraflux calibrate` (a grid or a Latin hypercube of settings,
  each run with the same seeds, scored on the workers against an observed map: figure of merit, overall
  agreement or the fuzzy similarity of the changes; the best settings written as a calibrated model). Without
  `--coordinator`, a coordinator and workers run in the command's own process. The data travels once, by
  SHA-256; workers keep a cache; a task whose worker stops sending heartbeats runs on another (three attempts).
- Model overrides (`Model::apply`): `$input` and `node.setting` (`node/inner.setting` inside a loop).
- `docs/cluster.md` and a Slurm template (`tools/slurm/terraflux-workers.sbatch`).
- Tests: replicates on workers give the bytes of local runs; a calibration finds the setting that made the
  observed map; the task of a vanished worker runs on another; workers sharing a cache fetch the data together.
## [0.3.0] - 2026-10-10

### Added
- The application (`terraflux app`, crate `terraflux-app`, egui on wgpu):
  - a wizard of new models (Dinamica EGO's, CLUE-S, ordered): the land use maps and drivers picked, the dates
    and steps taken from the band names, the data checked, misaligned drivers resampled to the land use map's
    grid on request, the model written with a note on every choice;
  - the run in the background, its maps drawn step by step as they come (zoom, pan, the cell under the
    pointer), stopped at any time;
  - the area of each class along the steps; the model's graph (what each node does and why); the model and the
    problems of its data;
  - validation against an observed map: figure of merit, hits, wrong hits, misses, false alarms, and the fuzzy
    similarity of the changes per window.
- The checks of a model's data (`terraflux_exec::lint`, also in `terraflux check`): grids, coordinate reference
  systems, whole classes, change between the two dates, drivers without values, constant, missing where the land
  use has a class or collinear; each with what to do.
- `validate.agreement` and `validate.similarity` (landschaft's `agreement` and `similarity_of_differences`).
- Resampling to another grid of the same coordinate reference system (`terraflux_raster::resample`,
  `terraflux resample`): nearest or mode for classes, bilinear or average for continuous values.
- The engine reports the values collected at each step and can be stopped.
- Tests of the application rendered off screen (egui_kittest): a model opened and run, every tab, the wizard
  stopping on misaligned data.
## [0.2.0] - 2026-10-10

### Added
- Models written by landschaft (1.44.0, `Model.save`) run here bit for bit as `ls.simulate` runs them: their fitted
  pieces are given in the graph by `woe.weights`, `glm.coefficients`, `markov.matrix` and `patches.settings` (NaN
  coefficients as `null` in JSON).
- Dynamic variables: `dynamic.distance_to` (exact Euclidean distance, map units or cells), `dynamic.share_of`
  (focal share of classes) and `stack.set`, recomputed from the simulated map at every step.
- PLUS's CARS: `plus.demand` (a Markov chain projected from the starting map, or an area demand) and `cars.step`.
- `stack.classes`: layers named by class or transition (`1_to_3`) as a potential computed elsewhere.
- Tests: models saved by landschaft (Dinamica, CLUE-S with a dynamic variable and neighbourhood rules, CARS) give
  the maps landschaft gives.

### Changed
- The submodule `extern/landschaft` at 1.44.0.
## [0.1.0] - 2026-10-10

### Added
- The model graph (`terraflux-graph`): models as TOML or JSON files (`format = "terraflux/1"`) with named
  inputs (maps, stacks, values), nodes running operators on inputs from `$inputs`, `node.port` outputs,
  `@carried` values and literals, a note per node, containers with a body (`control.repeat`: carried and
  collected values), outputs; checks of every operator, reference, port, setting and type, and of cycles.
- The operators (`terraflux-ops`), each the same code as landschaft's `ls.simulate` through its C++ kernels:
  `markov.fit`, `demand.from_maps`, `demand.table`, `glm.fit`, `glm.predict`, `woe.fit`, `woe.probability`,
  `patches.observe`, `dinamica.step`, `lulcc.history`, `lulcc.step` (CLUE-S and the ordered allocation with
  conversion rules, elasticities, neighbourhoods, restrictions and history), `rng.r`, `control.repeat`; the
  presets `dinamica` and `lulcc` (CLUE-S and ordered) write their graphs with a note on every choice.
- The exact engine (`terraflux-exec`): inputs read from GeoTIFF, the graph run in the order of its
  dependencies, outputs written as GeoTIFF (series of maps as a band per step), CSV and JSON.
- GeoTIFF in pure Rust (`terraflux-raster`): strips and tiles, pixel and band interleaving, no compression,
  Deflate, LZW and PackBits, horizontal and floating-point predictors, little- and big-endian, classic TIFF and
  BigTIFF; the georeference tags, GDAL's no-data value and band descriptions kept.
- The bridge to landschaft's kernels (`terraflux-ls-sys`, cxx): landschaft 1.43.0 built with CMake
  (`LANDSCHAFT_PYTHON=OFF`) from the submodule `extern/landschaft`.
- The `terraflux` command: `run`, `check`, `ops`, `new dinamica|clue|ordered`, `convert`.
- Tests: parity with landschaft bit for bit (Dinamica EGO's model on Sibuyan, CLUE-S and the ordered allocation
  on Plum Island with every rule), the GeoTIFF layouts GDAL writes, the graph's checks, the engine's loops.
