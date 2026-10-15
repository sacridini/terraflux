"""Figures of 'Rasters larger than memory': diagrams of strips, halos and overlapping tiles; landscape metrics of a
map of 64 million cells read whole and by blocks of rows (the same values, the memory measured); MSPA by
GuidosToolbox's overlapping tiles against MSPA of the whole map (where they differ, with the overlap)."""

import pathlib
import shutil
import subprocess
import sys
import time

import psutil
import rasterio
from matplotlib.patches import Rectangle
from rasterio.windows import Window

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[0] / "getting-started"))
from _common import figs, np, plt, CLC, save  # noqa: E402

SLUG = "large-data/larger-than-memory"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

# ------------------------------------------------------------------ diagrams
fig, axs = plt.subplots(1, 3, figsize=(10.5, 3.9), dpi=150)
H, W = 24, 18
for ax in axs:
    ax.set_xlim(-0.5, W + 0.5)
    ax.set_ylim(H + 0.5, -2.5)
    ax.set_aspect("equal")
    ax.set_axis_off()
    ax.add_patch(Rectangle((0, 0), W, H, fc="#eef2ee", ec=figs.INK2, lw=1))
# 1. strips of rows
ax = axs[0]
ax.set_title("1. Strips of rows (block_rows)", loc="left", fontsize=10)
for k, y in enumerate(range(0, H, 6)):
    ax.add_patch(Rectangle((0, y), W, 6, fc=figs.FOREST if k == 1 else "none", alpha=0.35 if k == 1 else 1,
                           ec=figs.INK2, lw=0.8, ls="--"))
ax.text(W / 2, 9, "in memory now", ha="center", va="center", fontsize=8.5, color=figs.INK)
ax.text(W / 2, -1.2, "sums per class added strip by strip", ha="center", fontsize=8, color=figs.INK2)
# 2. strips with a halo
ax = axs[1]
ax.set_title("2. Strips with a halo (windows)", loc="left", fontsize=10)
for y in range(0, H, 6):
    ax.add_patch(Rectangle((0, y), W, 6, fc="none", ec=figs.INK2, lw=0.8, ls="--"))
ax.add_patch(Rectangle((0, 4), W, 10, fc=figs.EMBER, alpha=0.25, ec="none"))
ax.add_patch(Rectangle((0, 6), W, 6, fc=figs.FOREST, alpha=0.4, ec="none"))
ax.annotate("", (W + 0.3, 4), (W + 0.3, 6), arrowprops=dict(arrowstyle="<->", color=figs.EMBER, lw=1))
ax.text(W / 2, 9, "strip computed", ha="center", va="center", fontsize=8.5)
ax.text(W / 2, 4.9, "halo: half a window", ha="center", va="center", fontsize=7.5, color="#9a5a10")
ax.text(W / 2, 13, "halo", ha="center", va="center", fontsize=7.5, color="#9a5a10")
ax.text(W / 2, -1.2, "every window sees all its cells", ha="center", fontsize=8, color=figs.INK2)
# 3. overlapping tiles
ax = axs[2]
ax.set_title("3. Overlapping tiles (MSPA)", loc="left", fontsize=10)
ax.add_patch(Rectangle((0, 0), 11, 13, fc=figs.FOREST, alpha=0.18, ec=figs.FOREST, lw=1.4))
ax.add_patch(Rectangle((7, 0), 11, 13, fc=figs.WATER, alpha=0.18, ec=figs.WATER, lw=1.4))
ax.add_patch(Rectangle((0, 9), 11, 15, fc=figs.EMBER, alpha=0.12, ec=figs.EMBER, lw=1.4))
ax.plot([9, 9], [0, 13], color=figs.INK, lw=1.2, ls=":")
ax.text(3.5, 5, "tile 1", fontsize=8.5)
ax.text(13, 5, "tile 2", fontsize=8.5)
ax.text(3.5, 18, "tile 3", fontsize=8.5)
ax.text(9, -1.2, "overlap: each keeps its nearer half", ha="center", fontsize=8, color=figs.INK2)
fig.tight_layout()
save(fig, out / "strategies.png")

