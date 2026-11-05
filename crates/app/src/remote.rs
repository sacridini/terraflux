//! Data on the web as layers of the Viewer: a COG, or any raster GDAL opens over the network (`/vsicurl/`,
//! `/vsis3/`...), and vector features (GeoJSON, FlatGeobuf...), by their address. The address is checked by opening
//! it (size, bands, coordinate system, or why not) in a thread; then a `raster.read` (or `vector.read`) of it is a
//! node of the model, kept as a layer as a file dropped on the Viewer is.
//!
//! A raster on the web is read by GDAL as it is asked for, by ranges of bytes: the window the map shows, at the
//! overview that fits the screen, never the whole file (see `terraflux_ops::files::File::open`). The tiles are made
//! in the tile threads, so a slow server shows as tiles that come late, and a failing one as the layer's error.

use std::sync::mpsc::{Receiver, channel};

use egui::RichText;
use terraflux_ops::Registry;

use crate::app::Session;

/// What an address holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Raster,
    Vector,
}

/// What opening an address told.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub kind: Kind,
    /// The path GDAL reads it by (`/vsicurl/https://...`).
    pub path: String,
    /// A raster: its size in cells, bands, cell type, overviews and the driver that reads it.
    pub size: (usize, usize),
    pub bands: usize,
    pub dtype: String,
    pub overviews: usize,
    pub driver: String,
    pub crs: Option<String>,
    /// Vector features: the names of the layers of the file.
    pub layers: Vec<String>,
}

impl Info {
    /// What it is, in a line.
    pub fn describe(&self) -> String {
        match self.kind {
            Kind::Raster => {
                let crs = self.crs.as_deref().map_or("no coordinate system".to_string(), crs_name);
                let ov = match self.overviews {
                    0 => "no overviews".to_string(),
                    1 => "1 overview".to_string(),
                    n => format!("{n} overviews"),
                };
                format!(
                    "{} x {} cells, {} band{} of {}, {crs}, {ov} ({})",
                    self.size.0,
                    self.size.1,
                    self.bands,
                    if self.bands == 1 { "" } else { "s" },
                    self.dtype,
                    self.driver
                )
            }
            Kind::Vector => {
                let n = self.layers.len();
                format!("vector features, {n} layer{}: {}", if n == 1 { "" } else { "s" }, self.layers.join(", "))
            }
        }
    }
}

/// The name of a coordinate system in its WKT (`PROJCS["WGS 84 / UTM zone 51N", ...`: `WGS 84 / UTM zone 51N`).
pub fn crs_name(wkt: &str) -> String {
    let name = wkt.split('"').nth(1).filter(|n| !n.is_empty());
    name.map_or_else(|| wkt.chars().take(40).collect(), String::from)
}

/// Whether an address names vector features, by the extension of its file (before any `?` or `#`).
pub fn kind_of(address: &str) -> Kind {
    let end = address.find(['?', '#']).unwrap_or(address.len());
    let file = address[..end].rsplit('/').next().unwrap_or("");
    let ext = file.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    if ["geojson", "json", "fgb", "gpkg", "kml", "gml", "parquet"].contains(&ext.as_str()) {
        Kind::Vector
    } else {
        Kind::Raster
    }
}

/// Open an address as GDAL does when a layer reads it: its path and what it holds, or why it cannot be read.
/// Slow with a slow server (up to GDAL's timeouts): run it in a thread.
pub fn probe(address: &str) -> Result<Info, String> {
    let address = address.trim();
    let path = terraflux_graph::remote_path(address).ok_or_else(|| {
        format!("{address}: not an address on the web (https://..., http://..., s3://..., gs://... or /vsicurl/...)")
    })?;
    match kind_of(&path) {
        Kind::Vector => {
            let layers = terraflux_gdal::vector_layers(&path).map_err(|e| format!("{path}: {e}"))?;
            if layers.is_empty() {
                return Err(format!("{path}: no layer of features"));
            }
            Ok(Info {
                kind: Kind::Vector,
                path,
                size: (0, 0),
                bands: 0,
                dtype: String::new(),
                overviews: 0,
                driver: String::new(),
                crs: None,
                layers,
            })
        }
        Kind::Raster => {
            let d = terraflux_gdal::Dataset::open(&path)?;
            let g = &d.meta.grid;
            Ok(Info {
                kind: Kind::Raster,
                size: (g.width, g.height),
                bands: d.meta.bands.len(),
                dtype: format!("{:?}", d.meta.dtype).to_lowercase(),
                overviews: d.overviews,
                driver: d.driver.clone(),
                crs: g.geo.wkt.clone(),
                layers: Vec::new(),
                path,
            })
        }
    }
}

/// The window that adds a layer from an address: what is typed, the address being checked, what was found.
#[derive(Default)]
pub struct Dialog {
    pub url: String,
    job: Option<(String, Receiver<Result<Info, String>>)>,
    /// What checking an address told (the address as typed then).
    pub result: Option<(String, Result<Info, String>)>,
}

impl Dialog {
    /// The dialog with an address already typed.
    pub fn with(url: &str) -> Dialog {
        Dialog { url: url.to_string(), ..Dialog::default() }
    }

