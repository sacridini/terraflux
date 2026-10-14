# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's cleaning of occurrence records gives, for Terraflux's tests, written to
tests/data/reference/sdm_clean.npz and .json, the records as sdm_clean_<case>.csv and the polygons of Natural Earth
the records fall in as sdm_clean.gpkg:

    python tools/make_sdm_clean_reference.py

The records of landschaft's tests of CoordinateCleaner (records near each kind of reference and controls; the
vertices of the circles, the antimeridian and the south pole), acaule's records (invalid ones too) and a species of
more than 10 000 records (cc_outl's raster variant): every test with its defaults and with other options. Natural
Earth's land, urban areas and countries are given as the polygons whose boxes hold a record (the same results as
the whole layers, which Terraflux downloads).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd

import landschaft as ls
from landschaft.sdm._refdata import reference_data

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))
import sdm_cases  # noqa: E402

OFF = dict(equal=False, zeros=False, capitals=False, centroids=False, seas=False, urban=False, countries=False,
           outliers=False, gbif=False, institutions=False, duplicates=False, aohi=False)


def large() -> pd.DataFrame:
    """A species of 10 500 records around three centres and a few far away, and a small one."""
    rng = np.random.default_rng(5)
    centres = np.array([[-55.0, -12.0], [-48.0, -20.0], [-62.0, -5.0]])
    k = rng.integers(0, 3, 10_480)
    xy = centres[k] + rng.normal(0, 1.2, (k.size, 2))
    far = np.column_stack([rng.uniform(-20, 40, 20), rng.uniform(-40, 50, 20)])
    xy = np.round(np.vstack([xy, far]), 3)
    d = pd.DataFrame({"lon": xy[:, 0], "lat": xy[:, 1], "species": "big"})
    small = pd.DataFrame({"lon": np.round(rng.uniform(-70, -60, 30), 2), "lat": np.round(rng.uniform(-30, -20, 30), 2),
                          "species": "small"})
    return pd.concat([d, small], ignore_index=True)


def records() -> dict[str, pd.DataFrame]:
    acaule = pd.read_csv(ROOT / "extern" / "landschaft" / "src" / "landschaft" / "sdm" / "data" / "acaule.csv")
    acaule = acaule[["species", "country", "ISO2", "lat", "lon", "institution"]]
    return {"synthetic": sdm_cases.cc_synthetic(), "edges": sdm_cases.cc_edges(), "acaule": acaule, "large": large()}


def subset(layer: str, recs: list[pd.DataFrame], columns: list[str]) -> gpd.GeoDataFrame:
    """The polygons of a Natural Earth layer whose boxes hold a record."""
    g = reference_data(layer)
    b = g.geometry.bounds.to_numpy()
    keep = np.zeros(len(g), dtype=bool)
    for d in recs:
        x = pd.to_numeric(d["lon"], errors="coerce").to_numpy(float)
        y = pd.to_numeric(d["lat"], errors="coerce").to_numpy(float)
        ok = np.isfinite(x) & np.isfinite(y)
        x, y = x[ok], y[ok]
        for i in range(len(g)):
            if not keep[i]:
                keep[i] = bool(((x >= b[i, 0]) & (x <= b[i, 2]) & (y >= b[i, 1]) & (y <= b[i, 3])).any())
    out = g.loc[keep, columns + ["geometry"]].reset_index(drop=True)
    return gpd.GeoDataFrame(out, geometry="geometry", crs="EPSG:4326")


