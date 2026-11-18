---
title: Settings
short: Settings
description: "Every section and option of Terraflux's Settings window, with its default, the place of the settings file on each system, what Back to the defaults keeps, where secrets go and what else is kept."
order: 70
---

**⚙** at the right of the toolbar opens the **Settings** window. It opens in the middle of the screen and you can
move it. Its sections are listed in a column at the left; a click shows one at the right. A change is kept at once,
and the look changes at once: there is no *OK* or *Apply*. The window is in English only.

| Section | Holds |
|---|---|
| [Appearance](#appearance) | The theme, the text's size, the interface zoom. |
| [Model builder](#model-builder) | How the canvas looks, and what happens when Terraflux starts. |
| [Runs](#runs) | Sleep, copies of outputs, notifications, the Python script. |
| [Performance](#performance) | Threads and the cache. |
| [Cloud storage](#cloud-storage) | Keys for Amazon S3, Google Cloud Storage and Azure. |
| [Node libraries](#node-libraries) | Folders and remote libraries of nodes. |
| [Keyboard](#keyboard) | The shortcuts, and how to change them. |
| [About & updates](#about-and-updates) | The version and the check for a new one. |

At the foot of the column, **Back to the defaults** ([below](#back-to-the-defaults)) resets the settings.

In the tables, a box that says *on* or *off* is a switch. *Default* is what a fresh installation has.

## Appearance

| Label | Default | What it does |
|---|---|---|
| **Theme** | Slate | The colours of the window, in a list. **Modern** themes have rounded panes and cards, switches and a font of their own: *Slate* (dark blue), *Graphite* (dark grey and orange), *Midnight* (deep navy and violet), *Canopy* (dark green), *Paper* (light, in the colours of this website) and *Ash* (light grey and orange). **Classic** themes keep the square look: *Classic*, *Immaterial*, *Field Notes*, *Riparian*, *Angst Robot* and *Twenty Four Carat*. The default is marked *(default)*. |
| **Text size** | 100 % | The size of the text, from 80 % to 160 %, in steps of 5 %. |
| **Interface zoom** | 100 % | The size of the whole interface, from 75 % to 200 %, in steps of 5 %. For a small laptop screen or a projector. |

## Model builder

Under **Model**:

| Label | Default | What it does |
|---|---|---|
| **the nodes of data in the colour of their type** | off | The model's inputs and the nodes that read, write, stack or plot get a header in the colour of what they give (a map blue, vectors pink...), as their wires do. |
| **a minimap of a large model on the canvas** | on | A small map in the canvas's corner when the model does not fit the view: click or drag on it to move the view. |
| **wires go around the boxes** | on | A wire that would cross another node's box is bent above or below it. A wire with reroute points of your own is left as you drew it. |
| **a node's main settings on its box** | on | A node's box lists the settings it needs, the ones given and its essential ones, in rows under its ports; the others are counted in a row that unfolds them. Off: the boxes show ports only. |
| **a start page on the canvas of an empty model** | on | The ways into a first model (your data, a template, a model of before) and into the documentation, over the canvas while the model has no node. Off: the bare canvas. |
| **open the Node pane by itself** | off | The **Node** tab opens beside the model with its first node. Off: a node's settings open in a panel beside its box, from its **⚙** or a click on a setting the box does not edit; the tab stays in **View**. |

Under **When Terraflux starts**:

| Label | Default | What it does |
|---|---|---|
| **reopen the models open last** | on | When Terraflux starts without a model given, the models that were open open again, each with its view: where the canvas looked, the node selected, what the Viewer showed. A model opened later gets its view back either way. |

## Runs

Under **While a model runs**:

| Label | Default | What it does |
|---|---|---|
| **keep the computer awake while a model runs** | on | While a model runs, **Many runs** goes on, or this computer works for another's batch, the computer does not go to sleep. The screen may still go dark. |

Under **Outputs**:

| Label | Default | What it does |
|---|---|---|
| **keep a copy of each run's outputs up to** *n* MB | 50 MB (0 to 100,000) | The small outputs of each run are copied into the `.terraflux` folder beside the model, so the **Runs** pane can still show and compare them after a later run writes over the files. The oldest copies go first. 0 copies nothing. |
| **(all copies up to** *n* MB**)** | 1,024 MB (1 to 1,000,000) | The most all the copies of a model take. Greyed out when the first number is 0. |

Under **When a run ends**:

| Label | Default | What it does |
|---|---|---|
| **tell when a run of more than** *n* s **ends** | on, 10 s (0 to 3,600) | When a run you asked for, or a batch of **Many runs**, takes this long or longer and ends while Terraflux is not in front, the system shows a notification and the window's button in the taskbar lights up ([Notifications](/app/window/#notifications)). |

Under **Python**:

| Label | Default | What it does |
|---|---|---|
| **write the model's Python script beside it, `<model>.py`, on each save and run** | off | The same script as **File > Python script...**, rewritten beside the model's file each time the model is saved and each time a run you asked for starts. |

## Performance

| Label | Default | What it does |
|---|---|---|
| **Threads**: *n* of the *N* cores (a quarter left free) | on (automatic) | How many of the computer's cores the operators use. Automatic is the cores minus a quarter of them, and at least one (on 20 cores: 15; on 8: 6). Turn it off to choose a number from 1 to that most with the slider. Terraflux never takes every core, so the computer stays usable while a model runs. The change takes effect at the next start. |
| **Cache** | 10 GB | The size under which the rasters made without a file (reprojected, clipped...) and the data downloaded are kept, from 1 to 200 GB on a logarithmic slider. The least recently used go beyond that size. It shows how many GB are used, and **Empty** clears the cache. Hover over it for the cache's folder. |

The cache's folder is `TERRAFLUX_CACHE` if that variable is set; else `%LOCALAPPDATA%\Terraflux\cache` on Windows,
`~/Library/Caches/Terraflux` on macOS and `$XDG_CACHE_HOME/terraflux` (`~/.cache/terraflux` when it is not set) on
Linux. The environment variable `TERRAFLUX_CACHE_GB` sets the size when the settings do not (0 for no limit).

## Cloud storage

The keys for reading `s3://`, `gs://` and `/vsiaz/` data in the Viewer and in runs. They are given to GDAL, the
library that reads these files. A field left empty leaves the environment's variable in charge (the same
variables GDAL reads by itself); the command line and the workers use the environment's variables only.

| Group | Fields |
|---|---|
| **Amazon S3, or compatible** | **access key id**; **secret key**; **session token** (temporary credentials only); **region** (`us-east-1`); **endpoint** (another S3 storage, as `host:port` or `http://host:port`); **public buckets, requests not signed** (a switch). |
| **Google Cloud Storage** | **access key id** and **secret key** (the interoperability keys); **service account** (the key's `.json` file); **public buckets, requests not signed**. |
| **Azure Blob Storage** | **account**; **account key**. |

The secret fields are masked. [Secrets](#secrets) says where they go. Everything is empty by default.

## Node libraries

Nodes of your own, saved with **Save as my node...**, go to your own library folder (shown at the top of **Shared
libraries**). This section adds more.

| Label | What it does |
|---|---|
| **Add a shared library...** | A folder of nodes shared with others (a group's folder on a shared drive, a repository cloned). Its nodes are offered beside yours and are saved in it when chosen. An **x** beside a folder stops using it; the folder stays. A `library.toml` in a folder may name its writers (`writers = ["ana", "bruno"]`), the only ones who save in it. |
| **Add a remote library** (a box and a button) | The address of a git repository (`https://github.com/lab/nodes`) or of a folder on the web. A git repository is cloned and pulled when Terraflux starts (git must be installed); a web folder is read from its `index.txt`, which Terraflux writes in each library it saves in. Remote nodes are offered beside yours and only read. **Update** fetches them again; an **x** stops using one. |

Your own library is `TERRAFLUX_NODES` if that variable is set; else a `nodes` folder in the application's data folder.

## Keyboard

A table of every action with its shortcuts, to change. See [Keyboard and mouse](/app/shortcuts/#changing-a-shortcut).

## About and updates

| Label | Default | What it does |
|---|---|---|
| **Version** | | The version of Terraflux and of GDAL, with how many file formats GDAL reads (*GDAL 3.x · n formats*), or *no GDAL: GeoTIFF only* if it is not found. |
| **look for a new version when Terraflux starts (once a day)** | on | Asks GitHub which Terraflux is the latest; nothing else is sent. |
| **Check now** | | Asks now. If a newer version is out, a button *Update to...* downloads, checks and installs it when Terraflux closes. An orange dot on the section says one is waiting. |

## Back to the defaults

**Back to the defaults**, at the foot of the column, puts every setting back as in a fresh installation, **except**:

- your **favourites** and the **names** of the collections;
- the choices of **Computers** (sharing this computer's batches, and lending it to another's, with their codes,
  threads and idle time);
- the **node libraries**, shared and remote;
- the **shortcuts**;
- the **cloud storage** keys.

Everything else goes back: the theme and sizes, the model-builder and run options, the threads and cache size,
the recent models, the Files pane's folders, the scale bar and grid, the figure's look, the basemaps you added,
the providers of the Assistant, and the marks of the update check. It does not touch the layout of the panes, the
secrets in the system's store, or anything saved beside a model.

## Where the settings are kept

The settings are one file, `settings.json`, a text file whose keys have the names of the settings. It is written when a
setting changes. The file is in the application's data folder, `Terraflux`:

| System | Settings file |
|---|---|
| Windows | `%APPDATA%\Terraflux\settings.json` |
| macOS | `~/Library/Application Support/Terraflux/settings.json` |
| Linux | `$XDG_CONFIG_HOME/Terraflux/settings.json`, or `~/.config/Terraflux/settings.json` when `XDG_CONFIG_HOME` is not set |

Set the environment variable **`TERRAFLUX_SETTINGS`** to the path of a file to use another settings file: one per
project, a portable installation, or a clean file for a test. The application's other data (the `recovery` copies and your own `nodes` library) follow the settings file's folder. Two files do not: the
layout of the panes, `layout.json`, and your own templates in `templates`, which stay in the default folder above.
On Linux and macOS the file is readable by you alone.

### Secrets

Secrets are never written to `settings.json` or to a model. They are kept in the system's store of credentials: the
Windows Credential Manager, the macOS Keychain or the Linux secret service (the service is named *Terraflux*).
They are:

- the **secret key** and the **session token** of Amazon S3, the **secret key** of Google Cloud Storage and the
  **account key** of Azure ([Cloud storage](#cloud-storage));
- the key of each provider of the **Assistant** that asks for one, kept by the provider's address.

A settings file written by an older Terraflux with these secrets in it is read once: the secrets move to the store
and the file is written again without them.

### Other things kept in the settings file

Not every setting is in the window. The file also keeps, by what you do:

| Kept | What |
|---|---|
| Favourites and their names | The operators in each coloured collection, and the names given to the seven collections. |
| Recent models | The last ten models opened or saved, for **File > Open recent**. |
| The Files pane | The folder it shows, the folders pinned to its places and the ones shown last. |
| **Scale bar** and **Coordinate grid** | The two aids of the Viewer's map ([The viewer](/app/viewer/#the-coordinate-grid)). |
| The figure's look | The font, the text size, and where the legend and scale bar stand ([Figures for publication](/app/figures/)). |
| The Viewer's window | Where it was on the screen and how large, when it is in a window of its own. |
| Custom basemaps | Up to five tile addresses, the last used first. |
| The Assistant's providers | The servers of chat models and the one chosen. The presets are *HU Berlin (KI-Service)*, *GWDG Chat AI (Academic Cloud)*, Ollama and LM Studio on this computer, OpenAI, *Claude (Anthropic)* and *Other*; the keys are in the system's store. |
| **Computers** | Whether this computer shares its batches or lends itself, with the codes, the threads and the idle time. |
| The update check | When it last asked, the version that ran last and the version an update was to install. |
| The shortcuts you changed | By the action's name ([Keyboard and mouse](/app/shortcuts/)). |

Keys the application does not know are ignored, and a file that cannot be read is read as if it were empty (the defaults).

## Where Terraflux keeps other things

| What | Where |
|---|---|
| The layout of the panes | `layout.json` in the data folder above. |
| Your templates | A `templates` folder beside it. |
| Edits not saved (autosave) | A `recovery` folder in the settings file's folder, a copy every 30 seconds while a model has edits not saved. |
| The journal of a model's runs | A `.terraflux` folder beside the model's file. |
| The cache of data and rasters | See [Performance](#performance). |
| The tiles of basemaps | `web-tiles` under `%LOCALAPPDATA%\Terraflux` on Windows and `~/Library/Caches/Terraflux` on macOS; on Linux in the system's temporary folder. |

Terraflux has no telemetry and sends no crash reports. It reaches the network to download a dataset of the catalog
or the reference data an operator needs on first use, to show a basemap or a layer from a URL, to talk to an
assistant you chose, to fetch a remote library, and, if it is on, for the check for a new version.
