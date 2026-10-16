"""Figures of "Maps larger than memory" (land-change/maps-larger-than-memory), also used by the guide
"Parallel and distributed computing".

Sibuyan's maps with every cell cut into 4 x 4 cells (432 x 512 cells of 62.5 m), Dinamica EGO's model with the
Expander and Patcher's settings given (patches.settings), run:

- by the exact engine (`terraflux run`);
- by the tiled engine with tiles of 128, 256 and 1024 cells (`--tiled --tile N`), and with `--disk`;
- by the tiled engine on a coordinator with two workers of this computer (`--coordinator`), then stopped;

and compared: the cells that differ between the tilings, and each run against the observed map of 2011
(validate.agreement, validate.similarity).

    python website/figures/land-change/tiled.py
"""

import pathlib
import shutil
import subprocess
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "parallel"))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np  # noqa: E402
import _common as pc  # noqa: E402

SLUG = "land-change/maps-larger-than-memory"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
w = figs.workdir("land-change-a/tiled")
model = pc.given_patches(pc.dinamica(w, 4), w / "model.toml")
text = dc.woe_settings(model.read_text(encoding="utf-8"),
                       ranges={"dist_sea": 250, "dist_roads": 250, "dist_urban": 250, "dist_streams": 250,
                               "elevation": 50, "slope": 2, "aspect": 30, "pop_density": 50})
model.write_text(text, encoding="utf-8")
(data / "model.toml").write_text(text, encoding="utf-8")
print(text[:3000])


def tf(*args):
    s, o = pc.tf(*map(str, args), cwd=w)
    return s, o


def last_map(folder):
    p = w / folder
    if (p / "maps.tif").exists():
        a = dc.read_stack(p / "maps.tif")[-1]
    else:
        a = figs.read(sorted((p / "maps").glob("step_*.tif"))[-1])[0].filled(np.nan)
    a[a == 255] = np.nan
    return a


runs = {}
s, o = tf("run", "model.toml", "--out", "exact", "-q")
runs["exact"] = last_map("exact")
print(f"exact: {s:.1f} s")
for tile in (128, 256, 1024):
    s, o = tf("run", "model.toml", "--tiled", "--tile", tile, "--out", f"tiled_{tile}")
    runs[f"tiled {tile}"] = last_map(f"tiled_{tile}")
    print(f"tiled {tile}: {s:.1f} s")
    if tile == 256:
        (data / "tiled_log.txt").write_text(o, encoding="utf-8")
        print(o[:1500])
s, o = tf("run", "model.toml", "--tiled", "--tile", 256, "--disk", "layers", "--out", "tiled_disk")
runs["tiled disk"] = last_map("tiled_disk")
print(f"tiled with --disk: {s:.1f} s; layers:", sorted(p.name for p in (w / "layers").rglob("*"))[:8])

# ---- on a coordinator with two workers (this computer), then stopped
exe = str(figs.binary())
procs = []
try:
    procs.append(subprocess.Popen([exe, "serve", "--listen", "127.0.0.1:7793", "--data", str(w / "coord")],
                                  cwd=w, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
    time.sleep(2)
    for k in (1, 2):
        procs.append(subprocess.Popen([exe, "worker", "--coordinator", "127.0.0.1:7793", "--name", f"worker-{k}",
                                       "--cache", str(w / f"cache{k}"), "--threads", "4"], cwd=w,
                                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
    time.sleep(2)
    s, o = tf("workers", "--coordinator", "http://127.0.0.1:7793")
    print(o)
    s, o = tf("run", "model.toml", "--tiled", "--tile", 256, "--disk", "layers_dist", "--coordinator",
              "http://127.0.0.1:7793", "--out", "tiled_dist")
    print(f"tiled on 2 workers: {s:.1f} s")
    print(o[-1200:])
    runs["tiled 2 workers"] = last_map("tiled_dist")
finally:
    for p in procs:
        p.terminate()
    for p in procs:
        try:
            p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            p.kill()
print("coordinator and workers stopped")

# ---- compare
names = list(runs)
for a in names:
    print(a, " ".join(f"{int(((runs[a] != runs[b]) & np.isfinite(runs[a]) & np.isfinite(runs[b])).sum()):>7}" for b in names))
VAL = """format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "big4_landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "big4_landuse.tif"
band = "2011"

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "SIM" }

[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "$t0", observed = "$t1", simulated = "simulated.raster" }

[[nodes]]
id = "similarity"
op = "validate.similarity"
inputs = { start = "$t0", observed = "$t1", simulated = "simulated.raster" }
params = { windows = [1, 5, 9] }

[outputs.agreement]
from = "agreement.table"

[outputs.similarity]
from = "similarity.table"
"""
rows = []
for name, arr in runs.items():
    p = w / f"cmp_{name.replace(' ', '_')}.tif"
    import rasterio
    with rasterio.open(w / "big4_landuse.tif") as src:
        prof = src.profile
    prof.update(count=1, dtype="float32", nodata=255.0)
    with rasterio.open(p, "w", **prof) as dst:
        dst.write(np.where(np.isnan(arr), 255, arr).astype("float32"), 1)
    o = dc.run(VAL.replace("SIM", p.name), w, name="val.toml", out="val")
    a = figs.read_csv(o / "agreement.csv")[0]
    sm = {int(float(r["window"])): float(r["minimum"]) for r in figs.read_csv(o / "similarity.csv")}
    forest = int(np.nansum(arr == 1))
    rows.append({"run": name, "forest cells": forest, "figure of merit": round(float(a["figure_of_merit"]), 3),
                 "similarity 1": round(sm[1], 3), "similarity 5": round(sm[5], 3), "similarity 9": round(sm[9], 3)})
    print(rows[-1])
figs.write_csv(data / "comparison.csv", rows)

# ---- maps: the north-west, exact and tiled
t0 = figs.read(w / "big4_landuse.tif", 1)[0].filled(np.nan)
t0[t0 == 255] = np.nan
C = (slice(16, 248), slice(32, 336))
dc.draw_change(t0[C], runs["exact"][C], out / "exact.png", legend=False, width=560)
dc.draw_change(t0[C], runs["tiled 256"][C], out / "tiled.png", legend=False, width=560)
dc.draw_change(t0[C], runs["tiled 2 workers"][C], out / "tiled_workers.png", legend=False, width=560)
dc.legend_strip(out / "change_legend.png", [("Unchanged", dc.UNCHANGED)] + [(f"to {n.lower()}", c) for k, (n, c) in
                                                                             dc.SIB.items() if k in (2, 3, 4)],
                width=640)
shutil.rmtree(w / "coord", ignore_errors=True)
print("done")
