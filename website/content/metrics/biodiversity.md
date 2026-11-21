---
title: Biodiversity (Biodiverse)
description: "The indices of Biodiverse, computed per cell over a neighbourhood: richness, abundance, diversity, richness estimators, endemism, rarity, turnover, phylogenetic diversity, endemism and turnover, label properties, matrices and randomisation."
order: 40
---

::: metrics biodiversity

## The example records {#example}

The values on the cards were computed on Biodiverse's own example, the one the biodiversity tutorials use: 365 records of
31 species snapped to cells of 100 km, and the phylogeny of the species. The cards show one cell, `3350000:750000`, with a
neighbourhood of the cells within 200 km (`C`) and, for the indices that compare two sets of cells, the cell itself against
its neighbourhood (`C2`). `at` only picks what that cell holds in a result. The example has no label or group properties
and no label matrix, so the cards of those calculations use a small toy from a seeded random generator (a mass per species,
an elevation per cell, random symmetric distances); the numeric-label cards read the record number as the label:

```python
import numpy as np
import pandas as pd
import terraflux as tf

# Biodiverse's example records: 365 records of 31 species, snapped to cells of 100 km
sites = tf.bio.example_data("sites")
bd = tf.bio.BaseData.from_table(sites, group_columns=["x", "y"], label_columns=["genus", "species"],
                                cell_sizes=[100000, 100000])
tree = tf.bio.read_tree(tf.bio.example_data("tree"))  # the phylogeny of those species
C = "sp_circle (radius => 200000)"                    # the neighbourhood: the cells within 200 km
C2 = ["sp_self_only()", C]                            # two sets: the cell itself (set 1), its neighbourhood (set 2)
G = "3350000:750000"                                  # the cell whose values the cards show

# a small seeded toy where the example has no data: a mass per species, an elevation per cell, a label matrix
rng = np.random.default_rng(1)
bd.set_label_properties(pd.DataFrame({"MASS": rng.integers(1, 51, len(bd.labels)).astype(float)}, index=bd.labels))
bd.set_group_properties(pd.DataFrame({"ELEV": rng.integers(0, 801, len(bd.groups)).astype(float)}, index=bd.groups))
v = rng.random((len(bd.labels), len(bd.labels)))
mx = tf.bio.Matrix(bd.labels, (v + v.T) / 2, self_similarity=0.0)
# numeric labels: the record number of the table is the label
bn = tf.bio.BaseData.from_table(sites, group_columns=["x", "y"], label_columns=["num"], cell_sizes=[100000, 100000])


def at(res, *keys, like=None, n=6):
    """What a result holds for the cell G: columns of its table, or the first n entries of a list index."""
    if keys and keys[0] in res.lists:
        d = res.lists[keys[0]]
        return d[d["group"] == G].set_index("key")["value"].head(n)
    row = res.table.loc[G]
    if like:
        return row.filter(like=like)
    return row[keys[0]] if len(keys) == 1 else row[list(keys)] if keys else row

print(bd)
print(at(tf.bio.spatial_analysis(bd, C, ["richness", "endemism_whole"]), "RICHNESS_ALL", "ENDW_WE", "ENDW_CWE"))
print(at(tf.bio.spatial_analysis(bd, C2, ["sorenson", "phylo_sorenson"], tree=tree), "SORENSON", "PHYLO_SORENSON"))
print(at(tf.bio.spatial_analysis(bd, C, "pe", tree=tree), "PE_WE", "PE_WE_P"))
```

```text
BaseData(127 groups, 31 labels, cell_sizes=[100000.0, 100000.0], 365 records)
RICHNESS_ALL    16.000000
ENDW_WE          4.330668
ENDW_CWE         0.270667
Name: 3350000:750000, dtype: float64
SORENSON          0.280000
PHYLO_SORENSON    0.254054
Name: 3350000:750000, dtype: float64
PE_WE      2.897136
PE_WE_P    0.136772
Name: 3350000:750000, dtype: float64
```

Every call of a card is one `tf.bio.spatial_analysis` (or, for the last three, `tf.bio.randomise` on its result), so it
computes the indices of all the cells of the basedata, not only of the one shown. The names of the calculations
(`"richness"`, `"pd"`...) are the settings of the operator `bio.spatial`; [`tf.bio.list_calculations()`][terraflux.bio.list_calculations]
lists them with the indices each gives.
