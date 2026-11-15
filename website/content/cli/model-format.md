---
title: The model file format
short: Model format
description: The reference of Terraflux's model file, a graph of operators in TOML or JSON, key by key, with how references resolve, what is layout and what is record, how to check a file and a JSON Schema to validate it.
order: 15
---

A Terraflux **model** is one text file that holds a graph of operators: what the model reads, the operators that
work on it, how they are connected, and what it writes. This page is the reference of that file, for people who
write or generate models by hand or by program (a script, an assistant, another tool) and want to know exactly
what Terraflux reads. To build a model in the application instead, see
[Building models](/tutorials/building-models/the-model-document/); for the commands that run a model, see
[Running models](/cli/models/).

## Overview

A model is a **TOML** file (`.toml`) or a **JSON** file (any other extension). The two hold the same data: a
model converts from one to the other without loss (`terraflux convert model.toml model.json`), and this page
shows the TOML form. Every model starts with the version of the format, `format = "terraflux/1"`.

A small complete model: it draws a landscape, makes a map of its forest, and writes it.

```toml
format = "terraflux/1"
name = "Forest of a random landscape"
description = "A landscape drawn by chance, its forest cells marked, written as a map."

[inputs.seed]
kind = "value"
value = 7

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 100, columns = 100, p = 0.5, weights = [0.6, 0.4], seed = "$seed" }
note = "Two classes in clusters: 1 forest, 2 farmland."

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { l = "landscape.map" }
params = { expression = "l == 1" }
note = "1 where the landscape is forest, 0 elsewhere."

[outputs.forest]
from = "forest.map"
path = "forest.tif"
dtype = "u8"
```

The model has one input (`seed`, a value), two nodes and one output. The node `landscape` reads the input as the
setting `"$seed"`; the node `forest` reads the map of `landscape` through its input `l`; the output writes that
map to `forest.tif`. The operators are [[nlm.random_cluster]] and [[raster.calc]]. Run it with
`terraflux run model.toml`.

## The top-level keys

Only `format` is required. Every other key may be left out; a model with no `nodes` is a valid, empty model.

| Key | Type | Required | Meaning |
|---|---|---|---|
| `format` | text | yes | The version of the format. The only value read is `"terraflux/1"`; a file with any other value is refused with the version it names. |
| `name` | text | no | The model's name. |
| `description` | text | no | What the model is, in a sentence or two. |
| `request` | text | no | The question the model answers, in the words of whoever asked. Record. |
| `summary` | text | no | A summary of the work that led to the model. Record. |
| `author` | text | no | Who made it: a person, an assistant, or both. Record. |
| `inputs` | table of tables | no | The data the model reads, by name. A node reads one as `$name`. |
| `nodes` | list of tables | no | The operators of the graph, in any order. |
| `outputs` | table of tables | no | What the model writes, by name. |
| `parameters` | list of tables | no | The values offered in a form to change a run without editing the graph. |
| `scenarios` | list of tables | no | Named sets of values of the parameters. |
| `tests` | list of tables | no | What `terraflux test` checks: an output against the file of what it should be. |
| `plots` | list of tables | no | Views of the results kept with the model. |
| `notes` | list of tables | no | Notes and frames drawn on the canvas. Layout. |
| `places` | table | no | Where the canvas draws the boxes of the inputs and values a scope reads. Layout. |
| `bookmarks` | list of tables | no | Views of the Viewer's map and of the canvas, kept by name. Layout. |

A name of an input, a node, a port or a plot is a letter or an underscore followed by letters, digits,
underscores or hyphens, in ASCII (`forest_2`, `dist-sea`). `terraflux check` refuses any other name.

## Model inputs

An input is data the model reads by a name. Its `kind` says what it is.

| `kind` | Keys | Meaning |
|---|---|---|
| `map` | `path` (required), `band`, `nodata`, `note` | One band of a raster file, as a map. `band` is a name or a 0-based number (default: the first). `nodata` replaces the file's no-data value. |
| `stack` | `path` (required), `bands`, `nodata`, `note` | Every band of a raster file, or the bands named in the list `bands`, as a stack of maps. |
| `value` | `value` (required), `note` | A number, text, list or table. |

