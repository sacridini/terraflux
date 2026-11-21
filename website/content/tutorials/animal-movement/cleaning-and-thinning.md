---
title: Clean a track of GPS errors and thin it
short: GPS errors and thinning
description: Drop the fixes a GPS marks as unreliable, flag the steps faster than the animal can move with a limit from its body mass (Hirt et al., 2017), see what each filter catches and what it costs, and thin a track to a regular interval in time or distance.
order: 80
level: intermediate
time: 60 min
ops: [move.filter_gps_quality, move.max_speed, move.flag_speed, move.thin_time, move.thin_distance, move.filter_per_interval]
series: Animal movement
part: 8
template: Cleaning a track
---

[Part 2](../cleaning-and-measures/) cleaned the fishers' tracks with what Movebank already knows (the records it hides,
the repeated times) and found, by looking at the speeds, that the worst of the data are the very short steps. This part
goes on to the errors that a record does not announce: the fix that put the animal 2 km from where it was. You will
drop the fixes a GPS marks as unreliable, flag the steps that no animal could take, thin the track to the interval that
the next analysis needs, and see on a track whose errors are known how many of them each filter caught, how many good
fixes it threw away with them, and what the errors that remain do to what you measure. At the end you will have a
clean track at a regular interval and a table that tells how clean.

The fishers' file has no gross errors (its fastest step is 9.8 m/s) and no GPS quality columns, so a track with errors
is made on purpose: M4's 8,958 locations get simulated satellite counts and dilution of precision, and 80 of them are
spoiled. Which ones is recorded in the file you can download, so every number below about what a filter caught is checked
against what was put in.

## The idea

### What a GPS gets wrong

A GPS collar solves the position from the signals of the satellites it sees. Three numbers that the receiver reports
with each fix say how much to trust it:

- the **number of satellites**: three is the least that gives a position, and a fix needs four to have an altitude; five
  is the usual floor for a reliable one;
- the **dilution of precision** (DOP): how the geometry of the satellites in the sky magnifies the errors. A value near 1
  is ideal; above 10 the position is poor;
- the **horizontal accuracy estimate** that some receivers (the e-obs collars) give in metres.

With few satellites or a high DOP the fix can be tens or hundreds of metres off, and now and then kilometres (Frair et
al., 2010). But a bad fix does not always look bad: a receiver can be sure of
a position that is wrong. So two filters work together: one on what the receiver says about the fix, and one on what
the movement says about it: **an animal cannot move faster than its body allows.**

### The speed limit

Hirt et al. (2017) fitted the maximum speed of hundreds of species of animals that fly, run or swim against their body
mass \( M \), and found a law that explains why the largest animals are not the fastest:

\[ v_\text{max} = a\,M^{b}\left(1 - e^{-h\,M^{i}}\right). \]

The speed grows with mass as a power law \( aM^b \), but the factor in brackets, a term that depends on the time an
animal needs to reach its top speed, bends the curve down for the largest. \( a \), \( b \), \( h \) and \( i \) are
fitted for each *mode* of locomotion (flying, running, swimming), and the fit explains 89 % of the variation among
species (\( R^2 = 0.893 \)). The law gives a **ceiling**, for horizontal speed in bursts,
not a cruising speed: specialists like the cheetah and the pronghorn exceed it, and the scatter between species is about
a factor of two more than the interval of the fit.

A step faster than the ceiling is an error. But which end of the step is wrong? From the data alone, neither can be told: a
jump to a wrong position and back makes *two* fast steps, and a track that jumped and stayed makes one. The filter flags
**both fixes of every fast step**, and the good fix next to a bad one is lost with it.

### Why thin

Thinning, taking fewer locations on purpose, is the other half of cleaning:

- Many analyses suppose **one sampling interval** (the dynamic Brownian bridge and the corridors of part 7, the steps of
  a step selection function in part 5): a track with a rhythm that changes means steps that are not comparable.
- A track sampled finer than the question needs gives **locations that are not independent** and exaggerates the use of
  the places where the animal rested; part 6 shows the home range changing with the interval.
- The **measure of an error** is the speed, and over a very short lag it is the error divided by a few seconds: thin
  before screening if the errors are of the order of the step.

[Part 2](../cleaning-and-measures/) already had [[move.filter_per_interval]], which keeps one location per interval of the
calendar (the first fix after :00 and the first after :30, say). It is the quickest, but its locations are not a regular
interval apart: two kept fixes can be seconds or two intervals apart. This part adds two that are:

- [[move.thin_time]] (move2utils' `mt_thin_time`): the longest chain of locations whose successive time lags are the
  interval \( \pm \) a tolerance, found by dynamic programming. A gap longer than the interval plus the tolerance
  splits the track and is never bridged.
- [[move.thin_distance]] (`mt_thin_distance`): the locations are about a distance of *travel* apart, so that the animal at
  rest is not oversampled and the animal in a hurry is not undersampled.

## When to use it

- **Before any measure** that a few errors spoil: the distance travelled, the speed, the area of a minimum convex polygon,
  a step selection function (which takes the habitat at the end of the step).
- **Before** the dynamic Brownian bridge, whose variance takes an error for movement ([part 6](../home-ranges/)), and the
  corridors ([part 7](../corridors-and-validation/)).
- **To compare** animals or studies at one interval.

What it is *not*: a filter finds the errors that its rule describes. A jump of 600 m in two minutes is a speed of 5 m/s,
possible for a fisher, so the speed filter lets it through (below). The rule of the cleaning is a decision for a report: say
which filters, with which settings, removed how many locations.

## What you need

A Movebank file with the columns of the GPS: `gps:satellite-count`, `gps:dop` and, for e-obs collars,
`eobs:horizontal-accuracy-estimate` (the nodes also find them with underscores or dashes). A filter on a column that the
file lacks is skipped **without a word**, so check which columns you have. The speeds need the times, and, for the
mass-based limit, the body mass of the animal (kilograms) and how it moves (flying, running, swimming).

Download the track with the errors: [`m4-with-errors.csv`](/data/tutorials/animal-movement/cleaning-and-thinning/m4-with-errors.csv).
It is M4's tracks of the first part, with the columns `gps:satellite-count` and `gps:dop` (simulated: 5 to 12 satellites and
a DOP of 0.8 to 8 for a good fix), and `injected`, that says which fixes were spoiled, in three kinds:

| `injected` | How many | What was done |
|---|---|---|
| jump, poor fix | 30 | The position moved 300 to 5,000 m in a random direction, and the fix given 3 or 4 satellites or a DOP of 12 to 30 |
| jump, good-looking fix | 30 | The position moved 200 to 3,000 m, and the fix left with the quality of a good one |
| poor fix, right position | 20 | The position left as it was, and the fix given 3 or 4 satellites or a DOP of 11 to 20 |

The first kind is a GPS that knows it is wrong, the second a GPS that does not, and the third a GPS that is pessimistic:
a filter on quality will lose these twenty, though their positions are right. The template is **Cleaning a track**.

## Build the model

### Quality, limit and speed

```tfgraph
#! caption: The track with errors read (node 1), the fixes of too few satellites or too high a dilution of precision dropped (node 2), the speed limit of the animal (node 3), the locations next to a step faster than it flagged (node 4) and the flagged ones removed (node 5).
#! numbered: true
#! highlight: quality, limit, speed, clean
format = "terraflux/1"

[[nodes]]
id = "read"
op = "move.read_tracks"
params = { path = "m4-with-errors.csv" }

[[nodes]]
id = "quality"
op = "move.filter_gps_quality"
inputs = { tracks = "read.tracks" }
params = { sat_min = 5, dop_max = 10 }
note = "At least 5 satellites and a DOP of 10 at most"

[[nodes]]
id = "limit"
op = "move.max_speed"
params = { mass = 4.5, mode = "running" }
note = "The fastest a running animal of 4.5 kg can go, with its interval"

[[nodes]]
id = "speed"
op = "move.flag_speed"
inputs = { tracks = "quality.tracks" }
params = { mass = 4.5, mode = "running" }
note = "Both ends of every step faster than that are flagged"

[[nodes]]
id = "clean"
op = "vector.filter"
inputs = { vector = "speed.tracks" }
params = { expression = "is_outlier == 0" }
note = "The track without them"
```

<div class="steps" markdown>

1. **Read the track** with [[move.read_tracks]] (node 1).

2. **Drop the unreliable fixes** with [[move.filter_gps_quality]] (node 2). `sat_min` is the fewest satellites (5 by
   default), `dop_max` the highest dilution of precision (10) and `hacc_max` the largest horizontal accuracy estimate in
   metres (100). Each takes `none` to leave that criterion out. The columns are Movebank's names (`gps:satellite-count`,
   `gps:dop`, `eobs:horizontal-accuracy-estimate`), also with underscores or dashes; **a column the file lacks skips its
   criterion silently**. A value that is missing does not drop a location, and the locations without a position
   (`drop_empty`, true) go too, since nothing can score them. This is move2utils' `mt_filter_gps_quality`.

