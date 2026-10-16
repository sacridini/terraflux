# Changelog

All notable changes to Terraflux are documented here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 the model format and the API may still change between
minor versions.

## [Unreleased]

### Added
- `stack.layer`: one layer of a stack as a map, by its name or its number from 0: a band of rasters, a layer an
  operator made (`total` of `prioritize.evaluate`'s `ferrier`), a step of a loop's series; names that are numbers
  too, which an expression cannot reach.
- The node editor's boxes are as wide as their texts need: a long name, and on every row an input's and an
  output's names side by side (`prioritize.optimize`'s no longer run into each other); measured in the fonts
  they are written in. Drag a box's right edge to make it wider (kept in the model as `width`, never narrower
  than its texts), double-click the edge to bring it back; Arrange's columns and the place of a new node follow
  the widths, and the column of what a scope reads is as wide as its longest name; a file's name at the bottom
  of a box is cut to the box's width.
- `prioritize.optimize` solves with Gurobi where it is installed and licensed (`solver = "gurobi"`, `threads`), as
  prioritizr's `add_gurobi_solver`: its library loaded when a node asks for it (found by `GUROBI_HOME`, the
  installers' folders or `TERRAFLUX_GUROBI`), nothing of Gurobi needed to build or run Terraflux; without it, HiGHS
  solves and a warning says why.
- How far a node is, as its operator tells it, in a bar at the top of the window: the prioritizations' gap proved
  falling to the gap asked for, with the best plan and the bound. **Stop** interrupts a node in the middle: the
  solvers stop (HiGHS, landschaft's engines and Gurobi), and a node that cannot stop in the middle is left to end
  in the background a second later, its results not shown. Operators are given the means (`Ctx::progress`,
  `Ctx::stopped`).
- `terraflux mcp`: run_model runs in a thread of its own, so the other tools (open_in_app...) answer while a model
  runs; it tells the client how it goes (progress notifications: the nodes, the solvers' gap) and stops when the
  client cancels it.
- An output of a model writes one layer of a stack alone with `layer` (`[outputs.importance]`, `from =
  "evaluation.ferrier"`, `layer = "total"`), and `terraflux mcp`'s `set_output` takes it.

### Changed
- `prioritize.optimize` solves as landschaft does: a minimum set with boundary penalties or symmetric
  connectivity rewards on landschaft's cut engine (a Lagrangian bound by minimum cuts, repaired selections), a
  budgeted minimisation with pair penalties on its closure engine, then HiGHS's MIP from their plan when the gap is
  not reached and the model has at most 50,000 columns. A minimum set of 49 species with a boundary penalty and
  locked-in areas on 85,732 cells: from 45 minutes (HiGHS's MIP alone) to 4 seconds, the same plan as landschaft.

## [0.82.0] - 2026-10-16

### Added
- `terraflux mcp`: Terraflux as a server of the Model Context Protocol, for AI assistants (Claude Desktop, Claude
  Code, any MCP client) to build models in plain words: the templates and the operators listed and described (a
  node of a template shown for each), a model made empty or from a template, its inputs, nodes, connections,
  settings, containers' bodies and outputs edited (each edit checked as the node editor checks it: a wrong
  connection, a setting the operator does not have or a cycle refused with the reason, the file left as it was;
  the problems left listed after each edit), the nodes arranged, the model checked, run and its outputs looked at
  (a raster's size, georeference, classes or statistics; a table's first lines), and the model opened in the
  application. New nodes are placed beside what they read; the file is written whole, so a program following it
  never reads half a model.
- The application follows the file of the model open: written by another program (an assistant through
  `terraflux mcp`, a text editor), it is read again, as an edit that ctrl+Z undoes (the data read again when the
  inputs changed). With only nodes moved here and not saved, the file's model is taken with the nodes where they
  were moved; with other edits not saved, a bar asks whether to read it or keep the model as it is.

## [0.81.0] - 2026-10-16

### Added
- A Files pane, in a tab beside the operators', a file manager for data:
  - back, forward and up (alt+arrows, the mouse's side buttons), the path's folders as buttons (a click on the
    bar, or ctrl+L, to type a path, the folders that match offered, tab completing), places (folders pinned with
    the star, the model's, home, documents, desktop, downloads, the drives, the folders seen last);
  - a folder's rasters, vectors and models (every file if asked; hidden files and the files that go with others
    left out), with their icons, size and age, sorted by any column (names as people sort them: `map2` before
    `map10`), filtered by a part of the name or a pattern (`*.tif`); a folder opens in place by its arrow;
  - a click chooses, ctrl+click one more, shift+click a range; the arrows, right and left (a folder opened or
    closed in place) and the first letters of a name from the keyboard;
  - the raster or vector features chosen described under the list (a picture of them, their size, coordinate
    system, cells, bands, type, no-data; features, kinds, attributes, bounds), read in a thread;
  - what is chosen dragged to the model's canvas is a node each that reads it, where it is let go (several at
    once); enter or a double click adds it after the node selected; a right click reads a folder of rasters as
    one stack, opens a model, copies the path, shows the file in the system's file manager.

  The folder shown, the folders pinned and seen last are kept between sessions.
- Rasters and vectors dragged from the system's file manager onto the main window or a window of its own: a node
  each that reads it (`raster.read` or `vector.read` by the extension); a model dropped is opened.
- A node that takes any inputs (`stack.of`'s maps, a group's values) shows a place for one more on the canvas: a
  connection let go on it, or anywhere on the node, adds an input named after the node it comes from
  (`elevation.map`: `elevation`, then `elevation_2`...). A raster joins a stack by a connection.
- At the viewer's closest zoom each cell's value is written in it (only the cells in view, read from the map in
  memory or the tiles kept).
- ctrl+D duplicates the nodes selected (beside them, reading what they read); ctrl+C copies them and ctrl+V
  pastes them, through the system's clipboard (to another container, model or window): new nodes each time, of
  new ids (`calc`: `calc_2`), connected to each other as the originals were, where the pointer is on the canvas.
- Space over the viewer's map hides it (what is under and over it seen alone), and shows it again.
- `table.read`: a table from a file, comma-separated (`.csv`), tab-separated (`.tsv`, `.txt`), with `;` and a
  decimal comma as Excel writes them where a comma marks the decimals (both found by themselves), or JSON; the
  columns of text left out (a table is of numbers), named in a warning. The Files pane lists `.csv` and `.tsv`
  files as tables (a node that reads them by a double click or a drag; their columns and first rows described).
- `table.write`: a table to a file, comma-separated (`.csv`, the default), tab-separated (`.tsv`, `.txt`) or
  JSON (`.json`); the separator and the decimal mark as a spreadsheet of the place expects (`;` and `,`). In a
  batch, each run writes it in its own folder, as the other writers.
- `view.plot` shows tables too (a `table` input): drawn as a chart in its tab, bars, lines, points or boxes
  (`chart`), of the columns `y` along `x`, a group by `by`; a chart suggested when none is asked (lines along a
  date, year, step or layer, else bars), and changed in the tab as any table's.
- Undo and redo of the model's edits: ctrl+Z, ctrl+shift+Z or ctrl+Y, and two buttons over the canvas. Each edit
  is a step (a node added, deleted, pasted, a connection, a setting); what changes while a button of the mouse is
  held (nodes dragged) is one.

### Changed
- `raster.read` has one output, `raster`: the file on disk, one band (`band`, else the first) where a node takes a
  raster and every band where it takes a stack, as its `map` and `stack` outputs were. A model that read them
  reads `raster` (brought up to date when it is read; the website's tutorials and the templates changed). The
  viewer shows one entry per raster read among the maps over a map (three before), and the buttons of a node's
  outputs only for a node of several. Types are named for people where the application shows them: a map in
  memory and rasters on disk are both a raster.
- A node added is named after its operator's whole name (`stack_of`, `raster_read`, `vector_read`,
  `landscape_mspa`), not after its last part (`of`, `read`).
- The operators' lists (the library, the search with "/", the menu) write the whole names (`stack.of`, the
  subject a shade weaker), not the part after the dot.
- On the model's canvas a node that reads or writes a file (`raster.read`, `vector.read`, `raster.write`...) and
  a model's input of a raster name the file at their bottom (its whole path when the pointer is over it). The
  nodes of data (the model's inputs, what reads, writes, stacks or plots) may have a header in the colour of the
  type they give (a map blue, vectors pink...), an option of the settings (Model), off at first.
- The viewer is a tab beside the model's, not under it, and opens with the first data (a raster or vectors read,
  a template; behind the model's tab when a model is opened). A run's plots open beside it without taking the
  front from the model.
- The operators' and the node's panes open a quarter narrower; the window opens maximized.
- The node editor's connections and ports in the colour of their type, the website's graphs' own: maps blue,
  stacks violet, tables gold, numbers green, vectors pink, agents ochre (a connection being drawn too).
- The operators pinned again at the window's left edge, the node's pane beside them, the model and the viewer in
  the rest. The operators' pane closed, they are in the drawer at the left edge and in the toolbar as before.

### Fixed
- The model's boxes, ports and connections drawn when another model is opened (a template after the blank model
  the application starts with): every editor's first shapes had the same generation, so the GPU kept the shapes
  it had and only the nodes' texts showed until a node was moved.
- The window opens maximized on Windows (made maximized, it was not once shown), and the model's pane is not blank
  when the layout kept had the viewer in front of it.
- A tab dragged out of the dock no longer trips egui's check of a widget drawn in two windows in one frame (its
  window of its own shown from the next frame).
- A pane opened from the View menu has the focus (it was put in the dock behind the tab in front).

## [0.80.1] - 2026-10-15

### Fixed
- `raster.read`'s `map` is the band chosen alone: since 0.80.0 a multi-band file's map reached map algebra, groups
  and submodels (inputs of any value) as a stack of every band. A band is still read once, whichever port asks.
- `terraflux calibrate` types a range by the model's value of the setting for every node (0.80.0 found only the
  model's first node).
- `terraflux run --set node.setting=value` refuses a setting the node's operator does not take (a misspelt name ran
  silently).
- The template "A city that grows (a loop)" grows: with its neighbours counted by `nbsum` (0.80.0), its bar was
  above every cell next to the old town; three city neighbours and a bar from 0.6 now.
- `terraflux serve` without a code on 127.0.0.1 prints the worker command with its address, and `terraflux jobs`
  says when there are none.

## [0.80.0] - 2026-10-15

### Added
- Templates for many more of the tools, each running as soon as it opens on data drawn by neutral landscape
  models: Forest gain and loss (simple, morphological and FAD change), Patches and their shapes (patch metrics,
  parcellation, contortion), Kinds of landscapes (spatial signatures, PCA, k-means), Network of a forest (MSPA's
  GTN components and importance), Habitat network (Conefor's IIC and PC, every patch's dPC), Circuit
  connectivity (Circuitscape's pairwise currents, Omniscape), Least costs and random walks (gdistance),
  Restoring a corridor (GTB's Restoration Planner), Irreplaceability of a plan (prioritizr's evaluation and a
  portfolio), A species in a warmer climate (response curves, importance, the projection and MESS), Drivers of
  land change (Markov, weights of evidence, logistic regressions), A city that grows (a cellular automaton in a
  loop), Farmers on their parcels (agents), CRAFTY, Land change analysis (transitions, Pontius's components,
  intensity analysis), Validating a simulation (figure of merit, fuzzy similarity, three-map comparison,
  kappas), Terrain of a surface, Carbon and its uncertainty (Monte Carlo) and More neutral landscapes.

### Changed
- The Templates menu in groups: patterns, landscape metrics, connectivity, conservation and restoration,
  species, land change, surfaces and uncertainty.
- `terraflux run --set node.setting=value` changes a node's setting, as a scenario does (`--set name=value` an
  input, as before).
- `terraflux calibrate` (and the Many runs pane) makes a range's values whole numbers when the model's value of
  the setting is one and decimals when it is not (`patches.expansion=0:1` of a share tries 0, 0.25, 0.5...), and
  rounds them to 12 significant digits (0.7, not 0.7000000000000001).
- `terraflux check` reports a setting written as a reference (`edge_width = "$x"`): settings take values.
- Map algebra: the radius of `nbsum nbmean nbmin nbmax nbcount` may be a number input (`nbmean(x, r)`).
- `stack.of` takes inputs named by numbers: `1_to_2` makes the layer a transition's (a probability for
  `dinamica.step`, so map algebra can make them), `1` a class's.
- `batch.run`'s table has a column `skipped` (inputs done before, not run again), and its report says so.
- A multi-band raster connected to an input of any type arrives as a stack of every band, not its first.
- A run writes its text outputs as plain text (`name.txt`), not as a JSON string.
- The node's note is edited in the Node pane ("Why").
- `terraflux serve --code` says how to start workers for its address (no beacons on 127.0.0.1), `terraflux
  workers` says when there are none and marks those not heard of for 30 s as silent, and `terraflux submit` prints
  its job once.
