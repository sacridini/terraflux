---
title: Regular steps and track summaries
short: Steps and summaries
description: Resample irregular tracks to a regular interval in bursts, as amt does, turn them into steps with lengths and turn angles, tell day from night by the sun, and summarise each track by its distances, straightness, sinuosity, spread and net squared displacement.
order: 40
level: intermediate
time: 50 min
ops: [move.resample, move.filter_min_n_burst, move.steps, move.time_of_day, move.nsd, move.track_summary]
series: Animal movement
part: 4
---

[Part 2](../cleaning-and-measures/) showed that every measure of a track depends on its sampling interval, and
that the fishers were sampled at three different ones, irregularly. In this part you will make the intervals
regular without inventing any location, as amt does: by keeping the fixes that fall on a regular grid of times
and cutting the track into **bursts** where they do not. From the bursts you will build **steps**, the unit of
amt's analyses, label each step as day, night, dawn or dusk from the position of the sun, follow each animal's
net squared displacement over the weeks, and summarise each track in a few numbers. At the end you will have the
fishers' steps at a common 10-minute interval, ready for the step selection functions of
[part 5](../step-selection/), and a table of their tracks that you can report.

## The idea

### Resampling into bursts

An irregular track can be made regular in two ways. [Part 3](../projections-and-interpolation/) placed
locations at regular times by interpolation, inventing them. **Resampling** keeps only real fixes: it walks
along the track and keeps a fix when it falls at the chosen interval after the last one kept, within a
tolerance. When no fix is close enough, the regular sequence is broken, and a new one, a new burst, begins at
the next fix.

!!! science "Key idea: amt's track_resample"
    With a rate \( r \) and a tolerance \( \tau \), from the first location of a track:

    1. the next location kept is the first one at least \( r - \tau \) after the last one kept;
    2. if it is at most \( r + \tau \) after it, it belongs to the same **burst**; otherwise it starts a new one;
    3. the locations in between are dropped.

    Every step within a burst then lasts \( r \pm \tau \). A step between bursts is not a step at all: amt
    never builds one. A burst of one or two locations gives no turn angle, so bursts are usually kept only from
    three locations on ([[move.filter_min_n_burst]]).

Signer et al. (2019) made this the first step of amt's workflow, because the step selection functions of part 5
compare steps by their lengths and turns, which must all have the same duration.

### Steps

A **step** in amt is a line from one location to the next of the same burst, with its start (`x1_`, `y1_`), end
(`x2_`, `y2_`), length (`sl_`), direction (`direction_p`), turn angle from the step before (`ta_`), start and
end times (`t1_`, `t2_`) and duration (`dt_`). It differs from move2's measures of part 2 in two ways: the
step is a feature of its own (a line), not a column of a location; and its direction is the angle of
mathematics, anticlockwise from east, in radians, not the azimuth clockwise from north. The turn angle is the
same quantity in both, but its sign follows the convention: positive to the left in amt (anticlockwise),
positive to the right in move2.

### Day and night

Most animals are not equally active around the clock. amt labels each location or step by the sun: **day**
between sunrise and sunset, **night** between sunset and sunrise, and, if asked, **dawn** and **dusk**, the
civil twilights when the sun is less than 6° below the horizon. The times of the sun come from the date and the
location, computed as the R package suncalc computes them (sunrise and sunset when the sun's upper edge is at
-0.833°, refraction included).

### Summaries of a track

