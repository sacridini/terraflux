# Python in the tutorials: the checklist

The checklist of the "Python in the tutorials" block (a `### In Python` section in every tutorial part, rule in
`AUTHORING.md`). Delete this file when the block is done. It is outside `content/`, so the site does not build it.

How it was made: every ```` ```tfgraph ```` block of every part (not the `*-in-python` pages, not
`object-based-image-analysis`; blocks with `#! fragment: true` are skipped, as `build.py --check` does) was written
to a TOML model and given to the MCP tool `python_script` of `terraflux.exe mcp` (1.60.0), whose summary says which
nodes have no counterpart in the library (a node that only depends on one of those is listed as "downstream"). A part's coverage is `full` if every node of every model has one, `none`
if no node has, `partial` otherwise, `no graph` if it has no complete model (only fragments, or none). The guesses of
section to part are by title and description only. Counts: animal-movement 7/0/1/0; biodiversity 3/0/0/0; building-models 8/1/0/1; change-detection 2/2/0/0; circuit-connectivity 4/0/0/0; distance-restoration 2/2/0/2; ecosystem-services 8/0/0/0; fragmentation 2/1/0/0; getting-started 5/2/0/0; grid-distances 2/0/0/0; habitat-networks 6/1/0/0; habitat-patterns 4/0/0/0; land-change 8/17/0/3; landscape-metrics 6/1/0/0; large-data 2/0/0/0; neutral-landscapes 2/0/0/0; spatial-prioritization 5/2/0/0; species-distributions 7/0/0/0; surface-metrics 2/2/0/0; terrain-hydrology 3/2/0/0; uncertainty 1/0/0/0 (full/partial/none/no graph).


## animal-movement

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `cleaning-and-measures.md` | — | full | `animal-movement-in-python.md` | |
| `cleaning-and-thinning.md` | — | full | `animal-movement-in-python.md` | |
| `corridors-and-validation.md` | `data.download` (downstream: `circuit.omniscape`, `raster.reclassify`, `raster.reproject`) | none | `animal-movement-in-python.md` | |
| `home-ranges.md` | — | full | `animal-movement-in-python.md` | |
| `ideas.md` | — | full | `animal-movement-in-python.md` | |
| `projections-and-interpolation.md` | — | full | `animal-movement-in-python.md` | |
| `step-selection.md` | — | full | `animal-movement-in-python.md` | |
| `steps-and-summaries.md` | — | full | `animal-movement-in-python.md` | |

`animal-movement-in-python.md`:
- ## Reading the tracks → cleaning-and-measures ?
- ## Steps → steps-and-summaries
- ## Thinning, interpolating, drawing → cleaning-and-thinning, projections-and-interpolation ?
- ## The landscape → step-selection ?
- ## Regular steps, in bursts → steps-and-summaries ?
- ## How each animal moved → cleaning-and-measures ?
- ## Random steps → step-selection ?
- ## The landscape at every step → step-selection
- ## Validation → corridors-and-validation ?
- ## Next steps → none (links)
- ## References → none


## biodiversity

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `phylogenetic-endemism.md` | — | full | `biodiversity-in-python.md` | |
| `regions.md` | — | full | `biodiversity-in-python.md` | |
| `richness-and-endemism.md` | — | full | `biodiversity-in-python.md` | |

`biodiversity-in-python.md`:
- ## From records to a BaseData → richness-and-endemism ?
- ## Indices in moving windows → richness-and-endemism ?
- ## Is it more than chance? Randomisations and CANAPE → phylogenetic-endemism
- ## Regions: cluster analysis → regions
- ## Saving and exporting → none (all parts) ?
- ## Going further → none (links)


## building-models

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `calculator.md` | — | full | — | |
| `checks-and-tests.md` | — | full | — | |
| `groups-and-submodels.md` | — | full | — | |
| `loops.md` | — | full | — | |
| `map-algebra.md` | — | full | — | |
| `plots-and-tables.md` | — | full | — | |
| `plugins.md` | `plugin.wasm` | partial | — | |
| `randomness.md` | — | full | — | |
| `stacks-and-layers.md` | — | full | — | |
| `the-model-document.md` | — | no graph | — | |

No `*-in-python.md` page in this category.


