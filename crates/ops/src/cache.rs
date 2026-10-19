//! The caches Terraflux keeps on disk (the rasters made without a path, the data downloaded), each held under a
//! size: what goes over it is removed, the least recently used first; whatever made it makes it again when asked.
//!
//! The size is the one set ([`set_limit`]: the application's settings), else `TERRAFLUX_CACHE_GB` (gigabytes; 0
//! for no limit), else [`DEFAULT_GB`].

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The size of each cache by default, in gigabytes.
pub const DEFAULT_GB: f64 = 10.0;

static SET: Mutex<Option<f64>> = Mutex::new(None);

/// The size of each cache in gigabytes (none: the default or `TERRAFLUX_CACHE_GB`; 0: no limit).
pub fn set_limit(gb: Option<f64>) {
    *SET.lock().unwrap_or_else(|e| e.into_inner()) = gb;
}

/// The size a cache is held under, in gigabytes (0: no limit).
pub fn limit_gb() -> f64 {
    let set = *SET.lock().unwrap_or_else(|e| e.into_inner());
    set.or_else(|| std::env::var("TERRAFLUX_CACHE_GB").ok().and_then(|v| v.trim().parse::<f64>().ok()))
        .unwrap_or(DEFAULT_GB)
}

/// The size a cache is held under, in bytes (none: no limit).
pub fn limit() -> Option<u64> {
    let gb = limit_gb();
    (gb > 0.0).then(|| (gb * 1e9) as u64)
}

/// The caches the application empties on demand: the rasters made without a path, the data downloaded.
pub fn emptied() -> Vec<PathBuf> {
    vec![root().join("prepare"), root().join("data")]
}

/// The root of Terraflux's caches: `TERRAFLUX_CACHE`, else the platform's ([`crate::sdm::refdata::cache_dir`]).
pub fn root() -> PathBuf {
    crate::sdm::refdata::cache_dir()
}

/// The bytes of a file or of a folder's files, and when it was used last (the newest of its times).
fn usage(p: &Path) -> (u64, SystemTime) {
    let Ok(m) = std::fs::symlink_metadata(p) else { return (0, SystemTime::UNIX_EPOCH) };
    let time = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    if !m.is_dir() {
        return (m.len(), time);
    }
    let (mut bytes, mut last) = (0, time);
    for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
        let (b, t) = usage(&e.path());
        bytes += b;
        last = last.max(t);
    }
    (bytes, last)
}

/// The bytes a cache holds.
pub fn size(dir: &Path) -> u64 {
    usage(dir).0
}

/// Mark a file or a folder of the cache as used now (it is kept over those used before it).
pub fn touch(p: &Path) {
    let now = SystemTime::now();
    let mark = |f: &Path| {
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(f) {
            let _ = file.set_modified(now);
        }
    };
    if p.is_dir() {
        for e in std::fs::read_dir(p).into_iter().flatten().flatten() {
            if e.path().is_file() {
                mark(&e.path());
            }
        }
    } else {
        mark(p);
    }
}

fn remove(p: &Path) {
    let _ = if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
}

/// Hold a cache (the files and folders in `dir`) under `limit` bytes: the least recently used removed first, never
/// those of `keep` (what a node just made); half-made files (`.part-`) of more than a day removed too. The bytes
/// freed.
pub fn prune(dir: &Path, limit: Option<u64>, keep: &[&Path]) -> u64 {
    prune_all(&[dir], limit, keep, std::time::Duration::ZERO)
}

/// [`prune`] over the entries of several folders together, removing only those unused for `idle` at least (what
/// another process may be reading now is left).
pub fn prune_all(dirs: &[&Path], limit: Option<u64>, keep: &[&Path], idle: std::time::Duration) -> u64 {
    let day = std::time::Duration::from_secs(24 * 3600);
    let mut entries: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
    let mut freed = 0;
    for e in dirs.iter().flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten()) {
        let p = e.path();
        let (bytes, last) = usage(&p);
        let name = e.file_name().to_string_lossy().into_owned();
        // half-made: `.part-` here, `.name-pid-n` of the workers' bundles
        let part = name.starts_with('.');
        if part && last.elapsed().is_ok_and(|age| age > day) {
            remove(&p);
            freed += bytes;
        } else if !part {
            entries.push((p, bytes, last));
        }
    }
    let Some(limit) = limit else { return freed };
    let mut total: u64 = entries.iter().map(|e| e.1).sum();
    entries.sort_by_key(|e| e.2);
    for (p, bytes, last) in entries {
        if total <= limit {
            break;
        }
        if keep.iter().any(|k| *k == p) || last.elapsed().map_or(true, |age| age < idle) {
            continue;
        }
        remove(&p);
        total -= bytes;
        freed += bytes;
    }
    freed
}

/// Empty a cache (the folder stays). The bytes freed.
pub fn clear(dir: &Path) -> u64 {
    let mut freed = 0;
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        freed += usage(&e.path()).0;
        remove(&e.path());
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_least_recently_used_go_first() {
        let d = std::env::temp_dir().join(format!("terraflux-cache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("folder")).unwrap();
        let old = SystemTime::now() - std::time::Duration::from_secs(3600);
        for (name, age) in [("a", 3), ("b", 2), ("folder/c", 1)] {
            let p = d.join(name);
            std::fs::write(&p, vec![0u8; 100]).unwrap();
            let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
            f.set_modified(old - std::time::Duration::from_secs(age * 60)).unwrap();
        }
        // a, the oldest, used now: b goes first, then the folder
        touch(&d.join("a"));
        assert_eq!(size(&d), 300);
        assert_eq!(prune(&d, Some(250), &[]), 100);
        assert!(!d.join("b").exists() && d.join("a").exists());
        assert_eq!(prune(&d, Some(100), &[&d.join("folder")]), 100);
        assert!(!d.join("a").exists() && d.join("folder").exists());
        assert_eq!(clear(&d), 100);
        assert_eq!(size(&d), 0);
        let _ = std::fs::remove_dir_all(&d);
    }
}
