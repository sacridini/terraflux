# SPDX-License-Identifier: GPL-3.0-or-later
"""What tf.vector gives for the spatial predicates, joins, nearest and overlay, for Terraflux's tests of
vector.select_by_location, vector.spatial_join, vector.nearest and vector.overlay.

Seeded layers, written as GeoPackages in tests/data/reference/vectors (pred_points, pred_lines, pred_polys as the
first layer; pred_zones as the second; EPSG:32723, coordinates on a grid of 150 m so that many features sit exactly on
the edges and vertices of the zones), and what the library gives on them, as index lists, in
tests/data/reference/vector_predicates.json:

    PYTHONPATH=<library>/src python tools/make_vector_predicates_reference.py

Run with the library whose tf.vector is wanted on the path (and shapely 2.1, GEOS 3.13).
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import geopandas as gpd
import numpy as np
import shapely
import terraflux as tf
from shapely.geometry import LineString, Point, Polygon, box

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
CRS = "EPSG:32723"
STEP = 150.0
PREDICATES = ("intersects", "within", "contains", "touches", "crosses", "overlaps", "covers", "covered_by",
              "contains_properly", "equals", "disjoint", "dwithin")
DISTANCE = 60.0
MAX_DISTANCE = 80.0
HOWS = ("intersection", "union", "difference", "symmetric_difference", "identity")


def num(v):
    v = float(v)
    return None if math.isnan(v) else v


def text(v):
    return None if v is None or (isinstance(v, float) and math.isnan(v)) else str(v)


def zones() -> gpd.GeoDataFrame:
    """40 polygons: a 6 x 6 grid of squares sharing edges (two with a hole), and four overlapping ones."""
    geoms = []
    for r in range(6):
        for c in range(6):
            x0, y0 = c * STEP, r * STEP
            sq = box(x0, y0, x0 + STEP, y0 + STEP)
            if (r, c) in ((1, 1), (4, 3)):
                sq = Polygon(sq.exterior.coords, [box(x0 + 30, y0 + 30, x0 + 120, y0 + 120).exterior.coords])
            geoms.append(sq)
    geoms += [
        box(100, 100, 400, 350),
        Point(600, 600).buffer(120, quad_segs=4),
        box(700, 100, 1000, 300),
        Polygon([(-80, 300), (200, 480), (-80, 700)]),
    ]
    n = len(geoms)
    rng = np.random.default_rng(5)
    val = np.round(rng.uniform(1, 50, n), 3)
    val[[3, 17, 29]] = np.nan
    return gpd.GeoDataFrame(
        {"zid": np.arange(n, dtype=float), "val": val, "name": [f"z{i}" for i in range(n)]},
        geometry=geoms, crs=CRS,
    )


def points() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(7)
    xy = [tuple(p) for p in np.round(rng.uniform(-60, 960, (110, 2)), 2)]
    on_x = [(rng.integers(0, 7) * STEP, round(rng.uniform(0, 900), 2)) for _ in range(60)]
    on_y = [(round(rng.uniform(0, 900), 2), rng.integers(0, 7) * STEP) for _ in range(60)]
    vertices = [(i * STEP, j * STEP) for i in range(7) for j in range(7) if (i + j) % 3 == 0]
    holes = [(x, y) for x in (180, 210, 270) for y in (180, 210, 270)] + [(480 + 30, 600 + 30), (600, 660)]
    far = [(5000, 5000), (-700, 400), (1300, 20), (1000.0, 1000.0)]
    xy += [(float(x), float(y)) for x, y in on_x + on_y + vertices + holes + far]
    n = len(xy)
    return gpd.GeoDataFrame(
        {"aid": np.arange(n, dtype=float), "num": np.round(rng.uniform(0, 10, n), 2),
         "name": [f"p{i}" for i in range(n)]},
        geometry=[Point(p) for p in xy], crs=CRS,
    )


def lines() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(9)
    geoms = [
        LineString([(0, 0), (900, 0)]),  # along the bottom edge
        LineString([(150, -50), (150, 950)]),  # along a shared edge
        LineString([(0, 150), (450, 150), (450, 450)]),  # along edges, turning at vertices
        LineString([(-100, -100), (1000, 1000)]),  # the diagonal through vertices
        LineString([(175, 175), (245, 245)]),  # inside a hole
        LineString([(30, 30), (120, 120)]),  # inside a square
        LineString([(600, 600), (1500, 600)]),  # from the centre of a disc out
        LineString([(2000, 2000), (2100, 2100)]),  # far away
    ]
    for _ in range(12):
        k = int(rng.integers(2, 5))
        geoms.append(LineString([tuple(p) for p in np.round(rng.uniform(-50, 950, (k, 2)), 1)]))
    n = len(geoms)
    return gpd.GeoDataFrame(
        {"aid": np.arange(n, dtype=float), "num": np.round(rng.uniform(0, 10, n), 2),
         "name": [f"l{i}" for i in range(n)]},
        geometry=geoms, crs=CRS,
    )


def polys() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(13)
    q = STEP / 2
    geoms = []
    for _ in range(24):
        x0, y0 = rng.integers(-1, 11, 2) * q
        w, h = rng.integers(1, 5, 2) * q
        geoms.append(box(x0, y0, x0 + w, y0 + h))
    geoms[5] = box(300, 300, 450, 450)  # equals a zone
    geoms[6] = geoms[3]  # a duplicate
    geoms += [
        Polygon(box(100, 100, 700, 500).exterior.coords, [box(250, 200, 400, 400).exterior.coords]),  # a hole
        Polygon(box(0, 600, 500, 900).exterior.coords, [box(75, 675, 150, 750).exterior.coords,
                                                         box(300, 700, 450, 850).exterior.coords]),
        Point(300, 300).buffer(100, quad_segs=4),
        Polygon([(500, 0), (950, 100), (800, 400)]),
        box(1200, 1200, 1300, 1300),  # far away
        box(0, 0, 150, 150),  # a zone
    ]
    n = len(geoms)
    return gpd.GeoDataFrame(
        {"aid": np.arange(n, dtype=float), "num": np.round(rng.uniform(0, 10, n), 2),
         "name": [f"g{i}" for i in range(n)]},
        geometry=geoms, crs=CRS,
    )


def measure(g, kind):
    if kind == "polys":
        return float(g.area)
    if kind == "lines":
        return float(g.length)
    return float(shapely.get_num_geometries(g))


def main() -> None:
    a_layers = {"points": points(), "lines": lines(), "polys": polys()}
    b = zones()
    out_dir = REF / "vectors"
    for name, gdf in a_layers.items():
        gdf.to_file(out_dir / f"pred_{name}.gpkg", driver="GPKG")
    b.to_file(out_dir / "pred_zones.gpkg", driver="GPKG")
    out: dict = {"terraflux": tf.__version__, "geos": ".".join(map(str, shapely.geos_version)),
                 "distance": DISTANCE, "max_distance": MAX_DISTANCE, "predicates": list(PREDICATES),
                 "relate": {}, "count": {}, "pairs": {}, "first": {}, "summary": {}, "nearest": {},
                 "nearest_max": {}, "overlay": {}}
    for name, a in a_layers.items():
        # what the files hold, as the app reads them
        a = gpd.read_file(out_dir / f"pred_{name}.gpkg")
        bb = gpd.read_file(out_dir / "pred_zones.gpkg")
        for key in ("relate", "count", "pairs", "first", "summary"):
            out[key][name] = {}
        for p in PREDICATES:
            d = DISTANCE if p == "dwithin" else None
            out["relate"][name][p] = [int(i) for i in np.flatnonzero(tf.vector.relate(a, bb, p, distance=d))]
            out["count"][name][p] = [int(i) for i in tf.vector.count(a, bb, p, distance=d)]
            if p == "disjoint":
                continue
            j = tf.vector.join(a, bb, p, how="all", distance=d)
            out["pairs"][name][p] = [[int(i), -1 if math.isnan(z) else int(z)] for i, z in zip(j["aid"], j["zid"])]
            j = tf.vector.join(a, bb, p, how="first", distance=d)
            out["first"][name][p] = {"zid": [num(v) for v in j["zid"]], "name_2": [text(v) for v in j["name_2"]],
                                     "val": [num(v) for v in j["val"]]}
            j = tf.vector.join(a, bb, p, how="summary", fields="val,zid", distance=d)
            out["summary"][name][p] = {c: [num(v) for v in j[c]] for c in ("count", "val_sum", "val_mean", "zid_sum")}
        for key, md in (("nearest", None), ("nearest_max", MAX_DISTANCE)):
            n = tf.vector.nearest(a, bb, max_distance=md)
            out[key][name] = {"zid": [num(v) for v in n["zid"]], "distance": [num(v) for v in n["distance"]]}
        out["overlay"][name] = {}
        for how in HOWS:
            if name != "polys" and how in ("union", "symmetric_difference"):
                continue
            r = tf.vector.overlay(a, bb, how)
            kind = name
            ia = [-1 if math.isnan(v) else int(v) for v in r["aid"]]
            ib = [-1 if math.isnan(v) else int(v) for v in r["zid"]] if "zid" in r else [-1] * len(r)
            out["overlay"][name][how] = [[i, j, measure(g, kind)] for i, j, g in zip(ia, ib, r.geometry)]
    path = REF / "vector_predicates.json"
    path.write_text(json.dumps(out, separators=(",", ":")) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
