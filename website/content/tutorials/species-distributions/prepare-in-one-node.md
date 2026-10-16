---
title: The whole preparation in one node
short: Preparation in one node
description: Clean, deduplicate and thin the records, draw the accessible area, select predictors, draw background points and make folds with sdm.prepare - every step's settings, what its report and settings outputs say, and how the choices change a model's scores.
order: 70
level: intermediate
time: 35 min
template: Species distribution model
ops: [sdm.prepare, sdm.fit, sdm.clean_occurrences, sdm.one_per_cell, sdm.thin, sdm.accessible_area, sdm.select_predictors, sdm.background, sdm.dataset, sdm.spatial_blocks]
series: Species distribution models
part: 7
---

Parts 2 and 3 prepared the sloth's records one node at a time: cleaning, one record per cell, thinning, the accessible
area, the predictors, the background, the folds. In practice you will often do all of it in one node,
[[sdm.prepare]], which runs the same steps in the same order with one setting each, and reports what every step did.
In this part you will see every setting of that node, read its two outputs that document a model (`report` and
`settings`), and measure how much each choice of the preparation changes the scores of the same model: more than the
choice of algorithm did in [part 4](../fitting-models/).

## The idea

A species distribution model is a chain of decisions before any model is fitted. Each one is defensible on its own,
and together they decide what the model can learn and how it will be judged (Araújo et al., 2019, list them among the
standards of a good model). Bundling them in one node has three advantages:

- **every choice is in one place**, so a model can be read, changed and rerun at a glance;
- **one seed** drives every random step (thinning, background, folds), so the same settings give the same records
  on any computer;
- **a report** counts the records each step removed, and a **settings** output lists every setting actually used,
  defaults included: the methods section of a paper, ready to copy.

The node runs these steps, in this order:

<div class="steps" markdown>

1. **clean**: CoordinateCleaner's tests of suspicious coordinates ([[sdm.clean_occurrences]], Zizka et al., 2019).
2. **one per cell**: a record per cell of the predictors ([[sdm.one_per_cell]]).
3. **thin**: records dropped until none is closer than a distance to another ([[sdm.thin]], spThin's algorithm,
   Aiello-Lammens et al., 2015).
4. **area**: the accessible area ([[sdm.accessible_area]]).
5. **predictors**: the predictors without collinearity inside the area ([[sdm.select_predictors]]).
6. **background**: background points inside the area ([[sdm.background]]).
7. **predictor values**: the predictors at the records and background points ([[sdm.dataset]]).
8. **folds**: the records split into folds for cross-validation (spatial blocks by default, [[sdm.spatial_blocks]]).

</div>

!!! science "Key idea: one setting per step"
    Each step's setting takes `false` (skip the step), `true` (run it with its node's defaults), **a number** (its
    main setting: the thinning distance in metres, the radius of the area in metres, the VIF threshold, the number of
    background points, the number of folds) or **an object** of the settings of its node, for anything more
    (`thin = { distance = 50000, reps = 20 }`, `area = { method = "convex_hull", distance = 100000 }`).

## When to use it

- **Almost always**, once you know what each step does (parts 2 and 3). It is what the template **Species
  distribution model** uses.
- Use the **separate nodes** when you need to look at a step's own outputs (CoordinateCleaner's `counts` per test,
  the variograms of the autocorrelation range, the VIF table), to put your own step in between (a target-group bias
  for the background, part 2), or to teach the steps one by one.

## What you need

- The **records**: points, or a table with coordinate columns. Columns named `x`/`y`, `lon`/`lat`,
  `longitude`/`latitude` or GBIF's `decimalLongitude`/`decimalLatitude` are found by themselves; `x` and `y` name
  others. A `species` setting keeps only the rows of one species of a table that holds several.
- The **predictors**: a stack (by name) or a map.
- Optionally: an `area` (polygons) or `background` points of your own, in place of those steps; `regions` (polygons
  such as ecoregions, for `area = { method = "regions" }`); `within` (polygons the area is cut to: the land of a
  country, a continent).

## Build the model

```tfgraph
#! caption: The careful preparation of parts 2 and 3 in one node (node 3): records cleaned and one per cell, thinned at 50 km, an accessible area of 500 km, predictors below a VIF of 10, 2 000 background points and five folds of spatial blocks of 500 km; then the model of part 5.
#! numbered: true
#! highlight: data
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
params = { clean = true, one_per_cell = true, thin = 50000, area = 500000, predictors = 10, background = 2000, folds = { size = 500000, k = 5 }, seed = 1 }

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
params = { regmult = 2, classes = "lqh" }

[[nodes]]
id = "predict"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

[outputs.report]
from = "data.report"

[outputs.settings]
from = "data.settings"
```