def main() -> None:
    recs = records()
    for name, d in recs.items():  # written and read back as Terraflux reads them
        d.to_csv(REF / f"sdm_clean_{name}.csv", index=False)
        recs[name] = pd.read_csv(REF / f"sdm_clean_{name}.csv", float_precision="round_trip")
    gpkg = REF / "sdm_clean.gpkg"
    gpkg.unlink(missing_ok=True)
    layers = {"land": subset("land50", list(recs.values()), []),
              "urban": subset("urban50", list(recs.values()), []),
              "countries": subset("countries50", list(recs.values()), ["iso_a2_eh", "iso_a3_eh"])}
    for k, g in layers.items():
        g.to_file(gpkg, layer=k, driver="GPKG")
    land, urban, countries = (gpd.read_file(gpkg, layer=k) for k in layers)
    dateline = pd.DataFrame({"lon": [179.995], "lat": [-16.5]})
    dateline.to_csv(REF / "sdm_clean_dateline.csv", index=False)

    # case: (records, settings of Terraflux's node, landschaft's arguments)
    full = dict(urban=True, countries="iso3", duplicates=True, aohi=True)
    cases: dict[str, tuple[str, dict, dict]] = {
        "synthetic": ("synthetic", full, full),
        "synthetic_mad": ("synthetic", {**OFF, "outliers": {"method": "mad"}}, {**OFF, "outliers": {"method": "mad"}}),
        "synthetic_distance": ("synthetic", {**OFF, "outliers": {"method": "distance", "tdi": 500}},
                               {**OFF, "outliers": {"method": "distance", "tdi": 500}}),
        "synthetic_thin": ("synthetic", {**OFF, "outliers": {"thinning": True}}, {**OFF, "outliers": {"thinning": True}}),
        "synthetic_verify": ("synthetic", {**OFF, "capitals": {"verify": True}, "centroids": {"verify": True},
                                           "gbif": {"verify": True, "buffer": 5000},
                                           "institutions": {"verify": True, "buffer": 200}},
                             {**OFF, "capitals": {"verify": True}, "centroids": {"verify": True},
                              "gbif": {"verify": True, "buffer": 5000}, "institutions": {"verify": True, "buffer": 200}}),
        "synthetic_options": ("synthetic", {**OFF, "capitals": {"buffer": 60000, "geod": False},
                                            "centroids": {"detail": "provinces", "buffer": 2000},
                                            "institutions": {"geod": True, "buffer": 300}, "zeros": 1,
                                            "aohi": {"taxa": ["Aves"], "buffer": 50000},
                                            "duplicates": {"additions": ["iso3"]}},
                              {**OFF, "capitals": {"buffer": 60000, "geod": False},
                               "centroids": {"detail": "provinces", "buffer": 2000},
                               "institutions": {"geod": True, "buffer": 300}, "zeros": 1,
                               "aohi": {"taxa": ["Aves"], "buffer": 50000}, "duplicates": {"additions": ["iso3"]}}),
        "acaule": ("acaule", {"countries": "ISO2", "duplicates": {"additions": ["institution"]}},
                   {"countries": "ISO2", "duplicates": {"additions": ["institution"]}}),
        "acaule_country": ("acaule", {**OFF, "centroids": {"detail": "country"}, "outliers": {"method": "mad",
                                                                                              "mltpl": 3}},
                           {**OFF, "centroids": {"detail": "country"}, "outliers": {"method": "mad", "mltpl": 3}}),
        "edges_cap": ("edges", {**OFF, "capitals": 10000}, {**OFF, "capitals": 10000}),
        "edges_cap_terra": ("edges", {**OFF, "capitals": {"geod": False}}, {**OFF, "capitals": {"geod": False}}),
        "edges_dateline": ("edges", {**OFF, "capitals": True, "_capitals_ref": "dateline"},
                           {**OFF, "capitals": {"ref": dateline}}),
        "edges_dateline_terra": ("edges", {**OFF, "capitals": {"geod": False}, "_capitals_ref": "dateline"},
                                 {**OFF, "capitals": {"ref": dateline, "geod": False}}),
        "edges_pole": ("edges", {**OFF, "centroids": {"detail": "country"}}, {**OFF, "centroids": {"detail": "country"}}),
        "edges_pole_big": ("edges", {**OFF, "centroids": {"detail": "country", "buffer": 60000}},
                           {**OFF, "centroids": {"detail": "country", "buffer": 60000}}),
        "edges_zeros": ("edges", {**OFF, "zeros": 0}, {**OFF, "zeros": 0}),
        "large": ("large", {**OFF, "outliers": True}, {**OFF, "outliers": True}),
        "large_mad": ("large", {**OFF, "outliers": {"method": "mad"}}, {**OFF, "outliers": {"method": "mad"}}),
        "large_distance": ("large", {**OFF, "outliers": {"method": "distance", "tdi": 1500}},
                           {**OFF, "outliers": {"method": "distance", "tdi": 1500}}),
    }
    out: dict[str, np.ndarray] = {}
    meta = {}
    for case, (rec, settings, kwargs) in cases.items():
        kw = dict(kwargs)
        for test, ref in (("seas", land), ("urban", urban)):
            if kw.get(test, test == "seas") is not False and not isinstance(kw.get(test), dict):
                kw[test] = ref
        if kw.get("countries"):
            kw["countries"] = {"column": kw["countries"], "ref": countries}
        f = ls.sdm.clean_occurrences(recs[rec], action="flag", **kw)
        for c in f.columns:
            if c.startswith("flag"):
                out[f"{case}/{c}"] = f[c].to_numpy(np.float64)
        counts = f.attrs["clean"]
        out[f"{case}/counts"] = np.array(list(counts.values()), dtype=np.float64)
        meta[case] = {"records": rec, "settings": settings, "counts": list(counts)}
        print(case, counts)
    np.savez_compressed(REF / "sdm_clean.npz", **out)
    (REF / "sdm_clean.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
