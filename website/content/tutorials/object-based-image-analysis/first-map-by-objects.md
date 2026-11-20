---
title: Your first map by objects in 10 minutes
short: Your first map by objects
description: "Open a template that cuts an image into objects, click a few objects of each class in the Viewer, and read the map of classes, its accuracy and its polygons; then put your own image in its place."
order: 10
level: beginner
time: 10 min
template: Classify an image by clicking samples
ops: [segment.multiresolution, objects.features, objects.samples, objects.classify, objects.to_vector, objects.sample, objects.accuracy, ml.fit, vector.write]
series: Object-based image analysis
part: 1
---

A satellite image is a grid of pixels, but the things you want to map (a lake, a forest stand, a field, a town) are
made of many pixels that look alike. **Object-based image analysis** (OBIA) starts by grouping the pixels into
**objects**, patches of neighbouring pixels with similar colours, and then classifies the objects instead of the
pixels. In this tutorial you will do it from start to end on an example image that comes with a template: look at the
objects, tell the program what a few of them are by clicking on them, read the map it makes, check how good the map
is, and take the polygons to a GIS. At the end, you will do the same with an image of your own. This is the first part of a series: each later part takes
one of these steps and goes deeper (the list is at the end).

## The idea

Pixel-by-pixel classification gives maps with a salt-and-pepper look: a roof is a mix of bright and dark pixels, a
field has a few pixels that look like bare soil. Objects smooth that out, and they also have properties a single pixel
does not: a size, a shape, a texture, the average colour of all of their pixels, the neighbours around them.

The flow of this tutorial has five steps, and each is a node of the model:

1. **Segment**: cut the image into objects ([[segment.multiresolution]], the method of Baatz and Schäpe, 2000, used
   by eCognition).
2. **Describe**: measure each object ([[objects.features]]): the mean and the standard deviation of each band, NDVI,
   area, compactness and texture.
3. **Give examples**: say what a few objects are, by their class: *water*, *forest*, *urban*... These are the
   **samples**.
4. **Classify**: a random forest, a method that learns from the examples which attributes tell the classes apart,
   gives every object a class ([[ml.fit]] learns, [[objects.classify]] applies).
5. **Check and export**: take a fresh sample of objects, tell what they really are, and measure the accuracy
   ([[objects.sample]], [[objects.accuracy]]); write the polygons ([[objects.to_vector]]).

## What you need

Nothing but Terraflux: the template draws its own image, a synthetic scene of six bands (blue, green, red, near
infrared and two short-wave infrared) with five classes, so the answers are known. The last section shows how to
replace it with an image of yours: a file with a layer per band.

The scene is 120 by 120 cells. The first picture below shows it in true colour and in false colour (near
infrared, red and green, in which living vegetation is red), and the map of classes it was drawn from. That map is
the truth, which a real image never gives you: here it lets us tell how good the result is.

![The synthetic image and its classes](/img/tutorials/object-based-image-analysis/first-map-by-objects/image.png "The scene of the template: true colour, false colour, and the five classes it was drawn from. The edges between classes are blurred and every band has noise, as in a real image.")

## Open the template and run it

In the application, open **Templates > Objects (OBIA) > Classify an image by clicking samples** (or **Help >
Tutorials with a model** and choose this tutorial). A new model opens with these nodes:

```tfgraph
#! caption: The template: an image (here drawn by chance), its objects, their attributes, the samples and the classes.
#! fragment: true
#! highlight: segments, features, classify
[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "image.image" }

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image" }

[[nodes]]
id = "training"
op = "objects.samples"
inputs = { objects = "features.objects", samples = "samples.vector" }

[[nodes]]
id = "model"
op = "ml.fit"
inputs = { table = "training.table" }
params = { column = "class", method = "random_forest", seed = 1 }

[[nodes]]
id = "classify"
op = "objects.classify"
inputs = { objects = "features.objects", model = "model.model", samples = "training.table" }
```

