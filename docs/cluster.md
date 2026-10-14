# Many runs on many machines

A batch is a model run many times with something changed each time: the seed (replicates), a set of inputs and
settings (scenarios), the settings a calibration tries. Terraflux runs batches on the cores of one machine or on
the workers of a coordinator, with the same commands. Every run is the exact engine's, so a task gives the same
maps on any worker as on the machine that submitted it.

## From the application

The toolbar's **Many runs...** opens a pane with the three kinds of batches, run on workers of this computer
(as many at once as chosen; each gets an equal share of the threads):

- **Replicates:** the number of runs and the first seed. The seeds the runs change are found in the model (its
  inputs named `seed`, else the seeds its nodes are given), or ticked by hand.
- **Scenarios:** a table, a row per scenario and a column per setting it changes, any of the model's inputs and
  nodes' settings picked by name (an empty cell keeps the model's value).
- **Calibration:** settings in ranges (every combination, or random draws of a Latin hypercube), the seeds of
  each try, the simulated maps, the map they start from (an input or a `raster.read`), the observed map and its
  band, and the score.

Each run keeps the model's outputs, the files its nodes write (in the run's folder) and the outputs ticked
under "Each run keeps" (by default, the maps of the nodes where the model ends). The results go to a folder per
run (`<model> runs/replicates 1/seed_1/...`) with `tasks.csv`; "Show the runs of ... in the model" adds a
`raster.read` of `runs/*/file.tif`, the runs as one series. A calibration writes `calibration.csv` and
`best.json`, plots its scores against each setting, and "Apply the best settings to the model" puts them in it.

### Other computers of the network

The toolbar's **Computers...** opens a pane of two parts, and nothing else to set up:

- **Other computers help this one:** "Let other computers help" shows this computer's code (`K7QM-2XPA-9DTE`).
  The runs of "Many runs" then also go to the computers of the local network given the code.
- **This computer helps another:** type the other's code and "Lend this computer". It finds the other on the
  network by itself and runs some of its runs: with the threads chosen, only when nobody uses it for some minutes
  (Windows and macOS), or paused at will. Without the window: `terraflux worker --code K7QM-2XPA-9DTE`.

Only the computers given the code take part, and what travels is encrypted (see "With a code" below). The
installer lets Terraflux through the Windows firewall for the local network; elsewhere, the system may ask the
first time. A network that does not carry broadcasts (some offices' Wi-Fi) needs the address instead:
`terraflux worker --code ... --coordinator <the other's address>:7700`.

The batches run on the application's own coordinator (its jobs in the user's application data, `pool`), with
as many workers of this computer as "Runs at once on this computer" says.

A batch goes on if the application is closed before its end: opened again, the pane offers to **go on** (the
runs done are not run again, their files not fetched again) or to let it go. While a batch runs, the computer
does not go to sleep.

## On one machine

Without `--coordinator`, `submit` and `calibrate` start a coordinator and `--workers` workers in their own
process (a quarter of the cores by default):

```bash
terraflux submit model.toml --replicates 20 --out replicates --workers 8
terraflux submit model.toml --scenarios scenarios.toml --out scenarios
terraflux calibrate model.toml --param patches.isometry=0.5:2.5 --param patches.expansion=0.2:0.8 \
    --observed landuse.tif:2011 --metric similarity:5 --seeds 1,2,3 --workers 8
```

`submit` writes a folder per task with the model's outputs and `tasks.csv` (worker, seconds, errors).
`calibrate` writes `calibration.csv` (every candidate, its settings and score, best first), `best.json`, and the
model with the best settings next to the original (`model_calibrated.toml`).

### Scenarios

```toml
[[scenario]]
name = "business_as_usual"
set = { "$seed" = 1 }

[[scenario]]
name = "compact_growth"
set = { "$seed" = 1, "patches.isometry" = 2.0, "patches.expansion" = 0.8 }

[[scenario]]
name = "other_drivers"
set = { "$drivers" = "drivers_2030.tif" }
```

`$name` sets an input (a value, or the path of a file input); `node.setting` sets a node's setting or literal
input (`simulation/allocate.prune_factor` for a node inside the loop). `--seed-input` names what holds the seed
of replicates and calibrations: an input (`seed`), a node's (`simulation.seed`), or several, separated by commas.

The files the nodes write (`raster.write`, `vector.write`) go to each task's folder, by their names.

### Calibration

