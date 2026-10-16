---
title: Sampling, time series and redundancy
short: Sampling and change
description: "Landscape metrics around sampling sites at several radii (metrics.sample), along a series of dates (metrics.series), and over a grid of zones with the correlations that show which metrics say the same thing (metrics.correlation)."
order: 50
level: intermediate
time: 50 min
ops: [metrics.sample, metrics.series, metrics.correlation, vector.read, vector.grid_zones, raster.reclassify]
series: Landscape metrics
part: 5
---

Most field studies do not need the metrics of a whole map. They need the landscape *around* each site where birds
were counted or traps were set, at the distance that matters to the species; or the metrics of the same landscape at
several dates; or the metrics of many zones, to compare them and to see which metrics carry independent information.
This part covers the three designs with three operators: [[metrics.sample]] for plots around sites (as
landscapemetrics' `sample_lsm`), [[metrics.series]] for a series of maps (as pylandstats' spatio-temporal analysis),
and [[metrics.correlation]] for the redundancy among metrics. At the end you will be able to build a table of
landscape predictors for a site-based study, follow a landscape through time, and choose a parsimonious set of
metrics.

## The idea

### The landscape around a site

A *landscape-scale* study relates a response measured at sites (species richness, abundance, occupancy, crop
pollination) to the landscape around each site, typically within circles of some radius. The radius is the
question's scale, and it is rarely known in advance: a bee may respond to the flowers within 250 m, a bird of prey to
the forest within 2 km. The **scale of effect** is the radius at which the landscape best explains the response
(Jackson & Fahrig, 2015). The usual way to find it is to measure the landscape at several radii and compare models.

Two rules of design matter as much as the metrics (Eigenbrod et al., 2011; Pasher et al., 2013): landscapes around
sites should **not overlap** (overlapping circles share cells, so they are not independent replicates), and they
should **span a wide range** of the variables of interest while keeping them as uncorrelated as possible, for
example habitat amount and fragmentation varying independently.

### A landscape through time

Metrics of the same area at several dates show *how* it changed: did the forest shrink at its edges (fewer cells,
the same patches) or break apart (more, smaller patches)? The maps must be comparable (the same grain, extent and
legend) or the change of the metrics reflects the maps, not the landscape.

### Many metrics, few dimensions

There are 133 metrics in landscapemetrics, but they are not 133 independent pieces of information. Some are
identical by definition (patch density is the number of patches over a constant area); others are tightly linked in
real landscapes. Factor analyses of dozens of metrics over many landscapes found about six or seven independent
dimensions: roughly amount, subdivision, edge, shape, aggregation, isolation and diversity (Riitters et al., 1995;
Cushman et al., 2008). A correlation matrix over many zones shows them for your own map.

## When to use it

- **[[metrics.sample]]**: sites with a response (counts, presence, traits); plots at one or several sizes, squares
  or circles around points, or polygons as they are (fields, farms, watersheds).
- **[[metrics.series]]**: a series of maps of the same area, of the whole maps or of zones of them.
- **[[metrics.correlation]]**: before choosing the predictors of a model, to drop metrics that duplicate others.
- **Not** to map the metric everywhere: that is the moving window of
  [part 4](/tutorials/landscape-metrics/moving-windows/). And for comparing two maps class by class (gains, losses,
  transitions), use [change detection](/tutorials/change-detection/).

## What you need

- A categorical map in a projected system with metres: [`clc.tif`](/data/tutorials/habitat-patterns/mspa/clc.tif),
  CORINE around Hannover, grouped into 7 classes as in part 4.
- Sites as vector points, in a file GDAL reads, in the map's coordinate system or another (Terraflux reprojects
  them): [`sites.geojson`](/data/tutorials/landscape-metrics/sampling-and-change/sites.geojson), six points made for
  this page.
