# SPDX-License-Identifier: GPL-3.0-or-later
"""What the library's tf.vector gives for centroid, hull, simplify, envelope, make_valid, explode and boundary, for
Terraflux's tests of vector.centroid, vector.hull, vector.simplify, vector.envelope, vector.make_valid,
vector.explode and vector.boundary: seeded polygons, points and lines written as GeoPackages to
tests/data/reference/vectors/ (shape_polys.gpkg, shape_valid.gpkg, shape_points.gpkg, shape_lines.gpkg), and each call's result
(the geometries as WKB hex, the attributes) to tests/data/reference/vector_shapes.json:

    PYTHONPATH=<terraflux-py>/src python tools/make_vector_shapes_reference.py

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
from shapely.geometry import LineString, MultiLineString, MultiPolygon, Point, Polygon

import terraflux as tf

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def box(x0, y0, x1, y1):
    return Polygon([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])


def polygons() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(23)
    angle = np.sort(rng.uniform(0, 2 * math.pi, 40))
    radius = rng.uniform(20, 60, 40)
    jagged = Polygon(np.c_[300 + radius * np.cos(angle), 300 + radius * np.sin(angle)])
    geoms = [
        Polygon([(0, 0), (100, 0), (100, 100), (0, 100)], [[(20, 20), (20, 40), (40, 40), (40, 20)]]),
        MultiPolygon([box(200, 0, 230, 30), box(250, 10, 300, 60), Polygon(box(320, 0, 400, 80).exterior, [box(340, 20, 360, 40).exterior])]),
        Polygon([(0, 200), (50, 250), (50, 200), (0, 250)]),  # a bow-tie
        Polygon([(100, 200), (200, 200), (200, 210), (110, 210), (110, 300), (100, 300)]),  # an L
        jagged,
        Polygon(),
        Polygon([(500, 0), (560, 0), (530, 52.3)]),
        MultiPolygon([box(0, 400, 10, 410), box(20, 400, 30, 410)]),
    ]
    return gpd.GeoDataFrame(
        {"id": np.arange(1, len(geoms) + 1, dtype=float), "name": [f"p{i}" for i in range(len(geoms))]},
        geometry=geoms,
        crs=32633,
    )


def points() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(5)
    n = 36
    xy = np.c_[rng.uniform(0, 1000, n), rng.uniform(0, 1000, n)]
    return gpd.GeoDataFrame(
        {
            "id": np.arange(1, n + 1, dtype=float),
            "group": rng.choice(["a", "b", "c"], n),
            "cls": rng.integers(1, 4, n).astype(float),
        },
        geometry=gpd.points_from_xy(xy[:, 0], xy[:, 1]),
        crs=32633,
    )


def lines() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(9)

    def walk(n, x0, y0):
        return LineString(np.c_[x0 + np.cumsum(rng.normal(8, 6, n)), y0 + np.cumsum(rng.normal(0, 6, n))])

    geoms = [
        walk(30, 0, 0),
        walk(12, 0, 200),
        MultiLineString([walk(8, 0, 400), walk(8, 300, 400)]),
        LineString([(0, 600), (10, 700)]),
        LineString([(0, 800), (100, 800)]),
        LineString([(500, 0), (600, 0), (600, 100), (500, 0)]),
        walk(50, 0, 900),
    ]
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float)}, geometry=geoms, crs=32633)


def hexed(g):
    if g is None:
        return None
    if shapely.is_empty(g):
        return "EMPTY"
    return shapely.to_wkb(g, hex=True, output_dimension=2, byte_order=1)


def columns(gdf) -> dict:
    """[name, values] per attribute, in the order of the columns (None for a missing value)."""
    out = []
    for name in gdf.columns:
        if name == gdf.geometry.name:
            continue
        v = gdf[name].tolist()
        out.append([name, [None if isinstance(x, float) and math.isnan(x) else x for x in v]])
    return out


def main() -> None:
    polys = polygons()
    # the same without the bow-tie: a union of an invalid polygon fails in GEOS (in both)
    files = {
        "shape_polys": polys,
        "shape_valid": polys.drop(index=2).reset_index(drop=True),
        "shape_points": points(),
        "shape_lines": lines(),
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

    for f in files:
        case(f, "centroid")
        case(f, "centroid", inside=True)
        case(f, "hull")
        case(f, "hull", kind="concave", ratio=0.3)
        case(f, "hull", kind="concave", ratio=0.7, allow_holes=True)
        if f != "shape_polys":
            case(f, "hull", dissolve=True)
        case(f, "simplify", tolerance=5.0)
        case(f, "simplify", tolerance=5.0, preserve_topology=False)
        case(f, "envelope")
        case(f, "make_valid")
        case(f, "explode")
        case(f, "boundary")
    case("shape_points", "hull", by="group")
    case("shape_points", "hull", by="cls", kind="concave", ratio=0.5)
    case("shape_valid", "hull", dissolve=True, kind="concave", ratio=0.2)
    case("shape_valid", "hull", by="name", kind="concave", ratio=0.2)
    case("shape_lines", "simplify", tolerance=20.0)
    out = {"library": tf.__version__, "geos": shapely.geos_version_string, "cases": cases}
    path = REF / "vector_shapes.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path, len(cases), "cases")


if __name__ == "__main__":
    sys.exit(main())
