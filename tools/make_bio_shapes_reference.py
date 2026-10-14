# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's spatial conditions on shapefiles give, for Terraflux's tests, written to
tests/data/reference/bio_shapes.json with the polygons in bio_shapes.gpkg and bio_shapes.shp:

    python tools/make_bio_shapes_reference.py

A grid of groups and polygons over it (overlapping ones, one with a hole, a copy of another, some groups on their
edges): sp_point_in_poly_shape (every feature, by an attribute's text or number, by position, the coordinates of
the processing group, other axes) as definition queries and neighbourhoods, and sp_points_in_same_poly_shape,
alone and with a circle. Per case the definition query's mask or the neighbour sets.
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


def main() -> None:
    rows = []
    rng = np.random.default_rng(4)
    for i in range(12):
        for j in range(10):
            for sp in rng.choice(["a", "b", "c", "d"], int(rng.integers(1, 4)), replace=False):
                rows.append({"x": i + 0.5, "y": j + 0.5, "z": float((i * j) % 5), "sp": sp})
    df = pd.DataFrame(rows)
    df.to_csv(REF / "bio_shapes.csv", index=False)
    polys = [shapely.Polygon([(1, 1), (6, 1.5), (5.5, 6.5), (1.5, 5)], [[(3, 3), (4.5, 3), (4, 4.5)]]),
             shapely.box(4, 4, 9.5, 8.5),  # overlaps the first; edges through cell centres
             shapely.box(0.5, 7.5, 3.5, 9.5),
             shapely.box(4, 4, 9.5, 8.5),  # a copy of the second
             shapely.Polygon([(8, 0), (12, 0.5), (11, 3.5), (8.5, 2.5)])]
    gdf = gpd.GeoDataFrame({"name": ["one", "two", "three", "four", "five"], "kind": [1, 2, 1, 3, 2]},
                           geometry=polys)
    (REF / "bio_shapes.gpkg").unlink(missing_ok=True)
    gdf.to_file(REF / "bio_shapes.gpkg", layer="polys", driver="GPKG")
    for ext in ("shp", "shx", "dbf", "cpg", "prj"):
        (REF / f"bio_shapes.{ext}").unlink(missing_ok=True)
    gdf.to_file(REF / "bio_shapes.shp")
    bd = ls.bio.BaseData.from_dataframe(df, ["x", "y"], "sp", cell_sizes=[1, 1])
    bd3 = ls.bio.BaseData.from_dataframe(df, ["z", "x", "y"], "sp", cell_sizes=[1, 1, 1])
    gp = (REF / "bio_shapes.gpkg").as_posix() + ":polys"
    shp = (REF / "bio_shapes").as_posix()
    queries = {
        "all": f"sp_point_in_poly_shape(file => '{gp}')",
        "shp": f"sp_point_in_poly_shape(file => '{shp}')",
        "text": f"sp_point_in_poly_shape(file => '{gp}', field_name => 'name', field_val => 'two')",
        "number": f"sp_point_in_poly_shape(file => '{gp}', field_name => 'kind', field_val => 1)",
        "position": f"sp_point_in_poly_shape(file => '{gp}', field_val => 5)",
        "negated": f"!sp_point_in_poly_shape(file => '{gp}', field_val => 1)",
    }
    out = {"queries": {}, "neighbours": {}, "axes": {}}
    for name, q in queries.items():
        out["queries"][name] = {"condition": q, "mask": ls.bio.definition_query(bd, q).astype(int).tolist()}
    conds = {
        "in_poly": f"sp_point_in_poly_shape(file => '{gp}')",
        "in_poly_coord": f"sp_point_in_poly_shape(file => '{gp}', point => \\@coord) && sp_circle(radius => 2)",
        "same": f"sp_points_in_same_poly_shape(file => '{gp}')",
        "same_circle": f"sp_points_in_same_poly_shape(file => '{gp}') && sp_circle(radius => 3)",
        "same_shp": f"sp_points_in_same_poly_shape(file => '{shp}')",
    }
    for name, c in conds.items():
        nb = ls.bio.neighbours(bd, [c])
        out["neighbours"][name] = {"condition": c, "groups": np.asarray(nb.groups).tolist(),
                                   "indptr": np.asarray(nb.sets[0][0]).tolist(),
                                   "indices": np.asarray(nb.sets[0][1]).tolist()}
    for name, c in {"in_poly_axes": f"sp_point_in_poly_shape(file => '{gp}', axes => [1, 2])",
                    "same_axes": f"sp_points_in_same_poly_shape(file => '{gp}', axes => [1, 2])"}.items():
        nb = ls.bio.neighbours(bd3, [c])
        out["axes"][name] = {"condition": c, "groups": np.asarray(nb.groups).tolist(),
                             "indptr": np.asarray(nb.sets[0][0]).tolist(), "indices": np.asarray(nb.sets[0][1]).tolist()}
    # the paths relative to the reference folder
    text = json.dumps(out).replace(REF.as_posix() + "/", "$REF/")
    (REF / "bio_shapes.json").write_text(text + "\n", encoding="utf-8")
    print({k: len(v) for k, v in out.items()}, [sum(v["mask"]) for v in out["queries"].values()])


if __name__ == "__main__":
    main()
