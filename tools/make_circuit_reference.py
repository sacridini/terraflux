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


def circuitscape_cases() -> None:
    """Circuitscape's pairwise and one-to-all (all-to-one) modes on rasters made here (lognormal resistances,
    focal points with a region of two cells, short-circuit polygons, a mask), with landschaft one thread (batches
    of 4 problems), written to tests/data/reference/circuitscape_landschaft.npz."""
    rng = np.random.default_rng(11)
    out: dict[str, np.ndarray] = {}
    cases = {}
    for name, shape, regions, polygons, mask in [("plain", (30, 40), False, False, False),
                                                  ("regions", (25, 35), True, False, False),
                                                  ("polygons", (30, 30), False, True, True)]:
        r = np.exp(rng.normal(0.5, 0.8, shape))
        r[rng.random(shape) < 0.04] = np.nan
        pts = np.zeros(shape)
        ny, nx = shape
        for k, (y, x) in enumerate([(2, 3), (ny - 3, 4), (ny // 2, nx - 3), (3, nx - 4), (ny - 2, nx // 2), (ny // 2, nx // 2)]):
            pts[y, x] = k + 1
            r[y, x] = 1.0
        if regions:
            pts[3, 3] = 1
            r[3, 3] = 1.0
        poly = None
        if polygons:
            poly = np.zeros(shape)
            poly[10:13, 10:14] = 1
            poly[20:22, 5:7] = 2
        m = None
        if mask:
            m = np.ones(shape)
            m[:, 0] = 0
        out[f"{name}/habitat"] = r
        out[f"{name}/points"] = pts
        if poly is not None:
            out[f"{name}/polygons"] = poly
        if m is not None:
            out[f"{name}/mask"] = m
        kw = {"polygons": poly, "mask": m, "n_threads": 1}
        p = ls.circuit.circuitscape_pairwise(r, pts, cum_map=True, max_map=True, **kw)
        out[f"{name}/pairwise/resistances"] = p.resistances
        out[f"{name}/pairwise/cum"] = p.cum_current
        out[f"{name}/pairwise/max"] = p.max_current
        strengths = np.array([[k + 1, 1.0 + 0.5 * k] for k in range(6)])
        for mode, all_to_one in (("one_to_all", False), ("all_to_one", True)):
            # (strengths go by position: not with focal regions, where landschaft, as Circuitscape, fails)
            for st in (None,) if regions else (None, strengths):
                key = f"{name}/{mode}" + ("_strengths" if st is not None else "")
                o = ls.circuit.circuitscape_one_to_all(r, pts, all_to_one=all_to_one, strengths=st, cum_map=True,
                                                       max_map=True, set_focal_node_currents_to_zero=True, **kw)
                out[f"{key}/resistances"] = o.resistances
                out[f"{key}/cum"] = o.cum_current
                out[f"{key}/max"] = o.max_current
        out[f"{name}/strengths"] = strengths
        cases[name] = {"polygons": poly is not None, "mask": m is not None}
    np.savez(REF / "circuitscape_landschaft.npz", **out)
    print(REF / "circuitscape_landschaft.npz", len(out))


PAIRS = {
    "include_list": "mode include\n1 2\n1 3\n2 5\n4 6\n1 6\n",
    "exclude_list": "mode exclude\n1 2\n3 4\n",
    "include_matrix": "min 1\nmax 5\n0 1 2 3 4 5 6\n1 0 1 0 2 9 1\n2 1 0 3 0 0 0\n3 0 3 0 1 0 0\n"
                      "4 2 0 1 0 0 1\n5 9 0 0 0 0 1\n6 1 0 0 1 1 0\n",
}


def pairs_and_networks() -> None:
    """Included pairs on rasters (pairwise, one-to-all and all-to-one, with and without focal regions) and
    Circuitscape's network test cases (landschaft's tests/data/reference/circuit/circuitscape/input/network*),
    written to tests/data/reference/circuit_networks.npz and .json (and the pairs files)."""
    from landschaft.circuit.ini import parse_ini

    z = np.load(REF / "circuitscape_landschaft.npz")
    out: dict[str, np.ndarray] = {}
    meta: dict = {"pairs": {}, "networks": []}
    for key, text in PAIRS.items():
        path = REF / f"circuit_pairs_{key}.txt"
        path.write_text(text, encoding="utf-8")
        for name in ("plain", "regions"):
            r, pts = z[f"{name}/habitat"], z[f"{name}/points"]
            p = ls.circuit.circuitscape_pairwise(r, pts, cum_map=True, included_pairs=str(path), n_threads=1)
            out[f"{key}/{name}/pairwise/resistances"] = p.resistances
            out[f"{key}/{name}/pairwise/cum"] = p.cum_current
            for mode, all_to_one in (("one_to_all", False), ("all_to_one", True)):
                o = ls.circuit.circuitscape_one_to_all(r, pts, all_to_one=all_to_one, cum_map=True, max_map=True,
                                                       included_pairs=str(path), n_threads=1)
                out[f"{key}/{name}/{mode}/resistances"] = o.resistances
                out[f"{key}/{name}/{mode}/cum"] = o.cum_current
                out[f"{key}/{name}/{mode}/max"] = o.max_current
    cs = LS / "circuitscape"
    for ini in sorted(p for d in ("network", "network_extra") for p in (cs / "input" / d).glob("*.ini")):
        c = parse_ini(ini)
        rel = lambda k: None if c.get(k) in (None, "None", "") or str(c.get(k)).startswith("(") else c[k]
        f = lambda k: None if rel(k) is None else str(cs / rel(k))
        t = lambda k: str(c.get(k)).lower() == "true"
        name = ini.stem
        case = {"name": name, "scenario": c["scenario"], "graph": rel("habitat_file"),
                "graph_is_resistance": t("habitat_map_is_resistances")}
        if c["scenario"] == "advanced":
            case.update(sources=rel("source_file"), grounds=rel("ground_file"),
                        ground_is_resistance=t("ground_file_is_resistances"), policy=c.get("remove_src_or_gnd", "keepall"))
            r = ls.circuit.circuitscape_network_advanced(f("habitat_file"), f("source_file"), f("ground_file"),
                                                         graph_is_resistance=case["graph_is_resistance"],
                                                         ground_is_resistance=case["ground_is_resistance"],
                                                         remove_src_or_gnd=case["policy"], n_threads=1)
            out[f"net/{name}/voltages"] = r.voltages
            out[f"net/{name}/node_currents"] = r.node_currents
            out[f"net/{name}/branch_currents"] = r.branch_currents
        else:
            pairs = f("included_pairs_file") if t("use_included_pairs") else None
            case.update(points=rel("point_file"), included_pairs=rel("included_pairs_file") if pairs else None,
                        voltage_maps=t("write_volt_maps"),
                        current_maps=t("write_cur_maps"))
            r = ls.circuit.circuitscape_network_pairwise(f("habitat_file"), f("point_file"),
                                                         graph_is_resistance=case["graph_is_resistance"],
                                                         included_pairs=pairs, voltage_maps=case["voltage_maps"],
                                                         current_maps=case["current_maps"], n_threads=1)
            out[f"net/{name}/resistances"] = r.resistances
            if r.cum_node_currents is not None:
                out[f"net/{name}/cum_node_currents"] = r.cum_node_currents
                out[f"net/{name}/cum_branch_currents"] = r.cum_branch_currents
            case["maps"] = sorted("/".join(map(str, k)) for k in r.maps)
            for k, v in r.maps.items():
                out[f"net/{name}/maps/" + "/".join(map(str, k))] = v
        meta["networks"].append(case)
    np.savez_compressed(REF / "circuit_networks.npz", **out)
    (REF / "circuit_networks.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "circuit_networks.npz", len(out))


def ini_cases() -> None:
    """Circuitscape's raster test INI files (run from its folder, landschaft one thread) and the Omniscape INI
    files of landschaft's references, without writing outputs: tests/data/reference/circuit_ini.npz and .json."""
    import warnings

    cs = LS / "circuitscape"
    out: dict[str, np.ndarray] = {}
    meta: dict = {"circuitscape": [], "omniscape": []}
    here = os.getcwd()
    os.chdir(cs)
    try:
        for ini in sorted((cs / "input" / "raster").rglob("*.ini")):
            rel = ini.relative_to(cs).as_posix()
            try:
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    r = ls.circuit.circuitscape_ini(rel, write_outputs=False, n_threads=1)
            except Exception as e:  # noqa: BLE001 - the error is part of the reference
                meta["circuitscape"].append({"ini": rel, "error": f"{type(e).__name__}: {e}"})
                continue
            keys = []
            for k in ("voltage", "current", "resistances", "cum_current", "max_current"):
                v = getattr(r, k)
                if v is not None:
                    out[f"cs/{rel}/{k}"] = np.asarray(v, dtype=np.float64)
                    keys.append(k)
            maps = sorted(k for k in r.maps if isinstance(k, str) and k != "solved")
            for k in maps:
                out[f"cs/{rel}/maps/{k}"] = np.asarray(r.maps[k], dtype=np.float64)
            meta["circuitscape"].append({"ini": rel, "keys": keys, "maps": maps})
    finally:
        os.chdir(here)
    for ini in sorted((LS / "omniscape_ini").glob("*.ini")):
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                ds = ls.circuit.omniscape_ini(ini, write_outputs=False, n_threads=1)
        except Exception as e:  # noqa: BLE001
            meta["omniscape"].append({"ini": ini.name, "error": f"{type(e).__name__}: {e}"})
            continue
        keys = [k for k in ("cum_currmap", "flow_potential", "normalized_cum_currmap", "classified_resistance")
                if k in ds]
        for k in keys:
            out[f"om/{ini.name}/{k}"] = np.asarray(ds[k].values, dtype=np.float64)
        meta["omniscape"].append({"ini": ini.name, "keys": keys})
    np.savez_compressed(REF / "circuit_ini.npz", **out)
    (REF / "circuit_ini.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "circuit_ini.npz", len(out))


INI_OUTPUTS = ["input/raster/pairwise/1/sgVerify1.ini", "input/raster/advanced/1/mgVerify1.ini",
               "input/raster/one_to_all/1/oneToAllVerify1.ini"]


def ini_outputs() -> None:
    """The files landschaft writes for three Circuitscape INI files (their copies in
    tests/data/reference/circuit_ini_out, inputs from Circuitscape's folder, outputs next to them)."""
    import shutil

    cs = LS / "circuitscape"
    dest = REF / "circuit_ini_out"
    shutil.rmtree(dest, ignore_errors=True)
    dest.mkdir()
    here = os.getcwd()
    os.chdir(cs)
    try:
        for rel in INI_OUTPUTS:
            name = Path(rel).stem
            text = (cs / rel).read_text(encoding="utf-8")
            lines = []
            for line in text.splitlines():
                if line.startswith("output_file"):
                    line = f"output_file = {(dest / name / name).as_posix()}.out"
                lines.append(line)
            ini = dest / f"{name}.ini"
            ini.write_text("\n".join(lines) + "\n", encoding="utf-8")
            ls.circuit.circuitscape_ini(ini, n_threads=1)
    finally:
        os.chdir(here)
    print(dest, sorted(p.name for p in dest.rglob("*") if p.is_file()))


if __name__ == "__main__":
    circuitscape_cases()
    pairs_and_networks()
    ini_cases()
    ini_outputs()
