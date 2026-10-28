---
title: "The raster calculator: scripts, a console and nodes of your own"
short: The raster calculator
description: Write map algebra as a short script close to Python in the Calculator pane, with its layers completed, its outputs previewed as you type and a console for quick numbers, then save it as a node of your own to use in any model.
order: 15
level: beginner
time: 40 min
ops: [raster.calc, control.group]
---

In this tutorial you write one script that does the work of half a dozen map algebra nodes: from the land use of
Sibuyan Island in 1997 and 2011 and a stack of its environmental variables, it finds where the forest stayed and
where it was lost, scores the forest of 1997 as habitat for a species that avoids roads and high ground, picks out
the best quarter of it, and gives the mean score as a number. You write it in the **Calculator** pane, which
colours the script, completes the names of the layers, shows each result as you type and computes any line you
ask it in a console. At the end you save the script as **a node of your own**, which you can add to any other
model like an operator.

If map algebra is new to you, read [Map algebra from scratch](../map-algebra/) first: it explains what a raster
is, what a local and a focal operation are, and how missing cells behave. This page builds on it.

## The idea

A [[raster.calc]] node computes new maps from other maps, cell by cell. Its *expression* used to be one formula
(`land == 1 and d.elevation < 1200`), and a model that needed six formulas needed six nodes. A **script** is a
short program of several lines: each line gives a name to a value, the later lines use the earlier names, and the
lines that begin with `out` are what leaves the node. A single expression is still a script of one line, so every
model written before keeps working.

The language is meant to read like Python and like numpy, the library most ecologists who program already know
(Harris et al., 2020): `=` gives a name a value and `==` compares; `and`, `or`, `not`; `a if c else b`; `**` for
powers; `#` starts a comment. Like numpy it works on whole arrays at once: `x * 2` doubles every cell, and
`x[x < 0] = 0` changes only the cells where the condition holds.

A script handles three kinds of values, and knowing which one you have explains most of what it does:

!!! science "Key idea: maps, stacks and numbers"
    - A **map** has one value per cell (a land use, an elevation). Arithmetic, comparisons and functions act
      cell by cell, as in Tomlin's local operations (Tomlin, 1990).
    - A **stack** has several maps of the same grid, each a *layer* with a name: the bands of a satellite image,
      the years of a land use series, a model's drivers. `s.elevation` is one layer (a map). `s * 2` or
      `s == 1` act on **every** layer and give a stack with the same names. `mean(s)`, `sum(s)`, `sd(s)`,
      `min(s)`, `max(s)`, `median(s)`, `count(s)` and `percentile(s, p)` reduce the layers **at each cell** to
      one map: "in how many years was this cell forest?" is `sum(s == 1)`.
    - A **number** is one value for the whole area. `x.mean()`, `x.sd()`, `x.median()`, `x.sum()`,
      `x.count()`, `x.min()`, `x.max()` and `x.percentile(p)` (with the brackets) reduce **all the cells** of a
      map or a stack to a number: \( \bar{x} = \frac{1}{n} \sum_{i=1}^{n} x_i \) over the \(n\) cells with a
      value. A number can be used in the next lines (`x > x.percentile(90)`) and can leave the node, where it
      feeds any setting that reads a number.

So `mean(s)` and `s.mean()` are different questions: the first is a map (the mean over the layers, cell by
cell), the second a number (the mean of every cell of every layer). Reductions of both kinds leave missing cells
out; the standard deviation is the sample one (dividing by \(n - 1\)) and the percentiles interpolate between the
two nearest values, as numpy and R do by default.

## When to use it

Use a script when the steps belong together and are small: an index made of a few scores, a mask cleaned in two
lines, a set of thresholds, a quick summary. One node keeps the reasoning in one place, and Terraflux computes all
its outputs in a single pass over the cells, which is faster than a chain of nodes.

Use separate nodes when a step is worth seeing on the canvas by itself, when another node needs an intermediate
result, or when the step is an operator of its own: a reclassification table reads better in
[[raster.reclassify]], a distance in [[distance.euclidean]], a moving window larger than a few cells in the
focal operators. When no expression can do it (a loop over cells that depends on the previous cell, a flood
fill), write a [plugin](../plugins/).

## What you need

