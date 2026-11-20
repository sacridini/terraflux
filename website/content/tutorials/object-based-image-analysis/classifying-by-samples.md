---
title: "Classifying by samples: nearest neighbour, random forest and accuracy"
short: Classifying by samples
description: "Tell Terraflux what a few objects of the Brasilia image are, let a nearest-neighbour rule and a random forest classify the rest, and measure the result with an error matrix and area-adjusted accuracies; and see why samples that lie side by side flatter the map."
order: 50
level: intermediate
time: 75 min
template: Classify an image by clicking samples
ops: [data.stac, segment.multiresolution, objects.features, table.read, table.of, vector.from_table, objects.samples, ml.fit, objects.classify, objects.nearest, objects.accuracy, objects.sample, objects.to_map]
series: Object-based image analysis
part: 5
---

You have objects and you have measured them. Now you name them. In this tutorial you give Terraflux a few objects of
each class of the Sentinel-2 image of Brasília (water, forest, cerrado, urban, bare soil), and two classifiers learn
from them: a **nearest-neighbour** rule, the one eCognition made popular, and a **random forest**. Each gives every
object a class. Then you measure how good the map is, honestly: with objects that were *not* used for training, an
error matrix, and area estimates that take the errors into account. At the end you will know how to give samples in
the Viewer, how to build the same model by hand, how to read an accuracy table, and one way in which samples
flatter the result without you noticing.

## The idea

A **supervised classification** has three parts: examples, a rule learned from them, and a check.

1. The **samples** (or training samples) are objects whose class you know. You give them by clicking: "this object
   is water, that one is forest". Each sample brings its row of attributes (part 4), and these rows are the
   *training table*.
2. A **classifier** learns from the table how the attributes differ from class to class, and applies what it has
   learned to every other object. Two classifiers are on this page:
    - The **nearest neighbour** classifies an object like its closest samples. The distance is measured in the
      feature space of part 4, with each attribute divided by its standard deviation over all the objects, so that
      a band in reflectance and an index from -1 to 1 count alike. Terraflux's [[objects.nearest]] follows eCognition's
      *standard nearest neighbour*: the membership of a class is \( e^{-k d^2} \) where \( d \) is the distance to the nearest sample of that
      class, \( k = \ln(1/\text{slope}) \), so that the membership is 1 on a sample and equals `slope` (0.2 by default)
      at a distance of 1. The object takes the class of the largest membership, and stays **unclassified** when even
      that is under `min` (0.1 by default).
    - The **random forest** (Breiman, 2001) grows many decision trees, each on a random draw of the samples and of the
      attributes, and lets them vote. The share of trees that vote for a class is its **probability**. Terraflux
      fits it with [[ml.fit]] (the same algorithm as scikit-learn's `RandomForestClassifier`) and applies it with
      [[objects.classify]]. Forests are a common choice for object-based classification (Belgiu & Drăguţ, 2016; Ma et al., 2017).
3. The **accuracy assessment** compares the map with a *reference*: objects whose true class you know, that the classifier
   never saw.

!!! science "Key idea"
    A classifier is always right about its training samples, so **accuracy has to be measured on other objects**. The
    **error matrix** (or confusion matrix) counts, for the reference objects, the class the map gave (rows) against the
    true class (columns). From it:

    - the **overall accuracy** is the share of reference objects on the diagonal;
    - the **user's accuracy** of a class is the share of the objects *mapped* to it that really are of it (the
      reliability of the map: if you walk to a place the map calls forest, how often is it forest?);
    - the **producer's accuracy** of a class is the share of the objects that are *really* of it that the map found
      (its completeness: how much of the forest did the map get?).

    These counts treat every object alike, but objects differ in size, and the classes differ in how much of the map they
    cover. Olofsson and colleagues (2013, 2014) show how to correct that: take the classes of the map as **strata**
    of area \( A_i \) (\( W_i = A_i / A \) of the total \( A \)), count in each stratum \( i \) the sampled objects of
    each true class \( j \), \( n_{ij} \) of the \( n_{i\cdot} \) sampled, and estimate the share of the area that
    is of class \( j \) as

    \[ \hat p_{\cdot j} = \sum_i W_i \, \frac{n_{ij}}{n_{i\cdot}}, \qquad \hat A_j = A\,\hat p_{\cdot j}. \]

    The areas \( \hat A_j \) come with confidence intervals, and are the numbers to report: they say how much of the
    landscape is of each class, *with* the map's mistakes taken out, which the map's own area never does. Terraflux's
    [[objects.accuracy]] makes all of these.

