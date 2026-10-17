---
title: "Gravity potentials as drivers: the pull of towns and markets"
short: Gravity potentials
description: Compute the gravitational interaction potential of towns, markets or roads (the sum of their masses over a power of the distance), choose its exponent, units, mask and method, and use it as a driver of land change instead of the distance to the nearest town.
order: 150
level: intermediate
time: 30 min
ops: [surface.gravity, dynamic.distance_to, dinamica.step, map.calc, nlm.percolation, nlm.distance_gradient]
series: Dinamica EGO in Terraflux
part: 6
---

The distance to the nearest town is one of the most common drivers of land change models, and one of the
crudest: a village of 500 people and a city of 50,000 count the same, and a cell between two towns is as remote
as a cell near only one. A **gravity potential** does better. It adds up the pull of every town on every cell,
each in proportion to its size and falling with the distance, the way geographers have measured the reach of
markets and populations since the 1930s. In this part you will compute it with [[surface.gravity]], Dinamica EGO's
*CalcInteractionPotentialMap*, for towns drawn by chance; see what its exponent, its units, its mask and its two
methods do; and use it as the probability of one step of clearing beside the same step driven by the distance to
the nearest town. At the end you will know when a potential is the better driver, how to compute it without edge
effects, and how to put it into a model.

## The idea

### From Newton to markets

The idea that interaction between places falls with the distance between them and grows with their size is
older than land change models. Reilly (1931) proposed a "law of retail gravitation": a town draws trade from the
countryside around it in proportion to its population and in inverse proportion to the square of the distance.
Stewart (1947) mapped the **population potential** of the United States, the sum over all people of one over
their distance, as a measure of how near a place is to everyone else. Hansen (1959) turned it into a measure of
**accessibility**, the sum of the opportunities (jobs, shops) around a place, each divided by a power of the
distance to it, and showed that it predicted where new housing would be built. The same sum, under the name
*interaction potential*, is one of Dinamica EGO's functors, and a driver of its land change models: the pull of
towns, markets or ports on the land around them.

!!! science "Key idea: the interaction potential"
    For every cell \(i\),

    \[ P_i = \sum_{k} \frac{m_k}{d_{ik}^{\,\beta}} \]

    where the sum is over the **sources** \(k\) (the cells of the `sources` map with a value other than 0), \(m_k\)
    is the mass of source \(k\) (its value: a population, a number of buyers, the capacity of a mill), \(d_{ik}\)
    the straight-line distance between the centres of cells \(i\) and \(k\), and \(\beta\) the `exponent` (2 by
    default). The distance is in cells by default. A source's own cell is counted at a distance of **one cell**,
    so that the sum stays finite: the cell of a town and its four direct neighbours get the same pull from it.

    With \(\beta = 0\) every cell gets the total mass of the map (no decay). With \(\beta = 2\) a town's pull at
    10 cells is a hundredth of its pull next door; with \(\beta = 1\), a tenth.

![The potential of three towns with three exponents](/img/tutorials/land-change/gravity-potentials/toy.png "Three towns of 10,000 (top left), 2,000 (bottom) and 4,000 people (right), and their summed potential (log scale; light high, dark low) with exponents 1, 2 and 3. With 1 the pull spreads over the whole map and the large town dominates it; with 3 each town is an island of pull.")

### Why not the distance to the nearest town

A potential differs from the distance to the nearest source in two ways that matter for land change:

- **Size.** A large town pulls harder and farther than a small one. A settler, a cattle buyer or a sawmill near a
  city has more market than one near a hamlet.
- **Sum.** A cell between three towns gets the pull of all three. Clusters of towns create regions of high
  potential that the nearest distance cannot see.

The price is one more setting, the exponent, and masses that must be known (a census, the number of buildings,
night lights).

## When to use it

Use a gravity potential as a driver when the pressure on the land comes from places of different sizes:

- **Towns and markets** as drivers of clearing, as here, or of urban growth.
- **Ports, mills and slaughterhouses**, with their capacity as the mass: where the demand for timber, soy or cattle
  is.
- **The land use itself**: the potential of the cells of a class (all of mass 1) is a smoothed density of that
  class at all distances, an alternative to [[dynamic.share_of]] in a window. Recomputed from the simulated map
  inside a loop, it is a [dynamic driver](/tutorials/land-change/dynamic-drivers/).
- **An attraction** for the [road constructor](/tutorials/land-change/roads-that-grow/): roads built towards
  where the markets pull.

