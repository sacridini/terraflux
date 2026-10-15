---
title: "The ordered allocation"
short: Ordered allocation
description: lulcc's second allocation, after Fuchs et al. (2013), serves the classes one at a time in an order you choose and gives each the cells most suitable for it; run it on Plum Island with terraflux new ordered, compare it with CLUE-S, and see why the order matters.
order: 550
level: intermediate
time: 40 min
template: CLUE-S
ops: [lulcc.step, glm.fit, glm.predict, demand.from_maps, rng.r, lulcc.history, control.repeat, validate.agreement, validate.three_map]
series: "Demand-driven allocation: CLUE-S and lulcc"
part: 6
---

CLUE-S lets every class compete for every cell at once, and adjusts the competition until the areas are right. lulcc
has a second, much simpler way to put a demand on a map: the **ordered allocation** of Fuchs et al. (2013). The
classes are served one after another, in an order you choose, and each takes the cells most suitable for it, or
gives up its least suitable ones, until it has the area the demand asks for. In this last part of the series you will
run it on Plum Island with the regressions and the demand of the earlier parts, compare its maps with CLUE-S's of
[part 4](/tutorials/land-change/clue-s-model/) cell by cell and against the observed map of 1999, and try every order
of the three classes, of which only one meets the demand. At the end you will know when the ordered allocation is
the better tool, how to choose its order, and how to read its warnings.

## The idea

### One class at a time

Fuchs et al. (2013) reconstructed the land use of Europe from 1900 to 2010 on a grid of 1 km. They had, for each
region and decade, the area of each land use from statistics, and needed to place it on a map. Rather than make the
classes compete, they allocated them in turn: the class served first takes the cells best suited to it, and the
next ones share what is left. Moulds et al. (2015) put the same idea into lulcc as the `OrderedModel`, next to CLUE-S,
and Terraflux runs lulcc's algorithm as [[lulcc.step]] with `allocation = { method = "ordered", order = [...] }`.

!!! science "Key idea: the ordered allocation of one step"
    For each class \(u\), in the order given, with \(n_u\) = its demand − its cells on the map:

    - if \(n_u > 0\) (the class **grows**), rank the cells it may take (those of classes not yet served, and those
      released by classes served before) by decreasing suitability \(P_{i,u}\), and give the class the first
      \(n_u\);
    - if \(n_u < 0\) (the class **shrinks**), rank its own cells by increasing suitability and release the first
      \(|n_u|\), for the classes served later to take;
    - then **lock** the class's cells: no class served later can take them.

    Released cells that no later class takes go back to the class that released them.

There is no iteration, no elasticity and no `scale`: each class is placed once, so a step takes a fraction of the
time of a CLUE-S step (1 second for the 14 years of Plum Island against 18). The decision rules of
[part 5](/tutorials/land-change/clue-s-rules/) (conversion matrix, history, neighbourhoods, restrictions) apply as
in CLUE-S; the elasticities do not, as in lulcc.

### Stochastic or deterministic

lulcc's ordered allocation is **stochastic** by default: it does not simply take the \(n_u\) most suitable cells.
It draws a random number \(u_i\) between 0 and 1 for every candidate and keeps those whose suitability passes it
(\(P_{i,u} \ge u_i\) for a growing class), in their rank order, redrawing until at least \(n_u\) pass; the class takes
the first \(n_u\) of them. A cell of suitability 0.9 almost always passes; one of 0.2 rarely does. With
`stochastic = false` (`terraflux new ordered --deterministic`) every candidate passes, and the class takes exactly
the \(n_u\) most suitable cells.

![The two ways of choosing cells](/img/tutorials/land-change/ordered-allocation/toy.png "A growing class needs 5 cells among 14 candidates (a toy drawing). Deterministic: the 5 most suitable. Stochastic: a random number is drawn for each candidate (black marks), and the class takes the first 5, in rank order, whose suitability passes it (dark purple); candidates that pass after the fifth (pale) are not taken.")

### Net change only

