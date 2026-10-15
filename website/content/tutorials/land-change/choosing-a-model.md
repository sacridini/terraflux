---
title: Choosing a land change model
short: Choosing a model
description: Which of Terraflux's land change models answers your question, by what each assumes about how much changes, where and in what shape, with the data each needs and what their validations on the same landscapes showed.
order: 650
level: beginner
time: 25 min
ops: [dinamica.step, lulcc.step, clumondo.step, cars.step, futures.simulation, molusce.step, crafty.allocate, demand.regional]
series: Other land change models
part: 6
---

This category has built eight kinds of land change model, each on real data. This last part of the series puts
them side by side, so that you can choose one for a question of your own: what each assumes, what it needs, what it
gives, and how their validations compared on the same two landscapes. Use it as a map back to the tutorials; it
adds no new operator.

## The idea

### Three questions, many answers

Every model answers three questions (Verburg et al., 2004): **how much** changes (the quantity), **where** (the
potential of each cell), and **in what shape** (the allocation), under **rules**. The families differ above all in
where the quantity comes from and in how the allocation places it:

| Model | How much | Where | In what shape | Time step |
|---|---|---|---|---|
| [Dinamica EGO](/tutorials/land-change/dinamica-on-sibuyan/) | Markov rates per transition, from two maps | weights of evidence per transition | patches: expansion of old ones and new ones, with sizes and isometry | any (usually a year) |
| [CLUE-S](/tutorials/land-change/clue-s-model/) (lulcc) | a demand of areas per class and year, from outside | logistic regression per class | cell by cell, classes competing with elasticities | a year |
| Ordered allocation (lulcc) | a demand of areas | logistic regression per class | classes served in a fixed order | a year |
| [CLUMondo](/tutorials/land-change/clumondo/) | a demand of goods and services | regression per land system | land systems competing, intensification allowed | a year |
| [PLUS](/tutorials/land-change/plus-cars/) (LEAS and CARS) | Markov or areas | random forest of where each class expanded | random patch seeds, a decaying threshold | a year |
| [FUTURES](/tutorials/land-change/futures/) | developed cells per subregion (population) | logistic per subregion with development pressure | patches grown from a library of observed sizes | a year |
| [MOLUSCE](/tutorials/land-change/molusce/) | the observed cross table, as a ceiling | weights of evidence (or another potential) | cell by cell, most confident cells | the whole period |
| [Agents](/tutorials/land-change/agents/) and [CRAFTY](/tutorials/land-change/crafty/) | decisions of agents; demand for services | agents' rules; capitals and production | parcels; competition for cells | a step of the agents |

Two additions apply to any of them: a **demand per region** ([[demand.regional]],
[A demand per region](/tutorials/land-change/regional-demand/)) and **dynamic drivers** recomputed from the
simulated map at every step ([Dynamic drivers](/tutorials/land-change/dynamic-drivers/)).

### What the choice changes

The families differ in assumptions that decide the answer:

- **Where the quantity comes from.** Markov rates continue the past; a demand of areas or goods is a scenario.
  If the question is "what if the trend goes on", rates are natural; if it is "what if the policy sets the areas",
  a demand.
- **Transitions or classes.** Dinamica EGO and MOLUSCE model transitions (forest to pasture has its own drivers and
  shapes); CLUE-S, CLUMondo and PLUS model classes competing for cells. Transitions suit landscapes where *how* a
  class arises matters (pasture from forest or from cropland); classes suit scenarios of many classes at once.
- **The shape.** Cell-by-cell allocations scatter change over the many cells of similar potential; patch-based ones
  (Dinamica EGO, PLUS, FUTURES) produce patches whose sizes come from data or settings. If the maps go on to
  landscape metrics, connectivity or habitat, choose a patch-based allocation, or check the simulated patches.
- **Who decides.** Statistical models describe the outcome of decisions; agent models describe the decisions, at the
  price of rules that are hard to observe.

## When to use it

Choose by the question first, then by the data:

| Your question | Start with | Because |
|---|---|---|
| Where will deforestation go if the trend continues, and in what patches? | Dinamica EGO | rates from the past, per-transition drivers, patch shapes |
| Where do the areas of a scenario (a policy target, an economic projection) go? | CLUE-S | the demand is the scenario; classes compete |
| Is a demand for food met by clearing more land or by intensifying? | CLUMondo | land systems supply goods; intensification is a choice |
| How do several classes expand at once, with new patches appearing? | PLUS | expansion analysis and random patch seeds |
| How does a town grow, infill against sprawl, county by county? | FUTURES | demand per county, development pressure, observed patch sizes |
| What would a MOLUSCE study give, or a quick first look? | MOLUSCE | the plugin's logic, reproduced |
| Do rates differ between municipalities, basins or biomes? | any, with a demand per region | each region keeps its own quantity |
| How do farmers' or managers' decisions change the land? | agents, CRAFTY | the decision makers are in the model |
| Is a driver altered by the change itself (distance to the newest clearing)? | any, with dynamic drivers | the driver follows the simulated map |
| Is the map larger than the computer's memory? | Dinamica EGO with the tiled engine | [Maps larger than memory](/tutorials/land-change/maps-larger-than-memory/) |

