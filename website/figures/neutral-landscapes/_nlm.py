"""Shared pieces of the figures of the neutral landscape tutorials: running models and writing their nodes."""

from __future__ import annotations

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402,F401

ROOT = figs.ROOT
AUGUSTA = ROOT / "extern/landschaft/tests/data/lsm/augusta.tif"
HEAD = 'format = "terraflux/1"\n'
HAB = {1: ("Matrix", "#ece6d6"), 2: ("Habitat", figs.FOREST)}


def work(name: str) -> pathlib.Path:
    w = figs.workdir(f"neutral-landscapes/{name}")
    shutil.copy(AUGUSTA, w / "augusta.tif")
    return w


def _val(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return json.dumps(v)
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(_val(x) for x in v) + "]"
    return repr(v)


def node(id: str, op: str, inputs: dict | None = None, params: dict | None = None) -> str:
    s = f'\n[[nodes]]\nid = "{id}"\nop = "{op}"\n'
    if inputs:
        s += "inputs = { " + ", ".join(f'{k} = "{v}"' for k, v in inputs.items()) + " }\n"
    if params:
        s += "params = { " + ", ".join(f"{k} = {_val(v)}" for k, v in params.items()) + " }\n"
    return s


def run(model: str, w: pathlib.Path, writes: dict[str, str], name="model.toml"):
    figs.run(figs.add_writes(model, writes), w, name=name)


def table(w: pathlib.Path, name: str) -> list[dict]:
    out = []
    for r in figs.read_csv(w / "output" / f"{name}.csv"):
        o = {}
        for k, v in r.items():
            try:
                o[k] = float(v)
            except ValueError:
                o[k] = float("nan") if v in ("", "NA", "NaN", "nan") else v
        out.append(o)
    return out