The ordered allocation changes a cell only when a class needs it or releases it: it makes the **net** change of the
demand and almost nothing more. CLUE-S makes the net change *plus* every swap its suitabilities and elasticities
allow (the reshuffle of part 4). Real landscapes do both: Fuchs et al. (2015) found that the gross change of European
land, counting every gain and loss, was two to four times its net change, and that reconstructions of net change
alone underestimate how much land changed. Neither allocation is right in general; the comparison below shows what
each gives on Plum Island.

## When to use it

- **Reconstructions and downscaling of statistics**: area per region and year known, location unknown, and no
  reason to invent swaps (Fuchs et al., 2013).
- **Clear priorities between classes**: a scenario where built land takes the best land first and agriculture what
  is left, or a protected class served first so that nothing takes its cells.
- **Fast runs**: thousands of runs for an uncertainty analysis or a calibration, where CLUE-S's iterations are
  too slow.
- **A baseline for CLUE-S**: the same regressions and demand with the simplest allocation, to see what CLUE-S's
  competition adds.

It is a poor choice when classes truly compete for the same land and no order is defensible, or when the swaps
between classes are themselves of interest (shifting cultivation, rotations): CLUE-S, or the Markov chains of
Dinamica EGO, make gross change; the ordered allocation does not.

## What you need

