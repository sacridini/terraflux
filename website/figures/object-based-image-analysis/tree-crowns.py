"""Figures of "Separate tree crowns" (tutorials/object-based-image-analysis/tree-crowns.md).

NAIP has no height model, so the canopy is found from the NDVI and the NDVI stands in for the surface: one model of
Terraflux on the NAIP clip of Plano, Texas (obia_data.py, read only; the page's model reads the clip with raster.read):
NDVI -> a Gaussian smooth (raster.focal) -> segment.threshold at 0.3 -> objects.rules (canopy / open) -> objects.split
by the smoothed NDVI -> objects.fuse by compactness -> objects.features -> objects.extrema (the greenest crown within a
radius). Then the experiments of the page, each a run of Terraflux (0.3 s each): h, sigma, min_size, max_neck, the
distance instead of the surface, the fuse threshold and the radius of the tops.

The only numbers not from Terraflux are the box compactness of the pieces (drawn in Python from the pieces' cells, to
show what objects.fuse measures against what objects.features measures) and the toy drawing of a tilted rectangle.
There is no reference for the crowns (no height model, no field data): the figures are judged by eye, and the page says so.
Run with: python website/figures/object-based-image-analysis/tree-crowns.py   (at most 2 threads: it sets them)
"""

import os
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402

