---
title: Observed corridors and validating connectivity
short: Corridors and validation
description: Find the stretches of a track that run along a corridor (LaPoint et al.'s criterion), check whether the animals use the cells a connectivity model says they should (Omniscape's current, Zeller et al.'s question), measure how far apart two utilisation distributions are, and ask from the values along each step whether an animal climbs a hill or follows a contour line.
order: 70
level: advanced
time: 80 min
ops: [move.corridor, move.validate, move.ud_distance, move.extract_covariates, move.dbbmm_ud, circuit.omniscape]
series: Animal movement
part: 7
template: Observed corridors against current
---

A connectivity model says where an animal should move between its habitats: a least-cost corridor, a map of the current
that Circuitscape or Omniscape draws. Animals with GPS collars have been there and can say whether it is true. In this part
you will lay the fishers' movement over an Omniscape current made from the land cover and ask three questions of it. Where
did the animals *run* straight along something that looks like a corridor? Do the cells the model says are best for
movement catch more of the animals' use than their share of the area? And, in the other direction, how different are the
places two animals used? At the end you will have the corridor segments of the seven fishers as lines, a table that scores
a surface against the use of the animals, the distances between their utilisation distributions, and a first look, from the
elevation along each step, at the question the next block of this series asks: does an animal follow the contour lines?

The answer to the second question is not the one that the model's author would like, and that is the point of validating.

## The idea

### Corridors in the tracks

A **corridor** in a landscape model is a route the model says animals will take. A **real corridor**, in LaPoint et al.
(2013), is the stretch of tracking data in which animals *did*: the segments of their tracks that are fast, run in a
consistent direction, and are used again and again. Their criterion, which move2utils (Kranstauber et al., 2026) implements
as `mt_corridor`, takes each segment of a track (the step from one location to the next) and asks three things:

1. **Is it fast?** Its speed is at least a threshold: an animal moving through a place, not feeding in it.
2. **Do the segments around it agree?** The neighbours are the segments, of all the tracks together, whose midpoints lie
   within half its own length of its midpoint; their directions have a *circular variance* at most a threshold.
3. **Are there enough of them, and do most qualify?** At least `min_segments` neighbours, and strictly more neighbours that
   satisfy 1 and 2 than not.

!!! science "Key idea: the circular variance of directions"
    Directions are angles, and the mean of 359° and 1° is not 180°. A set of directions \( \theta_i \) is summarised by the
    length of the mean of their unit vectors,

    \[ R = \sqrt{\left(\tfrac{1}{n}\sum \cos\theta_i\right)^2 + \left(\tfrac{1}{n}\sum \sin\theta_i\right)^2}, \qquad
    V = 1 - R. \]

    \( V \) is the **circular variance**: 0 when all the directions are the same, near 1 when they are spread evenly.
    Two animals that walk the same path in opposite directions should agree, so the criterion uses the *pseudo-azimuths*
    \( 2\theta \bmod 2\pi \): a direction and its opposite give the same pseudo-azimuth.

The speed and variance thresholds are the whole decision. If you leave them out, the node uses a quantile of your own tracks
(the upper quartile of the speeds, the lower quartile of the variances) and warns: a quarter of the segments pass the speed
test whatever the animals do, and the corridors found are relative to the data. For corridors that can be compared between
animals, seasons or sites, give both thresholds, computed once on all of them.

### Does the surface match the use?

Zeller et al. (2018) validated connectivity models with the GPS locations of dispersing pumas: they compared how well
resistance surfaces, least-cost corridors and circuit-theory current *captured* where the animals went. That is the question
of [[move.validate]]. The surface gives each cell a score (a current, a cost-distance kernel, minus a resistance); the
**corridors of the surface** are the cells with the best scores (the top 10 %, or those past a threshold). The use of the
animals is a utilisation distribution, their locations, or their corridor segments. The table has, for each animal and for
all of them pooled:

- **`rho`**: Spearman's rank correlation between the use and the score over the cells (positive always means the animals use
  the better cells more, also for a resistance). No p-value: cells next to each other are not independent.
- **`use_fraction`**: the share of the use inside the corridors of the surface;
- **`area_fraction`**: the share of the compared cells that are corridors, what a use blind to the surface would give;
- **`use_ratio`**: their quotient, *used against available*: above 1 the corridors catch more of the use than their area,
  1 is chance.

