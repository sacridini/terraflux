---
title: The vector calculator
short: Vector calculator
description: "Scripts over vector features in vector.calc and vector.filter: attributes of numbers and texts, the geometry's measures, attributes given with out and features kept by a condition, with a playground computed in your browser by Terraflux's own engine over Sibuyan's patches."
order: 20
level: beginner
time: 30 min
ops: [vector.calc, vector.filter, vector.geometry]
---

The vector calculator is two operators that read the same scripts: [[vector.calc]] gives every feature new
attributes computed from its others and from its geometry (the area in hectares, a class's name, a density), as
QGIS's field calculator does; [[vector.filter]] keeps the features where a condition holds (the forest patches
larger than 10 ha), as QGIS's Select by Expression or geopandas' `query`. Their language is the raster
calculator's, with texts and the geometry's measures added. This page has a playground over real features, then
the language, recipes and pitfalls.

## Try it

The playground computes over the 156 **patches** of Sibuyan Island's land use in 2011 (Philippines): each patch
is a polygon of connected cells of one class, made by [[vector.polygonize]], with these attributes:

| Attribute | What it is |
|---|---|
| `cover` | the class's name, a **text**: `"forest"`, `"coconut"`, `"grassland"`, `"rice"` or `"other"` |
| `value` | the class's code, 1 to 5 |
| `cells`, `area` | its cells of 250 m, and its area in m², from [[vector.polygonize]] |
| `elevation`, `slope` | its mean elevation (m) and slope (degrees), from [[vector.zonal]] |
| `dist_roads`, `pop_density` | its mean distance to a road (m) and population density |

The coordinates are in metres (UTM zone 51N), so `geometry.area` is in m² and `geometry.x`, `geometry.y` in
metres. Switch between [[vector.calc]] and [[vector.filter]] at the top; a script of `vector.calc` gives its
attributes with `out`, one of `vector.filter` ends with its condition.

```tfcalc
kind: vector
presets:
  - name: Areas in hectares, and the large patches
    script: |
      # the geometry's area is in m² (the coordinates are metres)
      out area_ha = geometry.area / 10000
      out large = area_ha >= 100
  - name: A text from a text
    script: |
      out kind = "natural" if cover == "forest" or cover == "grassland" else "farmed"
  - name: Shape and share of the island
    script: |
      # 1 for a circle, 0.785 for a square, less for long or ragged shapes
      out compact = 4 * pi * geometry.area / geometry.perimeter ** 2
      out share = geometry.area / geometry.area.sum()
  - name: Classes by conditions, None where they do not apply
    script: |
      out risk = "low"
      risk[dist_roads < 300] = "high"     # forest near a road
      risk[cover != "forest"] = None      # forest only
  - name: People in each patch
    script: |
      out people = pop_density * geometry.area / 1e6   # per km², times km²
      out total = people.sum()                         # one number, for every feature
  - name: "Filter: forest above 200 m"
    op: filter
    script: |
      cover == "forest" and elevation > 200
  - name: "Filter: by the node's inputs"
    op: filter
    inputs: { wanted: "forest", min_ha: 10 }
    script: |
      # wanted and min_ha are inputs of the node: change them above
      area_ha = geometry.area / 10000
      cover == wanted and area_ha >= min_ha
  - name: "Filter: the patches of the north"
    op: filter
    script: |
      geometry.y > 1370000
  - name: "One expression, its column named"
    column: area_ha
    script: |
      geometry.area / 10000
```

!!! tip "How to use the playground"
    - **The map** colours the features by an attribute: the first one the script gives, or any other chosen
      above it. Point at a feature to read its attributes (those given in orange); a filter fades the features it
      leaves out.
    - **The table** under the map has every attribute, those given shaded; click a column to sort by it, point
      at a row to find its feature on the map.
    - **Completions**: type `geometry.` for the measures, or the first letters of an attribute, a function or a
      method; ++ctrl+space++ asks at any point.
    - **Every name** adds the script's other names to the table (in grey).
    - **The console** computes one line in the script's scope: `cover.count()`, `geometry.area.sum() / 1e6`.
    - **The node's inputs** (`wanted`, `min_ha`) have boxes at the top: a number, or a text in quotes.

## The idea

A vector layer is a table with a geometry in each row: one **feature** per row (a patch, a reserve, a road, a
plot), its **attributes** in the columns. A script over features computes, as the raster calculator does over
cells, **for every feature at once**: `geometry.area / 10000` is the area of every patch, `cover == "forest"`
is 1 for the forest patches and 0 for the others. So everything of [the raster calculator](../raster/) holds,
with "feature" for "cell" and "attribute" for "map": the operators, the functions, `a if c else b`,
`name[condition] = value`, `None`, and the numbers of all the features (`x.sum()`, `x.percentile(90)`).

Three things are new:

