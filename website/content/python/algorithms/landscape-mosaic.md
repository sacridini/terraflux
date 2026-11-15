---
title: Landscape mosaic, contagion and clustering
description: "The landscape mosaic of natural, agricultural and developed cover, its heatmap, the foreground contagion (Pff) and the area clustering (FAC)."
order: 40
---

The same in the app: [Fragmentation and habitat loss](/tutorials/fragmentation/fragmentation-and-habitat-loss/) and [Change and reports](/tutorials/fragmentation/change-and-reports/) (the landscape mosaic and its heatmap). API: [`tf.landscape_mosaic`][terraflux.landscape_mosaic], [`tf.landscape_mosaic_stats`][terraflux.landscape_mosaic_stats], [`tf.landscape_mosaic_heatmap`][terraflux.landscape_mosaic_heatmap], [`tf.contagion_pff`][terraflux.contagion_pff] and [`tf.clustering_fac`][terraflux.clustering_fac], on the [fragmentation](/python/api/fragmentation/) page. Checked against GuidosToolbox's `recode` and `spatcon` programs and IDL code (see [how each tool is checked](/about/validation/#results-by-tool)).

The landscape mosaic analyzes spatial mixing between three landcover components:

1. **Natural (N)**
2. **Agriculture (A)**
3. **Developed / anthropic (D)**

## Landscape mosaic (LM)

The input is coded 0 missing, 1 agriculture, 2 forest (natural) and 3 developed. The example map of the library has only the first three codes, which is enough to run it.

```python
import terraflux as tf

data = tf.load_raster(tf.example_data("input"))

# 19-class or 103-class Landscape Mosaic
lm19 = tf.landscape_mosaic(data, window_size=27, output_classes=19)
lm103 = tf.landscape_mosaic(data, window_size=27, output_classes=103)

# 103-class barycentric statistics breakdown
stats = tf.landscape_mosaic_stats(lm103, is_lm103=True)
print(stats)

# GTB heatmap: share [%] of each of the 103 classes, as GTB stores it (single precision)
heat = tf.landscape_mosaic_heatmap(data, window_size=27)
heat.table          # share per class code, with its 19-class recode
heat.triangle       # 12 x 21 grid in GTB's triangle layout (NaN outside)
heat.to_csv("forest_lm_27_heatmap.csv")   # identical to GTB's heatmap.csv
```

## Contagion (Pff) and area clustering (FAC)

\(P_{ff}\) is the pixel co-occurrence probability of foreground; the clustering divides it by the foreground density \(P_f\).

```python
# Pff: pixel co-occurrence probability of foreground
pff = tf.contagion_pff(data, window_size=7)

# FAC: Foreground Area Clustering normalized by density (Pff / Pf)
fac = tf.clustering_fac(data, window_size=7)
```

## References

- Riitters, K., Wickham, J., O'Neill, R., Jones, B., & Smith, E. (2000). Global-scale patterns of forest fragmentation. *Conservation Ecology*, 4(2), 3.
- Riitters, K. H., et al. (1995). A factor analysis of landscape pattern and structure metrics. *Landscape Ecology*, 10(1), 23–39.
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
