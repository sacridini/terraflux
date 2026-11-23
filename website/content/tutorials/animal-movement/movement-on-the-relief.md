---
title: Movement on the relief
short: Movement on the relief
description: Ask whether a mountain animal follows the contour lines. Take the energy of walking over a DEM, the slope and ruggedness under each step and the height it gains along it, fit a step selection function with them for four Alpine ibex, read it as a relative selection strength, simulate paths from it and turn it into a resistance and a current.
order: 90
level: advanced
time: 90 min
ops: [cost.energy_landscape, surface.terrain, surface.tri, move.surface_distance, move.extract_covariates, move.fit_issf, move.log_rss, move.simulate_path, move.path_density, move.resistance, circuit.omniscape, move.validate, data.download]
series: Animal movement
part: 9
template: Movement on the relief
---

Does an animal of the mountains walk along the hillsides instead of up and down them? A step over a steep slope costs more than a step on flat ground, and a step that climbs 50 m costs more than one that stays at the same height, so an animal that saves energy should choose the second. The utilisation distributions of [part 6](../home-ranges/) cannot say: they are Brownian in the plane and know nothing of the ground. [Part 7](../corridors-and-validation/) took a first look from the values along the steps of the fishers, which did not follow the contour lines. Here you will ask the same question of four Alpine ibex in the Bargy massif, in the French Alps, with the tool made for it, the step selection function of [part 5](../step-selection/), and with terms that describe the relief. At the end you will have the energy of walking over the massif as a map, the slope, the ruggedness and the climb under every step, the coefficients of a step selection function with a confidence interval each, what they mean in relative selection strength, paths simulated from the model next to the real positions, and a resistance surface with the current of Omniscape that comes from it.

The answer is yes for one thing and no for the others, and the difference between them is the point of the page.

## The idea

### What a step costs on a slope

Walking uphill costs energy, and walking steeply downhill costs it too, as every hiker's knees know. Pontzer (2016) put the cost of legged locomotion of animals of any size into one allometric law. Berti et al. (2022) made it a landscape in their R package enerscape: the energy of walking onto each cell of a DEM, from the neighbours around it, over the distance in three dimensions and the slope between them.

!!! science "Key idea: the energy of a step"
    For an animal of body mass \( m \) (kg) walking on a slope of \( s \) degrees, the cost of one metre in joules is

    \[ c(s) = \left( 8\, m^{-0.34} + 50 \left( 1 + \sin\frac{(2 s - 74)\,\pi}{180} \right) m^{-0.12} \right) m, \]

    which is lowest on a gentle descent (a slope of −8 degrees) and grows quickly on a climb and on a steep descent. [[cost.energy_landscape]] gives, for each cell, the mean over its 8 neighbours of the cost of walking from the neighbour onto it, over their distance in three dimensions. It is a resistance surface in kilojoules or kilocalories, for animals (Pontzer) or for people (Looney et al., 2019, `model = "human"`). The border cells have no value.

    The cost is not a measurement of the animal. It says what a step costs a typical animal of that mass, and the step selection function says whether this animal acted as if it mattered.

### What a step crosses

The covariates of a step selection function are usually taken at the *end* of the step ([part 5](../step-selection/)). On a relief that says where the animal arrived, not what it did to get there: two steps to the same cell, one over a ridge, are not the same step. Four kinds of values describe the relief:

- **at the end of the step**: the *slope* in degrees (Horn's neighbours, as terra's `terrain`), the *terrain ruggedness index* (TRI: how much a cell's height differs from its neighbours', in metres) and the *energy*;
- **along the step**: with [[move.extract_covariates]] and `where = "along"`, the cells the step crosses in order give the **ascent** (the sum of the rises between successive cells), the **descent**, and the **delta**, the net height change \( |h_\text{last} - h_\text{first}| \). A step that follows a contour line has a small delta whatever its ascent; one that goes straight up a slope has an ascent equal to its delta;
- **the length on the relief**: [[move.surface_distance]] follows the step over the DEM and adds the three-dimensional chords. It is the distance walked, never shorter than the length on the plane;
- **the heights at both ends** (`where = "both"`), which will turn out to be what a simulation can use.

### The model

The integrated step selection function of Avgar et al. (2016), as in part 5, compares each observed step with random steps from the same start, by a conditional logistic regression (Fortin et al., 2005), with the step's length and turn in the model. Here the covariates are those of the relief. [[move.fit_issf]] fits it as R's `survival::clogit` does and gives its coefficients, their standard errors and intervals, and the log-likelihood.

!!! science "Key idea: the log relative selection strength"
    A coefficient says how much the log of the selection changes with one unit of a covariate, and with several covariates that move together (the slope, the TRI and the energy of a cell rise and fall as one) it is hard to read alone. The **log relative selection strength** (log-RSS; Avgar et al., 2017) puts them together: for two steps \( x_1 \) and \( x_2 \),

    \[ \log \text{RSS}(x_1, x_2) = \log \frac{w(x_1)}{w(x_2)} = \beta \cdot (x_1 - x_2), \]

    how much more (or less) an animal chooses a step with the covariates \( x_1 \) than one with \( x_2 \), both equally available. Zero is no preference, +0.69 is twice as much, −0.69 half. [[move.log_rss]] gives it with a Wald interval.

### From the model to a use of the landscape and a resistance

A step selection function is a model of movement, so it can be run: from a start, draw many candidate steps from the distributions of lengths and turns, weight each by \( \exp(\beta \cdot x) \) of the cells it ends in, take one, and repeat. Signer et al. (2024) showed that long paths from a fitted model give its steady-state utilisation distribution. [[move.simulate_path]] is amt's `simulate_path`; [[move.path_density]] counts the positions of the paths in the cells.

Two consequences of how it runs matter here. The simulation reads the covariates at the *end* of each candidate step (and at its start): terms *along* the step, such as the ascent, cannot be simulated. And it has no memory of where it has been: the animal has no home range, only the selection of the cells.

The habitat terms of the same model, without the terms of movement, are the *relative selection* of each cell, and a selection can be turned into a resistance (Zeller et al., 2012, review the ways to do it): [[move.resistance]] makes it a negative exponential of the scaled selection, the curve that Keeley et al. (2016) recommend, 1 for the cell most selected and 100 for the least, ready for a least-cost path or for circuit theory ([[circuit.omniscape]]).

## When to use it

