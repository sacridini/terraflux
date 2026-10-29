---
title: Projections, interpolation and simulated tracks
short: Projections and interpolation
description: Put tracks on a local map in metres with an azimuthal equidistant projection, place locations at regular times between the recorded ones, make tracks from a table, and simulate Brownian tracks to compare real movement with random motion.
order: 30
level: intermediate
time: 40 min
ops: [move.aeqd_crs, move.interpolate, move.as_tracks, move.simulate_brownian, vector.reproject, raster.reproject]
series: Animal movement
part: 3
---

Tracks come in longitude and latitude, at the times a collar happened to obtain a fix. Many analyses need
something else: coordinates in metres on a flat map, locations at the same times for every animal, or a track
of random movement to compare the real one with. In this part you will project the fishers' tracks from
[part 2](../cleaning-and-measures/) onto a map in metres centred on them, place a location every 10 minutes
between the recorded ones, turn a table of coordinates into tracks, and simulate Brownian motion, the simplest
model of random movement. At the end you will know when a projection matters and which one to use, what an
interpolated location is (and is not), and how a real animal's turns differ from chance.

## The idea

### Degrees are not distances

Longitude and latitude are angles. A degree of latitude is always about 111 km, but a degree of longitude
shrinks towards the poles as the cosine of the latitude: 111 km at the equator, 82 km at Albany (42.8° N), 56 km
at 60° N. A method that treats the two as the same unit on a flat plane stretches every east-west distance.

![The error of distances computed in flat degrees, by the direction of the step](/img/tutorials/animal-movement/projections-and-interpolation/flat-degrees.png "The error of every step's length of the fishers, against its direction, when longitude and latitude are taken as flat coordinates and multiplied by 111.32 km (orange), and when the tracks are projected with the azimuthal equidistant projection (green), both compared with move.distance on the sphere. Flat degrees are right for steps to the north or south and 36% too long for steps to the east or west.")

The figure shows it on the fishers: north-south steps come out right, east-west steps 36 % too long. Areas fare
worse (a cell of 0.01° × 0.01° is a rectangle, not a square), and so does anything drawn in "circles" of
degrees: buffers, kernels, random steps.

The measures of part 2 avoid the problem: they are computed on the Earth. But several methods need a flat plane
in metres: the random steps of [part 5](../step-selection/) are drawn as lengths and angles on a plane, kernel
densities and home ranges are areas on a plane, and many rasters of the landscape come in a projected system.
For those, the tracks must be **projected**.

!!! science "Key idea: the azimuthal equidistant projection"
    A projection maps the curved surface onto a plane, and no projection keeps every distance, angle and area.
    The **azimuthal equidistant** projection (Snyder, 1987) keeps two things exactly: the distance and the
    direction from its **centre** to every other point. Centred on the tracks, it gives a map in metres
    where the distortion grows with the distance from the centre, slowly: under 0.01 % within 150 km of it,
    0.1 % at about 500 km. move2's `mt_aeqd_crs` builds this projection centred on a set of tracks, and so does
    [[move.aeqd_crs]]:

    - the centre is the **centroid** of the locations (their mean on the sphere), the **center** of their
      range of longitudes and latitudes, or a point you give;
    - the projection is written as a PROJ string, `+proj=aeqd +lat_0=... +lon_0=... +units=m`, which every
      node with a `crs` setting reads.

    It is a *local* projection: right for an animal's home range or a region of a few hundred kilometres, wrong
    for a migration across a continent, where no single plane works and the measures on the Earth (part 2)
    are the safe choice.

### Locations at chosen times

Collars fix at irregular times (part 1). Some questions need the locations at chosen ones: where two animals
were at the same moment (to measure their distance, or a contact), where an animal was every hour (to compare
it with hourly weather), or where it was when a record of another sensor has a time but no location.
**Interpolation** answers with an estimate: the animal is assumed to have moved at a constant speed along the
shortest path between the recorded locations before and after the time.

