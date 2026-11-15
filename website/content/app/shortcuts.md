---
title: Keyboard and mouse
short: Keyboard and mouse
description: "Every keyboard shortcut of Terraflux grouped by where it applies, the mouse gestures, how to change a shortcut in Settings, and the F1 sheet."
order: 80
---

This page lists every shortcut of the application and the mouse gestures of the canvas and the maps. The same list is
in the application: press ++f1++ ([The F1 sheet](#the-f1-sheet)).

- Keys are written for Windows and Linux. On macOS, ++ctrl++ is the ++cmd++ key.
- Keys go to the pane under the pointer, and **not while you are typing in a box**. If a key does nothing, click on
  the canvas or the map first.
- A shortcut that works over a place (the canvas, a map) wins there over an *Everywhere* shortcut that is the same.
- A line marked *(fixed)* is part of editing a text or of how a widget works; it cannot be changed.

## Everywhere

| Keys | Does |
|---|---|
| ++f5++ | Run the whole model |
| ++ctrl+s++ | Save the model |
| ++ctrl+shift+s++ | Save the model as... |
| ++ctrl+alt+s++ | Save a version of the model... |
| ++ctrl+n++ | New model |
| ++ctrl+shift+o++ | Open a model... |
| ++ctrl+o++ | Open a raster or vectors... |
| ++ctrl+k++, ++ctrl+shift+p++ | The commands: anything, by its name |
| ++f1++ | Keyboard shortcuts |
| ++tab++ | To the model and to the last map seen (not while typing: ++tab++ goes to the next field then) |
| ++ctrl+w++ | Close the pane in front |
| ++ctrl+tab++ | Next model open (its tab) |
| ++ctrl+shift+tab++ | Model open before (its tab) |
| ++ctrl+shift+w++ | Close the model in front (its tab) |
| ++ctrl+z++ | Undo *(fixed)* |
| ++ctrl+shift+z++ | Redo *(fixed)*. ++ctrl+y++ does the same. |
| ++slash++ | Add an operator, by its name *(fixed)* |

## The model's canvas

The pointer over the canvas, or the canvas in focus.

| Keys | Does |
|---|---|
| ++home++ | The whole graph in view |
| ++f++ | The nodes selected in view (none selected: the whole graph) |
| ++left++, ++right++, ++up++, ++down++ | Go to the node beside *(fixed)* |
| ++enter++ | Show what the node selected gives *(fixed)* |
| ++ctrl+enter++ | Run up to the node selected |
| ++shift+enter++ | Open the settings of the node selected beside its box |
| ++delete++, ++backspace++ | Delete the nodes selected |
| ++ctrl+d++ | Duplicate the nodes selected |
| ++ctrl+c++ | Copy the nodes selected *(fixed)* |
| ++ctrl+v++ | Paste them *(fixed)* |
| ++ctrl+f++ | Find a node by id, operator, note or file (inside groups too) |
| ++ctrl+r++ | Rename the node selected |
| ++ctrl+l++ | Arrange the nodes |
| ++alt+shift+left++ | Align the nodes selected: left edges |
| ++alt+shift+right++ | Align the nodes selected: right edges |
| ++alt+shift+up++ | Align the nodes selected: top edges |
| ++alt+shift+down++ | Align the nodes selected: bottom edges |
| ++alt+shift+c++ | Align the nodes selected: horizontal centres |
| ++alt+shift+m++ | Align the nodes selected: vertical centres |
| ++alt+shift+h++ | Distribute the nodes selected horizontally (three or more) |
| ++alt+shift+v++ | Distribute the nodes selected vertically (three or more) |
| ++alt+shift+k++ | Put the nodes selected in a column, one under the other |
| ++alt+shift+r++ | Put the nodes selected in a row, side by side |
| ++ctrl+g++ | Group the nodes selected |
| ++ctrl+shift+g++ | Ungroup the group selected |
| ++b++ | Turn the nodes selected off or on again (bypass: their inputs passed on) |
| ++ctrl+a++ | Select all the nodes |
| ++esc++ | Select none *(fixed)* |
| ++ctrl+1++ to ++ctrl+9++ | Keep where the canvas looks in a slot, as a place saved with the model *(fixed)* |
| ++1++ to ++9++ | Go to the place in that slot (the Viewer has its own) *(fixed)* |

## Maps

The pointer over a map in the Viewer.

| Keys | Does |
|---|---|
| ++b++ | The satellite under the map |
| ++space++ | Hide the map, or show it again |
| ++m++ | Measure: click points for lengths, area and perimeter (++esc++ ends) |
| ++ctrl+g++ | Go to a coordinate (degrees or the map's units) and mark it (++esc++ lets the mark go) |
| ++alt+left++ | The view before (the mouse's back button too) |
| ++alt+right++ | The view after (the mouse's forward button too) |
| ++left++ | The step before (Simulation) |
| ++right++ | The step after (Simulation) |
| ++enter++ | Finish the shape being drawn *(fixed)* |
| ++backspace++ | Take the last point away (drawing, measuring) *(fixed)* |
| ++esc++ | Leave the shape, the line, the selection or the mark *(fixed)* |
| ++ctrl+1++ to ++ctrl+9++ | Keep the view in a slot, as a place saved with the model (Places) *(fixed)* |
| ++1++ to ++9++ | Go to the place in that slot *(fixed)* |

## The calculator

In the script and the console of the [Calculator](/tutorials/building-models/calculator/).

| Keys | Does |
|---|---|
| ++ctrl+enter++ | Give the script to the node and run up to it |
| ++ctrl+space++ | The names that fit (completions) |
| ++down++ | The next completion (the console: the line typed after) *(fixed)* |
| ++up++ | The completion before (the console: the line typed before) *(fixed)* |
| ++tab++, ++enter++ | Put the completion chosen in (the console: compute the line) *(fixed)* |
| ++esc++ | Close the completions *(fixed)* |

## Files

In the **Files** pane.

| Keys | Does |
|---|---|
| ++alt+left++ | Back to the folder before |
| ++alt+right++ | Forward to the folder after |
| ++alt+up++ | Up to the folder that holds this one |
| ++ctrl+l++ | Type a path |
| ++up++, ++down++ | The row above, below (++shift++: one more chosen) *(fixed)* |
| ++right++, ++left++ | Open, close the folder chosen *(fixed)* |
| ++enter++ | Open the entries chosen (a path typed: go there) *(fixed)* |

## The assistant

In the **Assistant** pane.

| Keys | Does |
|---|---|
| ++enter++ | Send the question *(fixed)* |
| ++esc++ | Stop the answer being written *(fixed)* |
| ++up++, ++down++ | The questions asked before, and after *(fixed)* |

## Attribute tables

| Keys | Does |
|---|---|
| ++ctrl+a++ | Select the rows shown |
| ++esc++ | Let the selection go *(fixed)* |

## Lists and boxes

The palette (++ctrl+k++), the searches (++slash++, ++ctrl+f++) and the notes.

| Keys | Does |
|---|---|
| ++down++ | Choose the next in a list *(fixed)* |
| ++up++ | Choose the one before in a list *(fixed)* |
| ++enter++ | Do the one chosen, confirm a box *(fixed)* |
| ++esc++ | Close a list or a box without doing it *(fixed)* |
| ++ctrl+enter++ | Save a note *(fixed)* |

## Mouse

### On the canvas

| Gesture | Does |
|---|---|
| Click | Select a node (++shift++ or ++ctrl++: one more). |
| Drag on a node | Move the nodes selected. |
| Drag on the empty canvas | Select the nodes in a box. |
| Right or middle drag | Move the view. |
| Wheel | Zoom. |
| Double click on a node | Show its output in the Viewer; a container: show its body. |

Also, and not on the F1 sheet:

| Gesture | Does |
|---|---|
| Drag from an output port to an input port | Connect them ([Wires](/app/canvas/#wires)). |
| Drag from a free input or a setting's socket to an output | Connect them the other way. |
| Drag a wire off a connected input | Carry it to another input. |
| ++ctrl++ + drag on a node | Drag a copy; the original stays. |
| ++alt++ + drag on a node | Lift it out of its wires; they are joined again. |
| Drag a node over a wire | Insert it in the wire. |
| Drag the right edge of a node | Make it wider. A double click brings back the width its names need. |
| Double click on a wire, or on a reroute point | Add a point there, or take it away. |
| Right click on a node, a wire's point, a note or the empty canvas | The menu of each ([Building on the canvas](/app/canvas/)). |
| Right click on a connected input | Take its wire away. |
| Double click on a note or a frame's title | Edit its text. |

### On a map

| Gesture | Does |
|---|---|
| Wheel, drag | Zoom, move. |
| Double click | The whole map in view. |
| Click | Pick a cell; click a feature to select its row. |
| Right click on a cell | **Trace this cell**, **Ask the assistant why**, **Explain with an MCP assistant**. |
| ++shift++ + drag | Select the features a rectangle meets. With ++alt++ too, a lasso. ++ctrl++ adds to the selection. |
| Mouse back and forward buttons | The view before, the view after. |

## Changing a shortcut

In **Settings**, choose **Keyboard**. It lists every action by where it applies, with its shortcuts.

1. Type in the box *filter the actions* to find one (by name or by key).
2. Click its shortcut. The button says *press the keys...*.
3. Press the new keys. A shortcut is a key with ++ctrl++, ++shift++ and ++alt++ if you like.
4. ++esc++ cancels, and ++backspace++ takes the shortcut away.
5. An action can have two shortcuts: the **+** button beside the first adds a second. An action with none shows *none*.
6. **Reset**, beside an action you changed, puts its shortcut back. **Reset all**, at the top, puts them all back.

When two actions share a shortcut, the window says so in red (the two are in the same place, so one of them will
not work) or in amber (one is an *Everywhere* action and the other works over a place: the place wins there).
Lines marked *(fixed)* above are shown in grey and cannot be changed. Menus, the palette and the F1 sheet all show
the shortcut you set. Your changes are in `settings.json` ([Settings](/app/settings/#where-the-settings-are-kept)),
under `shortcuts`; **Back to the defaults** keeps them.

## The F1 sheet

++f1++, or **View > Keyboard shortcuts**, opens the window *Keyboard shortcuts*: the tables of this page, by where
they work, with the shortcuts as you set them. For the canvas and the maps it starts with the mouse gestures
(*click*, *drag*, *right or middle drag*, *wheel*, *double click*). The line at the bottom of the window also
reminds you of the keys of the pane under the pointer: on the canvas, for example, *click: select*, *drag: a box of
nodes*, *double click: its map*, */: add an operator*.

The **commands** palette (++ctrl+k++) finds an action by its name when you do not remember its key.
