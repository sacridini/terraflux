---
title: "Case study: land use of a protected area from Sentinel-2"
short: "Case: protected area (Sentinel-2)"
description: "From the official boundary and two seasons of Sentinel-2 to a land-use map of a Brazilian ecological station, with a random forest on objects, an accuracy assessment against a stratified reference, adjusted areas, and the methods paragraph Terraflux drafts for a paper."
order: 150
level: advanced
time: 120 min
ops: [data.stac, vector.read, vector.rasterize, raster.calc, segment.multiresolution, objects.features, objects.samples, ml.fit, objects.classify, objects.to_map, objects.sample, objects.accuracy]
series: Object-based image analysis
part: 15
---

A manager of a protected area wants three numbers in a report: how much of the area is still native vegetation, how much of
it is forest and how much open savanna, and how much has been taken by roads and buildings. A scientist wants the same numbers
with a statement of how much to trust them. In this tutorial you will produce both for a real protected area, the **Estação
Ecológica do Jardim Botânico de Brasília** (4,502 ha), next to the Lago Paranoá of the series' other pages. You will cut it out of
two seasons of Sentinel-2 with its **official boundary**, segment it, classify the objects with a random forest trained on points
you label, assess the map against a reference you draw and read, correct the areas for the map's errors, and have Terraflux draft the
methods paragraph for a paper. At the end you will have a map as a GeoTIFF, a table of accuracies and areas, and a clear idea of which
of the map's numbers are sound and which are not. Some are not: the page says which, and why.

## The idea

A land-use or land-cover map in a paper is not a picture. It is **a classification, a sample and a statement of error**: the
classes and how they were told apart, a reference sample drawn so that the error can be estimated, and the map's accuracy and
class areas with confidence intervals. Olofsson et al. (2014) set out this practice, and it is what reviewers look for. The work
has four parts, each a node or two of one model:

1. **The data**: the boundary of the area (a polygon, from its official source) and the imagery (here Sentinel-2, two seasons, read
   by [[data.stac]]).
2. **The objects and their attributes**: a segmentation inside the boundary and the attributes of each object, as in
   [part 4](/tutorials/object-based-image-analysis/describing-objects/).
3. **The classification**: a random forest (Breiman, 2001) learned from training points and applied to every object, as in
   [part 5](/tutorials/object-based-image-analysis/classifying-by-samples/).
4. **The assessment**: a stratified random sample of the mapped objects, labelled with the true class, and
   [[objects.accuracy]]: the error matrix, the user's and producer's accuracies, the overall accuracy and the class areas corrected for the errors
   (Olofsson et al., 2014; Stehman and Foody, 2019).

![The protected area](/img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/area.png "The Estação Ecológica do Jardim Botânico de Brasília (yellow: the official boundary of the national registry), in Sentinel-2 true colour of the dry season, on the grid of this page (7.9 by 9.9 km). A notch of the boundary leaves out the built-up quarter at the top left; the road that runs along the east side is inside it.")

### Why this area

The station lies on the plateau of the Distrito Federal, in the **Cerrado**, the Brazilian savanna: mostly open grassland and
shrubby savanna on the interfluves, with gallery forests along the streams. The Cerrado is a global biodiversity hotspot and has lost
much of its native vegetation to farming and cities (Ratter et al., 1997; Klink and Machado, 2005), so a protected area
within a growing capital is a good place to ask what a boundary on a map protects. The station overlaps the south of the clip of Lago Paranoá used from
[part 2](/tutorials/object-based-image-analysis/what-is-an-object/) on. The Parque Nacional de Brasília, 42,000 ha, is the better known area of the region; at
10 m it is 4 million cells and a subject for [large images](/tutorials/object-based-image-analysis/large-images/). The station
is the right size for a tutorial.

## When to use it

- You need a **map and numbers for a paper or a management plan**, with an accuracy you can defend.
- You have **training examples** (points you can label from imagery, field visits or another map) and a way to draw a
  **separate** reference.
- The classes differ in the **season** as well as in colour: here, an open savanna that is brown in the dry season and green in
  the wet, against forest that is green in both.

It is not for **change** (two dates of the same area: [part 11](/tutorials/object-based-image-analysis/change-by-objects/)), and it
is not the right tool for **forest types** that Sentinel-2's 10 m bands cannot tell apart (the savanna's own physiognomies, from open
grassland to dense woodland, are a continuum, and this page maps them as one class). For a map that uses rules, with shape and context
at 0.6 m, see [part 14](/tutorials/object-based-image-analysis/case-urban-cover-naip/).

## What you need

- **The boundary of the area.** The official polygons of the Brazilian national registry of protected areas (the **Cadastro Nacional de Unidades
  de Conservação**, CNUC), published by the Ministry of the Environment on its open-data portal (dados.mma.gov.br) under
  **Creative Commons Attribution** (CC BY). This page uses the 2025_08 version of the shapefile, the feature with `uc_id` 1654
  (the registry gives 4,502 ha; the polygon measures 4,509 ha on the UTM grid). The one-feature layer is a download:
  [esec_jardim_botanico.gpkg](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/esec_jardim_botanico.gpkg)
  (98 KB; attribution in [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/ATTRIBUTION.txt)). If the
  official boundary of *your* area has no clear licence, a box drawn on the map is a fair substitute: say that it is a box.
