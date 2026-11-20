"""Figures of "Deep segmentation" (tutorials/object-based-image-analysis/deep-segmentation.md).

Real runs of segment.sam (Segment Anything, ONNX Runtime, the 75 MB quantised weights in Terraflux's cache) and of
objects.embed (a small Sentinel-2 encoder exported to ONNX here), on the series' two clips (obia_data.py), at most 2
cores: ONNX Runtime sets its own threads (Terraflux's cap, 15 here), so the processes are pinned to two logical
cores with psutil, and everything else is at 2 threads.

* SAM on the Sentinel-2 clip with 4, 8 and 16 prompt points per side, and on the NAIP clip, against the classical
  segmenters at the same number of objects.
* objects.embed with the encoder of ResNet-18 pretrained by SSL4EO-S12 (MoCo, Wang et al. 2023) from torchgeo's
  weights file in the torch cache (13 Sentinel-2 bands; its first layer is cut to the five bands of the clip), then
  k-means on the embeddings against k-means on the mean bands, scored on the 80 reference points of part 9.

Run with: python website/figures/object-based-image-analysis/deep-segmentation.py [--sam] [--embed] [--draw]
(--sam and --embed take about ten minutes each: their label maps are kept in website/.cache/obia/deep and --draw
makes the figures from them). Needs onnxruntime (pip) for SAM; torch, timm and onnx for the encoder (onnx may be on
PYTHONPATH after pip install --target).
"""

import csv
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "TERRAFLUX_THREADS", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS"):
    os.environ[_k] = "2"

figs = od.figs
SLUG = "deep-segmentation"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
DEEP = od.CACHE / "deep"
REFERENCE = od.DOWNLOADS / "classes-without-samples" / "reference_samples.csv"  # part 9's points, read only
CORES = [0, 1]  # the logical cores the model runs on

# the display window of the series' figures (rows, columns of the Sentinel-2 clip) and one of NAIP
S2_WIN = (90, 360, 200, 470)
NAIP_WIN = (250, 550, 350, 650)


# ------------------------------------------------------------------------------------------------ running


def ort_library() -> pathlib.Path:
    """The shared library of the onnxruntime Python package (it ships one in its capi folder)."""
    spec = importlib.util.find_spec("onnxruntime")
    if spec is None or not spec.submodule_search_locations:
        raise SystemExit("onnxruntime is not installed: pip install onnxruntime")
    capi = pathlib.Path(spec.submodule_search_locations[0]) / "capi"
    for n in ("onnxruntime.dll", "libonnxruntime.so", "libonnxruntime.dylib"):
        if (capi / n).exists():
            return capi / n
    found = sorted(capi.glob("libonnxruntime.so*"))
    if found:
        return found[0]
    raise SystemExit(f"no onnxruntime library in {capi}")


def run(model: str, work: pathlib.Path, name="model.toml", limit: float | None = None) -> dict:
    """terraflux run in `work`, ONNX Runtime found, 2 threads and two logical cores. Returns the seconds of every node
    (--profile), the wall seconds and the peak working set (MB)."""
    import psutil

    work.mkdir(parents=True, exist_ok=True)
    (work / name).write_text(model, encoding="utf-8")
    env = os.environ | {"ORT_DYLIB_PATH": str(ort_library())}
    t0 = time.time()
    p = subprocess.Popen([str(figs.binary()), "run", name, "--out", "out", "--threads", "2", "--no-journal", "--profile"],
                         cwd=work, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8")
    ps = psutil.Process(p.pid)
    try:
        ps.cpu_affinity(CORES)
    except (AttributeError, psutil.Error):
        pass
    peak = 0
    while p.poll() is None:
        if limit and time.time() - t0 > limit:
            p.kill()
            raise TimeoutError(f"{work}: no end after {limit} s")
        try:
            mi = ps.memory_info()
            peak = max(peak, getattr(mi, "peak_wset", mi.rss))
        except psutil.Error:
            pass
        time.sleep(0.1)
    out = p.stdout.read()
    if p.returncode != 0:
        raise SystemExit(f"terraflux run failed in {work}:\n{out}")
    nodes = {}
    for line in out.splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[0].replace(".", "").isdigit() and parts[1].endswith("%"):
            nodes[parts[-1]] = float(parts[0])
    return {"nodes": nodes, "wall": round(time.time() - t0, 1), "peak_mb": round(peak / 1e6), "log": out}


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def labels_of(path):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, -1).astype("int64")


