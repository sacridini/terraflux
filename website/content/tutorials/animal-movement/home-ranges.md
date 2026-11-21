---
title: Home ranges and utilisation distributions
short: Home ranges
description: Estimate the area an animal uses with a minimum convex polygon, a kernel density estimate, local convex hulls and the dynamic Brownian bridge, choose a bandwidth and a window, draw the 50 % and 95 % contours, compare animals by their overlap, and see what the estimator and the sampling interval do to the answer.
order: 60
level: advanced
time: 75 min
ops: [move.home_range, move.home_range_overlap, move.intensity_use, move.suggest_window, move.dbbmm_variance, move.dbbmm_ud, move.mask_segments, move.dbgb_variance, move.dbgb_ud, move.ud_volume, move.ud_outer_probability]
series: Animal movement
part: 6
template: Home range of a track
---

Where does an animal live? The first answer a tracking study gives is a map and a number: the area the animal used
over the weeks it wore its collar. In this part you will estimate that area for the fishers of
[part 1](../ideas/) in several ways, from the oldest (the outline of the locations) to the newest (a model of the animal
moving between its fixes), and you will see that they give areas that differ by a factor of two to four from the same
locations. At the end you will have the home range and the core area of a fisher as polygons, the whole
*utilisation distribution* behind them (the probability that the animal was in each cell), the overlap of the animals
that shared ground, and a clear idea of which estimator to choose, which settings to report, and what the sampling
interval does to all of them.

## The idea

### A home range is a probability map

Burt (1943) defined an animal's home range as the area it traverses in its normal activities of food gathering,
mating and caring for young; an occasional excursion outside it does not count. A map of where the animal was can
turn that sentence into a number only if someone says *how much of the use is enough*. The modern answer is the
**utilisation distribution** (UD): a surface that gives each cell the probability of finding the animal in it, the
cells adding up to 1. A **home range** is then the smallest area that holds a given share of the use, most often 95 %,
and a **core area** the smallest that holds 50 %. The outline of each share is its **isopleth**: the 95 % isopleth is
the contour that has 95 % of the use inside it.

The estimators differ in how they turn locations into that surface (or into a polygon, when they skip the surface):

- The **minimum convex polygon** (MCP) is the outline of the locations, as a rubber band stretched around them. To
  keep an outlying fix from stretching it, the polygon of level 0.95 leaves out the 5 % of the locations farthest from
  the centre. It has no surface behind it, so it cannot say where the animal spent its time, and it includes all the
  empty ground inside the band.
- The **kernel density estimate** (KDE; Worton, 1989) puts a small bell of width \( h \) on every location and adds
  the bells up. The *bandwidth* \( h \) is everything: a narrow one gives a bump on every fix, a wide one a single
  blob.
- **Local convex hulls** (LoCoH; Getz et al., 2007) draw a small convex polygon around each location and its
  \( k \) nearest neighbours, and add the smallest hulls first until they hold the share of the locations asked for.
  Where the animal never went (a river, a road), the hulls leave a hole, which the other two fill.
- The **dynamic Brownian bridge** (dBBMM; Horne et al., 2007; Kranstauber et al., 2012) does not treat the fixes as
  independent points. It treats the track as a path: between two fixes the animal moved somewhere, most likely along
  the straight line, with an uncertainty that grows with the time since the last fix and shrinks as the next one
  nears. The estimate adds up the bridges.

!!! science "Key idea: the kernel and the bridge"
    **The kernel estimate** of the density at a place \( x \), from the \( n \) locations \( x_i \) and a bandwidth
    \( h \) (the same for both coordinates here), is

    \[ \hat f(x) = \frac{1}{n\,h^2} \sum_{i=1}^{n} K\!\left(\frac{x - x_i}{h}\right), \]

    with \( K \) a bivariate normal density. The *reference* bandwidth, which is the default of amt (Signer et al.,
    2019), is \( h_\text{ref} = \sqrt{(\sigma_x^2 + \sigma_y^2)/2}\; n^{-1/6} \), where \( \sigma_x^2 \) and
    \( \sigma_y^2 \) are the variances of the coordinates: it is the best bandwidth for a single bell-shaped
    cloud of points, and oversmooths a track with several centres of use.

    **The Brownian bridge** between two fixes \( T \) minutes apart: at the time \( \alpha \) after the first, the
    animal's position is normal, centred on the point at the fraction \( \alpha / T \) of the straight line, with
    variance

    \[ \sigma^2(\alpha) = \frac{\alpha\,(T - \alpha)}{T}\,\sigma_m^2 + \left(\frac{T - \alpha}{T}\right)^2 \delta_1^2
    + \left(\frac{\alpha}{T}\right)^2 \delta_2^2. \]

    \( \sigma_m^2 \) is the *motion variance*, how much the animal wanders (in m² per minute), and \( \delta_1 \),
    \( \delta_2 \) are the standard deviations of the location error at the two fixes (Horne et al., 2007). The
    first term is zero at the two fixes and largest half way between them (\( T\sigma_m^2/4 \)): the bridge is narrow
    near a fix and wide in the middle. The *dynamic* model (Kranstauber et al., 2012) lets \( \sigma_m^2 \) change along the track: it
    slides a window over the locations, finds the variance that best explains each window and splits the window in two
    where that lowers the BIC, so a resting animal and a travelling one get different variances.

### Why a time-aware estimator

Points in a track are not independent samples of where the animal lives: two fixes two minutes apart say the same
thing. A kernel estimate counts them twice, and the more often the collar fixes the more it concentrates the use
around the places where the animal lingered, whatever the animal did. The bridge asks a different question: given
where it was and when, where could it have been in between? That is also the reason for a window: an animal that
rests for hours and then crosses its range in one go has two motion variances, and a single one for the whole track
smears both.

The **dynamic bivariate Gaussian bridge** (dBGB; Kranstauber et al., 2014) goes one step further: it gives the
bridge two variances, one *along* the direction the animal is heading and one *across* it. An animal that follows a
trail, a river bank or a fence line spreads along it much more than across it, and a bridge with a single variance
spreads the use in both directions alike.

## When to use it

- **The area an animal uses**, to compare with the size of a reserve, the home ranges of other populations or the
  area a species needs (home range sizes are the most reported number of tracking studies).
- **The core and the whole**: the 50 % and 95 % contours as the zones of a management plan, or as the study areas of
  the landscape metrics of [Landscape metrics](../../landscape-metrics/).
- **Overlap between animals**, as a first look at territoriality or at how animals share ground.
- **The use of a landscape**, by laying the UD over a map of habitat: the next part uses it to check a connectivity
  model.

And what it is *not*: a home range is not where the animal *can* go, nor a measure of its needs. It describes where
one animal was during one period, at one sampling interval, and it depends on how many locations there are (below).
It tells nothing of *why*; to ask which habitat the animal chose, use the step selection functions of
[part 5](../step-selection/). For tracks that are strongly autocorrelated and sampled irregularly the continuous-time
estimators of the ctmm package (autocorrelated kernel density) are the usual alternative; they are not part of
Terraflux.

Which estimator?

