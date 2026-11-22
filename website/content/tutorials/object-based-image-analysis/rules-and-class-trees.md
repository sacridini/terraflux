---
title: "Rules and class trees"
short: Rules and class trees
description: "Classify the objects of a Sentinel-2 image with no samples: write each class as a fuzzy rule on the NDWI, the NDVI and the brightness, let a class inherit its parent's rule in a class tree, read the memberships and the margin, and add a second pass that reads the neighbours."
order: 60
level: intermediate
time: 60 min
template: Classify an image by rules
ops: [segment.multiresolution, objects.features, objects.calc, objects.rules, view.plot]
series: Object-based image analysis
part: 6
---

A classifier trained on samples learns the rules from the examples you give it. Sometimes you already know the rules.
Water has a high NDWI. Dense vegetation has a high NDVI. A town is brighter than the dry grassland around it. In this
tutorial you will write such knowledge down as **rules** and apply it to the objects of the Sentinel-2 clip of Lago
Paranoá: no samples, no training, and every decision open to inspection. You will give each class a **fuzzy** rule (a
number from 0 to 1 that says how well an object fits the class), organise the classes in a **class tree** where a
child inherits what its parent says, read the **memberships** and the **margin** that tell you where the rules are
unsure, and add a second pass of rules that reads the classes of the neighbours. At the end you will have a map of
water, forest, built-up land and cerrado, the areas of each, and a clear idea of how far each threshold moves the answer.

## The idea

A hard rule says "forest if NDVI is above 0.5". An object with an NDVI of 0.49 is not forest and an object with 0.51 is,
though no one could tell them apart. The world does not have such sharp edges, and the numbers measured on an image have
noise on top of it. **Fuzzy logic**, introduced by Zadeh (1965), replaces the yes or no with a **degree of membership**
between 0 (not at all) and 1 (completely). The rule "forest if NDVI is high" becomes a ramp: 0 below an NDVI of 0.45,
1 above 0.6, and a straight line between. An object of 0.49 is forest to degree 0.27 and an object of 0.51 to degree
0.4: close, as they should be.

Terraflux gives four such shapes, in [[objects.calc]] and so in every rule. Each is drawn below, with its marks on the
axis.

![The fuzzy functions](/img/tutorials/object-based-image-analysis/rules-and-class-trees/functions.png "The four fuzzy functions of the rules, drawn from 0 to 1. above rises between its two numbers, below falls between them, between is a plateau with a rise and a fall (four numbers), about is a triangle centred on a value with a half-width.")

- `above(x, a, b)` is 0 up to `a`, 1 from `b` and a line between: "x is high".
- `below(x, a, b)` is the opposite: "x is low".
- `between(x, a, b, c, d)` rises from `a` to `b`, stays at 1 up to `c` and falls to 0 at `d`: "x is in a range".
- `about(x, m, w)` is 1 at `m` and falls to 0 at a distance `w`: "x is close to `m`".

Rules are combined as Zadeh did: `a and b` is the **smaller** of the two memberships (you are as much a member of both as
you are of the weaker one), `a or b` is the **larger**, and `not a` is `1 - a`. With values of exactly 0 and 1 this is
ordinary logic.

### The class hierarchy

Object-based analysis took this idea from the eCognition software, in which Benz and colleagues (2004) describe a
**class hierarchy**: classes with membership functions on the objects' features, arranged in a tree where a class
**inherits** the description of its parent. You do not repeat the parent's conditions in every child. If the class "land" is
"not water", then "forest" and "open ground" are two kinds of land and need only say what separates them.

!!! science "Key idea"
    **A class's membership.** Each class has its own rule \( r_c(o) \), a number from 0 to 1 for the object \( o \). If the
    class has a parent \( p \), its membership is the smaller of its own rule and its parent's membership:

    \[ \mu_c(o) = \min\big( r_c(o),\ \mu_p(o) \big), \]

    and a class with no parent has \( \mu_c = r_c \). A class with no rule of its own takes its parent's membership.
    The **leaves** of the tree are the classes that are nobody's parent. An object becomes the leaf of the **largest**
    membership, as long as that membership is at least a minimum (`min`, 0.1 by default; below it the object stays
    unclassified). The parents are never an answer: they only narrow what their children may take.