    /// Check the address typed, in a thread.
    pub fn check(&mut self, ctx: &egui::Context) {
        let url = self.url.trim().to_string();
        if url.is_empty() || self.job.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let (asked, ctx) = (url.clone(), ctx.clone());
        let _ = std::thread::Builder::new().name("terraflux-url-check".into()).spawn(move || {
            let _ = tx.send(probe(&asked));
            ctx.request_repaint();
        });
        self.result = None;
        self.job = Some((url, rx));
    }

    /// Whether an address is being checked.
    pub fn checking(&self) -> bool {
        self.job.is_some()
    }

    /// What was found for the address typed now, when it was checked.
    pub fn found(&self) -> Option<&Result<Info, String>> {
        self.result.as_ref().filter(|(u, _)| *u == self.url.trim()).map(|(_, r)| r)
    }

    /// A check that came is kept.
    pub fn poll(&mut self) {
        if let Some((url, rx)) = &self.job
            && let Ok(r) = rx.try_recv()
        {
            self.result = Some((url.clone(), r));
            self.job = None;
        }
    }
}

/// The window of *Add a layer from a URL*: the address, its check, the button that adds it.
pub fn window(ctx: &egui::Context, s: &mut Session, r: &Registry) {
    let Some(mut d) = s.url_layer.take() else { return };
    d.poll();
    let mut open = true;
    let mut add = false;
    egui::Window::new("Add a layer from a URL").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.label("A COG or any raster GDAL reads over the web, or vector features (GeoJSON, FlatGeobuf...).");
        ui.label(RichText::new("https://..., s3://..., gs://... or /vsicurl/...").weak().size(11.5));
        ui.add_space(4.0);
        let resp = ui.add(
            egui::TextEdit::singleline(&mut d.url)
                .hint_text("https://example.org/map.tif")
                .desired_width(420.0)
                .id(egui::Id::new("url layer address")),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            let typed = !d.url.trim().is_empty();
            if (ui
                .add_enabled(typed && !d.checking(), egui::Button::new("Check"))
                .on_hover_text("open it as the layer will: its size, bands and coordinate system, or why not")
                .clicked()
                || enter)
                && typed
            {
                d.check(ui.ctx());
            }
            let ok = matches!(d.found(), Some(Ok(_)));
            if ui
                .add_enabled(ok, crate::app::accent_button("Add as a layer", ok))
                .on_hover_text("a node of the model reads it, its layer on top: read by windows as the map asks")
                .clicked()
            {
                add = true;
            }
        });
        if d.checking() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("opening it...").weak());
            });
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        match d.found() {
            Some(Ok(info)) => {
                ui.label(info.describe());
            }
            Some(Err(e)) => {
                ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
            }
            None => {}
        }
    });
    if add && let Some(Ok(info)) = d.found().cloned() {
        match crate::builder::add_url(s, r, &info.path) {
            Ok(_) => return,
            Err(e) => d.result = Some((d.url.trim().to_string(), Err(e))),
        }
    }
    if open {
        s.url_layer = Some(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_an_address_holds_by_its_file() {
        for (a, k) in [
            ("https://host.org/maps/a.tif", Kind::Raster),
            ("/vsicurl/https://host.org/a.TIF?token=1", Kind::Raster),
            ("s3://bucket/a", Kind::Raster),
            ("https://host.org/a.geojson", Kind::Vector),
            ("https://host.org/a.GeoJSON?x=a.tif", Kind::Vector),
            ("/vsicurl/https://host.org/roads.fgb#frag", Kind::Vector),
            ("gs://bucket/dir.v2/a.parquet", Kind::Vector),
        ] {
            assert_eq!(kind_of(a), k, "{a}");
        }
    }

    #[test]
    fn the_name_of_a_crs() {
        assert_eq!(crs_name(r#"PROJCS["WGS 84 / UTM zone 51N",GEOGCS["WGS 84"]]"#), "WGS 84 / UTM zone 51N");
        assert_eq!(crs_name(r#"GEOGCRS["WGS 84",DATUM[]]"#), "WGS 84");
        assert_eq!(crs_name("EPSG:4326"), "EPSG:4326");
    }

    #[test]
    fn what_is_not_an_address_is_refused_at_once() {
        for a in ["", "data/a.tif", "C:\\data\\a.tif", "ftp:/host/a.tif"] {
            let e = probe(a).unwrap_err();
            assert!(e.contains("not an address"), "{a}: {e}");
        }
    }

    #[test]
    fn a_dialog_checks_what_was_typed_and_forgets_it_when_it_changes() {
        let mut d = Dialog::with("  nothing here ");
        let ctx = egui::Context::default();
        d.check(&ctx);
        assert!(d.checking());
        let t = std::time::Instant::now();
        while d.checking() && t.elapsed() < std::time::Duration::from_secs(10) {
            d.poll();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(d.found(), Some(Err(e)) if e.contains("not an address")));
        d.url = "other".into();
        assert!(d.found().is_none(), "another address: not checked");
    }
}
