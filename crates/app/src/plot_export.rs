//! A saved Plot drawn without the application: its layers read from the outputs of a run, coloured as the Viewer
//! would colour them from the Plot alone (the layer's palette, else the one the output's producer asks for, the
//! classes' colours of the model's legend), and written as a PNG or an RGBA GeoTIFF as *Export the view* does. For
//! `terraflux plot` and the MCP tool `export_plot`.

use std::path::Path;

use terraflux_graph::{Model, Plot};
use terraflux_ops::Value;

use crate::builder::{default_palette, layer_source, look_of, output_name, palette_named};
use crate::export_view::{Format, MAX_SIDE, Options, save_with, size_of};
use crate::figure::Panel;
use crate::map_view::{RasterLayer, Source};
use crate::palette::Coloring;

/// A Plot as a panel, and what of it was drawn.
pub struct Made {
    pub panel: Panel,
    /// The views beside it when the Plot compares side by side (the PNG has them, the GeoTIFF does not).
    pub beside: Vec<Panel>,
    /// The layers drawn, bottom first, as they are named in the Viewer (with their opacity when not 1).
    pub drawn: Vec<String>,
    /// The layers not drawn and why.
    pub skipped: Vec<String>,
    /// Said when the Plot compares its map with another (side by side: the views beside the map; else only the map).
    pub compare: Option<String>,
    /// Said when the Plot has a saved view (only that part of the map is drawn).
    pub framed: Option<String>,
}

/// What a layer is, once its value is read: a map (with the difference it is and its name, when it is one) or
/// features.
enum Kind {
    Map(Source, Option<Box<(crate::difference::Made, String)>>),
    Features(std::sync::Arc<terraflux_raster::vector::Vector>),
}

/// Where a layer's value is: an input of the model, or an output of the run (the node or the top-level container
/// that keeps it, and the port).
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    Input(String),
    Output(String, String),
}

impl Place {
    /// The text the run's values are asked by: `$name`, `node.port`.
    pub fn key(&self) -> String {
        match self {
            Place::Input(n) => format!("${n}"),
            Place::Output(n, p) => format!("{n}.{p}"),
        }
    }

    /// How a layer names it: the output's name in the Viewer, an input as `$name`.
    fn name(&self) -> String {
        match self {
            Place::Input(n) => format!("${n}"),
            Place::Output(n, p) => output_name(n, p),
        }
    }
}

/// Where the value of a layer's `from` is. `$name` is an input; `node.port` an output of a node of the top level; a
/// node inside containers as the Viewer keeps it: the top-level container's id and then `inner.port`
/// (`group.inner.port`, a group in it `group.mid.inner.port`, as [`crate::builder::shown_at`]), or by its place,
/// `group/inner.port` (a switch's branch and a loop's body too: the run keeps what they gave as the Viewer shows
/// it).
pub fn place_of(m: &Model, from: &str) -> Option<Place> {
    if let Some(name) = from.strip_prefix('$') {
        return terraflux_graph::valid_id(name).then(|| Place::Input(name.to_string()));
    }
    let valid = |s: &str| !s.is_empty() && s.split('.').all(terraflux_graph::valid_id);
    if from.contains('/') {
        // (a place: the containers, the node, then the port after the last dot)
        let (place, port) = from.rsplit_once('.')?;
        let mut path: Vec<String> = place.split('/').map(String::from).collect();
        let node = path.pop()?;
        if !path.iter().chain([&node]).all(|s| terraflux_graph::valid_id(s)) || !terraflux_graph::valid_id(port) {
            return None;
        }
        let (n, p) = crate::trace::shown_key(m, &path, &node, port)?;
        return Some(Place::Output(n, p));
    }
    let (node, port) = from.split_once('.')?;
    (terraflux_graph::valid_id(node) && valid(port)).then(|| Place::Output(node.to_string(), port.to_string()))
}

/// The value of a place in a run: an input by its name, an output of a top-level node (or of a container: what
/// its body's nodes gave is kept in it as `inner.port`).
pub fn lookup(
    inputs: &std::collections::BTreeMap<String, Value>,
    results: &terraflux_exec::Results,
    key: &str,
) -> Option<Value> {
    match key.strip_prefix('$') {
        Some(name) => inputs.get(name).cloned(),
        None => {
            let (node, port) = key.split_once('.')?;
            results.values.get(node)?.get(port).cloned()
        }
    }
}

/// The panel of the Plot `id` of a model. `value` gives the value of a layer's `from` (`node.port` of the run's
/// outputs, `$name` of its inputs).
pub fn panel_of(m: &Model, id: &str, value: &dyn Fn(&str) -> Option<Value>) -> Result<Made, String> {
    made(m, find(m, id)?, value)
}

/// The Plot `id` of a model, else what the model has (before a run that would be for nothing).
pub fn find<'a>(m: &'a Model, id: &str) -> Result<&'a Plot, String> {
    m.plot(id).ok_or_else(|| {
        let ids: Vec<&str> = m.plots.iter().map(|p| p.id.as_str()).collect();
        if ids.is_empty() {
            format!("the model has no plot {id:?} (it has no plots)")
        } else {
            format!("the model has no plot {id:?} (its plots: {})", ids.join(", "))
        }
    })
}

