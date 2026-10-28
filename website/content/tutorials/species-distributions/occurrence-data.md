---
title: "Occurrence data: download, clean and thin"
short: Occurrence data
description: Download records of a species from GBIF, flag the suspicious ones with CoordinateCleaner's tests, keep one per cell, thin them against sampling bias, and draw a background with the sampling effort of a target group.
order: 20
level: beginner
time: 45 min
ops: [sdm.download_gbif, sdm.clean_occurrences, sdm.reference_data, sdm.one_per_cell, sdm.thin, sdm.bias_surface, sdm.background, vector.read]
series: Species distribution models
part: 2
---

A species distribution model can be no better than its records, and records are rarely collected for modelling.
In this part you will download the records of a species from the Global Biodiversity Information Facility
(GBIF), find the ones whose coordinates are probably wrong, keep one record per cell of the predictors and thin
the rest so that well-visited places do not dominate the model. At the end you will have a clean set of records
for the brown-throated sloth, and you will know how many each step removed and why: the first lines of the
methods section of a paper.

This is part 2 of the series; [part 1](../ideas/) introduces the ideas and the data.

## The idea

### Where records come from

Most records of species are **opportunistic**: specimens collected over two centuries and kept in museums and
herbaria, and observations by naturalists, birders and, today, the users of apps such as iNaturalist. GBIF
gathers them from thousands of institutions into one index; in October 2026 it held 11 985 records of the
brown-throated sloth with coordinates. They were never designed as a sample of the species' range, and three
kinds of problems follow.

**Wrong coordinates.** A specimen labelled only "Bolivia" may have been given the coordinates of the centre of
Bolivia, or of La Paz; one without any locality may carry the address of the museum that holds it; a longitude
typed without its minus sign puts a Peruvian plant in Asia; an empty field read as zero puts it at 0° 0′ in the
Gulf of Guinea. Such records teach the model the climate of a capital or of the sea.

**Pseudo-replication.** Several records in one cell of the predictors carry the same values of every
predictor. Kept, they weigh that cell several times, and copies of one record can end up on both sides of a
validation split, which flatters the evaluation.

**Sampling bias.** Records cluster where people go: near roads, rivers, cities, research stations and in
countries with many naturalists. Of the sloth's 11 985 records in GBIF, 5 104 (43 %) come from Costa Rica, a
small part of its range. A model fitted to such records learns the bias as if it were a preference of the
species: if the visited places are warmer, warm looks suitable.

![Records of the sloth from GBIF, kept and flagged, and the 116 classic records](/img/tutorials/species-distributions/occurrence-data/gbif.png "The 11 985 records of the brown-throated sloth with coordinates in GBIF (downloaded by sdm.download_gbif in October 2026), after CoordinateCleaner's default tests: 9 495 pass (blue), 2 490 are flagged (orange), most of them on the coasts of Costa Rica and Panama, and in Panama City. In red, the 116 records of the classic data set the series models. Note the dense clusters in Costa Rica and Panama, and the empty interior of the Amazon.")

### The cures

Each problem has a standard cure, and Terraflux has a node for each:

| Problem | Cure | Node |
|---|---|---|
| Wrong coordinates | CoordinateCleaner's tests (Zizka et al., 2019) | [[sdm.clean_occurrences]] |
| Pseudo-replication | one record per cell of the predictors | [[sdm.one_per_cell]] |
| Sampling bias | spatial thinning (Aiello-Lammens et al., 2015) | [[sdm.thin]] |
| Sampling bias | a background drawn with the same bias (Phillips et al., 2009) | [[sdm.bias_surface]], then [[sdm.background]] with `bias` |

!!! science "Key idea: CoordinateCleaner"
    Zizka et al. (2019) collected the errors that most often affect records from biological collections into an
    R package, CoordinateCleaner, with one test per error. Each test compares the records with reference data
    (the capitals, the centroids of countries and provinces, the coastlines, the coordinates of some 11 600
    biodiversity institutions) or with the other records of the same species. Terraflux runs the same tests on
    the same reference data: the tables of CoordinateCleaner travel with Terraflux, and the land, urban areas and
    countries of Natural Earth are downloaded the first time a test needs them.

!!! science "Key idea: spatial thinning"
    Thinning removes records until no two are closer than a chosen distance. Which records to drop is decided at
    random, many times over, and the repetition that keeps the most records wins: the algorithm of the R package
    spThin (Aiello-Lammens et al., 2015). Thinning evens out the density of records, so a well-studied
    national park no longer counts fifty times as much as a remote valley; in tests with real species it often
    improved models (Boria et al., 2014). The price is fewer records.

## When to use it

