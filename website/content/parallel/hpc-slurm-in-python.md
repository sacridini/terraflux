---
title: "On a cluster (HPC, Slurm) in Python"
short: "On a cluster (HPC, Slurm) in Python"
description: "Scripts in batch jobs with the processors of the job (less a quarter), folders of rasters over job arrays, time limits and nodes without internet access."
order: 910
level: advanced
time: 35 min
---

The same in the app: [A university cluster with Slurm](/parallel/slurm-cluster).

A cluster runs work as **batch jobs**: a script asks a scheduler (Slurm, PBS,
LSF) for processors, memory and time, and runs on a compute node when they are
free. Terraflux notices when it runs inside a job and adapts by itself, so the
script you run on your computer runs unchanged in a job: it uses the
processors the job was given, keeps its downloads where the nodes can reach
them, and does not try to open windows.

**You will learn to:**

- install Terraflux on a cluster, with pip or a container;
- run a script in a job, with the processors it asked for, less a quarter;
- split a folder of rasters among the tasks of a **job array**, follow it and
  submit again what failed;
- survive the time limit of a job;
- prepare the data for compute nodes without internet access;
- spread the tasks of one script over several nodes;
- find out what Terraflux sees on a node.

The commands are Slurm's, the most common scheduler; PBS and LSF work the same
way where the text says so.

## Install

On the cluster's **login node** (the computer you reach with `ssh`), in a
virtual environment kept in your home or project folder, which the compute
nodes see too:

<!-- no-run -->
```bash
module load python/3.12            # the name of the module differs between clusters
python -m venv ~/venvs/terraflux
source ~/venvs/terraflux/bin/activate
pip install terraflux-py
```

The wheels need glibc 2.28 or newer (Rocky/Alma Linux 8, Ubuntu 20.04, Debian
11 and later), on x86-64 or ARM64. On an older system, or to keep everything
in one file, use the container image with **Apptainer** (formerly
Singularity), which most clusters have:

<!-- no-run -->
```bash
apptainer pull terraflux.sif docker://ghcr.io/sacridini/terraflux-py:latest
apptainer exec terraflux.sif python analysis.py
apptainer exec terraflux.sif terraflux-py doctor
```

Apptainer runs as you and sees your home and the current folder; give it other
folders with `--bind /scratch`. Use a version instead of `latest`
(`terraflux:1.51.0`) to keep the results of a study reproducible.

## One job: a script on one node

A job script is a shell script with `#SBATCH` lines that say what the job
needs:

<!-- no-run -->
```bash
#!/bin/bash
#SBATCH --job-name=fragmentation
#SBATCH --cpus-per-task=16
#SBATCH --mem=32G
#SBATCH --time=02:00:00

source ~/venvs/terraflux/bin/activate
python analysis.py
```

Submit it with `sbatch job.sh`; `squeue -u $USER` shows it waiting or running,
and its output goes to `slurm-<job>.out`. There is no `n_threads` to set:
inside a job every tool uses **the processors of the job less a quarter** (at
least one stays free, as on a desktop: Terraflux never uses all of them). Terraflux counts them from what the scheduler
confines the job to, or, on a cluster that does not, from
`SLURM_CPUS_PER_TASK` (`SLURM_CPUS_ON_NODE` without it; `NCPUS` on PBS,
`LSB_DJOB_NUMPROC` on LSF). A node of 128 cores running a job of
`--cpus-per-task=16` gives 12 threads, not 128.

What decides the number, from the first that applies:

| | |
|---|---|
| `n_threads=` | this call |
| `tf.set_default_threads(n)` | this process |
| `TERRAFLUX_THREADS` | this process and the processes it starts |
| a dask.distributed worker | its threads |
| `OMP_NUM_THREADS` | as for any OpenMP program |
| the job | its processors, less a quarter |
| the computer | its processors, less a quarter |

Clusters often load modules that export `OMP_NUM_THREADS=1` (for MPI
programs), which would leave the job on one thread: Terraflux then warns once
and says how to change it (`export TERRAFLUX_THREADS=$SLURM_CPUS_PER_TASK`; a quarter of the processors still stays free).

## Many rasters: a job array

