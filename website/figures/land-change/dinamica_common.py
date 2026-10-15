"""Shared pieces of the figures of the land change series on Dinamica EGO, the foundations and validation
(foundations_*.py, dinamica_*.py, validation_*.py): the Sibuyan data, its classes and colours, the model that
`terraflux new dinamica` writes, and a few drawings.

Sibuyan Island (the Philippines), land use in 1997 and 2011 on cells of 250 m, and 13 drivers: the classic example
of CLUE-S (Verburg & Veldkamp 2004), shipped with Terraflux's tests (tests/data/inputs).
"""

from __future__ import annotations

import json
import pathlib
import re
import shutil
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))
import figs  # noqa: E402

import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

DATA = figs.ROOT / "tests" / "data" / "inputs"

# Sibuyan's classes (1997 and 2011)
SIB = {
    1: ("Forest", "#3b8a5a"),
    2: ("Coconut", "#d9a441"),
    3: ("Grassland", "#a7c957"),
    4: ("Rice", "#4f9fc4"),
    5: ("Other", "#b4508a"),
}
NAMES = {k: v[0] for k, v in SIB.items()}
UNCHANGED = "#e4e2dc"

# the drivers of Sibuyan, as `woe.fit` sorts them (the index of the `variable` column of its table)
DRIVERS = ["dist_sea", "pop_density", "diorite", "ultramafic", "sediments", "no_erosion", "moderate_erosion",
           "elevation", "slope", "aspect", "dist_roads", "dist_urban", "dist_streams"]
SORTED = sorted(DRIVERS)
CATEGORICAL = ["diorite", "ultramafic", "sediments", "no_erosion", "moderate_erosion"]
# ranges chosen for the drivers (map units: metres, people per km2, degrees)
RANGES = {"dist_sea": 250, "dist_roads": 250, "dist_urban": 250, "dist_streams": 250, "elevation": 50,
          "slope": 2, "aspect": 30, "pop_density": 50}


def sibuyan(name: str, clean=True, pie=False) -> pathlib.Path:
    """A work folder with the data: sibuyan_landuse.tif (1997, 2011) and sibuyan_drivers.tif (and PIE's, when
    asked), the names the pages use."""
    w = figs.workdir(f"land-change-a/{name}", clean=clean)
    shutil.copy(DATA / "sibuyan_landuse.tif", w / "sibuyan_landuse.tif")
    shutil.copy(DATA / "sibuyan_drivers.tif", w / "sibuyan_drivers.tif")
    if pie:
        shutil.copy(DATA / "pie_landuse.tif", w / "pie_landuse.tif")
        shutil.copy(DATA / "pie_drivers.tif", w / "pie_drivers.tif")
    return w


def cli(*args, cwd: pathlib.Path) -> str:
    r = subprocess.run([str(figs.binary()), *map(str, args)], cwd=cwd, capture_output=True, text=True,
                       encoding="utf-8")
    if r.returncode != 0:
        raise SystemExit(f"terraflux {' '.join(map(str, args))} failed:\n{r.stdout}\n{r.stderr}")
    return r.stdout + r.stderr


def new_dinamica(work: pathlib.Path, out="model.toml", *extra) -> str:
    """The model `terraflux new dinamica` writes for Sibuyan (14 yearly steps from 1997 to 2011)."""
    cli("new", "dinamica", "--t0", "sibuyan_landuse.tif:1997", "--t1", "sibuyan_landuse.tif:2011", "--drivers",
        "sibuyan_drivers.tif",
        "--steps", 14, *extra, "-o", out, cwd=work)
    return (work / out).read_text(encoding="utf-8")


def toml_value(v) -> str:
    """A Python value as TOML (inline)."""
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return json.dumps(v)
    if isinstance(v, (int, np.integer)):
        return str(int(v))
    if isinstance(v, (float, np.floating)):
        return repr(float(v))
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(toml_value(x) for x in v) + "]"
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{k if re.match(r'^[A-Za-z_][A-Za-z0-9_]*$', k) else json.dumps(k)} = {toml_value(x)}"
                                for k, x in v.items()) + " }"
    raise TypeError(v)


def set_params(model: str, node: str, params: dict) -> str:
    """The model with settings of a node set (a `[nodes.params]` table of the node added or extended)."""
    head = f'id = "{node}"'
    i = model.index(head)
    nxt = model.find("[[nodes", i + len(head))
    block = model[i: nxt if nxt >= 0 else len(model)]
    lines = "".join(f"{k} = {toml_value(v)}\n" for k, v in params.items())
    m = re.search(r"\[nodes(\.body\.nodes)?\.params\]\n", block)
    if m:
        # replace the settings of the same names, add the others
        body = block[m.end():]
        for k in params:
            body = re.sub(rf"^{re.escape(k)} = .*?(?=^\w|^\[|\Z)", "", body, flags=re.M | re.S)
        block = block[: m.end()] + lines + body
    else:
        # (top-level nodes only: a body node's block may run into its container's tables)
        block = block.rstrip("\n") + f"\n\n[nodes.params]\n{lines}\n"
    return model[:i] + block + (model[nxt:] if nxt >= 0 else "")


