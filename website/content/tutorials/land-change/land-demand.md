---
title: "The demand: how much of each class"
short: The demand
description: Make the table of how many cells each land use must cover in each year, from observed maps, from a Markov chain or from a scenario, and see what a demand does and does not decide.
order: 520
level: intermediate
time: 35 min
ops: [demand.from_maps, demand.table, markov.fit, plus.demand, lulcc.step, control.repeat]
series: "Demand-driven allocation: CLUE-S and lulcc"
part: 3
---

A demand-driven model needs, before anything else, a **demand**: how many cells of each land use the map must
hold in each year. In this part you will make demands of three kinds for the Plum Island Ecosystems: the
linear trend of the observed maps ([[demand.from_maps]]), the projection of a Markov chain ([[markov.fit]] and
[[plus.demand]]), and a scenario written by hand ([[demand.table]]). You will connect a demand to the
allocation loop, run CLUE-S with two demands, and see the most important lesson about demands: they decide
*how much*, never *where*. A demand that keeps the forest's area constant does not keep the forest.

## The idea

### A demand is a table

A demand is a table with a row per year and a column per class. Each value is a number of **cells** (not
hectares): the area the class must cover in that year. The first row is the start (the map the simulation
starts from), and the simulation has one step per following row. For Plum Island's trend, the first rows are:

| Year | Forest (1) | Built (2) | Other (3) | Total |
|---|---|---|---|---|
| 1985 | 49,013 | 37,122 | 27,428 | 113,563 |
| 1986 | 48,683 | 37,660 | 27,220 | 113,563 |
| 1987 | 48,352 | 38,198 | 27,013 | 113,563 |
| ... | | | | |
| 1999 | 45,377 | 43,455 | 24,731 | 113,563 |

Every row adds up to the same total: the cells of the map that have a class. The allocation can move cells
between classes, but it cannot create or destroy land.

### Where a demand comes from

In the studies that use CLUE-S, the demand comes from one of four sources (Verburg et al., 2002; Verburg &
Overmars, 2009):

- **The trend of observed maps.** The areas measured on two or more maps, joined by straight lines and extended.
  The natural choice for calibrating and validating a model on a past period.
- **A Markov chain.** The rates at which each class turned into each other class in an observed period, applied
  to the map year after year. The amount of change then depends on how much of each class is left.
- **Another model.** An economic model of agricultural markets, a demographic projection (people times the
  built area per person), a forestry plan. Their hectares are converted to cells.
- **A scenario or a target.** A storyline ("the region protects its forest", "biofuels double"), or a policy
  target ("no net loss of wetlands"), written as a table.

!!! science "Key idea: the linear demand of observed maps"
    With maps at dates \( t_1 < t_2 < \dots < t_m \), the demand of class \(k\) at a date \(t\) is read off the
    straight line through the two observed areas around \(t\):

    \[ D_k(t) = A_k(t_a) + \frac{A_k(t_b) - A_k(t_a)}{t_b - t_a}\,(t - t_a) \]

    - \( A_k(t_a) \), \( A_k(t_b) \) are the cells of class \(k\) in the maps of dates \(t_a\) and \(t_b\): the
      two dates around \(t\) (interpolation), or the first two or last two dates for a \(t\) before or after
      them (extrapolation);
    - negative values are set to 0, and each row is rounded to whole cells that add up to the cells of the
      first map.

    This is lulcc's `approxExtrapDemand` (Moulds et al., 2015), which [[demand.from_maps]] reproduces.

