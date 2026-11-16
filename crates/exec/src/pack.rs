//! A model packed with its data: one `.tfpack` file (a zip file) to send to a supervisor or a reviewer, which opens
//! and runs without any change.
//!
//! The package holds the model (every path to a file rewritten to be relative, into `data/`), the files its nodes
//! read (a shapefile with its `.shx`, `.dbf`, `.prj` and `.cpg`; the rasters of a folder or of a pattern like
//! `runs/*/landuse.tif`; the models of `control.submodel` nodes, with their own data, as deep as they go; plugins),
//! optionally the outputs of the last run (`outputs/`), and `manifest.json`: the version of Terraflux, the date, and
//! each file with its SHA-256, its size and the path it came from. Files of the same name from different folders do
//! not collide (the second goes in a numbered folder). A file the model names that is not there is listed in the
//! manifest and in [`Plan::missing`], not skipped in silence. [`unpack`] writes the files to a folder, checks each
//! against the manifest and says which are not as they were packed.
//!
//! The extension is `.tfpack`: a zip file that every system opens by renaming it, with a name that tells what it is
//! (a double click can open it in Terraflux), the way `.docx` and `.jar` are zip files.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use terraflux_graph::{Input, Model, Node, SUBMODEL, file_settings, is_sent, relative_path};

use crate::zipfile::{Archive, Writer, civil};
use crate::{Error, Result};

/// The extension of a package.
pub const EXTENSION: &str = "tfpack";

/// The format of `manifest.json`.
pub const FORMAT: &str = "terraflux-pack/1";

/// The file of the manifest in a package.
pub const MANIFEST: &str = "manifest.json";

/// Submodels inside submodels go this deep (as the engine's).
const DEPTH: usize = 16;

/// A package this large (bytes) or more is told about before it is written.
pub const LARGE: u64 = 2 << 30;

/// What a file of a package is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Model,
    Submodel,
    Data,
    Output,
}

/// A file of the package, as the manifest lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Packed {
    /// Its path in the package (with `/`).
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    /// The path it came from (a model: the file, before its paths were rewritten).
    pub original: String,
    pub role: Role,
}

/// A file a model names that was not there to pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Missing {
    /// The path as the model has it.
    pub path: String,
    /// Where it is used (`node`, `container/node`, `$input`).
    pub at: String,
    /// The model file that names it (a submodel's, maybe).
    pub model: String,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} in {})", self.path, self.at, self.model)
    }
}

/// What `manifest.json` says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    /// The version of Terraflux that packed.
    pub terraflux: String,
    /// When (UTC, `2026-11-16T09:30:00Z`) and seconds since 1970.
    pub created: String,
    pub when: u64,
    /// The model's path in the package.
    pub model: String,
    pub files: Vec<Packed>,
    /// The files the model names that were not there: the package runs without them only if the nodes that read
    /// them are left out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<Missing>,
}

enum Content {
    File(PathBuf),
    Text(Vec<u8>),
}

struct Item {
    name: String,
    from: Content,
    bytes: u64,
    original: String,
    role: Role,
}

/// What would be packed: the files (and their sizes) and what is missing, before anything is written.
pub struct Plan {
    /// The model's path in the package.
    pub model: String,
    items: Vec<Item>,
    pub missing: Vec<Missing>,
}

impl Plan {
    /// The files of the package: path in it, bytes, role.
    pub fn files(&self) -> impl Iterator<Item = (&str, u64, Role)> {
        self.items.iter().map(|i| (i.name.as_str(), i.bytes, i.role))
    }

    /// The bytes of all the files (before they are compressed).
    pub fn bytes(&self) -> u64 {
        self.items.iter().map(|i| i.bytes).sum()
    }

    /// How many files.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Whether the package is [`LARGE`].
    pub fn large(&self) -> bool {
        self.bytes() >= LARGE
    }

    /// Write the package to `dest` (a file written whole, or not left behind): each file read once, hashed as it
    /// goes in. Returns the manifest and the size of the package.
    pub fn write(&self, dest: &Path) -> Result<(Manifest, u64)> {
        let tmp = dest.with_extension(format!("{EXTENSION}.part"));
        let made = self.write_to(&tmp);
        match made {
            Ok(m) => {
                std::fs::rename(&tmp, dest)?;
                let size = std::fs::metadata(dest).map_or(0, |m| m.len());
                Ok((m, size))
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                Err(e)
            }
        }
    }

