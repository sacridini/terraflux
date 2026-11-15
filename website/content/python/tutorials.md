---
title: Tutorials
description: "Step-by-step guides for the Python library, one per topic: what the analysis means, a complete example on the maps that ship with Terraflux, how to read the results and the advanced options."
order: 50
---

Step-by-step guides, one per topic. Each starts with what the analysis means
and why you would use it, walks through a complete example on the maps that
ship with Terraflux, shows the results and how to read them, and ends with
the advanced options. Every code block runs as it is: copy it into a notebook
and follow along.

!!! tip "Before you start"
    [Install Terraflux](/python/getting-started/installation/) and read [core concepts](/python/getting-started/concepts/): it explains how
    habitat maps are coded, which all tutorials rely on.

!!! info "New to landscape ecology?"
    The [Learn](/learn/) course explains what each analysis
    measures, the theory behind it and when to use it. The tutorials focus on
    how to run the tools.

!!! app "In the application"
    Each page here has a counterpart in the application's [tutorials](/tutorials/), which builds the same analysis as
    a graph of operators; the library's pages sit at the end of their category there, named "... in Python", and the
    two on several computers and on a cluster are in [Parallel computing](/parallel/).

- **[Habitat patterns](/tutorials/habitat-patterns/patterns-in-python/)**: Cores, edges, bridges and islets; objects by size; parcels and the shape of habitat boundaries.
- **[Fragmentation](/tutorials/fragmentation/fragmentation-in-python/)**: Habitat density at one or many scales, fragmentation classes, landscape mosaics, contagion and entropy.
- **[Landscape metrics](/tutorials/landscape-metrics/landscape-metrics-in-python/)**: Patch, class and landscape metrics, sample plots, moving windows, time series and spatial signatures.
- **[Habitat networks](/tutorials/habitat-networks/habitat-networks-in-python/)**: Patches and distances as a graph: connectivity indices, the importance of every patch and link, protected-area connectivity.
- **[Circuit connectivity](/tutorials/circuit-connectivity/circuit-connectivity-in-python/)**: Current flow between habitat areas and omnidirectional connectivity maps, on rasters and on networks.
- **[Distance & restoration](/tutorials/distance-restoration/distance-restoration-in-python/)**: Distance and cost maps, least-cost paths and corridors, restoration status and travel-time maps.
- **[Change detection](/tutorials/change-detection/change-detection-in-python/)**: What changed between two dates: habitat gains and losses, structural change, shifts between fragmentation classes.
- **[Large data & batch](/tutorials/large-data/large-data-in-python/)**: Folders of rasters, maps larger than memory, time stacks with dask and very large tables.
- **[Several computers](/parallel/many-computers-in-python/)**: Share many independent tasks (tiles, replicates, calibration points) among the computers of a lab without a cluster: bring them into a group, lend them, send batch jobs to it and find out why a helper does not show up.
- **[On a cluster (HPC, Slurm)](/parallel/hpc-slurm-in-python/)**: Scripts in batch jobs with the processors of the job (less a quarter), folders of rasters over job arrays, time limits and nodes without internet access.
- **[Routes on grids](/tutorials/grid-distances/grid-routes-in-python/)**: Travel-time surfaces, least-cost routes, random walks and randomised shortest paths; how much routes from a common origin overlap.
- **[Neutral landscapes](/tutorials/neutral-landscapes/neutral-landscapes-in-python/)**: Simulated landscapes as null models: random, gradients, fractals, clusters and mosaics; habitat maps and null-model tests.
- **[Surface texture](/tutorials/surface-metrics/surface-texture-in-python/)**: Continuous rasters without classes: roughness, height distribution, texture images, spatial structure, terrain indices of DEMs and the hilltops protected by Brazil's Forest Code.
- **[Terrain hydrology](/tutorials/terrain-hydrology/terrain-hydrology-in-python/)**: Filling and breaching depressions, flow directions and accumulation, streams, height above the nearest drainage and wetness.
- **[Watershed simulation](/tutorials/terrain-hydrology/watershed-model-in-python/)**: The SWAT+ model from a DEM, land use and soils: daily water balance, streamflow, soil loss on the map and a land use scenario.
- **[Biodiversity analysis](/tutorials/biodiversity/biodiversity-in-python/)**: Species records in grid cells: richness, endemism, phylogenetic diversity and endemism, randomisations, CANAPE and regions.
- **[Conservation priorities](/tutorials/spatial-prioritization/prioritization-in-python/)**: Rank a landscape for many species with Zonation: performance curves, weights, costs, existing reserves and connectivity.
- **[Optimized conservation plans](/tutorials/spatial-prioritization/conservation-plans-in-python/)**: The cheapest plan that meets every target: reserves, compactness, budgets, importance, actions against threats, and a report.
- **[Restoration priorities](/tutorials/distance-restoration/restoration-plans-in-python/)**: How much of every converted cell to restore: original vegetation, habitat gains, extinction risk, carbon, intactness, budgets, regions and uncertainty.
- **[Species distributions](/tutorials/species-distributions/species-distributions-in-python/)**: Clean and thin occurrence records, prepare predictors and background, fit and cross-validate models on spatial folds, project and check extrapolation.
- **[Animal movement](/tutorials/animal-movement/animal-movement-in-python/)**: Read GPS tracks, measure their steps, resample them in bursts and draw the random steps and covariates of a step selection function.
- **[Land change simulation](/tutorials/land-change/land-change-simulation-in-python/)**: Project land use from two maps: demand, transition potential and four allocations, validation at several resolutions, calibration, scenarios with replicates, roads and stocks that change with the model.