!!! science "Key idea: summarising a track"
    For a track of \( n \) locations \( p_1, \dots, p_n \) with step lengths \( l_i \) and turn angles
    \( \theta_i \):

    - **cum_dist**, the distance travelled: \( \sum l_i \).
    - **tot_dist**, the net distance: \( |p_n - p_1| \), from the first location to the last.
    - **straightness** (Batschelet, 1981): \( \text{tot\_dist} / \text{cum\_dist} \), 1 for a straight line,
      near 0 for an animal that comes back to where it started.
    - **msd**, the mean squared distance of the locations from their centroid, a measure of the spread of the
      locations (the square of the radius of gyration).
    - **sinuosity** (Benhamou, 2004): \( S = 2 \left[ p \left( \frac{1 + c}{1 - c} + b^2 \right) \right]^{-1/2} \),
      where \( p \) is the mean step length, \( c \) the mean cosine of the turn angles and \( b \) the
      coefficient of variation of the step lengths. It estimates the tortuosity of a random search path, in units
      of 1 / √length; higher is more tortuous.
    - **tac**, the turn angle correlation (Dray et al., 2010): the mean squared difference between consecutive
      turns on the unit circle, \( \frac{1}{n} \sum (\cos\theta_{i+1} - \cos\theta_i)^2 +
      (\sin\theta_{i+1} - \sin\theta_i)^2 \). It is 0 when every turn is the same, 2 when consecutive turns are
      independent and uniform, and up to 4 when they alternate.
    - **NSD**, the net squared displacement of each location: \( |p_i - p_1|^2 \), the squared distance from
      the first location (Bunnefeld et al., 2011).

Straightness suits directed movement (a migration, a dispersal); it is near 0 for any animal living in a home
range, whose last location is near its first. Sinuosity suits searching movement within an area. NSD over time
tells movement patterns apart: it stays within a band for a resident animal, rises and levels off for one that
disperses and settles, rises and falls back for a migrant that returns.

## When to use it

- **Resample** before any analysis that compares steps: step selection functions (part 5), the distributions of
  step lengths and turns, comparisons of animals or seasons. Choose the rate from the collars' schedule (the
  histogram of time lags of part 2): at least the scheduled interval, a multiple of it to compare animals on
  different schedules.
- **Time of day** to compare activity, or as a variable of a step selection function (do animals use roads at
  night and avoid them by day?).
- **NSD** to classify individuals as resident, dispersing, migrating or nomadic.
- **Summaries** for a table of the tracks in a paper, and to compare individuals at a common interval.

## What you need

The cleaned tracks of part 2, projected on the azimuthal equidistant projection of part 3 ([[move.aeqd_crs]]),
so that steps have lengths in metres and the random steps of part 5 can be drawn. [[move.steps]] also works in
longitude and latitude (lengths in metres on the sphere, `lonlat = true`), but the random steps need projected
tracks.

## Build the model

```tfgraph
#! caption: The projected tracks (node 5) resampled every 10 minutes with a tolerance of 1 minute (node 6), the bursts of at least 3 locations (node 7), their steps (node 8) and the time of day at the end of each step (node 9).
#! numbered: true
#! highlight: resample, bursts, steps, tod
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

[[nodes]]
id = "resample"
op = "move.resample"
inputs = { tracks = "aeqd.tracks" }
params = { rate = "10min", tolerance = "1min" }

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
id = "tod"
op = "move.time_of_day"
inputs = { tracks = "steps.steps" }
params = { include_crepuscule = true, where = "end" }
```

<div class="steps" markdown>

1. **Resample** with [[move.resample]] (library subject **Animal movement**), connected to `aeqd.tracks`.
   `rate` and `tolerance` take seconds (`600`) or a text with a unit (`10min`, `2h`, `30s`). The node keeps the
   locations of the regular sequence and adds the column `burst_`, the number of the burst within each track
   (1, 2...). `start` (0 by default) is the location each track starts from, counted from 0. Ten minutes suits
   both the 10-minute collars and the 2-minute ones, of which it keeps every fifth fix.

2. **Keep the bursts of 3 locations or more** with [[move.filter_min_n_burst]] (`min_n = 3`). A burst of two
   locations is one step with no turn angle; most analyses drop them.

3. **Build the steps** with [[move.steps]]: one line per step within each burst, with the columns described
   above. `keep_cols` copies the tracks' other columns onto the steps: those of the location the step starts at
   (`start`), ends at (`end`), or both (`both`, with the suffixes `_start` and `_end`). Steps never join two bursts.

4. **Label the time of day** with [[move.time_of_day]]. On steps, `where` chooses the end (the default, as amt)
   or the start of each step; `include_crepuscule = true` adds dawn and dusk to day and night. The node adds the
   column `tod` (or the name in `column`).