### Where it comes from

Supervised classification of remote-sensing images is older than objects. Congalton (1991) reviewed how to assess the
accuracy of such maps and fixed the vocabulary (the error matrix, user's and producer's accuracy) that you still read in
papers. The "good practices" of Olofsson et al. (2014) set out how to estimate areas, and their uncertainty,
from a probability sample of reference data, instead of counting the cells of a map. The random forest
(Breiman, 2001) became the default classifier of remote sensing in the 2010s; the nearest neighbour, with
fuzzy memberships, is the heart of eCognition's classification and still the most used by those who come from it. A
classifier tells you nothing about *why*; the attributes you give it (part 4) are what it can learn from.

## When to use it

Classification by samples is the right tool when:

- you know the classes you want, and can recognise them on the image (or in the field);
- the classes differ in some attributes of the objects, and you do not know which, or the difference is a mix of several;
- you want a map with a number for its accuracy.

Rules (part 6) are a better choice when you know *why* a class differs (water has a high NDWI) and want a transparent,
portable definition, and when you have no samples. Without any samples and classes, group the objects by their
attributes alone (part 9). In either case, an accuracy assessment is still needed.

## What you need

- **The objects and their attributes** of part 4: the Sentinel-2 clip of Lago Paranoá, cut by [[segment.multiresolution]]
  at the scale 2 into 3,035 objects, with [[objects.features]] to describe them.
- **Training samples**: points, or polygons, each with a class name, in any coordinate system. In the Viewer you make
  them by clicking.
- **Reference samples**, that were not used for training, to assess the map. A real study draws them with
  [[objects.sample]] and labels them from better data than the image (see the section on the reference).
- **A template to start from**: *Classify an image by clicking samples* is a small model, with a synthetic image, that
  has this whole chain; the page builds the same chain on the real image.

!!! note "The samples of this page"
    A figure cannot be clicked. The 93 training objects and the 80 reference objects of this page (downloads:
    [training_samples.csv](/data/tutorials/object-based-image-analysis/classifying-by-samples/training_samples.csv),
    [reference_samples.csv](/data/tutorials/object-based-image-analysis/classifying-by-samples/reference_samples.csv)) are drawn by a
    script (`website/figures/object-based-image-analysis/obia_samples.py`) from places of the clip that were looked at
    in true and false colour, one object per point. They stand for what you do in the Viewer; they are **not field
    data**, and the labels are those of someone who looked at the image: some of them are wrong, as in any real project. The reference
    objects are in different places from the training ones (the map below shows both), but they are **not a
    probability sample**, so the confidence intervals of the area estimates on this page are indicative only. Read
    the numbers as an example of how to read an accuracy table, not as the accuracy of a land-cover map of Brasília.

![The samples over the clip](/img/tutorials/object-based-image-analysis/classifying-by-samples/samples.png "The 93 training objects (circles) and the 80 reference objects (squares) over the clip, coloured by class. The reference objects are in other places from the training ones.")

## Give samples by clicking

In the application, samples are given in the Viewer, on the layer of objects, with the **Samples** tool:

<div class="steps" markdown>

1. Run the node that makes the objects (here, the one with the attributes), and select its layer *objects*.
2. In the panel beside the map, tick **Samples**. A palette of classes appears; type a class name in *new class* and
   press **Add** for each (water, forest, cerrado, urban, bare soil).
3. Choose a class in the palette and **click** an object: it gets that class. **Drag** over the map to paint
   several objects with the class; **right-click** an object to take its sample away. Each class shows how many samples it has, and a class
   with fewer than 10 is marked *few*.
4. Press **Apply**. The samples are written into the model as one step of undo. If the model has no classifier yet,
   Terraflux adds the chain: an [[objects.samples]] node, a [[ml.fit]] and an [[objects.classify]]. With **Live** ticked, they
   are written a moment after each click and the map follows.

</div>

A sample is a point at the object's interior cell nearest its centroid, so it survives another segmentation: if you
change the scale later, the point picks whichever object lies there. The points are written as a [[table.of]] with the
columns `x`, `y` and `class`, feeding a [[vector.from_table]] with the clip's coordinate system, which feeds
[[objects.samples]]. With more than 500 samples they go to a GeoPackage beside the model instead. The model below has the same
chain, except that it reads the points of the script from a CSV (the same columns) instead of a `table.of`. Give samples of different looks of each class (a
garden and a roof for urban, a shaded and a lit stand for forest): a few objects of each look teach more than many of one.

