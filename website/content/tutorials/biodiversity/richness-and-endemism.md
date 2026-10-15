---
title: Richness and endemism with Biodiverse
short: Richness and endemism
description: Turn species records into Biodiverse's basedata of cells and species, define neighbourhoods with spatial conditions, and map species richness, weighted endemism and corrected weighted endemism, seeing how the neighbourhood and the cell size change them.
order: 10
level: beginner
time: 45 min
ops: [bio.table, bio.spatial, bio.occurrences, bio.points, bio.vector, bio.raster, bio.hex, bio.h3, raster.write]
series: Biodiversity analysis
part: 1
---

Where are the places with the most species, and the places with species found nowhere else? Those two questions,
**richness** and **endemism**, are where most conservation assessments of biodiversity start, and Biodiverse
(Laffan et al., 2010) is the tool many biogeographers use to answer them from species records. Terraflux's `bio`
operators are a port of Biodiverse that give the same numbers. In this first part of three you load Biodiverse's
example records into a **basedata** (a table of cells and species), compute richness and two endemism indices
in each cell and in moving neighbourhoods, and see how the neighbourhood and the size of the cells change the
picture. At the end you will be able to map richness and endemism from any table of records, and know how to read
them.

## The idea

### Groups, labels and the basedata

Biodiverse has its own vocabulary, which Terraflux keeps:

- a **label** is what is counted: a species, a genus, or any taxon (here "Genus:sp1" to "Genus:sp31");
- a **group** is where it is counted: usually a square cell of a grid, identified by its centre;
- the **basedata** is the table of which labels occur in which groups, with their counts (sample counts).

Records with coordinates become groups by **snapping** each record to the cell that holds it: with cells of 100 km,
every record falls into one 100 km square, and every square with at least one record becomes a group.

### Neighbourhoods: spatial conditions

Biodiverse computes its indices for every group from the labels of its **neighbourhood**, described by a
*spatial condition*, a small expression in Biodiverse's own (Perl) syntax:

| Condition | Neighbourhood |
|---|---|
| `sp_self_only()` | the cell alone |
| `sp_circle (radius => 200000)` | the cells whose centres are within 200 km |
| `sp_block (size => 500000)` | the cells in the same 500 km block |
| `$D <= 250000` | the cells within 250 km (`$D` is the distance) |
| `sp_select_all()` | every cell: the whole dataset |

A moving neighbourhood smooths the map and answers a different question from the cell alone: "how many species
live in this region of 200 km?" rather than "how many were recorded in this square?". It also compensates in part
for uneven sampling between neighbouring cells.

### Richness and endemism

!!! science "Key idea: weighted endemism"
    **Richness** is the number of labels in the neighbourhood. **Weighted endemism** (WE; Crisp et al., 2001)
    weighs each species by how much of its range falls in the neighbourhood:

    \[ \mathrm{WE} = \sum_{s \in \text{neighbourhood}} \frac{r_s}{R_s} \]

    where \(R_s\) is the number of cells where species \(s\) occurs in the whole dataset (its range) and
    \(r_s\) the number of those cells in the neighbourhood. A species found only there counts 1; a species of
    100 cells, present in 2 of them, counts 0.02.

    **Corrected weighted endemism** (CWE) divides by richness, \(\mathrm{CWE} = \mathrm{WE}/\text{richness}\):
    the mean endemism of the species present, which removes the effect of richness itself.

Endemism matters to conservation because species with small ranges are the most exposed to loss: a centre of
endemism holds species that cannot be protected anywhere else. WE and CWE were developed to find such centres in
the Australian flora (Crisp et al., 2001), and Laffan and Crisp (2003) showed how strongly they depend on the scale
of analysis, which is one reason Biodiverse lets neighbourhoods vary.

## When to use it

- **Mapping diversity and endemism** from atlas data, herbarium and museum collections, or GBIF downloads.
- **Finding centres of endemism** for protected area planning; comparing the endemism within a reserve network
  with the endemism of the region (Laffan et al., 2013).
- **Preparing phylogenetic analyses** ([part 2](../phylogenetic-endemism/)) and regionalisations
  ([part 3](../regions/)), which use the same basedata and conditions.

What it is *not*: richness from records counts where species were *found*, which depends on where people looked.
To estimate where species *are*, model their distributions first ([species
distributions](/tutorials/species-distributions/ideas/)) and import the predicted presences as a basedata with
[[bio.raster]].

## What you need

- **Species records with coordinates**: Biodiverse's example data, [`sites.csv`](/data/tutorials/biodiversity/richness-and-endemism/sites.csv)
  (1,795 records, 72 KB), with columns `num` (a record number), `genus`, `species`, `x` and `y` (coordinates in
  metres). It has 31 species of an unnamed genus, which landschaft's documentation places in eastern Australia.
  Use it to learn; the numbers have no biological meaning beyond this example.