Clean every set of records that you did not collect yourself, and check your own: typing errors are universal.
One record per cell is always worth doing; [[sdm.prepare]] does it by default. Thin when the records are
clustered in a few well-sampled places, which is the rule for GBIF data, and especially when the clusters
coincide with a particular environment (the cool highlands where researchers live, the coasts where tourists
go).

Thinning is not the only cure for bias. When you have the records of many species collected the same way (all
the mammals of a museum, all the birds of a citizen science scheme), a **target-group background** is often
better: the background points are drawn with the density of those records, so that the bias is the same for the
presences and the background and cancels out (Phillips et al., 2009). In Terraflux, [[sdm.bias_surface]] makes
the map of the group's sampling effort and the `bias` input of [[sdm.background]] draws the background with it.

## What you need

- Records of a species: points in a vector file, a CSV with longitude and latitude columns, or a download from
  GBIF. The tests of coordinates assume **longitude and latitude in degrees** (WGS 84).
- For the tests against countries, a column with the country each record claims (ISO codes of two or three
  letters).
- An internet connection the first time the tests of the sea, urban areas or countries run (Natural Earth's
  polygons are downloaded once), or your own polygons on the `seas_ref`, `urban_ref` and `countries_ref` inputs.
- The predictors' grid, for one record per cell (the climate of [part 1](../ideas/)).
- For a target-group background, the records of the group: points or a table of coordinates, from GBIF
  ([[sdm.download_gbif]]) or your own collections.

Data of this part:

- [acaule.csv](/data/tutorials/species-distributions/occurrence-data/acaule.csv): 1 366 records of a wild potato
  of the Andes, *Solanum acaule*, downloaded from GBIF in 2013 for the R package dismo, with their country,
  locality, institution and basis of record. In the original file the species' name was spelled twelve ways
  (with authors, subspecies and forms); here they are all "Solanum acaule", because the outlier test compares the
  records of each species, and twelve spellings would make twelve small species.
- [bradypus.csv](/data/tutorials/species-distributions/ideas/bradypus.csv) and
  [bioclim.tif](/data/tutorials/species-distributions/ideas/bioclim.tif), the sloth and the climate of part 1.

## Download records from GBIF

[[sdm.download_gbif]] asks GBIF's occurrence search for the records of a species, no account needed. It returns
them as points in longitude and latitude, with their Darwin Core terms (the standard names of biodiversity data:
`decimalLongitude`, `basisOfRecord`, `coordinateUncertaintyInMeters`, `year`...) and GBIF's issues.

```tfgraph
#! caption: Records of the sloth from GBIF (node 1), flagged by CoordinateCleaner's tests (node 2) and shown (node 3).
#! numbered: true
#! highlight: gbif
format = "terraflux/1"

[[nodes]]
id = "gbif"
op = "sdm.download_gbif"
params = { species = "Bradypus variegatus" }

[[nodes]]
id = "clean"
op = "sdm.clean_occurrences"
inputs = { occurrences = "gbif.occurrences" }
params = { action = "flag" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { features = "clean.occurrences" }
params = { title = "GBIF records of the sloth" }
```

The settings of node 1 that matter:

- `species`: a scientific name, matched against GBIF's taxonomic backbone (the `query` output tells you which
  taxon it matched and with what confidence), or a taxon key.
- `country`: one or more two-letter ISO codes (`"BR"`, or `["CR", "PA"]`) to keep only the records of those
  countries; `year`: a year or a range (`[1990, 2020]`), to match the period of the climate layers;
  `basis_of_record`: `PRESERVED_SPECIMEN`, `HUMAN_OBSERVATION`...
- `max_uncertainty`: drop records whose coordinates are less precise than this many metres. With predictors at
  0.5° (55 km), an uncertainty of 10 km does no harm; with predictors at 1 km, it does.
- `has_geospatial_issue`: by default the records GBIF itself flags with geospatial issues are left out.
- `limit`: at most this many records. The search returns up to 100 000 records per query; for more, narrow it by
  country, area (the `area` input: only records in its bounding box) or years, or request a download on gbif.org.

!!! warning "Cite the data"
    The search API gives no DOI. For a publication, request the same records as a download on gbif.org, which
    gives a DOI to cite, and read it with [[vector.read]] (or with [[bio.occurrences]], which reads GBIF's zip
    files directly).

For the sloth the query found 11 985 records, every one of them returned (`"truncated": false` in the `query`
output). They span two centuries, but most are recent observations:

```tfchart
type: bar
data: /data/tutorials/species-distributions/occurrence-data/gbif_decades.csv
x: decade
y: [Specimens, Observations, Other]
colors: { Specimens: "#3b8a5a", Observations: "#2f6f9f", Other: "#b9c2bc" }
stacked: true
xlabel: Decade
ylabel: Records
height: 300
caption: The GBIF records of the sloth by decade and basis of record. Before 1980 they are museum specimens; since 2000, observations (mostly photographs shared through iNaturalist), 8 452 of them in the 2020s alone. Recent records match recent climate layers better, but they are also the most clustered around towns and parks.
```

