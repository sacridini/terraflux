---
title: Contributing
description: How to report a bug, ask for a feature, improve the documentation or send code to Terraflux.
order: 50
---

Terraflux is built in the open and is kept by one maintainer, [Eduardo Lacerda](https://github.com/sacridini).
Bug reports, ideas, corrections to the documentation and code are all welcome. This page says how to make them
easy to act on.

Everyone who takes part follows the [Code of Conduct](https://github.com/sacridini/terraflux/blob/main/CODE_OF_CONDUCT.md).
To report a security problem, do not open a public issue: see the
[security policy](https://github.com/sacridini/terraflux/blob/main/SECURITY.md).

## Report a bug

Open an [issue](https://github.com/sacridini/terraflux/issues/new). First look at the open issues and at the
[changelog](https://github.com/sacridini/terraflux/blob/main/CHANGELOG.md): it may be known or fixed in a newer
version. A report that someone else can reproduce is fixed much faster. Give:

1. **What you did**, step by step, from opening Terraflux or the command you typed.
2. **What you expected** and **what happened**. A screenshot helps for anything drawn wrongly.
3. **The version.** In the application, **Settings**, **About & updates**; on the command line,
   `terraflux --version`.
4. **The operating system** and its version (Windows 11, macOS 15, Ubuntu 24.04...), and how you installed Terraflux
   (the installer, the portable zip, a package, a build from source).
5. **The model**: the `.toml` file, with the submodels it uses. Small is best: remove the nodes that are not needed to
   show the problem. If the data matter, say what the maps are (size, cell size, coordinate system), or attach a
   small crop of them. Do not attach data you may not share.
6. **The message.** Copy the text of the error: from the **Problems** pane, from the node's tooltip, or from the
   terminal where `terraflux run model.toml` ran. If you ran it from the command line, the `manifest.json` written
   beside the outputs (it records the version, the settings and the seeds) is useful too.

Terraflux does not keep a log file of its own while it runs: what it has to say is in the application's panes and in
the terminal. The only log it writes is that of an update that did not install (`terraflux-update.log` in the
temporary folder, and the application tells you when it opens again). Its settings and caches are in the folders
listed in [Where Terraflux keeps its things](/quickstart/install/#where-terraflux-keeps-its-things); check there
before sending a settings file, and never post a key or a password.

If the application crashed, say what you were doing and whether it happens again. Edits not saved are recovered
when you open Terraflux again.

## Ask for a feature

Open an issue and start with **the problem**, not the solution: what you are trying to do, with which data, and what
you do today instead. Say which tool or paper does it now, if one does (Terraflux's methods follow reference
implementations, which it is tested against). A method that is not in Terraflux needs a reference: a paper, and the
software that implements it, so that the results can be compared. Several small requests are better as several
issues.

## Improve the documentation

Every page of this site ends with **Suggest a change to this page**: it opens the page's source on GitHub, where you
can edit it and propose the change as a pull request, with nothing to install. The pages are plain Markdown
files in `website/content/`. [AUTHORING.md](https://github.com/sacridini/terraflux/blob/main/website/AUTHORING.md)
gives the style: plain, precise English, short sentences, written for an ecologist who may not know the tool.

To see a change before sending it:

```bash
pip install -r website/requirements.txt
python website/build.py --serve      # http://localhost:8000, rebuilt when a file changes
```

[website/README.md](https://github.com/sacridini/terraflux/blob/main/website/README.md) says what is where. The
reference pages of the operators and of the commands are made from the program: to correct one, correct the
operator's documentation in the code. The pages of the Python library come from its own repository.

## Build and test the code

Terraflux needs Rust 1.85 or later, CMake and a C++17 compiler; GDAL is loaded at run time and is not needed to
build. The C++ kernels come from the terraflux-py submodule, so clone with it:

```bash
git clone --recurse-submodules https://github.com/sacridini/terraflux.git
cd terraflux
cargo build --release          # target/release/terraflux
```

The repository is a Cargo workspace of crates (`graph`, `ops`, `exec`, `raster`, `app`, `cli` and others; the
[README](https://github.com/sacridini/terraflux#building-from-source) lists them). Build and test the crate you
changed, with a filter when you can, and limit the cores so the computer stays usable:

```bash
cargo test -p terraflux-ops -j 4 -- --test-threads=4 name_of_a_test
RAYON_NUM_THREADS=4 OMP_NUM_THREADS=4 cargo test -p terraflux-exec -j 4
```

Format only the files you touched (`rustfmt file.rs`; the repository's `rustfmt.toml` applies). Write a test next to
the ones around it: a new operator or a changed result needs one. Some parity tests differ in the last bit on macOS
and Linux, because their references were written on Windows; the README lists them.

The Python library, [terraflux-py](https://github.com/sacridini/terraflux-py), is a separate repository with its
own issues, build and tests. Send changes to the methods' C++ kernels or to the Python API there.

## Send a change

1. Open an issue first for anything large, to agree on the approach before you write it.
2. Fork the repository, make a branch, and make the change small and on one subject.
3. Run the tests of the crates you touched.
4. Open a pull request that says what it changes and why, and links the issue.

**Commit messages** follow the style of the history: a type, the area in brackets, a colon and a sentence that says
what is different now.

```text
feat(figure): a figure's font and text size, in the preview and every format
fix(viewer): the map no longer jumps 24 px down after an edit
docs(website): operator data of 1.52.0
test: the flaky ui tests made steady
chore: release 1.52.0
```

The types are `feat`, `fix`, `perf`, `docs`, `test` and `chore`. Releases and version numbers are made by the
maintainer: do not change the version or the changelog's dated sections in a pull request; a line under
`[Unreleased]` in `CHANGELOG.md` is welcome.

## License

Terraflux is under the GNU General Public License, version 3 or later (GPL-3.0-or-later). By sending a
contribution, you agree that it is released under the same license, and that you have the right to do so (it is
your own work, or under a compatible license that you name).
