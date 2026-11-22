---
title: "Water by thresholds"
short: Water by thresholds
description: "Map the water of a Sentinel-2 scene with no samples and no scale to choose: cut the NDWI at the threshold Otsu's method finds, turn the water into polygons of the shore, and see what merging objects by spectral difference adds."
order: 70
level: beginner
time: 30 min
template: Water by Otsu on the NDWI
ops: [raster.calc, segment.threshold, objects.rules, segment.slic, segment.spectral_difference, objects.to_vector, vector.write]
series: Object-based image analysis
part: 7
---

Water is the one class that almost every remote-sensing project maps, and the one where object-based analysis can be
simplest. In this tutorial you will map the water of the Sentinel-2 clip of Lago Paranoá with no training samples and
no scale to choose: you compute a water index, let a statistical rule place one threshold, and keep the pieces above it
as objects. At the end you will have the lake, its small neighbouring ponds and a polygon file of their shores; you will
know how far the answer moves when the threshold does, and you will have met a second way of making objects from a
single band, [[segment.spectral_difference]], for the day one threshold is not enough.

## The idea

Water is easy to see because it absorbs the near infrared. A leaf, a roof or dry ground reflects the near infrared
(NIR) at least as strongly as the green light; clear water reflects a little green and almost no NIR. McFeeters (1996)
turned this difference into an index, the **normalised difference water index**, NDWI:

\[ \text{NDWI} = \frac{\text{green} - \text{nir}}{\text{green} + \text{nir}}. \]

It runs from −1 to 1. Water has a value above 0; vegetation, soil and buildings have values below it. The picture shows
it for the clip of the series, with the lake in blue and the land from orange to dark red.

![The NDWI of the clip](/img/tutorials/object-based-image-analysis/water-by-thresholds/ndwi.png "The NDWI of the part of the clip around the lake. Blue is high (water), red is low. The gallery forest along the stream below the lake is the darkest red.")

The index turns a picture of colours into **one number per cell**, and for water it splits the scene into two groups
that hardly overlap. The histogram makes this plain: two mountains, the land on the left (mean about −0.5) and the water
on the right (about 0.65), and a valley between them with very few cells.

![The histogram of the NDWI](/img/tutorials/object-based-image-analysis/water-by-thresholds/histogram.png "The NDWI of every cell of the clip, on a logarithmic scale. The land and the water are two modes with a thin valley between them. The orange line is the threshold found by Otsu's method, 0.065; the dashed line is NDWI = 0.")

A **threshold** is a value on that axis: the cells above it are water, the others are land. You could pick one by eye (0
is the number most papers use), but Otsu (1979) gave a way to let the data pick it. For each possible cut, take the two
groups it makes and measure how different they are; the best cut is the one that makes them most different.

!!! science "Key idea"
    **Otsu's method.** Cut the values at \( t \). The cells below it are group 0, the cells above it group 1. Let
    \( \omega_0 \) and \( \omega_1 \) be the shares of the cells in each group (they add to 1) and \( \mu_0 \) and
    \( \mu_1 \) the mean value of each. The **variance between the groups** is

    \[ \sigma_B^2(t) = \omega_0\, \omega_1\, (\mu_0 - \mu_1)^2. \]

    Otsu's threshold is the \( t \) that makes \( \sigma_B^2 \) as large as possible: where the two groups are most
    different from each other and, as a consequence, each is most alike inside. With a bimodal histogram it falls in the
    valley between the modes. The multi-level version, for three or more classes, puts \( k - 1 \) thresholds for \( k \)
    classes by the same rule (Liao et al., 2001).

An **object** here is not a chunk made by merging neighbours: it is each **connected piece of one class**. All the cells
above the threshold that touch form one object; all the land that touches, another. That is what
[[segment.threshold]] does (eCognition has the same algorithm, as *multi-threshold segmentation*), and it is why the
lake is a single object and not thousands of cells.

## When to use it

A threshold on one band suits what **one band separates** well:

- **water against land**, as here, with the NDWI (or the MNDWI of Xu, 2006, that replaces the NIR with the short-wave
  infrared and is less often fooled by buildings);
- burnt against unburnt ground (a burn index), snow against bare ground, a flooded area after a flood;
- anything a threshold of a ratio or a height (a canopy height model) cuts, with a clear gap in its histogram.

It is **not** the tool when:

