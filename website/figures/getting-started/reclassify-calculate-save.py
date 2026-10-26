"""Figures of 'Reclassify, calculate and save': rules on a toy map, CORINE's 26 classes in six groups, map algebra
(a forest's share around each cell, its edge), the sizes of the file types and of a COG, gaps filled, and a slope
and a hillshade."""

import json
import os

import rasterio
from rasterio.transform import from_origin

from _common import figs, np, plt, CLC, ELEVATION, NDVI, WINDOW, BROAD, BROAD_RULES, posix, clc_palette, classes_png
from _common import read, save, CLC_LEGEND

SLUG = "getting-started/reclassify-calculate-save"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)

# ------------------------------------------------------------------ a toy map to show the rules
toy = np.array([[1, 2, 3, 4, 5, 6],
                [2, 3, 4, 5, 6, 7],
                [3, 4, 5, 6, 7, 8],
                [9, 9, 6, 7, 8, 12],
                [9, 0, 0, 8, 12, 12],
                [0, 0, 255, 255, 12, 12]], dtype="uint8")
with rasterio.open(work / "toy.tif", "w", driver="GTiff", width=6, height=6, count=1, dtype="uint8", nodata=255,
                   crs="EPSG:32632", transform=from_origin(500000, 4600000, 30, 30)) as d:
    d.write(toy, 1)
TOY_RULES = "1 2 3 -> 1\n4..6 -> 2\n9 -> nan\n* -> 0"

legend = ", ".join(f'[{k}, "{n}", "{c}"]' for k, (n, c) in BROAD.items())
model = f'''format = "terraflux/1"

[[nodes]]
id = "toy"
op = "raster.read"
params = {{ path = "toy.tif" }}

[[nodes]]
id = "toy_rules"
op = "raster.reclassify"
inputs = {{ map = "toy.raster" }}
params = {{ rules = {json.dumps(TOY_RULES)} }}

[[nodes]]
id = "landcover"
op = "raster.read"
params = {{ path = "{posix(CLC)}" }}

[[nodes]]
id = "broad"
op = "raster.reclassify"
inputs = {{ map = "landcover.raster" }}
params = {{ rules = {json.dumps(BROAD_RULES)} }}

[[nodes]]
id = "named"
op = "raster.legend"
inputs = {{ map = "broad.map" }}
params = {{ classes = [{legend}] }}

[[nodes]]
id = "window"
op = "raster.clip"
inputs = {{ raster = "landcover.raster" }}
params = {{ extent = {WINDOW}, path = "window.tif" }}

[[nodes]]
id = "share"
op = "raster.calc"
inputs = {{ lc = "window.raster" }}
params = {{ expression = "nbmean(lc >= 23 and lc <= 25, 20)" }}

[[nodes]]
id = "edge"
op = "raster.calc"
inputs = {{ lc = "window.raster" }}
params = {{ expression = "(nbmin(lc >= 23 and lc <= 25, 1) == 0) + 1 if lc >= 23 and lc <= 25 else 0" }}

[[nodes]]
id = "u8"
op = "raster.write"
inputs = {{ value = "broad.map" }}
params = {{ path = "broad_u8.tif" }}

[[nodes]]
id = "i32"
op = "raster.write"
inputs = {{ value = "broad.map" }}
params = {{ path = "broad_i32.tif", type = "i32" }}

[[nodes]]
id = "f64"
op = "raster.write"
inputs = {{ value = "broad.map" }}
params = {{ path = "broad_f64.tif", type = "f64" }}

[[nodes]]
id = "broad_disk"
op = "raster.read"
params = {{ path = "broad_u8.tif" }}

[[nodes]]
id = "cog"
op = "raster.convert"
inputs = {{ raster = "broad_disk.raster" }}
params = {{ format = "COG", path = "broad_cog.tif" }}

[[nodes]]
id = "elevation"
op = "raster.read"
params = {{ path = "{posix(ELEVATION)}" }}

[[nodes]]
id = "elevation_utm"
op = "raster.reproject"
inputs = {{ raster = "elevation.raster" }}
params = {{ crs = "EPSG:32610", resolution = 30, path = "elevation_utm.tif" }}

[[nodes]]
id = "slope"
op = "raster.terrain"
inputs = {{ raster = "elevation_utm.raster" }}
params = {{ measure = "slope", path = "slope.tif" }}

[[nodes]]
id = "hillshade"
op = "raster.terrain"
inputs = {{ raster = "elevation_utm.raster" }}
params = {{ measure = "hillshade", path = "hillshade.tif" }}

[[nodes]]
id = "ndvi"
op = "raster.read"
params = {{ path = "{posix(NDVI)}" }}

[[nodes]]
id = "gaps"
op = "raster.calc"
inputs = {{ elevation = "elevation.raster", ndvi = "ndvi.raster" }}
params = {{ expression = "None if ndvi < 0.4 else elevation" }}

[[nodes]]
id = "gaps_file"
op = "raster.write"
inputs = {{ value = "gaps.map" }}
params = {{ path = "gaps.tif", type = "f32" }}

[[nodes]]
id = "gaps_disk"
op = "raster.read"
params = {{ path = "gaps.tif" }}

[[nodes]]
id = "filled"
op = "raster.fill"
inputs = {{ raster = "gaps_disk.raster" }}
params = {{ max_distance = 100, path = "filled.tif" }}
'''
model = figs.add_writes(model, {"toy_rules.map": "toy_out.tif", "named.map": "broad.tif", "share.map": "share.tif",
                                "edge.map": "edge.tif", "elevation.raster": "elevation.tif"})
