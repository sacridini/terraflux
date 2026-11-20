"""Figures of "Case study: land use of a protected area from Sentinel-2" (tutorials/object-based-image-analysis/case-protected-area-sentinel2.md).

The area is the Estacao Ecologica do Jardim Botanico de Brasilia (4,502 ha), whose official boundary is read from the
Cadastro Nacional de Unidades de Conservacao (Ministerio do Meio Ambiente, CC BY, dados.mma.gov.br, the 2025_08
polygons). One Terraflux model reads Sentinel-2 L2A from Earth Search with data.stac (a dry-season and a wet-season
composite, masked for clouds, on a UTM 23S grid of 10 m given with `crs`), burns the boundary into a mask, segments the
inside, describes the objects, learns a random forest from training points, classifies, and assesses the map against a
stratified reference sample (objects.sample).

Training points and reference points are read by eye from image chips (obia_case.py): the script draws them, writes the
chips and stops until the labels file exists (`labels/protected-area-training.csv`, `labels/protected-area-reference.csv`).
The training points are drawn from the groups of a k-means of the pixels, so that rare classes are represented (a
group is not a class: the label is what the reader sees). At most 2 threads.
Run with: python website/figures/object-based-image-analysis/case-protected-area-sentinel2.py
"""

import csv
import json
import os
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_case as oc  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402

for _k in ("TERRAFLUX_THREADS", "RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS"):
    os.environ[_k] = "2"

figs = od.figs
HERE = pathlib.Path(__file__).resolve().parent
SLUG = "case-protected-area-sentinel2"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
LABELS = HERE / "labels"
CELL_HA = 0.01
CRS = "EPSG:31983"
EXTENT = [190300, 8234900, 198200, 8244800]  # xmin, ymin, xmax, ymax (m): 790 x 990 cells of 10 m
X0, Y1 = EXTENT[0], EXTENT[3]
URL = "https://earth-search.aws.element84.com/v1"
DRY = "2024-07-15/2024-09-15"
WET = "2024-02-01/2024-03-31"
BOUNDARY = HERE.parent.parent / ".cache" / "protected" / "esec_jb.gpkg"
SCALE = 1.5
COLS = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "mean_swir22", "mean_ndvi_wet", "ndvi", "ndwi",
        "ndmi", "sd_nir", "sd_ndvi_wet", "compactness"]


def node(i, op, inputs, params, note=None):
    t = f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\n'
    if inputs:
        t += f"inputs = {inputs}\n"
    t += f"params = {params}\n"
    if note:
        t += f'note = "{note}"\n'
    return t


def build(scale=SCALE, train=False, tail=""):
    """The page's model. Without `train` it stops at the attributes of the objects."""
    m = 'format = "terraflux/1"\n'
    m += node("dry", "data.stac", None,
              f'{{ url = "{URL}", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16", "swir22"], '
              f'datetime = "{DRY}", mask = "auto", crs = "{CRS}", extent = {EXTENT}, cellsize = 10 }}',
              "The dry season, 15 July to 15 September 2024: the median of the clear cells.")
    m += node("wet", "data.stac", None,
              f'{{ url = "{URL}", collection = "sentinel-2-l2a", assets = ["red", "nir"], datetime = "{WET}", mask = "auto", '
              f'crs = "{CRS}", extent = {EXTENT}, cellsize = 10 }}', "The wet season, February and March 2024.")
    m += node("area", "vector.read", None, '{ path = "esec_jb.gpkg" }', "The official boundary (CNUC).")
    m += node("inside", "vector.rasterize", '{ vector = "area.vector", like = "dry.raster" }', "{ value = 1, fill = 0 }",
              "1 inside the boundary, 0 outside.")
    m += node("refl", "raster.calc", '{ d = "dry.raster", w = "wet.raster", m = "inside.map" }',
              '{ expression = "image = d / 10000\\nimage.ndvi_wet = (w.nir - w.red) / (w.nir + w.red)\\nimage[m != 1] = None\\nout image" }',
              "Reflectance 0 to 1, the wet-season NDVI as a layer, nothing outside the boundary.")
    m += node("seg", "segment.multiresolution", '{ image = "refl.image" }', f"{{ scale = {scale} }}", "The objects.")
    m += node("feat", "objects.features", '{ objects = "seg.objects", image = "refl.image" }',
              '{ features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "compactness"], swir = "swir16" }',
              "Means, spread, indices and compactness.")
    if train:
        m += node("train_table", "table.read", None, '{ path = "training_points.csv" }', "The training points: x, y and class.")
        m += node("train", "vector.from_table", '{ table = "train_table.table" }', f'{{ crs = "{CRS}" }}')
        m += node("samples", "objects.samples", '{ objects = "feat.objects", samples = "train.vector" }', "{}",
                  "The objects under the points, with their attributes.")
        cols = ", ".join(f'"{c}"' for c in COLS)
        m += node("fit", "ml.fit", '{ table = "samples.table" }',
                  f'{{ column = "class", columns = [{cols}], n_estimators = 300 }}', "A random forest.")
        m += node("rf", "objects.classify", '{ objects = "feat.objects", model = "fit.model", samples = "samples.table" }', "{}",
                  "Class and probabilities of every object.")
    return m + tail