!!! science "Key idea: linear interpolation in time"
    For a time \( t \) between two recorded times \( t_i < t < t_{i+1} \), the location is placed on the line
    from \( p_i \) to \( p_{i+1} \) at the fraction of the time elapsed,

    \[ f = \frac{t - t_i}{t_{i+1} - t_i}, \qquad p(t) = p_i + f\,(p_{i+1} - p_i). \]

    In longitude and latitude, "the line" is the great circle between the two locations, as move2 places it
    (with the s2 library). A time outside the span of a track gets no location; so does a time in a gap longer
    than a threshold, where the straight line is a poor guess (`max_time_lag`).

An interpolated location is not an observation. It is right only if the animal did go straight, at a constant
speed, between its two fixes; the longer the gap, the less likely that is. Use interpolated locations to align
times, never as data in their own right: not as used locations in a habitat selection analysis, not to "fill"
a track before measuring how tortuous it is.

### Random motion as a reference

How do we know that an animal moves *non-randomly*, that its steps keep a direction or that it returns to the
same places? By comparison with a model of random movement. The simplest is **Brownian motion**: at each step
the animal moves along x and along y by independent normal draws, of variance proportional to the time step,
\( \Delta x, \Delta y \sim N(0, \sigma^2 \Delta t) \). Its directions are uniform and independent of the step
before: it has no memory and no preferred heading. Its mean squared displacement from the start grows linearly
with time, \( E[r^2] = 2\sigma^2 t \), so it wanders away as the square root of time. Codling et al. (2008)
review this and the more realistic random walks (correlated, biased, with stops) used in movement ecology.

## When to use it

- **A projection** for anything drawn or computed on a plane: random steps (part 5), buffers around locations,
  kernel densities, the distances of [[vector.distance]], rasters in metres. Use [[move.aeqd_crs]] when the
  tracks cover up to a few hundred kilometres and no projection of the region is already in use. When your
  rasters are in a national or UTM system, project the tracks to *that* system instead ([[vector.reproject]]),
  so that the two match without resampling the rasters.
- **Interpolation** to align several animals in time, to place the records of other sensors, or to compare with
  a time series at regular times. For a regular interval of *observed* locations, resample instead
  ([[move.resample]], [part 4](../steps-and-summaries/)): it keeps only real fixes.
- **Brownian tracks** as a null model, to test a summary of the real tracks against random motion, and to learn
  how a method behaves on data whose process you know.
- **Tracks from a table** ([[move.as_tracks]]) whenever the data do not come from Movebank: a spreadsheet of
  VHF locations, a simulation's output, camera-trap detections of individuals.

## What you need

The cleaned tracks of part 2 (nodes 1 to 4 there). For a table, a CSV with a column of x or longitude, one of y
or latitude, one of time and one of the animal, and its coordinate system.

## Build the model

### A local projection in metres

```tfgraph
#! caption: The cleaned tracks (node 4) on a local azimuthal equidistant projection centred on them (node 5), their step lengths in metres (node 6), and a raster taken to the same projection (nodes 7 and 8).
#! numbered: true
#! highlight: aeqd, dem_local
format = "terraflux/1"

[[nodes]]
id = "read"
op = "move.read_tracks"
params = { path = "fishers.csv.gz" }

[[nodes]]
id = "located"
op = "vector.filter"
inputs = { vector = "read.tracks" }
params = { expression = "not isnull(geometry.x)" }

[[nodes]]
id = "visible"
op = "move.filter_movebank_visible"
inputs = { tracks = "located.vector" }

[[nodes]]
id = "unique"
op = "move.filter_unique"
inputs = { tracks = "visible.tracks" }

[[nodes]]
id = "aeqd"
op = "move.aeqd_crs"
inputs = { tracks = "unique.tracks" }
params = { center = "centroid", units = "m" }

[[nodes]]
id = "dist"
op = "move.distance"
inputs = { tracks = "aeqd.tracks" }

[[nodes]]
id = "dem"
op = "data.download"
params = { source = "copernicus_dem", dem_resolution = 30, extent = [-74.0, 42.65, -73.33, 42.9], cellsize = 0.0003 }

[[nodes]]
id = "dem_local"
op = "raster.reproject"
inputs = { raster = "dem.raster" }
params = { crs = { from = "aeqd.crs" }, resolution = 30 }
```