figs.run(model, work)


# ------------------------------------------------------------------ 1. the toy map, before and after the rules
def grid_fig(a, png, colours, title):
    h, w = a.shape
    fig, ax = plt.subplots(figsize=(2.9, 3.2), dpi=150)
    for r in range(h):
        for c in range(w):
            v = a[r, c]
            nod = np.ma.is_masked(v)
            ax.add_patch(plt.Rectangle((c, h - r - 1), 1, 1, facecolor="#ffffff" if nod else colours(v),
                                       edgecolor="#9aa59f" if nod else "white", hatch="////" if nod else None, lw=0.8))
            if not nod:
                ax.text(c + 0.5, h - r - 0.5, f"{int(v)}", ha="center", va="center", fontsize=9,
                        color="white" if (title == "before" and int(v) >= 8) or (title == "after" and int(v) == 2) else "black")
    ax.set_xlim(0, w)
    ax.set_ylim(0, h)
    ax.set_aspect("equal")
    ax.set_axis_off()
    ax.set_title(title, fontsize=10, loc="left")
    save(fig, png)


tin = read(work / "toy.tif")[0]
tout = read(work / "toy_out.tif")[0]
cm = plt.get_cmap("YlGnBu")
grid_fig(tin, out / "toy_in.png", lambda v: cm(0.1 + 0.07 * min(v, 12)), "before")
grid_fig(tout, out / "toy_out.png", lambda v: {0: "#ece8dc", 1: "#a7c957", 2: "#3b8a5a"}.get(int(v), "#ccc"), "after")
print("toy out", tout)

# ------------------------------------------------------------------ 2. CORINE's 26 classes in six groups
lc = read(CLC)[0]
present = sorted(int(v) for v in np.unique(lc.compressed()))
classes_png(lc, out / "clc_26.png", clc_palette(present), width=560, legend=False)
broad = read(work / "broad.tif")[0]
classes_png(broad, out / "clc_6.png", BROAD, width=560, legend=False)
classes_png(broad, out / "clc_6_legend.png", BROAD, width=760, counts=True, fontsize=9.5, legend_frac=0.3)
with rasterio.open(work / "broad.tif") as d:
    print("legend tag", d.tags().get("terraflux_legend", "")[:80])
tot = broad.count()
print("broad shares", {BROAD[k][0]: round(float((broad == k).sum()) / tot * 100, 2) for k in BROAD})

# ------------------------------------------------------------------ 3. map algebra
share = read(work / "share.tif")[0]
figs.raster(None, out / "share.png", array=share, cmap="Greens", vmin=0, vmax=1, unit="share of forest within 500 m",
            width=560)
edge = read(work / "edge.tif")[0]
figs.raster(None, out / "edge.png", array=edge[300:500, 300:500], colors={0: "#ece8dc", 1: "#3b8a5a", 2: "#e8901f"},
            labels={0: "not forest", 1: "forest interior", 2: "forest edge"}, width=560, counts=True)
print("share", float(share.min()), float(share.max()), float(share.mean()))
print("edge counts", {k: int((edge == k).sum()) for k in (0, 1, 2)})

# ------------------------------------------------------------------ 4. file types and sizes
for f in ("broad_u8.tif", "broad_i32.tif", "broad_f64.tif", "broad_cog.tif", "broad.tif"):
    with rasterio.open(work / f) as d:
        print(f, d.dtypes[0], os.path.getsize(work / f), "bytes", "overviews", d.overviews(1),
              d.profile.get("compress"), "tiled", d.profile.get("tiled"), d.profile.get("blockxsize"))
print("uncompressed bytes u8", 2000 * 2000, "f64", 2000 * 2000 * 8)

# ------------------------------------------------------------------ 5. gaps filled
el = read(work / "elevation.tif")[0]
gaps = read(work / "gaps.tif")[0]
filled = read(work / "filled.tif")[0]
holes = gaps.mask & ~el.mask
err = np.abs(filled[holes] - el[holes])
print("gaps", int(holes.sum()), f"{holes.mean() * 100:.1f}%", "filled left", int(filled.mask.sum()),
      "mean abs error", float(err.mean()), "p90", float(np.percentile(err.compressed(), 90)), "max", float(err.max()))
vmin, vmax = float(el.min()), float(el.max())
figs.raster(None, out / "gaps.png", array=gaps, cmap="terrain", vmin=vmin, vmax=vmax, legend=False, width=640)
figs.raster(None, out / "filled.png", array=filled, cmap="terrain", vmin=vmin, vmax=vmax, legend=False, width=640)

# ------------------------------------------------------------------ 6. slope and hillshade
sl = read(work / "slope.tif")[0]
hs = read(work / "hillshade.tif")[0]
figs.raster(work / "slope.tif", out / "slope.png", cmap="magma_r", unit="slope (degrees)", width=520)
figs.raster(work / "hillshade.tif", out / "hillshade.png", cmap="gray", unit="hillshade (0-255)", width=520)
print("slope", float(sl.min()), float(sl.max()), float(sl.mean()), "hillshade", float(hs.min()), float(hs.max()))
