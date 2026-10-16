---
title: Predictors and the accessible area
short: Predictors and background
description: Choose predictors, remove collinear ones with the variance inflation factor, measure their range of autocorrelation, and draw background points from the area the species could reach.
order: 30
level: intermediate
time: 50 min
ops: [sdm.vif, sdm.select_predictors, sdm.autocorrelation_range, sdm.accessible_area, sdm.background, sdm.dataset]
series: Species distribution models
part: 3
---

A model of presence-only records compares two things: the conditions at the records and the conditions
available to the species. This part is about both sides of that comparison. You will choose the
**predictors** (the environmental variables the model may use), remove those that duplicate each other, define
the **accessible area** the species could have reached, and draw **background points** from it. At the end you
will have a table of records and background points with their predictor values, ready to be fitted in [part
4](../fitting-models/), and you will have seen why the size of the accessible area changes the scores of a model
more than almost any other choice.

We continue with the brown-throated sloth: the 83 records left after the cleaning, one per cell and the thinning
at 50 km of [part 2](../occurrence-data/), and the nine bioclimatic variables of [part 1](../ideas/).

## The idea

### Choosing predictors

A predictor should have a plausible effect on the species: a physiological limit (the cold a tropical mammal
cannot stand, the drought a tree cannot survive), a resource, a condition of its habitat. Availability is not a
reason. Climate is the usual start because it limits most species at broad scales; at finer scales, land cover,
terrain and soils matter more.

![The nine bioclimatic variables](/img/tutorials/species-distributions/predictors-and-background/predictors.png "The nine bioclimatic variables of the example at 0.5° (temperatures in °C, precipitation in mm). Several look alike: bio1, bio6, bio8 and bio9 all show the warm tropics and the cold Andes and south; bio12 and bio16 the wet Amazon and Chocó.")

### Collinearity

Climate variables are strongly correlated. Where the annual mean temperature (bio1) is high, so are the
temperatures of the wettest and the driest quarters (bio8 and bio9). bio7, the annual range of temperature, is
exactly bio5 − bio6. Correlated predictors are a problem for three reasons (Dormann et al., 2013):

- the model can trade one for another with almost no change in fit, so the contribution of each is unstable and
  the response curves cannot be interpreted;
- the trade that worked in the data breaks down where the correlation changes: in another region, or in a future
  climate where temperature rises but rainfall does not follow;
- more predictors mean more parameters, and more chances to fit noise.

!!! science "Key idea: the variance inflation factor"
    The **variance inflation factor** (VIF) of a predictor measures how well the other predictors predict it.
    Regress predictor \(j\) on all the others and take

    \[ \mathrm{VIF}_j = \frac{1}{1 - R_j^2} \]

    where \(R_j^2\) is the coefficient of determination of that regression (the share of the predictor's variance
    the others explain). A predictor unrelated to the others has \(R_j^2 = 0\) and a VIF of 1; one that the others
    explain at 90 % has a VIF of 10; one that they determine exactly (bio7 from bio5 and bio6) has an infinite VIF.
    Values above 10 (some authors say 5) are the usual sign of trouble.

A stepwise procedure removes collinearity: compute every VIF, drop the predictor with the largest one, compute
again, and repeat until all are below the threshold. This is `vifstep` of the R package usdm (Naimi et al.,
2014). usdm's `vifcor` does the same with the pairwise correlation: of the most correlated pair above a threshold,
drop the one with the larger VIF. Terraflux's [[sdm.select_predictors]] does both, in that order, and keeps the
predictors you name whatever their VIF.

A VIF filter is a statistical safeguard, not a substitute for thinking. When two correlated variables mean
different things to the species, keep the one the biology points to.

### The accessible area

