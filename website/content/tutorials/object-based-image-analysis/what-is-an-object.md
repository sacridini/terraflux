---
title: "What an object is: segmentation"
short: What an object is
description: "Cut a Sentinel-2 image of Brasilia into objects with five segmenters, see what each does well, and learn what the scale, shape and compactness of the multiresolution method change."
order: 20
level: beginner
time: 45 min
ops: [data.stac, raster.calc, segment.multiresolution, segment.slic, segment.felzenszwalb, segment.watershed, segment.threshold, objects.to_vector, vector.write]
series: Object-based image analysis
part: 2
---

In the [first tutorial](/tutorials/object-based-image-analysis/first-map-by-objects/) you clicked a few objects and
got a map, but the objects came ready-made by a template. Here you open that box. You will download a real image, a
Sentinel-2 scene of the edge of a lake in Brasilia, and cut it into **objects** with five different methods, look at
the outlines each one draws, and learn what the three settings of the best-known method (the scale, the shape and the
compactness) do. At the end you will know what a segmentation is, which one to try first, and how to see the effect of
a setting before you commit to it, in the application and from the command line.

## The idea

A satellite image is a grid of **pixels**, each with one value per band. A pixel knows nothing about its neighbours:
it does not know that it is part of a lake, a roof or a field. **Object-based image analysis** (OBIA) first groups the
neighbouring pixels that belong together into **objects** (also called segments) and then works with the objects:
measuring them, naming them, mapping them. The grouping step is the **segmentation**, and the program that does it is a
**segmenter**.

![The Sentinel-2 clip of this series](/img/tutorials/object-based-image-analysis/what-is-an-object/clip.png "The Sentinel-2 clip of the series: the south edge of Lago Paranoá, Brasília, 9 August 2024, in true colours (about 5.8 × 5.8 km). The yellow box is the part drawn in the figures below.")

The clip above is the image of the whole series. It has a lake (dark, at the top), the neighbourhoods of Lago Sul
(the pale, textured blocks), strips of gallery forest along streams (dark green), the dry cerrado (brown, in the dry
season) and a few farm plots to the east. Look at the lake: thousands of pixels, all nearly the same dark colour.
Treated one by one they are thousands of separate answers to the question "is this water?"; as one object they are a
single answer, with an area, a perimeter and a shape.

![Pixels and the objects that group them](/img/tutorials/object-based-image-analysis/what-is-an-object/pixels-objects.png "Left: 40 × 40 pixels of the clip at the lake's edge, each pixel one square. Right: the same place as objects made by the multiresolution method (scale 2), each filled with its own mean colour.")

The picture above shows the idea on a small window. On the left are the pixels: each is a square of 10 m, and
neighbouring squares differ by noise as much as by anything real (a roof tile, a shadow, a tree). On the right, the
same window is cut into objects, each coloured with the average of its pixels. The noise has gone, the dark green
strip along the stream has become an object of its own, and each object now has properties that a pixel cannot have:
the standard deviation of its colour, its size, its outline, who its neighbours are. These properties are what the
later tutorials of the series use to classify.

### Where it comes from

The idea grew with the first images that were sharper than the things in them. When a pixel is much smaller than a tree
or a house, a single roof is a handful of pixels with different values, and classifying each pixel gives the
salt-and-pepper maps that remote-sensing users know well. The programs that appeared around the year 2000, eCognition
among them, cut the image into objects first. Baatz and Schäpe (2000) described the segmentation method that eCognition
made famous, the **multiresolution segmentation**, which Terraflux has as [[segment.multiresolution]]. A decade later,
Blaschke (2010) reviewed the field, from the software to the applications, in the journal of the photogrammetry
society. Hay and Castilla (2008) argued that the field deserved a name of its own, **geographic
object-based image analysis** (GEOBIA), and Blaschke and colleagues (2014) took stock of it and of where it was heading. You
will see both names, OBIA and GEOBIA, in the literature; they mean the same family of methods.