## Build the model

### Steps 1 to 3: the objects and their attributes

[[data.stac]], [[raster.reproject]] and [[raster.calc]] make the reflectance as in part 2; [[segment.multiresolution]]
cuts it at the scale 2; [[objects.features]] describes the objects. This time `features` is a list, to compute only
what the classifier reads: `mean` and `sd` of each band, the three indices, the texture (`contrast` and `homogeneity`, of
the brightness only, with `texture_bands`) and `compactness`. The short-wave infrared band is named for the NDMI, as in part 4.

```tfgraph
#! caption: Steps 1 to 3 (nodes 1 to 5): the scene, its reflectance, the objects and the attributes the classifier will read.
#! highlight: feat
#! numbered: true
#! fragment: true
[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }
note = "3,035 objects (nodes 1 to 3 as in part 2)."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "segments.objects", image = "refl.image" }
params = { features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"], texture_bands = ["brightness"], swir = "swir16" }
note = "Colour, texture and compactness."
```

### Step 4: the samples

[[objects.samples]] takes the objects and the points. A point gives the object under it; a **polygon** gives every
object with at least half of its cells inside it. An object picked with two different classes is left out, and so is a
point outside every object, with a warning. It gives three outputs: the training `table` (a row per sampled object, with
`class` and the attributes), `objects` (the same objects with the sampled ones classed, to see them) and `counts` (how many objects
and cells each class has). Classes are **numbered by their names in alphabetical order**: bare soil is 1, cerrado 2,
forest 3, urban 4, water 5. On this page's samples the counts are:

```tftable
data: /data/tutorials/object-based-image-analysis/classifying-by-samples/counts.csv
labels: { class: Class, objects: "Training objects", cells: "Cells" }
align: { class: left }
digits: 0
caption: "What objects.samples picked: the training objects of each class. Terraflux warns of the class under 10 objects: bare soil has 9."
```

### Step 5: the random forest

[[ml.fit]] learns from the table (`column = "class"`). Two settings matter here.

- **`columns`** names the predictors. Without it, every numeric column but the class is a predictor, and that includes
  `cells` and `area`, the size of the object, which the forest then uses. Choose the attributes you want it to read.
- **`n_estimators`** is the number of trees (100 by default; the page uses 300). More trees give a steadier result and cost time;
  the seed (`random_state`) is fixed, so the same model gives the same map every time.

[[objects.classify]] applies the fitted model to every object (the attributes are matched by the names of the
predictors, so the objects must come from [[objects.features]] with the same settings), and gives the objects with their
classes, a `table` (the class, the probability of each class and the **margin**: the largest probability minus the second)
and a `map` of the class numbers. Pass the samples' table to its `samples` input so the classes carry their names.
`min` leaves an object unclassified when its best probability is under it (0 by default: every object gets a class).

### Step 6: the nearest neighbour

[[objects.nearest]] takes the same objects and the same samples table. Its `columns` are the attributes of the feature space;
without them it uses the colours in both tables, the band means (`mean_<band>`), `brightness` and the indices. This is
eCognition's default, a rule that looks at colour. Two more settings, `slope` and `min`, are the ones of the formula above.

### Step 7: the reference and the accuracy

[[objects.accuracy]] compares a classified set of objects with the reference. The reference is `samples` (points or
polygons with the class, resolved to objects as [[objects.samples]] does) or `reference` (a table with the columns `id`
and `class`). Its setting `design` says how the reference was drawn: `objects` (each object of a class had the same chance,
as [[objects.sample]] draws them, or one point in each of a set of such objects: the adjusted estimates weight each sampled
object by its area) or `area` (points thrown at random over the map: each counts once). Set it: the page's model says
`design = "objects"`. It gives:

- `matrix`: the error matrix by objects, a row per class of the map and a column per reference class;
- `table`: a row per class, with the user's and producer's accuracies (adjusted, with confidence intervals) and the class's
  adjusted area in cells and in hectares, with its interval;
- `overall`: the overall accuracy by objects and adjusted, its confidence interval, and kappa;
- `errors`: the reference objects that the map got wrong.

In this model the reference points are a second table of points. In a real study you draw them from the map with
[[objects.sample]] (see below) and fill in their classes.

