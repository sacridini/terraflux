"""Figures of 'Comparing maps: quantity, allocation and resolution' (change-detection/comparing-maps).

Sibuyan's observed 2011 compared with two predictions: the null model (the 1997 map: no change) and a map simulated
by Terraflux's ordered allocation model (terraflux new ordered, lulcc's OrderedModel: logistic regressions on the 13
drivers, the observed 2011 areas as the demand). The comparisons: validate.futures (quantity and allocation
disagreement), validate.kappa, change.components (analysis = error), change.resolutions, change.memberships,
change.mad; and a sample's cross-tabulation scaled to the population (change.sample_to_population) for the
estimation of areas with a made-up sample (Olofsson et al.'s stratified estimator, its standard errors computed here).

    python website/figures/change-detection/comparing-maps.py
"""

import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _common as c  # noqa: E402
from _common import figs, np, plt  # noqa: E402

SLUG = "comparing-maps"
work, img, data = c.setup(SLUG, files=("sibuyan_landuse.tif", "sibuyan_drivers.tif"))

# ------------------------------------------------------------------------------------------------ the simulated map
print(figs.cli("new", "ordered", "--observed", "sibuyan_landuse.tif", "--times", "0,14", "--drivers",
               "sibuyan_drivers.tif", "--order", "2,3,4,5,1", "-o", "ordered.toml", cwd=work))
figs.run(work / "ordered.toml", work)
import shutil  # noqa: E402

import rasterio  # noqa: E402

with rasterio.open(work / "output" / "maps.tif") as ds:
    prof = ds.profile
    last = ds.read(ds.count)
prof.update(count=1)
with rasterio.open(work / "simulated_2011.tif", "w", **prof) as o:
    o.write(last, 1)
    o.set_band_description(1, "simulated 2011")

MODEL = """format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "observed"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "simulated_2011.tif" }

[[nodes]]
id = "disagreement"
op = "validate.futures"
inputs = { reference = "observed.map", simulated = "simulated.map", original = "land_1997.map" }

[[nodes]]
id = "null_model"
op = "validate.futures"
inputs = { reference = "observed.map", simulated = "land_1997.map" }

[[nodes]]
id = "kappa"
op = "validate.kappa"
inputs = { reference = "observed.map", simulated = "simulated.map" }

[[nodes]]
id = "kappa_null"
op = "validate.kappa"
inputs = { reference = "observed.map", simulated = "land_1997.map" }

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "simulated.map", to = "observed.map" }
params = { analysis = "error" }

[[nodes]]
id = "resolutions"
op = "change.resolutions"
inputs = { comp = "simulated.map", ref = "observed.map" }

[[nodes]]
id = "resolutions_null"
op = "change.resolutions"
inputs = { comp = "land_1997.map", ref = "observed.map" }

[[nodes]]
id = "outcome"
op = "map.calc"
inputs = { t0 = "land_1997.map", o = "observed.map", s = "simulated.map" }
params = { expression = "if o != t0 and s == o then 1 else if o != t0 and s != t0 then 2 else if o != t0 then 3 else if s != t0 then 4 else 0" }
"""
writes = {
    "disagreement.metrics": "fut_metrics.csv", "disagreement.classes": "fut_classes.csv",
    "null_model.metrics": "null_metrics.csv", "null_model.classes": "null_classes.csv",
    "kappa.table": "kappa.csv", "kappa_null.table": "kappa_null.csv",
    "components.table": "components.csv", "components.overall": "overall.csv",
    "resolutions.table": "resolutions.csv", "resolutions_null.table": "resolutions_null.csv",
    "outcome.map": "outcome.tif",
}
(work / "page_model.toml").write_text(MODEL, encoding="utf-8")
figs.run(figs.add_writes(MODEL, writes), work)
out = work / "output"
# single-band copies of the maps for the figures and the smaller models below (written here: raster.write of a
# raster.read node's map, a band chosen, writes every band of the file)
with rasterio.open(work / "sibuyan_landuse.tif") as ds:
    for name, band in (("land_1997.tif", 1), ("observed.tif", 2)):
        with rasterio.open(work / name, "w", **prof) as o:
            o.write(ds.read(band), 1)
