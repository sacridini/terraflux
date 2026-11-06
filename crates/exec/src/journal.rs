//! The journal of a model's runs: each run asked for recorded beside the model (`.terraflux/<model file>/`), with
//! when it ran, how long, how it ended, the values it ran with, the outputs it wrote and the model as it was. The
//! application, `terraflux run` and the MCP write it; the application's Runs pane reads it.
//!
//! `runs.jsonl` holds an [`Entry`] a line, appended (a line that does not read is skipped); `models/<sha256>.json`
//! the model as it ran, once for each hash. The outputs are not copied, only recorded: an output written over
//! since is told by [`Written::unchanged`].

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use terraflux_graph::{Input, Model};

use crate::{Error, Result};

/// The folder the journals are kept in, beside the model.
pub const FOLDER: &str = ".terraflux";

/// The folder a check run of a model writes its outputs to: `<model folder>/<model> runs/check`, replaced at each
/// check run, never the folder of the real outputs.
pub fn check_dir(model_file: &Path) -> PathBuf {
    let stem = model_file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "model".into());
    let dir = model_file.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    dir.join(format!("{stem} runs")).join("check")
}

/// How a run ended.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Done,
    Stopped,
    /// With its message.
    Failed(String),
}

/// What was run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// The whole model.
    #[default]
    Run,
    /// Up to a node (its place).
    To(String),
    /// Many runs.
    Batch(Batch),
    /// A check run: the whole model on a part of the map, or at a coarser resolution, its outputs apart from the real
    /// ones.
    Check(terraflux_ops::check::Check),
}

/// A batch of Many runs: one entry, with its runs counted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    /// Replicates, scenarios, calibration or sensitivity.
    #[serde(default)]
    pub kind: String,
    /// The folder of its results.
    #[serde(default)]
    pub out: PathBuf,
    #[serde(default)]
    pub runs: usize,
    #[serde(default)]
    pub failed: usize,
    /// A sensitivity analysis: its method and its factors, the most important first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub factors: Vec<Factor>,
}

/// A factor of a sensitivity analysis with its main index (the swing, mu* or ST).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Factor {
    pub key: String,
    /// The index's name.
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub index: Option<f64>,
}

/// An output a run wrote.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Written {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub size: u64,
    /// When the file was last written (milliseconds since 1970).
    #[serde(default)]
    pub modified: u64,
    /// Its SHA-256, when the run's manifest has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

fn stat(path: &Path) -> Option<(u64, u64, bool)> {
    let m = std::fs::metadata(path).ok()?;
    let modified = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64;
    Some((m.len(), modified, m.is_dir()))
}

impl Written {
    /// A file (or folder) written, as it is now; none when it is not there. `size` is the file's when not given
    /// (the manifest's count of a folder's bytes, say).
    pub fn of(name: &str, path: &Path, sha256: Option<String>, size: Option<u64>) -> Option<Written> {
        let (len, modified, _) = stat(path)?;
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        Some(Written {
            name: name.to_string(),
            path: path.to_string_lossy().into_owned(),
            size: size.unwrap_or(len),
            modified,
            sha256,
        })
    }

    /// Whether the file is there with the size and the time it was written at (a folder: the time only).
    pub fn unchanged(&self) -> bool {
        match stat(Path::new(&self.path)) {
            Some((len, modified, dir)) => modified == self.modified && (dir || len == self.size),
            None => false,
        }
    }
}

/// A run in the journal. Every field but the first has a default: the format can grow.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Unique: the start time in milliseconds (made one more when another run began in the same one).
    pub id: u64,
    /// When it began (seconds since 1970).
    #[serde(default)]
    pub when: u64,
    #[serde(default)]
    pub seconds: f64,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    /// The values the model ran with: its parameters (`node.setting`, `$name`) and the inputs of values.
    #[serde(default)]
    pub values: BTreeMap<String, serde_json::Value>,
    /// The SHA-256 of the model as it ran ([`Journal::model`] has it).
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub outputs: Vec<Written>,
    /// The manifest of the run, when it wrote one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// The version of Terraflux.
    #[serde(default)]
    pub terraflux: String,
    /// Where it was asked for: `app`, `cli` or `mcp`.
    #[serde(default)]
    pub from: String,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