- **To ask what an animal does on the relief**: whether it avoids steep ground or high-energy cells, whether it follows contours, whether it moves differently on a slope (the interaction of the step's length with the slope).
- **To explain a use that the DEM seems to explain**: the heights and slopes of a home range are covariates like any other, in a model that also says what the animal could have chosen.
- **To make a resistance surface from the animal's own movement**, as an alternative to a resistance from an expert or from a habitat model, and to check it against the animals ([part 7](../corridors-and-validation/)).

What it is not: the energy of walking is a cost *for a typical animal*, and a coefficient on it is a preference of this animal over this relief at this scale (a step of an hour, a DEM of 30 m). It is not a measure of the energy the animal spends.

## What you need

- **The tracks**, one location about every hour, projected in metres, and **a DEM** in metres, in the same projected system and with square cells. [[move.random_steps]] draws steps on a plane, and the energy and the length on the relief need heights and distances in the same unit. Here the tracks are those of four Alpine ibex (*Capra ibex*) of the Bargy massif, Haute-Savoie, from 31 May to 28 August 2013: 2,643 locations of about one an hour, the outside of the periods of massive sporting events (the column `MSE_or_reference` of the source is `without_MSE`). The data are those of Marchand et al. (2025), published on Zenodo under CC-BY 4.0; the file [`bargy_ibex_2013.csv`](/data/tutorials/animal-movement/movement-on-the-relief/bargy_ibex_2013.csv) has the four animals in the columns of a Movebank export (`individual-local-identifier`, `timestamp`, `location-long`, `location-lat`), and a [README](/data/tutorials/animal-movement/movement-on-the-relief/README.md) says where it comes from and what was changed. [[move.read_tracks]] reads it as it is.
- **The DEM**: the Copernicus GLO-30 elevation model, downloaded by [[data.download]] (`source = "copernicus_dem"`) for the area `[6.38, 45.94, 6.52, 46.08]` (west, south, east, north), 439 to 2,406 m, and put on UTM zone 32 North (EPSG:32632) with cells of 30 m: 531 by 378 cells, 15.9 by 11.3 km. *Produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved* (European Space Agency & Airbus, 2022).
- **The body mass** of the animal. The file does not give the sex or the mass of the four ibex. Adult Alpine ibex are heavy animals, in the order of 60 to 100 kg depending on sex (an approximate range, not a measurement of these animals): the page uses 80 kg, the middle, and shows below what another mass does to the answer (nothing, except the scale of the energy's coefficient). **Say the mass you used.**
- To start from a model, the template **Movement on the relief** (library subject **Animal movement**, group *Movement*). With **Use my data**, replace the DEM and the track with yours.

## Build the model

### The relief

```tfgraph
#! caption: The Copernicus DEM downloaded (node 1) and put on UTM 32N with cells of 30 m (node 2), its slope (node 3) and ruggedness (node 4), and the energy of walking onto each cell for an animal of 80 kg (node 5).
#! numbered: true
#! highlight: slope, tri, energy
format = "terraflux/1"

[[nodes]]
id = "dem_ll"
op = "data.download"
params = { source = "copernicus_dem", dem_resolution = 30, extent = [6.38, 45.94, 6.52, 46.08], cellsize = 0.0003 }

[[nodes]]
id = "dem"
op = "raster.reproject"
inputs = { raster = "dem_ll.raster" }
params = { crs = "EPSG:32632", resolution = 30, resampling = "bilinear", options = "-dstnodata nan" }

[[nodes]]
id = "slope"
op = "surface.terrain"
inputs = { map = "dem.raster" }
params = { v = "slope", unit = "degrees" }

[[nodes]]
id = "tri"
op = "surface.tri"
inputs = { map = "dem.raster" }
params = { s = 3 }

[[nodes]]
id = "energy"
op = "cost.energy_landscape"
inputs = { dem = "dem.raster" }
params = { mass = 80, unit = "kcal", neigh = 8, model = "animal" }
```

<div class="steps" markdown>

1. **Download the DEM** with [[data.download]] (node 1): `source = "copernicus_dem"`, `dem_resolution = 30` and the `extent` in degrees. The grid of the download is in longitude and latitude (`cellsize` is in degrees).

2. **Project it** with [[raster.reproject]] (node 2) to UTM 32N, `resolution = 30` and bilinear resampling, because heights are numbers and not classes. The `options` setting passes `-dstnodata nan` to gdalwarp: the corners of the rectangle in degrees, which fall outside the square of the projected raster, would otherwise be filled with 0 m, and the cells beside them would have cliffs of hundreds of metres in 30 m.

3. **The slope and the ruggedness** with [[surface.terrain]] (node 3, `v = "slope"`, in degrees) and [[surface.tri]] (node 4, a window of 3 cells). The edge cells have no value.

4. **The energy** with [[cost.energy_landscape]] (node 5): `mass = 80`, `unit = "kcal"` (1 kcal is 4,184 J), `neigh = 8` and `model = "animal"`. The DEM must be in metres with square cells; the node says so when it is not.

</div>

### The steps and their random steps

```tfgraph
#! caption: The tracks read (node 1) and projected to the DEM's UTM zone (node 2), one location about every hour in bursts of 3 or more (nodes 3 and 4), their steps (node 5), the gamma and von Mises distributions fitted to all of them (nodes 6 and 7) and 10 random steps for each observed step (node 8).
#! numbered: true
#! highlight: utm, random
format = "terraflux/1"

[[nodes]]
id = "read"
op = "move.read_tracks"
params = { path = "bargy_ibex_2013.csv" }

[[nodes]]
id = "utm"
op = "vector.reproject"
inputs = { vector = "read.tracks" }
params = { crs = "EPSG:32632" }

[[nodes]]
id = "resample"
op = "move.resample"
inputs = { tracks = "utm.vector" }
params = { rate = "1h", tolerance = "5min" }

[[nodes]]
id = "bursts"
op = "move.filter_min_n_burst"
inputs = { tracks = "resample.tracks" }
params = { min_n = 3 }

[[nodes]]
id = "steps"
op = "move.steps"
inputs = { tracks = "bursts.tracks" }

[[nodes]]
id = "sl_fit"
op = "move.fit_distribution"
inputs = { values = "steps.steps" }
params = { name = "gamma" }

[[nodes]]
id = "ta_fit"
op = "move.fit_distribution"
inputs = { values = "steps.steps" }
params = { name = "vonmises" }

[[nodes]]
id = "random"
op = "move.random_steps"
inputs = { steps = "steps.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { n_control = 10, seed = 1 }
```

[[move.read_tracks]] (node 1) reads the file and gives points in longitude and latitude; [[vector.reproject]] (node 2) takes them to the DEM's UTM zone. The steps are those of [part 4](../steps-and-summaries/): [[move.resample]] (node 3) keeps one location about every hour, within 5 minutes, in bursts; [[move.filter_min_n_burst]] (node 4) keeps the bursts of 3 locations or more; [[move.steps]] (node 5) makes the steps, 2,578 of them. The tentative distributions are fitted to all the steps at once, a gamma distribution for the lengths and a von Mises for the turns ([[move.fit_distribution]], nodes 6 and 7): a simulation needs one pair of distributions for the animal, and these ones are connected to the random steps and, later, to the fit. [[move.random_steps]] (node 8) draws 10 random steps for each observed step with a turn angle: 2,517 strata of 11 steps, 27,687 rows.

### The relief under the steps

```tfgraph
#! caption: The energy, the slope and the TRI at the end of every step (ends), the ascent and the net height change along it (climb), the heights at its start and end (both) and its length on the relief (distance).
#! highlight: ends, climb, both, distance
#! fragment: true
[[nodes]]
id = "ends"
op = "move.extract_covariates"
inputs = { tracks = "random.steps", energy = "energy.energy", slope = "slope.map", tri = "tri.map" }
params = { where = "end" }

[[nodes]]
id = "climb"
op = "move.extract_covariates"
inputs = { tracks = "ends.tracks", dem = "dem.raster" }
params = { where = "along", stats = ["ascent", "delta"] }

[[nodes]]
id = "both"
op = "move.extract_covariates"
inputs = { tracks = "climb.tracks", height = "dem.raster" }
params = { where = "both" }

[[nodes]]
id = "distance"
op = "move.surface_distance"
inputs = { steps = "both.tracks", dem = "dem.raster" }
```

[[move.extract_covariates]] is used three times. `ends` takes the energy, the slope and the TRI at the end of every step, in columns named after the inputs. `climb` takes the cells along the step (`where = "along"`) of the DEM and gives the ascent and the delta, in the columns `dem_along_ascent` and `dem_along_delta`. `both` takes the DEM's height at the start and at the end (`height_start` and `height_end`). Then [[move.surface_distance]] adds the length of the step on the relief, `sl_surface_`, from points every quarter of a cell along the step and the bilinear height at each.

### The fits

```tfgraph
#! caption: Three step selection functions: the relief at the end of the step (ssf_end), with the terms along it (ssf_full), and the model that a simulation can read (ssf_sim).
#! highlight: ssf_end, ssf_full, ssf_sim
#! fragment: true
[[nodes]]
id = "ssf_end"
op = "move.fit_issf"
inputs = { steps = "distance.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { formula = "case_ ~ energy + slope + tri + log(sl_) + cos(ta_) + log(sl_):slope" }

[[nodes]]
id = "ssf_full"
op = "move.fit_issf"
inputs = { steps = "distance.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { formula = "case_ ~ energy + slope + tri + dem_along_ascent + dem_along_delta + log(sl_) + cos(ta_) + log(sl_):slope" }

[[nodes]]
id = "ssf_sim"
op = "move.fit_issf"
inputs = { steps = "distance.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { formula = "case_ ~ energy + I(energy^2) + slope + I(height_end/1000) + I((height_end/1000)^2) + abs(height_end - height_start) + log(sl_) + cos(ta_) + log(sl_):slope" }
```

[[move.fit_issf]] reads the steps, the tentative distributions it keeps in the model (for the simulation) and a formula in R's syntax. The stratum is each step of each track (the `strata` default `track_step`: `step_id_` restarts in every track), so you do not need to build it. Three models are fitted:

- **`ssf_end`**: the relief at the end of the step, `energy + slope + tri`, with the movement terms `log(sl_)`, `cos(ta_)` and `log(sl_):slope`. The step's length and turn correct the distributions the random steps came from (Avgar et al., 2016); the interaction lets the length's effect change with the slope.
- **`ssf_full`**: the same, with the terms along the step, `dem_along_ascent` and `dem_along_delta`.
- **`ssf_sim`**: the model that can be simulated. It has only terms that the simulation reads at the ends of a step: the energy and its square (a preference for an energy in the middle of the range, not at its edges), the slope, the height of the cell at the end of the step and its square, in kilometres (the elevation band the ibex live in), and the net height change as `abs(height_end - height_start)`, which is the delta again, made from the heights at the two ends. The height terms are there because the simulation has no home range: without them the simulated animals wander through the whole massif (see below).

The formula takes `abs(...)` of a difference and `I(...)` of an expression with `+ - * / ^`, the functions `log`, `exp`, `sqrt`, `sin`, `cos`, `log1p` and `abs`, `factor(x)` and the interactions `a:b` and `a*b`. It does not take `pmax` or the other functions of R; make a column first ([[vector.calc]]).

### The log-RSS

```tfgraph
#! caption: The log-RSS of the slope (with the energy of a cell of that slope) against the median slope, and of a step that gains height, straight up or up and down, against a flat step.
#! highlight: rss_slope, rss_climb, rss_undulating
#! fragment: true
[[nodes]]
id = "rss_slope"
op = "move.log_rss"
inputs = { model = "ssf_sim.model" }
params = { x1 = { slope = [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0], energy = [2.03, 2.17, 2.69, 3.5, 4.65, 6.22, 8.19, 10.59, 13.59, 17.45, 22.21, 28.31, 36.2], height_end = [1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0], height_start = [1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0], sl_ = [46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0], ta_ = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] }, x2 = { slope = 33.3, energy = 9.77, height_end = 1915.0, height_start = 1900.0, sl_ = 46.0, ta_ = 0.0 }, ci = "se", level = 0.95 }

[[nodes]]
id = "rss_climb"
op = "move.log_rss"
inputs = { model = "ssf_full.model" }
params = { x1 = { dem_along_delta = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], dem_along_ascent = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], energy = [10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37], slope = [33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3], tri = [49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7], sl_ = [46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0], ta_ = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] }, x2 = { dem_along_delta = 0, dem_along_ascent = 0, energy = 10.37, slope = 33.3, tri = 49.7, sl_ = 46.0, ta_ = 0.0 }, ci = "se", level = 0.95 }

[[nodes]]
id = "rss_undulating"
op = "move.log_rss"
inputs = { model = "ssf_full.model" }
params = { x1 = { dem_along_delta = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], dem_along_ascent = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], energy = [10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37], slope = [33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3], tri = [49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7], sl_ = [46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0], ta_ = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] }, x2 = { dem_along_delta = 0, dem_along_ascent = 0, energy = 10.37, slope = 33.3, tri = 49.7, sl_ = 46.0, ta_ = 0.0 }, ci = "se", level = 0.95 }
```

[[move.log_rss]] needs a value of every covariate of the formula in both tables: `x1` the steps to compare (columns of lists) and `x2` the reference (one value each). `rss_slope` compares a step that ends on a slope of 0 to 60 degrees with one that ends on the median slope, 33.3 degrees, with the energy that a cell of each slope has (the median energy of the cells of the DEM in classes of 5 degrees), a step of 46 m (the median) and the same heights. `rss_climb` and `rss_undulating` compare a step that gains 10 to 100 m with a flat one, in the first case along a straight climb (ascent equal to net height change), in the second going up and down and ending at the same height (net height change 0).

### Paths, use and resistance

```tfgraph
#! caption: The control model of movement alone (plain), 40 paths simulated from the model of the relief (walk) and from the control (walk_plain), and the use of each as a share of the positions in each cell.
#! highlight: walk, use
#! fragment: true
[[nodes]]
id = "plain"
op = "move.fit_issf"
inputs = { steps = "distance.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { formula = "case_ ~ log(sl_) + cos(ta_)" }

[[nodes]]
id = "walk"
op = "move.simulate_path"
inputs = { model = "ssf_sim.model", start = "bursts.tracks", energy = "energy.energy", slope = "slope.map", height = "dem.raster" }
params = { n_steps = 300, n_paths = 10, n_control = 5000, seed = 11 }

[[nodes]]
id = "walk_plain"
op = "move.simulate_path"
inputs = { model = "plain.model", start = "bursts.tracks", energy = "energy.energy" }
params = { n_steps = 300, n_paths = 10, n_control = 5000, seed = 11 }

[[nodes]]
id = "use"
op = "move.path_density"
inputs = { paths = "walk.paths", like = "dem.raster" }

[[nodes]]
id = "use_plain"
op = "move.path_density"
inputs = { paths = "walk_plain.paths", like = "dem.raster" }
```

`plain` fits a model of movement alone, `log(sl_) + cos(ta_)`: the control, a simulation that does not know the relief. [[move.simulate_path]] takes the model, the covariates as inputs of any names (`energy`, `slope`, `height`, as the terms of the formula name them: `energy` at the end of the step, `height_end` and `height_start`), and the start: with a `start` of tracks, each track's first location is the start of `n_paths` paths, the direction that of its first step. Here 40 paths, 10 from each ibex's first location, of 300 steps (hours) each, with 5,000 candidate steps in each cycle (amt's default is a million: raise it for a final map). Path *i* uses the seed `seed + i - 1`. [[move.path_density]] counts the positions on the DEM's grid.

```tfgraph
#! caption: The resistance of the habitat terms of the model, Omniscape's current over it and the animals' positions against the resistance.
#! highlight: resistance, current, v_res
#! fragment: true
[[nodes]]
id = "resistance"
op = "move.resistance"
inputs = { model = "ssf_sim.model", energy = "energy.energy", slope = "slope.map", height = "dem.raster" }
params = { transform = "negexp", c = 8, r_max = 100 }

[[nodes]]
id = "current"
op = "circuit.omniscape"
inputs = { resistance = "resistance.resistance" }
params = { radius = 40, block_size = 11 }

[[nodes]]
id = "v_res"
op = "move.validate"
inputs = { use = "bursts.tracks", surface = "resistance.resistance" }
params = { kind = "resistance", quantile = 0.9 }
```

[[move.resistance]] keeps the terms of the model that describe the cell (the energy, its square, the slope and the height) and leaves out those of the movement and that of the net height change, which needs the start of a step; it says which. `negexp` with `c = 8` and `r_max = 100` makes the resistance 1 for the cell the model selects most and 100 for the one it selects least. [[circuit.omniscape]] gives the current that would flow if the animals moved in every direction at once, from every cell to those within 40 cells (1.2 km). [[move.validate]] puts the animals' positions over the resistance, as [part 7](../corridors-and-validation/) did over a current.

### The whole model

```tfgraph
#! caption: The whole model: the relief, the steps and their covariates, the three fits and the log-RSS, the paths and their use, the resistance, the current and the validation. It writes the DEM (dem.tif) and the resistance (resistance.tif).
format = "terraflux/1"

[[nodes]]
id = "read"
op = "move.read_tracks"
params = { path = "bargy_ibex_2013.csv" }

[[nodes]]
id = "utm"
op = "vector.reproject"
inputs = { vector = "read.tracks" }
params = { crs = "EPSG:32632" }

[[nodes]]
id = "resample"
op = "move.resample"
inputs = { tracks = "utm.vector" }
params = { rate = "1h", tolerance = "5min" }

[[nodes]]
id = "bursts"
op = "move.filter_min_n_burst"
inputs = { tracks = "resample.tracks" }
params = { min_n = 3 }

[[nodes]]
id = "steps"
op = "move.steps"
inputs = { tracks = "bursts.tracks" }

[[nodes]]
id = "sl_fit"
op = "move.fit_distribution"
inputs = { values = "steps.steps" }
params = { name = "gamma" }

[[nodes]]
id = "ta_fit"
op = "move.fit_distribution"
inputs = { values = "steps.steps" }
params = { name = "vonmises" }

[[nodes]]
id = "dem_ll"
op = "data.download"
params = { source = "copernicus_dem", dem_resolution = 30, extent = [6.38, 45.94, 6.52, 46.08], cellsize = 0.0003 }

[[nodes]]
id = "dem"
op = "raster.reproject"
inputs = { raster = "dem_ll.raster" }
params = { crs = "EPSG:32632", resolution = 30, resampling = "bilinear", options = "-dstnodata nan" }

[[nodes]]
id = "slope"
op = "surface.terrain"
inputs = { map = "dem.raster" }
params = { v = "slope", unit = "degrees" }

[[nodes]]
id = "tri"
op = "surface.tri"
inputs = { map = "dem.raster" }
params = { s = 3 }

[[nodes]]
id = "energy"
op = "cost.energy_landscape"
inputs = { dem = "dem.raster" }
params = { mass = 80, unit = "kcal", neigh = 8, model = "animal" }

[[nodes]]
id = "random"
op = "move.random_steps"
inputs = { steps = "steps.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { n_control = 10, seed = 1 }

[[nodes]]
id = "ends"
op = "move.extract_covariates"
inputs = { tracks = "random.steps", energy = "energy.energy", slope = "slope.map", tri = "tri.map" }
params = { where = "end" }

[[nodes]]
id = "climb"
op = "move.extract_covariates"
inputs = { tracks = "ends.tracks", dem = "dem.raster" }
params = { where = "along", stats = ["ascent", "delta"] }

[[nodes]]
id = "both"
op = "move.extract_covariates"
inputs = { tracks = "climb.tracks", height = "dem.raster" }
params = { where = "both" }

[[nodes]]
id = "distance"
op = "move.surface_distance"
inputs = { steps = "both.tracks", dem = "dem.raster" }

[[nodes]]
id = "ssf_end"
op = "move.fit_issf"
inputs = { steps = "distance.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { formula = "case_ ~ energy + slope + tri + log(sl_) + cos(ta_) + log(sl_):slope" }

[[nodes]]
id = "ssf_full"
op = "move.fit_issf"
inputs = { steps = "distance.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { formula = "case_ ~ energy + slope + tri + dem_along_ascent + dem_along_delta + log(sl_) + cos(ta_) + log(sl_):slope" }

[[nodes]]
id = "ssf_sim"
op = "move.fit_issf"
inputs = { steps = "distance.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { formula = "case_ ~ energy + I(energy^2) + slope + I(height_end/1000) + I((height_end/1000)^2) + abs(height_end - height_start) + log(sl_) + cos(ta_) + log(sl_):slope" }

[[nodes]]
id = "rss_slope"
op = "move.log_rss"
inputs = { model = "ssf_sim.model" }
params = { x1 = { slope = [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0], energy = [2.03, 2.17, 2.69, 3.5, 4.65, 6.22, 8.19, 10.59, 13.59, 17.45, 22.21, 28.31, 36.2], height_end = [1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0, 1915.0], height_start = [1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0, 1900.0], sl_ = [46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0], ta_ = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] }, x2 = { slope = 33.3, energy = 9.77, height_end = 1915.0, height_start = 1900.0, sl_ = 46.0, ta_ = 0.0 }, ci = "se", level = 0.95 }

[[nodes]]
id = "rss_climb"
op = "move.log_rss"
inputs = { model = "ssf_full.model" }
params = { x1 = { dem_along_delta = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], dem_along_ascent = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], energy = [10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37], slope = [33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3], tri = [49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7], sl_ = [46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0], ta_ = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] }, x2 = { dem_along_delta = 0, dem_along_ascent = 0, energy = 10.37, slope = 33.3, tri = 49.7, sl_ = 46.0, ta_ = 0.0 }, ci = "se", level = 0.95 }

[[nodes]]
id = "rss_undulating"
op = "move.log_rss"
inputs = { model = "ssf_full.model" }
params = { x1 = { dem_along_delta = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], dem_along_ascent = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], energy = [10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37, 10.37], slope = [33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3, 33.3], tri = [49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7, 49.7], sl_ = [46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0, 46.0], ta_ = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] }, x2 = { dem_along_delta = 0, dem_along_ascent = 0, energy = 10.37, slope = 33.3, tri = 49.7, sl_ = 46.0, ta_ = 0.0 }, ci = "se", level = 0.95 }

[[nodes]]
id = "plain"
op = "move.fit_issf"
inputs = { steps = "distance.steps", sl_distr = "sl_fit.distribution", ta_distr = "ta_fit.distribution" }
params = { formula = "case_ ~ log(sl_) + cos(ta_)" }

[[nodes]]
id = "walk"
op = "move.simulate_path"
inputs = { model = "ssf_sim.model", start = "bursts.tracks", energy = "energy.energy", slope = "slope.map", height = "dem.raster" }
params = { n_steps = 300, n_paths = 10, n_control = 5000, seed = 11 }

[[nodes]]
id = "walk_plain"
op = "move.simulate_path"
inputs = { model = "plain.model", start = "bursts.tracks", energy = "energy.energy" }
params = { n_steps = 300, n_paths = 10, n_control = 5000, seed = 11 }

[[nodes]]
id = "use"
op = "move.path_density"
inputs = { paths = "walk.paths", like = "dem.raster" }

[[nodes]]
id = "use_plain"
op = "move.path_density"
inputs = { paths = "walk_plain.paths", like = "dem.raster" }

[[nodes]]
id = "resistance"
op = "move.resistance"
inputs = { model = "ssf_sim.model", energy = "energy.energy", slope = "slope.map", height = "dem.raster" }
params = { transform = "negexp", c = 8, r_max = 100 }

[[nodes]]
id = "current"
op = "circuit.omniscape"
inputs = { resistance = "resistance.resistance" }
params = { radius = 40, block_size = 11 }

[[nodes]]
id = "v_res"
op = "move.validate"
inputs = { use = "bursts.tracks", surface = "resistance.resistance" }
params = { kind = "resistance", quantile = 0.9 }

[[nodes]]
id = "v_cur"
op = "move.validate"
inputs = { use = "bursts.tracks", surface = "current.current" }
params = { kind = "current", quantile = 0.9 }

[[nodes]]
id = "w_dem"
op = "raster.write"
inputs = { value = "dem.raster" }
params = { path = "dem.tif" }

[[nodes]]
id = "w_resistance"
op = "raster.write"
inputs = { value = "resistance.resistance" }
params = { path = "resistance.tif" }
```

Run it with ++ctrl+enter++ on the last node (or `terraflux run`). In the application, the nodes are in the library's subject **Animal movement**, and the template **Movement on the relief** has them; the file of the tracks is the one of node 1.

### In Python

The DEM comes from the model (it writes `dem.tif`, UTM 32N); everything else is the library, and the tracks are read from the same file:

```python
import numpy as np
import pandas as pd
import terraflux as tf

pd.set_option("display.width", 120, "display.max_columns", 10)
ID = "individual-local-identifier"
tracks = tf.load_tracks("bargy_ibex_2013.csv").to_crs("EPSG:32632")      # move.read_tracks, vector.reproject
print(len(tracks), tracks[ID].nunique(), "ibex")

dem = tf.load_raster("dem.tif")                                          # the DEM of the model
slope = tf.surface.terrain(dem, "slope")                                 # surface.terrain
tri = tf.surface.tri(dem, 3)                                             # surface.tri
energy = tf.cost.energy_landscape(dem, 80, unit="kcal")                  # cost.energy_landscape
print(dem.shape, int(dem.min()), int(dem.max()), round(float(energy.median()), 2))
```

```text
2643 4 ibex
(531, 378) 439 2406 5.26
```

The steps, the random steps and the covariates: the same calls as the nodes, in the same order.

```python
bursts = tf.move.filter_min_n_burst(tf.move.resample(tracks, "1h", "5min"), 3)
steps = tf.move.steps(bursts)
sl = tf.move.fit_distr(steps["sl_"], "gamma")                            # move.fit_distribution
ta = tf.move.fit_distr(steps["ta_"], "vonmises")
rnd = tf.move.random_steps(steps, n_control=10, sl_distr=sl, ta_distr=ta, seed=1)
data = tf.move.extract_covariates(rnd, {"energy": energy, "slope": slope, "tri": tri})
data = tf.move.along_summary(data, {"dem": dem}, stats=("ascent", "delta"))            # where = "along"
data = tf.move.extract_covariates(data, {"height": dem}, where="both")
data = tf.move.surface_distance(data, dem)
data["stratum"] = data[ID] + "_" + data["step_id_"].astype(int).astype(str)
print(len(steps), len(data), int(data["case_"].sum()), "observed steps")
cols = ["slope", "tri", "energy", "dem_along_ascent", "dem_along_delta", "sl_", "sl_surface_"]
print(data.groupby("case_")[cols].mean().round(2))
```

```text
2578 27687 2517 observed steps
       slope    tri  energy  dem_along_ascent  dem_along_delta    sl_  sl_surface_
case_                                                                             
False  32.41  51.36   11.32             22.08            37.55  98.32       110.44
True   32.47  51.67   11.48             24.41            27.87  98.75       106.61
```

`tf.move.fit_issf` is amt's `fit_issf` (R's `clogit`, exact partial likelihood), with the formula in R's syntax. The strata are a column of the formula:

```python
full = tf.move.fit_issf(data, "case_ ~ energy + slope + tri + dem_along_ascent + dem_along_delta + log(sl_) + cos(ta_) + "
                              "log(sl_):slope + strata(stratum)")
print(full.summary().round(4))
print(full.confint().round(4).loc[["dem_along_ascent", "dem_along_delta"]])
print(round(full.loglik[1], 1), round(full.aic, 1), full.nevent, "steps")
```

```text
                    coef  exp_coef      se        z       p
energy            0.0767    1.0797  0.0317   2.4217  0.0155
slope             0.0203    1.0205  0.0117   1.7306  0.0835
tri              -0.0278    0.9726  0.0140  -1.9863  0.0470
dem_along_ascent  0.0046    1.0046  0.0006   7.3687  0.0000
dem_along_delta  -0.0086    0.9914  0.0007 -12.1510  0.0000
log(sl_)          0.1449    1.1560  0.0378   3.8315  0.0001
cos(ta_)         -0.0098    0.9902  0.0296  -0.3315  0.7402
slope:log(sl_)   -0.0021    0.9979  0.0011  -1.9185  0.0550
                   2.5 %  97.5 %
dem_along_ascent  0.0034  0.0058
dem_along_delta  -0.0100 -0.0072
-5906.8 11829.6 2507 steps
```

The log-RSS of a step that gains height straight up, against a flat one:

```python
x1 = pd.DataFrame({"dem_along_ascent": np.arange(0, 101, 20.0), "dem_along_delta": np.arange(0, 101, 20.0),
                   "energy": 10.37, "slope": 33.3, "tri": 49.7, "sl_": 46.0, "ta_": 0.0})
x2 = x1.iloc[[0]].assign(dem_along_ascent=0.0, dem_along_delta=0.0)
print(tf.move.log_rss(full, x1, x2, ci="se")[["dem_along_ascent_x1", "log_rss", "lwr", "upr"]].round(3))
```

```text
   dem_along_ascent_x1  log_rss    lwr    upr
0                  0.0    0.000  0.000  0.000
1                 20.0   -0.080 -0.109 -0.052
2                 40.0   -0.161 -0.218 -0.103
3                 60.0   -0.241 -0.327 -0.155
4                 80.0   -0.322 -0.436 -0.207
5                100.0   -0.402 -0.545 -0.259
```

The short form, `tf.move.issf`, does the sequence from the tracks to the fit (resample, steps, random steps, covariates at the end, fit) in one call. It draws the random steps from distributions fitted to each animal (amt's default), not from one pair for all, so its numbers are not those of the model above:

```python
short = tf.move.issf(tracks, {"energy": energy, "slope": slope}, interval="1h", tolerance="5min", n_control=10, seed=1,
                     formula="case_ ~ energy + slope + log(sl_) + cos(ta_)")
print(short.summary().round(4)[["coef", "se"]])
```

```text
            coef      se
energy    0.0291  0.0089
slope    -0.0123  0.0050
log(sl_)  0.0108  0.0110
cos(ta_) -0.0297  0.0296
```

The model that can be simulated, its 40 paths and their use, from the model of the page:

```python
sim_formula = ("case_ ~ energy + I(energy^2) + slope + I(height_end/1000) + I((height_end/1000)^2) + "
               "abs(height_end - height_start) + log(sl_) + cos(ta_) + log(sl_):slope + strata(stratum)")
model = tf.move.fit_issf(data, sim_formula)
print(model.summary().round(4)[["coef", "se", "z"]])

layers = {"energy": energy, "slope": slope, "height": dem}               # the terms name them
paths = []
for animal, g in bursts.groupby(ID):                                     # 10 paths from the first location of each animal
    p0, p1 = g.geometry.iloc[0], g.geometry.iloc[1]
    start = tf.move.make_start((p0.x, p0.y), float(np.arctan2(p1.y - p0.y, p1.x - p0.x)), time=g["timestamp"].iloc[0], dt=3600)
    for j in range(10):
        kernel = tf.move.redistribution_kernel(model, start, layers, n_control=5000, seed=11 + j)
        paths.append(tf.move.simulate_path(kernel, 300).assign(track=f"{animal}_path_{j + 1}"))
paths = pd.concat(paths, ignore_index=True)
use = tf.move.path_density(paths[paths["step_"] > 0], dem)             # move.path_density
print(paths["track"].nunique(), "paths,", len(paths), "positions;", int((use > 0).sum()), "cells used, shares sum to", round(float(use.sum()), 6))
```

```text
                                   coef      se       z
energy                           0.1070  0.0239  4.4832
I(energy^2)                     -0.0019  0.0005 -3.9416
slope                           -0.0159  0.0076 -2.0870
I(height_end/1000)              10.7088  3.7669  2.8429
I((height_end/1000)^2)          -2.2680  0.9896 -2.2919
abs(height_end - height_start)  -0.0065  0.0007 -9.8395
log(sl_)                         0.1701  0.0393  4.3284
cos(ta_)                        -0.0084  0.0296 -0.2842
slope:log(sl_)                  -0.0019  0.0011 -1.6786
40 paths, 12040 positions; 7300 cells used, shares sum to 1.0
```

The resistance, the current and the animals against them:

```python
res = tf.move.resistance(model, layers, transform="negexp", c=8, r_max=100)       # move.resistance
current = tf.omniscape(res, radius=40, block_size=11)["cum_currmap"]                # circuit.omniscape
val = tf.move.validate(bursts, res, kind="resistance", quantile=0.9)                 # move.validate
print(res.shape, round(float(res.min()), 2), round(float(res.max()), 2), round(float(current.max()), 1))
print(val.table.round(3).tail(1).to_string(index=False))
```

```text
(531, 378) 1.0 100.0 22.1
 track  n_cells   rho  use_fraction  area_fraction  use_ratio  surface_used  surface_available  coverage
pooled    56550 0.173         0.653          0.201       3.25        12.955             42.678       1.0
```

`tf.move.resistance` warns that the term of the net height change, which needs the start of a step, is left out. The functions take `n_threads`; the results do not depend on it. The files of this part's figures are made by `website/figures/animal-movement/movement-on-the-relief.py`, which runs the operators and the library on the same data and compares them.

## Read the results

### The area and what a step costs

![The relief, the four ibex, the energy of walking and its relation to the slope](/img/tutorials/animal-movement/movement-on-the-relief/area.png "Left: the Copernicus DEM (hillshade, tinted by height) with the 2,517 observed steps' end points of the four ibex, the 2 km scale at the bottom. Middle: the energy of walking onto each cell for an animal of 80 kg, in kilocalories on a log scale. Right: the cells of the DEM, energy against slope, with the median in orange: from 2 kcal on the level to 36 kcal at 60 degrees.")

The ibex live on one side of the massif, on the long slope that runs from the south-west to the north-east, with a few excursions to the north and the south. The energy map follows the slope and not the height: it is high on the steep walls and low on the plateau and in the valleys, from about 2 kcal on level ground to more than 30 on the steepest cells. The curve on the right is nearly a function of the slope alone, because the cost per metre depends on the slope and the cells are of one size; the scatter around it comes from the height changes among the neighbours of a cell.

### What a step on the relief is

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/used-available.csv
digits: 1
caption: The observed steps and their 25,120 random steps: the slope, the TRI and the energy at their end, the ascent and the net height change along them, their length on the plane and on the relief, and how much longer the second is (in percent). The two rows have the same length of step.
```

At the *end* of the step, the observed and the random steps are the same: a mean slope of 32.5 and 32.4 degrees, a TRI of 51.7 and 51.4 m, an energy of 11.5 and 11.3 kcal. These are steep slopes for an animal that does not move uphill or downhill by chance, and the ibex's steps do not end on easier cells than the random ones. Along the step it is different. The observed steps change height by 27.9 m on average, the random ones by 37.6 m (a quarter more), and in the median 14.7 and 17.6 m; on the other hand the observed ones climb a little more, 24.4 against 22.1 m of ascent. As a result a step is 8.0 % longer on the relief than on the plane for the animals and 12.3 % longer for the random steps, of the same length on the plane (98.7 and 98.4 m): the animals' steps are the less steep ones.

![One stratum of I18 over the contour lines](/img/tutorials/animal-movement/movement-on-the-relief/stratum.png "One stratum of the ibex I18: the observed step of 166 m (orange), from the black point, and its ten random steps, coloured by their net height change (from light green, 0 m, to dark blue, 90 m), over the 30 m DEM with contour lines every 10 m. The observed step follows the contour lines to the south and changes height by 1.2 m. Five of the random steps cross the slope and change height by 64 to 91 m.")

The figure shows one stratum, the first of the ten with an observed step of 110 to 180 m that changes height by 2 m or less while the mean of its random steps changes it by more than 30 m: five of the ten random steps change height by more than 60 m, the observed one by 1.2 m. It is a chosen example, not the rule: the rule is the coefficient below.

![The net height change and the ascent of the observed and random steps](/img/tutorials/animal-movement/movement-on-the-relief/height-change.png "The distribution of the net height change along the step (left) and of its ascent (right), for the 2,517 observed steps and their 25,120 random steps. The last class holds the steps of 150 m and more.")

The difference lies at the two ends of the distribution of the net height change: more observed steps change height by under 5 m (a density of 0.071 against 0.064 in the first class) and fewer by 150 m or more (0.004 against 0.011). The distributions of the ascent are close, with fewer observed steps in the first class (0.102 against 0.109).

### The coefficients

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/coefficients-end.csv
digits: 4
caption: The step selection function with the relief at the end of the step (ssf_end, 2,507 steps): coefficient, standard error, 95 % interval, exp(coefficient), z and p. Log-likelihood −6,001.2 (the model without covariates: −6,011.5).
```

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/coefficients-full.csv
digits: 4
caption: The same with the terms along the step (ssf_full): the ascent and the net height change. Log-likelihood −5,906.8, AIC 11,829.6 (that of the model above: 12,014.4).
```

The terms at the end of the step say little. The energy has a positive coefficient (0.106, interval 0.045 to 0.166), which means that the ibex chose, if anything, the *costlier* cells; the slope and the TRI have coefficients of opposite signs and intervals that touch 0. They should not be read one by one: the slope, the TRI and the energy of the cells under the steps are correlated at 0.90 to 0.98, which the table below shows.

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/correlations.csv
digits: 2
heat: true
caption: The correlations of the relief's covariates at the end of the steps (slope, TRI, energy) and along them (ascent, net height change), of the 27,687 steps.
```

Taken one at a time, none of them is clear:

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/one-at-a-time.csv
digits: 4
caption: Each relief covariate alone in a model with log(sl_) and cos(ta_) (library): its coefficient, standard error, z, and the log-likelihood of the model. The movement terms alone give −6,011.2.
```

The slope (0.001, z 0.4), the TRI (0.001, z 1.0) and the energy (0.008, z 1.8) do not improve the model by more than 1.6 log-likelihood units, the ascent alone by 3.0, against 72 for the net height change along the step. The step selection function does not say that the ibex avoid steep slopes, rugged ground or costly cells.

The terms along the step do. Adding them to the model takes the log-likelihood from −6,001.2 to −5,906.8, and the Akaike information criterion from 12,014 to 11,830. The **net height change** has the coefficient −0.0086 per metre (interval −0.0100 to −0.0072, z −12.2): a step that changes height by 10 m is 8 % less likely to be chosen than one that does not, all else equal (exp(−0.086) = 0.92). The **ascent** has a positive one, 0.0046 per metre (0.0034 to 0.0058).

Both signs are needed to read it. For a given net change, an animal that goes up and down along the step is *more* likely to take it than one that goes straight, and for a given ascent, a smaller net change is more likely. A step that follows a contour line (a net change near 0) over a ground that undulates is the most likely of the three, and a step straight up a slope the least.

### The relative selection strength

![The log-RSS of the slope and of the height gained along the step](/img/tutorials/animal-movement/movement-on-the-relief/log-rss.png "Left: the log-RSS of a step that ends on a slope of 0 to 60 degrees (with the energy a cell of that slope has) against one that ends on the median slope, 33.3 degrees, from the model that can be simulated; the band is the 95 % interval. Right: the log-RSS of a step that gains 10 to 100 m along a straight climb (orange) and of one that goes up and down by the same ascent and ends at the same height (blue), against a flat step, from the model with the terms along the step.")

On the left, the slope's selection never goes past a log-RSS of −0.10 and +0.18 (a relative selection strength of 0.91 and 1.19) between 0 and 55 degrees, and at 20 degrees, where it is lowest, the interval is −0.19 to −0.01. The curve has a shallow U shape, a small preference for the gentlest cells and for the steep ones (45 to 50 degrees) over the middle, the sum of the terms of the energy (which rises and falls) and of the slope. With the intervals around 0 over most of the range, it is an effect that is not large.

On the right the effect is clearer and of the sign the question asked. A step that climbs 50 m straight up has a log-RSS of −0.20 (interval −0.27 to −0.13) against a flat step, a relative selection strength of 0.82; a straight climb of 100 m, −0.40 (−0.55 to −0.26), a strength of 0.67. A step of the same ascent that ends at the same height gains instead: +0.23 at 50 m (a strength of 1.26) and +0.46 at 100 m (1.58). The ibex's preference is not for flat steps but for **steps that end at the height where they started**.

### Is it the same for the four animals?

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/animals.csv
digits: 4
caption: The coefficients of the energy, the ascent and the net height change of ssf_full fitted to each ibex alone (library), with the 95 % interval. About 620 steps each.
```

Yes for the net height change: it is negative for all four, between −0.0065 and −0.0104 per metre, with intervals that do not touch 0. The ascent is positive for three (0.0044 to 0.0069) and null for I27. The energy is the least stable: only I27 has a clearly positive coefficient (0.25, 0.11 to 0.39), the other three are around 0, with intervals that include it. The pooled energy coefficient is mostly I27's. With four animals, you cannot say whether that is a difference between individuals or between the ground each one lives on.

### Paths simulated from the model

![The positions of the ibex and the use of paths simulated with the relief and without](/img/tutorials/animal-movement/movement-on-the-relief/paths.png "Left: the end points of the observed steps of the four ibex, smoothed (a Gaussian filter of 4 cells). Middle: the 40 paths of 300 steps simulated from the model with the relief (grey lines) and their use, smoothed the same way. Right: the same from a model of movement alone. Each panel's colours are scaled to its own maximum.")

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/simulation.csv
digits: 1
caption: The observed positions and the 40 paths simulated from the model with the relief and from the model of the movement alone: the mean step, the net height change per step, the slope, the energy and the height at the positions.
```

The paths are a test of the model that the coefficients alone are not. The model with the relief, started where the ibex were, stays in their band of heights: 1,758 m on average (standard deviation 227) against 1,905 m (235) for the animals, with a net height change of 25.7 m per step against 27.9 (the movement-only paths: 32.4, as the random steps are) and a slope of 29.7 degrees against 32.5. The movement-only model spreads over the whole massif (a standard deviation of 395 m), and 16 of its 40 paths leave the landscape before 300 steps; none of the 40 paths of the model with the relief does. Where the paths go is moderately like where the ibex were: in cells of 300 m, the rank correlation of the use of the animals and of the paths with the relief is 0.34, and 47 % of the paths' use falls in the 135 cells the animals used; for the movement-only paths, 0.25 and 32 %.

What the simulation cannot do is stay: the animals used 135 of the 1,961 cells of 300 m, the 40 paths with the relief 476 and those of the movement alone 684, because the model selects cells and has no home range, and the paths started from the animals' first locations drift. And the simulated animals are less steep than the real ones (a mean energy of 10.0 kcal against 11.5), because the energy's coefficient and its square put the optimum at a slope the cells of the ibex's belt have only in part. **A simulation of a step selection function is a test of what it has captured, not a forecast of where an animal will be.**

### A resistance and a current

![The resistance of the habitat terms and Omniscape's current](/img/tutorials/animal-movement/movement-on-the-relief/connectivity.png "Left: the resistance made from the energy, the slope and the height terms of the model, 1 (the cells it selects most) to 100 (least), with the ibex's positions in white. Right: the current of Omniscape over it (log scale), with a window of 40 cells and a target every 11.")

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/validation.csv
digits: 3
caption: The ibex's positions against the resistance (lower is better) and against the current, the best 10 % of the cells of each surface as the corridors, as in part 7: Spearman's rho, the share of the use in them, the share of the compared area they are (the cells inside the bounding box of each animal's positions) and their quotient.
```

The resistance is low along the belt where the animals live and high away from it, to the west and the north of the area: the model's preference for a band of heights and for an energy in the middle of its range. Omniscape's current follows it and channels through the belt. Against the animals' positions the best 10 % of the cells catch 65 % of the use where they are 20 % of the compared area, a use ratio of 3.25 (3.15 for the current). By animal, the ratio of the resistance is 6.0 for I10, 2.0 for I18, 0.86 for I27 and 0.91 for I9: the surface says much about two animals and little about the others.

**Do not read the ratio as a validation.** The animals that made the model are the animals that are tested against it; the positions are the data of the fit. [Part 7](../corridors-and-validation/) validated a surface against animals that did not make it, and the resistance of this page needs the same: other ibex, another year, or the animals' movement between ranges. The resistance also leaves out the term that matters most in the step selection function, the net height change, because a cell has no start: it is a map of which cells the ibex choose, not of how they cross them.

## Settings that change the answer

- **The body mass.** The energy is proportional to it, so the coefficient is inversely proportional: 0.140 per kcal for 40 kg, 0.077 for 80 and 0.054 for 120, with a z of 2.4 for all of them, a log-likelihood of −5,906.8 for all, and a coefficient of the net height change of −0.0086 for all.

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/body-mass.csv
digits: 4
caption: ssf_full with the energy of an animal of 40 to 120 kg (library): the mean energy at the end of the observed steps, its coefficient and standard error, the coefficient of the net height change and the log-likelihood. Only the scale of the energy changes.
```

- **The number of random steps.** The coefficient of the net height change is −0.0099 with 2 random steps per step, −0.0091 with 5, −0.0086 with 10, −0.0089 with 20 and −0.0087 with 50 (the intervals are of ±0.0014 from 10 on): stable from 5. The energy's coefficient moves more, from 0.059 to 0.079.

```tftable
data: /data/tutorials/animal-movement/movement-on-the-relief/controls.csv
digits: 4
caption: ssf_full with 2 to 50 random steps for each observed step (seed 1, library): the coefficient of the net height change with its 95 % interval, those of the ascent and the energy.
```

- **Which terms are in the model.** The slope, the TRI and the energy are correlated at 0.9 or more: change one of them and the signs of the others change. The net height change keeps its sign and size: −0.0071 alone, −0.0086 with the ascent and the terms at the end of the step, −0.0065 as `abs(height_end - height_start)` in the model that is simulated.
- **The tentative distributions** (pooled here, per animal in `tf.move.issf`) and **the seed** of the random steps: report them, as in part 5.
- **The DEM's resolution and the step's interval.** A DEM of 30 m under steps of 99 m on average (an hour apart) sees a step as three cells long. A finer DEM or a longer interval would measure other things; compare only models at the same ones.
- **The energy's model.** `model = "animal"` is Pontzer's allometric law for walking animals in general; the model of people (`"human"`, with a speed) is Looney et al.'s equation. For a species with a cost of locomotion measured, replace the surface with your own.
- **The number of paths, their length and `n_control`** of the simulation: the use map of 40 paths of 300 steps is grainy; amt's default of a million candidates is 200 times the page's. Raise them for a map you will report.

## Common pitfalls

- **The DEM's edges.** A DEM put on a projection by gdalwarp has corners that the source does not cover. Filled with 0 m they make cliffs of hundreds of metres beside the data; set the no data (`-dstnodata nan`) as node 2 does.
- **A DEM in degrees or feet.** The energy and the length on the relief need metres in both the heights and the distances; the nodes stop when the coordinate system is in degrees.
- **Correlated covariates.** Slope, TRI and energy are one thing seen three times. Look at their correlations, fit them one at a time and together, and read them with the log-RSS.
- **Terms along the step in a simulation.** The simulator reads the end of a step and its start. Make from the heights at the two ends what you can (`abs(height_end - height_start)` is the delta), and leave the ascent out, saying so.
- **No home range.** A simulation of a step selection function has no memory of where it has been: terms of the elevation band or of a distance to a centre keep it near the animal's range, but they are a choice of the modeller, not an estimate. Without them the paths wander.
- **Validating a resistance with the animals that made it.** See above.
- **Body mass.** Give it, and check the coefficient on the scale of the energy it was fitted at.
- **Reading a coefficient on the energy as a cost for the animal.** It is a preference over cells that cost more, at a given mass; an animal that chooses the costlier cells is not an animal that spends more.

## What the operators and the library agree on

The operators and the library were run on the same file (the DEM of the model, the same tracks). The covariates of the 27,687 rows (the energy, the slope, the TRI, the ascent, the delta and the heights at the two ends) are the same to the last bit, the lengths of the steps differ by 1e-9 m (the tracks are projected by two programs) and the lengths on the relief by 2e-8 m, and the three fits agree to 1e-13 in the coefficients and in their standard errors. The first simulated path (301 points) is the same to 1.5e-8 m. The simulation is the library's, which is amt's with R's generator (the same seed gives the same path as amt's `simulate_path`).

- **amt** (0.3.1; Signer et al., 2019) and **survival** (3.7-0): `fit_issf` and `log_rss` are bit for bit with R, as is `simulate_path` of the kernel of continuous space, as the library's documentation says; this page did not run R. `extract_covariates` is amt's with terra's `extract`, `extract_covariates_along` is the one of part 7. The bootstrap interval of `log_rss` (`ci = "boot"`) is amt's with three corrections that the library lists.
- **enerscape** (1.2.0; Berti et al., 2022): `cost.energy_landscape` is bit for bit with it, the cells' means in 80-bit arithmetic as its C++ does.
- **Not in R**: `move.surface_distance`, `move.path_density` (the use of paths, without the kernel density estimate of Signer et al., 2024), `move.resistance` (the formulas are the library's, from the literature's shapes) and `move.validate` (part 7).

## The template

**Movement on the relief** (library subject **Animal movement**, group *Movement*) runs a version of this page's model on a track that it simulates: a DEM from a fractal surface scaled to 500 to 2,500 m, its slope, its ruggedness and the energy of an animal of 70 kg, a track of 400 steps simulated from a step selection model whose coefficients are given by hand (an animal that avoids the energy), the steps with their random steps, the relief at their end and along them, the length on the relief, the fit with and without the terms along the step, the log-RSS of the slope, ten simulated paths with their use, the resistance and Omniscape's current. The coefficients are given by hand ([[move.issf_model]]), so the energy's coefficient comes out with the sign it was given: a test of the method. The DEM and the track are the data of **Use my data**: replace the DEM with yours (metres, projected, square cells), the track with [[move.read_tracks]] of your Movebank file (its time column is `timestamp`), and the mass with your animal's.

## Going further

- [The data of step selection functions](../step-selection/): the steps and random steps, and the forest as the covariate, which this part replaces with the relief.
- [Observed corridors and validating connectivity](../corridors-and-validation/): how to validate a surface against animals, and the first look at the values along the steps.
- [Circuit connectivity](../../circuit-connectivity/): resistance surfaces and Omniscape.
- The operators' reference: [[cost.energy_landscape]], [[move.surface_distance]], [[move.fit_issf]], [[move.log_rss]], [[move.issf_model]], [[move.simulate_path]], [[move.path_density]], [[move.resistance]], [[move.extract_covariates]], [[circuit.omniscape]].

## References

- Avgar, T., Lele, S. R., Keim, J. L., & Boyce, M. S. (2017). Relative selection strength: quantifying effect size in habitat- and step-selection inference. *Ecology and Evolution*, 7(14), 5322-5330. https://doi.org/10.1002/ece3.3122
- Avgar, T., Potts, J. R., Lewis, M. A., & Boyce, M. S. (2016). Integrated step selection analysis: bridging the gap between resource selection and animal movement. *Methods in Ecology and Evolution*, 7(5), 619-630. https://doi.org/10.1111/2041-210X.12528
- Berti, E., Davoli, M., Buitenwerf, R., Dyer, A., Hansen, O. L. P., Hirt, M., Svenning, J.-C., Terlau, J. F., Brose, U., & Vollrath, F. (2022). The r package enerscape: a general energy landscape framework for terrestrial movement ecology. *Methods in Ecology and Evolution*, 13(1), 60-67. https://doi.org/10.1111/2041-210X.13734
- European Space Agency, & Airbus (2022). Copernicus DEM. European Space Agency. https://doi.org/10.5270/esa-c5d3d65
- Fortin, D., Beyer, H. L., Boyce, M. S., Smith, D. W., Duchesne, T., & Mao, J. S. (2005). Wolves influence elk movements: behavior shapes a trophic cascade in Yellowstone National Park. *Ecology*, 86(5), 1320-1330. https://doi.org/10.1890/04-0953
- Keeley, A. T. H., Beier, P., & Gagnon, J. W. (2016). Estimating landscape resistance from habitat suitability: effects of data source and nonlinearities. *Landscape Ecology*, 31(9), 2151-2162. https://doi.org/10.1007/s10980-016-0387-5
- Looney, D. P., Santee, W. R., Hansen, E. O., Bonventre, P. J., Chalmers, C. R., & Potter, A. W. (2019). Estimating energy expenditure during level, uphill, and downhill walking. *Medicine & Science in Sports & Exercise*, 51(9), 1954-1960. https://doi.org/10.1249/MSS.0000000000002002
- Marchand, P., Risser, N., Petit, E., & Garel, M. (2025). Disturbance by massive sporting events in mountain areas: when and where matters for the protected Alpine ibex *Capra ibex*. *People and Nature*, 7(11), 3002-3016. https://doi.org/10.1002/pan3.70151
- Marchand, P., Risser, N., Petit, E., & Garel, M. (2025). Data from: Disturbance by massive sporting events in mountain areas: when and where matters for the protected Alpine ibex *Capra ibex* [Data set]. Zenodo. https://doi.org/10.5281/zenodo.17052197
- Pontzer, H. (2016). A unified theory for the energy cost of legged locomotion. *Biology Letters*, 12(2), 20150935. https://doi.org/10.1098/rsbl.2015.0935
- Signer, J., Fieberg, J., & Avgar, T. (2019). Animal movement tools (amt): R package for managing tracking data and conducting habitat selection analyses. *Ecology and Evolution*, 9(2), 880-890. https://doi.org/10.1002/ece3.4823
- Signer, J., Fieberg, J., Reineking, B., Schlägel, U., Smith, B., Balkenhol, N., & Avgar, T. (2024). Simulating animal space use from fitted integrated step-selection functions (iSSF). *Methods in Ecology and Evolution*, 15(1), 43-50. https://doi.org/10.1111/2041-210X.14263
- Zeller, K. A., McGarigal, K., & Whiteley, A. R. (2012). Estimating landscape resistance to movement: a review. *Landscape Ecology*, 27(6), 777-797. https://doi.org/10.1007/s10980-012-9737-0
