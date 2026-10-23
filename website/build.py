"""Build the Terraflux website: content/*.md and templates/ into dist/ (static files, for GitHub Pages).

    python website/build.py                 # dist/
    python website/build.py --serve         # build, then serve on http://localhost:8000 (rebuilds on change)
    python website/build.py --check         # also check every ```tfgraph model with `terraflux check`
    python website/build.py --out DIR       # another output folder
    python website/build.py --only PATH     # build only the pages under content/PATH (and the site's chrome)

Needs Python 3.11+ with markdown, pymdown-extensions, jinja2, pyyaml, pygments and pillow.
Every link between pages is relative, so the site works under any address (a GitHub Pages project, a domain).
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import html
import io
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import time

import jinja2
import markdown
import yaml
from markdown.extensions.toc import slugify_unicode
from pygments.formatters import HtmlFormatter

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
from lib import offline as OFFLINE  # noqa: E402
from lib.tfgraph import GraphError, Renderer, esc  # noqa: E402

CONTENT = HERE / "content"
STATIC = HERE / "static"
TEMPLATES = HERE / "templates"
DATA = HERE / "data"

WARNINGS: list[str] = []
CURRENT: dict = {"page": None}


def warn(msg: str):
    where = CURRENT["page"].src if CURRENT["page"] else "site"
    WARNINGS.append(f"{where}: {msg}")


# ------------------------------------------------------------------------------------------------ data


def load_yaml(p: pathlib.Path):
    return yaml.safe_load(p.read_text(encoding="utf-8"))


SITE = load_yaml(HERE / "site.yaml")
OPS = {o["name"]: o for o in json.loads((DATA / "ops.json").read_text(encoding="utf-8"))}
CLI = json.loads((DATA / "cli.json").read_text(encoding="utf-8"))
SUBJECTS = json.loads((DATA / "subjects.json").read_text(encoding="utf-8"))
SITE["version"] = CLI.get("version", "0.0.0")
# The downloads are those of the latest release published on GitHub (TERRAFLUX_RELEASE, given by the website's
# workflow), older than the operators' data while a new release's packages are still being built.
# TERRAFLUX_RELEASE_FILES (its files' names, one a line) leaves out the packages it does not have (made since).
if os.environ.get("TERRAFLUX_RELEASE"):
    SITE["release"] = os.environ["TERRAFLUX_RELEASE"].removeprefix("v")
    SITE["releases_public"] = True
else:
    SITE["release"] = SITE["version"]
RELEASE_FILES = set(os.environ.get("TERRAFLUX_RELEASE_FILES", "").split())


def subject_of(op: str) -> str:
    if op in SUBJECTS["io"]:
        return "IO"
    prefix = op.split(".")[0]
    for s in SUBJECTS["subjects"]:
        if prefix in s["prefixes"]:
            return s["title"]
    for c in SITE["tutorial_categories"]:
        if prefix in c.get("prefixes", []):
            return c["title"]
    return "Other"


def slug(s: str) -> str:
    return slugify_unicode(s, "-")


def op_anchor(op: str) -> str:
    return "op-" + op.replace(".", "-").replace("_", "-")


def op_url(op: str) -> str:
    return f"/operators/{slug(subject_of(op))}/#{op_anchor(op)}"


# ------------------------------------------------------------------------------------------------ pages


class Page:
    def __init__(self, src: pathlib.Path):
        self.src = src.relative_to(CONTENT).as_posix()
        text = src.read_text(encoding="utf-8")
        self.meta: dict = {}
        if text.startswith("---"):
            end = text.index("\n---", 3)
            self.meta = yaml.safe_load(text[3:end]) or {}
            text = text[end + 4 :]
        self.body = text
        parts = self.src[:-3].split("/")
        if parts[-1] == "index":
            parts = parts[:-1]
        self.url = "/" + "/".join(parts) + ("/" if parts else "")
        self.section = parts[0] if parts else ""
        self.title = self.meta.get("title", parts[-1] if parts else SITE["name"])
        self.order = self.meta.get("order", 999)
        self.html = ""
        self.toc: list = []
        self.text = ""

    @property
    def category(self):
        p = self.src.split("/")
        return p[1] if p[0] == "tutorials" and len(p) > 2 else None

    @property
    def is_index(self):
        return self.src.endswith("index.md")


def rel_prefix(url: str) -> str:
    depth = url.strip("/").count("/") + (1 if url.strip("/") else 0)
    return "../" * depth if depth else "./"


def relativize(html_text: str, url: str) -> str:
    """href="/x" and src="/x" made relative to the page at `url`."""
    root = rel_prefix(url)

    def fix(m):
        attr, q, path = m.group(1), m.group(2), m.group(3)
        if path.startswith("//"):
            return m.group(0)
        p = path[1:]
        if p == "" or p.startswith("#"):
            return f"{attr}={q}{root}{p}"
        return f"{attr}={q}{root}{p}"

    return re.sub(r'\b(href|src|srcset|data-src|data-full)=(["\'])(/[^"\']*)', lambda m: fix(m), html_text)


# ------------------------------------------------------------------------------------------------ markdown blocks

RENDERER = Renderer(OPS)
GRAPHS: list[tuple[str, str, dict]] = []  # (page, toml, options) for --check
IMAGE_SIZES: dict[str, tuple[int, int]] = {}


def image_size(src: str):
    if not src.startswith("/"):
        return None
    if src in IMAGE_SIZES:
        return IMAGE_SIZES[src]
    p = STATIC / src[1:]
    size = None
    if p.exists() and p.suffix.lower() in (".png", ".jpg", ".jpeg", ".webp", ".gif"):
        try:
            from PIL import Image

            with Image.open(p) as im:
                size = im.size
        except Exception:
            size = None
    elif not p.exists():
        warn(f"missing image {src}")
    IMAGE_SIZES[src] = size
    return size


def fence_tfgraph(source, language, class_name, options, md, **kwargs):
    try:
        svg, opts, name = RENDERER.render(source)
    except GraphError as e:
        warn(str(e))
        return f'<div class="admonition danger"><p>{esc(e)}</p></div>'
    GRAPHS.append((CURRENT["page"].src if CURRENT["page"] else "?", source, opts))
    cap = opts.get("caption", "")
    clean = "\n".join(l for l in source.splitlines() if not re.match(r"^\s*#!", l)).strip("\n")
    code = ""
    if opts.get("toml", True):
        hl = highlight(clean, "toml")
        code = (
            '<details class="tfg-code"><summary>Model file (TOML)</summary>'
            f'<div class="codeblock"><button class="copy" type="button" aria-label="Copy">Copy</button>{hl}</div></details>'
        )
    caption = f"<figcaption>{md_inline(cap)}</figcaption>" if cap else ""
    return (
        f'<figure class="tfg-figure"><div class="tfg-canvas zoomable" data-caption="{esc(cap)}" tabindex="0">{svg}'
        f'<button class="zoom-btn" type="button" aria-label="Enlarge">Enlarge</button></div>{caption}{code}</figure>'
    )


def highlight(code: str, lang: str) -> str:
    from pygments import highlight as ph
    from pygments.lexers import get_lexer_by_name

    try:
        lexer = get_lexer_by_name(lang)
    except Exception:
        lexer = get_lexer_by_name("text")
    return ph(code, lexer, HtmlFormatter(cssclass="highlight", nowrap=False))


def md_inline(text: str) -> str:
    out = markdown.markdown(text, extensions=["pymdownx.inlinehilite", "pymdownx.arithmatex"],
                            extension_configs={"pymdownx.arithmatex": {"generic": True}})
    return re.sub(r"^<p>(.*)</p>$", r"\1", out.strip(), flags=re.S)


def spec_of(source: str, kind: str) -> dict:
    try:
        return yaml.safe_load(source) or {}
    except yaml.YAMLError:
        pass
    # captions often hold ": " (a colon then a space): quote plain text values and try again
    def quote(m):
        v = m.group(3).strip()
        if v[:1] in "\"'{[|>" or not v:
            return m.group(0)
        return f"{m.group(1)}{m.group(2)}: {json.dumps(v, ensure_ascii=False)}"

    fixed = re.sub(r"^(\s*(?:-\s*)?)(caption|label|alt|title|xlabel|ylabel): (.*)$", quote, source, flags=re.M)
    try:
        return yaml.safe_load(fixed) or {}
    except yaml.YAMLError as e:
        warn(f"{kind}: not valid YAML: {e}")
        return {}


def img_tag(src: str, alt: str, cls: str = "", lazy=True) -> str:
    size = image_size(src)
    wh = f' width="{size[0]}" height="{size[1]}"' if size else ""
    return f'<img src="{esc(src)}" alt="{esc(alt)}"{wh}{" loading=\"lazy\"" if lazy else ""} decoding="async"{f" class={chr(34)}{cls}{chr(34)}" if cls else ""}>'


def fence_slider(source, language, class_name, options, md, **kwargs):
    """Frames shown one at a time with a slider (a parameter's values, a simulation's steps), optionally beside a
    chart whose x follows the slider and a table of numbers per frame."""
    s = spec_of(source, "tfslider")
    frames = s.get("frames")
    if not frames and "values" in s:
        frames = [{"value": v, "src": s["src"].replace("{value}", str(v))} for v in s["values"]]
    frames = frames or []
    if not frames:
        warn("tfslider without frames")
        return ""
    label = s.get("label", "")
    start = s.get("start", 0)
    imgs = []
    for i, f in enumerate(frames):
        imgs.append(
            f'<div class="sl-frame{" is-on" if i == start else ""}" data-i="{i}">'
            f'{img_tag(f["src"], f.get("alt", f"{label} {f.get("value", i)}"), lazy=i != start)}</div>'
        )
    data = [{"value": f.get("value", i), "caption": md_inline(f.get("caption", "")) if f.get("caption") else "",
             "stats": f.get("stats", {})} for i, f in enumerate(frames)]
    chart = ""
    if s.get("chart"):
        chart = chart_html(s["chart"], linked=True)
    stats = '<dl class="sl-stats"></dl>' if any(f.get("stats") for f in frames) else ""
    cap = s.get("caption", "")
    play = '<button class="sl-play" type="button" aria-label="Play">Play</button>' if s.get("play", len(frames) > 3) else ""
    return (
        f'<figure class="tf-slider{" has-chart" if chart else ""}" data-frames=\'{esc(json.dumps(data))}\' data-start="{start}">'
        f'<div class="sl-main"><div class="sl-view zoomable" data-caption="{esc(cap)}">{"".join(imgs)}'
        f'<button class="zoom-btn" type="button" aria-label="Enlarge">Enlarge</button></div>'
        f'<div class="sl-controls">{play}<label class="sl-label"><span>{esc(label)}</span>'
        f'<input type="range" min="0" max="{len(frames) - 1}" value="{start}" step="1" aria-label="{esc(label)}"></label>'
        f'<output class="sl-value"></output></div><div class="sl-caption"></div></div>'
        f'<div class="sl-side">{chart}{stats}</div>'
        + (f"<figcaption>{md_inline(cap)}</figcaption>" if cap else "")
        + "</figure>"
    )


def fence_compare(source, language, class_name, options, md, **kwargs):
    """Two images, one over the other, with a handle to swipe between them (before and after)."""
    s = spec_of(source, "tfcompare")
    a, b = s.get("before", {}), s.get("after", {})
    cap = s.get("caption", "")
    return (
        f'<figure class="tf-compare"><div class="cmp-view" style="--pos:50%">'
        f'<div class="cmp-a">{img_tag(a.get("src", ""), a.get("label", "before"))}<span class="cmp-tag">{esc(a.get("label", "Before"))}</span></div>'
        f'<div class="cmp-b">{img_tag(b.get("src", ""), b.get("label", "after"))}<span class="cmp-tag">{esc(b.get("label", "After"))}</span></div>'
        f'<input class="cmp-range" type="range" min="0" max="100" value="50" aria-label="Swipe between the two images">'
        f'<span class="cmp-handle" aria-hidden="true"></span></div>'
        + (f"<figcaption>{md_inline(cap)}</figcaption>" if cap else "")
        + "</figure>"
    )


def read_table(spec: dict) -> list[dict]:
    if "csv" in spec:
        text = spec["csv"]
    elif "data" in spec:
        p = STATIC / spec["data"].lstrip("/")
        if not p.exists():
            warn(f"tfchart: missing data {spec['data']}")
            return []
        text = p.read_text(encoding="utf-8")
    else:
        return spec.get("rows", [])
    rows = list(csv.DictReader(io.StringIO(text.strip())))
    out = []
    for r in rows:
        o = {}
        for k, v in r.items():
            k = (k or "").strip()
            try:
                o[k] = float(v)
            except (TypeError, ValueError):
                o[k] = (v or "").strip()
        out.append(o)
    return out


def chart_html(spec: dict, linked=False) -> str:
    rows = read_table(spec)
    if not rows:
        return ""
    ys = spec.get("y")
    if isinstance(ys, str):
        ys = [ys]
    cfg = {
        "type": spec.get("type", "line"),
        "x": spec.get("x", list(rows[0].keys())[0]),
        "y": ys or [k for k in rows[0].keys() if k != spec.get("x")],
        "colors": spec.get("colors", {}),
        "labels": spec.get("labels", {}),
        "xlabel": spec.get("xlabel", ""),
        "ylabel": spec.get("ylabel", ""),
        "title": spec.get("title", ""),
        "stacked": spec.get("stacked", False),
        "percent": spec.get("percent", False),
        "rows": rows,
        "linked": linked,
        "height": spec.get("height", 300),
        "xlog": spec.get("xlog", False),
        "ylog": spec.get("ylog", False),
        "series": spec.get("series"),
    }
    title = f'<div class="ch-title">{esc(cfg["title"])}</div>' if cfg["title"] else ""
    return (
        f'<div class="tf-chart" data-chart=\'{esc(json.dumps(cfg))}\'>{title}<div class="ch-plot" role="img" '
        f'aria-label="{esc(cfg["title"] or "chart")}"></div><div class="ch-legend"></div></div>'
    )


def fence_chart(source, language, class_name, options, md, **kwargs):
    s = spec_of(source, "tfchart")
    cap = s.get("caption", "")
    inner = chart_html(s)
    return f'<figure class="tf-chart-fig">{inner}' + (f"<figcaption>{md_inline(cap)}</figcaption>" if cap else "") + "</figure>"


def fence_gallery(source, language, class_name, options, md, **kwargs):
    """A row of small figures (2 to 4), each enlarged on a click."""
    s = spec_of(source, "tfgallery")
    items = s.get("images", [])
    cols = s.get("columns", min(max(len(items), 2), 4))
    cells = []
    for it in items:
        cap = it.get("caption", "")
        cells.append(
            f'<figure class="g-item"><div class="zoomable" data-caption="{esc(cap)}">{img_tag(it["src"], it.get("alt", cap))}</div>'
            + (f"<figcaption>{md_inline(cap)}</figcaption>" if cap else "")
            + "</figure>"
        )
    cap = s.get("caption", "")
    return (
        f'<figure class="tf-gallery" style="--cols:{cols}"><div class="g-grid">{"".join(cells)}</div>'
        + (f"<figcaption>{md_inline(cap)}</figcaption>" if cap else "")
        + "</figure>"
    )


def fence_table(source, language, class_name, options, md, **kwargs):
    """A table from a CSV (`data:` a file under static/, or `csv: |` inline): `columns` picks and orders them,
    `labels` renames them, `digits` rounds numbers, `heat: true` shades numeric cells by value (a matrix)."""
    s = spec_of(source, "tftable")
    rows = read_table(s)
    if not rows:
        warn("tftable without rows")
        return ""
    cols = s.get("columns") or list(rows[0].keys())
    labels = s.get("labels", {})
    digits = s.get("digits", 3)
    nums = [r[c] for r in rows for c in cols[1:] if isinstance(r.get(c), float)]
    lo, hi = (min(nums), max(nums)) if nums else (0, 1)

    def cell(v, first):
        if isinstance(v, float):
            txt = f"{int(v):,}" if v == int(v) and abs(v) < 1e15 else f"{v:,.{digits}f}".rstrip("0").rstrip(".")
            style = ""
            if s.get("heat") and not first and hi > lo:
                t = (v - lo) / (hi - lo)
                style = f' style="background: color-mix(in srgb, var(--forest) {round(t * 55)}%, transparent)"'
            return f'<td class="num"{style}>{esc(txt)}</td>'
        return f"<td>{esc(v)}</td>"

    head = "".join(f"<th>{esc(labels.get(c, c))}</th>" for c in cols)
    body = "".join("<tr>" + "".join(cell(r.get(c, ""), i == 0) for i, c in enumerate(cols)) + "</tr>" for r in rows)
    cap = s.get("caption", "")
    return (
        f'<figure class="tf-table"><div class="table-wrap"><table><thead><tr>{head}</tr></thead><tbody>{body}</tbody>'
        f"</table></div>" + (f"<figcaption>{md_inline(cap)}</figcaption>" if cap else "") + "</figure>"
    )


def fence_cli(source, language, class_name, options, md, **kwargs):
    """The help of a command, from data/cli.json: ```tfcli\nrun\n``` or ```tfcli\ncluster add\n```."""
    name = source.strip().split()
    if not name:
        text = CLI["help"]
    elif len(name) == 1:
        text = CLI["commands"].get(name[0], {}).get("help")
    else:
        text = CLI["commands"].get(name[0], {}).get("subcommands", {}).get(name[1])
    if not text:
        warn(f"tfcli: no command {source.strip()!r}")
        return ""
    return f'<div class="codeblock cli-help"><button class="copy" type="button" aria-label="Copy">Copy</button><pre><code>{esc(text.strip())}</code></pre></div>'


OP_LINK = re.compile(r"\[\[([a-z_]+\.[a-z0-9_]+)(\|[^\]]+)?\]\]")


def op_links(text: str) -> str:
    def rep(m):
        op = m.group(1)
        label = m.group(2)[1:] if m.group(2) else None
        if op not in OPS:
            warn(f"unknown operator [[{op}]]")
            return f"`{op}`"
        doc = OPS[op]["doc"].split(". ")[0][:160]
        doc = doc.replace('"', "'")
        inner = esc(label) if label else f"<code>{op}</code>"
        return f'<a class="op-link" href="{op_url(op)}" title="{esc(doc)}">{inner}</a>'

    # not inside code blocks
    parts = re.split(r"(```.*?```)", text, flags=re.S)
    return "".join(p if p.startswith("```") else OP_LINK.sub(rep, p) for p in parts)


MD_EXT = [
    "abbr", "attr_list", "def_list", "footnotes", "md_in_html", "tables", "admonition", "sane_lists",
    "toc", "pymdownx.superfences", "pymdownx.highlight", "pymdownx.inlinehilite", "pymdownx.details",
    "pymdownx.tabbed", "pymdownx.keys", "pymdownx.arithmatex", "pymdownx.tasklist", "pymdownx.mark",
    "pymdownx.smartsymbols", "pymdownx.betterem",
]


def md_config():
    fences = [
        {"name": "tfgraph", "class": "tfgraph", "format": fence_tfgraph},
        {"name": "tfslider", "class": "tfslider", "format": fence_slider},
        {"name": "tfcompare", "class": "tfcompare", "format": fence_compare},
        {"name": "tfchart", "class": "tfchart", "format": fence_chart},
        {"name": "tfgallery", "class": "tfgallery", "format": fence_gallery},
        {"name": "tfcli", "class": "tfcli", "format": fence_cli},
        {"name": "tftable", "class": "tftable", "format": fence_table},
    ]
    return {
        "toc": {"permalink": "#", "permalink_title": "Link to this section", "toc_depth": "2-3", "slugify": slugify_unicode},
        "pymdownx.superfences": {"custom_fences": fences},
        "pymdownx.highlight": {"css_class": "highlight", "guess_lang": False, "use_pygments": True},
        "pymdownx.tabbed": {"alternate_style": True},
        "pymdownx.arithmatex": {"generic": True},
        "footnotes": {"BACKLINK_TITLE": "Back to the text"},
    }


FIG = re.compile(r'<p>\s*<img alt="([^"]*)" src="([^"]+)"(?: title="([^"]*)")?\s*/?>\s*</p>')


def figures(text: str) -> str:
    def rep(m):
        alt, src, title = m.group(1), m.group(2), m.group(3)
        cap = html.unescape(title) if title else ""
        capt = f"<figcaption>{md_inline(cap)}</figcaption>" if cap else ""
        return (
            f'<figure class="fig"><div class="zoomable" data-caption="{esc(cap)}">{img_tag(html.unescape(src), html.unescape(alt))}'
            f'<button class="zoom-btn" type="button" aria-label="Enlarge">Enlarge</button></div>{capt}</figure>'
        )

    return FIG.sub(rep, text)


def _wrap_code(text: str) -> str:
    out = []
    i = 0
    tag = '<div class="highlight">'
    while True:
        j = text.find(tag, i)
        if j < 0:
            out.append(text[i:])
            break
        # already wrapped (tfgraph's model file)
        before = text[max(0, j - 80) : j]
        k = text.find("</pre></div>", j)
        if k < 0:
            out.append(text[i:])
            break
        k += len("</pre></div>")
        out.append(text[i:j])
        block = text[j:k]
        if 'class="copy"' in before:
            out.append(block)
        else:
            out.append(f'<div class="codeblock"><button class="copy" type="button" aria-label="Copy">Copy</button>{block}</div>')
        i = k
    return "".join(out)


def tables(text: str) -> str:
    return text.replace("<table>", '<div class="table-wrap"><table>').replace("</table>", "</table></div>")


def render_markdown(page: Page) -> None:
    CURRENT["page"] = page
    md = markdown.Markdown(extensions=MD_EXT, extension_configs=md_config())
    body = op_links(page.body)
    out = md.convert(body)
    out = figures(out)
    out = _wrap_code(out)
    out = tables(out)
    page.html = out
    page.toc = md.toc_tokens
    page.text = re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", " ", re.sub(r"<svg.*?</svg>", " ", out, flags=re.S))))
    CURRENT["page"] = None


# ------------------------------------------------------------------------------------------------ site


def collect(only: str | None) -> list[Page]:
    pages = []
    for p in sorted(CONTENT.rglob("*.md")):
        rel = p.relative_to(CONTENT).as_posix()
        if only and not rel.startswith(only) and not rel.endswith("index.md"):
            continue
        if p.name.startswith("_"):
            continue
        pages.append(Page(p))
    return pages


def tutorial_tree(pages: list[Page]):
    cats = []
    for c in SITE["tutorial_categories"]:
        items = [p for p in pages if p.category == c["slug"] and not p.is_index]
        items.sort(key=lambda p: (p.meta.get("order", 999), p.title))
        index = next((p for p in pages if p.category == c["slug"] and p.is_index), None)
        # series: pages sharing `series`, in the order of `part`
        groups = []
        seen = {}
        for p in items:
            s = p.meta.get("series")
            if s:
                if s not in seen:
                    seen[s] = {"series": s, "pages": []}
                    groups.append(seen[s])
                seen[s]["pages"].append(p)
            else:
                groups.append({"series": None, "pages": [p]})
        for g in groups:
            g["pages"].sort(key=lambda p: p.meta.get("part", 0))
        cats.append({**c, "pages": items, "groups": groups, "index": index})
    return cats


def series_info(page: Page, cats):
    s = page.meta.get("series")
    if not s:
        return None
    for c in cats:
        for g in c["groups"]:
            if g["series"] == s and page in g["pages"]:
                return {"title": s, "pages": g["pages"], "index": g["pages"].index(page)}
    return None


def sidebar(page: Page, pages: list[Page], cats):
    """The navigation at the left of a page: its section's pages (the tutorials by category)."""
    if page.section == "tutorials":
        return {"kind": "tutorials", "cats": cats}
    if page.section == "operators":
        return {"kind": "operators"}
    items = [p for p in pages if p.section == page.section]
    items.sort(key=lambda p: (not p.is_index, p.order, p.title))
    return {"kind": "section", "title": SITE["sections"].get(page.section, {}).get("title", ""), "pages": items}


