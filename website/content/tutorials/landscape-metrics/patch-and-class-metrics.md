---
title: Patch and class metrics
short: Patch and class
description: "Measure every patch of a land cover (area, shape, core, isolation) and every class (amount, subdivision, edge, aggregation), learn what each family of metrics means for species, and see how the edge depth and the aggregation of habitat move them."
order: 20
level: beginner
time: 50 min
template: Patches and their shapes
ops: [metrics.patch, metrics.class, raster.read]
series: Landscape metrics
part: 2
---

[Part 1](/tutorials/landscape-metrics/what-are-landscape-metrics/) introduced the three levels of metrics. This part
works at the first two. At the **patch** level, every wood, field and pond of a map gets its own numbers: how large
it is, how convoluted its outline, how much of it is far from an edge, how far it is from the next patch of its kind.
At the **class** level, those numbers are summarised for each land cover and joined by metrics of the class as a
whole: how much of the map it covers, how many pieces it is broken into, how much edge it has, how aggregated it is.
We run them on the land cover of Augusta, Georgia (the map of part 1), compute a few by hand on three toy patches,
and move the two things that change them most: the depth of the edge, and the aggregation of the habitat. At the end
you will be able to choose a small set of patch and class metrics for a question and explain what each one says.

## The idea

A patch is the unit an ecologist often thinks in: a wood, a wetland, a meadow. Its size matters for the species that
need large areas; its shape for those that avoid edges; its isolation for those that must move between patches. The
patch metrics measure these three things, each in several ways, and the class metrics summarise them over all the
patches of a class (McGarigal & Marks, 1995).

The metrics fall into families, which landscapemetrics and Terraflux follow (`type`):

| Family | What it measures | Patch level | Class level |
|---|---|---|---|
| Area and edge | size, amount, edge | `area`, `perim`, `gyrate` | `ca`, `pland`, `np`, `lpi`, `te`, `ed`, `area_mn`... |
| Shape | how convoluted | `shape`, `frac`, `para`, `circle`, `contig` | their means, sd, cv |
| Core area | interior beyond an edge depth | `core`, `cai`, `ncore` | `tca`, `cpland`, `ndca`, `cai_mn`... |
| Aggregation | isolation, subdivision, clumping | `enn` | `ai`, `clumpy`, `cohesion`, `division`, `mesh`, `pladj`, `enn_mn`... |

### Three patches of the same area

The clearest way to see what shape and core metrics do is three patches of exactly the same area.

![Three patches of 24 cells](/img/tutorials/landscape-metrics/patch-and-class-metrics/shapes.png "Three patches of 24 cells of 30 m (2.16 ha) each: a compact block, a strip two cells wide and a spiral, with their metrics computed by metrics.patch. The longer the outline for the same area, the higher the shape index and the fractal dimension; only the block has a core.")

!!! science "Key idea: shape and core"
    The **shape index** compares a patch's perimeter \(p\) with the most compact perimeter its area \(a\) could have
    (on a grid of cells, a square):

    \[ \text{SHAPE} = \frac{p}{4\sqrt{a}} \]

    in cell units. It is 1 for a square and grows without limit as the outline lengthens: 1.02 for the block (a
    perimeter of 20 cell sides for 24 cells), 1.43 for the strip (28), 2.55 for the spiral (50). The **fractal
    dimension** \(\text{FRAC} = 2 \ln(0.25\,p) / \ln a\) does the same on a scale from 1 (simple) to 2 (a line that
    fills the plane).

    The **core area** is what remains of a patch when the cells within the edge depth of its boundary are removed;
    the **core area index** is its share of the patch, \(\text{CAI} = 100\,\text{core}/a\). With an edge one cell
    deep, the block keeps a core of 4 × 2 cells (33%); the strip and the spiral, nothing. To a species that avoids
    edges, they are not habitat at all.

### Isolation and aggregation