The data of [Map algebra from scratch](../map-algebra/), from Sibuyan Island in the Philippines at 250 m:

- `landuse.tif`, the land use in two bands named `1997` and `2011` (1 forest, 2 coconut, 3 grassland, 4 rice,
  5 other); the sea has no value.
- `drivers.tif`, 13 bands named after what they hold: `dist_sea`, `pop_density`, `diorite`, `ultramafic`,
  `sediments`, `no_erosion`, `moderate_erosion`, `elevation` (m), `slope`, `aspect`, `dist_roads` (m),
  `dist_urban` and `dist_streams`.

Both are read as **stacks**, so the script reaches every band by its name. A band whose name is not a valid
name in the language is given one: `1997` becomes `_1997` (a name cannot start with a digit), `NDVI-2020`
becomes `NDVI_2020`, and a band without a name is `b1`, `b2`... The calculator shows both names.

## Open the calculator

The model has two inputs and one node:

```tfgraph
#! caption: The land use of both years and the drivers, read as stacks, and one raster.calc node with the whole script.
#! highlight: calc
format = "terraflux/1"

[inputs.lu]
kind = "stack"
path = "landuse.tif"

[inputs.d]
kind = "stack"
path = "drivers.tif"

[[nodes]]
id = "calc"
op = "raster.calc"
inputs = { lu = "$lu", d = "$d" }
params = { expression = "out years = sum(lu == 1)" }
note = "The forest's years, its loss, and a habitat index of the forest of 1997."
```

!!! app "In the application"
    Open the two files (++ctrl+o++, or drag them from the **Files** drawer), add a [[raster.calc]] from the
    library's **Map algebra** subject and connect each file's `raster` port to an input of it named `lu` and `d`
    (drag from the output to the `+ new input` row; the Node pane's **Add input** names an input too). The Node
    pane shows the node's script in an editor of its own, its words in colour and its first error underlined, for
    a script of a few lines. For the full editor, press the orange **Expand** button beside the script's name, or
    choose **View > Calculator**: the calculator edits the script of the map algebra node selected.

The pane has three parts. At the top, the node's inputs and what they are (`lu: stack · 2`, `d: stack · 13`;
hover over one to see its layers). In the middle, the script, with line numbers and its words in colour: inputs,
names you gave, layers, functions, numbers and comments each have their own. At the right of the script, a
preview of every output. At the bottom, the console.

The inputs are known once what feeds them has run (press **Apply and run** once), or, for rasters read from
files, from the files' headers before anything runs.

## Write the script

Type the script below line by line; the sections that follow explain it.

```python
# lu: the land use of 1997 and 2011 (a stack); d: the drivers (a stack)
forest = lu == 1                        # a stack: the forest of each year
out years = sum(forest)                 # years as forest: 0, 1 or 2
out lost = forest._1997 and not forest._2011

# a score from 0 to 1 for elevation and for roads, and the index
si_elev = clamp((1200 - d.elevation) / 900, 0, 1)
si_road = clamp(d.dist_roads / 3000, 0, 1)
out hsi = sqrt(si_elev * si_road)
hsi[not forest._1997] = None            # the forest of 1997 only

out best = hsi >= hsi.percentile(75)    # its best quarter
out mean_hsi = hsi.mean()
```

![The Calculator pane under the model](/img/tutorials/building-models/calculator/pane.png "The Calculator pane under the model. At its top, the node's inputs, two stacks; then the script in colour, and at its right the preview of each output (years first; scroll for lost, hsi, best and mean_hsi), computed on a sample of the cells as you type; at the bottom, the console, where hsi.percentile(75) was asked and answered at once. Drawn by Terraflux.")

### Stacks: the forest of each year

`forest = lu == 1` compares every layer of `lu` with 1, so `forest` is a stack of two layers with the same names
as `lu`'s: the forest of 1997 and the forest of 2011, 1 where forest and 0 elsewhere. `sum(forest)` adds the
layers at each cell: 2 where the cell was forest in both years, 1 in one of them, 0 in neither.

`forest._1997 and not forest._2011` reads two layers by name. When you type `forest.` or `d.`, the calculator
offers the layers of that stack, with their names in the file beside them; ++tab++ or ++enter++ takes the one
chosen, and ++ctrl+space++ asks for the names at any point.

