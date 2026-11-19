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
is, and take the polygons to a GIS. At the end, you will do the same with an image of your own.

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

Click the node `segments`. The Viewer shows the objects, each outlined, and says how many there are. In the controls
beside the map, **Fill** chooses how they are coloured (*no fill*, *by class* once they have classes, or *a colour
each*), and **Outline** and *width* the lines.

- Move the pointer over the map: the object under it is told and outlined.
- Click an object to select it. Below the map the attribute table has a row per object, and the selected object's
  row is picked. The **inspector** in the controls tells its attributes. Press ++n++ to add its neighbours, ++esc++
  to let go.
- Click `features` and look at the table: the columns are what the classifier will read.

The size of the objects comes from the **scale** of the segmentation. The template gives none, so
[[segment.multiresolution]] takes the first scale that [[segment.scale]] suggests for the image (the `scale` output
says which one). Select `segments`, set `scale` in the Node pane to a larger value, run again, and the objects are
larger and fewer; to a smaller one, and they are smaller and more numerous.

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

Apply adds two nodes to the model, a table of your points and the points made from it, and connects them to the node
`training`, in place of the template's own points. Run to `plot` again: the map has changed with your examples.
Click `classify` and set the **Fill** to *by class*: each object has the colour of its class, and the inspector tells
the probability of each class and the **margin** (the largest probability minus the next). Where the margin is low,
the forest hesitates: those are the objects where one more sample helps most.

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

The outputs are tables: the **confusion matrix** (the map's classes in rows, the reference's in columns), the user's
and producer's accuracies of each class, the overall accuracy and kappa, and, with the area estimates of Olofsson et
al. (2014), the area of each class with its confidence interval. Fifty objects per class is the least they advise
(a class with fewer is warned about). Never use the samples you clicked to check: the forest learned them, so they
are right more often than the rest of the map.

!!! note
    The example image is drawn from a map of classes, the node `land`, so there the truth is known. With an image of
    yours it is not, and the sample of objects labelled by you is the way to know.

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

- What the objects are and how they are made, on a real image: [What an object is: segmentation](/tutorials/object-based-image-analysis/what-is-an-object/).
- Classes by rules instead of examples: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/).
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
