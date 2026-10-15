"""Figures for the tutorials, from real runs of Terraflux.

A tutorial's figures are made by a script of its own (website/figures/<category>/<tutorial>.py) that uses this
module: it runs a model with the terraflux binary and draws the rasters and tables it writes, all in one style.

    import sys, pathlib; sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
    import figs

    work = figs.workdir("habitat-patterns/mspa")            # a scratch folder (website/.cache/work/...)
    model = figs.template("mspa-of-a-forest")               # a template's TOML (website/models/templates)
    model = figs.add_writes(model, {"mspa.map": "mspa.tif", "forest.map": "forest.tif"})
    figs.run(model, work)                                   # terraflux run, in that folder
    out = figs.img_dir("habitat-patterns/mspa")             # website/static/img/tutorials/habitat-patterns/mspa
    figs.raster(work / "mspa.tif", out / "mspa.png", palette=figs.mspa_palette(), title="MSPA")
    figs.raster(work / "forest.tif", out / "forest.png", colors={1: "#e9e4d8", 2: "#3b8a5a"},
                labels={1: "Background", 2: "Forest"})

Pages then show /img/tutorials/habitat-patterns/mspa/mspa.png. Keep figures small: 600 to 1100 px wide.
"""

from __future__ import annotations

import csv
import json
import pathlib
import re
import shutil
import subprocess
import tomllib

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

WEB = pathlib.Path(__file__).resolve().parents[1]
ROOT = WEB.parent

# the site's colours
FOREST = "#3b8a5a"
EMBER = "#e8901f"
WATER = "#2f6f9f"
INK = "#1c2420"
INK2 = "#48534d"
RULE = "#d9e0d9"
PAPER = "#ffffff"
# the application's colours of classes (Okabe & Ito's, then Tableau's), in order
CLASSES = [
    "#0072b2", "#e69f00", "#009e73", "#cc79a7", "#56b4e9", "#d55e00", "#f0e442", "#785ef0", "#8c564b", "#7f7f7f",
    "#bcbd22", "#17becf", "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#e377c2", "#9edae5", "#dbdb8d",
]
# land use as the tutorials draw it
LAND = {1: ("Forest", "#3b8a5a"), 2: ("Farming", "#d9a441"), 3: ("Water", "#3f7fae"), 4: ("Urban", "#b4508a"),
        5: ("Grassland", "#a7c957")}

def _font() -> str:
    from matplotlib import font_manager

    have = {f.name for f in font_manager.fontManager.ttflist}
    return next((f for f in ("Source Sans 3", "Source Sans Pro", "Segoe UI", "Helvetica Neue", "Arial") if f in have),
                "DejaVu Sans")


plt.rcParams.update({
    "font.family": _font(),
    "font.size": 10.5,
    "axes.edgecolor": RULE,
    "axes.labelcolor": INK2,
    "axes.titlesize": 12,
    "axes.titleweight": "bold",
    "axes.titlecolor": INK,
    "xtick.color": INK2,
    "ytick.color": INK2,
    "axes.spines.top": False,
    "axes.spines.right": False,
    "axes.grid": True,
    "grid.color": RULE,
    "grid.linestyle": (0, (2, 3)),
    "figure.facecolor": PAPER,
    "axes.facecolor": PAPER,
    "savefig.facecolor": PAPER,
    "legend.frameon": False,
})


# ------------------------------------------------------------------------------------------------ running models


def binary() -> pathlib.Path:
    for n in ("terraflux.exe", "terraflux"):
        p = ROOT / "target" / "release" / n
        if p.exists():
            return p
    w = shutil.which("terraflux")
    if w:
        return pathlib.Path(w)
    raise SystemExit("no terraflux binary: build it with cargo build --release")


def workdir(name: str, clean=True) -> pathlib.Path:
    d = WEB / ".cache" / "work" / name
    if clean and d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True, exist_ok=True)
    return d


def img_dir(name: str) -> pathlib.Path:
    d = WEB / "static" / "img" / "tutorials" / name
    d.mkdir(parents=True, exist_ok=True)
    return d