The tree used in this tutorial is below. Water and land split the image by the NDWI. Inside land, forest and open ground
split by the NDVI. Inside open ground, built-up and cerrado split by how bright the object is in visible light. Five
rules say what a flat list of four classes would need nine conditions for.

![The class tree](/img/tutorials/object-based-image-analysis/rules-and-class-trees/tree.png "The class tree of the page. The classes in colour are the leaves, what an object becomes; the grey ones, land and open, are parents and only narrow what their children may take. The numbers are the ends of the ramps.")

### What it is good for

Rules are **transparent**: you can say why an object has its class, and the application shows it object by object. They
need **no samples**, so they suit a place where no one has been, or a class too rare to sample. They **travel**: the
rules you wrote for one scene can be applied to the next, with their thresholds checked. And the memberships come out
with the classes, so you can map where the rules are sure and where they hesitate.

## When to use it

Rules are the right tool when:

- you **know the physics**: water absorbs the near infrared, green leaves reflect it, shadows are dark in every band;
- the classes are **few and well separated** by one or two attributes (water, vegetation, built-up, bare ground);
- you must **justify each threshold** in a report, or share the method with someone who will apply it to other images;
- you want a first map quickly, to find where more work is needed before collecting samples.

They are **not** the tool when:

- the classes overlap in every single attribute and only the combination of many tells them apart (crop types, tree
  species): samples and a classifier find those combinations, you would have to guess them
  ([part 5](/tutorials/object-based-image-analysis/classifying-by-samples/));
- you do not know the numbers. Thresholds depend on the sensor, the date, the units and the atmosphere. A rule written for
  one image is a hypothesis for another;
- you need an **accuracy** that is better than your reading of the map. Rules can be as wrong as samples can. The
  [accuracy assessment](/tutorials/object-based-image-analysis/classifying-by-samples/) applies to them in the same way.

Rules and samples also **combine**: samples classify the hard classes, rules mask the easy ones, or the rules'
memberships feed the classifier as attributes. This page keeps to rules.

## What you need

- **The image.** The Sentinel-2 clip of the series: blue, green, red, near infrared and `swir16`, UTM 23S at 10 m, read
  by [[data.stac]] as in [part 2](/tutorials/object-based-image-analysis/what-is-an-object/), or the download
  [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) (2.3 MB) with
  [[raster.read]]. Its attribution is in [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt).
  The bands must be named `red`, `green` and `nir`: the NDVI and NDWI of [[objects.features]] are found from the names.
- **Objects with attributes.** The multiresolution objects of scale 2 of
  [part 2](/tutorials/object-based-image-analysis/what-is-an-object/) (2,973 objects), and their spectral attributes
  ([part 4](/tutorials/object-based-image-analysis/describing-objects/) says what each means).
- **The template**, in the application: *Classify an image by rules* (group *Objects*). It runs on a synthetic image and
  has a class hierarchy of five classes; replace the image with yours with *Use my data* and move the thresholds.

## Build the model

### Steps 1 to 3: the image, the objects, their attributes

The first two nodes are those of the earlier parts: [[data.stac]] reads the scene of 9 August 2024 onto 10 m cells in
UTM zone 23S and [[raster.calc]] makes reflectance (0 to 1). They are in the full model below.
Then [[segment.multiresolution]] cuts the reflectance image at `scale = 2`, and [[objects.features]] measures the
objects.

The setting that matters is `features = ["spectral"]`: the mean, standard deviation, minimum, maximum and median of each
band, the `brightness`, and the three indices `ndvi`, `ndwi` and `ndmi` that [[objects.features]] makes from the **band
means** of each object (it finds the red, near-infrared, green and short-wave infrared bands by their names). The rules
read these columns by name.