## Clean the records

[[sdm.clean_occurrences]] runs the tests and either drops the records that fail any of them (`action = drop`,
the default) or keeps all of them with a column per test (`flag_capitals`, `flag_seas`...) and a column
`flagged` (`action = flag`). Flag first, look at the suspects on a map, then decide.

Let us clean the wild potato, a plant of the high Andes whose 1 366 records have the usual problems:

```tfgraph
#! caption: The records of Solanum acaule (node 1) flagged by CoordinateCleaner's tests (node 2), with the test of the country each record claims (column ISO2).
#! numbered: true
#! highlight: clean
format = "terraflux/1"

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "acaule.csv" }

[[nodes]]
id = "clean"
op = "sdm.clean_occurrences"
inputs = { occurrences = "records.vector" }
params = { action = "flag", countries = "ISO2" }
```

<div class="steps" markdown>

1. Add [[vector.read]] (library subject **IO**) with `path = acaule.csv`. Connect its **vector** port: it carries
   every column of the CSV, including the coordinates (`lon`, `lat`) and the text columns the tests use.
2. Add [[sdm.clean_occurrences]] (subject **Species distributions**) and drag from `records.vector` to its
   `occurrences` input. In the Node pane set `action = flag` and `countries = ISO2`, the column of the country
   codes. The test of countries runs only when you name that column.
3. Select the node and run it (++ctrl+enter++). Its `counts` output, under the canvas, gives the records each test
   flagged.

</div>

Each test takes `false` (skip it), `true` (its defaults), a number (its main option, usually a distance) or an
object of its options. The tests and their defaults:

| Test | Flags records... | Default |
|---|---|---|
| (invalid) | without coordinates, or outside −180 to 180 and −90 to 90 | always |
| `equal` | with the same longitude and latitude | on |
| `zeros` | within 0.5° of 0° 0′ | on (0.5 degrees) |
| `capitals` | within 10 km of a country's capital | on (10 000 m) |
| `centroids` | within 1 km of the centroid of a country or province | on (1 000 m) |
| `seas` | in the sea, against Natural Earth's land at the scale 1:50 million | on (scale 50) |
| `urban` | in Natural Earth's urban areas | off |
| `countries` | outside the country they claim | off (needs the column) |
| `outliers` | far from the other records of their species | on (quantile method, multiplier 5) |
| `gbif` | within 1 km of GBIF's headquarters in Copenhagen | on (1 000 m) |
| `institutions` | within 100 m of a biodiversity institution (museum, herbarium, university) | on (100 m) |
| `duplicates` | with the same species and coordinates as an earlier record | off |
| `aohi` | at artificial hotspots of occurrence (Park et al., 2023)[^aohi] | off |

[^aohi]: Places where records accumulate for reasons that have nothing to do with the species. The reference
    table travels with Terraflux; the test's `taxa` option selects the groups whose hotspots count.

For the potato the tests flag 324 records:

```tfchart
type: bar
data: /data/tutorials/species-distributions/occurrence-data/acaule_counts.csv
x: test
y: [records]
colors: { records: "#e8901f" }
xlabel: Test
ylabel: Records flagged
height: 280
caption: Records of Solanum acaule flagged by each test. A record may fail several tests; 324 fail at least one, and 1 042 pass them all.
```

284 records have no coordinates at all (`invalid`); 28 lie outside the country they claim, 24 far from all the
others, 15 in the sea, 6 near 0° 0′, 5 at a capital and 2 have equal longitude and latitude.

![Records of Solanum acaule kept and flagged, by test](/img/tutorials/species-distributions/occurrence-data/acaule_flags.png "The records of Solanum acaule with coordinates: kept (blue) and flagged (diamonds, by test). A 'Bolivian' record in southern Brazil, 20 degrees east of the Andes (outlier and wrong country), records in the lowlands of Peru far from the others (outliers), two specimens on La Paz (capital).")

A look at the suspects shows why each was flagged:

| Country claimed | Longitude | Latitude | Flagged by |
|---|---|---|---|
| Bolivia | −46.08 | −21.43 | countries, outliers |
| Peru | −76.62 | −7.47 | outliers |
| Bolivia | −68.15 | −16.50 | capitals |
| Antarctica | −76.30 | −76.30 | equal, outliers |
| Argentina | −66.03 | −21.88 | countries |
| Bolivia | −69.35 | −15.01 | countries |

The "Bolivian" record at 46° W lies in Brazil, 20 degrees east of the Andes: probably a mistyped longitude. Two
specimens sit on La Paz, a locality georeferenced to the city. A record in "Antarctica" has the same number as
longitude and latitude. Others are less clear: the "Argentinian" records at 21.9° S fall a few kilometres from
the border, and a herbarium label may be right about the country and slightly wrong about the point. This is why
flagging comes before dropping. With `action = drop` the node returns the 1 042 records that pass every test.

