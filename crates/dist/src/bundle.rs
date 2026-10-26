//! The files of a model sent to the workers: every path the model reads (its inputs, the files of
//! `raster.read` and `vector.read`, plugins, an observed map, the files a scenario gives) becomes a bundle, a
//! folder of files sent once by hash: the file and the files beside it of the same name (a shapefile's
//! `.shp`, `.shx`, `.dbf` and `.prj`, a raster's `.aux.xml`), the rasters of a folder, or those a pattern names
//! (`runs/*/landuse.tif`). The path becomes `bundle:<hash>/<the path in the folder>`; a worker makes the folder
//! once in its cache and reads the files there under their own names.

use std::path::{Component, Path, PathBuf};

use crate::net::Conn;
use crate::proto::{BlobId, Chunk};
use crate::store::{Store, bytes_chunks, file_chunks, hash_file};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// The files of a bundle: their paths in its folder (with `/`) and their hashes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub files: Vec<(String, String)>,
}

/// Send a file (unless the coordinator has it): its hash.
pub async fn send_file(c: &mut Conn, path: &Path) -> Result<String> {
    let p = path.to_path_buf();
    let (hash, size) =
        tokio::task::spawn_blocking(move || hash_file(&p)).await.map_err(|e| Error::Other(e.to_string()))??;
    if !c.has_blob(BlobId { hash: hash.clone() }).await?.into_inner().ok {
        c.put_blob(file_chunks(path.to_path_buf(), hash.clone(), size)).await?;
    }
    Ok(hash)
}

/// Send bytes (unless the coordinator has them): their hash.
pub async fn send_bytes(c: &mut Conn, data: Vec<u8>) -> Result<String> {
    let hash = crate::hash(&data);
    if !c.has_blob(BlobId { hash: hash.clone() }).await?.into_inner().ok {
        c.put_blob(bytes_chunks(data, hash.clone())).await?;
    }
    Ok(hash)
}

/// A blob in a store (fetched when it does not have it): its file.
pub async fn fetch(c: &mut Conn, store: &Store, hash: &str) -> Result<PathBuf> {
    if store.has(hash) {
        return Ok(store.path(hash));
    }
    let mut s = c.get_blob(BlobId { hash: hash.to_string() }).await?.into_inner();
    let mut w = store.writer()?;
    while let Some(Chunk { data, .. }) = s.message().await? {
        w.write(&data)?;
    }
    w.finish(store, Some(hash))?;
    Ok(store.path(hash))
}

/// A blob written to a file (whole, or not at all).
pub async fn fetch_to(c: &mut Conn, hash: &str, dest: &Path) -> Result<()> {
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d)?;
    }
    let store = Store::open(dest.parent().unwrap_or(Path::new(".")))?;
    let mut s = c.get_blob(BlobId { hash: hash.to_string() }).await?.into_inner();
    let mut w = store.writer()?;
    while let Some(Chunk { data, .. }) = s.message().await? {
        w.write(&data)?;
    }
    let got = w.finish(&store, Some(hash))?;
    std::fs::rename(store.path(&got), dest)?;
    Ok(())
}

/// The files beside a file of the same name (its stem followed by a dot): a shapefile's parts, a raster's
/// sidecars.
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

fn wild(c: &Component) -> bool {
    c.as_os_str().to_string_lossy().contains(['*', '?'])
}

/// What a path names, as files in a folder: the folder, the files (their paths in it) and the path in it.
pub fn gather(path: &Path) -> Result<(PathBuf, Vec<PathBuf>, String)> {
    let shown = path.display();
    if path.components().any(|c| wild(&c)) {
        // a pattern: the files it names, in the folder before its first pattern
        let base: PathBuf = path.components().take_while(|c| !wild(c)).collect();
        let files = terraflux_ops::files::files_of(&path.to_string_lossy()).map_err(|e| Error::Other(e.to_string()))?;
        let rel = path.strip_prefix(&base).map_err(|_| Error::Other(format!("{shown}: a pattern")))?;
        let all = files.iter().flat_map(|f| siblings(f)).collect();
        return Ok((base, all, slashes(rel)));
    }
    if path.is_dir() {
        let files = terraflux_ops::files::files_of(&path.to_string_lossy()).map_err(|e| Error::Other(e.to_string()))?;
        return Ok((path.to_path_buf(), files.iter().flat_map(|f| siblings(f)).collect(), String::new()));
    }
    if !path.is_file() {
        return Err(Error::Other(format!("{shown}: no such file")));
    }
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok((base, siblings(path), name))
}