!!! science "Key idea"
    A segmentation is a **partition** of the image: every cell with data belongs to exactly one object, and each
    object is a set of cells that touch. In symbols, if the objects are \( O_1, O_2, \ldots, O_n \) and \( I \) is
    the set of cells of the image that have a value, then

    \[ O_1 \cup O_2 \cup \cdots \cup O_n = I \qquad\text{and}\qquad O_i \cap O_j = \varnothing \ \text{ for } i \ne j. \]

    A good segmentation has objects that are alike inside and different from their neighbours. How alike is "alike
    enough" is the choice of the person segmenting: it is the **scale**. Too many small objects (**over-segmentation**)
    keep noise and make everything slower; too few large ones (**under-segmentation**) put two classes in one object,
    and no later step can separate them.

In Terraflux a cell without a value in every band (a gap, a masked cloud) belongs to no object. Every segmenter below
gives the same result every time you run it, on any number of threads.

## When to use it

Segmentation is the first step of every object-based workflow, and the choices you make here decide what the rest can
find. Reach for it when:

- the things you map are **bigger than a pixel** and made of pixels that look alike: lakes, forest stands, fields,
  roofs, blocks of a town;
- you want **properties of whole things**: area, shape, texture, mean index, the neighbours around them;
- you want **polygons at the end**, ready for a report or a GIS, instead of a raster with a ragged edge;
- pixel classification gives a speckled map.

It is **not** the best choice when the thing you map is smaller than a pixel (a narrow stream under canopy in a 10 m
image) or when you need a number for every pixel regardless of objects (an index map, a continuous cover fraction). For
those, stay with rasters. The other tutorials of the series build on this one: the scale that fits (part 3), the
description of the objects (part 4) and classification (parts 5 and 6). If you come from eCognition, the page
[Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/) maps its algorithms to Terraflux's.

## What you need

- **The image.** A Sentinel-2 level-2A scene (surface reflectance), with the bands blue, green, red and near infrared
  at 10 m and the short-wave infrared band `swir16` at 20 m, resampled to the same grid. The tutorial's model fetches
  it for you with [[data.stac]], from Earth Search, which needs no key. If you have no network, download the clip
  instead: [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) (2.3 MB, in UTM 23S at 10 m, five
  layers named `blue`, `green`, `red`, `nir` and `swir16`). Its attribution is in
  [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt): *Contains modified Copernicus
  Sentinel data 2024*.
- **A model with a few nodes.** The next section builds it step by step.
- **Nothing else.** Every number on this page comes from running that model, with the settings written beside each
  figure.

