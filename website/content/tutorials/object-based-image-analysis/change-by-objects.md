---
title: "Change by objects: two dates and a fuzzy transition matrix"
short: Change by objects
description: "Compare two Sentinel-2 dates of the same edge of Lago Paranoa object by object, bind the second classification to the first with a fuzzy transition matrix, and learn how much of the change you find is the maps' own error."
order: 110
level: advanced
time: 90 min
template: Land cover change by objects
ops: [data.stac, segment.multiresolution, objects.features, objects.samples, ml.fit, objects.classify, table.of, objects.temporal, objects.transition_matrix, objects.to_map]
series: Object-based image analysis
part: 11
---

A city grows, a forest is cleared, a lake shrinks. To measure it you need two images of the same place, and a way
to say, object by object, what the land was and what it is. This tutorial does that for the edge of Lago Paranoá in
Brasília with two Sentinel-2 scenes five years apart (13 August 2019 and 9 August 2024). You will cut each date into objects, classify
both with one model, bind the second date to the first with [[objects.temporal]] and a **fuzzy transition matrix**, and read the
table of what changed to what. And you will learn the main lesson of the method, the one that decides whether a change
map can be believed: **comparing two maps adds their errors**, and on this image the errors are larger than most of the change.

## The idea

### Comparing two dates

There are two broad ways to find change in two images (Hussain et al., 2013; Tewkesbury et al., 2015).

- **Compare the values.** Subtract the images (the NDVI of the second date minus the first), or look at how far the pixel
  moved in the band space. You get *how much* the surface changed, with no classes. It is sensitive to anything that changes the
  numbers: the season, the sun, the atmosphere.
- **Compare the classifications.** Classify each date and cross the two maps. You get *from what to what*: forest to urban,
  cerrado to bare soil. This is **post-classification comparison**, and it is what this page does with objects.

Objects help in both. The change is judged for an object and not for a pixel, so the speckle of single changed
pixels disappears and a change has a size and a shape (Chen et al., 2012). They bring a new difficulty too: the
objects of the two dates are not the same (each date is segmented on its own) and have to be linked.

### Why the errors add up

Say each map is right for 85 % of its objects. A place that did not change is wrongly called "changed" whenever **either** date
is wrong about it, and that is about \( 1 - 0.85 \times 0.85 \approx 28\,\% \) of the stable land if the errors were independent. A real change of a few percent
of the area drowns in it. This is the weakness of post-classification comparison, known for decades, and the reason a transition table
read straight from two maps cannot be trusted.

!!! science "Key idea"
    **Net change hides the error and gross change shows it.** The *net* change of a class is its area on the second date minus
    on the first. The *gross* change is every cell that changed class, in both directions. A classifier that is noisy
    but not biased gives a net change near the truth and a gross change far above it. Report the net change for
    the areas; do not read the gross change as the change that happened.

### The fuzzy transition matrix

Some of the errors can be ruled out *before* looking at the image, by what you know about the world: in five years
a lake does not turn into forest and a city block does not turn into cerrado. Feitosa and colleagues (2009) put it into a rule: a **transition
matrix** of **possibilities**, \( \pi_{ij} \) from 0 (impossible) to 1 (possible), for a place that was class \( i \) on the first date and
is class \( j \) on the second. InterIMAGE, the program of Costa et al. (2010), uses it in its multi-temporal analysis, and
Terraflux does the same.

The classifier gives each object a **membership** in every class, \( \mu_j \) between 0 and 1 (for the random forest of part 5, the
share of trees that vote for \( j \)). The membership of the second date is then cut to what the first date and the matrix allow:

