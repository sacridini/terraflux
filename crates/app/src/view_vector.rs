//! Vector features on the web read by view. A remote layer (`https://...`, `/vsicurl/...`, `WFS:`, `OAPIF:`, a bucket)
//! drawn over a map in the Viewer is not read whole: the features that meet the view are, by a job in a thread per
//! (layer, the view's box rounded outward to a coarse grid), so that a pan inside the same boxes asks for nothing and a
//! server with a spatial filter (a WFS, an OGC API, a FlatGeobuf with its index) sends only what is seen. The last few
//! boxes are kept; while the view's own is being read, the last one read is drawn. A layer whose node has run (it
//! read the whole layer) is drawn from its value as any other, and a local file is always read whole.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use terraflux_graph::Node;
use terraflux_raster::vector::Vector;

/// The boxes kept for each layer.
pub const KEPT: usize = 6;

/// The most reads going on at once.
const AT_ONCE: usize = 2;

/// The layer a node reads when it is one on the web read by view: its path (as GDAL opens it) and layer. None for a
/// local file, a node that is not `vector.read`, or one that has an `extent` of its own (it reads that, whole).
pub fn source_of(node: &Node) -> Option<(String, Option<String>)> {
    if node.op != "vector.read" {
        return None;
    }
    let path = node.params.get("path")?.as_str()?.trim();
    if path.is_empty() || !terraflux_graph::is_remote(path) {
        return None;
    }
    match node.params.get("extent") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Array(a)) if a.is_empty() => {}
        Some(_) => return None,
    }
    let layer = node.params.get("layer").and_then(|l| l.as_str()).filter(|l| !l.is_empty()).map(String::from);
    Some((terraflux_graph::remote_path(path).unwrap_or_else(|| path.to_string()), layer))
}

/// The box of the view `[xmin, ymin, xmax, ymax]` rounded outward to a grid whose step is the power of two that goes
/// two to four times in the larger side: the same box for the views a few steps apart, and one that holds the view.
pub fn rounded(view: [f64; 4]) -> Option<[f64; 4]> {
    let (w, h) = (view[2] - view[0], view[3] - view[1]);
    let side = w.max(h);
    if !view.iter().all(|v| v.is_finite()) || w < 0.0 || h < 0.0 || side <= 0.0 {
        return None;
    }
    let step = 2f64.powi((side / 2.0).log2().floor() as i32);
    Some([
        (view[0] / step).floor() * step,
        (view[1] / step).floor() * step,
        (view[2] / step).ceil() * step,
        (view[3] / step).ceil() * step,
    ])
}

/// What a read is for: the layer, the box's system and the box (its numbers' bits).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    path: String,
    layer: Option<String>,
    crs: Option<String>,
    bbox: [u64; 4],
}

impl Key {
    fn same_layer(&self, o: &Key) -> bool {
        self.path == o.path && self.layer == o.layer
    }
}

enum State {
    Reading(Receiver<Result<Vector, String>>),
    Done(Result<Arc<Vector>, String>),
}

/// The layers on the web read by view: what is being read and what was.
#[derive(Default)]
pub struct ViewVectors {
    /// The reads, the least recently used first.
    entries: Vec<(Key, State)>,
    /// The last features read of each layer, drawn while the view's box is read.
    last: HashMap<(String, Option<String>), Arc<Vector>>,
    /// Why reads failed, told once.
    errors: Vec<String>,
    /// How many reads were started (the server's requests, as far as this side knows).
    started: usize,
}

