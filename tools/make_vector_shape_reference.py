# SPDX-License-Identifier: GPL-3.0-or-later
"""What the library's tf.vector gives for split, shape and nearest with k, for Terraflux's tests of
vector.split, vector.shape and vector.nearest: seeded layers (forest-like polygons, some multipolygons, some with holes; roads, parcels as polygon
blades and tracks as lines to cut) written as GeoPackages to tests/data/reference/vectors/ (shape_forest.gpkg,
shape_tracks.gpkg, shape_parcels.gpkg, shape_roads.gpkg, shape_meander.gpkg, shape_straight.gpkg, shape_dangle.gpkg,
shape_touch.gpkg; for shape also shape_figures.gpkg and shape_utm.gpkg; for nearest nearest_records.gpkg,
nearest_twins.gpkg, nearest_lattice.gpkg and nearest_middle.gpkg), and each call's result (the geometries as WKB hex, the attributes) to
tests/data/reference/vector_shape.json, as {"library", "geos", "split": [cases], "shape": [cases], "nearest_k": [cases]}:

    PYTHONPATH=<terraflux-py>/src python tools/make_vector_shape_reference.py

Run with the library installed or on PYTHONPATH (shapely 2.1).
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import shapely
from shapely.geometry import LineString, MultiLineString, MultiPolygon, Polygon

import terraflux as tf

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def box(x0, y0, x1, y1, hole=None):
    ring = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    return Polygon(ring, [hole] if hole else None)


def forest() -> gpd.GeoDataFrame:
    """Forest-like polygons: a jagged one, a multipolygon (one part with a hole), a polygon with a hole, a square the
    straight roads cut in two equal halves, a multipolygon only touched, and one no blade reaches. The attribute
    ``part`` is a name the result's own column must not overwrite."""
    rng = np.random.default_rng(41)
    angle = np.sort(rng.uniform(0, 2 * math.pi, 36))
    radius = rng.uniform(120, 190, 36)
    jagged = Polygon(np.c_[200 + radius * np.cos(angle), 200 + radius * np.sin(angle)])
    geoms = [
        jagged,
        MultiPolygon([box(500, 100, 650, 250), box(700, 120, 820, 260, [(730, 150), (730, 200), (780, 200), (780, 150)]), box(520, 350, 600, 420)]),
        box(100, 500, 400, 800, [(200, 600), (200, 700), (300, 700), (300, 600)]),
        box(600, 500, 800, 700),
        MultiPolygon([box(850, 850, 900, 900), box(920, 850, 970, 900)]),
        box(0, 900, 50, 950),
    ]
    return gpd.GeoDataFrame(
        {"id": np.arange(1, len(geoms) + 1, dtype=float), "name": [f"f{i}" for i in range(1, len(geoms) + 1)], "part": np.arange(10, 10 + len(geoms), dtype=float)},
        geometry=geoms,
        crs=32633,
    )


def meander() -> gpd.GeoDataFrame:
    """A road that meanders across the whole area."""
    rng = np.random.default_rng(7)
    x = np.arange(-50, 1060, 25.0)
    y = 330 + 230 * np.sin(x / 170 + 0.4) + rng.normal(0, 8, len(x))
    return gpd.GeoDataFrame({"road": ["meander"]}, geometry=[LineString(np.c_[x, y])], crs=32633)


def straight() -> gpd.GeoDataFrame:
    """A straight road across the polygon with a hole (through its hole), and a vertical one through the middle of the
    square (two equal halves: the tie of the order)."""
    return gpd.GeoDataFrame({"road": ["east", "north"]}, geometry=[LineString([(-10, 690), (1010, 690)]), LineString([(700, 450), (700, 750)])], crs=32633)


def dangle() -> gpd.GeoDataFrame:
    """Blades that end inside a polygon (one also in a hole): nothing is cut."""
    return gpd.GeoDataFrame({"road": ["in", "hole"]}, geometry=[LineString([(120, 200), (230, 190)]), LineString([(250, 550), (250, 650)])], crs=32633)


def touch() -> gpd.GeoDataFrame:
    """Blades that touch the multipolygon from outside (an end on its edge, a corner) and one between its parts."""
    return gpd.GeoDataFrame(
        {"road": ["end", "corner", "between"]},
        geometry=[LineString([(800, 875), (850, 875)]), LineString([(960, 900), (1000, 940)]), LineString([(910, 800), (910, 950)])],
        crs=32633,
    )


def roads() -> gpd.GeoDataFrame:
    """Every road in one layer (the blades of one feature are the union of those that meet it)."""
    parts = [meander(), straight(), dangle(), touch()]
    geoms = [g for p in parts for g in p.geometry]
    return gpd.GeoDataFrame({"road": [r for p in parts for r in p["road"]]}, geometry=geoms, crs=32633)


def parcels() -> gpd.GeoDataFrame:
    """Polygons as blades: their boundaries cut."""
    geoms = [box(150, 150, 450, 450), box(650, 50, 760, 200), box(100, 480, 260, 640), box(900, 880, 960, 990)]
    return gpd.GeoDataFrame({"parcel": [f"p{i}" for i in range(1, 5)]}, geometry=geoms, crs=32633)


