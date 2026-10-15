"""Figures of "Occurrence data: download, clean, thin" (tutorials/species-distributions/occurrence-data).

    python website/figures/species-distributions/occurrence-data.py

GBIF's records of the sloth (sdm.download_gbif; downloaded once and kept in website/.cache/gbif, so that the
figures can be made again offline), the cleaning of the wild potato Solanum acaule's records by CoordinateCleaner's
tests (sdm.clean_occurrences), and the sloth's records one per cell and thinned at several distances (sdm.thin).
"""

from __future__ import annotations

import csv
import json
import shutil

import numpy as np
import matplotlib.pyplot as plt

import sdmfig as S
from sdmfig import figs

SLUG = "occurrence-data"
IMG = figs.img_dir(f"{S.CAT}/{SLUG}")
DAT = figs.data_dir(f"{S.CAT}/{SLUG}")
GBIF = figs.WEB / ".cache" / "gbif"


def num(c):
    import pandas as pd
    return pd.to_numeric(c, errors="coerce")


def acaule_csv():
    """acaule.csv of dismo, its twelve spellings of the species' name made one (the outlier test compares the
    records of each species), for readers to download."""
    rows = [r for r in csv.reader(open(S.DATA / "acaule.csv", encoding="utf-8")) if r]
    head = rows[0]
    i = head.index("species")
    for r in rows[1:]:
        r[i] = "Solanum acaule"
    with open(DAT / "acaule.csv", "w", newline="", encoding="utf-8") as f:
        csv.writer(f).writerows(rows)
    return len(rows) - 1


