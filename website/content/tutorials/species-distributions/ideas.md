---
title: "Species distribution models: the ideas"
short: The ideas
description: What a species distribution model estimates, from the niche and the BAM diagram to presences, background points and the limits of a suitability map, with a first model of the brown-throated sloth.
order: 10
level: beginner
time: 40 min
# template: Species distribution model
ops: [sdm.prepare, sdm.fit, sdm.maxnet_predict, sdm.background, view.plot]
series: Species distribution models
part: 1
---

Where could a species live? For most species nobody has mapped it. What we have are **records** (places
where the species was seen, collected or trapped) and **maps of the environment** (climate, soils, land cover).
A species distribution model relates the two and draws the result over every cell of a map. In this first part
of the series you will learn what such a model estimates and what it does not, why it needs "background
points", and you will run two models in Terraflux: one of a virtual species whose true distribution we know, and
one of a real animal, the brown-throated sloth of Central and South America. At the end you will have a map of
suitability for the sloth and a clear idea of how to read it.

The next parts go through each step in depth: the [records](../occurrence-data/), the
[predictors and the background](../predictors-and-background/), [fitting models](../fitting-models/),
[evaluating them](../evaluation-and-thresholds/), [projecting them to other climates](../projection-and-extrapolation/)
and the [whole preparation in one node](../prepare-in-one-node/).

## The idea

### Niches and distributions

A species persists where the conditions let its populations grow. Hutchinson (1957) gave this an exact form:
imagine a space whose axes are the environmental factors that matter to the species (temperature, rainfall,
the acidity of the soil...). The **fundamental niche** is the region of that space where the species could
maintain a population; the **realized niche** is the smaller part it actually occupies once competitors,
predators, diseases and mutualists are taken into account.

The niche lives in **environmental space**; the distribution lives in **geographic space**, on the map. A
species distribution model (SDM; also called an *ecological niche model* or a *habitat suitability model*)
moves between the two. It looks at the conditions where the species was recorded, learns which conditions
those are, then asks where on the map the same conditions occur.

![The records of the sloth plotted against two climate variables](/img/tutorials/species-distributions/ideas/envspace.png "The brown-throated sloth in environmental space: each grey dot is a cell of the map (Central and South America at 0.5°), each red dot a record of the sloth. The records sit in a corner of the space, warm (above about 22 °C) and wet (above about 1 000 mm a year). A model learns this corner, then maps the cells that fall in it.")

The figure shows this for the sloth. Each grey dot is a cell of the continent, placed by its annual mean
temperature and its annual rainfall; each red dot is a record of the sloth. The records crowd into the warm,
wet corner of the cloud. A model's task is to describe that corner and then colour every cell of the map by
how well it fits.

### The BAM diagram

Why is the distribution not simply "everywhere the climate is right"? Soberón and Peterson (2005) answered with a
diagram of three sets of places:

- **A** (*abiotic*): where the physical conditions (climate, soils) allow the species to grow;
- **B** (*biotic*): where the interactions allow it (its food, its hosts or pollinators, no competitor that
  excludes it);
- **M** (*movement*): the area the species has been able to reach by dispersal over the relevant time.

![The BAM diagram: three overlapping circles A, B and M](/img/tutorials/species-distributions/ideas/bam.png "The BAM diagram (after Soberón & Peterson, 2005). The species occupies G_O, where suitable abiotic conditions (A), suitable biotic conditions (B) and the accessible area (M) overlap. Places in A and B but outside M (G_I) are suitable but unreached: an introduced species could invade them.")

The species occurs where the three overlap (G<sub>O</sub>, the occupied area). Places that are suitable in A and
B but lie outside M are *invadable* (G<sub>I</sub>): empty not because they are unsuitable, but because the
species never got there. Think of a frog of an island, or of a plant that crossed an ocean in a ship and is now
spreading over a continent where it had never been.

!!! science "Key idea"
    A correlative model fitted to climate estimates something between **A** and the occupied area, depending
    on the data and on how **M** was handled. A suitability map is not a map of where the species *is*: it is a
    map of where the conditions resemble those at its records.

