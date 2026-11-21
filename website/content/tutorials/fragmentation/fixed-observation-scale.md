---
title: Connectivity, contagion and mosaics at a fixed scale
short: Fixed observation scale
description: "GuidosToolbox's fragmentation at a fixed observation scale (FOS) by density, edges or clustering, binary maps and % cover maps, Riitters' model of density and contagion, spatial entropy, GraySpatCon's moving windows and the landscape mosaic, with how the window changes them."
order: 20
level: intermediate
time: 55 min
template: Fragmentation of a forest
ops: [fragmentation.fos, fragmentation.contagion, fragmentation.clustering, fragmentation.entropy, fragmentation.entropy_map, fragmentation.gray_convolution, fragmentation.landscape_mosaic]
series: Measuring fragmentation
part: 2
---

[Part 1](/tutorials/fragmentation/fragmentation-and-habitat-loss/) measured how much forest surrounds every forest
cell. Two forest cells with 60% forest around them can be in very different places: at the straight edge of a large
block, or in a scatter of small woods. This part measures the **arrangement** of the forest in the window as well as
its amount, with GuidosToolbox's fragmentation at a fixed observation scale (FOS), Riitters' classic model of density
and contagion, and the spatial entropy; then extends the window to maps of % tree cover (GraySpatCon) and to the
**landscape mosaic**, which asks what the rest of the window is: fields or towns. At the end you will be able to
choose among GuidosToolbox's indices for a question, map them, and read their reports.

## The idea

### Density, edges and clustering

FOS computes, for every forest cell, a **connectivity** of the forest in the window around it, from 0 to 100%, in one
of three ways (Vogt & Riitters, 2017):

| Method | What it counts in the window | High when |
|---|---|---|
| **FAD**, forest area density | the share of forest (part 1) | there is much forest |
| **FED**, forest edge density | the edges between forest and the rest (GraySpatCon's clustering of the forest) | the forest has little edge for its amount |
| **FAC**, forest area clustering | the pairs of neighbouring cells that are both forest, among all pairs with data | the forest is in compact blocks |

All three are reported in GuidosToolbox's classes, the same as FAD's: rare, patchy, transitional, dominant, interior
and intact (or five classes, intact merged into interior). GuidosToolbox's report calls *fragmentation* the
complement, \(100\% - \text{connectivity}\).

!!! science "Key idea: density and contagion"
    Riitters et al. (2000) described the forest in the window around every forest cell by two probabilities:

    - \(P_f\), the share of the window that is forest: FAD (here divided by 100);
    - \(P_{ff}\), the *contagion*: of the pairs of neighbouring cells in the window that include a forest cell, the share
      in which both are forest. If you stand on forest in that window, the chance your neighbour is forest too.

    Together they place every cell in one of six categories: **interior** (\(P_f = 1\)), **patch** (\(P_f < 0.4\)),
    **transitional** (\(0.4 \le P_f < 0.6\)), and for \(P_f \ge 0.6\), **edge** when \(P_f > P_{ff}\) (the forest's
    non-forest neighbours are on one side, as at the edge of a block), **perforated** when \(P_f < P_{ff}\) (the gaps
    are holes inside the forest), and undetermined when they are equal. The model was used for the first global
    map of forest fragmentation (Riitters et al., 2000) and for the forests of the United States (Riitters et al.,
    2002).

### Entropy

**Spatial joint entropy** measures how mixed neighbouring cells are, from the pairs of neighbours: 0% when the map is
one class, 100% when forest and non-forest neighbours are as mixed as they can be. For the whole map it is one number;
in a moving window, a map of where the forest and the rest are most intermingled.

### % cover maps

Not every habitat is a yes-or-no. Maps of **% tree cover** (or % impervious surface, % cropland) are continuous, and
binarizing them at a threshold throws information away. GuidosToolbox's FOS reads such maps directly (values 0 to 100,
255 missing): the foreground is the cover from a threshold up, and the connectivity the mean cover in the window
(FAD) or its clustering (FED). GraySpatCon (Riitters and Vogt's moving-window program for % maps, part of
GuidosToolbox) computes more: the mean, median, standard deviation, Shannon's diversity or evenness of the cover around
every cell.

### The landscape mosaic

Density says how much forest surrounds a cell, not what the rest is. Is the forest surrounded by farmland or by
suburbs? The **landscape mosaic** (Riitters et al., 2009) describes every window by its shares of three land covers,
**agriculture**, **natural** and **developed**, and places it in a triangle of compositions divided into 19 classes:
*pure* (100% of one), *dominated* (more than 60% of one, named by the others present), and *mosaics* (none above
60%). The mosaic captures the human context of the habitat, which decides the kind of edge effects it receives: a
crop field and a road do not make the same edge.

