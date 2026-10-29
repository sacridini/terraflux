"""Figures of "Animal movement: the ideas" (website/content/tutorials/animal-movement/ideas.md).

    python website/figures/animal-movement/ideas.py
"""

import numpy as np
import matplotlib.pyplot as plt
import pandas as pd
from matplotlib.lines import Line2D

import movefig as mf
from movefig import figs

SLUG = "ideas"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")


def anatomy():
    """A toy track: locations, steps, a step length, a turn angle, a time lag."""
    p = np.array([[0, 0], [1.6, 0.5], [2.4, 1.9], [3.9, 2.2], [4.3, 3.6], [5.8, 3.9]])
    fig, ax = plt.subplots(figsize=(7.2, 3.6), dpi=150)
    ax.plot(p[:, 0], p[:, 1], color=figs.INK2, lw=1.4, zorder=1)
    ax.scatter(p[:, 0], p[:, 1], s=46, color=figs.EMBER, edgecolor="white", zorder=3)
    for i, (x, y) in enumerate(p):
        ax.text(x + 0.08, y - 0.32, f"$t_{i}$", fontsize=10, color=figs.INK)
    # a step's length
    a, b = p[1], p[2]
    n = np.array([-(b - a)[1], (b - a)[0]])
    n = n / np.linalg.norm(n) * 0.28
    ax.annotate("", xy=b + n, xytext=a + n, arrowprops=dict(arrowstyle="<->", color=figs.FOREST, lw=1.3))
    ax.text(*(a + b) / 2 + n * 2.2, "step length", color=figs.FOREST, ha="center", va="center", fontsize=9.5,
            rotation=np.degrees(np.arctan2(*(b - a)[::-1])))
    # the turn angle at p[2]: the heading kept (dashed) against the next step
    h = (p[2] - p[1]) / np.linalg.norm(p[2] - p[1])
    ax.plot([p[2, 0], p[2, 0] + 1.3 * h[0]], [p[2, 1], p[2, 1] + 1.3 * h[1]], ls="--", color=figs.INK2, lw=1)
    a0 = np.degrees(np.arctan2(h[1], h[0]))
    d = p[3] - p[2]
    a1 = np.degrees(np.arctan2(d[1], d[0]))
    from matplotlib.patches import Arc

    ax.add_patch(Arc(p[2], 1.3, 1.3, theta1=min(a0, a1), theta2=max(a0, a1), color=figs.WATER, lw=1.4))
    ax.text(p[2, 0] + 0.85, p[2, 1] + 0.42, "turn angle", color=figs.WATER, fontsize=9.5)
    # the time lag between two fixes
    ax.annotate("time lag\n$t_4 - t_3$", xy=(4.1, 2.9), xytext=(2.6, 3.5), fontsize=9.5, color=figs.INK2,
                ha="center", arrowprops=dict(arrowstyle="->", color=figs.INK2, lw=1))
    ax.set_xlim(-0.4, 6.4)
    ax.set_ylim(-0.7, 4.4)
    ax.set_aspect(1)
    ax.axis("off")
    fig.tight_layout()
    mf.save(fig, OUT / "anatomy.png")


