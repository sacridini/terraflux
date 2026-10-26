"""Figures of the tutorial "Map algebra from scratch" (building-models/map-algebra).

Sibuyan Island (Philippines), land use of 1997 and its drivers at 250 m (lulcc's and landschaft's example data):
a habitat suitability index, buffers, an edge mask and the suitable habitat above a threshold, all computed by
raster.calc in real runs of Terraflux.
"""

import shutil
import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.patches import Rectangle  # noqa: E402

NAME = "building-models/map-algebra"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
shutil.copy(INPUTS / "sibuyan_landuse.tif", work / "landuse.tif")
shutil.copy(INPUTS / "sibuyan_drivers.tif", work / "drivers.tif")

SIBUYAN = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Rice", "#e6d36a"),
           5: ("Other", "#b4508a")}
CELL_KM2 = 0.25 * 0.25

HEAD = """format = "terraflux/1"

[inputs.land]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"
"""


def calc(id_, expression, **inputs):
    ins = ", ".join(f'{k} = "{v}"' for k, v in inputs.items())
    return f"""
[[nodes]]
id = "{id_}"
op = "raster.calc"
inputs = {{ {ins} }}
params = {{ expression = "{expression}" }}
"""


HSI = "sqrt(si_elev * si_road) if land == 1 else 0"
nodes = [
    calc("si_elev", "clamp((1200 - d.elevation) / 900, 0, 1)", d="$drivers"),
    calc("si_road", "clamp(d.dist_roads / 3000, 0, 1)", d="$drivers"),
    calc("hsi", HSI, land="$land", si_elev="si_elev.map", si_road="si_road.map"),
    calc("road_buffer", "d.dist_roads <= 500", d="$drivers"),
    calc("farm_buffer", "nbmax(land == 2 or land == 4, 2) if land == 1 else None", land="$land"),
    calc("filled", "coalesce(land, 0)", land="$land"),
    calc("share", "nbmean(land == 1, 2)", land="$land"),
]
for r in (1, 2, 3):
    nodes.append(calc(f"edge{r}", f"if land == 1 then 2 - (nbmin(land == 1, {r}) == 0) else 0", land="$land"))
nodes.append(calc("edge_coast", "2 - (nbmin(coalesce(land, 0) == 1, 1) == 0) if land == 1 else 0", land="$land"))
for t in range(1, 10):
    nodes.append(calc(f"suit{t}", f"hsi >= {t / 10}", hsi="hsi.map"))

writes = {"si_elev.map": "si_elev.tif", "si_road.map": "si_road.tif", "hsi.map": "hsi.tif",
          "road_buffer.map": "road_buffer.tif", "farm_buffer.map": "farm_buffer.tif", "filled.map": "filled.tif",
          "share.map": "share.tif"}
writes.update({f"edge{r}.map": f"edge{r}.tif" for r in (1, 2, 3)})
writes["edge_coast.map"] = "edge_coast.tif"
writes.update({f"suit{t}.map": f"suit{t}.tif" for t in range(1, 10)})
model = figs.add_writes(HEAD + "".join(nodes), writes)
figs.run(model, work)

# ------------------------------------------------------------------------------------------------ the data
figs.raster(work / "landuse.tif", out / "landuse.png", palette=SIBUYAN, counts=True, width=760,
            title="Sibuyan, land use in 1997")
drivers, info = figs.read(work / "drivers.tif")
names = list(info["descriptions"])
for name, unit, cmap in (("elevation", "m", "terrain"), ("dist_roads", "m", "magma_r"), ("slope", "degrees", "viridis")):
    b = names.index(name) + 1
    figs.raster(work / "drivers.tif", out / f"driver_{name}.png", band=b, cmap=cmap, unit=unit, width=640,
                title=f"drivers.{name}")

# ------------------------------------------------------------------------------------------------ the index
for f, title in (("si_elev", "si_elev: lower is better"), ("si_road", "si_road: far from roads is better"),
                 ("hsi", "Habitat suitability index")):
    figs.raster(work / f"{f}.tif", out / f"{f}.png", cmap="viridis", vmin=0, vmax=1, width=640, title=title)

figs.raster(work / "road_buffer.tif", out / "road_buffer.png", colors={0: "#e9e4d8", 1: "#d55e00"},
            labels={0: "Farther", 1: "Within 500 m"}, width=640, title="d.dist_roads <= 500")
figs.raster(work / "farm_buffer.tif", out / "farm_buffer.png", colors={0: "#3b8a5a", 1: "#e8901f"},
            labels={0: "Farther", 1: "Within 2 cells"}, width=640,
            title="nbmax(land == 2 or land == 4, 2)")
figs.raster(work / "share.tif", out / "share.png", cmap="Greens", vmin=0, vmax=1, unit="share of forest", width=640,
            title="nbmean(land == 1, 2)")
figs.raster(work / "filled.tif", out / "filled.png", palette={0: ("0 (the sea)", "#c9d6df"), **SIBUYAN},
            width=640, title="coalesce(land, 0)")

EDGE = {0: ("Not forest", "#e9e4d8"), 1: ("Edge", "#e8901f"), 2: ("Interior", "#1f5a3a")}
for r in (1, 2, 3):
    figs.raster(work / f"edge{r}.tif", out / f"edge{r}.png", palette=EDGE, counts=True, width=640,
                title=f"Edge {r * 250} m deep (radius {r})")