### The tests on GBIF's sloths

The same tests on the 11 985 GBIF records of the sloth flag 2 490 of them: 1 823 in the sea, 606 within 10 km of a
capital, 125 at institutions and 3 at centroids. Look at the map above: most of the "sea" records are on the
beaches of Costa Rica and Panama, where sloths are photographed in the trees along the shore, and the coastline at
the scale 1:50 million passes a few hundred metres inland. Most of the "capital" records are in Panama City,
whose parks hold real sloths. The tests are right about the specimens of the past, and too strict for
precise photographs of today.

!!! tip "Tune the tests to the data"
    For coastal species, use Natural Earth's finer land (`seas = 10`, the scale 1:10 million) or your own
    coastline on the `seas_ref` input; for species that live in cities, `capitals = false`. Precise records (a
    `coordinateUncertaintyInMeters` of a few metres) rarely suffer from centroid errors. [[sdm.reference_data]]
    returns any of the reference data sets (capitals and centroids, institutions, Natural Earth's land, urban
    areas and countries) as features, to draw them over your records and see what the tests compare against.

## One record per cell

[[sdm.one_per_cell]] keeps the first record in each cell of a raster's grid and drops those outside it
(`drop_missing = true` also drops those on cells without a value, such as the sea of a climate layer). The model
sees the predictors only at the resolution of their cells: two records 5 km apart in a cell of 55 km are the same
record to it.

```tfgraph
#! caption: The sloth's records cleaned (node 3), one per cell of the climate (node 4) and thinned so that none is within 50 km of another (node 5).
#! numbered: true
#! highlight: cells, thin
format = "terraflux/1"

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "bradypus.csv" }

[[nodes]]
id = "climate"
op = "raster.read"
params = { path = "bioclim.tif" }

[[nodes]]
id = "clean"
op = "sdm.clean_occurrences"
inputs = { occurrences = "records.vector" }

[[nodes]]
id = "cells"
op = "sdm.one_per_cell"
inputs = { occurrences = "clean.occurrences", raster = "climate.raster" }

[[nodes]]
id = "thin"
op = "sdm.thin"
inputs = { occurrences = "cells.occurrences" }
params = { distance = 50000 }
```

For one record per cell only the grid matters, so connecting the climate's `stack` (every band) or its `map` (the
first band) gives the same result.

Of the sloth's 116 records, 113 pass the tests (one at a capital, one in the sea, one at an institution) and 92
remain one per cell: 21 records shared a cell with an earlier one.

## Thin the records

[[sdm.thin]] removes records at random until none is closer than `distance` metres to another (great-circle
distances on longitude and latitude), repeats this `reps` times (10 by default) and keeps the repetition that
kept the most. Its `kept` output lists how many each repetition kept. Move the slider:

```tfslider
label: Thinning distance (km)
caption: The 92 records of the sloth (one per cell) thinned at increasing distances; each shaded circle has a radius of half the distance, so that no two circles overlap. 25 km removes almost nothing at this resolution; 50 km evens out the clusters of Central America and the Andes; beyond 100 km whole regions are reduced to a few records.
frames:
  - { value: 0, src: /img/tutorials/species-distributions/occurrence-data/thin_0.png, stats: { Records: "92" } }
  - { value: 25, src: /img/tutorials/species-distributions/occurrence-data/thin_25.png, stats: { Records: "88" } }
  - { value: 50, src: /img/tutorials/species-distributions/occurrence-data/thin_50.png, stats: { Records: "83" } }
  - { value: 100, src: /img/tutorials/species-distributions/occurrence-data/thin_100.png, stats: { Records: "63" } }
  - { value: 200, src: /img/tutorials/species-distributions/occurrence-data/thin_200.png, stats: { Records: "42" } }
  - { value: 400, src: /img/tutorials/species-distributions/occurrence-data/thin_400.png, stats: { Records: "22" } }
chart:
  type: line
  data: /data/tutorials/species-distributions/occurrence-data/thinning.csv
  x: distance_km
  y: [records]
  colors: { records: "#d1495b" }
  xlabel: Distance (km)
  ylabel: Records kept
```

How to choose the distance? There is no rule, but three guides:

- **At least the size of a cell** of the predictors: below it, one record per cell has done the work (25 km
  removes only 4 records here, with cells of about 55 km).
- **The scale of the bias**: the size of the clusters of records you want to even out (a national park, the
  surroundings of a town). Look at the map.
- **What you can afford**: every record removed is information lost. Studies often use 10 to 50 km for cells of
  1 to 10 km. For the sloth, 50 km removes 9 records and evens out the worst clusters; we use it in the rest of the
  series.

