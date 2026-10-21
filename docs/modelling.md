# Changing a model's logic

A model is a graph of operators (`terraflux ops` lists them). Besides the presets' operators, four tools
let you change what a model does without changing Terraflux:

- the node editor;
- map algebra;
- groups and submodels;
- parameters and scenarios;
- plugins in WebAssembly.

## The node editor

The **Graph** tab of the application edits the model:

- **What a scope reads:** the column on the left. At the top level these are the model's inputs. Inside a
  container they also include its values (`@step` and the carried values of a loop, a group's inputs) and the
  outer nodes its body reads.
- **Nodes and connections:** each node shows its input ports on the left (required ones in orange until
  connected) and its output ports on the right. To connect two nodes, drag from an output to an input. Each
  connection is checked when it is made: a value of the wrong type, a cycle or a port the operator lacks is
  refused, with the reason.
- **Adding and moving:** pick an operator in the bar and click **Add node**. Drag a node by its header to
  move it; its place is saved in the model (`pos`).
- **The side panel:** click a node to edit it there. You can rename it (its references follow), type the
  value of an input, change its settings, add inputs (map algebra, groups, plugins), set a container's
  outputs, or delete it. Below them is the node's record: its note (what it is for), why it was chosen, the
  references it rests on, its assumptions and the alternatives set aside. With no node selected, the panel shows
  the model's record: its name, description, the request it answers, a summary of the conversation that led to
  it, and its author.
- **The record in the file:** `note`, `why`, `refs`, `assumptions` and `alternatives` on a node; `request`,
  `summary` and `author` on the model. All are optional, none changes what the model gives (a node whose record
  changes does not run again), and models without them stay valid.
- **Reviewing changes:** what an assistant changes is listed in the **Changes** pane, each change accepted or
  rejected; File, Compare lists the differences with the saved file, another file or the last commit.
- **Containers:** double-click a container, or use **Edit the body**, to edit its body. The trail in the bar
  takes you back up.
- **Saving:** **Save** writes the model where it was read, as written, with its submodels kept as
  references.

## Map algebra: `map.calc`

`map.calc` computes a map cell by cell from an expression, as Dinamica EGO's Calculate Map. Its inputs take
any names, and the expression uses those names:

```toml
[[nodes]]
id = "suitable"
op = "map.calc"
inputs = { land = "$t0", drivers = "$drivers", k = 500 }
params = { expression = "if land == 3 and drivers.dist_roads < k then 1 else 0" }
```

| | |
|---|---|
| values | numbers, `pi`, `null` (a missing cell), maps and numbers by input name, the layers of a stack as `stack.layer` |
| arithmetic | `+ - * / % ^` |
| comparisons | `< <= > >= == !=` (1 or 0) |
| logic | `and or not` (also `&& \|\| !`), `if c then a else b`, `c ? a : b` |
| functions | `abs sqrt exp ln log10 floor ceil round min max clamp(x, lo, hi) isnull(x) coalesce(x, y)` |
| neighbourhoods | `nbsum nbmean nbmin nbmax nbcount (x, r)`: the square of radius `r` cells around each cell, the cell included and missing cells left out |

A missing value makes the result missing; `isnull` tests for it, and `coalesce` replaces it. For example,
`nbmean(land == 2, 1)` is the share of class 2 in each 3 × 3 window.

## Groups and submodels

A `control.group` runs its body once. Its inputs (any names) are `@name` inside, and its outputs are
references of its body (`body.outputs`). A group gathers a few nodes into one, for instance to compute a
dynamic variable. Its body also reads the nodes around it (`node.port`), as a loop's does.

In the editor, ctrl+G puts the nodes selected into a new group (what read them from outside reads the group's
outputs, named after them) and ctrl+shift+G takes a group apart. **Group the model by stage** (ctrl+K) puts the
top level in a group per stage (data, preparation, model, validation, results), each with a note of what it is
for; the plots, the files written and the random generators stay outside. The canvas then shows a few blocks
linked by what they read, their notes under them, each opened by itself (a double click); zoomed out, a model
without groups shows its stages framed and named. A group keeps what its body gave in a run, so the Document pane
shows its steps' maps; it runs again whole when one of its nodes changes.

A `control.submodel` uses another model file as one node:

```toml
[[nodes]]
id = "forest"
op = "control.submodel"
inputs = { lu = "$t0" }
params = { path = "lib/forest.toml" }    # relative to this model's folder
```

The other model's inputs are the node's inputs of the same names. A value input the node does not give keeps
its own value; a file input it does not give becomes an input of the outer model, named `forest__name`. The
other model's outputs become the node's (`forest.map`).

Before a run, every submodel is expanded into a group (`terraflux run`, `check`, `submit` and the application
do it). Submodels may use submodels, but never themselves.

## Loops over collections: `control.each`

A `control.each` runs its body once for each element of a collection: the layers of a stack (a folder of rasters,
the bands of a file, 38 years of MapBiomas), the items of a series, the rows of a table (each a table of one row),
the features of vectors (each alone: 50 municipalities) or the values of a list (30 species). Inside, `@item` is
the element, `@index` its place (1, 2...) and `@name` its name (a layer's name, a feature's attribute `name`, the
first column of a row, the value of a list); its other inputs are `@name` as a group's.

```toml
[[nodes]]
id = "by_year"
op = "control.each"
inputs = { over = "years.raster", threshold = 0.4 }
[nodes.body]
collect = { forest = "forest.map", areas = "areas.table" }
[[nodes.body.nodes]]
id = "forest"
op = "map.calc"
inputs = { cover = "@item", t = "@threshold" }
params = { expression = "cover > t" }
[[nodes.body.nodes]]
id = "areas"
op = "raster.stats"
inputs = { map = "forest.map" }
```

What it collects is gathered: maps a stack of the elements' names (`by_year.forest`), tables one table with a
column `item` first (the year of `forest_2010`, the number a name ends with, or the element's place), other values
a series. Each element runs alone: nothing is carried from one to the next (that is `control.repeat`). A node of
one map given a stack runs once per layer by itself; `control.each` is for a body of several nodes.

## Choosing a branch: `control.switch`

A `control.switch` runs one of its branches: its body holds a group per branch, and the one whose id is the value
of its input `case` runs (`maxent`, `rf`; `case_2` for the number 2, `true` or `false` for a yes or no), or the
group `otherwise` when none is; the others do not run. Every branch gives the same outputs, which are the switch's
(`suitability.map`), with `branch`, the id of the one that ran. Its other inputs are `@name` inside, as a group's.

```toml
[[nodes]]
id = "suitability"
op = "control.switch"
inputs = { case = "$method", farm = "farmland.map" }
[[nodes.body.nodes]]
id = "distance"
op = "control.group"
inputs = { f = "@farm" }
[nodes.body.nodes.body]
outputs = { map = "near.map" }
# ... the nodes of the branch
[[nodes.body.nodes]]
id = "neighbourhood"
op = "control.group"
inputs = { f = "@farm" }
[nodes.body.nodes.body]
outputs = { map = "share.map" }
# ...
```

With the case an input of the model, a parameter (`choices = ["distance", "neighbourhood"]`) chooses the method, and
a scenario for each compares them side by side (the template Choose the method: a switch and its scenarios).

## Numbers and texts: `value.calc`

`value.calc` makes a number or a text that is not a map from its inputs: `expression` in map algebra's syntax and
functions over the inputs that are numbers (true and false as 1 and 0; a table's value as `table.column`, of the row
`row`, 1 the first and -1 the last), and `text` a template of the inputs, `{name}` replaced by each one's value and
`{value}` by the expression's (`{{` and `}}` for braces):

```toml
[[nodes]]
id = "path"
op = "value.calc"
inputs = { species = "@name", step = "@step" }
params = { text = "out/{species}_{step}.tif" }

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "model.map" }
params = { path = { from = "path.text" } }
```

Its value and its text are what settings read, the case of a switch, a threshold derived from a table.

## Settings that read values

A setting holds a value of its own, or reads one when the model runs: `"$name"` (an input of a value of the
model), `"@step"` or `"@name"` (a loop's step or carried value, a group's input), or `{ from = "node.port" }`
(another node's number, true or false, or text; written so because a text such as `"a.b"` stays a text).

```toml
[[nodes.body.nodes]]
id = "growth"
op = "hydro.streams"
inputs = { accumulation = "acc.map" }
params = { threshold = "@step" }          # a threshold that changes with the loop's step

[[nodes]]
id = "dci"
op = "hydro.dci"
inputs = { dem = "dem.map", streams = "rivers.map", barriers = "dams.vector" }
params = { passability = "$passability" }  # one value given to every setting that reads it
```

The node read runs first, as for an input. A value that is not a number, true or false or a text (a map, a
table) is refused when the model is checked. In a submodel, a setting that reads one of its inputs (`"$threshold"`)
is a setting the node that uses it gives (`inputs = { threshold = 0.3 }`), its own value when it does not: a
submodel shows its settings as inputs. In the application, a right click on a setting's name reads a value of the
model instead, and its row shows what it reads (`← $passability`).

## Parameters and scenarios

A model's **parameters** are the values its users change: an input of a value (`$name`) or a node's setting or
literal input (`node.setting`, `container/node.setting`), each with what a form needs. They change nothing by
themselves; the Parameters pane shows them as a form (a slider for a number in a range, a list for a few choices,
a box for yes or no), and each change is an edit of the model that runs it again.

```toml
[[parameters]]
key = "aptitude.range"
label = "Range of the suitability"
unit = "cells"
min = 5
max = 50
step = 5
help = "How far apart suited cells are."

[[scenarios]]
name = "patchy"
note = "A suitability of small patches."
values = { "aptitude.range" = 5 }
```

**Scenarios** are named sets of those values. The pane saves the values now as one, applies one in a click,
compares those checked side by side (each run up to an output chosen: its maps, tables or numbers in a column per
scenario, and their differences from the first), and sends them to Many runs (as its table of scenarios, or the
parameters of a range as its calibration). `terraflux run model.toml --scenario patchy` runs one.

## Plugins: `plugin.wasm`

A plugin is a function compiled to WebAssembly from any language (Rust, C, Zig, AssemblyScript...) and called
for every cell. Its arguments are the node's inputs in the order of `args` (else by name), each an `f64`
(maps give their cell's value, numbers the same value everywhere). It returns the cell's value. Missing cells
arrive as NaN.

```toml
[[nodes]]
id = "keep"
op = "plugin.wasm"
inputs = { land = "$t0", slope = "slope.map" }
params = { path = "plugins/keep.wasm", function = "cell", args = ["land", "slope"] }
```

In Rust:

```rust
// cargo build --release --target wasm32-unknown-unknown (crate-type = ["cdylib"])
#[unsafe(no_mangle)]
pub extern "C" fn cell(land: f64, slope: f64) -> f64 {
    if slope < 10.0 { land } else { 0.0 }
}
```

Or directly in the WebAssembly text format (`.wat`):

```wat
(module (func (export "cell") (param $land f64) (param $slope f64) (result f64)
  (if (result f64) (f64.lt (local.get $slope) (f64.const 10))
    (then (local.get $land)) (else (f64.const 0)))))
```

Plugins run in a sandbox:

- they import nothing, so they have no access to files or the network;
- every call has a budget of instructions, so a function that loops forever stops with an error.

This makes them safe to send to other machines: in a batch (`terraflux submit`, `calibrate`), the module
goes to the workers by hash, like the model's maps.
