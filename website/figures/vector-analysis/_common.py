"""What the figures of the vector analysis tutorials share: the layers they work on, and the drawing helpers.

The layers are made for the tutorials over a 20 x 20 km window of the CORINE Land Cover excerpt that ships with the
library (EPSG:3035, 25 m): the forest, the water and the towns are its own patches (a polygon per patch of connected
cells, as vector.polygonize makes them); the reserves, the road, the stations and the records are drawn for the
tutorials (the records are simulated, with a random generator of a fixed seed). Run `python
website/figures/vector-analysis/select-join-overlay.py` to make them again (they go to the two parts' download
folders, website/static/data/tutorials/vector-analysis/).

The library is read from the branch given by the environment variable TERRAFLUX_PY (a folder with `src/terraflux`),
else from the submodule extern/terraflux-py.
"""

from __future__ import annotations

import os
import pathlib
import shutil
import sys

HERE = pathlib.Path(__file__).resolve().parent
WEB = HERE.parents[1]
sys.path.insert(0, str(WEB / "tools"))
_lib = pathlib.Path(os.environ["TERRAFLUX_PY"]) if os.environ.get("TERRAFLUX_PY") else WEB.parent / "extern" / "terraflux-py"
sys.path.insert(0, str(_lib / "src"))

import figs  # noqa: E402
import geopandas as gpd  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import rasterio  # noqa: E402
import shapely  # noqa: E402
import terraflux as tf  # noqa: E402
from matplotlib.lines import Line2D  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402
from rasterio import features as rfeatures  # noqa: E402
from rasterio.transform import from_origin  # noqa: E402
from shapely.geometry import LineString, Point, box, shape  # noqa: E402

CRS = "EPSG:3035"
WINDOW = (4261250, 3262500, 4281250, 3282500)  # 20 x 20 km, the north-west corner of the CORINE excerpt
PARTS = ("select-join-overlay", "shapes-lines-clusters")
CATEGORY = "vector-analysis"

# the site's colours (figs.py), and the ones these figures add
FOREST, WATER, EMBER, INK, INK2, RULE = figs.FOREST, figs.WATER, figs.EMBER, figs.INK, figs.INK2, figs.RULE
TOWN = "#b4508a"
FARM = "#efe6c8"
GROUND = "#f6f4ec"
RED = "#c0392b"
VIOLET = "#6f5aa8"


def clc_window(window=WINDOW):
    """The CORINE codes of a window of the excerpt and the transform of its cells."""
    with rasterio.open(tf.example_data("clc")) as d:
        a = d.read(1)
    x0, y0, x1, y1 = window
    r0, r1 = int((3282500 - y1) / 25), int((3282500 - y0) / 25)
    c0, c1 = int((x0 - 4261250) / 25), int((x1 - 4261250) / 25)
    return a[r0:r1, c0:c1], from_origin(x0, y1, 25, 25)


def patches(a, T, codes, connectivity=4):
    """The polygons of the patches of the cells with a code in `codes`, with the code of their first cell."""
    m = np.isin(a, codes).astype("uint8")
    out = []
    for g, v in rfeatures.shapes(a.astype("int32"), mask=m == 1, transform=T, connectivity=connectivity):
        out.append((shape(g), int(v)))
    return out