It is *not* a travel time: the distance is a straight line, across rivers and mountains. When the barriers
matter, use a cost distance ([grid distances](/tutorials/#grid-distances), or [[dynamic.distance_to]] with a
cost). And it is not needed when the sources are all alike (wells, a single class of road): then the distance to
the nearest is simpler to read and to fit.

## What you need

- **The sources**: a map whose cells hold the mass of each source (the population of each town, on the cell of
  its centre), and no value (or 0) elsewhere. Rasterize a layer of towns with [[vector.rasterize]], burning the
  population.
- **The study area** and, around it, the sources that act on it: towns just outside the area pull on it too
  (below).
- An **exponent**: from the literature for the process, or fitted (try several, keep the one that best explains
  the observed change).

The models on this page draw their towns by chance and run as they are.

## Build the model

### Step 1: towns and their pull

```tfgraph
#! caption: Twenty-eight towns drawn by chance on a map of 25 by 25 km (nodes 1 to 3) and their interaction potential (node 4).
#! numbered: true
#! highlight: pull
format = "terraflux/1"

[[nodes]]
id = "spots"
op = "nlm.percolation"
params = { rows = 250, columns = 250, resolution = 100, prob = 0.0004, seed = 31 }
note = "Where the towns are: about 1 cell in 2,500."

[[nodes]]
id = "size"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 60, seed = 32 }

[[nodes]]
id = "towns"
op = "map.calc"
inputs = { s = "spots.map", g = "size.map" }
params = { expression = "if s == 1 then round(500 + 40000 * g * g * g) else null" }
note = "Each town's population; no value elsewhere."

[[nodes]]
id = "pull"
op = "surface.gravity"
inputs = { sources = "towns.map" }
params = { exponent = 2 }
```

1. **`spots`** ([[nlm.percolation]]) sets each cell of a 250 x 250 map of 100 m cells to 1 with a probability of
   0.0004: 28 cells, the towns.
2. **`size`** ([[nlm.gaussian_field]]) is a smooth surface from 0 to 1, so that towns near each other have similar
   sizes.
3. **`towns`** ([[map.calc]]) gives each town a population of 500 to 40,500, the cube of `size` making most towns
   small and a few large: here 790 to 13,135 people, 82,562 in all, and 4 towns of 5,000 or more holding 36% of
   them. Every other cell has no value, which [[surface.gravity]] reads as "not a source".
4. **`pull`** ([[surface.gravity]], subject *Surface metrics* in the library) sums the pull of the 28 towns on
   every cell, with \(\beta = 2\) and distances in cells. Connect `sources` to `towns.map`.

![The potential of the towns](/img/tutorials/land-change/gravity-potentials/pull.png "The potential of the 28 towns (circles, sized by population), on a log scale: 3.6 in the emptiest corner, 13,100 on the largest town. The pull is high on and around every town, and stays high between towns that are close.")

The values span four orders of magnitude, so look at them, and usually use them, on a log scale: a cell next to
the largest town has 3,600 times the potential of the emptiest corner.

### Step 2: the potential as a driver

To see what the potential changes, the model below clears 3% of the forest in one step of [[dinamica.step]],
with a probability made from the potential; a second version of it makes the probability from the distance to
the nearest town.

```tfgraph
#! caption: One step of clearing driven by the potential. The towns and their pull (nodes 1 to 4); a little clearing around each town (nodes 5 to 7); the probability of clearing from the pull (node 8), its stack (9), the rate (10), the patches (11) and the step (12).
#! numbered: true
#! highlight: by_pull, clearing
#! params: false
format = "terraflux/1"

[[nodes]]
id = "spots"
op = "nlm.percolation"
params = { rows = 250, columns = 250, resolution = 100, prob = 0.0004, seed = 31 }

[[nodes]]
id = "size"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 60, seed = 32 }

[[nodes]]
id = "towns"
op = "map.calc"
inputs = { s = "spots.map", g = "size.map" }
params = { expression = "if s == 1 then round(500 + 40000 * g * g * g) else null" }

[[nodes]]
id = "pull"
op = "surface.gravity"
inputs = { sources = "towns.map" }
params = { exponent = 2 }

[[nodes]]
id = "is_town"
op = "map.calc"
inputs = { t = "towns.map" }
params = { expression = "if isnull(t) then 0 else 1" }

[[nodes]]
id = "nearest"
op = "dynamic.distance_to"
inputs = { landscape = "is_town.map" }
params = { classes = 1 }

[[nodes]]
id = "land"
op = "map.calc"
inputs = { d = "nearest.distance" }
params = { expression = "if d <= 300 then 2 else 1" }
note = "1 forest, 2 cleared around the towns."

[[nodes]]
id = "by_pull"
op = "map.calc"
inputs = { land = "land.map", p = "pull.map" }
params = { expression = "if land == 1 then p / (p + 500) else 0" }

[[nodes]]
id = "transitions"
op = "stack.of"
inputs = { 1_to_2 = "by_pull.map" }

[[nodes]]
id = "rates"
op = "markov.matrix"
params = { classes = [1, 2], single = [[0.97, 0.03], [0, 1]] }

[[nodes]]
id = "patches"
op = "patches.settings"
params = { transitions = [{ from = 1, to = 2, expansion = 0.5, expander = [10, 60, 1.5], patcher = [10, 60, 1.5] }] }

[[nodes]]
id = "clearing"
op = "dinamica.step"
inputs = { landscape = "land.map", probability = "transitions.stack", rates = "rates.markov", patches = "patches.patches", step = 1, seed = 1 }
```

- **`is_town`**, **`nearest`** and **`land`** make the land at the start: [[dynamic.distance_to]] gives the
  distance in metres to the nearest town, and every cell within 300 m of a town is cleared (2); the rest is
  forest (1).
- **`by_pull`** ([[map.calc]]) is the probability of clearing, \(P / (P + 500)\): 0.5 where the potential is 500
  (the pull of 500 people one cell away, or of 50,000 people ten cells away), close to 1 where it is much higher,
  close to 0 where it is much lower. A form like this keeps the probability between 0 and 1 whatever the scale of
  the potential. In a real model, put the potential (or its logarithm) among the drivers and let
  [[woe.fit]] or [[glm.fit]] estimate its effect from observed change.
- **`transitions`**, **`rates`**, **`patches`** and **`clearing`** are one step of Dinamica EGO's allocation, as
  in [Expander and Patcher](/tutorials/land-change/expander-and-patcher/): 3% of the forest cleared, half by
  growing existing clearings, half in new patches of 10 ha on average.

The version driven by distance has the same nodes, with `by_pull` computed as
`if land == 1 then exp(-d / 1500) else 0` from `nearest.distance` (input `d`).

```tfgallery
columns: 2
caption: "The probability of clearing from the potential (left) and from the distance to the nearest town (right). The potential is high around the large towns and between neighbouring towns; the distance makes every town the same."
images:
  - { src: /img/tutorials/land-change/gravity-potentials/prob_pull.png, caption: "p / (p + 500)" }
  - { src: /img/tutorials/land-change/gravity-potentials/prob_distance.png, caption: "exp(-d / 1500)" }
```

## Read the results

```tfcompare
caption: Drag the handle to compare the clearing of one step (magenta) driven by the distance to the nearest town (left) and by the potential (right). Beige, the clearing at the start around each town.
before: { src: /img/tutorials/land-change/gravity-potentials/driver_distance.png, label: Distance to the nearest town }
after: { src: /img/tutorials/land-change/gravity-potentials/driver_pull.png, label: Gravity potential }
```

![Legend](/img/tutorials/land-change/gravity-potentials/legend.png)

Both runs clear the same 1,851 ha, and both put it around the towns: the mean distance of the new clearing to the
nearest town is 0.7 km in both. The difference is *which* towns. With the distance, every town draws about the
same share: 16% of the new clearing is next to one of the 4 towns of 5,000 people or more, which are 14% of the
towns. With the potential, 30% is, close to their 36% of the population: the clearing follows the people, not the
number of places.

Which is right is a question for the data, not for the method. Fit both on an observed period and compare them
on another one ([Validating a simulation](/tutorials/land-change/validating-a-simulation/)); in weights of
evidence, compare the contrast of the two variables ([Weights of evidence](/tutorials/land-change/weights-of-evidence/)).
Report the masses and their source, the exponent and how it was chosen, the units, and whether the potential
was recomputed during the simulation.

## Settings that change the answer

### The exponent

```tfslider
label: Exponent β
caption: The potential of the 28 towns with exponents of 0.5, 1, 2 and 3, each on its own log scale. The dashed line is the transect of the chart below.
frames:
  - { value: 0.5, src: /img/tutorials/land-change/gravity-potentials/beta_0.5.png, stats: { "Highest / lowest": "3.4" } }
  - { value: 1, src: /img/tutorials/land-change/gravity-potentials/beta_1.png, stats: { "Highest / lowest": "31" } }
  - { value: 2, src: /img/tutorials/land-change/gravity-potentials/beta_2.png, stats: { "Highest / lowest": "3,640" } }
  - { value: 3, src: /img/tutorials/land-change/gravity-potentials/beta_3.png, stats: { "Highest / lowest": "370,000" } }
start: 2
```

```tfchart
type: line
data: /data/tutorials/land-change/gravity-potentials/transect.csv
x: km
y: [b0.5, b1, b2, b3]
labels: { b0.5: "β = 0.5", b1: "β = 1", b2: "β = 2", b3: "β = 3" }
colors: { b0.5: "#1f5f3a", b1: "#3b8a5a", b2: "#d9a441", b3: "#b4508a" }
xlabel: Distance along the transect (km)
ylabel: Potential / its largest value
height: 300
caption: The potential along a west-east line through the largest town (at 7.4 km), divided by its largest value on the line. The town's cell and its two neighbours on the line share the top (all at a distance of one cell); two cells away the potential falls to a half with β = 1, a quarter with β = 2 and an eighth with β = 3, while with β = 0.5 it never falls below a third on the whole line.
```

The exponent is how far the pull reaches. With 0.5 the map is almost flat: every cell is "near" all the towns, and
the potential is little more than a regional trend. With 3 it is a set of peaks: only the cells right beside a
town feel it. Reilly's 2 is the usual default, and Dinamica EGO's (Stewart's population potential used 1). Choose it as you would choose the
range of a weights of evidence: try 1, 2 and 3 and keep the one whose potential best separates the cells that
changed from those that did not. A useful check: with a large exponent the potential behaves like the distance to
the nearest town (weighted by size); with a small one, like a smooth density of people.

### Units: cells or map units

With `unit = "cells"` (the default, Dinamica EGO's) the distances are counted in cells; with `unit = "map"`, in
map units (metres), and a source's own cell is at the square root of the cell's area. On a map of square cells
the two differ by a constant factor, the side of a cell to the power \(-\beta\): here every value with
`unit = "map"` is exactly \(100^{-2} = 10^{-4}\) of the value in cells. For a driver fitted by weights of evidence
or a regression the factor does not matter, but it does when a rule is written by hand (such as `p / (p + 500)`
above) or when maps of different resolutions are compared: use `map` units then.

### The mask and the edge of the map

A potential computed only from the towns *inside* a study area misses the towns just outside it, and cells near
the boundary look emptier than they are. The fix is to put the sources of a larger area on the map and to use the
`mask` to compute the potential only where you need it: the potential is missing where the mask is missing, but
the sources under the missing cells still count.

```tfgraph
#! caption: The potential inside a study area of 15 by 15 km (node study), with every town on the map counted (pull) and with only the towns inside (pull_inside).
#! highlight: pull, pull_inside
#! params: false
format = "terraflux/1"

[[nodes]]
id = "spots"
op = "nlm.percolation"
params = { rows = 250, columns = 250, resolution = 100, prob = 0.0004, seed = 31 }

[[nodes]]
id = "size"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 60, seed = 32 }

[[nodes]]
id = "towns"
op = "map.calc"
inputs = { s = "spots.map", g = "size.map" }
params = { expression = "if s == 1 then round(500 + 40000 * g * g * g) else null" }

[[nodes]]
id = "frame"
op = "nlm.distance_gradient"
params = { rows = 250, columns = 250, resolution = 100, origin = [51, 200, 51, 200], nlmr_compat = false, rescale = false }

[[nodes]]
id = "study"
op = "map.calc"
inputs = { d = "frame.map" }
params = { expression = "if d == 0 then 1 else null" }
note = "The study area; no value around it."

[[nodes]]
id = "inside"
op = "map.calc"
inputs = { t = "towns.map", s = "study.map" }
params = { expression = "if isnull(s) then null else t" }
note = "Only the towns of the study area."

[[nodes]]
id = "pull"
op = "surface.gravity"
inputs = { sources = "towns.map", mask = "study.map" }
params = { exponent = 2 }

[[nodes]]
id = "pull_inside"
op = "surface.gravity"
inputs = { sources = "inside.map", mask = "study.map" }
params = { exponent = 2 }
```

```tfgallery
columns: 3
caption: "The potential in the study area (the square) with all 28 towns counted (left) and with only the 8 inside it (middle), on the same log scale, and the share of the potential the second misses (right)."
images:
  - { src: /img/tutorials/land-change/gravity-potentials/mask_all.png, caption: "All towns, mask" }
  - { src: /img/tutorials/land-change/gravity-potentials/mask_inside.png, caption: "Towns inside only" }
  - { src: /img/tutorials/land-change/gravity-potentials/mask_loss.png, caption: "% missed" }
```

Without the 20 towns around it, the potential of the study area is underestimated by 40% at the median cell, and
by more than 25% in 72% of its cells; near the boundary, up to the whole of it. With \(\beta = 2\), towns several
kilometres outside still matter. Draw the sources over a buffer around the study area at least as wide as the
distance at which the pull of the largest town falls below what matters to you (with \(\beta = 2\), a town of
10,000 pulls with 1 at 100 cells).

### Direct sums or FFT

`method` chooses how the sum is computed. `direct` adds every pair of a source and a cell, in a fixed order, so
the result is the same to the last bit with any number of threads; its time grows with the number of sources times
the number of cells. `fft` computes the same sum as a convolution by fast Fourier transform, whose time depends
only on the size of the map, with rounding errors of about \(10^{-15}\) of the largest term. `auto` (the default)
takes `direct` up to 2,000 sources or \(10^8\) source-cell pairs, `fft` beyond.

| 12,724 sources on 62,500 cells (the cleared cells of a map, mass 1) | Run time (4 threads) |
|---|---:|
| `direct` | 0.37 s |
| `fft` | 0.11 s |

The two maps are identical to the precision of the files written (float32). The difference grows with the map:
for the potential of a land use class on a map of millions of cells, recomputed at every step of a simulation,
`fft` (or `auto`) is the only practical choice.

## Common pitfalls

- **Zeros instead of no value.** A cell with 0 is not a source, but a cell of mass 1 everywhere would be: make
  sure the sources map has no value (or 0) off the sources. A rasterized layer of towns with a background of 1
  makes every cell a town.
- **Masses of different kinds.** The masses must be of one kind (people, or tonnes, or jobs). A potential of
  towns weighted by population and of mills weighted by capacity are two drivers, not one sum.
- **The edge of the map.** See the mask, above: put the sources of a buffer on the map.
- **Distances in degrees.** With `unit = "map"` on a map in degrees, the distances are in degrees, which are not
  the same on the ground along a row and along a column. Project the maps first, or use cells on a projected
  grid.
- **Using the raw potential in a rule.** Its values span orders of magnitude; a linear rule on them (a
  probability proportional to \(P\)) gives almost everything to the cells next to the largest source. Use its
  logarithm, a saturating form, or fitted weights.
- **A static potential of a moving class.** If the sources are the cells of a land use the model changes, compute
  the potential inside the loop, from `@landscape`, as any [dynamic driver](/tutorials/land-change/dynamic-drivers/).
- **Comparing with Dinamica EGO.** Dinamica EGO computes each term and the sum in single precision, and scales
  the potential to the range of an integer map when asked for one; Terraflux sums in double precision and never
  scales. Expect relative differences of about \(10^{-7}\), more with many sources.

## Going further

- [Roads that grow](/tutorials/land-change/roads-that-grow/): the potential as an attraction of the road
  constructor, roads built towards the markets.
- [Dynamic drivers](/tutorials/land-change/dynamic-drivers/): how to recompute a driver inside the loop and put
  it back into the drivers.
- [Weights of evidence](/tutorials/land-change/weights-of-evidence/): how to fit the effect of the potential on
  observed change.
- [Taking a stock](/tutorials/land-change/taking-a-stock/): a probability of harvest from the pull of towns
  (fuelwood around villages).
- The operators: [[surface.gravity]], [[dynamic.distance_to]], [[dinamica.step]].

## References

- Hansen, W. G. (1959). How accessibility shapes land use. *Journal of the American Institute of Planners*, 25(2), 73–76. https://doi.org/10.1080/01944365908978307
- Reilly, W. J. (1931). *The law of retail gravitation*. New York: Knickerbocker Press.
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Stewart, J. Q. (1947). Empirical mathematical rules concerning the distribution and equilibrium of population. *Geographical Review*, 37(3), 461–485. https://doi.org/10.2307/211132