def tracks() -> gpd.GeoDataFrame:
    """Lines to cut: a diagonal, a multiline, one that touches a road only, one lying along a straight road (entirely
    covered by the blade), one on its own."""
    geoms = [
        LineString([(0, 0), (1000, 1000)]),
        MultiLineString([[(0, 100), (300, 120), (600, 80)], [(0, 500), (500, 520), (1000, 600)]]),
        LineString([(0, 400), (40, 380)]),
        LineString([(100, 690), (300, 690)]),
        LineString([(0, 990), (30, 995)]),
    ]
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float), "name": [f"t{i}" for i in range(1, len(geoms) + 1)]}, geometry=geoms, crs=32633)


def figures() -> gpd.GeoDataFrame:
    """Polygons of known shape: a circle-like buffer, a long thin rectangle east-west, the same turned (30 degrees
    and a hundred), a square (the two sides tie), a thin north-south one, a multipolygon with a hole, a triangle and a
    patch with a hole. ``inscribed_radius`` is a name the result's own column must not overwrite."""
    from shapely import affinity

    long_ew = box(0, 0, 400, 20)
    geoms = [
        shapely.Point(100, 100).buffer(60),
        long_ew,
        affinity.rotate(long_ew, 30, origin=(0, 0)),
        affinity.rotate(long_ew, 100, origin=(0, 0)),
        box(0, 0, 50, 50),
        box(0, 0, 8, 120),
        MultiPolygon([box(0, 0, 80, 60, [(20, 20), (20, 40), (60, 40), (60, 20)]), box(100, 10, 140, 90)]),
        Polygon([(0, 0), (90, 10), (30, 70)]),
        box(0, 0, 100, 100, [(30, 30), (30, 70), (70, 70), (70, 30)]),
    ]
    return gpd.GeoDataFrame(
        {"id": np.arange(1, len(geoms) + 1, dtype=float), "inscribed_radius": np.arange(len(geoms), dtype=float)},
        geometry=geoms,
        crs=32633,
    )


def utm() -> gpd.GeoDataFrame:
    """Patches at real UTM coordinates (a layer in a projected CRS far from the origin), one a multipolygon."""
    rng = np.random.default_rng(5)
    geoms = []
    for k in range(5):
        a = np.sort(rng.uniform(0, 2 * math.pi, 24))
        r = rng.uniform(300, 900, 24) * (1 + k)
        geoms.append(Polygon(np.c_[512345.25 + 3000 * k + r * np.cos(a), 7890123.5 + 2000 * k + 0.6 * r * np.sin(a)]))
    geoms.append(MultiPolygon([box(520000, 7890000, 520500, 7890300), box(521000, 7890100, 521400, 7890500)]))
    return gpd.GeoDataFrame({"id": np.arange(1, len(geoms) + 1, dtype=float)}, geometry=geoms, crs=31983)


def records() -> gpd.GeoDataFrame:
    """Records-like seeded points over the forest's area, with a number, a text and a missing text."""
    rng = np.random.default_rng(23)
    n = 40
    xy = np.round(rng.uniform(-50, 1050, (n, 2)), 3)
    kind = [None if i % 9 == 4 else ("a", "b", "c")[i % 3] for i in range(n)]
    return gpd.GeoDataFrame(
        {"id": np.arange(1, n + 1, dtype=float), "kind": kind, "distance": np.arange(n, dtype=float)},
        geometry=[shapely.Point(x, y) for x, y in xy],
        crs=32633,
    )


def twins() -> gpd.GeoDataFrame:
    """Points with duplicates (distance 0 apart, also three at one place) and a point far off."""
    pts = [(0, 0), (0, 0), (10, 0), (10, 0), (10, 0), (25, 5), (-30, 40), (500, 500)]
    return gpd.GeoDataFrame({"id": np.arange(1, len(pts) + 1, dtype=float), "tag": [f"t{i}" for i in range(len(pts))]}, geometry=[shapely.Point(p) for p in pts], crs=32633)


def lattice() -> gpd.GeoDataFrame:
    """Points on a regular grid (many exact ties in distance), in an order that is not the grid's."""
    pts = [(x * 10.0, y * 10.0) for y in range(5) for x in range(5)]
    order = np.random.default_rng(3).permutation(len(pts))
    return gpd.GeoDataFrame({"id": np.arange(1, len(pts) + 1, dtype=float)}, geometry=[shapely.Point(pts[i]) for i in order], crs=32633)


def middle() -> gpd.GeoDataFrame:
    """Points that tie in distance with the lattice's: one in the middle of four, one in the middle of a row."""
    pts = [(15.0, 15.0), (20.0, 20.0), (5.0, 0.0), (45.0, 25.0), (-10.0, 20.0)]
    return gpd.GeoDataFrame({"id": np.arange(1, len(pts) + 1, dtype=float), "tag": [f"m{i}" for i in range(len(pts))]}, geometry=[shapely.Point(p) for p in pts], crs=32633)


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
        out.append([name, [None if (isinstance(x, float) and math.isnan(x)) or x is pd.NA else x for x in v]])
    return out


