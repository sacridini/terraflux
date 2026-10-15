---
title: Many machines
description: Coordinators, workers, pairing codes, clusters reached by SSH and machines rented in the cloud, command by command.
order: 30
---

These commands spread batches (and the tiles of very large maps) over many computers: the lab's desktops, a
server, a university cluster, machines rented in the cloud. This page is the reference of each command; the
[guide to parallel and distributed computing](/parallel/) explains the ideas and walks through each setting
from the beginning.

## How the pieces fit

Three roles take part, and one computer may play several:

Coordinator (`terraflux serve`)
:   Keeps the batches (jobs), splits them into tasks, hands each task to a worker, and keeps the results until
    they are fetched. It needs one port the others can reach (7700 by default).

Workers (`terraflux worker`)
:   Ask the coordinator for tasks, run them, and send back what they wrote. They only connect *to* the
    coordinator, so they can sit behind firewalls. They can be started and stopped at any time.

Client (`terraflux submit`, `calibrate`, `run --tiled`, `results`)
:   Sends a batch with its data to the coordinator, follows it and fetches its files.

Every machine must run the **same version** of Terraflux: a worker or client of another version is turned away
with the version to install, so that every machine computes the same maps. The data travels once: each file is
sent by its fingerprint (SHA-256) and each worker keeps it in its cache.

## terraflux serve

```tfcli
serve
```

```bash
terraflux serve                                   # this computer only (127.0.0.1:7700)
terraflux serve --listen 0.0.0.0:7700 --code      # the network, with a pairing code (printed)
terraflux serve --listen 0.0.0.0:7700 --code K7QM-2XPA-9DTE --data D:\terraflux-jobs
```

- `--listen` decides who can reach it: `127.0.0.1` this computer only, `0.0.0.0` every network the machine is
  on.
- `--code` encrypts every connection (TLS) and lets in only the computers given the code. The coordinator also
  announces itself on the local network, so workers with the code find it without its address
  (`--no-beacons` turns that off).
- `--data` is where it keeps the data and the jobs. Started again on the same folder, it goes on with its jobs.
- `--timeout` is how long a worker may stay silent before its task goes to another one (60 seconds).
- When it starts, it prints the command that starts its workers: on `127.0.0.1` with its address
  (`terraflux worker --coordinator 127.0.0.1:7700 --code ...`, since it sends no beacons there and only this computer
  reaches it), on the network with the code alone.

!!! danger "Never expose a coordinator without a code"
    Without `--code`, a coordinator has no password and no encryption: anyone who reaches its port can submit
    tasks and read their data. Use a code, or keep it on `127.0.0.1` and reach it through an SSH tunnel
    (`ssh -L 7700:localhost:7700 server`).

## terraflux worker

```tfcli
worker
```

```bash
terraflux worker --code K7QM-2XPA-9DTE                        # finds the coordinator on the local network
terraflux worker --coordinator server.lab:7700 --code K7QM-2XPA-9DTE   # by its address (another network, a VPN)
terraflux worker --code K7QM-2XPA-9DTE --idle 10 --threads 6  # a desktop lent when nobody uses it
```

- `--idle 10` takes tasks only after 10 minutes without keyboard or mouse (Windows and macOS); a task already
  taken runs to its end. While it runs a task, a worker keeps its computer awake.
- `--threads` is what each task may use; leave some cores free on a computer someone works on.
- `--cache` is where the data of the tasks is kept between batches.
- A network that does not carry broadcasts (some Wi-Fi) needs `--coordinator` with the address.

In the application, the **Computers** pane does the same in two clicks: show this computer's code so others can
help it, or type another's code to lend this computer.

## terraflux workers and terraflux jobs

What the coordinator's workers are doing, and its batches:

```bash
terraflux workers --code K7QM-2XPA-9DTE
terraflux jobs --coordinator http://server.lab:7700
```

