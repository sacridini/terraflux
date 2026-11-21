"""Figures of "Observed corridors and validating connectivity"
(website/content/tutorials/animal-movement/corridors-and-validation.md).

The surface (an Omniscape current from the land cover), the dynamic Brownian bridge UDs, LaPoint's corridor segments,
the validation (move.validate) and the distance between UDs (move.ud_distance) are run by the terraflux binary, as
the page's models are; the numbers are checked against the terraflux library (tf.move) on the same tracks. The values
along the steps (the relief) are the operator's too. What the page tells about the two agreeing is printed here.

    PYTHONPATH=extern/terraflux-py/src TERRAFLUX_BIN=target/debug/terraflux.exe \\
        python website/figures/animal-movement/corridors-and-validation.py
"""

import os
import time

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
import terraflux as tf
from matplotlib.collections import LineCollection

import movefig as mf
from movefig import figs

SLUG = "corridors-and-validation"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")
ID = mf.ID
NT = min(8, int(os.environ.get("HEAVY_CORES", "4")))
CENTER = [-73.88, 42.79]
AEQD = "+proj=aeqd +lat_0=42.79 +lon_0=-73.88 +datum=WGS84 +units=m"
EXT = [-74.0, 42.65, -73.74, 42.90]
ERR = 20
GAP = 2  # hours: the bridges across longer gaps are left out of the distributions
# the thresholds of the corridors, given once for all the animals: the node's own (the 0.75 quantile of the speeds and the
# 0.25 quantile of the circular variances of these segments) are 0.157 m/s and 0.376 and give 17 corridor segments
# (printed below); these are chosen, a third of the speed and half of the variance, for more of them
SPEED, CIRCVAR = 0.05, 0.2
RESULTS = {}
CROP = (-6500, 8500, -11000, 8500)  # the map shown, metres from the centre: where the animals are

# the resistance of each land cover class (illustrative, not calibrated): tree cover 1, ... water 500
RULES = "10 -> 1\n20 -> 5\n30 -> 10\n40 -> 20\n50 -> 100\n60 -> 50\n80 -> 500\n90 -> 5\n* -> nan"

BASE = list(mf.CLEAN) + [
    {"id": "fishers", "op": "vector.filter", "inputs": {"vector": "unique.tracks"},
     "params": {"expression": 'individual_local_identifier != "M5"'}},
    {"id": "aeqd", "op": "move.aeqd_crs", "inputs": {"tracks": "fishers.vector"}, "params": {"center": CENTER}},
]
SURFACE = [
    {"id": "cover", "op": "data.download",
     "params": {"source": "esa_worldcover", "extent": EXT, "cellsize": 0.0003, "resampling": "mode"}},
    {"id": "flat", "op": "raster.reproject", "inputs": {"raster": "cover.raster"},
     "params": {"crs": AEQD, "resolution": 100, "resampling": "mode"}},
    {"id": "resistance", "op": "raster.reclassify", "inputs": {"map": "flat.raster"}, "params": {"rules": RULES}},
    {"id": "current", "op": "circuit.omniscape", "inputs": {"resistance": "resistance.map"},
     "params": {"radius": 30, "block_size": 5}},
    {"id": "uniform", "op": "raster.calc", "inputs": {"r": "resistance.map"}, "params": {"expression": "r * 0 + 1"}},
    {"id": "control", "op": "circuit.omniscape", "inputs": {"resistance": "uniform.map"},
     "params": {"radius": 30, "block_size": 5}},
    {"id": "w_res", "op": "raster.write", "inputs": {"value": "resistance.map"}, "params": {"path": "resistance.tif"}},
    {"id": "w_cur", "op": "raster.write", "inputs": {"value": "current.current"}, "params": {"path": "current.tif"}},
    {"id": "w_ctl", "op": "raster.write", "inputs": {"value": "control.current"}, "params": {"path": "control.tif"}},
    {"id": "w_cov", "op": "raster.write", "inputs": {"value": "cover.raster"}, "params": {"path": "cover.tif"}},
]
THIN = [{"id": "thin", "op": "move.thin_time", "inputs": {"tracks": "aeqd.tracks"}, "params": {"interval": "30 mins"}}]


