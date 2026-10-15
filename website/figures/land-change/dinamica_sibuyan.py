"""Figures of "Dinamica EGO on Sibuyan" (land-change/dinamica-on-sibuyan): the model `terraflux new dinamica` writes
for Sibuyan (14 yearly steps from 1997 to 2011, the weights' ranges of part 1), run by Terraflux; the map of every
step with the share of each class (a slider and its chart), the observed and simulated maps of 2011, five seeds,
the same model with the automatic ranges, and a projection from 2011 to 2025 (--start, --simulate).

    python website/figures/land-change/dinamica_sibuyan.py
"""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np  # noqa: E402

SLUG = "land-change/dinamica-on-sibuyan"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
t0, t1 = dc.landuse()
CROP = (slice(4, 62), slice(8, 84))     # the north-west of the island, where most change is

VALIDATE = """
[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "$t0", observed = "$t1", simulated = "simulation.landscape" }

[[nodes]]
id = "similarity"
op = "validate.similarity"
inputs = { start = "$t0", observed = "$t1", simulated = "simulation.landscape" }

[outputs.agreement]
from = "agreement.table"

[outputs.similarity]
from = "similarity.table"
"""


def with_validation(model: str) -> str:
    return model.rstrip() + "\n" + VALIDATE


def stack(path):
    return dc.read_stack(path)


# ---- the model as written, with the ranges of part 1
work = dc.sibuyan("sibuyan")
model = dc.woe_settings(dc.new_dinamica(work))
(data / "dinamica.toml").write_text(model, encoding="utf-8")
o = dc.run(with_validation(model), work)
maps = stack(o / "maps.tif")
maps[maps == 255] = np.nan
print("agreement", figs.read_csv(o / "agreement.csv"))
print("similarity", [(r["window"], round(float(r["minimum"]), 3)) for r in figs.read_csv(o / "similarity.csv")])

# the frames: 1997 (step 0) and the 14 steps; a stack of the classes' shares
frames = [t0] + [maps[i] for i in range(maps.shape[0])]
rows = []
total = np.isfinite(t0).sum()
for i, a in enumerate(frames):
    dc.draw_map(a, out / f"step_{i}.png", legend=False, width=640)
    r = {"step": i, "year": 1997 + i}
    for k, (n, _) in dc.SIB.items():
        r[n] = round(float((a == k).sum()) / total * 100, 2)
    r["changed"] = 0 if i == 0 else int(((frames[i] != frames[i - 1]) & np.isfinite(a)).sum())
    rows.append(r)
figs.write_csv(data / "areas.csv", rows)
dc.legend_strip(out / "legend.png", [(n, c) for n, c in dc.SIB.values()], width=640)
for r in rows:
    print(f'  - {{ value: {r["year"]}, src: /img/tutorials/{SLUG}/step_{r["step"]}.png, caption: "'
          f'{"1997, the start" if r["step"] == 0 else "Step " + str(r["step"])}", stats: {{ Forest: "{r["Forest"]:.1f}%", '
          f'Coconut: "{r["Coconut"]:.1f}%", Grassland: "{r["Grassland"]:.1f}%", Rice: "{r["Rice"]:.1f}%", '
          f'"Cells changed": "{r["changed"]}" }} }}')
obs = {n: round(float((t1 == k).sum()) / total * 100, 2) for k, (n, _) in dc.SIB.items()}
print("observed 2011 shares", obs)

# observed and simulated 2011, the same framing (a compare slider)
dc.draw_map(t1, out / "observed_2011.png", legend=False, width=760)
dc.draw_map(maps[-1], out / "simulated_2011.png", legend=False, width=760)
dc.draw_change(t0, t1, out / "change_observed.png", title="Observed change, 1997 to 2011", width=700)
dc.draw_change(t0, maps[-1], out / "change_simulated.png", title="Simulated change, 1997 to 2011", width=700)

# the requests of every step: what the Markov chain asked for, by transition
steps = dc.table(o / "steps.csv")
req = []
for r in steps:
    req.append({"step": int(r["step"]), "forest to coconut": int(r["requested_1_2"]),
                "forest to grassland": int(r["requested_1_3"]), "forest to rice": int(r["requested_1_4"]),
                "grassland to coconut": int(r["requested_3_2"]), "rice to coconut": int(r["requested_4_2"])})
figs.write_csv(data / "requested.csv", req)
print("requested step 1 / 14:", req[0], req[-1])

# ---- five seeds: the change of 2011 in the north-west
fom = []
for seed in (1, 2, 3, 4, 5):
    w = dc.sibuyan(f"sibuyan_seed{seed}")
    oo = dc.run(with_validation(model), w, "--set", f"seed={seed}")
    m = stack(oo / "maps.tif")[-1]
    m[m == 255] = np.nan
    dc.draw_change(t0[CROP], m[CROP], out / f"seed_{seed}.png", legend=False, width=520)
    a = figs.read_csv(oo / "agreement.csv")[0]
    s = {int(float(r["window"])): float(r["minimum"]) for r in figs.read_csv(oo / "similarity.csv")}
    fom.append({"seed": seed, "fom": round(float(a["figure_of_merit"]), 3), "sim1": round(s[1], 3),
                "sim5": round(s[5], 3)})
print("seeds", fom)
dc.legend_strip(out / "change_legend.png", [("Unchanged", dc.UNCHANGED)] + [(f"to {n.lower()}", c) for k, (n, c) in
                                                                             dc.SIB.items() if k in (2, 3, 4)],
                width=640)

# ---- the automatic ranges (as `terraflux new dinamica` writes the model)
w = dc.sibuyan("sibuyan_auto")
oo = dc.run(with_validation(dc.new_dinamica(w)), w)
print("automatic ranges:", figs.read_csv(oo / "agreement.csv"),
      [(r["window"], round(float(r["minimum"]), 3)) for r in figs.read_csv(oo / "similarity.csv")])
print("ranges, automatic:", len(dc.table(oo / "weights.csv")), "chosen:", len(dc.table(o / "weights.csv")))

# ---- a projection: fitted on 1997-2011, from 2011 for 14 steps (2025)
w = dc.sibuyan("sibuyan_2025")
dc.cli("new", "dinamica", "--t0", "sibuyan_landuse.tif:1997", "--t1", "sibuyan_landuse.tif:2011", "--drivers",
       "sibuyan_drivers.tif", "--steps", 14, "--start", "sibuyan_landuse.tif:2011", "--simulate", 14, "-o",
       "projection.toml", cwd=w)
proj = dc.woe_settings((w / "projection.toml").read_text(encoding="utf-8"))
(data / "projection.toml").write_text(proj, encoding="utf-8")
oo = dc.run(proj, w)
p = stack(oo / "maps.tif")[-1]
p[p == 255] = np.nan
dc.draw_map(t1, out / "map_2011.png", title="Observed, 2011", width=620, counts=True)
dc.draw_map(p, out / "map_2025.png", title="Projected, 2025", width=620, counts=True)
print("2025 shares", {n: round(float((p == k).sum()) / total * 100, 2) for k, (n, _) in dc.SIB.items()})
print("projection model head:\n", proj[:1400])
print("done")
