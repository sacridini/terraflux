"""Figures of "Roads and rivers by the skeleton" (tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton.md).

Three runs of Terraflux, all with the skeleton group of objects.features:
  * a toy raster of nine shapes of about the same area (shapes.tif, drawn here and published as the page's download):
    the table of what the skeleton gives for each, and what `spur` does to a ragged bar;
  * the NAIP clip of Plano, Texas (obia_data.py, read only): the light surfaces (bright, no vegetation) cut into
    objects with segment.threshold, their skeleton and area, and objects.rules that tell the long thin ones (streets)
    from the compact ones (roofs, courts) and the small ones; the sweep of `spur`; the objects' numbers;
  * the Sentinel-2 clip of Lago Paranoa (obia_data.py's published download): the very green cells (NDVI above 0.65),
    the gallery forests along the creeks against the patches of forest.

The skeletons DRAWN on the figures are an illustration made in Python (skimage's Zhang thinning of each object's cells,
with no pruning and none of Terraflux's clean-ups): Terraflux gives the numbers of the skeleton but not its cells.
Every number of the page comes from Terraflux. There is no reference for the classes: they are judged by eye.
Run with: python website/figures/object-based-image-analysis/roads-and-rivers-by-the-skeleton.py   (at most 2 threads)
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
from rasterio.transform import from_origin  # noqa: E402
from skimage.morphology import skeletonize  # noqa: E402

for _k in ("TERRAFLUX_THREADS", "RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "roads-and-rivers-by-the-skeleton"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
WORK = None

COLS = ["area", "skel_main_length", "skel_width", "skel_length_width", "skel_branches", "skel_ends", "skel_junctions",
        "skel_curvature"]
CLASS_COLOR = {"linear": "#2b6cb0", "compact": "#c0504d", "small": "#8f8f8f"}
NAIP_WIN = (250, 400, 560, 710)  # rows and columns of the detail (a curved street, a park, houses)
BLUE, RED, YEL = (0.17, 0.42, 0.69), (0.75, 0.31, 0.30), (1.0, 0.85, 0.1)


def num(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return float("nan")


def read_map(path):
    with rasterio.open(path) as ds:
        return ds.read(1, masked=True).astype("float64").filled(0).astype("int64")


def run(name, model, writes, files=None):
    work = WORK / name
    work.mkdir(parents=True, exist_ok=True)
    for src, dst in (files or {}).items():
        shutil.copy(src, work / dst)
    figs.run(figs.add_writes(model, writes), work, "--threads", "2", "--no-journal")
    return work


def table(path):
    return {int(float(r["id"])): r for r in figs.read_csv(path)}


def save(fig, name, width=900, colors=200):
    fig.savefig(OUT / name, dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / name))[..., :3], OUT / name, width=width, colors=colors)


# ------------------------------------------------------------------------------------------------------ the toy


def toy():
    """Nine shapes of about 400 cells on a 240 x 360 grid of 1 m cells."""
    H, W = 240, 360
    a = np.zeros((H, W), np.uint8)
    yy, xx = np.mgrid[0:H, 0:W]
    shapes = {}

    def put(name, mask):
        a[mask] = 1
        shapes[name] = mask

    put("square", (yy >= 20) & (yy < 40) & (xx >= 20) & (xx < 40))
    put("disc", (yy - 30) ** 2 + (xx - 80) ** 2 <= 11.3**2)
    put("straight bar", (yy >= 28) & (yy < 32) & (xx >= 110) & (xx < 210))
    t = np.deg2rad(25)
    u = (xx - 290) * np.cos(t) + (yy - 30) * np.sin(t)
    v = -(xx - 290) * np.sin(t) + (yy - 30) * np.cos(t)
    put("tilted bar", (abs(u) < 50) & (abs(v) < 2))
    cx, cy, R = 70, 170, 64
    ang = np.arctan2(-(yy - cy), (xx - cx))
    put("arc", (abs(np.hypot(xx - cx, yy - cy) - R) < 2) & (ang >= 0) & (ang <= np.pi / 2))
    put("L", ((xx >= 148) & (xx < 152) & (yy >= 120) & (yy < 174)) | ((yy >= 170) & (yy < 174) & (xx >= 148) & (xx < 202)))
    put("cross", ((yy >= 128) & (yy < 132) & (xx >= 240) & (xx < 290)) | ((xx >= 263) & (xx < 267) & (yy >= 105) & (yy < 155)))
    put("ring", abs(np.hypot(xx - 40, yy - 210) - 16) < 2)
    rng = np.random.default_rng(3)
    noise = np.zeros((H, W), bool)
    for x in range(100, 200, 5):
        if rng.random() < 0.7:
            d = int(rng.integers(1, 4))
            s = int(rng.choice([-1, 1]))
            lo, hi = (212, 212 + d) if s > 0 else (208 - d, 208)
            noise[(yy >= lo) & (yy < hi) & (xx >= x) & (xx < x + 2)] = True
    put("ragged bar", ((yy >= 208) & (yy < 212) & (xx >= 100) & (xx < 200)) | noise)
    return a, shapes


def toy_model(spur=None):
    sp = "" if spur is None else f", spur = {spur}"
    return f"""format = "terraflux/1"