![The layers of the drivers offered after the dot](/img/tutorials/building-models/calculator/completion.png "Typing d.dist_ offers the drivers' layers that begin with it: dist_sea, dist_roads, dist_urban and dist_streams. The functions, the methods and the inputs are completed the same way, each with a line of help. Drawn by Terraflux.")

### Names and conditions: the habitat index

The two scores and the index are those of [Map algebra from scratch](../map-algebra/): elevation scores 1 below
300 m and falls to 0 at 1,200 m, distance to roads scores 0 at the road and 1 beyond 3 km, and the index is
their geometric mean. Here each score is a name, used on the next line, and nothing but `hsi` leaves the node.

`hsi[not forest._1997] = None` is a *conditional assignment*: only the cells where the condition holds are
changed (here, the cells that were not forest in 1997 lose their value); the others keep the one they had. This
is numpy's rule, and it replaces the nested `a if c else b` of a single expression. Several such lines
apply in order, a later one over an earlier one.

!!! note "Where the condition has no value"
    Over the sea, `forest._1997` has no value, and neither has `not forest._1997`: a condition without a value
    changes nothing, so those cells keep what they had (here, no value either, because the drivers have none
    over the sea).

### Numbers: the best quarter and the mean

`hsi.percentile(75)` is one number, the value that 75 % of the forest's cells are below (the cells without a
value are left out). `hsi >= hsi.percentile(75)` then marks the best quarter of the forest of 1997, whatever the
scores' range. `out mean_hsi = hsi.mean()` makes a number an output of the node: in the application it shows as
a number, and another node can read it, for instance as a threshold in one of its settings.

### The outputs

Every `out` is an output port of the node, named as in the script: `calc.years`, `calc.lost`, `calc.hsi`,
`calc.best` and `calc.mean_hsi`. `out name = value` gives the name and makes it an output; `out name` alone makes
an output of a name given before. An output takes the value its name has at the end of the script, so `hsi`
leaves the node without the cells that line 10 took away.

The preview beside the script is computed about a third of a second after you stop typing, on a sample of up to
160,000 cells, so it stays quick on large maps; the node itself computes every cell when the model runs.

### Errors, where they are

A mistake is underlined where it is and told under the script with its line: a name that is neither an input
nor given before (with the one you probably meant: `hsii` → `hsi`), a layer the stack does not have, a bracket
not closed, `=` where a comparison was meant. Until it is fixed, the preview shows the outputs as they last were;
a script given to the node with a mistake in it makes the run stop at the node, with the same message.

### Give it to the node and run

The script typed in the pane is not the node's until you give it: ++ctrl+enter++ (or **Apply and run**) gives it
and runs the model up to the node; **Apply** gives it without running, as does leaving the editor. A dot beside
the buttons marks a script not given yet.

## The console

Under the script, the console computes one line in the script's scope, on every cell, without changing the node:
all the inputs and every name the script gives are there. Type a line and press ++enter++:

```text
❯ hsi.percentile(75)
= 0.814519
❯ forest._1997.sum() * 0.0625
= 295.1875
❯ lost.sum() / forest._1997.sum()
= 0.119204
```

- a number is answered at once: `hsi.mean()`, `forest._1997.sum() * 0.0625` (the forest's area in km², with
  cells of 6.25 ha), `d.elevation.max()`;
- a map or a stack is answered with its least, mean and largest values, its missing cells and its image:
  `d.dist_roads < 500`, `forest * 2`;
- `help` lists the language, `clear` empties the console, and ++arrow-up++ brings back the lines typed before.

Use it to check a value before you write it into the script: the share of forest that is lost, the range of a
driver, a threshold.

## Read the results

```tfgallery
columns: 2
caption: The four maps of the script, from one run of the model. The sea has no value.
images:
  - { src: /img/tutorials/building-models/calculator/years.png, caption: "years = sum(lu == 1)" }
  - { src: /img/tutorials/building-models/calculator/lost.png, caption: "lost = forest._1997 and not forest._2011" }
  - { src: /img/tutorials/building-models/calculator/hsi.png, caption: "hsi, the forest of 1997 only" }
  - { src: /img/tutorials/building-models/calculator/best.png, caption: "best = hsi >= hsi.percentile(75)" }
```