Each `--param key=from:to` is a range: `--method grid` tries `--n` values of each (every combination;
`from:to:points` gives a range its own number), `--method random` draws `--n` candidates (a Latin hypercube).
Every candidate runs with each of `--seeds`; its score is the mean. The metric is computed on the workers, the
last simulated map against the observed one from the same start (`--start`, the input `t0` by default):

| `--metric` | |
|---|---|
| `fom` | the figure of merit: hits / (hits + misses + false alarms + wrong hits) |
| `overall` | the share of cells of the same class |
| `similarity:<window>` | Dinamica EGO's fuzzy similarity of the changes in a window (1, 3, 5, 7, 9 or 11) |

## On many machines

```bash
# a machine the others reach
terraflux serve --listen 0.0.0.0:7700

# every machine that works (any number, started and stopped at any time)
terraflux worker --coordinator http://coordinator-host:7700

# anywhere that reaches the coordinator
terraflux submit model.toml --replicates 200 --coordinator http://coordinator-host:7700 --out results
terraflux workers --coordinator http://coordinator-host:7700

# a batch left to run: submitted, its files fetched later (those fetched already kept)
terraflux submit model.toml --replicates 200 --coordinator http://coordinator-host:7700 --detach
terraflux jobs --coordinator http://coordinator-host:7700
terraflux results job-3 --coordinator http://coordinator-host:7700 --out results
```

- **Connections:** workers only connect to the coordinator, so they need no open port and can run behind
  firewalls or NAT.
- **One version:** workers and clients of another version of Terraflux are turned away, with the version to
  install (every machine runs the same engine: the same maps).
- **Data:** the data travels once, in pieces of a megabyte, whatever its size. The client sends each file once
  (by its SHA-256), and each worker fetches it once into its cache (`--cache`). Every file the model reads goes:
  its inputs, the files of `raster.read` and `vector.read` (with the files beside them of the same name: a
  shapefile's `.shx`, `.dbf` and `.prj`, a raster's `.aux.xml`), the rasters of a folder or a pattern
  (`runs/*/landuse.tif`), plugins, a calibration's observed map, and the files a scenario names
  (`$drivers = "drivers_2030.tif"`, `roads.path = "roads_2030.gpkg"`). On a worker they are read from its
  cache under their own names.
- **Results:** a task's files go to the coordinator as they are written, and the client fetches each run's
  files as soon as it is done (a batch stopped half way keeps what it fetched).
- **The coordinator's disk:** it keeps the data, the results' files and its jobs in a folder (`--data`; by
  default the user's application data, `Terraflux/coordinator`). Started again on it, it goes on with its jobs:
  the tasks done stay done, those that were running run again. A job fetched to its end is forgotten, with its
  files.
- **Workers that vanish:** a worker sends heartbeats while it runs a task. If it is not heard of for `--timeout`
  seconds (60 by default), the task goes to another worker; a task is tried three times.
- **Computers that sleep:** a worker keeps its computer awake while it runs a task (Windows' execution state,
  `caffeinate` on macOS, `systemd-inhibit` on Linux); the screen may still turn off.
- **A computer someone uses:** `terraflux worker --idle 10` takes tasks only after 10 minutes without the
  keyboard or the mouse (Windows and macOS); a task taken runs to its end.
- **Slurm:** `tools/slurm/terraflux-workers.sbatch` starts workers on the nodes of a Slurm allocation.

### With a code

```bash
# the coordinator makes a code (or takes one: --code K7QM-2XPA-9DTE) and says on the network where it is
terraflux serve --listen 0.0.0.0:7700 --code

# workers and clients with the code find it by themselves on the local network
terraflux worker --code K7QM-2XPA-9DTE
terraflux submit model.toml --replicates 200 --code K7QM-2XPA-9DTE --out results

# or by its address (another network, a VPN)
terraflux worker --coordinator coordinator-host:7700 --code K7QM-2XPA-9DTE
```

- **The code:** 12 characters of Crockford's base 32 (60 bits; case, dashes and spaces do not matter, O reads 0
  and I and L read 1). A key is made from it slowly (PBKDF2-HMAC-SHA256, 100 000 rounds).
- **Encryption:** TLS 1.2 or 1.3, with a certificate the coordinator makes once and keeps in its folder.
- **Who is who:** a client or worker accepts the coordinator only once it proves it knows the code (an HMAC of
  the certificate it presented, made with the key: someone in between with a certificate of their own cannot
  make it); the certificate is then pinned. Every call then carries a token made from the key; the coordinator
  answers no call without it.
- **Found on the network:** a coordinator with a code that listens beyond this computer broadcasts every two
  seconds, on the UDP port 7701, its port and a beacon made from the key (it does not give the code away); a
  worker with the same code listens for it.