</div>

The summaries and the NSD read the tracks, not the steps:

```tfgraph
#! caption: The net squared displacement of every location (node 10), and the summaries of each track at 10 minutes (node 11).
#! fragment: true
#! highlight: nsd, summary
[[nodes]]
id = "nsd"
op = "move.nsd"
inputs = { tracks = "aeqd.tracks" }

[[nodes]]
id = "summary"
op = "move.track_summary"
inputs = { tracks = "bursts.tracks" }
```

[[move.nsd]] adds the column `nsd` to every location, in the squared units of the coordinates: square metres
here, square degrees on tracks in longitude and latitude (as amt), which is one more reason to project first.
[[move.track_summary]] gives one line per track (the track's line of [[move.track_lines]], with its number of
locations and its constant columns) with the columns `cum_dist`, `tot_dist`, `straightness`, `msd`, `sinuosity`
and `tac`.

### In Python

The model's nodes, one function each, on the projected tracks of part 3. [`tf.move.resample`][terraflux.move.resample]
takes `rate` and `tolerance` as seconds, as text (`"10min"`, `"2h"`, `"30s"`) or as timedeltas, and `start`; it adds the
column `burst_`. [`tf.move.steps`][terraflux.move.steps] works in longitude and latitude too (`lonlat=True` is the
default for a geographic CRS), and `keep_cols` copies the tracks' other columns onto the steps.

```python
import numpy as np
import pandas as pd
import terraflux as tf

pd.set_option("display.width", 120, "display.max_columns", 10)
ID = "individual-local-identifier"
tracks = tf.load_tracks(tf.move.example_data("fishers"))
tracks = tf.move.filter_unique(tf.move.filter_movebank_visible(tracks))
tracks = tracks[~tracks.geometry.is_empty].reset_index(drop=True)
local = tracks.to_crs(tf.move.aeqd_crs(tracks))                 # metres, so that the steps have lengths in metres

resampled = tf.move.resample(local, "10min", "1min")            # node 6
bursts = tf.move.filter_min_n_burst(resampled, 3)               # node 7: min_n = 3
steps = tf.move.steps(bursts)                                   # node 8
print(len(resampled), len(bursts), len(steps), "steps of", steps[ID].nunique(), "animals")
print(steps.columns.tolist())
print(steps[["sl_", "ta_", "direction_p", "dt_"]].head(3))
```

```text
12833 8641 7480 steps of 7 animals
['individual-local-identifier', 'burst_', 'x1_', 'x2_', 'y1_', 'y2_', 'sl_', 'direction_p', 'ta_', 't1_', 't2_', 'dt_']
         sl_       ta_  direction_p                    dt_
0  13.708823       NaN     0.497441 0 days 00:09:47.003000
1  83.809351  2.793589    -2.992155 0 days 00:10:14.997000
2  11.817641       NaN    -1.514482 0 days 00:09:16.002000
```

`direction_p` is anticlockwise from east, in radians, and `ta_` positive to the left (amt's conventions, unlike
`tf.move.azimuth` and `tf.move.turn_angle`). The first step of each burst has no turn angle. The rate decides which
animals have steps and which hours of the track they come from:

```python
rows = []
for rate, tol in [("2min", "20s"), ("10min", "1min"), ("30min", "3min"), ("1h", "6min"), ("2h", "12min")]:
    res = tf.move.resample(local, rate, tol)
    st = tf.move.steps(tf.move.filter_min_n_burst(res, 3))
    rows.append({"rate": rate, "tolerance": tol, "locations": len(res),
                 "bursts": int(res.groupby(ID)["burst_"].nunique().sum()), "steps": len(st),
                 "animals": st[ID].nunique(), "M4's median step (m)": st.loc[st[ID] == "M4", "sl_"].median()})
print(pd.DataFrame(rows).round(1).to_string(index=False))
```

```text
 rate tolerance  locations  bursts  steps  animals  M4's median step (m)
 2min       20s      30179   14504  14686        5                  28.4
10min      1min      12833    4783   7480        7                  76.8
30min      3min       5838    2096   3577        8                 166.6
   1h      6min       3678     694   2877        8                  80.7
   2h     12min       1991     457   1466        8                 144.9
```