Thinning is random: another `seed` may keep different records (the same number or nearly). Report the distance
and the seed.

## Sampling bias and the background

Thinning acts on the presences. The other cure acts on the background: draw it with the same bias. The
**background** (part 3) is a sample of the places available to the species, which the model compares with the
places where it was recorded. If the records crowd along rivers and around a few research stations while the
background is spread evenly, part of what the model learns is "where collectors went", not "where sloths live".

!!! science "Key idea: a target-group background"
    The records of a **target group**, all the species recorded by the same methods (the same collections, the same
    collectors, the same decades), show where people looked. Phillips et al. (2009) proposed to draw the background
    where the group was recorded: it then has the same geographic bias as the presences, and what still sets the
    presences apart is the environment the species chose. Over 226 species from diverse regions, the target-group
    background improved the average performance of every method they tried, by as much as the choice of method
    itself, and most where the group's records were most biased. Barber et al. (2022) compared four corrections on
    the hoverflies of Britain, whose sampling effort is known: the target-group background came closest to it, though
    it overcompensated in places.

### The sampling effort as a surface

[[sdm.bias_surface]] turns the group's records into a map of sampling effort: a **kernel density**, a smooth
surface that is high where records are many and close together, and falls off with the distance to them. Every
record puts a small bell-shaped hill on the map, and the surface is their sum, evaluated at the centre of every cell
of the predictors.

!!! science "Key idea: the kernel density"
    At a cell whose centre is \((x, y)\), with the group's records at \((x_k, y_k)\) and weights \(w_k\):

    \[ b(x, y) = \frac{1}{\sum_k w_k} \sum_k \frac{w_k}{h_x h_y} \, \phi\!\left(\frac{x - x_k}{h_x}\right)
    \phi\!\left(\frac{y - y_k}{h_y}\right) \]

    \(\phi\) is the standard normal density (the bell curve), and \(h_x\), \(h_y\) the **bandwidths**: the standard
    deviations of the bell in x and in y, in the units of the grid (degrees here, metres on a projected grid). A wide
    bandwidth makes a smooth surface that ignores small clusters; a narrow one sticks to the records. The default
    is R's normal reference rule, `bw.nrd`, in each direction:
    \(h = 1.06 \, \min(s, \mathrm{IQR}/1.34) \, n^{-1/5}\), where \(s\) is the standard deviation of the records'
    coordinates, IQR their interquartile range and \(n\) the number of records. The surface is the one R's
    `MASS::kde2d` computes; note that `kde2d(h = H)` corresponds to `bandwidth = H / 4` here. Dividing by
    \(\sum_k w_k\) makes \(b\) a density whose total over the plane is 1: the share of the group's records per unit
    of area (per square degree on this grid). Only its relative values matter for the background.

Such a surface is what Maxent calls a "bias file". Fourcade et al. (2014) made theirs as a Gaussian kernel density
of the records, rescaled from 1 to 20, and compared it with other corrections on a virtual species: thinning the
records was the only one that did well whatever the species and the kind of bias. Kramer-Schadt et al. (2013),
with the Malay civet of Borneo, also found thinning better, and a biased background the choice when records are
too few to thin. Both built the bias from the species' own records; a target group's records are better evidence
of where people looked, since they do not also hold where the species is common. Thinning and a target-group
background can be combined, as below.

For the sloth, we take as the target group the **museum specimens** of its order, Pilosa (sloths and anteaters),
and of the armadillos, Cingulata: 5 056 specimens with coordinates in GBIF (1 876 and 3 180), downloaded with
[[sdm.download_gbif]] (`species` = GBIF's taxon keys 1494 and 735, `basis_of_record = "PRESERVED_SPECIMEN"`, in
October 2026) and saved together in one file with [[vector.write]]. The group goes through the same cleaning as
the presences: 294 records are flagged (132 in the sea, 50 at capitals, 30 at country or province centroids, 73
outliers of their species, 11 at institutions; a record can fail several tests), and 4 762 remain, from 118
institutions and nearly two centuries (1826 to 2025). In a real study, choose the group by how the presences were
collected: specimens for specimens, the observations of the same scheme for observations.