impl Entry {
    /// An entry for a run that has just ended after `seconds`, asked for from `from`.
    pub fn new(kind: Kind, from: &str, seconds: f64) -> Entry {
        let id = now_ms().saturating_sub((seconds * 1000.0) as u64);
        Entry {
            id,
            when: id / 1000,
            seconds,
            kind,
            terraflux: env!("CARGO_PKG_VERSION").to_string(),
            from: from.to_string(),
            ..Default::default()
        }
    }

    /// The outputs the manifest of a run lists, as they are now (the hash and the size from the manifest).
    pub fn with_manifest(mut self, manifest: &crate::testing::Manifest, file: &Path) -> Entry {
        self.outputs = manifest
            .outputs
            .iter()
            .filter_map(|(name, h)| Written::of(name, Path::new(&h.path), Some(h.sha256.clone()), Some(h.bytes)))
            .collect();
        self.scenario = self.scenario.or_else(|| manifest.scenario.clone());
        self.manifest = Some(file.to_path_buf());
        self
    }
}

/// Record a run of a model's file made outside the application (`terraflux run`, the MCP): the model kept is the file
/// as written (read again; `ran`, the model after its scenario and its settings, when it does not read), the values
/// those `ran` had.
pub fn record_run(model_file: &Path, ran: &Model, mut entry: Entry) -> Result<()> {
    let journal = Journal::of(model_file).ok_or_else(|| Error::Model("the model has no file".into()))?;
    let source = Model::load(model_file).unwrap_or_else(|_| ran.clone());
    entry.values = values_of(ran);
    journal.record(entry, &source)
}

/// The values a model runs with: its parameters' (`node.setting`, `$name`) and its inputs of values (`$name`).
pub fn values_of(model: &Model) -> BTreeMap<String, serde_json::Value> {
    let mut out = BTreeMap::new();
    for p in &model.parameters {
        if let Some(v) = model.setting(&p.key) {
            out.insert(p.key.clone(), v.clone());
        }
    }
    for (name, input) in &model.inputs {
        if let Input::Value { value, .. } = input {
            out.insert(format!("${name}"), value.clone());
        }
    }
    out
}

