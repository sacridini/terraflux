# Changelog

All notable changes to Terraflux are documented here. The project follows
[Semantic Versioning](https://semver.org/); before 1.0 the model format and the API may still change between
minor versions.

## [Unreleased]

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