| If you want... | Use | Because |
|---|---|---|
| A number to compare with the literature | MCP at 0.95 (and a KDE) | It is what most papers report; say which |
| A smooth surface and contours | KDE with the plug-in bandwidth | A choice that follows the data; the reference bandwidth oversmooths |
| An outline that respects barriers | LoCoH | Hulls leave out the rivers and roads |
| The use of a track sampled at a regular interval | dBBMM | Uses the time between fixes, and the location error |
| A track that follows a line (a patrol, a trail) | dBGB | Separates the spread along the movement from the spread across it |

## What you need

Tracks in a **projected coordinate system** (metres): areas and bandwidths are in the units of the coordinates, and
amt's estimators on longitude and latitude would measure in degrees. Project with [[move.aeqd_crs]]
([part 3](../projections-and-interpolation/)); the nodes here refuse tracks in degrees. For the dynamic bridges,
longitude and latitude are accepted and projected inside.

Locations at a **regular interval**, or at least without large changes in the rhythm: the kernel estimators count
locations and the bridges count time. The fishers were sampled every 2, 10 or 15 minutes with gaps, so this part
thins them with [[move.thin_time]] (part 8 explains it): M4's 8,958 locations become one about every 10 minutes, and
for the comparison of the animals every 30 minutes. The template is **Home range of a track**, which makes a Brownian
track and estimates it three ways; its nodes are the ones below.

## Build the model

### Project, thin and estimate

```tfgraph
#! caption: M4's tracks (node 5, the animal chosen with a filter on the cleaned tracks of part 2), projected on a local map in metres (node 6) and thinned to one location about every 10 minutes (node 7).
#! numbered: true
#! highlight: m4, aeqd, thin
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
id = "m4"
op = "vector.filter"
inputs = { vector = "unique.tracks" }
params = { expression = 'individual_local_identifier == "M4"' }

[[nodes]]
id = "aeqd"
op = "move.aeqd_crs"
inputs = { tracks = "m4.vector" }
note = "Metres from the centre of M4's locations"

[[nodes]]
id = "thin"
op = "move.thin_time"
inputs = { tracks = "aeqd.tracks" }
params = { interval = "10 mins" }
note = "One location about every 10 minutes: the longest chain of lags of 10 minutes ± 1"
```

<div class="steps" markdown>

1. **Read and clean the tracks** as in [part 2](../cleaning-and-measures/) (nodes 1 to 4): the records with a
   location, the ones Movebank shows, one record per animal and time.

2. **Take one animal** with [[vector.filter]] (node 5). In a filter's expression the columns with dashes are written
   with underscores, so `individual-local-identifier` is `individual_local_identifier`. The estimators below work on
   one animal at a time, or on several in one node (each track gets its own polygons); one animal keeps the page
   simple.

3. **Project it** with [[move.aeqd_crs]] (node 6; [part 3](../projections-and-interpolation/)): the locations in
   metres east and north of the centre of M4's locations, which is what the polygons' areas and the bandwidths are
   measured in. The node also gives the coordinate system as a text (`crs`), which the maps of other nodes can share.

4. **Thin the track** with [[move.thin_time]] (node 7). M4 was fixed every 2 minutes when the collar found
   satellites, with gaps; the node keeps the longest chain of locations whose time lags are 10 minutes ± 1, which
   leaves 2,228 of the 8,958. A kernel estimate counts locations, so one that has four times as many fixes on the same
   resting place is four times denser there; a regular interval is what makes animals, and runs of one animal,
   comparable (below, "The sampling interval"). Part 8 is about this node.

</div>

### Three estimates from the locations

```tfgraph
#! caption: M4's home range three ways: the minimum convex polygon (node 8), the kernel density estimate with the reference bandwidth (node 9) and with the plug-in bandwidth (node 10), and local convex hulls with the 47 nearest neighbours (node 11), each at the levels 0.5 and 0.95.
#! fragment: true
#! highlight: mcp, kde, pi, locoh
[[nodes]]
id = "mcp"
op = "move.home_range"
inputs = { tracks = "thin.tracks" }
params = { method = "mcp", levels = [0.5, 0.95] }

[[nodes]]
id = "kde"
op = "move.home_range"
inputs = { tracks = "thin.tracks" }
params = { method = "kde", levels = [0.5, 0.95], bandwidth = "ref", res = 25, factor = 1.25 }

[[nodes]]
id = "pi"
op = "move.home_range"
inputs = { tracks = "thin.tracks" }
params = { method = "kde", levels = [0.5, 0.95], bandwidth = "pi", res = 25, factor = 1.25 }

[[nodes]]
id = "locoh"
op = "move.home_range"
inputs = { tracks = "thin.tracks" }
params = { method = "locoh", levels = [0.5, 0.95], type = "k", n = 47 }
```

[[move.home_range]] (library subject **Animal movement**) is one node for the three methods, as amt's `hr_mcp`,
`hr_kde` and `hr_locoh` are one family. It reads the projected `tracks` and gives:

