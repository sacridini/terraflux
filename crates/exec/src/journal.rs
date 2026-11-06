//! The journal of a model's runs: each run asked for recorded beside the model (`.terraflux/<model file>/`), with
//! when it ran, how long, how it ended, the values it ran with, the outputs it wrote and the model as it was. The
//! application, `terraflux run` and the MCP write it; the application's Runs pane reads it.
//!
//! `runs.jsonl` holds an [`Entry`] a line, appended (a line that does not read is skipped); `models/<sha256>.json`
//! the model as it ran, once for each hash. The small outputs are copied with the run (`outputs/<id>/<k>/`, within
//! the limits of [`Keep`]), so they can be seen after a later run writes over them; the rest is only recorded, and
//! an output written over since is told by [`Written::unchanged`].
//!
//! Several programs may write one journal at once: every write holds a lock on `<journal>/lock` ([`Journal::lock`])
//! and the files that are replaced are written aside and renamed, so a reader (which takes no lock) never sees half
//! a file. The folder `.terraflux/` keeps a `.gitignore` of its own: the journals are not committed.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// The copy kept with the run: its path in the journal's folder, with `/` ([`Journal::kept`] has the file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
}

/// How much of the outputs a journal copies, in bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keep {
    /// The most the copies of one run take (0: none is copied).
    pub per_run: u64,
    /// The most all the copies of the journal take: the oldest runs' copies are dropped first.
    pub total: u64,
}

impl Default for Keep {
    fn default() -> Keep {
        Keep { per_run: 50 << 20, total: 1 << 30 }
    }
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
            kept: None,
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
    keep: Keep,
}

const IGNORE: &str =
    "# Terraflux's journal of runs (see terraflux_exec::journal): kept on this machine, not committed\n*\n";

static TEMPS: AtomicU64 = AtomicU64::new(0);

/// Write a file aside, in its folder, then rename it over the target: a reader never sees half of it.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let n = TEMPS.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()));
    let done = std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, path));
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(done?)
}

/// The bytes of a file or of everything under a folder (links are not followed).
fn tree_bytes(path: &Path) -> u64 {
    let Ok(m) = std::fs::symlink_metadata(path) else { return 0 };
    if m.is_dir() {
        let Ok(rd) = std::fs::read_dir(path) else { return 0 };
        rd.flatten().map(|e| tree_bytes(&e.path())).sum()
    } else if m.is_file() {
        m.len()
    } else {
        0
    }
}

/// The files beside an output that belong to it (those that exist).
fn sidecars(path: &Path) -> Vec<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut all = vec![path.with_file_name(format!("{name}.aux.xml")), path.with_file_name(format!("{name}.ovr"))];
    match ext.as_str() {
        "shp" => {
            all.extend(["shx", "dbf", "prj", "cpg", "qix", "sbn", "sbx"].map(|e| path.with_extension(e)));
            all.push(path.with_file_name(format!("{name}.xml")));
        }
        "tif" | "tiff" => all.extend(["tfw", "tifw", "wld"].map(|e| path.with_extension(e))),
        _ => {}
    }
    all.retain(|p| p.is_file());
    all
}

impl Journal {
    /// The journal of a model's file: `.terraflux/<file name>/` beside it (none for a path without a file name).
    pub fn of(model_file: &Path) -> Option<Journal> {
        let name = model_file.file_name()?;
        let parent = model_file.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        Some(Journal { dir: parent.join(FOLDER).join(name), keep: Keep::default() })
    }

    /// The journal copying the outputs within `keep`.
    pub fn keeping(mut self, keep: Keep) -> Journal {
        self.keep = keep;
        self
    }

