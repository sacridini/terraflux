//! Edits not saved kept from a crash: while a model has edits not saved, a copy of it is written now and then in the
//! user's application data (`Terraflux/recovery/`), with a note beside it (the model's file, its name, when). While
//! the application runs it touches the notes of its copies (the pulse); a copy whose pulse stopped is of a Terraflux
//! that ended without saving (a crash, the power gone), offered to be recovered when the application opens. A copy is
//! deleted when its model is saved, or closed without saving by the user's word. Only files and their times: the same
//! on Windows, Linux and macOS.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use terraflux_graph::Model;

/// How often a model with edits not saved is copied (when it changed), and the pulse given.
pub const EVERY: Duration = Duration::from_secs(30);
const PULSE: Duration = Duration::from_secs(15);
/// A copy whose pulse is older than this is of a Terraflux no longer running.
const STOPPED: Duration = Duration::from_secs(60);

/// The note beside a copy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// The model's file (none: never saved), its name, when it was copied (seconds since 1970), the Terraflux that
    /// copied it and its process.
    pub path: Option<PathBuf>,
    pub title: String,
    pub copied: u64,
    pub version: String,
    pub pid: u32,
}

/// A copy left by a Terraflux that ended without saving.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub key: String,
    pub note: Note,
}

/// Where the copies are.
pub fn dir() -> PathBuf {
    crate::settings::data_dir().join("recovery")
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The name of a model's copy: its file's (hashed, FNV-1a), else a model never saved of this process, with the
/// process (two Terraflux with the same model open keep a copy each).
pub fn key_of(path: Option<&Path>, untitled: u64) -> String {
    let pid = std::process::id();
    match path {
        Some(p) => {
            let p = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in p.to_string_lossy().as_bytes() {
                h = (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
            }
            format!("{h:016x}-{pid}")
        }
        None => format!("untitled-{untitled}-{pid}"),
    }
}

/// A number for a model never saved, other than those of the other models of this process.
pub fn next_untitled() -> u64 {
    static N: AtomicU64 = AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Write a copy and its note (the note last: a copy is offered only with its note).
pub fn write(dir: &Path, key: &str, text: &str, note: &Note) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(format!("{key}.toml")), text)?;
    let json = serde_json::to_string_pretty(note).map_err(std::io::Error::other)?;
    std::fs::write(dir.join(format!("{key}.json")), json)
}

/// The pulse: the note touched (its time now).
pub fn beat(dir: &Path, key: &str) {
    if let Ok(f) = std::fs::File::options().write(true).open(dir.join(format!("{key}.json"))) {
        let _ = f.set_modified(SystemTime::now());
    }
}

/// A copy and its note deleted.
pub fn remove(dir: &Path, key: &str) {
    let _ = std::fs::remove_file(dir.join(format!("{key}.toml")));
    let _ = std::fs::remove_file(dir.join(format!("{key}.json")));
}

/// The copies of a Terraflux no longer running (their pulse stopped `stopped` ago or more; never this process's), the
/// last copied first.
pub fn left(dir: &Path, stopped: Duration) -> Vec<Found> {
    let mut all = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return all };
    let me = std::process::id();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let Some(key) = p.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
        let Ok(note) =
            std::fs::read_to_string(&p).map_err(|_| ()).and_then(|t| serde_json::from_str::<Note>(&t).map_err(|_| ()))
        else {
            continue;
        };
        let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).unwrap_or_default();
        if note.pid == me || age < stopped || !dir.join(format!("{key}.toml")).is_file() {
            continue;
        }
        all.push(Found { key, note });
    }
    all.sort_by_key(|f| std::cmp::Reverse(f.note.copied));
    all
}

/// The model of a copy.
pub fn read(dir: &Path, key: &str) -> Result<Model, String> {
    let text = std::fs::read_to_string(dir.join(format!("{key}.toml"))).map_err(|e| e.to_string())?;
    Model::from_toml(&text).map_err(|e| e.to_string())
}

/// How long ago a copy was made, in words.
pub fn ago(copied: u64) -> String {
    let s = now_secs().saturating_sub(copied);
    match s {
        0..60 => "less than a minute ago".into(),
        60..120 => "a minute ago".into(),
        120..3600 => format!("{} minutes ago", s / 60),
        3600..7200 => "an hour ago".into(),
        7200..172_800 => format!("{} hours ago", s / 3600),
        _ => format!("{} days ago", s / 86_400),
    }
}

/// The copies of the application's models: written when they change, their pulse given, and the copies left by a
/// Terraflux that ended without saving looked for now and then. Only the application's window has one (not the
/// tests, unless they ask: they would write in the user's data).
pub struct Keeper {
    pub dir: PathBuf,
    beat_at: Instant,
    looked_at: Option<Instant>,
    /// The copies of another Terraflux to offer, and those the user put off (Later) or answered.
    pub found: Vec<Found>,
    pub answered: Vec<String>,
}

impl Keeper {
    pub fn new(dir: PathBuf) -> Keeper {
        Keeper { dir, beat_at: Instant::now(), looked_at: None, found: Vec::new(), answered: Vec::new() }
    }

