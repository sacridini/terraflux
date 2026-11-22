"""Figures of "Large images" (tutorials/object-based-image-analysis/large-images.md).

Two kinds of run, both of Terraflux's own models, at most 2 threads:

* Seams (the Sentinel-2 clip of the series, 580 x 580): segment.multiresolution and segment.region_growing with and
  without tiles, with other tile sizes and margins, and with another seed (what two runs of the same method differ by
  anyway). Drawn: the objects around a tile border, where two runs part ways, and the numbers.
* Time and memory: a larger clip of the same scene (30 x 30 km, 3000 x 3000 cells, made once in website/.cache/obia/large
  with data.stac and cut into 500, 1000, 2000 and 3000 cells wide windows), segmented whole and by tiles on 1 and 2
  threads, with the seconds of the node and the peak working set of the process (psutil).

Run with: python website/figures/object-based-image-analysis/large-images.py [--seams] [--timings] [--counts]
(the timings take about ten minutes: their results are kept in website/.cache/obia/large/timings.json and used again
unless --timings is given).
"""

import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402

# obia_data sets 4 threads at import: this page runs at most 2
for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "TERRAFLUX_THREADS", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS"):
    os.environ[_k] = "2"

figs = od.figs
SLUG = "large-images"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
LARGE = od.CACHE / "large"
SCALE = 2.14  # what segment.scale suggests first on the clip (part 3)
WHOLE = 100000  # a tile side above the image: the whole image at once

# ----------------------------------------------------------------------------------------------- helpers


def run(model, work, threads=2, *args, name="model.toml"):
    return figs.run(model, work, "--threads", str(threads), "--no-journal", *args, name=name)


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def labels_of(work, name):
    with rasterio.open(work / f"{name}.tif") as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, -1).astype("int64")


def boundary(lab):
    return od.boundary(lab)


def dilate(b, n=1):
    from scipy.ndimage import binary_dilation

    return binary_dilation(b, structure=np.ones((3, 3), bool), iterations=n)


def f_measure(a, b, tol=1):
    """The agreement of the boundaries of two segmentations (cells), each within `tol` cells of the other's."""
    ba, bb = boundary(a), boundary(b)
    p = (ba & dilate(bb, tol)).sum() / max(ba.sum(), 1)
    r = (bb & dilate(ba, tol)).sum() / max(bb.sum(), 1)
    return float(2 * p * r / (p + r)) if p + r else 0.0


def seam_rates(lab, tile):
    """The share (%) of the pairs of neighbouring cells that are in different objects: across the tile borders, and
    across every other line of the image (rows and columns, both directions)."""
    h, w = lab.shape
    vert = (lab[:, :-1] != lab[:, 1:]).mean(axis=0)  # the pair c | c+1, over the rows
    horz = (lab[:-1, :] != lab[1:, :]).mean(axis=1)
    edges_v = [c - 1 for c in range(tile, w, tile)]
    edges_h = [r - 1 for r in range(tile, h, tile)]
    on = np.concatenate([vert[edges_v], horz[edges_h]]).mean()
    off = np.concatenate([np.delete(vert, edges_v), np.delete(horz, edges_h)]).mean()
    return round(float(on) * 100, 1), round(float(off) * 100, 1)


def count(lab):
    return int(len(np.unique(lab[lab >= 0])))


# ---------------------------------------------------------------------------------------------- the seams

TILE = 200  # 580 / 200: three tiles by three (the last 180 cells wide)
TILES = [400, 300, 200, 150, 100, 64]
MARGINS = [1, 4, 16, 32, 64]
SCALES = [1, 2.14, 4, 8]


