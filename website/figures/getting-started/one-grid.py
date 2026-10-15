"""Figures of 'One grid for all your maps': a land cover coarsened by majority from 25 to 400 m with the share of
its classes, the three rules of aggregation, a coarse product in degrees matched onto the land cover's grid, a
clip and a mosaic of tiles, and `terraflux resample`."""

import rasterio

from _common import figs, np, plt, CLC, WINDOW, posix, clc_palette, classes_png, read, save, CLC_LEGEND

SLUG = "getting-started/one-grid"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
SIZES = [25, 50, 100, 250, 500]

x0, y0, x1, y1 = WINDOW
xm, ym = (x0 + x1) // 2, (y0 + y1) // 2
nodes = [f'''format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = {{ path = "{posix(CLC)}" }}

[[nodes]]
id = "window"
op = "raster.clip"
inputs = {{ raster = "landcover.raster" }}
params = {{ extent = {WINDOW}, path = "window.tif" }}

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = {{ map = "window.raster" }}
params = {{ rules = "23..25 -> 1\\n* -> 0" }}

[[nodes]]
id = "forest_file"
op = "raster.write"
inputs = {{ value = "forest.map" }}
params = {{ path = "forest.tif" }}

[[nodes]]
id = "forest_float_file"
op = "raster.write"
inputs = {{ value = "forest.map" }}
params = {{ path = "forest_f32.tif", type = "f32" }}

[[nodes]]
id = "forest_float"
op = "raster.read"
params = {{ path = "forest_f32.tif" }}

[[nodes]]
id = "agg_average_byte"
op = "raster.resample"
inputs = {{ raster = "forest_disk.raster" }}
params = {{ resolution = 100, resampling = "average", path = "forest_average_byte.tif" }}

[[nodes]]
id = "forest_disk"
op = "raster.read"
params = {{ path = "forest.tif" }}

[[nodes]]
id = "agg_mode"
op = "raster.resample"
inputs = {{ raster = "forest_disk.raster" }}
params = {{ resolution = 100, resampling = "mode", path = "forest_mode.tif" }}

[[nodes]]
id = "agg_average"
op = "raster.resample"
inputs = {{ raster = "forest_float.raster" }}
params = {{ resolution = 100, resampling = "average", path = "forest_average.tif" }}

[[nodes]]
id = "agg_sum"
op = "raster.resample"
inputs = {{ raster = "forest_disk.raster" }}
params = {{ resolution = 100, resampling = "sum", path = "forest_sum.tif" }}

[[nodes]]
id = "coarse_product"
op = "raster.reproject"
inputs = {{ raster = "forest_float.raster" }}
params = {{ crs = "EPSG:4326", resolution = 0.006, resampling = "average", path = "forest_share_degrees.tif" }}

[[nodes]]
id = "matched"
op = "raster.match"
inputs = {{ raster = "coarse_product.raster", like = "window.raster" }}
params = {{ path = "forest_share_matched.tif" }}
''']
for s in SIZES[1:]:
    nodes.append(f'''[[nodes]]
id = "mode_{s}"
op = "raster.resample"
inputs = {{ raster = "window.raster" }}
params = {{ resolution = {s}, resampling = "mode", path = "mode_{s}.tif" }}

[[nodes]]
id = "near_{s}"
op = "raster.resample"
inputs = {{ raster = "window.raster" }}
params = {{ resolution = {s}, resampling = "near", path = "near_{s}.tif" }}

[[nodes]]
id = "auto_{s}"
op = "raster.resample"
inputs = {{ raster = "window.raster" }}
params = {{ resolution = {s}, path = "auto_{s}.tif" }}
''')
# four tiles of the window, then a mosaic of them
tiles = {"nw": [x0, ym, xm, y1], "ne": [xm, ym, x1, y1], "sw": [x0, y0, xm, ym], "se": [xm, y0, x1, ym]}
for k, e in tiles.items():
    nodes.append(f'''[[nodes]]
id = "tile_{k}"
op = "raster.clip"
inputs = {{ raster = "window.raster" }}
params = {{ extent = {e}, path = "tiles/{k}.tif" }}
''')
figs.run("\n".join(nodes), work)
mosaic = '''format = "terraflux/1"

[[nodes]]
id = "tiles"
op = "raster.read"
params = { path = "tiles/*.tif" }

[[nodes]]
id = "mosaic"
op = "raster.mosaic"
inputs = { raster = "tiles.raster" }
params = { path = "mosaic.tif" }
'''
figs.run(mosaic, work, name="mosaic.toml")