def prev_next(page: Page, pages: list[Page], cats):
    if page.section == "tutorials":
        seq = [p for c in cats for g in c["groups"] for p in g["pages"]]
    else:
        seq = sorted([p for p in pages if p.section == page.section], key=lambda p: (not p.is_index, p.order, p.title))
    if page not in seq:
        return None, None
    i = seq.index(page)
    return (seq[i - 1] if i > 0 else None), (seq[i + 1] if i + 1 < len(seq) else None)


def downloads():
    v = SITE["release"]
    tag = SITE["release_tag"].format(version=v)
    base = SITE["release_base"].format(tag=tag, version=v)
    out = []
    for d in SITE["downloads"]:
        files = []
        for f in d["files"]:
            name = f["name"].format(version=v)
            if RELEASE_FILES and name not in RELEASE_FILES:
                continue
            files.append({**f, "name": name, "url": base + name, "note": f.get("note", "").format(version=v)})
        if files:
            out.append({**d, "files": files})
    return out


def operators_pages():
    groups: dict[str, list[dict]] = {}
    for name, o in sorted(OPS.items()):
        groups.setdefault(subject_of(name), []).append(o)
    order = [s["title"] for s in SUBJECTS["subjects"]] + ["Other"]
    return [{"title": t, "slug": slug(t), "ops": groups[t]} for t in order if t in groups]


