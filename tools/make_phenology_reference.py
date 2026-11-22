# SPDX-License-Identifier: GPL-3.0-or-later
"""What scipy and numpy give for Terraflux's tests of the phenology kernels (objects.series, raster.phenology):
`scipy.signal.savgol_filter(x, window, order, mode="interp")` of noisy series, and the least squares fit of harmonics of a
one-year period (`numpy.linalg.lstsq`) of a series on uneven dates with holes, with and without a trend, written to
tests/data/reference/phenology.json:

    python -I tools/make_phenology_reference.py

Needs numpy and scipy. The harmonic model is Terraflux's: t the days since 1 January of the first date's year,
columns 1, [(t - mean t) / 365.25], cos and sin of 2 pi k t / 365.25; the amplitude sqrt(a^2 + b^2), the phase
atan2(b, a) from 0 to 2 pi, the RMSE over the values.
"""

from __future__ import annotations

import datetime
import json
from pathlib import Path

import numpy as np
from scipy.signal import savgol_filter

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
YEAR = 365.25


def epoch_days(d: datetime.date) -> int:
    return (d - datetime.date(1970, 1, 1)).days


def harmonics(t: np.ndarray, y: np.ndarray, n: int, trend: bool) -> list[float]:
    ok = np.isfinite(y)
    first = datetime.date(1970, 1, 1) + datetime.timedelta(days=int(t[0]))
    origin = epoch_days(datetime.date(first.year, 1, 1))
    tt = t[ok] - origin
    yy = y[ok]
    mean_t = tt.mean()
    cols = [np.ones_like(tt)]
    if trend:
        cols.append((tt - mean_t) / YEAR)
    for k in range(1, n + 1):
        a = 2 * np.pi * k * tt / YEAR
        cols += [np.cos(a), np.sin(a)]
    a = np.column_stack(cols)
    c, *_ = np.linalg.lstsq(a, yy, rcond=None)
    pred = a @ c
    rmse = float(np.sqrt(np.mean((yy - pred) ** 2)))
    out = [float(c[0])]
    first_h = 1
    if trend:
        out.append(float(c[1]))
        first_h = 2
    for k in range(n):
        ca, cb = c[first_h + 2 * k], c[first_h + 2 * k + 1]
        out += [float(np.hypot(ca, cb)), float(np.arctan2(cb, ca) % (2 * np.pi))]
    out.append(rmse)
    return out


def main() -> None:
    rng = np.random.default_rng(20261122)
    out: dict = {"numpy": np.__version__}
    # savgol on noisy series of 40 and 23 values
    cases = []
    for n, window, order in [(40, 7, 2), (40, 11, 3), (23, 5, 4), (23, 9, 2), (31, 15, 3)]:
        x = np.sin(np.linspace(0, 5, n)) * 3 + rng.normal(scale=0.4, size=n)
        cases.append({"x": x.tolist(), "window": window, "order": order,
                      "y": savgol_filter(x, window, order, mode="interp").tolist()})
    out["savgol"] = cases
    # harmonics: two years of 8-day composites (uneven by the holes), clouds as NaN
    start = datetime.date(2022, 3, 1)
    days = [epoch_days(start) + 8 * k for k in range(90)]
    t = np.array(days, float)
    origin = epoch_days(datetime.date(2022, 1, 1))
    tt = t - origin
    y = (0.45 + 0.02 * (tt - tt.mean()) / YEAR + 0.3 * np.cos(2 * np.pi * (tt - 200) / YEAR)
         + 0.08 * np.cos(4 * np.pi * (tt - 120) / YEAR) + rng.normal(scale=0.03, size=len(t)))
    y[rng.random(len(t)) < 0.2] = np.nan
    fits = []
    for n, trend in [(1, False), (2, False), (2, True), (3, True)]:
        fits.append({"n": n, "trend": trend, "measures": harmonics(t, y, n, trend)})
    out["harmonics"] = {"t": t.tolist(), "y": [None if not np.isfinite(v) else float(v) for v in y], "fits": fits}
    (REF / "phenology.json").write_text(json.dumps(out), encoding="utf-8")
    print("wrote", REF / "phenology.json")


if __name__ == "__main__":
    main()