impl ViewVectors {
    /// The features of a layer for a view (`[xmin, ymin, xmax, ymax]` in the system `crs`, none: the layer's): those of
    /// the view's box when read, else the last read of the layer (none before one). Starts the read in a thread when
    /// the box was not asked for (the thread asks `ctx` for a repaint when done).
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        source: &(String, Option<String>),
        view: &([f64; 4], Option<String>),
    ) -> Option<Arc<Vector>> {
        self.poll();
        let layer = (source.0.clone(), source.1.clone());
        let Some(bbox) = rounded(view.0) else { return self.last.get(&layer).cloned() };
        let key =
            Key { path: source.0.clone(), layer: source.1.clone(), crs: view.1.clone(), bbox: bbox.map(f64::to_bits) };
        match self.entries.iter().position(|(k, _)| *k == key) {
            // (used: the last to go)
            Some(i) => {
                let e = self.entries.remove(i);
                self.entries.push(e);
            }
            None if self.reading() < AT_ONCE => {
                let (tx, rx) = channel();
                let (path, name, crs, ctx) = (source.0.clone(), source.1.clone(), view.1.clone(), ctx.clone());
                let spawned = std::thread::Builder::new().name("terraflux-vector-view".into()).spawn(move || {
                    let r = terraflux_gdal::read_vector_in(&path, name.as_deref(), bbox, crs.as_deref());
                    let _ = tx.send(r);
                    ctx.request_repaint();
                });
                if spawned.is_ok() {
                    self.started += 1;
                    self.entries.push((key.clone(), State::Reading(rx)));
                    self.forget_old(&key);
                }
            }
            None => {}
        }
        match self.entries.iter().find(|(k, _)| *k == key) {
            Some((_, State::Done(Ok(v)))) => {
                self.last.insert(layer, v.clone());
                Some(v.clone())
            }
            _ => self.last.get(&layer).cloned(),
        }
    }

    /// The reads that came are kept.
    pub fn poll(&mut self) {
        for (_, state) in &mut self.entries {
            if let State::Reading(rx) = state
                && let Ok(r) = rx.try_recv()
            {
                *state = State::Done(r.map(Arc::new));
                if let State::Done(Err(e)) = state {
                    self.errors.push(e.clone());
                }
            }
        }
    }

    /// Reads going on.
    pub fn reading(&self) -> usize {
        self.entries.iter().filter(|(_, s)| matches!(s, State::Reading(_))).count()
    }

    /// How many reads were started in all.
    pub fn started(&self) -> usize {
        self.started
    }

    /// The boxes kept (read, or being) of a layer.
    pub fn kept(&self, source: &(String, Option<String>)) -> usize {
        self.entries.iter().filter(|(k, _)| k.path == source.0 && k.layer == source.1).count()
    }

    /// The features of a layer drawn now: the last read of it (none before one).
    pub fn shown(&self, source: &(String, Option<String>)) -> Option<Arc<Vector>> {
        self.last.get(source).cloned()
    }

    /// Why a read failed, once (the layer's own message).
    pub fn take_error(&mut self) -> Option<String> {
        (!self.errors.is_empty()).then(|| self.errors.remove(0))
    }

    /// The oldest boxes of the layer of `key` let go beyond [`KEPT`] (those being read stay).
    fn forget_old(&mut self, key: &Key) {
        while self.entries.iter().filter(|(k, _)| k.same_layer(key)).count() > KEPT {
            let Some(i) = self.entries.iter().position(|(k, s)| k.same_layer(key) && matches!(s, State::Done(_)))
            else {
                return;
            };
            self.entries.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_is_rounded_outward_to_a_coarse_grid() {
        let b = rounded([10.3, 20.1, 14.9, 23.2]).unwrap();
        // the larger side is 4.6: a step of 2
        assert_eq!(b, [10.0, 20.0, 16.0, 24.0]);
        assert!(b[0] <= 10.3 && b[1] <= 20.1 && b[2] >= 14.9 && b[3] >= 23.2);
        // a view a little to the side is the same box
        assert_eq!(rounded([10.5, 20.3, 15.1, 23.4]).unwrap(), b);
        // far enough: another one
        assert_ne!(rounded([13.0, 20.1, 17.6, 23.2]).unwrap(), b);
        // in metres, and the step is at most half the larger side
        let m = rounded([500_100.0, 6_000_000.0, 503_000.0, 6_002_000.0]).unwrap();
        assert!(m[0] <= 500_100.0 && m[2] >= 503_000.0 && (m[2] - m[0]) <= 3.0 * 2900.0 + 2048.0, "{m:?}");
        for bad in [[1.0, 1.0, 1.0, 1.0], [f64::NAN, 0.0, 1.0, 1.0], [2.0, 0.0, 1.0, 1.0]] {
            assert_eq!(rounded(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn only_a_layer_on_the_web_without_an_extent_is_read_by_view() {
        let node = |path: &str| Node::new("v", "vector.read").param("path", path);
        let by = |n: &Node| source_of(n).map(|(p, _)| p);
        assert_eq!(by(&node("https://host.org/a.fgb")).as_deref(), Some("/vsicurl/https://host.org/a.fgb"));
        assert_eq!(by(&node("WFS:https://host.org/wfs")).as_deref(), Some("WFS:https://host.org/wfs"));
        assert_eq!(
            by(&node("OAPIF:https://host.org/collections")).as_deref(),
            Some("OAPIF:https://host.org/collections")
        );
        assert_eq!(by(&node("/vsis3/bucket/a.fgb")).as_deref(), Some("/vsis3/bucket/a.fgb"));
        assert_eq!(by(&node("data/a.gpkg")), None, "a local file is read whole");
        assert_eq!(by(&node("")), None);
        assert_eq!(by(&Node::new("r", "raster.read").param("path", "https://host.org/a.tif")), None);
        let with = node("https://host.org/a.fgb").param("extent", serde_json::json!([0, 0, 1, 1]));
        assert_eq!(by(&with), None, "it reads its own extent");
        let layer = node("https://host.org/a.gpkg").param("layer", "roads");
        assert_eq!(source_of(&layer).unwrap().1.as_deref(), Some("roads"));
    }
}
