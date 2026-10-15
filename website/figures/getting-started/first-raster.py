"""Figures of 'Your first raster': the parts of a raster, a land cover map and its cells, continuous maps, no data,
a stack of drivers, a legend. Every map is written by a run of Terraflux and drawn from that file."""

from _common import *  # noqa: F403
from _common import figs, np, plt, mcolors, Patch, CLC, ELEVATION, NDVI, SIBUYAN, SIBUYAN_DRIVERS, WINDOW, posix
from _common import clc_palette, classes_png, read, save, CLC_LEGEND

SLUG = "getting-started/first-raster"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)

# ------------------------------------------------------------------ the model: read, legend, write
present = sorted(int(v) for v in np.unique(read(CLC)[0].compressed()))
legend_rows = ",\n  ".join(f'[{v}, "{CLC_LEGEND[v][0]}", "{CLC_LEGEND[v][1]}"]' for v in present)
model = f'''format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = {{ path = "{posix(CLC)}" }}

[[nodes]]
id = "legend"
op = "raster.legend"
inputs = {{ map = "landcover.map" }}
params = {{ classes = [
  {legend_rows}
] }}

[[nodes]]
id = "window"
op = "raster.clip"
inputs = {{ raster = "landcover.raster" }}
params = {{ extent = {WINDOW}, path = "window.tif" }}

[[nodes]]
id = "elevation"
op = "raster.read"
params = {{ path = "{posix(ELEVATION)}" }}

[[nodes]]
id = "ndvi"
op = "raster.read"
params = {{ path = "{posix(NDVI)}" }}

[[nodes]]
id = "sibuyan"
op = "raster.read"
params = {{ path = "{posix(SIBUYAN)}", band = "1997" }}

[[nodes]]
id = "sibuyan_no_nodata"
op = "raster.read"
params = {{ path = "{posix(SIBUYAN)}", band = "1997", nodata = -1 }}

[[nodes]]
id = "drivers"
op = "raster.read"
params = {{ path = "{posix(SIBUYAN_DRIVERS)}" }}

[[nodes]]
id = "sibuyan_legend"
op = "raster.legend"
inputs = {{ map = "sibuyan.map" }}
params = {{ classes = [[1, "Forest", "#3b8a5a"], [2, "Coconut", "#d9a441"], [3, "Grassland", "#a7c957"], [4, "Rice", "#e8d36a"], [5, "Other", "#b4508a"]] }}
'''
model = figs.add_writes(model, {"legend.map": "landcover.tif", "elevation.map": "elevation.tif", "ndvi.map": "ndvi.tif",
                                "sibuyan.map": "sibuyan_1997.tif", "sibuyan_no_nodata.map": "sibuyan_raw.tif",
                                "drivers.stack": "drivers.tif", "sibuyan_legend.map": "sibuyan_legend.tif"})
figs.run(model, work)