A relative `path` is relative to the folder of the model file. The path of an input can be replaced for one run
(`terraflux run model.toml --set land=other.tif`), and so can the value of a value input.

```toml
[inputs.land]
kind = "map"
path = "land_1997.tif"
band = "1997"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"
bands = ["slope", "dist_sea"]

[inputs.seed]
kind = "value"
value = 7
note = "The seed of the simulation."
```

## Nodes

Each `[[nodes]]` table is one operator.

| Key | Type | Required | Meaning |
|---|---|---|---|
| `id` | text | yes | The node's name, unique among the nodes of its level (the top level, or the body of one container). |
| `op` | text | yes | The operator, such as `raster.read`. The [operator reference](/operators/) lists them with their ports and settings. |
| `inputs` | table | no | What each input port reads, by port name (next section). |
| `params` | table | no | The operator's settings, by name. |
| `body` | table | containers | The nodes of a container, with what it carries and gives. |
| `note`, `why`, `refs`, `assumptions`, `alternatives` | text, and lists of texts | no | The record of the choice (see below). |
| `bypass` | boolean | no | Turn the node off without removing it: it does not run, and each of its outputs is the first of its inputs of the same type (a filter left out passes its map on). Default `false`. |
| `frozen` | boolean | no | Keep the node's outputs on disk, in `<model>.frozen/<id>/` next to the model, and use them instead of running the node. Top-level nodes only. Default `false`. |
| `pos`, `width`, `wires`, `shown`, `hidden` | | no | Layout (see below). |
| `uses` | table | no | On a group that is a node of the user's library: the name and version it was made from, and what it takes and gives. |

Two operators were renamed and still read: `stack.layer` is read as `stack.extract_layer`, and `map.calc` as
`raster.calc`. A model written with map algebra's old syntax, and one that holds `view.plot` nodes (the form
before Plots), is brought up to date when it is read.

The order of the nodes in the file does not matter. Terraflux runs each node after the nodes it reads, and refuses
a graph with a cycle.

## References and how they resolve

A **reference** is a text that names a value. There are three kinds.

| Form | Names | Where it can be written |
|---|---|---|
| `node.port` | An output of a node. | An input, a setting (as `{ from = "node.port" }`), `outputs`, a plot, a body's `carry`, `collect`, `outputs` and `until`. |
| `$name` | An input of the model. | The same, and a setting written as `"$name"`. |
| `@name` | A value of the enclosing container: an input of a group, `@step` in a loop, `@item` in `control.each`. | Inside a container's body only. |

How each resolves:

- **`$name`** looks the name up among the model's `inputs`, from any depth. A name that is not an input is an error.
- **`node.port`** looks for a node with that id, first in the level where the reference is written, then in each
  enclosing level, from the inside out. So a node inside a container can read a node outside it, and a node with
  the same id as an outer node hides it. A node outside a container cannot read a node inside it: the
  container's `body.outputs` or `body.collect` give what leaves. The one exception is a plot's layer, which may
  name a node inside containers as `group.inner.port` or `group/inner.port`.
- **`@name`** looks in the nearest enclosing container that has it. In a `control.group` or `control.switch`, the
  names are the container's own inputs. In a `control.each`, they are `@item`, `@index` and `@name` (the
  element, its place from 1 and its name), and the container's other inputs. In a `control.repeat`, `@step`
  (1, 2, 3...) and every value the body carries. Outside any container, `@name` is an error.

**An input that is a text is always a reference.** In `inputs`, a number, a boolean, a list or a table is a
literal value; a text is read as a reference, so there is no literal text input. Give text through a setting.

**Settings** (`params`) hold literal values (numbers, booleans, texts, lists, tables) as they are. A setting reads
a value instead of holding one in two ways:

```toml
params = { seed = "$seed", rows = "@size", title = { from = "title.text" } }
```

- a text written as `"$name"` or `"@name"` reads an input of the model or a value of the container;
- a table with the single key `from` reads an output of a node (any reference, so it is the way to read a
  `node.port`).

Any other text stays a text, even one such as `"a.b"`. A setting that reads a value must read a number, a whole
number, a boolean or a text. Settings the operator does not read are kept in the file, and `terraflux check`
warns about them.

## Containers and their bodies

