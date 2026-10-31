---
title: Projection and extrapolation
short: Projection and extrapolation
description: Project the sloth's model to a warmer, drier climate, map what it gains and loses, and find where the projection rests on conditions the model has never seen, with MESS, MOP and ExDet, and what clamping does there.
order: 60
level: advanced
time: 50 min
template: A species in a warmer climate
ops: [sdm.maxnet_predict, sdm.mess, sdm.mop, sdm.exdet, stack.set, raster.calc, sdm.fit]
series: Species distribution models
part: 6
---

Species distribution models are most often fitted for one reason: to ask where a species could live in another
time or place. Under a future climate, after an introduction to another continent, in the past. This is also where
they are least reliable, because a correlative model knows only the conditions it was fitted to. In this part you will
project the sloth's model to a simple warmer and drier scenario, map the suitability it gains and loses, and then map
the places where the projection is an **extrapolation**: conditions outside anything in the model's records, where
its answer is a guess. You will meet three tools for this (MESS, MOP and ExDet), what clamping does, and how to report
a projection honestly.

We continue with the sloth's model of [part 5](../evaluation-and-thresholds/): MaxNet with `classes = "lqh"` and
`regmult = 2`, fitted to 83 presences and 2 000 background points inside the accessible area, on four predictors:
bio5 (the maximum temperature of the warmest month), bio7 (the annual range of temperature), bio16 and bio17 (the
rain of the wettest and of the driest quarter).

## The idea

### Projecting a model

A fitted model is a function of the predictors. Projecting it means feeding it **other values of the same
predictors**: the climate of 2070 from a climate model, the climate of another continent, a scenario of your own.
Pearson & Dawson (2003) set out what this assumes: that the species is in equilibrium with today's climate (it lives
wherever the climate suits it), that the relation between climate and suitability stays the same, and that nothing
else (dispersal, competitors, land use, adaptation) changes. None is strictly true, and a projection is a map of where
the **climate** would suit the species, not a forecast of where it will be.

