---
title: Environment and files
description: The environment variables Terraflux reads, and where it keeps its settings, caches and jobs.
order: 40
---

Most people never need this page. It lists the **environment variables** that change how Terraflux behaves, and
the folders where it keeps its own files.

An environment variable is a setting given to a program by the terminal it starts from. It is set for one
session like this:

=== "Windows (PowerShell)"
    ```powershell
    $env:TERRAFLUX_THREADS = "8"
    terraflux run model.toml
    ```

=== "macOS and Linux"
    ```bash
    export TERRAFLUX_THREADS=8
    terraflux run model.toml
    # or for one command only:
    TERRAFLUX_THREADS=8 terraflux run model.toml
    ```

To keep one for good, add it to your shell's profile, or on Windows to the user's environment variables
(Settings, System, About, Advanced system settings, Environment Variables).

## Environment variables

| Variable | What it does |
|---|---|
| `TERRAFLUX_THREADS` | The threads every command may use, as `--threads` (by default all the cores but one). Terraflux's own tasks, landschaft's OpenMP kernels and the workers of a local batch share them. |
| `TERRAFLUX_GDAL` | The GDAL library to load. By default Terraflux loads the `gdal` folder next to the program (the installers bring it), then an installed GDAL. Without any, GeoTIFF is still read and written by Terraflux itself. |
| `TERRAFLUX_CACHE` | Where Terraflux's caches are kept: the rasters the preparation operators make without a `path` (`prepare`), the data downloaded (`data`, and the reference data of species distribution models). By default the system's cache folder. |
| `TERRAFLUX_CACHE_GB` | The size, in gigabytes, each cache (`prepare`, `data`, a worker's data) is held under, the least recently used removed first (10 by default; 0 for no limit). In the application, **Settings > Cache**. |
| `TERRAFLUX_CACHE_MB` | The memory, in megabytes, of the tiles of the tiled engine's layers kept decoded (2048 by default). More helps large tiled runs on machines with memory to spare. |
| `TERRAFLUX_SSH` | The SSH program used to reach clusters and servers (by default the system's `ssh`). |
| `TERRAFLUX_SETTINGS` | The application's settings file, instead of the one in the user's application data. Useful to keep a separate setup, for teaching for example. |
| `TERRAFLUX_NODES` | The folder of your own library of nodes (the groups and calculators saved with **Save as my node...**), instead of `nodes` in the folder of the settings file. It is read by the application and by `terraflux mcp`. Libraries shared with others are not set here but in the application's settings, **Node libraries** (see [Nodes of your own](/tutorials/building-models/calculator/#libraries-shared-with-your-group)). |
| `TERRAFLUX_TRACE` | When set, the tiled engine writes a line with the time of each phase to the error output, to see where a run spends its time. |
| `TERRAFLUX_PRESENT` | How the application's frames reach the screen: `novsync` shows them as soon as they are drawn. Try it if the window stutters when moved or resized. |
| `TERRAFLUX_LATENCY` | How many frames the application queues (1 by default; try 2 with the above). |
| `WGPU_BACKEND` | The graphics interface of the application: `dx12`, `vulkan`, `metal` or `gl`. Try another if the window is blank or the drivers are old. |

## Where Terraflux keeps its files

Terraflux keeps its own files in the folders each system provides for applications:

| What | Windows | macOS | Linux |
|---|---|---|---|
| Settings, layout of the panes, your templates, your nodes (`nodes`), saved clusters (`clusters.toml`) | `%APPDATA%\Terraflux` | `~/Library/Application Support/Terraflux` | `$XDG_CONFIG_HOME/Terraflux` (`~/.config/Terraflux`) |
| Downloaded data (`cache`) and basemap tiles (`web-tiles`) | `%LOCALAPPDATA%\Terraflux` | `~/Library/Caches/Terraflux` | `$XDG_CACHE_HOME` (`~/.cache`) |
| A coordinator's data and jobs (`serve` without `--data`) | the user's application data, `Terraflux/coordinator` | | |

Your models, data and results are never kept there: they stay where you put them. The outputs of `run` go next to
the model (`output/`) unless `--out` says otherwise; those of `submit` and `calibrate` to the folder of `--out`.

Deleting the cache folders is safe: what they hold is downloaded or made again when needed.