def run(model, work, *extra):
    r = figs.run(model, work, "--threads", "2", "--no-journal", *extra)
    return r


def read(path):
    with rasterio.open(path) as ds:
        return ds.read(1, masked=True).astype("float64").filled(np.nan)


def table(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def xy_to_rc(x, y):
    return int((Y1 - y) / 10), int((x - X0) / 10)


def rc_to_xy(r, c):
    return X0 + (c + 0.5) * 10, Y1 - (r + 0.5) * 10


def renderings(work):
    """True colour and near-infrared false colour of the dry composite (the first six layers of image.tif), 0..1."""
    with rasterio.open(work / "image.tif") as ds:
        a = ds.read(masked=True).astype("float64").filled(np.nan)
    tc = np.clip(np.nan_to_num(np.stack([a[2], a[1], a[0]], -1)) / 0.22, 0, 1) ** 0.8
    fc = np.clip(np.nan_to_num(np.stack([a[3], a[2], a[1]], -1)) / 0.45, 0, 1) ** 0.8
    wet = np.clip(np.nan_to_num((a[6] + 0.2) / 1.2), 0, 1)
    return a, tc, fc, wet


def write_inputs(work):
    """The boundary beside the model, and the stage-one run: image, object numbers and the mask."""
    shutil.copy(BOUNDARY, work / "esec_jb.gpkg")
    tail = node("seg_id", "objects.to_map", '{ objects = "seg.objects" }', '{ what = "id" }')
    m = figs.add_writes(build(tail=tail), {"refl.image": "image.tif", "seg_id.map": "seg_id.tif", "inside.map": "inside.tif",
                                           "dry.raster": "dry.tif", "wet.raster": "wet.tif"})
    r = run(m, work, "--profile")
    print(r.stdout[-700:])


def training_points(work):
    """150 pixels inside the boundary, 25 from each of six groups of a k-means of the pixels, shuffled."""
    from sklearn.cluster import KMeans

    a, tc, fc, wet = renderings(work)
    valid = np.all(np.isfinite(a), axis=0)
    rr, cc = np.nonzero(valid)
    rng = np.random.default_rng(5)
    sub = rng.choice(len(rr), 40000, replace=False)
    z = a[:, rr, cc].T
    mu, sd = z[sub].mean(0), z[sub].std(0)
    km = KMeans(6, n_init=4, random_state=0).fit((z[sub] - mu) / sd)
    lab = km.predict((z - mu) / sd)
    pts = []
    for g in range(6):
        idx = np.nonzero(lab == g)[0]
        pick = rng.choice(idx, 25, replace=False)
        pts += [(int(rr[i]), int(cc[i])) for i in pick]
    order = rng.permutation(len(pts))
    pts = [pts[i] for i in order]
    rows = [{"n": n, "x": round(rc_to_xy(*p)[0], 1), "y": round(rc_to_xy(*p)[1], 1)} for n, p in enumerate(pts, 1)]
    figs.write_csv(work / "training_order.csv", rows)
    return rows, pts, (tc, fc)


def stage_two(work, rows):
    """Training points with labels, the classification, and the reference sample."""
    lab = oc.read_labels(LABELS / "protected-area-training.csv")
    figs.write_csv(work / "training_points.csv", [{"x": r["x"], "y": r["y"], "class": lab[int(r["n"])]} for r in rows])
    tail = node("sample", "objects.sample", '{ objects = "rf.objects" }', "{ per_class = 50, seed = 1 }")
    tail += node("w_pts", "vector.write", '{ vector = "sample.points" }', '{ path = "sample_points.gpkg" }')
    tail += node("rf_id", "objects.to_map", '{ objects = "rf.objects" }', '{ what = "class" }')
    m = figs.add_writes(build(train=True, tail=tail), {"rf_id.map": "rf.tif", "rf.table": "rf_table.csv",
                                                       "fit.importance": "importance.csv", "samples.table": "samples.csv"})
    r = run(m, work, "--profile")
    print(r.stdout[-900:])
    return lab


def sample_order(work):
    import geopandas as gpd

    g = gpd.read_file(work / "sample_points.gpkg")
    rng = np.random.default_rng(3)
    order = rng.permutation(len(g))
    pts, rows = [], []
    for n, k in enumerate(order, 1):
        x, y = g.geometry.iloc[k].x, g.geometry.iloc[k].y
        pts.append(xy_to_rc(x, y))
        rows.append({"n": n, "id": int(g["id"].iloc[k]), "x": round(x, 1), "y": round(y, 1), "mapped": g["mapped"].iloc[k]})
    figs.write_csv(work / "sample_order.csv", rows)
    return rows, pts


CNUC = ("https://dados.mma.gov.br/dataset/44b6dc8a-dc82-4a84-8d95-1b0da7c85dac/resource/"
        "6ba9a557-87e8-4882-acb7-b3e0f0ea192d/download/shp_cnuc_2025_08.zip")


def make_boundary():
    """The boundary of the protected area from the MMA's polygons of the national registry (CC BY): the feature with
    uc_id 1654, in its own CRS (SIRGAS 2000, EPSG:4674)."""
    import urllib.request
    import zipfile

    import geopandas as gpd

    tmp = BOUNDARY.parent / "cnuc"
    tmp.mkdir(parents=True, exist_ok=True)
    z = tmp / "cnuc.zip"
    if not z.exists():
        urllib.request.urlretrieve(CNUC, z)
    with zipfile.ZipFile(z) as f:
        f.extractall(tmp)
    g = gpd.read_file(tmp / "cnuc_2025_08.shp", encoding="latin1", bbox=(-48.3, -16.2, -47.5, -15.4))
    r = g[g["uc_id"] == 1654].copy()
    assert len(r) == 1, "uc_id 1654 not found"
    r["nome_uc"] = "ESTACAO ECOLOGICA DO JARDIM BOTANICO DE BRASILIA"
    r[["uc_id", "nome_uc", "ha_total", "cria_ano", "geometry"]].to_file(BOUNDARY, driver="GPKG")


def main():
    if not BOUNDARY.exists():
        make_boundary()
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}", clean=False)
    write_inputs(work)
    rows, pts, (tc, fc) = training_points(work)
    if not (LABELS / "protected-area-training.csv").exists():
        oc.chip_sheets(tc, pts, work / "sheets", "train", half=16, per_sheet=20, cols=4, rgb2=fc)
        print("training sheets in", work / "sheets")
        return
    stage_two(work, rows)
    srows, spts = sample_order(work)
    if not (LABELS / "protected-area-reference.csv").exists():
        oc.chip_sheets(tc, spts, work / "sheets", "ref", half=16, per_sheet=20, cols=4, rgb2=fc)
        print("reference sheets in", work / "sheets")
        return
    figures(work, rows, pts, srows, spts)