def make_layers() -> dict[str, gpd.GeoDataFrame]:
    """Every layer of the two parts, as GeoDataFrames in EPSG:3035 (metres)."""
    a, T = clc_window()
    x0, y0, x1, y1 = WINDOW
    # forest: the patches of each of the three forest classes, the largest first
    names = {23: "broad-leaved", 24: "coniferous", 25: "mixed"}
    fp = sorted(patches(a, T, [23, 24, 25]), key=lambda p: -p[0].area)
    forest = gpd.GeoDataFrame({"patch": range(1, len(fp) + 1), "type": [names[v] for _, v in fp],
                               "area_ha": [round(g.area / 1e4, 1) for g, _ in fp]},
                              geometry=[g for g, _ in fp], crs=CRS)
    # water: the lakes (water bodies, 41) and the water courses (40)
    wp = sorted(patches(a, T, [40, 41]), key=lambda p: -p[0].area)
    water = gpd.GeoDataFrame({"kind": ["lake" if v == 41 else "river" for _, v in wp],
                              "area_ha": [round(g.area / 1e4, 1) for g, _ in wp]},
                             geometry=[g for g, _ in wp], crs=CRS)
    # towns: urban fabric (1, 2) of at least 20 ha
    tp = [p for p in patches(a, T, [1, 2]) if p[0].area >= 20e4]
    towns = gpd.GeoDataFrame({"town": range(1, len(tp) + 1), "area_ha": [round(g.area / 1e4, 1) for g, _ in tp]},
                             geometry=[g for g, _ in tp], crs=CRS)
    # the study area
    window = gpd.GeoDataFrame({"name": ["study area"]}, geometry=[box(*WINDOW)], crs=CRS)
    # reserves: the three largest forests grown by 150 m, shrunk by 60 and simplified, as a manager would draw them
    res = []
    for k in range(3):
        r = forest.geometry.iloc[k].buffer(150).buffer(-60).simplify(80)
        res.append(shapely.Polygon(r.exterior) if r.geom_type == "Polygon" else r)
    reserves = gpd.GeoDataFrame({"reserve": ["North", "East", "South"], "year": [1984, 1998, 2011]}, geometry=res, crs=CRS)
    # a planned road across the area
    road = LineString([(x0 + 300, y0 + 4200), (x0 + 5200, y0 + 7600), (x0 + 9800, y0 + 8300), (x0 + 13400, y0 + 12600),
                       (x1 - 300, y0 + 15800)])
    roads = gpd.GeoDataFrame({"name": ["planned road"], "lanes": [2]}, geometry=[road], crs=CRS)
    # six monitoring stations, one in each of the six largest forests, on a point guaranteed to lie in it
    st = [forest.geometry.iloc[k].representative_point() for k in range(6)]
    stations = gpd.GeoDataFrame({"station": [f"S{k + 1}" for k in range(6)]}, geometry=st, crs=CRS)
    # simulated records of a forest bird: clusters of points inside the forest and some scattered in the open
    rng = np.random.default_rng(23)
    big = forest[forest.area_ha >= 40].reset_index(drop=True)
    weights = big.area_ha / big.area_ha.sum()
    pts = []
    for _ in range(22):
        g = big.geometry.iloc[rng.choice(len(big), p=weights)]
        c = shapely.get_coordinates(g.representative_point())[0]
        for _ in range(int(rng.integers(8, 20))):
            p = Point(c + rng.normal(0, 380, 2))
            if p.within(box(*WINDOW)):
                pts.append(p)
    for _ in range(60):
        pts.append(Point(x0 + rng.random() * (x1 - x0), y0 + rng.random() * (y1 - y0)))
    order = rng.permutation(len(pts))
    pts = [pts[i] for i in order]
    records = gpd.GeoDataFrame({"record": range(1, len(pts) + 1), "year": rng.integers(2015, 2024, len(pts)),
                                "birds": rng.integers(1, 6, len(pts))}, geometry=pts, crs=CRS)
    return dict(forest=forest, water=water, towns=towns, window=window, reserves=reserves, road=roads,
                stations=stations, records=records)


def make_cover(size_m=5000, corner=(4268000, 3270000)):
    """The patches of every class of a small window, as a polygonized map gives them: (raw, noded).

    The raw polygons have only their own corners, so a neighbour's edge has no vertex where a third patch meets it and
    GEOS does not take them for a coverage. The noded ones (the boundaries noded, then polygonized, each piece given
    the class under its representative point) share every vertex with their neighbours: a valid coverage.
    """
    x0, y0 = corner
    a, T = clc_window((x0, y0, x0 + size_m, y0 + size_m))
    pol = patches(a, T, list(np.unique(a)), connectivity=4)
    raw = gpd.GeoDataFrame({"class": [v for _, v in pol]}, geometry=[g for g, _ in pol], crs=CRS)
    lines = shapely.node(shapely.union_all(shapely.boundary(raw.geometry.values)))
    pieces = shapely.get_parts(shapely.polygonize(shapely.get_parts(lines)))
    ip, ic = shapely.STRtree(raw.geometry.values).query(shapely.point_on_surface(pieces), predicate="within")
    cls = np.zeros(len(pieces), dtype=int)
    cls[ip] = raw["class"].values[ic]
    noded = gpd.GeoDataFrame({"class": cls}, geometry=pieces, crs=CRS)
    return raw, noded


def write_layers(layers: dict, raw, noded, parts=PARTS):
    """The layers as GeoPackages in each part's download folder."""
    for part in parts:
        d = figs.data_dir(f"{CATEGORY}/{part}")
        for name, g in layers.items():
            g.to_file(d / f"{name}.gpkg", driver="GPKG")
    d = figs.data_dir(f"{CATEGORY}/{parts[1]}")
    raw.to_file(d / "cover_raw.gpkg", driver="GPKG")
    noded.to_file(d / "cover.gpkg", driver="GPKG")


def save(fig, png: pathlib.Path):
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.05)
    plt.close(fig)
    figs._shrink(png)


def base_axes(figsize=(6.4, 6.0), window=WINDOW):
    fig, ax = plt.subplots(figsize=figsize)
    ax.set_xlim(window[0], window[2])
    ax.set_ylim(window[1], window[3])
    ax.set_aspect("equal")
    ax.grid(False)
    ax.set_facecolor(GROUND)
    ax.set_xticks([])
    ax.set_yticks([])
    for s in ax.spines.values():
        s.set_visible(True)
        s.set_color(RULE)
    return fig, ax


def draw(ax, g, **kw):
    """A GeoDataFrame or GeoSeries of any kind on an axes."""
    g.plot(ax=ax, **kw)
