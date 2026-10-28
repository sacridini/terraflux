---
title: The raster calculator
short: Raster calculator
description: "Map algebra over maps and stacks in raster.calc, in Python's syntax, with a playground computed in your browser by Terraflux's own engine over Sibuyan's land use and drivers, and the language in full, recipes and pitfalls."
order: 10
level: beginner
time: 30 min
ops: [raster.calc]
---

The raster calculator, [[raster.calc]], computes new maps from other maps, cell by cell, with a short script that
reads like Python: `forest = lu == 1`, `out lost = forest._1997 and not forest._2011`. This page has a playground
where you can write such scripts over real data and see every result at once, then the language part by part,
recipes for the tasks that come up in most analyses, and the mistakes worth knowing before they cost you a day.

## Try it

The playground below computes over Sibuyan Island (Philippines) at 250 m, 128 × 108 cells: `lu`, its land use in
1997 and 2011 (a stack of two layers, `lu._1997` and `lu._2011`: 1 forest, 2 coconut, 3 grassland, 4 rice,
5 other; the sea has no value), and `d`, seven of its drivers (a stack: `d.elevation` in m, `d.slope` in degrees,
`d.dist_roads`, `d.dist_urban`, `d.dist_streams` and `d.dist_sea` in m, `d.pop_density`). Pick an example, or
change the script: it runs as you type.

```tfcalc
kind: raster
presets:
  - name: The forest's years and its loss
    script: |
      # lu: the land use of 1997 and 2011 (a stack)
      forest = lu == 1                  # a stack: the forest of each year
      out years = sum(forest)           # in how many years: 0, 1 or 2
      out lost = forest._1997 and not forest._2011
      out lost_km2 = lost.sum() * 0.0625  # cells of 6.25 ha
  - name: A habitat index and its best quarter
    script: |
      # a score from 0 to 1 for elevation and for roads, and their geometric mean
      si_elev = clamp((1200 - d.elevation) / 900, 0, 1)
      si_road = clamp(d.dist_roads / 3000, 0, 1)
      out hsi = sqrt(si_elev * si_road)
      hsi[lu._1997 != 1] = None         # the forest of 1997 only
      out best = hsi >= hsi.percentile(75)
      out mean_hsi = hsi.mean()
  - name: Steep forest far from roads (a number input)
    inputs: { max_slope: 20 }
    script: |
      # max_slope is an input of the node: change it in the box above
      out steep = lu._2011 == 1 and d.slope > max_slope and d.dist_roads > 2000
      out steep_km2 = steep.sum() * 0.0625
  - name: Classes by ranges
    script: |
      # three elevation belts, as a reclassification would make them
      out belt = 1 if d.elevation < 300 else 2 if d.elevation < 800 else 3
      out middle = 300 <= d.elevation < 800   # comparisons chain, as in Python
  - name: Rescale and combine scores
    script: |
      e = d.elevation
      out scaled = (e - e.min()) / (e.max() - e.min())   # from 0 to 1
      w = 0.6                                         # the weight of roads
      out score = w * clamp(d.dist_roads / 3000, 0, 1) + (1 - w) * (1 - scaled)
  - name: Neighbourhoods and edges
    script: |
      forest = lu._2011 == 1
      out share = nbmean(forest, 2)     # the share of forest in the 5 x 5 cells around
      share[isnull(lu._2011)] = None    # (the sea next to the coast has a share too)
      farm = lu._2011 == 2 or lu._2011 == 4
      out edge = forest and nbmax(farm, 1) == 1   # forest touching coconut or rice
  - name: Missing values
    script: |
      land = lu._2011
      land[land == 5] = None            # "other" made missing
      out land
      out filled = coalesce(land, 0)    # 0 where land has no value
      out missing = isnull(land).sum()  # the sea and "other"
  - name: Change between the years, coded
    script: |
      # from-to codes: 12 is forest in 1997, coconut in 2011
      out change = lu._1997 * 10 + lu._2011
      out changed = lu._1997 != lu._2011
      s = lu
      s.changed = changed               # a layer added to a stack
      out s
```

