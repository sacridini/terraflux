---
title: Coming from InterIMAGE
short: Coming from InterIMAGE
description: What is what when you know InterIMAGE's knowledge-based interpretation: the semantic network is the class tree, a top-down operator is a class's branch, the bottom-up rule is objects.resolve, the temporal node is objects.temporal; with the template Each class its own way.
order: 30
level: advanced
time: 40 min
template: Each class its own way
ops: [objects.resolve, objects.temporal, objects.transition_matrix, objects.features, objects.samples, objects.classify, control.group, raster.calc]
---

InterIMAGE (PUC-Rio and INPE, built on the GeoAIDA system of Hannover) interprets an image by a **semantic network**:
the classes of the scene as a tree (the urban area, and in it buildings and roads; the vegetation; the water), where
each class is found by an operator of its own, and where hypotheses that disagree are decided by rules. This page
maps that onto Terraflux, which has the same idea as its **class tree**, and works through the template **Each class
its own way**. It also covers InterIMAGE's multi-temporal analysis. If you do not know InterIMAGE, you can read it as
a tutorial on classifying an image with a different method for each class.

## The idea

A scene is not classified by one rule or one classifier. Water is told apart by one index; vegetation by another;
buildings by their height above the ground and their shape; a land use inside the urban area only makes sense
*inside* the urban area. InterIMAGE says it in two directions (Da Costa et al., 2009):

- **Top-down**: a class looks for its **hypotheses** inside the region its mother class found. The operator of the
  class makes objects, each a candidate for the class, with a measure of how well it fits.
- **Bottom-up**: the hypotheses of sibling classes overlap, and a **decision rule** settles them: each place goes to
  one class, and a hypothesis of a class the evidence does not support is dropped.

In Terraflux this is a **class tree**, a way of looking at ordinary nodes: each class is a group
(`control.group`), and one [[objects.resolve]] per mother decides among the daughters.

## What is what

