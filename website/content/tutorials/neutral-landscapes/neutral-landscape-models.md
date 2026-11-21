---
title: Neutral landscape models
short: Neutral landscapes
description: "Why ecologists draw landscapes by chance, what every neutral landscape model of Terraflux (NLMR's) draws, how percolation makes habitat connect suddenly, and how the settings of a random cluster or a fractal surface control the pattern."
order: 10
level: beginner
time: 45 min
template: Neutral landscapes
ops: [nlm.random, nlm.percolation, nlm.random_cluster, nlm.fbm, nlm.gaussian_field, nlm.mpd, nlm.binarize, nlm.classify, nlm.merge]
series: Neutral landscapes
part: 1
---

A neutral landscape is a map drawn by chance, with known rules and nothing else: no rivers, no roads, no history.
That sounds useless to an ecologist, and it is the opposite. Because we know exactly how a neutral landscape was
made, it tells us what pattern chance alone would give, and so what in a real landscape needs another explanation.
It lets us make many landscapes that differ in one property only, the amount of habitat or its fragmentation, to
see how a species or a metric responds. And it gives every tool something to run on before the real data arrive.
This first part explains the idea and its history, shows every generator of Terraflux side by side, and moves the
settings that control the pattern; [part 2](/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/) uses
them in a study. At the end you will know which model to choose for which purpose and how to set it.

## The idea

### A null model for landscapes

Ecology uses *null models* to ask whether a pattern is more than chance: are these species found together more
often than random draws would give? Gardner et al. (1987) brought the idea to landscapes. They drew maps in which
every cell is habitat with a probability \(p\), independently of the others, and measured the clusters of habitat:
how many, how large, whether one crossed the map. Real landscapes could then be compared with these *neutral*
expectations: a real forest more aggregated than a random map of the same amount is aggregated *for a reason*
(soils, topography, history), and that reason becomes the question.

The random map was soon joined by maps with structure. **Fractal** landscapes, rough or smooth, spatially
autocorrelated as real ones are, were used to study how species respond to the amount and the fragmentation of
habitat (With & Crist, 1995; With, 1997). The **modified random clusters** of Saura and Martínez-Millán (2000) gave
patches with irregular, realistic shapes and controlled amounts of several classes. Software then gathered the
models in libraries: NLMpy in Python (Etherington et al., 2015) and NLMR in R (Sciaini et al., 2018). Terraflux
implements NLMR's models with its settings, and for the same seed draws the same map as NLMR does.

!!! science "Key idea: percolation"
    On a random map, the habitat's clusters grow with \(p\), the share of habitat, but not gradually. Below a
    *critical* share \(p_c\) the clusters are small and separate; just above it, one cluster suddenly spans the map
    (Gardner et al., 1987; Stauffer & Aharony, 1994). For an infinite square grid, \(p_c \approx 0.593\) when cells
    connect by their sides (4 neighbours) and \(p_c \approx 0.407\) when corners count too (8 neighbours). The
    threshold is a property of random maps; on autocorrelated maps habitat connects at lower amounts. Its lesson
    for real landscapes: connectivity can collapse after a small loss of habitat, near a threshold.

### What makes a model neutral

A neutral landscape model has three ingredients:

- **a surface or a process** drawn by chance: independent values, values correlated over a distance, a fractal, a
  gradient, a tessellation of polygons;
- **settings** that control its pattern: the share of habitat, the size of clusters, the roughness;
- **a seed** for the random numbers: the same seed and settings give the same map, another seed gives another map
  with the same properties, a *replicate*.

Many models draw a continuous surface from 0 to 1. A **binarization** cuts it at a share of habitat (the highest
cells become habitat), and a **classification** cuts it into classes of given shares. The amount is then exactly
controlled, and the surface controls only the arrangement.

## When to use it

- **As a null model**: is the aggregation, the shape or the connectivity of a real habitat more than chance?
  ([part 2](/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/))
- **As an experiment**: landscapes that vary in amount and fragmentation independently, with replicates, to see how
  a metric, a dispersal model or a population model responds; impossible with real landscapes, where the two go
  together (Fahrig, 2003).
- **As test data**: every model of this site that starts from a template draws its landscape with these models, so
  that it runs at once; replace them by your maps later.
- **Not** as a prediction of a real landscape: a neutral landscape has the statistical properties you set, not the
  history of a place.

## What you need

Nothing but Terraflux: these models make their own maps. Each takes the size (`rows`, `columns`), the cell's side
(`resolution`, so that metrics come out in hectares and metres) and a `seed`; or, connected to the input `like`, the
grid of another map, to draw a neutral landscape on the grid of a real one. In the application, the library's
subject **Neutral landscape models** has them all; the templates **Templates > Surfaces and uncertainty > Neutral
landscapes** (three of them) and **More neutral landscapes** (the others and the utilities) show them side by side.

