"""Shared pieces of the figure scripts of the ecosystem service tutorials.

The tutorials use Ribeirão Claro, an imaginary landscape of 7.2 x 5.4 km in cells of 30 m (240 x 180), in
SIRGAS 2000 / UTM 23S (EPSG:31983): two valleys draining south between hills of 600 to 1100 m, native forest on
the steep slopes, pasture, cropland and eucalyptus plantations below, a town near the outlet and its rivers. It is
made here from random fields with a fixed seed (`landscape()`), with a land use map of 2020, one of 1985 (the
base for rarity) and a scenario of 2040, the rasters each InVEST model needs and its tables. Terraflux's
hydro.* operators trace the rivers and the watersheds. Everything is written to a work folder and zipped to
website/static/data/tutorials/ecosystem-services/ribeirao-claro.zip for the readers.
"""

from __future__ import annotations

import csv
import json
import pathlib
import shutil
import sys
import zipfile

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))
import figs  # noqa: E402

import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

CATEGORY = "ecosystem-services"
H, W = 180, 240
PIXEL = 30.0
ORIGIN = (500000.0, 8000000.0)
CRS = "EPSG:31983"
SEED = 11

CLASSES = {1: ("Forest", "#3b8a5a"), 2: ("Pasture", "#a7c957"), 3: ("Cropland", "#d9a441"),
           4: ("Eucalyptus", "#2a9d8f"), 5: ("Urban", "#b4508a"), 6: ("Water", "#3f7fae")}
NAMES = {k: v[0] for k, v in CLASSES.items()}

# the tables of the models, by class (illustrative values: see the tutorials)
POOLS = {  # Mg C / ha
    "columns": ["lucode", "c_above", "c_below", "c_soil", "c_dead"],
    "rows": [[1, 140, 35, 95, 12], [2, 3, 6, 60, 0.5], [3, 4, 1, 45, 0.5], [4, 75, 18, 70, 6],
             [5, 5, 1, 30, 0], [6, 0, 0, 0, 0]],
}
SENSITIVITY = {
    "columns": ["lucode", "habitat", "crops", "roads", "urban"],
    "rows": [[1, 1.0, 0.6, 0.8, 0.9], [2, 0.3, 0.4, 0.5, 0.6], [3, 0.0, 0.0, 0.0, 0.0], [4, 0.2, 0.3, 0.4, 0.5],
             [5, 0.0, 0.0, 0.0, 0.0], [6, 0.0, 0.0, 0.0, 0.0]],
}
THREATS = [
    {"threat": "crops", "max_dist": 800, "weight": 0.7, "decay": "linear"},
    {"threat": "roads", "max_dist": 1500, "weight": 0.8, "decay": "exponential"},
    {"threat": "urban", "max_dist": 2500, "weight": 1.0, "decay": "exponential"},
]
BIOPHYSICAL_WY = {
    "columns": ["lucode", "lulc_veg", "root_depth", "kc"],
    "rows": [[1, 1, 5000, 1.0], [2, 1, 1500, 0.75], [3, 1, 1200, 0.7], [4, 1, 6000, 1.0], [5, 0, 1, 0.3],
             [6, 0, 1, 1.0]],
}
DEMAND = {"columns": ["lucode", "demand"], "rows": [[1, 0], [2, 5], [3, 150], [4, 0], [5, 900], [6, 0]]}
VALUATION = {
    "columns": ["ws_id", "efficiency", "fraction", "height", "kw_price", "cost", "time_span", "discount"],
    "rows": [[1, 0.8, 0.6, 40, 0.08, 20000, 30, 7], [2, 0.8, 0.6, 40, 0.08, 20000, 30, 7]],
}
BIOPHYSICAL_SDR = {
    "columns": ["lucode", "usle_c", "usle_p"],
    "rows": [[1, 0.001, 1], [2, 0.05, 1], [3, 0.2, 0.5], [4, 0.03, 1], [5, 0.01, 1], [6, 0.0, 1]],
}

