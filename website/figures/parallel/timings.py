"""Timings of the guide "Parallel and distributed computing": how the time of a job changes with the cores given.

On Sibuyan's model (Dinamica EGO's classic model, 14 steps) enlarged so that a run takes seconds:

- one run of the exact engine (`terraflux run`, 512 x 432 cells) with --threads 1 ... 19;
- a batch of 24 replicates of it (`terraflux submit --replicates 24`) with --workers 1 ... 19;
- one run of the tiled engine (`terraflux run --tiled --tile 256`, 1024 x 864 cells, the patches' settings given)
  with --threads 1 ... 19.

Each time is the shorter of `REPEAT` runs. Writes website/static/data/guide/parallel/{timings,speedup}.csv and
machine.txt. Takes about ten minutes on a machine of 20 threads.
"""

from __future__ import annotations

import csv
import os
import platform
import subprocess

from _common import DATA, dinamica, given_patches, tf, work

REPEAT = 2
CORES = [1, 2, 4, 8, 12, 19]

w = work("timings", clean=False)
exact = dinamica(w, 4)                                  # 512 x 432 cells
tiled = given_patches(dinamica(w, 8), w / "given8.toml")  # 1024 x 864 cells


def best(*args: str) -> float:
    return min(tf(*args, cwd=w)[0] for _ in range(REPEAT))


rows = []
for n in CORES:
    s_exact = best("run", exact.name, "--threads", str(n), "-q", "--out", "out_exact")
    s_batch = best("submit", exact.name, "--replicates", "24", "--workers", str(n), "--out", "out_batch")
    s_tiled = best("run", tiled.name, "--tiled", "--tile", "256", "--threads", str(n), "-q", "--out", "out_tiled")
    rows.append({"cores": n, "exact": round(s_exact, 2), "batch": round(s_batch, 2), "tiled": round(s_tiled, 2)})
    print(rows[-1], flush=True)

with open(DATA / "timings.csv", "w", newline="", encoding="utf-8") as f:
    wr = csv.DictWriter(f, fieldnames=["cores", "exact", "batch", "tiled"])
    wr.writeheader()
    wr.writerows(rows)

b = rows[0]
speed = [{"cores": r["cores"], "batch": round(b["batch"] / r["batch"], 2), "tiled": round(b["tiled"] / r["tiled"], 2),
          "exact": round(b["exact"] / r["exact"], 2), "ideal": r["cores"]} for r in rows]
with open(DATA / "speedup.csv", "w", newline="", encoding="utf-8") as f:
    wr = csv.DictWriter(f, fieldnames=["cores", "ideal", "batch", "tiled", "exact"])
    wr.writeheader()
    wr.writerows(speed)


def cpu_name() -> str:
    try:
        r = subprocess.run(["powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_Processor).Name"],
                           capture_output=True, text=True)
        return r.stdout.strip() or platform.processor()
    except OSError:
        return platform.processor()


(DATA / "machine.txt").write_text(f"{cpu_name()}; {os.cpu_count()} logical processors; {platform.platform()}\n",
                                  encoding="utf-8")
print((DATA / "machine.txt").read_text())
