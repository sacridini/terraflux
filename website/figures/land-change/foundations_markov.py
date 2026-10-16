"""Figures of "Transition rates with Markov chains" (land-change/transition-rates-markov-chains): the Markov chain of
Sibuyan fitted by Terraflux (markov.fit, 14 yearly steps), its matrices of the period and of a year, the areas it
projects (and what dividing the rates by 14 would give), and a scenario given as a matrix (markov.matrix)."""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np, plt  # noqa: E402

SLUG = "land-change/transition-rates-markov-chains"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = dc.sibuyan("markov")

MODEL = """format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "chain"
op = "markov.fit"
inputs = { t0 = "land_1997.raster", t1 = "land_2011.raster", steps = 14 }

[outputs.chain]
from = "chain.markov"
"""
res = dc.run(MODEL, work)
ch = json.loads((res / "chain.json").read_text())
cls = [int(c) for c in ch["classes"]]
k = len(cls)
P = np.array(ch["single"]).reshape(k, k)       # the period 1997-2011
Q = np.array(ch["multi"]).reshape(k, k)        # one year
print("classes", cls)
print("period:\n", np.round(P, 4))
print("year:\n", np.round(Q, 5))
print("max |Q^14 - P| =", np.abs(np.linalg.matrix_power(Q, 14) - P).max())
print("negative entries of Q:", (Q < -1e-12).sum(), Q.min())
naive = np.eye(k) + (P - np.eye(k)) / 14
print("naive^14 vs P, forest row:", np.round(np.linalg.matrix_power(naive, 14)[0], 4), np.round(P[0], 4))

names = [dc.NAMES[c] for c in cls]


def heat(M, png, title, fmt):
    fig, ax = plt.subplots(figsize=(5.6, 4.6), dpi=150)
    off = M.copy()
    np.fill_diagonal(off, np.nan)
    top = np.nanmax(off) * 1.15
    ax.imshow(np.where((np.eye(k) == 1) | (np.abs(M) < 5e-7), np.nan, np.clip(M, 0, None)), cmap="Oranges", vmin=0,
              vmax=top)
    for i in range(k):
        for j in range(k):
            v = M[i, j]
            col = figs.FOREST if i == j else ("white" if v > 0.6 * top else (figs.EMBER if v < 0 else figs.INK))
            ax.text(j, i, fmt(v), ha="center", va="center", fontsize=9.5, color=col,
                    fontweight="bold" if i == j or v < 0 else "normal")
    ax.set_xticks(range(k), names, rotation=30, ha="right")
    ax.set_yticks(range(k), names)
    ax.set_xlabel("to (2011)" if "period" in title else "to (next year)")
    ax.set_ylabel("from")
    ax.grid(False)
    for s in ax.spines.values():
        s.set_visible(False)
    ax.set_title(title, loc="left")
    fig.tight_layout()
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


heat(P, out / "matrix_period.png", "The period, 1997 to 2011", lambda v: "0" if v == 0 else f"{v:.3f}")
heat(Q, out / "matrix_year.png", "One year (the 14th root)", lambda v: "0" if abs(v) < 5e-7 else (f"{v:.0e}".replace("e-0", "e-") if abs(v) < 1e-4 else f"{v:.4f}"))

# the areas each chain projects, from 1997 (in % of the island)
t0, t1 = dc.landuse()
cells = np.count_nonzero(~np.isnan(t0))
a0 = np.array([np.sum(t0 == c) for c in cls], dtype=float)
a1 = np.array([np.sum(t1 == c) for c in cls], dtype=float)
rows = []
x_root, x_naive = a0.copy(), a0.copy()
for yr in range(1997, 2026):
    r = {"year": yr}
    for j, c in enumerate(cls):
        if c in (1, 2, 3):
            r[f"{dc.NAMES[c]} (root)"] = round(x_root[j] / cells * 100, 3)
            r[f"{dc.NAMES[c]} (rate / 14)"] = round(x_naive[j] / cells * 100, 3)
    rows.append(r)
    x_root = x_root @ Q
    x_naive = x_naive @ naive
figs.write_csv(data / "projection.csv", rows)
print("observed 2011 %:", np.round(a1 / cells * 100, 2))
print("projected 2011 root:", rows[14])

# the long run from 2011, by the yearly chain
rows = []
x = a1.copy()
for yr in range(2011, 2101):
    rows.append({"year": yr, **{dc.NAMES[c]: round(x[j] / cells * 100, 3) for j, c in enumerate(cls)}})
    x = x @ Q
figs.write_csv(data / "long_run.csv", rows)
print("2050:", rows[39], "\n2100:", rows[-1])

# a scenario given as a matrix: the forest's losses halved
S = P.copy()
S[0, 1:] = P[0, 1:] / 2
S[0, 0] = 1 - S[0, 1:].sum()
scen = f"""format = "terraflux/1"

[[nodes]]
id = "halved"
op = "markov.matrix"
params = {{ classes = {cls}, steps = 14, single = {dc.toml_value([[round(v, 6) for v in row] for row in S])} }}

[outputs.chain]
from = "halved.markov"
"""
print(scen)
res2 = dc.run(scen, work, name="scenario.toml", out="scenario")
sc = json.loads((res2 / "chain.json").read_text())
Qs = np.array(sc["multi"]).reshape(k, k)
print("scenario's yearly forest row:", np.round(Qs[0], 5))
rows = []
x, y = a1.copy(), a1.copy()
for yr in range(2011, 2051):
    rows.append({"year": yr, "trend": round(x[0] / cells * 100, 3), "halved": round(y[0] / cells * 100, 3)})
    x, y = x @ Q, y @ Qs
figs.write_csv(data / "scenario.csv", rows)
print("2050 forest: trend", rows[-1])
print("done")
