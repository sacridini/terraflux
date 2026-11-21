---
title: Species distributions
description: "The measures that judge a species distribution model (AUC, TSS, kappa, Boyce, the thresholds and omission rates) and the ones that say how far a projection extrapolates (MESS, MOP, ExDet)."
order: 80
---

::: metrics species-distributions

## The example model {#example}

The values on the cards were computed on the model of the species distribution tutorials: the Bradypus records and the
bioclimatic layers that ship with the library, a MaxNet model fitted with spatial folds. The evaluation cards read its
**out-of-fold predictions** at the 83 presences and the 2000 background points (what each record gets from the model fitted
without its block), the extrapolation cards a scenario with a warmest month 2 degrees hotter and a driest quarter 20%
drier. Nothing here is a statement about the sloth: the model is a small one, and the numbers only show what each measure
does on one fitted model:

```python
import numpy as np
import terraflux as tf

occ = tf.load_table(tf.sdm.example_data("bradypus"))
env = tf.load_raster(tf.sdm.example_data("bioclim"))
data = tf.sdm.prepare(occ, env, thin=50_000, area=500_000, background=2000,
                      folds={"size": 500_000, "k": 5}, seed=1)
res = tf.sdm.fit(data, {"maxnet": tf.sdm.MaxNet(regmult=2, classes="lqh")})

p = res.models["maxnet"].predict_proba(data.X)[:, 1]          # the model fitted on every record, at every record
pres = p[data.y == 1]                                         # training predictions at the presences
oof = res.predictions["maxnet"].to_numpy()                    # out-of-fold: the model fitted without the record's block
ok = ~np.isnan(oof)
o_pres, o_back = oof[ok & (data.y == 1)], oof[ok & (data.y == 0)]
ev = tf.sdm.evaluate(o_pres, o_back)
at = ev.table.loc[ev.table["tss"].idxmax()]                   # the row of the confusion table at the maximum TSS

future = env.copy()                                           # a scenario: a hotter warmest month, a drier driest quarter
future.loc[{"band": "bio5"}] = env.sel(band="bio5") + 20
future.loc[{"band": "bio17"}] = env.sel(band="bio17") * 0.8
new = future.sel(band=data.predictors)

print({k: round(float(ev.stats[k]), 2) for k in ("auc", "max_tss", "boyce")}, int(ev.stats["np"]), "presences")
print(round(float(at["threshold"]), 2), "at the maximum TSS:", int(at["tp"]), int(at["fp"]), int(at["fn"]), int(at["tn"]))
mess = tf.sdm.mess(data, new)
print(int((mess < 0).sum()), "cells with a MESS below 0 in the scenario")
```

```text
{'auc': 0.7, 'max_tss': 0.29, 'boyce': 0.93} 83 presences
0.65 at the maximum TSS: 44 488 39 1512
2912 cells with a MESS below 0 in the scenario
```

With background points in place of absences, the false positives (`fp`) are background points the model calls suitable, and
the specificity and the prevalence are properties of the background you drew: read the discrimination measures (AUC, TSS,
Boyce) and the omission rates, and treat the others with the cautions on their cards. The map measures need the reference
conditions (`data`) and the new ones (`new`) to have the same predictors.
