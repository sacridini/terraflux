"""Figures of "Home ranges and utilisation distributions" (website/content/tutorials/animal-movement/home-ranges.md).

The estimates are computed with the terraflux library (tf.move: amt's home ranges, move2utils' Brownian bridges) and
checked against the operators the page shows (move.home_range, move.dbbmm_ud, move.ud_volume...), run by the
terraflux binary on the same tracks: what the page tells about the two agreeing is printed here, not typed.

    PYTHONPATH=extern/terraflux-py/src TERRAFLUX_BIN=target/debug/terraflux.exe \\
        python website/figures/animal-movement/home-ranges.py
"""

import os
import time

import geopandas as gpd
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
import terraflux as tf
from matplotlib.colors import LinearSegmentedColormap

import movefig as mf
from movefig import figs

SLUG = "home-ranges"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")
ID = mf.ID
NT = min(8, int(os.environ.get("HEAVY_CORES", "4")))
LEVELS = [0.5, 0.95]
ERR = 20  # the location error taken for the fishers' collars, metres (their file has none)
CELL = 50  # the cell of the dBBMM, metres
GREENS = LinearSegmentedColormap.from_list("ud", ["#ffffff", "#cfe6d6", "#7fbf95", "#3b8a5a", "#1d4d33"])
EDGE = {0.5: figs.INK, 0.95: figs.EMBER}
RESULTS = {}


def km2(a):
    return float(a) / 1e6


def area_of(df, animal, level):
    """The area (km2) of an animal's polygon at a level in a hr_area table."""
    d = df[(df[df.columns[0]] == animal) & np.isclose(df.level.astype(float), level)]
    return km2(d.area.iloc[0])


def areas(polys, track=None):
    """{level: km2} of a table of polygons (the rows of one track)."""
    d = polys if track is None else polys[polys[polys.columns[0]] == track]
    return {round(float(r.level), 2): km2(r.area) for r in d.itertuples()}


def band_ud(ud, i=0):
    return ud.isel({ud.dims[0]: i}) if ud.ndim == 3 else ud


def draw_ud(ax, ud, pts=None, polys=None, power=0.5, vmax=None):
    """A UD as an image in metres, with the locations and the contours over it."""
    u = band_ud(ud)
    x, y = u.x.values, u.y.values
    ext = [x.min() - (x[1] - x[0]) / 2, x.max() + (x[1] - x[0]) / 2, y.min() - abs(y[1] - y[0]) / 2,
           y.max() + abs(y[1] - y[0]) / 2]
    v = np.asarray(u.values, float)
    v = np.where(v > 0, v, 0) ** power
    ax.imshow(v, extent=ext, origin="upper" if y[0] > y[-1] else "lower", cmap=GREENS, interpolation="nearest",
              vmin=0, vmax=vmax if vmax else np.nanmax(v))
    if pts is not None:
        ax.scatter(pts.geometry.x, pts.geometry.y, s=1.2, color=figs.INK2, alpha=0.5, lw=0, zorder=3)
    if polys is not None:
        for r in polys.itertuples():
            mf.outline(ax, r.geometry, color=EDGE[round(float(r.level), 2)], lw=1.5, zorder=5)


def contours(ud, levels=LEVELS):
    iso = tf.move.hr_isopleths(band_ud(ud), levels=levels)
    return iso


def prep():
    tr = mf.clean_tracks()
    m4 = tr[tr[ID] == "M4"].reset_index(drop=True)
    p4, crs4 = mf.projected(m4)
    s10 = mf.thin(p4, "10 mins")
    others = tr[tr[ID] != "M5"].reset_index(drop=True)
    po, _ = mf.projected(others)
    return tr, m4, p4, s10, po