!!! science "Key idea: attributes, texts and the geometry"
    - An **attribute** is a value per feature, a number or a **text**. Texts are compared with texts
      (`cover == "forest"`), chosen (`"high" if x > 10 else "low"`) and counted (`cover.count()`), but not added
      or compared with numbers: `cover > 3` is refused, with what to write instead.
    - The **geometry** is read by its measures: `geometry.area`, `geometry.perimeter`, `geometry.length`,
      `geometry.x` and `geometry.y` (the centroid), in the features' units, as [[vector.geometry]] gives them.
    - There are **no neighbourhoods**: a feature has no cells around it, so `nbmean` and the like are for maps.
      What is near a feature is a matter for [[vector.spatial_join]] or [[vector.distance]].

## vector.calc and vector.filter

=== "vector.calc"

    Every `out name = value` gives the features an attribute: a new one, or one of the same name **replaced**
    (`out cover = ...` rewrites `cover`). `out name` alone gives a name of the script. The other names of the
    script stay in it.

    ```python
    density = pop / geometry.area
    out density
    out dense = density > density.percentile(75)
    ```

    A script **without** `out` is one expression whose value goes to the attribute named by the `column`
    setting: `geometry.area / 10000` with `column = "area_ha"`. A script with `out` and a column is refused:
    the `out` lines name the attributes, leave the column empty.

=== "vector.filter"

    The script's **last line** is the condition: a feature is kept where it is true (not 0), and left out where
    it is false or missing. The lines before give names to the values it reads; `out` is refused, since the
    filter gives features, not attributes.

    ```python
    area_ha = geometry.area / 10000
    cover == "forest" and area_ha >= 10
    ```

    A condition of texts is refused (`cover` alone): a feature is kept where a comparison holds
    (`cover == "forest"`).

Both give the features (`vector`) and their attributes as a table (`table`).

## The language over features

### Attributes and inputs

- An attribute by its name: `elevation`, `cover`. A name the features do not have is told with the attributes
  they have.
- `feature`, the number of the feature (1, 2, ...), unless an attribute has that name.
- The node's other inputs by their names: numbers and texts (`cover == wanted`, `area_ha >= min_ha`), connected to
  another node's output or to an input of the model; in the model file a number can also be written as it is
  (`min_ha = 10`), and a text comes from an input of the model (`wanted = "$wanted"`, with `[inputs.wanted]`,
  `kind = "value"`, `value = "forest"`). Features and tables among the inputs are not read.
- An attribute that is not a valid name of the language is not reachable; rename it first (a name begins with a
  letter or `_`, and has letters, digits and `_`).

### Texts

| | |
|---|---|
| A text | in quotes, `"forest"` or `'forest'`; `\"` and `\'` inside |
| Compared | `cover == "forest"`, `cover != "other"`, and in the order of their characters, `"a" <= name < "n"` |
| Chosen | `"natural" if cover == "forest" else "farmed"`, `coalesce(kind, "unknown")` |
| Changed where | `risk[dist_roads < 300] = "high"` |
| Missing | `None`; an empty text is a missing value: `isnull(cover)` |
| Counted | `cover.count()`, the features with a text |
| Read as a number | `float(code)`: `"12.5"` is 12.5, a text that is not a number is missing |

A text is not a number: `cover + "s"`, `cover > 3`, `cover.mean()` and `abs(cover)` are refused, each with
the form to write. Texts are compared as they are written, capital letters and accents included: `"Forest"` is
not `"forest"`.

!!! note "Codes stored as texts"
    Shapefiles and spreadsheets often keep codes as texts (`"0101"`, `"12.5"`). `float(code) > 100` compares them
    as numbers; `code == "0101"` as texts, which keeps the leading zero.

### The geometry's measures

| | |
|---|---|
| `geometry.area` | the area of polygons, their holes left out (0 for points and lines) |
| `geometry.perimeter` | the length of polygons' rings, the holes' included |
| `geometry.length` | the length of lines (0 for points and polygons) |
| `geometry.x`, `geometry.y` | the centroid: of the area (polygons), of the length (lines), of the points |

They are in the units of the features' coordinates: square metres and metres in a projected system (UTM, an
Albers or Lambert projection), **square degrees** in geographic coordinates (longitude and latitude), which mean
nothing as an area. Reproject the features first ([[vector.reproject]]), or use [[vector.geometry]] with
`geodesic` for areas in m² on the ellipsoid.

`geometry` alone is refused (it is read by its measures), and so is giving it a value.

### Numbers of all the features

`x.sum()`, `x.mean()`, `x.sd()`, `x.median()`, `x.min()`, `x.max()`, `x.count()` and `x.percentile(p)` are one
number from every feature with a value, as over the cells of a map. They give shares (`geometry.area /
geometry.area.sum()`), ranks of a kind (`x >= x.percentile(90)`, the top tenth) and totals. A number given to an
attribute with `out` is given to every feature (`out total = people.sum()`).