figs.raster(work / "edge_coast.tif", out / "edge_coast.png", palette=EDGE, counts=True, width=640,
            title="The sea counted as not forest")
for f in ("edge1", "edge_coast"):
    figs.raster(work / f"{f}.tif", out / f"{f}_plain.png", palette=EDGE, legend=False, width=640)

# ------------------------------------------------------------------------------------------------ the threshold
rows = []
hsi = figs.read(work / "hsi.tif")[0]
for t in range(1, 10):
    a = figs.read(work / f"suit{t}.tif")[0]
    figs.raster(work / f"suit{t}.tif", out / f"suit_{t / 10:.1f}.png", colors={0: "#e9e4d8", 1: "#1f5a3a"},
                labels={0: "Below the threshold", 1: "Suitable habitat"}, width=700)
    area = float((a == 1).sum()) * CELL_KM2
    rows.append({"threshold": t / 10, "area": round(area, 2)})
figs.write_csv(data / "suitable_area.csv", rows)
print("suitable area:", rows)
forest = figs.read(work / "landuse.tif")[0]
print("forest km2", float((forest == 1).sum()) * CELL_KM2, "island km2", forest.count() * CELL_KM2)
for r in (1, 2, 3):
    e = figs.read(work / f"edge{r}.tif")[0]
    print("edge", r, "interior km2", float((e == 2).sum()) * CELL_KM2, "edge km2", float((e == 1).sum()) * CELL_KM2)
fb = figs.read(work / "farm_buffer.tif")[0]
print("forest near farms", float((fb == 1).sum()) * CELL_KM2)
print("hsi mean over forest", float(hsi[forest == 1].mean()))

# ------------------------------------------------------------------------------------------------ explanations
# a toy grid: a land map, then land == 1, then nbcount(land == 1, 1), cell by cell
toy = np.array([
    [1, 1, 1, 2, 2, 2],
    [1, 1, 1, 1, 2, 2],
    [1, 1, 2, 1, 2, 4],
    [1, 2, 2, 2, 4, 4],
    [np.nan, 2, 2, 4, 4, 4],
    [np.nan, np.nan, 2, 2, 4, 4],
])
is1 = np.where(np.isnan(toy), np.nan, (toy == 1).astype(float))
# the neighbourhood sums by Terraflux itself, on the toy grid written as a raster
import rasterio  # noqa: E402
from rasterio.transform import from_origin  # noqa: E402

with rasterio.open(work / "toy.tif", "w", driver="GTiff", width=6, height=6, count=1, dtype="float32", nodata=-1,
                   transform=from_origin(0, 6, 1, 1)) as ds:
    ds.write(np.where(np.isnan(toy), -1, toy).astype("float32"), 1)
toy_model = """format = "terraflux/1"

[inputs.land]
kind = "map"
path = "toy.tif"
""" + calc("nb", "nbsum(land == 1, 1)", land="$land")
figs.run(figs.add_writes(toy_model, {"nb.map": "toy_nb.tif"}), work, name="toy.toml")
nb = figs.read(work / "toy_nb.tif")[0].filled(np.nan)
colors = {1: SIBUYAN[1][1], 2: SIBUYAN[2][1], 4: SIBUYAN[4][1]}

fig, axes = plt.subplots(1, 3, figsize=(10.5, 3.9), dpi=150)
titles = ["land", "land == 1", "nbsum(land == 1, 1)"]
for k, (ax, grid) in enumerate(zip(axes, (toy, is1, nb))):
    ax.set_xlim(0, 6)
    ax.set_ylim(6, 0)
    ax.set_aspect("equal")
    ax.axis("off")
    ax.set_title(titles[k], fontsize=11.5, family="monospace", loc="left")
    for y in range(6):
        for x in range(6):
            v = grid[y, x]
            if np.isnan(v):
                fc, txt, tc = "#ffffff", "None", "#9aa39d"
            elif k == 0:
                fc, txt, tc = colors[int(v)], str(int(v)), "white" if v == 1 else figs.INK
            elif k == 1:
                fc, txt, tc = ("#1f5a3a", "1", "white") if v == 1 else ("#eef1ec", "0", figs.INK2)
            else:
                fc = plt.get_cmap("Greens")(0.15 + v / 9 * 0.8)
                txt, tc = str(int(v)), "white" if v >= 5 else figs.INK
            ax.add_patch(Rectangle((x, y), 1, 1, facecolor=fc, edgecolor="white", lw=1.5))
            ax.text(x + 0.5, y + 0.52, txt, ha="center", va="center", fontsize=10 if txt != "None" else 8, color=tc)
    if k == 2:
        ax.add_patch(Rectangle((1, 1), 3, 3, fill=False, edgecolor=figs.EMBER, lw=2.4))
    if k == 1:
        ax.add_patch(Rectangle((1, 1), 3, 3, fill=False, edgecolor=figs.EMBER, lw=2.4, ls=(0, (3, 2))))
fig.tight_layout()
fig.savefig(out / "toy_grid.png", dpi=150)
plt.close(fig)
figs._shrink(out / "toy_grid.png")
print("done")
