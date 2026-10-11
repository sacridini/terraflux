# SPDX-License-Identifier: GPL-3.0-or-later
"""The rasters Zonation 5's reference outputs were made from, for Terraflux's tests of prioritize.zonation.

landschaft keeps the z5 binary's outputs (extern/landschaft/tests/data/reference/prioritize) of rasters its tests
draw with numpy (tests/prioritize_cases.py) and write as one-strip GeoTIFFs; this writes those rasters, with the
values the files hold (their type's rounding, NaN for NoData), into tests/data/reference/zonation_inputs.npz,
the cases of the core group (their layers and ls.prioritize.zonation's arguments) and the files' NoData values
into zonation_inputs.json:

    python tools/make_zonation_inputs.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import prioritize_cases as pc  # noqa: E402


def file_values(spec: pc.RasterSpec) -> np.ndarray:
    """The values a raster's file gives back (as the tests write it), NaN for NoData."""
    v = np.asarray(spec.values, dtype=np.float64)
    nan = np.isnan(v)
    out = np.where(nan, 0.0, v).astype(spec.dtype).astype(np.float64)
    if spec.nodata is not None:
        out[nan] = np.nan
    return out


def main() -> None:
    cases = {name: c for name, c in pc.CASES.items() if c.group == "core"}
    used = sorted({r for c in cases.values() for r in c.layers}
                  | {v for c in cases.values() for v in c.params.values() if isinstance(v, str) and v in pc.RASTERS})
    out = ROOT / "tests" / "data" / "reference"
    np.savez_compressed(out / "zonation_inputs.npz", **{r: file_values(pc.RASTERS[r]) for r in used})
    meta = {name: {"layers": c.layers, "params": c.params} for name, c in cases.items()}
    # the files' NoData values (null: NaN or none), which value extraction tells from no value
    meta = {"cases": meta, "nodata": {r: (None if pc.RASTERS[r].nodata is None or np.isnan(pc.RASTERS[r].nodata)
                                          else float(pc.RASTERS[r].nodata)) for r in used}}
    (out / "zonation_inputs.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(f"{len(cases)} cases, rasters {used}")


if __name__ == "__main__":
    main()
