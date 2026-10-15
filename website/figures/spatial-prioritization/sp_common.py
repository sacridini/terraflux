"""Shared pieces of the figure scripts of the spatial prioritization tutorials.

The tutorials use Serra Verde, the synthetic planning region shipped with landschaft
(extern/landschaft/src/landschaft/prioritize/data): 60 x 90 cells of 1 km, eight features, a cost per cell,
protected areas, cities, threats. Every script copies these rasters into its work folder, runs Terraflux models
on them and draws what they write.
"""

from __future__ import annotations

import csv
import json
import pathlib
import shutil
import sys
import zipfile

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))
import figs  # noqa: E402

import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

DATA = figs.ROOT / "extern" / "landschaft" / "src" / "landschaft" / "prioritize" / "data"
CATEGORY = "spatial-prioritization"

FEATURES = ["jaguar", "lowland_tapir", "harpy_eagle", "maned_wolf", "golden_lion_tamarin", "neotropical_otter",
            "araucaria_forest", "mangrove"]
NAMES = {"jaguar": "Jaguar", "lowland_tapir": "Lowland tapir", "harpy_eagle": "Harpy eagle",
         "maned_wolf": "Maned wolf", "golden_lion_tamarin": "Golden lion tamarin",
         "neotropical_otter": "Neotropical otter", "araucaria_forest": "Araucaria forest", "mangrove": "Mangrove"}
SHORT = {"jaguar": "Jaguar", "lowland_tapir": "Tapir", "harpy_eagle": "Harpy eagle", "maned_wolf": "Maned wolf",
         "golden_lion_tamarin": "Tamarin", "neotropical_otter": "Otter", "araucaria_forest": "Araucaria",
         "mangrove": "Mangrove"}
# a colour per feature (the application's class colours)
FCOL = {f: figs.CLASSES[i] for i, f in enumerate(FEATURES)}

SELECTED = "#2e7d4f"
UNSELECTED = "#e7e3d7"
PROTECTED = "#785ef0"
URBAN = "#b4508a"
OCEAN = (1, 1, 1, 0)

HEAD = '''format = "terraflux/1"

[[nodes]]
id = "features"
op = "raster.read"
params = { path = "features.tif" }

[[nodes]]
id = "cost"
op = "raster.read"
params = { path = "cost.tif" }
'''


def read_node(id_: str, path: str) -> str:
    return f'\n[[nodes]]\nid = "{id_}"\nop = "raster.read"\nparams = {{ path = "{path}" }}\n'


def work(slug: str, name: str) -> pathlib.Path:
    """A clean work folder with Serra Verde's rasters in it."""
    w = figs.workdir(f"{CATEGORY}/{slug}/{name}")
    for f in DATA.glob("*.tif"):
        shutil.copy(f, w / f.name)
    return w


def run(slug: str, name: str, body: str, writes: dict | None = None, head: str = HEAD, quiet=True) -> pathlib.Path:
    """Runs HEAD + body in a work folder of its own; returns the folder (outputs in work/output)."""
    w = work(slug, name)
    model = head + body
    if writes:
        model = figs.add_writes(model, writes)
    figs.run(model, w, *(["-q"] if quiet else []))
    return w


def table(w: pathlib.Path, name: str) -> list[dict]:
    rows = figs.read_csv(w / "output" / f"{name}.csv")
    out = []
    for r in rows:
        o = {}
        for k, v in r.items():
            try:
                o[k] = float(v)
            except (TypeError, ValueError):
                o[k] = v
        out.append(o)
    return out


def number(w: pathlib.Path, name: str) -> float:
    return float(json.loads((w / "output" / f"{name}.json").read_text(encoding="utf-8")))


def grid(path: pathlib.Path, band: int = 1) -> np.ndarray:
    a, _ = figs.read(pathlib.Path(path), band)
    return a.filled(np.nan)


def land() -> np.ndarray:
    """True where Serra Verde has land (a cost: a planning unit)."""
    return np.isfinite(grid(DATA / "cost.tif"))


def features() -> dict[str, np.ndarray]:
    return {f: grid(DATA / "features.tif", i + 1) for i, f in enumerate(FEATURES)}


def img(slug: str) -> pathlib.Path:
    return figs.img_dir(f"{CATEGORY}/{slug}")


def data(slug: str) -> pathlib.Path:
    return figs.data_dir(f"{CATEGORY}/{slug}")


def save(fig, png: pathlib.Path):
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.08)
    plt.close(fig)
    figs._shrink(png)
    return png


def rgba_of(sel: np.ndarray, mask: np.ndarray | None = None, on=SELECTED, off=UNSELECTED,
            extra: list[tuple[np.ndarray, str]] = ()) -> np.ndarray:
    """An RGBA image: cells with sel > 0.5 in `on`, other land in `off`, no land transparent; `extra` layers
    (boolean arrays, colour) painted over in order."""
    mask = land() if mask is None else mask
    out = np.zeros(sel.shape + (4,))
    out[mask] = mcolors.to_rgba(off)
    out[mask & (np.nan_to_num(sel) > 0.5)] = mcolors.to_rgba(on)
    for m, c in extra:
        out[m & mask] = mcolors.to_rgba(c)
    return out


