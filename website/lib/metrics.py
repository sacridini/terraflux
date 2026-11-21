"""The metrics catalog as the site's pages use it (website/data/metrics.json, made by tools/metrics_catalog.py).

A page's line `::: metrics landscape-metrics` becomes the family's intro, a heading for each theme and, under it, a
heading and a card for each metric; `::: metrics-index` becomes the filterable list of every metric of every family
(static/js/metrics.js filters it; without JavaScript it is a plain table). The cards' HTML is held as placeholders
until the page's Markdown is converted (`expand()`, then the caller's `fill`), as the API reference's objects are.

build.py gives the Catalog the few helpers it needs from itself (Markdown of a text, operator links, code
highlighting...), so this module does not import it.
"""

from __future__ import annotations

import html
import json
import pathlib
import re

from .pyapi import ApiError, esc

LEVEL_ORDER = ["patch", "class", "landscape", "cell", "map", "region", "node", "link", "network", "species", "model"]
DIRECTIVE = re.compile(r"^:::[ \t]+(metrics|metrics-index)(?:[ \t]+([\w-]+))?[ \t]*$")
FENCE = re.compile(r"^\s*(```|~~~)")


def cap(s: str) -> str:
    return s[:1].upper() + s[1:]


class Catalog:
    def __init__(self, data_file: pathlib.Path, h):
        """`h`: the helpers of build.py: md (Markdown block to HTML, with the library's cross-references), op_links
        (text with [[operator]] links), highlight (code to HTML), op_url, api_url (a Python path to a URL or None),
        ops (the operators), warn."""
        self.h = h
        self.loaded = data_file.is_file()
        d = json.loads(data_file.read_text(encoding="utf-8")) if self.loaded else {}
        self.version = d.get("library_version", "")
        self.families: list[dict] = d.get("families", [])
        self.by_slug = {f["family"]: f for f in self.families}
        self.pending: dict[str, str] = {}
        self._n = 0

    # ---------------------------------------------------------------------- the directives

    def scan(self, body: str):
        lines = body.split("\n")
        out, fenced = [], None
        for i, line in enumerate(lines):
            fm = FENCE.match(line)
            if fm:
                fenced = None if fenced == fm.group(1) else (fenced or fm.group(1))
            m = None if fenced else DIRECTIVE.match(line)
            if m:
                out.append((i, m.group(1), m.group(2)))
        return lines, out

    def expand(self, body: str, page: str) -> str:
        """The body with each `::: metrics x` and `::: metrics-index` line replaced by Markdown headings and
        placeholders of HTML (replaced after the Markdown is converted: `fill`)."""
        lines, found = self.scan(body)
        if not found:
            return body
        out, last = [], 0
        for i, kind, arg in found:
            out.extend(lines[last:i])
            last = i + 1
            if kind == "metrics-index":
                if arg:
                    raise ApiError(f"{page}: `::: metrics-index` takes no argument")
                out.append(self.hold(self.index_html()))
                continue
            fam = self.by_slug.get(arg or "")
            if not fam:
                raise ApiError(f"{page}: `::: metrics {arg}`: no such family in data/metrics.json "
                               f"(known: {', '.join(self.by_slug) or 'none: run tools/metrics_catalog.py'})")
            out.append(self.family_markdown(fam))
        out.extend(lines[last:])
        return "\n".join(out)

    def hold(self, html_text: str) -> str:
        self._n += 1
        token = f"METRICSBLOCK{self._n:05d}END"
        self.pending[token] = html_text
        return f"\n{token}\n"

    def fill(self, text: str) -> str:
        for token, h in self.pending.items():
            text = re.sub(rf"<p>\s*{token}\s*</p>|{token}", lambda _m, h=h: h, text)
        self.pending.clear()
        return text

    def family_markdown(self, fam: dict) -> str:
        parts = [f"\n**Source:** {fam['source']}. {len(fam['metrics'])} metrics, "
                 f"{sum(len(m.get('variants', [])) for m in fam['metrics'])} of them with statistics that share a card.\n",
                 fam["intro"].strip() + "\n"]
        themes: dict[str, list[dict]] = {}
        for m in fam["metrics"]:
            themes.setdefault(m["theme"], []).append(m)
        for theme, ms in themes.items():
            parts.append(f"\n## {cap(theme)}\n")
            for m in ms:
                parts.append(f"\n### {m['name']} {{#{m['id']}}}\n")
                parts.append(self.hold(self.card_html(fam, m)))
        return "\n".join(parts)

    # ---------------------------------------------------------------------- a card

    def text(self, s: str) -> str:
        return self.h["md"](self.h["op_links"](s))

    def plain(self, s: str) -> str:
        t = self.h["md_inline"](s)
        return html.unescape(re.sub(r"\s+", " ", re.sub(r"<[^>]+>", " ", t))).strip()

    def formula(self, f) -> str:
        return "".join(f'<div class="arithmatex">\\[ {esc(x.strip())} \\]</div>' for x in ([f] if isinstance(f, str) else f))

    def op_link(self, op: str) -> str:
        o = self.h["ops"].get(op)
        if not o:
            self.h["warn"](f"metrics: unknown operator {op}")
            return f"<code>{esc(op)}</code>"
        doc = o["doc"].split(". ")[0][:160].replace('"', "'")
        return f'<a class="op-link" href="{self.h["op_url"](op)}" title="{esc(doc)}"><code>{esc(op)}</code></a>'

    def api_link(self, path: str) -> str:
        url = self.h["api_url"](path)
        label = "tf." + path.removeprefix("terraflux.")
        return f'<a href="{esc(url)}"><code>{esc(label)}</code></a>' if url else f"<code>{esc(label)}</code>"

    def value_html(self, value: str) -> str:
        if "\n" in value:
            return f'<pre class="mc-out">{esc(value)}</pre>'
        return f"<strong>{esc(value)}</strong>"

    def card_html(self, fam: dict, m: dict) -> str:
        sym = "".join(f'<dt><span class="arithmatex">\\({esc(k)}\\)</span></dt><dd>{self.h["md_inline"](v)}</dd>'
                      for k, v in m["symbols"].items())
        py = [f'<div class="mc-py">{self.h["highlight"](m["python"], "python")}</div>']
        if m.get("value"):
            py.append(f'<p class="mc-val">On <a href="#example">the example</a> of this page: {self.value_html(m["value"])}</p>')
        if m.get("api"):
            py.append(f'<p class="mc-note">All its options: {self.api_link(m["api"])}.</p>')
        app = ""
        if m.get("operator"):
            setting = f' Select it with <code>{esc(m["setting"])}</code>.' if m.get("setting") else ""
            app = (f'<section class="mc-part"><p class="mc-h">In the application</p>'
                   f'<p>The operator {self.op_link(m["operator"])} computes it.{setting}</p></section>')
        refs = ""
        if m.get("refs"):
            refs = ('<div class="mc-refs"><p class="mc-h">References</p><ul>'
                    + "".join(f"<li>{self.h['md_inline'](r)}</li>" for r in m["refs"]) + "</ul></div>")
        aliases = f'<p class="mc-alias">Also searched as {", ".join(esc(a) for a in m["aliases"])}.</p>' if m.get("aliases") else ""
        return (
            f'<div class="mc" data-metric="{esc(m["id"])}">'
            f'<p class="mc-meta"><code class="mc-id">{esc(m["id"])}</code><span class="mc-tag">{esc(m["level"])} level</span>'
            f'<span class="mc-tag">{esc(m["theme"])}</span></p>'
            f'<div class="mc-grid">'
            f'<section class="mc-part"><p class="mc-h">What it measures</p>{self.text(m["what"])}'
            f'<p class="mc-facts"><span><b>Unit</b> {esc(m["unit"])}</span><span><b>Range</b> {esc(m["range"])}</span></p>'
            f'<p class="mc-h mc-h2">When to use it</p>{self.text(m["use"])}{aliases}</section>'
            f'<section class="mc-part"><p class="mc-h">Formula</p>{self.formula(m["formula"])}'
            f'<dl class="mc-sym">{sym}</dl></section>'
            f'<section class="mc-part"><p class="mc-h">In Python</p>{"".join(py)}</section>'
            f'{app}</div>'
            f'{self.variants_html(m)}{refs}</div>'
        )

    def variants_html(self, m: dict) -> str:
        vs = m.get("variants")
        if not vs:
            return ""
        rows = "".join(
            f'<tr id="{esc(v["id"])}"><td><code>{esc(v["id"])}</code></td><td>{esc(v["level"])}</td><td>{esc(v["label"])}</td>'
            f'<td>{esc(v["value"])}</td><td>{self.op_link(v["operator"])} <code>{esc(v["setting"])}</code></td></tr>' for v in vs)
        seen, defs = set(), []
        for v in vs:
            if v["stat"] not in seen:
                seen.add(v["stat"])
                defs.append(f'<dt>{esc(v["label"])}</dt><dd><span class="arithmatex">\\( {esc(v["formula"])} \\)</span></dd>')
        return (
            f'<details class="mc-variants"><summary>Its statistics at the class and landscape level ({len(vs)} metrics)</summary>'
            '<p>The patch values of this card, summarised over the patches of each class or of the whole map: each row is a '
            'metric of its own, with the card\'s meaning and unit.</p>'
            '<div class="table-wrap"><table><thead><tr><th>Metric</th><th>Level</th><th>Statistic</th>'
            '<th>On the example map</th><th>In the application</th></tr></thead>'
            f'<tbody>{rows}</tbody></table></div>'
            f'<dl class="mc-sym mc-defs">{"".join(defs)}</dl>'
            '<p class="mc-note">Over \\(n\\) patches with values \\(x_i\\) and areas \\(a_i\\).</p></details>'
        )

    # ---------------------------------------------------------------------- the index

    def rows(self):
        """Every metric (cards and variants) as a dict: name, id, family slug and title, theme, level, unit, url, text."""
        for fam in self.families:
            base = f"/metrics/{fam['family']}/"
            for m in fam["metrics"]:
                yield {"name": m["name"], "id": m["id"], "family": fam["family"], "ftitle": fam["title"], "theme": m["theme"],
                       "level": m["level"], "unit": m["unit"], "url": f"{base}#{m['id']}", "what": m["what"],
                       "aliases": m.get("aliases", []), "variant_of": None}
                for v in m.get("variants", []):
                    yield {"name": v["name"], "id": v["id"], "family": fam["family"], "ftitle": fam["title"],
                           "theme": m["theme"], "level": v["level"], "unit": m["unit"], "url": f"{base}#{v['id']}",
                           "what": f"{v['label']} of {m['name'].lower()}: {v['text']}.", "aliases": m.get("aliases", []),
                           "variant_of": m["name"]}

    def index_html(self) -> str:
        rows = list(self.rows())
        levels = sorted({r["level"] for r in rows}, key=lambda x: (LEVEL_ORDER.index(x) if x in LEVEL_ORDER else 99, x))
        themes = sorted({r["theme"] for r in rows})
        fams = [(f["family"], f["title"]) for f in self.families]

        def opts(label, items):
            return (f'<label class="mc-f"><span>{label}</span><select data-filter="{label.lower()}"><option value="">All</option>'
                    + "".join(f'<option value="{esc(v)}">{esc(t)}</option>' for v, t in items) + "</select></label>")

        fam_cards = "".join(
            f'<a class="mc-fam" href="/metrics/{f["family"]}/"><strong>{esc(f["title"])}</strong>'
            f'<span>{esc(f["source"])}</span><em>{sum(1 for r in rows if r["family"] == f["family"])} metrics</em></a>'
            for f in self.families)
        trs = []
        for r in rows:
            hay = " ".join([r["name"], r["id"], r["ftitle"], r["theme"], r["level"], *r["aliases"],
                            self.plain(r["what"])[:200]]).lower()
            via = f'<span class="mc-via">statistic of {esc(r["variant_of"].lower())}</span>' if r["variant_of"] else ""
            trs.append(
                f'<tr data-family="{esc(r["family"])}" data-theme="{esc(r["theme"])}" data-level="{esc(r["level"])}" '
                f'data-s="{esc(hay)}"><td><a href="{esc(r["url"])}">{esc(r["name"])}</a>{via}</td>'
                f'<td><code>{esc(r["id"])}</code></td><td>{esc(r["level"])}</td><td>{esc(r["theme"])}</td>'
                f'<td>{esc(r["ftitle"])}</td><td>{esc(r["unit"])}</td></tr>')
        return (
            f'<div class="mc-index"><div class="mc-fams">{fam_cards}</div>'
            '<div class="mc-filters" hidden><label class="mc-f mc-q"><span>Search</span>'
            '<input type="search" data-filter="q" placeholder="Name, abbreviation or what it measures" autocomplete="off" '
            'spellcheck="false"></label>'
            + opts("Family", fams) + opts("Theme", [(t, cap(t)) for t in themes]) + opts("Level", [(v, cap(v)) for v in levels])
            + '<p class="mc-count" role="status" aria-live="polite"></p></div>'
            '<div class="table-wrap"><table class="mc-table"><thead><tr><th>Metric</th><th>Id</th><th>Level</th><th>Theme</th>'
            f'<th>Family</th><th>Unit</th></tr></thead><tbody>{"".join(trs)}</tbody></table></div>'
            '<p class="mc-none" hidden>No metric matches. Try fewer words or clear a filter.</p></div>'
        )

    # ---------------------------------------------------------------------- search

    def search_entries(self, search_entry, urls: set[str]):
        """The site's search entries: a card and each of its variants, titled with the name, the id and the aliases."""
        out = []
        for r in self.rows():
            if f"/metrics/{r['family']}/" not in urls:
                continue
            title = f"{r['name']} ({r['id']})" + (f" · {', '.join(r['aliases'])}" if r["aliases"] else "")
            out.append(search_entry(r["url"], title, "Metrics: " + r["ftitle"], self.plain(r["what"]), [cap(r["theme"]), r["level"]]))
        return out