## change-detection

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `comparing-maps.md` | — | full | `change-detection-in-python.md` | |
| `forest-gain-and-loss.md` | `change.fad` | partial | `change-detection-in-python.md` | |
| `intensity-analysis.md` | — | full | `change-detection-in-python.md` | |
| `transitions.md` | `change.suggest` | partial | `change-detection-in-python.md` | |

`change-detection-in-python.md`:
- ## Gains and losses → transitions ?
- ## Structural change vs. boundary noise → forest-gain-and-loss ?
- ## Shifts between fragmentation classes → forest-gain-and-loss ?
- ## Several scales at once → comparing-maps ?
- ## Change of the land-use mix → intensity-analysis ?
- ## Next steps → none (links)


## circuit-connectivity

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `circuit-theory.md` | — | full | `circuit-connectivity-in-python.md` | |
| `circuitscape.md` | — | full | `circuit-connectivity-in-python.md` | |
| `omniscape.md` | — | full | `circuit-connectivity-in-python.md` | |
| `resistance-surfaces.md` | — | full | `circuit-connectivity-in-python.md` | |

`circuit-connectivity-in-python.md`:
- ## A resistance map → resistance-surfaces
- ## Omnidirectional connectivity (Omniscape) → omniscape
- ## Between focal points (pairwise) → circuitscape
- ## Your own sources and grounds (advanced mode) → circuitscape ?
- ## One-to-all and all-to-one → circuitscape ?
- ## Existing projects and networks → none (reading Circuitscape/Omniscape projects) ?
- ## Good practice → circuit-theory ?
- ## Next steps → none (links)


## distance-restoration

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `costs-and-corridors.md` | — | full | `distance-restoration-in-python.md`, `restoration-plans-in-python.md` | |
| `distance-to-habitat.md` | `distance.proximity` | partial | `distance-restoration-in-python.md`, `restoration-plans-in-python.md` | |
| `ecological-intactness.md` | — | full | `distance-restoration-in-python.md`, `restoration-plans-in-python.md` | |
| `original-vegetation-and-habitat.md` | `sdm.area_of_habitat` | partial | `distance-restoration-in-python.md`, `restoration-plans-in-python.md` | |
| `restoration-plan.md` | — | no graph (+1 fragment) | `distance-restoration-in-python.md`, `restoration-plans-in-python.md` | |
| `several-benefits.md` | — | no graph (+3 fragment) | `distance-restoration-in-python.md`, `restoration-plans-in-python.md` | |

`distance-restoration-in-python.md`:
- ## Distance to the habitat edge → distance-to-habitat
- ## Restoration status → original-vegetation-and-habitat ?
- ## From distance to resistance → costs-and-corridors ?
- ## Cheapest routes between two habitat areas → costs-and-corridors
- ## Cost distance and least-cost paths → costs-and-corridors
- ## Zones of influence → ecological-intactness ?
- ## Next steps → none (links)

`restoration-plans-in-python.md`:
- ## How the model works → restoration-plan
- ## The data → restoration-plan
- ## Step 1: the original vegetation → original-vegetation-and-habitat ?
- ## Step 2: the habitat of every species → original-vegetation-and-habitat ?
- ## Step 3: a first plan → restoration-plan
- ## Step 4: more benefits → several-benefits
- ## Step 5: costs and budgets → restoration-plan ?
- ## Step 6: targets per region → restoration-plan ?
- ## Step 7: a priority gradient → restoration-plan ?
- ## Step 8: conserving and converting → several-benefits ?
- ## Step 9: uncertainty → restoration-plan ?
- ## Saving and sharing → none
- ## Going further → none (links)
- ## References → none


## ecosystem-services

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `carbon.md` | — | full | — | |
| `habitat-quality.md` | — | full | — | |
| `ideas.md` | — | full | — | |
| `nutrients.md` | — | full | — | |
| `pollination.md` | — | full | — | |
| `seasonal-water-yield.md` | — | full | — | |
| `sediment.md` | — | full | — | |
| `water-yield.md` | — | full | — | |

No `*-in-python.md` page in this category.


## fragmentation

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `change-and-reports.md` | `change.fad` (of the script generator only: the library has `tf.fad_multiscale_change`, used) | partial | `fragmentation-in-python.md` (removed) | done |
| `fixed-observation-scale.md` | — | full | `fragmentation-in-python.md` (removed) | done |
| `fragmentation-and-habitat-loss.md` | — | full | `fragmentation-in-python.md` (removed) | done |