!!! note
    An object's NDVI is made from its mean red and mean near infrared, not the mean of its cells' NDVI. For a
    homogeneous object the two are close; for a mixed one they differ, and the first is the one the rules see.

### Step 4: a new attribute with `objects.calc`

The table has no number for "how bright is the object in visible light". Brightness is the mean of **all five bands**,
and in the dry season the cerrado is bright in the near and short-wave infrared, so it does not separate the town from
the grassland. A new attribute does. [[objects.calc]] computes attributes from a script over the others, in the same
language as [[raster.calc]]: each column of the objects' table is a name, one statement a line, and `out name = value`
gives an attribute.

```text
out vis = (mean_red + mean_green + mean_blue) / 3
```

The node's `objects` output has the table with a new column, `vis`. The script can do more than arithmetic, as the second pass below and
[part 8](/tutorials/object-based-image-analysis/levels/) show: it knows the object's class (`cls`), its neighbours
(`nb.mean(x)`, `border_to("water")`) and its levels (`parent.x`, `children.area(cond)`).

!!! tip
    Where does a threshold come from? From the table. Read the values of the objects you know are water or town (in the
    application, click them in the Viewer: the inspector lists every attribute), look at the histogram of an attribute in the
    feature space, and put the ends of a ramp where the two groups part. On this clip the water objects have a median NDWI of
    0.64 and no other object is above 0.05. The visible brightness of the town's objects has a median of 0.103 (5th to
    95th percentile 0.077 to 0.193), that of the cerrado's 0.055 (0.042 to 0.073): the ramp from 0.06 to 0.09 lies where the two
    groups overlap least. The NDVI of the forest has a median of 0.62, that of the cerrado 0.43 and of the town 0.27. These
    numbers are of the classes the tree gives, so they are only a check that the ramps are placed in sensible gaps, not a
    validation.

NDBI, the normalised difference built-up index of Zha and colleagues (2003), is the usual idea for built-up land. On this dry
scene it is also high on the bare red soil patches of the cerrado, as much as on the town, so it is not used here. An index is
a hypothesis; look at it on the image before you rely on it.

### Step 5: the first rules

Add [[objects.rules]], connect `calc.objects` to `objects`, and write the classes. The setting `classes` is a list; each
class has:

- `name`: the class's name (text, no repeated names). The names are also what other rules and nodes call it;
- `rule`: an expression from [[objects.calc]] that gives a membership, 0 to 1 (a value outside is cut, a missing value is 0).
  Several lines are allowed, and the last is the value;
- `parent`: the name of another class of the list (optional). The parent may come before or after its child;
- `colour`: `#rrggbb`, for the maps (optional).

The first two classes, with no parent:

```toml
classes = [
  { name = "water", rule = "above(ndwi, -0.1, 0.2)", colour = "#2b6cb0" },
  { name = "land",  rule = "below(ndwi, -0.1, 0.2)" },
  # ...
]
```

`water` is 0 where the NDWI is −0.1 or less, 1 where it is 0.2 or more, and 0.5 at 0.05. The `land` rule is its opposite:
whatever is not water, to the degree it is not. The lake's NDWI is above 0.6, so its membership in water is 1 and in land
is 0. The roofs and the cerrado are below −0.2, so the reverse.

### Step 6: the class tree

Now the children. `forest` and `open` are two kinds of land: they say `parent = "land"` and only their own condition.
`built_up` and `cerrado` are two kinds of open ground, with `parent = "open"`.

