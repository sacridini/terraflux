"""Figures of tutorials/distance-restoration/original-vegetation-and-habitat: the original vegetation of
converted land and the area of habitat of eight species in Serra Verde."""

import numpy as np
from matplotlib.colors import to_rgba
from matplotlib import pyplot as plt
from matplotlib.patches import Patch

import _common as C
from _common import figs

SLUG = "distance-restoration/original-vegetation-and-habitat"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
C.serra(work, "land_cover", "past_cover", "features", "ecoregions")

MODEL = f"""format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = {{ path = "land_cover.tif" }}

[[nodes]]
id = "past"
op = "raster.read"
params = {{ path = "past_cover.tif" }}

[[nodes]]
id = "ecoregions"
op = "raster.read"
params = {{ path = "ecoregions.tif" }}

[[nodes]]
id = "eco_forest"
op = "raster.calc"
inputs = {{ e = "ecoregions.raster" }}
params = {{ expression = "0.31 if e == 1 else 0.98 if e == 2 else 0.91 if e == 3 else 0.2" }}

[[nodes]]
id = "eco_savanna"
op = "raster.calc"
inputs = {{ e = "ecoregions.raster" }}
params = {{ expression = "0 if e == 1 else 0.01 if e == 2 else 0.09 if e == 3 else 0.8" }}

[[nodes]]
id = "eco_mangrove"
op = "raster.calc"
inputs = {{ e = "ecoregions.raster" }}
params = {{ expression = "0.69 if e == 1 else 0.01 if e == 2 else 0" }}

[[nodes]]
id = "eco_shares"
op = "stack.of"
inputs = {{ forest = "eco_forest.map", savanna = "eco_savanna.map", mangrove = "eco_mangrove.map" }}

[[nodes]]
id = "vegetation"
op = "restoration.original_vegetation"
inputs = {{ land_cover = "cover.raster", past = "past.raster"ECO }}
params = {{ natural = ["forest", "savanna", "mangrove"], anthropic = ["pasture", "cropland"]RESTORE }}

[[nodes]]
id = "ranges"
op = "raster.read"
params = {{ path = "features.tif" }}

[[nodes]]
id = "habitat"
op = "sdm.area_of_habitat"
inputs = {{ ranges = "ranges.raster", land_cover = "cover.raster", original = "vegetation.original", restore = "vegetation.restore", anthropic = "vegetation.anthropic" }}
params = {{ natural = ["forest", "savanna", "mangrove"], suitability = {C.SUITABILITY} }}
"""


def model(restore="", eco=True):
    return MODEL.replace("RESTORE", restore).replace("ECO", ', ecoregions = "eco_shares.stack"' if eco else "")


W = 700
writes = {"vegetation.original": "original.tif", "vegetation.restore": "restore.tif", "vegetation.source": "source.tif",
          "vegetation.anthropic": "anthropic.tif", "habitat.gain": "gain.tif", "habitat.table": "aoh.csv",
          "vegetation.failsafe": "failsafe.csv"}
figs.run(figs.add_writes(model(), writes), work)
aoh = C.table(work, "aoh")
print("failsafe", C.table(work, "failsafe"))
src = figs.read(work / "source.tif")[0]
u, c = np.unique(src.compressed(), return_counts=True)
print("source", dict(zip(u.tolist(), c.tolist())))
anth = figs.read(work / "anthropic.tif")[0]

# the land cover: the share of each class
import rasterio  # noqa: E402

with rasterio.open(work / "land_cover.tif") as ds:
    lc = ds.read(masked=True).astype(float)
    names = list(ds.descriptions)
COL = {"forest": "#3b8a5a", "savanna": "#c9b458", "mangrove": "#2f8f8f", "pasture": "#e2a960", "cropland": "#b46a3c",
       "urban": "#7a5c8c"}
# a mixed colour per cell: the classes' colours weighted by their shares
rgb = np.zeros(lc.shape[1:] + (3,))
for k, n in enumerate(names):
    rgb += lc[k].filled(0)[..., None] * np.array(to_rgba(COL[n])[:3])
tot = lc.sum(0).filled(0)
rgb = np.where(tot[..., None] > 0, rgb / np.maximum(tot, 1e-9)[..., None], 1)
alpha = np.where(lc[0].mask, 0, 1)[..., None]
img = np.concatenate([rgb, alpha], -1)
fig = plt.figure(figsize=(W / 150, W / 150 * 60 / 90 * 0.78), dpi=150)
ax = fig.add_axes([0, 0, 0.74, 1])
ax.imshow(img, interpolation="nearest")
ax.set_axis_off()
fig.legend(handles=[Patch(color=COL[n], label=n.capitalize()) for n in names], loc="center left",
           bbox_to_anchor=(0.75, 0.5), fontsize=9, handlelength=1.1)