The same as CLUE-S ([part 4](/tutorials/land-change/clue-s-model/#what-you-need)): the observed maps for the demand,
the drivers and the formulas of the regressions, and a seed. Plus an **order**: every class of the demand, once.

!!! app "In the application"
    Open the template **CLUE-S** (**Templates → Land change**), double-click the `simulation` loop and select
    `allocate`: in the Node pane, replace its `allocation` by `{ method = "ordered", order = [2, 1, 3] }` (the
    template's classes are those of its maps) and remove the `elasticity`, which the ordered allocation does not
    use. **Wizard...** writes the whole model: choose *Ordered allocation* in *The model*. Its order is by default
    the classes' own (1, 2, 3); on Plum Island that order falls short of the demand (below), so set it.

## Build the model

### With one command

```bash
terraflux new ordered --observed pie_landuse.tif --times 1985,1991,1999 --drivers pie_drivers.tif \
  --formulas '{"1": ["elevation", "slope"], "2": ["elevation", "slope", "dist_built_1985"], "3": ["elevation", "slope"]}' \
  --order 2,1,3 -o ordered.toml
terraflux run ordered.toml --out ordered
```

`--order` lists the classes in the order they are served: built land (2) first, then forest (1), then other land
(3). Add `--deterministic` for the deterministic allocation, and the options of part 5 (`--allowed`,
`--neighbourhood`, `--restrictions`, `--history`) for rules. The [model written](/data/tutorials/land-change/ordered-allocation/ordered.toml)
is the CLUE-S model of part 4 with one setting changed:

```tfgraph
#! caption: The ordered model of Plum Island as terraflux new ordered writes it. Nodes 1 to 6 are those of CLUE-S (part 4); only the allocation of node 7 differs.
#! params: false
#! numbered: true
#! highlight: allocate
format = "terraflux/1"

[inputs.observed]
kind = "stack"
path = "pie_landuse.tif"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = { landscape = "$start", drivers = "$drivers" }
params = { formulas = { 1 = ["elevation", "slope"], 2 = ["elevation", "slope", "dist_built_1985"], 3 = ["elevation", "slope"] } }

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "$drivers", landscape = "$start" }

[[nodes]]
id = "demand"
op = "demand.from_maps"
inputs = { maps = "$observed" }
params = { times = [1985, 1991, 1999] }

[[nodes]]
id = "rng"
op = "rng.r"
inputs = { seed = "$seed" }

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = { landscape = "$start" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = "demand.steps", landscape = "$start", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", history = "@history", suitability = "suitability.suitability", demand = "demand.demand", step = "@step", rng = "rng.rng" }
params = { allocation = { method = "ordered", order = [2, 1, 3], stochastic = true } }
note = "The ordered allocation (Fuchs et al. 2013): classes served in order, cells drawn by suitability."

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
```

The connections are those of [part 4](/tutorials/land-change/clue-s-model/#build-the-model), and for the same
reasons: the suitability, the demand and the generator come from outside the loop; the map and its history are
carried from year to year. Node by node, what the ordered allocation uses:

- **`suitability`** ([[glm.predict]]): the ranking of the cells, class by class. In CLUE-S the suitabilities of all
  classes are compared within each cell; here each class's suitability is compared across cells. A regression that
  ranks the cells well is all the ordered allocation needs, even if its probabilities are not well calibrated.
- **`demand`** ([[demand.from_maps]]): \(n_u\) of each class at each step.
- **`rng`** ([[rng.r]]): the random numbers of the stochastic draws, R's own, so a run gives lulcc's maps.
- **`allocate`** ([[lulcc.step]]): `allocation = { method = "ordered", order = [2, 1, 3], stochastic = true }`.
  `order` must hold every class of the demand once.

Its `info` table (collected as `steps.csv`) has `iterations` (always 1) and `converged`: 1 if every class got its
demand. When a class cannot, Terraflux warns: "the demand was not met by N cells: too few cells could take a class".

## Read the results

### Ordered against CLUE-S

```tfcompare
caption: "The cells that changed class between 1985 and 1999, by their new class (forest green, built purple, other ochre; grey unchanged): CLUE-S with the elasticities of part 4 (left of the handle) and the ordered allocation, order 2, 1, 3 (right)."
before: { src: /img/tutorials/land-change/ordered-allocation/change_clue.png, label: CLUE-S }
after: { src: /img/tutorials/land-change/ordered-allocation/change_ordered.png, label: Ordered }
```

![Legend](/img/tutorials/land-change/ordered-allocation/change_legend.png)

```tftable
data: /data/tutorials/land-change/ordered-allocation/compare.csv
labels: { model: "Model", first_year: "Changed in 1986", changed: "Changed by 1999", hits: "Hits", misses: "Misses", false_alarms: "False alarms", fom: "Figure of merit", agreement: "Same class as 1999 (%)", seconds: "Run (s)" }
digits: 3
caption: "Both allocations on Plum Island, 1985 to 1999, the same regressions and demand, against the observed map of 1999 (validate.agreement). The observed change was 8,578 cells."
```

The difference is in the amount of change. CLUE-S changed 13,023 cells in its first year and 19,522 by 1999; the
ordered allocation changes about 600 a year, 7,506 by 1999, close to the 8,578 observed. It moves forest to built
land (4,809 cells; 4,250 observed), other land to built land (1,524; 2,248 observed) and other land to forest (1,173;
1,259 observed), and nothing else: no built land lost, no forest to other land (656 observed). Its map agrees with the
observed one in 87.7% of the cells against CLUE-S's 79.0%, because it leaves alone what did not change.

Yet the **figure of merit** (Pontius et al., 2008) is the same (0.062 against 0.063): of the change it simulates, the ordered allocation
puts no larger share in the right cells. It has fewer false alarms (6,264 against 16,892) because it changes less,
and fewer hits (927 against 1,609) for the same reason. The regressions, shared by both, decide where the change
goes.

```tfcompare
caption: "The observed change of 1985-1999 (left of the handle) and the ordered allocation's (right), in the same colours. The amounts are close; the places are mostly not."
before: { src: /img/tutorials/land-change/ordered-allocation/change_observed.png, label: Observed }
after: { src: /img/tutorials/land-change/ordered-allocation/change_ordered.png, label: Ordered }
```

### At coarser resolutions

```tfchart
type: line
data: /data/tutorials/land-change/ordered-allocation/fom.csv
x: block
y: [CLUE-S, Ordered, "Ordered, deterministic"]
colors: { CLUE-S: "#3b8a5a", Ordered: "#b4508a", "Ordered, deterministic": "#e3b3cf" }
xlabel: Block side (cells of about 100 m)
ylabel: Figure of merit
xlog: true
height: 300
caption: "The figure of merit at blocks of 1 to 16 cells (validate.three_map). The ordered allocation's change lands in the right neighbourhoods more often than CLUE-S's."
```

Compared in blocks, as in [Validating a simulation](/tutorials/land-change/validating-a-simulation/), the ordered
allocation pulls ahead: 0.25 against 0.17 at blocks of 8 cells (800 m), 0.36 against 0.21 at 16. The ordered
allocation's change is the net change, concentrated where the demand needs it; CLUE-S's reshuffle spreads false
alarms over the whole map, which no coarsening removes. For a regional question (how much forest will be lost in each
town), the ordered allocation is the more faithful of the two here.

### Year by year

```tfchart
type: line
data: /data/tutorials/land-change/ordered-allocation/yearly.csv
x: year
y: [CLUE-S, Ordered, "Ordered, deterministic"]
colors: { CLUE-S: "#3b8a5a", Ordered: "#b4508a", "Ordered, deterministic": "#e3b3cf" }
xlabel: Year
ylabel: Cells that changed class in the year
height: 280
caption: "The cells that changed each year from 1987. Left out: 1986, when CLUE-S changed 13,023 cells (the reshuffle of part 4) and the ordered allocation 610. After it, both change a few hundred cells a year, CLUE-S a little less."
```

### Stochastic and deterministic, and the seed

The deterministic run differs from the stochastic one in a handful of cells: 7,512 changed against 7,506, a figure of
merit of 0.062 in both. On Plum Island the cells at the top of each ranking have high suitabilities and pass their
draw almost always, so the stochastic allocation takes nearly the same cells. The seed matters more than this
suggests: five seeds give maps of 1999 that differ from the first in 839 to 878 cells, and 1,592 cells are built in
some of the runs and not in others.

![Where the seeds disagree](/img/tutorials/land-change/ordered-allocation/seeds.png "In how many of five runs (seeds 1 to 5) each cell not built in 1985 is built in 1999. Dark cells are built in every run, pale ones in some; the built land of 1985 is left blank.")

Use the stochastic allocation, with several seeds, when you want the uncertainty of the location; the deterministic
one when you want a single, repeatable map that depends on nothing but the suitabilities.

### What to report

The order and why; stochastic or deterministic and the seeds; whether every year met the demand (`steps.csv` and
the warnings); the regressions and the demand as for CLUE-S; and the validation at several resolutions.

## Settings that change the answer

### The order

```tfslider
label: Order of the classes
caption: "The change of 1985-1999 with each order of the three classes (real runs, stochastic, seed 1). Only built → forest → other meets the demand every year."
frames:
  - { value: "Forest → Built → Other", src: /img/tutorials/land-change/ordered-allocation/order_1_2_3.png, stats: { "Cells changed by 1999": "8,953", "Forest to built": "725", "Other to built": "5,608", "Other to forest": "0", "Short of the demand in 1999": "291 cells", "Figure of merit": "0.056" } }
  - { value: "Forest → Other → Built", src: /img/tutorials/land-change/ordered-allocation/order_1_3_2.png, stats: { "Cells changed by 1999": "2,750", "Forest to built": "1,458", "Other to built": "1,292", "Other to forest": "0", "Short of the demand in 1999": "3,583 cells", "Figure of merit": "0.036" } }
  - { value: "Built → Forest → Other", src: /img/tutorials/land-change/ordered-allocation/order_2_1_3.png, stats: { "Cells changed by 1999": "7,506", "Forest to built": "4,809", "Other to built": "1,524", "Other to forest": "1,173", "Short of the demand in 1999": "none", "Figure of merit": "0.062" } }
  - { value: "Built → Other → Forest", src: /img/tutorials/land-change/ordered-allocation/order_2_3_1.png, stats: { "Cells changed by 1999": "7,456", "Forest to built": "4,812", "Other to built": "1,521", "Other to forest": "1,123", "Short of the demand in 1999": "53 cells", "Figure of merit": "0.062" } }
  - { value: "Other → Forest → Built", src: /img/tutorials/land-change/ordered-allocation/order_3_1_2.png, stats: { "Cells changed by 1999": "2,720", "Forest to built": "1,432", "Other to built": "1,288", "Other to forest": "0", "Short of the demand in 1999": "3,613 cells", "Figure of merit": "0.036" } }
  - { value: "Other → Built → Forest", src: /img/tutorials/land-change/ordered-allocation/order_3_2_1.png, stats: { "Cells changed by 1999": "8,251", "Forest to built": "5,688", "Other to built": "645", "Other to forest": "1,918", "Short of the demand in 1999": "134 cells", "Figure of merit": "0.060" } }
start: 2
```

The order decides two things.

**Who gets the best land.** A class served first ranks every unlocked cell, and takes the most suitable. Served first,
built land takes forest and other land alike, by its suitability (4,809 cells of forest, 1,524 of other land). Served
after forest, it can take only the forest cells forest has just released and the cells of the classes not yet
served, so it takes other land (5,608 cells) and hardly any forest (725). The same demand, the same regressions,
and two very different stories of where Plum Island's suburbs went.

**Whether the demand is met.** On Plum Island built land grows every year, forest and other land shrink. A shrinking
class served before a growing one releases cells; but a class served *late* that needs to grow can take only the
cells released before it, and the stochastic draw takes a released cell only if its suitability passes its random
number, so it rarely takes them all. With other land or forest served before built land, built land gets a small
pool of released cells and falls short by thousands of cells (3,583 in 1999 with forest → other → built). Serving
built land, the class that grows, first avoids it; serving forest before other land lets forest, which built land
has taken too much of, win cells back from other land, which is not yet locked.

A rule of thumb: **serve the growing classes first**, in the order of the priority you can defend (the class that
outbids the others for land), and the shrinking ones after. Then check `converged` in `steps.csv`. The
deterministic allocation does not fail this way (every released cell passes), but its order still decides who gets
the best land.

### The rules

The rules of [part 5](/tutorials/land-change/clue-s-rules/) apply unchanged: a matrix that forbids a conversion
removes cells from a class's ranking, a restriction locks cells before any class is served. With an ordered
allocation, rules are the only way to stop a conversion, since there is no elasticity.

## Common pitfalls

- **An order that misses a class**, or lists one twice: the step stops ("'order' must contain the model's classes,
  each once").
- **Ignoring the warnings.** "The demand was not met by N cells" means that year's map has the wrong areas, and the
  error grows with the years (the slider above). Change the order, or use `--deterministic`.
- **Setting elasticities.** They are ignored by the ordered allocation (as in lulcc); `terraflux new ordered` does not
  write them.
- **Expecting gross change.** The ordered allocation makes the demand's net change. If swaps between classes matter
  (rotation, abandonment and regrowth), they must come from the demand (a demand per region, [regional
  demand](/tutorials/land-change/regional-demand/)) or from another model.
- **Reading a better agreement as a better model.** 87.7% against 79.0% comes from changing less; the figure of merit
  at one cell is the same. Compare models on the change, at several resolutions.

## Going further

- [Part 4: a complete CLUE-S model](/tutorials/land-change/clue-s-model/) and [part 5: the rules](/tutorials/land-change/clue-s-rules/).
- [A demand per region](/tutorials/land-change/regional-demand/): the allocation run region by region, as Fuchs et
  al. (2013) did with statistics.
- [Validating a simulation](/tutorials/land-change/validating-a-simulation/) and [calibration and many
  runs](/tutorials/land-change/calibration-and-many-runs/): seeds, replicates and the figure of merit.
- [Choosing a land change model](/tutorials/land-change/choosing-a-model/).
- The operators: [[lulcc.step]], [[validate.agreement]], [[validate.three_map]].

## References

- Fuchs, R., Herold, M., Verburg, P. H., & Clevers, J. G. P. W. (2013). A high-resolution and harmonized model approach for reconstructing and analysing historic land changes in Europe. *Biogeosciences*, 10(3), 1543–1559. https://doi.org/10.5194/bg-10-1543-2013
- Fuchs, R., Herold, M., Verburg, P. H., Clevers, J. G. P. W., & Eberle, J. (2015). Gross changes in reconstructions of historic land cover/use for Europe between 1900 and 2010. *Global Change Biology*, 21(1), 299–313. https://doi.org/10.1111/gcb.12714
- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Pontius, R. G., Jr., Boersma, W., Castella, J.-C., Clarke, K., de Nijs, T., Dietzel, C., et al. (2008). Comparing the input, output, and validation maps for several models of land change. *The Annals of Regional Science*, 42(1), 11–37. https://doi.org/10.1007/s00168-007-0138-2
