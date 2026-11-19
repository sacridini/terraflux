//! The Classes view of the Model pane (beside the diagram and the text): the model seen as the tree of its classes,
//! a card for each, indented by its depth ([`crate::class_tree`]): its name and colour, how it is found, the rule
//! and the samples when it has them, and the area a run found. A click on a card goes to its group in the diagram.
//! *Add a class...* makes the branch of a new class (a name, a colour, its mother, a recipe for its body) as one step
//! of undo.

use egui::{Color32, Margin, RichText, Sense, Ui, Vec2};
use terraflux_graph::Node;
use terraflux_ops::Registry;

use crate::app::Session;
use crate::class_tree::{self, Entry, NewClass, Recipe};
use crate::theme;

/// The form of *Add a class...*, kept between frames.
#[derive(Clone)]
struct Form {
    open: bool,
    name: String,
    colour: [u8; 3],
    parent: Option<String>,
    recipe: Recipe,
    /// Why the last try failed, and what the class added still needs.
    error: Option<String>,
    told: Vec<String>,
}

impl Default for Form {
    fn default() -> Self {
        Form {
            open: false,
            name: String::new(),
            colour: [0, 0, 0],
            parent: None,
            recipe: Recipe::Water,
            error: None,
            told: Vec::new(),
        }
    }
}

fn weak(t: impl Into<String>) -> RichText {
    RichText::new(t.into()).color(theme::weak())
}

fn colour_of(e: &Entry, k: usize) -> Color32 {
    match e.colour {
        Some([r, g, b]) => Color32::from_rgb(r, g, b),
        None => crate::palette::class_colour(k),
    }
}

/// The area as a short text.
fn area_text(a: f64) -> String {
    if a >= 1000.0 { format!("{a:.0}") } else { format!("{a:.2}") }
}

/// A card of the tree; true when it was clicked.
fn card(ui: &mut Ui, e: &Entry, k: usize) -> bool {
    let id = egui::Id::new(("class card", &e.node, &e.name));
    let shown = ui.horizontal(|ui| {
        ui.add_space(e.depth as f32 * 22.0);
        egui::Frame::new()
            .fill(theme::well())
            .stroke(ui.visuals().window_stroke)
            .corner_radius(theme::rounded(4.0))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.set_width((ui.available_width() - 4.0).max(120.0));
                ui.horizontal(|ui| {
                    let (swatch, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    ui.painter().rect_filled(swatch, 3.0, colour_of(e, k));
                    ui.label(RichText::new(&e.name).strong());
                    ui.label(weak(&e.method));
                    if e.rules {
                        ui.label(weak("(a class of objects.rules: read only)"));
                    }
                });
                if let Some(rule) = &e.rule {
                    ui.label(RichText::new(rule).monospace().color(theme::weak()));
                }
                let mut facts: Vec<String> = Vec::new();
                if let Some(n) = e.samples {
                    facts.push(format!("{n} training objects"));
                }
                if let Some(a) = e.area {
                    facts.push(format!("area found {}", area_text(a)));
                }
                if !facts.is_empty() {
                    ui.label(weak(facts.join("  |  ")));
                }
            })
            .response
    });
    let rect = shown.inner.rect;
    let r = ui.interact(rect, id, Sense::click()).on_hover_text(if e.rules {
        "select its objects.rules node in the diagram"
    } else {
        "select its group in the diagram"
    });
    r.clicked()
}