```tfgraph
#! caption: A target-group background. The sloth's records are cleaned and thinned as above (nodes 4 to 6); the group's specimens (node 3) are cleaned the same way (node 7) and turned into a surface of sampling effort (node 8), which weights the draw of the background (node 9).
#! numbered: true
#! highlight: group, clean_group, effort, background
format = "terraflux/1"

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "bradypus.csv" }

[[nodes]]
id = "climate"
op = "raster.read"
params = { path = "bioclim.tif" }

[[nodes]]
id = "group"
op = "vector.read"
params = { path = "xenarthra.gpkg" }

[[nodes]]
id = "clean"
op = "sdm.clean_occurrences"
inputs = { occurrences = "records.vector" }

[[nodes]]
id = "cells"
op = "sdm.one_per_cell"
inputs = { occurrences = "clean.occurrences", raster = "climate.raster" }

[[nodes]]
id = "thin"
op = "sdm.thin"
inputs = { occurrences = "cells.occurrences" }
params = { distance = 50000 }

[[nodes]]
id = "clean_group"
op = "sdm.clean_occurrences"
inputs = { occurrences = "group.vector" }

[[nodes]]
id = "effort"
op = "sdm.bias_surface"
inputs = { occurrences = "clean_group.occurrences", env = "climate.stack" }

[[nodes]]
id = "background"
op = "sdm.background"
inputs = { env = "climate.stack", occurrences = "thin.occurrences", bias = "effort.bias" }
params = { n = 2000, tryf = 1, seed = 1 }
```

<div class="steps" markdown>

1. **The group's records** (node 3): [[vector.read]] of the group's points (`xenarthra.gpkg`, the GBIF records of
   both orders). A table with coordinate columns works as well: columns named `x`/`y`, `lon`/`lat`,
   `longitude`/`latitude` or GBIF's `decimalLongitude`/`decimalLatitude` are found by themselves, and the `x` and
   `y` settings of [[sdm.bias_surface]] name others. Records in another coordinate system are transformed to the grid's.
2. **Clean them** (node 7): the same [[sdm.clean_occurrences]] as the presences. A specimen at a capital is no
   more evidence of effort there than of the species.
3. **The effort** (node 8): add [[sdm.bias_surface]] from the library's *Species distributions* subject. Connect
   the cleaned group to `occurrences` and the predictors to `env` (a stack or a single map: only its grid is used,
   and, with `mask` on, which cells have a value in every predictor). Its outputs: `bias`, the surface on the
   grid of the predictors, and `bandwidth`, the two bandwidths it used, `[x, y]`. Its settings:
    - `bandwidth`: `nrd` (the rule above, the default), one number for both directions, or `[x, y]`, in the grid's
      units;
    - `isotropic`: the larger of the two bandwidths in both directions (default `false`);
    - `limits`: `[min, max]`, the rule's bandwidths raised to the minimum or lowered to the maximum (either may be
      left empty, `null`); a floor keeps a tightly clustered group from making a surface of spikes on its records;
    - `weights`: a column of the records with each one's weight (specimens in a lot, visits to a site); default 1
      each;
    - `rescale`: `[lo, hi]`, the densities mapped linearly onto this range, for instance `[0.05, 0.95]`, so that
      every cell keeps some chance of being drawn;
    - `mask`: no value on the cells without a value in every predictor (default `true`).
4. **The background** (node 9): connect `effort.bias` to the `bias` input of [[sdm.background]]. Each cell is then
   drawn with a chance proportional to its effort (on a longitude-latitude grid, also to its width, as without a
   bias); cells with no value are never drawn, nor cells of effort 0. Set `tryf = 1` (below).

</div>

With the default rule, the bandwidths are 3.34° in longitude and 4.43° in latitude (about 370 and 490 km at the
equator):

```tfslider
label: Bandwidth (degrees)
caption: "The sampling effort of the target group (sdm.bias_surface), relative to the average cell (1), on a logarithmic scale; the group's 4 762 cleaned specimens in white. With the default rule (nrd, 3.34° by 4.43°) the surface is smooth: the busiest cell has 4.6 times the effort of the median cell. With a bandwidth of 1° the clusters of collecting show (Texas, with 448 specimens of the nine-banded armadillo; the Andes of Colombia; Paraguay and northern Argentina) and so do the gaps, such as the central Amazon and north-eastern Brazil: the busiest cell has 33 times the median's effort."
frames:
  - { value: "1", src: /img/tutorials/species-distributions/occurrence-data/bias_surface_1.png, stats: { "Busiest / median cell": "32.7", "Cells below 0.1": "8.0 %" } }
  - { value: "2", src: /img/tutorials/species-distributions/occurrence-data/bias_surface_2.png, stats: { "Busiest / median cell": "9.8", "Cells below 0.1": "4.4 %" } }
  - { value: "nrd (3.34, 4.43)", src: /img/tutorials/species-distributions/occurrence-data/bias_surface_nrd.png, stats: { "Busiest / median cell": "4.6", "Cells below 0.1": "2.0 %" } }
start: 2
play: false
```

The rule of `bw.nrd` was made for a single, roughly normal cloud of points. A group spread over a continent in
several clusters gets a wide bandwidth from it, which smooths the clusters away. There is no correct bandwidth;
choose one in the range of the clusters you can see on a map of the group's records, compare the surfaces, and
report the one you used.

