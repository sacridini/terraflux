---
title: "Classes without samples: k-means and self-organizing maps"
short: Classes without samples
description: "Group the objects of the Brasilia image by their attributes alone, with k-means and a self-organizing map, let the silhouette suggest how many groups there are, name the groups by reading them, and compare the result with the map made from samples."
order: 90
level: intermediate
time: 75 min
template: Unsupervised classification of an image (k-means)
ops: [data.stac, segment.multiresolution, objects.features, ml.kmeans, ml.som, objects.classify, objects.reclassify, objects.accuracy, table.read, vector.from_table]
series: Object-based image analysis
part: 9
---

In part 5 you told Terraflux what a few objects were and it classified the rest. Here you do the opposite: you give
it no samples at all. You ask it to **group** the 3,035 objects of the Sentinel-2 image of Brasília by how alike their
attributes are, and only afterwards, by looking at what each group is made of, you give the groups names. You will use two
tools for it, [[ml.kmeans]] and [[ml.som]] (a self-organizing map), apply them to the objects with
[[objects.classify]], and compare the result with part 5's random forest. At the end you will know when grouping is
worth doing, how to read the table and the chart that say how many groups there are, and why the map of groups is
a draft you still have to interpret.

## The idea

A **supervised** classification (part 5) learns from objects whose class you know. An **unsupervised** one learns from
nothing but the attributes: it looks for groups of objects that resemble each other and differ from the rest. The
groups are called **clusters**. A cluster has no name: the method knows that these 728 objects are alike, not that they
are cerrado. Naming is your part, and it is where ecological knowledge comes in.

Every object is a point in the **feature space** of part 4: one axis per attribute (the mean of each band, the NDVI,
the texture...). Objects of the same cover lie close together; clustering finds the clouds.

### k-means

**k-means** is the oldest and the most used. You say how many groups you want, \( k \). The method puts \( k \)
**centres** in the feature space, gives each object to its nearest centre, moves each centre to the mean of its objects, and
repeats until nothing moves (Lloyd, 1982). The first centres are chosen with the *k-means++* rule, which spreads them
out (Arthur & Vassilvitskii, 2007), and the whole is run `n_init` = 10 times from different starts, keeping the
run with the smallest total distance from each object to its centre (the **inertia**).

Two practical points matter more than the algorithm.

- **The attributes must count alike.** A band in reflectance (0 to 0.3) and a texture contrast (0 to 20) have very
  different ranges, and the one with the larger numbers would decide the groups alone. With `standardise` on (the
  default), Terraflux divides each attribute by its standard deviation first, as part 5's nearest neighbour does.
- **The groups are numbered by their centres, not by chance.** Terraflux orders the clusters by the first principal
  component of their centres, so cluster 1 is always at one end of the main gradient and the numbers (and the colours of a map)
  mean the same from one run to the next.

### Choosing k: the silhouette

How many groups? The data do not say directly. The **silhouette** (Rousseeuw, 1987) is one measure. For an object,
let \( a \) be its mean distance to the other objects of its cluster and \( b \) its mean distance to the objects of the
nearest other cluster. Then

\[ s = \frac{b - a}{\max(a,\, b)}, \]

which is near 1 when the object sits deep in its own cluster, near 0 when it is on the border between two, and negative
when it is probably in the wrong one. The mean of \( s \) over the objects is the silhouette *of that k*, and the k
with the highest value is the one with the best-separated groups. With `k = "auto"` Terraflux tries
every k from 2 to 10 and keeps the best silhouette. It also reports the inertia (which always falls as k grows, so it
cannot choose by itself) and the Calinski-Harabasz index, another measure of separation.