def main():
    mf.publish_data()
    anatomy()
    w = mf.work(SLUG)
    nodes = [
        mf.CLEAN[0],
        {"id": "located", "op": "vector.filter", "inputs": {"vector": "read.tracks"},
         "params": {"expression": "not isnull(geometry.x)"}},
        {"id": "lines", "op": "move.track_lines", "inputs": {"tracks": "located.vector"}},
        {"id": "animals", "op": "move.track_data", "inputs": {"tracks": "read.tracks"}},
        mf.LANDSCAPE[1],
        {"id": "wc", "op": "raster.write", "inputs": {"value": "cover.raster"}, "params": {"path": "cover.tif"}},
    ]
    got = mf.run(w, nodes, {"records": "read.tracks", "points": "located.vector", "lines": "lines.lines",
                            "animals": "animals.tracks"})
    rec, pts, lines, animals = got["records"], got["points"], got["lines"], got["animals"]
    print(len(rec), "records,", len(pts), "with a location")
    print(animals.columns.tolist())

    # the eight tracks over the land cover
    rgb, ext = mf.cover_image(w / "cover.tif")
    fig, ax = plt.subplots(figsize=(7.4, 4.2), dpi=150)
    ax.imshow(rgb, extent=ext, interpolation="nearest")
    for _, r in lines.iterrows():
        xs, ys = r.geometry.xy
        ax.plot(xs, ys, lw=0.55, color=mf.COLORS[r[mf.ID]], alpha=0.9)
    b = pts.total_bounds
    pad = 0.02
    ax.set_xlim(b[0] - pad, b[2] + pad)
    ax.set_ylim(b[1] - pad, b[3] + pad)
    mf.lonlat(ax)
    mf.scalebar(ax, 5)
    hs = [Line2D([], [], color=mf.COLORS[a], lw=2, label=a) for a in mf.ANIMALS]
    ax.legend(handles=hs, ncol=8, loc="upper center", bbox_to_anchor=(0.5, 1.13), frameon=False, fontsize=8.5,
              handlelength=1.2, columnspacing=1.0)
    fig.tight_layout()
    mf.save(fig, OUT / "tracks.png")

    # when each animal was followed
    pts["t"] = pd.to_datetime(pts["timestamp"])
    rec["t"] = pd.to_datetime(rec["timestamp"])

    def timeline(ax):
        for i, a in enumerate(mf.ANIMALS[::-1]):
            s = pts[pts[mf.ID] == a]
            ax.plot([s.t.min(), s.t.max()], [i, i], lw=7, color=mf.COLORS[a], solid_capstyle="butt")
            ax.text(s.t.max() + pd.to_timedelta(12, unit="D"), i, f"{len(s):,} locations", va="center", fontsize=8.5,
                    color=figs.INK2)
        ax.set_yticks(range(8), mf.ANIMALS[::-1])
        import matplotlib.dates as mdates

        ax.set_xlim(pd.Timestamp("2009-01-01"), pd.Timestamp("2011-10-01"))
        ax.xaxis.set_major_locator(mdates.MonthLocator(bymonth=[1, 7]))
        ax.xaxis.set_major_formatter(mdates.DateFormatter("%b %Y"))
        ax.grid(axis="y", visible=False)

    figs.chart(OUT / "timeline.png", timeline, width=900, height=360)

    # one day of M4: the fixes and the attempts that gave no location
    # local noon to noon (New York in winter: UTC - 5)
    day0, day1 = pd.Timestamp("2010-03-05 17:00"), pd.Timestamp("2010-03-06 17:00")
    r4 = rec[(rec[mf.ID] == "M4") & (rec.t >= day0) & (rec.t < day1)]
    ok = r4[~r4.geometry.is_empty & r4.geometry.notna()]
    miss = r4[r4.geometry.is_empty | r4.geometry.isna()]
    fig, (a1, a2) = plt.subplots(1, 2, figsize=(9.6, 4.4), dpi=150, gridspec_kw={"width_ratios": [1.15, 1]})
    a1.plot(ok.geometry.x, ok.geometry.y, color=figs.INK2, lw=0.8)
    sc = a1.scatter(ok.geometry.x, ok.geometry.y, c=(ok.t - day0).dt.total_seconds() / 3600, s=9, cmap="viridis",
                    zorder=3)
    mf.lonlat(a1)
    a1.locator_params(nbins=4)
    mf.scalebar(a1, 0.5)
    cb = fig.colorbar(sc, ax=a1, shrink=0.8)
    cb.set_label("Hours from noon", fontsize=9)
    a1.set_title("M4, 5 to 6 March 2010", loc="left")
    hrs_ok = (ok.t - day0).dt.total_seconds() / 3600
    hrs_miss = (miss.t - day0).dt.total_seconds() / 3600
    a2.eventplot([hrs_ok, hrs_miss], lineoffsets=[1, 0], linelengths=0.7, colors=[figs.FOREST, "#b8b8b8"],
                 linewidths=0.8)
    a2.set_yticks([1, 0], ["location", "no location"])
    a2.set_xlim(0, 24)
    a2.set_xticks(range(0, 25, 3), [f"{(12 + h) % 24:02d}:00" for h in range(0, 25, 3)])
    a2.set_xlabel("Local time (UTC - 5)")
    a2.grid(axis="y", visible=False)
    a2.set_title(f"{len(ok)} locations, {len(miss)} attempts without one", loc="left")
    fig.tight_layout()
    mf.save(fig, OUT / "one-day.png")

    # the table of the animals
    rows = []
    for a in mf.ANIMALS:
        s, r = pts[pts[mf.ID] == a], rec[rec[mf.ID] == a]
        lag = s.t.diff().dt.total_seconds().div(60).median()
        rows.append({"animal": a, "sex": "female" if a.startswith("F") else "male", "records": len(r),
                     "locations": len(s), "share located (%)": round(100 * len(s) / len(r), 1),
                     "first": s.t.min().strftime("%Y-%m-%d"), "last": s.t.max().strftime("%Y-%m-%d"),
                     "median interval (min)": round(lag, 1)})
    figs.write_csv(DATA / "animals.csv", rows)
    for r in rows:
        print(r)
    print(animals.drop(columns="geometry").to_string())


if __name__ == "__main__":
    main()