Keep the diagram in mind throughout the series: it explains why the background points are drawn from the area
the species could reach ([part 3](../predictors-and-background/)) and why a projection to another continent
or another century needs care ([part 6](../projection-and-extrapolation/)).

### Correlative models

There are two families of models. **Mechanistic** models start from physiology: the temperatures at which a
lizard can forage, the water balance of a tree, and compute where these limits are met. They need
experiments most species never had. **Correlative** models start from records: they relate the places where the
species was found to the environment there, with a statistical model or a machine learning algorithm. They need
only records and maps, which is why they became the most used tool of biogeography and conservation planning
(Guisan & Thuiller, 2005; Elith & Leathwick, 2009). Terraflux fits correlative models, and this series is about
them.

Correlative models are used to:

- fill in the gaps between records, to map the likely range of a species that was sampled in few places;
- find unknown populations: field surveys are sent where the model predicts high suitability;
- give the features of a [spatial prioritization](../../spatial-prioritization/) (a map per species) or the
  habitat of a [connectivity](../../habitat-networks/) analysis;
- project a distribution to a future climate, or to a region a species is invading.

### Presences, absences and background

How we fit the model depends on what the records are.

**Presence-absence** data come from surveys: someone visited a set of sites and wrote down where the species was
found *and where it was not*. With them, a model can estimate the probability that the species is present in a
cell (for the effort of that survey), with an ordinary logistic regression.

Most data are **presence-only**: specimens in museums and herbaria, and observations gathered by the Global
Biodiversity Information Facility (GBIF). They say where the species was seen, never where someone looked and did
not see it. A model of presence-only data contrasts the presences with **background points**: a random sample
of the cells of the study area, which describes the conditions *available* to the species.

!!! science "Key idea"
    Background points are **not absences**. The species may well live at some of them. The model learns how
    the conditions at the presences differ from the conditions available, which is a **relative** suitability.
    Without knowing how common the species is (its prevalence), the probability of presence cannot be recovered
    from presence-only data (Ward et al., 2009; Phillips et al., 2009).

This is why the outputs of presence-only models are called *suitability* and not *probability*, even when they
range from 0 to 1, and why where the background is drawn matters so much: the model can only tell the
presences apart from what the background offers.

### What a model can and cannot tell

A model can tell you which places resemble, in the predictors you chose, the places where the species was
recorded. It cannot tell you:

- whether the species is there now (it may have been extirpated, or never arrived: the M of the diagram);
- whether a predictor *causes* the pattern (a correlated factor that you did not include may be the real one);
- what happens under conditions the records never covered (a model extrapolates there, and different algorithms
  extrapolate differently; [part 6](../projection-and-extrapolation/));
- whether the records themselves were biased toward roads and cities ([part 2](../occurrence-data/)).

Every step of this series is a way of making the first statement as true as possible, and of being honest about
the others.

## When to use it

Use a species distribution model when you have records of a species and maps of the environment, and you need a
continuous map of where conditions suit it: to fill the gaps in an atlas, to guide surveys, to give the habitat
of a species to a fragmentation or connectivity analysis, to weigh a species in a conservation plan, or to ask how
its suitable area might shift in a future climate.

Do not use one when:

- you already have a good map of the species' habitat (a forest map for a forest specialist): the map is better
  than a model of it;
- the species' distribution is limited mostly by dispersal or history rather than by the environment (a recent
  invader still spreading, a species confined to islands it cannot leave): the model will learn the limits of
  M, not the niche;
- you have very few records (under about 10 to 15 after cleaning): even the simplest models become unreliable,
  and expert range maps may be better.

For **many species at once** in grid cells (richness, endemism), see the [biodiversity
tutorials](../../biodiversity/). To turn a suitability map into habitat patches, use
[MSPA](../../habitat-patterns/); to turn it into a resistance surface, see [circuit
connectivity](../../circuit-connectivity/).

## What you need

