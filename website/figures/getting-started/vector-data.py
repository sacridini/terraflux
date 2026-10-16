"""Figures of 'Vector data': three layers of features made for the tutorial over the CORINE window (reserves drawn
around the largest forests, a planned road, sample plots), then a Terraflux run that reads them, burns them into
the grid, measures the land cover in each reserve, extracts it at the plots and measures the distance to the road."""

import geopandas as gpd
import rasterio
from rasterio import features as rfeatures
from shapely.geometry import LineString, Point, shape
from shapely.ops import unary_union

from _common import figs, np, plt, Patch, CLC, WINDOW, posix, clc_palette, classes_png, read, save, CLC_LEGEND

SLUG = "getting-started/vector-data"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
CRS = "EPSG:3035"

# ------------------------------------------------------------------ the window of land cover (a first run)
clip = f'''format = "terraflux/1"
[[nodes]]
id = "landcover"
op = "raster.read"
params = {{ path = "{posix(CLC)}" }}
[[nodes]]
id = "window"
op = "raster.clip"
inputs = {{ raster = "landcover.raster" }}
params = {{ extent = {WINDOW}, path = "landcover.tif" }}
'''
figs.run(clip, work, name="clip.toml")

# ------------------------------------------------------------------ the features, made for the tutorial
with rasterio.open(work / "landcover.tif") as d:
    lc = d.read(1)
    T = d.transform
forest = np.isin(lc, [23, 24, 25]).astype("uint8")
lab = np.zeros_like(lc, dtype="int32")
polys = [(shape(g), v) for g, v in rfeatures.shapes(forest, mask=forest == 1, transform=T, connectivity=4)]
polys.sort(key=lambda p: -p[0].area)
names = ["South", "North", "West"]
reserves = []
for k, (g, _) in enumerate(polys[:3]):
    # a reserve: the forest grown by 150 m, its holes filled, simplified as a manager would draw it
    r = g.buffer(150).buffer(-60).simplify(80)
    r = shape({"type": "Polygon", "coordinates": [list(r.exterior.coords)]}) if r.geom_type == "Polygon" else r
    reserves.append(r)
gpd.GeoDataFrame({"id": [1, 2, 3], "name": names, "year": [1984, 1998, 2011]}, geometry=reserves, crs=CRS).to_file(
    work / "reserves.gpkg")
x0, y0, x1, y1 = WINDOW
road = LineString([(x0 + 300, y0 + 2600), (x0 + 4800, y0 + 5200), (x0 + 8200, y0 + 6100), (x0 + 11000, y0 + 9400),
                   (x1 - 300, y0 + 12800)])
gpd.GeoDataFrame({"name": ["planned road"], "lanes": [2]}, geometry=[road], crs=CRS).to_file(work / "road.gpkg")
rng = np.random.default_rng(12)
pts = [Point(x0 + 1000 + rng.random() * 13000, y0 + 1000 + rng.random() * 13000) for _ in range(12)]
gpd.GeoDataFrame({"plot": list(range(1, 13))}, geometry=pts, crs=CRS).to_file(work / "plots.gpkg")

# ------------------------------------------------------------------ the model
model = '''format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "landcover.tif" }

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }

[[nodes]]
id = "road"
op = "vector.read"
params = { path = "road.gpkg" }

[[nodes]]
id = "plots"
op = "vector.read"
params = { path = "plots.gpkg" }

[[nodes]]
id = "burnt"
op = "vector.rasterize"
inputs = { vector = "reserves.vector", like = "landcover.raster" }
params = { field = "id" }

[[nodes]]
id = "burnt_touched"
op = "vector.rasterize"
inputs = { vector = "reserves.vector", like = "landcover.raster" }
params = { field = "id", all_touched = true }

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landcover.raster" }
params = { rules = "23..25 -> 1\\n* -> 0" }

[[nodes]]
id = "forest_share"
op = "vector.zonal"
inputs = { vector = "reserves.vector", map = "forest.map" }
params = { stats = ["count", "mean"] }

[[nodes]]
id = "cover"
op = "vector.zonal"
inputs = { vector = "reserves.vector", map = "landcover.raster" }
params = { stats = ["majority", "unique"] }

[[nodes]]
id = "at_plots"
op = "vector.extract"
inputs = { vector = "plots.vector", map = "landcover.raster" }

[[nodes]]
id = "around_plots"
op = "vector.extract"
inputs = { vector = "plots.vector", map = "forest.map" }
params = { buffer = 250, stats = ["mean"] }

[[nodes]]
id = "distance"
op = "vector.distance"
inputs = { vector = "road.vector", like = "landcover.raster" }

[[nodes]]
id = "corridor"
op = "vector.buffer"
inputs = { vector = "road.vector" }
params = { distance = 500 }

[[nodes]]
id = "save_corridor"
op = "vector.write"
inputs = { vector = "corridor.vector" }
params = { path = "corridor.gpkg" }

[outputs.forest_share]
from = "forest_share.table"
[outputs.cover]
from = "cover.table"
[outputs.at_plots]
from = "at_plots.table"
[outputs.around_plots]
from = "around_plots.table"
'''
model = figs.add_writes(model, {"burnt.map": "burnt.tif", "burnt_touched.map": "burnt_touched.tif",
                                "distance.map": "distance.tif"})
