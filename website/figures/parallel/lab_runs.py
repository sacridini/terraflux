"""Real runs for the guide's pages on a coordinator and its workers (content/parallel/lab-computers.md):

1. a coordinator with a pairing code on this computer (listening on 127.0.0.1 only: no beacons on the network),
   two workers given the code, `terraflux workers` and `terraflux jobs`, a batch submitted with the code;
2. a worker stopped while it runs a task: when the task goes to the other worker (the coordinator's --timeout),
   and when the batch ends (writes data/guide/parallel/failure.csv for diagrams.py's failure figure);
3. a batch left to run (--detach) and fetched later (`terraflux results`).

Every process started here is stopped at the end.

    python website/figures/parallel/lab_runs.py
"""

from __future__ import annotations

import csv
import re
import subprocess
import time

from _common import DATA, TF, dinamica, work

w = work("lab", clean=True)
model = dinamica(w, 4)          # Sibuyan enlarged 4 times: a run takes about 4 s
exe = str(TF)
procs: list[subprocess.Popen] = []
log = open(w / "lab.log", "w", encoding="utf-8")


def start(*args, name):
    p = subprocess.Popen([exe, *map(str, args)], cwd=w, stdout=open(w / f"{name}.out", "w"), stderr=subprocess.STDOUT)
    procs.append(p)
    return p


def tf(*args, timeout=600):
    t = time.perf_counter()
    r = subprocess.run([exe, *map(str, args)], cwd=w, capture_output=True, text=True, encoding="utf-8",
                       timeout=timeout)
    out = (r.stdout + r.stderr).replace("terraflux.exe", "terraflux")
    log.write(f"$ terraflux {' '.join(map(str, args))}\n{out}\n")
    return time.perf_counter() - t, out


try:
    # ---- 1. a coordinator with a code
    serve = start("serve", "--listen", "127.0.0.1:7795", "--code", "--data", w / "coordinator", name="serve")
    time.sleep(3)
    text = (w / "serve.out").read_text(encoding="utf-8", errors="replace")
    print("serve printed:\n" + text)
    code = re.search(r"\b([0-9A-Z]{4}-[0-9A-Z]{4}-[0-9A-Z]{4})\b", text).group(1)
    for k in (1, 2):
        start("worker", "--coordinator", "127.0.0.1:7795", "--code", code, "--name", f"lab-pc-{k}", "--cache",
              w / f"cache{k}", "--threads", 4, name=f"worker{k}")
    time.sleep(4)
    s, o = tf("workers", "--coordinator", "127.0.0.1:7795", "--code", code)
    print("workers:\n" + o)
    s, o = tf("submit", model.name, "--replicates", 4, "--coordinator", "127.0.0.1:7795", "--code", code, "--out",
              "batch")
    print(f"4 replicates on 2 workers, with the code: {s:.1f} s\n" + o[-800:])
    print("tasks.csv:", (w / "batch" / "tasks.csv").read_text())
    s, o = tf("jobs", "--coordinator", "127.0.0.1:7795", "--code", code)
    print("jobs:\n" + o)
    # a wrong code is refused
    s, o = tf("workers", "--coordinator", "127.0.0.1:7795", "--code", "AAAA-BBBB-CCCC", timeout=60)
    print("with a wrong code:\n" + o)

    # ---- 2. a worker stopped while it runs a task
    sub = subprocess.Popen([exe, "submit", model.name, "--replicates", "2", "--first-seed", "11", "--coordinator",
                            "127.0.0.1:7795", "--code", code, "--out", "failure"], cwd=w, stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT, text=True, encoding="utf-8")
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < 60:
        s, o = tf("workers", "--coordinator", "127.0.0.1:7795", "--code", code)
        line = next((ln for ln in o.splitlines() if "lab-pc-1" in ln), "")
        if line and "idle" not in line:
            print("lab-pc-1 busy:", line)
            break
        time.sleep(0.3)
    time.sleep(0.5)
    victim = procs[1]                      # lab-pc-1
    victim.kill()
    t_kill = time.perf_counter()
    print("lab-pc-1 stopped")
    out, _ = sub.communicate(timeout=600)
    t_end = time.perf_counter() - t_kill
    print(out[-1500:])
    print(f"the batch ended {t_end:.1f} s after lab-pc-1 stopped")
    tasks = list(csv.DictReader(open(w / "failure" / "tasks.csv", encoding="utf-8")))
    print("tasks:", tasks)
    with open(DATA / "failure.csv", "w", newline="", encoding="utf-8") as f:
        wr = csv.writer(f)
        wr.writerow(["event", "seconds_after_stop"])
        wr.writerow(["batch_done", round(t_end, 1)])
        for t in tasks:
            wr.writerow([f"{t['task']} on {t['worker']} ({t['seconds']} s)", ""])
    s, o = tf("workers", "--coordinator", "127.0.0.1:7795", "--code", code)
    print("workers after:\n" + o)

    # ---- 3. a batch left to run, fetched later
    s, o = tf("submit", model.name, "--replicates", 3, "--first-seed", 21, "--coordinator", "127.0.0.1:7795",
              "--code", code, "--detach")
    print("detached:\n" + o)
    job = re.search(r"(job-\d+)", o).group(1)
    s, o = tf("jobs", "--coordinator", "127.0.0.1:7795", "--code", code)
    print("jobs:\n" + o)
    s, o = tf("results", job, "--coordinator", "127.0.0.1:7795", "--code", code, "--out", "later")
    print(f"results ({s:.1f} s):\n" + o[-600:])
finally:
    for p in procs:
        if p.poll() is None:
            p.terminate()
    for p in procs:
        try:
            p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            p.kill()
    log.close()
    print("stopped every process started here")