def main() -> None:
    files = {
        "shape_forest": forest(),
        "shape_tracks": tracks(),
        "shape_parcels": parcels(),
        "shape_roads": roads(),
        "shape_meander": meander(),
        "shape_straight": straight(),
        "shape_dangle": dangle(),
        "shape_touch": touch(),
        "shape_figures": figures(),
        "shape_utm": utm(),
        "nearest_records": records(),
        "nearest_twins": twins(),
        "nearest_lattice": lattice(),
        "nearest_middle": middle(),
    }
    (REF / "vectors").mkdir(exist_ok=True)
    for name, gdf in files.items():
        # a GeoPackage already there is kept (its bytes are not reproducible, its contents are seeded)
        if not (REF / "vectors" / f"{name}.gpkg").exists():
            gdf.to_file(REF / "vectors" / f"{name}.gpkg", driver="GPKG")
    split = []

    def case(file, lines):
        result = tf.vector.split(str(REF / "vectors" / f"{file}.gpkg"), str(REF / "vectors" / f"{lines}.gpkg"))
        split.append(
            {
                "file": file,
                "lines": lines,
                "geometries": [hexed(g) for g in result.geometry.values],
                "columns": columns(result),
            }
        )

    for lines in ("shape_roads", "shape_meander", "shape_straight", "shape_dangle", "shape_touch", "shape_parcels"):
        case("shape_forest", lines)
    for lines in ("shape_roads", "shape_meander", "shape_straight", "shape_parcels"):
        case("shape_tracks", lines)
    case("shape_roads", "shape_parcels")  # lines cut by polygon boundaries
    case("shape_parcels", "shape_roads")  # polygons cut by roads
    shape = []

    def shaped(file, tolerance=None, geometry="feature"):
        kw = {} if tolerance is None else {"tolerance": tolerance}
        result = tf.vector.shape(str(REF / "vectors" / f"{file}.gpkg"), geometry=geometry, **kw)
        shape.append(
            {
                "file": file,
                "tolerance": tolerance,
                "geometry": geometry,
                "geometries": [hexed(g) for g in result.geometry.values],
                "columns": columns(result),
            }
        )

    for file in ("shape_forest", "shape_figures", "shape_utm"):
        shaped(file)
        shaped(file, tolerance=0.5)
        for geometry in ("inscribed", "bounding", "rectangle"):
            shaped(file, geometry=geometry)
    shaped("shape_figures", tolerance=0.01, geometry="inscribed")
    shaped("shape_utm", tolerance=25.0, geometry="inscribed")
    nearest_k = []

    def near(file, other=None, k=1, max_distance=None):
        kw = {"k": k}
        if max_distance is not None:
            kw["max_distance"] = max_distance
        v = lambda f: str(REF / "vectors" / f"{f}.gpkg")
        result = tf.vector.nearest(v(file), None if other is None else v(other), **kw)
        nearest_k.append(
            {
                "file": file,
                "other": other,
                "k": k,
                "max_distance": max_distance,
                "geometries": [hexed(g) for g in result.geometry.values],
                "columns": columns(result),
            }
        )

    near("shape_forest", "shape_parcels", k=3)
    near("shape_forest", "shape_forest", k=3)  # itself included at distance 0
    near("shape_forest", k=3)
    near("shape_forest", k=1)
    near("nearest_records", "shape_forest", k=1)  # as before
    near("nearest_records", "shape_forest", k=1, max_distance=40.0)
    near("nearest_records", "shape_forest", k=4)
    near("nearest_records", "shape_forest", k=4, max_distance=60.0)
    near("nearest_records", "shape_parcels", k=10)  # more than the features
    near("nearest_records", "shape_parcels", k=2, max_distance=0.0)
    near("nearest_records", k=1)
    near("nearest_records", k=2)
    near("nearest_records", k=5, max_distance=80.0)
    near("nearest_twins", k=1)
    near("nearest_twins", k=3)
    near("nearest_twins", k=10)  # more than the other features
    near("nearest_twins", "nearest_twins", k=3)
    near("nearest_lattice", "nearest_middle", k=4)
    near("nearest_middle", "nearest_lattice", k=1)
    near("nearest_middle", "nearest_lattice", k=5)
    near("nearest_middle", "nearest_lattice", k=5, max_distance=12.0)
    near("nearest_lattice", k=1)
    near("nearest_lattice", k=3)
    near("nearest_lattice", k=3, max_distance=10.0)
    near("nearest_lattice", "nearest_twins", k=2)
    out = {"library": tf.__version__, "geos": shapely.geos_version_string, "split": split, "shape": shape, "nearest_k": nearest_k}
    path = REF / "vector_shape.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path, len(split), "split cases,", len(shape), "shape cases,", len(nearest_k), "nearest_k cases")
    for c in split:
        print(c["file"], c["lines"], len(c["geometries"]), "rows")


if __name__ == "__main__":
    sys.exit(main())