Background points describe the conditions the species could have chosen from. Where should they come from? Not
from the whole world: a background in the Sahara and Siberia makes it trivially easy to tell a sloth's records
apart, and the model learns little more than "tropical". The background belongs in **M**, the area the species
could have reached over its history (the M of the BAM diagram of [part 1](../ideas/#the-bam-diagram); Barve et
al., 2011). Places outside M may be suitable but empty for want of access; counting them as available tells the
model they are unsuitable.

In practice M is approximated by:

- a **buffer**: the land within a distance of the records (Terraflux merges geodesic circles around each record,
  so the distance is in metres on the ground at any latitude);
- the **convex hull** of the records, the smallest convex polygon holding them all, possibly widened by a
  distance;
- the **regions** that hold records (ecoregions, biomes, river basins), on the idea that a species has had access
  to the region it lives in.

The distance of a buffer should reflect how far the species disperses and how long it has had: a few tens of
kilometres for a land snail, hundreds for a large mammal over thousands of years. VanDerWal et al. (2009) showed
how strongly the result depends on it.

### Background points

[[sdm.background]] draws random cells with a value in every predictor, within an area, leaving out the cells of
the records. On longitude and latitude grids, cells near the poles are smaller than at the equator, so cells are
weighted by their width (as `randomPoints` of the R package dismo does). How many? Enough to describe the
conditions of the area: 10 000 is the default of Maxent and of Terraflux, but never more than the cells available.
On our grid of 0.5°, a buffer of 500 km holds a few thousand cells of land, so we draw 2 000.

### Autocorrelation and the size of spatial blocks

Nearby cells have similar climates: the predictors are **spatially autocorrelated**. This matters for the
evaluation of [part 4](../fitting-models/): test records next to training records are nearly copies of them, and
the scores flatter the model (Roberts et al., 2017). The cure is to test on blocks of space larger than the range
over which values stay alike.

!!! science "Key idea: the range of a variogram"
    A **variogram** plots how different the values of two cells are (half their squared difference, on average)
    against the distance between them. It rises with distance and levels off at the **sill**, the variance of the
    whole map; the distance where it levels off is the **range**, beyond which two cells are no more alike than
    two cells chosen at random. [[sdm.autocorrelation_range]] fits variograms (spherical, exponential, Gaussian
    and Matérn, as the R packages gstat and automap) to a sample of each predictor's cells, keeps the best and
    reports its range; their median gives the block size, as `cv_spatial_autocor` of blockCV does (Valavi et al.,
    2019).

## When to use it

Every model of presence-only data needs predictors and background points. Measure collinearity whenever you use
more than two or three climate variables. Restrict the background to an accessible area whenever the records
cover only part of the map (nearly always); think about M especially for species limited by barriers (rivers,
mountains, seas) or for invasive species whose native and invaded ranges differ.

If you have **presence-absence** data, the absences replace the background: feed them to [[sdm.dataset]] as a
table with a presence column of 1 and 0 and no background is drawn.

## What you need

- The predictors as one stack (one grid). Rasters of different resolutions or extents must first be put on one
  grid with [[raster.match]].
- The records (cleaned and thinned in [part 2](../occurrence-data/)).
- Optionally, polygons of regions (ecoregions, biomes) for an accessible area by regions, and polygons to cut
  the area to (a country, the land).

## Build the model

### Measure collinearity

```tfgraph
#! caption: The records thinned (node 3), the accessible area within 500 km of them (node 4), the VIF of the nine predictors inside it (node 5) and the selection of predictors below a VIF of 10 (node 6).
#! numbered: true
#! highlight: area, vif, select
format = "terraflux/1"

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "bradypus.csv" }

[[nodes]]
id = "climate"
op = "raster.read"
params = { path = "bioclim.tif" }

[[nodes]]
id = "thin"
op = "sdm.thin"
inputs = { occurrences = "records.vector" }
params = { distance = 50000 }

[[nodes]]
id = "area"
op = "sdm.accessible_area"
inputs = { occurrences = "thin.occurrences" }
params = { distance = 500000 }

[[nodes]]
id = "vif"
op = "sdm.vif"
inputs = { env = "climate.raster", area = "area.area" }
params = { seed = 1 }

[[nodes]]
id = "select"
op = "sdm.select_predictors"
inputs = { env = "climate.raster", area = "area.area" }
params = { vif = 10, seed = 1 }
```

For brevity the graph thins the raw records; in a real study connect the output of the cleaning and of
[[sdm.one_per_cell]] of [part 2](../occurrence-data/) to node 3. The climate's `stack` (node 2) holds every band of
`bioclim.tif`.

<div class="steps" markdown>

1. **The accessible area** (node 4): add [[sdm.accessible_area]] and connect `thin.occurrences` to its
   `occurrences`. Set `distance = 500000` (metres). The output `area` is one polygon in longitude and latitude
   (cut at the antimeridian when needed); connect polygons to its `within` input to cut it to the land of a
   country or a continent, or set `method = convex_hull` or `regions` (with polygons on `regions`).
2. **The VIF** (node 5): add [[sdm.vif]], connect the stack to `env` and the area to `area`. It samples up to
   `size` cells (5 000 by default) inside the area and returns a table with one column per predictor, its VIF.
3. **The selection** (node 6): add [[sdm.select_predictors]] with the same inputs. `vif = 10` is the threshold;
   `cor = 0.7` would first remove the most correlated pairs; `keep = ["bio12"]` keeps a predictor whatever its
   VIF. Its `env` output is the stack of the predictors kept, and `selection` lists the kept, the excluded (in the
   order they were dropped) and the VIFs of those kept.

</div>

The VIFs, over the whole map and inside the accessible area:

![Variance inflation factors of the nine predictors](/img/tutorials/species-distributions/predictors-and-background/vif.png "Variance inflation factors of the nine bioclimatic variables on a log scale, over the whole map (grey) and inside the accessible area of 500 km (blue); infinite values are drawn at a million. In green, the VIFs of the four predictors kept by the selection, all below 2.")

| Predictor | VIF, whole map | VIF, accessible area | Kept |
|---|---|---|---|
| bio1 | ∞ | ∞ | dropped 1st |
| bio5 | 9 952 | 5 813 | **1.25** |
| bio6 | 40 803 | 14 622 | dropped 2nd |
| bio7 | 33 680 | 5 453 | **1.65** |
| bio8 | 5.4 | 39.0 | dropped 5th |
| bio9 | ∞ | ∞ | dropped 3rd |
| bio12 | 58.3 | 36.9 | dropped 4th |
| bio16 | 35.3 | 18.7 | **1.67** |
| bio17 | 9.8 | 9.0 | **1.57** |

Two things to notice. The temperature variables are almost perfectly collinear: bio1 and bio9 have infinite VIFs
because, on this grid, a combination of the others reproduces them, and bio7 is bio5 − bio6. And collinearity
**depends on the area**: bio8, the temperature of the wettest quarter, is nearly independent of the others over
the whole continent (VIF 5.4) but strongly collinear inside the sloth's tropical area (39). This is why the VIF is
measured inside the accessible area.

The selection drops bio1, bio6, bio9, bio12 and bio8, one at a time, and keeps four predictors with VIFs between
1.25 and 1.67:

- **bio5**, the maximum temperature of the warmest month;
- **bio7**, the annual range of temperature;
- **bio16**, the precipitation of the wettest quarter;
- **bio17**, the precipitation of the driest quarter.

A threshold of 5 keeps the same four. They make ecological sense for a sloth: a slow mammal with a low and
variable body temperature, sensitive to cold and to seasonality, that feeds on the leaves of evergreen trees.

### The range of autocorrelation

Add [[sdm.autocorrelation_range]] and connect the predictors kept (`select.env`). Its outputs: `range`, the
median range in metres on longitude and latitude grids; `block_size`, the same in the grid's units (degrees here);
`layers`, the range, sill, nugget and smoothness (`kappa`) of each predictor's variogram; and `models`, the model
chosen for each.

| Predictor | Model | Range |
|---|---|---|
| bio5 | Matérn (Stein) | 1 329 km |
| bio7 | Matérn (Stein) | 2.7 million km |
| bio16 | Matérn (Stein) | 1.5 million km |
| bio17 | exponential | 1 192 km |

Two predictors have ranges of about 1 200 to 1 300 km. The other two have absurd ranges, far larger than the
Earth: their variograms keep rising across the whole continent and never level off, so the fitted range runs away
(their median, 740 807 km, is no guide at all). This is common with climate over a continent, and landschaft's
documentation warns about it: use the result as a guide, not blindly. Here the finite ranges suggest blocks of
the order of 1 000 km; but blocks that large would leave only a handful of blocks for 83 records, and folds of very
unequal sizes. We use **500 km**, a common order of magnitude for climate at this resolution, and check in [part
4](../fitting-models/) how much the choice of folds changes the scores.

!!! tip
    [[sdm.prepare]] takes `folds = { size = "auto" }` to size its blocks by the range of autocorrelation. Read the
    `layers` table first: with runaway ranges like these, give a size yourself.

### Draw the background

```tfgraph
#! caption: Background points drawn inside the accessible area from the predictors kept (node 7), and the table of records and background with their predictor values (node 8). It continues the graph above.
#! fragment: true
#! highlight: background, dataset
[[nodes]]
id = "background"
op = "sdm.background"
inputs = { env = "select.env", occurrences = "thin.occurrences", area = "area.area" }
params = { n = 2000, seed = 1 }

[[nodes]]
id = "dataset"
op = "sdm.dataset"
inputs = { presences = "thin.occurrences", background = "background.points", env = "select.env" }
```

<div class="steps" markdown>

1. **Background** (node 7): add [[sdm.background]]. Connect the predictors kept to `env` (only cells with a value
   in every predictor are drawn), the records to `occurrences` (their cells are left out, `exclude_occurrences`)
   and the area to `area`. Set `n = 2000` and a `seed`. Other ways to restrict the draw: `distance`, the largest
   distance from the records, or a band `[from, to]` (metres on longitude and latitude grids, the grid's units on
   projected ones); `envelope = 0.05`, only cells
   outside the records' 5th to 95th percentiles of every predictor (a background of clearly different conditions,
   sometimes used for pseudo-absences); `bias`, a map of weights (the target-group background of [part
   2](../occurrence-data/#sampling-bias-and-the-background)).
2. **The data set** (node 8): [[sdm.dataset]] takes the presences, the background points and the predictors, and
   returns a table with one row per record: `x`, `y`, `presence` (1 for a record, 0 for a background point) and one
   column per predictor, the value of its cell. Records without a value in some predictor (in the sea of a coarse
   climate layer, for instance) are left out.

</div>

![The accessible area, the background points and the records](/img/tutorials/species-distributions/predictors-and-background/background_area.png "The accessible area of the sloth: the land within 500 km of the 83 thinned records (12.6 million km², less than half the land of the map), 2 000 background points drawn inside it (grey) and the records (red).")

Compare with a background drawn from the whole map:

```tfcompare
caption: Background points drawn from the accessible area (left of the handle) and from the whole map (right). The whole-map background includes the deserts of Mexico, the high Andes and Patagonia, places a sloth has never had access to and that are easy to tell apart from its records.
before: { src: /img/tutorials/species-distributions/predictors-and-background/background_area.png, label: Accessible area }
after: { src: /img/tutorials/species-distributions/predictors-and-background/background_all.png, label: Whole map }
```

## Read the results

What to report:

- the predictors considered and why; those kept and their VIFs (the `selection` output); the threshold;
- how the accessible area was defined (method, distance, any polygons it was cut to) and its size (12.6 million
  km² here);
- the number of background points, and how they were drawn (within the area, with a bias or not, the seed).

A good sign: few predictors (3 to 6 is typical), all with VIFs well below the threshold, each with a reason. A bad
sign: a predictor kept only because it survived the filter, with no ecological meaning; or an accessible area that
spans regions the species could never have reached.

## Settings that change the answer

The size of the accessible area changes the scores more than almost any other choice. The same data, predictors
selected and folds made inside accessible areas of 250 km to the whole map, the same MaxNet fitted to each:

```tfslider
label: Accessible area
caption: Background points (grey) drawn from accessible areas of growing size, and the test AUC of the same MaxNet model over five folds of spatial blocks. The larger the area, the more of the background lies in places obviously unlike the records, and the higher the AUC, although the model is no better at telling suitable from unsuitable places where the sloth could live.
frames:
  - { value: "250 km", src: /img/tutorials/species-distributions/predictors-and-background/area_250.png, stats: { "Test AUC": "0.57", "TSS": "0.25", "Boyce": "0.23" } }
  - { value: "500 km", src: /img/tutorials/species-distributions/predictors-and-background/area_500.png, stats: { "Test AUC": "0.69", "TSS": "0.35", "Boyce": "0.52" } }
  - { value: "1000 km", src: /img/tutorials/species-distributions/predictors-and-background/area_1000.png, stats: { "Test AUC": "0.77", "TSS": "0.47", "Boyce": "0.48" } }
  - { value: "2000 km", src: /img/tutorials/species-distributions/predictors-and-background/area_2000.png, stats: { "Test AUC": "0.81", "TSS": "0.58", "Boyce": "0.52" } }
  - { value: "whole map", src: /img/tutorials/species-distributions/predictors-and-background/area_0.png, stats: { "Test AUC": "0.87", "TSS": "0.69", "Boyce": "0.57" } }
chart:
  type: line
  data: /data/tutorials/species-distributions/predictors-and-background/areas.csv
  x: area
  y: [auc_test, tss, boyce]
  labels: { auc_test: "Test AUC", tss: "TSS", boyce: "Boyce" }
  colors: { auc_test: "#2f6f9f", tss: "#3b8a5a", boyce: "#e8901f" }
  ylabel: Score
```

The test AUC rises from 0.57 to 0.87 as the background spreads from 250 km to the whole map. Has the model become
better? No: the task has become easier. With a background from the whole continent, most background points are in
places no sloth could live, and ranking the records above them is trivial (Lobo et al., 2008). With a background
within 250 km, the model must tell the records apart from places that are, for the most part, just as tropical and
just as wet, a harder and more relevant task. The Boyce index, which compares the records with what the
background offers at each level of suitability, changes much less. Two consequences:

- **scores can only be compared between models with the same background** (and the same folds);
- **the accessible area must be argued for**, not chosen to maximise the AUC.

The predictors also change: inside 250 to 1 000 km the selection keeps bio5, bio7, bio16 and bio17; inside 2 000 km
and over the whole map it also keeps bio8, whose collinearity with the others is a feature of the tropics.

## Common pitfalls

- **A background from the whole map**, chosen because it is the default. Restrict it to an accessible area, and
  say how you chose it.
- **Collinearity measured over the wrong area.** Measure it where the model will be fitted (the `area` input of
  [[sdm.vif]] and [[sdm.select_predictors]]).
- **Trusting the filter over the biology.** A predictor with a low VIF is not thereby relevant.
- **Projecting with predictors whose correlation changes.** Collinear predictors removed today may part ways in a
  future climate; [part 6](../projection-and-extrapolation/) shows how to detect new combinations (ExDet).
- **Asking for more background points than cells.** On coarse grids or small areas, count the cells first.
- **Distances in the wrong units.** [[sdm.accessible_area]], [[sdm.thin]] and the `distance` of
  [[sdm.background]] on longitude and latitude grids take metres; on projected grids the background's distance is
  in the grid's units. Spatial blocks take metres on longitude and latitude records.

## Going further

- [Part 4: Fitting models](../fitting-models/): MaxNet, GLM and random forests over spatial folds.
- [Part 7: The whole preparation in one node](../prepare-in-one-node/): `area`, `predictors` and `background` are
  steps of [[sdm.prepare]].
- The operators: [Species distributions](/operators/species-distributions/).

## References

- Barve, N., Barve, V., Jiménez-Valverde, A., Lira-Noriega, A., Maher, S. P., Peterson, A. T., Soberón, J., & Villalobos, F. (2011). The crucial role of the accessible area in ecological niche modeling and species distribution modeling. *Ecological Modelling*, 222(11), 1810–1819. https://doi.org/10.1016/j.ecolmodel.2011.02.011
- Dormann, C. F., Elith, J., Bacher, S., Buchmann, C., Carl, G., Carré, G., Marquéz, J. R. G., Gruber, B., Lafourcade, B., Leitão, P. J., Münkemüller, T., McClean, C., Osborne, P. E., Reineking, B., Schröder, B., Skidmore, A. K., Zurell, D., & Lautenbach, S. (2013). Collinearity: a review of methods to deal with it and a simulation study evaluating their performance. *Ecography*, 36(1), 27–46. https://doi.org/10.1111/j.1600-0587.2012.07348.x
- Lobo, J. M., Jiménez-Valverde, A., & Real, R. (2008). AUC: a misleading measure of the performance of predictive distribution models. *Global Ecology and Biogeography*, 17(2), 145–151. https://doi.org/10.1111/j.1466-8238.2007.00358.x
- Naimi, B., Hamm, N. A. S., Groen, T. A., Skidmore, A. K., & Toxopeus, A. G. (2014). Where is positional uncertainty a problem for species distribution modelling? *Ecography*, 37(2), 191–203. https://doi.org/10.1111/j.1600-0587.2013.00205.x
- Roberts, D. R., Bahn, V., Ciuti, S., Boyce, M. S., Elith, J., Guillera-Arroita, G., Hauenstein, S., Lahoz-Monfort, J. J., Schröder, B., Thuiller, W., Warton, D. I., Wintle, B. A., Hartig, F., & Dormann, C. F. (2017). Cross-validation strategies for data with temporal, spatial, hierarchical, or phylogenetic structure. *Ecography*, 40(8), 913–929. https://doi.org/10.1111/ecog.02881
- Valavi, R., Elith, J., Lahoz-Monfort, J. J., & Guillera-Arroita, G. (2019). blockCV: an R package for generating spatially or environmentally separated folds for k-fold cross-validation of species distribution models. *Methods in Ecology and Evolution*, 10(2), 225–232. https://doi.org/10.1111/2041-210X.13107
- VanDerWal, J., Shoo, L. P., Graham, C., & Williams, S. E. (2009). Selecting pseudo-absence data for presence-only distribution modeling: how far should you stray from what you know? *Ecological Modelling*, 220(4), 589–594. https://doi.org/10.1016/j.ecolmodel.2008.11.010
