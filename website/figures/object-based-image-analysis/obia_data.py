"""The two image clips shared by the object-based image analysis tutorials, and small drawing helpers.

Every page of the series uses the same two places, so the reader knows the landscape from one page to the next:

* Sentinel-2 L2A, 10 m, blue green red nir swir16: the edge of Lago Paranoa, Brasilia (city, gallery forest,
  dry cerrado, farmland, water), the scene of 2024-08-09 (Earth Search item S2A_22LHH_20240809_0_L2A, 0.0005 %
  cloud). Fetched with a Terraflux model whose node is `data.stac`.
* NAIP, 0.6 m, red green blue nir: a suburb of Plano, Texas (houses, trees, lawns, streets, ponds), the item
  tx_m_3309658_se_14_060_20241109 of the Planetary Computer. `data.stac` reads only the first band of an asset and
  NAIP's single asset `image` has four, so this clip is cut straight from the signed file (rasterio) and the models
  read it with `raster.read`.

Use, in a page's figure script:

    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    import obia_data as od

    tif = od.sentinel2()                       # Path of the cached clip (made once, website/.cache/obia/)
    model = od.sentinel2_model()               # TOML: data.stac `s2` (degrees), raster.reproject `utm`, `refl` (refl.image, 0-1)
    od.run(model + more_nodes, work)           # terraflux run, at most 4 threads
    rgb = od.rgb("sentinel2", tif)             # float RGB (rows, cols, 3), stretched for display
    img = od.outlines(rgb, labels, scale=2)    # the same, with the objects' outlines

Both clips:

* `od.sentinel2()`: float32, 580 x 580 cells, EPSG:31983 (SIRGAS 2000 / UTM 23S), square 10 m cells (extent
  190520, 8241280, 196320, 8247080), values reflectance x 10000 (a cell is 0.01 ha, `area` in the objects' table is in
  m2). Layers blue, green, red, nir, swir16 in this order (raster.reproject hands them on alphabetically; the module
  rewrites the cached file in the order above, and the download too). The model is data.stac (a box in degrees at
  0.00009) -> raster.reproject (crs, resolution = 10, extent) -> raster.calc `refl` (s / 10000, output `refl.image`).
* `od.naip()`: uint8, 1000 x 1000, EPSG:26914 at 0.6 m, layers red, green, blue, nir.
* `od.publish()` writes the small downloads (Sentinel-2 as Int16 with nodata -9999, so its object counts differ a
  little from the cache's: scale 2 gives 2,992 objects instead of 3,035) and their attribution to
  website/static/data/tutorials/.

Notes from part 2 (what-is-an-object.py), for the next pages:

* The display window of the figures is rows 90-360, columns 200-470 of the clip (lake edge, houses, gallery forest,
  cerrado); the pixels-and-objects window is rows 195-235, columns 270-310.
* multiresolution on refl.image: segment.scale suggests 2.14; scale 1 gives 11,145 objects, 2 gives 3,035 (1.11 ha),
  4 gives 739, 8 gives 187 (shape 0.1, compactness 0.5). A scale of 0.02 merges nothing (the scale is in the units
  of refl.image, about 1 to 8 here). The other segmenters standardise the bands; the multiresolution does not.
* Other counts: slic size 12 gives 2,270, felzenszwalb k = 2 gives 4,063, watershed defaults 747, threshold on the
  NDWI (raster.calc (s.green - s.nir) / (s.green + s.nir)) with 2 classes gives 12 objects, Otsu cut at 0.065 (water
  has NDWI above it).
* Image luminance: reflectance of the lake red about 0.01, the cerrado about 0.08.
* A raster.calc that returns a stack needs `out name = ...` (the output is then `refl.image`).
* Planetary Computer limits requests (HTTP 429): do not run many naip() fetches in a row.
"""

from __future__ import annotations

import json
import os
import pathlib
import sys
import urllib.parse
import urllib.request

# at most 4 threads, whatever runs
for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS"):
    os.environ[_k] = "4"

