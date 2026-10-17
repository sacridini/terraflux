---
title: Hilltops protected by Brazil's Forest Code
short: Hilltops (APP)
description: Find every hill of an elevation model with its base, height and mean slope, and map the hilltop permanent protection areas (APP de topo de morro) of Lei 12.651/2012, knowing which choices the law leaves open and how they change the map.
order: 40
level: intermediate
time: 45 min
ops: [raster.read, surface.hilltop, raster.reproject, raster.terrain, raster.write, table.write, view.plot]
series: Continuous surfaces
part: 4
---

In this tutorial you map the hilltop permanent protection areas of Brazil's Forest Code, the *Áreas de
Preservação Permanente de topo de morro*. You will read what the law says, see how Terraflux turns its words into
an algorithm (summits, saddles, bases, heights, slopes), build the model on a small landscape whose hills were
drawn to show each part of the rule, and then run it on real mountains. At the end you will have a map of the
protected hilltops and a table of every hill with its base, height and mean slope; you will know why a hill
qualifies or not, which inputs and settings change the answer (the plain, the resolution of the elevation model, the
way slope is measured), and why the result is a starting point for technical and legal work, not its end.

## The rule in the law

Brazil's law on the protection of native vegetation, the Forest Code (Lei nº 12.651, de 25 de maio de 2012), lists
in its article 4 the areas that are *Áreas de Preservação Permanente* (APP) "em zonas rurais ou urbanas". Its
article 3, II, defines an APP as an "área protegida, coberta ou não por vegetação nativa, com a função ambiental de
preservar os recursos hídricos, a paisagem, a estabilidade geológica e a biodiversidade, facilitar o fluxo gênico de
fauna e flora, proteger o solo e assegurar o bem-estar das populações humanas". Hilltops are its item IX:

> IX - no topo de morros, montes, montanhas e serras, com altura mínima de 100 (cem) metros e inclinação média
> maior que 25º, as áreas delimitadas a partir da curva de nível correspondente a 2/3 (dois terços) da altura mínima
> da elevação sempre em relação à base, sendo esta definida pelo plano horizontal determinado por planície ou espelho
> d'água adjacente ou, nos relevos ondulados, pela cota do ponto de sela mais próximo da elevação;
>
> (Lei nº 12.651/2012, art. 4º, IX)

In English: on the tops of hills, mounts, mountains and ranges at least 100 m high and with a mean slope greater
than 25°, the areas above the contour line at two thirds of the height of the elevation, always measured from its
base; the base is the horizontal plane of the adjacent plain or water body or, in undulating relief, the elevation
of the saddle point nearest to the elevation.

The rule has four parts, and each needs a definition before a computer can apply it:

1. **An elevation** (a hill): a summit and the ground that belongs to it.
2. **Its base**: the plain or water body around it, or in undulating relief the **saddle point** (*ponto de sela*)
   nearest to it: the lowest point of the ridge that joins it to another elevation, a col.
3. **Two criteria**: a height above the base of at least 100 m, and a mean slope (*inclinação média*) greater than
   25°.
4. **The protected area**: the part of the hill above the contour at base + 2/3 of the height.

![A profile through two peaks joined by a saddle, with their bases, heights and protected tops](/img/tutorials/surface-metrics/hilltops/profile.png "A west–east profile through the range of the synthetic landscape of this tutorial, as Terraflux measured it. Peak A stands 372 m above the plain at its foot; peak B, 184 m above the saddle that joins it to A. Each is protected above two thirds of its own height (orange lines), and the orange areas are the cells of the APP on this row.")

!!! science "Key idea: the height and the threshold of a hill"
    For a hill with summit elevation \( z_{\text{top}} \) and base \( z_{\text{base}} \):

    \[ H = z_{\text{top}} - z_{\text{base}} \qquad z_{\text{APP}} = z_{\text{base}} + \tfrac{2}{3}\,H \]

    \( H \) is the height of the hill, \( z_{\text{APP}} \) the elevation of the contour above which its top is
    protected. The hill qualifies when \( H \ge 100 \) m and its mean slope \( \bar{s} > 25° \). In the figure, A:
    \( z_{\text{base}} \) = 691 m (the plain), \( H \) = 372 m, \( z_{\text{APP}} \) = 691 + 248 = 939 m; B:
    \( z_{\text{base}} \) = 789 m (the saddle), \( H \) = 184 m, \( z_{\text{APP}} \) = 912 m.

