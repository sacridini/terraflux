//! A saved Plot drawn without the application: its layers read from the outputs of a run, coloured as the Viewer
//! would colour them from the Plot alone (the layer's palette, else the one the output's producer asks for, the
//! classes' colours of the model's legend), and written as a PNG or an RGBA GeoTIFF as *Export the view* does. For
//! `terraflux plot` and the MCP tool `export_plot`.

use std::path::Path;

use terraflux_graph::{Model, Plot, Ref};
use terraflux_ops::Value;

use crate::builder::{default_palette, layer_source, look_of, output_name, palette_named};
use crate::export_view::{Format, MAX_SIDE, save, size_of};
use crate::figure::Panel;
use crate::map_view::{RasterLayer, Source};
use crate::palette::Coloring;

/// A Plot as a panel, and what of it was drawn.
pub struct Made {
    pub panel: Panel,
    /// The layers drawn, bottom first, as they are named in the Viewer (with their opacity when not 1).
    pub drawn: Vec<String>,
    /// The layers not drawn and why.
    pub skipped: Vec<String>,
    /// Said when the Plot compares its map with another (only the map is drawn).
    pub compare: Option<String>,
    /// Said when the Plot has a saved view (only that part of the map is drawn).
    pub framed: Option<String>,
}

/// What a layer is, once its value is read.
enum Kind {
    Map(Source),
    Features(std::sync::Arc<terraflux_raster::vector::Vector>),
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

fn made(m: &Model, plot: &Plot, value: &dyn Fn(&str) -> Option<Value>) -> Result<Made, String> {
    let mut skipped = Vec::new();
    // the layers that are drawn, bottom first: (the layer, where it is from, what it is)
    let mut kept: Vec<(&terraflux_graph::PlotLayer, String, String, Kind)> = Vec::new();
    for l in &plot.layers {
        if l.hidden {
            skipped.push(format!("{}: hidden", l.from));
            continue;
        }
        if let Some(minus) = &l.minus {
            skipped.push(format!("{}: a difference with {minus}, drawn in the application only", l.from));
            continue;
        }
        let (node, port) = match Ref::parse(&l.from) {
            Ok(Ref::Port { node, port }) => (node, port),
            Ok(Ref::Input(name)) => (String::new(), name),
            _ => {
                skipped.push(format!("{}: not an output or an input of the model", l.from));
                continue;
            }
        };
        let Some(v) = value(&l.from) else {
            skipped.push(format!("{}: no value in the run", l.from));
            continue;
        };
        match &v {
            Value::Vector(f) => kept.push((l, node, port, Kind::Features(f.clone()))),
            other => match layer_source(other, l.band) {
                Some(src) => kept.push((l, node, port, Kind::Map(src))),
                None => skipped.push(format!("{}: not a map or features", l.from)),
            },
        }
    }
    // the map is the bottom one (opaque, its grid the picture's); the other maps go over it, each at its opacity
    // (the Viewer's map shown takes no opacity of its own, so a Plot's top map would hide the rest)
    let first = kept
        .iter()
        .position(|(.., k)| matches!(k, Kind::Map(_)))
        .ok_or_else(|| format!("the plot {:?} has no map to draw (only features or nothing)", plot.id))?;
    let colour = |l: &terraflux_graph::PlotLayer, node: &str, port: &str, src: &Source| {
        let palette = l
            .palette
            .as_deref()
            .and_then(palette_named)
            .unwrap_or_else(|| default_palette(&m.nodes, node, port, Some(src)));
        let look = look_of(src, palette);
        let legend = crate::legend_view::legend_in(&m.nodes, node, port, src);
        let look = match &legend {
            Some(c) => crate::legend_view::apply(&look, c),
            None => look,
        };
        let names = match &look.coloring {
            Coloring::Classes(c) => {
                c.iter().map(|(v, _)| (*v, crate::legend_view::name(&look, legend.as_deref(), *v))).collect()
            }
            Coloring::Continuous { .. } => Vec::new(),
        };
        (look.coloring, names)
    };
    let (l, node, port, Kind::Map(source)) = &kept[first] else { unreachable!("the first map") };
    let (coloring, names) = colour(l, node, port, source);
    let source = source.clone();
    let mut drawn = Vec::new();
    let mut rasters = Vec::new();
    let mut vectors = Vec::new();
    let at = |name: String, l: &terraflux_graph::PlotLayer| match l.opacity {
        Some(o) if (o - 1.0).abs() > 1e-9 => format!("{name} at {:.0}%", o * 100.0),
        _ => name,
    };
    for (i, (l, node, port, k)) in kept.iter().enumerate() {
        let name = if node.is_empty() { format!("${port}") } else { output_name(node, port) };
        match k {
            Kind::Map(_) if i == first => drawn.push(match l.opacity {
                Some(o) if o < 1.0 => format!("{name} (the map, drawn opaque)"),
                _ => format!("{name} (the map)"),
            }),
            Kind::Map(src) => {
                let (coloring, _) = colour(l, node, port, src);
                let opacity = l.opacity.unwrap_or(1.0).clamp(0.0, 1.0) as f32;
                drawn.push(at(name.clone(), l));
                rasters.push(RasterLayer { source: src.clone(), coloring, opacity, below: false, name });
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
    let compare = plot.compare.as_ref().map(|c| {
        format!(
            "the plot compares its map with {} ({}): only the map is drawn",
            c.with,
            c.how.as_deref().filter(|h| !h.trim().is_empty()).unwrap_or("side by side")
        )
    });
    let mut panel = Panel { source, coloring, names, caption: String::new(), vectors, rasters };
    // the saved view: only that part of the map (the vectors and the other maps follow its grid)
    let framed = match &plot.view {
        Some(v) if crate::frame::cells_of(panel.source.grid(), v.extent).is_some() => {
            panel = crate::frame::crop_panel(&panel, v.extent)?;
            let (w, h) = panel.source.size();
            Some(format!("the plot's saved view: only that part of the map is drawn ({w} x {h} cells)"))
        }
        Some(_) => Some("the plot's saved view is outside the map: the whole map is drawn".to_string()),
        None => None,
    };
    Ok(Made { panel, drawn, skipped, compare, framed })
}

/// The Plot written to `path` (a picture of the map, `scale` pixels to a cell); what to tell of it: the file, its
/// size and cells to a pixel, the layers drawn and not drawn.
pub fn write(m: &Made, scale: usize, format: Format, path: &Path) -> Result<String, String> {
    let scale = scale.max(1);
    let said = save(&m.panel, scale, format, path)?;
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
        // hidden, difference and what has no value skipped, each with the reason
        assert_eq!(made.skipped.len(), 3);
        assert!(made.skipped[0].contains("land.hidden") && made.skipped[0].contains("hidden"));
        assert!(made.skipped[1].contains("forest.diff") && made.skipped[1].contains("difference"));
        assert!(made.skipped[2].contains("land.missing") && made.skipped[2].contains("no value"));
        assert!(made.compare.is_none());
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
    fn a_comparison_leaves_the_map_alone() {
        let (mut m, v) = setup();
        m.plots[0].compare =
            Some(PlotCompare { with: "land.over".into(), how: Some("curtain".into()), ..Default::default() });
        let made = panel_of(&m, "all", &|from| v.get(from).cloned()).unwrap();
        let note = made.compare.unwrap();
        assert!(note.contains("land.over") && note.contains("curtain") && note.contains("only the map"), "{note}");
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
