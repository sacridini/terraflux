---
title: The data of step selection functions
short: Step selection
description: Compare each step an animal took with the steps it could have taken. Fit distributions to step lengths and turn angles, draw random steps from each observed start, as amt does, take the landscape under every step, and fit a conditional logistic regression with Terraflux that measures how strongly the fishers chose forest.
order: 50
level: advanced
time: 60 min
ops: [move.fit_distribution, move.distribution, move.random_numbers, move.random_steps, move.extract_covariates, move.fit_issf, move.time_of_day, vector.calc, data.download, raster.calc]
series: Animal movement
part: 5
---

Where does an animal go when it moves? [Part 4](../steps-and-summaries/) turned the fishers' tracks into regular
steps of 10 minutes. In this part you will ask what those steps chose. For every step a fisher took, you will
draw ten steps it could have taken from the same place, take the land cover and the elevation at the end of all
of them, and compare: did the real steps end in forest more often than the possible ones? This comparison of
**used** and **available** steps is the **step selection function** (SSF), the most used method for relating
animal movement to the landscape. Terraflux builds its data, as R's amt does, to the same numbers, and fits the model
itself, a conditional logistic regression, with [[move.fit_issf]]: this page shows how and what it says. [Part 9](../movement-on-the-relief/)
goes on from here with covariates of the relief.

## The idea

### Used and available, step by step

A resource selection function compares the places an animal used with places drawn at random from an area,
usually its home range. That raises the question of what was *available*: a fisher at one end of its range
could not reach the other end in 10 minutes. Step selection functions answer it with the animal's own movement.
From the start of every observed step, the steps the animal could have taken are drawn from the lengths and
turns it actually makes; each observed step and its random alternatives form a **stratum**, and the comparison
is made only within strata (Fortin et al., 2005; Thurfjell et al., 2014).

