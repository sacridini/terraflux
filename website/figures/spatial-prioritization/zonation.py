"""Figures of 'Ranking a landscape with Zonation' (spatial-prioritization/zonation).

Serra Verde ranked by prioritize.zonation: the rank map and its performance curves (a slider over the top
fraction), the marginal loss rules, costs, weights, the protected areas as a hierarchic mask, connectivity
kernels, habitat condition, and the same ranking from a Zonation 5 project (prioritize.zonation_project).
"""

import numpy as np
from scipy import ndimage

import sp_common as sp
from sp_common import figs, plt

SLUG = "zonation"
OUT = sp.img(SLUG)
DAT = sp.data(SLUG)
ALL8 = lambda v: "[" + ", ".join([v] * 8) + "]"  # noqa: E731


def zon(name, params="", inputs="", extra_nodes="", outputs=("curves", "auc", "features")):
    body = extra_nodes + f'''
[[nodes]]
id = "z"
op = "prioritize.zonation"
inputs = {{ features = "features.stack"{inputs} }}
params = {{ {params} }}
'''
    for o in outputs:
        body += f'\n[outputs.{o}]\nfrom = "z.{o}"\n'
    w = sp.run(SLUG, name, body, {"z.rank": "rank.tif"})
    return w, sp.grid(w / "rank.tif"), sp.table(w, "curves")


def kept(curves, top):
    """The share of every feature in the best `top` fraction of the cells (the curves at rank 1 - top)."""
    ranks = np.array([r["rank"] for r in curves])
    i = int(np.argmin(np.abs(ranks - (1 - top))))
    return {f: curves[i][f] for f in sp.FEATURES}, curves[i]