<div class="steps" markdown>

1. **Project the tracks.** Add [[move.aeqd_crs]] (library subject **Animal movement**) and connect
   `unique.tracks` to it. It gives three outputs: `crs`, the projection as a PROJ string; `tracks`, the tracks
   in it; and their `table`. For the fishers the centroid is at 42.807° N, 73.694° W, and the projection is
   `+proj=aeqd +lat_0=42.807093 +lon_0=-73.693649 +x_0=0 +y_0=0 +ellps=WGS84 +datum=WGS84 +units=m +no_defs`.
   `center` takes `centroid` (the default), `center` (the middle of the ranges of longitude and latitude) or a
   point `[longitude, latitude]`; `units` is `m` or `km`.

2. **Measure in metres.** Every `move` node works on projected tracks too: [[move.distance]] then gives straight
   lines in the projection's units, [[move.azimuth]] angles on the grid.

3. **Bring the rasters to the same projection.** The `crs` output is a text, and a setting can read a text from
   another node: set the `crs` of [[raster.reproject]] to `{ from = "aeqd.crs" }`. The elevation is then on a grid of 30 m cells in the same metres as the
   tracks. The same works for [[vector.reproject]] and any other layer.

</div>

The projection is computed from the tracks, so it follows them: run the same model on other animals and it is
centred on those.

### Locations every 10 minutes

```tfgraph
#! caption: M4's track (node 5) with a location every 10 minutes (node 6), gaps longer than an hour left empty, and only the records at the 10-minute times kept.
#! fragment: true
#! highlight: m4, every10
[[nodes]]
id = "unique"
op = "move.filter_unique"
inputs = { tracks = "visible.tracks" }

[[nodes]]
id = "m4"
op = "vector.filter"
inputs = { vector = "unique.tracks" }
params = { expression = "individual_local_identifier == \"M4\"" }

[[nodes]]
id = "every10"
op = "move.interpolate"
inputs = { tracks = "m4.vector" }
params = { times = "10 mins", max_time_lag = 3600, omit = true }
```

<div class="steps" markdown>

1. **Choose an animal** with [[vector.filter]]. Movebank's column `individual-local-identifier` has hyphens, which
   a script cannot write in a name; a script reads it as `individual_local_identifier` (every character other
   than a letter, a digit or `_` becomes `_`). The expression keeps M4's locations.

2. **Interpolate** with [[move.interpolate]]. `times` is either a unit (`10 mins`, `1 hour`, `day`...) for a
   regular sequence from the first time of each track to its last, or a list of times
   (`["2010-03-06 00:00:00", "2010-03-06 06:00:00"]`). Each time the track does not have is added as a record and
   located between the recorded locations before and after it. `max_time_lag` (in seconds) leaves empty the
   times inside a gap longer than it: here, gaps over an hour. `omit = true` keeps only the records at the chosen
   times, dropping the recorded ones in between; with `false` (the default) the track has both.

</div>

[[move.interpolate]] without `times` does something else: it gives a location to every record that has a time
but no location (an empty point), between the locations before and after it. This is how move2 places the
records of other sensors (an accelerometer, a thermometer) on the track: keep them in the tracks of part 1
instead of filtering them out, and interpolate.

### Tracks from a table

```tfgraph
#! caption: A table of locations (node 1) made into tracks (node 2): x and y in a UTM zone, the time in seconds and the animal by a number.
#! numbered: true
#! highlight: tracks
format = "terraflux/1"

[[nodes]]
id = "table"
op = "table.read"
params = { path = "locations.csv" }

[[nodes]]
id = "tracks"
op = "move.as_tracks"
inputs = { table = "table.table" }
params = { x = "easting", y = "northing", crs = "EPSG:32618", time = "seconds", track = "animal" }
```

