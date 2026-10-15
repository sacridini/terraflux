---
title: Groups and submodels
description: Gather a few nodes into one with control.group, and use a whole model file as one node with control.submodel, to find the core forest of Sibuyan Island at two dates with one model written once.
order: 40
level: intermediate
time: 35 min
ops: [control.group, control.submodel, landscape.mspa, map.calc, raster.read]
---

Models grow. A land change model may have a hundred nodes; a habitat analysis that you repeat for every year of a
time series or every species of a study has the same ten nodes again and again. In this tutorial you learn the
two ways Terraflux has to keep such models short and readable: a **group** ([[control.group]]) gathers a few nodes
of a model into one box, and a **submodel** ([[control.submodel]]) uses another model file as a single node. You
will write a small model that finds the **core forest** of a land use map, and use it twice, on the maps of 1997
and 2011 of Sibuyan Island in the Philippines, to measure how much core forest was lost in fourteen years.

## The idea

Programmers solved this problem long ago with *functions*: write a piece of work once, give it a name, its inputs
and its outputs, and call it wherever it is needed. Changing the function changes every call; reading the
program, you see the name and not the details. Ecological modellers have the same need. Land change software such
as Dinamica EGO organises its models in *containers* and *submodels* for exactly this reason (Soares-Filho et al.,
2002, 2009), and scientific workflow systems argue that modular, documented pieces are what makes an analysis
reproducible by others (Sandve et al., 2013).

Terraflux has two containers that run their body **once** (a third, [[control.repeat]], runs it many times; see
[Loops](../loops/)):

- **[[control.group]]**: nodes kept *inside* the model, in a box with its own inputs and outputs. Inside the box,
  its inputs are written `@name`; its outputs are references to nodes of its body, listed in `body.outputs`.
- **[[control.submodel]]**: a node that stands for **another model file**. The other model's inputs (its
  `[inputs]`) become the node's inputs; its outputs (its `[outputs]`) become the node's outputs.

!!! science "Key idea"
    A submodel is a group whose body lives in another file. Before every run (in the application, `terraflux run`,
    `check` and `submit`), each `control.submodel` node is **expanded**: the other model's nodes are copied into
    the node's body, its inputs `$name` become `@name`, and the node becomes a `control.group`. So the two behave
    the same; they differ in where the body is written, and so in who can reuse it.

## When to use it

- **The same steps on several maps.** Two dates of land use, several species, several regions: write the steps
  once as a submodel and give it each map. The core forest of 1997 and of 2011 below is one model used twice.
- **A library of your methods.** Your preparation of a land cover map, your habitat suitability index, your
  standard set of landscape metrics: kept in a folder `lib/`, shared with your lab, used by all your models. A
  correction made in the submodel reaches every model that uses it.
- **Readability.** A group puts five nodes that compute one thing (a dynamic variable, the context of a cell)
  behind one name. The canvas shows one box; double-click it to see inside.
- **Inside loops.** A group inside a [[control.repeat]] body keeps the body readable: one box per idea.

What they are *not* for: running a model many times with different settings or seeds. That is a batch
(`terraflux submit`; see [Randomness and reproducibility](../randomness/) and the [parallel computing
guide](/parallel/)). A group or submodel runs once per run of the model that holds it (or once per step, inside a
loop).

## What you need

- `sibuyan_landuse.tif`, the land use of **Sibuyan Island** (Philippines), in 1997 and 2011 (two bands named
  `1997` and `2011`), 128 x 108 cells of 250 m, classes 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other, the
  sea as no data. It is the classic example of the CLUE-S land change model (Verburg et al., 2002) as distributed
  with R's lulcc package (Moulds et al., 2015); see [Your first raster](../../getting-started/first-raster/).
- Two files of your own: the submodel, `lib/forest_core.toml`, and the model that uses it, `main.toml`, in the
  folder of the map:

```text
my-study/
├── sibuyan_landuse.tif
├── main.toml
└── lib/
    └── forest_core.toml
```

## A group: the context of every forest cell

Start with a group. Many analyses need, for every cell, a few maps about its surroundings: is it forest, what share
of the cells around it is forest, is it at the forest's edge. Three [[map.calc]] nodes do it. In a group they are
one box with one input, the land use, and two outputs, the share of forest around each cell and the edge.

