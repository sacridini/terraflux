"""Figures of the tutorial "Watershed simulation with SWAT+" (terrain-hydrology/swat-plus).

The SWAT+ project of Hubbard Brook watershed 2 that landschaft ships (hubbard_brook.zip, a TxtInOut folder,
2000-2002) simulated by Terraflux's swat.run: yearly, monthly and daily water balances of the basin, the daily
discharge of its channel, the average annual balance and soil loss of each HRU with both cover factors, and the
year of spin-up.

    python website/figures/terrain-hydrology/swat-plus.py
"""

import io
import json
import pathlib
import shutil
import sys
import zipfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _hydro as H  # noqa: E402
from _hydro import figs, np, plt  # noqa: E402

SLUG = "swat-plus"
work = figs.workdir(f"terrain-hydrology/{SLUG}")
out = figs.img_dir(f"terrain-hydrology/{SLUG}")
data = figs.data_dir(f"terrain-hydrology/{SLUG}")
shutil.copy2(H.SWAT_EXAMPLE, work / "hubbard_brook.zip")
numbers = {}


def swat(name, step, tables=None, nyskip=None, c_factor=None, outs=None):
    p = [f'path = "hubbard_brook.zip"', f'step = "{step}"']
    if tables:
        p.append("tables = [" + ", ".join(f'"{t}"' for t in tables) + "]")
    if nyskip is not None:
        p.append(f"nyskip = {nyskip}")
    if c_factor:
        p.append(f'c_factor = "{c_factor}"')
    model = f'''format = "terraflux/1"

[[nodes]]
id = "swat"
op = "swat.run"
params = {{ {", ".join(p)} }}
'''
    for t in outs or tables:
        model += f'''
[outputs.{name}_{t}]
from = "swat.{t}"
'''
    figs.run(model, work, name=f"{name}.toml")
    return {t: H.table(work, f"{name}_{t}") for t in (outs or tables)}


ALL = ["basin_wb", "basin_ls", "hru_wb", "hru_ls", "hru_pw", "channel_sd", "aquifer"]
year = swat("year", "year", outs=ALL + ["weather"])
aa = swat("aa", "aa", tables=["basin_wb", "basin_ls", "hru_wb", "hru_ls", "hru_pw"])
plant = swat("plant", "aa", tables=["basin_ls", "hru_ls"], c_factor="plant")
month = swat("month", "month", tables=["basin_wb", "basin_pw"], nyskip=0)
day = swat("day", "day", tables=["basin_wb", "channel_sd"], nyskip=0)
spin = swat("spin", "year", tables=["basin_wb", "aquifer"], nyskip=0)

# ------------------------------------------------------------------ the project's own description
z = zipfile.ZipFile(work / "hubbard_brook.zip")


def text(n):
    return z.read(n).decode("latin-1")


hrus = []
lum = {r.split()[1]: r.split()[5] for r in text("hru-data.hru").splitlines()[2:] if r.strip()}
area = {r.split()[1]: float(r.split()[3]) for r in text("hru.con").splitlines()[2:] if r.strip()}
topo = {r.split()[0]: (float(r.split()[1]), float(r.split()[2])) for r in text("topography.hyd").splitlines()[2:] if r.strip()}
LU = {"frsd_new_lum": "Young forest", "frsd_mat_lum": "Mature forest", "barn_lum": "Barren"}
for i in range(1, 11):
    n = f"hru{i:02d}"
    hrus.append(dict(hru=i, landuse=LU[lum[n]], area_ha=area[n], slope=topo[f"topo{n}"][0] * 100,
                     slope_len=topo[f"topo{n}"][1]))
numbers["hrus"] = hrus
numbers["total_ha"] = sum(h["area_ha"] for h in hrus)
numbers["files"] = len([n for n in z.namelist() if not n.endswith("/")])
numbers["stations"] = len({int(r["station"]) for r in year["weather"]})
numbers["weather_rows"] = len(year["weather"])
numbers["weather_cols"] = list(year["weather"][0].keys())
numbers["columns"] = {t: list(v[0].keys()) for t, v in year.items() if v}

# ------------------------------------------------------------------ the water balance of the basin
COLS = ["precip", "snofall", "snomlt", "et", "esoil", "eplant", "surq_gen", "latq", "perc", "wateryld", "pet", "cn"]
rows = []
for r in year["basin_wb"]:
    rows.append({"year": int(r["year"]), **{c: round(r[c], 1) for c in COLS}})
a0 = aa["basin_wb"][0]
rows.append({"year": "average", **{c: round(a0[c], 1) for c in COLS}})
# years as text (a zero-width space after them), so that the table does not print them as 2,001
figs.write_csv(data / "basin-year.csv", [{k: (f"{v}​" if k == "year" and v != "average" else
                                              "2001–2002" if k == "year" else v)
                                          for k, v in r.items() if k not in ("esoil", "eplant", "pet", "cn")}
                                         for r in rows])
