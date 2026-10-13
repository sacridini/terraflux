//! The legend editor: the names and colours of a map's classes, as the model gives them (a `raster.legend` node
//! up the way, through the plots), as the map carries them (its grid's `terraflux_legend`, a GeoTIFF written with
//! one), edited in the viewer's legend: applied as a `raster.legend` node after the node shown (or that node's
//! classes changed when it is one).

use egui::{Color32, RichText, Ui};
use terraflux_ops::Registry;
use terraflux_ops::legend::{Class, of_grid, parse, rgb};

use crate::app::{Look, Session, accent_button};
use crate::map_view::Source;
use crate::palette::Coloring;

/// The legend of an output: of a `raster.legend` node's settings (through the plots that show it), else the one
/// the map carries.
pub fn legend_of(s: &Session, node: &str, port: &str, shown: &Source) -> Option<Vec<Class>> {
    let mut at = (node.to_string(), port.to_string());
    for _ in 0..32 {
        let n = s.source.nodes.iter().find(|n| n.id == at.0)?;
        match n.op.as_str() {
            "raster.legend" => return n.params.get("classes").and_then(|c| parse(c).ok()),
            "view.plot" => match n.inputs.get("value") {
                Some(terraflux_graph::Source::Ref(r)) => match terraflux_graph::Ref::parse(r) {
                    Ok(terraflux_graph::Ref::Port { node, port }) => at = (node, port),
                    _ => break,
                },
                _ => break,
            },
            _ => break,
        }
    }
    match shown {
        Source::Map(m) => of_grid(&m.grid),
        Source::Rasters(r, l) => of_grid(r.grid_of(*l)),
        Source::Blank(_) => None,
    }
}

/// A look in a legend's colours (the classes it names; the others as they were).
pub fn apply(look: &Look, legend: &[Class]) -> Look {
    let mut l = look.clone();
    if let Coloring::Classes(c) = &mut l.coloring {
        for (v, col) in c.iter_mut() {
            if let Some(k) = legend.iter().find(|k| k.value == *v)
                && let Some([r, g, b]) = rgb(&k.colour)
            {
                *col = Color32::from_rgb(r, g, b);
            }
        }
    }
    l
}

/// A class's name in a legend, else as the look names it.
pub fn name(look: &Look, legend: Option<&[Class]>, v: f64) -> String {
    legend
        .and_then(|l| l.iter().find(|k| k.value == v && !k.name.is_empty()))
        .map(|k| k.name.clone())
        .unwrap_or_else(|| crate::builder::class_name(look, v))
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Write a legend in the model: the classes of the node shown when it is a top-level `raster.legend`, else a
/// `raster.legend` node after it (which is selected); the node holding it.
pub fn write(s: &mut Session, r: &Registry, node: &str, port: &str, classes: &[Class]) -> Result<String, String> {
    let rows: Vec<serde_json::Value> = classes.iter().map(|c| serde_json::json!([c.value, c.name, c.colour])).collect();
    let text = serde_json::Value::Array(rows).to_string();
    let top: &[String] = &[];
    if s.source.nodes.iter().any(|n| n.id == node && n.op == "raster.legend") {
        crate::editor::set_param(&mut s.source, top, node, "classes", &text)?;
        s.edited();
        return Ok(node.to_string());
    }
    let pos = crate::builder::beside(&s.source.nodes, Some(node));
    let id = crate::editor::add_node(&mut s.source, r, top, "raster.legend", pos)?;
    crate::editor::connect(&mut s.source, r, top, &format!("{node}.{port}"), &id, "map")?;
    crate::editor::set_param(&mut s.source, top, &id, "classes", &text)?;
    if let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == id) {
        n.note = Some("The names and colours of the classes, as set in the viewer's legend.".into());
    }
    s.edited();
    crate::builder::select(s, r, &id);
    Ok(id)
}

/// Whether the legend of an output is being edited.
pub fn editing(ui: &Ui, node: &str, port: &str) -> bool {
    ui.data(|d| d.get_temp::<Vec<Class>>(egui::Id::new(("legend editor", node, port))).is_some())
}

/// The legend's editor: a colour and a name per class, applied to the model (or left).
pub fn editor(
    ui: &mut Ui,
    s: &mut Session,
    r: &Registry,
    node: &str,
    port: &str,
    look: &Look,
    legend: Option<&[Class]>,
) {
    let Coloring::Classes(classes) = &look.coloring else { return };
    let id = egui::Id::new(("legend editor", node, port));
    let mut draft: Option<Vec<Class>> = ui.data(|d| d.get_temp(id));
    match &mut draft {
        None => {
            if ui.button("Edit the legend").on_hover_text("the classes' names and colours, kept in the model").clicked()
            {
                draft = Some(
                    classes
                        .iter()
                        .map(|(v, c)| Class { value: *v, name: name(look, legend, *v), colour: hex(*c) })
                        .collect(),
                );
            }
        }
        Some(d) => {
            let mut done = None;
            egui::Grid::new(id.with("rows")).num_columns(3).show(ui, |ui| {
                for c in d.iter_mut() {
                    let [rr, gg, bb] = rgb(&c.colour).unwrap_or([128, 128, 128]);
                    let mut col = Color32::from_rgb(rr, gg, bb);
                    if ui.color_edit_button_srgba(&mut col).changed() {
                        c.colour = hex(col);
                    }
                    ui.label(RichText::new(terraflux_ops::value::class_label(c.value)).monospace().weak());
                    ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(140.0));
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                if ui.add(accent_button("Apply", true)).on_hover_text("as a raster.legend node in the model").clicked()
                {
                    done = Some(true);
                }
                if ui.button("Leave").clicked() {
                    done = Some(false);
                }
            });
            match done {
                Some(true) => {
                    if let Err(e) = write(s, r, node, port, d) {
                        s.editor.message = Some(e);
                    }
                    draft = None;
                }
                Some(false) => draft = None,
                None => {}
            }
        }
    }
    ui.data_mut(|dd| match draft {
        Some(d) => {
            dd.insert_temp(id, d);
        }
        None => dd.remove::<Vec<Class>>(id),
    });
}