/// The form of *Add a class...*; the class asked for when *Add* is pressed.
fn form_ui(ui: &mut Ui, form: &mut Form, names: &[String]) -> Option<NewClass> {
    let mut asked = None;
    theme::card(ui, "Add a class", |ui| {
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut form.name).hint_text("Buildings").desired_width(180.0));
            ui.label("Colour");
            ui.color_edit_button_srgb(&mut form.colour);
        });
        ui.horizontal(|ui| {
            ui.label("Mother");
            let shown = form.parent.clone().unwrap_or_else(|| "none (a class of the scene)".into());
            egui::ComboBox::from_id_salt("class mother").selected_text(shown).show_ui(ui, |ui| {
                ui.selectable_value(&mut form.parent, None, "none (a class of the scene)");
                for n in names {
                    ui.selectable_value(&mut form.parent, Some(n.clone()), n);
                }
            });
            ui.label("Recipe");
            egui::ComboBox::from_id_salt("class recipe").selected_text(form.recipe.label()).show_ui(ui, |ui| {
                for r in Recipe::ALL {
                    ui.selectable_value(&mut form.recipe, r, r.label());
                }
            });
        });
        ui.label(weak(form.recipe.about()));
        if let Some(e) = &form.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        ui.horizontal(|ui| {
            if ui.button("Add").clicked() {
                asked = Some(NewClass {
                    name: form.name.trim().to_string(),
                    colour: form.colour,
                    parent: form.parent.clone(),
                    recipe: form.recipe,
                });
            }
            if ui.button("Cancel").clicked() {
                form.open = false;
                form.error = None;
            }
        });
    });
    asked
}

/// The scope's nodes (the model's, or a container's body).
fn nodes_of(s: &Session) -> &[Node] {
    s.source.scope(&s.editor.path).map_or(&[], |v| v.as_slice())
}

/// The Classes view.
pub fn pane(ui: &mut Ui, s: &mut Session, r: &Registry) {
    let id = egui::Id::new("classes form");
    let mut form: Form = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let mut tree = class_tree::read(nodes_of(s));
    if s.editor.path.is_empty() {
        class_tree::with_results(&mut tree, &s.source.nodes, &s.outputs);
    }
    let mut go: Option<String> = None;
    let mut asked: Option<NewClass> = None;
    egui::ScrollArea::vertical().id_salt("classes pane").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(Margin::same(10)).show(ui, |ui| {
            theme::section(ui, "Classes");
            ui.label(weak(
                "What there is in the scene, said first. Each class is a group of the model that looks for it inside \
                 its mother's region; the classes of one mother are decided together, the greatest membership wins.",
            ));
            ui.add_space(6.0);
            let rules = tree.iter().any(|e| e.rules);
            for (k, e) in tree.iter().enumerate() {
                if card(ui, e, k) {
                    go = Some(e.node.clone());
                }
                ui.add_space(3.0);
            }
            if tree.is_empty() {
                ui.label(weak(
                    "No classes yet. Add one: a group with the settings class and parent_class, its body reading \
                     @region and giving a membership.",
                ));
            }
            ui.add_space(6.0);
            if rules {
                ui.label(weak("The classes of objects.rules are read only: change them in its settings."));
            } else if form.open {
                let names: Vec<String> = tree.iter().map(|e| e.name.clone()).collect();
                asked = form_ui(ui, &mut form, &names);
            } else if ui.button("Add a class...").on_hover_text("a branch of the model for a new class").clicked() {
                form.open = true;
                form.colour = {
                    let c = crate::palette::class_colour(tree.len());
                    [c.r(), c.g(), c.b()]
                };
                form.error = None;
                form.told.clear();
            }
            for t in &form.told {
                ui.colored_label(ui.visuals().warn_fg_color, t);
            }
        });
    });
    if let Some(c) = asked {
        let mut model = s.source.clone();
        let path = s.editor.path.clone();
        match class_tree::add_class(&mut model, &path, r, &c) {
            Ok(told) => {
                s.source = model;
                s.history.name(&format!("added the class {}", c.name));
                s.edited();
                form = Form { told, ..Form::default() };
            }
            Err(e) => form.error = Some(e),
        }
    }
    if let Some(node) = go {
        crate::builder::select(s, r, &node);
        s.editor.classes = false;
    }
    ui.data_mut(|d| d.insert_temp(id, form));
}
