"""Figures of "Change of fragmentation and GuidosToolbox's reports" (fragmentation/change-and-reports).

GuidosToolbox's sample forest and a second date made by clearing it (the rule of habitat-patterns/networks-and-change):
the change of FAD at five scales (change.fad), the change of FOS (change.fos); CORINE around Hannover and a second
date where the city grows: the change of the landscape mosaic (change.mosaic).

    python website/figures/fragmentation/change-and-reports.py
"""

import json


def text_output(o, name):
    """A text output of a run: name.txt (Terraflux 0.80), or the JSON string of older versions."""
    t = o / f"{name}.txt"
    if t.exists():
        return t.read_text(encoding="utf-8")
    return json.loads((o / f"{name}.json").read_text(encoding="utf-8"))
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-patterns"))
import gtbfig as g  # noqa: E402
from gtbfig import figs, np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402

NAME = "fragmentation/change-and-reports"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)
for src in (g.GTB_INPUT, g.CLC):
    g.copy_data(src, work)
o = work / "output"
summary = {}
R = (slice(650, 950), slice(250, 550))

CLEAR = ("if f == 2 and p > 0.8 then 1 "
         "else if f == 2 and p > 0.55 and nbsum(f == 1, 3) >= 1 then 1 "
         "else if f == 1 and p < 0.15 and nbsum(f == 2, 1) >= 4 then 2 else f")
# the city grows into the farmland (and a little into the forest) beside it, where a smooth field is high
GROW = ("if c >= 12 and c <= 22 and p > 0.5 and nbsum(c <= 11, 4) >= 8 then 2 "
        "else if c >= 23 and c <= 25 and p > 0.75 and nbsum(c <= 11, 4) >= 20 then 2 else c")
nodes = [
    g.node("before", "raster.read", path="input.tif"),
    g.node("pressure", "nlm.gaussian_field", rows=1000, columns=1000, range=60, seed=28),
    g.node("after", "raster.calc", {"f": "before.map", "p": "pressure.map"}, expression=CLEAR),
    g.node("fadchange", "change.fad", {"before": "before.map", "after": "after.map"}),
    g.node("ms_a", "fragmentation.fad_multiscale", {"map": "before.map"}),
    g.node("ms_b", "fragmentation.fad_multiscale", {"map": "after.map"}),
    g.node("fos_a", "fragmentation.fos", {"map": "before.map"}, window=27, method="FAC"),
    g.node("fos_b", "fragmentation.fos", {"map": "after.map"}, window=27, method="FAC"),
    g.node("foschange", "change.fos", {"before": "fos_a.map", "after": "fos_b.map"}),
    g.node("corine", "raster.read", path="clc.tif"),
    g.node("urb", "nlm.gaussian_field", rows=2000, columns=2000, range=80, seed=5),
    g.node("corine_b", "raster.calc", {"c": "corine.map", "p": "urb.map"}, expression=GROW),
    g.node("mosaicchange", "change.mosaic", {"before": "corine.map", "after": "corine_b.map"}, window=27,
           agriculture=list(range(12, 23)), natural=list(range(23, 40)), developed=list(range(1, 12))),
    g.node("lm_a", "fragmentation.landscape_mosaic", {"map": "corine.map"}, window=27,
           agriculture=list(range(12, 23)), natural=list(range(23, 40)), developed=list(range(1, 12))),
    g.node("lm_b", "fragmentation.landscape_mosaic", {"map": "corine_b.map"}, window=27,
           agriculture=list(range(12, 23)), natural=list(range(23, 40)), developed=list(range(1, 12))),
]
writes = {"after.map": "after.tif", "ms_a.classes": "ms_a.tif", "ms_b.classes": "ms_b.tif", "fos_a.map": "fos_a.tif",
          "fos_b.map": "fos_b.tif", "foschange.map": "foschange.tif", "corine_b.map": "corine_b.tif",
          "mosaicchange.triangle": "mosaic_delta.tif", "lm_a.map": "lm_a.tif", "lm_b.map": "lm_b.tif"}
outputs = {"fadchange.stats": "fad_stats", "fadchange.matrix": "fad_matrix", "fadchange.report": "fad_report",
           "foschange.stats": "fos_stats", "foschange.classes": "fos_classes", "mosaicchange.heatmap": "lm_heatmap",
           "ms_a.stats": "ms_a_stats", "ms_b.stats": "ms_b_stats"}
model = figs.add_writes(g.toml_nodes(*nodes), writes)
for ref, nm in outputs.items():
    model += f'\n[outputs.{nm}]\nfrom = "{ref}"\n'
