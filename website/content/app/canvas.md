---
title: Building on the canvas
short: The canvas
description: "Nodes, wires, settings on the box and setting wires, groups and containers, Find, align, insert on a wire, the library and the data catalog, Trace and the checks shown on nodes."
order: 20
---

The **Model** pane is the canvas: the nodes of a model and the wires between them. This page lists what you can do
on it. The tutorials teach the same things with a model to build; this page is for looking up a name or a gesture.
To learn to build, start with [Your first model](/tutorials/getting-started/first-model/) and [Building
models](/tutorials/building-models/).

![The Model pane with a template](/img/tutorials/getting-started/tour/model-pane.png "The Model pane: the nodes, each with its name, its operator in small letters and its gear, its inputs (left) and outputs (right), and its main settings in rows. The green dot at the top right of a node says that it ran; mspa is selected (blue outline).")

## Nodes

A **node** is a box that runs one operator. It shows:

- its **name** (you can change it: ++ctrl+r++) and, under it, its operator in small letters, such as
  `landscape.mspa`, with a **⚙** at the right;
- its **inputs** on the left edge and its **outputs** on the right edge. An input marked orange must be connected
  before the model can run;
- its main **settings**, a row each ([Settings on the box](#settings-on-the-box));
- at the top right, from right to left: a **dot** for how it did in the last run (green ran, dark green from the
  cache, orange running, red failed), a round **!** mark if it has a problem (red an error, amber a warning), and a
  badge if it is a check ([Checks shown on nodes](#checks-shown-on-nodes));
- under its right corner, how long it took (*2.4 s*, or *cache*);
- `×12` at the right when it ran once for each of 12 layers.

Other marks: a red dot at the left of the title is a **breakpoint**; a ❄ at the top left corner says the result is
**frozen** (kept on disk next to the model and used instead of running the node); a node **bypassed** is turned off
and passes its inputs on. See [Checks, tests and reproducible runs](/tutorials/building-models/checks-and-tests/#freeze-a-result)
for freezing.

### Add a node

Three ways, all of which connect the new node to the node selected:

- click an operator in the **Operators** drawer, or in the **Operators** menu;
- press ++slash++ anywhere on the canvas, type part of an operator's name or of what it does, and press ++enter++;
- open a file with **File > Open a raster or vectors...** (++ctrl+o++), or drag it from the **Files** drawer onto
  the canvas: a node that reads it appears there.

A wire dragged from an output and let go on the empty canvas opens the same search, listing the operators that take
what the wire carries; the one you choose is born there, connected. A free input's wire let go on the empty canvas
does the same for the operators that *give* what the input takes.

### Select, move, copy, delete

| To | Do |
|---|---|
| Select a node | Click it. ++shift++ or ++ctrl++ click adds another. |
| Select several | Drag a box on the empty canvas, or press ++ctrl+a++ for all. ++esc++ lets them go. |
| Move | Drag a node. All nodes selected move together. |
| Make a node wider | Drag its right edge. A double click on the edge brings back the width its names need. |
| Copy and paste | ++ctrl+c++, ++ctrl+v++. |
| Duplicate | ++ctrl+d++. The copy is a step down and to the right, reading what the original read. |
| Copy by dragging | Hold ++ctrl++ while you drag: the originals stay. |
| Lift a node out of a chain | Hold ++alt++ while you drag it: the wires it was in are joined again. |
| Delete | ++delete++ or ++backspace++. The wires into and out of the node go too. |
| Turn off | ++b++: **Bypass**, the node is skipped and passes its inputs on. ++b++ again turns it on. |

A right click on a node opens its menu. The items, in order:

| Item | Does |
|---|---|
| **Use a value of its own**, **Show on the node**, **Hide from the node** | Only when you right-click a row of settings: take the wire off the setting, or show or hide that row. |
| **Settings...** | Opens the node's settings in a panel beside its box (also its **⚙**, or ++shift+enter++). |
| **Show on the node** (submenu) | Lists the settings folded away, to show each on the box. |
| **Connect the node selected here** | Connects the nodes selected into this node's inputs. |
| **Group the *n* nodes selected** (++ctrl+g++) | Puts them in a group. |
| **Open the group**, **Ungroup** (++ctrl+shift+g++), **Give it the outputs of its last nodes** | For a group. |
| **Save as my node...** | Saves a group or a calculator as a node of your library, to use in any model as one operator. |
| **Duplicate** (++ctrl+d++) | A copy. |
| **Run on a folder...** | Runs the node on every raster of a folder. |
| **Change file...** | For a node that reads a file: another file, keeping its wires and settings. |
| **Make it an input of the model** | For a `raster.read` node at the top level: the file becomes an input (`$name`) that the command line, **Many runs** and the scenarios can change. |
| **Take it out of the frame** | If the node is in a frame. |
| **Disconnect all inputs** | Takes off every wire into the node. The values you typed stay. |

With two or more nodes selected, a right click on one of them (or on the empty canvas) opens the selection's menu:
**Delete**, **Duplicate**, **Rename...** (all their names from a pattern, with `{n}` the number, `{id}`, `{op}` and
`{file}`), **Disconnect all their inputs**, **Group**, **Frame them**, **Align** (below), **Run to them**,
**Freeze** or **Unfreeze**, **Bypass** or **Turn them on**, and **Connect the others selected to** the node you
clicked.

## Wires

A **wire** carries a map, a table or a number from an output to an input. A dragged wire only connects ports that
fit: a map output does not go into a number input.

- **Connect.** Drag from an output port to an input port. Drag from a free input to an output that fits it. Let a
  wire go on a node, not on a port: if one free input fits it connects there, if several fit a list asks which.
  A node that takes any number of inputs (such as `stack.of`) gets a new input.
- **Swap.** Carry a wire off a connected input and let it go on another connected input that both fit: the two
  wires are exchanged.
- **Out of a group.** Inside a group, let a wire go on the edge of the group's scope: it becomes an output of the group.
- **Several at once.** Select several nodes, then drag the output of one of them onto a node: all selected outputs
  connect to it, top to bottom, in the node's free inputs (new inputs, if it takes any number, such as `stack.of`).
- **Move.** Drag a wire off a connected input and let go on another input: it moves there.
- **Cut.** Right-click a connected input, or drop a wire that you carried on the empty canvas.
- **What it carries.** Hover over a wire: its tooltip says the type and, after a run, what the output gave (a map's
  size, cells, coordinate system and range of values; a number; a table's size).
- **Around the boxes.** With **wires go around the boxes** on (the default), a wire that would cross another node is
  bent above or below it.

### Reroute points

A **reroute point** bends a wire where you want it, so it does not cross boxes or runs through a tidy path.

- Double-click on a wire to add a point there. Drag a point to move it.
- Double-click a point, or right-click it and choose **Remove point**, to take it away.
- A wire with points of your own is left as you drew it; **wires go around the boxes** does not move it.

### Insert on a wire

Drag a node over a wire and let go: the node is put in the middle of the wire. The wire's source goes into the node's
free input that fits best, and the node's first free output that fits goes into what the wire fed. If the node does
not fit, or it would make a cycle, nothing happens.

## Settings on the box

With **a node's main settings on its box** on (the default), a box lists the settings it needs, the ones you gave
and its essential ones, a row each, name at the left and value at the right. The other settings are counted in a
row, *+ 2 settings* (*· 1 given* if some were given), that unfolds them; *- fewer* folds them again. Optional
inputs fold in a row *+ n optional*. At most four rows show folded.

To change a setting on the box, click its value:

- a yes or no: a click turns it over;
- a few possible values (`connectivity`): a list opens under the row;
- a number or a text: a field opens over the value; ++enter++ (or a click elsewhere) keeps it;
- anything else (a script, a table of rules, a file, a list): the node's settings panel opens beside the box.

**Show on the node** and **Hide from the node** in a row's menu keep a setting always on the box or off it. The
panel (**Settings...**) lists every setting with what it means; the same content is in the **Node** pane.

![The settings panel of mspa](/img/tutorials/getting-started/tour/node-pane.png "The settings panel opened beside the box of mspa: its operator and what it does, its input map connected to forest.map, and its settings. A star marks an input that must be connected. Screenshot of Terraflux 1.51 (the theme Slate).")

### Setting wires

A setting can **read** a value from another node instead of holding one. A row of settings that can read a number,
a yes or no or a text has a small socket. Drag from the socket to an output that gives such a value, or drag an
output onto the row: the setting now reads it, and its value is drawn in the accent colour. A model's input
(`$name`) or a loop's carried value (`@name`) can be read the same way. The wire is drawn from the output to the
row, and hovering over it says which setting it feeds.

- To give the setting back its own value, right-click its row and choose **Use a value of its own**, or drag its
  wire off the socket.
- Setting wires take reroute points like other wires.
- The model file writes the setting as a reference: `"$name"`, `"@name"` or `{ from = "node.port" }`.

## Groups and containers

A **group** puts several nodes in one: it is one node on the canvas, and what read the nodes from outside reads its
outputs. A **container** is a node that holds a model of its own: a loop (`control.repeat`), a loop over a collection
(`control.each`), a switch (`control.switch`) and a group (`control.group`).

- **Group the nodes selected** (++ctrl+g++) makes a group and asks for its name. **Ungroup** (++ctrl+shift+g++) puts
  the nodes back, connected as they were.
- A double click on a container, or **Open the group**, shows its body; the bar says where you are (**Whole model**
  › the container's name) and **‹ Back** (or ++esc++) goes up a level.
- **Give it the outputs of its last nodes** makes an output of the group for each node that no other node of the
  group reads.
- **Save as my node...** keeps a group or a calculator as a node of your library (**Settings > Node libraries**
  holds shared libraries).
- The command **Group the model by stage (data, preparation, model...)** in ++ctrl+k++ groups the whole model by
  stage.

[Groups and submodels](/tutorials/building-models/groups-and-submodels/) and [Loops](/tutorials/building-models/loops/)
teach them with models.

## Find a node

++ctrl+f++ over the canvas opens a search box: *find a node: its id, operator, note or file*. It looks at every
depth (inside groups, loops and switches). Up to twelve matches are listed; choose with the arrows and press
++enter++: the canvas goes to the node, opening the containers on the way. The palette (++ctrl+k++) reaches nodes
too, as *Go to group/inner*.

Other moves on the canvas: ++home++ fits the whole graph, ++f++ the nodes selected (or the whole graph if none),
the arrows go to the node beside the one selected, and ++enter++ shows its output in the Viewer. The wheel zooms,
and a right or middle drag moves the view. A **minimap** in the corner of a large model (**a minimap of a large
model on the canvas**) shows where you are: click or drag on it. The digits ++1++ to ++9++ go to a place kept with
++ctrl+1++ to ++ctrl+9++; **Places** in the context menu of the empty canvas names them.

## Align and arrange

**Arrange** (++ctrl+l++) puts all the nodes in tidy columns, by what they read. To tidy only some, select them and use
the **Align** submenu of the selection's menu:

| Item | Does | Key |
|---|---|---|
| **Left edges**, **Right edges** | Lines the boxes up on the leftmost or the rightmost. | ++alt+shift+left++, ++alt+shift+right++ |
| **Top edges**, **Bottom edges** | Lines them up on the highest or the lowest. | ++alt+shift+up++, ++alt+shift+down++ |
| **Horizontal centres**, **Vertical centres** | Lines up their centres. | ++alt+shift+c++, ++alt+shift+m++ |
| **Distribute horizontally**, **Distribute vertically** | The same gap between the boxes; the outermost two stay. Needs three or more. | ++alt+shift+h++, ++alt+shift+v++ |
| **In a column**, **In a row** | One under the other, or side by side, in the order they have now. | ++alt+shift+k++, ++alt+shift+r++ |

## The library and the data catalog

### The library

The **Operators** drawer (move the pointer to the window's left edge), the **Operators** menu and the ++slash++
search hold the same library of about 300 operators, ordered by subject. The [tour](/tutorials/getting-started/tour/#the-operators-library)
lists the subjects and their prefixes, and the [operator reference](/operators/) every setting of every operator.

- **Favourites.** Right-click an operator to put it in a coloured collection: red, orange, yellow, green, blue,
  purple or grey. Your favourites are listed at the top of the drawer and in the **Favourites** submenu of the
  **Operators** menu.
- **Your own nodes.** A group or a calculator saved with **Save as my node...** appears in the library as one
  operator. Shared folders and remote libraries of nodes are added in **Settings > Node libraries**
  ([Settings](/app/settings/#node-libraries)).

### The data catalog

**File > Data catalog...** (or *Data catalog* in ++ctrl+k++) lists public datasets that Terraflux downloads onto your
model's grid: the climate (WorldClim, CHELSA), the Copernicus DEM, SoilGrids, MapBiomas Brazil, Hydrography90m, ESA
WorldCover, Hansen's Global Forest Change, MODIS and GBIF occurrences. A card says what a dataset holds, its
resolution, years, coverage, **licence** and the paper to cite. Choose what to fetch and the grid, then **Add to the
model** adds a [[data.download]] node set that way. **Preview** shows the first layer, coarse, before anything is
added. The [tour](/tutorials/getting-started/tour/#the-data-catalog) describes each choice.

## Trace: why a cell has its value

**Trace** answers: why does *this* cell have *this* value? Select a node, click a cell in the Viewer and click
**Trace this cell** in the bar above the map (or right-click the cell). The **Trace** pane opens with the node and
the cell, a sentence that sums up the answer and a card for each node on the way back to the data, each with the node's
value at that cell and how the node made it. A card's name selects its node and shows its map with the same cell
outlined. **Trace inside the loops** runs a loop again keeping what it gives at that cell. **Copy** copies the trace
as text; **Ask the assistant why** gives it to the assistant. The [tour](/tutorials/getting-started/tour/#why-a-cell-has-its-value-trace)
explains each kind of card.

## Checks shown on nodes

A [[check.assert]] node tests the model while it runs. After a run, a badge at the top right of its box says how it
fared:

| Badge | Colour | Meaning |
|---|---|---|
| ✔ | green | passed |
| ! | amber | failed, warning only |
| ✖ | red | failed, the run stopped |
| · | grey | not run yet, or turned off (bypass) |

A group or a loop that holds checks shows the worst of them on its own box. Hover over the title for the check's
words. The model's health is a line in the status bar (*3 checks: 2 passed, 1 warned*) in the colour of its worst
check, and the **Document** shows each check's verdict on its step. [Checks, tests and reproducible runs](/tutorials/building-models/checks-and-tests/#the-checks-in-the-application)
teaches them.

## See also

- [A tour of Terraflux](/tutorials/getting-started/tour/): the canvas in use.
- [The window and its panes](/app/window/): the Model pane's bar and the other panes.
- [Keyboard and mouse](/app/shortcuts/#the-models-canvas): every key of the canvas.