# ------------------------------------------------------------------ a map of 64 million cells
with rasterio.open(CLC) as src:
    a = src.read(1)
    prof = src.profile
n = 4
prof.update(width=a.shape[1] * n, height=a.shape[0] * n, tiled=True, blockxsize=256, blockysize=256,
            compress="lzw", transform=rasterio.Affine(25, 0, prof["transform"].c, 0, -25, prof["transform"].f))
with rasterio.open(work / "big.tif", "w", **prof) as dst:
    for i in range(n):
        for j in range(n):
            tile = a if (i + j) % 2 == 0 else a[:, ::-1]
            dst.write(tile, 1, window=Window(j * a.shape[1], i * a.shape[0], a.shape[1], a.shape[0]))
print("big.tif", (work / "big.tif").stat().st_size / 1e6, "MB on disk")

FORESTS = "23..25 -> 2\\n* -> 1"


def metrics_model(port, block):
    extra = f", block_rows = {block}" if block else ""
    return f'''format = "terraflux/1"
name = "Landscape metrics of a large map"

[[nodes]]
id = "landcover"
op = "raster.read"
params = {{ path = "big.tif" }}

[[nodes]]
id = "metrics"
op = "metrics.landscape"
inputs = {{ map = "landcover.{port}" }}
params = {{ metrics = ["ta", "np", "lpi", "ed", "area_mn", "shdi", "contag", "ai"]{extra} }}

[outputs.metrics]
from = "metrics.table"
'''