!!! science "Key idea"
    A low silhouette for every k (below about 0.25, says the operator's advice) means the objects have **no clear
    groups**: they form a continuum, and the clusters are a way of cutting it, not of finding classes. Even a
    high silhouette tells you the groups are separated *in the attributes you gave*, not that they are the classes
    you care about.

### The self-organizing map

A **self-organizing map** (SOM, Kohonen, 1982) is a different way to look at the same cloud. It lays a small
grid of **nodes** over the feature space, each node holding a weight for every attribute, a prototype object. Training
bends the grid to the data, so that each node sits where objects are, and, the key property, **nodes next to each
other on the grid hold similar objects**. After training each object belongs to its nearest node; the table of all the nodes
with their weights is the **codebook**.

What you get is a map of the feature space in two dimensions, which you can read:

- the **U-matrix** shows, for each node, its mean distance to its neighbours. It is large across the gaps between
  types of objects, and small inside one type: the borders of the groups show as ridges;
- a **component plane** shows one attribute's weight over the grid (where on the map the NDVI is high);
- the **counts** show how many objects each node holds.

A SOM has many nodes, usually more than there are classes, so to get classes the nodes are grouped in turn
(Vesanto & Alhoniemi, 2000): `clusters = 5` joins the nodes into five groups by Ward's method (or by k-means). This
*two-level* approach is what the operator does.

Two numbers say how good a trained map is. The **quantisation error** is the mean distance from each object to its
node (small is better, but it falls as the grid gets bigger, so it only compares maps of the same size). The
**topographic error** is the share of objects whose two nearest nodes are not neighbours on the grid; above 0.3 the map is
folded.

## When to use it

Grouping without samples is useful when:

- **you do not know the classes yet.** A first look at a new image or a new place: what kinds of objects are there?
  The clusters are a map of the variety, to plan where to take samples;
- **you want a quick map** without field data, and a name you give after the fact will do;
- **you want to check your classes.** If your forest samples fall in three different clusters, "forest" is
  perhaps three things (a shaded forest, a lit forest, a plantation);
- **there are too many samples to click.** Clusters that you name by looking at a few objects each are cheaper than hundreds of samples.

It is not the right tool when you need the classes of a legend (a land-cover map of a protected area with
classes fixed by a protocol): clusters do not follow the legend, and a class that is rare (bare soil here) or that looks like
another (a dark forest and the cerrado) may have no cluster of its own. Then use samples (part 5) or rules
(part 6). Whatever the method, a map still needs an accuracy assessment against reference objects; this page does one.

## What you need

- **The objects and their attributes**: the Sentinel-2 clip of Lago Paranoá, cut by [[segment.multiresolution]] at the scale 2
  into 3,035 objects and described by [[objects.features]] (parts 2 to 4). No samples.
- **The attributes you want to group by, named**: the `columns` of the two operators. This is the most
  important setting of the page and it has its own section.
- **A reference, only to check**: the 80 reference objects of part 5
  ([reference_samples.csv](/data/tutorials/object-based-image-analysis/classes-without-samples/reference_samples.csv)).
  They do not take part in the grouping; they only say afterwards whether the names you gave were right.
  As in part 5, they are drawn in boxes by a script and are not a probability sample, so read the accuracies as an
  example.
- **A template**: *Unsupervised classification of an image (k-means)* has the k-means chain on a synthetic image
  and runs as it is; *Types of landscape by a self-organizing map* does the same for the SOM.

## Build the model

### Steps 1 to 5: the objects and their attributes

The same five nodes as in part 5: [[data.stac]], [[raster.reproject]] and [[raster.calc]] (the reflectance) read the scene,
[[segment.multiresolution]] cuts it at the scale 2, and [[objects.features]] describes the objects. The attributes
are the colour (the mean of each band), the three indices, the texture of the brightness and the compactness. Their
table, the `table` output of [[objects.features]], is what the grouping reads: a row per object, a column per attribute.

### Step 6: k-means on the table

[[ml.kmeans]] reads the objects' table. Two of its settings matter here.

- **`columns`** names the variables. Without it, **every** numeric column of the table enters, and that includes
  `id` (the object's number), `cells` and `area` (its size): attributes that say nothing about the cover. Name the
  twelve attributes you want.
- **`k`** is a number, a list of numbers, or `"auto"` (the default).

The operator has five outputs. `model` is what [[objects.classify]] will apply; `centres` is a row per cluster with its size, its
inertia and the value of each attribute at its centre, in the attribute's own units (the table you read to name the
cluster); `choice` is a row per k tried with its inertia, silhouette and Calinski-Harabasz index; `clusters` and `table` are the
cluster of each cell and of each row, not needed here because the objects come next.

[[objects.classify]] takes the objects and the `model` and gives each object its cluster. This is the same operator as in part 5; with a model that has no
class labels (from [[ml.kmeans]] or [[ml.som]]) it needs no `samples`: each object takes its cluster, named `cluster 1`,
`cluster 2`..., and the probability of its cluster is 1 and of the others 0. The attributes are matched by the **names**
of the variables the model was fitted on, so the objects must carry them (here, by the same settings of
[[objects.features]]).

```tfgraph
#! caption: Steps 6 and 7 (nodes 1 and 2 here): k-means on the objects' table and the objects given their cluster. The five nodes before them are those of part 5.
#! highlight: km, cl
#! numbered: true
#! fragment: true
[[nodes]]
id = "km"
op = "ml.kmeans"
inputs = { table = "feat.table" }
params = { k = 5, columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast", "glcm_homogeneity", "sd_nir", "compactness"] }
note = "Five groups of the twelve attributes."

[[nodes]]
id = "cl"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "km.model" }
note = "cluster 1 ... cluster 5."
```

### Step 8: naming the groups

[[objects.reclassify]] renames classes: `classes` maps the old names to the new ones, and **two clusters given the
same name become one class**. `colours` gives each class its colour. You write this node *after* reading
the `centres` table and looking at the map (see *Read the results*).

### Steps 9 to 11: the self-organizing map

[[ml.som]] has the same inputs as [[ml.kmeans]] and the same `columns`. Its own settings:

- **`rows` and `cols`** are the size of the grid. Without them the grid has about \( 5\sqrt{n} \) nodes for \( n \)
  objects (Vesanto's rule), here 20 by 14 = 280 nodes for 3,035 objects, a note says so. We will see that
  this is too many, and use 8 by 8.
- **`clusters`** is the number of groups of nodes (0, the default, gives none), and **`grouping`** is `ward` (default) or `kmeans`.
- `training` is `batch` (default: all the objects, then the weights) or `online` (one object at a time, in random order); `topology`
  is `rectangular` or `hexagonal`; `neighbourhood`, `sigma`, `learning_rate`, `decay`, `iterations` and `init` are the
  details of the training, and the defaults are those of the MiniSom library.

Its outputs: `model`, `codebook` (a row per node: its `row` and `column`, the `count` of objects, its `group`, its `umatrix`
distance and the weight of each attribute in its own units), `quality` (the quantisation and topographic errors and the
number of empty nodes) and the maps `nodes` and `groups` for a stack of maps. [[objects.classify]] with a SOM model that
has `clusters` gives each object the **group** of its node (`group 1`...); without `clusters`, its node (`node 1`...).

### Steps 12 to 14: checking against the reference

[[objects.accuracy]] takes the named objects and the reference points and makes the error matrix as in part 5.

```tfgraph
#! caption: The whole model (the nodes of the first step are those of part 5): k-means and a 8 x 8 map on the same objects, each named by objects.reclassify, and both checked against the reference points (a CSV of x, y and class).
#! highlight: km, cl, named, som, clsom, named_som, acc_km, acc_som
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
note = "The table of attributes to group by."

[[nodes]]
id = "km"
op = "ml.kmeans"
inputs = { table = "feat.table" }
params = { k = 5, columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast", "glcm_homogeneity", "sd_nir", "compactness"] }
note = "k = 5; try k = auto."

[[nodes]]
id = "cl"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "km.model" }
note = "cluster 1 to 5."

[[nodes]]
id = "named"
op = "objects.reclassify"
inputs = { objects = "cl.objects" }
params = { classes = { "cluster 1" = "water", "cluster 2" = "cerrado", "cluster 3" = "forest", "cluster 4" = "urban", "cluster 5" = "urban" }, colours = { water = "#1f6fd0", cerrado = "#d9b02e", forest = "#1a8a3c", urban = "#c2338a" } }
note = "Named after reading the centres; two clusters are urban."

[[nodes]]
id = "som"
op = "ml.som"
inputs = { table = "feat.table" }
params = { rows = 8, cols = 8, clusters = 5, columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast", "glcm_homogeneity", "sd_nir", "compactness"] }
note = "8 x 8 nodes, grouped in 5."

[[nodes]]
id = "clsom"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "som.model" }
note = "group 1 to 5."

[[nodes]]
id = "named_som"
op = "objects.reclassify"
inputs = { objects = "clsom.objects" }
params = { classes = { "group 1" = "water", "group 2" = "cerrado", "group 3" = "forest", "group 4" = "urban", "group 5" = "urban" }, colours = { water = "#1f6fd0", cerrado = "#d9b02e", forest = "#1a8a3c", urban = "#c2338a" } }

[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_samples.csv" }
note = "The 80 reference points: x, y, class."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "acc_km"
op = "objects.accuracy"
inputs = { objects = "named.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "Error matrix of the named k-means."

[[nodes]]
id = "acc_som"
op = "objects.accuracy"
inputs = { objects = "named_som.objects", samples = "ref.vector" }
params = { design = "objects" }
```

=== "Application"

    Build the model from the template *Unsupervised classification of an image (k-means)* or by hand: the five nodes of part 5, then [[ml.kmeans]] with the objects' `table`, **Columns** chosen in the
    Node pane, and [[objects.classify]]. Select `km` and press ++ctrl+enter++, open its `choice` output in the Viewer
    (it draws the *choice of k*: inertia and silhouette against k) and its `centres`. Run `cl` and open its `map`.
    For the SOM, open the `codebook` output of [[ml.som]] in the Viewer: it draws the grid with four views (U-matrix,
    component planes, counts and groups); a click on a node, or on a group, lights its objects on the map in the
    Viewer, and a click on the map picks the node of the cell.

=== "Command line"

    Save the graph as `groups.toml` beside
    [reference_samples.csv](/data/tutorials/object-based-image-analysis/classes-without-samples/reference_samples.csv)
    (80 rows, the columns `x`, `y` and `class`) and run
    `terraflux run groups.toml --threads 4 --out output`. The model reads the scene from Earth Search; without a
    network, replace the `s2` and `utm` nodes by a [[raster.read]] of the clip
    ([sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif), the same grid, Int16: the `refl`
    node divides it by 10,000). The script that made this page's figures is `classes-without-samples.py`, in
    `website/figures/object-based-image-analysis/`.

## Read the results

### How many groups: the table of choices

Run [[ml.kmeans]] with `k = "auto"`. Its `choice` table, one row per k:

```tftable
data: /data/tutorials/object-based-image-analysis/classes-without-samples/choice.csv
labels: { k: "k", inertia: "Inertia", silhouette: "Silhouette", calinski_harabasz: "Calinski-Harabasz" }
digits: { k: 0, inertia: 0, silhouette: 3, calinski_harabasz: 0 }
caption: "ml.kmeans with k = auto on the 3,035 objects: what each k from 2 to 10 gives. The best silhouette, at k = 3, is the one that the operator keeps (and says so in a note)."
```

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/classes-without-samples/choice.csv
x: k
y: [silhouette]
labels: { silhouette: "Silhouette" }
colors: { silhouette: "#3b8a5a" }
xlabel: "Number of clusters k"
ylabel: "Mean silhouette"
height: 260
caption: "The silhouette of k = 2 to 10. It peaks at k = 3 (0.337) and then stays between 0.27 and 0.30."
```

The inertia falls steadily (from 25,535 to 8,553) and has no elbow to speak of. The silhouette is highest at k = 3 and **low everywhere**
(0.27 to 0.34), and the operator's advice says to be careful when it is below 0.25. These objects are a continuum, not
a few clean clouds: the town grades into the garden trees, the garden trees into the gallery forest. k = 3 is the cut with
the clearest gap, and here is what it is:

![The three clusters of k = auto](/img/tutorials/object-based-image-analysis/classes-without-samples/auto.png "k = auto chose three clusters: the water, the vegetation (cerrado and forest together) and the built-up objects with the bare ground. It is a good answer to a coarser question than ours.")

With three clusters the lake is one (blue), the vegetation another and the town the third. That is a perfectly good
answer for "where is water, where is built-up", and the silhouette is right to prefer it. It is not an answer for
a map with a forest class and a cerrado class. **The silhouette finds the cut that best separates the data, not the
number of classes in your legend.** When your legend has five classes, ask for k = 5 and judge the result by what the groups are.

The slider moves k from 2 to 8. The linked chart is the silhouette; the frame also shows the agreement with the
reference, the share of the 80 reference objects that fall in the cluster where their own class is the most frequent
(a *purity*, which grows with k almost whatever you do, so do not choose k by it):

```tfslider
label: Number of clusters k
caption: "ml.kmeans with k from 2 to 8 on the same twelve attributes. The numbers (and the colours) of the clusters follow their position along the first principal component of their centres, so they change little as k grows."
frames:
  - { value: 2, src: /img/tutorials/object-based-image-analysis/classes-without-samples/k_2.png, caption: "k = 2", stats: { Silhouette: "0.293", "Purity against the reference": "42.5 %" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/classes-without-samples/k_3.png, caption: "k = 3 (the best silhouette)", stats: { Silhouette: "0.337", "Purity against the reference": "57.5 %" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/classes-without-samples/k_4.png, caption: "k = 4", stats: { Silhouette: "0.297", "Purity against the reference": "68.8 %" } }
  - { value: 5, src: /img/tutorials/object-based-image-analysis/classes-without-samples/k_5.png, caption: "k = 5", stats: { Silhouette: "0.301", "Purity against the reference": "78.7 %" } }
  - { value: 6, src: /img/tutorials/object-based-image-analysis/classes-without-samples/k_6.png, caption: "k = 6", stats: { Silhouette: "0.302", "Purity against the reference": "81.2 %" } }
  - { value: 7, src: /img/tutorials/object-based-image-analysis/classes-without-samples/k_7.png, caption: "k = 7", stats: { Silhouette: "0.282", "Purity against the reference": "80.0 %" } }
  - { value: 8, src: /img/tutorials/object-based-image-analysis/classes-without-samples/k_8.png, caption: "k = 8", stats: { Silhouette: "0.285", "Purity against the reference": "80.0 %" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/classes-without-samples/k_slider.csv
  x: k
  y: [silhouette]
  labels: { silhouette: "Silhouette" }
  ylabel: "Mean silhouette"
```

### What the five clusters are

With k = 5, the `centres` table is where you read what each cluster is. It has the cluster's size and the value of every
attribute at its centre, in the attribute's own units (a few of them here):

```tftable
data: /data/tutorials/object-based-image-analysis/classes-without-samples/centres.csv
labels: { cluster: "Cluster", size: "Objects", mean_red: "Red", mean_nir: "NIR", ndvi: "NDVI", ndwi: "NDWI", glcm_contrast: "Texture contrast", glcm_homogeneity: "Texture homogeneity", named: "Name I gave it" }
digits: { cluster: 0, size: 0, mean_red: 3, mean_nir: 3, ndvi: 2, ndwi: 2, glcm_contrast: 2, glcm_homogeneity: 2 }
align: { named: left }
caption: "The five clusters of k-means: how many objects each holds and the value of some attributes at its centre (reflectance 0 to 1). The last column is the name I chose after reading the row and the map."
```

Read it as you would read a field description.

- **Cluster 1**, 144 objects, NDWI 0.53 and NIR 0.01: nothing reflects in the near infrared. **Water.**
- **Cluster 2**, 728 objects, NDVI 0.45, smooth texture (contrast 0.5, homogeneity 0.82), a red of 0.07: green but
  dry and uniform. **Cerrado.**
- **Cluster 3**, 500 objects, the highest NDVI of the five (0.67), the darkest red of the land clusters (0.04) and a
  rougher texture than the cerrado (contrast 1.7): **forest**, the dark canopy with its own shadows.
- **Cluster 4**, 1,129 objects, NDVI 0.38, texture contrast 4.9, homogeneity 0.46: a mix of roofs, gardens and streets in
  one object. **Urban**, the common kind.
- **Cluster 5**, 534 objects, the brightest (red 0.17), the lowest NDVI of the land clusters (0.21) and the roughest texture
  (contrast 9.6): bright roofs and bare ground. I could call it *bare soil*; the map says it is mostly sunlit roofs among the
  gardens, spread all over the town. So I name it **urban** too.

![The five clusters](/img/tutorials/object-based-image-analysis/classes-without-samples/clusters5.png "The same part of the clip as in the other tutorials, with the objects outlined (left) and their five k-means clusters (right). Cluster 1 is the lake, cluster 2 the cerrado in the lower right, cluster 3 the gallery forest along the streams, clusters 4 and 5 the town.")

That is the whole job of naming: write down, for each cluster, what you see in its attributes and on the map, and
give it a class. Three choices came up in this table, and they are yours, not the method's:

1. a name may be given to **two clusters**: both urban here. [[objects.reclassify]] joins them into one class;
2. a class may have **no cluster**: bare soil is 74 ha of this image, 2 % of the objects, and k = 5 spends its clusters on
   the town's variety instead;
3. a cluster may be **mixed**: cluster 2 is mostly cerrado, but 7 of the 16 reference forest objects fall in it (dark, smooth
   forest looks like cerrado to the attributes).

### Named and checked

The [[objects.reclassify]] of the model joins the clusters by the names above, and [[objects.accuracy]] compares the result
with the reference objects. This is the error matrix of the named k-means:

```tftable
data: /data/tutorials/object-based-image-analysis/classes-without-samples/mat_named.csv
heat: true
digits: 0
align: { "Map \\ reference": left }
caption: "Named k-means (k = 5): the 80 reference objects by the class the map gave (rows) and their true class (columns). 63 are on the diagonal. Bare soil has no row of its own: no cluster was named after it."
```

The overall accuracy by objects is **78.8 %** (63 of 80), kappa 0.72, and 80.9 % after the area adjustment of
part 5 (`design = "objects"`: the reference has one object per point, each weighted by its area). Water is right in all 16 objects, cerrado in 18 of 25 (the 7 forest objects that went to cerrado), and the 6 reference
bare soil objects are all called urban, since no cluster is named after them.

Now the same with the self-organizing map and for part 5's forest, on the same reference:

```tftable
data: /data/tutorials/object-based-image-analysis/classes-without-samples/accuracy.csv
labels: { map: "Map", variables: "Variables", overall: "Overall accuracy by objects", adjusted: "Overall, area-adjusted", kappa: "Kappa" }
align: { map: left, variables: left }
digits: { overall: 3, adjusted: 3, kappa: 3 }
caption: "The three maps against the same 80 reference objects. The random forest of part 5 learned from 93 samples; the other two from none, and the clusters were named by reading their centres."
```

![The three maps side by side](/img/tutorials/object-based-image-analysis/classes-without-samples/compare.png "The part of the clip of the other tutorials: the true colours with the objects, part 5's random forest, the named k-means and the named SOM (8 x 8 nodes, 5 groups). The three agree on the lake, the cerrado and the town, and differ in the forest and in the bare soil.")

**On this image the two unsupervised maps are as accurate as the supervised one**: 78.8 % and 80.0 % against 80.0 %. Do not
read that as "samples are not needed". It says three things instead.

- This landscape has a few well-separated covers (water, the dry cerrado, the town) and the five classes are the ones
  the attributes separate best. In a landscape where the classes of the legend are close in the attributes (two types of
  savanna, a pasture and a degraded pasture), clusters will not find them and samples will.
- The reference is small (80 objects) and not a probability sample: a difference of one or two objects is 1 to 2
  points, and the intervals of part 5 (about ±10 points) cover all three maps.
- The unsupervised maps got their accuracy *after* you looked at the clusters and joined two of them, so a part
  of the work that samples do at the start was done by hand at the end. And the one place that matters
  to an ecologist, **the forest**, is where they are worst: the area table below shows it.

By area, how the named k-means falls on part 5's forest map (hectares):

```tftable
data: /data/tutorials/object-based-image-analysis/classes-without-samples/vs_forest.csv
labels: { cluster: "k-means cluster", "bare soil": "Forest map: bare soil", cerrado: "cerrado", forest: "forest", urban: "urban", water: "water" }
columns: [cluster, "bare soil", cerrado, forest, urban, water]
align: { cluster: left }
digits: 0
heat: true
caption: "Hectares of each k-means cluster (rows) in each class of part 5's random forest (columns). 78.8 % of the cells have the same class on both maps."
```

The two maps agree on 78.8 % of the cells. The water is nearly the same (191 ha of cluster 1 are water in the forest
map; the other 31 ha the forest map calls forest). The big difference is in cluster 2: 705 ha of cerrado and
**334 ha that the forest map calls forest**, dark and smooth stands that the unsupervised method cannot tell from dry grass without
the samples that told the forest about them.

### The self-organizing map

The same attributes through [[ml.som]]. With the default grid (Vesanto's rule) the map has 20 by 14 = 280 nodes for 3,035 objects, about 11
per node, and 21 of the nodes are empty:

![The default map](/img/tutorials/object-based-image-analysis/classes-without-samples/som-default.png "The 20 x 14 map: U-matrix, objects on each node and the five groups of nodes (Ward's method). The yellow corners of the U-matrix and the count are the nodes that hold the lake (top left), at the end of the map.")

The four corner nodes hold 145, 174, 126 and 82 objects (the lake is the one at the top left) and the rest are spread thin.
Look at the groups: one of them joins the water and **a type of cerrado** (the reference says 16 water and 18
cerrado objects are in the same group). The probable reason is that Ward's method groups the *nodes*, each one counting once whatever it holds,
so the many thinly filled nodes between the lake and the vegetation weigh as much as the node with the lake's 145 objects. With 280 nodes the map
is too big for 3,035 objects. A smaller grid, 8 by 8:

![The 8 x 8 map](/img/tutorials/object-based-image-analysis/classes-without-samples/som-codebook.png "The 8 x 8 map: U-matrix, objects on each node and the five groups (top); NDVI, NDWI and texture contrast of the nodes' weights (bottom). The lake is the top-left corner, the U-matrix is high around it, NDVI grows to the right, texture contrast down.")

The six panels are what the application draws for the `codebook` table. The **U-matrix** is bright at the top left: the
lake is a different place from the rest, a gap. The **component planes** tell what each direction of the map is: NDVI grows
to the right, NDWI is high only in the top-left corner (the lake), texture contrast grows from top to bottom (from smooth
to rough: cerrado and forest at the top, roofs at the bottom). Reading the planes
together with the groups, group 1 (blue) is the water, group 2 the cerrado and group 3 the forest in the upper right,
groups 4 and 5 the town. I named them in that order and joined 4 and 5, as for k-means.

The grid size is a setting that changes the answer, so it has a slider:

```tfslider
label: Grid of the map
caption: "ml.som with clusters = 5 on grids from 3 x 3 to the default 20 x 14. The groups of each map are painted in the same colours as for k-means. Purity is the share of the reference objects that are in the group where their own class is the most frequent."
frames:
  - { value: 9, src: /img/tutorials/object-based-image-analysis/classes-without-samples/grid_3x3.png, caption: "3 x 3 nodes", stats: { "Objects per node": "337", "Quantisation error": "1.73", "Empty nodes": "0", Purity: "71.3 %" } }
  - { value: 16, src: /img/tutorials/object-based-image-analysis/classes-without-samples/grid_4x4.png, caption: "4 x 4 nodes", stats: { "Objects per node": "190", "Quantisation error": "1.63", "Empty nodes": "0", Purity: "83.8 %" } }
  - { value: 36, src: /img/tutorials/object-based-image-analysis/classes-without-samples/grid_6x6.png, caption: "6 x 6 nodes", stats: { "Objects per node": "84", "Quantisation error": "1.49", "Empty nodes": "1", Purity: "83.8 %" } }
  - { value: 64, src: /img/tutorials/object-based-image-analysis/classes-without-samples/grid_8x8.png, caption: "8 x 8 nodes (the page's)", stats: { "Objects per node": "47", "Quantisation error": "1.43", "Empty nodes": "2", Purity: "80.0 %" } }
  - { value: 120, src: /img/tutorials/object-based-image-analysis/classes-without-samples/grid_12x10.png, caption: "12 x 10 nodes", stats: { "Objects per node": "25", "Quantisation error": "1.45", "Empty nodes": "6", Purity: "85.0 %" } }
  - { value: 280, src: /img/tutorials/object-based-image-analysis/classes-without-samples/grid_20x14.png, caption: "20 x 14 nodes (Vesanto's rule, the default)", stats: { "Objects per node": "11", "Quantisation error": "1.50", "Empty nodes": "21", Purity: "66.2 %" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/classes-without-samples/grids.csv
  x: nodes
  y: [purity]
  labels: { purity: "Purity against the reference" }
  ylabel: "Purity"
```

From 16 to 120 nodes the groups are much the same (purity 80 to 85 %) and the water is a group of its own. At 3 by 3 the
map is too coarse to separate the cerrado from the forest; at 20 by 14 the water joins another group. The quantisation error
fell from 1.73 (3 by 3) to 1.43 (8 by 8) and rose again for the bigger grids (1.45, 1.50): it is **not** the way to choose, because
it measures how close the nodes are to the objects, not whether the groups are classes. The topographic error stays at 0.6 % or less
in all of them, so none of the maps is folded.

The named SOM (8 by 8) gives 80.0 % by objects, kappa 0.74, 84.6 % adjusted: the same by objects as the forest, and a few points higher than the k-means (80.9 %) and the forest (81.1 %) once each object counts by its area; with only 80 reference objects it is not a difference to rely on. What
the SOM adds is the *picture*: the map of the feature space, in which you see how the classes lie to each other, the gaps
between them and the nodes between two types, which k-means does not show.

## Settings that change the answer

### Which attributes: `columns`

If you leave `columns` out, [[ml.kmeans]] groups by every numeric column of the table. For the objects' table that is 19 columns, among
them `id`, `cells` and `area`. Here is the result with k = 5:

![With and without columns](/img/tutorials/object-based-image-analysis/classes-without-samples/columns.png "k-means with k = 5 on the twelve attributes (left) and on every numeric column of the table (right).")

```tftable
data: /data/tutorials/object-based-image-analysis/classes-without-samples/columns.csv
labels: { variables: "Variables", count: "How many", silhouette: "Silhouette", same_as_first: "Share of the cells in the same cluster as the first row" }
align: { variables: left }
digits: { count: 0, silhouette: 3, same_as_first: 3 }
caption: "k-means with k = 5 with the attributes named and with no columns."
```

The two maps look alike, and 92 % of the cells land in the same cluster, so you could miss it. But the silhouette falls from 0.30 to 0.25,
and the clusters now differ in their *object number* (the mean `id` runs from 406 in the lake cluster to 2,427 in the
cerrado one) and in their *size* (the mean area from 8,100 to 15,500 m2). An object's number is an accident of the segmentation and means nothing for the
cover, and its size is already told by the texture and the shape. The rule: **always name `columns`**, and never
include id, cells, area or the coordinates of the centroid.

### Standardise

`standardise` is on by default and right for attributes in different units. Turning it off lets the attribute with the largest numbers decide:
with reflectances (0 to 0.3), the indices (-1 to 1) and the texture contrast (0 to 20), the contrast would be nearly the whole distance. Turn
it off only for variables in the same unit (all bands, as reflectances).

### The seed

Both operators have a `seed`. For k-means it sets the starting centres (and the sample drawn, above a million records, which does not
happen with objects); the SOM starts from the principal components by default (`init = "pca"`), so its seed matters only with
`init = "random"`, with `training = "online"` or with a sample. We ran k-means and the 8 by 8 SOM with five seeds: the cluster
of each cell is the same in all the runs but for 0.1 % of the cells in one run of k-means. The groups are steady because the best of 10 starts is kept, and the numbering by
the first principal component makes them comparable. With a bigger k or with noisier data the seed matters more: check with
two or three seeds before you trust a map.

### Apply the model to something else

The `model` output of both operators holds the centres (or the weights), and the means and standard deviations of the fit. [[ml.predict]] applies it to
another table or to a stack of maps: each new record gets the **nearest centre of this fit**, not a re-clustering of the new data. This
is how you use the clusters of one year on the objects of another (part 11 does the opposite, a model per date). Because it
uses the means and scales of the fit, the new data must have the same attributes in the same units.

## Common pitfalls

- **Taking a cluster for a class.** A cluster is a group of objects that look alike; a class is a thing in the world.
  Read each cluster's `centres` row and its map before you name it, and expect to join clusters.
- **Choosing k by the inertia.** It always falls. Use the silhouette, or better the question: how many classes does the map need?
- **Trusting a high silhouette and ignoring a low one.** A silhouette under 0.25 for every k says the groups are a way of cutting
  a continuum.
- **Letting size and numbers into the variables.** `id`, `cells` and `area` enter when `columns` is left out.
- **A grid too big for the objects.** With a few records per node the nodes are filled by their neighbours; the groups of nodes
  then follow the empty space. Give `rows` and `cols`.
- **Forgetting that the numbering has no meaning of its own.** Cluster 3 is *the third along the first principal component*; it
  is not "forest" in another project. Always name from the `centres` or `codebook`.
- **Comparing with a reference after choosing the names by it.** If you name the clusters by looking at the reference points, their accuracy is
  flattered. Name them from the centres and the map, then check.
- **Applying the model to an image of another sensor or a different season** without the same attributes and units: the
  nearest centre is of the fit, and the new image may have no object near any of them.

## Going further

- Before: [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/) (part 5, the supervised map this page compares with)
  and [Describing objects](/tutorials/object-based-image-analysis/describing-objects/) (the attributes you group by).
- After: [Change by objects](/tutorials/object-based-image-analysis/change-by-objects/), where two classifications of two dates are bound by a
  transition matrix.
- The same grouping for maps and tables that are not objects, and for the principal components, with the operators
  [[ml.pca]], [[ml.kmeans]] and [[ml.som]]; the templates *Unsupervised classification of an image (k-means)* and *Types of landscape by a self-organizing map*.
- The operators of this page: [[ml.kmeans]], [[ml.som]], [[objects.classify]], [[objects.reclassify]], [[objects.accuracy]],
  [[ml.predict]], [[objects.features]].

## References

- Arthur, D., & Vassilvitskii, S. (2007). k-means++: the advantages of careful seeding. *Proceedings of the Eighteenth Annual
  ACM-SIAM Symposium on Discrete Algorithms (SODA '07)*, 1027-1035.
- Kohonen, T. (1982). Self-organized formation of topologically correct feature maps. *Biological Cybernetics*, 43(1), 59-69.
  https://doi.org/10.1007/BF00337288
- Lloyd, S. P. (1982). Least squares quantization in PCM. *IEEE Transactions on Information Theory*, 28(2), 129-137.
  https://doi.org/10.1109/TIT.1982.1056489
- Rousseeuw, P. J. (1987). Silhouettes: a graphical aid to the interpretation and validation of cluster analysis.
  *Journal of Computational and Applied Mathematics*, 20, 53-65. https://doi.org/10.1016/0377-0427(87)90125-7
- Vesanto, J., & Alhoniemi, E. (2000). Clustering of the self-organizing map. *IEEE Transactions on Neural Networks*,
  11(3), 586-600. https://doi.org/10.1109/72.846731