- the scene has **no water**: Otsu always finds a cut, even where there is nothing to cut (an example is in the
  pitfalls);
- the classes **overlap** in the band: a turbid lake and a wet soil can have the same NDWI. Use more bands and
  samples ([Your first map by objects](/tutorials/object-based-image-analysis/first-map-by-objects/) is a start);
- you want **many classes** (forest, grass, built-up, water). Use a classifier, or rules over several attributes.

If you want a map of the whole landscape rather than the water alone, the classification tutorials of the series
come next.

## What you need

- **The image.** The Sentinel-2 clip of the series: blue, green, red, near infrared and `swir16`, UTM 23S at 10 m, read by
  [[data.stac]] as in [part 2](/tutorials/object-based-image-analysis/what-is-an-object/), or the download
  [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) (2.3 MB) and
  [[raster.read]]. Its attribution is in [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt).
  The bands must be named `green` and `nir`, because the NDWI formula reads them by name; [[data.stac]] names them after the
  assets you ask for, and the download has its layers named the same way.
- **The template**, in the application: *Water by Otsu on the NDWI* (group *Objects*). It runs on a synthetic six-band image
  drawn by chance; replace the image with yours with *Use my data* or a [[raster.read]] of a stack with a green and a
  near-infrared band.

!!! note
    The NDWI is a **ratio** of two bands, so the units cancel: the same value comes out from Sentinel-2's integers (0 to
    10 000) as from reflectance (0 to 1). That is different from the multiresolution scale, which depends on the units
    ([part 3](/tutorials/object-based-image-analysis/choosing-the-scale/)). This page works on reflectance anyway, to stay
    on the same image as the others.

## Build the model

### Step 1: the image

The first two nodes are those of the previous tutorials: [[data.stac]] reads the scene of 9 August 2024 onto 10 m
cells in UTM zone 23S, and [[raster.calc]] makes reflectance. They are in the full model below; the fragment
shows the nodes of this page.

### Step 2: the index

A [[raster.calc]] node with the image `s` and the expression `(s.green - s.nir) / (s.green + s.nir)`. It is one line with
no `out`, so the node has one output, `map`. A band is read as `s.green`; if your layers are named otherwise, use their
names.

### Step 3: the threshold

Add [[segment.threshold]], connect `ndwi.map` to `image`, and give it `classes = 2`. Otsu is the default when you give no
`thresholds`. The settings are few:

- `classes` (2 to 8): the number of classes whose thresholds multi-level Otsu finds; `thresholds`, a list in ascending
  order, gives the cuts yourself instead.
- `band`: which band to cut, for a stack (the first by default). Here the NDWI is one layer.
- `bins` (default 256): the bins of the histogram Otsu works on; more place the cut more finely.
- `neighbours`: 4 (default, cells sharing an edge) or 8 (corners count too) for what "connected" means.

The outputs are `objects` (the connected pieces of the classes), `table` (a row per object: `id`, `cells`, `area` in map
units squared, `class` from 1, and the mean and standard deviation of the band cut, here `mean_image` and `sd_image`) and
`thresholds` (a row per cut: here 0.0607). **Class 1 is the values below the first threshold, class 2 those above it.**
With Otsu the threshold is also written in the run's warnings.

### Step 4: name the classes