!!! note
    Sentinel-2 files store reflectance as whole numbers, 10 000 times the real value (a reflectance of 0.07 is stored
    as 700). [[data.stac]] hands those numbers on as they are. Segmenters differ in how they treat the units, which
    is why the model divides the image by 10 000 in its third node. You will see why in
    [the pitfalls](#common-pitfalls).

## Build the model

### Step 1: the image

Add [[data.stac]] and give it the catalogue (`url`), the collection and the bands (`assets`). To make the page
reproducible the model asks for one scene, the one of 9 August 2024 (`datetime = "2024-08-09"`), a dry-season day with
almost no cloud (the item `S2A_22LHH_20240809_0_L2A`, 0.0005 % cloud), over a box in longitude and latitude
(`extent`, west, south, east, north, in degrees) on a grid of 0.00009 degrees (`cellsize`), which is about 10 m.

- **Why one date?** [[data.stac]] combines every scene it finds in the interval (by default with the median of the
  cells). With a single day it finds a single scene, and the figures of this page can be made again.
- **Why a box in degrees?** The grid of [[data.stac]] is given in longitude and latitude, as `extent` and `cellsize`
  (or, in the application, by connecting a map whose grid it should fill with the input `like`). Cells of
  0.00009 degrees are about 9.96 m high and 9.58 m wide at this latitude: not square, and not in metres. The next
  step fixes that.
- **A mask for clouds** (`mask = "auto"`) drops clouds and shadows pixel by pixel when a scene is cloudy. This scene
  needs none.

```tfgraph
#! caption: Steps 1 to 3: the scene read from Earth Search, put in UTM at 10 m, and its reflectance made 0 to 1 (nodes 1 to 3).
#! highlight: s2, utm, refl
#! numbered: true
#! fragment: true
[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }
note = "The scene of 9 August 2024, five bands (a box in degrees)."

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
note = "Reflectance from 0 to 1; the layers keep their names."
```

### Step 2: a projected grid

[[raster.reproject]] puts the stack in **SIRGAS 2000 / UTM zone 23S** (`crs = "EPSG:31983"`), a projected coordinate
system whose unit is the metre, on cells of exactly 10 m (`resolution = 10`) over a window of 5.8 × 5.8 km
(`extent`, `[xmin, ymin, xmax, ymax]` in metres of that system): 580 × 580 cells. Segmenters work on cells, but the
numbers you will report (the area of an object, its perimeter, its length) are only right when the cells are square and
measured in metres. UTM zone 23S is the one of Brasília; for your own study area use the zone, or the national
projection, that suits it. The cells are resampled bilinearly by default (`resampling = auto`), which suits
reflectance.

### Step 3: reflectance from 0 to 1

[[raster.calc]] takes the stack of five layers `s` and runs one line over each of them: `out image = s / 10000`. The
`out` word names the result (a stack asks for a name), and the layers keep the names `blue`, `green`, `red`, `nir` and
`swir16`, so later tutorials can ask for them by name. Run the node and look at the values: the lake's red band is
about 0.01, the cerrado's about 0.08 and the brightest roofs' 0.2 or more.

### Step 4: segment

Add [[segment.multiresolution]] and connect `refl.image` to its `image` input. Set `scale = 2`. The node has four
outputs: `objects` (the segments), `table` (one row per object with its `cells`, its `area`, and the mean and standard
deviation of each band), `scale` (the scale used, which is useful when you leave the setting empty and the node picks
one) and `below` (used with levels, part 8). The map of the objects in the next steps is made from `objects`.

### Step 5: polygons

[[objects.to_vector]] turns the objects into one polygon each, with its attributes (`id` and every column of the
`table`), and [[vector.write]] writes them to a file a GIS opens.

```tfgraph
#! caption: The whole model of this step: scene, UTM grid, reflectance, multiresolution objects (scale 2), polygons and a file.
#! highlight: segments, polygons
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }
note = "Five bands, one scene."

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
note = "Reflectance 0 to 1."

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }
note = "3,035 objects of about a hectare."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "segments.objects" }
note = "A polygon per object."

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "objects_scale2.gpkg" }
```

=== "Application"

    Start a new model and add the six nodes (click an operator in the library to add it), then **drag from an
    output port to an input port** to connect (`s2.raster` to the `s` input of `refl`, and so on). Choose a node and set its values in the **Node pane**. Select `segments` and press ++ctrl+enter++ (*Run to here*): the Viewer shows the objects, each
    outlined, with their number.

=== "Command line"

    Save the graph above as `objects.toml` (the **Model file (TOML)** toggle under it shows the file) and run it. The
    first run reads the scene from the server (a few seconds); later runs find it in Terraflux's cache.

    ```bash
    terraflux run objects.toml --threads 4 --out output
    ```

=== "Without a network"

    Download [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) beside the
    model and replace the nodes `s2` and `utm` with a file read (the file is already in UTM). Nothing else changes, except that the download is rounded to
    whole numbers: the counts on this page can differ by a few objects (scale 2 gives 2,992 objects from the file,
    3,035 from the server).

    ```toml
    [[nodes]]
    id = "s2"
    op = "raster.read"
    params = { path = "sentinel2_brasilia.tif" }
    ```

## Read the results

Run the model and open `segments` in the Viewer. The scale 2 on this image gives **3,035 objects** over 336,400
cells: about 111 cells, or 1.1 hectares, each. Their sizes are far from equal. In the dry brown cerrado, where neighbouring
cells are almost the same, objects are large; in the neighbourhoods, where the colour changes from roof to street to
tree every few cells, they are small. That is the character of this method: it grows an object until the next merge
would cost too much, so the size follows the local variation of the image.

![Multiresolution objects over the clip](/img/tutorials/object-based-image-analysis/what-is-an-object/seg_mr.png "The multiresolution segmentation, scale 2 (shape 0.1, compactness 0.5): 3,035 objects over the whole clip, drawn here over the part in the yellow box.")

Look at the figure with these questions in mind.

- **Does an object cover one thing?** The lake is cut into several objects, which is harmless (they are all water, and
  they can be joined later). A roof and the street beside it in one object would be harmful.
- **Do the outlines follow edges you can see?** Along the lake shore and the gallery forest they do; in the
  neighbourhoods, many objects follow the pattern of roof and shadow.
- **Is it too fine or too coarse for your question?** For the town's blocks this is fine; for the cerrado it is too
  fine. The scale picks the compromise, and part 3 shows how to choose it with numbers.

Terraflux gives a row of the `table` per object. The columns are `id`, `cells`, `area` (in the units of the grid: here
square metres, because the clip is in UTM), and `mean_<band>` and `sd_<band>` for every band.
They are what [[objects.features]] extends in part 4.

!!! tip
    To count or measure objects in hectares, divide `area` by 10 000 (a cell of 10 m is 100 m², or 0.01 ha, so
    `cells` / 100 is the area in hectares). This works because the clip is in a projected system; in longitude and
    latitude the area would be in square degrees, which no report wants.

## Five segmenters side by side

The same image, cut by five methods. The settings were chosen to give objects of a comparable size, about a hectare,
except where the method works otherwise. All run on reflectance from 0 to 1 over the same part of the clip.

```tfgallery
columns: 3
caption: "Outlines of five segmenters over the same part of the clip. Multiresolution: scale 2. SLIC: size 12. Felzenszwalb: k = 2. Watershed: the defaults. Threshold: two classes by Otsu, on the NDWI."
images:
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/seg_mr.png, caption: "Multiresolution, 3,035 objects" }
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/seg_slic.png, caption: "SLIC, 2,270 objects" }
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/seg_fz.png, caption: "Felzenszwalb, 1,643 objects" }
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/seg_ws.png, caption: "Watershed, 747 objects" }
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/seg_th.png, caption: "Threshold (NDWI), 12 objects" }
```

All five are nodes you add after `refl`, each with its own output of objects. Here are the nodes; the figure above is
what each one gave.

```tfgraph
#! caption: Five segmenters on the same image. The threshold reads the water index (the NDWI) made by a second raster.calc.
#! highlight: mr, slic, fz, ws, th
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }

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
id = "ndwi"
op = "raster.calc"
inputs = { s = "refl.image" }
params = { expression = "(s.green - s.nir) / (s.green + s.nir)" }
note = "The normalised difference water index."

[[nodes]]
id = "mr"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }

[[nodes]]
id = "slic"
op = "segment.slic"
inputs = { image = "refl.image" }
params = { size = 12 }

[[nodes]]
id = "fz"
op = "segment.felzenszwalb"
inputs = { image = "refl.image" }
params = { k = 2 }

[[nodes]]
id = "ws"
op = "segment.watershed"
inputs = { image = "refl.image" }

[[nodes]]
id = "th"
op = "segment.threshold"
inputs = { image = "ndwi.map" }
params = { classes = 2 }
```

```tftable
sortable: true
labels: { segmenter: Segmenter, objects: Objects, mean_ha: "Mean size (ha)", good_at: Good at }
align: { segmenter: left, good_at: left }
digits: { objects: 0, mean_ha: 2 }
caption: "What came out of each segmenter on the whole clip (336,400 cells). The mean size is cells per object, in hectares of about 0.01 ha a cell."
csv: |
  segmenter,objects,mean_ha,good_at
  Multiresolution,3035,1.11,Objects whose size follows the local variation
  SLIC,2270,1.48,"Even, compact tiles; a fast first cut"
  Felzenszwalb,1643,2.05,"Thin and long things (streams, roads) kept whole"
  Watershed,747,4.50,Objects bounded by strong edges
  Threshold,12,280.33,One band cut at its own natural gap (water)
```

### Multiresolution

[[segment.multiresolution]], Baatz and Schäpe's method, starts with every cell as its own object and merges neighbours, pass after pass, while the
cost of merging stays below the **scale squared**. The cost is made of a **colour** part (how much the standard
deviation of the bands grows when the two join, summed over the bands) and a **shape** part (how much the outline
gets less compact and less smooth). A pair merges only when each is the other's best neighbour, which keeps objects
growing evenly instead of one swallowing the rest. It is the method that most closely follows what eCognition does,
and the first choice when you want objects that follow the image.

It is good at objects of a natural size: big where the image is flat, small where it changes. It is slower than the
others (about a second on this clip, on one thread) and its scale depends on the units of the image, which is why
the model divides the image by 10 000.

### SLIC

[[segment.slic]] (Achanta et al., 2012) puts seed points on a regular lattice, and then runs a k-means in which every cell is
assigned to the nearest seed in colour **and** position, looking only at a window around each seed. `size` is the
approximate side of a superpixel in cells; `compactness` weighs position against colour (a higher value gives more
regular tiles). In the figure the tiles are almost a grid over the lake and the cerrado, where there is nothing to
follow, and they bend around roofs in town. SLIC is the fastest here, and the objects are all about the same size,
which is both its strength (a uniform start for the next step) and its weakness (it cuts a homogeneous field into
squares).

### Felzenszwalb

[[segment.felzenszwalb]] follows Felzenszwalb and Huttenlocher (2004), who treat the image as a graph: every cell is a node, joined to its neighbours by
an edge weighted with the difference of their bands. Taking the edges from the lowest weight up, two components join
when the edge is not heavier than the largest difference **inside** either of them plus `k` divided by its size, so a
small component joins easily and a large one needs a closer match. Segments under `min_size` cells (20 by default)
are then merged into a neighbour. Because the rule looks at the heaviest edge inside, a thin line of similar cells,
such as a stream or a road, can stay whole, and the sizes vary a lot. The outlines are ragged, as
the figure shows in the neighbourhoods.

### Watershed

[[segment.watershed]] (Vincent & Soille, 1991) thinks of the image's **gradient** (how fast the bands change from one cell to
the next) as a relief and floods it from its low points, the flat areas. Each flat area becomes a basin, and the
borders between basins fall on the crests, the strongest edges. By default the start points are the minima that stand
at least a twentieth of the strongest edge above the lowest cell they would join (`h`); a shallower minimum is noise.
With this rule the clip has only 747 objects, big ones. The lake shore is placed almost exactly, and so are the
bright roofs, but a whole neighbourhood can end up in one object. Lower `h` for more objects. You can also give your
own markers: a map whose cells with a value are the seeds.

