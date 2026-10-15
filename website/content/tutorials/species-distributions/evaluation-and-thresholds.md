---
title: Evaluation and thresholds
short: Evaluation and thresholds
description: What AUC, TSS, the Boyce index and omission rates measure, how to compute them for any model in Terraflux, and how to turn a suitability map into presence and absence with a threshold you can justify.
order: 50
level: intermediate
time: 50 min
ops: [sdm.evaluate, sdm.threshold, sdm.boyce, sdm.omission_rate, sdm.dataset, sdm.maxnet_predict, map.calc]
series: Species distribution models
part: 5
---

A model gives every place a suitability; before anyone uses the map, two questions need answers. How well does it
separate the places where the species is from the others? And where, if a yes-or-no map is needed, should the line
between suitable and unsuitable be drawn? In this part you will compute the usual scores for the sloth's model by
hand in a graph (AUC, TSS, the Boyce index and omission rates), see what each measures and what it hides, and turn
the suitability into presence and absence with eight different thresholds, which give suitable areas from 6 % to
96 % of the accessible area.

We use the model [part 4](../fitting-models/) arrived at: MaxNet with linear, quadratic and hinge features
(`classes = "lqh"`) and a regularisation multiplier of 2, fitted to the 83 presences and 2 000 background points
of the sloth inside its accessible area of 500 km.

## The idea

### Two kinds of error

A threshold \(t\) splits the suitability into "predicted present" (\(\geq t\)) and "predicted absent". Each record
then falls in one of four boxes:

| | Predicted present | Predicted absent |
|---|---|---|
| **Presence** | true positive (*a*) | false negative (*c*): an omission |
| **Background point** | false positive (*b*) | true negative (*d*) |

The **sensitivity** is the share of presences predicted present, \(a / (a + c)\); the **specificity** the share of
background points predicted absent, \(d / (b + d)\). Every threshold trades one for the other: a low threshold
catches every presence and calls most of the map suitable; a high one calls little suitable and misses presences.

With presence-only data, a "false positive" is a background point predicted suitable, and the species may well live
there: the background is a sample of what is available, not of absences. Specificity is then the share of the
available area predicted unsuitable. This is why the scores of presence-background models depend on the background
(part 3), and why none of them can reach its maximum.

### AUC: ranking presences above the background

The **receiver operating characteristic** (ROC) curve plots the sensitivity against 1 − specificity for every
threshold. The **area under the curve** (AUC) is the probability that a random presence gets a higher suitability
than a random background point (Hanley & McNeil, 1982): 1 for a perfect ranking, 0.5 for a random one. It needs no
threshold, which made it the most reported score in species distribution modelling (Fielding & Bell, 1997).

Its limits are well known (Lobo et al., 2008; Jiménez-Valverde, 2012). It grows with the extent of the background
(the farther the background reaches from the species, the easier the ranking); it weighs parts of the curve no one
uses; and with background points the best achievable AUC is below 1 and unknown. Compare AUCs only between models
fitted and tested on the same records.

### TSS: the best threshold's skill

The **true skill statistic** (Allouche, Tsoar & Kadmon, 2006) is sensitivity + specificity − 1 at a threshold: 0 for
a model no better than chance, 1 for a perfect one. Unlike Cohen's kappa, it does not depend on prevalence (the share
of presences among the records), which in a presence-background data set is set by how many background points you
drew. [[sdm.fit]] and [[sdm.evaluate]] report the largest TSS over all thresholds.

### The Boyce index: are high values where the presences are?

The **continuous Boyce index** (Boyce et al., 2002; Hirzel et al., 2006) was designed for presence-only data. It asks
whether the presences are more frequent than expected in places of high suitability, and less in places of low.

!!! science "Key idea: the predicted-to-expected ratio"
    Slide a window along the range of suitability. In each window \(i\), let \(P_i\) be the share of the presences
    whose suitability falls in it and \(E_i\) the share of the background (the available area) that does. Their ratio

    \[ F_i = \frac{P_i}{E_i} \]

    is above 1 where the model puts more presences than the area alone would, below 1 where fewer. A good model's
    \(F\) rises steadily with suitability. The **Boyce index** is the Spearman rank correlation between \(F_i\) and the
    suitability of the windows: near 1 for a model that orders the places well, 0 for one no better than chance,
    negative for one that orders them backwards.

