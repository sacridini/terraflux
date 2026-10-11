# SPDX-License-Identifier: GPL-3.0-or-later
"""The catalogue of landscape metrics, from landschaft, as Rust (crates/ops/src/lsm_catalog.rs):

    python tools/make_lsm_catalog.py

landschaft's metrics are those of the R package landscapemetrics (lsm_abbreviations_names), and as extras the
FRAGSTATS metrics landscapemetrics lacks (contrast, proximity, similarity, connectance and more statistics).
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "src"))

from landschaft.lsm.catalog import _full_table  # noqa: E402


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> None:
    df = _full_table()
    out = [
        "// SPDX-License-Identifier: GPL-3.0-or-later",
        "//",
        "// The catalogue of landscape metrics, written by tools/make_lsm_catalog.py from landschaft: do not edit.",
        "",
        "/// A landscape metric: its abbreviation, its name, its type, its level (patch, class or landscape) and",
        "/// whether it is one of FRAGSTATS' that landscapemetrics lacks.",
        "pub struct Metric {",
        "    pub metric: &'static str,",
        "    pub name: &'static str,",
        "    pub kind: &'static str,",
        "    pub level: &'static str,",
        "    pub extra: bool,",
        "}",
        "",
        "/// Every metric, by level (patch, class, landscape) and abbreviation.",
        f"pub static METRICS: [Metric; {len(df)}] = [",
    ]
    for r in df.itertuples():
        out.append(
            f"    Metric {{ metric: {rust_str(r.metric)}, name: {rust_str(r.name)}, kind: {rust_str(r.type)}, "
            f"level: {rust_str(r.level)}, extra: {'true' if r.extra else 'false'} }},"
        )
    out.append("];")
    path = ROOT / "crates" / "ops" / "src" / "lsm_catalog.rs"
    path.write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")
    # as cargo fmt leaves it
    subprocess.run(["rustfmt", "--edition", "2024", "--config-path", str(ROOT / "rustfmt.toml"), str(path)], check=True)
    print(f"{path}: {len(df)} metrics ({int(df['extra'].sum())} extras)")


if __name__ == "__main__":
    main()
