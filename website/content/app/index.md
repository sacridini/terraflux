---
title: The application
short: Overview
description: "A reference manual of Terraflux's window: its panes and menus, the canvas, the viewer, the settings and the keyboard, for looking things up."
order: 0
---

The **application** is Terraflux's window. In it you build a model as a chain of boxes, run it, look at what each
step gives, and save the maps, the figures and the model itself. This section is its **reference manual**: it says
what each part of the window is called, where it is, and what it does. Use it to look something up.

It does not teach. If you have never opened Terraflux, read [A tour of Terraflux](/tutorials/getting-started/tour/)
first: it walks you through the window, a template and a run in twenty minutes. The pages here are the long list
that the tour leaves out. Every name in **bold** is written as the window shows it, so you can find it on screen.

## The pages of this section

| Page | What it holds |
|---|---|
| [The window and its panes](/app/window/) | Every pane and what **View** turns on and off, the menus item by item, the tabs of models, the Changes pane, the History, recovery, files not found, notes and frames, run progress and notifications. |
| [Building on the canvas](/app/canvas/) | Nodes, wires, settings on the box and setting wires, groups and containers, Find, align, insert on a wire, the library and the data catalog, Trace, the checks shown on nodes. |
| [The viewer](/app/viewer/) | Layers, basemaps, Compare, the histogram, RGB, tables and selection, editing vertices, remote data, Go to, the graticule, Plots. |
| [Figures for publication](/app/figures/) | The **Figure...** window: a map as a figure for a paper, in PNG, SVG or PDF. |
| [Export to QGIS](/app/qgis/) | The run's maps and vectors, in their colours, as a QGIS project. |
| [Colour vision and accessibility](/app/colour-vision/) | **View > Colour vision**, colour-blind safe classes, and the warnings on legends. |
| [File formats](/app/formats/) | The files Terraflux reads and writes. |
| [Settings](/app/settings/) | Every section and option of the Settings window, where the settings file is, and what is kept in it. |
| [Keyboard and mouse](/app/shortcuts/) | Every shortcut, the mouse gestures, how to change a shortcut, and the F1 sheet. |

## The application, the command line and Python

Terraflux is one program with three ways in. They read and write the same model file, so you can start in one
and finish in another.

| Use | When |
|---|---|
| The **application** | You build, check and look at a model. You want to see maps, change a setting and see the result at once, make a figure, or explain a result cell by cell. |
| The [command line](/cli/) | The model is right and you want to run it many times, on a server or on a cluster, without a window: `terraflux run model.toml`. |
| The [Python library](/python/) | You work in a script or a notebook, with arrays and data frames, and want the same methods as functions. |

A model made in the application can also be written out as a script of the Python library (**File > Python
script...**), for a paper's code availability or a reviewer. [When to use the application, and when the command line](/tutorials/getting-started/tour/#when-to-use-the-application-and-when-the-command-line)
in the tour tells more.

## Conventions in this section

- A label in **bold** is a button, menu item or title as the window shows it: **View > Changes** is the menu
  **View**, then its item **Changes**.
- Keys are written for Windows and Linux: ++ctrl++ is the ++cmd++ key on macOS.
- The window's look changes with its theme. A button written **Use my data** in the default themes is written
  *Use my data...* in the classic ones; this section uses the first form.
- A value the window starts with is called its *default*.
