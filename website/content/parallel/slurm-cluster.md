---
title: A university cluster with Slurm
short: A cluster with Slurm
description: Run Terraflux batches on a university's high-performance computing cluster without logging in - what a cluster, a login node, a scheduler and an allocation are, SSH keys, terraflux cluster add and submit --on, the job script it writes, and how to be a good neighbour on a shared machine.
order: 30
---

Most universities have a **cluster**: hundreds of computers in a machine room, shared by researchers, reached over
the network. For a batch of thousands of runs, or of runs that take hours, it turns weeks into hours. Clusters
have a reputation for being hard to use: a terminal, a scheduler, job scripts. Terraflux hides most of it: you save
the cluster once, and `terraflux submit model.toml --replicates 500 --on hpc` runs the batch there from your own
computer, starting and stopping everything it needs. This page explains the words you will meet, how to get an
SSH key working (the one step you must do yourself), what Terraflux does on the cluster on your behalf, and how to
behave on a machine you share with hundreds of colleagues.

## The idea

### What a cluster is

![Your computer, the login node, the scheduler and the nodes](/img/guide/parallel/hpc.png "Your computer reaches the cluster's login node by SSH. Terraflux starts its coordinator there and asks the scheduler (Slurm) for nodes; the scheduler starts the workers on the nodes it gives, and they connect to the coordinator.")

- **Nodes**: the cluster's computers, each with many cores (32 to 128) and much memory. They are not reached
  directly from outside.
- **The login node**: the one computer you log into (`login.cluster.edu`), by **SSH**, to prepare files and submit
  work. It is shared by everyone logged in: it is for light work only.
- **The scheduler**: the program that shares the nodes among users. You do not start programs on the nodes
  yourself; you ask the scheduler for resources (so many nodes, cores per node, for so long) with a **job script**,
  and it starts your job when they are free. **Slurm** (Yoo et al., 2003) is the most common scheduler; its
  commands are `sbatch` (submit a job script), `squeue` (see the queue), `scancel` (cancel a job).
- **An allocation**: the nodes a job received, for its **walltime** (the time limit you asked for: the job is
  stopped when it runs out).
- **Partitions** (queues of nodes with different limits: `short`, `long`, `gpu`) and **accounts** (the project the
  time is charged to).
- **Fair share**: the more you used recently, the lower your priority. Asking for less often starts sooner.

### What Terraflux does on the cluster

For each batch, from your computer, `terraflux submit ... --on hpc`:

<div class="steps" markdown>

1. **checks** Terraflux on the other side, of the same version (`terraflux` on its PATH,
   `~/.terraflux/bin/terraflux`, or the `--program` saved);
2. **starts a coordinator** on the login node (unless one runs), with the profile's pairing code, listening on the
   profile's port, without beacons, its jobs in `~/.terraflux/coordinator-<port>`;
3. **asks Slurm for the workers**: it writes a job script and runs `sbatch` (`--nodes`, each with `--cpus`
   threads for `--per-node` workers, `--time`, `--partition`, `--account`); when Slurm starts the job, the workers
   connect to the coordinator on the login node by its name;
4. **reaches the coordinator through an SSH tunnel**, sends the data, follows the batch and fetches its files;
5. **stops** the workers (`scancel`) and the coordinator when the batch is done or stopped.

</div>

The coordinator on the login node is a light process: it holds a queue and moves files, it computes nothing. The
computing happens on the nodes Slurm gives.

!!! science "Key idea: the job script Terraflux writes"
    For a profile of 4 nodes of 32 threads with 2 workers each, the job script is, in essence:

    ```bash
    #!/bin/bash
    #SBATCH --job-name=terraflux-workers
    #SBATCH --nodes=4
    #SBATCH --ntasks-per-node=2
    #SBATCH --cpus-per-task=16
    #SBATCH --time=02:00:00
    #SBATCH --output=slurm-terraflux-%j.log
    #SBATCH --partition=short
    export OMP_NUM_THREADS="$SLURM_CPUS_PER_TASK"
    srun --kill-on-bad-exit=0 bash -c 'terraflux worker --coordinator login01:<port> --code <code> \
        --name "$(hostname)-$SLURM_PROCID" --threads "$SLURM_CPUS_PER_TASK" \
        --cache "${TMPDIR:-/tmp}/terraflux-cache-$SLURM_JOB_ID-$SLURM_PROCID"'
    ```

    Eight workers (`--ntasks-per-node` × `--nodes`), each with 16 threads (`--cpus` / `--per-node`), their data
    cached on each node's local disk. `srun --kill-on-bad-exit=0` keeps the others running if one fails.

