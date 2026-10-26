//! The README's pictures of the application, drawn off screen as the UI tests draw it and written to
//! `docs/images`: `cargo test --release -p terraflux-app --test readme_images -- --ignored`. Run again when the
//! application's look changes (the satellite picture needs the network).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_dock::{DockState, NodeIndex};
use egui_kittest::Harness;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;

const SIZE: (f32, f32) = (1440.0, 900.0);

fn images() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/images")
}

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
}

fn harness() -> Harness<'static, TerrafluxApp> {
    harness_at(SIZE, 1.0)
}

/// A window of `size` points drawn at `ppp` pixels a point (larger, for a picture cut to one part of it).
fn harness_at(size: (f32, f32), ppp: f32) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    unsafe { std::env::set_var("TERRAFLUX_SETTINGS", dir.join("readme-settings.json")) };
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    terraflux_app::canvas_gpu::install(&rs);
    let mut h = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), TerrafluxApp::new(None));
    h.run_steps(2);
    h
}

/// Step until `done`, two minutes at most.
fn until(h: &mut Harness<TerrafluxApp>, what: &str, done: impl Fn(&TerrafluxApp) -> bool) {
    let t = Instant::now();
    while !done(h.state()) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "{what}: too long");
    }
}

/// Nothing running, the maps shown coloured.
fn settled(h: &mut Harness<TerrafluxApp>) {
    h.run_steps(3);
    until(h, "the run and the maps", |a| {
        a.session.as_ref().is_none_or(|s| !s.running() && !s.loading() && !s.view.busy() && !s.preview_view.busy())
    });
    h.run_steps(3);
}

fn save(h: &mut Harness<TerrafluxApp>, name: &str) {
    settled(h);
    // the maps of the plots' tabs, coloured in threads of their own
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(2) {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    let img = h.render().expect("the window drawn by wgpu");
    std::fs::create_dir_all(images()).unwrap();
    img.save(images().join(name)).unwrap();
}

/// The panes for a picture, as the application first has them: the model over `below` (the first in front), the
/// node in the inspector at the right (the operators in their drawer).
fn layout(below: Vec<Pane>) -> DockState<Pane> {
    let mut d = DockState::new(vec![Pane::Model]);
    let s = d.main_surface_mut();
    let [model, _] = s.split_right(NodeIndex::root(), 0.78, vec![Pane::Node, Pane::Prepare, Pane::Files]);
    s.split_below(model, 0.4, below);
    d
}

/// A template run, its graph over its plots (the plot `front` in front), the node `select` selected.
fn template(h: &mut Harness<TerrafluxApp>, name: &str, select: &str, front: &str) {
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == name).expect("the template");
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, t);
    settled(h);
    let a = h.state_mut();
    let s = a.session.as_mut().unwrap();
    terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), select);
    let mut below: Vec<Pane> = s.map_tabs.keys().map(|&id| Pane::Map(id)).collect();
    below.sort_by_key(|p| !matches!(p, Pane::Map(id) if s.map_tabs[id].node == front));
    below.push(Pane::Viewer);
    a.dock = Some(layout(below));
    h.run_steps(3);
    let a = h.state_mut();
    a.session.as_mut().unwrap().editor.fit();
}

#[test]
#[ignore = "the README's pictures: run with --ignored"]
fn readme_images() {
    // the builder: a forest's fragmentation, its graph over its maps
    let mut h = harness();
    template(&mut h, "Fragmentation of a forest", "fos", "plot_fos");
    save(&mut h, "builder.png");

    // conservation planning: Zonation's priority rank
    let mut h = harness();
    template(&mut h, "Zonation", "zonation", "plot_rank");
    save(&mut h, "zonation.png");

    // land change: a city that grows in a loop
    let mut h = harness();
    template(&mut h, "A city that grows (a loop)", "growth", "plot_city");
    save(&mut h, "land-change.png");

    // the satellite under a land use map
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let mut h = harness();
    {
        let r = terraflux_ops::Registry::standard();
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        // the file has no CRS: Sibuyan is in UTM 51N
        terraflux_app::builder::add(
            s,
            &r,
            "raster.reproject",
            &[("crs", "EPSG:32651".into()), ("source_crs", "EPSG:32651".into())],
        )
        .unwrap();
    }
    until(&mut h, "the map with a CRS", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.preview_value().is_some()
    });
    {
        let a = h.state_mut();
        a.dock = Some(layout(vec![Pane::Viewer]));
        let s = a.session.as_mut().unwrap();
        s.preview_view.basemap = terraflux_app::map_view::Basemap::EsriImagery;
        s.preview_view.transparency = 0.45;
        s.editor.fit();
    }
    h.run_steps(3);
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().preview_view.base_busy() && t.elapsed() < Duration::from_secs(60) {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    if h.state().session.as_ref().unwrap().preview_view.base_tiles().0 > 0 {
        save(&mut h, "basemap.png");
    }
}

