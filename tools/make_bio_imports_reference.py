# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's Biodiverse imports give, for Terraflux's tests, written to tests/data/reference/bio_imports.json
with their inputs (bio_imports.tif, bio_imports_classes.tif, bio_imports.gpkg, bio_imports_*.zip / .txt / .csv):

    python tools/make_bio_imports_reference.py

Rasters (labels as bands, with a threshold, aggregated to larger cells; values as labels; one label for all),
vector features (points, lines and polygons laid over a fishnet: the cells they touch, the area or length of
their parts; counts from a column, binarised), records in hexagons (pointy and flat, an origin) and H3 cells, and
occurrence records of a GBIF download (a simple CSV in a zip, a tab-separated file, a Darwin Core Archive with
meta.xml, a comma-separated table) cleaned and in cells, also reprojected. Per case the groups, labels, counts,
coordinates and cell sizes.
"""

from __future__ import annotations

import json
import zipfile
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import rasterio
import shapely
from rasterio.transform import Affine

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
BD = ls.bio.BaseData


def dump(bd) -> dict:
    return {"groups": list(bd.groups), "labels": list(bd.labels), "counts": bd.counts.toarray().tolist(),
            "coords": np.asarray(bd.coords, dtype=np.float64).tolist(), "cell_sizes": list(map(float, bd.cell_sizes)),
            "grid": bd.grid if getattr(bd, "grid", None) else None}


def rasters() -> dict:
    ny, nx = 30, 40
    transform = Affine(1000.0, 0.0, 500000.0, 0.0, -1000.0, 8000000.0)
    bands = []
    for k in range(3):
        f = ls.nlm.gaussian_field(nx, ny, autocorr_range=6 + 3 * k, seed=60 + k).values
        f = np.round(f, 3)
        f[k * 5:k * 5 + 4, 10:16] = np.nan
        bands.append(f)
    with rasterio.open(REF / "bio_imports.tif", "w", driver="GTiff", width=nx, height=ny, count=3, dtype="float64",
                       transform=transform, crs="EPSG:32722", nodata=np.nan) as dst:
        for k, b in enumerate(bands):
            dst.write(b, k + 1)
        dst.set_band_description(1, "sp_a")
        dst.set_band_description(2, "sp_b")
    classes = np.floor(bands[0] * 4) + 1
    with rasterio.open(REF / "bio_imports_classes.tif", "w", driver="GTiff", width=nx, height=ny, count=1,
                       dtype="float64", transform=transform, crs="EPSG:32722", nodata=np.nan) as dst:
        dst.write(classes, 1)
    out = {
        "threshold": ("bio_imports.tif", {"threshold": 0.5, "cell_sizes": [3000.0, 3000.0]}),
        "values": ("bio_imports.tif", {}),
        "origin": ("bio_imports.tif", {"threshold": 0.4, "cell_sizes": [2500.0, 2000.0], "cell_origins": [300.0, 700.0]}),
        "given": ("bio_imports.tif", {"threshold": 0.3, "given_label": "all"}),
        "classes": ("bio_imports_classes.tif", {"labels_as_bands": False, "cell_sizes": [2000.0, 2000.0]}),
    }
    return {name: {"file": f, "settings": kw, **dump(BD.from_raster(REF / f, **kw))} for name, (f, kw) in out.items()}


def features() -> dict:
    geoms = [shapely.Polygon([(0, 0), (37, 3), (30, 26), (4, 21)], [[(10, 8), (20, 9), (16, 15)]]),
             shapely.box(42, -12, 61.5, 4.5),
             shapely.MultiPolygon([shapely.box(-20, 30, -5, 41), shapely.box(70, 40, 79, 55)]),
             shapely.LineString([(-15, -15), (12, -3), (33, -25)]),
             shapely.MultiLineString([[(50, 20), (65, 33)], [(0, 50), (25, 48)]]),
             shapely.Point(5.5, 5.5), shapely.Point(-3.2, 17.9), shapely.MultiPoint([(44, 44), (46.5, 41.2)])]
    gdf = gpd.GeoDataFrame({"sp": ["a", "b", "a", "c", "b", "d", "a", "c"], "n": [2, 1, 3, 1, 4, 5, 1, 2]},
                           geometry=geoms, crs="EPSG:32722")
    (REF / "bio_imports.gpkg").unlink(missing_ok=True)
    gdf.to_file(REF / "bio_imports.gpkg", layer="features", driver="GPKG")
    gdf = gpd.read_file(REF / "bio_imports.gpkg", layer="features")
    polys = gdf.iloc[:3]
    lines = gdf.iloc[3:5]
    cases = {
        "touch": (None, {"cell_sizes": [10.0, 10.0]}),
        "touch_origin": (None, {"cell_sizes": [7.0, 9.0], "cell_origins": [2.0, -3.0], "count_columns": ["n"]}),
        "area": ("polygons", {"cell_sizes": [10.0, 10.0], "measure": "area"}),
        "length": ("lines", {"cell_sizes": [8.0, 8.0], "measure": "length"}),
        "binarise": (None, {"cell_sizes": [10.0, 10.0], "count_columns": ["n"], "binarise_counts": True}),
    }
    out = {}
    for name, (part, kw) in cases.items():
        src = {"polygons": polys, "lines": lines}.get(part, gdf)
        out[name] = {"part": part, "settings": kw, **dump(BD.from_vector(src, "sp", **kw))}
    return out


def grids() -> dict:
    rng = np.random.default_rng(3)
    n = 400
    pts = pd.DataFrame({"x": np.round(rng.uniform(0, 60_000, n), 2), "y": np.round(rng.uniform(-30_000, 30_000, n), 2),
                        "sp": rng.choice(["a", "b", "c", "d"], n), "n": rng.integers(1, 4, n).astype(float)})
    ll = pd.DataFrame({"lat": np.round(rng.uniform(-24, -20, n), 5), "lon": np.round(rng.uniform(-48, -44, n), 5),
                       "sp": rng.choice(["a", "b", "c"], n)})
    pts.to_csv(REF / "bio_imports_points.csv", index=False)
    ll.to_csv(REF / "bio_imports_lonlat.csv", index=False)
    pts = pd.read_csv(REF / "bio_imports_points.csv", float_precision="round_trip")
    ll = pd.read_csv(REF / "bio_imports_lonlat.csv", float_precision="round_trip")
    out = {}
    for name, kw in {"pointy": {"size": 5000.0}, "flat": {"size": 7000.0, "orientation": "flat",
                                                          "origin": [1000.0, -2000.0], "count_column": "n"}}.items():
        args = dict(kw)
        if "origin" in args:
            args["origin"] = tuple(args["origin"])
        out[f"hex_{name}"] = {"settings": kw, **dump(BD.from_hex(pts, "x", "y", "sp", **args))}
    for res in (4, 6):
        out[f"h3_{res}"] = {"settings": {"resolution": res}, **dump(BD.from_h3(ll, "lat", "lon", "sp", res))}
    return out


HEADER = ["gbifID", "species", "genus", "specificEpithet", "decimalLongitude", "decimalLatitude",
          "coordinateUncertaintyInMeters", "basisOfRecord", "occurrenceStatus", "issue", "individualCount"]


def occurrence_rows() -> list[list[str]]:
    rng = np.random.default_rng(9)
    rows = []
    sp = [("Aus bus", "Aus", "bus"), ("Aus cus", "Aus", "cus"), ("Dus eus", "Dus", "eus")]
    for k in range(300):
        s = sp[k % 3]
        x, y = rng.uniform(-60, -40), rng.uniform(-25, -5)
        unc = "" if k % 4 == 0 else str(int(rng.integers(1, 5000)))
        bor = ["HUMAN_OBSERVATION", "PRESERVED_SPECIMEN", "human_observation"][k % 3 if k % 7 else 1]
        status = "ABSENT" if k % 23 == 0 else ("" if k % 5 == 0 else "PRESENT")
        issue = "ZERO_COORDINATE;OTHER" if k % 17 == 0 else ("COUNTRY_COORDINATE_MISMATCH" if k % 11 == 0 else "")
        cnt = "" if k % 6 == 0 else str(int(rng.integers(1, 9)))
        rows.append([str(k + 1), f" {s[0]} " if k % 13 == 0 else s[0], s[1], s[2], f"{x:.5f}", f"{y:.5f}", unc, bor,
                     status, issue, cnt])
    rows.append(["900", "", "Xus", "yus", "-50", "-10", "", "HUMAN_OBSERVATION", "PRESENT", "", ""])
    rows.append(["901", "Aus bus", "Aus", "bus", "-200", "-10", "", "HUMAN_OBSERVATION", "PRESENT", "", ""])
    rows.append(["902", "Aus bus", "Aus", "bus", "abc", "-10", "", "HUMAN_OBSERVATION", "PRESENT", "", ""])
    rows.append(rows[5][:])
    rows[-1][0] = "903"
    return rows


def occurrences() -> dict:
    rows = occurrence_rows()
    txt = "\n".join("\t".join(r) for r in [HEADER, *rows]) + "\n"
    (REF / "bio_imports_gbif.txt").write_text(txt, encoding="utf-8")
    with zipfile.ZipFile(REF / "bio_imports_gbif.zip", "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("0001234-260101120000000.csv", txt)
    # a Darwin Core Archive: a quoted, comma-separated core with a header line and the terms in meta.xml
    order = [0, 4, 5, 1, 2, 3, 6, 7, 8, 9, 10]
    terms = {"species": "http://rs.tdwg.org/dwc/terms/species", "genus": "http://rs.tdwg.org/dwc/terms/genus"}
    fields = "".join(f'<field index="{i}" term="{terms.get(HEADER[c], "http://rs.tdwg.org/dwc/terms/" + HEADER[c])}"/>'
                     for i, c in enumerate(order) if i > 0)
    meta = ('<?xml version="1.0" encoding="UTF-8"?><archive xmlns="http://rs.tdwg.org/dwc/text/">'
            '<core encoding="UTF-8" fieldsTerminatedBy="," linesTerminatedBy="\\n" fieldsEnclosedBy="&quot;" '
            'ignoreHeaderLines="1" rowType="http://rs.tdwg.org/dwc/terms/Occurrence">'
            f'<files><location>occurrence.csv</location></files><id index="0"/>{fields}</core></archive>')

    def q(v):
        return '"' + v.replace('"', '""') + '"'
    core = "\n".join(",".join(q(r[c]) for c in order) for r in [HEADER, *rows]) + "\n"
    with zipfile.ZipFile(REF / "bio_imports_dwca.zip", "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("meta.xml", meta)
        z.writestr("occurrence.csv", core)
    pd.DataFrame(rows, columns=HEADER).to_csv(REF / "bio_imports_gbif.csv", index=False)
    cases = {
        "zip": ("bio_imports_gbif.zip", {"cell_sizes": [1.0, 1.0]}),
        "txt": ("bio_imports_gbif.txt", {"cell_sizes": [2.0, 2.0], "max_uncertainty": 1000.0,
                                         "count": "individualCount"}),
        "dwca": ("bio_imports_dwca.zip", {"cell_sizes": [1.0, 1.0], "basis_of_record": ["HUMAN_OBSERVATION"],
                                          "exclude_issues": ["ZERO_COORDINATE"], "label": ["genus", "specificEpithet"]}),
        "csv": ("bio_imports_gbif.csv", {"cell_sizes": [0.5, 0.5], "present_only": False, "drop_duplicates": False}),
        "projected": ("bio_imports_gbif.zip", {"cell_sizes": [100_000.0, 100_000.0], "to_crs": "EPSG:3857"}),
    }
    out = {}
    for name, (f, kw) in cases.items():
        args = dict(kw)
        out[name] = {"file": f, "settings": kw, **dump(BD.from_occurrences(REF / f, **args))}
    return out


def main() -> None:
    out = {"raster": rasters(), "vector": features(), "grids": grids(), "occurrences": occurrences()}
    (REF / "bio_imports.json").write_text(json.dumps(out) + "\n", encoding="utf-8")
    for k, v in out.items():
        print(k, {n: (len(c["groups"]), len(c["labels"])) for n, c in v.items()})


if __name__ == "__main__":
    main()