[`tf.move.time_of_day`][terraflux.move.time_of_day] labels locations or steps (`where="end"`, as amt, or `"start"`)
from the sun's position, with the sunrise of the location's date in UTC; `include_crepuscule=True` adds dawn and dusk.
It returns a categorical, one label per step.

```python
steps["tod"] = tf.move.time_of_day(steps, include_crepuscule=True)
by = steps.groupby("tod", observed=True)["sl_"]
table = pd.DataFrame({"steps": by.size(), "share (%)": 100 * by.size() / len(steps),
                      "median step (m)": by.median(), "mean step (m)": by.mean()})
print(table.loc[["day", "dusk", "night", "dawn"]].round(1))
```

```text
       steps  share (%)  median step (m)  mean step (m)
tod                                                    
day     1553       20.8             22.6           55.8
dusk      46        0.6             24.7           62.0
night   5661       75.7             85.0          123.6
dawn     220        2.9             65.3          117.5
```

The summaries and the NSD are one function each, by animal ([`tf.move.nsd`][terraflux.move.nsd] gives one value per
location; the others a Series by track). The node [[move.track_summary]] is these six on the same tracks, so they take
the resampled tracks, with `burst_` and all:

```python
nsd = pd.Series(tf.move.nsd(local), index=local.index)           # node 10: squared units of the CRS
print((nsd.groupby(local[ID]).max() / 1e6).round(1).to_dict(), "km2 at most")

summary = pd.DataFrame({                                         # node 11
    "locations": bursts.groupby(ID).size(),
    "cum_dist (km)": tf.move.cum_dist(bursts) / 1000,
    "tot_dist (km)": tf.move.tot_dist(bursts) / 1000,
    "straightness": tf.move.straightness(bursts),
    "msd (km2)": tf.move.msd(bursts) / 1e6,
    "sinuosity": tf.move.sinuosity(bursts),
    "tac": tf.move.tac(bursts),
})
print(summary.round(3))
```

```text
{'F1': 4.8, 'F2': 7.2, 'F3': 2.7, 'M1': 33.1, 'M2': 69.4, 'M3': 13.4, 'M4': 12.1, 'M5': 11.8} km2 at most
                             locations  cum_dist (km)  tot_dist (km)  straightness  msd (km2)  sinuosity    tac
individual-local-identifier                                                                                    
F1                                1300        114.220          0.716         0.006      0.954      0.106  1.763
F2                                 575        109.580          1.408         0.013      1.285      0.096  1.806
F3                                 300         41.696          0.379         0.009      0.214      0.112  1.944
M2                                1520        173.790          4.461         0.026      3.937      0.089  1.795
M3                                 385         89.666          0.895         0.010      1.177      0.077  1.832
M4                                1801        225.495          1.461         0.006      1.806      0.111  1.880
M5                                2760        371.118          1.301         0.004      1.496      0.107  1.757
```

M1 has no bursts at 10 minutes and so no row, as in the table. On tracks in longitude and latitude `tf.move.nsd` gives
squared degrees, as amt does: project first, as above. The same functions on the raw tracks (every fix, no
resampling) give other, more tortuous numbers.

## Read the results

### Bursts