/// The panel a list of layers makes (the first map the map, the others over it, the features over all) and the
/// layers drawn; what is left out, with the reason, goes to `skipped`. `id` names the plot in what is said.
fn panel_from(
    m: &Model,
    id: &str,
    layers: &[terraflux_graph::PlotLayer],
    value: &dyn Fn(&str) -> Option<Value>,
    skipped: &mut Vec<String>,
) -> Result<(Panel, Vec<String>), String> {
    // the layers that are drawn, bottom first: (the layer, where it is from, what it is)
    let mut kept: Vec<(&terraflux_graph::PlotLayer, String, String, Kind)> = Vec::new();
    // a layer's look (its palette, range and legend), and the legend's classes
    let looked = |l: Option<&terraflux_graph::PlotLayer>, node: &str, port: &str, src: &Source| {
        let palette = l
            .and_then(|l| l.palette.as_deref())
            .and_then(palette_named)
            .unwrap_or_else(|| default_palette(&m.nodes, node, port, Some(src)));
        let look = crate::builder::ranged(look_of(src, palette), l.and_then(|l| l.range).map(|[a, b]| (a, b)));
        let legend = crate::legend_view::legend_in(&m.nodes, node, port, src);
        let look = match &legend {
            Some(c) => crate::legend_view::apply(&look, c),
            None => look,
        };
        (look, legend)
    };
    for l in layers {
        if l.hidden {
            skipped.push(format!("{}: hidden", l.from));
            continue;
        }
        let Some(place) = place_of(m, &l.from) else {
            skipped.push(format!("{}: not an output or an input of the model", l.from));
            continue;
        };
        let (node, port) = match &place {
            Place::Output(n, p) => (n.clone(), p.clone()),
            Place::Input(n) => (String::new(), n.clone()),
        };
        let Some(v) = value(&place.key()) else {
            skipped.push(format!("{}: no value in the run", l.from));
            continue;
        };
        // a difference: the layer's map minus (values) or from (classes) the map it is of, made here as the
        // Viewer makes it in a thread
        if let Some(minus) = &l.minus {
            let made = (|| {
                let from = place_of(m, minus).ok_or_else(|| format!("{minus} is not an output or an input"))?;
                let other = value(&from.key()).ok_or_else(|| format!("{minus} has no value in the run"))?;
                let (a, b) = (
                    layer_source(&other, l.minus_band).ok_or_else(|| format!("{minus} is not a map"))?,
                    layer_source(&v, l.band).ok_or_else(|| format!("{} is not a map", l.from))?,
                );
                let (an, ap) = match &from {
                    Place::Output(n, p) => (n.as_str(), p.as_str()),
                    Place::Input(n) => ("", n.as_str()),
                };
                let (la, legend_a) = looked(None, an, ap, &a);
                let (lb, legend_b) = looked(Some(l), &node, &port, &b);
                // (of two maps of classes the transitions, named as the Viewer names them)
                let classes = match (&la.coloring, &lb.coloring) {
                    (Coloring::Classes(ca), Coloring::Classes(cb)) => Some((
                        crate::builder::class_names(ca, &la, legend_a.as_deref()),
                        crate::builder::class_names(cb, &lb, legend_b.as_deref()),
                    )),
                    _ => None,
                };
                let d = crate::difference::make(&a, &b, classes.as_ref().map(|(x, y)| (x, y)))?;
                Ok::<_, String>((from, d))
            })();
            match made {
                Ok((from, d)) => {
                    let name = if d.names.is_empty() {
                        format!("{} minus {}", place.name(), from.name())
                    } else {
                        format!("{} to {}", from.name(), place.name())
                    };
                    kept.push((l, node, port, Kind::Map(d.source.clone(), Some(Box::new((d, name))))));
                }
                Err(e) => skipped.push(format!("{}: the difference with {minus}: {e}", l.from)),
            }
            continue;
        }
        match &v {
            Value::Vector(f) => kept.push((l, node, port, Kind::Features(f.clone()))),
            other => {
                // (a composite of three maps: its ranges worked out here, there is no window to keep waiting)
                let composite = l.rgb.as_ref().map(crate::rgb::Rgb::of_plot).and_then(|rgb| {
                    let (maps, names) = crate::builder::maps_of(other);
                    rgb.source_now(&maps, &names)
                });
                match composite.or_else(|| layer_source(other, l.band)) {
                    Some(src) => kept.push((l, node, port, Kind::Map(src, None))),
                    None => skipped.push(format!("{}: not a map or features", l.from)),
                }
            }
        }
    }
    // the map is the bottom one (opaque, its grid the picture's); the other maps go over it, each at its opacity
    // (the Viewer's map shown takes no opacity of its own, so a Plot's top map would hide the rest)
    let first = kept
        .iter()
        .position(|(.., k)| matches!(k, Kind::Map(..)))
        .ok_or_else(|| format!("the plot {id:?} has no map to draw (only features or nothing)"))?;
    let colour = |l: &terraflux_graph::PlotLayer,
                  node: &str,
                  port: &str,
                  src: &Source,
                  diff: Option<&(crate::difference::Made, String)>| {
        // (a difference: the colours and the transitions' names it made)
        if let Some((d, _)) = diff {
            return (d.coloring.clone(), d.names.clone());
        }
        // (a composite of three maps: its pixels are colours already)
        if matches!(src, Source::Rgb(_)) {
            return (Coloring::Packed, Vec::new());
        }
        let (look, legend) = looked(Some(l), node, port, src);
        let names = match &look.coloring {
            Coloring::Classes(c) => {
                c.iter().map(|(v, _)| (*v, crate::legend_view::name(&look, legend.as_deref(), *v))).collect()
            }
            Coloring::Continuous { .. } | Coloring::Packed | Coloring::Shade { .. } => Vec::new(),
        };
        (look.coloring, names)
    };
    let (l, node, port, Kind::Map(source, diff)) = &kept[first] else { unreachable!("the first map") };
    let (coloring, names) = colour(l, node, port, source, diff.as_deref());
    let source = source.clone();
    let mut drawn = Vec::new();
    let mut rasters = Vec::new();
    let mut vectors = Vec::new();
    let at = |name: String, l: &terraflux_graph::PlotLayer| {
        let name = match l.opacity {
            Some(o) if (o - 1.0).abs() > 1e-9 => format!("{name} at {:.0}%", o * 100.0),
            _ => name,
        };
        match crate::blend::Blend::of_plot(l.blend.as_deref()) {
            crate::blend::Blend::Normal => name,
            b => format!("{name}, {}", b.key()),
        }
    };
    // a layer's shade, as a layer over it (black as dark as the relief; none for a composite of three maps)
    let shade_of = |l: &terraflux_graph::PlotLayer, src: &Source, name: &str| {
        let shade = crate::shade::Shade::of_plot(l.shade.as_ref()?);
        let source = crate::shade::ShadeSource::of(src, &shade)?;
        let opacity = l.opacity.unwrap_or(1.0).clamp(0.0, 1.0) as f32;
        let layer = RasterLayer {
            source,
            coloring: shade.coloring(),
            opacity,
            below: false,
            name: String::new(),
            blend: Default::default(),
        };
        Some((layer, format!("{name}: {}", shade.label())))
    };
    for (i, (l, node, port, k)) in kept.iter().enumerate() {
        let name = if node.is_empty() { format!("${port}") } else { output_name(node, port) };
        match k {
            Kind::Map(src, diff) if i == first => {
                let name = diff.as_deref().map_or(name, |(_, n)| n.clone());
                drawn.push(match l.opacity {
                    Some(o) if o < 1.0 => format!("{name} (the map, drawn opaque)"),
                    _ => format!("{name} (the map)"),
                });
                // (its shade over it; the map is the base, so it is mixed with none)
                if let Some((shade, line)) = shade_of(l, src, &name).filter(|_| diff.is_none()) {
                    drawn.push(line);
                    rasters.push(shade);
                }
            }
            Kind::Map(src, diff) => {
                let (coloring, _) = colour(l, node, port, src, diff.as_deref());
                let name = diff.as_deref().map_or(name, |(_, n)| n.clone());
                let opacity = l.opacity.unwrap_or(1.0).clamp(0.0, 1.0) as f32;
                let blend = crate::blend::Blend::of_plot(l.blend.as_deref());
                drawn.push(at(name.clone(), l));
                let shade = shade_of(l, src, &name).filter(|_| diff.is_none());
                rasters.push(RasterLayer { source: src.clone(), coloring, opacity, below: false, name, blend });
                if let Some((shade, line)) = shade {
                    drawn.push(line);
                    rasters.push(shade);
                }
            }
            Kind::Features(f) => {
                let style = l.style.as_ref().map(crate::vector_tiles::Style::from_json).unwrap_or_default();
                match crate::builder::figure_vector(f, &style, source.grid()) {
                    Some(v) => {
                        drawn.push(format!("{name} (features)"));
                        vectors.push(v);
                    }
                    None => skipped
                        .push(format!("{}: its features could not be put on the map's coordinate system", l.from)),
                }
            }
        }
    }
    Ok((Panel { source, coloring, names, caption: String::new(), vectors, rasters }, drawn))
}

