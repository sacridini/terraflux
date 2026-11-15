"""Figures of 'A tour of Terraflux': what six templates give, from runs of the templates with the command line.

The pictures of the application itself (empty.png, window.png with its parts numbered, model-pane.png,
templates-menu.png and node-pane.png) are no longer made here: they are drawn off screen by the application's own
test, `cargo test --release -p terraflux-app --test readme_images tour_images -- --ignored`, in the default theme.
(They were cut from desktop captures, app_shots.ps1 and app-shots/, which went out of date with the window.)"""

from _common import figs, np, plt, save

SLUG = "getting-started/tour"
out = figs.img_dir(SLUG)
work = figs.workdir(SLUG)

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
