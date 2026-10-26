"""Figures of 'Groups and submodels': Sibuyan's forest in 1997 and 2011 (lulcc's data), a group that makes the
forest's context, a submodel (lib/forest_core.toml) used twice to find the core forest of both dates, and the core
forest lost between them."""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "building-models/groups-and-submodels"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
shutil.copy(figs.ROOT / "tests" / "data" / "inputs" / "sibuyan_landuse.tif", work / "sibuyan_landuse.tif")
(work / "lib").mkdir()

SUB = '''format = "terraflux/1"
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
op = "raster.calc"
inputs = { m = "mspa.map", lu = "$landuse" }
params = { expression = "None if isnull(lu) else m == 17 or m == 117" }
note = "1 where the forest is core (17, or 117 inside a core-opening), 0 on the rest of the island."

[outputs.core]
from = "core.map"

[outputs.mspa]
from = "mspa.map"
'''
(work / "lib" / "forest_core.toml").write_text(SUB, encoding="utf-8")

MAIN = '''format = "terraflux/1"
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
inputs = { landuse = "lu1997.raster" }
params = { path = "lib/forest_core.toml" }

[[nodes]]
id = "core2011"
op = "control.submodel"
inputs = { landuse = "lu2011.raster" }
params = { path = "lib/forest_core.toml" }

[[nodes]]
id = "change"
op = "raster.calc"
inputs = { a = "core1997.core", b = "core2011.core" }
params = { expression = "a * 2 + b" }

[[nodes]]
id = "context"
op = "control.group"
inputs = { land = "lu1997.raster" }

[nodes.body]
outputs = { share = "share.map", edge = "edge.map" }

[[nodes.body.nodes]]
id = "forest"
op = "raster.calc"
inputs = { l = "@land" }
params = { expression = "l == 1" }

[[nodes.body.nodes]]
id = "share"
op = "raster.calc"
inputs = { f = "forest.map" }
params = { expression = "nbmean(f, 2)" }

[[nodes.body.nodes]]
id = "edge"
op = "raster.calc"
inputs = { f = "forest.map", s = "share.map" }
params = { expression = "f == 1 and s < 1" }
'''
model = figs.add_writes(MAIN, {"change.map": "change.tif", "core1997.mspa": "mspa1997.tif",
                               "core2011.mspa": "mspa2011.tif", "context.share": "share.tif",
                               "context.edge": "edge.tif"})
(work / "main.toml").write_text(MAIN, encoding="utf-8")
log = ["$ terraflux check main.toml\n" + figs.cli("check", "main.toml", cwd=work).strip() + "\n"]
r = figs.run(model, work, name="figs.toml")
log.append("$ terraflux run main.toml\n" + (r.stdout + r.stderr).strip() + "\n")
(work / "cli.txt").write_text("\n".join(log), encoding="utf-8")
print("\n".join(log))

pal = figs.mspa_palette()
for y in (1997, 2011):
    figs.raster(work / f"mspa{y}.tif", out / f"mspa{y}.png", palette=pal, width=560, legend=False)
ch, _ = figs.read(work / "change.tif")
figs.raster(work / "change.tif", out / "change.png", width=820,
            palette={0: ("Never core (other land, edge...)", "#e9e4d8"), 1: ("Became core", "#2f6f9f"),
                     2: ("Core in 1997, not in 2011", "#d55e00"), 3: ("Core at both dates", "#3b8a5a")}, counts=True)
counts = {int(v): int(c) for v, c in zip(*np.unique(ch.compressed(), return_counts=True))}
print("change", counts, "cells", ch.count())
sh, _ = figs.read(work / "share.tif")
figs.raster(work / "share.tif", out / "share.png", cmap="viridis", unit="share of forest in 5 x 5 cells", width=560)
figs.raster(work / "edge.tif", out / "edge.png", colors={0: "#e9e4d8", 1: "#d55e00"}, labels={0: "other", 1: "forest edge"},
            width=560, legend=False)
for y in (1997, 2011):
    a, _ = figs.read(work / f"mspa{y}.tif")
    v, c = np.unique(a.compressed(), return_counts=True)
    print(y, {int(x): int(n) for x, n in zip(v, c)})