`fragmentation-in-python.md` went into the parts (redirect `/tutorials/fragmentation/fragmentation-in-python/` to
`fragmentation-and-habitat-loss/#in-python`, `redirects:` of `site.yaml`):
- ## Habitat density in a moving window → fragmentation-and-habitat-loss
- ## Several observation scales → fragmentation-and-habitat-loss
- ## Fragmentation at a fixed observation scale → fixed-observation-scale (FOS in Python, % cover in Python)
- ## Landscape mosaic → fixed-observation-scale (the landscape mosaic in Python)
- ## Other window statistics → fixed-observation-scale (density, contagion and entropy in Python; % cover in Python)
- ## Choosing a window size → fragmentation-and-habitat-loss (the window in metres), fixed-observation-scale (the odd window)
- ## Next steps → none (links)


## getting-started

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `coordinate-systems.md` | — | full | — | |
| `first-model.md` | — | full | — | |
| `first-raster.md` | — | full | — | |
| `one-grid.md` | `raster.mosaic` | partial | — | |
| `reclassify-calculate-save.md` | `raster.fill` | partial | — | |
| `tour.md` | — | full | — | |
| `vector-data.md` | — | full | — | |

No `*-in-python.md` page in this category.


## grid-distances

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `cost-distance.md` | — | full | `grid-routes-in-python.md` | |
| `random-walks.md` | — | full | `grid-routes-in-python.md` | |

`grid-routes-in-python.md`:
- ## A transition: the raster as a graph → cost-distance
- ## Hiking on a volcano → cost-distance
- ## From the best route to all likely routes → random-walks
- ## Routes of dispersal from a common origin → random-walks
- ## Advanced options → none


## habitat-networks

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `connectivity-as-a-graph.md` | — | full | `habitat-networks-in-python.md` | |
| `connectivity-indices.md` | — | full | `habitat-networks-in-python.md` | |
| `importance-of-each-patch.md` | — | full | `habitat-networks-in-python.md` | |
| `least-cost-links.md` | — | full | `habitat-networks-in-python.md` | |
| `linkage-mapper.md` | `linkage.barriers`, `linkage.pathways`, `linkage.pinchpoints` | partial | `habitat-networks-in-python.md` | |
| `mspa-network.md` | — | full | `habitat-networks-in-python.md` | |
| `protconn.md` | — | full | `habitat-networks-in-python.md` | |

`habitat-networks-in-python.md`:
- ## How habitat networks work → connectivity-as-a-graph ?
- ## A network from a land-cover map → connectivity-as-a-graph
- ## Connectivity at several distances → connectivity-indices
- ## Which patches matter? → importance-of-each-patch
- ## Which links matter? → least-cost-links ?
- ## Connectivity of protected areas (ProtConn) → protconn
- ## Networks from habitat patterns → mspa-network
- ## More → linkage-mapper ?
- ## Next steps → none (links)


## habitat-patterns

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `morphology.md` | — | full | `patterns-in-python.md` | |
| `mspa.md` | — | full | `patterns-in-python.md` | |
| `networks-and-change.md` | — | full | `patterns-in-python.md` | |
| `spa-and-objects.md` | — | full | `patterns-in-python.md` | |

`patterns-in-python.md`:
- ## Morphological Spatial Pattern Analysis (MSPA) → mspa, morphology ?
- ## Simplified pattern analysis (SPA) → spa-and-objects
- ## Objects by size: accounting → spa-and-objects ?
- ## Parcels: how divided is the landscape? → spa-and-objects ?
- ## Contortion: how convoluted are the boundaries? → spa-and-objects ?
- ## Next steps → none (links)