- **Records** of the species: a table with longitude and latitude columns (a CSV of a GBIF download, a
  spreadsheet of your own surveys), or points in a vector file. Terraflux can also download them from GBIF
  (`sdm.download_gbif`, [part 2](../occurrence-data/)).
- **Predictors**: rasters of the environment on one grid (the same cells, extent and coordinate reference
  system), one band per variable. Climate is the usual start (WorldClim, CHELSA); soils, terrain and land cover
  can be added. A *raster* is a grid of cells, each with a value; a *coordinate reference system* (CRS) says
  how the grid's coordinates relate to places on Earth (longitude and latitude in degrees, or metres of a
  projection).

This series uses the classic example of Maxent, also used by the R package dismo: 116 records of the
**brown-throated sloth** (*Bradypus variegatus*), and nine **bioclimatic variables** of WorldClim at a
resolution of 0.5° (about 55 km at the equator) over Central and South America. Temperatures are in tenths of a
degree Celsius (a value of 250 is 25 °C), rainfall in millimetres.

- [bradypus.csv](/data/tutorials/species-distributions/ideas/bradypus.csv): the records (columns species, lon,
  lat).
- [bioclim.tif](/data/tutorials/species-distributions/ideas/bioclim.tif): the nine variables, one band each,
  named bio1, bio5, bio6, bio7, bio8, bio9, bio12, bio16, bio17.

| Band | Meaning |
|---|---|
| bio1 | Annual mean temperature |
| bio5 | Maximum temperature of the warmest month |
| bio6 | Minimum temperature of the coldest month |
| bio7 | Annual range of temperature (bio5 − bio6) |
| bio8 | Mean temperature of the wettest quarter |
| bio9 | Mean temperature of the driest quarter |
| bio12 | Annual precipitation |
| bio16 | Precipitation of the wettest quarter |
| bio17 | Precipitation of the driest quarter |

![Records of the sloth over the annual mean temperature](/img/tutorials/species-distributions/ideas/records.png "The 116 records of the brown-throated sloth over the annual mean temperature (bio1). Most lie in Central America, along the Andes and in the Amazon basin.")

## A virtual species first

Before modelling a real species, it helps to model one whose answer we know. The template **Species
distribution model** (menu Templates → Species) does just that: it draws three predictors by chance, defines
where a virtual species finds its conditions, draws records of it there, and fits a model to the records.
Open it in the application, or paste the model below into a new model.

```tfgraph
#! caption: The first half of the template "Species distribution model": three predictors drawn by chance (nodes 1 to 4), the virtual species' true suitability (node 5) and 120 records drawn from it (node 6).
#! numbered: true
#! highlight: suitability, presences
format = "terraflux/1"

[[nodes]]
id = "temperature"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 14, seed = 21 }

[[nodes]]
id = "rainfall"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 22, seed = 22 }

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 6, seed = 23 }

[[nodes]]
id = "predictors"
op = "stack.of"
inputs = { temperature = "temperature.map", rainfall = "rainfall.map", relief = "relief.map" }

[[nodes]]
id = "suitability"
op = "map.calc"
inputs = { t = "temperature.map", r = "rainfall.map" }
params = { expression = "exp(-((t - 0.6) ^ 2) / 0.02 - ((r - 0.5) ^ 2) / 0.05)" }

[[nodes]]
id = "presences"
op = "sdm.background"
inputs = { env = "predictors.stack", bias = "suitability.map" }
params = { n = 120, seed = 3 }
```

```tfgraph
#! caption: The second half of the template: the records and the predictors prepared in one node (node 7, data), MaxNet fitted over spatial folds (node 8, fit), applied to every cell (node 9, predict) and shown (node 10, plot).
#! fragment: true
#! highlight: data, fit, predict
[[nodes]]
id = "data"
op = "sdm.prepare"
inputs = { occurrences = "presences.points", env = "predictors.stack" }
params = { clean = false, background = 800, folds = 4, seed = 1 }

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }

[[nodes]]
id = "predict"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "predict.prediction" }
params = { title = "Suitability (MaxNet, cloglog)" }
```