def search_entry(url: str, title: str, section: str, text: str, headings: list[str]):
    return {"u": url.lstrip("/"), "t": title, "s": section, "h": " · ".join(headings)[:400], "x": text[:4000]}


def flat_toc(tokens, depth=0):
    out = []
    for t in tokens:
        out.append(t["name"])
        out += flat_toc(t.get("children", []), depth + 1)
    return out


def webp_copy(src_dir: pathlib.Path, out_dir: pathlib.Path):
    """static/ copied to the output; PNG and JPEG images also as WebP when it is smaller (the pages prefer it)."""
    from PIL import Image

    cache = HERE / ".cache" / "webp"
    cache.mkdir(parents=True, exist_ok=True)
    for p in src_dir.rglob("*"):
        if p.is_dir():
            continue
        rel = p.relative_to(src_dir)
        dst = out_dir / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        if not dst.exists() or dst.stat().st_mtime < p.stat().st_mtime:
            shutil.copy2(p, dst)
        if p.suffix.lower() in (".png", ".jpg", ".jpeg"):
            key = hashlib.sha1((str(rel) + str(p.stat().st_mtime_ns)).encode()).hexdigest()
            c = cache / (key + ".webp")
            if not c.exists():
                tmp = c.with_suffix(f".{os.getpid()}.tmp")
                try:
                    with Image.open(p) as im:
                        lossless = p.suffix.lower() == ".png" and (im.mode in ("P", "L", "1") or len(im.getcolors(4096) or []) > 0)
                        im2 = im.convert("RGBA") if im.mode in ("P", "LA") else im
                        if lossless:
                            im2.save(tmp, "WEBP", lossless=True, method=6)
                        else:
                            im2.save(tmp, "WEBP", quality=86, method=6)
                    os.replace(tmp, c)
                except Exception:
                    try:
                        c.write_bytes(b"")
                    except OSError:
                        continue
            try:
                if c.stat().st_size and c.stat().st_size < p.stat().st_size * 0.92:
                    shutil.copy2(c, dst.with_suffix(".webp"))
            except OSError:
                pass


