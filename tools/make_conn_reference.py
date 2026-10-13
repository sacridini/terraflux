# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's habitat graphs give, for Terraflux's tests, written to tests/data/reference/conn.npz and
conn.json:

    python tools/make_conn_reference.py

Graphs of distances (a full matrix and a table of the pairs closer than a distance), of probabilities and of
links, with Makurhini's conventions and Conefor's: the indices overall, the nodes' importances (with the
betweenness), the importance of links and of changes to them, of nodes to restore, the components and the
centralities. The json holds each graph's nodes and pairs and each case's operator and settings; the npz the
tables (their columns in the json).
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pandas as pd

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
HG = ls.conn.HabitatGraph


def main() -> None:
    rng = np.random.default_rng(7)
    n = 14
    xy = rng.uniform(0, 1000, (n, 2))
    area = np.round(rng.uniform(1, 50, n), 3)
    ids = np.arange(10, 10 * (n + 1), 10)
    d = np.sqrt(((xy[:, None, :] - xy[None, :, :]) ** 2).sum(-1))
    np.fill_diagonal(d, 0.0)
    near = [(int(ids[i]), int(ids[j]), float(d[i, j])) for i in range(n) for j in range(i + 1, n) if d[i, j] < 600]
    prob = np.exp(-d / 300.0)
    np.fill_diagonal(prob, 0.0)
    link = (d < 350).astype(float)
    np.fill_diagonal(link, 0.0)
    restoration = (rng.uniform(size=n) < 0.7).astype(int)
    restoration[0] = 1

    graphs = {
        "matrix": {"nodes": {"id": ids.tolist(), "attribute": area.tolist()}, "matrix": d.tolist()},
        "near": {"nodes": {"id": ids.tolist(), "attribute": area.tolist()}, "pairs": near},
        "prob": {"nodes": {"id": ids.tolist(), "attribute": area.tolist()}, "matrix": prob.tolist()},
        "link": {"nodes": {"id": ids.tolist(), "attribute": area.tolist()}, "matrix": link.tolist()},
        "restore": {"nodes": {"id": ids.tolist(), "attribute": area.tolist(), "restoration": restoration.tolist()},
                    "pairs": near},
    }
    frames = {"near": pd.DataFrame(near, columns=["from", "to", "value"])}

    def make(name: str, **kw) -> ls.conn.HabitatGraph:
        g = graphs[name]
        conn = np.asarray(g["matrix"]) if "matrix" in g else frames["near"]
        extra = {"restoration": np.asarray(g["nodes"]["restoration"])} if "restoration" in g["nodes"] else {}
        return HG(np.asarray(g["nodes"]["attribute"]), conn, ids=np.asarray(g["nodes"]["id"]), **extra, **kw)

    out: dict[str, np.ndarray] = {}
    cases: list[dict] = []

    def case(name: str, graph: str, settings: dict, op: str, params: dict, frame: pd.DataFrame, **extra) -> None:
        out[name] = frame.to_numpy(dtype=np.float64)
        cases.append({"name": name, "graph": graph, "settings": settings, "op": op, "params": params,
                      "columns": [str(c) for c in frame.columns], **extra})

    all_metrics = list(ls.conn.METRICS)
    for conv, area_ in (("makurhini", None), ("conefor", 1e6)):
        st = {"convention": conv, **({"landscape_area": area_} if area_ else {})}
        g = make("matrix", convention=conv, landscape_area=area_)
        dists = [None, 300.0] if conv == "makurhini" else [300.0, 450.0]
        case(f"matrix_{conv}_overall", "matrix", st, "conn.overall", {"distance": dists}, g.overall(distance=dists))
        case(f"matrix_{conv}_nodes", "matrix", st, "conn.node_importance", {"distance": dists, "metrics": all_metrics},
             g.node_importance(all_metrics, distance=dists))
        for metric in ("IIC", "PC"):
            case(f"matrix_{conv}_links_{metric}", "matrix", st, "conn.link_importance",
                 {"distance": 300.0, "metric": metric}, g.link_importance(metric, distance=300.0))
        g = make("near", convention=conv, landscape_area=area_)
        case(f"near_{conv}_nodes", "near", st, "conn.node_importance", {"distance": 400.0, "probability": 0.3},
             g.node_importance(distance=400.0, probability=0.3))
        changes = [(int(ids[0]), int(ids[1]), 50.0), (int(ids[2]), int(ids[5]), 900.0),
                   (int(ids[3]), int(ids[4]), float(d[3, 4]) * 0.5), (int(ids[6]), int(ids[13]), 120.0)]
        for metric in ("IIC", "PC"):
            ch = pd.DataFrame(changes, columns=["from", "to", "value"])
            case(f"near_{conv}_change_{metric}", "near", st, "conn.link_change",
                 {"distance": 400.0, "metric": metric}, g.link_change(ch, metric, distance=400.0), changes=changes)
        g = make("restore", convention=conv, landscape_area=area_)
        case(f"restore_{conv}_nodes", "restore", st, "conn.node_importance", {"distance": 400.0},
             g.node_importance(distance=400.0))
        case(f"restore_{conv}_overall", "restore", st, "conn.overall", {"distance": 400.0},
             g.overall(distance=400.0))
    g = make("prob", kind="probability")
    case("prob_overall", "prob", {"kind": "probability"}, "conn.overall", {"distance": 0.3}, g.overall(distance=0.3))
    case("prob_nodes", "prob", {"kind": "probability"}, "conn.node_importance",
         {"distance": 0.3, "metrics": all_metrics}, g.node_importance(all_metrics, distance=0.3))
    g = make("link", kind="link")
    case("link_overall", "link", {"kind": "link"}, "conn.overall", {}, g.overall())
    case("link_nodes", "link", {"kind": "link"}, "conn.node_importance", {}, g.node_importance())
    g = make("matrix")
    comp = g.components(distance=200.0)
    case("components", "matrix", {}, "conn.components", {"distance": 200.0},
         pd.DataFrame({"id": comp.index.to_numpy(), "component": comp.to_numpy()}))
    case("centrality", "matrix", {}, "conn.centrality", {"distance": [450.0, 600.0]},
         g.centrality(distance=[450.0, 600.0]))
    case("centrality_prob", "matrix", {}, "conn.centrality", {"distance": 300.0, "probability": 0.5},
         g.centrality(distance=300.0, probability=0.5))
    case("centrality_strength", "matrix", {}, "conn.centrality", {"distance": 300.0, "binary": False},
         g.centrality(distance=300.0, binary=False))

    # Conefor's files: landschaft's, read back
    g = make("near", convention="conefor", landscape_area=1e6)
    g.to_conefor(REF / "conn_nodes.txt", REF / "conn_connections.txt")

    np.savez_compressed(REF / "conn.npz", **out)
    (REF / "conn.json").write_text(json.dumps({"graphs": graphs, "cases": cases}, indent=1) + "\n", encoding="utf-8")
    print(REF / "conn.npz", len(out))


if __name__ == "__main__":
    main()