## land-change

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `a-cellular-automaton-by-hand.md` | — | full | `land-change-simulation-in-python.md` | |
| `agents.md` | — | full | `land-change-simulation-in-python.md` | |
| `calibration-and-many-runs.md` | — | no graph | `land-change-simulation-in-python.md` | |
| `choosing-a-model.md` | — | no graph | `land-change-simulation-in-python.md` | |
| `clue-s-ideas.md` | `lulcc.history`, `rng.r` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `clue-s-model.md` | `lulcc.history`, `rng.r` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `clue-s-rules.md` | `lulcc.history`, `rng.r` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `clumondo.md` | `clumondo.ages` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `coupled-agents.md` | `crafty.allocate`, `lulcc.history`, `rng.r` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `crafty.md` | `crafty.allocate` | partial | `land-change-simulation-in-python.md` | |
| `dinamica-on-sibuyan.md` | — | full | `land-change-simulation-in-python.md` | |
| `dynamic-drivers.md` | `lulcc.history`, `rng.r` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `expander-and-patcher.md` | `dinamica.step` | partial | `land-change-simulation-in-python.md` | |
| `futures.md` | `futures.simulation` | partial | `land-change-simulation-in-python.md` | |
| `gravity-potentials.md` | `dinamica.step` | partial | `land-change-simulation-in-python.md` | |
| `land-demand.md` | `lulcc.history`, `plus.demand`, `rng.r` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `logistic-suitability.md` | — | full | `land-change-simulation-in-python.md` | |
| `maps-larger-than-memory.md` | — | no graph | `land-change-simulation-in-python.md` | |
| `molusce.md` | `molusce.step`, `molusce.total` | partial | `land-change-simulation-in-python.md` | |
| `ordered-allocation.md` | `lulcc.history`, `rng.r` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `plus-cars.md` | `plus.demand` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `regional-demand.md` | `lulcc.history`, `rng.r` (downstream: `control.repeat`) | partial | `land-change-simulation-in-python.md` | |
| `roads-that-grow.md` | `dinamica.step` | partial | `land-change-simulation-in-python.md` | |
| `taking-a-stock.md` | — | full | `land-change-simulation-in-python.md` | |
| `transition-rates-markov-chains.md` | — | full | `land-change-simulation-in-python.md` | |
| `validating-a-simulation.md` | — | full | `land-change-simulation-in-python.md` | |
| `weights-of-evidence.md` | — | full | `land-change-simulation-in-python.md` | |
| `what-land-change-models-are.md` | `change.suggest` | partial | `land-change-simulation-in-python.md` | |

`land-change-simulation-in-python.md`:
- ## The data → none (shared data)
- ## What changed → transition-rates-markov-chains ?
- ## How much: the demand → land-demand
- ## Where: the transition potential → logistic-suitability, weights-of-evidence ?
- ## With what shape: four allocations → expander-and-patcher, ordered-allocation ?
- ## One call: fit, check, project → clue-s-model ?
- ## Validation → validating-a-simulation
- ## Calibration → calibration-and-many-runs
- ## Scenarios and replicates → calibration-and-many-runs
- ## Variables that change with the map → dynamic-drivers
- ## Roads that grow with the model → roads-that-grow
- ## Taking a stock → taking-a-stock
- ## Other models → plus-cars, futures, molusce, crafty, clumondo ?


## landscape-metrics

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `landscape-level-and-diversity.md` | — | full | `landscape-metrics-in-python.md` | |
| `landscape-signatures.md` | — | full | `landscape-metrics-in-python.md` | |
| `large-maps.md` | — | full | `landscape-metrics-in-python.md` | |
| `moving-windows.md` | — | full | `landscape-metrics-in-python.md` | |
| `patch-and-class-metrics.md` | — | full | `landscape-metrics-in-python.md` | |
| `sampling-and-change.md` | `metrics.correlation`, `metrics.sample` | partial | `landscape-metrics-in-python.md` | |
| `what-are-landscape-metrics.md` | — | full | `landscape-metrics-in-python.md` | |

`landscape-metrics-in-python.md`:
- ## The three levels → what-are-landscape-metrics
- ## First metrics → patch-and-class-metrics
- ## Finding metrics → what-are-landscape-metrics ?
- ## Options → patch-and-class-metrics ?
- ## Maps of metrics → patch-and-class-metrics ?
- ## Sample plots → sampling-and-change
- ## Moving windows → moving-windows
- ## Zones → sampling-and-change ?
- ## Several maps and time series → sampling-and-change
- ## Spatial signatures → landscape-signatures
- ## Extra FRAGSTATS metrics → landscape-level-and-diversity ?
- ## Very large rasters and tables → large-maps
- ## Next steps → none (links)


## large-data

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `batch-processing.md` | — | full | `large-data-in-python.md` | |
| `larger-than-memory.md` | — | full | `large-data-in-python.md` | |

`large-data-in-python.md`:
- ## Collections → batch-processing ?
- ## Any tool on many files: batch → batch-processing
- ## MSPA of maps larger than memory → larger-than-memory
- ## Landscape metrics of files larger than memory → larger-than-memory
- ## Time stacks with dask → larger-than-memory ?
- ## Threads and memory → none (general)
- ## Next steps → none (links)


