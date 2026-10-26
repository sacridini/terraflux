---
title: "Weights of evidence: where change happens"
short: Weights of evidence
description: Learn where each transition is likely from its drivers, as Dinamica EGO does with weights of evidence, and read, check and edit the weights.
order: 100
level: intermediate
time: 45 min
ops: [woe.fit, woe.probability, woe.weights, markov.fit, validate.kappa, raster.calc]
series: Dinamica EGO in Terraflux
part: 1
---

A Markov chain says how many cells of forest become grassland each year; it says nothing about *which* cells.
Dinamica EGO answers that with **weights of evidence**: every driver (distance to roads, slope, elevation...) is
cut into ranges, and each range gets a weight that says how much more, or less, often the transition happened
there than elsewhere. Summed over the drivers, the weights give each cell a probability of each transition. In
this first part of the series on Dinamica EGO you will fit the weights of Sibuyan's transitions with [[woe.fit]],
read them as bars, turn them into maps of probability with [[woe.probability]], check that the drivers do not
say the same thing twice, and edit the weights by hand in the application, as Dinamica EGO users do.

## The idea

### Dinamica EGO

DINAMICA was created at the Universidade Federal de Minas Gerais (Brazil) by Soares-Filho et al. (2002) to
simulate deforestation in an Amazonian colonisation frontier; it was then used for the scenarios of the whole
Amazon basin (Soares-Filho et al., 2006) and for many studies of deforestation, urban growth and regrowth since.
Its current form, **Dinamica EGO** (Environment for Geoprocessing Objects), is a platform where models are graphs
of operators (Rodrigues & Soares-Filho, 2017; Ferreira et al., 2019), much like Terraflux's. Its classic land
change model has three pieces: transition rates from a Markov chain (the [previous series](/tutorials/land-change/transition-rates-markov-chains/)),
**weights of evidence** for the probabilities (this part) and the **Expander and Patcher** for the allocation
(the [next part](/tutorials/land-change/expander-and-patcher/)). Terraflux reproduces the method from its
publications and from measurements of the program, through the landschaft library.

### Evidence for a transition

Weights of evidence come from mineral exploration, where they were developed to combine maps of geological
evidence (faults, rock types, geochemical anomalies) into a map of where a deposit is likely (Bonham-Carter,
1994). In land change, the "deposit" is a transition, and the evidence is the drivers.

Take one transition, forest to grassland on Sibuyan, and one driver, the distance to roads. Of the 4,723 cells of
forest in 1997, 320 became grassland by 2011: on average, 6.8%. But not evenly:

![The forest of 1997 by its distance to roads, and the cells that became grassland](/img/tutorials/land-change/weights-of-evidence/evidence.png "The forest of 1997, darker nearer a road, and the cells that became grassland by 2011 (orange). Of the 918 cells of forest within 1 km of a road, 177 became grassland (19%); of the 1,995 more than 3 km away, 1.")

Within 1 km of a road, 19% of the forest became grassland; beyond 3 km, almost none. The distance to roads is
*evidence* for the transition. Weights of evidence turn this into a number per range of the driver.

!!! science "Key idea: the weight of a range"
    For a transition (here forest to grassland) and a range \(B\) of a driver (here 0 to 250 m from a road),
    with \(D\) the event "the cell made the transition":

    \[ W^+ = \ln \frac{P(B \mid D)}{P(B \mid \bar D)} = \ln \frac{n_B / n}{(b - n_B) / (a - n)} \]

    - \(a\): the cells that could make the transition (the cells of forest at the start), here 4,723;
    - \(n\): the cells that made it, here 320;
    - \(b\): the cells of \(a\) in the range, here 128;
    - \(n_B\): the cells of \(n\) in the range, here 24.

    So \(W^+ = \ln \frac{24/320}{104/4403} = \ln 3.18 = 1.155\): the cells that became grassland are 3.2 times
    more often in that range than those that did not. \(W^+ > 0\) attracts the transition, \(W^+ < 0\) repels it,
    \(W^+ = 0\) is neutral. The **contrast** \(C = W^+ - W^-\), where \(W^-\) is the same ratio for the cells
    *outside* the range, measures how strongly the range separates the two groups, and the weight is
    **significant** when \(|C|\) exceeds 1.96 times its standard deviation
    \(\sqrt{1/n_B + 1/(b-n_B) + 1/(n-n_B) + 1/(a-b-n+n_B)}\) (a 95% test).

