"""Figures of "Random walks and randomised shortest paths" (grid distances, part 2): the passages of walkers from
the southern forest to the north-eastern one over the class resistance, from a random walk to the least-cost path
as theta grows, and the commute, randomised-shortest-path and least-cost distances between five forests."""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-networks"))
import _clc  # noqa: E402
from _clc import figs, np, plt  # noqa: E402
from matplotlib import colors as mc  # noqa: E402

SLUG = "grid-distances/random-walks"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)
pts = figs.WEB / "static" / "data" / "tutorials" / "grid-distances" / "cost-distance" / "forests.geojson"
if not pts.exists():
    raise SystemExit("run website/figures/grid-distances/cost-distance.py first")
shutil.copy2(pts, work / "forests.geojson")

THETAS = [0.00001, 0.0001, 0.0003, 0.001, 0.003, 0.01]
RULES = _clc.RESISTANCE_RULES.replace("\n", "\\n")
MODEL = f"""format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = {{ path = "clc_100m.tif" }}

[[nodes]]
id = "resistance"
op = "raster.reclassify"
inputs = {{ map = "land.raster" }}
params = {{ rules = "{RULES}" }}

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = {{ map = "land.raster" }}
params = {{ habitat = [23, 24, 25] }}

[[nodes]]
id = "patches"
op = "conn.nodes"
inputs = {{ map = "land.raster" }}
params = {{ habitat = [23, 24, 25] }}

[[nodes]]
id = "patch"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "patches.table" }}
params = {{ column = "id" }}

[[nodes]]
id = "south"
op = "raster.calc"
inputs = {{ p = "patch.map" }}
params = {{ expression = "coalesce(p == 55, 0)" }}

[[nodes]]
id = "north_east"
op = "raster.calc"
inputs = {{ p = "patch.map" }}
params = {{ expression = "coalesce(p == 96, 0)" }}

[[nodes]]
id = "transition"
op = "cost.transition"
inputs = {{ map = "resistance.map" }}
params = {{ function = "mean", directions = 8, reciprocal = true }}

[[nodes]]
id = "for_costs"
op = "cost.geo_correction"
inputs = {{ transition = "transition.transition" }}
params = {{ type = "c", scale = true }}

[[nodes]]
id = "for_walks"
op = "cost.geo_correction"
inputs = {{ transition = "transition.transition" }}
params = {{ type = "r", scale = true }}

[[nodes]]
id = "walk"
op = "cost.passage"
inputs = {{ transition = "for_walks.transition", origins = "south.map", goals = "north_east.map" }}

[[nodes]]
id = "forests"
op = "vector.read"
params = {{ path = "forests.geojson" }}

[[nodes]]
id = "least_cost"
op = "cost.distance"
inputs = {{ transition = "for_costs.transition", from = "forests.vector" }}

[[nodes]]
id = "commute"
op = "cost.commute_distance"
inputs = {{ transition = "for_walks.transition", points = "forests.vector" }}
"""
writes = {"walk.map": "walk.tif", "least_cost.table": "least_cost.csv", "commute.table": "commute.csv",
          "patch.map": "patch.tif"}
for k, t in enumerate(THETAS):
    MODEL += f"""
[[nodes]]
id = "rsp_{k}"
op = "cost.passage"
inputs = {{ transition = "for_costs.transition", origins = "south.map", goals = "north_east.map" }}
params = {{ theta = {t} }}

[[nodes]]
id = "rspd_{k}"
op = "cost.rsp_distance"
inputs = {{ transition = "for_costs.transition", from = "forests.vector" }}
params = {{ theta = {t} }}
"""
    writes[f"rsp_{k}.map"] = f"rsp_{k}.tif"
    writes[f"rspd_{k}.table"] = f"rspd_{k}.csv"
figs.run(figs.add_writes(MODEL, writes), work)
o = work / "output"

patch = np.ma.filled(figs.read(work / "patch.tif")[0], 0).astype(int)
two = (patch == 55) | (patch == 96)
groups = _clc.group_array(np.ma.filled(figs.read(work / "clc_100m.tif")[0], 0))