def seams_model():
    m = od.sentinel2_model()
    writes = {}

    def add(i, op, params, extra=""):
        nonlocal m
        m += node(i, op, '{ image = "refl.image" }', "{ " + params + " }")
        m += node(f"{i}_map", "objects.to_map", f'{{ objects = "{i}.objects" }}', '{ what = "id" }')
        writes[f"{i}_map.map"] = f"{i}.tif"

    add("mr_whole", "segment.multiresolution", f"scale = {SCALE}, tile = {WHOLE}")
    add("mr_whole_s1", "segment.multiresolution", f"scale = {SCALE}, tile = {WHOLE}, seed = 1")
    add("mr_whole_s2", "segment.multiresolution", f"scale = {SCALE}, tile = {WHOLE}, seed = 2")
    add("mr_t200", "segment.multiresolution", f"scale = {SCALE}, tile = {TILE}")
    add("mr_t200_s1", "segment.multiresolution", f"scale = {SCALE}, tile = {TILE}, seed = 1")
    add("mr_t200_m1", "segment.multiresolution", f"scale = {SCALE}, tile = {TILE}, margin = 1")
    for t in TILES:
        if t != TILE:
            add(f"mr_t{t}", "segment.multiresolution", f"scale = {SCALE}, tile = {t}")
    for g in MARGINS:
        if g != 1:
            add(f"mr_t200_m{g}", "segment.multiresolution", f"scale = {SCALE}, tile = {TILE}, margin = {g}")
    for sc in SCALES:
        if sc != SCALE:
            add(f"mr_whole_sc{sc}", "segment.multiresolution", f"scale = {sc}, tile = {WHOLE}")
            add(f"mr_t200_sc{sc}", "segment.multiresolution", f"scale = {sc}, tile = {TILE}")
    add("rg_whole", "segment.region_growing", "threshold = 0.05")
    add("rg_whole_s1", "segment.region_growing", "threshold = 0.05, seed = 1")
    add("rg_t200", "segment.region_growing", f"threshold = 0.05, tile = {TILE}")
    add("rg_t200_m1", "segment.region_growing", f"threshold = 0.05, tile = {TILE}, margin = 1")
    add("rg_whole_ms", "segment.region_growing", "threshold = 0.05, minsize = 20")
    add("rg_whole_ms_s1", "segment.region_growing", "threshold = 0.05, minsize = 20, seed = 1")
    add("rg_t200_ms", "segment.region_growing", f"threshold = 0.05, minsize = 20, tile = {TILE}")
    return figs.add_writes(m, writes)


def draw_window(rgb, lab, w, scale=4, tile=TILE):
    """The outlines of the objects in the window w = (r0, r1, c0, c1), enlarged. A tile border is a faint dashed line,
    magenta where the two cells across it are in different objects."""
    r0, r1, c0, c1 = w
    img = od.outlines(rgb[r0:r1, c0:c1], lab[r0:r1, c0:c1], scale=scale)
    pink, faint = (0.9, 0.1, 0.65), (0.75, 0.9, 1.0)
    for t in range(tile, max(lab.shape), tile):
        if c0 <= t < c1:
            k = (t - c0) * scale
            for r in range(r0, r1):
                px = slice((r - r0) * scale, (r - r0 + 1) * scale)
                if lab[r, t - 1] != lab[r, t]:
                    img[px, max(k - 2, 0):k + 2] = pink
                elif (r - r0) % 2 == 0:
                    img[px, max(k - 1, 0):k + 1] = faint
        if r0 <= t < r1:
            k = (t - r0) * scale
            for c in range(c0, c1):
                px = slice((c - c0) * scale, (c - c0 + 1) * scale)
                if lab[t - 1, c] != lab[t, c]:
                    img[max(k - 2, 0):k + 2, px] = pink
                elif (c - c0) % 2 == 0:
                    img[max(k - 1, 0):k + 1, px] = faint
    return img


def seam_image(lab, tile):
    """The clip (grey) with the objects' outlines and, in magenta, the tile borders where the cells across are in
    different objects: a picture of its own, for a slider."""
    h, w = lab.shape
    rgb = od.rgb("sentinel2")
    base = np.clip(0.45 + 0.55 * rgb.mean(axis=2), 0, 1)
    img = od.outlines(np.stack([base] * 3, axis=-1), lab, color=(0.62, 0.62, 0.62), scale=2)
    pink = (0.85, 0.1, 0.65)
    for c in range(tile, w, tile):
        for r in np.where(lab[:, c - 1] != lab[:, c])[0]:
            img[2 * r:2 * r + 2, 2 * c - 3:2 * c + 3] = pink
    for r in range(tile, h, tile):
        for c in np.where(lab[r - 1, :] != lab[r, :])[0]:
            img[2 * r - 3:2 * r + 3, 2 * c:2 * c + 2] = pink
    return img


