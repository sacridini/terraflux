//! Reference objects in the Viewer, for `segment.evaluate`: polygons drawn on the map become, in one step of undo, a
//! node that reads them and a `segment.evaluate` of the objects shown; the numbers of its summary (ED2, D, F) are
//! shown beside the map, and where the segmentation errs is drawn over it: each object coloured by its os (the
//! over-segmentation: the reference it covers was cut, red) less its us (the under-segmentation: it swallowed what is
//! outside the reference, blue). A `segment.optimize` that has no reference yet is offered the same one.

use std::path::Path;
use std::sync::{Arc, Mutex};

use egui::{RichText, Ui, Vec2};
use terraflux_graph::{Model, Node, Source};
use terraflux_ops::{Map, Registry, Value};

use crate::app::Session;
use crate::map_view::{RasterLayer, Source as MapSource};
use crate::palette::{Coloring, Ramp};

/// The name the node that reads the reference objects takes.
const READER: &str = "reference";

/// The `segment.evaluate` nodes judging the objects `key` (`node`, `port`).
pub fn evaluators(m: &Model, key: &(String, String)) -> Vec<String> {
    let want = format!("{}.{}", key.0, key.1);
    m.nodes
        .iter()
        .filter(|n| n.op == "segment.evaluate" && matches!(n.inputs.get("objects"), Some(Source::Ref(r)) if *r == want))
        .map(|n| n.id.clone())
        .collect()
}

/// The reference of an evaluating node: the output it reads, `node.port`.
fn reference_of(m: &Model, id: &str) -> Option<String> {
    match m.nodes.iter().find(|n| n.id == id)?.inputs.get("reference") {
        Some(Source::Ref(r)) => Some(r.clone()),
        _ => None,
    }
}

/// Polygons drawn and written to `path` (a GeoPackage), as the reference of the objects `key`: a `vector.read` of the
/// file and a `segment.evaluate` of the objects with it (the reader replaced when the objects have an evaluating node
/// already), all in one step of undo. The reader's id.
pub fn add(s: &mut Session, r: &Registry, key: &(String, String), path: &Path) -> Result<String, String> {
    if !s.source.nodes.iter().any(|n| n.id == key.0) {
        return Err(format!("{} is not a node of the model's top level", key.0));
    }
    let before = s.source.clone();
    let made = write(s, r, key, path);
    match made {
        Ok(msg) => {
            s.history.name("reference objects");
            s.edited();
            Ok(msg)
        }
        Err(e) => {
            s.source = before;
            Err(e)
        }
    }
}

fn write(s: &mut Session, r: &Registry, key: &(String, String), path: &Path) -> Result<String, String> {
    let top: &[String] = &[];
    let from = format!("{}.{}", key.0, key.1);
    let pos = |s: &Session| crate::builder::beside(&s.source.nodes, Some(&key.0));
    let at = pos(s);
    let reader = crate::editor::add_node_named(&mut s.source, r, top, "vector.read", Some(READER), at)?;
    crate::editor::set_param(&mut s.source, top, &reader, "path", &path.to_string_lossy())?;
    if let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == reader) {
        n.note = Some(
            "The reference objects drawn on the map in the Viewer, polygons in a GeoPackage beside the model.".into(),
        );
    }
    let reference = format!("{reader}.vector");
    match evaluators(&s.source, key).first().cloned() {
        Some(e) => crate::editor::connect(&mut s.source, r, top, &reference, &e, "reference")?,
        None => {
            let at = pos(s);
            let e = crate::editor::add_node_named(&mut s.source, r, top, "segment.evaluate", Some("evaluate"), at)?;
            crate::editor::connect(&mut s.source, r, top, &from, &e, "objects")?;
            crate::editor::connect(&mut s.source, r, top, &reference, &e, "reference")?;
            if let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == e) {
                n.note = Some(
                    "The segmentation measured against the reference objects: over- and under-segmentation, D, ED2 and F \
                     (the summary), and the os and us of each object (objects), drawn over the map in the Viewer."
                        .into(),
                );
            }
        }
    }
    Ok(reader)
}

