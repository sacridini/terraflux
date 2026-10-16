---
title: Original vegetation and area of habitat
short: Vegetation and habitat
description: Find what converted land was before it was cleared, measure every species' area of habitat today and before conversion, and see why the species that have lost the most will drive a restoration plan.
order: 40
level: intermediate
time: 45 min
ops: [restoration.original_vegetation, sdm.area_of_habitat, stack.of, map.calc, raster.read]
series: Distance and restoration
part: 4
---

A restoration plan needs to know two things about every converted cell: what would grow back if it were
restored, and which species would gain from it. This part answers both for Serra Verde, the imaginary region
introduced in the [previous part](../ecological-intactness/). You find the *original vegetation* of its pasture
and cropland from a map of the past, then measure the *area of habitat* of eight species today and before
conversion, and what each converted cell would give each species if it were restored. At the end you have the
inputs of the restoration plans of the next two parts, and a first look at which species those plans will favour.

## The idea

### Fractions of land cover

Maps of land cover usually give each cell one class. At 1 km, though, a cell is rarely all forest or all pasture.
Plangea, the software behind the global restoration plans of Strassburg et al. (2019, 2020), works instead with
the **share** of every class in every cell: a cell may be 0.6 forest, 0.3 pasture and 0.1 cropland. Classes are
either **natural** (forest, savanna, mangrove: what restoration brings back) or **anthropic** (pasture, cropland:
what restoration replaces); the others (urban, water) are neither and are left alone.

### What was there before

Restoring a cell's pasture turns it back into the natural vegetation that was there, and that is not always the
vegetation around it today. Plangea decides the **original vegetation** of each cell's anthropic share in three
steps:

1. **The past.** If a map of the natural cover at an earlier date says what the cell held, its anthropic share is
   given back to the natural classes in the proportions they had then.
2. **The ecoregion.** If the past map says nothing (no natural cover left), or the cell is almost entirely
   converted (more than `threshold`, 95%, of its land anthropic), the proportions of the natural classes of its
   ecoregion are used instead.
3. **A failsafe.** If neither tells, the mean proportions over the whole map.

### The area of habitat

A species' range map (from IUCN or a distribution model) draws the outline of where it occurs, but much of the
land inside it is not habitat: a forest bird's range includes the pastures and towns within it. The **area of
habitat** (AOH) of a species is the part of its range covered by the land cover it lives in, within its known
elevations (Brooks et al., 2019). With shares of land cover, each cell of the range counts for its area times the
share of the classes the species lives in.

!!! science "Key idea"
    For species \(s\) with range \(R_s\), suitable classes \(S_s\), and \(p_{c,i}\) the share of class \(c\) in
    cell \(i\) of area \(A_i\):

    \[ \mathrm{AOH}_s = \sum_{i \in R_s} A_i \sum_{c \in S_s} p_{c,i} \]

    With today's shares it is the **current** habitat; with the original vegetation (today's natural land plus the
    anthropic share given back to the natural classes) it is the **potential** habitat, before conversion. The
    **gain** of cell \(i\) is the habitat the species would get from restoring one unit (the whole) of the cell:
    its area times the share of the restored land that becomes a suitable class.

### From habitat lost to extinction risk

Why measure habitat before and after? Because the species-area relationship links the habitat a species has left
to its risk of extinction. If a species keeps a fraction \(r\) of its original habitat, its risk is taken as
\(1 - r^z\) with \(z \approx 0.25\), the exponent of the species-area curve (Thomas et al., 2004; Strassburg et
al., 2019). The curve is steep near the left: a species that keeps half its habitat is far more at risk than one
that keeps 90%, and each hectare restored for it avoids far more risk.

## When to use it

- **Restoration planning**: the original vegetation and the habitat gains are the inputs of
  [[restoration.plan]] in the [next part](../restoration-plan/).
- **Red List assessments**: AOH is a recommended measure for the IUCN Red List, a tighter estimate of the area a
  species occupies than its range (Brooks et al., 2019).
- **Habitat loss by species**: the potential and the current habitat give each species' loss, the basis of
  indicators of the impact of land conversion.

