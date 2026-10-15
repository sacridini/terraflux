"""Figures of "Species distribution models: the ideas" (tutorials/species-distributions/ideas).

    python website/figures/species-distributions/ideas.py

The BAM diagram (drawn); the sloth's records over the climate; the records and the continent in environmental
space; a virtual species (the template "Species distribution model"): its true suitability and MaxNet's estimate,
for several numbers of records; and a first MaxNet model of the sloth.
"""

from __future__ import annotations

import numpy as np
import matplotlib.pyplot as plt
from matplotlib.patches import Circle

import sdmfig as S
from sdmfig import figs

SLUG = "ideas"
IMG = figs.img_dir(f"{S.CAT}/{SLUG}")
DAT = figs.data_dir(f"{S.CAT}/{SLUG}")


def bam():
    fig, ax = plt.subplots(figsize=(6.4, 4.4), dpi=150)
    ax.set_axis_off()
    ax.set_aspect("equal")
    sets = [("A", (-0.55, 0.25), "#3b8a5a", "Abiotic: the climate\nand soils it tolerates"),
            ("B", (0.55, 0.25), "#2f6f9f", "Biotic: food, hosts,\nno superior competitor"),
            ("M", (0.0, -0.62), "#e8901f", "Movement: the area it\nhas been able to reach")]
    for name, c, col, txt in sets:
        ax.add_patch(Circle(c, 1.0, facecolor=col, alpha=0.18, edgecolor=col, linewidth=1.6))
        dx, dy = (c[0] * 1.55, c[1] + 0.55) if name != "M" else (0, c[1] - 0.62)
        ax.text(c[0] * 1.05 + (0 if name == "M" else (-0.25 if name == "A" else 0.25)), c[1] + (0.45 if name != "M" else -0.5),
                name, ha="center", va="center", fontsize=20, fontweight="bold", color=col)
    ax.text(-1.95, 1.05, sets[0][3], ha="left", va="center", fontsize=9, color=figs.INK2)
    ax.text(1.95, 1.05, sets[1][3], ha="right", va="center", fontsize=9, color=figs.INK2)
    ax.text(0, -1.85, sets[2][3], ha="center", va="center", fontsize=9, color=figs.INK2)
    ax.text(0.0, -0.02, "G$_O$", ha="center", va="center", fontsize=13, fontweight="bold", color=figs.INK)
    ax.text(0.0, -0.22, "occupied", ha="center", va="center", fontsize=8.5, color=figs.INK)
    ax.text(0.0, 0.62, "G$_I$", ha="center", va="center", fontsize=12, fontweight="bold", color=figs.INK2)
    ax.text(0.0, 0.43, "invadable", ha="center", va="center", fontsize=8.5, color=figs.INK2)
    ax.set_xlim(-2.0, 2.0)
    ax.set_ylim(-2.05, 1.35)
    fig.tight_layout()
    S.save(fig, IMG / "bam.png")


def sloth():
    """The records and the climate: a map and the environmental space."""
    w = S.work(SLUG)
    m = S.HEAD + S.READ
    m += S.node("background", "sdm.background", {"env": "climate.stack", "occurrences": "records.vector"},
                {"n": 10000, "seed": 1})
    m += S.node("table", "sdm.dataset", {"presences": "records.vector", "background": "background.points",
                                         "env": "climate.stack"})
    m += S.outputs({"table": "table.records"})
    S.run(m, w)
    t = S.table(w, "table.csv")
    p = t["presence"] == 1
    a, info = figs.read(S.DATA / "bioclim.tif", 1)
    fig, ax = S.map_axes(800, legend_frac=0.22)
    S.base(ax)
    im = S.surface(ax, a / 10, info, cmap="RdYlBu_r")
    S.points(ax, t["x"][p], t["y"][p], size=14)
    S.colorbar(fig, im, "Annual mean temperature (°C)", rect=(0.80, 0.30, 0.03, 0.5))
    S.legend(fig, [("Records of the sloth", S.PRES, "o")], loc=(0.76, 0.86))
    S.save(fig, IMG / "records.png")

    def draw(ax):
        ax.scatter(t["bio1"][~p] / 10, t["bio12"][~p], s=3, c="#b9c2bc", label="Cells of the continent", lw=0)
        ax.scatter(t["bio1"][p] / 10, t["bio12"][p], s=16, c=S.PRES, edgecolors="white", linewidths=0.4,
                   label="Records of the sloth")
        ax.set_xlabel("Annual mean temperature (°C)")
        ax.set_ylabel("Annual precipitation (mm)")
        ax.legend(loc="upper left", fontsize=9)
    figs.chart(IMG / "envspace.png", draw, width=800, height=520)
    return int(p.sum()), int((~p).sum())