def plan_map(sel: np.ndarray, png: pathlib.Path, title: str | None = None, legend: list[tuple[str, str]] | None = None,
             extra: list[tuple[np.ndarray, str]] = (), width: int = 900, note: str | None = None, on=SELECTED):
    """A solution drawn on Serra Verde: selected cells, other land, the sea white."""
    legend = legend if legend is not None else [("Selected", on), ("Not selected", UNSELECTED)]
    w_in = width / 150
    fig = plt.figure(figsize=(w_in, w_in * 0.72 * 60 / 90 * 1.08 + (0.3 if title else 0.05)), dpi=150)
    ax = fig.add_axes([0, 0, 0.72 if legend else 1, 0.9 if title else 1])
    ax.set_axis_off()
    ax.imshow(rgba_of(sel, extra=extra, on=on), interpolation="nearest")
    if legend:
        fig.legend(handles=[Patch(facecolor=c, edgecolor="#00000022", label=l) for l, c in legend],
                   loc="center left", bbox_to_anchor=(0.73, 0.5), fontsize=9.5, handlelength=1.1, handleheight=1.1,
                   borderaxespad=0, labelspacing=0.55)
    if title:
        fig.text(0.0, 0.985, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    if note:
        fig.text(0.0, -0.02, note, ha="left", va="top", fontsize=9, color=figs.INK2)
    return save(fig, png)


def panels(arrays: list[np.ndarray], titles: list[str], png: pathlib.Path, cols: int = 4, cmap="viridis",
           vmin=None, vmax=None, width: int = 1000, kinds: list | None = None, colorbar: str | None = None,
           suptitle: str | None = None):
    """Several maps of Serra Verde in a grid of small panels. kinds[i] = 'plan' draws arrays[i] as a selection
    (or an RGBA image if it has 4 bands); else a colour ramp shared by all."""
    n = len(arrays)
    rows = (n + cols - 1) // cols
    w_in = width / 150
    ph = w_in / cols * 60 / 90
    fig, axs = plt.subplots(rows, cols, figsize=(w_in, rows * (ph + 0.32) + (0.35 if colorbar else 0)), dpi=150,
                            squeeze=False)
    mask = land()
    im = None
    for i, ax in enumerate(axs.flat):
        ax.set_axis_off()
        if i >= n:
            continue
        a = arrays[i]
        k = (kinds or [None] * n)[i]
        if a.ndim == 3:
            ax.imshow(a, interpolation="nearest")
        elif k == "plan":
            ax.imshow(rgba_of(a, mask), interpolation="nearest")
        else:
            cm = plt.get_cmap(cmap).copy()
            cm.set_bad((1, 1, 1, 0))
            im = ax.imshow(np.ma.masked_invalid(np.where(mask, a, np.nan)), cmap=cm, vmin=vmin, vmax=vmax,
                           interpolation="nearest")
        ax.set_title(titles[i], fontsize=10, loc="left", color=figs.INK, fontweight="bold", pad=3)
    if suptitle:
        fig.suptitle(suptitle, x=0.01, ha="left", fontsize=11.5, fontweight="bold", color=figs.INK)
    fig.tight_layout(h_pad=0.6, w_pad=0.4)
    if colorbar and im is not None:
        fig.subplots_adjust(bottom=0.14)
        cax = fig.add_axes([0.3, 0.05, 0.4, 0.025])
        cb = fig.colorbar(im, cax=cax, orientation="horizontal")
        cb.outline.set_visible(False)
        cb.ax.grid(False)
        cb.ax.tick_params(labelsize=8.5)
        cb.set_label(colorbar, fontsize=9, color=figs.INK2)
    return save(fig, png)


def write_csv(path: pathlib.Path, rows: list[dict]):
    figs.write_csv(path, rows)


def zip_data(slug: str, name: str, files: dict[str, pathlib.Path | str]):
    """A zip of files for readers to download: {name in the zip: source path or text}."""
    z = data(slug) / name
    with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as zf:
        for arc, src in files.items():
            if isinstance(src, pathlib.Path):
                zf.write(src, arc)
            else:
                zf.writestr(arc, src)
    return z


def serra_verde_zip(slug: str, layers=("features", "cost", "protected", "urban", "condition", "threats")):
    files = {f"serra-verde/{l}.tif": DATA / f"{l}.tif" for l in layers}
    files["serra-verde/README.txt"] = (
        "Serra Verde: a synthetic planning region of 60 x 90 cells of 1 km (EPSG:31983), from landschaft's\n"
        "example data. The sea has no value (it is not part of the problem).\n\n"
        "features.tif   8 bands, the habitat suitability (0-1) of: " + ", ".join(FEATURES) + "\n"
        "cost.tif       the cost of every cell (thousand USD)\n"
        "protected.tif  1 in the three existing protected areas\n"
        "urban.tif      1 in the three cities\n"
        "condition.tif  the condition of the habitat (0-1)\n"
        "threats.tif    2 bands, the intensity (0-1) of deforestation and fire\n")
    return zip_data(slug, "serra-verde.zip", files)


def fmt_money(v: float) -> str:
    """Thousand USD as millions."""
    return f"{v / 1000:.1f} M"
