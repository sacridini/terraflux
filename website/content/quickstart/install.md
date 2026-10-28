---
title: Installing Terraflux
short: Installing
description: The packages for Windows, macOS and Linux, what each installs and where, the command line on every system, and what to do when the application does not start or its window stutters.
order: 20
---

Terraflux is one program, `terraflux`, that is both the application (a window) and the command line. Every
package brings everything it needs: **GDAL** (the open-source library that reads and writes raster and vector
files, with its data and PROJ's, which handle coordinate systems) and the runtime libraries of its compiled
code. You do not need Python, R, or a GDAL of your own. This page gives the details of each package; the
[Quickstart](../) has the short version.

Download the files from the [download page](/download/). `<version>` below stands for the version number, such as
`0.80.0`.

## Windows

Windows 10 and 11, 64-bit.

=== "Installer (.msi)"

    `terraflux-<version>-x64.msi`. Double-click it and follow the steps. It installs Terraflux for you, without
    asking for the administrator's permission, in `%LOCALAPPDATA%\Programs\Terraflux`, adds a shortcut to the
    **Start menu**, and puts its folder on your **PATH**, so that `terraflux` works in any new terminal
    (PowerShell or the command prompt). It brings GDAL (the folder `gdal`) and the Visual C++ and OpenMP runtimes.
    Its last page says Terraflux is installed and opens it.

    **Updates.** Terraflux asks GitHub for a newer version when it starts (once a day; turn it off in the settings,
    ⚙). When there is one, **Update to X.Y.Z** appears in orange at the top: a click saves the model, downloads the
    installer, checks it against the release's checksums, installs it (only its progress shown) and opens
    Terraflux again with the model. Its version is beside its name at the top left, in the window's title and in
    the settings (**Check now** there asks at once).

    The updater of Terraflux 1.20.0 and older closed Terraflux and installed nothing: from one of those, install
    1.21.0 or later by hand once, and the updates work from then on.

    To remove it: **Settings > Apps > Installed apps > Terraflux > Uninstall** (no administrator either).

    !!! note "Installed by a version before 1.18?"
        The installers before 1.18 installed Terraflux for all the users, in Program Files, and Windows asked for
        the administrator. Remove that one first (Settings > Apps; Windows asks one last time): the new installer
        says so if it is still there.

=== "Portable (.zip)"

    `terraflux-<version>-windows-x64.zip` holds the same files. Unpack it anywhere (your documents, a USB stick)
    and double-click `terraflux.exe`. Nothing is installed and the PATH is not changed: to use the command line,
    run it by its full path (`C:\Tools\terraflux\terraflux.exe run model.toml`) or add its folder to the PATH
    yourself.

Started by a double click, Terraflux opens the application without a console window. A model file dropped on
`terraflux.exe`, or opened with it, opens in the application.

## macOS

Apple silicon (M1 or later).

=== "Disk image (.dmg)"

    `terraflux-<version>-macos-arm64.dmg`. Open it and drag **Terraflux** to **Applications**.

=== "Application (.zip)"

    `terraflux-<version>-macos-arm64.zip` holds the same `Terraflux.app`; unpack it and move it to
    **Applications**.

The application brings GDAL (its libraries inside `Terraflux.app/Contents/Frameworks/gdal`, GDAL's and PROJ's data
in `Contents/Resources`) and the OpenMP runtime. PROJ's large grids of datum shifts are left out to keep the
download small.

!!! warning "The first time: right-click, Open"
    Terraflux is signed *ad hoc*, not notarized by Apple. Downloaded from the Internet, a double click is refused
    the first time ("cannot be opened because the developer cannot be verified"). **Right-click** (or
    ctrl-click) **Terraflux** in Applications and choose **Open**, then **Open** again in the dialog. macOS
    remembers the choice; from then on a double click works.

**Updates.** As on Windows, Terraflux asks GitHub for a newer version when it starts (once a day; turn it off in
the settings, ⚙), and **Update to X.Y.Z** appears in orange at the top. When `Terraflux.app` is in a folder you
may write, it updates itself: it downloads the release's `.zip`, checks it against the release's checksums,
unpacks it beside the application, then closes, puts the new `Terraflux.app` in the place of the old one and
opens again with the model. If the application is in a folder you may not write, the window offers **Open its
page** instead: download the new version from there and install it as the first time.

## Linux

x86-64 with glibc 2.39 or newer: Ubuntu 24.04 and later, Debian 13, Fedora 40 and later, and other distributions
of 2024 or newer.

=== "Debian / Ubuntu (.deb)"

    ```bash
    sudo apt install ./terraflux_<version>_amd64.deb
    ```

    It installs Terraflux in `/opt/terraflux`, the command in `/usr/bin` and an entry in the desktop's menu.
    To remove it: `sudo apt remove terraflux`.

=== "Fedora / openSUSE (.rpm)"

    ```bash
    sudo dnf install ./terraflux-<version>-1.x86_64.rpm
    ```

    On openSUSE, `sudo zypper install ./terraflux-<version>-1.x86_64.rpm`. As the `.deb`, it installs Terraflux in
    `/opt/terraflux`, the command in `/usr/bin` and an entry in the menu, and it recommends the desktop's graphics
    libraries (EGL, Vulkan, X11's and Wayland's), which `dnf` installs with it unless told not to. To remove it:
    `sudo dnf remove terraflux`.

=== "Arch Linux (.pkg.tar.zst)"

    ```bash
    sudo pacman -U terraflux-<version>-1-x86_64.pkg.tar.zst
    ```

    The same `/opt/terraflux`, `/usr/bin` and menu entry. To remove it: `sudo pacman -R terraflux`.

=== "Archive (.tar.gz)"

    No root needed:

    ```bash
    tar xzf terraflux-<version>-linux-x64.tar.gz
    cd terraflux-<version>-linux-x64
    ./install.sh
    ```

    The folder stays where you unpacked it; `install.sh` links the command into `~/.local/bin` and adds Terraflux
    to the menu, for your user alone. If `terraflux` is not found afterwards, `~/.local/bin` is not on your PATH:
    add `export PATH="$HOME/.local/bin:$PATH"` to `~/.bashrc` and open a new terminal.

All of them bring GDAL (its library and every library it needs except the C and C++ runtimes, with GDAL's and PROJ's
data and the web's certificates) and the OpenMP runtime. The application also needs your desktop's own graphics
libraries: **EGL or Vulkan**, and **X11's `libxkbcommon-x11`** or **Wayland**. A desktop installation has them; a
minimal server or container does not (see below). The command line alone (`terraflux run`) needs none of them.

**Updates.** Terraflux tells of a newer version as on Windows (**Update to X.Y.Z**, in orange at the top), and
how it updates itself depends on how it was installed:

- **The archive (.tar.gz)**, in a folder you may write: the release's `.tar.gz` is downloaded, checked against
  the release's checksums and unpacked beside the folder; Terraflux closes, the new folder takes the place (and
  the name) of the old one, and Terraflux opens again with the model.
- **The packages (.deb, .rpm, Arch's .pkg.tar.zst)**, in `/opt/terraflux`: the new package is downloaded and
  checked, Terraflux closes, and the system's package manager installs it (`dpkg`, `rpm` or `pacman`), asking for
  the administrator's password through `pkexec`; then Terraflux opens again with the model. This needs `pkexec`
  (part of polkit, which most desktop installations have) on the PATH.

Without these (a folder you may not write, no `pkexec`, Terraflux built from source), the window offers **Open
its page**, to download the new version and install it by hand.

## The command line

Check that the command works:

```text
$ terraflux --version
terraflux 0.80.0
```

| System | Where the command is |
|---|---|
| Windows, installer | on the PATH: open a **new** terminal after installing |
| Windows, zip | in the folder you unpacked: use its full path, or add the folder to the PATH |
| macOS | inside the application: link it once with `ln -s /Applications/Terraflux.app/Contents/MacOS/terraflux /usr/local/bin/terraflux` (with `sudo` if `/usr/local/bin` is not yours) |
| Linux, .deb, .rpm or Arch's package | `/usr/bin/terraflux` |
| Linux, .tar.gz | `~/.local/bin/terraflux` |

`terraflux` with no command opens the application; `terraflux app model.toml` opens it with a model. The
commands are described in the [CLI reference](/cli/), and `terraflux help` lists them:

```tfcli
```

## When something goes wrong

### The application does not start (Linux)

Run `terraflux app` in a terminal to see the message. A missing `libEGL`, `libvulkan` or `libxkbcommon-x11` means
the desktop's graphics libraries are missing; on Ubuntu or Debian:

```bash
sudo apt install libegl1 libxkbcommon-x11-0
```

On a server without a screen, use the command line only: `terraflux run`, `check` and `submit` do not open a
window. A message about `GLIBC_2.39` means the distribution is older than the packages; use a newer one, or build
Terraflux from source (see the README in the repository).

### An update did not install

When Terraflux opens again after an update and is still the old version, the top of the window says **the update
failed**; its tooltip, and **Updates** in the settings, say *Terraflux X.Y.Z was not installed (this is still
...); its log: ...*. The log is `terraflux-update.log` in the system's temporary folder: the installer's own log
on Windows (`%TEMP%\terraflux-update.log`), the output of the script that replaced the application or ran the
package manager on macOS and Linux. A wrong administrator's password, or the dialog of `pkexec` closed, ends there
as *not installed*. Read it, then install the new version by hand from the [download page](/download/).

### The window stutters when it is moved or resized

How frames reach the screen can be changed with environment variables, set before starting Terraflux:

| Variable | Values | Effect |
|---|---|---|
| `TERRAFLUX_PRESENT` | `novsync` | frames shown as soon as they are drawn |
| `TERRAFLUX_LATENCY` | `2` | frames queued (1 by default) |
| `WGPU_BACKEND` | `dx12`, `vulkan`, `metal` or `gl` | the graphics interface used |

For example, in PowerShell:

```powershell
$env:WGPU_BACKEND = "gl"; terraflux app
```

and in a Linux or macOS terminal:

```bash
WGPU_BACKEND=vulkan terraflux app
```

`WGPU_BACKEND=gl` is also worth trying when the window opens black or does not open at all on an old graphics
card or a remote desktop.

### The computer is slow while a model runs

Terraflux uses all the cores but one by default, so the computer stays usable. Use fewer with **Settings >
Threads** in the application (from the next start), `--threads 4` on any command, or the environment variable
`TERRAFLUX_THREADS`.

### A basemap does not appear

Basemaps (satellite imagery and maps of the world under your data) are downloaded from their providers: they need
an Internet connection, and a map with a coordinate system. See
[A tour of Terraflux](/tutorials/getting-started/tour/#basemaps).

### A file does not open

Terraflux reads every format its GDAL reads. A GeoTIFF that opens in QGIS opens in Terraflux; if one does not,
check its path (paths in a model are relative to the model's folder) and open it alone with **File > Open a raster or
vectors...** to see the message.

## Where Terraflux keeps its things

| What | Windows | macOS | Linux |
|---|---|---|---|
| Settings, layout, favourites | `%APPDATA%\Terraflux` | `~/Library/Application Support/Terraflux` | `~/.config/Terraflux` (or `$XDG_CONFIG_HOME/Terraflux`) |
| Your templates | `%APPDATA%\Terraflux\templates` | `~/Library/Application Support/Terraflux/templates` | `~/.config/Terraflux/templates` |
| Basemap tiles (a cache) | `%LOCALAPPDATA%\Terraflux\web-tiles` | `~/Library/Caches/Terraflux/web-tiles` | `/tmp/Terraflux/web-tiles` (the temporary folder, `$TMPDIR` when it is set: emptied when the computer restarts on many systems) |

Removing these folders resets Terraflux to how it was installed.
