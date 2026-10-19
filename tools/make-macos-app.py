# SPDX-License-Identifier: GPL-3.0-or-later
"""Terraflux's macOS application and disk image, in target/macos:

    Terraflux.app                              the application (double click; its terraflux is the command too)
    terraflux-<version>-macos-<arch>.dmg       the application and a link to Applications, to drag it there
    terraflux-<version>-macos-<arch>.zip       the same application, to unpack anywhere

The application holds terraflux (the release build; OpenMP is linked into it) and a GDAL: its libraries and
those they need in Contents/Frameworks/gdal, relinked to load from there (@loader_path), its data and PROJ's
(proj.db, without the grids of datums) in Contents/Resources. Nothing else is needed. Everything is signed ad
hoc, which macOS asks of code on Apple silicon; an application signed so and downloaded is opened the first time
by a right click and Open (or `xattr -dr com.apple.quarantine Terraflux.app`).

    python3 tools/make-macos-app.py                      # GDAL of Homebrew (brew install gdal)
    python3 tools/make-macos-app.py --gdal /opt/local    # another GDAL's prefix (its lib/ and share/)
    python3 tools/make-macos-app.py --skip-build --no-dmg
"""

import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SYSTEM = ("/usr/lib/", "/System/")


def run(*args, capture=False):
    r = subprocess.run([str(a) for a in args], check=True, capture_output=capture, text=True)
    return r.stdout if capture else None


def brew_prefix(formula):
    try:
        return Path(run("brew", "--prefix", formula, capture=True).strip())
    except (OSError, subprocess.CalledProcessError):
        return None


