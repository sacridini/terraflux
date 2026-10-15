"""Figures of "Expander and Patcher" (land-change/expander-and-patcher): Dinamica EGO's model of Sibuyan run by
Terraflux with other shares of the Expander, other patch sizes and other isometries; the change simulated from 1997
to 2011 in each case, the number and size of the patches (metrics.class), what each function made at every step
(the steps table), the patches measured on the observed change (patches.observe), and a drawing of the two
functions."""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np, plt  # noqa: E402

SLUG = "land-change/expander-and-patcher"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = dc.sibuyan("alloc")

base = dc.woe_settings(dc.new_dinamica(work))
EXTRA = """
[[nodes]]
id = "metrics"
op = "metrics.class"
inputs = { map = "simulation.landscape" }
params = { metrics = ["np", "area_mn"], show = "np" }

[[nodes]]
id = "observed_metrics"
op = "metrics.class"
inputs = { map = "$t1" }
params = { metrics = ["np", "area_mn"], show = "np" }

[[nodes]]
id = "write_maps"
op = "raster.write"
inputs = { value = "simulation.maps" }
params = { path = "maps.tif" }
"""


def prepared(model):
    m = model.replace("[outputs.maps]", EXTRA + "\n[outputs.maps]")
    return dc.outputs(m, {"metrics": "metrics.table", "observed_metrics": "observed_metrics.table",
                          "patchset": "patches.patches"})


def run(model, name, seed=1):
    w = dc.sibuyan(f"alloc_{name}")
    o = dc.run(prepared(model), w, "--set", f"seed={seed}")
    return w, o


def np_of(o, cls=(2, 3, 4)):
    t = {int(r["class"]): r for r in dc.table(o / "metrics.csv")}
    return {c: (int(t[c]["np"]), float(t[c]["area_mn"])) for c in cls if c in t}


t0, t1 = dc.landuse()
CROP = (slice(4, 62), slice(8, 84))     # the north-west of the island, where most change is


def change_png(w, png, title=None, legend=False, width=560):
    sim = figs.read(w / "maps.tif", figs.bands(w / "maps.tif"))[0].filled(np.nan)
    a, b = t0[CROP], sim[CROP]
    dc.draw_change(a, b, png, title=title, legend=legend, width=width)
    return sim


# ---- the model as `terraflux new dinamica` writes it, with the ranges of part 1
w0, o0 = run(base, "base")
obs = json.loads((o0 / "patchset.json").read_text())
print("observed patches:")
for tr in obs["transitions"]:
    print(f"  {tr['from']:.0f}->{tr['to']:.0f} expansion {tr['expansion']:.3f} expander {np.round(tr['expander'], 2)} "
          f"patcher {np.round(tr['patcher'], 2)}")
print("observed np/area_mn 2011:", {int(r['class']): (r['np'], round(r['area_mn'], 2))
                                    for r in dc.table(o0 / "observed_metrics.csv")})
print("simulated np:", np_of(o0))
dc.draw_change(t0[CROP], t1[CROP], out / "observed_change.png", legend=False, width=560)
change_png(w0, out / "simulated_change.png")
dc.legend_strip(out / "legend.png", [("Unchanged", dc.UNCHANGED)] + [(f"to {n.lower()}", c) for k, (n, c) in
                                                                      dc.SIB.items() if k in (2, 3, 4)], width=700)
# what each function did at every step (forest to grassland, forest to coconut)
steps = dc.table(o0 / "steps.csv")
rows = []
for r in steps:
    rows.append({"step": int(r["step"]), "expanded 1-3": int(r["expanded_1_3"]), "patched 1-3": int(r["patched_1_3"]),
                 "expanded 1-2": int(r["expanded_1_2"]), "patched 1-2": int(r["patched_1_2"]),
                 "requested 1-3": int(r["requested_1_3"])})
figs.write_csv(data / "steps.csv", rows)
print("steps 1, 14:", rows[0], rows[-1])