## Every generator

```tfgraph
#! caption: Three neutral landscapes side by side: a random cluster of three classes, a Gaussian field and a fractal surface (the template Neutral landscapes).
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "clusters"
op = "nlm.random_cluster"
params = { rows = 256, columns = 256, p = 0.4, weights = [0.25, 0.25, 0.5], seed = 4 }
note = "Classes: a colour each."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 256, columns = 256, range = 20, seed = 4 }
note = "Values from 0 to 1."

[[nodes]]
id = "fractal"
op = "nlm.fbm"
params = { rows = 256, columns = 256, dimension = 1.5, seed = 4 }

[[plots]]
id = "plot_clusters"
layers = [{ from = "clusters.map" }]

[[plots]]
id = "plot_field"
layers = [{ from = "field.map" }]

[[plots]]
id = "plot_fractal"
layers = [{ from = "fractal.map" }]
```

Each generator is one node without inputs: add it from the library, set its size and seed in the Node pane, and
click it to see its `map` in the **Viewer**. The template keeps each map as a Plot (the Viewer's **Plots** menu).
Here are all fifteen, on grids of 200 × 200 cells.

```tfgallery
columns: 4
caption: "Terraflux's fifteen neutral landscape generators (NLMR's), 200 × 200 cells each. Continuous surfaces from 0 (dark) to 1 (yellow); maps of habitat in green and beige; maps of classes a colour per class."
images:
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_random.png, caption: "nlm.random" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_percolation.png, caption: "nlm.percolation (0.5)" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_planar_gradient.png, caption: "nlm.planar_gradient" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_edge_gradient.png, caption: "nlm.edge_gradient" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_distance_gradient.png, caption: "nlm.distance_gradient" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_mpd.png, caption: "nlm.mpd (roughness 0.6)" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_fbm.png, caption: "nlm.fbm (dimension 1.4)" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_gaussian_field.png, caption: "nlm.gaussian_field (range 20)" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_random_cluster.png, caption: "nlm.random_cluster (p 0.55)" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_neigh.png, caption: "nlm.neigh" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_random_rectangular_cluster.png, caption: "nlm.random_rectangular_ cluster" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_curds.png, caption: "nlm.curds" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_mosaic_tess.png, caption: "nlm.mosaic_tess" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_mosaic_gibbs.png, caption: "nlm.mosaic_gibbs" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/g_mosaic_field.png, caption: "nlm.mosaic_field" }
```

What each draws, and what it is good for:

| Generator | Draws | Good for |
|---|---|---|
| [[nlm.random]] | an independent value in every cell | the null model without any structure |
| [[nlm.percolation]] | habitat (1) with probability `prob`, else 0 | Gardner's random maps, percolation |
| [[nlm.planar_gradient]] | a linear gradient along `direction` | a climate, an altitude, a distance from a coast |
| [[nlm.edge_gradient]] | a gradient peaking in the middle of the map | a ridge, a valley floor |
| [[nlm.distance_gradient]] | the distance to a rectangle (`origin`) | the distance from a town or a source |
| [[nlm.mpd]] | a fractal by midpoint displacement (`roughness` 0 smooth to 1 rough) | classic fractal landscapes (Palmer, 1992) |
| [[nlm.fbm]] | fractional Brownian motion (`dimension`, small rough, large smooth) | fractal landscapes of controlled roughness (Keitt, 2000) |
| [[nlm.gaussian_field]] | a smooth surface autocorrelated over `range` cells | environmental surfaces, suitability, drivers |
| [[nlm.random_cluster]] | patches of classes with irregular shapes (`p`, `weights`) | realistic maps of land cover classes |
| [[nlm.neigh]] | categories grown from their neighbours | patchy maps of several categories |
| [[nlm.random_rectangular_cluster]] | overlapping rectangles | fields, plots, human land use |
| [[nlm.curds]] | hierarchical blocks of habitat (`curds`, `steps`) | habitat structured at several scales |
| [[nlm.mosaic_tess]] | Voronoi polygons of random values | polygon mosaics, parcels |
| [[nlm.mosaic_gibbs]] | Voronoi polygons from germs kept apart, in classes | mosaics of regular polygons |
| [[nlm.mosaic_field]] | the sum of random line tessellations | linear structures, faults |

And four utilities turn surfaces into maps: [[nlm.binarize]] (habitat and matrix at a share), [[nlm.classify]]
(classes of given shares, of a real map's shares, or by natural breaks), [[nlm.merge]] (a surface plus others
times a scaling: a gradient with noise) and [[nlm.rescale]] (to 0–1).

### Every generator in Python

All fifteen are in `tf.nlm`, with the names of the R package NLMR (the library is its port). A model returns an
`xarray.DataArray` on a grid with its origin at (0, 0) and cells of size `resolution`; the same `seed` gives the same
landscape on every run, and the same as NLMR. A few settings carry different names from the app's: the size is
`ncol, nrow` (columns first), and [`tf.nlm.fbm`][terraflux.nlm.fbm]'s `dimension` is `fract_dim`,
[`tf.nlm.gaussian_field`][terraflux.nlm.gaussian_field]'s `range` is `autocorr_range`,
[`tf.nlm.random_cluster`][terraflux.nlm.random_cluster]'s `weights` are `ai`, and `steps` of
[`tf.nlm.curds`][terraflux.nlm.curds] is `recursion_steps`. This is the gallery above, with the same seeds:

```python
import numpy as np
import terraflux as tf

models = {
    "random": tf.nlm.random(200, 200, seed=1),
    "percolation": tf.nlm.percolation(200, 200, prob=0.5, seed=1),
    "planar_gradient": tf.nlm.planar_gradient(200, 200, direction=60),
    "edge_gradient": tf.nlm.edge_gradient(200, 200, direction=60),
    "distance_gradient": tf.nlm.distance_gradient(200, 200, origin=(90, 110, 90, 110), nlmr_compat=False),
    "mpd": tf.nlm.mpd(200, 200, roughness=0.6, seed=2, nlmr_compat=False),
    "fbm": tf.nlm.fbm(200, 200, fract_dim=1.4, seed=3),
    "gaussian_field": tf.nlm.gaussian_field(200, 200, autocorr_range=20, seed=4),
    "random_cluster": tf.nlm.random_cluster(200, 200, p=0.55, ai=[0.3, 0.3, 0.4], seed=5, rescale=False),
    "neigh": tf.nlm.neigh(200, 200, p_neigh=0.95, p_empty=0.001, categories=4, seed=6, rescale=False),
    "random_rectangular_cluster": tf.nlm.random_rectangular_cluster(200, 200, minl=5, maxl=30, seed=7),
    "curds": tf.nlm.curds([0.7, 0.7, 0.7], [5, 5, 8], seed=8),   # 5 x 5 x 8 = 200 cells a side
    "mosaic_tess": tf.nlm.mosaic_tess(200, 200, germs=50, seed=8),
    "mosaic_gibbs": tf.nlm.mosaic_gibbs(200, 200, germs=60, R=10, patch_classes=5, seed=9, rescale=False),
    "mosaic_field": tf.nlm.mosaic_field(200, 200, n=20, seed=9),
}
for name, m in models.items():
    print(f"{name:27s} {str(m.shape):10s} {float(m.min()):5.2f} to {float(m.max()):5.2f}  {len(np.unique(m.values))} values")
```

```text
random                      (200, 200)  0.00 to  1.00  40000 values
percolation                 (200, 200)  0.00 to  1.00  2 values
planar_gradient             (200, 200)  0.00 to  1.00  40000 values
edge_gradient               (200, 200)  0.00 to  1.00  39776 values
distance_gradient           (200, 200)  0.00 to  1.00  3062 values
mpd                         (200, 200)  0.00 to  1.00  40000 values
fbm                         (200, 200)  0.00 to  1.00  40000 values
gaussian_field              (200, 200)  0.00 to  1.00  40000 values
random_cluster              (200, 200)  1.00 to  3.00  3 values
neigh                       (200, 200)  0.00 to  3.00  4 values
random_rectangular_cluster  (200, 200)  0.00 to  1.00  369 values
curds                       (200, 200)  0.00 to  1.00  2 values
mosaic_tess                 (200, 200)  0.00 to  1.00  50 values
mosaic_gibbs                (200, 200)  1.00 to  5.00  5 values
mosaic_field                (200, 200)  0.00 to  1.00  11647 values
```

Every model rescales its values to 0–1 by default; for the class models (`random_cluster`, `neigh`,
`mosaic_gibbs`) `rescale=False` keeps their codes (1, 2, 3 above) instead. Every model takes `n_threads`.

## Percolation: when habitat connects

```tfgraph
#! caption: A random map of 57.5% habitat and its patches by 4 neighbours, with the area of each.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "random"
op = "nlm.percolation"
params = { rows = 200, columns = 200, prob = 0.575, seed = 1 }
note = "Habitat (1) with probability 0.575."

[[nodes]]
id = "patches"
op = "metrics.patch"
inputs = { map = "random.map" }
params = { metrics = ["area"], directions = 4, show = "area" }
note = "The largest cluster is the patch of largest area."
```

[[nlm.percolation]]'s `prob` is the share of habitat. [[metrics.patch]] ([landscape metrics](/tutorials/landscape-metrics/patch-and-class-metrics/))
labels its clusters; `directions = 4` connects cells by their sides only. Move the slider through the threshold:

```tfslider
label: Habitat (share of the cells)
caption: "Random maps of 200 × 200 cells with growing shares of habitat (light green), their largest cluster by 4 neighbours in dark green. At 55% the largest cluster holds 4% of the habitat; at 57.5% it suddenly spans the map; at 62.5% it holds most of the habitat."
frames:
  - { value: 0.45, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/perc_0_45.png, caption: "45% habitat", stats: { "Largest cluster": "0.7% of the habitat", "Crosses the map": "no" } }
  - { value: 0.5, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/perc_0_5.png, caption: "50% habitat", stats: { "Largest cluster": "2.1% of the habitat", "Crosses the map": "no" } }
  - { value: 0.55, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/perc_0_55.png, caption: "55% habitat", stats: { "Largest cluster": "4.0% of the habitat", "Crosses the map": "no" } }
  - { value: 0.575, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/perc_0_575.png, caption: "57.5% habitat", stats: { "Largest cluster": "32.0% of the habitat", "Crosses the map": "yes" } }
  - { value: 0.6, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/perc_0_6.png, caption: "60% habitat", stats: { "Largest cluster": "45.8% of the habitat", "Crosses the map": "yes" } }
  - { value: 0.625, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/perc_0_625.png, caption: "62.5% habitat", stats: { "Largest cluster": "85.6% of the habitat", "Crosses the map": "yes" } }
  - { value: 0.65, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/perc_0_65.png, caption: "65% habitat", stats: { "Largest cluster": "93.5% of the habitat", "Crosses the map": "yes" } }
  - { value: 0.7, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/perc_0_7.png, caption: "70% habitat", stats: { "Largest cluster": "98.1% of the habitat", "Crosses the map": "yes" } }
chart:
  type: line
  data: /data/tutorials/neutral-landscapes/neutral-landscape-models/percolation_frames.csv
  x: prob
  y: [largest]
  labels: { largest: "Largest cluster, % of the habitat" }
  colors: { largest: "#1f5f3a" }
  ylabel: "% of the habitat"
```

Over ten replicates (seeds 1 to 10) and both neighbour rules, the curve is the same:

```tfchart
type: line
data: /data/tutorials/neutral-landscapes/neutral-landscape-models/percolation.csv
x: habitat
y: [n4, n8]
labels: { n4: "4 neighbours", n8: "8 neighbours" }
colors: { n4: "#3b8a5a", n8: "#e8901f" }
xlabel: "Habitat (% of the map)"
ylabel: "Largest cluster (% of the habitat)"
caption: "The share of the habitat in its largest cluster, the mean of 10 random maps of 200 × 200 cells at each amount. With 8 neighbours the habitat connects between 37.5 and 42.5% (the threshold 40.7%); with 4, between 57.5 and 62.5% (59.3%). A few percent of habitat more or less, near the threshold, decide whether the landscape is one network or dust."
```

With & Crist (1995) and With (1997) showed that species respond to such thresholds, at amounts that depend on how
they move: a species that crosses gaps of one cell sees the 8-neighbour curve, or an even lower one. And real habitat,
being aggregated, connects at lower amounts than random habitat: the next sections draw such landscapes.

### Percolation in Python

[`tf.nlm.percolation`][terraflux.nlm.percolation] gives 0 and 1 (habitat). The clusters are measured with
[`tf.landscape_metrics`][terraflux.landscape_metrics]: `directions=4` connects cells by their sides, and the largest
patch index `lpi` (a share of the whole map) over `pland` is the largest cluster's share of the habitat, the numbers
of the slider above:

```python
def class_values(m, cls, names, **options):
    t = tf.landscape_metrics(m, level="class", metric=names, **options)
    return t[t["class"] == cls].set_index("metric")["value"]


for prob in [0.45, 0.5, 0.55, 0.575, 0.6, 0.625, 0.65, 0.7]:
    m = tf.nlm.percolation(200, 200, prob=prob, seed=1)
    v = class_values(m, 1, ["pland", "lpi"], directions=4)
    print(f"{prob:5.3f}  habitat {v['pland']:5.1f} %  largest cluster {100 * v['lpi'] / v['pland']:5.1f} % of it")
```

```text
0.450  habitat  45.1 %  largest cluster   0.7 % of it
0.500  habitat  50.2 %  largest cluster   2.1 % of it
0.550  habitat  55.2 %  largest cluster   4.0 % of it
0.575  habitat  57.6 %  largest cluster  32.0 % of it
0.600  habitat  60.1 %  largest cluster  45.8 % of it
0.625  habitat  62.7 %  largest cluster  85.6 % of it
0.650  habitat  65.1 %  largest cluster  93.5 % of it
0.700  habitat  70.1 %  largest cluster  98.1 % of it
```

`prob` is a probability per cell, so the habitat actually drawn differs a little from it (57.6 % for 0.575). With
`directions=8` the jump moves to about 40 %, as in the chart above.

## Settings that change the pattern

### The random cluster's p

[[nlm.random_cluster]] (Saura & Martínez-Millán, 2000) first makes a random map with a share `p` of cells, joins
them into clusters, gives each cluster a class drawn with the `weights`, and fills the remaining cells with the
classes of their neighbours. `p` decides the size of the clusters.

```tfslider
label: p
caption: "Random clusters of two classes (habitat green, weights 0.4 and 0.6), with p from 0.05 to 0.65, the same seed. A small p makes many small, ragged patches; a larger p fewer and larger ones. Above the percolation threshold (0.59) one cluster takes almost the whole map and the weights are lost."
frames:
  - { value: 0.05, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/rc_0_05.png, caption: "p = 0.05", stats: { Habitat: "38.9%", Patches: "766", "Aggregation (AI)": "53.7%" } }
  - { value: 0.15, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/rc_0_15.png, caption: "p = 0.15", stats: { Habitat: "39.2%", Patches: "596", "Aggregation (AI)": "67.2%" } }
  - { value: 0.25, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/rc_0_25.png, caption: "p = 0.25", stats: { Habitat: "38.1%", Patches: "462", "Aggregation (AI)": "72.1%" } }
  - { value: 0.35, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/rc_0_35.png, caption: "p = 0.35", stats: { Habitat: "37.6%", Patches: "358", "Aggregation (AI)": "76.7%" } }
  - { value: 0.45, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/rc_0_45.png, caption: "p = 0.45", stats: { Habitat: "36.8%", Patches: "272", "Aggregation (AI)": "81.6%" } }
  - { value: 0.55, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/rc_0_55.png, caption: "p = 0.55", stats: { Habitat: "34.7%", Patches: "186", "Aggregation (AI)": "88.4%" } }
  - { value: 0.65, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/rc_0_65.png, caption: "p = 0.65: past the threshold", stats: { Habitat: "96.1%", Patches: "1", "Aggregation (AI)": "98.7%" } }
```

Two warnings come with it, both in the operator's description. The weights give the shares only roughly, because
each cluster takes its class as a whole: the habitat's share is close to its weight (0.4), not equal to it, and
drifts as `p` grows and the clusters become fewer and larger (from 39 to 35%). When the amount must be exact, cut a
surface with [[nlm.binarize]] instead. And `p` must stay below the percolation threshold of its neighbourhood (about
0.59 with the default of 4 neighbours), or one cluster spans the map and a single class takes most of it.

### The roughness of a fractal

[[nlm.fbm]] draws fractional Brownian motion; its `dimension`, between 0 and 2, sets how quickly the surface varies:
small values give rough surfaces, large ones smooth hills. Cut at the same share of habitat with [[nlm.binarize]],
the same 40% of habitat goes from dust to a few large blocks.

```tfslider
label: dimension
caption: "Fractal surfaces (left) of growing smoothness, each cut to exactly 40% habitat (right), the same seed. The amount is the same in every frame; only the fragmentation changes, from 812 patches to 167."
frames:
  - { value: 0.2, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/fbm_0_2.png, caption: "dimension = 0.2", stats: { Habitat: "40%", Patches: "812", "Aggregation (AI)": "55.3%" } }
  - { value: 0.6, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/fbm_0_6.png, caption: "dimension = 0.6", stats: { Habitat: "40%", Patches: "723", "Aggregation (AI)": "63.9%" } }
  - { value: 1.0, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/fbm_1_0.png, caption: "dimension = 1.0", stats: { Habitat: "40%", Patches: "516", "Aggregation (AI)": "74.1%" } }
  - { value: 1.4, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/fbm_1_4.png, caption: "dimension = 1.4", stats: { Habitat: "40%", Patches: "343", "Aggregation (AI)": "83.7%" } }
  - { value: 1.8, src: /img/tutorials/neutral-landscapes/neutral-landscape-models/fbm_1_8.png, caption: "dimension = 1.8", stats: { Habitat: "40%", Patches: "167", "Aggregation (AI)": "90.8%" } }
```

This pair, a surface whose smoothness sets the fragmentation and a cut that sets the amount, is the workhorse of
experiments on habitat amount and fragmentation (With & Crist, 1995): part 2 builds a full design with it.

```tfgraph
#! caption: A fractal surface cut to 40% habitat, and the class metrics of the habitat.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "surface"
op = "nlm.fbm"
params = { rows = 200, columns = 200, resolution = 30, dimension = 1.4, seed = 3 }

[[nodes]]
id = "habitat"
op = "nlm.binarize"
inputs = { map = "surface.map" }
params = { breaks = 0.4 }
note = "The highest 40% of the cells: habitat (2); the rest matrix (1)."

[[nodes]]
id = "metrics"
op = "metrics.class"
inputs = { map = "habitat.map" }
params = { metrics = ["pland", "np", "ai"], show = "ai" }
```

### The range of a Gaussian field

[[nlm.gaussian_field]]'s `range` is the distance, in cells, over which values stay alike: the size of its hills.
A `nugget` adds independent noise on top (default 0.2 of a variance of 5).

```tfgallery
columns: 3
caption: "Gaussian fields of range 3, 10 and 30 cells, the same seed. The range sets the grain of the landscape: a suitability that changes over a few cells or over a region."
images:
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/gf_3.png, caption: "range = 3" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/gf_10.png, caption: "range = 10" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/gf_30.png, caption: "range = 30" }
```

### From surfaces to maps

```tfgallery
columns: 4
caption: "A gradient from the west with fractal noise merged on it (nlm.merge, scaling 0.8), then cut three ways: into three classes of 50, 30 and 20% (nlm.classify with weights), into four classes by natural breaks (nlm.classify with n = 4: 23, 28, 25 and 24%), and into 30% habitat (nlm.binarize)."
images:
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/u_merged.png, caption: "nlm.merge" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/u_classes.png, caption: "nlm.classify, weights" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/u_jenks.png, caption: "nlm.classify, n = 4" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscape-models/u_habitat.png, caption: "nlm.binarize, 30%" }
```

```tfgraph
#! caption: A gradient with noise, cut into three classes: a land cover that changes along a climate.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "gradient"
op = "nlm.planar_gradient"
params = { rows = 200, columns = 200, direction = 90 }

[[nodes]]
id = "noise"
op = "nlm.fbm"
params = { rows = 200, columns = 200, dimension = 1.2, seed = 5 }

[[nodes]]
id = "merged"
op = "nlm.merge"
inputs = { primary = "gradient.map", secondary = "noise.map" }
params = { scaling = 0.8 }
note = "The gradient plus 0.8 times the noise, rescaled to 0-1."

[[nodes]]
id = "classes"
op = "nlm.classify"
inputs = { map = "merged.map" }
params = { weights = [0.5, 0.3, 0.2] }
note = "Classes 1, 2, 3 of 50, 30 and 20% of the cells."
```

[[nlm.merge]] takes the main surface on `primary` and one or more on `secondary`; the `scaling` weighs the
secondary ones, so 0.8 keeps the gradient visible under the noise. [[nlm.classify]]'s output is `classes`; with
its input `real` connected to a real map, it takes the shares of that map's classes (part 2 uses it).

### Settings in Python

The settings of the sections above, in the order they came. `p` of the random cluster (two classes, weights
`ai = [0.4, 0.6]`, seed 3; class 1 is the habitat), with the metrics of the habitat from the helper above:

```python
for p in (0.05, 0.25, 0.45, 0.55, 0.65):
    m = tf.nlm.random_cluster(200, 200, p=p, ai=[0.4, 0.6], seed=3, rescale=False)
    v = class_values(m, 1, ["pland", "np", "ai"])
    print(f"p {p:4.2f}  habitat {v['pland']:5.1f} %  patches {v['np']:4.0f}  AI {v['ai']:5.1f} %")
```

```text
p 0.05  habitat  38.9 %  patches  766  AI  53.7 %
p 0.25  habitat  38.1 %  patches  462  AI  72.1 %
p 0.45  habitat  36.8 %  patches  272  AI  81.6 %
p 0.55  habitat  34.7 %  patches  186  AI  88.4 %
p 0.65  habitat  96.1 %  patches    1  AI  98.7 %
```

The fractal's `fract_dim`, with the surface cut to 40 % by [`tf.nlm.binarize`][terraflux.nlm.binarize] (class 2 is the
habitat), and the Gaussian field's `autocorr_range`, measured as the correlation between cells 5 apart:

```python
for dim in (0.2, 0.6, 1.0, 1.4, 1.8):
    surface = tf.nlm.fbm(200, 200, resolution=30, fract_dim=dim, seed=3)
    habitat = tf.nlm.binarize(surface, 0.4)
    v = class_values(habitat, 2, ["pland", "np", "ai"])
    print(f"fract_dim {dim:3.1f}  habitat {v['pland']:4.1f} %  patches {v['np']:4.0f}  AI {v['ai']:5.1f} %")

for r in (3, 10, 30):
    f = tf.nlm.gaussian_field(200, 200, autocorr_range=r, seed=4).values
    print(f"autocorr_range {r:2d}  correlation of cells 5 apart {np.corrcoef(f[:, :-5].ravel(), f[:, 5:].ravel())[0, 1]:5.2f}")
```

```text
fract_dim 0.2  habitat 40.0 %  patches  812  AI  55.3 %
fract_dim 0.6  habitat 40.0 %  patches  723  AI  63.9 %
fract_dim 1.0  habitat 40.0 %  patches  516  AI  74.1 %
fract_dim 1.4  habitat 40.0 %  patches  343  AI  83.7 %
fract_dim 1.8  habitat 40.0 %  patches  167  AI  90.8 %
autocorr_range  3  correlation of cells 5 apart  0.17
autocorr_range 10  correlation of cells 5 apart  0.58
autocorr_range 30  correlation of cells 5 apart  0.77
```