CLASSES = ["bare soil", "built-up", "cerrado", "forest"]  # objects.samples numbers the classes alphabetically
COL = {"bare soil": "#e07b39", "built-up": "#c0504d", "cerrado": "#d9b86c", "forest": "#2f7d3b"}

ACC = """
[[nodes]]
id = "map"
op = "objects.to_map"
inputs = { objects = "rf.objects" }
params = { what = "class" }

[[nodes]]
id = "map_file"
op = "raster.write"
inputs = { value = "map.map" }
params = { path = "land_use.tif" }
note = "The land-use map, a GeoTIFF."

[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_points.csv" }
note = "The reference: x, y and the class read from the image."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "acc"
op = "objects.accuracy"
inputs = { objects = "rf.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "Error matrix, accuracies and adjusted areas."
"""


def paint(cls, window=None):
    pal = np.ones((len(CLASSES) + 1, 3))
    for k, n in enumerate(CLASSES, 1):
        h = COL[n]
        pal[k] = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    img = pal[np.nan_to_num(cls, nan=0).astype(int)]
    if window:
        r0, r1, c0, c1 = window
        img = img[r0:r1, c0:c1]
    return img


def save(img, name, width, colors=96):
    od.save_png(img, OUT / name, width=width, colors=colors)


def mpl_png(fig, name, width=1000, colors=96):
    p = OUT / name
    fig.savefig(p, dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(p))[..., :3], p, width=width, colors=colors)