**Isolation** is measured at the patch level by the Euclidean distance to the nearest patch of the same class
(`enn`), from cell centre to cell centre. With 8 neighbours, two separate patches are at least one cell apart, so
`enn` is never less than two cells.

**Aggregation** is a property of a class: are its cells together or scattered? Several metrics measure it, each
differently:

!!! science "Key idea: aggregation and cohesion"
    The **aggregation index** (He et al., 2000) counts the *like adjacencies* of a class, \(g_{ii}\) (the pairs of
    neighbouring cells, sharing a side, that are both of class \(i\)), against the most it could have for its area,
    \(\max g_{ii}\), all its cells in one compact block:

    \[ \text{AI} = 100 \, \frac{g_{ii}}{\max g_{ii}} \]

    0% when no two cells of the class touch, 100% when the class is one compact square.

    **Cohesion** (Schumaker, 1996) measures the physical connectedness of the class from the perimeters \(p_j\) and
    areas \(a_j\) of its patches and the number of cells \(Z\) of the map:

    \[ \text{COHESION} = 100 \left(1 - \frac{\sum_j p_j}{\sum_j p_j \sqrt{a_j}}\right) \left(1 - \frac{1}{\sqrt{Z}}\right)^{-1} \]

    It rises quickly towards 100 as the class forms large patches, and is most sensitive when the class is rare.
    Schumaker (1996) found it a better predictor of dispersal success in simulations than the other indices of his
    time.

### Means, and area-weighted means

A class metric is often a patch metric summarised: the mean area of the forest's patches (`area_mn`), its standard
deviation (`area_sd`), its coefficient of variation (`area_cv`). The mean over patches is dominated by the many small
patches. FRAGSTATS also offers the **area-weighted mean** (`area_am`, `shape_am`, `cai_am`...), in which every patch
weighs by its area: the value of the patch that a random cell of the class belongs to. It is usually closer to how
the habitat is experienced. Area-weighted means come from FRAGSTATS, not landscapemetrics; ask for them by name, or
with `extras = true`.

## When to use it

- **Patch metrics** when patches are the unit of the study: occupancy surveys of ponds or woodlots, where patch area
  and isolation are the predictors of island biogeography and metapopulation theory; or to find the patches with the
  most interior, the most edge, the most isolated.
- **Class metrics** to compare the habitat of several landscapes or dates: is there less forest, and is it more
  fragmented? A small set covers most questions: `pland` (amount), `np` or `mesh` (subdivision), `ed` (edge), `ai`
  or `cohesion` (aggregation), `enn_mn` (isolation), `shape_am` (shape) and `cai_am` (interior, with a justified
  edge depth).
- **Not** for the structural role of each cell (use [MSPA](/tutorials/habitat-patterns/mspa/)), nor for
  connectivity for a species with a dispersal distance (use [habitat networks](/tutorials/habitat-networks/)).

## What you need

A categorical map in a projected system with metres, as in part 1. We use
[`augusta.tif`](/data/tutorials/landscape-metrics/what-are-landscape-metrics/augusta.tif) (160 × 120 cells of 30 m).
In the application, **Templates > Landscape metrics > Patches and their shapes** starts a model with the patch
metrics, GuidosToolbox's parcellation and the contortion of the forest on a land cover drawn by chance.

## Build the model

```tfgraph
#! caption: The patch metrics of every patch and the class metrics of every class of a land cover.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "augusta.tif" }

[[nodes]]
id = "patches"
op = "metrics.patch"
inputs = { map = "cover.map" }
params = { metrics = ["area", "perim", "shape", "frac", "core", "cai", "enn", "gyrate"], show = "shape" }
note = "A row per patch; map: the shape index of each."

[[nodes]]
id = "classes"
op = "metrics.class"
inputs = { map = "cover.map" }
params = { metrics = ["ca", "pland", "np", "lpi", "area_mn", "area_am", "shape_mn", "shape_am", "ed", "ai", "cohesion", "enn_mn", "cai_mn"], show = "ai" }
note = "A row per class; map: the aggregation of each."
```