A folder of hundreds of tiles is better spread over many small jobs than run
in one large one: they start sooner, and a failure costs one part. A **job
array** is one script submitted as many tasks; each task gets its number in
`SLURM_ARRAY_TASK_ID`. `terraflux-py slurm` writes that script for a batch:

<!-- no-run -->
```bash
terraflux-py slurm fad tiles/ fad_out/ --cpus 16 --time 1:00:00
```

It counts the rasters, chooses the number of tasks (each task runs as many
rasters at once as it has processors), suggests the memory from the largest
raster and the tool, and prints the script it wrote to
`fad_out/.terraflux-batch/fad.sbatch`:

<!-- no-run -->
```bash
#SBATCH --job-name=terraflux-fad
#SBATCH --array=0-24
#SBATCH --cpus-per-task=16
#SBATCH --mem=6G
#SBATCH --time=1:00:00
#SBATCH --output=.../fad_out/.terraflux-batch/logs/slurm-%A_%a.out
#SBATCH --signal=B:TERM@120
#SBATCH --requeue

exec python -m terraflux batch fad tiles fad_out --shard array --requeue --threads-per-worker 1 --max-memory 5120MiB
```

Submit it with `sbatch`, or add `--submit`. Parameters of the tool follow the
folders as `key=value` (`terraflux-py slurm mspa tiles/ out/ edge_width=2`);
`--partition`, `--account`, `--tasks`, `--max-running`, `--sbatch` (any other
`#SBATCH` option) and `--setup` (a line run before Terraflux, such as
`module load ...`) shape the script.

Each task runs the same command, `terraflux-py batch ... --shard array`, and
processes its part of the rasters: in the order of the folder, raster `i` goes
to part `i mod n`. The split is kept in the output folder, so the tasks you
submit again later take the same rasters. From Python, the same is
`tf.batch(tool, inputs, output, shard="array")`, and outside an array that
processes every raster, so the script also runs on your computer:

```python
from pathlib import Path
import terraflux as tf

forest = tf.load_raster(tf.example_data("input"))
Path("tiles").mkdir(exist_ok=True)
for row in range(0, 1000, 250):
    tf.write_raster(forest[row:row + 250, :500], f"tiles/forest_{row}.tif")

runs = tf.batch("accounting", "tiles", "accounting_out", shard="array")   # every raster here
print(len(runs))
```

`shard="i/n"` gives a part by hand (part `i`, from 0, of `n`). PBS and LSF
arrays do not tell their tasks how many there are, so there it is
`--shard $PBS_ARRAY_INDEX/50` (or `$LSB_JOBINDEX`, counting from 0):

```python
first = tf.batch("accounting", "tiles", "parts_out", shard="0/2")   # rasters 0 and 2
print([run.input.name for run in first])
```

!!! note "Why not automatic"
    `tf.batch` takes its part only when asked (`shard="array"`): a script that
    runs a batch in every task of an array for another reason, a scenario per
    task for example, would otherwise process a fraction of its rasters
    without a word.

## Follow it and submit again what is left

Rasters already done are never computed again (each one's `.json`, or MSPA's
`.tif`, marks it done, written only once it is whole). `terraflux-py status`
reads the record the tasks keep in the output folder:

<!-- no-run -->
```bash
$ terraflux-py status fad_out/
fad in fad_out: 400 inputs, 391 done, 2 failed, 7 not done
  failed: tile_0137.tif (run 4567_12): MemoryError: ...
  not started: tile_0390.tif, tile_0391.tif, ...
To process the 9 left: sbatch --array=12,15-17 fad_out/.terraflux-batch/fad.sbatch
```

The last line submits only the tasks that have rasters left. The exit code is
0 when every raster is done and 3 otherwise, for scripts that chain jobs. From
Python:

```python
status = tf.batch_status("parts_out")
print(status.done, status.left)
```

## The time limit

Slurm stops a job at its time limit. With `--signal=B:TERM@120` (in the scripts
`terraflux-py slurm` writes) it warns two minutes before with SIGTERM: the
batch then starts no new raster, lets the ones at work end, and with
`--requeue` puts the task back in the queue to continue (`scontrol requeue`),
if it finished at least one raster in this run (a raster longer than the time
limit would otherwise come back forever). A job you cancel with `scancel` is
not requeued. In Python, the stop is `tf.batch.Stopped`, which holds the
results of the rasters that ended.