def count(lab):
    return int(len(np.unique(lab[lab >= 0])))


def with_map(m, writes, key, objects_ref):
    """Adds objects.to_map (the id) and the write of its map."""
    m += node(f"{key}_map", "objects.to_map", f'{{ objects = "{objects_ref}" }}', '{ what = "id" }')
    writes[f"{key}_map.map"] = f"{key}.tif"
    return m


def s2_image():
    return od.sentinel2_model()  # s2, utm, refl (refl.image, reflectance 0-1)


def naip_extent(win=NAIP_WIN):
    """The extent (xmin, ymin, xmax, ymax, metres) of a window (rows, columns) of the NAIP clip."""
    with rasterio.open(od.naip()) as ds:
        tr = ds.transform
    r0, r1, c0, c1 = win
    return [tr.c + c0 * tr.a, tr.f + r1 * tr.e, tr.c + c1 * tr.a, tr.f + r0 * tr.e]


def naip_image(work, window=False):
    """The NAIP clip copied beside the model; the node `naip` reads it, `nref` is the same scaled to 0-1. With `window`
    the node `crop` is the window of NAIP_WIN (300 x 300 cells) and `nref` is the window scaled."""
    import shutil

    shutil.copy(od.naip(), work / "naip.tif")
    m = od.naip_model()
    if window:
        m += node("crop", "raster.clip", '{ raster = "naip.raster" }', f"{{ extent = {naip_extent()} }}")
        m += node("nref", "raster.calc", '{ s = "crop.raster" }', '{ expression = "out image = s / 255" }')
    else:
        m += node("nref", "raster.calc", '{ s = "naip.raster" }', '{ expression = "out image = s / 255" }')
    return m


# ------------------------------------------------------------------------------------------ SAM runs

S2_POINTS = [4, 8, 16]
NAIP_POINTS = [8, 16]
NAIPW_POINTS = [8, 16, 24]


def sam_stage():
    DEEP.mkdir(parents=True, exist_ok=True)
    res = {}
    # --- Sentinel-2: SAM at three grids of prompts, SAM without the fill, and the classical segmenters
    work = DEEP / "work-s2-sam"
    for pts in S2_POINTS + ["16_nofill"]:
        key = f"s2_sam{pts}"
        fill = "true" if "nofill" not in str(pts) else "false"
        n = int(str(pts).split("_")[0])
        m = s2_image()
        m += node("sam", "segment.sam", '{ image = "refl.image" }', f'{{ bands = ["red", "green", "blue"], points = {n}, fill = {fill} }}')
        writes = {}
        m = with_map(m, writes, key, "sam.objects")
        r = run(figs.add_writes(m, writes), work / key)
        os.replace(work / key / f"{key}.tif", DEEP / f"{key}.tif")
        res[key] = {"seconds": r["nodes"].get("sam"), "wall": r["wall"], "peak_mb": r["peak_mb"], "points": n,
                    "fill": fill == "true"}
        print(key, res[key], count(labels_of(DEEP / f"{key}.tif")), flush=True)
    # --- NAIP
    for pts in NAIP_POINTS:
        key = f"naip_sam{pts}"
        w = DEEP / "work-naip" / key
        w.mkdir(parents=True, exist_ok=True)
        m = naip_image(w)
        m += node("sam", "segment.sam", '{ image = "naip.raster" }', f"{{ bands = [\"red\", \"green\", \"blue\"], points = {pts} }}")
        writes = {}
        m = with_map(m, writes, key, "sam.objects")
        r = run(figs.add_writes(m, writes), w)
        os.replace(w / f"{key}.tif", DEEP / f"{key}.tif")
        res[key] = {"seconds": r["nodes"].get("sam"), "wall": r["wall"], "peak_mb": r["peak_mb"], "points": pts, "fill": True}
        print(key, res[key], count(labels_of(DEEP / f"{key}.tif")), flush=True)
    (DEEP / "sam.json").write_text(json.dumps(res, indent=1))
    return res


