---
title: "Comparing maps: quantity, allocation and resolution"
short: Comparing two maps
description: Judge a map against a reference with quantity and allocation disagreement instead of kappa, see how the difference changes at coarser resolutions, compare continuous maps, and estimate areas of change from a sample.
order: 30
level: intermediate
time: 60 min
ops: [validate.futures, validate.kappa, change.components, change.resolutions, change.composite, change.memberships, change.mad, change.sample_to_population, change.simple, table.of]
series: Change detection
part: 3
---

Sooner or later every land change study compares a map with a **reference**: a simulated map with the map that
was observed, a new classification with field data, one producer's land cover with another's. In this tutorial
you will compare Sibuyan's observed land use of 2011 with two predictions of it, the **null model** (the 1997 map,
which predicts no change) and a map **simulated** by a land change model. You will measure how they disagree with
the reference in **quantity** and in **allocation** (Pontius & Millones, 2011), see why the popular kappa index
misleads, follow the difference as the maps are compared at coarser and coarser resolutions, compare two
continuous maps, and finally turn a reference **sample** into estimates of the areas of change with confidence
intervals, as good practice asks (Olofsson et al., 2014).

## The idea

Two maps of classes on the same grid can be cross-tabulated, as in
[part 1](/tutorials/change-detection/transitions/). When one is a reference, the rows are the **comparison** map
(the one being judged) and the columns the **reference**; the diagonal is **agreement** and the rest is
**disagreement**. Disagreement has two very different causes, and a single number hides which one is at work:

- the map has the wrong **quantity** of a class: too much forest, too little rice;
- the map has the right quantities but puts the classes in the wrong **places**: its **allocation** is wrong.

A land change model can be right about how much forest will be lost and wrong about where; a classification can
put the forest edge in the right place and still mislabel a whole class. The remedies differ: a better demand
(how much) or better suitability maps (where).

!!! science "Key idea: quantity and allocation disagreement"
    With \(p_{ij}\) the share of the map in class \(i\) on the comparison map and class \(j\) on the reference,
    \(p_{i+}\) and \(p_{+j}\) the row and column totals:

    \[ Q = \tfrac{1}{2} \sum_j \left| p_{+j} - p_{j+} \right| \qquad
       D = 1 - \sum_j p_{jj} \qquad A = D - Q \]

    \(D\) is the total disagreement, the share of the cells that differ; \(Q\), the **quantity disagreement**, is
    what the class totals alone force; \(A\), the **allocation disagreement**, is what remains: cells that could
    have agreed by swapping places. Each class \(j\) has its own \(q_j = |p_{+j} - p_{j+}|\) and
    \(a_j = 2\min(p_{+j} - p_{jj},\, p_{j+} - p_{jj})\); the totals are half their sums.

Robert Gilmore Pontius Jr. and Marco Millones proposed these two numbers in a paper titled "Death to Kappa"
(Pontius & Millones, 2011). Kappa, the index most papers report, compares the agreement with the agreement
expected by chance. But "chance" is a strange baseline for a map, and kappa mixes quantity and allocation into a
number that cannot be interpreted; two maps with the same kappa can fail for opposite reasons, and a map can have
a higher kappa than a better one. Foody (2020) explains in detail why kappa is unsuitable for judging the accuracy
of classified maps. Quantity and allocation disagreement are in the units everyone understands (a share of the
map), they add up to the total disagreement, and they say what is wrong.

Allocation itself splits into **exchange** and **shift**, as change did in part 1 (Pontius & Santacruz, 2014):
exchange is a pair of classes in each other's places, shift what is left.

A second idea is **resolution**. A model that puts a forest loss 250 m from where it happened is wrong at 250 m
and right at 1 km. Comparing the maps at coarser and coarser resolutions shows at what distance the allocation
errors become agreement. To do it without redrawing the classes, each coarse cell holds the *shares* of the
classes in its block of fine cells, and the agreement of two blocks is computed from those shares (the
**composite** method of Pontius & Cheuk, 2006). Quantity disagreement does not change with resolution; allocation
disagreement shrinks as errors fall within the same block.

