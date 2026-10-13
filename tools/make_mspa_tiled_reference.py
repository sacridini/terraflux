# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's mspa_tiled (GuidosToolbox's MSPA Tiling) gives, for Terraflux's tests: digests of the MSPA
maps of landschaft's input.tif by several tilings and settings, and of a map made of it whose tiles take GTB's
shortcuts (all missing, all foreground, background and missing, foreground and missing), written to
tests/data/reference/mspa_tiled.json:

    python tools/make_mspa_tiled_reference.py

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
INPUT = ROOT / "extern" / "landschaft" / "src" / "landschaft" / "data" / "input.tif"

CASES = [
    {"tile_size": 600, "overlap": 200},
    {"tile_size": 400, "overlap": 100, "connectivity": 4},
    {"tile_size": 300, "overlap": 120, "edge_width": 3, "internal": False},
    {"tile_size": 2000, "overlap": 200},
]


def digest(a) -> str:
    return hashlib.sha256(np.ascontiguousarray(np.asarray(a, dtype=np.uint8)).tobytes()).hexdigest()


def shortcuts(a: np.ndarray) -> np.ndarray:
    """input.tif's top-left 300 x 450 cells with bands all missing, all foreground, background and missing,
    foreground and missing."""
    m = a[:300, :450].copy()
    m[:100] = 0
    m[100:300, :150] = 2
    m[100:300, 150:250] = np.where(m[100:300, 150:250] == 0, 0, 1)
    m[100:200, 250:350] = np.where((np.arange(100)[:, None] + np.arange(100)[None]) % 7 == 0, 0, 2)
    return m


def main() -> None:
    a = ls.load_raster(INPUT).values
    out: dict = {"landschaft": ls.__version__, "cases": []}
    for case in CASES:
        out["cases"].append({**case, "map": "input", "digest": digest(ls.mspa_tiled(a, **case))})
    s = shortcuts(a)
    for case in [{"tile_size": 120, "overlap": 40}, {"tile_size": 90, "overlap": 30, "connectivity": 4}]:
        out["cases"].append({**case, "map": "shortcuts", "digest": digest(ls.mspa_tiled(s, **case))})
    out["whole"] = digest(ls.mspa(a))
    path = REF / "mspa_tiled.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