def crop(a, box):
    r0, r1, c0, c1 = box
    return a[r0:r1, c0:c1]


def reference_points(work, srows):
    lab = oc.read_labels(LABELS / "protected-area-reference.csv")
    rows = [{"x": r["x"], "y": r["y"], "class": lab[int(r["n"])]} for r in srows]
    figs.write_csv(work / "reference_points.csv", rows)
    figs.write_csv(DATA / "reference_points.csv", rows)
    return lab


def run_final(work, scale=SCALE, tag=None):
    """The model with the map and the accuracy; the figures' extras when `tag` is None (the page's run)."""
    w = work if tag is None else work / tag
    w.mkdir(exist_ok=True)
    for f in ("esec_jb.gpkg", "training_points.csv", "reference_points.csv"):
        if w != work:
            shutil.copy(work / f, w / f)
    writes = {"rf.table": "rf_table.csv", "acc.table": "acc_table.csv", "acc.overall": "acc_overall.csv",
              "acc.matrix": "acc_matrix.csv", "acc.errors": "acc_errors.csv", "map.map": "x"}
    writes.pop("map.map")
    extra = ""
    if tag is None:
        extra += node("seg_id", "objects.to_map", '{ objects = "rf.objects" }', '{ what = "id" }')
        writes |= {"seg_id.map": "seg_id.tif", "map.map": "map.tif", "fit.importance": "importance.csv", "feat.table": "feat.csv",
                   "samples.table": "samples.csv", "samples.counts": "counts.csv"}
    else:
        writes |= {"map.map": "map.tif"}
    m = figs.add_writes(build(scale=scale, train=True, tail=ACC + extra), writes)
    run(m, w)
    if tag is None:
        (w / "page_model.toml").write_text(build(train=True, tail=ACC), encoding="utf-8")
    return w


