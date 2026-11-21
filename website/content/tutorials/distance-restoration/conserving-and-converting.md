---
title: Conserving and converting
short: Conserving and converting
description: Plan which natural land to keep and where to meet a demand for land with the same model as a restoration plan - the habitat lost per unit of natural land, a threat to conserve against, a demand converted where it costs the least benefit, restoring what is converted, and projected changes per region.
order: 70
level: advanced
time: 35 min
series: Distance and restoration
part: 7
---

The plans of the previous parts restore converted land. The model behind them, the one of Strassburg et al. (2020),
answers two questions more with the same benefits and targets. **Which natural land should be kept?** Natural land is
lost where it is threatened, and conserving it saves the habitat it holds. **Where should a demand for land be met?**
Some land has to be converted (to farms, to towns), and the least harm is done where the species lose the least; and
what is converted can be restored elsewhere. In this last part you answer both for Serra Verde, with the
library, and put them together with projected demands per region.

!!! info "Background"
    This part continues [A restoration plan](../restoration-plan/): the problem, the benefits, the targets and `solve()`
    are the same, and so are the data of [part 4](../original-vegetation-and-habitat/). The operator
    [[restoration.plan]] takes the same maps as inputs (`conservable`, `convertible`, `threat`, `loss`); the rest of this
    part is in Python.

## What you need

The data of the earlier parts (Serra Verde, which the library ships), and one more result: the habitat each species
**loses** per unit of natural land converted, which [`tf.sdm.area_of_habitat`][terraflux.sdm.area_of_habitat] gives
with `loss=True`, besides the habitat a restored cell gains:

```python
import numpy as np
import terraflux as tf

P = tf.prioritize
cover = tf.load_raster(P.example_data("land_cover"))
past = tf.load_raster(P.example_data("past_cover"))
ecoregions = tf.load_raster(P.example_data("ecoregions"))
features = tf.load_raster(P.example_data("features"))
footprint = tf.load_raster(P.example_data("footprint"))
regions = tf.load_raster(P.example_data("regions"))
natural, anthropic = ["forest", "savanna", "mangrove"], ["pasture", "cropland"]

props = past / past.sum("band")                              # as in part 4
eco_props = props.copy()
for code in np.unique(ecoregions.values[np.isfinite(ecoregions.values)]):
    inside = ecoregions.values == code
    eco_props.values[:, inside] = np.nanmean(props.values[:, inside], axis=1)[:, None]
veg = tf.original_vegetation(cover, natural, anthropic, past=past, ecoregions=eco_props)

suitability = {
    "jaguar": ["forest", "savanna"], "lowland_tapir": ["forest"], "harpy_eagle": ["forest"],
    "maned_wolf": ["savanna"], "golden_lion_tamarin": ["forest"],
    "neotropical_otter": ["forest", "mangrove"], "araucaria_forest": ["forest"], "mangrove": ["mangrove"],
}
aoh = tf.sdm.area_of_habitat((features > 0).astype("float32"), cover, natural, suitability=suitability,
                             original=veg, loss=True)
nat = cover.sel(band=natural).sum("band")                    # the natural share of every cell
```

## Which natural land to keep

With `conservable=` the units of the problem are the natural land, and the baseline loses what is threatened:
`threat` is the share lost without protection (1 everywhere by default). Here the threat is larger where the human
footprint is high, so the land next to the highway and the cities is the most at risk. Conserving the best 100 km²
avoids the most extinction risk:

```python
keep = (P.restoration_problem(conservable=nat, threat=(footprint / 20).clip(0, 1))
          .add_extinction_risk_benefit(aoh, name="species")
          .add_area_target(100))
kept = keep.solve()
print(kept.status, f"benefit {kept.benefit:.4f}")
print(P.eval_risk_reduction(keep, kept).table[["species", "current", "baseline", "risk_after"]].round(3).to_string(index=False))
```

```text
optimal benefit 0.0457
            species  current  baseline  risk_after
             jaguar 2711.561  2677.322       0.016
      lowland_tapir  649.549   628.945       0.052
        harpy_eagle 1206.442  1206.415       0.000
         maned_wolf  365.584   363.618       0.012
golden_lion_tamarin   56.767    50.355       0.137
  neotropical_otter  311.500   300.578       0.050
   araucaria_forest  976.492   967.443       0.013
           mangrove   44.286    40.598       0.106
```

