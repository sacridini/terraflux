---
title: "Validating a simulation: is the model any good?"
short: Validating a simulation
description: Compare a simulated map with the observed one the right way, on the change and not on the whole map, with the figure of merit, hits, misses and false alarms, quantity and allocation disagreement and fuzzy similarity, and test a model on a period it did not see.
order: 300
level: intermediate
time: 50 min
template: Validating a simulation
ops: [validate.agreement, validate.similarity, validate.three_map, validate.futures, validate.kappa, validate.error_budget]
series: Validating and calibrating
part: 1
---

A land change model always produces a map; whether the map means anything is another question. In this first
part of the series on validation and calibration you will compare simulated maps with observed ones, the way the
literature on land change now recommends: on the **change**, not on the whole map, against the yardstick of a
model that changes nothing, at several resolutions, and on a period the model did not see. You will use the five
`validate.*` operators of Terraflux on Dinamica EGO's model of Sibuyan Island (from the
[previous series](/tutorials/land-change/dinamica-on-sibuyan/)) and on Plum Island, which has three dates. At the
end you will know which numbers to report, what good and bad values look like, and why a simulation that agrees
with 93% of the map can be worse than doing nothing.

## The idea

### Validation compares two maps that start from the same map

To validate a simulation you need three maps of the same grid: the **start** (the observed map the simulation
starts from), the **observed end** (what really happened) and the **simulated end** (what the model says
happened). Every cell is then in one of a few situations, which Pontius et al. (2008) named:

![The four outcomes of a cell on a toy grid](/img/tutorials/land-change/validating-a-simulation/toy_outcomes.png "A toy landscape of forest (green), grassland (light green) and coconut (ochre). 1, the start; 2, what was observed at the end; 3, what a model simulated; 4, every cell compared: H hits, M misses, F false alarms, W a wrong hit, grey the cells that stayed in both.")

- a **hit**: the cell changed, and the model changed it to the right class (H);
- a **miss**: the cell changed, and the model kept it as it was (M);
- a **false alarm**: the cell did not change, and the model changed it (F);
- a **wrong hit**: the cell changed, and the model changed it, but to another class (W);
- a **correct rejection**: the cell did not change in either map (grey).

### Persistence is the null model

Most of a landscape does not change in a decade. On Sibuyan, 90% of the cells hold the same class in 1997 and
2011. A "model" that changes nothing (**persistence**) therefore agrees with the observed map of 2011 in 90% of
the cells. Any model that changes the right *amount* in the wrong *places* gets each wrongly placed cell wrong
twice (a false alarm where it changed, a miss where it should have), and can agree with less of the map than
persistence does. The share of cells that agree, which looks like a natural score, mostly measures how little
the landscape changed. Agreement has to be measured on the change.

!!! science "Key idea: the figure of merit"
    \[ \text{FoM} = \frac{H}{H + M + W + F} \]

    with \(H\) hits, \(M\) misses, \(W\) wrong hits and \(F\) false alarms (counts of cells, or shares of the map).
    It is the share of the change, observed or simulated, that the model got right: 0 when no simulated change is
    right (persistence always scores 0), 1 when the simulated change is exactly the observed one. In the toy
    grid above, 3 hits, 3 misses, 1 wrong hit and 2 false alarms give \(3/9 = 0.33\).

The figure of merit was proposed by Pontius et al. (2008), who compared thirteen applications of nine land change
models and found figures of merit at the resolution of the data from about 1% to 59%, the landscapes with more
observed change tending to have the higher figures of merit. There is no universal threshold of a
"good" model: compare a model with persistence, with simpler models, and with itself at coarser resolutions.

### Quantity and allocation

A wrong cell can be wrong for two reasons: the model changed the wrong *amount* of a class (too much forest
cleared), or the right amount in the wrong *places*. Pontius and Millones (2011) split the total disagreement
between two maps into these two parts:

