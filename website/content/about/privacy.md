---
title: Privacy and network access
short: Privacy and network
description: What Terraflux sends over the network and when, what it never sends, and the hosts and ports an institution's IT needs to know.
order: 60
---

Terraflux runs on your computer and works on your files. It needs no account and no network to build and run a model
with your own data. This page lists everything it can send over the network, when, and how to stop it. It is written
from the program's source code (version {{version}}); if you find a difference, please
[report it](/about/contributing/).

## In short

- **No telemetry, no analytics, no crash reports, no account.** Terraflux does not send usage statistics, errors,
  your models, your maps or your settings to its author or to anyone else. Nothing is sent when you start it, except
  the update check below, which you can turn off.
- **Your models and data stay on your computer** unless you choose to send them: by using the assistant with an
  online provider, by reading or writing a cloud bucket, or by running on other computers.
- **Passwords and keys** (an assistant's key, cloud storage secrets) are kept in your system's store of credentials
  (Windows Credential Manager, macOS Keychain, the Secret Service on Linux), never in the settings file and never in
  a model.

## Always on, and how to turn it off

### The update check

When Terraflux starts, and at most once a day, it asks GitHub for the latest release: a request to
`https://api.github.com/repos/sacridini/terraflux/releases/latest`. The request carries no model, no file and no
identifier; as for any web request, GitHub sees your IP address and the program's name and version in the
`User-Agent` (`Terraflux/{{version}}`). If a newer version exists, the toolbar says so. The new version is downloaded
only if you ask for it, from the release's page on GitHub, and checked against the release's `SHA256SUMS.txt`
before it is installed.

To turn it off: **Settings, About & updates, Updates**, untick *look for a new version when Terraflux starts*.
**Check now** in the same place checks once, when you ask. The command line (`terraflux run` and the others) never
checks for updates.

Nothing else happens by itself. In particular, the map shows no basemap until you choose one, and no data is
downloaded until a node or a layer that reads it runs.

## When you ask for it

### Basemaps

A basemap under the maps (the choice in the map's toolbar; the default is none) is read as tiles, as you pan and
zoom, from the provider you chose. The provider sees the tiles your view needs (so, roughly, where you are looking),
your IP address and `Terraflux/{{version}}` as the program's name. The tiles are kept in a cache folder (see
[Where Terraflux keeps its things](/quickstart/install/#where-terraflux-keeps-its-things)).

| Basemap | Host |
|---|---|
| Esri World Imagery, Esri World Topographic Map | `server.arcgisonline.com` |
| OpenStreetMap | `tile.openstreetmap.org` |
| OpenTopoMap | `tile.opentopomap.org` |
| CARTO Light | `basemaps.cartocdn.com` |
| A basemap of your own (up to five, a tile address you give) | the host you give |

Each provider has its own terms of use and privacy policy; the public servers of OpenStreetMap and OpenTopoMap are
meant for light use.

### Data downloaded by operators

The operator `data.download` (and the data catalogue of the application, which uses it) reads data straight from the
servers of the providers, only the part your grid covers, when the node runs. `data.environment90m` and the operators
below do the same. What a server sees is the files asked for (so, the area and the variable) and your IP address.

| Data | Host |
|---|---|
| WorldClim 2.1 and its CMIP6 futures | `geodata.ucdavis.edu` |
| CHELSA 2.1 | `os.zhdk.cloud.switch.ch` |
| Copernicus DEM (30 and 90 m) | `copernicus-dem-30m.s3.amazonaws.com`, `copernicus-dem-90m.s3.amazonaws.com` |
| SoilGrids | `files.isric.org` |
| MapBiomas Brazil | `storage.googleapis.com` |
| Hansen's Global Forest Change | `storage.googleapis.com` |
| ESA WorldCover | `esa-worldcover.s3.eu-central-1.amazonaws.com` |
| Hydrography90m, Environment90m | `public.igb-berlin.de` |
| MODIS (vegetation indices, land surface temperature, burned area) | `planetarycomputer.microsoft.com` (and the storage it points to) |
| Natural Earth reference data (`sdm.reference_data`) | `naciscdn.org` |
| GBIF occurrence records (`sdm.download_gbif`) | `api.gbif.org` |

GBIF is asked for the species name you give and the filters you set. Movebank is not contacted: the movement
operators read the files you export from it.

### Your own data on the web and in the cloud

A raster or vector address you give (`https://...`, `s3://`, `gs://`, `/vsiaz/`, a WMTS, WFS or OGC API Features
service) is read from that server, only when a node or a layer uses it. Terraflux hands the work to GDAL, which
signs the requests with the credentials you gave under **Settings, Cloud storage** (or the usual environment
variables, such as `AWS_ACCESS_KEY_ID`, when you run from the command line). The secrets of the settings are kept in
the system's store of credentials. The tickboxes *no sign request* read public buckets without any key.

### Shared libraries of nodes

If you add a remote library of nodes in the settings (a git repository, or a folder on a web server), Terraflux
clones or reads it when the application starts and when you ask, and only reads it. There is none by default.

### The assistant

The **assistant** (the chat of the application) is off until you choose a provider. When you send a message to it,
Terraflux sends **to the provider you chose**: your question, the conversation so far, the application's
instructions to the model, and **the model that is open** (its nodes and settings as text, or the list of its nodes
when it is large, the model's title and the path of its file). It also sends what the tools return when the
assistant asks to read or run something (a node's result, the problems of a check, a file's summary). Your maps are
not sent as files, but numbers and names that the assistant asks for are part of the conversation. Do not use an
online provider with a model or data that must not leave your institution.

| Provider | Host |
|---|---|
| HU Berlin (KI-Service; university network or VPN) | `llm3-compute.cms.hu-berlin.de` |
| GWDG Chat AI (Academic Cloud) | `chat-ai.academiccloud.de` |
| OpenAI | `api.openai.com` |
| Claude (Anthropic) | `api.anthropic.com` |
| Ollama or LM Studio on this computer | `localhost:11434`, `localhost:1234`: nothing leaves the computer |
| Any other server you give | the host you give |

The provider's key, when it asks for one, is sent to that provider in the request and kept in the system's store of
credentials. Each provider's own privacy policy says what it does with what it receives.

### Assistants of your own (the MCP server)

`terraflux mcp` is a server that talks to an AI program on **your computer** through its standard input and output:
Terraflux opens no port and sends nothing to the network for it. What the assistant program (Claude Desktop, Claude
Code, Gemini CLI, Codex...) sends to its own provider is decided by that program, not by Terraflux: read its
privacy policy. See [Using an AI assistant](/cli/assistants/).

### Several computers

The coordinator and workers of [parallel and distributed computing](/parallel/) talk to each other, and only to each
other. A coordinator started with `terraflux serve` listens on `127.0.0.1:7700` by default: only this computer.
With `--listen 0.0.0.0:7700` it is open to the local network, and then:

- with a **pairing code** (`--code`), the connections are encrypted (TLS), the coordinator's identity is pinned and
  every call carries a token made from the code. Computers without the code cannot take tasks or read results;
- **without a code, the connection is plain and open** to anyone who can reach the port: use it only on a network you
  trust.

What travels is the tasks of a job (the model, its inputs and the parameters), the results, and the status of the
workers. A coordinator with a code announces itself on the local network every two seconds by a UDP broadcast to
port 7701; it carries the coordinator's port, its computer's name, its version and a beacon made from the code (it
does not give the code away). It is not sent beyond the local network, and a coordinator listening on `127.0.0.1`
says nothing to the network. Starting workers on another computer uses `ssh` (or the command in `TERRAFLUX_SSH`)
and a tunnel to the coordinator; machines rented in the cloud and set up by Terraflux's script download Terraflux's Linux package from
`github.com/sacridini/terraflux/releases` unless you give another place with `--package`.

## What stays on your computer

Terraflux keeps on your computer: its settings, a copy of unsaved edits (to recover after a crash), the cache of
results and of data downloaded, the basemaps' tiles, and the runs' outputs and manifests. They are listed in
[Where Terraflux keeps its things](/quickstart/install/#where-terraflux-keeps-its-things). A run's `manifest.json`
records the version, the settings, and fingerprints of your files: check it before you share it, as it may hold the
paths of your folders.

## For IT departments: hosts and ports

All connections go out on port 443 (HTTPS) except where noted. Terraflux checks certificates with the system's own store. If your network needs a proxy or inspects TLS, test
the update check and a data download first.

| Purpose | Hosts | Port | When |
|---|---|---|---|
| Update check, and the update | `api.github.com`, `github.com`, and GitHub's download hosts for release files | 443 | At start, once a day (can be turned off); the download only if asked |
| Basemaps | `server.arcgisonline.com`, `tile.openstreetmap.org`, `tile.opentopomap.org`, `basemaps.cartocdn.com` | 443 | If a basemap is chosen |
| Data operators | the hosts of the table above | 443 | When such a node runs |
| Assistant | the provider chosen (hosts above); `localhost:11434` or `:1234` for a local model | 443 (local: 11434, 1234) | When a message is sent |
| Coordinator and workers | the computers you choose | TCP 7700 (default, configurable) | Distributed runs |
| Coordinator's beacons | local network broadcast | UDP 7701 | A coordinator with a code, open to the network |
| Remote node libraries | the git or web host you give | 443 or 22 | If you add one |

## This website

The pages of this website are served by GitHub Pages. They set **no cookies** and have **no analytics, tracking
scripts or advertising**. The site remembers your light or dark choice in your browser's local storage
(`tf-theme`); it never leaves your browser. To draw the pages, your browser loads **fonts from Google Fonts**
(`fonts.googleapis.com`, `fonts.gstatic.com`) on every page, and, on the pages with mathematics, **KaTeX from
jsDelivr** (`cdn.jsdelivr.net`). Those services see your IP address and the page's address as they would for any
site that uses them; GitHub also records requests to its servers, under
[its privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).