def naip_window_stage():
    """SAM on a 300 x 300 cell window of NAIP (resized to the 1024 of SAM's input, so a prompt is a third as far apart in
    ground cells as on the whole clip)."""
    f = DEEP / "sam.json"
    res = json.loads(f.read_text()) if f.exists() else {}
    for pts in NAIPW_POINTS:
        key = f"naipw_sam{pts}"
        w = DEEP / "work-naipw" / key
        w.mkdir(parents=True, exist_ok=True)
        m = naip_image(w, window=True)
        m += node("sam", "segment.sam", '{ image = "crop.raster" }', f'{{ bands = ["red", "green", "blue"], points = {pts} }}')
        writes = {}
        m = with_map(m, writes, key, "sam.objects")
        r = run(figs.add_writes(m, writes), w)
        os.replace(w / f"{key}.tif", DEEP / f"{key}.tif")
        res[key] = {"seconds": r["nodes"].get("sam"), "wall": r["wall"], "peak_mb": r["peak_mb"], "points": pts, "fill": True}
        print(key, res[key], count(labels_of(DEEP / f"{key}.tif")), flush=True)
        f.write_text(json.dumps(res, indent=1))


def classical_stage():
    """Multiresolution and SLIC at a range of settings, to find the run with about the number of objects of SAM, on the
    Sentinel-2 clip and on the NAIP window."""
    for clip in ("s2", "naipw"):
        w = DEEP / f"work-{clip}-classical"
        w.mkdir(parents=True, exist_ok=True)
        if clip == "s2":
            base = s2_image()
            ref = "refl.image"
            scales = [1, 1.5, 2, 2.5, 3, 4, 6, 8, 9, 10, 12]
            sizes = [8, 12, 16, 20, 25, 30, 40, 46, 50, 60]
        else:
            base = naip_image(w, window=True)
            ref = "nref.image"
            scales = [0.6, 1.0, 2, 3, 4, 6, 8, 10, 12]
            sizes = [10, 14, 18, 22, 26, 30, 40]
        m = base
        writes = {}
        for s in scales:
            k = f"{clip}_mr_{str(s).replace('.', '_')}"
            m += node(k, "segment.multiresolution", f'{{ image = "{ref}" }}', f"{{ scale = {s} }}")
            m = with_map(m, writes, k, f"{k}.objects")
        for z in sizes:
            k = f"{clip}_slic_{z}"
            m += node(k, "segment.slic", f'{{ image = "{ref}" }}', f"{{ size = {z} }}")
            m = with_map(m, writes, k, f"{k}.objects")
        run(figs.add_writes(m, writes), w)
        for p in sorted(w.glob("*.tif")):
            if p.stem.startswith(clip + "_"):
                os.replace(p, DEEP / f"{p.stem}.tif")
        print(clip, "classical", {p.stem: count(labels_of(p)) for p in sorted(DEEP.glob(f"{clip}_*.tif"))}, flush=True)


# ------------------------------------------------------------------------------------ the encoder

BANDS13 = ["B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8", "B8A", "B9", "B10", "B11", "B12"]
USE = {"blue": "B2", "green": "B3", "red": "B4", "nir": "B8", "swir16": "B11"}
WEIGHTS = pathlib.Path.home() / ".cache" / "torch" / "hub" / "checkpoints" / "resnet18_sentinel2_all_moco-59bfdff9.pth"
ENCODER = DEEP / "ssl4eo_resnet18_5band.onnx"