# ------------------------------------------------------------------ 1. the parts of a raster (a drawing)
def anatomy():
    rng = np.random.default_rng(4)
    a = np.array([[1, 1, 1, 2, 2, 2, 3, 3],
                  [1, 1, 2, 2, 2, 3, 3, 3],
                  [1, 1, 1, 2, 2, 2, 3, 3],
                  [1, 1, 1, 1, 2, 2, 2, 3],
                  [9, 1, 1, 1, 1, 2, 2, 2],
                  [9, 9, 1, 1, 1, 1, 2, 2]])
    cols = {1: "#3b8a5a", 2: "#d9a441", 3: "#3f7fae"}
    fig, ax = plt.subplots(figsize=(7.4, 5.0), dpi=150)
    ax.set_axis_off()
    ax.set_aspect("equal")
    ox, oy = 0, 0
    for r in range(6):
        for c in range(8):
            v = a[r, c]
            y = 6 - r - 1
            if v == 9:
                ax.add_patch(plt.Rectangle((c, y), 1, 1, facecolor="#ffffff", edgecolor="#9aa59f", hatch="////",
                                           linewidth=0.8))
            else:
                ax.add_patch(plt.Rectangle((c, y), 1, 1, facecolor=cols[v], edgecolor="#ffffff", linewidth=1.2))
                ax.text(c + 0.5, y + 0.5, str(v), ha="center", va="center", color="white", fontsize=10,
                        fontweight="bold")
    # rows and columns
    for c in range(8):
        ax.text(c + 0.5, 6.25, str(c), ha="center", va="bottom", fontsize=8.5, color=figs.INK2)
    for r in range(6):
        ax.text(-0.25, 6 - r - 0.5, str(r), ha="right", va="center", fontsize=8.5, color=figs.INK2)
    ax.text(4, 6.85, "columns (0 to 7)", ha="center", fontsize=9, color=figs.INK2)
    ax.text(-1.15, 3, "rows (0 to 5)", ha="center", va="center", rotation=90, fontsize=9, color=figs.INK2)
    # the origin
    ax.plot([0], [6], "o", color=figs.EMBER, markersize=7, zorder=5)
    ax.annotate("origin: the top-left corner\nx = 500 000, y = 4 600 000", xy=(0, 6), xytext=(-2.6, 7.6),
                fontsize=8.5, color=figs.INK, arrowprops=dict(arrowstyle="->", color=figs.EMBER))
    # the cell size
    ax.annotate("", xy=(7, -0.35), xytext=(8, -0.35), arrowprops=dict(arrowstyle="<->", color=figs.INK))
    ax.text(7.5, -0.75, "cell size\n30 m", ha="center", va="top", fontsize=8.5)
    # the extent
    ax.add_patch(plt.Rectangle((0, 0), 8, 6, fill=False, edgecolor=figs.EMBER, linewidth=2.2, linestyle="--"))
    ax.text(8.25, 0.0, "extent: 8 x 30 m = 240 m wide,\n6 x 30 m = 180 m high\n(x 500 000 to 500 240,\n"
            "  y 4 599 820 to 4 600 000)", ha="left", va="bottom", fontsize=8.5, color=figs.INK)
    ax.annotate("a cell with no value\n(nodata): outside the\nstudy area", xy=(0.5, 0.5), xytext=(-3.3, -1.6),
                fontsize=8.5, color=figs.INK, arrowprops=dict(arrowstyle="->", color=figs.INK2))
    ax.annotate("row 1, column 5:\nvalue 3 (water)", xy=(5.5, 4.5), xytext=(8.25, 4.9), fontsize=8.5,
                arrowprops=dict(arrowstyle="->", color=figs.INK2))
    ax.text(8.25, 2.6, "CRS: WGS 84 / UTM 18N\n(EPSG:32618), in metres", fontsize=8.5, color=figs.INK)
    hs = [Patch(facecolor=cols[1], label="1 forest"), Patch(facecolor=cols[2], label="2 farming"),
          Patch(facecolor=cols[3], label="3 water")]
    ax.legend(handles=hs, loc="upper left", bbox_to_anchor=(0.73, 0.98), fontsize=8.5, handlelength=1, handleheight=1)
    ax.set_xlim(-3.6, 12.6)
    ax.set_ylim(-2.4, 8.2)
    save(fig, out / "anatomy.png")


anatomy()

# ------------------------------------------------------------------ 2. the land cover and its cells
lc, info = read(work / "landcover.tif")
classes_png(lc, out / "landcover.png", clc_palette(present), width=1000, ncol=1, fontsize=7.6, legend_frac=0.40,
            counts=True)

# the zooms: the 15 km window, 2.5 km, 12 x 12 cells
win = read(work / "window.tif")[0]
pal = clc_palette(np.unique(win.compressed()))
r0, c0 = 146, 72
classes_png(win, out / "zoom_1.png", pal, width=520, legend=False, boxes=[(r0, c0, 100, 100)])
classes_png(win[r0:r0 + 100, c0:c0 + 100], out / "zoom_2.png", pal, width=520, legend=False, boxes=[(44, 44, 12, 12)])


