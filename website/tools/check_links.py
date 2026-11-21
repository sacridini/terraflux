"""Check the built site's internal links: every relative href/src of website/dist/**/*.html must name a file that
exists, and an #anchor on a page must be an id of that page.

    python website/build.py && python website/tools/check_links.py            # all pages
    python website/tools/check_links.py tutorials/fragmentation               # pages under a folder of dist/
    python website/tools/check_links.py --dist OUT                             # a site built with --out OUT

Build into a fresh folder for this check: dist/ keeps the pages of earlier builds that no longer have a source.

Prints each broken link (page, href, why) and exits 1 if there is one. External links (http:, mailto:...) are not
checked. The redirect pages' targets are checked like any link.
"""

from __future__ import annotations

import html
import pathlib
import re
import sys
import urllib.parse

DIST = pathlib.Path(__file__).resolve().parent.parent / "dist"
LINK = re.compile(r"""\b(?:href|src)\s*=\s*["']([^"']+)["']""", re.I)
REFRESH = re.compile(r"""http-equiv=["']refresh["'][^>]*content=["'][^;]*;\s*url=([^"']+)["']""", re.I)
ID = re.compile(r"""\bid\s*=\s*["']([^"']+)["']""", re.I)
_ids: dict[pathlib.Path, set[str]] = {}


def ids(page: pathlib.Path) -> set[str]:
    if page not in _ids:
        _ids[page] = {html.unescape(m) for m in ID.findall(page.read_text(encoding="utf-8", errors="replace"))}
    return _ids[page]


def target(page: pathlib.Path, href: str) -> tuple[pathlib.Path | None, str]:
    href = html.unescape(href)
    if re.match(r"^[a-z][a-z0-9+.-]*:", href, re.I) or href.startswith("//"):
        return None, ""
    path, _, anchor = href.partition("#")
    path = urllib.parse.unquote(path.split("?")[0])
    if not path:
        return page, anchor
    t = (page.parent / path) if not path.startswith("/") else (DIST / path.lstrip("/"))
    t = pathlib.Path(*t.parts).resolve() if t.exists() else t
    if t.is_dir() or path.endswith("/"):
        t = t / "index.html"
    return t, anchor


def main() -> None:
    global DIST
    args = sys.argv[1:]
    if args[:1] == ["--dist"]:
        DIST, args = pathlib.Path(args[1]).resolve(), args[2:]
    under = [DIST / a for a in args] or [DIST]
    pages = sorted({p for u in under for p in (u.rglob("*.html") if u.is_dir() else [u])})
    if not pages:
        sys.exit(f"no pages under {', '.join(map(str, under))} (build the site first)")
    bad = 0
    for page in pages:
        text = page.read_text(encoding="utf-8", errors="replace")
        hrefs = set(LINK.findall(text)) | set(REFRESH.findall(text))
        for href in sorted(hrefs):
            t, anchor = target(page, href)
            if t is None:
                continue
            why = ""
            if not t.exists():
                why = "no such file"
            elif anchor and t.suffix == ".html" and anchor not in ids(t):
                why = f"no id #{anchor}"
            if why:
                bad += 1
                print(f"{page.relative_to(DIST).as_posix()}: {href} ({why})")
    print(f"{len(pages)} pages, {bad} broken links")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