def make_encoder():
    """ResNet-18 pretrained on Sentinel-2 by MoCo (SSL4EO-S12, torchgeo's weights), its first layer cut to the five
    bands of the clip, exported to ONNX as 1 x 5 x P x P in and 1 x 512 out (P free)."""
    if ENCODER.exists():
        return ENCODER
    extra = os.environ.get("TF_PYLIB")  # a folder with onnx (pip install --target), searched last
    if extra:
        sys.path.append(extra)
    import timm
    import torch

    torch.set_num_threads(2)
    sd = torch.load(WEIGHTS, map_location="cpu", weights_only=True)
    net = timm.create_model("resnet18", in_chans=13, num_classes=0, pretrained=False)
    print("load:", net.load_state_dict(sd, strict=True))
    keep = [BANDS13.index(b) for b in USE.values()]
    conv = net.conv1
    new = torch.nn.Conv2d(len(keep), conv.out_channels, conv.kernel_size, conv.stride, conv.padding, bias=False)
    new.weight.data = conv.weight.data[:, keep].clone()
    net.conv1 = new
    net.eval()
    DEEP.mkdir(parents=True, exist_ok=True)
    x = torch.zeros(1, len(keep), 32, 32)
    torch.onnx.export(net, x, str(ENCODER), input_names=["x"], output_names=["y"], opset_version=17, dynamo=False,
                      dynamic_axes={"x": {2: "h", 3: "w"}})
    return ENCODER


PATCHES = [8, 16, 32, 64]


def run_retry(model: str, work: pathlib.Path, tries: int = 4, limit: float = 150) -> dict:
    """`run`, started again when it does not end in `limit` seconds: with the build of 5022b27 a model with several
    branches next to an objects.embed stopped now and then (see the report), and none of these runs takes 30 s."""
    for k in range(tries):
        try:
            return run(model, work, limit=limit)
        except TimeoutError as e:
            print("retry:", e, flush=True)
    raise SystemExit(f"{work}: no end after {tries} tries")