The Boyce index uses only the presences and the available area, so it suits presence-only data, and the shape of the
\(F\) curve tells more than the number: a curve that rises only at the top says the model separates the best places
and not the rest.

### Omission rates: did the model memorise its records?

Set a threshold on the **training** presences and count the **test** presences below it (Radosavljevic & Anderson,
2014; Muscarella et al., 2014). With the lowest training suitability (the *minimum training presence*, MTP), a model
that generalises omits almost no test presences; with the 10th percentile of the training presences, about 10 %.
Much more means the model fits its training records more tightly than new ones. [[sdm.fit]] reports both per fold
(`or_mtp`, `or_10p`), and [[sdm.omission_rate]] computes them from any two sets of predictions.

### Thresholds

Many uses need a map of presence and absence: a range size, an overlap with protected areas, a habitat map for a
connectivity analysis. A **threshold** draws the line. Liu et al. (2005) compared twelve ways of choosing one and found
those that balance sensitivity and specificity among the best; for presence-only data, maximising sensitivity plus
specificity (the TSS) remains a sound default (Liu, White & Newell, 2013). [[sdm.threshold]] computes them all:

| Method | The threshold |
|---|---|
| `max_spec_sens` | maximises sensitivity + specificity (the largest TSS); the default |
| `equal_sens_spec` | where sensitivity equals specificity |
| `max_kappa` | maximises Cohen's kappa |
| `no_omission` | the highest that keeps every presence |
| `mtp` | the minimum training presence: the lowest suitability of a presence |
| `p10` | the 10th percentile of the presences: 90 % of them above it |
| `sensitivity` | the highest that keeps a given sensitivity (`sensitivity`, 0.9 by default) |
| `equal_prevalence` | the predicted prevalence equals the prevalence of the records |

## When to use it

- **Every model needs an evaluation**, on records it was not fitted to: the folds of [[sdm.fit]] (part 4), or
  independent data (a later survey, another region) through the nodes of this part.
- **Compare models** fitted to the same records: algorithms, settings, predictors. Use several scores at once.
- **Thresholds** only when the use needs a binary map. Keep the continuous suitability wherever the analysis can
  take it: prioritization ([Zonation](/tutorials/spatial-prioritization/zonation/) and [prioritizr](/tutorials/spatial-prioritization/prioritizr/)
  take amounts), connectivity (resistance from suitability), and any comparison of maps.

What the scores are *not*: proof that the map is right. A model with a fine AUC can be biased in the same way as its
records (records along roads, background along roads); only independent, well-designed data test that. And a
score of a projected model (another time, another region) says nothing about the projection ([part
6](../projection-and-extrapolation/)).

## What you need

- A fitted model ([[sdm.fit]] or [[sdm.maxnet]]) and the table of records it was fitted to.
- The **predictions at the presences and at the background** as two tables, or as maps. [[sdm.evaluate]],
  [[sdm.threshold]], [[sdm.boyce]] and [[sdm.omission_rate]] take any predictions, so they evaluate models from any
  software: give them a CSV of predictions read with [[vector.read]] or two maps.

## Build the model

### Predictions at the presences and at the background

The model of part 4 gives a map; the evaluation needs the predictions at the records. [[sdm.prepare]] passes on the
presences it kept (`presences`) and the background points (`background_points`); [[sdm.dataset]] reads the
predictors at each, and [[sdm.maxnet_predict]], given a table instead of a stack, predicts every row.

