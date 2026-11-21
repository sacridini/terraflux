"""Build the Terraflux website: content/*.md and templates/ into dist/ (static files, for GitHub Pages).

    python website/build.py                 # dist/
    python website/build.py --serve         # build, then serve on http://localhost:8000 (rebuilds on change)
    python website/build.py --check         # also check every ```tfgraph model with `terraflux check`
    python website/build.py --out DIR       # another output folder
    python website/build.py --only PATH     # build only the pages under content/PATH (and the site's chrome)

Needs Python 3.11+ with markdown, pymdown-extensions, jinja2, pyyaml, pygments and pillow (griffelib only for
tools/refresh_python_api.py, which makes data/python_api.json). A page's `::: terraflux.x` line (lib/pyapi.py) and
`{{< include path >}}` line are expanded here; see AUTHORING.md, "The Python library's pages".
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
from lib.pyapi import Api, ApiError  # noqa: E402
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


def template_groups() -> dict[str, str]:
    """The group in the application's Templates menu of each template, by name (read from crates/app/src/templates.rs,
    where the menu is made)."""
    src = (ROOT / "crates/app/src/templates.rs").read_text(encoding="utf-8")
    consts = dict(re.findall(r'^const (\w+): &str = "([^"]+)";', src, re.M))
    return {n: consts[g] for n, g in re.findall(r'name: "([^"]+)",\s*group: (\w+),', src) if g in consts}


TEMPLATE_GROUPS = template_groups()


def in_application(p) -> str | None:
    """How a tutorial's model opens in the application (the Templates menu, the Help > Tutorials with a model window),
    or None when it has no model: the page's template, and the model files published under static/data/tutorials."""
    parts = p.src[:-3].split("/")
    published = []
    if len(parts) == 3 and parts[0] == "tutorials":
        published = sorted((STATIC / "data/tutorials" / parts[1] / parts[2]).glob("*.toml"))
    template = p.meta.get("template")
    if template:
        group = TEMPLATE_GROUPS.get(template)
        menu = f"Templates &gt; {html.escape(group)} &gt; {html.escape(template)}" if group else "the Templates menu"
        return f"{menu}, or Help &gt; Tutorials with a model"
    if published:
        return f"Help &gt; Tutorials with a model &gt; {html.escape(p.title)}"
    return None
# the Python library's API reference (tools/refresh_python_api.py), used by the `::: terraflux.x` directive of a page
API = Api(DATA / "python_api.json")
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
    # an entry equal to the full name wins over a prefix (raster.stats is Raster analysis, not Data preparation)
    for s in SUBJECTS["subjects"]:
        if op in s["prefixes"]:
            return s["title"]
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
        # {{version}}: the version of Terraflux the site describes (data/cli.json), kept current at each release
        self.body = text.replace("{{version}}", SITE["version"]).replace("{{pyversion}}", API.version)
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


HTML_TAGS = set("""a abbr address area article aside b base bdi bdo blockquote body br button canvas caption cite code
col colgroup dd del details dfn div dl dt em figcaption figure footer h1 h2 h3 h4 h5 h6 head header hgroup hr html i img
input ins kbd li link main mark meta nav ol p picture pre q rp rt ruby s samp script section small source span strong
style sub summary sup table tbody td tfoot th thead title tr u ul var wbr math mi mn mo mrow msup msub mfrac msqrt mtext
annotation semantics svg g path rect circle ellipse line polyline polygon text tspan textpath defs marker lineargradient
radialgradient stop clippath mask pattern symbol use image foreignobject desc filter fegaussianblur feoffset feblend
fecolormatrix fecomposite feflood femerge femergenode fedropshadow feturbulence fedisplacementmap femorphology""".split())
# (form and interactive tags such as data, map, time, template or dialog are left out on purpose: no page writes them,
# and operator docs use some of those words as placeholders)
# Tags the pages do write but texts also use as placeholders (<output>, <label>): a tag when it opens with attributes
# (the sliders' <label class="sl-label"> and <output class="sl-value">, the tabs' <label for="__tabbed_1_1">) or
# closes, a placeholder when it is bare.
ATTR_TAGS = {"label", "output"}


def _known_tag(name: str, attrs: str = "", closing: bool = False) -> bool:
    name = name.lower()
    if name in ATTR_TAGS:
        return closing or bool(attrs.strip())
    return name in HTML_TAGS or "-" in name


def escape_unknown_tags(html: str) -> str:
    """Escapes, in rendered HTML, a `<name>` that is no HTML, MathML or SVG tag: texts write placeholders such as
    `<variable>_<statistic>` or `d.<name>` outside code, which a browser takes for tags and hides. Code is already
    escaped by then, so only such stray placeholders match."""
    def one(m):
        # `percentile_<p>`: a <p> glued to a word is a placeholder
        if m.group(4) or not _known_tag(m.group(2), m.group(3) or "", m.group(1) == "/"):
            return m.group(0).replace("<", "&lt;").replace(">", "&gt;")
        return m.group(0)
    return re.sub(r"<(/?)([A-Za-z_][\w.-]*)((?:\s[^<>]*)?)>|(?<=\w)<(p)>", one, html)


def _escape_placeholders(text: str) -> str:
    """The same on a short Markdown text (an operator's doc), outside its code spans, before it is rendered: there a
    `<p>` is never a paragraph the text means."""
    parts = re.split(r"(`[^`]*`)", text)
    for i in range(0, len(parts), 2):
        parts[i] = re.sub(r"<(/?)([A-Za-z_][\w.-]*)>",
                          lambda m: m.group(0) if _known_tag(m.group(2)) and m.group(2).lower() != "p"
                          else m.group(0).replace("<", "&lt;").replace(">", "&gt;"), parts[i])
    return "".join(parts)