def data_dir(name: str) -> pathlib.Path:
    """Where a tutorial's chart data (CSV) goes: website/static/data/tutorials/<name>, shown as /data/tutorials/..."""
    d = WEB / "static" / "data" / "tutorials" / name
    d.mkdir(parents=True, exist_ok=True)
    return d


def template(slug: str) -> str:
    return (WEB / "models" / "templates" / f"{slug}.toml").read_text(encoding="utf-8")


def add_writes(model: str, writes: dict[str, str]) -> str:
    """The model with its maps written: {"node.port": "file.tif"} adds a raster.write node (a map, a stack, a
    loop's series: a band per map); {"node.port": "name.csv"} makes the value an output of the model, which
    `terraflux run` writes into work/output (a table as output/name.csv)."""
    out = [model.rstrip(), ""]
    for i, (ref, path) in enumerate(writes.items()):
        if path.endswith(".tif"):
            out += ["[[nodes]]", f'id = "fig_write_{i}"', 'op = "raster.write"', f'inputs = {{ value = "{ref}" }}',
                    f'params = {{ path = "{path}" }}', ""]
        else:
            out += [f"[outputs.{pathlib.Path(path).stem}]", f'from = "{ref}"', ""]
    return "\n".join(out)


def run(model: str | pathlib.Path, work: pathlib.Path, *args: str, name="model.toml", check=True) -> subprocess.CompletedProcess:
    """Runs a model (its TOML text, or a file) with `terraflux run` in `work`; the paths of the model are relative
    to it. Extra arguments go to the command (`--set seed=3`)."""
    if isinstance(model, str) and "\n" in model:
        f = work / name
        f.write_text(model, encoding="utf-8")
    else:
        f = pathlib.Path(model)
    r = subprocess.run([str(binary()), "run", str(f), "--out", str(work / "output"), *args], cwd=work,
                       capture_output=True, text=True, encoding="utf-8")
    if check and r.returncode != 0:
        raise SystemExit(f"terraflux run failed:\n{r.stdout}\n{r.stderr}")
    return r


def cli(*args: str, cwd: pathlib.Path | None = None) -> str:
    r = subprocess.run([str(binary()), *args], cwd=cwd, capture_output=True, text=True, encoding="utf-8")
    return (r.stdout + r.stderr).replace("terraflux.exe", "terraflux")


# ------------------------------------------------------------------------------------------------ reading


def read(path: pathlib.Path, band: int = 1) -> tuple[np.ndarray, dict]:
    import rasterio

    with rasterio.open(path) as ds:
        a = ds.read(band, masked=True).astype("float64")
        info = {"count": ds.count, "descriptions": ds.descriptions, "nodata": ds.nodata, "crs": str(ds.crs),
                "transform": ds.transform, "shape": (ds.height, ds.width)}
    return a, info


def bands(path: pathlib.Path) -> int:
    import rasterio

    with rasterio.open(path) as ds:
        return ds.count


def read_csv(path: pathlib.Path) -> list[dict]:
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


# ------------------------------------------------------------------------------------------------ palettes


def _rs_table(name: str) -> list[tuple[int, int, int]]:
    src = (ROOT / "crates/ops/src/gtb_palettes.rs").read_text(encoding="utf-8")
    i = src.index("= [", src.index(f"pub const {name}:"))
    j = src.index("];", i)
    return [tuple(int(v) for v in m) for m in re.findall(r"\[(\d+), (\d+), (\d+)\]", src[i:j])]


def gtb_colors(name: str = "MSPA_ON") -> dict[int, str]:
    """GuidosToolbox's colour table (MSPA_ON, FAD, FAD5, ENTROPY, LM, CHANGE, DIST, ...) by value."""
    return {v: "#%02x%02x%02x" % rgb for v, rgb in enumerate(_rs_table(name))}


MSPA_NAMES = {0: "Background", 1: "Branch", 3: "Edge", 5: "Perforation", 9: "Islet", 17: "Core", 33: "Bridge",
              35: "Bridge in edge", 37: "Bridge in perforation", 65: "Loop", 67: "Loop in edge",
              69: "Loop in perforation", 100: "Core-opening", 129: "Missing", 220: "Border-opening"}


