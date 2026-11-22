//! The confusion matrix of an `objects.accuracy` node's `matrix` ([`crate::charts`]) and the classified objects in the
//! Viewer, linked: a cell picked in the chart (a class mapped, a reference class) lights the reference objects of that
//! pair on the map of the node that classified them (the rest veiled), and the list under the chart goes to each of
//! them. The pick is kept in the context by the accuracy node's name, so it stays while the Viewer shows the objects.
//! (ml.fit's `cv_matrix` has no objects: its chart has no link.)

use std::sync::{Arc, Mutex};

use egui::{Color32, RichText, Ui};
use terraflux_graph::Source;
use terraflux_ops::objects::Objects;
use terraflux_ops::{Map, Value};

use crate::app::Session;
use crate::charts::ConfusionPick;
use crate::map_view::{RasterLayer, Source as MapSource};
use crate::palette::Coloring;

/// The value of a cell the pick leaves, and the one it covers, in the map made to light them.
const VEIL: f64 = 1.0;
const LIT: f64 = 2.0;

/// The objects listed under the chart, at most.
const LISTED: usize = 300;

/// The cell picked of the `objects.accuracy` node `node`.
pub fn linked(ctx: &egui::Context, node: &str) -> Option<ConfusionPick> {
    ctx.data(|d| d.get_temp::<Option<ConfusionPick>>(egui::Id::new(("confusion link", node)))).flatten()
}

/// Pick a cell of the `objects.accuracy` node `node` (none: let go).
pub fn set_linked(ctx: &egui::Context, node: &str, pick: Option<ConfusionPick>) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("confusion link", node)), pick));
}

/// The node that made the objects an `objects.accuracy` node `acc` judges (the one its `objects` input comes from).
pub fn classified_by(s: &Session, acc: &str) -> Option<String> {
    let n = s.source.nodes.iter().find(|n| n.id == acc && n.op == "objects.accuracy")?;
    match n.inputs.get("objects") {
        Some(Source::Ref(r)) => r.split_once('.').map(|(node, _)| node.to_string()),
        _ => None,
    }
}

/// The reference objects of a picked pair, by id: those the node's `sampled` table lists mapped as the first class of
/// the pick and referenced as the second.
pub fn ids(s: &Session, acc: &str, pick: &ConfusionPick) -> Vec<u32> {
    let Some(Value::Table(t)) = s.value_of(acc, "sampled") else { return Vec::new() };
    t.rows.iter().filter(|r| t.cell(1, r[1]) == pick.0 && t.cell(2, r[2]) == pick.1).map(|r| r[0] as u32).collect()
}

/// The cells of the objects: [`LIT`] in those of `ids`, the others [`VEIL`], NaN where there is no object.
pub fn mask(o: &Objects, ids: &[u32]) -> Vec<f64> {
    let mut on = vec![false; o.count + 1];
    for &id in ids {
        if let Some(b) = on.get_mut(id as usize) {
            *b = true;
        }
    }
    o.labels
        .iter()
        .map(|&l| match l {
            0 => f64::NAN,
            l if on[l as usize] => LIT,
            _ => VEIL,
        })
        .collect()
}

/// The map made to light a pick, kept while the pick and the objects are the same (by the objects' address).
#[derive(Default)]
struct Made {
    cache: Option<(Option<ConfusionPick>, usize, Map)>,
}

fn made(ctx: &egui::Context, acc: &str) -> Arc<Mutex<Made>> {
    let id = egui::Id::new(("confusion lit", acc));
    ctx.data_mut(|d| d.get_temp_mut_or_insert_with::<Arc<Mutex<Made>>>(id, Default::default).clone())
}

/// The accuracy node whose pick the map shown (`node.port`) lights: the one judging the objects `node` made.
fn judge(ctx: &egui::Context, s: &Session, node: &str) -> Option<(String, ConfusionPick)> {
    s.source.nodes.iter().filter(|n| n.op == "objects.accuracy").find_map(|n| {
        let pick = linked(ctx, &n.id)?;
        (classified_by(s, &n.id).as_deref() == Some(node)).then(|| (n.id.clone(), pick))
    })
}