## neutral-landscapes

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `neutral-landscape-models.md` | — | full | `neutral-landscapes-in-python.md` | |
| `neutral-landscapes-in-a-study.md` | — | full | `neutral-landscapes-in-python.md` | |

`neutral-landscapes-in-python.md`:
- ## A gallery of models → neutral-landscape-models
- ## From continuous surfaces to habitat maps → neutral-landscape-models ?
- ## A null-model test → neutral-landscapes-in-a-study
- ## Reproducing NLMR → neutral-landscape-models ?
- ## Speed → none


## spatial-prioritization

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `evaluating-plans.md` | — | full | `conservation-plans-in-python.md`, `from-prioritizr-in-python.md`, `prioritization-in-python.md` | |
| `linear-programming.md` | — | full | `conservation-plans-in-python.md`, `from-prioritizr-in-python.md`, `prioritization-in-python.md` | |
| `marxan.md` | `prioritize.marxan`, `prioritize.proxirr` | partial | `conservation-plans-in-python.md`, `from-prioritizr-in-python.md`, `prioritization-in-python.md` | |
| `multi-action-planning.md` | — | full | `conservation-plans-in-python.md`, `from-prioritizr-in-python.md`, `prioritization-in-python.md` | |
| `spatial-design.md` | `prioritize.optimize` | partial | `conservation-plans-in-python.md`, `from-prioritizr-in-python.md`, `prioritization-in-python.md` | |
| `systematic-conservation-planning.md` | — | full | `conservation-plans-in-python.md`, `from-prioritizr-in-python.md`, `prioritization-in-python.md` | |
| `zonation.md` | — | full | `conservation-plans-in-python.md`, `from-prioritizr-in-python.md`, `prioritization-in-python.md` | |

`conservation-plans-in-python.md`:
- ## The data → none (shared data)
- ## What the reserves hold today → evaluating-plans ?
- ## A first plan → linear-programming ?
- ## A compact plan → spatial-design
- ## What each budget buys → linear-programming ?
- ## The places that matter most → marxan ?
- ## Actions against threats → multi-action-planning
- ## Zones of management → spatial-design ?
- ## Compared with a Zonation ranking → zonation ?
- ## Sharing the results → evaluating-plans ?
- ## Going further → none (links)

`from-prioritizr-in-python.md`:
- ## What changes, what stays → none (migration guide)
- ## A script, side by side → none
- ## prioritizr, function by function → none
- ## What to expect when you compare the results → none
- ## prioriactions → multi-action-planning ?
- ## See also → none (links)

`prioritization-in-python.md`:
- ## Features from a land-cover map → zonation ?
- ## A first ranking → zonation
- ## Complementarity, not hotspots → zonation ?
- ## Weights → zonation ?
- ## Costs and existing reserves → zonation ?
- ## Connectivity → zonation ?
- ## Zonation's files and projects → zonation ?
- ## Going further → none (links)


## species-distributions

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `evaluation-and-thresholds.md` | — | full | `species-distributions-in-python.md` | |
| `fitting-models.md` | — | full | `species-distributions-in-python.md` | |
| `ideas.md` | — | full | `species-distributions-in-python.md` | |
| `occurrence-data.md` | — | full | `species-distributions-in-python.md` | |
| `predictors-and-background.md` | — | full | `species-distributions-in-python.md` | |
| `prepare-in-one-node.md` | — | full | `species-distributions-in-python.md` | |
| `projection-and-extrapolation.md` | — | full | `species-distributions-in-python.md` | |

`species-distributions-in-python.md`:
- ## Cleaning occurrence records → occurrence-data
- ## The whole preparation in one call → prepare-in-one-node
- ## Step by step → predictors-and-background ?
- ## Spatial folds → evaluation-and-thresholds ?
- ## Fitting models → fitting-models
- ## Projecting the models → projection-and-extrapolation
- ## What the models learned → fitting-models ?
- ## From suitability to presence and absence → evaluation-and-thresholds
- ## A warmer climate → projection-and-extrapolation ?
- ## Maxent (MaxNet) → fitting-models
- ## Saving the results → none
- ## Going further → none (links)