What each node does:

1. **temperature**, **rainfall**, **relief** (nodes 1 to 3, [[nlm.gaussian_field]]): three smooth random
   surfaces from 0 to 1, standing in for climate layers. Their `range` sets how far values stay alike: 14 cells
   for temperature, 22 for rainfall, 6 for a rugged relief.
2. **predictors** (node 4, [[stack.of]]): the three maps as one *stack*, a set of maps of one grid, each layer
   named after its input (temperature, rainfall, relief). Models take their predictors as a stack.
3. **suitability** (node 5, [[map.calc]]): the truth. The species does best at a temperature of 0.6 and a
   rainfall of 0.5 and worse the farther a cell is from them (a bell-shaped response to each). Relief plays no
   part: it is a predictor the model will have to learn to ignore.
4. **presences** (node 6, [[sdm.background]]): 120 cells drawn at random with a chance proportional to the true
   suitability. This mimics a species recorded more often where it is more common.
5. **data** (node 7, [[sdm.prepare]]): the preparation of the records in one node. It keeps one record per cell,
   checks the predictors for collinearity, draws 800 background points and splits everything into 4 spatial
   folds. `clean = false` skips the tests of coordinates, which only make sense for longitude and latitude.
6. **fit** (node 8, [[sdm.fit]]): a MaxNet model (the open-source form of Maxent; Phillips et al., 2006) fitted to the presences
   against the background, once per fold to measure how well it predicts the fold it did not see, then once on
   all the records.
7. **predict** (node 9, [[sdm.maxnet_predict]]) and **plot** (node 10, [[view.plot]]): the model applied to every
   cell of the predictors, and its map.

!!! app "In the application"
    Choose **Templates → Species → Species distribution model**. The graph opens on the canvas. Click the node
    **plot** and press ++ctrl+enter++ (or **Run to here**): Terraflux runs it and everything it needs, and the
    map opens in a tab of its own. Click any node to see its settings in the Node pane and, under the canvas,
    what it gives: a map, a table, a value.

Because we drew the records ourselves, we can compare the model with the truth. Drag the handle:

```tfcompare
caption: The virtual species. Left of the handle, the true suitability (node 5); right, MaxNet's estimate from 160 records (node 9). Both on a scale of 0 to 1. The model finds the two areas of high suitability and leaves out the cells that are too cold, too warm or too dry; it is grainier than the truth because it also used relief, a predictor that plays no part.
before: { src: /img/tutorials/species-distributions/ideas/virtual_truth.png, label: True suitability }
after: { src: /img/tutorials/species-distributions/ideas/virtual_model.png, label: MaxNet }
```

## A first model of the sloth

Now a real species. The model reads the records and the climate, prepares them, fits MaxNet and maps it:

```tfgraph
#! caption: A first model of the sloth: the records (node 1) and the climate (node 2) prepared in one node (3), MaxNet fitted (4) and applied to the climate (5). Add a view.plot of the prediction to see it in a tab of its own.
#! numbered: true
#! highlight: data, fit, predict
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
params = { background = 5000 }

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }

[[nodes]]
id = "predict"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

```

<div class="steps" markdown>

1. **Read the records** with [[vector.read]] (library subject **IO**): set `path` to `bradypus.csv`. GDAL reads
   the CSV as features with the columns of the file; the SDM nodes find the coordinates in the columns named
   `lon` and `lat` by themselves (they also recognise `x`/`y`, `longitude`/`latitude` and GBIF's
   `decimalLongitude`/`decimalLatitude`). Connect its **vector** port, which carries every column.
2. **Read the climate** with [[raster.read]]: `path = bioclim.tif`; name the node `climate`. Its `stack` port holds
   the nine bands, named after the file's band names (its `map` port only the first band, bio1).
3. **Prepare** with [[sdm.prepare]] (subject **Species distributions**): drag from `records.vector` to its
   `occurrences` input and from `climate.raster` to `env`. Set `background = 5000`: this coarse map has fewer than
   10 000 cells of land, so the default of 10 000 points would be more than the cells available.