The pieces are numbered 1 and 2, not named. [[objects.rules]] gives them names, with one **fuzzy rule** for each: an
expression that is a **membership from 0 to 1**. `above(x, a, b)` is 0 up to `a`, 1 from `b`, and a line between, so
`above(class, 1, 2)` is 0 for class 1 and 1 for class 2: the `water` rule. `below(class, 1, 2)` is its opposite: `land`. The
classes are numbered in the order of the list, so water is 1 and land is 2 in the map this node writes (not the same
numbers as the threshold's). Each object takes the class with the highest membership; the `min` setting (0.1 by default)
leaves an object unclassified when its best membership is under it, which does not happen with a rule that is 0 or 1.

### Step 5: polygons of the shore

[[objects.to_vector]] gives one polygon per object, with its class (the name), `id`, `cells`, `area` and the mean and
standard deviation of the band. The edges follow the cell edges exactly, as stairs; `smooth` cuts the corners that many
times (1 to 3 look natural) and shrinks each polygon a little at each corner. [[vector.write]] writes the file, a
GeoPackage by the extension (also `shp`, `geojson`, `fgb`...). A [[view.plot]] on the map of the classes shows it in the
application.

```tfgraph
#! caption: The model: scene, UTM grid, reflectance, the NDWI, two classes by Otsu, water and land, polygons of the shore (smoothed once) and a file.
#! highlight: ndwi, split, water, polygons
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [190520, 8241280, 196320, 8247080], cellsize = 10, crs = "EPSG:31983" }
note = "Five bands, one scene."

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "s2.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance 0 to 1."

[[nodes]]
id = "ndwi"
op = "raster.calc"
inputs = { s = "refl.image" }
params = { expression = "(s.green - s.nir) / (s.green + s.nir)" }
note = "McFeeters' index: above 0 over water."

[[nodes]]
id = "split"
op = "segment.threshold"
inputs = { image = "ndwi.map" }
params = { classes = 2 }
note = "Two classes by Otsu: 15 objects, cut at 0.061."

[[nodes]]
id = "water"
op = "objects.rules"
inputs = { objects = "split.objects" }
params = { classes = [{ name = "water", rule = "above(class, 1, 2)", colour = "#2b6cb0" }, { name = "land", rule = "below(class, 1, 2)", colour = "#c8b88a" }] }
note = "Class 2 (high NDWI) is water, class 1 land."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "water.objects" }
params = { smooth = 1 }
note = "A polygon per object, with its class."

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "water.gpkg" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "water.map" }
params = { title = "Water (1) and land (2)" }
```

=== "Application"

    Open the template *Water by Otsu on the NDWI* (the library's *Templates*, group *Objects*) and run it
    (++ctrl+enter++ on the last node), or add the nodes above (click an operator in the library to add it, **drag from an
    output port to an input port** to connect, set the values in the **Node pane**). Select `split` and press
    ++ctrl+enter++: the Viewer shows the objects, and the node's `thresholds` output is the cut. To use your own image, put it in the
    place of the template's image with *Use my data*: the template needs a stack with layers named `green` and `nir`.

=== "Command line"

    Save the graph above as `water.toml` and run it. The polygons are written to `water.gpkg`.

    ```bash
    terraflux run water.toml --threads 4 --out output
    ```

=== "Without a network"

    Download [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) beside the model
    and replace the node `s2` with a [[raster.read]] of it. The download is rounded to whole numbers, so the
    numbers of this page can differ in the last decimals.

    ```toml
    [[nodes]]
    id = "s2"
    op = "raster.read"
    params = { path = "sentinel2_brasilia.tif" }
    ```

## Read the results

Otsu's cut on this clip is an NDWI of **0.061**. It gives **15 objects**: four of land, the largest all the ground that touches
(314,288 cells, 3,143 ha, 93.4 % of the clip), and eleven of water. The lake itself is the largest water object, **20,887
cells, 208.9 ha**; the others are a second arm of the lake cut by the clip's edge (739 cells, 7.4 ha) and small ponds and
wet hollows along the stream below the lake (from 229 cells down to a single cell). Together the water covers 22,108 cells,
**221.1 ha**, 6.6 % of the clip.

![The water of the clip](/img/tutorials/object-based-image-analysis/water-by-thresholds/water_clip.png "The water found by Otsu on the NDWI (blue) over the clip. The yellow box is the part drawn in the next figures, the white one the part of the polygons figure.")

The shore is placed by the NDWI of each cell: the cells at the edge of the lake, mixing water and land, fall on one side
or the other of the cut. That is the **mixed pixel** problem of a 10 m image, and it is why the polygons that come out are
good to about one cell (10 m), not better.

The polygons come out of [[objects.to_vector]]. Look at a part where the lake narrows and ponds appear:

![The polygons of the shore](/img/tutorials/object-based-image-analysis/water-by-thresholds/polygons.png "The water polygons over a detail of the clip (white box above): the cell edges (smooth = 0, left) and the corners cut once (smooth = 1, right).")

With `smooth = 0` the polygons follow the cell edges exactly, so their area is the area of the cells (221.08 ha); with
`smooth = 1` the outline looks like a shore and the area is 220.32 ha, 0.3 % smaller. Keep `smooth = 0` for areas you will
report and use `smooth` for maps.

**To report it:** the image (sensor, scene, date, cloud cover), the index and its formula, the method and the threshold
(Otsu on the NDWI, 0.061), the area of water and how many water bodies there are, and a map with the polygons over the
image. A single figure of the area has no error with it; the next section shows how far it moves.

## When the threshold moves

Move the slider. It shows the water for ten given thresholds (`thresholds = [t]`) over the part of the clip around the
lake, with the water area of each and, in the chart, how the area changes. The 0.061 frame is Otsu's own.

```tfslider
label: NDWI threshold
caption: "The water (blue, with a cyan edge) for ten given thresholds of the NDWI, over the lake and the town below it. The chart is the area of the water in the whole clip."
frames:
  - { value: -0.3, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_tn0_3.png, caption: "NDWI above -0.3", stats: { "Water area": "472.4 ha", "Water objects": "1,913" } }
  - { value: -0.2, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_tn0_2.png, caption: "NDWI above -0.2", stats: { "Water area": "294.1 ha", "Water objects": "1,219" } }
  - { value: -0.1, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_tn0_1.png, caption: "NDWI above -0.1", stats: { "Water area": "234.7 ha", "Water objects": "350" } }
  - { value: 0, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_t0.png, caption: "NDWI above 0", stats: { "Water area": "223.1 ha", "Water objects": "26" } }
  - { value: 0.061, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_otsu.png, caption: "NDWI above 0.061 (Otsu)", stats: { "Water area": "221.1 ha", "Water objects": "11" } }
  - { value: 0.2, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_t0_2.png, caption: "NDWI above 0.2", stats: { "Water area": "217.3 ha", "Water objects": "10" } }
  - { value: 0.3, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_t0_3.png, caption: "NDWI above 0.3", stats: { "Water area": "213.9 ha", "Water objects": "11" } }
  - { value: 0.4, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_t0_4.png, caption: "NDWI above 0.4", stats: { "Water area": "208.8 ha", "Water objects": "4" } }
  - { value: 0.5, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_t0_5.png, caption: "NDWI above 0.5", stats: { "Water area": "199.7 ha", "Water objects": "10" } }
  - { value: 0.6, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/thr_t0_6.png, caption: "NDWI above 0.6", stats: { "Water area": "175.3 ha", "Water objects": "40" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/water-by-thresholds/sweep.csv
  x: threshold
  y: [water_ha]
  labels: { water_ha: "Water area (ha)" }
  colors: { water_ha: "#2f6f9f" }
  ylabel: "Water area (ha)"
```

Read the slider from left to right.

- **Below about −0.1** the threshold lets in what is not water: at −0.3 the "water" is 472 ha, twice the lake, and the picture
  shows why: the roofs and the shaded ground of the town (the NDWI of a building is not as low as that of a leaf) and the
  dark stream corridor join the water. The count of objects, 1,913, is a sign: real water bodies are few and large.
- **From −0.1 to 0.5** the area hardly moves: 235 to 200 ha, within 10 % of Otsu's 221.0 ha, and the number of objects
  falls from 350 to a handful. This is the plateau of the valley in the histogram, where few cells lie, and any cut in it
  gives about the same lake. It is why a fixed cut of 0 (223.1 ha, 26 objects) works here, and why Otsu's 0.061 is a safe
  middle.
- **Above 0.5** the lake starts to shrink from its edges: the cells along the shore, where water is shallow or mixed with
  the bank, have a lower NDWI than the open water and are cut off (175 ha at 0.6, with 40 pieces as the edge breaks up).

So the area is stable across a range of thresholds, which is the sign of a good index on this image. Where the plateau is
narrow, or the valley is full of cells (turbid water, mixed pixels), the threshold you choose is the answer you get.

## Objects first: spectral difference

A threshold makes the pieces and the classes in one move. The other way is to **make objects first**, from the image, and
classify them afterwards. [[segment.spectral_difference]] (eCognition's *spectral difference segmentation*) does the
first part: it takes objects you already have, a superpixel layer or a multiresolution level, and merges neighbouring
objects whose mean values differ by less than `max_difference` (in the units of the image), recalculating the mean of
the merged object each time. It merges the pair with the smallest difference first, so the result is always the same.

The model below cuts the NDWI into small superpixels with [[segment.slic]] (`size = 6`: 9,401 objects), merges them
with a limit, and then names the classes with [[objects.rules]] on the mean NDWI of each object, with a ramp around the
Otsu cut instead of a step.

```tfgraph
#! caption: Objects first: SLIC superpixels of the NDWI (node 1), merged where the mean NDWI differs by less than 0.1 (node 2), then named by the mean (node 3).
#! highlight: sl, sd, owater
#! numbered: true
#! fragment: true
[[nodes]]
id = "sl"
op = "segment.slic"
inputs = { image = "ndwi.map" }
params = { size = 6 }
note = "9,398 superpixels."

[[nodes]]
id = "sd"
op = "segment.spectral_difference"
inputs = { objects = "sl.objects", image = "ndwi.map" }
params = { max_difference = 0.1 }
note = "554 objects."

[[nodes]]
id = "owater"
op = "objects.rules"
inputs = { objects = "sd.objects" }
params = { classes = [{ name = "water", rule = "above(mean_image, 0, 0.13)" }, { name = "land", rule = "below(mean_image, 0, 0.13)" }] }
note = "A ramp around 0.065: membership 0.5 there."
```

In the rules, `mean_image` is the mean NDWI of the object (the column is named after the band of the NDWI map);
`above(mean_image, 0, 0.13)` is 0 at an NDWI of 0 or less, 1 at 0.13 or more and 0.5 at 0.065, Otsu's cut, so an object
of mixed water and land gets a membership between the two classes instead of a hard answer.

```tfslider
label: Largest difference allowed
caption: "SLIC superpixels of the NDWI (size 6, 9,398 objects) merged by spectral difference, over the same part of the clip. The chart counts the objects of the whole clip."
frames:
  - { value: 0.02, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/sd_0_02.png, caption: "max_difference 0.02", stats: { Objects: "4,125" } }
  - { value: 0.05, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/sd_0_05.png, caption: "max_difference 0.05", stats: { Objects: "1,859" } }
  - { value: 0.1, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/sd_0_1.png, caption: "max_difference 0.1", stats: { Objects: "554" } }
  - { value: 0.2, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/sd_0_2.png, caption: "max_difference 0.2", stats: { Objects: "169" } }
  - { value: 0.4, src: /img/tutorials/object-based-image-analysis/water-by-thresholds/sd_0_4.png, caption: "max_difference 0.4", stats: { Objects: "25" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/water-by-thresholds/spectral_difference.csv
  x: max_difference
  y: [objects]
  labels: { objects: "Objects" }
  ylabel: "Objects in the clip"
```

At 0.02 the lake is still in pieces and the town is a mosaic of roofs; at 0.1 the lake is a single object with the
ponds, the shore is where it was, and the town's blocks are patches of similar tone; at 0.4 the whole land is a few huge
objects. The pitfall to know is **chaining**: a gradual change from one object to the next is merged step by step, so a
limit large against the contrast you want to keep joins things that are far apart in tone at the two ends of a ramp. Keep
the limit small against the gap between your classes (here the gap is above 1, so 0.1 is safe), and use a segmenter that
keeps edges for the objects it starts from, not single cells.

With the limit at 0.1 the objects classed as water cover **222.4 ha** in 58 objects, against 221.1 ha in 11 for the
pixels. The two maps differ in 346 of the 336,400 cells (0.1 %), at the shore. For the lake itself the second way gives
nothing more than the first. What it adds is that every object, water or land, has a mean, a size and a shape, and
that the rule can ask for more than a threshold, as the next tutorials do.

```tftable
sortable: true
align: { method: left }
digits: { cut: 3, water_ha: 1, water_objects: 0 }
labels: { method: Method, cut: "NDWI cut", water_ha: "Water area (ha)", water_objects: "Water objects" }
caption: "Four ways to the water of the clip. The first three cut pixels (segment.threshold); the last classes objects. The MNDWI is (green - swir16) / (green + swir16) and has its own Otsu cut."
csv: |
  method,cut,water_ha,water_objects
  "NDWI above 0 (given)",0,221.5,17
  "NDWI, Otsu",0.061,221.1,11
  "MNDWI, Otsu",-0.014,215.0,147
  "NDWI of SLIC + spectral difference 0.1, ramp 0 to 0.13",0.065,222.4,58
```

## Other classes and other indices

`classes = 3` asks Otsu for two thresholds. On the NDWI of this clip they are **−0.467 and 0.152**, which gives three groups:
the lowest NDWI (66 % of the cells, where the near infrared is far above the green: the gallery forest and the trees and
gardens among the houses), the middle (27 %, the roofs, the streets and the bare ground) and the highest (the water, 21,865
cells, 219 ha, a little less than with two classes because its cut is higher). It is not a land cover map (the NDWI was made
for water), but it shows what the multi-level version does.

![Three classes by Otsu](/img/tutorials/object-based-image-analysis/water-by-thresholds/three_classes.png "Three classes by multi-level Otsu on the NDWI: the lowest NDWI in green, the middle in tan, the highest (water) in blue, over the same part of the clip as the slider.")

The index is a choice, too. The **MNDWI** of Xu (2006) uses the short-wave infrared in the place of the near infrared, which the
clip has as `swir16`; on this clip Otsu puts its cut at −0.014 and finds 215.0 ha, 3 % less than the NDWI, in 147 pieces.
Neither is "right" without a reference: compare them with a map you trust, with a few points you checked on the image, or with
the accuracy methods of part 5.

## Common pitfalls

- **There is no water in the scene.** Otsu always splits the histogram in two, so it will split dry ground. In the 100 × 100
  window of the cerrado of part 3, where the NDWI runs only from −0.75 to −0.40, the cut falls at −0.567 and 61.7 % of the
  window becomes "water". The figure shows it. Check that the scene has water (the histogram has two modes), or give the
  `thresholds` yourself.

  ![Otsu in a window with no water](/img/tutorials/object-based-image-analysis/water-by-thresholds/cerrado.png "A 1 km window of dry cerrado (left) and what Otsu on its own NDWI calls water (right, in blue): 61.5 % of the cells, the ones with the highest NDWI of the window, while the darker vegetation stays land.")

- **Shadows and dark surfaces.** A deep shadow or a dark roof can have a high NDWI because both the green and the NIR are low.
  If the map has water where the town has tall buildings or the hills have shade, mask the shadows or add a second condition
  (a minimum area, with [[objects.remove_small]]).
- **Clouds.** Sentinel-2's `mask = "auto"` in [[data.stac]] drops clouds and their shadows when a scene has them.
- **Small things.** A thresholded map is full of tiny objects (the 1-cell ponds above); decide a smallest size you report and
  merge or drop the rest.
- **Mixed pixels and narrow rivers.** A stream narrower than a cell (10 m) does not make a water cell. Use a finer image or
  the other tutorials' methods, not a threshold.
- **Names of the bands.** The NDWI expression reads `s.green` and `s.nir`: with other names it fails. In [[objects.rules]] the
  class names are the ones of the list, and the numbers of the map follow the list's order, not those of the threshold.
- **One date.** The water of one day is the water of that day (a lake in the dry season, a flood in the wet one). Say the
  date, and use the same date of every year to compare years.

## Going further

- Before this: [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/), the scale of a segmentation;
  and [What an object is](/tutorials/object-based-image-analysis/what-is-an-object/), with the threshold among five
  segmenters.
- For a whole map, not only water: the classification tutorials of the series come next (samples, then rules).
- The operators of this page: [[segment.threshold]], [[segment.spectral_difference]], [[segment.slic]], [[objects.rules]],
  [[objects.to_vector]], [[vector.write]] and [[raster.calc]].
- Coming from eCognition: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/) matches its
  multi-threshold and spectral difference segmentations to these operators.

## References

- Achanta, R., Shaji, A., Smith, K., Lucchi, A., Fua, P., & Süsstrunk, S. (2012). SLIC superpixels compared to
  state-of-the-art superpixel methods. *IEEE Transactions on Pattern Analysis and Machine Intelligence*, 34(11),
  2274-2282. https://doi.org/10.1109/TPAMI.2012.120
- Liao, P.-S., Chen, T.-S., & Chung, P.-C. (2001). A fast algorithm for multilevel thresholding. *Journal of Information
  Science and Engineering*, 17(5), 713-727.
- McFeeters, S. K. (1996). The use of the Normalized Difference Water Index (NDWI) in the delineation of open water
  features. *International Journal of Remote Sensing*, 17(7), 1425-1432. https://doi.org/10.1080/01431169608948714
- Otsu, N. (1979). A threshold selection method from gray-level histograms. *IEEE Transactions on Systems, Man, and
  Cybernetics*, 9(1), 62-66. https://doi.org/10.1109/TSMC.1979.4310076
- Xu, H. (2006). Modification of normalised difference water index (NDWI) to enhance open water features in remotely
  sensed imagery. *International Journal of Remote Sensing*, 27(14), 3025-3033.
  https://doi.org/10.1080/01431160600589179