    fn write_to(&self, file: &Path) -> Result<Manifest> {
        let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let mut zip = Writer::new(std::io::BufWriter::new(std::fs::File::create(file)?), when);
        let mut files = Vec::new();
        for i in &self.items {
            let mut sha = Sha256::new();
            let mut bytes = 0u64;
            let mut tee = |r: &mut dyn Read| -> std::io::Result<()> {
                let mut r = Tee { r, sha: &mut sha, bytes: &mut bytes };
                zip.add(&i.name, i.bytes, &mut r).map(|_| ())
            };
            match &i.from {
                Content::Text(t) => tee(&mut t.as_slice())?,
                Content::File(p) => {
                    tee(&mut std::fs::File::open(p).map_err(|e| Error::Model(format!("{}: {e}", p.display())))?)?
                }
            }
            files.push(Packed {
                path: i.name.clone(),
                sha256: hex(&sha.finalize()),
                bytes,
                original: i.original.clone(),
                role: i.role,
            });
        }
        let (y, mo, d, h, mi, s) = civil(when);
        let manifest = Manifest {
            format: FORMAT.to_string(),
            terraflux: env!("CARGO_PKG_VERSION").to_string(),
            created: format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z"),
            when,
            model: self.model.clone(),
            files,
            missing: self.missing.clone(),
        };
        let text = serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Model(e.to_string()))?;
        zip.add(MANIFEST, text.len() as u64, &mut text.as_slice())?;
        zip.finish()?.into_inner().map_err(|e| Error::Io(e.into_error()))?.sync_all()?;
        Ok(manifest)
    }
}

/// A reader that hashes and counts what goes through it.
struct Tee<'a> {
    r: &'a mut dyn Read,
    sha: &'a mut Sha256,
    bytes: &'a mut u64,
}

impl Read for Tee<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.r.read(buf)?;
        self.sha.update(&buf[..n]);
        *self.bytes += n as u64;
        Ok(n)
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The outputs of the last whole run of a model, from its journal: the files that are still there. A model that
/// has no run in its journal gives the files of the `output` folder beside it (where a run writes by default), if
/// there is one.
pub fn last_outputs(model_file: &Path) -> Vec<PathBuf> {
    use crate::journal::{Journal, Kind};
    let last = Journal::of(model_file).and_then(|j| {
        j.entries()
            .into_iter()
            .rev()
            .find(|e| matches!(e.kind, Kind::Run) && e.outputs.iter().any(|w| Path::new(&w.path).is_file()))
    });
    if let Some(e) = last {
        return e.outputs.iter().map(|w| PathBuf::from(&w.path)).filter(|p| p.is_file()).collect();
    }
    let dir = match model_file.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.join("output"),
        _ => PathBuf::from("output"),
    };
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    files
}

/// The default file of a package of a model: its name with `.tfpack`, beside it.
pub fn default_package(model_file: &Path) -> PathBuf {
    model_file.with_extension(EXTENSION)
}

/// The default folder to unpack a package in: beside it, named like it (`study.tfpack` into `study`; `study 2` when
/// that is taken).
pub fn default_folder(package: &Path) -> PathBuf {
    let base = package.with_extension("");
    if !base.exists() {
        return base;
    }
    (2..).map(|k| PathBuf::from(format!("{} {k}", base.display()))).find(|p| !p.exists()).unwrap_or(base)
}

/// Whether a file is a package, by its extension.
pub fn is_package(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}

