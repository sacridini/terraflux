"""Figures of "Connectivity as a graph" (habitat networks, part 1): the land cover, its forest patches as the
nodes of a graph, the links at several distances and the components they make, from real runs of terraflux."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _clc  # noqa: E402
from _clc import figs, np, plt  # noqa: E402

SLUG = "habitat-networks/connectivity-as-a-graph"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)

DISTANCES = [250, 500, 1000, 2000, 5000, 10000]

MODEL = """format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "land.map" }
params = { habitat = [23, 24, 25], area_unit = "ha", distance = "edge", landscape_area = 250000 }

[[nodes]]
id = "overall"
op = "conn.overall"
inputs = { graph = "graph.graph" }
params = { distance = [250, 500, 1000, 2000, 5000, 10000], metrics = ["A", "NC", "NL", "H"] }

[[nodes]]
id = "ids"
op = "conn.node_importance"
inputs = { graph = "graph.graph" }
params = { distance = 1000, metrics = ["A"] }

[[nodes]]
id = "patches"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "ids.table" }
params = { column = "id" }

[[nodes]]
id = "pairs"
op = "conn.distances"
inputs = { map = "land.map" }
params = { habitat = [23, 24, 25], distance = "edge" }

[[nodes]]
id = "areas"
op = "conn.nodes"
inputs = { map = "land.map" }
params = { habitat = [23, 24, 25], area_unit = "ha" }
"""

writes = {"patches.map": "patches.tif"}
for d in (500, 1000, 2000, 5000):
    MODEL += f"""
[[nodes]]
id = "comp_{d}"
op = "conn.components"
inputs = {{ graph = "graph.graph" }}
params = {{ distance = {d} }}