The settings of node 3, one per step:

| Setting | Here | Default | What it does |
|---|---|---|---|
| `clean` | `true` | `true` | CoordinateCleaner's tests with their defaults (equal coordinates, 0/0, capitals, centroids, the sea, GBIF's headquarters, institutions); an object turns tests on or off (`{ urban = true, outliers = true }`) |
| `one_per_cell` | `true` | `true` | one record per cell of the predictors |
| `thin` | `50000` | off | the least distance between records, in metres |
| `area` | `500000` | off (the whole extent) | the radius of the buffer around the records, in metres, or `{ method, distance }` |
| `predictors` | `10` | `10` | the VIF threshold, or `{ vif, cor, keep }` |
| `background` | `2000` | `10000` | the number of background points, or `{ n, bias... }` |
| `folds` | `{ size = 500000, k = 5 }` | 5 spatial blocks | `k` blocks, or `{ method = "blocks", "quadrants", "checkerboard", "random", "jackknife" or "clusters", ... }`; blocks take `size = "auto"` (the predictors' range of autocorrelation) |
| `seed` | `1` | `1` | the seed of every random step |

Its outputs:

- `records`: the table for [[sdm.fit]]: `x`, `y`, `presence`, the predictors kept and `fold`;
- `env`: the predictors kept, to map the model;
- `area`: the accessible area (or the extent of the predictors);
- `presences` and `background_points`: the records left and the background, to draw or to evaluate (part 5);
- `report` and `settings`, below.

!!! app "In the application"
    **Templates → Species → Species distribution model** has the node with settings for maps drawn by chance
    (`clean = false`, since its points are not longitudes and latitudes). Select it, change the settings in the Node
    pane, and press ++ctrl+enter++: the `report` appears under the canvas. The settings that take an object are
    written in the Node pane as TOML, `{ size = 500000, k = 5 }`.

## Read the results

### The report

The `report` output, a row per step:

| Step | Presences | Removed | Detail |
|---|---|---|---|
| input | 116 | 0 | |
| clean | 113 | 3 | capitals 1, seas 1, institutions 1 |
| one per cell | 92 | 21 | |
| thin | 83 | 9 | 50000 m |
| area | 83 | 0 | |
| predictors | 83 | 0 | dropped bio1, bio6, bio8, bio9, bio12 |
| background | 83 | 0 | 2000 points |
| predictor values | 83 | 0 | |
| folds | 83 | 0 | 5 folds |

![The records each step removed.](/img/tutorials/species-distributions/prepare-in-one-node/steps.png "The sloth's 116 records and the step that removed each: three failed a test of coordinates (a capital, the sea, an institution), 21 fell in a cell that already had a record, and 9 were thinned out in the dense clusters of Colombia, Panama and the Guianas. The 83 kept, the accessible area within 500 km of them and its 2 000 background points.")

Read the report as the record of what your data went through. Losing 28 % of the records is normal for GBIF data
at 0.5°; losing most of them would ask whether the resolution, the thinning distance or a test is too strict.

### The settings

The `settings` output (JSON) lists every setting used, the defaults filled in, and what was kept:

```text
{
 "area": { "distance": 500000 },
 "background": { "n": 2000, "seed": 1 },
 "clean": {},
 "folds": { "k": 5, "method": "blocks", "seed": 1, "size": 500000 },
 "n_background": 2000,
 "n_presences": 83,
 "predictors": { "seed": 1, "vif": 10.0 },
 "predictors_used": ["bio5", "bio7", "bio16", "bio17"],
 "seed": 1,
 "thin": { "distance": 50000, "seed": 1 }
}
```

Keep it with the model's results: with the data and Terraflux's version, it is everything needed to make the same
records again. A methods section can be written from it and the report: "Of 116 records, 3 failed
CoordinateCleaner's tests (capitals, seas, institutions), 21 duplicated a cell of 0.5° and 9 were removed by thinning
at 50 km, leaving 83. Background points (2 000) were drawn within 500 km of the records; four predictors with VIF
below 10 were kept (bio5, bio7, bio16, bio17); the records were split into five folds of spatial blocks of 500 km."

## Settings that change the answer

The same model (MaxNet, `classes = "lqh"`, `regmult = 2`) after five preparations of the same records, each adding
one choice:

```tfchart
type: bar
data: /data/tutorials/species-distributions/prepare-in-one-node/recipes.csv
x: preparation
y: [auc_test, tss, boyce, or_10p]
labels: { auc_test: "Test AUC", tss: "TSS", boyce: "Boyce", or_10p: "Omission at 10 %" }
colors: { auc_test: "#2f6f9f", tss: "#3b8a5a", boyce: "#e8901f", or_10p: "#b4508a" }
ylabel: Mean over the folds
height: 330
caption: "The scores of the same model after five preparations. Defaults: cleaning and one per cell, 5 000 background points from the whole map, five spatial blocks of the default size. Thinned at 50 km. With an accessible area of 500 km and 2 000 points. With blocks of 500 km. With random folds instead of blocks."
```

| Preparation | Presences | Test AUC | TSS | Boyce | Omission at 10 % |
|---|---|---|---|---|---|
| Defaults (background 5 000) | 92 | 0.87 | 0.66 | 0.68 | 19 % |
| Thinned at 50 km | 83 | 0.87 | 0.67 | 0.60 | 19 % |
| Area of 500 km, 2 000 points | 83 | 0.69 | 0.34 | 0.45 | 14 % |
| Blocks of 500 km | 83 | 0.71 | 0.37 | 0.55 | 11 % |
| Random folds | 83 | 0.69 | 0.34 | 0.63 | 11 % |

The **accessible area** moves the scores most: the test AUC drops from 0.87 to 0.69 when the background is drawn near
the records rather than from the whole continent. The model is not worse; its task is harder and more honest ([part
3](../predictors-and-background/)). The thinning and the folds move them less, and the folds mostly move the Boyce
index, which on 14 to 20 test presences per fold is noisy. In [part 4](../fitting-models/) the three algorithms
differed by 0.05 in test AUC; here the preparation changes it by 0.18. **Report the preparation with the scores**:
they mean nothing without it.

Other settings worth knowing:

- **`background`** larger than the cells available: with the default of 10 000 and the accessible area of 500 km,
  the node warns that only 3 230 points could be drawn and takes every cell of the area. A census of the area is
  fine for MaxNet, but the number of points (and the prevalence of part 5) is then not the one you asked for.
- **`folds = { size = "auto" }`** sizes the blocks by the predictors' range of autocorrelation. Read the variograms
  first ([part 3](../predictors-and-background/)): on a continent their ranges can run away.
- **`area = { method = "regions" }`** with polygons on `regions` (ecoregions, biomes) draws the background from the
  regions that hold records; **`within`** cuts the area to the land of a country or a continent.
- **`predictors = { vif = 10, keep = ["bio12"] }`** keeps a predictor the biology calls for whatever its VIF.
- **`clean = false`** for records that are not longitudes and latitudes (a projected grid, the template's points).

## Common pitfalls

- **Default settings taken as neutral.** Every default is a choice (no thinning, no accessible area, 10 000
  background points); the `settings` output shows them, and they belong in the methods.
- **A different seed, a different model.** Thinning, background and folds are random; keep the seed, or report the
  spread over several seeds.
- **Too many background points** for the area: read the warnings and the report's `background` row.
- **Comparing models prepared differently.** Two models are comparable only on the same `records` table.
- **Forgetting the report.** A model of 83 records from 116 is not the same as one of 83 from 2 000; say how many
  each step removed.

## Going further

- The steps one by one: [Part 2: Occurrence data](../occurrence-data/) and [Part 3: Predictors and the accessible
  area](../predictors-and-background/).
- [Part 4: Fitting models](../fitting-models/), [Part 5: Evaluation and thresholds](../evaluation-and-thresholds/),
  [Part 6: Projection and extrapolation](../projection-and-extrapolation/).
- The operators: [[sdm.prepare]], and the nodes of its steps in the [operator reference](/operators/species-distributions/).

## References

- Aiello-Lammens, M. E., Boria, R. A., Radosavljevic, A., Vilela, B., & Anderson, R. P. (2015). spThin: an R package for spatial thinning of species occurrence records for use in ecological niche models. *Ecography*, 38(5), 541–545. https://doi.org/10.1111/ecog.01132
- Araújo, M. B., Anderson, R. P., Barbosa, A. M., Beale, C. M., Dormann, C. F., Early, R., et al. (2019). Standards for distribution models in biodiversity assessments. *Science Advances*, 5(1), eaat4858. https://doi.org/10.1126/sciadv.aat4858
- Valavi, R., Elith, J., Lahoz-Monfort, J. J., & Guillera-Arroita, G. (2019). blockCV: an R package for generating spatially or environmentally separated folds for k-fold cross-validation of species distribution models. *Methods in Ecology and Evolution*, 10(2), 225–232. https://doi.org/10.1111/2041-210X.13107
- Zizka, A., Silvestro, D., Andermann, T., Azevedo, J., Duarte Ritter, C., Edler, D., et al. (2019). CoordinateCleaner: standardized cleaning of occurrence records from biological collection databases. *Methods in Ecology and Evolution*, 10(5), 744–751. https://doi.org/10.1111/2041-210X.13152