[[nodes]]
id = "shapes"
op = "raster.read"
params = {{ path = "shapes.tif" }}

[[nodes]]
id = "pieces"
op = "segment.threshold"
inputs = {{ image = "shapes.raster" }}
params = {{ thresholds = [0.5], neighbours = 8 }}

[[nodes]]
id = "features"
op = "objects.features"
inputs = {{ objects = "pieces.objects", image = "shapes.raster" }}
params = {{ features = ["area", "skeleton"]{sp} }}

[[nodes]]
id = "ids"
op = "objects.to_map"
inputs = {{ objects = "features.objects" }}
params = {{ what = "id" }}
"""


def do_toy():
    a, shapes = toy()
    DATA.mkdir(parents=True, exist_ok=True)
    tif = DATA / "shapes.tif"
    with rasterio.open(tif, "w", driver="GTiff", height=a.shape[0], width=a.shape[1], count=1, dtype="uint8",
                       crs="EPSG:26914", transform=from_origin(0, a.shape[0], 1, 1), compress="deflate") as ds:
        ds.write(a, 1)
        ds.descriptions = ("shape",)
    w = run("toy", toy_model(), {"features.table": "features.csv", "ids.map": "ids.tif"}, {tif: "shapes.tif"})
    ids = read_map(w / "ids.tif")
    ft = table(w / "output" / "features.csv")
    rows, pick = [], {}
    for name, m in shapes.items():
        i = int(np.bincount(ids[m & (ids > 0)]).argmax())
        pick[name] = i
        x = ft[i]
        rows.append({"shape": name, **{c: num(x[c]) for c in COLS}, "curv_sd": num(x["skel_curvature_sd"])})
        print(name, {k: round(v, 3) if isinstance(v, float) else v for k, v in rows[-1].items() if k != "shape"})
    figs.write_csv(DATA / "toy.csv", [{k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()}
                                      for r in rows])
    # the illustration: the shapes and a thinning of their cells (skimage; nothing of Terraflux's clean-up)
    fig, ax = plt.subplots(3, 3, figsize=(9.2, 6.4), dpi=150)
    boxes = {"square": (10, 50, 10, 50), "disc": (10, 50, 60, 100), "straight bar": (10, 50, 105, 215),
             "tilted bar": (-5, 70, 235, 345), "arc": (100, 176, 64, 140), "L": (110, 180, 140, 210),
             "cross": (95, 165, 230, 300), "ring": (185, 235, 15, 65), "ragged bar": (195, 225, 95, 205)}
    for a_, (name, m) in zip(ax.flat, shapes.items()):
        r0, r1, c0, c1 = boxes[name]
        r0, c0 = max(r0, 0), max(c0, 0)
        sub = m[r0:r1, c0:c1]
        sk = skeletonize(sub)
        img = np.ones(sub.shape + (3,))
        img[sub] = (0.78, 0.86, 0.80)
        img[sk] = (0.10, 0.35, 0.22)
        a_.imshow(img, interpolation="nearest")
        x = [r for r in rows if r["shape"] == name][0]
        a_.set_title(f"{name}: {x['skel_main_length']:.0f} m long, "
                     + (f"{x['skel_width']:.1f} m wide" if x["skel_width"] == x["skel_width"] else "no width"),
                     fontsize=8)
        a_.set_axis_off()
    fig.suptitle("Nine shapes of about 400 m2: the cells (light) and an illustrative thinning (dark)", fontsize=9)
    fig.tight_layout()
    save(fig, "toy.png", width=1000)
    # the spur: the ragged bar with other values
    out = []
    for sp in (0, 1, 2, 3, 5, 8):
        w = run(f"toy_spur{sp}", toy_model(sp), {"features.table": "features.csv"}, {tif: "shapes.tif"})
        x = table(w / "output" / "features.csv")[pick["ragged bar"]]
        out.append({"spur": sp, "branches": int(num(x["skel_branches"])), "ends": int(num(x["skel_ends"])),
                    "junctions": int(num(x["skel_junctions"])), "main_length_m": round(num(x["skel_main_length"]), 1)})
        print("toy spur", out[-1])
    figs.write_csv(DATA / "toy_spur.csv", out)


# ------------------------------------------------------------------------------------------------------ NAIP

NAIP_RULES = """classes = [
  { name = "linear", rule = "above(class, 1, 2) and above(skel_main_length, 40, 60) and above(skel_length_width, 8, 12)", colour = "#2b6cb0" },
  { name = "compact", rule = "above(class, 1, 2) and above(area, 30, 60) and below(skel_length_width, 8, 12)", colour = "#c0504d" },
  { name = "small", rule = "above(class, 1, 2) and below(area, 30, 60)", colour = "#8f8f8f" },
]"""


def naip_model(spur=3, path="naip.tif", maps=True, rules=True):
    sp = f", spur = {spur}"
    m = f"""format = "terraflux/1"

