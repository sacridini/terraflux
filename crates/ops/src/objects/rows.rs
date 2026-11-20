//! `objects.features` on other machines (a cluster): the groups that read the image object by object (spectral, shape,
//! texture) are cut into tasks a worker runs on its own, from the rasters on disk and the objects' labels; the
//! coordinator joins the columns and works out the context (it reads the neighbours, the classes and the levels,
//! which a worker has not) from the same objects. The table is the one a run here makes: every task is the group's
//! own function on the same objects and image.
//!
//! The tasks: the spectral group whole, the shape group whole, and the texture one task per layer it reads (its
//! columns do not depend on the other layers'), put back in the order the group gives them.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_raster::Grid;

use super::Objects;
use super::big::{Layered, Part};
use super::features::{GLCM, GROUPS, Input, Roles, glcm_column, texture_layout};
use crate::files::Rasters;
use crate::{Error, Result};

/// The columns of a group: name and a value per object.
pub type FeatureColumns = Vec<(String, Vec<f64>)>;

/// The layers of one set of rasters on disk: where the files are, the no-data value of every layer of the files, and
/// which layers are read (their number among the layers of the files).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskLayers {
    pub files: Vec<PathBuf>,
    pub nodata: Vec<Option<f64>>,
    pub layers: Vec<usize>,
}

/// One task of the job: a group (by its place in the groups) and, for the texture, the one layer it reads (`brightness`
/// or a layer's name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureTask {
    pub group: usize,
    pub band: Option<String>,
}

/// The features of objects on rasters on disk, as another machine can work them out: the layers (the image's, then the
/// more's), the objects' labels in a file (raw little-endian u32, a cell each), and the settings of the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureJob {
    pub node: String,
    pub sources: Vec<DiskLayers>,
    pub labels: PathBuf,
    pub grid: Grid,
    pub count: usize,
    pub names: Vec<String>,
    /// How many layers are the image's.
    pub image: usize,
    /// The red, near infrared, green and short-wave infrared layers.
    pub roles: [Option<usize>; 4],
    pub levels: usize,
    /// The features of each group (in the order of the groups): name, and whether the user named it.
    pub chosen: Vec<Vec<(String, bool)>>,
    pub tasks: Vec<FeatureTask>,
}

impl FeatureJob {
    /// The files the job reads (to send them to another machine, or to find them there).
    pub fn paths_mut(&mut self) -> impl Iterator<Item = &mut PathBuf> {
        self.sources.iter_mut().flat_map(|s| s.files.iter_mut()).chain(std::iter::once(&mut self.labels))
    }

    /// The layers, opened.
    fn open(&self) -> Result<Vec<Part>> {
        let mut parts = Vec::new();
        for s in &self.sources {
            let name = s.files.first().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            let mut r = Rasters::open_files(&name, s.files.clone(), None, None)?;
            if r.layers.len() != s.nodata.len() || s.layers.iter().any(|&l| l >= r.layers.len()) {
                return Err(Error::msg(format!("{name}: not the rasters the job was made for")));
            }
            for (l, nd) in r.layers.iter_mut().zip(&s.nodata) {
                l.nodata = *nd;
            }
            let r = Arc::new(r);
            if (r.grid.width, r.grid.height) != (self.grid.width, self.grid.height) {
                return Err(Error::msg(format!("{name}: not the grid the job was made for")));
            }
            parts.extend(s.layers.iter().map(|&l| Part::Disk(r.clone(), l)));
        }
        Ok(parts)
    }

    /// The columns of some tasks (by number), one task after another (each uses the threads): what a worker does.
    pub fn run(&self, which: &[usize]) -> Result<Vec<FeatureColumns>> {
        crate::threads::ensure();
        let parts = self.open()?;
        if parts.len() != self.names.len() {
            return Err(Error::msg("the job's layers and their names differ"));
        }
        let bytes = std::fs::read(&self.labels).map_err(io)?;
        if bytes.len() != self.grid.cells() * 4 {
            return Err(Error::msg(format!("{}: not the labels the job was made for", self.labels.display())));
        }
        let labels: Vec<u32> = bytes.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b)).collect();
        drop(bytes);
        let o = Objects::from_labels(Arc::new(self.grid.clone()), labels, self.count)?;
        let [red, nir, green, swir] = self.roles;
        which
            .iter()
            .map(|&k| {
                let task = self.tasks.get(k).ok_or_else(|| Error::msg(format!("no task {k}")))?;
                let group = GROUPS.get(task.group).ok_or_else(|| Error::msg(format!("no group {}", task.group)))?;
                let want: Vec<(&'static str, bool)> = self
                    .chosen
                    .get(task.group)
                    .into_iter()
                    .flatten()
                    .filter_map(|(n, named)| group.features.iter().find(|f| *f == n).map(|f| (*f, *named)))
                    .collect();
                let input = Input {
                    node: &self.node,
                    o: &o,
                    index: Default::default(),
                    names: self.names.clone(),
                    data: Vec::new(),
                    strips: Some(Layered::new(parts.clone(), self.grid.width, self.grid.height)),
                    image: self.image,
                    roles: Roles { red, nir, green, swir },
                    levels: self.levels,
                    texture_bands: task.band.clone().map(|b| vec![b]),
                    spur: super::skeleton::SPUR,
                    below: None,
                };
                (group.run)(&input, &want)
            })
            .collect()
    }
}

