//! Terraflux's application: models built node by node (or created with a wizard), run as they are built with
//! what each node gives shown at once, the maps drawn as they come, the area of each class along the steps and
//! the validation against an observed map.

pub mod app;
pub mod basemap;
pub mod builder;
pub mod canvas_gpu;
pub mod charts;
pub mod editor;
pub mod editor_view;
pub mod map_view;
pub mod palette;
pub mod prepare;
pub mod prepare_view;
pub mod runner;
pub mod table_view;
pub mod templates;
pub mod theme;
pub mod vector_tiles;
pub mod vector_view;
pub mod wizard;

use std::path::PathBuf;

pub use app::TerrafluxApp;

/// How frames reach the screen: a frame queued at most (low latency, the window follows the pointer when it is
/// moved), in step with the screen. To try what is smoothest on a machine:
///
/// - `TERRAFLUX_PRESENT`: `vsync` (default) or `novsync` (frames as soon as they are drawn);
/// - `TERRAFLUX_LATENCY`: the frames queued, 1 (default) to 3;
/// - `WGPU_BACKEND`: `dx12`, `vulkan` or `gl` (the graphics interface; wgpu's choice by default).
pub fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{SurfaceConfig, WgpuConfiguration, wgpu::PresentMode};
    let present_mode = match std::env::var("TERRAFLUX_PRESENT").unwrap_or_default().to_lowercase().as_str() {
        "novsync" | "off" | "0" => PresentMode::AutoNoVsync,
        _ => PresentMode::AutoVsync,
    };
    let latency = std::env::var("TERRAFLUX_LATENCY").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(1).clamp(1, 3);
    WgpuConfiguration {
        surface: SurfaceConfig { present_mode, desired_maximum_frame_latency: Some(latency) },
        ..Default::default()
    }
}

/// Open the application's window (with a model, if given).
pub fn run(model: Option<PathBuf>) -> eframe::Result<()> {
    terraflux_ops::threads::ensure();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 820.0]).with_title("Terraflux"),
        wgpu_options: wgpu_options(),
        ..Default::default()
    };
    eframe::run_native(
        "Terraflux",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            // the builder's canvas is drawn by the GPU
            if let Some(rs) = &cc.wgpu_render_state {
                canvas_gpu::install(rs);
            }
            let mut app = TerrafluxApp::new(model);
            // the panes where they were left
            if let Some(path) = app::layout_file() {
                app.restore_layout(path);
            }
            Ok(Box::new(app))
        }),
    )
}