- **Sentinel-2 L2A** (10 m; blue, green, red, near infrared, two short-wave infrared bands), from Earth Search. Contains modified Copernicus
  Sentinel data. The Cerrado has a dry season (May to September) and a wet one (October to April), and the savanna
  changes between them; a **dry-season composite** (15 July to 15 September 2024) and a **wet-season** one (February and March 2024)
  give the classifier both pictures. [[data.stac]] combines the scenes of an interval cell by cell with the **median of the clear cells**
  (`composite = "median"` is the default), and `mask = "auto"` drops the clouds, shadows and cirrus of every scene with its scene
  classification before they are combined.
- **A grid in metres.** `crs = "EPSG:31983"` (SIRGAS 2000 / UTM zone 23S), with `extent` and `cellsize` in metres: 790 by 990 cells of 10 m.

![The two seasons](/img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/seasons.png "Inside the boundary: the dry-season composite in true colour and in near-infrared false colour (vegetation red), and the wet-season NDVI. The gallery forests are dark green in the dry season and bright in the NDVI of the wet season; the savanna is brown in the dry season and a mid green in the wet; the road along the east side and the quarter at the top are bare and bright.")

The first run reads about two months of scenes twice, and with two threads it took about ten minutes; Terraflux keeps what it
reads in its cache, so the next runs take seconds. For readers without a network, the two composites are downloads
([esec_dry.tif](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/esec_dry.tif) 6.1 MB and
[esec_wet.tif](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/esec_wet.tif) 2.2 MB, reflectance times 10,000, Int16).

## Build the model

### Step 1: the two composites and the boundary

Two [[data.stac]] nodes read the two seasons. The dry one asks for six bands (`assets`), the wet one for red and near infrared only, as only
its NDVI will be used. [[vector.read]] reads the boundary, and [[vector.rasterize]] burns it into the grid of the dry composite
(`like`) as a map of 1 inside and 0 outside (`value = 1`, `fill = 0`).

```tfgraph
#! caption: Two seasons of Sentinel-2 on a UTM grid (nodes 1 and 2), the official boundary (3) and its mask on the grid (4).
#! highlight: dry, wet, area, inside
#! numbered: true
#! fragment: true
[[nodes]]
id = "dry"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16", "swir22"], datetime = "2024-07-15/2024-09-15", mask = "auto", crs = "EPSG:31983", extent = [190300, 8234900, 198200, 8244800], cellsize = 10 }
note = "The dry season, 15 July to 15 September 2024: the median of the clear cells."

[[nodes]]
id = "wet"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["red", "nir"], datetime = "2024-02-01/2024-03-31", mask = "auto", crs = "EPSG:31983", extent = [190300, 8234900, 198200, 8244800], cellsize = 10 }
note = "The wet season, February and March 2024."

[[nodes]]
id = "area"
op = "vector.read"
params = { path = "esec_jb.gpkg" }
note = "The official boundary (CNUC)."

[[nodes]]
id = "inside"
op = "vector.rasterize"
inputs = { vector = "area.vector", like = "dry.raster" }
params = { value = 1, fill = 0 }
note = "1 inside the boundary, 0 outside."
```

The boundary is in the registry's coordinate system (SIRGAS 2000, geographic); [[vector.rasterize]] reprojects the features to the map's
system first. A cell is inside when its centre is.

!!! app "In the application"
    Add the two [[data.stac]] nodes and set their **assets**, **datetime**, **mask**, **crs**, **extent** and **cell size** in the Node pane;
    add [[vector.read]] with the path of the boundary file and [[vector.rasterize]]: drag the
    `dry` output to its **like** input. Press ++ctrl+enter++ on `inside`: the mask appears in the Viewer.

### Step 2: one image for the segmenter

[[raster.calc]] makes the image the segmenter reads from the three inputs `d` (dry), `w` (wet) and `m` (the mask). Four lines:

```text
image = d / 10000
image.ndvi_wet = (w.nir - w.red) / (w.nir + w.red)
image[m != 1] = None
out image
```

The first divides the six dry bands by 10,000 (reflectance 0 to 1), the second **adds a layer**, the NDVI of the wet season,
the third removes everything outside the boundary (`None` is a missing value: no object will be made there), and `out image` gives
the stack. The image has seven layers: `blue`, `green`, `red`, `nir`, `swir16`, `swir22` and `ndvi_wet`. Everything after reads those names.

### Step 3: the objects and their attributes

[[segment.multiresolution]] with `scale = 1.5` cuts the 450,929 cells inside the boundary into **5,861 objects** (0.77 ha on average). The scale is in
the image's units; on reflectance 0 to 1 with a layer of NDVI in it, 1.5 is a good middle between objects the size of a track and objects the
size of a hillside. [[objects.features]] gives each object the mean and the standard deviation of every layer, the NDVI, NDWI and NDMI of the dry
season, and its compactness:

```tfgraph
#! caption: The objects (node 1) and their attributes (node 2), from the seven-layer image (node 0, in the full model).
#! highlight: seg, feat
#! numbered: true
#! fragment: true
[[nodes]]
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 1.5 }
note = "The objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image" }
params = { features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "compactness"], swir = "swir16" }
note = "Means, spread, indices and compactness."
```