## When to use it

- **FOS-FAC or FED** to tell a forest cell inside a compact block from one in a scatter of small woods at the same
  density; to report the connectivity of a forest at one scale, with GuidosToolbox's report (ECA, coherence).
- **Riitters' model** to map edge versus perforation: whether a forest is being eaten from its edges or pierced from
  within.
- **Entropy** to compare how intermingled maps or regions are.
- **% cover maps** (FOS on gray maps, GraySpatCon) for tree cover products such as Global Forest Watch's, without
  choosing one threshold.
- **The landscape mosaic** for the human context of habitat: the threat a forest's neighbourhood poses (Riitters et
  al., 2012), the urbanization of a region.
- **Not** for the structural role of cells (MSPA) or connectivity for a species (habitat networks).

## What you need

As in part 1, a map coded 0 missing, 1 background, 2 forest, or classes given in `foreground`:
[`input.tif`](/data/tutorials/habitat-patterns/mspa/input.tif). For % cover, a map of 0–100 with 255 missing:
GraySpatCon's sample, a map of tree cover of the conterminous United States at 2.43 km (1,289 × 1,990 cells), shipped
with terraflux-py. For the mosaic, a land cover: [`clc.tif`](/data/tutorials/habitat-patterns/mspa/clc.tif), CORINE
around Hannover. In the application, **Templates > Patterns (GuidosToolbox) > Fragmentation of a forest** has FOS by
FAC; **Landscape mosaic** has the mosaic, contagion and entropy.

## FOS: density, edges and clustering

### Build the model

```tfgraph
#! caption: The forest's connectivity in windows of 27 cells by FAD, FED and FAC.
#! numbered: true
#! highlight: fos_fac
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "input.tif" }

[[nodes]]
id = "fos_fad"
op = "fragmentation.fos"
inputs = { map = "forest.raster" }
params = { window = 27, method = "FAD", classes = 6 }

[[nodes]]
id = "fos_fed"
op = "fragmentation.fos"
inputs = { map = "forest.raster" }
params = { window = 27, method = "FED", classes = 6 }

[[nodes]]
id = "fos_fac"
op = "fragmentation.fos"
inputs = { map = "forest.raster" }
params = { window = 27, method = "FAC", classes = 6 }
note = "Clustering: forest-forest pairs."
```

