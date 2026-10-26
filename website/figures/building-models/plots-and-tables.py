"""Figures of 'Plots and tables': plots stacked with under and opacity (a forest's MSPA over the land cover of a
15 x 15 km window of the CORINE excerpt), a table of class metrics and its chart, and tables typed in a model with
table.of (six ponds and the distances between them) made a habitat graph whose nodes' importance is computed."""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "getting-started"))
from _common import figs, np, plt, CLC, WINDOW, BROAD, BROAD_RULES, save  # noqa: E402

SLUG = "building-models/plots-and-tables"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
shutil.copy(CLC, work / "clc.tif")

legend = ", ".join(f'[{k}, "{n}", "{c}"]' for k, (n, c) in BROAD.items())
MODEL = f'''format = "terraflux/1"
name = "Forest over land cover"

[[nodes]]
id = "landcover"
op = "raster.read"
params = {{ path = "clc.tif" }}

[[nodes]]
id = "window"
op = "raster.clip"
inputs = {{ raster = "landcover.raster" }}
params = {{ extent = {WINDOW}, path = "window.tif" }}

[[nodes]]
id = "broad"
op = "raster.reclassify"
inputs = {{ map = "window.raster" }}
params = {{ rules = {json.dumps(BROAD_RULES)} }}

[[nodes]]
id = "named"
op = "raster.legend"
inputs = {{ map = "broad.map" }}
params = {{ classes = [{legend}] }}

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = {{ map = "named.map" }}
params = {{ foreground = [3], edge_width = 2 }}

[[nodes]]
id = "forest_only"
op = "raster.calc"
inputs = {{ m = "mspa.map" }}
params = {{ expression = "None if m == 0 or m == 100 or m == 220 else m" }}

[[nodes]]
id = "classes"
op = "metrics.class"
inputs = {{ map = "named.map" }}
params = {{ metrics = ["pland", "np", "area_mn", "ed"], show = "pland" }}

[[nodes]]
id = "plot_land"
op = "view.plot"
inputs = {{ value = "named.map" }}
params = {{ title = "Land cover" }}

[[nodes]]
id = "plot_forest"
op = "view.plot"
inputs = {{ value = "forest_only.map", under = "plot_land.shown" }}
params = {{ title = "MSPA over the land cover", opacity = 0.6 }}
'''
(work / "plots.toml").write_text(MODEL, encoding="utf-8")
print(figs.cli("check", "plots.toml", cwd=work))
figs.run(figs.add_writes(MODEL, {"named.map": "named.tif", "forest_only.map": "forest.tif", "classes.table": "classes.csv"}), work)

lc, _ = figs.read(work / "named.tif")
fo, _ = figs.read(work / "forest.tif")
pal = figs.mspa_palette()
from matplotlib import colors as mcolors  # noqa: E402


def rgba_of(a, palette):
    out_ = np.zeros(a.shape + (4,))
    f = a.filled(-1).astype(int)
    for v, (_, c) in palette.items():
        out_[f == v] = mcolors.to_rgba(c)
    return out_