And by the data you have:

- **Two maps and drivers**: Dinamica EGO, MOLUSCE, PLUS (all fit their quantity and potential on the two maps).
- **One map, drivers and areas from elsewhere**: CLUE-S, the ordered allocation (the demand is given).
- **Maps of land systems and the goods each supplies**: CLUMondo.
- **Developed land at two dates, subregions and population**: FUTURES.
- **Parcels, surveys of decisions**: agents, CRAFTY.
- **Three dates or more**: any model can then be validated on a period it did not see; do it
  ([Validating a simulation](/tutorials/land-change/validating-a-simulation/)).

## What you need

Each tutorial lists its data. In every case: the maps on one grid (extent, cell size, coordinate system), whole
class codes with a no-data value, drivers as a stack of named bands, and the time step in mind before fitting
anything. The commands `terraflux new dinamica`, `terraflux new clue` and `terraflux new ordered` write the classic
models from your files; the application's Wizard does the same in three steps, and its templates (Dinamica EGO,
CLUE-S, a cellular automaton, farmers as agents, CRAFTY) run on maps drawn by chance.

## Read the results

### How they scored on the same landscapes

The tutorials validated their models with the same measures, on two landscapes. The numbers are not a ranking
(different periods, drivers and settings), but they show what to expect:

| Landscape and period | Model | Figure of merit, cell | At coarser blocks |
|---|---|---:|---|
| Sibuyan, 1997-2011 (the calibration period) | Dinamica EGO, 13 drivers | 0.47 | 0.58 at 4 km |
| | Dinamica EGO, 2 drivers | 0.17 | 0.31 at 4 km |
| | MOLUSCE, 3 drivers, one iteration | 0.14 | 0.29 at 2 km |
| Plum Island, 1991-1999 (a period not fitted on) | Dinamica EGO, fitted on 1985-1991 | 0.032 | 0.26 at 1.6 km |
| | PLUS (CARS) | 0.03 | 0.37 at 1.6 km |
| | FUTURES (built land only) | 0.034 | |
| Plum Island, 1985-1999 | CLUE-S, lulcc's vignette | 0.063 | 0.21 at 1.6 km |

Three lessons stand out. The **landscape** matters more than the model: Sibuyan's change comes in large patches
that follow the drivers; Plum Island's suburban growth is scattered in small patches that no model with three
drivers places cell by cell, whatever its family. The **drivers** matter more than the allocation (13 against 2
drivers on Sibuyan). And every model gains at coarser resolutions: decide at which resolution the maps will be used,
and judge them there. Pontius et al. (2008) found the same across thirteen published applications of nine models.

### What to report, whatever the model

The three pieces and their sources; the rules; the settings and how they were chosen (calibration); replicates; the
validation against an observed map, against persistence, at several resolutions; and the model file itself.

## Settings that change the answer

Across families, the settings that most often change the conclusions are the same few: the **quantity** (rates or
demand: a scenario, so explore it), the **drivers** and their treatment (ranges, formulas), the **resistance to
change** (CLUE-S's elasticities, Dinamica's rates per transition), and the **shape** of the allocation (patch
sizes, isometry, seeds). Explore them with batches of runs ([Calibration and many
runs](/tutorials/land-change/calibration-and-many-runs/)).

## Common pitfalls

- **Choosing by familiarity.** A model chosen because it is common in the literature of a region may not fit the
  question; start from the question.
- **Comparing models on one run each**, or on the calibration period only.
- **Reporting the share of cells that agree.** It mostly measures persistence.
- **A scenario's demand without a source.** The demand is the scenario; justify it.
- **Ignoring the shape** when the maps feed landscape metrics or connectivity.

## Going further

- The foundations: [What land change models are](/tutorials/land-change/what-land-change-models-are/).
- [Validating a simulation](/tutorials/land-change/validating-a-simulation/) and
  [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/).
- The software packages compared by Mas et al. (2014).

## References

- Mas, J.-F., Kolb, M., Paegelow, M., Camacho Olmedo, M. T., & Houet, T. (2014). Inductive pattern-based land use/cover change models: a comparison of four software packages. *Environmental Modelling & Software*, 51, 94–111. https://doi.org/10.1016/j.envsoft.2013.09.010
- Pontius, R. G., Jr., Boersma, W., Castella, J.-C., Clarke, K., de Nijs, T., Dietzel, C., et al. (2008). Comparing the input, output, and validation maps for several models of land change. *The Annals of Regional Science*, 42(1), 11–37. https://doi.org/10.1007/s00168-007-0138-2
- Verburg, P. H., Schot, P. P., Dijst, M. J., & Veldkamp, A. (2004). Land use change modelling: current practice and research priorities. *GeoJournal*, 61(4), 309–324. https://doi.org/10.1007/s10708-004-4946-y
