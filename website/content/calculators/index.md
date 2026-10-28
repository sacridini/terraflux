---
title: The calculators
short: Overview
description: "Terraflux's two calculators, one language: map algebra over the cells of maps (raster.calc) and over the attributes and geometries of features (vector.calc, vector.filter), with playgrounds you can type in, computed in your browser by Terraflux's own engine."
order: 0
---

Most analyses need, somewhere between reading the data and running the method, a few lines of arithmetic: a mask
of the forest, a score from 0 to 1, the share of each patch in a landscape, the parcels larger than a threshold.
Terraflux has two **calculators** for that, both written in the same small language, which reads like Python:

- the **raster calculator**, [[raster.calc]], computes **maps** cell by cell from other maps and stacks of maps
  (a land use, the bands of an image, the drivers of a land change model), and numbers from all their cells;
- the **vector calculator**, [[vector.calc]] and [[vector.filter]], computes **attributes** of vector features
  (polygons, lines, points) from their other attributes, numbers and texts, and from their geometry, and keeps
  the features that meet a condition.

Each has a page of its own, with a **playground** at the top: a script over real data from Sibuyan Island
(Philippines) that you can change and run as you type. The playgrounds are computed in your browser by
Terraflux's own engine, the same code as the application's built to WebAssembly, so they give the numbers and the
messages the application gives.

- [**The raster calculator**](raster/): map algebra over maps and stacks: conditions, missing values, stacks and
  their layers, neighbourhoods, numbers of a whole map, outputs. Its playground computes over Sibuyan's land use of
  1997 and 2011 and seven of its drivers.
- [**The vector calculator**](vector/): scripts over features: texts (`cover == "forest"`), the geometry's
  measures (`geometry.area`), attributes given with `out`, features kept with a condition. Its playground
  computes over the 156 patches of Sibuyan's land use of 2011.

## Which one?

| You have | You want | Use |
|---|---|---|
| maps or stacks on one grid | new maps, cell by cell (a mask, a score, an index, a reclassification by ranges) | [[raster.calc]] |
| maps or stacks | one number of all the cells (the mean, the share of forest, a percentile) | [[raster.calc]], with `out name = x.mean()` |
| features with attributes | new attributes for every feature (the area in hectares, a class name, a density) | [[vector.calc]] |
| features with attributes | only the features that meet a condition (the forest patches above 10 ha) | [[vector.filter]] |
| features and a map | the map's values in each polygon or at each point | not a calculator: [[vector.zonal]], [[vector.extract]] |
| a table of numbers | new columns, rows kept | not these: [[table.calc]], [[table.filter]] |

When the step has an operator of its own, prefer it: a reclassification table reads better in
[[raster.reclassify]], a distance in [[distance.euclidean]], polygons from a map of classes in
[[vector.polygonize]]. A calculator is for the small steps between them, and for keeping several such steps in
one node.

## One language

Both calculators read the same language, and what you learn in one holds in the other:

- **Python's syntax**: `=` gives a name a value, `==` compares; `and`, `or`, `not`; `a if c else b`; `**` for
  powers; `#` begins a comment. The forms of other languages (`^`, `&&`, `||`, `!`, `null`, `ln`,
  `if c then a else b`, `c ? a : b`) are refused with the one to write.
- **Scripts of several lines**: each line gives a name a value that the later lines read, and the lines that begin
  with `out` are what leaves the node. A single expression is a script of one line.
- **Whole layers at once**, as numpy computes: `x * 2` doubles every cell (or every feature), and
  `x[x < 0] = 0` changes only the cells (features) where the condition holds.
- **Missing values** (`None`): a value computed from a missing one is missing, `isnull(x)` tests it and
  `coalesce(x, y)` gives `y` where `x` has none.
- **Numbers of all the cells (features)**: `x.mean()`, `x.sum()`, `x.percentile(90)`... are one number, which the
  next lines can use (`x > x.percentile(90)`).

What differs is what a value is. In a raster script, a value is a **map** (a value per cell), a **stack** of maps,
or a **number**. In a script over features, a value is an **attribute** (a value per feature), a number **or a
text**; the geometry is read by its measures, and there are no neighbourhoods (a feature has no neighbouring cells).

## Where they are in the application

!!! app "In the application"
    [[raster.calc]] is in the library's **Map algebra** subject, [[vector.calc]] and [[vector.filter]] in its
    **Vector analysis** subject; the quick search (++slash++) finds them by name. A [[raster.calc]]'s script has an editor of its own in the Node pane (its words in colour,
    its first error underlined) and the full **Calculator** pane (**Expand**, or **View > Calculator**), with names
    completed, every output previewed as you type, and a console. A [[vector.calc]]'s or [[vector.filter]]'s
    script is its `expression` setting in the Node pane, on several lines once it is long (`;` also separates two
    statements on one line).

The assistant and the MCP server write and explain both: their tools `write_calc` and `explain_calc` check a
script and try it on a sample of the cells or features before the model changes, and tell what each output gives.

## Going further

- [The raster calculator](raster/) and [the vector calculator](vector/): the languages in full, with playgrounds.
- [Map algebra from scratch](../tutorials/building-models/map-algebra/): the ideas behind cell-by-cell operations,
  built step by step over Sibuyan's maps.
- [The raster calculator, a tutorial](../tutorials/building-models/calculator/): one script in the Calculator pane,
  its console, and the script saved as a node of your own.
- [Vector data](../tutorials/getting-started/vector-data/): reading features, burning them into a grid, statistics
  of a map in each polygon.