fn io(e: std::io::Error) -> Error {
    Error::msg(e.to_string())
}

/// The labels in a temporary file (raw little-endian u32), removed when it goes out of scope.
struct LabelsFile(PathBuf);

impl LabelsFile {
    fn write(labels: &[u32]) -> Result<LabelsFile> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("terraflux-labels-{}-{n}.bin", std::process::id()));
        let file = LabelsFile(path);
        let mut w = std::io::BufWriter::new(std::fs::File::create(&file.0).map_err(io)?);
        for chunk in labels.chunks(1 << 16) {
            let bytes: Vec<u8> = chunk.iter().flat_map(|l| l.to_le_bytes()).collect();
            w.write_all(&bytes).map_err(io)?;
        }
        w.flush().map_err(io)?;
        Ok(file)
    }
}

impl Drop for LabelsFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The sets of rasters on disk the layers come from, or none when one is in memory or part of a file.
fn sources_of(parts: &[Part]) -> Option<Vec<DiskLayers>> {
    let mut out: Vec<(Arc<Rasters>, Vec<usize>)> = Vec::new();
    for p in parts {
        let Part::Disk(r, l) = p else { return None };
        match out.last_mut() {
            Some((last, layers)) if Arc::ptr_eq(last, r) => layers.push(*l),
            _ => out.push((r.clone(), vec![*l])),
        }
    }
    out.into_iter()
        .map(|(r, layers)| {
            Some(DiskLayers { files: r.paths()?, nodata: r.layers.iter().map(|l| l.nodata).collect(), layers })
        })
        .collect()
}

/// The columns of the groups that can run elsewhere (spectral, shape, texture), by group in the order of the groups,
/// worked out by `runner`; `None` for a group not offloaded. The whole is `None` when the layers are not all rasters
/// on disk another machine can open, or the runner takes none.
pub(super) fn offload(
    runner: &dyn super::TileCores,
    input: &Input,
    chosen: &[Vec<(&'static str, bool)>],
) -> Result<Option<Vec<Option<FeatureColumns>>>> {
    let Some(strips) = &input.strips else { return Ok(None) };
    let Some(sources) = sources_of(&strips.parts) else { return Ok(None) };
    let mut tasks = Vec::new();
    let mut targets = Vec::new();
    for (g, want) in chosen.iter().enumerate().take(3) {
        if want.is_empty() {
            continue;
        }
        if g == 2 {
            let (t, _) = texture_layout(input, want)?;
            for l in &t {
                let band = l.map_or("brightness".to_string(), |j| input.names[j].clone());
                tasks.push(FeatureTask { group: g, band: Some(band) });
            }
            targets = t;
        } else {
            tasks.push(FeatureTask { group: g, band: None });
        }
    }
    let mut out: Vec<Option<FeatureColumns>> = vec![None; GROUPS.len()];
    if tasks.is_empty() {
        return Ok(Some(out));
    }
    let labels = LabelsFile::write(&input.o.labels)?;
    let r = input.roles;
    let job = FeatureJob {
        node: input.node.to_string(),
        sources,
        labels: labels.0.clone(),
        grid: (*input.o.grid).clone(),
        count: input.o.count,
        names: input.names.clone(),
        image: input.image,
        roles: [r.red, r.nir, r.green, r.swir],
        levels: input.levels,
        chosen: chosen.iter().map(|g| g.iter().map(|(n, named)| (n.to_string(), *named)).collect()).collect(),
        tasks,
    };
    let Some(got) = runner.features(&job)? else { return Ok(None) };
    if got.len() != job.tasks.len() {
        return Err(Error::msg(format!("the cluster gave {} tasks of {}", got.len(), job.tasks.len())));
    }
    if got.iter().flatten().any(|(_, v)| v.len() != input.o.count) {
        return Err(Error::msg("the cluster gave a column of the wrong length"));
    }
    let mut texture: HashMap<String, Vec<f64>> = HashMap::new();
    for (task, cols) in job.tasks.iter().zip(got) {
        if task.group == 2 {
            texture.extend(cols);
        } else {
            out[task.group] = Some(cols);
        }
    }
    if !chosen[2].is_empty() {
        // the group's order: per statistic, each layer
        let (_, on) = texture_layout(input, &chosen[2])?;
        let mut cols = Vec::new();
        for (f, stat) in GLCM.iter().enumerate() {
            if !on[f] {
                continue;
            }
            for t in &targets {
                let name = glcm_column(stat, *t, &input.names);
                let v = texture.remove(&name).ok_or_else(|| Error::msg(format!("the cluster gave no {name}")))?;
                cols.push((name, v));
            }
        }
        out[2] = Some(cols);
    }
    Ok(Some(out))
}