def mspa_palette() -> dict[int, tuple[str, str]]:
    """value -> (name, colour) for an MSPA map, in GuidosToolbox's colours (internal features, +100, folded in)."""
    c = gtb_colors("MSPA_ON")
    out = {v: (n, c[v]) for v, n in MSPA_NAMES.items()}
    for v, n in list(MSPA_NAMES.items()):
        if v < 100 and v + 100 < 256 and v not in (0,):
            out[v + 100] = (n + " (internal)", c[v + 100])
    return out


def land_palette(n: int = 3) -> dict[int, tuple[str, str]]:
    return {k: LAND[k] for k in list(LAND)[:n]}


# ------------------------------------------------------------------------------------------------ drawing


def _fig_for(shape, width_px, legend):
    h, w = shape
    dpi = 150
    width_in = width_px / dpi
    map_w = width_in * (0.72 if legend else 1.0)
    height_in = map_w * h / w * 1.1 + 0.15
    return plt.figure(figsize=(width_in, max(height_in, 1.2)), dpi=dpi)


def raster(src: pathlib.Path, png: pathlib.Path, band: int = 1, palette: dict | None = None,
           colors: dict | None = None, labels: dict | None = None, cmap: str | None = None, vmin=None, vmax=None,
           title: str | None = None, legend: bool = True, unit: str = "", diverging: bool = False,
           width: int = 900, counts: bool = False, array: np.ndarray | None = None, log: bool = False) -> pathlib.Path:
    """A raster drawn as a map. Classes when `palette` ({value: (name, colour)}) or `colors` ({value: colour},
    with `labels`) are given, or when the values are a few whole numbers; otherwise a colour ramp with a bar
    (`cmap`, viridis by default; `diverging` centres it on 0). `counts` adds each class's share to its label."""
    a = array if array is not None else read(pathlib.Path(src), band)[0]
    a = np.ma.masked_invalid(a)
    vals = np.unique(a.compressed()) if a.count() else np.array([])
    classes = palette is not None or colors is not None or (
        cmap is None and not diverging and len(vals) <= 12 and np.all(np.mod(vals, 1) == 0))
    fig = _fig_for(a.shape, width, legend)
    top = 0.9 if title else 1.0
    ax = fig.add_axes([0, 0, 0.72 if legend else 1.0, top])
    ax.set_axis_off()
    if classes:
        if palette is None:
            colors = colors or {}
            labels = labels or {}
            palette = {}
            for i, v in enumerate(vals):
                v = int(v)
                palette[v] = (labels.get(v, str(v)), colors.get(v, CLASSES[i % len(CLASSES)]))
        present = [int(v) for v in vals if int(v) in palette]
        filled = a.filled(-1).astype(np.int64)
        keys = sorted(set(present) | {int(v) for v in vals})
        # any number of classes (a map of patch ids has thousands): a colour per key, cells indexed by search
        lut = np.zeros((len(keys) + 1, 4))
        for i, v in enumerate(keys):
            col = palette.get(v, (str(v), CLASSES[i % len(CLASSES)]))[1]
            lut[i + 1] = mcolors.to_rgba(col)
        karr = np.asarray(keys, dtype=np.int64)
        pos = np.searchsorted(karr, filled)
        pos = np.clip(pos, 0, max(len(karr) - 1, 0))
        idx = np.where((len(karr) > 0) & (karr[pos] == filled) & ~np.ma.getmaskarray(a), pos + 1, 0)
        lut[0] = (1, 1, 1, 0)
        rgba = lut[idx]
        ax.imshow(rgba, interpolation="nearest")
        if legend:
            total = a.count()
            handles = []
            for v in keys:
                if v not in palette:
                    continue
                name, col = palette[v]
                share = (filled == v).sum() / total * 100 if counts and total else None
                lab = f"{name}" + (f"  {share:.1f}%" if share is not None else "")
                handles.append(Patch(facecolor=col, edgecolor="#00000022", label=lab))
            fig.legend(handles=handles, loc="center left", bbox_to_anchor=(0.73, 0.5), fontsize=9.5,
                       handlelength=1.1, handleheight=1.1, borderaxespad=0, labelspacing=0.55)
    else:
        lo = np.nanmin(a) if vmin is None else vmin
        hi = np.nanmax(a) if vmax is None else vmax
        if diverging:
            m = max(abs(lo), abs(hi)) or 1
            norm = mcolors.TwoSlopeNorm(vmin=-m, vcenter=0, vmax=m)
            cm = cmap or "RdBu_r"
        elif log:
            norm = mcolors.LogNorm(vmin=max(lo, 1e-9), vmax=hi)
            cm = cmap or "viridis"
        else:
            norm = mcolors.Normalize(vmin=lo, vmax=hi)
            cm = cmap or "viridis"
        cmo = plt.get_cmap(cm).copy()
        cmo.set_bad((1, 1, 1, 0))
        im = ax.imshow(a, cmap=cmo, norm=norm, interpolation="nearest")
        if legend:
            cax = fig.add_axes([0.76, 0.1 * top + 0.02, 0.035, 0.76 * top])
            cb = fig.colorbar(im, cax=cax)
            cb.outline.set_visible(False)
            cb.ax.grid(False)
            cb.ax.tick_params(labelsize=9, color=RULE)
            if unit:
                cb.set_label(unit, fontsize=9.5, color=INK2)
    if title:
        fig.text(0.0, 0.985, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=INK)
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.08)
    plt.close(fig)
    _shrink(png)
    return png


