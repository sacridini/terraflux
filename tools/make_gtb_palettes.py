# SPDX-License-Identifier: GPL-3.0-or-later
"""GuidosToolbox's colour tables and class names, from landschaft, as Rust (crates/ops/src/gtb_palettes.rs):

    python tools/make_gtb_palettes.py

The tables are those landschaft writes into its GeoTIFFs and plots (src/landschaft/_palettes.py), GTB's own.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "src"))

from landschaft import _palettes as P  # noqa: E402
from landschaft._plotting._palettes import _fos_change_names  # noqa: E402
from landschaft.io import gtn_components_colormap, gtn_importance_colormap  # noqa: E402
from landschaft.mosaic import LM19_CLASSES, RECODE_103_TO_19  # noqa: E402

TABLES = {
    "MSPA_ON": ("GTB's MSPA table with transitions (mspacolorston): MSPA, SPA, Accounting", P.MSPA_ON_PALETTE),
    "FAD": ("FAD and FOS in 6 classes (fadcolors)", P.FAD_PALETTE),
    "FAD5": ("FAD and FOS in 5 classes (fadcolors5)", P.FAD5_PALETTE),
    "ENTROPY": ("entropy, contagion and clustering", P.ENTROPY_PALETTE),
    "LM": ("the landscape mosaic", P.LM_PALETTE),
    "CHANGE": ("simple and morphological change", P.CHANGE_PALETTE),
    "FOS_CHANGE": ("the change of FOS", P.FOSCHANGE_PALETTE),
    "DIST": ("the Euclidean distance's byte map", P.DIST_PALETTE),
    "CONTORTION": ("contortion", P.CONTORTION_PALETTE),
    "RESISTANCE": ("a resistance, cost distances and least-cost paths", P.RESISTANCE_PALETTE),
    "GTN_COMPONENTS": ("the components of a habitat network", [gtn_components_colormap()[i] for i in range(256)]),
    "GTN_IMPORTANCE": ("the importance of a habitat network's links and nodes",
                       [gtn_importance_colormap()[i] for i in range(256)]),
}


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> None:
    out = [
        "// SPDX-License-Identifier: GPL-3.0-or-later",
        "//",
        "// GuidosToolbox's colour tables and class names, written by tools/make_gtb_palettes.py from landschaft:",
        "// do not edit.",
        "",
    ]
    for name, (about, table) in TABLES.items():
        assert len(table) == 256, name
        out.append(f"/// GTB's colours of {about}, by value.")
        out.append(f"pub const {name}: [[u8; 3]; 256] = [")
        for i in range(0, 256, 8):
            out.append("    " + " ".join(f"[{r}, {g}, {b}]," for r, g, b in (c[:3] for c in table[i:i + 8])))
        out.append("];")
        out.append("")
    out.append("/// The classes of the landscape mosaic in 19 classes.")
    out.append(f"pub const LM19_CLASSES: [(u8, &str); {len(LM19_CLASSES)}] = [")
    out += [f"    ({k}, {rust_str(v)})," for k, v in sorted(LM19_CLASSES.items())]
    out.append("];")
    out.append("")
    out.append("/// The class of 19 of each class of 103 of the landscape mosaic (GTB's recodelm103).")
    out.append("pub const RECODE_103_TO_19: [u8; 256] = [")
    for i in range(0, 256, 16):
        out.append("    " + " ".join(f"{v}," for v in RECODE_103_TO_19[i:i + 16]))
    out.append("];")
    out.append("")
    names = _fos_change_names()
    out.append("/// The names of the values of FOS's change (its change + 100, and its codes).")
    out.append(f"pub const FOS_CHANGE_NAMES: [(u8, &str); {len(names)}] = [")
    out += [f"    ({k}, {rust_str(v)})," for k, v in sorted(names.items())]
    out.append("];")
    path = ROOT / "crates" / "ops" / "src" / "gtb_palettes.rs"
    path.write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")
    # as cargo fmt leaves it
    subprocess.run(["rustfmt", "--edition", "2024", "--config-path", str(ROOT / "rustfmt.toml"), str(path)], check=True)
    print(path)


if __name__ == "__main__":
    main()