Both take node 1's `map` and are independent. The `metrics` lists are the choices that matter: every metric you list
is a column of the `table`. `show` names the one painted on the output `map`: for the patches, every patch takes its
own value; for the classes, every cell takes the value of its class. Change `show` and run again to see another
metric as a map (++ctrl+enter++ on the node).

The **edge depth** of the core metrics is `edge_depth`, in cells (default 1). It belongs to the question, not to
the map: how far into a forest do edge effects reach for the species or process you study? Choose it from the
literature, as for MSPA's edge width.

## Read the results

### The patches

```tfgallery
columns: 3
caption: "Three patch metrics of Augusta as maps (`show`). Shape index: the large forests and the river's wetlands have long, convoluted outlines (yellow); small woods and fields are compact (dark). Fractal dimension tells the same story on a scale from 1 to 1.3. Radius of gyration: how far a patch extends, the river's corridor the furthest."
images:
  - { src: /img/tutorials/landscape-metrics/patch-and-class-metrics/aug_shape.png, caption: "Shape index" }
  - { src: /img/tutorials/landscape-metrics/patch-and-class-metrics/aug_frac.png, caption: "Fractal dimension" }
  - { src: /img/tutorials/landscape-metrics/patch-and-class-metrics/aug_gyrate.png, caption: "Radius of gyration (m)" }
```

The `table` has a row per patch. The six largest patches of evergreen forest:

| Patch | Area (ha) | Perimeter (m) | Shape | Fractal dim. | Core (ha) | CAI (%) | ENN (m) |
|---|---|---|---|---|---|---|---|
| 658 | 116.0 | 30,060 | 6.98 | 1.28 | 59.2 | 51 | 60 |
| 677 | 104.3 | 20,940 | 5.13 | 1.24 | 63.5 | 61 | 60 |
| 600 | 43.6 | 10,500 | 3.98 | 1.21 | 22.5 | 52 | 60 |
| 708 | 27.5 | 9,000 | 4.29 | 1.23 | 10.8 | 39 | 60 |
| 612 | 24.0 | 7,140 | 3.64 | 1.21 | 11.4 | 48 | 60 |
| 630 | 23.2 | 4,260 | 2.21 | 1.13 | 14.9 | 64 | 60 |

(All the evergreen patches are in
[evergreen_patches.csv](/data/tutorials/landscape-metrics/patch-and-class-metrics/evergreen_patches.csv).)
Patches 658 and 677 are of similar size, but 658 has half again as much outline: a shape index of 7.0 against 5.1,
and less core (59 ha against 64). For an interior species, 677 is the better forest. Patch 630, smaller but compact,
keeps 64% of itself as core. And every one of them is 60 m, two cells, from the next evergreen patch: in this
landscape the evergreen forest is one fabric cut by roads and streams one cell wide. Its median `enn` is 60 m.

```tfchart
type: scatter
data: /data/tutorials/landscape-metrics/patch-and-class-metrics/area_shape.csv
x: log10_area
y: [Deciduous forest, Evergreen forest, Mixed forest]
colors: { "Deciduous forest": "#68ab5f", "Evergreen forest": "#1c5f2c", "Mixed forest": "#b5c58f" }
xlabel: "Patch area (log10 of ha)"
ylabel: Shape index
caption: "Shape against area for every forest patch of two cells or more. Large patches have more convoluted outlines: shape grows with area, so compare shapes between patches of similar size, or use the fractal dimension, which depends less on it."
```

![Core area index of the patches](/img/tutorials/landscape-metrics/patch-and-class-metrics/aug_cai.png "The core area index of every patch, with an edge one cell (30 m) deep. The large forests and the wetlands keep half or more of their area as core; the small patches and strips none.")