The `baseline` is each species' habitat if the threatened land were lost; conserving the best 100 km² avoids a risk of
0.046, mostly for the golden lion tamarin and the mangrove, whose remnants are the most threatened. The solution has
`kept.conserved`, the share of each cell kept (as `restored` has for a restoration plan).

## Where to meet a demand for land

With `convertible=` the plan converts natural land where it costs the least benefit, and with `restorable` too (the
original vegetation, as before) it can restore elsewhere what it converts. A demand of 50 km² of farmland, and no net
loss: restore what is converted:

```python
swap = (P.restoration_problem(veg, convertible=nat)
          .add_extinction_risk_benefit(aoh, name="species")
          .add_area_target(50, sense="=", action="convert")     # a demand of 50 km2 of farmland
          .add_area_target(0, sense="="))                       # no net loss: restore what is converted
both = swap.solve()
print(both.status, {k: round(v, 1) for k, v in both.areas.items()}, f"benefit {both.benefit:.4f}")
```

```text
optimal {'restore': 50.0, 'convert': 50.0} benefit 0.1746
```

The 50 km² are converted outside every species' range, so the plan loses nothing and restores 50 km² where they count
most: the same benefit as restoring 50 km² alone (0.175, as in the [previous part](../restoration-plan/)). The solution
has a `converted` map beside `restored`, and `both.areas` the area of each action. A target without `action=` is the net
area (restored minus converted), which is how the second target above asks for no net loss.

## Projected demand per region

Projections of land demand per region (from an economic model, say) go in with `add_projected_change`: a positive
change is land spared for restoration, a negative one a demand for land to convert. Each region does what it projects:

```python
proj = (P.restoration_problem(veg, convertible=nat)
          .add_extinction_risk_benefit(aoh, name="species")
          .add_projected_change({1: -30, 2: 20, 3: 40}, regions))   # km2: region 1 needs 30
s = proj.solve()
for r in (1, 2, 3):
    inside = regions.values == r
    print(f"region {r}: restores {np.nansum(np.where(inside, s.restored.values, 0)):.0f} km2, "
          f"converts {np.nansum(np.where(inside, s.converted.values, 0)):.0f} km2")
```

```text
region 1: restores 0 km2, converts 30 km2
region 2: restores 20 km2, converts 0 km2
region 3: restores 40 km2, converts 0 km2
```

A plan that only restores cannot convert: there the land demanded elsewhere is taken from the restoration
(`compensate=True`, the default), and every region with land spared restores its share of the net spared land:

```python
only = (P.restoration_problem(veg)
          .add_extinction_risk_benefit(aoh, name="species")
          .add_projected_change({1: -30, 2: 20, 3: 40}, regions))   # compensate=True
s = only.solve()
for r in (1, 2, 3):
    inside = regions.values == r
    print(f"region {r}: restores {np.nansum(np.where(inside, s.restored.values, 0)):.0f} km2")
```

```text
region 1: restores 0 km2
region 2: restores 10 km2
region 3: restores 20 km2
```

Regions 2 and 3 restore 10 and 20 km² of the 30 km² they spare, the net of their 60 and region 1's demand of 30.
`compensate=False` makes each region restore what it spares (20 and 40 km²), and `restrict=False` lets a plan that
restores and converts act in every region in both directions. `sense` of the rows is `"="` (the change as projected).

## Common pitfalls

- **The habitat lost, not gained.** A plan that converts or conserves needs `loss=True` in
  [`tf.sdm.area_of_habitat`][terraflux.sdm.area_of_habitat]: the gain of a restoration plan says nothing of what
  conversion takes away.
- **Conservation and conversion together.** `conservable` and `convertible` do not mix: the same natural land is
  either kept or converted. A plan that conserves and one that converts are two problems.
- **The threat is an assumption.** Without a map of threats, conservation saves all the natural land it keeps
  (`threat=1`), which makes every plan look better than it is. Say where the threat comes from.

## Going further

- Share a target among ecosystems or regions: [Several benefits and trade-offs](../several-benefits/).
- The problem's other settings and the benefit of a plan: [A restoration plan](../restoration-plan/).
- The methods of the problem: [`tf.prioritize.restoration_problem`][terraflux.prioritize.restoration_problem].

## References

- Strassburg, B. B. N., Iribarrem, A., Beyer, H. L., Cordeiro, C. L., Crouzeilles, R., Jakovac, C. C., et al. (2020). Global priority areas for ecosystem restoration. *Nature*, 586(7831), 724–729. https://doi.org/10.1038/s41586-020-2784-9
