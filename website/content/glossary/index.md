---
title: Glossary
description: The words of GIS, computing and landscape ecology that the tutorials use, explained in plain language.
---

The tutorials define each term where it first matters. This page gathers them, so you can come back when a word
is unfamiliar. Terms are grouped by subject; use the search (++ctrl+k++) to jump to one.

## Maps and data

Raster
:   A map made of a grid of square **cells** (pixels), each holding one value: a land cover class, an elevation,
    a probability. Satellite images, land use maps and elevation models are rasters. Terraflux works mostly with
    rasters.

Cell, cell size, resolution
:   A cell is one square of a raster. Its **size** (or the raster's **resolution**) is the length of its side on
    the ground, for example 30 m. Smaller cells show more detail and make larger files. Areas computed by
    Terraflux come from the cell size: a 30 m cell covers 900 m², 0.09 ha.

Extent
:   The rectangle a map covers, given by its edges' coordinates (west, south, east, north).

Band, layer, stack
:   A raster file may hold several grids of the same size, called **bands**: the years of a land use series, the
    drivers of a model, the channels of an image. In Terraflux a set of maps on one grid is a **stack**, and each
    map in it a **layer**.

No data (nodata, missing)
:   A cell without a value: outside the study area, under a cloud, the sea. A raster marks it with a special
    value; Terraflux leaves those cells out of every count and calculation.

Categorical (classes) and continuous data
:   A **categorical** raster holds codes of classes (1 forest, 2 farming, 3 water): the numbers are labels, and
    averaging them makes no sense. A **continuous** raster holds measurements (elevation, temperature, a
    probability). The difference decides how a map may be resampled and which colours show it.

Data type
:   How a raster stores its numbers: whole numbers of 8, 16 or 32 bits (`uint8`, `int16`...) or decimal numbers
    (`float32`, `float64`). Classes fit in `uint8`; probabilities need floats.

GeoTIFF, COG
:   The usual raster file format (`.tif`), with its coordinates inside. A **Cloud Optimized GeoTIFF** (COG) is a
    GeoTIFF arranged in tiles with smaller copies (overviews), so that parts of it can be read quickly, even over
    the web.

Vector data
:   Maps made of **features** with coordinates: points (occurrence records, plots), lines (roads, rivers) and
    polygons (reserves, parcels, municipalities). Each feature carries **attributes** in a table. Files:
    GeoPackage (`.gpkg`), Shapefile (`.shp`), GeoJSON.

Rasterize
:   To burn vector features into a raster: each cell takes the value of the feature over it.

Zonal statistics
:   Statistics of a raster within each polygon (or zone): the mean elevation of each reserve, the forest area of
    each municipality.

GDAL
:   The open-source library that reads and writes almost every geographic file format. Terraflux brings its own
    copy, so you need not install it.

## Coordinates and grids

Coordinate reference system (CRS)
:   The rule that turns positions on the Earth into the numbers of a map. Each has a code, such as
    **EPSG:4326** (latitude and longitude, WGS 84) or **EPSG:31983** (UTM zone 23 S, SIRGAS 2000).

Geographic and projected coordinates
:   **Geographic** coordinates are angles (degrees of latitude and longitude); a degree is not a fixed distance
    on the ground. **Projected** coordinates are metres on a flat map, made by a projection. Areas, distances and
    most landscape metrics need projected coordinates, ideally an **equal-area** projection for areas.

Reprojection
:   Moving a map from one CRS to another. For a raster it means building a new grid and filling it by
    resampling.

Resampling
:   Filling the cells of a new grid from an old one. **Nearest neighbour** and **mode** (majority) keep the
    original values, so they suit classes; **bilinear** and **cubic** interpolation, and **mean**, suit
    continuous values; **sum** suits counts. Using interpolation on classes invents classes that do not exist.

Grid alignment, reference grid
:   Maps can be combined cell by cell only when they share the same CRS, cell size and cell edges. Terraflux's
    **reference grid** is the grid every map of a project is brought onto (`raster.match`).

## Landscape ecology

Landscape
:   An area that is spatially heterogeneous in at least one factor of interest, at the scale of the question.
    Its pattern is described by **composition** (what is there, and how much) and **configuration** (how it is
    arranged).

Patch
:   An area of one class, made of cells that touch each other. Whether cells touching only at a corner belong to
    the same patch depends on the **neighbourhood rule**: 4 neighbours (sides only) or 8 (sides and corners).

Grain and extent (scale)
:   The **grain** is the finest unit of observation (the cell size); the **extent** is the whole area studied.
    Most landscape measures change with both, so report them.

Edge, edge effect
:   The boundary between a habitat and its surroundings, where conditions differ from the interior (light, wind,
    predators). The **edge width** is how far into a patch that influence reaches.

Core area
:   The part of a habitat patch farther from its edge than the edge width.

Habitat loss and fragmentation
:   **Habitat loss** is the reduction of habitat amount; **fragmentation per se** is the breaking apart of the
    habitat that remains, for the same amount. The two are often confused, and their effects on species differ.

Connectivity
:   The degree to which a landscape lets organisms move between habitat patches. **Structural** connectivity
    depends on the layout alone; **functional** connectivity depends on how a given species moves.

Resistance (cost) surface
:   A raster saying how hard (costly, risky) it is for an organism to cross each cell. It is the input of
    least-cost and circuit analyses.

Corridor
:   A strip of habitat, or of easy terrain, linking two patches.

Neutral landscape model
:   A landscape drawn by chance with a known pattern, used as a null model: what would the pattern be if only
    chance shaped it?

## Conservation planning

Feature
:   Something a plan should conserve: a species, an ecosystem, a stock of carbon. Each is mapped as a raster of
    its amount or probability in each cell.

Planning unit
:   The unit a plan selects or ranks: a cell, a hexagon, a parcel, a watershed.

Target
:   How much of each feature a plan must hold (30 % of each species' range, 1,000 ha of each ecosystem).

Complementarity
:   Choosing places for what they add to the places already chosen, not for their own richness.

Irreplaceability
:   How necessary a planning unit is to meet the targets: an irreplaceable unit cannot be swapped for another.

## Modelling

Model, graph, node, operator
:   A Terraflux **model** is a **graph** of **nodes**. Each node runs one **operator** (read a map, reclassify it,
    run MSPA...) on the values that reach it through its **input ports** and gives values from its **output
    ports**. Connections carry values from an output to an input.

Parameter (setting)
:   A value that tunes how an operator works (an edge width, a window size), set in the node rather than
    connected.

Driver (explanatory variable)
:   In land change models, a map that helps explain where change happens: distance to roads, slope, soil.

Calibration and validation
:   **Calibration** fits a model's settings on observed data from a past period; **validation** compares what the
    model predicts with observations it did not see.

Stochastic model, seed, replicate
:   A **stochastic** model uses random numbers, so two runs differ. The **seed** fixes the random numbers: the
    same seed gives the same result. **Replicates** are runs with different seeds, to see the spread of
    outcomes.

Cellular automaton
:   A model where each cell changes state by rules that depend on its own state and on its neighbours',
    step after step.

Monte Carlo
:   Running a calculation many times with inputs drawn from their distributions, to see the distribution of
    the result: its uncertainty.

## Computing

Core, thread
:   A **core** is one processing unit of a computer's processor; a laptop has 4 to 16. A **thread** is a sequence
    of work a core runs. Work split into threads runs on several cores at once.

Process, machine, node
:   A **process** is a running program. A **machine** is one computer; in a cluster each machine is called a
    **node** (not to be confused with a model's nodes).

Coordinator and worker
:   In Terraflux's distributed mode, the **coordinator** hands out tasks (replicates, scenarios, tiles) and the
    **workers** run them, on the same machine or on many.

Cluster, HPC, Slurm
:   A **cluster** is a group of machines run as one, often a university's **high-performance computing** (HPC)
    facility. **Slurm** is the scheduler most of them use: you ask for nodes, cores and time, and your job runs
    when they are free.

Tile, halo
:   A part of a large raster processed on its own. A **halo** is the band of cells around a tile that it needs to
    read from its neighbours (for a window or a neighbourhood) without writing them.

Command line, terminal
:   A window where programs are run by typing commands, such as `terraflux run model.toml`. On Windows it is
    PowerShell or the Command Prompt; on macOS and Linux, the Terminal.

TOML
:   A plain-text format for settings, easy to read and write by hand. Terraflux saves models in it.