/// Plan the package of a model file: what it reads, found from its folder (and from the folders of its submodels),
/// and `outputs` (files, often [`last_outputs`]) to go with it. Nothing is written.
pub fn plan(model_file: &Path, outputs: &[PathBuf]) -> Result<Plan> {
    let mut p = Packer::default();
    let name = model_file.file_name().map_or("model.toml".to_string(), |n| n.to_string_lossy().into_owned());
    p.take(&name, &PathBuf::from(&name));
    p.model(model_file, &name)?;
    let top = p.items.len() - 1;
    // (the model first in the list)
    let item = p.items.remove(top);
    p.items.insert(0, item);
    for o in outputs {
        if !o.is_file() {
            p.missing.push(Missing {
                path: o.display().to_string(),
                at: "the last outputs".into(),
                model: name.clone(),
            });
            continue;
        }
        let file = o.file_name().map_or("output".to_string(), |n| n.to_string_lossy().into_owned());
        let dest = p.free(&[PathBuf::from(&file)], "outputs", &canon(o));
        let bytes = std::fs::metadata(o).map_or(0, |m| m.len());
        p.items.push(Item {
            name: format!("{dest}/{file}").replace("//", "/"),
            from: Content::File(o.clone()),
            bytes,
            original: absolute(o),
            role: Role::Output,
        });
        let name = p.items.last().map(|i| i.name.clone()).unwrap_or_default();
        p.take(&name, o);
    }
    Ok(Plan { model: name, items: p.items, missing: p.missing })
}

fn absolute(p: &Path) -> String {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().into_owned()
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()))
}

/// The key of a path of the package: names that differ only in case would be one file on Windows and macOS.
fn key(name: &str) -> String {
    name.to_lowercase()
}

#[derive(Default)]
struct Packer {
    items: Vec<Item>,
    /// The paths in the package taken, by name in lower case, and the file that took each.
    taken: BTreeMap<String, PathBuf>,
    /// What a path (made absolute) came to: its path in the package.
    placed: BTreeMap<String, String>,
    /// The submodels packed (by their file): their path in the package.
    subs: BTreeMap<PathBuf, String>,
    missing: Vec<Missing>,
    stack: Vec<PathBuf>,
}

impl Packer {
    fn take(&mut self, name: &str, from: &Path) {
        self.taken.insert(key(name), canon(from));
    }

    /// A folder to put files under (`root`, else `root/2`, `root/3`...) where none of `rels` is taken by another
    /// file than `of`.
    fn free(&self, rels: &[PathBuf], root: &str, of: &Path) -> String {
        for k in 1.. {
            let at = if k == 1 { root.to_string() } else { format!("{root}/{k}") };
            let ok = rels.iter().all(|r| match self.taken.get(&key(&format!("{at}/{}", slashes(r)))) {
                None => true,
                Some(t) => t == of,
            });
            if ok {
                return at;
            }
        }
        unreachable!()
    }

    /// Pack a model file (and what it reads), rewritten: at `dest` in the package.
    fn model(&mut self, file: &Path, dest: &str) -> Result<()> {
        let shown = file.display().to_string();
        let mut m = Model::load(file).map_err(|e| Error::Model(format!("{shown}: {e}")))?;
        let dir = match file.parent() {
            Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let here = Path::new(dest).parent().map(Path::to_path_buf).unwrap_or_default();
        let who = Who { dir: &dir, here: &here, model: &shown };
        let mut inputs = std::mem::take(&mut m.inputs);
        for (name, input) in inputs.iter_mut() {
            if let Input::Map { path, .. } | Input::Stack { path, .. } = input {
                self.point(path, &who, &format!("${name}"), false);
            }
        }
        m.inputs = inputs;
        self.nodes(&mut m.nodes, "", &who)?;
        let text = if Path::new(dest).extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) {
            m.to_toml()
        } else {
            m.to_json()
        }
        .map_err(|e| Error::Model(format!("{shown}: {e}")))?
        .into_bytes();
        let role = if self.stack.is_empty() { Role::Model } else { Role::Submodel };
        self.items.push(Item {
            name: dest.to_string(),
            bytes: text.len() as u64,
            from: Content::Text(text),
            original: absolute(file),
            role,
        });
        Ok(())
    }

    fn nodes(&mut self, nodes: &mut [Node], at: &str, who: &Who) -> Result<()> {
        for n in nodes.iter_mut() {
            let place = if at.is_empty() { n.id.clone() } else { format!("{at}/{}", n.id) };
            if n.op == SUBMODEL {
                self.submodel(n, &place, who)?;
            } else {
                let write = n.op.ends_with(".write");
                for p in file_settings(n) {
                    self.point(p, who, &place, write);
                }
            }
            if let Some(b) = &mut n.body {
                self.nodes(&mut b.nodes, &place, who)?;
            }
        }
        Ok(())
    }

