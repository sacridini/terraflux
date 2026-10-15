"""Shared pieces of the figures of the guide "Parallel and distributed computing" (content/parallel).

The data are Sibuyan Island's maps (tests/data/inputs: land use in 1997 and 2011, 13 drivers, 108 x 128 cells of
250 m; 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other, 255 no data), and the same maps with every cell cut into
k x k cells (`enlarged(k)`), to time models of a size that takes seconds rather than a fraction of one.

The models are those `terraflux new dinamica` writes (Dinamica EGO's classic model fitted on 1997-2011, 14 steps),
and the same with the Expander and Patcher's settings given (`patches.settings`) instead of measured, so that the
tiled engine gives the same maps whatever the tiling.

Images go to website/static/img/guide/parallel, chart data to website/static/data/guide/parallel.
"""

from __future__ import annotations

import pathlib
import re
import shutil
import subprocess
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import figs  # noqa: E402
import numpy as np  # noqa: E402

INPUTS = figs.ROOT / "tests" / "data" / "inputs"
TF = figs.binary()
IMG = figs.WEB / "static" / "img" / "guide" / "parallel"
DATA = figs.WEB / "static" / "data" / "guide" / "parallel"
IMG.mkdir(parents=True, exist_ok=True)
DATA.mkdir(parents=True, exist_ok=True)

SIB = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Rice", "#4f9fc4"),
       5: ("Other", "#b4508a")}


def work(name: str, clean: bool = False) -> pathlib.Path:
    return figs.workdir(f"parallel/{name}", clean=clean)


def tf(*args: str, cwd: pathlib.Path, check: bool = True) -> tuple[float, str]:
    """Runs terraflux in `cwd`: (seconds of wall time, its output)."""
    t = time.perf_counter()
    r = subprocess.run([str(TF), *args], cwd=cwd, capture_output=True, text=True, encoding="utf-8")
    s = time.perf_counter() - t
    out = (r.stdout + r.stderr).replace("terraflux.exe", "terraflux")
    if check and r.returncode != 0:
        raise SystemExit(f"terraflux {' '.join(args)} failed:\n{out}")
    return s, out


def enlarged(w: pathlib.Path, k: int) -> str:
    """Sibuyan's maps with every cell cut into k x k cells, written in `w` as big{k}_landuse.tif and
    big{k}_drivers.tif (cells of 250/k m): returns the prefix of their names."""
    import rasterio
    from rasterio.transform import Affine

    prefix = f"big{k}" if k > 1 else "sibuyan"
    for name in ("landuse", "drivers"):
        src = INPUTS / f"sibuyan_{name}.tif"
        dst = w / f"{prefix}_{name}.tif"
        if k == 1:
            shutil.copy(src, dst)
            continue
        if dst.exists():
            continue
        with rasterio.open(src) as ds:
            a = ds.read()
            prof = ds.profile
            desc = ds.descriptions
            t = ds.transform
        a = np.repeat(np.repeat(a, k, axis=1), k, axis=2)
        prof.update(width=a.shape[2], height=a.shape[1], transform=Affine(t.a / k, t.b, t.c, t.d, t.e / k, t.f),
                    compress="deflate", tiled=True, blockxsize=256, blockysize=256)
        with rasterio.open(dst, "w", **prof) as out:
            out.write(a)
            for i, d in enumerate(desc, 1):
                out.set_band_description(i, d)
    return prefix


def dinamica(w: pathlib.Path, k: int = 1, name: str | None = None) -> pathlib.Path:
    """`terraflux new dinamica` on Sibuyan (enlarged k times): the model file."""
    p = enlarged(w, k)
    f = w / (name or f"{p}.toml")
    tf("new", "dinamica", "--t0", f"{p}_landuse.tif:1997", "--t1", f"{p}_landuse.tif:2011", "--drivers",
       f"{p}_drivers.tif", "--steps", "14", "-o", f.name, cwd=w)
    return f


TRANSITIONS = [(1, 2), (1, 3), (1, 4), (1, 5), (2, 3), (3, 2), (3, 4), (3, 5), (4, 2), (4, 3)]


def given_patches(model: pathlib.Path, out: pathlib.Path) -> pathlib.Path:
    """The model with the Expander and Patcher's settings given (patches.settings) rather than measured."""
    t = model.read_text(encoding="utf-8")
    tr = ", ".join(f"{{ from = {a}, to = {b}, expansion = 0.5, expander = [4.0, 4.0, 1.5], patcher = [4.0, 4.0, 1.5] }}"
                   for a, b in TRANSITIONS)
    node = ('[[nodes]]\nid = "patches"\nop = "patches.settings"\nnote = "The Expander and Patcher\'s settings '
            'given, the same for every transition."\n\n[nodes.params]\ntransitions = [' + tr + ']\n\n[[nodes]]\n'
            'id = "simulation"')
    t = re.sub(r'\[\[nodes\]\]\nid = "patches".*?\n\[\[nodes\]\]\nid = "simulation"', lambda m: node, t, flags=re.S)
    out.write_text(t, encoding="utf-8")
    return out
