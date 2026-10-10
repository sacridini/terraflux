#!/bin/bash
# Terraflux workers on a Slurm cluster: one worker per task, each with its cores.
#
#   sbatch --export=COORDINATOR=http://login-node:7700 tools/slurm/terraflux-workers.sbatch
#
# Start the coordinator first (on a node the workers can reach), e.g. on the login node:
#   terraflux serve --listen 0.0.0.0:7700
# then submit batches to it from anywhere that reaches it:
#   terraflux submit model.toml --replicates 100 --coordinator http://login-node:7700 --out results
#
#SBATCH --job-name=terraflux-workers
#SBATCH --ntasks=8
#SBATCH --cpus-per-task=8
#SBATCH --time=04:00:00

: "${COORDINATOR:?set COORDINATOR, e.g. --export=COORDINATOR=http://login-node:7700}"
# the data of the tasks, kept on each node's local disk
CACHE="${TMPDIR:-/tmp}/terraflux-cache-${SLURM_JOB_ID}"

export OMP_NUM_THREADS="${SLURM_CPUS_PER_TASK}"
srun --kill-on-bad-exit=0 bash -c \
  'terraflux worker --coordinator "$COORDINATOR" --name "$(hostname)-${SLURM_PROCID}" --cache "'"$CACHE"'-${SLURM_PROCID}"'