The words *nearest saddle point* are the heart of the rule in hilly country. Peak B in the figure is not 355 m high
(its top above the plain): it is a hill of 184 m standing on the shoulder of A, and its base is the col between them.
Mountaineers call this height **topographic prominence**: the height of a summit above the lowest col (its *key
col*) on the way to any higher summit (Kirmse & de Ferranti, 2017). Terraflux reads the law's "nearest saddle point"
as the key col, for the reasons given below.

The rule changed in 2012. Under the previous Forest Code (Lei nº 4.771/1965, revoked by Lei 12.651), CONAMA's
Resolução nº 303/2002 protected hills from 50 m high with slopes above 30 % (about 17°) "na linha de maior
declividade", measured from the lowest depression around them, and also protected ridge lines and groups of nearby
hills. The new thresholds protect far fewer hilltops: in a hilly area of the Zona da Mata of Minas Gerais, Guimarães
& Moreira (2018) found that the hilltop APPs went from 29 % of the area under the old criteria to none under the new
ones. Soares-Filho et al. (2014) and Brancalion et al. (2016) review what the 2012 law changed for conservation and
restoration in general.

## How Terraflux finds hills and their bases

[[surface.hilltop]] runs landschaft's open implementation of the rule (written from the law, not from another
program). It has three steps.

**1. Summits and saddles: a falling water level.** Imagine the landscape flooded above its highest point, and the
water level falling slowly. The first land to appear is the highest summit, an island. As the level falls, other
summits appear as islands of their own, and the islands grow. When two islands touch, the cell where they touch is a
**saddle**: from that level down they are one landform. The lower of the two summits ends there: its base is the
saddle, its height its prominence. The higher one goes on growing, now with the other's island inside it.

![Three moments of the falling water level on the profile of the range](/img/tutorials/surface-metrics/hilltops/sweep.png "The algorithm as a falling water level (blue) on the profile above. Left: A and B are separate islands (green). Middle: just below the saddle they join, and B's base is the saddle. Right: A's island reaches the plain, which becomes its base.")

Terraflux does this on the grid: it sorts the cells from the highest down and joins each to the islands of its eight
neighbours already reached (a *union-find* structure; the history of the joins is a *merge tree*). Every summit, every
saddle and every base is found exactly, in one pass over the cells.

**2. The base of each summit.** A summit's island ends, and its base is fixed, at the first of these it meets:

- **a saddle** to the island of a higher summit (`base_kind` = 0, saddle): the law's "ponto de sela";
- **a cell of the plain**, if you give a map of plains and water bodies (`base_kind` = 1, plain): the elevation of
  that cell, the law's "plano horizontal determinado por planície ou espelho d'água adjacente";
- **neither**: this is the highest summit of a connected area, which never meets higher ground. Its base is the lowest
  cell of the area (`base_kind` = 2, lowest), often the edge of the map, as you will see.

**3. Slope, qualification and the protected top.** The **mean slope** of a hill is the mean of the slopes (Horn's
method, in degrees, as [[surface.terrain]] computes them) of the cells of its island above its base: the law's
"inclinação média". With **slope** = `line`, it is instead the angle of the straight line from the summit to the
hill's foot, the nearest cell at or below the base. A hill **qualifies** if its height is at least **min_height**
(100 m) and its slope greater than **min_slope** (25°). Its **APP** is the set of cells joined to its summit through
cells at least \( z_{\text{APP}} \) high: the cells above the threshold contour around that summit.

Where hills nest (B inside A's island), each is judged on its own. If B's top rises above A's threshold but joins A
only below it, B is not part of A's APP: it is a separate hill and qualifies or not by itself. Where the protected
tops of two qualifying hills overlap, the cell belongs to both, and the `hill` output names the one with the higher
threshold.

!!! note "The choices the law leaves open"
    Oliveira et al. (2019) list the questions a mapping method must answer, and show that the answers change the
    mapped area: what the nearest saddle is, how the mean slope is measured, what the base is when there is no plain,
    and at what scale to work. Terraflux's answers:

    | The law's words | Terraflux's reading | Other readings in use |
    |---|---|---|
    | *ponto de sela mais próximo da elevação* | the key col: the saddle where the hill's region meets higher ground (its prominence) | the saddle nearest in distance to the summit; the lowest point around the hill (CONAMA 303's "depressão mais baixa") |
    | *inclinação média* | the mean slope of the hill's cells above its base (**slope** = `mean`) | the slope of the line from the summit to its base (**slope** = `line`), or of several such lines |
    | *planície ou espelho d'água adjacente* | the cells you give as `plain`: the first one the hill meets sets its base | plains inferred from slope or elevation |
    | *altura mínima* of 100 m, *maior que* 25° | height ≥ 100 m, slope > 25° | |
    | *2/3 da altura mínima da elevação* | two thirds of the hill's height above its base (**fraction** = 2/3) | |

    These readings are documented and the code is open. If your state agency or the courts adopt another reading,
    say so, and compare.

