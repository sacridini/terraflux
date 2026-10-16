"""Figures of tutorials/distance-restoration/ecological-intactness: Beyer et al.'s intactness of Serra Verde's human
footprint, where restoring raises it most, and its sensitivity to the radius."""

import numpy as np

import _common as C
from _common import figs

SLUG = "distance-restoration/ecological-intactness"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
C.serra(work, "footprint", "land_cover")

MODEL = """format = "terraflux/1"

[[nodes]]
id = "footprint"
op = "raster.read"
params = { path = "footprint.tif" }

[[nodes]]
id = "pasture"
op = "raster.read"
params = { path = "land_cover.tif", band = "pasture" }

[[nodes]]
id = "cropland"
op = "raster.read"
params = { path = "land_cover.tif", band = "cropland" }

[[nodes]]
id = "reduction"
op = "map.calc"
inputs = { p = "pasture.raster", c = "cropland.raster" }
params = { expression = "if p + c > 0 then (4 * p + 7 * c) / (p + c) else 0" }

[[nodes]]
id = "intactness"
op = "restoration.intactness"
inputs = { footprint = "footprint.raster" }
params = { radius = RADIUS }

[[nodes]]
id = "gradient"
op = "restoration.intactness_gradient"
inputs = { footprint = "footprint.raster", reduction = "reduction.map" }
params = { radius = RADIUS }

[[nodes]]
id = "all_converted"
op = "map.calc"
inputs = { p = "pasture.raster", c = "cropland.raster" }
params = { expression = "p + c" }

[[nodes]]
id = "after"
op = "restoration.intactness"
inputs = { footprint = "footprint.raster", reduction = "reduction.map", restored = "all_converted.map" }
params = { radius = RADIUS }
"""


def model(radius=3):
    return MODEL.replace("RADIUS", str(radius))


figs.run(figs.add_writes(model(), {"footprint.raster": "footprint_out.tif", "reduction.map": "reduction.tif",
                                   "intactness.map": "intactness.tif", "gradient.map": "gradient.tif",
                                   "after.map": "after.tif", "all_converted.map": "converted.tif"}), work)
W = 760
hfi = figs.read(work / "footprint_out.tif")[0]
q = figs.read(work / "intactness.tif")[0]
g = figs.read(work / "gradient.tif")[0]
after = figs.read(work / "after.tif")[0]
conv = figs.read(work / "converted.tif")[0]
red = figs.read(work / "reduction.tif")[0]

figs.raster(work / "footprint_out.tif", out / "footprint.png", cmap="inferno_r", unit="human footprint (0-50)", width=W)
figs.raster(work / "intactness.tif", out / "intactness.png", cmap="viridis", vmin=0, vmax=1, unit="intactness",
            width=W)
figs.raster(work / "gradient.tif", out / "gradient.png", array=np.ma.masked_where(conv <= 0, g), cmap="magma",
            unit="gain per unit restored", width=W)
figs.raster(work / "reduction.tif", out / "reduction.png", array=np.ma.masked_where(conv <= 0, red), cmap="YlOrBr",
            unit="footprint removed", width=W)
figs.raster(work / "converted.tif", out / "converted.png", cmap="YlOrBr", vmin=0, vmax=1,
            unit="share of pasture + cropland", width=W)
# the same framing for the swipe: no colour bar
figs.raster(work / "intactness.tif", out / "compare_now.png", cmap="viridis", vmin=0, vmax=1, legend=False, width=W)
figs.raster(work / "after.tif", out / "compare_after.png", cmap="viridis", vmin=0, vmax=1, legend=False, width=W)

land = ~hfi.mask
print("mean intactness", float(q.mean()), "after", float(after.mean()), "sum", float(q.sum()), float(after.sum()))
print("cells with intactness > 0.5", float((q > 0.5).sum() / q.count()), "after", float((after > 0.5).sum() / after.count()))
print("gradient max", float(g.max()), "mean on converted", float(g[conv > 0].mean()))
print("footprint range", float(hfi.min()), float(hfi.max()), "mean", float(hfi.mean()))
print("converted km2", float(conv.sum()))

# the quality of habitat against the footprint, and the weight against the distance (the formulas)
gamma = -np.log(0.2) / 4


def draw_curves(ax):
    h = np.linspace(0, 20, 201)
    ax.plot(h, np.exp(-gamma * h), color=figs.FOREST, lw=2.2, label="quality q = exp(-γ h)")
    ax.plot(h, np.exp(-gamma * h) ** 0.5, color=figs.EMBER, lw=2.2, ls="--", label="q^z (z = 0.5)")
    ax.axvline(4, color=figs.INK2, lw=1, ls=":")
    ax.annotate("h = 4: q = 0.2", (4, 0.2), (6.5, 0.45), arrowprops=dict(arrowstyle="->", color=figs.INK2),
                fontsize=9.5, color=figs.INK2)
    ax.set_xlabel("human footprint h of a cell")
    ax.set_ylabel("habitat quality")
    ax.set_ylim(0, 1.03)
    ax.legend(loc="upper right")


figs.chart(out / "quality.png", draw_curves, width=760, height=380)

# ------------------------------------------------------------------ the radius
frames = []
for r in (1, 2, 3, 5, 8):
    w = figs.workdir(f"{SLUG}/r{r}")
    C.serra(w, "footprint", "land_cover")
    figs.run(figs.add_writes(model(r), {"intactness.map": "intactness.tif", "gradient.map": "gradient.tif"}), w)
    qq = figs.read(w / "intactness.tif")[0]
    gg = figs.read(w / "gradient.tif")[0]
    figs.raster(w / "intactness.tif", out / f"radius_{r}.png", cmap="viridis", vmin=0, vmax=1, unit="intactness",
                width=W)
    top = np.argsort(np.ma.masked_where(conv <= 0, gg).filled(-1).ravel())[::-1][:100]
    frames.append({"radius": r, "mean": round(float(qq.mean()), 4),
                   "above_half": round(float((qq > 0.5).sum() / qq.count() * 100), 2),
                   "top100": set(top.tolist())})
base = frames[2]["top100"]
rows = []
for f in frames:
    rows.append({"radius": f["radius"], "mean": f["mean"], "above_half": f["above_half"],
                 "shared_top100": len(f["top100"] & base)})
print("radius", rows)
figs.write_csv(data / "radius.csv", rows)
print("done")