The weights of the drivers are then added up, cell by cell, and turned into probabilities:

!!! science "Key idea: from weights to probabilities"
    For a cell \(x\) of class \(i\), with \(S_{ij}(x)\) the sum, over the drivers, of the \(W^+\) of the range
    the cell falls in for transition \(i \to j\) (a driver missing at the cell adds nothing):

    \[ P_{ij}(x) = \frac{e^{S_{ij}(x)}}{1 + \sum_k e^{S_{ik}(x)}} \]

    the sum running over the transitions from class \(i\). This is what [[woe.probability]] computes, as Dinamica
    EGO's *CalcWOfEProbabilityMap*. The probabilities of the transitions from a class are computed together, so a
    cell likely to become coconut is, accordingly, less likely to become grassland.

Adding the weights assumes that the drivers are **conditionally independent** given the transition: that each
brings evidence the others do not (Bonham-Carter, 1994; Agterberg & Cheng, 2002). Two drivers that say the same
thing (the distance to roads and the distance to the coast, on an island whose roads follow the coast) count the
same evidence twice. A [section below](#are-the-drivers-independent) checks it.

### Ranges of a continuous driver

A categorical driver (a soil type, a protected area) has one range per category. A continuous one (a distance, a
slope) has to be cut. Dinamica EGO does it in two stages, and so does [[woe.fit]]:

1. **Buffers.** The driver is cut into thin slices of width `increment` (250 m for distances, 2° for slope...),
   counting in each the cells that could change and those that did.
2. **Generalisation.** Neighbouring slices with a similar effect are merged. Along the cumulative curve of the
   slices, a new range starts where the direction of the curve deviates from that of the range so far by more
   than `tolerance` degrees (5 by default), or where the range would exceed `max_delta` cells; no range is
   smaller than `min_delta` cells.

The result is a handful of ranges per driver, each with its weight, as the bars below show.

## When to use it

Weights of evidence suit transition-based models (one set of weights per transition, which is what Dinamica's
allocation needs) and a reader who wants to *see* the effect of each driver: the bars of weight against distance
*are* the effect of distance. They need no assumption about the shape of a response (a weight per range can go up
and down), and they can be edited by hand, which is how experts bring knowledge the data lack.

Their limits: they ignore interactions between drivers (the effect of slope may depend on the distance to roads),
they assume conditional independence, and their ranges come from a fixed cut. A logistic regression per class
(CLUE-S) or a random forest (PLUS) are the alternatives in Terraflux; the [drivers of land
change template](#what-you-need) fits both weights and regressions on the same data, to compare.

Kolb et al. (2013) compared weights of evidence and other potential models in Dinamica EGO for a landscape of
southern Mexico; Almeida et al. (2003) used them for the urban growth of a Brazilian city; Mas et al. (2014)
compared the software packages that implement such models.

## What you need

- The two maps of the calibration period (the land use of 1997 and 2011, from
  [part 1 of the foundations](/tutorials/land-change/what-land-change-models-are/)).
- The **drivers**, a stack: a raster with one band per driver, each band named (`sibuyan_drivers.tif`, 13 bands).
  Distances in metres, slope in degrees, the soils as 0 and 1.
- The Markov chain of the period ([[markov.fit]]): its transitions with a rate are those the weights are fitted
  for.
- Alternatively, the template **Drivers of land change** draws its own map and drivers and fits the weights at
  once.

## Build the model

```tfgraph
#! caption: The weights of evidence of Sibuyan: the two dates, the drivers and the chain feed woe.fit; woe.probability turns the weights into a map per transition.
#! numbered: true
#! highlight: weights, probability
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }
note = "Land use at the start of the calibration period."

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }
note = "And at its end."

[[nodes]]
id = "drivers"
op = "raster.read"
params = { path = "sibuyan_drivers.tif" }
note = "The 13 drivers, a band each; the stack output is all of them."

[[nodes]]
id = "chain"
op = "markov.fit"
inputs = { t0 = "land_1997.raster", t1 = "land_2011.raster", steps = 14 }
note = "The transitions with a rate per step: those the weights are fitted for."

[[nodes]]
id = "weights"
op = "woe.fit"
inputs = { t0 = "land_1997.raster", t1 = "land_2011.raster", drivers = "drivers.raster", markov = "chain.markov" }
params = { ranges = { dist_sea = 250, dist_roads = 250, dist_urban = 250, dist_streams = 250, elevation = 50, slope = 2, aspect = 30, pop_density = 50 }, categorical = ["diorite", "ultramafic", "sediments", "no_erosion", "moderate_erosion"] }
note = "The weights of every transition on the ranges of every driver."

[[nodes]]
id = "probability"
op = "woe.probability"
inputs = { model = "weights.model", landscape = "land_1997.raster", drivers = "drivers.raster" }
note = "The probability of each transition in each cell of its origin class."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "probability.probability" }
params = { title = "Transition probabilities" }
```

<div class="steps" markdown>

1. **The data** (nodes 1 to 3): the two dates of land use (bands `1997` and `2011`) and the drivers. Use the
   drivers' `stack` output: every band, by its name.
2. **The chain** (node 4): [[markov.fit]] with `steps = 14`. Connecting its output to the weights' `markov` input
   fits weights for the transitions with a positive rate per step (on Sibuyan, ten). Without it, [[woe.fit]] fits
   every transition observed; its `transitions` setting (`[[1, 2], [1, 3]]`) names them instead.
3. **The weights** (node 5): add [[woe.fit]] from the library (subject *Weights of evidence*) and connect `t0`,
   `t1`, `drivers` and `markov`. Its settings, in the Node pane:
    - `ranges`: per driver, the width of the buffers (an `increment` in the driver's units), `"categorical"`, a
      list of limits (`[0, 500, 1000, 3000, 8000]`, used as they are, without generalisation), or a table of
      options (`{ increment = 250, tolerance = 3 }`). Without it every continuous driver is cut into buffers of a
      hundredth of its range.
    - `categorical`: the drivers whose values are categories, a range each (here the five soil maps of 0 and 1).
    - `increment`, `min_delta`, `max_delta`, `tolerance`: the defaults of every driver (automatic, 1 cell, 100,000
      cells, 5°).
    - `fix_abnormal`, `possible`, `weights`: below.
4. **The probabilities** (node 6): [[woe.probability]] reads the weights, a map of land use and the drivers. Its
   output is a stack with a layer per transition (`1_to_2`, `1_to_3`...), the probability in every cell of the
   origin class and no value elsewhere. Here it is computed on 1997; inside a simulation it is computed on the map
   of every step (part 3).
5. Run (++f5++). Select node 5 to see the weights (below), node 6 or the plot for the maps.

</div>

From the command line, make the report an output and run the model:

```toml
[outputs.weights]
from = "weights.table"
```

`terraflux run weights.toml` writes `output/weights.csv`, Dinamica EGO's weights report: a row per range of each
driver for each transition, with `from`, `to`, `variable`, `lower`, `upper`, `possible` (\(b\)), `executed`
(\(n_B\)), `weight` (\(W^+\)), `contrast` and `significant` (1 or 0). The `variable` column numbers the drivers
in alphabetical order of their names (0 `aspect`, 1 `diorite`, 2 `dist_roads`, 3 `dist_sea`...).

## Read the results

### The bars of weights

For each transition and driver, the weights of the ranges in bars: positive bars attract the transition, negative
ones repel it, orange bars are significant, grey ones are not.

![Weights of evidence of forest to grassland for four drivers](/img/tutorials/land-change/weights-of-evidence/bars_1_3.png "Forest to grassland: the weights of the ranges of the distance to roads, the distance to the sea, elevation and slope. Grassland replaced forest near roads, 1 to 3 km from the coast, below 300 m and on gentle slopes.")

Read forest to grassland. **Roads**: the first 2.25 km attract the transition (\(W^+\) of 1.2 and 0.8, both
significant), 2.5 to 3.75 km repel it strongly (−1.5 and −2.6), and beyond 3.75 km there is no bar: no cell
became grassland there, a case discussed below. **The sea**: grassland was most likely 1.2 to 3.2 km inland, and very
unlikely beyond 5 km (the mountains). **Elevation** and **slope** tell the same
story from another side: low, gentle land. The report of the distance to roads:

| Range (m) | Possible \(b\) | Executed \(n_B\) | \(W^+\) | Contrast | Significant |
|---|---:|---:|---:|---:|:---:|
| 0 – 250 | 128 | 24 | 1.155 | 1.209 | yes |
| 250 – 2,250 | 2,013 | 278 | 0.791 | 2.320 | yes |
| 2,250 – 2,500 | 213 | 11 | −0.289 | −0.301 | no |
| 2,500 – 2,750 | 196 | 3 | −1.542 | −1.578 | yes |
| 2,750 – 3,750 | 772 | 4 | −2.636 | −2.815 | yes |
| 3,750 – 7,500 | 1,401 | 0 | 0 | 0 | no |

The generalisation merged the eight buffers from 250 to 2,250 m into one range: their effect was similar enough.
Coconut, a commercial crop, is tied even more closely to roads:

![Weights of evidence of forest to coconut](/img/tutorials/land-change/weights-of-evidence/bars_1_2.png "Forest to coconut: within 250 m of a road W+ is 2.3 (27 of 128 cells changed); beyond 2 km no forest became coconut.")

and rice to the coastal plain:

![Weights of evidence of forest to rice](/img/tutorials/land-change/weights-of-evidence/bars_1_4.png "Forest to rice: within 1 km of the sea, below 50 m, on slopes under 4°; and in the ranges farthest from the streams.")

The distance to streams surprises: forest became rice mostly 1.5 to 2.25 km from a stream, not next to one. A weight
describes *where* a transition happened, not *why*; whether this is a real pattern (irrigation from elsewhere, a
plain between rivers) or a coincidence of a few cells is a question for someone who knows the island, and a reason
to look at every bar before trusting a fit.

A categorical driver has one bar per category. The ultramafic soils, poor for agriculture, strongly repel forest to
grassland (\(W^+ = -5.2\)) and forest to rice (−2.0); no forest on them became coconut, so that weight is the
0 of a range without transitions (see `fix_abnormal` below):

![Weights of evidence of the ultramafic soils](/img/tutorials/land-change/weights-of-evidence/bars_ultramafic.png "The ultramafic soils (1) against the others (0), for the three transitions from forest.")

### The probability maps

```tfgallery
columns: 3
caption: The probability of each transition from forest in 1997, from the weights (cells of other classes have no value). Each map is the sum of the weights of thirteen drivers, turned into a probability.
images:
  - { src: /img/tutorials/land-change/weights-of-evidence/prob_1_2.png, caption: "Forest to coconut" }
  - { src: /img/tutorials/land-change/weights-of-evidence/prob_1_3.png, caption: "Forest to grassland" }
  - { src: /img/tutorials/land-change/weights-of-evidence/prob_1_4.png, caption: "Forest to rice" }
```

The maps are where the model will place each transition: the allocation (next part) chooses cells in proportion
to them. The probabilities are relative: they rank cells, and the *number* of cells that change comes from the
Markov chain, not from these values. A good sign is that the high probabilities sit where the transition happened
(compare with the map of change in part 1) and that they are spread over more cells than changed (a map that is
high exactly on the changed cells and nowhere else has learned the past by heart).

### What to report

The drivers and why each is there; the transitions modelled; the increments (or the limits) of each continuous
driver; the bars of the most important drivers of each transition (as above); the association between drivers;
and any weight edited by hand, with its reason.

## Settings that change the answer

### The increment

The increment sets the thinnest range a driver can have. Too thin, and each buffer holds a handful of cells: the
weights are noisy, and many ranges are not significant. Too wide, and a real threshold is hidden in a range.
Move the slider through the increments of the distance to roads for forest to grassland:

```tfslider
label: Increment of the distance to roads (m)
caption: The ranges of the distance to roads for forest to grassland, cut from buffers of 75 m (the automatic hundredth of the range) to 1,000 m. Thin buffers give many small, mostly non-significant ranges; from 250 m the ranges are few and the effect clear. Four ranges are significant in every case.
values: [75, 100, 250, 500, 1000]
src: /img/tutorials/land-change/weights-of-evidence/inc_{value}.png
```

With the automatic increment (a hundredth of each driver's range: 75 m for roads, 3.6° for aspect, 0.01 for a
driver of 0 and 1), the fit of Sibuyan has 1,435 ranges; with the increments above, 677. A rule of thumb: an
increment no smaller than the cell size for distances (250 m here), and of a few units for slope (degrees) and
elevation (tens of metres). Drivers coded 0 and 1 should be `categorical`.

### Ranges without transitions: `fix_abnormal`

A range where no cell made the transition (or where every cell did) has no finite weight: \(\ln 0\). By default,
as in Dinamica EGO, its \(W^+\) is set to 0, which reads as *neutral*. Yet 1,401 cells of forest more than 3.75 km
from a road, none of which became grassland, are not neutral: they are strong evidence *against* the transition.
With `fix_abnormal = true`, such a range gets \(-\ln(b\,n + 1)\) (here \(-\ln(1{,}401 \times 320 + 1) = -13.0\)),
and a range where every cell changed \(\ln(b\,(a-n) + 1)\). Use it when the empty ranges are real (remote,
steep, protected land); leave it off when they are only small samples.

### `possible`

By default (`possible = "any"`) the cells that could make a transition are all the cells of its origin class.
With `"stay"`, only those that made it or stayed: the cells that went to *another* class are left out of the
transition's denominator, as some studies prefer when transitions compete.

## Are the drivers independent?

Adding weights double-counts evidence when two drivers carry the same information. Dinamica EGO users check pairs
of drivers, cut into their ranges, with two measures of association between categorical maps: **Cramér's V**,
from the chi-square statistic of their cross-tabulation, and the **joint information uncertainty** (JIU), from
their entropies; both are 0 for independent maps and 1 for maps that determine each other. Studies with Dinamica
EGO commonly treat values above about 0.5 as a sign that one of the two drivers should be dropped or the two
combined.

Terraflux computes both measures in [[validate.kappa]] (MOLUSCE's comparison of two maps of classes). Cut each
driver into classes on the cells of the origin class with [[raster.calc]], and compare the two:

```tfgraph
#! caption: The association of two drivers on the cells of forest of 1997: each cut into classes (500 m), compared by validate.kappa, whose table gives Cramér's V (cramer) and the joint information uncertainty (jiu).
#! highlight: association
format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.drivers]
kind = "stack"
path = "sibuyan_drivers.tif"

[[nodes]]
id = "roads"
op = "raster.calc"
inputs = { lu = "$t0", d = "$drivers" }
params = { expression = "floor(d.dist_roads / 500) if lu == 1 else None" }
note = "The distance to roads in classes of 500 m, on the forest only."

[[nodes]]
id = "sea"
op = "raster.calc"
inputs = { lu = "$t0", d = "$drivers" }
params = { expression = "floor(d.dist_sea / 500) if lu == 1 else None" }
note = "The distance to the sea, the same way."

[[nodes]]
id = "association"
op = "validate.kappa"
inputs = { reference = "roads.map", simulated = "sea.map" }
note = "Cramér's V and the joint information uncertainty of the two."
```

Here the drivers are inputs of the model (`[inputs.drivers]`, a stack), so `d.dist_roads` names a layer of the
stack. The pairs of Sibuyan's continuous drivers, on the forest of 1997:

![Cramér's V and the joint information uncertainty of pairs of drivers](/img/tutorials/land-change/weights-of-evidence/association.png "The association of every pair of continuous drivers on the forest of 1997 (distances in classes of 500 m or 1 km, elevation of 100 m, slope of 5°, population of 100 per km²). The strongest pair, distance to roads and to the sea, has V = 0.40 and JIU = 0.31.")

No pair reaches 0.5. The strongest is the distance to roads with the distance to the sea (V = 0.40), as expected on
an island whose roads run along the coast; elevation and slope come next (0.28). Keeping both members of these
pairs is defensible here, but their weights should be read together: part of the "effect of the sea" is the effect
of the roads near it. The measures depend on the classes the drivers are cut into; cut them as their weights' ranges
are, or in a few classes of similar size.

A coarser check runs whenever the data is read: `terraflux check` and the application's Wizard warn about
drivers that are constant or **collinear** (a Pearson correlation above 0.9 between two layers). On Sibuyan they
find none.

## Edit the weights in the application

Experts often know what the data cannot show: a road built after the calibration period, a reserve where the
weights of a range should be strongly negative, a range made noisy by a few cells. The application shows the
weights of a `woe.fit` node as Dinamica EGO's weights editor does, and every edit becomes a setting of the node.

!!! app "In the application: the weights view"
    Select the `woe.fit` node (with its `model` output shown under the canvas, after a run). The view has:

    - two lists at the top: the **transition** (`1 -> 3`) and the **variable**;
    - the **W+** of each range in **bars**, the significant ones in the accent colour, the others grey;
    - a **table** of the ranges: the range, its **cells** (possible), how many **changed**, its **W+** (a value
      you can drag or type), its **contrast** (marked `(ns)` when not significant), and buttons:
      **merge next** (this range and the next become one), **split** (two ranges, cut at its middle) and, for a
      weight set by hand, **fitted** (back to the weight fitted);
    - **Back to the fit**, when the variable of this transition has edits: its ranges and weights as the fit
      makes them.

    Merging and splitting change the limits of the ranges and run the fit again; a weight typed by hand replaces
    the fitted one. The view is read-only for weights given in the model (`woe.weights`).

An edit is written into the node's settings, so the model file keeps it, readable and versioned. Merging the three
ranges of the distance to roads from 2,250 to 3,750 m for forest to grassland, and setting the weight of its last
range (beyond 3,750 m, where no forest became grassland) to −3 by hand, gives:

```tfgraph
#! caption: The weights after two edits: forest to grassland has its own limits of the distance to roads (ranges["1->3"]), and the weight of its last range is set to -3 (weights["1->3"], the other three as fitted).
#! highlight: weights
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "drivers"
op = "raster.read"
params = { path = "sibuyan_drivers.tif" }

[[nodes]]
id = "weights"
op = "woe.fit"
inputs = { t0 = "land_1997.raster", t1 = "land_2011.raster", drivers = "drivers.raster" }
params = { transitions = [[1, 2], [1, 3], [1, 4]], ranges = { dist_roads = 250, "1->3" = { dist_roads = [0, 250, 2250, 3750, 7500] } }, weights = { "1->3" = { dist_roads = [1.155, 0.791, -1.547, -3.0] } } }
note = "Forest to coconut, grassland and rice; forest to grassland with its own ranges of the distance to roads and its last weight given."
```

`ranges["1->3"]` gives the limits of the ranges of a driver for one transition (the others keep the common
settings); `weights["1->3"]` gives a weight per range. In a model in JSON, `null` in that list keeps a fitted
weight; TOML has no `null`, so in a TOML model file every weight of the list is a number (here the three fitted
ones, 1.155, 0.791 and −1.547, copied from the report). When the ranges change,
weights given for the old ranges no longer fit: the application forgets them, and a model file with the wrong
number of weights stops with an error that says so.

Fitted weights can also be **frozen**: [[woe.weights]] holds a fitted model (`woe.fit`'s `model`, as JSON) and
gives it without fitting, for a model to run on other data with the weights of Sibuyan.

## Common pitfalls

- **Binary drivers left continuous.** A driver of 0 and 1 cut into buffers of 0.01 gives two ranges by luck of
  the generalisation. Declare it `categorical`.
- **The default increment.** A hundredth of each range is rarely a good increment; on Sibuyan it gives more than
  twice as many ranges as the increments chosen above. Choose an increment per driver.
- **Drivers that come from the change.** A distance to the 2011 grassland "explains" the grassland of 2011
  perfectly. Drivers must describe the landscape at the *start* of the period (or be recomputed during the
  simulation: dynamic drivers).
- **Reading non-significant weights.** A grey bar is a weight that could be chance; do not interpret its sign.
- **Overfitting.** Many drivers with thin ranges reproduce the calibration period well and the future badly. A
  probability map that lights up exactly the cells that changed, and nothing else, is a warning.
- **The drivers' names.** The report numbers the drivers in alphabetical order; a stack of unnamed bands has
  names like `band_1`. Name the bands before fitting, and keep them in the same order in every map you simulate.

## Going further

- [Expander and Patcher](/tutorials/land-change/expander-and-patcher/): how the probabilities become patches of
  change, the next part.
- [Dinamica EGO on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/): the full model.
- [Validating a simulation](/tutorials/land-change/validating-a-simulation/): whether the weights place change
  where it happened.
- The operators: [[woe.fit]], [[woe.probability]], [[woe.weights]], [[validate.kappa]].

## References

- Agterberg, F. P., & Cheng, Q. (2002). Conditional independence test for weights-of-evidence modeling. *Natural Resources Research*, 11(4), 249–255. https://doi.org/10.1023/A:1021193827501
- Almeida, C. M. de, Batty, M., Monteiro, A. M. V., Câmara, G., Soares-Filho, B. S., Cerqueira, G. C., & Pennachin, C. L. (2003). Stochastic cellular automata modeling of urban land use dynamics: empirical development and estimation. *Computers, Environment and Urban Systems*, 27(5), 481–509. https://doi.org/10.1016/S0198-9715(02)00042-X
- Bonham-Carter, G. F. (1994). *Geographic Information Systems for Geoscientists: Modelling with GIS*. Pergamon.
- Ferreira, B. M., Soares-Filho, B. S., & Pereira, F. M. Q. (2019). The Dinamica EGO virtual machine. *Science of Computer Programming*, 173, 3–20. https://doi.org/10.1016/j.scico.2018.02.002
- Kolb, M., Mas, J.-F., & Galicia, L. (2013). Evaluating drivers of land-use change and transition potential models in a complex landscape in Southern Mexico. *International Journal of Geographical Information Science*, 27(9), 1804–1827. https://doi.org/10.1080/13658816.2013.770517
- Mas, J.-F., Kolb, M., Paegelow, M., Camacho Olmedo, M. T., & Houet, T. (2014). Inductive pattern-based land use/cover change models: a comparison of four software packages. *Environmental Modelling & Software*, 51, 94–111. https://doi.org/10.1016/j.envsoft.2013.09.010
- Rodrigues, H., & Soares-Filho, B. (2017). A short presentation of Dinamica EGO. In M. T. Camacho Olmedo, M. Paegelow, J.-F. Mas, & F. Escobar (Eds.), *Geomatic Approaches for Modeling Land Change Scenarios* (pp. 493–498). Springer. https://doi.org/10.1007/978-3-319-60801-3_35
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Soares-Filho, B. S., Nepstad, D. C., Curran, L. M., Cerqueira, G. C., Garcia, R. A., Ramos, C. A., et al. (2006). Modelling conservation in the Amazon basin. *Nature*, 440(7083), 520–523. https://doi.org/10.1038/nature04389