### How the bias reaches the background

[[sdm.background]] draws its points as R's dismo does: it first draws `n × tryf` candidate cells **without
replacement**, with the chances given by the bias, leaves out the cells of the records, and then keeps `n` of the
candidates at random. With the default `tryf = 3`, the 2 000 points come from 6 000 candidates, 61 % of the 9 775
cells with a climate: nearly all the busy cells are among the candidates, but so are most of the others, and the final
random choice dilutes the bias. With `tryf = 1` the bias acts fully. The cost: candidates that fall on a record's
cell are dropped and not replaced, so the sloth's background has 1 975 points instead of 2 000 (the node warns).

```tftable
data: /data/tutorials/species-distributions/occurrence-data/target_group.csv
digits: 2
caption: "The sampling effort (the default surface, 1 = the map's average) at the points of four backgrounds of the sloth, and the share of the points in the quarter of the cells with the most effort. A background drawn at random has the map's average effort and a quarter of its points in the busiest quarter; the target-group background with tryf = 1 has half of them there."
```

```tfcompare
caption: The sloth's 83 thinned records (red) with 2 000 background points drawn at random from the whole map (left of the handle) and 1 975 drawn with the effort of the target group, tryf = 1 (right). The target-group background crowds into Texas and Mexico, the Andes of Colombia and the Chaco of Paraguay and Argentina, where the group was collected, and thins out over the central Amazon, north-eastern Brazil and Patagonia.
before: { src: /img/tutorials/species-distributions/occurrence-data/background_random.png, label: Random }
after: { src: /img/tutorials/species-distributions/occurrence-data/background_group.png, label: Target group }
```

The difference in climate is smaller than the difference on the map: the median annual rainfall at the background
points goes from about 1 240 mm (random) to 1 250 mm (target group), against 2 496 mm at the sloth's records. The group
was collected in dry places as well as wet ones. The background's purpose is not to look like the presences, but to
share their bias; what the model learns from it is part 4's subject.