/// The segment.optimize nodes with no reference.
fn bare_optimizers(m: &Model) -> Vec<&Node> {
    m.nodes.iter().filter(|n| n.op == "segment.optimize" && !n.inputs.contains_key("reference")).collect()
}

/// Whether the errors of the evaluating node are drawn over the map.
pub fn errors_shown(ctx: &egui::Context, evaluate: &str) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(("reference errors", evaluate)))).unwrap_or(false)
}

/// Draw (or not) the errors of the evaluating node over the map.
pub fn show_errors(ctx: &egui::Context, evaluate: &str, on: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("reference errors", evaluate)), on));
}

/// The numbers of an evaluating node's summary: (name, value).
pub fn numbers(s: &Session, evaluate: &str) -> Vec<(&'static str, f64)> {
    let Some(Value::Table(t)) = s.value_of(evaluate, "summary") else { return Vec::new() };
    ["ed2", "d", "f_overlap", "os", "us"]
        .into_iter()
        .filter_map(|n| t.column(n).and_then(|c| c.first().copied()).map(|v| (n, v)))
        .collect()
}

/// The cells' value, os less us, of each object, NaN where there is none or it has no reference.
pub fn errors_of(o: &terraflux_ops::objects::Objects) -> Vec<f64> {
    let (Some(os), Some(us)) = (o.table.column("os"), o.table.column("us")) else {
        return vec![f64::NAN; o.labels.len()];
    };
    let per: Vec<f64> = os.iter().zip(&us).map(|(a, b)| a - b).collect();
    o.labels.iter().map(|&l| if l == 0 { f64::NAN } else { per[l as usize - 1] }).collect()
}

/// The map made to draw the errors, kept while the objects are the same (by their address).
#[derive(Default)]
struct Made {
    cache: Option<(usize, Map)>,
}

fn made(ctx: &egui::Context, evaluate: &str) -> Arc<Mutex<Made>> {
    let id = egui::Id::new(("reference lit", evaluate));
    ctx.data_mut(|d| d.get_temp_mut_or_insert_with::<Arc<Mutex<Made>>>(id, Default::default).clone())
}

/// The layer of the errors over the objects shown (or their map) when an evaluating node judges them and is asked to
/// draw them: os less us, blue (the objects swallowed what is outside the reference) to red (the reference was cut).
pub fn layers(ctx: &egui::Context, s: &Session, node: &str, port: &str) -> Vec<RasterLayer> {
    let key = (node.to_string(), if port == "map" { "objects".to_string() } else { port.to_string() });
    let Some(e) = evaluators(&s.source, &key).into_iter().find(|e| errors_shown(ctx, e)) else { return Vec::new() };
    let Some(Value::Objects(o)) = s.value_of(&e, "objects") else { return Vec::new() };
    let at = Arc::as_ptr(&o) as usize;
    let st = made(ctx, &e);
    let mut st = st.lock().unwrap_or_else(|x| x.into_inner());
    if st.cache.as_ref().is_none_or(|(k, _)| *k != at) {
        st.cache = Some((at, Map::new(o.grid.clone(), errors_of(&o))));
    }
    let Some((_, map)) = &st.cache else { return Vec::new() };
    vec![RasterLayer {
        source: MapSource::from(map),
        coloring: Coloring::Continuous { min: -1.0, max: 1.0, ramp: Ramp::Diverging, log: false },
        opacity: 0.8,
        below: false,
        name: format!("{e}: os less us"),
        blend: Default::default(),
    }]
}

/// The map that draws the errors of the evaluating node `evaluate` as it was last made.
pub fn lit(ctx: &egui::Context, evaluate: &str) -> Option<Map> {
    let st = made(ctx, evaluate);
    let st = st.lock().unwrap_or_else(|e| e.into_inner());
    st.cache.as_ref().map(|(_, m)| m.clone())
}

