"""Figures of 'A tour of Terraflux': the application's window with its parts numbered (from real screenshots of the
application, kept in app-shots/; app_shots.ps1 took them), the Templates menu, the Node pane, and what six
templates give, from runs of the templates with the command line."""

from PIL import Image

from _common import figs, np, plt, save

SLUG = "getting-started/tour"
out = figs.img_dir(SLUG)
work = figs.workdir(SLUG)
SHOTS = figs.WEB / "figures" / "getting-started" / "app-shots"

# ------------------------------------------------------------------ the window, its parts numbered
win = Image.open(SHOTS / "mspa-of-a-forest.png").convert("RGB")
W, H = win.size
marks = [
    (1, 1190, 14, "toolbar"),
    (2, 867, 44, "run"),
    (3, 292, 80, "node"),
    (4, 178, 42, "prepare"),
    (5, 560, 150, "model"),
    (6, 800, 77, "bar"),
    (7, 505, 424, "map tab"),
    (8, 1120, 458, "save"),
    (9, 1158, 600, "legend"),
    (10, 40, 785, "keys"),
]
fig = plt.figure(figsize=(W / 150, H / 150), dpi=150)
ax = fig.add_axes([0, 0, 1, 1])
ax.imshow(np.asarray(win))
ax.set_axis_off()
for n, x, y, _ in marks:
    ax.add_patch(plt.Circle((x, y), 15, color=figs.EMBER, ec="white", lw=1.6, zorder=5))
    ax.text(x, y + 0.5, str(n), color="white", ha="center", va="center", fontsize=8.5, fontweight="bold", zorder=6)
fig.savefig(work / "window_big.png", dpi=150)
plt.close(fig)
im = Image.open(work / "window_big.png").convert("RGB")
im = im.resize((1100, round(1100 * im.height / im.width)), Image.LANCZOS)
im.save(out / "window.png", optimize=True)

# ------------------------------------------------------------------ the empty window
im = Image.open(SHOTS / "empty.png").convert("RGB")
im = im.resize((1100, round(1100 * im.height / im.width)), Image.LANCZOS)
im.save(out / "empty.png", optimize=True)

# ------------------------------------------------------------------ the Templates menu and the Node pane, cropped
Image.open(SHOTS / "templates-menu.png").convert("RGB").crop((0, 0, 700, 270)).save(out / "templates-menu.png", optimize=True)
Image.open(SHOTS / "mspa-of-a-forest.png").convert("RGB").crop((0, 30, 332, 570)).save(out / "node-pane.png", optimize=True)
Image.open(SHOTS / "mspa-of-a-forest.png").convert("RGB").crop((335, 30, 1384, 405)).save(out / "model-pane.png", optimize=True)

# ------------------------------------------------------------------ what six templates give (run by the command line)
runs = [
    ("mspa-of-a-forest", {"mspa.map": "a.tif"}, dict(palette=figs.mspa_palette(), legend=False), "MSPA of a forest"),
    ("fragmentation-of-a-forest", {"fad.map": "a.tif"}, dict(cmap="RdYlGn", legend=False), "Fragmentation of a forest"),
    ("circuit-connectivity", {"pairwise.cum": "a.tif"}, dict(log=True, cmap="magma", legend=False), "Circuit connectivity"),
    ("terrain-of-a-surface", {"heat_load.map": "a.tif"}, dict(cmap="inferno", legend=False), "Terrain of a surface"),
    ("neutral-landscapes", {"fractal.map": "a.tif"}, dict(cmap="terrain", legend=False), "Neutral landscapes"),
    ("carbon-and-its-uncertainty", {"carbon.sd": "a.tif"}, dict(cmap="viridis", legend=False), "Carbon and its uncertainty"),
]
for slug, writes, kw, title in runs:
    d = figs.workdir(f"{SLUG}/{slug}")
    model = figs.add_writes(figs.template(slug), writes)
    figs.run(model, d)
    arr = None
    if slug.startswith("fragmentation"):
        a, _ = figs.read(d / "a.tif")
        arr = np.ma.masked_where(a > 100, a)  # GTB's codes above 100: not forest
        kw.update(vmin=0, vmax=100)
    figs.raster(d / "a.tif", out / f"t_{slug}.png", width=420, array=arr, **kw)
    print("ran", slug)
print("done")