```tfgraph
#! caption: The predictors read at the presences (node 3) and at the background points (node 4), the model's predictions at each (nodes 5 and 6), and their evaluation (node 7). Nodes 1 and 2 are the preparation and the fit of part 4.
#! fragment: true
#! numbered: true
#! highlight: at_presences, at_background, p_pres, p_back, evaluation
[[nodes]]
id = "data"
op = "sdm.prepare"
inputs = { occurrences = "records.vector", env = "climate.stack" }
params = { thin = 50000, area = 500000, background = 2000, folds = { size = 500000, k = 5 }, seed = 1 }

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
params = { regmult = 2, classes = "lqh" }

[[nodes]]
id = "at_presences"
op = "sdm.dataset"
inputs = { presences = "data.presences", env = "data.env" }

[[nodes]]
id = "at_background"
op = "sdm.dataset"
inputs = { presences = "data.background_points", env = "data.env" }

[[nodes]]
id = "p_pres"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "at_presences.records" }

[[nodes]]
id = "p_back"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "at_background.records" }

[[nodes]]
id = "evaluation"
op = "sdm.evaluate"
inputs = { presences = "p_pres.prediction", background = "p_back.prediction" }
```

<div class="steps" markdown>

1. **The predictors at the presences** (node 3): [[sdm.dataset]] with `data.presences` on its `presences` input and
   the predictors kept (`data.env`) on `env`. With no background, its `records` table has a row per presence: `x`,
   `y`, `presence` and the four predictors.
2. **At the background** (node 4): the same node with the background points on `presences` (it marks them 1, which
   does not matter here: only the predictors are used).
3. **The predictions** (nodes 5 and 6): [[sdm.maxnet_predict]] with the model on `model` and a table on `env` returns
   a table with one column, `prediction`, a row per record.
4. **The evaluation** (node 7): [[sdm.evaluate]] with the presences' predictions on `presences` and the
   background's on `background`. Its outputs: `stats` (AUC, the point-biserial correlation and its p-value, the
   numbers of presences and background points, the prevalence, the largest TSS and the Boyce index), `table` (the
   confusion table at every threshold: the four counts, kappa, sensitivity `tpr`, specificity `tnr` and TSS among
   others), `thresholds` (the threshold of each method) and `boyce` (the windows: `hs`, their suitability, and
   `f_ratio`, their \(F\)).

</div>

For **independent test data**, give node 3 a table of the test records (points read with [[vector.read]]) and the
background of the test region to node 4: the same nodes give the test scores.

### Thresholds and the binary map

```tfgraph
#! caption: A threshold (node 1) and the map it makes (node 3): suitability at or above it is 1, the rest 0.
#! fragment: true
#! numbered: true
#! highlight: threshold, binary
[[nodes]]
id = "threshold"
op = "sdm.threshold"
inputs = { presences = "p_pres.prediction", background = "p_back.prediction" }
params = { method = "max_spec_sens" }

[[nodes]]
id = "predict"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

[[nodes]]
id = "binary"
op = "map.calc"
inputs = { s = "predict.prediction", t = "threshold.threshold" }
params = { expression = "if s >= t then 1 else 0" }
```

[[sdm.threshold]] returns a number on its `threshold` port. [[map.calc]] takes numbers as inputs as well as maps:
connect the suitability map to `s` and the threshold to `t`, and the expression `if s >= t then 1 else 0` makes the
binary map. [[sdm.fit]] also has a `threshold` output: the `max_spec_sens` threshold on all its records.

!!! app "In the application"
    Add the nodes from the *Species distributions* subject of the library, connect them as in the graphs and press
    ++ctrl+enter++ on `evaluation`: its `stats` table appears under the canvas; click the `table` port to see the
    confusion table, which you can sort by `tss`. Selecting `threshold` shows the number; `binary` the map.

## Read the results

### The scores

The `stats` of [[sdm.evaluate]], on all the records the model was fitted to, beside the cross-validated scores of
[[sdm.fit]]'s `summary`:

| Score | All records (node 7) | Mean over the 5 test folds |
|---|---|---|
| AUC | 0.72 | 0.70 ± 0.09 |
| Largest TSS | 0.31 | 0.37 ± 0.11 |
| Boyce | 0.92 | 0.55 ± 0.31 |
| Omission at MTP | | 4 % |
| Omission at 10th pct. | | 11 % |