```tfgraph
#! caption: The model: scene, UTM grid, reflectance, objects of scale 2, their attributes, the visible brightness, the class tree, and the map of the classes.
#! highlight: calc, rules
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
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }
note = "2,973 objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image" }
params = { features = ["spectral"] }
note = "Means, brightness, NDVI, NDWI per object."

[[nodes]]
id = "calc"
op = "objects.calc"
inputs = { objects = "feat.objects" }
params = { expression = "out vis = (mean_red + mean_green + mean_blue) / 3" }
note = "The brightness in visible light."

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "calc.objects" }
params = { classes = [
  { name = "water", rule = "above(ndwi, -0.1, 0.2)", colour = "#2b6cb0" },
  { name = "land", rule = "below(ndwi, -0.1, 0.2)" },
  { name = "forest", rule = "above(ndvi, 0.45, 0.6)", parent = "land", colour = "#2f7d3b" },
  { name = "open", rule = "below(ndvi, 0.45, 0.6)", parent = "land" },
  { name = "built_up", rule = "above(vis, 0.06, 0.09)", parent = "open", colour = "#c0504d" },
  { name = "cerrado", rule = "below(vis, 0.06, 0.09)", parent = "open", colour = "#d9b86c" },
] }
note = "Water, forest, built-up and cerrado by fuzzy rules in a class tree."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "rules.map" }
params = { title = "Classes by rules" }
```

=== "Application"

    Open the template *Classify an image by rules* (the library's *Templates*, group *Objects*) and run it (++ctrl+enter++ on
    the last node), or add the nodes above (click an operator in the library to add it, **drag from an output port to an
    input port** to connect, set the values in the **Node pane**). Select `rules` and press ++ctrl+enter++: the Viewer
    shows the classes. Click an object: the inspector lists its attributes, its membership in **every** class, the margin,
    and the value each rule had for it. *Feature space...* in the Viewer's side panel draws an attribute's histogram for the
    classes and can write a range you drag into `objects.rules` as a fuzzy rule (*Make a rule*). The classes of
    `objects.rules` are edited in the node's settings.