### Missing values

A feature without a value (`None`, an empty text) gives missing values in
what is computed from it, as a missing cell does, `and` and `or` included: where `cover` is missing,
`cover == "forest" or elevation > 50` is missing, not 1. A filter leaves such features out; `isnull(x)` finds
them and `coalesce(x, y)` fills them.

## Recipes

| To | Write |
|---|---|
| an area in hectares, in km² | `geometry.area / 10000`, `geometry.area / 1e6` |
| a density | `pop / (geometry.area / 1e6)` (per km²) |
| a share of the total | `geometry.area / geometry.area.sum()` |
| the largest tenth | `geometry.area >= geometry.area.percentile(90)` |
| a compactness | `4 * pi * geometry.area / geometry.perimeter ** 2` |
| a class from numbers | `"small" if area_ha < 10 else "medium" if area_ha < 100 else "large"` |
| a class from texts | `"natural" if cover == "forest" or cover == "grassland" else "farmed"` |
| a code made a name | `"forest" if value == 1 else "coconut" if value == 2 else "other"` |
| a text code as a number | `float(code)` |
| features in a box | `500000 <= geometry.x < 510000 and geometry.y > 1370000` |
| features of a list of classes | `cover == "rice" or cover == "coconut"` |
| a missing value filled | `coalesce(owner, "unknown")` |
| a number per feature | `feature` (1, 2, ...) |

## In the application, the model file and the assistant

=== "Application"

    Add a [[vector.calc]] or a [[vector.filter]] from the library's **Vector analysis** subject (or ++slash++ to
    search) and connect the features to its `vector` input. The script is its `expression` setting in the Node
    pane: a box of one line for a short expression, of several lines once it is longer than 40 characters or has
    a line break (`;` also separates two statements on one line). A script with an error makes the run stop at
    the node, with the message and its line. Connect a number or a text to a `+ new input` to give the script an
    input of that name.

=== "Model file"

    ```toml
    [[nodes]]
    id = "patches"
    op = "vector.read"
    params = { path = "patches.gpkg" }

    [[nodes]]
    id = "attrs"
    op = "vector.calc"
    inputs = { vector = "patches.vector" }
    params = { expression = """
    out area_ha = geometry.area / 10000
    out kind = "natural" if cover == "forest" else "farmed"
    """ }

    [[nodes]]
    id = "large_forest"
    op = "vector.filter"
    inputs = { vector = "attrs.vector", min_ha = 10 }
    params = { expression = 'kind == "natural" and area_ha >= min_ha' }
    ```

    A script without `out` names its attribute in `column`: `params = { expression = "geometry.area / 10000",
    column = "area_ha" }`.

=== "Assistant"

    The assistant (and any client of `terraflux mcp`) writes these with the tool `write_calc` and `op`
    `vector.calc` or `vector.filter`: the script is checked (its language, the kinds of its values, every name an
    attribute, an input or given before) and run on a sample of the features (up to 5,000) **before** the model
    changes. The reply tells each attribute given (its classes or texts and their shares, or its least, mean and
    largest values) or how many features the filter keeps, or why it was refused. `explain_calc` takes such a
    script apart: the features and their attributes, and what every name holds.

## Common pitfalls

- **Texts in quotes.** `cover == forest` reads an attribute or an input named `forest`; the text is
  `cover == "forest"`.
- **Capital letters and spaces count.** `"Forest"`, `"forest "` and `"forest"` are three texts. Look at the
  values in the table (or with `cover.count()` in the console) before you compare them.
- **Areas in degrees.** Features in longitude and latitude have areas in square degrees. Reproject them, or use
  [[vector.geometry]] with `geodesic`.
- **`area` and `geometry.area`.** An attribute called `area` is whatever the file holds (here the cells' area from
  [[vector.polygonize]], equal to the geometry's); `geometry.area` is always measured from the geometry, after a
  clip or an overlay too.
- **`out` in a filter, or none in a calculation.** [[vector.filter]] takes a condition, not `out`;
  [[vector.calc]] without `out` needs its `column`.
- **A missing value spreads**, through `and` and `or` too: fill it with `coalesce`, or test it with `isnull`.
- **`round` and `%`** are not Python's for halves and negative numbers: `round(2.5)` is 3, `-7 % 3` is -1.

## Going further

- [The raster calculator](../raster/): the language over maps, with stacks and neighbourhoods.
- [Vector data](../../tutorials/getting-started/vector-data/): reading features, burning them into a grid,
  statistics of a map in each polygon, values at points.
- The operator reference: [[vector.calc]], [[vector.filter]], [[vector.geometry]], [[vector.zonal]],
  [[vector.polygonize]], [[vector.spatial_join]].
