# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's SWAT+ gives, for Terraflux's tests: the Hubbard Brook W2 example of SWAT+ (landschaft's
hydro/swat/data/hubbard_brook.zip) run at every step with every table printed, and at a step with some tables,
a spin-up and the plants' C factor; written to tests/data/reference/swat.npz (the cases to swat.json):

    python tools/make_swat_reference.py

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import landschaft as ls
from landschaft.hydro.swat import STEPS, TABLES, WEATHER_COLUMNS
from landschaft.hydro.swat._run import _run_raw

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
EXAMPLE = ROOT / "extern" / "landschaft" / "src" / "landschaft" / "hydro" / "swat" / "data" / "hubbard_brook.zip"

CASES = [
    {"name": "year", "step": "year"},
    {"name": "day", "step": "day"},
    {"name": "month", "step": "month", "tables": ["hru_ls", "channel_sd", "basin_wb"], "nyskip": 1},
    {"name": "aa_plant", "step": "aa", "c_factor": "plant"},
]


def frame(df, obj) -> np.ndarray:
    """year, day of the year, the object (not for the basin) and the columns, as the Terraflux table."""
    dates = df.index.get_level_values("date")
    cols = [dates.year.to_numpy(np.float64), dates.dayofyear.to_numpy(np.float64)]
    if obj != "basin":
        cols.append(df.index.get_level_values(obj).to_numpy(np.float64))
    return np.column_stack(cols + [df[c].to_numpy(np.float64) for c in df.columns])


def main() -> None:
    project = ls.hydro.swat.load_txtinout(EXAMPLE)
    out: dict[str, np.ndarray] = {}
    for case in CASES:
        chosen = case.get("tables", list(TABLES))
        tables = {t: ([case["step"]] if t in chosen else []) for t in TABLES}
        r = ls.hydro.swat.run(project, tables=tables, nyskip=case.get("nyskip"), c_factor=case.get("c_factor", "swatplus"))
        for t in chosen:
            out[f"{case['name']}/{t}"] = frame(r.table(t, case["step"]), TABLES[t][1])
        w = r.weather
        names = _run_raw(project, dump=[1])["stations"]   # the port's numbers of the stations
        stations = {s: k + 1 for k, s in enumerate(names)}
        dates = w.index.get_level_values("date")
        out[f"{case['name']}/weather"] = np.column_stack(
            [dates.year.to_numpy(np.float64), dates.dayofyear.to_numpy(np.float64),
             np.array([stations[s] for s in w.index.get_level_values("station")], np.float64)]
            + [w[c].to_numpy(np.float64) for c in WEATHER_COLUMNS])
    np.savez_compressed(REF / "swat.npz", **out)
    meta = {"landschaft": ls.__version__, "steps": list(STEPS), "cases": CASES}
    (REF / "swat.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "swat.npz", len(out))


if __name__ == "__main__":
    main()