=== "Command line"

    Save the graph above as `rules.toml` and run it. The run's outputs are what the model's `[outputs]` name; add
    `[outputs.classes]` with `from = "rules.table"` for the table of memberships.

    ```bash
    terraflux run rules.toml --threads 4 --out output
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

The tree has the rules above. **How inheritance works, in numbers.** For an object with an NDWI of −0.5, an NDVI of 0.3
and a visible brightness of 0.12: `water` is 0; `land` is 1; `forest` is min(0, 1) = 0; `open` is min(1, 1) = 1;
`built_up` is min(1, 1) = 1, because its `vis` is above 0.09; `cerrado` is min(0, 1) = 0. The object is a town object, with
a membership of 1. An object with an NDVI of 0.5, which is on the forest's ramp, has `forest` 0.33 and `open` 0.67. The
children of `open` inherit that 0.67 as a ceiling: `built_up` is at most 0.67 and so is `cerrado`. The leaf of the largest
membership wins, so the object is forest if the open ground's children are below 0.33 and one of them if not.

The same classes written as a flat list, with no parents, need every condition in every rule:

```toml
classes = [
  { name = "water", rule = "above(ndwi, -0.1, 0.2)" },
  { name = "forest", rule = "below(ndwi, -0.1, 0.2) and above(ndvi, 0.45, 0.6)" },
  { name = "built_up", rule = "below(ndwi, -0.1, 0.2) and below(ndvi, 0.45, 0.6) and above(vis, 0.06, 0.09)" },
  { name = "cerrado", rule = "below(ndwi, -0.1, 0.2) and below(ndvi, 0.45, 0.6) and below(vis, 0.06, 0.09)" },
]
```

It gives **the same map**: the tree and the flat list differ in 0 of the 336,400 cells. The tree is not another method, it
is the shorter way to write this one, and the one in which a change to `land` (a new threshold) changes every child at once.

## Read the results

The classes `rules` gives are in three outputs: `objects` (the objects with their classes and memberships, for the next
node), `table` (one row per object: `id`, `class` as text, one column `m_<class>` per class and `margin`) and `map` (the
class of each cell's object). The numbers on the map follow the **order of the list**, parents included: water is 1, land
2, forest 3, open 4, built-up 5 and cerrado 6. Only the leaves appear on it.

![The classes by rules](/img/tutorials/object-based-image-analysis/rules-and-class-trees/classes.png "The objects of the clip classified by the class tree: the lake in blue, the dense vegetation in green, the town in red and the dry grassland of the cerrado in tan. The same clip as the image below.")

Drag the handle to compare the classes with the image they came from.

```tfcompare
caption: The Sentinel-2 clip (true colour) and the classes by rules. The red of the town follows the blocks, with the dark gaps between them (streets with trees, gardens, lots) in tan; the forest follows the gallery of the stream and the woodland to the south-east.
before: { src: /img/tutorials/object-based-image-analysis/rules-and-class-trees/clip.png, label: Image }
after: { src: /img/tutorials/object-based-image-analysis/rules-and-class-trees/classes_clean.png, label: Classes by rules }
```

The areas:

```tftable
align: { class: left }
digits: { hectares: 1, share: 1, cells: 0, objects: 0 }
labels: { class: Class, cells: Cells, hectares: Hectares, share: "Share of the clip (%)", objects: Objects }
caption: "The area of each class of the tree, from the map the node writes (a cell is 0.01 ha), and the number of objects of each."
data: /data/tutorials/object-based-image-analysis/rules-and-class-trees/areas.csv
```

The lake is 216.8 ha. The water of [part 7](/tutorials/object-based-image-analysis/water-by-thresholds/), found by Otsu's
cut of the NDWI on cells and not on objects, is 219.6 ha: 1.8 % more. Two methods that agree by that much on water, a class
a threshold separates well, are two views of one lake. The other classes have no such check here: nobody measured them. To
claim a number of hectares for forest or for the town you need reference data and an accuracy assessment
([part 5](/tutorials/object-based-image-analysis/classifying-by-samples/)); the table above is the map's own answer.

### Memberships and the margin

The map shows one class per object. The **memberships** show how it got there. [[objects.rules]] keeps all of them in the
table, and the figure draws six.

![Memberships and margin](/img/tutorials/object-based-image-analysis/rules-and-class-trees/memberships.png "The memberships of the objects of the clip, 0 (dark) to 1 (yellow), in four classes and in land (a parent), and the margin, the best membership minus the second best. Water is 1 only in the lake. Forest and cerrado trade places in the south-east, where the NDVI lies on the ramp between them.")

- **Water** is 1 in the lake and 0 elsewhere: the NDWI separates them so well that the ramp is hardly used.
- **Land** is its complement, and the parent of forest and of open ground (itself the parent of built-up and cerrado).
- **Forest** is 1 in the gallery forest along the streams and in the woodland of the south-east, and has **intermediate**
  values (green-blue) in the south, where the NDVI of the vegetation is between 0.45 and 0.6. These objects are partly
  forest and partly open ground.
- **Built-up** is high in the blocks of the town and in a few bright bare patches; it is 0 where the visible brightness is low.
- **Cerrado** is the open ground that is dark: high in the grassland, also in the dark gaps of the town.

The **margin** is the largest membership minus the second largest. It is 1 where the object belongs to one class
and nothing else competes, and close to 0 where two classes fit about equally. It is the map to look at before you trust the
classes: **278 objects, 8.9 % of the clip (299 ha), have a margin below 0.2**. They are in two places: the south-east, where
forest and cerrado meet on a continuum of NDVI, and the edges of the town, where mixed objects hold both roofs and trees.
Those are the objects to inspect, to give samples to, or to send to a second rule.

A lone threshold would have given the same classes and told you none of this. With the ramps almost steps (a width of 0.0002 in
the NDVI, the NDWI and the brightness, centred where the fuzzy ramps have their centres), the map is **identical, cell for
cell**. The ramps do not change what the objects become, because the object takes the larger membership and a ramp keeps the
order of the two around its centre. They change what you **know about each object**: how far it is from the decision.

### A minimum membership

`min`, the least membership for a class, is the other use of this. Set `min = 0.6` and an object whose best class fits it to
less than 0.6 stays unclassified, in no class and without a value on the map:

```toml
[nodes.params]
min = 0.6
classes = [ ... ]
```

![Objects with no class when min is 0.6](/img/tutorials/object-based-image-analysis/rules-and-class-trees/strict.png "The same classes with min = 0.6. The magenta objects are left unclassified: 278 objects, 8.9 % of the clip, where the rules are not sure enough.")

The default, 0.1, leaves every object classified here: each has a leaf with a membership of at least 0.1. With a
stricter `min` you choose to leave the doubtful objects empty, and finish them another way
(a classifier on samples, a second pass, a person with the object editor). It is the honest answer for an object the rules cannot decide.
Whatever is left unclassified has `cls == "unclassified"` in a later [[objects.calc]] or [[objects.rules]].

## A second pass that reads the neighbours

Some classes cannot be told by the object alone. A dark object that is tan on the map is "cerrado" by its colour, but one
that is **surrounded by roofs** is not open grassland: it is a gap in the town, a street with trees, a garden, an empty lot.
The rule needs the **class of the neighbours**.

A rule can read neighbours' classes, with `border_to("built_up")` (the share of the object's perimeter shared with objects of
that class) or `nb.count(cls == "water")` (how many neighbours satisfy a condition), but only the classes the objects
**already have**, from a node before it. So you chain a second [[objects.rules]] after the first. In it each class repeats
the old class with `cls == "water"` (the class an object has before this node) and the one to split gets a new rule:

```tfgraph
#! caption: A second pass on the classes of the first. The old classes are kept by cls; a cerrado object that shares at least half its perimeter with built-up objects becomes urban open ground.
#! highlight: second
#! fragment: true
[[nodes]]
id = "second"
op = "objects.rules"
inputs = { objects = "rules.objects" }
params = { classes = [
  { name = "water", rule = 'cls == "water"', colour = "#2b6cb0" },
  { name = "forest", rule = 'cls == "forest"', colour = "#2f7d3b" },
  { name = "built_up", rule = 'cls == "built_up"', colour = "#c0504d" },
  { name = "cerrado", rule = 'cls == "cerrado" and border_to("built_up") < 0.5', colour = "#d9b86c" },
  { name = "urban_open", rule = 'cls == "cerrado" and border_to("built_up") >= 0.5', colour = "#a7d676" },
] }
note = "Dark objects surrounded by the town."
```

`cls == "water"` is true (1) or false (0), so each of those rules is a plain yes or no, and `and` keeps the smaller of the two
values. The classes are new: the second pass has its own numbering (water 1, forest 2, built-up 3, cerrado 4, urban open 5).

![The second pass, over a detail of the town](/img/tutorials/object-based-image-analysis/rules-and-class-trees/second_pass.png "A detail of the town, 1.4 km wide, in the image (left), after the first pass (middle) and after the second (right). The pale green objects are the cerrado-coloured objects that share at least half their perimeter with built-up ones. The detail is the yellow box of the clip.")

![Where the detail is](/img/tutorials/object-based-image-analysis/rules-and-class-trees/detail_box.png "The detail of the figure above is the yellow box on the clip.")

The second pass moves **26,541 cells, 265 ha, in 302 objects**, from cerrado to urban open ground, and the cerrado falls
from 1,241 ha to 976 ha. On the detail the pale objects are the dark streets, the tree-lined avenues, the yards and the open
lots between the blocks. A reader can check this on the image itself, and a report should say how: the rule is
"half the object's perimeter touches the town", the 0.5 is a choice, and no field data says whether these are gardens.

!!! warning
    The second pass reads only the classes from **before** the node. A class made in the same node cannot be read in it. If
    the second pass should itself depend on a third (the urban open ground that touches the lake), chain a third node.

## Settings that change the answer

The thresholds are choices, and the answer depends on them. The slider moves the lower end of the forest's ramp, which is
0.45 in the tree, with the width fixed at 0.15 (the ramp is `above(ndvi, a, a + 0.15)` and the cerrado's ramp is its
complement). The decision between forest and open ground is made where the two memberships are equal, at the **middle** of
the ramp, `a + 0.075`.

```tfslider
label: Lower end of the forest's NDVI ramp
caption: "The classes for five positions of the forest ramp (the NDVI ramp is a to a + 0.15; the tree, the other rules and the objects are the same). The chart gives the hectares of each class in the clip."
frames:
  - { value: 0.3, src: /img/tutorials/object-based-image-analysis/rules-and-class-trees/forest_0_3.png, caption: "Forest from NDVI 0.3 to 0.45", stats: { Forest: "2,011 ha", "Built-up": "906 ha", Cerrado: "231 ha" } }
  - { value: 0.375, src: /img/tutorials/object-based-image-analysis/rules-and-class-trees/forest_0_375.png, caption: "Forest from NDVI 0.375 to 0.525", stats: { Forest: "1,362 ha", "Built-up": "994 ha", Cerrado: "792 ha" } }
  - { value: 0.45, src: /img/tutorials/object-based-image-analysis/rules-and-class-trees/forest_0_45.png, caption: "Forest from NDVI 0.45 to 0.6 (the tree of the page)", stats: { Forest: "896 ha", "Built-up": "1,010 ha", Cerrado: "1,241 ha" } }
  - { value: 0.525, src: /img/tutorials/object-based-image-analysis/rules-and-class-trees/forest_0_525.png, caption: "Forest from NDVI 0.525 to 0.675", stats: { Forest: "513 ha", "Built-up": "1,011 ha", Cerrado: "1,623 ha" } }
  - { value: 0.6, src: /img/tutorials/object-based-image-analysis/rules-and-class-trees/forest_0_6.png, caption: "Forest from NDVI 0.6 to 0.75", stats: { Forest: "315 ha", "Built-up": "1,011 ha", Cerrado: "1,821 ha" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/rules-and-class-trees/forest_sweep.csv
  x: lower_limit
  y: [forest_ha, built_up_ha, cerrado_ha]
  labels: { forest_ha: "Forest", built_up_ha: "Built-up", cerrado_ha: "Cerrado" }
  colors: { forest_ha: "#2f7d3b", built_up_ha: "#c0504d", cerrado_ha: "#d9b86c" }
  ylabel: "Hectares in the clip"
```

Read it from left to right.

- **The forest is the unstable class.** Its area goes from 2,011 ha to 315 ha, **a factor of six**, as one number moves by 0.3 in
  the NDVI. The area changes steadily with the threshold and does not flatten anywhere, which is what a histogram with
  no valley between two groups does (the NDVI of the whole clip has quartiles of 0.30, 0.41 and 0.53). The boundary between
  "forest" and "not forest" is a choice here, not something the image shows. The lowest frames call most of the south
  "forest"; the highest keep only the gallery along the streams and the darkest woodland.
- **The built-up class hardly moves** between 0.375 and 0.6 (994 to 1,011 ha): the town is bright in the visible, and its
  ramp is far from the forest's. At 0.3 the forest rule reaches the town's greener blocks and built-up falls to 906 ha.
- **The water does not move** because its rule was not touched. How steady its area is when its own threshold moves is in
  [part 7](/tutorials/object-based-image-analysis/water-by-thresholds/): a plateau, because the NDWI has a valley between
  water and land and the NDVI of forest and cerrado has none.

So the rules need to be **checked against something**. Place the ramps where the data say (the histograms of the attribute for
samples you know), and report the sensitivity as you report the threshold: "forest area between 513 and 1,362 ha for a
shift of the NDVI ramp of ±0.075".

## Common pitfalls

- **A rule on units that changed.** The ramps here are for reflectance 0 to 1. The same image as integers (0 to 10,000) puts
  every `vis` above 0.09. NDVI and NDWI are ratios and do not change; the brightness and the band means do. Say the units in the note of the node.
- **A class with no member.** An object with a membership of 0 in every class is unclassified even with `min = 0`. If a map has
  empty holes, look for a rule that is 0 where you expected 1. A single rule such as `above(class, 1, 2)` with no rule for the
  other class leaves every object of the other class unclassified; when no object is above the threshold, the **whole map is
  empty**. Give the other class its rule (`below(class, 1, 2)`), or give the objects a class in the first place.
- **Parents and numbers.** The map's numbers follow the list's order, parents included (here forest is 3 and built-up is 5, not 1
  and 2). Use the table's text `class`, or [[objects.to_map]] with `what = "class"`, and not the numbers, when you reuse them.
- **A class that becomes a parent.** A class that is nobody's parent is a leaf and can be an answer. The moment another class
  names it as `parent`, it stops being one and objects that were it go to its children. Check which classes are leaves when you
  change a tree.
- **Two classes with names that look the same.** `built-up` and `built_up` make the same column (`m_built_up`) and the
  operator refuses the list. Use one spelling, with underscores.
- **Using the second pass's classes in the first.** A rule reads what the objects were given **before** its node. In the same node
  `border_to("water")` is an error if the objects have no classes.
- **Rules that are not independent.** `and` is the minimum and does not add evidence: three conditions each at 0.6 give 0.6,
  not 0.9. A class that needs several weak signs to add up is a job for a classifier ([part 5](/tutorials/object-based-image-analysis/classifying-by-samples/)).
- **Checking the rules with the data you made them from.** The ramps came from this clip. The map shown is a description, not a
  validation.

## Going further

- Before this: [Describing objects](/tutorials/object-based-image-analysis/describing-objects/), where each attribute the
  rules read is defined; and [Water by thresholds](/tutorials/object-based-image-analysis/water-by-thresholds/), the simplest
  rule, one threshold.
- Next: [Levels](/tutorials/object-based-image-analysis/levels/) puts the objects at several scales at once, and rules that
  read the objects above and below.
- Samples instead of rules: [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/), with
  an accuracy assessment you can apply to the map of this page too.
- The operators of this page: [[objects.rules]], [[objects.calc]], [[objects.features]], [[segment.multiresolution]] and
  [[view.plot]].
- Coming from eCognition: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/) matches its class
  description, the minimum membership and `and (min)` to these settings.

## References

- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality
  multi-scale image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Benz, U. C., Hofmann, P., Willhauck, G., Lingenfelder, I., & Heynen, M. (2004). Multi-resolution, object-oriented fuzzy
  analysis of remote sensing data for GIS-ready information. *ISPRS Journal of Photogrammetry and Remote Sensing*, 58(3-4),
  239-258. https://doi.org/10.1016/j.isprsjprs.2003.10.002
- McFeeters, S. K. (1996). The use of the Normalized Difference Water Index (NDWI) in the delineation of open water
  features. *International Journal of Remote Sensing*, 17(7), 1425-1432. https://doi.org/10.1080/01431169608948714
- Tucker, C. J. (1979). Red and photographic infrared linear combinations for monitoring vegetation. *Remote Sensing of
  Environment*, 8(2), 127-150. https://doi.org/10.1016/0034-4257(79)90013-0
- Zadeh, L. A. (1965). Fuzzy sets. *Information and Control*, 8(3), 338-353.
  https://doi.org/10.1016/S0019-9958(65)90241-X
- Zha, Y., Gao, J., & Ni, S. (2003). Use of normalized difference built-up index in automatically mapping urban areas from TM
  imagery. *International Journal of Remote Sensing*, 24(3), 583-594. https://doi.org/10.1080/01431160304987
