"""Figures of tutorials/distance-restoration/costs-and-corridors: resistance, accumulated cost, least-cost paths and
the connectivity a corridor restores (GuidosToolbox's Restoration Planner), in a forest of south-western Oregon."""

import itertools

import numpy as np
from matplotlib import pyplot as plt
from matplotlib.patches import Patch

import _common as C
from _common import figs

SLUG = "distance-restoration/costs-and-corridors"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
C.copy(work, (C.OREGON / "orforest.tif", "ndvi.tif"))

BASE = """format = "terraflux/1"

[[nodes]]
id = "ndvi"
op = "raster.read"
params = { path = "ndvi.tif" }

[[nodes]]
id = "projected"
op = "raster.reproject"
inputs = { raster = "ndvi.raster" }
params = { crs = "EPSG:32610", resolution = 30 }

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { n = "projected.raster" }
params = { expression = "0 if isnull(n) else 2 if n >= 0.7 else 1" }

[[nodes]]
id = "objects"
op = "landscape.accounting"
inputs = { map = "forest.map" }

[[nodes]]
id = "status"
op = "restoration.status"
inputs = { map = "forest.map" }

[[nodes]]
id = "resistance"
op = "restoration.resistance"
inputs = { map = "forest.map" }
"""

PAIR = """
[[nodes]]
id = "a"
op = "raster.calc"
inputs = { ids = "objects.ids" }
params = { expression = "ids == A" }

[[nodes]]
id = "b"
op = "raster.calc"
inputs = { ids = "objects.ids" }
params = { expression = "ids == B" }

[[nodes]]
id = "cost_a"
op = "restoration.cost_distance"
inputs = { resistance = "resistance.map", sources = "a.map" }

[[nodes]]
id = "path"
op = "restoration.least_cost_path"
inputs = { resistance = "resistance.map", a = "a.map", b = "b.map" }

[[nodes]]
id = "corridor"
op = "restoration.corridor"
inputs = { resistance = "resistance.map", a = "a.map", b = "b.map", habitat = "forest.map" }

[[nodes]]
id = "marker"
op = "raster.calc"
inputs = { f = "forest.map", a = "a.map", b = "b.map" }
params = { expression = "3 if f == 0 else 1 if a else 2 if b else 0" }

[[nodes]]
id = "isochrone"
op = "restoration.isochrone"
inputs = { resistance = "resistance.map", marker = "marker.map" }

"""

# the three largest objects, from a first run
figs.run(figs.add_writes(BASE, {"objects.objects": "objects.csv", "status.stats": "status.csv",
                                "forest.map": "forest.tif", "resistance.map": "resistance.tif",
                                "objects.ids": "ids.tif"}), work)
objs = sorted(C.table(work, "objects"), key=lambda r: -r["cells"])
largest = [int(r["id"]) for r in objs[:3]]
print("largest objects", [(int(r["id"]), int(r["cells"])) for r in objs[:3]])
status = C.table(work, "status")[0]
print("status", status)
figs.write_csv(data / "status.csv", [{k: round(v, 2) for k, v in status.items()}])

forest = figs.read(work / "forest.tif")[0]
res = figs.read(work / "resistance.tif")[0]
ids = figs.read(work / "ids.tif")[0]
W = 620
figs.raster(work / "resistance.tif", out / "resistance.png", array=np.ma.masked_where(forest == 0, res),
            cmap="YlOrBr", unit="resistance", width=W)

LETTERS = {largest[0]: "A", largest[1]: "B", largest[2]: "C"}


def base_rgb(highlight=()):
    img = np.zeros(forest.shape + (4,))
    img[...] = (0.914, 0.882, 0.788, 1)
    f = forest.filled(0)
    img[f == 2] = (0.62, 0.77, 0.66, 1)
    img[f == 0] = (1, 1, 1, 1)
    for k in highlight:
        img[ids.filled(0) == k] = (0.13, 0.4, 0.25, 1)
    return img


def save_map(img, png, handles=None, overlay=None):
    fig = plt.figure(figsize=(W / 150, W / 150 * 370 / 272 * 0.78), dpi=150)
    ax = fig.add_axes([0, 0, 0.72, 1])
    ax.imshow(img, interpolation="nearest")
    if overlay is not None:
        overlay(ax)
    ax.set_axis_off()
    if handles:
        fig.legend(handles=handles, loc="center left", bbox_to_anchor=(0.73, 0.5), fontsize=8.5, handlelength=1.1)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.05)
    plt.close(fig)
    figs._shrink(png)


def label_objects(ax, which):
    for k in which:
        yy, xx = np.nonzero(ids.filled(0) == k)
        ax.text(xx.mean(), yy.mean(), LETTERS[k], ha="center", va="center", fontsize=13, fontweight="bold",
                color="white", bbox=dict(boxstyle="circle,pad=0.25", fc="#1c2420", ec="none", alpha=0.8))


save_map(base_rgb(largest), out / "objects.png",
         [Patch(color=(0.13, 0.4, 0.25), label="The three largest\nforest objects"),
          Patch(color=(0.62, 0.77, 0.66), label="Other forest"), Patch(color=(0.914, 0.882, 0.788), label="Open land")],
         overlay=lambda ax: label_objects(ax, largest))