### Threshold

[[segment.threshold]] is different in kind. It cuts **one band** (here the normalised difference water index, NDWI, \( \frac{\text{green} - \text{nir}}{\text{green} + \text{nir}} \), high on water) at one or more thresholds, and every connected piece of a class is an object.
Terraflux finds the thresholds with Otsu's method (Otsu, 1979), the cut that makes the two groups of values as
different as possible, or takes the ones you give. For this clip Otsu puts the cut at an NDWI of 0.065, and the result
is 12 objects: the lake, a few ponds and the land around them, which is one big connected object. It cannot divide a
neighbourhood, but for the one thing it was made for, water, it follows the shore as cleanly as anything above. Part 7
of the series is about it.

!!! note
    This is not a ranking. Each segmenter solves a slightly different problem, and in a real project you often use
    two: a threshold for water, a multiresolution for the rest. Terraflux's other segmenters
    ([[segment.quickshift]], [[segment.meanshift]], [[segment.region_growing]], [[segment.snic]]) are in the operator
    reference.

## The multiresolution settings

[[segment.multiresolution]] has three settings that change the objects: `scale`, `shape` and `compactness`. A fourth,
`weights`, gives each band a weight (all 1 by default), and the seed changes the order the regions are visited in.

### The scale

The scale is the main lever. It is the **square root of the cost** a merge may have (the cost is in the image's units
squared), so a larger scale lets objects keep merging and grow bigger. The slider below shows seven runs of the same
image, from the scale 1 to the scale 8, over the same part of the clip. Move it and watch the objects grow, the
number fall, and the town's blocks turn from single roofs to whole streets and then into mixed blocks.