\[ \mu'_j(o) = \min\Big(\mu_j(o),\; \max_i \min\big(\mu^{\text{before}}_i(o),\; \pi_{ij}\big)\Big), \]

where \( o \) is an object of the second date, \( \mu^{\text{before}}_i(o) \) is the mean, over the cells of \( o \), of the memberships of the objects of the first
date that lie under those cells (this is how objects of different segmentations are linked), and \( \pi_{ij} \) comes from the matrix. In words: the
second date can be class \( j \) only as far as some class \( i \) of the first date, which the object really had, may become \( j \). The
class of the object is the highest \( \mu'_j \); if even that is under `min`, the object gets no class.

## When to use it

- You have **two classifications** of the same place on the same grid, with memberships, and a question about *what became what*
  (deforestation, urban growth, wetland loss).
- You know **which transitions are impossible or rare** in the time between the dates.
- You want the change as **patches**, with a size, to check one by one.

It is not the tool for subtle change (a forest degraded but still forest, a crop in a different state): the classes have to differ. It
needs two dates that can be compared: the same season, the same sensor, atmospherically corrected scenes. For the change as a
number, without classes, subtract an index. For the same thing on a table or a stack, with principal components, see the template
*Change between two dates by principal components*. And a map of change used for an area estimate still needs a reference sample, as in
part 5 (Olofsson et al., 2013, 2014).

## What you need

- **Two dates on the same grid.** Both are Sentinel-2 L2A of the tile 22LHH at 10 m, clipped to the same 580 by 580 cells, both
  at the end of the dry season (the cerrado is dry and the days are cloud-free). The 2024 scene is the one of the whole series
  ([sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif)); the 2019 scene is
  [sentinel2_brasilia_2019.tif](/data/tutorials/object-based-image-analysis/change-by-objects/sentinel2_brasilia_2019.tif)
  (same grid, Int16; the attribution is in [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/change-by-objects/ATTRIBUTION.txt)).
- **Training samples**, of **places that did not change** between the dates, so that one set of points can teach the classifier what each
  class is: the 93 points of part 5
  ([training_samples.csv](/data/tutorials/object-based-image-analysis/change-by-objects/training_samples.csv)).
- **A matrix** of what can become what. Here, written by hand.
- **A template**: *Land cover change by objects* is a small model with synthetic images, classed by rules, that has this chain; the page does
  it on the real scenes, classed by a random forest.

![The two dates](/img/tutorials/object-based-image-analysis/change-by-objects/rgb_2024.png "The 2024 scene in true colours (the clip of the other tutorials: 5.8 by 5.8 km at the south edge of the lake). Compare it with the 2019 scene in the slider below.")

```tfcompare
caption: Drag the handle to compare the two dates in true colours. The lake, the town and the cerrado are the same; look at the new roofs and the cleared ground.
before: { src: /img/tutorials/object-based-image-analysis/change-by-objects/rgb_2019.png, label: 13 August 2019 }
after: { src: /img/tutorials/object-based-image-analysis/change-by-objects/rgb_2024.png, label: 9 August 2024 }
```

Not much changed, and what did is small: the mean NDVI difference between the dates is 0.01, and only 1.8 % of the cells lost more than 0.2 of NDVI and
1.6 % gained that much. The 2019 scene is a little brighter than the 2024 one (the median blue is 387 against 327, the red 749
against 689, the NIR 2076 against 1954, in reflectance times 10,000), the usual difference between two days: keep it in mind.

## Build the model

### Steps 1 to 4: each date in objects

[[data.stac]] reads each scene from Earth Search over the same box in degrees; [[raster.reproject]] puts it on the same UTM grid
(`extent` and `resolution` are the same for both, so the cells coincide) and [[raster.calc]] makes the reflectance. For the 2019 day the
catalogue has the scene twice (processing baselines 02.13 and 05.00): the `query` setting, a JSON text, keeps one. Each date goes through
[[segment.multiresolution]] at the scale 2 and [[objects.features]] with the same settings, the twelve attributes of part 5: **both dates need
the same attributes** for one model to read them.

```tfgraph
#! caption: Steps 1 to 4 (the nodes of the 2019 date): the earlier scene over the same box and grid, its reflectance, its objects and their attributes. The 2024 date has the same four nodes and one more for the clip.
#! highlight: s2_e, utm_e, refl_e, seg_e, feat_e
#! numbered: true
#! fragment: true
[[nodes]]
id = "s2_e"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2019-08-13", query = "{\"s2:processing_baseline\": {\"eq\": \"02.13\"}}", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }
note = "13 August 2019."

[[nodes]]
id = "utm_e"
op = "raster.reproject"
inputs = { raster = "s2_e.raster" }
params = { crs = "EPSG:31983", resolution = 10, extent = [190520, 8241280, 196320, 8247080] }
note = "The same grid as the 2024 clip."

[[nodes]]
id = "refl_e"
op = "raster.calc"
inputs = { s = "utm_e.raster" }
params = { expression = "out image = s / 10000" }

[[nodes]]
id = "seg_e"
op = "segment.multiresolution"
inputs = { image = "refl_e.image" }
params = { scale = 2 }
note = "Objects of the first date: its own segmentation."

[[nodes]]
id = "feat_e"
op = "objects.features"
inputs = { objects = "seg_e.objects", image = "refl_e.image" }
params = { features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"], texture_bands = ["brightness"], swir = "swir16" }
```

### Steps 5 to 8: one classifier for both dates

[[objects.samples]] picks the training objects of the 2024 date under the points, [[ml.fit]] learns a random forest from them (with `columns`
naming the twelve attributes: part 5 says why), and **two** [[objects.classify]] nodes apply the same model, one to the objects of each date. The
`samples` input gives the classes their names, which is what lets the two dates and the matrix agree on "forest" and "urban".

Why one model and not one per date? The matrix compares classes, and a single definition of each class on both dates makes a change a
change in the image and not in the classifier. The price is the difference in brightness between the dates noted above, which
the model has to absorb. The samples are of places that did not change, an assumption you
should check (a training point that was a field in 2019 and a roof in 2024 teaches the wrong class to one of the dates).

### Step 9: the matrix

The matrix is a table with a column `class` (text) and one column per class; the row is the class of the first date and the numbers
are the possibilities of each class on the second. It is made with [[table.of]]. This one, written by hand for five years in the
Brasília area:

```tftable
data: /data/tutorials/object-based-image-analysis/change-by-objects/matrix_hand.csv
heat: true
digits: 2
align: { "From \\ to": left }
caption: "The possibility of each transition, written by hand: rows are the class in 2019, columns the class in 2024. Water stays water; a built-up class stays built-up or at most turns to bare soil (a demolition, 0.2); forest may become anything but water; a cerrado may become forest only rarely (0.3)."
```

These numbers are judgements, and the page says so: what matters is which cells are 0 (impossible) and which are 1. In your
study, state the matrix and why. You can also **estimate** it from data with [[objects.transition_matrix]]; the section on the settings does
that, and shows what it gives.

### Step 10: the temporal comparison

[[objects.temporal]] has three inputs: `before` (the objects of the first date, with classes and memberships), `after` (the second date, on the same grid) and
`matrix`. Its settings are `min`, the lowest adjusted membership that gives a class (default 0.1), and `both`, which also adjusts the first date from the
second. The outputs:

- `change`: a row per pair of classes, `from` and `to`, with the number of objects (of the second date), the `cells` and the `area` (map units squared);
- `objects` and `map`: the second date with its adjusted classes, and the class of each cell;
- `before`: the first date, as given or (with `both`) adjusted.

The classes are matched **by name** with the matrix. The `change` table is the one the land-change models of Terraflux read as the observed dates
([[change.transitions]] has the same form for rasters).

```tfgraph
#! caption: The whole model: the 2024 date in objects (nodes 1 to 5), the 2019 date (6 to 10), one random forest trained on the 2024 samples (11 to 14), both dates classified with it (15 and 16), the matrix (17) and the temporal comparison (18).
#! highlight: c_early, c_late, hand, change
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }
note = "9 August 2024."

[[nodes]]
id = "utm"
op = "raster.reproject"
inputs = { raster = "s2.raster" }
params = { crs = "EPSG:31983", resolution = 10, extent = [190520, 8241280, 196320, 8247080] }

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "utm.raster" }
params = { expression = "out image = s / 10000" }

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "segments.objects", image = "refl.image" }
params = { features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"], texture_bands = ["brightness"], swir = "swir16" }

[[nodes]]
id = "s2_e"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2019-08-13", query = "{\"s2:processing_baseline\": {\"eq\": \"02.13\"}}", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }
note = "13 August 2019."

[[nodes]]
id = "utm_e"
op = "raster.reproject"
inputs = { raster = "s2_e.raster" }
params = { crs = "EPSG:31983", resolution = 10, extent = [190520, 8241280, 196320, 8247080] }

[[nodes]]
id = "refl_e"
op = "raster.calc"
inputs = { s = "utm_e.raster" }
params = { expression = "out image = s / 10000" }

[[nodes]]
id = "seg_e"
op = "segment.multiresolution"
inputs = { image = "refl_e.image" }
params = { scale = 2 }

[[nodes]]
id = "feat_e"
op = "objects.features"
inputs = { objects = "seg_e.objects", image = "refl_e.image" }
params = { features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"], texture_bands = ["brightness"], swir = "swir16" }

[[nodes]]
id = "train_table"
op = "table.read"
params = { path = "training_samples.csv" }
note = "93 points of places that did not change: x, y, class."

[[nodes]]
id = "train"
op = "vector.from_table"
inputs = { table = "train_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "samples"
op = "objects.samples"
inputs = { objects = "feat.objects", samples = "train.vector" }
note = "The training objects of 2024."

[[nodes]]
id = "fit"
op = "ml.fit"
inputs = { table = "samples.table" }
params = { column = "class", columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast", "glcm_homogeneity", "sd_nir", "compactness"], n_estimators = 300 }
note = "One random forest for both dates."

[[nodes]]
id = "c_late"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "fit.model", samples = "samples.table" }
note = "2024: classes and memberships."

[[nodes]]
id = "c_early"
op = "objects.classify"
inputs = { objects = "feat_e.objects", model = "fit.model", samples = "samples.table" }
note = "2019, with the same model."

[[nodes]]
id = "hand"
op = "table.of"
params = { columns = ["class", "bare soil", "cerrado", "forest", "urban", "water"], rows = [["bare soil", 1, 1, 0.2, 1, 0], ["cerrado", 1, 1, 0.3, 1, 0], ["forest", 1, 0.5, 1, 1, 0], ["urban", 0.2, 0, 0, 1, 0], ["water", 0, 0, 0, 0, 1]] }
note = "What can become what in five years."

[[nodes]]
id = "change"
op = "objects.temporal"
inputs = { before = "c_early.objects", after = "c_late.objects", matrix = "hand.table" }
params = { min = 0.1 }
note = "The second date bound to the first: the table of what changed to what."
```

=== "Application"

    Build the two dates (the first from the template *Land cover change by objects*, or by hand), give the samples in the Viewer on
    the 2024 objects as in part 5 (**Samples**, **Apply**: it writes the samples nodes, [[ml.fit]] and [[objects.classify]]), copy the
    [[objects.classify]] node and connect the copy to the objects and attributes of the 2019 date, with the same model. Add [[table.of]] for the
    matrix (its settings `columns` and `rows`, in the Node pane) and [[objects.temporal]]. Select it and press ++ctrl+enter++; its `change`
    output is the table, and its `map` is the class of each cell in 2024.

=== "Command line"

    Save the graph as `change.toml` beside
    [training_samples.csv](/data/tutorials/object-based-image-analysis/change-by-objects/training_samples.csv) (the columns `x`, `y`, `class`) and run
    `terraflux run change.toml --threads 4 --out output`. Without a network, replace the `s2`, `utm` and `s2_e`, `utm_e` nodes by two
    [[raster.read]] of the two clips (the 2024 one in the series' downloads, the 2019 one above); the nodes `refl` and `refl_e` divide by 10,000.
    The script that made this page's figures is `change-by-objects.py`, with `obia_dates.py` for the earlier date, in `website/figures/object-based-image-analysis/`.

## Read the results

### The two maps

![The classes of the two dates](/img/tutorials/object-based-image-analysis/change-by-objects/classes.png "The random forest of part 5 on the two dates (scale 2, the same 93 samples). The lake, the town and the cerrado are in place on both; the forest and cerrado of the south and the east are not the same.")

The class areas of the two maps:

```tftable
data: /data/tutorials/object-based-image-analysis/change-by-objects/class_areas.csv
labels: { class: Class, ha_2019: "2019 (ha)", ha_2024: "2024 (ha)" }
align: { class: left }
digits: 0
caption: "The area of each class on each date, from the two classifications. The net change of the urban class is -1 ha."
```

The **net** changes are small and say little: urban 1,230 ha then 1,229 ha, water 199 then 191, forest +42, cerrado +34, and bare soil
from 142 ha to 74 ha, the one that changed most in relative terms. A city that grew would show more built-up land in 2024. Does it
mean that nothing changed? The next table says that something did, in both directions, and that the net change is a balance
of much larger movements.

### The comparison as it is

First the comparison with no help: a matrix of ones (every transition possible), which is the same as crossing the two maps. The page shows it
with the matrix that you wrote, and the other two matrices of the settings:

```tftable
data: /data/tutorials/object-based-image-analysis/change-by-objects/variants.csv
labels: { variant: "How the dates were compared", changed_ha: "Changed (ha)", changed_share: "Share of the clip", became_urban_ha: "Became urban (ha)", left_urban_ha: "Stopped being urban (ha)", none_ha: "No class (ha)" }
align: { variant: left }
digits: { changed_ha: 0, changed_share: 3, became_urban_ha: 0, left_urban_ha: 0, none_ha: 0 }
caption: "Cells whose class differs between the two dates, by how the second date was bound to the first (min 0.1 in all). The clip is 3,364 ha."
```

Crossing the two maps as they are, **23.1 % of the clip, 776 ha, changed class in five years**. That is not a land-cover change: it is the
sum of the errors of two classifications, as the idea section predicted. The matrix you wrote cuts it to 692 ha (20.6 %) and reduces the
cells that *stopped being urban* from 226 to 143 ha, the transitions it calls (almost) impossible. It does not cut the cells that became urban (239 ha against
214), because a forest becoming urban is allowed by the matrix, and it should be: it is the change you are looking for.

![The change maps](/img/tutorials/object-based-image-analysis/change-by-objects/change_legend.png "The colours of the change maps.")

```tfcompare
caption: Drag the handle. Magenta: cells that became urban. Blue: cells that stopped being urban. Amber: any other change. The map on the left is the two maps crossed as they are; the one on the right has the matrix.
before: { src: /img/tutorials/object-based-image-analysis/change-by-objects/change_naive.png, label: Compared as they are }
after: { src: /img/tutorials/object-based-image-analysis/change-by-objects/change_by_hand.png, label: With the matrix }
```

Look at where the changes are. The **amber** cells cover the cerrado and the gallery forest in the south: that is forest turning to cerrado
and back (113 ha of the first, 53 ha of cerrado to forest, in the table below), which the matrix allows, for a savanna shifts between woody and open and
the two classes are close in the attributes. A good part of it is the classifier hesitating between them, in a dry August, from one day to the other. The **blue** cells are
scattered through the town: built-up objects that on one date were taken for garden trees. **Magenta** cells are on the edge of the
town, where there are new roofs, and also inside it.

```tftable
data: /data/tutorials/object-based-image-analysis/change-by-objects/transitions_hand.csv
heat: true
digits: 0
align: { "From \\ to": left }
caption: "Hectares of each transition between 2019 (rows) and 2024 (columns), with the matrix written by hand (objects.temporal's change table). The diagonal is what did not change class."
```

Two rows deserve a second look. Urban to forest is still 99 ha although the matrix gives it 0. The matrix does not delete a transition; it cuts the
membership of the second date to what the first date allows, and the `from` of the table is the *highest* membership of the first date. An object that was
urban on 85 % of its votes and forest on 15 % can still become forest by the 15 %, if the second date is sure of forest. The matrix removes the
transitions that need a class the first date did not give any room for. And water to forest, 11 ha, is probably at the edge of the lake, where
the water level and the plants of the shore are not the same in two Augusts.

### Looking at the patches

The only way to know what a change map is worth is to look at it. The 461 patches of the map that became urban (touching cells, with the matrix) add up to 239 ha; the 24
largest, 93 ha of them, from 12.8 ha to 2.2 ha, are in this sheet, 2019 on the left of each pair and 2024 on the right, the patch outlined in yellow:

![The 24 largest patches that became urban](/img/tutorials/object-based-image-analysis/change-by-objects/patches.png "The 24 largest patches that became urban with the matrix written by hand: 40 by 40 cells (400 m), the 2019 scene and the 2024 scene, with the patch outlined. Number 1 is the largest.")

Here is my reading of them, with the eye at 10 m: **6 show new roofs or cleared ground with confidence** (numbers 3, 8, 13, 15, 16 and 23: for example, number 3 is a large
orange roof where in 2019 there was a pale field), **7 look probable** (1, 2, 4, 5, 6, 7 and 12: more bright structures in 2024) and **11 look unchanged or doubtful** (9, 10,
11, 14, 17 to 22 and 24: built-up blocks and street trees that look the same on both dates). At this resolution one cannot settle it. That is the real message: only
a quarter of the 24 largest patches are sure, in a landscape whose city was already built in 2019. A figure of change for a paper needs more than this sheet: a
reference of higher resolution or from the field, and an estimate of the area with its interval as in part 5, from a sample of the map's patches.

!!! tip "What to report"
    The classification of each date with its accuracy (part 5), the matrix and the reason for each of its cells, the net areas, the transitions that
    matter with their areas, and an estimate of the error of the change (a reference sample of the change classes, not only of the classes of each date). Do not report
    the area of the change map as the area that changed.

## Settings that change the answer

### `min`: how sure the second date must be

The slider moves `min` from 0.1 to 0.5 with the matrix written by hand. An object whose best adjusted membership is under `min` gets no class
(white in the maps), and cells without a class do not count as changed:

```tfslider
label: min (lowest adjusted membership)
caption: "objects.temporal with the matrix written by hand and min from 0.1 to 0.5. The cells with no class are white; the numbers count the clip's cells (3,364 ha)."
frames:
  - { value: 0.1, src: /img/tutorials/object-based-image-analysis/change-by-objects/min_1.png, caption: "min = 0.1 (the default)", stats: { "Changed": "20.6 %", "No class": "0.0 %", "Became urban (ha)": "239", "Stopped being urban (ha)": "143" } }
  - { value: 0.2, src: /img/tutorials/object-based-image-analysis/change-by-objects/min_2.png, caption: "min = 0.2", stats: { "Changed": "20.4 %", "No class": "0.2 %", "Became urban (ha)": "239", "Stopped being urban (ha)": "140" } }
  - { value: 0.3, src: /img/tutorials/object-based-image-analysis/change-by-objects/min_3.png, caption: "min = 0.3", stats: { "Changed": "18.4 %", "No class": "3.2 %", "Became urban (ha)": "236", "Stopped being urban (ha)": "103" } }
  - { value: 0.4, src: /img/tutorials/object-based-image-analysis/change-by-objects/min_4.png, caption: "min = 0.4", stats: { "Changed": "14.1 %", "No class": "11.5 %", "Became urban (ha)": "204", "Stopped being urban (ha)": "62" } }
  - { value: 0.5, src: /img/tutorials/object-based-image-analysis/change-by-objects/min_5.png, caption: "min = 0.5", stats: { "Changed": "7.1 %", "No class": "30.3 %", "Became urban (ha)": "90", "Stopped being urban (ha)": "32" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/change-by-objects/min.csv
  x: min
  y: [changed, no_class]
  labels: { changed: "Share changed", no_class: "Share with no class" }
  ylabel: "Share of the clip"
```

Up to 0.3 the matrix only trims the transitions (the cells that stopped being urban go from 143 to 103 ha) and the gain of urban land
stays at 236 to 239 ha. At 0.5, a third of the clip has no class, because the trees' votes are often split between two classes: the change falls to
7 % **because the map says less**, not because there is less change. A `min` that leaves out a third of the map is not a result. Raise `min` until
the implausible changes thin out (here, 0.3) and report it.

### `both`

With `both = true` the first date is adjusted from the second the same way. Here it cut the cells that stopped being urban further, from 143 to 89 ha, and raised those
that became urban, from 239 to 259 ha, with 655 ha changed in all (19.5 %). It is a more consistent pair of maps (what one date allows the other, in both
directions) and it is **more** work done by the matrix and less by the image: use it when you trust the matrix more than you trust the
classifier, and say so.

### Which matrix: written or estimated

[[objects.transition_matrix]] estimates the matrix from what was seen: for each class of the first date, the count of each transition over the count of its most frequent one, so
that the commonest transition is 1 and one never seen is 0 (or `floor`). It reads either `transitions` (a table `from`, `to`, `cells`, the form of the `change` table or of
[[change.transitions]]) or `samples` (a row per sampled place, with its class on both dates, in two text columns, `before` and `after` by default; `from_column` and `to_column` rename them).

**From the transitions found.** Feed it the table of the maps crossed as they are. The result:

```tftable
data: /data/tutorials/object-based-image-analysis/change-by-objects/matrix_from_transitions.csv
heat: true
digits: 2
align: { "From \\ to": left }
caption: "The matrix estimated by objects.transition_matrix from the change table of the two maps crossed as they are. Forest to urban is 0.15, urban to forest 0.16: the noise looks like a rule."
```

It is a mirror of the errors. Urban to forest comes out as 0.16 and forest to urban as 0.15, nearly equal, which is what noise looks like. The estimate is
only as good as the maps it comes from, and it is circular (the errors become the rule that is supposed to correct them). With it the first table gives 636 ha
changed, fewer than the hand-written matrix's 692, but nothing says it is closer to the truth.

**From samples.** The honest way is to give it places whose class you know on both dates. Here, the 93 training places (stable by assumption: the same class twice) and the 6
patches above that I confirmed (their class on the first date by the map, urban on the second):

```tftable
data: /data/tutorials/object-based-image-analysis/change-by-objects/matrix_from_samples.csv
heat: true
digits: 2
align: { "From \\ to": left }
caption: "The matrix estimated from 93 stable places and 6 confirmed changes to urban. Only the transitions seen get a value: forest to urban 0.14 (3 changes against 22 stable forest places), bare soil to urban 0.33 (3 against 9)."
```

Every transition that none of the 99 places showed is **0**: cerrado to urban, forest to cerrado and the rest are called impossible, which is wrong. That matrix gives
623 ha changed (18.5 %), and with it the page would silently delete real change. This is the lesson of the table: the possibility of a transition is a
statement about the world, and it needs more evidence than a sample of stable places and a handful of changes; `floor` (for instance 0.05) lets
an unseen transition stay possible but rare. Estimate the matrix from a *probability sample* of both dates with the change classes in it, or write it by hand and justify it.

## Common pitfalls

- **Reading the transition table as the change.** It contains the errors of both maps. Check patches, estimate the area with a reference sample, report the net areas.
- **Dates that cannot be compared.** Different seasons, sun angles, sensors or atmospheric corrections make everything look changed. Here both are in
  August (the 13th and the 9th); the 2019 scene is still brighter. Compare the medians of an unchanged class (the lake) first.
- **Different grids.** [[objects.temporal]] needs both dates on the same grid; a half-cell shift makes every edge a change. Reproject with the same
  `extent` and `resolution`, as the model does.
- **Training on a changed place.** A sample that is a field in 2019 and a roof in 2024 gives one date the wrong class. Sample places you know are stable.
- **A matrix of zeros for what you did not see.** See above: a transition not in the sample is not impossible.
- **Taking `min` high to get a clean map.** The cleanliness is classes taken away. Report the share left without a class.
- **Class names that differ between the dates.** The matrix is matched by name; a "Forest" and a "forest" are two classes.
- **Comparing the area of the change patches with an area of change.** A patch is a place where the map says "became urban"; many are wrong.

## Going further

- Before: [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/) (the classifier and the accuracy) and
  [Classes without samples](/tutorials/object-based-image-analysis/classes-without-samples/) (a model of clusters can be applied to the second date with [[ml.predict]]).
- Change as a number, without classes, and the observed dates that the land-change models read: [[change.transitions]].
- The operators of this page: [[objects.temporal]], [[objects.transition_matrix]], [[objects.classify]], [[objects.samples]], [[ml.fit]],
  [[table.of]], [[objects.to_map]].

## References

- Chen, G., Hay, G. J., Carvalho, L. M. T., & Wulder, M. A. (2012). Object-based change detection. *International Journal of Remote Sensing*,
  33(14), 4434-4457. https://doi.org/10.1080/01431161.2011.648285
- Costa, G. A. O. P., et al. (2010). Knowledge-based interpretation of remote sensing data with the InterIMAGE system: major characteristics and
  recent developments. *Proceedings of the 3rd International Conference on Geographic Object-Based Image Analysis (GEOBIA 2010)*.
- Feitosa, R. Q., Costa, G. A. O. P., Mota, G. L. A., Pakzad, K., & Costa, M. C. O. (2009). Cascade multitemporal classification based on fuzzy
  Markov chains. *ISPRS Journal of Photogrammetry and Remote Sensing*, 64(2), 159-170. https://doi.org/10.1016/j.isprsjprs.2008.09.011
- Hussain, M., Chen, D., Cheng, A., Wei, H., & Stanley, D. (2013). Change detection from remotely sensed images: from pixel-based to object-based
  approaches. *ISPRS Journal of Photogrammetry and Remote Sensing*, 80, 91-106. https://doi.org/10.1016/j.isprsjprs.2013.03.006
- Olofsson, P., Foody, G. M., Stehman, S. V., & Woodcock, C. E. (2013). Making better use of accuracy data in land change studies: estimating
  accuracy and area and quantifying uncertainty using stratified estimation. *Remote Sensing of Environment*, 129, 122-131.
  https://doi.org/10.1016/j.rse.2012.10.031
- Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for estimating area and
  assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42-57. https://doi.org/10.1016/j.rse.2014.02.015
- Tewkesbury, A. P., Comber, A. J., Tate, N. J., Lamb, A., & Fisher, P. F. (2015). A critical synthesis of remotely sensed optical image change
  detection techniques. *Remote Sensing of Environment*, 160, 1-14. https://doi.org/10.1016/j.rse.2015.01.006
