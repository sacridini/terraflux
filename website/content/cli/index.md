---
title: Command line reference
short: Overview
description: Every terraflux command, what it is for and how to write it, for people who rarely use a terminal.
order: 0
---

Everything Terraflux does in its window it can also do from the **command line**: run a model, check it, run
fifty replicates, calibrate a setting, or send a batch to a cluster. You type a command, press ++enter++, and
Terraflux works without opening a window. The command line is how you run a model on a server, repeat an
analysis exactly, or let a script run many of them in turn.

This reference is written for people who rarely use a terminal. It starts with how to open one and how a command
is written; the next pages go through the commands by what they are for:

| Page | Commands |
|---|---|
| [Running models](/cli/models/) | `run`, `check`, `new`, `convert`, `ops`, `resample`, `app` |
| [Batches: replicates, scenarios, calibration](/cli/batches/) | `submit`, `calibrate`, `jobs`, `results` |
| [Many machines](/cli/distributed/) | `serve`, `worker`, `workers`, `cluster`, `cloud-init` |
| [Environment and files](/cli/environment/) | the environment variables and where Terraflux keeps its files |

## Open a terminal

=== "Windows"
    Press ++win++, type **PowerShell** and open it. The Windows installer puts `terraflux` on the `PATH`, the list
    of folders where Windows looks for programs, so the command works from any folder once a new terminal is
    opened after the installation.

=== "macOS"
    Open **Terminal** (Applications, Utilities). The command lives inside the application; link it once so that
    `terraflux` works anywhere:

    ```bash
    ln -s /Applications/Terraflux.app/Contents/MacOS/terraflux /usr/local/bin/terraflux
    ```

=== "Linux"
    Open your distribution's terminal (often ++ctrl+alt+t++). The `.deb` package puts the command in
    `/usr/bin`; the archive's `install.sh` links it into `~/.local/bin`.

Check that it works:

```text
> terraflux --version
terraflux 0.80.0
```

If the terminal answers that it does not know the command, open a new terminal (the `PATH` is read when a
terminal starts), or use the full path to the program.

## How a command is written

A command is the program's name, a **command** saying what to do, then **arguments** and **options**:

```text
terraflux  run  model.toml  --out results  --set seed=7
   │        │       │            │              │
 program command argument      option       option with a value
```

- An **argument** is what the command works on, here the model file. It has no name; its place says what it is.
- An **option** starts with `--` (or `-` and one letter, its short form: `-o` is `--out`). Some take a value
  after a space; others are switches, on when they are written (`--quiet`).
- Options can come in any order after the command.
- A path with spaces needs quotes: `terraflux run "My models/forest.toml"`.
- Paths are relative to the folder the terminal is in. `cd` changes it: `cd C:\Users\me\project` on Windows,
  `cd ~/project` elsewhere.

Every command explains itself with `--help`, and those explanations are what this reference shows in grey
boxes:

```bash
terraflux --help          # the commands
terraflux run --help      # the options of run
terraflux cluster add --help
```

## The commands

```tfcli

```

### Options every command takes

`--threads N`
:   How many threads Terraflux may use. By default it uses all the cores of the computer but one, so the machine
    stays usable; it never takes every core unless you ask. The environment variable `TERRAFLUX_THREADS` sets
    the same for every command (see [Environment and files](/cli/environment/)).

`-h`, `--help`
:   The command's help.

`-V`, `--version`
:   The version of Terraflux (with `terraflux` alone: `terraflux --version`).

Running `terraflux` with no command, or double-clicking the program, opens the application.

## When something goes wrong

Terraflux says what failed, where, and why, then stops with a non-zero exit code (so a script can tell that it
failed). The most common messages:

`The system cannot find the path specified` / `No such file or directory`
:   A path in the command or in the model does not exist from where Terraflux looks. Paths inside a model are
    relative to the model file's folder; paths on the command line are relative to the terminal's folder.

`TOML parse error ... missing field 'format'`
:   The file is not a Terraflux model, or its first line `format = "terraflux/1"` is missing.

`unknown operator`
:   A node names an operator that does not exist in this version; `terraflux ops` lists them all.

Before running a long model, `terraflux check model.toml` finds most problems in a fraction of a second: every
operator, every connection between nodes and every input file is checked without running anything.

## Going further

- The model file the commands read: [Your first model](/tutorials/getting-started/first-model/) and
  [Building models](/tutorials/#building-models).
- Running many models at once, on many machines: the guide to
  [parallel and distributed computing](/parallel/).