[[nodes]]
id = "naip"
op = "raster.read"
params = {{ path = "{path}" }}

[[nodes]]
id = "light"
op = "raster.calc"
inputs = {{ s = "naip.raster" }}
params = {{ expression = "out light = 1 if ((s.red + s.green + s.blue) / 3 > 150 and (s.nir - s.red) / (s.nir + s.red) < 0.15) else 0" }}

[[nodes]]
id = "pieces"
op = "segment.threshold"
inputs = {{ image = "light.light" }}
params = {{ thresholds = [0.5], neighbours = 8 }}

[[nodes]]
id = "features"
op = "objects.features"
inputs = {{ objects = "pieces.objects", image = "light.light" }}
params = {{ features = ["area", "skeleton"]{sp} }}
"""
    if rules:
        m += f"""
[[nodes]]
id = "rules"
op = "objects.rules"
inputs = {{ objects = "features.objects" }}

[nodes.params]
{NAIP_RULES}
"""
    if maps:
        m += """
[[nodes]]
id = "ids"
op = "objects.to_map"
inputs = { objects = "features.objects" }
params = { what = "id" }
"""
        if rules:
            m += """
[[nodes]]
id = "cls"
op = "objects.to_map"
inputs = { objects = "rules.objects" }
params = { what = "class" }
"""
    return m


def naip_run(name, spur=3):
    w = run(name, naip_model(spur), {"features.table": "features.csv", "rules.table": "rules.csv", "ids.map": "ids.tif",
                                     "cls.map": "cls.tif", "light.light": "light.tif"}, {od.naip(): "naip.tif"})
    ft = table(w / "output" / "features.csv")
    rt = table(w / "output" / "rules.csv")
    return w, ft, rt, read_map(w / "ids.tif"), read_map(w / "cls.tif")


def do_naip(rgb):
    w, ft, rt, ids, cls = naip_run("naip")
    # light objects: class 2 of the threshold
    light = {i: x for i, x in ft.items() if num(x["class"]) == 2}
    names = ["linear", "compact", "small"]
    by = {n: [i for i in light if rt[i]["class"] == n] for n in names}
    none = [i for i in light if rt[i]["class"] == ""]
    print("light objects", len(light), {n: len(v) for n, v in by.items()}, "unclassified", len(none))
    area = lambda ids_: sum(num(light[i]["area"]) for i in ids_)  # noqa: E731
    print("area (m2)", {n: round(area(v)) for n, v in by.items()}, "unclassified", round(area(none)))

    # the map of the whole clip
    img = rgb.copy()
    for k, col in ((1, BLUE), (2, RED), (3, (0.55, 0.55, 0.55))):
        m = cls == k
        img[m] = img[m] * 0.35 + np.array(col) * 0.65
    fig = plt.figure(figsize=(6.6, 6.6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(img, interpolation="nearest")
    r0, r1, c0, c1 = NAIP_WIN
    ax.add_patch(plt.Rectangle((c0, r0), c1 - c0, r1 - r0, fill=False, ec=YEL, lw=1.6))
    ax.set_axis_off()
    save(fig, "classes.png", width=900)
    od.save_png(rgb, OUT / "naip.png", width=700)

    # the same light surfaces in one colour: what the objects are before the rules
    lightmap = read_map(w / "light.tif")
    img2 = rgb.copy()
    img2[lightmap == 1] = img2[lightmap == 1] * 0.35 + np.array((0.95, 0.75, 0.1)) * 0.65
    fig = plt.figure(figsize=(6.6, 6.6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(img2, interpolation="nearest")
    ax.set_axis_off()
    save(fig, "light.png", width=900)

    # the detail with the illustrative skeletons
    r0, r1, c0, c1 = NAIP_WIN
    s = 4
    base = rgb[r0:r1, c0:c1]
    big = np.kron(base, np.ones((s, s, 1)))
    sub_ids = ids[r0:r1, c0:c1]
    sub_cls = cls[r0:r1, c0:c1]
    tint = np.zeros(sub_ids.shape + (3,))
    for k, col in ((1, BLUE), (2, RED), (3, (0.55, 0.55, 0.55))):
        tint[sub_cls == k] = col
    mask = np.kron((sub_cls > 0).astype(float), np.ones((s, s))) > 0
    bigtint = np.kron(tint, np.ones((s, s, 1)))
    big = np.where(mask[..., None], big * 0.55 + bigtint * 0.45, big)
    fig = plt.figure(figsize=(big.shape[1] / 150, big.shape[0] / 150), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(big, interpolation="nearest")
    # skeleton of each light object seen in the window, drawn from its whole mask (the objects reach beyond the window)
    seen = [int(i) for i in np.unique(sub_ids[(sub_cls > 0) & (sub_cls < 3)])]
    for i in seen:
        m = ids == i
        rr, cc = np.where(m)
        ra, rb, ca, cb = rr.min(), rr.max() + 1, cc.min(), cc.max() + 1
        sk = skeletonize(m[ra:rb, ca:cb])
        y, x = np.where(sk)
        y, x = y + ra, x + ca
        ok = (y >= r0) & (y < r1) & (x >= c0) & (x < c1)
        ax.plot((x[ok] - c0 + 0.5) * s, (y[ok] - r0 + 0.5) * s, ",", color="#ffffff", ms=1)
        ax.scatter((x[ok] - c0 + 0.5) * s, (y[ok] - r0 + 0.5) * s, s=1.2, c="#ffffff", linewidths=0)
    ax.set_axis_off()
    save(fig, "detail.png", width=720)
    print("objects in the detail:", len(seen))

    # area against length / width
    fig, ax = plt.subplots(figsize=(6.2, 4.4), dpi=150)
    for n in names:
        pts = [(num(light[i]["area"]), num(light[i]["skel_length_width"])) for i in by[n]]
        pts = [p for p in pts if p[1] == p[1]]
        ax.scatter([p[0] for p in pts], [p[1] for p in pts], s=8, alpha=0.6, color=CLASS_COLOR[n], label=n, linewidths=0)
    ax.set_xscale("log")
    ax.axhspan(8, 12, color=figs.EMBER, alpha=0.12, lw=0)
    ax.text(ax.get_xlim()[1] * 0.95, 10, "ramp 8 to 12", color=figs.EMBER, fontsize=8, ha="right", va="center")
    ax.set_xlabel("area of the object (m2, log scale)")
    ax.set_ylabel("skel_length_width")
    ax.legend(frameon=False, fontsize=8, loc="upper left")
    fig.tight_layout()
    save(fig, "scatter.png", width=760)
    # the same area, both kinds: objects of 500 to 1500 m2
    sel = [i for i in light if 500 <= num(light[i]["area"]) <= 1500]
    both = {n: [i for i in sel if rt[i]["class"] == n] for n in ("linear", "compact")}
    print("objects of 500 to 1500 m2:", {n: len(v) for n, v in both.items()}, "unclassified",
          len([i for i in sel if rt[i]["class"] == ""]))
    rows = []
    for n in ("linear", "compact"):
        for q, key in (("area", "area"), ("main length", "skel_main_length"), ("width", "skel_width"),
                       ("length / width", "skel_length_width"), ("branches", "skel_branches")):
            v = np.array([num(light[i][key]) for i in both[n]])
            rows.append({"class": n, "objects": len(v), "measure": q, "median": round(float(np.nanmedian(v)), 1),
                         "q1": round(float(np.nanpercentile(v, 25)), 1), "q3": round(float(np.nanpercentile(v, 75)), 1)})
    figs.write_csv(DATA / "same_area.csv", rows)
    for r in rows:
        print(r)
    # the biggest linear objects
    big_ids = sorted(by["linear"], key=lambda i: -num(light[i]["area"]))[:8]
    figs.write_csv(DATA / "linear_top.csv", [
        {"id": i, "area_m2": round(num(light[i]["area"])), "main_length_m": round(num(light[i]["skel_main_length"]), 1),
         "width_m": round(num(light[i]["skel_width"]), 1), "length_width": round(num(light[i]["skel_length_width"]), 1),
         "branches": int(num(light[i]["skel_branches"])), "junctions": int(num(light[i]["skel_junctions"]))}
        for i in big_ids])
    figs.write_csv(DATA / "classes.csv", [
        {"class": n, "objects": len(by[n]), "area_ha": round(area(by[n]) / 1e4, 2)} for n in names] +
        [{"class": "no class", "objects": len(none), "area_ha": round(area(none) / 1e4, 2)}])
    # the biggest object: the street network
    top = max(light, key=lambda i: num(light[i]["area"]))
    print("biggest light object", top, {c: num(light[top][c]) for c in COLS})
    # the spur
    out = []
    for sp in (0, 1, 3, 6, 10, 20):
        w2, ft2, rt2, _, _ = naip_run(f"naip_spur{sp}", sp)
        lt = {i: x for i, x in ft2.items() if num(x["class"]) == 2}
        b = np.array([num(lt[i]["skel_branches"]) for i in lt if num(lt[i]["area"]) >= 60])
        n_lin = sum(1 for i in lt if rt2[i]["class"] == "linear")
        n_cmp = sum(1 for i in lt if rt2[i]["class"] == "compact")
        out.append({"spur": sp, "branches_of_the_biggest": int(num(lt[top]["skel_branches"])),
                    "main_length_of_the_biggest_m": round(num(lt[top]["skel_main_length"]), 1),
                    "median_branches": float(np.median(b)), "linear_objects": n_lin, "compact_objects": n_cmp})
        print("spur", out[-1])
    figs.write_csv(DATA / "spur.csv", out)
    # the units of the same number at another cell size: the skeleton of the biggest object, branches by length
    return ft, rt, ids, cls


# ------------------------------------------------------------------------------------------------ Sentinel-2

S2_RULES = """classes = [
  { name = "gallery forest", rule = "above(class, 1, 2) and above(area, 20000, 40000) and above(skel_length_width, 8, 14)", colour = "#2b6cb0" },
  { name = "forest patch", rule = "above(class, 1, 2) and above(area, 20000, 40000) and below(skel_length_width, 8, 14)", colour = "#c0504d" },
  { name = "small", rule = "above(class, 1, 2) and below(area, 20000, 40000)", colour = "#8f8f8f" },
]"""


def s2_model(path="sentinel2_brasilia.tif", maps=True):
    m = f"""format = "terraflux/1"

