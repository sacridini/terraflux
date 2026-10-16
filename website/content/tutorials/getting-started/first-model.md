---
title: Your first model
description: Build a model by hand from nodes, ports and typed connections; set its settings, read its notes, run it to a node and let the cache spare you waiting; read the model as a TOML file, check and run it from the command line with other data, and save it as a template of your own.
order: 70
level: beginner
time: 40 min
ops: [raster.read, raster.reclassify, landscape.mspa, view.plot, raster.write]
---

In the earlier tutorials you opened maps, reclassified them and saved them. In this one you put those steps
together into a **model** that you build yourself, node by node: from a land cover map of northern Germany to a
map of its **core forest**, the forest more than 50 m from any edge. On the way you will learn the vocabulary every
later tutorial uses (nodes, ports, connections, settings, notes), what the cache does when you change a setting,
what the model file looks like, how to run it from the command line on another map, and how to keep it as a
template of your own. At the end you will have a model file that you can run on any land cover map.

## The idea: nodes, ports and connections

A Terraflux model is a set of **nodes** joined by **connections**.

Node
:   A box that runs one **operator**: [[raster.read]] reads a file, [[raster.reclassify]] changes classes,
    [[landscape.mspa]] runs MSPA. A node has a **name** (its *id*, such as `forest`), which you choose, and an
    operator, which says what it does.