- For the series, the maps of all dates as the bands of one file, or a stack:
  [`forest_dates.tif`](/data/tutorials/landscape-metrics/sampling-and-change/forest_dates.tif), six dates of a forest
  (1) and farmland (2), 200 × 200 cells of 30 m.

## Plots around sites

### Build the model

```tfgraph
#! caption: CORINE in 7 classes, six sites, and the metrics of the landscape within 250, 500, 1000 and 2000 m of each.
#! numbered: true
#! highlight: plots, land
format = "terraflux/1"

[[nodes]]
id = "corine"
op = "raster.read"
params = { path = "clc.tif" }

[[nodes]]
id = "cover"
op = "raster.reclassify"
inputs = { map = "corine.raster" }
params = { rules = "1..11 -> 1\n12 -> 2\n13..22 -> 3\n23..25 -> 4\n26..34 -> 5\n35..39 -> 6\n40..44 -> 7" }

[[nodes]]
id = "sites"
op = "vector.read"
params = { path = "sites.geojson" }

[[nodes]]
id = "plots"
op = "metrics.sample"
inputs = { map = "cover.map", plots = "sites.vector" }
params = { level = "class", metrics = ["pland", "np", "ed"], size = [250, 500, 1000, 2000], shape = "circle", id_column = "site" }
note = "A row per site, radius and class."

[[nodes]]
id = "land"
op = "metrics.sample"
inputs = { map = "cover.map", plots = "sites.vector" }
params = { level = "landscape", metrics = ["shdi", "pr", "ed"], size = [250, 500, 1000, 2000], shape = "circle", id_column = "site" }
note = "A row per site and radius."
```

[[metrics.sample]] has two inputs: the `map` (node 2's classes) and the `plots` (node 3's points). Drag both. Its
settings:

- `level`: `landscape` (default), `class` or `patch`: the level of the metrics of each plot.
- `size`: for points, the radius of a circle or the half side of a square, in map units (here metres); a list gives
  every size, a row each.
- `shape`: `square` (default) or `circle` (a polygon of 100 vertices, as landscapemetrics).
- `id_column`: the attribute that names each site in the table (default 1, 2...).

Each plot is cut from the map as R's terra does with `crop(mask = TRUE)`: the cells the plot touches are kept, the
others are no data, and the plot is measured as a landscape of its own. Its `plots` output gives the plots as
polygons, to draw them or to check them.

![Six sites and their circles](/img/tutorials/landscape-metrics/sampling-and-change/sites.png "The six sites on the land cover, with circles of 250, 500, 1000 and 2000 m. Site 6 sits in a large forest, site 4 in the city, sites 1 to 3 at the edges of forests in farmland, site 5 in the open plain.")

### Read the results

The `table` of node 4 has a row per site, radius and class. The forest's share around each site:

```tfchart
type: line
data: /data/tutorials/landscape-metrics/sampling-and-change/forest_by_radius.csv
x: radius_m
y: [site 1, site 2, site 3, site 4, site 5, site 6]
xlabel: Radius of the circle (m)
ylabel: "Forest, % of the circle"
caption: "The share of forest within 250 to 2000 m of each site. Sites 1 and 2 are in forest at 250 m (90 to 99%) but in mixed farmland at 2 km (35 to 49%); site 3 is the opposite, in open land with forest further out. The ranking of the sites changes with the radius."
```

```tfchart
type: line
data: /data/tutorials/landscape-metrics/sampling-and-change/shdi_by_radius.csv
x: radius_m
y: [site 1, site 2, site 3, site 4, site 5, site 6]
xlabel: Radius of the circle (m)
ylabel: "Shannon's diversity of the circle"
caption: "Shannon's diversity within each radius (node 5). It rises with the radius at every site, as larger circles take in more classes, but not equally: site 3 becomes the most diverse at 1 km, site 6 stays the least diverse."
```