def picture(text: str, out_dir: pathlib.Path) -> str:
    """<img src="x.png"> as <picture> with the WebP made beside it, when there is one."""

    def rep(m):
        tag, src = m.group(0), m.group(1)
        if not src.startswith("/") or not re.search(r"\.(png|jpe?g)$", src, re.I):
            return tag
        webp = re.sub(r"\.(png|jpe?g)$", ".webp", src, flags=re.I)
        if (out_dir / webp[1:]).exists():
            return f'<picture><source srcset="{webp}" type="image/webp">{tag}</picture>'
        return tag

    return re.sub(r'<img src="([^"]+)"[^>]*>', rep, text)


def check_graphs():
    exe = None
    for n in ("terraflux.exe", "terraflux"):
        p = ROOT / "target" / "release" / n
        if p.exists():
            exe = p
    exe = exe or shutil.which("terraflux")
    if not exe:
        print("--check: no terraflux binary (target/release or the PATH)")
        return
    bad = 0
    with tempfile.TemporaryDirectory() as d:
        for i, (page, toml, opts) in enumerate(GRAPHS):
            if opts.get("fragment"):
                continue
            f = pathlib.Path(d) / f"m{i}.toml"
            text = toml if "format" in toml else 'format = "terraflux/1"\n' + toml
            f.write_text(text, encoding="utf-8")
            r = subprocess.run([str(exe), "check", str(f)], capture_output=True, text=True, encoding="utf-8")
            msg = (r.stdout + r.stderr).strip()
            errors = [l for l in msg.splitlines() if "error" in l.lower() and "no errors" not in l.lower()]
            # files a reader brings (paths) are not here
            errors = [l for l in errors if not re.search(r"path|file|not found|No such|cannot open|does not exist", l, re.I)]
            if r.returncode != 0 and errors:
                bad += 1
                print(f"CHECK {page} graph {i}: {msg}")
    print(f"--check: {len(GRAPHS)} graphs, {bad} with errors")