fig.savefig(out / "land_cover.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
plt.close(fig)
figs._shrink(out / "land_cover.png")
print("class areas km2", {n: round(float(lc[k].sum()), 1) for k, n in enumerate(names)})

figs.raster(work / "anthropic.tif", out / "anthropic.png", array=np.ma.masked_where(anth <= 0, anth), cmap="YlOrBr",
            vmin=0, vmax=1, unit="share of pasture + cropland", width=W)
for b, n in enumerate(C.NATURAL, start=1):
    o = figs.read(work / "original.tif", b)[0]
    figs.raster(work / "original.tif", out / f"original_{n}.png", array=np.ma.masked_where(anth <= 0, o),
                cmap="Greens" if n == "forest" else "YlOrBr" if n == "savanna" else "GnBu", vmin=0, vmax=1,
                unit=f"original share of {n}", width=520)
figs.raster(work / "source.tif", out / "source.png", array=np.ma.masked_where(anth <= 0, src),
            palette={0: ("Past cover", "#3b8a5a"), 1: ("Ecoregion", "#2f6f9f"), 2: ("Failsafe (map mean)", "#e8901f")},
            counts=True, width=W)

# the area of habitat of each species
rows = []
for s, r in zip(C.SPECIES, aoh):
    rows.append({"species": C.NAMES[s], "current": round(r["current"], 1), "potential": round(r["potential"], 1),
                 "left": round(r["current"] / r["potential"] * 100, 1),
                 "risk": round(1 - (r["current"] / r["potential"]) ** 0.25, 4)})
figs.write_csv(data / "aoh.csv", rows)
for r in rows:
    print(r)

frames = []
for b, s in enumerate(C.SPECIES, start=1):
    gmap = figs.read(work / "gain.tif", b)[0]
    rng = figs.read(work / "features.tif", b)[0]
    arr = np.ma.masked_where(rng.filled(0) <= 0, gmap)
    figs.raster(work / "gain.tif", out / f"gain_{s}.png", array=arr, cmap="viridis", vmin=0, vmax=1,
                unit="habitat gained", width=W)
    frames.append(s)

# the species-area relationship: risk against the habitat left
def draw_sar(ax):
    r = np.linspace(0.001, 1, 300)
    for z, ls in ((0.15, ":"), (0.25, "-"), (0.35, "--")):
        ax.plot(r * 100, 1 - r ** z, color=figs.FOREST if z == 0.25 else figs.INK2, lw=2.2 if z == 0.25 else 1.2,
                ls=ls, label=f"z = {z}")
    for row in rows:
        ax.plot(row["left"], row["risk"], "o", color=figs.EMBER, ms=6)
    offsets = {"Golden lion tamarin": (-38, 0.12), "Mangrove": (-4, 0.16), "Lowland tapir": (-30, 0.13)}
    for row in rows:
        if row["species"] in offsets:
            dx, dy = offsets[row["species"]]
            lab = row["species"] + (" and otter" if row["species"] == "Lowland tapir" else "")
            ax.annotate(lab, (row["left"], row["risk"]), (row["left"] + dx, row["risk"] + dy),
                        fontsize=8.5, color=figs.INK2, arrowprops=dict(arrowstyle="-", color=figs.INK2, lw=0.6))
    ax.set_xlabel("habitat left (% of the potential)")
    ax.set_ylabel("extinction risk 1 - (left)^z")
    ax.set_xlim(0, 101)
    ax.set_ylim(0, 0.6)
    ax.legend(loc="upper right")


figs.chart(out / "sar.png", draw_sar, width=820, height=420)

# ------------------------------------------------------------------ without the ecoregions' shares
alt = figs.workdir(f"{SLUG}/flat")
C.serra(alt, "land_cover", "past_cover", "features", "ecoregions")
figs.run(figs.add_writes(model(eco=False), {"vegetation.original": "original.tif", "vegetation.source": "source.tif",
                                            "habitat.table": "aoh.csv"}), alt)
for tag, w in (("with", work), ("without", alt)):
    o = figs.read(w / "original.tif", 3)[0]
    figs.raster(w / "original.tif", out / f"mangrove_{tag}.png", array=np.ma.masked_where(anth <= 0, o), cmap="PuBu",
                vmin=0, vmax=0.7, legend=False, width=W)
aoh2 = C.table(alt, "aoh")
cmp = [{"species": C.NAMES[s], "with": round(a["potential"], 1), "without": round(b["potential"], 1)}
       for s, a, b in zip(C.SPECIES, aoh, aoh2)]
figs.write_csv(data / "ecoregions.csv", cmp)
print("ecoregions vs flat", cmp)

# ------------------------------------------------------------------ elevation limits
el = figs.workdir(f"{SLUG}/elevation")
C.serra(el, "land_cover", "past_cover", "features", "ecoregions", "elevation")
m = model().replace('inputs = { ranges = "ranges.raster",',
                    'inputs = { elevation = "elevation.map", ranges = "ranges.raster",')
m = m.replace('[[nodes]]\nid = "habitat"', '[[nodes]]\nid = "elevation"\nop = "raster.read"\n'
              'params = { path = "elevation.tif" }\n\n[[nodes]]\nid = "habitat"')
m = m.replace('suitability = {', 'elevation_limits = [["araucaria_forest", 0.8, 1.0]], suitability = {', 1)
(el / "model_elevation.toml").write_text(m, encoding="utf-8")
figs.run(figs.add_writes(m, {"habitat.table": "aoh.csv"}), el)
aoh3 = C.table(el, "aoh")
for s, a, b in zip(C.SPECIES, aoh, aoh3):
    if a["range_cells"] != b["range_cells"]:
        print("elevation", s, a["range_cells"], "->", b["range_cells"], round(a["current"], 1), "->", round(b["current"], 1),
              round(a["potential"], 1), "->", round(b["potential"], 1))
figs.raster(el / "elevation.tif", out / "elevation.png", cmap="terrain", vmin=0, vmax=1.1, unit="relative elevation",
            width=W)
print("done")