!!! science "Key idea: the step selection function"
    The probability that, of the \( J + 1 \) steps of stratum \( s \) (the observed one and \( J \) random ones),
    the animal took step \( i \) is

    \[ P(i \mid s) = \frac{\exp(\beta_1 x_{i1} + \dots + \beta_k x_{ik})}{\sum_{j \in s} \exp(\beta_1 x_{j1} + \dots + \beta_k x_{jk})} \]

    where \( x_{i1}, \dots, x_{ik} \) are the covariates of step \( i \) (the land cover at its end, its
    length...). It is the likelihood of a **conditional logistic regression** with one case per stratum, the
    model of matched case-control studies (R's `survival::clogit`). Only differences *within* a stratum count:
    a covariate that is the same for all the steps of a stratum (the time of day, the animal's sex) has no
    effect of its own, only through its interactions with covariates that vary.

    \( \exp(\beta) \) is the **relative selection strength**: how many times more likely the animal is to take
    a step that ends where the covariate is one unit higher, than an otherwise equal step, when both are
    equally available (Fieberg et al., 2021).

### Where the random steps come from

amt draws random steps in three moves (Signer et al., 2019):

1. fit a **gamma distribution** to the step lengths and a **von Mises distribution** to the turn angles of
   each track, the *tentative* distributions of its movement;
2. from the start of every observed step with a turn angle, draw `n_control` lengths and turns from them;
3. turn each drawn angle from the direction of the previous step, and walk the drawn length.

### Integrated step selection

The tentative distributions are fitted to the observed steps, which were themselves shaped by the landscape. Avgar
et al. (2016) showed that adding the step's length and turn to the model, as covariates, corrects them: with
\( \log(\text{sl}) \) and \( \cos(\text{ta}) \) among the covariates, the shape of the gamma distribution
becomes \( k + \beta_{\log \text{sl}} \) and the concentration of the von Mises distribution
\( \kappa + \beta_{\cos \text{ta}} \) (\( \text{sl} \) itself would correct the gamma's scale). The model then
estimates **habitat selection and movement together**: the **integrated step selection analysis** (iSSA), amt's
`fit_issf`.

## When to use it

- **How animals use a landscape while they move**: selection of forest, avoidance of roads or of open land,
  attraction to water, at the scale of a step.
- **Selection that changes with time or state**: by day and by night, by season, with the distance to a
  predator (Fortin et al., 2005), through interactions with the covariates.
- **Movement that depends on the landscape**: faster or straighter in some covers (interactions of
  \( \log(\text{sl}) \) or \( \cos(\text{ta}) \) with a covariate at the start of the step).
- **Resistance surfaces from data**: the coefficients of an SSF are one way of calibrating the costs of
  [circuit connectivity](../../circuit-connectivity/) or [grid distances](../../grid-distances/).

An SSF needs regular steps (part 4) at an interval at which the animal could plausibly choose among the random
steps. With fixes days apart, the "available" steps are no longer local, and a resource selection function
over the home range is the simpler choice.

## What you need

- The fishers' steps of part 4: resampled every 10 minutes with a tolerance of 1 minute, in bursts of 3
  locations or more, projected ([[move.aeqd_crs]]). [[move.random_steps]] refuses steps in longitude and
  latitude: it draws lengths and angles on a plane.
- **The landscape** under the steps: ESA WorldCover 2021 (land cover, 10 m) and the Copernicus DEM (30 m),
  which [[data.download]] fetches for the fishers' area. The rasters can stay in longitude and latitude:
  [[move.extract_covariates]] takes the steps to each raster's coordinate system.
- To fit the model, nothing else: [[move.fit_issf]] is R's `survival::clogit` (amt's `fit_issf`) in Terraflux. R with
  the package survival or amt gives the same numbers.

## Build the model

```tfgraph
#! caption: The steps of part 4 (nodes 1 to 8) with 10 random steps each (node 9), the land cover and elevation downloaded (nodes 10, 11), the forest as 1 and the rest as 0 (node 12), their values at the end of every step (node 13), the steps written for R or Python (node 14), the length clipped at 1 m (node 15) and the step selection function fitted (node 16).
#! numbered: true
#! highlight: random, dem, cover, forest, covariates, write, clipped, fit
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
id = "random"
op = "move.random_steps"
inputs = { steps = "steps.steps" }
params = { n_control = 10, seed = 1 }

[[nodes]]
id = "dem"
op = "data.download"
params = { source = "copernicus_dem", dem_resolution = 30, extent = [-74.0, 42.65, -73.33, 42.9], cellsize = 0.0003 }

[[nodes]]
id = "cover"
op = "data.download"
params = { url = "https://esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/map/ESA_WorldCover_10m_2021_v200_N42W075_Map.tif", extent = [-74.0, 42.65, -73.33, 42.9], cellsize = 0.0003, resampling = "mode" }

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { cover = "cover.raster" }
params = { expression = "1 if cover == 10 else 0" }

[[nodes]]
id = "covariates"
op = "move.extract_covariates"
inputs = { tracks = "random.steps", forest = "forest.map", elevation = "dem.raster" }

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "covariates.tracks" }
params = { path = "random-steps.gpkg" }

[[nodes]]
id = "clipped"
op = "vector.calc"
inputs = { vector = "covariates.tracks" }
params = { column = "sl_clip", expression = "sl_ if sl_ > 1 else 1" }

[[nodes]]
id = "fit"
op = "move.fit_issf"
inputs = { steps = "clipped.vector" }
params = { formula = "case_ ~ forest + I(elevation / 100) + log(sl_clip) + cos(ta_)" }
```

<div class="steps" markdown>

1. **Draw the random steps** with [[move.random_steps]] (library subject **Animal movement**), connected to
   `steps.steps`. `n_control` is the number of random steps per observed step (10); `seed` the seed of R's
   generator, so that the same seed gives amt's steps. The node fits a gamma and a von Mises distribution to
   each track itself; its output `distributions` lists them. Each observed step gets `case_ = 1` and its random
   steps `case_ = 0`; `step_id_` numbers the strata **within each track**.

2. **Download the landscape** with two [[data.download]] nodes over the same extent and cell size: the
   Copernicus DEM (`source = "copernicus_dem"`), and the WorldCover tile by its `url`, resampled by the most
   common class (`resampling = "mode"`), as a categorical map must be.

3. **Make the covariate** with [[raster.calc]]: `1 if cover == 10 else 0`, the tree cover of WorldCover as 1,
   every other class as 0.

4. **Take the values at the steps' ends** with [[move.extract_covariates]]. Connect the steps to `tracks` and
   each raster to an input of the name you want for its column (here `forest` and `elevation`). On steps, the
   value is taken at the end (`where = "end"`, the default, as amt), at the start (`start`) or at both (`both`,
   columns suffixed `_start` and `_end`). The steps are projected to each raster's coordinate system, and the
   value is that of the cell holding the point (terra's `extract`): none outside the raster or on no data.

5. **Write the steps** with [[vector.write]], to fit the model outside Terraflux. The `table` output of
   [[move.extract_covariates]] has only the columns that are numbers, without the animal's name, which the
   strata need; write the vector.

6. **Fit the model** with [[move.fit_issf]] (node 16), connected to the steps with their covariates. The formula
   is R's, in a text: `case_` on the covariates, `log(sl_clip)` and `cos(ta_)`, the step's length and turn that
   make the analysis *integrated*. The lengths are clipped at 1 m by [[vector.calc]] (node 15) first, because
   random steps can be drawn far shorter (1,622 of them are under a metre) and the formula takes the logarithm of
   a column, not `pmax` of R. The stratum is each step of each track (`strata = "track_step"`, the default:
   `step_id_` restarts in every track, so it is not enough alone), so you do not make it. The `table` output has
   each coefficient with its standard error, z, p, exp(coefficient) and 95 % interval; `fit`, a row with the
   log-likelihood, the AIC and the number of steps.

</div>

### The distributions on their own

[[move.random_steps]] fits the distributions it needs, but they can be fitted, inspected and drawn from as nodes
of their own:

```tfgraph
#! caption: A gamma distribution fitted to all the step lengths (sl) and a von Mises distribution to all the turn angles (ta), 20 000 draws of each, and random steps (given) from two distributions given by their parameters (slow, straight) rather than fitted per track.
#! fragment: true
#! highlight: sl, ta, given
[[nodes]]
id = "sl"
op = "move.fit_distribution"
inputs = { values = "steps.steps" }
params = { name = "gamma" }

[[nodes]]
id = "ta"
op = "move.fit_distribution"
inputs = { values = "steps.steps" }
params = { name = "vonmises" }

[[nodes]]
id = "sl_draws"
op = "move.random_numbers"
inputs = { distribution = "sl.distribution" }
params = { n = 20000, seed = 1 }

[[nodes]]
id = "ta_draws"
op = "move.random_numbers"
inputs = { distribution = "ta.distribution" }
params = { n = 20000, seed = 1 }

[[nodes]]
id = "slow"
op = "move.distribution"
params = { name = "gamma", shape = 0.8, scale = 100 }

[[nodes]]
id = "straight"
op = "move.distribution"
params = { name = "vonmises", kappa = 0.5 }

[[nodes]]
id = "given"
op = "move.random_steps"
inputs = { steps = "steps.steps", sl_distr = "slow.distribution", ta_distr = "straight.distribution" }
params = { n_control = 10, seed = 1 }
```

[[move.fit_distribution]] fits `gamma`, `exp`, `lnorm` or `hnorm` to the step lengths (`sl_`) and `vonmises`
or `unif` to the turn angles (`ta_`), by amt's methods (`fit_distr`), to the same numbers; `column` chooses
another column, and a table works as well as steps. [[move.distribution]] makes a distribution from parameters
you give (amt's `make_distribution`), [[move.random_numbers]] draws from either. Connected to `sl_distr` and
`ta_distr`, they replace the per-track fits of [[move.random_steps]]: one distribution for all the animals,
or a distribution taken from another study.

### In Python

The data are the same calls, and the model is the library's `tf.move.fit_issf`. The steps are those of part 4 (10 minutes, tolerance 1 minute, projected), and the random
steps, their number and the seed are the page's:

```python
import numpy as np
import pandas as pd
import terraflux as tf

pd.set_option("display.width", 120, "display.max_columns", 10)
ID = "individual-local-identifier"
tracks = tf.load_tracks(tf.move.example_data("fishers"))
tracks = tf.move.filter_unique(tf.move.filter_movebank_visible(tracks))
tracks = tracks[~tracks.geometry.is_empty].reset_index(drop=True)
local = tracks.to_crs(tf.move.aeqd_crs(tracks))                 # random_steps refuses longitude and latitude
steps = tf.move.steps(tf.move.filter_min_n_burst(tf.move.resample(local, "10min", "1min"), 3))

sl = tf.move.fit_distr(steps["sl_"], "gamma")                   # move.fit_distribution: also exp, lnorm, hnorm
ta = tf.move.fit_distr(steps["ta_"], "vonmises")                # or unif
print(sl)
print(ta)
draws = tf.move.random_numbers(sl, n=20000, seed=1)             # move.random_numbers
print(len(draws), round(float(draws.mean()), 1), "m on average")
```

```text
Distribution(name='gamma', params={'shape': 0.7940565253577497, 'scale': 137.2285731348917})
Distribution(name='vonmises', params={'kappa': 0.08284384664726341, 'mu': 0.0})
20000 109.4 m on average
```

The pooled distributions are those of the page (shape 0.794, scale 137.2, kappa 0.083). [`tf.move.random_steps`][terraflux.move.random_steps]
fits them to each animal itself; the fits are in the result's `attrs`:

```python
ssf = tf.move.random_steps(steps, n_control=10, seed=1)         # node 9
print(len(ssf), int(ssf["case_"].sum()), "strata of", ssf[ID].nunique(), "animals")
meta = ssf.attrs["terraflux.move"]
fits = pd.DataFrame({a: {"gamma shape": d.params["shape"], "gamma scale (m)": d.params["scale"],
                         "von Mises kappa": meta["ta_distr"][a].params["kappa"]}
                     for a, d in meta["sl_distr"].items()}).T
print(fits.round(3))
print(ssf[[ID, "step_id_", "case_", "sl_", "ta_"]].head(4))
```

```text
66286 6026 strata of 7 animals
    gamma shape  gamma scale (m)  von Mises kappa
F1        0.647          115.464            0.349
F2        1.048          152.138            0.328
F3        1.007          103.129            0.374
M2        0.622          143.861            0.266
M3        1.062          151.033            0.436
M4        0.974          114.381            0.266
M5        0.955          126.857            0.367
  individual-local-identifier  step_id_  case_         sl_       ta_
0                          F1         3   True   15.945515  3.106502
1                          F1         3  False   18.927724 -0.894106
2                          F1         3  False   90.432637  0.415735
3                          F1         3  False  118.993832 -1.111006
```

The observed steps have `case_` True and `step_id_` numbers the strata **within each animal**. Distributions you give
(`tf.move.make_distribution`, amt's `make_distribution`) replace the per-animal fits, one for all the animals or taken
from another study:

```python
slow = tf.move.make_distribution("gamma", shape=0.8, scale=100)
straight = tf.move.make_distribution("vonmises", kappa=0.5)
given = tf.move.random_steps(steps, n_control=10, sl_distr=slow, ta_distr=straight, seed=1)
print(given.groupby("case_")["sl_"].median().round(1).to_dict())
```

```text
{False: 50.0, True: 55.8}
```

The page's landscape is downloaded (ESA WorldCover and the Copernicus DEM). The library carries another one, in the
projection of the United States, that covers four of the fishers (F1, F2, M1 and M4): the ASTER elevation and the
GlobCover land use. [`tf.move.extract_covariates`][terraflux.move.extract_covariates] reads rasters at the end of each
step (`where="start"` or `"both"` too), whatever their resolution and CRS; a dict names the columns. M1 has no steps at
10 minutes, so the model below is of F1, F2 and M4, and its numbers are not the page's (another landscape, another
resolution, three animals), though the method is the same:

```python
elevation = tf.load_raster(tf.move.example_data("elevation"))
landuse = tf.load_raster(tf.move.example_data("landuse"))
inside = ssf[ssf[ID].isin(["F1", "F2", "M4"])]
data = tf.move.extract_covariates(inside, {"elevation": elevation, "landuse": landuse})
data = data.dropna(subset=["elevation", "landuse"]).copy()
data["forest"] = data["landuse"].isin([50, 70, 100]).astype(int)    # GlobCover's closed-canopy forest classes
data["tod"] = tf.move.time_of_day(data)                             # at the end of each step
print(len(data), "steps;", elevation.rio.resolution()[0], "m cells of elevation,", landuse.rio.resolution()[0], "m of land use")
print(data.groupby("case_")[["forest", "elevation"]].mean().round(3))
```

```text
28270 steps; 44.0 m cells of elevation, 220.0 m of land use
       forest  elevation
case_                   
False   0.899     94.322
True    0.909     94.866
```

The conditional logistic regression is [`tf.move.fit_issf`][terraflux.move.fit_issf], amt's `fit_issf`: survival's `clogit`
with the exact partial likelihood, with its formula in R's syntax, bit for bit with R's. The stratum is the animal and
the step, because `step_id_` restarts in every animal; `log(sl_clip)` and `cos(ta_)` correct the movement (the
*integrated* analysis), with the lengths clipped at 1 m before the logarithm in a column, as node 15 does (the formula
takes no `pmax`):

```python
data["stratum"] = data[ID].astype(str) + "_" + data["step_id_"].astype(int).astype(str)
data["sl_clip"] = data["sl_"].clip(lower=1)
formula = "case_ ~ forest + I(elevation / 100) + log(sl_clip) + cos(ta_) + strata(stratum)"
fit = tf.move.fit_issf(data, formula)                           # move.fit_issf
print(data["stratum"].nunique(), "strata")
print(fit.summary().round(3)[["coef", "se", "exp_coef"]])

data["forest_night"] = data["forest"] * (data["tod"] == "night")     # time of day enters through an interaction
night = tf.move.fit_issf(data, formula.replace("forest +", "forest + forest_night +"))
print(night.summary().round(3)[["coef", "se", "exp_coef"]])
```

```text
2570 strata
                   coef     se  exp_coef
forest            0.167  0.103     1.182
I(elevation/100)  1.376  0.344     3.958
log(sl_clip)     -0.002  0.015     0.998
cos(ta_)         -0.319  0.029     0.727
                   coef     se  exp_coef
forest            0.048  0.249     1.049
forest_night      0.144  0.273     1.155
I(elevation/100)  1.386  0.344     4.000
log(sl_clip)     -0.002  0.015     0.998
cos(ta_)         -0.319  0.029     0.727
```

On these three animals forest is selected little (the coefficient is 0.17, a relative selection strength of 1.2, and
its interval includes 1), where the page's seven animals in the WorldCover map select it strongly. The land use cells
are 220 m wide, four times the median step (56 m): the forest at the end of a step barely differs from the one at its
start, the pitfall of covariates that do not vary within strata. The elevation coefficient (1.4 per 100 m) rests on
differences of a few metres within strata, on 44 m cells: read it with the page's caution. The coefficient of `cos_ta`
is negative here too: the observed steps go straight on less often than the random ones, drawn around 0. For the
model in R, the page's code fits it with `survival` (below); the part [9](../movement-on-the-relief/) fits it on a relief.

## Read the results

### The tentative distributions

![The step lengths and the fitted gamma distribution](/img/tutorials/animal-movement/step-selection/gamma.png "The lengths of the 7,480 steps of 10 minutes (green) and 20,000 draws of the gamma distribution fitted to all of them (orange; shape 0.794, scale 137.2 m). The two steps over 800 m are left out of the drawing.")

![The turn angles and the fitted von Mises distribution](/img/tutorials/animal-movement/step-selection/vonmises.png "The turn angles of the 6,319 of them that have one (blue) and 20,000 draws of the von Mises distribution fitted to all of them (orange; kappa 0.083, centred on 0). The observed turns peak at 0° and at ±180°; the von Mises distribution, with one peak, is nearly flat.")

A gamma distribution with a shape under 1, as here, has its mode at 0: most steps short, a long tail of
travel, which fits the lengths well. The turn angles are harder. The fishers either go on (0°) or turn back
(±180°), and a von Mises distribution centred on 0 has a single peak: fitted to all the turns, it is almost
uniform. Fitted per track, as [[move.random_steps]] does, it is more concentrated:

```tftable
data: /data/tutorials/animal-movement/step-selection/distributions.csv
digits: 3
caption: The distributions move.random_steps fitted to each fisher's 10-minute steps (its distributions output), and those fitted to all the steps at once (move.fit_distribution). M1 has no steps at 10 minutes (part 4).
```

!!! warning "How amt fits the turns"
    amt fits the von Mises distribution with circular's `mle.vonmises`, which estimates the mean direction as
    well as \( \kappa \), and then keeps \( \kappa \) and sets the mean direction to 0. For an animal that
    turns back more often than it goes on, the fitted \( \kappa \) measures its concentration around 180°, but
    the random steps are drawn concentrated around 0°. F1 and M2 are such animals here: the mean cosine of
    their turns is negative (−0.17 and −0.13), yet their \( \kappa \) is 0.35 and 0.27. Terraflux does as amt;
    the \( \cos(\text{ta}) \) term of an iSSA corrects for it, as the model below shows.

### The steps and their strata

```tftable
data: /data/tutorials/animal-movement/step-selection/used-available.csv
digits: 1
caption: The observed steps and their random steps (n_control = 10): how many, the share that ends in tree cover, the mean elevation at their end and their median length.
```

The 7,480 steps of part 4 become 6,026 strata of 11 steps each, 66,286 steps in all: the first step of each
burst has no turn angle (no previous step to turn from), and bursts of fewer than 3 steps are left out. The
observed steps end in tree cover more often than the random ones (96.4 % against 91.0 %); their elevations are
the same within a metre.

![One stratum over the land cover](/img/tutorials/animal-movement/step-selection/stratum.png "One stratum of M4: the observed step (orange) from its start (black) and its ten random steps (dashed), over the land cover the values are taken from. Two random steps end in wetland, the rest, like the observed step, in tree cover. The long step to the south-west ends in tree cover, at the edge of a patch of cropland.")

A single stratum shows the comparison the model makes. M4 started this step in tree cover, near a wetland and open
water. Of its ten random alternatives, two end in the wetland; the observed step, and eight of the random ones,
end in forest. Strata like this one, where the forest varies, carry the information about forest selection; a
stratum entirely in forest says nothing about it. Here 38 % of the strata have both forest and other covers
among their ends.

### The conditional logistic regression

Node 16 fits the model, and the table below is its `table` output. In R, the same model from the file of node 14:

```r
library(sf)
library(survival)
d <- st_drop_geometry(st_read("output/random-steps.gpkg"))
# step_id_ restarts in every track: the stratum is the animal and the step
d$stratum <- paste(d$individual.local.identifier, d$step_id_)
m <- clogit(case_ ~ forest + I(elevation / 100) + log(pmax(sl_, 1)) + cos(ta_) +
              strata(stratum), data = d)
summary(m)
```

With amt, the same model is `fit_issf(d, case_ ~ forest + I(elevation / 100) + log(pmax(sl_, 1)) + cos(ta_) +
strata(stratum))`. R's `sf` reads the column `individual-local-identifier` as `individual.local.identifier`.
The lengths are clipped at 1 m before the logarithm, because random steps can be drawn far shorter (1,622 of
them are under a metre). The numbers below are the `table` output of node 16 (which equals R's `clogit` on these
data, as the library's documentation says: this page did not run R):

```tftable
data: /data/tutorials/animal-movement/step-selection/clogit.csv
digits: 3
caption: The integrated step selection function of the seven fishers at 10 minutes, 10 random steps per observed step, 6,026 strata: the coefficients, their standard errors, and the relative selection strength exp(coef).
```

**Forest.** \( \exp(1.221) = 3.4 \): of two steps equally available, equal in length and turn and elevation,
one ending in tree cover and one not, the fishers were 3.4 times as likely to take the one into forest (95 %
interval 2.9 to 4.0). Fishers are known as forest carnivores; the SSF measures their preference at the scale of a
10-minute step.

**Elevation.** \( 0.22 \pm 0.13 \) per 100 m: no clear selection. The ends of a stratum's steps lie within a few
hundred metres of each other, and the elevation among them varies by 18 m in the median stratum; on average the
observed and random ends differ by half a metre. An SSF sees only the contrast within strata, so a covariate must vary at the scale
of a step to be estimated well.

**Movement.** The coefficient of \( \log(\text{sl}) \) is small (0.027): the gamma distributions fitted to each
track describe the fishers' step lengths well. That of \( \cos(\text{ta}) \) is clearly negative (−0.236): the
observed steps go straight on less often than the random steps drawn from the von Mises distributions, which
are concentrated around 0° (see the warning above). By Avgar et al.'s (2016) correction, the fishers' \( \kappa \)
is about 0.24 lower than the tentative ones (0.27 to 0.44), between 0.03 and 0.2: at 10 minutes their turns
have little preferred direction around 0°, as the two-peaked histogram shows. The correction is strictly one per
animal, each with its own tentative distribution; one model for all of them, as here, gives their average.

### How many random steps

```tfchart
type: line
data: /data/tutorials/animal-movement/step-selection/controls.csv
x: controls
y: [forest, lower, upper]
labels: { forest: "Coefficient of forest", lower: "95 % interval, lower", upper: "95 % interval, upper" }
colors: { forest: "#3b8a5a", lower: "#9aa59a", upper: "#9aa59a" }
xlog: true
xlabel: Random steps per observed step (n_control)
ylabel: Coefficient of forest
height: 300
caption: The coefficient of forest, and its 95 % interval, with 2, 5, 10, 20 and 50 random steps per observed step (seed 1 each time).
```

With 2 random steps per stratum, the estimate is higher and less precise; from 5 on it settles at 1.20 to 1.22,
and its interval narrows little beyond 10. More random steps reduce the noise of the sampling of availability,
not the noise of the animals' choices, which is fixed by the number of observed steps. The usual advice is to
check, as here, that the estimates no longer move (Thurfjell et al., 2014; Fieberg et al., 2021); the cost of
more steps is only computing time ([[move.random_steps]] and [[move.extract_covariates]] take seconds for the
66,286 steps here, 300,000 at 50 controls).

### Forest by day and by night

The time of day is the same for all the steps of a stratum, so it enters the model only through an interaction.
Label every step with [[move.time_of_day]], connected to `covariates.tracks`, and add `forest × night`:

```tftable
data: /data/tutorials/animal-movement/step-selection/clogit-tod.csv
digits: 3
caption: The same model with the interaction of forest and night (move.time_of_day at the end of the step): forest selection by day, and how much it differs at night.
```

By day, the coefficient of forest is 1.33; at night it is 0.12 lower, but with a standard error of 0.23: the
data show no difference. Three quarters of the strata are at night (4,726 of 6,026), when the fishers move,
and the day's 1,300 strata give the day's coefficient its wider interval.

## Settings that change the answer

- **The interval of the steps** (part 4): an SSF at 10 minutes and one at 2 hours describe choices at different
  scales, and their coefficients differ; compare only SSFs at the same interval.
- **n_control** of [[move.random_steps]]: the chart above. Check that the estimates are stable.
- **The tentative distributions**: per track (the default), pooled, or given ([[move.distribution]] into
  `sl_distr`, `ta_distr`). Include \( \log(\text{sl}) \) and \( \cos(\text{ta}) \) in the model: without
  movement terms, the habitat coefficients can be biased by the way availability was sampled (Forester et al.,
  2009; Avgar et al., 2016).
- **seed**: another seed draws other random steps, and the coefficients move a little. Report the seed, and
  check that the conclusions do not depend on it.
- **where** of [[move.extract_covariates]]: the end of the step (where the animal chose to go) is the usual
  choice; the start serves for covariates that change how it moves (interactions with \( \log(\text{sl}) \) or
  \( \cos(\text{ta}) \)).
- **The covariates' resolution**: the land cover here is resampled to cells of about 25 by 33 m (`cellsize =
  0.0003` degrees), the scale of the steps. Coarser cells blur the contrasts within strata.

## Common pitfalls

- **Strata across animals.** `step_id_` restarts in every track. A stratum of `step_id_` alone joins up to seven
  animals' steps in one stratum, and the model is wrong without any error: here it gives forest 1.06 and
  elevation −0.03, instead of 1.22 and 0.22. Build the stratum from the animal and the step.
- **Pooling animals.** One model for all the fishers treats their steps as one animal's. Animals differ in what
  they select; fit one model per animal and summarise them, or a mixed conditional logistic regression with
  random slopes per animal (Muff et al., 2020).
- **Unprojected steps.** [[move.random_steps]] refuses steps in longitude and latitude. Project with
  [[move.aeqd_crs]] first (part 3).
- **Steps off the map.** A random step that ends outside a raster, or on its no data, gets no value, and R drops
  its row. Download the landscape over the tracks' extent with a margin of a few long steps.
- **Categorical rasters resampled as numbers.** A land cover resampled by bilinear or cubic interpolation gets
  classes that do not exist (the mean of forest and water). Use `resampling = "mode"` or `"near"` for
  categories.
- **A von Mises centred on 0.** amt's tentative turns are centred on 0 even for animals that reverse more often
  than they go on (the warning above): include \( \cos(\text{ta}) \) in the model.
- **Covariates that do not vary within strata.** Time of day, season, the animal's sex: only their interactions
  can be estimated.

## Going further

- [Regular steps and track summaries](../steps-and-summaries/): the steps this part starts from.
- [Species distribution models](../../species-distributions/ideas/): where a species can live, from occurrences
  instead of tracks.
- [Movement on the relief](../movement-on-the-relief/): the same model with the energy of walking, the slope and the climb of
  a DEM as covariates, for Alpine ibex, and what to do with the fit (log-RSS, simulated paths, a resistance).
- [Circuit connectivity](../../circuit-connectivity/): resistance surfaces, which SSF coefficients can calibrate.
- The operators' reference: [[move.fit_distribution]], [[move.distribution]], [[move.random_numbers]],
  [[move.random_steps]], [[move.extract_covariates]], [[move.fit_issf]].

## References

- Avgar, T., Potts, J. R., Lewis, M. A., & Boyce, M. S. (2016). Integrated step selection analysis: bridging
  the gap between resource selection and animal movement. *Methods in Ecology and Evolution*, 7(5), 619-630.
  https://doi.org/10.1111/2041-210X.12528
- Fieberg, J., Signer, J., Smith, B., & Avgar, T. (2021). A 'How to' guide for interpreting parameters in
  habitat-selection analyses. *Journal of Animal Ecology*, 90(5), 1027-1043.
  https://doi.org/10.1111/1365-2656.13441
- Forester, J. D., Im, H. K., & Rathouz, P. J. (2009). Accounting for animal movement in estimation of resource
  selection functions: sampling and data analysis. *Ecology*, 90(12), 3554-3565.
  https://doi.org/10.1890/08-0874.1
- Fortin, D., Beyer, H. L., Boyce, M. S., Smith, D. W., Duchesne, T., & Mao, J. S. (2005). Wolves influence elk
  movements: behavior shapes a trophic cascade in Yellowstone National Park. *Ecology*, 86(5), 1320-1330.
  https://doi.org/10.1890/04-0953
- Muff, S., Signer, J., & Fieberg, J. (2020). Accounting for individual-specific variation in habitat-selection
  studies: efficient estimation of mixed-effects models using Bayesian or frequentist computation. *Journal of
  Animal Ecology*, 89(1), 80-92. https://doi.org/10.1111/1365-2656.13087
- Signer, J., Fieberg, J., & Avgar, T. (2019). Animal movement tools (amt): R package for managing tracking data
  and conducting habitat selection analyses. *Ecology and Evolution*, 9(2), 880-890.
  https://doi.org/10.1002/ece3.4823
- Thurfjell, H., Ciuti, S., & Boyce, M. S. (2014). Applications of step-selection functions in ecology and
  conservation. *Movement Ecology*, 2, 4. https://doi.org/10.1186/2051-3933-2-4