shutil.copy2(work / "simulated_2011.tif", work / "simulated.tif")
T = {k: c.table(out / v) for k, v in writes.items() if v.endswith(".csv")}
for k, v in T.items():
    print(k)
    for r in v:
        print("   ", {a: (round(b, 5) if isinstance(b, float) else b) for a, b in r.items()})

# ------------------------------------------------------------------------------------------------ maps
obs = figs.read(work / "observed.tif")[0]
sim = figs.read(work / "simulated.tif")[0]
t0 = figs.read(work / "land_1997.tif")[0]
fig, axes = plt.subplots(1, 3, figsize=(10, 3.5), dpi=150)
for ax, a, t in zip(axes, (t0, obs, sim), ("Null model (the 1997 map)", "Observed 2011 (reference)",
                                         "Simulated 2011")):
    c.classes_map(a, c.SIB, ax)
    ax.set_title(t, loc="left", fontsize=11)
fig.legend(handles=c.legend_handles(c.SIB), loc="lower center", ncol=5, fontsize=9.5, bbox_to_anchor=(0.5, -0.02))
fig.subplots_adjust(bottom=0.1, wspace=0.03)
c.save(fig, img / "three_maps.png")
for name, a in (("observed", obs), ("simulated", sim)):
    figs.raster(None, img / f"{name}.png", array=a, palette=c.SIB, legend=False, width=760)

oc = figs.read(work / "outcome.tif")[0]
OUT = {0: ("Persistence, simulated as persistence", "#ebe7de"), 1: ("Hit: change simulated right", "#2f6f9f"),
       2: ("Wrong hit: change, wrong class", "#9b59b6"), 3: ("Miss: change simulated as persistence", "#e8901f"),
       4: ("False alarm: persistence simulated as change", "#c0392b")}
figs.raster(None, img / "outcome.png", array=oc, palette=OUT, counts=True, width=900)

# ------------------------------------------------------------------------------------------------ disagreement bars
fm, nm = T["disagreement.metrics"][0], T["null_model.metrics"][0]
rows = [{"map": "Null model (1997)", "quantity": round(nm["total_quantity"] * 100, 3),
         "allocation": round(nm["total_allocation"] * 100, 3)},
        {"map": "Simulated 2011", "quantity": round(fm["total_quantity"] * 100, 3),
         "allocation": round(fm["total_allocation"] * 100, 3)}]
figs.write_csv(data / "disagreement.csv", rows)
names = {k: v[0] for k, v in c.SIB.items()}
rows = []
for a, b in zip(T["disagreement.classes"], T["null_model.classes"]):
    k = int(a["class"])
    rows.append({"class": names[k], "sim_quantity": round(a["quantity"] * 100, 3),
                 "sim_allocation": round(a["allocation"] * 100, 3), "null_quantity": round(b["quantity"] * 100, 3),
                 "null_allocation": round(b["allocation"] * 100, 3)})
figs.write_csv(data / "disagreement_classes.csv", rows)

# ------------------------------------------------------------------------------------------------ resolutions
res, resn = T["resolutions.table"], T["resolutions_null.table"]
rows = []
for a, b in zip(res, resn):
    if a["Quantity"] is None:
        continue
    rows.append({"resolution": int(a["Resolution"]), "sim_quantity": round(a["Quantity"], 3),
                 "sim_exchange": round(a["Exchange"], 3), "sim_shift": round(a["Shift"], 3),
                 "null_overall": round(b["Overall"], 3) if b["Overall"] is not None else ""})
figs.write_csv(data / "resolutions.csv", rows)

# the forest's share in blocks of 1, 2, 4, 8, 16 cells: simulated less observed
RES_MODEL = """format = "terraflux/1"

[[nodes]]
id = "observed"
op = "raster.read"
params = { path = "observed.tif" }

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "simulated.tif" }

[[nodes]]
id = "obs_shares"
op = "change.memberships"
inputs = { map = "observed.map" }
params = { factor = FACTOR }

[[nodes]]
id = "sim_shares"
op = "change.memberships"
inputs = { map = "simulated.map" }
params = { factor = FACTOR }

[[nodes]]
id = "composite"
op = "change.composite"
inputs = { comp = "simulated.map", ref = "observed.map" }
params = { factor = FACTOR }
"""
import shutil  # noqa: E402

