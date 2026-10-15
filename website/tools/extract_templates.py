"""Extract the application's templates (crates/app/src/templates.rs) into website/models/templates/*.toml."""
import re, pathlib
root = pathlib.Path(__file__).resolve().parents[2]
src = (root / "crates/app/src/templates.rs").read_text(encoding="utf-8")
out = root / "website/models/templates"
out.mkdir(parents=True, exist_ok=True)
for block in src.split("    Template {")[1:]:
    m = re.match(r'\s*name: "([^"]+)",\s*group: (\w+),.*?toml: r#"(.*?)"#', block, re.S)
    if not m:
        continue
    name, group, toml = m.groups()
    slug = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
    (out / f"{slug}.toml").write_text(toml.lstrip(), encoding="utf-8")
    print(slug, group)
