//! The Plots of a model: views of its results kept with it (`[[plots]]`), outside the graph. A Plot is what the
//! Viewer shows saved: its layers (maps and vector features, the first at the bottom, each as opaque as asked, in
//! its palette, band and style), a map compared with them, a table's chart, a title and the record of why. The
//! Document, the report, the QGIS project and the Python script read them.
//!
//! A model of an older Terraflux had `view.plot` nodes instead; [`Model::plots_from_nodes`] makes them Plots.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{Model, Node, Ref, Source, setting_ref_mut};

/// A view of the model's results kept with it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plot {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// What it shows, the first at the bottom.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<PlotLayer>,
    /// A map compared with the top map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare: Option<PlotCompare>,
    /// A table's chart (a layer that is a table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<PlotChart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// A layer of a Plot: an output (`node.port`) or an input (`$name`) of the model, and how it is drawn.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlotLayer {
    pub from: String,
    /// How opaque, 0 to 1 (default 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// The palette (by its name in the Viewer; default: by what the map is).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// The band of a stack or the step of a loop shown (default the first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub band: Option<usize>,
    /// Vector features' style (as the Viewer keeps it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<serde_json::Value>,
    /// Drawn as a difference: `from` minus this map (`node.port`) for maps of values, the transitions from this map
    /// to `from` for maps of classes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minus: Option<String>,
    /// Kept in the Plot without being drawn.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// A map compared with a Plot's top map, and how: `curtain`, `side by side` (default) or `difference`; side by
/// side, up to two more maps, each in a view of its own (`also`: the views move together).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlotCompare {
    pub with: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub how: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<String>,
}

/// A table's chart: `bars`, `lines`, `points` or `boxes` (default: suggested), the column along x, the columns
/// drawn (comma-separated), the column whose values make the groups.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlotChart {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

impl PlotLayer {
    pub fn new(from: &str) -> PlotLayer {
        PlotLayer { from: from.to_string(), ..Default::default() }
    }
}

/// The operator the Plots take the place of.
pub const PLOT_OP: &str = "view.plot";

impl Model {
    /// The Plot `id`.
    pub fn plot(&self, id: &str) -> Option<&Plot> {
        self.plots.iter().find(|p| p.id == id)
    }

