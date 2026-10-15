"""Figures of "Regions from species composition" (biodiversity, part 3): Biodiverse's cluster analysis of the example
cells (Sorenson, average linkage), cut into 3 to 8 clusters, its dendrogram, and the same with a spatial condition
that keeps the regions contiguous, from real runs of terraflux (bio.table, bio.cluster)."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _bio  # noqa: E402
from _bio import figs, np, plt  # noqa: E402

SLUG = "biodiversity/regions"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_bio.setup(work)

KS = [3, 4, 5, 6, 8]
MODEL = """format = "terraflux/1"

[[nodes]]
id = "sites"
op = "bio.table"
params = { path = "sites.csv", group_columns = ["x", "y"], label_columns = ["genus", "species"], cell_sizes = [100000, 100000] }
"""
writes = {}
for k in KS:
    MODEL += f"""
[[nodes]]
id = "cluster_{k}"
op = "bio.cluster"
inputs = {{ basedata = "sites.basedata" }}
params = {{ index = "SORENSON", linkage = "link_average", k = {k}, calculations = ["richness"] }}
"""
    writes[f"cluster_{k}.map"] = f"cluster_{k}.tif"
    writes[f"cluster_{k}.clusters"] = f"clusters_{k}.csv"
MODEL += """
[[nodes]]
id = "contiguous"
op = "bio.cluster"
inputs = { basedata = "sites.basedata" }
params = { index = "SORENSON", linkage = "link_average", k = 5, conditions = ["sp_circle (radius => 150000)"], calculations = ["richness"] }

[[nodes]]
id = "jaccard"
op = "bio.cluster"
inputs = { basedata = "sites.basedata" }
params = { index = "JACCARD", linkage = "link_average", k = 5, calculations = ["richness"] }
"""
writes.update({"contiguous.map": "contiguous.tif", "contiguous.clusters": "contiguous.csv",
               "jaccard.map": "jaccard.tif", "cluster_5.nodes": "nodes.csv", "cluster_5.table": "table5.csv"})
figs.run(figs.add_writes(MODEL, writes), work)
o = work / "output"


def read_map(tif):
    import rasterio

    with rasterio.open(tif) as ds:
        a = ds.read(1, masked=True).astype(float)
        b = ds.bounds
    return a, (b.left, b.right, b.bottom, b.top)


def palette(n):
    return {i: (f"Cluster {i}", figs.CLASSES[i - 1]) for i in range(1, n + 1)}


COLOUR_OF = {}  # k -> {cluster id: colour index}, colours following the regions from one cut to the next
prev = None
for k in KS:
    a, ext = read_map(work / f"cluster_{k}.tif")
    af = np.ma.filled(a, 0).astype(int)
    ids = [i for i in range(1, k + 1) if (af == i).any()]
    if prev is None:
        cmap_k = {i: n for n, i in enumerate(sorted(ids, key=lambda i: -(af == i).sum()))}
    else:
        pa, pc = prev
        cmap_k, used = {}, set()
        # every new cluster takes the colour of the old cluster it overlaps most, largest clusters first
        for i in sorted(ids, key=lambda i: -(af == i).sum()):
            olap = {pc[j]: int(((af == i) & (pa == j)).sum()) for j in pc}
            best = max((c for c in olap if c not in used), key=lambda c: olap[c], default=None)
            if best is None or olap[best] == 0:
                best = next(n for n in range(len(figs.CLASSES)) if n not in used and n not in pc.values())
            cmap_k[i] = best
            used.add(best)
    COLOUR_OF[k] = cmap_k
    prev = (af, cmap_k)
    pal = {i: (f"Region {n + 1}", figs.CLASSES[n]) for i, n in cmap_k.items()}
    _bio.cell_map(a, out / f"clusters_{k}.png", palette=pal, extent=ext, width=900, legend=False)
    sizes = sorted((int((a == i).sum()) for i in range(1, k + 1)), reverse=True)
    print("k", k, "cluster sizes", sizes)
for name in ("contiguous", "jaccard"):
    a, ext = read_map(work / f"{name}.tif")
    print(name, "sizes", sorted((int((a == i).sum()) for i in range(1, 6)), reverse=True))
# agreement between Sorenson and Jaccard at k = 5 (adjusted Rand index)
s5 = np.ma.filled(read_map(work / "cluster_5.tif")[0], 0).ravel()
j5 = np.ma.filled(read_map(work / "jaccard.tif")[0], 0).ravel()
c5 = np.ma.filled(read_map(work / "contiguous.tif")[0], 0).ravel()


def ari(a, b):
    m = (a > 0) & (b > 0)
    a, b = a[m].astype(int), b[m].astype(int)
    from math import comb
    t = np.zeros((a.max() + 1, b.max() + 1), dtype=int)
    for x, y in zip(a, b):
        t[x, y] += 1
    sc = sum(comb(int(v), 2) for v in t.ravel())
    sa = sum(comb(int(v), 2) for v in t.sum(1))
    sb = sum(comb(int(v), 2) for v in t.sum(0))
    n = comb(len(a), 2)
    exp = sa * sb / n
    return (sc - exp) / ((sa + sb) / 2 - exp)


print("ARI sorenson-jaccard", round(ari(s5, j5), 2), "sorenson-contiguous", round(ari(s5, c5), 2))

# ---------------------------------------------------------------- the dendrogram, tips coloured by cluster (k = 5)
nodes = figs.read_csv(o / "nodes.csv")
cl5 = {r["group"]: int(float(r["cluster"])) for r in figs.read_csv(o / "clusters_5.csv")}
print("node columns", list(nodes[0].keys()))
parent = {r["node"]: r["parent"] for r in nodes}
length = {r["node"]: float(r["length"] or 0) for r in nodes}
group = {r["node"]: r["group"] for r in nodes if r.get("group")}
children = {}
for n, p in parent.items():
    if p:
        children.setdefault(p, []).append(n)
root = next(n for n, p in parent.items() if not p)
depth = {root: 0.0}
order = []


def walk(n):
    for c in children.get(n, []):
        depth[c] = depth[n] + length[c]
        walk(c)
    if n not in children:
        order.append(n)


walk(root)
ypos = {n: i for i, n in enumerate(order)}


def yc(n):
    if n in ypos:
        return ypos[n]
    ys = [yc(c) for c in children[n]]
    ypos[n] = (min(ys) + max(ys)) / 2
    return ypos[n]


yc(root)
top = max(depth.values())
depth = {n: top - d for n, d in depth.items()}  # the height of each join: the dissimilarity at which it was made
fig, ax = plt.subplots(figsize=(9, 3.4), dpi=150)
for n, cs in children.items():
    for c in cs:
        ax.plot([ypos[n], ypos[c]], [depth[n], depth[n]], color=figs.INK2, lw=0.7)
        ax.plot([ypos[c], ypos[c]], [depth[n], depth[c]], color=figs.INK2, lw=0.7)
for n in order:
    g = group.get(n)
    k = cl5.get(g)
    if k:
        ax.plot(ypos[n], depth[n], "s", ms=3.2, color=figs.CLASSES[COLOUR_OF[5][k]])
ax.axhline(top - float(nodes[0]["length"] or 0) * 0, color="none")
ax.set_xticks([])
ax.grid(False)
ax.spines["bottom"].set_visible(False)
ax.set_ylabel("Dissimilarity joined")
fig.tight_layout()
fig.savefig(out / "dendrogram.png", dpi=150)
plt.close(fig)
figs._shrink(out / "dendrogram.png")
print("tips", len(order), "root", root, "max depth", round(max(depth.values()), 3))
