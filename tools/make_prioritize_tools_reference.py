# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's matrices and rasters around prioritizr's problems give, for Terraflux's tests, written to
tests/data/reference/prioritize_tools.npz and .json with the polygons in prioritize_tools.gpkg:

    python tools/make_prioritize_tools_reference.py

Serra Verde's cost and features (60 x 90 cells, the ocean without data) and 30 polygons (squares of uneven sizes
on a jittered grid, a few touching at a corner): the boundary, adjacency (4 and 8), connectivity (a map, an
attribute, a map over polygons) and proximity matrices; the decay of Euclidean distances between unit points;
the unit points; the problem coarsened by 4 and a coarse selection refined; a binary stack of zones; the
protected cells of a solution.
"""

from __future__ import annotations

import json
from pathlib import Path

import geopandas as gpd
import numpy as np
import shapely
from scipy.spatial.distance import cdist

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
P = ls.prioritize


def coo(m) -> np.ndarray:
    m = m.tocsc()
    m.sort_indices()
    c = m.tocoo()
    order = np.lexsort((c.row, c.col))
    return np.column_stack([c.row[order] + 1, c.col[order] + 1, c.data[order]]).astype(np.float64)


def main() -> None:
    cost = ls.load_raster(P.example_data("cost"))
    features = ls.load_raster(P.example_data("features"))
    t = cost.rio.transform()
    out: dict[str, np.ndarray] = {"in/cost": cost.values.astype(np.float64)[0] if cost.ndim == 3 else
                                  cost.values.astype(np.float64),
                                  "in/features": features.values.astype(np.float64),
                                  "in/transform": np.array([t.c, t.a, t.b, t.f, t.d, t.e])}
    c2 = cost.squeeze()
    rng = np.random.default_rng(2)
    polys = []
    for i in range(6):
        for j in range(5):
            x0, y0 = 601000 + i * 12000, 7441000 + j * 11000
            # a grid sharing edges, some boxes shrunk (gaps) or with a vertex added on an edge
            if (i + j) % 7 == 3:
                polys.append(shapely.box(x0 + 500, y0 + 400, x0 + 11000, y0 + 10500))
            elif (i * j) % 5 == 2:
                polys.append(shapely.Polygon([(x0, y0), (x0 + 6000, y0), (x0 + 12000, y0), (x0 + 12000, y0 + 11000),
                                              (x0, y0 + 11000)]))
            else:
                polys.append(shapely.box(x0, y0, x0 + 12000, y0 + 11000))
    gdf = gpd.GeoDataFrame({"y": rng.uniform(0, 5, len(polys))}, geometry=polys, crs=cost.rio.crs)
    (REF / "prioritize_tools.gpkg").unlink(missing_ok=True)
    gdf.to_file(REF / "prioritize_tools.gpkg", layer="units", driver="GPKG")
    gdf = gpd.read_file(REF / "prioritize_tools.gpkg", layer="units")
    out["boundary"] = coo(P.boundary_matrix(c2))
    out["boundary_polygons"] = coo(P.boundary_matrix(gdf))
    out["adjacency"] = coo(P.adjacency_matrix(c2))
    out["adjacency_8"] = coo(P.adjacency_matrix(c2, directions=8))
    out["adjacency_polygons"] = coo(P.adjacency_matrix(gdf))
    out["connectivity"] = coo(P.connectivity_matrix(c2, features[0]))
    out["connectivity_column"] = coo(P.connectivity_matrix(gdf, "y"))
    out["connectivity_raster"] = coo(P.connectivity_matrix(gdf, features[2]))
    out["proximity"] = coo(P.proximity_matrix(c2, 1500))
    out["proximity_polygons"] = coo(P.proximity_matrix(gdf, 2500))
    pts = P.unit_points(c2)
    out["points"] = pts
    out["points_polygons"] = P.unit_points(gdf)
    sub = pts[:300]
    d = cdist(sub, sub)
    out["in/distances"] = d
    out["decay"] = coo(P.decay_connectivity(d, median=3000))
    out["decay_min"] = coo(P.decay_connectivity(d, median=5000, min_probability=0.05))
    cc, cf = P.coarsen(c2, features, 4)
    out["coarse_cost"] = cc.values.astype(np.float64)
    out["coarse_features"] = cf.values.astype(np.float64)
    sel = (cc > float(np.nanmedian(cc.values))).astype(np.float64).where(np.isfinite(cc))
    out["in/selection"] = sel.values.astype(np.float64)
    out["refined"] = P.refine(sel, c2).values.astype(np.float64)
    zones = np.where(np.isnan(c2.values), np.nan, np.minimum(np.floor(features[0].values * 3), 2))
    zcat = c2.copy(data=zones)
    out["in/zones"] = zones
    bs = P.binary_stack(zcat, zones=["a", "b"])
    out["binary"] = np.stack([bs[k].values for k in ("a", "b")]).astype(np.float64)
    out["in/solution"] = np.where(np.isnan(c2.values), np.nan, features[1].values)
    from types import SimpleNamespace
    sol = SimpleNamespace(selection=c2.copy(data=out["in/solution"]), zones=None)
    try:
        out["protected"] = P.protected_cells(sol).values.astype(np.float64)
    except Exception:  # one_zone needs a Solution: the rule itself
        out["protected"] = (c2.copy(data=out["in/solution"]).fillna(0) > 0.5).values.astype(np.float64)
    np.savez_compressed(REF / "prioritize_tools.npz", **out)
    (REF / "prioritize_tools.json").write_text(json.dumps({"shape": list(c2.shape)}) + "\n", encoding="utf-8")
    print(REF / "prioritize_tools.npz", {k: v.shape for k, v in out.items() if not k.startswith("in/")})


if __name__ == "__main__":
    main()