    /// The `view.plot` nodes of the top level made Plots (a model of an older Terraflux): each one's layers its
    /// `under` chain's then its own map and features (or its table, with its chart), its `compared` and
    /// `compare`, its title, note and why. The nodes go; what read a plot's `shown` reads what it showed (an input
    /// that read only a plot of nothing taken away). Plots inside containers stay nodes. What was made, in the
    /// notes returned.
    pub fn plots_from_nodes(&mut self) -> Vec<String> {
        fn inner_ids(nodes: &[Node], out: &mut BTreeSet<String>) {
            for n in nodes {
                if let Some(b) = &n.body {
                    for m in &b.nodes {
                        out.insert(m.id.clone());
                    }
                    inner_ids(&b.nodes, out);
                }
            }
        }
        let mut inner = BTreeSet::new();
        inner_ids(&self.nodes, &mut inner);
        let plots: BTreeMap<String, Node> = self
            .nodes
            .iter()
            .filter(|n| n.op == PLOT_OP && !inner.contains(&n.id) && !self.plots.iter().any(|p| p.id == n.id))
            .map(|n| (n.id.clone(), n.clone()))
            .collect();
        if plots.is_empty() {
            return Vec::new();
        }
        // what a plot's `shown` is: what it shows (through the plots it shows), none for a plot of nothing
        fn shown(plots: &BTreeMap<String, Node>, id: &str, seen: &mut Vec<String>) -> Option<String> {
            if seen.iter().any(|s| s == id) {
                return None;
            }
            seen.push(id.to_string());
            let n = plots.get(id)?;
            let r = ["value", "features", "table"].iter().find_map(|p| match n.inputs.get(*p) {
                Some(Source::Ref(r)) => Some(r.clone()),
                _ => None,
            })?;
            through(plots, &r, seen)
        }
        // a reference, through the plots it reads
        fn through(plots: &BTreeMap<String, Node>, r: &str, seen: &mut Vec<String>) -> Option<String> {
            match Ref::parse(r) {
                Ok(Ref::Port { node, port }) if port == "shown" && plots.contains_key(&node) => {
                    shown(plots, &node, seen)
                }
                _ => Some(r.to_string()),
            }
        }
        let resolve = |r: &str| through(&plots, r, &mut Vec::new());
        // the layers of a plot, the lowest first: its `under` chain's, then its own
        fn layers(plots: &BTreeMap<String, Node>, id: &str, seen: &mut Vec<String>, out: &mut Vec<PlotLayer>) {
            if seen.iter().any(|s| s == id) {
                return;
            }
            seen.push(id.to_string());
            let Some(n) = plots.get(id) else { return };
            let input = |p: &str| match n.inputs.get(p) {
                Some(Source::Ref(r)) => Some(r.clone()),
                _ => None,
            };
            if let Some(u) = input("under") {
                match Ref::parse(&u) {
                    Ok(Ref::Port { node, .. }) if plots.contains_key(&node) => layers(plots, &node, seen, out),
                    _ => out.push(PlotLayer::new(&u)),
                }
            }
            let opacity = n.params.get("opacity").and_then(|v| v.as_f64()).filter(|o| *o != 1.0);
            if let Some(v) = input("value").and_then(|r| through(plots, &r, &mut Vec::new())) {
                out.push(PlotLayer { opacity, ..PlotLayer::new(&v) });
            }
            for p in ["features", "table"] {
                if let Some(v) = input(p).and_then(|r| through(plots, &r, &mut Vec::new())) {
                    out.push(PlotLayer::new(&v));
                }
            }
        }
        let text = |n: &Node, k: &str| n.params.get(k).and_then(|v| v.as_str()).map(String::from);
        let mut notes = Vec::new();
        for n in self.nodes.iter().filter(|n| plots.contains_key(&n.id)) {
            let mut ls = Vec::new();
            layers(&plots, &n.id, &mut Vec::new(), &mut ls);
            let compare = match n.inputs.get("compared") {
                Some(Source::Ref(r)) => resolve(r).map(|with| PlotCompare {
                    with,
                    how: text(n, "compare").filter(|h| !h.is_empty()),
                    also: Vec::new(),
                }),
                _ => None,
            };
            let chart = PlotChart { kind: text(n, "chart"), x: text(n, "x"), y: text(n, "y"), by: text(n, "by") };
            let chart = (chart != PlotChart::default()).then_some(chart);
            self.plots.push(Plot {
                id: n.id.clone(),
                title: text(n, "title").filter(|t| !t.is_empty()),
                layers: ls,
                compare,
                chart,
                note: n.note.clone(),
                why: n.why.clone(),
            });
            notes.push(format!("node {:?}: a view.plot, kept now as the Plot {:?} (shown in the Viewer)", n.id, n.id));
        }
        self.nodes.retain(|n| !plots.contains_key(&n.id));
        // what read a plot reads what it showed
        let fix = |r: &mut String| -> bool {
            match Ref::parse(r) {
                Ok(Ref::Port { node, .. }) if plots.contains_key(&node) => match resolve(r) {
                    Some(to) => {
                        *r = to;
                        true
                    }
                    None => false,
                },
                _ => true,
            }
        };
        fn walk(nodes: &mut [Node], fix: &dyn Fn(&mut String) -> bool) {
            for n in nodes {
                n.inputs.retain(|_, s| match s {
                    Source::Ref(r) => fix(r),
                    Source::Literal(_) => true,
                });
                for v in n.params.values_mut() {
                    if let Some(r) = setting_ref_mut(v) {
                        fix(r);
                    }
                }
                if let Some(b) = &mut n.body {
                    walk(&mut b.nodes, fix);
                    b.carry.retain(|_, r| fix(r));
                    b.collect.retain(|_, r| fix(r));
                    b.outputs.retain(|_, r| fix(r));
                    if b.until.as_mut().is_some_and(|u| !fix(u)) {
                        b.until = None;
                    }
                }
            }
        }
        walk(&mut self.nodes, &fix);
        self.outputs.retain(|_, o| fix(&mut o.from));
        self.tests.retain_mut(|t| fix(&mut t.from));
        for p in &mut self.plots {
            p.layers.retain_mut(|l| fix(&mut l.from) && l.minus.as_mut().is_none_or(fix));
            if p.compare.as_mut().is_some_and(|c| !fix(&mut c.with)) {
                p.compare = None;
            }
            if let Some(c) = &mut p.compare {
                c.also.retain_mut(|a| fix(a));
            }
        }
        // (the parameters and scenarios of a plot's settings: a title is no scenario)
        let of_plot = |k: &str| k.split_once('.').is_some_and(|(n, _)| plots.contains_key(n));
        self.parameters.retain(|p| !of_plot(&p.key));
        for s in &mut self.scenarios {
            s.values.retain(|k, _| !of_plot(k));
        }
        notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        let mut m = Model::new("plots");
        m.inputs.insert("land".into(), crate::Input::Value { value: serde_json::json!(1), note: None });
        m.nodes.push(Node::new("cover", "raster.read").param("path", "cover.tif"));
        m.nodes.push(Node::new("roads", "vector.read").param("path", "roads.gpkg"));
        m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "cover.raster"));
        m.nodes.push(Node::new("base", PLOT_OP).input("value", "cover.raster").param("title", "The cover"));
        m.nodes.push(
            Node::new("top", PLOT_OP)
                .input("value", "mspa.map")
                .input("features", "roads.vector")
                .input("under", "base.shown")
                .input("compared", "cover.raster")
                .param("opacity", 0.5)
                .param("compare", "curtain")
                .note("MSPA over the cover"),
        );
        m.nodes.push(Node::new("metrics", "metrics.class").input("map", "top.shown"));
        m.nodes.push(
            Node::new("chart", PLOT_OP).input("table", "metrics.table").param("chart", "bars").param("x", "class"),
        );
        m.outputs
            .insert("shown".into(), crate::Output { from: "top.shown".into(), path: None, dtype: None, layer: None });
        m
    }

    #[test]
    fn plot_nodes_become_plots() {
        let mut m = model();
        let notes = m.plots_from_nodes();
        assert_eq!(notes.len(), 3);
        assert!(m.nodes.iter().all(|n| n.op != PLOT_OP));
        let ids: Vec<&str> = m.plots.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["base", "top", "chart"]);
        let base = m.plot("base").unwrap();
        assert_eq!(base.title.as_deref(), Some("The cover"));
        assert_eq!(base.layers, [PlotLayer::new("cover.raster")]);
        let top = m.plot("top").unwrap();
        assert_eq!(
            top.layers,
            [
                PlotLayer::new("cover.raster"),
                PlotLayer { opacity: Some(0.5), ..PlotLayer::new("mspa.map") },
                PlotLayer::new("roads.vector"),
            ]
        );
        assert_eq!(
            top.compare,
            Some(PlotCompare { with: "cover.raster".into(), how: Some("curtain".into()), also: Vec::new() })
        );
        assert_eq!(top.note.as_deref(), Some("MSPA over the cover"));
        let chart = m.plot("chart").unwrap();
        assert_eq!(chart.layers, [PlotLayer::new("metrics.table")]);
        assert_eq!(chart.chart.as_ref().unwrap().kind.as_deref(), Some("bars"));
        // what read a plot reads what it showed
        let metrics = m.node("metrics").unwrap();
        assert_eq!(metrics.inputs.get("map"), Some(&Source::Ref("mspa.map".into())));
        assert_eq!(m.outputs["shown"].from, "mspa.map");
        // once only
        assert!(m.plots_from_nodes().is_empty());
        assert_eq!(m.plots.len(), 3);
    }

    #[test]
    fn plots_are_kept_in_the_file() {
        let mut m = model();
        m.plots_from_nodes();
        let text = m.to_toml().unwrap();
        assert!(text.contains("[[plots]]"), "{text}");
        let back = Model::from_toml(&text).unwrap();
        assert_eq!(back.plots, m.plots);
        // a model without plots writes none
        assert!(!Model::new("none").to_toml().unwrap().contains("plots"));
    }

    #[test]
    fn a_plot_of_nothing_and_one_inside_a_group() {
        let mut m = Model::new("plots");
        m.nodes.push(Node::new("empty", PLOT_OP));
        m.nodes.push(Node::new("reads", "raster.calc").input("a", "empty.shown"));
        let body = crate::Body { nodes: vec![Node::new("inner", PLOT_OP)], ..Default::default() };
        m.nodes.push(Node::new("group", "control.group").body(body));
        m.plots_from_nodes();
        assert_eq!(m.plots.len(), 1);
        assert!(m.plots[0].layers.is_empty());
        assert!(m.node("reads").unwrap().inputs.is_empty(), "the input of a plot of nothing taken away");
        let group = m.node("group").unwrap();
        assert_eq!(group.body.as_ref().unwrap().nodes[0].op, PLOT_OP, "a plot inside a group stays a node");
    }
}
