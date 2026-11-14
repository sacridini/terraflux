# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's SWAT+ gives with parameters changed, for Terraflux's tests (swat.set): the Hubbard Brook W2
example (landschaft's hydro/swat/data/hubbard_brook.zip) with rows added to its calibration.cal by
SwatProject.set_text, the tables SWAT+ prints by year; written to tests/data/reference/swat_set.npz (the cases to
swat_set.json):

    python tools/make_swat_set_reference.py

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import landschaft as ls
from landschaft.hydro.swat import TABLES

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
EXAMPLE = ROOT / "extern" / "landschaft" / "src" / "landschaft" / "hydro" / "swat" / "data" / "hubbard_brook.zip"

CASES = [
    {"name": "awc", "changes": [{"parameter": "awc", "change": "pctchg", "value": 20}]},
    {"name": "cn2", "changes": [{"parameter": "cn2", "change": "absval", "value": 70}]},
    {
        "name": "chain",
        "changes": [
            {"parameter": "awc", "change": "pctchg", "value": 20, "first_layer": 1, "last_layer": 2},
            {"parameter": "cn2", "change": "abschg", "value": -5, "objects": [1, -3, 6]},
            {"parameter": "esco", "change": "absval", "value": 0.6},
        ],
    },
]


def frame(df, obj) -> np.ndarray:
    """year, day of the year, the object (not for the basin) and the columns, as the Terraflux table."""
    dates = df.index.get_level_values("date")
    cols = [dates.year.to_numpy(np.float64), dates.dayofyear.to_numpy(np.float64)]
    if obj != "basin":
        cols.append(df.index.get_level_values(obj).to_numpy(np.float64))
    return np.column_stack(cols + [df[c].to_numpy(np.float64) for c in df.columns])


def change(project, c) -> None:
    """A row after the rows calibration.cal has, its count one more (the file the project names in file.cio)."""
    lines = project.text("calibration.cal").split("\n")
    n = int(lines[1])
    objects = c.get("objects", [])
    row = " ".join(str(x) for x in [c["parameter"], c["change"], c["value"], 0, c.get("first_layer", 0),
                                   c.get("last_layer", 0), 0, 0, 0, 0, len(objects), *objects])
    lines.insert(3 + n, row)
    lines[1] = str(n + 1)
    project.set_text("calibration.cal", "\n".join(lines))


def main() -> None:
    base = ls.hydro.swat.load_txtinout(EXAMPLE)
    out: dict[str, np.ndarray] = {}
    for case in CASES:
        project = base.copy()
        for c in case["changes"]:
            change(project, c)
        tables = {t: ["year"] for t in TABLES}
        r = ls.hydro.swat.run(project, tables=tables)
        for t in TABLES:
            out[f"{case['name']}/{t}"] = frame(r.table(t, "year"), TABLES[t][1])
    np.savez_compressed(REF / "swat_set.npz", **out)
    meta = {"landschaft": ls.__version__, "cases": CASES}
    (REF / "swat_set.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "swat_set.npz", len(out))


if __name__ == "__main__":
    main()