The scores on all records are **training** scores: the model has seen these records. They are what the thresholds
are set on, but the fold scores are the ones to report as the model's performance. Two things stand out. The Boyce
index is high on all the records (0.92) and noisy over the folds (0.55, with a standard deviation of 0.31): each test
fold holds only 14 to 20 presences, too few for a smooth \(F\) curve. And the AUC is modest, because the background
is close to the records (part 3); the omission rates near 0 and 10 % say the model is not overfitted.

### The ROC curve and the trade-off of thresholds

```tfchart
type: line
data: /data/tutorials/species-distributions/evaluation-and-thresholds/roc.csv
x: fpr
y: [Model, Chance]
colors: { Model: "#2f6f9f", Chance: "#b7c4bc" }
xlabel: 1 - specificity (share of the background predicted suitable)
ylabel: Sensitivity
height: 300
caption: "The ROC curve of the sloth's model, from sdm.evaluate's confusion table: every threshold a point. The area under it is the AUC (0.72); the grey diagonal, a model no better than chance (AUC 0.5). Hover to read the pairs."
```

```tfchart
type: line
data: /data/tutorials/species-distributions/evaluation-and-thresholds/by-threshold.csv
x: threshold
y: [Sensitivity, Specificity, TSS]
colors: { Sensitivity: "#d1495b", Specificity: "#6b7a72", TSS: "#3b8a5a" }
xlabel: Threshold of suitability
ylabel: Share, or TSS
height: 300
caption: "Sensitivity falls and specificity rises as the threshold rises; their sum minus one, the TSS, is flat between 0.45 and 0.70 and peaks at 0.66 (0.31). A flat top means many thresholds are almost equally good: the choice among them is about the use of the map, not about the model."
```

![The predictions at the presences and at the background, with three thresholds.](/img/tutorials/species-distributions/evaluation-and-thresholds/histogram.png "The suitability predicted at the 83 presences (red) and at the 2 000 background points (grey). The two overlap widely: much of the accessible area is as suitable as the places with records, as expected when the background lies close to them. The dashed lines are three thresholds: the minimum training presence (0.18), the 10th percentile of the presences (0.33) and the maximum TSS (0.66).")

### The Boyce curve

```tfchart
type: line
data: /data/tutorials/species-distributions/evaluation-and-thresholds/boyce.csv
x: suitability
y: ["P/E"]
colors: { "P/E": "#e8901f" }
xlabel: Suitability (the window's middle)
ylabel: Predicted / expected
height: 280
caption: "The predicted-to-expected ratio F along the suitability (sdm.evaluate's boyce table, a moving window a tenth of the range wide). Below about 0.55 the presences are mostly rarer than the area would suggest (F below 1, with a bump near 0.3); above 0.75 they are 1.7 to 9.5 times more frequent than expected. The curve rises almost steadily: Boyce 0.92."
```

A curve that rises steadily is what a good model of habitat suitability gives. A curve that is flat and then jumps,
or that falls at the top, would say that the model separates only part of the range of suitability, or that its
highest values are in places the records avoid.

### Thresholds and the maps they draw

The eight methods of [[sdm.threshold]] on the same predictions:

```tfslider
label: Threshold method
caption: "The sloth's map made binary by each method of sdm.threshold (green: suitable; the accessible area outlined; the presences in red). The share of the accessible area called suitable runs from 96 % to 6 %. Note the south of Chile: its cool, wet, unseasonal climate is suitable to the model, outside the accessible area, where the sloth has never been (part 6)."
frames:
  - { value: equal_prevalence, src: /img/tutorials/species-distributions/evaluation-and-thresholds/binary_equal_prevalence.png, stats: { "Threshold": "0.04", "Sensitivity": "100 %", "Specificity": "4 %", "Area suitable": "96 %" } }
  - { value: no_omission, src: /img/tutorials/species-distributions/evaluation-and-thresholds/binary_no_omission.png, stats: { "Threshold": "0.18", "Sensitivity": "100 %", "Specificity": "18 %", "Area suitable": "85 %" } }
  - { value: "sensitivity 0.95", src: /img/tutorials/species-distributions/evaluation-and-thresholds/binary_sensitivity.png, stats: { "Threshold": "0.30", "Sensitivity": "95 %", "Specificity": "31 %", "Area suitable": "72 %" } }
  - { value: p10, src: /img/tutorials/species-distributions/evaluation-and-thresholds/binary_p10.png, stats: { "Threshold": "0.33", "Sensitivity": "90 %", "Specificity": "34 %", "Area suitable": "69 %" } }
  - { value: equal_sens_spec, src: /img/tutorials/species-distributions/evaluation-and-thresholds/binary_equal_sens_spec.png, stats: { "Threshold": "0.60", "Sensitivity": "64 %", "Specificity": "64 %", "Area suitable": "39 %" } }
  - { value: max_spec_sens, src: /img/tutorials/species-distributions/evaluation-and-thresholds/binary_max_spec_sens.png, stats: { "Threshold": "0.66", "Sensitivity": "53 %", "Specificity": "78 %", "Area suitable": "26 %" } }
  - { value: max_kappa, src: /img/tutorials/species-distributions/evaluation-and-thresholds/binary_max_kappa.png, stats: { "Threshold": "0.81", "Sensitivity": "24 %", "Specificity": "96 %", "Area suitable": "6 %" } }
start: 5
play: false
```

| Method | Threshold | Sensitivity | Specificity | TSS | Accessible area suitable |
|---|---|---|---|---|---|
| `equal_prevalence` | 0.04 | 100 % | 4 % | 0.04 | 96 % |
| `no_omission` | 0.18 | 100 % | 18 % | 0.18 | 85 % |
| `mtp` | 0.18 | 100 % | 18 % | 0.18 | 85 % |
| `sensitivity` (0.95) | 0.30 | 95 % | 31 % | 0.26 | 72 % |
| `p10` | 0.33 | 90 % | 34 % | 0.24 | 69 % |
| `equal_sens_spec` | 0.60 | 64 % | 64 % | 0.28 | 39 % |
| `max_spec_sens` | 0.66 | 53 % | 78 % | 0.31 | 26 % |
| `max_kappa` | 0.81 | 24 % | 96 % | 0.20 | 6 % |

The suitable area changes fifteen-fold between methods, from the same model. Which one?

- **For a range map** (where could the species be?), keep most presences: `p10` or a sensitivity of 0.9 to 0.95. A
  sloth recorded in a place is there, and a map that leaves out half the records misses known populations.
- **For the core of the range** (where conditions are best, for instance for a reintroduction), `max_spec_sens`.
  It is the best-skilled threshold, and with presence-background data it is stricter than it looks: it calls 26 % of
  the area suitable and keeps only 53 % of the presences, because "specificity" here rewards calling the
  available area unsuitable, whether or not the sloth lives there.
- **`max_kappa` and `equal_prevalence`** depend on the prevalence, which with background points is your choice of
  how many points to draw (83 presences against 2 000 points: 4 %). They make little sense for presence-background
  data: here one calls 6 % of the area suitable and the other 96 %.
- **`no_omission` and `mtp`** let the single least suitable record decide, often a doubtful one.

### Reporting

