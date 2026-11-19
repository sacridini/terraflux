//! The SOM view of an `ml.som` node's codebook ([`crate::charts`]) and the maps of the node (`nodes`, `groups`) in the
//! Viewer, linked: a node (or group) picked in the view lights its cells on the map (the rest veiled), and a cell
//! clicked on the map picks its node (or group) in the view. The pick is kept in the context by the node's name, so
//! it stays while the Viewer shows the other output.

use std::sync::{Arc, Mutex};

use egui::Color32;
use terraflux_ops::{Map, Value};

use crate::app::Session;
use crate::charts::{SomMap, SomPick, som_map};
use crate::map_view::{RasterLayer, Source};
use crate::palette::Coloring;

/// The value of a cell the pick leaves, and the one it covers, in the map made to light them.
const VEIL: f64 = 1.0;
const LIT: f64 = 2.0;

/// The node or group picked of the `ml.som` node `node`.
pub fn linked(ctx: &egui::Context, node: &str) -> Option<SomPick> {
    ctx.data(|d| d.get_temp::<Option<SomPick>>(egui::Id::new(("som link", node)))).flatten()
}

/// Pick a node or group of the `ml.som` node `node` (none: let go).
pub fn set_linked(ctx: &egui::Context, node: &str, pick: Option<SomPick>) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("som link", node)), pick));
}

/// The pick a cell of the map of nodes (or, `groups`, of groups) with the value `v` is.
pub fn cell_pick(v: f64, groups: bool) -> Option<SomPick> {
    if !v.is_finite() || v < 1.0 {
        return None;
    }
    let k = v as usize;
    Some(if groups { SomPick::Group(k) } else { SomPick::Node(k) })
}

/// The cells of a map of nodes (`groups` false) or of groups that a pick covers: [`LIT`], the others [`VEIL`], NaN
/// where the map has no value.
pub fn mask(values: &[f64], groups: bool, pick: SomPick, book: &SomMap) -> Vec<f64> {
    // (the group of each node by its number, looked up once for every cell)
    let top = book.nodes.iter().map(|n| n.node).max().unwrap_or(0);
    let mut group_of: Vec<Option<usize>> = vec![None; top + 1];
    for n in &book.nodes {
        group_of[n.node] = n.group;
    }
    let group_of = |k: usize| group_of.get(k).copied().flatten();
    values
        .iter()
        .map(|&v| {
            let Some(cell) = cell_pick(v, groups) else { return f64::NAN };
            let on = match (cell, pick) {
                (SomPick::Node(a), SomPick::Node(b)) | (SomPick::Group(a), SomPick::Group(b)) => a == b,
                (SomPick::Node(n), SomPick::Group(g)) => group_of(n) == Some(g),
                (SomPick::Group(g), SomPick::Node(n)) => group_of(n) == Some(g),
            };
            if on { LIT } else { VEIL }
        })
        .collect()
}

/// The map made to light a pick, kept while the pick and the map are the same (by the map's address).
#[derive(Default)]
struct Made {
    cache: Option<(Option<SomPick>, usize, Map)>,
}

fn made(ctx: &egui::Context, node: &str, port: &str) -> Arc<Mutex<Made>> {
    let id = egui::Id::new(("som lit", node, port));
    ctx.data_mut(|d| d.get_temp_mut_or_insert_with::<Arc<Mutex<Made>>>(id, Default::default).clone())
}

/// Whether `node.port` is the `nodes` or `groups` map of an `ml.som` node: which of them (groups?).
fn is_map_of(s: &Session, node: &str, port: &str) -> Option<bool> {
    let groups = match port {
        "nodes" => false,
        "groups" => true,
        _ => return None,
    };
    s.source.nodes.iter().any(|n| n.id == node && n.op == "ml.som").then_some(groups)
}

