# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's accessible area gives, for Terraflux's tests, written to tests/data/reference/sdm_area.gpkg (a
layer per case with landschaft's polygon, and the inputs) and sdm_area.json:

    python tools/make_sdm_area_reference.py

Bradypus' records: buffers of two distances and resolutions, the convex hull (and widened), the countries that
hold the records (and widened), a buffer cut to the land; records on both sides of the antimeridian. GEOS through
GDAL is not shapely's GEOS, so the tests compare the polygons (their areas and what one has that the other has
not), not their vertices.
"""

from __future__ import annotations

import json
from pathlib import Path

import geopandas as gpd
import pandas as pd
from pyproj import Geod

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def main() -> None:
    brad = pd.read_csv(ROOT / "extern" / "landschaft" / "src" / "landschaft" / "sdm" / "data" / "bradypus.csv")
    fiji = pd.DataFrame({"lon": [179.2, 179.8, -179.6, -178.9, 178.4], "lat": [-16.5, -17.9, -16.2, -18.5, -17.1]})
    brad.to_csv(REF / "sdm_area_bradypus.csv", index=False)
    fiji.to_csv(REF / "sdm_area_fiji.csv", index=False)
    brad = pd.read_csv(REF / "sdm_area_bradypus.csv", float_precision="round_trip")
    fiji = pd.read_csv(REF / "sdm_area_fiji.csv", float_precision="round_trip")
    clean = REF / "sdm_clean.gpkg"
    countries = gpd.read_file(clean, layer="countries")
    land = gpd.read_file(clean, layer="land")
    cases = {
        "buffer": ("bradypus", {"distance": 300_000.0}),
        "buffer_small": ("bradypus", {"distance": 100_000.0, "resolution": 32}),
        "hull": ("bradypus", {"method": "convex_hull"}),
        "hull_wide": ("bradypus", {"method": "convex_hull", "distance": 200_000.0, "resolution": 16}),
        "regions": ("bradypus", {"method": "regions", "_regions": True}),
        "regions_wide": ("bradypus", {"method": "regions", "_regions": True, "distance": 50_000.0, "resolution": 16}),
        "land": ("bradypus", {"distance": 400_000.0, "_within": True}),
        "dateline": ("fiji", {"distance": 150_000.0}),
    }
    out = REF / "sdm_area.gpkg"
    out.unlink(missing_ok=True)
    geod = Geod(ellps="WGS84")
    meta = {}
    for name, (rec, kw) in cases.items():
        args = {k: v for k, v in kw.items() if not k.startswith("_")}
        if kw.get("_regions"):
            args["regions"] = countries
        if kw.get("_within"):
            args["within"] = land
        a = ls.sdm.accessible_area(brad if rec == "bradypus" else fiji, **args)
        a.to_file(out, layer=name, driver="GPKG")
        g = a.geometry.iloc[0]
        meta[name] = {"records": rec, "settings": kw, "area": g.area, "geodesic": abs(geod.geometry_area_perimeter(g)[0]),
                      "bounds": list(g.bounds)}
        print(name, g.geom_type, round(g.area, 4), list(map(lambda v: round(v, 3), g.bounds)))
    (REF / "sdm_area.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