    fn submodel(&mut self, n: &mut Node, place: &str, who: &Who) -> Result<()> {
        let Some(path) = n.params.get("path").and_then(|p| p.as_str()).map(str::to_string) else { return Ok(()) };
        let file = if Path::new(&path).is_absolute() { PathBuf::from(&path) } else { who.dir.join(&path) };
        if !file.is_file() {
            self.missing.push(Missing { path, at: place.to_string(), model: who.model.to_string() });
            return Ok(());
        }
        let canonical = canon(&file);
        let dest = match self.subs.get(&canonical) {
            Some(d) => d.clone(),
            None => {
                if self.stack.contains(&canonical) {
                    return Err(Error::Model(format!("the submodel {place:?} uses itself ({})", file.display())));
                }
                if self.stack.len() >= DEPTH {
                    return Err(Error::Model(format!("submodels nested more than {DEPTH} deep at {place:?}")));
                }
                let name = file.file_name().map_or("submodel.toml".into(), |n| n.to_string_lossy().into_owned());
                let root = self.free(&[PathBuf::from(&name)], "data/submodels", &canonical);
                let dest = format!("{root}/{name}");
                self.take(&dest, &file);
                self.subs.insert(canonical.clone(), dest.clone());
                self.stack.push(canonical);
                let done = self.model(&file, &dest);
                self.stack.pop();
                done?;
                dest
            }
        };
        n.params.insert("path".into(), relative_path(Path::new(&dest), who.here).into());
        Ok(())
    }

    /// The path of a node's setting or an input, made to read the file in the package (or noted as missing).
    fn point(&mut self, p: &mut String, who: &Who, at: &str, write: bool) {
        if p.trim().is_empty() || is_sent(p) {
            return;
        }
        let given = Path::new(p.as_str());
        if write {
            // (a file written is not packed; one at an absolute path would be written there by the recipient)
            if given.is_absolute() {
                let name = given.file_name().map_or("out".to_string(), |n| n.to_string_lossy().into_owned());
                let up = "../".repeat(who.here.components().count());
                *p = format!("{up}output/{name}");
            }
            return;
        }
        let full = if given.is_absolute() { given.to_path_buf() } else { who.dir.join(given) };
        match self.place(&full) {
            Some(dest) => *p = relative_path(Path::new(&dest), who.here),
            None => self.missing.push(Missing { path: p.clone(), at: at.to_string(), model: who.model.to_string() }),
        }
    }

    /// Put what a path names (a file with its siblings, a folder, a pattern) in the package: the path to give the
    /// nodes. None when it is not there.
    fn place(&mut self, path: &Path) -> Option<String> {
        let id = absolute(path);
        if let Some(d) = self.placed.get(&id) {
            return Some(d.clone());
        }
        let (base, files, rel, prefix) = gather(path)?;
        if files.is_empty() {
            return None;
        }
        let rels: Vec<PathBuf> = files
            .iter()
            .map(|f| {
                f.strip_prefix(&base)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|_| f.file_name().unwrap_or_default().into())
            })
            .collect();
        let under = |root: &str, r: &Path| format!("{root}/{}", slashes(r)).replace("//", "/");
        // the first folder where no file of this one meets another file of the same name
        let root = (1..)
            .map(|k| {
                let root = if k == 1 { "data".to_string() } else { format!("data/{k}") };
                if prefix.is_empty() { root } else { format!("{root}/{prefix}") }
            })
            .find(|root| {
                rels.iter()
                    .zip(&files)
                    .all(|(r, f)| self.taken.get(&key(&under(root, r))).is_none_or(|t| *t == canon(f)))
            })?;
        for (r, f) in rels.iter().zip(&files) {
            let name = under(&root, r);
            if self.taken.contains_key(&key(&name)) {
                continue;
            }
            self.take(&name, f);
            let bytes = std::fs::metadata(f).map_or(0, |m| m.len());
            self.items.push(Item {
                name,
                from: Content::File(f.clone()),
                bytes,
                original: absolute(f),
                role: Role::Data,
            });
        }
        let dest = if rel.is_empty() { root } else { format!("{root}/{rel}") };
        self.placed.insert(id, dest.clone());
        Some(dest)
    }
}

struct Who<'a> {
    /// The folder of the model that names the path.
    dir: &'a Path,
    /// The folder of that model in the package ("" for the top).
    here: &'a Path,
    /// That model's file, to say where a missing path is named.
    model: &'a str,
}

