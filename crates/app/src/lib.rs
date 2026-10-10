//! Terraflux's application: models created with a wizard, their graph, their run with the maps drawn as they
//! come, the area of each class along the steps and the validation against an observed map.

pub mod app;
pub mod graph_view;
pub mod map_view;
pub mod palette;
pub mod runner;
pub mod wizard;

use std::path::PathBuf;

pub use app::TerrafluxApp;

/// Open the application's window (with a model, if given).
pub fn run(model: Option<PathBuf>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 820.0]).with_title("Terraflux"),
        ..Default::default()
    };
    eframe::run_native("Terraflux", options, Box::new(move |_cc| Ok(Box::new(TerrafluxApp::new(model)))))
}
