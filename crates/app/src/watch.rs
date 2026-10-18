//! The model's file followed: when another program writes it (a text editor, an assistant through `terraflux
//! mcp`), the model is read again, as an edit of its own that ctrl+Z undoes. With edits not saved in the
//! application, it asks first.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use terraflux_graph::Model;

use crate::app::Session;

/// How often the file is looked at.
const EVERY: Duration = Duration::from_millis(300);

/// What tells a file's versions apart: when it was written last, and its size.
type Stamp = Option<(SystemTime, u64)>;

fn stamp(path: &Path) -> Stamp {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// A thread looking at a file: it says when the file changed (and wakes the application up).
pub struct Watch {
    pub path: PathBuf,
    changed: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl Watch {
    /// (changed at first: the file is looked at once, for what was written to it since the model was read)
    pub fn start(path: &Path, wake: impl Fn() + Send + 'static) -> Watch {
        let changed = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, s, p) = (changed.clone(), stop.clone(), path.to_path_buf());
        let mut last = stamp(&p);
        std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                std::thread::sleep(EVERY);
                let now = stamp(&p);
                if now != last && now.is_some() {
                    last = now;
                    c.store(true, Ordering::Relaxed);
                    wake();
                }
            }
        });
        Watch { path: path.to_path_buf(), changed, stop }
    }

    /// Whether the file changed since this was asked last.
    pub fn changed(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Once a frame: the file of the model followed, and what another program wrote to it taken (or kept to be asked
/// about, when the model has edits not saved: [`Session::on_disk`]).
pub fn follow(s: &mut Session, ctx: &egui::Context) {
    let Some(path) = s.path.clone() else {
        s.watch = None;
        return;
    };
    if s.watch.as_ref().is_none_or(|w| w.path != path) {
        let c = ctx.clone();
        s.watch = Some(Watch::start(&path, move || c.request_repaint()));
        return;
    }
    if !s.watch.as_ref().is_some_and(Watch::changed) {
        return;
    }
    let m = match Model::load(&path) {
        Ok(m) => m,
        Err(e) => {
            s.notice = Some((format!("{} changed but does not read: {e}", path.display()), std::time::Instant::now()));
            return;
        }
    };
    // (what the application saved itself, or the same model written again)
    if m == s.source {
        s.on_disk = None;
        s.saved = Some(m);
        return;
    }
    if !s.dirty {
        take(s, m);
    } else if s.saved.as_ref().is_some_and(|saved| unplaced(saved) == unplaced(&s.source)) {
        // only nodes moved here: the file's model, in the places they were moved to
        let mut m = m;
        keep_places(&s.source.nodes, &mut m.nodes);
        take(s, m);
    } else {
        s.on_disk = Some(m);
    }
}

/// A model without the places (and widths) of its nodes.
fn unplaced(m: &Model) -> Model {
    fn clear(nodes: &mut [terraflux_graph::Node]) {
        for n in nodes {
            n.pos = None;
            n.width = None;
            if let Some(b) = &mut n.body {
                clear(&mut b.nodes);
            }
        }
    }
    let mut m = m.clone();
    clear(&mut m.nodes);
    m
}

/// The nodes of `into` where the nodes of the same id are in `from`, scope by scope.
fn keep_places(from: &[terraflux_graph::Node], into: &mut [terraflux_graph::Node]) {
    for n in into {
        let Some(f) = from.iter().find(|f| f.id == n.id) else { continue };
        if f.pos.is_some() {
            n.pos = f.pos;
        }
        if f.width.is_some() {
            n.width = f.width;
        }
        if let (Some(fb), Some(b)) = (&f.body, &mut n.body) {
            keep_places(&fb.nodes, &mut b.nodes);
        }
    }
}

/// The model as its file has it now, in place of the one being edited (ctrl+Z goes back to that).
pub fn take(s: &mut Session, m: Model) {
    let inputs = m.inputs != s.source.inputs;
    let saved = s.path.as_ref().and_then(|p| Model::load(p).ok());
    s.on_disk = None;
    // what another program changed (an assistant through terraflux mcp): to review
    crate::review::begin(s, "another program (an assistant through terraflux mcp, an editor)", true);
    s.source = m;
    crate::builder::forget_missing(s);
    s.edited();
    // (the places moved here kept: still to be saved)
    s.dirty = saved.as_ref() != Some(&s.source);
    s.saved = saved;
    if inputs {
        s.read_data();
    }
    let name = s.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    s.notice = Some((
        format!("{} changed on disk: read again (ctrl+Z goes back)", name.unwrap_or_else(|| "the model".into())),
        std::time::Instant::now(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_of_the_file_is_seen() {
        let dir = std::env::temp_dir().join(format!("terraflux-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.toml");
        Model::new("a").save(&path).unwrap();
        let w = Watch::start(&path, || {});
        assert!(w.changed(), "looked at once at first");
        std::thread::sleep(EVERY * 2);
        assert!(!w.changed());
        Model::new("a longer name").save(&path).unwrap();
        let t = std::time::Instant::now();
        while !w.changed() {
            assert!(t.elapsed() < Duration::from_secs(5), "the change was not seen");
            std::thread::sleep(EVERY / 3);
        }
        drop(w);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `follow` until the session's model is taken from the file, or kept to be asked about.
    fn until_seen(s: &mut Session, ctx: &egui::Context, seen: impl Fn(&Session) -> bool) {
        let t = std::time::Instant::now();
        while !seen(s) {
            assert!(t.elapsed() < Duration::from_secs(5), "the file's change was not taken");
            follow(s, ctx);
            std::thread::sleep(EVERY / 3);
        }
    }

    #[test]
    fn the_file_written_by_another_program_is_taken() {
        use terraflux_graph::Node;
        let dir = std::env::temp_dir().join(format!("terraflux-follow-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.toml");
        Model::new("m").save(&path).unwrap();
        let ctx = egui::Context::default();
        let mut s = Session::open(&path).unwrap();
        follow(&mut s, &ctx);
        // no edits here: taken, an edit that ctrl+Z undoes
        let mut b = Model::new("m");
        b.nodes.push(Node::new("land", "nlm.random").param("rows", 10).param("columns", 10));
        b.nodes[0].pos = Some([0.0, 0.0]);
        b.save(&path).unwrap();
        until_seen(&mut s, &ctx, |s| s.source.nodes.len() == 1);
        assert!(!s.dirty && s.history.can_undo());
        // a node moved here, then another added there: the file's model, the node where it was moved
        s.source.nodes[0].pos = Some([50.0, 60.0]);
        s.edited();
        let mut c = b.clone();
        c.nodes.push(Node::new("other", "nlm.random"));
        c.save(&path).unwrap();
        until_seen(&mut s, &ctx, |s| s.source.nodes.len() == 2);
        assert_eq!(s.source.nodes[0].pos, Some([50.0, 60.0]));
        assert!(s.dirty, "the place moved to is still to be saved");
        // an edit here not saved, and the file changed: asked about, the model here kept
        crate::editor::set_note(&mut s.source, &[], "other", "mine").unwrap();
        s.edited();
        let mut d = c.clone();
        d.nodes.pop();
        d.save(&path).unwrap();
        until_seen(&mut s, &ctx, |s| s.on_disk.is_some());
        assert_eq!(s.source.nodes.len(), 2);
        let on_disk = s.on_disk.clone().unwrap();
        take(&mut s, on_disk);
        assert_eq!(s.source.nodes.len(), 1);
        assert!(s.on_disk.is_none());
        // saved here: nothing to take
        s.save().unwrap();
        follow(&mut s, &ctx);
        std::thread::sleep(EVERY * 2);
        follow(&mut s, &ctx);
        assert!(s.on_disk.is_none() && !s.dirty);
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