for _k in ("TERRAFLUX_THREADS", "RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "tree-crowns"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
WORK = None

X0, Y0, CELL = 705030.6, 3659058.0, 0.6  # the clip: the corner and the cell (m)
WIN = (120, 270, 0, 150)  # rows and columns of the window the figures show (90 x 90 m)
SCALE = 4
YELLOW = (1.0, 0.85, 0.1)

# the page's settings
P = dict(h=0.03, sigma=1, min_size=10, max_neck=None, by="surface", threshold=1.5, radius=15)


def model(h=P["h"], sigma=P["sigma"], min_size=P["min_size"], max_neck=P["max_neck"], by=P["by"],
          threshold=P["threshold"], radius=P["radius"], path="naip.tif", maps=True, extra="",
          fitting="compactness", fuse_params=None):
    """The page's model (with the settings of a run) and the to_map nodes the figures read."""
    split = f'by = "{by}", classes = ["canopy"], h = {h}'
    if by == "surface":
        split += f", sigma = {sigma}"
    split += f", min_size = {min_size}"
    if max_neck is not None:
        split += f", max_neck = {max_neck}"
    fuse = (f'seeds = ["canopy"], candidates = ["canopy"], fitting = "{fitting}", goal = "min", improve = false'
            + (f", threshold = {threshold}" if threshold is not None else ""))
    if fuse_params is not None:
        fuse = f'seeds = ["canopy"], candidates = ["canopy"], fitting = "{fitting}", goal = "min"' + fuse_params
    surface = ', surface = "smooth.map"' if by == "surface" else ""
    m = f"""format = "terraflux/1"

[[nodes]]
id = "naip"
op = "raster.read"
params = {{ path = "{path}" }}

[[nodes]]
id = "ndvi"
op = "raster.calc"
inputs = {{ s = "naip.raster" }}
params = {{ expression = "out ndvi = (s.nir - s.red) / (s.nir + s.red)" }}

[[nodes]]
id = "smooth"
op = "raster.focal"
inputs = {{ map = "ndvi.ndvi" }}
params = {{ stat = "mean", shape = "gaussian", radius = 3, sigma = 1 }}

[[nodes]]
id = "clumps"
op = "segment.threshold"
inputs = {{ image = "smooth.map" }}
params = {{ thresholds = [0.3], neighbours = 8 }}

[[nodes]]
id = "classes"
op = "objects.rules"
inputs = {{ objects = "clumps.objects" }}

[nodes.params]
classes = [
  {{ name = "canopy", rule = "above(class, 1, 2)" }},
  {{ name = "open", rule = "below(class, 1, 2)" }},
]

[[nodes]]
id = "split"
op = "objects.split"
inputs = {{ objects = "classes.objects"{surface} }}
params = {{ {split} }}

[[nodes]]
id = "fuse"
op = "objects.fuse"
inputs = {{ objects = "split.objects" }}
params = {{ {fuse} }}

[[nodes]]
id = "features"
op = "objects.features"
inputs = {{ objects = "fuse.objects", image = "smooth.map" }}
params = {{ features = ["mean", "max", "area", "shape"] }}

[[nodes]]
id = "tops"
op = "objects.extrema"
inputs = {{ objects = "features.objects" }}
params = {{ attribute = "max_image", kind = "max", among = ["canopy"], radius = {radius}, class = "top" }}
"""
    if maps:
        for nid, src, what in (("m_clumps", "classes", "id"), ("m_clumps_c", "classes", "class"), ("m_split", "split", "id"), ("m_split_c", "split", "class"),
                               ("m_fuse", "fuse", "id"), ("m_fuse_c", "fuse", "class"), ("m_tops", "tops", "id"),
                               ("m_tops_c", "tops", "class")):
            m += (f'\n[[nodes]]\nid = "{nid}"\nop = "objects.to_map"\ninputs = {{ objects = "{src}.objects" }}\n'
                  f'params = {{ what = "{what}" }}\n')
    return m + extra


WRITES = {f"{n}.map": f"{n}.tif" for n in ("m_clumps", "m_clumps_c", "m_split", "m_split_c", "m_fuse", "m_fuse_c", "m_tops", "m_tops_c")}
WRITES |= {"smooth.map": "smooth.tif", "split.report": "split_report.csv", "fuse.report": "fuse_report.csv",
           "tops.table": "tops.csv", "fuse.count": "fuse_count.csv"}


def read_map(path):
    with rasterio.open(path) as ds:
        return ds.read(1, masked=True).astype("float64").filled(0).astype("int64")


def read_csv(path):
    return figs.read_csv(path)


def run(name, **kw):
    """One run of the page's model with other settings: the maps and tables the figures need."""
    work = WORK / name
    work.mkdir(parents=True, exist_ok=True)
    shutil.copy(od.naip(), work / "naip.tif")
    wr = dict(WRITES)
    if kw.get("by") == "distance":
        wr.pop("smooth.map")
    figs.run(figs.add_writes(model(**kw), wr), work, "--threads", "2", "--no-journal")
    r = {k[:-4]: read_map(work / k) for k in ("m_clumps.tif", "m_clumps_c.tif", "m_split.tif", "m_split_c.tif", "m_fuse.tif",
                                              "m_fuse_c.tif", "m_tops.tif", "m_tops_c.tif")}
    out = work / "output"
    r["tops_table"] = read_csv(out / "tops.csv")
    r["fuse_report"] = read_csv(out / "fuse_report.csv")
    r["split_report"] = read_csv(out / "split_report.csv")
    # canopy = class 1 (and 3, the tops, which were canopy)
    r["n_clumps"] = len(np.unique(r["m_clumps"][r["m_clumps_c"] == 1]))
    r["n_split"] = len(np.unique(r["m_split"][r["m_split_c"] == 1]))
    r["n_fuse"] = len(np.unique(r["m_fuse"][r["m_fuse_c"] == 1]))
    cs = r["m_tops_c"]
    r["n_tops"] = len(np.unique(r["m_tops"][cs == 3]))
    r["areas"] = areas(r["m_fuse"], r["m_fuse_c"] == 1)
    r["areas_split"] = areas(r["m_split"], r["m_split_c"] == 1)
    return r


def areas(ids, mask):
    v, n = np.unique(ids[mask], return_counts=True)
    return n * CELL * CELL


def px(x, y):
    return (x - X0) / CELL, (Y0 - y) / CELL


def tops_xy(r):
    pts = []
    for x in r["tops_table"]:
        if x["extremum"] == "1":
            pts.append(px(float(x["centroid_x"]), float(x["centroid_y"])))
    return pts


def labels_of(ids, cls, keep=(1, 3)):
    """The ids with the cells of the classes not kept (the open ground) set to 0: only the canopy pieces have outlines."""
    return np.where(np.isin(cls, keep), ids, 0)


def draw(rgb, labels, name, tops=None, win=WIN, scale=SCALE, under=None, width=700, tint=None, colors=200):
    """The window of the image with the outlines of the pieces (and, if given, the tops as pink dots).
    `under` is a second set of labels drawn first in white (the pieces before the fuse)."""
    r0, r1, c0, c1 = win
    img = rgb[r0:r1, c0:c1]
    if under is not None:
        img = od.outlines(img, under[r0:r1, c0:c1], color=(1, 1, 1), scale=scale, thick=2)
        # the second set on the big image
        big = img
        lab = np.kron(labels[r0:r1, c0:c1], np.ones((scale, scale), dtype=labels.dtype))
        b = od.boundary(lab)
        b = b | np.roll(b, 1, 0) | np.roll(b, 1, 1)
        big[b] = YELLOW
        img = big
    else:
        img = od.outlines(img, labels[r0:r1, c0:c1], color=YELLOW, scale=scale, thick=2)
    if tint is not None:
        t = np.kron(tint[r0:r1, c0:c1], np.ones((scale, scale), dtype=bool))
        img = np.where(t[..., None], img * 0.6 + np.array([0.91, 0.56, 0.12]) * 0.4, img)
    fig = plt.figure(figsize=(img.shape[1] / 150, img.shape[0] / 150), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(img, interpolation="nearest")
    for x, y in tops or []:
        if r0 <= y < r1 and c0 <= x < c1:
            ax.plot((x - c0) * scale, (y - r0) * scale, "o", ms=7, mfc="#ff2a6d", mec="white", mew=1.2, zorder=5)
    ax.set_axis_off()
    fig.savefig(OUT / name, dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / name))[..., :3], OUT / name, width=width, colors=colors)


