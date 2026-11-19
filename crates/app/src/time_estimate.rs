//! How long a segmenter will take, told in the Node pane before it runs: from the rates of the benchmark battery
//! ([`terraflux_ops::objects::estimate`]) and the size of its image (the value of its input when that ran, else the grid
//! the memory estimate worked out from the file headers). When the image is big and the operator has tiles, *Use tiles*
//! writes `tile` as one step of undo. The rates are one machine's, so each run that finishes corrects them: the
//! last ratio of the seconds seen to the seconds told is kept for each operator in the settings.

use eframe::egui::{self, RichText, Ui};
use terraflux_graph::{Node, Source};
use terraflux_ops::Value;
use terraflux_ops::objects::{IMAGE_SEGMENTERS, SUGGESTED_TILE, estimate};

use crate::app::Session;

/// The estimate shown from this many seconds on, and the tiles offered from the same.
pub const SHOW_SECONDS: f64 = 2.0;
pub const OFFER_SECONDS: f64 = 30.0;

/// The cells and the bands of the image a node reads: from the value of its input when it ran, else from the grids of
/// the memory estimate (None while that is being worked out, or for a file that does not read).
pub fn image_of(s: &Session, n: &Node) -> Option<(u64, usize)> {
    if let Some(size) = ran_size(s, n) {
        return Some(size);
    }
    let Source::Ref(r) = n.inputs.get("image")? else { return None };
    let (e, _) = s.memory_poll()?;
    e.grids.get(r.split_once('.')?.0).map(|(cells, layers)| (*cells, *layers as usize))
}

/// The cells and the bands of the value the node's image input holds, when it ran.
fn ran_size(s: &Session, n: &Node) -> Option<(u64, usize)> {
    let Source::Ref(r) = n.inputs.get("image")? else { return None };
    let (node, port) = r.split_once('.')?;
    match s.value_of(node, port)? {
        Value::Map(m) => Some((m.grid.cells() as u64, 1)),
        Value::Stack(st) => Some((st.grid.cells() as u64, st.layers.len().max(1))),
        _ => None,
    }
}

/// The seconds a node will take, about, on `threads` threads, and the estimate itself (the machine's correction
/// applied).
pub fn of(s: &Session, n: &Node, threads: usize) -> Option<(terraflux_ops::objects::Estimate, u64, usize)> {
    if !IMAGE_SEGMENTERS.contains(&n.op.as_str()) {
        return None;
    }
    let (cells, bands) = image_of(s, n)?;
    let params = n.params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let mut e = estimate(&n.op, &params, cells, bands, threads)?;
    let k = ratio(&n.op);
    e.seconds *= k;
    if let Some((_, secs)) = &mut e.by_tiles {
        *secs *= k;
    }
    Some((e, cells, bands))
}

/// How the seconds seen compared with the seconds told the last time the operator ran here (1 before one did).
pub fn ratio(op: &str) -> f64 {
    crate::settings::get().time_ratios.get(op).copied().unwrap_or(1.0)
}

fn seconds_text(s: f64) -> String {
    if s < 1.0 {
        format!("{:.0} ms", s * 1000.0)
    } else if s < 120.0 {
        format!("{s:.0} s")
    } else if s < 7200.0 {
        format!("{:.0} min", s / 60.0)
    } else {
        format!("{:.1} h", s / 3600.0)
    }
}

/// What the Node pane says: "about 5 min on 8 threads; by tiles of 512, about 1 min".
pub fn text(e: &terraflux_ops::objects::Estimate, threads: usize) -> String {
    let on = format!("{threads} thread{}", if threads == 1 { "" } else { "s" });
    let mut t = format!("about {} on {on}", seconds_text(e.seconds));
    if e.tile > 0 {
        t.push_str(&format!(" (tiles of {})", e.tile));
    }
    if let Some((side, secs)) = e.by_tiles {
        t.push_str(&format!("; by tiles of {side}, about {}", seconds_text(secs)));
    }
    t
}