def woe_settings(model: str, ranges=None, categorical=None) -> str:
    return set_params(model, "woe", {"ranges": ranges or RANGES, "categorical": categorical or CATEGORICAL})


def outputs(model: str, outs: dict[str, str]) -> str:
    """Outputs added: {"name": "node.port"}; `terraflux run` writes them (tables as CSV, values as JSON)."""
    return model.rstrip() + "\n" + "".join(f'\n[outputs.{k}]\nfrom = "{v}"\n' for k, v in outs.items())


def run(model: str, work: pathlib.Path, *args, name="model.toml", out="output") -> pathlib.Path:
    f = work / name
    f.write_text(model, encoding="utf-8")
    cli("run", f, "--out", work / out, "-q", *args, cwd=work)
    return work / out


def table(path: pathlib.Path) -> list[dict]:
    rows = []
    for r in figs.read_csv(path):
        rows.append({k: (float(v) if re.match(r"^-?[\d.eE+-]+$|^NaN$|^nan$", v or "") else v) for k, v in r.items()
                     if k is not None and not k.startswith("#")})
    return [r for r in rows if not str(next(iter(r.values()), "")).startswith("#")]


def read_stack(path: pathlib.Path) -> np.ndarray:
    import rasterio

    with rasterio.open(path) as ds:
        a = ds.read(masked=True).astype("float64")
    return a.filled(np.nan)


def downloads(slug: str):
    """The data of Sibuyan offered for download beside a page."""
    d = figs.data_dir(slug)
    for f in ("sibuyan_landuse.tif", "sibuyan_drivers.tif"):
        shutil.copy(DATA / f, d / f)


def landuse() -> tuple[np.ndarray, np.ndarray]:
    a = read_stack(DATA / "sibuyan_landuse.tif")
    a[a == 255] = np.nan
    return a[0], a[1]


def palette(classes=SIB) -> dict:
    return {k: (n, c) for k, (n, c) in classes.items()}


def draw_map(arr: np.ndarray, png: pathlib.Path, title=None, legend=True, width=700, counts=False):
    return figs.raster(None, png, array=arr, palette=palette(), title=title, legend=legend, width=width,
                       counts=counts)


def draw_change(start: np.ndarray, end: np.ndarray, png: pathlib.Path, title=None, legend=True, width=700):
    """The cells that changed, in their new class, the others in light grey."""
    code = np.where(np.isnan(start) | np.isnan(end), np.nan, np.where(start != end, end, 0))
    pal = {0: ("Unchanged", UNCHANGED)}
    pal.update({k: (f"to {n.lower()}", c) for k, (n, c) in SIB.items()})
    return figs.raster(None, png, array=code, palette=pal, title=title, legend=legend, width=width)


def legend_strip(png: pathlib.Path, items: list[tuple[str, str]], width=900, ncol=None):
    """A legend alone, for images drawn without one (compare sliders)."""
    fig = plt.figure(figsize=(width / 150, 0.42), dpi=150)
    handles = [Patch(facecolor=c, edgecolor="#00000022", label=n) for n, c in items]
    fig.legend(handles=handles, loc="center", ncol=ncol or len(items), fontsize=9.5, handlelength=1.1,
               handleheight=1.1, columnspacing=1.4)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


def weights_rows(path: pathlib.Path, f: int, t: int, var: str) -> list[dict]:
    """The rows of `woe.fit`'s table (weights.csv) of one transition and variable."""
    k = SORTED.index(var)
    return [r for r in table(path) if int(r["from"]) == f and int(r["to"]) == t and int(r["variable"]) == k]


def weight_bars(ax, rows, xlabel="", categorical=False, unit=""):
    """W+ of each range in bars, the significant ones in the site's orange (as the application draws them)."""
    if not rows:
        return
    if categorical:
        xs = list(range(len(rows)))
        labels = [f"{r['lower']:g}" for r in rows]
        ax.bar(xs, [r["weight"] for r in rows], width=0.7,
               color=[figs.EMBER if r["significant"] else "#b9c0ba" for r in rows])
        ax.set_xticks(xs, labels)
    else:
        lo = [r["lower"] for r in rows]
        wd = [r["upper"] - r["lower"] for r in rows]
        ax.bar(lo, [r["weight"] for r in rows], width=wd, align="edge", edgecolor="white", linewidth=0.6,
               color=[figs.EMBER if r["significant"] else "#b9c0ba" for r in rows])
    ax.axhline(0, color=figs.INK2, linewidth=0.8)
    ax.set_ylabel("W+")
    if xlabel:
        ax.set_xlabel(xlabel + (f" ({unit})" if unit else ""))
