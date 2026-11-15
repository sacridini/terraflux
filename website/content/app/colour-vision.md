---
title: Colour vision and accessibility
short: Colour vision
description: Check maps and charts as readers with colour vision deficiency see them, the warning for classes that look alike, colour-blind safe colours, and the text size, zoom, themes and keyboard use that make the application easier to read.
order: 60
---

About 1 man in 12 and 1 woman in 200 have a colour vision deficiency; red and green are the colours most often confused.
A map that reads well to you can be a puzzle to a reviewer or a student. Terraflux helps in three ways: it
**shows your maps and charts as such a reader sees them**, it **warns you when two classes look alike**, and it
offers **colours chosen to be told apart**. This page explains them, then the settings that make the window itself easier
to read, and the keyboard.

## View > Colour vision

**View > Colour vision** is a menu with four choices. It draws everything as that reader sees it, until you set it back:

| Choice | What it simulates |
|---|---|
| *normal colour vision* | nothing changes (the default) |
| *protanopia (red-blind)* | no red cones (about 1% of men) |
| *deuteranopia (green-blind)* | no green cones (about 1% of men; the milder, more common anomalous green vision falls between this and normal) |
| *tritanopia (blue-blind)* | no blue cones (rare) |

The simulation is the one of Machado, Oliveira and Fernandes (2009), at full severity. It simulates the complete loss of one
kind of cone. The milder forms are not simulated.

While a vision other than normal is on, a button at the top of the window says so, for example *seen with
deuteranopia (green-blind) · back to normal*. A click on it sets normal vision back. The choice is not kept: the
next time Terraflux starts, it is normal again.

### What it applies to

- **Maps**: every map and its layers, the vector layers, and the swatches and colour bars of the legends.
- **Charts**: the table viewer's charts, the area of the classes along the steps, series, calibration, profiles,
  weights of evidence, sensitivity charts, and the bars of gains and losses.
- **Pictures made from the Viewer**: the maps of a figure, of **Export the view...** and the frames of a GIF or an MP4 are
  drawn with the vision chosen when you save them.

It does not change the window: the buttons, menus and themes keep their colours. It does not change the QGIS export, which writes the colours as they are.

!!! tip "Look at it before you submit"
    Choose each of the three visions in turn and look at the map you will publish. Can you still tell every class
    apart, and read the legend? Then put **normal colour vision** back before you save the figure for print: the maps
    are drawn in the vision chosen at that moment. See [Figures for publication](/app/figures/).

## Classes that look alike

A legend of classes checks its colours. When two classes are too close, it writes a warning under the legend, for
example:

> forest and pasture look alike
>
> forest and pasture look alike for deuteranopia

The first says the two colours look alike to **normal vision**. The second says they look alike **only to the vision
you chose** in **View > Colour vision**: with normal vision on, only the first kind is checked. Each pair is named once. At most three pairs are
named, then *and N more pairs*.

"Look alike" has a number. It is the colour difference **CIEDE2000** (Sharma et al., 2005) between the two
colours, worked out in the Lab colour space that models how people see differences. Below **10**, the warning appears. For
comparison, a difference of 2 to 3 is what most readers can tell apart when two swatches touch. A legend's swatches
are small and apart, and the patches of a map can be small, so the limit is looser: 10 is what a reader still mixes up in those
conditions.

The warning has a button, **Colour-blind safe colours**. A click puts Okabe and Ito's colours (below) on the classes of the map. The warning
is worked out again only when the colours or the vision change, so the legend stays fast.

## Colour-blind safe classes

Okabe and Ito (2008) published a set of eight colours chosen to be told apart by readers with every common colour
vision. Terraflux uses them in two places:

- The first seven colours of the default palette for classes are Okabe and Ito's.
- **Colours** (the palette menu of a map in the Viewer: **Colours ⏷** or **🎨 Colours**) has an entry
  **colour-blind safe classes**, for a map of classes. It gives the classes Okabe and Ito's eight colours (blue, orange, bluish green, reddish purple, sky blue, vermilion, yellow, black), in the order of the classes.
  With more than eight classes the same colours come back lightened, more so the later they are. No set of colours can
  keep many classes apart for every reader: if you have more than eight, think about grouping classes, or labelling the map.

A **Plot** keeps this choice with the model: its layer's palette is written as `colour-blind safe`, and the Plot opens, is
exported and enters the report with those colours.

## Palettes for values

For a map of values, the palette menu **Colours** lists eight ramps. Two are made for readers with colour
vision deficiency and say so:

| Ramp | Use it for |
|---|---|
| **cividis (for colour-blind readers)** | values from low to high. Blue to yellow, and it reads the same to readers with any common colour vision (Nuñez et al., 2018) |
| **orange to purple, centred on 0 (for colour-blind readers)** | values on both sides of zero: differences, changes, correlations |

The others are *viridis* (the default for values), *blue to red, centred on 0*, *terrain*, *grey*, *red to green* (NDVI and
similar) and *rank*. Avoid *red to green* when your readers may be colour-blind, and prefer the ramp that is centred
on 0 and made for them when a map shows a difference. Each ramp can be put on a scale of logarithms.

## Text size, zoom and themes

These settings are in the settings window (**⚙**, section **Appearance**; see [Settings](/app/settings/)):

- **Text size**: from 80% to 160% of the usual size, in steps of 5%. It makes the text of the whole window larger or smaller.
- **Interface zoom**: from 75% to 200%. It scales the whole window, text and controls together. Use it on a laptop with a
  small screen or with a projector.
- **Theme**: 12 themes. The light ones are **Paper**, **Ash** (modern), **Immaterial** and **Field Notes** (classic); the
  others are dark. Light themes are easier for some readers in a bright room, and are the kind to choose for a projector. The theme
  does not change the colours of the maps.

The changes apply to the window at once; the settings are kept for the next start.

## The keyboard

Many actions of Terraflux are reachable by name or by key, without the mouse:

- **Ctrl+K** (or Ctrl+Shift+P) opens **Commands**: type part of the name of anything (a pane, a template, an
  action such as *History*, a node) and press **Enter**. On macOS, **Cmd** takes the place of **Ctrl**.
- **F1** opens the sheet of **Keyboard shortcuts** (also in **View**).
- **/** opens the search of operators and adds one to the model; **Tab** moves between the Model pane and the place you were; the **arrow keys** over the canvas go
  to the next node, and **Enter** shows its output. **F5** runs the model; **Ctrl+Enter** runs to the node selected.
- In **Settings**, section **Keyboard**, you can bind your own keys to the actions.

See [Shortcuts](/app/shortcuts/) for the table of keys.

## What is not covered

The tools above are about colour, size and keys. A map is a picture and has no text description, so these tools do not
make a map readable to a screen reader. For a figure you publish, give its meaning in the caption and the text of your paper.

## References

- Machado, G. M., Oliveira, M. M., & Fernandes, L. A. F. (2009). A physiologically-based model for simulation of color vision deficiency. *IEEE Transactions on Visualization and Computer Graphics*, 15(6), 1291–1298. https://doi.org/10.1109/TVCG.2009.113
- Nuñez, J. R., Anderton, C. R., & Renslow, R. S. (2018). Optimizing colormaps with consideration for color vision deficiency to enable accurate interpretation of scientific data. *PLOS ONE*, 13(7), e0199239. https://doi.org/10.1371/journal.pone.0199239
- Okabe, M., & Ito, K. (2008). Color Universal Design (CUD): How to make figures and presentations that are friendly to colorblind people. https://jfly.uni-koeln.de/color/
- Sharma, G., Wu, W., & Dalal, E. N. (2005). The CIEDE2000 color-difference formula: Implementation notes, supplementary test data, and mathematical observations. *Color Research & Application*, 30(1), 21–30. https://doi.org/10.1002/col.20070