!!! tip "How to use the playground"
    - **Completions.** Type `d.` or `lu.` and the layers are offered; a function's or a name's first letters offer
      the rest. ++arrow-down++ / ++arrow-up++ choose, ++tab++ or ++enter++ take it, ++ctrl+space++ asks at any
      point.
    - **Errors** are underlined where they are and told under the script, with their line. The results then fade
      until the script is right again.
    - **The maps.** Point at a map to read its value at that cell, with the values of the inputs the script reads
      there. Maps of a few whole numbers are drawn as classes (with their shares), the others on a ramp from
      their least to their largest value; the checkered cells have no value.
    - **Every name** (at the top right) shows what every name of the script holds at its end, not only its
      outputs.
    - **The console** computes one line in the script's scope: `lost.sum()`, `d.elevation.max()`,
      `hsi.percentile(90)`.
    - A number input of the node (`max_slope`) has a box of its own at the top.

    The playground computes every cell; the application's Calculator pane previews on a sample of a large map,
    and the node computes every cell when the model runs.

## The idea

A script is a short program over whole maps. Each line gives a name a value (`forest = lu == 1`), the later lines
read the earlier names, and the lines that begin with `out` are the node's outputs. Like numpy, the library most
ecologists who program know (Harris et al., 2020), it computes on whole arrays at once: `x * 2` doubles every
cell, and `x[x < 0] = 0` changes only the cells where the condition holds. The operations are Tomlin's local and
focal ones (Tomlin, 1990): a cell's value from the values of the same cell in other maps, or of the cells around
it.

A script handles three kinds of values; knowing which one you have explains most of what it does.

!!! science "Key idea: maps, stacks and numbers"
    - A **map** has one value per cell. Arithmetic, comparisons and functions act cell by cell.
    - A **stack** has several maps of one grid, each a **layer** with a name: the years of a land use series, the
      bands of an image, a model's drivers. `s.elevation` is one layer (a map); `s * 2` and `s == 1` act on every
      layer and give a stack of the same layers; `sum(s)`, `mean(s)`... reduce the layers at each cell to a map.
    - A **number** is one value for the whole area: `x.mean()`, `x.sum()`, `x.percentile(75)` reduce all the cells
      of a map (or of every layer of a stack) to one number. The later lines can use it (`x >= x.percentile(75)`),
      and it can leave the node, where another node's setting reads it.

## The language

### Values and names

- **Numbers** (`3`, `0.25`, `1e6`), `pi`, `True` and `False` (1 and 0), and `None`, a missing value.
- **The node's inputs** by the names of its ports: maps, stacks and numbers. A stack's layers are `s.layer`. A
  layer whose name is not a name of the language gets one: `1997` is `_1997` (a name cannot begin with a digit),
  `NDVI-2020` is `NDVI_2020`, a band without a name `b1`, `b2`...
- **Names you give**: `name = value`. A name is a value, not a formula: `y = x`, then `x = x * 10`, leaves `y`
  as `x` was. A name given in the script hides an input of the same name from there on.
- **Lines**: one statement a line, or several separated by `;`; `#` begins a comment; a line goes on after an
  open bracket until it closes.

### Arithmetic, comparisons, logic

| | |
|---|---|
| Arithmetic | `+ - * / %`, and `**` for a power: `(nir - red) / (nir + red)`, `d.dist_roads ** 0.5` |
| Comparisons | `< <= > >= == !=`, giving 1 (true) or 0 (false), chained as in Python: `300 <= d.elevation < 800` |
| Logic | `and`, `or`, `not`: `lu._2011 == 1 and d.slope > 20` |
| A choice | `a if c else b`, and in a row: `1 if e < 300 else 2 if e < 800 else 3` |

Every operation is made cell by cell. A map with a number applies the number to every cell (`d.elevation / 1000`);
two maps pair their cells (`lu._1997 * 10 + lu._2011`).

### Functions