```tfslider
label: Scale
caption: "The same part of the clip segmented with the scale from 1 to 8 (shape 0.1, compactness 0.5). The chart counts the objects of the whole clip."
frames:
  - { value: 1, src: /img/tutorials/object-based-image-analysis/what-is-an-object/scale_1.png, caption: "Scale 1", stats: { Objects: "11,145", "Mean size": "0.30 ha" } }
  - { value: 1.5, src: /img/tutorials/object-based-image-analysis/what-is-an-object/scale_1_5.png, caption: "Scale 1.5", stats: { Objects: "5,219", "Mean size": "0.64 ha" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/what-is-an-object/scale_2.png, caption: "Scale 2", stats: { Objects: "3,035", "Mean size": "1.11 ha" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/what-is-an-object/scale_3.png, caption: "Scale 3", stats: { Objects: "1,338", "Mean size": "2.51 ha" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/what-is-an-object/scale_4.png, caption: "Scale 4", stats: { Objects: "739", "Mean size": "4.55 ha" } }
  - { value: 6, src: /img/tutorials/object-based-image-analysis/what-is-an-object/scale_6.png, caption: "Scale 6", stats: { Objects: "313", "Mean size": "10.75 ha" } }
  - { value: 8, src: /img/tutorials/object-based-image-analysis/what-is-an-object/scale_8.png, caption: "Scale 8", stats: { Objects: "187", "Mean size": "17.99 ha" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/what-is-an-object/scale.csv
  x: scale
  y: [objects]
  labels: { objects: "Objects" }
  ylabel: "Objects in the clip"
```

