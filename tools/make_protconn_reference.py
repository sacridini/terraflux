# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's ProtConn gives, for Terraflux's tests, written to tests/data/reference/protconn.gpkg (the
protected areas, a region and two regions), protconn.npz and protconn.json:

    python tools/make_protconn_reference.py

Protected areas that overlap each other, cross the region's border or lie outside it within the transboundary
distance; ProtConn at two distances, with transboundary nodes around the nodes and around the region, the
region's parts as nodes (bound), the probability exp(-1), least-cost distances over a resistance, a single
protected area, the importance of every protected area (delta) and ProtConn per region.
"""

from __future__ import annotations

import json
from pathlib import Path

import geopandas as gpd
import numpy as np
import shapely

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
C = ls.conn


def main() -> None:
    rng = np.random.default_rng(9)
    pas = []
    for _ in range(16):
        x, y = rng.uniform(-200, 1400), rng.uniform(-150, 1100)
        r = rng.uniform(30, 120)
        pas.append(shapely.Point(x, y).buffer(r, quad_segs=6))
    pas.append(shapely.box(380, 380, 520, 470))
    pas.append(shapely.box(450, 430, 600, 560))  # overlaps the one before
    region = shapely.Polygon([(0, 0), (1200, 0), (1250, 500), (1200, 960), (0, 960), (-40, 480)])
    regions = [shapely.box(0, 0, 600, 960), shapely.box(600, 0, 1200, 960)]
    path = REF / "protconn.gpkg"
    path.unlink(missing_ok=True)
    gpd.GeoDataFrame(geometry=pas, crs="EPSG:32722").to_file(path, layer="protected", driver="GPKG")
    gpd.GeoDataFrame(geometry=[region], crs="EPSG:32722").to_file(path, layer="region", driver="GPKG")
    gpd.GeoDataFrame({"rid": [7, 9]}, geometry=regions, crs="EPSG:32722").to_file(path, layer="regions", driver="GPKG")
    pas = gpd.read_file(path, layer="protected")
    region = gpd.read_file(path, layer="region")
    regions = gpd.read_file(path, layer="regions")
    resistance = ls.nlm.gaussian_field(45, 36, autocorr_range=6, resolution=30.0, seed=12) * 9 + 1

    out: dict[str, np.ndarray] = {"in/resistance": resistance.values}
    cases: list[dict] = []

    def case(name: str, op: str, params: dict, frame, **extra) -> None:
        out[name] = frame.drop(columns="geometry", errors="ignore").to_numpy(dtype=np.float64)
        cases.append({"name": name, "op": op, "params": params, "columns": [str(c) for c in frame.columns
                                                                            if c != "geometry"], **extra})

    case("plain", "conn.protconn", {"distance": [500.0, 1000.0]}, C.protconn(pas, region, distance=[500.0, 1000.0]))
    case("trans_nodes", "conn.protconn", {"distance": 800.0, "transboundary": 400.0, "bound": True},
         C.protconn(pas, region, distance=800.0, transboundary=400.0, bound=True))
    case("trans_region", "conn.protconn",
         {"distance": 800.0, "transboundary": 300.0, "transboundary_type": "region", "area_unit": "ha"},
         C.protconn(pas, region, distance=800.0, transboundary=300.0, transboundary_type="region", area_unit="ha"))
    case("prob_none", "conn.protconn", {"distance": 600.0, "probability": None, "pij_min": 0.001},
         C.protconn(pas, region, distance=600.0, probability=None, pij_min=0.001))
    case("least_cost", "conn.protconn", {"distance": 3000.0, "distance_type": "least-cost"},
         C.protconn(pas, region, distance=3000.0, distance_type="least-cost", resistance=resistance),
         resistance=True)
    case("single", "conn.protconn", {"distance": 500.0, "only": [16]},
         C.protconn(pas.iloc[[16]], region, distance=500.0))
    table, delta = C.protconn(pas, region, distance=[600.0], transboundary=300.0, delta=True)
    case("delta_table", "conn.protconn", {"distance": [600.0], "transboundary": 300.0, "delta": True}, table)
    out["delta"] = delta.drop(columns="geometry").to_numpy(dtype=np.float64)
    out["delta_areas"] = np.asarray(shapely.area(delta.geometry.values), dtype=np.float64)
    case("regions", "conn.protconn_regions", {"distance": 700.0, "id": "rid"},
         C.protconn_regions(pas, regions, id="rid", distance=700.0))

    # grids: the cells' coordinates, then the indicators per cell
    grids = []
    for name, kw in {"hex": {"cell_area": 40000.0}, "square_clip": {"cell_area": 50000.0, "hexagonal": False,
                                                                    "clip": True},
                     "hex_inside": {"cell_area": 30000.0, "grid_boundary": True}}.items():
        g = C.make_grid(region, **kw)
        out[f"grid_{name}"] = shapely.get_coordinates(g.geometry.values)
        out[f"grid_{name}_n"] = shapely.get_num_coordinates(g.geometry.values).astype(np.float64)
        grids.append({"name": name, "params": kw})
    vector_cases = []
    for name, kw in {"cgrid_protconn": {"distance": 500.0, "cell_area": 90000.0},
                     "cgrid_pc": {"distance": 500.0, "cell_area": 90000.0, "protconn_index": False,
                                  "area_unit": "ha"}}.items():
        g = C.connectivity_grid(pas, region, **kw)
        cols = [c for c in g.columns if c != "geometry"]
        out[name] = g[cols].to_numpy(dtype=np.float64)
        vector_cases.append({"name": name, "op": "conn.connectivity_grid", "params": kw, "columns": cols})
    t1 = pas
    t2 = gpd.GeoDataFrame(geometry=pas.geometry.values[[k for k in range(len(pas)) if k % 4 != 1]].buffer(-8.0),
                          crs=pas.crs)
    t3 = gpd.GeoDataFrame(geometry=pas.geometry.values[[k for k in range(len(pas)) if k % 3 != 0]], crs=pas.crs)
    times = [t1, t2, t3]
    for k, t in enumerate(times):
        t.to_file(path, layer=f"time{k + 1}", driver="GPKG")
    times = [gpd.read_file(path, layer=f"time{k + 1}") for k in range(3)]
    for name, kw in {"dgrid_iic": {"distance": 300.0, "cell_area": 160000.0},
                     "dgrid_pc": {"distance": 400.0, "cell_area": 160000.0, "metric": "PC", "hexagonal": False,
                                  "names": ["a", "b", "c"]}}.items():
        g = C.eca_change_grid(times, region, **kw)
        num = [c for c in g.columns if c not in ("geometry", "time", "comparison", "change")]
        out[name] = g[num].to_numpy(dtype=np.float64)
        vector_cases.append({"name": name, "op": "conn.eca_change_grid", "params": kw, "columns": num,
                             "texts": {c: g[c].tolist() for c in ("time", "comparison", "change")}})

    np.savez_compressed(REF / "protconn.npz", **out)
    (REF / "protconn.json").write_text(json.dumps({"cases": cases, "grids": grids, "vectors": vector_cases},
                                                  indent=1) + "\n", encoding="utf-8")
    print(REF / "protconn.npz", len(out), list(delta.columns))


if __name__ == "__main__":
    main()
