---
title: Habitat patterns
description: "The classes of a binary habitat map as GuidosToolbox computes them: MSPA's core, islet, perforation, edge, loop, bridge and branch, SPA's simplified classes and porosity, the parcellation indices and the contortion of objects."
order: 110
---

::: metrics habitat-patterns

## The example map {#example}

The values on the cards were computed on the forest map of GuidosToolbox's sample, the one the habitat patterns
tutorials use: 1000 x 1000 cells of 25 m, coded 0 missing, 1 background and 2 forest. MSPA was run with an edge width of
2 cells (50 m); [`tf.mspa_stats`][terraflux.mspa.mspa_stats] gives the cells of every class and their shares of the
mapped area and of the habitat in one table:

```python
import terraflux as tf

# forest of GuidosToolbox's sample (1000 x 1000 cells of 25 m): 0 missing, 1 background, 2 forest
forest = tf.load_raster(tf.example_data("input"))

morph = tf.mspa(forest, edge_width=2)
print(tf.mspa_stats(morph).round(2).to_string())
print(f"SPA5 porosity {tf.spa(forest, variant=5).porosity:.2f}%")
```

```text
                pixels  pct_data  pct_foreground
class                                           
Core            264641     26.47           61.76
Islet            26040      2.60            6.08
Perforation       9608      0.96            2.24
Edge             59657      5.97           13.92
Loop             20237      2.02            4.72
Bridge           27772      2.78            6.48
Branch           20535      2.05            4.79
Background      514124     51.43             NaN
Core-Opening      2732      0.27             NaN
Border-Opening   54384      5.44             NaN
Missing            270       NaN             NaN
SPA5 porosity 1.42%
```

The forest is 42.9% of the mapped area; its core is 62% of it with an edge of 50 m, and 75% with an edge of 25 m
(`edge_width=1`, the default). The background classes have no share of the habitat, and the missing cells no share of
anything. The seven habitat classes and the ten shares of the mapped area each add up to 100%. SPA's porosity says that
1.42% of the contiguous forest and its holes is hole.