[[fragmentation.fos]] takes the map and four settings that matter: `window` (the side in cells, odd), `method` (FAD,
FED or FAC), `classes` (5 or 6: whether intact is apart from interior) and `connectivity` (8 or 4, for counting the
patches of the report). Its outputs: `map` (the connectivity, 0–100%, on the forest; 101 background, 102 missing),
`stats` (the share of the forest in each class), `summary` and `report` (GuidosToolbox's report, below).

### Read the results

```tfgallery
columns: 3
caption: "The south of the map (300 × 300 cells), FOS at 27 cells by FAD, FED and FAC, in the same classes (intact dark green, interior green, dominant light green, transitional yellow, patchy orange, rare red; background grey). The three agree on the large forests and the small woods; they differ at the forests' edges and in narrow strips, where FED rewards straight edges and FAC penalises thin shapes."
images:
  - { src: /img/tutorials/fragmentation/fixed-observation-scale/fos-fad-zoom.png, caption: "FAD" }
  - { src: /img/tutorials/fragmentation/fixed-observation-scale/fos-fed-zoom.png, caption: "FED" }
  - { src: /img/tutorials/fragmentation/fixed-observation-scale/fos-fac-zoom.png, caption: "FAC" }
```

| Method | Intact | Interior | Dominant | Transitional | Patchy | Rare | Average |
|---|---|---|---|---|---|---|---|
| FAD | 7.3 | 32.7 | 32.2 | 14.0 | 12.2 | 1.6 | 73.5% |
| FED | 12.5 | 28.4 | 32.0 | 13.7 | 12.1 | 1.5 | 73.9% |
| FAC | 6.9 | 30.1 | 31.4 | 14.6 | 13.8 | 3.2 | 70.6% |

FOS by FAD gives exactly the numbers of FAD in part 1: the same index. FED classes more of the forest as intact (12.5%) and less
as interior: it measures the forest's edges in the window rather than its area.
FAC is the strictest: thin strips of forest, whose cells have many non-forest neighbours, fall to patchy and rare
(3.2% rare against 1.6%). With five classes, intact and interior merge (37.0% for FAC).

### GuidosToolbox's report

The `report` output is GuidosToolbox's text report, the same file it writes (fos.txt); select the node to see it,
or make it an output of the model. For FAC at 27 cells:

```text
FOS parameter settings:
Binary
Foreground connectivity: 8-connectivity
FOS-type selected: FAC_6class
Pixel resolution [m]: 25
Window size [pixels]: 27
Observation scale [(window size * pixel resolution)^2]: 45.5625 hectares <-> 112.587 acres
...
A) Image summary:
Reporting unit area [pixels]: 999730
Foreground area [pixels]: 428490
Foreground area [%]: 42.8606
Number of foreground patches: 2850
Average foreground patch size [pixels]: 150.347
B) Reporting levels
- Average FAC at WS27 [%]: 70.5904
- ECA (Equivalent Connected Area) [pixels]: 221293
- COH (Coherence = ECA/ECA_max*100) [%]: 51.6448
- AVCON (average connectivity) at WS27 [%]: 30.2554
- COH_ru (ECA/Reporting unit area*100) [%]: 22.1353
```

(The full report: [fos-report.txt](/data/tutorials/fragmentation/fixed-observation-scale/fos-report.txt).) Its
reporting levels are the numbers to put in a table:

- **Average FAC**: the mean connectivity of the forest's cells, 70.6%.
- **ECA**: the equivalent connected area of the forest's patches, \(\sqrt{\sum a_i^2}\), the size of one patch that
  would offer as much reachable forest (see [habitat patterns, part 4](/tutorials/habitat-patterns/networks-and-change/));
  **COH**, ECA over the total forest: 51.6%, so the forest behaves as half of one block.
- **AVCON**: the average connectivity over the whole reporting area, forest or not: 30.3%. It falls with both the loss
  of forest and its fragmentation.
- **COH_ru**: ECA over the reporting area.

`pixel_resolution` sets the cell size used in the report's hectares (default 25 m): set it to your map's.

## How the window changes the answer

```tfslider
label: Window (cells)
caption: "FOS by FAC in the south of the map at windows from 5 to 135 cells. At small windows most of the forest is intact; as the window grows, the large forests turn interior then dominant, and the valley's woods transitional and patchy. Numbers: % of the whole forest in each class, and the average connectivity."
frames:
  - { value: 5, src: /img/tutorials/fragmentation/fixed-observation-scale/fac-5.png, caption: "5 cells", stats: { Intact: "61.8%", Interior: "6.2%", Dominant: "15.3%", Transitional: "8.5%", Patchy: "7.1%", Average: "84.9%" } }
  - { value: 9, src: /img/tutorials/fragmentation/fixed-observation-scale/fac-9.png, caption: "9 cells", stats: { Intact: "43.1%", Interior: "14.3%", Dominant: "21.3%", Transitional: "9.7%", Patchy: "9.4%", Average: "80.3%" } }
  - { value: 15, src: /img/tutorials/fragmentation/fixed-observation-scale/fac-15.png, caption: "15 cells", stats: { Intact: "25.1%", Interior: "23.2%", Dominant: "26.3%", Transitional: "11.3%", Patchy: "11.3%", Average: "76.0%" } }
  - { value: 27, src: /img/tutorials/fragmentation/fixed-observation-scale/fac-27.png, caption: "27 cells", stats: { Intact: "6.9%", Interior: "30.1%", Dominant: "31.4%", Transitional: "14.6%", Patchy: "13.8%", Average: "70.6%" } }
  - { value: 45, src: /img/tutorials/fragmentation/fixed-observation-scale/fac-45.png, caption: "45 cells", stats: { Intact: "0.6%", Interior: "25.9%", Dominant: "34.1%", Transitional: "19.7%", Patchy: "17.0%", Average: "65.3%" } }
  - { value: 81, src: /img/tutorials/fragmentation/fixed-observation-scale/fac-81.png, caption: "81 cells", stats: { Intact: "0%", Interior: "15.5%", Dominant: "32.5%", Transitional: "27.8%", Patchy: "22.3%", Average: "58.5%" } }
  - { value: 135, src: /img/tutorials/fragmentation/fixed-observation-scale/fac-135.png, caption: "135 cells", stats: { Intact: "0%", Interior: "8.3%", Dominant: "26.9%", Transitional: "35.6%", Patchy: "28.0%", Average: "52.9%" } }
chart:
  type: area
  data: /data/tutorials/fragmentation/fixed-observation-scale/fac-windows.csv
  x: window
  y: [intact, interior, dominant, transitional, patchy, rare]
  colors: { intact: "#007800", interior: "#00af00", dominant: "#8cc864", transitional: "#ffc800", patchy: "#fa8c5a", rare: "#d73228" }
  stacked: true
  ylabel: "% of the forest"
```

The stacked chart is the forest's *fragmentation profile*: the intact share vanishes by 45 cells, the interior share
peaks at about 27 cells and then shrinks, and transitional and patchy grow steadily. Two forests can have the same
profile at one window and different ones at another; a profile over several windows is the most complete
description.

**Choosing the window.** GuidosToolbox's FAD-MS uses 7, 13, 27, 81 and 243 cells; with 25 m cells, from 3 ha to
3,700 ha. For one scale, choose it from the process: the depth of edge effects on microclimate (tens of metres), the
home range of a forest bird (a few hectares to tens), the area of a mammal's territory (hundreds of hectares). And
report it in metres and hectares, not only in cells: the report does the conversion with `pixel_resolution`.

### FOS in Python

[`tf.fos`][terraflux.fos] is the operator: the map, `window_size`, `method` (`"FAD"`, `"FED"` or `"FAC"`), `classes`
(5 or 6) and `connectivity` (8 or 4) are its settings. The three methods on the sample forest map, which the library
ships as example data:

```python
import pandas as pd
import terraflux as tf

forest = tf.load_raster(tf.example_data("input"))   # 0 missing, 1 background, 2 forest
fos = {m: tf.fos(forest, window_size=27, method=m) for m in ("FAD", "FED", "FAC")}
table = pd.DataFrame({m: f.stats.set_index("class")["percent"] for m, f in fos.items()}).T
table["Average"] = [f.average for f in fos.values()]
print(table[["Intact", "Interior", "Dominant", "Transitional", "Patchy", "Rare", "Average"]].round(1))
```

```text
class  Intact  Interior  Dominant  Transitional  Patchy  Rare  Average
FAD       7.3      32.7      32.2          14.0    12.2   1.6     73.5
FED      12.5      28.4      32.0          13.7    12.1   1.5     73.9
FAC       6.9      30.1      31.4          14.6    13.8   3.2     70.6
```

The table of the part. The map of one method is `fos["FAC"].map` (0 to 100% on the forest, 101 background, 102
missing), and with `classes=5` the intact share joins the interior. Besides the class shares, the result holds the
numbers of GuidosToolbox's report; `summary()` is the report itself, the text of `fos.txt`:

```python
fac = fos["FAC"]
print(f"{fac.n_patches} patches, average {fac.average_patch_size:.1f} cells; "
      f"ECA {fac.eca:.0f}, COH {fac.coherence:.1f}%, AVCON {fac.avcon:.1f}%")
```

```text
2850 patches, average 150.3 cells; ECA 221293, COH 51.6%, AVCON 30.3%
```

The report writes its hectares with `pixel_resolution` (25 m by default): give your map's cell size. The window
sweep of the slider is a loop over `window_size`; the table is the stacked chart's data:

```python
rows = {}
for w in (5, 9, 15, 27, 45, 81, 135):
    s = tf.fos(forest, window_size=w, method="FAC")
    rows[w] = {**s.stats.set_index("class")["percent"].round(2), "Average": round(s.average, 2)}
profile = pd.DataFrame(rows).T[["Intact", "Interior", "Dominant", "Transitional", "Patchy", "Rare", "Average"]]
print(profile.rename_axis("window"))
```

```text
        Intact  Interior  Dominant  Transitional  Patchy  Rare  Average
window                                                                 
5        61.76      6.22     15.26          8.51    7.05  1.20    84.86
9        43.10     14.25     21.32          9.71    9.38  2.25    80.28
15       25.12     23.20     26.27         11.34   11.26  2.81    76.02
27        6.88     30.13     31.40         14.61   13.83  3.16    70.59
45        0.62     25.91     34.14         19.68   17.03  2.62    65.27
81        0.00     15.45     32.49         27.75   22.31  2.01    58.45
135       0.00      8.25     26.92         35.56   27.96  1.31    52.88
```

Two decimals here; the slider rounds them to one, so 15.45 shows there as 15.5. The window must be odd and at least 3;
how to say it in metres is in [part 1's Python section](/tutorials/fragmentation/fragmentation-and-habitat-loss/#in-python).

## Density and contagion: Riitters' model

```tfgraph
#! caption: The forest's density and contagion in windows of 7 cells, the two axes of Riitters et al.'s model; and the spatial entropy, of the whole map and in windows of 27 cells.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "input.tif" }

[[nodes]]
id = "density"
op = "fragmentation.fad"
inputs = { map = "forest.raster" }
params = { window = 7 }
note = "Pf x 100."

[[nodes]]
id = "contagion"
op = "fragmentation.contagion"
inputs = { map = "forest.raster" }
params = { window = 7 }
note = "Pff x 100."

[[nodes]]
id = "entropy"
op = "fragmentation.entropy"
inputs = { map = "forest.raster" }
note = "One number for the map."

[[nodes]]
id = "entropy_map"
op = "fragmentation.entropy_map"
inputs = { map = "forest.raster" }
params = { window = 27 }
```

Riitters' categories are a rule on two maps of the same window: node 2's density and node 3's contagion. With
[[raster.calc]] the rule is one expression; here we drew it from the two maps:

![Density against contagion](/img/tutorials/fragmentation/fixed-observation-scale/pf-pff.png "Every forest cell of the map by its density (Pf) and contagion (Pff) in a window of 7 cells (a log scale of counts). The diagonal separates edge (below: the window's non-forest is on one side) from perforated (above: holes inside the forest); the dashed lines separate patch, transitional and the rest.")

![Riitters' categories](/img/tutorials/fragmentation/fixed-observation-scale/riitters-zoom.png "The six categories in the south of the map. The large forests are interior, pierced by perforations (light green) around clearings; their outlines are edge (blue); the valley's woods transitional and patch.")

Over the whole map: interior 51.7% of the forest, perforated 18.3%, edge 11.5%, transitional 8.4%, patch 7.1%,
undetermined 3.1%. A forest whose loss comes as perforation (clearings inside it) and one whose loss comes from its
edges inwards have the same FAD and different categories; the
[change between dates](/tutorials/fragmentation/change-and-reports/) shows which is happening.

[[fragmentation.contagion]] and [[fragmentation.clustering]] (FAC as a plain map) take the map and `window`; both
give the % on the forest and GuidosToolbox's codes elsewhere.

### Entropy

[[fragmentation.entropy]] gives one number, the spatial joint entropy of the whole map: 32.5% here, a map far from
maximally mixed; its `pairs` table counts every kind of pair of neighbours. [[fragmentation.entropy_map]] computes
it in a window around every cell:

![Entropy in windows of 27 cells](/img/tutorials/fragmentation/fixed-observation-scale/entropy27.png "The spatial entropy within 27 cells of every cell of the map. Dark where the window is all forest or all fields; bright where forest and fields are most intermingled, as in the valley's mosaic in the north.")

### Density, contagion and entropy in Python

| Function | Result |
|---|---|
| `tf.fad(forest, window_size=7)` | the density, \(P_f\) |
| `tf.contagion_pff(forest, window_size=7)` | contagion, \(P_{ff}\): the chance that a neighbour of a forest cell is forest |
| `tf.clustering_fac(forest, window_size=7)` | clustering: the share of all pairs of neighbours that are forest-forest |
| `tf.spatial_entropy(forest)` | one index (0 to 100%) for the whole map |
| `tf.spatial_entropy_map(forest, window_size=27)` | the same in a window around every cell (default 50) |

The functions are [`tf.contagion_pff`][terraflux.contagion_pff], [`tf.clustering_fac`][terraflux.clustering_fac],
[`tf.spatial_entropy`][terraflux.spatial_entropy] and [`tf.spatial_entropy_map`][terraflux.spatial_entropy_map]. The
contagion and clustering maps are bytes, 0 to 100 on the forest, as in the application; `as_byte=False` gives 0 to 1
with NaN off the forest. The library has no function for Riitters' six categories, which are a rule on two maps, a few
lines of numpy (in the application, one [[raster.calc]]):

```python
import numpy as np

pf = tf.fad(forest, window_size=7).density.values / 100        # density; 101 and 102 off the forest
pff = tf.contagion_pff(forest, window_size=7).values / 100     # contagion; a byte map, 0-100 on the forest
on = (pf <= 1) & (pff <= 1)                                    # the forest cells

cat = np.zeros(pf.shape, dtype=int)                            # 0: not forest
cat[on & (pf < 0.4)] = 5                                       # patch
cat[on & (pf >= 0.4) & (pf < 0.6)] = 4                         # transitional
cat[on & (pf >= 0.6) & (pf > pff)] = 3                         # edge
cat[on & (pf >= 0.6) & (pf < pff)] = 2                         # perforated
cat[on & (pf >= 0.6) & (pf == pff)] = 6                        # undetermined
cat[on & (pf == 1)] = 1                                        # interior
names = ["interior", "perforated", "edge", "transitional", "patch", "undetermined"]
print({n: round(float((cat == i + 1).sum() / on.sum() * 100), 2) for i, n in enumerate(names)})

entropy = tf.spatial_entropy(forest)                           # one number for the map
entropy27 = tf.spatial_entropy_map(forest, window_size=27)     # a map, 0-100%
print(round(entropy.entropy_index, 1), round(float(np.nanmax(entropy27)), 1))
```

```text
{'interior': 51.74, 'perforated': 18.3, 'edge': 11.45, 'transitional': 8.42, 'patch': 7.05, 'undetermined': 3.05}
32.5 84.4
```

The shares of the part, with two decimals (the part rounds them to one, so 11.45 is 11.5 there); the entropy is the
32.5% of the map.

## % cover: tree cover of the United States

```tfgraph
#! caption: FOS of a % tree cover map, and GraySpatCon's standard deviation of the cover around every cell.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "gscinput.tif" }
note = "% tree cover, 255 missing."

[[nodes]]
id = "fos"
op = "fragmentation.fos"
inputs = { map = "cover.raster" }
params = { window = 7, method = "FAD", gray_threshold = 30 }
note = "Forest: 30% cover or more."

[[nodes]]
id = "variability"
op = "fragmentation.gray_convolution"
inputs = { map = "cover.raster" }
params = { metric = "stddev", window = 7 }
```

[[fragmentation.fos]] recognises a % cover map by its values (above 4); `grayscale = true` forces it. The
`gray_threshold` (default 30) decides which cells are forest, but the connectivity uses the cover itself: with FAD,
it is the mean cover in the window. [[fragmentation.gray_convolution]] computes one of GraySpatCon's metrics of the
cover in the window (`mean`, `median`, `majority`, `min`, `max`, `range`, `stddev`, `shannon_diversity`,
`shannon_evenness`, `diagonal_contagion`).

```tfgallery
columns: 3
caption: "Tree cover of the conterminous United States at 2.43 km (GraySpatCon's sample); FOS by FAD in windows of 7 cells (17 km) on the cells of 30% cover or more; and the standard deviation of the cover within 7 cells. The eastern forests are dominant, the West's forests patchy or transitional; the cover varies most where forest meets prairie and on the slopes of the western mountains."
images:
  - { src: /img/tutorials/fragmentation/fixed-observation-scale/treecover.png, caption: "Tree cover (%)" }
  - { src: /img/tutorials/fragmentation/fixed-observation-scale/treecover-fos.png, caption: "FOS, FAD" }
  - { src: /img/tutorials/fragmentation/fixed-observation-scale/treecover-sd.png, caption: "SD of the cover" }
```

Forest (30% cover or more) is 29.0% of the area with data, in 17,785 patches. Its average connectivity (the mean
cover around forest cells) is 49.7%: dominant 38.3% of the forest, transitional 29.8%, patchy 28.8%, rare 2.9%, and
interior a mere 0.2% (a window of 17 km with 90% mean cover is rare at this resolution).

### % cover in Python

[`tf.fos`][terraflux.fos] reads a % cover map by its values, as the operator does (`grayscale=True` forces it), and
`gray_threshold` (default 30) decides which cells are forest. GraySpatCon's metrics are
[`tf.gray_spatial_convolution`][terraflux.gray_spatial_convolution]: `metric` is `"mean"`, `"median"`, `"majority"`,
`"min"`, `"max"`, `"range"`, `"stddev"`, `"shannon_diversity"`, `"shannon_evenness"` or `"diagonal_contagion"`.

```python
cover = tf.load_raster(tf.example_data("gscinput"))            # % tree cover, 0-100; 255 missing
gray = tf.fos(cover, window_size=7, method="FAD", gray_threshold=30)   # forest: 30% cover or more
valid = cover.values <= 100
print(f"forest {(cover.values >= 30)[valid].mean() * 100:.1f}% of the area with data, "
      f"{gray.n_patches} patches, average connectivity {gray.average:.1f}%")
print(gray.stats.round(1).to_string(index=False))
spread = tf.gray_spatial_convolution(cover, metric="stddev", window_size=7)   # float32; 255 stays missing
print(spread.dtype, round(float(np.nanmax(spread)), 1))
```

```text
forest 29.0% of the area with data, 17785 patches, average connectivity 49.7%
       class  percent
        Rare      2.9
      Patchy     28.8
Transitional     29.8
    Dominant     38.3
    Interior      0.2
      Intact      0.0
float32 48.3
```

The numbers of the part. `gray.grayscale` is `True`, and `gray.map` the connectivity, the mean cover in the window with FAD.

## The landscape mosaic

```tfgraph
#! caption: CORINE's classes grouped into agriculture, natural and developed land, and the landscape mosaic in windows of 27 cells (675 m).
#! numbered: true
#! highlight: mosaic
format = "terraflux/1"

[[nodes]]
id = "corine"
op = "raster.read"
params = { path = "clc.tif" }

[[nodes]]
id = "mosaic"
op = "fragmentation.landscape_mosaic"
inputs = { map = "corine.raster" }
params = { window = 27, agriculture = [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22], natural = [23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39], developed = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] }
note = "Water (40-44) is in none: missing."
```

[[fragmentation.landscape_mosaic]] takes a land cover and the classes of each of the three groups (or a map already
coded 1 agriculture, 2 natural, 3 developed). Classes in no group are missing: here water, which is neither a
pressure nor a habitat for this question. Its outputs: `map` (19 classes, or 103 with `classes = 103`), `stats` (the
cells of each of the 103 classes and its class of 19), `heatmap` and `triangle` (the shares on GuidosToolbox's
triangle), `heatmap_csv` (GuidosToolbox's heatmap.csv).

```tfcompare
caption: "Around Hannover: the land cover in the mosaic's three groups (left of the handle: agriculture 61%, natural 22%, developed 15%), and the landscape mosaic within 675 m of every cell (right). Pure agriculture (dark blue) covers the open plains; the forests are pure natural (green) at their centres and agriculture-natural mosaics at their edges; the city is pure developed (dark red), ringed by developed-agriculture classes."
before: { src: /img/tutorials/fragmentation/fixed-observation-scale/mosaic-input.png, label: Land cover }
after: { src: /img/tutorials/fragmentation/fixed-observation-scale/mosaic.png, label: Mosaic }
```

![The mosaic's heatmap](/img/tutorials/fragmentation/fixed-observation-scale/mosaic-triangle.png "GuidosToolbox's heatmap: every window as a point in the triangle of compositions (natural at the top, developed bottom left, agriculture bottom right), the colour the share of the cells in each part of the triangle (a log scale). The landscape is concentrated in the pure corners and along the edges of the triangle (mixes of two groups); mixes of all three are rare.")

The largest classes: pure agriculture 28.7% of the cells with data, agriculture above 60% with some of the others
12.3%, agriculture–developed 9.4%, agriculture–natural 8.8%, pure natural 8.6%, pure developed 5.0%. Riitters et al.
(2009) followed the shift of the United States' forests through this triangle between dates: forests moving towards
the developed corner are under pressures that their density alone does not show.

!!! note "Reading the class names"
    A name lists the classes from the most abundant: `Forest-Agriculture` is a natural cover above 60% with
    agriculture as the second class (10% or more), `Forest-Agriculture-Developed` the same with developed land at
    10% or more too, and `Mosaic ...` a mix where no class reaches 60%. Report the class numbers with their names.

### The landscape mosaic in Python

The land cover goes in as three classes, 1 agriculture, 2 natural, 3 developed and 0 missing: [`tf.reclassify`][terraflux.reclassify]
makes it from CORINE (the ranges are `low <= value < high`), with the groups of the graph. The library names the natural
class *Forest* in its class names; it is the same class.

```python
landcover = tf.load_raster(tf.example_data("clc"))             # CORINE around Hannover
three = tf.reclassify(landcover, [(1, 12, 3),                  # developed
                                  (12, 23, 1),                 # agriculture
                                  (23, 40, 2)],                # natural; the water (40-44) is in none: missing
                      others=0)
mosaic = tf.landscape_mosaic(three, window_size=27)            # 19 classes; output_classes=103 for the finer ones
stats = tf.landscape_mosaic_stats(three, window_size=27)       # the cells of each of the 103 classes
shares = stats.groupby("recode_name_19")["percentage"].sum().sort_values(ascending=False)
print(shares.head(6).round(2).to_string())
```

```text
recode_name_19
Pure Agriculture (100%)    28.65
Agriculture (>60%)         12.33
Agriculture-Developed       9.43
Agriculture-Forest          8.83
Pure Forest (100%)          8.62
Forest-Agriculture          6.09
```

The largest classes of the part, with two decimals. `tf.plot(mosaic)` draws the map with the colours of the 19 classes (`tf.LM19_CLASSES` names them,
`tf.lm_colormap()` colours them). The shares of the windows on GuidosToolbox's triangle, the *heatmap*, come from
[`tf.landscape_mosaic_heatmap`][terraflux.landscape_mosaic_heatmap], which writes GuidosToolbox's `heatmap.csv`:

```python
heat = tf.landscape_mosaic_heatmap(three, window_size=27)
heat.to_csv("heatmap.csv")
```

`heat.table` has the shares of the 103 classes and `heat.triangle` the 12 × 21 grid of the triangle. The change of the
heatmap between two dates is in [part 3](/tutorials/fragmentation/change-and-reports/#in-python).

## Settings that change the answer

- **The window**, above; and the cell size it multiplies.
- **The method of FOS**: FAD measures amount, FED edge, FAC clustering. Say which.
- **5 or 6 classes**: intact apart or not.
- **The threshold of a % cover map** (`gray_threshold`): which cells are forest.
- **The groups of the mosaic**: what counts as natural (are pastures agriculture or natural?) changes every class.
- **Missing data**: left out of the windows' counts. Large areas of missing data (water, outside a country) shrink the
  windows near them.

## Common pitfalls

- **Comparing FOS of different methods or windows.** The classes have the same names, not the same meaning.
- **A % map read as binary.** A map of 0–100 is read as % cover automatically; a binary map coded 0/1/2 that also
  has larger codes may be taken for % cover: check the report's first line (Binary or Grayscale).
- **The report's hectares.** They use `pixel_resolution` (25 m by default), not the map's cell size.
- **Entropy across maps of different amounts.** Entropy is highest at 50% forest; compare it between maps with
  similar amounts, or read it with FAD.
- **A mosaic with water as natural.** Water counted as natural makes lakes "natural cores". Leave it out unless it is
  part of the question.

## Going further

- [Part 3](/tutorials/fragmentation/change-and-reports/): how these indices change between two dates.
- [Landscape metrics, moving windows](/tutorials/landscape-metrics/moving-windows/): any landscape metric in a window.
- [Habitat patterns](/tutorials/habitat-patterns/): MSPA's classes of the same forest.
- The operator reference: [[fragmentation.fos]], [[fragmentation.contagion]], [[fragmentation.clustering]],
  [[fragmentation.entropy]], [[fragmentation.entropy_map]], [[fragmentation.gray_convolution]],
  [[fragmentation.landscape_mosaic]].

## References

- Riitters, K., Wickham, J., O'Neill, R., Jones, B., & Smith, E. (2000). Global-scale patterns of forest fragmentation. *Conservation Ecology*, 4(2), 3. https://doi.org/10.5751/ES-00209-040203
- Riitters, K. H., O'Neill, R. V., Wickham, J. D., & Jones, K. B. (1996). A note on contagion indices for landscape analysis. *Landscape Ecology*, 11(4), 197–202. https://doi.org/10.1007/BF02071810
- Riitters, K. H., Wickham, J. D., O'Neill, R. V., Jones, K. B., Smith, E. R., Coulston, J. W., et al. (2002). Fragmentation of continental United States forests. *Ecosystems*, 5(8), 815–822. https://doi.org/10.1007/s10021-002-0209-2
- Riitters, K. H., Wickham, J. D., & Wade, T. G. (2009). An indicator of forest dynamics using a shifting landscape mosaic. *Ecological Indicators*, 9(1), 107–117. https://doi.org/10.1016/j.ecolind.2008.02.003
- Riitters, K. H., Wickham, J. D., Wade, T. G., & Vogt, P. (2012). Global survey of anthropogenic neighborhood threats to conservation of grass-shrub and forest vegetation. *Journal of Environmental Management*, 97, 116–121. https://doi.org/10.1016/j.jenvman.2011.11.009
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Vogt, P., Riitters, K., Rambaud, P., d'Annunzio, R., Lindquist, E., & Pekkarinen, A. (2022). GuidosToolbox Workbench: spatial analysis of raster maps for ecological applications. *Ecography*, 2022(3), e05864. https://doi.org/10.1111/ecog.05864
