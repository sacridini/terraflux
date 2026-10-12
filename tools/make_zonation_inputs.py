# SPDX-License-Identifier: GPL-3.0-or-later
"""The rasters Zonation 5's reference outputs were made from, for Terraflux's tests of prioritize.zonation.

landschaft keeps the z5 binary's outputs (extern/landschaft/tests/data/reference/prioritize) of rasters its tests
draw with numpy (tests/prioritize_cases.py) and write as one-strip GeoTIFFs; this writes those rasters, with the
values the files hold (their type's rounding, NaN for NoData), into tests/data/reference/zonation_inputs.npz,
the cases (their group, layers, ls.prioritize.zonation's arguments as prioritize.zonation's settings and the
post-processing calls) and the files' NoData values into zonation_inputs.json, Zonation's post-processing files
(lzma-compressed by landschaft) as words into zonation_post.json, and the impacts landschaft accumulates for the
impact cases (Zonation projects, read by its read_project) into zonation_impacts.json:

    python tools/make_zonation_inputs.py
"""

from __future__ import annotations

import json
import lzma
import sys
import tempfile
import warnings
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tools"))

import prioritize_cases as pc  # noqa: E402
from landschaft.prioritize import zonation  # noqa: E402
from landschaft.prioritize._connectivity import Interaction, Kernel  # noqa: E402
from landschaft.prioritize._impacts import CumulativeScenario, ImpactScenario  # noqa: E402
from landschaft.prioritize._postprocess import parse_calls  # noqa: E402
from landschaft.prioritize._project import read_project  # noqa: E402
from make_reference_prioritize import write_project  # noqa: E402

REF = ROOT / "extern" / "landschaft" / "tests" / "data" / "reference" / "prioritize"
# the groups of cases Terraflux runs
GROUPS = ("core", "transform", "connectivity", "post", "impact")


def file_values(spec: pc.RasterSpec) -> np.ndarray:
    """The values a raster's file gives back (as the tests write it), NaN for NoData."""
    v = np.asarray(spec.values, dtype=np.float64)
    nan = np.isnan(v)
    out = np.where(nan, 0.0, v).astype(spec.dtype).astype(np.float64)
    if spec.nodata is not None:
        out[nan] = np.nan
    return out


def rasters_in(v) -> set:
    """The rasters a case's settings name (in lists, tuples, kernels' interactions, scenarios)."""
    if isinstance(v, str):
        return {v} if v in pc.RASTERS else set()
    if isinstance(v, dict):
        return set().union(*(rasters_in(x) for x in v.values())) if v else set()
    if isinstance(v, (list, tuple)):
        return set().union(*(rasters_in(x) for x in v)) if v else set()
    if isinstance(v, Interaction):
        return rasters_in(v.resource)
    return set()


def stem(path) -> str | None:
    return None if path is None else Path(path).stem


def settings(v):
    """A case's settings as prioritize.zonation's: kernels, interactions and impact scenarios as objects (their
    rasters by name), a matrix connectivity as {kernel, matrix}, an absolute retention as {absolute: raster},
    arrays as lists."""
    if isinstance(v, Kernel):
        return {"shape": v.shape, "distance": float(v.distance), "cut": float(v.cut)}
    if isinstance(v, Interaction):
        return {"kernel": settings(v.kernel), "resource": v.resource, "positive": bool(v.positive),
                "beta": float(v.beta), "normalize": bool(v.normalize)}
    if isinstance(v, ImpactScenario):
        return {"sources": stem(v.sources), "subareas": stem(v.subareas), "name": v.name,
                "area_factor": float(v.area_factor), "rasters": bool(v.rasters),
                "impacts": [{"group": s.group, "source": s.source, "footprint": float(s.footprint),
                             "buffers": [[int(r), float(x)] for r, x in s.buffers]} for s in v.impacts]}
    if isinstance(v, CumulativeScenario):
        return {"name": v.name, "subareas": stem(v.subareas), "area_factor": float(v.area_factor),
                "rasters": bool(v.rasters), "direct": v.direct, "direct_rasters": [stem(r) for r in v.direct_rasters],
                "direct_matrix": settings(v.direct_matrix), "cumulative": v.cumulative,
                "pressures": [{"raster": stem(p.raster), "normalize": bool(p.normalize), "offset": float(p.offset),
                               "divisor": float(p.divisor)} for p in v.pressures],
                "cumulative_matrix": settings(v.cumulative_matrix), "aggregation": v.aggregation,
                "functions": [{"group": g, "pressure": p, "points": [[float(x), float(y)] for x, y in pts]}
                              for (g, p), pts in v.functions.items()]}
    if isinstance(v, tuple) and len(v) == 2 and isinstance(v[0], Kernel):
        return {"kernel": settings(v[0]), "matrix": v[1]}
    if isinstance(v, tuple) and len(v) == 2 and v[0] in ("absolute", "fraction"):
        return {v[0]: v[1]}
    if isinstance(v, dict):
        return {k: settings(x) for k, x in v.items()}
    if isinstance(v, (list, tuple)):
        return [settings(x) for x in v]
    if isinstance(v, np.ndarray):
        return v.tolist()
    if isinstance(v, (np.floating, np.integer, np.bool_)):
        return v.item()
    return v


