# SPDX-License-Identifier: GPL-3.0-or-later
"""Terraflux for Linux, in target/linux:

    terraflux-<version>-linux-<arch>/          the program's folder (terraflux is the application and the command)
    terraflux-<version>-linux-<arch>.tar.gz    the folder, to unpack anywhere (its install.sh links it for the user)
    terraflux_<version>_<debian arch>.deb      the same folder in /opt/terraflux, the command in /usr/bin, a menu entry
    terraflux-<version>-1.<rpm arch>.rpm       the same for Fedora, openSUSE...
    terraflux-<version>-1-<arch>.pkg.tar.zst   the same for Arch Linux (pacman -U)

The folder holds terraflux (the release build without its debugging information; its runtime libraries that are
not the system's, as libgomp, in lib/) and a GDAL: its library and every library it needs but the system's own
(the C and C++ runtimes) in gdal/, each finding the others there ($ORIGIN), with GDAL's data (gdal-data), PROJ's
(projlib, without the grids of datums) and the certificates of the web (curl-ca-bundle.crt). Nothing else is
needed but a glibc as new as the one it was built with (printed): build on the oldest distribution to be
supported.

    python3 tools/make-linux-package.py                    # the system's GDAL (apt install libgdal-dev)
    python3 tools/make-linux-package.py --gdal /opt/gdal   # another GDAL's prefix (its lib/ and share/)
    python3 tools/make-linux-package.py --skip-build --no-deb --no-rpm --no-arch

patchelf is needed (apt install patchelf, or pip install patchelf); dpkg-deb for the .deb; nfpm
(https://nfpm.goreleaser.com) for the .rpm and the Arch package.
"""

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOGO = ROOT / "assets" / "logo"
# the system's own libraries, never brought: the C library and its parts, the C++ and GCC runtimes the program
# is built against
SYSTEM = re.compile(
    r"^(linux-vdso|linux-gate|ld-linux.*|libc|libm|libdl|libpthread|librt|libresolv|libutil|libanl|libnsl|"
    r"libBrokenLocale|libmvec|libstdc\+\+|libgcc_s)\.so"
)
# the grids of datums PROJ fetches when allowed
GRIDS = (".tif", ".tiff", ".gtx", ".gsb", ".byn")


def run(*args, capture=False):
    r = subprocess.run([str(a) for a in args], check=True, capture_output=capture, text=True)
    return r.stdout if capture else None


def needed(path):
    """The libraries a file needs, by name, and where the system's loader finds them (None: not found)."""
    out = {}
    for line in run("ldd", path, capture=True).splitlines():
        m = re.match(r"\s*(\S+)\s+=>\s+(not found|\S+)", line)
        if m:
            out[m[1]] = None if m[2] == "not found" else Path(os.path.realpath(m[2]))
    return out


def bundle(roots, dest, patchelf):
    """The files `roots` (name, path) and every library they need but the system's, in `dest` under the names
    they are needed by, each loading the others from there."""
    dest.mkdir(parents=True, exist_ok=True)
    names = {}
    todo = list(roots)
    while todo:
        name, real = todo.pop()
        if name in names:
            if names[name] != real:
                raise SystemExit(f"two libraries named {name}: {names[name]} and {real}")
            continue
        names[name] = real
        for dep, where in needed(real).items():
            if SYSTEM.match(dep):
                continue
            if where is None:
                raise SystemExit(f"{real}: {dep} not found")
            todo.append((dep, where))
    for name, real in names.items():
        target = dest / name
        shutil.copyfile(real, target)
        target.chmod(0o755)
        run(patchelf, "--set-rpath", "$ORIGIN", target)
    return names


def glibc_needed(files):
    """The newest glibc any of the files asks for."""
    best = (2, 0)
    for f in files:
        out = subprocess.run(["objdump", "-T", str(f)], capture_output=True, text=True).stdout
        for m in re.finditer(r"GLIBC_(\d+)\.(\d+)", out):
            best = max(best, (int(m[1]), int(m[2])))
    return f"{best[0]}.{best[1]}"