figs.run(model, work)

# ------------------------------------------------------------------------------------------------ FAD change
st = figs.read_csv(o / "fad_stats.csv")
mx = figs.read_csv(o / "fad_matrix.csv")
summary["fad_stats"] = st
summary["fad_matrix_head"] = mx[:3]
print("fad stats cols", list(st[0].keys()))
for r in st:
    print(r)
print("matrix cols", list(mx[0].keys()))
(data / "fad-change-report.txt").write_text(text_output(o, "fad_report"),
                                            encoding="utf-8")
# the % of the common forest whose fragmentation increased / decreased, per scale
rows = []
for r in st:
    wdw = r.get("window", r.get("scale", ""))
    inc = [v for k, v in r.items() if "increase" in k]
    dec = [v for k, v in r.items() if "decrease" in k]
    rows.append({"window": "average" if str(wdw) == "0" else f"{wdw} cells",
                 "increased": round(float(inc[0]), 2) if inc else "",
                 "decreased": round(float(dec[0]), 2) if dec else ""})
figs.write_csv(data / "fad-change.csv", rows)
# the matrix of the average (window 0): forest of each class at A, to each class at B, as % of the row
NAMES = {0: "background", 1: "rare", 2: "patchy", 3: "transitional", 4: "dominant", 5: "interior", 6: "intact"}
avg = [r for r in mx if str(r.get("window", r.get("scale", ""))) in ("27",)]
summary["matrix_27"] = avg
ka = g.read(work / "ms_a.tif")
kb = g.read(work / "ms_b.tif")
g.classmap(g.density_classes(np.where(ka <= 6, 0, ka).astype(float)) if False else ka[R], out / "ms-a-zoom.png",
           {**g.FAD_CLASS, 5: ("Interior (90-100%)", "#00af00")}, legend=False, width=640)
g.classmap(kb[R], out / "ms-b-zoom.png", {**g.FAD_CLASS, 5: ("Interior (90-100%)", "#00af00")}, legend=False, width=640)
print("ms classes codes", np.unique(ka))

# a heatmap of the matrix at 27 cells: rows class at A, columns class at B, % of the row
if avg:
    keys = [k for k in avg[0] if k not in ("window", "scale", "class", "class_a", "from")]
    print("matrix keys", keys)
    M = np.array([[float(r[k]) for k in keys] for r in avg])
    rown = [NAMES.get(int(float(r.get("class", r.get("class_a", r.get("from", 0))))), "?") for r in avg]
    P = M / np.maximum(M.sum(1, keepdims=True), 1) * 100
    fig, ax = plt.subplots(figsize=(7.4, 5.2), dpi=150)
    im = ax.imshow(P, cmap="Greens", vmin=0, vmax=100)
    ax.set_xticks(range(len(keys)), [NAMES.get(int(k.split("_")[-1]), k) if k.split("_")[-1].isdigit() else k
                                     for k in keys], rotation=30, ha="right")
    ax.set_yticks(range(len(rown)), rown)
    ax.set_xlabel("class at date B")
    ax.set_ylabel("class at date A")
    ax.grid(False)
    for i in range(P.shape[0]):
        for j in range(P.shape[1]):
            if M[i, j] > 0:
                ax.text(j, i, f"{P[i, j]:.0f}", ha="center", va="center", fontsize=8.5,
                        color="white" if P[i, j] > 55 else figs.INK)
    cb = fig.colorbar(im, ax=ax, fraction=0.04, pad=0.02)
    cb.outline.set_visible(False)
    cb.set_label("% of the class at A", color=figs.INK2)
    fig.tight_layout()
    fig.savefig(out / "fad-matrix-27.png", dpi=150)
    plt.close(fig)
    figs._shrink(out / "fad-matrix-27.png")
    summary["matrix_27_pct"] = {rown[i]: dict(zip(keys, np.round(P[i], 1).tolist())) for i in range(len(rown))}