| | |
|---|---|
| `abs sqrt exp log log10 floor ceil round` | of every cell; `log` is the natural logarithm, `round` rounds a half away from 0 |
| `min(a, b, ...)`, `max(a, b, ...)` | the least, the largest of several maps or numbers at each cell: `max(d.elevation, 100)` |
| `clamp(x, lo, hi)` | `x` held between `lo` and `hi`: `clamp(d.dist_roads / 3000, 0, 1)` |
| `isnull(x)` | 1 where `x` has no value, 0 elsewhere |
| `coalesce(x, y)` | `x`, and `y` where `x` has no value |
| `nbsum nbmean nbmin nbmax nbcount (x, r)` | over the square of `2r + 1` cells on a side around each cell |

### Changing some cells

`name[condition] = value` changes the cells where the condition holds; the others keep their value. It is
numpy's rule, and it reads better than a nested `a if c else b`:

```python
x = d.slope
x[x > 30] = 30                    # slopes above 30 degrees capped
x[lu._2011 == 5] = None           # and "other" left out
```

Several such lines apply in order, a later one over an earlier one. A cell where the condition has no value
(over the sea, `lu._2011 == 5` has none) is not changed.

`s.layer = value` replaces a layer of a stack or adds one: `s.changed = lu._1997 != lu._2011`.

### Missing values

A cell without a value (the sea, a cloud, a mask) is `None`, and a value computed from a missing one is missing:
`None + 1`, `None > 3` and `not None` are `None`, and so is `0 / 0`; `1 / 0` is infinite (`inf`), and `sqrt` of
a negative number is missing. Three tools handle them:

- `isnull(x)`, which is 1 where `x` has no value: `isnull(lu._2011).sum()` counts the cells of the sea;
- `coalesce(x, y)`, which fills: `coalesce(land, 0)`;
- `x[condition] = None`, which takes cells away: `hsi[lu._1997 != 1] = None`.

The numbers of a whole map (`x.mean()`, `x.sum()`) and the reductions of a stack's layers (`mean(s)`) leave the
missing cells out; `min(a, b)` of several maps is missing where any of them is.

!!! warning "`and` and `or` with a missing value"
    In Python, `True or None` is `True`. In map algebra, a missing value makes the result missing: over the sea,
    `isnull(lu._2011) or lu._2011 == 1` is `None`, not 1. Test the missing cells with `isnull` alone, or fill
    them first with `coalesce`.

### Stacks and their layers

An operation on a stack is made on each layer, with a number or a map (`lu == 1`, `d * 2`), and layer by layer
with another stack of as many layers. Its layers keep their names, so `(lu == 1)._1997` is the forest of 1997.

At each cell, over the layers (the missing ones left out), a map:

| | |
|---|---|
| `mean(s) sd(s) median(s) sum(s) min(s) max(s)` | the mean, standard deviation, median, sum, least, largest of the layers |
| `count(s)` | how many layers have a value |
| `percentile(s, p)` | the `p`-th percentile of the layers |

`sum(lu == 1)` is in how many years a cell was forest. `count(lu == 1)` is **not**: it counts the layers with a
value, 2 on all of the island.

### Numbers of all the cells

`x.mean() x.sd() x.median() x.sum() x.count() x.min() x.max() x.percentile(p)`, with the brackets, give one number
from every cell of `x` with a value (of every layer of a stack). The standard deviation is the sample one
(dividing by \(n - 1\)), and percentiles interpolate between the two nearest values, as numpy and R do by default.

`mean(s)` and `s.mean()` answer different questions: the first is a map (the mean of the layers at each cell),
the second one number (the mean of every cell of every layer). `mean(x)` of a single map is refused, and points to
`x.mean()`.

A number depends on the cells its map has **when it is computed**: in the habitat example, `hsi.percentile(75)`
comes after `hsi[lu._1997 != 1] = None`, so it is the best quarter of the forest of 1997; move it before, and it
is of the whole island.

### Neighbourhoods

