---
title: Signatures and kinds of landscapes
short: Kinds of landscapes
description: "Cut a land cover into squares, describe each square by its metrics (its spatial signature), summarise them with principal components and non-negative factors, and group the squares into kinds of landscapes by k-means, choosing the number of groups with a clustergram."
order: 60
level: advanced
time: 50 min
template: Kinds of landscapes
ops: [metrics.signatures, metrics.pca, metrics.nmf, metrics.cluster, metrics.clustergram, raster.reclassify]
series: Landscape metrics
part: 6
---

A region of 50 × 50 km holds several kinds of landscape: open plains of arable land, mosaics of fields, pasture and
woods, forests, towns. Ecologists and planners often need them as units: to stratify a sampling design, to report
how each kind changed, to compare regions. This part builds them from the metrics: the map is cut into squares, each
square is described by a handful of landscape and class metrics (its *spatial signature*), the signatures are
summarised by principal components and by non-negative factors, and the squares are grouped by k-means. At the end
you will have a map of kinds of landscapes, a table that says what each kind is, and a defensible number of kinds.

## The idea

Every square of the map is a small landscape, and a set of metrics places it as a point in a space of several
dimensions: its diversity, its edge density, its largest patch, its share of forest... Squares that are close in that
space are alike. Two families of methods then make sense of the cloud of points:

- **Ordination** reduces many correlated metrics to a few axes. Principal component analysis (PCA) finds the
  directions along which the squares differ most; [part 5](/tutorials/landscape-metrics/sampling-and-change/)
  showed that landscape metrics are strongly correlated, so a few components usually hold most of the variation
  (Riitters et al., 1995; Cushman et al., 2008). Non-negative matrix factorization (NMF; Lee & Seung, 1999) does
  something similar with factors that only add up, which are often easier to read as "parts" of a landscape.
- **Classification** groups the squares. K-means puts them into \(k\) groups so that each square is closer to the
  mean of its group than to any other; a **clustergram** (Schonlau, 2002) shows how the groups split as \(k\) grows,
  to help choose \(k\).

The approach follows pylandstats' spatial signature analysis (Bosch, 2019), which Terraflux reproduces through
landschaft: the same signatures, the same PCA and NMF, the same k-means from the same seeds, so the groups are the
same as in Python. Nowosad and Stepinski (2019) used a related idea, two numbers of information theory per square, to
map kinds of landscape over whole continents.

!!! science "Key idea: z-scores and principal components"
    Metrics have different units (ha, m/ha, %), so before PCA and k-means each metric \(x\) is turned into a
    *z-score*, \(z = (x - \bar{x}) / s\): its distance from the mean of all squares in standard deviations. Then
    every metric weighs the same. A principal component is a weighted sum of the z-scores,
    \(\text{PC}_1 = \sum_j l_{1j} z_j\); the weights \(l_{1j}\) are its **loadings**, and its share of the variance
    says how much of the differences between squares it captures.

## When to use it

- **Stratifying**: sample sites spread over the kinds of landscape, so that every kind is represented.
- **Mapping and reporting by kind**: how much of each kind there is, how each changed between two dates
  ([[metrics.signatures]] takes a stack, a row per layer, as well as zones).
- **Finding gradients**: the first principal component is often a gradient of heterogeneity or intensification
  that is a useful predictor in itself.
- **Not** a land cover classification: the groups describe the *pattern* of squares of fixed size, not what a cell
  is. And not a substitute for knowledge of the region: always name the groups by looking at their metrics and maps.

## What you need

A categorical map in a projected system: [`clc.tif`](/data/tutorials/habitat-patterns/mspa/clc.tif), CORINE around
Hannover, grouped into 7 classes as in parts 4 and 5. The squares' size is a choice of scale like a window's: large
enough to hold a landscape (here 2.5 km, 100 × 100 cells), small enough to be homogeneous. In the application,
**Templates > Landscape metrics > Kinds of landscapes** builds the same chain on a land cover drawn along a gradient.

## Build the model