def impact_case(case) -> tuple[list, dict, dict, set]:
    """An impact case's layers and settings (its project read), and landschaft's impacts of it."""
    with tempfile.TemporaryDirectory() as tmp, warnings.catch_warnings():
        warnings.simplefilter("ignore")
        tmp = Path(tmp)
        write_project(case, tmp)
        kw = read_project(tmp / "settings.z5", case.flags, mode=case.mode)
        kw.pop("postprocessing_file", None)
        features = kw.pop("features")
        res = zonation(features, **kw)
        params = settings(kw)
        used = {stem(f) for f in features}
        for sc in (params.get("impact"), params.get("cumulative_impact")):
            if sc:
                used |= {sc.get("sources"), sc.get("subareas")} | set(sc.get("direct_rasters", []))
                used |= {p["raster"] for p in sc.get("pressures", [])}
        used.discard(None)
        tables = {k: {"columns": list(df.columns), "rows": df.to_numpy(object).tolist()}
                  for k, df in res.impacts.items() if k != "rasters"}
        return [stem(f) for f in features], params, tables, used


def main() -> None:
    cases = {name: c for name, c in pc.CASES.items() if c.group in GROUPS}
    used = set()
    meta, impacts = {}, {}
    for name, c in cases.items():
        if c.group == "impact":
            layers, params, tables, rasters = impact_case(c)
            meta[name] = {"group": c.group, "layers": layers, "params": params}
            impacts[name] = tables
            used |= rasters
            continue
        used |= set(c.layers) | rasters_in(c.params)
        meta[name] = {"group": c.group, "layers": c.layers, "params": settings(c.params)}
        if c.files and "pp.txt" in c.files:
            calls = parse_calls(c.files["pp.txt"])
            used |= {Path(p).stem for _, args in calls for p in args if isinstance(p, str) and Path(p).stem in pc.RASTERS}
            meta[name]["post"] = [[f, args] for f, args in calls]
    used = sorted(used)
    out = ROOT / "tests" / "data" / "reference"
    np.savez_compressed(out / "zonation_inputs.npz", **{r: file_values(pc.RASTERS[r]) for r in used})
    # the files' NoData values (null: NaN or none), which value extraction tells from no value
    meta = {"cases": meta, "nodata": {r: (None if pc.RASTERS[r].nodata is None or np.isnan(pc.RASTERS[r].nodata)
                                          else float(pc.RASTERS[r].nodata)) for r in used}}
    (out / "zonation_inputs.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    # Zonation's post-processing files (of its LOAD run of the ranking: the curves of the ranking returned), kept
    # lzma-compressed by landschaft: their lines split into words
    post = {}
    with np.load(REF / "post.npz") as z:
        for name, c in cases.items():
            if c.group != "post":
                continue
            prefix = f"{name}/load/post/"
            post[name] = {}
            for k in z.files:
                file = k[len(prefix):]
                if not k.startswith(prefix) or file.endswith(".tif") or file.startswith("highest"):
                    continue
                lines = [ln.split() for ln in lzma.decompress(z[k].tobytes()).decode().splitlines()]
                # the comparison's first line names the file compared (a path of the machine that ran Zonation)
                post[name][file] = lines[1:] if file.startswith("solution_comparison") else lines
    (out / "zonation_post.json").write_text(json.dumps(post, indent=0) + "\n", encoding="utf-8")
    (out / "zonation_impacts.json").write_text(json.dumps(impacts, indent=0) + "\n", encoding="utf-8")
    print(f"{len(cases)} cases, rasters {used}")


if __name__ == "__main__":
    main()