!!! warning "Our definitions"
    Of Zeller et al. (2018) only the abstract was available when this operator was written: the measures above are the usual
    ones of that kind and **not a copy of the paper's**. Cite the paper for the idea, and say what the node computes
    (the definitions above and in the operator's reference) rather than "Zeller's validation".

### The distance between distributions

How far apart are two animals' ranges? An overlap index (part 6) says how much they share, and says nothing of the ground
between two ranges that share none: two ranges 1 km apart and two 30 km apart both overlap by 0. The **earth mover's
distance** (EMD; Rubner et al., 2000) is the minimum work needed to turn one distribution into the other, moving
probability as one would earth: the sum, over pairs of cells, of the probability moved times the distance between the cells,
least over all ways of moving it. It is in metres, and it grows with the distance between the ranges. It is exact on a small
grid and slow on a large one: **Sinkhorn's** algorithm (Cuturi, 2013) adds an entropy term that makes the problem a pair
of matrix scalings, far faster, at the price of a bias that the regularisation (`reg`) controls.

### Climbing, or following a contour

The covariates of a step selection function ([part 5](../step-selection/)) are taken at the *end* of the step. The step
itself crosses cells, and the cells it crosses tell more. For a step over a DEM, the node [[move.extract_covariates]] with
`where = along` gives, with the cells in order from the start to the end of the step: the **ascent**, the sum of the rises
between successive cells; the **descent**, the sum of the falls (a positive number); and the **delta**, the difference
between the last and the first. A step that follows a contour line has a small ascent and a small descent; one that goes
straight up a slope has an ascent equal to its delta. The question of how animals move on a relief, which the next block
of this series takes up, starts from them.

## When to use it

- **To check a connectivity model** (cost-distance, Circuitscape, Omniscape) against the movement of animals, ideally *other*
  animals than the ones that fed the model, and ideally *dispersing* ones: a surface for the movement between habitats is not
  tested by the home range of a resident.
- **To find the corridors in tracking data**, as a map of where the animals ran and used the same way again and again.
- **To compare animals' ranges** by how far apart they are, not only by how much they share.
- **To prepare** covariates for a step selection function that asks about the relief.

What it is not: a corridor segment is a property of the movement, not of the landscape. It does not say that the animals
need that route.

## What you need

- The **tracks**, projected on a local map in metres, at **one sampling interval**: the search radius of the corridor
  criterion is half a step, so a mix of intervals makes the neighbourhoods mean different things (the node warns). Part 8 thins
  with [[move.thin_time]]; here the seven fishers near Albany are at one location per 30 minutes (M5, 40 km east of the
  rest, is left out).
- A **surface**: the resistance or the current, as a raster in the same coordinate system. Here the land cover of ESA
  WorldCover 2021 downloaded by [[data.download]], put on the same projection at cells of 100 m, and a resistance assigned
  to each class, **an illustration and not a calibrated model**: tree cover 1, shrubland 5, grassland 10, cropland 20, bare
  50, built-up 100, water 500, wetland 5. The template is **Observed corridors against current**.
- **A surface larger than the animals' range**. Omniscape cuts its windows at the edge of the map and makes artifacts there;
  and the distribution of a track has to fit in the grid: the surface here is 27.8 by 21.3 km for animals living in about
  11 by 16.

## Build the model

### The surface

```tfgraph
#! caption: The land cover downloaded (node 1), put on the local map of the tracks with cells of 100 m (node 2), its resistance (node 3) and Omniscape's current (node 4).
#! numbered: true
#! highlight: flat, resistance, current
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "data.download"
params = { source = "esa_worldcover", extent = [-74.0, 42.65, -73.74, 42.90], cellsize = 0.0003, resampling = "mode" }

[[nodes]]
id = "flat"
op = "raster.reproject"
inputs = { raster = "cover.raster" }
params = { crs = "+proj=aeqd +lat_0=42.79 +lon_0=-73.88 +datum=WGS84 +units=m", resolution = 100, resampling = "mode" }
note = "The same local map the tracks will be on (centred on [-73.88, 42.79])"

[[nodes]]
id = "resistance"
op = "raster.reclassify"
inputs = { map = "flat.raster" }
params = { rules = "10 -> 1\n20 -> 5\n30 -> 10\n40 -> 20\n50 -> 100\n60 -> 50\n80 -> 500\n90 -> 5\n* -> nan" }
note = "An illustrative resistance for each land cover class"

[[nodes]]
id = "current"
op = "circuit.omniscape"
inputs = { resistance = "resistance.map" }
params = { radius = 30, block_size = 5 }
note = "Omniscape: the current from every cell to those within 3 km"
```

<div class="steps" markdown>

1. **Download the cover** with [[data.download]] (node 1): `source = "esa_worldcover"` gives the land cover of 2020 or 2021
   (`years`), here over an extent several kilometres larger than the tracks on every side, to hold the windows of the
   bridges. `resampling = "mode"` keeps classes whole.

2. **Put it on the tracks' map** with [[raster.reproject]] (node 2): the tracks are on an azimuthal equidistant projection
   centred on a point you give to [[move.aeqd_crs]] (`center`, longitude and latitude), and the surface on the same PROJ text.
   Cells of 100 m with `mode`, because a mean of land cover classes is no class.

3. **Give a resistance to each class** with [[raster.reclassify]] (node 3): a rule a line, `value -> new value`, and `* -> nan`
   makes any other cell no data. **These numbers are yours to defend**: use a resistance from the literature for your species,
   or from a habitat model (Use my data in the application takes your raster in their place).

4. **Compute the current** with [[circuit.omniscape]] (node 4): a window of `radius` cells (30 cells, 3 km) moves across the map
   in blocks of `block_size` cells (5); in each, a unit of current goes from every source cell in the window to the target, and
   the currents add up. Where the movement from all around gets squeezed (a gap between built-up areas, a bridge), the current
   is high; in a large forest it spreads and is low. That is how Omniscape reads *omnidirectional* connectivity.

</div>

### The tracks and their distributions

```tfgraph
#! caption: The cleaned tracks of all the fishers but M5, on the local map of the surface, at one location per 30 minutes, the variance of their bridges, the bridges across gaps of more than 2 hours left out and the utilisation distribution on the grid of the surface.
#! fragment: true
#! highlight: aeqd, thin, masked, ud
[[nodes]]
id = "near"
op = "vector.filter"
inputs = { vector = "unique.tracks" }
params = { expression = 'individual_local_identifier != "M5"' }

[[nodes]]
id = "aeqd"
op = "move.aeqd_crs"
inputs = { tracks = "near.vector" }
params = { center = [-73.88, 42.79] }

[[nodes]]
id = "thin"
op = "move.thin_time"
inputs = { tracks = "aeqd.tracks" }
params = { interval = "30 mins" }

[[nodes]]
id = "ud_var"
op = "move.dbbmm_variance"
inputs = { tracks = "thin.tracks" }
params = { location_error = 20 }

[[nodes]]
id = "ud_masked"
op = "move.mask_segments"
inputs = { tracks = "ud_var.tracks" }
params = { gap = 2 }

[[nodes]]
id = "ud"
op = "move.dbbmm_ud"
inputs = { tracks = "ud_masked.tracks", like = "resistance.map" }
```

The tracks are read and cleaned as in [part 2](../cleaning-and-measures/) (its first four nodes, not repeated) and
projected with `center` given, so that they are on the surface's map. The distributions are those of
[part 6](../home-ranges/): the variance with a location error of 20 m and the default window (31 locations, margin 11),
the bridges across gaps of more than 2 hours left out (254 of the 3,646), and the grid taken from the surface with `like`.
With `like`, the grid is never enlarged: an error says how large the surface has to be.

### The corridors

```tfgraph
#! caption: LaPoint's corridor criterion on the tracks, with its two thresholds given, and the same with none given (the node takes them from the tracks).
#! fragment: true
#! highlight: corridor, thresholds
[[nodes]]
id = "corridor"
op = "move.corridor"
inputs = { tracks = "thin.tracks" }
params = { speed_threshold = 0.05, circvar_threshold = 0.2 }

[[nodes]]
id = "thresholds"
op = "move.corridor"
inputs = { tracks = "thin.tracks" }
```

[[move.corridor]] gives:

- `tracks`: the tracks with, for the segment *leaving* each location (the last location of a track has none): `corridor`
  (`corridor` or `not corridor`), `corridor_speed` (m/s), `corridor_azimuth` (degrees), `corridor_circvar` (empty with fewer than
  two neighbours) and `corridor_n_neighbours`;
- `lines`: the corridor segments as two-point lines, with their track, time and those columns;
- `speed_threshold` and `circvar_threshold`: the thresholds used (numbers), and `thresholds`, a table of them;
- `table`: the numbers of each segment without the geometry.

The settings: `speed_threshold` in m/s, `circvar_threshold` between 0 and 1, and `min_segments` (2 by default). **The
`min_segments` counts every neighbour, the segment itself included**, not only those that qualify as the manual of move2utils says:
the code does it so and so does the node. Longitude and latitude are projected inside to a local map; the speeds are metres per
second whatever the tracks' units.

### Validate

```tfgraph
#! caption: The use of the animals against the current, as the utilisation distribution, as the corridor segments and as the locations, each with the corridors of the surface the best 10 % of its cells.
#! fragment: true
#! highlight: v_ud, v_lines, v_pts
[[nodes]]
id = "v_ud"
op = "move.validate"
inputs = { use = "ud.ud", surface = "current.current" }
params = { kind = "current", quantile = 0.9 }

[[nodes]]
id = "v_lines"
op = "move.validate"
inputs = { use = "corridor.lines", surface = "current.current" }
params = { kind = "current", quantile = 0.9, track = "individual-local-identifier" }

[[nodes]]
id = "v_pts"
op = "move.validate"
inputs = { use = "thin.tracks", surface = "current.current" }
params = { kind = "current", quantile = 0.9 }
```

[[move.validate]] reads the use (`use`: a distribution, locations or lines) and the surface (`surface`). The `kind` of the
surface says how to read it: `current` (a current, a cost-distance kernel, anything higher is better), `resistance` (lower is
better) or `binary` (a least-cost corridor: the cells that are not 0). The corridors of the surface are the cells at or above
the `quantile` of the scores over the whole surface (0.9: the best 10 %), or those past a `threshold` in the surface's own
units. **To compare surfaces give each the same quantile** (the same share of the area), **to compare animals on one surface
the same threshold**. The use is read on the surface's grid (nearest cell when the grids differ); points count 1 in the cell
that holds them, lines their length (cut into pieces of `spacing`, half a cell by default). The outputs are `table` (a row for
each track and one `pooled`), `note` (the pooled row in words), `threshold` (the surface value the corridors start at) and
`corridor` (the raster of the corridors).

### The distance between two distributions

```tfgraph
#! caption: The earth mover's distance between the distributions of six fishers, by Sinkhorn's method and by the exact solver.
#! fragment: true
#! highlight: dist, dist_exact
[[nodes]]
id = "dist"
op = "move.ud_distance"
inputs = { ud = "ud6.ud" }

[[nodes]]
id = "dist_exact"
op = "move.ud_distance"
inputs = { ud = "ud6.ud" }
params = { method = "exact" }
```

[[move.ud_distance]] takes the distributions of several animals (the bands of one raster, as [[move.dbbmm_ud]] gives them: one
grid) and gives a row for each pair of bands in `table`; or two rasters (`ud` and `ud_b`) and a single number, `distance`. Each
distribution is first cut to the cells inside its `mask_quantile` volume contour (0.999 by default) and divided by its sum.
`method` is `sinkhorn` (the default; `reg` the regularisation, 0.05, `max_iter` 500, `tol` 1e-9) or `exact` (Rubner's solver as
R's emdist has it: Euclidean cost, single precision, practical on coarse grids only, as its memory grows with the square of the
cells of both together). On longitude and latitude grids give `gc` for distances in metres on the sphere; `threshold` clips the cost at
a distance (Sinkhorn only). Here the six distributions are on cells of 250 m.

### The values along the steps

```tfgraph
#! caption: The steps of the fishers at 10 minutes, each with five random steps and the elevation along every one of them: mean, ascent, descent and delta.
#! fragment: true
#! highlight: steps, random, along
[[nodes]]
id = "steps"
op = "move.steps"
inputs = { tracks = "bursts.tracks" }

[[nodes]]
id = "random"
op = "move.random_steps"
inputs = { steps = "steps.steps" }
params = { n_control = 5, seed = 1 }

[[nodes]]
id = "along"
op = "move.extract_covariates"
inputs = { tracks = "random.steps", elevation = "dem.raster" }
params = { where = "along", stats = ["mean", "ascent", "descent", "delta"] }
```

The steps are made as in [part 4](../steps-and-summaries/) (resampled at 10 minutes in bursts, nodes not repeated) and the
elevation is the 30 m Copernicus DEM that [[data.download]] gives (`source = "copernicus_dem"`, part 5). With `where = "along"`,
[[move.extract_covariates]] reads every cell the straight line of the step touches (amt's `extract_covariates_along`) and
adds, for each raster input, the columns `<name>_along_<stat>`: `mean`, `min` and `max` of the cells with data, `ascent` and
`descent`, and `delta`; `stats` picks some, and the output `along` is the table of the values themselves. **The cells are
those the line touches, in the order of their number**, not along the step: the node puts them in order along the step for
the ascent and the descent.

### In Python

The surface comes from the model above (it writes `resistance.tif`); the rest is the library:

```python
import numpy as np
import pandas as pd
import terraflux as tf

ID = "individual-local-identifier"
tracks = tf.load_tracks(tf.move.example_data("fishers"))
tracks = tf.move.filter_unique(tf.move.filter_movebank_visible(tracks))
tracks = tracks[~tracks.geometry.is_empty]
near = tracks[tracks[ID] != "M5"].reset_index(drop=True)
near = near.to_crs(tf.move.aeqd_crs(near, center=(-73.88, 42.79)))
p30 = near[tf.move.thin_time(near, "30 mins")].reset_index(drop=True)

resistance = tf.load_raster("resistance.tif")                  # made by the model: cells of 100 m
current = tf.omniscape(resistance, radius=30, block_size=5)["cum_currmap"]
print(resistance.shape, round(float(current.min()), 3), round(float(current.max()), 1))
```

```text
(278, 213) 0.009 70.4
```

```python
v = tf.move.dbbmm_variance(p30, location_error=20)
gap = ((p30.groupby(ID)["timestamp"].shift(-1) - p30["timestamp"]).dt.total_seconds() > 2 * 3600).to_numpy()
ud = tf.move.dbbmm_ud(tf.move.mask_segments(v, gap), like=resistance)
print(ud.shape, int(gap.sum()), "bridges left out")

default = tf.move.corridor(p30)                                # the node's own thresholds, with a warning
print({k: round(float(x), 3) for k, x in default.attrs["corridor"].items() if k != "min_segments"})
cor = tf.move.corridor(p30, speed_threshold=0.05, circvar_threshold=0.2)
lines = tf.move.corridor_lines(cor)
print(len(lines), "corridor segments of", int(cor["corridor_speed"].notna().sum()))
```

```text
(7, 278, 213) 254 bridges left out
{'speed_threshold': 0.157, 'circvar_threshold': 0.376}
42 corridor segments of 3639
```

```python
val = tf.move.validate(ud, current, kind="current", quantile=0.9)
print(val.table.round(3).to_string(index=False))
print(val.note)
print(tf.move.validate(lines, current, kind="current", quantile=0.9, track=ID).table.tail(1).round(3).to_string(index=False))
```

```text
 track  n_cells    rho  use_fraction  area_fraction  use_ratio  surface_used  surface_available  coverage
    F1      558 -0.026         0.314            0.1      3.142        22.654             16.036       1.0
    F2      674  0.008         0.002            0.1      0.020        19.954             16.036       1.0
    F3      169 -0.028         0.000            0.1      0.003        17.261             16.036       1.0
    M1     1986  0.259         0.012            0.1      0.122        19.604             16.036       1.0
    M2     1680  0.144         0.016            0.1      0.162        20.106             16.036       1.0
    M3      679  0.269         0.041            0.1      0.406        17.510             16.036       1.0
    M4      706  0.085         0.092            0.1      0.920        18.640             16.036       1.0
pooled     4702  0.047         0.068            0.1      0.682        19.390             16.036       1.0
pooled: 6.82% of the use lies in the corridors, 10% of the area: 0.682 times its share, about what chance gives. Spearman's rho is 0.0473: no rank relation between the use and the surface.
 track  n_cells   rho  use_fraction  area_fraction  use_ratio  surface_used  surface_available  coverage
pooled    13248 0.046         0.269          0.077      3.484        21.254             17.465       1.0
```

```python
six = p30[p30[ID] != "M4"].reset_index(drop=True)
v6 = tf.move.dbbmm_variance(six, location_error=20)
gap6 = ((six.groupby(ID)["timestamp"].shift(-1) - six["timestamp"]).dt.total_seconds() > 2 * 3600).to_numpy()
ud6 = tf.move.dbbmm_ud(tf.move.mask_segments(v6, gap6), res=250)
sinkhorn = tf.move.ud_distance(ud6)                            # a table of the pairs
exact = tf.move.ud_distance(ud6, method="exact")
both = sinkhorn.merge(exact, on=["a", "b"], suffixes=("_sinkhorn", "_exact"))
both["difference %"] = (both["distance_sinkhorn"] / both["distance_exact"] - 1) * 100
print(both[(both["a"] == "F2") & (both["b"] == "M1")].round(1).to_string(index=False))
print(round(float(both["difference %"].max()), 1), "% at most")
```

```text
 a  b  distance_sinkhorn  distance_exact  difference %
F2 M1             2179.6          1952.4          11.6
11.6 % at most
```

```python
# the values along the steps: the DEM that the model downloaded (dem.tif), the steps at 10 minutes
dem = tf.load_raster("dem.tif")
bursts = tf.move.filter_min_n_burst(tf.move.resample(near, rate="10min", tolerance="1min"), min_n=3)
steps = tf.move.random_steps(tf.move.steps(bursts), n_control=5, seed=1)
along = tf.move.along_summary(steps, {"elevation": dem}, stats=("ascent", "descent", "delta"))
by_case = along.groupby("case_")[["elevation_along_ascent", "elevation_along_descent", "elevation_along_delta"]].mean()
print(by_case.round(2))
```

```text
       elevation_along_ascent  elevation_along_descent  elevation_along_delta
case_                                                                        
False                    3.76                     4.70                   4.01
True                     4.32                     4.29                   3.38
```

`tf.move.extract_covariates_along(steps, covariates)` gives the list of the values of each step (as amt does); `along_summary` the columns.
The functions take `n_threads`; the results do not depend on it.

## Read the results

### The surface and the corridors

![The resistance with the corridor segments, and the current with the animals' locations](/img/tutorials/animal-movement/corridors-and-validation/surface.png "Left: the resistance of the land cover (logarithmic, darker is higher: the river running from the west is water, the greys are crops, built-up land and bare ground), the 30-minute segments of the seven fishers in grey and the 42 corridor segments in orange. Right: Omniscape's current (logarithmic) with the locations of the animals in white. Coordinates in kilometres from the centre of the projection.")

The corridor segments are few and short: **42 of the 3,639 segments (1.2 %)** of the 30-minute tracks, with a speed of at least
0.05 m/s (180 m per hour, a third of the node's own upper quartile) and directions that agree (a circular variance of at
most 0.2). They are short and scattered along the diagonal strip where most of the animals live, with a group of longer
ones 1 to 2 km east and 7 to 8 km south of the centre. With the thresholds the node
would take from these tracks (0.157 m/s and 0.376) there are only 17.

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/corridor-sensitivity.csv
digits: 2
caption: The corridor segments of the seven fishers at 30 minutes (3,639 segments) for the thresholds of this page, the node's own and each setting changed in turn.
```

The count is very sensitive to the thresholds: from 1 segment (`min_segments` 10) to 108 (a circular variance of 0.4), a hundredfold. A *corridor* here is a **relative** property: it says which of the segments of these
tracks are faster and straighter than the rest, with these thresholds, and the report has to give them.

### Does the current match the use?

![The best 10 % of the current and the corridor segments of the animals](/img/tutorials/animal-movement/corridors-and-validation/corridors.png "The corridors of the surface (the best 10 % of the cells of Omniscape's current, green) and the 42 corridor segments of the fishers (orange). Most of the length of the orange segments is outside the green patches, but 27 % of it is inside them, against 7.7 % of the area (the table below).")

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/validation.csv
digits: 3
caption: The use of the seven fishers (pooled) against three surfaces: Omniscape's current, the resistance itself (lower is better) and, as a control, the current of a landscape of one uniform resistance. The use ratio is the share of the use inside the best 10 % of the cells over the share of the area.
```

![The use ratio of each use and surface](/img/tutorials/animal-movement/corridors-and-validation/use-ratio.png "Used against available: the share of the use inside the best 10 % of the cells of each surface over the share of the compared area. Above the dashed line the surface's corridors catch more of the use than their area; below, less.")

This is the finding of the part, and it has two halves.

- **The corridor segments agree with the current**: 26.9 % of their length lies inside the best 10 % of the cells of the current,
  against 7.7 % of the area they could have been in: a use ratio of 3.5. When the fishers *run* in a straight line, they do
  it where the current is high.
- **The use does not**: the utilisation distribution has 6.8 % of its probability inside the best 10 % of the cells, against
  10 %, a ratio of 0.68, and a rank correlation of 0.05; the note of the node reads it as "about what chance gives" and "no
  rank relation". The locations alone are close to chance too (1.15, rho 0.09). The fishers' *time* is blind to the current.

This is not a failure of the animals or of the operator, and a plausible reason is what the current measures. Omniscape's current is
high where the movement from all around is squeezed (between built-up areas, across a bridge), and low inside a large block of
forest, where it spreads; a fisher *lives* in the forest and *crosses* the gaps. The home range of a resident is the use that a
current for the movement between habitats is not about, which is why Zeller et al. validated with dispersing animals (this
reading is a hypothesis: the table does not test it). **Choose the use that matches what the surface is for**: a surface of movement between habitats against the animals'
movement, a habitat suitability against their use of the habitat.

The **controls** do their job. The current of a landscape with one resistance everywhere, a map of the geometry of the window and
the edge, has no relation to the use (rho −0.016, no use at all in its best 10 %). The resistance itself, with the animals using
the lowest-resistance cells more (rho 0.17), shows the problem of a surface with few distinct values: its "best 10 %" is the
70 % of the map that is tree cover, all tied at a resistance of 1, so the ratio (1.3) says only that fishers prefer trees.

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/validation-animals.csv
digits: 3
caption: The use of each fisher against the current: the cells compared, Spearman's rho, the share of the use inside the best 10 % of the cells and the use ratio. coverage is the part of the distribution that fell on cells with a current (all of it).
```

Each animal gives a different answer: F1 uses the best cells 3.1 times more than chance, F2 and F3 almost never, the males M1, M2 and M3 an
eighth to two-fifths of chance and M4 about chance (0.92). F3's 169 cells and F1's 558 are few. **Report the table, not the pooled row alone**, and the number of
cells.

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/validation-quantile.csv
digits: 2
caption: The use of the seven fishers (pooled) inside the best 50, 25, 10 and 5 % of the cells of the current: the corridors of the surface at four sizes.
```

And the answer depends on what a corridor of the surface is: 1.34 for the best half of the cells, 0.85 for the best quarter, 0.68
for the best tenth, 1.07 for the best twentieth. A statistic that moves above and below 1 as the cut moves is a sign that the
surface says little about the use at all. Give each surface you compare the same cut.

### How far apart are two ranges?

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/ud-distance-matrix.csv
digits: 2
heat: true
caption: The earth mover's distance between the dynamic Brownian bridge distributions of six fishers, by Sinkhorn's method (km): the cost of moving the use of one animal onto the other's.
```

The closest pairs are F3 and M3 (0.9 km) and F2 and M1 (2.2 km): the two that overlapped most in part 6. They are followed by F1 and M2 (3.6 km, the third overlapping
pair) and then the pairs that share little or nothing, 4 to 8 km apart; the overlap indices of part 6 cannot tell a pair 4 km apart from one
8 km apart.

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/ud-distance-pairs.csv
digits: 2
caption: The 15 pairs by Sinkhorn's method (reg 0.05) and by the exact solver, in metres, and the difference.
```

![Sinkhorn's distance against the exact one](/img/tutorials/animal-movement/corridors-and-validation/emd.png "The 15 pairs of the table: Sinkhorn's distance (reg = 0.05) against the exact earth mover's distance. The dashed line is equality.")

Sinkhorn is within 5 % of the exact distance for 13 of the 15 pairs (0.15 to 4.6 %), and not for the two nearest: 11.6 % for F2 and M1, 10.8 % for F3
and M3. The entropy term spreads the transport a little, and an animal compared with its own neighbour gains a few hundred metres
that the exact solution does not have; **the bias is in metres, so it is large for small distances.** The regularisation
controls it:

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/ud-distance-reg.csv
digits: 2
caption: The distance of F2 and M1 by Sinkhorn's method for the regularisation reg from 0.5 to 0.01, and by the exact solver: the distance, the time of the computation and its difference to the exact one.
```

From `reg` 0.5 to 0.01 the bias goes from 34 % to 1.9 %, and the time from 0.01 to 0.04 s (the exact one: 0.06 s on this
grid of 89 by 85 cells). Below 0.01 the matrix of the algorithm underflows. On a grid as coarse as this one the exact solver is
affordable (15 pairs in 0.9 s against 0.08 s for Sinkhorn); on a fine grid, its memory grows with the square of the cells and
only Sinkhorn is possible. **Compare distances only with the same settings**, as the table's differences are of the order of
the effect you may be measuring.

### Does a fisher follow the contour lines?

![One day of M4's steps over the relief, coloured by the ascent](/img/tutorials/animal-movement/corridors-and-validation/relief-night.png "M4's 36 steps at 10 minutes from noon on 5 March 2010 to noon on the 6th (local time), coloured by the ascent along each step, over the contour lines of the 30 m Copernicus DEM (every 5 m). Steps that follow a contour are dark; yellow ones climb 8 m or more.")

A step over the contour lines, the long yellow ones in the west, climbs, and the short dark ones in the middle keep to a
level. That is the picture of a step; the question of a *selection* is whether the animal chooses steps that climb less than
the steps it could have taken instead. For that, each observed step has five random steps from the same start, with the same
distribution of lengths:

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/relief.csv
digits: 2
caption: The steps of the fishers at 10 minutes (M1, sampled every 15 minutes, has none): the mean ascent along the observed steps and along five random steps from each start, in metres and per 100 m of step, and the share of the steps that climb less than 2 m.
```

![The ascent per 100 m of step, observed and random](/img/tutorials/animal-movement/corridors-and-validation/relief-compare.png "The distribution of the ascent per 100 m of step for the 4,139 observed steps and for their 20,695 random steps (the last class is 20 m and more).")

```tftable
data: /data/tutorials/animal-movement/corridors-and-validation/relief-classes.csv
digits: 1
caption: The steps by what they do on the relief: flat or along a contour (an ascent under 2 m), climbing or descending (an ascent of 2 m or more whose delta is more than 0.8 of it), and up and down (the rest). Our classes, not the library's.
```

On this relief and at these scales, **nothing suggests that the fishers follow contour lines**. The mean ascent of the observed steps
(4.3 m) is, if anything, 15 % *higher* than that of the random steps (3.8 m); per 100 m of step it is 4.4 against 4.0 m; and 54.5 % of the observed steps
are flat or along a contour against 55.5 % of the random ones. For F2, F3, M3 and M4 (steps of 102 to 172 m) fewer of the observed steps are flat than of the random ones, by 7 to 12 percentage
points; for F1 and M2, whose steps are shorter (72 and 88 m), 3 to 4 points more. The difference between observed and random is small, and the
step selection function of the next block is the tool to say whether it is real: what this part shows is how to get the
numbers it needs. Two cautions. The DEM is a surface model of 30 m cells, with the noise of the canopy: its ascent along a step on
flat ground is not 0, which is why "flat" is under 2 m. And the cells a line touches count as much as the cells under it:
a 100 m step crosses about five cells, and the ascent is the sum of the rises *between them*.

## Settings that change the answer

- **The thresholds of the corridors** (the table above).
- **The resistance and the window** of the surface: the answer to the validation is a property of the surface, and the
  resistances here are illustrative. A different radius gives a different current.
- **The cut of the surface's corridors** (`quantile` or `threshold`): the second table of the validation.
- **The gaps left out** of the distributions, and the **location error**: the distributions are those of part 6, with all the
  choices of part 6.
- **`reg`** of Sinkhorn, and the **mask** (`mask_quantile`) of the distributions.
- **The stats and the DEM's cells** of the values along the steps.

## Common pitfalls

- **Validating a surface with the data that made it.** A resistance calibrated on the locations of the animals flatters
  itself. Validate with other animals, other years, or dispersal paths.
- **The wrong use for the question.** The home range of a resident against a surface of movement between habitats (above).
- **Corridor thresholds from the tracks themselves.** The node's own quantiles make a quarter of any set of segments fast: give
  thresholds, the same for every animal you compare.
- **Mixed sampling intervals.** The corridor's neighbourhood is half a step: a track at 2 minutes and one at 15 meet each
  other's midpoints at different scales. Thin to one interval ([[move.thin_time]], part 8).
- **Edges.** Omniscape cuts its windows at the map's edge, and the distribution of a bridge needs room: make the surface larger
  than the range of the animals (the current at the edge of the map above is an artifact).
- **A surface of few values.** The best 10 % of a map of eight resistance classes is whole classes, not 10 %: the ratio is
  against the real share of the area, which `area_fraction` tells.
- **No p-value.** The cells of a distribution are not independent; do not compute one from `rho`.
- **A mean for categories.** A land cover reprojected with a bilinear or cubic resampler has classes that do not exist; use `mode`.
- **Sinkhorn's bias on near distributions.** See above.

## What the operators and the library agree on

The operators and the library (`tf.move`) were run on the same tracks, and the projected locations of the Rust projection
and of pyproj's differ by 0.0 m (they coincide to the last bit on these 3,646 locations). The corridor segments (42 each), the
validation of the lines (to 6e-13), the validation of the distribution against the same file as a surface (to 1e-16), both
earth mover's distances (to 0.0) and the values along 4,139 steps (the mean ascent, to the last digit) agree. The validation of
the distribution differed by 5e-7 when the library read the current from the GeoTIFF the operator wrote, in single precision, and
the operator used it in memory in double: the ranks of the nearly tied cells of the correlation moved. Against the R packages:

- **move2utils** (0.4.6; Kranstauber et al., 2026): `mt_corridor` and `emd` bit for bit where the arithmetic is R's: the
  corridor needs the buffer of GEOS 3.12 (a polygon of 120 vertices, not a circle) and the test of a point on its boundary,
  both ported; Sinkhorn is bit for bit with R's reference BLAS and the exact solver is `emdist`'s (0.3-3, MIT licence).
  From longitude and latitude, the azimuth and the circular variance differ from R's by about 1e-7 (the PROJ of the library
  and of R), and the corridors found are the same. The speed uses the exact interval in nanoseconds, as `move.speed` does, and
  not the conversion of the minutes of the time difference of R's: 1 unit in the last place. Two mistakes of `mt_corridor` are
  corrected: a search radius in metres on a track in feet, and a projection moved to the datum EPSG:4236.
- **amt** (0.3.1; Signer et al., 2019): `extract_covariates_along`, bit for bit with amt and terra 1.9-50 (the clipped grid
  and GDAL 3.9.3's `ALL_TOUCHED` line walk, ported). One caveat: R's text of 15 digits (`paste0`) differs from the `%.15g` of
  C in about 1 % of the random numbers of 15 to 16 digits (1e-14 relative), which only changes the cells of a coordinate that falls
  exactly on the edge of a cell. `along_summary` (ascent, descent, delta) is not in amt.
- **`move.validate`** is not in R. **The definitions are ours**, made from what Zeller et al. (2018) measure; only their abstract
  was available.

## The template

**Observed corridors against current** (library subject **Animal movement**, group *Movement*) runs this part's models on two simulated
tracks over a simulated landscape: the resistance from a neutral landscape model, Omniscape's current, the dynamic Brownian
bridge of each animal, LaPoint's corridor segments, and [[move.validate]] of the distributions and of the lines, with a chart of
the rank correlations. The resistance is marked as the data of **Use my data**: replace it with your own, and the simulation with your tracks.

## Going further

- [Home ranges and utilisation distributions](../home-ranges/): the distributions that this part validates.
- [Clean a track of GPS errors and thin it](../cleaning-and-thinning/): the interval the corridor criterion supposes.
- [The data of step selection functions](../step-selection/): the steps and the covariates whose values along the step this part adds.
- [Circuit connectivity](../../circuit-connectivity/): resistance surfaces and Omniscape.
- The operators' reference: [[move.corridor]], [[move.validate]], [[move.ud_distance]], [[move.extract_covariates]],
  [[circuit.omniscape]].

## References

- Cuturi, M. (2013). Sinkhorn distances: lightspeed computation of optimal transport. *Advances in Neural Information
  Processing Systems*, 26, 2292-2300. https://arxiv.org/abs/1306.0895
- Kranstauber, B., Safi, K., & Scharf, A. K. (2026). move2utils: a utility toolkit for the move2 ecosystem. *bioRxiv*.
  https://doi.org/10.64898/2026.07.07.736908
- LaPoint, S., Gallery, P., Wikelski, M., & Kays, R. (2013). Animal behavior, cost-based corridor models, and real corridors.
  *Landscape Ecology*, 28(8), 1615-1630. https://doi.org/10.1007/s10980-013-9910-0
- Rubner, Y., Tomasi, C., & Guibas, L. J. (2000). The earth mover's distance as a metric for image retrieval. *International
  Journal of Computer Vision*, 40(2), 99-121. https://doi.org/10.1023/A:1026543900054
- Signer, J., Fieberg, J., & Avgar, T. (2019). Animal movement tools (amt): R package for managing tracking data and
  conducting habitat selection analyses. *Ecology and Evolution*, 9(2), 880-890. https://doi.org/10.1002/ece3.4823
- Zeller, K. A., Jennings, M. K., Vickers, T. W., Ernest, H. B., Cushman, S. A., & Boyce, W. M. (2018). Are all data types and
  connectivity models created equal? Validating common connectivity approaches with dispersal data. *Diversity and
  Distributions*, 24(7), 868-879. https://doi.org/10.1111/ddi.12742
