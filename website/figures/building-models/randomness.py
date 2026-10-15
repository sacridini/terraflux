"""Figures of 'Randomness and reproducibility': a neutral landscape drawn with eight seeds, the core forest of 100
replicates run by `terraflux submit --replicates` (its spread, and the mean as replicates accumulate), the same seed
giving the same bits whatever the threads, and a batch of scenarios."""

import hashlib
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "building-models/randomness"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

MODEL = '''format = "terraflux/1"
name = "Core of a random forest"

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.55, weights = [0.3, 0.15, 0.55], seed = 7 }
note = "Three classes drawn by chance: 1 farming, 2 water, 3 forest (55 %)."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "landscape.map" }
params = { foreground = [3], edge_width = 1 }
note = "MSPA of the forest (class 3)."

[[nodes]]
id = "classes"
op = "metrics.class"
inputs = { map = "mspa.map" }
params = { metrics = ["pland"], show = "pland" }
note = "The share of the map of every MSPA class (pland, %)."

[outputs.mspa]
from = "mspa.map"

[outputs.classes]
from = "classes.table"

[outputs.landscape]
from = "landscape.map"
'''
(work / "model.toml").write_text(MODEL, encoding="utf-8")
log = []


def cli(*args):
    text = figs.cli(*args, cwd=work)
    log.append("$ terraflux " + " ".join(args) + "\n" + text.strip() + "\n")
    return text


N = 100
cli("submit", "model.toml", "--replicates", str(N), "--seed-input", "landscape.seed", "--out", "replicates")


def core_share(folder):
    rows = figs.read_csv(folder / "classes.csv")
    d = {int(float(r["class"])): float(r["pland"]) for r in rows}
    forest = sum(v for k, v in d.items() if k not in (0, 100, 129, 220))
    return d.get(17, 0.0) + d.get(117, 0.0), forest


rows, running = [], []
for s in range(1, N + 1):
    core, forest = core_share(work / "replicates" / f"seed_{s}")
    rows.append({"seed": s, "core": round(core, 2), "forest": round(forest, 2)})
cores = np.array([r["core"] for r in rows])
for k in range(1, N + 1):
    m = cores[:k].mean()
    se = cores[:k].std(ddof=1) / np.sqrt(k) if k > 1 else 0
    running.append({"replicates": k, "mean": round(m, 3), "low": round(m - 1.96 * se, 3), "high": round(m + 1.96 * se, 3)})
figs.write_csv(data / "replicates.csv", rows)
figs.write_csv(data / "running.csv", running)
print("core % mean", cores.mean(), "sd", cores.std(ddof=1), "min", cores.min(), "max", cores.max())
print("q2.5 q97.5", np.quantile(cores, [0.025, 0.975]))
hist, edges = np.histogram(cores, bins=np.arange(np.floor(cores.min()), np.ceil(cores.max()) + 1, 1.0))
figs.write_csv(data / "histogram.csv", [{"core": f"{edges[i]:.0f}-{edges[i + 1]:.0f}", "replicates": int(h)}
                                         for i, h in enumerate(hist)])

# the first eight seeds: the landscape and its MSPA
land = {1: ("Farming", "#d9a441"), 2: ("Water", "#3f7fae"), 3: ("Forest", "#3b8a5a")}
frames = []
for s in range(1, 9):
    d = work / "replicates" / f"seed_{s}"
    figs.raster(d / "landscape.tif", out / f"land_{s}.png", palette=land, width=520, legend=False)
    figs.raster(d / "mspa.tif", out / f"mspa_{s}.png", palette=figs.mspa_palette(), width=520, legend=False)
    frames.append((s, rows[s - 1]))
print("frames", frames)

# the same seed, twice, with 1 thread and with all: the same bits
digests = []
for name, extra in (("a", []), ("b", ["--threads", "1"])):
    figs.run(MODEL, work / name if (work / name).mkdir() is None else work / name, *extra)
    a, _ = figs.read(work / name / "output" / "mspa.tif")
    digests.append(hashlib.sha256(np.ascontiguousarray(a.filled(-1)).tobytes()).hexdigest()[:16])
log.append(f"# mspa.tif of seed 7, all threads: {digests[0]}; one thread: {digests[1]}\n")

# scenarios: the edge width and the seed changed together
(work / "scenarios.toml").write_text('''[[scenario]]
name = "edge1"
set = { "mspa.edge_width" = 1 }

[[scenario]]
name = "edge2"
set = { "mspa.edge_width" = 2 }

[[scenario]]
name = "edge3_seed2"
set = { "mspa.edge_width" = 3, "landscape.seed" = 2 }
''', encoding="utf-8")
cli("submit", "model.toml", "--scenarios", "scenarios.toml", "--out", "scenarios")
for sc in ("edge1", "edge2", "edge3_seed2"):
    print(sc, core_share(work / "scenarios" / sc))
(work / "cli.txt").write_text("\n".join(log), encoding="utf-8")
print("\n".join(log))

# the slider's frames: the landscape and its MSPA side by side
from PIL import Image  # noqa: E402

for s in range(1, 9):
    a = Image.open(out / f"land_{s}.png").convert("RGB")
    b = Image.open(out / f"mspa_{s}.png").convert("RGB")
    c = Image.new("RGB", (a.width + b.width + 20, max(a.height, b.height)), "white")
    c.paste(a, (0, 0))
    c.paste(b, (a.width + 20, 0))
    c.save(out / f"seed_{s}.png", optimize=True)
    (out / f"land_{s}.png").unlink()
    (out / f"mspa_{s}.png").unlink()
