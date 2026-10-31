# Changelog

All notable changes to Terraflux are documented here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 the model format and the API may still change between
minor versions.

## [Unreleased]

### Added
- **Compare** in the Viewer and the map tabs: another output of the run, or another layer of the one shown (a
  step of a loop, a band), beside the map. As a **Curtain**, the other map right of a line dragged across the map
  by its handle, the names of both at the top; **Side by side**, two views that zoom and pan together, a cross in
  one where the pointer is in the other. The two in the same colours (the classes of both, or the range of both),
  the values of both under the pointer; a map of another grid placed by its coordinates.
- **Methods and citations** (File menu, the Document pane, ctrl+K): the methods section of a paper drafted from
  the model. A paragraph with the version of Terraflux and each method the model uses, with its papers and the
  software reproduced ("We computed morphological spatial pattern analysis (MSPA; Soille & Vogt, 2009) with
  Terraflux, which reproduces GuidosToolbox (Vogt & Riitters, 2017)."); the model step by step; the references
  in the APA style (those of the website's Cite Terraflux, and the nodes' own), copied, or saved as BibTeX or RIS.

- **This cell in every map**: a cell clicked in the Viewer or a map tab lists its value in every map of the run at
  the same place (by their coordinates; a stack's first layer, rasters' band chosen), a name showing that map,
  the list copied as text.
- **Draw on the map** (the Viewer): polygons, rectangles, points or lines clicked on the map shown, a study area
  or plots; **Add to the model** writes them as a GeoPackage beside the model, in the map's coordinate system, read
  by a new `vector.read` node and drawn over the map.
- **Data catalog** (File menu, ctrl+K): the datasets `data.download` fetches as cards (WorldClim 2.1 and its
  CMIP6 futures, CHELSA 2.1 and its futures, the Copernicus DEM, SoilGrids 2.0, MapBiomas Brazil, Hydrography90m),
  each with its resolution, years, coverage, licence and the paper to cite; searched, filtered by theme. Its choices
  (the bioclimatic variables by name, months, model, scenario and period, soil property and depths, years) and a
  grid (a map of the model, or a box of longitudes and latitudes); **Add to the model** adds the `data.download`
  node so set, the reference in its references (Methods and citations cites it).
- **Figure...** in the Viewer: the map shown as a figure for a paper, alone, beside the one compared, or every layer
  of a stack or a series as panels (a), (b)... in columns: a title, the width of a journal's column or page (mm),
  150 to 1200 DPI, the legend (one for maps of the same colours, else each map's), a scale bar in metres or
  kilometres (also on longitudes and latitudes), a north arrow and the coordinates along the edges, previewed as it
  is set. Saved as PNG (its resolution recorded) or as SVG, its text kept as text to edit in Inkscape or Illustrator.
- **Export to QGIS...** (File menu, ctrl+K, and the viewer's Save output): the maps and vector layers of the run
  chosen, written to a folder (GeoTIFF, GeoPackage; a raster read from a file referred to where it is) with a QGIS
  project that opens them in Terraflux's colours: the classes as a paletted map with their names, values on the
  same ramp and range, the vector layers on top, the coordinate system of the maps; a `.qml` style beside each map
  written. **Open in QGIS** opens it.
- **Save as GIF...**: the steps of a simulation (the Simulation pane), the maps of an input's stack, or the
  layers of a series, a stack or rasters (the Viewer) as an animated GIF, for a talk or a class. Each map in the
  colours it is shown in (a series' steps in one colouring), its step or name in its corner and its legend under
  it, a small map enlarged and a large one read every few cells; made in the background, the last frame held.
- Colour-blind readers: the ramps **cividis** and **orange to purple, centred on 0** (ColorBrewer's PuOr) in the
  Viewer's colours, and **View > Colour vision**, every map, legend and picture drawn as a reader with
  protanopia, deuteranopia or tritanopia sees them (Machado et al. 2009), said in the bar at the top until set back.

### Changed
- The rules of **raster.reclassify** are a table of three columns in the Node pane, **from**, **to** and
  **becomes**: a range is its start and its end in two fields (`0`, `10`, `2`), no `0..10` to type; `to` left
  empty takes only the values in from (`1 2 3`, `*`), `inf` as the end takes every value above the start. The
  rules' text takes a line of three fields too, `0 10 2` (or a table's rows pasted, with commas or tabs), the
  range `0..10 -> 2`; models written with `..` read as before.

## [1.27.0] - 2026-10-31

### Added
- Modern themes: **Slate** (dark), **Paper** (light, in the colours and the font of the website, Source Sans 3)
  and **Canopy**, beside the classic ones, kept as they were. Rounded tabs and panes, the Node pane in cards
  (inputs, settings, body, the run), its references as coloured chips, switches for Bypass and Freeze, lighter
  plot grids and a quieter toolbar. Slate and Canopy use Inter.

### Fixed
- Larger text (Settings, **Text size**) no longer spills out of its boxes, in the classic and the modern looks:
  the toolbar puts its tools in a **Tools** menu when there is no room, the status bar, the Node pane's name and
  **Rename**, the Operators pane (long subjects and names cut short), the model's bar, the Files pane, Many
  runs, Parameters and the Assistant fit their panes at every size. A test draws every pane, menu and window at
  each size and theme and checks that nothing overflows.
- Windows no longer asks for the firewall when a pool shared on this computer only (the tests) announces itself:
  its beacons stay on this computer. The firewall is asked only when the computer is shared or lent on the network.

## [1.26.0] - 2026-10-30

### Added
- The table gathered from the files of a folder names each row's file in its column `layer` (`clc_r0c0`), where it
  held the file's number. Tables can name the numbers of a column (`labels`, as a factor in R): the CSV, the JSON
  and the text of `table.write`, the outputs of a run, the table viewer and the charts show the names. When the
  files' names end in different numbers (`forest_2010`), the column still holds those numbers.
- A chain of nodes run on a folder keeps in each file's record what is read after it, when small (a table, a
  number, a text). A model with MSPA and its metrics no longer runs every file again: the files done before are
  taken from their records, the table of all of them the same.
- The files that failed in a chain of nodes are listed in the Node pane of its first node, each with why, with
  **Run again** (the files done before not run again) and **Copy the list**. The engine tells them to its observer
  (`Observer::failures`).
- `distance.proximity` and `landscape.mspa_tiled` given a folder run on each of its files, as `raster.terrain`
  and `raster.fill` do. `mspa_tiled` writes a file per file, by its name (`{name}` in `path`, else the name before
  the file's).

### Fixed
- `distance.proximity` and `landscape.mspa_tiled` given a folder measured only its first file, without saying so.

## [1.25.0] - 2026-10-30

### Added
- A folder of rasters goes straight into any node that takes a map, and runs through the chain file by file.
  A `raster.read` of a folder (or a pattern) feeds a node where it takes one map. That node, and the nodes after
  it that take its maps, run as one chain, file by file: a file is read, passed along the chain and written,
  then let go. Before, each node ran on all the files before the next one, with every result in memory.
  - A file that fails does not stop the others. A warning says which files failed and why; the run fails only
    when every file does.
  - A warning that some files give is told once, with the files that gave it.
  - At the top level, the maps of the chain that are only written to files are not kept in memory when they
    would take more than 2 GB.
- A chain of nodes on the files of a folder runs several files at the same time, as many as the threads. The
  `raster.read` setting `workers` sets fewer for large files, each of which is in memory while it runs. The
  files run in a pool of that many threads, which the operators share. The results, failures and warnings are
  the same, in the files' order, whatever the number of workers.
- A chain that writes files records each file next to its first file written (`mspa/clc_r0c0_mspa.json`): the
  file read, the files written, the nodes and their settings, the time taken and Terraflux's version. When
  nothing but the chain's `raster.write`s reads what it gives, a file whose record says it was done with the
  same nodes, settings and file, with its files still there, is not run again. A run stopped halfway goes on
  where it stopped, and a run again after failures runs only the files that failed.
- The boxes of what a scope reads on the canvas (the model's inputs `$name`, a container's values `@name`, the
  outer nodes a body reads) are moved, selected and deleted like the nodes. Their places are kept in the model
  (`places`); those never moved stay in the column at the left.
  - An input of the model, clicked, is edited in the Node pane: renamed (what reads it, its parameters and
    scenarios follow), its file chosen or its value typed, its note written, or deleted.
  - **Make it a node** turns an input of a raster file into a `raster.read` of its file, read by what read it.
    **Make it an input of the model**, in a top-level `raster.read`'s right-click menu, does the reverse.
  - Deleting an input of the model (Delete, its menu, or the MCP tool `remove_input`) disconnects what read it.
    Before, the MCP tool left those references dangling.
- `raster.read`'s `first`: only the first files of a folder, to try the nodes after it on a few before all.
- "Run on a folder..." in the node pane and in a node's new right-click menu on the canvas. It reads a folder
  where the node takes its first map: the `raster.read` that feeds that input reads the folder, else one is
  added at the node's left.
- A node given two folders (or more) where it takes maps runs on their files paired by name: `uso/bacia_07.tif`
  with `estradas/bacia_07.tif`. Names that differ are paired once each folder's common start and end are cut at a
  `_`, `-`, `.` or space (`uso_bacia_07` with `estradas_bacia_07`). The files with no pair are left out, with a
  warning. The records and the resuming cover the files paired.
- **Process a folder...** in the File menu, the command palette and the empty canvas. It asks for the folder, then
  for the operator (the operators that take a map), and builds the nodes: a `raster.read` of the folder, the
  operator, a `raster.write` of its map to `<folder>_<op>/{name}_<op>.tif` and a `table.write` of its table to
  `<folder>_<op>.csv`. The operator is selected, to choose its settings.
- **Make it a chain of nodes** in the Node pane of a `batch.run` turns it into the same nodes: a `raster.read` of
  its folder, its operator with its settings, and writers to its output folder. It is refused, saying why, when
  something reads its table or report, or when it reads a list of files or the sub-folders.
- The tutorial "Batch processing" is rewritten for folders given to the nodes, and so is the batch section of
  "Large maps".

### Fixed
- The files of a folder of different grids (tiles, basins, municipalities) keep their names through the nodes
  run once per file. `raster.write` with `{name}` wrote `10_mspa.tif` instead of `clc_r0c0_mspa.tif`.
- `raster.write` of maps of different grids without `{name}` in its path writes a file each, named by the map's
  name followed by the file's name (`mspa/mspa.tif` gives `mspa/clc_r0c0_mspa.tif`). Before, it wrote one
  file on the first map's grid.
- The landscape metrics of rasters on disk (`metrics.patch`, `metrics.class`, `metrics.landscape`,
  `metrics.window`, `metrics.sample`) run once per file of a folder and per band of a stack, as the operators of
  a map do. Before, they measured only the first file, without saying so.

## [1.24.2] - 2026-10-29

### Fixed
- `move.extract_covariates` reads rasters whose coordinate system is only an EPSG code (as `data.download`'s)
  under tracks in another one: every value was empty, the tracks were not reprojected. The species
  distribution operators read such rasters the same way now.

## [1.24.1] - 2026-10-29

1.24.0 was not published: its Linux build failed. This release has its additions and these fixes.

### Fixed
- The Linux build: landschaft's x87 `log1p` (R's, which the movement operators use) did not compile with GCC.
- Vectors are written to GeoPackage (and SQLite) in one transaction: 47 000 points in half a second, not 29
  seconds (a commit per feature before).
- `vector.filter` and `vector.calc` read attributes whose names have other characters than letters, digits and
  `_`, as Movebank's: `individual-local-identifier` is `individual_local_identifier` in a script,
  `eobs:type-of-fix` is `eobs_type_of_fix` (the error of a name not found lists them so).

## [1.24.0] - 2026-10-29

### Added
- Animal movement, `move.*` (landschaft's `ls.move`: the methods of move2 and amt), each bit for bit with
  landschaft on its reference tracks (`tools/make_move_reference.py`):
  - tracks: `move.read_tracks` reads Movebank's files (csv, csv.gz or zip, the columns typed as pandas does),
    `move.as_tracks` makes tracks of points or of a table's coordinates, `move.simulate_brownian` draws Brownian
    tracks; a track is a point vector with a time column (ISO 8601 texts, read in UTC, or numbers) and a track
    column, both found by their usual names or given by the `time` and `track` settings;
  - measures, each a column added: `move.distance` (on the ellipsoid in longitude and latitude),
    `move.time_lags`, `move.speed`, `move.azimuth`, `move.turn_angle`, in the units chosen;
  - filters: `move.filter_unique` (the duplicated times, by subsets or the first, last or a sample),
    `move.filter_per_interval`, `move.filter_movebank_visible`;
  - lines: `move.track_lines`, `move.segments`, `move.track_data`; `move.interpolate` (at given times or every
    unit, the great circles of s2 in longitude and latitude) and `move.aeqd_crs` (the azimuthal equidistant CRS
    centred on the tracks, and the tracks in it);
  - amt: `move.resample` (rate and tolerance, the bursts), `move.filter_min_n_burst`, `move.steps`,
    `move.time_of_day` (day, night, dawn and dusk from the sun's times), `move.nsd`, `move.track_summary`
    (distances, straightness, mean squared displacement, sinuosity, temporal autocorrelation of the directions);
  - step selection: `move.distribution` and `move.fit_distribution` (gamma, exponential, log-normal, half-normal,
    von Mises, uniform; the fits with their covariances), `move.random_numbers`, `move.random_steps` (the control
    steps drawn as amt draws them) and `move.extract_covariates` (the maps' values at the steps' ends or starts).
  A step distribution is a new type of the graph's values, written as JSON. The library has a subject for them,
  Animal movement.
- `vector.calc` and `vector.filter` read texts: the features' text attributes compared with texts
  (`uso == "floresta"`, `"a" <= nome < "n"`), chosen (`"alto" if pop > 100 else "baixo"`, `coalesce`,
  `uso[pop > 100] = "outro"`), tested with `isnull` (an empty text has no value) and counted (`uso.count()`); a
  text input of the node compared too, and `float(cod)` reads a text attribute that holds numbers. A text where a
  number goes (a sum, `uso > 3`, `uso.mean()`) is refused before running, saying what to write instead.
- The features' measures in their scripts: `geometry.area`, `geometry.perimeter`, `geometry.length`,
  `geometry.x`, `geometry.y` (those of `vector.geometry`), and `feature`, the feature's number.
- `vector.calc` runs scripts of many lines, as `raster.calc` does: each `out name = ...` gives an attribute, new
  or replacing one, and the column is left empty; without `out` the last line goes to the column as before.
  `vector.filter` runs scripts too, ending with the condition.
- The assistant and the MCP server write and explain these calculators (write_calc with `op` vector.calc or
  vector.filter, explain_calc): the script checked and tried on a sample of the features (up to 5000) before the
  model changes, the reply telling each attribute given (its classes or texts and their shares) or how many
  features the filter keeps.
- The website has a Calculators section: the raster calculator and the vector calculator, each with its language
  in full, recipes and pitfalls, and a playground where scripts run as you type over Sibuyan's maps and land-use
  patches, computed in the browser by Terraflux's own map algebra built to WebAssembly (the crate
  `terraflux-calc-web`). Every example on the pages gives what `terraflux run` gives.
- The website's series on ecosystem services has three parts more, each from real runs on Ribeirão Claro:
  nutrient delivery (`services.ndr`), seasonal water yield (`services.seasonal_water_yield`) and crop pollination
  by wild bees (`services.pollination`). New pages and sections for the application: the model's document and its
  reports, Trace, the Changes pane, the application's own assistant, Step through a loop, the Parameters pane and
  Compare in Many runs, shared node libraries (`TERRAFLUX_NODES`) and updates on macOS and Linux; checks, tests
  and reproducible runs (`check.assert`, `terraflux test`, `terraflux reproduce`, the manifest, frozen nodes).

### Changed
- A run's manifest holds the files the nodes read (`reads`: a `raster.read`'s, a `table.read`'s, the threats of
  habitat quality...), the files they wrote (`writes`, compared again by `terraflux reproduce`, but a GeoPackage)
  and the frozen results used (`frozen`), each with its SHA-256, and every path also from the manifest's folder.
- `terraflux test` and `terraflux reproduce` use the results of the frozen nodes, as `terraflux run` does (a
  frozen result changed or gone since the run noted); `--thaw` runs every node instead.

### Fixed
- `terraflux reproduce` of a study whose folder moved (copied to another computer, renamed) stopped with "The
  system cannot find the path specified": the model, the inputs and the files read are found from the manifest's
  folder first, and a file found at neither place is named.
- `terraflux run model.toml` (a model named without its folder) stopped with "cannot make an empty path
  absolute": a model's folder is now the current one when its path names none, in every command.
- A setting's default given as a list (`(default [0.05, 0.95])`) was cut at its first comma in the Node pane's
  hint and the document ("default [0.05").

## [1.23.0] - 2026-10-28

### Added
- The document says what a step does in a sentence written for its operator, with its settings and where its
  inputs come from ("A Gaussian random field of 100 × 120 cells: autocorrelation range 10 cells, variance 5,
  nugget 0.2; seed 7."; "Habitat split into habitat and matrix, a share 0.4 of it habitat."), for the operators
  models use most (reading and writing, the neutral landscapes, Markov, regressions, weights of evidence,
  allocation, metrics, MSPA, validation, SDM, carbon, prioritization); in the HTML and PDF reports too.
- The document edits a setting's list or table in place, item by item (add, change, take away), as the Node pane
  does.
- The document's pictures of maps in the viewer's palettes: the one chosen by hand for the output, else by what
  made it (MSPA's classes in GuidosToolbox's colours, elevations, ranks...), the classes in the model's legend's
  colours.
- The Changes pane compares the model with any commit of its file ("Compare with a commit (git)": the last 50,
  with their dates and messages), not only the last one.
- A node's tables before and now side by side in the Changes pane, the cells that changed (by row and column
  name) highlighted and the columns only one of them has in the colour of what was added or removed.
- Inside a group, the node selected shows its map in the viewer, named by its place in the group
  (`preparation/temperature`), with its own outputs to choose from and Trace from its cells.
- Compare in Many runs (Parameters): the scenarios checked run at once as a batch of Many runs (on this computer
  and those helping it, or a cluster), their results shown side by side in the pane when it ends.
- Scenarios compared on a map of classes differ by their changes: the class each cell changed to (nothing where
  it stayed), how many cells changed class and the most common changes ("2 → 3: 8517"), instead of the values
  subtracted.
- Trace: a card's name selects its node and shows its map in the viewer with the traced cell outlined; a patch
  operator's cell (labels, sieves, MSPA, patch metrics) is explained by its whole patch (its class, the cells joined
  to it by 4 or 8 neighbours as the operator joins them, the rows and columns it spans); an allocation (CLUE-S,
  ordered, Dinamica's Expander and Patcher, alone or in a loop traced inside) tells the steps the cell changed
  class at and each step's demand (the cells of each class and how much they changed) or the rate of its
  transition.
- The nodes the assistant reads or changes glow on the canvas while it works on them (a halo that fades).
- The assistant's model evaluated on a fixed set of tasks (Evaluate this model, in the server's settings): 8
  tasks of drawing, changing, fixing, explaining and running models, each in a model of its own and checked on what
  it made (a threshold cell by cell, a loop's last number...); the last score of each model kept and shown with
  its server, so models are compared on the same tasks.
- Calculator: a rectangle dragged on a map of the preview computes that part alone, every cell of it when they
  are few enough (Whole map, or a double-click, goes back to all of it).
- Calculator: a stack the script makes from another, its cells changed one by one (`s2 = s * 2`), has its layers
  completed after a dot (`s2.`).
- The script of a calculator in the Node pane completes names as it is typed, as the Calculator does, its inputs
  known by their values in the last run.
- Shared libraries of nodes (a group's folder on a shared drive, a repository cloned), added in the settings: their
  nodes in the operators' pane beside yours, each library in a section of its own, and the library to save a node
  in chosen in the dialog.
- A node of your libraries in the quick search (`/`) and in the assistant's and the MCP server's tools:
  `list_operators` lists them, `describe_operator` tells one (`my:name`), `add_node` adds it with its settings.

### Changed
- The values a node of your library takes (numbers, whole numbers, yes or no, texts), unless another node feeds
  them, are its settings: in the Node pane's Settings, its own value shown and given back when the field is
  emptied; on the canvas folded away as an operator's optional inputs.
- A group runs again only the nodes of its body that changed and what depends on them (and the same for the groups
  inside it), not its whole body: the cache keeps the outputs of the nodes of the groups by place.

## [1.22.0] - 2026-10-27

### Added
- The checks' badges: each `check.assert` shows how it fared in the last run (passed in green, failed and only
  warned in amber, failed and stopped the run in red, not run or turned off in grey) on its node, where the tooltip
  says what failed, and on its step of the document; a container shows the worst of the checks it holds, inside
  loops and groups too. The model's health is a line in the status bar and at the top of the document ("3 checks:
  2 passed, 1 warned"), each check listed on hover.
- A whole run in the application of a model saved writes the model's outputs and the manifest of the run
  (`output/manifest.json` next to the model), as `terraflux run` does: `terraflux reproduce` runs it again. The
  status bar says so after the time of the run ("manifest", which opens the folder).
- Stop stops the long operators between their parts, and they tell how far they are: MSPA by tiles
  (`landscape.mspa_tiled`, between two tiles), the circuits from one to all and all to one (`circuit.one_to_all`
  and the INI files' runs, between two batches of points), the folds of `sdm.fit`, the replacement and rank
  importance of `prioritize.evaluate` (between two lots of solves, between two budgets) and the metrics by blocks
  (`metrics.*` with `block_rows` or rasters on disk, between two blocks; `metrics.window`, between two strips).
- A node's result frozen (Freeze in the Node pane, `frozen = true` in the model file): its outputs written next to
  the model (`<model>.frozen/<node>/`, a file each: maps in GeoTIFF, those not of whole numbers in float64, tables
  in CSV, vectors in GeoPackage, numbers, texts and fitted models in JSON; the same values read back) and used
  instead of running it, in later sessions and by `terraflux run` too, whatever it reads; what only it reads does
  not run either. A ❄ marks it on the canvas. Unchecked, the files go and it runs again. A node frozen before it
  ran is frozen when it runs; a random generator, agents or a series not of maps cannot be.
- A loop step by step, as at a breakpoint: with **Step** on in the status bar, the run pauses after each step of a
  loop (its maps shown as they come), the status bar saying where ("paused at years: step 2 of 10") with **Next**
  (the next step) and **Continue** (on to the end); Stop stops it there.

## [1.21.0] - 2026-10-27

### Added
- Three more of InVEST's models (natcap.invest 3.20) as landschaft 1.54 reimplements them, the same float32 and
  float64 bits (landschaft's references, `tests/data/reference/services_invest.npz`):
  `services.ndr` (the nutrient delivery ratio: nitrogen and phosphorus loads, surface and subsurface, their
  effective retention down to the streams, the delivery ratio, the exports, the index of connectivity and the
  totals by watershed; MFD or D8; the load types for every class or by class code),
  `services.pollination` (crop pollination by wild bees: each species' supply and abundance by season, the total
  abundance, and with farm polygons the pollinators on the farms, the total and wild yields and the means by farm;
  the guilds a list or InVEST's guild table, a file) and `services.seasonal_water_yield` (the monthly and annual
  quickflow of the curve numbers and rain events, constant or by climate zone, the local recharge, the available
  recharge, the evapotranspiration, the baseflow and its sum from upslope, the VRI, the tables by watershed and
  by watershed and month; or the routing of a recharge map alone). scipy's `exp1` is ported for the quickflow.
  The subsurface distance of the NDR is in metres (InVEST measures it in cells against a length in metres).
- The assistant writes map algebra. The tool `write_calc` (in the application and in `terraflux mcp`) writes a
  `raster.calc`'s script, a new node or the one named: checked before anything changes (its language, every name
  an input of the node or given a value before, the layers a stack has) and tried on a sample of its inputs' cells
  (the last run's values, or the rasters' files, sampled; `terraflux mcp` runs the model up to what it reads): a
  script that fails is refused with the reason and the model left as it was, and the reply says what each output
  gives (a map's classes and their shares, or its least, mean and largest values). `explain_calc` takes a script
  apart: each input and what it is (a map, a stack and the layers it is read as, a number), the layers read, and
  what each name holds at the end on the sample. Both are among the few tools of a small model. In the Calculator
  pane, a field asks the assistant to write the script (**Write**) and **Explain** asks it to explain it; the change
  comes to the Changes pane like the assistant's others.
- Terraflux updates itself on macOS and Linux too: the application (`Terraflux.app`, in a folder the user may
  write) is replaced by the release's zip, unpacked beside it; the folder of the `.tar.gz` by the release's; the
  `.deb`, `.rpm` and Arch packages are installed by the system's manager, the administrator's password asked
  (`pkexec`). Each closes, waits, installs and opens again with the model, as on Windows. Elsewhere (Windows' zip, a
  build, a folder the user may not write) the release's page is opened, as before.
- An update that did not install is told when Terraflux opens again ("Terraflux X was not installed"), with its log
  (`terraflux-update.log` in the temporary folder: the installer's on Windows, the script's elsewhere).

### Fixed
- The update on Windows closed Terraflux and installed nothing: PowerShell, started detached from any console to
  install once the application had closed, ends before it runs anything. It now has a console of its own, not
  shown (and leaves the application's job when the job lets it). A Terraflux 1.20.0 or older still has the old
  updater: install 1.21.0 by hand once.
- The settings are kept on macOS (`~/Library/Application Support/Terraflux`) and on Linux without `XDG_CONFIG_HOME`
  (`~/.config/Terraflux`); they were not kept there at all.

## [1.20.0] - 2026-10-27

### Added
- `sdm.bias_surface`: the sampling effort of a target group (the records of every species sampled the same way) as
  a Gaussian kernel density on the grid of the predictors, the bias of `sdm.background` (a target-group background,
  Maxent's bias file). It is landschaft's `bias_surface`, `MASS::kde2d`'s density at the cells' centres, with R's
  `bw.nrd` in each direction by default (`kde2d(h = H)` is `bandwidth = H / 4`), a number or `[x, y]`, `isotropic`,
  bandwidths raised or lowered to `limits`, the records' `weights` from a column, a linear `rescale` (e.g.
  `[0.05, 0.95]`, so that every cell keeps a chance) and the cells without predictors masked. The bandwidths are
  landschaft's to the bit, the density up to rounding.
- `sdm.fit` gives the out-of-fold predictions (`predictions`): the records with a column `prediction`, each one's
  suitability from the model fitted without its fold (none in fold 0), the same as landschaft's
  `SDMResult.predictions`. They score the model on what it did not see and feed models stacked on it (an ensemble,
  a calibration, the distance to an expert range).

## [1.19.0] - 2026-10-26

### Added
- Terraflux updates itself. It asks GitHub for the latest release when it starts (at most once a day; off in the
  settings) and shows **Update to X.Y.Z** in orange in the toolbar. On Windows, the Terraflux of the installer
  saves the model, downloads the new installer, checks it against the release's `SHA256SUMS.txt`, installs it once
  the application has closed (its progress only, no question, no administrator) and opens again with the model;
  the next start says "updated to X.Y.Z". Elsewhere (the zip, macOS, Linux) the release's page opens.
- Terraflux's version is shown: beside its name in the toolbar, in the window's title and in the settings, with
  the updates (**Check now**, the check at the start turned on or off).
- The Windows installer has pages: its progress, then the last page, which says Terraflux is installed and opens
  it (ticked).

### Changed
- Inside a container (a group, a loop, a switch, a node of your own), the canvas is framed in orange and its
  corner says which one it shows and its kind, and the Model pane's bar has **‹ Back** (also Esc on the canvas,
  nothing selected) and the trail as buttons (*Whole model › ...*), each back to its level. The Node pane's
  **Edit the body** is **Open inside ›**.
- The completions of the calculator show the one chosen by the keyboard (up and down, enter or tab puts it in):
  it was not marked, so the keys seemed not to work. Their keys are told under the list.

### Fixed
- Installing or removing Terraflux on Windows stalled the whole computer for about a minute: Windows made a restore
  point first (a snapshot of the disk), even for the installer of 1.18, which does not need the administrator.
  The installer no longer asks for one.

## [1.18.0] - 2026-10-26

### Changed
- The Windows installer installs Terraflux for the user, without the administrator's permission: Windows no longer
  asks to allow it when installing or uninstalling (nor makes a restore point first, which made the installer seem
  stuck). It goes to `%LOCALAPPDATA%\Programs\Terraflux`, with a shortcut in the user's Start menu and the folder
  on the user's PATH. A Terraflux installed for all the users by an earlier installer is to be removed first (the
  installer says so). The installer no longer adds a rule to Windows' firewall: the firewall asks the first time
  Terraflux is opened to the computers of the local network.

## [1.17.0] - 2026-10-26

### Changed
- Map algebra has one syntax, Python's, in every expression (`raster.calc`, `value.calc`, `check.assert`,
  `table.filter`, `table.calc`, `vector.filter`, `vector.calc`, `uncertainty.delta`, `uncertainty.monte_carlo`,
  `agents.update`'s rules): `a if c else b`, `**`, `and or not`, `None`, `log`. The forms of other languages
  (`if c then a else b`, `c ? a : b`, `^`, `&& || !`, `null`, `ln`) are refused with the one to write ("write **
  for a power (not ^)"), told at the token in the calculator and in the Node pane. A model written before is
  rewritten when it is read, only the statements that had an old form (their comments and brackets kept), with a
  note of each (`terraflux_graph::old_algebra`); saving it keeps the new form. The nodes of your library too.
- `map.calc` is `raster.calc`: a model with `map.calc` reads it as `raster.calc` (as `stack.layer` became
  `stack.extract_layer`). Its node's default id is `raster_calc`.
- The Node pane of a `raster.calc` edits its script in an editor of code under the setting's name, the pane's
  width (the terminal's font, its words in colour, its first error underlined and told under it), with an orange
  Expand button beside the name: the Calculator, the full editor (the names completed, the outputs shown as they
  are typed, the lines numbered, the console), in a pane of its own. It replaces Open in the calculator.
- The console of the calculator writes a missing value as `None`, and a map's missing cells as "missing".
- `terraflux mcp`'s `open_in_app` opens the model in the Terraflux already open, in place of its model, and
  brings its window to the front, rather than starting a new window each time: each application listens on a port
  of this computer only, told with a secret in a file of the application's data (`instances`). A new window opens
  only when none is open, or when the model open has edits not saved or a wizard is open (the answer says which).

### Fixed
- Runs of many tries (seeds, scenarios, a batch) failed with "the bundle ... could not be made" once Windows'
  Storage Sense had cleaned the temporary folder: it removes the empty mark of a bundle not used for a while,
  and the bundle's folder, left without it, was never made again. A folder without its mark is now made again.

## [1.16.0] - 2026-10-26

### Added
- Map algebra writes scripts, in a language close to Python's (an expression of before is a script of one line):
  one statement a line (or separated by `;`, `#` a comment), `name = value`, `name[where] = value` (the cells
  where the condition holds, the others keeping theirs), `s.layer = value` (a stack's layer replaced or added),
  and `out name = value` for each output of the node, a map, a stack or a number (the ports are the script's
  `out`s; without any, the last value is `map`). An operation on a stack is made on each of its layers;
  `mean sd median sum count min max (s)` and `percentile(s, p)` are the value of each cell over a stack's
  layers, and `x.mean()`, `x.sd()`, `x.median()`, `x.sum()`, `x.count()`, `x.min()`, `x.max()` and
  `x.percentile(p)` one number from all the cells (missing values left out in both). Also `a if c else b`,
  chained comparisons (`5 <= x < 8`), `**`, `True`, `False`, `None` and `log`. A stack's layers are read by
  name with every name made one (`1997` is `s._1997`, a band without a name `s.b1`), and a band name begun with
  a digit is told how to write it.
- The Calculator pane (View, or "Open in the calculator" in a map algebra node's pane) writes the script of the
  `map.calc` node selected, under the model: its words in colour, its first error underlined where it is and told
  with its line (a name that is no input, with the one meant; a layer a stack does not have), the names completed
  as they are typed or with ctrl+space (the inputs, the names given in the script, the functions and the methods
  with their help, and after `s.` a stack's layers with their names in the file, known from the last run or from
  the raster's header before anything runs), and each output shown beside it as the script is typed, computed on
  a sample of the cells. A console under it computes a line in the script's scope on every cell (`img.mean()`
  gives the number at once; a map, its numbers and its image) without changing the node. ctrl+enter gives the
  script to the node and runs the model up to it.
- Nodes of your own: a group or a calculator saved as a node of your library ("Save as my node..." in its pane,
  or the commands), used in any model as one operator from the Operators pane (My nodes), the commands or ctrl+K.
  The dialog shows what it takes and gives, found from what it is given and what its scripts read: each input's
  type, the layers a stack must have (`img.nir` read: `nir`), its value when none is given; each output's type;
  all of them, the name and what it does changed there before it is saved. Each save is a new version, a file of
  its own (`ndvi@3.toml`, in `nodes` of the application's data or `TERRAFLUX_NODES`). In a model the node is a
  group with `uses` (the name, the version, its ports) and a copy of that version's body: the model runs the
  version it was made with on any computer, with or without the library, and a newer version changes it only when
  it is updated to it (its pane offers it, keeping what it is given). Its pane also tells when it was changed
  since its version (Save as a new version) and makes it a plain group again. The check takes its ports' types
  and needs (an input not given takes its value of its own, in the run too), and a stack given it without a layer
  it reads is told when connected and stops the run with the layers missing and those it has.
- The tutorial The raster calculator (Building models): one script over Sibuyan's land use of 1997 and 2011 and
  its drivers gives the forest's years, its loss, a habitat index of the forest of 1997, its best quarter and its
  mean, written in the Calculator pane with its completions, previews and console, and saved as a node of your own
  with its versions; the language in full and its pitfalls. Its maps and numbers come from a run of Terraflux and
  its pictures of the application from `cargo test --release -p terraflux-app --test readme_images calculator --
  --ignored`.

### Changed
- Map algebra is faster, with the same results to the bit: an expression is compiled (the numbers folded, a value
  written twice computed once, the maps read in place) and run over blocks of cells in parallel, and `nbmin` and
  `nbmax` take the same time whatever the radius (van Herk and Gil-Werman, along the rows and then the columns).
  On 9 million cells with four threads, local expressions run 5 to 20 times faster, `nbmax(a, 5)` 18 times and
  `nbmin(b, 25)` 400 times (45 s to 0.11 s). `map.calc`, `value.calc`, `check.assert`, `uncertainty.delta`, the
  agents' rules and the tables' expressions share the evaluator.

## [1.15.0] - 2026-10-21

### Added
- Reproducing a run: every `terraflux run` writes `manifest.json` with its outputs (the version of Terraflux, the
  model and every input file with its SHA-256, the values, scenario and seeds, every output's SHA-256), and
  `terraflux reproduce manifest.json` runs it again and tells whether every output is the same, byte for byte.
- Testing a model: `[[tests]]` in the model name a value and the file of what it should be, within a tolerance;
  `terraflux test` runs models (or folders of them) and checks their tests and `check.assert` nodes, and
  `--update` writes the expected files from the run.

## [1.14.0] - 2026-10-21

### Added
- Debugging a model: a node turned off without being taken away (`bypass`; B on the canvas): it does not run, each
  of its outputs the first of its inputs of the same type, its box faint; how long every node took and about how
  much memory what it gave holds (a loop's body summed over its steps), in the node's pane and in
  `terraflux run --profile`; and `check.assert`, a check that stops the run when its expression is false (in every
  cell with a value, for maps), with its message and what failed, or only warns.

## [1.13.0] - 2026-10-21

### Added
- `control.switch`: one branch of a model run, chosen by its case, a group per branch in its body (the one named as
  the case, `case_<n>` for a number, `otherwise` for the rest), its outputs the branch's, every branch giving the
  same ones; with the case an input, a parameter chooses the method and each choice is a scenario. `value.calc`: a
  number from an expression in map algebra's syntax over the node's inputs (a table's value as `table.column`), and a
  text with them in it (`out/{species}_{step}.tif`), for the settings that read values. The template Choose the
  method: a switch and its scenarios.

## [1.12.0] - 2026-10-21

### Added
- `control.each`, a loop over a collection: its body runs once for each layer of a stack (a folder of rasters, the
  bands of a file), item of a series, row of a table, feature of vectors or value of a list, with `@item`, `@index`
  and `@name` inside (its other inputs as a group's); what it collects is gathered, maps as a stack of the elements'
  names, tables as one table with a column `item` first, the rest as a series. The template For each year: one
  analysis per map (control.each).

### Changed
- An input of a value that a run is not given takes the model's own value in the engine too (as the command line
  and the application already did).

## [1.11.0] - 2026-10-21

### Added
- Settings that read values: a setting holds a value of its own or reads one when the model runs, `"$input"` (an
  input of a value), `"@step"` or `"@carried"` (a loop's step or carried value, a group's input) or
  `{ from = "node.port" }` (another node's number, true or false, or text). A threshold changes with the loop's
  step, one input feeds several settings, and a submodel shows its settings as inputs (a setting reading
  `"$threshold"` is given by the node that uses it). The node read runs first and counts in the cache; a map or a
  table read is refused when the model is checked; renaming, copying, grouping and removing nodes follow the
  references. In the application, a right click on a setting's name reads a value of the model, and the row shows
  what it reads.

## [1.10.0] - 2026-10-21

### Added
- Hydrography90m and Environment90m, the river network of the world at 90 m and its environment per
  sub-catchment: `data.download` with `source = "hydrography90m"` reads only the window of the grid from the tiles of
  any of its variables (sub-catchments, segments, basins, outlets, flow directions and accumulation, stream orders,
  slopes, distances, flow indices); `data.environment90m` gives the climate (CHELSA, observed and projected), land
  cover (ESA CCI by year), soil (SoilGrids), elevation (MERIT DEM), aridity (CGIAR) and hydrography of every
  sub-catchment of a map (their min, max, range, mean or sd), the tables of the tiles downloaded once into the cache,
  and a column on the map.

## [1.9.0] - 2026-10-21

### Added
- The connectivity of rivers, as R's riverconn and hydrographr: `hydro.dci` splits the network into segments at the
  barriers (dams, weirs, culverts: points with their passability up and down) and gives the dendritic connectivity
  index of each network (Cote et al. 2009; potamodromous and diadromous, symmetric or asymmetric), the share of its
  network a fish can reach from every stretch, and for every barrier the DCI without it and its gain (riverconn's
  d_index), ranked: which dams to remove first; the same values as riverconn 0.3. `hydro.network_distance` gives the
  distance along the river between every two points, `hydro.along_network` the stretches up and down from each point
  within a distance, and `hydro.centrality` the degree, Shreve magnitude, river above, betweenness and closeness of
  every link. The template Rivers: dams and their connectivity (DCI).

## [1.8.0] - 2026-10-21

### Added
- Rivers as a network, as hydrographr does on the Hydrography90m: `hydro.snap` moves points (gauges, dams,
  records of a species) onto the streams, to the nearest stream cell or the nearest of a river with a flow
  accumulation of at least a minimum; `hydro.upstream` summarises the land upstream of every link of
  `hydro.stream_links`, or of every point, its cells and area, the count, sum, mean, min, max and standard deviation
  of maps and the share of each class of a map (the forest upstream of each stretch), with a column painted on the
  links; `hydro.upstream_catchment` gives the catchment of every point as a polygon with the point's attributes, and
  the map of the catchments nested between one point and the next. In Rust, over the D8 directions of `hydro`. The
  template Rivers: the land upstream and the gauges' catchments.
- Packages for Fedora (`.rpm`, also openSUSE) and Arch Linux (`.pkg.tar.zst`), with the `.deb`: the same folder in
  `/opt/terraflux`, the command in `/usr/bin`, an entry in the menu (`tools/make-linux-package.py`, with
  [nfpm](https://nfpm.goreleaser.com)). At every release each Linux package is installed in a container of Ubuntu
  24.04, Debian 13, Fedora or Arch and runs a model with its GDAL before it is published; the website's downloads
  list them. The release workflow run without a tag builds and tries the packages without publishing them.

### Changed
- The Assistant pane looks like a terminal: the conversation in a fixed font on one ground, each line marked in its
  gutter (❯ the question, ● the answer and each tool called, coloured by how it went, └ the first line of its
  result, a click opening it whole, ∴ the thinking); a prompt where enter sends, up and down bring back what was
  typed and esc stops the answer; commands after a slash: /new, /stop, /review, /accept, /reject, /explain, /setup
  and /help.

## [1.7.2] - 2026-10-20

### Added
- The packages of every release are built by GitHub Actions at its tag `vX.Y.Z` (`.github/workflows/release.yml`):
  the MSI and the zip for Windows, the disk image and the zip for macOS (Apple silicon), the `.deb` and the
  archive for Linux, published as the tag's release with its changes and their checksums (`SHA256SUMS.txt`).
  The website is published on GitHub Pages after each release, its downloads those of the latest one.

### Fixed
- Terraflux builds on macOS again: landschaft's reading of SWAT+ files used `std::from_chars` of reals, which
  Apple's libc++ does not have (`strtof`/`strtod` there, the same values).

## [1.7.1] - 2026-10-19

### Fixed
- Vector layers draw as quickly zoomed in as zoomed out. A tile no longer goes through every vertex of the
  features over it: the runs of their vertices off the tile, or smaller than a pixel, are drawn as one line each.
  A tile deep in a municipality of 100 000 vertices is drawn about 30 times quicker.

### Changed
- On macOS the release build's line tables go into a `.dSYM` (`.cargo/config.toml`), not into object files left
  in `target/` (about 2 GB at each build). `tools/make-macos-app.py` removes the workspace's crates of earlier
  builds before building, which cargo never does.

## [1.7.0] - 2026-10-19

### Added
- A node that takes one map, given many layers (a stack, the bands of a file, a folder of maps, a loop's series),
  runs once per layer: its maps come out as a stack of the same layer names (a series for a loop's), its tables
  as one table with a column `layer` (the year of names such as `forest_2010`), its other values as a series.
  Stacks given together are paired layer by layer. The canvas marks such a node ×N.
- `raster.write` with `{name}` in its path writes a file per layer, named after it
  (`mspa/{name}_mspa.tif`).

### Changed
- A file of several bands connected to a port of one map no longer gives that port its first band only: the
  node runs on every band. `raster.read`'s `band` chooses one. Where a port still takes one map alone
  (`stack.of`, `agents.parcels`, the inputs of groups), the bands left out are told in a warning.
- The rasters the preparation operators make without a `path` are kept in Terraflux's cache (`prepare`), made
  once and used again while their sources and settings are the same, instead of a new copy at every run in the
  temporary folder; the old copies there are removed. The caches of rasters made, of data downloaded and of the
  workers are held under 10 GB each (`TERRAFLUX_CACHE_GB`, **Settings > Cache**, which also empties them).
- The tiled engine's layers on disk (`--disk`) are removed as soon as the run no longer needs them, instead of
  every step's staying after the run.
- Builds take four jobs at a time (`.cargo/config.toml`), and landschaft's C++ is built once per profile
  (`target/<profile>/landschaft`), not again for every change of the crate's hash.

## [1.6.0] - 2026-10-18

### Added
- Statistics of rasters: `raster.stats` (a map, or every layer of a stack or of a series, summarised as a table:
  count, nodata, sum, mean, std, min, percentiles, max..., and a histogram), `stack.reduce` (a statistic of the
  layers at every cell: mean, std, min, max, median, sum, count, majority, a percentile, the layer of the largest
  or smallest), `raster.zonal` (the statistics of a map in the zones of a map of zone ids, as a table and as a map
  of every cell given its zone's), `raster.tabulate` (the cells, area, hectares or share of every class in every
  zone), `raster.correlation` (Pearson's or Spearman's correlation and the covariance between maps, the matrix of a
  stack, the line of each pair), `raster.local_correlation` (the correlation, slope, intercept and r² of two maps
  in a moving window) and `raster.standardize` (z-score, min-max or a percentile stretch, with each layer's centre
  and scale); the library's subject Raster analysis, and the template Raster statistics: summaries, zones and
  correlation.
- Neighbourhoods and the cleaning of maps of classes: `raster.focal` (a statistic in a moving window around every
  cell: mean, median, majority, std, variety, range, a percentile...; a square, a circle, an annulus or a Gaussian
  kernel), `raster.majority` (the majority filter, as ArcGIS's), `raster.sieve` (the patches under a size merged
  into their largest neighbour, as gdal_sieve), `raster.label` (the patches numbered, as r.clump, with a table of
  their cells and area), `raster.expand` and `raster.shrink` (classes grown or shrunk a number of cells); the
  template Cleaning a map of classes: filter, sieve, label and grow.
- Vector tools: `vector.polygonize` (a map of classes as polygons, a patch each with its holes, as
  gdal_polygonize), `vector.filter` and `vector.calc` (features kept or given an attribute by an expression over
  their attributes), `vector.geometry` (area, perimeter, length and centroid; geodesic areas), `vector.overlay`
  (intersection, union and difference of two layers of polygons, as geopandas' overlay), `vector.spatial_join` (the
  attributes of the feature met, or the features met counted and their attributes summed and averaged: the points
  in every polygon), `vector.reproject`, `vector.from_table` (points from a table's x and y) and `vector.points`
  (random or regular points inside polygons); the library's subject Vector analysis, and the template Vector tools:
  polygons, overlay and points.
- Classic spatial analysis: `vector.density` (the kernel density of points, quartic as ArcGIS's or triweight,
  Epanechnikov, triangular, uniform, per square unit, hectare or km², Silverman's radius by default; or the points
  in each cell counted), `raster.contour` (contour lines at an interval or at levels, as gdal_contour),
  `raster.viewshed` (how many observers see each cell, their height and the target's, a reach, the earth's
  curvature), `raster.cell_area` (the true area of every cell, on the WGS84 ellipsoid for longitudes and latitudes)
  and `raster.classify` (quantiles, equal intervals, Jenks's natural breaks, standard deviations or breaks given,
  with a table of the classes); the template Spatial analysis: density, contours, viewshed and classes.
- A cell explained by an assistant through `terraflux mcp` (Claude Code, Claude Desktop...): Explain with an MCP
  assistant in a cell's menu (a right click) keeps its trace for it and copies a request to paste; the server's
  prompt `explain_cell` (a slash command in Claude Code) brings it the trace and the question.

### Fixed
- The menu of a right click on a map's cell went as soon as the pointer moved to it: it stays now until an item is
  chosen (or a click elsewhere).

## [1.5.0] - 2026-10-18

### Added
- Tables worked on, as dplyr and tidyr do (the tables of metrics, transitions, zonal statistics, evaluations):
  `table.filter` (the rows where an expression is true: class == 3 and area > 10), `table.calc` (a column computed
  from the others, the expressions of map algebra), `table.aggregate` (grouped by columns and summarised: sum, mean,
  count, min, max, sd, median), `table.join` (two tables by key columns: left, inner, full), `table.sort` (sorted,
  the first rows kept), `table.select` (columns chosen, dropped, renamed) and `table.pivot` (long to wide, as the
  transitions into a matrix, and back); the library's subject Tables, and the template Tables: filter, summarise,
  join and pivot.

## [1.4.0] - 2026-10-18

### Added
- Why a cell has its value (the Trace pane): a cell of a map clicked, then Trace this cell, and the chain of values
  that made it, back through the graph: a card per node with its value there and how it was made (map algebra's
  expression with the values in place of its names, the window of cells a neighbourhood's operator reads, the
  layers a model weighs, a loop's value step by step, through groups), its inputs a click away, a sentence of it at
  the top. Trace inside the loops runs the model again keeping what their bodies give at the cell, step by step.
- The trace with the assistant: Ask the assistant why sends it to the chat, to be explained in words; the tool
  `trace_cell` (in the application, and in `terraflux mcp` with `inside_loops`) gives an assistant the same chain,
  and the assistants are told to explain results from it.
- A right click on a cell of a map (the viewer, a map's tab): Trace this cell, or Ask the assistant why. The last
  trace made is kept for an assistant through `terraflux mcp` too (its tool `last_trace`).
- Why a node failed (the Problems pane): the last run's failure explained (the node and its message, what it was
  given side by side: where each input comes from, what it is, its grid's size, cells and coordinate system, what
  differs from the first marked; what likely does not fit and what puts it right, raster.match or
  raster.reproject), then the graph's checks, the data's findings and the run's warnings, each a click away from
  its node. Opened from the status bar ("the run failed: why?", the problems counted) and from the node's pane (Why
  it failed); Ask the assistant sends a failure, explained, to the assistant. The assistant is told the same when a
  run fails, and `run_model` (in the application and in `terraflux mcp`) answers a failure with it.

### Fixed
- A pane brought to the front by another while the panes were drawn did not come (the assistant's Changes, among
  others).
- The assistant's text ran out of its pane's side when something in it was wider (a tool's call): the panes that
  scroll by themselves (the assistant, the document, a trace, the parameters, the changes, the problems) wrap at
  their sides; a tool's call is cut to the pane's width, and "fix problems" is beside Send.

## [1.3.0] - 2026-10-18

### Added
- The assistant is told of the problems that come up, for it to fix them: in a conversation going on, a run that
  fails (whoever started it) and the model's errors when an answer of its ends are sent to it on their own, each
  once and a few times one after another at most until the user writes ("fix problems" in its bar turns it off).
- The collections of favourites are named (a right click on one), not only by their colours.

### Changed
- The panes as they first are: the node's pane (and Prepare) at the left of the model, nothing at the right; the
  files in a drawer of their own at the window's left edge, as the operators (a tab each on the strip); the
  assistant at the right when it opens, not with the node's pane. A layout kept of another version is let go.
- The operators' pane begins with the favourites: the buttons to open rasters, vectors and folders and to save a
  raster are gone (File, Open a raster or vectors, ctrl+O, and the Files drawer open data), and GDAL's version is
  in the Settings.
- Tab goes between the model and the last map seen (a map's tab or the viewer), and from any other pane to the
  model.
- `stack.layer` is `stack.extract_layer` (a model that names the old one reads as before).
- The run's progress: the whole run (its nodes done, and the share of the one running: a loop's steps, what its
  operator tells), begun anew at every run and gone once it ends; its words beside the bar, readable whatever the
  bar shows under them (the bars of a node's progress and of Many runs too).

### Fixed
- A GeoTIFF of a projection of its own (no EPSG code: a country's Albers, ArcGIS's projections) had no coordinate
  system for the basemap and the warping: its WKT now comes from GDAL. And the basemap's "no CRS" stayed after a map
  of one was shown again.
- The run's progress bar showed the last loop of an earlier run, and did not move in a model without loops.

## [1.2.0] - 2026-10-18

### Added
- A model read in stages. ctrl+G puts the nodes selected into a group (what read them from outside reads the
  group's outputs, named after them; refused when it would make a cycle) and ctrl+shift+G takes a group apart;
  **Group the model by stage** (ctrl+K) puts the top level in a group per stage (data, preparation, model,
  validation, results), each with a note, the plots, files written and random generators left outside, and lays it
  out again. The canvas shows the groups as blocks linked by what their bodies read (a thinner line), their notes
  under them; zoomed out, a model without groups shows its stages framed and named. The Document pane and the
  reports read a section per group (its note under its title), what a group gives named by the node of its body
  that gives it, and the plots of a group's output as the figures of that node.
- The tools of `terraflux mcp` and of the assistant: `group_nodes`, `ungroup` and `group_by_stage`; the assistants
  are told to put a model built in a group per stage.
- Parameters as a form (the Parameters pane): the values a user is to change, kept in the model (`parameters`,
  optional in `terraflux/1`: an input of a value or a node's setting or literal input, with its label, unit,
  range, step, choices and help; a setting cannot read an input, so a parameter names it directly). A slider for a
  number in a range, a list for a few choices, a box for yes or no, a field for the others; each change an edit
  (ctrl+Z) and the model run again. A setting of the model added from a list, its label and help taken from its
  operator's description.
- Scenarios (`scenarios` in the model: named sets of the parameters' values, with a note): the values now saved
  under a name, a scenario applied in a click, sent to Many runs as its table of scenarios, the parameters of a
  range sent to its calibration, and scenarios compared side by side: each run to an output chosen, its maps (or
  tables, or numbers) in a column per scenario with their differences from the first. `terraflux run --scenario
  name`; `set_parameter`, `remove_parameter`, `set_scenario`, `remove_scenario` and `run_model`'s `scenario` for
  the assistants. The Document pane and the reports list the parameters and the scenarios; `check` warns of a
  parameter or a scenario that names nothing the model has.

### Changed
- A group keeps what the nodes of its body gave with its outputs (as `node.port`, which no reference names): the
  Document pane and the reports show its steps' maps after a run.
- The document's order and stages are found in time proportional to the nodes and connections (the canvas uses
  them).

## [1.1.0] - 2026-10-18

### Added
- The model read as a document (the Document pane, a tab beside the model's): the same graph as the methods
  section of a paper, for whoever reads, checks and decides rather than builds. The top-level nodes in five stages
  (data, preparation, model, validation, results; the analysis when there is no model), each stage in the order of
  the dependencies with the ties broken by where the nodes are on the canvas; a step each, with its note, why,
  inputs, settings in words (the operator's own description, and the default when the value differs), map
  algebra's expression with its variables linked to their nodes, references, assumptions and alternatives; the
  data of one kind in a row, the plumbing (stacks, generators) in a line; a loop's or group's body within it, with
  what it carries, collects and gives; the plots as numbered figures of the steps they show; the maps, series and
  tables of the last run beside the steps. A step's name selects its node; a setting of a single value is edited in
  place (ctrl+Z undoes it) and the model runs again. The design, drawn first by hand on three models, is in
  `docs/DOCUMENT_VIEW.md`.
- Reports: the document written as one HTML page (its images in it, read without Terraflux) or as a PDF, from the
  pane, File, Export report, or `terraflux report model.toml [-o report.pdf] [--run]` (`--run`: the maps and tables
  of the steps in it).

## [1.0.0] - 2026-10-18

### Added
- The model as the record of its choices: optional fields of a node beside its `note` (`why`, `refs`,
  `assumptions`, `alternatives`) and of the model (`request`, `summary`, `author`), in the format `terraflux/1`
  (older models stay valid; none changes what a node gives, nor runs it again). The tools of `terraflux mcp` and
  of the assistant take them (`add_node`, `edit_node`, `new_model`, the new `set_model_info`); `add_node` asks for
  a note and a why when they are missing, and `check_model` advises of the nodes made without them (those copied
  from a template aside). The assistant signs what it changes (its model and server among the authors, the first
  question of the conversation as the request). The Node pane shows and edits the record, and the model's own when
  no node is selected; the canvas shows a node's why with its note.
- The review of a model's changes (the Changes pane), as a pull request's: what the assistant changes, or another
  program writes to the model's file (an assistant through `terraflux mcp`), listed as nodes added, removed and
  changed, with each connection, setting and record before and after, and the model's inputs, outputs and record;
  the maps a changed node gives before and now side by side; the changed nodes outlined on the canvas. Each change
  accepted or rejected (an edit that ctrl+Z undoes), or all at once; the assistant's pane says how many are to
  review. File, Compare lists the differences with the saved file, another file or the last commit (git).

## [0.99.0] - 2026-10-18

### Added
- The assistant (View, Assistant): a chat beside the graph with a model that explains, changes and runs the
  model open, with the tools of `terraflux mcp` called in the application. Each change is an edit of its own
  (ctrl+Z undoes it) with the node it touched selected; a run goes in the application, its maps shown there, and
  the assistant is told what the outputs gave. "Explain" asks about the node selected. The model is told the
  model open, its operators' documentation and the node selected.
- The assistant talks to any server of OpenAI's chat completions API with tool calling, streamed: a university's
  (the HU Berlin's KI-Service, from its network or VPN; the GWDG's Chat AI), one on this computer (Ollama,
  LM Studio) or a commercial provider (OpenAI, Anthropic). A server's address, model, key (kept in the system's
  store of credentials, never in the settings or a model), thinking (Qwen 3 on vLLM) and a small set of tools for
  small models; "Test the connection" checks that the server answers, has the model and lets it call tools.

### Changed
- The tools of `terraflux mcp` live in the application's library (`terraflux_app::tools`), shared by the server
  and the assistant; the server's tools and answers are as they were.
- The viewer opens in a tab behind the model's graph when the first data comes (a template, a file read): the
  graph stays in front. The assistant is a tab of the inspector at the right (the layout kept is reset once).

## [0.98.0] - 2026-10-18

### Added
- `conn.metapopulation`, a metapopulation in a network of habitat patches over time, Hanski's incidence function
  model as R's MetaLandSim 2.0 simulates it (spom), the same run for a seed (R's runif after set.seed): every step
  each patch's connectivity to the occupied ones (an exponential or power kernel of the distance times their
  areas, or times the patch's own), an empty patch colonised (logistic, exponential or power in the connectivity)
  and an occupied one lost (by its area, or with the rescue effect). The patches occupied at every step and each
  patch's incidence. The patches come as a table (x, y, area, occupied) or as points with those attributes.
- The template "Metapopulation (incidence function model)": a century in 60 drawn patches.

## [0.97.0] - 2026-10-18

### Added
- `sdm.migrate`, a species' dispersal through the habitat suitability of successive periods as MigClim 1.6
  simulates it, the same run for a seed (its random numbers are the C runtime's rand(), ported): colonisation from
  occupied cells within a dispersal kernel's reach, weighted by the source's propagule production while it matures
  and by the sink's suitability; weak or strong barriers; long-distance dispersal; cells that turn unsuitable kept
  until the period ends. The final state with MigClim's codes, the cells occupied, the state after every step and
  the statistics of every step.
- The template "Species migration (MigClim)": three periods of a moving climate, a river as a barrier.

## [0.96.0] - 2026-10-18

### Added
- Interpolation of points onto a map as R's gstat 2.1 does it: `stats.variogram`, the sample variogram of points
  (gstat's classes of distance up to a cutoff) and a nugget with a spherical, exponential or Gaussian structure
  fitted to it (fit.variogram's iterated weighted least squares, its Gauss-Newton steps, weights and stopping rule:
  the converged fits to the bit); `stats.kriging`, ordinary kriging with every point or the nearest, the prediction
  and its variance, under the model fitted or one given; `stats.idw`, inverse distance weighting. The points are
  features with their values in an attribute; the result takes the cells of a map.
- The template "Interpolation: IDW and kriging": points sampled from a drawn soil property, its variogram, kriging,
  inverse distance weighting and their errors.

## [0.95.0] - 2026-10-18

### Added
- Spatial statistics, a new subject of the library: `stats.trend`, the monotonic trend of every cell along a series
  of maps (the Mann-Kendall test with its variance corrected for ties, and Sen's slope with its intercept, as
  pymannkendall's original_test: the same numbers); `stats.moran`, the global Moran's I of a map with its inference
  under normality and randomisation and a pseudo p of permutations; `stats.lisa`, the local Moran's I of every cell,
  its quadrant, z and the clusters of a pseudo p of conditional permutations; `stats.getis_ord`, Getis and Ord's Gi*
  with its z, p and the hot and cold spots at 90, 95 and 99% (PySAL's esda, to rounding; rook or queen contiguity of
  the cells with a value, row-standardised). scipy's normal distribution (Cephes' ndtr, erf and erfc) to the bit.
- The template "Trend and hot spots": five years of a drawn NDVI, its trend, LISA's clusters and Gi*'s hot spots.

## [0.94.0] - 2026-10-18

### Added
- Global sensitivity analysis as SALib 1.6 does it, the same numbers: `sensitivity.design` (Morris's trajectories
  on a grid of levels, or Saltelli's runs of a scrambled Sobol' sequence, scipy.stats.qmc's: Joe and Kuo's direction
  numbers, the linear matrix scrambling and the digital shift) and `sensitivity.analyze` (Morris's μ, μ*, σ and μ*'s
  bootstrap interval; Sobol's first, total and second order indices with their intervals; scipy's normal quantile,
  Cephes' ndtri). `terraflux sensitivity model --param node.setting=from:to --method morris|sobol --output
  node.port` runs the design's runs here or on a coordinator's workers (clusters and clouds as `submit`), each
  measuring the output on the worker (a number, a table's column, a map's mean, sum, min or max), and writes the
  design with the outputs and the indices.

### Fixed
- `ml.partial_dependence` of a classifier of three classes or more is now scikit-learn's to the bit (numpy's mean
  down the columns of the probabilities adds in order), not to rounding.

## [0.93.0] - 2026-10-17

### Added
- Brazil's Forest Code (Lei 12.651/2012 with Lei 12.727/2012): `forest_code.app` (the Áreas de Preservação
  Permanente of art. 4: the margins of rivers from the channel's edge by their width, of natural lakes by their area
  (urban zones' 30 m), springs, veredas, slopes above 45°, hilltops, land above 1,800 m and protected classes such
  as mangroves; in the rural areas consolidated by 22 July 2008, art. 61-A's narrower margins by the property's
  fiscal modules) and `forest_code.reserve` (every property's Reserva Legal: art. 12's share, the native vegetation
  counted outside and, as art. 15 allows, inside the APPs, the deficit or surplus; art. 67's properties of up to 4
  fiscal modules keep the vegetation of 2008). Exact Euclidean distances (Felzenszwalb and Huttenlocher) with the
  nearest feature of every cell. The library's subject Forest Code (Brazil) and the template Forest Code: APP and
  Reserva Legal.

## [0.92.0] - 2026-10-17

### Added
- `data.download`: data downloaded on demand and warped onto a map's grid (or an extent of longitudes and
  latitudes), GDAL reading straight from the servers only the part the grid covers: WorldClim 2.1 (the 1970-2000
  climate, its monthly variables and elevation; the CMIP6 futures of a model, scenario and period), CHELSA 2.1
  (1981-2010 and CMIP6 futures), the Copernicus DEM (GLO-30 or GLO-90, its tiles found and mosaicked), SoilGrids (a
  property at depths) and MapBiomas Brazil (collections 9, 10 and 11, by year), or any raster a URL or path names;
  a file per layer in the cache, downloaded once.

## [0.91.0] - 2026-10-17

### Added
- Verra's VT0007, the unplanned deforestation allocation of a jurisdiction (REDD+), as Clark Labs' UDef-ARP:
  `redd.nrt` (the negligible risk threshold of the distance to the forest's edge), `redd.vulnerability` (the
  benchmark classes of geometric widths from that distance, or the alternative from a map of vulnerability),
  `redd.fit` (the relative frequency of deforestation by class and subdivision, and its density), `redd.allocate`
  (the density of another period adjusted to the expected deforestation, or to a period's observed one, never above
  a cell; a region the fit lacks takes its class's average) and `redd.assess` (the predicted against the actual
  deforestation in the cells of an assessment grid: the mean and median absolute errors, the intersection over
  union, R², the OLS and Theil-Sen lines); the same maps and tables as UDef-ARP's own functions.
- `carbon.emissions`: the carbon emitted and removed by land change over a series of maps (the IPCC's
  stock-difference method), a stock per class or a curve of regrowth (Chapman-Richards or linear) from the date a
  class appears in a cell; by period, by transition, and every cell's net emission.
- The library's subject Carbon and REDD+, and the template REDD+ baseline (VT0007).

## [0.90.0] - 2026-10-17

### Added
- What a fitted model learnt, as scikit-learn's inspection tells it, the same numbers: `ml.importance` (the
  permutation importance of each predictor: the drop of the accuracy or of R² when its values are shuffled among the
  records, numpy's RandomState shuffles; the mean, the standard deviation and every shuffle) and
  `ml.partial_dependence` (the mean prediction along a grid of a predictor's values: its distinct values, or from
  one percentile to another as scipy's mquantiles; the value, or the probability of each class).

## [0.89.0] - 2026-10-17

### Added
- Histogram gradient boosting (`terraflux_ops::ml::boosting`), scikit-learn's HistGradientBoostingClassifier (two
  classes or more) and HistGradientBoostingRegressor ported to the bit, without early stopping: the predictors
  binned by numpy's quantiles (from a subsample of 200,000 records when there are more), the gradients and hessians
  of the half squared error, binomial or multinomial loss as float32, the trees grown best leaf first from
  histograms (Python's heap breaking the ties of gain), the leaves shrunk by the learning rate; the same
  predictions as scikit-learn 1.9. `ml.fit` and `ml.potential` take it with `method = "gradient_boosting"`
  (`learning_rate`, `max_iter`, `max_leaf_nodes`, `l2_regularization`, `max_bins`, a share of the predictors per
  split); its importances are each predictor's share of the gain of its splits.

## [0.88.0] - 2026-10-17

### Added
- Machine learning (`ml.*`), scikit-learn's random forests ported to the bit (now `terraflux_ops::ml::forest`,
  shared with `sdm.fit`): `ml.fit` (RandomForestClassifier of any number of classes, or RandomForestRegressor of
  values, on the rows of a table or the cells of maps, a sample of them drawn as numpy draws it; the predictors'
  importances, the mean decrease of impurity), `ml.predict` (the class or the value and each class's probability, on
  maps or a table) and `ml.potential` (landschaft's Estimator: a forest per class of where it is, or where it
  appeared between two maps: PLUS's land expansion analysis, LEAS, the growth probability `cars.step` takes; with
  each driver's contribution), each the same numbers as scikit-learn 1.9 and landschaft. The trees are grown and the
  cells predicted in parallel (the same bits). A fitted model is a value of its own (`ml_model`), written as JSON.
  The library's subject Machine learning, and the template PLUS (LEAS and CARS).

## [0.87.0] - 2026-10-17

### Fixed
- `services.habitat_quality`'s `threats`, when it names InVEST's threats table, is taken from the model's folder
  (and kept relative to it when the model is saved), as the `path` of the operators that read files; it was read
  from the folder the program ran in (`terraflux_graph::FILE_SETTINGS`).

### Added
- The accuracy and the area of a map from a stratified random sample, the good practices of Olofsson et al. (2014):
  `accuracy.sample_size` (the size that gives the overall accuracy a standard error, from the user's accuracy
  expected of each class, and its share of each stratum: a minimum in every one, the rest in proportion; in
  proportion, equally or by Neyman's allocation), `accuracy.sample` (the points drawn in each stratum without
  replacement as numpy's `default_rng(seed).choice`, to be labelled with their reference class) and
  `accuracy.assess` (the error matrix in shares of the area, the user's, producer's and overall accuracies and each
  class's area estimated from the sample, with standard errors and confidence intervals; R's mapaccuracy, its
  `olofsson` and, for strata of their own, `stehman2014`, to 1e-12). The library's subject Map accuracy and area,
  and the template Accuracy and area of a map.
- The website's tutorials of the 0.86 operators: Terrain hydrology (depressions and flow; streams, watersheds and
  HAND; wetness and connectivity; SWAT+), Ecosystem services with InVEST (the ideas, carbon, habitat quality, water
  yield, sediment), geomorphons and the hilltops of Brazil's Forest Code, and the Dinamica series' Patcher of a
  stock, roads that grow and gravity potentials.
- The website to read offline: `python website/build.py --offline` makes a copy that opens from the disk, without a
  server or a network (links to each page's index.html, the search as a script, KaTeX and the fonts within, the
  images in WebP only), and its zip (43 MB), to share.

## [0.86.0] - 2026-10-17

### Added
- Terrain hydrology (`hydro.*`, landschaft 1.50's `ls.hydro`, bit for bit): depression filling (Priority-Flood,
  with or without the epsilon that drains the filled areas) and least-cost breaching (WhiteboxTools'
  BreachDepressionsLeastCost); flow directions (D8 as ESRI's codes, D-infinity azimuths, pygeoprocessing's
  multiple flow directions packed as the InVEST models route); flow accumulation (D8, D-infinity, FD8, Quinn's,
  pygeoprocessing's MFD; in cells, area or specific contributing area; with weights); streams by a threshold or
  traced as InVEST traces them; the distance to them along the MFD; HAND; stream links with their table
  (downstream link, length, elevations, Strahler order) and outlets snapped to them; watersheds; the wetness,
  stream power and sediment transport indices; the index of connectivity of Borselli et al. (2008).
- Ecosystem services, InVEST's models (natcap.invest 3.20) as landschaft reimplements them, the same float32 bits:
  `services.carbon` (storage, its pools, the sequestration and its net present value, the totals),
  `services.habitat_quality` (degradation by threat and in all, quality, rarity; access from a raster or
  polygons), `services.water_yield` (the Budyko curve, actual and potential evapotranspiration, the tables of
  watersheds and subwatersheds with consumption, realized supply and hydropower) and `services.sdr` (USLE, the
  sediment delivery ratio, export, deposition, avoided erosion and export, by watershed). numpy's own float32
  exp, sin and cos (its AVX2 kernels, not the C library's) are ported for them (`terraflux_ops::npmath`).
- Landforms and potentials of a surface (landschaft 1.50's, bit for bit): `surface.geomorphons` (GRASS's
  r.geomorphon 8.4 with four of its errors corrected: the ten landforms, named and in GRASS's colours in the viewer,
  the ternary and binary patterns, the measures, the share of each form), `surface.hilltop` (the hilltops of
  Brazil's Forest Code, APP de topo de morro of Lei 12.651/2012: every summit with its base, height and slope, the
  protected upper part of the hills that qualify) and `surface.gravity` (Dinamica EGO's
  CalcInteractionPotentialMap, by direct sums or an FFT).
- Dinamica EGO's functors of landschaft 1.50, bit for bit: `dinamica.continuous_patcher` (PatcherContinuousState:
  patches that take a stock, biomass or timber, up to a quota, step by step) and the road constructor of the Amazon
  models: `dinamica.cost_map` (CalcCostMap), `dinamica.destinations` (CalcDestinationMap), `dinamica.pathways`
  (CalcPathwayMap) and `dinamica.roads` (a step of road building, with the distance to the roads as a dynamic
  variable).
- SWAT+ (landschaft 1.50's port of SWAT+ 62.0.1, bit for bit): `swat.run` simulates a project (a TxtInOut folder,
  or a zip of one) and gives the tables SWAT+ prints at a step (day, month, year, average annual): the water,
  nutrient, losses and plant balances of the HRUs and of the basin, the SWAT-DEG channels, the aquifers, and the
  daily weather of each station; a spin-up, MUSLE's C of SWAT+ 62 or of the plants. Its folder is a file setting
  (chosen in the Node pane, kept relative to the model).
- The library's subjects Terrain hydrology and Ecosystem services.

### Changed
- landschaft is at its 1.50.0 (the submodule; its `hydrocore` and `servcore` linked).
- Debug and release builds keep line tables only (backtraces with their lines), the dependencies of debug builds
  no debug information; and the integration tests of `terraflux-ops`, `terraflux-exec` and `terraflux-app` are
  one program per crate (`tests/main.rs`, a module per file; `circuit_ini`, `ui` and `readme_images` programs of
  their own), not one per file each linking landschaft's C++ whole: `target` had grown to 400 GB. A test is now
  run as `cargo test -p terraflux-ops --test ops hydro::`.

## [0.85.0] - 2026-10-17

### Changed
- The operators' pane closed, a strip at the window's left edge says they are there ("OPERATORS ›" along it): the
  pointer over it brings their drawer out, a click opens their pane.
- The Node pane of an operator of many settings shows its essential ones and folds the others as advanced
  ("Advanced (39)"; with the count of those the model gives, "39 · 1 given", open then): the inputs and settings
  most models set, listed for the 34 operators of ten settings or more in `terraflux_ops::essentials` (a test keeps
  every name one of its operator's); the boxes of the canvas show the essential inputs too. A setting is a row: its
  name at the left (what it does on hover), its value at the right, the empty field hinting its default ("default
  0.1"); a dot marks the settings the model gives, and a right-click on the name takes one back to its default.
  The inputs are rows too, what feeds them cut to the pane.

## [0.84.0] - 2026-10-17

### Added
- The palette of commands (ctrl+K, ctrl+shift+P; View > Commands): everything the application does, found by
  typing: its actions (run, save, arrange, undo...), the panes, the models opened last, the templates, the nodes of
  the scope shown (to go to one) and the operators (to add one after the node selected). The arrows choose, enter
  does it.
- The keyboard's shortcuts in a window of their own (F1; View > Keyboard shortcuts), by where they work.
- File > Open recent: the ten models opened or saved last. New shortcuts: ctrl+N (a new model), ctrl+shift+O (open
  a model), ctrl+shift+S (save it as).

### Changed
- The bar at the top in three parts: the menus at the left (File: new, open, recent, save, save as, open data;
  Templates; Operators; View), running the model in the middle (Run, Stop, the model's name with a dot while it
  has edits not saved, how the run goes), the tools at the right (Wizard, Many runs, Computers) and the settings
  in a gear. The window's title names the model, with the dot too.
- The status bar says what the keys and the mouse do in the pane in focus (the canvas's, the maps', else the
  application's) instead of every shortcut at once; "not saved" left it for the title.
- The accent is kept for the main action (Run) and for the node selected on the canvas: what is selected in a
  list or a tab is the accent softened into the panel, "Open a raster..." and "Run to here" are plain buttons,
  and the place in the model is a trail of links (Top level › growth) rather than buttons.
- The Node pane puts first what is edited: the problems found in the node under its name, then the first sentence
  of what its operator does (the rest a click away), its inputs (those it needs and those given; the optional
  others folded, "Optional inputs (9)"), its settings named in words ("Gray threshold"; the name and what it does
  on hover), a container's body, and last the note of why it is there. A setting that is a list of numbers or texts
  (`weights = [2.0, 1.0, 1.0, 1.5]`) or a table of them by name is edited item by item, with a row to add one and
  a button to take one away; `{ }` edits it as text.
- The canvas: a box shows the inputs its node needs and those given, its optional others folded in a row ("+ 10
  optional", the names on hover) that a click unfolds and folds again; a connection let go on a node rather than on
  an input goes to the free input it fits, or to the one chosen in a list when several do. A node with problems (an
  input it needs not given, the data's, the run's error) has a mark, red or amber, that tells them on hover; a
  port's hover tells its type and whether it is needed. Zoomed out, the nodes' names stay readable (never under
  11 points, in the middle of their boxes, cut to them) where they used to vanish under 35%. Undo and Redo are
  named (their arrows were not in the font).
- The panes as they first are: the model in most of the window and the node selected in an inspector at its right
  (the preparation and the files in tabs beside it); the operators in their drawer at the window's left edge, the
  Operators menu, "/" and the commands, their pane, shown from View, taking the left edge (the layout kept of
  earlier versions is let go once). The collections of favourites are named, with their counts ("■ Red 1"), only
  those with operators shown, and a word on how to fill one when none has any.
- Every theme's text, weak text included, reads at 4.5:1 at least on its panels, canvas and bars (WCAG's AA, kept
  by a test): the weak text of Default, Immaterial and Riparian stronger, Immaterial's accent darker (white on it),
  the light themes' warnings, errors and links darker; the operator, the file and the folded inputs written on a
  node's box between the weak and the plain text. A second light theme, **Field Notes**: warm paper and a forest's
  green, for daylight and projectors.

## [0.83.0] - 2026-10-16

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

### Fixed
- A model run in the application or by `terraflux run` (each in a thread of its own) gave its OpenMP kernels and
  solvers one thread, OpenMP's for a new thread: they have the threads in use again (every core but one by
  default), as landschaft's.

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