- **A coordinate system in metres** with equal areas, so that cells are equal: reproject records in degrees first
  (`to_crs` of [[bio.occurrences]]), or use cells in degrees knowing that they shrink towards the poles.
- **A cell size**: here 100 km. Choose it from the precision of the records (the cells should be larger than the
  records' uncertainty) and from the scale of the question.

Other sources become a basedata with other operators: [[bio.occurrences]] reads GBIF downloads and Darwin Core
archives (with filters on uncertainty, basis of record and GBIF's issues), [[bio.points]] and [[bio.vector]] read
points, lines and polygons (range maps: every cell a polygon touches), [[bio.raster]] reads rasters (one label per
layer, for example thresholded distribution models), and [[bio.hex]] and [[bio.h3]] count records in hexagons
instead of squares.

![The 1,795 records over the 127 cells of 100 km that they fall in](/img/tutorials/biodiversity/richness-and-endemism/records.png "Biodiverse's example data: the records (dots) and the 127 cells of 100 km with at least one record (green, darker with more species). The records run in an arc, densest at its bottom.")

## Build the model

### 1. The basedata

[[bio.table]] (library subject **Biodiversity analysis**) reads a delimited text file as Biodiverse's import
does. Its settings:

- `path`: the file, `sites.csv`;
- `group_columns = ["x", "y"]`: the columns that place a record;
- `cell_sizes = [100000, 100000]`: the size of the cells along each group column (in metres here). 0 keeps the
  coordinates as they are; a negative size treats a column as text (groups by name, such as a region);
- `label_columns = ["genus", "species"]`: the columns that name a label, joined with `:` ("Genus:sp1");
- `count_columns`: columns of counts, summed (default 1 per record). `binarise = true` makes every count 1.

Its output, `basedata`, is the groups × labels table that every `bio` operator reads.

### 2. The spatial analysis

[[bio.spatial]] runs Biodiverse's calculations over the neighbourhood of every group. Connect `sites.basedata`
to its `basedata`, and set:

- `conditions = ["sp_circle (radius => 200000)"]`: one condition (or two, for the indices that compare two
  neighbourhoods);
- `calculations = ["richness", "endemism_whole"]`: Biodiverse's calculations by name (with or without `calc_`).
  `richness` gives `RICHNESS_ALL` (and `RICHNESS_SET1`); `endemism_whole` gives `ENDW_WE` (weighted endemism),
  `ENDW_CWE` (corrected), `ENDW_RICHNESS` and `ENDW_SINGLE`, with ranges measured over the whole dataset
  (`endemism_central` measures them differently when two neighbourhoods are used).

Its outputs: `table`, a row per group with its coordinates and every index; `maps`, a stack with an index per
layer when the groups are cells of two axes (as here); `lists`, the list indices.

```tfgraph
#! caption: Records into cells of 100 km, then richness and endemism in each cell alone and within 200 km of it, the maps written to GeoTIFFs.
#! numbered: true
#! highlight: sites, cells, neighbourhood
format = "terraflux/1"

[[nodes]]
id = "sites"
op = "bio.table"
params = { path = "sites.csv", group_columns = ["x", "y"], label_columns = ["genus", "species"], cell_sizes = [100000, 100000] }
note = "The records snapped to cells of 100 km."

[[nodes]]
id = "cells"
op = "bio.spatial"
inputs = { basedata = "sites.basedata" }
params = { conditions = ["sp_self_only()"], calculations = ["richness", "endemism_whole"] }
note = "Each cell alone."

[[nodes]]
id = "neighbourhood"
op = "bio.spatial"
inputs = { basedata = "sites.basedata" }
params = { conditions = ["sp_circle (radius => 200000)"], calculations = ["richness", "endemism_whole"] }
note = "The cells within 200 km."

[[nodes]]
id = "save"
op = "raster.write"
inputs = { value = "neighbourhood.maps" }
params = { path = "indices_200km.tif" }
note = "A band per index, named after it."
```

The run takes a fraction of a second. [[raster.write]] writes the `maps` stack as a GeoTIFF with one band per
index, each named after it (`RICHNESS_ALL`, `ENDW_WE`...), ready for a GIS.

=== "Application"

    Select `neighbourhood` and press ++ctrl+enter++: its result shows under the canvas, and a double-click opens
    the maps in a tab of their own.

=== "Command line"

    ```bash
    terraflux run endemism.toml
    ```

    writes `indices_200km.tif` next to the model; add `[outputs.indices] from = "neighbourhood.table"` to the
    model to get the table as a CSV with `--out`.

## Read the results

### Ranges: the raw material of endemism

```tfchart
type: bar
data: /data/tutorials/biodiversity/richness-and-endemism/range-sizes.csv
x: cells
y: [species]
labels: { species: "Species" }
colors: { species: "#3b8a5a" }
xlabel: Range size (cells of 100 km)
ylabel: Species
height: 260
caption: "The range sizes of the 31 species, in cells of 100 km. Four occur in one or two cells; one in 49. Endemism indices give the narrow-ranged species most of their weight."
```

Most species are found in a few cells: 13 of the 31 in five cells or fewer, one in 49. This skew is typical of
real data, and it is why weighted endemism differs from richness: a cell with three narrow-ranged species can
outweigh one with ten widespread ones.

### Richness and weighted endemism

Move the slider over the neighbourhood: the cell alone, then circles of 100, 200 and 300 km. The maps are weighted
endemism on one scale for all frames; the numbers are richness and endemism.

```tfslider
label: Neighbourhood
caption: "Weighted endemism of every cell, for its own records and for the cells within 100, 200 and 300 km of it, on the same scale. Larger neighbourhoods smooth the map and raise the values; the two concentrations, at the bottom of the arc and on its right arm, stand out from 200 km."
frames:
  - { value: "Cell alone", src: /img/tutorials/biodiversity/richness-and-endemism/we_self.png, stats: { "Condition": "sp_self_only()", "Richness (max, mean)": "11, 2.9", "WE (max)": "2.34", "CWE (max)": "0.34" } }
  - { value: "100 km", src: /img/tutorials/biodiversity/richness-and-endemism/we_100.png, stats: { "Condition": "sp_circle 100 km", "Richness (max, mean)": "15, 6.0", "WE (max)": "3.04", "CWE (max)": "0.48" } }
  - { value: "200 km", src: /img/tutorials/biodiversity/richness-and-endemism/we_200.png, stats: { "Condition": "sp_circle 200 km", "Richness (max, mean)": "17, 8.6", "WE (max)": "5.37", "CWE (max)": "0.49" } }
  - { value: "300 km", src: /img/tutorials/biodiversity/richness-and-endemism/we_300.png, stats: { "Condition": "sp_circle 300 km", "Richness (max, mean)": "18, 10.8", "WE (max)": "8.29", "CWE (max)": "0.57" } }
```

```tfgallery
columns: 2
caption: "Richness in each cell alone (left) and within 200 km (right), on the same scale. The cell map is patchy (it reflects where records were made); the neighbourhood map shows the gradient: most species at the bottom of the arc and on its right arm."
images:
  - { src: /img/tutorials/biodiversity/richness-and-endemism/richness_self.png, caption: "Richness, cell alone" }
  - { src: /img/tutorials/biodiversity/richness-and-endemism/richness_200.png, caption: "Richness, 200 km" }
```

How to read them:

- **Richness of the cell alone is mostly sampling.** A cell has 2.9 species on average and 11 at most; its
  neighbours often have far fewer, because fewer records were made there. Within 200 km, richness rises to 8.6
  on average and the map becomes a gradient.
- **Weighted endemism follows richness, but not entirely.** Their rank correlation is 0.80 for the cells alone
  and 0.88 within 200 km: species-rich places tend to have more narrow-ranged species simply because they have
  more species. The cell with the highest WE on its own (2.34, with 7 species) is at the bottom of the arc; within
  200 km the maximum (5.37, with 16 species) is also at the bottom of the arc, a little to the right.
- **Corrected weighted endemism removes richness.** CWE correlates only weakly with richness (0.27 alone, 0.32
  within 200 km), and its highest values within 200 km are on the upper right arm (0.49, with 7 species): fewer
  species, but more of them narrow-ranged.

```tfgallery
columns: 2
caption: "Within 200 km: weighted endemism (left) and corrected weighted endemism (right). WE is highest where many species overlap; CWE where the species present have, on average, the smallest ranges."
images:
  - { src: /img/tutorials/biodiversity/richness-and-endemism/we_200.png, caption: "Weighted endemism (WE)" }
  - { src: /img/tutorials/biodiversity/richness-and-endemism/cwe_200.png, caption: "Corrected weighted endemism (CWE)" }
```

!!! paper "In the literature"
    Report the data source and its filters, the coordinate system, the cell size, the spatial condition, and the
    index names as Biodiverse gives them (ENDW_WE, ENDW_CWE). Crisp et al. (2001) and Laffan and Crisp (2003) are
    the classic examples of mapping weighted endemism; González-Orozco et al. (2011) of richness and endemism of a
    genus across a continent.

## Settings that change the answer

### The cell size

```tfslider
label: Cell size
caption: "Richness of each cell alone with cells of 50, 100 and 200 km (each map on its own scale). Larger cells hold more records and more species, and fewer cells are empty between records."
frames:
  - { value: "50 km", src: /img/tutorials/biodiversity/richness-and-endemism/cells_50.png, stats: { "Cells with records": "277", "Richness (max, mean)": "8, 2.0", "WE (max)": "1.70" } }
  - { value: "100 km", src: /img/tutorials/biodiversity/richness-and-endemism/cells_100.png, stats: { "Cells with records": "127", "Richness (max, mean)": "11, 2.9", "WE (max)": "2.34" } }
  - { value: "200 km", src: /img/tutorials/biodiversity/richness-and-endemism/cells_200.png, stats: { "Cells with records": "51", "Richness (max, mean)": "13, 4.1", "WE (max)": "2.45" } }
```

The cell size changes every number: mean richness doubles from 50 to 200 km, and the ranges, measured in cells,
change with it, so weighted endemism is not comparable between cell sizes. Laffan and Crisp (2003) recommend
analysing several scales. A neighbourhood larger than the cell (as in the first slider) is another way to vary
the scale while keeping the cells' precision.

### The others

- **The spatial condition**: its size, and its shape (circles, blocks, annuli for "what is around but not here").
- **`endemism_whole` or `endemism_central`**: ranges over the whole dataset or relative to the neighbourhoods.
- **Counts or presence**: with `binarise`, a cell with 50 records of a species counts the same as one with 1. The
  indices of this part use presence; abundance-based indices (`abc_counts`...) use the counts.
- **The data's extent**: ranges are measured within the dataset. A species that is narrow-ranged in your study
  area may be widespread beyond it; restrict "endemism" to what the data can show.

## Common pitfalls

- **Sampling effort.** Richness of records rises with the number of records. Look at the number of records per
  cell before interpreting richness, and compare with a randomisation ([part 2](../phylogenetic-endemism/)).
- **Coordinates in degrees** with a cell size meant in metres (or the reverse): every record snaps to one cell, or
  each to its own.
- **Duplicates and errors**: records of the same species at the same place inflate counts (but not presence);
  misidentified or misplaced records create false narrow-range "endemics". [[bio.occurrences]] drops duplicates
  and records with GBIF's issues; check the remaining outliers by hand.
- **Edge effects**: neighbourhoods at the edge of the data are smaller and have lower values.
- **CWE in species-poor cells**: one narrow-ranged species alone makes CWE high. Within 200 km, a cell with a single
  species has a CWE of 0.43, among the highest. Read CWE beside richness.

## Going further

- [Part 2: phylogenetic diversity, endemism and CANAPE](../phylogenetic-endemism/): the tree of life in the
  same cells, and tests against chance.
- [Part 3: regions from species composition](../regions/): cluster analysis of the cells.
- [Species distributions](/tutorials/species-distributions/occurrence-data/): cleaning GBIF records.
- The operators: [[bio.table]], [[bio.occurrences]], [[bio.points]], [[bio.vector]], [[bio.raster]],
  [[bio.hex]], [[bio.h3]], [[bio.properties]], [[bio.spatial]].

## References

- Crisp, M. D., Laffan, S., Linder, H. P., & Monro, A. (2001). Endemism in the Australian flora. *Journal of Biogeography*, 28(2), 183–198. https://doi.org/10.1046/j.1365-2699.2001.00524.x
- González-Orozco, C. E., Laffan, S. W., & Miller, J. T. (2011). Spatial distribution of species richness and endemism of the genus *Acacia* in Australia. *Australian Journal of Botany*, 59(7), 601–609. https://doi.org/10.1071/BT11112
- Laffan, S. W., & Crisp, M. D. (2003). Assessing endemism at multiple spatial scales, with an example from the Australian vascular flora. *Journal of Biogeography*, 30(4), 511–520. https://doi.org/10.1046/j.1365-2699.2003.00875.x
- Laffan, S. W., Lubarsky, E., & Rosauer, D. F. (2010). Biodiverse, a tool for the spatial analysis of biological and related diversity. *Ecography*, 33(4), 643–647. https://doi.org/10.1111/j.1600-0587.2010.06237.x
- Laffan, S. W., Ramp, D., & Roger, E. (2013). Using endemism to assess representation of protected areas – the family Myrtaceae in the Greater Blue Mountains World Heritage Area. *Journal of Biogeography*, 40(3), 570–578. https://doi.org/10.1111/jbi.12001
