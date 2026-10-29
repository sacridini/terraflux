"""Figures of "The data of step selection functions" (website/content/tutorials/animal-movement/step-selection.md).

The steps, the random steps and the covariates are Terraflux's (move.random_steps, move.extract_covariates);
the conditional logistic regression that the page fits on them is written here in numpy (Newton-Raphson on the
partial likelihood, Breslow's and the exact likelihood being the same with one case per stratum), as R's
survival::clogit and amt::fit_issf fit it.

    python website/figures/animal-movement/step-selection.py
"""

import numpy as np
import matplotlib.pyplot as plt
import pandas as pd

import movefig as mf
from movefig import figs

SLUG = "step-selection"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")
CONTROLS = [2, 5, 10, 20, 50]

STEPS = list(mf.CLEAN) + [
    {"id": "aeqd", "op": "move.aeqd_crs", "inputs": {"tracks": "unique.tracks"}},
    {"id": "resample", "op": "move.resample", "inputs": {"tracks": "aeqd.tracks"},
     "params": {"rate": "10min", "tolerance": "1min"}},
    {"id": "bursts", "op": "move.filter_min_n_burst", "inputs": {"tracks": "resample.tracks"},
     "params": {"min_n": 3}},
    {"id": "steps", "op": "move.steps", "inputs": {"tracks": "bursts.tracks"}},
]
LAND = list(mf.LANDSCAPE) + [
    {"id": "forest", "op": "raster.calc", "inputs": {"cover": "cover.raster"},
     "params": {"expression": "1 if cover == 10 else 0"}},
]


def clogit(df: pd.DataFrame, cols: list[str], strata: str = "stratum") -> pd.DataFrame:
    """Conditional logistic regression of case_ on cols, one case per stratum: coefficients, standard errors."""
    d = df.dropna(subset=cols + ["case_"]).copy()
    # strata with their case and at least one control
    ok = d.groupby(strata)["case_"].transform(lambda c: (c.sum() == 1) and (len(c) > 1))
    d = d[ok.astype(bool)]
    X = d[cols].to_numpy(float)
    y = d["case_"].to_numpy(float)
    g = pd.factorize(d[strata])[0]
    beta = np.zeros(len(cols))
    for _ in range(50):
        eta = X @ beta
        m = pd.Series(eta).groupby(g).transform("max").to_numpy()
        w = np.exp(eta - m)
        s = np.bincount(g, weights=w)
        p = w / s[g]
        xbar = np.vstack([np.bincount(g, weights=p * X[:, j]) for j in range(X.shape[1])]).T
        grad = (X[y == 1] - xbar[g[y == 1]]).sum(axis=0)
        xc = X - xbar[g]
        H = (xc * p[:, None]).T @ xc
        step = np.linalg.solve(H, grad)
        beta += step
        if np.abs(step).max() < 1e-10:
            break
    se = np.sqrt(np.diag(np.linalg.inv(H)))
    return pd.DataFrame({"term": cols, "coef": beta, "se": se, "exp(coef)": np.exp(beta)}), int(len(np.unique(g)))


def design(r: pd.DataFrame) -> pd.DataFrame:
    r = r.copy()
    r["stratum"] = r[mf.ID].astype(str) + "_" + r["step_id_"].astype(int).astype(str)
    r["elevation_100m"] = r["elevation"] / 100
    r["log_sl"] = np.log(r["sl_"].clip(lower=1))
    r["cos_ta"] = np.cos(r["ta_"])
    return r


TERMS = ["forest", "elevation_100m", "log_sl", "cos_ta"]
LABELS = {"forest": "forest (tree cover at the end of the step)", "elevation_100m": "elevation (per 100 m)",
          "log_sl": "log(step length)", "cos_ta": "cos(turn angle)"}