/// Whether *Use tiles* is offered for an estimate.
pub fn offers_tiles(e: &terraflux_ops::objects::Estimate) -> bool {
    e.seconds >= OFFER_SECONDS && e.by_tiles.is_some_and(|(_, secs)| secs < e.seconds)
}

/// *Use tiles*: `tile` written into the node (one step of undo). The node must be in the model's top level.
pub fn use_tiles(s: &mut Session, id: &str) -> Result<(), String> {
    let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == id) else {
        return Err(format!("{id} is not a node of the model's top level"));
    };
    let tiles = crate::live_segment::registry().sig(&n.op).is_some_and(|g| g.params.iter().any(|p| p.name == "tile"));
    if !tiles {
        return Err(format!("{}: no tiles", n.op));
    }
    n.params.insert("tile".into(), SUGGESTED_TILE.into());
    s.history.name(&format!("Tiles of {SUGGESTED_TILE} on {id}"));
    s.edited();
    Ok(())
}

/// The line in the Node pane, on the row of the last run's time: the estimate and, when it is long and tiles would
/// shorten it, *Use tiles*. Nothing for a node that is not a segmenter of the top level, or a short estimate.
pub fn line(ui: &mut Ui, s: &mut Session, n: &Node, path: &[String]) {
    if !path.is_empty() || !IMAGE_SEGMENTERS.contains(&n.op.as_str()) {
        return;
    }
    let threads = terraflux_ops::threads::ensure();
    let Some((e, _, _)) = of(s, n, threads) else {
        // (the grid is still being read from the headers)
        if s.memory_pending() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        return;
    };
    if e.seconds < SHOW_SECONDS {
        return;
    }
    ui.add(egui::Label::new(RichText::new(text(&e, threads)).weak()).extend()).on_hover_text(
        "estimated before running, from the rates of the benchmark battery (one machine's: corrected by how long it took here the last time) and the size of the image; the content of the image and most settings are not counted",
    );
    if offers_tiles(&e)
        && ui
            .button("Use tiles")
            .on_hover_text(format!("tile = {SUGGESTED_TILE}: the image is cut in tiles segmented in parallel and stitched at their borders; the objects at the borders can differ a little"))
            .clicked()
        && let Err(m) = use_tiles(s, &n.id)
    {
        s.editor.message = Some(m);
    }
}

/// A run of a segmenter finished: the ratio of the seconds seen to the seconds told, kept for the operator.
pub fn learn(s: &Session, at: &str, seconds: f64) {
    let Some(n) = s.source.nodes.iter().find(|n| n.id == at) else { return };
    if !IMAGE_SEGMENTERS.contains(&n.op.as_str()) || seconds < 0.2 {
        return;
    }
    let Some((cells, bands)) = ran_size(s, n) else { return };
    let params = n.params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let Some(told) = estimate(&n.op, &params, cells, bands, terraflux_ops::threads::ensure()) else { return };
    if told.seconds < 0.2 {
        return;
    }
    let k = (seconds / told.seconds).clamp(0.1, 10.0);
    let op = n.op.clone();
    crate::settings::update(|st| {
        st.time_ratios.insert(op, k);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::objects::Estimate;

    #[test]
    fn the_text_gives_the_threads_and_the_tiles() {
        let e = Estimate { seconds: 300.0, tile: 0, by_tiles: Some((512, 60.0)) };
        assert_eq!(text(&e, 8), "about 5 min on 8 threads; by tiles of 512, about 60 s");
        let e = Estimate { seconds: 60.0, tile: 512, by_tiles: None };
        assert_eq!(text(&e, 1), "about 60 s on 1 thread (tiles of 512)");
    }

    #[test]
    fn tiles_are_offered_for_a_long_run_they_shorten() {
        let long = Estimate { seconds: 300.0, tile: 0, by_tiles: Some((512, 60.0)) };
        assert!(offers_tiles(&long));
        assert!(!offers_tiles(&Estimate { seconds: 10.0, ..long.clone() }), "short");
        assert!(!offers_tiles(&Estimate { by_tiles: Some((512, 400.0)), ..long.clone() }), "no gain");
        assert!(!offers_tiles(&Estimate { tile: 512, by_tiles: None, ..long }), "already tiled");
    }
}