def embed_stage():
    """The multiresolution objects of the Sentinel-2 clip embedded with patches of 8, 16, 32 and 64 cells, each grouped
    by k-means (5), and the same objects grouped by their mean bands. One run for each, with a time limit."""
    DEEP.mkdir(parents=True, exist_ok=True)
    make_encoder()
    import shutil

    names = ", ".join(f'"{b}"' for b in od.S2_BANDS)
    drop = ["id", "cells", "area"] + [f"{p}_{b}" for p in ("mean", "sd") for b in od.S2_BANDS]
    dropt = ", ".join(f'"{c}"' for c in drop)
    out = {"nodes": {}, "wall": 0.0, "peak_mb": 0}
    for p in PATCHES:
        w = DEEP / "work-embed" / f"p{p}"
        w.mkdir(parents=True, exist_ok=True)
        shutil.copy(ENCODER, w / ENCODER.name)
        m = s2_image()
        m += node("segs", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2.14 }")
        writes = {f"emb{p}.table": f"embeddings{p}.csv"}
        m += node(f"emb{p}", "objects.embed", '{ objects = "segs.objects", image = "refl.image" }',
                  f'{{ model = "{ENCODER.name}", patch = {p}, bands = [{names}], normalize = false }}')
        m += node(f"sel{p}", "table.select", f'{{ table = "emb{p}.table" }}', f"{{ drop = [{dropt}] }}")
        m += node(f"km{p}", "ml.kmeans", f'{{ table = "sel{p}.table" }}', "{ k = 5 }")
        m += node(f"cl{p}", "objects.classify", f'{{ objects = "emb{p}.objects", model = "km{p}.model" }}', "{ }")
        m += node(f"cl{p}_cls", "objects.to_map", f'{{ objects = "cl{p}.objects" }}', '{ what = "class" }')
        writes[f"cl{p}_cls.map"] = f"cl{p}.tif"
        if p == PATCHES[0]:
            m = with_map(m, writes, "segs", "segs.objects")
        r = run_retry(figs.add_writes(m, writes), w)
        print(r["log"][-900:], flush=True)
        os.replace(w / f"cl{p}.tif", DEEP / f"embed_cl{p}.tif")
        if p == PATCHES[0]:
            os.replace(w / "segs.tif", DEEP / "embed_segs.tif")
        os.replace(w / "out" / f"embeddings{p}.csv", DEEP / f"embeddings{p}.csv")
        out["nodes"].update({f"p{p}_{k}": v for k, v in r["nodes"].items()})
        out["wall"] += r["wall"]
        out["peak_mb"] = max(out["peak_mb"], r["peak_mb"])
    # the same objects grouped by their mean bands
    w = DEEP / "work-embed" / "mean"
    w.mkdir(parents=True, exist_ok=True)
    m = s2_image()
    m += node("segs", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2.14 }")
    m += node("feat", "objects.features", '{ objects = "segs.objects", image = "refl.image" }', '{ features = ["mean"] }')
    cols = ", ".join(f'"mean_{b}"' for b in od.S2_BANDS)
    m += node("km_mean", "ml.kmeans", '{ table = "feat.table" }', f"{{ k = 5, columns = [{cols}] }}")
    m += node("cl_mean", "objects.classify", '{ objects = "feat.objects", model = "km_mean.model" }', "{ }")
    m += node("cl_mean_cls", "objects.to_map", '{ objects = "cl_mean.objects" }', '{ what = "class" }')
    run_retry(figs.add_writes(m, {"cl_mean_cls.map": "cl_mean.tif"}), w)
    os.replace(w / "cl_mean.tif", DEEP / "embed_cl_mean.tif")
    (DEEP / "embed.json").write_text(json.dumps(out, indent=1))
    print(out)


# --------------------------------------------------------------------------------------------- drawing


def lab(name):
    return labels_of(DEEP / f"{name}.tif")


def s2_stack():
    return od.read_stack(od.sentinel2())


def naip_stack(win=None):
    a = od.read_stack(od.naip())
    if win:
        r0, r1, c0, c1 = win
        a = a[:, r0:r1, c0:c1]
    return a


def inside_variance(stack, labels):
    """The share of the image's variance (mean over the bands) that is left inside the objects: 0 when every object is
    one value, 1 when the objects explain nothing."""
    ok = labels >= 0
    ids = labels[ok]
    shares = []
    for band in stack:
        v = band[ok]
        good = ~np.isnan(v)
        size = int(ids.max()) + 1
        s1 = np.bincount(ids[good], weights=v[good], minlength=size)
        s2 = np.bincount(ids[good], weights=v[good] ** 2, minlength=size)
        nn = np.bincount(ids[good], minlength=size)
        within = (s2 - np.where(nn > 0, s1 ** 2 / np.maximum(nn, 1), 0)).sum()
        total = ((v[good] - v[good].mean()) ** 2).sum()
        shares.append(within / total)
    return float(np.mean(shares))


def shore_error(labels, stack):
    """The share (%) of the cells that are on the other side of the lake shore than most of their object (water against
    land by the NDWI of part 3, cut at 0.065)."""
    green, nir = stack[1], stack[3]
    ndwi = (green - nir) / (green + nir)
    water = np.where(np.isnan(ndwi), -1, (ndwi >= 0.065).astype(int))
    ok = (labels >= 0) & (water >= 0)
    ids = labels[ok]
    w = (water[ok] == 1).astype(float)
    n = np.bincount(ids)
    sw = np.bincount(ids, weights=w)
    major = (sw[ids] / n[ids]) > 0.5
    return float(((w > 0.5) != major).mean() * 100)