Port
:   A place where data goes in or comes out. **Input ports** are on the left edge of a node, **output ports** on
    the right. [[raster.reclassify]] has one input, `map`, and one output, also called `map`; [[raster.read]] has
    one output, `raster` (see [Your first raster](../first-raster/#open-a-raster)).

Connection
:   A line from an output port to an input port: "give the map that `landcover` reads to `forest`". In the model
    file a connection is written as `node.port`: the input `map` of `forest` takes `landcover.raster`.

Setting
:   A value that changes how the operator works but does not come from another node: the rules of a
    reclassification, the edge width of MSPA, the path of a file. Settings are typed in the Node pane.

Note
:   A sentence attached to a node that says **why** it is there and why its settings are what they are. Notes do
    not change the result; they are for the person who reads the model next (often yourself, a year later).

!!! science "Key idea: typed connections"
    Every port has a **type**: a *map* (one grid of values), a *stack* (several maps on one grid), *rasters on
    disk* (a file not yet read), a *table*, a *number*, *vector* features, a *series* (one value per step of a
    loop)... A connection is accepted only when the type of the output **fits** the type of the input. Terraflux
    checks this the moment you draw the connection, and again before every run, so a model never runs halfway and
    then fails because a table arrived where a map was expected.

The rules of fitting are few:

| Output gives | Fits an input that takes | Why |
|---|---|---|
| the same type | that type | a map into a map |
| a number or a whole number | a number or a whole number | the operator checks that a whole number is whole |
| rasters on disk | a map or a stack | read when the node runs |
| a map, a stack or a series of maps | rasters on disk | written where needed |
| anything | an input of type *any* | the operator checks it itself |

## When to build a model yourself

Start from a template whenever one is close to what you want (see [A tour of Terraflux](../tour/#templates-models-to-start-from)).
Build one yourself when your analysis is a chain of steps no template has, when you want to understand every step
of an analysis you will publish, or when you will run the same steps on many maps. A model is also the clearest
way to write the methods section of a paper: every step and every setting is in it, with a note.

## What you need

- Terraflux, and ten minutes with the [tour](../tour/) if you have not used the window yet.
- A land cover map. This tutorial uses `clc.tif`, the excerpt of **CORINE Land Cover** around Hanover (50 x 50
  km, cells of 25 m, 2000 x 2000 cells) of [Your first raster](../first-raster/#what-you-need). Any map of classes
  works; only the class codes of forest change.

## Build the model

### Step 1: read the map

<div class="steps" markdown>

1. Click **New** in the toolbar: an empty model.
2. Press ++ctrl+o++ (or **Open a raster...** at the top of the operators' drawer) and choose `clc.tif`.
3. A node `raster.read` appears, selected, and its map is shown in the Viewer.
4. Rename it: press ++ctrl+r++, type `landcover`, press ++enter++. Names are what connections refer to, so short,
   meaningful names make a model readable.

</div>

### Step 2: make the forest MSPA's foreground

MSPA works on two classes: the habitat it analyses, the *foreground*, coded 2, and everything else, the
*background*, coded 1. CORINE has three forest classes: 23 (broad-leaved), 24 (coniferous) and 25 (mixed).

With `landcover` selected, press ++slash++, type `reclass` and press ++enter++. A node [[raster.reclassify]]
appears to the right, **already connected**: its input `map` takes `landcover.raster`. Rename it `forest` and type
its rules in the Node pane:

```text
23..25 -> 2
* -> 1
```

The first rule turns every value from 23 to 25 into 2; the second, `*`, turns every other value into 1. The Node
pane shows the rules as a table, with the classes of the map it reads and their shares beside them (**A rule per
class** fills the table with one row per class). The syntax is explained in
[Reclassify, calculate and save](../reclassify-calculate-save/#reclassify-rules-one-per-line).

```tfgraph
#! caption: Steps 1 and 2: read the land cover, and make its forest the foreground (2) and everything else the background (1).
#! highlight: forest
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "clc.tif" }
note = "CORINE Land Cover around Hanover, 25 m."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landcover.raster" }
params = { rules = "23..25 -> 2\n* -> 1" }
note = "CORINE's forests (23 broad-leaved, 24 coniferous, 25 mixed) become MSPA's foreground (2), the rest 1."
```

Press ++ctrl+enter++ to run up to `forest` and look at the Viewer: a map of two classes, forest and not forest.

![The forest of the CORINE excerpt](/img/tutorials/getting-started/first-model/forest.png "The output of forest: CORINE's three forest classes as 2 (green), the rest as 1. Forest covers 20.6% of the 2,500 km².")

### Step 3: MSPA

With `forest` selected, press ++slash++, type `mspa`, choose [[landscape.mspa]] (not `mspa_tiled`, which is for
maps larger than memory) and press ++enter++. Rename it `mspa`. Its input `map` takes `forest.map`. In its
settings, set `edge_width` to `2`.

What does `edge_width` mean ecologically? It is the depth, in cells, of the band along the border of the forest
that counts as **edge** rather than core. With cells of 25 m, 2 cells is 50 m: the distance over which you consider
that the conditions of the forest are changed by its border (light, temperature, wind, nest predators). Edge
effects on forest birds, microclimate and trees have been measured from a few tens to a few hundred metres
(Murcia, 1995; Harper et al., 2005): choose the value from what is known of your species or process, and say so in
the node's note.

### Step 4: see the result, and keep it

With `mspa` selected, add [[view.plot]] (++slash++, `plot`). It shows MSPA in a tab of its own, refreshed at every
run, without writing any file. To keep the map as a file, select `mspa` and add [[raster.write]] (++slash++,
`write`), and give it a `path`, such as `forest_mspa.tif` (**Choose...** opens a dialog). Paths are written
relative to the folder of the model file, so a model and its data can move together to another computer.

```tfgraph
#! caption: The whole model: read, reclassify, MSPA, then a plot to see it and a write to keep it.
#! highlight: mspa
#! numbered: true
format = "terraflux/1"
name = "Core forest"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "clc.tif" }
note = "CORINE Land Cover around Hanover, 25 m."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landcover.raster" }
params = { rules = "23..25 -> 2\n* -> 1" }
note = "CORINE's forests (23 broad-leaved, 24 coniferous, 25 mixed) become MSPA's foreground (2), the rest 1."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 2 }
note = "An edge of 2 cells (50 m): the forest within 50 m of its border is edge, not core."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "Core forest" }

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "mspa.map" }
params = { path = "forest_mspa.tif" }
```

Press ++f5++ to run the whole model. Then save it: **Save as...**, `core-forest.toml`, in the folder of `clc.tif`.

!!! app "Connecting by hand"
    The search and the library connect a new node to the node selected for you. To connect two nodes that are
    already on the canvas, **drag from an output port** (a dot on the right edge of a node) **to an input port** (a
    dot on the left edge of another). If the types do not fit, or the connection would make a loop (a node that
    depends on itself), Terraflux refuses it and says why in the Model pane's bar. To take a connection away, drag
    it off its input (dropped on another input, it moves there), or right-click the input. Inputs that must be
    connected are marked with a star in the Node pane.

## Read the results

![The MSPA classes of the forest around Hanover](/img/tutorials/getting-started/first-model/mspa.png "MSPA of the forest of the CORINE excerpt with an edge of 2 cells (50 m). Core in green, edge in black; branches, bridges and loops are thin. Shares of the whole map; the background is everything that is not forest.")

Of the 4,000,000 cells, 824,612 are forest (515 km², 20.6% of the map). With an edge of 50 m:

| MSPA class | Cells | km² | Share of the forest |
|---|---|---|---|
| Core | 671,624 | 419.8 | 81.4% |
| Edge | 134,248 | 83.9 | 16.3% |
| Branch | 13,542 | 8.5 | 1.6% |
| Bridge (and bridge in edge) | 4,713 | 2.9 | 0.6% |
| Islet, loop, perforation | 485 | 0.3 | 0.1% |

In the application, the legend beside the map gives the cells of each class; **Metadata** gives the cell size to
turn them into areas (cells x 625 m²). The large forests in the south and north-east are mostly core; the smallest
woods of the farmland have little or none, because a wood narrower than 100 m has no cell farther than 50 m from its
border, and their thin strips become branches and bridges.

## Settings that change the answer

The share of core depends strongly on the edge width. The same model, run on a 15 x 15 km window of the map with
six edge widths:

```tfslider
label: Edge width (cells of 25 m)
caption: "MSPA of the forest in the north-west of the map with an edge from 25 to 200 m. As the edge widens, cores shrink, small woods lose their core entirely, and the share of core in the forest falls from 88% to 41%."
frames:
  - { value: 1, src: /img/tutorials/getting-started/first-model/edge_1.png, caption: "edge 25 m", stats: { "Core": "88.1%", "Edge": "11.3%" } }
  - { value: 2, src: /img/tutorials/getting-started/first-model/edge_2.png, caption: "edge 50 m", stats: { "Core": "77.2%", "Edge": "19.7%" } }
  - { value: 3, src: /img/tutorials/getting-started/first-model/edge_3.png, caption: "edge 75 m", stats: { "Core": "70.0%", "Edge": "24.2%" } }
  - { value: 4, src: /img/tutorials/getting-started/first-model/edge_4.png, caption: "edge 100 m", stats: { "Core": "63.6%", "Edge": "28.0%" } }
  - { value: 6, src: /img/tutorials/getting-started/first-model/edge_6.png, caption: "edge 150 m", stats: { "Core": "51.1%", "Edge": "33.7%" } }
  - { value: 8, src: /img/tutorials/getting-started/first-model/edge_8.png, caption: "edge 200 m", stats: { "Core": "41.1%", "Edge": "35.8%" } }
chart:
  type: line
  data: /data/tutorials/getting-started/first-model/edge.csv
  x: edge_width
  y: [core, edge]
  labels: { core: "Core", edge: "Edge" }
  colors: { core: "#2ca02c", edge: "#1c2420" }
  ylabel: "% of the forest"
```

This is why a paper must report the edge width it used, and why a value chosen from the biology of the species is
better than the default. The tutorial [MSPA](../../habitat-patterns/mspa/) goes through MSPA's other settings.

## Run to a node, and the cache

You rarely need to run the whole model. **Run to here** (or ++ctrl+enter++) runs the node selected and only the
nodes it needs. And Terraflux keeps the outputs of every node of the model in a **cache** between runs: when you
run again, a node whose result cannot have changed is not run, its result is taken from the cache.

![Which nodes run again after a change](/img/tutorials/getting-started/first-model/cache.png "The cache at work. After the first run every result is kept. Change edge_width and only mspa and the nodes after it run again; the land cover is not read again and the reclassification is not redone. Changing a note or moving a node changes no result, so nothing runs.")

How does Terraflux know that a result cannot have changed? It keeps, for each node, a *fingerprint* of what the
result came from: the node's operator, inputs and settings, the fingerprints of the nodes it depends on, the
model inputs it reads, and the size and date of the files it reads. If any of those changed, the node runs again,
and so do all the nodes after it. Its place on the canvas and its note are not part of the fingerprint. If a file
on disk changes (you saved a new version of `clc.tif`), its date changes, and the node that reads it runs again.

With **update after every edit** ticked in the Model pane's bar, the model runs up to the node shown after every
change. On a model of a few seconds this makes the result follow your settings as you type; untick it for models
that take minutes.

The dot at the top right of each node tells you what happened in the last run: green, it ran; darker green, its
result came from the cache; red, it failed (the Node pane says why).

!!! note "What is not kept"
    A node whose output is a random number generator (such as [[rng.r]], carried along a loop) is never taken from
    the cache, since its state changes every time it is used. See [Randomness and reproducibility](../../building-models/randomness/).

## The model as a file

Click **Text** in the Model pane's bar: the canvas is replaced by the model as text, with any problems found in it
listed above it. **Diagram** brings the canvas back. This text is the file **Save** writes, in a format called TOML
(a plain-text format of `key = value` lines and `[sections]`). Here is `core-forest.toml` as the command line
section below uses it, with a small change explained there:

```toml
format = "terraflux/1"
name = "Core forest"
description = "The core of the forest of a land cover map, by MSPA."

[inputs.landcover]
kind = "map"
path = "clc.tif"
note = "CORINE Land Cover, 25 m."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "$landcover" }
params = { rules = "23..25 -> 2\n* -> 1" }
note = "CORINE's forests (23 broad-leaved, 24 coniferous, 25 mixed) become MSPA's foreground (2), the rest 1."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 2 }
note = "An edge of 2 cells (50 m): the forest within 50 m of its border is edge, not core."

[outputs.mspa]
from = "mspa.map"
```

Line by line:

- `format` says which version of the format this is; `name` and `description` are shown in the application.
- `[inputs.landcover]` declares an **input of the model**: data the model reads, with a name. `kind = "map"` reads
  one band of a raster (`kind = "stack"` every band, `kind = "value"` a number or any value). Nodes refer to it as
  `$landcover`.
- Each `[[nodes]]` block is a node: its `id`, its `op`, its `inputs` (each a reference `node.port`, a model input
  `$name`, or a literal value such as `10`), its `params` (the settings) and its `note`.
- `[outputs.mspa]` declares an **output of the model**: what `terraflux run` writes, here the map of `mspa.map`.

The order of the nodes in the file does not matter: Terraflux runs them in the order of their connections. In the
application, a node's place on the canvas is saved as `pos = [x, y]`. Notes are shown in the Node pane under
**Why** and when the pointer rests on a node's header; you write them in the model file with any text editor.

!!! tip "A model is text: keep it with your data"
    Because a model is plain text, you can compare two versions of it, keep it in a version control system such as
    git, send it by email, and put it in the supplementary material of a paper, where it says more precisely than
    any methods section what was done.

## From the command line

The command `terraflux` runs the same file without the window. On Windows the installer puts it on the PATH;
on macOS and Linux see [Installing](/quickstart/install/). Open a terminal in the folder of the model.

**Check** the model without running it: every operator, connection, type and file is checked.

```text
$ terraflux check core-forest.toml
core-forest.toml: no errors
```

Here is what `check` says about a model with two mistakes, a table connected to MSPA's map and a misspelt output
(`mspa.mapp`):

```text
$ terraflux check broken.toml
error: mspa.map: classes.table gives a Table, the port takes a Map
error: plot.value: node "mspa" (landscape.mspa) has no output "mapp"
```

**Run** it. The outputs of the model are written to a folder `output` next to the model (or to the folder given
by `--out`), each named after its output:

```text
$ terraflux run core-forest.toml
Core forest: inputs read (0.0 s)
wrote output\mspa.tif
done in 0.3 s
```

**Run it on other data** with `--set`, which replaces a model input: `name=path` for a file, `name=value` for a
value. This is why the file above reads the land cover as an input (`$landcover`) instead of with a
[[raster.read]] node: an input can be swapped from the command line. Here the same model runs on a 15 x 15 km
window of the map, its result written to another folder:

```text
$ terraflux run core-forest.toml --set landcover=window.tif --out window
Core forest: inputs read (0.0 s)
wrote window\mspa.tif
done in 0.0 s
```

![MSPA of the window](/img/tutorials/getting-started/first-model/mspa_window.png "window/mspa.tif: the same model run on a 15 x 15 km window of the map, by changing only the input on the command line.")

The command's full help:

```tfcli
run
```

!!! warning "raster.write writes where its path says, whatever --out says"
    `--out` is the folder of the model's **outputs** (`[outputs.name]`). A [[raster.write]] node writes to its own
    `path`, relative to the model's folder, at every run. Run a model with a [[raster.write]] twice with
    different `--set` and the second run overwrites the first one's file. For models you will run from the
    command line on many inputs, prefer outputs; keep [[raster.write]] for files you always want at the same
    place.

`--set` also changes a node's **setting**, written `node.setting=value`: `--set mspa.edge_width=3` runs MSPA with
an edge of 3 cells (75 m) without touching the file. Give each such run its own `--out`. To try many edge widths at
once, run a batch of scenarios, which change settings the same way (`terraflux submit --scenarios`, see the
[parallel computing guide](/parallel/)).

## Save your own template

A model you will start from again (your standard preparation of a land cover map, say) can become a template of
your own:

<div class="steps" markdown>

1. Open **Templates**. Under the groups, at **Yours**, type a name in the box (for example `Core forest of CORINE`).
2. Click **Save this model as a template**.
3. From now on it is listed under **Yours**; clicking it opens a copy as a new model and runs it, like the
   templates of Terraflux. The small **x** beside it removes it.

</div>

The template is a model file in the folder `templates` of your user's application data (`%APPDATA%\Terraflux\templates`
on Windows, `~/Library/Application Support/Terraflux/templates` on macOS, `~/.config/Terraflux/templates` on
Linux). Its file paths are made **absolute** when it is saved, so that it finds `clc.tif` wherever the new model
is saved; move the data and the template must be changed (or give it a node that asks for the file, as the
template *MSPA of your map* does with an empty `path`).

## Common pitfalls

- **A node runs on the wrong band.** [[raster.read]]'s `map` is the first band unless `band` says otherwise. Check
  the band in **Metadata**.
- **The foreground's code.** MSPA without a `foreground` setting expects 2 for the habitat and 1 for the rest
  (0 for no data); a reclassification to 0 and 1 gives an empty result. Either reclassify to 1 and 2, or give
  `foreground` the forest's classes and skip the reclassification.
- **Settings in cells, ecology in metres.** `edge_width` is in cells. The same 2 cells are 50 m on CORINE and 60 m
  on a Landsat map of 30 m. Write the distance in metres in the node's note.
- **Paths that break when the model moves.** Keep data in the model's folder or below it: paths are saved relative
  to it. Absolute paths (`C:\Users\...`) do not travel to a colleague's computer.
- **--set with a misspelt name.** A name without a dot must be an input of the model (`--set landcovr=...` fails
  with *the model has no input*); `node.setting` must name a node of the model (*no node "msp"*). A setting the
  node does not have (`mspa.edge_widht=3`) is not reported, and the run uses the setting of the file: check the
  setting's name in the Node pane or in `terraflux ops`.

## Going further

- [Map algebra](../../building-models/map-algebra/): your own expressions as nodes.
- [Groups and submodels](../../building-models/groups-and-submodels/): a whole model as one node.
- [Loops](../../building-models/loops/): simulations step by step.
- [MSPA](../../habitat-patterns/mspa/): what each class means for species.
- The [CLI reference](/cli/), and the operators [[raster.read]], [[raster.reclassify]], [[landscape.mspa]],
  [[view.plot]], [[raster.write]].

## References

- Harper, K. A., Macdonald, S. E., Burton, P. J., Chen, J., Brosofske, K. D., Saunders, S. C., Euskirchen, E. S., Roberts, D., Jaiteh, M. S., & Esseen, P.-A. (2005). Edge influence on forest structure and composition in fragmented landscapes. *Conservation Biology*, 19(3), 768–782. https://doi.org/10.1111/j.1523-1739.2005.00045.x
- Murcia, C. (1995). Edge effects in fragmented forests: implications for conservation. *Trends in Ecology & Evolution*, 10(2), 58–62. https://doi.org/10.1016/S0169-5347(00)88977-6
- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