def seam_panel(ax, lab, tile, title):
    """The clip with the objects' outlines in grey and, in magenta, the tile borders where the two cells across them
    are in different objects."""
    h, w = lab.shape
    rgb = od.rgb("sentinel2")
    base = np.clip(0.45 + 0.55 * rgb.mean(axis=2), 0, 1)
    img = od.outlines(np.stack([base] * 3, axis=-1), lab, color=(0.62, 0.62, 0.62), scale=2)
    for c in range(tile, w, tile):
        bad = np.where(lab[:, c - 1] != lab[:, c])[0]
        for r in bad:
            img[2 * r:2 * r + 2, 2 * c - 2:2 * c + 2] = (0.85, 0.1, 0.65)
    for r in range(tile, h, tile):
        bad = np.where(lab[r - 1, :] != lab[r, :])[0]
        for c in bad:
            img[2 * r - 2:2 * r + 2, 2 * c:2 * c + 2] = (0.85, 0.1, 0.65)
    ax.imshow(img, interpolation="nearest")
    ax.set_axis_off()
    on, off = seam_rates(lab, tile)
    ax.set_title(f"{title}\n{on:.0f} % of the pairs across the borders differ", fontsize=9.5)


def seams():
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}-seams")
    t0 = time.time()
    run(seams_model(), work, 2)
    print("seams model run", round(time.time() - t0, 1), "s")
    # the same model on 1 thread: the same objects?
    w1 = figs.workdir(f"{od.CATEGORY}/{SLUG}-seams-1")
    run(seams_model(), w1, 1)
    same = {}
    L = {}
    for p in sorted(work.glob("*.tif")):
        if p.stem.startswith(("mr_", "rg_")):
            a, b = labels_of(work, p.stem), labels_of(w1, p.stem)
            L[p.stem] = a
            same[p.stem] = bool(np.array_equal(a, b))
    print("same objects on 1 and 2 threads:", same)
    assert all(same.values()), same

    rgb = od.rgb("sentinel2")
    L["mr_whole_sc2.14"], L["mr_t200_sc2.14"] = L["mr_whole"], L["mr_t200"]
    n = {k: count(v) for k, v in L.items()}
    print("objects", n)

    # --- the window around the crossing of two tile borders
    win = (140, 260, 140, 260)
    for key, name in (("mr_whole", "whole"), ("mr_t200", "tiles"), ("mr_t200_m1", "margin1")):
        od.save_png(draw_window(rgb, L[key], win, scale=5), OUT / f"win_{name}.png", width=600)

    # --- a picture for each tile size (tile lines of 200 drawn on the whole-image run)
    for t in [None] + sorted(TILES, reverse=True):
        lab, side = (L["mr_whole"], TILE) if t is None else (L[f"mr_t{t}"], t)
        od.save_png(seam_image(lab, side), OUT / f"tile_{'whole' if t is None else t}.png", width=640, colors=40)

    # --- the borders of the whole clip
    for stem, keys in (("seams_mr", ("mr_whole", "mr_t200", "mr_t200_m1")), ("seams_rg", ("rg_whole", "rg_t200", "rg_t200_m1"))):
        fig, axs = plt.subplots(1, 3, figsize=(11, 4.1), dpi=150)
        titles = ("Whole image (the lines are only drawn)", "Tiles of 200, margin 64", "Tiles of 200, margin 1")
        for ax, k, t in zip(axs, keys, titles):
            seam_panel(ax, L[k], TILE, t)
        fig.tight_layout()
        fig.savefig(OUT / f"{stem}.png", dpi=150)
        plt.close(fig)
        figs._shrink(OUT / f"{stem}.png")

    # --- the numbers
    def row(label, key, ref):
        on, off = seam_rates(L[key], TILE)
        return {"run": label, "objects": n[key], "vs_whole_f": round(f_measure(L[key], L[ref]), 3) if key != ref else 1.0,
                "on_borders": on, "elsewhere": off}

    rows = [
        row("whole, seed 0", "mr_whole", "mr_whole"),
        row("whole, seed 1 (another run)", "mr_whole_s1", "mr_whole"),
        row("whole, seed 2 (another run)", "mr_whole_s2", "mr_whole"),
        row("tiles of 200, margin 64", "mr_t200", "mr_whole"),
        row("tiles of 200, margin 64, seed 1", "mr_t200_s1", "mr_whole"),
        row("tiles of 200, margin 1", "mr_t200_m1", "mr_whole"),
    ]
    figs.write_csv(DATA / "seams_mr.csv", rows)
    for r in rows:
        print(r)
    rows = [
        row("whole, seed 0", "rg_whole", "rg_whole"),
        row("whole, seed 1 (another run)", "rg_whole_s1", "rg_whole"),
        row("tiles of 200, margin 64", "rg_t200", "rg_whole"),
        row("tiles of 200, margin 1", "rg_t200_m1", "rg_whole"),
        row("whole, minsize 20", "rg_whole_ms", "rg_whole_ms"),
        row("whole, minsize 20, seed 1", "rg_whole_ms_s1", "rg_whole_ms"),
        row("tiles of 200, minsize 20", "rg_t200_ms", "rg_whole_ms"),
    ]
    figs.write_csv(DATA / "seams_rg.csv", rows)
    for r in rows:
        print(r)

    # --- the tile size, the margin and the scale
    base = L["mr_whole"]
    on, off = seam_rates(base, TILE)  # (one tile: the lines of 200 are only imagined)
    rows = [{"tile": 580, "objects": n["mr_whole"], "f": 1.0, "on_borders": on, "elsewhere": off}]
    for t in sorted(TILES, reverse=True):
        k = f"mr_t{t}"
        on, off = seam_rates(L[k], t)
        rows.append({"tile": t, "objects": n[k], "f": round(f_measure(L[k], base), 3), "on_borders": on, "elsewhere": off})
    figs.write_csv(DATA / "tile_size.csv", rows)
    print(rows)
    rows = []
    for g in MARGINS:
        k = f"mr_t200_m{g}" if g != 64 else "mr_t200"
        on, off = seam_rates(L[k], TILE)
        rows.append({"margin": g, "objects": n[k], "f": round(f_measure(L[k], base), 3), "on_borders": on, "elsewhere": off})
    figs.write_csv(DATA / "margin.csv", rows)
    print(rows)
    rows = []
    for s in SCALES:
        a, b = L[f"mr_whole_sc{s}"], L[f"mr_t200_sc{s}"]
        on, off = seam_rates(b, TILE)
        wo, wf = seam_rates(a, TILE)
        rows.append({"scale": s, "objects_whole": count(a), "objects_tiles": count(b), "whole_on_lines": wo,
                     "tiles_on_borders": on, "elsewhere": off})
    figs.write_csv(DATA / "scale.csv", rows)
    print(rows)