def operators_check(w, s10):
    """The page's models run by the terraflux binary, against the library."""
    base = list(mf.CLEAN) + [
        {"id": "m4", "op": "vector.filter", "inputs": {"vector": "unique.tracks"},
         "params": {"expression": 'individual_local_identifier == "M4"'}},
        {"id": "aeqd", "op": "move.aeqd_crs", "inputs": {"tracks": "m4.vector"}},
        {"id": "thin", "op": "move.thin_time", "inputs": {"tracks": "aeqd.tracks"}, "params": {"interval": "10 mins"}},
    ]
    nodes = base + [
        {"id": "mcp", "op": "move.home_range", "inputs": {"tracks": "thin.tracks"},
         "params": {"method": "mcp", "levels": LEVELS}},
        {"id": "kde", "op": "move.home_range", "inputs": {"tracks": "thin.tracks"},
         "params": {"method": "kde", "levels": LEVELS, "bandwidth": "ref", "res": 25, "factor": 1.25}},
        {"id": "pi", "op": "move.home_range", "inputs": {"tracks": "thin.tracks"},
         "params": {"method": "kde", "levels": LEVELS, "bandwidth": "pi", "res": 25, "factor": 1.25}},
        {"id": "locoh", "op": "move.home_range", "inputs": {"tracks": "thin.tracks"},
         "params": {"method": "locoh", "levels": LEVELS, "type": "k", "n": 47}},
        {"id": "window", "op": "move.suggest_window", "inputs": {"tracks": "thin.tracks"}},
        {"id": "var", "op": "move.dbbmm_variance", "inputs": {"tracks": "thin.tracks"},
         "params": {"location_error": ERR, "window_size": 25, "margin": 7}},
        {"id": "ud", "op": "move.dbbmm_ud", "inputs": {"tracks": "var.tracks"}, "params": {"res": CELL}},
        {"id": "volume", "op": "move.ud_volume", "inputs": {"ud": "ud.ud"}, "params": {"levels": LEVELS}},
        {"id": "masked", "op": "move.mask_segments", "inputs": {"tracks": "var.tracks"}, "params": {"gap": 2}},
        {"id": "ud_gap", "op": "move.dbbmm_ud", "inputs": {"tracks": "masked.tracks"}, "params": {"res": CELL}},
        {"id": "volume_gap", "op": "move.ud_volume", "inputs": {"ud": "ud_gap.ud"}, "params": {"levels": LEVELS}},
        {"id": "outer", "op": "move.ud_outer_probability", "inputs": {"points": "thin.tracks", "ud": "ud.ud"}},
        {"id": "first", "op": "vector.filter", "inputs": {"vector": "thin.tracks"},
         "params": {"expression": 'timestamp < "2010-03-06"'}},
        {"id": "second", "op": "vector.filter", "inputs": {"vector": "thin.tracks"},
         "params": {"expression": 'timestamp >= "2010-03-06"'}},
        {"id": "ud_first", "op": "move.dbbmm_ud", "inputs": {"tracks": "first.vector"},
         "params": {"location_error": ERR, "window_size": 25, "margin": 7, "res": CELL, "ext_factor": 1.0}},
        {"id": "outer_second", "op": "move.ud_outer_probability", "inputs": {"points": "second.vector", "ud": "ud_first.ud"}},
        {"id": "use", "op": "move.intensity_use", "inputs": {"tracks": "thin.tracks"}},
    ]
    outs = {"mcp": "mcp.table", "kde": "kde.table", "pi": "pi.table", "locoh": "locoh.table",
            "window": "window.table", "volume": "volume.table", "volume_gap": "volume_gap.table",
            "outer": "outer.points", "outer_second": "outer_second.points", "use": "use.table", "thin": "thin.tracks"}
    t = time.time()
    got = mf.run(w, nodes, outs)
    print(f"operators run in {time.time() - t:.1f} s")
    return got


