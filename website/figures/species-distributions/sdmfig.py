"""Shared pieces of the figures of the species distribution tutorials (website/figures/species-distributions).

The data are the classic example of Maxent and of R's dismo: 116 records of the brown-throated sloth (*Bradypus
variegatus*) and nine WorldClim bioclimatic variables at 0.5 degrees over Central and South America, as they travel
with landschaft (extern/landschaft/src/landschaft/sdm/data). The models are run with the terraflux binary; this
module writes the data where the models read them, builds the models' TOML and draws maps with points over them.
"""

from __future__ import annotations

import json
import pathlib
import shutil
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))
import figs  # noqa: E402

import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.lines import Line2D  # noqa: E402

CAT = "species-distributions"
DATA = figs.ROOT / "extern" / "landschaft" / "src" / "landschaft" / "sdm" / "data"
# the data a reader downloads: the records and the climate in the first part's folder
PUBLIC = figs.WEB / "static" / "data" / "tutorials" / CAT / "ideas"

PRES = "#d1495b"     # presences
BACK = "#6b7a72"     # background points
FLAG = "#e8901f"

BIOCLIM = {
    "bio1": "Annual mean temperature",
    "bio5": "Max temperature of the warmest month",
    "bio6": "Min temperature of the coldest month",
    "bio7": "Annual range of temperature (bio5 - bio6)",
    "bio8": "Mean temperature of the wettest quarter",
    "bio9": "Mean temperature of the driest quarter",
    "bio12": "Annual precipitation",
    "bio16": "Precipitation of the wettest quarter",
    "bio17": "Precipitation of the driest quarter",
}


def publish_data():
    """The records and the climate where readers download them."""
    PUBLIC.mkdir(parents=True, exist_ok=True)
    for f in ("bradypus.csv", "bioclim.tif"):
        if not (PUBLIC / f).exists():
            shutil.copy(DATA / f, PUBLIC / f)


def work(slug: str, files=("bradypus.csv", "bioclim.tif")) -> pathlib.Path:
    w = figs.workdir(f"{CAT}/{slug}")
    for f in files:
        shutil.copy(DATA / f, w / f)
    return w


# ------------------------------------------------------------------------------------------------ models

HEAD = 'format = "terraflux/1"\n'

READ = '''
[[nodes]]
id = "records"
op = "vector.read"
params = { path = "bradypus.csv" }

[[nodes]]
id = "climate"
op = "raster.read"
params = { path = "bioclim.tif" }
'''


def toml_value(v) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return json.dumps(v)
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{k} = {toml_value(x)}" for k, x in v.items()) + " }"
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(toml_value(x) for x in v) + "]"
    return repr(v)


def node(id: str, op: str, inputs: dict | None = None, params: dict | None = None) -> str:
    s = f'\n[[nodes]]\nid = "{id}"\nop = "{op}"\n'
    if inputs:
        s += f"inputs = {toml_value(inputs)}\n"
    if params:
        s += f"params = {toml_value(params)}\n"
    return s


def outputs(d: dict) -> str:
    return "".join(f'\n[outputs.{k}]\nfrom = "{v}"\n' for k, v in d.items())


CAREFUL = {"thin": 50000, "area": 500000, "background": 2000, "folds": {"size": 500000, "k": 5}, "seed": 1}


def prepared(**prep) -> str:
    """The records and climate read, and prepared by sdm.prepare (the settings given, else CAREFUL's)."""
    p = dict(CAREFUL)
    p.update(prep)
    p = {k: v for k, v in p.items() if v is not None}
    return HEAD + READ + node("data", "sdm.prepare", {"occurrences": "records.vector", "env": "climate.raster"}, p)


def run(model: str, w: pathlib.Path, name="model.toml"):
    return figs.run(model, w, name=name)


def out(w: pathlib.Path, name: str):
    p = w / "output" / name
    if p.suffix == ".json":
        return json.loads(p.read_text(encoding="utf-8"))
    if p.suffix == ".csv":
        return figs.read_csv(p)
    return p


def table(w: pathlib.Path, name: str) -> dict[str, np.ndarray]:
    return table_csv(w / "output" / name)