figs.run(model, work)

res = gpd.read_file(work / "reserves.gpkg")
rd = gpd.read_file(work / "road.gpkg")
pl = gpd.read_file(work / "plots.gpkg")
cor = gpd.read_file(work / "corridor.gpkg")
ext = [x0, x1, y0, y1]
pal = clc_palette(np.unique(lc))


def rgba_classes(a, palette):
    from matplotlib import colors as mc
    img = np.ones(a.shape + (4,))
    for v, (_, c) in palette.items():
        img[a == v] = mc.to_rgba(c)
    return img


# ------------------------------------------------------------------ 1. the three kinds of features (a drawing)
def kinds():
    fig, axs = plt.subplots(1, 3, figsize=(7.6, 2.8), dpi=150)
    for ax in axs:
        ax.set_xticks([])
        ax.set_yticks([])
        ax.set_aspect("equal")
        ax.set_xlim(0, 10)
        ax.set_ylim(0, 8)
        ax.grid(False)
    p = [(1.5, 6), (3.5, 2.5), (6, 5), (8, 1.8), (7.8, 6.5)]
    axs[0].scatter(*zip(*p), s=40, color=figs.EMBER, edgecolor="white", zorder=3)
    for i, (x, y) in enumerate(p, 1):
        axs[0].text(x + 0.35, y + 0.3, str(i), fontsize=8)
    axs[0].set_title("Points", loc="left")
    axs[0].text(0.3, 0.3, "plots, records, nests", fontsize=8, color=figs.INK2)
    axs[1].plot([0.5, 3, 5, 7, 9.5], [1, 3.5, 3, 6, 7], color="#5b5b5b", lw=2.5)
    axs[1].plot([1, 2.5, 4, 6, 9], [7.5, 6, 6.5, 4.5, 2], color=figs.WATER, lw=2)
    axs[1].set_title("Lines", loc="left")
    axs[1].text(0.3, 0.3, "roads, rivers, fences", fontsize=8, color=figs.INK2)
    from matplotlib.patches import Polygon as P
    axs[2].add_patch(P([(1, 1), (4.5, 1.5), (5, 5), (2, 6.5), (0.8, 4)], facecolor="#3b8a5a55", edgecolor=figs.FOREST, lw=1.6))
    axs[2].add_patch(P([(5.8, 3), (9.2, 2.2), (9.4, 7), (6.5, 7.2)], facecolor="#3b8a5a55", edgecolor=figs.FOREST, lw=1.6))
    axs[2].text(2.6, 3.6, "1", fontsize=9)
    axs[2].text(7.6, 4.8, "2", fontsize=9)
    axs[2].set_title("Polygons", loc="left")
    axs[2].text(0.3, 0.3, "reserves, parcels, regions", fontsize=8, color=figs.INK2)
    fig.tight_layout()
    save(fig, out / "kinds.png")


kinds()


