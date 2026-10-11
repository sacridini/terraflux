# SPDX-License-Identifier: GPL-3.0-or-later
"""The maps plangea's reference outputs were made from, for Terraflux's tests of the restoration operators.

landschaft keeps plangea's outputs (extern/landschaft/tests/data/reference/plangea) of maps its tests draw with
numpy (tests/plangea_cases.py); this writes those maps into tests/data/reference/plangea_inputs.npz, and the
names and tables into plangea_inputs.json:

    python tools/make_plangea_inputs.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import plangea_cases as pc  # noqa: E402

REF = ROOT / "extern" / "landschaft" / "tests" / "data" / "reference" / "plangea"


def footprint(spec):
    """As landschaft's test_plangea_reference._footprint."""
    h = pc.footprint(spec["shape"], spec["seed"], spec.get("low", 0.0), nan=not spec.get("gradient"))
    if spec.get("gradient"):
        h[3, 4] = np.nan
    return h


def main() -> None:
    arrays = {
        "land_cover": pc.land_cover(),
        "past": pc.past(),
        "ecoregions": pc.ecoregions(),
        "elevation": pc.elevation(),
    }
    species, r = pc.ranges()
    arrays["ranges"] = r
    for name, spec in pc.IT_CASES.items():
        arrays[f"{name}/footprint"] = footprint(spec)
        if spec.get("restore"):
            conv = pc.conversion(spec["shape"], spec["seed"] + 100)
            arrays[f"{name}/reduction"] = pc.reduction(conv)
            if not spec.get("gradient"):
                arrays[f"{name}/restored"] = pc.restored(spec["shape"], spec["seed"] + 200)
    # the ecosystems: the ecoregion codes and the restored shares of the master pixels (ec_x)
    arrays["ecoregion_codes"] = pc.ecoregion_codes()
    with np.load(REF / "reference.npz") as z:
        arrays["ec_x"] = pc.ec_x(z["ec_land/master_index"].size)
    habitats: dict[str, list[str]] = {}
    for s, c in pc.habitat_table().itertuples(index=False):
        habitats.setdefault(str(s), []).append(str(c))
    limits = [[str(s), None if np.isnan(lo) else float(lo), None if np.isnan(hi) else float(hi)]
              for s, lo, hi in pc.limits_table().itertuples(index=False)]
    meta = {
        "classes": pc.CLASSES,
        "natural": pc.NATURAL,
        "anthropic": pc.ANTHROPIC,
        "species": list(species),
        "res": pc.RES,
        "suitability": habitats,
        "elevation_limits": limits,
        "it_cases": {k: {kk: (float(v) if isinstance(v, (float, np.floating)) else v) for kk, v in s.items()
                         if kk != "shape"} for k, s in pc.IT_CASES.items()},
        "oa_cases": pc.OA_CASES,
        "bd_cases": pc.BD_CASES,
        "scen_cases": pc.SCEN_CASES,
        "esys_cases": pc.ESYS_CASES,
        "scen_cd_cases": pc.SCEN_CD_CASES,
    }
    out = ROOT / "tests" / "data" / "reference"
    np.savez_compressed(out / "plangea_inputs.npz", **{k: np.ascontiguousarray(v, dtype=np.float64)
                                                       for k, v in arrays.items()})
    (out / "plangea_inputs.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(out / "plangea_inputs.npz", sorted(arrays))


if __name__ == "__main__":
    main()