4. **Fit** with [[sdm.fit]]: connect `data.records` to `records`. The defaults fit MaxNet over the folds.
5. **Predict** with [[sdm.maxnet_predict]]: connect `fit.model` to `model` and `data.env` to `env` (the predictors
   that the preparation kept), then **plot** the prediction with [[view.plot]].

</div>

From the command line, save the model as `sloth.toml` next to the two files and run it:

```bash
terraflux run sloth.toml --out results
```

## Read the results

![Suitability of the sloth from a first MaxNet model](/img/tutorials/species-distributions/ideas/first_model.png "Suitability of the brown-throated sloth from MaxNet with the default preparation (cloglog output, 0 to 1), and the 92 records left after cleaning and keeping one per cell. High in the humid lowlands of the Amazon and the Guianas, along the Pacific coast of Colombia and in Central America; low in the dry, cold and seasonal south.")

The map is a **cloglog** suitability between 0 and 1 (the default output of MaxNet; [part 4](../fitting-models/)
explains the outputs). It is high where the climate resembles that of the records (the humid tropical lowlands
from Central America to the Atlantic forest of Brazil) and low in the deserts of Mexico, the dry diagonal of
Brazil, the high Andes and the temperate south.

Select the node **data** to read its `report` output under the canvas: what each step of the preparation did.
For the sloth it says:

| Step | Presences | Removed | Detail |
|---|---|---|---|
| input | 116 | 0 | |
| clean | 113 | 3 | capitals 1, seas 1, institutions 1 |
| one per cell | 92 | 21 | |
| area | 92 | 0 | whole extent of the predictors |
| predictors | 92 | 0 | dropped bio1, bio6, bio9, bio12 |
| background | 92 | 0 | 5000 points |
| folds | 92 | 0 | 5 folds |

Three records failed the tests of coordinates (one at a capital city, one in the sea, one at a biodiversity
institution); 21 shared a cell of 0.5° with an earlier record; four temperature and rainfall variables were
dropped because the others predicted them almost exactly (collinearity); 5 000 background points were drawn;
and the records were split into five folds of spatial blocks. The model kept bio5, bio7, bio8, bio16 and bio17.

Select **fit** to read its `summary`: the means over the five folds of the scores on the records each fold left
out. For this first model the test AUC is 0.85 and the Boyce index 0.62; [part 5](../evaluation-and-thresholds/)
explains them. Do not take them at face value yet: the background was drawn from the whole continent, including
Patagonia, which is easy to tell apart from a tropical sloth. [Part 3](../predictors-and-background/) shows why
this inflates the scores and how to draw a fairer background.

!!! tip "How to report a model"
    A methods section should say where the records came from and how many were kept after each step (the
    `report`), which predictors were used and why, how the background was drawn (how many points, from which
    area), which algorithm and settings, how it was evaluated (the folds) and with which scores, and which output
    the map shows. The `settings` output of [[sdm.prepare]] lists every choice, seeds included.

## Settings that change the answer

The number of records is the first thing that limits a model. The virtual species lets us see how: the same
template with 20 to 320 records drawn from the same truth.

```tfslider
label: Records
caption: MaxNet's estimate of the virtual species from more and more records (red dots), drawn from the same true suitability. With 20 records the model sees only part of the conditions and the map is coarse; from about 80 records it finds both areas of high suitability. The table gives the rank correlation between the estimate and the truth over all cells, and the test AUC and Boyce index over the four folds.
frames:
  - { value: 20, src: /img/tutorials/species-distributions/ideas/virtual_20.png, stats: { "Correlation with the truth": "0.90", "Test AUC": "0.85", "Boyce": "0.24" } }
  - { value: 40, src: /img/tutorials/species-distributions/ideas/virtual_40.png, stats: { "Correlation with the truth": "0.94", "Test AUC": "0.79", "Boyce": "0.29" } }
  - { value: 80, src: /img/tutorials/species-distributions/ideas/virtual_80.png, stats: { "Correlation with the truth": "0.98", "Test AUC": "0.80", "Boyce": "0.64" } }
  - { value: 160, src: /img/tutorials/species-distributions/ideas/virtual_160.png, stats: { "Correlation with the truth": "0.99", "Test AUC": "0.79", "Boyce": "0.81" } }
  - { value: 320, src: /img/tutorials/species-distributions/ideas/virtual_320.png, stats: { "Correlation with the truth": "0.98", "Test AUC": "0.79", "Boyce": "0.90" } }
```

