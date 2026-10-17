"""The website as files to open without a server or a network (`build.py --offline`): to share on a USB stick or in
a zip.

Pages opened from the disk (file://) differ from pages served: a link to a folder shows the folder, not its
index.html; `fetch` of a file is refused; nothing from the internet comes. So, in the offline build:

- every link to a page names its index.html;
- the search index is a script (search.js) instead of a JSON fetched;
- KaTeX (the formulas) and the fonts are copied into vendor/ from their CDNs (downloaded once, kept in
  website/.cache/vendor);
- the images are only WebP where the build made one (the browsers of today all show it), the PNG left out;
- the files for a web host (sitemap, robots, 404, .nojekyll) are left out.
"""

from __future__ import annotations

import json
import pathlib
import re
import urllib.request

KATEX = "https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/"
FONTS_CSS = ("https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;600&family=Newsreader:opsz,wght@6..72,"
             "450;6..72,560;6..72,640&family=Source+Sans+3:ital,wght@0,400;0,600;0,700;1,400&display=swap")
# Google serves WOFF2 to browsers it knows
UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36"


def _get(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def _cached(cache: pathlib.Path, rel: str, url: str) -> bytes:
    p = cache / rel
    if not p.exists():
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(_get(url))
    return p.read_bytes()


def vendor(out: pathlib.Path, cache: pathlib.Path) -> None:
    """KaTeX and the fonts into out/vendor (from the cache, downloaded the first time)."""
    dst = out / "vendor"
    for rel in ("katex.min.css", "katex.min.js", "contrib/auto-render.min.js"):
        b = _cached(cache / "katex", rel, KATEX + rel)
        (dst / "katex" / rel).parent.mkdir(parents=True, exist_ok=True)
        (dst / "katex" / rel).write_bytes(b)
    # the fonts KaTeX's CSS names: WOFF2 is the first of each list, the one browsers take
    css = (cache / "katex" / "katex.min.css").read_text(encoding="utf-8")
    # only the WOFF2 of each font (the WOFF and TTF after it are for old browsers, not copied)
    (dst / "katex" / "katex.min.css").write_text(
        re.sub(r',url\(fonts/[^)]+\.(?:woff|ttf)\) format\("[^"]+"\)', "", css), encoding="utf-8")
    for f in sorted(set(re.findall(r"url\((fonts/[^)]+\.woff2)\)", css))):
        b = _cached(cache / "katex", f, KATEX + f)
        (dst / "katex" / f).parent.mkdir(parents=True, exist_ok=True)
        (dst / "katex" / f).write_bytes(b)
    css = _cached(cache / "fonts", "fonts.css", FONTS_CSS).decode("utf-8")
    (dst / "fonts").mkdir(parents=True, exist_ok=True)

    def local(m):
        url = m.group(1)
        name = url.rstrip("/").split("/")[-1]
        # the same file name may come from two families: keep the folder of the family in the name
        key = re.sub(r"[^A-Za-z0-9.]+", "-", url.split("//", 1)[-1].split("/", 1)[-1])
        b = _cached(cache / "fonts", key, url)
        (dst / "fonts" / key).write_bytes(b)
        return f"url({key})" if name else m.group(0)

    (dst / "fonts" / "fonts.css").write_text(re.sub(r"url\((https://[^)]+)\)", local, css), encoding="utf-8")


def external(html_text: str) -> str:
    """The CDN's addresses as the vendored files' (absolute from the site's root, before the links are made
    relative)."""
    html_text = html_text.replace(KATEX, "/vendor/katex/")
    html_text = re.sub(r'<link rel="preconnect" href="https://fonts\.(googleapis|gstatic)\.com"[^>]*>\n?', "", html_text)
    return re.sub(r'<link rel="stylesheet" href="https://fonts\.googleapis\.com/css2\?[^"]*">',
                  '<link rel="stylesheet" href="/vendor/fonts/fonts.css">', html_text)


def webp_only(html_text: str, out: pathlib.Path) -> str:
    """Every /....png that has a WebP beside it in the output, as the WebP (in attributes and in data alike)."""

    def rep(m):
        path = m.group(1)
        return path + (".webp" if (out / (path[1:] + ".webp")).exists() else ".png")

    return re.sub(r"(/[A-Za-z0-9_./\-]+?)\.png\b", rep, html_text)


def drop_png(out: pathlib.Path) -> int:
    """The PNG that have a WebP beside them (no page names them any more): their bytes."""
    freed = 0
    for p in out.rglob("*.png"):
        if p.with_suffix(".webp").exists():
            freed += p.stat().st_size
            p.unlink()
    return freed


def _index(url: str) -> str:
    """A relative link to a folder as its index.html, the query and the fragment kept."""
    m = re.match(r"([^?#]*)([?#].*)?$", url)
    path, rest = m.group(1), m.group(2) or ""
    if path in ("", ".") and rest.startswith("#"):
        return url  # a fragment of this page
    if path == "" or path.endswith("/") or path in (".", ".."):
        path = (path.rstrip("/") + "/" if path not in ("", ".") else "./") + "index.html"
        if path.startswith("/"):
            path = path[1:]
    return path + rest


def links(html_text: str) -> str:
    """The relative links of a page (after `relativize`) to folders as their index.html."""

    def rep(m):
        attr, q, url = m.group(1), m.group(2), m.group(3)
        if re.match(r"[a-z][a-z0-9+.-]*:", url, re.I) or url.startswith(("//", "#")):
            return m.group(0)
        return f"{attr}={q}{_index(url)}"

    return re.sub(r'\b(href)=(["\'])([^"\']*)', rep, html_text)


def search_script(search: list[dict]) -> str:
    """The search index as a script setting window.TF_SEARCH, its addresses naming the index.html."""
    entries = [{**e, "u": _index(e["u"]) if e["u"] else "index.html"} for e in search]
    return "window.TF_SEARCH=" + json.dumps(entries, ensure_ascii=False, separators=(",", ":")) + ";\n"