```tfgraph
#! caption: CORINE in 7 classes cut into squares of 2.5 km, the signature of every square, its principal components and factors, and the squares grouped.
#! numbered: true
#! highlight: signatures
format = "terraflux/1"

[[nodes]]
id = "corine"
op = "raster.read"
params = { path = "clc.tif" }

[[nodes]]
id = "cover"
op = "raster.reclassify"
inputs = { map = "corine.map" }
params = { rules = "1..11 -> 1\n12 -> 2\n13..22 -> 3\n23..25 -> 4\n26..34 -> 5\n35..39 -> 6\n40..44 -> 7" }
note = "1 urban, 2 arable, 3 pasture and mixed farming, 4 forest..."

[[nodes]]
id = "signatures"
op = "metrics.signatures"
inputs = { maps = "cover.map" }
params = { grid = 2500, landscape_metrics = ["shdi", "ed", "lpi", "contag", "ai"], class_metrics = ["pland"], classes = [1, 2, 3, 4] }
note = "A row per square of 2.5 km."

[[nodes]]
id = "pca"
op = "metrics.pca"
inputs = { table = "signatures.table" }
params = { components = 3 }

[[nodes]]
id = "nmf"
op = "metrics.nmf"
inputs = { table = "signatures.table" }
params = { factors = 3 }

[[nodes]]
id = "groups"
op = "metrics.cluster"
inputs = { table = "signatures.table" }
params = { k = 4, seed = 0 }
note = "Four kinds of landscape."

[[nodes]]
id = "clustergram"
op = "metrics.clustergram"
inputs = { table = "signatures.table" }
params = { k_min = 1, k_max = 8 }
```

<div class="steps" markdown>

1. **The map** (nodes 1 and 2): read and group the classes, as in part 4.
2. **The signatures** (node 3). Connect node 2's `map` to the `maps` input of [[metrics.signatures]]. `grid` tiles
   the map with squares of that side in map units (2,500 m); `plots` would take polygons or points instead. Its
   `table` has a row per square (`zone`, numbered row by row from the top left), a column per landscape metric, and
   a column per class metric and class: `pland_1` to `pland_4`, the share of urban, arable, pasture and forest.
   `classes` limits the class columns to the classes that matter.
3. **The analyses** (nodes 4 to 7) all take the signatures' `table`: drag from it to each. By default they use every
   column but the keys (`zone`, `landscape`, `class`, `date`); `columns` picks some.

</div>

Choosing the metrics is the important decision. Here, five landscape metrics covering diversity (`shdi`), edge
(`ed`), dominance (`lpi`) and aggregation (`contag`, `ai`), and the shares of the four main classes: composition and
configuration both. Metrics that are near copies of each other (`ed` and `te`, `np` and `pd`) should not both be in:
they would weigh twice.

## Read the results

### The signatures

![The map cut into squares](/img/tutorials/landscape-metrics/landscape-signatures/grid.png "CORINE around Hannover in 7 classes, cut into 400 squares of 2.5 km.")

The table has 400 rows. The first square, in the north-west corner, has 66% arable land, 14% pasture, 12% forest and
8% urban, a Shannon diversity of 1.01, an edge density of 30 m/ha and a largest patch of 63%.

### Principal components

The `loadings` table of [[metrics.pca]]:

| | PC1 | PC2 | PC3 |
|---|---|---|---|
| Variance explained | 52% | 17% | 14% |
| AI | −0.41 | 0.15 | 0.09 |
| CONTAG | −0.41 | −0.02 | 0.02 |
| ED | 0.43 | −0.13 | −0.08 |
| LPI | −0.43 | −0.04 | −0.01 |
| SHDI | 0.44 | 0.01 | 0.00 |
| % urban | 0.00 | 0.00 | 0.86 |
| % arable | −0.15 | −0.67 | −0.35 |
| % pasture | 0.29 | −0.04 | 0.06 |
| % forest | 0.00 | 0.72 | −0.36 |

Read each component by its largest loadings:

- **PC1 (52%)** is a gradient of **heterogeneity**: positive for diversity and edge, negative for aggregation,
  contagion and the largest patch. Squares high on PC1 are fine-grained mosaics; squares low on it are dominated by
  one large patch, whatever its class.
- **PC2 (17%)** opposes **forest** (+0.72) to **arable land** (−0.67): what dominates.
- **PC3 (14%)** is **urban** (+0.86).

