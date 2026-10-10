//! The client of a coordinator: it sends the data of a model (once, by hash), submits jobs, follows them and
//! reads their results.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use terraflux_graph::{Input, Model};
use tonic::transport::Channel;

use crate::evaluate::Evaluation;
use crate::proto::coordinator_client::CoordinatorClient;
use crate::proto::*;
use crate::{Error, Result};

pub struct Client {
    c: CoordinatorClient<Channel>,
}

/// A task of a job: its label and its overrides of the model.
pub type TaskOf = (String, BTreeMap<String, serde_json::Value>);

impl Client {
    pub async fn connect(url: &str) -> Result<Client> {
        Ok(Client { c: crate::worker::connect(url).await? })
    }

    /// Send a file (unless the coordinator has it): `blob:<hash>`.
    pub async fn upload(&mut self, path: &Path) -> Result<String> {
        let data = std::fs::read(path).map_err(|e| Error::Other(format!("{}: {e}", path.display())))?;
        let hash = crate::hash(&data);
        if !self.c.has_blob(BlobId { hash: hash.clone() }).await?.into_inner().ok {
            self.c.put_blob(Blob { hash: hash.clone(), data }).await?;
        }
        Ok(format!("blob:{hash}"))
    }

    /// The model with its files sent: their paths (relative to `base`) become `blob:<hash>`.
    pub async fn bundle(&mut self, model: &Model, base: &Path) -> Result<Model> {
        let mut m = model.clone();
        for input in m.inputs.values_mut() {
            if let Input::Map { path, .. } | Input::Stack { path, .. } = input
                && !path.starts_with("blob:")
            {
                let p = Path::new(path.as_str());
                let full = if p.is_absolute() { p.to_path_buf() } else { base.join(p) };
                *path = self.upload(&full).await?;
            }
        }
        // the plugins' modules
        let mut paths = Vec::new();
        plugin_paths(&mut m.nodes, &mut |p| paths.push(p.clone()));
        let mut sent = std::collections::BTreeMap::new();
        for p in paths {
            if !p.starts_with("blob:") && !sent.contains_key(&p) {
                let full = if Path::new(&p).is_absolute() { Path::new(&p).to_path_buf() } else { base.join(&p) };
                sent.insert(p.clone(), self.upload(&full).await?);
            }
        }
        plugin_paths(&mut m.nodes, &mut |p| {
            if let Some(b) = sent.get(p.as_str()) {
                *p = b.clone();
            }
        });
        Ok(m)
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

    /// Drop a job and its results from the coordinator.
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

    /// The results of a job's tasks, in the order of the tasks.
    pub async fn results(&mut self, job: &str) -> Result<Vec<TaskResult>> {
        let mut r = self.c.results(JobId { id: job.to_string() }).await?.into_inner().results;
        r.sort_by_key(|t| t.task_id.rsplit_once('/').and_then(|(_, i)| i.parse::<usize>().ok()).unwrap_or(0));
        Ok(r)
    }

    pub async fn workers(&mut self) -> Result<Vec<WorkerState>> {
        Ok(self.c.workers(Empty {}).await?.into_inner().workers)
    }
}

/// Every plugin's path in some nodes (their bodies' included).
pub fn plugin_paths(nodes: &mut [terraflux_graph::Node], f: &mut dyn FnMut(&mut String)) {
    for n in nodes {
        if n.op == terraflux_graph::PLUGIN
            && let Some(serde_json::Value::String(p)) = n.params.get_mut("path")
        {
            f(p);
        }
        if let Some(b) = &mut n.body {
            plugin_paths(&mut b.nodes, f);
        }
    }
}
