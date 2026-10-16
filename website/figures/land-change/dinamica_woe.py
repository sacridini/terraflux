"""Figures of "Weights of evidence" (land-change/weights-of-evidence): the weights of evidence of Sibuyan's transitions
fitted by Terraflux (woe.fit, Dinamica EGO's method), their bars for a few drivers, the ranges cut with other
increments, the probability maps (woe.probability), and the association between drivers (Cramer's V and the
joint information uncertainty, by validate.kappa on the drivers cut into classes)."""

import itertools
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np, plt  # noqa: E402

SLUG = "land-change/weights-of-evidence"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = dc.sibuyan("woe")

HEAD = """format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "drivers"
op = "raster.read"
params = { path = "sibuyan_drivers.tif" }

[[nodes]]
id = "chain"
op = "markov.fit"
inputs = { t0 = "land_1997.raster", t1 = "land_2011.raster", steps = 14 }
"""


def fit_model(ranges, categorical=dc.CATEGORICAL, extra=""):
    return HEAD + f"""
[[nodes]]
id = "weights"
op = "woe.fit"
inputs = {{ t0 = "land_1997.raster", t1 = "land_2011.raster", drivers = "drivers.raster", markov = "chain.markov" }}
params = {{ ranges = {dc.toml_value(ranges)}, categorical = {dc.toml_value(categorical)} }}

[[nodes]]
id = "probability"
op = "woe.probability"
inputs = {{ model = "weights.model", landscape = "land_1997.raster", drivers = "drivers.raster" }}

[[nodes]]
id = "write"
op = "raster.write"
inputs = {{ value = "probability.probability" }}
params = {{ path = "probability.tif" }}

[outputs.weights]
from = "weights.table"
{extra}"""


model = fit_model(dc.RANGES)
print(model)
res = dc.run(model, work)
W = res / "weights.csv"
rows = dc.table(W)
print("rows of the report:", len(rows))

UNITS = {"dist_roads": "m", "dist_sea": "m", "dist_urban": "m", "dist_streams": "m", "elevation": "m",
         "slope": "degrees", "pop_density": "people per km²", "aspect": "degrees"}
LABEL = {"dist_roads": "Distance to roads", "dist_sea": "Distance to the sea", "dist_urban": "Distance to towns",
         "dist_streams": "Distance to streams", "elevation": "Elevation", "slope": "Slope",
         "pop_density": "Population density", "aspect": "Aspect", "ultramafic": "Ultramafic soils"}

# the report of one transition and variable, for the page
print("1->3 dist_roads:")
for r in dc.weights_rows(W, 1, 3, "dist_roads"):
    print(f"  {r['lower']:.0f}-{r['upper']:.0f}  possible {r['possible']:.0f} executed {r['executed']:.0f}  "
          f"W+ {r['weight']:.3f} C {r['contrast']:.3f} sig {r['significant']:.0f}")
print("1->2 dist_roads:")
for r in dc.weights_rows(W, 1, 2, "dist_roads"):
    print(f"  {r['lower']:.0f}-{r['upper']:.0f}  possible {r['possible']:.0f} executed {r['executed']:.0f}  "
          f"W+ {r['weight']:.3f} C {r['contrast']:.3f} sig {r['significant']:.0f}")
# the table of one transition and variable as a CSV (for a chart)
tab = [{"range": f"{r['lower']:.0f}-{r['upper']:.0f}", "W+": round(r["weight"], 3)}
       for r in dc.weights_rows(W, 1, 3, "dist_roads")]
figs.write_csv(data / "roads_1_3.csv", tab)

# how many significant ranges each driver has, per transition (a summary)
trs = sorted({(int(r["from"]), int(r["to"])) for r in rows})
print("transitions:", trs)
summary = []
for v in dc.SORTED:
    k = dc.SORTED.index(v)
    rr = [r for r in rows if int(r["variable"]) == k and (int(r["from"]), int(r["to"])) in [(1, 2), (1, 3), (1, 4)]]
    sig = sum(r["significant"] for r in rr)
    summary.append((v, len(rr), int(sig), max(abs(r["contrast"]) for r in rr)))
for s in sorted(summary, key=lambda x: -x[3]):
    print(f"  {s[0]:18s} ranges {s[1]:3d} significant {s[2]:3d} max|C| {s[3]:.2f}")