## When to use it

Use a cluster when the batch would take days on your computers: thousands of replicates, a calibration of several
settings with many seeds, scenarios with replicates, or runs that each take hours. For a single large map, the
tiled engine can also use the cluster's nodes ([Maps larger than memory](/parallel/maps-larger-than-memory/)).

## What you need

- **An account** on the cluster (ask your university's research computing service), and the name of its login node.
- **Terraflux of the same version** there: unpack the Linux archive (`terraflux-<version>-linux-x64.tar.gz`) in your
  home folder and link the command to `~/.terraflux/bin/terraflux`, or ask the administrators for a module. The
  Linux build needs glibc 2.39 or newer (Ubuntu 24.04's); on an older system, ask for a container or build it there.
- **An SSH key** that logs in without a password prompt (below).
- Your **partition** and **account** names, and the cluster's limits (cores per node, longest walltime).

### SSH keys, once

Terraflux uses your system's own SSH (OpenSSH: built into Windows 10 and later, macOS and Linux), with your keys
and your `~/.ssh/config`. It runs SSH in batch mode, so the login must not ask for a password:

```bash
ssh-keygen -t ed25519                       # a key pair in ~/.ssh (give it a passphrase)
ssh-copy-id me@login.cluster.edu            # Linux, macOS: the public key added on the cluster
# Windows (PowerShell):
type $env:USERPROFILE\.ssh\id_ed25519.pub | ssh me@login.cluster.edu "cat >> ~/.ssh/authorized_keys"
ssh me@login.cluster.edu                    # must now log in without the cluster's password
```

With a passphrase, load the key into an agent once per session (`ssh-add`; on Windows, start the "OpenSSH
Authentication Agent" service). A `~/.ssh/config` entry gives the cluster a short name and its options:

```text
Host hpc
    HostName login.cluster.edu
    User me
    IdentityFile ~/.ssh/id_ed25519
```

Some clusters require a second factor at every login; ask whether SSH keys from your computer are allowed, or use
`ControlMaster` connections (one login, reused). `TERRAFLUX_SSH` names another SSH program if needed.

## Run it

### Save the cluster once

```bash
terraflux cluster add hpc --host me@login.cluster.edu --slurm --nodes 4 --cpus 32 --per-node 2 \
    --time 02:00:00 --partition short --account my-project
terraflux cluster test hpc        # SSH, and Terraflux's version there
terraflux cluster list            # the clusters and servers saved
```

```tfcli
cluster add
```

`--cpus` is the threads of each node used, `--per-node` the workers per node (each with `--cpus` / `--per-node`
threads), `--nodes` the nodes of the allocation, `--time` its walltime. The profile is kept in the user's
application data (`clusters.toml`), with a pairing code and a port of its own. A server of your own (not a cluster)
is saved with `--workers N` instead of `--slurm`: N workers on the host itself, sharing its threads.

### Run batches there

```bash
terraflux submit model.toml --replicates 500 --on hpc --out replicates
terraflux calibrate model.toml --param patches.isometry=0.5:2.5 --observed landuse.tif:2011 --on hpc
```

Nothing else changes: the results come back to `--out` on your computer, as from a batch on your computer. If your
computer goes away during the batch (switched off, the network lost), the batch goes on there; follow it again,
and fetch its files, with:

```bash
terraflux results job-4 --on hpc --out replicates
terraflux cluster stop hpc        # stop Terraflux's coordinator and workers there (its jobs kept)
terraflux cluster remove hpc      # forget the profile
```

In the application, the **Computers** pane adds and checks clusters and servers (**Add a cluster or server...**:
the user and host, the SSH port, Terraflux there, where the runs go, threads and runs per node, partition and
account), and the **Many runs** pane offers them in **Run on**.

### By hand, with the template script

If you prefer to run Slurm yourself, `tools/slurm/terraflux-workers.sbatch` in Terraflux's repository starts
workers on the nodes of an allocation for a coordinator you started:

```bash
terraflux serve --listen 0.0.0.0:7700 --code              # on the login node (or a node the workers reach)
sbatch --export=COORDINATOR=http://login01:7700 terraflux-workers.sbatch
terraflux submit model.toml --replicates 100 --coordinator http://login01:7700 --code <code> --out results
```

The template has no code in its worker command: add `--code <code>` to it when the coordinator has one.

### A first session, step by step

<div class="steps" markdown>

1. **Log in once by hand** (`ssh hpc`) and look around: `sinfo` lists the partitions and their nodes,
   `sacctmgr show assoc user=$USER` (or the documentation of your cluster) your accounts, `df -h ~` the space of your
   home folder. Note the partition for short jobs and its longest walltime.
2. **Install Terraflux** in your home: copy the Linux archive (`scp terraflux-<version>-linux-x64.tar.gz hpc:`),
   unpack it, and link the command: `mkdir -p ~/.terraflux/bin && ln -s ~/terraflux-*/terraflux ~/.terraflux/bin/terraflux`.
   `~/.terraflux/bin/terraflux --version` must print your version.
3. **Save the profile** on your computer (`terraflux cluster add ...`) with a small allocation: 1 node, its cores,
   30 minutes, the short partition.
4. **Test it**: `terraflux cluster test hpc`.
5. **Run a small batch**: `terraflux submit model.toml --replicates 20 --on hpc --out test`. Watch it on the cluster
   with `squeue -u $USER` in another terminal: the job `terraflux-workers` appears, runs, and disappears when the
   batch ends.
6. **Read `test/tasks.csv`**: the seconds per run on the cluster's nodes. Size the real allocation from it, change
   the profile (`cluster remove`, then `add`), and run the real batch.

</div>

## Read the results

The results are those of any batch ([Many runs on your computer](/parallel/many-runs/#what-a-batch-writes)); in
`tasks.csv`, the `worker` column names each node (`node042-0`, `node042-1`...). On the cluster, Slurm writes the
workers' output to `~/.terraflux/slurm-terraflux-<job>.log` and the coordinator its own to
`~/.terraflux/serve-<port>.log`: the first places to look when something fails ([Troubleshooting](/parallel/troubleshooting/)).

## Being a good neighbour

A cluster is shared by hundreds of people; its rules are there so that everyone gets a fair turn.

- **Read the cluster's rules.** Some forbid any long-running process on the login node, even a light one like
  Terraflux's coordinator. Ask the administrators; if so, run the coordinator on a node of its own (a small, long
  job) and the workers in another job, with the template script above.
- **Ask for what you use.** `--cpus` should match the threads your workers use, `--time` the batch's length plus a
  margin. A job asking for 4 nodes for 48 hours waits longer, and blocks others, more than one asking for 4 hours.
- **Time a small batch first** (10 runs), then size the allocation: runs × seconds per run / workers.
- **Stop what you started.** Terraflux cancels its job when the batch ends; after an interrupted batch, check
  `squeue -u $USER` and run `terraflux cluster stop hpc`.
- **Keep data where it belongs.** Large inputs and results belong in the cluster's scratch or project space, not
  in a small home folder; the workers' cache is on each node's local disk (`$TMPDIR`).
- **Acknowledge** the cluster in your papers, as most computing services ask.

## Common pitfalls

- **SSH asks for a password**: Terraflux's SSH runs in batch mode and fails. Set up the key and the agent, and
  check that `ssh me@login.cluster.edu` logs in without a prompt.
- **Another version of Terraflux there**: `cluster test` says which; install the same.
- **The job waits in the queue**: it is the scheduler, not Terraflux. `squeue -u $USER` shows the reason; ask for
  fewer nodes or less time, or another partition.
- **Workers that cannot reach the login node**: some clusters do not let nodes connect to the login node's ports.
  Ask the administrators, or run the coordinator on a node.
- **A walltime too short**: the workers are stopped when it ends, and the tasks they were running go back to the
  queue; the batch waits for new workers. Ask for enough time.

## Going further

- [The cloud](/parallel/cloud/): machines rented by the hour, set up by a script.
- [Troubleshooting](/parallel/troubleshooting/).

## References

- Yoo, A. B., Jette, M. A., & Grondona, M. (2003). SLURM: Simple Linux Utility for Resource Management. In D. Feitelson, L. Rudolph, & U. Schwiegelshohn (Eds.), *Job Scheduling Strategies for Parallel Processing* (Lecture Notes in Computer Science, Vol. 2862, pp. 44–60). Springer. https://doi.org/10.1007/10968987_3