![M4's bursts of one day at 10 minutes](/img/tutorials/animal-movement/steps-and-summaries/bursts.png "One day of M4 (noon to noon, 5 to 6 March 2010) resampled every 10 minutes with a tolerance of 1 minute, each burst in its own colour, over all its locations (grey). The day's 248 locations become 76, in 25 bursts: wherever the collar missed the 10-minute mark by more than a minute, a burst ends.")

Resampling M4's day from part 1 shows what the method keeps. Of 248 locations, 76 fall on the 10-minute grid;
they form 25 bursts, many of them short, because the collar's fixes drifted around its schedule and failed at
times. The long bursts are in the night, when the fisher travelled and the collar fixed well. Resampling did not
invent a single location: every coloured point is a fix.

How much survives depends on the rate:

```tftable
data: /data/tutorials/animal-movement/steps-and-summaries/rates.csv
digits: 1
caption: The fishers resampled at five rates, each with a tolerance of a tenth of it: the locations kept, the bursts, the steps in bursts of 3 locations or more, the animals that have any, and M4's median step length.
```

At 2 minutes only the five animals on that schedule have steps; at 10 minutes M1, fixed every 15, has none (its
fixes are never 9 to 11 minutes apart); from 30 minutes on all eight have. The rate is therefore also a choice
of *which animals* enter the analysis. And of *which parts of a track*: M4's median step does not grow steadily
with the rate (28, 77, 167, 81 and 145 m), because the steps that fit each grid come from different hours: 9 % of
its 30-minute steps start between 07:00 and 17:00 local time, 31 % of its hourly ones, when it rested and its
collar fixed sparsely. A resampled track keeps the periods that happen to fit the
grid, so check that they are not a biased sample (all night, or all from one week).

### Step lengths and turns

![The step lengths at 10 minutes](/img/tutorials/animal-movement/steps-and-summaries/lengths.png "The length of the 7,480 steps of 10 minutes of seven fishers. Most are short (half are under 59 m), a long tail reaches over a kilometre.")

![The turn angles at 10 minutes](/img/tutorials/animal-movement/steps-and-summaries/turns.png "The turn angles of the same steps (ta_, in degrees). Peaks at 0° (straight on) and at ±180° (reversals).")

Steps of a regular 10 minutes have the shape amt's analyses expect: lengths skewed to the right, with many short
steps (resting, lingering) and a long tail of travel; turns concentrated at 0° and at ±180°. Part 5 fits
distributions to exactly these two histograms: a gamma distribution to the lengths, a von Mises distribution to
the turns.

### Night owls

![The mean step length by hour of the day](/img/tutorials/animal-movement/steps-and-summaries/time-of-day.png "The mean length of the 10-minute steps by the local hour at which they start (UTC - 5), coloured by whether most of the hour's steps end at night or by day.")

```tftable
data: /data/tutorials/animal-movement/steps-and-summaries/time-of-day.csv
digits: 1
caption: The 10-minute steps by the time of day at their end (move.time_of_day with include_crepuscule): how many, and their lengths.
```

The fishers are nocturnal in these data: their steps average about 120 m per 10 minutes at night and 25 to 30 m
around midday, and three quarters of the 10-minute steps end at night. The share of night steps is not the share
of time: the collars obtained more regular fixes at night, so night steps are over-represented, and a model that
pools them must take that into account (part 5 uses time of day as a variable). Dawn and dusk are short in
winter at this latitude, about half an hour each, and hold few steps.

### Net squared displacement

![The net squared displacement of each fisher over the days](/img/tutorials/animal-movement/steps-and-summaries/nsd.png "The squared distance (km²) of each fisher from its first location, over the days of tracking. Each animal goes out and comes back within a band: none disperses or migrates.")

None of the eight NSD curves rises and levels off: every fisher went out and came back within a band, the
pattern of a resident animal in a home range. The bands differ by an order of magnitude: F3 stays within 2.7 km²
(about 1.6 km from its first location), M2 goes up to 70 km² (8 km) late in its track, and M1 makes regular
trips to about 30 km² (5.5 km) and back, every few days. The fishers' breeding season, in March and April, falls
within the tracking of most of these animals, and long trips by males in search of females are a likely reason
for such excursions; the tracks alone cannot tell.

### The summaries

```tftable
data: /data/tutorials/animal-movement/steps-and-summaries/summary.csv
digits: 3
caption: The summaries of each track (move.track_summary) on the locations resampled every 10 minutes, in bursts of 3 or more. M1 has none at this rate. Distances in km, msd in km², sinuosity in 1/√m.
```

The cumulative distances (42 to 371 km) mostly reflect how long and how densely each animal was followed: compare
animals by distance per day at a common rate (part 2), not by these totals. The straightness is near 0 for all,
as for any resident animal; it says nothing here. The msd, the spread of each animal's locations, ranges from
0.2 km² (F3) to 3.9 km² (M2), in line with the NSD curves. The sinuosity (0.08 to 0.11) and tac (1.76 to 1.94,
below the 2 of independent turns) are similar across animals: at 10 minutes the fishers' paths are equally
tortuous, with slightly correlated turns.

To report: the summaries, the rate and tolerance they were computed at, and the number of locations and steps
they rest on.

## Settings that change the answer

- **rate** and **tolerance** of [[move.resample]]: the table of rates above. A wider tolerance keeps more fixes
  and longer bursts, but the steps' durations vary more; amt's examples use a tenth of the rate or less.
- **min_n** of [[move.filter_min_n_burst]]: 3 keeps every burst with a turn; a higher value keeps only long
  bursts (fewer, more regular steps).
- **start** of [[move.resample]]: the location the grid starts from. A different start keeps different fixes;
  with a short tolerance the results change little.
- **include_crepuscule** and **where** of [[move.time_of_day]]: with only day and night, the twilight steps join
  them; the time at the end or at the start of a step can differ by one label at the boundaries.
- **The track summarised**: summaries of the raw tracks (every fix) differ from those at 10 minutes (more steps,
  more tortuous). Give [[move.track_summary]] the resampled tracks for a fair comparison.

## Common pitfalls

- **Steps across gaps.** Steps built on a track that was not resampled join locations hours apart. Resample, or
  at least split the track into bursts, before [[move.steps]].
- **Animals that vanish.** An animal whose schedule does not fit the rate has no bursts and silently drops out
  (M1 at 10 minutes). Check the number of animals after [[move.filter_min_n_burst]].
- **NSD in squared degrees.** On tracks in longitude and latitude, [[move.nsd]] gives squared degrees, as amt
  does. Project first.
- **Directions from two conventions.** `direction_p` of [[move.steps]] is anticlockwise from east (amt);
  [[move.azimuth]] is clockwise from north (move2). In longitude and latitude, amt reads a compass bearing as if
  it were an angle from east; Terraflux converts it (π/2 minus the bearing), so its directions are right where
  amt's are rotated.
- **Times in UTC.** The time of day is computed from UTC times and the location, so it is right whatever the
  local time zone; but a histogram by hour (as above) must convert to local time to be read.

## Going further

- [The data of step selection functions](../step-selection/): the steps of this part with random steps and the
  landscape under them.
- The operators' reference: [[move.resample]], [[move.filter_min_n_burst]], [[move.steps]],
  [[move.time_of_day]], [[move.nsd]], [[move.track_summary]].

## References

- Batschelet, E. (1981). *Circular statistics in biology*. Academic Press.
- Benhamou, S. (2004). How to reliably estimate the tortuosity of an animal's path: straightness, sinuosity, or
  fractal dimension? *Journal of Theoretical Biology*, 229(2), 209-220. https://doi.org/10.1016/j.jtbi.2004.03.016
- Bunnefeld, N., Börger, L., van Moorter, B., Rolandsen, C. M., Dettki, H., Solberg, E. J., & Ericsson, G.
  (2011). A model-driven approach to quantify migration patterns: individual, regional and yearly differences.
  *Journal of Animal Ecology*, 80(2), 466-476. https://doi.org/10.1111/j.1365-2656.2010.01776.x
- Calenge, C., Dray, S., & Royer-Carenzi, M. (2009). The concept of animals' trajectories from a data analysis
  perspective. *Ecological Informatics*, 4(1), 34-41. https://doi.org/10.1016/j.ecoinf.2008.10.002
- Dray, S., Royer-Carenzi, M., & Calenge, C. (2010). The exploratory analysis of autocorrelation in
  animal-movement studies. *Ecological Research*, 25(3), 673-681. https://doi.org/10.1007/s11284-010-0701-7
- Signer, J., Fieberg, J., & Avgar, T. (2019). Animal movement tools (amt): R package for managing tracking data
  and conducting habitat selection analyses. *Ecology and Evolution*, 9(2), 880-890.
  https://doi.org/10.1002/ece3.4823