numbers["basin_year"] = rows
numbers["basin_ls"] = {"swatplus": aa["basin_ls"][0], "plant": plant["basin_ls"][0]}

# monthly, all three years (nyskip 0): where the water goes
mrows = []
import datetime as dt  # noqa: E402

for r, q in zip(month["basin_wb"], month["basin_pw"]):
    d = dt.date(int(r["year"]), 1, 1) + dt.timedelta(days=int(r["day"]) - 1)
    mrows.append({"month": d.strftime("%Y-%m"), "Precipitation": round(r["precip"], 1),
                  "Snowfall": round(r["snofall"], 1), "Snowmelt": round(r["snomlt"], 1),
                  "Evapotranspiration": round(r["et"], 1), "Surface runoff": round(r["surq_gen"], 1),
                  "Lateral flow": round(r["latq"], 1), "Percolation": round(r["perc"], 1),
                  "Snowpack": round(r["snopack"], 1), "Leaf area index": round(q["lai"], 2),
                  "Residue (t/ha)": round(q["residue"] / 1000, 2)})
figs.write_csv(data / "basin-month.csv", mrows)
numbers["month_first"] = mrows[:3]

# daily: rain and the discharge of the channel, a hydrograph
drows = day["basin_wb"]
ch = day["channel_sd"]
dates = [dt.date(int(r["year"]), 1, 1) + dt.timedelta(days=int(r["day"]) - 1) for r in drows]
pcp = np.array([r["precip"] for r in drows])
melt = np.array([r["snomlt"] for r in drows])
flo = np.array([r["flo_out"] for r in ch])
numbers["daily"] = dict(days=len(drows), flo_mean=float(flo.mean()), flo_max=float(flo.max()),
                        flo_max_date=str(dates[int(flo.argmax())]), pcp_max=float(pcp.max()),
                        pcp_max_date=str(dates[int(pcp.argmax())]),
                        zero_days=int((flo < 1e-4).sum()),
                        flo_by_month={m: float(np.mean([f for f, d in zip(flo, dates) if d.month == m]))
                                      for m in range(1, 13)})
numbers["channel_cols"] = list(ch[0].keys())
fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(9.6, 4.8), dpi=150, sharex=True,
                               gridspec_kw={"height_ratios": [1, 2], "hspace": 0.08})
ax1.bar(dates, pcp, width=1.0, color=figs.WATER, lw=0, label="Precipitation")
ax1.bar(dates, melt, width=1.0, color="#9ecae1", lw=0, label="Snowmelt")
ax1.invert_yaxis()
ax1.set_ylabel("mm/day")
ax1.legend(fontsize=9, loc="lower left", ncol=2)
ax2.plot(dates, flo, color=figs.INK, lw=0.9)
ax2.fill_between(dates, flo, color=figs.FOREST, alpha=0.25, lw=0)
ax2.set_ylabel("Discharge (m³/s)")
ax2.set_xlabel("")
fig.tight_layout()
fig.savefig(out / "hydrograph.png", dpi=150)
plt.close(fig)
figs._shrink(out / "hydrograph.png")

# ------------------------------------------------------------------ the HRUs: average annual water and soil loss
hw = {int(r["hru"]): r for r in aa["hru_wb"]}
hl = {int(r["hru"]): r for r in aa["hru_ls"]}
hp = {int(r["hru"]): r for r in plant["hru_ls"]}
hpw = {int(r["hru"]): r for r in aa["hru_pw"]}
hrows = []
for h in hrus:
    i = h["hru"]
    hrows.append({"HRU": i, "Land use": h["landuse"], "Area (ha)": round(h["area_ha"], 2),
                  "Slope (%)": round(h["slope"], 1), "ET (mm)": round(hw[i]["et"], 0),
                  "Surface runoff (mm)": round(hw[i]["surq_gen"], 0), "Lateral flow (mm)": round(hw[i]["latq"], 0),
                  "Percolation (mm)": round(hw[i]["perc"], 0),
                  "Soil loss, SWAT+ 62 C (t/ha)": round(hl[i]["sedyld"], 2),
                  "Soil loss, plant C (t/ha)": round(hp[i]["sedyld"], 3)})
figs.write_csv(data / "hru-aa.csv", hrows)
numbers["hru_aa"] = hrows
numbers["hru_pw"] = {i: {k: hpw[i][k] for k in ("lai", "bioms", "residue", "lai_max", "bm_max")} for i in hpw}
# area-weighted soil loss
A = sum(h["area_ha"] for h in hrus)
numbers["sed_weighted"] = dict(swatplus=sum(hl[h["hru"]]["sedyld"] * h["area_ha"] for h in hrus) / A,
                               plant=sum(hp[h["hru"]]["sedyld"] * h["area_ha"] for h in hrus) / A)

