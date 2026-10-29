"""Figures of "Projections, interpolation and simulated tracks"
(website/content/tutorials/animal-movement/projections-and-interpolation.md).

    python website/figures/animal-movement/projections-and-interpolation.py
"""

import numpy as np
import matplotlib.pyplot as plt
import pandas as pd

import movefig as mf
from movefig import figs

SLUG = "projections-and-interpolation"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")

# the gap of M4 that the interpolation fills: from 19:00 to 22:00 UTC on 5 March 2010
W0, W1 = pd.Timestamp("2010-03-05 22:30"), pd.Timestamp("2010-03-06 03:00")


def main():
    w = mf.work(SLUG)
    nodes = list(mf.CLEAN) + [
        # a local projection, and the measures in it and on the ellipsoid
        {"id": "aeqd", "op": "move.aeqd_crs", "inputs": {"tracks": "unique.tracks"}},
        {"id": "dist_m", "op": "move.distance", "inputs": {"tracks": "aeqd.tracks"}},
        {"id": "dist_ll", "op": "move.distance", "inputs": {"tracks": "unique.tracks"}},
        {"id": "az_ll", "op": "move.azimuth", "inputs": {"tracks": "dist_ll.tracks"}, "params": {"units": "deg"}},
        # M4 at a regular 10 minutes, the gaps over an hour left empty
        {"id": "m4", "op": "vector.filter", "inputs": {"vector": "unique.tracks"},
         "params": {"expression": 'individual_local_identifier == "M4"'}},
        {"id": "every10", "op": "move.interpolate", "inputs": {"tracks": "m4.vector"},
         "params": {"times": "10 mins", "max_time_lag": 3600, "omit": True}},
        {"id": "every10_all", "op": "move.interpolate", "inputs": {"tracks": "m4.vector"},
         "params": {"times": "10 mins", "omit": True}},
        # Brownian tracks: four of 1000 steps, and one fitted to the fishers' steps of 10 minutes
        {"id": "brown", "op": "move.simulate_brownian",
         "params": {"t": list(range(0, 1001)), "sigma": 1, "tracks": 4, "seed": 1}},
        {"id": "brown_turn", "op": "move.turn_angle", "inputs": {"tracks": "brown.tracks"}, "params": {"units": "deg"}},
        {"id": "thin", "op": "move.filter_per_interval", "inputs": {"tracks": "unique.tracks"},
         "params": {"unit": "10 mins"}},
        {"id": "thin_turn", "op": "move.turn_angle", "inputs": {"tracks": "thin.tracks"}, "params": {"units": "deg"}},
    ]
    outs = {"aeqd": "aeqd.tracks", "crs": "aeqd.crs", "dm": "dist_m.tracks", "dll": "az_ll.tracks",
            "m4": "m4.vector", "every10": "every10.tracks", "every10_all": "every10_all.tracks",
            "brown": "brown_turn.tracks", "thin": "thin_turn.tracks"}
    try:
        got = mf.run(w, nodes, outs)
    except SystemExit as e:
        raise SystemExit(str(e)[:3000])
    crs = (w / "output" / "crs.json").read_text(encoding="utf-8") if (w / "output" / "crs.json").exists() else None
    print("crs:", crs)
    aeqd, dm, dll = got["aeqd"], got["dm"], got["dll"]
    print(aeqd.crs)

    # the tracks in metres from the centre
    fig, ax = plt.subplots(figsize=(7.4, 4.0), dpi=150)
    for a in mf.ANIMALS:
        s = aeqd[aeqd[mf.ID] == a]
        ax.plot(s.geometry.x / 1000, s.geometry.y / 1000, lw=0.5, color=mf.COLORS[a], label=a)
    ax.scatter([0], [0], marker="+", s=80, color=figs.INK, zorder=4)
    ax.text(0.4, 0.4, "centre", fontsize=9, color=figs.INK)
    ax.set_aspect(1)
    ax.grid(True)
    ax.set_xlabel("km east of the centre")
    ax.set_ylabel("km north of the centre")
    ax.legend(ncol=8, loc="upper center", bbox_to_anchor=(0.5, 1.14), frameon=False, fontsize=8.5, handlelength=1.2,
              columnspacing=1.0)
    fig.tight_layout()
    mf.save(fig, OUT / "aeqd.png")

    # the distance in the projection against the one on the sphere, and the one of flat degrees
    d_sph = dll["distance"].values
    d_proj = dm["distance"].values
    az = np.radians(dll["azimuth"].values)
    x, y = dll.geometry.x.values, dll.geometry.y.values
    same = np.r_[dll[mf.ID].values[1:] == dll[mf.ID].values[:-1], False]
    dx = np.r_[np.diff(x), np.nan]
    dy = np.r_[np.diff(y), np.nan]
    d_flat = np.hypot(dx, dy) * 111_320
    ok = same & np.isfinite(d_sph) & (d_sph > 50)
    rel_proj = (d_proj[ok] - d_sph[ok]) / d_sph[ok] * 100
    rel_flat = (d_flat[ok] - d_sph[ok]) / d_sph[ok] * 100
    print("projection vs sphere: median %.4f%%, max %.4f%%" % (np.median(np.abs(rel_proj)), np.abs(rel_proj).max()))
    print("flat degrees vs sphere: median %.1f%%, range %.1f%% to %.1f%%" % (np.median(rel_flat), rel_flat.min(),
                                                                         rel_flat.max()))

    def flat(ax):
        deg = np.degrees(az[ok]) % 360
        ax.scatter(deg, rel_flat, s=2, alpha=0.3, color=figs.EMBER, rasterized=True,
                   label="degrees taken as flat (× 111.32 km)")
        ax.scatter(deg, rel_proj, s=2, alpha=0.3, color=figs.FOREST, rasterized=True,
                   label="the azimuthal equidistant projection")
        ax.set_xticks([0, 90, 180, 270, 360], ["N", "E", "S", "W", "N"])
        ax.set_xlabel("Direction of the step")
        ax.set_ylabel("Error of the distance (%)")
        ax.legend(frameon=False, fontsize=9, markerscale=5, loc="upper center", bbox_to_anchor=(0.5, 1.16), ncol=2)

    figs.chart(OUT / "flat-degrees.png", flat, width=900, height=420)

    # M4's gap filled every 10 minutes
    m4, e10, e10a = got["m4"], got["every10"], got["every10_all"]
    for d in (m4, e10, e10a):
        d["t"] = pd.to_datetime(d["timestamp"], format="ISO8601")
    raw = m4[(m4.t >= W0) & (m4.t < W1)]
    reg = e10[(e10.t >= W0) & (e10.t < W1)]
    rega = e10a[(e10a.t >= W0) & (e10a.t < W1)]
    print(len(raw), "recorded,", len(reg), "at 10 minutes,", int((reg.geometry.isna() | reg.geometry.is_empty).sum()), "left empty")
    fig, axs = plt.subplots(1, 2, figsize=(9.6, 4.2), dpi=150, sharex=True, sharey=True)
    for ax, d, title in ((axs[0], rega, "every 10 minutes"), (axs[1], reg, "and gaps over an hour left empty")):
        ax.plot(raw.geometry.x, raw.geometry.y, color="#c9cfc9", lw=0.9)
        ax.scatter(raw.geometry.x, raw.geometry.y, s=5, color="#9aa59f", zorder=2)
        empty = d.geometry.isna() | d.geometry.is_empty
        p = d[~empty]
        # the line broken where a time has no location
        xs = [np.nan if e else g.x for e, g in zip(empty, d.geometry)]
        ys = [np.nan if e else g.y for e, g in zip(empty, d.geometry)]
        ax.plot(xs, ys, color=figs.INK, lw=1.2)
        ax.scatter(p.geometry.x, p.geometry.y, s=18, color=figs.EMBER, edgecolor="white", lw=0.4, zorder=3)
        mf.lonlat(ax)
        ax.locator_params(nbins=4)
        ax.set_title(title, loc="left", fontsize=10.5)
    mf.scalebar(axs[0], 0.5)
    fig.tight_layout()
    mf.save(fig, OUT / "interpolate.png")
    lag = raw.t.diff().dt.total_seconds().div(60)
    print("largest recorded lag in the window (min):", lag.max(), "at", raw.t[lag.idxmax()])

    # Brownian tracks
    b = got["brown"]
    fig, ax = plt.subplots(figsize=(7.4, 4.6), dpi=150)
    for k, (tr, s) in enumerate(b.groupby("track", sort=False)):
        ax.plot(s.geometry.x, s.geometry.y, lw=0.7, color=figs.CLASSES[k], label=f"track {int(tr)}")
        ax.scatter([s.geometry.x.iloc[-1]], [s.geometry.y.iloc[-1]], s=20, color=figs.CLASSES[k], zorder=3)
    ax.scatter([0], [0], marker="+", s=90, color=figs.INK, zorder=4)
    ax.set_aspect(1)
    ax.set_xlabel("x")
    ax.set_ylabel("y")
    ax.legend(frameon=False, fontsize=9)
    fig.tight_layout()
    mf.save(fig, OUT / "brownian.png")

    # turn angles: Brownian against the fishers at 10 minutes
    fig, axs = plt.subplots(1, 2, figsize=(6.6, 3.4), dpi=150, subplot_kw={"projection": "polar"})
    bins = np.radians(np.arange(-180, 181, 15))
    th = got["thin"]
    for ax, v, title in ((axs[0], b.turn_angle, "Brownian motion"),
                         (axs[1], th[th[mf.ID].isin(["F2", "F3", "M3", "M4", "M5"])].turn_angle,
                          "fishers, every 10 minutes")):
        v = np.radians(v.dropna().values)
        h, _ = np.histogram(v, bins=bins)
        ax.bar(bins[:-1] + np.diff(bins) / 2, h / h.sum(), width=np.diff(bins), color=figs.WATER, alpha=0.8,
               edgecolor="white", lw=0.5)
        ax.set_theta_zero_location("N")
        ax.set_theta_direction(-1)
        ax.set_yticklabels([])
        ax.set_ylim(0, 0.12)
        ax.set_xticks(np.radians([0, 90, 180, 270]), ["0°", "90°", "180°", "-90°"], fontsize=8.5)
        ax.set_title(title, fontsize=10.5, pad=12)
    fig.tight_layout()
    mf.save(fig, OUT / "brownian-turns.png")


if __name__ == "__main__":
    main()