## When to use it

- **Environmental compliance and the rural environmental registry.** Every rural property in Brazil must be
  registered in the *Cadastro Ambiental Rural* (CAR, art. 29 of the law) with its APPs. Oliveira & Fernandes Filho
  (2016), who published an automatic method for hilltop APPs, stress how much mapping them at large scale matters to
  the CAR. Terraflux maps them for a whole municipality or basin in one run, for checking declared APPs, for
  licensing studies and for planning restoration.
- **Research on the Forest Code.** How much land the rule protects, where, and how that changed between the old and
  the new rule (Guimarães & Moreira, 2018), or under different readings of the law (Oliveira et al., 2019).
- **Conservation and restoration planning.** Hilltop APPs are part of the network of protected vegetation on
  properties; with riparian APPs and legal reserves they are inputs to prioritization (see
  [Spatial prioritization](../../spatial-prioritization/)) and to scenarios of land change, where they become areas
  where clearing is not allowed (see [Land change simulation](../../land-change/)).

What it is not:

- **Not a legal demarcation.** The result depends on the elevation model and on readings of the law. It is a
  technical starting point that must be checked in the field and by the people responsible for legal work.
- **Not the other APPs of article 4.** Areas above 1,800 m (item X), steep slopes above 45° (item V), riverbanks,
  springs, *veredas* and the others need their own maps.
- **Not CONAMA 303/2002 in full**, nor Dinamica EGO's `CalcHilltop` (see [below](#conama-3032002-and-dinamica-egos-calchilltop)).
- **Not a landform classification.** For ridges, slopes and valleys everywhere, see [geomorphons](../geomorphons/).

## What you need