Report the scores on test data (the folds' means and standard deviations, or independent data), with the
background and the folds they depend on: AUC, TSS, Boyce and the omission rates together, never AUC alone. For a
binary map, give the method and the value of the threshold, the sensitivity it keeps, and the suitable area, and
say why that method suits the use.

## Settings that change the answer

- **The data you evaluate on.** Training scores flatter the model; random folds may too (part 4). The same model
  scores Boyce 0.92 on its training records and 0.55 over spatial folds.
- **The background.** AUC, TSS and specificity are all measured against the background: a wider background raises
  them all ([part 3](../predictors-and-background/)).
- **The Boyce window.** [[sdm.boyce]] (and the `boyce` of [[sdm.evaluate]]) uses a moving window a tenth of the
  range wide by default (`window`); `nclass` sets classes instead. Few presences make the index noisy whatever the
  window.
- **The threshold method**, as the slider shows: a factor of 15 in the area called suitable.

## Common pitfalls

- **Reporting the AUC alone,** or comparing AUCs of models with different backgrounds or study areas.
- **Evaluating on the training records** and calling it validation. Use the folds, or independent data.
- **Choosing the threshold that gives the map you expected.** Choose the method for the use, before looking, and
  report it.
- **Kappa with background points.** It depends on the prevalence, which the number of background points sets.
- **A binary map where a continuous one would do.** Thresholds throw away the gradient of suitability; keep it for
  prioritization, connectivity and comparisons.
- **Reading specificity as absence.** A background point predicted suitable is not an error in the field: it may hold
  the species.

## Going further

- [Part 6: Projection and extrapolation](../projection-and-extrapolation/): the model in another climate, and where
  its suitability cannot be trusted (south Chile is an example).
- [Part 4: Fitting models](../fitting-models/): the cross-validated scores of [[sdm.fit]].
- Binary maps as habitat for patterns and networks: [MSPA](/tutorials/habitat-patterns/mspa/).
- The operators: [[sdm.evaluate]], [[sdm.threshold]], [[sdm.boyce]], [[sdm.omission_rate]].

## References

- Allouche, O., Tsoar, A., & Kadmon, R. (2006). Assessing the accuracy of species distribution models: prevalence, kappa and the true skill statistic (TSS). *Journal of Applied Ecology*, 43(6), 1223–1232. https://doi.org/10.1111/j.1365-2664.2006.01214.x
- Boyce, M. S., Vernier, P. R., Nielsen, S. E., & Schmiegelow, F. K. A. (2002). Evaluating resource selection functions. *Ecological Modelling*, 157(2–3), 281–300. https://doi.org/10.1016/S0304-3800(02)00200-4
- Fielding, A. H., & Bell, J. F. (1997). A review of methods for the assessment of prediction errors in conservation presence/absence models. *Environmental Conservation*, 24(1), 38–49. https://doi.org/10.1017/S0376892997000088
- Hanley, J. A., & McNeil, B. J. (1982). The meaning and use of the area under a receiver operating characteristic (ROC) curve. *Radiology*, 143(1), 29–36. https://doi.org/10.1148/radiology.143.1.7063747
- Hirzel, A. H., Le Lay, G., Helfer, V., Randin, C., & Guisan, A. (2006). Evaluating the ability of habitat suitability models to predict species presences. *Ecological Modelling*, 199(2), 142–152. https://doi.org/10.1016/j.ecolmodel.2006.05.017
- Jiménez-Valverde, A. (2012). Insights into the area under the receiver operating characteristic curve (AUC) as a discrimination measure in species distribution modelling. *Global Ecology and Biogeography*, 21(4), 498–507. https://doi.org/10.1111/j.1466-8238.2011.00683.x
- Liu, C., Berry, P. M., Dawson, T. P., & Pearson, R. G. (2005). Selecting thresholds of occurrence in the prediction of species distributions. *Ecography*, 28(3), 385–393. https://doi.org/10.1111/j.0906-7590.2005.03957.x
- Liu, C., White, M., & Newell, G. (2013). Selecting thresholds for the prediction of species occurrence with presence-only data. *Journal of Biogeography*, 40(4), 778–789. https://doi.org/10.1111/jbi.12058
- Lobo, J. M., Jiménez-Valverde, A., & Real, R. (2008). AUC: a misleading measure of the performance of predictive distribution models. *Global Ecology and Biogeography*, 17(2), 145–151. https://doi.org/10.1111/j.1466-8238.2007.00358.x
- Muscarella, R., Galante, P. J., Soley-Guardia, M., Boria, R. A., Kass, J. M., Uriarte, M., & Anderson, R. P. (2014). ENMeval: an R package for conducting spatially independent evaluations and estimating optimal model complexity for Maxent ecological niche models. *Methods in Ecology and Evolution*, 5(11), 1198–1205. https://doi.org/10.1111/2041-210X.12261
- Radosavljevic, A., & Anderson, R. P. (2014). Making better Maxent models of species distributions: complexity, overfitting and evaluation. *Journal of Biogeography*, 41(4), 629–643. https://doi.org/10.1111/jbi.12227
