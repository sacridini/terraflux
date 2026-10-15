---
title: Parallel and distributed computing
short: Overview
description: What parallel computing is, in plain words, what parts of a landscape analysis can be split, how much faster it gets, and how Terraflux uses the cores of your computer, the computers of a lab, a university cluster and machines rented in the cloud.
order: 0
---

A land change model that runs in a second is a pleasure; the same model run five hundred times for its
uncertainty, or once over a whole country at 30 m, is a week of waiting. This guide is for an ecologist who has
never used a cluster and does not want to become a computer scientist: it explains, from the beginning, what
"parallel" means, which parts of an analysis can be split, how much faster they get, and how Terraflux spreads
them over the cores of your computer, the computers of your lab, a university cluster or machines rented by the
hour. Every number in it was measured on real runs of Terraflux.

![From a core to the cloud](/img/guide/parallel/scales.png "The scales of parallel computing: a core does one thing at a time; a computer has several cores sharing its memory; the computers of a lab are joined by a network; a cluster has hundreds of computers shared by many people; the cloud rents machines by the hour.")

## The idea

### Cores, threads, processes, machines

A few words, defined once:

- A **core** is the part of a processor that executes instructions: it does one thing at a time. A laptop of today
  has 8 to 16; a desktop computer up to 32; a server 64 or more.
- A **thread** is a sequence of instructions a program runs; a program with several threads can keep several cores
  busy at once. Many processors run two threads per core ("hyper-threading"), so the system counts twice as many
  *logical* processors as cores.
- A **process** is a running program, with its own memory. Two processes do not share memory; they exchange data by
  files or messages.
- A **machine** (a computer, or a **node** of a cluster) has its cores and its memory. Machines exchange data over a
  network, much more slowly than the cores of one machine share their memory.
- **Memory** (RAM) is where the maps being computed are held. When a map does not fit, it must be read from disk
  in pieces.

Parallel computing is splitting a job into parts that run at the same time: on several cores of one machine
(threads), or on several machines (distributed computing).

### What can be split

Three kinds of work split easily, and Terraflux splits all three:

![Four kinds of parallel work](/img/guide/parallel/kinds.png "Replicates (the same model with another seed each), scenarios (variants of the model, one run each), calibration (settings tried in ranges, each run scored) and tiles (one large map cut in pieces).")

1. **Many runs of a model.** Replicates (the same model with another random seed), scenarios (variants of the
   model), a calibration (settings tried one after the other). The runs do not depend on each other: a hundred
   runs on ten cores take about a tenth of the time. Computer scientists call this *embarrassingly parallel*; it is
   the easiest, most efficient kind, and the most common in ecology.
2. **The cells of one map.** Most operations on a map (a reclassification, a probability from weights, a moving
   window) compute each cell, or each block of cells, independently: the cores of one machine share the map and
   each takes a part. Terraflux's operators do this by themselves.
3. **The tiles of a map too large for memory.** The map is cut into tiles that are read, computed and written one
   group at a time, on the cores of one machine or on many machines
   ([Maps larger than memory](/parallel/maps-larger-than-memory/)).