`workers` prints a line per worker: its name, threads, tasks done, and `idle`, `running job-4/1`, or
`silent (stopped?)` when the coordinator has not heard of it for over 30 seconds; with none, `no workers`.

Batches are submitted with `submit` and `calibrate` (see [Batches](/cli/batches/)) by adding `--coordinator`
or `--code`; without either, they start a coordinator and workers on this machine.

## terraflux cluster

A cluster (with the Slurm scheduler) or a server is saved once as a **profile**; `submit --on <name>`,
`calibrate --on` and `results --on` then run batches there through SSH, without you logging in: Terraflux starts a
coordinator on the remote machine, starts the workers (directly, or as a Slurm job), opens an SSH tunnel to it,
sends the data, follows the batch, fetches the files, and stops everything when the batch ends.

```tfcli
cluster
```

```tfcli
cluster add
```

```bash
# a Slurm cluster: 4 nodes of 32 threads, 2 workers per node, 2 hours, in the partition "short"
terraflux cluster add hpc --host me@login.cluster.edu --slurm --nodes 4 --cpus 32 --per-node 2 \
    --time 02:00:00 --partition short

# a server: 8 runs at once on it
terraflux cluster add lab --host me@lab-server --workers 8

terraflux cluster test hpc        # SSH works, and Terraflux there has this version
terraflux cluster list
terraflux submit model.toml --replicates 500 --on hpc --out replicates
terraflux cluster stop hpc        # stop its coordinator and workers (the jobs are kept)
terraflux cluster remove lab
```

Before the first use:

1. SSH must log in **without asking a password**: with a key (`ssh-keygen`, then `ssh-copy-id me@host`, or the
   cluster's own instructions) or an agent. Test it with `ssh me@host` first.
2. Terraflux of the same version must be installed there: on the `PATH`, in `~/.terraflux/bin/terraflux`, or
   where `--program` says. The Linux archive needs no root.
3. `terraflux cluster test <name>` checks both.

`--host` can be an alias of `~/.ssh/config`, which is the easiest way to use a jump host, another port or a
special key. `TERRAFLUX_SSH` names another SSH program. The profiles are kept in the user's application data
(`clusters.toml`).

A batch whose computer was turned off goes on on the cluster; `terraflux results <job> --on <name>` follows it
again. For those who prefer to write their own job scripts, `tools/slurm/terraflux-workers.sbatch` in the
repository starts workers in a Slurm allocation.

## terraflux cloud-init

Prints the setup script ("user data") that a cloud provider runs when it creates a machine (AWS, Google Cloud,
Azure, DigitalOcean, Hetzner... all accept cloud-init). The machine downloads and installs this version of
Terraflux by itself.

```tfcli
cloud-init
```

```bash
terraflux cloud-init > server.yaml                    # a server with Terraflux installed
terraflux cluster add cloud --host ubuntu@203.0.113.7 --workers 16
terraflux submit model.toml --replicates 1000 --on cloud --out replicates

# more machines working for that server's coordinator (its address on the provider's private network)
terraflux cloud-init --worker --on cloud --coordinator 10.0.0.5 > worker.yaml
```

A worker machine keeps `terraflux worker` running as a service, started again if it stops; the service runs as an
unprivileged user made by systemd (`DynamicUser=yes`), writing only its cache (`/var/cache/terraflux`). The provider's
firewall must let the workers reach the server's port. Machines rented by the hour cost money while they run,
even idle: shut them down when the batch is fetched.

## Running one very large map on many machines

`terraflux run --tiled --coordinator` sends the tiles of a model too large for one machine to the workers; see
[Running models](/cli/models/#maps-larger-than-memory) and the guide's page on maps larger than memory.

```bash
terraflux worker --coordinator http://server:7700              # on every machine that works
terraflux run model.toml --tiled --disk run-1/layers --coordinator http://server:7700
```

With `--shared-disk`, workers that see the same files at the same paths (a cluster's parallel file system, a
network share) read and write the layers themselves, and only sums travel.