def virtual():
    """The template's virtual species: the truth, and MaxNet with more and more records."""
    tpl = figs.template("species-distribution-model")
    out = {}
    for n in (20, 40, 80, 160, 320):
        w = figs.workdir(f"{S.CAT}/{SLUG}/virtual_{n}")
        m = tpl.replace("params = { n = 120, seed = 3 }", f"params = {{ n = {n}, seed = 3 }}")
        assert m != tpl
        m = figs.add_writes(m, {"suitability.map": "truth.tif", "predict.prediction": "pred.tif"})
        m += S.outputs({"summary": "fit.summary", "records": "data.records", "report": "data.report"})
        S.run(m, w)
        truth, info = figs.read(w / "truth.tif")
        pred, _ = figs.read(w / "pred.tif")
        rec = S.table(w, "records.csv")
        s = S.out(w, "summary.csv")[0]
        r = spearman(truth.compressed(), pred.compressed())
        out[n] = dict(r=r, auc=float(s["auc_test_mean"]), boyce=float(s["boyce_mean"]),
                      n=int((rec["presence"] == 1).sum()))
        draw_virtual(pred, info, rec, IMG / f"virtual_{n}.png", f"MaxNet from {out[n]['n']} records")
        if n == 160:
            draw_virtual(truth, info, None, IMG / "virtual_truth.png", None)
            draw_virtual(pred, info, None, IMG / "virtual_model.png", None)
    rows = [{"records": v["n"], "correlation": round(v["r"], 3), "auc": round(v["auc"], 3),
             "boyce": round(v["boyce"], 3)} for v in out.values()]
    figs.write_csv(DAT / "virtual.csv", rows)
    return out


def spearman(a, b):
    ra = np.argsort(np.argsort(a))
    rb = np.argsort(np.argsort(b))
    return float(np.corrcoef(ra, rb)[0, 1])


def draw_virtual(a, info, rec, png, title):
    fig = plt.figure(figsize=(5.6, 3.95), dpi=150)
    ax = fig.add_axes([0, 0, 0.84, 0.91])
    ax.set_axis_off()
    cm = plt.get_cmap("viridis").copy()
    ext = S.extent(info)
    im = ax.imshow(a, cmap=cm, vmin=0, vmax=1, interpolation="nearest", extent=ext)
    if rec is not None:
        p = rec["presence"] == 1
        ax.scatter(rec["x"][p], rec["y"][p], s=9, c=S.PRES, edgecolors="white", linewidths=0.4)
    ax.set_xlim(ext[0], ext[1])
    ax.set_ylim(ext[2], ext[3])
    cax = fig.add_axes([0.87, 0.12, 0.03, 0.7])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.ax.tick_params(labelsize=8.5)
    cb.ax.grid(False)
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=11, fontweight="bold", color=figs.INK)
    S.save(fig, png)


def first_model():
    w = S.work(SLUG + "_first")
    m = S.HEAD + S.READ
    m += S.node("data", "sdm.prepare", {"occurrences": "records.vector", "env": "climate.stack"}, {"background": 5000})
    m += S.node("fit", "sdm.fit", {"records": "data.records"})
    m += S.node("predict", "sdm.maxnet_predict", {"model": "fit.model", "env": "data.env"})
    m = figs.add_writes(m, {"predict.prediction": "suitability.tif"})
    m += S.outputs({"summary": "fit.summary", "records": "data.records", "report": "data.report",
                    "settings": "data.settings"})
    S.run(m, w)
    a, info = figs.read(w / "suitability.tif")
    rec = S.table(w, "records.csv")
    p = rec["presence"] == 1
    fig, ax = S.map_axes(800, legend_frac=0.22)
    S.base(ax)
    im = S.surface(ax, a, info, vmin=0, vmax=1)
    S.points(ax, rec["x"][p], rec["y"][p], size=10)
    S.colorbar(fig, im, "Suitability (cloglog)", rect=(0.80, 0.30, 0.03, 0.5))
    S.legend(fig, [("Records used", S.PRES, "o")], loc=(0.76, 0.86))
    S.save(fig, IMG / "first_model.png")
    return S.out(w, "summary.csv")[0], S.out(w, "report.json"), S.out(w, "settings.json")


if __name__ == "__main__":
    S.publish_data()
    bam()
    print("sloth", sloth())
    v = virtual()
    for n, x in v.items():
        print(n, {k: round(y, 3) if isinstance(y, float) else y for k, y in x.items()})
    s, rep, st = first_model()
    print({k: round(float(x), 3) for k, x in s.items()})
    for r in rep:
        print(r)
    print(st)
