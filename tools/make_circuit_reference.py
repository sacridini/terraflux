# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's circuits give, for Terraflux's tests: Omniscape on the cases of landschaft's references
(tests/data/reference/circuit/omniscape.npz, with Omniscape.jl's options), one more with conditions, a
reclassification and single precision, and the advanced mode on its cases (advanced_inputs.npz), written to
tests/data/reference/circuit_landschaft.npz (and the extra case's options to circuit_landschaft.json):

    python tools/make_circuit_reference.py

Run with the landschaft the submodule points at installed. landschaft's baseline build (not its AVX2 one, which
Terraflux does not build) is what Terraflux's equals bit for bit.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

os.environ["LANDSCHAFT_CIRCUIT_AVX2"] = "0"

import numpy as np  # noqa: E402

import landschaft as ls  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
LS = ROOT / "extern" / "landschaft" / "tests" / "data" / "reference" / "circuit"
OPTION_NAMES = {"connect_four_neighbors_only": "four_neighbors", "calc_flow_potential": "flow_potential",
                "calc_normalized_current": "normalized_current"}

EXTRA = {"radius": 5, "block_size": 3, "buffer": 1, "flow_potential": True, "normalized_current": True,
         "reclass_table": [[1.0, 2.0], [7.0, None]], "precision": "single", "comparison1": "within",
         "condition1_lower": -0.5, "condition1_upper": 0.5}


def main() -> None:
    out: dict[str, np.ndarray] = {}
    cases: dict[str, dict] = {}
    z = np.load(LS / "omniscape.npz")
    for name in sorted({k.split("/")[0] for k in z.files}):
        options = json.loads(str(z[f"{name}/options"]))
        kw = {OPTION_NAMES.get(k, k): v for k, v in options.items() if k != "source_from_resistance"}
        source = None if options.get("source_from_resistance") else z[f"{name}/source"]
        ds = ls.omniscape(z[f"{name}/resistance"], source=source, **kw)
        cases[name] = {"options": kw, "source": source is not None}
        out[f"omniscape/{name}/resistance"] = z[f"{name}/resistance"]
        if source is not None:
            out[f"omniscape/{name}/source"] = source
        for k in ds.data_vars:
            out[f"omniscape/{name}/{k}"] = ds[k].values.astype(np.float64)
    # conditions, a reclassification and single precision: os_test1's resistance rounded (plus 1), the condition
    r = np.round(z["os_test1/resistance"]) + 1
    cond = np.nan_to_num(r, nan=1.0)
    ds = ls.omniscape(r, condition1=cond, **{k: v for k, v in EXTRA.items()})
    out["extra/resistance"] = r
    out["extra/condition1"] = cond
    for k in ds.data_vars:
        out[f"extra/{k}"] = ds[k].values.astype(np.float64)
    a = np.load(LS / "advanced_inputs.npz")
    for name in sorted({k.split("/")[0] for k in a.files}):
        res = ls.circuit.advanced(a[f"{name}/conductance"], a[f"{name}/source"], a[f"{name}/ground"],
                                  four_neighbors=bool(a[f"{name}/four"]))
        for k in ("conductance", "source", "ground"):
            out[f"advanced/{name}/{k}"] = a[f"{name}/{k}"]
        out[f"advanced/{name}/four"] = np.array(float(a[f"{name}/four"]))
        out[f"advanced/{name}/current"] = np.asarray(res.current, dtype=np.float64)
        out[f"advanced/{name}/voltage"] = np.asarray(res.voltage, dtype=np.float64)
    path = REF / "circuit_landschaft.npz"
    np.savez(path, **out)
    (REF / "circuit_landschaft.json").write_text(json.dumps({"landschaft": ls.__version__, "omniscape": cases, "extra": EXTRA}) + "\n",
                                                 encoding="utf-8")
    print(path, len(out))


if __name__ == "__main__":
    main()