def frames(src: pathlib.Path, pattern: str, out_dir: pathlib.Path, **kw) -> list[pathlib.Path]:
    """Every band of a stack (a simulation's steps) drawn with the same settings: pattern 'step_{i}.png'.
    Continuous maps share one colour range."""
    n = bands(src)
    if "vmin" not in kw and "palette" not in kw and "colors" not in kw:
        arrs = [read(src, b)[0] for b in range(1, n + 1)]
        kw.setdefault("vmin", float(min(np.nanmin(x) for x in arrs)))
        kw.setdefault("vmax", float(max(np.nanmax(x) for x in arrs)))
    out = []
    for b in range(1, n + 1):
        out.append(raster(src, out_dir / pattern.format(i=b), band=b, **kw))
    return out


def chart(png: pathlib.Path, draw, width: int = 900, height: int = 420, title: str | None = None) -> pathlib.Path:
    """A matplotlib chart in the site's style: `draw(ax)` draws on the axes."""
    fig, ax = plt.subplots(figsize=(width / 150, height / 150), dpi=150)
    draw(ax)
    if title:
        ax.set_title(title, loc="left")
    fig.tight_layout()
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    _shrink(png)
    return png


def write_csv(path: pathlib.Path, rows: list[dict]):
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)


def class_areas(src: pathlib.Path, classes: dict[int, str] | None = None) -> list[dict]:
    """The share of each class in every band of a stack: rows {step, <class name>: percent}, for a tfchart."""
    rows = []
    for b in range(1, bands(src) + 1):
        a = read(src, b)[0]
        total = a.count() or 1
        r = {"step": b}
        vals = classes or {int(v): str(int(v)) for v in np.unique(a.compressed())}
        for v, name in vals.items():
            r[name] = round(float((a == v).sum()) / total * 100, 3)
        rows.append(r)
    return rows


def _shrink(png: pathlib.Path):
    """PNG saved with an optimised palette when that keeps it exact enough (maps of classes compress well)."""
    try:
        from PIL import Image

        im = Image.open(png)
        if im.mode in ("RGBA", "RGB"):
            q = im.convert("RGB").quantize(colors=256, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
            tmp = png.with_suffix(".tmp.png")
            q.save(tmp, optimize=True)
            if tmp.stat().st_size < png.stat().st_size:
                tmp.replace(png)
            else:
                tmp.unlink()
    except Exception:
        pass
