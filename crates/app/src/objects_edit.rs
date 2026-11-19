//! The manual edits of objects (merge, cut, delete) written into the model, in the Viewer's inspector.
//!
//! An edit is kept in an `objects.edit` node as a place (the interior cell nearest an object's centroid, as the
//! samples mode picks it; a line's vertices), not as an object number, so it holds when the segmentation above
//! changes. [`apply`] appends it to the `objects.edit` node that reads the objects, or makes one right after the
//! segmenter (the node the objects come from, walking up from a classification to the segmenter) and moves each
//! node that read the segmenter's objects onto it. One step of undo.

use terraflux_graph::{Model, Node, Source};
use terraflux_ops::Registry;
use terraflux_ops::objects::Objects;

use crate::app::Session;
use crate::objects_inspect::{centre_of, sample_cell};

/// What the inspector asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// The selected objects become the first one (the lowest id).
    Merge(Vec<u32>),
    /// The selected objects are left with no object, or taken by their most similar neighbour.
    Delete(Vec<u32>, bool),
    /// A line, in places of the objects' grid (x, y), cuts the objects it crosses.
    Cut(Vec<[f64; 2]>),
}

impl Edit {
    fn name(&self) -> String {
        match self {
            Edit::Merge(ids) => format!("merge {} objects", ids.len()),
            Edit::Delete(ids, _) => format!("delete {} objects", ids.len()),
            Edit::Cut(_) => "cut objects".into(),
        }
    }
}

/// The place of an object: the centre of the interior cell nearest its centroid.
fn place(o: &Objects, id: u32) -> Result<[f64; 2], String> {
    let (r, c) = sample_cell(o, id).ok_or_else(|| format!("no object {id} in these objects"))?;
    Ok(centre_of(&o.grid, r, c))
}

/// The edit as the entries of `objects.edit`'s `edits`.
pub fn steps(o: &Objects, e: &Edit) -> Result<Vec<serde_json::Value>, String> {
    match e {
        Edit::Merge(ids) => {
            if ids.len() < 2 {
                return Err("select two objects or more to merge".into());
            }
            let pts: Result<Vec<[f64; 2]>, String> = ids.iter().map(|&i| place(o, i)).collect();
            Ok(vec![serde_json::json!({ "merge": pts? })])
        }
        Edit::Delete(ids, into) => {
            if ids.is_empty() {
                return Err("select an object to delete".into());
            }
            let into = if *into { "neighbour" } else { "none" };
            ids.iter().map(|&i| Ok(serde_json::json!({ "delete": place(o, i)?, "into": into }))).collect()
        }
        Edit::Cut(line) => {
            if line.len() < 2 {
                return Err("a cut is a line of two points or more".into());
            }
            Ok(vec![serde_json::json!({ "cut": line })])
        }
    }
}

fn node<'a>(m: &'a Model, id: &str) -> Option<&'a Node> {
    m.nodes.iter().find(|n| n.id == id)
}

/// The node the edit goes after: walking up from the objects shown, the `objects.edit` or the `segment.*` node
/// (what is between, as a classification, is run again after the edit); the node itself when there is none.
pub fn base_of(m: &Model, start: &str) -> Option<String> {
    let mut at = start.to_string();
    for _ in 0..=m.nodes.len() {
        let n = node(m, &at)?;
        if n.op == "objects.edit" || n.op.starts_with("segment.") {
            return Some(at);
        }
        match n.inputs.get("objects") {
            Some(Source::Ref(r)) => at = r.split_once('.')?.0.to_string(),
            _ => return Some(at),
        }
    }
    None
}

/// The `objects.edit` node an edit of objects made by `base` goes to: `base` itself, or the one that reads its
/// objects.
pub fn edit_of(m: &Model, base: &str) -> Option<String> {
    let n = node(m, base)?;
    if n.op == "objects.edit" {
        return Some(n.id.clone());
    }
    let want = format!("{base}.objects");
    m.nodes
        .iter()
        .find(|e| e.op == "objects.edit" && matches!(e.inputs.get("objects"), Some(Source::Ref(r)) if *r == want))
        .map(|e| e.id.clone())
}

/// Write the edit into the model as one step of undo: appended to the `objects.edit` of the objects `key` (a node and
/// its output), or a new one made after the segmenter. A message of what was done.
pub fn apply(s: &mut Session, r: &Registry, key: &(String, String), o: &Objects, e: &Edit) -> Result<String, String> {
    let new = steps(o, e)?;
    let before = s.source.clone();
    match write(s, r, &key.0, &new) {
        Ok(msg) => {
            s.history.name(&e.name());
            s.edited();
            Ok(msg)
        }
        Err(err) => {
            s.source = before;
            Err(err)
        }
    }
}