/// A bar of the diverging ramp with what its ends mean.
fn legend(ui: &mut Ui) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width().clamp(120.0, 220.0), 10.0), egui::Sense::hover());
    let steps = 40;
    for k in 0..steps {
        let t = k as f32 / (steps - 1) as f32;
        let x0 = rect.left() + rect.width() * k as f32 / steps as f32;
        let x1 = rect.left() + rect.width() * (k + 1) as f32 / steps as f32;
        let r = egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1, rect.bottom()));
        ui.painter().rect_filled(r, 0.0, crate::palette::seen(Ramp::Diverging.colour(t)));
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("under: swallowed").size(10.5).weak());
        ui.add_space((rect.width() - 150.0).max(8.0));
        ui.label(RichText::new("over: cut").size(10.5).weak());
    });
}

/// In the Viewer's side panel for the objects `key`: the numbers of their `segment.evaluate`, the button to draw their
/// errors over the map and, for a `segment.optimize` with no reference, the one to give it this one.
pub fn section(ui: &mut Ui, s: &mut Session, r: &Registry, key: &(String, String)) {
    let evaluating = evaluators(&s.source, key);
    if evaluating.is_empty() {
        return;
    }
    crate::theme::section(ui, "Reference objects");
    for e in &evaluating {
        ui.label(RichText::new(e).strong());
        let n = numbers(s, e);
        if n.is_empty() {
            ui.label(RichText::new("run the model to measure the segmentation").weak().size(11.0));
        } else {
            ui.horizontal_wrapped(|ui| {
                for (name, v) in &n {
                    ui.label(format!(
                        "{}: {}",
                        name.to_uppercase().replace("F_OVERLAP", "F"),
                        crate::table_view::cell(*v)
                    ));
                }
            });
            ui.label(RichText::new("0 is the best of ED2, D, OS and US; 1 of F").weak().size(10.5));
        }
        let mut on = errors_shown(ui.ctx(), e);
        if ui
            .checkbox(&mut on, "Show where it errs")
            .on_hover_text(
                "each object coloured by os less us: red where the reference was cut, blue where it was swallowed",
            )
            .changed()
        {
            show_errors(ui.ctx(), e, on);
        }
        if on {
            legend(ui);
        }
        // (an optimizer with no reference is offered this one)
        if let Some(reference) = reference_of(&s.source, e) {
            let bare: Vec<String> = bare_optimizers(&s.source).iter().map(|n| n.id.clone()).collect();
            for o in bare {
                if ui
                    .button(format!("Use as the reference of {o}"))
                    .on_hover_text("its scores gain os, us, d, ed2 and f_overlap; the objective ed2, d or f_overlap chooses by them")
                    .clicked()
                {
                    let before = s.source.clone();
                    match crate::editor::connect(&mut s.source, r, &[], &reference, &o, "reference") {
                        Ok(()) => {
                            s.history.name("reference of segment.optimize");
                            s.edited();
                        }
                        Err(m) => {
                            s.source = before;
                            s.notice = Some((format!("not connected: {m}"), std::time::Instant::now()));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::objects::Objects;
    use terraflux_raster::Grid;

    #[test]
    fn each_object_is_os_less_us() {
        let o = Objects::from_labels(Arc::new(Grid::new(4, 1)), vec![1, 1, 0, 2], 2).unwrap();
        let mut t = o.table.clone();
        t.columns.extend(["os".to_string(), "us".to_string()]);
        t.rows[0].extend([0.75, 0.25]);
        t.rows[1].extend([f64::NAN, f64::NAN]);
        let o = o.with_table(t).unwrap();
        let v = errors_of(&o);
        assert_eq!((v[0], v[1]), (0.5, 0.5));
        assert!(v[2].is_nan() && v[3].is_nan());
        // an objects table without the columns has no errors
        let bare = Objects::from_labels(Arc::new(Grid::new(2, 1)), vec![1, 1], 1).unwrap();
        assert!(errors_of(&bare).iter().all(|x| x.is_nan()));
    }
}