[[move.as_tracks]] takes a table (from [[table.read]]) or a layer of points (from [[vector.read]]) and makes
tracks of it: `x` and `y` are the table's columns of coordinates, `crs` their coordinate system (EPSG:32618 is
UTM zone 18 north, which covers eastern New York), `time` and `track` the columns of the time and the animal
when they do not have the usual names. The tracks are sorted by animal and time (`sort = true`), as every
`move` node needs. A table from [[table.read]] holds numbers only, so give it numeric times (seconds since a
start, for instance) and numeric animal codes; for times as text and names of animals, read the file as points
with [[vector.read]] (which keeps text columns) or keep it in Movebank's format for [[move.read_tracks]].

### Brownian tracks

```tfgraph
#! caption: Four Brownian tracks of 1000 steps from (0, 0) (node 1), with their turn angles (node 2).
#! numbered: true
#! highlight: brown
format = "terraflux/1"

[[nodes]]
id = "brown"
op = "move.simulate_brownian"
params = { t = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10], sigma = 1, tracks = 4, seed = 1 }

[[nodes]]
id = "turns"
op = "move.turn_angle"
inputs = { tracks = "brown.tracks" }
params = { units = "deg" }
```

[[move.simulate_brownian]] draws tracks of Brownian motion as move2's `mt_sim_brownian_motion` does, with R's
random numbers: the same `seed` gives the same tracks in Terraflux and in R. `t` is the list of times (the
figures below use 0 to 1000), the same for every track or a list per track; `sigma` the diffusion, one number
or one per step; `tracks` how many (named 1, 2...) or their names; `start` the first location. The tracks have
no coordinate system: x and y are in the units of `sigma`.

## Read the results

### The tracks in metres

