//! The client of a coordinator: it checks the coordinator is of its version, sends the files of a model (once,
//! by hash), submits jobs, follows them, reads their results and fetches their files.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use terraflux_graph::{Input, Model};

use crate::evaluate::Evaluation;
use crate::proto::*;
use crate::{Error, Result, bundle};

pub struct Client {
    c: crate::net::Conn,
}

/// A task of a job: its label and its overrides of the model.
pub type TaskOf = (String, BTreeMap<String, serde_json::Value>);

/// The operators whose `path` a model reads (sent with it).
pub const READ_OPS: [&str; 3] = [terraflux_graph::PLUGIN, "raster.read", "vector.read"];

impl Client {
    /// Connect to a coordinator of this version of Terraflux (plain).
    pub async fn connect(url: &str) -> Result<Client> {
        Client::connect_with(url, None).await
    }

    /// Connect to a coordinator of this version of Terraflux, with its pairing code if it has one.
    pub async fn connect_with(url: &str, code: Option<&crate::secure::Code>) -> Result<Client> {
        let mut c = crate::net::connect(url, code).await?;
        let theirs = c.hello(Empty {}).await?.into_inner().version;
        if theirs != crate::coordinator::VERSION {
            return Err(Error::Other(format!(
                "the coordinator at {url} is Terraflux {theirs}, this is {}: use the same version on every machine",
                crate::coordinator::VERSION
            )));
        }
        Ok(Client { c })
    }

    /// Send a file, the files beside it of the same name, a folder's or a pattern's files (relative to `base`):
    /// `bundle:<hash>/...`.
    pub async fn send(&mut self, path: &str, base: &Path) -> Result<String> {
        bundle::send(&mut self.c, path, base).await
    }

    /// Send a file as a blob (unless the coordinator has it): its hash.
    pub async fn put_file(&mut self, path: &Path) -> Result<String> {
        bundle::send_file(&mut self.c, path).await
    }

    /// Send a file: `bundle:<hash>/<its name>`.
    pub async fn upload(&mut self, path: &Path) -> Result<String> {
        self.send(&path.to_string_lossy(), Path::new("")).await
    }

    /// The model with its files sent (its inputs' and those its nodes read, relative to `base`): their paths
    /// become `bundle:<hash>/...`.
    pub async fn bundle(&mut self, model: &Model, base: &Path) -> Result<Model> {
        let mut m = model.clone();
        for input in m.inputs.values_mut() {
            if let Input::Map { path, .. } | Input::Stack { path, .. } = input {
                *path = self.send(path, base).await?;
            }
        }
        let mut paths = Vec::new();
        read_paths(&mut m.nodes, &mut |p| paths.push(p.clone()));
        let mut sent = BTreeMap::new();
        for p in paths {
            if let std::collections::btree_map::Entry::Vacant(e) = sent.entry(p) {
                let b = self.send(e.key(), base).await?;
                e.insert(b);
            }
        }
        read_paths(&mut m.nodes, &mut |p| {
            if let Some(b) = sent.get(p.as_str()) {
                *p = b.clone();
            }
        });
        Ok(m)
    }

    /// The tasks with the files their overrides name sent: the paths given to the model's file inputs
    /// (`$drivers = "drivers_2030.tif"`) and to the nodes that read files (`roads.path`).
    pub async fn bundle_tasks(&mut self, model: &Model, base: &Path, tasks: &[TaskOf]) -> Result<Vec<TaskOf>> {
        let mut out = Vec::with_capacity(tasks.len());
        for (label, o) in tasks {
            let mut o = o.clone();
            for (key, value) in o.iter_mut() {
                let serde_json::Value::String(path) = value else { continue };
                let file = match key.strip_prefix('$') {
                    Some(name) => matches!(model.inputs.get(name), Some(Input::Map { .. } | Input::Stack { .. })),
                    None => key.rsplit_once('.').is_some_and(|(node, setting)| {
                        setting == "path" && node_at(model, node).is_some_and(|n| READ_OPS.contains(&n.op.as_str()))
                    }),
                };
                if file {
                    *path = self.send(path, base).await?;
                }
            }
            out.push((label.clone(), o));
        }
        Ok(out)
    }

    pub async fn submit(
        &mut self,
        name: &str,
        model: &Model,
        tasks: &[TaskOf],
        return_files: bool,
        evaluation: Option<&Evaluation>,
    ) -> Result<String> {
        let spec = JobSpec {
            name: name.to_string(),
            model_json: model.to_json().map_err(|e| Error::Other(e.to_string()))?,
            tasks: tasks
                .iter()
                .map(|(label, o)| {
                    Ok(TaskSpec {
                        label: label.clone(),
                        overrides_json: serde_json::to_string(o)?,
                        payload: Vec::new(),
                    })
                })
                .collect::<Result<_>>()?,
            return_files,
            evaluation_json: match evaluation {
                Some(e) => serde_json::to_string(e)?,
                None => String::new(),
            },
            kind: String::new(),
            payload: Vec::new(),
        };
        Ok(self.c.submit(spec).await?.into_inner().id)
    }