3. **Find the limit** with [[move.max_speed]] (node 3), that needs no tracks: the body mass in kilograms (`mass`), the
   `mode` (`flying`, the default, `running` or `swimming`) and the level of the interval (`ci_level`, 0.95). It gives
   `v_max` in m/s, its `lower` and `upper` ends (the uncertainty of the fit, by the delta method), `kmh`, and a `table`.
   A fisher is 4.5 kg here: a round number inside what adult males weigh, since the file does not give the mass. The
   node warns about a mass outside the data of Hirt et al. (3e-8 to 1.084e5 kg). Pick the mode of the animal's *fast*
   movement: a bat flies, a fisher runs.

4. **Flag the fast steps** with [[move.flag_speed]] (node 4). The speed of a step is its length over its time lag in
   metres per second (the great-circle length for longitude and latitude, the coordinates' units over seconds
   otherwise). A location is flagged when the step *leaving* it or the step *arriving* at it is faster than the limit;
   steps with a zero or negative lag have no speed. The limit is `v_max` (a speed of your own) or `mass` with `mode`, which
   gives the same number as node 3 and adds no margin; `jitter` also flags the locations next to steps shorter than it
   (metres), the noise floor of the device. The node adds `is_outlier` (any flag), `step_speed`, `is_speed_above_cap`,
   `is_step_below_jitter` (with `jitter`), `flagged_by_speed` and `loglr_speed`, and the limit used goes out as `v_max`.
   With `remove` true it gives only the locations that were not flagged. **The locations without a position must be dropped
   first** (node 2 does it), and the quality filtered before the speeds, since the bad fixes make the false jumps.

5. **Keep the track that is left** with [[vector.filter]] (node 5): `is_outlier == 0`. Or set `remove = true` in node 4.

</div>

On the whole of M4 with the errors, node 2 drops 50 fixes (30 poor jumps and 20 poor fixes with a right position), and
node 4 flags 27 more; 8,881 of the 8,958 locations are left.

### Thin

```tfgraph
#! caption: The clean track thinned to one location about every 10 minutes (node 6), and to one about every 100 m of travel, by the greedy walk (node 7) and by a chain of lengths 100 m ± 10 (node 8).
#! fragment: true
#! highlight: thin, by_distance, by_interval
[[nodes]]
id = "thin"
op = "move.thin_time"
inputs = { tracks = "clean.vector" }
params = { interval = "10 mins" }

[[nodes]]
id = "by_distance"
op = "move.thin_distance"
inputs = { tracks = "clean.vector" }
params = { distance = 100 }

[[nodes]]
id = "by_interval"
op = "move.thin_distance"
inputs = { tracks = "clean.vector" }
params = { distance = 100, method = "interval" }
```

[[move.thin_time]] takes the `interval` (seconds, or a text such as `45 mins` or `2 hours`) and a `tolerance` of the same
form (a tenth of the interval by default, at most the interval). Among chains of the same length, `criterion` `closest`
(the default) takes the one with the least total deviation from the interval, `first` the one that ends first. The
locations need not be sorted; those without a time are never kept; the geometry is not read. [[move.thin_distance]] adds
the distances of the steps in metres whatever the coordinate system, and with `method` `step` (the default) keeps the
first location and then each one at which the travel since the last kept reaches `distance`; with `interval` it keeps the
longest chain whose successive travel is `distance` \( \pm \) `tolerance`. Both give the locations kept (`output` selected,
the default), or all of them with the column `thin_selected` (`flag`).

### In Python

```python
import numpy as np
import terraflux as tf

tracks = tf.load_tracks("m4-with-errors.csv")                 # the file of this page
print(len(tracks), tracks["injected"].fillna("").value_counts().to_dict())

limit = tf.move.v_phys_estimate(4.5, "running")               # Hirt et al. (2017)
print(round(limit.v_max, 2), round(limit.lower, 2), round(limit.upper, 2), "m/s;", round(limit.kmh, 1), "km/h")

quality = tf.move.filter_gps_quality(tracks)                  # sat_min 5, dop_max 10
flagged = tf.move.flag_speed(quality, mass=4.5, mode="running")
clean = flagged[~flagged["is_outlier"].astype(bool)].reset_index(drop=True)
print(len(tracks) - len(quality), "dropped by the quality,", int(flagged["is_outlier"].sum()), "flagged by the speed;",
      len(clean), "left")
print(round(flagged.attrs["v_max_used"], 2), "m/s;", int(flagged["is_speed_above_cap"].sum()), "steps over it")
```

