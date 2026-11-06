//! The last session reopened, and each model's view: the models open when Terraflux closed (their tabs, the one in
//! front) in `last-session.json`; of each model by its file, in `views.json` (the last [`KEPT`]), where the canvas
//! looked (the container open, the camera), the node selected, what the Viewer showed (the same Plot that *Update
//! the plot* keeps, never written in the model) and the pane in front. A model opened (by *Open recent* too) gets its
//! view back; what the Viewer showed runs the quiet way (as a double click does), never the whole model.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::{Pane, Session};

/// The views kept, of the models opened last.
pub const KEPT: usize = 100;

/// The models open when Terraflux closed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Last {
    pub models: Vec<PathBuf>,
    pub front: usize,
}

/// The view of a model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// The model's file (absolute).
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// The canvas's camera on the scope: where it is moved to (pixels) and the zoom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<([f32; 2], f32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<terraflux_graph::Plot>,
    /// The pane in front, by its title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Views {
    /// The last kept last.
    views: Vec<View>,
}

fn key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).or_else(|_| std::path::absolute(path)).unwrap_or_else(|_| path.to_path_buf())
}

fn read<T: for<'a> Deserialize<'a> + Default>(file: &Path) -> T {
    std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write<T: Serialize>(file: &Path, v: &T) {
    if let Some(d) = file.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Ok(t) = serde_json::to_string_pretty(v) {
        // (written aside, then put in place: never half a file)
        let tmp = file.with_extension("json.tmp");
        if std::fs::write(&tmp, t).is_ok() {
            let _ = std::fs::rename(&tmp, file);
        }
    }
}

/// The files, in a folder (the application's data in the application; another in the tests).
#[derive(Debug, Clone)]
pub struct Store {
    pub dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Store {
        Store { dir }
    }

    fn last_file(&self) -> PathBuf {
        self.dir.join("last-session.json")
    }

    fn views_file(&self) -> PathBuf {
        self.dir.join("views.json")
    }

    /// The models open when Terraflux closed (those whose file is gone left out).
    pub fn last(&self) -> Last {
        let mut l: Last = read(&self.last_file());
        let front = l.models.get(l.front).cloned();
        l.models.retain(|p| p.is_file());
        l.front = front.and_then(|f| l.models.iter().position(|p| *p == f)).unwrap_or(0);
        l
    }

    pub fn keep_last(&self, l: &Last) {
        write(&self.last_file(), l);
    }

    /// The view of a model, when one was kept.
    pub fn view_of(&self, path: &Path) -> Option<View> {
        let k = key(path);
        let all: Views = read(&self.views_file());
        all.views.into_iter().rev().find(|v| v.path == k)
    }

    /// Views kept (each the last of its model; the oldest dropped past [`KEPT`]).
    pub fn keep_views(&self, new: Vec<View>) {
        if new.is_empty() {
            return;
        }
        let mut all: Views = read(&self.views_file());
        for mut v in new {
            v.path = key(&v.path);
            all.views.retain(|w| w.path != v.path);
            all.views.push(v);
        }
        let over = all.views.len().saturating_sub(KEPT);
        all.views.drain(..over);
        write(&self.views_file(), &all);
    }
}

/// The view of a model with a file now (`pane`: the one in front, for the model in front).
pub fn view(s: &mut Session, pane: Option<Pane>) -> Option<View> {
    let path = s.path.clone()?;
    Some(View {
        path,
        scope: s.editor.path.clone(),
        camera: s.editor.camera().map(|c| ([c.pan.x, c.pan.y], c.zoom)),
        selected: s.selected.clone(),
        viewer: crate::builder::view_plot(s),
        pane: pane.map(|p| p.title().to_string()),
    })
}

/// A model's view put back: the container open (when it is there), the camera, the node selected, what the Viewer
/// showed (its nodes run the quiet way once the data is read); the pane that was in front.
pub fn apply(s: &mut Session, r: &terraflux_ops::Registry, v: &View) -> Option<Pane> {
    if !v.scope.is_empty() && s.source.scope(&v.scope).is_some() {
        s.editor.path = v.scope.clone();
    }
    if let Some((pan, zoom)) = v.camera {
        s.editor.set_camera(egui::Vec2::new(pan[0], pan[1]), zoom);
    }
    if let Some(p) = &v.viewer {
        let tops: Vec<String> = p
            .layers
            .iter()
            .filter_map(|l| match terraflux_graph::Ref::parse(&l.from) {
                Ok(terraflux_graph::Ref::Port { node, .. }) => Some(node),
                _ => None,
            })
            .filter(|n| s.source.nodes.iter().any(|x| &x.id == n))
            .collect();
        if !tops.is_empty() {
            crate::builder::show_plot(s, r, p);
            s.plot_open = None;
            s.ask_viewer = false;
            for n in tops {
                if !s.show_queue.contains(&n) {
                    s.show_queue.push(n);
                }
            }
        }
    }
    // (after the Viewer: its Plot selects the node it shows)
    if let Some(id) = &v.selected
        && s.source.scope(&s.editor.path).is_some_and(|n| n.iter().any(|n| &n.id == id))
    {
        s.selected = Some(id.clone());
    }
    v.pane.as_deref().and_then(|t| Pane::ALL.into_iter().find(|p| p.title() == t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join("terraflux-restore-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_views_and_the_last_session_are_kept() {
        let d = tmp("kept");
        let store = Store::new(d.join("data"));
        let (a, b) = (d.join("a.toml"), d.join("b.toml"));
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        assert_eq!(store.last(), Last::default());
        assert!(store.view_of(&a).is_none());
        let v = View {
            path: a.clone(),
            selected: Some("land".into()),
            camera: Some(([10.0, -4.0], 1.5)),
            ..Default::default()
        };
        store.keep_views(vec![v.clone()]);
        let back = store.view_of(&a).unwrap();
        assert_eq!((back.selected, back.camera), (v.selected.clone(), v.camera));
        // kept again: the last one only
        store.keep_views(vec![View { path: a.clone(), selected: Some("roads".into()), ..Default::default() }]);
        assert_eq!(store.view_of(&a).unwrap().selected.as_deref(), Some("roads"));
        // past KEPT models, the oldest dropped
        let many: Vec<View> =
            (0..KEPT).map(|k| View { path: d.join(format!("m{k}.toml")), ..Default::default() }).collect();
        store.keep_views(many);
        assert!(store.view_of(&a).is_none(), "the oldest dropped");
        // the session: a file gone left out, the front following
        store.keep_last(&Last { models: vec![d.join("gone.toml"), a.clone(), b.clone()], front: 2 });
        assert_eq!(store.last(), Last { models: vec![a, b], front: 1 });
    }
}