    /// Once a frame: a model's copy written (it has edits not saved that changed since its last copy, the last one
    /// `EVERY` ago or more) or deleted (saved since); the pulse; the copies left looked for (at the start, then every
    /// half a minute: one left by a crash a moment ago is not seen as stopped at once).
    pub fn tick(&mut self, sessions: &mut [&mut crate::app::Session]) {
        for s in sessions.iter_mut() {
            self.keep(s);
        }
        if self.beat_at.elapsed() >= PULSE {
            self.beat_at = Instant::now();
            for s in sessions.iter() {
                if let Some((key, _)) = &s.copy {
                    beat(&self.dir, key);
                }
            }
        }
        if self.looked_at.is_none_or(|t| t.elapsed() >= EVERY) {
            self.looked_at = Some(Instant::now());
            self.found = left(&self.dir, STOPPED).into_iter().filter(|f| !self.answered.contains(&f.key)).collect();
        }
    }

    fn keep(&self, s: &mut crate::app::Session) {
        let key = key_of(s.path.as_deref(), s.untitled);
        if !s.worth_keeping() {
            if let Some((k, _)) = s.copy.take() {
                remove(&self.dir, &k);
            }
            return;
        }
        let changed = s.copy.as_ref().is_none_or(|(k, rev)| *rev != s.revision || *k != key);
        if !changed || s.copied_at.is_some_and(|t| t.elapsed() < EVERY) {
            return;
        }
        let Ok(text) = s.source.to_toml() else { return };
        let note = Note {
            path: s.path.as_ref().map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone())),
            title: s.title(),
            copied: now_secs(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            pid: std::process::id(),
        };
        if let Some((old, _)) = s.copy.take().filter(|(k, _)| *k != key) {
            remove(&self.dir, &old);
        }
        s.copied_at = Some(Instant::now());
        s.copy = Some((key.clone(), s.revision));
        let dir = self.dir.clone();
        std::thread::spawn(move || {
            let _ = write(&dir, &key, &text, &note);
        });
    }

    /// A copy answered (recovered, discarded, put off): not offered again in this session.
    pub fn answer(&mut self, key: &str, discard: bool) {
        if discard {
            remove(&self.dir, key);
        }
        self.answered.push(key.to_string());
        self.found.retain(|f| f.key != key);
    }

    /// A model's copy deleted (its edits thrown away by the user's word, or the model saved).
    pub fn forget(&self, s: &mut crate::app::Session) {
        if let Some((k, _)) = s.copy.take() {
            remove(&self.dir, &k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join("terraflux-recovery-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn note(path: Option<&str>, pid: u32) -> Note {
        Note { path: path.map(PathBuf::from), title: "land".into(), copied: now_secs(), version: "1.0.0".into(), pid }
    }

    #[test]
    fn a_copy_is_written_and_read_back_with_its_file() {
        let d = tmp("written");
        let mut m = Model::new("land");
        m.nodes.push(terraflux_graph::Node::new("suit", "raster.calc"));
        let n = note(Some("/data/land.toml"), 1);
        write(&d, "k-1", &m.to_toml().unwrap(), &n).unwrap();
        assert_eq!(read(&d, "k-1").unwrap(), m);
        // (just written: its pulse is fresh, not left)
        assert!(left(&d, Duration::from_secs(60)).is_empty());
        let found = left(&d, Duration::ZERO);
        assert_eq!(found, vec![Found { key: "k-1".into(), note: n }]);
        remove(&d, "k-1");
        assert!(left(&d, Duration::ZERO).is_empty());
        assert!(std::fs::read_dir(&d).unwrap().next().is_none());
    }

    #[test]
    fn this_processs_copies_are_never_left() {
        let d = tmp("mine");
        write(&d, "mine", "format = \"terraflux/1\"", &note(None, std::process::id())).unwrap();
        assert!(left(&d, Duration::ZERO).is_empty());
    }

    #[test]
    fn the_pulse_keeps_a_copy_from_being_left() {
        let d = tmp("pulse");
        write(&d, "other", "", &note(None, 1)).unwrap();
        let old = SystemTime::now() - Duration::from_secs(600);
        std::fs::File::options().write(true).open(d.join("other.json")).unwrap().set_modified(old).unwrap();
        assert_eq!(left(&d, STOPPED).len(), 1, "its pulse stopped ten minutes ago");
        beat(&d, "other");
        assert!(left(&d, STOPPED).is_empty(), "a pulse just given");
    }

    #[test]
    fn keys_tell_files_and_models_never_saved_apart() {
        let a = key_of(Some(Path::new("/x/a.toml")), 1);
        assert_eq!(a, key_of(Some(Path::new("/x/a.toml")), 2));
        assert_ne!(a, key_of(Some(Path::new("/x/b.toml")), 1));
        assert_ne!(key_of(None, 1), key_of(None, 2));
        assert!(a.ends_with(&format!("-{}", std::process::id())));
    }

    #[test]
    fn how_long_ago_in_words() {
        let now = now_secs();
        assert_eq!(ago(now), "less than a minute ago");
        assert_eq!(ago(now - 600), "10 minutes ago");
        assert_eq!(ago(now - 3 * 3600), "3 hours ago");
        assert_eq!(ago(now - 3 * 86_400), "3 days ago");
    }
}
