# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's habitat graphs of vector features give, for Terraflux's tests, written to
tests/data/reference/conn_vector.gpkg (the features), conn_vector.npz and conn_vector.json:

    python tools/make_conn_vector_reference.py

Polygons (round ones, C-shaped ones whose centroid falls outside them, multipolygons, one with a hole) on a
grid of 30 m cells: the nodes (their areas, an attribute weighted by the area), the distances between
centroids and edges (GEOS's), least-cost distances over a resistance and a graph of the features with a node
importance. The json holds each case's operator, settings and columns; the npz the results and the resistance.
"""

from __future__ import annotations

import json
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import shapely

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
C = ls.conn


def features() -> gpd.GeoDataFrame:
    rng = np.random.default_rng(5)
    geoms = []
    for k in range(14):
        x, y = rng.uniform(100, 1100), rng.uniform(100, 860)
        r = rng.uniform(20, 70)
        if k % 5 == 1:  # a C: its centroid outside
            outer = shapely.Point(x, y).buffer(r, quad_segs=8)
            g = outer.difference(shapely.box(x - r * 0.4, y - r * 0.4, x + r * 1.1, y + r * 0.4))
        elif k % 5 == 3:  # two parts
            g = shapely.MultiPolygon([shapely.Point(x, y).buffer(r, quad_segs=6),
                                      shapely.Point(x + 3 * r, y + r).buffer(r * 0.5, quad_segs=6)])
        elif k == 4:  # a hole
            g = shapely.Point(x, y).buffer(r, quad_segs=8).difference(shapely.Point(x, y).buffer(r / 2, quad_segs=8))
        else:
            g = shapely.Point(x, y).buffer(r, quad_segs=8)
        geoms.append(g)
    return gpd.GeoDataFrame({"pid": np.arange(101, 115), "quality": np.round(rng.uniform(0.2, 1.0, 14), 3)},
                            geometry=geoms, crs="EPSG:32722")


def main() -> None:
    nodes = features()
    path = REF / "conn_vector.gpkg"
    path.unlink(missing_ok=True)
    nodes.to_file(path, layer="nodes", driver="GPKG")
    nodes = gpd.read_file(path, layer="nodes")
    resistance = ls.nlm.gaussian_field(40, 32, autocorr_range=6, resolution=30.0, seed=12) * 9 + 1

    out: dict[str, np.ndarray] = {"in/resistance": resistance.values}
    cases: list[dict] = []

    def case(name: str, op: str, params: dict, frame: pd.DataFrame) -> None:
        out[name] = frame.to_numpy(dtype=np.float64)
        cases.append({"name": name, "op": op, "params": params, "columns": [str(c) for c in frame.columns]})

    case("nodes_area", "conn.nodes", {"id": "pid", "area_unit": "ha"}, C.node_table(nodes, id="pid", area_unit="ha"))
    case("nodes_quality", "conn.nodes", {"attribute": "quality", "weighted": True},
         C.node_table(nodes, attribute="quality", weighted=True))
    case("dist_centroid", "conn.distances", {"id": "pid"}, C.distance_table(nodes, id="pid"))
    case("dist_edge", "conn.distances", {"distance": "edge", "max_distance": 400.0},
         C.distance_table(nodes, distance="edge", max_distance=400.0))
    case("dist_edge_all", "conn.distances", {"distance": "edge", "distance_unit": "km"},
         C.distance_table(nodes, distance="edge", distance_unit="km"))
    case("dist_lc", "conn.distances", {"distance": "least-cost", "id": "pid"},
         C.distance_table(nodes, distance="least-cost", resistance=resistance, id="pid"))
    g = C.HabitatGraph.from_vector(nodes, id="pid", distance="edge", area_unit="ha")
    imp = g.node_importance(["PC", "IIC"], distance=250.0)
    case("graph_nodes", "conn.node_importance", {"distance": 250.0, "metrics": ["PC", "IIC"]}, imp)
    out["graph_habitat_area"] = np.array([g.habitat_area])

    case("focal_iic", "conn.focal_nodes", {"distance": [150.0, 300.0], "search_buffer": [200.0, 400.0], "id": "pid"},
         C.focal_nodes(nodes, distance=[150.0, 300.0], search_buffer=[200.0, 400.0], id="pid"))
    case("focal_pc", "conn.focal_nodes",
         {"distance": 250.0, "search_buffer": 300.0, "metric": "PC", "attribute": "quality", "threshold": 600.0},
         C.focal_nodes(nodes, distance=250.0, search_buffer=300.0, metric="PC", attribute="quality",
                       threshold=600.0))

    # settlement probabilities (SAMC)
    absorption = resistance.copy(data=np.where(resistance.values > 6, 0.002, 0.0005))
    out["in/absorption"] = absorption.values
    for name, kw in {"samc": {"dispersal_distance": 200.0},
                     "samc_mean": {"dispersal_distance": 150.0, "settle": "mean", "vertex_fraction": 0.5,
                                   "directions": 4, "cost_function": "makurhini"},
                     "samc_absorb": {"absorption": True, "vertices": 6, "id": "pid"}}.items():
        args = dict(kw)
        if args.pop("absorption", False):
            args["absorption"] = absorption
        ids, P = C.settlement_probabilities(nodes, resistance, **args)
        out[name] = np.column_stack([np.asarray(ids, dtype=np.float64), P])
        cases.append({"name": name, "op": "conn.settlement_probabilities", "params": kw,
                      "columns": ["id"] + [str(i) for i in ids]})
    g = C.HabitatGraph.from_samc(nodes, resistance, dispersal_distance=200.0, area_unit="ha")
    case("samc_graph", "conn.overall", {"distance": 0.001, "metrics": ["PC", "F", "AWF"]},
         g.overall(["PC", "F", "AWF"], distance=0.001))

    np.savez_compressed(REF / "conn_vector.npz", **out)
    (REF / "conn_vector.json").write_text(json.dumps(cases, indent=1) + "\n", encoding="utf-8")
    print(REF / "conn_vector.npz", len(out))


if __name__ == "__main__":
    main()