def cells_with_values(a, png):
    h, w = a.shape
    fig, ax = plt.subplots(figsize=(3.5, 3.5), dpi=150)
    ax.set_axis_off()
    for r in range(h):
        for c in range(w):
            v = int(a[r, c])
            ax.add_patch(plt.Rectangle((c, h - r - 1), 1, 1, facecolor=CLC_LEGEND[v][1], edgecolor="white", lw=0.8))
            dark = v in (1, 2, 3, 4, 7, 8, 24, 36, 40)
            ax.text(c + 0.5, h - r - 0.5, str(v), ha="center", va="center", fontsize=7,
                    color="white" if dark else figs.INK)
    ax.set_xlim(0, w)
    ax.set_ylim(0, h)
    ax.set_aspect("equal")
    save(fig, png)


sub = win[r0 + 44:r0 + 56, c0 + 44:c0 + 56]
cells_with_values(sub, out / "zoom_3.png")
zoom_classes = sorted(int(v) for v in np.unique(sub))

# ------------------------------------------------------------------ 3. continuous maps
figs.raster(work / "elevation.tif", out / "elevation.png", cmap="terrain", unit="metres", width=560)
figs.raster(work / "ndvi.tif", out / "ndvi.png", cmap="RdYlGn", unit="NDVI", width=560)
el = read(work / "elevation.tif")[0]
nd = read(work / "ndvi.tif")[0]

# ------------------------------------------------------------------ 4. no data respected, or read as a value
sib = read(work / "sibuyan_1997.tif")[0]
raw = read(work / "sibuyan_raw.tif")[0]
sib_cols = {1: "#3b8a5a", 2: "#d9a441", 3: "#a7c957", 4: "#e8d36a", 5: "#b4508a", 255: "#4a4a4a"}
for name, arr in (("nodata_on", sib), ("nodata_off", raw)):
    figs.raster(None, out / f"{name}.png", array=arr, colors=sib_cols, legend=False, width=760)
classes_png(sib, out / "sibuyan_legend.png",
            {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Rice", "#e8d36a"),
             5: ("Other", "#b4508a")}, width=760, counts=True, fontsize=9, legend_frac=0.28)

# ------------------------------------------------------------------ 5. a stack: the drivers of Sibuyan, a band each
names = list(read(work / "drivers.tif")[1]["descriptions"])
pretty = {"dist_sea": "distance to the sea (m)", "pop_density": "population density", "diorite": "diorite soils (0/1)",
          "ultramafic": "ultramafic soils (0/1)", "sediments": "sediments (0/1)", "no_erosion": "no erosion (0/1)",
          "moderate_erosion": "moderate erosion (0/1)", "elevation": "elevation (m)", "slope": "slope (degrees)",
          "aspect": "aspect (degrees)", "dist_roads": "distance to roads (m)", "dist_urban": "distance to towns (m)",
          "dist_streams": "distance to streams (m)"}
frames = []
for b, n in enumerate(names, start=1):
    a = read(work / "drivers.tif", b)[0]
    binary = set(np.unique(a.compressed()).tolist()) <= {0.0, 1.0}
    if binary:
        figs.raster(None, out / f"driver_{b}.png", array=a, colors={0: "#e9e4d8", 1: "#3b6e8f"},
                    labels={0: "0 (no)", 1: "1 (yes)"}, width=760)
    else:
        figs.raster(None, out / f"driver_{b}.png", array=a, cmap="viridis", unit=pretty.get(n, n), width=760)
    frames.append((b, n, pretty.get(n, n), float(a.min()), float(a.max())))

# ------------------------------------------------------------------ numbers for the page
import rasterio  # noqa: E402

with rasterio.open(CLC) as d:
    facts = dict(crs=str(d.crs), size=(d.width, d.height), res=d.res, dtype=d.dtypes[0], nodata=d.nodata,
                 bounds=d.bounds)
total = lc.count()
print("CLC", facts, "classes", len(present))
for v in present:
    print(f"  {v:2d} {CLC_LEGEND[v][0]:40s} {(lc == v).sum() / total * 100:6.2f}%")
print("zoom classes", zoom_classes)
print("elevation", float(el.min()), float(el.max()), "ndvi", float(nd.min()), float(nd.max()))
print("sibuyan", sib.count(), "cells with a value of", sib.size, "; raw 255 cells", int((raw == 255).sum()))
for fr in frames:
    print("driver", fr)