```text
8958 {'': 8878, 'jump, poor fix': 30, 'jump, good-looking fix': 30, 'poor fix, right position': 20}
10.47 9.77 11.17 m/s; 37.7 km/h
50 dropped by the quality, 27 flagged by the speed; 8881 left
10.47 m/s; 27 steps over it
```

```python
# thin: filter_thin_time and filter_thin_distance give the locations, thin_time and thin_distance a True / False each
ten = tf.move.filter_thin_time(clean, "10 mins")
lag = ten["timestamp"].diff().dt.total_seconds().to_numpy()[1:] / 60
print(len(ten), "locations; lags within 10 min +- 1:", round(float((abs(lag - 10) <= 1).mean()) * 100, 1), "%")
by_step = tf.move.filter_thin_distance(clean, 100)
by_chain = tf.move.filter_thin_distance(clean, 100, "interval")
print(len(by_step), len(by_chain))
```

```text
2202 locations; lags within 10 min +- 1: 71.1 %
2601 1656
```

`tf.move.gps_quality(tracks)` gives the True / False of each location (`filter_gps_quality` applies it), and
`flag_speed(..., remove=True)` the track without the flagged. The functions take `n_threads`; the result does not depend on it.

## Read the results

### What each filter caught

```tftable
data: /data/tutorials/animal-movement/cleaning-and-thinning/detection.csv
digits: 0
caption: What was put into M4's track, and what each filter did to it: the 8,958 locations, 80 of them spoiled.
```

The quality filter caught every error that looked like one: the 30 jumps with poor fixes, and, as a cost, the 20 poor
fixes whose positions were right. Of the 30 jumps that looked good, the speed limit caught 10, and 20 stay in the
track. To the speed filter's tally of 27 flagged locations, 17 are good fixes: the neighbours of the bad ones.

![The speed of each step against the limit](/img/tutorials/animal-movement/cleaning-and-thinning/speeds.png "The speed of every step of M4's track after the quality filter, on log scales, against the limit of a 4.5 kg running animal (10.5 m/s, 38 km/h). In orange, the steps to or from the 30 fixes that were spoiled but looked good; most are below the limit.")

The reason is in the figure. The speeds of the untouched fixes top out just under the limit (the fastest of M4's
original track is 9.8 m/s), and the spoiled steps are spread from a few millimetres per second to 60 m/s: only those over 10.5 m/s, 10 of the 30,
can be told from the animal. A jump of 200 to 1,250 m in the two minutes between fixes implies a speed
under the limit. **The speed limit is a ceiling, and it catches the errors that are larger than the animal could move in
the interval**; the smaller ones stay in.