fn write(s: &mut Session, r: &Registry, shown: &str, new: &[serde_json::Value]) -> Result<String, String> {
    let top: &[String] = &[];
    if node(&s.source, shown).is_none() {
        return Err(format!("{shown} is not a node of the model's top level: no edit can go after it"));
    }
    let base = base_of(&s.source, shown).ok_or_else(|| format!("no segmenter found above {shown}"))?;
    let (id, made) = match edit_of(&s.source, &base) {
        Some(id) => (id, false),
        None => {
            let want = format!("{base}.objects");
            let at = crate::builder::beside(&s.source.nodes, Some(&base));
            let id = crate::editor::add_node_named(&mut s.source, r, top, "objects.edit", Some("edit"), at)?;
            crate::editor::connect(&mut s.source, r, top, &want, &id, "objects")?;
            // the segmenter's image, for the table and for the similarity of a delete into a neighbour
            let image = match node(&s.source, &base).and_then(|n| n.inputs.get("image")) {
                Some(Source::Ref(img)) if !img.starts_with('$') && !img.starts_with('@') => Some(img.clone()),
                _ => None,
            };
            if let Some(img) = image {
                crate::editor::connect(&mut s.source, r, top, &img, &id, "image")?;
            }
            // each node that read the segmenter's objects reads the edited ones now
            let readers: Vec<(String, String)> = s
                .source
                .nodes
                .iter()
                .filter(|n| n.id != id)
                .flat_map(|n| {
                    n.inputs
                        .iter()
                        .filter(|(_, src)| matches!(src, Source::Ref(x) if *x == want))
                        .map(|(port, _)| (n.id.clone(), port.clone()))
                        .collect::<Vec<_>>()
                })
                .collect();
            for (n, port) in &readers {
                crate::editor::insert_between(&mut s.source, r, top, &id, &want, "objects", "objects", n, port)?;
            }
            if let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == id) {
                n.note = Some(
                    "Objects corrected by hand in the Viewer (merge, cut, delete), kept as places on the map, not as \
                     object numbers."
                        .into(),
                );
            }
            (id, true)
        }
    };
    let n = s.source.nodes.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("no node {id}"))?;
    let mut list: Vec<serde_json::Value> =
        n.params.get("edits").and_then(|v| v.as_array().cloned()).unwrap_or_default();
    list.extend(new.iter().cloned());
    let total = list.len();
    n.params.insert("edits".into(), list.into());
    Ok(format!(
        "{} written in {}{id} ({total} edits): run the model to see them",
        if new.len() == 1 { "1 edit".to_string() } else { format!("{} edits", new.len()) },
        if made { "the new node " } else { "" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An image, its segmentation, features of the objects, and a classification of them.
    fn session() -> Session {
        let mut m = Model::new("edit");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 12).param("columns", 12).param("seed", 1));
        m.nodes.push(Node::new("seg", "segment.chessboard").input("image", "land.map").param("size", 4));
        m.nodes.push(Node::new("feat", "objects.features").input("objects", "seg.objects").input("image", "land.map"));
        m.nodes.push(Node::new("to_map", "objects.to_map").input("objects", "seg.objects"));
        m.nodes.push(
            Node::new("rules", "objects.rules")
                .input("objects", "feat.objects")
                .param("classes", serde_json::json!([{ "name": "big", "rule": "above(cells, 1, 4)" }])),
        );
        Session::from_model(m).unwrap()
    }

    /// The objects the chessboard gives (3 x 3 blocks of 4 x 4 cells).
    fn blocks() -> Objects {
        let labels: Vec<u32> = (0..144).map(|i| ((i / 12) / 4 * 3 + (i % 12) / 4 + 1) as u32).collect();
        Objects::from_labels(std::sync::Arc::new(terraflux_raster::Grid::new(12, 12)), labels, 9).unwrap()
    }

    fn node_in<'a>(s: &'a Session, id: &str) -> &'a Node {
        s.source.nodes.iter().find(|n| n.id == id).unwrap()
    }

    #[test]
    fn the_steps_are_places_inside_the_objects() {
        let o = blocks();
        let v = steps(&o, &Edit::Merge(vec![1, 2])).unwrap();
        let pts = v[0]["merge"].as_array().unwrap();
        assert_eq!(pts.len(), 2);
        // (the interior cells nearest the centroids: inside blocks 1 and 2 of the top row, y down)
        let p = |k: usize| (pts[k][0].as_f64().unwrap(), pts[k][1].as_f64().unwrap());
        assert!(p(0).0 > 0.0 && p(0).0 < 4.0 && p(0).1 < 0.0 && p(0).1 > -4.0, "{:?}", p(0));
        assert!(p(1).0 > 4.0 && p(1).0 < 8.0, "{:?}", p(1));
        let d = steps(&o, &Edit::Delete(vec![3, 4], true)).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0]["into"], "neighbour");
        assert!(steps(&o, &Edit::Merge(vec![1])).is_err());
        assert!(steps(&o, &Edit::Merge(vec![1, 99])).is_err());
        assert!(steps(&o, &Edit::Cut(vec![[0.0, 0.0]])).is_err());
    }

    #[test]
    fn the_node_goes_between_the_segmenter_and_its_readers_and_undo_takes_it_away_in_one_step() {
        let r = Registry::standard();
        let mut s = session();
        let before = s.source.clone();
        let o = blocks();
        let msg = apply(&mut s, &r, &("seg".into(), "objects".into()), &o, &Edit::Merge(vec![1, 2])).unwrap();
        assert!(msg.contains("new node") && msg.contains("edit"), "{msg}");
        assert_eq!(s.source.nodes.len(), before.nodes.len() + 1);
        let e = node_in(&s, "edit");
        assert_eq!(e.op, "objects.edit");
        assert_eq!(e.inputs.get("objects"), Some(&Source::Ref("seg.objects".into())));
        assert_eq!(e.inputs.get("image"), Some(&Source::Ref("land.map".into())));
        assert_eq!(e.params["edits"].as_array().unwrap().len(), 1);
        // the readers of the segmenter read the edit now
        assert_eq!(node_in(&s, "feat").inputs.get("objects"), Some(&Source::Ref("edit.objects".into())));
        assert_eq!(node_in(&s, "to_map").inputs.get("objects"), Some(&Source::Ref("edit.objects".into())));
        assert!(e.pos.is_some());
        assert!(s.history.can_undo());
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, before, "one undo puts the model back");
        crate::builder::undo_redo(&mut s, true);
        assert!(s.source.nodes.iter().any(|n| n.id == "edit"), "and redo brings it back");
    }

    #[test]
    fn a_second_edit_is_appended_to_the_node_that_reads_the_objects_and_undone_in_one_step() {
        let r = Registry::standard();
        let mut s = session();
        let o = blocks();
        // (shown from the classification: the edit still goes after the segmenter)
        apply(&mut s, &r, &("rules".into(), "objects".into()), &o, &Edit::Merge(vec![1, 2])).unwrap();
        // (a frame ends: the next edit is a step of its own)
        s.history.tick(&s.source, false);
        assert_eq!(node_in(&s, "edit").inputs.get("objects"), Some(&Source::Ref("seg.objects".into())));
        let after_first = s.source.clone();
        let msg = apply(&mut s, &r, &("seg".into(), "objects".into()), &o, &Edit::Delete(vec![5], false)).unwrap();
        assert!(!msg.contains("new node"), "{msg}");
        assert_eq!(s.source.nodes.len(), after_first.nodes.len(), "no second node");
        let list = node_in(&s, "edit").params["edits"].as_array().unwrap().clone();
        assert_eq!(list.len(), 2);
        s.history.tick(&s.source, false);
        assert!(list[0].get("merge").is_some() && list[1].get("delete").is_some());
        // also when shown from the edit node itself
        apply(&mut s, &r, &("edit".into(), "objects".into()), &o, &Edit::Cut(vec![[1.0, -1.0], [5.0, -1.0]])).unwrap();
        assert_eq!(node_in(&s, "edit").params["edits"].as_array().unwrap().len(), 3);
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(node_in(&s, "edit").params["edits"].as_array().unwrap().len(), 2);
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, after_first, "one undo takes back one edit");
    }

    #[test]
    fn an_edit_that_cannot_be_written_leaves_the_model_as_it_was() {
        let r = Registry::standard();
        let mut s = session();
        let before = s.source.clone();
        let o = blocks();
        assert!(apply(&mut s, &r, &("nothing".into(), "objects".into()), &o, &Edit::Merge(vec![1, 2])).is_err());
        assert!(apply(&mut s, &r, &("seg".into(), "objects".into()), &o, &Edit::Merge(vec![1])).is_err());
        assert_eq!(s.source, before);
        assert!(!s.history.can_undo());
    }
}