[[nodes]]
id = "s2"
op = "raster.read"
params = {{ path = "{path}" }}

[[nodes]]
id = "green"
op = "raster.calc"
inputs = {{ s = "s2.raster" }}
params = {{ expression = "out green = 1 if (s.nir - s.red) / (s.nir + s.red) > 0.65 else 0" }}

[[nodes]]
id = "pieces"
op = "segment.threshold"
inputs = {{ image = "green.green" }}
params = {{ thresholds = [0.5], neighbours = 8 }}

[[nodes]]
id = "features"
op = "objects.features"
inputs = {{ objects = "pieces.objects", image = "green.green" }}
params = {{ features = ["area", "skeleton"] }}

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = {{ objects = "features.objects" }}

[nodes.params]
{S2_RULES}
"""
    if maps:
        m += """
[[nodes]]
id = "ids"
op = "objects.to_map"
inputs = { objects = "features.objects" }
params = { what = "id" }

[[nodes]]
id = "cls"
op = "objects.to_map"
inputs = { objects = "rules.objects" }
params = { what = "class" }
"""
    return m


def do_s2():
    src = od.DOWNLOADS / "sentinel2_brasilia.tif"
    assert src.exists(), 'run obia_data.py first: it publishes the clips'
    w = run("s2", s2_model(), {"features.table": "features.csv", "rules.table": "rules.csv", "ids.map": "ids.tif",
                               "cls.map": "cls.tif"}, {src: "sentinel2_brasilia.tif"})
    ft, rt = table(w / "output" / "features.csv"), table(w / "output" / "rules.csv")
    ids, cls = read_map(w / "ids.tif"), read_map(w / "cls.tif")
    rgb = od.rgb("sentinel2")
    img = rgb.copy()
    for k, col in ((1, BLUE), (2, RED), (3, (0.7, 0.7, 0.7))):
        m = cls == k
        img[m] = img[m] * 0.3 + np.array(col) * 0.7
    fig = plt.figure(figsize=(6.6, 6.6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(img, interpolation="nearest")
    # the illustrative thinning of the gallery forests
    for i in [i for i in ft if rt[i]["class"] == "gallery forest"]:
        m = ids == i
        rr, cc = np.where(m)
        ra, rb, ca, cb = rr.min(), rr.max() + 1, cc.min(), cc.max() + 1
        y, x = np.where(skeletonize(m[ra:rb, ca:cb]))
        ax.scatter(x + ca, y + ra, s=0.8, c="#ffffff", linewidths=0)
    ax.set_axis_off()
    save(fig, "s2_classes.png", width=900)
    od.save_png(rgb, OUT / "s2.png", width=700)
    rows = []
    green = {i: x for i, x in ft.items() if num(x["class"]) == 2}
    for n in ("gallery forest", "forest patch", "small"):
        v = [i for i in green if rt[i]["class"] == n]
        rows.append({"class": n, "objects": len(v), "area_ha": round(sum(num(green[i]["area"]) for i in v) / 1e4, 1)})
    none = [i for i in green if rt[i]["class"] == ""]
    rows.append({"class": "no class", "objects": len(none), "area_ha": round(sum(num(green[i]["area"]) for i in none) / 1e4, 1)})
    figs.write_csv(DATA / "s2_classes.csv", rows)
    print(rows)
    top = sorted(green, key=lambda i: -num(green[i]["area"]))[:6]
    figs.write_csv(DATA / "s2_top.csv", [
        {"id": i, "class": rt[i]["class"] or "no class", "area_ha": round(num(green[i]["area"]) / 1e4, 1),
         "main_length_m": round(num(green[i]["skel_main_length"])), "width_m": round(num(green[i]["skel_width"]), 1),
         "length_width": round(num(green[i]["skel_length_width"]), 1), "branches": int(num(green[i]["skel_branches"])),
         "junctions": int(num(green[i]["skel_junctions"]))} for i in top])
    for r in figs.read_csv(DATA / "s2_top.csv"):
        print(r)


def main():
    global WORK
    od.naip()
    WORK = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    do_toy()
    do_naip(od.rgb("naip"))
    do_s2()
    (WORK / "page_naip.toml").write_text(naip_model(path="naip_plano.tif", maps=False), encoding="utf-8")


if __name__ == "__main__":
    main()