def bridges(tracks, like, name="ud", **grid):
    """The variance of the bridges, the bridges across gaps of more than 2 hours left out, and the distribution."""
    ins = {"tracks": f"{name}_masked.tracks"}
    if like:
        ins["like"] = like
    return [
        {"id": f"{name}_var", "op": "move.dbbmm_variance", "inputs": {"tracks": tracks},
         "params": {"location_error": ERR}},
        {"id": f"{name}_masked", "op": "move.mask_segments", "inputs": {"tracks": f"{name}_var.tracks"},
         "params": {"gap": GAP}},
        {"id": name, "op": "move.dbbmm_ud", "inputs": ins, "params": grid},
    ]


def lib_bridges(tracks, nt, **grid):
    """The same with the library."""
    v = tf.move.dbbmm_variance(tracks, location_error=ERR, n_threads=nt)
    nxt = tracks.groupby(ID)["timestamp"].shift(-1)
    gap = ((nxt - tracks["timestamp"]).dt.total_seconds() / 3600 > GAP).to_numpy()
    return tf.move.dbbmm_ud(tf.move.mask_segments(v, gap), n_threads=nt, **grid), int(gap.sum())


def validate_node(name, use, surface, **params):
    return {"id": name, "op": "move.validate", "inputs": {"use": use, "surface": surface}, "params": params}