base = rgba_of(lc, BROAD)
over = rgba_of(fo, pal)
for name, layers in (("land", [(base, 1.0)]), ("forest", [(over, 1.0)]), ("stacked", [(base, 1.0), (over, 0.6)])):
    img = np.ones(lc.shape + (3,))
    for rgba, op in layers:
        a = rgba[..., 3:4] * op
        img = img * (1 - a) + rgba[..., :3] * a
    fig = plt.figure(figsize=(4.2, 4.2), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(img, interpolation="nearest")
    ax.set_axis_off()
    save(fig, out / f"{name}.png")

# the table of class metrics
rows = figs.read_csv(work / "output" / "classes.csv")
print(rows)
names = {k: n for k, (n, _) in BROAD.items()}
figs.write_csv(data / "classes.csv", [{"class": names.get(int(float(r["class"])), r["class"]),
                                       **{k: round(float(r[k]), 2) for k in ("pland", "np", "area_mn", "ed")}}
                                      for r in rows])

# ------------------------------------------------------------------ table.of: six ponds and their distances
ponds = {1: (0, 0, 4.0), 2: (700, 300, 1.5), 3: (1300, -200, 2.5), 4: (2600, 100, 6.0), 5: (1900, 900, 0.8),
         6: (3600, -300, 3.0)}
pairs = []
ids = sorted(ponds)
for i in ids:
    for j in ids:
        if i < j:
            d = float(np.hypot(ponds[i][0] - ponds[j][0], ponds[i][1] - ponds[j][1]))
            pairs.append([i, j, round(d)])
node_rows = ", ".join(f"[{i}, {ponds[i][2]}]" for i in ids)
pair_rows = ", ".join(f"[{a}, {b}, {d}]" for a, b, d in pairs)
PONDS = f'''format = "terraflux/1"
name = "Six ponds"

[[nodes]]
id = "ponds"
op = "table.of"
params = {{ columns = ["id", "attribute"], rows = [{node_rows}] }}
note = "Six ponds and their area (ha)."

[[nodes]]
id = "distances"
op = "table.of"
params = {{ columns = ["from", "to", "value"], rows = [{pair_rows}] }}
note = "The distance between every two ponds, edge to edge (m)."

[[nodes]]
id = "graph"
op = "conn.graph"
inputs = {{ nodes = "ponds.table", pairs = "distances.table" }}
params = {{ kind = "distance" }}

[[nodes]]
id = "importance"
op = "conn.node_importance"
inputs = {{ graph = "graph.graph" }}
params = {{ distance = 1000, probability = 0.5, metrics = ["PC"] }}
note = "dPC: the share of the network's connectivity lost without each pond, for a newt whose chance of crossing 1 km is 0.5."

[outputs.importance]
from = "importance.table"
'''
(work / "ponds.toml").write_text(PONDS, encoding="utf-8")
print(figs.cli("check", "ponds.toml", cwd=work))
d = figs.workdir(f"{SLUG}/ponds")
shutil.copy(work / "ponds.toml", d / "ponds.toml")
figs.run(d / "ponds.toml", d)
imp = figs.read_csv(d / "output" / "importance.csv")
print(imp[0].keys())
for r in imp:
    print({k: r[k] for k in r if k in ("id", "node", "dPC", "dPCintra", "dPCflux", "dPCconnector", "distance")})
(work / "ponds_model.txt").write_text(PONDS, encoding="utf-8")
keep = [k for k in imp[0] if k in ("id", "dPC", "dPCintra", "dPCflux", "dPCconnector")]
figs.write_csv(data / "importance.csv", [{k: (round(float(r[k]), 2) if k != "id" else int(float(r[k]))) for k in keep} for r in imp])

# the ponds drawn
fig, ax = plt.subplots(figsize=(7, 2.9), dpi=150)
dpc = {int(float(r["id"])): float(r["dPC"]) for r in imp}
for a, b, dd in pairs:
    p = 0.5 ** (dd / 1000)
    if p > 0.2:
        ax.plot([ponds[a][0], ponds[b][0]], [ponds[a][1], ponds[b][1]], color=figs.INK2, lw=3 * p, alpha=0.7, zorder=1)
for i, (x, y, area) in ponds.items():
    ax.scatter(x, y, s=area * 90, color=figs.WATER, zorder=2, edgecolor="white")
    ax.text(x, y - 260, f"pond {i}\n{area} ha, dPC {dpc[i]:.0f}%", ha="center", va="top", fontsize=7.5, color=figs.INK)
ax.set_xlim(-500, 4100)
ax.set_ylim(-1100, 1200)
ax.set_aspect("equal")
ax.set_xlabel("m")
ax.grid(False)
ax.set_yticks([])
ax.spines["left"].set_visible(False)
save(fig, out / "ponds.png")
