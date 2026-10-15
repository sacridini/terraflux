# The Terraflux website

The site published at https://sacridini.github.io/terraflux/: the home page, downloads, the quickstart, the
tutorials, the guide to parallel and distributed computing, the command line and operator references.

It is a static site made by a small Python generator (`build.py`): Markdown pages in `content/`, Jinja templates
in `templates/`, and plain CSS and JavaScript in `static/`, with no framework and no build step for the browser.
Every link between pages is relative, so the folder it makes works under any address.

## Building

```bash
pip install -r website/requirements.txt
python website/build.py              # website/dist
python website/build.py --serve      # and serve it on http://localhost:8000, rebuilt when a file changes
python website/build.py --check      # also check every model graph with `terraflux check` (needs the binary)
```

GitHub Actions (`.github/workflows/website.yml`) builds and publishes it on GitHub Pages at every push to `main`
that touches `website/`. Enable Pages with **GitHub Actions** as its source in the repository's settings.

## What is where

| Path | What it holds |
|---|---|
| `site.yaml` | the name, the navigation, the downloads (and `releases_public`), the tutorials' categories |
| `content/` | the pages, in Markdown with front matter; `content/tutorials/<category>/` the tutorials |
| `templates/` | the pages' layouts (home, documentation, downloads, tutorials index, operator reference) |
| `static/` | CSS, JavaScript, images (`img/tutorials/...` the tutorials' figures) and chart data (`data/...`) |
| `lib/tfgraph.py` | model graphs drawn as SVG, the way the application draws them |
| `data/` | the operators (`ops.json`), the commands' help (`cli.json`), the library's subjects, from the binary |
| `models/` | models the site shows; `models/templates/` the application's templates as TOML |
| `figures/` | the scripts that make the tutorials' figures by running Terraflux |
| `tools/` | `refresh_data.py` (data/ from the binary), `extract_templates.py`, `figs.py` (figures), `make_social.py` |

## Writing

See [AUTHORING.md](AUTHORING.md): the audience, the shape of a tutorial, the components (model graphs,
sliders, before-and-after, charts, galleries), how figures are made from real runs, and how to check a page.

## After a new release of Terraflux

```bash
python website/tools/refresh_data.py       # operators and commands from target/release/terraflux
python website/tools/extract_templates.py  # the application's templates
python website/figures/<category>/<tutorial>.py   # figures whose operators changed
```

When the packages of a release are published on GitHub, set `releases_public: true` in `site.yaml`: the
download buttons then point to the release's files.