# ----------------------------------------------------------------------------------------- the large clip


def large_clip():
    """website/.cache/obia/large: large_refl.tif (reflectance, blue green red nir swir16, 3000 x 3000, 30 km) and its
    windows crop_500, crop_1000, crop_2000 (centred)."""
    LARGE.mkdir(parents=True, exist_ok=True)
    if (LARGE / "crop_2000.tif").exists():
        return
    raw = LARGE / "big_raw.tif"
    if not raw.exists():
        # the 30 km square, EPSG:31983, 10 m cells
        m = (
            'format = "terraflux/1"\n\n[[nodes]]\nid = "s2"\nop = "data.stac"\n'
            f'params = {{ url = "{od.S2_URL}", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], '
            f'datetime = "{od.S2_DATE}", extent = [178420, 8229180, 208420, 8259180], cellsize = {od.S2_CELL}, '
            f'crs = "{od.S2_CRS}" }}\n'
        )
        run(figs.add_writes(m, {"s2.raster": "big_raw.tif"}), LARGE, 2)
    from rasterio.windows import Window

    order = od.S2_BANDS
    with rasterio.open(raw) as d:
        names = list(d.descriptions)
        a = (d.read()[[names.index(b) for b in order]] / 10000).astype("float32")
        prof = d.profile | {"compress": "deflate", "predictor": 3}
        tr = d.transform
    with rasterio.open(LARGE / "large_refl.tif", "w", **prof) as o:
        o.write(a)
        o.descriptions = tuple(order)
    for n in (500, 1000, 2000):
        r0 = (3000 - n) // 2
        p = prof | {"width": n, "height": n, "transform": rasterio.windows.transform(Window(r0, r0, n, n), tr)}
        with rasterio.open(LARGE / f"crop_{n}.tif", "w", **p) as o:
            o.write(a[:, r0:r0 + n, r0:r0 + n])
            o.descriptions = tuple(order)


def clip_path(n):
    return "large_refl.tif" if n == 3000 else f"crop_{n}.tif"