def main():
    w = mf.work(SLUG)
    tr, m4, p4, s10, po = prep()
    print("M4:", len(p4), "locations,", len(s10), "at 10 minutes")
    RESULTS["m4_n"] = len(p4)
    RESULTS["m4_n10"] = len(s10)
    lag = s10.timestamp.diff().dt.total_seconds() / 3600

    # ------------------------------------------------------------------ the three polygon and kernel estimators
    mcp = tf.move.hr_mcp(s10, levels=LEVELS)
    trast = tf.move.make_trast(s10, factor=1.25, res=25)
    kde_ref = tf.move.hr_kde(s10, trast=trast, levels=LEVELS)
    h_pi = tf.move.hr_kde_pi(s10)
    kde_pi = tf.move.hr_kde(s10, h=h_pi, trast=trast, levels=LEVELS)
    lscv = tf.move.hr_kde_lscv(s10, n_threads=NT)
    kde_lscv = tf.move.hr_kde(s10, h=lscv.h, trast=trast, levels=LEVELS)
    h_rs = tf.move.hr_kde_ref_scaled(s10, trast=trast, levels=0.95)
    kde_rs = tf.move.hr_kde(s10, h=(float(h_rs["h"].iloc[0]),) * 2, trast=trast, levels=LEVELS)
    k_locoh = int(round(np.sqrt(len(s10))))
    locoh = tf.move.hr_locoh(s10, n=k_locoh, levels=LEVELS)
    print("bandwidths: ref", tf.move.hr_kde_ref(s10).iloc[0].tolist(), "pi", h_pi.iloc[0].tolist(), "lscv",
          lscv.h.iloc[0].tolist(), "ref_scaled", h_rs.iloc[0].tolist(), "locoh k", k_locoh)

    # ------------------------------------------------------------------ the dynamic Brownian bridge
    sug = tf.move.suggest_dbbmm_window(s10)
    ws, mg = sug.window_size, sug.margin
    print("suggested", sug)
    v = tf.move.dbbmm_variance(s10, location_error=ERR, window_size=ws, margin=mg, n_threads=NT)
    t = time.time()
    ud = tf.move.dbbmm_ud(v, res=CELL, n_threads=NT)
    print(f"dBBMM UD {time.time() - t:.1f} s", ud.shape)
    iso_bb = contours(ud)

    # ------------------------------------------------------------------ the operators against the library
    got = operators_check(w, s10)
    chk = {}

    def level_areas(tab):
        d = tab[[c for c in tab.columns if c in ("level", "area")]]
        return {round(float(a), 2): float(b) for a, b in zip(d["level"], d["area"])}

    lib_areas = {"mcp": areas(tf.move.hr_area(mcp)), "kde": areas(tf.move.hr_area(kde_ref)),
                 "pi": areas(tf.move.hr_area(kde_pi)), "locoh": areas(tf.move.hr_area(locoh)),
                 "volume": areas(iso_bb)}
    worst = 0.0
    for k in ("mcp", "kde", "pi", "locoh", "volume"):
        a = level_areas(got[k])
        for lev, val in a.items():
            lv = lib_areas[k].get(lev)
            rel = abs(val / 1e6 - lv) / lv if lv else float("nan")
            print(f"  operator vs library {k} {lev}: {val / 1e6:.6f} km2 vs {lv:.6f} (rel {rel:.2e})")
            worst = max(worst, rel)
    print("operators == library, worst relative difference of an area:", f"{worst:.2e}")
    RESULTS["parity_worst"] = worst
    print("operator window:", got["window"].drop(columns=[c for c in got["window"].columns if c == "geometry"]).to_dict("records"))

    # ------------------------------------------------------------------ figure: the estimators
    fig, axs = plt.subplots(2, 2, figsize=(7.6, 6.2), dpi=150, sharex=True, sharey=True)
    pts = s10
    panels = [("Minimum convex polygon", tf.move.hr_isopleths(mcp), None),
              (f"Kernel density, reference bandwidth ({tf.move.hr_kde_ref(s10).hx.iloc[0]:.0f} m)",
               tf.move.hr_isopleths(kde_ref), tf.move.hr_ud(kde_ref)),
              (f"LoCoH, k = {k_locoh}", tf.move.hr_isopleths(locoh), None),
              (f"Dynamic Brownian bridge (window {ws}, margin {mg})", iso_bb, ud)]
    for ax, (title, iso, u) in zip(axs.flat, panels):
        if u is not None:
            draw_ud(ax, u, pts=None, polys=None, power=0.5)
        ax.plot(pts.geometry.x, pts.geometry.y, color=figs.INK2, lw=0.25, alpha=0.5, zorder=3)
        ax.scatter(pts.geometry.x, pts.geometry.y, s=1.0, color=figs.INK2, alpha=0.6, lw=0, zorder=3)
        for r in iso.itertuples():
            lev = round(float(r.level), 2)
            lev = 0.95 if lev > 0.9 else 0.5
            mf.outline(ax, r.geometry, color=EDGE[lev], lw=1.6, zorder=5)
        mf.kmaxes(ax, title)
        ax.title.set_fontsize(9)
    for ax in axs.flat:
        ax.set_xlim(-4500, 3200)
        ax.set_ylim(-3000, 3300)
    for ax in axs[:, 1]:
        ax.set_ylabel("")
    for ax in axs[0, :]:
        ax.set_xlabel("")
    axs[0, 0].plot([], [], color=EDGE[0.95], lw=1.6, label="95 %")
    axs[0, 0].plot([], [], color=EDGE[0.5], lw=1.6, label="50 %")
    axs[0, 0].legend(loc="upper left", fontsize=8.5, frameon=False)
    fig.tight_layout()
    mf.save(fig, OUT / "estimators.png")
    # the first panel's limits come from the polygons: print the extent the tracks have
    print("extent of M4 (m):", [round(v) for v in (s10.geometry.x.min(), s10.geometry.x.max(), s10.geometry.y.min(),
                                                   s10.geometry.y.max())])

    # ------------------------------------------------------------------ the areas of each estimator
    gaps = np.where((s10.timestamp.shift(-1) - s10.timestamp).dt.total_seconds().fillna(0) / 3600 > 2, True, False)
    v_gap = tf.move.mask_segments(v, gaps)
    ud_gap = tf.move.dbbmm_ud(v_gap, res=CELL, n_threads=NT)
    iso_gap = contours(ud_gap)
    ga = areas(iso_gap)
    RESULTS["gap2_segments"] = int(gaps.sum())
    op_gap = level_areas(got["volume_gap"])
    print("gap 2 h: library", ga, "operator", {k: x / 1e6 for k, x in op_gap.items()})
    RESULTS["gap_parity"] = max(abs(op_gap[k] / 1e6 - ga[k]) / ga[k] for k in ga)
    for_table = [
        ("Minimum convex polygon", areas(tf.move.hr_area(mcp)), ""),
        (f"Kernel density, reference bandwidth", areas(tf.move.hr_area(kde_ref)), f"{tf.move.hr_kde_ref(s10).hx.iloc[0]:.0f}"),
        ("Kernel density, reference scaled to one part", areas(tf.move.hr_area(kde_rs)), f"{h_rs.iloc[0].iloc[0]:.0f}"),
        ("Kernel density, plug-in bandwidth", areas(tf.move.hr_area(kde_pi)), f"{h_pi.hx.iloc[0]:.0f}"),
        ("Kernel density, cross-validated bandwidth", areas(tf.move.hr_area(kde_lscv)), f"{lscv.h.iloc[0].iloc[0]:.0f}"),
        (f"LoCoH, k = {k_locoh}", areas(tf.move.hr_area(locoh)), ""),
        ("Dynamic Brownian bridge", areas(iso_bb), ""),
        ("Dynamic Brownian bridge, gaps over 2 h left out", ga, ""),
    ]
    rows = []
    for name, a, h in for_table:
        lo = sorted(a)
        rows.append({"estimator": name, "bandwidth (m)": h, "core 50 % (km²)": round(a[lo[0]], 2),
                     "home range 95 % (km²)": round(a[lo[-1]], 2)})
    figs.write_csv(DATA / "estimators.csv", rows)
    for r in rows:
        print(r)

    # ------------------------------------------------------------------ the bandwidths
    def parts(geom):
        return len(geom.geoms) if hasattr(geom, "geoms") else 1

    bw = []
    fig, axs = plt.subplots(2, 2, figsize=(7.6, 6.2), dpi=150, sharex=True, sharey=True)
    for ax, (name, hr, h) in zip(axs.flat, [("reference", kde_ref, tf.move.hr_kde_ref(s10).hx.iloc[0]),
                                       ("reference scaled", kde_rs, h_rs.iloc[0].iloc[0]),
                                       ("plug-in", kde_pi, h_pi.hx.iloc[0]),
                                       ("cross-validation", kde_lscv, lscv.h.iloc[0].iloc[0])]):
        iso = tf.move.hr_isopleths(hr)
        draw_ud(ax, tf.move.hr_ud(hr), pts=s10)
        np95 = 0
        for r in iso.itertuples():
            lev = 0.95 if r.level > 0.9 else 0.5
            mf.outline(ax, r.geometry, color=EDGE[lev], lw=1.4, zorder=5)
            if lev == 0.95:
                np95 = parts(r.geometry)
        a = areas(iso)
        bw.append({"bandwidth": name, "h (m)": round(float(h)), "polygons at 95 %": np95,
                   "core 50 % (km²)": round(a[0.5], 2), "home range 95 % (km²)": round(a[0.95], 2)})
        mf.kmaxes(ax, f"{name}: {h:.0f} m")
        ax.title.set_fontsize(9)
        ax.set_xlim(-4500, 3200)
        ax.set_ylim(-3000, 3300)
    for ax in axs[:, 1]:
        ax.set_ylabel("")
    for ax in axs[0, :]:
        ax.set_xlabel("")
    fig.tight_layout()
    mf.save(fig, OUT / "bandwidths.png")
    figs.write_csv(DATA / "bandwidths.csv", bw)
    print(bw)
    print("lscv converged:", lscv.converged.to_dict() if hasattr(lscv.converged, "to_dict") else lscv.converged)

    # ------------------------------------------------------------------ the bandwidth slider: multiples of ref
    h0 = float(tf.move.hr_kde_ref(s10).hx.iloc[0])
    slides = []
    for i, mult in enumerate([0.25, 0.5, 1.0, 1.5, 2.0]):
        hr = tf.move.hr_kde(s10, h=(h0 * mult,) * 2, trast=trast, levels=LEVELS)
        iso = tf.move.hr_isopleths(hr)
        fig, ax = plt.subplots(figsize=(6.0, 4.4), dpi=150)
        draw_ud(ax, tf.move.hr_ud(hr), pts=s10)
        np95 = 0
        for r in iso.itertuples():
            lev = 0.95 if r.level > 0.9 else 0.5
            mf.outline(ax, r.geometry, color=EDGE[lev], lw=1.6, zorder=5)
            if lev == 0.95:
                np95 = parts(r.geometry)
        mf.kmaxes(ax)
        ax.set_xlim(-4500, 3200)
        ax.set_ylim(-3000, 3300)
        mf.kmscale(ax, 1)
        fig.tight_layout()
        mf.save(fig, OUT / f"bw_{i}.png")
        a = areas(iso)
        slides.append({"value": i, "multiple": mult, "h": round(h0 * mult), "polygons": np95, "core": round(a[0.5], 2),
                       "home": round(a[0.95], 2)})
    figs.write_csv(DATA / "bandwidth-slider.csv", slides)
    print(slides)

    # ------------------------------------------------------------------ the interval changes the estimate
    rows = []
    for label, interval in [("as recorded (2 minutes, with gaps)", None), ("10 minutes", "10 mins"),
                            ("30 minutes", "30 mins"), ("1 hour", "1 hour"), ("2 hours", "2 hours")]:
        s = p4 if interval is None else mf.thin(p4, interval)
        r = {"interval": label, "locations": len(s)}
        r["MCP 95 %"] = round(km2(tf.move.hr_area(tf.move.hr_mcp(s, levels=0.95)).area.iloc[0]), 2)
        kd = tf.move.hr_kde(s, trast=tf.move.make_trast(p4, factor=1.25, res=25), levels=0.95)
        r["KDE reference 95 %"] = round(km2(tf.move.hr_area(kd).area.iloc[0]), 2)
        r["KDE plug-in 95 %"] = round(km2(tf.move.hr_area(tf.move.hr_kde(
            s, h=tf.move.hr_kde_pi(s), trast=tf.move.make_trast(p4, factor=1.25, res=25), levels=0.95)).area.iloc[0]), 2)
        r["LoCoH k = √n 95 %"] = round(km2(tf.move.hr_area(tf.move.hr_locoh(
            s, n=int(round(np.sqrt(len(s)))), levels=0.95)).area.iloc[0]), 2)
        if interval is not None:
            sg = tf.move.suggest_dbbmm_window(s)
            vv = tf.move.dbbmm_variance(s, location_error=ERR, window_size=sg.window_size, margin=sg.margin,
                                        n_threads=NT)
            uu = tf.move.dbbmm_ud(vv, res=CELL, n_threads=NT)
            r["dBBMM 95 %"] = round(areas(contours(uu))[0.95], 2)
        else:
            r["dBBMM 95 %"] = ""
        rows.append(r)
        print(r)
    figs.write_csv(DATA / "interval.csv", rows)

    # ------------------------------------------------------------------ the variance of the bridge
    t0 = s10.timestamp.iloc[0]
    days = (s10.timestamp - t0).dt.total_seconds() / 86400
    fig, axs = plt.subplots(1, 2, figsize=(9.0, 3.5), dpi=150, gridspec_kw={"width_ratios": [1.35, 1]})
    ax = axs[0]
    mv = v.motion_variance.values
    ax.scatter(days, mv, s=3, color=figs.FOREST, alpha=0.7, lw=0)
    ax.set_yscale("log")
    ax.set_ylim(max(np.nanmin(mv[mv > 0]), 1), np.nanmax(mv) * 1.5)
    ax.set_xlabel("Days since the first location")
    ax.set_ylabel("Motion variance (m² per minute, log scale)")
    ax.set_title("The variance along the track", fontsize=10.5, loc="left")
    ax = axs[1]
    sc = ax.scatter(v.geometry.x, v.geometry.y, c=np.log10(np.clip(mv, 1, None)), s=4, cmap="viridis", lw=0)
    mf.kmaxes(ax, "Where it is high")
    cb = fig.colorbar(sc, ax=ax, shrink=0.8)
    cb.set_label("log10 of the variance", fontsize=9)
    fig.tight_layout()
    mf.save(fig, OUT / "variance.png")
    RESULTS["variance_median"] = float(np.nanmedian(mv))
    RESULTS["variance_p5_p95"] = [float(np.nanpercentile(mv, 5)), float(np.nanpercentile(mv, 95))]
    RESULTS["interest_share"] = float(v.interest.mean())
    print("variance", RESULTS["variance_median"], RESULTS["variance_p5_p95"], "interest", RESULTS["interest_share"])

    # ------------------------------------------------------------------ the dBBMM and its contours
    fig, ax = plt.subplots(figsize=(6.4, 4.8), dpi=150)
    draw_ud(ax, ud, pts=s10, polys=iso_bb)
    mf.kmaxes(ax)
    ax.set_xlim(-4500, 3200)
    ax.set_ylim(-3000, 3300)
    mf.kmscale(ax, 1)
    fig.tight_layout()
    mf.save(fig, OUT / "dbbmm.png")

    # ------------------------------------------------------------------ the gaps: a slider
    lags_next = (s10.timestamp.shift(-1) - s10.timestamp).dt.total_seconds().fillna(0) / 3600
    frames = []
    for i, (label, thr) in enumerate([("none", None), ("12 hours", 12), ("4 hours", 4), ("2 hours", 2), ("1 hour", 1)]):
        if thr is None:
            uu, vv = ud, v
            nmask = 0
        else:
            m = (lags_next > thr).to_numpy()
            nmask = int(m.sum())
            uu = tf.move.dbbmm_ud(tf.move.mask_segments(v, m), res=CELL, n_threads=NT)
        iso = contours(uu)
        a = areas(iso)
        fig, ax = plt.subplots(figsize=(6.0, 4.4), dpi=150)
        draw_ud(ax, uu, pts=s10, polys=iso)
        mf.kmaxes(ax)
        ax.set_xlim(-4500, 3200)
        ax.set_ylim(-3000, 3300)
        mf.kmscale(ax, 1)
        fig.tight_layout()
        mf.save(fig, OUT / f"gap_{i}.png")
        frames.append({"value": i, "left out": label, "segments": nmask, "core": round(a[0.5], 2),
                       "home": round(a[0.95], 2)})
    figs.write_csv(DATA / "gap-slider.csv", frames)
    print(frames)
    RESULTS["gaps_over_1h"] = int((lags_next > 1).sum())

    # ------------------------------------------------------------------ the location error
    rows = []
    for e in [0, 5, 10, 20, 50, 100]:
        vv = tf.move.dbbmm_variance(s10, location_error=e, window_size=ws, margin=mg, n_threads=NT)
        uu = tf.move.dbbmm_ud(vv, res=CELL, n_threads=NT)
        a = areas(contours(uu))
        rows.append({"location error (m)": e, "median variance (m² per minute)": round(float(np.nanmedian(
            vv.motion_variance)), 1), "core 50 % (km²)": round(a[0.5], 3), "home range 95 % (km²)": round(a[0.95], 3)})
        print(rows[-1])
    figs.write_csv(DATA / "location-error.csv", rows)

    # ------------------------------------------------------------------ the window
    rows = []
    for ws_, mg_ in [(11, 3), (25, 7), (51, 15), (101, 25), (201, 49)]:
        vv = tf.move.dbbmm_variance(s10, location_error=ERR, window_size=ws_, margin=mg_, n_threads=NT)
        uu = tf.move.dbbmm_ud(vv, res=CELL, n_threads=NT)
        a = areas(contours(uu))
        rows.append({"window": ws_, "margin": mg_, "hours": round(ws_ * 10 / 60, 1),
                     "locations with a variance": int(vv.motion_variance.notna().sum()),
                     "core 50 % (km²)": round(a[0.5], 3), "home range 95 % (km²)": round(a[0.95], 3)})
        print(rows[-1])
    figs.write_csv(DATA / "window.csv", rows)

    # ------------------------------------------------------------------ the dBGB
    ws2, mg2 = 31, 15
    vb = tf.move.dbbmm_variance(s10, location_error=ERR, window_size=ws2, margin=mg2, n_threads=NT)
    ub = tf.move.dbbmm_ud(vb, res=CELL, n_threads=NT)
    vg = tf.move.dbgb_variance(s10, location_error=ERR, window_size=ws2, margin=mg2, n_threads=NT)
    t = time.time()
    ug = tf.move.dbgb_ud(vg, res=CELL, n_threads=NT)
    print(f"dBGB UD of M4 {time.time() - t:.1f} s")
    ib, ig = contours(ub), contours(ug)

    # a simulated animal that patrols a line (a river bank, a fence), where the direction matters
    pat = patrol()
    patrol_pts = gpd.GeoDataFrame(pat, geometry=gpd.points_from_xy(pat.x, pat.y), crs=32618)
    patrol_pts["timestamp"] = pd.Timestamp("2026-01-01", tz="UTC") + pd.to_timedelta(pat.time, unit="s")
    patrol_pts["individual-local-identifier"] = "patrol"
    pat.to_csv(DATA / "patrol.csv", index=False)
    pv = tf.move.dbbmm_variance(patrol_pts, location_error=3, window_size=31, margin=15, n_threads=NT)
    pu = tf.move.dbbmm_ud(pv, res=10, n_threads=NT)
    pg = tf.move.dbgb_variance(patrol_pts, location_error=3, window_size=31, margin=15, n_threads=NT)
    pgu = tf.move.dbgb_ud(pg, res=10, n_threads=NT)
    ipb, ipg = contours(pu), contours(pgu)
    print("patrol sd along/across:", float(pg.para_sd.median()), float(pg.orth_sd.median()))
    cmp_rows = []
    for name, ibb, igb in [("M4, the fisher", ib, ig), ("the simulated patrol", ipb, ipg)]:
        a1, a2 = areas(ibb), areas(igb)
        cmp_rows.append({"track": name, "dBBMM core (km²)": round(a1[0.5], 3), "dBGB core (km²)": round(a2[0.5], 3),
                         "dBBMM 95 % (km²)": round(a1[0.95], 3), "dBGB 95 % (km²)": round(a2[0.95], 3),
                         "dBGB / dBBMM at 95 %": round(a2[0.95] / a1[0.95], 2)})
        print(cmp_rows[-1])
    figs.write_csv(DATA / "dbgb.csv", cmp_rows)
    RESULTS["dbgb_sd"] = {"M4": [float(vg.para_sd.median()), float(vg.orth_sd.median())],
                          "patrol": [float(pg.para_sd.median()), float(pg.orth_sd.median())]}
    fig, axs = plt.subplots(2, 2, figsize=(8.4, 6.2), dpi=150, gridspec_kw={"height_ratios": [1.15, 0.85]})
    for ax, u, iso, ptsx, title, xl, yl in [
            (axs[0, 0], ub, ib, s10, "M4: dBBMM", (-4500, 3200), (-3000, 3300)),
            (axs[0, 1], ug, ig, s10, "M4: dBGB", (-4500, 3200), (-3000, 3300))]:
        draw_ud(ax, u, pts=ptsx, polys=iso)
        mf.kmaxes(ax, title)
        ax.set_xlim(*xl)
        ax.set_ylim(*yl)
    for ax, u, iso, title in [(axs[1, 0], pu, ipb, "A stretch of the patrol: dBBMM"), (axs[1, 1], pgu, ipg, "A stretch of the patrol: dBGB")]:
        draw_ud(ax, u, pts=patrol_pts, polys=iso, power=0.5)
        mf.kmaxes(ax, title)
        ax.set_xlim(1700, 2300)
        ax.set_ylim(-300, 300)
    for ax in axs.flat:
        ax.title.set_fontsize(9.5)
    fig.tight_layout()
    mf.save(fig, OUT / "dbgb.png")

    # ------------------------------------------------------------------ the outer probability of the locations
    half = pd.Timestamp("2010-03-06", tz="UTC")
    first, second = s10[s10.timestamp < half].reset_index(drop=True), s10[s10.timestamp >= half].reset_index(drop=True)
    ud1 = tf.move.dbbmm_ud(first, location_error=ERR, window_size=ws, margin=mg, res=CELL, ext_factor=1.0, n_threads=NT)
    iso1 = contours(ud1)
    op = tf.move.ud_outer_probability(second, ud1)
    sec = second.assign(outer=op)
    inside = float(np.nanmean(op <= 0.95))
    print("second half: locations", len(sec), "outside the grid", int(np.isnan(op).sum()), "inside the 95 %",
          round(inside * 100, 1), "% ; over 0.99:", int((op > 0.99).sum()), "; median", float(np.nanmedian(op)))
    RESULTS["outer"] = {"first": len(first), "second": len(second), "inside95": inside,
                        "over99": int((op > 0.99).sum()), "nan": int(np.isnan(op).sum())}
    own = tf.move.ud_outer_probability(first, ud1)
    print("the first half's own locations: over 0.99:", int((own > 0.99).sum()), "median", float(np.nanmedian(own)))
    RESULTS["outer"]["own_over99"] = int((own > 0.99).sum())
    RESULTS["outer"]["own_median"] = float(np.nanmedian(own))
    RESULTS["outer"]["half"] = str(half)
    fig, ax = plt.subplots(figsize=(6.6, 4.8), dpi=150)
    draw_ud(ax, ud1, polys=iso1)
    sc = ax.scatter(sec.geometry.x, sec.geometry.y, c=sec.outer, s=6, cmap="viridis_r", vmin=0, vmax=1, lw=0, zorder=4)
    out = sec[sec.outer > 0.95]
    ax.scatter(out.geometry.x, out.geometry.y, s=26, facecolor="none", edgecolor=figs.EMBER, lw=0.9, zorder=6)
    mf.kmaxes(ax)
    ax.set_xlim(-4500, 3200)
    ax.set_ylim(-3000, 3300)
    cb = fig.colorbar(sc, ax=ax, shrink=0.8)
    cb.set_label("Outer probability of the later locations", fontsize=9)
    fig.tight_layout()
    mf.save(fig, OUT / "outer.png")
    o2 = got["outer_second"]["outer_probability"].values
    print("operator outer probability of the second half vs library:", len(o2), len(op),
          float(np.nanmax(np.abs(o2 - op))) if len(o2) == len(op) else "different lengths")
    op_all = tf.move.ud_outer_probability(s10, ud)
    print("operator outer probability vs library, largest difference:",
          float(np.nanmax(np.abs(got["outer"]["outer_probability"].values - op_all))))

    # ------------------------------------------------------------------ several animals at one interval
    p30 = mf.thin(po, "30 mins")
    names = sorted(p30[ID].unique())
    n30 = p30[ID].value_counts().to_dict()
    mcp30 = tf.move.hr_mcp(p30, levels=0.95)
    tr30 = tf.move.make_trast(p30)
    kref = tf.move.hr_kde(p30, trast=tr30, levels=[0.5, 0.95])
    hpi = tf.move.hr_kde_pi(p30)
    kpi = tf.move.hr_kde(p30, h=hpi, trast=tr30, levels=[0.5, 0.95])
    lsc = tf.move.hr_kde_lscv(p30, n_threads=NT)
    klsc = tf.move.hr_kde(p30, h=lsc.h, trast=tr30, levels=[0.5, 0.95])
    print("trast cells:", tr30.shape, "res", float(abs(tr30.x[1] - tr30.x[0])))
    lo30 = {a: tf.move.hr_area(tf.move.hr_locoh(p30[p30[ID] == a], n=int(round(np.sqrt(n30[a]))), levels=0.95)).area.iloc[0]
            for a in names}
    iu = tf.move.intensity_use(p30)
    sug30 = tf.move.suggest_dbbmm_window(p30)
    win_rows, rows = [], []
    for a in names:
        s = p30[p30[ID] == a].reset_index(drop=True)
        sg = sug30[a]
        win_rows.append({"animal": a, "locations": len(s), "median lag (min)": round(sg.median_dt_secs / 60, 1),
                         "window": sg.window_size, "margin": sg.margin, "window (hours)": round(sg.window_hours, 1),
                         "windows that fit": sg.n_windows})
        vv = tf.move.dbbmm_variance(s, location_error=ERR, window_size=sg.window_size, margin=sg.margin, n_threads=NT)
        uu = tf.move.dbbmm_ud(vv, res=100, n_threads=NT)
        bb95 = areas(contours(uu))[0.95]
        ka, kp, mc = tf.move.hr_area(kref), tf.move.hr_area(kpi), tf.move.hr_area(mcp30)
        rows.append({
            "animal": a, "locations": len(s), "MCP 95 %": round(area_of(mc, a, 0.95), 2),
            "KDE reference": round(area_of(ka, a, 0.95), 2), "KDE plug-in": round(area_of(kp, a, 0.95), 2),
            "LoCoH": round(km2(lo30[a]), 2), "dBBMM": round(bb95, 2), "intensity of use": round(float(iu[a]), 1)})
        print(rows[-1])
    figs.write_csv(DATA / "animals.csv", rows)
    figs.write_csv(DATA / "windows.csv", win_rows)

    # the overlap of the pairs that share ground
    allx = tf.move.hr_overlap(kref, type="vi", which="all")
    first = allx.columns[0]
    print("vi pairs above 0.05:", allx[allx.overlap > 0.05].to_dict("records"))
    pairs = [("F2", "M1"), ("F3", "M3"), ("F1", "M2")]
    orows = []
    for a, b in pairs:
        r = {"pair": f"{a} and {b}"}
        for typ in ("hr", "phr", "vi", "ba", "udoi", "hd"):
            o = tf.move.hr_overlap(kref, type=typ, which="all")
            sel = o[(o["from"] == a) & (o["to"] == b)]
            if typ in ("hr",):
                sel = sel[sel.level == 0.95]
            r[typ] = round(float(sel.overlap.iloc[0]), 3)
        orows.append(r)
        print(r)
    figs.write_csv(DATA / "overlap.csv", orows)
    brow = []
    for name, hr in [("reference", kref), ("plug-in", kpi), ("cross-validation", klsc)]:
        r = {"bandwidth": name}
        for typ in ("vi", "udoi"):
            o = tf.move.hr_overlap(hr, type=typ, which="all")
            r[typ] = round(float(o[(o["from"] == "F2") & (o["to"] == "M1")].overlap.iloc[0]), 3)
        oc = tf.move.hr_overlap(hr, type="vi", conditional=True, which="all")
        oc = oc[(oc["from"] == "F2") & (oc["to"] == "M1")]
        for lev in (0.5, 0.95):
            r[f"vi inside the {int(lev * 100)} % contours"] = round(float(oc[oc.level == lev].overlap.iloc[0]), 3)
        brow.append(r)
        print(r)
    figs.write_csv(DATA / "overlap-bandwidth.csv", brow)

    # the two animals with the most overlap
    fig, axs = plt.subplots(1, 2, figsize=(9.0, 3.6), dpi=150)
    for ax, (a, b) in zip(axs, [("F2", "M1"), ("F3", "M3")]):
        for who, col, k in [(a, mf.COLORS[a], kref), (b, mf.COLORS[b], kref)]:
            s = p30[p30[ID] == who]
            ax.scatter(s.geometry.x, s.geometry.y, s=2.5, color=col, alpha=0.5, lw=0)
            iso = tf.move.hr_isopleths(k)
            iso = iso[iso[iso.columns[0]] == who]
            for r in iso.itertuples():
                mf.outline(ax, r.geometry, color=col, lw=2.0 if r.level > 0.9 else 1.0, zorder=5)
        ax.plot([], [], color=mf.COLORS[a], lw=2, label=a)
        ax.plot([], [], color=mf.COLORS[b], lw=2, label=b)
        ax.legend(loc="best", fontsize=9, frameon=False)
        mf.kmaxes(ax, f"{a} and {b}")
        ax.title.set_fontsize(10)
    fig.tight_layout()
    mf.save(fig, OUT / "overlap.png")

    print("RESULTS", RESULTS)