[[nodes]]
id = "compmap_{d}"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "comp_{d}.table" }}
params = {{ column = "component" }}
"""
    writes[f"compmap_{d}.map"] = f"components_{d}.tif"
writes.update({"overall.table": "overall.csv", "pairs.table": "pairs.csv", "areas.table": "areas.csv"})
figs.run(figs.add_writes(MODEL, writes), work)

land = figs.read(work / "clc_100m.tif")[0]
groups = _clc.group_array(np.ma.filled(land, 0))
ids = figs.read(work / "patches.tif")[0]
pairs = figs.read_csv(work / "output" / "pairs.csv")
areas = {int(r["id"]): float(r["attribute"]) for r in figs.read_csv(work / "output" / "areas.csv")}
overall = figs.read_csv(work / "output" / "overall.csv")

# ---------------------------------------------------------------- the land cover
figs.raster(None, out / "landcover.png", array=groups.astype(float), palette=_clc.group_palette(), counts=True,
            width=980)

# ---------------------------------------------------------------- the patches, by area
area_map = np.full(ids.shape, np.nan)
filled = np.ma.filled(ids, 0).astype(int)
for i, a in areas.items():
    area_map[filled == i] = a
_clc.network_map(out / "patches.png", ids, values=area_map, cmap="YlGn", log=True, vmin=1, unit="Area of the patch (ha)",
                 width=900)

# ---------------------------------------------------------------- the links at 1 km
links_1k = [(int(r["from"]), int(r["to"])) for r in pairs if float(r["distance"]) <= 1000]
_clc.network_map(out / "graph-1km.png", ids, links=links_1k, width=900, node_color="#7fb48f")

# ---------------------------------------------------------------- components at several distances
def comp_png(d):
    c = figs.read(work / f"components_{d}.tif")[0]
    cf = np.ma.filled(c, 0).astype(int)
    sizes = np.bincount(cf.ravel())
    sizes[0] = 0
    order = np.argsort(-sizes)
    rgba = np.zeros(c.shape + (4,))
    from matplotlib import colors as mc
    rgba[:] = mc.to_rgba(_clc.MATRIX)
    palette = figs.CLASSES
    rank = 0
    for comp in order:
        if sizes[comp] == 0:
            continue
        members = np.unique(filled[cf == comp])
        if len(members) == 1:
            col = "#9aa39d"
        else:
            col = palette[rank % len(palette)]
            rank += 1
        rgba[cf == comp] = mc.to_rgba(col)
    fig = plt.figure(figsize=(4, 4), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_axis_off()
    ax.imshow(rgba, interpolation="nearest")
    links = [(int(r["from"]), int(r["to"])) for r in pairs if float(r["distance"]) <= d]
    cen = _clc.centroids(ids)
    from matplotlib.collections import LineCollection
    segs = [[cen[i], cen[j]] for i, j in links]
    ax.add_collection(LineCollection(segs, linewidths=0.35, colors=[(0.11, 0.14, 0.13, 0.55)]))
    png = out / f"components-{d}.png"
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


for d in (500, 1000, 2000, 5000):
    comp_png(d)

# ---------------------------------------------------------------- the numbers of the graph at each distance
rows = []
for r in overall:
    d = int(float(r["distance"]))
    linked = {int(p["from"]) for p in pairs if float(p["distance"]) <= d} | {int(p["to"]) for p in pairs if float(p["distance"]) <= d}
    rows.append({"distance": f"{d / 1000:g} km" if d >= 1000 else f"{d} m", "NC": int(float(r["NC"])),
                 "alone": len(areas) - len(linked), "NL": int(float(r["NL"])), "A": round(float(r["A"]))})
figs.write_csv(data / "graph-numbers.csv", rows)
print("patches:", len(areas), "forest (ha):", round(sum(areas.values())))
for r in rows:
    print(r)
big = sorted(areas.values(), reverse=True)
print("largest patches:", big[:5], "median:", np.median(list(areas.values())), "patches <= 5 ha:",
      sum(a <= 5 for a in areas.values()))

# ---------------------------------------------------------------- kernels (a formula, drawn)
krows = []
for dist in range(0, 6001, 100):
    r = {"distance": dist}
    for d50 in (500, 1000, 2000):
        r[f"d{d50}"] = round(float(np.exp(np.log(0.5) / d50 * dist)), 4)
    krows.append(r)
figs.write_csv(data / "kernels.csv", krows)

# ---------------------------------------------------------------- structural and functional: a drawing
rng = np.random.default_rng(4)
pts = np.array([[1.0, 1.2], [2.2, 1.6], [3.0, 0.6], [4.4, 1.4], [5.6, 2.4], [2.0, 3.2], [4.0, 3.4], [6.0, 0.8]])
rad = np.array([0.42, 0.28, 0.2, 0.35, 0.3, 0.38, 0.18, 0.24])


def drawing(ax, thr, title):
    ax.set_aspect("equal")
    ax.set_xlim(0.2, 6.8)
    ax.set_ylim(-0.1, 4.1)
    ax.set_axis_off()
    for i in range(len(pts)):
        for j in range(i + 1, len(pts)):
            gap = np.hypot(*(pts[i] - pts[j])) - rad[i] - rad[j]
            if gap <= thr:
                ax.plot(*zip(pts[i], pts[j]), color=figs.INK, lw=1.6, zorder=1)
    for (x, y), r in zip(pts, rad):
        ax.add_patch(plt.Circle((x, y), r, color=figs.FOREST, zorder=2))
    ax.set_title(title, loc="left", fontsize=10.5)


fig, axs = plt.subplots(1, 2, figsize=(9, 3.1), dpi=150)
drawing(axs[0], 0.75, "A beetle that crosses 750 m of fields")
drawing(axs[1], 1.6, "A marten that crosses 1.6 km")
for ax in axs:
    ax.plot([0.4, 1.4], [-0.02, -0.02], color=figs.INK2, lw=2)
    ax.text(0.9, 0.05, "1 km", ha="center", va="bottom", fontsize=8.5, color=figs.INK2)
fig.tight_layout()
fig.savefig(out / "structural-functional.png", dpi=150)
plt.close(fig)
figs._shrink(out / "structural-functional.png")
