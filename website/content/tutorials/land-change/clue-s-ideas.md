---
title: "Demand-driven allocation: the ideas of CLUE-S"
short: "CLUE-S: the ideas"
description: How CLUE-S and lulcc decide where land use goes when the amount of each class is fixed in advance, shown on real runs over the Plum Island Ecosystems.
order: 500
level: intermediate
time: 35 min
ops: [lulcc.step, glm.fit, glm.predict, demand.from_maps, lulcc.history, rng.r, control.repeat]
series: "Demand-driven allocation: CLUE-S and lulcc"
part: 1
---

This series builds a complete CLUE-S model, the most widely used model of land use change driven by a
*demand*: the area each land use must cover in each year is decided first, and the model's job is to find
where that area goes. In this first part you will learn the logic of the model without building anything yet:
what a demand is, what a *suitability* map is, and how CLUE-S makes classes compete for cells, step by step,
until every class has its area. Everything is shown on real runs of Terraflux over a real landscape, the
Plum Island Ecosystems of north-eastern Massachusetts, which the R package lulcc uses as its example. At the end
you will be able to read a CLUE-S model, know when it suits a question, and know what the next parts will
build.

## The idea

### Three questions every land change model answers

A land change model answers three questions (Verburg et al., 2004):

1. **How much** of each land use will there be? Hectares of cropland in 2030, the built area a growing population
   needs, a rate of deforestation.
2. **Where** will it be? Which cells become cropland, which forest is cleared first.
3. **Under which rules**? Built land does not become forest in a year; a nature reserve stays as it is; an
   orchard resists conversion more than a pasture.

