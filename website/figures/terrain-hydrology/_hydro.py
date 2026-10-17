"""Shared bits of the figure scripts of the tutorials on terrain hydrology.

The data: landschaft's SRTM elevation of south-western Oregon (orelevation.tif, 1 arc-second) and its summer NDVI
(orforest.tif), read as `elevation.tif` and `ndvi.tif` and reprojected to UTM zone 10N in cells of 30 m, as in
the tutorial "Terrain from an elevation model".
"""

from __future__ import annotations

import pathlib
import shutil
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))
import figs  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

ROOT = figs.ROOT
OREGON = ROOT / "extern/landschaft/src/landschaft/surface/data"
SWAT_EXAMPLE = ROOT / "extern/landschaft/src/landschaft/hydro/swat/data/hubbard_brook.zip"

EXTENT = "[500010, 4760850, 508110, 4771920]"
CELL = 30.0

# the model's first two nodes in every page: the DEM read and reprojected
HEAD = f'''format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = {{ path = "elevation.tif" }}

[[nodes]]
id = "elevation"
op = "raster.reproject"
inputs = {{ raster = "dem.raster" }}
params = {{ crs = "EPSG:32610", resolution = 30, extent = {EXTENT}, resampling = "bilinear" }}

[[nodes]]
id = "shade"
op = "raster.terrain"
inputs = {{ raster = "elevation.raster" }}
params = {{ measure = "hillshade" }}
'''

NDVI = f'''
[[nodes]]
id = "ndvi_read"
op = "raster.read"
params = {{ path = "ndvi.tif" }}

[[nodes]]
id = "ndvi"
op = "raster.reproject"
inputs = {{ raster = "ndvi_read.raster" }}
params = {{ crs = "EPSG:32610", resolution = 30, extent = {EXTENT}, resampling = "bilinear" }}
'''


def setup(slug: str):
    """The work folder (with elevation.tif and ndvi.tif), the images' and the data's folders of a page."""
    work = figs.workdir(f"terrain-hydrology/{slug}")
    shutil.copy2(OREGON / "orelevation.tif", work / "elevation.tif")
    shutil.copy2(OREGON / "orforest.tif", work / "ndvi.tif")
    return work, figs.img_dir(f"terrain-hydrology/{slug}"), figs.data_dir(f"terrain-hydrology/{slug}")


def rd(work: pathlib.Path, name: str) -> np.ma.MaskedArray:
    return figs.read(work / name)[0]


def table(work: pathlib.Path, name: str) -> list[dict]:
    """A table the model wrote to work/output/<name>.csv, numbers as floats."""
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