The three utilities, on the gradient with noise: [`tf.nlm.merge`][terraflux.nlm.merge] (`primary + scaling * sum(secondary)`,
rescaled to 0–1 unless `rescale=False`), [`tf.nlm.classify`][terraflux.nlm.classify] by `weighting` (shares, cumulative
and relative to their sum; a `None` takes what is left) or by natural breaks with `n`, and `tf.nlm.binarize`, whose
`breaks` may be a list:

```python
gradient = tf.nlm.planar_gradient(200, 200, direction=90)
noise = tf.nlm.fbm(200, 200, fract_dim=1.2, seed=5)
merged = tf.nlm.merge(gradient, noise, scaling=0.8)


def shares(m):
    values, counts = np.unique(m.values, return_counts=True)
    return dict(zip(values.astype(int).tolist(), np.round(100 * counts / m.size, 1).tolist()))


print("weighting", shares(tf.nlm.classify(merged, weighting=[0.5, 0.3, 0.2])))
print("n = 4    ", shares(tf.nlm.classify(merged, n=4)))
print("binarize ", shares(tf.nlm.binarize(merged, 0.3)))
```

```text
weighting {1: 50.0, 2: 30.0, 3: 20.0}
n = 4     {1: 22.8, 2: 27.9, 3: 25.1, 4: 24.2}
binarize  {1: 70.0, 2: 30.0}
```