The model also has nodes for the example points (`sample_table`, `samples`), the polygons (`polygons`, `write`) and a
Plot (`plot`). Click the last node, `plot`, and press ++ctrl+enter++ (**Run to here** in the Model pane's bar): the
nodes it needs run, and the Viewer shows the map of classes.

## Look at the objects

Click the node `segments`. The Viewer shows the objects, each outlined, and says how many there are: 116 in this
scene. In the controls
beside the map, **Fill** chooses how they are coloured (*no fill*, *by class* once they have classes, or *a colour
each*), and **Outline** and *width* the lines.

![The objects over the image](/img/tutorials/object-based-image-analysis/first-map-by-objects/objects.png "The 116 objects of the template, outlined over the image in true colour. An object stops where the colour changes; a big uniform area (the town in the middle) is cut into a few large objects.")

- Move the pointer over the map: the object under it is told and outlined.
- Click an object to select it. Below the map the attribute table has a row per object, and the selected object's
  row is picked. The **inspector** in the controls tells its attributes. Press ++n++ to add its neighbours, ++esc++
  to let go.
- Click `features` and look at the table: the columns are what the classifier will read.

![One object and its neighbours](/img/tutorials/object-based-image-analysis/first-map-by-objects/selected.png "An object selected (orange) and, after N, its neighbours (blue), drawn here as the Viewer draws them. The inspector lists the attributes of the selected object.")

The size of the objects comes from the **scale** of the segmentation. The template gives none, so
[[segment.multiresolution]] takes the first scale that [[segment.scale]] suggests for the image (the `scale` output
says which one: 1.92 here). Select `segments`, set `scale` in the Node pane to a larger value, run again, and the
objects are larger and fewer; to a smaller one, and they are smaller and more numerous. Move the slider to see it
on this scene, with the scale from half to four times the suggestion.

```tfslider
label: Scale (times the suggested 1.92)
caption: "The same image segmented with the scale set to 0.5, 1, 2 and 4 times the one that segment.scale suggests (shape 0.1 and compactness 0.5, the node's defaults). Each is a run of segment.multiresolution."
frames:
  - { value: 0.5, src: /img/tutorials/object-based-image-analysis/first-map-by-objects/scale_0.5.png, caption: "Scale 0.96", stats: { Objects: "401" } }
  - { value: 1, src: /img/tutorials/object-based-image-analysis/first-map-by-objects/scale_1.png, caption: "Scale 1.92 (the suggestion)", stats: { Objects: "116" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/first-map-by-objects/scale_2.png, caption: "Scale 3.85", stats: { Objects: "30" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/first-map-by-objects/scale_4.png, caption: "Scale 7.7", stats: { Objects: "17" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/first-map-by-objects/scales.csv
  x: factor
  y: [objects]
  ylabel: Objects
```

At half the scale the town is broken into pieces; at four times it, the image has 17 objects and some of them hold
two classes. The suggestion sits between.

!!! tip
    Objects should be about the size of the things you want to map: smaller than a forest stand, a few times larger
    than a single pixel. If one object holds two classes, lower the scale; if a field is broken into many pieces,
    raise it. The scale is in the image's units, so an image of reflectance from 0 to 1 and one from 0 to 10000 need
    different scales: leave `scale` empty and the suggestion follows the image.

## Click the samples

The template comes with 38 example points. Replace them with yours, by clicking on the objects:

<div class="steps" markdown>

1. Click the node `features` (the objects the node `training` reads), and press **Run to here**: the palette needs
   the classes of the model.
2. In the controls beside the map, tick **Samples**. A palette appears, a line per class (water, forest, grassland,
   cropland, urban) with the number of samples it has: a class with fewer than 10 is marked *few*.
3. Choose a class in the palette and **click** an object of that class on the map. Drag to paint several objects.
   A **right click** takes a sample away. A sample is a point inside the object (near its centre), so it still
   points to the same place if you segment the image again.
4. Give each class at least 10 objects, in different places and looks: dark and bright forests, a river and a
   pond. To add a class, type its name in the field *new class* and press **Add**.
5. Press **Apply**: the samples are written into the model, as one step of undo. With **Live** ticked they are
   written after a short pause, as you click.

</div>

![The template's samples on the objects](/img/tutorials/object-based-image-analysis/first-map-by-objects/samples.png "The template's 38 example points, a coloured dot on an object each, over the objects: eight per class, six of grassland. The palette would mark each class *few*.")

Apply adds two nodes to the model, a table of your points and the points made from it, and connects them to the node
`training`, in place of the template's own points. Run to `plot` again: the map has changed with your examples.
Click `classify` and set the **Fill** to *by class*: each object has the colour of its class, and the inspector tells
the probability of each class and the **margin** (the largest probability minus the next). Where the margin is low,
the forest hesitates: those are the objects where one more sample helps most.

![The map of classes](/img/tutorials/object-based-image-analysis/first-map-by-objects/classes.png "Left: the classes the forest gave the objects, from the template's 38 points. Right: the classes the scene was drawn from. 93.8 % of the cells have the right class; the mistakes are along the borders and in a few small objects.")

Here the forest knows the truth, so it can be counted: 93.8 % of the 14,400 cells of the map have the class the
scene was drawn from. In your own image you will not have that number, which is why the next section exists.

![The margin of each object](/img/tutorials/object-based-image-analysis/first-map-by-objects/margin.png "The margin of every object: yellow where one class is far ahead, dark where two are close. The dark objects are slivers along the borders between classes, where the blur of the image mixes two classes.")

The margin map says where to look. The thin dark objects follow the borders between two classes: their colours
are a mix of both, and neither class owns them. They are the first place to add samples, or to ask whether a
smaller scale would give them cleaner neighbours.

## Check the map

A map without an accuracy is an opinion. To measure it, you need objects whose true class you know that the
classifier did **not** learn from. [[objects.sample]] draws them: a stratified random sample, the same chance for
every object of a class, which is what the accuracy formulas of Olofsson et al. (2014) assume.

```tfgraph
#! caption: A sample of the classified objects, written for you to label, then read back and compared with the map.
#! fragment: true
#! highlight: check, accuracy
[[nodes]]
id = "check"
op = "objects.sample"
inputs = { objects = "classify.objects" }
params = { per_class = 50, seed = 7 }

[[nodes]]
id = "check_file"
op = "vector.write"
inputs = { vector = "check.points" }
params = { path = "check_points.gpkg" }

[[nodes]]
id = "labelled"
op = "vector.read"
params = { path = "check_points_labelled.gpkg" }

[[nodes]]
id = "accuracy"
op = "objects.accuracy"
inputs = { objects = "classify.objects", samples = "labelled.vector" }
params = { class = "reference" }
```

<div class="steps" markdown>

1. Add the node `check` ([[objects.sample]]) after `classify` and the node `check_file` ([[vector.write]]). Run
   `check_file`: the points are written beside the model.
2. Open `check_points.gpkg` in any GIS, for example QGIS. Each point is an object, with its column `mapped` (the
   class the map gave it) and an empty column `reference`. Fill `reference` with the true class of each (from the
   image, a field visit or a better map) and save the file as `check_points_labelled.gpkg`. Use the same class names
   as the map.
3. Add [[vector.read]] of the labelled file (`labelled`) and [[objects.accuracy]], with the points on `samples` and
   `class = "reference"`.
4. Run `accuracy`.

</div>

With the template's 116 objects, no class has 50 of them, so [[objects.sample]] gives every object of each class: the
sample is the whole map. That is a limit of this small example, and it has a cost: the 38 objects the forest learned
from are in it too. Labelled from the scene's truth, the sample gives the matrix below; leave the 38 training objects
out and the forest is right on 61 of the other 78 objects (78 %), a little worse than the matrix says. On a real image,
with hundreds of objects per class, a sample of 50 holds few of the objects you clicked.

```tftable
data: /data/tutorials/object-based-image-analysis/first-map-by-objects/matrix.csv
labels: { map: "Map \\ reference" }
heat: true
digits: 0
align: { map: left }
caption: "The confusion matrix of the 116 objects of the template (every object, since no class has 50), labelled from the scene's truth: the class the map gave (rows) against the true class (columns). 95 are on the diagonal, 81.9 %; kappa 0.77."
```

The outputs are tables: the **confusion matrix** (the map's classes in rows, the reference's in columns), the user's
and producer's accuracies of each class, the overall accuracy and kappa, and, with the area estimates of Olofsson et
al. (2014), the area of each class with its confidence interval. Fifty objects per class is the least they advise
(a class with fewer is warned about). Never use the samples you clicked to check: the forest learned them, so they
are right more often than the rest of the map.

!!! note
    The example image is drawn from a map of classes, the node `land`, so there the truth is known. With an image of
    yours it is not, and the sample of objects labelled by you is the way to know.

The matrix is read row by row: of the 32 objects the map called grassland, 20 are grassland, 5 are cropland, 3 forest,
3 water and 1 urban. Those objects are mostly on the borders, which the margin map showed. The other tables hold the user's and
producer's accuracies and the areas with their confidence intervals. To report an accuracy, give the overall number
with its interval, the matrix, and how the sample was drawn.

## Take the polygons

The template already ends in two nodes: `polygons` ([[objects.to_vector]], with `smooth = 1`, which cuts the stair
corners once) and `write` ([[vector.write]]), which writes `objects_classified.gpkg` beside the model: a polygon per
object, with its class and its attributes in the table. Open it in QGIS or ArcGIS. Keep `smooth` at 0 when you will
count or measure the polygons, since smoothing moves the edges a little. To have the map of classes as a raster
instead, [[objects.to_map]] gives one number per class (`what = "class"`).

## Do it with your image

**File > Use my data...** puts your files in place of the data the template draws.

<div class="steps" markdown>

1. Open the template, then **File > Use my data...**. On the page *Your data*, the row of the image (the node `image`)
   has **Choose a file...**: choose your image, a file with a layer per band.
2. The layers of the image should be named for the bands they are (`blue`, `green`, `red`, `nir`, `swir1`, `swir2`
   where your image has them): the NDVI and NDWI attributes find the bands by these names. If your layers are named
   otherwise, give `red`, `nir` and `green` in the settings of `features`.
3. Go through the pages with **Next** and press **Apply** on the last (tick **Run it now** to run the model once its
   data is read).
4. The example points belong to the example image: they mean nothing on yours. Click your own samples as above
   and **Apply** them.

</div>

Things to watch with real images:

- **Scale of the values.** Reflectance from 0 to 1, or digital numbers from 0 to 10000? Leave the segmentation's
  `scale` empty so that it follows the image.
- **Few samples.** Under 10 per class the palette says *few*: add more, in different looks of the class.
- **Coordinate system.** The polygons carry the image's, so an image without one gives polygons that a GIS cannot
  place.
- **Big images.** [[segment.multiresolution]] cuts a large image into tiles (`tile` and `margin`) and reads big
  images in windows from disk.

## Going further

This is part 1 of the series *Object-based image analysis*. The next parts take each step of this tutorial on a real
Sentinel-2 image of the edge of Lago Paranoá, Brasília:

- **Part 2**, what the objects are and how they are made, with five segmenters: [What an object is: segmentation](/tutorials/object-based-image-analysis/what-is-an-object/).
- **Part 3**, how many objects: [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/).
- **Part 4**, the attributes the classifier reads: [Describing objects](/tutorials/object-based-image-analysis/describing-objects/).
- **Part 5**, your clicked samples, the nearest neighbour and the accuracy in depth: [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/).
- **Part 6**, classes by rules instead of examples: [Rules and class trees](/tutorials/object-based-image-analysis/rules-and-class-trees/).
- **Part 7**, water with no samples: [Water by thresholds](/tutorials/object-based-image-analysis/water-by-thresholds/).
- **Part 8**, several levels of objects: [Levels](/tutorials/object-based-image-analysis/levels/).
- **Part 9**, classes with no samples at all: [Classes without samples](/tutorials/object-based-image-analysis/classes-without-samples/).
- **Part 10**, correcting objects by hand: [Editing by hand](/tutorials/object-based-image-analysis/editing-by-hand/).
- **Part 11**, change between two dates: [Change by objects](/tutorials/object-based-image-analysis/change-by-objects/).

And, outside the series:

- Classes by rules, for those who know eCognition: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/).
- A different method for each class: [Coming from InterIMAGE](/tutorials/object-based-image-analysis/from-interimage/).
- Other segmentations ([[segment.slic]], [[segment.watershed]], [[segment.meanshift]], [[segment.region_growing]], [[segment.quickshift]]), and the guide `objects` of the
  MCP server, which an assistant can follow to build the model with you.

## References

- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality multi-scale
  image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Drăguţ, L., Tiede, D., & Levick, S. R. (2010). ESP: a tool to estimate scale parameter for multiresolution image
  segmentation of remotely sensed data. *International Journal of Geographical Information Science*, 24(6), 859-871.
  https://doi.org/10.1080/13658810903174803
- Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for
  estimating area and assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42-57.
  https://doi.org/10.1016/j.rse.2014.02.015