**Security without a code:** the coordinator has no authentication or encryption. Anyone who reaches its port
can submit tasks and read their data. It listens on 127.0.0.1 unless given another address. Without a code,
expose it only on a trusted network, or reach it through an SSH tunnel (`ssh -L 7700:localhost:7700
coordinator-host`).

## A cluster or a server, by SSH

A cluster (Slurm) or a server of one's own is saved once, and batches run there without logging in:

```bash
terraflux cluster add hpc --host me@login.cluster.edu --slurm --nodes 4 --cpus 32 --per-node 2 \
    --time 02:00:00 --partition short
terraflux cluster add lab --host me@lab-server --workers 8      # a server: 8 runs at once on it
terraflux cluster test hpc                                         # SSH, and Terraflux's version there

terraflux submit model.toml --replicates 500 --on hpc --out replicates
terraflux calibrate model.toml --param patches.isometry=0.5:2.5 --observed landuse.tif:2011 --on hpc
terraflux results job-4 --on hpc --out replicates                  # a batch whose client went away
terraflux cluster stop hpc
```

For each batch the client:

1. checks Terraflux on the other side, of the same version (`terraflux` on its PATH, `~/.terraflux/bin/terraflux`,
   or `--program`);
2. starts its coordinator there (unless it runs), with a pairing code of the profile's, listening on a port of the
   profile's, its jobs in `~/.terraflux/coordinator-<port>`;
3. starts the workers: on the host (`--workers`, sharing its threads), or in a Slurm allocation (an `sbatch` of
   `--nodes`, each with `--cpus` threads for `--per-node` workers, `--time`, `--partition`, `--account`; the workers
   reach the coordinator on the login node by its name);
4. reaches the coordinator through an SSH tunnel, sends the data, follows the batch and fetches its files;
5. stops the workers (`scancel`) and the coordinator once the batch is done or stopped.

A batch whose client went away (the computer turned off) goes on there; `terraflux results <job> --on <name>`
follows it again. SSH is the system's own, with the user's keys and `~/.ssh/config`: it must log in without a
password asked (a key, an agent). The profiles are kept in the user's application data (`clusters.toml`). In
the application, the Computers pane adds and checks them, and "Many runs" offers them in **Run on**.

## What travels

| Message | From | To |
|---|---|---|
| the model (JSON), the overrides of each task, the evaluation | client | coordinator, then each worker |
| the files the model reads (once, by hash, in pieces) | client | coordinator, then each worker once |
| a task's output files (maps as GeoTIFF, tables as CSV), or its metrics | worker | coordinator, then client |

| a pass of the tiled engine (what to compute, the tiles of each task, the layers' tiles by hash) | client | coordinator, then each worker |
| the model's maps (once), the layers' tiles of a task's tiles and their neighbours (by hash) | client | coordinator, then the worker |
| a pass's part of some tiles (histograms, counts, keys), the tiles it wrote | worker | coordinator, then client |

## One large map on many machines

Batches run many small models; the tiled engine runs one model too large for a machine (a continent at
30 m). Its tiles are spread over the workers:

```bash
terraflux worker --coordinator http://coordinator-host:7700          # on every machine that works
terraflux run model.toml --tiled --disk run-1/layers --coordinator http://coordinator-host:7700
```

- **No storage in common:** the model's maps go to each worker once (by hash, as a batch's files). The steps'
  layers are kept on the disk of the machine that runs the model (`--disk`), a file per tile; a pass lists each
  layer's tiles and their hashes, a worker fetches those of its tiles and of their neighbours (the halos; every
  tile of a layer when a halo is wider than a tile), and sends back by hash the tiles it wrote, which are fetched
  into the layer's folder. A tile goes once to each worker that needs it.
- **Shared storage** (`--shared-disk`): the workers read the model's maps and read and write the steps' layers
  themselves, at the same paths as the machine that runs the model (NFS, SMB, a cluster's parallel file system);
  only the passes and their sums travel through the coordinator.
- **Passes:** every pass over the tiles (a histogram, a count, the growth of the patches) is a job whose
  tasks are runs of contiguous tiles (`4` per worker); the machine that runs the model merges their parts
  and decides the next pass.
- **The same maps:** the parts merge in any order into the same sums, so the maps are those of the same model
  run with `--tiled` on one machine, however many workers and whichever computed which tiles.
- **Workers that vanish:** as with batches, their tasks go to other workers; a task writes whole tiles under
  their final names only when done, so a task run twice writes the same tiles.

