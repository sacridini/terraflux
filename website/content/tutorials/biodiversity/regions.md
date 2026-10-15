---
title: Regions from species composition
short: Regions (cluster analysis)
description: Group cells with similar species into biogeographic regions with Biodiverse's cluster analysis, read its dendrogram, choose the number of regions, and know what the dissimilarity index and the linkage change.
order: 30
level: intermediate
time: 35 min
ops: [bio.table, bio.cluster, bio.region_grower, raster.write]
series: Biodiversity analysis
part: 3
---

Parts [1](../richness-and-endemism/) and [2](../phylogenetic-endemism/) measured each place on its own. Another
classic question compares places with each other: **which places have similar species, and where are the
boundaries between groups of them?** Biogeographers answer it by **regionalisation**, clustering cells by their
species composition into regions (bioregions). In this last part you cluster the cells of Biodiverse's example
data with Biodiverse's cluster analysis, read the dendrogram, cut it into three to eight regions, and see what the
choice of index and linkage does. At the end you will be able to draw regions from records and defend how many
you drew.

## The idea

### Dissimilarity between cells

Two cells are similar if they share most of their species. The **Sorenson** dissimilarity (Biodiverse's default)
measures it from three counts: \(a\), the species the two cells share, and \(b\) and \(c\), those found in only one
of them.

!!! science "Key idea: Sorenson dissimilarity and average linkage"
    \[ \text{Sorenson} = 1 - \frac{2a}{2a + b + c} \]

    It is 0 for cells with the same species and 1 for cells with none in common. **Jaccard**,
    \(1 - a/(a + b + c)\), is a monotonic function of it (\(J = 2S/(1+S)\)), so the two rank pairs of cells
    alike.

    **Hierarchical clustering** starts with each cell alone and repeatedly joins the two most similar clusters,
    until one remains. The **linkage** says how similar two clusters are: with **average linkage** (UPGMA,
    Biodiverse's default), the mean dissimilarity of all pairs of cells between them; with *minimum* linkage, the
    most similar pair (long, straggly clusters); with *maximum*, the least similar pair (compact ones).

The result is a **dendrogram**: a tree whose leaves are the cells and whose joins are at the height of the
dissimilarity at which two clusters were merged. Cutting it at a height gives regions; cutting it to keep \(k\)
branches gives \(k\) regions. Kreft and Jetz (2010) proposed a framework for this kind of regionalisation from
species distributions (the choice of the index, the clustering method and the number of regions), and
phylogenetic versions (`PHYLO_SORENSON`) group cells by shared branches rather than shared species.

## When to use it

- **Delineating bioregions** for reporting, for representative reserve systems, or as units for further analysis.
- **Finding transition zones**: where cells of different regions meet, and where the dendrogram joins them only
  high up.
- **Describing beta diversity**: how much composition changes across the study area.

What it is not: a test. Clusters always form; whether they are biologically meaningful depends on the data, the
index and the cut. Compare several choices, and check the regions against independent knowledge.

## What you need

- **The basedata** of part 1, from [`sites.csv`](/data/tutorials/biodiversity/richness-and-endemism/sites.csv),
  in cells of 100 km: 127 cells and 31 species.
- **A dissimilarity index** (`SORENSON`, `JACCARD`, `S2`, `BRAY_CURTIS` for abundances, `PHYLO_SORENSON` with a
  tree), a **linkage**, and a **number of regions** or a way to choose it.

## Build the model

[[bio.cluster]] (library subject **Biodiversity analysis**) takes the basedata (and a `tree` for phylogenetic
indices, a `matrix` for indices of a label matrix). Its settings:

- `index = "SORENSON"`: the dissimilarity;
- `linkage = "link_average"`: or `link_average_unweighted`, `link_minimum`, `link_maximum`, `link_recalculate`;
- `k = 5`: cut the dendrogram into five clusters, giving the `clusters` table (each cell's cluster) and a `map`;
- `conditions`: spatial conditions limiting which cells may be joined, for spatially constrained regions;
- `calculations`: Biodiverse indices computed for the cells of every node of the dendrogram (the `table`
  output), for example `["richness"]`;
- `seed` and `tie_breaker`: how ties between equally similar pairs are broken (Biodiverse's random generator, so
  the same seed gives Biodiverse's dendrogram).

Its outputs: `tree` (the dendrogram, as a tree), `nodes` (a row per node: its parent, its length, Biodiverse's
values), `table`, and with `k`, `clusters` and `map`.

```tfgraph
#! caption: The cells of 100 km clustered by their species (Sorenson dissimilarity, average linkage), cut into five regions and written as a raster.
#! numbered: true
#! highlight: cluster
format = "terraflux/1"

[[nodes]]
id = "sites"
op = "bio.table"
params = { path = "sites.csv", group_columns = ["x", "y"], label_columns = ["genus", "species"], cell_sizes = [100000, 100000] }

[[nodes]]
id = "cluster"
op = "bio.cluster"
inputs = { basedata = "sites.basedata" }
params = { index = "SORENSON", linkage = "link_average", k = 5, calculations = ["richness"] }
note = "Cells joined by shared species; the dendrogram cut into five regions."

[[nodes]]
id = "save"
op = "raster.write"
inputs = { value = "cluster.map" }
params = { path = "regions.tif" }
```

!!! note "Calculations are optional"
    Without `calculations`, [[bio.cluster]] gives the dendrogram and the regions but no `table`; a cheap one such as
    `["richness"]` adds the richness of each node's cells, often worth reading beside the regions.

The clustering of 127 cells took well under a second. Cutting into another number of regions means another `k`;
the dendrogram itself does not change.

## Read the results

### The dendrogram

![The dendrogram of the 127 cells, its leaves coloured by the five regions](/img/tutorials/biodiversity/regions/dendrogram.png "The dendrogram: each leaf is a cell, each join at the Sorenson dissimilarity at which two clusters merged (average linkage). Coloured squares: the five regions of the cut at k = 5. Many joins happen at 0.33 and below (cells sharing most of their species); the five regions join each other only above 0.85.")

Read a dendrogram from the bottom. Cells with nearly the same species join low (dissimilarity 0.1 to 0.3); groups
join higher; the last joins, near 0.9, are between groups that share few species. A long vertical gap between the
joins of a group and the join above it is a well-separated region. The first division of the tree (at the top)
separates the left arm and the bottom of the arc (light and dark blue leaves) from the right arm (green, pink and
orange). The colours are those of the maps below, which keep a region's colour from one cut to the next.

### How many regions?

Move the slider: each frame cuts the same dendrogram into more regions.

```tfslider
label: Number of regions (k)
caption: "The cells cut into 3 to 8 regions (Sorenson, average linkage); a region keeps its colour when a cut splits off part of it. The arc splits first into its left arm, its right arm and the rest; then the right arm divides along its length, and small groups break off."
frames:
  - { value: "3", src: /img/tutorials/biodiversity/regions/clusters_3.png, stats: { "Regions": "3", "Cells per region": "73, 30, 24" } }
  - { value: "4", src: /img/tutorials/biodiversity/regions/clusters_4.png, stats: { "Regions": "4", "Cells per region": "73, 24, 17, 13" } }
  - { value: "5", src: /img/tutorials/biodiversity/regions/clusters_5.png, stats: { "Regions": "5", "Cells per region": "49, 24, 24, 17, 13" } }
  - { value: "6", src: /img/tutorials/biodiversity/regions/clusters_6.png, stats: { "Regions": "6", "Cells per region": "49, 24, 21, 17, 13, 3" } }
  - { value: "8", src: /img/tutorials/biodiversity/regions/clusters_8.png, stats: { "Regions": "8", "Cells per region": "43, 21, 19, 17, 13, 6, 5, 3" } }
```

With three regions the data fall into the left arm with the bottom of the arc and its lower tail (73 cells), the
upper right arm with the few cells at the top (30), and the middle of the right arm (24). At four, the upper group
divides (17 and 13 cells); at five, the left arm (24 cells) separates from the bottom of the arc (49). From six on, the new regions are small (3 to 6 cells): splits of a few cells, often on the edges of the data,
that are as likely to reflect sampling as biogeography.

Choosing \(k\) is a judgement: look for cuts where the dendrogram has long branches (the regions are well
separated), where regions are spatially coherent, and where they make sense to someone who knows the taxa. Kreft
and Jetz (2010) review statistics that help (the share of dissimilarity explained, silhouettes).

### Comparing indices

Clustering with `JACCARD` instead gives exactly the same five regions here (the cells are joined in the same
order, since Jaccard is a monotonic function of Sorenson and the linkage averages them similarly). Indices that are
*not* monotonic transformations change the regions: `S2` (Simpson's), which ignores differences in richness and
looks only at turnover, and `BRAY_CURTIS`, which uses abundances. Comparing two or three indices is the simplest
check of how robust the regions are.

!!! paper "In the literature"
    Report the cell size, the index, the linkage, the number of regions and how it was chosen, and show the
    dendrogram beside the map. Laffan et al. (2016) show how range-weighted indices of turnover sharpen the
    boundaries between regions; Kreft and Jetz (2010) give the general framework.

## Settings that change the answer

- **The index**: presence (Sorenson, Jaccard), turnover only (S2), abundance (Bray-Curtis), phylogenetic
  (PHYLO_SORENSON). Choose by what a region should mean.
- **The linkage**: average (the usual), minimum (chains), maximum (compact clusters).
- **The cell size**: larger cells hold more species and make cells more alike; very small cells, with one or two
  species, make dissimilarities of 0 or 1 and unstable clusters.
- **Spatial constraints**: with `conditions`, only neighbouring cells may join, so regions are contiguous. This
  changes the method's meaning (regions are then partly defined by geography), so report it.
- **Ties**: with few species per cell, many pairs share the same dissimilarity, and the order of joins depends on
  how ties are broken (`seed`, `tie_breaker`). Run two seeds and compare.

## Common pitfalls

- **Species-poor cells.** A cell with one species is dissimilar to almost everything and forms odd clusters. Drop
  cells with fewer than a few species (`definition_query`, a spatial condition that groups must pass, such as
  `sp_richness_greater_than (threshold => 2)`),
  or use larger cells.
- **Uneven sampling.** Poorly sampled cells look different because they lack species, not because they have
  others. S2 is less sensitive to this than Sorenson.
- **Reading regions as discrete.** Biotas change gradually as often as abruptly. The height at which regions join
  says how distinct they are.
- **Too many regions.** Small regions of a few cells at the data's edges rarely mean much.

## Going further

- [Part 1](../richness-and-endemism/) and [part 2](../phylogenetic-endemism/) of the series.
- [[bio.region_grower]]: Biodiverse's region grower, which joins cells so as to maximise an index of the joined
  regions (richness, PE...) instead of their similarity.
- [Change detection](/tutorials/change-detection/comparing-maps/): comparing two maps of regions.
- The operators: [[bio.cluster]], [[bio.region_grower]], [[bio.table]], [[bio.tree]], [[bio.matrix]].

## References

- Kreft, H., & Jetz, W. (2010). A framework for delineating biogeographical regions based on species distributions. *Journal of Biogeography*, 37(11), 2029–2053. https://doi.org/10.1111/j.1365-2699.2010.02375.x
- Laffan, S. W., Lubarsky, E., & Rosauer, D. F. (2010). Biodiverse, a tool for the spatial analysis of biological and related diversity. *Ecography*, 33(4), 643–647. https://doi.org/10.1111/j.1600-0587.2010.06237.x
- Laffan, S. W., Rosauer, D. F., Di Virgilio, G., Miller, J. T., González-Orozco, C. E., Knerr, N., Thornhill, A. H., & Mishler, B. D. (2016). Range-weighted metrics of species and phylogenetic turnover can better resolve biogeographic transition zones. *Methods in Ecology and Evolution*, 7(5), 580–588. https://doi.org/10.1111/2041-210X.12513