def system_gdal():
    """The system's libgdal (ldconfig's), and its prefix."""
    places = ("/sbin/ldconfig", "/usr/sbin/ldconfig")
    ldconfig = shutil.which("ldconfig") or next((p for p in places if Path(p).exists()), "ldconfig")
    out = subprocess.run([ldconfig, "-p"], capture_output=True, text=True).stdout
    for line in out.splitlines():
        m = re.match(r"\s*(libgdal\.so\.\d+)\s.*=>\s*(\S+)", line)
        if m:
            return Path(m[2]), Path("/usr")
    return None, None


def proj_data(src, dest):
    """PROJ's database and its small files, not the grids of datums."""
    dest.mkdir(parents=True, exist_ok=True)
    for f in src.iterdir():
        if f.is_file() and f.suffix.lower() not in GRIDS:
            shutil.copyfile(f, dest / f.name)


DESKTOP = """[Desktop Entry]
Type=Application
Name=Terraflux
GenericName=Land change modelling
Comment=Land change models, landscape analysis and spatial prioritization
Exec={exe} %f
Icon={icon}
Terminal=false
Categories=Science;Geography;Education;
Keywords=land use;land change;GIS;landscape;prioritization;
"""

INSTALL = """#!/bin/sh
# Links Terraflux for this user: the command in ~/.local/bin and a menu entry (no administrator needed).
set -e
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
ln -sf "$here/terraflux" "$HOME/.local/bin/terraflux"
sed -e "s|@EXE@|$here/terraflux|" -e "s|@ICON@|$here/terraflux.png|" "$here/terraflux.desktop.in" \\
    > "$HOME/.local/share/applications/terraflux.desktop"
echo "terraflux linked in $HOME/.local/bin (put it on the PATH if it is not) and added to the menu"
"""


def control(version, arch, glibc, size_kb):
    return f"""Package: terraflux
Version: {version}
Architecture: {arch}
Maintainer: Eduardo Lacerda
Installed-Size: {size_kb}
Depends: libc6 (>= {glibc}), libstdc++6, libgcc-s1
Recommends: libegl1, libvulkan1, libx11-6, libxcursor1, libxi6, libxkbcommon-x11-0, libwayland-client0
Section: science
Priority: optional
Homepage: https://github.com/sacridini/terraflux
Description: land change models, landscape analysis and spatial prioritization
 Terraflux runs land change models (Dinamica EGO's, CLUE-S, FUTURES...) as graphs of operators, with
 landschaft's tools: landscape metrics, GuidosToolbox, circuits, connectivity, species distribution
 models and spatial prioritization. It brings its own GDAL.
"""


# the libraries of the application's desktop on Fedora (the command line needs none; an Arch desktop has them)
RPM_RECOMMENDS = ["mesa-libEGL", "vulkan-loader", "libX11", "libXcursor", "libXi", "libxkbcommon-x11",
                  "libwayland-client"]