    /// Submit a job of kind `kind`: its payload and its tasks' (labelled by their number).
    pub async fn submit_payload(&mut self, kind: &str, payload: Vec<u8>, tasks: Vec<Vec<u8>>) -> Result<String> {
        let spec = JobSpec {
            name: kind.to_string(),
            kind: kind.to_string(),
            payload,
            tasks: tasks
                .into_iter()
                .enumerate()
                .map(|(i, p)| TaskSpec { label: i.to_string(), overrides_json: String::new(), payload: p })
                .collect(),
            ..Default::default()
        };
        Ok(self.c.submit(spec).await?.into_inner().id)
    }

    /// Wait (on the coordinator) until every task of a job is done or failed.
    pub async fn wait_done(&mut self, job: &str) -> Result<JobStatus> {
        loop {
            let s = self.c.wait(WaitRequest { id: job.to_string(), wait_ms: 30_000 }).await?.into_inner();
            if s.done + s.failed == s.total {
                return Ok(s);
            }
        }
    }

    /// Drop a job, its results and their files from the coordinator.
    pub async fn forget(&mut self, job: &str) -> Result<()> {
        self.c.forget(JobId { id: job.to_string() }).await?;
        Ok(())
    }

    pub async fn status(&mut self, job: &str) -> Result<JobStatus> {
        Ok(self.c.status(JobId { id: job.to_string() }).await?.into_inner())
    }

    /// Wait until every task of a job is done or failed, telling `progress` along the way.
    pub async fn wait(&mut self, job: &str, mut progress: impl FnMut(&JobStatus)) -> Result<JobStatus> {
        let mut last = None;
        loop {
            let s = self.status(job).await?;
            if last.as_ref() != Some(&(s.done, s.failed, s.running)) {
                progress(&s);
                last = Some((s.done, s.failed, s.running));
            }
            if s.done + s.failed == s.total {
                return Ok(s);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// The results of a job's tasks, in the order of the tasks (their files on the coordinator, by hash).
    pub async fn results(&mut self, job: &str) -> Result<Vec<TaskResult>> {
        let mut r = self.c.results(JobId { id: job.to_string() }).await?.into_inner().results;
        r.sort_by_key(|t| t.task_id.rsplit_once('/').and_then(|(_, i)| i.parse::<usize>().ok()).unwrap_or(0));
        Ok(r)
    }

    /// A file of a result, written to `dest`.
    pub async fn download(&mut self, hash: &str, dest: &Path) -> Result<()> {
        bundle::fetch_to(&mut self.c, hash, dest).await
    }

    /// A file of a result, in memory.
    pub async fn fetch(&mut self, hash: &str) -> Result<Vec<u8>> {
        let mut s = self.c.get_blob(BlobId { hash: hash.to_string() }).await?.into_inner();
        let mut out = Vec::new();
        while let Some(c) = s.message().await? {
            out.extend_from_slice(&c.data);
        }
        if crate::hash(&out) != hash {
            return Err(Error::Other(format!("the blob {hash} arrived damaged")));
        }
        Ok(out)
    }

    /// The jobs the coordinator has, done or not.
    pub async fn jobs(&mut self) -> Result<Vec<JobStatus>> {
        Ok(self.c.jobs(Empty {}).await?.into_inner().jobs)
    }

    pub async fn workers(&mut self) -> Result<Vec<WorkerState>> {
        Ok(self.c.workers(Empty {}).await?.into_inner().workers)
    }
}

/// The node of a path of ids (`simulation/allocate`: a node of a container's body).
fn node_at<'a>(m: &'a Model, path: &str) -> Option<&'a terraflux_graph::Node> {
    let mut parts = path.split('/');
    let first = parts.next()?;
    let mut node = m.nodes.iter().find(|n| n.id == first)?;
    for id in parts {
        node = node.body.as_ref()?.nodes.iter().find(|n| n.id == id)?;
    }
    Some(node)
}

/// Every path a model's nodes read (plugins, rasters, vectors; their bodies' included).
pub fn read_paths(nodes: &mut [terraflux_graph::Node], f: &mut dyn FnMut(&mut String)) {
    for n in nodes {
        if READ_OPS.contains(&n.op.as_str())
            && let Some(serde_json::Value::String(p)) = n.params.get_mut("path")
        {
            f(p);
        }
        if let Some(b) = &mut n.body {
            read_paths(&mut b.nodes, f);
        }
    }
}