In this run, 260 km² were forest in both years, and 35.2 km² of the 295.2 km² of forest of 1997 were gone by
2011: 11.9 %. The index of the forest of 1997 has a mean of 0.627 (`mean_hsi`, the number among the outputs); it
is low on the high slopes of the centre, above 1200 m, and along the roads, and its best quarter, the cells at
0.81 or more, covers 73.8 km² in patches on the east and the north.

As in [Map algebra from scratch](../map-algebra/), the index is a relative score made for this tutorial, not a
published model of a species, and the "best quarter" is a choice: report the rule (the script) with the area it
gives.

## Save it as a node of your own

The script now does one job well: from a land use stack and the drivers, it gives a habitat index and its best
quarter. To use it again in another model, save it as **a node of your own**.

!!! app "In the application"
    Select the node and press **Save as my node...** in the Node pane (or **Save the node selected as my node**
    in the commands, ++ctrl+k++). The same works for a group of nodes (++ctrl+g++ makes one).

The dialog shows what the node takes and gives, as Terraflux found it, for you to change before saving:

![The dialog that saves the calculator as a node of your own](/img/tutorials/building-models/calculator/save-dialog.png "Save as my node: its name and what it does; each input with its type, the layers it reads (found in the script: lu must have _1997 and _2011, d must have elevation and dist_roads), and its value when none is given; each output with its type, known from the last run. Drawn by Terraflux.")

- **Name** and **What it does**: how the node appears in your library (letters, digits and `_`).
- **Inputs**: each one's type (a map, a stack, a number...), the **layers it reads**, found in the script
  (`lu` must have `_1997` and `_2011`, `d` must have `elevation` and `dist_roads`), and **its value** when none
  is given: an input that was a number in the model (a threshold, say) keeps it as its own value, and then need
  not be connected.
- **Outputs**: each one's type, from what it gave in the last run.

**Save** writes the node into your library as `forest_hsi@1.toml`, its first version, and the node of the model
becomes that node: a calculator becomes a *group* of one node, its inputs the group's and each `out` an output.
On the canvas its box names it `forest_hsi@1`, and its ports have the colours of their types.

![The node of your own in the model](/img/tutorials/building-models/calculator/my-node.png "The calculator, now the node forest_hsi@1: its ports typed, and in the Node pane what it is, what each input needs, and the buttons to save a new version or make it a plain group again. Your nodes are listed in the Operators pane, under My nodes. Drawn by Terraflux.")

### Using it in another model

Your nodes are listed in the **Operators** pane under **My nodes** (the search finds them by name and by what they
do) and in the commands (`Add forest_hsi@1`). A click adds one after the node selected, as an operator is added.
Hover over it to see what it takes and gives.

When you connect a stack to it, Terraflux checks the layers it reads: a land use stack whose bands are named
otherwise (`lu_1997`, say) is told at once, in the node's pane and when the model runs, with the layers missing
and those the stack has.

### Versions

Each save is a new version, and a version once saved never changes. A model keeps **a copy** of the version it
uses, in the model file:

```toml
[[nodes]]
id = "calc"
op = "control.group"
inputs = { lu = "$lu", d = "$d" }
[nodes.uses]
name = "forest_hsi"
version = 1
description = "The forest's years, its loss, and a habitat index of the forest of 1997."
[nodes.uses.inputs.lu]
type = "stack"
bands = ["_1997", "_2011"]
[nodes.uses.inputs.d]
type = "stack"
bands = ["elevation", "dist_roads"]
[nodes.uses.outputs.hsi]
type = "map"
# ... the other outputs, then the body: the raster.calc node and its script
```

This has two consequences that matter for science. A model sent to a colleague runs without your library. And a
model of last year gives the same result today, even if you have improved the node since: a newer version reaches
a model only when you press **Update to forest_hsi@2** in the node's pane (what the node is given is kept where the
new version still takes it). The pane also tells when the node was changed inside the model since its version, and
offers to save the change as a new version, or to make it a plain group again.

## The language in full

