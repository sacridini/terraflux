"""Figures of "Editing by hand" (tutorials/object-based-image-analysis/editing-by-hand.md).

A figure run cannot click, so the edits the inspector would write (a merge, a cut, a delete) are written here as the
settings of an objects.edit node, in the format the application writes (places on the map, not object numbers), and
the model runs on the Sentinel-2 clip of the series (obia_data.py): multiresolution at scale 4 -> objects.edit ->
objects.features -> a random forest from the training points of obia_samples.py (read, not changed) -> the classes,
assessed against the reference points. Then the experiments of the page, each a run of Terraflux:
  * the objects and the classes before and after each of the three edits (a figure per edit),
  * the accuracy with and without the edits,
  * the same edits on other segmentations (scales 3, 4, 6, 8, and SLIC): the report table of each, and what moved,
  * the cost of the other way to fix the same objects: a smaller scale.
Run with: python website/figures/object-based-image-analysis/editing-by-hand.py   (at most 2 threads: it sets them)
"""

import csv
import os
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_data as od  # noqa: E402
import obia_samples as sm  # noqa: E402
import rasterio  # noqa: E402

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
           "TERRAFLUX_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "editing-by-hand"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
WORK = None

SCALE = 4
R0, R1, C0, C1 = 90, 360, 200, 470  # the part of the clip the other pages show
FEATURES = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"]
COLS = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast",
        "glcm_homogeneity", "sd_nir", "compactness"]
CLASSES = sm.CLASSES  # alphabetical: bare soil, cerrado, forest, urban, water
COLORS = sm.COLORS

# The three edits, as places of the clip (row, column of a cell; the file has their map coordinates).
# merge: eight objects of the lake, the first one's the survivor; cut: a line across the object that holds the bare
# ground and the first roofs of the town; delete: one white roof the forest called bare soil, taken by its neighbour.
MERGE = [(142, 243), (124, 256), (132, 273), (165, 257), (165, 282), (169, 231), (116, 248), (114, 229)]
CUT = [(196, 254), (196, 294)]
DELETE = (299, 244)


def pt(rc):
    x, y = sm.xy(*rc)
    return [round(x, 1), round(y, 1)]


def edits_value(edits=None):
    """The `edits` setting as TOML (the format objects.edit reads: places in the objects' coordinate system)."""
    e = {"merge": MERGE, "cut": CUT, "delete": DELETE} if edits is None else edits
    out = []
    if e.get("merge"):
        out.append("{ merge = [" + ", ".join(str(pt(p)) for p in e["merge"]) + "] }")
    if e.get("cut"):
        out.append("{ cut = [" + ", ".join(str(pt(p)) for p in e["cut"]) + "] }")
    if e.get("delete"):
        out.append(f'{{ delete = {pt(e["delete"])}, into = "neighbour" }}')
    return "[" + ", ".join(out) + "]"


def tl(v):
    return "[" + ", ".join(f'"{x}"' for x in v) + "]"