Many models answer the first two questions together, from the same observed change (Dinamica EGO's Markov
chains and weights of evidence, for example: see [What land change models are](/tutorials/land-change/what-land-change-models-are/) and the
[first half of this category](/tutorials/#land-change)).
CLUE-S separates them. The *how much* comes from outside the map, as a table of areas per year called the
**demand**: a trend of the observed maps, the output of an economic model, a policy target, a storyline of a
scenario. The map's job is only the *where*. This split is the reason CLUE-S is so common in scenario studies:
the same spatial model can be run with a "business as usual" demand, a "forest protection" demand and an
"agricultural expansion" demand, and the maps differ only because the demands differ.

!!! science "Key idea: top-down quantity, bottom-up location"
    A demand-driven model takes the **quantity** of each land use as given (top-down: from regional statistics,
    projections or scenarios) and decides its **location** cell by cell (bottom-up: from the local conditions of
    each cell, its *suitability*). The demand of a class at a year is a number of cells; the allocation must
    make the map hold exactly (or nearly) that many cells of the class.

### Where it comes from

The **CLUE** framework (*Conversion of Land Use and its Effects*) was developed at Wageningen University by
Veldkamp and Fresco (1996) to study land use in Costa Rica on a coarse grid of large cells, each holding a
mixture of land uses. Verburg et al. (1999) formalised its allocation procedure: the land uses compete for the
cells, and their competition is adjusted until the national areas are met. Three years later, Verburg et al.
(2002) published **CLUE-S** (the *S* for *small* regional extents): the same idea at the fine resolution of
regional studies, with one land use per cell, a logistic regression for the suitability of each land use, and
decision rules for the conversions. Its demonstration was Sibuyan Island in the Philippines. The model then
grew into **Dyna-CLUE** (Verburg & Overmars, 2009), which added conversions that follow their own local dynamics
(the regrowth of abandoned farmland), and **CLUMondo**, which allocates land *systems* that supply goods and
services (see [CLUMondo](/tutorials/land-change/clumondo/)).

The original CLUE-S was a closed Windows program. Moulds, Buytaert and Mijic (2015) re-implemented it, together
with a simpler **ordered** allocation (Fuchs et al., 2013), in the open-source R package **lulcc**, with the
Plum Island data as its example. Terraflux runs lulcc's algorithm through the landschaft library: the same
regressions (R's `glm` operation for operation), the same demand, the same decision rules and even the same
random numbers as R (the `rng.r` node gives R's generator after `set.seed()`), so a Terraflux run of lulcc's
model gives the same maps as lulcc itself.

### The landscape: Plum Island Ecosystems

The Plum Island Ecosystems (PIE) are a coastal landscape of forest, towns, farmland and salt marsh north of
Boston, studied by a Long Term Ecological Research site. lulcc's example data map it on 434 × 497 cells of about
100 m (Massachusetts State Plane, EPSG:26986) in three classes, in 1985, 1991 and 1999 (Pontius & Parmentier,
2014, used the same maps to teach the ROC):

| Code | Class | 1985 | 1999 |
|---|---|---|---|
| 1 | Forest | 49,013 cells (43.2%) | 45,377 cells (40.0%) |
| 2 | Built | 37,122 cells (32.7%) | 43,455 cells (38.3%) |
| 3 | Other (all the land that is neither forest nor built) | 27,428 cells (24.2%) | 24,731 cells (21.8%) |

![The observed maps of 1985, 1991 and 1999](/img/tutorials/land-change/clue-s-ideas/observed.png "The three observed maps of Plum Island Ecosystems (lulcc's example data). Built land grows, mostly into forest; the coastal east is mostly *Other*.")

Between 1985 and 1999, 7.6% of the cells changed class. The map of the change (made in Terraflux with one
`map.calc` node, `if a == b then 0 else a * 10 + b`) shows the process the model must reproduce: suburban
growth scattered across the whole watershed, mostly into forest (4,250 cells) and into *Other* land (2,248 cells),
and some *Other* land returning to forest (1,259 cells).

![The change between 1985 and 1999](/img/tutorials/land-change/clue-s-ideas/change.png "Where Plum Island changed between 1985 and 1999. Most change is new built land, in many small patches spread over the watershed.")

### How much: the demand

The demand is a table: a row per year, a column per class, the number of cells of each class. When there are
observed maps, the simplest demand is their trend: [[demand.from_maps]] counts the cells of each class in each
map and draws straight lines through them, year by year (lulcc's `approxExtrapDemand`). For Plum Island the
demand of 1986 is 48,683 forest cells, 37,660 built and 27,220 other: one sixth of the way from the 1985 counts
to the 1991 ones. Part 3, [The demand](/tutorials/land-change/land-demand/), builds demands of every kind,
including scenarios written by hand.

### Where: the suitability

For each class, a **suitability** map gives every cell a value between 0 and 1: how likely the class is there,
given the cell's *drivers* (its elevation, its slope, its distance to built land...). CLUE-S estimates it with a
**logistic regression** of the presence of the class on the drivers ([[glm.fit]], then [[glm.predict]]): a
class found mostly on flat land near towns gets a high suitability on flat land near towns. Part 2,
[Suitability from logistic regressions](/tutorials/land-change/logistic-suitability/), fits and reads them.

### The competition between classes

The heart of CLUE-S is how it turns suitabilities into one map that meets the demand. Every cell takes the
class with the largest **total probability**:

!!! science "Key idea: the total probability of a class in a cell"
    \[ TPROP_{i,u} = P_{i,u} + ELAS_u + ITER_u \]

    - \( P_{i,u} \) is the suitability of class \(u\) in cell \(i\) (from the logistic regression);
    - \( ELAS_u \) is the **conversion elasticity** of class \(u\), added only when \(u\) is the cell's current
      class: a bonus for staying as it is (0 for a class that changes easily, up to 1 for one that hardly ever
      changes);
    - \( ITER_u \) is the **iteration variable** of class \(u\): one number per class, the same for all cells,
      that the model adjusts until the areas are right.

    Each cell takes the class \(u\) with the highest \(TPROP_{i,u}\) (Verburg et al., 2002).

Take six real cells of Plum Island. Their suitabilities (from the regressions of part 2) are close: in cell 3,
built land wins by 0.03. With the suitabilities alone, two cells would be forest, two built and two other.

![Six cells and their suitabilities](/img/tutorials/land-change/clue-s-ideas/six_cells.png "The suitability of each class in six real cells of Plum Island. Each cell takes the class of the highest bar.")

Now suppose the demand asks for more built land than the suitabilities give. The model raises Built's
iteration variable. Adding 0.1 to Built's suitability everywhere does not change the cells where another class
leads by far (cell 5), but it flips the close cells: cells 1, 2 and 6 now become built.

![The same cells with 0.1 added to Built](/img/tutorials/land-change/clue-s-ideas/six_cells_boost.png "The same six cells with Built's iteration variable raised by 0.1: the cells where Built was close behind now take it.")

That is the whole trick. Over the whole map, raising a class's iteration variable takes cells from the other
classes, starting with the cells where the class was nearly winning. The model repeats:

<div class="steps" markdown>

1. Every cell takes the class of highest \(TPROP\) (with a tiny random *jitter* added to break ties).
2. The cells of each class are counted and compared with the demand.
3. Each class's iteration variable moves by `scale` × (demand − cells allocated): up for a class short of
   cells, down for a class with too many.
4. When the largest difference between allocation and demand is below `max_diff` cells, and the mean below
   `mean_diff`, the step is done. Otherwise back to 1, up to `max_iter` passes.

</div>

Then the next year starts from the map just allocated, with the next row of the demand; its iteration
variables start again from zero (as in lulcc).

### Watching the iterations

The dashboard below is made of real runs: the first step of Plum Island's model (from 1985 to 1986) stopped
after 1, 2, 3... iterations (`max_iter`), with the settings of lulcc's vignette. The map shows the cells the
step changed, by their new class; the numbers compare the cells of each class with the demand of 1986.

```tfslider
label: Iterations allowed (max_iter)
caption: "The first step of CLUE-S on Plum Island, stopped after a given number of iterations (real runs). At the first pass every cell takes the class of highest suitability plus elasticity, far from the demand; as the iteration variables move, the counts close in on it. The allocation met the demand (within 50 cells) at the 52nd iteration. Grey: cells that kept their class; colours: the class a cell took."
frames:
  - { value: 1, src: /img/tutorials/land-change/clue-s-ideas/iter_1.png, stats: { "Forest": "58,305 (asked 48,683)", "Built": "33,330 (asked 37,660)", "Other": "21,928 (asked 27,220)", "Cells changed": "11,916" } }
  - { value: 2, src: /img/tutorials/land-change/clue-s-ideas/iter_2.png, stats: { "Forest": "57,514 (asked 48,683)", "Built": "33,474 (asked 37,660)", "Other": "22,575 (asked 27,220)", "Cells changed": "11,676" } }
  - { value: 3, src: /img/tutorials/land-change/clue-s-ideas/iter_3.png, stats: { "Forest": "56,818 (asked 48,683)", "Built": "33,629 (asked 37,660)", "Other": "23,116 (asked 27,220)", "Cells changed": "11,493" } }
  - { value: 5, src: /img/tutorials/land-change/clue-s-ideas/iter_5.png, stats: { "Forest": "55,548 (asked 48,683)", "Built": "33,894 (asked 37,660)", "Other": "24,121 (asked 27,220)", "Cells changed": "11,285" } }
  - { value: 10, src: /img/tutorials/land-change/clue-s-ideas/iter_10.png, stats: { "Forest": "53,184 (asked 48,683)", "Built": "34,453 (asked 37,660)", "Other": "25,926 (asked 27,220)", "Cells changed": "11,325" } }
  - { value: 20, src: /img/tutorials/land-change/clue-s-ideas/iter_20.png, stats: { "Forest": "50,271 (asked 48,683)", "Built": "36,011 (asked 37,660)", "Other": "27,281 (asked 27,220)", "Cells changed": "12,182" } }
  - { value: 30, src: /img/tutorials/land-change/clue-s-ideas/iter_30.png, stats: { "Forest": "49,051 (asked 48,683)", "Built": "37,122 (asked 37,660)", "Other": "27,390 (asked 27,220)", "Cells changed": "12,813" } }
  - { value: 40, src: /img/tutorials/land-change/clue-s-ideas/iter_40.png, stats: { "Forest": "48,758 (asked 48,683)", "Built": "37,499 (asked 37,660)", "Other": "27,306 (asked 27,220)", "Cells changed": "12,982" } }
  - { value: 60, src: /img/tutorials/land-change/clue-s-ideas/iter_60.png, stats: { "Forest": "48,690 (asked 48,683)", "Built": "37,610 (asked 37,660)", "Other": "27,263 (asked 27,220)", "Cells changed": "13,023" } }
chart:
  type: line
  data: /data/tutorials/land-change/clue-s-ideas/iterations.csv
  x: iterations
  y: [Forest, Built, Other, Forest demand, Built demand, Other demand]
  colors: { Forest: "#3b8a5a", Built: "#b4508a", Other: "#d9a441", "Forest demand": "#9fd0b0", "Built demand": "#e3b3cf", "Other demand": "#f0d79f" }
  xlabel: Iterations
  ylabel: Cells of the class
```

Two things stand out, and both will matter in the parts to come.

- **The demand is met, the map is not "right".** The counts converge on the demand, but nothing in the
  iterations checks that the cells chosen are where the change really happened. That is the job of the
  suitability (part 2), the rules (part 5) and the validation against an observed map (part 4).
- **The net change is small, the gross change is not.** The demand of 1986 differs from the 1985 map by a few
  hundred cells per class, yet the converged step changed 13,023 cells (11% of the landscape), many of them
  built cells turned into forest. With an elasticity of only 0.2, any cell whose suitability favours another
  class by more than 0.2 switches. Real built land does not turn into forest in a year: part 5,
  [Rules](/tutorials/land-change/clue-s-rules/), shows how elasticities and a conversion matrix stop that.

## When to use it

CLUE-S fits questions where the **amount** of each land use is known or assumed, and the question is **where**
it will go and what that means for the landscape:

- **Scenarios of regional land use.** The demand comes from storylines or economic models, and the spatial
  model is the same for all of them (Verburg et al., 2002; Verburg & Overmars, 2009). Ecologists then read the
  maps for habitat loss, fragmentation or connectivity.
- **Policy targets.** "No net loss of forest", "20% more cropland for biofuels": the target is the demand, and
  the model shows which places pay for it.
- **Several classes changing at once**, in competition: farmland and towns both growing into forest. CLUE-S
  allocates all classes together, unlike models that handle one transition at a time.
- **Reconstructions of past land use** from statistics, where the areas are known by region and year and their
  location is not (the ordered allocation of Fuchs et al., 2013: part 6).

It is not the best choice when:

- **The amount itself is the question.** CLUE-S takes the demand as given; it does not predict it. A Markov
  chain or an economic model must come first.
- **The shape of the change matters most.** CLUE-S allocates cell by cell and tends to scatter change over the
  many cells of similar suitability. Deforestation frontiers that grow as patches, with sizes and shapes, are
  the domain of Dinamica EGO's Expander and Patcher (see the [first half of this category](/tutorials/#land-change))
  or of [PLUS](/tutorials/land-change/plus-cars/) and [FUTURES](/tutorials/land-change/futures/).
- **Decisions of people are the subject.** If the question is how farmers respond to a subsidy, the
  decision-makers must be in the model: see [agents](/tutorials/land-change/agents/) and
  [CRAFTY](/tutorials/land-change/crafty/).

[Choosing a land change model](/tutorials/land-change/choosing-a-model/) compares all of them.

## What you need

- **A map of land use at the start**, with integer class codes (1, 2, 3...) and a no-data value outside the study
  area. Every class of the map must have a demand.
- **Drivers**: rasters on the same grid (same extent, cell size and coordinate system) with the variables that
  explain where each class is: elevation, slope, distances to roads, towns or rivers, soils, population
  density. A *stack* (one raster with a band per driver, each band named) is the most convenient form.
- **A demand**: the cells of each class in each year. With two or more observed maps, Terraflux interpolates it
  for you.
- **Optionally**: restricted areas (reserves), a map of how long each cell has had its class, conversion
  rules (parts 5 and 6).

This series uses lulcc's Plum Island data. Download them to follow along:
[pie_landuse.tif](/data/tutorials/land-change/clue-s-ideas/pie_landuse.tif) (three bands, named 1985, 1991
and 1999; 255 is no data) and [pie_drivers.tif](/data/tutorials/land-change/clue-s-ideas/pie_drivers.tif)
(bands `elevation`, `slope` and `dist_built_1985`, the distance to the built land of 1985 in metres). They come from lulcc (GPL 2 or later).

!!! app "In the application"
    **Templates → Land change → CLUE-S** opens a small CLUE-S model on maps drawn by chance: two dates of
    land use, two drivers, the regressions, the demand and the loop. It runs at once (++f5++) and is a good
    place to click through the nodes. Its allocation sets `scale = 0.000005` and `max_iter = 5000`: with lulcc's
    default `scale` (0.0005) its 25,600 cells never converge (see the settings below). The operators are in the library under **Land change simulation →
    lulcc and CLUE-S**.

## The model at a glance

The whole model is seven nodes. The parts of this series build it piece by piece; here it is once, numbered,
so you can see where each idea lives.

```tfgraph
#! caption: CLUE-S on Plum Island, as `terraflux new clue` writes it (made compact; settings hidden, open the model file to see them). Nodes 1 and 2 give the suitability, node 3 the demand, nodes 4 and 5 what the loop needs, and the loop (node 6) runs `lulcc.step` (node 7) once per year.
#! numbered: true
#! params: false
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
params = { allocation = { method = "clue", jitter = 0.0002, scale = 0.000001, max_iter = 1000, max_diff = 50, mean_diff = 50 }, elasticity = [0.2, 0.2, 0.2] }

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
```

| Node | Operator | What it gives | Explained in |
|---|---|---|---|
| 1 | [[glm.fit]] | a logistic regression per class | [part 2](/tutorials/land-change/logistic-suitability/) |
| 2 | [[glm.predict]] | the suitability maps, a layer per class | [part 2](/tutorials/land-change/logistic-suitability/) |
| 3 | [[demand.from_maps]] | the cells of each class in each year, and the number of steps | [part 3](/tutorials/land-change/land-demand/) |
| 4 | [[rng.r]] | R's random number generator, seeded | [part 4](/tutorials/land-change/clue-s-model/) |
| 5 | [[lulcc.history]] | how long each cell has had its class | [part 5](/tutorials/land-change/clue-s-rules/) |
| 6 | [[control.repeat]] | the loop over the years | [part 4](/tutorials/land-change/clue-s-model/) |
| 7 | [[lulcc.step]] | one year of allocation | parts [1](/tutorials/land-change/clue-s-ideas/), [4](/tutorials/land-change/clue-s-model/), [5](/tutorials/land-change/clue-s-rules/), [6](/tutorials/land-change/ordered-allocation/) |

The same model can be written by one command, from the observed maps and the drivers:

```bash
terraflux new clue --observed pie_landuse.tif --times 1985,1991,1999 --drivers pie_drivers.tif \
  --formulas '{"1": ["elevation", "slope"], "2": ["elevation", "slope", "dist_built_1985"], "3": ["elevation", "slope"]}' \
  --elasticity 0.2,0.2,0.2 \
  --settings '{"jitter": 0.0002, "scale": 0.000001, "max_iter": 1000, "max_diff": 50, "mean_diff": 50}' \
  -o clue.toml
terraflux run clue.toml        # output/maps.tif (a band per year) and output/steps.csv
```

## Read the results

What this part's runs show, and how to read the same things in your own model:

- **`steps.csv`** (the `info` table of each step, collected by the loop) has, for each year, the number of
  iterations, whether the allocation converged (`converged` = 1), the largest and mean differences from the
  demand in cells, and the cells of each class. Look at `converged` first: a 0 means the map of that year does
  not meet the demand, and Terraflux also prints a warning.
- **The cells changed per step** are not in the table; compare consecutive maps (a `map.calc` of
  `a != b`). A gross change much larger than the net change of the demand is a sign of low elasticities or
  weak regressions.
- **The maps** (`maps.tif`, a band per year) are what you analyse afterwards: landscape metrics, habitat,
  connectivity.

In a paper, report the demand (its source and its table), the regressions (their drivers and coefficients),
the elasticities and rules, the allocation's settings, and whether every step converged.

## Settings that change the answer

The allocation has five settings, all in `allocation` of [[lulcc.step]]:

| Setting | What it does | lulcc's vignette | Default |
|---|---|---|---|
| `jitter` | half-width of the random noise added to the suitabilities at each pass | 0.0002 | 0.0001 |
| `scale` | how far the iteration variables move per cell of difference | 0.000001 | 0.0005 |
| `max_iter` | passes before giving up (the last allocation is kept) | 1000 | 1000 |
| `max_diff` | largest difference from the demand accepted (cells) | 50 | 5 |
| `mean_diff` | mean absolute difference accepted (cells) | 50 | 5 |

The defaults are lulcc's, and they do **not** suit Plum Island: with `scale` = 0.0005, a difference of 4,000
cells moves an iteration variable by 2, ten times the range of the suitabilities, and the allocation swings
from one class to another without ever settling (part 4 shows that run). A rule of thumb: `scale` × (a typical
difference in cells) should be a small fraction of the gaps between suitabilities, about 0.01. The tolerances
`max_diff` and `mean_diff` should be small next to the yearly change of the demand (Plum Island's built land
grows by about 450 cells a year, so 50 cells is acceptable; 5 is very strict).

The elasticities change the answer far more than these settings: they decide how much of the landscape is
reshuffled every year. Part 5 has a dashboard over them.

## Common pitfalls

- **Classes without a demand.** Every class present in the start map must be in the demand, or the step stops
  with "the landscape has classes without demand". A stray code (0 for water, say) must be reclassified or made
  no data.
- **No data in a driver.** A cell missing any driver of a class's regression gets no suitability for that class
  and cannot take it. Check that the drivers cover the whole map.
- **Demand that does not add up.** A demand made by hand must add up, in every row, to the cells of the map
  with a class. [[demand.from_maps]] guarantees it; a table written by hand must be checked.
- **Mistaking convergence for accuracy.** A converged allocation meets the areas; only a comparison with an
  observed map says whether the places are right.
- **Ignoring the warnings.** "The allocation did not meet the demand in 1000 iterations" means the map of that
  year has the wrong areas. Fix `scale` (smaller) or the tolerances (larger) before reading the maps.

## Going further

- Part 2: [Suitability from logistic regressions](/tutorials/land-change/logistic-suitability/).
- Part 3: [The demand: how much of each class](/tutorials/land-change/land-demand/).
- Part 4: [A complete CLUE-S model](/tutorials/land-change/clue-s-model/).
- Part 5: [Rules: elasticities, conversions, neighbourhoods and restrictions](/tutorials/land-change/clue-s-rules/).
- Part 6: [The ordered allocation](/tutorials/land-change/ordered-allocation/).
- The other families of models: [Choosing a land change model](/tutorials/land-change/choosing-a-model/).
- Reference: [[lulcc.step]], [[demand.from_maps]], [[glm.fit]].

## References

- Fuchs, R., Herold, M., Verburg, P. H., & Clevers, J. G. P. W. (2013). A high-resolution and harmonized model approach for reconstructing and analysing historic land changes in Europe. *Biogeosciences*, 10(3), 1543–1559. https://doi.org/10.5194/bg-10-1543-2013
- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Pontius, R. G., Jr., & Parmentier, B. (2014). Recommendations for using the relative operating characteristic (ROC). *Landscape Ecology*, 29(3), 367–382. https://doi.org/10.1007/s10980-013-9984-8
- Veldkamp, A., & Fresco, L. O. (1996). CLUE: a conceptual model to study the Conversion of Land Use and its Effects. *Ecological Modelling*, 85(2–3), 253–270. https://doi.org/10.1016/0304-3800(94)00151-0
- Verburg, P. H., de Koning, G. H. J., Kok, K., Veldkamp, A., & Bouma, J. (1999). A spatial explicit allocation procedure for modelling the pattern of land use change based upon actual land use. *Ecological Modelling*, 116(1), 45–61. https://doi.org/10.1016/S0304-3800(98)00156-2
- Verburg, P. H., & Overmars, K. P. (2009). Combining top-down and bottom-up dynamics in land use modeling: exploring the future of abandoned farmlands in Europe with the Dyna-CLUE model. *Landscape Ecology*, 24(9), 1167–1181. https://doi.org/10.1007/s10980-009-9355-7
- Verburg, P. H., Schot, P. P., Dijst, M. J., & Veldkamp, A. (2004). Land use change modelling: current practice and research priorities. *GeoJournal*, 61(4), 309–324. https://doi.org/10.1007/s10708-004-4946-y
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