def build(out: pathlib.Path, only: str | None = None, check=False, offline=False):
    t0 = time.time()
    WARNINGS.clear()
    GRAPHS.clear()
    out.mkdir(parents=True, exist_ok=True)
    webp_copy(STATIC, out)
    # the logo and icons from the repository's assets
    for f in ("terraflux-logo-light.svg", "terraflux-logo-dark.svg", "terraflux-icon.svg", "terraflux-mark-light.svg", "terraflux-mark-dark.svg"):
        src = ROOT / "assets" / "logo" / f
        if src.exists():
            (out / "img").mkdir(exist_ok=True)
            shutil.copy2(src, out / "img" / f)
    for f in ("terraflux-icon-32.png", "terraflux-icon-256.png", "terraflux-icon-512.png"):
        src = ROOT / "assets" / "logo" / "png" / f
        if src.exists():
            shutil.copy2(src, out / "img" / f)
    for f in ("graph.png", "map.png", "validation.png"):
        src = ROOT / "docs" / "images" / f
        if src.exists():
            (out / "img" / "app").mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, out / "img" / "app" / f)
            if not (STATIC / "img" / "app" / f).exists():
                (STATIC / "img" / "app").mkdir(parents=True, exist_ok=True)
                shutil.copy2(src, STATIC / "img" / "app" / f)
    (out / "css").mkdir(exist_ok=True)
    (out / "css" / "pygments.css").write_text(pygments_css(), encoding="utf-8")

    env = jinja2.Environment(loader=jinja2.FileSystemLoader(TEMPLATES), autoescape=jinja2.select_autoescape(["html"]),
                             trim_blocks=True, lstrip_blocks=True)
    env.globals.update(site=SITE, year=dt.date.today().year, slug=slug, op_url=op_url, op_anchor=op_anchor)
    env.filters["md"] = lambda s: jinja2.utils.markupsafe.Markup(md_inline(s or ""))
    env.filters["cells"] = cells_svg

    pages = collect(only)
    all_pages = pages if not only else collect(None)
    for p in pages:
        render_markdown(p)
    cats = tutorial_tree(all_pages)
    asset_v = hashlib.sha1(b"".join((STATIC / x).read_bytes() for x in ("css/site.css", "js/site.js") if (STATIC / x).exists())).hexdigest()[:8]
    env.globals["asset_v"] = asset_v
    dl = downloads()
    search = []

    def write(url: str, html_text: str):
        if offline:
            html_text = OFFLINE.webp_only(OFFLINE.external(html_text), out)
        else:
            html_text = picture(html_text, out)
        html_text = relativize(html_text, url)
        if offline:
            html_text = OFFLINE.links(html_text)
        d = out / url.strip("/")
        d.mkdir(parents=True, exist_ok=True)
        (d / "index.html").write_text(html_text, encoding="utf-8")

    for p in pages:
        CURRENT["page"] = p
        prev, nxt = prev_next(p, all_pages, cats)
        tpl = p.meta.get("layout", "page.html")  # `template` names the application's template a page starts from
        ctx = dict(page=p, sidebar=sidebar(p, all_pages, cats), prev=prev, next=nxt, cats=cats,
                   series=series_info(p, cats), downloads=dl, root=rel_prefix(p.url))
        if p.category:
            ctx["category"] = next((c for c in cats if c["slug"] == p.category), None)
        write(p.url, env.get_template(tpl).render(**ctx))
        search.append(search_entry(p.url, p.title, ctx.get("category", {}).get("title") if ctx.get("category") else
                                   SITE["sections"].get(p.section, {}).get("title", ""), p.text, flat_toc(p.toc)))
        CURRENT["page"] = None

    # pages made from data
    home_graph, _, _ = RENDERER.render((HERE / "models" / "home.toml").read_text(encoding="utf-8"))
    write("/", env.get_template("home.html").render(cats=cats, downloads=dl, ops_count=len(OPS), root="./", page=None,
                                                    home_graph=home_graph, parity=PARITY, bibtex=bibtex()))
    write("/download/", env.get_template("download.html").render(downloads=dl, root=rel_prefix("/download/"), page=None))
    write("/tutorials/", env.get_template("tutorials.html").render(cats=cats, root=rel_prefix("/tutorials/"), page=None,
                                                                  sidebar={"kind": "tutorials", "cats": cats}))
    # a page per category of tutorials, its own address (/tutorials/<category>/), as the pages link to it
    for c in cats:
        url = f"/tutorials/{c['slug']}/"
        here = {"title": c["title"], "url": url, "category": c["slug"], "meta": {"description": c["blurb"]}}
        write(url, env.get_template("tutorials.html").render(cats=[c], root=rel_prefix(url), page=here,
                                                             heading=c["title"], lede=c["blurb"],
                                                             sidebar={"kind": "tutorials", "cats": cats}))
    opgroups = operators_pages()
    write("/operators/", env.get_template("operators_index.html").render(groups=opgroups, root=rel_prefix("/operators/"),
                                                                         page=None, total=len(OPS)))
    for g in opgroups:
        url = f"/operators/{g['slug']}/"
        write(url, env.get_template("operators.html").render(group=g, groups=opgroups, root=rel_prefix(url), page=None,
                                                             md_op=md_op))
        for o in g["ops"]:
            search.append({"u": url.lstrip("/") + "#" + op_anchor(o["name"]), "t": o["name"], "s": "Operators: " + g["title"],
                           "h": "", "x": o["doc"]})
    if offline:
        # files to open from the disk: the search as a script, KaTeX and the fonts within, the images in WebP only
        (out / "search.js").write_text(OFFLINE.search_script(search), encoding="utf-8")
        OFFLINE.vendor(out, HERE / ".cache" / "vendor")
        freed = OFFLINE.drop_png(out)
        (out / "README.txt").write_text(
            f"Terraflux {SITE['version']}: the website, to read without a network.\n\n"
            "Open index.html in a web browser (double-click it). Every page, figure, formula and the search work "
            "from these files.\n\nThe site online: " + SITE["url"] + "\n", encoding="utf-8")
        for w in WARNINGS:
            print("WARNING", w)
        print(f"built {len(pages)} pages, {len(opgroups)} operator pages to read offline into {out} in "
              f"{time.time() - t0:.1f} s ({freed / 1e6:.0f} MB of PNG left out; {len(WARNINGS)} warnings)")
        return
    # the page GitHub Pages shows for any missing address: links from the site's root, by <base>
    (out / "404.html").write_text(relativize(env.get_template("404.html").render(root=SITE["url"], page=None), "/"),
                                  encoding="utf-8")
    (out / "search.json").write_text(json.dumps(search, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    (out / ".nojekyll").write_text("", encoding="utf-8")
    urls = ["/", "/download/", "/tutorials/", "/operators/"] + [f"/tutorials/{c['slug']}/" for c in cats]
    urls += [p.url for p in all_pages] + [f"/operators/{g['slug']}/" for g in opgroups]
    base = SITE["url"].rstrip("/")
    (out / "sitemap.xml").write_text(
        '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        + "".join(f"  <url><loc>{base}{u}</loc></url>\n" for u in dict.fromkeys(urls))
        + "</urlset>\n", encoding="utf-8")
    (out / "robots.txt").write_text(f"User-agent: *\nAllow: /\nSitemap: {base}/sitemap.xml\n", encoding="utf-8")
    if check:
        check_graphs()
    for w in WARNINGS:
        print("WARNING", w)
    print(f"built {len(pages)} pages, {len(opgroups)} operator pages into {out} in {time.time() - t0:.1f} s ({len(WARNINGS)} warnings)")


PARITY = [
    ("GuidosToolbox", "MSPA, fragmentation, accounting", "Vogt & Riitters 2017"),
    ("landscapemetrics", "Patch, class and landscape metrics", "Hesselbarth et al. 2019"),
    ("Conefor", "Habitat graphs, the PC and IIC indices", "Saura & Torné 2009"),
    ("Circuitscape and Omniscape", "Current flow through resistance", "McRae et al. 2008"),
    ("gdistance", "Least-cost and random-walk distances", "van Etten 2017"),
    ("Zonation 5", "Priority ranking of a landscape", "Moilanen et al. 2022"),
    ("Marxan", "Reserve selection by simulated annealing", "Ball et al. 2009"),
    ("InVEST", "Carbon, habitat quality, water yield and sediment", "Nelson et al. 2009"),
    ("MaxNet and ENMeval", "Species distribution models", "Phillips et al. 2017"),
    ("Biodiverse", "Endemism and phylogenetic diversity", "Laffan et al. 2010"),
    ("Dinamica EGO", "Weights of evidence, Expander and Patcher", "Soares-Filho et al. 2002"),
    ("CLUE-S, lulcc and PLUS", "Demand-driven land allocation", "Verburg et al. 2002; Liang et al. 2021"),
]


def bibtex() -> str:
    a = SITE["author"].split()
    lines = [
        "@software{terraflux,",
        f"  author  = {{{a[-1]}, {' '.join(a[:-1])}}},",
        "  title   = {Terraflux: landscape ecology and land change",
        "             modelling from a laptop to a cluster},",
        f"  version = {{{SITE['version']}}},",
        f"  year    = {{{dt.date.today().year}}},",
        f"  url     = {{{SITE['repo']}}},",
        f"  license = {{{SITE['license']}}}",
        "}",
    ]
    return "\n".join(lines)


def md_op(text: str):
    return jinja2.utils.markupsafe.Markup(md_inline(text or ""))


def cells_svg(pattern: str, size: int = 22) -> jinja2.utils.markupsafe.Markup:
    """The small icon of a category: 3x3 cells as the logo's (forest, change)."""
    s = size / 3
    r = []
    for i, ch in enumerate(pattern[:9]):
        if ch == "0":
            cls = "c0"
        elif ch == "1":
            cls = "c1"
        else:
            cls = "c2"
        x, y = (i % 3) * s, (i // 3) * s
        r.append(f'<rect class="{cls}" x="{x + 0.6:.2f}" y="{y + 0.6:.2f}" width="{s - 1.2:.2f}" height="{s - 1.2:.2f}" rx="{s * 0.18:.2f}"/>')
    return jinja2.utils.markupsafe.Markup(
        f'<svg class="cells" viewBox="0 0 {size} {size}" width="{size}" height="{size}" aria-hidden="true">{"".join(r)}</svg>'
    )


def pygments_css() -> str:
    """The code's colours: a light style, and a dark one under [data-theme="dark"]; the background the page's."""
    from pygments.styles import get_all_styles

    dark_style = "github-dark" if "github-dark" in set(get_all_styles()) else "monokai"
    light = HtmlFormatter(style="friendly").get_style_defs(".highlight")
    dark = HtmlFormatter(style=dark_style).get_style_defs('[data-theme="dark"] .highlight')
    strip = lambda css: re.sub(r"(\.highlight \{[^}]*?)background: #[0-9a-fA-F]+;", r"\1", css)  # noqa: E731
    return strip(light) + "\n" + strip(dark) + "\n"


def serve(out: pathlib.Path, port: int, only):
    import http.server
    import threading

    def watch():
        def stamp():
            return max((p.stat().st_mtime for d in (CONTENT, STATIC, TEMPLATES) for p in d.rglob("*") if p.is_file()), default=0)

        last = stamp()
        while True:
            time.sleep(1)
            s = stamp()
            if s != last:
                last = s
                try:
                    build(out, only)
                except Exception as e:  # keep serving
                    print("build failed:", e)

    threading.Thread(target=watch, daemon=True).start()
    handler = lambda *a, **k: http.server.SimpleHTTPRequestHandler(*a, directory=str(out), **k)  # noqa: E731
    print(f"serving {out} on http://localhost:{port}/")
    http.server.ThreadingHTTPServer(("", port), handler).serve_forever()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=pathlib.Path, default=HERE / "dist")
    ap.add_argument("--serve", action="store_true")
    ap.add_argument("--port", type=int, default=8000)
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--only")
    ap.add_argument("--clean", action="store_true")
    ap.add_argument("--offline", action="store_true",
                    help="the site to open from the disk, without a server or a network, and a zip of it (default "
                         "folder: website/.cache/offline/terraflux-website-<version>)")
    a = ap.parse_args()
    if a.offline and a.out == HERE / "dist":
        a.out = HERE / ".cache" / "offline" / f"terraflux-website-{SITE['version']}"
        a.clean = True
    if a.clean and a.out.exists():
        shutil.rmtree(a.out)
    build(a.out, a.only, a.check, a.offline)
    if a.offline:
        zipped = shutil.make_archive(str(a.out), "zip", root_dir=a.out.parent, base_dir=a.out.name)
        print(f"{zipped}: {pathlib.Path(zipped).stat().st_size / 1e6:.0f} MB")
    if a.serve:
        serve(a.out, a.port, a.only)


if __name__ == "__main__":
    main()
