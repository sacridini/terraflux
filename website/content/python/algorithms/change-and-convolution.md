---
title: Change detection and spatial convolution
description: "Simple and morphological change, FOS and FAD-MS change, the landscape mosaic heatmap change, and moving-window metrics of grayscale maps."
order: 60
---

The same in the app: [Change and reports](/tutorials/fragmentation/change-and-reports/), [Networks and change](/tutorials/habitat-patterns/networks-and-change/), [Forest gain and loss](/tutorials/change-detection/forest-gain-and-loss/) and [Comparing maps](/tutorials/change-detection/comparing-maps/). The same as a worked tutorial in Python: [Change detection in Python](/tutorials/change-detection/change-detection-in-python/). API: [`tf.simple_change`][terraflux.simple_change], [`tf.morphological_change`][terraflux.morphological_change], [`tf.fos_change`][terraflux.fos_change], [`tf.fad_multiscale_change`][terraflux.fad_multiscale_change] and [`tf.landscape_mosaic_change`][terraflux.landscape_mosaic_change] on the [change detection](/python/api/change/) page, [`tf.gray_spatial_convolution`][terraflux.gray_spatial_convolution] on the [fragmentation](/python/api/fragmentation/) page. Checked against GuidosToolbox's IDL code and `grayspatcon` (see [how each tool is checked](/about/validation/#results-by-tool)).

The examples use the example map of the library as the map of the first date, a copy of it with a clear-cut and a plantation as the second, and a grayscale cover map for the convolution.

```python
import terraflux as tf

map_t1 = tf.load_raster(tf.example_data("input"))   # 0 missing, 1 background, 2 foreground
map_t2 = map_t1.copy()
cut = map_t2.values[600:700, 100:260]
cut[cut == 2] = 1                                   # a clear-cut
plantation = map_t2.values[250:330, 420:520]
plantation[plantation == 1] = 2                     # a new plantation

fos_t1 = tf.fos(map_t1, window_size=27).map         # the maps of the fixed observation scale
fos_t2 = tf.fos(map_t2, window_size=27).map
forest_2000, forest_2020 = map_t1, map_t2
lm_input_2000, lm_input_2020 = map_t1, map_t2
img = tf.load_raster(tf.example_data("gscinput"), validate=None)   # 0-100 % cover
```

## Simple change (A to B)

Computes the 4-class binary transition:

```python
sc = tf.simple_change(map_t1, map_t2)
print(sc.summary())
# sc.change_map values:
# 11: Stable Background
# 12: Foreground Gain
# 21: Foreground Loss
# 22: Stable Foreground
# 255: Missing / Nodata
```

## Morphological change detection (MCD)

Eliminates spurious boundary chatter through geodesic reconstruction of core changes (\(d > 2\)) and computes landscape elasticity:

```python
mcd = tf.morphological_change(map_t1, map_t2)
print(mcd.summary())
# mcd.change_map values:
# 11: Stable BG
# 12: MCD Gain (real structural expansion)
# 21: MCD Loss (real structural loss)
# 22: Stable FG
# 176: Spurious boundary chatter
# mcd.elasticity: ratio of core change to total area change
```

## Fragmentation (FOS) change

Produces continuous delta FOS maps and a \(6 \times 6\) density class transition matrix:

```python
fc = tf.fos_change(fos_t1, fos_t2)
print(fc.summary())
print(fc.matrix_6x6)
# - fc.change_map: delta FOS shifted by +100
# - fc.frag_decrease_pct: % of shifts with reduced fragmentation
# - fc.frag_increase_pct: % of shifts with increased fragmentation
```

## FAD multiscale change

GTB's *Change → FAD Multiscale*: for every FAD-MS observation scale and the multiscale map,
a 7×7 table of pixels moving between fragmentation classes (background, rare, patchy,
transitional, dominant, interior, intact) from time A to time B:

```python
ch = tf.fad_multiscale_change(forest_2000, forest_2020)   # maps, FAD-MS results or GTB *_fad_mscale.tif
sc = ch.scales["mscale"]            # or ch.scales[7], ch.scales[13], ...
sc.matrix                           # 7x7 pixel counts, rows A0-Background..A6-Intact, columns B0..B6
sc.percent                          # 6x6 foreground transitions in % of the changed pixels
sc.frag_decrease_pct, sc.frag_increase_pct   # above / below the matrix diagonal
ch.averages                         # FAD_AV and AVCON at A and B per scale, and their change
print(ch.summary("mscale"))         # GTB FAD_change_mscale.txt layout
```

Both maps must have the same size, and either both or neither may contain non-fragmenting
background (input 4). The tables are identical to GTB's.

## Landscape mosaic heatmap change

GTB's *Change → Landscape Mosaic Heatmap*: the difference B − A of two heatmaps.

```python
d = tf.landscape_mosaic_change(lm_input_2000, lm_input_2020, window_size=27)
d.table            # percent_a, percent_b, change per LM class
d.to_csv("delta_heatmap.csv")   # identical to GTB's delta_heatmap.csv
```

## Continuous grayscale spatial convolution (GraySpatCon)

Computes moving-window spatial metrics over continuous grayscale/percentage rasters (\(0..100\)); the window sums run on the 101-bin sliding histograms described in [fragmentation and distance](/python/algorithms/fragmentation-and-distance/):

```python
gsc_mean = tf.gray_spatial_convolution(img, metric="mean", window_size=7)
gsc_std = tf.gray_spatial_convolution(img, metric="stddev", window_size=7)
gsc_ent = tf.gray_spatial_convolution(img, metric="shannon_diversity", window_size=7)
```

**Supported metrics**:

* `"mean"` / `"average"`
* `"median"`
* `"majority"` / `"mode"`
* `"min"` / `"max"` / `"range"`
* `"stddev"`
* `"shannon_diversity"`
* `"shannon_evenness"`
* `"diagonal_contagion"`

## References

- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Riitters, K., Wickham, J., O'Neill, R., Jones, B., & Smith, E. (2000). Global-scale patterns of forest fragmentation. *Conservation Ecology*, 4(2), 3.
- Shannon, C. E. (1948). A mathematical theory of communication. *The Bell System Technical Journal*, 27(3), 379–423.

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