    /// Take the journal's lock, across programs (waits for it): the folder and its `.gitignore` made, `<journal>/lock`
    /// held until the file returned is dropped.
    fn lock(&self) -> Result<std::fs::File> {
        std::fs::create_dir_all(&self.dir)?;
        if let Some(top) = self.dir.parent() {
            // (never over one that is there)
            match std::fs::OpenOptions::new().write(true).create_new(true).open(top.join(".gitignore")) {
                Ok(mut f) => f.write_all(IGNORE.as_bytes())?,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.dir.join("lock"))?;
        file.lock()?;
        Ok(file)
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
    /// The small outputs are copied (see [`Keep`]) unless it is a batch.
    pub fn record(&self, mut entry: Entry, model: &Model) -> Result<()> {
        let text = model.to_json().map_err(|e| Error::Model(e.to_string()))?;
        entry.model = sha_of(&text);
        let _lock = self.lock()?;
        let snap = self.snapshot(&entry.model);
        if !snap.exists() {
            std::fs::create_dir_all(self.dir.join("models"))?;
            write_atomic(&snap, text.as_bytes())?;
        }
        // (an id another run has: one more)
        let known: Vec<u64> = self.entries().iter().map(|e| e.id).collect();
        while known.contains(&entry.id) {
            entry.id += 1;
        }
        if !matches!(entry.kind, Kind::Batch(_)) && self.keep.per_run > 0 {
            self.copy_outputs(&mut entry);
        }
        let line = serde_json::to_string(&entry).map_err(|e| Error::Model(e.to_string()))?;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(self.runs())?;
        writeln!(file, "{line}")?;
        drop(file);
        self.prune(entry.id);
        Ok(())
    }

    fn outputs_dir(&self) -> PathBuf {
        self.dir.join("outputs")
    }

    /// Copy the outputs that are files, with their sidecars, while they fit in `keep.per_run`; `kept` set on each.
    fn copy_outputs(&self, entry: &mut Entry) {
        let mut left = self.keep.per_run;
        for (k, w) in entry.outputs.iter_mut().enumerate() {
            let path = Path::new(&w.path);
            let Some(file) = path.file_name().filter(|_| path.is_file()) else { continue };
            let mut files = vec![path.to_path_buf()];
            files.extend(sidecars(path));
            let bytes: u64 = files.iter().map(|f| std::fs::metadata(f).map_or(0, |m| m.len())).sum();
            if bytes > left {
                continue;
            }
            let to = self.outputs_dir().join(entry.id.to_string()).join(k.to_string());
            let copied = std::fs::create_dir_all(&to).and_then(|_| {
                files.iter().try_for_each(|f| std::fs::copy(f, to.join(f.file_name().unwrap_or_default())).map(|_| ()))
            });
            if copied.is_ok() {
                left -= bytes;
                w.kept = Some(format!("outputs/{}/{k}/{}", entry.id, file.to_string_lossy()));
            } else {
                let _ = std::fs::remove_dir_all(&to);
            }
        }
    }

    /// Drop the oldest runs' copies while all take more than `keep.total`; never those of run `current`.
    fn prune(&self, current: u64) {
        let Ok(rd) = std::fs::read_dir(self.outputs_dir()) else { return };
        let mut runs: Vec<(u64, u64)> =
            rd.flatten().filter_map(|e| Some((e.file_name().to_str()?.parse().ok()?, tree_bytes(&e.path())))).collect();
        runs.sort_unstable();
        let mut total: u64 = runs.iter().map(|r| r.1).sum();
        for (id, bytes) in runs {
            if total <= self.keep.total {
                break;
            }
            if id != current && std::fs::remove_dir_all(self.outputs_dir().join(id.to_string())).is_ok() {
                total -= bytes;
            }
        }
    }

    /// The copy kept of an output, when it has one and the file is there.
    pub fn kept(&self, w: &Written) -> Option<PathBuf> {
        let rel = w.kept.as_deref()?;
        if rel.split('/').any(|p| p == "..") {
            return None;
        }
        let path = self.dir.join(rel);
        path.is_file().then_some(path)
    }

    /// The bytes of all the copies kept.
    pub fn kept_bytes(&self) -> u64 {
        tree_bytes(&self.outputs_dir())
    }

    /// Remove all the copies kept; the bytes removed. The entries keep their `kept` (see [`Journal::kept`]).
    pub fn clear_kept(&self) -> Result<u64> {
        let _lock = self.lock()?;
        let bytes = self.kept_bytes();
        match std::fs::remove_dir_all(self.outputs_dir()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(bytes)
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
        let _lock = self.lock()?;
        let snap = self.snapshot(&sha);
        if !snap.exists() {
            std::fs::create_dir_all(self.dir.join("models"))?;
            write_atomic(&snap, text.as_bytes())?;
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
        let _lock = self.lock()?;
        let all = self.versions();
        let Some(gone) = all.iter().find(|v| v.id == id) else { return Ok(()) };
        let sha = gone.model.clone();
        let rest: Vec<&Version> = all.iter().filter(|v| v.id != id).collect();
        let mut text = String::new();
        for v in &rest {
            text.push_str(&serde_json::to_string(v).map_err(|e| Error::Model(e.to_string()))?);
            text.push('\n');
        }
        write_atomic(&self.versions_file(), text.as_bytes())?;
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

    /// An entry with one output per path, with the id given.
    fn entry_with(id: u64, kind: Kind, paths: &[&Path]) -> Entry {
        let mut e = Entry::new(kind, "app", 0.0);
        e.id = id;
        e.outputs = paths.iter().map(|p| Written::of("o", p, None, None).unwrap()).collect();
        e
    }

    #[test]
    fn the_gitignore_is_made_once_and_ignores_itself() {
        let d = dir("ignore");
        let j = Journal::of(&d.join("m.toml")).unwrap();
        j.record(Entry::new(Kind::Run, "app", 0.0), &model(10)).unwrap();
        let ignore = d.join(FOLDER).join(".gitignore");
        let text = std::fs::read_to_string(&ignore).unwrap();
        assert!(text.lines().any(|l| l == "*"));
        // (one that is there is left as it is)
        std::fs::write(&ignore, "mine\n").unwrap();
        j.record(Entry::new(Kind::Run, "app", 0.0), &model(10)).unwrap();
        assert_eq!(std::fs::read_to_string(&ignore).unwrap(), "mine\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn journals_of_one_file_written_at_once_keep_every_run() {
        let d = dir("threads");
        let file = d.join("m.toml");
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let file = file.clone();
                std::thread::spawn(move || {
                    let j = Journal::of(&file).unwrap();
                    for _ in 0..20 {
                        j.record(Entry::new(Kind::Run, "app", 0.0), &model(10)).unwrap();
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let mut ids: Vec<u64> = Journal::of(&file).unwrap().entries().iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), 80);
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 80);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_output_is_kept_after_it_is_written_over() {
        let d = dir("kept");
        let out = d.join("land.tif");
        std::fs::write(&out, "one").unwrap();
        let j = Journal::of(&d.join("m.toml")).unwrap();
        j.record(entry_with(1000, Kind::Run, &[&out]), &model(10)).unwrap();
        std::fs::write(&out, "another one").unwrap();
        let w = j.entries()[0].outputs[0].clone();
        assert_eq!(w.kept.as_deref(), Some("outputs/1000/0/land.tif"));
        assert!(!w.unchanged());
        assert_eq!(std::fs::read(j.kept(&w).unwrap()).unwrap(), b"one");
        assert_eq!(j.kept_bytes(), 3);
        // (an old journal's entry has no `kept`)
        let old: Written = serde_json::from_str("{\"name\":\"n\",\"path\":\"p\"}").unwrap();
        assert!(old.kept.is_none() && j.kept(&old).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_output_over_the_limit_is_skipped_and_the_next_tried() {
        let d = dir("limit");
        let (big, small, folder) = (d.join("big.tif"), d.join("small.tif"), d.join("folder"));
        std::fs::write(&big, vec![1u8; 100]).unwrap();
        std::fs::write(&small, vec![2u8; 10]).unwrap();
        std::fs::create_dir_all(&folder).unwrap();
        let j = Journal::of(&d.join("m.toml")).unwrap().keeping(Keep { per_run: 50, total: 1000 });
        j.record(entry_with(1000, Kind::Run, &[&big, &folder, &small]), &model(10)).unwrap();
        let e = &j.entries()[0];
        assert!(e.outputs[0].kept.is_none() && e.outputs[1].kept.is_none());
        assert_eq!(e.outputs[2].kept.as_deref(), Some("outputs/1000/2/small.tif"));
        assert_eq!(j.kept_bytes(), 10);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_batch_or_a_zero_limit_copies_nothing() {
        let d = dir("none");
        let out = d.join("land.tif");
        std::fs::write(&out, "one").unwrap();
        let j = Journal::of(&d.join("m.toml")).unwrap();
        j.record(entry_with(1000, Kind::Batch(Batch::default()), &[&out]), &model(10)).unwrap();
        let none = j.clone().keeping(Keep { per_run: 0, total: 1 << 20 });
        none.record(entry_with(2000, Kind::Run, &[&out]), &model(10)).unwrap();
        assert!(j.entries().iter().all(|e| e.outputs[0].kept.is_none()));
        assert!(!d.join(FOLDER).join("m.toml").join("outputs").exists());
        assert_eq!(j.kept_bytes(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_shapefile_is_kept_with_its_sidecars() {
        let d = dir("shp");
        let shp = d.join("roads.shp");
        for ext in ["shp", "shx", "dbf", "prj"] {
            std::fs::write(d.join(format!("roads.{ext}")), ext).unwrap();
        }
        std::fs::write(d.join("other.dbf"), "no").unwrap();
        let j = Journal::of(&d.join("m.toml")).unwrap();
        j.record(entry_with(1000, Kind::Run, &[&shp]), &model(10)).unwrap();
        let w = j.entries()[0].outputs[0].clone();
        let copy = j.kept(&w).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(copy.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["roads.dbf", "roads.prj", "roads.shp", "roads.shx"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_total_drops_the_oldest_copies_and_clear_empties() {
        let d = dir("total");
        let out = d.join("land.tif");
        let j = Journal::of(&d.join("m.toml")).unwrap().keeping(Keep { per_run: 1000, total: 150 });
        for id in [1000, 2000] {
            std::fs::write(&out, vec![1u8; 100]).unwrap();
            j.record(entry_with(id, Kind::Run, &[&out]), &model(10)).unwrap();
        }
        let all = j.entries();
        assert!(j.kept(&all[0].outputs[0]).is_none());
        assert!(j.kept(&all[1].outputs[0]).is_some());
        assert_eq!(j.kept_bytes(), 100);
        assert_eq!(j.clear_kept().unwrap(), 100);
        assert_eq!(j.kept_bytes(), 0);
        assert!(j.kept(&all[1].outputs[0]).is_none());
        assert_eq!(j.clear_kept().unwrap(), 0);
        assert!(j.entries()[1].outputs[0].kept.is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_journal_is_beside_the_model() {
        let j = Journal::of(Path::new("m.toml")).unwrap();
        assert_eq!(j.dir(), Path::new(".").join(FOLDER).join("m.toml"));
        assert!(Journal::of(Path::new("")).is_none());
    }
}
