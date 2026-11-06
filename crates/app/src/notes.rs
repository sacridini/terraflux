//! The notes and frames of the canvas ([`terraflux_graph::Note`]): a note is a text put somewhere, a frame a
//! rectangle around nodes with a title (their boxes' rectangle and a margin, so it follows *Arrange* and the
//! drags). Drawn behind the boxes; a note dragged by itself, a frame by its title (its nodes with it); a double click
//! edits the text, Delete takes the one selected away, its menu gives its colour.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use terraflux_graph::{Model, NOTE_COLOURS, Note};

use crate::editor_view::Action;

/// A note's width (graph units), the room around a frame's nodes and the height of its title.
pub const NOTE_WIDTH: f32 = 240.0;
const PAD: f32 = 14.0;
const TITLE: f32 = 28.0;
const TEXT_SIZE: f32 = 13.0;

/// A note or frame laid out (graph units): all of it, and where it is held to be dragged (a note all of it, a frame
/// its title).
#[derive(Debug, Clone)]
pub struct Laid {
    pub id: String,
    pub frame: bool,
    pub rect: Rect,
    pub grip: Rect,
    pub nodes: Vec<String>,
    pub colour: Color32,
    pub text: String,
}

/// A note's colour by its name (none, or one not known: amber).
pub fn colour(name: Option<&str>) -> Color32 {
    match name.unwrap_or("amber") {
        "blue" => Color32::from_rgb(80, 150, 235),
        "green" => Color32::from_rgb(90, 185, 100),
        "red" => Color32::from_rgb(225, 85, 75),
        "purple" => Color32::from_rgb(170, 110, 225),
        "grey" => Color32::from_rgb(150, 150, 150),
        _ => Color32::from_rgb(235, 175, 50),
    }
}

/// The notes and frames of a scope laid out: a frame around the boxes of its nodes there (`boxed`: a node's box,
/// none when it is not in the scene), a note at its place, as tall as its text wrapped needs (`measure`: the height
/// of a text wrapped to a width, in graph units). `moving`: a note dragged, where it is now.
pub fn layout(
    model: &Model,
    scope: &[String],
    boxed: &dyn Fn(&str) -> Option<Rect>,
    measure: &dyn Fn(&str, f32) -> f32,
    moving: Option<(&str, Pos2)>,
) -> Vec<Laid> {
    let mut out: Vec<Laid> = Vec::new();
    for n in model.notes_of(scope) {
        let c = colour(n.colour.as_deref());
        if n.is_frame() {
            let Some(u) = n.nodes.iter().filter_map(|id| boxed(id)).reduce(|a, b| a.union(b)) else { continue };
            let rect = Rect::from_min_max(u.min - Vec2::new(PAD, PAD + TITLE), u.max + Vec2::splat(PAD));
            let grip = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + TITLE));
            out.push(Laid {
                id: n.id.clone(),
                frame: true,
                rect,
                grip,
                nodes: n.nodes.clone(),
                colour: c,
                text: n.title().into(),
            });
        } else {
            let at = match moving {
                Some((id, p)) if id == n.id => p,
                _ => Pos2::from(n.at.unwrap_or([0.0, 0.0])),
            };
            let shown = if n.text.trim().is_empty() { "(an empty note)" } else { n.text.as_str() };
            let h = measure(shown, NOTE_WIDTH - 2.0 * 10.0) + 2.0 * 9.0;
            let rect = Rect::from_min_size(at, Vec2::new(NOTE_WIDTH, h.max(34.0)));
            out.push(Laid {
                id: n.id.clone(),
                frame: false,
                rect,
                grip: rect,
                nodes: Vec::new(),
                colour: c,
                text: shown.into(),
            });
        }
    }
    // the frames first: under the notes (a note on a frame is held before it)
    out.sort_by_key(|l| !l.frame);
    out
}

/// The note or frame held at a point (graph units): the last drawn first.
pub fn hit(laid: &[Laid], p: Pos2) -> Option<&Laid> {
    laid.iter().rev().find(|l| l.grip.contains(p))
}

