"""Shared bits of the figure scripts of the tutorials on distance and restoration."""

from __future__ import annotations

import pathlib

import numpy as np
import shutil
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))
import figs  # noqa: E402

ROOT = figs.ROOT
OREGON = ROOT / "extern/landschaft/src/landschaft/surface/data"
SERRA = ROOT / "extern/landschaft/src/landschaft/prioritize/data"

# Serra Verde's natural and anthropic classes, and which classes each species lives in
NATURAL = ["forest", "savanna", "mangrove"]
ANTHROPIC = ["pasture", "cropland"]
SPECIES = ["jaguar", "lowland_tapir", "harpy_eagle", "maned_wolf", "golden_lion_tamarin", "neotropical_otter",
           "araucaria_forest", "mangrove"]
NAMES = {"jaguar": "Jaguar", "lowland_tapir": "Lowland tapir", "harpy_eagle": "Harpy eagle",
         "maned_wolf": "Maned wolf", "golden_lion_tamarin": "Golden lion tamarin",
         "neotropical_otter": "Neotropical otter", "araucaria_forest": "Araucaria forest", "mangrove": "Mangrove"}
SUITABILITY = ('{ jaguar = ["forest", "savanna"], lowland_tapir = ["forest"], harpy_eagle = ["forest"], '
               'maned_wolf = ["savanna"], golden_lion_tamarin = ["forest"], neotropical_otter = ["forest", "mangrove"], '
               'araucaria_forest = ["forest"], mangrove = ["mangrove"] }')

FOREST = "#3b8a5a"
OPEN = "#e9e1c9"
MISSING = "#ffffff"
GTB = {0: ("Missing", "#f2f2f2"), 1: ("Open land", OPEN), 2: ("Forest", FOREST)}


def copy(work: pathlib.Path, *pairs: tuple[pathlib.Path, str]):
    """The data files a model reads, copied into its work folder under plain names."""
    for src, name in pairs:
        shutil.copy2(src, work / name)


def serra(work: pathlib.Path, *names: str):
    copy(work, *[(SERRA / f"{n}.tif", f"{n}.tif") for n in names])


def table(work: pathlib.Path, name: str) -> list[dict]:
    rows = figs.read_csv(work / "output" / f"{name}.csv")
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


def land_mask() -> "np.ndarray":
    """Serra Verde's cells on land (with a land cover)."""
    a = figs.read(SERRA / "land_cover.tif")[0]
    return ~np.ma.getmaskarray(a)


def share_map(arr, png, vmax=1.0, cmap="YlGn", unit="share of the cell restored", legend=True, width=700):
    """A plan's shares on Serra Verde: land without a share in pale grey, the ocean white, the shares on a ramp."""
    from matplotlib import colors as mcolors
    from matplotlib import pyplot as plt

    land = land_mask()
    a = np.ma.filled(np.ma.masked_invalid(np.ma.asarray(arr, dtype=float)), 0.0)
    fig = figs._fig_for(a.shape, width, legend)
    ax = fig.add_axes([0, 0, 0.72 if legend else 1.0, 1.0])
    ax.set_axis_off()
    base = np.zeros(a.shape + (4,))
    base[land] = mcolors.to_rgba("#e6e6e2")
    ax.imshow(base, interpolation="nearest")
    cm = plt.get_cmap(cmap).copy()
    cm.set_bad((1, 1, 1, 0))
    im = ax.imshow(np.ma.masked_where(~land | (a <= 1e-6), a), cmap=cm, vmin=0, vmax=vmax, interpolation="nearest")
    if legend:
        cax = fig.add_axes([0.76, 0.12, 0.035, 0.76])
        cb = fig.colorbar(im, cax=cax)
        cb.outline.set_visible(False)
        cb.ax.grid(False)
        cb.ax.tick_params(labelsize=9, color=figs.RULE)
        cb.set_label(unit, fontsize=9.5, color=figs.INK2)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.08)
    plt.close(fig)
    figs._shrink(png)