def fig_map(png, arr, cmap="viridis", title=None, unit="", vmin=None, vmax=None, norm=None, under=None, alpha=1.0,
            width=760, legend=True, classes=None, counts=True, extra=None, crop=None):
    """A map in the site's style, optionally over a hillshade (under) and with a colour bar or a class legend.
    `classes` {value: (name, colour)}; `extra(ax)` draws more on the map (lines, points); `crop` (r0, r1, c0, c1)."""
    if crop is not None:
        r0, r1, c0, c1 = crop
        arr = arr[r0:r1, c0:c1]
        under = under[r0:r1, c0:c1] if under is not None else None
    h, w = arr.shape
    dpi = 150
    win = width / dpi
    mapw = win * (0.74 if legend else 1.0)
    fig = plt.figure(figsize=(win, mapw * h / w + (0.35 if title else 0.1)), dpi=dpi)
    top = 1 - (0.32 / (mapw * h / w + 0.35)) if title else 1.0
    ax = fig.add_axes([0, 0, 0.74 if legend else 1.0, top])
    ax.set_axis_off()
    if under is not None:
        ax.imshow(under, cmap="gray", vmin=0, vmax=255, interpolation="nearest")
    if classes is not None:
        rgba = np.zeros(arr.shape + (4,))
        f = np.ma.filled(arr, -999)
        for v, (_, col) in classes.items():
            rgba[f == v] = mcolors.to_rgba(col)
        if under is not None:
            rgba[..., 3] *= alpha
        ax.imshow(rgba, interpolation="nearest")
        if legend:
            total = max(int(np.ma.count(arr)), 1)
            hs = [Patch(facecolor=c, edgecolor="#00000022",
                        label=f"{n}  {(f == v).sum() / total * 100:.0f}%" if counts else n)
                  for v, (n, c) in classes.items()]
            fig.legend(handles=hs, loc="center left", bbox_to_anchor=(0.75, 0.5), fontsize=9.5, handlelength=1.1,
                       handleheight=1.1, labelspacing=0.55)
    else:
        cm = plt.get_cmap(cmap).copy()
        cm.set_bad((1, 1, 1, 0))
        im = ax.imshow(arr, cmap=cm, vmin=None if norm is not None else vmin, vmax=None if norm is not None else vmax,
                       norm=norm, alpha=alpha, interpolation="nearest")
        if legend:
            cax = fig.add_axes([0.78, 0.12 * top, 0.035, 0.74 * top])
            cb = fig.colorbar(im, cax=cax)
            cb.outline.set_visible(False)
            cb.ax.grid(False)
            cb.ax.tick_params(labelsize=9, color=figs.RULE)
            if unit:
                cb.set_label(unit, fontsize=9.5, color=figs.INK2)
    if extra is not None:
        extra(ax)
        ax.set_xlim(-0.5, w - 0.5)
        ax.set_ylim(h - 0.5, -0.5)
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.06)
    plt.close(fig)
    figs._shrink(png)
    return png


def legend_strip(png, classes, ncol=None):
    """A legend drawn alone (beside a slider or a comparison)."""
    fig = plt.figure(figsize=(7.2, 0.5), dpi=150)
    hs = [Patch(facecolor=c, edgecolor="#00000022", label=n) for _, (n, c) in classes.items()]
    fig.legend(handles=hs, loc="center", ncol=ncol or len(hs), fontsize=9.5, handlelength=1.1, handleheight=1.1,
               columnspacing=1.2)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.04)
    plt.close(fig)
    figs._shrink(png)


def write_tif(path: pathlib.Path, a: np.ndarray, cell: float = 10.0, x0: float = 500000.0, y0: float = 4800000.0,
              crs: str = "EPSG:32610"):
    """A small GeoTIFF (a toy elevation model) in UTM, float64, NaN as no value."""
    import rasterio
    from rasterio.transform import from_origin

    a = np.asarray(a, dtype="float64")
    with rasterio.open(path, "w", driver="GTiff", height=a.shape[0], width=a.shape[1], count=1, dtype="float64",
                       crs=crs, transform=from_origin(x0, y0, cell, cell), nodata=float("nan")) as ds:
        ds.write(a, 1)


def stats(a) -> dict:
    v = np.ma.compressed(a)
    return dict(mean=float(v.mean()), min=float(v.min()), max=float(v.max()), sd=float(v.std()), n=int(v.size),
                p10=float(np.percentile(v, 10)), p50=float(np.percentile(v, 50)), p90=float(np.percentile(v, 90)))


def make_filled(work: pathlib.Path):
    """Part 1's model up to the filled DEM, saved as the reader saves it (filled.tif, 64-bit floats), with the
    reprojected elevation and its hillshade for the drawings; parts 2 and 3 start by reading filled.tif."""
    model = HEAD + '''
[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "elevation.raster" }

[[nodes]]
id = "save_filled"
op = "raster.write"
inputs = { value = "filled.map" }
params = { path = "filled.tif", type = "f64" }
'''
    figs.run(figs.add_writes(model, {"elevation.raster": "elevation_utm.tif", "shade.raster": "shade.tif"}), work,
             name="part1.toml")


# parts 2 and 3: the filled DEM of part 1, read
FILLED = '''format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }
'''
