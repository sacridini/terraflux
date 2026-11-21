---
title: Terraflux for Python
short: Overview
description: "Fast, exact landscape analysis for Python: habitat patterns, fragmentation, landscape metrics, connectivity, prioritization, species distributions and land change, from a single raster to maps larger than memory."
order: 0
---

**Fast, exact landscape analysis for Python.** Map habitat structure, measure fragmentation, compute landscape
metrics and model ecological connectivity, from a single raster on a laptop to continental maps larger than
memory. Terraflux brings the established methods of landscape ecology together in one Python package, with a
C++17/OpenMP core behind a numpy / xarray / dask / rasterio API. It reproduces the reference tools of each method,
checked against their outputs in the test suite, and runs from a few to several hundred times faster.

Python 3.10+ · C++17 / OpenMP core · numpy · xarray · dask · GPL-3.0-or-later ·
[PyPI](https://pypi.org/project/terraflux-py/) · [GitHub](https://github.com/sacridini/terraflux-py)

```bash
pip install terraflux-py
```

!!! note "Called landschaft until 1.58"
    The library was published as `landschaft` up to version 1.58; `pip install landschaft` now installs
    `terraflux`, and `import landschaft` keeps working for a while, with a deprecation warning. New code should
    use `import terraflux as tf`.

!!! app "Application or library?"
    The application builds the same analyses as graphs of operators and runs them on one computer or a cluster; it
    runs the same kernels as this library, so both give the same results. The library is for people who prefer to
    write code or work in notebooks. A model fitted in one runs in the other: *File > Python script* in the
    application writes a model as a script of this library. The application's [quickstart](/quickstart/) is a
    different page from this section's [quickstart](/python/getting-started/quickstart/).

Where to go:

- [Get started](/python/getting-started/): installation, a first run, configuration, concepts, data and plotting.
- [Tutorials](/python/tutorials/): each topic from the basics to the advanced options, on real maps.
- The [API reference](/python/api/): every function and class, with its parameters and a runnable example.
- [Migrating from prioritizr and prioriactions](/tutorials/spatial-prioritization/from-prioritizr-in-python/), and the
  [changelog](/python/changelog/).

## What you can do

- **[Habitat patterns](/tutorials/habitat-patterns/)**: split habitat into cores, edges,
  bridges, loops and islets; classify objects by size; describe the shape and texture of every patch.
- **[Fragmentation](/tutorials/fragmentation/)**: habitat density at many observation
  scales, fragmentation classes, landscape mosaics, contagion and spatial entropy, as maps and statistics.
- **[Landscape metrics](/tutorials/landscape-metrics/)**: 130+ patch, class and
  landscape metrics, sample plots, moving windows, time series, spatial signatures and rasters larger than memory.
- **[Habitat networks](/tutorials/habitat-networks/)**: patches and distances as a
  graph: equivalent connected area, probability of connectivity, node and link importance, protected-area
  connectivity.
- **[Circuit connectivity](/tutorials/circuit-connectivity/)**: current flow between
  habitat areas and omnidirectional connectivity across whole landscapes, on rasters and on networks.
- **[Distance and restoration](/tutorials/distance-restoration/distance-restoration-in-python/)**: distance maps,
  cost surfaces, least-cost paths and corridors, restoration priorities and travel-time (isochrone) maps.
- **[Routes on grids](/tutorials/grid-distances/)**: travel-time surfaces, least-cost and
  random-walk distances, randomised shortest paths and the overlap of dispersal routes.
- **[Change detection](/tutorials/change-detection/)**: compare two dates: simple and
  morphological change, shifts between fragmentation classes and heat maps of landscape change.
- **[Surface metrics](/tutorials/surface-metrics/)**: roughness, height distribution and
  texture of continuous rasters (ISO 25178) such as NDVI or elevation, terrain indices of DEMs and protected
  hilltops.
- **[Neutral landscapes](/tutorials/neutral-landscapes/)**: simulated landscapes as
  null models: random maps, gradients, fractal surfaces, Gaussian fields, clusters and mosaics.
- **[Biodiversity analysis](/tutorials/biodiversity/)**: species records in grid cells:
  richness, endemism, phylogenetic diversity and endemism, randomisations and regions.
- **[Conservation priorities](/tutorials/spatial-prioritization/prioritization-in-python/)**: rank whole
  landscapes for many species: performance curves, costs, reserves, connectivity and impact assessment.
- **[Species distributions](/tutorials/species-distributions/species-distributions-in-python/)**: clean occurrence
  records, choose predictors and background points, fit and evaluate models with spatial folds, and project them
  to new climates.
- **[Land change simulation](/tutorials/land-change/land-change-simulation-in-python/)**: project land use from
  two maps: demand, transition potential and allocation, validation against the observed change, calibration,
  scenarios with replicates, and roads and stocks that change with the model.
- **[Watershed simulation](/tutorials/terrain-hydrology/watershed-model-in-python/)**: the SWAT+ model from a DEM,
  land use and soils: daily water balance, streamflow, soil loss on the map and land use scenarios.
- **[Large data and batch](/tutorials/large-data/)**: folders of rasters in parallel, tiles
  for maps larger than memory, streaming tables with millions of rows.

## What it covers, and what it reproduces

| Area | What it covers | Reproduces |
|---|---|---|
| **[Habitat patterns](/python/api/patterns/)** | MSPA, SPA, object accounting, contortion | GuidosToolbox |
| **[Fragmentation](/python/api/fragmentation/)** | density at many scales (FAD, FOS), landscape mosaics, contagion, entropy | GuidosToolbox |
| **[Landscape metrics](/python/api/landscape-metrics/)** | 133 patch, class and landscape metrics, sample plots, moving windows, time series | landscapemetrics, FRAGSTATS |
| **[Habitat networks](/python/api/networks/)** | PC, IIC, ECA, node and link importance, ProtConn | Conefor, Makurhini |
| **[Circuit connectivity](/python/api/circuits/)** | omnidirectional and pairwise current flow, rasters and networks | Circuitscape, Omniscape |
| **[Distance & restoration](/python/api/distance/)** | cost surfaces, least-cost paths and corridors, isochrone maps | GuidosToolbox |
| **[Change detection](/python/api/change/)** | simple and morphological change, fragmentation transitions | GuidosToolbox |
| **[Routes on grids](/python/api/cost/)** | transitions from any rule, travel-time surfaces, least-cost distances and paths, random walks, randomised shortest paths, route overlap | gdistance |
| **[Surface metrics](/python/api/surface/)** | texture of continuous rasters (ISO 25178): roughness, height distribution, bearing area curve, slopes, peaks and valleys, autocorrelation, orientation, fractal dimension, trend surfaces, texture images; terrain indices (TPI, TRI, VRM, curvature, HLI...) | geodiv, spatialEco |
| **[Terrain hydrology](/python/api/hydro/)** | depression filling, D8, D-infinity and multiple flow directions, flow accumulation, streams, height above the nearest drainage (HAND), wetness, stream power and sediment transport indices | WhiteboxTools |
| **[Watershed model](/python/api/swat/)** | daily water, sediment and nutrient balances of a watershed, results on maps | SWAT+ |
| **[Neutral landscape models](/python/api/nlm/)** | simulated landscapes as null models: random, percolation, gradients, fractal surfaces, Gaussian fields, mosaics, clusters, classification | NLMR |
| **[Biodiversity analysis](/python/api/biodiversity/)** | species records in grid cells: richness, endemism, phylogenetic diversity and endemism, randomisations, CANAPE, clustering | Biodiverse |
| **[Spatial prioritization](/python/api/prioritization/)** | ranking of whole landscapes for many features (CAZ and ABF rules), weights, costs, hierarchic masks, connectivity transforms, performance curves, post-processing, impact assessment; maps larger than memory; and optimization of the planning units | Zonation 5, prioritizr, prioriactions |
| **[Species distributions](/python/api/sdm/)** | cleaning and thinning occurrence records, accessible area, collinearity filters, background points, spatial cross-validation folds, evaluation (AUC, TSS, Boyce index), Maxent (MaxNet), ensembles of scikit-learn models, projection and extrapolation (MESS, MOP, ExDet) | CoordinateCleaner, spThin, usdm, dismo, blockCV, ENMeval, ecospat, predicts, maxnet, mop |
| **[Land change simulation](/python/api/simulate/)** | change analysis (intensity, components of difference, Markov chains), transition potential (weights of evidence, logistic regression, any scikit-learn model), allocation (CLUE-S, ordered, land systems, Expander/Patcher, random patch seeds, patch growing), dynamic variables, regional demands, validation at several resolutions, fuzzy similarity, calibration, scenarios with replicates | lulcc, OpenLand, diffeR, CLUMondo, FUTURES, MOLUSCE, Dinamica EGO, PLUS |
| **[Ecosystem services](/python/api/services/)** | carbon storage and sequestration with valuation, habitat degradation, quality and rarity, crop pollination by wild bees, water yield, sediment delivery | InVEST |
| **[Animal movement](/python/api/move/)** | Movebank tracks, steps, speeds and turns, filters, track lines; random steps for step selection | move2, amt |
| **[Map accuracy and area](/python/api/accuracy/)** | stratified samples of a map, its accuracy and the area of its classes | mapaccuracy (Olofsson et al. 2014) |
| **[Deforestation allocation](/python/api/redd/)** | Verra's VT0007 as UDef-ARP | UDef-ARP |
| **[Brazil's Forest Code](/python/api/forest-code/)** | APPs of rivers, lakes, springs, slopes and hilltops; the Reserva Legal's balance per property | |
| **[Uncertainty](/python/api/uncertainty/)**, **[Sensitivity](/python/api/sensitivity/)**, **[Statistics of maps](/python/api/stats/)** | error propagation, Morris and Sobol indices, trends, Moran's I, LISA, Gi*, kriging, IDW | SALib, pymannkendall, PySAL, gstat |
| **[Data and configuration](/python/api/data/)**, **[Data on demand](/python/api/download/)** | rasters, vectors and tables in and out, colour tables, threads; WorldClim, CHELSA, Copernicus DEM, SoilGrids, MapBiomas, STAC | |
| **[Large data](/python/api/batch/)** | out-of-core tiles and row blocks, parallel batches of rasters, tasks shared with the other computers of a lab (`terraflux-py worker`) | |

## A taste

=== "Habitat patterns"

    ```python
    import terraflux as tf

    forest = tf.load_raster(tf.example_data("input"))  # 0 missing, 1 background, 2 forest
    morph = tf.mspa(forest, edge_width=2)              # cores, edges, bridges, loops...
    tf.mspa_stats(morph)                                # area of every class
    ```

=== "Landscape metrics"

    ```python
    import terraflux as tf

    landcover = tf.load_raster(tf.example_data("clc"))
    tf.landscape_metrics(landcover, level="landscape", metric=["shdi", "contag", "ed"])
    ```

=== "Habitat networks"

    ```python
    import terraflux as tf

    landcover = tf.load_raster(tf.example_data("clc"))
    graph = tf.HabitatGraph.from_raster(
        landcover, habitat=[23, 24, 25],          # forest classes
        distance="edge", max_distance=2000,       # edge-to-edge distances, metres
        landscape_area=landcover.size * 25 * 25,  # 25 m cells
    )
    graph.overall(["PC", "IIC"], distance=1000, probability=0.5)
    ```

=== "Circuit connectivity"

    ```python
    import terraflux as tf

    landcover = tf.load_raster(tf.example_data("clc"))[:600, :600]
    resistance = tf.reclassify(landcover, {23: 1.0, 24: 1.0, 25: 1.0},   # forest is easy to cross
                               others=25.0, dtype="float64")
    flow = tf.omniscape(resistance, radius=30, block_size=5)
    tf.plot(flow)                     # on an interactive map over satellite imagery
    ```

Every function takes numpy arrays, georeferenced `xarray.DataArray`s or dask arrays, and returns the same type;
parallel functions use most of the cores unless given `n_threads` (never all of them: a quarter stays free, see
[configuration](/python/getting-started/configuration/#threads)).

## Why Terraflux

- **Fast.** A C++17 core with OpenMP, cache-friendly algorithms and exact sparse solvers. Typical runs take
  seconds where the original tools take minutes or hours. [See the benchmarks](/benchmarks/speed/).
- **Exact.** The tools are checked against the reference implementations of their methods, to the last bit where
  the method allows it and to the solver's precision where it does not.
  [How each tool is checked](/benchmarks/accuracy/).
- **Pythonic.** numpy arrays, georeferenced `xarray.DataArray`s, dask, GeoTIFFs, GeoDataFrames and pandas tables
  in and out. No GUI, no temporary files, no other language to install.
- **Scales.** Rasters larger than memory by tiles or row blocks, collections of maps in parallel within a memory
  budget, and most of the cores by default.

The library follows [Semantic Versioning](https://semver.org/): from 1.0 on, the public API changes
incompatibly only in a new major version. The version these pages describe is {{pyversion}}.

## Validated against established tools

Terraflux reproduces the methods of GuidosToolbox (MSPA, fragmentation, accounting, restoration planning), the R
package landscapemetrics and FRAGSTATS (landscape metrics), Conefor Sensinode and Makurhini (habitat networks),
Circuitscape and Omniscape (circuit connectivity), gdistance (routes on grids), geodiv and spatialEco (surface
metrics), NLMR (neutral landscape models), Zonation 5 (spatial prioritization), the R packages for species
distributions (CoordinateCleaner, usdm, dismo, blockCV, ENMeval, predicts, ecospat, maxnet) and the land change
models (lulcc, CLUMondo, FUTURES, MOLUSCE, Dinamica EGO) and SWAT+ (watershed simulation), and is
[tested against their outputs](/benchmarks/accuracy/). You do not need to know any of them to use it: the
[tutorials](/python/tutorials/) start from scratch, and the [application's tutorials](/tutorials/) explain the landscape
ecology behind every tool.

| Tools | Compared with | Agreement | Speed-up |
|---|---|---|---|
| MSPA | GuidosToolbox `mspa` 2.3 | identical pixels | 10-17x |
| Landscape metrics | landscapemetrics 2.2.2 | identical bits | 35-500x |
| Habitat networks | Makurhini, Conefor | to 1e-12 | 13-620x |
| Omniscape, Circuitscape | Omniscape 0.7, Circuitscape 6.1 | to 2e-11 | 3-45x |
| Grid distances | gdistance 1.6 | identical bits (least cost); 1e-14 (random walks) | 3-400x |
| Neutral landscape models | NLMR 1.4 (same seed) | identical bits | 2-6000x |
| Spatial prioritization | Zonation 5 v2.4 | identical rank maps and output files | 4-11x |
| Species distributions | CoordinateCleaner, spThin, usdm, dismo, blockCV, ENMeval, predicts, maxnet | identical bits, random steps included (same seed) | maxnet 14x, kmeans 8x, weighted sampling 70x |
| Land change simulation | lulcc, OpenLand, diffeR, CLUMondo, r.futures, MOLUSCE; Dinamica EGO (measured) | identical bits (R's and NumPy's random numbers included); Dinamica to its 15 digits | 7-43x |

Methods, machines, full tables and the known differences are on the [benchmarks and accuracy](/benchmarks/)
page.

## License

GPL-3.0-or-later, as the application; see [About](/about/#license) for the third-party notes.