```tfgraph
#! caption: The group "context": its input land is @land inside; its body makes the forest (1/0), its share in the 5 x 5 cells around each cell, and the forest cells whose surroundings are not all forest; body.outputs names what comes out.
#! highlight: context
format = "terraflux/1"

[[nodes]]
id = "lu1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "context"
op = "control.group"
inputs = { land = "lu1997.map" }
note = "The forest around every cell: its share in 5 x 5 cells, and the forest's edge."

[nodes.body]
outputs = { share = "share.map", edge = "edge.map" }

[[nodes.body.nodes]]
id = "forest"
op = "map.calc"
inputs = { l = "@land" }
params = { expression = "l == 1" }

[[nodes.body.nodes]]
id = "share"
op = "map.calc"
inputs = { f = "forest.map" }
params = { expression = "nbmean(f, 2)" }

[[nodes.body.nodes]]
id = "edge"
op = "map.calc"
inputs = { f = "forest.map", s = "share.map" }
params = { expression = "f == 1 and s < 1" }
```

Read the group from the outside in:

- The node `context` is a [[control.group]]. Its **inputs** take any names: here `land`, connected to
  `lu1997.map`. Any number of inputs can be added, maps, stacks or numbers.
- Inside the body, `@land` is that input. The body's nodes are ordinary nodes; they can also read nodes of the
  outer model directly, such as `lu1997.map`.
- `body.outputs` says what the group gives: `share = "share.map"` makes the group's output `context.share` the
  map of its inner node `share`. Outside, other nodes connect to `context.share` and `context.edge` as to any
  node's outputs; the inner nodes `forest`, `share` and `edge` are not visible outside.

```tfgallery
columns: 2
caption: "The group's two outputs for 1997. Left, context.share: the share of forest in the 5 x 5 cells (1.25 x 1.25 km) around each cell, from 0 on the coastal plains to 1 in the interior. Right, context.edge: forest cells with some non-forest around them."
images:
  - { src: /img/tutorials/building-models/groups-and-submodels/share.png, caption: "context.share" }
  - { src: /img/tutorials/building-models/groups-and-submodels/edge.png, caption: "context.edge" }
```

!!! app "In the application"
    Add a group from the library's subject **Model flow** ([[control.group]]). In the Node pane, **Add input**
    gives it an input: type a name (`land`) and a reference (`lu1997.map`), or a value. **Edit the body** (or a
    double click on the group) takes you inside: the column on the left of the canvas lists what the body can
    read, `@land` among them, and you add nodes as at the top level. The trail in the Model pane's bar (*Top
    level / context*) takes you back. To set an output, select the group and, in the Node pane's **Body**
    section, choose `outputs`, type its name (`share`) and the inner reference (`share.map`), and click **Set**.

## A submodel: a model used as a node

A group is part of one model. To use the same steps in several models, or several times in one, write them as a
model of their own: a **submodel**. Any model with `[inputs]` and `[outputs]` can be one.

### Step 1: the submodel

`lib/forest_core.toml` takes a land use map and gives two maps: MSPA of its forest, and a map of core forest (1)
against the rest of the land (0). MSPA (Soille & Vogt, 2009) splits the forest into core, edge, bridges and other
classes; the [MSPA tutorial](../../habitat-patterns/mspa/) explains them. Here the edge is 2 cells (500 m) wide.

```tfgraph
#! caption: lib/forest_core.toml, the submodel. Its input landuse ($landuse) has a file of its own, used when the model runs alone; its outputs are core and mspa.
#! numbered: true
format = "terraflux/1"
name = "Forest core"
description = "The core of a land use map's forest: MSPA of its forest (class 1), and a map of core (1) and the rest of the land (0)."

[inputs.landuse]
kind = "map"
path = "../sibuyan_landuse.tif"
band = "1997"
note = "A land use map whose class 1 is forest; a node that uses this model gives its own."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "$landuse" }
params = { foreground = [1], edge_width = 2 }
note = "The forest split into core, edge, bridges...; an edge of 2 cells (500 m)."

[[nodes]]
id = "core"
op = "map.calc"
inputs = { m = "mspa.map", lu = "$landuse" }
params = { expression = "if isnull(lu) then null else m == 17 or m == 117" }
note = "1 where the forest is core (17, or 117 inside a core-opening), 0 on the rest of the island."

[outputs.core]
from = "core.map"

[outputs.mspa]
from = "mspa.map"
```

Three things make it a good submodel:

1. **It runs on its own.** Its input `landuse` has a file (`../sibuyan_landuse.tif`, the band of 1997), so
   `terraflux run lib/forest_core.toml` runs it and you can test it alone, without the model that will use it.