!!! science "Key idea: a null model"
    To judge a simulated map of time 2, compare it with a map that predicts **no change**: the map of time 1.
    Most landscapes change little, so the null model is often very accurate overall, and a model that cannot beat
    it at predicting *where* change happens has not learned much (Pontius et al., 2008). The figure of merit, the
    share of the cells changed in either map that the model got right, focuses on the change instead of the
    persistence.

## When to use it

- **Validating a land change model**: the simulated map of the end of the calibration period against the
  observed one, with the null model as a benchmark (and the three-map comparison of the
  [land change simulation](/tutorials/#land-change) tutorials, [[validate.three_map]], for the change itself).
- **Assessing a classification**: a land cover map against reference data, where quantity disagreement says the
  areas are biased and allocation disagreement says the boundaries are wrong.
- **Comparing two products** of the same date (two land cover datasets, two forest masks) before choosing one.
- **Continuous maps** (a forest's share, a biomass, a suitability): the mean absolute deviation and its
  components ([[change.mad]]).
- **Estimating areas of change**: when a map is used to report how much forest was lost, the area should be
  estimated from a reference sample, not counted on the map (Olofsson et al., 2014).

What it is not for: none of these numbers says whether the map is good enough for a purpose; that depends on the
question. And they do not test significance: a model 1% better than another may not be meaningfully better.

## What you need

- `sibuyan_landuse.tif`, as in parts 1 and 2: Sibuyan's land use in 1997 and 2011 (1 forest, 2 coconut,
  3 grassland, 4 rice, 5 other; cells of 250 m; modified for teaching).
- `simulated_2011.tif`: a map of 2011 simulated from the 1997 map by a land change model. It was made with
  Terraflux's ordered allocation model (`terraflux new ordered`, lulcc's OrderedModel): logistic regressions of
  each class on 13 drivers (distance to the sea and to roads, elevation, slope, soils...), and the **observed
  2011 areas** as its demand. Because it was given the true quantities, its quantity disagreement must be zero:
  everything it gets wrong is allocation. That makes it a clear example, and a typical calibration run.

![The null model, the observed 2011 and the simulated 2011](/img/tutorials/change-detection/comparing-maps/three_maps.png "Three maps of Sibuyan: the 1997 map as a prediction of 2011 (null model), the observed 2011 (the reference) and the simulated 2011. The simulation added grassland in the east and kept the forest of the north-west.")

```tfcompare
caption: "The observed 2011 (left) and the simulated 2011 (right), in the classes' colours: forest dark green, coconut light green, grassland ochre, rice blue, other grey. Drag the handle: the simulation has the same areas, in partly different places."
before: { src: /img/tutorials/change-detection/comparing-maps/observed.png, label: "Observed 2011" }
after: { src: /img/tutorials/change-detection/comparing-maps/simulated.png, label: "Simulated 2011" }
```

## Build the model

### Disagreement and kappa

Read the three maps with [[raster.read]] (library: **IO**): the 1997 band, the 2011 band (the reference), and the
simulated map. Then add [[validate.futures]] from the **Land change simulation** subject: it is the validation of
FUTURES (`r.futures.validation`), and it computes Pontius and Millones's quantity and allocation disagreement per
class and in total, with kappa. Connect the observed map to `reference`, the simulated one to `simulated`, and the
1997 map to `original` (with the original it also computes kappa simulation; van Vliet et al., 2011).

Add a second [[validate.futures]] with the 1997 map as `simulated`: that is the null model. And add
[[validate.kappa]] (MOLUSCE's comparison) to each pair, to see the kappas side by side.

```tfgraph
#! caption: The simulated map and the null model (the 1997 map), each compared with the observed 2011: quantity and allocation disagreement, and kappa.
#! highlight: disagreement, null_model, kappa, kappa_null
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "observed"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }
note = "The reference."

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "simulated_2011.tif" }

[[nodes]]
id = "disagreement"
op = "validate.futures"
inputs = { reference = "observed.raster", simulated = "simulated.raster", original = "land_1997.raster" }
note = "Quantity and allocation disagreement, kappa, kappa simulation."

[[nodes]]
id = "null_model"
op = "validate.futures"
inputs = { reference = "observed.raster", simulated = "land_1997.raster" }
note = "The 1997 map as a prediction of 2011."

[[nodes]]
id = "kappa"
op = "validate.kappa"
inputs = { reference = "observed.raster", simulated = "simulated.raster" }

[[nodes]]
id = "kappa_null"
op = "validate.kappa"
inputs = { reference = "observed.raster", simulated = "land_1997.raster" }
```

[[validate.futures]] gives two tables: `metrics`, one row with `total_quantity`, `total_allocation`, `kappa` and,
with an original map, `kappasimulation` (its other columns, hits, misses and the figure of merit, are for maps of
0 and 1 only, and stay empty here); and `classes`, a row per class with its `quantity` and `allocation`. Both are
shares (0 to 1), not percentages. [[validate.kappa]]'s table has `kappa`, `kappa_location`, `kappa_histogram`,
`correctness` (the % of cells that agree), Cramer's V, the joint information uncertainty and the correlation.

### Exchange and shift, and a map of the errors

The disagreement is a cross-tabulation, so [[change.components]] (library: **Change detection**) splits it as it
split change in part 1. Connect the simulated map to `from` and the observed one to `to`, and set `analysis` to
`error`: the columns become omission, agreement and commission, and the allocation splits into exchange and shift.

To see *where* the simulation went wrong, add a [[raster.calc]] with three inputs named `t0` (1997), `o` (observed)
and `s` (simulated), and this expression:

```text
if o != t0 and s == o then 1
else if o != t0 and s != t0 then 2
else if o != t0 then 3
else if s != t0 then 4
else 0
```

(written on one line in the node). It classes every cell as a **hit** (1: changed, and simulated to the right
class), a **wrong hit** (2: changed, simulated to another class), a **miss** (3: changed, simulated as unchanged),
a **false alarm** (4: unchanged, simulated as changed) or a correct persistence (0).

```tfgraph
#! caption: The disagreement of the simulated map split into exchange and shift, and a map of hits, wrong hits, misses and false alarms.
#! highlight: components, outcome, plot
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "observed"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "simulated_2011.tif" }

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "simulated.raster", to = "observed.raster" }
params = { analysis = "error" }
note = "Rows: the simulated map; columns: the reference."

[[nodes]]
id = "outcome"
op = "raster.calc"
inputs = { t0 = "land_1997.raster", o = "observed.raster", s = "simulated.raster" }
params = { expression = "1 if o != t0 and s == o else 2 if o != t0 and s != t0 else 3 if o != t0 else 4 if s != t0 else 0" }
note = "1 hit, 2 wrong hit, 3 miss, 4 false alarm, 0 persistence."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "outcome.map" }
params = { title = "Hits, misses and false alarms" }
```

=== "Application"

    Save the model next to the two files and press ++f5++. Select the `disagreement` node to see its tables in
    the Viewer; the `plot` node opens the map of the outcomes in a tab of its own.

=== "Command line"

    Add the tables as outputs and run the model:

    ```toml
    [outputs.disagreement]
    from = "disagreement.metrics"

    [outputs.disagreement_classes]
    from = "disagreement.classes"

    [outputs.null_model]
    from = "null_model.metrics"
    ```

    ```bash
    terraflux run compare_2011.toml --out output
    ```

## Read the results

### The simulated map against the null model

| | Null model (1997 map) | Simulated 2011 |
|---|---:|---:|
| Agreement (`correctness`) | **89.7%** | 87.9% |
| Quantity disagreement | 7.8% | **0.0%** |
| Allocation disagreement | **2.5%** | 12.1% |
| Kappa | **0.820** | 0.800 |
| Kappa location | **0.950** | 0.800 |
| Kappa histogram | 0.864 | **1.000** |
| Kappa simulation | (none) | 0.392 |

```tfchart
type: bar
stacked: true
data: /data/tutorials/change-detection/comparing-maps/disagreement.csv
x: map
y: [quantity, allocation]
labels: { quantity: "Quantity disagreement", allocation: "Allocation disagreement" }
colors: { quantity: "#2f6f9f", allocation: "#e8901f" }
ylabel: "% of the island"
height: 260
caption: "The total disagreement with the observed 2011, split into quantity and allocation (validate.futures). The null model is wrong about how much; the simulation about where."
```

The null model agrees with 2011 on 89.7% of the island; the simulated map on 87.9%. A model that predicts no
change at all beats the simulation in overall agreement and in kappa, as happens to many land change models
(Pontius et al., 2008). The split explains why, and what each got wrong:

- The **null model** is all quantity: it has the forest of 1997, 7.8% of the island too much. Its allocation
  disagreement (2.5%) is the swap of part 1: the classes that moved without changing area.
- The **simulation** has the right quantities (it was given them), and all of its 12.1% of disagreement is
  allocation: it put the change in the wrong places. 7.5% is exchange (cells of two classes in each other's
  places) and 4.6% shift.

Kappa sees none of this. The two kappas are nearly equal (0.82 and 0.80) for opposite failures; kappa location
and kappa histogram try to separate them, but they are ratios against chance whose values are hard to interpret.
Kappa simulation (van Vliet et al., 2011) compares the simulated *transitions* with those expected by chance: at
0.39, the model's changes agree with the observed ones better than chance, but not by much.

### Where the simulation was right

![Hits, wrong hits, misses and false alarms of the simulated map](/img/tutorials/change-detection/comparing-maps/outcome.png "The outcome of the simulation at every cell. The model placed grassland in the east-centre (false alarms, red) where the forest persisted, and missed the grassland that appeared in the north-west (misses, orange).")

Of the cells that changed in either map, the model got 294 right (hits, 4.1% of the island), 116 changed to the
wrong class (1.6%), 333 were missed (4.6%) and 425 were false alarms (5.9%). The **figure of merit** is
294 / (294 + 116 + 333 + 425) = **25.2%**: of all the change, observed or simulated, a quarter was simulated
right. The map shows the pattern behind the number: the model expected the forest to be cleared from the east,
next to the farmland, while in reality the largest clearing happened in the north-west. Its errors are not
random noise: they are a wrong guess about where the frontier was.

!!! tip "The figure of merit and the three-map comparison"
    [[validate.agreement]] gives the figure of merit and its four terms directly, and [[validate.three_map]] the
    full budget of hits, misses, wrong hits and false alarms per class and per transition at several
    resolutions. Both are in the template **Validating a simulation** (menu **Templates → Land change**) and in
    the [land change simulation](/tutorials/#land-change) tutorials.

### Per class

```tfchart
type: bar
data: /data/tutorials/change-detection/comparing-maps/disagreement_classes.csv
x: class
y: [null_quantity, null_allocation, sim_quantity, sim_allocation]
labels: { null_quantity: "Null: quantity", null_allocation: "Null: allocation", sim_quantity: "Simulated: quantity", sim_allocation: "Simulated: allocation" }
colors: { null_quantity: "#9fbcd4", null_allocation: "#f2c58f", sim_quantity: "#2f6f9f", sim_allocation: "#e8901f" }
ylabel: "% of the island"
height: 300
caption: "Each class's quantity and allocation disagreement (validate.futures's classes table) for the null model and the simulated map. The totals are half the sums over the classes."
```

The simulation's allocation errors are largest for forest (7.1%) and grassland (7.2%), the two sides of the
frontier it misplaced. "Other" land is instructive: it barely changed in reality (2 cells), but the model moved
3.3% of the island's worth of it around, because nothing told the model that this class does not change. A land
change model should be given the transitions that are allowed (a conversion matrix) and the classes that stay.

### At coarser resolutions

[[change.resolutions]] (diffeR's `differenceMR`) cross-tabulates the two maps at their own resolution, then by the
composite method in blocks of 2, 4, 8... cells per side, and gives the quantity, exchange and shift at each, in %
of the cells compared. [[change.memberships]] gives the share of each class in every block (a layer per class),
and [[change.composite]] the composite cross-tabulation at one block size.

```tfgraph
#! caption: The comparison at coarser resolutions: the components at blocks of 2, 4, 8... cells, the forest's share in blocks of 4 x 4 cells on both maps, and their composite cross-tabulation.
#! highlight: resolutions, obs_shares, sim_shares, composite
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "observed"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "simulated_2011.tif" }

[[nodes]]
id = "resolutions"
op = "change.resolutions"
inputs = { comp = "simulated.raster", ref = "observed.raster" }
params = { factor = 2 }
note = "Blocks of 1, 2, 4, 8... cells per side."

[[nodes]]
id = "obs_shares"
op = "change.memberships"
inputs = { map = "observed.raster" }
params = { factor = 4 }
note = "Each class's share in blocks of 4 x 4 cells (1 km)."

[[nodes]]
id = "sim_shares"
op = "change.memberships"
inputs = { map = "simulated.raster" }
params = { factor = 4 }

[[nodes]]
id = "composite"
op = "change.composite"
inputs = { comp = "simulated.raster", ref = "observed.raster" }
params = { factor = 4 }
note = "Agreement of a class in a block: the smaller of its two shares."
```

The slider shows the difference between the simulated and the observed forest share in blocks of 1 to 16 cells
(250 m to 4 km), with the components at each resolution:

```tfslider
label: Block (cells per side)
caption: "Simulated less observed forest share in blocks of 1 to 16 cells (blue: the model has too much forest; red: too little), and the quantity, exchange and shift between the two maps at each resolution (change.resolutions), in % of the cells compared."
frames:
  - { value: 1, src: /img/tutorials/change-detection/comparing-maps/forest_diff_1.png, caption: "250 m: every cell is right (0) or wholly wrong (±1)", stats: { Quantity: "0.00%", Exchange: "7.50%", Shift: "4.59%", Agreement: "87.9%" } }
  - { value: 2, src: /img/tutorials/change-detection/comparing-maps/forest_diff_2.png, caption: "500 m: 1,740 complete blocks", stats: { Quantity: "0.65%", Exchange: "6.95%", Shift: "3.31%", Agreement: "89.1%" } }
  - { value: 4, src: /img/tutorials/change-detection/comparing-maps/forest_diff_4.png, caption: "1 km: 408 complete blocks", stats: { Quantity: "0.54%", Exchange: "6.30%", Shift: "3.24%", Agreement: "89.9%" } }
  - { value: 8, src: /img/tutorials/change-detection/comparing-maps/forest_diff_8.png, caption: "2 km: 92 complete blocks", stats: { Quantity: "0.88%", Exchange: "5.41%", Shift: "2.44%", Agreement: "91.3%" } }
  - { value: 16, src: /img/tutorials/change-detection/comparing-maps/forest_diff_16.png, caption: "4 km: 17 complete blocks", stats: { Quantity: "2.96%", Exchange: "3.16%", Shift: "0.38%", Agreement: "93.5%" } }
chart:
  type: line
  data: /data/tutorials/change-detection/comparing-maps/blocks.csv
  x: resolution
  y: [quantity, exchange, shift]
  labels: { quantity: "Quantity", exchange: "Exchange", shift: "Shift" }
  colors: { quantity: "#2f6f9f", exchange: "#e8901f", shift: "#b4508a" }
  xlabel: "Resolution (m)"
  ylabel: "% of the cells compared"
```

As the blocks grow, misplaced forest and its misplaced grassland fall in the same block and cancel: the
disagreement drops from 12.1% at 250 m to 6.5% at 4 km, and the shift almost vanishes (4.6% to 0.4%). The model's
errors are local, within a couple of kilometres. The red band of the east-centre, though, survives at 2 km: the
model did put forest loss on the wrong side of the island.

The quantity *rises*, from 0 to 3%, although the two maps have exactly the same areas. That is not a property of
the model but of the island: [[change.resolutions]] and [[change.composite]] leave out every block that has a
cell without data or that is cut by the map's edge, as diffeR does. On an island, the coastal blocks drop out
first, and at 4 km only 17 inland blocks remain, whose areas need not match. Read the coarse resolutions of a
map with irregular borders with care; the composite agreement in the stats is computed on the same complete
blocks.

For the null model, the same table ([[change.resolutions]] with the 1997 map as `comp`) falls from 10.3% to 5.0%
at 4 km, mostly in quantity, which coarsening cannot remove; the numbers are in this tutorial's data,
`resolutions.csv`.

### Continuous maps: the mean absolute deviation

Not every map has classes. To compare two continuous maps, [[change.mad]] computes the **mean absolute
deviation** between them and splits it into components (diffeR's MAD; Pontius, Thontteh & Chen, 2008): **quantity** (the
difference of their means), **strata** (differences between the strata's means that cancel across strata), and
**element** (differences between cells that cancel within a stratum), at the maps' resolution and at blocks of
2, 4, 8... cells.

Here the continuous maps are the forest's share in a 5 × 5 window around every cell (a [[raster.calc]] with
`nbmean(m == 1, 2)`), on the observed and the simulated 2011, with the land use of 1997 as strata:

```tfgraph
#! caption: The forest's share around every cell on both maps, and their mean absolute deviation with the 1997 land use as strata.
#! highlight: mad
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "observed"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "simulated_2011.tif" }

[[nodes]]
id = "observed_forest"
op = "raster.calc"
inputs = { m = "observed.raster" }
params = { expression = "nbmean(m == 1, 2)" }
note = "The forest's share in the 5 x 5 cells around."

[[nodes]]
id = "simulated_forest"
op = "raster.calc"
inputs = { m = "simulated.raster" }
params = { expression = "nbmean(m == 1, 2)" }

[[nodes]]
id = "mad"
op = "change.mad"
inputs = { grid1 = "simulated_forest.map", grid2 = "observed_forest.map", strata = "land_1997.raster" }
note = "Quantity, strata and element, at every resolution."
```

![The forest's share on the observed and the simulated maps](/img/tutorials/change-detection/comparing-maps/forest_share.png "The forest's share in 5 x 5 cells around every cell, observed (left) and simulated (right) in 2011: two continuous maps.")

```tfchart
type: area
stacked: true
data: /data/tutorials/change-detection/comparing-maps/mad.csv
x: resolution
y: [quantity, strata, element]
labels: { quantity: "Quantity", strata: "Strata", element: "Element" }
colors: { quantity: "#2f6f9f", strata: "#e8901f", element: "#b4508a" }
xlabel: "Resolution"
ylabel: "Mean absolute deviation (share of forest)"
height: 300
caption: "The mean absolute deviation between the simulated and observed forest shares and its components at every resolution (change.mad)."
```

At 250 m the two maps differ by 0.068 of forest share on average: 0.0003 of quantity (the mean shares are almost
equal, as the areas were), 0.004 of strata (the model has slightly too much forest around some classes of 1997 and
too little around others) and 0.064 of element. The element component is the allocation of the continuous map;
it shrinks as the blocks grow (0.034 at 16 km) and disappears when the whole island is one block.

### Areas of change from a sample

Everything above treats the reference map as truth. In practice the reference is a **sample**: points visited in
the field or interpreted on high-resolution images, each with its reference class. Olofsson et al. (2014) set out
the good practice for using such a sample to assess a map of change and to **estimate the areas** of each class
of change. Its central message: the area of forest loss counted on the map (pixel counting) is biased by the
map's errors; the sample gives an unbiased estimate, with a confidence interval.

The design they recommend is a **stratified random sample**, using the map's classes as strata. Suppose the map
is Sibuyan's forest change between 1997 and 2011 ([[change.simple]] with `foreground = [1]`): 4,160 cells of
stable forest, 563 of forest loss and 2,503 of stable non-forest. From each stratum, 100 points are drawn and
labelled by an interpreter. The table below is **a made-up sample, for illustration only**:

| Map stratum \ Reference | Stable forest | Forest loss | Stable non-forest | Points |
|---|---:|---:|---:|---:|
| Stable forest | 96 | 3 | 1 | 100 |
| Forest loss | 9 | 86 | 5 | 100 |
| Stable non-forest | 1 | 4 | 95 | 100 |

!!! science "Key idea: the stratified estimator of area"
    With \(n_{ij}\) the sample points of map stratum \(i\) with reference class \(j\), \(n_{i}\) the points of
    stratum \(i\), \(W_i\) its share of the map and \(A\) the map's area, the estimated share of class \(j\) is

    \[ \hat p_{\cdot j} = \sum_i W_i \frac{n_{ij}}{n_{i}} \qquad \hat A_j = A \, \hat p_{\cdot j} \qquad
       S(\hat p_{\cdot j}) = \sqrt{\sum_i W_i^2 \, \frac{\tfrac{n_{ij}}{n_{i}} \left(1 - \tfrac{n_{ij}}{n_{i}}\right)}{n_{i} - 1}} \]

    and \(\hat A_j \pm 1.96\, A\, S(\hat p_{\cdot j})\) is its 95% confidence interval (Olofsson et al., 2014).

[[change.sample_to_population]] (diffeR's `sample2pop`) does the first step: it scales each row of the sample's
cross-tabulation to its stratum's mapped size, \(\text{size}_i \times n_{ij} / n_i\). With the strata's sizes in
cells, the columns of the result sum to the estimated areas \(\hat A_j\) in cells. The two tables are written in
[[table.of]] nodes, a table typed in the node's settings (library: **Other**; or press ++slash++ over the canvas
and type its name):

```tfgraph
#! caption: A sample's cross-tabulation scaled to the mapped size of each stratum: the columns' sums are the estimated areas.
#! highlight: scaled
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "sample"
op = "table.of"
params = { columns = ["map", "1", "2", "3"], rows = [[1, 96, 3, 1], [2, 9, 86, 5], [3, 1, 4, 95]] }
note = "Rows: map strata (1 stable forest, 2 loss, 3 stable non-forest); columns: reference classes. Made up."

[[nodes]]
id = "mapped"
op = "table.of"
params = { columns = ["category", "size"], rows = [[1, 4160], [2, 563], [3, 2503]] }
note = "The cells of each stratum on the map (from change.simple's stats)."

[[nodes]]
id = "scaled"
op = "change.sample_to_population"
inputs = { matrix = "sample.table", population = "mapped.table" }
```

The scaled table, in cells, and its column sums in km²:

| Map stratum \ Reference | Stable forest | Forest loss | Stable non-forest |
|---|---:|---:|---:|
| Stable forest | 3993.6 | 124.8 | 41.6 |
| Forest loss | 50.7 | 484.2 | 28.2 |
| Stable non-forest | 25.0 | 100.1 | 2377.9 |
| **Estimated area (km²)** | **254.3** | **44.3** | **153.0** |

![Mapped and estimated areas of the forest's change](/img/tutorials/change-detection/comparing-maps/areas.png "The areas of stable forest, forest loss and stable non-forest counted on the map (grey) and estimated from the made-up sample (green), with 95% confidence intervals.")

With this sample, forest loss is estimated at **44.3 ± 10.9 km²** (95%), against 35.2 km² counted on the map: the
map *misses* loss (3 of the 100 points of its stable forest stratum are loss, and that stratum covers most of the
island), so pixel counting underestimates it. The same table gives the accuracies: the **user's accuracy** of a class (the share of
its map stratum that is right: 86% for loss) and the **producer's accuracy** (the share of the class's estimated
area that the map labels right: 68% for loss), and the overall accuracy (94.9%). Terraflux gives the scaled
table; the standard errors are the short calculation of the formula above, in a spreadsheet or a script.

!!! warning "Only what the operator does"
    [[change.sample_to_population]] scales the rows; it does not draw the sample, compute standard errors or
    allocate the points among strata. The sample size and its allocation (Olofsson et al. recommend at least 50 to
    100 points in the rare classes of change) are decided before the fieldwork.

!!! paper "How to report it"
    For a model: overall agreement, quantity and allocation disagreement (and exchange and shift), for the model
    and for the null model, and the figure of merit; a map of hits, misses and false alarms; the components at
    several resolutions. For a map of change: the sample design, the error matrix in estimated area proportions,
    user's, producer's and overall accuracy, and the estimated areas with their confidence intervals (Olofsson et
    al., 2014). Report kappa only if a journal insists, and then with the disagreement components beside it.

## Settings that change the answer

- **Which map is the reference.** In [[change.components]] and [[change.resolutions]], `ref`/`to` gives the
  columns. Swapping the maps swaps omission and commission and producer's and user's accuracy.
- **`factor`** of [[change.resolutions]] (default 2): blocks grow by this factor at every step. A factor of 3
  gives blocks of 1, 3, 9, 27 cells.
- **`percent`** of [[change.resolutions]]: % of the cells compared (default) or proportions.
- **`population`** of [[change.resolutions]]: a table of each class's population, to scale the first matrix
  as [[change.sample_to_population]] does, when the comparison map is a sample.
- **Strata** of [[change.mad]]: without them, the strata component is zero; with them, it says whether the
  errors are between regions (or classes) or within them.
- **The null model.** Which map counts as "no change" matters: the map of the start of the simulation, on the
  same grid and legend as the simulated one.

## Common pitfalls

- **Reporting kappa alone.** It cannot tell quantity from allocation errors and often ranks a useless map above
  a useful one; here it preferred the null model.
- **No null model.** A simulation with 88% agreement sounds good until a map of no change scores 90%.
- **The sample treated as the map.** Counting the area of loss on the map and reporting it as the area of loss
  ignores the map's errors, which are usually largest in the rare classes of change.
- **Blocks dropped at coarse resolutions.** Composite comparisons leave out blocks with missing cells or cut by
  the edge: on an island, a coast or a watershed, the domain shrinks as the blocks grow, and the quantity
  component can change for that reason alone.
- **Shares, not percentages.** [[validate.futures]] reports shares (0.12), [[change.resolutions]] percentages
  (12.1); [[validate.kappa]]'s `correctness` is a percentage.
- **Comparing maps of different legends.** Two products rarely share classes; translate one legend into the other
  first ([[raster.reclassify]]) and say how.

## Going further

- [Part 1](/tutorials/change-detection/transitions/) and [part 2](/tutorials/change-detection/intensity-analysis/):
  the change between two dates, and its intensity.
- [Part 4](/tutorials/change-detection/forest-gain-and-loss/): GuidosToolbox's change of a forest's structure.
- [Land change simulation](/tutorials/#land-change): how simulated maps such as this one are made and validated
  with Dinamica EGO's fuzzy similarity and the three-map comparison.
- Operators: [[validate.futures]], [[validate.futures_grid]] (the same per tile or region), [[validate.kappa]],
  [[validate.error_budget]] (Pontius's budget of agreement at several resolutions), [[change.resolutions]],
  [[change.composite]], [[change.memberships]], [[change.mad]], [[change.sample_to_population]].

## References

- Foody, G. M. (2020). Explaining the unsuitability of the kappa coefficient in the assessment and comparison of the accuracy of thematic maps obtained by image classification. *Remote Sensing of Environment*, 239, 111630. https://doi.org/10.1016/j.rse.2019.111630
- Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for estimating area and assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42–57. https://doi.org/10.1016/j.rse.2014.02.015
- Pontius, R. G., Jr., Boersma, W., Castella, J.-C., Clarke, K., de Nijs, T., Dietzel, C., Duan, Z., Fotsing, E., Goldstein, N., Kok, K., Koomen, E., Lippitt, C. D., McConnell, W., Mohd Sood, A., Pijanowski, B., Pithadia, S., Sweeney, S., Trung, T. N., Veldkamp, A. T., & Verburg, P. H. (2008). Comparing the input, output, and validation maps for several models of land change. *The Annals of Regional Science*, 42(1), 11–37. https://doi.org/10.1007/s00168-007-0138-2
- Pontius, R. G., Jr., & Cheuk, M. L. (2006). A generalized cross-tabulation matrix to compare soft-classified maps at multiple resolutions. *International Journal of Geographical Information Science*, 20(1), 1–30. https://doi.org/10.1080/13658810500391024
- Pontius, R. G., Jr., & Millones, M. (2011). Death to Kappa: birth of quantity disagreement and allocation disagreement for accuracy assessment. *International Journal of Remote Sensing*, 32(15), 4407–4429. https://doi.org/10.1080/01431161.2011.552923
- Pontius, R. G., Jr., & Santacruz, A. (2014). Quantity, exchange, and shift components of difference in a square contingency table. *International Journal of Remote Sensing*, 35(21), 7543–7554. https://doi.org/10.1080/2150704X.2014.969814
- Pontius, R. G., Jr., Thontteh, O., & Chen, H. (2008). Components of information for multiple resolution comparison between maps that share a real variable. *Environmental and Ecological Statistics*, 15(2), 111–142. https://doi.org/10.1007/s10651-007-0043-y
- van Vliet, J., Bregt, A. K., & Hagen-Zanker, A. (2011). Revisiting Kappa to account for change in the accuracy assessment of land-use change models. *Ecological Modelling*, 222(8), 1367–1375. https://doi.org/10.1016/j.ecolmodel.2011.01.017
