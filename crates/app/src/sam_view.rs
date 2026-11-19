//! The question before SAM's weights are downloaded: a `segment.sam` with no encoder and decoder given downloads its
//! weights the first time it runs (75 MB, or 348 MB for the full ones), so a run that would do that is held back
//! ([`crate::app::Session::sam_need`]) and a window says what is coming: the size, the licence, where from, where
//! they are kept, and the choice between the two (written to the node's `weights`, one step of undo). Agreeing is
//! remembered on this computer for each of the two ([`crate::settings::Settings::sam_agreed`]). The progress and the
//! stopping of the download are the run's own (the status bar, *Stop*).

use egui::RichText;
use terraflux_ops::objects::weights;

use crate::app::Session;

/// The licence of the weights (the export of SAM ViT-B named in [`weights`]).
const LICENCE: &str = "Apache-2.0";

/// A run held back for the question.
pub struct Ask {
    /// The `segment.sam` node whose weights are in question.
    pub node: String,
    /// What the run was to do: the node it runs to (none: the whole model), and the check run it was, if any.
    pub target: Option<String>,
    pub check: Option<terraflux_ops::check::Check>,
}

/// What the window was told.
enum Act {
    Run,
    Cancel,
    Pick(&'static str),
}

/// The variant a node's `weights` names (quant when it says nothing).
fn variant(s: &Session, node: &str) -> String {
    let w = s.source.nodes.iter().find(|n| n.id == node).and_then(|n| n.params.get("weights")?.as_str());
    w.filter(|w| !w.is_empty()).unwrap_or("quant").to_string()
}

/// The size in megabytes (10^6 bytes, as the progress says it).
fn mb(bytes: u64) -> u64 {
    (bytes + 500_000) / 1_000_000
}

/// The folder the weights of a source are kept in.
pub fn folder(src: &weights::Source) -> std::path::PathBuf {
    terraflux_ops::cache::root().join("sam").join(src.name)
}

/// Write the choice to the node: one step of undo.
fn pick(s: &mut Session, node: &str, w: &str) {
    let before = s.source.clone();
    match crate::editor::set_param(&mut s.source, &[], node, "weights", w) {
        Ok(()) => {
            s.history.name(&format!("Weights of {node}: {w}"));
            s.edited();
        }
        Err(e) => {
            s.source = before;
            s.notice = Some((format!("{node}: {e}"), std::time::Instant::now()));
        }
    }
}

/// The run held back, started (agreed to, or not needed any more).
fn go(s: &mut Session, ctx: &egui::Context) {
    if let Some(a) = s.sam_ask.take() {
        let ctx = ctx.clone();
        s.start_run(a.target, Box::new(move || ctx.request_repaint()), false, a.check);
    }
}

/// The window, while a run is held back for it.
pub fn window(ctx: &egui::Context, s: &mut Session) {
    let Some(a) = &s.sam_ask else { return };
    let (node, target) = (a.node.clone(), a.target.clone());
    // (the choice written, or the node taken away: nothing to download any more, so the run goes on)
    if s.sam_need(target.as_deref()).is_none() {
        go(s, ctx);
        return;
    }
    let w = variant(s, &node);
    let Ok(src) = weights::source(&w) else {
        s.sam_ask = None;
        return;
    };
    let mut act = None;
    let modal = egui::Modal::new(egui::Id::new("sam weights")).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.label(RichText::new("Download the weights of SAM?").strong().size(15.0));
        ui.add_space(4.0);
        ui.label(format!(
            "{node} (segment.sam) needs the weights of the Segment Anything Model, which are not on this computer. \
             They are downloaded once and kept for every model."
        ));
        ui.add_space(6.0);
        for (name, text) in [("quant", "Quantised"), ("full", "Full precision")] {
            let size = weights::source(name).map_or(0, |s| mb(s.bytes));
            if ui.radio(w == name, format!("{text} ({size} MB)")).clicked() && w != name {
                act = Some(Act::Pick(if name == "quant" { "quant" } else { "full" }));
            }
        }
        ui.add_space(6.0);
        egui::Grid::new("sam facts").num_columns(2).spacing([8.0, 3.0]).show(ui, |ui| {
            ui.label(RichText::new("size").weak());
            ui.label(format!("{} MB", mb(src.bytes)));
            ui.end_row();
            ui.label(RichText::new("licence").weak());
            ui.label(LICENCE);
            ui.end_row();
            ui.label(RichText::new("from").weak());
            ui.label(src.url);
            ui.end_row();
            ui.label(RichText::new("kept in").weak());
            ui.label(folder(src).display().to_string());
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "The status bar shows the download; Stop cancels it. To use your own files, give the node's encoder \
                 and decoder.",
            )
            .small()
            .color(crate::theme::weak()),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add(crate::app::accent_button("Download and run", true)).clicked() {
                act = Some(Act::Run);
            }
            if ui.button("Cancel").clicked() {
                act = Some(Act::Cancel);
            }
        });
    });
    if modal.should_close() && act.is_none() {
        act = Some(Act::Cancel);
    }
    match act {
        Some(Act::Pick(v)) => pick(s, &node, v),
        Some(Act::Run) => {
            let agreed = w.clone();
            crate::settings::update(|st| {
                if !st.sam_agreed.contains(&agreed) {
                    st.sam_agreed.push(agreed);
                }
            });
            go(s, ctx);
        }
        Some(Act::Cancel) => {
            s.sam_ask = None;
            s.run_queue.clear();
        }
        None => {}
    }
}