def timing_model(op, n, tile, table=False):
    p = {"segment.multiresolution": f"scale = {SCALE}", "segment.region_growing": "threshold = 0.05, minsize = 20"}[op]
    if tile is not None:
        p += f", tile = {tile}"
    m = (
        'format = "terraflux/1"\n\n[[nodes]]\nid = "img"\nop = "raster.read"\n'
        f'params = {{ path = "{clip_path(n)}" }}\n\n[[nodes]]\nid = "seg"\nop = "{op}"\ninputs = {{ image = "img.raster" }}\n'
        f"params = {{ {p} }}\n\n"
    )
    if table:
        m += '[outputs.table]\nfrom = "seg.table"\n'
    return m


def timed(op, n, tile, threads, table=False):
    """One run: the seconds of the segmenter's node (from --profile), the wall seconds and the peak working set (MB)."""
    import psutil

    tag = f"{op.split('.')[1]}_{n}_{tile}_{threads}{'_t' if table else ''}"
    work = LARGE / "runs" / tag
    work.mkdir(parents=True, exist_ok=True)
    for f in LARGE.glob("*.tif"):
        link = work / f.name
        if not link.exists():
            try:
                os.link(f, link)
            except OSError:
                import shutil

                shutil.copy(f, link)
    model = timing_model(op, n, tile, table)
    (work / "model.toml").write_text(model, encoding="utf-8")
    t0 = time.time()
    # the log goes to a file: a pipe nobody reads fills (4 KB on Windows) and stops the run at its next write
    log = tempfile.TemporaryFile("w+", encoding="utf-8")
    p = subprocess.Popen([str(figs.binary()), "run", "model.toml", "--out", "out", "--threads", str(threads), "--no-journal",
                          "--profile"], cwd=work, stdout=log, stderr=subprocess.STDOUT, text=True, encoding="utf-8")
    ps = psutil.Process(p.pid)
    peak = 0
    while p.poll() is None:
        try:
            mi = ps.memory_info()
            peak = max(peak, getattr(mi, "peak_wset", mi.rss))
        except psutil.Error:
            pass
        time.sleep(0.05)
    log.seek(0)
    out = log.read()
    log.close()
    wall = time.time() - t0
    if p.returncode != 0:
        raise SystemExit(f"{tag} failed:\n{out}")
    m = re.search(r"^\s*([\d.]+)\s+[\d.]+%\s+\S+\s+\S+\s+seg\s*$", out, re.M)
    sec = float(m.group(1)) if m else float("nan")
    cnt = None
    if table:
        cnt = sum(1 for _ in open(work / "out" / "table.csv", encoding="utf-8")) - 1
    return {"op": op, "size": n, "tile": tile if tile is not None else 0, "threads": threads, "seconds": sec,
            "wall": round(wall, 2), "peak_mb": round(peak / 1e6), "objects": cnt}


CASES = {
    # (op, sizes)
    "segment.multiresolution": [500, 1000, 2000, 3000],
    "segment.region_growing": [500, 1000, 2000],
}


def timings(redo=False):
    f = LARGE / "timings.json"
    if f.exists() and not redo:
        return json.loads(f.read_text())
    large_clip()
    res = []
    for op, sizes in CASES.items():
        for n in sizes:
            for tile, threads in ((WHOLE, 1), (WHOLE, 2), (512, 1), (512, 2)):
                best = None
                for rep in range(2 if n <= 2000 else 1):
                    r = timed(op, n, tile, threads)
                    if best is None or r["seconds"] < best["seconds"]:
                        best = r
                print(best, flush=True)
                res.append(best)
            f.write_text(json.dumps(res, indent=1))
    return res


