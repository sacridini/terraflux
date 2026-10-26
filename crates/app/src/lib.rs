//! Terraflux's application: models built node by node (or created with a wizard), run as they are built with
//! what each node gives shown at once, the maps drawn as they come, the area of each class along the steps and
//! the validation against an observed map.

pub mod app;
pub mod basemap;
pub mod batch_view;
pub mod builder;
pub mod calc_view;
pub mod canvas_gpu;
pub mod charts;
pub mod chat;
pub mod commands;
pub mod computers;
pub mod diagnose;
pub mod document;
pub mod document_view;
pub mod editor;
pub mod editor_view;
pub mod files_view;
pub mod handoff;
pub mod legend_view;
pub mod llm;
pub mod logo;
pub mod map_view;
pub mod metadata;
pub mod my_nodes;
pub mod node_view;
pub mod palette;
pub mod params_view;
pub mod prepare;
pub mod prepare_view;
pub mod problems_view;
pub mod report;
pub mod review;
pub mod runner;
pub mod settings;
pub mod table_view;
pub mod templates;
pub mod theme;
pub mod tools;
pub mod trace;
pub mod trace_view;
pub mod undo;
pub mod vector_tiles;
pub mod vector_view;
pub mod watch;
pub mod wizard;
pub mod woe_view;

use std::path::PathBuf;

pub use app::TerrafluxApp;

/// How frames reach the screen: a frame queued at most (low latency, the window follows the pointer when it is
/// moved), in step with the screen. To try what is smoothest on a machine:
///
/// - `TERRAFLUX_PRESENT`: `vsync` (default) or `novsync` (frames as soon as they are drawn);
/// - `TERRAFLUX_LATENCY`: the frames queued, 1 (default) to 3;
/// - `WGPU_BACKEND`: `dx12`, `vulkan`, `metal` or `gl` (the graphics interface; wgpu's choice by default).
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
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_title("Terraflux")
            .with_icon(logo::window_icon()),
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
            // the models handed by `terraflux mcp` opened here, not in a window of their own
            let ctx = cc.egui_ctx.clone();
            app.handoff = handoff::listen(move || ctx.request_repaint());
            // maximized once shown (made maximized, a window shown after its first frame is not, on Windows)
            app.maximize = Some(0);
            // the panes where they were left
            if let Some(path) = app::layout_file() {
                app.restore_layout(path);
            }
            Ok(Box::new(app))
        }),
    )
}