```tfgraph
#! caption: The whole model: objects, attributes, the training points (read from a CSV of x, y and class), a random forest and a nearest neighbour, and the accuracy of each against the reference points (a second CSV).
#! highlight: samples, fit, rf, nn, acc_rf, acc_nn
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
note = "3,035 objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "segments.objects", image = "refl.image" }
params = { features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"], texture_bands = ["brightness"], swir = "swir16" }
note = "Colour, texture and compactness."

[[nodes]]
id = "train_table"
op = "table.read"
params = { path = "training_samples.csv" }
note = "The 93 training points: x, y, class."

[[nodes]]
id = "train"
op = "vector.from_table"
inputs = { table = "train_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_samples.csv" }
note = "The 80 reference points."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "samples"
op = "objects.samples"
inputs = { objects = "feat.objects", samples = "train.vector" }
note = "Training objects, with their attributes."

[[nodes]]
id = "fit"
op = "ml.fit"
inputs = { table = "samples.table" }
params = { column = "class", columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast", "glcm_homogeneity", "sd_nir", "compactness"], n_estimators = 300 }
note = "A random forest."

[[nodes]]
id = "rf"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "fit.model", samples = "samples.table" }
note = "Class and probabilities."

[[nodes]]
id = "nn"
op = "objects.nearest"
inputs = { objects = "feat.objects", samples = "samples.table" }
note = "Nearest samples."

[[nodes]]
id = "acc_rf"
op = "objects.accuracy"
inputs = { objects = "rf.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "Error matrix, accuracies and areas."

[[nodes]]
id = "acc_nn"
op = "objects.accuracy"
inputs = { objects = "nn.objects", samples = "ref.vector" }
params = { design = "objects" }
```

=== "Application"

    Build the first five nodes, run the `feat` node and give the samples in the Viewer as described above; **Apply**
    writes `train_table`, `train`, `samples`, `fit` and `rf` for you. Add [[objects.nearest]] with the objects of
    `feat` and the `table` of `samples`, and [[objects.accuracy]] with the objects of the classifier and the reference
    points (a second table of points, or a vector file read with [[vector.read]]). Select `acc_rf` and press
    ++ctrl+enter++.