!!! science "Key idea: quantity and allocation disagreement"
    For each class \(j\), with \(p_{j}^{obs}\) and \(p_{j}^{sim}\) its shares of the observed and the simulated
    map, the **quantity disagreement** is \(q_j = |p_{j}^{sim} - p_{j}^{obs}|\), and the total is
    \(Q = \frac{1}{2}\sum_j q_j\). The **allocation disagreement** \(A\) is the rest of the cells that disagree:
    \(Q + A = 1 -\) the share of cells that agree. A model whose quantities come from a Markov chain fitted on
    the same two maps has \(Q \approx 0\) on the calibration period by construction; its allocation is what
    there is to judge.

They also argued that **kappa**, long the standard statistic for comparing maps, should be abandoned: it compares
agreement with that of a random allocation no one proposes as a model, and mixes quantity and allocation into one
number that is hard to interpret. **Kappa simulation** (van Vliet et al., 2011) repairs part of it by comparing the
simulated and observed *transitions* rather than the classes.

### Near misses: resolution and fuzzy similarity

A patch of grassland simulated two cells from where it really appeared is a better simulation than one on the
other side of the island, yet both count as one miss and one false alarm. Two methods give credit to near misses.

**Multiple resolutions.** Pontius et al. (2011) compare the three maps again after merging blocks of 2 × 2,
4 × 4, 8 × 8 cells...: a block holds a share of each class, and the hits, misses and false alarms are computed on
the shares. As the blocks grow, near misses become hits. The resolution at which the figure of merit becomes
acceptable is the resolution at which the simulated maps can be used.

**Fuzzy similarity.** Hagen (2003) gave each cell a *membership* in the classes of the other map, decreasing with
the distance to the nearest cell of that class, and Hagen-Zanker (2009) improved the statistic; Dinamica EGO applies
the method to the cells that changed:

!!! science "Key idea: the fuzzy similarity of the changes"
    Keep only the cells that changed between the start and each end map (by their new class). For a changed cell
    of one map, look for cells of the same new class in the other map within a window of \(w \times w\) cells,
    and take the membership

    \[ \mu = 2^{-d/2} \]

    where \(d\) is the distance (in cells) to the nearest one: 1 on the same cell, 0.71 next to it, 0.5 two cells
    away, 0 beyond the window. The similarity of the first map to the second is the mean membership of its changed
    cells; computed both ways, Dinamica's result is the **minimum** of the two, so that a model cannot score well
    by changing everything (high similarity of the observed to the simulated) or almost nothing (high similarity
    of the simulated to the observed).

![The membership of a near miss](/img/tutorials/land-change/validating-a-simulation/membership.png "The membership given to a cell of the other map at each distance from a changed cell (centre), in windows of 3 x 3 and 5 x 5 cells: 2^(-d/2), 0 outside the window.")

## When to use it

Validate every simulation whose maps you will use: the analyses that follow (habitat, connectivity, carbon) are
only as good as the simulated landscape. Validation answers *how close is the simulated change to the observed
change, at which resolution*; it does not answer whether a scenario of the future is right, which no observed map
can tell. Use:

- the **figure of merit**, with its hits, misses, false alarms and wrong hits, as the main number;
- the same at **coarser resolutions**, or the **fuzzy similarity** in growing windows, to see how far the near
  misses are;
- **quantity and allocation disagreement** to say which part of the model is wrong;
- **persistence** and a **simpler model** as yardsticks;
- landscape metrics of the simulated and observed maps (number of patches, edge density) when the shape matters,
  as in [Expander and Patcher](/tutorials/land-change/expander-and-patcher/).

Kappa and the share of cells that agree are reported by many studies; report them only next to the measures above.

## What you need

- The **start** map, the **observed end** map and the **simulated end** map, on the same grid, with the same class
  codes. The simulated map is usually the last band of a simulation's `maps.tif` (a [[raster.read]] with its band,
  or the loop's `landscape` output in the same model).
- Ideally, **three dates**: one period to fit the model, a later one to validate it (below).
- The template **Validating a simulation** (Templates, Land change) has every operator of this page on maps drawn
  by chance, each node with a note.

## Build the model