| | |
|---|---|
| Lines | one statement a line, or several separated by `;`; `#` starts a comment; a line may continue inside brackets |
| Names | `name = value`; `out name = value` and `out name` for the outputs; a name given in the script hides an input of the same name |
| Conditions | `target[condition] = value` (only where the condition holds); `s.layer = value` replaces or adds a layer of a stack |
| Values | numbers, `pi`, `None` (no value), `True` and `False` (1 and 0); the inputs; `s.layer` |
| Arithmetic | `+ - * / %` and `**` for powers |
| Comparisons | `< <= > >= == !=`, chained as in Python: `300 <= d.elevation < 1200` |
| Logic | `and`, `or`, `not`; `a if c else b` |
| Functions | `abs sqrt exp log log10 floor ceil round` (`log` the natural logarithm), `clamp(x, lo, hi)`, `isnull(x)`, `coalesce(x, y)`, `min(a, b, ...)`, `max(a, b, ...)` |
| Neighbourhoods | `nbsum nbmean nbmin nbmax nbcount (x, r)`: the square of radius `r` cells around each cell |
| Over a stack's layers, at each cell | `mean sd median sum count min max (s)`, `percentile(s, p)` |
| Over all the cells, a number | `x.mean() x.sd() x.median() x.sum() x.count() x.min() x.max()`, `x.percentile(p)` |

Stacks and maps mix as you would expect: a stack with a number or a map acts on each layer; two stacks act layer
by layer and must have the same number of layers.

## Common pitfalls

- **`=` and `==`.** `x = 1` gives `x` the value 1; `x == 1` asks whether it is 1. In a condition, always `==`.
- **Python's syntax, and no other.** `^`, `&&`, `||`, `!`, `null`, `ln`, `if c then a else b` and `c ? a : b`
  are refused, each with the form to write (`**`, `and`, `or`, `not`, `None`, `log`, `a if c else b`). A model
  written by an older Terraflux in those forms is rewritten in Python's when it is opened.
- **No `&` or `|`.** In numpy, `a > 5 & b < 8` is read as `a > (5 & b) < 8`, a classic trap; the calculator
  has `and` and `or`, which mean what they say.
- **`mean(s)` is not `s.mean()`.** The first is a map, the mean of the layers at each cell; the second one
  number. `mean(x)` of a single map is refused and points to `x.mean()`.
- **`min(a, b)` and `min(s)`.** With several maps, `min` is the least of them at each cell, and a missing value
  in any gives a missing cell; with one stack, it is the least of its layers, the missing ones left out.
- **Layer names.** A band named `1997` is `_1997` in a script. If the calculator marks a layer as unknown, hover
  over the input at the top of the pane to see the names it has.
- **The preview is a sample.** It is computed on at most 160,000 cells spread over the map; a neighbourhood's
  radius is scaled to the sample. The run computes every cell: look at the result, not only at the preview.
- **A number of which cells?** `hsi.percentile(75)` depends on the cells `hsi` has when it is computed: here
  only the forest of 1997, because line 10 took the others away before. Move that line after `best`, and the
  quarter is of every cell of the island, the farms and the grassland included.

## Going further

- [The raster calculator](../../../calculators/raster/): every part of the language with a playground that
  runs Terraflux's engine in your browser, recipes and pitfalls; and [the vector calculator](../../../calculators/vector/)
  for the attributes of features.
- [Map algebra from scratch](../map-algebra/): the ideas behind the operations, missing values, edges and buffers.
- [Stacks and layers](../stacks-and-layers/): build stacks from maps and folders of files.
- [Groups and submodels](../groups-and-submodels/): groups, and other model files used as one node.
- [Plugins in WebAssembly](../plugins/): when a script is not enough.
- The operator reference: [[raster.calc]], [[control.group]].

## References

- Harris, C. R., Millman, K. J., van der Walt, S. J., Gommers, R., Virtanen, P., Cournapeau, D., Wieser, E., Taylor, J., Berg, S., Smith, N. J., Kern, R., Picus, M., Hoyer, S., van Kerkwijk, M. H., Brett, M., Haldane, A., del Río, J. F., Wiebe, M., Peterson, P., … Oliphant, T. E. (2020). Array programming with NumPy. *Nature*, 585(7825), 357–362. https://doi.org/10.1038/s41586-020-2649-2
- Tomlin, C. D. (1990). *Geographic Information Systems and Cartographic Modeling*. Prentice Hall.
