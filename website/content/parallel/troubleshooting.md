---
title: Troubleshooting, and which tool for which job
short: Troubleshooting
description: What to do when a batch is slow, a worker does not find its coordinator, SSH refuses, a cluster job waits or a run fails on a worker, with the messages Terraflux prints, and a table of which tool fits which job.
order: 60
---

Parallel computing adds machines, networks and schedulers between you and your results, and each can fail in its
own way. This page collects the problems people meet most, what Terraflux says when they happen, and what to do,
from the simplest case (one computer) to the cluster. It ends with a table of which tool fits which job, the
summary of this guide.

## The idea: find which layer fails

A batch passes through layers, and a problem belongs to one of them. Check them in this order:

<div class="steps" markdown>

1. **The model.** Does `terraflux run model.toml` work on your computer, alone? If not, nothing parallel will.
   `terraflux check model.toml` finds errors of the model file.
2. **The batch on one computer.** Does `terraflux submit model.toml --replicates 2` work here? It uses the same
   coordinator and workers as the other cases, in one process.
3. **The network.** Can the other computer reach the coordinator's address and port (and UDP 7701 for the codes)?
4. **The remote machine.** Is Terraflux there, of the same version? Does SSH log in without a password?
5. **The scheduler.** Did Slurm start the workers' job, or is it waiting?

</div>

The logs: `submit` prints the progress and each failure; `tasks.csv` has a row per task with its `error`;
`terraflux workers` lists the workers and when each was last heard of; on a cluster, `~/.terraflux/serve-<port>.log`
(the coordinator) and `~/.terraflux/slurm-terraflux-<job>.log` (the workers).

## Problems and what to do

### On one computer