=== "Command line"

    Save the graph as `classify.toml` beside the two files of points,
    [training_samples.csv](/data/tutorials/object-based-image-analysis/classifying-by-samples/training_samples.csv)
    (93 rows) and [reference_samples.csv](/data/tutorials/object-based-image-analysis/classifying-by-samples/reference_samples.csv)
    (80 rows), each with the columns `x`, `y` (metres, EPSG:31983, the clip's system) and `class`. The model reads them with
    [[table.read]] and [[vector.from_table]], and gives the numbers of this page (80.0 %, kappa 0.74). The script that
    drew the points is `classifying-by-samples.py`, with `obia_samples.py`, in `website/figures/object-based-image-analysis/`.
    To run your own, replace the two `table.read` and `vector.from_table` pairs by a [[vector.read]] of your files (a GeoPackage of points with a column `class`):

    ```toml
    [[nodes]]
    id = "train"
    op = "vector.read"
    params = { path = "training_samples.gpkg" }
    ```

    and run `terraflux run classify.toml --threads 4 --out output`.

## Read the results

### The two maps

![Random forest and nearest neighbour maps](/img/tutorials/object-based-image-analysis/classifying-by-samples/maps.png "The part of the clip of the other tutorials: the objects over the true colours, and the maps of the random forest and of the nearest neighbour (scale 2, the samples of the previous figure). Unclassified objects are grey.")

Both maps find the lake and put the dry cerrado in the lower right. They disagree about most of the town. The forest classes the
gallery forest along the streams and the trees of the gardens; the nearest neighbour leaves 3.5 % of the cells
unclassified (grey; many of them along the shore of the lake, where the colour is neither water nor land) and
scatters the forest and bare soil classes over the neighbourhoods.

The forest of this page reads twelve attributes, and its importance table (the `importance` output of [[ml.fit]],
which adds up to 1) is led by the texture: `glcm_homogeneity` (0.185), `glcm_contrast` (0.122), `sd_nir` (0.101)
and `mean_green` (0.091). That is part 4's finding again: texture separates the town from the cerrado and the forest.
The nearest neighbour, in its default form, does not read the texture at all.

### How sure is each object

The `table` of [[objects.classify]] keeps the **probability of each class** and the **margin**, the best minus the
second best. A margin near 1 is an object the trees agree on; near 0, one that is almost a coin toss between two classes.

![The margin of the random forest](/img/tutorials/object-based-image-analysis/classifying-by-samples/margin.png "The random forest's classes (left) and the margin of each object (right: 1 the trees agree, 0 a tie between two classes). The lake and the cerrado are sure; the town is not.")

The lake and the cerrado are the sure ones. Of the 3,035 objects, 31 % have a margin below 0.3, mostly in the
neighbourhoods and at the edge of the gallery forest, where a garden full of trees is both urban and forest; the median
margin is 0.46. In the Viewer, the **Feature space** window lists the objects whose class is *least certain*
(*Most uncertain*): they are the ones where one more sample helps most.

### The error matrix

The reference objects, classed by the map. Rows are the class the forest gave, columns the true class; the diagonal is
right.

```tftable
data: /data/tutorials/object-based-image-analysis/classifying-by-samples/matrix_rf.csv
heat: true
digits: 0
align: { "Map \\ reference": left }
caption: "Random forest: the 80 reference objects by the class the map gave (rows) and their true class (columns). 64 are on the diagonal."
```

The overall accuracy is 64 of 80, **80.0 %** (kappa 0.74). The matrix tells where the rest went.

- **Water** and **cerrado** are mapped without a mistake (14 of 14 and 18 of 20: two forest objects are called cerrado).
- **Forest** is over-mapped: of the 24 objects the map calls forest, 14 are forest, 6 are really urban and 2 are water;
  the urban ones are the "garden full of trees" again.
- **Bare soil** is never found: no object is mapped to it among the reference ones, and the 6 reference objects of bare soil
  went to urban (4) and forest (2). Nine training objects, in three small patches, were too few to learn it.

The same for the nearest neighbour:

```tftable
data: /data/tutorials/object-based-image-analysis/classifying-by-samples/matrix_nn.csv
heat: true
digits: 0
align: { "Map \\ reference": left }
caption: "Nearest neighbour (its default attributes): the same reference objects. 42 are on the diagonal, 52.5 %."
```

### User's and producer's accuracy, and the areas

```tftable
data: /data/tutorials/object-based-image-analysis/classifying-by-samples/accuracy_rf.csv
labels: { class: Class, mapped_objects: "Sampled objects mapped to it", reference_objects: "Reference objects", users: "User's accuracy", users_ci: "± 95 %", producers: "Producer's accuracy", producers_ci: "± 95 %", area_ha: "Adjusted area (ha)", area_ha_ci: "± 95 % (ha)", map_ha: "Mapped area (ha)" }
align: { class: left }
digits: { mapped_objects: 0, reference_objects: 0, users: 2, users_ci: 2, producers: 2, producers_ci: 2, area_ha: 0, area_ha_ci: 0, map_ha: 0 }
caption: "Random forest: the area-adjusted user's and producer's accuracies of each class with their 95 % confidence intervals, the class's adjusted area and the area of the map. Bare soil has no sampled object mapped to it: its user's accuracy cannot be estimated (Terraflux says so in a warning)."
```

Read the table by class.

- **Cerrado**: user's accuracy 0.94 ± 0.14 and producer's 1.00. The map is reliable for cerrado, and finds all of it. The map has 753 ha of it;
  the adjusted estimate is 705 ± 107 ha.
- **Forest**: the user's accuracy is only 0.61 ± 0.31, the producer's 0.93 ± 0.14. The map finds the forest but also calls
  many other things forest, so its area, **1,117 ha, is too large: the adjusted estimate is 735 ± 361 ha**. That is the
  kind of correction the adjusted areas are for.
- **Urban**: 0.88 and 0.79, the map's 1,229 ha against an estimate of 1,373 ± 379 ha.
- **Water**: all the objects the map calls water are water (1.00), but a few real water objects are
  called forest, so the producer's accuracy is 0.74 ± 0.41 and the estimate, 259 ± 142 ha, is larger than the 191 ha of the map.
- **Bare soil**: mapped as 74 ha; the estimate is 218 ± 268 ha. The interval says the map knows almost nothing about this class.

The **overall** accuracy by objects is 80.0 %; the area-adjusted one is **81.1 % ± 12.6** (95 %). The adjusted one weighs each sampled object by its area and each class
by how much of the map it covers (`design = "objects"`: every object of a class had the same chance of being drawn); here it
ends close to the share of objects, with a wider interval. For
the nearest neighbour, the same reference gives an overall accuracy of 52.5 % (adjusted 50.7 ± 14.4 %, kappa 0.40).

!!! tip "What to report"
    In a paper: the error matrix by objects (the counts), the user's and producer's accuracies with their intervals, the
    overall accuracy, and **the areas adjusted, with their intervals** instead of (or next to) the areas of the map. Say
    how the reference was drawn and who labelled it. Do not report kappa alone: it is a function of the
    matrix and says nothing the matrix does not.

The `errors` table lists the 16 reference objects that the forest got wrong, with the object's number, the class
mapped and the true class. In the Viewer, each leads the map to the object: look at them, it is the cheapest way to see what the classifier
misses.

### The reference, in a real study

Every number above depends on the reference, and the reference must be made apart from the training samples, with a **probability sample**: every object of
a class has the same chance of being drawn. [[objects.sample]] draws it from the classified objects, stratified by class:
`per_class` objects of each class, or `n` in all, shared equally. Olofsson et al. (2014) advise at least 50 per class. On the
forest of this page, `per_class = 20` gives:

```tftable
data: /data/tutorials/object-based-image-analysis/classifying-by-samples/draw.csv
labels: { class: Class, objects: "Objects of the class", sample: "Drawn" }
align: { class: left }
digits: 0
caption: "objects.sample with per_class = 20 on the forest's map: 20 objects of each class, whatever the class's size. Bare soil has 71 objects, urban 1,394."
```

The node gives a `points` output: a point at the cell nearest each object's centroid that is in the object, with the object's `id`,
its `mapped` class and a column `reference`, empty. You write the point layer (a GeoPackage), open it in a GIS or in
the application's attribute table, and fill `reference` with the true class, from an image of higher resolution, from the
field, from a photo interpretation, never from the same image and the same eyes as the training. Then you read the layer back
and give it to [[objects.accuracy]] with `class = "reference"`. The page does not do that here because it has no better source than the
image.

## Samples that lie side by side

A map can be accurate on paper and wrong in the places you did not sample. The reason is **spatial autocorrelation**: things near each other
look alike. Objects in the same patch of cerrado have the same colour and texture, so a sample picked among them is a copy
of its neighbour. If the reference is made of neighbours of the training objects, the classifier meets again the very
cases it learned, and the accuracy is flattering. This is a known problem in ecological and remote-sensing mapping
(Roberts et al., 2017; Ploton et al., 2020).

The experiment: one place for each of four classes (water, forest, cerrado, urban: bare soil is too small), 122 objects
in all, picked by the script. Half of each class, at random, trains a forest. The other half is used as a reference, **next to the
training objects**, and the accuracy is measured. Then the same forest is assessed on the 74 reference objects of other
places, the ones in the map above. Six random splits:

```tfchart
type: bar
data: /data/tutorials/object-based-image-analysis/classifying-by-samples/autocorrelation.csv
x: split
y: [neighbours, elsewhere]
labels: { neighbours: "Tested on neighbours of the training objects", elsewhere: "Tested in other places" }
colors: { neighbours: "#e8901f", elsewhere: "#3b8a5a" }
ylabel: "Overall accuracy by objects"
height: 320
caption: "The same forest, six random splits of the same samples. Tested on objects beside the ones it was trained on, it looks better than tested on objects far from them, by 9 points on average (85.0 % against 76.3 %), and by up to 14."
```

Every split is higher on the neighbours, by 3 to 14 points. The difference is not huge here (the landscape is simple and the samples
came from big, uniform places), but it is **always in the same direction**, and it can be much larger in other landscapes. The cure is in the design: **draw the reference over the whole map, with a probability sample
([[objects.sample]]), and keep it apart from the training objects.** The application warns when
the accuracy node reads the same samples as the node that trained the model: the accuracy is then assessed with the samples the
model was trained on. For the training samples themselves, spread them over the whole study area and over the different looks of
each class: five places of ten objects teach more than one of fifty.

## Settings that change the answer

### How many samples

The slider trains the forest with 3, 5, 8, 12 and 20 objects of each class (a class with fewer gives all it has: bare soil has 9
and water 14, so the last frame has 83 training objects), always assessed on the same 80 reference objects. The objects
are drawn at random from the 93, so each frame is one draw: with other seeds the line would wobble.

```tfslider
label: Training objects per class
caption: "The forest trained with from 3 to 20 objects per class (at most the class has), assessed on the same reference objects. Each frame is one random draw of the training objects."
frames:
  - { value: 3, src: /img/tutorials/object-based-image-analysis/classifying-by-samples/n_3.png, caption: "3 per class (15 objects)", stats: { "Overall accuracy": "62.5 %", Kappa: "0.52" } }
  - { value: 5, src: /img/tutorials/object-based-image-analysis/classifying-by-samples/n_5.png, caption: "5 per class (25 objects)", stats: { "Overall accuracy": "62.5 %", Kappa: "0.53" } }
  - { value: 8, src: /img/tutorials/object-based-image-analysis/classifying-by-samples/n_8.png, caption: "8 per class (40 objects)", stats: { "Overall accuracy": "65.0 %", Kappa: "0.56" } }
  - { value: 12, src: /img/tutorials/object-based-image-analysis/classifying-by-samples/n_12.png, caption: "12 per class (57 objects)", stats: { "Overall accuracy": "71.3 %", Kappa: "0.63" } }
  - { value: 20, src: /img/tutorials/object-based-image-analysis/classifying-by-samples/n_20.png, caption: "20 per class (83 objects)", stats: { "Overall accuracy": "80.0 %", Kappa: "0.74" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/classifying-by-samples/samples_per_class.csv
  x: per_class
  y: [overall, kappa]
  labels: { overall: "Overall accuracy", kappa: "Kappa" }
  ylabel: "Accuracy on the reference objects"
```

Accuracy rises with the number of samples, from 62.5 % with 3 per class to 80 % with 20, and the maps change as it grows (look at the town). The curve has not flattened out by 20. A few good samples of every look
of a class are worth more than many of one, but a class with a handful (bare soil here) stays poorly mapped.

### Which attributes

The same forest and the same nearest neighbour, with four sets of attributes, assessed on the same 80 reference objects:

```tftable
data: /data/tutorials/object-based-image-analysis/classifying-by-samples/attribute_sets.csv
labels: { attributes: "Attributes", predictors: "Predictors", rf_overall: "Forest: overall", rf_kappa: "Forest: kappa", nn_overall: "Nearest neighbour: overall", nn_kappa: "Nearest neighbour: kappa" }
align: { attributes: left }
digits: { predictors: 0, rf_overall: 3, rf_kappa: 2, nn_overall: 3, nn_kappa: 2 }
caption: "Overall accuracy by objects and kappa of the random forest and the nearest neighbour with four sets of attributes. The reference objects are the same in all rows: the table reports how the answer changes, it is not how to choose the attributes."
```

Three things, none of them a rule. The band means alone give 62.5 % with the forest. **Adding the three indices makes it worse**
(52.5 %): the indices are made from the same bands, so they bring no new information and only a bit of noise to a small sample. **Adding texture** brings
the forest to 77.5 %: this is the largest step, and it is the one that part 4's separability predicted (the texture separated the
town from the cerrado). The nearest neighbour does not profit from it in this sample (56 %), and
stays at 52 % to 56 % in the four rows. Choose the attributes with the feature space and a reference you set aside beforehand, not by
the best number in a table like this one, which is measured on the very objects you would judge the choice with.