A container is a node whose operator is `control.repeat`, `control.each`, `control.switch` or `control.group`,
with a `body` table. The [[control.repeat]], [[control.each]], [[control.switch]] and [[control.group]] pages say
what each does; this is how they are written.

| Key of `body` | Used by | Meaning |
|---|---|---|
| `nodes` | all | The nodes inside, written as `[[nodes.body.nodes]]`. Their ids are unique among themselves. |
| `carry` | `repeat` | `name = "node.port"`: the value that becomes `@name` in the next step. The first value is the container's input of the same name, which must exist. `step` cannot be carried. |
| `collect` | `repeat`, `each` | `name = "node.port"`: the value collected at every step (or element) into a series, a stack of maps or a table, given as the container's output `name`. |
| `outputs` | `group` | `name = "node.port"`: what the group gives, read outside as `group.name`. |
| `order` | `group` | Layout: the order the outputs are shown in. |
| `until` | `repeat` | A reference to a yes-or-no value. When it is true at the end of a step, the loop stops, and that step is neither collected nor carried. |

The container's own inputs are the ports of the container node: `times` for a loop, `over` for `control.each`,
`case` for `control.switch`; a group takes any names. A `control.switch` holds one `control.group` per branch in
its body, named by the value of `case` (`maxent`, `rf`), or `case_1` for the number 1, or `otherwise` for the
rest; every branch must give the same outputs, and the switch carries, collects and gives nothing of its own.
`control.each` carries nothing and has no `until` or `outputs`.

A loop that grows a city by 25 steps:

```toml
[[nodes]]
id = "growth"
op = "control.repeat"
inputs = { times = 25, city = "town.map" }

[nodes.body]
carry = { city = "grow.map" }
collect = { maps = "grow.map" }

[[nodes.body.nodes]]
id = "grow"
op = "raster.calc"
inputs = { c = "@city", suit = "suitability.map", step = "@step" }
params = { expression = "1 if c == 0 and suit > 0.6 - step * 0.008 else c" }
```

Outside, `growth.maps` is the stack of every step's map and `growth.city` the last one. A node of the file may
also be a `control.submodel`, which stands for another model file (`params.path`, relative to this model's
folder); it is expanded into a group before the model runs. See
[Groups and submodels](/tutorials/building-models/groups-and-submodels/) and
[Loops](/tutorials/building-models/loops/).

## Outputs

An output is a value the model writes into the output folder (the folder `output` next to the model, or the one
`--out` names).

| Key | Required | Meaning |
|---|---|---|
| `from` | yes | The value written: `node.port`, `$name` or `@name`. |
| `path` | no | The file, relative to the output folder. Default: the output's name with the usual extension. |
| `dtype` | no | The cell type of a raster: `u8`, `i32`, `f32` and so on. |
| `layer` | no | One layer of a stack, one of several rasters, or one step of a series, written alone: its name or 0-based number, as text. |

## Parameters and scenarios

A **parameter** names a value of the model a person may change in a form: a value input, or a setting of a node.
It changes nothing by itself and holds no value; the value stays where it is written.

| Key | Required | Meaning |
|---|---|---|
| `key` | yes | What it changes: `$name` (a value input) or `node.setting` (`container/node.setting` inside a container). |
| `label` | no | Its name in the form. Default: the key. |
| `unit` | no | A unit shown beside it. |
| `min`, `max`, `step` | no | The range of a number (a slider when both ends are given) and its step. |
| `choices` | no | The values it takes, when they are a few. |
| `help` | no | What it is and how it acts on the model. |

A **scenario** is a named set of values, run with `terraflux run model.toml --scenario "by distance"`:

```toml
[[parameters]]
key = "$method"
label = "Method"
choices = ["distance", "neighbourhood"]

[[scenarios]]
name = "by distance"
note = "The nearer the farmland, the more suited."
values = { "$method" = "distance" }
```

`values` maps keys, written as in a parameter's `key`, to values. `terraflux check` warns of a key that names
nothing the model has (a node renamed, say).

## Tests

A `[[tests]]` table says what a value of the model should be. `terraflux test` runs the model and compares.

| Key | Required | Meaning |
|---|---|---|
| `from` | yes | The value tested, `node.port`. |
| `expected` | yes | The file of what it should be (a raster, a CSV table, a JSON value or a text), relative to the model's folder. |
| `tolerance` | no | The largest difference of a number allowed. Default `1e-9`. |
| `note` | no | What the test checks. |