![Nearest neighbour distance](/img/tutorials/landscape-metrics/patch-and-class-metrics/aug_enn.png "The distance from every patch to the nearest patch of its class (a log scale). Most patches are 60 to 100 m from the next: the classes of this map recur everywhere. The bright spots are isolated patches of rare classes, such as the emergent wetlands, 2.8 km apart.")

### The classes

The class `table` (the full table in
[augusta_classes.csv](/data/tutorials/landscape-metrics/patch-and-class-metrics/augusta_classes.csv)), for six
classes:

| Class | PLAND (%) | NP | Mean area (ha) | Area-weighted mean (ha) | ED (m/ha) | AI (%) | COHESION | ENN_MN (m) |
|---|---|---|---|---|---|---|---|---|
| Evergreen forest | 37.5 | 139 | 4.7 | 48.0 | 113.5 | 77.4 | 94.6 | 76 |
| Deciduous forest | 19.1 | 123 | 2.7 | 26.7 | 73.8 | 71.3 | 92.0 | 94 |
| Mixed forest | 10.2 | 173 | 1.0 | 4.0 | 70.3 | 49.0 | 80.0 | 99 |
| Pasture/hay | 9.5 | 76 | 2.2 | 10.3 | 35.8 | 73.0 | 87.3 | 102 |
| Woody wetlands | 8.3 | 10 | 14.3 | 114.8 | 14.5 | 88.9 | 97.3 | 324 |
| Developed, open space | 4.1 | 241 | 0.3 | 1.1 | 38.0 | 31.1 | 64.5 | 88 |

How to read it:

- **The mean and the area-weighted mean** of the evergreen forest's patches are 4.7 and 48 ha. The first says most
  patches are small; the second that a random evergreen cell lies in a patch of about 48 ha. The second is the one
  that matters to a forest animal.
- **Woody wetlands** are the most aggregated class (AI 89%, cohesion 97) in 10 patches with little edge for their
  area (14.5 m/ha): the river's floodplain forest is a few large, continuous blocks. But their patches are far apart
  (324 m on average): what little there is outside the floodplain is isolated.
- **Mixed forest** covers 10% in 173 small patches with an AI of 49%: it is scattered, mostly thin transitions
  between the other two forests, not a habitat of its own.
- **Developed open space** has 241 patches of 0.3 ha on average and the lowest aggregation: strips along roads.

![Aggregation index of each class](/img/tutorials/landscape-metrics/patch-and-class-metrics/aug_ai.png "Every cell painted with the aggregation index of its class (`classes.map` with show = ai). The wetlands along the river stand out; the roads, lawns and verges (developed open space) are the least aggregated.")

```tfchart
type: scatter
data: /data/tutorials/landscape-metrics/patch-and-class-metrics/ai_pland.csv
x: pland
y: [Forest, Developed, Open and farmed, Water and wetlands]
colors: { Forest: "#3b8a5a", Developed: "#d0574a", "Open and farmed": "#dcc339", "Water and wetlands": "#466b9f" }
xlabel: "Share of the map, PLAND (%)"
ylabel: "Aggregation index, AI (%)"
caption: "The aggregation of the 14 classes against their share of the map. Common classes tend to be more aggregated: a class that covers more of the map has more chances to form large patches. That is why aggregation should be compared at a similar amount, or against a neutral model with the same amount."
```

## Settings that change the answer

### The depth of the edge

The core area depends entirely on `edge_depth`. Move the slider to deepen the edge from 30 to 180 m.