`swir = "swir16"` tells [[objects.features]] which layer is the short-wave infrared the NDMI uses; the other bands are found by their names.

![Attributes of the objects](/img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/attributes.png "Four attributes over a detail of 2.2 by 2.2 km that holds all four classes. The dry-season NDVI and the wet-season NDVI do not show the same things: the forest is high in both, the savanna low in the dry and mid in the wet. The NDMI and the short-wave infrared tell bare ground and roads from plants.")

![The objects](/img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/objects.png "The 5,861 objects over the same detail, in false colour. A forest patch is a few objects, the savanna large ones, a road a string of small ones.")

### Step 4: training points

A random forest needs **examples**: objects of which you know the class. They came from the author's eyes, as in
[part 5](/tutorials/object-based-image-analysis/classifying-by-samples/) but with a design that guards against the usual flaw of a hand-picked
sample. On a landscape that is 82 % one class, picking points at random would give a hundred savanna points and three forest points.
So 150 points were drawn at random in **six groups of a k-means of the pixels** (25 in each group), so that the rare things are in the
sample: the groups are a way to spread the points, not classes. Each point was shown as a **chip**, 330 m of the image around it in
true colour and false colour, and labelled with what is under the cross, without the map: `cerrado` (savanna, open or shrubby: brown in the dry season),
`forest` (dark green, bright red in false colour: gallery forest and woodland), `built-up` (roofs and the paved road), `bare soil` (the orange of
bare ground and the road shoulders). The reading is the author's, from 10 m cells in which a road is a single cell wide: not field data, and
sometimes a guess.

![The training points](/img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/training.png "The 150 training points over the false-colour image, coloured by the class read from the chips: 71 cerrado, 41 built-up, 26 forest and 12 bare soil. Many fall along the east edge, where the road and the built-up fringe are.")

The points go into the model as a table of `x`, `y` and `class` (**[training_points.csv](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/training_points.csv)**),
read by [[table.read]] and turned into points by [[vector.from_table]]. [[objects.samples]] picks the **objects under the points** (139 objects: 11 of
bare soil, 33 built-up, 69 cerrado and 26 forest; a point gives the object of its cell, and an object picked with two different classes is left out), with their attributes.
[[ml.fit]] learns a forest of 300 trees from that table, using thirteen attributes (the band means, the wet-season NDVI, the NDVI, NDWI and NDMI, the spread of the near infrared and of the wet-season
NDVI, and the compactness); [[objects.classify]] applies it to every object, giving the probability of each class and the **margin**, the largest minus the second largest.

```tfgraph
#! caption: Training points read from a table (nodes 1 and 2), the training objects (3), a random forest (4) and the classification of every object (5).
#! highlight: train_table, samples, fit, rf
#! numbered: true
#! fragment: true
[[nodes]]
id = "train_table"
op = "table.read"
params = { path = "training_points.csv" }
note = "The training points: x, y and class."

[[nodes]]
id = "train"
op = "vector.from_table"
inputs = { table = "train_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "samples"
op = "objects.samples"
inputs = { objects = "feat.objects", samples = "train.vector" }
note = "The objects under the points, with their attributes."

[[nodes]]
id = "fit"
op = "ml.fit"
inputs = { table = "samples.table" }
params = { column = "class", columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "mean_swir22", "mean_ndvi_wet", "ndvi", "ndwi", "ndmi", "sd_nir", "sd_ndvi_wet", "compactness"], n_estimators = 300 }
note = "A random forest."

[[nodes]]
id = "rf"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "fit.model", samples = "samples.table" }
note = "Class and probabilities of every object."
```

```tfchart
type: bar
horizontal: true
data: /data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/importance.csv
x: attribute
y: [importance]
labels: { importance: "Importance" }
colors: { importance: "#3b8a5a" }
xlabel: "Mean decrease of impurity (the forest's importance)"
caption: "What the forest used. The spread of the near infrared inside an object, the dry-season NDVI and the visible bands carry most; the wet-season NDVI, which looked so telling in the figure above, ranks tenth of thirteen."
```

The importances say something a manager would want to know: the forest did not need the wet season much. An object of road or building is not
a green thing in either season, and the savanna and the forest differ in the dry-season near infrared and its spread inside the object. The wet-season layer may still have helped the segmentation, but the classifier hardly used it. Never assume that more data helps: look.

### Step 5: the map

[[objects.to_map]] with `what = "class"` makes the land-use map, which [[raster.write]] writes as a GeoTIFF
([land_use.tif](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/land_use.tif): 1 bare soil, 2 built-up, 3 cerrado, 4 forest;
`objects.samples` numbers the classes in alphabetical order).

![The land-use map](/img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/map.png "The land-use map of the station: cerrado (tan) over four fifths of it, the gallery forests (green) along the streams in the north and a forest patch near the south-east, and the road corridor and the built-up fringe on the east side and in the north-east. The square is the detail of the other figures.")

```tfcompare
caption: Drag the handle to compare the dry-season false-colour image (vegetation red) with the map (same size).
before: { src: /img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/image_plain.png, label: Sentinel-2 }
after: { src: /img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/map_plain.png, label: Map }
```

