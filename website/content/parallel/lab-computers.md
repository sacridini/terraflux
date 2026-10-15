---
title: The lab's computers
short: The lab's computers
description: Put the computers of a lab, an office or a home to work on one batch - a coordinator and its workers, pairing codes, how computers find each other, what is encrypted, how data travels, what happens when a computer goes away, and the Computers pane.
order: 20
---

Most labs have more computers than they use at night: the desktops of students, a workstation, an old server
under a desk. Terraflux can make them work on the same batch of runs, with nothing to install but Terraflux and
nothing to configure but a code. This page explains the pieces (a **coordinator**, its **workers**, a **client**),
how the computers find each other and prove who they are, how the data and the results travel, what happens when a
computer is switched off in the middle of a run, and how to do it all from the application's **Computers** pane.
Everything shown was run on one computer, with two workers playing the lab's machines.

## The idea

### A coordinator and its workers

![A coordinator and its workers](/img/guide/parallel/coordinator.png "Your computer submits a batch to the coordinator; workers on other computers ask it for tasks, fetch the files they lack once, run, send heartbeats and send the results back.")

- The **coordinator** (`terraflux serve`) keeps the queue of tasks, the data of the batches and the results, in a
  folder of its own (`--data`, by default the user's application data, `Terraflux/coordinator`). It does no
  computing itself; any computer can be it, ideally one that stays on.
- A **worker** (`terraflux worker`) asks the coordinator for a task, fetches the files the task needs that it does
  not have yet, runs the model, sends a **heartbeat** every 5 seconds while it runs, sends the files the run wrote,
  and asks again. Any number of workers, started and stopped at any time.
- A **client** (`terraflux submit`, `terraflux calibrate`, or the application's Many runs) sends the batch (the
  model, the settings of each task, its files) and fetches each run's files as soon as it is done.

Workers and clients only open connections *to* the coordinator, never the other way: they need no open port, and
can sit behind a firewall or a home router.

### A code: who may take part

On a shared network you do not want anyone to submit tasks to your coordinator or read your data. A **pairing
code** solves this without accounts or certificates to manage:

![How a worker finds and trusts the coordinator](/img/guide/parallel/pairing.png "The coordinator announces itself on the local network (a beacon made from the code, not the code); the worker connects by TLS, checks that the coordinator knows the code, pins its certificate, and from then on every call carries a token made from the code.")

!!! science "Key idea: what the code does"
    The code (for example `K7QM-2XPA-9DTE`) is 12 characters of Crockford's base 32, 60 random bits; case, dashes
    and spaces do not matter, and O reads 0, I and L read 1. A key is derived from it slowly (PBKDF2-HMAC-SHA256,
    100,000 rounds), so guessing codes is expensive. With the key:

    - **found on the network**: a coordinator listening beyond its own computer broadcasts every two seconds, on
      the UDP port 7701, its port and a beacon made from the key; a worker with the same code recognises it;
    - **encrypted**: every connection is TLS 1.2 or 1.3, with a certificate the coordinator makes once;
    - **proved both ways**: the coordinator proves it knows the code (an HMAC of its certificate made with the key;
      someone in between with a certificate of their own cannot make it), the certificate is then pinned, and every
      call carries a token made from the key; the coordinator answers no call without it.

Without a code, a coordinator has no authentication and no encryption: anyone who reaches its port can submit
tasks and read their data. It then listens only on its own computer (`127.0.0.1`) unless told otherwise; expose it
only on a trusted network, or reach it through an SSH tunnel (`ssh -L 7700:localhost:7700 coordinator-host`).

### Data travels once, by hash

Every file a model reads (its inputs, the files of [[raster.read]] and [[vector.read]] with the files beside them
of the same name, such as a shapefile's `.dbf`, the rasters of a pattern, plugins, a calibration's observed map,
the files a scenario names) goes to the coordinator once, identified by its **SHA-256 hash**, in pieces of a
megabyte. Each worker fetches each file once into its cache (`--cache`) and reads it there under its own name. A
second batch with the same drivers sends nothing again. Results go the other way: a task's files to the
coordinator as they are written, then to the client as soon as the run is done.

### Computers that go away

![A worker stopped during a run](/img/guide/parallel/failure.png "A real run: two workers, two tasks of about 3.5 s. lab-pc-1 was stopped during its task; lab-pc-2 finished its own; 60 s without a heartbeat later, the coordinator gave the stopped task to lab-pc-2, and the batch ended 63 s after the stop.")

A worker that is not heard of for `--timeout` seconds (60 by default, a setting of `serve`) loses its task, which
goes back to the queue for another worker; a task is tried three times. Since every run is the exact engine's with
its own seed, a task run again gives the same maps. The coordinator keeps its jobs on disk: stopped and started
again on the same `--data`, it goes on (tasks done stay done; those that were running run again). A worker keeps
its computer awake while it runs a task (the screen may still turn off).

### What travels

| Message | From | To |
|---|---|---|
| the model (as JSON), the settings of each task, the metric of a calibration | client | coordinator, then each worker |
| the files the model reads (once, by hash, in pieces of a megabyte) | client | coordinator, then each worker once |
| a task's output files (maps as GeoTIFF, tables as CSV), or its score | worker | coordinator, then client |
| heartbeats (every 5 seconds while a task runs) | worker | coordinator |

Nothing else: the workers never see each other, never open your files directly, and keep only their cache.

## When to use it

Use the lab's computers when a batch would take more than a night on one computer, and the lab has computers that
are idle at some hours: tens to hundreds of replicates, scenarios with replicates, calibrations of several
settings. For thousands of runs or runs of hours each, a [cluster](/parallel/slurm-cluster/) is better; for one map
too large for memory, see [Maps larger than memory](/parallel/maps-larger-than-memory/).

## What you need

- The **same version of Terraflux** on every computer: a worker or client of another version is turned away, with
  the version to install (the same engine must give the same maps).
- A **network** between them. On a home or lab network the computers find each other by the code. A network that
  does not carry broadcasts (some offices' Wi-Fi, a VPN) needs the coordinator's address instead.
- The **firewall**: the coordinator's port (7700 by default) must accept connections from the others, and UDP 7701
  carries the beacons. The Windows installer lets Terraflux through the firewall for the local network; elsewhere
  the system may ask the first time.

## Run it

### From the command line

```bash
# on the computer that coordinates: listen on the network, with a code (printed)
terraflux serve --listen 0.0.0.0:7700 --code

# on every computer that works: it finds the coordinator by its code
terraflux worker --code K7QM-2XPA-9DTE

# anywhere: submit a batch and fetch its results
terraflux submit model.toml --replicates 200 --code K7QM-2XPA-9DTE --out results
```

`serve` prints where it listens and its code:

```text
coordinator 0.80.0 listening on 127.0.0.1:7795, its jobs kept in ...\lab\coordinator
its code: 6ARR-5REK-J0Z2
start workers on this computer with `terraflux worker --coordinator 127.0.0.1:7795 --code 6ARR-5REK-J0Z2` (listening on 127.0.0.1, it is not seen from other computers: --listen 0.0.0.0:7795 for them)
```

Here it listens on `127.0.0.1` for the test: on this address it sends no beacons and only this computer reaches
it, so it prints the command with its address. Listening on the network (`--listen 0.0.0.0:7700`), it prints the
command with the code alone, `terraflux worker --code ...`, which finds it by its beacons. With a wrong code, every
command is refused:

```text
Error: the coordinator at 127.0.0.1:7795 does not know this code
```

On a network without broadcasts, give the address: `terraflux worker --coordinator 192.168.1.20:7700 --code ...`.
`serve --code K7QM-2XPA-9DTE` reuses a code (so that workers started earlier keep working);
`--no-beacons` keeps the coordinator silent on the network.

Useful options of `worker`: `--threads` (the threads of each task, by default all the cores but one), `--name` (how
it appears in the lists), `--cache` (where its data is kept), and `--idle 10`: take tasks only after 10 minutes
without the keyboard or the mouse (Windows and macOS), so that a colleague's computer works only when the colleague
does not (a task taken runs to its end).

### A night on the lab's computers, step by step

<div class="steps" markdown>

1. **Choose the coordinator**: a computer that stays on all night (yours, or the lab's server). Start
   `terraflux serve --listen 0.0.0.0:7700 --code` (or tick **Let other computers help** in the Computers pane) and
   note the code.
2. **Install the same version** of Terraflux on the other computers (the installer, or the zip).
3. **Lend them**: on each, `terraflux worker --code <code> --idle 10`, or Computers, **Lend this computer**, with
   *Only when nobody uses this computer for* 10 minutes. Their owners can stop lending at any time.
4. **Check** that they are there: `terraflux workers --code <code>`, or the list in the Computers pane.
5. **Submit** the batch, detached so that your own computer may sleep if it is not the coordinator:
   `terraflux submit model.toml --replicates 400 --code <code> --detach`.
6. **In the morning**, `terraflux results <job> --code <code> --out results` fetches the files; `tasks.csv` says
   which computer ran what.

</div>

### Following the work

```bash
terraflux workers --code K7QM-2XPA-9DTE        # the workers and what they do
terraflux jobs --code K7QM-2XPA-9DTE           # the jobs, done or not
```

```text
worker-1	lab-pc-1	4 threads	2 tasks done	silent (stopped?)	seen 64 s ago
worker-2	lab-pc-2	4 threads	4 tasks done	idle	seen 0 s ago

job-5	Dinamica EGO's classic model	0 of 3 done	1 running	0 failed
```

A worker is `idle` (waiting for a task), `running` a task (`running job-4/1`: the job and the task), or
`silent (stopped?)` when the coordinator has not heard of it for over 30 seconds: here lab-pc-1 had been stopped a
minute before. A worker running a task sends a heartbeat every few seconds, so a silent one is usually switched off,
asleep or cut from the network. With no worker at all, `workers` says so: ``no workers: start one with `terraflux
worker` and the coordinator's address or code``.

A batch can be left to run: `--detach` submits it and prints its job, and `terraflux results` follows it to its end
later, from any computer that reaches the coordinator, writing the files (those fetched already are kept):

```bash
terraflux submit model.toml --replicates 200 --code K7QM-2XPA-9DTE --detach
# job-5
# its results later: terraflux results job-5 --code K7QM-2XPA-9DTE --out batch
terraflux results job-5 --code K7QM-2XPA-9DTE --out results
```

A job fetched to its end is forgotten by the coordinator, with its files: `jobs` no longer lists it.

### In the application: the Computers pane

The toolbar's **Computers...** opens a pane in two parts, with nothing else to set up:

- **Other computers help this one**: tick **Let other computers help**, and the pane shows **This computer's
  code** (with **Copy**, and **New code**: the computers given the old one stop helping) and the port it listens
  on. From then on, the runs of the Many runs pane also go to the computers of the network given the code; the
  pane lists them, with their threads, their runs done and whether they are waiting or running.
- **This computer helps another**: type the code another computer shows and **Lend this computer**. It finds the
  other on the network by itself and runs some of its runs, with the **Threads it uses** chosen, optionally **Only
  when nobody uses this computer for** some minutes (Windows and macOS). **Stop lending** at any time.

Without the window, the same lending is `terraflux worker --code ...`. The same pane saves clusters and servers
reached by SSH ([A cluster with Slurm](/parallel/slurm-cluster/)) and copies the setup scripts of machines rented in
the cloud ([The cloud](/parallel/cloud/)).

## Read the results

The results are those of a batch on one computer ([Many runs on your computer](/parallel/many-runs/#what-a-batch-writes)):
a folder per task and `tasks.csv`, whose `worker` column says which computer ran each task, and `seconds` how long.
Four replicates of about 3.4 seconds on two workers of 4 threads took 7.0 seconds from the submit to the last file
written, data transfer included:

```text
task,worker,seconds,ok,error
seed_1,worker-2,3.47,true,""
seed_2,worker-1,3.40,true,""
seed_3,worker-1,3.31,true,""
seed_4,worker-2,3.31,true,""
```

The maps are the same as on one computer: each task's run is the exact engine's with its seed.

## Settings that change the answer

None of the settings of this page changes a map; they change how long a batch takes and how robust it is:
`--timeout` (how soon a vanished worker's task is given away: too short, and a slow task on a busy computer is run
twice), `--threads` of each worker, `--idle`, and the number of workers.

## Common pitfalls

- **Workers that do not find the coordinator**: a network without broadcasts, a firewall blocking UDP 7701, or a
  coordinator listening on `127.0.0.1` (it then sends no beacons, and only its own computer reaches it: it says so
  when it starts). Give `--coordinator <address>:7700` with the code, and listen on `0.0.0.0`. See [Troubleshooting](/parallel/troubleshooting/).
- **A coordinator without a code on a shared network**: anyone can read the data. Use a code.
- **Computers of different versions**: install the same version everywhere.
- **The coordinator's computer switched off**: the workers wait; start it again on the same `--data` and the jobs go
  on.
- **Laptops on battery**: a worker keeps its computer awake while it runs a task; plug laptops in.

## Going further

- [A cluster with Slurm](/parallel/slurm-cluster/): hundreds of cores, with the same commands.
- [Troubleshooting](/parallel/troubleshooting/).

```tfcli
serve
```

```tfcli
worker
```
