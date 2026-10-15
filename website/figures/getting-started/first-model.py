"""Figures of 'Your first model': the forest of the CORINE excerpt and its MSPA from a model run by the command line,
the cache drawn (what runs again after a change), and the real output of `terraflux check` and `terraflux run`
(written to cli.txt, for the page)."""

import json
import shutil

from matplotlib.patches import FancyBboxPatch

from _common import figs, np, plt, CLC, WINDOW, posix, save

SLUG = "getting-started/first-model"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
shutil.copy(CLC, work / "clc.tif")

MODEL = '''format = "terraflux/1"
name = "Core forest"
description = "The core of the forest of a land cover map, by MSPA."

[inputs.landcover]
kind = "map"
path = "clc.tif"
note = "CORINE Land Cover, 25 m."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "$landcover" }
params = { rules = "23..25 -> 2\\n* -> 1" }
note = "CORINE's forests (23 broad-leaved, 24 coniferous, 25 mixed) become MSPA's foreground (2), the rest 1."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 2 }
note = "An edge of 2 cells (50 m): the forest within 50 m of its border is edge, not core."

[outputs.mspa]
from = "mspa.map"
'''
(work / "core-forest.toml").write_text(MODEL, encoding="utf-8")

# a window of the map, for --set
clip = f'''format = "terraflux/1"
[[nodes]]
id = "window"
op = "raster.clip"
inputs = {{ raster = "big.raster" }}
params = {{ extent = {WINDOW}, path = "window.tif" }}
[[nodes]]
id = "big"
op = "raster.read"
params = {{ path = "clc.tif" }}
'''
figs.run(clip, work, name="clip.toml")

log = []


def cli(*args):
    text = figs.cli(*args, cwd=work)
    log.append("$ terraflux " + " ".join(args) + "\n" + text.strip() + "\n")
    return text


cli("check", "core-forest.toml")
cli("run", "core-forest.toml")
import os
log.append("$ ls output\n" + "  ".join(sorted(os.listdir(work / "output"))) + "\n")
cli("run", "core-forest.toml", "--set", "landcover=window.tif", "--out", "window")

# a model with a wrong connection, for check's message
bad = MODEL.replace('inputs = { map = "forest.map" }', 'inputs = { map = "classes.table" }')
bad = bad.replace('[outputs.mspa]', '''[[nodes]]
id = "classes"
op = "metrics.class"
inputs = { map = "forest.map" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.mapp" }

[outputs.mspa]''')
(work / "broken.toml").write_text(bad, encoding="utf-8")
cli("check", "broken.toml")
(work / "cli.txt").write_text("\n".join(log), encoding="utf-8")
print("\n".join(log))

# ------------------------------------------------------------------ the maps
fa, _ = figs.read(work / "output" / "mspa.tif")
figs.raster(work / "output" / "mspa.tif", out / "mspa.png", palette=figs.mspa_palette(), width=900, counts=True)
# the forest itself, from the reclassification (computed again here from the land cover, the same rule)
lc, _ = figs.read(CLC)
forest = np.ma.where((lc >= 23) & (lc <= 25), 2, 1)
figs.raster(None, out / "forest.png", array=forest, colors={1: "#e9e4d8", 2: figs.FOREST},
            labels={1: "Not forest (1)", 2: "Forest (2)"}, width=900, counts=True)
figs.raster(work / "window" / "mspa.tif", out / "mspa_window.png", palette=figs.mspa_palette(), width=700)
vals, counts = np.unique(fa.compressed(), return_counts=True)
stats = {int(v): int(c) for v, c in zip(vals, counts)}
(work / "stats.json").write_text(json.dumps(stats), encoding="utf-8")
print(stats)

# ------------------------------------------------------------------ the cache, drawn
nodes = ["landcover", "forest", "mspa", "plot"]
ops = ["raster.read", "raster.reclassify", "landscape.mspa", "view.plot"]


def chain(ax, title, state):
    ax.set_xlim(0, 4)
    ax.set_ylim(0, 1)
    ax.set_axis_off()
    ax.set_title(title, loc="left", fontsize=10.5)
    colours = {"run": ("#e8f5e3", "#2f8a3a", "runs"), "cache": ("#eef0ee", "#7d877f", "from the cache"),
               "changed": ("#fdebd3", figs.EMBER, "changed: runs")}
    for i, (n, o) in enumerate(zip(nodes, ops)):
        face, edge, word = colours[state[i]]
        ax.add_patch(FancyBboxPatch((i + 0.08, 0.3), 0.74, 0.42, boxstyle="round,pad=0.02", fc=face, ec=edge, lw=1.6))
        ax.text(i + 0.45, 0.58, n, ha="center", va="center", fontsize=9.5, fontweight="bold", color=figs.INK)
        ax.text(i + 0.45, 0.43, o, ha="center", va="center", fontsize=7, color=figs.INK2)
        ax.text(i + 0.45, 0.16, word, ha="center", va="center", fontsize=8, color=edge)
        if i < 3:
            ax.annotate("", (i + 1.08, 0.51), (i + 0.84, 0.51), arrowprops=dict(arrowstyle="->", color=figs.INK2))


fig, axs = plt.subplots(3, 1, figsize=(7.2, 4.6), dpi=150)
chain(axs[0], "1. The first run: every node runs", ["run", "run", "run", "run"])
chain(axs[1], "2. edge_width changed from 2 to 3: mspa and what follows it run again", ["cache", "cache", "changed", "run"])
chain(axs[2], "3. A note changed, a node moved: nothing runs again", ["cache", "cache", "cache", "cache"])
fig.tight_layout()
save(fig, out / "cache.png")
print("done")

# ------------------------------------------------------------------ the edge width, on the window (a slider)
rows = []
for ew in [1, 2, 3, 4, 6, 8]:
    d = work / f"edge{ew}"
    d.mkdir(exist_ok=True)
    m = MODEL.replace("edge_width = 2", f"edge_width = {ew}").replace('path = "clc.tif"', 'path = "../window.tif"')
    figs.run(m, d)
    a, _ = figs.read(d / "output" / "mspa.tif")
    fg = np.isin(a.filled(0), [1, 3, 9, 17, 33, 35, 65, 67, 101, 103, 105, 109, 117, 133, 135, 165, 167])
    core = np.isin(a.filled(0), [17, 117])
    edge = np.isin(a.filled(0), [3, 103])
    rows.append({"edge_width": ew, "metres": ew * 25, "core": round(core.sum() / fg.sum() * 100, 1),
                 "edge": round(edge.sum() / fg.sum() * 100, 1)})
    figs.raster(d / "output" / "mspa.tif", out / f"edge_{ew}.png", palette=figs.mspa_palette(), width=700, legend=False)
figs.write_csv(figs.data_dir(SLUG) / "edge.csv", rows)
print(rows)
