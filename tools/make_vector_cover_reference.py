# SPDX-License-Identifier: GPL-3.0-or-later
"""What the library's tf.vector gives for smooth, coverage_node, coverage_simplify and coverage_validate, for
Terraflux's tests of vector.smooth, vector.coverage_node, vector.coverage_simplify and vector.coverage_validate: seeded
layers (a polygonized staircase, lines, polygons with holes, a 4 x 4 coverage of jagged polygons, a coverage with an
overlap and gaps, a noisier polygonized map and the staircase reprojected) written as GeoPackages to
tests/data/reference/vectors/ (cover_stairs.gpkg, cover_lines.gpkg, cover_polys.gpkg, cover_jagged.gpkg,
cover_bad.gpkg, cover_gap.gpkg, cover_noisy.gpkg, cover_reprojected.gpkg), and each call's result (the geometries as
WKB hex, the attributes; a bool as 0/1) to tests/data/reference/vector_cover.json:

    PYTHONPATH=<terraflux-py>/src python tools/make_vector_cover_reference.py

Run with the library installed or on PYTHONPATH (shapely 2.1 for the coverage functions).
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import geopandas as gpd
import numpy as np
import shapely
from rasterio.features import shapes
from shapely.geometry import LineString, MultiLineString, MultiPolygon, Point, Polygon, shape

import terraflux as tf

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def stairs() -> gpd.GeoDataFrame:
    """A seeded 4-class raster of 16 x 16 cells of 10 m, polygonized: staircase outlines, holes and multipolygons."""
    rng = np.random.default_rng(31)
    coarse = rng.integers(1, 5, (4, 4))
    img = np.kron(coarse, np.ones((4, 4), dtype=int))
    flip = rng.random(img.shape) < 0.12
    img = np.where(flip, rng.integers(1, 5, img.shape), img).astype("int32")
    # rasterio's transform: x right, y down from the top-left corner
    from rasterio.transform import from_origin

    geoms, cls = [], []
    for g, v in shapes(img, transform=from_origin(0, 160, 10, 10)):
        geoms.append(shape(g))
        cls.append(float(v))
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float), "cls": cls}, geometry=geoms, crs=32633)


def noisy() -> gpd.GeoDataFrame:
    """A seeded 5-class raster of 24 x 24 cells of 30 m (blocks of 3 cells, 8% of the cells redrawn), polygonized:
    many junctions of three patches on a straight edge of a fourth."""
    from rasterio.transform import from_origin

    rng = np.random.default_rng(5)
    img = np.kron(rng.integers(1, 6, (8, 8)), np.ones((3, 3), dtype=int))
    img = np.where(rng.random(img.shape) < 0.08, rng.integers(1, 6, img.shape), img).astype("int32")
    geoms, cls = [], []
    for g, v in shapes(img, transform=from_origin(500000, 7000000, 30, 30)):
        geoms.append(shape(g))
        cls.append(float(v))
    return gpd.GeoDataFrame({"cls": cls}, geometry=geoms, crs=32723)


def lines() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(9)

    def walk(n, x0, y0):
        return LineString(np.c_[x0 + np.cumsum(rng.normal(8, 6, n)), y0 + np.cumsum(rng.normal(0, 6, n))])

    geoms = [
        walk(30, 0, 0),
        walk(12, 0, 200),
        MultiLineString([walk(8, 0, 400), walk(8, 300, 400)]),
        LineString([(0, 600), (10, 700)]),  # two vertices: unchanged
        LineString([(0, 800), (100, 800), (100, 900)]),  # the hand case, scaled
        LineString([(500, 0), (600, 0), (600, 100), (500, 0)]),  # closed
        Point(5, 5),
    ]
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float)}, geometry=geoms, crs=32633)


def polys() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(23)
    angle = np.sort(rng.uniform(0, 2 * math.pi, 40))
    radius = rng.uniform(20, 60, 40)
    jagged = Polygon(np.c_[300 + radius * np.cos(angle), 300 + radius * np.sin(angle)])

    def box(x0, y0, x1, y1):
        return Polygon([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])

    geoms = [
        Polygon([(0, 0), (100, 0), (100, 100), (0, 100)], [[(20, 20), (20, 40), (40, 40), (40, 20)], [(60, 60), (80, 60), (70, 80)]]),
        MultiPolygon([box(200, 0, 230, 30), box(250, 10, 300, 60), Polygon(box(320, 0, 400, 80).exterior, [box(340, 20, 360, 40).exterior])]),
        jagged,
        Polygon([(500, 0), (560, 0), (530, 52.3)]),  # a triangle: 4 coordinates, cut to a hexagon
        box(0, 400, 4, 404),  # the square of the hand case
    ]
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float)}, geometry=geoms, crs=32633)


def jagged_coverage(n=4, size=100.0, seed=17) -> gpd.GeoDataFrame:
    """An n x n coverage: the corners of the grid shifted, every edge a jagged polyline shared by its two cells."""
    rng = np.random.default_rng(seed)
    corner = np.array([[(i * size, j * size) for j in range(n + 1)] for i in range(n + 1)], dtype=float)
    corner[1:-1, 1:-1] += rng.uniform(-size * 0.15, size * 0.15, (n - 1, n - 1, 2))

    def edge(a, b):
        """The polyline from a to b (both included), the same whichever way it is walked."""
        a, b = tuple(a), tuple(b)
        flip = a > b
        p, q = (np.array(b), np.array(a)) if flip else (np.array(a), np.array(b))
        k = 8
        t = np.linspace(0, 1, k + 2)[1:-1]
        d = q - p
        normal = np.array([-d[1], d[0]]) / np.hypot(*d)
        amp = rng.normal(0, size * 0.03, k)
        mid = p + np.outer(t, d) + np.outer(amp, normal)
        pts = np.vstack([p, mid, q])
        return pts[::-1] if flip else pts

    cache = {}

    def get(a, b):
        key = tuple(sorted([a, b]))
        if key not in cache:
            cache[key] = edge(corner[key[0]], corner[key[1]])
        pts = cache[key]
        return pts if (a, b) == key else pts[::-1]

    geoms, ids = [], []
    for i in range(n):
        for j in range(n):
            ring = [get((i, j), (i + 1, j)), get((i + 1, j), (i + 1, j + 1)), get((i + 1, j + 1), (i, j + 1)), get((i, j + 1), (i, j))]
            geoms.append(Polygon(np.vstack([r[:-1] for r in ring])))
            ids.append(float(len(ids) + 1))
    return gpd.GeoDataFrame({"id": ids, "name": [f"c{int(i)}" for i in ids]}, geometry=geoms, crs=32633)


def bad_coverage() -> gpd.GeoDataFrame:
    """A 3 x 3 grid of squares of 10 with an overlap (cell 5 grows into 4), a narrow gap (cell 8 pulled 0.3 away from 7)
    and a feature with a hole."""
    def box(x0, y0, x1, y1):
        return Polygon([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])

    geoms = []
    for j in range(3):
        for i in range(3):
            x0, y0, x1, y1 = i * 10.0, j * 10.0, i * 10.0 + 10, j * 10.0 + 10
            k = j * 3 + i + 1
            if k == 5:
                x0 -= 2  # overlaps cell 4
            if k == 8:
                x0 += 0.3  # leaves a narrow gap to cell 7
            geoms.append(box(x0, y0, x1, y1))
    return gpd.GeoDataFrame({"id": np.arange(1, 10, dtype=float)}, geometry=geoms, crs=32633)


def gap_coverage() -> gpd.GeoDataFrame:
    """Three squares of 10 in a row with a gap of 0.3 between the second and the third: valid unless gap_width > 0.3."""
    def box(x0, y0, x1, y1):
        return Polygon([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])

    geoms = [box(0, 0, 10, 10), box(10, 0, 20, 10), box(20.3, 0, 30.3, 10)]
    return gpd.GeoDataFrame({"id": [1.0, 2.0, 3.0]}, geometry=geoms, crs=32633)


def hexed(g):
    if g is None:
        return None
    if shapely.is_empty(g):
        return "EMPTY"
    return shapely.to_wkb(g, hex=True, output_dimension=2, byte_order=1)


def columns(gdf) -> list:
    """[name, values] per attribute, in the order of the columns (a bool as 0 or 1, None for a missing value)."""
    out = []
    for name in gdf.columns:
        if name == gdf.geometry.name:
            continue
        v = gdf[name].tolist()
        v = [float(x) if isinstance(x, (bool, np.bool_)) else x for x in v]
        out.append([name, [None if isinstance(x, float) and math.isnan(x) else x for x in v]])
    return out


def main() -> None:
    files = {
        "cover_stairs": stairs(),
        "cover_lines": lines(),
        "cover_polys": polys(),
        "cover_jagged": jagged_coverage(),
        "cover_bad": bad_coverage(),
        "cover_gap": gap_coverage(),
        "cover_noisy": noisy(),
        # the staircase in Web Mercator: coordinates no longer exactly on the neighbours' edges
        "cover_reprojected": stairs().to_crs(3857),
    }
    (REF / "vectors").mkdir(exist_ok=True)
    for name, gdf in files.items():
        gdf.to_file(REF / "vectors" / f"{name}.gpkg", driver="GPKG")
    cases = []

    def case(file, op, **params):
        path = REF / "vectors" / f"{file}.gpkg"
        result = getattr(tf.vector, op)(str(path), **params)
        cases.append(
            {
                "file": file,
                "op": f"vector.{op}",
                "params": params,
                "geometries": [hexed(g) for g in result.geometry.values],
                "columns": columns(result),
            }
        )

    for f in ("cover_stairs", "cover_lines", "cover_polys"):
        for it in (1, 2, 3):
            case(f, "smooth", iterations=it)
            case(f, "smooth", iterations=it, preserve_ends=False)
    case("cover_stairs", "coverage_simplify", tolerance=15.0)
    case("cover_stairs", "coverage_simplify", tolerance=15.0, preserve_boundary=True)
    for tol in (2.0, 6.0):
        case("cover_jagged", "coverage_simplify", tolerance=tol)
        case("cover_jagged", "coverage_simplify", tolerance=tol, preserve_boundary=True)
    case("cover_jagged", "coverage_validate")
    case("cover_stairs", "coverage_validate")
    case("cover_bad", "coverage_validate")
    case("cover_bad", "coverage_validate", gap_width=1.0)
    case("cover_gap", "coverage_validate")
    case("cover_gap", "coverage_validate", gap_width=0.5)
    case("cover_polys", "coverage_validate")  # holes and a multipolygon: its own overlaps, if any
    for f in ("cover_stairs", "cover_noisy", "cover_jagged", "cover_bad", "cover_gap", "cover_polys"):
        case(f, "coverage_node")
    case("cover_reprojected", "coverage_node")
    case("cover_reprojected", "coverage_node", tolerance=1e-6)
    case("cover_reprojected", "coverage_node", tolerance=0.01)
    case("cover_bad", "coverage_node", tolerance=0.5)  # the gap of 0.3: the neighbours' corners taken across it
    out = {"library": tf.__version__, "geos": shapely.geos_version_string, "cases": cases}
    path = REF / "vector_cover.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path, len(cases), "cases")


if __name__ == "__main__":
    sys.exit(main())