## surface-metrics

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `geomorphons.md` | `raster.reproject` (downstream: `raster.calc`, `raster.terrain`, `surface.geomorphons`, `table.write`) | partial | `surface-texture-in-python.md` | |
| `hilltops.md` | — | full | `surface-texture-in-python.md` | |
| `surface-texture.md` | — | full | `surface-texture-in-python.md` | |
| `terrain.md` | `raster.reproject` (downstream: `raster.calc`, `raster.terrain`, `surface.curvature`, `surface.hli`, `surface.terrain`, `surface.tpi`, `surface.tri`, `surface.vrm`) | partial | `surface-texture-in-python.md` | |

`surface-texture-in-python.md`:
- ## A surface and its trend → surface-texture
- ## Whole-surface metrics → surface-texture
- ## The bearing area curve → surface-texture
- ## Texture images → surface-texture
- ## Spatial structure → surface-texture ?
- ## Terrain indices → terrain
- ## Landforms → geomorphons
- ## Hilltop protection areas → hilltops
- ## Validation → none (checks against references) ?


## terrain-hydrology

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `depressions-and-flow.md` | `raster.reproject` (downstream: `hydro.fill`, `hydro.flow_accumulation`, `hydro.flow_direction`, `raster.calc`, `raster.write`) | partial | `terrain-hydrology-in-python.md`, `watershed-model-in-python.md` | |
| `streams-watersheds-and-hand.md` | — | full | `terrain-hydrology-in-python.md`, `watershed-model-in-python.md` | |
| `swat-plus.md` | — | full | `terrain-hydrology-in-python.md`, `watershed-model-in-python.md` | |
| `swat-plus-from-rasters.md` | — | full | `terrain-hydrology-in-python.md`, `watershed-model-in-python.md` | |
| `wetness-and-connectivity.md` | `raster.reproject` (downstream: `hydro.connectivity_index`, `raster.calc`) | partial | `terrain-hydrology-in-python.md`, `watershed-model-in-python.md` | |

`terrain-hydrology-in-python.md`:
- ## A DEM and its depressions → depressions-and-flow
- ## Flow directions and accumulation → depressions-and-flow
- ## Streams → streams-watersheds-and-hand
- ## Height above the nearest drainage → streams-watersheds-and-hand
- ## Wetness → wetness-and-connectivity
- ## Validation → none ?
- ## Next steps → none (links)

`watershed-model-in-python.md`:
- ## The landscape → swat-plus-from-rasters ?
- ## Dividing the watershed → swat-plus-from-rasters ?
- ## Weather → swat-plus-from-rasters ?
- ## The project → swat-plus-from-rasters
- ## Simulating → swat-plus
- ## Streamflow → swat-plus
- ## Soil loss → swat-plus ?
- ## A land use scenario → swat-plus-from-rasters ?
- ## Projects from elsewhere → swat-plus ?


## uncertainty

| part | operators without counterpart | coverage | in-python page | done |
|---|---|---|---|---|
| `monte-carlo.md` | — | full | — | |

No `*-in-python.md` page in this category.


## Operators the library lacks

### `rng.r` (8 parts)

- Reason: R's generator (set.seed) is the library's private _core.RRandom: its functions take a seed instead
- Parts: `land-change/clue-s-ideas`, `land-change/clue-s-model`, `land-change/clue-s-rules`, `land-change/coupled-agents`, `land-change/dynamic-drivers`, `land-change/land-demand`, `land-change/ordered-allocation`, `land-change/regional-demand`

### `lulcc.history` (8 parts)

- Reason: the Python library keeps the history of the classes inside tf.simulate.run's simulation
- Parts: `land-change/clue-s-ideas`, `land-change/clue-s-model`, `land-change/clue-s-rules`, `land-change/coupled-agents`, `land-change/dynamic-drivers`, `land-change/land-demand`, `land-change/ordered-allocation`, `land-change/regional-demand`

### `raster.reproject` (4 parts)

- Reason: its extent: not in the script yet
- Parts: `surface-metrics/geomorphons`, `surface-metrics/terrain`, `terrain-hydrology/depressions-and-flow`, `terrain-hydrology/wetness-and-connectivity`

### `dinamica.step` (3 parts)

- Reason: the Python library allocates only within a whole model: tf.simulate.run(t0, t1, drivers, preset="dinamica", until=...) fits, validates and projects it
- Parts: `land-change/expander-and-patcher`, `land-change/gravity-potentials`, `land-change/roads-that-grow`

### `change.fad` (2 parts)

