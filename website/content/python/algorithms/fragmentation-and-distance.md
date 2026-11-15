---
title: Fragmentation and distance transforms
description: "Forest area density (FAD), the fixed observation scale (FOS), Euclidean distance, influence zones and proximity, and directional entropy."
order: 30
---

The same in the app: [Fixed observation scale](/tutorials/fragmentation/fixed-observation-scale/), [Fragmentation and habitat loss](/tutorials/fragmentation/fragmentation-and-habitat-loss/) and [Change and reports](/tutorials/fragmentation/change-and-reports/). The same as a worked tutorial in Python: [Fragmentation in Python](/tutorials/fragmentation/fragmentation-in-python/). API: [`tf.fad`][terraflux.fad], [`tf.fad_multiscale`][terraflux.fad_multiscale], [`tf.fos`][terraflux.fos], [`tf.spatial_entropy`][terraflux.spatial_entropy] and [`tf.spatial_entropy_map`][terraflux.spatial_entropy_map] on the [fragmentation](/python/api/fragmentation/) page, [`tf.euclidean_distance`][terraflux.euclidean_distance], [`tf.influence_zones`][terraflux.influence_zones] and [`tf.proximity`][terraflux.proximity] on the [distance and restoration](/python/api/distance/) page. Checked against GuidosToolbox's IDL code and its `spatcon` and `grayspatcon` programs (see [how each tool is checked](/about/validation/#results-by-tool)).

Every sliding-window tool here costs the same per pixel whatever the window: FAD uses **2D integral images** (summed-area tables), so each pixel takes \(O(1)\) operations from \(7 	imes 7\) up to \(243 	imes 243\) windows, and the grayscale convolution of [change and convolution](/python/algorithms/change-and-convolution/) uses **101-bin sliding histograms** that update by columns in \(O(1)\) instead of scanning the window again.

The examples use the example map of the library (0 missing, 1 background, 2 foreground):

```python
import terraflux as tf

data = tf.load_raster(tf.example_data("input"))
```

## Forest area density (FAD and FAD multiscale)

Calculates foreground pixel proportion within square moving windows.

```python
# Single window size
fad_res = tf.fad(data, window_size=27)
print(f"Average FAD: {fad_res.stats['average_fad'].iloc[0]:.2f}%")   # fad_res.density: the map, fad_res.stats: the class shares

# Multiscale FAD across 5 default GTB window scales: 7, 13, 27, 81, 243
multi = tf.fad_multiscale(data, window_sizes=[7, 13, 27, 81, 243])
# multi.single_scales maps window_size -> DataArray; multi.multiscale_map is their rounded mean
```

Inputs may also contain GTB's special backgrounds: `3` (special background, coded 105 in the
maps) and `4` (non-fragmenting special background, treated as missing when measuring density,
coded 106). `multi.stats` lists the class shares, `average_fad` and GTB's `avcon`
(average FAD × foreground / non-missing area) per scale. The maps are identical to GTB's FAD-MS
outputs (`*_fad_<window>.tif`, `*_fad_mscale.tif`).

**FAD classes (`tf.FAD_CLASSES`)**:

1. Rare (\(0..9\%\))
2. Patchy (\(10..39\%\))
3. Transitional (\(40..59\%\))
4. Dominant (\(60..89\%\))
5. Interior (\(90..100\%\))

## Fragmentation at a fixed observation scale (FOS)

GTB's *Fragmentation → FOS*, with all its methods and inputs:

```python
r = tf.fos(data, window_size=27, method="FAD", classes=6)   # "FAD", "FED" or "FAC"; 5 or 6 classes
r.map          # 0-100 connectivity of each foreground pixel; 101 BG, 102 missing, 105/106 special BG
r.stats        # share [%] of the foreground per class (Rare .. Interior/Intact)
r.histogram    # share [%] of the foreground pixels with each value 0..100
r.average, r.avcon, r.n_patches, r.average_patch_size, r.eca, r.coherence, r.coherence_ru
print(r.summary())  # GTB's fos.txt report

gray = tf.load_raster(tf.example_data("gscinput"), validate=None)  # 0-100 % cover, 255 missing, 103/104 special BG
r = tf.fos(gray, 13, "FED", 5, gray_threshold=30)   # foreground = cover >= 30
```