| In InterIMAGE | In Terraflux |
|---|---|
| Semantic network | The class tree: the Model pane's **Classes** button shows it, a card for each class with how it is found, its rule, its samples and its area |
| A node (a class) of the network | A `control.group` with the settings `class` (the name), `parent_class` (its mother's name, none for a class of the scene) and `colour` |
| Top-down operator, the class's interpretation | The class's branch: the nodes inside its group. The body reads `@region` (its mother's region) and the data it needs, and gives a map `membership`, 0 to 1 |
| Hypotheses of a class | The cells and objects where the group's `membership` is above 0 |
| Bottom-up operator and decision rule | [[objects.resolve]]: in each cell the class of the greatest membership wins (if it reaches `min`; a tie goes by `order`) |
| Region of a daughter class | `region_<class>`: 1 where the mother class won, no value elsewhere, read by the daughter as `@region` |
| Conflicts between hypotheses | The `conflicts` output of [[objects.resolve]]: the cells of each class's hypotheses that another class won; and `doubt`, the second highest membership, to see where to look next |
| Training a classifier on samples inside a class | A class's recipe **by samples**: a random forest fitted on the objects the samples point to |
| Temporal node, multi-temporal interpretation | [[objects.temporal]] |
| Fuzzy transition matrix | The input `matrix` of [[objects.temporal]]: written by hand with `table.of`, or estimated with [[objects.transition_matrix]] |

## Open the template

**Templates > Objects (OBIA) > Each class its own way**. The model draws an image (put yours in its place with **File
> Use my data...**), cuts it into objects, and has a class tree of five classes:

- **Water**, found by its NDWI: no samples needed, one index tells it.
- **Vegetation**, found by its NDVI.
- **Built**, found by samples (cropland and urban objects), because no single index tells them from the rest.
- **Forest** and **Grassland**, inside the cells **Vegetation** won, each by samples.

Click **Classes** in the Model pane's bar. The tree shows a card for each class, with its mother, its colour, how it
is found (*water by NDWI*, *vegetation by NDVI*, *by samples*), the rule that gives its membership, how many objects
it was trained on, and, once the model has run, the area it covers. A click on a card selects its group in the diagram,
and the Viewer shows what the group gives, its membership map. Open the group to read its body: the branch is an
ordinary graph you can read and change.

### A class's branch

A branch is a small graph that follows one pattern. This is the body of the class Water:

```tfgraph
#! caption: The body of the class Water: its membership is a ramp of the NDWI, with no value outside the region.
#! fragment: true
#! highlight: m
[[nodes]]
id = "m"
op = "raster.calc"
inputs = { image = "@image", region = "@region" }
params = { expression = "ndwi = (image.green - image.nir) / (image.green + image.nir)\nout membership = clamp(ndwi / 0.3, 0, 1) * region" }
note = "Water by NDWI: the membership rises from 0 at an NDWI of 0 to 1 at 0.3 (the bands named green and nir)."
```

That is the body the recipe *water by NDWI* writes. The other recipes differ in the method, not in the pattern: the
branch's inputs are named `@image`, `@objects`, `@samples`, `@ndsm`...; the region of the mother is `@region`
(a map of 1 and no value, and the number 1, every cell, for a class of the scene); and the output `membership` is a
map from 0 to 1 with no value outside `@region`. Any method that gives such a map can be a class's branch: an
index, a classifier, a rule, a height threshold, or a model you wrote. That is what makes it top-down.

### Add a class

Press **Add a class...** under the cards. The form asks for a **Name**, a **Colour**, a **Mother** (none for a class of
the scene, or any class of the tree) and a **Recipe**:

| Recipe | What the branch does |
|---|---|
| water by NDWI | the membership rises from 0 at an NDWI of 0 to 1 at 0.3 (bands named `green` and `nir`) |
| vegetation by NDVI | from 0 at an NDVI of 0.2 to 1 at 0.5 (bands named `red` and `nir`) |
| buildings by height | from 0 at 2 m above the ground to 1 at 5 m (an nDSM, the height of the surface above the ground) |
| by samples | a random forest fitted on the objects the samples point to; the membership is 1 for the objects it gives this class |

Press **Add**. The branch is made, put into the decision of its mother's other daughters, and the change is one step
of undo. Then change what is inside: raise a threshold, replace a recipe's body by your own method.

## Decide between the hypotheses

The classes of one mother are decided by one [[objects.resolve]]: `members_of_<mother>` (a stack with a layer for each
daughter) feeds `resolve_of_<mother>` (for the classes of the scene, `members` and `resolve`). In each cell, **the
class of the greatest membership wins**, if it reaches `min` (under it the cell has no class); a tie goes by the
order. The cells won become objects, one for each connected area of a class (`merge = connected`), one for each class
(`global`), or one for each cell (`none`, to look at the arbitration itself).

Outputs worth reading:

- `class`: a map of the winning class, numbered by the classes' names in alphabetical order. The template's `leaves`
  node joins the roots' decision (Built 1, Vegetation 2, Water 3) and Vegetation's children (Forest 1, Grassland 2)
  into one map of leaf classes: 1 built, 2 forest, 3 grassland, 4 water.
- `membership`: the winning membership. Low values are decisions made on weak evidence.
- `doubt`: the second highest membership. Where it is close to the winning one, two classes disputed the cell: put
  samples there.
- `conflicts`: a table of the cells of each class's hypotheses that another class won. A big count between two
  classes says they are confused; a small one that their methods agree.

## Two dates

InterIMAGE's multi-temporal analysis binds the classes of two dates with a **fuzzy transition matrix**: from what a
place was at the first date, some classes are possible at the second and some are not. A forest cleared in ten years
may be grassland, cropland or town, but not water. The change is the same idea as the class tree, applied between
dates: knowledge of the world corrects what the spectra of a noisy image say.

```tfgraph
#! caption: Two dates classified alike, and the second one corrected by the matrix of what can become what.
#! fragment: true
#! highlight: temporal
[[nodes]]
id = "matrix"
op = "table.of"
params = { columns = ["class", "water", "forest", "grassland", "cropland", "urban"], rows = [
  ["water", 1, 0, 0, 0, 0],
  ["forest", 0, 1, 0.5, 0.3, 1],
  ["grassland", 0, 0.5, 1, 1, 1],
  ["cropland", 0, 0.3, 0.5, 1, 1],
  ["urban", 0, 0, 0, 0.1, 1],
] }

[[nodes]]
id = "temporal"
op = "objects.temporal"
inputs = { before = "classes_2000.objects", after = "classes_2020.objects", matrix = "matrix.table" }
```

The matrix has a row for each class of the first date (the column `class`, text) and a column for each class of the
second: a number from 0, impossible, to 1, common. [[objects.temporal]] links the first date's memberships to each
object of the second (the segmentations may differ), and each membership of the second date becomes the smaller of
itself and the largest of (the membership before, the possibility of the transition): the max-min composition of
fuzzy logic. With `both`, the first date is adjusted from the second the same way. Its outputs are the objects of the
second date with the adjusted classes, the map, and the table `change`: the class of each object of the second date
before and after. That table can enter the land change models (`change.transitions`, the Markov and cellular
automata models) as the observed dates.

If you do not have the knowledge to write the matrix, [[objects.transition_matrix]] estimates it from `samples` (a
table with the class of each sample at the two dates) or from `transitions` (the table of `change.transitions`): each
transition's count over the count of the most frequent from the same origin, so the most frequent is 1 and one never
seen is 0 (or `floor`). Where InterIMAGE searches the matrix with a genetic algorithm, this reads it straight from the
samples.

The template **Land cover change by objects** is this, with two dates of a drawn image, rules for the classes and a
hand-written matrix.

## What is different

- **The tree is a view, not a file format.** The model is an ordinary graph; the Classes button reads it by the
  convention of the `class`, `parent_class` and `colour` settings, so the same model can be edited as a graph and as
  a tree. A model that uses [[objects.rules]] for its hierarchy shows in the Classes view too, read only: change its
  classes in the node's settings.
- **The decision is per cell**, then the winning cells become objects. A hypothesis partly overlapped by another loses
  only the cells where another class won.
- **Operators are nodes.** Whatever method gives a map of membership can be a class's branch: you are not limited to a
  list of interpretation operators.

## Going further

- [Your first map by objects in 10 minutes](/tutorials/object-based-image-analysis/first-map-by-objects/).
- [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/): the class hierarchy with
  inherited rules, in one node.

## References

- Da Costa, G. A. O. P., De Pinho, C. M. D., Feitosa, R. Q., De Almeida, C. M., Kux, H. J. H., Fonseca, L. M. G., &
  Oliveira, D. A. B. (2009). InterIMAGE: uma plataforma cognitiva open source para a interpretação automática de imagens
  digitais. *Revista Brasileira de Cartografia*, 60(4). https://doi.org/10.14393/rbcv60n4-44859
- Mota, G. L. A., Feitosa, R. Q., Coutinho, H. L. C., Liedtke, C.-E., Müller, S., Pakzad, K., & Meirelles, M. S. P.
  (2007). Multitemporal fuzzy classification model based on
  class transition possibilities. *ISPRS Journal of Photogrammetry and Remote Sensing*, 62(3), 186-200.
  https://doi.org/10.1016/j.isprsjprs.2007.04.001
