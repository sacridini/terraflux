---
title: The landscape level, diversity and contagion
short: Landscape level
description: "Metrics of the whole mosaic: richness, Shannon's and Simpson's diversity and evenness, contagion and interspersion, and the information-theory metrics of Nowosad and Stepinski; on four landscapes of the same composition and on a real map with two legends."
order: 30
level: intermediate
time: 40 min
template: Landscape metrics
ops: [metrics.landscape, raster.reclassify, nlm.random_cluster, nlm.classify]
series: Landscape metrics
part: 3
---

A class metric looks at one land cover; a landscape metric looks at all of them together. How many kinds of land
cover are there, and how evenly do they share the map? Are the classes in large blocks or finely mixed? Does every
class meet every other, or only some? This part answers these questions with [[metrics.landscape]], first on four
landscapes built to have exactly the same composition, so that only the arrangement differs, then on the land cover of
Augusta with two legends, its 15 classes and the same map grouped into 8. At the end you will know which landscape
metrics measure composition and which configuration, and why a landscape metric is only as meaningful as the legend
it is computed on.

## The idea

### Diversity: how many classes, and how even

Diversity indices come from information theory and community ecology. A landscape's classes are treated like the
species of a community, their shares of the map like the species' relative abundances.

!!! science "Key idea: richness, Shannon's and Simpson's diversity"
    With \(m\) classes covering the shares \(P_1, \ldots, P_m\) of the landscape:

    - **Patch richness**, \(\text{PR} = m\): the number of classes.
    - **Shannon's diversity**, \(\text{SHDI} = -\sum_i P_i \ln P_i\) (Shannon, 1948): 0 with one class, \(\ln m\) when
      all \(m\) classes have the same share. It weighs rare classes more than Simpson's index does.
    - **Simpson's diversity**, \(\text{SIDI} = 1 - \sum_i P_i^2\) (Simpson, 1949): the probability that two cells
      picked at random are of different classes. 0 with one class, \(1 - 1/m\) at most.
    - The **evenness** indices divide by the most they could be with \(m\) classes:
      \(\text{SHEI} = \text{SHDI} / \ln m\), \(\text{SIEI} = \text{SIDI} / (1 - 1/m)\), from 0 (one class dominates) to 1
      (all equal).

    All are **composition** metrics: they ignore where the classes are.

### Contagion and interspersion: how mixed