Doubling the scale does not halve the number of objects: from the scale 2 to 4 the count fell from 3,035 to 739, a
factor of four, as an object's area grows roughly with the square of the scale. At the scale 8 the lake is a few
objects and the town has merged into district-sized pieces that mix roofs, trees and streets. There is no scale that
is right in itself: it is right when the objects are about the size of what you want to classify. If you leave `scale`
empty, the node uses the first scale suggested by [[segment.scale]] (here about 2.14, close to the 2 used above), and says so in a warning and in
its `scale` output. That suggestion is where part 3 starts.

### Shape and compactness

`shape` (0 to 0.9, default 0.1) is the weight of the shape part of the cost against the colour part. At 0 only colour
counts, and objects follow the image whatever outline results; with more weight on shape, objects prefer outlines
that are compact and smooth, even at some cost in colour. `compactness` (0 to 1, default 0.5) says, within the shape
part, whether to prefer compact outlines (close to a circle or a square) or smooth ones (a short boundary, however long
the object). Both show up at the same scale: a higher weight on shape leaves the objects smaller, because the shape
term adds to a cost that the scale still limits. On this image more compactness gave more objects too.

```tfgallery
columns: 3
caption: "The effect of shape at scale 2 (compactness 0.5)."
images:
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/sh1.png, caption: "shape 0.1 (default), 3,035 objects" }
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/sh5.png, caption: "shape 0.5, 7,982 objects" }
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/sh9.png, caption: "shape 0.9, 12,927 objects" }
```