What it is *not* for: AOH counts habitat, not its quality, its fragmentation or whether it is occupied. For the
habitat suitability of one species from its records, see [species distributions](/tutorials/#species-distributions);
for patches and connectivity, [habitat patterns](/tutorials/#habitat-patterns) and
[habitat networks](/tutorials/#habitat-networks).

## What you need

All maps on one grid, in a projected CRS (or with `cell_area` given in km²). For Serra Verde (60 × 90 cells of
1 km, EPSG:31983):

- `land_cover.tif`: the share (0 to 1) of six classes in every cell, as six bands named `forest`, `savanna`,
  `mangrove`, `pasture`, `cropland` and `urban`. Band names matter: the operators find the classes by them.
- `past_cover.tif`: the share of the three natural classes in the past, before most of the conversion (bands
  `forest`, `savanna`, `mangrove`).
- `ecoregions.tif`: four ecoregions, 1 coast, 2 lowland forest, 3 montane forest, 4 savanna.
- `features.tif`: eight bands, one per species or habitat type (`jaguar`, `lowland_tapir`, `harpy_eagle`,
  `maned_wolf`, `golden_lion_tamarin`, `neotropical_otter`, `araucaria_forest`, `mangrove`), each a suitability
  from 0 to 1. A cell above 0 is in the range.
- A table of the classes each species lives in (the `suitability` setting below).

![Serra Verde's land cover: each cell coloured by the mix of its classes](/img/tutorials/distance-restoration/original-vegetation-and-habitat/land_cover.png "The land cover of Serra Verde, each cell's colour the mix of its classes' colours weighted by their shares. Forest covers 3,391 km², savanna 381, mangrove 52, pasture 461, cropland 455 and the three cities 84. Pasture and cropland, 916 km², follow the highway, the rivers and the cities.")

## Build the model

### The ecoregions' natural shares

[[restoration.original_vegetation]] takes the ecoregion's natural proportions as a *stack* (a set of maps on
one grid, each with a name): for every cell, the shares of forest, savanna and mangrove of its ecoregion. Here
they are the mean proportions of the natural classes in each ecoregion's past cover, computed once outside the
model:

| Ecoregion | Forest | Savanna | Mangrove |
|---|---|---|---|
| 1 Coast | 0.31 | 0 | 0.69 |
| 2 Lowland forest | 0.98 | 0.01 | 0.01 |
| 3 Montane forest | 0.91 | 0.09 | 0 |
| 4 Savanna | 0.20 | 0.80 | 0 |

Three [[map.calc]] nodes write them on the map from the ecoregion codes, for example for forest:

```text
if e == 1 then 0.31 else if e == 2 then 0.98 else if e == 3 then 0.91 else 0.2
```

and [[stack.of]] joins them into one stack, naming each layer as its input (`forest`, `savanna`, `mangrove`).

### The original vegetation

Add [[restoration.original_vegetation]]. Connect `cover.raster` (the land cover read by [[raster.read]], every
band as a layer) to `land_cover`, `past.raster` to `past` and `eco_shares.stack` to `ecoregions`. Set
`natural = ["forest", "savanna", "mangrove"]` and `anthropic = ["pasture", "cropland"]`. Its outputs:

- `original`: the original share of each natural class (today's natural land plus the anthropic share given back).
- `restore`: the proportions restored land returns to in each cell (`restore_to = "current"` makes it the cell's
  current natural cover where it has some, instead of the original).
- `source`: where each cell's proportions came from, 0 the past, 1 the ecoregion, 2 the failsafe.
- `anthropic`: the share of anthropic land, the land a plan can restore.
- `failsafe`: the failsafe proportions of each natural class.

### The area of habitat

Add [[sdm.area_of_habitat]]. Its inputs: `ranges` (a stack, one layer per species: here `ranges.raster`, the
features read whole), `land_cover`, and the vegetation's `original`, `restore` and `anthropic`. Its settings:

- `natural`: the natural classes, as above.
- `suitability`: the classes each species lives in, `{species: [classes]}`; a species not listed lives in every
  natural class (unless `include_unlisted = false`).
- `elevation_limits` with an `elevation` map: `[[species, lowest, highest], ...]` removes from each range the
  cells outside the species' elevations, as Brooks et al. (2019) recommend.
- `cell_area`: a cell's area in km², read from a projected grid by default.

Its outputs are `table` (per species, in the order of the ranges' layers: `current` and `potential` habitat in
km², `range_cells`, and `valid`, 1 when the species counts in a plan) and `gain` (a stack: the habitat each species
gains per unit of each cell restored).

```tfgraph
#! caption: Original vegetation and area of habitat: the ecoregions' natural shares as a stack, plangea's original vegetation of the converted land, and every species' habitat today, before conversion, and what restoring each cell gives it.
#! highlight: eco_shares, vegetation, habitat
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "land_cover.tif" }
note = "Six bands: the share of each class."

[[nodes]]
id = "past"
op = "raster.read"
params = { path = "past_cover.tif" }

[[nodes]]
id = "ecoregions"
op = "raster.read"
params = { path = "ecoregions.tif" }

[[nodes]]
id = "eco_forest"
op = "map.calc"
inputs = { e = "ecoregions.raster" }
params = { expression = "if e == 1 then 0.31 else if e == 2 then 0.98 else if e == 3 then 0.91 else 0.2" }

[[nodes]]
id = "eco_savanna"
op = "map.calc"
inputs = { e = "ecoregions.raster" }
params = { expression = "if e == 1 then 0 else if e == 2 then 0.01 else if e == 3 then 0.09 else 0.8" }

[[nodes]]
id = "eco_mangrove"
op = "map.calc"
inputs = { e = "ecoregions.raster" }
params = { expression = "if e == 1 then 0.69 else if e == 2 then 0.01 else 0" }

[[nodes]]
id = "eco_shares"
op = "stack.of"
inputs = { forest = "eco_forest.map", savanna = "eco_savanna.map", mangrove = "eco_mangrove.map" }

[[nodes]]
id = "vegetation"
op = "restoration.original_vegetation"
inputs = { land_cover = "cover.raster", past = "past.raster", ecoregions = "eco_shares.stack" }
params = { natural = ["forest", "savanna", "mangrove"], anthropic = ["pasture", "cropland"] }

[[nodes]]
id = "ranges"
op = "raster.read"
params = { path = "features.tif" }
note = "Eight species: a cell above 0 is in the range."

[[nodes]]
id = "habitat"
op = "sdm.area_of_habitat"
inputs = { ranges = "ranges.raster", land_cover = "cover.raster", original = "vegetation.original", restore = "vegetation.restore", anthropic = "vegetation.anthropic" }
params = { natural = ["forest", "savanna", "mangrove"], suitability = { jaguar = ["forest", "savanna"], lowland_tapir = ["forest"], harpy_eagle = ["forest"], maned_wolf = ["savanna"], golden_lion_tamarin = ["forest"], neotropical_otter = ["forest", "mangrove"], araucaria_forest = ["forest"], mangrove = ["mangrove"] } }
```

## Read the results

### Where the original vegetation came from

```tfgallery
columns: 2
caption: Left, the share of every cell in pasture and cropland. Right, where its original vegetation came from. The past map explains 95% of the cells with converted land; the ecoregion's shares fill the 117 cells almost entirely converted, the rings around the three cities.
images:
  - { src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/anthropic.png, caption: "Converted land" }
  - { src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/source.png, caption: "Source of the original vegetation" }
```

```tfgallery
columns: 3
caption: The original share of each natural class on the converted land. Most converted land was forest; savanna in the west, mangrove along the coast.
images:
  - { src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/original_forest.png, caption: "Forest" }
  - { src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/original_savanna.png, caption: "Savanna" }
  - { src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/original_mangrove.png, caption: "Mangrove" }
```

Why give the ecoregions at all? Without them, plangea's rule gives the nearly converted cells **equal shares** of
every natural class: a third forest, a third savanna, a third mangrove, even inland. Drag the handle to compare:

```tfcompare
caption: The original share of mangrove on the converted land with the ecoregions' shares (left) and without them (right). Without them the cells around the inland cities get a third of mangrove each. (Dark blue: 0.7 or more.)
before: { src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/mangrove_with.png, label: With the ecoregions }
after: { src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/mangrove_without.png, label: Without }
```

Here it changes little for the species (the inland mangrove is outside the mangrove's range), but the golden lion
tamarin's potential habitat drops from 101.8 to 97.9 km² and the lowland tapir's from 781 to 761 km² without the
ecoregions, because a third of those cells' forest became savanna and mangrove. On real data, where whole
districts are converted, the difference can be large. In the `source` map, code 1 marks these cells whether or
not ecoregions were given.

### Habitat today and before conversion

```tfchart
type: bar
data: /data/tutorials/distance-restoration/original-vegetation-and-habitat/aoh.csv
x: species
y: [potential, current]
labels: { potential: "Potential (before conversion)", current: "Current" }
colors: { potential: "#c9d8c4", current: "#3b8a5a" }
ylabel: Area of habitat (km²)
height: 320
caption: Every species' area of habitat today and before conversion, from sdm.area_of_habitat's table. The jaguar has the most habitat, the mangrove and the golden lion tamarin the least.
```

| Species | Current (km²) | Potential (km²) | Left | Risk \(1 - r^{0.25}\) |
|---|---|---|---|---|
| Jaguar | 2,711.6 | 2,862.8 | 94.7% | 0.014 |
| Lowland tapir | 649.5 | 781.3 | 83.1% | 0.045 |
| Harpy eagle | 1,206.4 | 1,206.6 | 100.0% | 0.000 |
| Maned wolf | 365.6 | 382.0 | 95.7% | 0.011 |
| Golden lion tamarin | 56.8 | 101.8 | 55.8% | 0.136 |
| Neotropical otter | 311.5 | 374.2 | 83.2% | 0.045 |
| Araucaria forest | 976.5 | 1,019.7 | 95.8% | 0.011 |
| Mangrove | 44.3 | 68.7 | 64.5% | 0.104 |

The absolute areas say little about need. What matters is the share left: the golden lion tamarin, in two small
lowland fragments, keeps 56% of its habitat, and the mangrove 65%; the harpy eagle, in remote mountain forest,
has lost almost nothing.

![Extinction risk against the habitat left, with the eight species on the curve](/img/tutorials/distance-restoration/original-vegetation-and-habitat/sar.png "The species-area relationship: the risk 1 - r^z of a species that keeps a share r of its habitat, for z = 0.15, 0.25 and 0.35, and the eight species of Serra Verde (orange) at z = 0.25. The tamarin and the mangrove sit on the steep part of the curve.")

On the curve, the tamarin and the mangrove carry nearly two thirds of the total risk (0.136 + 0.104 of 0.365).
Restoring their habitat moves them down the steep part of the curve; restoring the jaguar's moves it along the
flat part. A plan that minimises extinction risk will go to them first, as the next part shows.

### What restoring each cell gives each species

```tfslider
label: Species
caption: The habitat each species gains per unit of a cell restored (km², cells of 1 km²), inside its range and on converted land. 1 means the whole cell becomes its habitat; 0, a cell in its range whose restored land would become a class it does not live in.
frames:
  - { value: Jaguar, src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/gain_jaguar.png, stats: { "Current": "2,712 km²", "Potential": "2,863 km²", "Left": "94.7%" } }
  - { value: Lowland tapir, src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/gain_lowland_tapir.png, stats: { "Current": "650 km²", "Potential": "781 km²", "Left": "83.1%" } }
  - { value: Harpy eagle, src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/gain_harpy_eagle.png, stats: { "Current": "1,206 km²", "Potential": "1,207 km²", "Left": "100%" } }
  - { value: Maned wolf, src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/gain_maned_wolf.png, stats: { "Current": "366 km²", "Potential": "382 km²", "Left": "95.7%" } }
  - { value: Golden lion tamarin, src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/gain_golden_lion_tamarin.png, stats: { "Current": "57 km²", "Potential": "102 km²", "Left": "55.8%" } }
  - { value: Neotropical otter, src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/gain_neotropical_otter.png, stats: { "Current": "312 km²", "Potential": "374 km²", "Left": "83.2%" } }
  - { value: Araucaria forest, src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/gain_araucaria_forest.png, stats: { "Current": "977 km²", "Potential": "1,020 km²", "Left": "95.8%" } }
  - { value: Mangrove, src: /img/tutorials/distance-restoration/original-vegetation-and-habitat/gain_mangrove.png, stats: { "Current": "44 km²", "Potential": "69 km²", "Left": "64.5%" } }
start: 4
play: false
```

The gain maps are where the plan will act: the tamarin's two fragments, the coastal strip of the mangrove, the
river valleys of the otter and the tapir.

### How to report it

Report the land cover data and classes (which are natural, which anthropic), the past map and how the
ecoregions' proportions were made, the share of converted land each source explained, and per species the range
source, the suitable classes, the elevation limits, and the current and potential habitat with the share left.

## Settings that change the answer

- **Suitability.** Which classes a species lives in decides its habitat. If the jaguar lived only in forest,
  its habitat would shrink to the forest of its range. Take the classes from the IUCN habitat classification of
  each species, and check them with experts.
- **Elevation limits.** With the relative elevation of `elevation.tif` connected to `elevation` and
  `elevation_limits = [["araucaria_forest", 0.8, 1.0]]`, the araucaria forest keeps only the cells of its range
  at 0.8 or above: its range falls from 1,050 to 635 cells, its current habitat from 976 to 620 km² and its
  potential from 1,020 to 635 km². The share left rises from 95.8% to 97.7%, because the conversion happened at
  lower elevations. Limits remove habitat that the land cover alone would count, which is why Brooks et al. (2019)
  include them in the definition of AOH.
- **The threshold** of [[restoration.original_vegetation]] (0.95): below it the past map is trusted even for
  heavily converted cells; above it, the ecoregion's proportions are used for fewer of them.
- **The ranges.** A suitability map above 0 makes a generous range. A higher threshold (each band read by its
  name with [[raster.read]], a [[map.calc]] of `f > 0.5` on it, and the results joined with [[stack.of]]), or
  ranges from a distribution model, make a tighter one, and all the areas fall.

## Common pitfalls

- **Band names.** The operators find classes and species by the names of the layers. A GeoTIFF without band
  descriptions gives layers named by number, and the settings must use those names.
- **Shares, not classes.** The land cover is a stack of shares from 0 to 1 that sum to at most 1 in each cell. A
  map of classes must be turned into shares first (one [[map.calc]] per class, `lc == 3`, then [[stack.of]]).
- **The order of the species.** The table's rows follow the order of the ranges' layers, with no names; keep
  that order when you read it.
- **No original vegetation.** Give the `original` input: without it [[sdm.area_of_habitat]] cannot compute the
  gains a plan needs.
- **Units.** Areas are in km² from a projected grid (or `cell_area`); on a grid in degrees, each cell's true area
  on the ellipsoid is used.

## Going further

- The plan that restores this habitat where it avoids most extinction risk:
  [A restoration plan](../restoration-plan/), the next part.
- Where human pressure is highest: [Ecological intactness](../ecological-intactness/), the previous part.
- Habitat suitability from species records: [species distributions](/tutorials/#species-distributions).
- The operators: [[restoration.original_vegetation]], [[sdm.area_of_habitat]].

## References

- Brooks, T. M., Pimm, S. L., Akçakaya, H. R., Buchanan, G. M., Butchart, S. H. M., Foden, W., et al. (2019). Measuring terrestrial area of habitat (AOH) and its utility for the IUCN Red List. *Trends in Ecology & Evolution*, 34(11), 977–986. https://doi.org/10.1016/j.tree.2019.06.009
- Strassburg, B. B. N., Beyer, H. L., Crouzeilles, R., Iribarrem, A., Barros, F., de Siqueira, M. F., et al. (2019). Strategic approaches to restoring ecosystems can triple conservation gains and halve costs. *Nature Ecology & Evolution*, 3(1), 62–70. https://doi.org/10.1038/s41559-018-0743-8
- Strassburg, B. B. N., Iribarrem, A., Beyer, H. L., Cordeiro, C. L., Crouzeilles, R., Jakovac, C. C., et al. (2020). Global priority areas for ecosystem restoration. *Nature*, 586(7831), 724–729. https://doi.org/10.1038/s41586-020-2784-9
- Thomas, C. D., Cameron, A., Green, R. E., Bakkenes, M., Beaumont, L. J., Collingham, Y. C., et al. (2004). Extinction risk from climate change. *Nature*, 427(6970), 145–148. https://doi.org/10.1038/nature02121