# ------------------------------------------------------------------------------------------------ FOS change
fc = g.read(work / "foschange.tif").astype(float)
print("fos change codes", np.unique(fc)[:5], np.unique(fc)[-6:])
summary["fos_stats"] = figs.read_csv(o / "fos_stats.csv")
summary["fos_classes"] = figs.read_csv(o / "fos_classes.csv")
# the map holds A - B + 100 (GTB): above 100 the connectivity fell. Drawn as B - A, losses in red
delta = np.where(fc <= 200, 100 - fc, np.nan)
rgb = np.ones(fc.shape + (3,))
norm = mcolors.TwoSlopeNorm(vmin=-60, vcenter=0, vmax=60)
cmap = plt.get_cmap("RdBu")
ok = ~np.isnan(delta)
rgb[ok] = cmap(norm(np.clip(delta[ok], -60, 60)))[:, :3]
rgb[fc == 250] = mcolors.to_rgb("#00b050")
rgb[fc == 251] = mcolors.to_rgb("#000000")
rgb[fc == 252] = mcolors.to_rgb("#e6e6e6")
rgb[fc == 254] = (1, 1, 1)
fig = plt.figure(figsize=(7.0, 5.2), dpi=150)
ax = fig.add_axes([0.0, 0.0, 0.78, 1.0])
ax.imshow(rgb[R], interpolation="nearest")
ax.set_axis_off()
cax = fig.add_axes([0.81, 0.30, 0.03, 0.62])
cb = fig.colorbar(plt.cm.ScalarMappable(norm=norm, cmap=cmap), cax=cax)
cb.outline.set_visible(False)
cb.set_label("change of FAC, B - A (points)", color=figs.INK2, fontsize=9)
from matplotlib.patches import Patch  # noqa: E402
fig.legend(handles=[Patch(facecolor="#000000", label="Forest lost"), Patch(facecolor="#00b050", label="Forest gained"),
                    Patch(facecolor="#e6e6e6", label="Stable background")], loc="lower left",
           bbox_to_anchor=(0.79, 0.02), fontsize=8.5, frameon=False)
fig.savefig(out / "fos-change-zoom.png", dpi=150)
plt.close(fig)
figs._shrink(out / "fos-change-zoom.png")

# ------------------------------------------------------------------------------------------------ the mosaic's change
hm = figs.read_csv(o / "lm_heatmap.csv")
print("heatmap cols", list(hm[0].keys()))
summary["heatmap_head"] = hm[:4]
tri = g.read(work / "mosaic_delta.tif").astype(float)
print("delta triangle", np.nanmin(tri), np.nanmax(tri))
fig, ax = plt.subplots(figsize=(7.2, 4.6), dpi=150)
t = np.ma.masked_invalid(tri)
m = float(np.nanmax(np.abs(tri))) or 1
im = ax.imshow(t, cmap="RdBu_r", norm=mcolors.TwoSlopeNorm(vmin=-m, vcenter=0, vmax=m), interpolation="nearest")
ax.set_xticks([])
ax.set_yticks([])
ax.grid(False)
for s in ax.spines.values():
    s.set_visible(False)
for (cx, cy, txt, ha) in ((10, -1.0, "pure natural", "center"), (0, 11.9, "pure developed", "left"),
                          (20, 11.9, "pure agriculture", "right")):
    ax.text(cx, cy, txt, ha=ha, va="center", fontsize=9.5, color=figs.INK)
ax.set_ylim(12.5, -1.7)
cb = fig.colorbar(im, ax=ax, fraction=0.04, pad=0.02)
cb.outline.set_visible(False)
cb.set_label("change of the share (points), B - A", color=figs.INK2)
fig.tight_layout()
fig.savefig(out / "mosaic-delta.png", dpi=150)
plt.close(fig)
figs._shrink(out / "mosaic-delta.png")
# the land cover's change and the mosaic at both dates (a zoom on the city)
ca = g.read(work / "clc.tif")
cb_ = g.read(work / "corine_b.tif")
grown = (cb_ != ca)
summary["grown_cells"] = int(grown.sum())
summary["urban_a"] = round(float(((ca >= 1) & (ca <= 11)).mean() * 100), 2)
summary["urban_b"] = round(float(((cb_ >= 1) & (cb_ <= 11)).mean() * 100), 2)
Z = (slice(500, 1500), slice(1000, 2000))
LMC = figs.gtb_colors("LM")
pal = {k: (str(k), LMC[k]) for k in range(1, 20)}
pal[0] = ("Missing", "#ffffff")
g.classmap(g.read(work / "lm_a.tif")[Z], out / "lm-a-zoom.png", pal, legend=False, width=640)
g.classmap(g.read(work / "lm_b.tif")[Z], out / "lm-b-zoom.png", pal, legend=False, width=640)
la, lb = g.read(work / "lm_a.tif"), g.read(work / "lm_b.tif")
tot = (la > 0).sum()
summary["lm_shares"] = {k: [round(float((la == k).sum()) / tot * 100, 2), round(float((lb == k).sum()) / tot * 100, 2)]
                        for k in range(1, 20)}
(work / "summary.json").write_text(json.dumps(summary, indent=1, default=str))
print(json.dumps(summary, indent=1, default=str)[:8000])
