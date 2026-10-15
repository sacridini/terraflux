---
title: Machines rented in the cloud
short: The cloud
description: Rent machines from a cloud provider for a few hours, set them up with the script terraflux cloud-init writes, run batches on them from your computer, and switch them off - with the firewall, costs and data to keep in mind.
order: 40
---

When the lab has no spare computers and the university no cluster, or the deadline is next week, machines can be
rented by the hour from a cloud provider (Amazon Web Services, Google Cloud, Microsoft Azure, DigitalOcean,
Hetzner and many others). A machine of 32 cores costs about as much per hour as a coffee, and twenty of them can
run in an afternoon what a desktop computer runs in a month. This page shows how to set such machines up for
Terraflux with one generated script, run batches on them with the commands of the [cluster
page](/parallel/slurm-cluster/), and switch them off. It assumes no experience with the cloud.

## The idea

![A server and its workers in the cloud](/img/guide/parallel/cloud.png "Your computer reaches the server by SSH; the server runs the coordinator (and workers) for each batch; worker machines on the provider's private network keep a worker running for it. The firewall lets SSH in from your address only, and the coordinator's port only from the private network.")

A cloud machine is a computer you create in a web console (or with the provider's command line): you choose its
size (cores and memory), its operating system (Ubuntu 24.04 here), its network, and you can give it a
**user data** script that runs the first time it starts, written for **cloud-init**, the tool nearly every
provider uses to set machines up. `terraflux cloud-init` writes that script:

- for a **server**: Terraflux installed in `/opt/terraflux`, the `terraflux` command linked in `/usr/local/bin`.
  You then save it as a server (`terraflux cluster add cloud --host ubuntu@<address> --workers 16`) and send
  batches to it with `--on cloud`, exactly as to a cluster: for each batch, Terraflux starts its coordinator and
  workers there by SSH and stops them after.
- for a **worker machine** (`--worker`): Terraflux installed, and a service, `terraflux-worker`, that keeps a worker
  running for the server's coordinator, started again if it stops and when the machine restarts. The service runs
  as an unprivileged user that systemd makes for it (`DynamicUser=yes`), which can write only its cache
  (`/var/cache/terraflux`, made by systemd: `CacheDirectory=terraflux`), not as the administrator (root).

```bash
terraflux cloud-init > server.yaml
terraflux cloud-init --worker --on cloud --coordinator 10.0.0.5 > worker.yaml
```

The server's script, as written by Terraflux 0.80.0:

```yaml
#cloud-config
# Terraflux: paste as the machine's "user data" (cloud-init) when it is created.
package_update: true
packages: [curl, ca-certificates]
runcmd:
  - mkdir -p /opt
  - curl -fsSL https://github.com/sacridini/terraflux/releases/download/v0.80.0/terraflux-0.80.0-linux-x64.tar.gz -o /tmp/terraflux.tar.gz
  - tar -xzf /tmp/terraflux.tar.gz -C /opt
  - rm -rf /opt/terraflux && mv /opt/terraflux-*-linux-* /opt/terraflux
  - ln -sf /opt/terraflux/terraflux /usr/local/bin/terraflux
```

A worker's adds the service (here with `--coordinator 10.0.0.5:7700 --code K7QM-2XPA-9DTE --threads 8`):

```yaml
write_files:
  - path: /etc/systemd/system/terraflux-worker.service
    content: |
      [Unit]
      Description=Terraflux worker
      After=network-online.target
      Wants=network-online.target
      [Service]
      ExecStart=/usr/local/bin/terraflux worker --coordinator 10.0.0.5:7700 --code K7QM-2XPA-9DTE --cache /var/cache/terraflux --name %H --threads 8
      DynamicUser=yes
      CacheDirectory=terraflux
      Restart=always
      RestartSec=10
      Nice=5
      [Install]
      WantedBy=multi-user.target
runcmd:
  - ...                               # the same installation as the server's
  - systemctl daemon-reload
  - systemctl enable --now terraflux-worker
```

`--on cloud` takes the server's port and code from the saved profile, and `--coordinator` its address on the
provider's **private network** (10.0.0.5 here): the machines talk to each other inside the provider's network, and
the coordinator's port need not be open to the internet.

### The words of the providers

Each provider names the same things differently; the ones that matter here:

| Thing | AWS | Google Cloud | Azure | Others |
|---|---|---|---|---|
| a machine | EC2 instance | Compute Engine VM | virtual machine | droplet, server, VPS |
| its size | instance type | machine type | VM size | plan |
| the system it starts with | AMI | image | image | image |
| the setup script | user data | startup script / user data (cloud-init) | custom data | user data |
| the private network | VPC, subnet | VPC network | virtual network | private network |
| the firewall | security group | firewall rules | network security group | firewall |
| where it is | region, zone | region, zone | region | location |

Whatever the names, the steps below are the same: choose an Ubuntu image, a size, a region near you (or near your
data), a private network, your SSH key and the user data, then open the firewall for SSH from your address.

### How big, how many

A batch is fastest per unit of money when the machines are busy and none waits. Two rules help:

- **Many medium machines rather than one huge one**, for batches of independent runs: a run of a patch-based model
  hardly uses more than one thread ([overview](/parallel/#measured-how-much-faster-on-one-desktop-computer)), so
  several workers per machine and several machines scale better than one machine of 128 cores, and a lost machine
  costs less. The server can run workers too (`--workers`).
- **Enough memory per worker**: a worker holds its model's maps; with 8 workers of a model that needs 3 GB, a
  machine of 32 GB leaves room for the system.

A worked plan: a model of 40 seconds a run, 2,000 replicates. On one machine of 16 cores with 8 workers: 2,000 ×
40 / 8 = 10,000 seconds, about 2.8 hours. On the server and four worker machines of the same size (40 workers):
about 35 minutes, plus a few minutes for the machines to start and install. Five machines for an hour cost about
five machine-hours: look up the hourly price of the size in your provider's list and multiply. Time a few runs on
the first machine before creating the others.

## When to use it

- A batch too large for your computers, once or for a few weeks, without a cluster.
- A deadline: twenty machines for an afternoon.
- A collaborator elsewhere: a server both can submit to (with its code).

It costs money by the hour, so it suits bursts of work; for steady, long use a cluster or a lab server is cheaper.

## What you need

- An **account** with a provider, and a payment method or research credits (most providers offer them to
  universities).
- An **SSH key** (as for [a cluster](/parallel/slurm-cluster/#ssh-keys-once)), given to the provider when the
  machine is created.
- Terraflux's Linux package reachable from the machines: by default the release of your version on GitHub;
  `--package <url>` points to a copy you host (a bucket of the provider, for example).

## Run it

<div class="steps" markdown>

1. **Write the server's script**: `terraflux cloud-init > server.yaml` (or, in the application, the Computers
   pane's **Copy the server's script**).
2. **Create the server** in the provider's console: Ubuntu 24.04, a size (16 cores and 32 GB is a good start), your
   SSH key, a **private network**, and paste `server.yaml` into the user data field. Note its public address
   (203.0.113.7) and its private one (10.0.0.5).
3. **Open the firewall** for SSH (port 22) from your address only.
4. **Save and test it**: `terraflux cluster add cloud --host ubuntu@203.0.113.7 --workers 16`, then
   `terraflux cluster test cloud` (a few minutes after creation: cloud-init needs time to install).
5. **Run a batch**: `terraflux submit model.toml --replicates 1000 --on cloud --out replicates`.
6. **More machines** (optional): `terraflux cloud-init --worker --on cloud --coordinator 10.0.0.5 > worker.yaml`
   (or **Copy a worker's script**), create machines in the same private network with it as user data, and allow
   the coordinator's port from the private network in the firewall (`terraflux cluster list` shows the profiles; the
   port is in `clusters.toml`). They join the server's batches by themselves.
7. **Fetch everything, then delete the machines.** A machine that is stopped may still be billed for its disk; a
   deleted one is not.

</div>

## Read the results

As for any batch: the files come back to `--out` on your computer, with `tasks.csv`. In `tasks.csv` the workers of
the worker machines are named after their host names (`--name %H`), and the server's workers after the server.
Read the `seconds` column to check that the machines are as fast as you planned: a cloud core is often slower than a
recent desktop core, and a machine of a "shared" (burstable) size slows down after a while of full use. If the
seconds per run of the worker machines are much higher than on your computer, choose another size for the next
batch.

### Keeping the results

The results are on your computer as soon as each run is done; nothing needs to be copied off the machines, and
they can be deleted when the batch ends. A batch left to run (`--detach`) keeps its files on the server's disk
until they are fetched with `terraflux results <job> --on cloud`: fetch them before deleting the server.

### Doing it again

Keep `server.yaml` and `worker.yaml` with the project: the next time, creating the machines takes minutes. The
profile saved by `terraflux cluster add` keeps its code and port; if the server gets a new address, add the profile
again (`terraflux cluster remove cloud`, then `add`: a new profile has a new code and port, so write
`worker.yaml` again too). Regenerate the scripts after upgrading Terraflux on your
computer, since the machines must run the same version.

## Costs and good practice

- **Estimate first**: time a run on your computer, and work out machines × hours. Providers' price lists give the
  cost per hour of each size; storage and data leaving the provider are billed too.
- **Switch off** what you do not use. Set a budget alert in the provider's console.
- **Keep the coordinator's port private**: open it to the private network only; the code adds encryption and
  authentication, but there is no reason to expose the port to the world.
- **Use the same version** of Terraflux everywhere: the scripts install the version that wrote them; regenerate them
  after an upgrade.
- **Keep your data's rules**: some data (species records, private land) may not leave your institution or country;
  choose the provider's region accordingly, or do not use the cloud for them.

## Common pitfalls

- **The release is not reachable**: if your version's package is not published (or the machines have no internet),
  the installation fails; give `--package` a URL the machines can reach. Look in `/var/log/cloud-init-output.log` on
  the machine.
- **Workers that never join**: the firewall does not allow the coordinator's port on the private network, or the
  machines are in another network or region.
- **`cluster test` fails right after creation**: cloud-init has not finished; wait a few minutes.
- **A worker machine used for other things**: the worker service runs as an unprivileged user and writes only its
  cache, but it takes the threads it is given whenever the coordinator has tasks; rent machines for the batch rather
  than lending one that serves something else.

## Going further

- [Maps larger than memory](/parallel/maps-larger-than-memory/): a large map on many machines.
- [Troubleshooting](/parallel/troubleshooting/).

```tfcli
cloud-init
```