**Contagion** (O'Neill et al., 1988; Li & Reynolds, 1993) measures how clumped the classes are, from the adjacencies
of cells: if you stand on a cell of class \(i\), how likely is the neighbouring cell to be of class \(k\)? A landscape
of large blocks has mostly like adjacencies (forest beside forest) and a high contagion; a finely mixed one has many
unlike adjacencies and a contagion near 0. It mixes two things, how *dispersed* the cells of each class are and how
*interspersed* the classes are with each other (Riitters et al., 1996), which is why FRAGSTATS also gives the
**interspersion and juxtaposition index** (IJI), which measures only the second: from the edges between *patches*,
how evenly every class meets every other class. 100% when every pair of classes shares the same length of edge; low
when some classes meet only some others.

!!! science "Key idea: contagion"
    With \(P_i\) the share of class \(i\) and \(g_{ik}\) the number of adjacencies between cells of classes \(i\)
    and \(k\) (counted twice, both ways), the contagion of \(m\) classes is

    \[ \text{CONTAG} = 100 \left[ 1 + \frac{\sum_{i}\sum_{k} P_i \frac{g_{ik}}{\sum_k g_{ik}} \ln\!\left(P_i \frac{g_{ik}}{\sum_k g_{ik}}\right)}{2 \ln m} \right] \]

    The fraction inside is an entropy of adjacencies scaled by the largest it could be, \(2 \ln m\): 0 when the
    classes are maximally mixed, so CONTAG is 0; and CONTAG approaches 100 when every class is one block.

### Information theory: a consistent frame

Nowosad and Stepinski (2019) showed that two numbers from information theory, computed on the pairs of neighbouring
cells, place any categorical map in a space that separates composition from configuration:

- the **marginal entropy** (`ent`): the diversity of the classes (Shannon's diversity in bits, \(\log_2\));
- the **conditional entropy** (`condent`): how uncertain the class of a cell's neighbour is when the cell's class is
  known: high when the classes are finely mixed, low when they are in blocks;
- the **mutual information** (`mutinf` = `ent` − `condent`): how much a cell's class tells about its neighbour's,
  the aggregation; and its relative version `relmutinf` (mutual information over marginal entropy).

They are landscapemetrics' *complexity* metrics (`type = "complexity"`).

## When to use it

- **Diversity** (`pr`, `shdi`, `sidi`, `shei`) to compare the heterogeneity of landscapes: a habitat-heterogeneity
  hypothesis (more kinds of land cover, more species), the diversity of farming landscapes, the variety of habitats
  available around a site.
- **Contagion, IJI, AI** to say whether the mosaic is coarse or fine-grained; landscape-level `ed` for the amount of
  edge of all kinds.
- **The information-theory metrics** when you want two numbers that are clearly composition and configuration, for
  example to classify many landscapes (part 6).
- **Not** to describe one habitat: landscape metrics treat all classes alike, the forest and the parking lot. For one
  habitat use the class level ([part 2](/tutorials/landscape-metrics/patch-and-class-metrics/)).

## What you need

A categorical map in a projected system, as in parts 1 and 2:
[`augusta.tif`](/data/tutorials/landscape-metrics/what-are-landscape-metrics/augusta.tif). The four landscapes of the
first section are drawn by Terraflux's neutral landscape models, so those models run as they are. In the
application, the template **Templates > Landscape metrics > Landscape metrics** has a [[metrics.landscape]] node set
to the diversity metrics (`type = "diversity"`).

## Four landscapes, one composition

### Build them

```tfgraph
#! caption: Four classes of 25% each, arranged in clusters by a random cluster model, and the landscape metrics of the map.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, resolution = 30, p = 0.5, weights = [0.25, 0.25, 0.25, 0.25], seed = 5 }
note = "Four classes, 25% each, in clusters."

[[nodes]]
id = "landscape"
op = "metrics.landscape"
inputs = { map = "cover.map" }
params = { metrics = ["pr", "shdi", "shei", "sidi", "siei", "ed", "lpi", "contag", "iji", "ai", "ent", "condent", "mutinf", "relmutinf"] }
```

[[nlm.random_cluster]] draws clusters of cells and gives each a class, roughly in the proportions of `weights`; `p` sets how
large the clusters grow. Its `map` goes to the `map` input of [[metrics.landscape]], whose `table` has one row with
every metric listed. The other three landscapes change only node 1:

- **One dominant class**: the same model with `weights = [0.82, 0.06, 0.06, 0.06]`;
- **Salt and pepper**: [[nlm.random]] (a value at random in every cell) cut into four equal classes by
  [[nlm.classify]] (`weights = [0.25, 0.25, 0.25, 0.25]`);
- **Smooth bands**: [[nlm.gaussian_field]] with `range = 25` (a smooth surface) cut the same way. Because
  `nlm.classify` cuts the surface by its values, class 1 is the lowest ground and class 4 the highest, so class 1
  meets only class 2, and so on.

```tfgraph
#! caption: The salt-and-pepper landscape: a random value in every cell, cut into four classes of equal share.
#! fragment: true
format = "terraflux/1"

[[nodes]]
id = "s"
op = "nlm.random"
params = { rows = 150, columns = 150, resolution = 30, seed = 5 }

[[nodes]]
id = "cover"
op = "nlm.classify"
inputs = { map = "s.map" }
params = { weights = [0.25, 0.25, 0.25, 0.25] }

[[nodes]]
id = "landscape"
op = "metrics.landscape"
inputs = { map = "cover.classes" }
params = { metrics = ["pr", "shdi", "contag", "iji", "condent", "mutinf"] }
```

Note the port: [[nlm.classify]]'s output is `classes`, not `map`.

### Read them

```tfgallery
columns: 4
caption: "Four landscapes of 150 × 150 cells (forest green, farming ochre, grassland light green, urban magenta). The first is dominated by one class; the other three have exactly the same composition, four classes of 25%, arranged at random, in clusters, and in smooth bands."
images:
  - { src: /img/tutorials/landscape-metrics/landscape-level-and-diversity/four_dominant.png, caption: "One dominant class" }
  - { src: /img/tutorials/landscape-metrics/landscape-level-and-diversity/four_salt.png, caption: "Salt and pepper" }
  - { src: /img/tutorials/landscape-metrics/landscape-level-and-diversity/four_clusters.png, caption: "Clusters" }
  - { src: /img/tutorials/landscape-metrics/landscape-level-and-diversity/four_smooth.png, caption: "Smooth bands" }
```

| Metric | Dominant | Salt and pepper | Clusters | Smooth bands |
|---|---|---|---|---|
| PR (classes) | 4 | 4 | 4 | 4 |
| SHDI | 0.51 | 1.39 | 1.38 | 1.39 |
| SHEI | 0.37 | 1.00 | 1.00 | 1.00 |
| SIDI | 0.23 | 0.75 | 0.75 | 0.75 |
| ED (m/ha) | 43 | 497 | 125 | 188 |
| LPI (%) | 87.3 | 0.3 | 5.4 | 13.8 |
| CONTAG (%) | 72.5 | 0.0 | 25.2 | 23.7 |
| IJI (%) | 73.4 | 100.0 | 99.8 | 65.9 |
| AI (%) | 93.9 | 25.1 | 81.7 | 72.1 |
| ENT (bits) | 0.74 | 2.00 | 2.00 | 2.00 |
| CONDENT (bits) | 0.36 | 2.00 | 1.00 | 1.05 |
| MUTINF (bits) | 0.37 | 0.00 | 1.00 | 0.95 |

Read the table by rows:

- **The diversity rows (PR to SIDI, and ENT) cannot tell the last three apart.** They have the same composition, and
  diversity is composition. SHDI = 1.386 = ln 4 is the most four classes can have, SHEI = 1. Only the dominated
  landscape differs.
- **The configuration rows (ED to AI, CONDENT, MUTINF) separate them.** Salt and pepper has the most edge, no
  contagion, no mutual information: knowing a cell's class tells you nothing about its neighbour's. Clusters have a
  quarter of that edge, a contagion of 25% and a mutual information of 1 bit.
- **IJI tells the smooth bands apart.** Its classes are as clumped as the clusters (similar contagion), but class 1
  meets only class 2 and class 4 only class 3: interspersion falls to 66%, while in the two random arrangements every
  class meets every other (IJI near 100).
- **The dominant landscape** has a high contagion (72.5%) mostly because one class covers 82% of it: contagion
  mixes composition into configuration. Its IJI is lower because the three small classes rarely meet each other.

Contagion and AI disagree on the smooth bands (contagion below the clusters', AI above): they weigh different
things, the distribution of adjacencies among classes and the like adjacencies of each class.

### Diversity as one class takes over

```tfchart
type: line
data: /data/tutorials/landscape-metrics/landscape-level-and-diversity/dominance.csv
x: forest
y: [shdi, shei, sidi, siei]
labels: { shdi: "Shannon's diversity (SHDI)", shei: "Shannon's evenness (SHEI)", sidi: "Simpson's diversity (SIDI)", siei: "Simpson's evenness (SIEI)" }
colors: { shdi: "#3b8a5a", shei: "#9fd18b", sidi: "#e8901f", siei: "#f3c37b" }
xlabel: "Share of the dominant class (%)"
ylabel: Value
caption: "Four classes in clusters, one of them growing from 22% to 97% of the map and the other three sharing the rest. Diversity falls slowly at first and then steeply. Shannon's index falls later than Simpson's: it weighs the rare classes more, and they stay present."
```

The curve is not linear: a landscape can lose much of its evenness before the indices notice, and a landscape that
is 90% one class still scores SHDI 0.45. Compare diversity between landscapes with the same number of classes, or use
the evenness indices.

## A real map with two legends

The same land cover can be described with many legends. The NLCD of Augusta has 15 classes; its first level groups
them into 8 (water, developed, barren, forest, shrub, grassland, agriculture, wetlands). [[raster.reclassify]] does it
with a rule per line.

```tfgraph
#! caption: Augusta's land cover with its 15 classes and grouped into the NLCD's 8 classes of level 1, and the landscape metrics of both.
#! numbered: true
#! highlight: level1
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "augusta.tif" }

[[nodes]]
id = "level1"
op = "raster.reclassify"
inputs = { map = "cover.raster" }
params = { rules = "11 -> 1\n21 22 23 24 -> 2\n31 -> 3\n41 42 43 -> 4\n52 -> 5\n71 -> 6\n81 82 -> 7\n90 95 -> 8" }
note = "15 classes into 8."

[[nodes]]
id = "m15"
op = "metrics.landscape"
inputs = { map = "cover.raster" }
params = { metrics = ["np", "ed", "lpi", "pr", "shdi", "shei", "contag", "iji", "ai", "mesh"] }

[[nodes]]
id = "m8"
op = "metrics.landscape"
inputs = { map = "level1.map" }
params = { metrics = ["np", "ed", "lpi", "pr", "shdi", "shei", "contag", "iji", "ai", "mesh"] }
```

In the Node pane of node 2, the rules are typed one per line: `21 22 23 24 -> 2` puts the four developed classes in
class 2. Nodes 3 and 4 are the same metrics on the two maps, side by side.

```tfcompare
caption: "Augusta with its 15 classes (left of the handle) and grouped into 8 (right). The three forests become one, the four developed classes one: the boundaries between forest types vanish, and so do their edges."
before: { src: /img/tutorials/landscape-metrics/landscape-level-and-diversity/aug_15.png, label: 15 classes }
after: { src: /img/tutorials/landscape-metrics/landscape-level-and-diversity/aug_8.png, label: 8 classes }
```

![The 8 classes of level 1](/img/tutorials/landscape-metrics/landscape-level-and-diversity/aug_8_legend.png "The 8 classes of level 1 and their shares: forest 66.8%, agriculture 9.5%, developed 8.7%, wetlands 8.3%, grassland 3.6%, shrub 2.2%, water 0.8%, barren under 0.1%.")

| Metric | 15 classes | 8 classes |
|---|---|---|
| Patches (NP) | 1,144 | 459 |
| Edge density (m/ha) | 212 | 114 |
| Largest patch (%) | 7.4 | 15.7 |
| Classes (PR) | 15 | 8 |
| Shannon's diversity | 1.90 | 1.16 |
| Shannon's evenness | 0.70 | 0.56 |
| Contagion (%) | 43.9 | 56.9 |
| Interspersion (IJI, %) | 67.6 | 67.4 |
| Aggregation (AI, %) | 68.8 | 83.3 |
| Effective mesh size (ha) | 34.3 | 118.8 |

Nothing on the ground changed, yet the number of patches fell by 60%, edge density by half, diversity by 40%, and
contagion rose by 13 points. Only IJI barely moved. The legend is as much a choice of scale as the cell size: the
map's *thematic resolution* (Castilla et al., 2009). Two studies of the "same" landscape with different legends will report different landscapes. Choose the legend
for the question (what does the species distinguish?) and keep it the same for every map you compare.

!!! tip "Report it"
    For landscape-level metrics, report the legend (the classes and how they were grouped) together with grain and
    extent. Diversity is best reported with richness and an evenness index; configuration with one edge or
    aggregation metric (`ed` or `contag`) and, if the classes' juxtaposition matters, `iji`.

## Settings that change the answer

- **The legend**, above.
- **`directions`** for the patch-based metrics (`np`, `lpi`, `iji`, `mesh`); contagion and AI use adjacencies of
  cells and do not depend on it.
- **For the complexity metrics**, `neighbourhood` (4 or 8 neighbours for the pairs of cells, default 4), `ordered`
  (ordered pairs, default true) and `base` (the logarithm, default `log2`, in bits). Report them with the values.
- **`classes_max`** for the relative richness `rpr` (richness over the most classes there could be).

## Common pitfalls

- **Reading diversity as configuration.** A landscape of four classes in fine salt and pepper and one in four large
  blocks have the same SHDI. If the question is about fragmentation, diversity is the wrong metric.
- **Contagion with one dominant class.** Contagion rises with dominance as well as with clumping; read it with an
  evenness index.
- **Contagion with one class.** With a single class, contagion and the evenness indices are undefined (NA): a
  zone of a map that is all forest has no contagion. Tables of many zones then have NA values (part 5).
- **Comparing landscapes with different numbers of classes.** Richness drives SHDI. Use evenness, or the same legend.
- **No data as a class.** As in part 1: a value meaning "no data" counted as a class raises richness and diversity.

## Going further

- [Part 4](/tutorials/landscape-metrics/moving-windows/): the landscape metrics of the window around every cell,
  as maps.
- [Part 6](/tutorials/landscape-metrics/landscape-signatures/): many landscapes compared and classified by their
  metrics.
- [Neutral landscapes](/tutorials/neutral-landscapes/): the generators used above, and how to use them as null
  models.
- The operator reference: [[metrics.landscape]], [[raster.reclassify]], [[nlm.random_cluster]], [[nlm.classify]].

## References

- Castilla, G., Larkin, K., Linke, J., & Hay, G. J. (2009). The impact of thematic resolution on the patch-mosaic model of natural landscapes. *Landscape Ecology*, 24(1), 15–23. https://doi.org/10.1007/s10980-008-9310-z
- Li, H., & Reynolds, J. F. (1993). A new contagion index to quantify spatial patterns of landscapes. *Landscape Ecology*, 8(3), 155–162. https://doi.org/10.1007/BF00125347
- Li, H., & Wu, J. (2004). Use and misuse of landscape indices. *Landscape Ecology*, 19(4), 389–399. https://doi.org/10.1023/B:LAND.0000030441.15628.d6
- McGarigal, K., & Marks, B. J. (1995). *FRAGSTATS: spatial pattern analysis program for quantifying landscape structure* (General Technical Report PNW-GTR-351). USDA Forest Service, Pacific Northwest Research Station. https://doi.org/10.2737/PNW-GTR-351
- Nowosad, J., & Stepinski, T. F. (2019). Information theory as a consistent framework for quantification and classification of landscape patterns. *Landscape Ecology*, 34(9), 2091–2101. https://doi.org/10.1007/s10980-019-00830-x
- O'Neill, R. V., Krummel, J. R., Gardner, R. H., Sugihara, G., Jackson, B., DeAngelis, D. L., et al. (1988). Indices of landscape pattern. *Landscape Ecology*, 1(3), 153–162. https://doi.org/10.1007/BF00162741
- Riitters, K. H., O'Neill, R. V., Wickham, J. D., & Jones, K. B. (1996). A note on contagion indices for landscape analysis. *Landscape Ecology*, 11(4), 197–202. https://doi.org/10.1007/BF02071810
- Shannon, C. E. (1948). A mathematical theory of communication. *Bell System Technical Journal*, 27(3), 379–423. https://doi.org/10.1002/j.1538-7305.1948.tb01338.x
- Simpson, E. H. (1949). Measurement of diversity. *Nature*, 163, 688. https://doi.org/10.1038/163688a0