# ---- the Expander's share
rows = []
for e in (0.0, 0.25, 0.5, 0.75, 1.0):
    w, o = run(dc.set_params(base, "patches", {"expansion": e}), f"exp{e}")
    change_png(w, out / f"expansion_{e}.png")
    n = np_of(o)
    rows.append({"expansion": e, "coconut": n[2][0], "grassland": n[3][0], "rice": n[4][0]})
    print("expansion", e, n)
figs.write_csv(data / "expansion_np.csv", rows)

# ---- the isometry
rows = []
for iso in (0.5, 1.0, 1.5, 2.0, 2.5):
    w, o = run(dc.set_params(base, "patches", {"isometry": iso}), f"iso{iso}")
    change_png(w, out / f"isometry_{iso}.png")
    n = np_of(o)
    rows.append({"isometry": iso, "coconut": n[2][0], "grassland": n[3][0], "rice": n[4][0]})
    print("isometry", iso, n)
figs.write_csv(data / "isometry_np.csv", rows)

# ---- the patch sizes: the observed settings with every mean size set (variance: the mean squared / 2)
i = base.index('[[nodes]]\nid = "patches"')
j = base.index('[[nodes]]\nid = "simulation"')
rows = []
for size in (1, 4, 16, 36):
    trs = []
    for tr in obs["transitions"]:
        trs.append({"from": int(tr["from"]), "to": int(tr["to"]), "expansion": round(tr["expansion"], 4),
                    "expander": [size, size * size / 2, 1.5], "patcher": [size, size * size / 2, 1.5]})
    node = (f'[[nodes]]\nid = "patches"\nop = "patches.settings"\n'
            f'params = {{ transitions = {dc.toml_value(trs)} }}\n\n')
    m = base[:i] + node + base[j:]
    if size == 16:
        print(node[:600])
    w, o = run(m, f"size{size}")
    change_png(w, out / f"size_{size}.png")
    n = np_of(o)
    rows.append({"size": size, "coconut": n[2][0], "grassland": n[3][0], "rice": n[4][0]})
    print("size", size, n)
figs.write_csv(data / "size_np.csv", rows)

# ---- drawing: the Expander and the Patcher on a toy landscape
rng = np.random.default_rng(3)
g = np.ones((14, 20))
g[3:8, 3:8] = 3          # an existing patch of grassland
g[2, 5] = g[8, 4] = 3
fig, axs = plt.subplots(1, 3, figsize=(9.6, 3.0), dpi=150)
cols = {1: dc.SIB[1][1], 3: dc.SIB[3][1], 9: "#c9e089", 8: figs.EMBER}


def draw(ax, a, title):
    rgb = np.zeros(a.shape + (3,))
    for v, c in cols.items():
        rgb[a == v] = plt.matplotlib.colors.to_rgb(c)
    ax.imshow(rgb, interpolation="nearest")
    ax.set_xticks(np.arange(-0.5, a.shape[1], 1), [])
    ax.set_yticks(np.arange(-0.5, a.shape[0], 1), [])
    ax.tick_params(length=0)
    ax.grid(True, color="white", linewidth=0.5, linestyle="-")
    ax.set_title(title, loc="left", fontsize=10.5)


draw(axs[0], g, "Start: forest and grassland")
e = g.copy()
for (r, c) in [(3, 8), (4, 8), (5, 8), (4, 9), (8, 5), (8, 6), (9, 5), (2, 6), (2, 4)]:
    e[r, c] = 9
draw(axs[1], e, "Expander: around grassland")
p = g.copy()
for (r, c) in [(10, 14), (10, 15), (11, 14), (11, 15), (9, 15), (11, 16), (3, 15), (3, 16), (4, 16)]:
    p[r, c] = 8