def nfpm_config(version, deb_arch, glibc, root, files):
    """nfpm's settings (JSON being YAML) for the .rpm and the Arch package of the tree `root`: /opt/terraflux,
    the link in /usr/bin and `files` (the menu entry and the icons), not the folders of the system."""
    contents = [
        {"src": str(root / "opt" / "terraflux"), "dst": "/opt/terraflux", "type": "tree"},
        {"src": "/opt/terraflux/terraflux", "dst": "/usr/bin/terraflux", "type": "symlink"},
    ]
    contents += [{"src": str(root / f), "dst": "/" + f.as_posix()} for f in files]
    return {
        "name": "terraflux",
        "arch": deb_arch,
        "platform": "linux",
        "version": version,
        "release": 1,
        "maintainer": "Eduardo Lacerda",
        "description": "Land change models, landscape analysis and spatial prioritization. It brings its own GDAL.",
        "homepage": "https://github.com/sacridini/terraflux",
        "license": "GPL-3.0-or-later",
        "contents": contents,
        "overrides": {
            "rpm": {"depends": [f"glibc >= {glibc}", "libstdc++", "libgcc"], "recommends": RPM_RECOMMENDS},
            "archlinux": {"depends": [f"glibc>={glibc}", "gcc-libs"]},
        },
        "archlinux": {"packager": "Eduardo Lacerda"},
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--gdal", type=Path,
                    help="the prefix of a GDAL (lib/libgdal.so*, share/gdal); the system's by default")
    ap.add_argument("--proj", type=Path, help="the prefix of PROJ (share/proj); GDAL's by default")
    ap.add_argument("--no-gdal", action="store_true", help="no GDAL in the package (it takes an installed one)")
    ap.add_argument("--patchelf", default=shutil.which("patchelf") or "patchelf")
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--no-deb", action="store_true")
    ap.add_argument("--no-rpm", action="store_true")
    ap.add_argument("--no-arch", action="store_true", help="no Arch Linux package")
    ap.add_argument("--nfpm", default=shutil.which("nfpm") or "nfpm", help="nfpm, which makes the .rpm and the Arch package")
    a = ap.parse_args()
    if not sys.platform.startswith("linux"):
        raise SystemExit("the Linux package is made on Linux")
    if shutil.which(a.patchelf) is None and not Path(a.patchelf).is_file():
        raise SystemExit("no patchelf: apt install patchelf, or pip install patchelf, or --patchelf <path>")

    version = re.search(r'^version = "(.+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]
    machine = platform.machine()
    arch = {"x86_64": "x64", "aarch64": "arm64"}.get(machine, machine)
    deb_arch = {"x86_64": "amd64", "aarch64": "arm64"}.get(machine, machine)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not a.skip_build:
        run("cargo", "build", "--release", "-p", "terraflux-cli", "--manifest-path", ROOT / "Cargo.toml")
    exe = target / "release" / "terraflux"
    if not exe.exists():
        raise SystemExit(f"no {exe}: build it first")

    out = target / "linux"
    name = f"terraflux-{version}-linux-{arch}"
    folder = out / name
    if folder.exists():
        shutil.rmtree(folder)
    folder.mkdir(parents=True)
    # the program, without its debugging information, its runtime libraries in lib/
    prog = folder / "terraflux"
    run("objcopy", "--strip-debug", exe, prog)
    prog.chmod(0o755)
    libs = [(d, w) for d, w in needed(prog).items() if not SYSTEM.match(d)]
    if any(w is None for _, w in libs):
        raise SystemExit(f"{exe}: {[d for d, w in libs if w is None]} not found")
    bundle(libs, folder / "lib", a.patchelf)
    run(a.patchelf, "--set-rpath", "$ORIGIN/lib", prog)
    for f in ("LICENSE", "README.md", "CHANGELOG.md"):
        shutil.copyfile(ROOT / f, folder / f)

    if not a.no_gdal:
        if a.gdal:
            found = sorted((a.gdal / "lib").glob("libgdal.so.*"), key=lambda p: len(p.name))
            lib, prefix = (found[0] if found else None), a.gdal
        else:
            lib, prefix = system_gdal()
        if lib is None:
            raise SystemExit("no GDAL: apt install libgdal-dev (or gdal-bin), or --gdal <prefix>, or --no-gdal")
        soname = run(a.patchelf, "--print-soname", lib, capture=True).strip() or lib.name
        gdal = folder / "gdal"
        names = bundle([(soname, Path(os.path.realpath(lib)))], gdal, a.patchelf)
        # GDAL's plugins, their libraries with GDAL's
        plugins = Path(os.path.realpath(lib)).parent / "gdalplugins"
        if plugins.is_dir():
            dest = gdal / "gdalplugins"
            dest.mkdir()
            for p in sorted(plugins.iterdir()):
                if p.suffix == ".so":
                    deps = [(d, w) for d, w in needed(p).items() if not SYSTEM.match(d) and d not in names]
                    names.update(bundle(deps, gdal, a.patchelf))
                    shutil.copyfile(p, dest / p.name)
                    run(a.patchelf, "--set-rpath", "$ORIGIN/..", dest / p.name)
                elif p.is_file():
                    shutil.copyfile(p, dest / p.name)
        size = sum(f.stat().st_size for f in gdal.rglob("*") if f.is_file())
        print(f"GDAL {lib}: {len(names)} libraries, {size / 2**20:.0f} MB")
        data = prefix / "share" / "gdal"
        if not data.is_dir():
            raise SystemExit(f"no GDAL data in {data}")
        shutil.copytree(data, gdal / "gdal-data")
        proj = (a.proj or prefix) / "share" / "proj"
        if not (proj / "proj.db").exists():
            raise SystemExit(f"no proj.db in {proj}: --proj <prefix>")
        proj_data(proj, gdal / "projlib")
        for ca in ("/etc/ssl/certs/ca-certificates.crt", "/etc/pki/tls/certs/ca-bundle.crt"):
            if Path(ca).is_file():
                shutil.copyfile(ca, gdal / "curl-ca-bundle.crt")
                break
    elf = [prog, *folder.joinpath("lib").glob("*"), *folder.joinpath("gdal").rglob("*.so*")]
    glibc = glibc_needed([f for f in elf if f.is_file()])
    shutil.copyfile(LOGO / "png" / "terraflux-icon-256.png", folder / "terraflux.png")
    (folder / "terraflux.desktop.in").write_text(DESKTOP.format(exe="@EXE@", icon="@ICON@"))
    (folder / "install.sh").write_text(INSTALL)
    (folder / "install.sh").chmod(0o755)
    print(f"{folder} (glibc {glibc} or later)")

    tgz = out / f"{name}.tar.gz"
    with tarfile.open(tgz, "w:gz") as t:
        t.add(folder, arcname=name)
    made = [tgz]
    want_deb = not a.no_deb and shutil.which("dpkg-deb") is not None
    nfpm = shutil.which(a.nfpm) or (a.nfpm if Path(a.nfpm).is_file() else None)
    formats = [f for f, off in (("rpm", a.no_rpm), ("archlinux", a.no_arch)) if not off]
    if formats and nfpm is None:
        print("no nfpm: no .rpm nor Arch package (https://nfpm.goreleaser.com, or --nfpm <path>)")
        formats = []
    if want_deb or formats:
        # the tree of the packages: the folder in /opt/terraflux, the command in /usr/bin, the menu entry, the icons
        root = out / "pkgroot"
        if root.exists():
            shutil.rmtree(root)
        opt = root / "opt" / "terraflux"
        shutil.copytree(folder, opt, symlinks=True)
        (opt / "install.sh").unlink()
        (opt / "terraflux.desktop.in").unlink()
        (opt / "terraflux.png").unlink()
        files = [Path("usr/share/applications/terraflux.desktop")]
        (root / files[0]).parent.mkdir(parents=True)
        (root / files[0]).write_text(DESKTOP.format(exe="/opt/terraflux/terraflux", icon="terraflux"))
        for n in (48, 64, 128, 256, 512):
            icon = Path(f"usr/share/icons/hicolor/{n}x{n}/apps/terraflux.png")
            (root / icon).parent.mkdir(parents=True)
            shutil.copyfile(LOGO / "png" / f"terraflux-icon-{n}.png", root / icon)
            files.append(icon)
        if want_deb:
            (root / "usr" / "bin").mkdir(parents=True)
            (root / "usr" / "bin" / "terraflux").symlink_to("../../opt/terraflux/terraflux")
            size_kb = sum(f.stat().st_size for f in root.rglob("*") if f.is_file()) // 1024
            (root / "DEBIAN").mkdir()
            (root / "DEBIAN" / "control").write_text(control(version, deb_arch, glibc, size_kb))
            deb = out / f"terraflux_{version}_{deb_arch}.deb"
            deb.unlink(missing_ok=True)
            run("dpkg-deb", "--root-owner-group", "-Zxz", "--build", root, deb, capture=True)
            made.append(deb)
        if formats:
            config = out / "nfpm.yaml"
            config.write_text(json.dumps(nfpm_config(version, deb_arch, glibc, root, files), indent=2))
            rpm_arch = {"amd64": "x86_64", "arm64": "aarch64"}.get(deb_arch, deb_arch)
            names = {"rpm": f"terraflux-{version}-1.{rpm_arch}.rpm",
                     "archlinux": f"terraflux-{version}-1-{rpm_arch}.pkg.tar.zst"}
            for fmt in formats:
                pkg = out / names[fmt]
                pkg.unlink(missing_ok=True)
                run(nfpm, "package", "--config", config, "--packager", fmt, "--target", pkg, capture=True)
                made.append(pkg)
            config.unlink()
        shutil.rmtree(root)
    for f in made:
        print(f"{f} ({f.stat().st_size / 2**20:.0f} MB)")


if __name__ == "__main__":
    main()
