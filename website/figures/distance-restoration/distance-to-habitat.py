"""Figures of tutorials/distance-restoration/distance-to-habitat: the distance to a forest in south-western Oregon."""

import numpy as np
from matplotlib import pyplot as plt
from matplotlib.colors import ListedColormap

import _common as C
from _common import figs

SLUG = "distance-restoration/distance-to-habitat"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
C.copy(work, (C.OREGON / "orforest.tif", "ndvi.tif"))

MODEL = """format = "terraflux/1"

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
op = "map.calc"
inputs = { n = "projected.raster" }
params = { expression = "if isnull(n) then 0 else if n >= THRESHOLD then 2 else 1" }

[[nodes]]
id = "distance"
op = "distance.euclidean"
inputs = { map = "forest.map" }

[[nodes]]
id = "to_forest"
op = "distance.proximity"
inputs = { raster = "forest.map" }
params = { values = [2] }

[[nodes]]
id = "zones"
op = "distance.influence_zones"
inputs = { map = "forest.map" }
params = { min_area = MINAREA }
"""


def model(threshold=0.7, min_area=1000):
    return MODEL.replace("THRESHOLD", str(threshold)).replace("MINAREA", str(min_area))


# ------------------------------------------------------------------ the main run
m = figs.add_writes(model(), {"projected.raster": "ndvi_utm.tif", "forest.map": "forest.tif",
                              "distance.map": "distance.tif", "to_forest.raster": "to_forest.tif",
                              "zones.zones": "zones.tif", "zones.ids": "zone_ids.tif",
                              "distance.stats": "dstats.csv", "zones.objects": "zobjects.csv"})
figs.run(m, work)

ndvi = figs.read(work / "ndvi_utm.tif")[0]
forest = figs.read(work / "forest.tif")[0]
dist = figs.read(work / "distance.tif")[0]
prox = figs.read(work / "to_forest.tif")[0]
W = 620

figs.raster(work / "ndvi_utm.tif", out / "ndvi.png", cmap="YlGn", vmin=0.1, vmax=0.85, unit="NDVI", width=W)
figs.raster(work / "forest.tif", out / "forest.png", palette=C.GTB, width=W, counts=True)

# the signed distance in metres (30 m cells), forest positive
signed = np.ma.masked_where(forest == 0, dist * 30)
figs.raster(work / "distance.tif", out / "signed.png", array=signed, cmap="PiYG", diverging=True,
            unit="metres (+ forest, - open land)", width=W)
figs.raster(work / "to_forest.tif", out / "to_forest.png", array=np.ma.masked_where(forest == 0, prox),
            cmap="magma_r", unit="metres to forest", width=W)

# how much of each side lies within a distance of the edge
inside = (dist[forest == 2] * 30).compressed()
outside = (-dist[forest == 1] * 30).compressed()
rows = []
for d in range(0, 421, 30):
    rows.append({"distance": d,
                 "open": round(float((outside <= d).mean() * 100), 2),
                 "forest": round(float((inside <= d).mean() * 100), 2)})
figs.write_csv(data / "reach.csv", rows)
stats = C.table(work, "dstats")
print("distance stats", stats)
print("open within 60 m", next(r for r in rows if r["distance"] == 60))

# the zones of influence
zones = figs.read(work / "zones.tif")[0]
ids = figs.read(work / "zone_ids.tif")[0]
rng = np.random.default_rng(4)
nz = int(np.nanmax(zones))
cols = plt.get_cmap("tab20")(np.arange(20))
lut = np.vstack([[1, 1, 1, 1]] + [cols[(k * 7) % 20] for k in range(nz)])
lut[1:, :3] = lut[1:, :3] * 0.45 + 0.55          # pale zones
img = lut[np.nan_to_num(zones.filled(0)).astype(int)]
obj = (ids.filled(-9) > 0) & (forest.filled(0) == 2)
img[obj] = [0.16, 0.42, 0.27, 1]
img[(forest.filled(0) == 2) & ~obj] = [0.55, 0.72, 0.6, 1]
img[zones.filled(-1) == 0] = [0.1, 0.1, 0.1, 1]
img[forest.filled(0) == 0] = [1, 1, 1, 1]
fig = plt.figure(figsize=(W / 150, W / 150 * 370 / 272 * 0.78), dpi=150)
ax = fig.add_axes([0, 0, 0.72, 1])
ax.imshow(img, interpolation="nearest")
ax.set_axis_off()
from matplotlib.patches import Patch  # noqa: E402