The lesson of the two charts: **the radius is not a detail.** A model of a response against "forest cover around the
site" can give opposite answers at 250 m and at 2 km, because the sites change order. Measure at several radii and
compare the fit of the models (by AIC or the deviance explained); the radius that fits best is the scale of effect, and
report it with the others' fits, as Jackson & Fahrig (2015) recommend. Check that the largest circles do not overlap:
here even those of 2 km are kilometres apart.

The table also gives `percentage_inside`, the share of the plot's area that has cells of the map: 107% for the
circles of 250 m, 101% for those of 2 km. It is above 100 because every cell the circle touches is kept, so the cells
cover a little more than the circle; small plots are the most affected. Below 100 means the plot runs off the map
or onto no data: such plots are measured on less landscape than the others.

!!! paper "In the literature"
    Landscape metrics around sites are the predictors of thousands of studies. A typical design: the forest cover and
    edge density within radii of 250 m to 3 km around bird point counts, one model per radius, the best radius
    reported as the species' scale of effect. Jackson and Fahrig (2015) reviewed such studies and found that many
    did not test a wide enough range of radii, the best one often being the largest tested: measure up to radii
    larger than you expect.

## A landscape through time

### The six dates

The series was drawn with Terraflux's neutral landscape models: a forest (a fractal surface cut at 70%) cleared from
the west over six dates (2000 to 2025), each date clearing the forest cells whose "pressure" (a gradient from the west
with noise) is among the highest 12, 24, 36, 48 and 60%. The six maps were stacked into the six bands of
`forest_dates.tif`.

```tfgallery
columns: 3
caption: "The six dates (forest green, farmland beige): the clearing front moves east, eating the forest along a ragged line."
images:
  - { src: /img/tutorials/landscape-metrics/sampling-and-change/forest_2000.png, caption: "2000" }
  - { src: /img/tutorials/landscape-metrics/sampling-and-change/forest_2005.png, caption: "2005" }
  - { src: /img/tutorials/landscape-metrics/sampling-and-change/forest_2010.png, caption: "2010" }
  - { src: /img/tutorials/landscape-metrics/sampling-and-change/forest_2015.png, caption: "2015" }
  - { src: /img/tutorials/landscape-metrics/sampling-and-change/forest_2020.png, caption: "2020" }
  - { src: /img/tutorials/landscape-metrics/sampling-and-change/forest_2025.png, caption: "2025" }
```

### Build the model

```tfgraph
#! caption: The class metrics of six dates of a forest.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "forest_dates.tif" }
note = "Six bands, one per date."

[[nodes]]
id = "series"
op = "metrics.series"
inputs = { maps = "dates.raster" }
params = { level = "class", metrics = ["pland", "np", "ed", "lpi", "area_mn", "ai", "enn_mn"], dates = [2000, 2005, 2010, 2015, 2020, 2025] }
note = "A row per class and date."
```

