---
title: Figures for publication
short: Figures
description: Make a figure of a map for a paper (layouts, font, legend, scale bar, placement, size and resolution, PNG, SVG and PDF), export the view as a picture, and save an animation.
order: 40
---

This page is the reference for everything that turns a map in the Viewer into a file: the **Figure...** window,
**Export the view...**, and the animations (GIF or MP4). For a gentle first walk through the Viewer see
[A tour of Terraflux](/tutorials/getting-started/tour/); for the Plots that keep a view with the model see
[Plots and figures](/tutorials/building-models/plots-and-tables/).

Which one to use:

| You want | Use | Writes |
|---|---|---|
| A figure for a paper, with legend, scale bar and coordinates, at a journal's width | **Figure...** | PNG, SVG, PDF |
| The map as the Viewer shows it, to open in QGIS or put on a slide | **Export the view...** | PNG, GeoTIFF |
| A film of the steps of a simulation or the layers of a stack | **Save as GIF...**, **Save as MP4...** | GIF, MP4 |
| The same maps in a QGIS project | [Export to QGIS](/app/qgis/) | a project and its layers |

## Figure...

**Figure...** is a button at the top of the panel beside the map in the Viewer. It opens a window called
**Figure**: its settings are in a column at the left and the figure's **preview** is at the right. The preview is
drawn again every time you change a setting. Nothing is written until you click one of the **Save** buttons.

![The figure is drawn from the map shown](/img/tutorials/building-models/plots-and-tables/land.png "A map as the Viewer draws it, before the figure adds a legend, a scale bar and the coordinates. The Figure window starts from what the Viewer shows: its colours, the layers drawn with it and the vector layers over it.")

### What the figure shows

The figure is made of the map shown in the Viewer: its colours, the maps drawn with it (under or over it, at
their opacity) and the vector layers drawn over it, in their colours and cut at the map's edge, with their labels.
If you change the colours or the layers in the Viewer, close the window and open it again.

When the Viewer has more than one map to offer, the column has a list **Maps** with one choice for each layout:

| Choice | The figure |
|---|---|
| *this map* | the map shown, alone |
| *this map and the one compared* | the map and the one it is compared with, beside each other, in the same colours |
| *the N views side by side* | the maps of a comparison **Side by side** (more than two views) |
| *the difference of the two* | the difference map of a comparison made as a **Difference** (a ramp centred on 0, or the transitions between classes) |
| *every layer (N)* | every layer of a stack, a series of steps or the bands of a raster, as panels (at most 12 layers) |

With more than one map the panels are lettered **(a)**, **(b)**... above each map, followed by the map's name, in
columns. **Columns** (1 to 6, never more than the number of maps) sets how many panels sit in a row. The steps of
a series share one set of colours, so that a value has the same colour in every panel. The layers of a stack each
keep their own.

!!! note "Defaults"
    A figure of one map starts at 1 column and 85 mm wide. A figure of several maps starts with 2 columns and 180 mm.
    You can change both.

### Title, size and resolution