`nbsum(x, r)`, `nbmean(x, r)`, `nbmin(x, r)`, `nbmax(x, r)` and `nbcount(x, r)` read the square of radius `r` cells
around each cell, the cell included (`r = 1`: 3 × 3 cells; `r = 2`: 5 × 5), the missing cells left out; `nbcount`
counts those with a value. `nbmean(lu._2011 == 1, 2)` is the share of forest in the 5 × 5 cells around each
cell, a moving window as in FRAGSTATS (McGarigal & Marks, 1995); `nbmax(farm, 1) == 1` marks the cells next to
farmland. `r` is a whole number or a number input. For larger windows, circles or weights, use the focal
operators ([[raster.focal]]).

Since the missing cells are left out, a cell without a value next to cells with one gets a value: the sea along
the coast has a share of forest. Take it away with `share[isnull(lu._2011)] = None`.

### Outputs

- `out name = value` gives a name and makes it an output of the node; `out name` alone makes an output of a name
  given before. An output takes the value its name has **at the end** of the script.
- An output is a map, a stack or a number, an output port of the node named as in the script: `calc.years`,
  `calc.lost`, `calc.lost_km2`.
- A script without `out` gives its last value as the output `map`: `lu._2011 == 1 and d.elevation > 500` is a
  script too.

### What is refused

The calculator reads Python's syntax and no other. Each of these is refused before anything runs, with the form to
write:

| Written | Write |
|---|---|
| `x ^ 2` | `x ** 2` |
| `a && b`, `a \|\| b`, `!a` | `a and b`, `a or b`, `not a` |
| `null` | `None` |
| `ln(x)` | `log(x)` |
| `if c then a else b`, `c ? a : b` | `a if c else b` |
| `x = 1` in a condition | `x == 1` |
| `"forest"` | a text is a value of features' attributes ([the vector calculator](../vector/)), not of a map |

A name that is neither an input nor given before is told with the one you probably meant
(`d.elevaton`: did you mean `d.elevation`?), and a layer the stack does not have with the layers it has.

## Recipes

| To | Write |
|---|---|
| a mask of one class | `forest = lu._2011 == 1` |
| a mask of several classes | `farm = lu._2011 == 2 or lu._2011 == 4` |
| classes by ranges | `1 if e < 300 else 2 if e < 800 else 3` |
| a value capped | `x[x > 30] = 30`, or `min(x, 30)` |
| a score from 0 to 1 | `clamp((x - lo) / (hi - lo), 0, 1)` |
| a map rescaled to 0 to 1 | `(x - x.min()) / (x.max() - x.min())` |
| standardized (z-scores) | `(x - x.mean()) / x.sd()` (of one layer: `x = d.elevation`) |
| a weighted sum of scores | `0.6 * a + 0.4 * b` |
| a normalized difference | `(s.nir - s.red) / (s.nir + s.red)` |
| from-to codes of two years | `lu._1997 * 10 + lu._2011` |
| the years in a class | `sum(lu == 1)` |
| the cells that changed | `lu._1997 != lu._2011` |
| a class's area | `(lu._2011 == 1).sum() * cell_area` (0.0625 km² for cells of 250 m) |
| the top 10 % | `x >= x.percentile(90)` |
| holes filled | `coalesce(x, 0)` |
| cells left out | `x[mask == 0] = None` |
| a moving-window share | `nbmean(lu._2011 == 1, 2)` |
| an edge | `forest and nbmin(forest, 1) == 0` |

## In the application, the model file and the assistant

=== "Application"

    Add a [[raster.calc]] from the library's **Map algebra** subject (or ++slash++ to search), and connect what it
    reads to its `+ new input` row, naming each input as the script reads it (`lu`, `d`). The Node pane edits the
    script in an editor of code, its words in colour and its first error underlined; the orange **Expand**
    button (or **View > Calculator**) opens the Calculator pane: the inputs and their layers at the top, the script
    with completions, a preview of every output as you type (on a sample of up to 160,000 cells; drag a rectangle
    on a map to preview its part in more detail), and a console at the bottom (`help` lists the language).
    ++ctrl+enter++ gives the script to the node and runs the model up to it. The tutorial
    [The raster calculator](../../tutorials/building-models/calculator/) walks through the pane, and saving a
    script as a node of your own.