2. **Its inputs and outputs have clear names.** They are the submodel's "signature": `landuse` in, `core` and
   `mspa` out. A node that uses it will connect to these names.
3. **Its notes say what it expects** (a map whose class 1 is forest) and what each setting means.

Node 2 is [[map.calc]] with the expression `if isnull(lu) then null else m == 17 or m == 117`: MSPA's code for
core is 17, and 117 for core inside a *core-opening* (a hole in the forest); every other cell of the island is
0, and the sea, which has no value in the land use, stays without one. See [Map algebra](../map-algebra/) for the
language.

### Step 2: use it twice

`main.toml` reads the two dates and uses the submodel for each:

```tfgraph
#! caption: main.toml. The two submodel nodes run lib/forest_core.toml, one on each date; change compares their core maps.
#! highlight: core1997, core2011
#! numbered: true
format = "terraflux/1"
name = "Core forest lost"

[[nodes]]
id = "lu1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "lu2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "core1997"
op = "control.submodel"
inputs = { landuse = "lu1997.map" }
params = { path = "lib/forest_core.toml" }
note = "The core forest of 1997."

[[nodes]]
id = "core2011"
op = "control.submodel"
inputs = { landuse = "lu2011.map" }
params = { path = "lib/forest_core.toml" }
note = "The core forest of 2011."

[[nodes]]
id = "change"
op = "map.calc"
inputs = { a = "core1997.core", b = "core2011.core" }
params = { expression = "a * 2 + b" }
note = "0 never core, 1 became core, 2 core lost, 3 core at both dates."
```

Each [[control.submodel]] node has one setting, `path`: the submodel's file, **relative to the folder of the model
that uses it**. Its inputs have the names of the submodel's inputs (`landuse`), and its outputs the names of the
submodel's outputs: `core1997.core`, `core1997.mspa`, `core2011.core`... Node 5 combines the two core maps into one
code per cell: `a * 2 + b` is 3 when the cell is core at both dates, 2 when it was core in 1997 and not in 2011, 1
when it became core, 0 when it was never core.

Check and run it from the folder of `main.toml`:

```text
$ terraflux check main.toml
main.toml: no errors

$ terraflux run main.toml
Core forest lost: inputs read (0.0 s)
done in 0.0 s
```

## What happens to the inputs a node does not give

When a submodel node is expanded, each of the submodel's inputs is filled in this order:

1. **the node gives it**: `landuse = "lu1997.map"` above;
2. **a value input** (`kind = "value"`) the node does not give keeps the submodel's own value: a submodel can have
   defaults, such as a threshold;
3. **a file input** (`kind = "map"` or `"stack"`) the node does not give becomes an **input of the outer model**,
   named after the node and the input: `core1997__landuse`, with the submodel's file (its path made relative to
   the outer model's folder). It can then be replaced from the command line like any input
   (`--set core1997__landuse=other.tif`).

Giving the node an input the submodel does not have is an error (*the submodel "core1997" has no input "..."*).
Submodels may use submodels, up to 16 deep, but never themselves.

!!! warning "Settings are not inputs"
    A submodel's **settings** (MSPA's `edge_width` above) are fixed in its file: a node that uses the submodel
    cannot give them, because settings cannot be connected to inputs. Writing a setting as a reference to an input
    (`edge_width = "$width"`) does not work either: `terraflux check` stops on it with *settings take values, not
    references*. To make a setting change from use to use, either write variants of the submodel, or, when the
    operator takes the value as an *input port* (a number in a [[map.calc]] expression, the `seed` of a
    simulation), declare it as a value input of the submodel and give it from the node.

    For a one-off run, the command line can still change a setting inside a submodel: its nodes are named after
    the node that uses it, `core1997/mspa` for the MSPA of `core1997`, so
    `terraflux run main.toml --set core1997/mspa.edge_width=3` runs the 1997 core with an edge of 3 cells (750 m)
    and leaves 2011's at 2. Check the name: a setting the node does not have is not reported.

## Read the results

```tfgallery
columns: 2
caption: "MSPA of Sibuyan's forest in 1997 and 2011 (edge of 500 m), the outputs core1997.mspa and core2011.mspa. Core in green, edge in black. Between the dates the forest retreated from the coast and from the valleys of the east, and the core with it."
images:
  - { src: /img/tutorials/building-models/groups-and-submodels/mspa1997.png, caption: "1997" }
  - { src: /img/tutorials/building-models/groups-and-submodels/mspa2011.png, caption: "2011" }
```