def nearest(prefix, target):
    """The classical run (files `prefix_*.tif`) whose number of objects is the nearest to `target`."""
    best = None
    for p in sorted(DEEP.glob(f"{prefix}_*.tif")):
        n = count(labels_of(p))
        if best is None or abs(n - target) < abs(best[1] - target):
            best = (p.stem, n)
    return best


def window_png(rgb, labels, win, name, scale=2, width=640):
    r0, r1, c0, c1 = win
    od.save_png(od.outlines(rgb[r0:r1, c0:c1], labels[r0:r1, c0:c1], scale=scale), OUT / name, width=width, colors=64)


def draw():
    sam = json.loads((DEEP / "sam.json").read_text())
    s2 = s2_stack()
    rgb2 = od.rgb("sentinel2")
    rgbn = od.rgb("naip")
    # --- Sentinel-2: the slider of the prompts
    for p in S2_POINTS:
        window_png(rgb2, lab(f"s2_sam{p}"), S2_WIN, f"s2_sam{p}.png")
    # --- the runs: time against prompts
    rows = []
    for key, r in sam.items():
        clip = {"s2": "Sentinel-2 clip", "naip": "NAIP clip", "naipw": "NAIP window"}[key.split("_")[0]]
        rows.append({"run": key, "clip": clip, "points": r["points"], "prompts": r["points"] ** 2, "fill": r["fill"],
                     "objects": count(lab(key)), "seconds": round(r["seconds"], 1), "peak_mb": r["peak_mb"]})
    rows.sort(key=lambda r: (r["clip"], r["prompts"], not r["fill"]))
    figs.write_csv(DATA / "sam_runs.csv", rows)
    for r in rows:
        print(r)
    ok = [r for r in rows if r["fill"]]
    b, a = np.polyfit([r["prompts"] for r in ok], [r["seconds"] for r in ok], 1)
    print(f"seconds = {a:.1f} + {b:.2f} x prompts")
    # --- Sentinel-2: SAM against the classical segmenters at about its number of objects
    t16 = count(lab("s2_sam16"))
    mr = nearest("s2_mr", t16)
    sl = nearest("s2_slic", t16)
    cmp = []
    for label, key in (("SAM, 16 points", "s2_sam16"),
                       (f"Multiresolution, scale {mr[0].split('_', 2)[2].replace('_', '.')}", mr[0]),
                       (f"SLIC, size {sl[0].split('_')[2]}", sl[0])):
        L = lab(key)
        cmp.append({"method": label, "objects": count(L), "mean_ha": round(580 * 580 * 0.01 / count(L), 2),
                    "inside_variance": round(inside_variance(s2, L), 3), "shore_error_pct": round(shore_error(L, s2), 2)})
    figs.write_csv(DATA / "compare_s2.csv", cmp)
    for r in cmp:
        print(r)
    window_png(rgb2, lab(mr[0]), S2_WIN, "s2_mr.png")
    # --- the NAIP window
    nw = NAIP_WIN
    wr = naip_stack(nw)
    rgbw = rgbn[nw[0]:nw[1], nw[2]:nw[3]]
    full = (0, nw[1] - nw[0], 0, nw[3] - nw[2])
    for p in NAIPW_POINTS:
        window_png(rgbw, lab(f"naipw_sam{p}"), full, f"naip_sam{p}.png", scale=3, width=640)
    t = count(lab(f"naipw_sam{max(NAIPW_POINTS)}"))
    mrn = nearest("naipw_mr", t)
    sln = nearest("naipw_slic", t)
    entries = [(f"SAM, {p} points", f"naipw_sam{p}") for p in NAIPW_POINTS]
    entries += [(f"Multiresolution, scale {mrn[0].split('_', 2)[2].replace('_', '.')}", mrn[0]),
                (f"SLIC, size {sln[0].split('_')[2]}", sln[0])]
    cmp = []
    for label, key in entries:
        L = lab(key)
        cmp.append({"method": label, "objects": count(L), "mean_cells": round(300 * 300 / count(L)),
                    "inside_variance": round(inside_variance(wr, L), 3)})
    figs.write_csv(DATA / "compare_naip.csv", cmp)
    for r in cmp:
        print(r)
    window_png(rgbw, lab(mrn[0]), full, "naip_mr.png", scale=3, width=640)
    window_png(rgbw, lab(sln[0]), full, "naip_slic.png", scale=3, width=640)
    od.save_png(rgbw, OUT / "naip_window.png", width=640, colors=64)
    # the whole NAIP clip with 16 points: the grid is too sparse for houses
    window_png(rgbn, lab("naip_sam16"), (0, 1000, 0, 1000), "naip_whole_sam16.png", scale=1, width=640)
    embed_draw()