fig.legend(handles=[Patch(color=(0.16, 0.42, 0.27), label="Object (1000+ cells)"),
                    Patch(color=(0.55, 0.72, 0.6), label="Smaller forest"),
                    Patch(color=(0.8, 0.8, 0.8), label="Zone (a colour each)"),
                    Patch(color=(0.1, 0.1, 0.1), label="Line between zones")],
           loc="center left", bbox_to_anchor=(0.73, 0.5), fontsize=8.5, handlelength=1.1)
fig.savefig(out / "zones.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
plt.close(fig)
zobj = C.table(work, "zobjects")
print("zones objects", len(zobj), sorted(int(r["area"]) for r in zobj))

# ------------------------------------------------------------------ the threshold of the forest
frames = []
for t in (0.6, 0.65, 0.7, 0.75):
    w = figs.workdir(f"{SLUG}/t{t}")
    C.copy(w, (C.OREGON / "orforest.tif", "ndvi.tif"))
    no_zones = model(t).split('[[nodes]]\nid = "zones"')[0]
    figs.run(figs.add_writes(no_zones, {"forest.map": "forest.tif", "distance.map": "distance.tif",
                                        "distance.stats": "dstats.csv"}), w)
    f = figs.read(w / "forest.tif")[0]
    d = figs.read(w / "distance.tif")[0]
    s = np.ma.masked_where(f == 0, d * 30)
    figs.raster(w / "distance.tif", out / f"signed_{t}.png", array=s, cmap="PiYG", diverging=True,
                vmin=-600, vmax=600, unit="metres", width=W)
    st = {r["foreground"]: r for r in C.table(w, "dstats")}
    frames.append({"t": t, "forest": float((f == 2).sum() / (f > 0).sum() * 100),
                   "open_mean": st[0]["mean"] * 30, "open_max": st[0]["max"] * 30,
                   "forest_mean": st[1]["mean"] * 30})
print("thresholds", frames)
figs.write_csv(data / "thresholds.csv", [{k: round(v, 2) for k, v in fr.items()} for fr in frames])

# ------------------------------------------------------------------ the toy: a signed distance cell by cell
toy = np.zeros((8, 10), int)
toy[1:6, 1:5] = 1
toy[2:7, 6:9] = 1
toy[4, 5] = 1
yy, xx = np.mgrid[0:8, 0:10]
fg = np.argwhere(toy == 1)
bg = np.argwhere(toy == 0)
sd = np.zeros(toy.shape)
for y in range(8):
    for x in range(10):
        other = bg if toy[y, x] == 1 else fg
        d = np.sqrt(((other - [y, x]) ** 2).sum(1)).min()
        sd[y, x] = d if toy[y, x] == 1 else -d


def draw_toy(ax):
    ax.imshow(toy, cmap=ListedColormap([C.OPEN, C.FOREST]), interpolation="nearest")
    for y in range(8):
        for x in range(10):
            v = sd[y, x]
            ax.text(x, y, f"{v:.1f}".rstrip("0").rstrip("."),
                    ha="center", va="center", fontsize=8.5, color="white" if toy[y, x] else figs.INK)
    ax.set_xticks(np.arange(-0.5, 10, 1), minor=True)
    ax.set_yticks(np.arange(-0.5, 8, 1), minor=True)
    ax.grid(which="minor", color="white", linewidth=1.2, linestyle="-")
    ax.grid(which="major", visible=False)
    ax.tick_params(which="both", length=0, labelbottom=False, labelleft=False)
    for s in ax.spines.values():
        s.set_visible(False)


figs.chart(out / "toy.png", draw_toy, width=700, height=540)
print("done")