Class 1 is the lowest values: with `binarize` the highest values become the habitat (2). A list of `breaks` gives a
stack with a dimension `p` (`maps.sel(p=0.4)`, part 2 uses it). The app's input `real` is `real_land=` of
[`tf.nlm.classify`][terraflux.nlm.classify]: a real map whose class shares the classes copy (also in part 2);
`level_names` names the classes, and `mask_val` marks a value to leave out.

## Reproducing NLMR

Every model follows its NLMR counterpart value for value, and the seed is NLMR's `user_seed`:

<!-- no-run -->
```r
# R
library(NLMR)
r <- nlm_mpd(ncol = 129, nrow = 129, roughness = 0.7, user_seed = 42)
```

```python
# Python: the same values
m = tf.nlm.mpd(129, 129, roughness=0.7, seed=42)
print(m.shape, m.attrs["seed"], round(float(m.mean()), 6))
```

```text
(129, 129) 42 0.537649
```

A few models behave in NLMR differently from what their documentation says. `nlmr_compat=True` (the default) keeps
these behaviours so that results match, and `nlmr_compat=False` does what the documentation promises (the gallery
above used it for `mpd` and `distance_gradient`):

| Model | NLMR's behaviour (`nlmr_compat=True`) | `nlmr_compat=False` |
|---|---|---|
| `mpd` | square output of `max(nrow, ncol) ± 1` cells | the size asked for |
| `distance_gradient` | `origin` as rows then columns; great-circle distances in metres for rasters up to 360 × 90 cells | origin as columns then rows; Euclidean distances |
| `mosaic_field` | `n = 1` sums three mosaics | one mosaic |
| `mosaic_tess`, `mosaic_gibbs` | values given in GEOS's order of the Voronoi cells (with shapely) | each germ its own value |