In this tutorial the "future" is a simple scenario, not a climate projection: **bio5 two degrees warmer** and
**bio17 (the driest quarter's rain) 20 % lower** everywhere. Real studies use the outputs of several climate models
for each scenario (WorldClim distributes them on the same grids as its present climate; Fick & Hijmans, 2017).

!!! app "The climate of the future, from the data catalog"
    **File > Data catalog...** adds the bioclimatic variables of a CMIP6 model, scenario and period (WorldClim 2.1's
    futures, or CHELSA 2.1's at 1 km) as a [[data.download]] node on the grid of your present-day predictors, the
    paper to cite in the node's references.

### Extrapolation

Within the conditions of its records and background, a model **interpolates**. Outside them it **extrapolates**, and
there its responses are whatever its mathematics makes of empty space: a GLM's line keeps rising, a tree stays flat,
a hinge keeps its slope. Different algorithms that agree today may diverge wildly there (Thuiller et al., 2005;
Elith, Kearney & Phillips, 2010), and future climates with no analogue today are common in projections
(Fitzpatrick & Hargrove, 2009). Two kinds of new conditions matter (Mesgaran, Cousens & Webber, 2014):

- **novel values**: a predictor outside the range of the reference data (hotter than the hottest record or
  background point);
- **novel combinations**: every predictor within its range, but in a combination never seen (hot *and* wet where the
  reference was hot and dry).

!!! science "Key idea: MESS"
    The multivariate environmental similarity surface (Elith et al., 2010) compares each predictor in a cell with its
    distribution in the reference data. With \(f\) the percentage of reference values below the cell's value \(p\),
    and \(\min\) and \(\max\) the reference's range, the similarity of that predictor is

    \[ S = \begin{cases} 100\,\dfrac{p - \min}{\max - \min} & f = 0 \\[4pt] 2f & 0 < f \le 50 \\[2pt]
       2(100 - f) & 50 < f < 100 \\[2pt] 100\,\dfrac{\max - p}{\max - \min} & f = 100 \end{cases} \]

    100 at the median, falling towards the edges of the range, and **negative outside it** (by how far, in percent
    of the range). The MESS of a cell is its lowest similarity over the predictors: negative where at least one
    predictor is outside anything the model saw.

**MOP**, the mobility-oriented parity (Owens et al., 2013), measures how far each new condition is from its nearest
reference conditions (the mean distance to the closest 1 % of them) and flags the cells outside the ranges.
**ExDet** (Mesgaran et al., 2014) separates the two kinds of novelty: **NT1**, negative where values fall outside the
ranges; and **NT2**, for cells inside the ranges, the Mahalanobis distance (a distance that accounts for the
correlation between predictors) relative to the largest within the reference, above 1 for a combination never seen.

**Clamping** treats extrapolation at the source: before predicting, every predictor is held at the edge of its
training range, so responses stay flat beyond it (Phillips et al., 2006). MaxNet clamps by default.

## When to use it

- **Climate change**: the suitability of a species under future climates, the gains and losses of its range, the
  places that stay suitable (refugia), as in the many studies following Thuiller et al. (2005) for Europe's plants.
- **Invasions**: where an introduced species could establish, from its native range.
- **The past**: palaeoclimates, to understand today's distributions.
- **Always with a map of extrapolation** beside the projection: MESS, MOP or ExDet over the same new conditions.

What it is *not*: a forecast. Dispersal (can the species reach the new places?), biotic interactions, land use and
evolution are outside the model. For dispersal, combine the projection with a barrier or a distance limit (the
accessible area of [part 3](../predictors-and-background/) is a first step); for land use, see the [land change
simulations](/tutorials/land-change/).

## What you need

- A fitted model and the **predictors under the new conditions**, as a stack with the **same names** as the
  predictors the model was fitted on (here `bio5`, `bio7`, `bio16`, `bio17`), on any grid.
- The **reference data** the model knows: its records table (the `records` of [[sdm.prepare]]: presences and
  background) or a stack of the conditions of the training area.
- The template **Templates → Species → A species in a warmer climate** does all of this on predictors drawn by chance:
  a model, its response curves and importance, the suitability today and 2 °C warmer, their difference and the MESS.

## Build the model

### The new conditions and the projection

```tfgraph
#! caption: The sloth's model (nodes 3 and 4) projected to a climate in which bio5, the maximum temperature of the warmest month, is 2 °C higher (nodes 5 to 7), and the MESS of that climate against the records (node 8).
#! numbered: true
#! highlight: hotter, future, tomorrow, mess
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
id = "data"
op = "sdm.prepare"
inputs = { occurrences = "records.vector", env = "climate.raster" }
params = { thin = 50000, area = 500000, background = 2000, folds = { size = 500000, k = 5 }, seed = 1 }

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
params = { regmult = 2, classes = "lqh" }

[[nodes]]
id = "hotter"
op = "raster.calc"
inputs = { e = "data.env" }
params = { expression = "e.bio5 + 20" }
note = "bio5 is in tenths of a degree: +2 °C."

[[nodes]]
id = "future"
op = "stack.set"
inputs = { stack = "data.env", map = "hotter.map" }
params = { name = "bio5" }

[[nodes]]
id = "tomorrow"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "future.stack" }

[[nodes]]
id = "mess"
op = "sdm.mess"
inputs = { reference = "data.records", env = "future.stack" }
params = { full = true }
```

<div class="steps" markdown>

1. **The model** (nodes 1 to 4), as in parts 4 and 5.
2. **The new conditions** (nodes 5 and 6). With a scenario's rasters you would read them with [[raster.read]] and
   build a stack named like the model's predictors. Here they are made from today's: [[raster.calc]] takes the stack of
   predictors (`data.env`) on an input named `e` and reads a layer as `e.bio5`; the expression adds 20, since
   WorldClim's temperatures are in tenths of a degree. [[stack.set]] puts the result back into the stack **under
   the name `bio5`**, replacing today's layer. The model finds its predictors by name, so the name is what matters.
3. **The projection** (node 7): [[sdm.maxnet_predict]] with the same model and the new stack. `clamp` (true by
   default for MaxNet, as fitted) holds every predictor within its training range; `clamp = false` lets the
   responses run on.
4. **The extrapolation** (node 8): [[sdm.mess]] with the reference conditions (`data.records`: the predictors at the
   presences and the background) on `reference` and the new conditions on `env`. Its `result` is a stack: `mess`,
   and with `full = true` each predictor's own similarity (`bio5_mess`...).

</div>

The figures add a second change, bio17 times 0.8, with another [[raster.calc]] and [[stack.set]] on the output of node 6,
and compare the projection with the suitability today:

```tfgraph
#! caption: The drier step (nodes 1 and 2), the suitability today (3), the change (4), and the other two measures of extrapolation (5 and 6). It continues the graph above.
#! fragment: true
#! numbered: true
#! highlight: drier, future2, shift, mop, exdet
[[nodes]]
id = "drier"
op = "raster.calc"
inputs = { e = "data.env" }
params = { expression = "e.bio17 * 0.8" }

[[nodes]]
id = "future2"
op = "stack.set"
inputs = { stack = "future.stack", map = "drier.map" }
params = { name = "bio17" }

[[nodes]]
id = "today"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

[[nodes]]
id = "shift"
op = "raster.calc"
inputs = { a = "today.prediction", b = "tomorrow.prediction" }
params = { expression = "b - a" }

[[nodes]]
id = "mop"
op = "sdm.mop"
inputs = { reference = "data.records", env = "future2.stack" }
params = { type = "detailed" }

[[nodes]]
id = "exdet"
op = "sdm.exdet"
inputs = { reference = "data.records", env = "future2.stack" }
```

(With the drier step, `tomorrow` and `mess` take `future2.stack` instead of `future.stack`.)

[[sdm.mop]]'s settings: `distance` (`euclidean`, the default, or `mahalanobis`), `percentage` (the share of the
nearest reference conditions averaged, 1 %), `scale` and `center` (standardise the predictors first; with
Euclidean distances on predictors of different units, set both, or temperature in tenths of a degree and rain in
millimetres weigh differently), and `type`: `basic` (a layer flagging cells outside the ranges), `simple` (how many
predictors are outside), `detailed` (which ones, below and above, coded as sums of powers of 2). [[sdm.exdet]]
returns two layers: `exdet` (NT1 where negative, NT2 elsewhere) and `mic`, the **most influential covariate**: the
place of the predictor that contributes most to the novelty of each cell, counted from 1 in the order of the
predictors (1 for the first, bio1 here), and 0 where the cell shows no novelty.

!!! app "In the application"
    **Templates → Species → A species in a warmer climate** is this model on predictors drawn by chance, with its
    four maps in tabs of their own (today, 2 °C warmer, gains and losses, MESS). Replace the `warmer` node with a
    [[raster.read]] of a scenario's layer, named as the predictor it replaces.

## Read the results

### What changes

```tfcompare
caption: The sloth's suitability today (left of the handle) and in the scenario of a warmer and drier climate (right), on the same scale of 0 (dark) to 1 (yellow); the accessible area outlined and the records in red. Suitability falls across the Amazon lowlands and holds along the Andean foothills, in Central America and on the coast of the Guianas.
before: { src: /img/tutorials/species-distributions/projection-and-extrapolation/today.png, label: Today }
after: { src: /img/tutorials/species-distributions/projection-and-extrapolation/tomorrow.png, label: Warmer and drier }
```

![The change of suitability.](/img/tutorials/species-distributions/projection-and-extrapolation/shift.png "The change of suitability (scenario minus today). Nearly everywhere the sloth's suitability falls, most in the central and eastern Amazon, where the warmest months are already the hottest of its range.")

![Gains and losses at the threshold of maximum TSS.](/img/tutorials/species-distributions/projection-and-extrapolation/gains-losses.png "Suitable and unsuitable cells today and in the scenario, at the threshold of maximum TSS (0.66, part 5). Inside the accessible area, 810 cells are suitable today: 483 stay suitable, 327 (40 %) are lost, none is gained.")

At the threshold of maximum TSS chosen in part 5, 40 % of the suitable cells inside the accessible area are lost and
none is gained. The losses are in the central Amazon, where bio5 is already high. What stays suitable is in the
Chocó, Central America, the Andean foothills and the coast of the Guianas: the places a conservation plan for the
sloth under this scenario would look at first. Outside the accessible area, southern Chile stays suitable to the model
in both climates; it is out of the sloth's reach, which only the accessible area, not the model, knows.

### Where the projection is an extrapolation

```tfcompare
caption: The MESS of today's climate (left of the handle) and of the scenario (right) against the sloth's records and background. Red cells (below 0) have at least one predictor outside the reference's range. Today the novel climates are outside the sloth's world (the deserts and temperate lands of North America, Patagonia); in the scenario they spread into central Brazil and the southern Amazon, where the warmest months become hotter than anything in the reference.
before: { src: /img/tutorials/species-distributions/projection-and-extrapolation/mess-now.png, label: Today }
after: { src: /img/tutorials/species-distributions/projection-and-extrapolation/mess.png, label: Scenario }
```

The numbers, over the 9 775 cells of land:

| | Today | Scenario |
|---|---|---|
| Cells with MESS below 0 | 2 063 | 2 912 |
| ... inside the accessible area | | 225 |
| Novel through bio5 (MESS of bio5 below 0) | | 1 428 |
| Novel through bio7 | | 1 980 |
| Novel through bio16 | | 6 |
| Novel through bio17 | | 0 |

Two kinds of extrapolation hide in these numbers. **bio7** (the annual range of temperature, unchanged in the
scenario) is novel in about 2 000 cells today and in the scenario alike: the temperate lands, where the seasons are far
stronger than anywhere in the tropical reference. The model is asked about them today too, and its answer there
(unsuitable, mostly) rests on no data at all. **bio5** is new: the scenario pushes the hottest places of the sloth's
range beyond the hottest it was recorded or sampled in. Inside the accessible area, 225 cells become novel; their
projected losses are an extrapolation of the model's response at its hot edge. The drier step adds no novelty: the reference
already includes places with almost no rain in the driest quarter.

```tfgallery
columns: 2
caption: Two more views of the same novelty. Left, MOP's distance from each cell's conditions to the nearest 1 % of the reference conditions (grey cells, out of range, have none). Right, ExDet's combined layer (red below 0, NT1: values outside the ranges; blue from 0 to 1, NT2: combinations within the ranges). No cell reaches NT2 above 1: inside the ranges, the scenario creates no combination of predictors the reference lacks.
images:
  - { src: /img/tutorials/species-distributions/projection-and-extrapolation/mop.png, caption: "MOP distance" }
  - { src: /img/tutorials/species-distributions/projection-and-extrapolation/exdet.png, caption: "ExDet (NT1 and NT2)" }
```

The three measures agree on the cells outside the ranges (2 912 in each). ExDet's `mic` says which predictor drives
the novelty: bio5 in 1 002 cells, bio7 in 1 904, bio16 in 6. MOP adds a gradient inside the ranges: the high Andes
and southern Chile are far from any reference condition even where no predictor is out of range.

### Clamping

![The difference made by clamping.](/img/tutorials/species-distributions/projection-and-extrapolation/clamp-diff.png "The projection without clamping minus the projection with it. The largest difference is 0.06, in a handful of cells: MaxNet's regularised responses are nearly flat at the edges of the range, so clamping changes little here. With more complex models, or hinges with steep slopes at the edge, it can change a lot.")

Clamping changes the scenario's map by at most 0.06 and in only 5 cells by more than 0.05. The responses of this
model are already flat at their edges (part 4's response curves). Clamping is not a cure for extrapolation: it replaces
one assumption (the response continues) with another (it stops). Neither is data. Mask or flag the novel cells.

### Reporting a projection

- The scenario: its source (climate models, emission pathway, period), how it was put on the model's grid, and its
  predictors' names and units.
- The projected suitability, and gains, losses and stable areas at a stated threshold.
- **A map of extrapolation** (MESS, MOP or ExDet) with the reference data named, and the share of the projected area
  that is novel. Better still, shade or mask the novel cells on the projection itself.
- Whether the model clamps, and the assumptions (no dispersal limits, no adaptation, the same accessible area).
- With several climate models, the agreement between them (in how many is a cell suitable).

## Settings that change the answer

- **The reference data.** MESS against the records and background (the conditions the model was fitted on) asks
  "is this outside what the model knows?". Against a stack of the whole present map, it asks "is this climate new
  on Earth?", a different question with different answers.
- **The predictors.** Each one is a chance of extrapolation. Collinear predictors that part ways in the future (part
  3) are the usual source of novel combinations, which only ExDet's NT2 or MOP see.
- **MOP's `scale`, `center` and `distance`.** Unscaled Euclidean distances weigh the predictors by their units;
  Mahalanobis distances account for their correlation.
- **Clamping** and the **algorithm**: different algorithms extrapolate differently; an ensemble's spread is one
  measure of the uncertainty of a projection.
- **The threshold**, when gains and losses are counted (part 5).

## Common pitfalls

- **Predictors with different names** in the new stack. The model finds them by name: a layer named `bio5_2070`
  is not `bio5`. Rename with [[stack.set]] (`name`).
- **Different units.** WorldClim's older versions store temperatures in tenths of a degree, the newer in degrees; a
  scenario in the wrong units is a novel climate everywhere.
- **Projecting without mapping extrapolation.** Every projection needs its MESS, MOP or ExDet map beside it.
- **Reading the projection as a forecast.** It maps climatic suitability. Whether the species can get there, and
  survive the competitors and the land use it meets, is another question.
- **Counting gains outside the reach of the species.** Southern Chile is "suitable" today and in the scenario; the
  sloth will not get there. Restrict gains to areas the species can reach.
- **One climate model.** Climate models disagree; use several and report their agreement.

## Going further

- [Part 7: The whole preparation in one node](../prepare-in-one-node/): every setting of [[sdm.prepare]].
- Projected suitability as features of a conservation plan: [Systematic conservation planning](/tutorials/spatial-prioritization/systematic-conservation-planning/).
- The operators: [[sdm.mess]], [[sdm.mop]], [[sdm.exdet]], [[sdm.maxnet_predict]].

## References

- Elith, J., Kearney, M., & Phillips, S. (2010). The art of modelling range-shifting species. *Methods in Ecology and Evolution*, 1(4), 330–342. https://doi.org/10.1111/j.2041-210X.2010.00036.x
- Fick, S. E., & Hijmans, R. J. (2017). WorldClim 2: new 1-km spatial resolution climate surfaces for global land areas. *International Journal of Climatology*, 37(12), 4302–4315. https://doi.org/10.1002/joc.5086
- Fitzpatrick, M. C., & Hargrove, W. W. (2009). The projection of species distribution models and the problem of non-analog climate. *Biodiversity and Conservation*, 18(8), 2255–2261. https://doi.org/10.1007/s10531-009-9584-8
- Mesgaran, M. B., Cousens, R. D., & Webber, B. L. (2014). Here be dragons: a tool for quantifying novelty due to covariate range and correlation change when projecting species distribution models. *Diversity and Distributions*, 20(10), 1147–1159. https://doi.org/10.1111/ddi.12209
- Owens, H. L., Campbell, L. P., Dornak, L. L., Saupe, E. E., Barve, N., Soberón, J., et al. (2013). Constraints on interpretation of ecological niche models by limited environmental ranges on calibration areas. *Ecological Modelling*, 263, 10–18. https://doi.org/10.1016/j.ecolmodel.2013.04.011
- Pearson, R. G., & Dawson, T. P. (2003). Predicting the impacts of climate change on the distribution of species: are bioclimate envelope models useful? *Global Ecology and Biogeography*, 12(5), 361–371. https://doi.org/10.1046/j.1466-822X.2003.00042.x
- Phillips, S. J., Anderson, R. P., & Schapire, R. E. (2006). Maximum entropy modeling of species geographic distributions. *Ecological Modelling*, 190(3–4), 231–259. https://doi.org/10.1016/j.ecolmodel.2005.03.026
- Thuiller, W., Lavorel, S., Araújo, M. B., Sykes, M. T., & Prentice, I. C. (2005). Climate change threats to plant diversity in Europe. *Proceedings of the National Academy of Sciences*, 102(23), 8245–8250. https://doi.org/10.1073/pnas.0409902102