rows = []
for i, (p, q) in enumerate(itertools.combinations(largest, 2)):
    w = figs.workdir(f"{SLUG}/pair{i}")
    C.copy(w, (C.OREGON / "orforest.tif", "ndvi.tif"))
    model = BASE + PAIR.replace("ids == A", f"ids == {p}").replace("ids == B", f"ids == {q}")
    (work / f"pair{i}.toml").write_text(model, encoding="utf-8")
    figs.run(figs.add_writes(model, {"cost_a.map": "cost_a.tif", "path.path": "path.tif", "path.cost": "path_cost.tif",
                                     "path.stats": "path.csv", "corridor.stats": "corridor.csv",
                                     "isochrone.map": "iso.tif", "isochrone.cost": "iso_cost.tif"}), w)
    ps = C.table(w, "path")[0]
    cs = C.table(w, "corridor")[0]
    pair = f"{LETTERS[p]}-{LETTERS[q]}"
    print(pair, "path", ps, "\n   corridor", cs)
    rows.append({"pair": pair, "restored": int(cs["restored"]), "expense": round(cs["expense"], 1),
                 "delta_coherence": round(cs["delta_coherence"], 2), "delta_eca": round(cs["delta_eca"], 1),
                 "efficiency": round(cs["efficiency"], 3), "length": round(ps.get("length", float("nan")), 1)})
    path = figs.read(w / "path.tif")[0].filled(0) > 0
    img = base_rgb((p, q))
    def thick(m, r=1):
        o = m.copy()
        for dy in range(-r, r + 1):
            for dx in range(-r, r + 1):
                o |= np.roll(np.roll(m, dy, 0), dx, 1)
        return o

    img[thick(path, 2)] = (0.11, 0.14, 0.13, 1)
    img[thick(path & (forest.filled(0) == 2))] = (0.95, 0.6, 0.1, 1)
    img[thick(path & (forest.filled(0) == 1))] = (0.85, 0.2, 0.1, 1)
    save_map(img, out / f"corridor_{pair}.png",
             [Patch(color=(0.85, 0.2, 0.1), label="Path: open land\n(to restore)"),
              Patch(color=(0.95, 0.6, 0.1), label="Path: forest"),
              Patch(color=(0.13, 0.4, 0.25), label="The two objects"),
              Patch(color=(0.62, 0.77, 0.66), label="Other forest")],
             overlay=lambda ax, p=p, q=q: label_objects(ax, (p, q)))
    if i == 0:
        ca = figs.read(w / "cost_a.tif")[0]
        figs.raster(w / "cost_a.tif", out / "cost_a.png", array=np.ma.masked_where(forest == 0, ca), cmap="viridis",
                    unit="accumulated cost from A", width=W)
        pc = figs.read(w / "path_cost.tif")[0]
        lo = float(pc.min())
        figs.raster(w / "path_cost.tif", out / "path_cost.png", array=np.ma.masked_where(forest == 0, pc - lo),
                    cmap="magma", vmax=float(np.percentile((pc - lo).compressed(), 60)),
                    unit="cost above the least", width=W)
figs.write_csv(data / "corridors.csv", rows)

# ------------------------------------------------------------------ the toy: accumulated cost by Dijkstra
R = np.array([[2, 2, 10, 40, 40, 10],
              [2, 3, 20, 60, 30, 5],
              [3, 5, 30, 80, 10, 3],
              [5, 10, 20, 20, 5, 2],
              [10, 30, 40, 10, 3, 2]], float)
src = (0, 0)
cost = np.full(R.shape, np.inf)
cost[src] = 0
done = np.zeros(R.shape, bool)
while not done.all():
    c = np.where(done, np.inf, cost)
    y, x = np.unravel_index(np.argmin(c), c.shape)
    done[y, x] = True
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            ny, nx = y + dy, x + dx
            if (dy or dx) and 0 <= ny < R.shape[0] and 0 <= nx < R.shape[1] and not done[ny, nx]:
                step = (R[y, x] + R[ny, nx]) / 2 * (np.sqrt(2) if dy and dx else 1)
                cost[ny, nx] = min(cost[ny, nx], cost[y, x] + step)
# the cheapest path back from the far corner
path = [(4, 5)]
while path[-1] != src:
    y, x = path[-1]
    best = None
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            ny, nx = y + dy, x + dx
            if (dy or dx) and 0 <= ny < R.shape[0] and 0 <= nx < R.shape[1]:
                step = (R[y, x] + R[ny, nx]) / 2 * (np.sqrt(2) if dy and dx else 1)
                if abs(cost[ny, nx] + step - cost[y, x]) < 1e-9:
                    best = (ny, nx)
    path.append(best)


def draw_toy(axs):
    for ax, (vals, title, cm) in zip(axs, ((R, "Resistance of each cell", "YlOrBr"),
                                            (cost, "Accumulated cost from the top-left cell", "viridis"))):
        ax.imshow(vals, cmap=cm, interpolation="nearest")
        for (y, x), v in np.ndenumerate(vals):
            ax.text(x, y, f"{v:.1f}".rstrip("0").rstrip("."), ha="center", va="center", fontsize=9,
                    color="white" if (cm == "viridis" and v < cost.max() * 0.6) or (cm == "YlOrBr" and v > 30) else figs.INK)
        ax.set_title(title, fontsize=10.5, loc="left")
        ax.set_xticks([])
        ax.set_yticks([])
        ax.grid(False)
    py, px = zip(*path)
    axs[1].plot(px, py, color="#e8901f", lw=2.5, alpha=0.9)
    axs[0].plot(px, py, color="#2f6f9f", lw=2.5, alpha=0.9)


fig, axs = plt.subplots(1, 2, figsize=(1000 / 150, 340 / 150), dpi=150)
draw_toy(axs)
fig.tight_layout()
fig.savefig(out / "toy.png", dpi=150)
plt.close(fig)
figs._shrink(out / "toy.png")
print("toy cost", cost.round(1), path)
print("done")