HERE = pathlib.Path(__file__).resolve().parent
WEB = HERE.parents[1]
sys.path.insert(0, str(WEB / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

CACHE = WEB / ".cache" / "obia"
DOWNLOADS = WEB / "static" / "data" / "tutorials" / "object-based-image-analysis"
CATEGORY = "object-based-image-analysis"

# ---------------------------------------------------------------------------------------------- Sentinel-2

S2_URL = "https://earth-search.aws.element84.com/v1"
S2_ITEM = "S2A_22LHH_20240809_0_L2A"
S2_DATE = "2024-08-09"
S2_BANDS = ["blue", "green", "red", "nir", "swir16"]
# west, south, east, north (degrees): about 5.5 x 5.5 km at the south edge of the lake
S2_EXTENT = [-47.89, -15.89, -47.835, -15.835]
S2_CELL = 0.00009
# the clip itself: EPSG:31983, xmin, ymin, xmax, ymax (m), 580 x 580 cells of 10 m (the part of the box above)
S2_CRS = "EPSG:31983"
S2_UTM_EXTENT = [190520, 8241280, 196320, 8247080]


def sentinel2_node(node_id: str = "s2") -> str:
    """The `data.stac` node of the Sentinel-2 clip, as a model's TOML (its output is `<id>.raster`)."""
    bands = ", ".join(f'"{b}"' for b in S2_BANDS)
    return (
        f'[[nodes]]\nid = "{node_id}"\nop = "data.stac"\n'
        f'params = {{ url = "{S2_URL}", collection = "sentinel-2-l2a", assets = [{bands}], datetime = "{S2_DATE}", '
        f"extent = {S2_EXTENT}, cellsize = {S2_CELL} }}\n"
    )


def reflectance_node(node_id: str = "refl", source: str = "s2.raster") -> str:
    """A `raster.calc` node that divides the layers by 10 000: reflectance from 0 to 1 (the layers keep their names)."""
    return (
        f'[[nodes]]\nid = "{node_id}"\nop = "raster.calc"\ninputs = {{ s = "{source}" }}\n'
        'params = { expression = "out image = s / 10000" }\n'
    )


def utm_node(node_id: str = "utm", source: str = "s2.raster") -> str:
    """A `raster.reproject` node: the clip in SIRGAS 2000 / UTM 23S, square 10 m cells, 580 x 580 of them."""
    return (
        f'[[nodes]]\nid = "{node_id}"\nop = "raster.reproject"\ninputs = {{ raster = "{source}" }}\n'
        f'params = {{ crs = "{S2_CRS}", resolution = 10, extent = {S2_UTM_EXTENT} }}\n'
    )


def sentinel2_model(node_id: str = "s2", with_reflectance: bool = True) -> str:
    """A model with the `data.stac` node `s2` (degrees), the `raster.reproject` node `utm` after it (EPSG:31983, 10 m)
    and, unless `with_reflectance` is false, the node `refl` (output `refl.image`, reflectance 0 to 1, what the
    segmenters of the series read)."""
    m = 'format = "terraflux/1"\n\n' + sentinel2_node(node_id) + "\n" + utm_node("utm", f"{node_id}.raster")
    if with_reflectance:
        m += "\n" + reflectance_node("refl", "utm.raster")
    return m


def sentinel2() -> pathlib.Path:
    """The Sentinel-2 clip as a GeoTIFF (cached): the model above, run once."""
    tif = CACHE / "sentinel2.tif"
    if not tif.exists():
        work = CACHE / "work-s2"
        work.mkdir(parents=True, exist_ok=True)
        model = figs.add_writes(sentinel2_model(with_reflectance=False), {"utm.raster": "sentinel2.tif"})
        run(model, work)
        # (raster.reproject hands the layers on in alphabetical order: rewritten as blue green red nir swir16)
        import rasterio

        with rasterio.open(work / "sentinel2.tif") as src:
            names = list(src.descriptions)
            data = src.read()
            prof = src.profile | {"compress": "deflate"}
        with rasterio.open(tif, "w", **prof) as out:
            out.write(data[[names.index(b) for b in S2_BANDS]])
            out.descriptions = tuple(S2_BANDS)
    return tif


# ---------------------------------------------------------------------------------------------------- NAIP

PC = "https://planetarycomputer.microsoft.com/api/stac/v1"
NAIP_ITEM = "tx_m_3309658_se_14_060_20241109"
NAIP_CENTER = (33.0480, -96.8010)  # lat, lon: Plano, Texas
NAIP_PIXELS = 1000  # 600 m at 0.6 m
NAIP_BANDS = ["red", "green", "blue", "nir"]


def _get(url: str):
    with urllib.request.urlopen(url, timeout=60) as r:
        return json.load(r)


def naip() -> pathlib.Path:
    """The NAIP clip as a GeoTIFF (cached), cut from the signed file of the item."""
    tif = CACHE / "naip.tif"
    if tif.exists():
        return tif
    import rasterio
    from pyproj import Transformer
    from rasterio.windows import Window

    CACHE.mkdir(parents=True, exist_ok=True)
    item = _get(f"{PC}/collections/naip/items/{NAIP_ITEM}")
    href = item["assets"]["image"]["href"]
    signed = _get("https://planetarycomputer.microsoft.com/api/sas/v1/sign?href=" + urllib.parse.quote(href, safe=""))["href"]
    with rasterio.open(signed) as ds:
        lat, lon = NAIP_CENTER
        x, y = Transformer.from_crs(4326, ds.crs, always_xy=True).transform(lon, lat)
        row, col = ds.index(x, y)
        win = Window(col - NAIP_PIXELS // 2, row - NAIP_PIXELS // 2, NAIP_PIXELS, NAIP_PIXELS)
        data = ds.read(window=win)
        profile = ds.profile | {
            "width": NAIP_PIXELS, "height": NAIP_PIXELS, "transform": ds.window_transform(win),
            "driver": "GTiff", "compress": "deflate", "predictor": 2, "tiled": False, "count": 4,
        }
        profile.pop("blockxsize", None)
        profile.pop("blockysize", None)
        profile.pop("photometric", None)
        profile.pop("interleave", None)
    with rasterio.open(tif, "w", **profile) as out:
        out.write(data)
        out.descriptions = tuple(NAIP_BANDS)
    return tif


def naip_node(node_id: str = "naip", path: str = "naip.tif") -> str:
    """The node that reads the NAIP clip (the file is the download of the series)."""
    return f'[[nodes]]\nid = "{node_id}"\nop = "raster.read"\nparams = {{ path = "{path}" }}\n'


def naip_model(node_id: str = "naip", path: str = "naip.tif") -> str:
    return 'format = "terraflux/1"\n\n' + naip_node(node_id, path)


# --------------------------------------------------------------------------------------------------- running


def run(model: str, work: pathlib.Path, *args: str):
    """terraflux run in `work` with at most 4 threads."""
    return figs.run(model, work, "--threads", "4", "--no-journal", *args)


def read_stack(tif: pathlib.Path) -> np.ndarray:
    """All bands as float (bands, rows, cols), no data as NaN."""
    import rasterio

    with rasterio.open(tif) as ds:
        return ds.read(masked=True).astype("float64").filled(np.nan)


# -------------------------------------------------------------------------------------------------- drawing


def rgb(kind: str, tif: pathlib.Path | None = None, gamma: float = 0.8) -> np.ndarray:
    """A display RGB (rows, cols, 3) in 0..1 from a clip: `sentinel2` or `naip`. Sentinel-2 is stretched from 0 to
    a reflectance of 0.22, NAIP is 8 bit."""
    if kind == "sentinel2":
        a = read_stack(tif or sentinel2())
        v = np.stack([a[2], a[1], a[0]], axis=-1) / 10000 / 0.22
    else:
        a = read_stack(tif or naip())
        v = np.stack([a[0], a[1], a[2]], axis=-1) / 255
    return np.clip(np.nan_to_num(v), 0, 1) ** gamma


def false_color(kind: str, tif: pathlib.Path | None = None) -> np.ndarray:
    """Near infrared, red, green as red, green, blue: vegetation shows red."""
    if kind == "sentinel2":
        a = read_stack(tif or sentinel2())
        v = np.stack([a[3], a[2], a[1]], axis=-1) / 10000 / 0.45
    else:
        a = read_stack(tif or naip())
        v = np.stack([a[3], a[0], a[1]], axis=-1) / 255
    return np.clip(np.nan_to_num(v), 0, 1) ** 0.8


def boundary(labels: np.ndarray) -> np.ndarray:
    """True on the cells that touch a cell of another object on their right or below."""
    b = np.zeros(labels.shape, bool)
    b[:, :-1] |= labels[:, :-1] != labels[:, 1:]
    b[:-1, :] |= labels[:-1, :] != labels[1:, :]
    return b


def outlines(image: np.ndarray, labels: np.ndarray, color=(1.0, 0.85, 0.1), scale: int = 2, thick: int = 1) -> np.ndarray:
    """The image (rows, cols, 3 in 0..1) enlarged `scale` times with the objects' outlines drawn on it, one line
    where two objects touch (`thick` extra upscaled pixels wide)."""
    big = np.kron(image, np.ones((scale, scale, 1)))
    lab = np.kron(labels, np.ones((scale, scale), dtype=labels.dtype))
    b = boundary(lab)
    for _ in range(thick - 1):
        b = b | np.roll(b, 1, 0) | np.roll(b, 1, 1)
    big[b] = color
    return big


def save_png(img: np.ndarray, path: pathlib.Path, width: int | None = None, colors: int = 256) -> pathlib.Path:
    """An image (rows, cols, 3 in 0..1) as a PNG, resized to `width` and quantised to keep the file small."""
    from PIL import Image

    im = Image.fromarray((np.clip(img, 0, 1) * 255 + 0.5).astype("uint8"))
    if width and im.width != width:
        im = im.resize((width, round(im.height * width / im.width)), Image.LANCZOS)
    im = im.quantize(colors=colors, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    im.save(path, optimize=True)
    return path


# ----------------------------------------------------------------------------------------------- downloads

ATTRIBUTION = """Data of the object-based image analysis tutorials of Terraflux (https://terraflux.org)

sentinel2_brasilia.tif
  Sentinel-2 L2A, scene S2A_22LHH_20240809_0_L2A (9 August 2024), from Earth Search
  (https://earth-search.aws.element84.com/v1). Bands blue, green, red, nir (10 m) and swir16 (20 m, resampled to the
  grid), reflectance x 10000, Int16, EPSG:31983 (SIRGAS 2000 / UTM 23S), square 10 m cells, 580 x 580. The area is the edge of
  Lago Paranoa, Brasilia, Brazil.
  Contains modified Copernicus Sentinel data 2024.

naip_plano.tif
  NAIP (National Agriculture Imagery Program), item tx_m_3309658_se_14_060_20241109 (9 November 2024), from the
  Microsoft Planetary Computer. Bands red, green, blue and nir, 8 bit, EPSG:26914, 0.6 m, a window of 600 x 600 m in
  Plano, Texas, United States.
  Imagery by USDA Farm Production and Conservation, Business Center, Geospatial Enterprise Operations; US Government
  work, in the public domain.
"""


def publish() -> list[pathlib.Path]:
    """Small downloads for readers without a network: Int16 for Sentinel-2, the NAIP clip as it is."""
    import rasterio

    DOWNLOADS.mkdir(parents=True, exist_ok=True)
    out = []
    a = read_stack(sentinel2())
    dst = DOWNLOADS / "sentinel2_brasilia.tif"
    with rasterio.open(sentinel2()) as ds:
        prof = ds.profile | {"dtype": "int16", "nodata": -9999, "compress": "deflate", "predictor": 2, "tiled": False}
        prof.pop("blockxsize", None)
        prof.pop("blockysize", None)
    with rasterio.open(dst, "w", **prof) as o:
        o.write(np.where(np.isnan(a), -9999, np.rint(a)).astype("int16"))
        o.descriptions = tuple(S2_BANDS)
    out.append(dst)
    dst = DOWNLOADS / "naip_plano.tif"
    dst.write_bytes(naip().read_bytes())
    out.append(dst)
    txt = DOWNLOADS / "ATTRIBUTION.txt"
    txt.write_text(ATTRIBUTION, encoding="utf-8")
    out.append(txt)
    return out


if __name__ == "__main__":
    for p in publish():
        print(p, p.stat().st_size // 1024, "KB")
    img_dir = figs.img_dir(f"{CATEGORY}/data")
    save_png(rgb("sentinel2"), img_dir / "sentinel2.png", width=800)
    save_png(rgb("naip"), img_dir / "naip.png", width=800)