- `polygons`: a polygon for every track and level, with `track`, `level`, `what` ("estimate") and `area` (the
  coordinate system's unit squared, square metres here). For a kernel estimate the polygon is the contour line of
  the cumulative distribution at the level, closed and united.
- `table`: the same without the geometry, for a chart or a CSV.
- `ud` (kernel density only): the utilisation distribution as a stack with a band for each track, the cells adding up
  to 1.
- `bandwidths` (kernel density only): the bandwidth of each track, `hx` and `hy`.

The settings that matter, by method:

| Setting | What it does |
|---|---|
| `method` | `mcp` (the default), `kde` or `locoh` |
| `levels` | The share of the use each polygon holds: a number or a list, 0 to 1 (the default is 0.95) |
| `bandwidth` (kde) | `ref` (default), `ref_scaled`, `pi`, `lscv`, a number of metres, or a pair `[x, y]` |
| `res`, `factor` (kde) | The cell of the estimate and how far beyond the locations the grid reaches: cells of 1/100 of the largest side and 1.5 by default; or `like`, the raster whose cells to use |
| `type`, `n` (locoh) | `k` (the `n` nearest neighbours, a whole number; try about the square root of the number of locations), `r` (the neighbours within a radius of `n`) or `a` (those whose cumulative distance, nearest first, is at most `n`) |
| `rescale` (kde) | `none`, `unitvar` or `xvar`: scale the coordinates before finding the bandwidth |
| `which_min` (lscv) | The lowest criterion (`global`) or the largest local minimum |
| `num_parts` (ref_scaled) | The most polygons wanted (1) |

A **grid for the estimate** matters for the kernel: the estimate is evaluated at the cells of a template laid over
the locations, and the 95 % contour must lie inside it, or the node stops and says so. Here the cells are 25 m and the
grid reaches a quarter of the largest side beyond the locations. With several animals in one node, one template holds
them all, which is what makes their UDs comparable: the overlap below needs it.

### The bandwidth

The kernel estimate has one setting that changes its answer by a factor of three: the bandwidth \( h \). Terraflux
has the four of amt:

- **`ref`**, the reference bandwidth \( \sqrt{(\sigma_x^2+\sigma_y^2)/2}\,n^{-1/6} \). It is the optimum for a single
  bell-shaped cloud, so it is wide when the animal uses several separate places: the variance of the locations is
  large because of the distances *between* the places, and the bandwidth spreads each place over its neighbours.
- **`ref_scaled`**, the reference bandwidth reduced until the isopleth at the highest level has at most `num_parts`
  polygons (one by default). It is the reference one with a patch against the worst of the oversmoothing.
- **`pi`**, the plug-in bandwidth of Sheather and Jones (1991) (KernSmooth's `dpik`, raised to the power 5/6 as amt
  does). It estimates the curvature of the density from the data, so it follows several centres of use.
- **`lscv`**, least-squares cross-validation (Silverman, 1986): the bandwidth, among 100 from 0.1 to 2 times the
  reference one, whose estimate best predicts the locations left out. It tends to undersmooth with autocorrelated data,
  and often its minimum is at an end of the range, which the node reports as a warning; here it is: the bandwidth it
  chooses is 0.1 times the reference, the lowest it tries.

On a track of GPS fixes, which are autocorrelated (a fisher resting for an hour gives six nearly identical fixes),
the data-based bandwidths read every cluster of fixes as a place of its own, as the results below show.

### The dynamic Brownian bridge

```tfgraph
#! caption: The window the sampling suggests (node 8), the variance along the track (node 9), the utilisation distribution (node 10) and its 50 % and 95 % contours (node 11).
#! fragment: true
#! highlight: window, var, ud, volume
[[nodes]]
id = "window"
op = "move.suggest_window"
inputs = { tracks = "thin.tracks" }

[[nodes]]
id = "var"
op = "move.dbbmm_variance"
inputs = { tracks = "thin.tracks" }
params = { location_error = 20, window_size = 25, margin = 7 }

[[nodes]]
id = "ud"
op = "move.dbbmm_ud"
inputs = { tracks = "var.tracks" }
params = { res = 50 }

[[nodes]]
id = "volume"
op = "move.ud_volume"
inputs = { ud = "ud.ud" }
params = { levels = [0.5, 0.95] }
```

The bridge takes four nodes, each with one job.

<div class="steps" markdown>

1. **Choose the window** with [[move.suggest_window]] (node 8). The variance of the animal's movement is estimated in
   a *window* of `window_size` consecutive locations that slides along the track, and `margin` locations at each end
   of every window are left out of the estimate (they have no neighbours on one side). The node proposes both from
   the sampling: a window of about four hours (`target_hours`) at the track's median time lag, an odd number of
   locations of at least 11 and at most a quarter of the track, and a margin of a quarter of it. For M4 at 10 minutes
   it gives 25 locations (4.2 hours) and a margin of 7. The table has one row per track, with the median lag, the
   window in hours and the number of whole windows that fit.

2. **Estimate the variance** with [[move.dbbmm_variance]] (node 9). In each window the node finds, by maximum
   likelihood, the Brownian motion variance that best explains the locations between their neighbours (each second
   location is left out in turn and predicted by a bridge between its two neighbours); it also tries a split of the
   window in two parts with a variance each, and keeps it when that lowers the BIC. The variance of a location is the
   mean over the windows that cover it. It adds four columns to the tracks: `motion_variance` (m² per minute; empty
   where no window covers), `in_windows` (how many do), `interest` (1 where every window covers: the segments that will
   count in the distribution) and `location_error` (the errors used). `window_size` and `margin` must be odd, and
   `window_size` at least twice the margin plus one.

   `location_error` is **the standard deviation of the error of the locations, in metres**, and the node insists on
   it. It can be a number (for all locations), the name of a column (the error of each fix) or `auto`, which reads the
   e-obs column `eobs:horizontal-accuracy-estimate` or the classes of Argos' `argos_lc` (3: 250 m, 2: 500 m, 1: 1,500 m,
   0 and A: 5,000 m, B: 10,000 m). The fishers' file has none, so 20 m is assumed here, a value a modern collar
   gives under a forest canopy; the results show how little it matters at this error and how much it does at 100 m.
   A fix with no error is filled in by `location_error_na`: the track's `median` (default), `mean`, `zero` or
   `approx` (along the track).

3. **Spread the bridges over a grid** with [[move.dbbmm_ud]] (node 10). It takes the tracks with or without the
   variance's columns (without them it computes the variance with `window_size`, `margin` and `location_error` of its
   own). Each pair of consecutive locations is a bridge; the node walks along it in steps of `time_step` (by default a
   fifteenth of the shortest time lag), and at each step spreads a bivariate normal with the bridge's variance over the
   cells within four standard deviations. The cells add up all the steps and each band is divided by its sum: the
   probability of the animal being in the cell.

   The grid is the box of the locations widened by `ext_factor` (half of its sides by default), in cells of `res`
   metres or of `dim_size` cells along its longest side (100 by default). The node enlarges the grid when the windows
   of the bridges do not fit (and says so with `verbose`). Or give `like`, a raster whose grid to use: it is never
   enlarged, and an error says how large it has to be. Several animals in one node share one grid; with the bursts of
   [[move.resample]] a band is a burst. **Start with a coarse grid**: the cost grows with the cells times the steps of the
   bridges, so a first look with `dim_size` 50 is quick and tells whether the settings are sensible.

4. **Draw the contours** with [[move.ud_volume]] (node 11). The *volume* of a cell is the probability in the cells at
   least as likely as it: add the cells from the most to the least probable and keep the running total. The cells with a
   volume up to 0.95 are the smallest area that holds 95 % of the use, which is the definition of the home range. The node
   gives the `volume` (a raster, a band for each band of the UD) and, with `levels`, the contour of each level as
   `polygons` (with the band, the level and the area) and as a `table`. A contour that reaches the edge of the grid is an
   error: make the grid larger (`ext_factor`).

</div>

**Gaps.** A bridge between two locations a day apart is very wide, and puts probability on the straight line across
ground the animal may never have crossed. [[move.mask_segments]] leaves bridges out of the distribution:

```tfgraph
#! caption: The bridges across gaps longer than 2 hours left out (node 12), and the distribution and contours without them (nodes 13 and 14).
#! fragment: true
#! highlight: masked, ud_gap, volume_gap
[[nodes]]
id = "masked"
op = "move.mask_segments"
inputs = { tracks = "var.tracks" }
params = { gap = 2 }

[[nodes]]
id = "ud_gap"
op = "move.dbbmm_ud"
inputs = { tracks = "masked.tracks" }
params = { res = 50 }

[[nodes]]
id = "volume_gap"
op = "move.ud_volume"
inputs = { ud = "ud_gap.ud" }
params = { levels = [0.5, 0.95] }
```

`gap` is in hours: the segments whose time lag is longer are left out. Or name the segments (`segments`, a list of
numbers or an object by track; the segment *i* is the bridge from location *i* to *i* + 1, counted from 0), or a
`column` of the tracks that is not 0 where the segment starting there should go. The masked bridges leave no mass, and
the rest is divided by its sum: the distribution describes the time actually followed.