The map is plausible, and plausibility is not accuracy. Step 6 measures it.

### Step 6: the reference and the assessment

[[objects.sample]] draws the reference: a **stratified random sample of the mapped objects**, the strata being the map's classes, every object
of a class having the same chance. With `per_class = 50` and `seed = 1`, it gives 50 objects of each of the four classes (200 in all), each with a point at the cell nearest
its centroid that lies in the object and a column `reference`, empty. Olofsson et al. (2014) advise at least 50 per class, and more for the
rare classes that matter most.

```tfgraph
#! caption: Draw the reference sample and write the points, to be read.
#! highlight: sample, points_file
#! numbered: true
#! fragment: true
[[nodes]]
id = "sample"
op = "objects.sample"
inputs = { objects = "rf.objects" }
params = { per_class = 50, seed = 1 }
note = "50 objects of each class, at random."

[[nodes]]
id = "points_file"
op = "vector.write"
inputs = { vector = "sample.points" }
params = { path = "sample_points.gpkg" }
```

**How the reference was made.** There is no field data. The 200 points were labelled by the author, **by eye, from the image**: shuffled (a point's number does
not tell its class), shown as the same chips, without the map, and read with the same four classes. What this reference is not: it is not field data; it is not
independent of the image the map came from; and one person made it, with the hesitations a 10 m cell invites (a track in the savanna: `cerrado` or
`bare soil`? A road shoulder: `bare soil` or `built-up`?). Of the 200 points the reader called 63 `cerrado`, 66 `bare soil`, 37 `forest` and 34 `built-up`,
a very different mix from the map's, which is the point of stratifying by the map's classes. The file is
[reference_points.csv](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/reference_points.csv) (`x`, `y`, `class`): change a label you disagree with and run again.

The model reads it with [[table.read]] and [[vector.from_table]] and gives it to [[objects.accuracy]] as `samples`:

```tfgraph
#! caption: The map written (nodes 1 and 2), the reference read (3 and 4), and the accuracy of the map against it (5).
#! highlight: map, ref_table, ref, acc
#! numbered: true
#! fragment: true
[[nodes]]
id = "map"
op = "objects.to_map"
inputs = { objects = "rf.objects" }
params = { what = "class" }

[[nodes]]
id = "map_file"
op = "raster.write"
inputs = { value = "map.map" }
params = { path = "land_use.tif" }
note = "The land-use map, a GeoTIFF."

[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_points.csv" }
note = "The reference: x, y and the class read from the image."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "acc"
op = "objects.accuracy"
inputs = { objects = "rf.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "Error matrix, accuracies and adjusted areas."
```

### The whole model

```tfgraph
#! caption: The whole model: the two seasons, the boundary and its mask (1 to 4), one image (5), objects and attributes (6, 7), training (8 to 12), the map (13, 14) and the assessment (15 to 17).
#! highlight: dry, wet, refl, fit, acc
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dry"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16", "swir22"], datetime = "2024-07-15/2024-09-15", mask = "auto", crs = "EPSG:31983", extent = [190300, 8234900, 198200, 8244800], cellsize = 10 }
note = "The dry season, 15 July to 15 September 2024: the median of the clear cells."

[[nodes]]
id = "wet"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["red", "nir"], datetime = "2024-02-01/2024-03-31", mask = "auto", crs = "EPSG:31983", extent = [190300, 8234900, 198200, 8244800], cellsize = 10 }
note = "The wet season, February and March 2024."

[[nodes]]
id = "area"
op = "vector.read"
params = { path = "esec_jb.gpkg" }
note = "The official boundary (CNUC)."

[[nodes]]
id = "inside"
op = "vector.rasterize"
inputs = { vector = "area.vector", like = "dry.raster" }
params = { value = 1, fill = 0 }
note = "1 inside the boundary, 0 outside."

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { d = "dry.raster", w = "wet.raster", m = "inside.map" }
params = { expression = "image = d / 10000\nimage.ndvi_wet = (w.nir - w.red) / (w.nir + w.red)\nimage[m != 1] = None\nout image" }
note = "Reflectance 0 to 1, the wet-season NDVI as a layer, nothing outside the boundary."

[[nodes]]
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 1.5 }
note = "The objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image" }
params = { features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "compactness"], swir = "swir16" }
note = "Means, spread, indices and compactness."

[[nodes]]
id = "train_table"
op = "table.read"
params = { path = "training_points.csv" }
note = "The training points: x, y and class."

[[nodes]]
id = "train"
op = "vector.from_table"
inputs = { table = "train_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "samples"
op = "objects.samples"
inputs = { objects = "feat.objects", samples = "train.vector" }
note = "The objects under the points, with their attributes."

[[nodes]]
id = "fit"
op = "ml.fit"
inputs = { table = "samples.table" }
params = { column = "class", columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "mean_swir22", "mean_ndvi_wet", "ndvi", "ndwi", "ndmi", "sd_nir", "sd_ndvi_wet", "compactness"], n_estimators = 300 }
note = "A random forest."

[[nodes]]
id = "rf"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "fit.model", samples = "samples.table" }
note = "Class and probabilities of every object."

[[nodes]]
id = "map"
op = "objects.to_map"
inputs = { objects = "rf.objects" }
params = { what = "class" }

[[nodes]]
id = "map_file"
op = "raster.write"
inputs = { value = "map.map" }
params = { path = "land_use.tif" }
note = "The land-use map, a GeoTIFF."

[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_points.csv" }
note = "The reference: x, y and the class read from the image."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "acc"
op = "objects.accuracy"
inputs = { objects = "rf.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "Error matrix, accuracies and adjusted areas."
```