The validation can be added to the model that simulates (as in [part 3 of the Dinamica series](/tutorials/land-change/dinamica-on-sibuyan/#the-simulated-and-the-observed-change))
or kept in a model of its own that reads the maps. Here, a model of its own, reading Sibuyan's observed maps and
the last band (step 14) of the simulation's `maps.tif`:

```tfgraph
#! caption: The validation of Sibuyan's simulation: the start, the observed end and the simulated end (inputs) feed four comparisons. Each gives a table.
#! numbered: true
format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"

[inputs.simulated]
kind = "map"
path = "sibuyan/maps.tif"
band = "14"

[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "$t0", observed = "$t1", simulated = "$simulated" }
note = "The share of cells of the same class, the figure of merit and its terms."

[[nodes]]
id = "similarity"
op = "validate.similarity"
inputs = { start = "$t0", observed = "$t1", simulated = "$simulated" }
params = { windows = [1, 3, 5, 7, 9, 11] }
note = "Dinamica EGO's fuzzy similarity of the changes in windows of growing size."

[[nodes]]
id = "three_map"
op = "validate.three_map"
inputs = { t0 = "$t0", t1 = "$t1", simulated = "$simulated" }
params = { factors = [1, 2, 4, 8, 16] }
note = "Hits, misses, wrong hits and false alarms at blocks of 1 to 16 cells (Pontius et al. 2011)."

[[nodes]]
id = "disagreement"
op = "validate.futures"
inputs = { reference = "$t1", simulated = "$simulated", original = "$t0" }
note = "Quantity and allocation disagreement, kappa and kappa simulation."
```

1. **[[validate.agreement]]** (node 1): `start`, `observed`, `simulated`. Its table has one row: `overall` (the
   share of cells of the same class), `figure_of_merit`, and the counts `hits`, `misses`, `false_alarms`,
   `wrong_hits`.
2. **[[validate.similarity]]** (node 2): the same three maps; `windows` lists the window sides (odd numbers),
   `decay` is `"exponential"` (Dinamica's, the default) or `"constant"` (every cell of the window counts 1). A row
   per window: `simulated` (the similarity of the simulated change to the observed), `observed` (the other way),
   `minimum` (Dinamica's result), and the changed cells of each map.
3. **[[validate.three_map]]** (node 3, lulcc's three-map comparison): `t0`, `t1`, `simulated`, and `factors`, the
   block sides. Four tables: `budget` (misses, hits, wrong hits, false alarms and correct rejections as shares of
   the map, overall, per class and per transition, per block size), `figure_of_merit` (overall and per class, per
   block size), `transitions` (per transition) and `table` (lulcc's).
4. **[[validate.futures]]** (node 4, FUTURES' validation): the `reference` (observed end), the `simulated` map and
   the `original` (start). Its `metrics` table holds `total_quantity`, `total_allocation`, `kappa` and
   `kappasimulation`; `classes`, the quantity and allocation disagreement of each class. (For maps of 0 and 1,
   developed or not, it adds hits, misses and the figure of merit.)

[[validate.kappa]] (MOLUSCE's kappa, kappa location and histogram, and Cramér's V) and [[validate.error_budget]]
(Pontius's budget of errors at several resolutions) complete the set; both are in the template.

From the command line, make the tables outputs (`[outputs.agreement] from = "agreement.table"`, and so on) and
`terraflux run validate.toml` writes them as CSV files. In the application, select a node to see its table in
the Viewer; the **Validation** pane compares the last map of a simulation with an observed one (figure of merit,
hits, misses and false alarms, and the fuzzy similarity in windows of growing size):

![The application's Validation pane](/img/app/validation.png "The Validation pane: the last simulated map against the observed one.")

## Read the results

### Sibuyan, cell by cell

![Hits, misses, wrong hits and false alarms on Sibuyan](/img/tutorials/land-change/validating-a-simulation/outcomes.png "Every cell of Sibuyan compared: the simulation of 1997-2011 (Dinamica EGO's model, 13 drivers, seed 1) against the observed map of 2011.")

Of the 743 cells that changed between 1997 and 2011, the model got 461 right (hits), 63 to the wrong class (wrong
hits) and missed 219; it changed 247 cells that did not change (false alarms). The figure of merit is
461 / (461 + 219 + 63 + 247) = **0.47**. The map shows where: hits in the middle of the large patches of grassland
and coconut, misses and false alarms at their edges, and the small scattered changes of the east almost all missed.

```tfcompare
caption: Drag the handle between the observed change of 1997-2011 (left) and the simulated change (right), by their new class (ochre coconut, light green grassland, blue rice; grey unchanged).
before: { src: /img/tutorials/land-change/validating-a-simulation/change_observed.png, label: Observed change }
after: { src: /img/tutorials/land-change/validating-a-simulation/change_simulated.png, label: Simulated change }
```

!!! app "Why was this cell missed? Trace it"
    The map of outcomes says *where* the model was wrong; the application's **Trace** says *why* at one cell. In the
    model that simulates (the template **Dinamica EGO**, or the model of the
    [Dinamica series](/tutorials/land-change/dinamica-on-sibuyan/)), select the loop, click a cell of its map that is
    a miss or a false alarm, then **Trace this cell** and, in the Trace pane, **Trace inside the loops**: the model
    runs again keeping what the loop gives at that cell, step by step. The loop's card then tells the steps at
    which the cell changed class, each with the rate of its transition (Dinamica's Markov chain: the share of the
    cells of the origin class that change each step) or, in a CLUE-S model, the step's demand (the cells of each
    class, and how many more or fewer than at the step before); a missed cell says that its class stayed the same at
    every step. For a cell that changed, a line **why** gives its probability of that transition (Dinamica) or its
    suitability for each class (CLUE-S), ranked among the cells: a false alarm with a high probability is the
    model's map of probabilities at fault, one with a low probability the draw. Traced on a few hits, misses, wrong hits and false alarms, this tells when the model changed each
    cell and to what, beside what was observed, and how much change that step had to place. See the
    [tour](/tutorials/getting-started/tour/#why-a-cell-has-its-value-trace) for the Trace pane.

### Against the yardsticks

The same comparisons for the model, for the same model with two drivers only (elevation and slope), and for
persistence:

| | 13 drivers | 2 drivers | Persistence |
|---|---:|---:|---:|
| Cells of the same class (`overall`) | 0.927 | 0.859 | 0.897 |
| Figure of merit | **0.466** | 0.172 | 0 |
| Hits / misses / wrong hits / false alarms | 461 / 219 / 63 / 247 | 211 / 462 / 70 / 487 | 0 / 743 / 0 / 0 |
| Quantity disagreement | 0.001 | 0.001 | 0.078 |
| Allocation disagreement | 0.073 | 0.141 | 0.025 |
| Kappa | 0.879 | 0.767 | 0.820 |
| Kappa simulation | 0.609 | 0.243 | 0 |
| Fuzzy similarity, 5 × 5 window | 0.742 | 0.436 | 0 |

Read it row by row. The two-driver model agrees with **less** of the map than doing nothing (0.859 against
0.897), and kappa ranks it below persistence too, although it does get a sixth of the change right: both numbers
reward not changing. The figure of merit, kappa simulation and the fuzzy similarity rank the three as they should.
Both models have almost no quantity disagreement (their rates come from the same two maps); all their error is
allocation, twice as much with two drivers. Persistence has the opposite profile: all quantity (it changed nothing)
and little allocation.

### Near misses

```tfchart
type: line
data: /data/tutorials/land-change/validating-a-simulation/similarity.csv
x: window
y: ["13 drivers", "2 drivers"]
colors: { "13 drivers": "#3b8a5a", "2 drivers": "#e8901f" }
xlabel: Window side (cells)
ylabel: Fuzzy similarity (minimum)
height: 300
caption: Dinamica EGO's fuzzy similarity of the simulated and observed change in windows of 1 to 11 cells, for the model with 13 drivers and with 2. Most of the gain comes from the first windows (near misses one or two cells away).
```

```tfchart
type: line
data: /data/tutorials/land-change/validating-a-simulation/fom_blocks.csv
x: block
y: ["13 drivers", "2 drivers", "persistence"]
colors: { "13 drivers": "#3b8a5a", "2 drivers": "#e8901f", "persistence": "#7f7f7f" }
xlabel: Block side (cells; 1 = 250 m, 16 = 4 km)
ylabel: Figure of merit
xlog: true
height: 300
caption: The figure of merit of the three-map comparison at blocks of 1 to 16 cells. The model with 13 drivers rises from 0.47 to 0.58; the one with 2 drivers stays below 0.31 even at 4 km; persistence stays at 0.
```

Both curves rise with the window or the block, and the rise is information: the 13-driver model places most of
its errors within one or two cells of the observed change (its similarity jumps from 0.60 to 0.71 between the
windows of 1 and 3); the 2-driver model's errors are farther away. If the maps will be used at 1 km (to
summarise forest per municipality, say), the figure of merit at blocks of 4 cells is the relevant one.

```tfgallery
columns: 2
caption: The north-west of Sibuyan compared cell by cell (colours as in the map above). Left, 13 drivers; right, 2 drivers, whose grassland and coconut land in the wrong valleys.
images:
  - { src: /img/tutorials/land-change/validating-a-simulation/outcomes_13.png, caption: "13 drivers: FoM 0.47" }
  - { src: /img/tutorials/land-change/validating-a-simulation/outcomes_2.png, caption: "2 drivers: FoM 0.17" }
```

### The honest test: a period the model did not see

All the numbers above compare the simulation of 1997-2011 with the map of 2011, the map the model was fitted on.
They measure how well the model *describes* the period, not how well it would *predict* another. Plum Island
(the data of the [CLUE-S series](/tutorials/land-change/clue-s-ideas/)) has three dates, so the test can be
honest: fit on 1985-1991, then simulate from the observed map of 1991 for eight years and compare with 1999.

```bash
terraflux new dinamica --t0 pie_landuse.tif:1985 --t1 pie_landuse.tif:1991 --drivers pie_drivers.tif \
    --steps 6 --start pie_landuse.tif:1991 --simulate 8 -o validation.toml
```

```tfchart
type: line
data: /data/tutorials/land-change/validating-a-simulation/pie_periods.csv
x: block
y: ["calibration period (1985-1991)", "validation period (1991-1999)"]
colors: { "calibration period (1985-1991)": "#7f7f7f", "validation period (1991-1999)": "#3b8a5a" }
xlabel: Block side (cells of 100 m)
ylabel: Figure of merit
xlog: true
height: 300
caption: Dinamica EGO's model of Plum Island fitted on 1985-1991, its figure of merit on the calibration period (1985 to 1991) and on the validation period (1991 to 1999), at blocks of 1 to 16 cells.
```

![Plum Island, simulated against observed change on the validation period](/img/tutorials/land-change/validating-a-simulation/pie_outcomes.png "Plum Island, 1991 to 1999: the model fitted on 1985-1991 against what happened. The new built land is scattered in hundreds of small patches; the model places most of it near, but rarely on, the right cells.")

Two lessons. First, the figure of merit on the validation period (0.032 at 100 m) is the same as on the
calibration period: this model is not overfitted, it describes Plum Island as well (or as badly) in a period it did
not see. Second, it is low: the suburban growth of Plum Island is scattered in small patches that three drivers
(elevation, slope and the distance to the built land of 1985) cannot place cell by cell. The simulation agrees
with 92.0% of the map of 1999, and persistence with **95.8%**: on the map as a whole, doing nothing beats the
model. Only at blocks of 1.6 km does its figure of merit reach 0.26. Such a model can say how much built land
each part of the watershed may gain, not which fields. Its quantity disagreement (0.009) is also no longer zero:
the rates of 1985-1991 did not hold exactly in the 1990s, which no fit on the calibration period can reveal.

### How to report it

Report, for the validation period if you have one: the figure of merit with its four terms; the quantity and
allocation disagreement; the figure of merit at several resolutions (or the fuzzy similarity in several windows),
as a curve; the same numbers for persistence; and the map of hits, misses and false alarms. Say which seed or how
many replicates (the mean and range over replicates are better than one run).

### In Python

[`S.validate`][terraflux.simulate.validate] gathers the four comparisons of the model above (and
[[validate.kappa]] and [[validate.error_budget]]) in one call, on the start, the observed end and the simulated end.
The simulated map is made here too: Dinamica EGO's model of the
[Dinamica series](/tutorials/land-change/dinamica-on-sibuyan/) with the same ranges, the 14 yearly steps of
1997 to 2011 and seed 1 (the library's allocation is the application's, so the cells are the same):

```python
import pandas as pd
import terraflux as tf

S = tf.simulate
lu = tf.load_raster(S.example_data("sibuyan_landuse"))
drivers = tf.load_raster(S.example_data("sibuyan_drivers"))
t0, t1 = lu[0].where(lu[0] != 255), lu[1].where(lu[1] != 255)      # 255: outside the island

ranges = {"dist_sea": 250, "dist_roads": 250, "dist_urban": 250, "dist_streams": 250, "elevation": 50,
          "slope": 2, "aspect": 30, "pop_density": 50,
          **{c: "categorical" for c in ["diorite", "ultramafic", "sediments", "no_erosion", "moderate_erosion"]}}
model = S.presets.dinamica(t0, t1, drivers, steps=14, woe={"ranges": ranges})
simulated = model.run(t0, drivers, seed=1).maps[-1]               # the last step, 2011

v = S.validate(t0, t1, simulated, factors=[1, 2, 4, 8, 16], windows=[1, 3, 5, 7, 9, 11])
print(v)
print(S.agreement(t1, simulated, t0))
print(v.similarity.round(3))
```

```text
Validation(figure of merit 0.4657, kappa 0.8790, similarity 0.7593 at 11 cells)
{'overall': 0.9267921394962635, 'figure_of_merit': 0.46565656565656566, 'hits': 461, 'misses': 219, 'false_alarms': 247, 'wrong_hits': 63, 'quantity': {1: 1, 2: -1, 3: 3, 4: -1, 5: -2}}
   window  simulated  observed  minimum  simulated_cells  observed_cells
0       1      0.598     0.620    0.598              771             743
1       3      0.744     0.706    0.706              771             743
2       5      0.789     0.742    0.742              771             743
3       7      0.799     0.754    0.754              771             743
4       9      0.802     0.758    0.758              771             743
5      11      0.802     0.759    0.759              771             743
```

`S.agreement` is [[validate.agreement]]: `overall`, the `figure_of_merit` and its four counts (and the `quantity`
difference of each class, simulated less observed cells). `v.similarity` is [[validate.similarity]], a row per
window with the similarity of the simulated change to the observed (`simulated`), the other way (`observed`) and
Dinamica's result, the `minimum`; `decay="constant"` counts every cell of the window as 1. The three-map comparison
and the other tables are attributes of `v`:

```python
fom = v.three_map.figure_of_merit()
print(fom["overall"].round(3))                  # the figure of merit by block side (1 = 250 m, 16 = 4 km)
budget = v.three_map.agreement_budget()
print(budget[budget.level == "overall"][["factor", "hits", "misses", "wrong_hits", "false_alarms"]].round(4))
print(round(v.futures["total_quantity"], 4), round(v.futures["total_allocation"], 3), round(v.kappa["kappa"], 3))
```

```text
factor
1     0.466
2     0.484
4     0.524
8     0.563
16    0.580
Name: figure_of_merit, dtype: float64
     factor    hits  misses  wrong_hits  false_alarms
0         1  0.0638  0.0303      0.0087        0.0342
31        2  0.0652  0.0279      0.0099        0.0316
62        4  0.0682  0.0234      0.0124        0.0262
93        8  0.0711  0.0197      0.0128        0.0228
124      16  0.0664  0.0164      0.0140        0.0176
0.0006 0.073 0.879
```

`figure_of_merit()` has `overall`, `category` (the change from each class) and `transition`; the `budget` gives
shares of the map, overall, per class and per transition, per block size. `v.futures` is
[[validate.futures]] (`total_quantity`, `total_allocation`, `kappasimulation`...), `v.kappa` MOLUSCE's
kappa, `v.error_budget` Pontius's budget at coarser resolutions, and `v.summary()` the headline numbers in one
Series. Against the yardsticks of the part, the same model with elevation and slope only, and persistence (the map
of 1997 as the simulation):

```python
two = drivers.sel(band=["elevation", "slope"])
sim2 = S.presets.dinamica(t0, t1, two, steps=14, woe={"ranges": {"elevation": 50, "slope": 2}}).run(
    t0, two, seed=1).maps[-1]

def score(sim):
    a, s = S.agreement(t1, sim, t0), S.validate(t0, t1, sim).summary()
    return {"cells of the same class": a["overall"], "figure of merit": a["figure_of_merit"],
            "hits": a["hits"], "misses": a["misses"], "wrong hits": a["wrong_hits"],
            "false alarms": a["false_alarms"], "quantity disagreement": s["quantity_disagreement"],
            "allocation disagreement": s["allocation_disagreement"], "kappa": s["kappa"],
            "kappa simulation": s["kappa_simulation"], "similarity 5 x 5": s["similarity_5"]}

table = pd.DataFrame({"13 drivers": score(simulated), "2 drivers": score(sim2), "persistence": score(t0)})
print(table.round(3).to_string(float_format=lambda x: f"{x:g}"))
```

```text
                         13 drivers  2 drivers  persistence
cells of the same class       0.927      0.859        0.897
figure of merit               0.466      0.172            0
hits                            461        211            0
misses                          219        462          743
wrong hits                       63         70            0
false alarms                    247        487            0
quantity disagreement         0.001      0.001        0.078
allocation disagreement       0.073       0.14        0.025
kappa                         0.879      0.767         0.82
kappa simulation              0.609      0.243            0
similarity 5 x 5              0.742      0.436            0
```

These are the part's table, row by row (only the allocation disagreement of two drivers differs, in the last digit:
0.140 here, 0.141 there). The curves of the part are `v.similarity["minimum"]` by window and `fom["overall"]` by block
side. A model simulated by another program is validated the same way: read its map with
[`tf.load_raster`][terraflux.load_raster] and pass it as `simulated`.

The honest test is the same call on a period the model did not see. Plum Island has three dates: fit on 1985-1991,
simulate eight steps from the observed map of 1991 and compare with 1999. The steps of the Markov chain are 6 (the
length of the fitted period), so the allocation is told to take 8:

```python
pie = tf.load_raster(S.example_data("pie_landuse"))
pie_drivers = tf.load_raster(S.example_data("pie_drivers"))
m85, m91, m99 = (pie.sel(band=b).where(pie.sel(band=b) != 255) for b in ["1985", "1991", "1999"])

pie_model = S.presets.dinamica(m85, m91, pie_drivers, steps=6,
                               woe={"ranges": {"elevation": 5, "slope": 1, "dist_built_1985": 100}})
calibration = S.validate(m85, m91, pie_model.run(m85, pie_drivers, seed=1).maps[-1])   # 1985 to 1991
pie_model.allocation.steps = 8                                                          # 1991 to 1999
sim99 = pie_model.run(m91, pie_drivers, seed=1).maps[-1]
validation = S.validate(m91, m99, sim99)                                                # start: 1991

curve = pd.DataFrame({"calibration period (1985-1991)": calibration.three_map.figure_of_merit()["overall"],
                      "validation period (1991-1999)": validation.three_map.figure_of_merit()["overall"]})
print(curve.round(3))
a = S.agreement(m99, sim99, m91)
print(f"agree with 1999: {a['overall'] * 100:.1f}%, persistence {S.agreement(m99, m91, m91)['overall'] * 100:.1f}%")
print(validation.summary()[["quantity_disagreement", "allocation_disagreement"]].round(3))
```

```text
        calibration period (1985-1991)  validation period (1991-1999)
factor                                                               
1                                0.032                          0.032
2                                0.042                          0.043
4                                0.072                          0.074
8                                0.145                          0.148
16                               0.275                          0.264
agree with 1999: 92.0%, persistence 95.8%
quantity_disagreement      0.009
allocation_disagreement    0.072
Name: value, dtype: float64
```

The curve is the part's chart: the figure of merit on the validation period (0.032 at 100 m) equals the one on
the calibration period, and reaches 0.26 at blocks of 1.6 km. Persistence agrees with more of the map (95.8 %
against 92.0 %), and the quantity disagreement is no longer zero (0.009). Note the `start`: hits and misses are
defined from the map the simulation started from (1991 here, not 1985).

## Settings that change the answer

- **The window and the decay of the similarity.** With `decay = "constant"`, every cell of the window counts 1,
  so the similarity grows faster with the window; Dinamica's exponential decay is the usual choice. Always say
  which window a number is for.
- **The block sizes** of the three-map comparison (`factors`). Blocks that do not divide the map leave a margin;
  report the block sizes in cells and in metres.
- **Which map is the start.** Hits and misses are defined from the start map: comparing a simulation from 1991 with
  the map of 1999 needs `start` = 1991, not 1985.
- **The seed.** On Sibuyan, five seeds give figures of merit from 0.43 to 0.48 and similarities (5 × 5) from 0.72
  to 0.77. Differences between models smaller than that are noise; compare means over replicates
  ([Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/)).

## Common pitfalls

- **Reporting the share of cells that agree.** It mostly measures persistence. A model at 93% may be worse than
  doing nothing.
- **Validating on the calibration period** and calling it validation. Say "goodness of fit" for it, and keep a
  later period for validation whenever three dates exist.
- **Comparing kappa between landscapes.** Kappa depends on how much changed and on the shares of the classes;
  it cannot say whether a model is useful.
- **Maps with different no-data.** A cell without a value in any of the three maps is left out of the comparison
  ([[validate.three_map]] says so); check that the simulated map has the same island or study area as the observed.
- **One run.** A figure of merit from one seed is one draw.
- **Forgetting the resolution of use.** If the maps will be summarised at 1 km, judge them at 1 km.

## Going further

- [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/): replicates, and settings chosen
  by these scores.
- [Maps larger than memory](/tutorials/land-change/maps-larger-than-memory/): the same validation on large maps.
- [Dinamica EGO on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/) and the [complete CLUE-S
  model](/tutorials/land-change/clue-s-model/), validated the same way.
- The operators: [[validate.agreement]], [[validate.similarity]], [[validate.three_map]], [[validate.futures]],
  [[validate.kappa]], [[validate.error_budget]].

## References

- Hagen, A. (2003). Fuzzy set approach to assessing similarity of categorical maps. *International Journal of Geographical Information Science*, 17(3), 235–249. https://doi.org/10.1080/13658810210157822
- Hagen-Zanker, A. (2009). An improved Fuzzy Kappa statistic that accounts for spatial autocorrelation. *International Journal of Geographical Information Science*, 23(1), 61–73. https://doi.org/10.1080/13658810802570317
- Pontius, R. G., Jr., Boersma, W., Castella, J.-C., Clarke, K., de Nijs, T., Dietzel, C., et al. (2008). Comparing the input, output, and validation maps for several models of land change. *The Annals of Regional Science*, 42(1), 11–37. https://doi.org/10.1007/s00168-007-0138-2
- Pontius, R. G., Jr., & Millones, M. (2011). Death to Kappa: birth of quantity disagreement and allocation disagreement for accuracy assessment. *International Journal of Remote Sensing*, 32(15), 4407–4429. https://doi.org/10.1080/01431161.2011.552923
- Pontius, R. G., Jr., Peethambaram, S., & Castella, J.-C. (2011). Comparison of three maps at multiple resolutions: a case study of land change simulation in Cho Don District, Vietnam. *Annals of the Association of American Geographers*, 101(1), 45–62. https://doi.org/10.1080/00045608.2010.517742
- van Vliet, J., Bregt, A. K., & Hagen-Zanker, A. (2011). Revisiting Kappa to account for change in the accuracy assessment of land-use change models. *Ecological Modelling*, 222(8), 1367–1375. https://doi.org/10.1016/j.ecolmodel.2011.01.017