PREFIX = "website/.cache/work"


def img(slug: str) -> pathlib.Path:
    return figs.img_dir(f"{CATEGORY}/{slug}")


def data(slug: str) -> pathlib.Path:
    return figs.data_dir(f"{CATEGORY}/{slug}")


def work(slug: str, name: str = "", clean=True) -> pathlib.Path:
    return figs.workdir(f"{CATEGORY}/{slug}" + (f"/{name}" if name else ""), clean=clean)


def palette(classes=None) -> dict:
    return {k: v for k, v in CLASSES.items() if classes is None or k in classes}


def transform():
    from rasterio.transform import from_origin

    return from_origin(ORIGIN[0], ORIGIN[1], PIXEL, PIXEL)


def write(path: pathlib.Path, a: np.ndarray, dtype="float32", nodata=None):
    import rasterio

    a = np.asarray(a)
    prof = dict(driver="GTiff", height=a.shape[0], width=a.shape[1], count=1, dtype=dtype, crs=CRS,
                transform=transform(), nodata=nodata, compress="deflate")
    with rasterio.open(path, "w", **prof) as ds:
        ds.write(a.astype(dtype), 1)


def write_table(path: pathlib.Path, t: dict):
    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(t["columns"])
        for r in t["rows"]:
            w.writerow([int(v) if float(v).is_integer() and abs(v) < 1e9 else v for v in r])


def grid(path: pathlib.Path, band: int = 1) -> np.ndarray:
    a, _ = figs.read(path, band)
    return a.filled(np.nan)


# ------------------------------------------------------------------------------------------------ the landscape


def _field(rng, scale, shape=(H, W)):
    from scipy.ndimage import gaussian_filter

    f = gaussian_filter(rng.standard_normal(shape), scale, mode="reflect")
    return (f - f.mean()) / f.std()


def _norm(a):
    return (a - np.nanmin(a)) / (np.nanmax(a) - np.nanmin(a))


def _line(mask, pts, width=0):
    """Cells along a polyline of (row, col) points."""
    for (r0, c0), (r1, c1) in zip(pts[:-1], pts[1:]):
        n = int(max(abs(r1 - r0), abs(c1 - c0)) * 3) + 1
        for t in np.linspace(0, 1, n):
            r, c = int(round(r0 + (r1 - r0) * t)), int(round(c0 + (c1 - c0) * t))
            mask[max(r - width, 0):r + width + 1, max(c - width, 0):c + width + 1] = True


def _polygonize(labels: np.ndarray, field: str):
    """Polygons of the positive labels, as a GeoDataFrame with the label as `field`."""
    import geopandas as gpd
    from rasterio import features
    from shapely.geometry import shape
    from shapely.ops import unary_union

    rows = {}
    for geom, v in features.shapes(labels.astype("int32"), mask=labels > 0, transform=transform()):
        rows.setdefault(int(v), []).append(shape(geom))
    ids = sorted(rows)
    return gpd.GeoDataFrame({field: ids}, geometry=[unary_union(rows[i]) for i in ids], crs=CRS)