def layouts():
    """The same 2000 x 2000 window, tile 512, read from a striped GeoTIFF (as written above), from a tiled one, and from
    memory (a raster.calc copy): the seconds of the segmenter on 1 and 2 threads and the peak working set."""
    f = LARGE / "layouts.json"
    if f.exists():
        return json.loads(f.read_text())
    large_clip()
    src = LARGE / "crop_2000.tif"
    tiled = LARGE / "crop_2000_tiled.tif"
    if not tiled.exists():
        with rasterio.open(src) as d:
            prof = d.profile | {"tiled": True, "blockxsize": 256, "blockysize": 256, "compress": "deflate", "predictor": 3}
            a = d.read()
            names = d.descriptions
        with rasterio.open(tiled, "w", **prof) as o:
            o.write(a)
            o.descriptions = names
    global clip_path, timing_model
    keep_clip, keep_model = clip_path, timing_model
    res = []
    try:
        for name, path, mem in (("striped GeoTIFF", "crop_2000.tif", False), ("tiled GeoTIFF", "crop_2000_tiled.tif", False),
                                ("in memory", "crop_2000.tif", True)):
            for th in (1, 2):
                clip_path = lambda n, p=path: p  # noqa: E731
                if mem:
                    def timing_model(op, n, tile, table=False, path=path):  # noqa: F811
                        return (
                            'format = "terraflux/1"\n\n[[nodes]]\nid = "disk"\nop = "raster.read"\n'
                            f'params = {{ path = "{path}" }}\n\n[[nodes]]\nid = "img"\nop = "raster.calc"\n'
                            'inputs = { s = "disk.raster" }\nparams = { expression = "out image = s" }\n\n'
                            f'[[nodes]]\nid = "seg"\nop = "{op}"\ninputs = {{ image = "img.image" }}\n'
                            f"params = {{ scale = {SCALE}, tile = {tile} }}\n"
                        )
                else:
                    timing_model = keep_model
                r = timed("segment.multiresolution", 2000, 512, th)
                r["layout"] = name
                print(r, flush=True)
                res.append(r)
    finally:
        clip_path, timing_model = keep_clip, keep_model
    f.write_text(json.dumps(res, indent=1))
    return res


def counts():
    f = LARGE / "counts.json"
    large_clip()
    res = []
    for op, sizes in CASES.items():
        for n in sizes:
            for tile in (WHOLE, 512):
                r = timed(op, n, tile, 2, table=True)
                print(r, flush=True)
                res.append(r)
    f.write_text(json.dumps(res, indent=1))
    return res


# ------------------------------------------------------------------------------------- drawing and tables


def scheme():
    """How the tiles work, on a toy: the cores and one window with its margin, two neighbouring windows each segmented
    on its own, and the pieces that meet at the border."""
    from matplotlib.patches import Ellipse, Rectangle

    fig, axs = plt.subplots(1, 3, figsize=(11, 3.9), dpi=150)
    F, E, W, INK = figs.FOREST, figs.EMBER, figs.WATER, figs.INK
    for ax in axs:
        ax.set_xlim(0, 30)
        ax.set_ylim(30, 0)
        ax.set_aspect("equal")
        ax.set_xticks([])
        ax.set_yticks([])
        ax.grid(False)
        for sp in ax.spines.values():
            sp.set_visible(True)
            sp.set_color(figs.RULE)
    # 1: the image in 3 x 3 tiles, one with its window
    ax = axs[0]
    for i in range(3):
        for j in range(3):
            ax.add_patch(Rectangle((10 * j, 10 * i), 10, 10, fill=False, ec=INK, lw=1.2))
    ax.add_patch(Rectangle((10, 10), 10, 10, fc=F, alpha=0.25, ec="none"))
    ax.add_patch(Rectangle((7, 7), 16, 16, fill=False, ec=E, lw=1.8, ls=(0, (4, 3))))
    ax.text(15, 15, "core", ha="center", va="center", color=INK, fontsize=9)
    ax.text(15, 8.4, "margin", ha="center", va="center", color=E, fontsize=9)
    ax.set_title("1. Tiles with a margin:\nthe window is core + margin", fontsize=9.5)
    # 2: two neighbouring windows, each segmented on its own
    ax = axs[1]
    for x0, c, name in ((3, F, "window of the left tile"), (13, W, "window of the right tile")):
        ax.add_patch(Rectangle((x0, 9), 14, 12, fill=False, ec=c, lw=1.6))
    ax.plot([15, 15], [8, 24], color=INK, lw=1.2)
    ax.add_patch(Ellipse((13.2, 15), 9, 7, fc=F, alpha=0.45, ec=F))
    ax.add_patch(Ellipse((17.2, 15.6), 8, 6.4, fc=W, alpha=0.45, ec=W))
    ax.text(10, 23.5, "left tile's objects", ha="center", color=F, fontsize=8.5)
    ax.text(21, 23.5, "right tile's objects", ha="center", color=W, fontsize=8.5)
    ax.text(15, 6.6, "tile border", ha="center", color=INK, fontsize=8.5)
    ax.set_title("2. Each window is segmented alone:\nthe two disagree in the overlap", fontsize=9.5)
    # 3: the cores are kept, the pieces meet at the border
    ax = axs[2]
    for x0, c in ((3, F), (15, W)):
        ax.add_patch(Rectangle((x0, 9), 12, 12, fill=False, ec=c, lw=1.0, ls=(0, (2, 2))))
    # the left tile keeps the part of its object left of the border, the right tile the part of its own on the right
    left = Ellipse((13.2, 15), 9, 7, fc=F, alpha=0.5, ec=F)
    right = Ellipse((17.2, 15.6), 8, 6.4, fc=W, alpha=0.5, ec=W)
    ax.add_patch(left)
    ax.add_patch(right)
    left.set_clip_path(Rectangle((3, 9), 12, 12, transform=ax.transData))
    right.set_clip_path(Rectangle((15, 9), 12, 12, transform=ax.transData))
    ax.plot([15, 15], [8, 24], color=INK, lw=1.4)
    ax.text(15, 6.4, "two pieces: merged again only if\ntheir cost is under scale squared", ha="center", color=INK,
            fontsize=8.2)
    ax.set_title("3. Only the cores are kept:\nthe pieces at the border may merge", fontsize=9.5)
    fig.subplots_adjust(left=0.02, right=0.98, top=0.82, bottom=0.03, wspace=0.06)
    fig.savefig(OUT / "scheme.png", dpi=150)
    plt.close(fig)
    figs._shrink(OUT / "scheme.png")


