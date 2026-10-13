# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's MaxNet gives, for Terraflux's tests, written to tests/data/reference/sdm_maxnet.npz and
.json:

    python tools/make_sdm_maxnet_reference.py

Three continuous predictors and a categorical one (land cover classes) on a grid, presences drawn where a
known suitability is high and background points everywhere: MaxNet with the default classes (hinges and
thresholds with this many presences), linear and quadratic only, products, a stronger regularisation and the
presences kept out of the background; its coefficients (names and values), alpha and entropy, and its
predictions on the stack and on a table (cloglog, logistic, exponential and link; clamped and not).
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pandas as pd
import xarray as xr

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.sdm


def main() -> None:
    layers = []
    for k, name in enumerate(["temp", "prec", "soil"]):
        f = ls.nlm.gaussian_field(40, 30, autocorr_range=8 + 3 * k, resolution=1000.0, seed=31 + k)
        layers.append(f.copy(data=np.round(f.values * (10 + 5 * k) + k, 3)))
    cover = ls.nlm.random_cluster(40, 30, p=0.5, ai=[0.4, 0.3, 0.2, 0.1], resolution=1000.0, seed=40,
                                  rescale=False).astype(np.float64)
    layers.append(cover)
    names = ["temp", "prec", "soil", "cover"]
    env = xr.concat(layers, dim="band").assign_coords(band=names)
    env = env.rio.write_transform(layers[0].rio.transform())
    v = {n: env.sel(band=n).values.ravel() for n in names}
    suit = np.exp(-((v["temp"] - 6) ** 2) / 8 - ((v["prec"] - 9) ** 2) / 30) * np.where(v["cover"] == 2, 1.5, 1.0)
    rng = np.random.default_rng(8)
    pres_cells = rng.choice(v["temp"].size, 90, replace=False, p=suit / suit.sum())
    bg_cells = rng.choice(v["temp"].size, 400, replace=False)
    pres = pd.DataFrame({n: v[n][pres_cells] for n in names})
    bg = pd.DataFrame({n: v[n][bg_cells] for n in names})
    X = pd.concat([pres, bg], ignore_index=True)
    y = np.r_[np.ones(len(pres)), np.zeros(len(bg))]
    table = pd.DataFrame({n: v[n][::7] for n in names})
    table.loc[3, "cover"] = 9.0  # a class unseen in training

    out: dict[str, np.ndarray] = {"in/env": env.values, "in/pres": pres.to_numpy(), "in/bg": bg.to_numpy(),
                                  "in/table": table.to_numpy()}
    cases: list[dict] = []
    for name, kw in {"default": {"categorical": ["cover"]},
                     "lq": {"classes": "lq", "categorical": ["cover"]},
                     "lqp": {"classes": "lqp", "regmult": 2.0},
                     "ht": {"classes": "ht", "regmult": 0.5, "categorical": ["cover"]},
                     "nosamples": {"classes": "lq", "add_samples_to_background": False}}.items():
        args = dict(kw)
        Xk = X if "cover" in args.get("categorical", []) else X.drop(columns="cover")
        m = S.MaxNet(categorical=args.pop("categorical", None), **args).fit(Xk, y)
        out[f"{name}/betas"] = np.array(list(m.betas_.values()))
        out[f"{name}/stats"] = np.array([m.alpha_, m.entropy_])
        envk = env if "cover" in kw.get("categorical", []) else env.sel(band=["temp", "prec", "soil"])
        tk = table if "cover" in kw.get("categorical", []) else table.drop(columns="cover")
        for t in ("cloglog", "logistic", "exponential", "link"):
            out[f"{name}/stack_{t}"] = np.asarray(
                m.predict_output(pd.DataFrame({b: envk.sel(band=b).values.ravel() for b in envk.band.values}),
                                 type=t), dtype=np.float64)
        out[f"{name}/table"] = np.asarray(m.predict_output(tk), dtype=np.float64)
        out[f"{name}/table_free"] = np.asarray(m.predict_output(tk, clamp=False), dtype=np.float64)
        cases.append({"name": name, "params": kw, "names": list(m.betas_), "classes": m.feature_classes_,
                      "with_cover": "cover" in kw.get("categorical", [])})

    np.savez_compressed(REF / "sdm_maxnet.npz", **out)
    (REF / "sdm_maxnet.json").write_text(json.dumps(cases, indent=1) + "\n", encoding="utf-8")
    print(REF / "sdm_maxnet.npz", len(out), [c["classes"] for c in cases])


if __name__ == "__main__":
    main()
