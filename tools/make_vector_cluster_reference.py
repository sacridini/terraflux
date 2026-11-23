# SPDX-License-Identifier: GPL-3.0-or-later
"""What the library's tf.vector gives for voronoi, delaunay and cluster, for Terraflux's tests of vector.voronoi,
vector.delaunay and vector.cluster: seeded points (with duplicates), polygons and a clip area written as GeoPackages to
tests/data/reference/vectors/ (cluster_sites.gpkg, cluster_trees.gpkg, cluster_polys.gpkg, cluster_shapes.gpkg,
cluster_area.gpkg), and each call's result (the geometries as WKB hex, the attributes) to
tests/data/reference/vector_cluster.json. The Delaunay cases use irregular shapes (cluster_shapes): the corners of
rectangles are exactly cocircular, and GEOS 3.13 and 3.14 break that tie differently.

    PYTHONPATH=<terraflux-py>/src python tools/make_vector_cluster_reference.py

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
from shapely.geometry import MultiPolygon, Polygon

import terraflux as tf

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def box(x0, y0, x1, y1):
    return Polygon([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])


def sites() -> gpd.GeoDataFrame:
    """60 seeded points with 6 places repeated."""
    rng = np.random.default_rng(31)
    xy = np.c_[rng.uniform(0, 1000, 54), rng.uniform(0, 1000, 54)]
    xy = np.r_[xy, xy[[3, 3, 10, 20, 30, 41]]]
    order = rng.permutation(len(xy))
    xy = xy[order]
    return gpd.GeoDataFrame(
        {"id": np.arange(1, len(xy) + 1, dtype=float), "name": [f"s{i}" for i in range(len(xy))]},
        geometry=gpd.points_from_xy(xy[:, 0], xy[:, 1]),
        crs=32633,
    )


def trees() -> gpd.GeoDataFrame:
    """300 points: five Gaussian groups of different spread and a scatter, with a column named 'cluster'."""
    rng = np.random.default_rng(77)
    centres = [(200, 200, 25), (700, 250, 40), (450, 600, 30), (150, 800, 20), (850, 850, 50)]
    parts = [np.c_[rng.normal(cx, s, 50), rng.normal(cy, s, 50)] for cx, cy, s in centres]
    parts.append(np.c_[rng.uniform(0, 1000, 50), rng.uniform(0, 1000, 50)])
    xy = np.vstack(parts)
    xy = np.round(xy, 1)
    xy = xy[rng.permutation(len(xy))]
    return gpd.GeoDataFrame(
        {"id": np.arange(1, len(xy) + 1, dtype=float), "cluster": np.full(len(xy), 99.0)},
        geometry=gpd.points_from_xy(xy[:, 0], xy[:, 1]),
        crs=32633,
    )


def polys() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(12)
    geoms = []
    for i in range(30):
        x, y = rng.uniform(0, 800, 2)
        w, h = rng.uniform(10, 40, 2)
        geoms.append(box(x, y, x + w, y + h))
    geoms[5] = MultiPolygon([box(100, 100, 130, 130), box(160, 100, 190, 130)])
    geoms[9] = Polygon(box(300, 300, 380, 380).exterior, [box(320, 320, 340, 340).exterior])
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float)}, geometry=geoms, crs=32633)


def shapes() -> gpd.GeoDataFrame:
    """Irregular polygons (no four vertices on a circle): triangles, a holed polygon and a multipolygon."""
    rng = np.random.default_rng(404)
    geoms = []
    for _ in range(12):
        c = rng.uniform(0, 800, 2)
        geoms.append(Polygon(c + rng.uniform(-30, 30, (3, 2))))
    ring = np.sort(rng.uniform(0, 2 * math.pi, 9))
    rad = rng.uniform(40, 70, 9)
    geoms.append(Polygon(np.c_[400 + rad * np.cos(ring), 400 + rad * np.sin(ring)], [np.c_[400 + 10 * np.cos(ring[:4]), 400 + 8 * np.sin(ring[:4])]]))
    geoms.append(MultiPolygon([Polygon(rng.uniform(900, 1000, (3, 2))), Polygon(rng.uniform(0, 100, (3, 2)))]))
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float)}, geometry=geoms, crs=32633)


def area() -> gpd.GeoDataFrame:
    """Two overlapping polygons and a separate one: a union with a hole-free shape that cuts most cells."""
    geoms = [box(150, 150, 650, 600), Polygon([(500, 400), (900, 450), (850, 850), (450, 800)]), box(-50, 700, 120, 1050)]
    return gpd.GeoDataFrame({"id": [1.0, 2.0, 3.0]}, geometry=geoms, crs=32633)


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
        v = gdf[name].tolist()
        out.append([name, [None if isinstance(x, float) and math.isnan(x) else x for x in v]])
    return out


def main() -> None:
    files = {"cluster_sites": sites(), "cluster_trees": trees(), "cluster_polys": polys(), "cluster_shapes": shapes(), "cluster_area": area()}
    (REF / "vectors").mkdir(exist_ok=True)
    for name, gdf in files.items():
        gdf.to_file(REF / "vectors" / f"{name}.gpkg", driver="GPKG")
    cases = []

    def path(name):
        return str(REF / "vectors" / f"{name}.gpkg")

    def case(file, op, clip=None, **params):
        kw = dict(params)
        if clip is not None:
            kw["clip"] = path(clip)
        result = getattr(tf.vector, op)(path(file), **kw)
        cases.append(
            {
                "file": file,
                "op": f"vector.{op}",
                "clip": clip,
                "params": params,
                "geometries": [hexed(g) for g in result.geometry.values],
                "columns": columns(result),
            }
        )

    case("cluster_sites", "voronoi")
    case("cluster_sites", "voronoi", clip="cluster_area")
    case("cluster_sites", "voronoi", tolerance=2.0)
    case("cluster_sites", "voronoi", clip="cluster_area", tolerance=2.0)
    case("cluster_trees", "voronoi", clip="cluster_area")
    for f in ("cluster_sites", "cluster_trees", "cluster_shapes"):
        case(f, "delaunay")
        case(f, "delaunay", edges=True)
    case("cluster_sites", "delaunay", tolerance=5.0)
    for mp in (1, 3, 5):
        for d in (15.0, 30.0):
            case("cluster_trees", "cluster", distance=d, min_points=mp)
    case("cluster_trees", "cluster", distance=30.0, min_points=3, column="grp")
    case("cluster_sites", "cluster", distance=150.0, min_points=2)
    for mp in (1, 2):
        case("cluster_polys", "cluster", distance=30.0, min_points=mp)
        case("cluster_polys", "cluster", distance=0.0, min_points=mp)
    out = {"library": tf.__version__, "geos": shapely.geos_version_string, "shapely": shapely.__version__, "cases": cases}
    target = REF / "vector_cluster.json"
    target.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(target, len(cases), "cases")


if __name__ == "__main__":
    sys.exit(main())