def gbif():
    """The sloth's records with coordinates in GBIF, cleaned with the default tests."""
    GBIF.mkdir(parents=True, exist_ok=True)
    cached = GBIF / "bradypus_variegatus.gpkg"
    w = S.work(SLUG + "_gbif", files=())
    if not cached.exists():
        m = S.HEAD + S.node("gbif", "sdm.download_gbif", params={"species": "Bradypus variegatus"})
        m = m + S.node("save", "vector.write", {"vector": "gbif.occurrences"}, {"path": "gbif.gpkg"})
        m += S.outputs({"query": "gbif.query"})
        S.run(m, w, name="download.toml")
        shutil.copy(w / "gbif.gpkg", cached)
        shutil.copy(w / "output" / "query.json", GBIF / "bradypus_variegatus_query.json")
    shutil.copy(cached, w / "gbif.gpkg")
    query = json.loads((GBIF / "bradypus_variegatus_query.json").read_text(encoding="utf-8"))
    m = S.HEAD + S.node("records", "vector.read", params={"path": "gbif.gpkg"})
    m += S.node("clean", "sdm.clean_occurrences", {"occurrences": "records.vector"},
                {"action": "flag", "outliers": True})
    m += S.node("save", "vector.write", {"vector": "clean.occurrences"}, {"path": "flags.gpkg"})
    m += S.outputs({"counts": "clean.counts"})
    S.run(m, w)
    import geopandas as gpd
    g = gpd.read_file(w / "flags.gpkg")
    counts = S.out(w, "counts.csv")[0]
    years = g["year"].dropna().astype(int)
    old = S.table_csv(S.DATA / "bradypus.csv")
    fig, ax = S.map_axes(800, legend_frac=0.26)
    S.base(ax)
    ok = g["flagged"] == 0
    x = g.geometry.x
    y = g.geometry.y
    S.points(ax, x[ok], y[ok], color="#2f6f9f", size=5, edge="none", alpha=0.5)
    S.points(ax, x[~ok], y[~ok], color=S.FLAG, size=12, edge="white")
    S.points(ax, old["lon"], old["lat"], color=S.PRES, size=10, marker="o")
    S.legend(fig, [(f"GBIF, kept ({int(ok.sum()):,})", "#2f6f9f", "o"),
                   (f"GBIF, flagged ({int((~ok).sum()):,})", S.FLAG, "o"),
                   ("dismo's 116 records", S.PRES, "o")], loc=(0.73, 0.75))
    S.save(fig, IMG / "gbif.png")
    # records per decade, by basis of record
    dec = (years // 10 * 10)
    basis = g.loc[years.index, "basisOfRecord"]
    rows = []
    for d in range(1900, 2030, 10):
        sel = dec == d
        r = {"decade": f"{d}s"}
        for b, name in (("HUMAN_OBSERVATION", "Observations"), ("PRESERVED_SPECIMEN", "Specimens")):
            r[name] = int((sel & (basis == b)).sum())
        r["Other"] = int(sel.sum()) - r["Observations"] - r["Specimens"]
        rows.append(r)
    figs.write_csv(DAT / "gbif_decades.csv", rows)
    return query, counts, len(g), int(ok.sum())


def acaule():
    w = S.work(SLUG + "_acaule", files=())
    shutil.copy(DAT / "acaule.csv", w / "acaule.csv")
    m = S.HEAD + S.node("records", "vector.read", params={"path": "acaule.csv"})
    m += S.node("clean", "sdm.clean_occurrences", {"occurrences": "records.vector"},
                {"action": "flag", "countries": "ISO2"})
    m += S.node("save", "vector.write", {"vector": "clean.occurrences"}, {"path": "flags.gpkg"})
    m += S.outputs({"counts": "clean.counts"})
    m2 = S.HEAD + S.node("records", "vector.read", params={"path": "acaule.csv"})
    m2 += S.node("clean", "sdm.clean_occurrences", {"occurrences": "records.vector"}, {"countries": "ISO2"})
    m2 += S.node("save", "vector.write", {"vector": "clean.occurrences"}, {"path": "kept.gpkg"})
    S.run(m, w)
    S.run(m2, w, name="drop.toml")
    import geopandas as gpd
    g = gpd.read_file(w / "flags.gpkg")
    kept = gpd.read_file(w / "kept.gpkg")
    counts = S.out(w, "counts.csv")[0]
    figs.write_csv(DAT / "acaule_counts.csv",
                   [{"test": k, "records": int(float(v))} for k, v in counts.items() if k != "total"])
    lon = num(g["lon"])
    lat = num(g["lat"])
    tests = ["equal", "zeros", "capitals", "centroids", "seas", "countries", "outliers", "gbif", "institutions"]
    fig, ax = S.map_axes(820, legend_frac=0.27)
    S.base(ax)
    ax.set_xlim(-90, -35)
    ax.set_ylim(-56, 15)
    ok = g["flagged"] == 0
    S.points(ax, lon[ok], lat[ok], color="#2f6f9f", size=7, edge="none", alpha=0.6)
    cols = {"equal": "#cc79a7", "zeros": "#56b4e9", "capitals": "#d55e00", "seas": "#0072b2",
            "countries": "#e69f00", "outliers": "#d1495b"}
    items = [(f"Kept ({int(ok.sum())})", "#2f6f9f", "o")]
    for t in ("countries", "outliers", "seas", "capitals", "zeros", "equal"):
        sel = g[f"flag_{t}"] == 1
        if sel.any():
            S.points(ax, lon[sel], lat[sel], color=cols[t], size=22, edge="white", lw=0.6, marker="D", z=4)
            items.append((f"{t} ({int(sel.sum())})", cols[t], "D"))
    S.legend(fig, items, loc=(0.70, 0.55))
    S.save(fig, IMG / "acaule_flags.png")
    # a few flagged records, why
    rows = []
    sus = g[(g["flagged"] == 1) & (g["flag_invalid"] == 0)]
    for _, r in sus.head(12).iterrows():
        why = ", ".join(t for t in tests if r[f"flag_{t}"] == 1)
        rows.append({"country": r["country"], "lon": r["lon"], "lat": r["lat"], "why": why})
    return counts, len(g), len(kept), rows


def thinning():
    w = S.work(SLUG + "_thin")
    rows = []
    frames = []
    dists = [0, 25, 50, 100, 200, 400]
    m = S.HEAD + S.READ
    m += S.node("clean", "sdm.clean_occurrences", {"occurrences": "records.vector"})
    m += S.node("cells", "sdm.one_per_cell", {"occurrences": "clean.occurrences", "raster": "climate.stack"})
    outs = {"cells": "cells.occurrences"}
    for d in dists[1:]:
        m += S.node(f"thin_{d}", "sdm.thin", {"occurrences": "cells.occurrences"}, {"distance": d * 1000})
        m += S.node(f"w_{d}", "vector.write", {"vector": f"thin_{d}.occurrences"}, {"path": f"thin_{d}.gpkg"})
        outs[f"kept_{d}"] = f"thin_{d}.kept"
    m += S.node("w_0", "vector.write", {"vector": "cells.occurrences"}, {"path": "thin_0.gpkg"})
    m += S.node("w_clean", "vector.write", {"vector": "clean.occurrences"}, {"path": "clean.gpkg"})
    m += S.outputs({k: v for k, v in outs.items() if k != "cells"})
    S.run(m, w)
    import geopandas as gpd
    n_clean = len(gpd.read_file(w / "clean.gpkg"))
    for d in dists:
        g = gpd.read_file(w / f"thin_{d}.gpkg")
        lon = num(g["lon"])
        lat = num(g["lat"])
        fig, ax = S.map_axes(700)
        S.base(ax)
        ax.set_xlim(-95, -33)
        ax.set_ylim(-30, 20)
        if d:
            # the thinning distance as circles around the kept records (in degrees at the equator)
            r = d * 1000 / 111325 / 2
            for x, y in zip(lon, lat):
                ax.add_patch(plt.Circle((x, y), r, facecolor=S.PRES, alpha=0.18, edgecolor=S.PRES, linewidth=0.5))
        S.points(ax, lon, lat, size=16)
        ax.text(-94, -28.5, f"{len(g)} records", fontsize=12, fontweight="bold", color=figs.INK)
        S.save(fig, IMG / f"thin_{d}.png")
        rows.append({"distance_km": d, "records": len(g)})
    figs.write_csv(DAT / "thinning.csv", rows)
    return n_clean, rows


if __name__ == "__main__":
    print("acaule rows", acaule_csv())
    print("acaule", acaule())
    print("thin", thinning())
    print("gbif", gbif())
