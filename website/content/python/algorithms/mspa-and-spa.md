---
title: MSPA and SPA
description: "Morphological Spatial Pattern Analysis, its tiling for maps larger than memory, the simplified SPA variants, the class codes and the class statistics."
order: 10
---

The same in the app: [MSPA step by step on a real forest map](/tutorials/habitat-patterns/mspa/), [Simplified patterns, objects and distances](/tutorials/habitat-patterns/spa-and-objects/). The same as a worked tutorial in Python: [Habitat patterns in Python](/tutorials/habitat-patterns/patterns-in-python/). API: [`tf.mspa`][terraflux.mspa], [`tf.mspa_stats`][terraflux.mspa_stats], [`tf.spa`][terraflux.spa], [`tf.mspa_tiled`][terraflux.mspa_tiled] and [`tf.tile_layout`][terraflux.tile_layout], on the [habitat patterns](/python/api/patterns/) and [large data](/python/api/batch/) pages. Checked against GuidosToolbox's `mspa` executable: identical pixels (see [how each tool is checked](/about/validation/#results-by-tool)).

The examples on this page use the example map of the library, coded 0 missing, 1 background, 2 foreground:

```python
import terraflux as tf

data = tf.load_raster(tf.example_data("input"))   # 1000 x 1000, uint8
```

## MSPA

MSPA segments foreground habitat into 7 mutually exclusive morphological classes and openings based on geometric connectivity and distance to boundaries.

```python
result = tf.mspa(
    data,
    connectivity=8,   # Foreground connectivity: 8 (default) or 4
    edge_width=1,     # Effective edge width in pixels (1 to 255)
    transition=True,  # Transition parameter for color styling
    internal=True,    # Identifies internal perforations and core openings
    n_threads=None,   # OpenMP threads (None / 0: most of the cores)
)
```

## MSPA Tiling (maps larger than memory)

GTB's *MSPA Tiling*: overlapping tiles processed one by one (or `n_workers` at a time) and put back together.
A GeoTIFF path is processed out of core, reading and writing one tile window at a time:

```python
tf.write_raster(data, "europe_forest.tif")      # a GeoTIFF of your own; here the example map, written to a file
out = tf.mspa_tiled("europe_forest.tif")        # -> europe_forest_8_1_1_1.tif (GTB naming)
out = tf.mspa_tiled("europe_forest.tif", "mspa.tif", connectivity=4, n_workers=4, overwrite=True)
m = tf.mspa_tiled(data, tile_size=6500, overlap=2000)  # arrays / DataArrays in memory
tf.tile_layout(20000)  # [(read_start, read_stop, keep_start, keep_stop), ...] per tile along an axis
```

Tiles are `tile_size` pixels including the `overlap` (GTB: 6500 and 2000 on MS-Windows, 9500 on Linux,
8500 on macOS; GTB fixes the edge width to 1); each tile keeps its half of every overlap. Tiles with only
missing data, only foreground, or lacking background or foreground are classified directly, as in GTB.
The result is identical to GTB's MSPA Tiling. It equals MSPA of the whole map when the objects are small
compared with the overlap; otherwise classes that depend on whole objects (loops, bridges, islets, openings)
can differ near the seams — use `tf.mspa()` on the whole map whenever it fits in memory (about 32 bytes per pixel).

## Simplified Pattern Analysis (SPA2/3/5/6)

GTB's simplified alternatives to MSPA (8-connected foreground, 1-pixel edge width):

```python
r = tf.spa(data, variant=5)   # 2, 3, 5 or 6
r.map       # 17 core (SPA2: contiguous), 1 margin (SPA2: small linear features), 3 edge,
            # 5 perforation, 9 islet (SPA6), 100 core-opening, 0 background, 129 missing
r.stats     # area, % of data, % of foreground per class
r.counts, r.porosity
print(r.summary())
```

Identical to GTB (maps and statistics).

## MSPA classes table (`tf.MSPA_CLASSES`)

| Pixel Value | Class Name | Description |
| :---: | :--- | :--- |
| **17** | **Core** | Interior habitat area beyond edge width distance from perimeter. |
| **9** | **Islet** | Disconnected habitat patch too small to contain core. |
| **5** | **Perforation** | Internal boundary pixels surrounding interior openings. |
| **3** | **Edge** | External perimeter boundary pixels of habitat patches. |
| **33 / 35 / 37** | **Bridge** | Structural corridor connecting two or more distinct Core patches. |
| **65 / 67 / 69** | **Loop** | Linear corridor connecting back to the same Core patch. |
| **1** | **Branch** | Linear corridor connected to Core at one end and terminating in matrix. |
| **100** | **Core-opening** | Background perforation completely enclosed within Core. |
| **220** | **Border-opening** | Background perforation touching the external boundary. |
| **+100** | **Internal feature** | Any feature situated inside an interior perforation (`internal=True`). |

## Class statistics


```python
df_stats = tf.mspa_stats(result)
print(df_stats)
```

## References

- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
- Vogt, P., Riitters, K. H., Iwanowski, M., Estreguil, C., Kozak, J., & Soille, P. (2007). Mapping landscape corridors. *Ecological Indicators*, 7(2), 481–488. https://doi.org/10.1016/j.ecolind.2006.11.001
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