fn slashes(p: &Path) -> String {
    p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}

fn wild(c: &Component) -> bool {
    c.as_os_str().to_string_lossy().contains(['*', '?'])
}

/// The files beside a file of the same name (its stem and its whole name followed by a dot): a shapefile's parts,
/// a raster's sidecars.
fn siblings(file: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (file.parent(), file.file_name()) else { return vec![file.to_path_buf()] };
    let name = name.to_string_lossy().to_lowercase();
    let stem = file.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let dir = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
    let mut out = vec![file.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_lowercase();
            if n != name
                && (n.starts_with(&format!("{stem}.")) || n.starts_with(&format!("{name}.")))
                && e.path().is_file()
            {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

/// What a path names, as files: the folder they are relative to, the files, the rest of the path after that folder
/// (the file's name, the pattern after its folder, nothing for a folder) and the name of the folder to put them in
/// (empty for a file). None when it is not there.
fn gather(path: &Path) -> Option<(PathBuf, Vec<PathBuf>, String, String)> {
    let named = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let list = |p: &str| terraflux_ops::files::files_of(p).ok();
    if path.components().any(|c| wild(&c)) {
        let base: PathBuf = path.components().take_while(|c| !wild(c)).collect();
        let files = list(&path.to_string_lossy())?;
        let rel = path.strip_prefix(&base).ok().map(slashes)?;
        let all = files.iter().flat_map(|f| siblings(f)).collect();
        return Some((base.clone(), all, rel, named(&base)));
    }
    if path.is_dir() {
        let files = if terraflux_ops::swat::is_txtinout(path) {
            terraflux_ops::swat::folder_files(path).ok()?
        } else {
            let found = list(&path.to_string_lossy()).unwrap_or_default();
            if found.is_empty() {
                // (no rasters: the files of the folder)
                let mut all: Vec<PathBuf> =
                    std::fs::read_dir(path).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
                all.sort();
                all
            } else {
                found.iter().flat_map(|f| siblings(f)).collect()
            }
        };
        return Some((path.to_path_buf(), files, String::new(), named(path)));
    }
    if !path.is_file() {
        return None;
    }
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    Some((base, siblings(path), named(path), String::new()))
}

/// What [`unpack`] did.
#[derive(Debug)]
pub struct Unpacked {
    pub folder: PathBuf,
    /// The model, unpacked.
    pub model: PathBuf,
    pub manifest: Manifest,
    /// The files written.
    pub files: usize,
    /// What is not as it was packed: a file changed (its SHA-256 or its CRC-32 differs), cut, or not in the
    /// manifest, or one the manifest lists that the package lacks. Empty when the package is whole.
    pub problems: Vec<String>,
}

/// A name in a package that stays inside the folder it is unpacked in.
fn safe(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(['\\', ':'])
        && name.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Unpack a package into `folder` (made when it is not there; files of the same name are written over), and check
/// every file against the manifest. The files are written even when something does not check: the problems are in
/// the result, and the caller says no to the package.
pub fn unpack(package: &Path, folder: &Path) -> Result<Unpacked> {
    let shown = package.display();
    let file = std::fs::File::open(package).map_err(|e| Error::Model(format!("{shown}: {e}")))?;
    let mut zip = Archive::open(std::io::BufReader::new(file))
        .map_err(|e| Error::Model(format!("{shown} is not a package: {e}")))?;
    let at = zip.find(MANIFEST).ok_or_else(|| Error::Model(format!("{shown} has no {MANIFEST}: not a package")))?;
    let mut text = Vec::new();
    if zip.entries[at].size > 64 << 20 {
        return Err(Error::Model(format!("{shown}: the manifest is too large")));
    }
    zip.extract(at, &mut text).map_err(|e| Error::Model(format!("{shown}: {MANIFEST}: {e}")))?;
    let manifest: Manifest =
        serde_json::from_slice(&text).map_err(|e| Error::Model(format!("{shown}: {MANIFEST}: {e}")))?;
    if !manifest.format.starts_with("terraflux-pack/") {
        return Err(Error::Model(format!(
            "{shown}: a package of format {:?}, not one this Terraflux knows",
            manifest.format
        )));
    }
    std::fs::create_dir_all(folder)?;
    let mut problems = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut files = 0;
    for i in 0..zip.entries.len() {
        let name = zip.entries[i].name.clone();
        if name == MANIFEST || zip.entries[i].is_dir() {
            continue;
        }
        if !safe(&name) {
            problems.push(format!("{name}: a name that leads out of the folder, not unpacked"));
            continue;
        }
        let dest = folder.join(&name);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut sha = Sha256::new();
        let out = std::fs::File::create(&dest).map_err(|e| Error::Model(format!("{}: {e}", dest.display())))?;
        let mut out = HashWrite { w: std::io::BufWriter::new(out), sha: &mut sha };
        let read = zip.extract(i, &mut out);
        let flushed = out.w.flush();
        files += 1;
        seen.insert(name.clone());
        if let Err(e) = read.and(flushed) {
            problems.push(format!("{name}: {e}"));
            continue;
        }
        match manifest.files.iter().find(|f| f.path == name) {
            None => problems.push(format!("{name}: not in the manifest")),
            Some(f) if f.sha256 != hex(&sha.finalize()) => {
                problems.push(format!("{name}: changed (its SHA-256 is not the one in the manifest)"))
            }
            Some(_) => {}
        }
    }
    for f in &manifest.files {
        if !seen.contains(&f.path) {
            problems.push(format!("{}: in the manifest, not in the package", f.path));
        }
    }
    std::fs::write(folder.join(MANIFEST), &text)?;
    let model = folder.join(&manifest.model);
    if !safe(&manifest.model) || !model.is_file() {
        problems.push(format!("{}: the model is not there", manifest.model));
    }
    Ok(Unpacked { folder: folder.to_path_buf(), model, manifest, files, problems })
}

struct HashWrite<'a, W: Write> {
    w: W,
    sha: &'a mut Sha256,
}

impl<W: Write> Write for HashWrite<'_, W> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.sha.update(b);
        self.w.write_all(b)?;
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.w.flush()
    }
}

/// The size of a number of bytes in words (`3.4 GB`).
pub fn size_text(bytes: u64) -> String {
    let b = bytes as f64;
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..0x10_0000 => format!("{:.1} KB", b / 1024.0),
        0x10_0000..0x4000_0000 => format!("{:.1} MB", b / 1_048_576.0),
        _ => format!("{:.1} GB", b / 1_073_741_824.0),
    }
}