```tfslider
label: Edge depth (m)
caption: "The core area index of every patch of Augusta as the edge deepens from 1 to 6 cells (30 to 180 m). At 30 m the large forests are half core; at 90 m only the interiors of the largest forests and of the wetlands remain; at 180 m almost nothing. Numbers: the core of each forest as a % of the landscape (cpland), and the evergreen forest's separate cores (ndca)."
frames:
  - { value: 30, src: /img/tutorials/landscape-metrics/patch-and-class-metrics/cai_1.png, caption: "edge_depth = 1 cell (30 m)", stats: { "Evergreen core": "16.9%", "Deciduous core": "6.4%", "Mixed core": "0.9%", "Evergreen cores": "140" } }
  - { value: 60, src: /img/tutorials/landscape-metrics/patch-and-class-metrics/cai_2.png, caption: "edge_depth = 2 cells (60 m)", stats: { "Evergreen core": "7.5%", "Deciduous core": "1.8%", "Mixed core": "0.1%", "Evergreen cores": "71" } }
  - { value: 90, src: /img/tutorials/landscape-metrics/patch-and-class-metrics/cai_3.png, caption: "edge_depth = 3 cells (90 m)", stats: { "Evergreen core": "3.1%", "Deciduous core": "0.5%", "Mixed core": "0.0%", "Evergreen cores": "36" } }
  - { value: 120, src: /img/tutorials/landscape-metrics/patch-and-class-metrics/cai_4.png, caption: "edge_depth = 4 cells (120 m)", stats: { "Evergreen core": "1.2%", "Deciduous core": "0.1%", "Mixed core": "0.0%", "Evergreen cores": "17" } }
  - { value: 150, src: /img/tutorials/landscape-metrics/patch-and-class-metrics/cai_5.png, caption: "edge_depth = 5 cells (150 m)", stats: { "Evergreen core": "0.5%", "Deciduous core": "0.0%", "Mixed core": "0.0%", "Evergreen cores": "5" } }
  - { value: 180, src: /img/tutorials/landscape-metrics/patch-and-class-metrics/cai_6.png, caption: "edge_depth = 6 cells (180 m)", stats: { "Evergreen core": "0.2%", "Deciduous core": "0.0%", "Mixed core": "0.0%", "Evergreen cores": "3" } }
chart:
  type: line
  data: /data/tutorials/landscape-metrics/patch-and-class-metrics/core_depth.csv
  x: depth_m
  y: [evergreen, deciduous, mixed]
  labels: { evergreen: "Evergreen forest", deciduous: "Deciduous forest", mixed: "Mixed forest" }
  colors: { evergreen: "#1c5f2c", deciduous: "#68ab5f", mixed: "#b5c58f" }
  ylabel: "Core, % of the landscape"
```

The evergreen forest covers 37.5% of the map; with an edge of 30 m, 16.9% of the map is its core; with 90 m, 3.1%.
The deciduous and mixed forests lose their core much faster: they are narrower. If edge effects on your species reach
100 m, this landscape has almost no interior forest, whatever its forest cover says. Report the edge depth with
every core metric, and when it is uncertain, report the curve.

### The aggregation of habitat

The class metrics of configuration are meant to measure arrangement, but they also respond to the amount of
habitat. To see them respond to arrangement alone, we hold the amount fixed: four maps of exactly 35% habitat cut
from fractal surfaces ([[nlm.fbm]]) of growing smoothness.

```tfgallery
columns: 4
caption: "Four landscapes of 160 × 160 cells with exactly 35% habitat, from the most scattered (left) to the most aggregated (right), made by cutting fractal surfaces of dimension parameter 0.4, 0.9, 1.4 and 1.9 at 35%."
images:
  - { src: /img/tutorials/landscape-metrics/patch-and-class-metrics/agg_0_4.png, caption: "0.4: 604 patches, AI 57%" }
  - { src: /img/tutorials/landscape-metrics/patch-and-class-metrics/agg_0_9.png, caption: "0.9: 396 patches, AI 70%" }
  - { src: /img/tutorials/landscape-metrics/patch-and-class-metrics/agg_1_4.png, caption: "1.4: 204 patches, AI 82%" }
  - { src: /img/tutorials/landscape-metrics/patch-and-class-metrics/agg_1_9.png, caption: "1.9: 72 patches, AI 90%" }
```