def embed_draw():
    if not (DEEP / "embed.json").exists():
        return
    from collections import Counter

    from matplotlib import colors as mc

    keys = ["cl_mean"] + [f"cl{p}" for p in PATCHES]
    labels = {k: lab(f"embed_{k}") for k in keys}
    rgb2 = od.rgb("sentinel2")
    r0, r1, c0, c1 = S2_WIN
    ref = list(csv.DictReader(open(REFERENCE, encoding="utf-8")))
    x0, y1 = od.S2_UTM_EXTENT[0], od.S2_UTM_EXTENT[3]
    pts = [(int((y1 - float(r["y"])) // 10), int((float(r["x"]) - x0) // 10), r["class"]) for r in ref]
    pal = {1: "#0072b2", 2: "#e69f00", 3: "#009e73", 4: "#cc79a7", 5: "#d55e00"}

    def accuracy(L):
        by = {}
        for r, c, cl in pts:
            by.setdefault(int(L[r, c]), []).append(cl)
        return sum(Counter(v).most_common(1)[0][1] for v in by.values()) / len(pts), by

    import numpy as _np

    rows = []
    acc, by = accuracy(labels["cl_mean"])
    rows.append({"grouping": "mean bands (5 columns)", "patch": "", "distinct_vectors": "", "constant_columns": "",
                 "groups_with_a_point": len(by), "accuracy": round(acc, 3)})
    for p in PATCHES:
        table = list(csv.DictReader(open(DEEP / f"embeddings{p}.csv", encoding="utf-8")))
        cols = [c for c in table[0] if c.startswith("emb_")]
        A = _np.array([[float(x[c]) for c in cols] for x in table])
        acc, by = accuracy(labels[f"cl{p}"])
        rows.append({"grouping": f"embeddings, patch {p}", "patch": p,
                     "distinct_vectors": len({tuple(_np.round(a, 5)) for a in A}),
                     "constant_columns": int((A.std(0) == 0).sum()),
                     "groups_with_a_point": len(by), "accuracy": round(acc, 3)})
    figs.write_csv(DATA / "embed_accuracy.csv", rows)
    for r in rows:
        print(r)

    def draw_map(L, path, width=560):
        img = _np.zeros(L.shape + (3,))
        for k in _np.unique(L[L > 0]):
            img[L == k] = mc.to_rgb(pal.get(int(k), "#7f7f7f"))
        od.save_png(_np.kron(img, _np.ones((2, 2, 1))), path, width=width)

    draw_map(labels["cl_mean"][r0:r1, c0:c1], OUT / "groups_mean.png")
    for p in PATCHES:
        draw_map(labels[f"cl{p}"][r0:r1, c0:c1], OUT / f"groups_emb{p}.png")


def main():
    args = set(sys.argv[1:])
    if "--sam" in args:
        sam_stage()
        classical_stage()
    if "--naipw" in args:
        naip_window_stage()
    if "--classical" in args:
        classical_stage()
    if "--encoder" in args:
        print(make_encoder())
    if "--embed" in args:
        embed_stage()
    if "--draw" in args:
        draw()


if __name__ == "__main__":
    main()