`nlm_fbm` and `nlm_gaussianfield` print a message in NLMR 1.4 (RandomFields left CRAN); Terraflux gives the values of
the replacements NLMR ships internally (`simulate_fbm`, `simulate_gaussian_field`).

The models run in C++ with OpenMP. The random numbers are drawn in NLMR's order, and everything else runs in
parallel: the fast Fourier transforms of the spectral models, the diamond-square steps, the labelling and gap filling
of the cluster models, the rasterisation of the mosaics. Typical gains over NLMR are 2 to 8 times for the models NLMR
computes in C++ or with FFTs, and 100 to 6000 times for those it computes cell by cell in R (`percolation`,
`random_cluster`, `mosaic_field`); see the [benchmarks](/benchmarks/speed/#neutral-landscape-models).

## Common pitfalls

- **One map is not an answer.** A neutral landscape is one draw; a conclusion needs replicates (different seeds)
  and their spread (part 2).
- **Forgetting the cell size.** Without `resolution`, a cell is 1 unit wide: metrics in hectares are then
  meaningless. Give the cell size of the real maps you compare with.
- **Comparing amounts that drift.** Random clusters and percolation give approximately the share asked for; cut a
  surface with [[nlm.binarize]] when the amount must be exact.
- **Random cluster above the threshold**, above.
- **Reading a neutral pattern as realistic.** Fractal surfaces have no roads, rivers or straight field boundaries;
  real landscapes made by people are often more regular than any of these models.
- **The same seed everywhere.** Two nodes with the same seed and settings draw the same map: vary the seeds of
  replicates, and of nodes that should be independent.

## Going further

- [Part 2](/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/): neutral landscapes as an experiment and
  as a null model, with replicates.
- [Landscape metrics](/tutorials/landscape-metrics/): measuring the patterns drawn here.
- [Fragmentation](/tutorials/fragmentation/fragmentation-and-habitat-loss/): habitat loss and fragmentation, and
  the thresholds of real landscapes.
- The Python reference: [`tf.nlm`](/python/api/nlm/), with the models of this part and their NLMR equivalents.
- The operator reference: [[nlm.random]], [[nlm.percolation]], [[nlm.random_cluster]], [[nlm.fbm]],
  [[nlm.gaussian_field]], [[nlm.mpd]], [[nlm.binarize]], [[nlm.classify]], [[nlm.merge]].

## References

- Etherington, T. R., Holland, E. P., & O'Sullivan, D. (2015). NLMpy: a Python software package for the creation of neutral landscape models within a general numerical framework. *Methods in Ecology and Evolution*, 6(2), 164–168. https://doi.org/10.1111/2041-210X.12308
- Fahrig, L. (2003). Effects of habitat fragmentation on biodiversity. *Annual Review of Ecology, Evolution, and Systematics*, 34, 487–515. https://doi.org/10.1146/annurev.ecolsys.34.011802.132419
- Gardner, R. H., Milne, B. T., Turner, M. G., & O'Neill, R. V. (1987). Neutral models for the analysis of broad-scale landscape pattern. *Landscape Ecology*, 1(1), 19–28. https://doi.org/10.1007/BF02275262
- Keitt, T. H. (2000). Spectral representation of neutral landscapes. *Landscape Ecology*, 15(5), 479–494. https://doi.org/10.1023/A:1008193015770
- Palmer, M. W. (1992). The coexistence of species in fractal landscapes. *The American Naturalist*, 139(2), 375–397. https://doi.org/10.1086/285332
- Saura, S., & Martínez-Millán, J. (2000). Landscape patterns simulation with a modified random clusters method. *Landscape Ecology*, 15(7), 661–678. https://doi.org/10.1023/A:1008107902848
- Sciaini, M., Fritsch, M., Scherer, C., & Simpkins, C. E. (2018). NLMR and landscapetools: an integrated environment for simulating and modifying neutral landscape models in R. *Methods in Ecology and Evolution*, 9(11), 2240–2248. https://doi.org/10.1111/2041-210X.13076
- Stauffer, D., & Aharony, A. (1994). *Introduction to percolation theory* (2nd ed.). Taylor & Francis.
- With, K. A. (1997). The application of neutral landscape models in conservation biology. *Conservation Biology*, 11(5), 1069–1080. https://doi.org/10.1046/j.1523-1739.1997.96210.x
- With, K. A., & Crist, T. O. (1995). Critical thresholds in species' responses to landscape structure. *Ecology*, 76(8), 2446–2459. https://doi.org/10.2307/2265819
