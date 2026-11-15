//! The charts follow the colour vision chosen (View > Colour vision). A program of its own: the vision is the whole
//! program's, and the other tests compare colours as they are. A picture of the same chart in normal vision and as a
//! green-blind reader sees it is saved in `target/tmp` (chart-vision-*.png).

use std::path::Path;

use egui_kittest::Harness;
use terraflux_app::charts::{Chart, Kind};
use terraflux_app::palette::{Vision, class_colour, seen, set_vision};
use terraflux_app::table_view::{TableState, show};
use terraflux_ops::Table;

/// Four groups of points over two metrics.
fn table() -> Table {
    let rows = (0..40)
        .map(|i| {
            let g = f64::from(i % 4);
            vec![g, f64::from(i) * 0.5 + g, 20.0 + g * 7.0 - f64::from(i % 5)]
        })
        .collect();
    Table { columns: vec!["group".into(), "x".into(), "y".into()], rows, ..Default::default() }
}

/// The table's points chart drawn, its pixels (none when there is no renderer) and its picture saved at `path`.
fn drawn(path: &Path) -> Option<Vec<u8>> {
    let t = table();
    let id = egui::Id::new("vision chart");
    let chart = Chart { kind: Kind::Points, columns: vec![2], x: Some(1), by: Some(0), ..Default::default() };
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let mut h = Harness::builder()
        .with_size((900.0, 600.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui(move |ui| {
            terraflux_app::theme::apply(ui.ctx());
            show(ui, &t, id);
        });
    h.ctx.data_mut(|d| d.insert_temp(id, TableState { chart, ..Default::default() }));
    h.run_steps(4);
    match h.render() {
        Ok(img) => {
            img.save(path).unwrap();
            Some(img.as_raw().clone())
        }
        Err(e) => {
            eprintln!("no renderer ({e}): chart not drawn");
            None
        }
    }
}

/// The same chart drawn as a green-blind reader sees it is another picture, and the groups' colours are the ones
/// that vision sees.
#[test]
fn a_chart_is_drawn_in_the_vision_chosen() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp");
    std::fs::create_dir_all(&dir).unwrap();
    set_vision(Vision::Normal);
    let normal = drawn(&dir.join("chart-vision-normal.png"));
    set_vision(Vision::Deuteranopia);
    let blind = drawn(&dir.join("chart-vision-deuteranopia.png"));
    let colours: Vec<_> = (0..4).map(|i| (class_colour(i), seen(class_colour(i)))).collect();
    set_vision(Vision::Normal);
    // (the first group's blue shifts less than the others)
    assert!(colours.iter().filter(|(a, b)| a != b).count() >= 3, "{colours:?}");
    if let (Some(a), Some(b)) = (normal, blind) {
        let changed = a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count();
        assert!(changed > 200, "only {changed} pixels differ");
    }
}