```tfgallery
columns: 3
caption: "The effect of compactness at scale 2 with shape 0.5."
images:
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/co1.png, caption: "compactness 0.1, 2,963 objects" }
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/sh5.png, caption: "compactness 0.5, 7,982 objects" }
  - { src: /img/tutorials/object-based-image-analysis/what-is-an-object/co9.png, caption: "compactness 0.9, 13,326 objects" }
```

Colour follows the image; shape tidies it. For natural landscapes, where edges are irregular, leave `shape` low
(the default 0.1 is a sound start) and change the scale first. Raise `shape` and `compactness` when the objects are
man-made (buildings, fields) and you want regular outlines, and expect to raise the scale to keep the same number of
objects.

## In the application

You do not have to run the model to see what a setting does. The application segments the part of the image you are
looking at while you move a slider.

**Live segmentation.** Click a node that is a segmenter (any of the five above, and also [[segment.snic]],
[[segment.spectral_difference]], [[segment.region_growing]], [[segment.quickshift]] and [[segment.meanshift]]) and
run it once, so that its image is in memory. In the controls beside the map, a **Live segmentation** section has a
slider per main setting:

| Segmenter | Sliders |
|---|---|
| multiresolution | `scale` (logarithmic), `shape`, `compactness` |
| SLIC and SNIC | `size` (cells), `compactness` |
| Felzenszwalb | `k`, `sigma` (cells) |
| watershed | `h`, `compactness` |
| threshold | `classes` |

Move one and the part of the image in view, with a small margin, is segmented in a worker thread of its own; a few
moments later the new outlines are drawn over the map in magenta, with the number of objects. Pan or zoom and the new
view is segmented. Nothing in the model changes until you press **Apply**, which writes the values in the node's
settings as one step of undo (**Reset** goes back). For the multiresolution, the panel also draws the ESP curve of the
window, with buttons for the scales it suggests: part 3 explains it.

**Compare...** The button under the sliders opens a window, *Compare segmenters*, that cuts the same window of the
image with several segmenters at once and shows each in a thumbnail with its outlines, the **number of objects** and
the **seconds** it took. A checkbox for each choice: multiresolution, SLIC, watershed and Felzenszwalb are ticked at
the start; threshold, quickshift, region growing and mean shift are one click away. Each segmenter runs with the
settings of a node of the same kind on the same image if the model has one, and with the sliders' defaults if not. The
window is the whole image when it has no more cells than the slowest method does in a second (250,000 cells); this
tutorial's clip has 336,400, so it compares on the part in view: zoom in on the lake shore and see. Under each result,
**Use** puts that segmenter in place of the node you are looking at: whatever read the old node's objects now reads the
new one's, the old node stays (with nothing connected to it) and the change is one step of undo.

!!! tip
    Use Compare once, on a part that has all of your classes (a lake shore with a town and a forest in this clip),
    to choose the method, and the sliders to choose the scale. Then run the node on the whole image.

## Common pitfalls

- **The scale means something different for every image.** [[segment.multiresolution]] does not standardise the
  bands: it uses the values as they are, and its scale is in those units. The same scale 2 on reflectance 0 to 1 and on
  the raw 0 to 10 000 would give completely different objects (and Terraflux warns when it sees an image that looks
  like 0 to 10 000). The other segmenters of this page standardise the bands by default (so `k` and `size` mean the
  same on any image), and the scale of the multiresolution is the one you must watch. Divide Sentinel-2 by 10 000, as
  step 2 does, and keep that choice in every model of a series.
- **Comparing scales across images.** A scale of 2 here is not a scale of 2 in a Landsat scene or in a drone image.
  Compare objects (their count and size), not the number.