def measured(model_text, name):
    f = work / f"{name}.toml"
    f.write_text(model_text, encoding="utf-8")
    t0 = time.time()
    p = subprocess.Popen([str(figs.binary()), "run", str(f), "--out", str(work / name)], cwd=work,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    proc = psutil.Process(p.pid)
    peak = 0
    while p.poll() is None:
        try:
            peak = max(peak, proc.memory_info().rss)
        except psutil.Error:
            pass
        time.sleep(0.02)
    outp = p.communicate()
    if p.returncode:
        raise SystemExit(outp)
    return time.time() - t0, peak / 2**20


runs = []
for name, port, block in (("whole", "map", None), ("blocks_512", "raster", 512), ("blocks_128", "raster", 128)):
    secs, mib = measured(metrics_model(port, block), name)
    runs.append({"run": name, "seconds": round(secs, 1), "peak_mib": round(mib)})
    print(name, secs, mib)
figs.write_csv(data / "memory.csv", runs)
t1 = figs.read_csv(work / "whole" / "metrics.csv")
t2 = figs.read_csv(work / "blocks_512" / "metrics.csv")
same = all(abs(float(x[k]) - float(y[k])) < 1e-9 for x, y in zip(t1, t2) for k in x)
print("tables identical:", same, t1, t2)
(work / "same.txt").write_text(str(same), encoding="utf-8")
for r in t1[:4]:
    print(r)

# ------------------------------------------------------------------ MSPA tiled against MSPA whole
shutil.copy(CLC, work / "clc.tif")
with rasterio.open(CLC) as src:
    lc = src.read(1)
    p2 = src.profile
forest = np.where((lc >= 23) & (lc <= 25), 2, 1).astype("uint8")
p2.update(dtype="uint8", nodata=None, compress="lzw")
with rasterio.open(work / "forest.tif", "w", **p2) as dst:
    dst.write(forest, 1)
MSPA = '''format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "forest.tif" }

[[nodes]]
id = "whole"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 2 }

[outputs.whole]
from = "whole.map"
'''
figs.run(MSPA, work, name="mspa_whole.toml")
whole, _ = figs.read(work / "output" / "whole.tif")
rows = []
frames = []
for ov in (0, 25, 50, 100, 200):
    m = f'''format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = {{ path = "forest.tif" }}

[[nodes]]
id = "tiled"
op = "landscape.mspa_tiled"
inputs = {{ map = "forest.raster" }}
params = {{ edge_width = 2, tile_size = {500 + 2 * ov}, overlap = {2 * ov}, path = "tiled_{ov}.tif" }}
'''
    r = figs.run(m, work, name=f"tiled_{ov}.toml", check=False)
    if r.returncode:
        print("overlap", ov, "failed:", r.stdout, r.stderr)
        continue
    t, _ = figs.read(work / f"tiled_{ov}.tif")
    diff = (t.filled(0) != whole.filled(0))
    nd = int(diff.sum())
    rows.append({"overlap": 2 * ov, "differ": nd, "share": round(nd / diff.size * 100, 4)})
    print("overlap", 2 * ov, "cells that differ", nd)
    show = np.where(diff, 2, np.where(whole.filled(0) == 0, 0, 1))
    figs.raster(None, out / f"seams_{2 * ov}.png", array=np.ma.asarray(show),
                colors={0: "#ffffff", 1: "#cfe3d2", 2: "#d62728"},
                labels={0: "background", 1: "forest, same class", 2: "a different class"}, width=640, legend=False)
    frames.append(2 * ov)
figs.write_csv(data / "seams.csv", rows)
figs.raster(work / "output" / "whole.tif", out / "mspa_whole.png", palette=figs.mspa_palette(), width=640, legend=False)
print(rows)

# ------------------------------------------------------------------ the tiled engine: Dinamica EGO's model of Sibuyan
d = figs.workdir(f"{SLUG}/tiled-engine")
for f in ("sibuyan_landuse.tif", "sibuyan_drivers.tif"):
    shutil.copy(figs.ROOT / "tests" / "data" / "inputs" / f, d / f)
print(figs.cli("new", "dinamica", "--t0", "sibuyan_landuse.tif:1997", "--t1", "sibuyan_landuse.tif:2011",
               "--drivers", "sibuyan_drivers.tif", "--steps", "14", "-o", "model.toml", cwd=d))
logs = {}
for name, extra in (("exact", []), ("tiled32", ["--tiled", "--tile", "32"]), ("tiled64", ["--tiled", "--tile", "64"]),
                    ("tiled32_1thread", ["--tiled", "--tile", "32", "--threads", "1"])):
    logs[name] = figs.cli("run", "model.toml", "--out", name, *extra, cwd=d)
with rasterio.open(d / "exact" / "maps.tif") as e:
    ex = e.read(e.count, masked=True).astype("float64")
maps = {"exact": ex}
for name in ("tiled32", "tiled64", "tiled32_1thread"):
    maps[name] = figs.read(d / name / "maps" / "step_014.tif")[0]
with rasterio.open(d / "sibuyan_landuse.tif") as o:
    maps["observed"] = o.read(2, masked=True).astype("float64")
names = {1: "Forest", 2: "Coconut", 3: "Grassland", 4: "Rice", 5: "Other"}
area_rows = []
for k in ("observed", "exact", "tiled32", "tiled64"):
    v, c = np.unique(maps[k].compressed(), return_counts=True)
    area_rows.append({"map": k, **{names[int(x)]: int(n) for x, n in zip(v, c)}})
figs.write_csv(data / "engine_areas.csv", area_rows)
print(area_rows)
for a, b in (("exact", "tiled32"), ("tiled32", "tiled64"), ("tiled32", "tiled32_1thread")):
    print(a, "vs", b, "cells that differ:", int((maps[a].filled(0) != maps[b].filled(0)).sum()))
land = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Rice", "#e8d36a"),
        5: ("Other", "#b4508a")}
for k in ("exact", "tiled32"):
    figs.raster(None, out / f"engine_{k}.png", array=maps[k], palette=land, width=600, legend=False)
print(logs["tiled32"])