def main():
    w = mf.work(SLUG)
    t = time.time()
    # ------------------------------------------------------------------ the surface and the tracks, by the operators
    nodes = BASE + THIN + SURFACE + [
        *bridges("thin.tracks", "resistance.map"),
        {"id": "thresholds", "op": "move.corridor", "inputs": {"tracks": "thin.tracks"}},
        {"id": "corridor", "op": "move.corridor", "inputs": {"tracks": "thin.tracks"},
         "params": {"speed_threshold": SPEED, "circvar_threshold": CIRCVAR}},
        validate_node("v_ud", "ud.ud", "current.current", kind="current", quantile=0.9),
        validate_node("v_lines", "corridor.lines", "current.current", kind="current", quantile=0.9, track=ID),
        validate_node("v_ud_res", "ud.ud", "resistance.map", kind="resistance", quantile=0.9),
        validate_node("v_ud_ctl", "ud.ud", "control.current", kind="current", quantile=0.9),
        validate_node("v_lines_ctl", "corridor.lines", "control.current", kind="current", quantile=0.9, track=ID),
        validate_node("v_pts", "thin.tracks", "current.current", kind="current", quantile=0.9),
        validate_node("v_q50", "ud.ud", "current.current", quantile=0.5),
        validate_node("v_q75", "ud.ud", "current.current", quantile=0.75),
        validate_node("v_q95", "ud.ud", "current.current", quantile=0.95),
    ]
    outs = {"thin": "thin.tracks", "corridor": "corridor.tracks", "lines": "corridor.lines",
            "thr": "thresholds.thresholds", "v_ud": "v_ud.table", "v_lines": "v_lines.table",
            "v_ud_res": "v_ud_res.table", "v_ud_ctl": "v_ud_ctl.table", "v_lines_ctl": "v_lines_ctl.table",
            "v_pts": "v_pts.table", "v_q50": "v_q50.table", "v_q75": "v_q75.table", "v_q95": "v_q95.table"}
    nodes += [
        {"id": "w_ud", "op": "raster.write", "inputs": {"value": "ud.ud"}, "params": {"path": "ud.tif"}},
        {"id": "w_cor", "op": "raster.write", "inputs": {"value": "v_ud.corridor"}, "params": {"path": "best10.tif"}},
    ]
    got = mf.run(w, nodes, outs)
    print(f"model run in {time.time() - t:.1f} s")
    thr = got["thr"]
    print("default thresholds:", thr.to_dict("records"))
    print("note of the UD:", (w / "output" / "v_ud_note.txt").read_text() if (w / "output" / "v_ud_note.txt").exists() else "")

    # ------------------------------------------------------------------ the same with the library
    tr = mf.clean_tracks()
    tr = tr[tr[ID] != "M5"].reset_index(drop=True)
    p, crs = mf.projected(tr, center=tuple(CENTER))
    p30 = mf.thin(p, "30 mins")
    # the operators' own thinned tracks, projected by the Rust projection, go to the library: the same inputs
    same = len(p30) == len(got["thin"])
    print("thinned tracks: library", len(p30), "operator", len(got["thin"]))
    thin_op = got["thin"].reset_index(drop=True)
    d = np.hypot(thin_op.geometry.x.to_numpy() - p30.geometry.x.to_numpy(),
                 thin_op.geometry.y.to_numpy() - p30.geometry.y.to_numpy()) if same else [np.nan]
    print("largest distance between the projected locations of the operator and the library (m):", float(np.nanmax(d)))
    RESULTS["projection_diff_m"] = float(np.nanmax(d))
    thin_op["timestamp"] = pd.to_datetime(thin_op["timestamp"], utc=True)
    p30 = thin_op
    print("30-minute tracks:", p30[ID].value_counts().sort_index().to_dict(), "operator:", len(got["thin"]))
    RESULTS["n30"] = len(p30)
    res = tf.load_raster(w / "resistance.tif")
    cur = tf.load_raster(w / "current.tif")
    ctl = tf.load_raster(w / "control.tif")
    print("surface:", res.shape, "resistance", float(res.min()), float(res.max()), "current", float(cur.min()),
          float(cur.max()))
    RESULTS["grid"] = list(res.shape)
    ud, n_gap = lib_bridges(p30, NT, like=res)
    print("bridges across gaps of more than", GAP, "hours left out:", n_gap)
    RESULTS["gap_segments"] = n_gap
    cor_default = tf.move.corridor(p30)
    print("library default thresholds:", cor_default.attrs.get("corridor"))
    cor = tf.move.corridor(p30, speed_threshold=SPEED, circvar_threshold=CIRCVAR)
    lines = tf.move.corridor_lines(cor)
    print("corridor segments:", len(lines), "of", int(cor.corridor_speed.notna().sum()), "segments; operator lines:",
          len(got["lines"]))
    RESULTS["segments"] = int(cor.corridor_speed.notna().sum())
    RESULTS["corridor_segments"] = len(lines)
    v_ud = tf.move.validate(ud, cur, kind="current", quantile=0.9)
    v_lines = tf.move.validate(lines, cur, kind="current", quantile=0.9, track=ID)

    def worst(a, b, cols=("rho", "use_fraction", "area_fraction", "use_ratio")):
        a = a.reset_index(drop=True)
        b = b.reset_index(drop=True)
        return max(float(np.nanmax(np.abs(a[c].astype(float) - b[c].astype(float)))) for c in cols)

    d1 = worst(v_ud.table, got["v_ud"])
    d2 = worst(v_lines.table, got["v_lines"])
    print("operator vs library, validate of the UD:", f"{d1:.2e}", "of the lines:", f"{d2:.2e}")
    RESULTS["validate_parity"] = max(d1, d2)
    # the library read the current from a GeoTIFF (float32) that the operator had in memory: give both the same file
    same_nodes = BASE + THIN + [
        {"id": "cover", "op": "raster.read", "params": {"path": "resistance.tif"}},
        {"id": "surface", "op": "raster.read", "params": {"path": "current.tif"}},
        *bridges("thin.tracks", "cover.raster"),
        validate_node("v_ud", "ud.ud", "surface.raster", kind="current", quantile=0.9),
    ]
    g_same = mf.run(w, same_nodes, {"v_ud": "v_ud.table"}, name="same.toml")
    d3 = worst(v_ud.table, g_same["v_ud"])
    print("operator vs library, validate of the UD against the same float32 surface:", f"{d3:.2e}")
    RESULTS["validate_parity_same_file"] = d3

    # ------------------------------------------------------------------ the sensitivity of the corridors
    rows = []
    for label, kw in [("the default: quantiles of these tracks", {}),
                      (f"speed {SPEED} m/s, circular variance {CIRCVAR}", dict(speed_threshold=SPEED, circvar_threshold=CIRCVAR)),
                      ("speed 0.02 m/s", dict(speed_threshold=0.02, circvar_threshold=CIRCVAR)),
                      ("speed 0.10 m/s", dict(speed_threshold=0.10, circvar_threshold=CIRCVAR)),
                      ("circular variance 0.1", dict(speed_threshold=SPEED, circvar_threshold=0.1)),
                      ("circular variance 0.4", dict(speed_threshold=SPEED, circvar_threshold=0.4)),
                      ("min_segments 5", dict(speed_threshold=SPEED, circvar_threshold=CIRCVAR, min_segments=5)),
                      ("min_segments 10", dict(speed_threshold=SPEED, circvar_threshold=CIRCVAR, min_segments=10))]:
        c = tf.move.corridor(p30, **kw)
        used = c.attrs["corridor"]
        n = int((c.corridor == "corridor").sum())
        rows.append({"setting": label, "speed threshold (m/s)": round(float(used["speed_threshold"]), 3),
                     "circular variance": round(float(used["circvar_threshold"]), 3), "corridor segments": n,
                     "share of all segments": round(n / RESULTS["segments"] * 100, 1)})
        print(rows[-1])
    figs.write_csv(DATA / "corridor-sensitivity.csv", rows)

    # ------------------------------------------------------------------ the validation tables
    def pooled(tab):
        return tab[tab[tab.columns[0]].astype(str) == "pooled"].iloc[0]

    vrows = []
    for surface, use, tab in [("Omniscape current", "the UD", got["v_ud"]), ("Omniscape current", "corridor segments",
                                                                         got["v_lines"]),
                              ("Omniscape current", "the locations", got["v_pts"]),
                              ("resistance (lower is better)", "the UD", got["v_ud_res"]),
                              ("uniform resistance (a control)", "the UD", got["v_ud_ctl"]),
                              ("uniform resistance (a control)", "corridor segments", got["v_lines_ctl"])]:
        r = pooled(tab)
        vrows.append({"surface": surface, "use": use, "Spearman's rho": round(float(r.rho), 3),
                      "use in the best 10 %": round(float(r.use_fraction) * 100, 1),
                      "area of the best 10 %": round(float(r.area_fraction) * 100, 1),
                      "use ratio": round(float(r.use_ratio), 2), "cells": int(r.n_cells)})
        print(vrows[-1])
    figs.write_csv(DATA / "validation.csv", vrows)

    ua = got["v_ud"]
    arows = []
    for r in ua.itertuples():
        arows.append({"animal": r[1], "cells": int(r.n_cells),
                      "Spearman's rho": round(float(r.rho), 3), "use in the best 10 %": round(float(r.use_fraction) * 100, 1),
                      "use ratio": round(float(r.use_ratio), 2), "coverage": round(float(r.coverage), 3)})
    figs.write_csv(DATA / "validation-animals.csv", arows)
    print(arows)

    qrows = []
    for q, key in [(0.5, "v_q50"), (0.75, "v_q75"), (0.9, "v_ud"), (0.95, "v_q95")]:
        r = pooled(got[key])
        qrows.append({"corridors are the best": f"{round((1 - q) * 100):d} % of the cells",
                      "use in the corridors": round(float(r.use_fraction) * 100, 1),
                      "area of the corridors": round(float(r.area_fraction) * 100, 1),
                      "use ratio": round(float(r.use_ratio), 2)})
    figs.write_csv(DATA / "validation-quantile.csv", qrows)
    print(qrows)

    # ------------------------------------------------------------------ figures: the surface, the corridors
    bounds = [float(res.x.min()), float(res.x.max()), float(res.y.min()), float(res.y.max())]
    ext = [bounds[0] - 50, bounds[1] + 50, bounds[2] - 50, bounds[3] + 50]
    rv = np.asarray(res.values, float)
    cv = np.asarray(cur.values, float)
    fig, axs = plt.subplots(1, 2, figsize=(9.6, 4.9), dpi=150, sharex=True, sharey=True)
    ax = axs[0]
    ax.imshow(np.log10(rv), extent=ext, cmap="Greys", vmin=0, vmax=3.0, alpha=0.75)
    ax.set_title("Resistance (log scale) and the corridor segments", fontsize=10, loc="left")
    allseg = [np.array([[a.x, a.y], [b.x, b.y]]) for a, b in zip(p30.geometry[:-1], p30.geometry[1:])]
    same = (p30[ID].values[:-1] == p30[ID].values[1:])
    allseg = [s for s, k in zip(allseg, same) if k]
    ax.add_collection(LineCollection(allseg, colors=figs.INK2, linewidths=0.35, alpha=0.5))
    ax.add_collection(LineCollection([np.array(g.coords) for g in lines.geometry], colors=figs.EMBER, linewidths=1.4))
    ax = axs[1]
    im = ax.imshow(np.log10(np.where(cv > 0, cv, np.nan)), extent=ext, cmap="viridis")
    ax.set_title("Omniscape current (log scale) and the use", fontsize=10, loc="left")
    for i, a in enumerate(sorted(p30[ID].unique())):
        s = p30[p30[ID] == a]
        ax.scatter(s.geometry.x, s.geometry.y, s=1.2, color="white", alpha=0.55, lw=0)
    for a in axs:
        mf.kmaxes(a, None, grid=False)
    axs[1].set_ylabel("")
    axs[0].set_xlim(CROP[0], CROP[1])
    axs[0].set_ylim(CROP[2], CROP[3])
    mf.kmscale(axs[0], 2)
    fig.colorbar(im, ax=axs[1], shrink=0.8).set_label("log10 of the current", fontsize=9)
    fig.tight_layout()
    mf.save(fig, OUT / "surface.png")

    # the corridors of the animals and those of the surface
    best = tf.load_raster(w / "best10.tif")
    fig, ax = plt.subplots(figsize=(6.6, 5.2), dpi=150)
    ax.imshow(np.where(np.asarray(best.values) == 1, 1.0, np.nan), extent=ext, cmap="Greens", vmin=0, vmax=1.6, alpha=0.9)
    ax.imshow(np.where(np.isnan(rv), np.nan, 0.0), extent=ext, cmap="Greys", vmin=0, vmax=3, alpha=0.1)
    ax.add_collection(LineCollection([np.array(g.coords) for g in lines.geometry], colors=figs.EMBER, linewidths=1.3))
    mf.kmaxes(ax, None, grid=False)
    ax.set_xlim(CROP[0], CROP[1])
    ax.set_ylim(CROP[2], CROP[3])
    mf.kmscale(ax, 2)
    ax.plot([], [], color="#7fbf95", lw=6, label="best 10 % of the current")
    ax.plot([], [], color=figs.EMBER, lw=1.5, label="corridor segments of the animals")
    ax.legend(loc="upper center", bbox_to_anchor=(0.5, -0.12), ncol=2, fontsize=8.5, frameon=False)
    fig.tight_layout()
    mf.save(fig, OUT / "corridors.png")

    # the validation as a chart: how many times more of the use than of the area
    def vchart(ax):
        keep = [i for i, r in enumerate(vrows) if np.isfinite(r["use ratio"])]
        short = ["UD, current", "segments, current", "locations, current", "UD, resistance", "UD, control",
                 "segments, control"]
        cols = [figs.FOREST, figs.FOREST, figs.FOREST, figs.WATER, "#9a9a9a", "#9a9a9a"]
        ax.bar(range(len(keep)), [vrows[i]["use ratio"] for i in keep], color=[cols[i] for i in keep])
        for j, i in enumerate(keep):
            ax.text(j, vrows[i]["use ratio"] + 0.05, f"{vrows[i]['use ratio']:.2f}", ha="center", fontsize=9)
        ax.axhline(1, color=figs.EMBER, lw=1.2, ls="--")
        ax.text(len(keep) - 0.45, 1.05, "chance", color=figs.EMBER, ha="right", fontsize=9)
        ax.set_xticks(range(len(keep)), [short[i] for i in keep], fontsize=8.5)
        ax.set_ylabel("Use ratio")

    figs.chart(OUT / "use-ratio.png", vchart, width=900, height=400)

    # ------------------------------------------------------------------ the distance between UDs
    animals = ["F1", "F2", "F3", "M1", "M2", "M3"]
    six = p30[p30[ID].isin(animals)].reset_index(drop=True)
    ud6, _ = lib_bridges(six, NT, res=250)
    print("six UDs:", ud6.shape)
    t = time.time()
    sk = tf.move.ud_distance(ud6, n_threads=NT)
    t_sk = time.time() - t
    t = time.time()
    ex = tf.move.ud_distance(ud6, method="exact")
    t_ex = time.time() - t
    print(f"sinkhorn {t_sk:.1f} s, exact {t_ex:.1f} s")
    m = sk.merge(ex, on=["a", "b"], suffixes=("_sinkhorn", "_exact"))
    m["difference %"] = (m.distance_sinkhorn / m.distance_exact - 1) * 100
    print(m.round(3).to_string())
    RESULTS["emd_time"] = [t_sk, t_ex]
    RESULTS["emd_max_diff_pct"] = float(m["difference %"].abs().max())
    figs.write_csv(DATA / "ud-distance-pairs.csv", [
        {"pair": f"{r.a} and {r.b}", "Sinkhorn (m)": round(r.distance_sinkhorn), "exact (m)": round(r.distance_exact),
         "difference (%)": round(r._5, 2)} for r in m.itertuples()])
    # a matrix (Sinkhorn, km) for a heat table
    mat = pd.DataFrame(0.0, index=animals, columns=animals)
    for r in sk.itertuples():
        mat.loc[r.a, r.b] = mat.loc[r.b, r.a] = r.distance / 1000
    mat.insert(0, "animal", mat.index)
    figs.write_csv(DATA / "ud-distance-matrix.csv", [{"animal": i, **{c: round(float(mat.loc[i, c]), 2) for c in animals}}
                                                     for i in animals])

    # operators against the library, on the same UDs
    dist_nodes = BASE + THIN + [
        {"id": "six", "op": "vector.filter", "inputs": {"vector": "thin.tracks"},
         "params": {"expression": 'individual_local_identifier != "M4"'}},
        *bridges("six.vector", None, name="ud6", res=250),
        {"id": "dist", "op": "move.ud_distance", "inputs": {"ud": "ud6.ud"}},
        {"id": "dist_exact", "op": "move.ud_distance", "inputs": {"ud": "ud6.ud"}, "params": {"method": "exact"}},
    ]
    try:
        g2 = mf.run(w, dist_nodes, {"dist": "dist.table", "dist_exact": "dist_exact.table"}, name="dist.toml")
        dd = np.nanmax(np.abs(g2["dist"]["distance"].values - sk.distance.values) / sk.distance.values)
        de = np.nanmax(np.abs(g2["dist_exact"]["distance"].values - ex.distance.values) / ex.distance.values)
        print("operator vs library, ud_distance (relative):", f"sinkhorn {dd:.2e}", f"exact {de:.2e}")
        RESULTS["emd_parity"] = [float(dd), float(de)]
    except SystemExit as e:
        print("dist model:", str(e)[:800])

    pair = ud6.isel(band=[animals.index("F2"), animals.index("M1")])
    rrows = []
    for reg in (0.5, 0.2, 0.1, 0.05, 0.02, 0.01):
        t = time.time()
        d = tf.move.ud_distance(pair.isel(band=0), pair.isel(band=1), reg=reg, max_iter=2000, n_threads=NT)
        dt = time.time() - t
        rrows.append({"reg": reg, "distance (m)": round(float(d)), "time (s)": round(dt, 2)})
    d_ex = tf.move.ud_distance(pair.isel(band=0), pair.isel(band=1), method="exact")
    for r in rrows:
        r["difference to exact (%)"] = round((r["distance (m)"] / d_ex - 1) * 100, 2)
    rrows.append({"reg": "exact", "distance (m)": round(float(d_ex)), "time (s)": round(t_ex / len(m), 2),
                  "difference to exact (%)": 0.0})
    figs.write_csv(DATA / "ud-distance-reg.csv", rrows)
    print(rrows)

    def emd_chart(ax):
        ax.scatter(m.distance_exact / 1000, m.distance_sinkhorn / 1000, s=28, color=figs.FOREST, zorder=3)
        hi = max(m.distance_exact.max(), m.distance_sinkhorn.max()) / 1000 * 1.05
        ax.plot([0, hi], [0, hi], color=figs.EMBER, lw=1.2, ls="--")
        ax.set_xlabel("Exact earth mover's distance (km)")
        ax.set_ylabel("Sinkhorn (km)")
        ax.set_aspect(1)

    figs.chart(OUT / "emd.png", emd_chart, width=520, height=470)

    # ------------------------------------------------------------------ the values along the steps: the relief
    relief_nodes = BASE + [
        {"id": "dem", "op": "data.download",
         "params": {"source": "copernicus_dem", "dem_resolution": 30, "extent": EXT, "cellsize": 0.0003}},
        {"id": "resample", "op": "move.resample", "inputs": {"tracks": "aeqd.tracks"},
         "params": {"rate": "10min", "tolerance": "1min"}},
        {"id": "bursts", "op": "move.filter_min_n_burst", "inputs": {"tracks": "resample.tracks"},
         "params": {"min_n": 3}},
        {"id": "steps", "op": "move.steps", "inputs": {"tracks": "bursts.tracks"}},
        {"id": "random", "op": "move.random_steps", "inputs": {"steps": "steps.steps"},
         "params": {"n_control": 5, "seed": 1}},
        {"id": "along", "op": "move.extract_covariates", "inputs": {"tracks": "random.steps", "elevation": "dem.raster"},
         "params": {"where": "along", "stats": ["mean", "ascent", "descent", "delta"]}},
        {"id": "w_dem", "op": "raster.write", "inputs": {"value": "dem.raster"}, "params": {"path": "dem.tif"}},
    ]
    t = time.time()
    g3 = mf.run(w, relief_nodes, {"along": "along.tracks", "steps": "steps.steps"}, name="relief.toml")
    print(f"relief model run in {time.time() - t:.1f} s")
    a = g3["along"]
    print(a.columns.tolist())
    a["case"] = np.where(a.case_.astype(bool), "observed", "random")
    a = a.dropna(subset=["elevation_along_ascent"])
    a["ascent100"] = a.elevation_along_ascent / a.sl_.clip(lower=1) * 100
    a["flat"] = a.elevation_along_ascent < 2
    ratio = a.elevation_along_delta / a.elevation_along_ascent.replace(0, np.nan)
    a["kind"] = np.select([a.elevation_along_ascent < 2, ratio > 0.8], ["flat or along a contour", "climb or descent"],
                          "up and down")
    print(a.groupby("case")[["sl_", "elevation_along_ascent", "elevation_along_descent", "elevation_along_delta",
                             "ascent100", "flat"]].mean().round(3))
    print(pd.crosstab(a.case, a.kind, normalize="index").round(3))
    RESULTS["relief_steps"] = int(a.case_.sum())
    RESULTS["relief_rows"] = len(a)
    rows = []
    for who, d in list(a.groupby(ID)) + [("all", a)]:
        o, r = d[d.case == "observed"], d[d.case == "random"]
        rows.append({"animal": who, "steps": len(o), "mean step (m)": round(o.sl_.mean(), 1),
                     "ascent, observed (m)": round(o.elevation_along_ascent.mean(), 2),
                     "ascent, random (m)": round(r.elevation_along_ascent.mean(), 2),
                     "per 100 m, observed": round(o.ascent100.mean(), 2),
                     "per 100 m, random": round(r.ascent100.mean(), 2),
                     "flat steps, observed (%)": round(o.flat.mean() * 100, 1),
                     "flat steps, random (%)": round(r.flat.mean() * 100, 1)})
        print(rows[-1])
    figs.write_csv(DATA / "relief.csv", rows)
    crows = []
    for kind in ["flat or along a contour", "climb or descent", "up and down"]:
        crows.append({"kind of step": kind,
                      "observed (%)": round((a[a.case == "observed"].kind == kind).mean() * 100, 1),
                      "random (%)": round((a[a.case == "random"].kind == kind).mean() * 100, 1)})
    figs.write_csv(DATA / "relief-classes.csv", crows)
    print(crows)

    # the library on the observed steps
    dem = tf.load_raster(w / "dem.tif")
    lt = tf.move.resample(p[p[ID] != "M5"], rate="10min", tolerance="1min")
    lt = tf.move.filter_min_n_burst(lt, min_n=3)
    lsteps = tf.move.random_steps(tf.move.steps(lt), n_control=5, seed=1)
    lsum = tf.move.along_summary(lsteps, {"elevation": dem}, stats=("ascent", "descent", "delta", "mean"))
    lsum = lsum[lsum.case_.astype(bool)]
    obs = a[a.case == "observed"]
    print("library vs operator, mean ascent of the observed steps:", float(lsum.elevation_along_ascent.mean()),
          float(obs.elevation_along_ascent.mean()), "steps", len(lsum), len(obs))
    RESULTS["relief_parity"] = [float(lsum.elevation_along_ascent.mean()), float(obs.elevation_along_ascent.mean())]

    # a stretch of M4's steps over the terrain
    ob = a[(a.case == "observed") & (a[ID] == "M4")]
    ob = ob.assign(t=pd.to_datetime(ob["t1_"]))
    day0, day1 = pd.Timestamp("2010-03-05 17:00", tz="UTC"), pd.Timestamp("2010-03-06 17:00", tz="UTC")
    if ob.t.dt.tz is None:
        day0, day1 = day0.tz_localize(None), day1.tz_localize(None)
    night = ob[(ob.t >= day0) & (ob.t < day1)]
    print("M4's night:", len(night), "steps; total ascent", float(night.elevation_along_ascent.sum()))
    de = [float(dem.x.min()), float(dem.x.max()), float(dem.y.min()), float(dem.y.max())]
    RESULTS["night_steps"] = len(night)
    relief_figures(a, dem, de, night, p)

    print("RESULTS", RESULTS)