def md_inline(text: str) -> str:
    out = markdown.markdown(_escape_placeholders(text), extensions=["pymdownx.inlinehilite", "pymdownx.arithmatex"],
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
        "y": ys or [k for k in rows[0].keys() if k not in (spec.get("x"), spec.get("color_by"))],
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
    # benchmark options (horizontal bars, log value axis, reference line, dot plot, colour by a column)
    kind = cfg["type"]
    if kind not in ("line", "area", "bar", "scatter", "dot"):
        warn(f"tfchart: type {kind!r} is not line, area, bar, scatter or dot")
    for k in ("horizontal", "log", "labels_at_end"):
        if spec.get(k):
            cfg[k] = True
    for k in ("format", "color_by", "min", "max", "digits"):
        if spec.get(k) is not None:
            cfg[k] = spec[k]
    if spec.get("ylog"):
        cfg["log"] = True
    rule = spec.get("rule")
    if rule is not None:
        if isinstance(rule, dict) and isinstance(rule.get("value"), (int, float)):
            cfg["rule"] = {"value": rule["value"], "label": str(rule.get("label", ""))}
        else:
            warn("tfchart: rule needs a value (rule: { value: 1, label: same speed })")
    if cfg.get("log"):
        nums = [r[k] for r in rows for k in cfg["y"] if isinstance(r.get(k), float)]
        if any(v <= 0 for v in nums):
            warn("tfchart: log axis with values <= 0")
    if cfg.get("color_by"):
        if cfg["color_by"] not in rows[0]:
            warn(f"tfchart: color_by column {cfg['color_by']!r} not in the data")
        elif len({r.get(cfg["color_by"]) for r in rows}) > 5:
            warn("tfchart: color_by has more than 5 groups (the categorical palette has 5 validated slots)")
    title = f'<div class="ch-title">{esc(cfg["title"])}</div>' if cfg["title"] else ""
    data = "" if linked or spec.get("data_table") is False else chart_data_html(cfg, rows)
    return (
        f'<div class="tf-chart" data-chart=\'{esc(json.dumps(cfg))}\'>{title}<div class="ch-plot" role="img" '
        f'aria-label="{esc(cfg["title"] or "chart")}"></div><div class="ch-legend"></div>{data}</div>'
    )


def num_txt(v: float, digits: int | None = None) -> str:
    """A number as the tables show it: grouped thousands, `digits` decimals if given, else about three digits."""
    if digits is not None:
        return f"{v:,.{digits}f}"
    if v == int(v) and abs(v) < 1e15:
        return f"{int(v):,}"
    d = 0 if abs(v) >= 1000 else 1 if abs(v) >= 100 else 2 if abs(v) >= 10 else 3
    return f"{v:,.{d}f}".rstrip("0").rstrip(".")


def chart_data_html(cfg: dict, rows: list[dict]) -> str:
    """The chart's numbers as a table under a 'Show the data' toggle (a chart is never the only way to read a value)."""
    labels = cfg["labels"]
    xk = cfg["x"]
    cols = [xk] + ([cfg["color_by"]] if cfg.get("color_by") in rows[0] else []) + [k for k in cfg["y"] if k != xk]
    fmt_s = cfg.get("format") or ("{v}%" if cfg.get("percent") else "{v}")
    head = "".join(f"<th scope=\"col\">{esc(labels.get(c, cfg['xlabel'] if c == xk and cfg['xlabel'] else c))}</th>" for c in cols)
    body = []
    for r in rows:
        cells = []
        for c in cols:
            v = r.get(c, "")
            if isinstance(v, float) and c == xk:
                cells.append(f'<td class="num">{esc(f"{v:g}")}</td>')
            elif isinstance(v, float):
                cells.append(f'<td class="num">{esc(fmt_s.replace("{v}", num_txt(v, cfg.get("digits"))))}</td>')
            else:
                cells.append(f"<td>{esc(v)}</td>")
        body.append("<tr>" + "".join(cells) + "</tr>")
    return (
        '<details class="ch-data"><summary>Show the data</summary><div class="table-wrap"><table>'
        f"<thead><tr>{head}</tr></thead><tbody>{''.join(body)}</tbody></table></div></details>"
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
    `labels` renames them, `digits` rounds numbers (one number, or one per column), `format` gives a column's text
    (`{v}×`), `align` aligns a column, `badge` turns a column's values into status pills (the text stays), `link` turns
    the file paths in a column (`tools/x.py`) into links under a base address, `heat: true` shades numeric cells by
    value (a matrix) and `sortable: true` sorts by a click on a header."""
    s = spec_of(source, "tftable")
    rows = read_table(s)
    if not rows:
        warn("tftable without rows")
        return ""
    cols = s.get("columns") or list(rows[0].keys())
    for c in cols:
        if c not in rows[0]:
            warn(f"tftable: column {c!r} not in the data")
    labels = s.get("labels", {})
    digits = s.get("digits", 3)
    dmap = digits if isinstance(digits, dict) else {}
    dflt = dmap.get("default", 3) if isinstance(digits, dict) else digits
    fmts = s.get("format") or {}
    aligns = s.get("align") or {}
    if isinstance(aligns, str):
        aligns = {c: aligns for c in cols[1:]}
    badges = s.get("badge") or {}
    links = s.get("link") or {}
    tones = {"good": "good", "warning": "warning", "bad": "bad", "danger": "bad", "info": "info", "neutral": "info"}
    glyph = {"good": "✓", "warning": "!", "bad": "✕", "info": ""}
    sortable = bool(s.get("sortable"))
    nums = [r[c] for r in rows for c in cols[1:] if isinstance(r.get(c), float)]
    lo, hi = (min(nums), max(nums)) if nums else (0, 1)

    def cell(v, c, first):
        al = aligns.get(c)
        if al is not None and al not in ("left", "center", "right"):
            warn(f"tftable: align {al!r} is not left, center or right")
            al = None
        bmap = badges.get(c)
        if bmap and str(v) in {str(k) for k in bmap}:
            tone = next(tones.get(str(t), "info") for k, t in bmap.items() if str(k) == str(v))
            g = glyph[tone]
            pill = f'<span class="pill pill-{tone}">' + (f'<span aria-hidden="true">{g}</span> ' if g else "") + f"{esc(v)}</span>"
            cls = f' class="a-{al}"' if al else ""
            return f'<td{cls} data-v="{esc(str(v).lower())}">{pill}</td>'
        if isinstance(v, float):
            if c in dmap:
                txt = f"{v:,.{dmap[c]}f}"
            else:
                txt = f"{int(v):,}" if v == int(v) and abs(v) < 1e15 else f"{v:,.{dflt}f}".rstrip("0").rstrip(".")
            if c in fmts:
                txt = str(fmts[c]).replace("{v}", txt)
            style = ""
            if s.get("heat") and not first and hi > lo:
                t = (v - lo) / (hi - lo)
                style = f' style="background: color-mix(in srgb, var(--forest) {round(t * 55)}%, transparent)"'
            cls = "num" + (f" a-{al}" if al else "")
            return f'<td class="{cls}" data-v="{v}"{style}>{esc(txt)}</td>'
        cls = f' class="a-{al}"' if al else ""
        if c in links and isinstance(v, str) and v:
            base = str(links[c])
            html = re.sub(r"(tools/[\w./-]+\.(?:py|R|patch))", lambda m: f'<a href="{esc(base + m.group(1))}">{m.group(1)}</a>', esc(v))
            return f'<td{cls} data-v="{esc(str(v).lower())}">{html}</td>'
        return f'<td{cls} data-v="{esc(str(v).lower())}">{esc(v)}</td>'

    def th(c):
        al = aligns.get(c)
        if al is None and any(isinstance(r.get(c), float) for r in rows) and c != cols[0]:
            al = "right"
        cls = f' class="a-{al}"' if al in ("left", "center", "right") else ""
        t = esc(labels.get(c, c))
        if sortable:
            return f'<th scope="col"{cls} aria-sort="none"><button type="button" class="th-sort">{t}<span class="sort-ind" aria-hidden="true"></span></button></th>'
        return f'<th scope="col"{cls}>{t}</th>'

    head = "".join(th(c) for c in cols)
    body = "".join(
        f'<tr data-i="{n}">' + "".join(cell(r.get(c, ""), c, i == 0) for i, c in enumerate(cols)) + "</tr>"
        for n, r in enumerate(rows)
    )
    cap = s.get("caption", "")
    cid = "tt-" + hashlib.sha1(source.encode("utf-8")).hexdigest()[:8]
    label = f' aria-labelledby="{cid}"' if cap else ""
    return (
        f'<figure class="tf-table{" is-sortable" if sortable else ""}"><div class="table-wrap"><table{label}><thead><tr>{head}</tr></thead>'
        f"<tbody>{body}</tbody></table></div>"
        + (f'<figcaption id="{cid}">{md_inline(cap)}</figcaption>' if cap else "")
        + "</figure>"
    )


def calc_version() -> str:
    """A hash of the playgrounds' files (their scripts, styles, engine and data), for the browsers' caches."""
    files = ["js/calc.js", "css/calc.css", "js/calc-engine.js", "data/calculators/raster.js", "data/calculators/vector.js"]
    return hashlib.sha1(b"".join((STATIC / f).read_bytes() for f in files if (STATIC / f).exists())).hexdigest()[:8]


def fence_calc(source, language, class_name, options, md, **kwargs):
    """A calculator's playground (static/js/calc.js): a script over Sibuyan's maps (`kind: raster`, raster.calc) or
    its patches (`kind: vector`, vector.calc and vector.filter), computed in the page by Terraflux's own engine
    built to WebAssembly (tools/build_calc.py), on the data of figures/calculators/playground.py. `presets`: the
    examples offered (name, script; for features op, column, inputs), `start` the one shown first."""
    s = spec_of(source, "tfcalc")
    kind = s.get("kind", "raster")
    presets = s.get("presets") or []
    if kind not in ("raster", "vector"):
        warn(f"tfcalc: kind {kind!r} is neither raster nor vector")
    if not presets:
        warn("tfcalc without presets")
        return ""
    for p in presets:
        if not p.get("name") or not p.get("script"):
            warn("tfcalc: a preset without a name or a script")
    spec = {"kind": kind, "presets": presets, "start": s.get("start", 0)}
    v = calc_version()
    first = presets[spec["start"]]["script"]
    cap = s.get("caption", "")
    return (
        f'<figure class="tf-calc-fig"><div class="tf-calc" data-spec=\'{esc(json.dumps(spec))}\' data-v="{v}" '
        f'data-src="/data/calculators/{kind}.js?v={v}"><noscript><p>The playground needs JavaScript. Its first '
        f'example:</p>{highlight(first, "python")}</noscript></div>'
        + (f"<figcaption>{md_inline(cap)}</figcaption>" if cap else "")
        + "</figure>"
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
        {"name": "tfcalc", "class": "tfcalc", "format": fence_calc},
    ]
    return {
        "toc": {"permalink": "#", "permalink_title": "Link to this section", "toc_depth": "2-3", "slugify": slugify_unicode},
        "pymdownx.superfences": {"custom_fences": fences},
        "pymdownx.highlight": {"css_class": "highlight", "guess_lang": False, "use_pygments": True},
        "pymdownx.tabbed": {"alternate_style": True},
        "pymdownx.arithmatex": {"generic": True},
        "footnotes": {"BACKLINK_TITLE": "Back to the text"},
    }


# ------------------------------------------------------------------------------------------------ Python API reference

DOC_MD = None  # the Markdown of docstrings: one converter, reset between texts
INCLUDE = re.compile(r"^\{\{<\s*include\s+(\S+)\s*>\}\}[ \t]*$", re.M)


def doc_md(text: str, inline=False) -> str:
    """A docstring's Markdown as HTML (its `[x][terraflux.y]` references linked, RST's `::` dropped)."""
    global DOC_MD
    if DOC_MD is None:
        exts = ["tables", "def_list", "admonition", "sane_lists", "pymdownx.superfences", "pymdownx.highlight",
                "pymdownx.inlinehilite", "pymdownx.arithmatex", "pymdownx.betterem"]
        DOC_MD = markdown.Markdown(extensions=exts, extension_configs={
            "pymdownx.highlight": {"css_class": "highlight", "guess_lang": False, "use_pygments": True},
            "pymdownx.arithmatex": {"generic": True}})
    text = re.sub(r"::[ \t]*$", ":", API.autorefs(text), flags=re.M)
    DOC_MD.reset()
    out = DOC_MD.convert(text)
    if inline and out.count("<p>") == 1 and out.startswith("<p>") and out.endswith("</p>"):
        out = out[3:-4]
    return out


def api_table(head: list[str], rows: list[list[str]]) -> str:
    th = "".join(f"<th>{h}</th>" for h in head)
    body = "".join("<tr>" + "".join(f"<td>{c}</td>" for c in r) + "</tr>" for r in rows)
    return f'<table><thead><tr>{th}</tr></thead><tbody>{body}</tbody></table>'


def api_code(s: str) -> str:
    return f"<code>{esc(s)}</code>" if s else ""


def api_sections(obj: dict) -> str:
    """The parsed docstring of an object as HTML: summary, text, tables, examples, notes."""
    out = []
    if obj.get("summary"):
        out.append(f'<div class="api-summary">{doc_md(obj["summary"])}</div>')
    for sec in obj["doc"]:
        t = sec["t"]
        if t == "text":
            out.append(f'<div class="api-text">{doc_md(sec["x"])}</div>')
        elif t in ("params", "attrs"):
            items = sec["i"]
            has_type = any(i[1] for i in items)
            has_def = t == "params" and any(i[3] for i in items)
            head = ["Name"] + (["Type"] if has_type else []) + (["Default"] if has_def else []) + ["Description"]
            rows = []
            for i in items:
                # names written together in the docstring ("a, b" one description) share a row
                if rows and i[2] and rows[-1][0] == i[2]:
                    rows[-1][1].append(i)
                else:
                    rows.append((i[2], [i]))
            rows = [[("<br>".join(api_code(x[0]) for x in g))] + (["<br>".join(api_code(x[1]) for x in g)] if has_type else [])
                    + (["<br>".join(api_code(x[3]) or "&nbsp;" for x in g)] if has_def else [])
                    + [doc_md(d, inline=True)] for d, g in rows]
            out.append(f'<p class="api-sec">{esc(sec["h"])}</p>' + api_table(head, rows))
        elif t == "prose":
            out.append(f'<p class="api-sec">{esc(sec["h"])}</p><div class="api-text">{doc_md(sec["x"])}</div>')
        elif t == "returns":
            items = sec["i"]
            has_name = any(i[0] for i in items)
            has_desc = any(i[2] for i in items)
            head = (["Name"] if has_name else []) + ["Type"] + (["Description"] if has_desc else [])
            rows = [([api_code(i[0])] if has_name else []) + [api_code(i[1])] + ([doc_md(i[2], inline=True)] if has_desc else [])
                    for i in items]
            out.append(f'<p class="api-sec">{esc(sec["h"])}</p>' + api_table(head, rows))
        elif t == "raises":
            rows = [[api_code(i[0]), doc_md(i[1], inline=True)] for i in sec["i"]]
            out.append(f'<p class="api-sec">{esc(sec["h"])}</p>' + api_table(["Type", "Description"], rows))
        elif t == "ex":
            parts = []
            for kind, text in sec["i"]:
                parts.append(doc_md(text) if kind == "t" else highlight(text, "pycon"))
            out.append(f'<p class="api-sec">{esc(sec["h"])}</p><div class="api-examples">{"".join(parts)}</div>')
        elif t == "adm":
            kind = re.sub(r"[^a-z]+", "-", sec["h"].lower()).strip("-")
            cls = {"notes": "note", "note": "note", "see-also": "info", "warnings": "warning", "references": "info",
                   "reference": "info"}.get(kind, "note")
            out.append(f'<div class="admonition {cls} api-adm"><p class="admonition-title">{esc(sec["h"])}</p>'
                       f'{doc_md(sec["x"])}</div>')
    return "".join(out)


def api_kind(obj: dict) -> str:
    return {"classmethod": "class method", "staticmethod": "static method"}.get(obj["kind"], obj["kind"])


def api_source(obj: dict) -> str:
    if not obj.get("src"):
        return ""
    url = f'{API.repo}/blob/v{API.version}/{obj["src"]}#L{obj["line"]}'
    return f'<a class="api-src" href="{esc(url)}" rel="noopener">source</a>'


def api_head(obj: dict, public: str, root: bool, show_path: bool, source: bool = True) -> str:
    bases = ""
    if obj.get("bases"):
        bases = f' <span class="api-bases">({", ".join(esc(b) for b in obj["bases"])})</span>' if root else ""
    typ = f' <span class="api-type">{esc(obj["type"])}</span>' if obj.get("type") else ""
    path = f' <code class="api-path">{esc(public)}</code>' if show_path else ""
    return f'<p class="api-meta"><span class="api-kind">{api_kind(obj)}</span>{path}{bases}{typ} {api_source(obj) if source else ""}</p>'


def render_api(api: Api, public: str, canon: str, opts: dict, level: int) -> str:
    """The reference of an object (the directive's expansion): signature, summary, tables, examples, members."""
    obj = api.objects[canon]
    show_sig = opts.get("show_signature", True)
    source = opts.get("show_source", True)
    parts = [f'<div class="api-obj api-{obj["kind"].replace(" ", "-")}">', api_head(obj, public, True, True, source)]
    if show_sig and obj.get("sig"):
        parts.append(highlight(obj["sig"], "python"))
    elif obj.get("value") and show_sig:
        parts.append(highlight(f'{obj["name"]} = {obj["value"]}', "python"))
    if opts.get("show_docstring", True):
        parts.append(api_sections(obj))
    for m, tail in api.members_of(canon, opts):
        mo = api.objects[m]
        mp = public + tail
        parts.append(
            f'<div class="api-member"><h{min(level + 1, 6)} id="{esc(mp)}" class="api-mh"><code>{esc(mo["name"])}</code>'
            f'<a class="headerlink" href="#{esc(mp)}" title="Link to this section">#</a></h{min(level + 1, 6)}>'
            f'{api_head(mo, mp, False, False, source)}'
            + (highlight(mo["sig"], "python") if show_sig and mo.get("sig") else "")
            + (highlight(f'{mo["name"]} = {mo["value"]}', "python") if show_sig and mo.get("value") else "")
            + api_sections(mo) + "</div>")
    parts.append("</div>")
    return "".join(parts)


def expand_includes(text: str, page: Page) -> str:
    """`{{< include path >}}` on a line of its own: the file (a path from the repository's root, such as
    extern/terraflux-py/CHANGELOG.md) read at build time; its leading `# Title` line is dropped (the page has its
    own title)."""

    def rep(m):
        rel = m.group(1)
        p = (ROOT / rel).resolve()
        if ROOT not in p.parents or not p.is_file():
            msg = f"include: {rel} not found (the submodule extern/terraflux-py: git submodule update --init)"
            if os.environ.get("CI"):
                raise ApiError(f"{page.src}: {msg}")
            warn(msg)
            return f"*The text of this page is read from `{rel}` when the site is built, and it was not there.*"
        body = p.read_text(encoding="utf-8")
        return re.sub(r"\A\s*#[ \t][^\n]*\n", "", body)

    return INCLUDE.sub(rep, text)


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
    """Wraps each bare <table> of the Markdown in a scrolling box; the tables the components write (tftable, a chart's
    data) come already wrapped, and a closing tag added to them would close the page's column early."""
    return re.sub(r'(?<!<div class="table-wrap">)<table>(.*?)</table>', r'<div class="table-wrap"><table>\1</table></div>',
                  text, flags=re.S)


def _id_refs(h: str) -> str:
    """Gives each <li> of a list of references the id of its reference (ref_key)."""
    def one(m):
        plain = html.unescape(re.sub(r"<[^>]+>", "", m.group(1)))
        return f'<li id="{ref_key(plain)}">{m.group(1)}</li>'
    return re.sub(r"<li>(.*?)</li>", one, h, flags=re.S)


def render_markdown(page: Page) -> None:
    CURRENT["page"] = page
    cfg = md_config()
    if page.meta.get("toc_depth"):  # front matter `toc_depth: 2` keeps only the headings down to h2 in the contents
        cfg["toc"]["toc_depth"] = f"2-{int(page.meta['toc_depth'])}"
    md = markdown.Markdown(extensions=MD_EXT, extension_configs=cfg)
    body = expand_includes(page.body, page)
    body = API.expand(body, page.src, render_api)
    body = API.autorefs(body)
    body = op_links(body)
    out = API.fill(md.convert(body))
    out = figures(out)
    out = _wrap_code(out)
    out = tables(out)
    out = escape_unknown_tags(out)
    if page.src == "about/citing.md":  # an anchor on each reference, for the operators' "Cite" lines
        head, sep, tail = out.partition('<h2 id="references"')
        out = head + sep + _id_refs(tail)
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
    title = SITE["sections"].get(page.section, {}).get("title", "")
    entries = section_entries(page.section, pages)
    if entries is not None:
        return {"kind": "grouped", "title": title, "entries": entries}
    items = [p for p in pages if p.section == page.section]
    items.sort(key=lambda p: (not p.is_index, p.order, p.title))
    return {"kind": "section", "title": title, "pages": items}


def group_of(p: Page) -> str | None:
    """The subfolder of a page inside its section (content/<section>/<sub>/x.md), None for a page of the section."""
    parts = p.src.split("/")
    return parts[1] if len(parts) > 2 else None


def section_entries(section: str, pages: list[Page]):
    """The sidebar of a section whose pages live in subfolders: its own pages and a group for each subfolder, in
    one list ordered by `order` (the section's index first). A group's heading is its title from the section's
    `groups:` mapping in site.yaml (a text, or {title, order}), else the title of its lead page (the page at
    /<section>/<sub>/, either <sub>/index.md or <sub>.md, listed first as "Overview"), else the folder's name; its
    place among the pages is the mapping's `order`, else the lead page's, else last. None for a section with no
    subfolder (then the sidebar is the flat list of its pages)."""
    items = [p for p in pages if p.section == section]
    subs: dict[str, list[Page]] = {}
    for p in items:
        if group_of(p):
            subs.setdefault(group_of(p), []).append(p)
    if not subs:
        return None
    cfg = SITE["sections"].get(section, {}).get("groups", {}) or {}
    leads = {sub: next((p for p in items if p.url == f"/{section}/{sub}/"), None) for sub in subs}
    entries = []
    for p in items:
        if not group_of(p) and p not in leads.values():
            entries.append({"key": (not p.is_index, p.order, p.title), "page": p})
    for sub, ps in subs.items():
        c = cfg.get(sub)
        lead = leads[sub]
        title = (c if isinstance(c, str) else (c or {}).get("title")) or (lead and (lead.meta.get("short") or lead.title)) \
            or sub.replace("-", " ").capitalize()
        order = (c.get("order") if isinstance(c, dict) else None)
        order = order if order is not None else (lead.order if lead else 999)
        rest = sorted([p for p in ps if p is not lead], key=lambda p: (p.order, p.title))
        entries.append({"key": (True, order, title), "group": {"title": title, "slug": sub, "lead": lead, "pages": rest}})
    entries.sort(key=lambda e: e["key"])
    return entries


def prev_next(page: Page, pages: list[Page], cats):
    if page.section == "tutorials":
        seq = [p for c in cats for g in c["groups"] for p in g["pages"]]
    elif (entries := section_entries(page.section, pages)) is not None:
        seq = []
        for e in entries:
            if "page" in e:
                seq.append(e["page"])
            else:
                seq += ([e["group"]["lead"]] if e["group"]["lead"] else []) + e["group"]["pages"]
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


def og_image(page) -> str:
    """The absolute URL of a page's sharing image: its front matter `image:`, else the first PNG or JPEG of its text
    (crawlers read neither SVG nor WebP), else img/social.png."""
    base = SITE["url"]
    src = None
    if page is not None and hasattr(page, "meta"):
        src = page.meta.get("image")
        if not src:
            for m in re.finditer(r'<img src="(/img/[^"]+\.(?:png|jpe?g))"', getattr(page, "html", ""), re.I):
                if (STATIC / m.group(1)[1:]).is_file():
                    src = m.group(1)
                    break
    if not src:
        return base + "img/social.png"
    return src if src.startswith("http") else base + src.lstrip("/")


def operator_usage(pages: list[Page], cats):
    """Per operator: the pages that list it in their `ops` (tutorials by category and order, then other pages), and the
    application's templates (models/templates/*.toml) with a node of it, linked to the page that starts from them."""
    rank = {c["slug"]: i for i, c in enumerate(cats)}
    key = lambda p: (rank.get(p.category, len(rank)) if p.category else len(rank), p.order, p.title)
    used: dict[str, list[dict]] = {}
    for p in sorted((p for p in pages if p.meta.get("ops")), key=key):
        for o in dict.fromkeys(p.meta["ops"]):
            used.setdefault(o, []).append({"title": p.meta.get("short") or p.title, "url": p.url})
    by_template = {}
    for p in pages:
        if p.meta.get("template"):
            by_template.setdefault(p.meta["template"], p.url)
    tmpl: dict[str, list[dict]] = {}
    for f in sorted((HERE / "models" / "templates").glob("*.toml")):
        text = f.read_text(encoding="utf-8")
        m = re.search(r'^name\s*=\s*"([^"]+)"', text, re.M)
        if not m:
            continue
        for o in dict.fromkeys(re.findall(r'^\s*op\s*=\s*"([^"]+)"', text, re.M)):
            tmpl.setdefault(o, []).append({"title": m.group(1), "url": by_template.get(m.group(1))})
    return used, tmpl


CITING = CONTENT / "about" / "citing.md"


def ref_key(text: str) -> str:
    """A reference's anchor from its APA line or its short citation: the first author's family name and the year
    ("Soille & Vogt (2009)" and "Soille, P., & Vogt, P. (2009). ..." both give ref-soille-2009)."""
    fam = re.split(r",| &| et al\.| \(", text.strip(), 1)[0]
    year = re.search(r"\((\d{4})\)", text).group(1)
    return "ref-" + slug(f"{fam} {year}")


def citation_rules():
    """The methods table and the references of the citing page (`## The methods`, `## References`), read when the site
    is built so that the page is the one source: ([{method, pats, related, cites: [(label, key)]}], {key: APA text})."""
    text = CITING.read_text(encoding="utf-8")
    sect = lambda h: re.search(rf"^## {h}\n(.*?)(?=^## |\Z)", text, re.S | re.M).group(1)
    refs = {}
    for line in sect("References").splitlines():
        if line.startswith("- "):
            refs[ref_key(line[2:])] = line[2:].replace("*", "").strip()
    rules = []
    for line in sect("The methods").splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) != 3 or cells[0] in ("Method", "---") or set(cells[0]) <= set("-"):
            continue
        cites = []
        for c in cells[2].split(";"):
            c = c.strip()
            if ref_key(c) not in refs:
                warn(f"{CITING.name}: the citation '{c}' has no reference in the list")
            cites.append((c, ref_key(c)))
        rules.append({"method": cells[0], "pats": re.findall(r"`([^`]+)`", cells[1]),
                      "related": "related" in cells[1], "cites": cites})
    return rules, refs


def operator_citations(names):
    """Per operator, the methods of the citing page that apply to it: [{method, cites: [{label, url, apa}]}]. A pattern
    is `prefix.*`, `prefix*` or an exact name; "X and related" takes the operators of X's group that no other row
    of the group names (the prioritization's, apart from Zonation and the multi-action planning)."""
    rules, refs = citation_rules()

    def hit(pat: str, op: str) -> bool:
        if pat.endswith("*"):
            return op.startswith(pat[:-1])
        return op == pat

    out: dict[str, list[dict]] = {}
    for r in rules:
        for pat in r["pats"]:
            group = pat.split(".")[0] + "."
            others = [q for o in rules if o is not r and not o["related"] for q in o["pats"] if q.startswith(group)]
            for op in names:
                if hit(pat, op) or (r["related"] and op.startswith(group) and not any(hit(q, op) for q in others)):
                    cur = out.setdefault(op, [])
                    if not any(c["method"] == r["method"] for c in cur):
                        cur.append({"method": r["method"],
                                    "cites": [{"label": l, "url": "/about/citing/#" + k, "apa": refs.get(k, "")} for l, k in r["cites"]]})
    return out


def node_skeleton(o: dict) -> str | None:
    """A node of the operator as a model file writes it: its required inputs wired to placeholders (`upstream.<type>`,
    the node and port that give it), its required settings as placeholders and the optional ones named in a comment.
    Containers hold a body, so they get none."""
    if o.get("container"):
        return None
    ids = o["name"].split(".", 1)[1]
    if ids == "upstream":  # not the placeholder's own name
        ids = o["name"].replace(".", "_")
    lines = ["[[nodes]]", f'id = "{ids}"', f'op = "{o["name"]}"']
    ins = []
    for i in o["inputs"]:
        if not i["required"]:
            continue
        port = {"raster": "map", "any": "map"}.get(i["type"], i["type"])
        ins.append(f'{i["name"]} = 1' if i["type"] in ("integer", "number") else f'{i["name"]} = "upstream.{port}"')
    if ins:
        lines.append("inputs = { " + ", ".join(ins) + " }")
    req = [f'{p["name"]} = ' + ("1" if p["type"] in ("integer", "number") else f'"<{p["name"]}>"')
           for p in o["params"] if p["required"]]
    if req:
        lines.append("params = { " + ", ".join(req) + " }")
    if ins or req:
        lines[0:0] = ['# "upstream.map" stands for the node and port that give an input; "<name>" and 1 for values to fill']
    opt = [p["name"] for p in o["params"] if not p["required"]]
    if opt:  # their defaults are the operator's own: a node leaves them out
        lines.append("# optional settings: " + ", ".join(opt))
    return "\n".join(lines)


def skeleton_html(o: dict) -> str:
    code = node_skeleton(o)
    if not code:
        return ""
    return ('<details class="op-node"><summary>As a node in a model file</summary>'
            f'<div class="codeblock"><button class="copy" type="button" aria-label="Copy">Copy</button>{highlight(code, "toml")}</div></details>')



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
    # TERRAFLUX_BIN names another build (a worktree's, while the MCP servers hold target/release)
    exe = pathlib.Path(os.environ["TERRAFLUX_BIN"]) if os.environ.get("TERRAFLUX_BIN") else None
    for n in ("terraflux.exe", "terraflux"):
        p = ROOT / "target" / "release" / n
        if p.exists() and exe is None:
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


def urls_of_pages(all_pages, cats, opgroups) -> list[str]:
    """Every address the build writes a page at (the sitemap's list)."""
    urls = ["/", "/download/", "/tutorials/", "/operators/"] + [f"/tutorials/{c['slug']}/" for c in cats]
    return urls + [p.url for p in all_pages] + [f"/operators/{g['slug']}/" for g in opgroups]


REDIRECT_PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Moved: {title}</title>
<meta http-equiv="refresh" content="0; url={target}">
<link rel="canonical" href="{canon}">
<meta name="robots" content="noindex">
</head>
<body>
<p>This page has moved to <a href="{link}">{link_text}</a>.</p>
</body>
</html>
"""


def write_redirects(out: pathlib.Path, pages: set[str]):
    """`redirects:` of site.yaml: <out>/<old path>/index.html sends the reader to the new address.

    Not in the sitemap or the search index (they are made from the pages). A redirect whose old path is a page, or
    whose target is not one, fails the build."""
    base = SITE["url"].rstrip("/")
    for old, new in (SITE.get("redirects") or {}).items():
        old = "/" + old.strip("/") + "/"
        path, _, anchor = new.partition("#")
        if old in pages:
            raise SystemExit(f"redirects: {old} is a page; remove the redirect or the page")
        if path not in pages:
            raise SystemExit(f"redirects: {old} -> {new}: {path} is not a page")
        target = rel_prefix(old) + new[1:]  # relative, as the site's links are
        d = out / old.strip("/")
        d.mkdir(parents=True, exist_ok=True)
        (d / "index.html").write_text(REDIRECT_PAGE.format(
            title=html.escape(new), target=html.escape(target, quote=True), canon=html.escape(base + path, quote=True),
            link=html.escape(target, quote=True), link_text=html.escape(new)), encoding="utf-8")


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
    env.globals.update(site=SITE, year=dt.date.today().year, slug=slug, op_url=op_url, op_anchor=op_anchor, og_image=og_image)
    env.filters["md"] = lambda s: jinja2.utils.markupsafe.Markup(md_inline(s or ""))
    env.filters["cells"] = cells_svg

    pages = collect(only)
    all_pages = pages if not only else collect(None)
    # where each documented Python object lives, for the links to it from any page
    API.index.clear()
    API.documented.clear()
    API.unresolved.clear()
    for p in all_pages:
        API.register(p.body, p.src, p.url)
    for p in pages:
        render_markdown(p)
    cats = tutorial_tree(all_pages)
    asset_v = hashlib.sha1(b"".join((STATIC / x).read_bytes() for x in ("css/site.css", "js/site.js") if (STATIC / x).exists())).hexdigest()[:8]
    env.globals["asset_v"] = asset_v
    env.globals["calc_v"] = calc_version()
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
                   series=series_info(p, cats), downloads=dl, root=rel_prefix(p.url), page_url=p.url,
                   in_app=in_application(p))
        if p.category:
            ctx["category"] = next((c for c in cats if c["slug"] == p.category), None)
        write(p.url, env.get_template(tpl).render(**ctx))
        search.append(search_entry(p.url, p.title, ctx.get("category", {}).get("title") if ctx.get("category") else
                                   SITE["sections"].get(p.section, {}).get("title", ""), p.text, flat_toc(p.toc)))
        CURRENT["page"] = None

    # pages made from data
    home_graph, _, _ = RENDERER.render((HERE / "models" / "home.toml").read_text(encoding="utf-8"))
    write("/", env.get_template("home.html").render(cats=cats, downloads=dl, ops_count=len(OPS), root="./", page=None,
                                                    home_graph=home_graph, parity=PARITY, bibtex=bibtex(), page_url="/",
                                                    py_example=highlight(PY_EXAMPLE, "python")))
    write("/download/", env.get_template("download.html").render(downloads=dl, root=rel_prefix("/download/"), page=None, page_url="/download/"))
    write("/tutorials/", env.get_template("tutorials.html").render(cats=cats, root=rel_prefix("/tutorials/"), page=None, page_url="/tutorials/",
                                                                  sidebar={"kind": "tutorials", "cats": cats}))
    # a page per category of tutorials, its own address (/tutorials/<category>/), as the pages link to it
    for c in cats:
        url = f"/tutorials/{c['slug']}/"
        here = {"title": c["title"], "url": url, "category": c["slug"], "meta": {"description": c["blurb"]}}
        write(url, env.get_template("tutorials.html").render(cats=[c], root=rel_prefix(url), page=here, page_url=url,
                                                             heading=c["title"], lede=c["blurb"],
                                                             sidebar={"kind": "tutorials", "cats": cats}))
    for url, anchor, canon in API.documented:
        o = API.objects[canon]
        plain = re.sub(r"\[`?([^\]`]*)`?\]\[[^\]]*\]", r"\1", o["summary"]).replace("`", "")
        search.append({"u": url.lstrip("/") + "#" + anchor, "t": anchor, "s": "Python API", "h": "", "x": plain})
    opgroups = operators_pages()
    used_in, templates_of = operator_usage(all_pages, cats)
    cites = operator_citations(list(OPS))
    since_file = DATA / "ops_since.json"  # tools/ops_since.py; operators of the first snapshot have no "Since"
    sinces = json.loads(since_file.read_text(encoding="utf-8")) if since_file.is_file() else {"floor": None, "ops": {}}
    since = {n: v for n, v in sinces["ops"].items() if v != sinces["floor"]}
    skeletons = {n: skeleton_html(o) for n, o in OPS.items()}
    print(f"operators: {len(cites)} cited, {len(since)} with a version, {sum(1 for v in skeletons.values() if v)} with a node")
    write("/operators/", env.get_template("operators_index.html").render(groups=opgroups, root=rel_prefix("/operators/"), page_url="/operators/",
                                                                         page=None, total=len(OPS)))
    for g in opgroups:
        url = f"/operators/{g['slug']}/"
        write(url, env.get_template("operators.html").render(group=g, groups=opgroups, root=rel_prefix(url), page=None, page_url=url,
                                                             md_op=md_op, used_in=used_in, templates_of=templates_of,
                                                             cites=cites, since=since, skeletons=skeletons))
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
    write_redirects(out, set(urls_of_pages(all_pages, cats, opgroups)))
    # the page GitHub Pages shows for any missing address: links from the site's root, by <base>
    (out / "404.html").write_text(relativize(env.get_template("404.html").render(root=SITE["url"], page=None), "/"),
                                  encoding="utf-8")
    (out / "search.json").write_text(json.dumps(search, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    (out / ".nojekyll").write_text("", encoding="utf-8")
    urls = urls_of_pages(all_pages, cats, opgroups)
    base = SITE["url"].rstrip("/")
    (out / "sitemap.xml").write_text(
        '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        + "".join(f"  <url><loc>{base}{u}</loc></url>\n" for u in dict.fromkeys(urls))
        + "</urlset>\n", encoding="utf-8")
    (out / "robots.txt").write_text(f"User-agent: *\nAllow: /\nSitemap: {base}/sitemap.xml\n", encoding="utf-8")
    if check:
        check_graphs()
        if API.unresolved:
            names = sorted(set(API.unresolved))
            print(f"--check: {len(API.unresolved)} references to Python objects no page documents "
                  f"({len(names)} objects; shown as plain code): {', '.join(names)}")
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
    ("InVEST", "Carbon, habitat quality, water, sediment, nutrients and pollination", "Nelson et al. 2009"),
    ("MaxNet and ENMeval", "Species distribution models", "Phillips et al. 2017"),
    ("Biodiverse", "Endemism and phylogenetic diversity", "Laffan et al. 2010"),
    ("Dinamica EGO", "Weights of evidence, Expander and Patcher", "Soares-Filho et al. 2002"),
    ("CLUE-S, lulcc and PLUS", "Demand-driven land allocation", "Verburg et al. 2002; Liang et al. 2021"),
    ("prioritizr", "Conservation plans by integer linear programming", "Hanson et al. 2025"),
    ("SWAT+ and WhiteboxTools", "Watersheds, flow routing and the SWAT+ model", "Bieger et al. 2017; Lindsay 2016"),
    ("move2 and amt", "GPS tracks cleaned, measured and turned into steps", "Kranstauber et al. 2024; Signer et al. 2019"),
    ("NLMR", "Neutral landscape models with the same seeds", "Sciaini et al. 2018"),
]

# The Python example on the home page (runs as it is: the library's example data).
PY_EXAMPLE = """import terraflux as tf

landcover = tf.load_raster(tf.example_data("clc"))
forest = tf.reclassify(landcover, foreground=[23, 24, 25])   # CORINE's forests

morph = tf.mspa(forest)      # GuidosToolbox's MSPA, the same numbers
tf.plot(morph)"""


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
    try:
        build(a.out, a.only, a.check, a.offline)
    except ApiError as e:
        sys.exit(f"build failed: {e}")
    if a.offline:
        zipped = shutil.make_archive(str(a.out), "zip", root_dir=a.out.parent, base_dir=a.out.name)
        print(f"{zipped}: {pathlib.Path(zipped).stat().st_size / 1e6:.0f} MB")
    if a.serve:
        serve(a.out, a.port, a.only)


if __name__ == "__main__":
    main()