/// Whether a Plot's comparison puts the maps side by side (as the Viewer takes it: unless it says curtain or
/// difference).
fn side_by_side(c: &terraflux_graph::PlotCompare) -> bool {
    !matches!(c.how.as_deref().map(str::trim), Some("curtain" | "difference"))
}

fn made(m: &Model, plot: &Plot, value: &dyn Fn(&str) -> Option<Value>) -> Result<Made, String> {
    let mut skipped = Vec::new();
    let (mut panel, drawn) = panel_from(m, &plot.id, &plot.layers, value, &mut skipped)?;
    // the views beside the map when the Plot compares side by side: each the map of `with` (and `also`) in the
    // map's colours with the Plot's layers over it, or the layers of its own
    let mut beside = Vec::new();
    if let Some(c) = plot.compare.as_ref().filter(|c| side_by_side(c)) {
        for (i, with) in std::iter::once(&c.with).chain(&c.also).enumerate() {
            let own = c.layers.get(i).filter(|l| !l.is_empty());
            let view = match own {
                Some(list) => {
                    let mut left_out = Vec::new();
                    let r = panel_from(m, &plot.id, list, value, &mut left_out).map(|(p, _)| p);
                    skipped.extend(left_out.into_iter().map(|t| format!("the view of {with}: {t}")));
                    r
                }
                None => {
                    // (the band: the first when none is chosen)
                    let band = if i == 0 { c.band } else { c.also_bands.get(i - 1).copied().filter(|b| *b > 0) };
                    place_of(m, with)
                        .and_then(|p| value(&p.key()))
                        .and_then(|v| layer_source(&v, band))
                        .map(|source| Panel { source, ..panel.clone() })
                        .ok_or_else(|| format!("{with} is not a map with a value in the run"))
                }
            };
            match view {
                Ok(p) => beside.push(p),
                Err(e) => skipped.push(format!("{with}: not drawn beside the map: {e}")),
            }
        }
    }
    let compare = plot.compare.as_ref().map(|c| {
        let how = c.how.as_deref().filter(|h| !h.trim().is_empty()).unwrap_or("side by side");
        if side_by_side(c) && !beside.is_empty() {
            format!(
                "the plot compares its map with {} ({how}): the PNG has the {} views side by side, the GeoTIFF only the map (it has one grid)",
                c.with,
                1 + beside.len()
            )
        } else {
            format!("the plot compares its map with {} ({how}): only the map is drawn", c.with)
        }
    });
    // the saved view: only that part of the map (the vectors and the other maps follow its grid)
    let framed = match &plot.view {
        Some(v) if crate::frame::cells_of(panel.source.grid(), v.extent).is_some() => {
            panel = crate::frame::crop_panel(&panel, v.extent)?;
            beside = beside.into_iter().map(|p| crate::frame::crop_panel(&p, v.extent).unwrap_or(p)).collect();
            let (w, h) = panel.source.size();
            Some(format!("the plot's saved view: only that part of the map is drawn ({w} x {h} cells)"))
        }
        Some(_) => Some("the plot's saved view is outside the map: the whole map is drawn".to_string()),
        None => None,
    };
    Ok(Made { panel, beside, drawn, skipped, compare, framed })
}