/// The words that tell what a plan holds and lacks, to show before the package is written.
pub fn words(plan: &Plan) -> String {
    let mut s = format!("{} files, {}", plan.count(), size_text(plan.bytes()));
    if plan.large() {
        s += " (over 2 GB: a large file to send; leave out the data the recipient has)";
    }
    if !plan.missing.is_empty() {
        s += &format!("; {} not found:", plan.missing.len());
        for m in &plan.missing {
            s += &format!("\n  {m}");
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("terraflux-pack-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_package_holds_the_model_and_what_it_reads_and_unpacks_checked() {
        let d = dir("whole");
        // a shapefile with its siblings, a folder, a pattern, two files of one name, a submodel with its own data
        for f in ["roads.shp", "roads.shx", "roads.dbf", "roads.prj", "roads.cpg", "roadside.shp"] {
            write(&d.join("gis").join(f), f);
        }
        write(&d.join("a/land.tif"), "a");
        write(&d.join("b/land.tif"), "b");
        write(&d.join("tiles/t1.tif"), "t1");
        write(&d.join("tiles/t2.tif"), "t2");
        write(&d.join("runs/x/out.tif"), "x");
        write(&d.join("runs/y/out.tif"), "y");
        write(&d.join("sub/inner.csv"), "1,2");
        write(&d.join("sub/land.tif"), "s");
        write(
            &d.join("sub/part.toml"),
            "format = \"terraflux/1\"\n[[nodes]]\nid = \"t\"\nop = \"table.read\"\nparams = { path = \"inner.csv\" }\n\
             [[nodes]]\nid = \"l\"\nop = \"raster.read\"\nparams = { path = \"land.tif\" }\n",
        );
        let abs = d.join("b/land.tif").to_string_lossy().replace('\\', "/");
        write(
            &d.join("model.toml"),
            &format!(
                "format = \"terraflux/1\"\n\
             [[nodes]]\nid = \"r1\"\nop = \"raster.read\"\nparams = {{ path = \"a/land.tif\" }}\n\
             [[nodes]]\nid = \"r2\"\nop = \"raster.read\"\nparams = {{ path = \"{abs}\" }}\n\
             [[nodes]]\nid = \"v\"\nop = \"vector.read\"\nparams = {{ path = \"gis/roads.shp\" }}\n\
             [[nodes]]\nid = \"f\"\nop = \"raster.read\"\nparams = {{ path = \"tiles\" }}\n\
             [[nodes]]\nid = \"p\"\nop = \"raster.read\"\nparams = {{ path = \"runs/*/out.tif\" }}\n\
             [[nodes]]\nid = \"gone\"\nop = \"raster.read\"\nparams = {{ path = \"nowhere/none.tif\" }}\n\
             [[nodes]]\nid = \"s\"\nop = \"control.submodel\"\nparams = {{ path = \"sub/part.toml\" }}\n\
             [[nodes]]\nid = \"w\"\nop = \"raster.write\"\nparams = {{ path = \"{abs}.out.tif\" }}\n"
            ),
        );
        write(&d.join("out/result.txt"), "result");
        let plan = plan(&d.join("model.toml"), &[d.join("out/result.txt")]).unwrap();
        let names: Vec<&str> = plan.files().map(|f| f.0).collect();
        for want in [
            "model.toml",
            "data/land.tif",
            "data/2/land.tif",
            "data/roads.shp",
            "data/roads.shx",
            "data/roads.dbf",
            "data/roads.prj",
            "data/roads.cpg",
            "data/tiles/t1.tif",
            "data/tiles/t2.tif",
            "data/runs/x/out.tif",
            "data/runs/y/out.tif",
            "data/submodels/part.toml",
            "data/inner.csv",
            "data/3/land.tif",
            "outputs/result.txt",
        ] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        assert!(!names.contains(&"data/roadside.shp"), "{names:?}");
        assert_eq!(names[0], "model.toml");
        assert_eq!(plan.missing.len(), 1);
        assert_eq!(plan.missing[0].at, "gone");
        assert!(words(&plan).contains("nowhere/none.tif"));
        // written, then unpacked elsewhere
        let file = d.join("study.tfpack");
        let (manifest, size) = plan.write(&file).unwrap();
        assert!(size > 0 && manifest.files.len() == plan.count());
        assert!(manifest.files.iter().all(|f| f.sha256.len() == 64));
        let land = manifest.files.iter().find(|f| f.path == "data/2/land.tif").unwrap();
        assert_eq!((land.bytes, land.original.replace('\\', "/").ends_with("b/land.tif")), (1, true));
        let out = dir("whole-out");
        let u = unpack(&file, &out).unwrap();
        assert_eq!(u.problems, Vec::<String>::new());
        assert_eq!(u.files, plan.count());
        assert_eq!(std::fs::read_to_string(out.join("data/runs/y/out.tif")).unwrap(), "y");
        // the model's paths are relative: the files it names are in the folder
        let m = Model::load(&u.model).unwrap();
        let path = |id: &str| m.nodes.iter().find(|n| n.id == id).unwrap().params["path"].as_str().unwrap().to_string();
        assert_eq!(path("r1"), "data/land.tif");
        assert_eq!(path("r2"), "data/2/land.tif");
        assert_eq!(path("p"), "data/runs/*/out.tif");
        assert_eq!(path("gone"), "nowhere/none.tif");
        assert_eq!(path("w"), "output/land.tif.out.tif");
        for id in ["r1", "r2", "v", "f", "s"] {
            assert!(out.join(path(id)).exists(), "{id}: {}", path(id));
        }
        // the submodel finds its own data from its place in the package
        let sub = Model::load(out.join("data/submodels/part.toml")).unwrap();
        let sp = |id: &str| sub.nodes.iter().find(|n| n.id == id).unwrap().params["path"].as_str().unwrap().to_string();
        assert_eq!(sp("t"), "../inner.csv");
        assert_eq!(sp("l"), "../3/land.tif");
        assert!(out.join("data/submodels").join(sp("l")).exists());
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn a_file_changed_after_packing_is_told() {
        let d = dir("changed");
        write(&d.join("land.tif"), "land land land land land land");
        write(
            &d.join("model.toml"),
            "format = \"terraflux/1\"\n[[nodes]]\nid = \"r\"\nop = \"raster.read\"\nparams = { path = \"land.tif\" }\n",
        );
        let file = d.join("m.tfpack");
        plan(&d.join("model.toml"), &[]).unwrap().write(&file).unwrap();
        // the same size, other letters, stored as deflated data would be: rewrite the package with another file
        // under the same manifest
        let out = dir("changed-out");
        assert!(unpack(&file, &out).unwrap().problems.is_empty());
        // a package whose manifest hash is not the file's
        let mut zip = Archive::open(std::io::BufReader::new(std::fs::File::open(&file).unwrap())).unwrap();
        let mut manifest = Vec::new();
        let at = zip.find(MANIFEST).unwrap();
        zip.extract(at, &mut manifest).unwrap();
        let mut m: Manifest = serde_json::from_slice(&manifest).unwrap();
        let bad = d.join("bad.tfpack");
        let mut w = Writer::new(std::fs::File::create(&bad).unwrap(), 0);
        for (k, e) in zip.entries.clone().iter().enumerate() {
            if e.name == MANIFEST {
                continue;
            }
            let mut data = Vec::new();
            zip.extract(k, &mut data).unwrap();
            if e.name == "data/land.tif" {
                data = b"land land land land land LAND".to_vec();
            }
            w.add(&e.name, data.len() as u64, &mut data.as_slice()).unwrap();
        }
        w.add("data/extra.txt", 1, &mut b"x".as_slice()).unwrap();
        m.files.push(Packed {
            path: "data/ghost.txt".into(),
            sha256: "0".repeat(64),
            bytes: 1,
            original: String::new(),
            role: Role::Data,
        });
        let text = serde_json::to_vec(&m).unwrap();
        w.add(MANIFEST, text.len() as u64, &mut text.as_slice()).unwrap();
        w.finish().unwrap();
        let out2 = dir("changed-out2");
        let u = unpack(&bad, &out2).unwrap();
        let said = u.problems.join("\n");
        assert!(said.contains("data/land.tif: changed"), "{said}");
        assert!(said.contains("data/extra.txt: not in the manifest"), "{said}");
        assert!(said.contains("data/ghost.txt: in the manifest, not in the package"), "{said}");
        assert_eq!(u.problems.len(), 3, "{said}");
        for x in [&d, &out, &out2] {
            let _ = std::fs::remove_dir_all(x);
        }
    }

    #[test]
    fn a_name_that_leads_out_of_the_folder_is_not_unpacked() {
        let d = dir("slip");
        let file = d.join("evil.tfpack");
        let mut w = Writer::new(std::fs::File::create(&file).unwrap(), 0);
        w.add("../escaped.txt", 1, &mut b"x".as_slice()).unwrap();
        let m = Manifest {
            format: FORMAT.into(),
            terraflux: "0".into(),
            created: String::new(),
            when: 0,
            model: "m.toml".into(),
            files: vec![],
            missing: vec![],
        };
        let text = serde_json::to_vec(&m).unwrap();
        w.add(MANIFEST, text.len() as u64, &mut text.as_slice()).unwrap();
        w.finish().unwrap();
        let out = d.join("out");
        let u = unpack(&file, &out).unwrap();
        assert!(u.problems.iter().any(|p| p.contains("leads out")), "{:?}", u.problems);
        assert!(!d.join("escaped.txt").exists());
        assert!(unpack(&d.join("none.tfpack"), &out).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_names_of_the_package_and_its_folder() {
        let d = dir("names");
        assert_eq!(default_package(Path::new("a/m.toml")), Path::new("a/m.tfpack"));
        let pack = d.join("study.tfpack");
        assert_eq!(default_folder(&pack), d.join("study"));
        std::fs::create_dir_all(d.join("study")).unwrap();
        assert_eq!(default_folder(&pack), PathBuf::from(format!("{} 2", d.join("study").display())));
        assert!(is_package(Path::new("x.TFPACK")) && !is_package(Path::new("x.zip")));
        assert_eq!(size_text(1536), "1.5 KB");
        let _ = std::fs::remove_dir_all(&d);
    }
}