def node(i, op, inputs, params="{ }"):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def read_csv(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def num(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return float("nan")


def read_map(path):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, 0).astype("int64")


def head(data_stac=False, seg="segment.multiresolution", seg_params=f"{{ scale = {SCALE} }}"):
    """The clip (the page's data.stac model, or its cached file for the many small runs), reflectance and the segmenter."""
    if data_stac:
        m = od.sentinel2_model()
    else:
        tif = str(od.sentinel2()).replace("\\", "/")
        m = 'format = "terraflux/1"\n\n[[nodes]]\nid = "s2"\nop = "raster.read"\nparams = { path = "%s" }\n' % tif
        m += node("refl", "raster.calc", '{ s = "s2.raster" }', '{ expression = "out image = s / 10000" }')
    return m + node("segments", seg, '{ image = "refl.image" }', seg_params)


def edit_node(edits=None, i="edit"):
    return node(i, "objects.edit", '{ objects = "segments.objects", image = "refl.image" }',
                f"{{ edits = {edits_value(edits)} }}")


def classify(src, train="train"):
    """Features of the objects of `src`, the forest from the training points, the classes."""
    m = node("feat", "objects.features", f'{{ objects = "{src}.objects", image = "refl.image" }}',
             f'{{ features = {tl(FEATURES)}, texture_bands = ["brightness"], swir = "swir16" }}')
    m += node("samples", "objects.samples", f'{{ objects = "feat.objects", samples = "{train}.vector" }}')
    m += node("fit", "ml.fit", '{ table = "samples.table" }',
              f'{{ column = "class", columns = {tl(COLS)}, n_estimators = 300 }}')
    m += node("rf", "objects.classify", '{ objects = "feat.objects", model = "fit.model", samples = "samples.table" }')
    return m


def frozen(src="edit"):
    """The forest of the objects as they were (no edit), applied to the attributes of the edited objects."""
    m = node("feat_a", "objects.features", f'{{ objects = "{src}.objects", image = "refl.image" }}',
             f'{{ features = {tl(FEATURES)}, texture_bands = ["brightness"], swir = "swir16" }}')
    m += node("rf_a", "objects.classify", '{ objects = "feat_a.objects", model = "fit.model", samples = "samples.table" }')
    m += node("acc_a", "objects.accuracy", '{ objects = "rf_a.objects", samples = "ref.vector" }', '{ design = "objects" }')
    m += node("idmap_a", "objects.to_map", '{ objects = "rf_a.objects" }', '{ what = "id" }')
    return m + node("clsmap_a", "objects.to_map", '{ objects = "rf_a.objects" }', '{ what = "class" }')


def maps(src="feat"):
    m = node("idmap", "objects.to_map", f'{{ objects = "{src}.objects" }}', '{ what = "id" }')
    m += node("clsmap", "objects.to_map", '{ objects = "rf.objects" }', '{ what = "class" }')
    return m


def acc(ref="ref"):
    return node("acc", "objects.accuracy", f'{{ objects = "rf.objects", samples = "{ref}.vector" }}', '{ design = "objects" }')


def run(model, name, writes, log=False):
    work = WORK / name
    work.mkdir(parents=True, exist_ok=True)
    r = figs.run(figs.add_writes(model, writes), work, "--threads", "2", "--no-journal")
    if log:
        print(r.stdout[-700:], r.stderr[-1500:])
    return work


def outputs(work):
    ids = read_map(work / "ids.tif")
    cls = read_map(work / "cls.tif") if (work / "cls.tif").exists() else None
    return ids, cls


def window(a):
    return a[R0:R1, C0:C1]


def paint(cls):
    img = np.ones(cls.shape + (3,)) * 0.9
    for k, n in enumerate(CLASSES, 1):
        img[cls == k] = matplotlib.colors.to_rgb(COLORS[n])
    return img


def blend(rgb, cls, a=0.55):
    return rgb * (1 - a) + paint(cls) * a * (cls > 0)[..., None] + rgb * a * (cls == 0)[..., None]


def overall(path):
    return {k: num(v) for k, v in read_csv(path)[0].items()}


def main():
    global WORK
    od.sentinel2()
    WORK = figs.workdir(f"{od.CATEGORY}/{SLUG}")

    # the objects at scale 4 (no edit): to place the training and the reference points, one per object
    w = run(head() + node("idmap", "objects.to_map", '{ objects = "segments.objects" }', '{ what = "id" }'), "ids",
            {"idmap.map": "ids.tif"})
    ids0 = read_map(w / "ids.tif")
    taken = set()
    train = sm.draw(sm.TRAIN, ids0, seed=7, taken=taken)
    ref = sm.draw(sm.REFERENCE, ids0, seed=11, taken=taken)
    print(len(train), "training objects,", len(ref), "reference objects, scale", SCALE, "objects", ids0.max())
    for name, pts in (("training_samples.csv", train), ("reference_samples.csv", ref)):
        figs.write_csv(DATA / name, [{"x": sm.xy(r, c)[0], "y": sm.xy(r, c)[1], "class": cls} for cls, r, c in pts])
    pts_nodes = sm.table_nodes("train", train) + sm.table_nodes("ref", ref)

    # ------------------------------------------------------------------ the page's model, from the CSVs and data.stac
    page = head(True) + edit_node()
    for pre, f in (("train", "training_samples.csv"), ("ref", "reference_samples.csv")):
        page += node(f"{pre}_table", "table.read", "{ }", f'{{ path = "{f}" }}')
        page += node(pre, "vector.from_table", f'{{ table = "{pre}_table.table" }}', f'{{ crs = "{sm.CRS}" }}')
    page += classify("edit") + acc()
    wd = WORK / "page"
    wd.mkdir(parents=True, exist_ok=True)
    for f in ("training_samples.csv", "reference_samples.csv"):
        (wd / f).write_text((DATA / f).read_text(encoding="utf-8"), encoding="utf-8")
    run(page, "page", {"acc.overall": "overall.csv", "edit.report": "report.csv"}, log=True)
    print("the page's model:", overall(WORK / "page" / "output" / "overall.csv"))
    print((WORK / "page" / "output" / "report.csv").read_text())

    # ------------------------------------------------------------------ before and after, from the cached clip
    before = head() + pts_nodes + classify("segments") + acc() + maps("segments")
    after = head() + edit_node() + pts_nodes + classify("edit") + acc() + maps("edit")
    wr = {"idmap.map": "ids.tif", "clsmap.map": "cls.tif", "acc.overall": "overall.csv", "acc.matrix": "matrix.csv",
          "acc.table": "classes.csv", "samples.counts": "counts.csv"}
    wb = run(before, "before", wr)
    wa = run(after, "after", wr | {"edit.report": "report.csv", "edit.table": "table.csv"}, log=True)
    ids_b, cls_b = outputs(wb)
    ids_a, cls_a = outputs(wa)
    ob, oa = overall(wb / "output" / "overall.csv"), overall(wa / "output" / "overall.csv")
    print("before", ob)
    print("after", oa)
    print("objects", len(np.unique(ids_b[ids_b > 0])), "->", len(np.unique(ids_a[ids_a > 0])))
    print((wa / "output" / "report.csv").read_text())
    rgb = od.rgb("sentinel2")

    # the same forest (trained on the objects as they were) on the edited objects: what the edits change by themselves
    fz = head() + edit_node() + pts_nodes + classify("segments") + frozen("edit")
    wf = run(fz, "frozen", {"clsmap_a.map": "cls.tif", "idmap_a.map": "ids.tif", "acc_a.overall": "overall.csv",
                            "samples.counts": "counts.csv"})
    cls_f = read_map(wf / "cls.tif")
    of = overall(wf / "output" / "overall.csv")
    print("frozen forest on the edited objects", of)
    print("cells that changed class with the same forest:", int((cls_f != cls_b).sum()))
    print("  retrained:", int((cls_a != cls_b).sum()))
    cb_, ca_ = read_csv(wb / "output" / "counts.csv"), read_csv(wa / "output" / "counts.csv")
    figs.write_csv(DATA / "training_counts.csv", [
        {"class": b["class"], "objects_before": int(b["objects"]), "objects_after": int(a["objects"])}
        for b, a in zip(cb_, ca_)])
    print(cb_, ca_)
    figs.write_csv(DATA / "changes.csv", [
        {"what": "the same forest on the edited objects", "cells_changed_class": int((cls_f != cls_b).sum()),
         "overall_objects": of["overall_objects"]},
        {"what": "the forest trained again on the edited objects", "cells_changed_class": int((cls_a != cls_b).sum()),
         "overall_objects": oa["overall_objects"]}])

    # which cells changed class, and which objects the edits touched
    changed = (cls_b != cls_a)
    print("cells that changed class:", int(changed.sum()))
    for a in range(len(CLASSES)):
        for b in range(len(CLASSES)):
            n = int(((cls_b == a + 1) & (cls_a == b + 1)).sum())
            if a != b and n:
                print(f"  {CLASSES[a]} -> {CLASSES[b]}: {n} cells")

    # ------------------------------------------------------------------ figures
    zoom = {
        "merge": (95, 180, 218, 305),
        "cut": (168, 216, 240, 310),
        "delete": (285, 315, 228, 262),
    }
    scale = {"merge": 7, "cut": 12, "delete": 18}
    marks = {"merge": [sm.xy(*p) for p in MERGE], "cut": CUT, "delete": DELETE}

    def draw_zoom(kind, ids, cls, name, mark=True):
        r0, r1, c0, c1 = zoom[kind]
        s = scale[kind]
        base = rgb[r0:r1, c0:c1]
        img = od.outlines(base, ids[r0:r1, c0:c1], scale=s, thick=2 if s > 8 else 1, color=(1.0, 0.85, 0.1))
        if cls is not None:
            cimg = od.outlines(blend(base, cls[r0:r1, c0:c1]), ids[r0:r1, c0:c1], scale=s, thick=2 if s > 8 else 1,
                               color=(1.0, 1.0, 1.0))
        fig = plt.figure(figsize=(img.shape[1] / 150, img.shape[0] / 150), dpi=150)
        ax = fig.add_axes([0, 0, 1, 1])
        ax.imshow(cimg if cls is not None else img, interpolation="nearest")
        if mark:
            if kind == "merge":
                for (r, c) in MERGE:
                    ax.scatter((c - c0 + 0.5) * s, (r - r0 + 0.5) * s, s=34, c="#ffffff", edgecolors="#c2338a", linewidths=1.6, zorder=4)
            elif kind == "cut":
                ax.plot([(c - c0 + 0.5) * s for _, c in CUT], [(r - r0 + 0.5) * s for r, _ in CUT], color="#ff2a6d", lw=2.2, zorder=4)
                ax.scatter([(c - c0 + 0.5) * s for _, c in CUT], [(r - r0 + 0.5) * s for r, _ in CUT], s=30, c="#ff2a6d", zorder=5)
            else:
                ax.scatter((DELETE[1] - c0 + 0.5) * s, (DELETE[0] - r0 + 0.5) * s, marker="x", s=70, c="#ff2a6d", linewidths=2.4, zorder=5)
        ax.set_axis_off()
        fig.savefig(OUT / name, dpi=150)
        plt.close(fig)
        od.save_png(np.asarray(plt.imread(OUT / name))[..., :3], OUT / name, width=860 if kind != "delete" else 640, colors=200)

    for kind in zoom:
        draw_zoom(kind, ids_b, cls_b, f"{kind}_before.png")
        draw_zoom(kind, ids_a, cls_f, f"{kind}_after.png", mark=False)  # the same forest: only the edits change

    # where the edits are, on the clip
    fig = plt.figure(figsize=(5.4, 5.4), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb[R0:R1, C0:C1])
    for kind, col in (("merge", "#c2338a"), ("cut", "#ff2a6d"), ("delete", "#ffd91a")):
        r0, r1, c0, c1 = zoom[kind]
        ax.add_patch(plt.Rectangle((c0 - C0, r0 - R0), c1 - c0, r1 - r0, fill=False, ec=col, lw=1.8))
        ax.text(c0 - C0 + 3, r0 - R0 + 10, kind, color=col, fontsize=9, fontweight="bold",
                bbox=dict(fc="#000000aa", ec="none", pad=1.5))
    ax.set_axis_off()
    fig.savefig(OUT / "where.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "where.png"))[..., :3], OUT / "where.png", width=700, colors=200)

    # the model's tables
    import shutil

    shutil.copy(wa / "output" / "report.csv", DATA / "report.csv")
    shutil.copy(wb / "output" / "matrix.csv", DATA / "matrix_before.csv")
    shutil.copy(wa / "output" / "matrix.csv", DATA / "matrix_after.csv")
    figs.write_csv(DATA / "accuracy.csv", [
        {"model": "without the edits", "objects": int(len(np.unique(ids_b[ids_b > 0]))), "sampled": int(ob["objects"]),
         "overall_objects": ob["overall_objects"], "overall_adjusted": ob["overall_adjusted"], "kappa": ob["kappa"]},
        {"model": "with the edits", "objects": int(len(np.unique(ids_a[ids_a > 0]))), "sampled": int(oa["objects"]),
         "overall_objects": oa["overall_objects"], "overall_adjusted": oa["overall_adjusted"], "kappa": oa["kappa"]},
    ])

    # ------------------------------------------------------------------ the same edits on other segmentations
    rows = []
    frames = []
    for lab, seg, sp in [("scale 3", "segment.multiresolution", "{ scale = 3 }"),
                         ("scale 4", "segment.multiresolution", "{ scale = 4 }"),
                         ("scale 6", "segment.multiresolution", "{ scale = 6 }"),
                         ("scale 8", "segment.multiresolution", "{ scale = 8 }"),
                         ("SLIC", "segment.slic", "{ size = 25 }")]:
        key = lab.replace(" ", "")
        m0 = head(seg=seg, seg_params=sp)
        m = (m0 + node("idmap0", "objects.to_map", '{ objects = "segments.objects" }', '{ what = "id" }')
             + edit_node() + node("idmap", "objects.to_map", '{ objects = "edit.objects" }', '{ what = "id" }'))
        w = run(m, f"seg_{key}", {"idmap.map": "ids.tif", "idmap0.map": "ids0.tif", "edit.report": "report.csv"})
        i0 = read_map(w / "ids0.tif")
        i1 = read_map(w / "ids.tif")
        rep = read_csv(w / "output" / "report.csv")
        print(lab, len(np.unique(i0[i0 > 0])), "->", len(np.unique(i1[i1 > 0])))
        for r in rep:
            print("   ", r)
        rows.append({"segmentation": lab, "objects": int(len(np.unique(i0[i0 > 0]))),
                     "objects_after": int(len(np.unique(i1[i1 > 0]))),
                     **{f"{r['kind']}_status": r["status"] for r in rep},
                     **{f"{r['kind']}_cells": r["cells"] for r in rep}})
        img = od.outlines(window(rgb), window(i1), scale=3, thick=1, color=(1.0, 0.85, 0.1))
        fig = plt.figure(figsize=(img.shape[1] / 150, img.shape[0] / 150), dpi=150)
        ax = fig.add_axes([0, 0, 1, 1])
        ax.imshow(img, interpolation="nearest")
        f = lambda v, o: (v - o + 0.5) * 3  # noqa: E731
        ax.scatter([f(c, C0) for _, c in MERGE], [f(r, R0) for r, _ in MERGE], s=26, c="#ffffff", edgecolors="#c2338a", linewidths=1.5, zorder=4)
        ax.plot([f(c, C0) for _, c in CUT], [f(r, R0) for r, _ in CUT], color="#ff2a6d", lw=2.4, zorder=4)
        ax.scatter(f(DELETE[1], C0), f(DELETE[0], R0), marker="x", s=60, c="#ff2a6d", linewidths=2.2, zorder=5)
        ax.set_axis_off()
        fig.savefig(OUT / f"on_{key}.png", dpi=150)
        plt.close(fig)
        od.save_png(np.asarray(plt.imread(OUT / f"on_{key}.png"))[..., :3], OUT / f"on_{key}.png", width=640, colors=160)
        frames.append((key, lab))
    figs.write_csv(DATA / "other_segmentations.csv", rows)

    # ------------------------------------------------------------------ the other way: a smaller scale
    alt = []
    for lab, sp, with_edit in [("scale 4", "{ scale = 4 }", False), ("scale 4 and the edits", "{ scale = 4 }", True),
                               ("scale 3", "{ scale = 3 }", False), ("scale 2", "{ scale = 2 }", False)]:
        m = head(seg_params=sp) + (edit_node() if with_edit else "") + pts_nodes
        m += classify("edit" if with_edit else "segments") + acc() + node("idmap", "objects.to_map", f'{{ objects = "{"edit" if with_edit else "segments"}.objects" }}', '{ what = "id" }')
        w = run(m, "alt_" + lab.replace(" ", "_"), {"acc.overall": "overall.csv", "idmap.map": "ids.tif"})
        o = overall(w / "output" / "overall.csv")
        i = read_map(w / "ids.tif")
        alt.append({"choice": lab, "objects": int(len(np.unique(i[i > 0]))), "sampled": int(o["objects"]), "overall_objects": o["overall_objects"],
                    "overall_adjusted": o["overall_adjusted"], "kappa": o["kappa"]})
        print(alt[-1])
    figs.write_csv(DATA / "alternatives.csv", alt)


if __name__ == "__main__":
    main()