![Hirt et al.'s law for flying, running and swimming animals](/img/tutorials/animal-movement/cleaning-and-thinning/hirt.png "The maximum speed against body mass for the three modes of locomotion (move.max_speed): the line is the estimate and the band its interval (the uncertainty of the fit). The point is M4's limit.")

### What the errors cost, and what is left

```tftable
data: /data/tutorials/animal-movement/cleaning-and-thinning/stages.csv
digits: 1
caption: M4's track as recorded, with the 80 errors, and after each filter: the locations left, the distance travelled, the fastest step and the minimum convex polygons at 100 % and 95 %.
```

![M4's track with the errors and after the two filters](/img/tutorials/animal-movement/cleaning-and-thinning/cleaned.png "M4's track with the errors put in (left; the fixes that were spoiled are coloured) and after the quality filter and the speed limit (right; the spoiled fixes that stay in the track are orange). Coordinates in kilometres from the centre of the track.")

The errors put 181 km on the track's length, 48 % more than the true 376 km, and a polygon of all the locations with 40.8 km²
that should be 11.7. The quality filter removes most of it (445 km, 17.3 km²); the speed limit more (404 km, 15.4 km²). That
leaves 28 km (7 %) more distance and a polygon 3.8 km² larger than the truth from the 20 jumps that no filter caught, each
adding twice its length to the track. The 95 % polygon, which discards the farthest 5 % of the fixes, is less sensitive,
but still 10.5 km² against 8.8.

This is the lesson of the table: **cleaning reduces the damage and does not remove it**. Report the distance as "the
sum of the steps of the cleaned track, thinned to 10 minutes", and prefer measures that do not depend on a single fix
(the 95 % polygon, a kernel density, the median speed) when the errors cannot be found.

### Choosing the limit

```tftable
data: /data/tutorials/animal-movement/cleaning-and-thinning/limit.csv
digits: 1
caption: The speed limit at the ends of Hirt et al.'s interval, 25 % above their estimate (a margin for real sprints) and three times it: the locations flagged, how many of them are errors and how many are untouched fixes, and the errors left in the track (after the quality filter).
```

The limit has a flat effect between the two ends of Hirt et al.'s interval (9.8 to 11.2 m/s): 35 to 26 locations flagged, 10
to 13 of them errors. Adding 25 %, which move2utils suggests for animals whose sprints are real, loses little (9 errors found
instead of 10); three times the estimate is no limit at all (3 errors found). The best protection against a limit that
is too low is not a larger limit but the published top speed of your species (`v_max`), when there is one.

### Thinning in time

```tftable
data: /data/tutorials/animal-movement/cleaning-and-thinning/thin-time.csv
digits: 1
caption: M4's clean track thinned to 10 minutes by the clock (move.filter_per_interval) and by chains of lags of 10 minutes ± 1 and ± 3 (move.thin_time): the locations kept and what the lags between them are.
```

![The time between the locations kept by each thinning](/img/tutorials/animal-movement/cleaning-and-thinning/thin-lags.png "The time between consecutive locations kept by one per 10-minute interval of the clock and by move.thin_time with a tolerance of 1 and of 3 minutes. The last class is 40 minutes and more: the gaps of the track.")

The clock thinning keeps 2,938 locations, and the lags between them range from under a minute to twice the interval (4.6 % are
under 5 minutes). A chain with a tolerance of 1 minute keeps 2,202, with lags of 9 to 11 minutes except where the track has a
gap, which the chain never bridges (29 % of the steps have a longer lag, 258 of them over an hour); it is the one to use for
steps that have to be comparable. A tolerance of 3 minutes
keeps more (3,091) and the lags spread over 7 to 13 minutes: *the tolerance is how irregular the thinned track may
be*. The number of locations kept is not a measure of quality; the lags are. A tolerance too narrow for the jitter of the
fixes splits the track into short chains: widen it before you accept the loss.

### Thinning in distance

```tftable
data: /data/tutorials/animal-movement/cleaning-and-thinning/thin-distance.csv
digits: 1
caption: One night of M4 (noon on 5 March 2010 to noon on the 6th, local time): the locations kept by each thinning, and the lengths and time lags of the steps between them.
```

![One night of M4 thinned in time and in distance](/img/tutorials/animal-movement/cleaning-and-thinning/thin-distance.png "One night of M4: all its locations, one about every 30 minutes (move.thin_time), one about every 100 m of travel by the greedy walk (move.thin_distance, step) and by a chain of 100 m ± 10 (interval).")

A time thinning samples the track evenly in time, and draws a night of steps of 1 to 916 m. A distance thinning draws the
route more evenly: 82 locations of steps of 14 to 513 m by the greedy walk, 58 of 8 to 527 m by the chain; where the
animal rested for an hour its two-minute fixes collapse into one, and where it travelled they stay. Use it when the
question is about the route (how the track follows a river, how a step crosses a road), time thinning when it is about
the rhythm (a home range, speeds, a step selection function).

## Settings that change the answer

- **`sat_min`, `dop_max`, `hacc_max`**: stricter limits drop more good fixes with the bad. Look at the distribution of the
  column and at the speeds of the fixes in the margin before choosing.
- **The limit**: from the estimate, from `v_max` of the species, or from the estimate plus a margin; and the mode.
- **`jitter`**: the noise floor of the device. A resting animal's fixes scatter by a few metres, and a step shorter than
  that is not movement.
- **The interval and the tolerance** of the thinning, which fix both how many locations stay and how regular they are.

## Common pitfalls

- **A column the file lacks.** [[move.filter_gps_quality]] skips a criterion whose column is missing, without a word, and
  a file with none of them only loses its empty locations. Look at the table it gives (the locations that remain).
- **The order.** The quality before the speeds (the bad fixes make the false jumps), the empty locations out before
  both, and the thinning *after* the screening: thinning first may keep the bad fix and lose its neighbours.
- **Both ends flagged.** A fast step flags the fix before it and after it, so one error costs a good location. With a track
  at a short interval the cost is small; at a long interval, the neighbours are a larger share of the track.
- **A limit from the track.** A limit that is a quantile of the track's own speeds finds the fastest of every track, errors or not.
  The limit comes from the animal.
- **Too long an interval for the limit.** A jump is as fast as the interval is short; over several hours a bad fix is
  at a speed any animal could do. Screen at the finest interval you have, then thin.
- **The wrong mode.** A flying animal's limit (at 4.5 kg, 31.7 m/s) is three times a running one's (10.5 m/s), and the swimming
  one (5.4 m/s) is smaller yet: `move.max_speed` defaults to `flying`.
- **Cleaning away the animal.** A species with long bursts of real speed (a racing pigeon) loses its fastest real moves to a
  limit that is too low: use `v_max` from its biology. Nothing here is automatic: move2utils' tools that choose the limit
  from the data, and its cascade of detectors for outliers, are not in Terraflux (they come from a preprint, still under review).
- **Report it.** The number of locations dropped by each filter and the settings belong in the methods.

## What the operators and the library agree on

The operators and the library (`tf.move`) were run on the same track: after the quality filter 8,908 locations,
27 flagged by the speed and 8,881 left, the same locations in the same order. Against move2utils (0.4.6;
Kranstauber et al., 2026): `mt_filter_gps_quality`, `mt_flag_speed_cap` with a hard cap, `v_phys_estimate`, `mt_thin_time` and
`mt_thin_distance` give the same results, bit for bit, on the tracks the tests use (the fishers and move2utils' own
synthetic tracks with known outliers). Two mistakes of move2utils are corrected, and the tests compare with its code
corrected: `mt_filter_gps_quality` does not find Movebank's own column names (`gps:satellite-count`, `gps:dop`,
`eobs:horizontal-accuracy-estimate`; only `gps_...` and `gps-...`), so on a Movebank file it filters nothing; and
`mt_flag_speed_cap` loses the columns of a track with a single location to score and then fails to bind the tracks of a
set that has one. What is not here is the rest of its cleaning: the automatic limits, the cascade of detectors,
`mt_clean_track`.

## The template

**Cleaning a track** (library subject **Animal movement**, group *Movement*) builds this part's models on two simulated
tracks into which it puts bad fixes: the quality filter, the limit from a body mass and the speed filter with the locations
removed, and the thinning to an hour, with the tracks before and after. Replace the simulation by [[move.read_tracks]] of
your own file; the mass and the mode of the template (a 5 kg flying animal) are the example's: set yours.

## Going further

- [Clean tracks and measure them](../cleaning-and-measures/): the cleaning with Movebank's flags and the measures of a
  step, from which this part goes on.
- [Home ranges and utilisation distributions](../home-ranges/): the errors a home range hates, and the Brownian bridge,
  which needs a regular interval.
- [Observed corridors and validating connectivity](../corridors-and-validation/): corridors, which suppose it too.
- The operators' reference: [[move.filter_gps_quality]], [[move.max_speed]], [[move.flag_speed]], [[move.thin_time]],
  [[move.thin_distance]], [[move.filter_per_interval]].

## References

- Frair, J. L., Fieberg, J., Hebblewhite, M., Cagnacci, F., DeCesare, N. J., & Pedrotti, L. (2010). Resolving issues of
  imprecise and habitat-biased locations in ecological analyses using GPS telemetry data. *Philosophical Transactions of
  the Royal Society B*, 365(1550), 2187-2200. https://doi.org/10.1098/rstb.2010.0084
- Hirt, M. R., Jetz, W., Rall, B. C., & Brose, U. (2017). A general scaling law reveals why the largest animals are not the
  fastest. *Nature Ecology & Evolution*, 1(8), 1116-1122. https://doi.org/10.1038/s41559-017-0241-4
- Kranstauber, B., Safi, K., & Scharf, A. K. (2026). move2utils: a utility toolkit for the move2 ecosystem. *bioRxiv*.
  https://doi.org/10.64898/2026.07.07.736908

In Python: [Animal movement in Python](/tutorials/animal-movement/animal-movement-in-python/).
