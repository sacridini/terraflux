"""What the figures of the series "Demand-driven allocation: CLUE-S and lulcc" share: lulcc's Plum Island
Ecosystems data, its classes and colours, and the CLUE-S model that `terraflux new clue` writes for it.

The scripts clue_*.py import it:

    from clue_common import *          # figs, PIE, CLASSES, pie_work, new_model, ...
"""

from __future__ import annotations

import json
import pathlib
import shutil
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402

PIE = figs.ROOT / "tests" / "data" / "inputs"
# lulcc's classes of Plum Island Ecosystems
CLASSES = {1: ("Forest", "#3b8a5a"), 2: ("Built", "#b4508a"), 3: ("Other", "#d9a441")}
NAMES = {k: v[0] for k, v in CLASSES.items()}
COLORS = {v[0]: v[1] for v in CLASSES.values()}
YEARS = [1985, 1991, 1999]
# lulcc's vignette: the regressions, the elasticities and the CLUE settings
FORMULAS = {"1": ["elevation", "slope"], "2": ["elevation", "slope", "dist_built_1985"], "3": ["elevation", "slope"]}
SETTINGS = {"jitter": 0.0002, "scale": 0.000001, "max_iter": 1000, "max_diff": 50, "mean_diff": 50}
ELASTICITY = "0.2,0.2,0.2"


# the regressions of the vignette and the suitability they give (the first nodes of the CLUE-S model)
SUITABILITY = """format = "terraflux/1"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = { landscape = "$start", drivers = "$drivers" }
params = { formulas = { 1 = ["elevation", "slope"], 2 = ["elevation", "slope", "dist_built_1985"], 3 = ["elevation", "slope"] } }

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "$drivers", landscape = "$start" }
"""


def pie_work(name: str) -> pathlib.Path:
    """A clean work folder with lulcc's Plum Island Ecosystems maps in it."""
    w = figs.workdir(name)
    for f in ("pie_landuse.tif", "pie_drivers.tif"):
        shutil.copy(PIE / f, w / f)
    return w


def new_model(work: pathlib.Path, kind: str = "clue", *, formulas=FORMULAS, elasticity: str | None = ELASTICITY,
              settings: dict | None = SETTINGS, extra: tuple = (), name="model.toml") -> str:
    """The model `terraflux new clue` (or `ordered`) writes for Plum Island, its start band named by its year."""
    args = [str(figs.binary()), "new", kind, "--observed", "pie_landuse.tif", "--times", "1985,1991,1999",
            "--drivers", "pie_drivers.tif", "--formulas", json.dumps(formulas)]
    if kind == "clue":
        if elasticity is not None:
            args += ["--elasticity", elasticity]
        if settings is not None:
            args += ["--settings", json.dumps(settings)]
    args += list(extra) + ["-o", name]
    r = subprocess.run(args, cwd=work, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise SystemExit(f"terraflux new failed:\n{r.stdout}\n{r.stderr}")
    text = (work / name).read_text(encoding="utf-8")
    return text.replace('band = "0"', 'band = "1985"')


def panels(arrays, titles, png, pal=None, width=1100, legend=True, ncols=None):
    """Several class maps side by side with one legend (the same colours in all)."""
    import matplotlib.pyplot as plt
    import numpy as np
    from matplotlib import colors as mcolors
    from matplotlib.patches import Patch

    pal = pal or dict(CLASSES)
    n = len(arrays)
    ncols = ncols or n
    nrows = (n + ncols - 1) // ncols
    h, w = arrays[0].shape
    keys = sorted(pal)
    lut = np.zeros((len(keys) + 1, 4))
    for i, k in enumerate(keys):
        lut[i + 1] = mcolors.to_rgba(pal[k][1])
    fig_w = width / 150
    map_w = fig_w / ncols
    fig_h = nrows * map_w * h / w + (0.55 if legend else 0) + 0.3 * nrows
    fig, axes = plt.subplots(nrows, ncols, figsize=(fig_w, fig_h), dpi=150, squeeze=False)
    for ax in axes.flat:
        ax.set_axis_off()
    for ax, a, t in zip(axes.flat, arrays, titles):
        a = np.ma.masked_invalid(np.ma.asarray(a, dtype=float))
        idx = np.zeros(a.shape, dtype=int)
        f = a.filled(-9999)
        for i, k in enumerate(keys):
            idx[f == k] = i + 1
        rgba = lut[idx]
        rgba[idx == 0] = (1, 1, 1, 0)
        ax.imshow(rgba, interpolation="nearest")
        ax.set_title(t, fontsize=11, loc="left")
    if legend:
        handles = [Patch(facecolor=pal[k][1], edgecolor="#00000022", label=pal[k][0]) for k in keys]
        fig.legend(handles=handles, loc="lower center", ncol=len(keys), fontsize=9.5, handlelength=1.1,
                   handleheight=1.1, bbox_to_anchor=(0.5, 0.0))
    fig.subplots_adjust(left=0.01, right=0.99, top=0.93 if nrows == 1 else 0.96, bottom=0.1 if legend else 0.01,
                        wspace=0.03, hspace=0.12)
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)
    return png


def palette():
    return dict(CLASSES)


def class_cells(path: pathlib.Path, band: int = 1) -> dict[int, int]:
    a = figs.read(path, band)[0]
    return {k: int((a == k).sum()) for k in CLASSES}


def pie_map(png, src, band=1, title=None, legend=True, counts=False, width=760, array=None):
    return figs.raster(src, png, band=band, palette=palette(), title=title, legend=legend, counts=counts,
                       width=width, array=array)