- **Title** is written at the top of the figure. Empty means no title.
- **Width** is in millimetres, from 40 to 400. The figure's height follows from its maps. The two small
  buttons **1 column** (85 mm, a journal's column) and **page** (180 mm, the width of a page) set the usual
  widths. There is no setting in inches: 1 inch is 25.4 mm.
- **Resolution** is 150, 300, 600 or 1200 DPI (dots per inch). It sets how many pixels a PNG has and how finely the maps
  are read, and it is written into the PNG file.

The size in pixels of a PNG is the width in inches times the resolution:

| Width | 300 DPI | 600 DPI |
|---|---|---|
| 85 mm (a column) | about 1,000 px | about 2,000 px |
| 180 mm (a page) | about 2,100 px | about 4,250 px |

The maps are read as finely as the resolution shows them, and no finer. If the figure is too narrow for its maps
(many columns on a narrow width), the preview says so: widen the figure or use fewer columns.

### What is drawn on the map

Five check boxes turn the parts of a figure on and off. All are on at first.

| Box | What it draws |
|---|---|
| **Legend** | the classes with their colours and names, or a colour bar for continuous values (see below) |
| **Scale bar** | a bar in metres or kilometres; on a map in longitudes and latitudes, in metres or kilometres at the middle latitude. Needs a map with a coordinate system |
| **North arrow** | an arrow with an N at the top right of the map. Drawn only on maps whose north is at the top (not on rotated grids) |
| **Coordinates** | lines across the map at round coordinates, with their values along the edges, in the map's own coordinates (degrees on a map in longitudes and latitudes) |
| **Location inset** | a small map of the world in the map's bottom right corner, the map's place marked in red. Needs a coordinate system, and a map large enough on the page |

A map with no coordinate system (a neutral landscape drawn by chance, for example) gets no scale bar and no
inset: it is not on Earth. See [Coordinate systems](/tutorials/getting-started/coordinate-systems/).

#### The legend

For a map of classes the legend lists each class with its swatch and its name (the names of the legend you gave,
or the ones the Viewer shows). A very long legend is cut and ends with *and N more*. For a map of values the legend is a
colour bar, labelled at both ends and in the middle; on a scale of logarithms the bar says *logarithmic*.
Several maps in the same colours share one legend, at the right of the figure. When the maps have colours of their
own, each has its legend under it. The vector layers drawn over the map add their legend too (their colour classes, ranges or sizes).

### The font and the size of the text

**Font** offers three typefaces: **Source Sans 3** (the website's, and the default), **Inter**, and
**Ubuntu Light (egui)**, the application's own. **Text size** is the size of the legend's text in points, from 4 to
16 (7.5 at first); the title, the captions, the coordinates, the north arrow and the scale bar follow it. A
button **reset** brings back Source Sans 3 at 7.5 pt.

Sizes are real sizes on paper. Because the width is in millimetres and the text is in points, a letter 7.5 pt high
in the preview is 7.5 pt in the journal, at the width you chose. Check the instructions to authors of your
journal for the smallest text it accepts, and look at the preview at the width it will have in print.

The font is the same in every format:

- **PNG** draws the letters, as the font's outlines.
- **SVG** names the font in its text (`Source Sans 3`, `Inter` or `Ubuntu Light`, then a similar one that
  every computer has). The text stays text. If the font is not installed on the computer that opens the file,
  Inkscape or Illustrator shows another one.
- **PDF** embeds the font: the text is text that can be searched and copied, and looks the same on every computer.

### Placing the legend and the scale bar

By default the legend stands at the right of the maps (under each map when they have different legends) and the scale
bar at the map's bottom left. You can put either one where you want, in three ways.

1. **Drag it in the preview.** Point at the legend or the scale bar: the pointer becomes a hand. Drag it; a frame shows
   where it will stand. Let go close to a corner (within 4% of the room it can move in) and it **snaps to that
   corner**. Elsewhere it stays where you left it.
2. **Choose a corner.** The rows **Legend** and **Scale bar** each have a list: *Default*, *Top left*, *Top right*,
   *Bottom left*, *Bottom right* and *Custom*.
3. **Choose a place in percent.** With *Custom*, two fields **x** and **y** give the place as a percentage of
   the free room: x = 0 % puts the box against the left edge, 100 % against the right edge; y = 0 % at the top,
   100 % at the bottom.

A legend placed by hand goes **over** the figure, over the map (over each map when the maps have their own
legends). It never leaves the figure.

### What is kept for the next figure

The font, the text size and the places of the legend and of the scale bar are saved in your settings (the key
`figure_look` of `settings.json`, see [Settings](/app/settings/)), every time you change them. The next figure
opens with them. The title, the width, the resolution, the columns and the check boxes are **not** kept: each figure
starts with the defaults above.

### Saving

Three buttons write the figure: **Save PNG...**, **Save SVG...** and **Save PDF...**. Each asks for a file
(`figure.png`, `figure.svg` or `figure.pdf` is suggested). The file is written in the background and the window says
*saved:* with its path, or why it was not saved.

| Format | What it holds | Use it for |
|---|---|---|
| PNG | pixels at the resolution chosen; the resolution is recorded in the file, so a word processor places it at its width | manuscripts, slides, the web |
| SVG | the lines and the text as lines and text, the maps as pictures | editing in Inkscape or Illustrator |
| PDF | a page the figure's size, lines and text as lines and text, the font embedded, the maps as pictures | journals that take PDF figures, LaTeX |

For SVG and PDF the maps are pictures read at 300 DPI or the resolution you chose if it is higher.

!!! tip "For a journal"
    1. Choose the width: **1 column** (85 mm) or **page** (180 mm), or the width in the journal's instructions.
    2. Choose the resolution the journal asks for. Journals commonly ask for 300 DPI or more, and often 600 DPI for
       figures with fine lines or small text. The journal's instructions give the number.
    3. Leave the font at Source Sans 3 or choose the one the journal's style uses. Check the text at its final size in the preview.
    4. Ask for what you need: a journal that takes vector figures wants the **PDF** (or the SVG, which you
       edit and export from your drawing program). A PNG is enough for most others.
    5. Put colour vision back to normal in **View > Colour vision** before you save (see
       [Colour vision and accessibility](/app/colour-vision/)): the maps are drawn in the vision chosen at that time.
    6. Write the legend's units and the map's source in the caption of your paper, not in the Title.

## Export the view...

**Export the view...**, beside **Figure...**, writes the map as the Viewer draws it, with its layers, vectors and
labels. It has **no legend or decorations**: the Figure has them. It is for putting a map in a project of a GIS or a
slide, where the other parts are drawn by something else.

In the window **Export the view**:

- **Size**: *1 pixel a cell*, *x2* or *x4*. The window tells the size in pixels. The longest side is 8,000 pixels at
  most; a larger map is read every few cells, and the window says so.
- **with the views side by side**, when the Viewer compares side by side: all the views in one PNG, a small gap
  between them. A GeoTIFF has one grid, so it holds the map shown alone.
- **with the basemap**, when a basemap is shown: the web tiles under the map, with the data as opaque as in the Viewer.
- **Save PNG...** writes a picture with its transparency (the cells without a value are transparent).
- **Save GeoTIFF...** writes the same picture as four bands of bytes (red, green, blue and alpha) placed on the map's
  grid, to open in QGIS over other data.

A saved Plot can be exported without the application, by its id. On the command line:

```bash
terraflux plot model.toml cover -o cover.png
terraflux plot model.toml cover -o cover.tif --scale 2
```

`--scale` is 1, 2 or 4 pixels to a cell; the file's extension (`.png` or `.tif`) chooses the format. The command
runs the model and draws the Plot's layers in their palettes. A Plot that compares side by side gives its views side
by side in the PNG, and its map alone in the GeoTIFF. The same is done by the `export_plot` tool of the
[AI assistants](/cli/assistants/).

## Animations

A map of several layers (a stack, the steps of a loop, the bands of a raster) has **Play** beside its slider. Under it
are a **speed** and two buttons. The **Simulation** pane has the same ones for the steps of a run.

- **speed** is in frames a second, from 0.5 to 25. It is the same for playing and for saving.
- **Save as GIF...** writes an animated GIF, for a talk or a class.
- **Save as MP4...** writes a video (H.264) that slides and video players take. Terraflux makes the video itself:
  there is nothing to install.

Each frame has the layer's name in its corner (for a simulation, *step 12 of 50*) and its legend under it
(at most 16 classes, then *and N more*). The steps of a series share one set of colours. The layers of a stack keep their own. A small map
is enlarged (its longest side is at least 480 pixels) and a large one is read every few cells (800 pixels at most
on a side). The animation is made in the background, and you can go on working. The window tells when it is saved. Frames
are drawn in the colour vision chosen (see [Colour vision and accessibility](/app/colour-vision/)).

## Going further

- [Colour vision and accessibility](/app/colour-vision/): check that a figure reads for every reader.
- [Export to QGIS](/app/qgis/): the run's maps and vectors as a QGIS project.
- [File formats](/app/formats/): every format the application reads and writes.
- [The viewer](/app/viewer/): the map the figure is made from.