Three axes hold 83% of the variation of ten metrics. The `scores` table places every square on them.

![The squares on the first two components](/img/tutorials/landscape-metrics/landscape-signatures/biplot.png "Every square on PC1 and PC2, coloured by its group of the k-means below, and the loadings as arrows. Arable squares (orange) lie low on PC2, forest squares (green) high; mosaics (blue) are to the right, where diversity and edge point.")

### Non-negative factors

[[metrics.nmf]] scales every metric to 0–1 and approximates the table by three factors whose weights are all
positive. Its `components` table:

| Factor | Largest weights | Reads as |
|---|---|---|
| 1 | % arable 1.65, LPI 1.60, AI 1.45, CONTAG 1.15 | large uniform fields |
| 2 | ED 1.03, SHDI 1.01, % pasture 0.46 | a fine mosaic |
| 3 | AI 0.73, LPI 0.66, % forest 0.61 | large forests |

Where PCA gives contrasts (forest *against* arable), NMF gives parts that add up: every square is a mix of
"uniform fields", "mosaic" and "forest", with its `scores` the amount of each. The `error` output says how well the
three factors reproduce the table; try more factors if it is large.

### Kinds of landscapes

[[metrics.cluster]] groups the squares by k-means of their z-scores (`method = "kmeans"`, starts from `seed`, the
best of `n_init` starts), or by a hierarchical linkage (`ward`, `average`...). Its `table` is the signatures with a
`cluster` column; rows with a missing metric are left out.

```tfslider
label: Number of groups, k
caption: "The squares grouped by k-means for k = 2 to 7 (the colours are the groups' numbers, not the same kind at every k). At k = 2 the map splits into the arable plains and the rest; at k = 3 and 4 smaller groups dominated by one class break off (the forests, the city of Hannover in the east); beyond, the mosaic splits into finer variants. Grey: 10 squares left out, all of one class, where contagion is undefined."
frames:
  - { value: 2, src: /img/tutorials/landscape-metrics/landscape-signatures/clusters_2.png, caption: "k = 2", stats: { "Squares per group": "207 / 183" } }
  - { value: 3, src: /img/tutorials/landscape-metrics/landscape-signatures/clusters_3.png, caption: "k = 3", stats: { "Squares per group": "201 / 138 / 51" } }
  - { value: 4, src: /img/tutorials/landscape-metrics/landscape-signatures/clusters_4.png, caption: "k = 4", stats: { "Squares per group": "194 / 139 / 37 / 20" } }
  - { value: 5, src: /img/tutorials/landscape-metrics/landscape-signatures/clusters_5.png, caption: "k = 5", stats: { "Squares per group": "141 / 90 / 109 / 32 / 18" } }
  - { value: 6, src: /img/tutorials/landscape-metrics/landscape-signatures/clusters_6.png, caption: "k = 6", stats: { "Squares per group": "114 / 80 / 108 / 51 / 19 / 18" } }
  - { value: 7, src: /img/tutorials/landscape-metrics/landscape-signatures/clusters_7.png, caption: "k = 7", stats: { "Squares per group": "101 / 67 / 34 / 95 / 59 / 21 / 13" } }
```

With \(k = 4\), the mean metrics of each group say what it is:

| Group | Squares | SHDI | ED (m/ha) | % urban | % arable | % forest | Kind |
|---|---|---|---|---|---|---|---|
| 0 | 194 | 1.04 | 26.4 | 13 | 42 | 22 | mixed farmland (19% pasture) |
| 1 | 139 | 0.56 | 14.5 | 8 | 81 | 7 | arable plains |
| 2 | 37 | 0.44 | 8.8 | 3 | 12 | 76 | forests |
| 3 | 20 | 0.52 | 11.3 | 81 | 8 | 3 | city |

![What each group is](/img/tutorials/landscape-metrics/landscape-signatures/group_profiles.png "The mean z-score of every metric in each of the four groups: red above the average square, blue below. Group 0 is above average in diversity and edge; groups 1 to 3 are each dominated by one class (arable, forest, urban) with a large patch and high contagion.")