## Plots

A `[[plots]]` table is a view of the results kept with the model: what the Viewer shows, saved. Plots sit outside
the graph: they are never run, and the Document, the reports, the QGIS export and the Python script read them.

| Key | Meaning |
|---|---|
| `id` (required) | The plot's name, unique among the plots. |
| `title`, `note`, `why` | Its caption and its record. |
| `layers` | The layers, the first at the bottom. A layer has `from` (required: `node.port`, `$name`, or a node inside containers), and optionally `opacity` (0 to 1), `palette`, `range` (`[least, most]`), `band`, `style`, `minus` and `minus_band` (drawn as a difference), `hidden`, `rgb` (a colour composite), `selected`, `blend` (`normal`, `multiply`, `screen`), `shade` (a relief of its own), `name` and `group`. |
| `compare` | A map compared with the top map: `with` (required), `how` (`curtain`, `side by side`, `difference`), `also`, `band`, `also_bands`, `layers`. |
| `chart` | A table's chart: `kind` (`bars`, `lines`, `points`, `boxes`), `x`, `y` (columns, comma-separated), `by`. |
| `view` | The saved part of the map: `extent = [west, south, east, north]`, with an area. |
| `groups` | Groups of layers, one level: each has `id`, `name` and optionally `collapsed`, `hidden`, `opacity`. |

```toml
[[plots]]
id = "city"
title = "The city after 25 steps"

[[plots.layers]]
from = "growth.city"
palette = "viridis"
```

## The record: note, why, references

A node's **record** says why it is there. None of it changes what the node gives, and changing it does not run
the node again.