def table_csv(path) -> dict[str, np.ndarray]:
    rows = figs.read_csv(path)
    cols = rows[0].keys()
    res = {}
    for c in cols:
        try:
            res[c] = np.array([float(r[c]) if r[c] not in ("", "NA", "nan") else np.nan for r in rows])
        except ValueError:
            res[c] = np.array([r[c] for r in rows])
    return res


# ------------------------------------------------------------------------------------------------ drawing


def extent(info) -> list[float]:
    t = info["transform"]
    h, w = info["shape"]
    return [t.c, t.c + t.a * w, t.f + t.e * h, t.f]


def land(path=None):
    """The land of the climate map (cells with values), for a grey base."""
    a, info = figs.read(path or (DATA / "bioclim.tif"), 1)
    return ~np.ma.getmaskarray(np.ma.masked_invalid(a)), info


def map_axes(width=900, legend_frac=0.0, shape=(192, 186), title=None):
    dpi = 150
    w_in = width / dpi
    map_w = w_in * (1 - legend_frac)
    h_in = map_w * shape[0] / shape[1] + (0.3 if title else 0.05)
    fig = plt.figure(figsize=(w_in, h_in), dpi=dpi)
    top = 1 - (0.3 / h_in if title else 0.0)
    ax = fig.add_axes([0, 0, 1 - legend_frac, top])
    ax.set_axis_off()
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    return fig, ax


def base(ax, grey="#e9ece6"):
    m, info = land()
    rgba = np.zeros(m.shape + (4,))
    rgba[m] = mcolors.to_rgba(grey)
    ax.imshow(rgba, extent=extent(info), interpolation="nearest")
    ax.set_xlim(extent(info)[0], extent(info)[1])
    ax.set_ylim(extent(info)[2], extent(info)[3])
    return info


def base_limits(ax):
    e = extent(land()[1])
    ax.set_xlim(e[0], e[1])
    ax.set_ylim(e[2], e[3])
    ax.set_aspect("equal")
    ax.set_axis_off()


def surface(ax, a, info, cmap="viridis", vmin=None, vmax=None, norm=None):
    cm = plt.get_cmap(cmap).copy()
    cm.set_bad((1, 1, 1, 0))
    im = ax.imshow(np.ma.masked_invalid(a), extent=extent(info), cmap=cm, vmin=vmin, vmax=vmax, norm=norm,
                   interpolation="nearest")
    return im


def colorbar(fig, im, label="", rect=(0.80, 0.18, 0.03, 0.6), ticks=None):
    cax = fig.add_axes(list(rect))
    cb = fig.colorbar(im, cax=cax, ticks=ticks)
    cb.outline.set_visible(False)
    cb.ax.grid(False)
    cb.ax.tick_params(labelsize=9, color=figs.RULE)
    if label:
        cb.set_label(label, fontsize=9.5, color=figs.INK2)
    return cb


def points(ax, x, y, color=PRES, size=9, label=None, marker="o", edge="white", lw=0.4, alpha=1.0, z=3):
    ax.scatter(x, y, s=size, c=color, marker=marker, edgecolors=edge, linewidths=lw, label=label, alpha=alpha,
               zorder=z)


def legend(fig, items, loc=(0.80, 0.5), fontsize=9.5):
    """items: [(label, colour, kind)] kind 'o' a point, 's' a square patch."""
    hs = []
    for lab, col, kind in items:
        if kind == "s":
            hs.append(Line2D([0], [0], marker="s", linestyle="", markersize=9, markerfacecolor=col,
                             markeredgecolor="#00000022", label=lab))
        else:
            hs.append(Line2D([0], [0], marker=kind, linestyle="", markersize=6, markerfacecolor=col,
                             markeredgecolor="white", label=lab))
    fig.legend(handles=hs, loc="center left", bbox_to_anchor=loc, fontsize=fontsize, handlelength=1.0,
               borderaxespad=0, labelspacing=0.6)


def save(fig, png: pathlib.Path):
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.06)
    plt.close(fig)
    figs._shrink(png)
    return png
