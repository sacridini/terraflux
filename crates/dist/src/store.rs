//! Blobs on disk, by their SHA-256: a coordinator's data and results, a worker's cache. Files are written to a
//! file of their own and renamed when whole and checked, so several writers (workers sharing a cache) and a
//! stop half way never leave a damaged blob.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::proto::Chunk;
use crate::{Error, Result};

/// The size of the pieces files travel in.
pub const CHUNK: usize = 1 << 20;

/// The SHA-256 of a file, in hexadecimal, read in pieces.
pub fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut f = std::fs::File::open(path).map_err(|e| Error::Other(format!("{}: {e}", path.display())))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        size += n as u64;
    }
    Ok((hex::encode(h.finalize()), size))
}

/// A folder of blobs (`<hash>` files).
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

/// A blob being written: to a file of its own, hashed as it comes.
pub struct Writing {
    file: Option<std::fs::File>,
    tmp: PathBuf,
    hasher: Sha256,
}

impl Store {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Store> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Store { dir })
    }

    pub fn path(&self, hash: &str) -> PathBuf {
        self.dir.join(hash)
    }

    pub fn has(&self, hash: &str) -> bool {
        valid(hash) && self.path(hash).is_file()
    }

    pub fn remove(&self, hash: &str) {
        if valid(hash) {
            let _ = std::fs::remove_file(self.path(hash));
        }
    }

    /// Start writing a blob.
    pub fn writer(&self) -> Result<Writing> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let tmp = self.dir.join(format!(".part-{}-{n}", std::process::id()));
        Ok(Writing { file: Some(std::fs::File::create(&tmp)?), tmp, hasher: Sha256::new() })
    }

    /// Keep bytes as a blob: its hash.
    pub fn put(&self, data: &[u8]) -> Result<String> {
        let mut w = self.writer()?;
        w.write(data)?;
        w.finish(self, None)
    }
}

impl Writing {
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.hasher.update(data);
        self.file.as_mut().ok_or_else(|| Error::Other("a blob written after its end".into()))?.write_all(data)?;
        Ok(())
    }

    /// The blob whole: checked against the hash it was sent as (if any) and named by its hash.
    pub fn finish(mut self, store: &Store, expected: Option<&str>) -> Result<String> {
        drop(self.file.take());
        let hash = hex::encode(std::mem::take(&mut self.hasher).finalize());
        let tmp = self.tmp.clone();
        if let Some(e) = expected.filter(|e| !e.is_empty())
            && e != hash
        {
            let _ = std::fs::remove_file(&tmp);
            return Err(Error::Other(format!("the blob {e} arrived damaged")));
        }
        let path = store.path(&hash);
        if let Err(e) = std::fs::rename(&tmp, &path) {
            let _ = std::fs::remove_file(&tmp);
            // another writer of the same blob was first
            if !path.is_file() {
                return Err(e.into());
            }
        }
        Ok(hash)
    }
}

impl Drop for Writing {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.tmp);
    }
}

/// A hash, not a path.
fn valid(hash: &str) -> bool {
    !hash.is_empty() && hash.len() <= 128 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The pieces of a file, read in a thread: the first says the hash and the size.
pub fn file_chunks(path: PathBuf, hash: String, size: u64) -> ReceiverStream<Chunk> {
    let (tx, rx) = mpsc::channel(4);
    tokio::task::spawn_blocking(move || {
        let Ok(mut f) = std::fs::File::open(&path) else { return };
        let mut first = true;
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = match f.read(&mut buf) {
                Ok(n) => n,
                Err(_) => return,
            };
            if n == 0 && !first {
                return;
            }
            let c = Chunk {
                hash: if first { hash.clone() } else { String::new() },
                data: buf[..n].to_vec(),
                size: if first { size } else { 0 },
            };
            first = false;
            if tx.blocking_send(c).is_err() || n == 0 {
                return;
            }
        }
    });
    ReceiverStream::new(rx)
}

/// The pieces of bytes in memory.
pub fn bytes_chunks(data: Vec<u8>, hash: String) -> impl tokio_stream::Stream<Item = Chunk> + Send + 'static {
    let size = data.len() as u64;
    let mut pieces: Vec<Chunk> =
        data.chunks(CHUNK).map(|c| Chunk { hash: String::new(), data: c.to_vec(), size: 0 }).collect();
    if pieces.is_empty() {
        pieces.push(Chunk::default());
    }
    pieces[0].hash = hash;
    pieces[0].size = size;
    tokio_stream::iter(pieces)
}