!!! science "Key idea: the demand of a Markov chain"
    A Markov chain moves the vector of cells per class one year at a time:

    \[ \mathbf{n}(t+1) = \mathbf{n}(t)\,\mathbf{P} \]

    - \( \mathbf{n}(t) \) is the row of cells of each class in year \(t\);
    - \( \mathbf{P} \) is the yearly transition matrix: \( P_{jk} \) is the share of the cells of class \(j\)
      that become class \(k\) in a year. From two maps 14 years apart, [[markov.fit]] computes the matrix of the
      period and its 14th root, the yearly matrix (a matrix whose 14th power is the period's).

    The linear demand moves the same **number** of cells every year; a Markov chain moves the same **share** of
    each class, so a shrinking class loses fewer cells each year. Markov chains are explained in the
    [foundations of this category](/tutorials/#land-change).

## When to use it

| Demand | Use it for | Watch out for |
|---|---|---|
| Linear trend ([[demand.from_maps]]) | calibration and validation on an observed period; short projections | far extrapolations: a straight line can run a class to zero |
| Markov chain ([[markov.fit]] + [[plus.demand]]) | projections where change is proportional to what is left (deforestation of a shrinking forest) | the rates are those of one period; the chain tends to an equilibrium that may be meaningless |
| Table ([[demand.table]]) | scenarios, targets, demands from other models | rows must add up to the cells of the map |

For validation, use the observed trend of the period you validate on: then the demand is right by construction,
and the comparison with the observed map measures the allocation alone (where), not the demand (how much). This
is the logic of [part 4](/tutorials/land-change/clue-s-model/).

## What you need

- **For a trend:** two or more maps of the same area, on the same grid, with the same class codes, and the date
  of each. Here, the three bands of `pie_landuse.tif` (1985, 1991, 1999).
- **For a Markov chain:** two maps and the number of time steps between them (14 years here).
- **For a scenario:** the areas of each class in each year, converted to cells. A cell of Plum Island is
  99.92 m × 99.95 m, 0.999 ha, so hectares and cells are nearly the same number; on a grid of 30 m, a hectare
  is 11.1 cells. Download the maps from [part 1](/tutorials/land-change/clue-s-ideas/#what-you-need).

## Build the model

### A demand from observed maps

Add a [[demand.from_maps]] node (library: **Land change simulation → lulcc and CLUE-S**) and connect the
observed maps to `maps`. The maps come as a **stack**, a band per date. Two settings:

- `times`: the date of each band. Without it, the band names are used when they are numbers (here they are:
  1985, 1991, 1999), else 0, 1, 2...
- `out_times`: the dates the demand is wanted for. Without it, every whole date from the first map to the last
  (1985, 1986... 1999: 14 steps). With `out_times`, the demand can go beyond the last map (extrapolation) or use
  steps of several years.

```tfgraph
#! caption: A linear demand from the maps of 1985, 1991 and 1999, extended to 2030. The output steps is the number of steps after the first date.
#! highlight: trend
format = "terraflux/1"

[inputs.observed]
kind = "stack"
path = "pie_landuse.tif"

[[nodes]]
id = "trend"
op = "demand.from_maps"
inputs = { maps = "$observed" }
params = { times = [1985, 1991, 1999], out_times = [1985, 1990, 1995, 2000, 2005, 2010, 2015, 2020, 2025, 2030] }
note = "The cells of each class at each date: straight lines through the observed counts."

[outputs.demand]
from = "trend.demand"
```

The node has two outputs. `demand` is the table. `steps` is the number of rows after the first: it is what
the allocation loop's `times` takes, so that the loop runs exactly once per row of the demand. A run writes a
demand as JSON (`output/demand.json`: its classes, times and table).

![The linear demand of Plum Island](/img/tutorials/land-change/land-demand/trend.png "The linear demand of Plum Island: interpolated between the observed counts of 1985, 1991 and 1999 (dots), and extrapolated from the last two after 1999.")

Between the maps, the lines join the dots; after 1999 they follow the slope of 1991–1999. By 2030 the trend
asks for 55,487 cells of built land (49% of the landscape, up from 33% in 1985) and 38,968 of forest.

### A demand from a Markov chain

A Markov chain needs two maps and the number of steps between them ([[markov.fit]]), and a starting map to
project from ([[plus.demand]], which projects the chain from the cells of that map and rounds each year to whole
cells that add up to the map's).

```tfgraph
#! caption: A Markov chain fitted on 1985-1999 (14 yearly steps) and projected from the map of 1999 for 31 years. plus.demand turns the chain into a demand that lulcc.step accepts.
#! highlight: markov, projection
format = "terraflux/1"

[inputs.t1985]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.t1999]
kind = "map"
path = "pie_landuse.tif"
band = "1999"

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = { t0 = "$t1985", t1 = "$t1999", steps = 14 }
note = "The transition matrix of 1985-1999 and its 14th root, the yearly matrix."

[[nodes]]
id = "projection"
op = "plus.demand"
inputs = { start = "$t1999", markov = "markov.markov" }
params = { steps = 31 }
note = "The cells of each class from 1999 to 2030, year by year."
```

Despite its name, [[plus.demand]] (made for PLUS's CARS) gives an ordinary demand, the same type as the other
two, so it plugs into [[lulcc.step]] as well.

The matrix of the period, as [[markov.fit]] measured it (rows: the class in 1985; columns: the class in 1999):

| 1985 \ 1999 | Forest | Built | Other |
|---|---|---|---|
| Forest | 0.900 | 0.087 | 0.013 |
| Built | 0.000 | 0.996 | 0.004 |
| Other | 0.046 | 0.082 | 0.872 |

and its yearly root has, for example, 0.0065 from forest to built: 0.65% of the forest becomes built land each
year.

### A demand written by hand: a scenario

[[demand.table]] takes the table directly: `classes`, then a row of cells per year (the first is the start),
and optionally `times`. As a counterfactual experiment, suppose Plum Island had decided in 1985 to keep its
forest area: built land grows as it did, but only into other land. The forest column stays at 49,013 cells;
the built column is the observed trend; the other column is what is left (113,563 cells minus the other two).

```tfgraph
#! caption: A scenario as a table: the forest kept at its 1985 area, built land growing as observed, other land taking the difference. (The table is in the model file.)
#! highlight: protection
format = "terraflux/1"

[[nodes]]
id = "protection"
op = "demand.table"
params = { table = { classes = [1, 2, 3], rows = [[49013, 37122, 27428], [49013, 37660, 26890], [49013, 38198, 26352], [49013, 38736, 25814], [49013, 39274, 25276], [49013, 39812, 24738], [49013, 40350, 24200], [49013, 40738, 23812], [49013, 41126, 23424], [49013, 41514, 23036], [49013, 41902, 22648], [49013, 42291, 22259], [49013, 42679, 21871], [49013, 43067, 21483], [49013, 43455, 21095]], times = [1985, 1986, 1987, 1988, 1989, 1990, 1991, 1992, 1993, 1994, 1995, 1996, 1997, 1998, 1999] } }
note = "Forest kept at 49,013 cells; built land as observed; other land the rest."
```

### Connecting a demand to the allocation

Whatever its source, a demand enters the model at two places: its `demand` output goes to the `demand` input of
[[lulcc.step]], inside the loop, and its `steps` output goes to the loop's `times`. Inside the loop, `@step`
(1, 2, 3...) tells [[lulcc.step]] which row to allocate. Here is the complete model with the scenario in place
of the trend (only node 3 changed from the model of [part 1](/tutorials/land-change/clue-s-ideas/#the-model-at-a-glance)):

```tfgraph
#! caption: CLUE-S with the scenario's demand: protection.demand feeds the allocation inside the loop, protection.steps the number of iterations.
#! highlight: protection
#! params: false
format = "terraflux/1"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

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
id = "protection"
op = "demand.table"
params = { table = { classes = [1, 2, 3], rows = [[49013, 37122, 27428], [49013, 37660, 26890], [49013, 38198, 26352], [49013, 38736, 25814], [49013, 39274, 25276], [49013, 39812, 24738], [49013, 40350, 24200], [49013, 40738, 23812], [49013, 41126, 23424], [49013, 41514, 23036], [49013, 41902, 22648], [49013, 42291, 22259], [49013, 42679, 21871], [49013, 43067, 21483], [49013, 43455, 21095]], times = [1985, 1986, 1987, 1988, 1989, 1990, 1991, 1992, 1993, 1994, 1995, 1996, 1997, 1998, 1999] } }

[[nodes]]
id = "rng"
op = "rng.r"
inputs = { seed = 1 }

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = { landscape = "$start" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = "protection.steps", landscape = "$start", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", history = "@history", suitability = "suitability.suitability", demand = "protection.demand", step = "@step", rng = "rng.rng" }
params = { allocation = { method = "clue", jitter = 0.0002, scale = 0.000001, max_iter = 1000, max_diff = 50, mean_diff = 50 }, elasticity = [0.2, 0.2, 0.2] }

[outputs.maps]
from = "simulation.maps"
```

!!! app "In the application"
    To swap a demand, add the new demand node and drag from its `steps` output onto the loop's `times`. Then
    double-click the loop to edit its body, click `allocate`, and in the side panel type `protection.demand` as
    the value of its `demand` input (a body may read any node outside it by its reference).

## Read the results

### Linear or Markov?

```tfchart
type: line
data: /data/tutorials/land-change/land-demand/trend_vs_markov.csv
x: year
y: [Forest (linear), Forest (Markov), Built (linear), Built (Markov), Other (linear), Other (Markov)]
colors: { "Forest (linear)": "#3b8a5a", "Forest (Markov)": "#9fd0b0", "Built (linear)": "#b4508a", "Built (Markov)": "#e3b3cf", "Other (linear)": "#d9a441", "Other (Markov)": "#f0d79f" }
xlabel: Year
ylabel: Cells
height: 340
caption: "The linear trend of demand.from_maps and the projection of the 1985-1999 Markov chain, from 1999 to 2030 (real outputs). Over three decades they differ by less than 1,000 cells per class at Plum Island."
```

At Plum Island the two demands stay within 1,000 cells of each other until 2030: built land 55,487 (linear)
against 55,541 (Markov); forest 38,968 against 38,173. They would part ways over longer horizons, the linear
trend running the declining classes towards zero and the Markov chain slowing towards its equilibrium. When two
reasonable demands agree, that is a reassuring sign; when they disagree, run the model with both and report the
range.

### What a demand protects: the scenario run

Both demands, the trend and the protection scenario, were run through the same CLUE-S model from 1985 to
1999 (14 steps, every step converged). The simulated maps of 1999:

```tfcompare
caption: "Drag the handle: the simulated map of 1999 with the trend's demand (left) and with the protection scenario's (right). Green forest, purple built, yellow other. The protection scenario keeps 3,700 more cells of forest, mostly at the expense of other land."
before: { src: /img/tutorials/land-change/land-demand/sim1999_trend.png, label: "Trend" }
after: { src: /img/tutorials/land-change/land-demand/sim1999_protection.png, label: "Forest protected" }
```

| Simulated 1999 | Forest | Built | Other |
|---|---|---|---|
| Trend | 45,355 | 43,427 | 24,781 |
| Forest protected | 49,026 | 43,409 | 21,128 |

The demand did what it was asked: the forest area of 1999 is that of 1985. But look at which cells are
forest:

![Forest lost and gained in the protection scenario](/img/tutorials/land-change/land-demand/protection_forest_moves.png "The protection scenario: of the forest of 1985, 7,734 cells are not forest in the simulated 1999 (red), replaced by 7,747 cells of new forest elsewhere (green).")

In the protection scenario, 7,734 cells of the 1985 forest (16%) are something else by 1999, and 7,747 new
cells of forest appear elsewhere, mostly in the west where forest's suitability is highest. The **area** is
protected; the **forest** is not. For a species that needs old forest, this scenario is not protection at all.

!!! warning "A demand decides how much, never where"
    To keep particular places as they are, use **restrictions** (cells that cannot change) or a high
    **elasticity** for the class, both in [part 5](/tutorials/land-change/clue-s-rules/). To make forest
    regrowth slow, use the conversion matrix's time codes (also part 5).

How to report a demand in a paper: its source (trend of which maps, which Markov period, which scenario), the
table itself (or its first and last rows and its rate), the conversion from hectares to cells if any, and, for
scenarios, the storyline behind each column.

## Settings that change the answer

- **The dates of the observed maps.** With three maps, the linear demand has a kink at the middle date: the
  slope of 1985–1991 before it, that of 1991–1999 after. Built land grew by 538 cells a year until 1991 and by
  388 a year afterwards, and the extrapolation keeps the later rate.
- **`out_times`.** Steps of 5 years instead of 1 make one allocation per 5 years: fewer, larger changes, and the
  elasticities and conversion rules act less often. Choose the step from the process (and keep it the same
  between calibration and projection).
- **The demand of the classes you do not care about.** In the protection scenario, *Other* absorbs everything.
  Whatever class takes the remainder in a hand-written scenario, it is a choice: say so.

## Common pitfalls

- **Rows that do not add up.** A hand-written row whose total differs from the cells of the map cannot be met:
  the allocation never converges and warns at every step. Make one class the remainder, as above.
- **Classes missing from the demand.** Every class present in the start map must have a column (water too, even
  if it never changes: give it a constant column).
- **A start row that is not the start map.** The first row of a [[demand.table]] is the start; the first step
  allocates the second row. If your table starts with the first projected year, the model skips it.
- **Hectares instead of cells.** Demands from statistics are in hectares or km²: divide by the area of a cell.
- **Extrapolating too far.** A linear trend over a short period, extended for decades, can empty a class. Look
  at the demand (`output/demand.json`) before running the model.
- **Confusing a validation with a forecast.** A model validated with the observed trend's demand has been
  given the right amounts; its projection, with an extrapolated demand, has not.

## Going further

- Part 4: [A complete CLUE-S model](/tutorials/land-change/clue-s-model/).
- Part 5: [Rules](/tutorials/land-change/clue-s-rules/): restrictions and elasticities that protect places.
- [A demand per region](/tutorials/land-change/regional-demand/): each municipality or basin with its own table.
- Markov chains in depth, in the [foundations of this category](/tutorials/#land-change).
- Reference: [[demand.from_maps]], [[demand.table]], [[markov.fit]], [[plus.demand]].

## References

- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Verburg, P. H., & Overmars, K. P. (2009). Combining top-down and bottom-up dynamics in land use modeling: exploring the future of abandoned farmlands in Europe with the Dyna-CLUE model. *Landscape Ecology*, 24(9), 1167–1181. https://doi.org/10.1007/s10980-009-9355-7
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