def box_compactness(ids, keep):
    """length x width / area of the axis-aligned box of every piece (what objects.fuse measures)."""
    from scipy import ndimage

    ids = np.where(keep, ids, 0)
    out = {}
    sl = ndimage.find_objects(ids)
    for i, s in enumerate(sl, 1):
        if s is None:
            continue
        n = int((ids[s] == i).sum())
        h, w = s[0].stop - s[0].start, s[1].stop - s[1].start
        out[i] = (h * w / n, n)
    return out


def main():
    global WORK
    od.naip()
    WORK = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    rgb = od.rgb("naip")

    # ------------------------------------------------------------------ the page's run
    base = run("base")
    print("clumps", base["n_clumps"], "pieces", base["n_split"], "after the fuse", base["n_fuse"], "tops", base["n_tops"])
    print("split report rows", len(base["split_report"]), "fuse report rows", len(base["fuse_report"]))
    print("median area m2", float(np.median(base["areas"])), "mean", float(np.mean(base["areas"])))
    cls_s = base["m_split_c"]
    # the model of the page, from its own text (with the figure's path)
    (WORK / "page.toml").write_text(model(path="naip_plano.tif", maps=False), encoding="utf-8")

    # ------------------------------------------------------------------ where, what the surface is
    fig = plt.figure(figsize=(4.5, 4.5), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb)
    r0, r1, c0, c1 = WIN
    ax.add_patch(plt.Rectangle((c0, r0), c1 - c0, r1 - r0, fill=False, ec=YELLOW, lw=2))
    ax.set_axis_off()
    fig.savefig(OUT / "where.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "where.png"))[..., :3], OUT / "where.png", width=420, colors=128)

    with rasterio.open(WORK / "base" / "smooth.tif") as ds:
        sm = ds.read(1, masked=True).astype("float64").filled(np.nan)
    r0, r1, c0, c1 = WIN
    fig, ax = plt.subplots(1, 3, figsize=(10.5, 3.7), dpi=150)
    ax[0].imshow(rgb[r0:r1, c0:c1])
    ax[0].set_title("NAIP, true colour", fontsize=9)
    im = ax[1].imshow(sm[r0:r1, c0:c1], cmap="viridis", vmin=-0.1, vmax=0.7)
    ax[1].set_title("Smoothed NDVI: the surface", fontsize=9)
    ax[2].imshow(rgb[r0:r1, c0:c1])
    cl = base["m_clumps"][r0:r1, c0:c1]
    canopy = np.isin(base["m_fuse_c"], (1, 3))[r0:r1, c0:c1]
    ov = np.zeros(cl.shape + (4,))
    ov[canopy] = (0.18, 0.55, 0.35, 0.55)
    ax[2].imshow(ov)
    ax[2].set_title("NDVI above 0.3: the canopy", fontsize=9)
    for a in ax:
        a.set_axis_off()
    fig.colorbar(im, ax=ax[1], fraction=0.046, pad=0.02).ax.tick_params(labelsize=7)
    fig.tight_layout()
    fig.savefig(OUT / "surface.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "surface.png"))[..., :3], OUT / "surface.png", width=1000, colors=128)

    # ------------------------------------------------------------------ the four steps of the page's run
    clump_lab = labels_of(base["m_clumps"], base["m_clumps_c"], keep=(1,))
    draw(rgb, clump_lab, "step_clumps.png", width=500, colors=128)
    draw(rgb, labels_of(base["m_split"], base["m_split_c"]), "step_split.png", width=500, colors=128)
    draw(rgb, labels_of(base["m_fuse"], base["m_fuse_c"]), "step_fuse.png", width=500, colors=128)
    draw(rgb, labels_of(base["m_tops"], base["m_tops_c"]), "step_tops.png", tops=tops_xy(base), width=500, colors=128)
    plain = od.outlines(rgb[r0:r1, c0:c1], np.zeros((r1 - r0, c1 - c0), dtype="int64"), scale=SCALE)
    od.save_png(plain, OUT / "step_rgb.png", width=500, colors=128)

    # the whole clip with its tops
    lab = labels_of(base["m_tops"], base["m_tops_c"])
    full = rgb
    fig = plt.figure(figsize=(6.6, 6.6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(full, interpolation="nearest")
    t = tops_xy(base)
    ax.plot([p[0] for p in t], [p[1] for p in t], "o", ms=3.2, mfc="#ff2a6d", mec="white", mew=0.4)
    ax.set_axis_off()
    fig.savefig(OUT / "whole.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "whole.png"))[..., :3], OUT / "whole.png", width=800, colors=128)

    # ------------------------------------------------------------------ the numbers of every stage
    figs.write_csv(DATA / "stages.csv", [
        {"stage": "Canopy clumps (segment.threshold)", "objects": base["n_clumps"]},
        {"stage": "Crowns after objects.split", "objects": base["n_split"]},
        {"stage": "After objects.fuse", "objects": base["n_fuse"]},
        {"stage": "Tops (objects.extrema)", "objects": base["n_tops"]},
    ])

    # ------------------------------------------------------------------ h
    def sweep(name, key, values, fmt, extra=None, stat="n_split", images=True):
        rows, frames = [], []
        for v in values:
            kw = dict(extra or {})
            kw[key] = v
            r = run(f"{name}_{v}", **kw)
            lab = labels_of(r["m_split"], r["m_split_c"]) if stat == "n_split" else labels_of(r["m_fuse"], r["m_fuse_c"])
            if stat == "n_tops":
                lab = labels_of(r["m_tops"], r["m_tops_c"])
            if stat == "n_fuse" and v == 0:
                lab = labels_of(r["m_split"], r["m_split_c"])  # no join: the pieces of the split
            fn = f"{name}_{str(v).replace('.', '_')}.png"
            if images:
                draw(rgb, lab, fn, tops=tops_xy(r) if stat == "n_tops" else None, width=440, colors=64)
            row = {key: v, "clumps": r["n_clumps"], "crowns": r["n_split"],
                   "clumps_cut": len(r["split_report"]), "largest_crown_m2": round(float(r["areas_split"].max()), 1)}
            if stat == "n_fuse":
                row |= {"after_fuse": r["n_fuse"], "largest_after_fuse_m2": round(float(r["areas"].max()), 1)}
            if stat == "n_tops":
                row |= {"tops": r["n_tops"]}
            rows.append(row)
            frames.append((v, fn, r))
            print(name, v, rows[-1])
        figs.write_csv(DATA / f"{name}.csv", rows)
        return rows, frames

    sweep("h", "h", [0.01, 0.02, 0.03, 0.05, 0.1, 0.2], "{}", extra={})
    sweep("sigma", "sigma", [0, 1, 2, 3], "{}", extra={})
    sweep("min", "min_size", [0, 5, 10, 25, 50], "{}", extra={}, images=False)
    sweep("neck", "max_neck", [None, 4, 2, 1, 0.5], "{}", extra={})
    sweep("fuse", "threshold", [0, 1.2, 1.3, 1.5, 2.0, 3.0], "{}", stat="n_fuse")
    sweep("radius", "radius", [5, 10, 15, 25, 40], "{}", stat="n_tops")

    # ------------------------------------------------------------------ the distance instead of the surface
    dist_rows = []
    for h in (0.3, 0.6, 1.2, 2.4):
        r = run(f"dist_{h}", by="distance", h=h, threshold=0)
        draw(rgb, labels_of(r["m_split"], r["m_split_c"]), f"dist_{str(h).replace('.', '_')}.png", width=440, colors=64)
        dist_rows.append({"by": "distance", "h": h, "clumps": r["n_clumps"], "crowns": r["n_split"],
                          "clumps_cut": len(r["split_report"]), "largest_crown_m2": round(float(r["areas_split"].max()), 1)})
        print("dist", dist_rows[-1])
    s = run("surf_nofuse", threshold=0)
    dist_rows.append({"by": "surface (NDVI)", "h": P["h"], "clumps": s["n_clumps"], "crowns": s["n_split"],
                      "clumps_cut": len(s["split_report"]), "largest_crown_m2": round(float(s["areas_split"].max()), 1)})
    figs.write_csv(DATA / "distance.csv", dist_rows)
    draw(rgb, labels_of(s["m_split"], s["m_split_c"]), "surface_split.png", width=440, colors=64)

    # ------------------------------------------------------------------ what fuse joined
    nf = run("fuse_none", threshold=0)
    before = labels_of(nf["m_split"], nf["m_split_c"])
    after = labels_of(base["m_fuse"], base["m_fuse_c"])
    joined = np.isin(base["m_fuse"], [int(r["object"]) for r in base["fuse_report"]]) & (base["m_fuse_c"] == 1)
    r0, r1, c0, c1 = WIN
    win_joined = len(np.unique(base["m_fuse"][r0:r1, c0:c1][joined[r0:r1, c0:c1]]))
    print("objects joined by the fuse in the window:", win_joined)
    draw(rgb, before, "fuse_before.png", width=520, tint=joined, colors=96)
    draw(rgb, after, "fuse_after.png", width=520, tint=joined, colors=96)
    lw = run("fuse_lw", fitting="max(compactness, length_width / 2)")
    print("fuse with the length over the width: objects", lw["n_fuse"], "largest", round(float(lw["areas"].max()), 1))
    draw(rgb, labels_of(lw["m_fuse"], lw["m_fuse_c"]), "fuse_lw.png", width=560, colors=128,
         tint=np.isin(lw["m_fuse"], [int(r["object"]) for r in lw["fuse_report"]]) & (lw["m_fuse_c"] == 1))
    figs.write_csv(DATA / "fuse_fitting.csv", [
        {"fitting": "compactness", "objects": base["n_fuse"], "joined": len(base["fuse_report"]),
         "largest_m2": round(float(base["areas"].max()), 1)},
        {"fitting": "max(compactness, length_width / 2)", "objects": lw["n_fuse"], "joined": len(lw["fuse_report"]),
         "largest_m2": round(float(lw["areas"].max()), 1)}])
    modes = []
    for label, fp in (("improve, no threshold", ""), ("improve, threshold 1.5", ", threshold = 1.5"),
                      ("no improve, threshold 1.5 (the page)", ", improve = false, threshold = 1.5"),
                      ("improve and threshold 1.5, mode all", ", threshold = 1.5, mode = \"all\""),
                      ("no improve, no threshold", ", improve = false")):
        r = run("mode_" + str(len(modes)), fuse_params=fp)
        modes.append({"setting": label, "objects": r["n_fuse"], "joined": len(r["fuse_report"]),
                      "largest_m2": round(float(r["areas"].max()), 1)})
        print("fuse mode", modes[-1])
    figs.write_csv(DATA / "fuse_modes.csv", modes)
    figs.write_csv(DATA / "fuse_report.csv", [{"object": r["object"], "parts": r["parts"]} for r in base["fuse_report"][:12]])
    parts = [int(r["parts"]) for r in base["fuse_report"]]
    print("fuse joins:", len(parts), "objects, parts per object", {k: parts.count(k) for k in sorted(set(parts))})

    # ------------------------------------------------------------------ the compactness of what the fuse joined
    # objects.fuse measures the rotated minimum rectangle, as objects.features: the joined objects, measured by
    # objects.features after the fuse, must reach the threshold
    ft = {int(float(x["id"])): x for x in base["tops_table"]}
    joined_ids = [int(r["object"]) for r in base["fuse_report"]]
    jc = np.array([float(ft[i]["compactness"]) for i in joined_ids if i in ft])
    allc = np.array([float(x["compactness"]) for x in base["tops_table"] if x["extremum"] != ""])
    print("compactness (objects.features) of the", len(jc), "objects made by joins: max", float(jc.max()),
          "median", float(np.median(jc)), "| all canopy objects: median", float(np.median(allc)),
          "share above 1.5", float((allc > 1.5).mean()))
    figs.write_csv(DATA / "fuse_check.csv", [
        {"objects": "made by a join", "count": len(jc), "median_compactness": round(float(np.median(jc)), 2),
         "max_compactness": round(float(jc.max()), 2)},
        {"objects": "all canopy objects", "count": len(allc), "median_compactness": round(float(np.median(allc)), 2),
         "max_compactness": round(float(allc.max()), 2)}])

    # the profile: what h means (an explanatory drawing)
    x = np.linspace(0, 10, 400)
    y = 0.45 * np.exp(-((x - 2.5) / 1.0) ** 2) + 0.38 * np.exp(-((x - 6.0) / 1.1) ** 2) + 0.30 * np.exp(-((x - 8.4) / 0.7) ** 2) + 0.12
    fig, ax = plt.subplots(figsize=(7.4, 3.0), dpi=150)
    ax.plot(x, y, color=figs.FOREST, lw=2)
    ax.fill_between(x, y, 0, color=figs.FOREST, alpha=0.12)
    pk = [(2.5, y[np.argmin(abs(x - 2.5))]), (6.0, y[np.argmin(abs(x - 6.0))]), (8.4, y[np.argmin(abs(x - 8.4))])]
    sd = [(4.1, y[np.argmin(abs(x - 4.1))]), (7.5, y[np.argmin(abs(x - 7.5))])]
    sd = [(xx, min(y[(x > a) & (x < b)])) for (xx, _), (a, b) in zip(sd, ((2.5, 6.0), (6.0, 8.4)))]
    sd = [(x[(x > a) & (x < b)][np.argmin(y[(x > a) & (x < b)])], m) for (a, b), (_, m) in zip(((2.5, 6.0), (6.0, 8.4)), sd)]
    for (px_, py_) in pk:
        ax.plot(px_, py_, "o", color="#ff2a6d", mec="white", ms=7, zorder=5)
    for (sx, sy), (px_, py_) in zip(sd, pk[1:]):
        ax.plot([sx, sx], [sy, py_], color=figs.EMBER, lw=1.6)
        ax.annotate("", xy=(sx, py_), xytext=(sx, sy), arrowprops=dict(arrowstyle="<->", color=figs.EMBER, lw=1.4))
    ax.text(sd[0][0] + 0.12, (sd[0][1] + pk[1][1]) / 2, "h: a big peak", color=figs.EMBER, fontsize=8, va="center")
    ax.text(sd[1][0] + 0.12, (sd[1][1] + pk[2][1]) / 2, "less than h: no crown", color=figs.EMBER, fontsize=8, va="center")
    ax.text(pk[0][0], pk[0][1] + 0.02, "centre", ha="center", fontsize=8)
    ax.text(pk[1][0], pk[1][1] + 0.02, "centre", ha="center", fontsize=8)
    ax.text(pk[2][0], pk[2][1] + 0.02, "bump", ha="center", fontsize=8)
    ax.set_xlabel("distance along a line across the canopy")
    ax.set_ylabel("surface (NDVI)")
    ax.set_yticks([])
    ax.set_xticks([])
    ax.set_ylim(0, 0.68)
    fig.tight_layout()
    fig.savefig(OUT / "profile.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "profile.png"))[..., :3], OUT / "profile.png", width=800, colors=64)

    # ------------------------------------------------------------------ the outputs of the page (tables)
    figs.write_csv(DATA / "split_report.csv", [dict(r) for r in base["split_report"][:10]])
    print("split report columns", list(base["split_report"][0].keys()) if base["split_report"] else None)


if __name__ == "__main__":
    main()
