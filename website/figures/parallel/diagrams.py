"""Explanatory drawings of the guide "Parallel and distributed computing": how the cores, the machines, a
coordinator and its workers, a cluster, the cloud and the tiled engine fit together. Drawn with matplotlib in the
site's style (no run of Terraflux, except the tiles' figure, which draws a real map of the tiled engine when
timings.py has made it).

    python website/figures/parallel/diagrams.py
"""

from __future__ import annotations

import numpy as np
import matplotlib.pyplot as plt
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch, Rectangle

from _common import DATA, IMG, SIB, figs, work

INK, INK2, RULE = figs.INK, figs.INK2, figs.RULE
GREEN, EMBER, BLUE = figs.FOREST, figs.EMBER, figs.WATER
PALE = {"green": "#e7f1ea", "ember": "#fbeedd", "blue": "#e3edf5", "grey": "#f1f3f1"}


def canvas(w_px: int, h_px: int, xlim: float, ylim: float):
    fig = plt.figure(figsize=(w_px / 150, h_px / 150), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_xlim(0, xlim)
    ax.set_ylim(0, ylim)
    ax.set_axis_off()
    ax.set_aspect("equal")
    return fig, ax


def box(ax, x, y, w, h, title=None, text=None, fill="grey", edge=INK2, size=9.5, bold=True, r=0.08, lw=1.0,
        ls="-", tcolor=INK, valign="center"):
    ax.add_patch(FancyBboxPatch((x, y), w, h, boxstyle=f"round,pad=0,rounding_size={r}",
                                facecolor=PALE.get(fill, fill), edgecolor=edge, linewidth=lw, linestyle=ls))
    if title and text:
        ax.text(x + w / 2, y + h - 0.13, title, ha="center", va="top", fontsize=size, fontweight="bold", color=tcolor)
        ax.text(x + w / 2, y + h - 0.42, text, ha="center", va="top", fontsize=size - 1.5, color=INK2,
                linespacing=1.35)
    elif title:
        ax.text(x + w / 2, y + h / 2 if valign == "center" else y + h - 0.13, title, ha="center",
                va="center" if valign == "center" else "top", fontsize=size,
                fontweight="bold" if bold else "normal", color=tcolor, linespacing=1.3)


def arrow(ax, a, b, color=INK2, text=None, off=(0, 0.12), size=8.5, both=False, ls="-", rad=0.0, lw=1.2,
          ha="center", tcolor=None):
    ax.add_patch(FancyArrowPatch(a, b, arrowstyle="<|-|>" if both else "-|>", mutation_scale=11, color=color,
                                 linewidth=lw, linestyle=ls, connectionstyle=f"arc3,rad={rad}", shrinkA=2, shrinkB=2))
    if text:
        mx, my = (a[0] + b[0]) / 2 + off[0], (a[1] + b[1]) / 2 + off[1]
        ax.text(mx, my, text, ha=ha, va="bottom", fontsize=size, color=tcolor or color, linespacing=1.3)


def save(fig, name):
    p = IMG / name
    fig.savefig(p, dpi=150, facecolor="white")
    plt.close(fig)
    figs._shrink(p)
    print("wrote", p)


def cores(ax, x, y, n_cols, n_rows, s=0.16, gap=0.05, color=GREEN):
    for i in range(n_rows):
        for j in range(n_cols):
            ax.add_patch(Rectangle((x + j * (s + gap), y + i * (s + gap)), s, s, facecolor=color, edgecolor="none"))


# ------------------------------------------------------------------------------------------------ scales


def scales():
    fig, ax = canvas(1100, 290, 16.5, 4.35)
    # a core
    box(ax, 0.2, 0.9, 2.2, 3.2, fill="white", edge=RULE)
    ax.add_patch(Rectangle((0.95, 2.1), 0.7, 0.7, facecolor=GREEN))
    ax.text(1.3, 3.75, "A core", ha="center", fontsize=10.5, fontweight="bold", color=INK)
    ax.text(1.3, 1.75, "does one thing\nat a time", ha="center", va="top", fontsize=8.5, color=INK2)
    # a computer
    box(ax, 2.8, 0.9, 3.0, 3.2, fill="white", edge=RULE)
    ax.text(4.3, 3.75, "A computer (a node)", ha="center", fontsize=10, fontweight="bold", color=INK)
    ax.add_patch(FancyBboxPatch((3.2, 2.0), 2.2, 1.35, boxstyle="round,pad=0,rounding_size=0.06",
                                facecolor=PALE["green"], edgecolor=GREEN, linewidth=0.8))
    cores(ax, 3.4, 2.15, 7, 4, s=0.2, gap=0.07)
    ax.add_patch(Rectangle((3.2, 1.5), 2.2, 0.3, facecolor=PALE["blue"], edgecolor=BLUE, linewidth=0.8))
    ax.text(4.3, 1.65, "memory, shared", ha="center", va="center", fontsize=7.5, color=BLUE)
    ax.text(4.3, 1.3, "cores share its memory", ha="center", va="top", fontsize=8.5, color=INK2)
    # a lab
    box(ax, 6.2, 0.9, 3.2, 3.2, fill="white", edge=RULE)
    ax.text(7.8, 3.75, "The lab's computers", ha="center", fontsize=10.5, fontweight="bold", color=INK)
    for k, (cx, cy) in enumerate([(6.55, 2.45), (7.35, 2.45), (8.15, 2.45), (8.95, 2.45)]):
        ax.add_patch(Rectangle((cx - 0.3, cy), 0.6, 0.6, facecolor=PALE["green"], edgecolor=GREEN, linewidth=0.8))
        cores(ax, cx - 0.22, cy + 0.08, 3, 3, s=0.12, gap=0.04)
        ax.plot([cx, cx], [cy, 1.95], color=INK2, lw=0.8)
    ax.plot([6.55, 8.95], [1.95, 1.95], color=INK2, lw=1.2)
    ax.text(7.8, 1.75, "a network joins them;\neach has its own memory", ha="center", va="top", fontsize=8.5,
            color=INK2)
    # a cluster
    box(ax, 9.8, 0.9, 3.4, 3.2, fill="white", edge=RULE)
    ax.text(11.5, 3.75, "A cluster (HPC)", ha="center", fontsize=10.5, fontweight="bold", color=INK)
    ax.add_patch(Rectangle((10.05, 2.55), 0.75, 0.6, facecolor=PALE["ember"], edgecolor=EMBER, linewidth=0.8))
    ax.text(10.43, 2.85, "login", ha="center", va="center", fontsize=7.5, color=EMBER)
    for i in range(3):
        for j in range(3):
            x0, y0 = 11.05 + j * 0.7, 2.15 + i * 0.42
            ax.add_patch(Rectangle((x0, y0), 0.6, 0.34, facecolor=PALE["green"], edgecolor=GREEN, linewidth=0.7))
            cores(ax, x0 + 0.05, y0 + 0.06, 6, 2, s=0.07, gap=0.025)
    ax.text(11.5, 1.75, "hundreds of nodes,\nshared by many people", ha="center", va="top", fontsize=8.5,
            color=INK2)
    # the cloud
    box(ax, 13.6, 0.9, 2.7, 3.2, fill="white", edge=RULE)
    ax.text(14.95, 3.75, "The cloud", ha="center", fontsize=10.5, fontweight="bold", color=INK)
    ax.add_patch(FancyBboxPatch((13.85, 2.0), 2.2, 1.3, boxstyle="round,pad=0,rounding_size=0.3",
                                facecolor=PALE["blue"], edgecolor=BLUE, linewidth=0.8, linestyle=(0, (3, 2))))
    for k, cx in enumerate([14.2, 14.75, 15.3]):
        ax.add_patch(Rectangle((cx, 2.35), 0.45, 0.55, facecolor=PALE["green"], edgecolor=GREEN, linewidth=0.8))
        cores(ax, cx + 0.05, 2.42, 3, 4, s=0.09, gap=0.03)
    ax.text(14.95, 1.75, "machines rented\nby the hour", ha="center", va="top", fontsize=8.5, color=INK2)
    # scale arrow
    arrow(ax, (0.4, 0.45), (16.1, 0.45), color=INK2)
    ax.text(0.4, 0.55, "more cores, more memory in all, more to set up", fontsize=8.5, color=INK2, va="bottom")
    save(fig, "scales.png")


# ------------------------------------------------------------------------------------------------ Amdahl


def amdahl():
    n = np.unique(np.round(np.logspace(0, 7, 200, base=2)).astype(int))

    def draw(ax):
        for p, c in [(0.99, GREEN), (0.95, BLUE), (0.9, "#785ef0"), (0.75, EMBER), (0.5, "#b4508a")]:
            s = 1 / ((1 - p) + p / n)
            ax.plot(n, s, color=c, lw=2, label=f"{int(p * 100)}% of the work can be split")
        ax.plot(n, n, color=INK2, lw=1, ls=(0, (3, 3)), label="ideal: n cores, n times faster")
        ax.set_xscale("log", base=2)
        ax.set_yscale("log", base=2)
        ax.set_xticks([1, 2, 4, 8, 16, 32, 64, 128])
        ax.set_xticklabels(["1", "2", "4", "8", "16", "32", "64", "128"])
        ax.set_yticks([1, 2, 4, 8, 16, 32, 64])
        ax.set_yticklabels(["1x", "2x", "4x", "8x", "16x", "32x", "64x"])
        ax.set_ylim(0.9, 80)
        ax.set_xlabel("Cores")
        ax.set_ylabel("Times faster than one core")
        ax.legend(loc="upper left", fontsize=8.5)

    figs.chart(IMG / "amdahl.png", draw, width=900, height=470)


# ------------------------------------------------------------------------------------------------ kinds of work


def kinds():
    fig, ax = canvas(1100, 300, 16.5, 4.5)
    rng = np.random.default_rng(4)
    panels = [("Replicates", "the same model,\nanother seed each"), ("Scenarios", "variants of the model,\none run each"),
              ("Calibration", "settings tried in ranges,\neach scored"), ("Tiles", "one large map\ncut in pieces")]
    for k, (t, s) in enumerate(panels):
        x0 = 0.2 + k * 4.1
        box(ax, x0, 0.2, 3.8, 4.1, fill="white", edge=RULE)
        ax.text(x0 + 1.9, 3.95, t, ha="center", va="top", fontsize=10.5, fontweight="bold", color=INK)
        ax.text(x0 + 1.9, 0.9, s, ha="center", va="top", fontsize=8.5, color=INK2, linespacing=1.3)
        if k < 2:
            for i in range(4):
                xx = x0 + 0.35 + i * 0.82
                a = rng.random((6, 6))
                if k == 0:
                    a = (a > 0.45).astype(float)
                elif k == 1:
                    a = (a > 0.25 + 0.17 * i).astype(float)
                ax.imshow(a, extent=(xx, xx + 0.7, 1.2, 1.9), cmap="Greens" if k < 2 else "viridis", vmin=-0.3,
                          vmax=1.2, interpolation="nearest", zorder=2)
                lab = [f"seed {i + 1}", "ABCD"[i], ""][k] if k < 2 else ""
                if lab:
                    ax.text(xx + 0.35, 2.0, lab, ha="center", va="bottom", fontsize=7.5, color=INK2)
            ax.add_patch(Rectangle((x0 + 1.2, 2.45), 1.4, 0.75, facecolor=PALE["ember"], edgecolor=EMBER, lw=0.8))
            ax.text(x0 + 1.9, 2.82, "model", ha="center", va="center", fontsize=8.5, color=EMBER)
            for i in range(4):
                arrow(ax, (x0 + 1.9, 2.45), (x0 + 0.7 + i * 0.82, 2.25), color=INK2, lw=0.7)
        elif k == 2:
            g = np.array([[0.62, 0.7, 0.66], [0.71, 0.77, 0.69], [0.64, 0.68, 0.6]])
            ax.imshow(g, extent=(x0 + 0.9, x0 + 2.9, 1.2, 3.2), cmap="viridis", interpolation="nearest", zorder=3)
            ax.text(x0 + 1.9, 2.2, "best", ha="center", va="center", fontsize=7.5, color=INK, zorder=4)
            ax.text(x0 + 0.8, 2.2, "setting a", ha="right", va="center", fontsize=7.5, color=INK2, rotation=90)
            ax.text(x0 + 1.9, 3.27, "setting b", ha="center", va="bottom", fontsize=7.5, color=INK2)
            ax.add_patch(Rectangle((x0 + 0.9, 1.2), 2.0, 2.0, fill=False, edgecolor=INK2, lw=0.6, zorder=4))
        else:
            a = rng.random((24, 24))
            from scipy.ndimage import uniform_filter

            a = uniform_filter(a, 5) > 0.5
            ax.imshow(a, extent=(x0 + 0.6, x0 + 3.2, 1.15, 3.55), cmap="Greens", vmin=-0.3, vmax=1.2,
                      interpolation="nearest", zorder=2)
            for t_ in np.linspace(0, 2.6, 4):
                ax.plot([x0 + 0.6 + t_, x0 + 0.6 + t_], [1.15, 3.55], color=EMBER, lw=1.2, zorder=3)
            for t_ in np.linspace(0, 2.4, 4):
                ax.plot([x0 + 0.6, x0 + 3.2], [1.15 + t_, 1.15 + t_], color=EMBER, lw=1.2, zorder=3)
    save(fig, "kinds.png")


# ------------------------------------------------------------------------------------------------ coordinator


def coordinator():
    fig, ax = canvas(1100, 520, 16.5, 7.8)
    box(ax, 0.2, 2.6, 3.6, 2.6, "Your computer",
        "terraflux submit model.toml\n--replicates 200\n--coordinator host:7700", fill="ember", edge=EMBER, size=10)
    box(ax, 6.7, 2.3, 4.0, 3.2, "The coordinator",
        "terraflux serve\n\nthe queue of tasks\nthe data, by hash (once)\nthe results' files\nits jobs, on its disk",
        fill="blue", edge=BLUE, size=10)
    for k, y in enumerate([5.9, 3.45, 1.0]):
        box(ax, 12.7, y, 3.6, 1.55, ["Worker: lab-pc-1", "Worker: lab-pc-2", "Worker: a cluster node"][k],
            "terraflux worker\ncache: the data it fetched", fill="green", edge=GREEN, size=9.5)
        arrow(ax, (12.7, y + 0.78), (10.7, 3.9 + (k - 1) * -0.55), color=GREEN, lw=1.3)
    ax.text(8.7, 5.75, "workers call the coordinator: they need no open port", ha="center", va="bottom",
            fontsize=8.5, color=GREEN)
    arrow(ax, (3.8, 4.3), (6.7, 4.3), color=EMBER, text="the model, each task's\nsettings, the files it reads",
          size=8.5)
    arrow(ax, (6.7, 3.3), (3.8, 3.3), color=BLUE, text=None)
    ax.text(5.25, 3.15, "each run's files,\nas soon as it is done", ha="center", va="top", fontsize=8.5, color=BLUE)
    ax.text(8.7, 1.75, "a worker's cycle:", ha="center", fontsize=8.5, fontweight="bold", color=INK2)
    ax.text(8.7, 1.55, "1 ask for a task   2 fetch the files it lacks\n3 run, a heartbeat every 5 s\n"
            "4 send the files back   5 ask again", ha="center", va="top", fontsize=8.5, color=INK2, linespacing=1.4)
    save(fig, "coordinator.png")


# ------------------------------------------------------------------------------------------------ pairing code


def pairing():
    fig, ax = canvas(1100, 560, 16.5, 8.4)
    xs = {"w": 3.0, "c": 13.0}
    box(ax, 1.2, 7.3, 3.6, 0.8, "Worker (or client)", fill="green", edge=GREEN)
    box(ax, 11.2, 7.3, 3.6, 0.8, "Coordinator", fill="blue", edge=BLUE)
    for x in xs.values():
        ax.plot([x, x], [0.3, 7.3], color=RULE, lw=1.5)
    steps = [
        ("c", "w", "1  a beacon every 2 s, by UDP to the port 7701 of the network:\n"
                   "    its port and a fingerprint made from the code (not the code)", 6.2),
        ("w", "c", "2  a TLS connection to that port (encrypted from here on)", 5.2),
        ("c", "w", "3  proof: an HMAC of its certificate made with the key of the code;\n"
                   "    the worker checks it and pins the certificate", 4.0),
        ("w", "c", "4  every call carries a token made from the key", 3.0),
        ("c", "w", "5  tasks, data, results: only between computers given the code", 2.1),
    ]
    for a, b, text, y in steps:
        col = BLUE if a == "c" else GREEN
        arrow(ax, (xs[a], y), (xs[b], y), color=col, lw=1.4)
        ax.text(8.0, y + 0.1, text, ha="center", va="bottom", fontsize=8.8, color=INK, linespacing=1.35)
    ax.text(8.0, 1.2, "The code (e.g. K7QM-2XPA-9DTE): 12 characters, 60 bits; the key is made from it slowly\n"
            "(PBKDF2, 100 000 rounds), so what travels on the network does not give it away.",
            ha="center", va="top", fontsize=8.5, color=INK2, linespacing=1.4)
    save(fig, "pairing.png")


# ------------------------------------------------------------------------------------------------ a worker vanishes


def failure():
    """From a real run (lab_runs.py: a coordinator on this computer, two workers, 2 replicates of 432 x 512 cells of
    about 3.5 s each): lab-pc-1 was stopped while running seed_11; lab-pc-2 ended seed_12 about 3 s later; after
    60 s without a heartbeat (--timeout) seed_11 went back to the queue and lab-pc-2 ran it (3.05 s): the batch
    ended 63.2 s after the stop (data/guide/parallel/failure.csv)."""
    end = 63.2
    import csv as _csv
    try:
        rows = list(_csv.DictReader(open(DATA / "failure.csv", encoding="utf-8")))
        end = float(next(r["seconds_after_stop"] for r in rows if r["event"] == "batch_done"))
    except (OSError, StopIteration, ValueError):
        pass
    rerun = 3.05

    def draw(ax):
        ax.set_xlim(-17, 72)
        ax.set_ylim(-0.6, 2.2)
        ax.barh(1.5, 15, left=-15, height=0.45, color=GREEN, alpha=0.35)
        ax.text(-14.6, 1.5, "lab-pc-1: seed_11", va="center", fontsize=8.5, color=INK)
        ax.plot([0, 0], [-0.5, 2.0], color="#d55e00", lw=1.5)
        ax.text(0.8, 1.95, "lab-pc-1 stopped", color="#d55e00", fontsize=8.5, va="top")
        ax.barh(0.6, 18, left=-15, height=0.45, color=BLUE, alpha=0.5)
        ax.text(-14.6, 0.6, "lab-pc-2: seed_12", va="center", fontsize=8.5, color=INK)
        ax.barh(0.6, rerun, left=end - rerun, height=0.45, color=GREEN, alpha=0.6)
        ax.text(end - rerun - 0.6, 0.6, "lab-pc-2: seed_11 again", va="center", ha="right", fontsize=8.5, color=INK)
        ax.annotate("", xy=(end - rerun, 1.15), xytext=(0, 1.15), arrowprops=dict(arrowstyle="<->", color=INK2, lw=1))
        ax.text((end - rerun) / 2, 1.22, "no heartbeat for 60 s (--timeout): the task goes back to the queue",
                ha="center", fontsize=8.5, color=INK2)
        ax.plot([end, end], [-0.5, 1.0], color=INK2, lw=1, ls=(0, (2, 2)))
        ax.text(end - 0.5, -0.35, f"batch done: {end:.0f} s", ha="right", fontsize=8.5, color=INK2)
        ax.set_yticks([])
        ax.set_xlabel("Seconds after lab-pc-1 stopped")
        ax.grid(axis="y", visible=False)
        ax.spines["left"].set_visible(False)

    figs.chart(IMG / "failure.png", draw, width=1000, height=330)


# ------------------------------------------------------------------------------------------------ a cluster


def hpc():
    fig, ax = canvas(1100, 560, 16.5, 8.4)
    box(ax, 0.2, 3.2, 3.5, 2.4, "Your computer",
        "terraflux submit model.toml\n--replicates 500 --on hpc", fill="ember", edge=EMBER, size=10)
    box(ax, 5.6, 4.6, 4.4, 3.3, "Login node",
        "the coordinator, started for\nthe batch (serve --code),\nits jobs in ~/.terraflux\n\nsbatch: the workers' job",
        fill="blue", edge=BLUE, size=10)
    box(ax, 5.6, 0.5, 4.4, 2.4, "The scheduler (Slurm)",
        "queues the jobs of everyone;\ngives the nodes asked for\n(--nodes, --cpus, --time)", fill="grey",
        edge=INK2, size=10)
    ax.add_patch(FancyBboxPatch((11.6, 0.5), 4.7, 7.4, boxstyle="round,pad=0,rounding_size=0.1",
                                facecolor="white", edgecolor=GREEN, linewidth=1.0, linestyle=(0, (4, 2))))
    ax.text(13.95, 7.6, "The allocation's nodes", ha="center", va="top", fontsize=10, fontweight="bold",
            color=INK)
    for k in range(4):
        y = 5.6 - k * 1.45
        ax.add_patch(FancyBboxPatch((12.0, y), 3.9, 1.1, boxstyle="round,pad=0,rounding_size=0.06",
                                    facecolor=PALE["green"], edgecolor=GREEN, linewidth=0.8))
        ax.text(13.95, y + 0.55, f"node {k + 1}: 2 workers", ha="center", va="center", fontsize=8.8, color=INK)
        arrow(ax, (12.0, y + 0.55), (10.0, 6.1 - k * 0.25), color=GREEN, lw=0.9)
    arrow(ax, (3.7, 4.7), (5.6, 6.0), color=EMBER)
    ax.text(3.9, 6.0, "SSH (port 22):\nstart, then\na tunnel", ha="center", va="bottom", fontsize=8.5,
            color=EMBER)
    arrow(ax, (7.8, 4.6), (7.8, 2.9), color=INK2, text=None)
    ax.text(7.95, 3.75, "sbatch", fontsize=8.5, color=INK2, va="center")
    arrow(ax, (10.0, 1.7), (11.6, 1.7), color=INK2)
    ax.text(10.8, 1.85, "starts", fontsize=8.5, color=INK2, ha="center")
    save(fig, "hpc.png")


# ------------------------------------------------------------------------------------------------ the cloud


def cloud():
    fig, ax = canvas(1100, 500, 16.5, 7.5)
    box(ax, 0.3, 2.6, 3.0, 2.2, "Your computer", "terraflux submit ...\n--on cloud", fill="ember", edge=EMBER,
        size=10)
    ax.add_patch(FancyBboxPatch((5.0, 0.3), 11.2, 6.9, boxstyle="round,pad=0,rounding_size=0.4",
                                facecolor="#f6f9fc", edgecolor=BLUE, linewidth=1.0, linestyle=(0, (5, 3))))
    ax.text(10.6, 6.95, "The provider's private network (e.g. 10.0.0.0/24)", ha="center", va="top", fontsize=9.5,
            color=BLUE, fontweight="bold")
    box(ax, 5.6, 2.3, 4.2, 3.4, "The server",
        "public 203.0.113.7, private 10.0.0.5\ncloud-init: Terraflux installed\n\nfor each batch: the coordinator\n"
        "and --workers on it", fill="blue", edge=BLUE, size=10)
    for k in range(3):
        y = 5.0 - k * 1.95
        box(ax, 11.6, y, 4.2, 1.5, f"Worker machine {k + 1}",
            "cloud-init --worker:\na service terraflux-worker", fill="green", edge=GREEN, size=9.5)
        arrow(ax, (11.6, y + 0.75), (9.8, 4.0 - (k - 1) * 0.6), color=GREEN, lw=1.0)
    ax.text(10.55, 1.0, "workers call\n10.0.0.5:<port>", ha="center", fontsize=8.5, color=GREEN)
    arrow(ax, (3.3, 3.7), (5.6, 3.7), color=EMBER)
    ax.text(4.15, 3.85, "SSH 22, from\nyour address", ha="center", va="bottom", fontsize=8.5, color=EMBER)
    ax.text(10.6, 0.55, "Firewall: SSH from your address; the coordinator's port only from the private network",
            ha="center", fontsize=8.5, color=INK2)
    save(fig, "cloud.png")


# ------------------------------------------------------------------------------------------------ tiles


def tiles():
    """A real map of the tiled engine (timings.py's given8, step 14) with its tiles of 256 cells and a halo."""
    w = work("timings", clean=False)
    src = w / "out_tiled" / "maps" / "step_014.tif"
    if not src.exists():
        print("tiles: run timings.py first")
        return
    a, _ = figs.read(src)
    a = np.ma.masked_where((a == 255) | a.mask, a)
    rgba = np.ones(a.shape + (4,))
    for v, (_, c) in SIB.items():
        import matplotlib.colors as mc

        rgba[a.filled(0) == v] = mc.to_rgba(c)
    rgba[a.mask] = (1, 1, 1, 0)
    h, wd = a.shape
    fig = plt.figure(figsize=(1000 / 150, 1000 / 150 * h / wd * 0.98), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgba, interpolation="nearest")
    for x in range(0, wd + 1, 256):
        ax.axvline(x - 0.5, color=INK, lw=0.8, alpha=0.7)
    for y in range(0, h + 1, 256):
        ax.axhline(y - 0.5, color=INK, lw=0.8, alpha=0.7)
    tx, ty, r = 512, 256, 64
    ax.add_patch(Rectangle((tx - 0.5, ty - 0.5), 256, 256, fill=False, edgecolor=EMBER, lw=2.5))
    ax.add_patch(Rectangle((tx - r - 0.5, ty - r - 0.5), 256 + 2 * r, 256 + 2 * r, fill=False, edgecolor=EMBER,
                           lw=1.4, ls=(0, (4, 3))))
    ax.text(tx + 128, ty + 128, "a tile\n256 x 256", ha="center", va="center", fontsize=10, color="white",
            fontweight="bold")
    ax.text(tx + 256 + r + 6, ty - r + 4, "its halo: 64 cells\n(twice --radius)", ha="left", va="top", fontsize=9,
            color=INK, bbox=dict(facecolor="white", edgecolor="none", alpha=0.85, pad=2))
    ax.set_xlim(-0.5, wd - 0.5)
    ax.set_ylim(h - 0.5, -0.5)
    ax.set_axis_off()
    save(fig, "tiles.png")


def tiles_flow():
    fig, ax = canvas(1100, 470, 16.5, 7.05)
    box(ax, 0.3, 1.0, 4.4, 5.2, "Your machine",
        "(it runs the model)\n\nterraflux run model.toml\n--tiled --disk layers\n--coordinator host:7700\n\nfits the model;\n"
        "keeps each step's layers\nin --disk, a file per tile;\nmerges the parts of\nevery pass", fill="ember",
        edge=EMBER, size=10)
    box(ax, 6.3, 2.0, 3.8, 3.2, "Coordinator", "a pass = a job;\nits tasks: runs of\nneighbouring tiles\n(4 per worker)",
        fill="blue", edge=BLUE, size=10)
    for k, y in enumerate([4.9, 2.85, 0.8]):
        box(ax, 11.8, y, 4.4, 1.6, f"Worker {k + 1}", "fetches its tiles and halos,\nsends back the tiles it wrote",
            fill="green", edge=GREEN, size=9.5)
        arrow(ax, (11.8, y + 0.8), (10.1, 3.6 + (1 - k) * 0.5), color=GREEN, lw=1.0, both=True)
    arrow(ax, (4.7, 4.1), (6.3, 4.1), color=EMBER, text="what to compute;\nthe tiles' hashes", size=8.5, off=(0, 0.1))
    arrow(ax, (6.3, 3.0), (4.7, 3.0), color=BLUE)
    ax.text(5.5, 2.85, "sums: histograms,\ncounts; the new tiles", ha="center", va="top", fontsize=8.5, color=BLUE)
    ax.text(8.2, 0.75, "--shared-disk: the workers read and write\nthe layers themselves; only sums travel",
            ha="center", va="top", fontsize=8.5, color=INK2)
    save(fig, "tiles_flow.png")


if __name__ == "__main__":
    scales()
    amdahl()
    kinds()
    coordinator()
    pairing()
    failure()
    hpc()
    cloud()
    tiles()
    tiles_flow()
