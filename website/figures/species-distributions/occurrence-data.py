"""Figures of "Occurrence data: download, clean, thin" (tutorials/species-distributions/occurrence-data).

    python website/figures/species-distributions/occurrence-data.py

GBIF's records of the sloth (sdm.download_gbif; downloaded once and kept in website/.cache/gbif, so that the
figures can be made again offline), the cleaning of the wild potato Solanum acaule's records by CoordinateCleaner's
tests (sdm.clean_occurrences), the sloth's records one per cell and thinned at several distances (sdm.thin), and a
target-group background: the GBIF specimens of Pilosa and Cingulata (also kept in website/.cache/gbif) as a surface
of sampling effort (sdm.bias_surface) that weights sdm.background.

    python website/figures/species-distributions/occurrence-data.py bias     # only the target group (or acaule,
                                                                             # thin, gbif)
"""

from __future__ import annotations

import csv
import json
import shutil
import sys

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


# the target group of the sloth: the museum specimens of its order (sloths and anteaters) and of the armadillos
GROUP = {"Pilosa": 1494, "Cingulata": 735}   # GBIF backbone taxon keys


def group_records(w):
    """The group's specimens with coordinates in GBIF (sdm.download_gbif, kept in website/.cache/gbif), in one
    GeoPackage in the work folder: the file the page's model reads."""
    import geopandas as gpd
    import pandas as pd
    GBIF.mkdir(parents=True, exist_ok=True)
    parts, queries = [], {}
    for name, key in GROUP.items():
        cached = GBIF / f"{name.lower()}_specimens.gpkg"
        query = GBIF / f"{name.lower()}_specimens_query.json"
        if not cached.exists():
            d = S.work(SLUG + "_download", files=())
            m = S.HEAD + S.node("gbif", "sdm.download_gbif",
                                params={"species": key, "basis_of_record": "PRESERVED_SPECIMEN"})
            m += S.node("save", "vector.write", {"vector": "gbif.occurrences"}, {"path": "gbif.gpkg"})
            m += S.outputs({"query": "gbif.query"})
            S.run(m, d, name="download.toml")
            shutil.copy(d / "gbif.gpkg", cached)
            shutil.copy(d / "output" / "query.json", query)
        g = gpd.read_file(cached)
        keep = [c for c in ("species", "family", "year", "basisOfRecord", "institutionCode") if c in g]
        g = g[keep + ["geometry"]]
        g["order"] = name
        parts.append(g)
        queries[name] = json.loads(query.read_text(encoding="utf-8"))
    g = gpd.GeoDataFrame(pd.concat(parts, ignore_index=True), crs=parts[0].crs)
    g.to_file(w / "xenarthra.gpkg", driver="GPKG")
    return g, queries


BIAS_MODEL = S.HEAD + S.READ + '''
[[nodes]]
id = "group"
op = "vector.read"
params = { path = "xenarthra.gpkg" }

[[nodes]]
id = "clean"
op = "sdm.clean_occurrences"
inputs = { occurrences = "records.vector" }

[[nodes]]
id = "cells"
op = "sdm.one_per_cell"
inputs = { occurrences = "clean.occurrences", raster = "climate.raster" }

[[nodes]]
id = "thin"
op = "sdm.thin"
inputs = { occurrences = "cells.occurrences" }
params = { distance = 50000 }

[[nodes]]
id = "clean_group"
op = "sdm.clean_occurrences"
inputs = { occurrences = "group.vector" }

[[nodes]]
id = "effort"
op = "sdm.bias_surface"
inputs = { occurrences = "clean_group.occurrences", env = "climate.stack" }

[[nodes]]
id = "background"
op = "sdm.background"
inputs = { env = "climate.stack", occurrences = "thin.occurrences", bias = "effort.bias" }
params = { n = 2000, tryf = 1, seed = 1 }
'''