/// The Plot written to `path` (a picture of the map, `scale` pixels to a cell); what to tell of it: the file, its
/// size and cells to a pixel, the layers drawn and not drawn.
pub fn write(m: &Made, scale: usize, format: Format, path: &Path) -> Result<String, String> {
    let scale = scale.max(1);
    let options = Options { beside: m.beside.clone(), ..Options::scaled(scale) };
    let said = save_with(&m.panel, &options, format, path)?;
    let (w, h) = m.panel.source.size();
    let (_, coarse) = size_of((w, h), scale);
    let mut s = said;
    if coarse {
        let step = (w.max(h) * scale).div_ceil(MAX_SIDE).max(1);
        s.push_str(&format!("; the map of {w} x {h} cells, a pixel for every {step} cells of a side"));
    } else {
        s.push_str(&format!(
            "; the map of {w} x {h} cells, {scale} pixel{} to a cell",
            if scale == 1 { "" } else { "s" }
        ));
    }
    s.push_str(&format!("\nDrawn, bottom first: {}", m.drawn.join("; ")));
    if !m.skipped.is_empty() {
        s.push_str(&format!("\nNot drawn: {}", m.skipped.join("; ")));
    }
    if let Some(c) = &m.compare {
        s.push_str(&format!("\nNote: {c}"));
    }
    if let Some(c) = &m.framed {
        s.push_str(&format!("\nNote: {c}"));
    }
    Ok(s)
}

/// The format asked for (`png`, `tif`), else the file's by its extension (`.png`, `.tif`, `.tiff`).
pub fn format_for(path: &Path, asked: Option<&str>) -> Result<Format, String> {
    match asked.map(str::to_lowercase).as_deref() {
        Some("png") => return Ok(Format::Png),
        Some("tif" | "tiff" | "geotiff") => return Ok(Format::GeoTiff),
        Some(other) => return Err(format!("format is png or tif, not {other:?}")),
        None => {}
    }
    match path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
        Some("png") => Ok(Format::Png),
        Some("tif" | "tiff") => Ok(Format::GeoTiff),
        _ => Err(format!("{} is not a .png or a .tif file (or give the format)", path.display())),
    }
}