frames = []
for f in (1, 2, 4, 8, 16):
    w = figs.workdir(f"{c.CATEGORY}/{SLUG}-f{f}")
    for n in ("observed.tif", "simulated.tif"):
        shutil.copy2(work / n, w / n)
    figs.run(figs.add_writes(RES_MODEL.replace("FACTOR", str(f)),
                             {"obs_shares.stack": "obs.tif", "sim_shares.stack": "sim.tif",
                              "composite.table": "composite.csv"}), w)
    o1 = figs.read(w / "obs.tif", 1)[0]
    s1 = figs.read(w / "sim.tif", 1)[0]
    d = (s1 - o1)
    big = np.kron(d.filled(np.nan), np.ones((f, f)))[:108, :128]
    big = np.pad(big, ((0, 108 - big.shape[0]), (0, 128 - big.shape[1])), constant_values=np.nan)
    big = np.ma.masked_invalid(big)
    # the island's outline under the blocks
    figs.raster(None, img / f"forest_diff_{f}.png", array=big, diverging=True, vmin=-1, vmax=1, cmap="RdBu",
                width=760, unit="simulated − observed forest share")
    comp = c.table(w / "output" / "composite.csv")
    agree = sum(r[str(int(r["comp"]))] for r in comp)
    blocks = int(d.count())
    r = next((x for x in res if int(x["Multiples"]) == f), None)
    frames.append((f, blocks, r, agree))
    print("factor", f, "blocks", blocks, "agreement", round(agree, 4), "res row", r)

figs.write_csv(data / "blocks.csv", [{"resolution": int(r["Resolution"]), "quantity": round(r["Quantity"], 3),
                                      "exchange": round(r["Exchange"], 3), "shift": round(r["Shift"], 3)}
                                     for (_, _, r, _) in frames])
for f, blocks, r, agree in frames:
    print(f"SLIDER factor {f}: {blocks} blocks, Q {r['Quantity']:.2f} E {r['Exchange']:.2f} S {r['Shift']:.2f} "
          f"overall {r['Overall']:.2f}, composite agreement {agree * 100:.1f}%")

# ------------------------------------------------------------------------------------------------ MAD of forest shares
MAD_MODEL = """format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "land_1997.tif" }

[[nodes]]
id = "observed"
op = "raster.read"
params = { path = "observed.tif" }

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "simulated.tif" }

[[nodes]]
id = "observed_forest"
op = "map.calc"
inputs = { m = "observed.map" }
params = { expression = "nbmean(m == 1, 2)" }

[[nodes]]
id = "simulated_forest"
op = "map.calc"
inputs = { m = "simulated.map" }
params = { expression = "nbmean(m == 1, 2)" }

[[nodes]]
id = "mad"
op = "change.mad"
inputs = { grid1 = "simulated_forest.map", grid2 = "observed_forest.map", strata = "land_1997.map" }
"""
w = figs.workdir(f"{c.CATEGORY}/{SLUG}-mad")
for n in ("observed.tif", "simulated.tif", "land_1997.tif"):
    shutil.copy2(work / n, w / n)
figs.run(figs.add_writes(MAD_MODEL, {"mad.table": "mad.csv", "observed_forest.map": "of.tif",
                                     "simulated_forest.map": "sf.tif"}), w)
mad = c.table(w / "output" / "mad.csv")
print("MAD", mad)
figs.write_csv(data / "mad.csv", [{"resolution": f'{r["Resolution"] / 1000:g} km', "quantity": round(r["Quantity"], 5),
                                   "strata": round(r["Strata"], 5), "element": round(r["Element"], 5)}
                                  for r in mad])
of = figs.read(w / "of.tif")[0]
sf = figs.read(w / "sf.tif")[0]
fig, axes = plt.subplots(1, 2, figsize=(9, 3.9), dpi=150)
for ax, a, t in zip(axes, (of, sf), ("Observed 2011", "Simulated 2011")):
    im = ax.imshow(a, cmap="Greens", vmin=0, vmax=1, interpolation="nearest")
    ax.set_axis_off()
    ax.set_title(t, loc="left", fontsize=11)
cb = fig.colorbar(im, ax=axes, fraction=0.03, pad=0.02)
cb.outline.set_visible(False)
cb.set_label("forest share in 5 × 5 cells", fontsize=9.5)
c.save(fig, img / "forest_share.png")

