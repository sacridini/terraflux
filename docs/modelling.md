# Changing a model's logic

A model is a graph of operators (`terraflux ops` lists them). Besides the presets' operators, four tools
let you change what a model does without changing Terraflux:

- the node editor;
- map algebra;
- groups and submodels;
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
dynamic variable.

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
