# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's habitat graphs of raster patches give, for Terraflux's tests, written to
tests/data/reference/conn_raster.npz and conn_raster.json:

    python tools/make_conn_raster_reference.py

A map of classes (NLMR's random cluster, cells of 30 m) and a resistance with a barrier of missing cells: the
nodes (the patches of a habitat class, or a raster of patch ids with weighted attributes), the distances
between centroids and edges, least-cost (Makurhini's and the mean conductance, 8 and 4 directions) and commute
times with the straight distance where the barrier cuts, a graph of the patches and a node importance mapped
back onto them. The json holds each case's operator and settings and the tables' columns; the npz the inputs
and the results.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pandas as pd

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
C = ls.conn


def main() -> None:
    classes = ls.nlm.random_cluster(40, 32, p=0.42, ai=[0.35, 0.65], resolution=30.0, seed=11, rescale=False)
    classes = classes.astype(np.float64)
    resistance = ls.nlm.gaussian_field(40, 32, autocorr_range=6, resolution=30.0, seed=12) * 9 + 1
    r = resistance.values.copy()
    r[:, 19:21] = np.nan
    r[30:, 19:21] = 2.0
    r[10:13, 19:21] = 3.0
    resistance = resistance.copy(data=r)
    ids = np.asarray(ls.Landscape(np.ma.masked_array((classes.values == 2).astype(np.int32),
                                                     mask=classes.values != 2), res=(30.0, 30.0)).labels(),
                     dtype=np.float64)
    ids[ids == 0] = np.nan
    patch_ids = classes.copy(data=ids)

    out: dict[str, np.ndarray] = {"in/classes": classes.values, "in/resistance": r, "in/patch_ids": ids}
    cases: list[dict] = []

    def case(name: str, op: str, inputs: dict, params: dict, frame: pd.DataFrame) -> None:
        out[name] = frame.to_numpy(dtype=np.float64)
        cases.append({"name": name, "op": op, "inputs": inputs, "params": params,
                      "columns": [str(c) for c in frame.columns]})

    hab = {"habitat": 1}
    case("nodes_habitat", "conn.nodes", {"map": "classes"}, {**hab, "area_unit": "ha"},
         C.node_table(classes, habitat=1, area_unit="ha"))
    case("nodes_habitat4", "conn.nodes", {"map": "classes"}, {**hab, "directions": 4},
         C.node_table(classes, habitat=1, directions=4))
    n = int(np.nanmax(ids))
    attr = np.round(np.linspace(0.5, 2.0, n), 3)
    case("nodes_weighted", "conn.nodes", {"map": "patch_ids"}, {"attribute": attr.tolist(), "weighted": True},
         C.node_table(patch_ids, attribute=attr, weighted=True))
    case("dist_centroid", "conn.distances", {"map": "classes"}, {**hab, "max_distance": 0.6, "distance_unit": "km"},
         C.distance_table(classes, habitat=1, max_distance=0.6, distance_unit="km"))
    case("dist_edge", "conn.distances", {"map": "classes"}, {**hab, "distance": "edge"},
         C.distance_table(classes, habitat=1, distance="edge"))
    case("dist_edge_ids", "conn.distances", {"map": "patch_ids"}, {"distance": "edge", "max_distance": 200.0},
         C.distance_table(patch_ids, distance="edge", max_distance=200.0))
    for fn in ("makurhini", "mean"):
        for dirs in (8, 4):
            p = {**hab, "distance": "least-cost", "cost_function": fn, "cost_directions": dirs}
            case(f"dist_lc_{fn}_{dirs}", "conn.distances", {"map": "classes", "resistance": "resistance"}, p,
                 C.distance_table(classes, habitat=1, distance="least-cost", resistance=resistance,
                                  cost_function=fn, cost_directions=dirs))
    case("dist_lc_drop", "conn.distances", {"map": "classes", "resistance": "resistance"},
         {**hab, "distance": "least-cost", "unreachable": "drop", "max_distance": 5000.0},
         C.distance_table(classes, habitat=1, distance="least-cost", resistance=resistance, unreachable="drop",
                          max_distance=5000.0))
    case("dist_commute", "conn.distances", {"map": "classes", "resistance": "resistance"},
         {**hab, "distance": "commute-time"},
         C.distance_table(classes, habitat=1, distance="commute-time", resistance=resistance))
    g = C.HabitatGraph.from_raster(classes, habitat=1, area_unit="ha")
    imp = g.node_importance(["PC", "IIC"], distance=300.0)
    case("graph_nodes", "conn.node_importance", {"map": "classes"}, {"distance": 300.0, "metrics": ["PC", "IIC"]},
         imp)
    out["graph_habitat_area"] = np.array([g.habitat_area])
    out["graph_map"] = np.asarray(g.to_raster(imp, "dPC"), dtype=np.float64)

    np.savez_compressed(REF / "conn_raster.npz", **out)
    (REF / "conn_raster.json").write_text(json.dumps(cases, indent=1) + "\n", encoding="utf-8")
    print(REF / "conn_raster.npz", len(out))


if __name__ == "__main__":
    main()