/// The frame whose rectangle a point is in (the innermost: the smallest), not counting `but`.
pub fn frame_at<'a>(laid: &'a [Laid], p: Pos2, but: &[String]) -> Option<&'a Laid> {
    laid.iter()
        .filter(|l| l.frame && l.rect.contains(p) && !but.contains(&l.id))
        .min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
}

/// The text's height wrapped to a width (graph units), as the canvas writes a note.
pub fn measurer(painter: &egui::Painter) -> impl Fn(&str, f32) -> f32 + '_ {
    move |t: &str, w: f32| painter.layout(t.to_string(), FontId::proportional(TEXT_SIZE), Color32::WHITE, w).size().y
}

/// Draw them, behind the boxes: a frame washed in its colour with its title at the top, a note as a card;
/// `chosen`, outlined.
pub fn draw(
    painter: &egui::Painter,
    laid: &[Laid],
    screen: &dyn Fn(Pos2) -> Pos2,
    zoom: f32,
    v: &egui::Visuals,
    chosen: Option<&str>,
) {
    let clip = painter.clip_rect();
    for l in laid {
        let on = Rect::from_min_max(screen(l.rect.min), screen(l.rect.max));
        if !on.intersects(clip) {
            continue;
        }
        let picked = chosen == Some(l.id.as_str());
        let stroke = Stroke::new(if picked { 2.5 } else { 1.2 }, if picked { v.strong_text_color() } else { l.colour });
        let radius = 8.0 * zoom.min(1.5);
        if l.frame {
            painter.rect_filled(on, radius, l.colour.gamma_multiply(0.07));
            painter.rect_stroke(on, radius, stroke, egui::StrokeKind::Inside);
            let size = (16.0 * zoom).max(9.0);
            let title = if l.text.is_empty() { "(a frame)" } else { l.text.as_str() };
            let grip = Rect::from_min_max(screen(l.grip.min), screen(l.grip.max));
            painter.with_clip_rect(grip.intersect(clip)).text(
                grip.left_center() + Vec2::new(10.0 * zoom, 0.0),
                Align2::LEFT_CENTER,
                title,
                FontId::proportional(size),
                l.colour,
            );
        } else {
            let fill = v.extreme_bg_color.lerp_to_gamma(l.colour, 0.22);
            painter.rect_filled(on, radius, fill);
            painter.rect_stroke(on, radius, stroke, egui::StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_min_size(on.min, Vec2::new(4.0 * zoom.max(0.5), on.height())),
                egui::CornerRadius { nw: radius as u8, sw: radius as u8, ne: 0, se: 0 },
                l.colour,
            );
            if zoom >= 0.25 {
                let galley = painter.layout(
                    l.text.clone(),
                    FontId::proportional(TEXT_SIZE * zoom),
                    v.text_color(),
                    (NOTE_WIDTH - 20.0) * zoom,
                );
                painter.with_clip_rect(on.intersect(clip)).galley(
                    on.min + Vec2::new(10.0, 9.0) * zoom,
                    galley,
                    v.text_color(),
                );
            }
        }
    }
}

/// What the entries of the menus do.
pub const ADD_HELP: &str = "a note put here, its text typed at once (for whoever reads the model; never run)";
pub const FRAME_HELP: &str = "a frame around them with a title: it follows them, dragging its title drags them, a \
     node let go in it joins it";

/// A note's or a frame's menu (right-clicked).
pub fn menu(ui: &mut egui::Ui, n: &Note, actions: &mut Vec<Action>) {
    let what = if n.is_frame() { "frame" } else { "note" };
    if ui.button("Edit the text...").on_hover_text("a double click does it too").clicked() {
        actions.push(Action::EditNote(n.id.clone()));
        ui.close();
    }
    ui.menu_button("Colour", |ui| {
        for name in NOTE_COLOURS {
            let on = n.colour.as_deref().unwrap_or("amber") == name;
            let text = egui::RichText::new(format!("● {name}")).color(colour(Some(name)));
            if ui.add(egui::Button::new(text).selected(on)).clicked() {
                actions.push(Action::NoteColour(n.id.clone(), Some(name.to_string())));
                ui.close();
            }
        }
    });
    let help = if n.is_frame() { "the frame taken away; its nodes stay" } else { "the note taken away (Delete)" };
    if ui.button(format!("Delete the {what}")).on_hover_text(help).clicked() {
        actions.push(Action::RemoveNote(n.id.clone()));
        ui.close();
    }
}