# ---- bars: four drivers of two transitions
def panel(tr, vars_, png, title):
    fig, axs = plt.subplots(2, 2, figsize=(9.6, 6.0), dpi=150)
    for ax, v in zip(axs.flat, vars_):
        rr = dc.weights_rows(W, tr[0], tr[1], v)
        dc.weight_bars(ax, rr, LABEL[v], categorical=v in dc.CATEGORICAL, unit=UNITS.get(v, ""))
        ax.set_title(LABEL[v], loc="left", fontsize=11)
        if not v in dc.CATEGORICAL:
            ax.set_xlim(left=0)
    fig.suptitle(title, x=0.01, ha="left", fontsize=12.5, fontweight="bold", color=figs.INK)
    fig.tight_layout()
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


panel((1, 3), ["dist_roads", "dist_sea", "elevation", "slope"], out / "bars_1_3.png",
      "Forest to grassland: W+ of each range (orange: significant)")
panel((1, 2), ["dist_roads", "dist_sea", "elevation", "slope"], out / "bars_1_2.png",
      "Forest to coconut: W+ of each range (orange: significant)")
panel((1, 4), ["dist_streams", "dist_sea", "elevation", "slope"], out / "bars_1_4.png",
      "Forest to rice: W+ of each range (orange: significant)")

# a categorical driver
fig, axs = plt.subplots(1, 3, figsize=(9.6, 2.9), dpi=150, sharey=True)
for ax, tr in zip(axs, [(1, 2), (1, 3), (1, 4)]):
    dc.weight_bars(ax, dc.weights_rows(W, tr[0], tr[1], "ultramafic"), "ultramafic", categorical=True)
    ax.set_xticks([0, 1], ["other soils (0)", "ultramafic (1)"])
    ax.set_xlabel("")
    ax.set_title(f"Forest to {dc.NAMES[tr[1]].lower()}", loc="left", fontsize=11)
fig.tight_layout()
fig.savefig(out / "bars_ultramafic.png", dpi=150)
plt.close(fig)
figs._shrink(out / "bars_ultramafic.png")

# ---- the evidence: forest of 1997 by its distance to roads, and the cells that became grassland
t0, t1 = dc.landuse()
d = figs.read(work / "sibuyan_drivers.tif", dc.DRIVERS.index("dist_roads") + 1)[0].filled(np.nan)
fig, ax = plt.subplots(figsize=(7.4, 5.4), dpi=150)
bands = np.where(t0 == 1, np.minimum(np.floor(d / 1000), 6), np.nan)
cm = plt.get_cmap("Greens_r", 7)
im = ax.imshow(bands, cmap=cm, vmin=-0.5, vmax=6.5, interpolation="nearest")
other = np.where(~np.isnan(t0) & (t0 != 1), 1, np.nan)
ax.imshow(other, cmap=plt.matplotlib.colors.ListedColormap(["#e4e2dc"]), interpolation="nearest")
yy, xx = np.where((t0 == 1) & (t1 == 3))
ax.scatter(xx, yy, s=9, c=figs.EMBER, marker="s", linewidths=0, label="became grassland by 2011")
ax.set_axis_off()
cb = fig.colorbar(im, ax=ax, ticks=range(7), fraction=0.035, pad=0.02)
cb.ax.set_yticklabels(["0-1 km", "1-2", "2-3", "3-4", "4-5", "5-6", "> 6 km"])
cb.outline.set_visible(False)
cb.set_label("forest of 1997: distance to roads", color=figs.INK2)
ax.legend(loc="lower left", fontsize=9.5, markerscale=1.6)
fig.tight_layout()
fig.savefig(out / "evidence.png", dpi=150)
plt.close(fig)
figs._shrink(out / "evidence.png")
# the counts behind it
for lo in range(0, 7):
    m = (t0 == 1) & (np.minimum(np.floor(d / 1000), 6) == lo)
    print(f"  roads band {lo}: forest {m.sum():5d} to grassland {np.sum(m & (t1 == 3)):4d}")

# ---- probability maps
names = {(1, 2): "Forest to coconut", (1, 3): "Forest to grassland", (1, 4): "Forest to rice"}
desc = figs.read(work / "probability.tif")[1]["descriptions"]
print("probability layers:", desc)
for b, key in enumerate(desc, start=1):
    f, t = [int(float(x)) for x in key.replace("_to_", " ").split()]
    if (f, t) in names:
        a = figs.read(work / "probability.tif", b)[0].filled(np.nan)
        figs.raster(None, out / f"prob_{f}_{t}.png", array=a, cmap="magma_r", unit="probability",
                    title=names[(f, t)], width=520, log=False, vmin=0, vmax=float(np.nanpercentile(a, 99.5)))

