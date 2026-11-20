"""Write the application's index of the tutorials: crates/app/src/tutorials.json.

- the categories of website/site.yaml (`tutorial_categories`), in the site's order;
- in each, the pages of website/content/tutorials/<category>/ in the site's order (`order`, then the title; the
  category's index apart): the slug, the title, the `template:` of the front matter (the name of a template of the
  application, or null), the models published under website/static/data/tutorials/<category>/<page>/*.toml and all
  the files of that folder (the data the models read), both as paths of the site.

    python website/tools/tutorial_index.py

The file is committed and read by the application (src/tutorials.rs), whose test fails when it is out of date.
"""

import json
import pathlib

import yaml

HERE = pathlib.Path(__file__).resolve().parent
WEBSITE = HERE.parent
OUT = WEBSITE.parent / "crates/app/src/tutorials.json"


def front_matter(path: pathlib.Path) -> dict:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---"):
        return {}
    return yaml.safe_load(text[3 : text.index("\n---", 3)]) or {}


def index() -> dict:
    site = yaml.safe_load((WEBSITE / "site.yaml").read_text(encoding="utf-8"))
    content = WEBSITE / "content/tutorials"
    models = WEBSITE / "static/data/tutorials"
    categories = []
    for c in site["tutorial_categories"]:
        pages = []
        for f in (content / c["slug"]).glob("*.md"):
            if f.name == "index.md":
                continue
            meta = front_matter(f)
            pages.append((meta.get("order", 999), meta.get("title", f.stem), f.stem, meta.get("template")))
        pages.sort(key=lambda p: (p[0], p[1]))
        tutorials = []
        for _, title, slug, template in pages:
            folder = models / c["slug"] / slug
            found = sorted(folder.glob("*.toml"), key=lambda f: f.name)
            files = sorted((f for f in folder.glob("*") if f.is_file()), key=lambda f: f.name)
            tutorials.append(
                {
                    "slug": slug,
                    "title": title,
                    "template": template,
                    "models": [f"/data/tutorials/{c['slug']}/{slug}/{m.name}" for m in found],
                    "files": [f"/data/tutorials/{c['slug']}/{slug}/{f.name}" for f in files],
                }
            )
        categories.append({"slug": c["slug"], "title": c["title"], "tutorials": tutorials})
    return {"categories": categories}


if __name__ == "__main__":
    OUT.write_text(json.dumps(index(), indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {OUT}")