fn slashes(p: &Path) -> String {
    p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}

/// Send what a path names (relative to `base`): `bundle:<hash>/<path in the bundle>`.
pub async fn send(c: &mut Conn, path: &str, base: &Path) -> Result<String> {
    if terraflux_graph::is_sent(path) {
        return Ok(path.to_string());
    }
    let p = Path::new(path);
    let full = if p.is_absolute() { p.to_path_buf() } else { base.join(p) };
    let (dir, files, rel) = gather(&full)?;
    let mut m = Manifest::default();
    for f in files {
        let name = f.strip_prefix(&dir).map(slashes).unwrap_or_else(|_| f.to_string_lossy().into_owned());
        if m.files.iter().any(|(n, _)| *n == name) {
            continue;
        }
        m.files.push((name, send_file(c, &f).await?));
    }
    let hash = send_bytes(c, serde_json::to_vec(&m)?).await?;
    Ok(if rel.is_empty() { format!("bundle:{hash}") } else { format!("bundle:{hash}/{rel}") })
}

/// The local path of a path sent (`bundle:<hash>/...`): the bundle's folder made once in the cache.
pub async fn local(c: &mut Conn, cache: &Path, path: &str) -> Result<String> {
    let Some(rest) = path.strip_prefix("bundle:") else { return Ok(path.to_string()) };
    let (hash, rel) = rest.split_once('/').unwrap_or((rest, ""));
    let store = Store::open(cache.join("blobs"))?;
    let dir = cache.join("bundles").join(hash);
    if !dir.join(".complete").exists() {
        let manifest: Manifest = serde_json::from_slice(&std::fs::read(fetch(c, &store, hash).await?)?)?;
        // made in a folder of its own, then renamed: workers sharing a cache make it once
        let tmp = cache.join("bundles").join(format!(".{hash}-{}-{}", std::process::id(), crate::unique()));
        for (name, h) in &manifest.files {
            let from = fetch(c, &store, h).await?;
            let to = tmp.join(name);
            if to.components().any(|c| matches!(c, Component::ParentDir)) {
                return Err(Error::Other(format!("a bundle's file outside it: {name}")));
            }
            std::fs::create_dir_all(to.parent().unwrap())?;
            if std::fs::hard_link(&from, &to).is_err() {
                std::fs::copy(&from, &to)?;
            }
        }
        std::fs::create_dir_all(&tmp)?;
        std::fs::write(tmp.join(".complete"), b"")?;
        // a folder without its mark is a leftover (the mark is written before the rename): a cleaner of temporary
        // files (Windows' Storage Sense) removes the empty mark of a bundle not used for a while
        if dir.exists() && !dir.join(".complete").exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        if std::fs::rename(&tmp, &dir).is_err() {
            let _ = std::fs::remove_dir_all(&tmp);
            if !dir.join(".complete").exists() {
                return Err(Error::Other(format!("the bundle {hash} could not be made")));
            }
        }
    }
    let local = if rel.is_empty() { dir } else { dir.join(rel) };
    Ok(local.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_brings_its_siblings_and_a_pattern_its_files() {
        let d = std::env::temp_dir().join(format!("terraflux-gather-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("runs/a")).unwrap();
        std::fs::create_dir_all(d.join("runs/b")).unwrap();
        for f in ["roads.shp", "roads.shx", "roads.dbf", "roads.prj", "roadside.shp", "runs/a/x.tif", "runs/b/x.tif"] {
            std::fs::write(d.join(f), f).unwrap();
        }
        let (base, files, rel) = gather(&d.join("roads.shp")).unwrap();
        assert_eq!(base, d);
        assert_eq!(rel, "roads.shp");
        let names: Vec<String> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["roads.dbf", "roads.prj", "roads.shp", "roads.shx"]);
        let (base, files, rel) = gather(&d.join("runs").join("*").join("x.tif")).unwrap();
        assert_eq!((base, files.len(), rel.as_str()), (d.join("runs"), 2, "*/x.tif"));
        assert!(gather(&d.join("none.tif")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