=== "Application"

    Build the nodes in order and press ++ctrl+enter++ on a node to run to it. The training can be done in the Viewer instead of from a file:
    tick **Samples** on the layer of objects of `feat`, add the four classes, click objects (part 5), and **Apply** writes
    the nodes `train_table` to `rf` for you. Select `acc` and run: the error matrix, the accuracy table, the overall accuracy and
    the list of the reference points the map got wrong are its outputs.

=== "Command line"

    Download [model.toml](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/model.toml), the boundary
    ([esec_jardim_botanico.gpkg](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/esec_jardim_botanico.gpkg), saved
    as `esec_jb.gpkg`), [training_points.csv](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/training_points.csv)
    and [reference_points.csv](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/reference_points.csv) beside it:

    ```bash
    terraflux run model.toml --threads 4 --out output
    ```

    To write the accuracy tables add `[outputs.accuracy]` with `from = "acc.table"`, and likewise for `acc.matrix` and `acc.overall`.

=== "Without a network"

    Download [esec_dry.tif](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/esec_dry.tif) and
    [esec_wet.tif](/data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/esec_wet.tif) and replace the nodes `dry` and `wet`
    with [[raster.read]] nodes of the same names (the layers are named as the assets):

    ```toml
    [[nodes]]
    id = "dry"
    op = "raster.read"
    params = { path = "esec_dry.tif" }

    [[nodes]]
    id = "wet"
    op = "raster.read"
    params = { path = "esec_wet.tif" }
    ```

## Read the results

### The numbers

```tftable
data: /data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/matrix.csv
columns: [map, "bare soil", "built-up", cerrado, forest, total]
labels: { map: "Map \\ reference", total: Sampled }
align: { map: left }
digits: 0
heat: true
caption: "The 200 reference objects by the class the map gave (rows) and the class read from the image (columns). 155 are on the diagonal."
```

By objects, the map and the reading agree at **155 of 200 points (77.5 %)**, kappa 0.70. Weighted by the area of the map's classes, the overall accuracy is **94.4 % ± 1.9 %**
(95 % confidence). The two numbers are far apart and both are right: the savanna is 82 % of the area and the map gets it almost always right, so a number
weighted by area is high; a number that counts each class's objects alike is lower, because the small classes are the hard ones. **Report both** and say which is which.

The weighting depends on how the sample was drawn, and the model says so with the setting **`design`** of [[objects.accuracy]]. [[objects.sample]] gives **every object
of a class the same chance**, large or small, so the page uses `design = "objects"`: each sampled object is weighted by its **area** (the ratio estimator of
Stehman, 2014, and Radoux and Bogaert, 2017), so that a 20-ha patch of savanna weighs more than a 0.1-ha patch of road. If the reference were points thrown at random over the map,
where the chance of a point is proportional to area, you would set `design = "area"` and each point would count once. Leave the setting out and Terraflux guesses from your
points, and warns that it guessed: say which design you used.

```tftable
data: /data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/accuracy.csv
columns: [class, users, users_ci, producers, producers_ci, map_ha, area_ha, area_ha_ci]
align: { class: left }
labels: { class: Class, mapped_objects: "Sampled objects", reference_objects: "Read as this class", users: "User's accuracy", users_ci: "± 95 %", producers: "Producer's accuracy", producers_ci: "± 95 %", map_ha: "Mapped area (ha)", area_ha: "Adjusted area (ha)", area_ha_ci: "± 95 % (ha)" }
digits: { mapped_objects: 0, reference_objects: 0, users: 2, users_ci: 2, producers: 2, producers_ci: 2, map_ha: 1, area_ha: 1, area_ha_ci: 1 }
caption: "User's and producer's accuracies, weighted by area, and areas by class (Olofsson et al., 2014; design = objects). The intervals are the sampling error only; the reference is the author's reading of the same image."
```

- **Cerrado**: user's 0.99, producer's 0.97. The savanna is mapped almost perfectly, and nothing else is said of it: the station is a savanna. 3,780 ± 78 ha estimated, 3,702 mapped.
- **Forest**: user's 0.77 (by objects 0.70: 11 of the 50 sampled forest objects are savanna in the reading, as the dense shrubby savanna next to the gallery forest has a high NDVI too), a producer's accuracy
  of 1.00 (every forest the reader found is in a forest-mapped object, which says only that no forest hid in the 50 savanna objects sampled). The map
  has 615.9 ha of forest; the estimate is **475 ± 70 ha**: the map over-counts the forest by about a third.
- **Built-up**: user's 0.54 (by objects 0.58). Of 50 sampled built-up objects, 19 are `bare soil` in the reading: the road corridor is mapped as built-up where it is bare ground or the orange shoulder of the road.
  The classes touch and the 10 m cell mixes them. 157 ha mapped, 105 ± 35 ha estimated.
