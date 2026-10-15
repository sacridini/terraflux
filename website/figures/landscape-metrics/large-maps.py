"""Numbers of "Large maps by blocks of rows": the class metrics of the CORINE map whole and by blocks of rows (the
same values), and a map nine times larger measured by blocks."""

import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _lm as L  # noqa: E402
from _lm import figs, np  # noqa: E402

SLUG = "landscape-metrics/large-maps"
w = L.work("large-maps")
dat = figs.data_dir(SLUG)

# the landscape level (metrics.class and metrics.patch by blocks give no map, and so fail: see the report)
MET = ["ta", "np", "lpi", "ed", "area_mn", "shape_mn", "ai", "cohesion", "core_mn", "enn_mn", "contag", "shdi"]
res = {}
times = []
for rows in (None, 512, 128):
    p = {"metrics": MET}
    if rows:
        p["block_rows"] = rows
    model = (L.HEAD + L.node("cover", "raster.read", params={"path": "clc.tif"})
             + L.node("land", "metrics.landscape", {"map": "cover.raster"}, p))
    t = time.time()
    L.run(model, w, {"land.table": f"blocks_{rows}.csv"}, name=f"blocks_{rows}.toml")
    dt = time.time() - t
    res[rows] = L.table(w, f"blocks_{rows}")
    times.append({"block_rows": rows or "whole map", "seconds": round(dt, 2)})
    print(rows, dt)
diff = 0.0
for rows in (512, 128):
    for a, b in zip(res[None], res[rows]):
        for m in a:
            if not (np.isnan(a[m]) and np.isnan(b[m])):
                diff = max(diff, abs(a[m] - b[m]) / max(abs(a[m]), 1e-12))
print("largest relative difference", diff)

# a map nine times larger (the CORINE map tiled 3 x 3: 6000 x 6000 cells, 36 million), measured by blocks
a, _ = figs.read(L.CLC)
big = np.tile(a.filled(0).astype("uint8"), (3, 3))
L.write_tif(w / "clc_3x3.tif", big, res=25.0, crs="EPSG:3035")
model = (L.HEAD + L.node("cover", "raster.read", params={"path": "clc_3x3.tif"})
         + L.node("land", "metrics.landscape", {"map": "cover.raster"},
                  {"metrics": ["np", "ed", "ai", "shdi"], "block_rows": 512}))
t = time.time()
L.run(model, w, {"land.table": "big.csv"}, name="big.toml")
print(L.table(w, "big"))
dt = time.time() - t
times.append({"block_rows": "512 (map 3 x 3, 36 million cells)", "seconds": round(dt, 2)})
print("big", dt)
figs.write_csv(dat / "times.csv", times)
for t in times:
    print(t)

# the moving window by strips of rows: its maps written to a GeoTIFF (a band per metric)
model = (L.HEAD + L.node("cover", "raster.read", params={"path": "clc.tif"})
         + L.node("window", "metrics.window", {"map": "cover.raster"},
                  {"metrics": ["shdi", "ed"], "window": 9, "block_rows": 256, "path": "clc_window9.tif"}))
t = time.time()
L.run(model, w, {}, name="window.toml")
print("window by strips", round(time.time() - t, 2), figs.bands(w / "clc_window9.tif"),
      figs.read(w / "clc_window9.tif")[1]["descriptions"])

# ---------------------------------------------------------------- figures: how a map is read by blocks of rows
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Rectangle  # noqa: E402

img = figs.img_dir(SLUG)
rng = np.random.default_rng(4)
toy = np.ones((24, 36), dtype=int)
# a forest that runs across the three blocks, and a few small woods
toy[3:21, 14:19] = 2
toy[6:9, 8:24] = 2
toy[15:18, 19:30] = 2
for (r, c) in ((2, 3), (11, 30), (19, 5), (13, 6)):
    toy[r:r + 2, c:c + 3] = 2
fig, axes = plt.subplots(1, 2, figsize=(10.4, 4.2), dpi=150)
for ax, title in zip(axes, ("Metrics by blocks of rows", "A moving window by strips")):
    rgb = np.array([mcolors.to_rgb("#ece6d6" if v == 1 else figs.FOREST) for v in toy.ravel()]).reshape(toy.shape + (3,))
    ax.imshow(rgb, interpolation="nearest")
    ax.set_xticks([])
    ax.set_yticks([])
    for s in ax.spines.values():
        s.set_visible(False)
    ax.set_title(title, loc="left", fontsize=11)
    for k, y in enumerate((-0.5, 7.5, 15.5)):
        ax.add_patch(Rectangle((-0.5, y), 36, 8, fill=False, ec=figs.INK, lw=1.6))
        if ax is axes[0]:
            ax.text(36.2, y + 4, f"block {k + 1}", va="center", fontsize=9, color=figs.INK)
axes[0].text(16.5, 11.5, "one patch\nin 3 blocks", ha="center", va="center", fontsize=8.5, color="white",
             fontweight="bold")
# the window: strip 2 with a halo of half a window above and below
axes[1].add_patch(Rectangle((-0.5, 7.5 - 2), 36, 12, fc=figs.EMBER, alpha=0.25, ec=figs.EMBER, lw=1.4, ls="--"))
axes[1].add_patch(Rectangle((9.5, 12.5 - 2), 5, 5, fill=False, ec="#b03a2e", lw=1.6))
axes[1].text(36.2, 11.5, "strip 2\n+ halo of\nhalf a\nwindow", va="center", fontsize=9, color=figs.INK)
fig.tight_layout()
fig.savefig(img / "blocks.png", dpi=150)
plt.close(fig)
figs._shrink(img / "blocks.png")

# the moving window made by strips, its first band (Shannon's diversity)
figs.raster(w / "clc_window9.tif", img / "window9.png", band=1, cmap="viridis", vmin=0, vmax=1.8,
            unit="Shannon diversity (SHDI), 9 x 9 cells of 25 m", width=760)
