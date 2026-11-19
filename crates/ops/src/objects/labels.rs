//! The labels of objects (a u32 a cell): in memory, or in a memory-mapped temporary file when they are too big for
//! memory. Either way they read as a slice of u32; a file is deleted when the labels are dropped.

use std::fs::OpenOptions;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use memmap2::MmapMut;

/// The bytes above which labels are kept in a memory-mapped file instead of memory (an eighth of what an image may
/// take by default, [`super::big::memory_limit`]; the tests set it to 0 to try the files on small images).
pub static LABELS_DISK_BYTES: AtomicU64 = AtomicU64::new(u64::MAX);

/// Whether labels of `n` cells go to disk.
fn on_disk(n: usize) -> bool {
    let limit = match LABELS_DISK_BYTES.load(Ordering::Relaxed) {
        u64::MAX => super::big::memory_limit() / 8,
        l => l,
    };
    n > 0 && n as u64 * 4 > limit
}

/// A temporary file of `n` u32 mapped into memory.
pub struct Mapped {
    map: Option<MmapMut>,
    len: usize,
    path: PathBuf,
}

impl Mapped {
    fn create(n: usize) -> std::io::Result<Mapped> {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "terraflux-labels-{}-{}.bin",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path)?;
        let made = file.set_len(n as u64 * 4).and_then(|_| {
            // SAFETY: the file is ours (a new name with the process's id), only this map reads and writes it
            unsafe { MmapMut::map_mut(&file) }
        });
        match made {
            Ok(map) => Ok(Mapped { map: Some(map), len: n, path }),
            Err(e) => {
                drop(file);
                std::fs::remove_file(&path).ok();
                Err(e)
            }
        }
    }
}

impl Drop for Mapped {
    fn drop(&mut self) {
        // unmapped first: Windows keeps a mapped file from being deleted
        self.map = None;
        std::fs::remove_file(&self.path).ok();
    }
}

/// Each cell's object: a slice of u32 in memory or in a mapped file.
pub enum Labels {
    Mem(Vec<u32>),
    Disk(Box<Mapped>),
}

impl Labels {
    /// `n` zeros, in memory or in a file by [`LABELS_DISK_BYTES`].
    pub fn zeros(n: usize) -> Labels {
        if on_disk(n) {
            // (a new file reads as zeros)
            if let Ok(m) = Mapped::create(n) {
                return Labels::Disk(Box::new(m));
            }
        }
        Labels::Mem(vec![0; n])
    }

    /// Whether the labels are in a file.
    pub fn is_mapped(&self) -> bool {
        matches!(self, Labels::Disk(_))
    }
}

impl From<Vec<u32>> for Labels {
    /// The labels as they are, moved to a file when they are over the threshold.
    fn from(v: Vec<u32>) -> Labels {
        if on_disk(v.len()) {
            let mut l = Labels::zeros(v.len());
            if l.is_mapped() {
                l.copy_from_slice(&v);
                return l;
            }
        }
        Labels::Mem(v)
    }
}

impl Deref for Labels {
    type Target = [u32];

    fn deref(&self) -> &[u32] {
        match self {
            Labels::Mem(v) => v,
            Labels::Disk(m) => match &m.map {
                // SAFETY: a map is page aligned and `len` u32 long (the file was made that size)
                Some(map) => unsafe { std::slice::from_raw_parts(map.as_ptr() as *const u32, m.len) },
                None => &[],
            },
        }
    }
}

impl DerefMut for Labels {
    fn deref_mut(&mut self) -> &mut [u32] {
        match self {
            Labels::Mem(v) => v,
            Labels::Disk(m) => {
                let len = m.len;
                match &mut m.map {
                    // SAFETY: as in `deref`, and the map is borrowed for writing
                    Some(map) => unsafe { std::slice::from_raw_parts_mut(map.as_mut_ptr() as *mut u32, len) },
                    None => &mut [],
                }
            }
        }
    }
}

impl Clone for Labels {
    fn clone(&self) -> Labels {
        Labels::from(self.to_vec())
    }
}

impl PartialEq for Labels {
    fn eq(&self, o: &Labels) -> bool {
        **self == **o
    }
}

impl PartialEq<Vec<u32>> for Labels {
    fn eq(&self, o: &Vec<u32>) -> bool {
        **self == **o
    }
}

impl std::fmt::Debug for Labels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Labels({} cells{})", self.len(), if self.is_mapped() { ", mapped" } else { "" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_in_a_file_read_write_and_vanish_with_the_value() {
        let mut l = Labels::Disk(Box::new(Mapped::create(1000).unwrap()));
        assert_eq!(l.len(), 1000);
        assert!(l.iter().all(|&x| x == 0));
        for (i, x) in l.iter_mut().enumerate() {
            *x = i as u32 * 3;
        }
        let c = l.clone();
        assert_eq!(c[999], 2997);
        assert_eq!(l, c);
        let Labels::Disk(m) = &l else { panic!("a file") };
        let path = m.path.clone();
        assert!(path.exists());
        drop(l);
        assert!(!path.exists());
    }
}