### The settings of the nearest neighbour

```tftable
data: /data/tutorials/object-based-image-analysis/classifying-by-samples/nn_settings.csv
labels: { slope: "slope", min: "min", unclassified_share: "Share of the cells unclassified", overall: "Overall accuracy", kappa: "Kappa" }
digits: { slope: 2, min: 2, unclassified_share: 3, overall: 3, kappa: 2 }
caption: "The nearest neighbour on the band means and the three indices, with five pairs of the settings slope and min."
```

`slope` and `min` change the share of the objects left unclassified (0 % when `min` is 0, 7 % with a `slope` of 0.05, where a
distance of 1 gives a membership of 0.05) and the accuracy by a few points: here from 48.7 % to 55.0 %. It is a way to
say "I do not know" about the objects far from every sample, instead of forcing a class on them. Whether that is better depends on what
the map is for: a map for planning field work may want the unclassified.

## Common pitfalls

- **Testing on the training samples.** The classifier is always right about them. Keep the reference apart. The application warns when
  the accuracy node reads the samples of the training node.
- **A reference that is not a probability sample.** Boxes, roadside points and "the places I know" are not it. The adjusted
  areas assume that every object of a class had the same chance of being sampled. This page's reference, drawn in boxes, does not meet that, as
  the note above says.