**Direction.** [[move.dbgb_variance]] and [[move.dbgb_ud]] are the same two steps for the directional bridge
(Kranstauber et al., 2014). The variance is split along the direction from the expected position to the next location
and across it; each axis has a variance of its own and up to one break point of its own; the columns are `para_sd`
and `orth_sd` (standard deviations), `n_estim`, `seg_interest` and `dbgb_location_error`. The distribution spreads a
normal with two standard deviations along and across the direction of the bridge. The directional model needs more:
long tracks (windows of 31 locations with margins of 15 leave a core of few), a larger grid than the dBBMM's (an error
tells the size of the `like` it needs), and it counts only the core of each track.

```tfgraph
#! caption: The directional variance (node 15) and distribution (node 16), with the windows of the isotropic bridge.
#! fragment: true
#! highlight: gvar, gud
[[nodes]]
id = "gvar"
op = "move.dbgb_variance"
inputs = { tracks = "thin.tracks" }
params = { location_error = 20, window_size = 31, margin = 15 }

[[nodes]]
id = "gud"
op = "move.dbgb_ud"
inputs = { tracks = "gvar.tracks" }
params = { res = 50 }
```

**New locations against a distribution.** [[move.ud_outer_probability]] gives each location its *outer probability*:
the volume of the distribution at the cell that holds it, the share of the use in the cells at least as likely as that
one. It is small in the core of the use and near 1 far outside it. Fed with the locations the distribution was built
from it says nothing (each location raised the cells around it), so give it locations that the distribution did not
see: another period of the same animal, another animal, or the fixes a cleaning step doubts (part 8). With a
distribution of several bands, `band` names the one to use.

```tfgraph
#! caption: Where the second half of M4's track falls in the distribution of the first (node 18, with the first half made by a filter on the time, node 17, not shown).
#! fragment: true
#! highlight: outer
[[nodes]]
id = "outer"
op = "move.ud_outer_probability"
inputs = { points = "second.vector", ud = "ud_first.ud" }
```

The last node of this section is [[move.intensity_use]], for a track and not a distribution: the length of the track
divided by the square root of the area of its 100 % MCP, a number without a unit that says how many times the animal
went back and forth over the ground it covered.

### Several animals, and their overlap

```tfgraph
#! caption: The seven fishers near Albany projected (node 5) and at one location per 30 minutes (node 6), their kernel estimates on one grid (node 7) and the overlap of their home ranges (node 8) and of their distributions (node 9).
#! numbered: true
#! highlight: kde, overlap_hr, overlap_vi
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
id = "unique"
op = "move.filter_unique"
inputs = { tracks = "located.vector" }

[[nodes]]
id = "near"
op = "vector.filter"
inputs = { vector = "unique.tracks" }
params = { expression = 'individual_local_identifier != "M5"' }

[[nodes]]
id = "aeqd"
op = "move.aeqd_crs"
inputs = { tracks = "near.vector" }

[[nodes]]
id = "thin"
op = "move.thin_time"
inputs = { tracks = "aeqd.tracks" }
params = { interval = "30 mins" }

[[nodes]]
id = "kde"
op = "move.home_range"
inputs = { tracks = "thin.tracks" }
params = { method = "kde", levels = [0.5, 0.95], bandwidth = "ref" }

[[nodes]]
id = "overlap_hr"
op = "move.home_range_overlap"
inputs = { polygons = "kde.polygons" }
params = { type = "hr", which = "all" }

[[nodes]]
id = "overlap_vi"
op = "move.home_range_overlap"
inputs = { ud = "kde.ud" }
params = { type = "vi", which = "all" }
```

M5 is left out: it lives 40 km east of the others, and one template holds all the animals of a node, so its distance
would make the grid ten times larger for nothing. [[move.home_range_overlap]] compares the animals of a
[[move.home_range]] node, for the pairs `consecutive` (the default), `all` (each against each other, both ways) or
`one_to_all` (the first against the others). The six indices (Fieberg and Kochanny, 2005):

| `type` | Reads | Says |
|---|---|---|
| `hr` | The polygons | The share of the first animal's polygon that the second's covers, at each level; not symmetric |
| `phr` | The distributions | The probability that the second animal is inside the first's home range |
| `vi` | The distributions | The volume of intersection: the sum over the cells of the smaller of the two probabilities, from 0 (apart) to 1 (the same) |
| `ba` | The distributions | Bhattacharyya's affinity: the sum of the square roots of the products of the two probabilities, from 0 to 1 |
| `udoi` | The distributions | The utilisation distribution overlap index: above 1 when both animals concentrate their use in the same few cells |
| `hd` | The distributions | Hellinger's distance: 0 for equal distributions, larger the more they differ |

The distribution indices need kernel estimates made on the same grid, which one node gives. With `conditional` the
distributions are cut to the cells inside each level's isopleth (the `levels` of the polygons, or `levels`) and divided
by their sum, one row per level: the overlap of the cores (0.5) is then not drowned by the tails.

### In Python

Everything above is in the library, as `tf.move` (amt's `hr_*` functions under their own names, and move2utils'
bridges). The same estimates of M4:

```python
import numpy as np
import pandas as pd
import terraflux as tf

ID = "individual-local-identifier"
tracks = tf.load_tracks(tf.move.example_data("fishers"))
tracks = tf.move.filter_unique(tf.move.filter_movebank_visible(tracks))
tracks = tracks[~tracks.geometry.is_empty]

m4 = tracks[tracks[ID] == "M4"].reset_index(drop=True)
m4 = m4.to_crs(tf.move.aeqd_crs(m4))                          # metres from the centre
m4 = m4[tf.move.thin_time(m4, "10 mins")].reset_index(drop=True)
print(len(m4), "locations at about 10 minutes")
```

```text
2228 locations at about 10 minutes
```

```python
levels = [0.5, 0.95]
trast = tf.move.make_trast(m4, factor=1.25, res=25)           # the cells of the kernel estimate
estimates = {
    "MCP": tf.move.hr_mcp(m4, levels=levels),
    "KDE, reference": tf.move.hr_kde(m4, trast=trast, levels=levels),
    "KDE, plug-in": tf.move.hr_kde(m4, h=tf.move.hr_kde_pi(m4), trast=trast, levels=levels),
    "LoCoH, k = 47": tf.move.hr_locoh(m4, n=47, levels=levels),
}
for name, hr in estimates.items():
    area = tf.move.hr_area(hr).set_index("level")["area"] / 1e6
    print(f"{name:15s} core {area[0.5]:5.2f} km2, home range {area[0.95]:5.2f} km2")
print(tf.move.hr_kde_ref(m4).round(0).iloc[0].tolist(), tf.move.hr_kde_pi(m4).round(0).iloc[0].tolist())
```

```text
MCP             core  1.77 km2, home range  8.08 km2
KDE, reference  core  2.38 km2, home range  9.99 km2
KDE, plug-in    core  0.48 km2, home range  3.21 km2
LoCoH, k = 47   core  0.32 km2, home range  3.41 km2
[261.0, 261.0] [35.0, 36.0]
```

