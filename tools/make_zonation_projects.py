# SPDX-License-Identifier: GPL-3.0-or-later
"""The Zonation 5 projects of landschaft's reference cases, for Terraflux's tests of prioritize.zonation_project.

landschaft runs the z5 binary on a project per case (tests/prioritize_cases.py, written by
tools/make_reference_prioritize.py's write_project: settings.z5, the feature list, the link files and the rasters)
and keeps its outputs (extern/landschaft/tests/data/reference/prioritize). This writes the rasters of the cases
once into tests/data/reference/zonation_projects/ (the same files, one-strip GeoTIFFs) and the cases into
zonation_projects/projects.json: per case its group, the z5 switches, mode and bits, the text files of its project
and the rasters it uses:

    python tools/make_zonation_projects.py
"""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tools"))

import prioritize_cases as pc  # noqa: E402
from make_reference_prioritize import write_project  # noqa: E402

OUT = ROOT / "tests" / "data" / "reference" / "zonation_projects"


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    cases = {}
    for name, case in pc.CASES.items():
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            write_project(case, tmp)
            files, rasters = {}, []
            for p in sorted(tmp.iterdir()):
                if p.suffix == ".tif":
                    rasters.append(p.name)
                    target = OUT / p.name
                    if target.exists():
                        assert target.read_bytes() == p.read_bytes(), p.name
                    else:
                        shutil.copyfile(p, target)
                else:
                    files[p.name] = p.read_text(encoding="utf-8")
        cases[name] = {"group": case.group, "flags": " ".join(case.flags), "mode": case.mode, "bits": case.bits,
                       "files": files, "rasters": rasters}
    (OUT / "projects.json").write_text(json.dumps(cases, indent=1) + "\n", encoding="utf-8")
    print(f"{len(cases)} projects, {len(list(OUT.glob('*.tif')))} rasters")


if __name__ == "__main__":
    main()