def emit_timings():
    """The charts' and tables' data from the runs of `timings`."""
    res = json.loads((LARGE / "timings.json").read_text())
    rows = []
    for r in res:
        cells = r["size"] ** 2 / 1e6
        rows.append({"op": r["op"].split(".")[1], "size": r["size"], "cells_m": round(cells, 2),
                     "tiles": "no" if r["tile"] >= r["size"] else str(r["tile"]), "threads": r["threads"],
                     "seconds": r["seconds"], "peak_mb": r["peak_mb"]})
    figs.write_csv(DATA / "timings.csv", rows)
    for op in ("multiresolution", "region_growing"):
        by = {}
        for r in rows:
            if r["op"] == op:
                by.setdefault(r["size"], {})[(r["tiles"] != "no", r["threads"])] = r
        wide = []
        for size, d in sorted(by.items()):
            if size < 1000:
                continue  # a tile of 512 holds the whole of 500 x 500: no tiles there
            row = {"cells_m": round(size ** 2 / 1e6, 2), "size": size}
            for tiled in (False, True):
                for th in (1, 2):
                    r = d.get((tiled, th))
                    key = f"{'tiles' if tiled else 'whole'}_{th}"
                    row[key] = r["seconds"] if r else ""
                    row[f"mb_{key}"] = r["peak_mb"] if r else ""
            wide.append(row)
        figs.write_csv(DATA / f"time_{op}.csv", wide)
        print(op)
        for w in wide:
            print(w)

    # the objects found, whole and by tiles, on 2 threads
    cnt = json.loads((LARGE / "counts.json").read_text())
    rows = []
    for op in ("segment.multiresolution", "segment.region_growing"):
        for size in sorted({r["size"] for r in cnt if r["op"] == op}):
            if size < 1000:
                continue
            a = next(r for r in cnt if r["op"] == op and r["size"] == size and r["tile"] >= size)
            b = next(r for r in cnt if r["op"] == op and r["size"] == size and r["tile"] < size)
            rows.append({"method": op.split(".")[1], "size": size, "whole": a["objects"], "tiles": b["objects"],
                         "difference_pct": round((b["objects"] / a["objects"] - 1) * 100, 1)})
    figs.write_csv(DATA / "counts.csv", rows)
    for r in rows:
        print(r)
    lay = json.loads((LARGE / "layouts.json").read_text())
    rows = [{"layout": r["layout"], "threads": r["threads"], "seconds": r["seconds"], "peak_mb": r["peak_mb"]} for r in lay]
    figs.write_csv(DATA / "layouts.csv", rows)


def main():
    args = set(sys.argv[1:])
    if not args or "--seams" in args:
        od.sentinel2()
        seams()
    if not args or "--timings" in args:
        timings(redo="--timings" in args)
        emit_timings()
    if not args or "--scheme" in args:
        scheme()
    if "--emit" in args:
        emit_timings()
    if "--layouts" in args:
        layouts()
    if "--counts" in args:
        counts()


if __name__ == "__main__":
    main()