- **Samples from one place.** See the experiment: they flatter the accuracy. Spread them.
- **A class with a handful of samples.** Under 10 objects the application marks a class *few* and Terraflux warns. The forest
  of this page never found bare soil (9 samples).
- **Samples of two classes in one object.** The object is left out of the training. Click inside one object, or paint the objects, not a
  line across a boundary. A polygon takes the objects that it covers by half.
- **Classes in alphabetical order.** The class numbers of the map follow the alphabetical order of the names (bare soil 1, cerrado 2, ...), not the order you added
  them in. Read the classes by name in the `table` outputs.
- **Position and size as predictors.** Without `columns`, [[ml.fit]] reads every numeric column, `cells`, `area` and the
  centroid's coordinates included. A forest that reads where an object is learns the map of the samples. Choose `columns`.
- **Attributes that do not match.** [[objects.classify]] matches the predictors by name: the objects must come from [[objects.features]]
  with the same settings as the training. A model fitted on a table with `ndmi` fails on objects without it.
- **Reading kappa as the accuracy.** Report the matrix, the user's and producer's accuracies and the adjusted areas.
- **Reporting the map's area as the area of the class.** The area of a map includes its errors. The adjusted area, with its interval, is the estimate.
- **Too many correlated attributes.** The forest does not mind much, the nearest neighbour does: each attribute adds to the distance.