- `terraflux cloud-init --worker`: the worker service runs as an unprivileged user (systemd's `DynamicUser`).
- `terraflux new clue` and `new ordered` write the start band by its name when the observed file has band names.

### Fixed
- A model no longer stops when an output given only with some settings or inputs is missing and nothing reads
  it; a node that reads it is told why. This was the cause of `gave no output` with `metrics.class` and
  `metrics.patch` without `show` or by blocks, `bio.cluster` without calculations, Marxan's and ProxIRR's tables
  mode, Omniscape without its flow potential, and circuit networks without their maps.
- The templates counted neighbours in a state with `nbcount` (cells with a value) where `nbsum` was meant: the
  city of "A city that grows" spread to every suitable cell, and the land change templates' second date grew
  farming almost everywhere. The CLUE-S template takes a smaller iteration step (lulcc's default never met the
  demand on its 25,600 cells).
- The names of the landscape mosaic's classes 6 to 12 (two of them shared one), as landschaft 55d03a8.
- `sdm.maxnet` with a records table on `data` no longer takes the coordinates, the folds and the weights as
  predictors.
- `prioritize.calibrate`'s pareto approach takes any objective (it always solves min shortfall within budgets).
- A raster on disk written by `raster.write` no longer has its band's number as its description.
- The descriptions of `change.fos` (the sign of its change), `networks.gtn_importance` (within 1, 5, 10 % of the
  most important), `conn.*` (least-cost distances between centroids; importance without measuring distances
  again), `circuit.one_to_all` (all-to-one gives no resistances), `nlm.random_cluster`, `sdm.exdet` (mic is
  1-based), `sdm.fit`'s feature classes and `prioritize.calibrate`'s time limit; the README and `run --tiled` on
  what the tile size changes.

## [0.79.0] - 2026-10-14

### Added
- Machines rented in the cloud: `terraflux cloud-init` writes the setup script (cloud-init user data) of a server
  (Terraflux installed from the Linux package of this version, then saved with `terraflux cluster add`) or, with
  `--worker`, of a machine that keeps a worker running as a service for a server's coordinator (`--on <server>
  --coordinator <its private address>`: its port and code from the profile). In the application, the Computers
  pane copies both scripts.

## [0.78.0] - 2026-10-14

### Changed
- The tiled engine on many machines needs no storage in common: the model's maps go to the workers once (by
  hash), and the steps' layers by their tiles (a pass lists each layer's tiles and their hashes; a worker fetches
  those of its tiles and of their neighbours, and sends back by hash the tiles it wrote, fetched into the layer's
  folder on the machine that runs the model). The same maps as on one machine. `terraflux run --tiled
  --coordinator ... --shared-disk` keeps the workers reading and writing the maps on shared storage, as before.

## [0.77.0] - 2026-10-14

### Added
- Clusters and servers reached by SSH, saved once (`terraflux cluster add/list/test/stop/remove`; in the
  application, the Computers pane): `submit --on`, `calibrate --on` and `results --on` check Terraflux there (the
  same version), start its coordinator (with a pairing code of the profile's) and its workers (on the host, or in a
  Slurm allocation of `sbatch`), reach it through an SSH tunnel, and stop them once the batch is done; a batch
  whose client went away goes on there and is followed again by its job. "Many runs" in the application runs on a
  cluster chosen in **Run on**.
- `terraflux worker --threads` (the threads of each task) and `terraflux serve --no-beacons`.

## [0.76.0] - 2026-10-14

### Added
- Other computers of the local network help with the runs, from a code and nothing else: the **Computers** pane
  (the toolbar's "Computers...") shows this computer's code when "Let other computers help" is on, and lends this
  computer to another's by typing the other's code ("Lend this computer": the threads it uses, only when nobody
  uses it for some minutes, paused at will). The computers find each other on the network by themselves; the
  sharing computer lists those helping, the lent one says whom it works for and how many runs it did. "Many runs"
  goes to the application's own coordinator (its jobs in the user's application data), which other computers
  join when it is shared.
- Pairing codes for coordinators (`terraflux serve --code`, `worker --code`, `submit --code`, `calibrate --code`,
  `jobs --code`, `results --code`, `run --tiled --code`): TLS with a certificate the coordinator makes for
  itself, the coordinator's proof that it knows the code (an HMAC of its certificate) before anything is sent,
  the certificate then pinned, and every call with a token made from the code (PBKDF2-HMAC-SHA256); without
  `--coordinator`, the coordinator of the code is found on the local network by its beacons (UDP port 7701).
- The Windows installer lets Terraflux through the firewall for the local network.

## [0.75.0] - 2026-10-14

### Added
- Batches taken up again: the coordinator keeps its data, the results' files and its jobs on disk (`terraflux
  serve --data`, by default in the user's application data) and goes on with them when started again (the tasks
  that were running run again). `terraflux submit --detach` only submits a batch, `terraflux jobs` lists a
  coordinator's jobs and `terraflux results <job>` follows one to its end and writes its files (those written
  already kept). In the application, a batch the application was closed on is offered again ("Go on").
- Every file a model reads travels with it: the files of `raster.read` and `vector.read`, with the files beside
  them of the same name (a shapefile's parts, a raster's sidecars), the rasters of a folder or a pattern, and the
  files a scenario names; they were read from the paths of the machine that submitted the batch.
- The computer kept awake while a worker runs a task (Windows, macOS, Linux); `terraflux worker --idle <minutes>`
  takes tasks only when the computer's user is away (Windows and macOS).

### Changed
- Workers and clients of another version than the coordinator's are turned away, with the version to install
  (`WorkerHello.version` was not checked).
- Files travel in pieces of a megabyte (no more messages of up to a gigabyte held in memory); a task's files go
  to the coordinator as blobs, and the client fetches each run's files as soon as it is done.

## [0.74.0] - 2026-10-14

### Added
- Many runs from the application: the "Many runs" pane (the toolbar's "Many runs...") runs the model again and
  again on workers of this computer, as many at once as chosen. Replicates (the model's seeds found by
  themselves, or chosen), scenarios in a table (a row per scenario, a column per setting it changes, any input
  or node setting picked by name) and calibration (settings tried in ranges, every combination or random draws,
  each with some seeds, scored against an observed map by the fuzzy similarity, the figure of merit or the
  share of cells of the same class). The runs' progress and the time left; the results as a table, in a folder
  per run; the runs of a map as one series in the model in a click; a calibration's ranking, its scores against
  each setting and its best settings applied to the model in a click.
- `raster.read` takes patterns in the folders' names too (`runs/*/landuse.tif`: a file of each run, the layers
  named by their folders).
- `terraflux_dist::run`: a batch from start to end (placed here or on a coordinator, its data sent, followed,
  its results written), the command line's and the application's.

### Changed
- In a batch, the files the nodes write (`raster.write`, `vector.write`, `landscape.mspa_tiled`) go to each run's
  folder and come back with its outputs, instead of every run writing the same file.
- A batch's seed may be a node's (`--seed-input simulation.seed`) or several (`seed,other.seed`); a calibration
  may score any output of maps and start from a node's map (a `raster.read`), not only the model's inputs.

## [0.73.0] - 2026-10-14

### Added
- scikit-learn's models in `sdm.fit` (landschaft's fit with them): `model = "glm"` (StandardScaler and
  LogisticRegression, the L2 penalty's `C`; the optimum scikit-learn converges to, within its tolerance) and
  `model = "random_forest"` (RandomForestClassifier: the trees grown as its Cython grows them, from numpy's legacy
  MT19937 seeds and bootstraps, its rand_r, introsort and Gini; `n_estimators`, `max_depth`, `min_samples_split`,
  `min_samples_leaf`, `max_features`, `bootstrap`, `random_state`), the same trees and probabilities to the bit. The
  records weighted as landschaft's dataset weighs them (`weights`: balanced by default, equal, or a weight column),
  the forest's bootstraps drawn with the weights as scikit-learn draws them. The fitted model (`SdmModel`) is MaxNet,
  a GLM or a forest; `sdm.maxnet_predict`, `sdm.response_curves` and `sdm.importance` take any of them.

## [0.72.0] - 2026-10-14

### Changed
- `nlm.mosaic_tess` and `nlm.mosaic_gibbs` follow NLMR by default (`nlmr_compat`, as landschaft's): the k-th value
  goes to the k-th Voronoi cell in the order GEOS lists them (sf's `st_voronoi` of the germs' union), the values
  as floats as `terra::rasterize` writes them; `nlmr_compat = false` keeps the earlier maps (each germ its own
  value). The same maps as landschaft's, to the bit.

### Added
- GEOS's own C API for what GDAL does not pass on (`terraflux_gdal::geos`): the geos_c library next to GDAL's (in
  the installers' GDAL too), else the system's; its Voronoi diagram.

## [0.71.0] - 2026-10-14

### Added
- The calibration of FUTURES' patches (landschaft's calibrate_patches, r.futures.calib): `futures.calibrate`
  takes futures.simulation's inputs and settings and the development observed at the end; the patches developed
  in between give the patch library and the histograms of patch area and compactness, and every combination of
  the compactness means, ranges and discount factors is simulated `repeat` times from the start and compared with
  them (the histograms' chi-square distance, numpy's histograms and sums). The table of the combinations from the
  best and the library, the same as landschaft's to the bit.

## [0.70.0] - 2026-10-14

### Added
- Linux packages (`tools/make-linux-package.py`, in `target/linux`): a folder to keep anywhere, as a tar.gz (its
  `install.sh` links the command into `~/.local/bin` and adds a menu entry for the user) and a Debian package
  (`/opt/terraflux`, the command in `/usr/bin`, the menu entry). They bring the system's GDAL (its library and every
  library it needs but the C and C++ runtimes, in `gdal/`, each loading the others from there; GDAL's and PROJ's
  data and the web's certificates) and the OpenMP runtime: nothing else is needed but a glibc as new as the one
  they were built with. Terraflux builds and runs on Linux (Ubuntu 24.04: the application on Wayland and X11).

## [0.69.0] - 2026-10-14

### Added
- Multi-action planning on rasters (landschaft's actions_raster_data, after prioripy): `prioritize.actions_raster`
  takes the cells with a monitoring cost as the planning units, a layer per feature (its amounts) and per threat
  (its intensity), the actions' costs as numbers or maps (one for all, per threat in order or by name), the units
  locked in or out by maps, a sensitivity table and the cells' shared edges as boundary data; builds
  prioriactions's tables as landschaft builds them (relative targets shares of what each feature has where a
  threat harms it and where none does) and solves them as `prioritize.actions`; the monitored cells and every
  threat's actions as maps. The same tables and optima as landschaft's on its test rasters and on its example.

## [0.68.0] - 2026-10-14

### Added
- Zonation 5's projects (landschaft's read_project and run_project): `prioritize.zonation_project` reads a
  settings file (`.z5`), its feature list (weights, groups, thresholds, ABF's z, zero modes, value extraction as
  `file.tif/3`) and link files (conditions, retention, weight groups, connectivity kernels, interactions and
  matrices, impact scenarios and their binning) with the switches of the z5 command line (`-wgtcrxXhabfdiImsep`,
  `--area=N`, `--mode=`), reads the rasters they name and runs the ranking of `prioritize.zonation`, adding up the
  features by the blocks of rows of their files as z5 does; with `-p` the calls of the post-processing file run on
  the ranking one after another (the tables numbered by call, the maps named as Zonation's files). The projects of
  landschaft's 65 cases give the same rank maps as the z5 binary, to the bit.
- The height of a raster file's blocks (strips or tiles), read by Terraflux's own TIFF reader and by GDAL.

## [0.67.0] - 2026-10-14

### Added
- The matrices and rasters around prioritizr's problems (landschaft's prioritize): `prioritize.boundary_matrix`,
  `prioritize.adjacency_matrix` (4 or 8 directions), `prioritize.connectivity_matrix` (the shared boundary times
  the mean of a map or an attribute) and `prioritize.proximity_matrix` of cells or polygons, as the i, j, x tables
  the problems take; `prioritize.decay_connectivity` (distances between units as Conefor's dispersal
  probability), `prioritize.unit_points`, `prioritize.coarsen` and `prioritize.refine` (a problem on coarser blocks
  and its selection back on the fine grid), `prioritize.binary_stack` (zone numbers as a 0/1 layer per zone) and
  `prioritize.protected_cells`. The same as landschaft's, to the bit.

## [0.66.0] - 2026-10-14

### Added
- FUTURES' inputs (landschaft's simulate): `futures.devpressure` (r.futures.devpressure: the development pressure
  by occurrence, gravity or kernel, its r.mfilter by landschaft's C++, padded or not) and `futures.patch_library`
  (r.futures.calib's library of the patches developed between two maps, whole or per subregion, with a threshold;
  in the form of futures.simulation's patch_sizes). The same as landschaft's, to the bit (the kernel to the last
  bit of a float).

## [0.65.0] - 2026-10-14

### Added
- The comparisons of simulated and observed land change (landschaft's simulate): `validate.three_map` (lulcc's
  three-map comparison of Pontius et al. 2011 at blocks of several sizes, its table by landschaft's C++, the budget
  of agreement and disagreement and the figure of merit, overall, per class and per transition),
  `validate.kappa` (MOLUSCE's kappa, kappa location and histogram, correctness, Cramer's V, joint information
  uncertainty, correlation), `validate.error_budget` (MOLUSCE's error budget of Pontius 2002 at coarser
  resolutions), `validate.futures` and `validate.futures_grid` (r.futures.validation and gridvalidation:
  disagreement, kappa, kappa simulation and the change metrics, whole, by tiles or by regions). The same as
  landschaft's, to the bit (MOLUSCE's correlation and joint information uncertainty, whose numpy uses BLAS and its
  own logarithm, to 1e-12).

## [0.64.0] - 2026-10-14

### Added
- diffeR's comparisons at coarser resolutions (landschaft's simulate): `change.memberships` (the share of each
  class in blocks of cells), `change.composite` (the composite cross-tabulation of Pontius & Cheuk),
  `change.resolutions` (differenceMR: quantity, exchange and shift at resolutions growing by a factor, scaled to a
  population or not), `change.mad` (the mean absolute deviation of two continuous maps by quantity, strata and
  element, at every resolution) and `change.sample_to_population` (sample2pop). R's and terra's sums; the same as
  landschaft's, to the bit.

## [0.63.0] - 2026-10-14

### Added
- `vector.exact_sum` and `vector.distance` out of core (`block_rows`, and rasters on disk of more than 250 million
  cells): the exact sums with only the values under each polygon read from disk (exactextract's coverage of the
  polygon on the whole grid, in landschaft's pieces), the distances by strips into a GeoTIFF (the features burnt
  strip by strip, the column passes through a file of the nearest rows below, the rows' envelopes); the same bits
  as in memory.

## [0.62.0] - 2026-10-14

### Added
- Biodiverse's spatial conditions on shapefiles in `bio.spatial`, `bio.randomise` and the other nodes of
  conditions: `sp_point_in_poly_shape` (the groups inside or on the features of a vector file, `file.gpkg:layer`
  or a shapefile, or of those with an attribute's value, or the feature at a position; the neighbour's or the
  processing group's coordinates, other axes) and `sp_points_in_same_poly_shape` (the groups in the same features,
  identical geometries one); the same masks and neighbourhoods as landschaft's.

## [0.61.0] - 2026-10-14

### Added
- Biodiverse's other imports (landschaft's BaseData.from_raster, from_vector, from_hex, from_h3 and
  from_occurrences): `bio.raster` (a label per layer and its values as counts, a threshold for presences, larger
  group cells; or the values as labels), `bio.vector` (points, lines and polygons over a fishnet of cells: the
  cells they touch, or the area or length of their parts, GEOS through GDAL; counts from attributes, binarised),
  `bio.hex` (hexagons, pointy or flat), `bio.h3` (H3 cells, their ids kept) and `bio.occurrences` (a GBIF download,
  the zip or its table, a Darwin Core Archive with meta.xml, a delimited file or records with Darwin Core columns
  such as `sdm.download_gbif`'s; cleaned as read_occurrences cleans them, read a line at a time through GDAL's
  virtual file systems; in cells of degrees or of a projection). The same groups, labels, counts and coordinates
  as landschaft's.

## [0.60.0] - 2026-10-14

### Added
- Occurrence records from GBIF (landschaft's download_gbif): `sdm.download_gbif`, GBIF's occurrence search API
  through GDAL's HTTP (no account): a scientific name matched to GBIF's backbone (a synonym gives the accepted
  name's records) or a taxon key, by country, the box of an area, years, basis of record; up to 100 000 records,
  pages asked a few at a time; the records with coordinates as points with their Darwin Core terms and GBIF's
  issues, those less precise than a distance left out; the query, the taxon matched and the count.

## [0.59.0] - 2026-10-14

### Added
- The preparation of a species distribution model in one node (landschaft's prepare): `sdm.prepare` cleans the
  records, keeps one per cell, thins them, draws the accessible area, selects the predictors in it, draws
  background points in it, takes the predictors' values and makes the folds (spatial blocks, of the
  autocorrelation range too, quadrants, a checkerboard, random, jackknife or clusters), each step the node of its
  own with its settings and one seed; an area or background points of one's own in place of those steps; the
  report of what each step removed and every setting. The same records, predictors, folds and report as
  landschaft's, to the bit.
- The builder's template "Species distribution model": predictors and presences drawn by chance, `sdm.prepare`,
  MaxNet fitted over spatial folds and its map of suitability.

## [0.58.0] - 2026-10-14

### Added
- The area accessible to a species (landschaft's accessible_area): `sdm.accessible_area`, the land within a
  distance of the records (geodesic circles, GeographicLib's, merged), their convex hull or the regions that hold
  them (widened by a distance), cut at the antimeridian and folded back, within a mask; the polygon of landschaft
  (its areas, planar and geodesic, to 1e-9; GEOS through GDAL, so not its vertices).

## [0.57.0] - 2026-10-14

### Added
- The cleaning of occurrence records (landschaft's clean_occurrences, CoordinateCleaner's tests):
  `sdm.clean_occurrences` flags or drops records with invalid coordinates, equal longitude and latitude, at 0/0,
  at capitals, country and province centroids, GBIF's headquarters, biodiversity institutions or artificial
  hotspots (the circles of CoordinateCleaner from landschaft's C++, geosphere's or terra's), in the sea, in towns,
  outside the country they claim, far from the other records of their species (quantile, MAD or distance; the
  raster variant of large species) or duplicated; each test off, on, with its main option or with an object of
  options, with the `verify` of the capitals, centroids, GBIF and institutions. `sdm.reference_data` gives the
  reference data as features. CoordinateCleaner's tables travel inside Terraflux (tools/make_sdm_refdata.py);
  Natural Earth's land, urban areas and countries are downloaded on first use into the cache (pinned by their
  SHA-256), or connected as polygons of one's own. The same flags as landschaft's, record by record.
- Occurrence records as features without geometries (a CSV read by GDAL): their coordinates from the columns.

## [0.56.0] - 2026-10-13

### Added
- The choice of predictors and their range of autocorrelation (landschaft's vif, select_predictors and
  autocorrelation_range, after usdm and blockCV): `sdm.vif` (R's lm over the complete cells, usdm's sample of
  them on large grids, by latitude on lon/lat grids, or a table's rows), `sdm.select_predictors` (vifcor and
  vifstep, predictors kept, within an area) and `sdm.autocorrelation_range` (gstat's variograms, spherical,
  exponential, Gaussian and Matern, fitted to a sample of every layer; their median range and the block size).
  The same as landschaft's, to the bit.

## [0.55.0] - 2026-10-13

### Added
- The species distribution modelling workflow with MaxNet (landschaft's dataset, fit, response_curves and
  importance): `sdm.dataset` (presences, or a table of 1 and 0, and background points with the predictors'
  values at their cells), `sdm.fit` (one fit per fold of the records with AUC of training and test, their
  difference, the largest TSS, Boyce's index and the omission rates; their means and standard deviations as
  pandas computes them; the model on every record and its threshold), `sdm.response_curves` (each predictor
  over its range, the others at their mean or median) and `sdm.importance` (the drop of AUC when a predictor
  is shuffled, numpy's generator). The same as landschaft's, to the bit.

## [0.54.0] - 2026-10-13

### Added
- Partitions of records for cross-validation (landschaft's folds, after ENMeval and blockCV), as a fold column
  added to the records: `sdm.random_folds`, `sdm.jackknife_folds`, `sdm.quadrant_folds` (the four
  orientations), `sdm.checkerboard_folds` (one or two aggregations of the predictors' cells),
  `sdm.spatial_blocks` (hexagons pointy or flat, squares of a size or of rows and columns; random, systematic
  or checkerboard assignment), `sdm.spatial_clusters` (R's kmeans of the coordinates or of the predictors) and
  `sdm.buffer_folds` (buffered leave-one-out). R's generator and long double arithmetic; the same as
  landschaft's, to the bit (with a seed: landschaft draws one when none is given, Terraflux defaults to 1).

## [0.53.0] - 2026-10-13

### Added
- Occurrence records and background points for species distribution models (landschaft's thin, one_per_cell
  and background): `sdm.thin` (spThin's thinning, the repetition that keeps the most), `sdm.one_per_cell` (the
  first record per cell, those on missing cells left out or not) and `sdm.background` (dismo's randomPoints with
  R's sampling: the cells' widths on lon/lat grids, a bias, an area, a band of distances from the records,
  outside their environmental envelope). Records as points or tables of coordinates (lon/lat columns
  recognised; PROJ through GDAL between coordinate systems). The same as landschaft's, to the bit.

## [0.52.0] - 2026-10-13

### Added
- MaxNet species distribution models (landschaft's MaxNet, the maxnet package; a new `SdmModel` type):
  `sdm.maxnet` (from a table with a presence column, or tables of presences and background points; the
  default classes or any of l, q, p, h, t; categorical predictors; the regularisation multiplier; the presences
  added to the background or not) with its coefficients by feature name, and `sdm.maxnet_predict` (on a stack
  of the predictors or a table; cloglog, logistic, exponential or the link; clamped or not). The same as
  landschaft's, to the bit.

## [0.51.0] - 2026-10-13

### Added
- The evaluation of species distribution models (landschaft's evaluate, boyce, threshold and omission_rate):
  `sdm.evaluate` (AUC, the point-biserial correlation and its p-value, prevalence, the largest TSS and Boyce's
  index; dismo's confusion table at every threshold and the optimal thresholds), `sdm.boyce` (ecospat's
  continuous Boyce index in a moving window, classes or breaks), `sdm.threshold` and `sdm.omission_rate`
  (ENMeval's mtp and p10). The same as landschaft's, to the bit, but for the p-value of the correlation
  (Student's t) to rounding.

## [0.50.0] - 2026-10-13

### Added
- Extrapolation of species distribution models (landschaft's mess, mop and exdet): `sdm.mess` (Elith's MESS,
  per predictor and the least), `sdm.mop` (the mop package's MOP: basic, simple or detailed ranges, Euclidean
  distances scaled and centered or not, Mahalanobis, rescaled, over the in-range, out-of-range or all
  conditions) and `sdm.exdet` (Mesgaran's NT1, NT2 and most influential covariate), on stacks of predictors or
  tables. MESS and the Euclidean MOP are landschaft's to the bit; the Mahalanobis MOP and ExDet (numpy's
  covariance, LAPACK's inverse) to rounding.

## [0.49.0] - 2026-10-13

### Added
- Settlement probabilities from a spatial absorbing Markov chain (the samc package's model, landschaft's
  settlement_probabilities and HabitatGraph.from_samc): `conn.settlement_probabilities` (a random walk over a
  resistance, absorbed by an absorption raster or from the dispersal distance; the probability of settling
  between nodes over their boundary vertices, summed or averaged) and `conn.graph_from_samc`. landschaft's C++
  for the chain, Eigen's sparse LU for its inverse (scipy's SuperLU in landschaft: the same to rounding).
- `conn.protconn_raster`: ProtConn of a raster of protected areas (its patches at their centroids; the
  region's features rasterized or a raster of the region; transboundary patches, the region's parts). The same
  as landschaft's, to the bit. The `conn` module is all in Terraflux.

## [0.48.0] - 2026-10-13

### Added
- Saura's ProtConn (Makurhini's MK_ProtConn, landschaft's protconn): `conn.protconn` (the protected areas
  clipped to a region and dissolved, transboundary ones around the nodes or the region, the region's parts
  with `bound`, edge or least-cost distances, a single protected area; ECA, PC, Prot, ProtConn, ProtUnconn,
  RelConn and ProtConn's fractions; every protected area's dProt, dProtConn and varProtConn with `delta`) and
  `conn.protconn_regions`. Grids over a region: `conn.make_grid` (sf's st_make_grid hexagons or squares,
  clipped or wholly inside), `conn.connectivity_grid` (ProtConn or PC per cell, Makurhini's MK_Connect_grid)
  and `conn.eca_change_grid` (the change of ECA per cell between times, MK_dECA_grid). The same as
  landschaft's, to the bit.

## [0.47.0] - 2026-10-13

### Added
- Habitat graphs of vector features (landschaft's node_table, distance_table and HabitatGraph.from_vector):
  `conn.nodes` and `conn.distances` take features too (GDAL's areas of the polygons, an attribute column or
  values, weighted or not; the distances between centroids, the largest part's or GEOS's point on the surface
  when it falls outside, between edges with GEOS's distance, or least-cost and commute times over a
  resistance), `conn.graph_from_vector`, `conn.to_vector` (a node table joined onto the features) and
  `conn.focal_nodes` (Makurhini's MK_Focal_nodes: each node's importance in the graph of the nodes within a
  search buffer of it). GEOS through GDAL (`terraflux_gdal::Geos`, `difference`, `make_valid`). The same as
  landschaft's, to the bit.

### Fixed
- A lint of 0.46.0 (`conn::graph::Readings`).

## [0.46.0] - 2026-10-13

### Added
- `conn.eca_change`: the change of the equivalent connected area from habitat graph to habitat graph (a series
  of them, e.g. years; Makurhini's MK_dECA, landschaft's eca_change): the habitat's area and ECA of PC or IIC
  per distance, their changes, their ratio and how they compare. The same as landschaft's, to the bit.

## [0.45.0] - 2026-10-13

### Added
- Habitat graphs of raster patches (landschaft's node_table, distance_table and HabitatGraph.from_raster, as
  Makurhini's nodesfile and distancefile): `conn.nodes` (each patch's area in a unit, or an attribute, weighted
  or not; the patches of habitat classes with 8 or 4 neighbours, or a raster of patch ids), `conn.distances`
  (between centroids, edges, least-cost over a resistance as gdistance with Makurhini's or the mean
  conductance, or commute times; the straight distance for the pairs a resistance does not join),
  `conn.graph_from_raster` and `conn.to_map` (a node table's column back on the patches). The same as
  landschaft's, to the bit.

## [0.44.0] - 2026-10-13

### Added
- Habitat graphs (landschaft's HabitatGraph, a new `HabitatGraph` type): `conn.graph` from a table of nodes
  and a table of pairs or an n x n matrix (distances, probabilities or links; Makurhini's conventions or
  Conefor's; nodes to restore), `conn.read_conefor` and `conn.write_conefor` (Conefor Sensinode's files),
  `conn.overall` (NC, NL, H, CCP, LCP, IIC, F, AWF, PC, their equivalent connectivity and fractions),
  `conn.node_importance` (dI and its intra, flux and connector fractions, BC, BC(IIC), BC(PC); the gain of each
  node to restore, Makurhini's dIICres/dPCres or Conefor's d*add), `conn.link_importance`, `conn.link_change`,
  `conn.components` and `conn.centrality` (Makurhini's MK_RMCentrality, without igraph's walktrap
  communities). The same as landschaft's, to the bit, but for what it computes with numpy's BLAS, SIMD
  exponentials or LAPACK (the eigenvector centrality, the strengths on probabilities, the gains of nodes
  to restore): those to rounding.

## [0.43.0] - 2026-10-13

### Added
- The rest of landschaft's neutral landscape models (its port of NLMR): `nlm.random`, `nlm.percolation`,
  `nlm.planar_gradient` and `nlm.edge_gradient` (a direction drawn with the seed when none is given),
  `nlm.distance_gradient` (terra's haversine or GDAL's proximity as NLMR, or exact distances), `nlm.curds`
  (wheyed too), `nlm.mpd` (NLMR's grid or the size asked for, on a torus too), `nlm.neigh`,
  `nlm.random_rectangular_cluster`, `nlm.mosaic_field` (every step's sum and the infinite field too),
  `nlm.mosaic_tess` and `nlm.mosaic_gibbs` (each germ with its own value: NLMR's order of GEOS's Voronoi cells is
  not reproduced), and the utilities `nlm.classify` (by weights, a real landscape's shares or Jenks's natural
  breaks), `nlm.binarize`, `nlm.merge` and `nlm.rescale`. The same as landschaft's, to the bit.

### Changed
- `nlm.gaussian_field` and `nlm.fbm` rescale with NLMR's util_rescale (all 0 when the minimum and maximum are
  all.equal), as landschaft.

## [0.42.1] - 2026-10-13

### Fixed
- GDAL no longer writes its `.properties` index next to the gzipped rasters it reads (`/vsigzip/`): reading
  Circuitscape's gzipped test grids left the landschaft submodule modified.

## [0.42.0] - 2026-10-13

### Added
- Circuitscape's and Omniscape's INI files (landschaft's circuitscape_ini and omniscape_ini):
  `circuit.circuitscape_ini` runs a Circuitscape configuration on rasters (advanced, pairwise, one-to-all and
  all-to-one, with its polygons, mask, reclassification, included pairs and variable source strengths; ASCII
  grids read in double, gzipped ones, GeoTIFFs and text lists; paths from the working directory or next to the
  INI) and writes its outputs as Circuitscape names them (ASCII grids with C++'s shortest round trip, or
  GeoTIFF; the resistances and their three columns; the configuration), the pairs' and points' voltage and
  current maps too; `circuit.omniscape_ini` runs an Omniscape configuration (every option of Omniscape 0.7,
  conditions, reclass tables with missing values, single precision) and writes its project folder. The same as
  landschaft's, to the bit, on Circuitscape's fifty-one raster test cases and landschaft's Omniscape
  configurations, and the files written byte for byte.

## [0.41.0] - 2026-10-13

### Added
- Circuitscape's network mode and included pairs (landschaft's circuit module): `circuit.network_advanced`
  (currents and grounds on a graph of edges: node voltages, node and branch currents) and
  `circuit.network_pairwise` (the effective resistances between focal nodes, with each pair's voltages and
  currents and their sums), the graphs, focal nodes and values read as Circuitscape reads its text files; and
  Circuitscape's include / exclude pairs files (lists and min/max matrices) for `circuit.pairwise`,
  `circuit.one_to_all` and the networks, focal regions with included pairs rebuilding the graph per point. The
  same as landschaft's, to the bit, on Circuitscape's network test cases and the pairs cases made here.

## [0.40.0] - 2026-10-13

### Added
- Batch processing (landschaft's batch, GuidosToolbox's Batch Process): `batch.run` runs any operator on the
  rasters of a folder, a glob pattern or a list, in parallel, one raster per worker, and writes what it gives
  named after the input and the operator (maps as GeoTIFF, tables as CSV, texts) with a JSON per input (the
  parameters, the files, the numbers, the time); inputs already done are skipped unless overwrite, errors are
  reported per file, and MSPA keeps GuidosToolbox's naming.

## [0.39.0] - 2026-10-13

### Added
- Propagation of uncertainty (landschaft's uncertainty module, the tools plangea uses), a subject of its own
  in the builder: `uncertainty.sum_sd` (the standard deviation of a total of uncertain values: quadratic,
  linear, or with a correlation), `uncertainty.product_sd` (Goodman's variance of a product, cell by cell or of
  the total), `uncertainty.interval` (the normal confidence interval, cephes' ndtri as scipy's norm.ppf),
  `uncertainty.delta` (the delta method over an expression of maps, numbers and uncertain parameters, map.calc's
  expressions; central or forward differences) and `uncertainty.monte_carlo` (draws of the parameters with
  numpy's generator: its ziggurat normal, PCG64 and seed; the mean, standard deviation and quantiles as numpy
  computes them). The same as landschaft's, to the bit (`tools/make_uncertainty_reference.py`).

## [0.38.0] - 2026-10-13

### Added
- Properties and label matrices of Biodiverse's analyses (landschaft's bio module): `bio.properties` (label or
  group properties of a basedata from a Biodiverse properties file; they feed the lbprop and gpprop
  calculations, and RANGE and ABUNDANCE the labels' ranges and abundances) and `bio.matrix` (a label matrix,
  full, a triangle or sparse pairs; a new type of value) for the matrix calculations of `bio.spatial`,
  `bio.randomise`, `bio.cluster` and `bio.region_grower`. `bio.randomise` moves group properties by set or by
  item (randomise_group_props_by), within subsets too. The reading, the property tables and the shuffles are
  ported from landschaft's Python. The same as landschaft's, to the bit.

## [0.37.0] - 2026-10-13

### Added
- Biodiverse's cluster analysis and region grower (landschaft's bio module): `bio.cluster` (groups joined by
  the dissimilarity of their labels: Sorenson, Jaccard and S2 directly, Bray-Curtis, the phylogenetic ones and
  the rest pair by pair; every linkage, link_recalculate through a callback from landschaft's C++ to Rust;
  several conditions, a definition query, tie breakers, singletons) and `bio.region_grower` (regions grown by
  an index of the merged groups). The dendrogram as a tree, its nodes with Biodiverse's values, the
  calculations of each node in Biodiverse's hierarchical mode and, with k, the dendrogram cut into k clusters as
  a table and a map. The tree's finishing (lengths, zero-length branches raised, roots joined, ladderised,
  terminals numbered) and the cut are ported from landschaft's Python. The same as landschaft's, to the bit, in
  thirteen cases.

## [0.36.0] - 2026-10-13

### Added
- Randomisations of Biodiverse's analyses (landschaft's bio module): `bio.randomise` runs an analysis, then
  randomises the basedata again and again with Biodiverse's Mersenne Twister (rand_structured and its spatial
  forms with their allocation orders, backtracking and seed groups; the independent swaps, modified, and
  curveball; rand_csr_by_group; rand_nochange), within subsets of groups or a definition query, with labels kept
  in place and tree terminal names shuffled, and compares each index: the counters (C, P, Q, SUMX, SUMXX, T),
  p-ranks, z-scores and CANAPE, as tables and maps. The subsets, the labels kept in place, the merging, the tree
  shuffle and the counters are ported from landschaft's Python; the algorithms are its C++. The same as
  landschaft's, to the bit, in fourteen cases, scalar and list indices.

## [0.35.0] - 2026-10-13

### Added
- Phylogenetic trees and indices (landschaft's bio module): a new type of value, the tree, and `bio.tree` (a
  Newick or Nexus file read as Biodiverse reads it: unnamed nodes `0___`..., `__dup` names, the Translate table,
  all-zero lengths made 1, a forest given a root; the reader ported from landschaft's Python); `bio.spatial`
  takes a tree for the phylogenetic indices (PD, PE, phylo ABC, Sorenson and Jaccard, RPD and RPE, AED, NRI and
  NTI, VPD, the clade contributions and losses...: Biodiverse's trimming to the labels and the totals ported
  too, PhyloCom's random tips from a seed) and gives the list indices as JSON. The same as landschaft's, to the
  bit, on its test trees (trimmed, polytomies, names that do not match).

## [0.34.0] - 2026-10-13

### Added
- Biodiversity analyses (landschaft's bio module, its port of Biodiverse), a subject of their own in the builder
  and a new type of value, the basedata (Biodiverse's groups x labels sample counts with the groups'
  coordinates): `bio.table` (a delimited text file read like Biodiverse's import: group columns snapped to cells,
  kept as written or as text axes, labels joined with ':', counts summed; separator and quote guessed) and
  `bio.points` (point records read like its feature import); `bio.spatial` (Biodiverse's spatial analysis:
  every index of landschaft's C++ engine without trees, matrices or properties, over the neighbourhoods of
  spatial conditions in Biodiverse's Perl, one or two sets, with a definition query; a table of the indices
  of each group and their maps when the groups are cells). The Perl of the conditions (its tokenizer and Pratt
  parser), their compilation, Biodiverse's result types and the recycling of neighbour lists are ported from
  landschaft's Python to Rust. The same as landschaft's, to the bit, scalar and list indices, on its example
  sites under twenty conditions (`tools/make_bio_reference.py`).

### Fixed
- The test of RSP's method 2 allows the 1e-11 that landschaft's own runs differ by on asymmetric transitions
  (Eigen's SparseLU), where 1e-12 failed now and then.

## [0.33.0] - 2026-10-13

### Added
- The rest of landschaft's grid distances: `cost.barriers` (the connections across a boundary: per break of
  continuous values, or per pair of categories in gdistance's order, one or both ways) and `cost.areas` (per
  category), sets of transitions in one stack (the layout's `count`); `cost.mahal` (gdistance's Mahalanobis
  transition of a stack of maps: R's cov, solve, %*% and mean, LAPACK's recursive LU over R's BLAS rewritten
  in Rust); and `cost.path_inc` (the overlap and non-overlap of the routes from a common origin, random walks or
  randomised shortest paths, weighted by the costs, a transition or a set). The same as landschaft's, to the
  bit.

## [0.32.0] - 2026-10-13

### Added
- Random walks over grid distances (landschaft's cost module, gdistance's through its C++):
  `cost.commute_distance` (the expected commute distances, each separate part of the graph with its own volume),
  `cost.passage` (the net passages of a random walk, gdistance's grounded cell or the exact currents; with theta,
  randomised shortest paths, net or total), `cost.rsp_distance` (methods 1 and 2, net or total). And the
  matrix's own algebra: `cost.normalize` (row, col, symm), `cost.sum_reciprocal`, `cost.combine` (sum,
  difference, product in the common layout), `cost.math` (R's sqrt, exp, log and powers, factors and
  reciprocals on the connections) and `cost.to_map` (gdistance's raster(): sums and means of rows and columns).
  The same as landschaft's, to the bit, but for RSP's method 2, whose Eigen SparseLU changes its last bits from
  run to run in landschaft itself (within 1e-12).

## [0.31.0] - 2026-10-13

### Added
- Grid distances (landschaft's cost module, the R package gdistance through its C++), a subject of their own in
  the builder: `cost.transition` (the connections of every cell with its 4, 8 or 16 neighbours, a built-in
  function of the two cells' values: mean, min, max, sum, prod, from, to, diff, harmonic, geometric; symmetric or
  not; its reciprocal for a conductance of a resistance), kept as a stack of a layer per direction whose grid
  carries the layout; `cost.geo_correction` (types c and r, scaled or not; great-circle distances on
  longitude/latitude grids), `cost.acc_cost` (from points or a mask), `cost.distance` (among points or from
  points to others; a table) and `cost.shortest_path` (lines through the cells' centres and a map of the share of
  the paths through each cell). Global longitude/latitude grids wrap around as gdistance's. The same as
  landschaft's, to the bit, on its example rasters and a global grid (`tools/make_cost_reference.py`).

## [0.30.0] - 2026-10-13

### Added
- The terrain of surfaces (landschaft's surface module: spatialEco's, terra's and geodiv's numbers through its
  C++): `surface.terrain` (slope and aspect, 8 or 4 neighbours), `surface.tpi` (a rectangle of cells or a circle of
  a radius, normalised or not), `surface.tri`, `surface.vrm`, `surface.srr`, `surface.dissection`,
  `surface.curvature` (planform, profile, total, McNab's, Bolstad's), `surface.spherical_sd`, `surface.hsp`,
  `surface.trasp`, `surface.hli` (McCune & Keon's three equations, the latitude of the map's centre), and geodiv's
  `surface.fit_plane` (a polynomial trend of order 0-6 or the best, and the surface without it), `surface.peaks`
  (peaks or valleys), `surface.bearing` (the Abbott-Firestone curve) and `surface.aacf` (the areal
  autocorrelation). The same as landschaft's, to the bit, on its example surfaces.

## [0.29.0] - 2026-10-13

### Added
- Surface metrics (landschaft's surface module, ISO 25178 as the R package geodiv computes them, geodiv's bits),
  a subject of their own in the builder: `surface.metrics` (the 27 metrics of a surface: amplitude, hybrid,
  local features, the bearing area curve, spatial and fractal; a table) and `surface.window` (the metrics in a
  square or circular moving window around every cell; a map each), geographic grids measured as such
  (`terraflux_gdal::is_geographic`). The same as landschaft's, to the bit, on its example surfaces
  (`tools/make_surface_reference.py`).

## [0.28.0] - 2026-10-13

### Added
- Circuitscape's raster modes with focal points: `circuit.pairwise` (the effective resistance between every pair
  of focal points, focal regions of an id's cells, the cumulative and largest current maps) and
  `circuit.one_to_all` (one-to-all and all-to-one: the voltage over the strength at each point, strengths by
  position as Circuitscape, the cumulative and largest currents, the focal nodes' currents at 0), with
  short-circuit polygons, a mask, averaged resistances and a reclassification; the same as landschaft's, to the
  bit (`tools/make_circuit_reference.py`).

## [0.27.0] - 2026-10-13

### Added
- Connectivity by circuits (landschaft's C++ of Circuitscape 6.1 and Omniscape 0.7), a subject of its own in the
  builder: `circuit.omniscape` (Omniscape's moving-window circuits: the cumulative current, the flow potential and
  the normalised current; sources from the resistance or given, their threshold and cutoff, a reclassification,
  conditional connectivity with future conditions, single precision, the no-data mask) and `circuit.advanced`
  (Circuitscape's advanced mode on a raster: currents and voltages from sources and grounds). The same maps as
  landschaft's, to the bit, on the cases of its references (themselves checked against Omniscape.jl and
  Circuitscape.jl) (`tools/make_circuit_reference.py`).

## [0.26.0] - 2026-10-13

### Added
- The weights of evidence in the viewer, editable as in Dinamica EGO: a transition and a variable chosen, the W+
  of its ranges in bars (the significant in the accent) and the ranges in a table (cells, changed, W+, contrast):
  two ranges merged or one split, a weight set by hand (or back to the fitted one), the fit run again. `woe.fit`
  takes `ranges` per transition (`"1->2": {variable: limits}`, as landschaft's (from, to) keys) and `weights`
  set by hand (`{"1->2": {variable: [W+ or null per range]}}`), in the probabilities too; the tiled engine takes
  the weights and refuses ranges of one transition.
- Legends: `raster.legend` gives the names and colours of a map's classes, carried with the map in its grid's
  metadata (`terraflux_legend`) to the maps made from it with its classes and to the GeoTIFF it is written to;
  `raster.reclassify` leaves them behind. The viewer colours and names the classes so, and edits them ("Edit the
  legend": a colour and a name per class, applied as a `raster.legend` node after the node shown, or that node
  changed).

### Fixed
- A node selected by what the viewer shows (the legend's node, the WoE's...) is shown at once (the viewer no
  longer put back the node it had shown).

## [0.25.0] - 2026-10-13

### Added
- The analysis of land change, landschaft's `changes` (OpenLand's and diffeR's numbers, to the bit):
  `change.crosstab` (the transition matrix of two maps), `change.transitions` (between consecutive dates and from
  the first to the last, the area of each class at each date, the matrix), `change.intensity` (Aldwaik & Pontius's
  intensity analysis at the interval, category and transition levels, with its stationarity; km2 or cells),
  `change.components` (gain, persistence and loss, and the quantity, exchange and shift components of each class
  and overall, diffeR's diffTablej and overallDiff), `change.count` (how many times each cell changed) and
  `change.suggest` (the transitions worth modelling: the largest first, those of 1 % of the change at least).
  Tested against landschaft on Pie (three dates) and Sibuyan (`tools/make_change_reference.py`).
- The assistant of the analysis of changes: in the Prepare pane, a series of maps of classes shows the
  transitions worth modelling with their shares, and one click adds the analysis (a node each, with a note).

## [0.24.0] - 2026-10-13

### Added
- The viewer's Metadata (beside Save output): what rasters, maps or vector features are, in a window: the files
  and their reader, the layers, the data type and no-data value, the coordinate system (and its WKT), the cells
  (their number and size), the extent and origin; of features their number, kinds, attributes and bounds; copied
  as text.

### Changed
- The Operators menu of the toolbar is a classic menu (as Photoshop's): the favourites by colour, then a submenu
  per subject, its groups a level below; the drawer at the left edge keeps the search and the collections.
- ctrl+R selects the node's whole name: what is typed replaces it.

## [0.23.0] - 2026-10-13

### Added
- Maps stacked in the viewer: other maps of the run drawn over the one shown ("Maps over it", each as opaque as
  asked, placed by its coordinates whatever its cells), and in the model: `view.plot` takes `features` (vectors)
  and `under` (another plot's shown, drawn beneath it), plots stacking as deep as wanted (`builder::plot_stack`).
- A cell clicked on a stack, a series or rasters of many layers: its values along the layers in a chart beside
  the map (by the layers' names when they are years), a click on the chart shows that layer.
- Close in, the cell under the pointer outlined in white; the cell clicked in the accent.
- The zoom stops out at about the world when the map has a coordinate system (at 32 points across without one).
- B over a map puts the satellite's imagery under it, or takes it away.
- A map opened goes in a tab beside the viewer, wherever the viewer is (a window of its own too).
- Favourites in the operators' library, as Ableton Live's collections: seven colours, an operator put in them by
  a right click, the squares of its colours beside its name; a colour chosen shows its operators.
- Settings (in the toolbar): the theme (Default, Immaterial, Riparian, Angst Robot, Twenty Four Carat), the
  text's size, the interface's zoom, the threads the functions use (from the next start); kept in the user's
  application data (`settings.json`, or `TERRAFLUX_SETTINGS`).

### Changed
- A node added after the one selected takes its value in its first input that fits it when it must have none
  (a plot's map).

### Fixed
- Texts of some operators had runs of spaces in them (the doc of `view.plot`, `metrics.window`,
  `metrics.signatures`, `demand.regional`, `table.of`, `submodel`...).

## [0.22.0] - 2026-10-13

### Added
- Basemaps fetched and drawn anew: the web tiles of the service fetched by a pool of twelve threads shared by
  every view (each tile once, kept on disk and decoded in memory), and drawn by the GPU as textures on meshes whose
  vertices are carried to the map's coordinate system once per tile: nothing is warped on the processor, a tile
  shows as it comes, two levels coarser first, the coarser ones kept under those still to come. GDAL's HTTP
  (`terraflux_gdal::fetch`), image decoding (`decode_image`, palettes too) and coordinate transformations
  (`Transform`).
- The operators in a drawer at the window's left edge (out while the pointer is at the edge or over it) and in
  the toolbar (Operators); the node's and the Prepare panes at the left.
- Tabs dragged out of the dock become windows of their own (on another screen too); ctrl+W closes the tab in
  front of the window it is pressed in, and each window takes its own keys (F5, ctrl+S, ctrl+O).
- ctrl+O opens a raster or vector features (a node that reads the file, by its extension); vector.read and
  vector.write choose their file in a dialog, and their paths are kept relative to the model as the rasters'.
- Backspace deletes the node selected (as delete; the key macOS deletes with).
- The vector layers' line width (the points' size) beside their fill.
- Charts zoom with the wheel around the point under the pointer, as maps.

### Changed
- The attributes' table of vectors is only the node's table output (no longer repeated beside the features).

## [0.21.0] - 2026-10-13

### Added
- The Prepare pane (beside the node's): the model's reference grid (the land use map's suggested, or the node
  selected), the data of the node selected next to it (coordinate system, cells, extent and what differs; classes,
  values or counts) and the steps a click away, each a node of an operator already there with a note saying why:
  onto the grid (`raster.match`, resampled by what the values are), reprojected (`raster.reproject`), the slope
  (`raster.terrain`), the distance to a class (`dynamic.distance_to`); of vectors the distance to them
  (`vector.distance`), burnt into the grid (`vector.rasterize`, by an attribute or not), the statistics of the
  grid's map in each polygon (`vector.zonal`); every raster read off the grid put on it at once. The reference
  grid of a model opened is the one its steps put maps on (`terraflux_app::prepare`).

## [0.20.0] - 2026-10-13

### Added
- Out of core, the rest of the landscape metrics: `metrics.window` by strips of rows with the window's half side
  above and below (every cell's window whole, so the same maps to the bit), of rasters on disk into a GeoTIFF of a
  band per metric (`path`); `metrics.sample` reads each plot's window alone from the rasters on disk (and the map's
  classes for `all_classes` by blocks); `metrics.series` and `metrics.signatures` take rasters on disk, a layer
  per date, whole maps by blocks of rows and zones window by window. `block_rows` on all of them, and rasters on
  disk of more than 250 million cells always; the same tables as in memory, to the bit. `vector.grid_zones` reads
  only the grid of its map.
- Vectors over rasters larger than memory: `vector.zonal` by strips of rows (the zones that fit a strip by
  landschaft's zonal statistics on it, a zone too large read run after run and summed as landschaft sums it, its
  order statistics from its values or, for whole numbers, their counts), `vector.extract` from the strips of rows
  that hold points (landschaft's extract of a block, a row around for bilinear), `vector.rasterize` into a GeoTIFF
  strip by strip (`path`; polygons by their runs, lines by GDAL in the grid's pixels, points by their cell). The
  same values as in memory, to the bit, but GDAL's lines through cells' corners with `all_touched`.
- `RowsWriter` writes several bands side by side (`create_bands`, `push_bands`).

### Changed
- `metrics.window`'s maps, `vector.rasterize`'s map and the maps of `metrics.series`, `metrics.signatures`,
  `metrics.sample`, `vector.zonal` and `vector.extract` are of the type `Raster` (a map, a stack or rasters on
  disk): any map connects as before.

## [0.19.0] - 2026-10-13

### Added
- Vector layers over the maps in the viewer ("Vectors over the map": any vector of the run, reprojected to the
  map, in a colour or coloured by an attribute, polygons filled as asked), and vectors alone drawn the same way
  over a grid that spans them (with a basemap under them). The layers are drawn as tiles threads rasterize
  (tiny-skia, antialiased) from an R-tree of the features made once per layer on a thread of its own, vertices
  closer than a pixel skipped and features smaller than one drawn as dots: the screen only draws textures, so
  layers of millions of features move as smoothly as a map (a quarter of a million polygons indexed in 60 ms and
  the whole drawn in 90 ms, off the screen's thread). The feature under the pointer is found by the tree and
  named with its attributes.

- `vector.extract`: the values of a map or a stack at points, as landschaft's extract (the cell or bilinear, to
  the bit), or the statistics of the cells within a radius of them.
- `vector.buffer`, `vector.clip` and `vector.dissolve`: buffers of features (or one of them all), features cut to a
  mask's polygons, features merged all or by an attribute, by GEOS through GDAL; the same features and areas as
  geopandas (`tools/make_vector_ops_reference.py`).

- The colours of a map chosen by hand in the viewer (Colours: viridis, centred on 0, terrain, grey, red to green,
  rank, each on a scale of logarithms or not), remembered for that output.
- Circuit currents (current maps, flow potential) are shown on a scale of logarithms, as landschaft's plot.
- What a map is travels with it: raster.write writes the product of GuidosToolbox a map is (MSPA, FAD, SPA...) as
  `landschaft_product` in the GeoTIFF's metadata, as landschaft does, and rasters on disk that say so are shown
  as that product whatever their name (`product` sets it by hand); MSPA by tiles writes it too. The dataset's
  metadata items are read and written with the GeoTIFF (`GeoTags::metadata`).
- Models saved by landschaft with a potential Terraflux has no form for (a scikit-learn estimator, a
  multi-criteria evaluation, MOLUSCE's weights) run as `ls.simulate` runs them: landschaft writes their
  predictions on the starting map (`stack.classes`). A test case of the bridge (`tests/data/bridge/estimator`).
- Regional demands (`demand.regional`, a new type `Regional`): the map of region ids and each region's demand
  (cells per class and step, or a Markov chain). `lulcc.step` allocates region by region in the order of their
  ids with the one generator, `plus.demand` makes CARS's table of each region from its own cells and `cars.step`
  meets them all, `dinamica.step` runs each region with its own rates, its cells only and a seed of its own;
  cells of no region keep their class. As landschaft's `Regional`, bit for bit (three test cases of the bridge).
- `dynamic.distance_to` takes a transition of conductances (`cost`, a layer per direction, and its `layout`):
  the accumulated cost from the nearest cell of the classes (gdistance's accCost, through landschaft's
  `costcore`, now linked), as landschaft's `distance_to(..., cost=...)`.
- MOLUSCE's simulator: `rng.numpy` (numpy's legacy generator, as MOLUSCE draws), `molusce.total` (the total
  weight of each transition from MOLUSCE's weights of evidence and the drivers' categories) and `molusce.step`
  (the transition of largest weight, or the class of largest potential, with its confidence; the cells of each
  observed transition with the highest confidence change), through landschaft's C++ (`sim/molusce.cpp`). Models
  saved by landschaft with MOLUSCE run bit for bit (three test cases of the bridge).
- CLUMondo: `clumondo.ages` (the ages at the start: a map, or CLUMondo's random ages from the seed) and
  `clumondo.step` (one year: the suitability from logistic regressions in floats or given maps, neighbourhood
  functions, location preferences, then the allocation on the demand for goods and services, through
  landschaft's C++ of CLUMondo, with the 2017 executable's extensions and the land system roles' caps and pair
  resistances). Its `stop` ends the loop at a year without solution, as CLUMondo.
- FUTURES: `futures.simulation` runs r.futures.simulation as landschaft's `PatchGrowing` (subregions, potential
  subregions, patch libraries, development pressure, weights, zoning and its effects, incentives, restrictions)
  with its flood submodel (depths or flood probabilities and HAND, damage curves, response, adaptation,
  redistribution), every step in one run of landschaft's C++. A test case of the bridge.
- `control.repeat` takes `until` in its body: a reference to a yes-or-no value; when it is true at the end of a
  step the loop ends there, that step neither collected nor carried.

- The builder without the mouse: with the model's canvas in focus and the pointer over it the arrows go to the node
  beside the one selected (the first, top left, when none is; the view follows), enter shows its output in the
  viewer; with "/" to add operators, a model is built from the keyboard.
- Templates of Dinamica EGO's classic model and of CLUE-S (lulcc's), on two dates of land use drawn by chance,
  calibrated and run three steps ahead as soon as they open.
- Templates of one's own: a model saved as a template (Templates, "Save this model as a template"; its files'
  paths made absolute) and listed under Yours, opened as a new model, removed with its x.

### Changed
- A continuous map's ramp spans its values' 2nd to 98th percentiles (98th of the absolute values centred on 0),
  as landschaft's plot, no longer their least and most: a few outliers no longer wash it out.
- `vector.zonal`'s statistics are worked out by a function the extraction in buffers shares.
- The vector panel works out a layer's table, counts and grid once, not at every frame.
- `dinamica::step` takes `Rates::One(&markov)` or `Rates::Regional(&regional)`; `lulcc::step` takes
  `Demands::One` or `Demands::Regional`, `plus::step` `CarsDemand::One` or `CarsDemand::Regional`. The `demand`
  inputs of `lulcc.step` and `cars.step` and `rates` of `dinamica.step` are optional (or `regional`).

## [0.18.0] - 2026-10-13

### Added
- `vector.join`: a table's columns as attributes of features, by a key (the feature's number or an attribute):
  the clusters of zones (metrics.cluster) on their squares (vector.grid_zones), burnt into a map of the groups by
  vector.rasterize.
- Charts of tables: box plots of the columns by group (landschaft's plot_cluster_metrics), and metrics.pca's
  loadings as the circle of correlations (every metric an arrow to its correlations with two components).
- `metrics.patch` of rasters on disk by blocks writes the patches to a GeoTIFF block by block (`patches_path`),
  never whole in memory; the same patches as the map in memory.

### Changed
- MSPA by tiles writes its GeoTIFF through the same writer of rows in order (`files::RowsWriter`).

## [0.17.0] - 2026-10-13

### Added
- Spatial signatures, as pylandstats' SpatialSignatureAnalysis in landschaft (`tools/make_signature_reference.py`):
  - `metrics.signatures`: the table of landscapes described by their metrics, a row per zone of a map (polygons,
    buffers around points, a grid of squares) or per layer of a stack, a column per landscape metric and per
    class metric and class (`pland_11`...), to the bit;
  - `metrics.pca` (the principal components of the z-scores: scores, loadings and the share of the variance),
    `metrics.nmf` (non-negative factors: NNDSVDa start, HALS sweeps), `metrics.cluster` (k-means from numpy's
    generator, PCG64 and its SeedSequence, so the same starts; or scipy's linkages: ward, average, complete,
    single, centroid, median, weighted, cut as fcluster's maxclust) and `metrics.clustergram` (the clusters for a
    range of k and their means, on the first principal axis or not). The same clusters as landschaft, label by
    label; the components, factors and means within 1e-11 of numpy's LAPACK.
- The landscape and class metrics of a signatures node are chosen in the metrics' list, as those of the other
  metric nodes.
- Landscape metrics of maps larger than memory: `metrics.patch`, `metrics.class` and `metrics.landscape` read a
  map by blocks of rows (`block_rows`, a multiple of 64) through landschaft's out-of-core landscape (its C++
  `TiledLandscape`: the patches joined across the seams), the same tables to the bit as the map in memory; rasters
  on disk of more than 250 million cells always are. prox, simi, econ, connect (and dcore by FRAGSTATS' convention)
  need the whole map, as in landschaft; chosen by default or by type they are left out.
- `landscape.mspa_tiled`: GuidosToolbox's MSPA Tiling, as landschaft's mspa_tiled, the same maps
  (`tools/make_mspa_tiled_reference.py`): MSPA by overlapping tiles with GTB's shortcuts, of a map in memory, or of
  rasters on disk read by windows and written tile by tile to a GeoTIFF (GTB's name next to the input by default).

- The Windows installer and a portable zip (`tools/make-installer.ps1`, in `target/installer`): Terraflux in
  Program Files with a Start menu shortcut and the PATH, GDAL in the folder `gdal`, the Visual C++ and OpenMP
  runtimes next to the program (a clean Windows has neither). The MSI is built by WiX 5, a .NET tool of the
  repository (`.config/dotnet-tools.json`).

- Tables drawn as charts of several metrics at once in the viewer: bars, lines or points of some columns (a panel
  each) along another, the rows split by a column (a class, a zone, a cluster) in colours, as landschaft's
  plot_metric and plot_cluster_metrics; the clustergram of `metrics.clustergram`'s means (landschaft's
  plot_clustergram); a correlation matrix (`metrics.correlation`) as a heatmap.

- Planning units of polygons with management zones in `prioritize.optimize` (landschaft's polygon_zone_units): a
  cost column per zone, every zone's own features or the base ones with contributions, summed over the polygons
  exactly; prioritizr's compile() of its case to the bit, and its optimum.
- `prioritize.h3_units`: H3 hexagons as planning units, as landschaft's h3_units (h3o, a port of Uber's H3): the
  hexagons of a resolution over a cost map, in its coordinate system, with the exact sums of the costs under them;
  the same hexagons and costs to the bit (`tools/make_h3_reference.py`).

### Changed
- A table's chart (formerly the bars of one column) is chosen in the viewer by its kind, its columns, the column
  along x and the one that splits the rows; a series suggests the date along x and the class as groups.
- The map input of `metrics.patch`, `metrics.class` and `metrics.landscape` takes rasters on disk as they are
  (read by blocks when large or asked to); the maps of patches and metrics are given for maps in memory only.

## [0.16.0] - 2026-10-12

### Added
- Settings of a few values are chosen in a list (the operators' settings say their values, `PortSpec::choices`;
  57 of them), yes-or-no settings too, with their default; the metrics of a landscape metrics node in a list of
  their own: all (the default), by type, or one by one.
- Tab goes to the model's tab and Tab again back to the tab it came from; ctrl+W closes the tab in focus; ctrl+R
  renames the node selected (a box over it).
- A connection is taken off by dragging it away from its input (onto another input: moved) or by a right click
  on the input.
- The viewer writes the coordinates under the pointer in its corner (and the cell's row, column and value), for
  maps and vector features.

### Changed
- The library opens with its subjects closed but IO (reading and writing rasters and vectors, a subject of its
  own) and Plot (formerly Visualization); "Data & configuration" is "Data preparation".
- The model as text is a button of the model's tab (Text), no longer a tab of its own.

## [0.15.0] - 2026-10-12

### Added
- `metrics.sample`: landscape metrics of sample plots, as landscapemetrics' sample_lsm and scale_sample: square or
  circular buffers (R's 100 vertices, its x87 sines) of one or several sizes around points, or polygons; each plot
  cut as terra's crop(mask = TRUE) (GDAL's all-touched cells), the share of it inside the map (geodesic areas
  when the plots' coordinate system is known, by GDAL 3.9's GeographicLib), every class of the map with
  `all_classes`; the plots as features. The 9 cases of landscapemetrics that landschaft keeps, the same rows and
  bits.
- `vector.grid_zones`: square zones tiling a map (landschaft's grid_zones).
- `metrics.series`: landscape and class metrics along a series of maps (a layer per date) as pylandstats'
  SpatioTemporalAnalysis in landschaft, of the whole maps, of zones, of buffers or of a grid; absent classes 0 for
  the counts and areas (`fillna`). The same values as landschaft (`tools/make_lsm_reference.py`).
- `metrics.correlation`: R's cor (Pearson, Spearman, Kendall) between the metrics of a table, as
  landscapemetrics' calculate_correlation, to the bit.

### Fixed
- The EPSG code of a GeoTIFF whose projection is its own (user-defined) was that of its datum (e.g. 4326 for an
  Albers on WGS 84); it has none now.

## [0.14.0] - 2026-10-12

### Added
- The rest of GuidosToolbox, by landschaft's C++ of GTB, the same maps and reports as landschaft
  (`tools/make_gtb_reference.py`):
  - the habitat network of an MSPA map (`networks.gtn_components`, `networks.gtn_importance`: GTN's components,
    ECA and coherence, the dPC importance of every link and stepping-stone node; `networks.conefor`: Conefor
    Sensinode's node and link tables), with GTB's colours and reports;
  - the Restoration Planner: the habitat's status (`restoration.status`: ECA, coherence, the restoration potential
    and RAC), a resistance from the distance to the habitat (`restoration.resistance`), the cost distance
    (`restoration.cost_distance`), the least-cost path and the corridor between two objects with its gain in ECA
    and coherence (`restoration.least_cost_path`, `restoration.corridor`), the isochrones from one object or
    between two (`restoration.isochrone`);
  - the zones of influence of the objects and their proximity (`distance.influence_zones`,
    `distance.objects_proximity`, experimental as in landschaft);
  - the change of FAD at several scales between two dates (`change.fad`: the cells moved between the classes at
    every window, the change of FAD_AV and AVCON);
  - the landscape mosaic's heatmap and triangle (`fragmentation.landscape_mosaic`'s `heatmap`, `triangle` and
    `heatmap_csv`) and its change between two dates (`change.mosaic`);
  - GTB's text reports of FOS (fos.txt), SPA (spaN.txt) and Accounting (acc.txt) as a `report` output, with the
    cell size the report converts to hectares (`pixel_resolution`); Accounting's three largest objects come from
    the C++.

## [0.13.0] - 2026-10-12

### Added
- Vector features in the model: a value type (`Vector`: geometries in 2-D, attributes as numbers or texts, the
  coordinate system), read and written by OGR (`vector.read`, `vector.write`; GeoPackage, Shapefile, GeoJSON,
  FlatGeobuf, KML...; a model's vector outputs written as GeoPackage), reprojected by PROJ where they meet a map
  of another coordinate system.
- `vector.rasterize`: features burnt into a map's grid as landschaft's rasterize, the same cells: polygons by
  landschaft's C++ port of GDAL's rasterizer (centre rule or all touched), lines by GDAL, points by their cell;
  an attribute or a value, replaced or added up, a fill.
- `vector.zonal`: a map's (or every layer of a stack's) statistics in every polygon as landschaft's zonal_stats
  (count, sum, mean, min, max, range, std, var, median, majority, minority, unique, percentiles).
- `vector.exact_sum`: the coverage-weighted sums over polygons of exactextractr's exact_extract, bit for bit
  (landschaft's port of exactextract).
- `vector.distance`: the exact Euclidean distance from every cell to the features.
- Planning units of polygons in `prioritize.optimize`, `evaluate` and `calibrate` (`polygons`, `cost_column`):
  the features' amounts exactextractr's sums, the boundary penalties on the polygons' shared paths and the
  contiguity on the polygons that touch; prioritizr's models bit for bit, its boundary and adjacency matrices of
  polygons exactly (`tools/make_vector_inputs.py` writes the cases' polygons).
- The viewer draws vector features (polygons, lines, points; zoom and pan, the feature under the pointer named)
  beside their attributes, and saves them as GeoPackage, GeoJSON or Shapefile; "Open vectors..." in the builder.
- The build links landschaft's io library (zones of polygons, exact sums).

## [0.12.0] - 2026-10-12

### Added
- The minimum extinction risk objective in `prioritize.optimize` (`min_extinction_risk`, landschaft's extension of
  prioritizr): within the budget, the least sum of the features' species-area risk 1 - (held / total)^z (`z`
  one or per feature), piecewise linear through `segments` + 1 points of the curve; feature weights, every
  decision type, management zones (every feature over all its zones); the risks of the solution
  (`extinction_risk`). `prioritize.evaluate`'s `extinction_z`: the features' exact risk and their sum. landschaft's
  models bit for bit, its optima and risks (`tools/make_extinction_reference.py`); the optimum against
  enumeration.

## [0.11.0] - 2026-10-12

### Added
- Zonation's transforms in `prioritize.zonation`, through landschaft's C++ quantized data (now kept between its
  steps: `prio_data_*` in `terraflux-ls-sys`): conditions (`condition`, `renormalize`, `condition_maps`; -c),
  retention (a fraction everywhere, fractions or amounts per cell; -r), the pre-processing maps (`preprocessing`:
  richness and range-size rarity of the weight classes and groups, weighted; -b) and the features as ranked
  (`transformed`; -f).
- Zonation's connectivity transforms: single-feature smoothing with a dispersal kernel (uniform, triangular,
  negexp, gaussian; times itself; -d), interactions with resources, positive or negative, before and after the
  matrices (-i, -I), connectivity matrices (-m); Zonation's FFT convolution in blocks, in Rust.
- Zonation's impact assessment: impact sources with footprints and rings (-s) and cumulative impacts (direct
  habitat loss and pressures through response functions, -e), per subarea, source, zone and value bin; the
  features lose what the scenarios take before the ranking (`impact_groups`, `impact_bins`, `impact`,
  `cumulative_impact`, `impact_maps`; the tables `impact_zones` and `impact_cumulative`, the maps of the groups).
- Zonation's post-processing of the ranking: statistics of areas (external, a range of ranks, its separate
  8-connected areas) with the coverage of every feature, the comparison with another solution, the histogram of
  the features' coverage and the flexibility map.
- All of landschaft's Zonation cases: the 24 of the transform, connectivity, post-processing and impact groups
  now with the 41 of the core, the ranks bit for bit with the z5 binary, the transformed and pre-processing maps,
  the impact maps and the post-processing tables (word for word) too (`tools/make_zonation_inputs.py`).

## [0.10.0] - 2026-10-12

### Added
- prioritizr's systematic conservation planning (`prioritize.optimize`): the planning units of a cost raster and
  the features' amounts in them, the objectives min set, min shortfall, min largest shortfall, max utility, max
  features, max cover and min penalties, relative, absolute, loglinear and manual targets, feature weights,
  binary, proportion and semicontinuous decisions, locked units; boundary penalties (prioritizr's boundary
  matrix of the raster, an edge factor), linear penalties and constraints (a map), neighbour constraints; the model
  compiled in Rust (`terraflux_ops::milp`,
  a port of landschaft's), solved by HiGHS through landschaft's C++ (its covering and wide engines); the solution
  on the grid, a summary and the targets' coverage. The models are prioritizr 8.0.6's compile(), bit for bit, on
  the 25 raster cases landschaft keeps, and HiGHS reaches their optima (Gurobi's at gap 0)
  (`tools/make_optimize_inputs.py`).
- Planning units in tables for `prioritize.optimize`: a table of units (a cost column, the features' columns,
  locked units' columns; units without cost dropped) or Marxan's (pu, spec, rij); boundary, connectivity and
  asymmetric connectivity matrices and adjacency matrices as tables (i, j, x); linear penalties and constraints of
  a column; penalties and constraints in the order given (`spatial`); the selection per unit as a table. The
  models are prioritizr's compile(), bit for bit, on landschaft's 12 table cases.
- Portfolios of solutions for `prioritize.optimize` (`portfolio`): cuts (the best distinct solutions, prioritizr's
  no-good cut fixed), top (each to optimality), gap (within `pool_gap` of the optimum) and shuffle (the columns in
  random orders); a column per solution and the frequency of selection, a row per solution in the summary, and
  with maps a layer per solution and the frequency map. The objectives of prioritizr's (Gurobi's solution pool)
  on the cases landschaft keeps.
- `prioritize.evaluate` of table units and of `prioritize.optimize`'s selection (a table): the boundary of a
  boundary matrix, the connectivity and asymmetric connectivity of connectivity matrices (prioritizr's warnings
  when a matrix does not suit its summary), the per-unit results as a table (`units`: the solution, the rarity,
  Ferrier's, the replacement and rank importance). prioritizr's numbers bit for bit on landschaft's table case,
  and its replacement importance of a max utility problem with boundary penalties.
- Contiguity constraints for `prioritize.optimize` (`contiguity`, `feature_contiguity`: the selected units, or every
  feature's units, form one patch; the raster's 4 neighbours or an adjacency matrix): landschaft's exact
  formulation (a spanning arborescence with flows), not prioritizr's, which misses patches (it can find a worse
  one, or none); solved in three steps: a first patch from prioritizr's restriction, directed cuts separated by
  maximum flows on the linear relaxation, then the MIP with the cuts. With prioritizr's restriction it reaches
  prioritizr's optima (Gurobi's at gap 0) on landschaft's four cases; the exact model finds the best patch of a
  grid by enumeration.
- Manual bounds for `prioritize.optimize` (prioritizr's add_manual_bounded_constraints and
  add_manual_locked_constraints): tables of units and their bounds (`bounds`: pu, lower, upper) or statuses
  (`status`: pu, status), after the locked units; prioritizr's compile() bit for bit and its optima on the 7 cases
  landschaft keeps.
- The phylogenetic objectives of `prioritize.optimize`: max_phylo_div and max_phylo_end (a phylogeny in Newick,
  its tips the features; the branches met where a target below them is). prioritizr's compile() bit for bit (with
  its tip-order bug fixed when the tips come in another order) and its optima on landschaft's 4 cases.
- prioritizr's calibration of trade-offs (`prioritize.calibrate`): the blended approach (a range of penalty
  weights, by default around Cohon et al.'s penalty), the hierarchical one (the least penalty within cost
  thresholds above the least cost), the Pareto front of cost and shortfall, and Cohon's penalty; boundary,
  connectivity or linear penalties; every candidate's cost and criterion, and the candidates TOPSIS and the knee of
  the curve choose; the solves in parallel. prioritizr's optima, thresholds and least cost on landschaft's 3 cases.
- Management zones for `prioritize.optimize` (prioritizr's zones): planning units that go to at most one zone (or
  exactly one, `mandatory_allocation`), a cost per zone (`zone_costs`, a layer per zone; or a cost column per
  zone), every zone's own features (`zone_features`, prioritizr's zones(); a mapping of columns) or the base
  features with every zone's contribution (a number, one per feature, a raster or a column), Marxan's rij with a
  zone column; targets summed over the zones, per zone (a features x zones matrix) or by hand with lists of zones
  (`manual_targets`); every objective, with one budget or one per zone; weights per target or per feature and
  zone; locked units and manual bounds per zone; the solution as the zone of every unit and a layer per zone. The
  models are prioritizr's compile() bit for bit on landschaft's 23 zone cases, solved on landschaft's zones
  engines to prioritizr's optima.
- Spatial penalties and constraints with zones: boundary penalties (edge factors per zone, a zones matrix of how much
  a boundary between two zones counts), connectivity and asymmetric connectivity penalties with zones matrices,
  linear penalties and constraints with data per zone (`zone_penalty_data`, `zone_constraint_data`), neighbour
  constraints with k per zone, and contiguity per cluster of zones (landschaft's exact formulation). prioritizr's
  compile() bit for bit on landschaft's 10 cases.
- `prioritize.evaluate` with zones (`zone_solution`, a layer per zone, or the selection): the units, cost,
  boundary, connectivity and asymmetric connectivity overall and per zone, the features' representation and the
  targets' coverage (with lists of zones). prioritizr's numbers bit for bit on landschaft's 4 cases.
- Multi-action planning as prioriactions 0.5.0 (`prioritize.actions`), through landschaft's C++: which units to
  monitor and which actions against which threats to take in them, at the least cost for the features' recovery
  and conservation targets (minimizeCosts) or for the most benefit within a budget (maximizeBenefits); the
  boundary of the units and of every threat's actions, linear, quadratic or cubic responses; prioriactions's
  inputData() checks, defaults and warnings; the monitoring, the actions, getActions(), the cost, the benefit
  (total and per unit), the potential benefit and the connectivity. prioriactions's model bit for bit, its optima
  and its figures on landschaft's 6 cases (`tools/make_optimize_inputs.py` writes their tables).
- Restoration planning as plangea (`restoration.plan`): how much of every converted cell to restore for the most
  benefit within area targets (in the landscape or per region, absolute or shares) and a budget: the reduction of
  the species' extinction risk (the species-area relationship on `sdm.area_of_habitat`'s table and gain; z and a
  restoration discount per species) and linear benefits (carbon per km²). exact: the optimum within a tolerance
  by simplicial decomposition (a proven Frank-Wolfe bound; species at their potential lifted); steps: plangea's
  procedure (the target in steps, the benefits recomputed between them); uniform. plangea's units, bounds,
  biodiversity layers and plans of its steps on the cases landschaft keeps.
- The reduction of the extinction risk a plan brings and its uncertainty from z (plangea's multi_exrisk_aggregate:
  correlated, in quadrature or linear), in `restoration.plan`'s summary and species table; plangea's numbers.
- The ecosystems' risk of collapse: `restoration.ecosystem_areas` (every ecoregion's natural area now and before
  conversion, and what restoring a cell gives it; plangea's ec over all the ecoregion's land, the corrected form)
  and `restoration.ecosystem_targets` (a restoration target shared among the ecosystems for the least risk:
  plangea's steps, or the exact optimum). plangea's layers and targets on the cases landschaft keeps.
- Conservation and conversion plans (`restoration.plan`'s `conservable`, `convertible` and `threat`; plangea's C,
  D and R + D): conserving keeps the habitat the baseline loses, converting takes it away (the habitat lost per
  unit of natural land: `sdm.area_of_habitat`'s new `loss`); the area targets of the net change with
  restoration and conversion; the maps conserved and converted. plangea's units, layers and plans of its steps
  on the 4 cases landschaft keeps, and their exact plans.
- Trade-off curves in `restoration.plan` (`levels`: the landscape target growing; plangea's gradient steps, or
  exact plans nested or on the frontier) with the gradient (the levels acting on every cell), and projected
  changes of natural land per region (`projected_change`; plangea's econometric targets, compensated as
  harmonize_sr_sparable). plangea's plans and spared land on the cases landschaft keeps.
- The intactness benefit in `restoration.plan` (`hfi`, `reduction`, `increase`; landschaft's
  add_intactness_benefit): the total ecological intactness a plan gains, for restoration, conservation and
  conversion. Linearised at the plan of nothing (restoration.intactness_gradient, to the bit; recomputed between
  plangea's steps) or exact (`intactness_form`: the total intactness itself, not concave: successive concave
  minorants to a local optimum, bounded by the secants of the pairs through landschaft's C++
  `intactness_secant`).
- The cheapest plan for a benefit (`restoration.plan`'s `min_benefit`, landschaft's min-cost objective): regula
  falsi on the budget, each step the most benefit for it, the cost proven by the budgets that fall short.
- Sets of weights compared (`restoration.plan`'s `weights`, plangea's post_choose_weights): a plan per set, the
  benefits and cost rescaled over the sets and ranked by mean over deviation; the table and the best set's plan.
  landschaft's plans, costs and tables on the bd_absent problem (`tools/make_plangea_plan_reference.py`).
- `table.of`: a table written in the settings (the small tables of a planning problem).
- Templates: Restoration plan (plangea) and Multi-action planning (prioriactions).
- prioritizr's evaluation of a solution (`prioritize.evaluate`): the units selected, their cost and boundary, the
  features' representation, the targets' coverage, Ferrier et al.'s irreplaceability and the rarity-weighted
  richness; prioritizr's numbers bit for bit on the cases landschaft keeps; and on request the replacement
  importance (a solve without each unit selected, in parallel) and the rank importance (incremental solutions
  over budgets), prioritizr's to 1e-9.
- Template: Conservation planning (prioritizr).
- Marxan 4 (`prioritize.marxan`) through landschaft's C++: from Marxan's tables (pu, spec, puvspr, bound) or from
  maps (every cell a planning unit, its shared edges its boundaries), every option of input.dat; every run's
  reserve, the best, the selection frequency, the summary and the penalties. The same runs as Marxan 4.0.6 on the
  8 cases landschaft keeps (`tools/make_marxan_inputs.py`).
- ProxIRR irreplaceability (`prioritize.proxirr`): alpha, beta and gamma from Marxan's tables or maps, R's
  proxirr 0.5's bits (`tools/make_proxirr_inputs.py`).
- Template: Marxan and irreplaceability.
- Zonation 5's priority ranking (`prioritize.zonation`) through landschaft's C++: every rule (CAZ1, CAZ2, CAZMAX,
  ABF, CAZP, CAZPW, RAND, LOAD), weights and weight groups, thresholds, value extraction, the zero modes, 8 or 16
  bits, costs, hierarchic masks, analysis areas, the iterations' settings; the rank map, the performance curves,
  the features' table, the AUC. The same rank maps, bit for bit, and iterations as the z5 binary on the 41 cases
  landschaft keeps (`tools/make_zonation_inputs.py` writes the rasters they were made from).
- The rank in Zonation's ramp (dark to red), for the maps named rank or priority.
- `stack.of`: maps as one stack, a layer per input (the features of a prioritization).
- Template: Zonation.
- `.npy` files in Fortran order, and of 16-bit and unsigned 32-bit integers.
- Restoration planning's inputs after plangea, through landschaft's C++, plangea's bits: ecological intactness
  (Beyer et al. 2020) of a human footprint, before and after restoring cells (`restoration.intactness`), its
  gradient (`restoration.intactness_gradient`), the original vegetation of converted land
  (`restoration.original_vegetation`: past cover, ecoregions, failsafe shares) and the area of habitat of species
  (`sdm.area_of_habitat`: suitable classes, elevation limits, the habitat gained per cell restored). Tested against
  the plangea outputs landschaft keeps (`tools/make_plangea_inputs.py` writes the maps they were made from).
- Template: Ecological intactness.
- `GeoTags::is_geographic`, `linear_unit` and `geokey`.
- The build links landschaft's prioritization (Zonation), restoration planning and MILP (HiGHS) libraries
  (landschaft's new `LANDSCHAFT_MILP` option).

### Fixed
- Numbers in models and settings are read to the bit (serde_json's float_roundtrip): a decimal could become the
  double next to its own (the comparisons with landschaft's JSON references are now exact too).
- Outputs given on request or for some inputs only (`prioritize.evaluate`'s replacement, rank, coverage and
  Ferrier's; `prioritize.optimize`'s maps with tables) no longer fail the run: an operator's optional outputs may
  be missing.
- Closing a pane, or the maps of a past session, no longer leaves an empty floating window that panics when
  drawn (egui_dock's retain_tabs): such windows are removed.

## [0.9.0] - 2026-10-11

### Added
- Landscape metrics, landscapemetrics' and FRAGSTATS', through landschaft's C++: `metrics.patch` (a row per
  patch, the map of the patches and of a metric), `metrics.class` (a row per class, the map of a metric),
  `metrics.landscape` and `metrics.window` (the landscape's metrics in a moving window, square or circular, a map
  each). The metrics by name (`ca`, `lsm_c_ca`), by type, or every one of the level, FRAGSTATS' extras when asked;
  every option of landscapemetrics (directions, boundaries, edge depth, complexity) and FRAGSTATS' (convention,
  search radius, contrast and similarity weights). The catalogue of the 256 metrics comes from landschaft
  (`tools/make_lsm_catalog.py`).
- Tested against the R package landscapemetrics: every value landschaft keeps of its calculate_lsm (14 rasters, 5
  sets of options, more than 100,000 values) and of window_lsm.
- Tables in the viewer: as many rows as they have (those in view laid out), sorted by a click on a column, filtered
  by words or a condition (`area > 1`, `np != 1`, `ed = NA`), and a column drawn as bars by class or patch.
- Template: Landscape metrics (per class with the share as a map, the landscape's diversity, a moving window).
- More of GuidosToolbox through landschaft's C++: `fragmentation.contagion` (Pff) and `fragmentation.clustering`
  (FAC) in a moving window, `fragmentation.entropy` (the map's spatial joint entropy) and
  `fragmentation.entropy_map`, `fragmentation.landscape_mosaic` (19 or 103 classes, from a map coded so or the
  classes of agriculture, natural and developed land given), `fragmentation.gray_convolution` (GraySpatCon's ten
  metrics of a % cover), `landscape.parcellation` (APS, AWAPS, division, PARC per class and of the map),
  `landscape.contortion` (and GTB's display of it), `distance.euclidean` (signed, with GTB's byte map),
  `change.simple`, `change.morphological` and `change.fos`.
- Their tests: the landscape mosaic of 103 classes against GTB's own outputs (the digests landschaft keeps), the
  others against landschaft's (`tools/make_gtb_reference.py` writes `tests/data/reference/gtb_landschaft.json`):
  every map bit for bit, the reports' numbers to their last bits.
- GTB's colour tables and names for them (`tools/make_gtb_palettes.py` writes `terraflux_ops::gtb_palettes` from
  landschaft): the mosaic's 19 classes, change, FOS change, entropy, distance and contortion; ramps in named bands
  in the legend (10-19%..., distances in cells, the contortion in fifths).
- Template: Landscape mosaic (the mosaic of a land cover, the contagion and entropy of its natural land).

### Changed
- The library shows the operator's name in one line and its prefix beside it (long names broke in two).
- A legend puts the values of the same name in one line whatever their colours (the bands of a ramp).

## [0.8.0] - 2026-10-11

### Added
- The rest of GuidosToolbox's main tools, through landschaft's port (its C++, as MSPA), the same bits as GTB's
  outputs that landschaft keeps (`tools/make_gtb_inputs.py` writes the maps they were made from):
  `landscape.spa` (SPA2, 3, 5 and 6, the objects counted and the porosity), `landscape.accounting` (the
  objects in size classes: GTB's batch classes, those given or "auto"; the objects' ids and areas as maps),
  `fragmentation.fad` and `fragmentation.fad_multiscale` (the density at each window, their average, its
  classes, AVCON), `fragmentation.fos` (FAD, FED or FAC, 5 or 6 classes, binary maps or % covers, GTB's report:
  ECA, coherence). Every one takes the foreground's classes or a map in GTB's codes.
- Their maps in GTB's colours with the names of their values (`terraflux_ops::landscape::Product`), by the
  operator and port that made them or the names GTB gives its files; the legend of a density (FAD, FOS) in its
  classes, rare to intact, a line each.
- Templates: Fragmentation of a forest (FAD-MS and FOS), Patterns and objects (SPA6 and Accounting).
- `/` searches the operators from the keyboard: the arrows choose, enter adds the one chosen connected to the node
  selected, esc closes the search. The library's search ranks names before descriptions.
- Templates (the Templates menu): models that run as soon as they open, to see a flow and check it works: MSPA
  of a forest, MSPA of your map, a forest from a surface, MSPA's edge width, neutral landscapes.
- Neutral landscape models from landschaft's port of NLMR: `nlm.random_cluster`, `nlm.gaussian_field`,
  `nlm.fbm` (a size, or another map's grid; a seed).
- `raster.reclassify`'s rules as a table in the node: the classes of the map read with their share (A rule per
  class), or its range (4 equal ranges), and how to write the rules.
- Open a folder as a stack (the library, and A folder... on a `raster.read`).
- Colours by what the values are: a ramp centred on 0 for values on both sides of it; terrain, grey and
  red-to-green ramps for elevations, hillshades and NDVI (by the names of nodes, files and bands); MSPA's
  colours and names through `view.plot` and for files named so.
- GDAL, loaded when the program runs (`terraflux-gdal`: a few functions of its C API, no GDAL to build): every
  format it reads, read in windows at any resolution with a handle per thread; `raster.reproject` (gdalwarp: a
  CRS, a resolution, an extent, the method by the data), `raster.match` (landschaft's match_grid),
  `raster.clip` (gdal_translate); the CRS of rasters it read written to the files made. Found in the folder
  `gdal` next to the program (`tools/bundle-gdal.ps1`), in `TERRAFLUX_GDAL`, or installed.
- `raster.reclassify`: rules `1 2 3 -> 1`, `0..10 -> 2`, `* -> nan`.
- More of GDAL: `raster.resample`, `raster.convert` (formats, cell types), `raster.mosaic` (a series of tiles
  into one raster), `raster.terrain` (gdaldem), `raster.fill` (gdal_fillnodata), `distance.proximity`
  (gdal_proximity). A series may hold files of different grids (each layer on its file's grid; no stack of
  them).
- Several nodes at once: a box of selection, shift or ctrl and a click, ctrl+A, Esc; the nodes selected moved
  together and deleted together. Arrange (ctrl+L): columns by depth, each ordered by the rows of the nodes it
  reads.
- A double click on a node opens its output in a map tab of its own (run first if it has not run), beside the
  viewer; as many map tabs as one likes, each movable to another window.
- The View menu turns each pane on or off.
- `view.plot` (Visualization): its map tab opens by itself when it runs and is refreshed at every run; nothing
  is written. Save output, in the viewer and every map tab: the output to a file (GeoTIFF, CSV or JSON, in the
  background, the status bar telling when done), or to a file and a `raster.write` in the model.
- Nodes move anywhere on the canvas (no more limit at the top and the left).
- Basemaps in the viewer and the map tabs: Esri's imagery and topographic map, OpenStreetMap, OpenTopoMap and
  Carto's light map, by GDAL's WMS driver (tiles kept in the user's cache), warped onto the data's grid tile by
  tile in threads of their own (finer than the data's cells when zoomed in, around the data too); the data shown
  or hidden over it, at an opacity; the services' attribution written on the map
  (`terraflux_gdal::tile_service`, `warp_image`). GDAL's web requests use its certificates and say they are
  Terraflux's.
- Rasters of any size: `raster.read` reads headers only and gives rasters on disk (`Value::Raster`, type
  `raster`; a folder or a pattern gives a series), read whole only where a port takes a map or a stack (up to
  two billion cells); `raster.write` copies them in blocks. `terraflux_raster::image::Image` reads windows at
  any step, from overviews when the file has them, decoding only the chunks needed, in parallel, with a cache.
- The viewer draws maps from a pyramid of tiles made by threads (the tiles in view, at the screen's
  resolution; the coarser one shown until a tile comes): a raster of 7.3 billion cells shows in 0.4 s and a
  view 67 times closer completes in 0.3 s, frames under 10 ms.
- Panes in a dock (`egui_dock`): dragged, split, closed and opened again (Panels menu), or taken to a window of
  their own on another screen; the layout kept in the user's application data.
- The library follows landschaft's subjects (the sections of its reference), land change simulation grouped by
  model; the subjects still empty shown dimmed.
- The application opens in the model builder: a library of the operators by subject (with a search) whose
  operators come connected to the node selected, rasters opened and saved from it, and under the canvas what
  the node selected gives (a map with its legend, the maps of a stack or of a loop's steps, a table, a value).
  A run stops at the node shown (ctrl+enter, or by itself after every edit) and takes the rest from the cache;
  each node shows how it did (running, run, from the cache, failed); F5 runs the whole model, del deletes a
  node, ctrl+S saves. New models start empty (New), the wizard is a button (Wizard...).
- Runs that keep what the nodes gave (`Engine::run_with` and `Cache`): a node runs again only when it, a node it
  depends on, an input or a file it reads changed (its place on the canvas and its note do not count); a run
  may stop at a node (its ancestors only, and only their errors stop it); loops give back what they collected
  from the cache. Nodes that give a random generator, those after them, and `raster.write`, always run.
- `raster.read` (a band as a map, every band as a stack) and `raster.write` (a map, a stack or a loop's maps,
  in the smallest type that holds them): rasters read and written by nodes, their paths relative to the
  model's folder in the file and absolute once it is expanded, as plugins'.
- `landscape.mspa`: GuidosToolbox's Morphological Spatial Pattern Analysis through landschaft's port (the
  `gtbcore` library), its foreground the classes given or GTB's codes; shown in GTB's colours with the names
  of its classes. Tested bit for bit against the outputs of GTB's `mspa` executable that landschaft keeps.
- `terraflux_raster::npy::read_npz` and `DataType::fitting` (the cell type a raster is written in).
- The builder's canvas for large diagrams: drawn by the GPU (wgpu pipelines in egui's render pass: the boxes,
  ports and connections as instances, the curves and the smoothed edges in the shaders), uploaded only when the
  graph changes; a camera (drag the background or with the middle button, the wheel zooms around the pointer,
  Home fits the graph) instead of a scrolled area; the texts of the nodes in view only, and only when readable;
  a grid finds what is under the pointer; a node dragged moves alone in the scene. With 10,000 nodes a frame
  takes about 0.1 ms (it took 4.7 s with 2,000), dragging a node 0.5 ms, an edit about 30 ms.
- How frames are presented: one frame queued by default (the window follows the pointer more closely);
  `TERRAFLUX_PRESENT`, `TERRAFLUX_LATENCY` and wgpu's `WGPU_BACKEND` to try what is smoothest on a machine.
- Tests of the canvas with the pointer (select, move, connect, show an output, zoom) and with 5,000 nodes;
  timings with 100, 1,000 and 10,000 nodes (`cargo test --release -p terraflux-app --test canvas -- --ignored
  --nocapture`).
- The map tab shows the maps of the inputs too: a stack's one after the other (a slider, the left and right
  keys, play) with small images of all of them; maps of the land use's classes in its colours.

### Changed
- A double click on a node shows its map in its one tab: brought to the front when it is open already, a new one
  only once it is closed (it opened a tab at every double click).
- A folder or a pattern read by `raster.read` gives every band of every file (named `file band` when a file
  has more than one), as one stack when they share a grid; it gave the first band of each file.
- No more tabs at the top: the panes are arranged in the dock; the application opens with the model, the
  operators, the node and the viewer (the simulation's maps, the areas and the validation from the View menu).
- The library's groups (land change simulation's models) are closed until opened.
- The canvas: dragging the background selects; the view moves with the right or the middle button.
- The application's look: mid greys in flat panels, an orange accent for what is selected or on (after Ableton
  Live), a status bar with the cell under the pointer.
- The wizard has three steps: the data is checked as soon as it is chosen, in a thread, and what is wrong is
  shown below it (no step only to say that the data is right).
- A faster application: a model's data is read in a thread (the window answers at once); the area of each
  class is counted once per map, in the run's thread, instead of at every frame (the charts and the map tab
  drew slower the more steps there were); maps are coloured from tables, in a thread (a frame never waits
  for a map: the one shown before stays meanwhile), and their textures kept to be shown again at once; the
  legends of the maps shown are worked out in threads; a run uses the data read when the model was opened.
  With a map of 2048 x 1728 cells shown and MSPA running on it, frames take under 4 ms.

### Fixed
- The name of an MSPA value from 156 up overflowed (a panic in debug builds).
- Checking a model and ordering its nodes took time growing with the square of its nodes (a node looked for in
  a list for every reference): 141 ms for 10,000 nodes, now 10 ms; the engine's runs and fingerprints too. The
  order of the nodes no longer recurses (chains of any length).
- The automatic layout of the canvas took time growing with the cube of the nodes, at every frame.
- The operators' signatures are made once (`Registry::sig`), not at every look-up.
- The text tab lays out only the lines in view, and the model's text is made only when the tab is shown.
- The node editor no longer shows the references inside a loop's body as sources of the top level.
- The wizard's check of an ordered allocation no longer fails before the order of the classes is chosen.

## [0.7.1] - 2026-10-10

### Changed
- `terraflux` without a command (a double click on the executable) opens the application, and so does a model
  file given alone (dropped on the executable, "open with"); on Windows the console opened for it goes away (a
  terminal's stays).

## [0.7.0] - 2026-10-10

### Added
- Agents (type `Agents`: a population, a column per attribute): `agents.table` (macro agents written in the
  model), `agents.parcels` (an agent per parcel, its size and the means of maps over it), `agents.update`
  (behaviour: rules `attribute = expression` in order, map algebra's expressions), `agents.map` (an attribute
  painted on the parcels) and `agents.summary` (count, sums and means, to collect). Agents are carried along
  loops as the map is, and written as CSV.
- CRAFTY's competition for land (`crafty.allocate`): agent functional types produce services from the cells'
  capitals; unmet demand sets the services' value; managers give up below a threshold, and the most competitive
  type takes offered cells over the incumbent by its giving-in threshold; the demand's columns mapped to
  services (`services`); the same maps with the same seed.
- `docs/agents.md`.
- Tests: the agents' operators; CRAFTY's supply meets the demand and each type settles where its capitals are;
  a coupled model (a growing population giving the demand, parcel managers subsidising fertility, CRAFTY
  allocating) run by the engine.

## [0.6.0] - 2026-10-10

### Added
- The node editor (the application's Graph tab): operators added from a list, ports connected by dragging
  (each connection checked against the operators' signatures: types, cycles, ports), nodes moved (their place
  saved in the model, `pos`), renamed (their references follow) and deleted, values and settings typed, the
  bodies of loops and groups edited in their own scope (what they read from outside on the left), the model
  saved as written. `terraflux_app::editor` holds the editing operations.
- Map algebra (`map.calc`, as Dinamica EGO's Calculate Map): an expression over inputs of any names (maps,
  numbers, the layers of stacks as `stack.layer`): arithmetic, comparisons, `and or not`, `if then else`,
  functions, and neighbourhood functions (`nbsum nbmean nbmin nbmax nbcount`); missing cells propagate.
- Groups (`control.group`: a body run once, its inputs `@name` inside, its outputs `body.outputs`) and
  submodels (`control.submodel`: another model file as one node, expanded into a group before a run; its
  file inputs become inputs of the outer model).
- WebAssembly plugins (`plugin.wasm`, wasmi): a function of any language compiled to WebAssembly called for
  every cell, in a sandbox (no imports, a budget of instructions); batches send the module to the workers by
  hash.
- Operators may take inputs of any name (`OpSignature::extra`).
- `docs/modelling.md`.
- Tests: the editor's operations (types, cycles, renames, removals, bodies) and the editor in the application;
  map algebra; groups and submodels in the engine; plugins (and their sandbox); a submodel of map algebra and a
  plugin give on workers the bytes of a local run.

### Changed
- The application's Graph tab is the node editor (the read-only diagram is gone).

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