# ------------------------------------------------------------------ 2. the features over the land cover
def overlay(png, show_corridor=False, show_dist=False):
    fig, ax = plt.subplots(figsize=(6.4, 6.4), dpi=150)
    if show_dist:
        d = read(work / "distance.tif")[0]
        im = ax.imshow(d, extent=ext, cmap="viridis", interpolation="nearest")
        cb = fig.colorbar(im, ax=ax, shrink=0.75, pad=0.02)
        cb.set_label("distance to the road (m)")
        cb.outline.set_visible(False)
    else:
        ax.imshow(rgba_classes(lc, pal), extent=ext, interpolation="nearest")
    if show_corridor:
        cor.boundary.plot(ax=ax, color="white", lw=1.6, linestyle="--")
    res.boundary.plot(ax=ax, color="black", lw=1.4)
    for _, r in res.iterrows():
        c = r.geometry.representative_point()
        ax.text(c.x, c.y, r["name"], fontsize=8, ha="center", va="center", fontweight="bold",
                bbox=dict(facecolor="white", alpha=0.8, edgecolor="none", pad=1.2))
    rd.plot(ax=ax, color="#222222", lw=2.4)
    rd.plot(ax=ax, color="#f2f2f2", lw=0.9)
    pl.plot(ax=ax, color=figs.EMBER, edgecolor="black", markersize=28, zorder=4)
    for _, p in pl.iterrows():
        ax.text(p.geometry.x + 180, p.geometry.y + 180, str(p["plot"]), fontsize=7.5, color="black",
                bbox=dict(facecolor="white", alpha=0.7, edgecolor="none", pad=0.6))
    ax.set_xlim(x0, x1)
    ax.set_ylim(y0, y1)
    ax.set_axis_off()
    save(fig, png)


overlay(out / "overlay.png")
overlay(out / "distance.png", show_corridor=True, show_dist=True)

# ------------------------------------------------------------------ 3. burnt into the grid: cell centres or every cell touched
b = read(work / "burnt.tif")[0]
bt = read(work / "burnt_touched.tif")[0]
burn_pal = {0: ("Outside the reserves (0)", "#ece8dc"), 1: (f"1 {names[0]}", "#3b8a5a"), 2: (f"2 {names[1]}", "#d9a441"),
            3: (f"3 {names[2]}", "#2f6f9f")}
classes_png(b, out / "burnt.png", burn_pal, width=760, counts=True, legend_frac=0.30, fontsize=9)
# a zoom on an edge, the polygon's outline over the cells
g = res.geometry.iloc[2]
cx, cy = min(g.exterior.coords, key=lambda p: p[0])
col = int((cx - x0) / 25)
row = int((y1 - cy) / 25)
r0, c0 = max(row - 14, 0), max(col - 6, 0)
for name, arr in (("edge_centre", b), ("edge_touched", bt)):
    fig, ax = plt.subplots(figsize=(4.2, 4.2), dpi=150)
    sub = arr[r0:r0 + 28, c0:c0 + 28]
    e = [x0 + c0 * 25, x0 + (c0 + 28) * 25, y1 - (r0 + 28) * 25, y1 - r0 * 25]
    from matplotlib import colors as mc
    ax.imshow(np.where(np.ma.filled(sub, 0) > 0, 1, 0), cmap=mc.ListedColormap(["#ece8dc", "#3b8a5a"]), vmin=0, vmax=1,
              extent=e, interpolation="nearest")
    for k in range(29):
        ax.axvline(e[0] + k * 25, color="white", lw=0.4)
        ax.axhline(e[2] + k * 25, color="white", lw=0.4)
    res.boundary.plot(ax=ax, color="black", lw=1.6)
    ax.set_xlim(e[0], e[1])
    ax.set_ylim(e[2], e[3])
    ax.set_axis_off()
    save(fig, out / f"{name}.png")

# ------------------------------------------------------------------ 4. the tables
share = figs.read_csv(work / "output" / "forest_share.csv")
cover = figs.read_csv(work / "output" / "cover.csv")
at = figs.read_csv(work / "output" / "at_plots.csv")
around = figs.read_csv(work / "output" / "around_plots.csv")
print("forest_share", share)
print("cover", cover)
print("at plots", at)
print("around plots", around)
for k, r in enumerate(res.itertuples(), 1):
    print(k, r.name, "area ha", round(r.geometry.area / 1e4, 1))
print("burnt cells", {v: int((b == v).sum()) for v in (1, 2, 3)}, "touched", {v: int((bt == v).sum()) for v in (1, 2, 3)})
print("road length m", round(road.length))
dist = read(work / "distance.tif")[0]
print("distance max", float(dist.max()), "within 500 m share", float((dist <= 500).mean()))
for k in sorted(set(int(float(r["value"])) for r in at)):
    print(k, CLC_LEGEND[k][0])