# ------------------------------------------------------------------ 1. coarsening by majority: frames and shares
win = read(work / "window.tif")[0]
orig = sorted(int(v) for v in np.unique(win.compressed()))
pal = clc_palette(orig)
watch = {12: "arable", 24: "coniferous", 23: "broadleaved", 2: "urban", 18: "pastures", 40: "river", 36: "peat"}
rows_mode, rows_near = [], []
frames = []
for s in SIZES:
    m = win if s == 25 else read(work / f"mode_{s}.tif")[0]
    n = win if s == 25 else read(work / f"near_{s}.tif")[0]
    if s != 25:
        a = read(work / f"auto_{s}.tif")[0]
        assert (a == m).all(), f"auto is not mode at {s}"
    classes_png(m, out / f"mode_{s}.png", pal, width=900, legend=True, fontsize=7.4, legend_frac=0.38, all_keys=True)
    tot = m.count()
    r = {"cell": s}
    rn = {"cell": s}
    for v, name in watch.items():
        r[name] = round(float((m == v).sum()) / tot * 100, 2)
        rn[name] = round(float((n == v).sum()) / n.count() * 100, 2)
    r["classes"] = len(np.unique(m.compressed()))
    rn["classes"] = len(np.unique(n.compressed()))
    rows_mode.append(r)
    rows_near.append(rn)
    frames.append((s, m.shape, r))
figs.write_csv(data / "shares_mode.csv", rows_mode)
# the rare classes by both methods, for a second chart
both = [{"cell": a["cell"], "river_mode": a["river"], "river_near": b["river"], "urban_mode": a["urban"],
         "urban_near": b["urban"]} for a, b in zip(rows_mode, rows_near)]
figs.write_csv(data / "rare_classes.csv", both)
for f in frames:
    print("frame", f)
for r in rows_near:
    print("near", r)

# ------------------------------------------------------------------ 2. the three rules of aggregation
fmode = read(work / "forest_mode.tif")[0]
favg = read(work / "forest_average.tif")[0]
fsum = read(work / "forest_sum.tif")[0]
forest = read(work / "forest.tif")[0]
figs.raster(None, out / "agg_mode.png", array=fmode, colors={0: "#ece8dc", 1: "#3b8a5a"},
            labels={0: "not forest", 1: "forest"}, width=520)
figs.raster(None, out / "agg_average.png", array=favg, cmap="Greens", vmin=0, vmax=1, unit="share of forest", width=520)
figs.raster(None, out / "agg_sum.png", array=fsum, cmap="Greens", vmin=0, vmax=16, unit="forest cells of 25 m", width=520)
print("forest at 25 m", float(forest.mean()), "cells", int(forest.sum()))
print("mode share", float(fmode.mean()), "average mean", float(favg.mean()), "sum total", float(fsum.sum()),
      "sum max", float(fsum.max()), "shape", fsum.shape)

# ------------------------------------------------------------------ 3. a coarse product in degrees, matched onto the grid
prod = read(work / "forest_share_degrees.tif")
matched = read(work / "forest_share_matched.tif")
with rasterio.open(work / "forest_share_degrees.tif") as d:
    pinfo = (str(d.crs), d.width, d.height, d.res, d.bounds)
with rasterio.open(work / "forest_share_matched.tif") as d:
    minfo = (str(d.crs), d.width, d.height, d.res, d.bounds, d.dtypes[0])
print("product", pinfo)
print("matched", minfo)
figs.raster(None, out / "product_degrees.png", array=prod[0], cmap="Greens", vmin=0, vmax=1, unit="share of forest",
            width=520)