# ------------------------------------------------------------------ the spin-up year
srows = []
for r in spin["basin_wb"]:
    srows.append({"year": int(r["year"]), **{c: round(r[c], 1) for c in ("precip", "et", "surq_gen", "latq", "perc",
                                                                          "wateryld", "sw_init", "sw_final")}})
numbers["spin"] = srows
aq = [r for r in spin["aquifer"] if int(r["aquifer"]) == 1]
numbers["spin_aquifer"] = [{k: round(r[k], 1) for k in ("year", "flo", "stor", "rchrg", "dep_wt")} for r in aq]
numbers["aquifer_cols"] = list(spin["aquifer"][0].keys())
srows_csv = []
for r, q in zip(srows, aq):
    srows_csv.append({"year": f"{r['year']}​" + (" (spin-up)" if r["year"] == 2000 else ""), "precip": r["precip"], "et": r["et"], "surq_gen": r["surq_gen"],
                      "perc": r["perc"], "wateryld": r["wateryld"], "aquifer_flo": round(q["flo"], 1),
                      "aquifer_stor": round(q["stor"], 1)})
figs.write_csv(data / "spin-up.csv", srows_csv)

# ------------------------------------------------------------------ a drawing of the project's objects
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch  # noqa: E402

fig, ax = plt.subplots(figsize=(9.6, 3.9), dpi=150)
ax.set_xlim(0, 10)
ax.set_ylim(0, 4.2)
ax.set_axis_off()
ax.grid(False)


def box(x, y, w, h, t, col, sub=""):
    ax.add_patch(FancyBboxPatch((x, y), w, h, boxstyle="round,pad=0.04,rounding_size=0.12", fc=col, ec="#00000033"))
    ax.text(x + w / 2, y + h / 2 + (0.13 if sub else 0), t, ha="center", va="center", fontsize=10.5,
            color=figs.INK, fontweight="bold")
    if sub:
        ax.text(x + w / 2, y + h / 2 - 0.2, sub, ha="center", va="center", fontsize=8.5, color=figs.INK2)


def arrow(a, b, t="", off=(0, 0.12)):
    ax.add_patch(FancyArrowPatch(a, b, arrowstyle="-|>", mutation_scale=12, color=figs.INK2, lw=1.2))
    if t:
        ax.text((a[0] + b[0]) / 2 + off[0], (a[1] + b[1]) / 2 + off[1], t, ha="center", fontsize=8.5,
                color=figs.INK2)


box(0.1, 3.0, 2.3, 0.9, "Weather", "#e9f1f7", "gauges and generator")
for k in range(10):
    ax.add_patch(FancyBboxPatch((2.9 + (k % 5) * 0.42, 2.55 + (1 - k // 5) * 0.5), 0.34, 0.38,
                                boxstyle="round,pad=0.02,rounding_size=0.05",
                                fc="#d9a441" if k in (4, 5) else figs.FOREST, ec="white", alpha=0.85))
    ax.text(3.07 + (k % 5) * 0.42, 2.74 + (1 - k // 5) * 0.5, str(k + 1), ha="center", va="center", fontsize=7.5,
            color="white")
ax.text(3.9, 3.75, "10 HRUs (forest, barren)", ha="center", fontsize=9.5, color=figs.INK, fontweight="bold")
box(2.9, 1.0, 2.0, 0.85, "Routing unit", "#eef3e8", "rtu10: the 10 HRUs")
box(6.0, 1.95, 2.0, 0.85, "Channel", "#e3eef6", "cha1, SWAT-DEG")
box(6.0, 0.15, 2.0, 0.85, "Shallow aquifer", "#f1ece1", "aqu10")
box(8.45, 0.15, 1.45, 0.85, "Deep aquifer", "#ece6da", "aqu_deep01")
ax.text(9.15, 2.38, "outlet", ha="center", fontsize=9, color=figs.INK2)
arrow((2.45, 3.45), (2.85, 3.1), "")
arrow((3.9, 2.5), (3.9, 1.9), "")
arrow((4.95, 1.6), (5.95, 2.2), "surface and\nlateral flow", off=(-0.35, 0.2))
arrow((4.95, 1.2), (5.95, 0.65), "percolation", off=(-0.45, -0.32))
arrow((7.0, 1.05), (7.0, 1.9), "baseflow", off=(0.45, -0.05))
arrow((8.05, 0.57), (8.4, 0.57))
arrow((8.05, 2.37), (8.75, 2.37))
fig.savefig(out / "objects.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
plt.close(fig)
figs._shrink(out / "objects.png")

(work / "numbers.json").write_text(json.dumps(numbers, indent=1, default=float))
print(json.dumps({k: v for k, v in numbers.items() if k not in ("columns",)}, indent=1, default=float))