```tfchart
type: line
data: /data/tutorials/landscape-metrics/patch-and-class-metrics/aggregation.csv
x: dimension
y: [ai, pladj, cohesion, lpi, enn_mn]
labels: { ai: "AI (%)", pladj: "PLADJ (%)", cohesion: "COHESION", lpi: "Largest patch (%)", enn_mn: "Mean ENN (m)" }
xlabel: Smoothness of the surface (fbm's dimension)
ylabel: Value
caption: "Class metrics of the habitat as it aggregates at a constant 35%. AI and PLADJ (the share of like adjacencies) rise together from 57 to 90; the largest patch doubles; cohesion barely moves (96 to 99), being already near its ceiling at this amount. The mean distance to the nearest patch rises: fewer patches are further apart."
```

Two lessons. First, metrics of aggregation are not interchangeable: cohesion saturates at amounts where AI still
discriminates, and the opposite happens when habitat is rare. Second, `enn_mn` *rises* as the habitat aggregates:
with fewer patches, the nearest one is further away. A higher mean isolation does not mean a worse landscape for a
species; read isolation together with patch size and number. Neel et al. (2004) mapped such behaviours over full
gradients of amount and aggregation, and their figures are worth a look before choosing a metric.

## Common pitfalls

- **Comparing shapes of patches of different sizes.** Shape grows with area on a grid (the chart above).
- **Using `area_mn` to say the habitat is fragmented.** A falling mean patch area can mean large patches broke, or
  that many new small ones appeared while the large ones stayed intact. Look at `np`, `lpi` and `area_am` together.
- **Ignoring the amount.** Edge density, patch number, isolation and aggregation all change with `pland`. Report
  `pland` beside them, and test configuration at a fixed amount (part 5 and the
  [neutral landscapes](/tutorials/neutral-landscapes/) tutorials).
- **Core metrics without an edge depth.** A core area "of 17%" means nothing without "with an edge of 30 m".
- **Single-cell patches.** A patch of one cell has no defined aggregation (`ai` is NA) and a shape of 1; rare classes
  of a few cells (here, cultivated crops, one cell) give unstable class metrics. Drop or merge them.
- **The border.** Patches cut by the map's border look smaller and simpler than they are; the largest patches are the
  most likely to be cut.

## Going further

- [Part 3](/tutorials/landscape-metrics/landscape-level-and-diversity/): the landscape level, diversity and
  contagion.
- [Habitat patterns](/tutorials/habitat-patterns/spa-and-objects/): GuidosToolbox's objects by size,
  parcellation and the effective mesh size, the contortion of outlines.
- The operator reference: [[metrics.patch]], [[metrics.class]].

## References

- He, H. S., DeZonia, B. E., & Mladenoff, D. J. (2000). An aggregation index (AI) to quantify spatial patterns of landscapes. *Landscape Ecology*, 15(7), 591–601. https://doi.org/10.1023/A:1008102521322
- Hesselbarth, M. H. K., Sciaini, M., With, K. A., Wiegand, K., & Nowosad, J. (2019). landscapemetrics: an open-source R tool to calculate landscape metrics. *Ecography*, 42(10), 1648–1657. https://doi.org/10.1111/ecog.04617
- McGarigal, K., & Marks, B. J. (1995). *FRAGSTATS: spatial pattern analysis program for quantifying landscape structure* (General Technical Report PNW-GTR-351). USDA Forest Service, Pacific Northwest Research Station. https://doi.org/10.2737/PNW-GTR-351
- Neel, M. C., McGarigal, K., & Cushman, S. A. (2004). Behavior of class-level landscape metrics across gradients of class aggregation and area. *Landscape Ecology*, 19(4), 435–455. https://doi.org/10.1023/B:LAND.0000030521.19856.cb
- Schumaker, N. H. (1996). Using landscape indices to predict habitat connectivity. *Ecology*, 77(4), 1210–1225. https://doi.org/10.2307/2265590