The bandwidths are `hr_kde_ref`, `hr_kde_ref_scaled`, `hr_kde_pi` and `hr_kde_lscv` (a table by track, in metres), and
`hr_kde(h=...)` takes a number, a pair or such a table. The polygons are `tf.move.hr_isopleths(hr)` (the contours of
a kernel estimate or the polygons made), a GeoDataFrame to plot or to write with `tf.write_vector`.

```python
sug = tf.move.suggest_dbbmm_window(m4)
print(sug.window_size, sug.margin, round(sug.window_hours, 1), "hours")

v = tf.move.dbbmm_variance(m4, location_error=20, window_size=25, margin=7)
print(v[["motion_variance", "in_windows", "interest", "location_error"]].describe().loc[["count", "50%", "max"]].round(1))
ud = tf.move.dbbmm_ud(v, res=50)                               # a band per track, summing to 1
print(ud.shape, round(float(ud.sum()), 6))
iso = tf.move.hr_isopleths(ud.isel(band=0), levels=levels)     # the contours of the volume
print((iso.set_index("level")["area"] / 1e6).round(2).to_dict())
```

```text
25 7 4.2 hours
       motion_variance  in_windows  location_error
count           2215.0      2215.0          2228.0
50%              362.4        12.0            20.0
max             5335.3        12.0            20.0
(1, 152, 235) 1.0
{0.95: 7.87, 0.5: 0.61}
```

```python
# leave out the bridges across gaps of more than 2 hours (segment i: from location i to i + 1)
lag = (m4["timestamp"].shift(-1) - m4["timestamp"]).dt.total_seconds() / 3600
gap = (lag > 2).to_numpy()
ud_gap = tf.move.dbbmm_ud(tf.move.mask_segments(v, gap), res=50)
iso_gap = tf.move.hr_isopleths(ud_gap.isel(band=0), levels=levels)
print(int(gap.sum()), "bridges left out;", (iso_gap.set_index("level")["area"] / 1e6).round(2).to_dict())

# the directional bridge: longer windows and margins of 15
g = tf.move.dbgb_variance(m4, location_error=20, window_size=31, margin=15)
ud_dir = tf.move.dbgb_ud(g, res=50)
print(ud_dir.shape, g[["para_sd", "orth_sd"]].median().round(1).to_dict())
```

```text
76 bridges left out; {0.95: 3.6, 0.5: 0.25}
(1, 175, 235) {'para_sd': 22.0, 'orth_sd': 12.9}
```

```python
# where the later half of the track falls in the distribution of the first
half = m4["timestamp"] < pd.Timestamp("2010-03-06", tz="UTC")
first = tf.move.dbbmm_ud(m4[half], location_error=20, window_size=25, margin=7, res=50, ext_factor=1.0)
outer = tf.move.ud_outer_probability(m4[~half], first)
print(len(outer), "later locations;", round(float((outer <= 0.95).mean()) * 100, 1), "% inside the 95 % contour")

# several animals, at one interval, on one grid: the overlap of their kernel estimates
others = tracks[tracks[ID] != "M5"].reset_index(drop=True)
others = others.to_crs(tf.move.aeqd_crs(others))
p30 = others[tf.move.thin_time(others, "30 mins")].reset_index(drop=True)
kde30 = tf.move.hr_kde(p30, trast=tf.move.make_trast(p30), levels=levels)
vi = tf.move.hr_overlap(kde30, type="vi", which="all")
print(vi[(vi["from"] == "F2") & (vi["to"] == "M1")].round(3).to_string(index=False))
print(tf.move.intensity_use(p30).round(1).to_dict())
```

```text
1277 later locations; 93.4 % inside the 95 % contour
from to  level  overlap
  F2 M1    1.0    0.466
{'F1': 33.9, 'F2': 37.6, 'F3': 37.0, 'M1': 32.4, 'M2': 27.9, 'M3': 38.7, 'M4': 60.4}
```

The functions take `n_threads` (the default is most of the cores; the result does not depend on it).
`tf.move.home_range(tracks, "kde", ...)` is a shortcut for the three methods.

## Read the results

### Estimates of one home range