def bias():
    """sdm.bias_surface on the group's specimens, the background drawn with it (tryf 1 and the default 3), one
    drawn without, the surface at other bandwidths, and the effort at each background's points."""
    import geopandas as gpd
    from matplotlib import colors as mcolors
    w = S.work(SLUG + "_bias")
    g, queries = group_records(w)
    m = BIAS_MODEL
    m += S.node("random", "sdm.background", {"env": "climate.stack", "occurrences": "thin.occurrences"},
                {"n": 2000, "seed": 1})
    m += S.node("tryf3", "sdm.background",
                {"env": "climate.stack", "occurrences": "thin.occurrences", "bias": "effort.bias"},
                {"n": 2000, "seed": 1, "tryf": 3})
    # the rescaled surface: every cell keeps a chance
    m += S.node("effort_floor", "sdm.bias_surface", {"occurrences": "clean_group.occurrences", "env": "climate.stack"},
                {"rescale": [0.05, 0.95]})
    m += S.node("floor", "sdm.background",
                {"env": "climate.stack", "occurrences": "thin.occurrences", "bias": "effort_floor.bias"},
                {"n": 2000, "seed": 1, "tryf": 1})
    writes = {"thin": "thin.occurrences", "group_clean": "clean_group.occurrences", "bg_group": "background.points",
              "bg_random": "random.points", "bg_tryf3": "tryf3.points", "bg_floor": "floor.points"}
    for f, ref in writes.items():
        m += S.node(f"w_{f}", "vector.write", {"vector": ref}, {"path": f"{f}.gpkg"})
    outs = {"bandwidth": "effort.bandwidth", "counts": "clean_group.counts"}
    m += S.node("w_effort", "raster.write", {"value": "effort.bias"}, {"path": "effort_nrd.tif"})
    for b in (1, 2):
        m += S.node(f"effort_{b}", "sdm.bias_surface",
                    {"occurrences": "clean_group.occurrences", "env": "climate.stack"}, {"bandwidth": b})
        m += S.node(f"w_effort_{b}", "raster.write", {"value": f"effort_{b}.bias"}, {"path": f"effort_{b}.tif"})
    m += S.outputs(outs)
    S.run(m, w)
    bw = S.out(w, "bandwidth.json")
    counts = S.out(w, "counts.csv")[0]
    clean = gpd.read_file(w / "group_clean.gpkg")
    thin = gpd.read_file(w / "thin.gpkg")
    thin = gpd.GeoDataFrame(thin, geometry=gpd.points_from_xy(num(thin["lon"]), num(thin["lat"])))
    # the effort relative to the map's average (1: an average cell), the same colours for every bandwidth
    norm = mcolors.LogNorm(vmin=0.05, vmax=20)
    surf = {}
    for b in ("nrd", 1, 2):
        a, info = figs.read(w / f"effort_{b}.tif", 1)
        a = np.array(np.ma.filled(np.ma.asarray(a, dtype=float), np.nan))
        rel = a / np.nanmean(a)
        surf[b] = rel
        fig, ax = S.map_axes(820, legend_frac=0.22)
        S.base(ax)
        im = S.surface(ax, np.clip(rel, 0.05, 20), info, cmap="viridis", norm=norm)
        S.points(ax, clean.geometry.x, clean.geometry.y, color="white", size=2.0, edge="none", alpha=0.6)
        S.base_limits(ax)
        cb = S.colorbar(fig, im, label="Sampling effort (1 = the map's average)", rect=(0.80, 0.22, 0.03, 0.55),
                        ticks=[0.1, 1, 10])
        cb.ax.set_yticklabels(["0.1", "1", "10"])
        S.save(fig, IMG / f"bias_surface_{b}.png")
    # the backgrounds, drawn alike for a before-and-after
    for name in ("random", "group"):
        p = gpd.read_file(w / f"bg_{name}.gpkg")
        fig, ax = S.map_axes(700)
        S.base(ax)
        S.points(ax, p.geometry.x, p.geometry.y, color=S.BACK, size=3, edge="none", alpha=0.8, z=2)
        S.points(ax, thin.geometry.x, thin.geometry.y, size=12)
        S.base_limits(ax)
        S.save(fig, IMG / f"background_{name}.png")
    # the effort and the climate at each background's points
    rel = surf["nrd"].ravel()
    top = np.nanquantile(rel, 0.75)
    v1 = np.ma.filled(np.ma.asarray(figs.read(S.DATA / "bioclim.tif", 1)[0], dtype=float), np.nan)
    v12 = np.ma.filled(np.ma.asarray(figs.read(S.DATA / "bioclim.tif", 7)[0], dtype=float), np.nan)
    rows = []
    for name, f in (("Random", "bg_random"), ("Target group, tryf = 3", "bg_tryf3"),
                    ("Target group, tryf = 1", "bg_group"), ("Rescaled to [0.05, 0.95], tryf = 1", "bg_floor")):
        c = gpd.read_file(w / f"{f}.gpkg")["cell"].to_numpy().astype(int)
        rows.append({"background": name, "points": len(c), "effort": round(float(np.mean(rel[c])), 2),
                     "top_quarter": round(100 * float(np.mean(rel[c] >= top)), 1),
                     "bio1": float(np.median(np.ravel(v1)[c])), "bio12": float(np.median(np.ravel(v12)[c]))})
    figs.write_csv(DAT / "target_group.csv", [{"Background": r["background"],
                                                "Mean effort at the points": r["effort"],
                                                "Points in the busiest quarter (%)": r["top_quarter"]}
                                               for r in rows])
    stats = {b: {"max_over_median": round(float(np.nanmax(r) / np.nanmedian(r)), 1),
                 "under_0.1": round(100 * float(np.nanmean(r < 0.1)), 1)} for b, r in surf.items()}
    nq = {k: q.get("count") for k, q in queries.items()}
    return {"downloaded": len(g), "per_order": g["order"].value_counts().to_dict(), "gbif_counts": nq,
            "cleaned": len(clean), "counts": counts, "bandwidth": bw, "rows": rows, "surfaces": stats,
            "cells": int(np.isfinite(rel).sum()), "n_institutions": int(clean["institutionCode"].nunique()),
            "years": [int(clean["year"].min()), int(clean["year"].max())] if "year" in clean else None}


if __name__ == "__main__":
    only = sys.argv[1:]
    if not only or "acaule" in only:
        print("acaule rows", acaule_csv())
        print("acaule", acaule())
    if not only or "thin" in only:
        print("thin", thinning())
    if not only or "gbif" in only:
        print("gbif", gbif())
    if not only or "bias" in only:
        print("bias", bias())
