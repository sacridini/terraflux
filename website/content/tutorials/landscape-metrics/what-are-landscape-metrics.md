---
title: What landscape metrics are
short: What metrics are
description: "The ideas behind landscape metrics: composition and configuration, patches by 4 or 8 neighbours, the patch, class and landscape levels, and why grain and extent change every number; with a first model on a real land cover."
order: 10
level: beginner
time: 40 min
template: Landscape metrics
ops: [metrics.patch, metrics.class, metrics.landscape, raster.read, raster.resample]
series: Landscape metrics
part: 1
---

A landscape metric is a number that describes the pattern of a map: how much of each land cover there is, how many
pieces it is broken into, how large and how convoluted they are, how mixed the classes are. Ecologists use them to
compare landscapes, to follow one landscape through time, and to relate pattern to what lives in it. This first part
of the series explains what the metrics measure and the choices that every one of them depends on, then runs a first
model on a real land cover map: the 133 metrics of landscapemetrics (and 123 more of FRAGSTATS) at the three levels
of patch, class and landscape. At the end you will be able to read a table of landscape metrics and know which
settings to report with it.

## The idea

### Pattern and process

Landscape ecology starts from one claim: the spatial pattern of a landscape affects ecological processes, and the
processes in turn shape the pattern (Turner, 1989). A forest bird's chance of breeding depends on how far its nest is
from an edge; a beetle's chance of reaching the next wood depends on how far it is and what lies between; a fire
spreads further through a continuous fuel than through a patchwork. To test such ideas we need to *measure* pattern,
and to measure it the same way in every study. Quantitative indices appeared in the 1980s (O'Neill et al., 1988);
the program FRAGSTATS gathered them into one library with standard definitions (McGarigal & Marks, 1995), and the
R package landscapemetrics reimplemented them in open source (Hesselbarth et al., 2019). Terraflux computes the
metrics of landscapemetrics with the same definitions (through the library terraflux-py), so its numbers can be
compared with the literature.

### Composition and configuration

Every metric describes one of two aspects of a map (McGarigal & Marks, 1995; Gustafson, 1998):

- **Composition**: *what* is in the landscape and in which amounts, without reference to where. The share of each
  class, the number of classes, their diversity. Composition does not change if you shuffle the cells.
- **Configuration**: *how* it is arranged. The size and shape of patches, their isolation, how aggregated or
  interspersed the classes are. Configuration changes when you shuffle the cells.

```tfcompare
caption: "Two landscapes of 150 × 150 cells of 30 m with exactly 40% forest: drag the handle. Their composition is the same; their configuration could hardly differ more. Left: 93 patches, an edge density of 64 m/ha, a mean patch of 8.7 ha, an aggregation index of 88%. Right: 410 patches, 320 m/ha of edge, a mean patch of 2.0 ha, an aggregation index of 40%."
before: { src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/cc_clumped.png, label: Clumped }
after: { src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/cc_random.png, label: Random }
```

The distinction matters ecologically. The amount of habitat (composition) and its fragmentation (configuration) have
different effects on species, and many configuration metrics change with the amount alone (Neel et al., 2004): with
less forest, there are usually more and smaller patches. Measure both, and read configuration in the light of
composition. The [fragmentation tutorials](/tutorials/fragmentation/) come back to this.

### Patches, and the neighbour rule

Most metrics are built on **patches**: groups of cells of the same class connected to each other. "Connected" needs a
rule. With **4 neighbours** (the *rook's* rule) two cells belong to the same patch only if they share a side; with
**8 neighbours** (the *queen's* rule) touching at a corner is enough.

![Patches by 8 and 4 neighbours](/img/tutorials/landscape-metrics/what-are-landscape-metrics/toy_patches.png "A map of 8 × 8 cells and its patches. With 8 neighbours (centre) it has 6 patches: 2 of forest, 1 of farming, 3 of water. With 4 neighbours (right) it has 10: the forest cells that touch only at a corner become separate patches (5 of forest), and so does a piece of farming (2).")

The rule is a statement about the organism: can it cross where two patches touch only at a corner? For a forest
bird, probably; for a ground beetle in a hedgerow network, perhaps not. FRAGSTATS and landscapemetrics use 8
neighbours by default, and so does Terraflux (`directions = 8`). Every metric that counts or measures patches depends
on it: say which you used.

### Three levels

The metrics are computed at three levels (McGarigal & Marks, 1995):

| Level | One value per | Examples | Question |
|---|---|---|---|
| **Patch** | patch | area, perimeter, shape index, core area, distance to the nearest patch of the same class | How large, how convoluted, how isolated is *this* wood? |
| **Class** | class | share of the landscape (`pland`), number of patches (`np`), edge density (`ed`), aggregation (`ai`) | How much forest is there, and how is it arranged? |
| **Landscape** | map | number of classes (`pr`), Shannon's diversity (`shdi`), contagion (`contag`) | How diverse and how mixed is the whole mosaic? |

Class and landscape metrics are often summaries of the patch metrics: the mean patch area of the forest
(`area_mn`), its standard deviation (`area_sd`), its coefficient of variation (`area_cv`). Others exist only at
their level: diversity needs several classes, contagion the adjacencies of the whole map.

!!! science "Key idea: a metric is a count or a ratio"
    Most metrics are simple. The share of class \(i\) is \(\text{PLAND}_i = 100 \, A_i / A\), its area over the
    landscape's. Its edge density is \(\text{ED}_i = 10\,000 \, E_i / A\), the length of its edges \(E_i\) in metres
    per hectare of landscape \(A\) in m². Shannon's diversity is
    \(\text{SHDI} = -\sum_{i=1}^{m} P_i \ln P_i\) over the \(m\) classes and their shares \(P_i\). Computing a few
    by hand on a small map is the best way to understand them; the next parts do it for the main families.

### Grain and extent

Every metric depends on two properties of the map that have nothing to do with the landscape itself:

- the **grain**: the size of a cell (and the smallest patch the mappers drew, the *minimum mapping unit*);
- the **extent**: the area the map covers.

A finer grain shows more small patches and more edge; a larger extent takes in more classes and larger patches.
This was shown early (Turner et al., 1989) and studied systematically since: some metrics change predictably with
the grain, others erratically (Wu, 2004). The slider shows the same land cover at five grains, from its own 30 m to
240 m, each coarser map made by the most common class of the cells it covers.

```tfslider
label: Cell size (m)
caption: "The land cover of Augusta (Georgia, USA) at five grains. As the cells grow, small patches and thin strips (roads, streams) vanish and the large forests merge: the number of patches falls twenty-fold, the edge density five-fold, and three rare classes disappear."
frames:
  - { value: 30, src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/grain_30.png, caption: "30 m: the map as it is", stats: { Patches: "1,144", "Edge density": "212 m/ha", Classes: "15", SHDI: "1.90", "Mean evergreen patch": "4.7 ha" } }
  - { value: 60, src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/grain_60.png, caption: "60 m", stats: { Patches: "607", "Edge density": "143 m/ha", Classes: "14", SHDI: "1.87", "Mean evergreen patch": "10.2 ha" } }
  - { value: 90, src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/grain_90.png, caption: "90 m", stats: { Patches: "336", "Edge density": "102 m/ha", Classes: "13", SHDI: "1.82", "Mean evergreen patch": "18.7 ha" } }
  - { value: 150, src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/grain_150.png, caption: "150 m", stats: { Patches: "135", "Edge density": "64 m/ha", Classes: "11", SHDI: "1.71", "Mean evergreen patch": "54.5 ha" } }
  - { value: 240, src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/grain_240.png, caption: "240 m", stats: { Patches: "50", "Edge density": "38 m/ha", Classes: "10", SHDI: "1.51", "Mean evergreen patch": "172.8 ha" } }
chart:
  type: line
  data: /data/tutorials/landscape-metrics/what-are-landscape-metrics/grain.csv
  x: cell
  y: [np, ed]
  labels: { np: "Patches", ed: "Edge density (m/ha)" }
  colors: { np: "#3b8a5a", ed: "#e8901f" }
  ylabel: Value
```

None of these maps is wrong; they answer questions at different scales. The lesson is practical: **compare metrics
only between maps of the same grain, extent and legend**, and choose the grain for the organism and the process, not
because a map happens to exist at it.

## When to use it

Landscape metrics answer questions of the form "how does this landscape differ from that one", "how did it change",
and "does the pattern around a site explain what we observe there":

- comparing the fragmentation of the forest of several regions, or of one region at two dates;
- describing the landscape around sampling sites (bird counts, pollinator traps, camera traps) and using the metrics
  as predictors (part 5);
- mapping where a landscape is diverse or fragmented with a moving window (part 4);
- classifying many landscapes into kinds by their metrics (part 6).

They are *not* a measure of connectivity for a given species: a metric of aggregation or isolation describes the
map, not the movement of an animal through it. For that, use the [habitat networks](/tutorials/habitat-networks/)
or [circuit connectivity](/tutorials/circuit-connectivity/) tutorials. For the structural roles of habitat cells
(core, edge, corridor) use [MSPA](/tutorials/habitat-patterns/); for fragmentation measured around every cell at
several scales, the [fragmentation](/tutorials/fragmentation/) tutorials. And metrics need a question: computing
all 256 and looking for something that correlates is the most common misuse (Li & Wu, 2004; Kupfer, 2012).

## What you need

A **categorical map**: a raster whose values are classes (land cover, habitat types, vegetation), in a **projected
coordinate system** whose unit is the metre, so that areas come out in hectares and lengths in metres. The metrics
read the cell size from the map. On a map in degrees, areas and distances are wrong: project it first
([coordinate systems](/tutorials/getting-started/coordinate-systems/)). Cells without a value (no data) are left out:
they are not a class.

This series uses two real maps:

- [`augusta.tif`](/data/tutorials/landscape-metrics/what-are-landscape-metrics/augusta.tif): an excerpt of
  160 × 120 cells of 30 m (1,728 ha) of the US National Land Cover Database around Augusta, Georgia, the sample map
  of landscapemetrics, with its 15 classes (open water, four kinds of developed land, three of forest, wetlands...);
- [`clc.tif`](/data/tutorials/habitat-patterns/mspa/clc.tif): CORINE Land Cover around Hannover, Germany, 2,000 ×
  2,000 cells of 25 m, used in parts 4 to 7.

In the application, **Templates > Landscape metrics > Landscape metrics** starts a model with class, landscape and
moving-window metrics on a land cover drawn by chance; replace its first node with a [[raster.read]] of your map.

## Build the model

```tfgraph
#! caption: A land cover read once, and its metrics at the three levels.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "augusta.tif" }
note = "15 classes of land cover, 30 m."

[[nodes]]
id = "patches"
op = "metrics.patch"
inputs = { map = "cover.raster" }
params = { metrics = ["area"], show = "area" }
note = "A row per patch; a map of each patch's area."

[[nodes]]
id = "classes"
op = "metrics.class"
inputs = { map = "cover.raster" }
params = { metrics = ["ca", "pland", "np"], show = "pland" }
note = "A row per class."

[[nodes]]
id = "landscape"
op = "metrics.landscape"
inputs = { map = "cover.raster" }
params = { metrics = ["ta", "np", "pr", "shdi"] }
note = "One row for the map."
```

<div class="steps" markdown>

1. **Read the map.** In the library, open **IO** and click [[raster.read]]; in the Node pane, set `path` to your
   file. Its output `map` is the land cover as one band of classes.
2. **Patch metrics.** Open **Landscape metrics** and click [[metrics.patch]]. Drag from the `map` output of node 1
   to its `map` input. In `metrics`, list the metrics you want (here only `area`); with none, it computes every
   patch metric. `show = "area"` also paints every patch with its area on the output `map`.
3. **Class metrics.** Click [[metrics.class]] and connect node 1's `map` to it. `pland` is the share of the map,
   `ca` the class's area in hectares, `np` its number of patches; `show = "pland"` paints every class with its
   share.
4. **Landscape metrics.** Click [[metrics.landscape]] and connect it the same way. `ta` is the total area, `np` all
   the patches, `pr` the number of classes (*patch richness*), `shdi` Shannon's diversity.
5. Select a node and press ++ctrl+enter++: the model runs up to it, and its `table` shows in the Viewer. Click a
   column's header to sort it.

</div>

All three take the same input and do not depend on each other, so they run in parallel. Each gives a `table`; the
patch node also gives `patches`, the map of the patches numbered (useful to find a patch of the table on the map),
and the patch and class nodes a `map` of the metric named in `show`. Instead of listing metrics, `type` picks a
family (`area and edge`, `shape`, `core area`, `aggregation`, `diversity`, `complexity`), and `extras = true` adds
FRAGSTATS' metrics that landscapemetrics lacks (area-weighted means, `_am`, and others).

From the command line, the same model runs with `terraflux run model.toml`; tables that you make outputs of the
model are written as CSV:

```toml
[outputs.class_metrics]
from = "classes.table"
```

!!! note "Tables, and a map with `show`"
    [[metrics.class]] and [[metrics.patch]] always give their `table`; their `map` (the value of one metric painted
    on each class or patch) only when `show` names that metric. A node that reads `map` needs `show`.

## Read the results

![The land cover of Augusta](/img/tutorials/landscape-metrics/what-are-landscape-metrics/augusta.png "The land cover of Augusta, Georgia, in the NLCD's colours, with the share of each class: evergreen forest (37.5%) and deciduous forest (19.1%) dominate, with the Savannah river's wetlands and water across the middle and the city's developed land in the south.")

**The landscape** (`landscape.table`): 1,728 ha, 1,144 patches, 15 classes, a Shannon diversity of 1.90. With 15
classes, the most SHDI could be is \(\ln 15 = 2.71\): the classes are far from equally common.

**The classes** (`classes.table`), the five largest:

| Class | Area (ha) | Share (`pland`, %) | Patches (`np`) |
|---|---|---|---|
| Evergreen forest | 648.7 | 37.5 | 139 |
| Deciduous forest | 329.3 | 19.1 | 123 |
| Mixed forest | 177.0 | 10.2 | 173 |
| Pasture/hay | 164.3 | 9.5 | 76 |
| Woody wetlands | 143.0 | 8.3 | 10 |

Already the table tells a story. Woody wetlands cover 8% of the map in only 10 patches: a few large, continuous
blocks along the river. Mixed forest covers 10% in 173 patches: many small pieces, often transitions between the
evergreen and deciduous forests. Developed open space (lawns, road verges) covers only 4.1% but in 241 patches, the
most of any class: thin, scattered strips.

**The patches** (`patches.table`): 1,144 rows. The largest is 127.9 ha; 352 are single cells of 0.09 ha. Half of the
patches are smaller than 0.27 ha, while the ten largest hold 35% of the map. That is typical: a few large patches
hold most of the area, and very many small ones most of the count. A mean over patches (`area_mn`, 1.5 ha) describes
neither; part 2 shows the better summaries.

```tfgallery
columns: 3
caption: "The three outputs as maps. Left: every patch in a colour of its own (`patches.patches`). Centre: every patch painted with its area (`patches.map`, a log scale): the large forests and the river in yellow, the strips of developed land in dark blue. Right: every class painted with its share (`classes.map`)."
images:
  - { src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/aug_patches.png, caption: "The 1,144 patches" }
  - { src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/aug_area.png, caption: "Patch area (ha)" }
  - { src: /img/tutorials/landscape-metrics/what-are-landscape-metrics/aug_pland.png, caption: "Class share (%)" }
```

!!! tip "Report it"
    With any landscape metric, report: the map (source, date, cell size, minimum mapping unit, classes and how you
    grouped them), the extent, the neighbour rule (4 or 8), whether the map's border counted as edge, the edge depth
    for core areas, and the software and its version ("Terraflux, computing landscapemetrics' definitions").
    Without them, a number cannot be compared with anyone else's.

### In Python

The library, [terraflux-py](/python/), has the same metrics, 133 of landscapemetrics (and more with `extras=True`), with
the same definitions. [`tf.Landscape`][terraflux.Landscape] reads the map once, finds its patches, and then answers any
number of requests at the three levels. The map is the file of this part, `augusta.tif`, so the numbers are the model's:

```python
import numpy as np
import terraflux as tf

cover = tf.load_raster("augusta.tif")           # the NLCD of Augusta, 160 x 120 cells of 30 m
lc = tf.Landscape(cover)                        # the patches are found once; directions=8 by default
print(lc.landscape_metrics(["ta", "np", "pr", "shdi"]).round(2).to_string(index=False))
classes = lc.class_metrics(["ca", "pland", "np"])
print(classes.sort_values("pland", ascending=False).head(5).round(1).to_string(index=False))
```

```text
    ta     np   pr  shdi
1728.0 1144.0 15.0   1.9
 class    ca  pland    np
    42 648.7   37.5 139.0
    41 329.3   19.1 123.0
    43 177.0   10.2 173.0
    81 164.3    9.5  76.0
    90 143.0    8.3  10.0
```

These are the tables of nodes 4, 3 and 2: the landscape's one row, and the five largest classes with their area in
hectares, share and patches. The tables are *wide* (a column per metric); `class` is the land-cover code of the NLCD
(42 evergreen forest, 41 deciduous, 43 mixed forest, 81 pasture, 90 woody wetlands). The patch level is
`lc.patch_metrics([...])`, a row per patch (`class`, `id` and the metrics); the patches of the graph's `patches` output
are its `id`s, and `lc.to_raster("area")` paints each with its value (`show = "area"`):

```python
area = lc.patch_metrics(["area"])["area"]
print(len(area), area.max().round(1), (area < 0.1).sum(), area.median().round(2),
      (area.nlargest(10).sum() / area.sum()).round(2), area.mean().round(2))
```

```text
1144 127.9 352 0.27 0.35 1.51
```

The patches, the largest (127.9 ha), the single cells, the median, the share of the ten largest (35% of the map) and
the mean (1.5 ha) of the paragraph above. Areas are in hectares and lengths in map units, from the cell size of the map.

**The neighbour rule.** `directions` is an option of the `Landscape` (and of every function that makes one). The toy
map of the figure is a numpy array; its cells have size 1, so `res=10` gives them the figure's 10 m:

```python
toy = np.array([[1, 1, 1, 2, 2, 2, 3, 3], [1, 1, 2, 2, 2, 3, 3, 3], [1, 1, 1, 2, 1, 3, 3, 1],
                [3, 1, 1, 1, 2, 1, 3, 1], [3, 3, 1, 2, 2, 2, 1, 1], [3, 3, 2, 2, 3, 2, 1, 1],
                [2, 2, 2, 3, 3, 1, 1, 1], [2, 2, 1, 3, 3, 1, 1, 1]], dtype="uint8")
for d in (8, 4):
    t = tf.Landscape(toy, res=10, directions=d)
    print(d, "neighbours:", int(t.landscape_metrics(["np"])["np"].iloc[0]), "patches;",
          "forest, farming, water:", t.class_metrics(["np"])["np"].astype(int).tolist())
```

```text
8 neighbours: 6 patches; forest, farming, water: [2, 1, 3]
4 neighbours: 10 patches; forest, farming, water: [5, 2, 3]
```

**Composition and configuration.** The two 150 × 150 landscapes with 40% forest are made with the library's own
neutral landscape models, [`tf.nlm.fbm`][terraflux.nlm.fbm] and [`tf.nlm.random`][terraflux.nlm.random], cut at 0.4 by
[`tf.nlm.binarize`][terraflux.nlm.binarize]; the seed gives the same maps as the graph's nodes:

```python
clumped = tf.nlm.binarize(tf.nlm.fbm(150, 150, resolution=30, fract_dim=1.7, seed=3), 0.4)
shuffled = tf.nlm.binarize(tf.nlm.random(150, 150, resolution=30, seed=3), 0.4)
for name, m in (("clumped", clumped), ("random", shuffled)):
    c = tf.Landscape(m).class_metrics(["pland", "np", "ed", "area_mn", "ai"])
    print(name, c[c["class"] == 2].round(1).to_dict("records")[0])
```

```text
clumped {'class': 2, 'pland': 40.0, 'np': 93.0, 'ed': 63.9, 'area_mn': 8.7, 'ai': 88.4}
random {'class': 2, 'pland': 40.0, 'np': 410.0, 'ed': 319.5, 'area_mn': 2.0, 'ai': 39.9}
```

The same amount (`pland` 40%), 93 and 410 patches, as the slider's caption says.

**Grain.** [`tf.reproject`][terraflux.reproject] with `res=` resamples a raster in its own coordinate system, and
`resampling="mode"` gives each coarse cell the most common class of the cells it covers:

```python
for res in (30, 60, 90, 150, 240):
    m = cover if res == 30 else tf.reproject(cover, res=res, resampling="mode")
    t = tf.Landscape(m).landscape_metrics(["np", "ed", "pr", "shdi"]).iloc[0]
    print(res, "m:", int(t["np"]), "patches,", round(t["ed"], 1), "m/ha,", int(t["pr"]), "classes, SHDI", round(t["shdi"], 2))
```

```text
30 m: 1144 patches, 212.1 m/ha, 15 classes, SHDI 1.9
60 m: 607 patches, 142.5 m/ha, 14 classes, SHDI 1.87
90 m: 342 patches, 102.1 m/ha, 13 classes, SHDI 1.81
150 m: 135 patches, 63.6 m/ha, 11 classes, SHDI 1.71
240 m: 50 patches, 38.1 m/ha, 10 classes, SHDI 1.51
```

The slider's numbers, but for the 90 m map: 342 patches here against 336 in the slider. Most likely the two resamplings
break the ties between equally common classes differently; every other number agrees.

**Finding metrics.** [`tf.list_metrics`][terraflux.list_metrics] is the catalogue, with each metric's name, type and
level. The same filters (`level`, `metric`, `name`, `type`; a leading minus excludes) select the metrics everywhere
they are asked for:

```python
catalogue = tf.list_metrics()
print(len(catalogue), catalogue["type"].unique().tolist())
print(tf.list_metrics(level="landscape", type="diversity metric")[["metric", "name"]].head(4).to_string(index=False))
print(lc.metrics(level="class", metric=["area", "shape"]).shape,                 # area_mn, area_sd, shape_mn...
      lc.metrics(level="landscape", type="-complexity metric").shape)            # everything but complexity
```

```text
133 ['area and edge metric', 'core area metric', 'shape metric', 'aggregation metric', 'complexity metric', 'diversity metric']
metric                               name
 msidi modified simpson's diversity index
 msiei  modified simpson's evenness index
    pr                     patch richness
   prd             patch richness density
(90, 5) (61, 5)
```

A metric name given in `metric` selects its family (`area` gives `area_mn`, `area_sd`, `area_cv`), the way `metrics =
[]` and `type` do in the nodes. Upper-case abbreviations and full names work in the wide tables
(`lc.landscape_metrics(["SHDI", "shannon_diversity_index", "contagion"])`). `lc.metrics(...)` is the *long* form, a row
per value (`level`, `class`, `id`, `metric`, `value`), which is convenient for many metrics and levels at once;
`tf.landscape_metrics(cover, level="landscape", metric="shdi")` is the one-call form, without keeping a `Landscape`.
`extras=True` adds FRAGSTATS' metrics that landscapemetrics lacks (`area_am`, `area_md`, `area_ra`...), as in the
nodes. Statistics over patches exist for every patch metric: the mean `_mn`, the standard deviation `_sd` and the
coefficient of variation `_cv`.

**The settings** listed under "Settings that change the answer" are keyword arguments of `Landscape` (and of the one-call functions):

```python
for opts in ({}, {"directions": 4}, {"count_boundary": True}, {"convention": "fragstats"}):
    t = tf.Landscape(cover, **opts).landscape_metrics(["np", "ed", "lpi"]).iloc[0]
    print(opts or "defaults", "->", int(t["np"]), "patches,", round(t["ed"], 1), "m/ha,", round(t["lpi"], 1), "%")
```

```text
defaults -> 1144 patches, 212.1 m/ha, 7.4 %
{'directions': 4} -> 1952 patches, 212.1 m/ha, 7.4 %
{'count_boundary': True} -> 1144 patches, 221.8 m/ha, 7.4 %
{'convention': 'fragstats'} -> 1144 patches, 212.1 m/ha, 7.4 %
```

`directions` changes the patches (1,952 with 4 neighbours); `count_boundary` adds the map's border to the edge;
`consider_boundary`, `edge_depth`, `classes_max` (the potential number of classes, for the relative richness `rpr`),
`neighbourhood`, `ordered` and `base` (the complexity metrics), and `nodata` and `res` (the value of no data and the cell
size, which a numpy array does not have) complete the list. `convention="fragstats"` changes nothing in these three
metrics of Augusta, which the two programs agree on.

## Settings that change the answer

The same settings exist on every metrics operator, because they change the patches or the edges that all the metrics
are built on:

- **`directions`**: 8 (default) or 4 neighbours. With 4, diagonal strips (roads, streams, hedgerows drawn as a
  staircase of cells) break into many patches: the number of patches and every per-patch mean change.
- **`count_boundary`**: whether the map's border counts as edge (default no). With it, a landscape cut by its
  border has more edge.
- **`consider_boundary`**: whether cells at the map's border can be core (default no).
- **`edge_depth`**: the depth of the edge, in cells, for the core-area metrics (part 2).
- **`convention`**: `landscapemetrics` (default) or `fragstats`, where the two programs differ (FRAGSTATS counts
  edges in physical lengths and the sizes of disjunct cores differently).

And three choices outside the operators: the **grain** (the slider above), the **extent**, and the **legend**: the
same map with its 15 classes grouped into 8 has fewer, larger patches and a lower diversity (part 3).

## Common pitfalls

- **A map in degrees.** Areas in "hectares" and edges in "metres" are then meaningless. Project to an equal-area
  system first.
- **No data coded as a class.** A value of 0 or 255 that means "outside the study area" becomes a class with
  patches and edges unless it is set as no data ([[raster.read]]'s `nodata`, or reclassify it to no data).
- **Too many metrics.** Metrics are strongly correlated: a handful of independent dimensions (amount, subdivision,
  edge, shape, aggregation, isolation, diversity) explain most of their variation across landscapes (Riitters et
  al., 1995; Cushman et al., 2008). Choose one per dimension that matters for your question (part 5 shows the
  correlations).
- **Comparing across grains, extents or legends.** See above.
- **Treating a difference as significant.** A metric is one value of one map. Whether two values differ beyond
  what chance would give needs replicates or a model of the pattern: the
  [neutral landscapes](/tutorials/neutral-landscapes/) tutorials show how.
- **Map error.** Misclassified cells make spurious small patches and edges; patch counts and edge metrics suffer
  most. Know the map's accuracy.

## Going further

- [Part 2](/tutorials/landscape-metrics/patch-and-class-metrics/): patch and class metrics, area, shape, core,
  isolation and aggregation.
- [Part 3](/tutorials/landscape-metrics/landscape-level-and-diversity/): the landscape level, diversity and
  contagion.
- [Part 4](/tutorials/landscape-metrics/moving-windows/): metrics in a moving window.
- [Part 5](/tutorials/landscape-metrics/sampling-and-change/): metrics around sites, through time, and their
  redundancy.
- [Part 6](/tutorials/landscape-metrics/landscape-signatures/): kinds of landscapes from their metrics.
- [Part 7](/tutorials/landscape-metrics/large-maps/): maps larger than memory.
- The operator reference: [[metrics.patch]], [[metrics.class]], [[metrics.landscape]].

## References

- Cushman, S. A., McGarigal, K., & Neel, M. C. (2008). Parsimony in landscape metrics: strength, universality, and consistency. *Ecological Indicators*, 8(5), 691–703. https://doi.org/10.1016/j.ecolind.2007.12.002
- Gustafson, E. J. (1998). Quantifying landscape spatial pattern: what is the state of the art? *Ecosystems*, 1(2), 143–156. https://doi.org/10.1007/s100219900011
- Hesselbarth, M. H. K., Sciaini, M., With, K. A., Wiegand, K., & Nowosad, J. (2019). landscapemetrics: an open-source R tool to calculate landscape metrics. *Ecography*, 42(10), 1648–1657. https://doi.org/10.1111/ecog.04617
- Kupfer, J. A. (2012). Landscape ecology and biogeography: rethinking landscape metrics in a post-FRAGSTATS landscape. *Progress in Physical Geography*, 36(3), 400–420. https://doi.org/10.1177/0309133312439594
- Li, H., & Wu, J. (2004). Use and misuse of landscape indices. *Landscape Ecology*, 19(4), 389–399. https://doi.org/10.1023/B:LAND.0000030441.15628.d6
- McGarigal, K., & Marks, B. J. (1995). *FRAGSTATS: spatial pattern analysis program for quantifying landscape structure* (General Technical Report PNW-GTR-351). USDA Forest Service, Pacific Northwest Research Station. https://doi.org/10.2737/PNW-GTR-351
- Neel, M. C., McGarigal, K., & Cushman, S. A. (2004). Behavior of class-level landscape metrics across gradients of class aggregation and area. *Landscape Ecology*, 19(4), 435–455. https://doi.org/10.1023/B:LAND.0000030521.19856.cb
- O'Neill, R. V., Krummel, J. R., Gardner, R. H., Sugihara, G., Jackson, B., DeAngelis, D. L., et al. (1988). Indices of landscape pattern. *Landscape Ecology*, 1(3), 153–162. https://doi.org/10.1007/BF00162741
- Riitters, K. H., O'Neill, R. V., Hunsaker, C. T., Wickham, J. D., Yankee, D. H., Timmins, S. P., et al. (1995). A factor analysis of landscape pattern and structure metrics. *Landscape Ecology*, 10(1), 23–39. https://doi.org/10.1007/BF00158551
- Turner, M. G. (1989). Landscape ecology: the effect of pattern on process. *Annual Review of Ecology and Systematics*, 20, 171–197. https://doi.org/10.1146/annurev.es.20.110189.001131
- Turner, M. G., O'Neill, R. V., Gardner, R. H., & Milne, B. T. (1989). Effects of changing spatial scale on the analysis of landscape pattern. *Landscape Ecology*, 3(3–4), 153–162. https://doi.org/10.1007/BF00131534
- Wu, J. (2004). Effects of changing scale on landscape pattern analysis: scaling relations. *Landscape Ecology*, 19(2), 125–138. https://doi.org/10.1023/B:LAND.0000021711.40074.ae