The largest patch covers 48% of an average square of group 0 and around 80% in the others. Naming the groups is the analyst's job, from such a table: the algorithm only says that the squares of a group are
alike.

### How many kinds?

There is no single right \(k\). The clustergram shows how the groups form as \(k\) grows: for every \(k\), a dot per
group at the group's mean on the first principal component, sized by its number of squares, and lines that follow
the squares from one \(k\) to the next.

![Clustergram](/img/tutorials/landscape-metrics/landscape-signatures/clustergram.png "The clustergram of the squares for k = 1 to 8. The split at k = 2 is clean and stable; at k = 3 and 4 small groups break off at the ends of PC1 without disturbing the large ones; from k = 5 the large groups are reshuffled (many crossing lines): the new groups are subdivisions of a continuum.")

A good \(k\) is one after which the large groups stay stable: here 4. Beyond, the groups cut a continuous gradient
at arbitrary places. Choose \(k\) also by use: four kinds are easy to sample and report; seven may be needed to
separate the variants of the mosaic if they matter to the species.

## Settings that change the answer

- **The square's size** (`grid`): a scale, as the window's in part 4. Small squares are often one class; large ones
  mix kinds.
- **The metrics**: the groups are only as meaningful as the metrics chosen. Redundant metrics weigh twice; a metric
  irrelevant to the question still pulls the groups.
- **The method**: k-means makes compact groups of similar size in z-score space; Ward's linkage behaves similarly;
  `average` and `single` linkages make unequal groups and chains.
- **`components`**: clustering the first principal components instead of the metrics removes the noise of the minor
  axes.
- **The seed**: k-means starts at random; `seed` and `n_init` make the result repeatable. Report them.

## Common pitfalls

- **Squares cut by the map's border** (the last row and column when the map is not a multiple of the square) are
  smaller: their metrics are not comparable. Drop them, or choose a square that divides the map.
- **Missing values**: squares of a single class have no contagion; they are left out of PCA and clustering (here 10).
  If such squares matter (all forest, all city), choose metrics defined for them, or add them back as a kind of
  their own.
- **Reading k-means groups as natural types**: they are a partition of a continuum unless the clustergram shows
  clean, stable splits.
- **Two dates with different groups**: to follow change, classify the squares of both dates *together* (one table,
  the date as a key), so that the groups mean the same thing at both.

## Going further

- [Part 7](/tutorials/landscape-metrics/large-maps/): signatures of maps larger than memory (`block_rows`).
- [Biodiversity](/tutorials/biodiversity/): clusters of regions by their species, Biodiverse's analyses.
- The operator reference: [[metrics.signatures]], [[metrics.pca]], [[metrics.nmf]], [[metrics.cluster]],
  [[metrics.clustergram]].

## References

- Bosch, M. (2019). PyLandStats: an open-source Pythonic library to compute landscape metrics. *PLOS ONE*, 14(12), e0225734. https://doi.org/10.1371/journal.pone.0225734
- Cushman, S. A., McGarigal, K., & Neel, M. C. (2008). Parsimony in landscape metrics: strength, universality, and consistency. *Ecological Indicators*, 8(5), 691–703. https://doi.org/10.1016/j.ecolind.2007.12.002
- Lee, D. D., & Seung, H. S. (1999). Learning the parts of objects by non-negative matrix factorization. *Nature*, 401(6755), 788–791. https://doi.org/10.1038/44565
- Nowosad, J., & Stepinski, T. F. (2019). Information theory as a consistent framework for quantification and classification of landscape patterns. *Landscape Ecology*, 34(9), 2091–2101. https://doi.org/10.1007/s10980-019-00830-x
- Riitters, K. H., O'Neill, R. V., Hunsaker, C. T., Wickham, J. D., Yankee, D. H., Timmins, S. P., et al. (1995). A factor analysis of landscape pattern and structure metrics. *Landscape Ecology*, 10(1), 23–39. https://doi.org/10.1007/BF00158551
- Schonlau, M. (2002). The clustergram: a graph for visualizing hierarchical and nonhierarchical cluster analyses. *The Stata Journal*, 2(4), 391–402. https://doi.org/10.1177/1536867X0200200405
