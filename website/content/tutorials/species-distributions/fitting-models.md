---
title: "Fitting models: MaxNet, GLM and random forests"
short: Fitting models
description: Fit MaxNet, a GLM and random forests to the sloth's records over spatial folds, read their response curves and the importance of the predictors, and tune the regularisation that keeps a model from memorising its records.
order: 40
level: intermediate
time: 60 min
ops: [sdm.fit, sdm.maxnet_predict, sdm.response_curves, sdm.importance, sdm.spatial_blocks, sdm.random_folds, sdm.quadrant_folds, sdm.checkerboard_folds, sdm.spatial_clusters, sdm.prepare]
series: Species distribution models
part: 4
---

The records are clean, the predictors chosen and the background drawn: now a model can learn what separates the
places where the sloth was recorded from the places available to it. In this part you will fit three kinds of
model to the same data (MaxNet, a logistic regression and a random forest), test each one on records it has not
seen, read what each learned (response curves and the importance of the predictors), and tune the setting that
decides how closely a model may follow its records. At the end you will have a model you can defend, five
cross-validated scores to report with it, and a sense of how much the choice of algorithm matters (less than the
data, more than nothing).

We continue with the brown-throated sloth as [part 3](../predictors-and-background/) left it: the records cleaned,
one per cell of 0.5° and thinned at 50 km (83 presences), the accessible area within 500 km of them, four
predictors (bio5, bio7, bio16 and bio17) and 2 000 background points.

## The idea

### What a model of presences and background learns