- **Bare soil**: user's 0.91 (when the map says bare soil it is) but a producer's accuracy of 0.21, and an area of **149 ± 54 ha against 34 ha mapped**. See the warning.

!!! warning "A point on a track labels a whole object"
    149 ha of bare soil in a station where the map finds 34 ha is hard to believe: tracks 3 to 5 m wide cannot cover that much of a savanna. The estimator is not the cause (with the
    `objects` design it weights by area, as it should; before that setting existed this page's estimate was 262 ± 207 ha, and it fell to 149 ± 54). The cause is the **label**. Two of the 50 sampled
    savanna objects had their point on a track, which the reader called `bare soil`; each of those objects is large (hectares), and the estimator takes the point's label for the whole object,
    so 2 of 50 objects, weighted by area, become 4 % of the savanna's 3,702 ha. A point reads what is under the point; an area estimate needs the label of the **area**. With a rare class that lives in thin
    features (tracks, road shoulders) at 10 m, label the object by what covers most of it, or draw the reference as **cells** of a raster map ([[accuracy.assess]]) and read the cell, not
    the object. Until then, quote the user's accuracy (0.91) and the mapped area of the bare soil (34 ha), and not its estimated area.

![The reference points that the map got wrong](/img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/errors.png "Twelve of the 45 reference points that the map got wrong, in true colour (330 m chips): the map's class and the class read. Forest against shrubby savanna, road shoulders (orange) against built-up, and tracks. The reader's label is itself a judgement.")

The forest/savanna confusion is the one an ecologist cares about, and it is partly a matter of definition: a **cerradão** (a woodland of the Cerrado) and a gallery forest are
both forest to a manager, and a dense shrubby savanna is not. At 10 m the map cannot settle it, and neither can a reader looking at the same 10 m.

### How sure is the map

[[objects.classify]] gives each object the probability of every class, and the **margin**, the largest minus the second largest. A margin near 0 is an object the forest could not decide:

![The margin of the classification](/img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/margin.png "The margin of each object (light: sure; dark: unsure). The doubtful objects are along the road corridor and the tracks, in the cluster of buildings at the north-east and at the edges of the gallery forests, where the reference found most of the errors.")

683 objects have a margin under 0.3, **7.1 % of the area**. They are where a field visit or a second look would pay off most, and where a manager should treat the class as a probability.

### What you can report, and the methods paragraph

What the data support: **savanna and forest in the area, the road corridor and the built-up fringe, with the stated accuracies**. They do not support the area of bare soil (see the warning). A report can say:
the station is about 82 % savanna (3,780 ± 78 ha estimated, user's accuracy 0.99), 10 % to 14 % forest (475 ± 70 ha estimated, 616 ha mapped), and about 4 % built-up and bare ground along
its east side, which is the highway (191 ha mapped).

```tftable
data: /data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/map_areas.csv
align: { class: left }
labels: { class: Class, hectares: "Mapped area (ha)", share: "Share of the station (%)" }
digits: { hectares: 1, share: 1 }
caption: "Area of each class counted on the map, inside the boundary (4,509 ha)."
```

Terraflux can draft the methods section from the model. `terraflux methods` reads the model file and writes the methods it uses, with the papers to cite, the model step by
step and the references; `-f bibtex` or `-f ris` gives the references for your reference manager:

```bash
terraflux methods model.toml > methods.txt
terraflux methods model.toml -f bibtex -o refs.bib
```

??? note "What it wrote for this model"
    ```text
    Methods

    The analysis was run in Terraflux 1.58.0 (Lacerda, 2026), as a model of 17 steps (page_model.toml); the model's file, with every setting, is given with this paper. We computed multiresolution segmentation into objects (Baatz & Schäpe, 2000), random forest learning (Breiman, 2001) and the accuracy of a classification of objects and the sample to assess it (Olofsson et al., 2014; Radoux & Bogaert, 2017; Stehman, 2014) with Terraflux.

    The model, step by step

    1. dry (data.stac): The scenes of sentinel-2-l2a from a STAC catalogue (blue, green, red, nir, swir16, swir22) of 2024-07-15/2024-09-15. The dry season, 15 July to 15 September 2024: the median of the clear cells.
    2. wet (data.stac): The scenes of sentinel-2-l2a from a STAC catalogue (red, nir) of 2024-02-01/2024-03-31. The wet season, February and March 2024.
    3. area (vector.read): Reads the features of esec_jb.gpkg. The official boundary (CNUC).
    4. train_table (table.read): Reads the table training_points.csv. The training points: x, y and class.
    5. ref_table (table.read): Reads the table reference_points.csv. The reference: x, y and the class read from the image.
    6. inside (vector.rasterize): The features of area burnt into the grid of dry, the value 1. 1 inside the boundary, 0 outside.
    7. train (vector.from_table): Points from the columns x and y of train_table in EPSG:31983.
    8. ref (vector.from_table): Points from the columns x and y of ref_table in EPSG:31983.
    9. refl (raster.calc): Maps computed cell by cell from its inputs by a script. Reflectance 0 to 1, the wet-season NDVI as a layer, nothing outside the boundary.
    10. seg (segment.multiresolution): Refl merged into objects by multiresolution segmentation at scale 1.5. The objects.
    11. feat (objects.features): The attributes mean, sd, ndvi, ndwi, ndmi, compactness of the objects of seg, from refl. Means, spread, indices and compactness.
    12. samples (objects.samples): The objects under the samples of train (the class in class), as a training table. The objects under the points, with their attributes.
    13. fit (ml.fit): A random forest or gradient boosting fitted to the records of samples. A random forest.
    14. rf (objects.classify): The objects of feat classed by fit. Class and probabilities of every object.
    15. map (objects.to_map): The objects of rf as a map of their class.
    16. acc (objects.accuracy): The accuracy of a classification of objects against objects of known class: Design objects. Error matrix, accuracies and adjusted areas.
    17. map_file (raster.write): Writes map to land_use.tif. The land-use map, a GeoTIFF.

    References

    Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality multi-scale image segmentation. In J. Strobl, T. Blaschke & G. Griesebner (Eds.), Angewandte Geographische Informationsverarbeitung XII (pp. 12–23). Wichmann.
    Breiman, L. (2001). Random forests. Machine Learning, 45(1), 5–32. https://doi.org/10.1023/A:1010933404324
    Lacerda, E. (2026). Terraflux: landscape ecology and land change modelling from a laptop to a cluster (Version 1.58.0) [Computer software]. https://github.com/sacridini/terraflux
    Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for estimating area and assessing accuracy of land change. Remote Sensing of Environment, 148, 42–57. https://doi.org/10.1016/j.rse.2014.02.015
    Radoux, J., & Bogaert, P. (2017). Good practices for object-based accuracy assessment. Remote Sensing, 9(7), 646. https://doi.org/10.3390/rs9070646
    Stehman, S. V. (2014). Estimating area and map accuracy for stratified random sampling when the strata are different from the map classes. International Journal of Remote Sensing, 35(13), 4923–4939. https://doi.org/10.1080/01431161.2014.930207
    ```

It is a draft, and a good one for the part that is easy to forget: **every setting and every node**. It does not know what only you know. Add the data (scene, dates, mask), the boundary's source and licence,
how the training and reference points were made, and the numbers. It cites the segmentation, the random forest and the accuracy design, but not the data (Sentinel-2) or the boundary; those you add:

> The land use of the Estação Ecológica do Jardim Botânico de Brasília (4,509 ha within the official boundary of the national registry of protected areas,
> CNUC 2025_08) was mapped from Sentinel-2 L2A imagery (Drusch et al., 2012): the median of the clear observations (scene classification mask) of
> 15 July to 15 September 2024 (blue, green, red, near infrared, two short-wave infrared bands) and of February and March 2024 (NDVI), at 10 m on the UTM 23S grid.
> The area was segmented by multiresolution segmentation (Baatz and Schäpe, 2000; scale 1.5 on reflectance 0 to 1; 5,861 objects). A random forest (Breiman, 2001; 300 trees; 13 attributes)
> was trained on 139 objects under 150 points labelled by one analyst from the image, and classified every object into cerrado, forest, built-up and bare soil. The map was assessed
> (Olofsson et al., 2014) with 200 objects drawn at random in strata of the map's classes (50 per class) and labelled by the same analyst blind to the map: overall accuracy 77.5 % by
> objects (kappa 0.70), 94.4 % ± 1.9 % weighted by area (objects design: the ratio estimator of Stehman, 2014); user's accuracy, weighted by area, 0.99 for cerrado, 0.77 for forest, 0.54 for built-up and 0.91 for bare soil.
> The estimated area of forest was 475 ± 70 ha. The reference was not independent of the imagery and is not field data, and the estimated area of bare soil (149 ± 54 ha against 34 ha mapped) is not
> reported, because points on tracks label whole objects.
> Processing was done with Terraflux 1.58.0 (Lacerda, 2026).

## Settings that change the answer

**The scale of the objects.** Everything above is for `scale = 1.5`. The slider reruns the whole model at another scale, trains on the same 150 points and assesses against the same 200.

```tfslider
label: Scale of the objects
caption: "The classification of the detail for four scales. The chart is the overall accuracy weighted by area, with its 95 % interval, against the same reference points."
frames:
  - { value: 1, src: /img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/scale_1.png, caption: "Scale 1", stats: { Accuracy: "92.2 %", "By objects": "78.4 %", Kappa: "0.71" } }
  - { value: 1.5, src: /img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/scale_1_5.png, caption: "Scale 1.5 (the page's)", stats: { Accuracy: "94.4 %", "By objects": "77.5 %", Kappa: "0.70" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/scale_2.png, caption: "Scale 2", stats: { Accuracy: "90.9 %", "By objects": "79.8 %", Kappa: "0.73" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/case-protected-area-sentinel2/scale_3.png, caption: "Scale 3", stats: { Accuracy: "90.8 %", "By objects": "81.6 %", Kappa: "0.75" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/case-protected-area-sentinel2/scales.csv
  x: scale
  y: [overall_adjusted]
  labels: { overall_adjusted: "Overall accuracy by area (%)" }
  colors: { overall_adjusted: "#3b8a5a" }
  ylabel: "Overall accuracy by area (%)"
```

- **The two accuracies do not move together.** The accuracy weighted by area is highest at 1.5 (94 %) and falls to 91 % at scales 2 and 3; the accuracy by objects rises from 77.5 % to 82 % and kappa from 0.70 to 0.75. Bigger
  objects are purer about the savanna and blur the small classes; the accuracy by objects, which has fewer objects of the hard classes to count, rises. The intervals (±2 to ±7 points) cover part of the
  differences. No scale is "right"; choose it for the smallest class you must map, here a road 1 cell wide.
- **Do not choose by this sweep.** It reuses the reference. A scale picked from the best of four values is tuned to those 200 points. Choose with the training points and keep the reference for one final assessment.
- **What moves the result more than the scale** is the sample (a different 150 points would give a different forest) and the classes' definition, which no setting can change.

## Common pitfalls

- **Reporting one accuracy.** 94 % and 77.5 % are the same map. Say which weighs what, and give the class accuracies.
- **A point label taken for an object's label.** The 149 ha of bare soil. See the warning in "Read the results"; label the object by what covers most of it, or sample cells.
- **The wrong `design`.** Objects drawn with the same chance (`objects.sample`) are `design = "objects"`; points thrown at random over the map are `"area"`. The default guesses and warns.
- **A reference that is not independent.** Labels read from the image that made the map share its blind spots, and the same person read the training points. It is better than nothing and much weaker than field data; say so.
- **Using the reference to choose.** Settings picked because they raised the accuracy on the reference give an accuracy that is not an accuracy. Keep a separate tuning sample.
- **A boundary taken for the truth of the ground.** The polygon of the registry follows a legal line, and here includes a stretch of highway: the map shows what is *inside the line*, not what the station protects.
- **Tracks and roads at 10 m.** A track is a fraction of a cell. A point on it can be labelled `bare soil` for an object that is savanna. Decide the rule before you label (a track is `cerrado`, a road is `built-up`) and keep to it.
- **A CRS in degrees.** `data.stac` without a projected `crs` makes a grid in degrees; give `crs`, with `extent` and `cellsize` in metres.
- **Composites that are too short.** Two months of scenes in the dry season are almost always clear; a composite of one scene in the wet season is full of clouds. `mask = "auto"` drops them, and the median of several dates is a far better image than any one of them. It is slow to read the first time.
- **Fire.** The Cerrado burns, and a recent burn is dark in a dry-season image and can look like something else. Look at the scenes the composite is made of.

## Going further

- The same chain with rules and shape on 0.6 m imagery of a neighbourhood: [part 14](/tutorials/object-based-image-analysis/case-urban-cover-naip/).
- A random forest on objects, the nearest neighbour, and a first look at accuracy:
  [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/).
- Classes without samples, from the attributes alone: [Classes without samples](/tutorials/object-based-image-analysis/classes-without-samples/).
- Two dates and the change of the objects between them: [Change by objects](/tutorials/object-based-image-analysis/change-by-objects/).
- Bigger areas, such as the Parque Nacional de Brasília (42,000 ha): [Large images](/tutorials/object-based-image-analysis/large-images/).
- The operators of this page: [[data.stac]], [[vector.read]], [[vector.rasterize]], [[raster.calc]], [[segment.multiresolution]], [[objects.features]],
  [[objects.samples]], [[ml.fit]], [[objects.classify]], [[objects.to_map]], [[objects.sample]], [[objects.accuracy]].

## References

- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality
  multi-scale image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Breiman, L. (2001). Random forests. *Machine Learning*, 45(1), 5-32. https://doi.org/10.1023/A:1010933404324
- Drusch, M., Del Bello, U., Carlier, S., Colin, O., Fernandez, V., Gascon, F., Hoersch, B., Isola, C., et al. (2012). Sentinel-2: ESA's optical
  high-resolution mission for GMES operational services. *Remote Sensing of Environment*, 120, 25-36. https://doi.org/10.1016/j.rse.2011.11.026
- Klink, C. A., & Machado, R. B. (2005). Conservation of the Brazilian Cerrado. *Conservation Biology*, 19(3), 707-713.
  https://doi.org/10.1111/j.1523-1739.2005.00702.x
- Radoux, J., & Bogaert, P. (2017). Good practices for object-based accuracy assessment. *Remote Sensing*, 9(7), 646.
  https://doi.org/10.3390/rs9070646
- Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for estimating area and
  assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42-57. https://doi.org/10.1016/j.rse.2014.02.015
- Ratter, J. A., Ribeiro, J. F., & Bridgewater, S. (1997). The Brazilian Cerrado vegetation and threats to its biodiversity. *Annals of
  Botany*, 80(3), 223-230. https://doi.org/10.1006/anbo.1997.0469
- Stehman, S. V. (2014). Estimating area and map accuracy for stratified random sampling when the strata are different from the map classes.
  *International Journal of Remote Sensing*, 35(13), 4923-4939. https://doi.org/10.1080/01431161.2014.930207
- Stehman, S. V., & Foody, G. M. (2019). Key issues in rigorous accuracy assessment of land cover products. *Remote Sensing of
  Environment*, 231, 111199. https://doi.org/10.1016/j.rse.2019.05.018