=== "Model file"

    The script is the node's `expression` setting; a script of several lines is a TOML string of several lines:

    ```toml
    [[nodes]]
    id = "calc"
    op = "raster.calc"
    inputs = { lu = "landuse.raster", d = "drivers.raster", max_slope = 20 }
    params = { expression = """
    forest = lu == 1
    out years = sum(forest)
    out steep = lu._2011 == 1 and d.slope > max_slope
    """ }
    ```

    Another node reads its outputs as `calc.years` and `calc.steep`. Run it with `terraflux run model.toml`.

=== "Assistant"

    The assistant (and any client of `terraflux mcp`) writes a calculator with the tool `write_calc`: the script
    is checked (its language, every name an input or given before, the layers a stack has) and run on a sample of
    the cells **before** the model changes, and the reply tells what each output gives (a map's classes and their
    shares, or its least, mean and largest values), or why it was refused. `explain_calc` takes a calculator
    apart: each input, the layers it reads, and what every name holds on a sample. Ask it in plain words: "a map
    of the forest of 2011 above 500 m, and its area in km²".

## Common pitfalls

- **`=` and `==`.** `x = 1` gives `x` the value 1; `x == 1` asks whether it is 1.
- **No `&` or `|`.** Use `and` and `or`. In numpy, `a > 5 & b < 8` is read as `a > (5 & b) < 8`, a classic trap.
- **`count(s)` is not a count of true layers.** It counts the layers with a value; in how many layers a condition
  holds is `sum(s == 1)`.
- **`s.mean()` of a stack is of every layer at once.** `(d - d.mean()) / d.sd()` mixes metres, degrees and people;
  standardize each layer by itself: `(d.elevation - d.elevation.mean()) / d.elevation.sd()`.
- **Python's arithmetic, almost.** `round(2.5)` is 3 (a half is rounded away from 0; Python gives 2), and `%`
  keeps the sign of the number divided (`-7 % 3` is -1; Python gives 2).
- **Missing values spread**, through `and` and `or` too (see above). Look at the missing cells of an output: the
  maps of the playground draw them checkered.
- **Neighbourhoods reach into missing cells.** A window around a cell of the sea has land in it, so the sea gets
  a value; take it away with `x[isnull(land)] = None`.
- **A number of which cells?** A percentile or a mean depends on the cells the map has at that line (see
  *Numbers of all the cells*).
- **Units.** Areas of cells are those of the grid: cells of 250 m are 6.25 ha, 0.0625 km². A map in geographic
  coordinates (degrees) has cells of different areas: reproject it first ([coordinate systems](../../tutorials/getting-started/coordinate-systems/)).

## Going further

- [The vector calculator](../vector/): the same language over features, with texts and geometries.
- [Map algebra from scratch](../../tutorials/building-models/map-algebra/): local and focal operations, missing
  values, buffers and edges, built step by step.
- [The raster calculator, a tutorial](../../tutorials/building-models/calculator/): the Calculator pane, its
  console, and a script saved as a node of your own, with its versions.
- [Stacks and layers](../../tutorials/building-models/stacks-and-layers/): stacks from maps and folders of files.
- The operator reference: [[raster.calc]], [[raster.reclassify]], [[raster.focal]].

## References

- Harris, C. R., Millman, K. J., van der Walt, S. J., Gommers, R., Virtanen, P., Cournapeau, D., Wieser, E., Taylor, J., Berg, S., Smith, N. J., Kern, R., Picus, M., Hoyer, S., van Kerkwijk, M. H., Brett, M., Haldane, A., del Río, J. F., Wiebe, M., Peterson, P., … Oliphant, T. E. (2020). Array programming with NumPy. *Nature*, 585(7825), 357–362. https://doi.org/10.1038/s41586-020-2649-2
- McGarigal, K., & Marks, B. J. (1995). *FRAGSTATS: spatial pattern analysis program for quantifying landscape structure* (General Technical Report PNW-GTR-351). USDA Forest Service, Pacific Northwest Research Station. https://doi.org/10.2737/PNW-GTR-351
- Tomlin, C. D. (1990). *Geographic Information Systems and Cartographic Modeling*. Prentice Hall.