/// The layer lighting the pick over the map shown when it is the nodes or groups of an `ml.som` node: the cells the
/// pick covers in the accent, the others veiled. None without a pick.
pub fn layers(ctx: &egui::Context, s: &Session, node: &str, port: &str) -> Vec<RasterLayer> {
    let Some(groups) = is_map_of(s, node, port) else { return Vec::new() };
    let Some(pick) = linked(ctx, node) else { return Vec::new() };
    let (Some(Value::Map(map)), Some(Value::Table(book))) = (s.value_of(node, port), s.value_of(node, "codebook"))
    else {
        return Vec::new();
    };
    let Some(book) = som_map(&book) else { return Vec::new() };
    let key = Arc::as_ptr(&map.data) as usize;
    let st = made(ctx, node, port);
    let mut st = st.lock().unwrap_or_else(|e| e.into_inner());
    if st.cache.as_ref().is_none_or(|(p, k, _)| *p != Some(pick) || *k != key) {
        let lit = Map::new(map.grid.clone(), mask(&map.data, groups, pick, &book));
        st.cache = Some((Some(pick), key, lit));
    }
    let Some((_, _, lit)) = &st.cache else { return Vec::new() };
    let accent = crate::palette::seen(crate::theme::accent());
    let steps = vec![(VEIL, Color32::from_black_alpha(150)), (LIT, accent)];
    vec![RasterLayer {
        source: Source::from(lit),
        coloring: Coloring::Classes(steps),
        opacity: 1.0,
        below: false,
        name: match pick {
            SomPick::Node(k) => format!("SOM node {k}"),
            SomPick::Group(g) => format!("SOM group {g}"),
        },
        blend: Default::default(),
    }]
}

/// The map that lights the pick over `node.port` as it was last drawn: [`LIT`] where the pick covers a cell, [`VEIL`]
/// elsewhere, NaN where the output has no value.
pub fn lit(ctx: &egui::Context, node: &str, port: &str) -> Option<Map> {
    let st = made(ctx, node, port);
    let st = st.lock().unwrap_or_else(|e| e.into_inner());
    st.cache.as_ref().map(|(_, _, m)| m.clone())
}

/// A cell clicked on the map shown (`picked`: the view's last click, which is the same cell whichever output is
/// shown): when the map is the nodes or groups of an `ml.som` node, its node (or group) is the pick of the node's SOM
/// view. Called at every frame of a map, `salt` naming the view: the cell it already had when it was first called, or
/// had at the last frame (another output shown meanwhile), is not a click.
pub fn clicked(ctx: &egui::Context, s: &Session, salt: &str, node: &str, port: &str, picked: Option<(usize, usize)>) {
    let id = egui::Id::new(("som cell", salt));
    let was: Option<Option<(usize, usize)>> = ctx.data_mut(|d| d.get_temp(id));
    ctx.data_mut(|d| d.insert_temp(id, picked));
    let Some((row, col)) = picked.filter(|_| was.is_some_and(|w| w != picked)) else { return };
    let Some(groups) = is_map_of(s, node, port) else { return };
    let Some(Value::Map(map)) = s.value_of(node, port) else { return };
    let v = map.data.get(row * map.grid.width + col).copied().unwrap_or(f64::NAN);
    set_linked(ctx, node, cell_pick(v, groups));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charts::{SomNode, SomPick::*};

    fn book() -> SomMap {
        let node = |k: usize, g: usize| SomNode {
            node: k,
            row: 1,
            col: k,
            count: 1.0,
            group: Some(g),
            umatrix: 0.0,
            weights: vec![],
        };
        SomMap {
            rows: 1,
            cols: 4,
            hex: false,
            names: vec![],
            nodes: vec![node(1, 1), node(2, 1), node(3, 2), node(4, 2)],
        }
    }

    #[test]
    fn a_pick_lights_the_cells_of_its_node_or_group() {
        let nodes = [1.0, 2.0, f64::NAN, 3.0, 4.0, 2.0];
        let lit = |m: Vec<f64>| m.iter().map(|v| if v.is_nan() { -1 } else { *v as i32 }).collect::<Vec<_>>();
        assert_eq!(lit(mask(&nodes, false, Node(2), &book())), [1, 2, -1, 1, 1, 2]);
        // a group on the map of nodes: the cells of all its nodes
        assert_eq!(lit(mask(&nodes, false, Group(1), &book())), [2, 2, -1, 1, 1, 2]);
        // on the map of groups: a group, or a node's group
        let groups = [1.0, 1.0, 2.0, f64::NAN, 2.0];
        assert_eq!(lit(mask(&groups, true, Group(2), &book())), [1, 1, 2, -1, 2]);
        assert_eq!(lit(mask(&groups, true, Node(2), &book())), [2, 2, 1, -1, 1]);
    }

    #[test]
    fn a_cell_is_the_pick_of_its_node_or_group() {
        assert_eq!(cell_pick(5.0, false), Some(Node(5)));
        assert_eq!(cell_pick(2.0, true), Some(Group(2)));
        assert_eq!(cell_pick(f64::NAN, false), None);
        assert_eq!(cell_pick(0.0, true), None);
    }
}
