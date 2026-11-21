---
title: Tutorials
description: "The tutorials teach the application and the Python library together: every part that the library can repeat has an In Python section, with the same data and the same numbers as the graph."
order: 50
---

The [tutorials](/tutorials/) teach the application and the library together. Each part explains an analysis, builds
it as a graph in the application and then, in its **In Python** section, does the same with the library: the same
data, the same settings and the same numbers, so you can learn either way, or both, and check one against the other.
Every code block runs as it is: copy it into a notebook and follow along.

!!! tip "Before you start"
    [Install Terraflux](/python/getting-started/installation/), run the [quickstart](/python/getting-started/quickstart/)
    and read [core concepts](/python/getting-started/concepts/): it explains how habitat maps are coded, which all
    tutorials rely on. The application's [first steps](/tutorials/getting-started/) repeat in Python what they
    analyse: reading a raster, reclassifying it, MSPA, reprojecting, vectors.

!!! info "Where to find the Python"
    In a short part, the section `In Python` comes at the end. In a long one, a section follows each step it repeats,
    named after the step (`Streams in Python`, `Tuning in Python`...). A few parts are all Python, for what only the
    library does (collections of files, dask, a script from prioritizr...). The parts the library cannot repeat yet
    (object-based image analysis, Linkage Mapper) have no section.

The categories, and what their Python covers:

- **[Habitat patterns](/tutorials/habitat-patterns/)**: Cores, edges, bridges and islets; objects by size; parcels and the shape of habitat boundaries; the network of cores and bridges and change between dates.
- **[Fragmentation](/tutorials/fragmentation/)**: Habitat density at one or many scales, fragmentation classes, landscape mosaics, contagion and entropy, and their change between dates.
- **[Landscape metrics](/tutorials/landscape-metrics/)**: Patch, class and landscape metrics, sample plots, moving windows, time series, spatial signatures and rasters larger than memory.
- **[Habitat networks](/tutorials/habitat-networks/)**: Patches and distances as a graph: connectivity indices, the importance of every patch and link, least-cost links, the network of a forest's structure, protected-area connectivity.
- **[Circuit connectivity](/tutorials/circuit-connectivity/)**: Current flow between habitat areas and omnidirectional connectivity maps, on rasters and on networks, from the resistance surface to Circuitscape and Omniscape.
- **[Distance & restoration](/tutorials/distance-restoration/)**: Distance and cost maps, least-cost paths and corridors, restoration status, and restoration plans with extinction risk, carbon, intactness, budgets, regions, conservation and conversion.
- **[Change detection](/tutorials/change-detection/)**: What changed between two dates: transition matrices, gains and losses, intensity analysis, comparison of maps and habitat change.
- **[Large data & batch](/tutorials/large-data/)**: Folders of rasters run in parallel, maps larger than memory, MSPA by tiles, and time stacks with dask.
- **[Several computers](/parallel/many-computers-in-python/)**: Share many independent tasks (tiles, replicates, calibration points) among the computers of a lab without a cluster: bring them into a group, lend them, send batch jobs to it and find out why a helper does not show up.
- **[On a cluster (HPC, Slurm)](/parallel/hpc-slurm-in-python/)**: Scripts in batch jobs with the processors of the job (less a quarter), folders of rasters over job arrays, time limits and nodes without internet access.
- **[Routes on grids](/tutorials/grid-distances/)**: Travel-time surfaces, least-cost routes, random walks and randomised shortest paths; how much routes from a common origin overlap.
- **[Neutral landscapes](/tutorials/neutral-landscapes/)**: Simulated landscapes as null models: the fifteen generators and their NLMR names, percolation, habitat maps, reproducing NLMR, an experiment of amount by fragmentation and the null-model test.
- **[Surface metrics](/tutorials/surface-metrics/)**: Continuous rasters without classes: roughness, height distribution, texture images, spatial structure, terrain indices of DEMs, landforms (geomorphons) and the hilltops protected by Brazil's Forest Code.
- **[Terrain hydrology](/tutorials/terrain-hydrology/)**: Depressions, flow, streams, watersheds, HAND, wetness and connectivity, and the SWAT+ watershed model from a DEM or a project.
- **[Biodiversity analysis](/tutorials/biodiversity/)**: Species records in grid cells: richness, endemism, phylogenetic diversity and endemism, randomisations, CANAPE and regions.
- **[Ecosystem services](/tutorials/ecosystem-services/)**: InVEST's models with `tf.services`: carbon, habitat quality, water yield, sediment, nutrients, pollination and seasonal water yield.
- **[Uncertainty](/tutorials/uncertainty/)**: The uncertainty of a model's parameters carried to its maps: Monte Carlo, the delta method, intervals and the uncertainty of a total.
- **[Conservation priorities](/tutorials/spatial-prioritization/prioritization-in-python/)**: Rank a landscape for many species with Zonation: performance curves, weights, costs, existing reserves and connectivity.
- **[From prioritizr and prioriactions](/tutorials/spatial-prioritization/from-prioritizr-in-python/)**: The equivalent of every function of the R packages, a script side by side, and what to expect when you compare the results.
- **[Optimized conservation plans](/tutorials/spatial-prioritization/conservation-plans-in-python/)**: The cheapest plan that meets every target: reserves, compactness, budgets, importance, actions against threats, and a report.
- **[Species distributions](/tutorials/species-distributions/)**: Clean and thin occurrence records, prepare predictors and background, fit and cross-validate models on spatial folds, evaluate, project and map extrapolation.
- **[Animal movement](/tutorials/animal-movement/)**: Read GPS tracks, clean and thin them, measure their steps, resample them in bursts, estimate home ranges, validate corridors and draw the random steps and covariates of a step selection function.
- **[Land change simulation](/tutorials/land-change/land-change-simulation-in-python/)**: Project land use from two maps: demand, transition potential and four allocations, validation at several resolutions, calibration, scenarios with replicates, roads and stocks that change with the model.
