"""Figures of 'Intensity analysis' (change-detection/intensity-analysis).

change.intensity on Plum Island's three dates (1985, 1991, 1999: two periods) and on Sibuyan's two (1997, 2011),
drawn as Aldwaik & Pontius's charts: bars of the observed intensities against the uniform intensity (a dashed
line), at the interval, category and transition levels; and the transition level of Sibuyan for the gains of
coconut, grassland and rice in turn (a slider).

    python website/figures/change-detection/intensity-analysis.py
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _common as c  # noqa: E402
from _common import figs, np, plt  # noqa: E402

SLUG = "intensity-analysis"
work, img, data = c.setup(SLUG, files=("sibuyan_landuse.tif", "pie_landuse.tif"))

PIE_MODEL = """format = "terraflux/1"

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "pie_landuse.tif" }

[[nodes]]
id = "transitions"
op = "change.transitions"
inputs = { maps = "dates.raster" }

[[nodes]]
id = "intensity"
op = "change.intensity"
inputs = { maps = "dates.raster" }
params = { gain = 2, loss = 1 }
"""

SIB_MODEL = """format = "terraflux/1"

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "sibuyan_landuse.tif" }

[[nodes]]
id = "intensity"
op = "change.intensity"
inputs = { maps = "dates.raster" }
params = { gain = GAIN, loss = 1 }
"""

PORTS = ["interval", "category_gain", "category_loss", "transition_gain", "transition_loss",
         "category_gain_stationarity", "category_loss_stationarity", "transition_gain_stationarity",
         "transition_loss_stationarity"]


def run(model, w, extra=None):
    writes = {f"intensity.{p}": f"{p}.csv" for p in PORTS}
    writes.update(extra or {})
    figs.run(figs.add_writes(model, writes), w)
    return {p: c.table(w / "output" / f"{p}.csv") for p in PORTS}


pie = run(PIE_MODEL, work, {"transitions.areas": "areas.csv", "dates.raster": "pie.tif"})
(work / "page_model.toml").write_text(PIE_MODEL, encoding="utf-8")
for p in PORTS:
    print("PIE", p)
    for r in pie[p]:
        print("   ", {k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items()})
print("PIE areas", c.table(work / "output" / "areas.csv"))

# ------------------------------------------------------------------------------------------------ PIE maps
fig, axes = plt.subplots(1, 3, figsize=(10, 3.1), dpi=150)
for b, (ax, t) in enumerate(zip(axes, ("1985", "1991", "1999")), start=1):
    a = figs.read(work / "pie.tif", b)[0]
    c.classes_map(a, c.PIE, ax)
    ax.set_title(t, loc="left")
fig.legend(handles=c.legend_handles(c.PIE), loc="lower center", ncol=3, fontsize=9.5, bbox_to_anchor=(0.5, -0.02))
fig.subplots_adjust(bottom=0.12, wspace=0.03)
c.save(fig, img / "pie_dates.png")

PNAME = {k: v[0] for k, v in c.PIE.items()}
SNAME = {k: v[0] for k, v in c.SIB.items()}
DASH = dict(color=figs.INK, lw=1.4, ls=(0, (4, 3)))


def period(r):
    return f"{int(r['year_from'])}–{int(r['year_to'])}"


# ------------------------------------------------------------------------------------------------ interval level
iv = pie["interval"]


def draw_interval(ax):
    labels = [period(r) for r in iv]
    y = np.arange(len(iv))
    ax.barh(y, [r["St"] for r in iv], color=[figs.EMBER if r["St"] > r["U"] else figs.WATER for r in iv], height=0.55)
    ax.axvline(iv[0]["U"], **DASH)
    ax.text(iv[0]["U"] + 0.006, -0.62, f"uniform U = {iv[0]['U']:.3f}", fontsize=9.5, color=figs.INK, va="center")
    for k, r in enumerate(iv):
        ax.text(max(r["St"], iv[0]["U"]) + 0.008, k, f"{r['St']:.3f}  {'fast' if r['St'] > r['U'] else 'slow'}",
                va="center", fontsize=9.5, color=figs.INK2)
    ax.set_yticks(y, labels)
    ax.invert_yaxis()
    ax.set_xlabel("Annual change, % of the area")
    ax.set_xlim(0, 0.75)
    ax.grid(axis="y", visible=False)
    ax.set_ylim(len(iv) - 0.4, -0.95)


figs.chart(img / "pie_interval.png", draw_interval, width=900, height=300,
           title="Interval level: how fast each period changed")
figs.write_csv(data / "pie_interval.csv", [{"period": period(r), "observed": round(r["St"], 4),
                                            "uniform": round(r["U"], 4)} for r in iv])


# ------------------------------------------------------------------------------------------------ category level
def category_axes(rows_gain, rows_loss, names, palette, title):
    periods = sorted({period(r) for r in rows_gain + rows_loss})
    fig, axes = plt.subplots(len(periods), 2, figsize=(9.6, 1.0 + 2.0 * len(periods)), dpi=150, squeeze=False)
    for i, p in enumerate(periods):
        for j, (rows, key, word) in enumerate(((rows_gain, "To", "Gain"), (rows_loss, "From", "Loss"))):
            ax = axes[i][j]
            rr = [r for r in rows if period(r) == p]
            rate = "Gtj" if word == "Gain" else "Lti"
            cl = sorted(names)
            vals = {int(r[key]): r[rate] for r in rr}
            st = rr[0]["St"] if rr else 0
            y = np.arange(len(cl))
            ax_max = max([vals.get(k, 0) for k in cl] + [st])
            ax.barh(y, [vals.get(k, 0) for k in cl], color=[palette[k][1] for k in cl], height=0.6)
            ax.axvline(st, **DASH)
            for k, cc in enumerate(cl):
                v = vals.get(cc, 0)
                tag = "active" if v > st else "dormant"
                ax.text(max(v, st) + 0.03 * ax_max, k, f"{v:.2f}  {tag}", va="center", fontsize=8.5,
                        color=figs.INK if v > st else figs.INK2)
            ax.set_yticks(y, [names[k] for k in cl])
            ax.invert_yaxis()
            ax.grid(axis="y", visible=False)
            ax.set_title(f"{'Gains' if word == 'Gain' else 'Losses'}, {p}", loc="left", fontsize=11)
            ax.set_xlim(0, max([vals.get(k, 0) for k in cl] + [st]) * 1.45 + 0.05)
            if i == len(periods) - 1:
                ax.set_xlabel("Intensity, % of the class a year")
    fig.suptitle(title, x=0.01, ha="left", fontsize=12, fontweight="bold", color=figs.INK)
    fig.tight_layout()
    return fig


fig = category_axes(pie["category_gain"], pie["category_loss"], PNAME, c.PIE,
                    "Category level: each class's gains and losses against the period's rate (dashed)")
c.save(fig, img / "pie_category.png")


# ------------------------------------------------------------------------------------------------ transition level
def transition_axes(rows_g, rows_l, names, palette, gain, loss, title):
    periods = sorted({period(r) for r in rows_g + rows_l})
    fig, axes = plt.subplots(len(periods), 2, figsize=(9.6, 1.0 + 1.9 * len(periods)), dpi=150, squeeze=False)
    for i, p in enumerate(periods):
        for j, (rows, key, rate, uni, word) in enumerate(((rows_g, "From", "Rtin", "Wtn", f"{names[gain]} gained from"),
                                                          (rows_l, "To", "Qtmj", "Vtm", f"{names[loss]} lost to"))):
            ax = axes[i][j]
            rr = [r for r in rows if period(r) == p]
            cl = [int(r[key]) for r in rr]
            y = np.arange(len(cl))
            ax.barh(y, [r[rate] for r in rr], color=[palette[k][1] for k in cl], height=0.6)
            u = rr[0][uni] if rr else 0
            ax.axvline(u, **DASH)
            for k, r in enumerate(rr):
                v = r[rate]
                ax.text(max(v, u) + 0.03 * max([x[rate] for x in rr] + [u]), k,
                        f"{v:.2f}  {'targeted' if v > u else 'avoided'}", va="center", fontsize=8.5,
                        color=figs.INK if v > u else figs.INK2)
            ax.set_yticks(y, [names[k] for k in cl])
            ax.invert_yaxis()
            ax.grid(axis="y", visible=False)
            ax.set_title(f"{word}, {p}", loc="left", fontsize=11)
            ax.set_xlim(0, max([r[rate] for r in rr] + [u]) * 1.5 + 0.05)
            if i == len(periods) - 1:
                ax.set_xlabel("Intensity, % of the other class a year")
    fig.suptitle(title, x=0.01, ha="left", fontsize=12, fontweight="bold", color=figs.INK)
    fig.tight_layout()
    return fig


fig = transition_axes(pie["transition_gain"], pie["transition_loss"], PNAME, c.PIE, 2, 1,
                      "Transition level: which classes built land took, and which took the forest")
c.save(fig, img / "pie_transition.png")

# tfchart data: category level of Plum Island, one row per period and class
rows = []
for r in pie["category_gain"]:
    l = next(x for x in pie["category_loss"] if period(x) == period(r) and x["From"] == r["To"])
    rows.append({"class": f"{PNAME[int(r['To'])]} {period(r)}", "gain": round(r["Gtj"], 4), "loss": round(l["Lti"], 4),
                 "uniform": round(r["St"], 4)})
figs.write_csv(data / "pie_category.csv", rows)
rows = []
for r in pie["transition_gain"]:
    rows.append({"from": f"{PNAME[int(r['From'])]} {period(r)}", "intensity": round(r["Rtin"], 4),
                 "uniform": round(r["Wtn"], 4)})
figs.write_csv(data / "pie_transition_gain.csv", rows)

# ------------------------------------------------------------------------------------------------ Sibuyan
sib = {}
for gain in (2, 3, 4):
    w = figs.workdir(f"{c.CATEGORY}/{SLUG}-sib{gain}")
    import shutil  # noqa: E402

    shutil.copy2(c.FILES["sibuyan_landuse.tif"], w / "sibuyan_landuse.tif")
    sib[gain] = run(SIB_MODEL.replace("GAIN", str(gain)), w)
s3 = sib[3]
for p in ("interval", "category_gain", "category_loss", "transition_gain", "transition_loss"):
    print("SIB", p)
    for r in s3[p]:
        print("   ", {k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items()})

fig = category_axes(s3["category_gain"], s3["category_loss"], SNAME, c.SIB,
                    "Sibuyan, category level: gains and losses against the period's rate (dashed)")
c.save(fig, img / "sib_category.png")
rows = []
for k in sorted(SNAME):
    g = next((x["Gtj"] for x in s3["category_gain"] if int(x["To"]) == k), 0.0)
    l = next((x["Lti"] for x in s3["category_loss"] if int(x["From"]) == k), 0.0)
    rows.append({"class": SNAME[k], "gain": round(g, 4), "loss": round(l, 4), "uniform": round(s3["interval"][0]["St"], 4)})
figs.write_csv(data / "sib_category.csv", rows)

frames = []
for gain in (2, 3, 4):
    t = sib[gain]
    rr = t["transition_gain"]
    u = rr[0]["Wtn"]

    def draw(ax, rr=rr, u=u, gain=gain):
        cl = [int(r["From"]) for r in rr]
        everyone = [k for k in sorted(SNAME) if k != gain]
        vals = {int(r["From"]): r["Rtin"] for r in rr}
        y = np.arange(len(everyone))
        ax.barh(y, [vals.get(k, 0) for k in everyone], color=[c.SIB[k][1] for k in everyone], height=0.6)
        ax.axvline(u, **DASH)
        ax.text(u + 0.03, len(everyone) - 0.2, f"uniform W = {u:.3f}", fontsize=9.5, va="center")
        for k, cc in enumerate(everyone):
            v = vals.get(cc, 0)
            ax.text(max(v, u) + 0.04, k, f"{v:.3f}  {'targeted' if v > u else 'avoided'}", va="center", fontsize=9,
                    color=figs.INK if v > u else figs.INK2)
        ax.set_yticks(y, [SNAME[k] for k in everyone])
        ax.invert_yaxis()
        ax.grid(axis="y", visible=False)
        ax.set_xlim(0, 2.6)
        ax.set_ylim(len(everyone) + 0.15, -0.6)
        ax.set_xlabel("Intensity of the gain from each class, % of that class a year")

    figs.chart(img / f"sib_gain_{gain}.png", draw, width=900, height=330,
               title=f"Sibuyan 1997–2011: the gains of {SNAME[gain].lower()}, from each class")
    targeted = [SNAME[int(r["From"])] for r in rr if r["Rtin"] > u]
    frames.append((gain, targeted, u))
    print("SIB gain", gain, "targets", targeted, "W", u, [(SNAME[int(r['From'])], round(r['Rtin'], 3)) for r in rr])

fig = transition_axes(s3["transition_gain"], s3["transition_loss"], SNAME, c.SIB, 3, 1,
                      "Sibuyan, transition level: where grassland came from, where forest went")
c.save(fig, img / "sib_transition.png")
print("images:", sum(p.stat().st_size for p in img.glob("*.png")) / 1e6, "MB")