What does not split: the steps of one simulation (year 5 needs year 4), and the parts of an algorithm that take
one decision after another (Dinamica EGO's Expander places a patch, then the next one, on the map the first left).

### How much faster: Amdahl's law in plain words

If a job takes 100 minutes and 90 of them can be split, then on 10 cores those 90 minutes become 9, but the 10
minutes that cannot be split stay 10: 19 minutes in all, about 5 times faster, not 10. On 100 cores: 10.9 minutes,
9 times faster. However many cores you add, the job never goes below the 10 minutes that run one after the other.
This is **Amdahl's law** (Amdahl, 1967):

!!! science "Key idea: Amdahl's law"
    With a share \(p\) of the work that can be split over \(n\) cores, the job is faster by

    \[ S(n) = \frac{1}{(1 - p) + p / n} \]

    and never by more than \(1 / (1 - p)\). With \(p = 0.9\): 5.3 times on 10 cores, 9.2 on 100, never 10.

![Amdahl's law](/img/guide/parallel/amdahl.png "How much faster a job gets with more cores, for several shares of work that can be split. With 95% splittable, 128 cores give 17 times, not 128.")

In practice there are further costs: the parts must be sent out and gathered (on a network, that is slow), the
cores compete for memory, and a machine running other programs lends fewer cores than it has. That is why the
measured speed-ups below fall short of the curves.

### What Terraflux does by default

Without anything set, every command of Terraflux uses **all the cores but one** (leaving one for you and the
system), never every core. The threads it uses are, in order of precedence:

1. `--threads N` on the command line (any command: `run`, `submit`, `serve`...);
2. else the environment variable `TERRAFLUX_THREADS`;
3. else the threads chosen in the application's **Settings**;
4. else all the cores but one.

```bash
terraflux run model.toml --threads 4        # this run: 4 threads
export TERRAFLUX_THREADS=8                  # every command of this shell (Linux, macOS)
$env:TERRAFLUX_THREADS = 8                  # the same in Windows' PowerShell
```

The threads are shared by Terraflux's own tasks and by the C++ kernels of landschaft it calls (OpenMP), so the
total never exceeds what was asked. A batch of runs on one machine gives each of its workers an equal share.

### Measured: how much faster on one desktop computer

Three jobs on Sibuyan's land change model (Dinamica EGO's classic model, 14 steps), enlarged so that a run takes
seconds, on a desktop computer of 20 logical processors (an Intel Core i5-13600K: 6 performance cores with two
threads each, 8 efficiency cores) with the cores given from 1 to 19:

- one run of the exact engine (`terraflux run --threads N`, 432 × 512 cells);
- a batch of 24 replicates of it (`terraflux submit --replicates 24 --workers N`);
- one run of the tiled engine (`terraflux run --tiled --tile 256 --threads N`, 864 × 1,024 cells).

```tfchart
type: line
data: /data/guide/parallel/speedup.csv
x: cores
y: [ideal, batch, tiled, exact]
labels: { ideal: "Ideal (n cores, n times)", batch: "24 replicates (workers)", tiled: "Tiled engine (threads)", exact: "One run, exact engine (threads)" }
colors: { ideal: "#9aa39d", batch: "#3b8a5a", tiled: "#2f6f9f", exact: "#e8901f" }
xlabel: Cores given (workers or threads)
ylabel: Times faster than with one
height: 320
caption: The speed-up of three jobs with the cores given, measured (the shorter of two runs each). The batch of replicates gains most, up to 3.5 times with 8 workers; one run of the exact engine gains nothing.
```

| Cores | One run, exact (s) | 24 replicates (s) | Tiled run (s) |
|---:|---:|---:|---:|
| 1 | 3.9 | 79.1 | 35.3 |
| 2 | 3.8 | 43.9 | 19.9 |
| 4 | 3.9 | 30.9 | 12.3 |
| 8 | 3.8 | 22.8 | 11.1 |
| 12 | 3.6 | 24.7 | 13.1 |
| 19 | 3.6 | 24.9 | 11.9 |

Read it as an ecologist reads an experiment:

- **One run of the exact engine does not get faster.** Its time goes into Dinamica EGO's allocation, which places
  patches one after another on one map: the part that cannot be split dominates (\(p\) near 0). More cores do not
  help one such run; they help **many** runs.
- **Replicates gain most**, 1.8 times with 2 workers, 3.5 times with 8, then nothing more. Beyond 8, the runs
  compete for the memory and the efficiency cores are slower; other programs were running on the computer during
  the measurements too. On a server of identical cores the curve keeps rising further.
- **The tiled engine** gains 2.9 times with 4 threads and little after: its passes over the tiles are parallel, its
  decisions between passes are not.

The lesson for planning: on a desktop computer, run 4 to 8 runs at once; for more, add machines, not threads.

### Why an ecologist needs it

Most of what makes a landscape analysis credible is repetition, and repetition is what parallel computing makes
cheap:

- **Stochastic models** give a different map with every seed: a land change simulation, a neutral landscape, an
  agent-based model. One run is an anecdote; fifty runs give the probability of each outcome.
- **Uncertainty** in the inputs (a driver measured with error, a carbon stock known within a range) is propagated by
  running the model many times with inputs drawn from their distributions (Monte Carlo).
- **Sensitivity**: which settings change the conclusions? A grid of settings, each with replicates, quickly makes
  hundreds of runs.
- **Calibration** tries many settings against an observed map, each with a few seeds.
- **Scenarios** multiply everything above by the number of futures compared.

A study with 5 scenarios × 50 replicates × a model of 30 seconds is 7,500 seconds on one core, about two hours; with
a sensitivity analysis of 20 settings on top, it is two days. On 8 cores it is an afternoon; on a cluster, an hour.

### Measure before you buy

Before planning a large batch, time what you have: run the model once with `terraflux run` and note the seconds;
watch the memory it takes in the system's task manager (Windows), Activity Monitor (macOS) or `top` (Linux); then
run a batch of 8 replicates with 1, 2 and 4 workers. Those three numbers (the time of one run, the memory of one
run, and how the batch scales on your computer) tell you how many workers your computer takes and whether you need
more computers. The measurements above are an example.

## When to use it

| Your job | Where to run it | Page |
|---|---|---|
| A model that runs in seconds, once | as it is: all the cores but one | this page |
| Tens of replicates, a few scenarios, a small calibration | your computer, a batch | [Many runs on your computer](/parallel/many-runs/) |
| Hundreds of runs, and a few computers in the lab or at home | the computers of the network, with a code | [The lab's computers](/parallel/lab-computers/) |
| Thousands of runs, or runs of hours each | a university cluster (Slurm) | [A cluster with Slurm](/parallel/slurm-cluster/) |
| A cluster you do not have, for a few days | machines rented in the cloud | [The cloud](/parallel/cloud/) |
| One map larger than memory (a country at 30 m) | the tiled engine, one machine or many | [Maps larger than memory](/parallel/maps-larger-than-memory/) |
| Something goes wrong | | [Troubleshooting](/parallel/troubleshooting/) |

The same commands work everywhere: `terraflux submit` and `terraflux calibrate` run here, on a coordinator
(`--coordinator`, `--code`) or on a saved cluster (`--on`), and a model gives the same maps on any worker as on
your computer.

## What you need

Nothing more than Terraflux for one computer. For several computers, the same version of Terraflux on each (a
coordinator turns away workers of another version), a network between them, and, for a cluster or the cloud, an
account and SSH. The pages that follow say what each case needs.

## How it works, in one picture

![A coordinator and its workers](/img/guide/parallel/coordinator.png "Your computer submits a batch to a coordinator; workers on any number of machines ask it for tasks, fetch the files they lack (once, by hash), run, send back a heartbeat and the results.")

Every case in this guide uses one design. A **coordinator** (`terraflux serve`) keeps a queue of tasks. **Workers**
(`terraflux worker`) on any number of machines ask it for a task, fetch the data they lack, run it, and send the
results back. A **client** (`terraflux submit`, `calibrate`, or the application) submits a batch and fetches its
results. On one computer the three live in one process; across machines they talk over the network. Workers only
ever connect *to* the coordinator, so they need no open port: they can sit behind firewalls.

## Common pitfalls

- **Expecting one run to get faster with more cores.** Many runs do; one patch-based simulation hardly does.
- **Giving every core.** The computer becomes unusable and the gain stops well before; leave the default.
- **Too many runs at once for the memory.** Each run holds its maps; 8 runs of a model that needs 4 GB need 32 GB.
- **Different versions of Terraflux** on different machines: workers of another version are refused (the same
  engine gives the same maps).

## Going further

- [Many runs on your computer](/parallel/many-runs/), the next page.
- [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/): what replicates and calibrations
  are for, in a land change study.
- The command reference: `terraflux submit`, `calibrate`, `serve`, `worker` ([CLI](/cli/)).

## References

- Amdahl, G. M. (1967). Validity of the single processor approach to achieving large scale computing capabilities. In *Proceedings of the April 18-20, 1967, Spring Joint Computer Conference (AFIPS '67)* (pp. 483–485). ACM. https://doi.org/10.1145/1465482.1465560