def figures(work, rows, pts, srows, spts):
    import geopandas as gpd

    lab = reference_points(work, srows)
    w = run_final(work)
    out = w / "output"
    a, tc, fc, wet = renderings(work)
    stats = {}
    # ------------------------------------------------------------------ the area
    g = gpd.read_file(work / "esec_jb.gpkg").to_crs(CRS)
    stats["boundary_ha"] = float(g.geometry.area.sum() / 1e4)
    with rasterio.open(work / "dry.tif") as ds:
        dry = ds.read().astype("float64")
    tc_full = np.clip(np.stack([dry[2], dry[1], dry[0]], -1) / 10000 / 0.22, 0, 1) ** 0.8
    fc_full = np.clip(np.stack([dry[3], dry[2], dry[1]], -1) / 10000 / 0.45, 0, 1) ** 0.8
    fig = plt.figure(figsize=(5.4, 6.6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(tc_full, extent=(EXTENT[0], EXTENT[2], EXTENT[1], EXTENT[3]))
    g.boundary.plot(ax=ax, color="#ffd91a", linewidth=1.4)
    ax.plot([EXTENT[0] + 300, EXTENT[0] + 1300], [EXTENT[1] + 300] * 2, color="white", lw=3)
    ax.text(EXTENT[0] + 800, EXTENT[1] + 450, "1 km", color="white", ha="center", fontsize=9, fontweight="bold")
    ax.set_xlim(EXTENT[0], EXTENT[2])
    ax.set_ylim(EXTENT[1], EXTENT[3])
    ax.set_axis_off()
    mpl_png(fig, "area.png", 700, 200)
    # ------------------------------------------------------------------ the seasons
    valid = np.all(np.isfinite(a), axis=0)
    fig, axes = plt.subplots(1, 3, figsize=(10.5, 4.6), dpi=150)
    for ax, (im, t, kw) in zip(axes, [(tc, "Dry season, true colour", {}), (fc, "Dry season, near infrared false colour", {}),
                                      (np.where(valid, a[6], np.nan), "Wet-season NDVI", dict(cmap="viridis", vmin=0.2, vmax=0.95))]):
        if kw:
            ax.imshow(im, **kw)
        else:
            ax.imshow(np.where(valid[..., None], im, 1.0))
        ax.set_title(t, fontsize=8.5, loc="left")
        ax.set_axis_off()
    fig.tight_layout(pad=0.4)
    mpl_png(fig, "seasons.png", 1100, 160)
    # ------------------------------------------------------------------ the classified map
    cls = read(work / "map.tif")
    cells = {n: int((cls == k).sum()) for k, n in enumerate(CLASSES, 1)}
    tot = sum(cells.values())
    stats["map_cells"] = cells
    stats["valid_cells"] = int(valid.sum())
    ids = np.nan_to_num(read(work / "seg_id.tif"), nan=0).astype(int)
    stats["objects"] = int(ids.max())
    # a detail with all the classes: the 220-cell window with the highest entropy of the classes
    best, box = -1, None
    for r0 in range(0, cls.shape[0] - 220, 40):
        for c0 in range(0, cls.shape[1] - 220, 40):
            sub = cls[r0:r0 + 220, c0:c0 + 220]
            p = np.array([(sub == k).sum() for k in range(1, 5)], float)
            if p.sum() < 0.8 * 220 * 220:
                continue
            p /= p.sum()
            e = -(p[p > 0] * np.log(p[p > 0])).sum()
            if e > best:
                best, box = e, (r0, r0 + 220, c0, c0 + 220)
    stats["detail"] = box
    img = paint(cls)
    img[~valid] = 1.0
    fig = plt.figure(figsize=(5.2, 6.6), dpi=150)
    ax = fig.add_axes([0, 0.06, 1, 0.94])
    ax.imshow(img, interpolation="nearest")
    ax.add_patch(plt.Rectangle((box[2], box[0]), 220, 220, fill=False, ec="#222222", lw=1.2))
    ax.set_axis_off()
    from matplotlib.patches import Patch

    ax.legend(handles=[Patch(fc=COL[n], label=n) for n in CLASSES], loc="upper center", bbox_to_anchor=(0.5, -0.0), ncol=4,
              frameon=False, fontsize=8)
    mpl_png(fig, "map.png", 700, 32)
    plain_fc = np.where(valid[..., None], fc, 1.0)
    save(plain_fc, "image_plain.png", 640, 160)
    save(img, "map_plain.png", 640, 24)
    rows_a = [{"class": n, "hectares": round(c * CELL_HA, 1), "share": round(100 * c / tot, 1)} for n, c in cells.items()]
    figs.write_csv(DATA / "map_areas.csv", rows_a)
    # ------------------------------------------------------------------ objects, attributes and the detail
    feat = {k: np.array([float(r[k]) for r in table(out / "feat.csv")]) for k in table(out / "feat.csv")[0]}

    def cmap_of(col):
        v = np.full(ids.shape, np.nan)
        m = ids > 0
        v[m] = feat[col][ids[m] - 1]
        return v

    from_ids = od.outlines(crop(fc, box), crop(ids, box), color=(1.0, 0.9, 0.2), scale=3)
    save(from_ids, "objects.png", 720, 160)
    fig, axes = plt.subplots(2, 2, figsize=(7.4, 7.0), dpi=150)
    for ax, (col, lo, hi, t) in zip(axes.flat, [("ndvi", 0.1, 0.8, "NDVI, dry season"), ("mean_ndvi_wet", 0.4, 0.95, "NDVI, wet season"),
                                                ("ndmi", -0.2, 0.3, "NDMI (nir, swir)"), ("mean_swir22", 0.05, 0.25, "Short-wave infrared 2")]):
        ax.imshow(crop(cmap_of(col), box), vmin=lo, vmax=hi, cmap="viridis", interpolation="nearest")
        ax.set_title(t, fontsize=8.5, loc="left")
        ax.set_axis_off()
    fig.tight_layout(pad=0.5)
    mpl_png(fig, "attributes.png", 800, 128)
    # ------------------------------------------------------------------ the training points
    tl = oc.read_labels(LABELS / "protected-area-training.csv")
    fig = plt.figure(figsize=(5.2, 6.6), dpi=150)
    ax = fig.add_axes([0, 0.06, 1, 0.94])
    ax.imshow(np.where(valid[..., None], fc * 0.8 + 0.2, 1.0))
    for (r, c), row in zip(pts, rows):
        ax.scatter([c], [r], s=26, c=COL[tl[int(row["n"])]], edgecolors="white", linewidths=0.6)
    ax.set_axis_off()
    ax.legend(handles=[Patch(fc=COL[n], label=f"{n} ({sum(1 for v in tl.values() if v == n)})") for n in CLASSES],
              loc="upper center", bbox_to_anchor=(0.5, 0.0), ncol=4, frameon=False, fontsize=8)
    mpl_png(fig, "training.png", 700, 200)
    stats["training"] = {n: sum(1 for v in tl.values() if v == n) for n in CLASSES}
    cnt = table(out / "counts.csv")
    stats["samples_counts"] = cnt
    # ------------------------------------------------------------------ importance
    imp = table(out / "importance.csv")[0]  # one row: a column per attribute
    items = sorted(((k, float(v)) for k, v in imp.items()), key=lambda kv: -kv[1])
    figs.write_csv(DATA / "importance.csv", [{"attribute": k, "importance": round(v, 4)} for k, v in items])
    stats["importance"] = items
    # ------------------------------------------------------------------ probabilities: the margin
    rt = table(out / "rf_table.csv")
    stats["rf_table_keys"] = list(rt[0].keys())
    if "margin" in rt[0]:
        mg = np.array([float(r["margin"]) for r in rt])
        mm = np.full(ids.shape, np.nan)
        m = ids > 0
        mm[m] = mg[ids[m] - 1]
        fig = plt.figure(figsize=(5.2, 6.4), dpi=150)
        ax = fig.add_axes([0, 0, 1, 1])
        ax.imshow(mm, vmin=0, vmax=1, cmap="viridis", interpolation="nearest")
        ax.set_axis_off()
        mpl_png(fig, "margin.png", 640, 64)
        cellsa = np.bincount(ids.ravel(), minlength=len(mg) + 1)[1:]
        stats["margin_lt_03_cells"] = float(cellsa[mg < 0.3].sum() / cellsa.sum())
        stats["margin_lt_03_objects"] = int((mg < 0.3).sum())
    # ------------------------------------------------------------------ accuracy
    ov = table(out / "acc_overall.csv")[0]
    stats["overall"] = {k: float(v) for k, v in ov.items()}
    mat = {r["map"]: r for r in table(out / "acc_matrix.csv")}
    rows_m = []
    for n_ in CLASSES:
        rr = {"map": n_}
        for c_ in CLASSES:
            rr[c_] = int(float(mat[n_][c_]))
        rr["total"] = sum(rr[c_] for c_ in CLASSES)
        rows_m.append(rr)
    figs.write_csv(DATA / "matrix.csv", rows_m)
    at = {r["class"]: r for r in table(out / "acc_table.csv")}
    rows_acc = []
    for n_ in CLASSES:
        r = at[n_]
        rows_acc.append({"class": n_, "mapped_objects": int(float(r["objects"])), "reference_objects": int(float(r["reference_objects"])),
                         "users": round(float(r["user_adjusted"]), 3), "users_ci": round(float(r["user_ci"]), 3),
                         "producers": round(float(r["producer_adjusted"]), 3), "producers_ci": round(float(r["producer_ci"]), 3),
                         "map_ha": round(float(r["map_cells"]) * CELL_HA, 1), "area_ha": round(float(r["area_ha"]), 1),
                         "area_ha_ci": round(float(r["area_ha_ci"]), 1)})
    figs.write_csv(DATA / "accuracy.csv", rows_acc)
    stats["acc_rows"] = rows_acc
    from collections import Counter

    stats["reference_by_label"] = dict(Counter(lab.values()))
    # error chips
    wrong = [(int(r["n"]), pt_, r["mapped"], lab[int(r["n"])]) for r, pt_ in zip(srows, spts) if lab[int(r["n"])] != r["mapped"]]
    stats["wrong"] = len(wrong)
    pick = wrong[:: max(1, len(wrong) // 12)][:12]
    fig, axes = plt.subplots(3, 4, figsize=(10.5, 6.8), dpi=150)
    for ax in axes.flat:
        ax.set_axis_off()
    for ax, (n_, (r, c), mp, lb) in zip(axes.flat, pick):
        ra, rb, ca, cb = max(0, r - 16), min(tc.shape[0], r + 16), max(0, c - 16), min(tc.shape[1], c + 16)
        ax.imshow(tc[ra:rb, ca:cb], extent=(ca, cb, rb, ra), interpolation="bicubic")
        ax.plot([c], [r], marker="+", color="#ffff00", ms=9, mew=1.2)
        ax.set_title(f"map: {mp}, read: {lb}", fontsize=8, color="#c0392b", pad=2)
    fig.tight_layout(pad=0.6)
    mpl_png(fig, "errors.png", 1000, 200)
    # ------------------------------------------------------------------ the scale
    sweep = []
    for s in (1, 1.5, 2, 3):
        tag = f"scale_{str(s).replace('.', '_')}"
        if s == SCALE:
            ovs, idm, cl = ov, ids, cls
        else:
            ww = run_final(work, s, tag=tag)
            ovs = table(ww / "output" / "acc_overall.csv")[0]
            cl = read(ww / "map.tif")
            idm = None
        nobj = int(np.nanmax(ids)) if s == SCALE else None
        sweep.append({"scale": s, "overall_adjusted": round(100 * float(ovs["overall_adjusted"]), 1), "ci": round(100 * float(ovs["overall_ci"]), 1),
                      "overall_objects": round(100 * float(ovs["overall_objects"]), 1), "kappa": round(float(ovs["kappa"]), 2)})
        save(np.kron(crop(paint(cl), box), np.ones((3, 3, 1))), f"{tag}.png", 640, 24)
    figs.write_csv(DATA / "scales.csv", sweep)
    stats["sweep"] = sweep
    # ------------------------------------------------------------------ downloads and the methods text
    shutil.copy(work / "esec_jb.gpkg", DATA / "esec_jardim_botanico.gpkg")
    shutil.copy(work / "training_points.csv", DATA / "training_points.csv")
    shutil.copy(work / "map.tif", DATA / "land_use.tif")
    shutil.copy(w / "page_model.toml", DATA / "model.toml")
    for nm_, src in (("esec_dry.tif", "dry.tif"), ("esec_wet.tif", "wet.tif")):
        with rasterio.open(work / src) as ds:
            data = np.rint(ds.read()).astype("int16")
            prof = ds.profile | {"dtype": "int16", "compress": "deflate", "predictor": 2, "tiled": False, "nodata": None}
            prof.pop("blockxsize", None)
            prof.pop("blockysize", None)
            desc = ds.descriptions
        with rasterio.open(DATA / nm_, "w", **prof) as o:
            o.write(data)
            o.descriptions = desc
    (DATA / "ATTRIBUTION.txt").write_text(
        "esec_jardim_botanico.gpkg: boundary of the Estacao Ecologica do Jardim Botanico de Brasilia (uc_id 1654), from the "
        "Cadastro Nacional de Unidades de Conservacao (CNUC), polygons of 2025_08, Ministerio do Meio Ambiente e Mudanca do Clima, "
        "https://dados.mma.gov.br (licence Creative Commons Attribution, CC BY). One feature, in SIRGAS 2000 (EPSG:4674).\n\n"
        "esec_dry.tif, esec_wet.tif: Sentinel-2 L2A (Earth Search), the median of the clear cells of 15 July to 15 September 2024 "
        "(blue, green, red, nir, swir16, swir22) and of February and March 2024 (red, nir), reflectance x 10000, Int16, EPSG:31983, "
        "10 m. Contains modified Copernicus Sentinel data 2024.\n\n"
        "training_points.csv, reference_points.csv: points labelled by eye from the image (see the tutorial); not field data.\n",
        encoding="utf-8")
    meth = figs.cli("methods", str(w / "page_model.toml"), cwd=w)
    (DATA / "methods.txt").write_text(meth, encoding="utf-8")
    stats["methods_chars"] = len(meth)
    (work / "summary.json").write_text(json.dumps(stats, indent=1, default=str), encoding="utf-8")
    print(json.dumps(stats, indent=1, default=str)[:6500])


if __name__ == "__main__":
    main()
