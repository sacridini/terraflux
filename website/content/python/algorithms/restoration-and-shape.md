---
title: Restoration and morphological shape
description: "Restoration status, cost distance and corridors, accounting by object size, isochrone maps, parcellation and habitat contortion."
order: 50
---

The same in the app: [Costs and corridors](/tutorials/distance-restoration/costs-and-corridors/), [Restoration plan](/tutorials/distance-restoration/restoration-plan/), [Ecological intactness](/tutorials/distance-restoration/ecological-intactness/) and [Simplified patterns, objects and distances](/tutorials/habitat-patterns/spa-and-objects/). The same as worked tutorials in Python: [Restoration plans in Python](/tutorials/distance-restoration/restoration-plans-in-python/) and [Habitat patterns in Python](/tutorials/habitat-patterns/patterns-in-python/). API: [`tf.restoration_status`][terraflux.restoration_status], [`tf.cost_distance`][terraflux.cost_distance], [`tf.least_cost_path`][terraflux.least_cost_path], [`tf.restoration_corridor`][terraflux.restoration_corridor], [`tf.isochrone_map`][terraflux.isochrone_map], [`tf.isochrone_map_ab`][terraflux.isochrone_map_ab] and [`tf.distance_to_resistance`][terraflux.distance_to_resistance] on the [distance and restoration](/python/api/distance/) page, [`tf.accounting`][terraflux.accounting], [`tf.parcellation`][terraflux.parcellation] and [`tf.contortion`][terraflux.contortion] on the [habitat patterns](/python/api/patterns/) page. Checked against GuidosToolbox's IDL code and its `ggeo` program (see [how each tool is checked](/about/validation/#results-by-tool)).

The examples use the example map of the library, an MSPA map of it, a resistance map and two markers: the two largest forest objects, A and B, as the places to connect.

```python
import numpy as np
import terraflux as tf

data = tf.load_raster(tf.example_data("input"))        # 0 missing, 1 background, 2 foreground
mspa_map = tf.mspa(data, edge_width=2)
resistance = tf.distance_to_resistance(data)           # 2 foreground (cost 1), 3-100 background resistance

objects = tf.accounting(data)                          # the two largest forest objects
ids = objects.ids.values
marker = np.zeros(data.shape, dtype="uint8")
marker[ids == objects.largest[0][0]] = 1               # object A
marker[ids == objects.largest[1][0]] = 2               # object B
marker[data.values == 0] = 3                           # missing: not crossed
```

## Restoration status (RSS)

Evaluates landscape connectivity before and after hypothetical restoration of all non-core pixels:

```python
rss = tf.restoration_status(mspa_map)
print(rss.summary())
# Attributes:
# - eca, doc            (equivalent connected area, degree of coherence)
# - cnoa                (Critical Nodes of Attachment)
# - rest_pot, rac       (restoration potential, Restorable Area of Connectivity)
# - data_area, nr_comp, eca_max, larg_obj, aps
```

## Cost distance and least-cost corridors

Anisotropic 8-neighbor Dijkstra least-cost routing across resistance surfaces (1-100, 0 impassable):

```python
# 1. Cost Distance Map from Source Mask
cost_map = tf.cost_distance(resistance, marker == 1)

# 2. Two-ended Least Cost Path
lcp = tf.least_cost_path(resistance, marker == 1, marker == 2)
print(f"Total path cost: {lcp.expense:.2f}, length: {lcp.path_length} pixels, to restore: {lcp.rest_pixels}")

# 3. Restoration Corridor (Dijkstra forward-backward intersection)
corridor = tf.restoration_corridor(data, resistance, marker == 1, marker == 2)
print(corridor.rest_pixels, corridor.delta_eca, corridor.delta_doc)
```

## Accounting (object size classes)

GTB's *Objects → Accounting*: labels every foreground object and groups it into
up to six size classes. The maps are identical to GTB's `_acc.tif`,
`_acc_ids.tif` and `_acc_pixels.tif`, and the numbers match its `acc.txt` report:

```python
acc = tf.accounting(data, thresholds=(50, 250, 1000, 5000, 20000), connectivity=8,
                    highlight_largest=True)
print(acc.summary())          # GTB acc.txt layout
acc.stats                     # per class: range, colour, objects, area, % objects, % FG area
acc.classes                   # uint8 GTB colour codes (see tf.ACCOUNTING_CLASSES)
acc.ids                       # int32 object IDs (0 BG, -1 missing, -3/-4 special BG)
acc.area                      # int32 object area in pixels on every object pixel
tf.write_raster(acc.classes, "forest_acc.tif")   # with GTB's colour table
```

* Input: `uint8`, 0 = missing, 1 = background, 2 = foreground, 3/4 = special background.
* `thresholds`: 1 to 5 non-decreasing upper bounds (repeats dropped). Class 1 is
  `[1, t1]`, class k is `(t(k-1), tk]`, and the last class holds the larger objects.
  `"auto"` uses the classes GTB's dialog proposes (0.1/1/10/50/90% of the 8-connected
  object-area range).
* IDs are numbered as in GTB: raster order starting from the bottom row, because GTB
  flips the image before `label_region`.
* `highlight_largest` paints the three largest objects pink (code 80), like
  GTB's "big3pink". It needs at least three objects. Objects of equal area share a rank.
* The median is the upper middle value for an even count (IDL `MEDIAN`), and the
  standard deviation is the sample one (IDL `MOMENT`).

## Isochrone maps and restoration setup

GTB's *Restoration Planner*. Resistance maps are coded 2 = foreground (cost 1) and
3–100 = background resistance; marker maps 1 = object A, 2 = object B, 3 = missing.
A step between neighbouring pixels (8-connectivity, diagonals included) costs the mean
of their resistances, as in GTB's `ggeo` program:

```python
resist = tf.distance_to_resistance(data)          # or any 2/3-100 resistance map
iso = tf.isochrone_map(resist, marker)            # cost from A (object B is ignored)
iso.map      # 0-100 scaled cost, 101 markers, 102 missing, 104 unreachable
iso.cost     # cost as GTB rounds it
ab = tf.isochrone_map_ab(resist, marker)          # cheapest A-B route through every pixel
ab.lcp       # least-cost path (1 pixel wide), code 103 in ab.map
```

The isochrone maps are identical to GTB's (tested against GTB's code and its `ggeo`
program). `cost_distance()` / `least_cost_path()` above use a different cost model
(floating point, diagonal steps weighted by √2).

## Landscape parcellation

Computes patch division, effective mesh size, and splitting indices:

```python
parc = tf.parcellation(data, connectivity=8, return_labels=True)
print(parc.summary())
# Class-level metrics table:
# - APS (Average Patch Size)
# - AWAPS (Effective Mesh Size / Area-Weighted APS)
# - DIVISION (Landscape Division Index)
# - PARC (Logarithmic Parcellation Index [%])
```

## Habitat contortion

Calculates boundary tortuosity and perimeter inflection points:

```python
contortion_map = tf.contortion(data, min_area=1)
# Values represent shape tortuosity per component
```

## References

- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Saura, S., & Pascual-Hortal, L. (2007). A new habitat availability index to integrate connectivity in landscape conservation planning: comparison with existing indices and application to a case study. *Landscape and Urban Planning*, 83(2–3), 91–103. https://doi.org/10.1016/j.landurbplan.2007.03.005
- Jaeger, J. A. (2000). Landscape division, splitting index, and effective mesh size: new measures of landscape fragmentation. *Landscape Ecology*, 15(2), 115–130.

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