[[metrics.series]] takes the `maps` as a raster of several bands (node 1's `raster`) or a stack. `dates` names the
bands in order; without it, the bands' names (here `y2000`...) are used. `level = "class"` gives a row per class and
date, every class at every date (a class absent at a date gets 0 for its area, edge and count metrics: `fillna`).
Give it `plots` (polygons, or points with `size`) or `grid` (squares of a side) to follow zones of the map instead
of the whole.

### Read the results

```tfchart
type: line
data: /data/tutorials/landscape-metrics/sampling-and-change/forest_series.csv
x: date
y: [pland, lpi, ed, ai]
labels: { pland: "Forest (PLAND, %)", lpi: "Largest patch (LPI, %)", ed: "Edge density (m/ha)", ai: "Aggregation (AI, %)" }
colors: { pland: "#3b8a5a", lpi: "#9fd18b", ed: "#e8901f", ai: "#2f6f9f" }
xlabel: Date
ylabel: Value
caption: "The forest's class metrics along the six dates. Its share halves, from 70% to 34%; the largest patch follows it closely; the edge density falls; the aggregation stays at 92-94% throughout."
```

| Date | Forest (%) | Patches | Largest patch (%) | Mean patch (ha) | Edge density (m/ha) | AI (%) | Mean ENN (m) |
|---|---|---|---|---|---|---|---|
| 2000 | 70.0 | 137 | 67.6 | 18.4 | 69.8 | 92.5 | 75 |
| 2005 | 61.8 | 210 | 55.5 | 10.6 | 70.3 | 91.6 | 77 |
| 2010 | 56.0 | 172 | 53.0 | 11.7 | 57.0 | 92.5 | 89 |
| 2015 | 50.5 | 109 | 49.0 | 16.7 | 43.4 | 93.8 | 83 |
| 2020 | 43.8 | 95 | 42.3 | 16.6 | 43.6 | 92.8 | 84 |
| 2025 | 34.2 | 129 | 32.6 | 9.5 | 36.8 | 92.2 | 82 |

This is habitat **loss without fragmentation**: the forest halves, but what remains is as aggregated as before, in one
large block (the largest patch is almost all the forest at every date). The number of patches and the mean patch
size go up and down as the ragged front cuts off small pieces and then clears them; read alone, they would suggest
fragmentation in 2005 and recovery in 2015. That is the danger of following one configuration metric through time:
report the amount (`pland`) and the largest patch beside it, and look at the maps.

## Which metrics say the same thing?

### Build the model

```tfgraph
#! caption: CORINE cut into 400 squares of 2.5 km, the landscape metrics of each, and their correlations.
#! numbered: true
#! highlight: zones, correlation
format = "terraflux/1"

[[nodes]]
id = "corine"
op = "raster.read"
params = { path = "clc.tif" }

[[nodes]]
id = "cover"
op = "raster.reclassify"
inputs = { map = "corine.raster" }
params = { rules = "1..11 -> 1\n12 -> 2\n13..22 -> 3\n23..25 -> 4\n26..34 -> 5\n35..39 -> 6\n40..44 -> 7" }

[[nodes]]
id = "zones"
op = "vector.grid_zones"
inputs = { map = "corine.raster" }
params = { size = 2500 }
note = "Squares of 2.5 km tiling the map."

[[nodes]]
id = "metrics"
op = "metrics.sample"
inputs = { map = "cover.map", plots = "zones.zones" }
params = { level = "landscape", metrics = ["np", "pd", "te", "ed", "lsi", "lpi", "area_mn", "ai", "contag", "division", "mesh", "shdi", "sidi", "pr"] }
note = "A row per zone."

[[nodes]]
id = "correlation"
op = "metrics.correlation"
inputs = { table = "metrics.table" }
params = { method = "spearman" }
```

[[vector.grid_zones]] reads only the grid of its input (node 1's `raster`) and makes square polygons of `size` map
units, row by row. [[metrics.sample]] measures each square as a plot (polygons are used as they are, no `size`).
[[metrics.correlation]] takes the table and correlates every pair of its metric columns (the key columns, such as
`plot_id`, are left out), with Pearson's, Spearman's (ranks, robust to the skewed distributions most metrics have)
or Kendall's method, as landscapemetrics' `calculate_correlation`.

### Read the results

![Correlations among 14 landscape metrics](/img/tutorials/landscape-metrics/sampling-and-change/correlation.png "Spearman's correlations among 14 landscape metrics over the 400 zones of 2.5 km. Dark red: strongly positive; dark blue: strongly negative. Contagion is NA: 10 zones hold only one class, where contagion is undefined, and R's correlation of a column with missing values is missing.")

Of the 78 pairs of the 13 metrics with values, 57 have a correlation stronger than 0.7 and 19 stronger than 0.9:

- **Identical by definition** over zones of equal area: `np` and `pd` (1.00), `area_mn` and `np` (−1.00, the mean
  area is the area over the number of patches); `te`, `ed` and `lsi` (1.00); `division` and `mesh` (−1.00).
- **Nearly identical** because they measure the same property: `ai` against `ed` (−0.99, aggregation is the mirror
  of edge), `lpi` against `division` (−0.99), `shdi` against `sidi` (0.98).
- **Less redundant**: `pr`, the number of classes, is the most independent (0.5 to 0.7 with the rest).

So these 14 metrics carry perhaps four pieces of information about these zones: subdivision (np), edge or
aggregation (ed), dominance of the largest patch (lpi or mesh) and diversity (shdi, pr). Choose one of each, guided
by the question.

!!! warning "Correlation with NA"
    A metric with any missing value gets NA correlations with every other metric, as in R. Here contagion is lost
    because of 10 single-class zones. Drop those zones from the table, or drop contagion, before correlating; and
    remember that single-class landscapes are real landscapes: dropping them changes the population you describe.

## Settings that change the answer

- **The radius** of the plots, above; and their **shape**: a square of half side \(r\) covers 27% more than a circle
  of radius \(r\).
- **The neighbour rule, edge depth and border settings**, as in every metrics operator (part 1).
- **The zone size** for the correlation: correlations among metrics depend on the extent of the units and the
  range of landscapes they cover. Recompute them for your own map and units.
- **The method of correlation**: Spearman's for metrics with skewed distributions (most of them).

## Common pitfalls

- **Overlapping plots** treated as independent replicates.
- **One radius chosen in advance** and never tested.
- **Plots at the map's border**: check `percentage_inside`.
- **Maps of different dates made differently**: a new sensor or classifier changes the metrics as much as the
  landscape does. Check with the [change detection](/tutorials/change-detection/) tutorials' methods.
- **Predictors that duplicate each other**: two metrics with a correlation of 0.95 in one regression model give
  unstable coefficients. Keep one.

## Going further

- [Part 6](/tutorials/landscape-metrics/landscape-signatures/): the metrics of many zones summarised by principal
  components and grouped into kinds of landscapes.
- [Change detection](/tutorials/change-detection/): transitions, gains and losses between maps.
- [Species distributions](/tutorials/species-distributions/): landscape predictors in a distribution model.
- The operator reference: [[metrics.sample]], [[metrics.series]], [[metrics.correlation]], [[vector.grid_zones]].

## References

- Cushman, S. A., McGarigal, K., & Neel, M. C. (2008). Parsimony in landscape metrics: strength, universality, and consistency. *Ecological Indicators*, 8(5), 691–703. https://doi.org/10.1016/j.ecolind.2007.12.002
- Eigenbrod, F., Hecnar, S. J., & Fahrig, L. (2011). Sub-optimal study design has major impacts on landscape-scale inference. *Biological Conservation*, 144(1), 298–305. https://doi.org/10.1016/j.biocon.2010.09.007
- Hesselbarth, M. H. K., Sciaini, M., With, K. A., Wiegand, K., & Nowosad, J. (2019). landscapemetrics: an open-source R tool to calculate landscape metrics. *Ecography*, 42(10), 1648–1657. https://doi.org/10.1111/ecog.04617
- Jackson, H. B., & Fahrig, L. (2015). Are ecologists conducting research at the optimal scale? *Global Ecology and Biogeography*, 24(1), 52–63. https://doi.org/10.1111/geb.12233
- Pasher, J., Mitchell, S. W., King, D. J., Fahrig, L., Smith, A. C., & Lindsay, K. E. (2013). Optimizing landscape selection for estimating relative effects of landscape variables on ecological responses. *Landscape Ecology*, 28(3), 371–383. https://doi.org/10.1007/s10980-013-9852-6
- Riitters, K. H., O'Neill, R. V., Hunsaker, C. T., Wickham, J. D., Yankee, D. H., Timmins, S. P., et al. (1995). A factor analysis of landscape pattern and structure metrics. *Landscape Ecology*, 10(1), 23–39. https://doi.org/10.1007/BF00158551
