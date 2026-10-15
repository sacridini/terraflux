---
title: "Rules: elasticities, conversions, neighbourhoods and restrictions"
short: "CLUE-S: the rules"
description: Tell CLUE-S which conversions are allowed and when, where land may not change, and which neighbours a class needs, with lulcc's conversion matrix, history, neighbourhood rules and restricted areas, and see on Plum Island what each rule does to the map.
order: 540
level: intermediate
time: 50 min
template: CLUE-S
ops: [lulcc.step, lulcc.history, glm.fit, glm.predict, demand.from_maps, rng.r, control.repeat]
series: "Demand-driven allocation: CLUE-S and lulcc"
part: 5
---

The model of [part 4](/tutorials/land-change/clue-s-model/) met the demand every year, but in its first year it
turned 13,023 cells (11% of Plum Island) into another class for a net change of a few hundred, and some of them were
towns turned into forest. Its only rule was the elasticity. In this part you will add the other **decision rules**
of CLUE-S, as lulcc implements them: a **restricted area** where nothing may change, a **conversion matrix** that
says which class may become which, with **time limits** that read how long a cell has had its class (its
**history**), and **neighbourhood rules** that let a class go only where it already has neighbours. For each rule you
will see what it does to Plum Island's simulation from 1985 to 1999, with the same regressions, demand and settings
as part 4. At the end you will know how to write each rule, in the model file and with `terraflux new clue`, and
how to tell when a rule is doing what you meant.

## The idea

### Why rules

The suitability of [part 2](/tutorials/land-change/logistic-suitability/) says where each class is *likely*, and the
demand of [part 3](/tutorials/land-change/land-demand/) how much of it there must be. Neither says what *cannot*
happen: a town does not become forest in a year, a nature reserve is not built on, a field abandoned last year does
not become old forest, a new suburb grows next to an old one. Verburg et al. (2002) made these **conversion
settings** and **spatial policies** a part of CLUE-S of their own, next to the suitability and the demand, because
they come from knowledge of the land (laws, plans, the ecology of succession) rather than from a regression.

lulcc implements five of them (Moulds et al., 2015), and Terraflux's [[lulcc.step]] applies them in the same order
at every step:

!!! science "Key idea: how a step applies the rules"
    For every cell \(i\) and class \(u\), start from the total probability of part 1,
    \(TPROP_{i,u} = P_{i,u} + ELAS_u\,[\,u \text{ is the cell's class}\,]\), then:

    1. **neighbourhood rules**: \(TPROP_{i,u}\) is removed (the class cannot go there) where the share of class
       \(u\) in the window around \(i\), on the map at the start of the step, is below the class's threshold;
    2. **conversion matrix**: \(TPROP_{i,u}\) is removed where the matrix forbids the conversion from the cell's
       class to \(u\), given the cell's history and the direction of the demand;
    3. **automatic conversion**: a cell whose *own* class has just been removed must leave it, and takes the
       allowed class of highest \(TPROP\) at once;
    4. **restrictions**: a restricted cell keeps its class;
    5. the cells left are **allocated** as in part 1, until the demand is met.

    After the step, the history of a cell that kept its class grows by 1, and that of a cell that changed is set to 0.

The elasticity (part 4) adds to the total probability; every other rule removes options. A rule never forces a
change on its own, except through step 3: a rule that forbids a cell its own class sends it away.

### The conversion matrix and its codes

The **conversion matrix** (`allowed`) has a row per class *from* and a column per class *to*, in the order of the
demand's classes (1 forest, 2 built, 3 other on Plum Island). Each entry is a code:

| Code | The conversion from the row's class to the column's is allowed... |
|---|---|
| `1` | always |
| `0` | never |
| `-1` | only while the demand of the row's class *decreases* (the class is giving up land) |
| `-2` | only while the demand of the column's class *increases* (the class needs land) |
| `100 + t` | only while the cell has had its class for *less* than \(t\) steps |
| `1000 + t` | only once the cell has had its class for \(t\) steps *or more* |

The time codes read the **history**: how many steps each cell has had its class. `1000 + t` is a minimum time
before a conversion (a young plantation cannot be harvested; land must lie fallow 8 years before it is forest),
`100 + t` a maximum (shifting cultivation: a field is left after 5 years). On the diagonal, the same codes say how
long a class may *stay*: `105` on forest's diagonal would force every forest cell out after 5 years.

![The time codes](/img/tutorials/land-change/clue-s-rules/time_codes.png "When a conversion with each code is allowed (green), by the cell's history: 1 always; 105 while the cell has had its class for less than 5 steps; 1008 once it has had it for 8 steps or more.")

The reference case of Plum Island with every rule, which Terraflux's tests run against landschaft's port of lulcc,
uses this matrix:

|  | to forest | to built | to other |
|---|---|---|---|
| **forest** | 1 | 1008 | 105 |
| **built** | 0 | 1 | 0 |
| **other** | −1 | −2 | 1 |

Read row by row: forest may become built land only after 8 years as forest, and other land only while it has been
forest less than 5 years (young forest is cleared, old forest is not); built land never changes; other land may
become forest while the demand for other land falls, and built land while the demand for built land grows.

### Neighbourhoods and restrictions

A **neighbourhood rule** (`neighbourhood`) gives a class a threshold: the class may go to a cell only where the mean
of its presence in a window around the cell (3 × 3 by default, the cell included, missing cells left out) reaches
the threshold. With a 3 × 3 window, a threshold of 0.3 for built land means at least 3 of the 9 cells built: a new
built cell needs three built neighbours. This is the simplest form of the neighbourhood effects Verburg et al.
(2004) measured in land use patterns, and which [dynamic drivers](/tutorials/land-change/dynamic-drivers/) model
another way, as a variable of the regressions.

**Restrictions** are a map: every cell where it is not 0 keeps its class. A nature reserve, a zone where building is
forbidden, an area the scenario protects. The demand is for the whole map, so what cannot happen in the restricted
area must happen elsewhere.

## When to use it

- **Policies and scenarios.** A reserve, a green belt, a moratorium on clearing: restrictions, or a matrix that
  forbids the conversion. Scenario studies with CLUE-S compare the same demand with and without such a policy.
- **Irreversible or slow conversions.** Built land that does not revert, forest that needs decades to mature: a row
  of zeros, or a time code.
- **Succession and rotation.** Fallow that becomes forest after some years, crops that move after a few: the time
  codes with the history, and Dyna-CLUE's autonomous conversions beyond them (Verburg & Overmars, 2009).
- **Compact growth.** New towns next to old ones: a neighbourhood rule.
- **Agents' decisions.** A restriction map can come from agents: a council that freezes its land (see
  [a coupled model](/tutorials/land-change/coupled-agents/#agents-in-a-clue-s-model)).

Rules are hypotheses, like the regressions. Write them from what is known of the land (laws, plans, the observed
transitions of [change detection](/tutorials/change-detection/transitions/)), not to improve a score: as the
results below show, they change *how much* of the landscape moves far more than whether the change lands in the
right places.

## What you need

- The model of [part 4](/tutorials/land-change/clue-s-model/) and its data, `pie_landuse.tif` and `pie_drivers.tif`
  ([part 1](/tutorials/land-change/clue-s-ideas/#what-you-need)).
- For restrictions, a map on the same grid: 0 free, anything else restricted. Here the block of that reference case,
  [`pie_restrictions.tif`](/data/tutorials/land-change/clue-s-rules/pie_restrictions.tif): 100 × 150 cells (10 by 15
  km) in the north of the watershed, holding 12,839 cells with a class (6,727 forest, 3,528 built, 2,584 other). It
  is not a real reserve; it stands for one.
- For time codes, a map of the history at the start, if known: the steps each cell has had its class. Without one,
  every cell starts at 1. Here the same case's map, [`pie_history.tif`](/data/tutorials/land-change/clue-s-rules/pie_history.tif),
  which draws a history from 0 to 12 years at random for each cell: a stand-in for a real one, which would come
  from a series of older maps.

!!! app "In the application"
    The template **CLUE-S** (**Templates → Land change**) has the same nodes on maps drawn by chance. The rules are
    settings of its `allocate` node (inside the `simulation` loop): select it and edit `allowed`, `neighbourhood`
    and `elasticity` in the Node pane. A restriction map is connected to its `restrictions` port, and a history map
    to the `history` port of the `history` node.

## Build the model

### With one command

Each rule is an option of `terraflux new clue`:

```bash
terraflux new clue --observed pie_landuse.tif --times 1985,1991,1999 --drivers pie_drivers.tif \
  --formulas '{"1": ["elevation", "slope"], "2": ["elevation", "slope", "dist_built_1985"], "3": ["elevation", "slope"]}' \
  --elasticity 0.2,0.2,0.2 \
  --settings '{"jitter": 0.0002, "scale": 0.000001, "max_iter": 1000, "max_diff": 50, "mean_diff": 50}' \
  --allowed '[[1,1008,105],[0,1,0],[-1,-2,1]]' \
  --history pie_history.tif \
  --restrictions pie_restrictions.tif \
  --neighbourhood '{"rules": {"2": 0.3}}' \
  -o clue_rules.toml
terraflux run clue_rules.toml --out clue_rules
```

The allocation's settings are those of lulcc's vignette, as in part 4 (with lulcc's default `scale` of 0.0005 the
allocation never converges on a map of this size). Leave out the options you do not need: each rule works alone.
The [model written](/data/tutorials/land-change/clue-s-rules/clue_rules.toml) by this command is the one below.

### The model

```tfgraph
#! caption: The CLUE-S model of part 4 with every rule. Two inputs are new, the restrictions and the history at the start; the rules themselves are settings of the step (node 7).
#! params: false
#! numbered: true
#! highlight: history, allocate
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

[inputs.restrictions]
kind = "map"
path = "pie_restrictions.tif"
note = "restricted cells keep their class"

[inputs.history]
kind = "map"
path = "pie_history.tif"
note = "the steps each cell has had its class"

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
inputs = { landscape = "$start", history = "$history" }
note = "How long each cell has had its class (1 where not given)."

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
inputs = { landscape = "@landscape", history = "@history", suitability = "suitability.suitability", demand = "demand.demand", step = "@step", rng = "rng.rng", restrictions = "$restrictions" }
params = { allocation = { method = "clue", jitter = 0.0002, scale = 0.000001, max_iter = 1000, max_diff = 50, mean_diff = 50 }, elasticity = [0.2, 0.2, 0.2], allowed = [[1, 1008, 105], [0, 1, 0], [-1, -2, 1]], neighbourhood = { rules = { 2 = 0.3 } } }

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
```

What changed from part 4, node by node:

- **The input `restrictions`** (a map) goes straight to the `restrictions` port of **`allocate`** (node 7). It is
  read from outside the loop: the restricted area is the same every year. To make it change (a reserve created in
  1990), compute it inside the loop from `@step`, as the councils of [a coupled model](/tutorials/land-change/coupled-agents/#agents-in-a-clue-s-model) do.
- **The input `history`** goes to the optional `history` port of **`history`** ([[lulcc.history]], node 5), which
  gives every cell with a class its starting history (1 where the map has none). Its output starts the loop's
  carried value `history`, and the loop's `carry` passes on, every year, the history that `allocate` updated.
  Without that carry, the time codes would read the history of 1985 every year and a `1008` would never open.
- **`allocate`'s settings** hold the rules: `allowed`, the matrix, a list of rows in the demand's class order;
  `neighbourhood = { rules = { 2 = 0.3 } }`, a threshold per class, with an optional `weights`, the window (an odd
  matrix of weights, one for all classes or one per class: `weights = { 2 = [[1, 1, 1, 1, 1], ...] }`); and
  `elasticity`, as before.

## Read the results

### A restricted block

```tfcompare
caption: "The cells that changed class between 1985 and 1999 (by their new class: forest green, built purple, other ochre; grey unchanged), without restrictions (left of the handle) and with the restricted block outlined (right). Every other setting is the vignette's of part 4."
before: { src: /img/tutorials/land-change/clue-s-rules/change_free.png, label: No restrictions }
after: { src: /img/tutorials/land-change/clue-s-rules/change_restricted.png, label: Restricted block }
```

![Legend](/img/tutorials/land-change/clue-s-rules/change_legend.png)

Without restrictions, 2,638 cells of the block changed class by 1999, 941 of them to built land. With the block
restricted, none did. The demand is the same, so the built land went elsewhere: outside the block, 8,547 cells
became built instead of 8,020. The total change, though, *fell*, from 19,522 to 16,990 cells: most of the change the
block lost was the reshuffle of the first year (part 4), forest and other land trading places where the regressions
disagree with the map of 1985, which the demand did not need. A restriction removes that noise along with the change
you meant to prevent.

![The restricted block](/img/tutorials/land-change/clue-s-rules/restrictions.png "The reference case's block of restrictions over Plum Island's map of 1985.")

### Conversion matrices

| Run | Changed in 1986 | Changed by 1999 | Built land lost | Years converged | Same class as 1999 | Figure of merit |
|---|---:|---:|---:|---|---:|---:|
| Elasticities only (part 4) | 13,023 | 19,522 | 2,656 | 14 of 14 | 79.0% | 0.063 |
| Restricted block | 10,555 | 16,990 | 2,240 | 14 of 14 | 80.7% | 0.061 |
| Built land cannot change | 8,402 | 14,850 | 0 | 14 of 14 | 82.6% | 0.061 |
| lulcc's matrix, history 1 | 8,404 | 15,591 | 0 | 12 of 14 | 82.0% | 0.059 |
| lulcc's matrix, history map | 5,891 | 12,419 | 0 | 14 of 14 | 84.3% | 0.060 |
| Every rule (the model above) | 4,734 | 11,217 | 0 | 14 of 14 | 85.1% | 0.061 |

*Real runs of the model of part 4 with each rule added (`terraflux new clue` with the options of the command
above). The observed change from 1985 to 1999 was 8,578 cells; the agreement and the figure of merit are those of
[part 4](/tutorials/land-change/clue-s-model/#against-the-observed-map-of-1999), against the observed map of 1999.*

**Built land cannot change** (`--allowed '[[1,1,1],[0,1,0],[1,1,1]]'`): the row of built land is `0, 1, 0`. The
observed maps agree: of the 37,122 built cells of 1985, 165 had another class in 1999 (part 1). The 2,656 built cells
the vignette turned into forest and other land are gone, and the first year's reshuffle falls by a third, to 8,402
cells. The agreement with the map of 1999 rises from 79.0% to 82.6%. This is the rule most CLUE-S applications start
with.

**lulcc's matrix** adds the time and demand codes. With the history at 1 everywhere, the `1008` of forest to built
holds every forest cell for 7 years: no forest becomes built land until 1993, then about 390 cells a year at once,
and two years (1990 and 1992) do not converge, because the built land demanded must then all come from other land.
With the history map, cells start with 0 to 12 years and some may become built land from the first year:

```tfchart
type: line
data: /data/tutorials/land-change/clue-s-rules/forest_to_built.csv
x: year
y: [Vignette, "lulcc's matrix, history 1", "lulcc's matrix, history map"]
colors: { Vignette: "#7f7f7f", "lulcc's matrix, history 1": "#b4508a", "lulcc's matrix, history map": "#3b8a5a" }
xlabel: Year
ylabel: Forest cells that became built land
height: 300
caption: "Forest that became built land each year, with the elasticities only (grey), with lulcc's matrix and a history of 1 everywhere (purple: nothing until 1993), and with the matrix and the history map (green). The first year of the vignette, 1,468 cells, is the reshuffle."
```

A time code makes the history at the start part of the model. Without a real one, a history of 1 everywhere says
"every cell changed last year", which is rarely true; at least start the simulation a few years before the period
you analyse, so that the histories have time to spread, or build the history from older maps.

### Neighbourhood rules

A neighbourhood rule on built land, with built land unable to change (so that the rule only decides where *new*
built land goes, see the pitfalls). Move the slider through thresholds for a 3 × 3 window: 0.2 means at least 2
built cells among the 9, 0.5 at least 5.

```tfslider
label: Neighbourhood threshold of built land
caption: "New built land from 1985 to 1999 (dark purple) and the built land of 1985 (pale) in the south-west of the watershed, with each threshold (real runs). Up to 0.5 the demand is met every year and the new built land clusters into fewer, larger patches next to the old; at 0.6 too few cells pass the threshold and most years fail."
frames:
  - { value: "none", src: /img/tutorials/land-change/clue-s-rules/nb_0.0.png, stats: { "New built cells": "6,308", "Patches": "2,968", "Mean patch": "2.13 cells", "Years converged": "14 of 14", "Figure of merit": "0.061" } }
  - { value: "0.2", src: /img/tutorials/land-change/clue-s-rules/nb_0.2.png, stats: { "New built cells": "6,293", "Patches": "2,815", "Mean patch": "2.24 cells", "Years converged": "14 of 14", "Figure of merit": "0.061" } }
  - { value: "0.3", src: /img/tutorials/land-change/clue-s-rules/nb_0.3.png, stats: { "New built cells": "6,293", "Patches": "2,512", "Mean patch": "2.51 cells", "Years converged": "14 of 14", "Figure of merit": "0.062" } }
  - { value: "0.4", src: /img/tutorials/land-change/clue-s-rules/nb_0.4.png, stats: { "New built cells": "6,291", "Patches": "2,214", "Mean patch": "2.84 cells", "Years converged": "14 of 14", "Figure of merit": "0.061" } }
  - { value: "0.5", src: /img/tutorials/land-change/clue-s-rules/nb_0.5.png, stats: { "New built cells": "6,284", "Patches": "2,168", "Mean patch": "2.90 cells", "Years converged": "14 of 14", "Figure of merit": "0.060" } }
  - { value: "0.6", src: /img/tutorials/land-change/clue-s-rules/nb_0.6.png, stats: { "New built cells": "3,821", "Patches": "1,741", "Mean patch": "2.19 cells", "Years converged": "6 of 14", "Figure of merit": "0.046" } }
start: 2
chart:
  type: bar
  data: /data/tutorials/land-change/clue-s-rules/neighbourhood.csv
  x: threshold
  y: [patches]
  labels: { patches: "Patches of new built land" }
  colors: { patches: "#b4508a" }
  xlabel: Threshold
  ylabel: Patches
```

![Legend](/img/tutorials/land-change/clue-s-rules/nb_legend.png)

```tfcompare
caption: "The same close-up: the new built land observed from 1985 to 1999 (left of the handle) and simulated with a threshold of 0.5 (right). The observed growth came in 1,585 patches of 4.1 cells on average, larger and fewer than any simulation's."
before: { src: /img/tutorials/land-change/clue-s-rules/nb_observed.png, label: Observed }
after: { src: /img/tutorials/land-change/clue-s-rules/nb_0.5.png, label: Threshold 0.5 }
```

The rule does what it says: the higher the threshold, the more the new built land clusters next to the old, from
2,968 patches to 2,168. Up to 0.5 the demand is met every year, since Plum Island's built land is everywhere and few
cells lack built neighbours. At 0.6 (6 built cells of 9) the cells that may become built are too few for the demand,
and 8 years of 14 fail to converge: a threshold has a ceiling set by the map. The figure of merit hardly moves: the
rule changes the *shape* of the growth, not whether the regressions put it in the right neighbourhoods. To judge a
rule of shape, compare shapes: patch numbers and sizes against the observed change, with
[landscape metrics](/tutorials/#landscape-metrics).

### What to report

Every rule, in words and as the model writes it: the matrix with its codes, the thresholds and windows, the
restricted areas (their source and area), the history at the start and where it came from. And for each run,
whether every year converged (`steps.csv`): rules make convergence harder.

## Settings that change the answer

- **The elasticities.** Part 4's slider: still the strongest setting of the reshuffle. Rules and elasticities
  interact: a row of zeros in the matrix does for a class what an elasticity of 1 would try to do.
- **The thresholds and windows** of the neighbourhood rules: the slider above. A larger window (`weights`, odd sides)
  with the same threshold asks for neighbours further away.
- **The history at the start**, with time codes: the chart above. A history of 1 everywhere delays every `1000 + t`
  by \(t - 1\) years.
- **The codes `-1` and `-2`**, which follow the demand's direction: change the demand (part 3) and they open or
  close.
- **The allocation's `max_iter` and `scale`.** Rules leave fewer cells to move, and the iteration variables must
  climb further: when years stop converging after you add a rule, raise `max_iter` before blaming the rule.

## Common pitfalls

- **A rule that forbids a class its own cells.** A neighbourhood rule applies to the cells that already have the
  class too: with a threshold of 0.5 on built land and no matrix, a built cell with fewer than 5 built cells in its
  window is converted automatically. In a real run, 10,134 built cells became forest or other land by 1999 (2,656
  without the rule), and the first year changed 20,276 cells. Protect existing land with the matrix (`0, 1, 0` for built
  land), as the slider above does, or keep thresholds low. The same holds for a time code on the diagonal.
- **Restrictions do not stop automatic conversions.** A restricted cell keeps its class unless a rule forbids it
  that class: then it is converted like any other (with a forest threshold of 0.9 and the block restricted, every
  forest cell of the block changed). Check the restricted area after adding rules.
- **The matrix in the wrong order.** Rows and columns follow the demand's classes in increasing order of code (here
  1, 2, 3), not the order of a legend. A square matrix of the wrong size stops the step.
- **History not carried.** The loop must carry `allocate.history` (as `terraflux new clue` writes it); otherwise
  the time codes see the starting history every year.
- **A demand the rules cannot meet.** Forbid enough conversions and the demand has nowhere to go: `converged` = 0
  and a warning. A restricted area plus a matrix plus a threshold can leave a class too few cells.
- **Rules tuned to the score.** The figure of merit barely changed with any rule here; rules that only raise the
  agreement with the observed map (by freezing more of it) do not make a better model of change.

## Going further

- [Part 6: the ordered allocation](/tutorials/land-change/ordered-allocation/), lulcc's other allocation, which uses
  the same rules but no elasticities.
- [Part 4](/tutorials/land-change/clue-s-model/): the model these rules were added to, and its elasticities.
- [Dynamic drivers](/tutorials/land-change/dynamic-drivers/): neighbourhood effects as a variable of the
  regressions, recomputed each year.
- [A coupled model](/tutorials/land-change/coupled-agents/): restrictions decided by agents.
- [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/): choosing elasticities and
  thresholds against observed maps.
- The operators: [[lulcc.step]], [[lulcc.history]], [[control.repeat]].

## References

- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Verburg, P. H., de Nijs, T. C. M., Ritsema van Eck, J., Visser, H., & de Jong, K. (2004). A method to analyse neighbourhood characteristics of land use patterns. *Computers, Environment and Urban Systems*, 28(6), 667–690. https://doi.org/10.1016/j.compenvurbsys.2003.07.001
- Verburg, P. H., & Overmars, K. P. (2009). Combining top-down and bottom-up dynamics in land use modeling: exploring the future of abandoned farmlands in Europe with the Dyna-CLUE model. *Landscape Ecology*, 24(9), 1167–1181. https://doi.org/10.1007/s10980-009-9355-7
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