![The core forest lost between 1997 and 2011](/img/tutorials/building-models/groups-and-submodels/change.png "change.map: the core forest of both dates compared, as shares of the island's 7,226 cells. 544 cells (34 km²) of core forest in 1997 were not core in 2011; none became core.")

| | Cells of 250 m | km² |
|---|---|---|
| Forest in 1997 | 4,723 | 295.2 |
| Forest in 2011 | 4,160 | 260.0 |
| Core forest in 1997 | 3,673 | 229.6 |
| Core forest in 2011 | 3,129 | 195.6 |
| Core lost (code 2) | 544 | 34.0 |
| Core gained (code 1) | 0 | 0 |

The forest shrank by 11.9% and its core by 14.8%: the core lost a larger share than the forest. That is the usual
signature of fragmentation: clearing at the forest's border turns the core behind it into edge, so each hectare
cleared costs more than a hectare of core (Haddad et al., 2015). Reported in a paper, the methods would say: *the
core forest was mapped with MSPA (GuidosToolbox's, as implemented in Terraflux 0.79) on the forest class of each
date, with an edge width of 500 m and 8-connectivity*, and the model files would go in the supplementary
material: `main.toml` and `lib/forest_core.toml` together.

## Settings that change the answer

Everything that changes the core of one date changes it for both, because both use the same file: change
`edge_width` in `lib/forest_core.toml` and run `main.toml` again. That is the point of a submodel: one place to
change a method, no risk that the two dates were treated differently. The edge width itself matters a lot (see
[Your first model](../../getting-started/first-model/#settings-that-change-the-answer) and the
[MSPA tutorial](../../habitat-patterns/mspa/)).

## Common pitfalls

- **A path relative to the wrong folder.** The `path` of a submodel node is relative to the model that uses it;
  the paths *inside* the submodel (its input files, its [[raster.read]] and [[raster.write]] nodes, its plugins)
  are relative to the **submodel's** own folder. Terraflux makes them absolute when it expands the submodel, so
  each file is found where its own model says.
- **Writing files from a submodel.** A [[raster.write]] inside a submodel used twice writes the same file twice,
  the second run over the first. Give the outputs to the outer model instead (`[outputs]` of the submodel), and
  write them there.
- **Names inside and outside.** Inside a group, its inputs are `@name`; inside a submodel file, they are `$name`
  (it is a model of its own). Writing `$land` inside a group's body refers to the outer model's input `land`, not
  to the group's.
- **A group's output not set.** A group without `body.outputs` gives nothing: the nodes inside run, but no other
  node can use their results.
- **A submodel that is not checked alone.** Run `terraflux check lib/forest_core.toml` on the submodel too: an
  error inside it appears in the outer model under the node's name, which is harder to read.

## Going further

- [Loops](../loops/): [[control.repeat]], the container that runs its body many times.
- [Map algebra](../map-algebra/): the expressions of [[map.calc]].
- [Plugins in WebAssembly](../plugins/): your own operator, written in any language.
- [MSPA](../../habitat-patterns/mspa/), and the [Land change simulation](../../land-change/) tutorials, whose
  models use groups inside their loops.

## References

- Haddad, N. M., Brudvig, L. A., Clobert, J., Davies, K. F., Gonzalez, A., Holt, R. D., Lovejoy, T. E., Sexton, J. O., Austin, M. P., Collins, C. D., Cook, W. M., Damschen, E. I., Ewers, R. M., Foster, B. L., Jenkins, C. N., King, A. J., Laurance, W. F., Levey, D. J., Margules, C. R., ... Townshend, J. R. (2015). Habitat fragmentation and its lasting impact on Earth's ecosystems. *Science Advances*, 1(2), e1500052. https://doi.org/10.1126/sciadv.1500052
- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Sandve, G. K., Nekrutenko, A., Taylor, J., & Hovig, E. (2013). Ten simple rules for reproducible computational research. *PLoS Computational Biology*, 9(10), e1003285. https://doi.org/10.1371/journal.pcbi.1003285
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Soares-Filho, B., Rodrigues, H., & Costa, W. (2009). *Modeling environmental dynamics with Dinamica EGO*. Centro de Sensoriamento Remoto, Universidade Federal de Minas Gerais.
- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