![The fishers' tracks on the azimuthal equidistant projection centred on them](/img/tutorials/animal-movement/projections-and-interpolation/aeqd.png "The tracks on the azimuthal equidistant projection centred on their centroid (the cross), in kilometres east and north of it. The centre falls between the animals west and east of the Hudson, about 24 km from the farthest locations.")

The projected tracks look like the ones of part 1, which is the point: at this scale a good projection changes
nothing the eye can see, but its coordinates are metres. The centroid falls in the middle, between the seven
animals in the west and M5 in the east, in a place no fisher went: the centre is a property of the projection,
not of the animals.

How exact are distances in it? The green points of the first figure compare each step's length in the
projection with [[move.distance]] in longitude and latitude: they differ by 0.09 % in the median and 0.27 % at
most, under 3 m in a kilometre. And this small difference is not the projection's: [[move.distance]] measures
on a sphere of radius 6,371 km (as move2 does), the projection on the WGS 84 ellipsoid, whose radius of
curvature at 42.8° N is 6,365 km from north to south and 6,388 km from east to west. East-west steps are 0.27 %
longer on the ellipsoid, north-south steps 0.1 % shorter. For tracks of a region, the two are equally good;
flat degrees are not.

### Filling a gap

![M4's locations every 10 minutes, with and without the long gaps](/img/tutorials/animal-movement/projections-and-interpolation/interpolate.png "M4 on the night of 5 to 6 March 2010: the recorded locations (grey) and the locations every 10 minutes (orange). Left: every gap is filled along a straight line. Right: with max_time_lag = 3600 s, the 73-minute gap in the middle is left empty.")

The night of M4 from part 1 had a gap of 73 minutes, when the fisher crossed 500 m between two resting places.
Interpolated every 10 minutes (left), the gap is filled by seven locations on the straight line, evenly spaced:
the assumption of constant speed made visible. Nothing says the fisher went that way. With `max_time_lag = 3600`
(right), the seven times keep their records but no location, and the track honestly shows a gap.

Over M4's whole track, the 10-minute sequence has 7,203 times, and 3,367 of them fall in gaps of more than an
hour: nearly half. That is the cost of a regular sequence over an irregular track, and the reason to keep
`max_time_lag` set: without it, half of the "track" would be straight lines.

### Brownian motion against a fisher

![Four Brownian tracks](/img/tutorials/animal-movement/projections-and-interpolation/brownian.png "Four Brownian tracks of 1000 steps with sigma = 1, from the cross; the dots are where they ended.")

![Turn angles of Brownian motion and of the fishers](/img/tutorials/animal-movement/projections-and-interpolation/brownian-turns.png "The turn angles of the Brownian tracks (left) and of the five fishers fixed every 2 minutes, thinned to one location per 10 minutes (right). Brownian turns are uniform; the fishers' are not.")

The Brownian tracks wander with no direction: some stay near the start, one strays 50 units away and comes
back part of the way, as a process whose displacement grows only as the square root of time. Their turn angles are uniform, every direction as likely as
any other. The fishers at 10 minutes are different: more steps go straight on (near 0°) and more go straight
back (near 180°) than chance would give, and fewer turn sideways. Straight-on is **directional persistence**,
the animal heading somewhere; straight back is the animal returning, or the location error around a resting
place (part 2). A test of the difference (Rayleigh's test, or a comparison of the distribution of turn angles)
is a test against this null model.

## Settings that change the answer

- **max_time_lag** of [[move.interpolate]]: the longest gap the straight line may bridge. Shorter is safer;
  choose it from the histogram of time lags (part 2), a few times the scheduled interval.
- **omit**: with `true`, the result has only the chosen times, a regular track; with `false`, the chosen times
  are added to the recorded ones, which is what you want to locate other records without losing fixes.
- **center** of [[move.aeqd_crs]]: the centroid or the center of the range give almost the same projection for
  compact tracks; for a migration with a long tail, the center of the range keeps both ends at similar
  distances. A fixed point (`[-73.75, 42.8]`) gives every model run the same projection, whatever the animals.
- **sigma** and **t** of [[move.simulate_brownian]]: the spread grows as \( \sigma \sqrt{t} \); give it the
  `sigma` of the real animals (the standard deviation of their displacements per unit of time) to compare at the
  same scale.

## Common pitfalls

- **Distances in flat degrees.** Never compute lengths, areas or buffers on longitude and latitude as if they
  were metres. Either measure on the Earth (part 2) or project.
- **A local projection used far from its centre.** An azimuthal equidistant projection is excellent within a
  few hundred kilometres of its centre and increasingly distorted beyond. For tracks across a continent, use the
  measures on the Earth.
- **Projected tracks and unprojected rasters.** A raster in longitude and latitude and tracks in metres do not
  overlay. Reproject one to the other ([[raster.reproject]] with the `crs` of [[move.aeqd_crs]], or
  [[vector.reproject]] of the tracks to the raster's system). [[move.extract_covariates]] (part 5) takes the
  points to each raster's system by itself.
- **Interpolated locations used as data.** They are estimates on a straight line. Keep them out of habitat
  selection and out of measures of tortuosity, and keep `max_time_lag` set.
- **Expressions on Movebank's columns.** Write `individual_local_identifier`, not `individual-local-identifier`
  (which a script reads as a subtraction).

## Going further

- [Regular steps and track summaries](../steps-and-summaries/): resampling the observed locations to a regular
  interval, without inventing any.
- [The data of step selection functions](../step-selection/): random steps on the projected tracks.
- The operators' reference: [[move.aeqd_crs]], [[move.interpolate]], [[move.as_tracks]],
  [[move.simulate_brownian]], [[vector.reproject]], [[raster.reproject]].

## References

- Codling, E. A., Plank, M. J., & Benhamou, S. (2008). Random walk models in biology. *Journal of the Royal
  Society Interface*, 5(25), 813-834. https://doi.org/10.1098/rsif.2008.0014
- Kranstauber, B., Safi, K., & Scharf, A. K. (2024). move2: R package for processing movement data. *Methods in
  Ecology and Evolution*, 15(9), 1561-1567. https://doi.org/10.1111/2041-210X.14383
- Snyder, J. P. (1987). *Map projections: a working manual* (U.S. Geological Survey Professional Paper 1395).
  U.S. Government Printing Office. https://doi.org/10.3133/pp1395