draw(axs[2], p, "Patcher: new patches")
fig.tight_layout()
fig.savefig(out / "functions.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
plt.close(fig)
figs._shrink(out / "functions.png")

# ---- a toy landscape (neutral landscape models), one step of the Expander and Patcher with given settings
TOY = """format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, p = 0.5, weights = [0.8, 0.2], seed = 4 }
note = "The map at the start: 1 forest, 2 farmland."

[[nodes]]
id = "suitability"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 30, seed = 5 }
note = "A driver: how well each cell suits farming."

[[nodes]]
id = "drivers"
op = "stack.of"
inputs = { suitability = "suitability.map" }

[[nodes]]
id = "observed"
op = "map.calc"
inputs = { land = "land.map", s = "suitability.map" }
params = { expression = "if land == 1 and s > 0.68 then 2 else land" }
note = "A change to learn from: forest cleared where farming suits it best."

[[nodes]]
id = "chain"
op = "markov.fit"
inputs = { t0 = "land.map", t1 = "observed.map", steps = 1 }

[[nodes]]
id = "weights"
op = "woe.fit"
inputs = { t0 = "land.map", t1 = "observed.map", drivers = "drivers.stack", markov = "chain.markov" }
params = { ranges = { suitability = 0.05 } }

[[nodes]]
id = "probability"
op = "woe.probability"
inputs = { model = "weights.model", landscape = "land.map", drivers = "drivers.stack" }

[[nodes]]
id = "patches"
op = "patches.settings"
params = { transitions = [{ from = 1, to = 2, expansion = EXP, expander = [MEAN, VAR, ISO], patcher = [MEAN, VAR, ISO] }] }

[[nodes]]
id = "allocate"
op = "dinamica.step"
inputs = { landscape = "land.map", probability = "probability.probability", rates = "chain.markov", patches = "patches.patches", step = 1, seed = 1 }

[[nodes]]
id = "write_start"
op = "raster.write"
inputs = { value = "land.map" }
params = { path = "start.tif" }

[[nodes]]
id = "write_end"
op = "raster.write"
inputs = { value = "allocate.landscape" }
params = { path = "end.tif" }

[[nodes]]
id = "write_probability"
op = "raster.write"
inputs = { value = "probability.probability" }
params = { path = "probability.tif" }

[outputs.info]
from = "allocate.info"
"""
TOYCOL = {1: ("Forest", dc.SIB[1][1]), 2: ("Farmland at the start", "#d9a441"), 3: ("New farmland", "#b4508a")}


def toy(name, exp=0.5, mean=20, var=200, iso=1.5):
    m = (TOY.replace("EXP", repr(exp)).replace("MEAN", repr(mean)).replace("VAR", repr(var))
         .replace("ISO", repr(iso)))
    w = figs.workdir(f"land-change-a/toy_{name}")
    dc.run(m, w)
    a = figs.read(w / "start.tif")[0].filled(np.nan)
    b = figs.read(w / "end.tif")[0].filled(np.nan)
    code = np.where(a != b, 3, a)
    figs.raster(None, out / f"toy_{name}.png", array=code, palette=TOYCOL, legend=False, width=520)
    info = dc.table(w / "output" / "info.csv")[0]
    from scipy import ndimage
    lab, k = ndimage.label(code == 3, structure=np.ones((3, 3)))
    sizes = np.bincount(lab.ravel())[1:]
    print(f"toy {name}: expanded {info['expanded_1_2']:.0f} patched {info['patched_1_2']:.0f} "
          f"patches made {info['patches_1_2']:.0f}; new clumps {k}, mean {sizes.mean():.1f}, largest {sizes.max()}")
    return w, m


w, m = toy("base")
(data / "toy.toml").write_text(m, encoding="utf-8")
figs.raster(w / "probability.tif", out / "toy_probability.png", cmap="viridis", unit="probability", width=600)
a = figs.read(w / "start.tif")[0].filled(np.nan)
figs.raster(None, out / "toy_start.png", array=a, palette={1: TOYCOL[1], 2: TOYCOL[2]}, width=600)
for e in (0.0, 0.5, 1.0):
    toy(f"exp_{e}", exp=e)
for mean in (1, 4, 20, 80):
    toy(f"size_{mean}", mean=mean, var=mean * mean / 2)
for iso in (0.5, 1.0, 1.5, 2.0, 3.0):
    toy(f"iso_{iso}", iso=iso)
dc.legend_strip(out / "toy_legend.png", [(n, c) for n, c in TOYCOL.values()], width=600)
print("done")