The template [Species distribution model](../ideas/#a-virtual-species-first) uses the same input the other way
round: it draws the *presences* of its virtual species with the suitability as the bias. [Part
3](../predictors-and-background/) covers the background in full: how many points, from which area.

## Read the results

What to keep and report from this part:

- **How many records at each step**: downloaded (or read), passing the tests (and which tests removed how many:
  the `counts` output), one per cell, thinned. For the sloth: 116, 113, 92, 83.
- **For a target-group background**: the group (taxa, kind of record, source and date of the download), its
  records before and after cleaning (5 056 and 4 762), the bandwidths (the `bandwidth` output: 3.34° and 4.43°), any
  `limits`, `weights` or `rescale`, and the background's `n`, `tryf` and seed.
- **The settings**: the tests used and their options, the thinning distance and its seed, the date of the GBIF
  query and its filters (country, years, uncertainty).
- **A map** of the records kept and removed, in the supplementary material.

A good sign: the records removed are few and their reasons are clear on the map. A bad sign: a test removes a
large share of the records (the 1 823 coastal sloths): check whether it is right before accepting it.

## Settings that change the answer

- **The thinning distance** (the slider above): from 92 to 22 records between 0 and 400 km.
- **The scale of the sea test**: coastal records stay or go depending on the coastline's detail.
- **The outlier test** (`outliers`): its `method` (`quantile`, `mad` or `distance`), its multiplier `mltpl` (5)
  and, for the distance method, `tdi`, the distance in km beyond which a record is an outlier (1 000). A species
  with a few genuine disjunct populations (on isolated mountains, for example) can lose them to this test; check
  the flagged records before dropping them.
- **The GBIF filters**: limiting the records to the years of the climate layers, or to precise coordinates,
  changes both the number of records and their bias.
- **The bandwidth of the effort** (the slider above): from 1° to the rule's 3.3° by 4.4°, the busiest cell goes
  from 33 to 4.6 times the median one.
- **`tryf` with a bias**: with the default of 3, the sloth's background has 37.5 % of its points in the busiest
  quarter of the map; with 1, 51.2 % (a random background: 23.9 %).
- **`rescale`**: `[0.05, 0.95]` gives the emptiest cell a chance 19 times smaller than the busiest one's, instead
  of nearly none; here it moves the share in the busiest quarter from 51.2 % to 46.9 %.

## Common pitfalls

- **Coordinates that are not longitude and latitude.** The tests compare with reference data in degrees. Project
  records in metres back to WGS 84 first.
- **Twelve spellings of one species.** The outlier test works per species: unify the names, or name the column of
  the species with the `species` setting.
- **Dropping without looking.** Flag first and map the flags. A test can be wrong for your data.
- **Old specimens and new climate.** A record of 1920 in a place that is now a city or a pasture is still evidence
  of the climate the species tolerates, but not of the land cover. Filter by years if your predictors include land
  cover.
- **Uncertain coordinates and fine predictors.** A record with an uncertainty of 20 km is useless with predictors
  at 1 km. Use `max_uncertainty`.
- **Thinning too hard.** With 20 records left, the model will be crude whatever the bias.
- **A target group that was not sampled like the presences.** Specimens of a group show where museums collected,
  not where a citizen-science scheme observed. A group whose effort differs from the presences' brings a bias of
  its own.
- **Not looking at the group's records.** They need the same cleaning as the presences, and a look: among the
  group's specimens in Texas are 18 of *Glyptotherium texanum*, an extinct glyptodont. Filter by `year` or by
  species where it matters.
- **A bias diluted by `tryf`.** With the default `tryf = 3` and a coarse grid, the candidates cover most of the
  map; use `tryf = 1` with a bias.
- **Bandwidths in degrees.** On a longitude-latitude grid the bandwidths are in degrees, and a degree of
  longitude shrinks towards the poles (to 87 % of its equatorial width at 30°, half at 60°): over a wide range of
  latitudes, the kernel is narrower in kilometres far from the equator. A projected grid in metres avoids this.

## Going further

- [Part 3: Predictors and the accessible area](../predictors-and-background/): the background, the area it is
  drawn from and the predictors.
- [Part 7: The whole preparation in one node](../prepare-in-one-node/): cleaning, one per cell and thinning are the
  first steps of [[sdm.prepare]] (`clean`, `one_per_cell`, `thin`).
- To count many species in grid cells from a GBIF download, see the [biodiversity tutorials](../../biodiversity/).
- The operators: [Species distributions](/operators/species-distributions/).

## References

- Aiello-Lammens, M. E., Boria, R. A., Radosavljevic, A., Vilela, B., & Anderson, R. P. (2015). spThin: an R package for spatial thinning of species occurrence records for use in ecological niche models. *Ecography*, 38(5), 541–545. https://doi.org/10.1111/ecog.01132
- Barber, R. A., Ball, S. G., Morris, R. K. A., & Gilbert, F. (2022). Target-group backgrounds prove effective at correcting sampling bias in Maxent models. *Diversity and Distributions*, 28(1), 128–141. https://doi.org/10.1111/ddi.13442
- Boria, R. A., Olson, L. E., Goodman, S. M., & Anderson, R. P. (2014). Spatial filtering to reduce sampling bias can improve the performance of ecological niche models. *Ecological Modelling*, 275, 73–77. https://doi.org/10.1016/j.ecolmodel.2013.12.012
- Fourcade, Y., Engler, J. O., Rödder, D., & Secondi, J. (2014). Mapping species distributions with MAXENT using a geographically biased sample of presence data: a performance assessment of methods for correcting sampling bias. *PLoS ONE*, 9(5), e97122. https://doi.org/10.1371/journal.pone.0097122
- Kramer-Schadt, S., Niedballa, J., Pilgrim, J. D., Schröder, B., Lindenborn, J., Reinfelder, V., Stillfried, M., Heckmann, I., Scharf, A. K., Augeri, D. M., Cheyne, S. M., Hearn, A. J., Ross, J., Macdonald, D. W., Mathai, J., Eaton, J., Marshall, A. J., Semiadi, G., Rustam, R., Bernard, H., Alfred, R., Samejima, H., Duckworth, J. W., Breitenmoser-Wuersten, C., Belant, J. L., Hofer, H., & Wilting, A. (2013). The importance of correcting for sampling bias in MaxEnt species distribution models. *Diversity and Distributions*, 19(11), 1366–1379. https://doi.org/10.1111/ddi.12096
- Park, D. S., Xie, Y., Thammavong, H. T., Tulaiha, R., & Feng, X. (2023). Artificial Hotspot Occurrence Inventory (AHOI). *Journal of Biogeography*, 50(2), 441–449. https://doi.org/10.1111/jbi.14543
- Phillips, S. J., Dudík, M., Elith, J., Graham, C. H., Lehmann, A., Leathwick, J., & Ferrier, S. (2009). Sample selection bias and presence-only distribution models: implications for background and pseudo-absence data. *Ecological Applications*, 19(1), 181–197. https://doi.org/10.1890/07-2153.1
- Zizka, A., Silvestro, D., Andermann, T., Azevedo, J., Duarte Ritter, C., Edler, D., Farooq, H., Herdean, A., Ariza, M., Scharn, R., Svantesson, S., Wengström, N., Zizka, V., & Antonelli, A. (2019). CoordinateCleaner: standardized cleaning of occurrence records from biological collection databases. *Methods in Ecology and Evolution*, 10(5), 744–751. https://doi.org/10.1111/2041-210X.13152