figs.raster(None, out / "product_matched.png", array=matched[0], cmap="Greens", vmin=0, vmax=1, unit="share of forest",
            width=520)

# ------------------------------------------------------------------ 4. clip: the window on the whole map
full = read(CLC)[0]
r0 = int((3282500 - y1) / 25)
c0 = int((x0 - 4261250) / 25)
classes_png(full[::2, ::2], out / "clip_box.png", clc_palette(np.unique(full.compressed())), width=560, legend=False,
            boxes=[(r0 / 2, c0 / 2, 300, 300)])


# ------------------------------------------------------------------ 5. tiles and their mosaic
def tiles_fig():
    fig, axs = plt.subplots(2, 2, figsize=(4.2, 4.2), dpi=150)
    for ax, k in zip(axs.ravel(), ["nw", "ne", "sw", "se"]):
        t = read(work / "tiles" / f"{k}.tif")[0]
        from matplotlib import colors as mc
        img = np.ones(t.shape + (4,))
        for v, (_, c) in pal.items():
            img[t == v] = mc.to_rgba(c)
        ax.imshow(img, interpolation="nearest")
        ax.set_title(f"tiles/{k}.tif", fontsize=8.5, loc="left")
        ax.set_axis_off()
    fig.tight_layout(pad=0.6)
    save(fig, out / "tiles.png")


tiles_fig()
mos = read(work / "mosaic.tif")[0]
classes_png(mos, out / "mosaic.png", pal, width=420, legend=False)
print("mosaic equal to the window:", bool((mos == win).all()), mos.shape)

# ------------------------------------------------------------------ 6. two grids that do not line up (a drawing)
def misaligned():
    fig, ax = plt.subplots(figsize=(4.6, 3.4), dpi=150)
    for i in range(7):
        ax.plot([i, i], [0, 5], color=figs.FOREST, lw=1.2)
    for j in range(6):
        ax.plot([0, 6], [j, j], color=figs.FOREST, lw=1.2)
    for i in range(6):
        ax.plot([0.4 + i * 1.25, 0.4 + i * 1.25], [-0.3, 4.7], color=figs.EMBER, lw=1.2, ls="--")
    for j in range(5):
        ax.plot([0.4, 0.4 + 5 * 1.25], [-0.3 + j * 1.25, -0.3 + j * 1.25], color=figs.EMBER, lw=1.2, ls="--")
    ax.text(0, 5.2, "the land cover's grid: 30 m", color=figs.FOREST, fontsize=8.5)
    ax.text(3.2, -0.75, "a driver's grid: 37.5 m, shifted", color=figs.EMBER, fontsize=8.5)
    ax.set_xlim(-0.3, 7.0)
    ax.set_ylim(-1.0, 5.6)
    ax.set_aspect("equal")
    ax.set_axis_off()
    save(fig, out / "misaligned.png")


misaligned()

# ------------------------------------------------------------------ 7. terraflux resample, from the command line
txt = figs.cli("resample", "forest_average.tif", "--like", "window.tif", "-o", "forest_share_25m.tif", cwd=work)
print("CLI said:", txt)
print("cli exists", (work / "forest_share_25m.tif").exists())
c = read(work / "forest_share_25m.tif")
print("cli result", c[1]["shape"], float(c[0].min()), float(c[0].max()))
fb = read(work / "forest_average_byte.tif")[0]
print("average of the byte map: values", np.unique(fb.compressed()), "mean", float(fb.mean()))
print("average of the float map: mean", float(favg.mean()), "values like", np.unique(favg.compressed())[:6])

# ------------------------------------------------------------------ the shares relative to those at 25 m, for the chart
base = rows_mode[0]
rel = [{"cell": r["cell"], **{k: round(r[k] / base[k] * 100, 1) for k in ("arable", "coniferous", "pastures", "urban",
                                                                         "broadleaved", "river")}} for r in rows_mode]
figs.write_csv(data / "shares_relative.csv", rel)
for r in rel:
    print("relative", r)