| Key | Meaning |
|---|---|
| `note` | What the node is for, in a sentence. |
| `why` | Why this operator and these settings, and not others. |
| `refs` | The references the choice rests on (papers, methods, the operator's documentation), a list of texts. |
| `assumptions` | What the node assumes to be true, a list of texts. |
| `alternatives` | The alternatives considered and set aside, and why, a list of texts. |

The model has its own at the top: `request`, `summary` and `author`. The Document pane and the reports tell the
record beside the graph; see [The model's document](/tutorials/building-models/the-model-document/).

## Layout: positions, frames and wires

Some keys only say how the model is drawn on the canvas. **They do not change a result**: the engine ignores
them, and a model with all of them removed gives the same maps and tables.

- `pos = [x, y]` and `width` of a node: where its box is and how wide, in the coordinates of its level.
- `wires`: reroute points of the wires a node reads, a table from an input port name to a list of `[x, y]` points,
  from the output to the input. The wire into a *setting* (a setting that reads a node) is kept under
  `setting:<name>`:

  ```toml
  [nodes.wires]
  city = [[120.0, 40.0], [160.0, 40.0]]
  "setting:title" = [[90.0, 200.0]]
  ```

- `shown` and `hidden` of a node: settings its box always shows, or keeps folded.
- `places` of the model: where the canvas draws the boxes of what a level reads besides its nodes. The key is the
  containers' ids joined by `/`, then `|`, then the reference: `"|$land"` at the top, `"loop|@step"` in `loop`.
- `[[notes]]`: free texts (`at = [x, y]`) and frames around nodes (`nodes = [...]`, the text's first line the
  title), each with an `id`, a `text`, an optional `colour` (`amber`, `blue`, `green`, `red`, `purple`, `grey`) and
  a `scope` (the ids of the containers it is in, from the top; none for the top level). Checks and runs never
  see them.
- `[[bookmarks]]`: places kept by name, each with a `name`, an optional `slot` (1 to 9) and a `view` that is
  either `kind = "map"` (`extent`, optional `crs` and `layers`) or `kind = "canvas"` (`x`, `y`, `zoom`, and
  `scope`).
- `order` of a group's `body`: the order its outputs are shown in.

Two layout-looking keys do change the results: `bypass` and `frozen` (see Nodes).

## TOML and JSON

TOML and JSON hold the same data, key for key; the file's extension chooses the reader (`.toml`, in any case,
reads TOML; every other extension reads JSON). The list `[[nodes]]` is a JSON array of objects, `[inputs.seed]` is
an object under `inputs`, and the TOML inline table `{ rows = 100 }` is a JSON object. The same loop in JSON:

```json
{
  "format": "terraflux/1",
  "nodes": [
    {
      "id": "growth",
      "op": "control.repeat",
      "inputs": { "times": 25, "city": "town.map" },
      "body": {
        "carry": { "city": "grow.map" },
        "collect": { "maps": "grow.map" },
        "nodes": [
          { "id": "grow", "op": "raster.calc", "inputs": { "c": "@city" }, "params": { "expression": "c" } }
        ]
      }
    }
  ]
}
```

JSON has `null` and TOML does not. An optional key (one that holds a single value, such as `note`) set to `null`
is read as not given; Terraflux never writes one, and the schema does not accept it.
Terraflux writes TOML with the keys it knows in a fixed order and leaves out every key that has its default
value, so a file saved by the application is shorter than one written by hand.

## Keys Terraflux does not know

Terraflux **ignores a key it does not know**, at any level of the file: a misspelt `nodse` or a `colour` on a
node gives no error and does nothing. This lets a model written by a later version, with keys this one has not
heard of, open in an earlier one. The unknown keys are lost when the earlier version saves the model again, as
it writes only the keys it knows.

The exceptions are the keys that are *values* and not part of the format: the settings in a node's `params` and
the keys of `values` in a scenario are kept as they are. A setting the operator does not read gets a warning from
`terraflux check`, and an input port the operator does not have is an error.

Because a misspelt key is silent, check a hand-written file with `terraflux check` (it catches wrong operators,
ports, settings and references) and with the schema below. The schema also accepts unknown keys, as Terraflux does, so it catches a misspelt key
only where it leaves out a required one (`op` written as `operator`, say).

## Compatibility

A model saved by an earlier version of Terraflux opens in later ones. This holds from 1.0.0 on, when the format
was named `terraflux/1`: new keys are added as optional (the record of 1.0.0, the Plots, the layout keys), and
what an older file used is read and brought up to date (operators renamed, old map algebra syntax, `view.plot`
nodes made Plots). A change that old models could not survive would come only with a new major version of
Terraflux, and a new `format` value, which an older version refuses by name.

## Validate a file

**With Terraflux.** `terraflux check` reads the file, checks every operator, setting and connection (an output
must fit the input it feeds), the names, the order of the graph, and that every input file exists. It exits with
1 when there is an error:

```text
> terraflux check model.toml
model.toml: no errors
```

**With the JSON Schema.** [model.schema.json](/schema/model.schema.json) (JSON Schema draft 2020-12, `$id`
`https://sacridini.github.io/terraflux/schema/model.schema.json`) describes the structure of the file: the keys,
their types, the three kinds of input, the plots, and the names and references that `check` also requires. It
does not know the operators, so it cannot say whether a port or a setting exists: use `terraflux check` for that.
Editors that know JSON Schema (VS Code with a YAML or TOML extension, for instance) can use it to complete and
check a model.

A TOML model validates as its data, so a script parses the TOML first (`tomllib` is in Python 3.11 and later).
With Python's `jsonschema`:

```python
import json, tomllib, urllib.request
from jsonschema import Draft202012Validator

url = "https://sacridini.github.io/terraflux/schema/model.schema.json"
schema = json.load(urllib.request.urlopen(url))
with open("model.toml", "rb") as f:
    model = tomllib.load(f)

for error in Draft202012Validator(schema).iter_errors(model):
    path = "/".join(str(p) for p in error.absolute_path)
    print(f"{path or '(top)'}: {error.message}")
```

For a JSON model, `check-jsonschema` does it from the command line:

```bash
pip install check-jsonschema
check-jsonschema --schemafile https://sacridini.github.io/terraflux/schema/model.schema.json model.json
```

To check a TOML model with it, convert it first: `terraflux convert model.toml model.json`.

## Going further

- [The model's document](/tutorials/building-models/the-model-document/): the record read as the methods of a
  paper.
- [Groups and submodels](/tutorials/building-models/groups-and-submodels/) and
  [Loops](/tutorials/building-models/loops/): containers in the application.
- [Running models](/cli/models/): `run`, `check`, `test`, `convert`.
- [The operator reference](/operators/): every operator, with its ports and settings.