# ---- other increments of the distance to roads (forest to grassland)
incs = [75, 100, 250, 500, 1000]
for inc in incs:
    r = dict(dc.RANGES)
    r["dist_roads"] = inc
    w = dc.sibuyan(f"woe_inc{inc}")
    o = dc.run(fit_model(r), w)
    rr = dc.weights_rows(o / "weights.csv", 1, 3, "dist_roads")
    fig, ax = plt.subplots(figsize=(6.4, 3.4), dpi=150)
    dc.weight_bars(ax, rr, "Distance to roads", unit="m")
    ax.set_xlim(0, 7500)
    ax.set_ylim(-3.2, 2.2)
    ax.set_title(f"Forest to grassland, increment {inc} m: {len(rr)} ranges", loc="left", fontsize=11)
    fig.tight_layout()
    fig.savefig(out / f"inc_{inc}.png", dpi=150)
    plt.close(fig)
    figs._shrink(out / f"inc_{inc}.png")
    print(f"increment {inc}: {len(rr)} ranges, {sum(x['significant'] for x in rr):.0f} significant")

# ---- association between drivers: Cramer's V and JIU on the forest's cells, the drivers in classes
CUT = {"dist_sea": 500, "dist_roads": 500, "dist_urban": 1000, "dist_streams": 250, "elevation": 100, "slope": 5,
       "pop_density": 100}
vs = list(CUT)
nodes = []
for v in vs:
    nodes.append(f"""
[[nodes]]
id = "c_{v}"
op = "map.calc"
inputs = {{ lu = "$t0", d = "$drivers" }}
params = {{ expression = "if lu == 1 then floor(d.{v} / {CUT[v]}) else null" }}
""")
pairs = list(itertools.combinations(vs, 2))
for a, b in pairs:
    nodes.append(f"""
[[nodes]]
id = "k_{a}__{b}"
op = "validate.kappa"
inputs = {{ reference = "c_{a}.map", simulated = "c_{b}.map" }}

[outputs.k_{a}__{b}]
from = "k_{a}__{b}.table"
""")
w = dc.sibuyan("woe_assoc")
# (the drivers as an input of the model: map.calc reads the layers of a stack input by name, d.dist_sea, but not
# those of raster.read's stack output)
INPUTS = """format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.drivers]
kind = "stack"
path = "sibuyan_drivers.tif"
"""
assoc = INPUTS + "".join(nodes)
print(assoc[:1200])
o = dc.run(assoc, w)
V = np.full((len(vs), len(vs)), np.nan)
J = np.full((len(vs), len(vs)), np.nan)
for a, b in pairs:
    t = dc.table(o / f"k_{a}__{b}.csv")[0]
    i, j = vs.index(a), vs.index(b)
    V[j, i] = t["cramer"]
    J[i, j] = t["jiu"]
    print(f"  {a:13s} {b:13s} V {t['cramer']:.3f} JIU {t['jiu']:.3f}")
fig, ax = plt.subplots(figsize=(6.8, 5.6), dpi=150)
M = np.where(np.isnan(V), J, V)
ax.imshow(M, cmap="Oranges", vmin=0, vmax=0.8)
for i in range(len(vs)):
    for j in range(len(vs)):
        if i == j:
            continue
        x = M[i, j]
        ax.text(j, i, f"{x:.2f}", ha="center", va="center", fontsize=9.5,
                color="white" if x > 0.5 else figs.INK, fontweight="bold" if x >= 0.5 else "normal")
SHORT = {"dist_sea": "Dist. to sea", "dist_roads": "Dist. to roads", "dist_urban": "Dist. to towns",
         "dist_streams": "Dist. to streams", "elevation": "Elevation", "slope": "Slope", "pop_density": "Population"}
lab = [SHORT[v] for v in vs]
ax.set_xticks(range(len(vs)), lab, rotation=35, ha="right")
ax.set_yticks(range(len(vs)), lab)
ax.grid(False)
for s in ax.spines.values():
    s.set_visible(False)
ax.set_xlabel("below the diagonal: Cramér's V; above it: joint information uncertainty", fontsize=9,
              color=figs.INK2, labelpad=8)
ax.set_title("Association between drivers, cells of forest in 1997", loc="left", fontsize=11.5)
fig.tight_layout()
fig.savefig(out / "association.png", dpi=150)
plt.close(fig)
figs._shrink(out / "association.png")
print("done")
