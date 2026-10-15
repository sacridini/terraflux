---
title: Phylogenetic diversity, endemism and CANAPE
short: PD, PE and CANAPE
description: Add a phylogeny to the species records, map phylogenetic diversity and phylogenetic endemism, test them against randomised data, and classify the centres of endemism into neo-, palaeo-, mixed and super-endemism with CANAPE.
order: 20
level: intermediate
time: 50 min
ops: [bio.tree, bio.table, bio.spatial, bio.randomise, raster.write]
series: Biodiversity analysis
part: 2
---

Two places with ten species each are not equally diverse if the first holds ten species of one genus and the
second ten species from ten families: the second holds more of the tree of life. In this part you bring the
**phylogeny** into the analysis of [part 1](../richness-and-endemism/): you map **phylogenetic diversity** (PD),
the length of the tree's branches present in each neighbourhood, and **phylogenetic endemism** (PE), those
branches weighted by how restricted they are. Then you ask whether the patterns are more than what richness alone
would give, with Biodiverse's **randomisations**, and classify the significant places with **CANAPE** into
centres of recent and of ancient endemism. At the end you will have the maps and the tests that spatial
phylogenetics studies report.

## The idea

### Phylogenetic diversity

Faith (1992) proposed to measure the diversity of a set of species by the **total length of the branches** of the
phylogeny that connect them (to the root). Branch length stands for evolutionary history, or for the features
(genes, traits) accumulated along a lineage: a set of distantly related species spans more of the tree, and so
more features, than a set of close relatives. PD has become a standard measure in conservation, from setting
priorities between areas to valuing the evolutionarily distinct species.

### Phylogenetic endemism

Rosauer et al. (2009) combined PD with weighted endemism (part 1). Each branch of the tree has a **range**, the
cells where any of the species descending from it occurs. Phylogenetic endemism weighs every branch by the share
of its range inside the neighbourhood:

!!! science "Key idea: PD and PE"
    For the set of branches \(B\) connecting the species of a neighbourhood to the root, with lengths \(L_b\),

    \[ \mathrm{PD} = \sum_{b \in B} L_b \qquad\qquad \mathrm{PE} = \sum_{b \in B} L_b \, \frac{r_b}{R_b} \]

    where \(R_b\) is the range of branch \(b\) (the cells of all its descendant species) and \(r_b\) the part of
    it inside the neighbourhood. A long branch found only here adds its whole length to PE; a short branch, or a
    widespread one, adds little. PE is high where **long, range-restricted lineages** occur.

### Is it more than chance? Randomisations

PD and PE grow with richness: more species bring more branches. To ask whether a place has *more* (or less)
phylogenetic diversity or endemism than its richness explains, Biodiverse randomises the data many times and
compares. Its default, **rand_structured**, reshuffles which species occur in which cells while keeping the
richness of every cell and the range size of every species (Laffan & Crisp, 2003). Each index is then given a
**p-rank**: the share of randomisations in which the observed value was higher. A p-rank of 0.99 means the
observed value beat 99% of the random ones.

### CANAPE

Mishler et al. (2014) added a test that tells *why* a place is a centre of endemism. **Relative PE** (RPE) is PE on
the real tree divided by PE on a comparison tree with the same topology but equal branch lengths. If the
range-restricted branches of a place are **longer** than average (old lineages that survive in a small area),
RPE is high; if they are **shorter** (young lineages, recent speciation), RPE is low. The **categorical analysis of
neo- and palaeo-endemism** (CANAPE) then classifies each cell:

1. PE on the real or on the comparison tree is significantly high (p-rank ≥ 0.95), else **not significant**;
2. if so, RPE significantly high: **palaeo-endemism** (old lineages); significantly low: **neo-endemism**
   (recent radiations); neither: **mixed endemism**;
3. if both PEs are highly significant (p-rank ≥ 0.99): **super-endemism**.

Mishler et al. developed it on the Australian *Acacia*, and it has since been used for floras and faunas across
the world (for example the native flora of California; Thornhill et al., 2017).

## When to use it

- **Prioritising places for the evolutionary history they hold**, beyond species counts.
- **Understanding the history of a flora or fauna**: refugia of old lineages (palaeo-endemism) against cradles of
  recent speciation (neo-endemism).
- **Testing whether a pattern is real**: any Biodiverse index can be randomised in the same way.

## What you need

- **The records** of part 1: [`sites.csv`](/data/tutorials/biodiversity/richness-and-endemism/sites.csv).
- **A phylogeny** of the labels, in Newick or Nexus, with branch lengths, its tips named as the labels
  ("Genus:sp1"). Here Biodiverse's example tree,
  [`tree.nex`](/data/tutorials/biodiversity/richness-and-endemism/tree.nex). **It is not a real phylogeny**: its
  file says it was made by a Sorenson clustering of the example data. It serves to learn the method; with real
  data, use a dated or molecular phylogeny of your taxa.
- **Time**: 999 randomisations of the example took a few seconds; for thousands of cells and species, plan for
  minutes to hours.