def rank_map(rank, png, title=None, width=900, outline=None):
    fig = plt.figure(figsize=(width / 150, width / 150 * 0.62), dpi=150)
    ax = fig.add_axes([0, 0, 0.86, 0.92 if title else 1])
    cm = plt.get_cmap("viridis").copy()
    cm.set_bad((1, 1, 1, 0))
    im = ax.imshow(np.ma.masked_invalid(rank), cmap=cm, vmin=0, vmax=1, interpolation="nearest")
    if outline is not None:
        ax.contour(outline.astype(float), levels=[0.5], colors="white", linewidths=1.2)
    ax.set_axis_off()
    cax = fig.add_axes([0.88, 0.12, 0.025, 0.7])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.ax.grid(False)
    cb.ax.tick_params(labelsize=8.5)
    cb.set_label("Rank (1: kept last)", fontsize=9, color=figs.INK2)
    if title:
        fig.text(0.0, 0.99, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    return sp.save(fig, png)


def top_frame(rank, top, png):
    """The best `top` share of the cells in green over the rest of the rank map in grey."""
    mask = sp.land()
    thr = 1 - top
    rgba = np.zeros(rank.shape + (4,))
    g = np.clip(np.nan_to_num(rank), 0, 1)
    shade = 0.93 - 0.25 * g
    rgba[mask] = np.stack([shade, shade, shade * 0.97, np.ones_like(shade)], axis=-1)[mask]
    from matplotlib import colors as mc
    rgba[mask & (np.nan_to_num(rank, nan=-1) >= thr)] = mc.to_rgba(sp.SELECTED)
    fig = plt.figure(figsize=(760 / 150, 760 / 150 * 60 / 90), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgba, interpolation="nearest")
    ax.set_axis_off()
    sp.save(fig, png)


def edges(sel):
    """The length (km) of the outline of a selection: cell sides between selected and other land or the sea."""
    a = np.pad(sel.astype(int), 1)
    return int(np.abs(np.diff(a, axis=0)).sum() + np.abs(np.diff(a, axis=1)).sum())


def patches(sel):
    lab, n = ndimage.label(sel, structure=np.ones((3, 3)))
    return n


# ------------------------------------------------------------------------------------------------ the base ranking
def base():
    w, rank, curves = zon("base", 'rule = "CAZ2"', ', cost = "cost.map"')
    auc = sp.number(w, "auc")
    rank_map(rank, OUT / "rank.png")
    print("base auc", auc, "iterations?")
    # the slider: frames at top fractions, the chart's rows the same fractions
    tops = [5, 10, 17, 20, 30, 40, 50, 60, 70, 80, 90]
    rows, frames = [], []
    for t in tops:
        k, row = kept(curves, t / 100)
        top_frame(rank, t / 100, OUT / f"top_{t}.png")
        cost_share = 1 - row["cost"] / curves[0]["cost"]
        sel = np.nan_to_num(rank, nan=-1) >= 1 - t / 100
        cost_kept = float(np.nansum(sp.grid(sp.DATA / "cost.tif") * sel)) / curves[0]["cost"]
        r = {"top": t}
        r.update({sp.SHORT[f]: round(100 * k[f], 1) for f in sp.FEATURES})
        rows.append(r)
        frames.append({"top": t, "mean": 100 * np.mean(list(k.values())), "min": 100 * min(k.values()),
                       "minf": sp.SHORT[min(k, key=k.get)], "cost": 100 * cost_kept})
    sp.write_csv(DAT / "top-fractions.csv", rows)
    # the whole curves, every 1 % of the cells, for a chart of their own
    full = []
    for t in range(0, 101, 1):
        k, row = kept(curves, t / 100)
        r = {"top": t}
        r.update({sp.SHORT[f]: round(100 * k[f], 2) for f in sp.FEATURES})
        r["Mean"] = round(100 * np.mean(list(k.values())), 2)
        full.append(r)
    sp.write_csv(DAT / "curves.csv", full)
    for f in frames:
        print(f"top {f['top']}%: mean {f['mean']:.1f}, min {f['min']:.1f} ({f['minf']}), cost {f['cost']:.1f}%")
    print({t: rows[i] for i, t in enumerate(tops) if t in (17, 30)})
    return rank, curves, frames, auc


# ------------------------------------------------------------------------------------------------ rules
def rules():
    out = {}
    imgs = []
    for rule, label in (("CAZMAX", "CAZMAX"), ("CAZ1", "CAZ1"), ("CAZ2", "CAZ2"), ("ABF", "ABF (z = 0.25)")):
        w, rank, curves = zon(f"rule_{rule}", f'rule = "{rule}"', ', cost = "cost.map"')
        out[rule] = (rank, curves, sp.number(w, "auc"))
        imgs.append((rank, label))
    fig, axs = plt.subplots(1, 4, figsize=(10.4, 2.35), dpi=150)
    cm = plt.get_cmap("viridis").copy()
    cm.set_bad((1, 1, 1, 0))
    for ax, (rank, label) in zip(axs, imgs):
        ax.imshow(np.ma.masked_invalid(rank), cmap=cm, vmin=0, vmax=1, interpolation="nearest")
        ax.set_axis_off()
        ax.set_title(label, loc="left", fontsize=10.5)
    fig.tight_layout(w_pad=0.5)
    sp.save(fig, OUT / "rules.png")
    rows = []
    for f in sp.FEATURES:
        r = {"feature": sp.SHORT[f]}
        for rule in out:
            r[rule] = round(100 * kept(out[rule][1], 0.17)[0][f], 1)
        rows.append(r)
    sp.write_csv(DAT / "rules-top17.csv", rows)
    for rule in out:
        k = kept(out[rule][1], 0.17)[0]
        print(rule, "auc", round(out[rule][2], 3), "top17 mean", round(100 * np.mean(list(k.values())), 1),
              "min", round(100 * min(k.values()), 1))
    return out


# ------------------------------------------------------------------------------------------------ costs and weights
def costs():
    _, with_cost, cw = zon("cost_on", 'rule = "CAZ2"', ', cost = "cost.map"')
    _, no_cost, cn = zon("cost_off", 'rule = "CAZ2"')
    fig, axs = plt.subplots(1, 2, figsize=(9.6, 3.2), dpi=150)
    cm = plt.get_cmap("viridis").copy()
    cm.set_bad((1, 1, 1, 0))
    for ax, r, t in ((axs[0], no_cost, "Without costs (area)"), (axs[1], with_cost, "With the cost of land")):
        ax.imshow(np.ma.masked_invalid(r), cmap=cm, vmin=0, vmax=1, interpolation="nearest")
        ax.set_axis_off()
        ax.set_title(t, loc="left", fontsize=11)
    fig.tight_layout()
    sp.save(fig, OUT / "cost.png")
    total = cw[0]["cost"]
    cost = sp.grid(sp.DATA / "cost.tif")
    rows = []
    for share in range(0, 101, 2):
        r = {"budget": share}
        for label, rank in (("Without costs", no_cost), ("With costs", with_cost)):
            # the best cells that fit in `share` % of the total cost
            order = np.argsort(-np.nan_to_num(rank, nan=-1), axis=None)
            c = np.nan_to_num(cost.ravel()[order])
            take = order[np.cumsum(c) <= share / 100 * total]
            sel = np.zeros(rank.size, bool)
            sel[take] = True
            fs = sp.features()
            r[label] = round(100 * np.mean([np.nansum(fs[f].ravel()[sel]) / np.nansum(fs[f]) for f in sp.FEATURES]), 2)
        rows.append(r)
    sp.write_csv(DAT / "cost-curves.csv", rows)
    print("cost curves at 10%:", rows[5], "20%:", rows[10])


def weights():
    wts = "[1, 1, 1, 1, 3, 1, 1, 3]"
    _, r1, c1 = zon("w_off", 'rule = "CAZ2"', ', cost = "cost.map"')
    _, r3, c3 = zon("w_on", f'rule = "CAZ2", weights = {wts}', ', cost = "cost.map"')
    for t in (0.1, 0.17):
        a, b = kept(c1, t)[0], kept(c3, t)[0]
        print("weights top", t, {sp.SHORT[f]: (round(100 * a[f], 1), round(100 * b[f], 1)) for f in sp.FEATURES})


# ------------------------------------------------------------------------------------------------ protected areas
def mask():
    w, rank, curves = zon("mask", 'rule = "CAZ2"', ', cost = "cost.map", hierarchic_mask = "protected.map"',
                          sp.read_node("protected", "protected.tif"))
    prot = sp.grid(sp.DATA / "protected.tif") > 0.5
    rank_map(rank, OUT / "mask.png", outline=prot)
    share = prot.sum() / sp.land().sum()
    k = kept(curves, share)[0]
    print(f"protected: {100 * share:.1f}% of the land; features held:",
          {sp.SHORT[f]: round(100 * k[f], 1) for f in sp.FEATURES})
    rows = [{"feature": sp.SHORT[f], "Protected areas": round(100 * k[f], 1),
             "Protected + best extension to 17 %": round(100 * kept(curves, 0.17)[0][f], 1)} for f in sp.FEATURES]
    sp.write_csv(DAT / "gap-analysis.csv", rows)
    return share, k


# ------------------------------------------------------------------------------------------------ connectivity
def connectivity():
    frames = []
    for d in (0, 1, 2, 4, 8):
        p = 'rule = "CAZ2"' + ("" if d == 0 else f', connectivity = {ALL8("{shape = " + chr(34) + "negexp" + chr(34) + f", distance = {d}" + "}")}')
        w, rank, curves = zon(f"conn_{d}", p, ', cost = "cost.map"')
        sel = np.nan_to_num(rank, nan=-1) >= 0.83
        top_frame(rank, 0.17, OUT / f"conn_{d}.png")
        k = kept(curves, 0.17)[0]
        # coverage of the raw features by the best 17 % (the curves are of the smoothed features)
        fs = sp.features()
        raw = np.mean([np.nansum(fs[f] * sel) / np.nansum(fs[f]) for f in sp.FEATURES])
        mins = min(np.nansum(fs[f] * sel) / np.nansum(fs[f]) for f in sp.FEATURES)
        frames.append({"d": d, "patches": patches(sel), "raw": 100 * raw, "edge": edges(sel), "min": 100 * mins})
        print(f"kernel {d}: {patches(sel)} patches, outline {edges(sel)} km, raw mean coverage {100 * raw:.1f}%, "
              f"min {100 * mins:.1f}%")
    return frames


# ------------------------------------------------------------------------------------------------ condition
def condition():
    cond = sp.read_node("condition", "condition.tif") + '''
[[nodes]]
id = "conditions"
op = "stack.of"
inputs = { condition = "condition.map" }
'''
    w, rank, curves = zon("condition", f'rule = "CAZ2", condition = {ALL8(chr(34) + "condition" + chr(34))}',
                          ', cost = "cost.map", condition_maps = "conditions.stack"', cond)
    _, base_rank, _ = zon("condition_off", 'rule = "CAZ2"', ', cost = "cost.map"')
    c = sp.grid(sp.DATA / "condition.tif")
    fig, axs = plt.subplots(1, 2, figsize=(9.6, 3.2), dpi=150)
    cm = plt.get_cmap("YlGn").copy()
    cm.set_bad((1, 1, 1, 0))
    im = axs[0].imshow(np.ma.masked_invalid(c), cmap=cm, vmin=0, vmax=1, interpolation="nearest")
    axs[0].set_title("Habitat condition (0 to 1)", loc="left", fontsize=11)
    cb = fig.colorbar(im, ax=axs[0], fraction=0.035, pad=0.02)
    cb.outline.set_visible(False)
    cb.ax.tick_params(labelsize=8.5)
    d = rank - base_rank
    dm = plt.get_cmap("RdBu_r").copy()
    dm.set_bad((1, 1, 1, 0))
    im2 = axs[1].imshow(np.ma.masked_invalid(d), cmap=dm, vmin=-0.5, vmax=0.5, interpolation="nearest")
    axs[1].set_title("Rank with condition minus without", loc="left", fontsize=11)
    cb2 = fig.colorbar(im2, ax=axs[1], fraction=0.035, pad=0.02)
    cb2.outline.set_visible(False)
    cb2.ax.grid(False)
    cb.ax.grid(False)
    cb2.ax.tick_params(labelsize=8.5)
    for ax in axs:
        ax.set_axis_off()
    fig.tight_layout()
    sp.save(fig, OUT / "condition.png")


# ------------------------------------------------------------------------------------------------ a Zonation 5 project
def project():
    """A z5 project of Serra Verde: a settings file, a feature list with weights, a file per feature; run by
    prioritize.zonation_project with -wh (weights, hierarchic mask) and compared with prioritize.zonation."""
    import rasterio
    w = sp.work(SLUG, "project")
    (w / "features").mkdir()
    with rasterio.open(sp.DATA / "features.tif") as src:
        prof = src.profile
        prof.update(count=1)
        for i, f in enumerate(sp.FEATURES):
            with rasterio.open(w / "features" / f"{f}.tif", "w", **prof) as dst:
                dst.write(src.read(i + 1), 1)
    weights = {f: (3 if f in ("golden_lion_tamarin", "mangrove") else 1) for f in sp.FEATURES}
    flist = "weight filename name\n" + "".join(f"{weights[f]} features/{f}.tif {f}\n" for f in sp.FEATURES)
    (w / "features.txt").write_text(flist, encoding="utf-8")
    settings = ("feature list file = features.txt\n"
                "hierarchic mask layer = protected.tif\n"
                "cost layer = cost.tif\n")
    (w / "settings.z5").write_text(settings, encoding="utf-8")
    model = '''format = "terraflux/1"

[[nodes]]
id = "project"
op = "prioritize.zonation_project"
params = { path = "settings.z5", flags = "-whX", mode = "CAZ2" }

[outputs.curves]
from = "project.curves"
'''
    model += sp.read_node("features", "features.tif") + sp.read_node("cost", "cost.tif") + \
        sp.read_node("protected", "protected.tif") + '''
[[nodes]]
id = "direct"
op = "prioritize.zonation"
inputs = { features = "features.stack", cost = "cost.map", hierarchic_mask = "protected.map" }
params = { rule = "CAZ2", weights = [1, 1, 1, 1, 3, 1, 1, 3] }
'''
    model = figs.add_writes(model, {"project.rank": "project_rank.tif", "direct.rank": "direct_rank.tif"})
    figs.run(model, w, "-q")
    a, b = sp.grid(w / "project_rank.tif"), sp.grid(w / "direct_rank.tif")
    diff = np.nanmax(np.abs(a - b))
    print("project vs direct: max |rank difference| =", diff)
    sp.zip_data(SLUG, "serra-verde-z5.zip", {
        "serra-verde-z5/settings.z5": settings,
        "serra-verde-z5/features.txt": flist,
        **{f"serra-verde-z5/features/{f}.tif": w / "features" / f"{f}.tif" for f in sp.FEATURES},
        "serra-verde-z5/cost.tif": sp.DATA / "cost.tif",
        "serra-verde-z5/protected.tif": sp.DATA / "protected.tif",
    })
    return diff


if __name__ == "__main__":
    import sys
    todo = sys.argv[1:] or ["base", "rules", "costs", "weights", "mask", "connectivity", "condition", "project"]
    for t in todo:
        globals()[t]()