/// The SHA-256 of a text, in hex.
fn sha_of(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The hash a model is kept under in the journal ([`Entry::model`]): the SHA-256 of its JSON.
pub fn hash_of(model: &Model) -> Option<String> {
    model.to_json().ok().map(|t| sha_of(&t))
}

/// A model's journal.
#[derive(Debug, Clone, PartialEq)]
pub struct Journal {
    dir: PathBuf,
}

impl Journal {
    /// The journal of a model's file: `.terraflux/<file name>/` beside it (none for a path without a file name).
    pub fn of(model_file: &Path) -> Option<Journal> {
        let name = model_file.file_name()?;
        let parent = model_file.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        Some(Journal { dir: parent.join(FOLDER).join(name) })
    }

    /// The journal's folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn runs(&self) -> PathBuf {
        self.dir.join("runs.jsonl")
    }

    fn snapshot(&self, sha: &str) -> PathBuf {
        self.dir.join("models").join(format!("{sha}.json"))
    }

    /// Add a run: the model as it ran kept (once for each hash), the entry appended with its model's hash.
    pub fn record(&self, mut entry: Entry, model: &Model) -> Result<()> {
        let text = model.to_json().map_err(|e| Error::Model(e.to_string()))?;
        entry.model = sha_of(&text);
        let snap = self.snapshot(&entry.model);
        if !snap.exists() {
            std::fs::create_dir_all(self.dir.join("models"))?;
            std::fs::write(&snap, text)?;
        }
        // (an id another run has: one more)
        let known: Vec<u64> = self.entries().iter().map(|e| e.id).collect();
        while known.contains(&entry.id) {
            entry.id += 1;
        }
        let line = serde_json::to_string(&entry).map_err(|e| Error::Model(e.to_string()))?;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(self.runs())?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// The runs, the oldest first; a line that does not read is skipped.
    pub fn entries(&self) -> Vec<Entry> {
        let Ok(text) = std::fs::read_to_string(self.runs()) else { return Vec::new() };
        text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    /// The size of the journal's file (it only grows): to tell, cheaply, that another program wrote to it.
    pub fn size(&self) -> u64 {
        std::fs::metadata(self.runs()).map_or(0, |m| m.len())
    }

    /// The model as it ran, by its hash.
    pub fn model(&self, sha: &str) -> Option<Model> {
        let text = std::fs::read_to_string(self.snapshot(sha)).ok()?;
        Model::from_json(&text).ok()
    }

    fn versions_file(&self) -> PathBuf {
        self.dir.join("versions.jsonl")
    }

    /// Save a version: the model as it is now kept (once for each hash, beside the runs' snapshots) under a label.
    /// A model equal to the newest version's is saved again: a version is a named point in time.
    pub fn save_version(&self, label: &str, note: Option<&str>, model: &Model, from: &str) -> Result<Version> {
        let label = label.trim();
        if label.is_empty() {
            return Err(Error::Model("a version needs a label".into()));
        }
        let text = model.to_json().map_err(|e| Error::Model(e.to_string()))?;
        let sha = sha_of(&text);
        let snap = self.snapshot(&sha);
        if !snap.exists() {
            std::fs::create_dir_all(self.dir.join("models"))?;
            std::fs::write(&snap, text)?;
        }
        let mut id = now_ms();
        let known: Vec<u64> = self.versions().iter().map(|v| v.id).collect();
        while known.contains(&id) {
            id += 1;
        }
        let note = note.map(str::trim).filter(|n| !n.is_empty()).map(String::from);
        let v = Version { id, when: id / 1000, label: label.to_string(), model: sha, note, from: from.to_string() };
        let line = serde_json::to_string(&v).map_err(|e| Error::Model(e.to_string()))?;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(self.versions_file())?;
        writeln!(file, "{line}")?;
        Ok(v)
    }

    /// The versions saved, the oldest first; a line that does not read is skipped.
    pub fn versions(&self) -> Vec<Version> {
        let Ok(text) = std::fs::read_to_string(self.versions_file()) else { return Vec::new() };
        text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    /// The size of the versions' file: to tell, cheaply, that another program wrote to it.
    pub fn versions_size(&self) -> u64 {
        std::fs::metadata(self.versions_file()).map_or(0, |m| m.len())
    }

    /// Delete a version. Its snapshot goes with it unless a run or another version has that model.
    pub fn delete_version(&self, id: u64) -> Result<()> {
        let all = self.versions();
        let Some(gone) = all.iter().find(|v| v.id == id) else { return Ok(()) };
        let sha = gone.model.clone();
        let rest: Vec<&Version> = all.iter().filter(|v| v.id != id).collect();
        let mut text = String::new();
        for v in &rest {
            text.push_str(&serde_json::to_string(v).map_err(|e| Error::Model(e.to_string()))?);
            text.push('\n');
        }
        std::fs::write(self.versions_file(), text)?;
        if !rest.iter().any(|v| v.model == sha) && !self.entries().iter().any(|e| e.model == sha) {
            let _ = std::fs::remove_file(self.snapshot(&sha));
        }
        Ok(())
    }
}

/// A named version of a model. Every field but the first has a default: the format can grow.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Version {
    /// Unique: the time it was saved in milliseconds.
    pub id: u64,
    /// When it was saved (seconds since 1970).
    #[serde(default)]
    pub when: u64,
    #[serde(default)]
    pub label: String,
    /// The SHA-256 of the model saved ([`Journal::model`] has it).
    #[serde(default)]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Where it was saved: `app`, `cli` or `mcp`.
    #[serde(default)]
    pub from: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("terraflux-journal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn model(rows: i64) -> Model {
        let text = format!(
            "format = \"terraflux/1\"\n[[nodes]]\nid = \"land\"\nop = \"nlm.random\"\n\
             params = {{ rows = {rows}, columns = 8, seed = 4 }}\n\
             [[parameters]]\nkey = \"land.rows\"\n"
        );
        Model::from_toml(&text).unwrap()
    }

    #[test]
    fn a_run_is_recorded_and_read_back() {
        let d = dir("round");
        let file = d.join("m.toml");
        let j = Journal::of(&file).unwrap();
        assert!(j.entries().is_empty());
        let m = model(10);
        let mut e = Entry::new(Kind::To("land".into()), "app", 1.5);
        e.status = Status::Failed("no data".into());
        e.values = values_of(&m);
        j.record(e.clone(), &m).unwrap();
        let mut f = Entry::new(Kind::Run, "cli", 0.2);
        f.id = e.id;
        j.record(f, &m).unwrap();
        let all = j.entries();
        assert_eq!(all.len(), 2);
        // (the same start time in milliseconds: the second made unique)
        assert_eq!(all[1].id, e.id + 1);
        assert_eq!(all[0].kind, Kind::To("land".into()));
        assert_eq!(all[0].status, Status::Failed("no data".into()));
        assert_eq!(all[0].values["land.rows"], serde_json::json!(10));
        assert_eq!(all[0].from, "app");
        assert_eq!(j.model(&all[0].model).unwrap(), m);
        assert!(d.join(FOLDER).join("m.toml").join("runs.jsonl").is_file());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_model_is_kept_once_for_each_hash() {
        let d = dir("snap");
        let j = Journal::of(&d.join("m.toml")).unwrap();
        for rows in [10, 10, 12] {
            j.record(Entry::new(Kind::Run, "app", 0.0), &model(rows)).unwrap();
        }
        let all = j.entries();
        assert_eq!(all[0].model, all[1].model);
        assert_ne!(all[0].model, all[2].model);
        assert_eq!(std::fs::read_dir(d.join(FOLDER).join("m.toml").join("models")).unwrap().count(), 2);
        assert_eq!(j.model(&all[2].model).unwrap().nodes[0].params["rows"], serde_json::json!(12));
        assert!(j.model("nothing").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_broken_line_is_skipped() {
        let d = dir("broken");
        let j = Journal::of(&d.join("m.toml")).unwrap();
        j.record(Entry::new(Kind::Run, "app", 0.0), &model(10)).unwrap();
        let mut f = std::fs::OpenOptions::new().append(true).open(j.runs()).unwrap();
        writeln!(f, "{{ not json").unwrap();
        // (an entry of a later version: a field unknown, one missing)
        writeln!(f, "{{\"id\": 7, \"later\": true}}").unwrap();
        drop(f);
        j.record(Entry::new(Kind::Run, "app", 0.0), &model(10)).unwrap();
        let all = j.entries();
        assert_eq!(all.len(), 3);
        assert_eq!(all[1].id, 7);
        assert_eq!(all[1].status, Status::Done);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_output_written_over_is_told() {
        let d = dir("over");
        let out = d.join("land.tif");
        std::fs::write(&out, "one").unwrap();
        let w = Written::of("land", &out, None, None).unwrap();
        assert!(w.unchanged());
        std::fs::write(&out, "another one").unwrap();
        assert!(!w.unchanged());
        let w = Written::of("land", &out, None, None).unwrap();
        assert!(w.unchanged());
        // (the same size, written later)
        let mut late = w.clone();
        late.modified += 5000;
        assert!(!late.unchanged());
        std::fs::remove_file(&out).unwrap();
        assert!(!w.unchanged());
        assert!(Written::of("land", &out, None, None).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn versions_are_saved_listed_and_deleted() {
        let d = dir("versions");
        let j = Journal::of(&d.join("m.toml")).unwrap();
        assert!(j.versions().is_empty());
        assert!(j.save_version("  ", None, &model(10), "app").is_err());
        let a = j.save_version("before", Some("the first"), &model(10), "app").unwrap();
        // (the same model as the newest: saved again, under its own label)
        let b = j.save_version("again", Some("  "), &model(10), "mcp").unwrap();
        let c = j.save_version("after", None, &model(12), "cli").unwrap();
        let all = j.versions();
        assert_eq!(all.iter().map(|v| v.label.as_str()).collect::<Vec<_>>(), ["before", "again", "after"]);
        assert_eq!((a.model.clone(), b.note.clone()), (b.model.clone(), None));
        assert_ne!(a.model, c.model);
        assert!(a.id < b.id && b.id < c.id);
        assert_eq!(all[0].note.as_deref(), Some("the first"));
        assert_eq!(all[1].from, "mcp");
        assert_eq!(j.model(&c.model).unwrap(), model(12));
        let snaps = d.join(FOLDER).join("m.toml").join("models");
        assert_eq!(std::fs::read_dir(&snaps).unwrap().count(), 2);
        // the snapshot stays while another version has it, and goes with the last
        j.delete_version(a.id).unwrap();
        assert_eq!(j.versions().len(), 2);
        assert!(j.model(&a.model).is_some());
        j.delete_version(b.id).unwrap();
        assert!(j.model(&a.model).is_none());
        assert_eq!(j.versions()[0].label, "after");
        // a snapshot a run uses stays
        j.record(Entry::new(Kind::Run, "app", 0.0), &model(12)).unwrap();
        j.delete_version(c.id).unwrap();
        assert!(j.versions().is_empty());
        assert!(j.model(&c.model).is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_journal_is_beside_the_model() {
        let j = Journal::of(Path::new("m.toml")).unwrap();
        assert_eq!(j.dir(), Path::new(".").join(FOLDER).join("m.toml"));
        assert!(Journal::of(Path::new("")).is_none());
    }
}