def passage_png(a, png):
    """Passages over the land cover, pale: log scale from 0.001 to 1, the two forests in green."""
    base = np.ones(a.shape + (4,))
    for i, (_, col, _) in enumerate(_clc.GROUPS, start=1):
        base[groups == i] = 0.22 * np.array(mc.to_rgba(col)) + 0.78
    v = np.ma.filled(np.abs(a).astype(float), 0)
    norm = mc.LogNorm(vmin=1e-3, vmax=1)
    cm = plt.get_cmap("inferno_r")
    rgba = cm(norm(np.clip(v, 1e-3, 1)))
    show = v >= 1e-3
    img = base.copy()
    img[show] = rgba[show]
    img[two] = mc.to_rgba(figs.FOREST)
    fig = plt.figure(figsize=(5.6, 5.6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_axis_off()
    ax.imshow(img, interpolation="nearest")
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


frames = []
walk = figs.read(work / "walk.tif")[0]
maps = [("walk", "Random walk", walk)] + [(f"rsp_{k}", f"theta = {t:g}", figs.read(work / f"rsp_{k}.tif")[0])
                                          for k, t in enumerate(THETAS)]
for name, label, a in maps:
    passage_png(a, out / f"{name}.png")
    v = np.abs(np.ma.filled(a, 0))
    v[two] = 0
    wide = int((v >= 0.01).sum())
    frames.append({"name": name, "label": label, "cells_001": wide})
    print(label, "cells with passage >= 0.01:", wide, "max outside forests:", round(float(v.max()), 3))

# the colour bar alone
fig = plt.figure(figsize=(6, 0.62), dpi=150)
cax = fig.add_axes([0.05, 0.55, 0.9, 0.25])
cb = fig.colorbar(plt.cm.ScalarMappable(norm=mc.LogNorm(1e-3, 1), cmap="inferno_r"), cax=cax, orientation="horizontal")
cb.outline.set_visible(False)
cb.ax.tick_params(labelsize=8.5)
cb.set_label("Net passages of a walker through the cell", fontsize=9, color=figs.INK2)
fig.savefig(out / "passage-scale.png", dpi=150, bbox_inches="tight", pad_inches=0.04)
plt.close(fig)

# distances between the five forests' points
names = [f["properties"]["name"] for f in json.loads((work / "forests.geojson").read_text())["features"]]


def matrix(path):
    rows = figs.read_csv(path)
    return np.array([[float(r[k]) for k in list(r.keys())[1:]] for r in rows])


lc = matrix(o / "least_cost.csv")
cd = matrix(o / "commute.csv")
rs = [matrix(o / f"rspd_{k}.csv") for k in range(len(THETAS))]
print("names", names)
print("least cost\n", lc.round(0))
print("commute\n", cd.round(0))
rows = []
for k, t in enumerate(THETAS):
    rows.append({"theta": f"θ = {t:g}", "rsp": round(float(rs[k][0, 1]), 1), "least_cost": round(float(lc[0, 1]), 1)})
    print("theta", t, "South -> North-east RSP", rs[k][0, 1].round(1), "least", lc[0, 1].round(1))
figs.write_csv(data / "rsp-distance.csv", rows)
iu = np.triu_indices(len(names), 1)


def ranks(v):
    return np.argsort(np.argsort(v))


for nm, m in (("commute", cd), ("rsp 1e-4", rs[1]), ("rsp 0.003", rs[4])):
    print("Spearman least cost vs", nm, round(float(np.corrcoef(ranks(lc[iu]), ranks(m[iu]))[0, 1]), 2))
pairs = []
for a, b in zip(*iu):
    pairs.append({"pair": f"{names[a]} – {names[b]}", "least_cost": round(float(lc[a, b])),
                  "commute": round(float(cd[a, b]) / 1e6, 2)})
for p in pairs:
    print(p)
figs.write_csv(data / "pairs.csv", pairs)
(work / "frames.json").write_text(json.dumps(frames, indent=1), encoding="utf-8")