def macho(path):
    with open(path, "rb") as f:
        return f.read(4) in (b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe")


def load_commands(path):
    """The libraries a Mach-O file loads and the rpaths it searches."""
    deps, rpaths = [], []
    lines = run("otool", "-l", path, capture=True).splitlines()
    for i, line in enumerate(lines):
        line = line.strip()
        if line in ("cmd LC_LOAD_DYLIB", "cmd LC_LOAD_WEAK_DYLIB", "cmd LC_REEXPORT_DYLIB"):
            deps.append(lines[i + 2].split()[1])
        elif line == "cmd LC_RPATH":
            rpaths.append(lines[i + 2].split()[1])
    return deps, rpaths


def resolve(ref, loader, rpaths):
    """Where a reference of `loader` points to, or None for the system's own libraries."""
    if ref.startswith(SYSTEM):
        return None
    here = str(Path(loader).parent)
    if ref.startswith("@loader_path/") or ref.startswith("@executable_path/"):
        return Path(os.path.realpath(ref.replace(ref.split("/")[0], here, 1)))
    if ref.startswith("@rpath/"):
        for rp in rpaths:
            p = Path(rp.replace("@loader_path", here).replace("@executable_path", here)) / ref[len("@rpath/") :]
            if p.exists():
                return Path(os.path.realpath(p))
        raise SystemExit(f"{loader}: {ref} not found in its rpaths {rpaths}")
    return Path(os.path.realpath(ref))


def bundle_gdal(gdal_lib, frameworks):
    """GDAL and every library it needs (but the system's) in `frameworks`, relinked to each other there."""
    plugins_src = gdal_lib.parent / "gdalplugins"
    roots = [gdal_lib]
    if plugins_src.is_dir():
        roots += sorted(p for p in plugins_src.iterdir() if p.suffix in (".dylib", ".so") and macho(p))
    # file → the name it is kept under; references → what each file's references are changed to
    names, refs, todo = {}, {}, [(Path(os.path.realpath(r)), r) for r in roots]
    while todo:
        real, given = todo.pop()
        if real in names:
            continue
        # a library by the name it is referred to (libgdal.37.dylib, not libgdal.3.11.0.dylib)
        names[real] = Path(given).name
        deps, rpaths = load_commands(real)
        refs[real] = []
        for d in deps:
            target = resolve(d, real, rpaths)
            if target is None:
                continue
            refs[real].append((d, target))
            todo.append((target, d))
    taken = {}
    for real, name in names.items():
        if name in taken and taken[name] != real:
            raise SystemExit(f"two libraries named {name}: {taken[name]} and {real}")
        taken[name] = real

    plugins = frameworks / "gdalplugins"
    for real, name in names.items():
        is_plugin = real.parent == Path(os.path.realpath(plugins_src))
        dest = (plugins if is_plugin else frameworks) / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(real, dest)
        dest.chmod(0o755)
        up = "../" if is_plugin else ""
        args = ["-id", f"@loader_path/{name}"]
        for ref, target in refs[real]:
            args += ["-change", ref, f"@loader_path/{up}{names[target]}"]
        _, rpaths = load_commands(dest)
        for rp in dict.fromkeys(rpaths):
            args += ["-delete_rpath", rp]
        # the headers may have no room for longer names: Homebrew links with -headerpad_max_install_names
        subprocess.run(["install_name_tool", *args, dest], check=True, stderr=subprocess.DEVNULL)
        run("codesign", "--force", "--sign", "-", dest, capture=True)
    return len(names), sum(f.stat().st_size for f in frameworks.rglob("*") if f.is_file())


def proj_data(src, dest):
    """PROJ's database and its small files, not the grids of datums (PROJ fetches them when allowed)."""
    dest.mkdir(parents=True, exist_ok=True)
    for f in src.iterdir():
        if f.is_file() and f.suffix.lower() not in (".tif", ".tiff", ".gtx", ".gsb", ".byn"):
            shutil.copyfile(f, dest / f.name)


def info_plist(version, minimum):
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Terraflux</string>
    <key>CFBundleDisplayName</key>
    <string>Terraflux</string>
    <key>CFBundleIdentifier</key>
    <string>io.github.sacridini.terraflux</string>
    <key>CFBundleExecutable</key>
    <string>terraflux</string>
    <key>CFBundleIconFile</key>
    <string>terraflux</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>{minimum}</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.education</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>Eduardo Lacerda, GPL-3.0-or-later</string>
</dict>
</plist>
"""


def newest_minos(files):
    """The newest macOS any of the files was built for (Homebrew builds for the macOS it runs on)."""
    best = (11, 0)
    for f in files:
        out = run("vtool", "-show-build", f, capture=True)
        for m in re.finditer(r"minos (\d+)\.(\d+)", out):
            best = max(best, (int(m[1]), int(m[2])))
    return f"{best[0]}.{best[1]}"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gdal", type=Path, help="the prefix of a GDAL (lib/libgdal*.dylib, share/gdal); Homebrew's by default")
    ap.add_argument("--proj", type=Path, help="the prefix of PROJ (share/proj); Homebrew's, or GDAL's, by default")
    ap.add_argument("--no-gdal", action="store_true", help="no GDAL in the application (it takes an installed one)")
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--no-dmg", action="store_true")
    a = ap.parse_args()
    if sys.platform != "darwin":
        raise SystemExit("the macOS application is made on macOS")

    version = re.search(r'^version = "(.+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]
    arch = {"arm64": "arm64", "x86_64": "x64"}.get(platform.machine(), platform.machine())
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not a.skip_build:
        run("cargo", "build", "--release", "-p", "terraflux-cli", "--manifest-path", ROOT / "Cargo.toml")
    exe = target / "release" / "terraflux"
    if not exe.exists():
        raise SystemExit(f"no {exe}: build it first")

    out = target / "macos"
    app = out / "Terraflux.app"
    if app.exists():
        shutil.rmtree(app)
    contents = app / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    resources = contents / "Resources"
    resources.mkdir()
    shutil.copyfile(exe, contents / "MacOS" / "terraflux")
    (contents / "MacOS" / "terraflux").chmod(0o755)
    for f in ("LICENSE", "README.md", "CHANGELOG.md"):
        shutil.copyfile(ROOT / f, resources / f)
    (contents / "PkgInfo").write_text("APPL????")
    shutil.copyfile(ROOT / "assets" / "logo" / "terraflux.icns", resources / "terraflux.icns")

    minimum = newest_minos([exe])
    if not a.no_gdal:
        prefix = a.gdal or brew_prefix("gdal")
        if prefix is None or not (prefix / "lib").is_dir():
            raise SystemExit("no GDAL: `brew install gdal`, or --gdal <prefix>, or --no-gdal")
        libs = sorted((prefix / "lib").glob("libgdal*.dylib"), key=lambda p: (p.is_symlink(), len(p.name)))
        if not libs:
            raise SystemExit(f"no libgdal*.dylib in {prefix / 'lib'}")
        frameworks = contents / "Frameworks" / "gdal"
        count, size = bundle_gdal(libs[0], frameworks)
        print(f"GDAL from {prefix}: {count} libraries, {size / 2**20:.0f} MB")
        shutil.copytree(prefix / "share" / "gdal", resources / "gdal-data")
        proj = a.proj or brew_prefix("proj") or prefix
        if not (proj / "share" / "proj" / "proj.db").exists():
            raise SystemExit(f"no proj.db in {proj / 'share' / 'proj'}: --proj <prefix>")
        proj_data(proj / "share" / "proj", resources / "projlib")
        minimum = newest_minos([exe, *frameworks.rglob("*.dylib")])
    (contents / "Info.plist").write_text(info_plist(version, minimum))
    run("codesign", "--force", "--sign", "-", app, capture=True)
    run("codesign", "--verify", "--strict", app)
    print(f"{app} (macOS {minimum} or later)")

    zip_ = out / f"terraflux-{version}-macos-{arch}.zip"
    zip_.unlink(missing_ok=True)
    run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", app, zip_)
    made = [zip_]
    if not a.no_dmg:
        stage = out / "dmg"
        if stage.exists():
            shutil.rmtree(stage)
        stage.mkdir()
        run("ditto", app, stage / "Terraflux.app")
        (stage / "Applications").symlink_to("/Applications")
        dmg = out / f"terraflux-{version}-macos-{arch}.dmg"
        dmg.unlink(missing_ok=True)
        run("hdiutil", "create", "-quiet", "-volname", f"Terraflux {version}", "-srcfolder", stage, "-ov",
            "-format", "UDZO", dmg)
        shutil.rmtree(stage)
        made.append(dmg)
    for f in made:
        print(f"{f} ({f.stat().st_size / 2**20:.0f} MB)")


if __name__ == "__main__":
    main()
