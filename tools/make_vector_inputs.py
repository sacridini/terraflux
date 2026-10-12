# SPDX-License-Identifier: GPL-3.0-or-later
"""Vector features for Terraflux's tests of the vector operators and of planning units of polygons: the polygons of
landschaft's optimize cases (tests/optimize_cases.py: the planning units prioritizr's references were made with, the
shapes of exactextractr's) as GeoPackages in tests/data/reference/vectors, and a layer of polygons, lines and points;
landschaft's rasterize and zonal_stats of them on the cases' grid into vectors_reference.npz and .json:

    python tools/make_vector_inputs.py
"""

from __future__ import annotations

import json
import sys
import warnings
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import geopandas as gpd  # noqa: E402
import landschaft as ls  # noqa: E402
import optimize_cases as oc  # noqa: E402
from shapely.geometry import LineString, MultiPoint, Point, box  # noqa: E402

OUT = ROOT / "tests" / "data" / "reference" / "vectors"


def mixed():
    """Polygons (overlapping), lines and points with an attribute, on the cases' grid (100 m cells)."""
    r = oc.RES
    geoms = [box(1.2 * r, 1.4 * r, 5.6 * r, 4.5 * r), box(4.0 * r, 3.0 * r, 9.3 * r, 7.7 * r),
             LineString([(0.5 * r, 10.5 * r), (13.7 * r, 6.2 * r), (14.5 * r, 0.3 * r)]),
             LineString([(2.0 * r, 0.5 * r), (2.0 * r, 11.5 * r)]),
             Point(10.5 * r, 10.5 * r), MultiPoint([(12.2 * r, 2.7 * r), (3.3 * r, 8.8 * r), (3.4 * r, 8.9 * r)])]
    return gpd.GeoDataFrame({"v": [3.0, 5.0, 7.0, 11.0, 13.0, 17.0], "name": list("abcdef")}, geometry=geoms,
                            crs="EPSG:32723")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    layers = {"pu_polygons": oc.pu_polygons(), "pu_polygons_partial": oc.pu_polygons_partial(),
              "shapes": oc.shapes(), "zone_polygons": oc.zone_polygons(), "mixed": mixed()}
    for name, g in layers.items():
        path = OUT / f"{name}.gpkg"
        if path.exists():
            path.unlink()
        g.to_file(path, layer=name, driver="GPKG")
    like = oc.features().isel(band=0)
    arrays, meta = {}, {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for key, kw in {"centre": {}, "touched": {"all_touched": True}, "field": {"field": "v"},
                        "add": {"field": "v", "merge": "add", "all_touched": True},
                        "fill": {"field": "v", "fill": np.nan}}.items():
            arrays[f"rasterize/{key}"] = np.asarray(ls.rasterize(layers["mixed"], like, dtype="float64", **kw).values,
                                                    dtype=np.float64)
        stats = ["count", "sum", "mean", "min", "max", "std", "median", "majority", "unique", "percentile_90"]
        for zones in ("shapes", "pu_polygons_partial"):
            for touched in (False, True):
                t = ls.zonal_stats(oc.features(), layers[zones], stats=stats, all_touched=touched)
                t = t[[c for c in t.columns if c != "geometry" and np.issubdtype(t[c].dtype, np.number)]]
                key = f"zonal/{zones}/{'touched' if touched else 'centre'}"
                arrays[key] = t.to_numpy(np.float64)
                meta[key] = list(map(str, t.columns))
    np.savez_compressed(OUT.parent / "vectors_reference.npz", **arrays)
    (OUT.parent / "vectors_reference.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(sorted(p.name for p in OUT.iterdir()), sorted(arrays))


if __name__ == "__main__":
    main()
