---
title: Algorithms
short: Overview
description: "The algorithms of the Python library as they are implemented, with their parameters, class codes, formulas and references: MSPA, fragmentation, landscape metrics, connectivity graphs, circuits, neutral landscapes and routes on grids."
order: 0
---

These pages describe **the algorithms as the library implements them**: what each tool computes, its parameters and
options, the class codes of its maps, the formulas, the conventions it follows to reproduce the original tool, and
the references. The [tutorials](/python/tutorials/) teach a method step by step on real maps, and the
[API reference](/python/api/) lists every function; these pages are what to read when you want to know exactly what
a tool does, for example to compare it with the program it reproduces or to cite it.

Every page starts from the tutorials of the same subject in the application and in Python, and links the API pages of
the functions it describes. How each tool is checked against the original, with the numbers, is in
[validation and benchmarks](/about/validation/).

## How the algorithms are implemented

- **Line-by-line parity.** The GuidosToolbox tools are reimplemented directly from its C, IDL and miallib sources
  (`fsp.c`, `spatcon.c`, `GraySpatCon.c`, `recode.c`, `guidostoolbox.pro`); the others follow the code of the R
  package, the Julia program or the executable they reproduce, down to the arithmetic where the result depends on it.
- **C++17 and OpenMP.** The heavy raster operations (sliding windows, geodesic reconstructions, topological graph
  searches, Euclidean distance transforms) run in C++17, multi-threaded with OpenMP, and release the Python GIL.
  Results never depend on the number of threads.
- **O(1) sliding windows.** FAD uses 2D integral images (summed-area tables), so each pixel costs the same from a
  7 x 7 to a 243 x 243 window; GraySpatCon uses 101-bin sliding histograms that update by columns. See
  [fragmentation and distance transforms](/python/algorithms/fragmentation-and-distance/).
- **Georeferencing preserved.** Every high-level function takes and returns `xarray.DataArray` objects, keeping the
  CRS, the affine geotransform, the coordinates and the NoData mask. Rasters are described in
  [core concepts](/python/getting-started/concepts/), the threads rule in
  [configuration](/python/getting-started/configuration/#threads).

## The pages

### Habitat patterns

| Page | What it covers | Reproduces |
|---|---|---|
| [MSPA and SPA](/python/algorithms/mspa-and-spa/) | Morphological Spatial Pattern Analysis, MSPA tiling for maps larger than memory, SPA2/3/5/6, the class codes and statistics | GuidosToolbox `mspa` |
| [Networks from MSPA](/python/algorithms/mspa-networks/) | Network components, ECA and DOC, dPC of nodes and links, Conefor files | GuidosToolbox GTN |
| [Restoration and morphological shape](/python/algorithms/restoration-and-shape/) | Restoration status, cost distance and corridors, accounting, isochrone maps, parcellation, contortion | GuidosToolbox Restoration Planner, Accounting |

### Fragmentation and change

| Page | What it covers | Reproduces |
|---|---|---|
| [Fragmentation and distance transforms](/python/algorithms/fragmentation-and-distance/) | FAD and multiscale FAD, FOS, Euclidean distance, influence zones, proximity, directional entropy | GuidosToolbox, `spatcon` |
| [Landscape mosaic, contagion and clustering](/python/algorithms/landscape-mosaic/) | Landscape mosaic of 19 and 103 classes, its heatmap, Pff and FAC | GuidosToolbox, `recode` |
| [Change detection and spatial convolution](/python/algorithms/change-and-convolution/) | Simple change, MCD, FOS and FAD-MS change, heatmap change, GraySpatCon | GuidosToolbox, `grayspatcon` |

### Landscape metrics

| Page | What it covers | Reproduces |
|---|---|---|
| [Landscape metrics](/python/algorithms/landscape-metrics/) | The patch, class and landscape metrics, the options, the extras, the exact arithmetic | landscapemetrics, FRAGSTATS |
| [Landscape metrics: plots, windows and signatures](/python/algorithms/landscape-metrics-extras/) | Sample plots, moving windows, time series, spatial signatures, plots, utilities | landscapemetrics, pylandstats |

### Connectivity and distances

| Page | What it covers | Reproduces |
|---|---|---|
| [Habitat connectivity graphs](/python/algorithms/connectivity-graphs/) | Nodes and distances, IIC and PC, importances, centrality, SAMC, ProtConn, ECA over time | Conefor Sensinode, Makurhini |
| [Circuit connectivity](/python/algorithms/circuit-connectivity/) | Advanced mode, Omniscape, conditional connectivity, INI files, the raster and network modes of Circuitscape | Circuitscape, Omniscape |
| [Distances and routes on grids](/python/algorithms/grid-distances/) | Transitions, cost surfaces, least-cost distances and paths, random walks, randomised shortest paths | gdistance |

### Neutral landscapes and many rasters

| Page | What it covers | Reproduces |
|---|---|---|
| [Neutral landscape models](/python/algorithms/neutral-landscapes/) | The models of NLMR and the landscapetools utilities | NLMR |
| [Batch processing and raster collections](/python/algorithms/batch-processing/) | Any tool over many rasters, bounded memory, resuming, several computers | GuidosToolbox Batch Process |

The other areas of the library (spatial prioritization, species distributions, land change simulation, surface
metrics, terrain and hydrology, biodiversity, animal movement and the rest) are documented by their
[API pages](/python/api/) and [tutorials](/python/tutorials/); their methods and checks are listed in
[validation and benchmarks](/about/validation/#results-by-tool).