![The example tree of the 31 species](/img/tutorials/biodiversity/phylogenetic-endemism/tree.png "Biodiverse's example tree: 31 species, branch lengths from the root to the tips of about 1 (an ultrametric tree), 21.2 in all. Made by clustering the species, it stands in for a real phylogeny here.")

## Build the model

### 1. The tree

[[bio.tree]] reads a tree as Biodiverse does: `path` (the file, or the Newick text itself), `format` (`nexus` or
`newick`, guessed from the text), and `index` (which tree of a file with several). Nexus *Translate* tables are
applied; nodes without names get Biodiverse's names (`0___`, `1___`...).

### 2. PD, PE and RPE

The [[bio.spatial]] of part 1 takes the tree in its `tree` input. Add the phylogenetic calculations to its list:
`pd` (PD, and PD_P, its share of the whole tree), `pe` (PE_WE and PE_WE_P) and `phylo_rpe2` (PHYLO_RPE2, the
relative PE, with PHYLO_RPE_NULL2 and PHYLO_RPE_DIFF2).

### 3. Randomisations and CANAPE

[[bio.randomise]] runs the same analysis on randomised data. It takes the basedata and the tree, the same
`conditions`, and the `calculations` to test; `function = "rand_structured"` (the default), `iterations = 999`
and a `rand_seed` (the same seed gives the same randomisations, and the same as Biodiverse's). Its outputs:
`table` (the observed indices), `p_rank`, `z_scores`, `comparison` (Biodiverse's counters), and, when `pe` and
`phylo_rpe2` are among the calculations, `canape` (a code per cell: 0 not significant, 1 neo, 2 palaeo, 3 mixed,
4 super); `maps` has the p-ranks and CANAPE as layers.

```tfgraph
#! caption: Records in cells of 100 km and the example tree; PD, PE and relative PE within 200 km; and the same tested against 999 randomisations, with CANAPE.
#! numbered: true
#! highlight: tree, indices, random
format = "terraflux/1"

[[nodes]]
id = "sites"
op = "bio.table"
params = { path = "sites.csv", group_columns = ["x", "y"], label_columns = ["genus", "species"], cell_sizes = [100000, 100000] }

[[nodes]]
id = "tree"
op = "bio.tree"
params = { path = "tree.nex" }
note = "Tips named as the labels, Genus:sp1..."

[[nodes]]
id = "indices"
op = "bio.spatial"
inputs = { basedata = "sites.basedata", tree = "tree.tree" }
params = { conditions = ["sp_circle (radius => 200000)"], calculations = ["richness", "endemism_whole", "pd", "pe", "phylo_rpe2"] }
note = "PD, PE and RPE within 200 km."

[[nodes]]
id = "random"
op = "bio.randomise"
inputs = { basedata = "sites.basedata", tree = "tree.tree" }
params = { conditions = ["sp_circle (radius => 200000)"], calculations = ["richness", "pd", "pe", "phylo_rpe2"], function = "rand_structured", iterations = 999, rand_seed = 2024 }
note = "999 randomisations keeping richness and ranges; p-ranks and CANAPE."

[[nodes]]
id = "save"
op = "raster.write"
inputs = { value = "random.maps" }
params = { path = "canape.tif" }
```

## Read the results

### PD and PE

```tfgallery
columns: 2
caption: "Within 200 km: phylogenetic diversity (left) and phylogenetic endemism (right). PD follows richness almost exactly; PE picks out two areas, the bottom of the arc and the upper right arm."
images:
  - { src: /img/tutorials/biodiversity/phylogenetic-endemism/pd.png, caption: "Phylogenetic diversity (PD)" }
  - { src: /img/tutorials/biodiversity/phylogenetic-endemism/pe_we.png, caption: "Phylogenetic endemism (PE)" }
```

```tfchart
type: scatter
data: /data/tutorials/biodiversity/phylogenetic-endemism/pd-vs-richness.csv
x: richness
y: [pd]
labels: { pd: "PD" }
colors: { pd: "#3b8a5a" }
xlabel: Species within 200 km
ylabel: Phylogenetic diversity
height: 260
caption: "PD against richness for the 127 cells (within 200 km): rank correlation 0.99. With this tree, PD adds almost nothing to richness; it is the endemism and the randomisations that bring new information."
```

PD is nearly a function of richness here (rank correlation 0.99), largest (11.9) where richness is largest (17
species). This is common, and the reason PD alone is rarely enough. PE follows weighted endemism closely (0.96)
but not exactly: it rises highest (3.77) in a cell of the upper right arm, where long branches have small ranges.

### Against chance

![The p-rank of PE against 999 randomisations](/img/tutorials/biodiversity/phylogenetic-endemism/prank_pe.png "The p-rank of PE: red where PE is higher than in 95% (or 99%) of the randomisations, blue where it is lower. The upper right arm and the left tip have more phylogenetic endemism than their richness explains; the lower tail has less.")

The randomisations change the picture. The bottom of the arc, high in PE, is **not significant**: its PE is what
its many species would give anyway. The upper right arm is significantly high (15 cells at p ≥ 0.95, 2 at
p ≥ 0.99), and so are a few cells at the left tip. The lower tail of the arc is significantly **low** (19 cells at
p ≤ 0.01): its species are widespread ones, and so are their branches.

### CANAPE

![CANAPE classes of the cells](/img/tutorials/biodiversity/phylogenetic-endemism/canape.png "CANAPE: 35 of the 127 cells are centres of endemism. 28 mixed and 2 super-endemism on the upper right arm (and one mixed cell at the bottom), 5 palaeo-endemism at the left tip and on the left arm. No neo-endemism.")

| CANAPE class | Cells |
|---|---|
| Not significant | 92 |
| Neo-endemism | 0 |
| Palaeo-endemism | 5 |
| Mixed endemism | 28 |
| Super-endemism | 2 |

Read with care, given the artificial tree, the story is one of two centres: a large **mixed** centre on the upper
right arm, with two cells of **super-endemism** at its heart, where short and long range-restricted branches occur
together; and a small **palaeo-endemic** centre on the left, where the restricted branches are long, the pattern of
old lineages persisting in a small area. CANAPE has more cells (35) than the p-rank map of PE alone (17 above
0.95), because its first test accepts significance on either tree. These counts are those landschaft's
documentation reports for the same data and seed, which Biodiverse also gives.

!!! paper "In the literature"
    Report the phylogeny (source, dating, how tips were matched to the records), the spatial condition, the
    randomisation (its function, iterations and seed) and the significance thresholds, the PD and PE maps, the
    CANAPE map with its counts, and how many tips of the tree had no record. Mishler et al. (2014) and Thornhill
    et al. (2017) are good models.

## Settings that change the answer

- **The phylogeny.** Branch lengths are what PD and PE measure: a dated tree, a molecular one, or one with equal
  lengths give different maps. Missing species bias PD downwards where they occur.
- **The randomisation.** `rand_structured` keeps richness and ranges; the spatially structured forms
  (`rand_spatially_structured`, `rand_diffusion`, `rand_random_walk`) also keep ranges contiguous, a stricter null
  model that usually makes fewer cells significant; `rand_independent_swaps` and `rand_curveball` are swap algorithms.
- **Iterations**: 999 is the minimum for a p-rank of 0.99 to mean something; more make the edges of significance
  stable.
- **The neighbourhood and the cell size**, as in part 1: CANAPE at 100 km cells within 200 km is one scale of many.

## Common pitfalls

- **Tips that do not match labels.** A label absent from the tree is ignored by phylogenetic indices, silently
  lowering PD and PE. Check that every label is a tip (same spelling, same separator).
- **Reading PD as more than richness.** With many trees PD correlates strongly with richness. Look at the
  randomisations (or at PD's standardised effect) before claiming high phylogenetic diversity.
- **Too few iterations**, or comparing p-ranks of runs with different seeds and conditions.
- **Multiple testing**: with 127 cells and a threshold of 0.05, about six cells are expected significant by chance
  alone in each tail. Prefer clusters of significant cells to single ones.

## Going further

- [Part 1: richness and endemism](../richness-and-endemism/).
- [Part 3: regions from species composition](../regions/), including phylogenetic dissimilarity
  (`PHYLO_SORENSON`).
- [Spatial prioritization](/tutorials/spatial-prioritization/systematic-conservation-planning/): taking PE
  surfaces into a conservation plan.
- The operators: [[bio.tree]], [[bio.spatial]], [[bio.randomise]], [[bio.matrix]], [[bio.properties]].

## References

- Faith, D. P. (1992). Conservation evaluation and phylogenetic diversity. *Biological Conservation*, 61(1), 1–10. https://doi.org/10.1016/0006-3207(92)91201-3
- Laffan, S. W., & Crisp, M. D. (2003). Assessing endemism at multiple spatial scales, with an example from the Australian vascular flora. *Journal of Biogeography*, 30(4), 511–520. https://doi.org/10.1046/j.1365-2699.2003.00875.x
- Laffan, S. W., Lubarsky, E., & Rosauer, D. F. (2010). Biodiverse, a tool for the spatial analysis of biological and related diversity. *Ecography*, 33(4), 643–647. https://doi.org/10.1111/j.1600-0587.2010.06237.x
- Mishler, B. D., Knerr, N., González-Orozco, C. E., Thornhill, A. H., Laffan, S. W., & Miller, J. T. (2014). Phylogenetic measures of biodiversity and neo- and paleo-endemism in Australian *Acacia*. *Nature Communications*, 5, 4473. https://doi.org/10.1038/ncomms5473
- Rosauer, D., Laffan, S. W., Crisp, M. D., Donnellan, S. C., & Cook, L. G. (2009). Phylogenetic endemism: a new approach for identifying geographical concentrations of evolutionary history. *Molecular Ecology*, 18(19), 4061–4072. https://doi.org/10.1111/j.1365-294X.2009.04311.x
- Thornhill, A. H., Baldwin, B. G., Freyman, W. A., et al. (2017). Spatial phylogenetics of the native California flora. *BMC Biology*, 15(1). https://doi.org/10.1186/s12915-017-0435-x