## Nodes without internet access

Compute nodes often cannot reach the internet. Terraflux downloads a few
things on first use: the reference data of species occurrence cleaning
(`tf.sdm.reference_data`) and some example data. Download them on the login
node, into the cache that the compute nodes see too:

<!-- no-run -->
```bash
terraflux-py fetch          # all; or: terraflux-py fetch sdm, terraflux-py fetch examples
```

On a node without access, a download says so and names the `terraflux-py fetch`
that gets it, instead of a network error. The cache is `~/.cache/terraflux/python`;
when the home quota is small, point it at the scratch space with
`export TERRAFLUX_CACHE=/scratch/$USER/terraflux-cache` (in the job script
too). Occurrence records from GBIF (`tf.sdm.download_gbif`) also need the
internet: download them on the login node and save them as a table.

Plots cannot open a window in a job: `tf.plot(...)` says so once, and
`.save("map.html")` or `.save("map.png")` writes them.

## Several nodes for one script

The functions that run many independent tasks (`batch`, `simulate.scenarios`,
`simulate.calibrate`, `sdm.fit`, `monte_carlo`, `parallel.map`...) take
`on=tf.helpers()` and share their tasks with
[helper computers](/parallel/many-computers-in-python/). In an allocation of several nodes, the
other nodes become helpers of the first:

<!-- no-run -->
```bash
#!/bin/bash
#SBATCH --nodes=4 --ntasks-per-node=1 --cpus-per-task=32 --time=04:00:00

source ~/venvs/terraflux/bin/activate
srun --nodes=3 --ntasks=3 --cpus-per-task=32 --exclude="$SLURMD_NODENAME" terraflux-py worker --connect "$SLURMD_NODENAME" &
python scenarios.py        # with tf.helpers() as pool: ... on=pool
```

Inside a job the coordinator does not look for helpers by broadcast (it does
not pass between nodes), and `terraflux-py worker` needs `--connect`. The
helpers wait for the group that `tf.helpers()` creates in the configuration
folder of your home, which every node sees. The results are the same as on one
computer. A job array is simpler when the tasks are rasters of a folder; the
helpers are for the other many-task functions.

## What Terraflux sees

`terraflux-py doctor` prints what Terraflux finds on the computer it runs on:
run it in a job to check the threads, and paste it in a question or an issue:

<!-- no-run -->
```bash
$ srun --cpus-per-task=8 terraflux-py doctor
terraflux 1.51.0 (Python 3.12.4, Linux x86_64, glibc 2.34)
  compiled core: _core.cpython-312-x86_64-linux-gnu.so, OpenMP yes, AVX2+FMA yes
Threads (the default of n_threads): 6 threads (8 of 128 processors available to this process; Slurm job 4567; a quarter kept free)
Job: Slurm job 4567 on node042
Cache: /home/me/.cache/terraflux/python (default), writable
Network:
  GBIF (sdm.download_gbif)               NO: the name does not resolve (no DNS)  (api.gbif.org)
  ...
  ! This computer seems to have no internet access ...: `terraflux-py fetch` downloads the data ...
Display: none, inside a Slurm job: plots are not shown, .save() writes them
```

From Python, `tf.doctor()`:

```python
tf.doctor(network=False)
```

## Troubleshooting

| What you see | Why, and what to do |
|---|---|
| a warning about `OMP_NUM_THREADS=1` | a module exported it: `export TERRAFLUX_THREADS=$SLURM_CPUS_PER_TASK` |
| the job is slower than the desktop | it got fewer processors: ask with `--cpus-per-task`, check with `terraflux-py doctor` |
| `Disk quota exceeded` in the cache | `export TERRAFLUX_CACHE=/scratch/$USER/terraflux-cache` |
| `could not reach ... no internet access` | `terraflux-py fetch` on the login node |
| `GLIBC_2.28 not found` when importing | the system is older than the wheels: use the Apptainer image |
| a task killed with `OUT_OF_MEMORY` | more `--mem`, fewer `--cpus` per task, or `--max-memory` lower |
| tasks of a resubmitted array take other rasters | the split changed: `terraflux-py slurm` warns; delete `.terraflux-batch/split.json` only to start over |