Two lessons hide in the table. First, more records give a better map, quickly at first and then hardly at all: from about 80 records the estimate ranks the cells almost exactly as the truth does. Second,
the AUC hardly changes, and is highest with 20 records, when the map is worst: with few test records it is
noisy, and it measures how well presences are ranked above the background, not how close the map is to the
truth. The Boyce index follows the improvement better. This is why the series evaluates models with several
scores at once ([part 5](../evaluation-and-thresholds/)).

## Common pitfalls

- **Reading suitability as probability.** A cloglog value of 0.8 from presence-only data does not mean the
  species is present in 80 % of such cells. It is a relative index; only presence-absence surveys can estimate a
  probability.
- **Raw records straight into the model.** GBIF records include coordinates at country centroids, at museums,
  at 0° 0′, in the sea. Clean them first ([part 2](../occurrence-data/)).
- **A background from the whole world.** The farther the background reaches beyond where the species could
  go, the easier it is to tell apart, and the more the scores flatter the model ([part 3](../predictors-and-background/)).
- **Dozens of predictors.** Correlated climate layers make a model unstable; choose few, for an ecological reason
  ([part 3](../predictors-and-background/)).
- **Trusting a projection where the model extrapolates.** Map where the new conditions lie outside those the
  model knows ([part 6](../projection-and-extrapolation/)).
- **Different grids.** All predictors must share one grid. Use [[raster.match]] to put a layer on another's grid
  before stacking them.

## Going further

- [Part 2: Occurrence data](../occurrence-data/): download records from GBIF, clean them and deal with sampling
  bias.
- [Part 3: Predictors and the accessible area](../predictors-and-background/).
- The operators of this subject: [Species distributions](/operators/species-distributions/) in the operator reference.

## References

- Elith, J., & Leathwick, J. R. (2009). Species distribution models: ecological explanation and prediction across space and time. *Annual Review of Ecology, Evolution, and Systematics*, 40, 677–697. https://doi.org/10.1146/annurev.ecolsys.110308.120159
- Guisan, A., & Thuiller, W. (2005). Predicting species distribution: offering more than simple habitat models. *Ecology Letters*, 8(9), 993–1009. https://doi.org/10.1111/j.1461-0248.2005.00792.x
- Hutchinson, G. E. (1957). Concluding remarks. *Cold Spring Harbor Symposia on Quantitative Biology*, 22, 415–427. https://doi.org/10.1101/SQB.1957.022.01.039
- Phillips, S. J., Anderson, R. P., & Schapire, R. E. (2006). Maximum entropy modeling of species geographic distributions. *Ecological Modelling*, 190(3–4), 231–259. https://doi.org/10.1016/j.ecolmodel.2005.03.026
- Phillips, S. J., Dudík, M., Elith, J., Graham, C. H., Lehmann, A., Leathwick, J., & Ferrier, S. (2009). Sample selection bias and presence-only distribution models: implications for background and pseudo-absence data. *Ecological Applications*, 19(1), 181–197. https://doi.org/10.1890/07-2153.1
- Soberón, J., & Peterson, A. T. (2005). Interpretation of models of fundamental ecological niches and species' distributional areas. *Biodiversity Informatics*, 2, 1–10. https://doi.org/10.17161/bi.v2i0.4
- Ward, G., Hastie, T., Barry, S., Elith, J., & Leathwick, J. R. (2009). Presence-only data and the EM algorithm. *Biometrics*, 65(2), 554–563. https://doi.org/10.1111/j.1541-0420.2008.01116.x