![M4's home range by four estimators](/img/tutorials/animal-movement/home-ranges/estimators.png "M4's locations at about 10 minutes (grey) and the 95 % (orange) and 50 % (black) isopleths of the minimum convex polygon, the kernel density estimate with the reference bandwidth, local convex hulls (k = 47) and the dynamic Brownian bridge (window 25, margin 7, location error 20 m). The green is the utilisation distribution, darker where there is more use (square root scale). Coordinates in kilometres from the centre of the locations.")

```tftable
data: /data/tutorials/animal-movement/home-ranges/estimators.csv
columns: [estimator, bandwidth (m), core 50 % (km²), home range 95 % (km²)]
digits: 2
caption: The core (50 %) and the home range (95 %) of M4 by each estimator. The cross-validated bandwidth is the lowest one it tries (it did not converge); the dynamic Brownian bridge is also given without the bridges across gaps of more than 2 hours.
```

The same 2,228 locations give home ranges from 2.7 to 10.0 km², a factor of 3.7, and cores from 0.25 to 2.4 km², a
factor of ten. None of the numbers is wrong; they answer different questions.

- The **MCP** (8.1 km²) and the **reference kernel** (10.0 km²) are outlines. The MCP contains the ground between the
  western and the eastern groups of locations, where the track only crosses; the kernel adds a margin of about twice
  its bandwidth around every location, and its 50 % contour joins the places of most use into two large blobs.
- The **LoCoH** (3.4 km²) and the **plug-in kernel** (3.2 km²) follow the clusters of locations, and leave the corridors
  between them out. Their cores are the clusters themselves: 0.32 and 0.48 km².
- The **dynamic Brownian bridge** (7.9 km²) is as large as the MCP, and the reason is in the track, not the method:
  the thinned track has 251 gaps of more than an hour, of up to 23 hours, and a bridge across a gap spreads probability
  over every route the animal could have taken. When the 76 bridges across gaps of more than 2 hours are left out,
  the home range falls to 3.6 km², next to the plug-in kernel's and LoCoH's, and the core to 0.25 km².

A report of a home range is therefore a report of an estimator and its settings: "the 95 % kernel home range, plug-in
bandwidth of 35 m, cells of 25 m, 3.2 km²". A number alone cannot be compared with another paper's.

### The bandwidth

![The kernel estimate with four bandwidths](/img/tutorials/animal-movement/home-ranges/bandwidths.png "The kernel density estimate of M4's locations with the reference bandwidth (261 m), the reference bandwidth scaled to one polygon (258 m), the plug-in bandwidth (35 m) and the cross-validated one (26 m). The 95 % isopleth is orange, the 50 % black.")

```tftable
data: /data/tutorials/animal-movement/home-ranges/bandwidths.csv
digits: 2
caption: The bandwidth, the number of polygons of the 95 % isopleth, and the areas, for the four bandwidths.
```

The reference bandwidth and its scaled form are the same here (258 against 261 m): the isopleth already has one
polygon. The plug-in and the cross-validated bandwidths are ten times smaller, and the 95 % isopleth breaks in
56 polygons for the plug-in one: a polygon round every place where the animal stopped. That is not a failure of the
estimator, which does what it is asked, but of the question: the locations of a fisher resting for an hour are six
nearly identical points, and the data-based bandwidths read each rest as a separate centre of use. The cross-validation
shows it more: its criterion kept falling down to the lowest bandwidth it tried (one tenth of the reference, 26 m),
which it reports as a warning.

### Move the bandwidth yourself

```tfslider
label: Bandwidth, times the reference
frames:
  - { value: 0, src: /img/tutorials/animal-movement/home-ranges/bw_0.png, caption: "0.25 (65 m)", stats: { Polygons: "11", Core: "0.79 km²", Home range: "4.68 km²" } }
  - { value: 1, src: /img/tutorials/animal-movement/home-ranges/bw_1.png, caption: "0.5 (131 m)", stats: { Polygons: "3", Core: "1.35 km²", Home range: "6.97 km²" } }
  - { value: 2, src: /img/tutorials/animal-movement/home-ranges/bw_2.png, caption: "1 (261 m, the reference)", stats: { Polygons: "1", Core: "2.38 km²", Home range: "9.99 km²" } }
  - { value: 3, src: /img/tutorials/animal-movement/home-ranges/bw_3.png, caption: "1.5 (392 m)", stats: { Polygons: "1", Core: "3.21 km²", Home range: "13.11 km²" } }
  - { value: 4, src: /img/tutorials/animal-movement/home-ranges/bw_4.png, caption: "2 (522 m)", stats: { Polygons: "1", Core: "3.97 km²", Home range: "16.55 km²" } }
caption: M4's kernel estimate with the bandwidth set to a multiple of the reference one (a number of metres in the node, bandwidth = 131). Polygons is the number of parts of the 95 % isopleth.
```

Doubling the bandwidth from 261 to 522 m takes the home range from 10.0 to 16.6 km², and halving it brings it to
7.0. A kernel home range's area has no value of its own: it is the answer to a bandwidth.

### The variance of the movement

![The motion variance along M4's track](/img/tutorials/animal-movement/home-ranges/variance.png "The motion variance of M4's track (10 minutes, window 25, margin 7, location error 20 m). Left: against time, a point per location, on a log scale. Right: on the map, the logarithm of the variance.")

The median motion variance is 362 m² per minute, and the central 90 % of the locations run from 4.4 to 1,926: a factor of
more than 400 between the quietest and the most restless stretches of the same animal. A model with one variance for the
whole track averages them; the dynamic one gives each stretch its own. Low variance means that the locations of a
window lie close to the line between their neighbours, high that they wander. 98.4 % of the segments are covered by every
window, so they count in the distribution (`interest`); the others lie within the margins at the ends of the track.

### The distribution and its contours

![M4's utilisation distribution and its contours](/img/tutorials/animal-movement/home-ranges/dbbmm.png "The dynamic Brownian bridge utilisation distribution of M4 (cells of 50 m) with the 95 % (orange) and 50 % (black) contours of its volume (move.ud_volume) and the locations.")

The core is not one blob but a dozen or so small ones: the places where the fisher stayed for hours. The 95 % contour wraps
them and the paths between, and in the south-east follows a thin strip to the end of the track.

### The gaps

```tfslider
label: Bridges left out when the gap is longer than
frames:
  - { value: 0, src: /img/tutorials/animal-movement/home-ranges/gap_0.png, caption: "none left out", stats: { Segments: "0", Core: "0.61 km²", Home range: "7.87 km²" } }
  - { value: 1, src: /img/tutorials/animal-movement/home-ranges/gap_1.png, caption: "12 hours", stats: { Segments: "16", Core: "0.35 km²", Home range: "5.40 km²" } }
  - { value: 2, src: /img/tutorials/animal-movement/home-ranges/gap_2.png, caption: "4 hours", stats: { Segments: "30", Core: "0.29 km²", Home range: "4.15 km²" } }
  - { value: 3, src: /img/tutorials/animal-movement/home-ranges/gap_3.png, caption: "2 hours", stats: { Segments: "76", Core: "0.25 km²", Home range: "3.60 km²" } }
  - { value: 4, src: /img/tutorials/animal-movement/home-ranges/gap_4.png, caption: "1 hour", stats: { Segments: "251", Core: "0.27 km²", Home range: "3.50 km²" } }
caption: M4's dynamic Brownian bridge distribution with the bridges of the segments whose time lag is longer than the gap left out (move.mask_segments, gap). Segments is the number left out.
```

Sixteen bridges across gaps of 12 hours and more, 6 % of the 251 that last more than an hour, account for
2.5 km² of the 7.9 km². The longer the gap the less the track says about the route; leaving the bridges across it out
turns the home range into the area the animal was *seen* to use, a lower bound. Which gap to leave out is a choice that
the report must state (the vignette of move2utils on gap-aware distributions does the same with a gap of its own).

### The settings of the bridge

```tftable
data: /data/tutorials/animal-movement/home-ranges/location-error.csv
digits: 3
caption: M4's dynamic Brownian bridge with the location error from 0 to 100 m (window 25, margin 7, cells of 50 m).
```

Up to 20 m, the location error hardly changes the home range (8.0 to 7.9 km²), because the animal moves much more than
that in ten minutes. At 100 m the error is larger than the movement: the median variance falls to zero, because
the node attributes the scatter of the fixes to the error, the distribution is then only the error's bell on each fix,
and the core grows. **A location error above the typical step of the track is a sign that the track is too finely
sampled for the error, or that the error is too large.** Give the error of your receiver, not a guess.

```tftable
data: /data/tutorials/animal-movement/home-ranges/window.csv
digits: 3
caption: M4's dynamic Brownian bridge with five windows (location error 20 m, cells of 50 m). The margin is about a quarter of the window.
```

The window is the other choice. A short one (11 locations, under two hours) sees more changes of behaviour and is
noisier: the home range is largest (8.9 km²) and the core smallest. A long one (201 locations, 33 hours) averages the
changes: the home range shrinks to 6.8 km². The suggestion (25 locations, four hours) is in between. A window must span
a few changes of behaviour, several hours to days for most GPS tracks, and the track must have a few windows.

### Along and across: the directional bridge

![The dynamic Brownian bridge and the directional one](/img/tutorials/animal-movement/home-ranges/dbgb.png "Top: M4's distribution by the dBBMM and the dBGB (window 31, margin 15, location error 20 m). Bottom: a stretch of a simulated animal that patrols a curved line (1,500 locations ten minutes apart, location error 3 m; the data are in patrol.csv), with the 95 % (orange) and 50 % (black) contours of each.")

```tftable
data: /data/tutorials/animal-movement/home-ranges/dbgb.csv
digits: 3
caption: The core and the home range of M4 and of the simulated patrol by the isotropic bridge and the directional one (cells of 50 m for M4 and of 10 m for the patrol).
```

On the fisher the two give the same home range (7.3 and 7.2 km², a ratio of 0.98): a fisher hunting in a forest does not
follow a line, and its spread along its direction (the median standard deviation is 22 m) is not much larger than its
spread across it (13 m). On the simulated patrol, which moves along a line with a few metres of wander across it,
the standard deviations are 10 m along and 3 m across, and the directional bridge draws the use as a strip a
third narrower: a home range 0.73 times the isotropic one. Use it for animals that follow a trail, a river bank
or a fence, and with tracks long enough for its windows; for the rest, the dBBMM is the safer model.

### New locations against a distribution

![Where the later locations of M4 fall in the distribution of the first part of its track](/img/tutorials/animal-movement/home-ranges/outer.png "M4's distribution from the locations before 6 March 2010 (contours at 50 % and 95 %) and the outer probability of the later locations (move.ud_outer_probability): dark where the distribution gave them much use, light where it gave them little. Circled: locations outside the 95 % contour.")

The 951 locations before 6 March give a distribution. Of the 1,277 later ones, 93.4 % fall inside its 95 % contour, none has an
outer probability above 0.99, and the median is 0.63 (the median of the first part's own locations is 0.35). The 6.6 % outside
are mostly one cluster in the north, about a kilometre from the centre, which is the largest core of the whole track's
distribution above: a place that the animal began to use in the second half. This is what the outer probability is for. It does not tell an error from a change of habits, it tells which locations the distribution of
the earlier movements finds unlikely, and these are the ones to look at on the map. Fed with the locations the distribution
was built from it would flag almost nothing, as each of them raised the cells around it.

### The estimators across the interval

```tftable
data: /data/tutorials/animal-movement/home-ranges/interval.csv
digits: 2
caption: M4's 95 % home range (km²) as it was recorded and thinned to longer intervals (move.thin_time), by five estimators. The dynamic Brownian bridge on the track as recorded (minutes of computing) was not run.
```

No estimator is independent of the sampling interval. The MCP drifts down slowly (8.8 to 7.6 km²) as fewer locations reach
the edges. The reference kernel goes up (9.3 to 11.1 km²): its bandwidth is \( n^{-1/6} \), so it grows as the
locations are fewer. The plug-in kernel and the LoCoH go down (by 9 % and 19 % from 10 minutes to an hour). The
dynamic Brownian bridge goes up from 7.9 to 11.0 km² between 10 minutes and 2 hours (not steadily: 9.4 at 30 minutes and 9.1 at an hour) as the bridges between the fixes widen. If you compare animals
or studies, thin them to one interval first (the next section does it for the fishers); if you cannot, say so.

### Several animals

```tftable
data: /data/tutorials/animal-movement/home-ranges/animals.csv
digits: 2
caption: The seven fishers near Albany at one location per 30 minutes (move.thin_time): the 95 % home range (km²) by five estimators, and the intensity of use of the track. The dynamic Brownian bridge uses the window each animal's sampling suggests (11 locations, margin 3: the least) and a location error of 20 m.
```

At one interval the animals are comparable. The two males M1 and M2 have the largest ranges by every estimator (the MCP says 20 and 17 km², the
reference kernel 31 and 27, LoCoH 8 and 5); F1, F2, M3 and M4 have 5 to 8 km² by the MCP, and F3 only 1.1. The
estimators keep their order: LoCoH and the plug-in kernel are the smallest, the reference kernel the largest. The
*intensity of use*, the length of the track over the square root of its 100 % MCP, is 28 to 39 for most and 60 for M4,
which was followed for 50 days, the others for 16 to 24: a longer track over the same ground has more going back and
forth. It grows with the sampling rate as well, so compare it at one interval, as here.

### Overlap

![The kernel estimates of two pairs of fishers that share ground](/img/tutorials/animal-movement/home-ranges/overlap.png "The 95 % (thick) and 50 % (thin) isopleths of the reference kernel of F2 and M1, and of F3 and M3, with their locations at one per 30 minutes. In both pairs the female's range lies inside the male's.")

```tftable
data: /data/tutorials/animal-movement/home-ranges/overlap.csv
digits: 3
caption: Six overlap indices (move.home_range_overlap, reference bandwidth, one grid) of the pairs of fishers that share ground, the first animal of each pair against the second. hr: 95 % polygons; phr: probability that the second is inside the first's home range; vi: volume of intersection; ba: Bhattacharyya's affinity; udoi: overlap index; hd: Hellinger's distance.
```

F2 and M1 share a lot: the whole of F2's 95 % polygon lies inside M1's (`hr` = 1), and an M1 location has 0.79 probability
of being inside F2's home range. But the volume of intersection is 0.47: less than half of the use is shared, since M1 also
uses ground F2 does not. F3 and M3 look alike by the polygons and by `phr` (0.88) but their distributions are
concentrated in the same few cells (`udoi` 48.8). F1 and M2 overlap less (`vi` 0.17, `hr` 0.63). The other pairs share
almost nothing (`vi` below 0.06). The polygon index tells only whether a range lies inside another, the distribution
ones how the use is shared.

```tftable
data: /data/tutorials/animal-movement/home-ranges/overlap-bandwidth.csv
digits: 3
caption: The overlap of F2 and M1 with the three kernel bandwidths: the volume of intersection and the overlap index of the whole distributions, and the volume of intersection conditional on the 50 % and 95 % isopleths.
```

And the bandwidth changes the overlap as it changed the area: with the reference bandwidth the volume of intersection
of F2 and M1 is 0.47, with the plug-in one 0.27 and with the cross-validated one 0.21. Conditional on the core (the
50 % isopleth) it is 0.27, 0.09 and 0.10. Overlap of home ranges is not interaction: two animals can use the same
ground in different weeks, and these were not followed together in all cases.

## Settings that change the answer

The sliders and tables above are the sensitivity analysis:

- **The estimator** (a factor of 3.7 in the home range, ten in the core).
- **The bandwidth** of the kernel (65 m to 522 m: 4.7 to 16.6 km²).
- **The gaps left out** of the Brownian bridge (none to 1 hour: 7.9 to 3.5 km²).
- **The window and the location error** of the Brownian bridge.
- **The interval** of the locations.
- **The level.** The 95 % isopleth is a convention; 90 % or 99 % changes the area much more for a kernel (whose tails are long)
  than for an MCP.

Report them. A table like the first one of this part, with the estimator and its settings in each row, says more than a number.

## Common pitfalls

- **Locations in degrees.** [[move.home_range]] and [[move.intensity_use]] stop on tracks in longitude and latitude: areas
  and bandwidths would be in degrees. Project with [[move.aeqd_crs]] first. The dynamic bridges accept degrees and
  project inside, giving the distribution in metres.
- **A template that is too small.** The 95 % isopleth of a kernel estimate that reaches the edge of the grid is an error:
  give a larger `factor` (or a `like`). With several animals, one template holds them all: a distant animal makes it
  large (M5 above).
- **A grid coarser than the bandwidth.** Cells much larger than the bandwidth blur the estimate (the node warns). Here
  the cells are 25 m and the smallest bandwidth 26 m.
- **Comparing areas of different samples.** Areas depend on the interval, the number of locations and how long the animal
  was followed. Thin to one interval, and compare animals followed for similar periods.
- **Gaps in the Brownian bridge.** A long gap is a wide bridge. Look at the time lags (part 2) before trusting a dBBMM, and
  decide the gap to leave out with [[move.mask_segments]].
- **No location error.** A real error taken for movement inflates the variance and the distribution; an error larger than
  the movement drives the variance to zero. Use the receiver's.
- **A window that does not fit.** A track shorter than the window gets no distribution (a warning); the dBGB's windows reach
  further than the dBBMM's, and an error says how large a `like` it needs.
- **Outer probability of the locations the distribution was built from.** Each location raised the cells around it: use
  other locations.
- **Ranges that are not interaction.** Overlap says that two animals used the same ground, not that they met.
- **One number.** An area without its estimator, bandwidth or window, level, interval and cell is not a result.

## What the operators and the library agree on

Every number of this part was computed by the library (`tf.move`) and by the operators, on the same tracks; the areas of the
minimum convex polygons, the kernel estimates with the reference and plug-in bandwidths, the local hulls, the contours of the
Brownian bridge (with and without the gaps), and the outer probabilities agree to the last digit (a relative difference of
0.0 in the run that made these figures). Against the R packages the claim is the following:

- **amt** (0.3.1, Signer et al., 2019): from projected tracks the polygons, the areas, the bandwidths and the kernel
  distributions are those of `hr_mcp`, `hr_kde` (with the four bandwidths) and `hr_isopleths`, bit for bit; so are the
  overlap of polygons and the six indices, to 1e-12 (relative), since the distribution is normalised again. The LoCoH
  areas agree to 1e-8 (relative): GEOS 3.13's buffer of the hulls has other vertices than the 3.12 of the references.
  Where amt is wrong the result is the right one, tested against corrected code: `hr_mcp` and `intensity_use` fail
  on a track without a coordinate system, and `hr_kde_pi` and `hr_kde_lscv` with `rescale = "unitvar"` fail on a tibble.
- **move2utils** (0.4.6, Kranstauber et al., 2026): from projected tracks the variance and the distribution of the dBBMM
  and the dBGB, the masks, the volume and the outer probability are bit for bit those of `mt_dbbmm_variance`,
  `mt_dbbmm_ud`, `mt_dbgb_variance`, `mt_dbgb_ud`, `mt_mask_segments`, `ud_volume` and `ud_outer_probability`, for any number
  of threads. One cell of one dBGB test grid differs by one unit in the last place from R (the `acos` of Windows' C library
  against R's). From longitude and latitude the projection is done inside, and the result agrees with R by a tolerance
  (the PROJ of the library and the PROJ of R differ by a few units in the last place of the coordinates, which the variance
  carries to about 1e-7). `crs = "input"` warps the distribution back to degrees as `terra` does, bilinearly, and it is
  not bit for bit.
- **Where move2utils is wrong the right thing is done**, and tested against the code corrected: its `mt_aeqd_crs` centres
  the projection on a point moved to EPSG:4236 (the Hu Tzu Shan datum, about a kilometre away); its `location_error = "auto"`
  and the column names do not find Movebank's `eobs:horizontal-accuracy-estimate`; the loop that reaches the last cell of
  the kernel reads one cell past the grid, which here is a window that does not fit.

## The template

**Home range of a track** (library subject **Animal movement**, group *Movement*) builds this part's models on a simulated
Brownian track of 150 locations: the minimum convex polygon, the reference kernel and the dynamic Brownian bridge with its 50 %
and 95 % contours, and the three areas as bar charts side by side. Replace the simulation by [[move.read_tracks]] of your own
Movebank file (project the tracks first), and set the location error of your collar.

## Going further

- [Observed corridors and validating connectivity](../corridors-and-validation/): lay the distribution over a connectivity
  model, find the corridors the animals followed, and compare distributions with the earth mover's distance.
- [Clean a track of GPS errors and thin it](../cleaning-and-thinning/): the errors a home range hates, and the thinning
  that this part used.
- [Landscape metrics](../../landscape-metrics/): the home range and core contours are polygons that the metrics of a
  landscape take as zones.
- The operators' reference: [[move.home_range]], [[move.home_range_overlap]], [[move.intensity_use]],
  [[move.suggest_window]], [[move.dbbmm_variance]], [[move.dbbmm_ud]], [[move.mask_segments]], [[move.dbgb_variance]],
  [[move.dbgb_ud]], [[move.ud_volume]], [[move.ud_outer_probability]].

## References

- Burt, W. H. (1943). Territoriality and home range concepts as applied to mammals. *Journal of Mammalogy*, 24(3), 346-352.
  https://doi.org/10.2307/1374834
- Fieberg, J., & Kochanny, C. O. (2005). Quantifying home-range overlap: the importance of the utilization distribution.
  *Journal of Wildlife Management*, 69(4), 1346-1359. https://doi.org/10.2193/0022-541X(2005)69[1346:QHOTIO]2.0.CO;2
- Getz, W. M., Fortmann-Roe, S., Cross, P. C., Lyons, A. J., Ryan, S. J., & Wilmers, C. C. (2007). LoCoH: nonparameteric
  kernel methods for constructing home ranges and utilization distributions. *PLoS ONE*, 2(2), e207.
  https://doi.org/10.1371/journal.pone.0000207
- Horne, J. S., Garton, E. O., Krone, S. M., & Lewis, J. S. (2007). Analyzing animal movements using Brownian bridges.
  *Ecology*, 88(9), 2354-2363. https://doi.org/10.1890/06-0957.1
- Kranstauber, B., Kays, R., LaPoint, S. D., Wikelski, M., & Safi, K. (2012). A dynamic Brownian bridge movement model to
  estimate utilization distributions for heterogeneous animal movement. *Journal of Animal Ecology*, 81(4), 738-746.
  https://doi.org/10.1111/j.1365-2656.2012.01955.x
- Kranstauber, B., Safi, K., & Bartumeus, F. (2014). Bivariate Gaussian bridges: directional factorization of diffusion in
  Brownian bridge models. *Movement Ecology*, 2(1), 5. https://doi.org/10.1186/2051-3933-2-5
- Kranstauber, B., Safi, K., & Scharf, A. K. (2026). move2utils: a utility toolkit for the move2 ecosystem. *bioRxiv*.
  https://doi.org/10.64898/2026.07.07.736908
- Sheather, S. J., & Jones, M. C. (1991). A reliable data-based bandwidth selection method for kernel density estimation.
  *Journal of the Royal Statistical Society: Series B (Methodological)*, 53(3), 683-690.
  https://doi.org/10.1111/j.2517-6161.1991.tb01857.x
- Signer, J., Fieberg, J., & Avgar, T. (2019). Animal movement tools (amt): R package for managing tracking data and
  conducting habitat selection analyses. *Ecology and Evolution*, 9(2), 880-890. https://doi.org/10.1002/ece3.4823
- Silverman, B. W. (1986). *Density estimation for statistics and data analysis*. Chapman & Hall.
- Wand, M. P., & Jones, M. C. (1995). *Kernel smoothing*. Chapman & Hall/CRC. https://doi.org/10.1201/b14876
- Worton, B. J. (1989). Kernel methods for estimating the utilization distribution in home-range studies. *Ecology*, 70(1),
  164-168. https://doi.org/10.2307/1938423
