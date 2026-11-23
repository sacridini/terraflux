# SPDX-License-Identifier: GPL-3.0-or-later
"""What the library's tf.vector.relate, tf.vector.count and tf.vector.distance give, for Terraflux's tests of the
spatial functions of vector.calc and vector.filter (within(x), count(x, "contains"), distance(x)): a seeded case of
polygons, points and lines on an integer grid (so that points lie on edges, polygons share sides and distances are
exact), written to tests/data/reference/vectors/calc_*.gpkg with the answers in tests/data/reference/vector_calc.json:

    python tools/make_vector_calc_reference.py

Run with the library installed (terraflux-py with tf.vector).
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import geopandas as gpd
import numpy as np
import shapely
import terraflux as tf
from shapely.geometry import LineString, Point, box

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
CRS = "EPSG:32723"


def num(v):
    v = float(v)
    return None if math.isnan(v) else v


def main() -> None:
    rng = np.random.default_rng(23)
    # 30 rectangles on a 40 x 40 grid (some touch, some overlap), with a population and a use
    zones = []
    for _ in range(30):
        x0, y0 = rng.integers(0, 32, 2)
        w, h = rng.integers(2, 9, 2)
        zones.append(box(x0, y0, x0 + w, y0 + h))
    zones = gpd.GeoDataFrame(
        {"pop": rng.integers(1, 100, len(zones)).astype(float),
         "uso": rng.choice(["forest", "pasto", "water"], len(zones))},
        geometry=zones, crs=CRS)
    # 60 points on the grid (many on the edges of the rectangles), some far away
    sites = [Point(int(x), int(y)) for x, y in rng.integers(0, 44, (60, 2))]
    sites = gpd.GeoDataFrame({"site": np.arange(1, 61, dtype=float)}, geometry=sites, crs=CRS)
    # 8 lines with integer vertices
    lines = [LineString([(int(a), int(b)) for a, b in rng.integers(0, 44, (int(n), 2))])
             for n in rng.integers(2, 5, 8)]
    lines = gpd.GeoDataFrame({"road": np.arange(1, 9, dtype=float)}, geometry=lines, crs=CRS)
    for name, g in (("zones", zones), ("sites", sites), ("lines", lines)):
        tf.write_vector(g, REF / "vectors" / f"calc_{name}.gpkg")
    layers = {"zones": zones, "sites": sites, "lines": lines}

    cases = []

    def add(a, b, fn, predicate=None, distance=None):
        if fn == "relate":
            got = tf.vector.relate(layers[a], layers[b], predicate, distance=distance).astype(float)
        elif fn == "count":
            got = tf.vector.count(layers[a], layers[b], predicate, distance=distance).astype(float)
        else:
            got = tf.vector.distance(layers[a], layers[b])
        cases.append({"a": a, "b": b, "fn": fn, "predicate": predicate, "distance": distance,
                      "expected": [num(v) for v in got]})

    predicates = ("intersects", "within", "contains", "touches", "crosses", "overlaps", "covers", "covered_by",
                  "contains_properly", "equals", "disjoint")
    for a, b in (("zones", "sites"), ("sites", "zones"), ("zones", "zones"), ("zones", "lines"), ("lines", "zones"),
                 ("sites", "lines")):
        for p in predicates:
            add(a, b, "relate", p)
            add(a, b, "count", p)
        add(a, b, "relate", "dwithin", 2.0)
        add(a, b, "count", "dwithin", 3.0)
        add(a, b, "distance")
    out = {"terraflux": tf.__version__, "geos": ".".join(map(str, shapely.geos_version)),
           "pop": [float(v) for v in zones["pop"]], "cases": cases}
    (REF / "vector_calc.json").write_text(json.dumps(out))
    print(f"{len(cases)} cases")


if __name__ == "__main__":
    main()
