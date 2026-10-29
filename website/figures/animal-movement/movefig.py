"""Shared pieces of the figures of the animal movement tutorials (website/figures/animal-movement).

The data are move2's example (and amt's): the GPS tracks of eight fishers (*Pekania pennanti*) near Albany, New
York, 2009 to 2011 (LaPoint et al., 2013, Movebank's study "Martes pennanti LaPoint New York"), the Movebank
export that travels in tests/data/move/fishers.csv.gz. The landscape (elevation, land cover) is downloaded by
Terraflux itself (data.download: the Copernicus DEM and ESA WorldCover 2021). The models are run with the
terraflux binary; this module builds their TOML, reads what they write and draws the tracks.
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

CAT = "animal-movement"
FISHERS = figs.ROOT / "tests" / "data" / "move" / "fishers.csv.gz"
# the data a reader downloads: in the first part's folder
PUBLIC = figs.WEB / "static" / "data" / "tutorials" / CAT / "ideas"
ID = "individual-local-identifier"

# the area of the tracks, with a margin: [west, south, east, north]
EXTENT = [-74.0, 42.65, -73.33, 42.9]
CELL = 0.0003
WORLDCOVER = ("https://esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/map/"
              "ESA_WorldCover_10m_2021_v200_N42W075_Map.tif")
COVER = {10: ("Tree cover", "#3b8a5a"), 20: ("Shrubland", "#a7a35a"), 30: ("Grassland", "#c9d68a"),
         40: ("Cropland", "#e8c86a"), 50: ("Built-up", "#b4508a"), 60: ("Bare", "#c8bba6"),
         80: ("Water", "#3f7fae"), 90: ("Wetland", "#6fb3b8")}

ANIMALS = ["F1", "F2", "F3", "M1", "M2", "M3", "M4", "M5"]
COLORS = dict(zip(ANIMALS, figs.CLASSES))


def publish_data():
    """The tracks where readers download them."""
    PUBLIC.mkdir(parents=True, exist_ok=True)
    if not (PUBLIC / FISHERS.name).exists():
        shutil.copy(FISHERS, PUBLIC / FISHERS.name)


def work(slug: str) -> pathlib.Path:
    w = figs.workdir(f"{CAT}/{slug}")
    shutil.copy(FISHERS, w / FISHERS.name)
    return w


def _value(v) -> str:
    return json.dumps(v) if not isinstance(v, (dict,)) else "{ " + ", ".join(f"{k} = {_value(x)}" for k, x in v.items()) + " }"


def toml(nodes: list[dict], outputs: dict[str, str] | None = None) -> str:
    """A model's TOML from nodes {id, op, inputs, params}; outputs {name: "node.port"}."""
    out = ['format = "terraflux/1"', ""]
    for n in nodes:
        out += ["[[nodes]]", f'id = "{n["id"]}"', f'op = "{n["op"]}"']
        for k in ("inputs", "params"):
            if n.get(k):
                out.append(f"{k} = {_value(n[k])}")
        out.append("")
    for name, ref in (outputs or {}).items():
        out += [f"[outputs.{name}]", f'from = "{ref}"', ""]
    return "\n".join(out)


# the tracks read and cleaned, as the first two parts build them
CLEAN = [
    {"id": "read", "op": "move.read_tracks", "params": {"path": "fishers.csv.gz"}},
    {"id": "located", "op": "vector.filter", "inputs": {"vector": "read.tracks"},
     "params": {"expression": "not isnull(geometry.x)"}},
    {"id": "visible", "op": "move.filter_movebank_visible", "inputs": {"tracks": "located.vector"}},
    {"id": "unique", "op": "move.filter_unique", "inputs": {"tracks": "visible.tracks"}},
]

# the landscape, downloaded by Terraflux
LANDSCAPE = [
    {"id": "dem", "op": "data.download",
     "params": {"source": "copernicus_dem", "dem_resolution": 30, "extent": EXTENT, "cellsize": CELL}},
    {"id": "cover", "op": "data.download",
     "params": {"url": WORLDCOVER, "extent": EXTENT, "cellsize": CELL, "resampling": "mode"}},
]


def run(w: pathlib.Path, nodes: list[dict], outputs: dict[str, str], name="model.toml") -> dict:
    """Runs the model in w and reads its outputs: vectors as GeoDataFrames, tables as DataFrames, the rest as
    the paths written."""
    import geopandas as gpd
    import pandas as pd

    out = w / "output"
    if out.exists():
        shutil.rmtree(out)
    figs.run(toml(nodes, outputs), w, name=name)
    got = {}
    for k in outputs:
        for ext in (".gpkg", ".csv", ".tif", ".json"):
            p = out / f"{k}{ext}"
            if p.exists():
                got[k] = gpd.read_file(p) if ext == ".gpkg" else pd.read_csv(p) if ext == ".csv" else p
                break
    return got


def lonlat(ax, lat=42.78):
    """Axes in longitude and latitude with the scale of the ground (a degree of longitude is cos(lat) of one of
    latitude)."""
    ax.set_aspect(1 / np.cos(np.radians(lat)))
    ax.grid(False)
    ax.set_xlabel("Longitude")
    ax.set_ylabel("Latitude")
    ax.tick_params(labelsize=8.5)
    ax.ticklabel_format(useOffset=False)


def scalebar(ax, km=2, lat=42.78, where=(0.05, 0.06)):
    """A scale bar of km kilometres in axes in longitude and latitude."""
    x0, x1 = ax.get_xlim()
    y0, y1 = ax.get_ylim()
    dx = km / (111.32 * np.cos(np.radians(lat)))
    x = x0 + where[0] * (x1 - x0)
    y = y0 + where[1] * (y1 - y0)
    ax.plot([x, x + dx], [y, y], color=figs.INK, lw=2.5, solid_capstyle="butt")
    ax.text(x + dx / 2, y + 0.012 * (y1 - y0), f"{km} km", ha="center", va="bottom", fontsize=8.5, color=figs.INK)


def save(fig, png: pathlib.Path):
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)
    return png


def cover_image(path: pathlib.Path):
    """The land cover as an RGB image and its extent, for imshow."""
    import rasterio
    from matplotlib import colors as mcolors

    with rasterio.open(path) as r:
        a = r.read(1)
        b = r.bounds
    rgb = np.ones(a.shape + (3,))
    for v, (_, c) in COVER.items():
        rgb[a == v] = mcolors.to_rgb(c)
    # lightened, so tracks show over it
    rgb = 1 - 0.45 * (1 - rgb)
    return rgb, [b.left, b.right, b.bottom, b.top]