def main():
    w = mf.work(SLUG)
    nodes = STEPS + LAND + [
        {"id": "sl", "op": "move.fit_distribution", "inputs": {"values": "steps.steps"}, "params": {"name": "gamma"}},
        {"id": "ta", "op": "move.fit_distribution", "inputs": {"values": "steps.steps"},
         "params": {"name": "vonmises"}},
        {"id": "sl_draws", "op": "move.random_numbers", "inputs": {"distribution": "sl.distribution"},
         "params": {"n": 20000, "seed": 1}},
        {"id": "ta_draws", "op": "move.random_numbers", "inputs": {"distribution": "ta.distribution"},
         "params": {"n": 20000, "seed": 1}},
        {"id": "wc", "op": "raster.write", "inputs": {"value": "cover.raster"}, "params": {"path": "cover.tif"}},
    ]
    outs = {"steps": "steps.steps", "sl": "sl.table", "ta": "ta.table", "sl_draws": "sl_draws.table",
            "ta_draws": "ta_draws.table"}
    for n in CONTROLS:
        nodes += [
            {"id": f"rs{n}", "op": "move.random_steps", "inputs": {"steps": "steps.steps"},
             "params": {"n_control": n, "seed": 1}},
            {"id": f"cov{n}", "op": "move.extract_covariates",
             "inputs": {"tracks": f"rs{n}.steps", "forest": "forest.map", "elevation": "dem.raster"}},
        ]
        outs[f"cov{n}"] = f"cov{n}.tracks"
    outs["distributions"] = "rs10.distributions"
    nodes.append({"id": "tod", "op": "move.time_of_day", "inputs": {"tracks": "cov10.tracks"}})
    outs["tod"] = "tod.tracks"
    got = mf.run(w, nodes, outs)
    st = got["steps"]
    print("fits:", got["sl"].to_dict("records"), got["ta"].to_dict("records"))
    print(got["distributions"].drop(columns="geometry").to_string())
    dist = [{"animal": r[mf.ID], "gamma shape": round(r.sl_shape, 3), "gamma scale (m)": round(r.sl_scale, 1),
             "von Mises kappa": round(r.ta_kappa, 3)} for _, r in got["distributions"].iterrows()]
    dist.append({"animal": "all (pooled)", "gamma shape": round(got["sl"]["shape"][0], 3),
                 "gamma scale (m)": round(got["sl"].scale[0], 1), "von Mises kappa": round(got["ta"].kappa[0], 3)})
    figs.write_csv(DATA / "distributions.csv", dist)

    # the fitted distributions over the observed steps
    sl_par = dict(zip(got["sl"].columns, got["sl"].iloc[0]))
    ta_par = dict(zip(got["ta"].columns, got["ta"].iloc[0]))
    print(sl_par, ta_par)

    def sl_fig(ax):
        bins = np.arange(0, 825, 25)
        ax.hist(st.sl_.dropna(), bins=bins, density=True, color=figs.FOREST, alpha=0.6, label="observed steps")
        ax.hist(got["sl_draws"]["value"], bins=bins, density=True, histtype="step", lw=1.6, color=figs.EMBER,
                label="draws of the fitted gamma")
        ax.set_xlabel("Step length (m)")
        ax.set_ylabel("Density")
        ax.legend(frameon=False, fontsize=9)

    figs.chart(OUT / "gamma.png", sl_fig, width=900, height=380)

    def ta_fig(ax):
        bins = np.arange(-180, 181, 10)
        ax.hist(np.degrees(st.ta_.dropna()), bins=bins, density=True, color=figs.WATER, alpha=0.6,
                label="observed turns")
        ax.hist(np.degrees(got["ta_draws"]["value"]), bins=bins, density=True, histtype="step", lw=1.6,
                color=figs.EMBER, label="draws of the fitted von Mises")
        ax.set_xticks(range(-180, 181, 45))
        ax.set_xlabel("Turn angle (degrees)")
        ax.set_ylabel("Density")
        ax.legend(frameon=False, fontsize=9, loc="upper center")

    figs.chart(OUT / "vonmises.png", ta_fig, width=900, height=380)

    # one stratum over the land cover
    r10 = got["cov10"]
    print(r10.columns.tolist())
    print(len(r10), "rows,", int(r10.case_.sum()), "observed steps,", r10[mf.ID].nunique(), "animals")
    import rasterio
    from matplotlib import colors as mcolors
    from matplotlib.patches import Patch
    from pyproj import Transformer

    # drawn over the land cover the values are taken from (lon/lat), the steps taken back to lon/lat
    with rasterio.open(w / "cover.tif") as src:
        cov = src.read(1)
        tr = src.transform
    rgb, extent = mf.cover_image(w / "cover.tif")
    m4 = r10[r10[mf.ID] == "M4"]
    sid = sorted(m4.step_id_.unique())[400]
    s = m4[m4.step_id_ == sid].copy()
    back = Transformer.from_crs(r10.crs, "EPSG:4326", always_xy=True)
    s["lon1"], s["lat1"] = back.transform(s.x1_.to_numpy(), s.y1_.to_numpy())
    s["lon2"], s["lat2"] = back.transform(s.x2_.to_numpy(), s.y2_.to_numpy())
    obs = s[s.case_ == 1].iloc[0]
    fig, ax = plt.subplots(figsize=(6.8, 5.0), dpi=150)
    ax.imshow(rgb, extent=extent, interpolation="nearest")
    for _, q in s[s.case_ == 0].iterrows():
        ax.plot([q.lon1, q.lon2], [q.lat1, q.lat2], color=figs.INK2, lw=1.1, ls="--")
        ax.scatter([q.lon2], [q.lat2], s=26, color="white", edgecolor=figs.INK2, zorder=3)
    ax.plot([obs.lon1, obs.lon2], [obs.lat1, obs.lat2], color=figs.EMBER, lw=2.4)
    ax.scatter([obs.lon2], [obs.lat2], s=40, color=figs.EMBER, edgecolor="white", zorder=4)
    ax.scatter([obs.lon1], [obs.lat1], s=40, color=figs.INK, zorder=4)
    xs = np.r_[s.lon1, s.lon2]
    ys = np.r_[s.lat1, s.lat2]
    # 80 m around the steps
    py = 80 / 111320
    px = py / np.cos(np.radians(ys.mean()))
    ax.set_xlim(xs.min() - px, xs.max() + px)
    ax.set_ylim(ys.min() - py, ys.max() + py)
    mf.lonlat(ax, lat=ys.mean())
    ax.locator_params(axis="x", nbins=4)
    mf.scalebar(ax, km=0.1, lat=ys.mean())
    # the classes in view, for the legend
    r0, c0 = rasterio.transform.rowcol(tr, xs.min() - px, ys.max() + py)
    r1, c1 = rasterio.transform.rowcol(tr, xs.max() + px, ys.min() - py)
    seen = np.unique(cov[r0:r1 + 1, c0:c1 + 1])
    ax.legend(handles=[Patch(color=1 - 0.45 * (1 - np.array(mcolors.to_rgb(c))), label=n)
                       for v, (n, c) in mf.COVER.items() if v in seen],
              loc="upper left", bbox_to_anchor=(1.01, 1), frameon=False, fontsize=8.5)
    fig.tight_layout()
    mf.save(fig, OUT / "stratum.png")
    print("stratum", sid, s[["case_", "sl_", "forest", "elevation"]].to_string())

    # used and available
    rows = []
    for case, name in ((1, "observed"), (0, "random")):
        q = r10[r10.case_ == case]
        rows.append({"steps": name, "n": len(q), "ends in forest (%)": round(100 * q.forest.mean(), 1),
                     "mean elevation (m)": round(q.elevation.mean(), 1),
                     "median length (m)": round(q.sl_.median(), 1)})
        print(rows[-1])
    figs.write_csv(DATA / "used-available.csv", rows)

    # the conditional logistic regression at 10 controls, and its stability with the number of controls
    fit, nstrata = clogit(design(r10), TERMS)
    print(fit.to_string(), nstrata, "strata")
    table = [{"term": LABELS[t], "coef": round(c, 3), "se": round(e, 3), "exp(coef)": round(x, 3)}
             for t, c, e, x in fit.itertuples(index=False)]
    figs.write_csv(DATA / "clogit.csv", table)
    curve = []
    for n in CONTROLS:
        f, _ = clogit(design(got[f"cov{n}"]), TERMS)
        row = f.set_index("term")
        curve.append({"controls": n, "forest": round(row.loc["forest", "coef"], 4),
                      "lower": round(row.loc["forest", "coef"] - 1.96 * row.loc["forest", "se"], 4),
                      "upper": round(row.loc["forest", "coef"] + 1.96 * row.loc["forest", "se"], 4)})
        print(curve[-1])
    figs.write_csv(DATA / "controls.csv", curve)

    # by time of day: forest selection at night and by day
    t = design(got["tod"])
    t["forest_night"] = t.forest * (t.tod == "night")
    f, _ = clogit(t, ["forest", "forest_night", "elevation_100m", "log_sl", "cos_ta"])
    print(f.to_string())
    figs.write_csv(DATA / "clogit-tod.csv", [
        {"term": {"forest": "forest (by day)", "forest_night": "forest × night (the difference at night)",
                  "elevation_100m": "elevation (per 100 m)", "log_sl": "log(step length)",
                  "cos_ta": "cos(turn angle)"}[tm], "coef": round(c, 3), "se": round(e, 3)}
        for tm, c, e, _ in f.itertuples(index=False)])


if __name__ == "__main__":
    main()
