# SPDX-License-Identifier: GPL-3.0-or-later
"""The maps GuidosToolbox's reference outputs were made from, for Terraflux's tests of the GTB operators.

landschaft keeps GTB's outputs as digests (extern/landschaft/tests/data/reference/idl_reference.json) of maps
its tools draw with numpy; this writes those maps, with the names the digests use, into
tests/data/reference/gtb_inputs.npz so that Rust can run the same cases:

    python tools/make_gtb_inputs.py

Needs numpy, scipy and rasterio (landschaft's test scenes).
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
LS = ROOT / "extern" / "landschaft"
sys.path.insert(0, str(LS / "tests"))
sys.path.insert(0, str(LS / "tools"))

import make_reference_idl as ref  # noqa: E402


def main() -> None:
    maps: dict[str, np.ndarray] = {}
    for inputs in (ref.accounting_inputs(), ref.spa_inputs(), ref.fad_inputs(), ref.fos_inputs()):
        for name, a in inputs.items():
            a = np.ascontiguousarray(a, dtype=np.uint8)
            assert name not in maps or np.array_equal(maps[name], a), name
            maps[name] = a
    # every map a reference case reads, and only those, as the digests say
    cases = json.loads((LS / "tests" / "data" / "reference" / "idl_reference.json").read_text())
    used = {c["input"]: c["input_sha256"] for k in ("accounting", "spa", "fad_mscale", "fos") for c in cases[k]}
    for name, digest in used.items():
        assert hashlib.sha256(maps[name].tobytes()).hexdigest() == digest, f"{name} is not the map of the digests"
    out = ROOT / "tests" / "data" / "reference" / "gtb_inputs.npz"
    np.savez_compressed(out, **{n: maps[n] for n in sorted(used)})
    print(f"{out}: {', '.join(sorted(used))}")


if __name__ == "__main__":
    main()