| Symptom | Likely cause | What to do |
|---|---|---|
| Every replicate gives the same map | the model reads no seed, or not the one replicates set | check `--seed-input`; the Many runs pane says "the model has no seed" |
| `--param x=0:1` tries only 0 and 1 | the model gives the setting as a whole number (`x = 1`), or does not give it and the ends are whole numbers | give it with decimals in the model (`x = 1.0`), or write `0.0:1.0` |
| The computer crawls during a batch | too many workers for the memory (swapping) | fewer `--workers`; watch the memory with a small batch |
| A batch gets no faster with more workers | memory, disk, slower cores, other programs; one run hardly uses more than one thread | 4 to 8 workers on a desktop; add machines instead ([overview](/parallel/#measured-how-much-faster-on-one-desktop-computer)) |
| A task fails, `ok = false` in `tasks.csv` | the model fails with that task's settings (a scenario's file missing, a setting out of range) | read its `error`; run the model alone with the same values (`--set seed=3`, `--set patches.isometry=2`) |
| `scenarios: ...` or `two scenarios are named ...` | the scenarios file is not valid TOML, or repeats a name | fix the file |

### On the network

| Symptom | Likely cause | What to do |
|---|---|---|
| A worker started with `--code` prints its first line and then nothing | it found no coordinator with that code: the coordinator listens on `127.0.0.1` (no beacons), runs `--no-beacons`, or the network does not carry broadcasts (some Wi-Fi, VPNs), or a firewall blocks UDP 7701 | give the address: `terraflux worker --coordinator <address>:7700 --code <code>`; start `serve --listen 0.0.0.0:7700` |
| `the coordinator at ... does not know this code` | a mistyped code, or the coordinator was given a new one | copy the code again (Computers pane, **Copy**); case, dashes and spaces do not matter |
| `... is not a code (12 letters and digits, as K7QM-2XPA-9DTE)` | a code of the wrong length | copy it again |
| `this worker is Terraflux X, the coordinator Y: install Terraflux Y on this machine` | different versions | install the same version everywhere |
| `terraflux workers` says `no workers` | no worker reached this coordinator: they were not started, or look for another code or address | start them with the command `serve` printed; see the first row |
| A worker listed as `silent (stopped?)` | not heard of for over 30 seconds: switched off, asleep, its program stopped, or cut from the network | start it again; its running task goes to another worker after `--timeout` |
| A worker is listed but takes no task | `--idle` and someone uses the computer; or it is running a long task | wait, or check `terraflux workers` |
| A task runs twice | its worker was not heard of for `--timeout` seconds (60), so the task went to another | a busy or sleeping computer; raise `serve --timeout`, keep computers awake |
| `the blob ... arrived damaged` | a file corrupted in transit (its hash does not match) | run the batch again; if it repeats, check the network and the disks |
| The coordinator's computer was switched off | | start `serve` again with the same `--data`: the jobs go on |

### SSH, clusters and the cloud

| Symptom | Likely cause | What to do |
|---|---|---|
| `ssh ...: ...` errors at once | SSH asks for a password (batch mode refuses), the host is unknown, or the key is not loaded | `ssh me@host` must log in without a prompt: a key ([SSH keys](/parallel/slurm-cluster/#ssh-keys-once)), `ssh-add`, accept the host key once by hand |
| `... has Terraflux X, this computer Y: install the same version on both` | another version on the cluster | install the same version there (`~/.terraflux/bin/terraflux`, or `--program`) |
| The batch waits and no worker appears | Slurm has not started the job | `squeue -u $USER` on the login node: the reason (priority, resources); ask for fewer nodes or less time, or another partition |
| Workers start (in `squeue`) but never connect | the nodes cannot reach the login node's port | ask the administrators; or run the coordinator in a job of its own and the workers with the template script |
| `the tunnel stopped` | the SSH connection dropped | the batch goes on there; `terraflux results <job> --on <name>` follows it again |
| Workers stop in the middle of a batch | the walltime ran out | ask for a longer `--time`, or run smaller batches |
| A cloud machine never answers | cloud-init is still installing, or failed | wait a few minutes; on the machine, `/var/log/cloud-init-output.log`; `--package` if the release cannot be downloaded |

### The tiled engine

| Symptom | Likely cause | What to do |
|---|---|---|
| `the tiled engine runs Dinamica EGO's classic model: ...` | the model has something it does not run (no loop, dynamic variables in the loop, another allocation) | run it with the exact engine, or simplify the loop |
| `the tiled Expander and Patcher take a 3 x 3 window` | `window` other than `[3, 3]` | use the default window |
| Maps differ between tile sizes | the patches measured by [[patches.observe]] within tiles | give them with [[patches.settings]] |
| Many machines are slower than one | each pass is a round trip through the coordinator | use many machines only for maps that do not fit on one |
| The disk fills | `--disk` keeps every layer of every step | a larger disk: budget several times the land use map's size per step |

## Reading a batch's traces

`tasks.csv` is the first place to look after any batch. A healthy batch of replicates on two lab computers:

```text
task,worker,seconds,ok,error
seed_1,worker-2,3.47,true,""
seed_2,worker-1,3.40,true,""
seed_3,worker-1,3.31,true,""
seed_4,worker-2,3.31,true,""
```

What to look for:

- **`ok = false`**: read `error`, and repeat that task alone on your computer with its settings
  (`terraflux run model.toml --set seed=3`, and a scenario's node settings as `--set patches.isometry=2`).
- **One worker much slower** than the others (`seconds`): a busy computer, a slower processor, a machine swapping;
  give it fewer threads or leave it out.
- **One worker running nearly every task**: the others are not taking tasks (asleep, `--idle`, or lost); check
  `terraflux workers`, whose last column says how long ago each was heard of. A worker that was stopped stays in the
  list, marked `silent (stopped?)` once it has not been heard of for over 30 seconds, with a growing "seen ... ago".
- **Tasks that took far longer than one run on your computer**: too many workers on one machine, or a disk too
  slow for the files.

## Asking for help

When a problem resists, the people who can help (a colleague, your cluster's administrators, Terraflux's issue
tracker) need the same few things: the command you ran and what it printed, the version (`terraflux --version`) on
each machine, where each part ran (your computer, a lab computer, a cluster's login node or nodes), `tasks.csv`, and
for a cluster the two logs in `~/.terraflux`. Leave out codes and private data: a pairing code gives access to the
coordinator.

## When to use which tool

The summary of this guide:

| Job | Size | Tool | Command |
|---|---|---|---|
| One model, once | fits in memory | your computer, all cores but one | `terraflux run model.toml` |
| Replicates, scenarios, a calibration | up to a night on one computer | your computer, a batch | `terraflux submit --replicates`, `--scenarios`; `terraflux calibrate` |
| The same, larger | days on one computer, a few computers at hand | the lab's computers, with a code | `terraflux serve --code`, `terraflux worker --code`, `submit --code` |
| The same, much larger | thousands of runs, or hours each | a cluster with Slurm | `terraflux cluster add --slurm`, `submit --on` |
| A burst, without a cluster | a deadline, a few days | machines rented in the cloud | `terraflux cloud-init`, `cluster add --workers`, `submit --on` |
| A batch left to run | any | any coordinator | `submit --detach`, `terraflux jobs`, `terraflux results` |
| One land change model of a huge area | larger than memory | the tiled engine, one machine | `terraflux run --tiled --disk layers` |
| The same, faster | billions of cells | the tiled engine on workers | `run --tiled --disk layers --coordinator ...` (`--shared-disk` on a cluster's file system) |
| Landscape metrics, zonal statistics of a huge map | larger than memory | the same operators by blocks | `block_rows`, automatic beyond 250 million cells |
| MSPA of a huge map | larger than memory | GuidosToolbox's tiles | [[landscape.mspa_tiled]] |

And in the application: **Many runs** for batches (on this computer, the computers helping it, or a saved cluster),
**Computers** to share runs with the network, lend this computer, save clusters and servers, and copy cloud scripts,
**Settings** for the threads.

## Common pitfalls

- **Debugging the parallel part first.** Run the model alone, then a batch of two on your computer, then elsewhere.
- **Changing two things at once** (a new version and a new cluster): change one, test, then the next.
- **Forgetting what was started.** After an interrupted batch on a cluster: `squeue -u $USER`,
  `terraflux cluster stop <name>`. After tests on your computer: stop the `serve` and `worker` you started.

## Going further

- Back to the [overview](/parallel/), or to [Many runs on your computer](/parallel/many-runs/).
- The tutorials that use batches: [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/).

```tfcli
workers
```

```tfcli
results
```