/// The assistant explaining a node, answered by the Humboldt-Universität's model (from its network or VPN):
/// `cargo test --release -p terraflux-app --test readme_images assistant -- --ignored`.
/// The model read as a document: a species distribution model, its steps with their notes and settings in words,
/// the maps of the run beside them, a step selected.
#[test]
#[ignore = "the README's pictures: run with --ignored"]
fn document_image() {
    let mut h = harness();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "A species in a warmer climate").unwrap();
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, t);
    settled(&mut h);
    let a = h.state_mut();
    let s = a.session.as_mut().unwrap();
    terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "suitability");
    let mut d = DockState::new(vec![Pane::Document, Pane::Model]);
    let _ = d.main_surface_mut().split_right(NodeIndex::root(), 0.74, vec![Pane::Node, Pane::Prepare, Pane::Files]);
    a.dock = Some(d);
    save(&mut h, "document.png");
}

#[test]
#[ignore = "the README's pictures: run with --ignored (the HU Berlin's network)"]
fn assistant_image() {
    let mut h = harness();
    terraflux_app::settings::update(|s| s.provider = Some(0));
    template(&mut h, "Fragmentation of a forest", "fos", "plot_fos");
    {
        let a = h.state_mut();
        let mut d = DockState::new(vec![Pane::Model]);
        let s = d.main_surface_mut();
        let [model, _] = s.split_right(NodeIndex::root(), 0.6, vec![Pane::Assistant, Pane::Node]);
        let below: Vec<Pane> = a.session.as_ref().unwrap().map_tabs.keys().map(|&id| Pane::Map(id)).collect();
        s.split_below(model, 0.45, below);
        a.dock = Some(d);
        let ctx = h.ctx.clone();
        let a = h.state_mut();
        let s = a.session.as_ref().unwrap();
        a.chat.send(
            "What does this model do? And explain the node fos: what it reads and what it gives.".into(),
            s,
            &ctx,
        );
    }
    let t = Instant::now();
    while h.state().chat.busy() && t.elapsed() < Duration::from_secs(100) {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    for i in &h.state().chat.items {
        println!("{i:?}");
    }
    save(&mut h, "assistant.png");
}

/// The review of what the assistant changed (the Humboldt-Universität's model, from its network or VPN): the
/// change before and after, the node marked on the canvas.
#[test]
#[ignore = "the README's pictures: run with --ignored (the HU Berlin's network)"]
fn review_image() {
    let mut h = harness();
    terraflux_app::settings::update(|s| s.provider = Some(0));
    template(&mut h, "Fragmentation of a forest", "fos", "plot_fos");
    {
        let ctx = h.ctx.clone();
        let a = h.state_mut();
        let s = a.session.as_ref().unwrap();
        a.chat.send(
            "Make the window of the node fos 13 cells instead of 27, and record why in the node.".into(),
            s,
            &ctx,
        );
    }
    let t = Instant::now();
    while h.state().chat.busy() && t.elapsed() < Duration::from_secs(100) {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    for i in &h.state().chat.items {
        println!("{i:?}");
    }
    settled(&mut h);
    {
        let a = h.state_mut();
        let mut d = DockState::new(vec![Pane::Model]);
        let s = d.main_surface_mut();
        let [model, _] = s.split_right(NodeIndex::root(), 0.6, vec![Pane::Review, Pane::Assistant, Pane::Node]);
        let below: Vec<Pane> = a.session.as_ref().unwrap().map_tabs.keys().map(|&id| Pane::Map(id)).collect();
        s.split_below(model, 0.45, below);
        a.dock = Some(d);
    }
    save(&mut h, "review.png");
}

/// The website's pictures of the raster calculator (the tutorial building-models/calculator), written to
/// `website/static/img/tutorials/building-models/calculator`: the Calculator pane with its script, previews and
/// console, a stack's layers completed, the dialog that saves the calculator as a node of your own, and that node
/// in the model. `cargo test --release -p terraflux-app --test readme_images calculator -- --ignored`.
#[test]
#[ignore = "the website's pictures: run with --ignored"]
fn calculator_images() {
    use egui_kittest::kittest::Queryable;
    let site =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../website/static/img/tutorials/building-models/calculator");
    std::fs::create_dir_all(&site).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("calculator-images");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(data("sibuyan_landuse.tif"), dir.join("landuse.tif")).unwrap();
    std::fs::copy(data("sibuyan_drivers.tif"), dir.join("drivers.tif")).unwrap();
    unsafe { std::env::set_var("TERRAFLUX_NODES", dir.join("nodes")) };
    let script = "# lu: the land use of 1997 and 2011 (a stack); d: the drivers (a stack)
forest = lu == 1                        # a stack: the forest of each year
out years = sum(forest)                 # years as forest: 0, 1 or 2
out lost = forest._1997 and not forest._2011

# a score from 0 to 1 for elevation and for roads, and the index
si_elev = clamp((1200 - d.elevation) / 900, 0, 1)
si_road = clamp(d.dist_roads / 3000, 0, 1)
out hsi = sqrt(si_elev * si_road)
hsi[not forest._1997] = None            # the forest of 1997 only

out best = hsi >= hsi.percentile(75)    # its best quarter
out mean_hsi = hsi.mean()";
    let mut m = terraflux_graph::Model::new("Sibuyan's forest");
    m.inputs.insert(
        "lu".into(),
        terraflux_graph::Input::Stack { path: "landuse.tif".into(), bands: None, nodata: None, note: None },
    );
    m.inputs.insert(
        "d".into(),
        terraflux_graph::Input::Stack { path: "drivers.tif".into(), bands: None, nodata: None, note: None },
    );
    m.nodes.push(
        terraflux_graph::Node::new("calc", "raster.calc")
            .input("lu", "$lu")
            .input("d", "$d")
            .param("expression", script)
            .note("The forest's years, its loss, and a habitat index of the forest of 1997."),
    );
    let p = dir.join("sibuyan.toml");
    m.save(&p).unwrap();
    // (a smaller window drawn larger: each picture is cut to the part it shows, and its words stay readable)
    const PPP: f32 = 1.5;
    let mut h = harness_at((1300.0, 760.0), PPP);
    h.state_mut().open(&p);
    h.state_mut().session.as_mut().unwrap().wait();
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    settled(&mut h);
    {
        let a = h.state_mut();
        let s = a.session.as_mut().unwrap();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "calc");
        let mut d = DockState::new(vec![Pane::Model]);
        let surface = d.main_surface_mut();
        let [model, _] = surface.split_right(NodeIndex::root(), 0.78, vec![Pane::Node, Pane::Operators]);
        surface.split_below(model, 0.3, vec![Pane::Calculator]);
        a.dock = Some(d);
        a.session.as_mut().unwrap().editor.fit();
    }
    h.run_steps(5);
    // the previews, then a line of the console
    let t = Instant::now();
    while h.query_all_by_label_contains(" · max ").next().is_none() && t.elapsed() < Duration::from_secs(60) {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("calculator console input")));
    h.run_steps(2);
    h.event(egui::Event::Text("hsi.percentile(75)".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    let t = Instant::now();
    while h.query_by_label_contains("= 0.").is_none() && t.elapsed() < Duration::from_secs(60) {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    // the window, or the part of it at x, y of width w and height h (in points)
    let shoot = |h: &mut Harness<TerrafluxApp>, name: &str, part: Option<[f32; 4]>| {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(2) {
            h.step();
            std::thread::sleep(Duration::from_millis(20));
        }
        let img = h.render().expect("the window drawn by wgpu");
        let img = match part {
            Some(r) => {
                let [x, y, w, ht] = r.map(|v| (v * PPP).round() as u32);
                image::imageops::crop_imm(&img, x, y, w, ht).to_image()
            }
            None => img,
        };
        img.save(site.join(name)).unwrap();
    };
    shoot(&mut h, "pane.png", Some([0.0, 0.0, 1012.0, 760.0]));
    // a stack's layers offered after the dot
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("calculator script")));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::End);
    h.event(egui::Event::Text("\nout remote = d.dist_".into()));
    h.run_steps(3);
    shoot(&mut h, "completion.png", Some([30.0, 268.0, 720.0, 355.0]));
    // (the line typed taken back: leaving the editor would give it to the node)
    for _ in "\nout remote = d.dist_".chars() {
        h.key_press(egui::Key::Backspace);
    }
    h.run_steps(3);
    // saved as a node of your own: the dialog, then the node
    {
        let r = terraflux_ops::Registry::standard();
        let s = h.state_mut().session.as_mut().unwrap();
        s.editor.chosen.clear();
        h.ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("calculator script")));
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::my_nodes::begin(s, &r, "calc");
        let d = s.editor.saving.as_mut().expect("the dialog");
        d.name = "forest_hsi".into();
    }
    h.run_steps(3);
    shoot(&mut h, "save-dialog.png", Some([14.0, 14.0, 565.0, 425.0]));
    h.get_by_label("Save").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.saving.is_none(), "saved");
    {
        let r = terraflux_ops::Registry::standard();
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &r, "calc");
        let mut d = DockState::new(vec![Pane::Model]);
        let _ = d.main_surface_mut().split_right(NodeIndex::root(), 0.7, vec![Pane::Node, Pane::Operators]);
        h.state_mut().dock = Some(d);
        h.state_mut().session.as_mut().unwrap().editor.fit();
    }
    h.run_steps(3);
    shoot(&mut h, "my-node.png", Some([228.0, 45.0, 1062.0, 625.0]));
}