/// The text of a note being edited: a window by it, the text, OK (ctrl+enter) or Cancel (Esc).
pub fn editor(ctx: &egui::Context, editing: &mut Option<(String, String, bool)>, at: Pos2, actions: &mut Vec<Action>) {
    let Some((id, text, focused)) = editing else { return };
    let mut done = None;
    egui::Window::new("Note").id(egui::Id::new("note text")).collapsible(false).resizable(false).default_pos(at).show(
        ctx,
        |ui| {
            ui.label(egui::RichText::new("For a frame, the first line is its title.").weak().size(11.0));
            let r = ui.add(egui::TextEdit::multiline(text).desired_rows(4).desired_width(300.0));
            if !*focused {
                r.request_focus();
                *focused = true;
            }
            let (ok, cancel) = ui
                .input(|i| (i.modifiers.command && i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
            ui.horizontal(|ui| {
                if ui.add(crate::app::accent_button("OK", true)).on_hover_text("ctrl+enter").clicked() || ok {
                    done = Some(true);
                }
                if ui.button("Cancel").on_hover_text("Esc").clicked() || cancel {
                    done = Some(false);
                }
            });
        },
    );
    match done {
        Some(true) => {
            actions.push(Action::SetNote(id.clone(), text.trim_end().to_string()));
            *editing = None;
        }
        Some(false) => *editing = None,
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Node;

    #[test]
    fn a_frame_follows_its_nodes_and_a_note_its_place() {
        let mut m = Model::new("m");
        m.nodes.push(Node::new("a", "nlm.random"));
        m.nodes.push(Node::new("b", "nlm.random"));
        m.notes.push(Note {
            id: "frame1".into(),
            text: "Inputs\nmore".into(),
            nodes: vec!["a".into(), "b".into()],
            ..Default::default()
        });
        m.notes.push(Note { id: "note1".into(), text: "hello".into(), at: Some([500.0, 0.0]), ..Default::default() });
        let boxes = |at: [f32; 2]| {
            move |id: &str| match id {
                "a" => Some(Rect::from_min_size(Pos2::from(at), Vec2::new(100.0, 50.0))),
                "b" => Some(Rect::from_min_size(Pos2::new(at[0], at[1] + 100.0), Vec2::new(100.0, 50.0))),
                _ => None,
            }
        };
        let measure = |_: &str, _: f32| 18.0;
        let laid = layout(&m, &[], &boxes([0.0, 0.0]), &measure, None);
        assert_eq!(laid.len(), 2);
        let f = &laid[0];
        assert!(f.frame && f.text == "Inputs", "the frames first, by their title");
        assert_eq!(f.rect, Rect::from_min_max(Pos2::new(-PAD, -PAD - TITLE), Pos2::new(100.0 + PAD, 150.0 + PAD)));
        // its nodes moved: the frame with them
        let moved = layout(&m, &[], &boxes([200.0, 40.0]), &measure, None);
        assert_eq!(moved[0].rect.min, Pos2::new(200.0 - PAD, 40.0 - PAD - TITLE));
        // held by its title, not inside; the note anywhere on it
        assert_eq!(hit(&laid, Pos2::new(10.0, -PAD - 5.0)).map(|l| l.id.as_str()), Some("frame1"));
        assert!(hit(&laid, Pos2::new(50.0, 25.0)).is_none());
        assert_eq!(hit(&laid, Pos2::new(510.0, 10.0)).map(|l| l.id.as_str()), Some("note1"));
        assert_eq!(frame_at(&laid, Pos2::new(50.0, 75.0), &[]).map(|l| l.id.as_str()), Some("frame1"));
        // a note dragged: where it is now
        let dragged = layout(&m, &[], &boxes([0.0, 0.0]), &measure, Some(("note1", Pos2::new(0.0, 400.0))));
        assert_eq!(dragged[1].rect.min, Pos2::new(0.0, 400.0));
    }
}