def patrol():
    """A simulated animal that patrols a curved line, 1500 locations ten minutes apart: its velocity along the line
    persists (an autoregressive walk, reflected at the ends), its distance across it wanders a few metres."""
    rng = np.random.default_rng(7)
    n, dt, length = 1500, 600.0, 5000.0
    v = np.zeros(n)
    s = np.zeros(n)
    c = np.zeros(n)
    s[0], v[0] = 1000.0, 0.1
    for i in range(1, n):
        v[i] = 0.95 * v[i - 1] + 0.06 * rng.normal()
        s[i] = s[i - 1] + v[i] * dt
        if s[i] < 0:
            s[i], v[i] = -s[i], abs(v[i])
        if s[i] > length:
            s[i], v[i] = 2 * length - s[i], -abs(v[i])
        c[i] = 0.9 * c[i - 1] + 3 * rng.normal()
    th = 2 * np.pi * s / 4000
    dy = 600 * 2 * np.pi / 4000 * np.cos(th)
    nrm = np.hypot(1, dy)
    x = s + c * (-dy / nrm) + rng.normal(0, 3, n)
    y = 600 * np.sin(th) + c * (1 / nrm) + rng.normal(0, 3, n)
    return pd.DataFrame({"time": np.arange(n) * dt, "x": np.round(x, 2), "y": np.round(y, 2), "track": "patrol"})


if __name__ == "__main__":
    main()