/// The layer lighting the pick over the objects or map shown when it is the output of the node that classified the
/// objects an `objects.accuracy` node judges: the reference objects of the picked pair in the accent, the other objects
/// veiled. None without a pick.
pub fn layers(ctx: &egui::Context, s: &Session, node: &str, port: &str) -> Vec<RasterLayer> {
    if port != "objects" && port != "map" {
        return Vec::new();
    }
    let Some((acc, pick)) = judge(ctx, s, node) else { return Vec::new() };
    let Some(Value::Objects(o)) = s.value_of(node, "objects") else { return Vec::new() };
    let key = Arc::as_ptr(&o) as usize;
    let st = made(ctx, &acc);
    let mut st = st.lock().unwrap_or_else(|e| e.into_inner());
    if st.cache.as_ref().is_none_or(|(p, k, _)| p.as_ref() != Some(&pick) || *k != key) {
        let lit = Map::new(o.grid.clone(), mask(&o, &ids(s, &acc, &pick)));
        st.cache = Some((Some(pick.clone()), key, lit));
    }
    let Some((_, _, lit)) = &st.cache else { return Vec::new() };
    let accent = crate::palette::seen(crate::theme::accent());
    let steps = vec![(VEIL, Color32::from_black_alpha(150)), (LIT, accent)];
    vec![RasterLayer {
        source: MapSource::from(lit),
        coloring: Coloring::Classes(steps),
        opacity: 1.0,
        below: false,
        name: format!("mapped {}, reference {}", pick.0, pick.1),
        blend: Default::default(),
    }]
}

/// The map that lights the pick of the accuracy node `acc` as it was last drawn: [`LIT`] in the objects of the pair,
/// [`VEIL`] in the others, NaN where there is no object.
pub fn lit(ctx: &egui::Context, acc: &str) -> Option<Map> {
    let st = made(ctx, acc);
    let st = st.lock().unwrap_or_else(|e| e.into_inner());
    st.cache.as_ref().map(|(_, _, m)| m.clone())
}

/// The object asked to go to, by the node that made the objects (the map shown there takes the view to it).
pub fn take_go(ctx: &egui::Context, node: &str) -> Option<u32> {
    ctx.data_mut(|d| d.remove_temp::<u32>(egui::Id::new(("confusion go", node))))
}

fn ask_go(ctx: &egui::Context, node: &str, id: u32) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("confusion go", node)), id));
}

/// Under the chart of an `objects.accuracy` node's matrix: the objects of the picked cell, a button to show them on
/// the map and a list that goes to each.
pub fn panel(ui: &mut Ui, s: &mut Session, acc: &str) {
    let Some(src) = classified_by(s, acc) else { return };
    let Some(pick) = linked(ui.ctx(), acc) else { return };
    let list = ids(s, acc, &pick);
    let out = if matches!(s.value_of(&src, "objects"), Some(Value::Objects(_))) { "objects" } else { "map" };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(format!("{} objects, mapped {}, reference {}", list.len(), pick.0, pick.1)).strong());
        if ui.button("Show them on the map").clicked() {
            s.preview = Some((src.clone(), out.to_string()));
        }
    });
    if list.is_empty() {
        return;
    }
    egui::ScrollArea::vertical().id_salt(("confusion objects", acc)).max_height(160.0).show(ui, |ui| {
        for &id in list.iter().take(LISTED) {
            if ui.selectable_label(false, format!("object {id}")).clicked() {
                ask_go(ui.ctx(), &src, id);
                s.preview = Some((src.clone(), out.to_string()));
            }
        }
        if list.len() > LISTED {
            ui.label(RichText::new(format!("and {} more", list.len() - LISTED)).weak().size(11.0));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_raster::Grid;

    #[test]
    fn the_objects_of_the_ids_are_lit_and_the_others_veiled() {
        // 4 x 2 cells, three objects and a hole
        let o = Objects::from_labels(Arc::new(Grid::new(4, 2)), vec![1, 1, 2, 0, 3, 3, 2, 2], 3).unwrap();
        let m: Vec<i32> = mask(&o, &[2]).iter().map(|v| if v.is_nan() { -1 } else { *v as i32 }).collect();
        assert_eq!(m, [1, 1, 2, -1, 1, 1, 2, 2]);
        // an id that is no object lights nothing
        assert!(mask(&o, &[9]).iter().all(|v| v.is_nan() || *v == VEIL));
    }
}