- Reason: its matrix, stats and report put together the tables of tf.fad_multiscale_change's scales (one per window) in Terraflux's layout: not in the script yet
- Parts: `change-detection/forest-gain-and-loss`, `fragmentation/change-and-reports`
- Only the script generator lacks it: the library has `tf.fad_multiscale_change`, which `fragmentation/change-and-reports` uses.

### `change.suggest` (2 parts)

- Reason: the Python library has no function that suggests the transitions to model
- Parts: `change-detection/transitions`, `land-change/what-land-change-models-are`

### `crafty.allocate` (2 parts)

- Reason: CRAFTY's competition between agent functional types is not in the Python library
- Parts: `land-change/coupled-agents`, `land-change/crafty`

### `plus.demand` (2 parts)

- Reason: the Python library makes CARS' demand inside tf.simulate.CARS (tf.simulate.presets.plus), not by a function of its own
- Parts: `land-change/land-demand`, `land-change/plus-cars`

### `data.download` (1 parts)

- Reason: its source esa_worldcover: not a function of the library
- Parts: `animal-movement/corridors-and-validation`

### `plugin.wasm` (1 parts)

- Reason: a WebAssembly plugin has no counterpart in Python
- Parts: `building-models/plugins`

### `distance.proximity` (1 parts)

- Reason: gdal_proximity's distances (two passes over the cells, an approximation of the Euclidean distance): the Python library has no function for it, and scipy's exact distance transform gives other numbers
- Parts: `distance-restoration/distance-to-habitat`

### `sdm.area_of_habitat` (1 parts)

- Reason: its input restore: not in the script yet
- Parts: `distance-restoration/original-vegetation-and-habitat`

### `raster.mosaic` (1 parts)

- Reason: gdalwarp's mosaic of the tiles (its CRS, resolution and resampling): the Python library has none (tf.landscape_mosaic is GuidosToolbox's Landscape Mosaic, another thing), and rasterio's merge does not resample as gdalwarp does
- Parts: `getting-started/one-grid`

### `raster.fill` (1 parts)

- Reason: gdal_fillnodata's search of the values and their weights are GDAL's: not in the library
- Parts: `getting-started/reclassify-calculate-save`

### `linkage.pathways` (1 parts)

- Reason: no function of the Python library known for it yet
- Parts: `habitat-networks/linkage-mapper`

### `linkage.pinchpoints` (1 parts)

- Reason: no function of the Python library known for it yet
- Parts: `habitat-networks/linkage-mapper`

### `linkage.barriers` (1 parts)

- Reason: no function of the Python library known for it yet
- Parts: `habitat-networks/linkage-mapper`

### `clumondo.ages` (1 parts)

- Reason: the Python library runs CLUMondo only within a whole model (tf.simulate.CLUMondo, tf.simulate.presets.clumondo), the ages of its patches inside
- Parts: `land-change/clumondo`

### `futures.simulation` (1 parts)

- Reason: the Python library runs FUTURES only as a whole model: tf.simulate.presets.futures(potential, demand, patch_sizes, ...)
- Parts: `land-change/futures`

### `molusce.total` (1 parts)

- Reason: the Python library gives the total weights only of a fitted tf.simulate.MolusceWoE, which has no way to take weights given
- Parts: `land-change/molusce`

### `molusce.step` (1 parts)

- Reason: the Python library runs MOLUSCE's simulator only within a whole model: tf.simulate.presets.molusce(t0, t1, drivers, iterations=...) fits and simulates it
- Parts: `land-change/molusce`

### `metrics.sample` (1 parts)

- Reason: its id_column with several sizes: not in the script yet
- Parts: `landscape-metrics/sampling-and-change`

### `metrics.correlation` (1 parts)

- Reason: the Python library's calculate_correlation takes the long table of the metrics (level, class, id, metric, value) and gives the lower triangle of the matrix as pairs; Terraflux's takes a wide table and gives the square matrix, the same R correlations in another shape: not in the script yet
- Parts: `landscape-metrics/sampling-and-change`

### `prioritize.marxan` (1 parts)

- Reason: from maps: not in the script yet (the library's marxan takes Marxan's tables)
- Parts: `spatial-prioritization/marxan`

### `prioritize.proxirr` (1 parts)

- Reason: from maps: not in the script yet (the library's eval_proxirr_marxan takes Marxan's tables)
- Parts: `spatial-prioritization/marxan`

### `prioritize.optimize` (1 parts)

- Reason: its input polygons: not in the script yet
- Parts: `spatial-prioritization/spatial-design`