# ------------------------------------------------------------------------------------------------ area estimation
S_MODEL = """format = "terraflux/1"

[[nodes]]
id = "sample"
op = "table.of"
params = { columns = ["map", "1", "2", "3"], rows = [[1, 96, 3, 1], [2, 9, 86, 5], [3, 1, 4, 95]] }

[[nodes]]
id = "mapped"
op = "table.of"
params = { columns = ["category", "size"], rows = [[1, STABLE_FOREST], [2, LOSS], [3, STABLE_OTHER]] }

[[nodes]]
id = "scaled"
op = "change.sample_to_population"
inputs = { matrix = "sample.table", population = "mapped.table" }
"""
SIMPLE = """format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "land_1997.tif" }

[[nodes]]
id = "observed"
op = "raster.read"
params = { path = "observed.tif" }

[[nodes]]
id = "forest_change"
op = "change.simple"
inputs = { before = "land_1997.map", after = "observed.map" }
params = { foreground = [1] }
"""
w = figs.workdir(f"{c.CATEGORY}/{SLUG}-sample")
for n in ("observed.tif", "land_1997.tif"):
    shutil.copy2(work / n, w / n)
figs.run(figs.add_writes(SIMPLE, {"forest_change.stats": "simple.csv"}), w)
st = c.table(w / "output" / "simple.csv")[0]
print("SIMPLE", st)
pop = {1: int(st["stable_foreground"]), 2: int(st["loss"]), 3: int(st["stable_background"] + st["gain"])}
sm = S_MODEL.replace("STABLE_FOREST", str(pop[1])).replace("LOSS", str(pop[2])).replace("STABLE_OTHER", str(pop[3]))
(work / "sample_model.toml").write_text(sm, encoding="utf-8")
figs.run(figs.add_writes(sm, {"scaled.table": "scaled.csv"}), w)
scaled = c.table(w / "output" / "scaled.csv")
print("SCALED", scaled)

# Olofsson et al. (2014): the stratified estimator of the areas and its standard errors (eq. 9-10)
n = np.array([[96, 3, 1], [9, 86, 5], [1, 4, 95]], dtype=float)
A = np.array([pop[1], pop[2], pop[3]], dtype=float) * c.SIB_KM2
W = A / A.sum()
ni = n.sum(1)
p = W[:, None] * n / ni[:, None]
area = p.sum(0) * A.sum()
se = np.sqrt(((W[:, None] ** 2) * (n / ni[:, None]) * (1 - n / ni[:, None]) / (ni[:, None] - 1)).sum(0)) * A.sum()
users = np.diag(n) / ni
producers = np.diag(p) / p.sum(0)
overall = np.trace(p)
lab = ["Stable forest", "Forest loss", "Stable non-forest"]
for k in range(3):
    print(f"{lab[k]}: mapped {A[k]:.2f} km2, estimated {area[k]:.2f} +- {1.96 * se[k]:.2f} (95%), "
          f"user's {users[k]:.3f}, producer's {producers[k]:.3f}; scaled column sum "
          f"{sum(r[str(k + 1)] for r in scaled) * c.SIB_KM2:.2f}")
print("overall accuracy", round(overall, 4))


def draw(ax):
    x = np.arange(3)
    ax.bar(x - 0.18, A, width=0.34, color="#bfc8c0", label="Mapped area (pixel counting)")
    ax.bar(x + 0.18, area, width=0.34, color=figs.FOREST, label="Estimated from the sample")
    ax.errorbar(x + 0.18, area, yerr=1.96 * se, fmt="none", ecolor=figs.INK, capsize=4, lw=1.2)
    for k in range(3):
        ax.text(x[k] - 0.18, A[k] + 6, f"{A[k]:.1f}", ha="center", fontsize=9, color=figs.INK2)
        ax.text(x[k] + 0.18, area[k] + 1.96 * se[k] + 6, f"{area[k]:.1f} ± {1.96 * se[k]:.1f}", ha="center",
                fontsize=9, color=figs.INK)
    ax.set_xticks(x, lab)
    ax.set_ylabel("km²")
    ax.grid(axis="x", visible=False)
    ax.legend(loc="upper right", fontsize=9.5)
    ax.set_ylim(0, 330)


figs.chart(img / "areas.png", draw, width=900, height=380,
           title="Areas of the forest's change: mapped, and estimated from a sample (95% intervals)")
print("images:", sum(p.stat().st_size for p in img.glob("*.png")) / 1e6, "MB")