/// Pixels to a cell: 1, 2 or 4.
pub fn check_scale(scale: u64) -> Result<usize, String> {
    match scale {
        1 | 2 | 4 => Ok(scale as usize),
        other => Err(format!("the scale is 1, 2 or 4 pixels to a cell, not {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::Palette;
    use crate::palette::Ramp;
    use std::collections::HashMap;
    use std::sync::Arc;
    use terraflux_graph::{Node, PlotCompare, PlotLayer};
    use terraflux_ops::Map;

    fn grid(w: usize, h: usize) -> Arc<terraflux_raster::Grid> {
        let mut g = terraflux_raster::Grid::new(w, h);
        g.transform = [100.0, 10.0, 0.0, 500.0, 0.0, -10.0];
        Arc::new(g)
    }

    fn map(w: usize, h: usize, f: impl Fn(usize) -> f64) -> Value {
        Value::Map(Map::new(grid(w, h), (0..w * h).map(f).collect()))
    }

    fn polygon() -> Value {
        use terraflux_raster::vector::{Geometry, Vector};
        let ring = vec![[120.0, 480.0], [140.0, 480.0], [140.0, 460.0], [120.0, 460.0], [120.0, 480.0]];
        Value::Vector(Arc::new(Vector {
            geometries: vec![Geometry::Polygon(vec![ring])],
            attributes: Vec::new(),
            crs: None,
        }))
    }

    /// A model of two nodes and a Plot of every kind of layer; the values its outputs would have.
    fn setup() -> (Model, HashMap<String, Value>) {
        let mut m = Model::new("plots");
        m.nodes.push(Node::new("land", "nlm.random"));
        m.nodes.push(Node::new("forest", "nlm.random"));
        let layer = |from: &str| PlotLayer::new(from);
        m.plots.push(Plot {
            id: "all".into(),
            layers: vec![
                // continuous, in the grey ramp, half opaque
                PlotLayer { opacity: Some(0.5), palette: Some("grey".into()), ..layer("land.map") },
                PlotLayer { hidden: true, ..layer("land.hidden") },
                PlotLayer { minus: Some("land.map".into()), ..layer("forest.diff") },
                PlotLayer { style: Some(serde_json::json!({ "fill": 1.0 })), ..layer("$zones") },
                layer("forest.map"),
                layer("land.missing"),
                // over the map shown
                PlotLayer { opacity: Some(0.25), ..layer("land.over") },
            ],
            ..Default::default()
        });
        let mut v = HashMap::new();
        v.insert("land.map".to_string(), map(4, 4, |i| i as f64 * 0.1));
        v.insert("forest.map".to_string(), map(4, 4, |i| (i % 3) as f64));
        v.insert("land.over".to_string(), map(4, 4, |i| (i % 2) as f64 + 0.5));
        v.insert("$zones".to_string(), polygon());
        (m, v)
    }

    #[test]
    fn a_plot_becomes_a_panel() {
        let (m, v) = setup();
        let made = panel_of(&m, "all", &|from| v.get(from).cloned()).unwrap();
        // the bottom map is the map, the other maps over it at their opacity, the features over all
        assert_eq!(made.panel.rasters.len(), 2);
        let (forest, over) = (&made.panel.rasters[0], &made.panel.rasters[1]);
        assert!(!forest.below && !over.below);
        assert_eq!((forest.opacity, over.opacity), (1.0, 0.25));
        assert_eq!(made.panel.vectors.len(), 1);
        assert_eq!(made.panel.vectors[0].fill, 1.0);
        // the order and the names told
        assert_eq!(
            made.drawn,
            ["land.map (the map, drawn opaque)", "$zones (features)", "forest.map", "land.over at 25%"]
        );
        // hidden and what has no value skipped (a difference too, when its layer has none), each with the reason
        assert_eq!(made.skipped.len(), 3);
        assert!(made.skipped[0].contains("land.hidden") && made.skipped[0].contains("hidden"));
        assert!(made.skipped[1].contains("forest.diff") && made.skipped[1].contains("no value"));
        assert!(made.skipped[2].contains("land.missing") && made.skipped[2].contains("no value"));
        assert!(made.compare.is_none());
    }

    /// A layer's blend mode and its relief, as the Plot keeps them, are in the picture `terraflux plot` writes.
    #[test]
    fn the_blend_and_the_shade_of_a_layer_are_drawn() {
        let mut m = Model::new("relief");
        m.nodes.push(Node::new("dem", "nlm.random"));
        m.nodes.push(Node::new("tint", "nlm.random"));
        let shade = crate::shade::Shade { azimuth: 270.0, altitude: 30.0, z: 2.0, strength: 0.8 };
        m.plots.push(Plot {
            id: "relief".into(),
            layers: vec![
                PlotLayer { shade: Some(shade.to_plot()), ..PlotLayer::new("dem.map") },
                PlotLayer { blend: Some("multiply".into()), ..PlotLayer::new("tint.map") },
                PlotLayer { blend: Some("overlay".into()), shade: Some(shade.to_plot()), ..PlotLayer::new("dem.over") },
            ],
            ..Default::default()
        });
        let mut v = HashMap::new();
        v.insert("dem.map".to_string(), map(8, 8, |i| (i % 8) as f64 * 0.7 + (i / 8) as f64 * 0.3));
        v.insert("tint.map".to_string(), map(8, 8, |i| i as f64 * 0.1));
        v.insert("dem.over".to_string(), map(8, 8, |i| (i % 5) as f64 * 0.37));
        let made = panel_of(&m, "relief", &|from| v.get(from).cloned()).unwrap();
        // the map's shade over it, the multiplied layer, then the layer a mode this does not know (normal) and its shade
        let r = &made.panel.rasters;
        assert_eq!(r.len(), 4);
        assert!(matches!(r[0].source, Source::Shade(_)) && r[0].coloring == shade.coloring());
        assert_eq!(r[1].blend, crate::blend::Blend::Multiply);
        assert_eq!(r[2].blend, crate::blend::Blend::Normal);
        assert!(matches!(r[3].source, Source::Shade(_)));
        assert_eq!(
            made.drawn,
            [
                "dem.map (the map)",
                "dem.map: shaded: 270°, 30°, ×2",
                "tint.map, multiply",
                "dem.over",
                "dem.over: shaded: 270°, 30°, ×2"
            ]
        );
    }

    #[test]
    fn the_palette_of_the_plot_is_honoured() {
        let (m, v) = setup();
        let made = panel_of(&m, "all", &|from| v.get(from).cloned()).unwrap();
        // the map asks for grey; the layers that do not have the colours the Viewer gives their values
        let Value::Map(land) = &v["land.map"] else { panic!() };
        let grey = Palette::Chosen(Ramp::Grey, false).look(&land.data).coloring;
        assert_eq!(made.panel.coloring, grey);
        assert_ne!(made.panel.coloring, Palette::Auto.look(&land.data).coloring);
        let Value::Map(over) = &v["land.over"] else { panic!() };
        assert_eq!(made.panel.rasters[1].coloring, Palette::Auto.look(&over.data).coloring);
        // a layer of whole numbers: classes, named as the Viewer names them
        let Coloring::Classes(c) = &made.panel.rasters[0].coloring else {
            panic!("{:?}", made.panel.rasters[0].coloring)
        };
        assert_eq!(c.len(), 3);
        // (and the map of whole numbers as the map: its classes named)
        let mut m = m;
        m.plots[0].layers.swap(0, 4);
        let made = panel_of(&m, "all", &|from| v.get(from).cloned()).unwrap();
        assert_eq!(made.panel.names.len(), 3);
    }

    #[test]
    fn a_saved_view_draws_only_its_part() {
        let (mut m, v) = setup();
        let value = |from: &str| v.get(from).cloned();
        let whole = panel_of(&m, "all", &value).unwrap();
        assert!(whole.framed.is_none());
        // columns 1 and 2, rows 2 and 3 of the 4 x 4 map (x 110 to 130, y 460 to 480)
        m.plots[0].view = terraflux_graph::PlotView::new(110.0, 460.0, 130.0, 480.0);
        let cut = panel_of(&m, "all", &value).unwrap();
        assert!(cut.framed.as_deref().is_some_and(|t| t.contains("saved view") && t.contains("2 x 2")));
        assert_eq!(cut.panel.source.size(), (2, 2));
        assert_eq!(cut.panel.source.grid().transform, [110.0, 10.0, 0.0, 480.0, 0.0, -10.0]);
        // the layers drawn and the vectors are the same
        assert_eq!(cut.drawn, whole.drawn);
        assert_eq!(cut.panel.vectors.len(), 1);
        // the picture is that part of the whole one, the polygon (x 120 to 140) in its column
        let (a, b) = (
            crate::export_view::picture(&whole.panel, 1).unwrap(),
            crate::export_view::picture(&cut.panel, 1).unwrap(),
        );
        assert_eq!((a.width(), b.width(), b.height()), (4, 2, 2));
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(b.pixel(x, y), a.pixel(x + 1, y + 2), "({x}, {y})");
            }
        }
        assert_ne!(b.pixel(0, 0), b.pixel(1, 0), "the polygon covers only the second column");
        // written: the picture is the part's size, and the file says so
        let dir = std::env::temp_dir().join(format!("terraflux-plot-view-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("cut.png");
        let said = write(&cut, 2, Format::Png, &png).unwrap();
        assert!(said.contains("4 x 4 px") && said.contains("saved view"), "{said}");
        let _ = std::fs::remove_dir_all(&dir);
        // a view away from the map: the whole map, and it says so
        m.plots[0].view = terraflux_graph::PlotView::new(0.0, 0.0, 5.0, 5.0);
        let away = panel_of(&m, "all", &value).unwrap();
        assert_eq!(away.panel.source.size(), (4, 4));
        assert!(away.framed.unwrap().contains("outside"));
    }

    #[test]
    fn a_curtain_or_a_difference_leaves_the_map_alone() {
        let (mut m, v) = setup();
        for how in ["curtain", "difference"] {
            m.plots[0].compare =
                Some(PlotCompare { with: "land.over".into(), how: Some(how.into()), ..Default::default() });
            let made = panel_of(&m, "all", &|from| v.get(from).cloned()).unwrap();
            let note = made.compare.unwrap();
            assert!(note.contains("land.over") && note.contains(how) && note.contains("only the map"), "{note}");
            assert!(made.beside.is_empty());
        }
    }

    /// A Plot that compares side by side is written with its views side by side in the PNG, the map alone in the
    /// GeoTIFF; a view with layers of its own draws them; one that is no map is told.
    #[test]
    fn a_comparison_side_by_side_is_written_beside_the_map() {
        let (mut m, v) = setup();
        m.plots[0].compare = Some(PlotCompare {
            with: "forest.map".into(),
            also: vec!["land.over".into(), "land.nothing".into()],
            ..Default::default()
        });
        let value = |from: &str| v.get(from).cloned();
        let made = panel_of(&m, "all", &value).unwrap();
        assert_eq!(made.beside.len(), 2, "{:?}", made.skipped);
        assert!(made.skipped.iter().any(|t| t.contains("land.nothing") && t.contains("beside")), "{:?}", made.skipped);
        // the views beside the map are those maps, in the map's colours, with the same layers over them
        let Value::Map(forest) = &v["forest.map"] else { panic!() };
        let Source::Map(shown) = &made.beside[0].source else { panic!() };
        assert_eq!(shown.data, forest.data);
        assert_eq!(made.beside[0].coloring, made.panel.coloring);
        assert_eq!(made.beside[0].rasters.len(), made.panel.rasters.len());
        let note = made.compare.clone().unwrap();
        assert!(
            note.contains("side by side") && note.contains("3 views") && note.contains("GeoTIFF only the map"),
            "{note}"
        );
        let dir = std::env::temp_dir().join(format!("terraflux-plot-beside-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (png, tif) = (dir.join("views.png"), dir.join("views.tif"));
        let said = write(&made, 1, Format::Png, &png).unwrap();
        assert!(said.contains("3 views side by side"), "{said}");
        let back = tiny_skia::Pixmap::load_png(&png).unwrap();
        let g = crate::export_view::GUTTER as u32;
        assert_eq!((back.width(), back.height()), (4 + g + 4 + g + 4, 4));
        // each view's own picture is where it should be
        let alone = crate::export_view::picture(&made.beside[0], 1).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(back.pixel(4 + g + x, y), alone.pixel(x, y), "({x}, {y})");
            }
        }
        let said = write(&made, 1, Format::GeoTiff, &tif).unwrap();
        assert!(said.contains("one grid"), "{said}");
        let r = terraflux_raster::tiff::read(&tif).unwrap();
        assert_eq!((r.grid.width, r.grid.height), (4, 4));
        // a view of its own layers draws them: here only the forest map, no layers over it
        m.plots[0].compare = Some(PlotCompare {
            with: "forest.map".into(),
            layers: vec![vec![PlotLayer::new("forest.map")]],
            ..Default::default()
        });
        let made = panel_of(&m, "all", &value).unwrap();
        assert_eq!((made.beside.len(), made.beside[0].rasters.len(), made.beside[0].vectors.len()), (1, 0, 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A difference layer (`minus`): the maps of values subtracted (the other minus this one, in a ramp round 0), the
    /// maps of classes as their transitions; drawn over the map in their own colours.
    #[test]
    fn a_difference_layer_is_made_and_drawn() {
        let mut m = Model::new("diff");
        for id in ["a", "b", "x", "y"] {
            m.nodes.push(Node::new(id, "nlm.random"));
        }
        m.plots.push(Plot {
            id: "change".into(),
            layers: vec![
                PlotLayer::new("a.map"),
                PlotLayer { minus: Some("a.map".into()), opacity: Some(0.5), ..PlotLayer::new("b.map") },
            ],
            ..Default::default()
        });
        m.plots.push(Plot {
            id: "classes".into(),
            layers: vec![PlotLayer { minus: Some("x.map".into()), ..PlotLayer::new("y.map") }],
            ..Default::default()
        });
        let mut v = HashMap::new();
        v.insert("a.map".to_string(), map(4, 4, |i| i as f64 * 0.5));
        v.insert("b.map".to_string(), map(4, 4, |i| i as f64 * 1.5 + 0.25));
        // classes 1 and 2: cell 0 stays 1, cell 1 goes 1 to 2, the rest stay 2
        v.insert("x.map".to_string(), map(4, 4, |i| if i < 2 { 1.0 } else { 2.0 }));
        v.insert("y.map".to_string(), map(4, 4, |i| if i == 0 { 1.0 } else { 2.0 }));
        let value = |from: &str| v.get(from).cloned();
        let made = panel_of(&m, "change", &value).unwrap();
        assert!(made.skipped.is_empty(), "{:?}", made.skipped);
        assert_eq!(made.drawn, ["a.map (the map)", "b.map minus a.map at 50%"]);
        // the layer is the difference: b - a = 2 a + 0.25, in a ramp centred on 0
        let layer = &made.panel.rasters[0];
        let Source::Map(d) = &layer.source else { panic!() };
        assert_eq!(d.data.iter().copied().take(4).collect::<Vec<_>>(), [0.25, 1.25, 2.25, 3.25]);
        assert_eq!(layer.opacity, 0.5);
        let Coloring::Continuous { min, max, .. } = layer.coloring else { panic!("{:?}", layer.coloring) };
        assert!(min < 0.0 && max > 0.0 && (min + max).abs() < 1e-9, "{min} {max}");
        assert!(layer.name.contains("minus"), "{}", layer.name);
        // classes: the transitions, named, each a class with its colour; the cells that stayed apart
        let made = panel_of(&m, "classes", &value).unwrap();
        assert_eq!(made.drawn[0], "x.map to y.map (the map)");
        let Coloring::Classes(c) = &made.panel.coloring else { panic!("{:?}", made.panel.coloring) };
        assert!(c.len() >= 2, "{c:?}");
        assert!(made.panel.names.iter().any(|(_, n)| n.contains("->") || n.contains("to")), "{:?}", made.panel.names);
        // written: the picture draws the transitions' colours
        let pm = crate::export_view::picture(&made.panel, 1).unwrap();
        let (c0, c1) = (pm.pixel(0, 0).unwrap(), pm.pixel(1, 0).unwrap());
        assert_ne!((c0.red(), c0.green(), c0.blue()), (c1.red(), c1.green(), c1.blue()), "stayed, and changed");
        // a map that is not there: left out with the reason, the other layers drawn
        m.plots[0].layers.push(PlotLayer { minus: Some("gone.map".into()), ..PlotLayer::new("b.map") });
        let made = panel_of(&m, "change", &value).unwrap();
        assert!(made.skipped.iter().any(|t| t.contains("difference with gone.map")), "{:?}", made.skipped);
        assert_eq!(made.panel.rasters.len(), 1);
    }

    /// A node inside a container, as the Viewer names it (the container's id, then `inner.port`) or by its place
    /// (`group/inner.port`): found, and read from the run where the container keeps it.
    #[test]
    fn a_node_inside_a_container_is_found() {
        use terraflux_graph::Body;
        let mut m = Model::new("nested");
        m.nodes.push(
            Node::new("g", terraflux_graph::GROUP)
                .body(Body { nodes: vec![Node::new("inner", "nlm.random")], ..Default::default() }),
        );
        m.nodes.push(Node::new("top", "nlm.random"));
        let out = Place::Output("g".into(), "inner.map".into());
        assert_eq!(place_of(&m, "g.inner.map"), Some(out.clone()));
        assert_eq!(place_of(&m, "g/inner.map"), Some(out));
        assert_eq!(place_of(&m, "top.map"), Some(Place::Output("top".into(), "map".into())));
        assert_eq!(place_of(&m, "$cut"), Some(Place::Input("cut".into())));
        for bad in ["nope", "g/gone.map", "top/x.map", "g..map", "$", "a.b-.", "g/inner"] {
            let found = place_of(&m, bad);
            // ("g/gone.map": a place the model has no container for is still the container's key, the run has none)
            assert!(found.is_none() || bad == "g/gone.map", "{bad}: {found:?}");
        }
        // the plot draws it
        m.plots.push(Plot {
            id: "inside".into(),
            layers: vec![PlotLayer::new("top.map"), PlotLayer::new("g/inner.map")],
            ..Default::default()
        });
        let mut v = HashMap::new();
        v.insert("top.map".to_string(), map(4, 4, |i| i as f64 * 0.1));
        v.insert("g.inner.map".to_string(), map(4, 4, |i| (i % 2) as f64));
        let made = panel_of(&m, "inside", &|from| v.get(from).cloned()).unwrap();
        assert!(made.skipped.is_empty(), "{:?}", made.skipped);
        assert_eq!(made.drawn, ["top.map (the map)", "g.inner.map"]);
        assert_eq!(made.panel.rasters.len(), 1);
        // the values a run keeps: a container's, `inner.port`
        let mut results =
            terraflux_exec::Results { values: Default::default(), warnings: Vec::new(), timings: Vec::new() };
        results.values.insert("g".into(), [("inner.map".to_string(), v["g.inner.map"].clone())].into());
        results.values.insert("top".into(), [("map".to_string(), v["top.map"].clone())].into());
        let inputs = [("cut".to_string(), Value::Number(0.5))].into();
        assert!(matches!(lookup(&inputs, &results, "g.inner.map"), Some(Value::Map(_))));
        assert!(matches!(lookup(&inputs, &results, "top.map"), Some(Value::Map(_))));
        assert!(matches!(lookup(&inputs, &results, "$cut"), Some(Value::Number(_))));
        assert!(lookup(&inputs, &results, "g.gone").is_none() && lookup(&inputs, &results, "$none").is_none());
    }

    #[test]
    fn what_cannot_be_drawn_is_told() {
        let (m, v) = setup();
        let e = panel_of(&m, "nope", &|from| v.get(from).cloned()).err().unwrap();
        assert!(e.contains("no plot \"nope\"") && e.contains("all"), "{e}");
        // features alone: no map
        let mut m = m;
        m.plots.push(Plot { id: "zones".into(), layers: vec![PlotLayer::new("$zones")], ..Default::default() });
        let e = panel_of(&m, "zones", &|from| v.get(from).cloned()).err().unwrap();
        assert!(e.contains("no map"), "{e}");
        assert_eq!(format_for(Path::new("a.PNG"), None), Ok(Format::Png));
        assert_eq!(format_for(Path::new("a.tiff"), None), Ok(Format::GeoTiff));
        assert_eq!(format_for(Path::new("a"), Some("tif")), Ok(Format::GeoTiff));
        assert!(format_for(Path::new("a.jpg"), None).is_err());
        assert!(check_scale(3).is_err() && check_scale(4) == Ok(4));
    }

    #[test]
    fn a_plot_is_written() {
        let (m, v) = setup();
        let made = panel_of(&m, "all", &|from| v.get(from).cloned()).unwrap();
        let dir = std::env::temp_dir().join(format!("terraflux-plot-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("all.png");
        let said = write(&made, 2, Format::Png, &png).unwrap();
        assert!(said.contains("8 x 8 px") && said.contains("2 pixels to a cell"), "{said}");
        assert!(said.contains("Drawn, bottom first:") && said.contains("Not drawn:"), "{said}");
        let back = tiny_skia::Pixmap::load_png(&png).unwrap();
        assert_eq!((back.width(), back.height()), (8, 8));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