- **Over-segmenting "to be safe".** Many tiny objects look careful but keep the noise, slow every later step and
  make the table of attributes huge. A size a few times larger than a pixel, and smaller than the things you map, is
  safer.
- **Under-segmenting to get a clean map.** An object that holds two classes cannot be fixed later. Check the
  boundary of a class you care about (the lake shore, a field edge) at the scale you pick.
- **Judging by one place.** A scale that is right in town is too fine in the cerrado. Look at all the classes, and
  consider levels (part 8 of the series) when they need different scales.
- **Areas in square degrees.** Had the clip stayed in longitude and latitude, areas would be in square degrees and
  the cells not square. Always [[raster.reproject]] to a projected system (UTM or an equal-area one) before
  segmenting if you will report areas.
- **Cells without data.** A cell with no value in some band is no object: the objects have holes where the image has
  gaps. Fill them or mask them in the way your question needs.
- **The threshold's objects are huge.** Connected pieces of a class are objects, so all the land that touches is one
  object. That is not a mistake, but it is not a general segmentation either.

## Going further

- Next in the series: the right scale, with the ESP tool and the [[segment.optimize]] operator (part 3), and then
  describing the objects, classifying them from samples and from rules (parts 4 to 6).
- A hands-on first map: [Your first map by objects](/tutorials/object-based-image-analysis/first-map-by-objects/).
- The operators of this page: [[segment.multiresolution]], [[segment.slic]], [[segment.felzenszwalb]],
  [[segment.watershed]], [[segment.threshold]], [[objects.to_vector]] and [[raster.calc]].
- The Viewer and its controls: [the application's Viewer](/app/viewer/).

## References

- Achanta, R., Shaji, A., Smith, K., Lucchi, A., Fua, P., & Süsstrunk, S. (2012). SLIC superpixels compared to
  state-of-the-art superpixel methods. *IEEE Transactions on Pattern Analysis and Machine Intelligence*, 34(11),
  2274-2282. https://doi.org/10.1109/TPAMI.2012.120
- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality
  multi-scale image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Blaschke, T. (2010). Object based image analysis for remote sensing. *ISPRS Journal of Photogrammetry and Remote
  Sensing*, 65(1), 2-16. https://doi.org/10.1016/j.isprsjprs.2009.06.004
- Blaschke, T., Hay, G. J., Kelly, M., Lang, S., Hofmann, P., Addink, E., Queiroz Feitosa, R., van der Meer, F.,
  van der Werff, H., van Coillie, F., & Tiede, D. (2014). Geographic object-based image analysis: towards a new
  paradigm. *ISPRS Journal of Photogrammetry and Remote Sensing*, 87, 180-191.
  https://doi.org/10.1016/j.isprsjprs.2013.09.014
- Felzenszwalb, P. F., & Huttenlocher, D. P. (2004). Efficient graph-based image segmentation. *International
  Journal of Computer Vision*, 59(2), 167-181. https://doi.org/10.1023/B:VISI.0000022288.19776.77
- Hay, G. J., & Castilla, G. (2008). Geographic object-based image analysis (GEOBIA): a new name for a new
  discipline. In T. Blaschke, S. Lang, & G. J. Hay (Eds.), *Object-based image analysis: spatial concepts for
  knowledge-driven remote sensing applications* (pp. 75-89). Springer.
  https://doi.org/10.1007/978-3-540-77058-9_4
- Otsu, N. (1979). A threshold selection method from gray-level histograms. *IEEE Transactions on Systems, Man, and
  Cybernetics*, 9(1), 62-66. https://doi.org/10.1109/TSMC.1979.4310076
- Vincent, L., & Soille, P. (1991). Watersheds in digital spaces: an efficient algorithm based on immersion
  simulations. *IEEE Transactions on Pattern Analysis and Machine Intelligence*, 13(6), 583-598.
  https://doi.org/10.1109/34.87344