def landscape(clean=False) -> pathlib.Path:
    """The files of Ribeirão Claro in website/.cache/work/ecosystem-services/landscape (made once)."""
    d = figs.WEB / ".cache" / "work" / CATEGORY / "landscape"
    if (d / "done").exists() and not clean:
        return d
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    rng = np.random.default_rng(SEED)
    y, x = np.mgrid[0:H, 0:W].astype("float64")

    # -- elevation: higher in the north, two valleys running south with a ridge between, hills on them
    xs = x + 14 * _field(rng, 28)
    z = 700 + 260 * (1 - y / H) ** 1.3
    z += 85 * np.cos(4 * np.pi * xs / W)
    z += 38 * _field(rng, 10) + 10 * _field(rng, 3)
    z -= 50 * np.exp(-((y - H) / 40) ** 2)  # the plain at the outlets
    dem = z.astype("float32")
    write(d / "dem.tif", dem)

    # rivers and slopes from Terraflux
    model = f'''format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = {{ path = "dem.tif" }}

[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = {{ dem = "dem.raster" }}

[[nodes]]
id = "acc"
op = "hydro.flow_accumulation"
inputs = {{ dem = "filled.map" }}

[[nodes]]
id = "slope"
op = "surface.terrain"
inputs = {{ map = "dem.raster" }}
params = {{ v = "slope" }}
'''
    run(model, d, {"acc.map": "acc.tif", "slope.map": "slope.tif", "filled.map": "filled.tif"})
    acc = grid(d / "acc.tif")
    slope = grid(d / "slope.tif")
    if np.isnan(slope).any():  # the border: the nearest cell's slope
        from scipy.ndimage import distance_transform_edt

        idx = distance_transform_edt(np.isnan(slope), return_distances=False, return_indices=True)
        slope = slope[tuple(idx)]
    acc = np.nan_to_num(acc)

    river = acc > 900
    near_river = _near(river, 2)

    # -- land use 2020
    s = 0.55 * _norm(slope) + 0.25 * _norm(dem) + 0.25 * _norm(_field(rng, 5))
    town_r, town_c = H - 18, int(W * 0.62)
    dist_town = np.hypot((y - town_r) * 1.0, (x - town_c) * 1.25)
    town_noise = 4 * _field(rng, 3)

    def build(forest_q, town_radius, crops_q, plantations):
        lu = np.full((H, W), 2, "int16")
        lu[s > np.quantile(s, forest_q)] = 1
        rest = lu == 2
        gentle = _norm(slope) + 0.35 * _norm(_field(np.random.default_rng(SEED + 5), 4))
        lu[rest & (gentle < np.quantile(gentle[rest], crops_q))] = 3
        for (r0, c0, h, w) in plantations:
            block = np.zeros((H, W), bool)
            block[r0:r0 + h, c0:c0 + w] = True
            lu[block & (lu != 1)] = 4
        lu[dist_town + town_noise < town_radius] = 5
        lu[river] = 6
        return lu

    plantations = [(30, 20, 14, 22), (70, 150, 12, 18), (100, 200, 16, 20), (120, 40, 10, 16)]
    cur = build(0.60, 15, 0.35, plantations)
    base = build(0.42, 7, 0.20, [])  # 1985: more forest, a village, no plantations

    # 2040: the forest edges on gentle ground cleared, crops on the pasture of gentle slopes, the town grows,
    # a new plantation, and the pasture and crops along the rivers restored to forest
    fut = cur.copy()
    forest = cur == 1
    edge = forest & _near(~forest & (cur != 6), 3)
    clear = edge & (s < np.quantile(s[forest], 0.45))
    fut[clear] = 2
    past = fut == 2
    gentle = _norm(slope) + 0.3 * _norm(_field(rng, 4))
    fut[past & (gentle < np.quantile(gentle[past], 0.35))] = 3
    block = np.zeros((H, W), bool)
    block[40:56, 175:200] = True
    fut[block & (fut != 1) & (fut != 6)] = 4
    fut[(dist_town + town_noise < 24) & (fut != 6)] = 5
    riparian = _near(river, 2) & ~river & np.isin(fut, [2, 3])
    fut[riparian] = 1
    fut[river] = 6
    for name, lu in (("lulc_2020", cur), ("lulc_1985", base), ("lulc_2040", fut)):
        write(d / f"{name}.tif", lu, "int16", nodata=0)

    # -- threats: crops, roads, the town
    roads = np.zeros((H, W), bool)
    _line(roads, [(H - 1, town_c + 4), (town_r, town_c), (120, 150), (80, 160), (40, 150), (0, 140)])
    _line(roads, [(town_r, town_c), (H - 25, 120), (H - 40, 60), (H - 30, 0)])
    _line(roads, [(80, 160), (70, 200), (60, W - 1)])
    roads_fut = roads.copy()
    _line(roads_fut, [(H - 40, 60), (110, 55), (60, 40), (20, 60)])  # a new road into the western hills
    write(d / "crops_2020.tif", (cur == 3).astype("float32"))
    write(d / "roads_2020.tif", roads.astype("float32"))
    write(d / "urban_2020.tif", (cur == 5).astype("float32"))
    write(d / "crops_2040.tif", (fut == 3).astype("float32"))
    write(d / "roads_2040.tif", roads_fut.astype("float32"))
    write(d / "urban_2040.tif", (fut == 5).astype("float32"))

    # -- climate and soils
    precip = 1350 + 1.1 * (dem - 650) + 25 * _field(rng, 20)
    eto = 1250 - 0.45 * (dem - 650) + 15 * _field(rng, 20)
    depth = 600 + 2200 * (1 - _norm(slope)) ** 2 * (0.6 + 0.4 * _norm(_field(rng, 8)))
    pawc = 0.10 + 0.12 * _norm(_field(rng, 12))
    write(d / "precipitation.tif", precip.round(1))
    write(d / "eto.tif", eto.round(1))
    write(d / "depth.tif", depth.round(0))
    write(d / "pawc.tif", pawc.round(3))
    erosivity = 5200 + 3.0 * (precip - 1350)
    erodibility = 0.022 + 0.018 * _norm(_field(rng, 9))
    write(d / "erosivity.tif", erosivity.round(0))
    write(d / "erodibility.tif", erodibility.round(4))
    np.save(d / "river.npy", river)

    # -- watersheds: the areas draining to the two outlets on the southern edge, and sub-watersheds above two
    # points on each main river
    edge_cells = [(H - 1, c) for c in range(W)]
    acc_edge = np.array([acc[r, c] for r, c in edge_cells])
    outlets = []
    for c in np.argsort(acc_edge)[::-1]:
        if all(abs(c - o) > 40 for o in outlets):
            outlets.append(int(c))
        if len(outlets) == 2:
            break
    outlets.sort()
    seeds = np.zeros((H, W), "float32")
    for i, c in enumerate(outlets):
        seeds[H - 1, c] = i + 1
    write(d / "outlets.tif", seeds)
    model = f'''format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = {{ path = "filled.tif" }}

[[nodes]]
id = "seeds"
op = "raster.read"
params = {{ path = "outlets.tif" }}

[[nodes]]
id = "ws"
op = "hydro.watersheds"
inputs = {{ dem = "dem.raster", seeds = "seeds.raster" }}
'''
    figs.run(figs.add_writes(model, {"ws.map": "ws.tif"}), d, "-q")
    ws = grid(d / "ws.tif")
    # sub-watersheds: the outlets and, on each main river, the cell halfway up its accumulation
    sub = seeds.copy()
    k = 3
    for i, c in enumerate(outlets):
        inside = (ws == i + 1) & river
        target = acc[H - 1, c] * 0.45
        rr, cc = np.nonzero(inside)
        j = np.argmin(np.abs(acc[rr, cc] - target))
        sub[H - 1, c] = 0
        sub[rr[j], cc[j]] = k
        sub[H - 1, c] = k + 1
        k += 2
    write(d / "subseeds.tif", sub)
    model2 = model.replace("outlets.tif", "subseeds.tif")
    figs.run(figs.add_writes(model2, {"ws.map": "subws.tif"}), d, "-q", name="model2.toml")
    subws = grid(d / "subws.tif")
    ws_i = np.nan_to_num(ws).astype("int32")
    sub_i = np.nan_to_num(subws).astype("int32")
    # renumber the sub-watersheds 1..4
    remap = {v: i + 1 for i, v in enumerate(sorted(set(np.unique(sub_i)) - {0}))}
    sub_i = np.vectorize(lambda v: remap.get(v, 0))(sub_i)
    _polygonize(ws_i, "ws_id").to_file(d / "watersheds.gpkg", driver="GPKG")
    _polygonize(sub_i, "subws_id").to_file(d / "subwatersheds.gpkg", driver="GPKG")

    # -- access to threats: a state park in the northern forest (0) and its buffer zone (0.5)
    import geopandas as gpd
    from shapely.geometry import Polygon

    def ring(pts):
        return Polygon([(ORIGIN[0] + c * PIXEL, ORIGIN[1] - r * PIXEL) for r, c in pts])

    park = ring([(5, 70), (5, 120), (35, 128), (55, 110), (50, 75), (30, 62)])
    buffer = ring([(0, 55), (0, 135), (40, 142), (66, 115), (62, 68), (35, 48)]).difference(park)
    gpd.GeoDataFrame({"name": ["park", "buffer zone"], "access": [0.0, 0.5]}, geometry=[park, buffer],
                     crs=CRS).to_file(d / "access.gpkg", driver="GPKG")

    # -- the tables
    for name, t in (("carbon_pools", POOLS), ("sensitivity", SENSITIVITY), ("biophysical_wy", BIOPHYSICAL_WY),
                    ("demand", DEMAND), ("valuation", VALUATION), ("biophysical_sdr", BIOPHYSICAL_SDR)):
        write_table(d / f"{name}.csv", t)
    with open(d / "threats.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["threat", "max_dist", "weight", "decay"])
        for t in THREATS:
            w.writerow([t["threat"], t["max_dist"], t["weight"], t["decay"]])
    (d / "done").write_text("ok")
    return d


def _near(mask, cells):
    from scipy.ndimage import binary_dilation

    return binary_dilation(mask, iterations=cells)


FILES = ["dem.tif", "lulc_1985.tif", "lulc_2020.tif", "lulc_2040.tif", "crops_2020.tif", "roads_2020.tif",
         "urban_2020.tif", "crops_2040.tif", "roads_2040.tif", "urban_2040.tif", "precipitation.tif", "eto.tif",
         "depth.tif", "pawc.tif", "erosivity.tif", "erodibility.tif", "watersheds.gpkg", "subwatersheds.gpkg",
         "access.gpkg", "carbon_pools.csv", "threats.csv", "sensitivity.csv", "biophysical_wy.csv", "demand.csv",
         "valuation.csv", "biophysical_sdr.csv"]


def copy_inputs(dst: pathlib.Path, names=None):
    src = landscape()
    for n in names or FILES:
        shutil.copy(src / n, dst / n)


def zip_landscape(dst: pathlib.Path):
    src = landscape()
    with zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED) as z:
        for n in FILES:
            z.write(src / n, f"ribeirao-claro/{n}")


def run(model: str, w: pathlib.Path, writes: dict, name="model.toml"):
    r = figs.run(figs.add_writes(model, writes), w, "-q", name=name, check=False)
    if r.returncode != 0:
        raise SystemExit(f"terraflux run failed in {w}:\n{r.stdout}\n{r.stderr}")
    return r


def land_map(a, png, title=None, classes=None, width=900, legend=True, counts=False):
    vals = {int(v) for v in np.unique(a[~np.isnan(a)])}
    return figs.raster(None, png, array=a, palette=palette(classes or vals), title=title, width=width,
                       legend=legend, counts=counts)


def toml_table(t: dict) -> str:
    rows = ", ".join("[" + ", ".join(_num(v) for v in r) + "]" for r in t["rows"])
    cols = ", ".join(f'"{c}"' for c in t["columns"])
    return f"{{ columns = [{cols}], rows = [{rows}] }}"


def _num(v):
    return str(int(v)) if float(v).is_integer() else repr(float(v))


__all__ = ["figs", "plt", "np", "mcolors", "Patch", "json"]