- **An elevation model in metres.** [[surface.hilltop]] accepts projected coordinates or longitude and latitude (it
  then converts the cells' sides to metres at the centre of the map), but projected coordinates are better for the
  areas in the table; for Brazil, SIRGAS 2000 / UTM (EPSG:31978 to EPSG:31985). The model must describe the
  **ground**: radar models such as SRTM include part of the forest canopy, which adds to the heights of forested
  hills and changes slopes. ANADEM, a terrain model of South America made from Copernicus GLO-30 by removing the
  vegetation's bias with machine learning and GEDI's lidar heights (Laipelt et al., 2024), and topographic data from
  IBGE or state surveys are better choices where available.
- **A resolution suited to the hills.** Coarse cells flatten slopes: a hill steeper than 25° on a 10 m model can fall
  below it on a 90 m one. Oliveira et al. (2019) found resolutions of about 30 to 75 m adequate in their area, finer
  data producing too many small saddles and coarser data merging neighbouring hills. Report the resolution you used.
- **A map of plains and water bodies** (optional but important): any raster on the same grid, non-zero (or 1) on
  floodplains, lakes, reservoirs, rivers and the sea, 0 or no value elsewhere. Hydrography and water maps (rivers,
  MapBiomas' water class), floodplain maps, or careful masks of flat lowland made with [[map.calc]] all serve. Without
  it, every hill's base is a saddle, and the highest summit's base is the lowest cell of the map.
- **The files of this tutorial**: [`hills.tif`](/data/tutorials/surface-metrics/hilltops/hills.tif), a synthetic
  landscape of 4.2 × 6 km in cells of 30 m (SIRGAS 2000 / UTM zone 23S), and
  [`plain.tif`](/data/tutorials/surface-metrics/hilltops/plain.tif), its valley floor and river. The valley floor falls
  from 700 m in the west to 620 m in the east; on it stand five hills drawn to show each part of the rule: a range of
  two peaks (A and B), a lone steep hill (C), a high but gentle hill (D) and a steep knoll (E).

![The synthetic landscape](/img/tutorials/surface-metrics/hilltops/synthetic-dem.png "hills.tif: five hills on a valley floor that falls to the east. A and B form a range joined by a saddle; C stands alone; D is 208 m high but gentle; E is steep but low.")

## Build the model

### Hills and bases

<div class="steps" markdown>

1. Save `hills.tif` and `plain.tif` in a folder. In the library, open **IO** and click [[raster.read]]; set **path**
   to `hills.tif` and rename the node `dem` (++ctrl+r++).
2. With `dem` selected, open **Surface metrics** and click [[surface.hilltop]]; it comes connected through its `dem`
   input. Rename it `hilltop`. Leave its settings at the law's values: **min_height** = 100, **min_slope** = 25,
   **slope** = `mean`, **fraction** = 2/3 (0.6667).
3. Press ++ctrl+enter++: the panel under the canvas shows the `app` output; its header lists the node's outputs,
   and choosing `hills` there shows the table.

</div>

```tfgraph
#! caption: The hilltops of the synthetic landscape, with the law's settings, without a plain.
#! highlight: hilltop
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "hills.tif" }
note = "Elevation in metres, SIRGAS 2000 / UTM 23S."

[[nodes]]
id = "hilltop"
op = "surface.hilltop"
inputs = { dem = "dem.raster" }
params = { min_height = 100, min_slope = 25, slope = "mean" }
note = "Lei 12.651/2012, art. 4, IX."
```

The outputs of [[surface.hilltop]]:

- `app`: 1 in the protected tops of the hills that qualify, 0 elsewhere on the model.
- `hill`: for each protected cell, the hill (a row of `hills`) whose top holds it; no value elsewhere.
- `hills`: a table with a row per summit, the highest first: `hill` (its number), `x`, `y` (the summit), `top`,
  `base`, `base_kind` (0 saddle, 1 plain, 2 lowest), `base_x`, `base_y` (the saddle, the plain cell met, or the
  lowest cell nearest the summit), `height`, `mean_slope` and `line_slope` (degrees), `area` and `area_ha` (the
  hill's region above its base, in map units² and in hectares), `qualifies` (1 or 0), `threshold`
  (\( z_{\text{APP}} \)) and `app_cells` (the cells of its protected top).

Without a plain, two hills qualify, B and C, but A, the highest, does not: as the highest summit of the map it never meets
higher ground, so its base is the lowest cell of the whole map (618 m, at the eastern edge), its height 445 m, and
its "mean slope" that of every cell of the map above that level, 5°, mostly valley floor. The rule is applied to the
whole landscape as if it were one hill.

### Add the plain

<div class="steps" markdown>

1. Add a second [[raster.read]], named `plain`, with **path** = `plain.tif`.
2. Drag from its `raster` port to the `plain` input of `hilltop`.
3. Run again.

</div>

```tfgraph
#! caption: The plain and the river given to the operator: a hill's base is the first plain cell its region meets.
#! highlight: plain
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "hills.tif" }

[[nodes]]
id = "plain"
op = "raster.read"
params = { path = "plain.tif" }
note = "1 on the valley floor and the river, 0 on the hills."

[[nodes]]
id = "hilltop"
op = "surface.hilltop"
inputs = { dem = "dem.raster", plain = "plain.raster" }
params = { min_height = 100, min_slope = 25, slope = "mean" }
```

Now A's base is the plain at its western foot (691 m, `base_kind` 1), its height 372 m and its mean slope 30.4°: it
qualifies, and its top is protected above 939 m.

```tfcompare
caption: "Drag the handle: the protected hilltops (orange) without the plain (left) and with it (right; the plain in blue). Without the plain, A is measured from the lowest corner of the map and fails."
before: { src: /img/tutorials/surface-metrics/hilltops/synthetic-app-noplain.png, label: Without plain }
after: { src: /img/tutorials/surface-metrics/hilltops/synthetic-app.png, label: With plain }
```

### Save the results and see them

<div class="steps" markdown>

1. Add a [[raster.write]] after `hilltop`'s `app` output, **path** = `app.tif`, and a [[table.write]] after its `hills`
   output, **path** = `hills.csv`.
2. For a picture, add a [[raster.terrain]] after `dem` with **measure** = `hillshade`, a [[view.plot]] after it, and a
   second [[view.plot]] with `hilltop`'s `app` in `value`, the first plot's `shown` in `under` and **opacity** = 0.6.

</div>

```tfgraph
#! caption: The protected tops written to a GeoTIFF, the table of hills to a CSV file, and the tops shown over the relief.
#! highlight: save_app, save_hills, shade, relief, map
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "hills.tif" }

[[nodes]]
id = "plain"
op = "raster.read"
params = { path = "plain.tif" }

[[nodes]]
id = "hilltop"
op = "surface.hilltop"
inputs = { dem = "dem.raster", plain = "plain.raster" }
params = { min_height = 100, min_slope = 25, slope = "mean" }

[[nodes]]
id = "save_app"
op = "raster.write"
inputs = { value = "hilltop.app" }
params = { path = "app.tif" }

[[nodes]]
id = "save_hills"
op = "table.write"
inputs = { table = "hilltop.hills" }
params = { path = "hills.csv" }

[[nodes]]
id = "shade"
op = "raster.terrain"
inputs = { raster = "dem.raster" }
params = { measure = "hillshade" }

[[nodes]]
id = "relief"
op = "view.plot"
inputs = { value = "shade.raster" }
params = { title = "Relief" }

[[nodes]]
id = "map"
op = "view.plot"
inputs = { value = "hilltop.app", under = "relief.shown" }
params = { title = "Hilltop APP", opacity = 0.6 }
```

=== "Application"

    Press ++f5++ to run the whole model. The tab *Hilltop APP* shows the protected tops over the relief; select
    `hilltop` and choose `hills` in the header of the panel under the canvas to see the table. In a GIS, the GeoTIFF `app.tif` can be turned into polygons for
    a report or for the CAR.

=== "Command line"

    Save the model as `hilltops.toml` beside the two files and run:

    ```bash
    terraflux check hilltops.toml
    terraflux run hilltops.toml --out output
    ```

    `app.tif` and `hills.csv` are written beside the model.

For a separator that a Brazilian spreadsheet reads, set [[table.write]]'s **separator** to `;` and **decimal** to
`,`.

## Read the results

### The synthetic landscape

![The protected hilltops of the synthetic landscape](/img/tutorials/surface-metrics/hilltops/synthetic.png "The result of the model above: the hilltop APPs (orange) of A, B and C over the relief, with the plain given to the operator (blue).")

```tftable
data: /data/tutorials/surface-metrics/hilltops/synthetic-hills.csv
labels: { hill: "Hill", top: "Top (m)", base: "Base (m)", base_kind: "Base", height: "Height (m)", mean_slope: "Mean slope (°)", line_slope: "Line slope (°)", qualifies: "Qualifies", threshold: "APP from (m)", app_ha: "APP (ha)" }
digits: 1
caption: "The five hills in the hills table (the two other rows are bumps of the valley floor less than a metre high). APP area from app_cells × 0.09 ha."
```

Each hill shows one part of the rule:

- **A** stands 372 m above the plain at its foot, with a mean slope of 30.4°: it qualifies, and 17.6 ha of its top
  are protected. Its base is the plain at its *western* foot (691 m), the highest plain cell it touches, because the
  valley floor falls eastwards; its eastern foot is about 20 m lower. On sloping floodplains, decide whether that is
  the reading you want (see the pitfalls).
- **B** is measured from the saddle that joins it to A (789 m), not from the plain: its height is 184 m, not 355 m.
  It qualifies on its own, with a protected top of 5.3 ha above 912 m.
- **C** is a lone steep hill, 166 m high: it qualifies. Its base is listed as a saddle at the level of the plain: it
  meets the higher ground of A's region (which, falling, has spread over the valley floor) on the plain itself.
- **D** is 208 m high but its mean slope is only 13.8°: it does not qualify. A high, gentle hill is not protected by
  the 2012 rule, however large.
- **E** is steep (30.4°) but only 78 m high: it does not qualify.

### Real mountains

The same model on the Oregon elevation model of the [first part](../terrain/), reprojected to UTM zone 10N in cells of
30 m as there (no plain given), finds 292 summits. Most are bumps a few metres high on larger landforms; five stand at
least 100 m above their base, and **one** qualifies: a summit of 1,065 m, 157 m above its saddle, with a mean slope
of 25.1°. Its protected top covers 82 cells, 7.4 ha.

![The summits of at least 100 m in the Oregon mountains and the one hilltop APP](/img/tutorials/surface-metrics/hilltops/oregon.png "The five summits at least 100 m above their base (triangles), their saddles (circles) and the single hilltop APP (orange) of the Oregon mountains at 30 m. The highest summit (top centre) has no saddle: its base is the lowest cell of the map.")

```tftable
data: /data/tutorials/surface-metrics/hilltops/oregon-hills.csv
labels: { hill: "Hill", top: "Top (m)", base: "Base (m)", base_kind: "Base", height: "Height (m)", mean_slope: "Mean slope (°)", line_slope: "Line slope (°)", area_ha: "Area (ha)", qualifies: "Qualifies" }
digits: 1
caption: "The five summits of the Oregon model at least 100 m above their base, from the hills table."
```

![A closer look at the hill that qualifies](/img/tutorials/surface-metrics/hilltops/oregon-zoom.png "The hill that qualifies, with contours every 20 m: its summit (triangle), its saddle at 908 m (circle), the contour of its base (dashed blue) and the contour at two thirds of its height, 1,013 m (orange), which bounds the protected top.")

Three things are worth seeing in this table:

- **The highest summit fails for a reason that has nothing to do with it.** With no plain given, its base is the
  lowest cell of the map (440 m, in the south-east corner), its height 948 m, and its mean slope (18.1°) is that of
  the whole map. The result depends on where the map was cut: give the plains and rivers, or read the model with the
  valley that surrounds the range.
- **The hill that qualifies does so by 0.1°.** Its mean slope is 25.12°. A different elevation model, a different
  resolution or a different reading of "inclinação média" may change the decision, as the next section shows.
- **It touches the edge of the map.** Its base contour reaches the western edge (in the close-up, the dashed line
  runs off the map). Part of the hill, and maybe a lower saddle, lie outside. Always read a margin around the area of
  interest wide enough to hold whole hills and their saddles.

### How to report

Give the elevation model (source, date, whether it is a surface or a terrain model), its resolution and projection;
the plains and water bodies used as bases and their source; the operator and its settings (min_height, min_slope,
slope = mean or line, fraction); and how many summits, how many hills of at least 100 m, how many qualify and the
area protected. Give the table of qualifying hills (base kind, base, height, slope, threshold) as an appendix: it lets
another analyst check each decision.

## Settings that change the answer

### The slope threshold and how slope is measured

The law fixes the threshold at 25°, but the number of hills that pass it depends on how steep the elevation model
makes them. On the Oregon model, lowering **min_slope** step by step shows how close to the threshold the hills sit:

```tfchart
type: line
data: /data/tutorials/surface-metrics/hilltops/min-slope.csv
x: min_slope
y: ["Hills that qualify"]
colors: { "Hills that qualify": "#e8901f" }
xlabel: min_slope (degrees)
ylabel: Hills
height: 260
caption: "Hills of the Oregon model that qualify at slope thresholds from 15° to 30° (min_height = 100 m, mean slope). At 17.5° the highest summit, measured from the corner of the map, passes too, and the protected area jumps from 12 ha to 1,892 ha."
```

The jump at 17.5° is the edge effect of the previous section, not a property of the mountains: a reason to always
give the plains.

With **slope** = `line` (the angle from the summit to its foot), two hills qualify instead of one: the hill of 743 m
has a mean slope of 23.2° but a line slope of 25.6°. The two measures differ most on hills whose flanks are uneven
(steep near the top, gentle below, or the reverse). Oliveira et al. (2019) found that a mean over the hill's cells
generally protected more area than lines from the summit; the law says *média* without saying of what, so state your
choice.

### The resolution of the elevation model

Coarse cells smooth the relief, and slopes fall. Move the slider to see the same mountains at 30, 60, 90 and 120 m:

```tfslider
label: Cell size (m)
caption: "The hilltop APPs of the Oregon model reprojected to cells of 30 to 120 m. The same five summits stand at least 100 m above their base at every resolution, but the steepest of them loses its slope: 25.1° at 30 m, 20.5° at 120 m. From 60 m on, no hill qualifies."
frames:
  - { value: 30, src: /img/tutorials/surface-metrics/hilltops/res_30.png, caption: "30 m", stats: { Summits: "292", "Hills that qualify": "1", "APP": "7.4 ha", "Steepest mean slope": "25.1°" } }
  - { value: 60, src: /img/tutorials/surface-metrics/hilltops/res_60.png, caption: "60 m", stats: { Summits: "137", "Hills that qualify": "0", "APP": "0 ha", "Steepest mean slope": "23.6°" } }
  - { value: 90, src: /img/tutorials/surface-metrics/hilltops/res_90.png, caption: "90 m", stats: { Summits: "92", "Hills that qualify": "0", "APP": "0 ha", "Steepest mean slope": "21.7°" } }
  - { value: 120, src: /img/tutorials/surface-metrics/hilltops/res_120.png, caption: "120 m", stats: { Summits: "65", "Hills that qualify": "0", "APP": "0 ha", "Steepest mean slope": "20.5°" } }
start: 0
chart:
  type: line
  data: /data/tutorials/surface-metrics/hilltops/resolution.csv
  x: cell_m
  y: ["Steepest mean slope (degrees)"]
  colors: { "Steepest mean slope (degrees)": "#b5452c" }
  xlabel: Cell size (m)
  height: 240
```

Heights are robust to the resolution (a summit and a saddle are still there at 120 m, a little lower and higher);
slopes are not. Victoria (2011), mapping hilltops of the old rule in Campinas (SP) from four elevation models, found
that an automatic method made for maps at 1:50,000 did not suit more detailed data, and had to adapt it. Oliveira et
al. (2019) proposed lowering the slope threshold for coarse models to compensate, which
they say needs more testing. The law has no such correction: use the finest reliable terrain model you have, and
report its resolution. Finer is not always better either: on lidar models of 5 and 15 m, Oliveira et al. (2019) found
so many saddle points that the relief was cut into hills less than 10 m high. The merge tree copes with that (a small
bump has a small height and fails, and the hill it sits on is still measured from its own saddle), but slopes from a
very fine model include micro-relief (boulders, gullies, terraces) and are steeper than those of coarser data.

### fraction and min_height

**fraction** (2/3 by default) moves the threshold contour: 0.5 protects the upper half of each hill. The law fixes it;
change it only to test the sensitivity or to apply another rule. **min_height** (100 m) is the law's; CONAMA 303/2002
used 50 m.

### CONAMA 303/2002 and Dinamica EGO's CalcHilltop

With **min_height** = 50, **min_slope** = 16.7 (30 %) and **slope** = `line`, [[surface.hilltop]] applies the height and
slope thresholds of CONAMA 303/2002 to the law's geometry. On the Oregon model, 11 hills qualify instead of 1, and 89.2
ha are protected instead of 7.4 ha:

```tfcompare
caption: "Drag the handle: the hilltop APPs of the Oregon model under Lei 12.651/2012 (left, 7.4 ha) and with CONAMA 303/2002's thresholds of height and slope (right, 11 hills, 89.2 ha)."
before: { src: /img/tutorials/surface-metrics/hilltops/oregon-law.png, label: Lei 12.651/2012 }
after: { src: /img/tutorials/surface-metrics/hilltops/oregon-conama.png, label: CONAMA thresholds }
```

This is **not** CONAMA's full rule, and should not be presented as such. CONAMA 303/2002 (art. 2, IV to VI, and art. 3,
V and VI) also distinguished *morros* (50 to 300 m) from *montanhas* (above 300 m, with no slope criterion), measured
the base from "a cota da depressão mais baixa ao seu redor" rather than a saddle, protected the *linhas de cumeada*
(ridge lines) above two thirds of the height of their lowest peak in segments of 1,000 m, and grouped hills whose
summits are less than 500 m apart, protecting the whole group above two thirds of the height of its lowest hill.
Terraflux implements none of those.

Dinamica EGO, the land change platform of the Federal University of Minas Gerais (Soares-Filho et al., 2002), has a
functor for hilltops, `CalcHilltop`, that follows the older CONAMA rule on an elevation model cut into elevation
bands: a hill's base is the band of a local minimum reached downhill, and its slope criterion is the angle from the
summit to the nearest cell of the base band that borders other features or the edge of the map. landschaft's authors
measured it, with Dinamica EGO 8.13, at 5° on an isolated cone of 21.8° on a plain. Its results therefore differ from
both the 2012 law and [[surface.hilltop]], and the two should not be mixed in one study. In landschaft's validation,
only the heights of the two were compared (they agree to 0.05 m on cones); [[surface.hilltop]] is checked against an
independent computation on random elevation models and on cones, ranges and lakes of known geometry.

## Common pitfalls

- **No plain.** Without a map of plains and water bodies, the highest summit of the map is measured from the map's
  lowest cell, and its mean slope includes the whole map. Give the plains, or at least check every row of the table
  with `base_kind` = 2.
- **Sloping plains.** The base is the first plain cell a hill's region meets, the highest one at its foot. On a valley
  floor that falls steeply, the hill's downhill side is lower than its base. If your agency uses another reading (the
  mean elevation of the foot, the lowest plain cell), compute it from the table's coordinates and say so.
- **Edges.** Hills cut by the edge of the map are measured incompletely, and their saddles may lie outside. Read the
  elevation model with a wide margin (several kilometres in mountains), and cut the results afterwards.
- **Surface models.** SRTM and similar models include part of the canopy: a forested hilltop looks higher and its
  edges steeper than the ground. Prefer a terrain model (ANADEM, lidar, topographic surveys).
- **Resolution.** Slopes fall with coarser cells; the decision for hills near 25° depends on it. Report the
  resolution, and test another if many hills are near the threshold.
- **Geographic coordinates.** They work, but areas are computed with the cell size at the centre of the map; for
  large areas or reports of hectares, use a projected CRS.
- **Holes.** Cells without value break a hill's region apart, and a hole at a saddle or a foot moves the base. Fill
  small holes first ([[raster.fill]]).
- **Consolidated areas.** The map shows where the law protects hilltops, not what is allowed there: in rural areas
  occupied before 22 July 2008 (*áreas rurais consolidadas*, art. 3, IV), article 63 allows some activities to
  continue on hilltops, and forbids new conversion. Legal consequences are for legal work.

## Going further

- [Landforms with geomorphons](../geomorphons/): ridges, slopes and valleys everywhere, at a scale you choose.
- [Terrain from an elevation model](../terrain/): slope and the indices behind the mean slope.
- [Terrain hydrology](../../terrain-hydrology/): rivers and floodplains, from which a map of plains can be made.
- [Land change simulation](../../land-change/) and [Spatial prioritization](../../spatial-prioritization/): APPs as
  restrictions and as targets.
- The operators' reference: [[surface.hilltop]], [[surface.terrain]], [[raster.reproject]], [[map.calc]].

## References

- Brancalion, P. H. S., Garcia, L. C., Loyola, R., Rodrigues, R. R., Pillar, V. D., & Lewinsohn, T. M. (2016). A critical analysis of the Native Vegetation Protection Law of Brazil (2012): updates and ongoing initiatives. *Natureza & Conservação*, 14, 1–15. https://doi.org/10.1016/j.ncon.2016.03.003
- Brasil. (2012). Lei nº 12.651, de 25 de maio de 2012. Dispõe sobre a proteção da vegetação nativa. *Diário Oficial da União*, 28 May 2012. https://www.planalto.gov.br/ccivil_03/_ato2011-2014/2012/lei/l12651.htm
- Conselho Nacional do Meio Ambiente (CONAMA). (2002). Resolução nº 303, de 20 de março de 2002. Dispõe sobre parâmetros, definições e limites de Áreas de Preservação Permanente. *Diário Oficial da União*, 90, 13 May 2002, section 1, p. 68.
- Guimarães, F. S., & Moreira, A. A. M. (2018). As áreas de preservação permanente de topo de morro e a Lei Federal nº 12.651/2012: um estudo de caso na região da Zona da Mata – MG. *Caminhos de Geografia*, 19(67), 175–187. https://doi.org/10.14393/hygeia196712
- Kirmse, A., & de Ferranti, J. (2017). Calculating the prominence and isolation of every mountain in the world. *Progress in Physical Geography: Earth and Environment*, 41(6), 788–802. https://doi.org/10.1177/0309133317738163
- Laipelt, L., Comini de Andrade, B., Collischonn, W., de Amorim Teixeira, A., Paiva, R. C. D., & Ruhoff, A. (2024). ANADEM: A digital terrain model for South America. *Remote Sensing*, 16(13), 2321. https://doi.org/10.3390/rs16132321
- Oliveira, G. C., & Fernandes Filho, E. I. (2016). Automated mapping of permanent preservation areas on hilltops. *Cerne*, 22(1), 111–120. https://doi.org/10.1590/01047760201622012100
- Oliveira, G. C., Fernandes Filho, E. I., & Francelino, M. R. (2019). Mapping of permanent preservation areas on hilltops: Technical issues. *Floresta e Ambiente*, 26(4), e20170262. https://doi.org/10.1590/2179-8087.026217
- Soares-Filho, B. S., Cerqueira, G. C., & Pennachin, C. L. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Soares-Filho, B., Rajão, R., Macedo, M., Carneiro, A., Costa, W., Coe, M., Rodrigues, H., & Alencar, A. (2014). Cracking Brazil's Forest Code. *Science*, 344(6182), 363–364. https://doi.org/10.1126/science.1246663
- Victoria, D. C. (2011). Influência da escala em uma metodologia de delimitação automática de áreas de preservação permanente em topo de morro e montanha e adaptação do método para escalas detalhadas. *Revista Brasileira de Cartografia*, 62(3). https://doi.org/10.14393/rbcv62n3-43684