* **FAD** (area density): share of foreground in the window (spatcon, or GraySpatCon metric 1 — the mean cover — for grayscale maps).
* **FED** (edge density): GraySpatCon metric 52 ("clustering") of the window's pixel pairs.
* **FAC** (area clustering): spatcon's foreground-foreground adjacency share; binary maps only, as in GTB.

Six classes separate *Intact* (100) from *Interior* (90-99); five merge them (colour table `fadcolors5`).
A map is taken as grayscale when its maximum exceeds 4 (as in GTB) unless `grayscale=` is given. Patch statistics
use `connectivity` (8 or 4). Maps and statistics are identical to GTB's batch FOS.

## Euclidean distance transform

Computes exact Signed Euclidean Distance transform using Meijster's \(O(N)\) algorithm:

```python
dist_res = tf.euclidean_distance(data)
# dist_res.signed_distance : continuous distance (positive inside FG, negative inside BG)
# dist_res.byte_map        : 256-level byte-scaled distance map for GIS visualization
# dist_res.mean_fg_distance: average distance from perimeter to interior (mean_bg_distance for the background)
# dist_res.max_fg_distance : maximum distance to edge (max_bg_distance for the background)
```

## Influence zones and proximity (experimental)

GTB's *Distance → Influence Zones / Proximity*. The objects (8-connected foreground
areas of at least `min_area` pixels, with their holes filled) divide the map into the
zones of the watershed of the distance to the objects:

```python
iz = tf.influence_zones(data, min_area=5000, core_buffer=0, buffer=10)
iz.classes, iz.ids, iz.zones          # GTB viewport codes, object ids, zone labels
px = tf.proximity(data, min_area=5000)
px.distance, px.cag, px.lines         # proximity map, connected area gain, watershed pixels
px.to_csv("input_proximity_5000_0_0.csv")   # GTB's proximity CSV
```

These tools are **not yet identical to GTB**: IDL's `MORPH_DISTANCE`, which GTB uses, does
not always pick the nearest object next to the boundaries between zones, and its exact
rule is still unknown. The objects, ids, holes, buffers and CAG follow GTB's code
line by line.

## Spatial joint directional entropy

Measures textural heterogeneity and spatial order across 4 directions (\(0^\circ, 45^\circ, 90^\circ, 135^\circ\)): the frequency of transitions among foreground, background and missing pixels in the 8 directions of the neighbourhood, normalized to 0-100 %, as GuidosToolbox's `entropy_mspainp` and, for the local map, `batch_ent`:

```python
# Global entropy indices
ent_res = tf.spatial_entropy(data)
print(f"Normalized spatial entropy index: {ent_res.entropy_index:.4f} %")   # ent_res.histogram: the transitions

# Moving-window local entropy map
ent_map = tf.spatial_entropy_map(data, window_size=50)
```

## References

- Vogt, P., Riitters, K. H., Iwanowski, M., Estreguil, C., Kozak, J., & Soille, P. (2007). Mapping landscape corridors. *Ecological Indicators*, 7(2), 481–488. https://doi.org/10.1016/j.ecolind.2006.11.001
- Riitters, K., et al. (2002). Fragmentation of continental United States forests. *Ecosystems*, 5(8), 815–822.
- Meijster, A., Roerdink, J. B., & Hesselink, W. H. (2000). A general algorithm for computing distance transforms in linear time. *Mathematical Morphology and its applications to image and signal processing*, 331–340.
- Riitters, K. H., et al. (1995). A factor analysis of landscape pattern and structure metrics. *Landscape Ecology*, 10(1), 23–39.
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
