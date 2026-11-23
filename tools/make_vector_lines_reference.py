# SPDX-License-Identifier: GPL-3.0-or-later
"""What the library's tf.vector gives for points_along, substring, split_lines, locate, shortest_line and
closest_point, for Terraflux's tests of vector.points_along, vector.substring, vector.split_lines, vector.locate,
vector.shortest_line and vector.closest_point: seeded lines (some multi-part, one with a zero-length segment, one
empty), points and polygons written as GeoPackages to tests/data/reference/vectors/ (line_lines.gpkg,
line_points.gpkg, line_polys.gpkg), and each call's result (the geometries as WKB hex, the attributes) to
tests/data/reference/vector_lines.json:

    PYTHONPATH=<terraflux-py>/src python tools/make_vector_lines_reference.py

Run with the library installed or on PYTHONPATH.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import geopandas as gpd
import numpy as np
import shapely
from shapely.geometry import LineString, MultiLineString, Point, Polygon

import terraflux as tf

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def lines() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(31)

    def walk(n, x0, y0, sx=8.0, sy=6.0):
        return LineString(np.c_[x0 + np.cumsum(rng.normal(sx, sy, n)), y0 + np.cumsum(rng.normal(0, sy, n))])

    zero = walk(10, 0, 300)
    xy = list(zero.coords)
    xy.insert(5, xy[4])  # a zero-length segment
    xy.insert(8, xy[7])
    geoms = [
        walk(30, 0, 0),
        walk(12, 0, 200),
        MultiLineString([walk(8, 0, 400), walk(8, 300, 400), walk(5, 600, 400)]),
        LineString(xy),
        LineString([(0, 600), (10, 700)]),
        LineString([(0, 800), (100, 800)]),
        LineString([(500, 0), (600, 0), (600, 100), (500, 0)]),
        walk(50, 0, 900, 3.1, 4.7),
        LineString(),
        MultiLineString([walk(6, 0, 1100), walk(6, 0, 1150)]),
    ]
    n = len(geoms)
    return gpd.GeoDataFrame(
        {
            "id": np.arange(1, n + 1, dtype=float),
            "name": [f"l{i}" for i in range(n)],
            "part": rng.integers(0, 3, n).astype(float),
        },
        geometry=geoms,
        crs=32633,
    )


def points() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(6)
    n = 28
    xy = np.c_[rng.uniform(-50, 700, n), rng.uniform(-50, 1250, n)]
    geoms = [Point(x, y) for x, y in xy]
    geoms[3] = Point(0, 0)  # on the first line's start
    geoms[4] = Point(550, 0)  # on a segment of the closed line
    geoms[5] = Point()
    return gpd.GeoDataFrame(
        {
            "id": np.arange(1, n + 1, dtype=float),
            "name": [f"p{i}" for i in range(n)],
            "distance": rng.uniform(0, 1, n),
            "moved": rng.integers(0, 2, n).astype(float),
        },
        geometry=geoms,
        crs=32633,
    )


def polygons() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(12)
    geoms = []
    for k in range(6):
        cx, cy = rng.uniform(0, 600), rng.uniform(0, 1200)
        a = np.sort(rng.uniform(0, 2 * math.pi, 9))
        r = rng.uniform(15, 40, 9)
        geoms.append(Polygon(np.c_[cx + r * np.cos(a), cy + r * np.sin(a)]))
    geoms.append(Polygon([(0, 0), (60, 0), (60, 60), (0, 60)], [[(20, 20), (20, 40), (40, 40), (40, 20)]]))
    geoms.append(Polygon())
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float)}, geometry=geoms, crs=32633)


def hexed(g):
    if g is None:
        return None
    if shapely.is_empty(g):
        return "EMPTY"
    return shapely.to_wkb(g, hex=True, output_dimension=2, byte_order=1)


def columns(gdf) -> list:
    """[name, values] per attribute, in the order of the columns (None for a missing value)."""
    out = []
    for name in gdf.columns:
        if name == gdf.geometry.name:
            continue
        v = [float(x) if isinstance(x, (bool, np.bool_)) else x for x in gdf[name].tolist()]
        out.append([name, [None if isinstance(x, float) and math.isnan(x) else x for x in v]])
    return out


def main() -> None:
    files = {"line_lines": lines(), "line_points": points(), "line_polys": polygons()}
    (REF / "vectors").mkdir(exist_ok=True)
    for name, gdf in files.items():
        gdf.to_file(REF / "vectors" / f"{name}.gpkg", driver="GPKG")
    cases = []

    def path(f):
        return str(REF / "vectors" / f"{f}.gpkg")

    def case(file, op, other=None, **params):
        args = [path(file)] if other is None else [path(file), path(other)]
        result = getattr(tf.vector, op)(*args, **params)
        cases.append(
            {
                "file": file,
                "other": other,
                "op": f"vector.{op}",
                "params": params,
                "geometries": [hexed(g) for g in result.geometry.values],
                "columns": columns(result),
            }
        )

    L = "line_lines"
    for p in (
        {"spacing": 50.0},
        {"spacing": 17.3, "start": 5.0},
        {"spacing": 0.1 + 0.2, "start": 1.0},
        {"spacing": 1000.0},
        {"spacing": 40.0, "start": 5000.0},
        {"n": 1},
        {"n": 2},
        {"n": 7},
        {"n": 100},
    ):
        case(L, "points_along", **p)
    for s, e, norm in (
        (0.0, 100.0, False),
        (12.5, 12.5, False),
        (0.0, 1e9, False),
        (-5.0, 20.0, False),
        (30.0, 31.0, False),
        (0.25, 0.75, True),
        (0.3, 1.0, True),
        (0.0, 0.0, True),
        (0.9, 1.7, True),
    ):
        case(L, "substring", start=s, end=e, normalized=norm)
    for length in (50.0, 17.7, 100.0, 1000.0, 0.5 + 0.25):
        case(L, "split_lines", length=length)
    for a in ("line_points", "line_polys", L):
        case(a, "locate", other=L)
    for a, b in (("line_points", L), ("line_polys", L), (L, "line_points"), (L, L), ("line_points", "line_polys")):
        case(a, "shortest_line", other=b)
    case("line_points", "shortest_line", other=L, suffix="_b")
    case("line_points", "closest_point", other=L)
    case("line_points", "closest_point", other=L, max_distance=60.0)
    case("line_points", "closest_point", other=L, max_distance=0.0)
    case("line_points", "closest_point", other="line_polys")
    out = {"library": tf.__version__, "geos": shapely.geos_version_string, "cases": cases}
    p = REF / "vector_lines.json"
    p.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(p, len(cases), "cases")


if __name__ == "__main__":
    sys.exit(main())