def relief_figures(a, dem, de, night, p):
    """The two figures of the relief: a night of M4 over the terrain, and the ascent of observed and random steps."""
    from pyproj import Transformer

    crs = p.crs
    tr = Transformer.from_crs(crs, 4326, always_xy=True)
    fig, ax = plt.subplots(figsize=(6.6, 4.8), dpi=150)
    from scipy.ndimage import uniform_filter

    smooth = uniform_filter(np.asarray(dem.values, float), size=3)
    ax.imshow(smooth, extent=de, cmap="Greys", vmin=np.nanpercentile(smooth, 2), vmax=np.nanpercentile(smooth, 99) + 40,
              alpha=0.35)
    gx_ = np.linspace(de[0], de[1], smooth.shape[1])
    gy_ = np.linspace(de[3], de[2], smooth.shape[0])
    ax.contour(gx_, gy_, smooth, levels=np.arange(0, 400, 5), colors="#7a847d", linewidths=0.5, alpha=0.8)
    lo = []
    asc = []
    for r in night.itertuples():
        x1, y1 = tr.transform(r.x1_, r.y1_)
        x2, y2 = tr.transform(r.x2_, r.y2_)
        lo.append([[x1, y1], [x2, y2]])
        asc.append(r.elevation_along_ascent)
    lc = LineCollection(lo, array=np.clip(np.array(asc), 0, 8), cmap="viridis", linewidths=2.0)
    ax.add_collection(lc)
    xs = [q[0][0] for q in lo]
    ys = [q[0][1] for q in lo]
    ax.set_xlim(min(xs) - 0.004, max(xs) + 0.004)
    ax.set_ylim(min(ys) - 0.003, max(ys) + 0.003)
    mf.lonlat(ax)
    ax.locator_params(nbins=5)
    mf.scalebar(ax, 0.5)
    cb = fig.colorbar(lc, ax=ax, shrink=0.85)
    cb.set_label("Ascent along the step (m; 8 and more in yellow)", fontsize=9)
    fig.tight_layout()
    mf.save(fig, OUT / "relief-night.png")

    def comp(ax):
        bins = np.arange(0, 21, 1.0)
        for case, col in [("observed", figs.FOREST), ("random", figs.EMBER)]:
            d = a[a.case == case].ascent100.clip(upper=20)
            ax.hist(d, bins=bins, density=True, histtype="step", lw=1.8, color=col, label=case)
        ax.set_xlabel("Ascent per 100 m of step (m; 20 and more in the last class)")
        ax.set_ylabel("Density")
        ax.legend(frameon=False, fontsize=9)

    figs.chart(OUT / "relief-compare.png", comp, width=900, height=380)


if __name__ == "__main__":
    main()