Every record table of this series has a row per point: a **presence** (1, a record of the sloth) or a
**background point** (0, a place available to it), and the value of every predictor there. A species distribution
model is a function of the predictors that gives presences higher values than the background. It does not know
where the sloth is absent: the background is a sample of what was *available*, and some background points may well
hold sloths. The output is a **relative suitability**, a ranking of places, not a probability of presence (see
[part 1](../ideas/#presences-absences-and-background)).

The three algorithms of [[sdm.fit]] learn that function in different ways.

**MaxNet** is Maxent (Phillips et al., 2006), the most used method for presence-only data, as rewritten in open
source by Phillips et al. (2017). Maxent does not use the predictors directly. It builds **features** from them:
transformations that let a response bend, rise and fall, or change abruptly.

![The feature classes of MaxNet](/img/tutorials/species-distributions/fitting-models/feature-classes.png "The feature classes MaxNet builds from a predictor z (scaled 0 to 1). Linear and quadratic features give smooth trends and humps; a hinge is flat up to a knot and then rises (forward) or falls (reverse), and MaxNet places many knots along the range; a threshold jumps at a value; a product joins two predictors, so the effect of one can depend on the other.")

!!! science "Key idea: Maxent as a penalised regression"
    With \(h_j(z)\) the features of the predictors \(z\) in a cell and \(\beta_j\) their coefficients, Maxent models
    the relative rate of occurrence over the cells of the study area as

    \[ \pi(z) = \frac{\exp\big(\sum_j \beta_j h_j(z)\big)}{\sum_{i \in \text{background}} \exp\big(\sum_j \beta_j h_j(z_i)\big)} \]

    The coefficients maximise the likelihood of the presences **minus a penalty** \(\sum_j \lambda_j |\beta_j|\),
    the *lasso*. The penalty pushes most coefficients to exactly zero, so that only the features that earn their
    place stay in the model. Each \(\lambda_j\) is set by Maxent's rules from the data, and multiplied by the
    **regularisation multiplier** (`regmult`, 1 by default): a larger multiplier, a simpler model. Maxent is the
    same as a logistic regression of presences against an infinitely weighted background (Fithian & Hastie,
    2013), and as a Poisson point process (Warton & Shepherd, 2010), which is how maxnet fits it (Elith et al., 2011).

Which **feature classes** MaxNet may use is a setting (`classes`): letters among `l` (linear), `q` (quadratic),
`p` (product), `h` (hinge) and `t` (threshold). By default, as in maxnet, they depend on the number of presences:
`l` below 10, `lq` below 15, `lqh` below 80 and `lqph` from 80 up. The **output** turns the model into a value from
0 to 1: `cloglog` (the default, recommended by Phillips et al., 2017) or `logistic`.

**A GLM** (generalised linear model) here is a logistic regression: the log-odds of a presence as a straight-line
function of the predictors, standardised first, with a small ridge penalty (`C`, the inverse of its strength). It
is the classic of species distribution modelling: smooth, easy to read, and predictable when projected. As fitted
by [[sdm.fit]] it has one term per predictor, so every response is a rising or a falling S, never a hump.

**A random forest** (Breiman, 2001) grows hundreds of classification trees, each on a bootstrap sample of the
records and trying a random subset of the predictors at each split, and averages their votes. It finds
interactions and thresholds without being told, and it can **memorise** its training records: a tree grown to
the end puts every presence in a leaf of its own. With far more background than presences it also learns mostly
about the background. Valavi et al. (2021) showed that forests for presence-only data work well when the classes
are balanced. [[sdm.fit]] weights the records by default (`weights = "balanced"`: every presence 1, every
background point the number of presences over the number of background points) for the GLM and the forest.

### Testing on records the model has not seen

A model always fits its own records well. The question is whether it predicts **other** places. **k-fold
cross-validation** splits the records into k groups (folds); each fold in turn is set aside, the model is fitted
to the others and scored on it. [[sdm.fit]] does this for every fold of the `fold` column, then fits the model once
more on every record (the model it returns).

How the folds are made matters. Records close together share their climate, so a test record next to a training
record is almost a copy of it, and **random folds** flatter the model. **Spatial blocks** (Valavi et al., 2019;
Roberts et al., 2017) keep the test records away from the training ones: the records fall in blocks of a size
larger than the range of autocorrelation, and whole blocks go to a fold.

```tfgallery
columns: 3
caption: Five ways of making folds of the sloth's records (presences large, background small, a colour per fold). Spatial blocks of 500 km (sdm.spatial_blocks), random folds (sdm.random_folds), the four quadrants of the presences (sdm.quadrant_folds, ENMeval's block method), a checkerboard of the grid aggregated by 10 (sdm.checkerboard_folds, two folds) and k-means clusters of the coordinates (sdm.spatial_clusters).
images:
  - { src: /img/tutorials/species-distributions/fitting-models/folds_blocks.png, caption: "Spatial blocks" }
  - { src: /img/tutorials/species-distributions/fitting-models/folds_random.png, caption: "Random" }
  - { src: /img/tutorials/species-distributions/fitting-models/folds_quadrants.png, caption: "Quadrants" }
  - { src: /img/tutorials/species-distributions/fitting-models/folds_checkerboard.png, caption: "Checkerboard" }
  - { src: /img/tutorials/species-distributions/fitting-models/folds_clusters.png, caption: "Clusters" }
```

The scores computed on each test fold are those of [[sdm.fit]]'s `metrics`: the AUC on the training and the test
records and their difference, the largest TSS, the Boyce index and two omission rates. [Part
5](../evaluation-and-thresholds/) explains each in detail. For now: the **test AUC** is the chance that a test
presence scores above a background point (0.5 is chance); **TSS** and **Boyce** say how well the suitability
separates and orders the records; the **omission rates** are the share of test presences that fall below a
threshold set on the training presences, and the **AUC difference** (train minus test) grows when a model
overfits (Muscarella et al., 2014; Radosavljevic & Anderson, 2014).

## When to use it

- **MaxNet** is the default for presence-only records: it was designed for them, it has been benchmarked more
  than any other method, and with tuned regularisation it is among the best (Valavi et al., 2022).
- **A GLM** when interpretation matters more than the last point of AUC, when records are few, and when the model
  will be projected far from its data: its straight-line responses extrapolate in a predictable (if not always
  realistic) way.
- **A random forest** when you suspect interactions or sharp thresholds, and have enough records (a few hundred)
  to let it find them without memorising.
- **Several**, and compare. Valavi et al. (2022) found large differences between species in which method did best;
  an algorithm that disagrees with the others is worth a look at its response curves.

Fitting a model is not a substitute for presence-absence surveys when you need a probability of occurrence, nor
for a mechanistic model when you need to know *why* the species is limited. And no algorithm repairs biased
records or a background drawn from the wrong area ([part 2](../occurrence-data/), [part
3](../predictors-and-background/)).

## What you need

- A **records table** with the columns `presence` (1 or 0), the predictors and `fold`: the `records` output of
  [[sdm.prepare]] or of [[sdm.dataset]] (with a folds node after it). The columns `x`, `y`, `presence`, `fold`,
  `weight` and `cell` are never used as predictors; every other column is, unless `predictors` lists them.
- The **predictors** as a stack, to map the model: the `env` output of [[sdm.prepare]], the predictors it kept.
- The data of the series: [bradypus.csv](/data/tutorials/species-distributions/ideas/bradypus.csv) and
  [bioclim.tif](/data/tutorials/species-distributions/ideas/bioclim.tif).

## Build the model

### Prepare, fit, predict

The preparation of parts 2 and 3 fits in one [[sdm.prepare]] node ([part 7](../prepare-in-one-node/) explains all
its settings): records cleaned and one per cell, thinned at 50 km, an accessible area of 500 km, the predictors
below a VIF of 10, 2 000 background points and five folds of spatial blocks of 500 km.

```tfgraph
#! caption: The sloth's records prepared (node 3), MaxNet fitted over five folds of spatial blocks (node 4), mapped (node 5), and what it learned: its response curves (node 6) and the importance of each predictor (node 7).
#! numbered: true
#! highlight: fit, predict, curves, importance
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
params = { thin = 50000, area = 500000, background = 2000, folds = { size = 500000, k = 5 }, seed = 1 }

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
params = { model = "maxnet", regmult = 1 }

[[nodes]]
id = "predict"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

[[nodes]]
id = "curves"
op = "sdm.response_curves"
inputs = { model = "fit.model", records = "data.records" }

[[nodes]]
id = "importance"
op = "sdm.importance"
inputs = { model = "fit.model", records = "data.records" }
```

<div class="steps" markdown>

1. **Read and prepare** (nodes 1 to 3), as in the previous parts. The `records` output of node 3 is the table
   the model learns from (83 presences, 2 000 background points, a fold each); its `env` output is the stack of
   the four predictors kept.
2. **Fit** (node 4): add [[sdm.fit]] from the library's *Species distributions* subject and drag `data.records`
   to its `records` input. In the Node pane, `model` chooses the algorithm (`maxnet`, `glm` or `random_forest`);
   the other settings belong to one algorithm or another (below). Its outputs: `model` (the model fitted on every
   record), `metrics` (a row per fold), `summary` (the mean and standard deviation of each score over the folds)
   and `threshold` (the suitability that maximises sensitivity plus specificity on all the records).
3. **Map** (node 5): [[sdm.maxnet_predict]] applies any fitted model (despite its name, also a GLM or a forest) to
   a stack of the predictors, found by name: connect `fit.model` to `model` and `data.env` to `env`. For a projection
   to another climate, connect a stack of the same predictors under the same names ([part
   6](../projection-and-extrapolation/)).
4. **Explain** (nodes 6 and 7): [[sdm.response_curves]] and [[sdm.importance]] both take the model and the records.
   Add [[view.plot]] nodes on `predict.prediction` to see the map in a tab of its own.

</div>

!!! app "In the application"
    Start from **Templates → Species → Species distribution model**: it has the prepare, fit and predict nodes on
    predictors drawn by chance. Replace the predictors and the presences with [[raster.read]] and [[vector.read]]
    nodes of your own data. Select the node `fit` and press ++ctrl+enter++: the `summary` table appears under the
    canvas; click the `metrics` port to see every fold. **A species in a warmer climate**, in the same menu, adds
    the response curves and the importance.

From the command line, add `[outputs.summary]` with `from = "fit.summary"` (and the same for `fit.metrics`,
`curves.curves` and `importance.importance`) and run `terraflux run sloth-fit.toml --out results`: each table is
written as a CSV file.

### The other algorithms

The same node, other settings. The **GLM**:

```toml
[[nodes]]
id = "glm"
op = "sdm.fit"
inputs = { records = "data.records" }
params = { model = "glm", C = 1 }
```

`C` is the inverse of the strength of the ridge penalty (1 by default; smaller, a flatter model); `standardize`
(true by default) scales the predictors first so that the penalty treats them alike.

A **random forest**:

```toml
[[nodes]]
id = "forest"
op = "sdm.fit"
inputs = { records = "data.records" }
params = { model = "random_forest", n_estimators = 500, max_depth = 3 }
```

`n_estimators` is the number of trees (100 by default); `min_samples_leaf`, `max_depth` and `min_samples_split`
limit how finely each tree may split the records; `max_features` how many predictors a split tries (the square root
of their number by default); `random_state` the seed. The forest is scikit-learn's, tree for tree.

### Other folds

[[sdm.prepare]] makes spatial blocks by default. To fit the same records over other folds, put a folds node
between the preparation and the fit: it returns the records with a new `fold` column.

```tfgraph
#! caption: The same records over random folds (node 2) and k-means clusters of their coordinates (node 4), each fitted with MaxNet.
#! fragment: true
#! numbered: true
#! highlight: random, clusters
[[nodes]]
id = "random"
op = "sdm.random_folds"
inputs = { records = "data.records" }
params = { k = 5, seed = 1 }

[[nodes]]
id = "fit_random"
op = "sdm.fit"
inputs = { records = "random.records" }

[[nodes]]
id = "clusters"
op = "sdm.spatial_clusters"
inputs = { records = "data.records" }
params = { k = 5, seed = 1 }

[[nodes]]
id = "fit_clusters"
op = "sdm.fit"
inputs = { records = "clusters.records" }
```

[[sdm.spatial_blocks]] (`size` in metres on longitude and latitude records, `k`, hexagons or squares),
[[sdm.quadrant_folds]], [[sdm.checkerboard_folds]] (which also needs the predictors on `env`, for their grid) and
[[sdm.jackknife_folds]] (a fold per presence, for very few records) work the same way.

## Read the results

### The scores over the folds

The `metrics` of the MaxNet model, a row per test fold:

| Fold | Test presences | Training AUC | Test AUC | TSS | Boyce | Omission (10th pct.) |
|---|---|---|---|---|---|---|
| 1 | 20 | 0.73 | 0.69 | 0.32 | 0.64 | 15 % |
| 2 | 17 | 0.72 | 0.77 | 0.44 | 0.78 | 6 % |
| 3 | 16 | 0.76 | 0.58 | 0.22 | 0.23 | 19 % |
| 4 | 14 | 0.72 | 0.79 | 0.51 | 0.75 | 0 % |
| 5 | 16 | 0.77 | 0.61 | 0.25 | 0.18 | 31 % |
| **Mean ± sd** | | **0.74 ± 0.02** | **0.69 ± 0.09** | **0.35 ± 0.12** | **0.52 ± 0.29** | **14 ± 12 %** |

Two folds (3 and 5) are predicted much worse than the others: the model fitted without their blocks does not
know their conditions well. This spread is information, not noise to hide: report the standard deviation with the
mean. A test AUC of 0.69 looks modest beside the 0.85 of the first model of [part 1](../ideas/), but the task is
harder: the background now comes from places the sloth could reach, most of them just as tropical (part 3 showed
the effect).

The omission rate at the 10th percentile should be near 10 % for a model that generalises; 14 % is close. The
`threshold` output (0.61 here) is the suitability that best separates presences from background on all the
records: [part 5](../evaluation-and-thresholds/) shows how to choose one.

### The three algorithms

```tfgallery
columns: 3
caption: The sloth's suitability from MaxNet (feature classes by default, regmult 1), a GLM and a random forest of trees of depth 3, all fitted to the same records over the same folds. The three agree on the humid lowlands of the Amazon, the Chocó and Central America; the forest is blockier and the GLM smoother; MaxNet and the forest give more to the coast of the Guianas.
images:
  - { src: /img/tutorials/species-distributions/fitting-models/model_0.png, caption: "MaxNet" }
  - { src: /img/tutorials/species-distributions/fitting-models/model_1.png, caption: "GLM" }
  - { src: /img/tutorials/species-distributions/fitting-models/model_2.png, caption: "Random forest" }
```

```tfchart
type: bar
data: /data/tutorials/species-distributions/fitting-models/models.csv
x: model
y: [auc_train, auc_test, tss, boyce]
labels: { auc_train: "Training AUC", auc_test: "Test AUC", tss: "TSS", boyce: "Boyce" }
colors: { auc_train: "#b7c4bc", auc_test: "#2f6f9f", tss: "#3b8a5a", boyce: "#e8901f" }
ylabel: Mean over the five folds
height: 340
caption: "MaxNet with five sets of feature classes, the GLM and three random forests, on the same folds. The test scores are close (AUC 0.65 to 0.71), the training AUCs are not: the forest with default settings scores 1.0 on its training records and only 0.65 on the test ones, and it puts every training presence at the top, so that its omission rates are 100 %. Shallower trees (depth 3) or leaves of at least 20 records bring it back."
```

The forest with default settings is the textbook case of **overfitting**: trees grown to the end memorise every
training presence (training AUC 1.0), its test AUC is the lowest of all, and the threshold set on its training
presences (their minimum, 1.0) is one no test presence reaches. Limiting the trees fixes it: with leaves of 20 records the test AUC is 0.71, with
a depth of 3 it is 0.68 and the omission falls further. The GLM, the simplest model, does as well as any (test AUC
0.71, Boyce 0.62). With 83 presences and four predictors there is not much more to learn than smooth trends.

### What the models learned

![Response curves of the three models](/img/tutorials/species-distributions/fitting-models/curves.png "The response curves of MaxNet, the GLM and the random forest (depth 3): the suitability as each predictor varies over its range in the records, the other three held at their mean. The red ticks are the presences. All three agree that suitability falls with the annual range of temperature (bio7) and rises with the rain of the wettest quarter (bio16); MaxNet's curves bend where its hinges have knots, the forest's are steps.")

A **response curve** shows the suitability as one predictor varies and the others stay at their mean (`fixed =
"median"` holds them at the median). Read them as the model's partial view of the niche:

- **bio7, the annual range of temperature**, matters most: suitability is high where temperature varies by about
  10 °C over the year or less (the humid tropics) and falls to nothing beyond 20 °C. This is ecologically sound: a
  sloth cannot regulate its body temperature well.
- **bio16, the rain of the wettest quarter**, raises suitability up to about 1 500 mm.
- **bio5 and bio17** change little. MaxNet's upturn at the right end of bio17 rests on two records: a curve where
  there are no records is the model's guess (look at the ticks).

```tfchart
type: bar
data: /data/tutorials/species-distributions/fitting-models/importance.csv
x: predictor
y: [MaxNet, GLM, Random forest]
colors: { MaxNet: "#2f6f9f", GLM: "#3b8a5a", "Random forest": "#e8901f" }
ylabel: Drop of the AUC when shuffled
height: 260
caption: "Permutation importance (sdm.importance): how much the AUC over the records falls when one predictor is shuffled (5 times, the mean). bio7 dominates in all three models; the forest spreads its importance more evenly."
```

[[sdm.importance]] shuffles each predictor in turn (`n_repeats` times, 5 by default, with a `seed`) and measures how
much the AUC falls: a predictor the model relies on breaks it when shuffled. Its `predictors` output lists them, the
most important first. With collinear predictors, the importance is shared between them in arbitrary ways, one more
reason for the selection of [part 3](../predictors-and-background/).

### Reporting a model

In the methods: the algorithm and its settings (for MaxNet the feature classes and the regularisation multiplier;
for a forest the trees, leaf size or depth; for a GLM its terms and penalty), how the records were weighted, the
folds (design, number and size), and the output (cloglog). In the results: the mean and standard deviation of the
test AUC, TSS, Boyce and the omission rates over the folds (the `summary` table), the response curves of the main
predictors, their importance, and the map. ODMAP, a standard protocol for reporting distribution models (Zurell et al.,
2020), lists the rest.

## Settings that change the answer

### The regularisation multiplier

The multiplier scales MaxNet's penalty on its coefficients. Too low, and the model follows every record (and every
quirk of the sampling); too high, and it flattens into a trend. The slider shows the same records with multipliers
from 0.25 to 8.

```tfslider
label: Regularisation multiplier (regmult)
caption: "MaxNet's suitability with growing regularisation (the accessible area outlined in white, the presences in red). With 0.25 the model keeps 32 features, its map is patchy and its training AUC (0.79) far above its test AUC (0.68); with 4 it keeps 5 features, the map is smooth, and the test scores are the best of the series. The chart: the test AUC, the Boyce index and the difference between training and test AUC over the folds."
frames:
  - { value: 0.25, src: /img/tutorials/species-distributions/fitting-models/regmult_0_25.png, stats: { "Features": "32", "Test AUC": "0.675", "AUC train - test": "0.141", "Boyce": "0.43", "Omission 10 %": "27 %" } }
  - { value: 0.5, src: /img/tutorials/species-distributions/fitting-models/regmult_0_5.png, stats: { "Features": "23", "Test AUC": "0.680", "AUC train - test": "0.119", "Boyce": "0.45", "Omission 10 %": "19 %" } }
  - { value: 1, src: /img/tutorials/species-distributions/fitting-models/regmult_1.png, stats: { "Features": "16", "Test AUC": "0.685", "AUC train - test": "0.099", "Boyce": "0.52", "Omission 10 %": "14 %" } }
  - { value: 2, src: /img/tutorials/species-distributions/fitting-models/regmult_2.png, stats: { "Features": "9", "Test AUC": "0.705", "AUC train - test": "0.085", "Boyce": "0.55", "Omission 10 %": "11 %" } }
  - { value: 4, src: /img/tutorials/species-distributions/fitting-models/regmult_4.png, stats: { "Features": "5", "Test AUC": "0.714", "AUC train - test": "0.092", "Boyce": "0.58", "Omission 10 %": "12 %" } }
  - { value: 8, src: /img/tutorials/species-distributions/fitting-models/regmult_8.png, stats: { "Features": "4", "Test AUC": "0.710", "AUC train - test": "0.096", "Boyce": "0.47", "Omission 10 %": "10 %" } }
start: 2
chart:
  type: line
  data: /data/tutorials/species-distributions/fitting-models/regmult.csv
  x: regmult
  y: [auc_test, boyce, auc_diff]
  labels: { auc_test: "Test AUC", boyce: "Boyce", auc_diff: "AUC train - test" }
  colors: { auc_test: "#2f6f9f", boyce: "#e8901f", auc_diff: "#b4508a" }
  xlabel: regmult
  xlog: true
  ylabel: Mean over the folds
```

The numbers tell a consistent story. As the multiplier rises from 0.25 to 4, the gap between training and test AUC
shrinks, the test AUC and the Boyce index rise and the omission rate falls towards 10 %: the model stops fitting
noise. At 8 the model has only 4 features left and the Boyce index drops: it has become too simple to order the
places well. A multiplier of 2 to 4 is the best compromise for these data, above Maxent's default of 1, as is common
with few records (Radosavljevic & Anderson, 2014).

This is what ENMeval does (Muscarella et al., 2014): fit a grid of multipliers and feature classes over spatial
folds, and choose the simplest model with low omission and a small AUC difference, or the best by AICc. In
Terraflux, put several [[sdm.fit]] nodes side by side, or a [[control.repeat]] loop over `regmult`, and compare
their `summary` tables. Choose on the test scores, never on the map you like best.

### The feature classes

| Classes | Training AUC | Test AUC | AUC difference | Boyce | Omission 10 % |
|---|---|---|---|---|---|
| `l` | 0.72 | 0.71 | 0.09 | 0.60 | 13 % |
| `lq` | 0.72 | 0.71 | 0.09 | 0.58 | 12 % |
| `lqh` | 0.74 | 0.69 | 0.10 | 0.52 | 14 % |
| `lqph` | 0.74 | 0.69 | 0.10 | 0.52 | 15 % |
| `lqpht` | 0.75 | 0.69 | 0.11 | 0.50 | 16 % |

More classes give a closer fit to the training records and slightly worse test scores: with these data, linear and
quadratic features are enough. Hinges earn their place with many records and responses with sharp turns.

!!! warning "The default classes of the folds and of the final model can differ"
    The default classes are maxnet's, chosen by the presences of each fit. Each fold of the cross-validation is
    fitted on about 66 presences (`lqh`), while the model returned is fitted on all 83 (`lqph`). The scores of
    `summary` are then those of a slightly different model from the one you map. This is maxnet's behaviour, kept so
    that the results match landschaft's; the operator's description says so too. Set `classes` yourself (for
    instance `classes = "lqh"`) so that the folds and the final model match.

### The folds

| Folds | Number | Test AUC | TSS | Boyce | Omission 10 % |
|---|---|---|---|---|---|
| Spatial blocks of 500 km | 5 | 0.685 | 0.35 | 0.52 | 14 % |
| Random | 5 | 0.687 | 0.34 | 0.56 | 16 % |
| Quadrants | 4 | 0.662 | 0.32 | 0.58 | 17 % |
| Checkerboard (×10) | 2 | 0.680 | 0.30 | 0.63 | 13 % |
| k-means clusters | 5 | 0.691 | 0.46 | 0.44 | 32 % |

Here random folds score about as well as spatial blocks. The thinning at 50 km and the coarse grid have already
removed most of the near-duplicates that make random folds optimistic; on fine grids with clustered records the
difference is often large (Roberts et al., 2017; Valavi et al., 2019). Clusters give the most extreme folds: each
holds a whole region (Central America, the Atlantic forest), and omits a third of its presences, a test of
**transfer** to regions the model has not seen. Choose the design for the use of the model: blocks for mapping
within the region, clusters or quadrants when the model will be projected elsewhere.

## Common pitfalls

- **Judging a model by its training AUC.** The model always fits its own records; only the test scores mean
  anything, and a large difference between the two is the sign of overfitting.
- **A random forest with default settings.** It memorises presence-only data. Limit the trees (`min_samples_leaf`,
  `max_depth`) and keep the records balanced (`weights = "balanced"`, the default).
- **Tuning on the map.** Choose settings by their cross-validated scores, then look at the map, not the other way
  round.
- **Comparing scores across different backgrounds or folds.** Fit the models you compare on the same records
  table; [part 3](../predictors-and-background/) showed how much the background moves the AUC.
- **Response curves where there are no records.** Read them together with the distribution of the records (the
  ticks of the figure); the ends of a curve are often extrapolations.
- **Reading cloglog suitability as probability.** It is a relative index of suitability; only presence-absence
  data can calibrate a probability.
- **Extra columns in a records table.** [[sdm.fit]], and the lower-level [[sdm.maxnet]] given a records table on its
  `data`, leave out the columns that are not predictors (`x`, `y`, `fold`, `weight`, `cell`...) and use every other
  column. A column of your own in a table you bring (a site's code, a year) would become a predictor: keep only the
  predictors and those columns. [[sdm.maxnet]] fits once, on every record; [[sdm.fit]] adds the cross-validation.

## Going further

- [Part 5: Evaluation and thresholds](../evaluation-and-thresholds/): AUC, TSS, Boyce and omission in detail, and
  how to turn suitability into presence and absence.
- [Part 6: Projection and extrapolation](../projection-and-extrapolation/): the model in another climate, and where
  not to trust it.
- [Part 7: The whole preparation in one node](../prepare-in-one-node/).
- Suitability maps feed conservation plans: [Systematic conservation planning](/tutorials/spatial-prioritization/systematic-conservation-planning/).
- The operators: [[sdm.fit]], [[sdm.maxnet]], [[sdm.maxnet_predict]], [[sdm.response_curves]], [[sdm.importance]].

## References

- Breiman, L. (2001). Random forests. *Machine Learning*, 45(1), 5–32. https://doi.org/10.1023/A:1010933404324
- Elith, J., Phillips, S. J., Hastie, T., Dudík, M., Chee, Y. E., & Yates, C. J. (2011). A statistical explanation of MaxEnt for ecologists. *Diversity and Distributions*, 17(1), 43–57. https://doi.org/10.1111/j.1472-4642.2010.00725.x
- Fithian, W., & Hastie, T. (2013). Finite-sample equivalence in statistical models for presence-only data. *The Annals of Applied Statistics*, 7(4), 1917–1939. https://doi.org/10.1214/13-AOAS667
- Muscarella, R., Galante, P. J., Soley-Guardia, M., Boria, R. A., Kass, J. M., Uriarte, M., & Anderson, R. P. (2014). ENMeval: an R package for conducting spatially independent evaluations and estimating optimal model complexity for Maxent ecological niche models. *Methods in Ecology and Evolution*, 5(11), 1198–1205. https://doi.org/10.1111/2041-210X.12261
- Phillips, S. J., Anderson, R. P., Dudík, M., Schapire, R. E., & Blair, M. E. (2017). Opening the black box: an open-source release of Maxent. *Ecography*, 40(7), 887–893. https://doi.org/10.1111/ecog.03049
- Phillips, S. J., Anderson, R. P., & Schapire, R. E. (2006). Maximum entropy modeling of species geographic distributions. *Ecological Modelling*, 190(3–4), 231–259. https://doi.org/10.1016/j.ecolmodel.2005.03.026
- Radosavljevic, A., & Anderson, R. P. (2014). Making better Maxent models of species distributions: complexity, overfitting and evaluation. *Journal of Biogeography*, 41(4), 629–643. https://doi.org/10.1111/jbi.12227
- Roberts, D. R., Bahn, V., Ciuti, S., Boyce, M. S., Elith, J., Guillera-Arroita, G., et al. (2017). Cross-validation strategies for data with temporal, spatial, hierarchical, or phylogenetic structure. *Ecography*, 40(8), 913–929. https://doi.org/10.1111/ecog.02881
- Valavi, R., Elith, J., Lahoz-Monfort, J. J., & Guillera-Arroita, G. (2019). blockCV: an R package for generating spatially or environmentally separated folds for k-fold cross-validation of species distribution models. *Methods in Ecology and Evolution*, 10(2), 225–232. https://doi.org/10.1111/2041-210X.13107
- Valavi, R., Elith, J., Lahoz-Monfort, J. J., & Guillera-Arroita, G. (2021). Modelling species presence-only data with random forests. *Ecography*, 44(12), 1731–1742. https://doi.org/10.1111/ecog.05615
- Valavi, R., Guillera-Arroita, G., Lahoz-Monfort, J. J., & Elith, J. (2022). Predictive performance of presence-only species distribution models: a benchmark study with reproducible code. *Ecological Monographs*, 92(1), e01486. https://doi.org/10.1002/ecm.1486
- Warton, D. I., & Shepherd, L. C. (2010). Poisson point process models solve the "pseudo-absence problem" for presence-only data in ecology. *The Annals of Applied Statistics*, 4(3), 1383–1402. https://doi.org/10.1214/10-AOAS331
- Zurell, D., Franklin, J., König, C., Bouchet, P. J., Dormann, C. F., Elith, J., et al. (2020). A standard protocol for reporting species distribution models. *Ecography*, 43(9), 1261–1277. https://doi.org/10.1111/ecog.04960
