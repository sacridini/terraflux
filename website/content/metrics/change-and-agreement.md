---
title: Change and agreement
description: "The measures that compare two maps of classes: the components of difference, kappa and its variants, the agreement of a simulation (hits, misses, figure of merit, three maps), intensity analysis, fuzzy similarity and the accuracy of a map from a sample."
order: 90
---

::: metrics change-and-agreement

## The example maps {#example}

The values on the cards were computed on the land use of Sibuyan Island, the Philippines, in 1997 and 2011 that ships with
the library, and on a simulated 2011 map made from them with a seeded generator, since the island has no model run to
judge: it undoes half of the observed changes and flips 1 percent of the cells that did not change, so that each measure has
something to say. It is not the result of a land change model. The accuracy cards use a stratified random sample of the 2011
map whose reference labels are read from the simulation, in place of an interpreter's:

```python
import numpy as np
import pandas as pd
import terraflux as tf

lu = tf.load_raster(tf.simulate.example_data("sibuyan_landuse"))      # Sibuyan Island, 1997 and 2011, 5 classes
t0 = np.asarray(lu.isel(band=0)).astype(float)
t1 = np.asarray(lu.isel(band=1)).astype(float)
t0[t0 == 255] = np.nan                                                # 255 is no value
t1[t1 == 255] = np.nan

# a simulated 2011: undo half of the changes and flip 1 percent of the cells that did not change
rng = np.random.default_rng(1)
sim = t1.copy()
changed = (t0 != t1) & ~np.isnan(t0) & ~np.isnan(t1)
undo = changed & (rng.random(t0.shape) < 0.5)
sim[undo] = t0[undo]
stay = ~changed & ~np.isnan(t0)
flip = stay & (rng.random(t0.shape) < 0.01)
sim[flip] = np.where(t0[flip] == 1, 2, 1)

zones = np.where(np.arange(t1.shape[1]) < t1.shape[1] // 2, 1.0, 2.0) * np.ones(t1.shape)   # a west and an east stratum
nonforest = lambda a: np.where(np.isnan(a), np.nan, (a != 1).astype(float))                # the rest against forest (class 1)
o, r, s = nonforest(t0), nonforest(t1), nonforest(sim)

pts = tf.accuracy.sample(lu.isel(band=1), n=100, seed=1)               # a stratified sample of the 2011 map
pts["reference"] = sim[pts["row"], pts["col"]]                         # its reference labels, read from the simulation

print(int((~np.isnan(t1)).sum()), "cells with data;", int(changed.sum()), "changed;", int(undo.sum()), "changes undone and", int(flip.sum()), "cells flipped in the simulation")
print(len(pts), "sample points; agreement of the simulation with the 2011 map:", round(float(np.nanmean((sim == t1)[~np.isnan(t1)])), 4))
```

```text
7226 cells with data; 743 changed; 394 changes undone and 56 cells flipped in the simulation
500 sample points; agreement of the simulation with the 2011 map: 0.9377
```

Two things follow from how the simulation is made. Its overall agreement (94%) is mostly the persistence of the cells that
did not change, which is why the measures of change (hits, misses, figure of merit, intensity) say more than kappa or
correctness; and the measures that need continuous maps (MAD) are run on the class codes only to show the call.