## Going further

- Before: [Describing objects](/tutorials/object-based-image-analysis/describing-objects/) (the attributes, the feature space) and
  [What an object is](/tutorials/object-based-image-analysis/what-is-an-object/) (the segmentation).
- After: classify by rules and a hierarchy of classes (part 6), group objects without samples (part 9), and correct the map by hand (part 10).
- A shorter version of this chain, with a synthetic image: [Your first map by objects](/tutorials/object-based-image-analysis/first-map-by-objects/).
- The operators of this page: [[objects.samples]], [[ml.fit]], [[objects.classify]], [[objects.nearest]], [[objects.sample]],
  [[objects.accuracy]], [[objects.features]].
- The Viewer's tools for samples and the feature space: [the application's Viewer](/app/viewer/).

## References

- Belgiu, M., & Drăguţ, L. (2016). Random forest in remote sensing: a review of applications and future directions. *ISPRS
  Journal of Photogrammetry and Remote Sensing*, 114, 24-31. https://doi.org/10.1016/j.isprsjprs.2016.01.011
- Breiman, L. (2001). Random forests. *Machine Learning*, 45(1), 5-32. https://doi.org/10.1023/A:1010933404324
- Congalton, R. G. (1991). A review of assessing the accuracy of classifications of remotely sensed data. *Remote Sensing of
  Environment*, 37(1), 35-46. https://doi.org/10.1016/0034-4257(91)90048-B
- Ma, L., Li, M., Ma, X., Cheng, L., Du, P., & Liu, Y. (2017). A review of supervised object-based land-cover image
  classification. *ISPRS Journal of Photogrammetry and Remote Sensing*, 130, 277-293.
  https://doi.org/10.1016/j.isprsjprs.2017.06.001
- Olofsson, P., Foody, G. M., Stehman, S. V., & Woodcock, C. E. (2013). Making better use of accuracy data in land change
  studies: estimating accuracy and area and quantifying uncertainty using stratified estimation. *Remote Sensing of
  Environment*, 129, 122-131. https://doi.org/10.1016/j.rse.2012.10.031
- Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for
  estimating area and assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42-57.
  https://doi.org/10.1016/j.rse.2014.02.015
- Ploton, P., Mortier, F., Réjou-Méchain, M., Barbier, N., Picard, N., Rossi, V., Dormann, C., Cornu, G., Viennois, G.,
  Bayol, N., Lyapustin, A., Gourlet-Fleury, S., & Pélissier, R. (2020). Spatial validation reveals poor predictive
  performance of large-scale ecological mapping models. *Nature Communications*, 11, 4540.
  https://doi.org/10.1038/s41467-020-18321-y
- Roberts, D. R., Bahn, V., Ciuti, S., Boyce, M. S., Elith, J., Guillera-Arroita, G., Hauenstein, S., Lahoz-Monfort, J. J.,
  Schröder, B., Thuiller, W., Warton, D. I., Wintle, B. A., Hartig, F., & Dormann, C. F. (2017). Cross-validation
  strategies for data with temporal, spatial, hierarchical, or phylogenetic structure. *Ecography*, 40(8), 913-929.
  https://doi.org/10.1111/ecog.02881
