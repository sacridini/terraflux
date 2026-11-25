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
    harness_marked(size, ppp, Marks::default())
}

/// Numbers drawn over the window (a picture's parts, explained below it): where, and the number.
type Marks = std::sync::Arc<std::sync::Mutex<Vec<(egui::Pos2, String)>>>;

/// A window with `marks` drawn over it, each number in an orange disc.
fn harness_marked(size: (f32, f32), ppp: f32, marks: Marks) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    // (the pictures show the default theme, Slate: the settings file starts from the defaults, written once)
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::fs::write(dir.join("readme-settings.json"), r#"{ "theme": "Slate" }"#).unwrap();
        unsafe { std::env::set_var("TERRAFLUX_SETTINGS", dir.join("readme-settings.json")) };
        // (a model not saved writes under Documents/Terraflux: a folder of the tests', never the user's)
        unsafe { std::env::set_var("TERRAFLUX_DOCUMENTS", std::env::temp_dir().join("terraflux-test-documents")) };
    });
    terraflux_app::settings::update(|s| s.theme = "Slate".into());
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
        .build_ui_state(
            move |ui, app: &mut TerrafluxApp| {
                app.frame(ui);
                let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("marks")));
                for (at, n) in marks.lock().unwrap().iter() {
                    painter.circle(
                        *at,
                        11.0,
                        egui::Color32::from_rgb(240, 160, 20),
                        egui::Stroke::new(1.5, egui::Color32::BLACK),
                    );
                    painter.text(
                        *at,
                        egui::Align2::CENTER_CENTER,
                        n,
                        egui::FontId::proportional(13.0),
                        egui::Color32::BLACK,
                    );
                }
            },
            TerrafluxApp::new(None),
        );
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

/// The panes for a picture, as the application has them now: the model over `below` (the first in front), no
/// Node pane (a node's settings are on its box and in the panel beside it), the operators and the preparation in
/// their drawers at the window's left edge.
fn layout(below: Vec<Pane>) -> DockState<Pane> {
    layout_at(below, 0.52)
}

/// [`layout`] with the model taking `share` of the height.
fn layout_at(below: Vec<Pane>, share: f32) -> DockState<Pane> {
    let mut d = DockState::new(vec![Pane::Model]);
    d.main_surface_mut().split_below(NodeIndex::root(), share, below);
    d
}

/// The share of the height the model takes in the pictures of templates: room for whole boxes and their thumbnails.
const MODEL_SHARE: f32 = 0.6;

/// A template run, its graph over the Viewer showing the plot `front`, the node `select` selected.
fn template(h: &mut Harness<TerrafluxApp>, name: &str, select: &str, front: &str) {
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == name).expect("the template");
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, t);
    settled(h);
    let a = h.state_mut();
    let s = a.session.as_mut().unwrap();
    // (the Viewer shows the Plot asked for, the node asked for selected)
    terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), front);
    s.selected = Some(select.to_string());
    // (the strip that offers Use my data hidden: it would cover the top of the graph)
    s.my_data.hidden = true;
    a.dock = Some(layout_at(vec![Pane::Viewer], MODEL_SHARE));
    h.run_steps(3);
    let a = h.state_mut();
    a.session.as_mut().unwrap().editor.fit();
}

/// The canvas brought to `zoom` at least, about the middle of the Model pane: a fit that leaves the boxes as
/// compact chips (their names cut) shows them whole, their ports and settings, as a picture of the builder should.
fn whole_boxes(h: &mut Harness<TerrafluxApp>, zoom: f32) {
    h.run_steps(3);
    let e = &mut h.state_mut().session.as_mut().unwrap().editor;
    let (Some(cam), Some(origin)) = (e.camera(), e.screen_at(egui::Pos2::ZERO)) else { return };
    if cam.zoom >= zoom {
        return;
    }
    // (the canvas's corner on the screen; the middle of the Model pane, above the Viewer, under the bars)
    let corner = origin - cam.pan;
    let middle = egui::pos2(SIZE.0 / 2.0, (corner.y + SIZE.1 * MODEL_SHARE) / 2.0);
    let at = (middle - origin) / cam.zoom;
    e.set_camera(middle - corner - at * zoom, zoom);
    h.run_steps(3);
}

#[test]
#[ignore = "the README's pictures: run with --ignored"]
fn readme_images() {
    // the builder: a forest's fragmentation, its graph over its maps
    let mut h = harness();
    template(&mut h, "Fragmentation of a forest", "fos", "plot_fos");
    whole_boxes(&mut h, 0.65);
    save(&mut h, "builder.png");

    // conservation planning: Zonation's priority rank
    let mut h = harness();
    template(&mut h, "Zonation", "zonation", "plot_rank");
    save(&mut h, "zonation.png");

    // land change: a city that grows in a loop
    let mut h = harness();
    template(&mut h, "A city that grows (a loop)", "growth", "plot_city");
    whole_boxes(&mut h, 0.6);
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
    // (the Node pane opened beside it by hand: it does not open by itself; the Prepare pane is a drawer)
    let mut d = DockState::new(vec![Pane::Document, Pane::Model]);
    let _ = d.main_surface_mut().split_right(NodeIndex::root(), 0.74, vec![Pane::Node]);
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
        s.split_below(model, 0.45, vec![Pane::Viewer]);
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
        s.split_below(model, 0.45, vec![Pane::Viewer]);
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
        terraflux_graph::Input::Stack { path: "landuse.tif".into(), bands: None, nodata: None, note: None, data: None },
    );
    m.inputs.insert(
        "d".into(),
        terraflux_graph::Input::Stack { path: "drivers.tif".into(), bands: None, nodata: None, note: None, data: None },
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
    let mut h = harness_at((1300.0, 860.0), PPP);
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
    // the console is a bar until it is wanted: opened, then the previews, then a line of it
    h.get_by_label_contains("console").click();
    h.run_steps(3);
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
        // (the pointer, left over the console's bar, not in the picture)
        h.event(egui::Event::PointerGone);
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
    shoot(&mut h, "pane.png", Some([0.0, 0.0, 1012.0, 860.0]));
    // a stack's layers offered after the dot
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("calculator script")));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::End);
    h.event(egui::Event::Text("\nout remote = d.dist_".into()));
    h.run_steps(3);
    shoot(&mut h, "completion.png", Some([30.0, 278.0, 720.0, 400.0]));
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
    shoot(&mut h, "my-node.png", Some([228.0, 45.0, 1062.0, 700.0]));
}

/// The tour's pictures (website/static/img/tutorials/getting-started/tour): the window as it opens, the Templates
/// menu with its group Patterns open, the template "MSPA of a forest" run with the window's parts numbered as the
/// tour explains them, the Model pane alone, and a node's settings panel beside its box (`node-pane.png`).
/// `cargo test --release -p terraflux-app --test readme_images tour -- --ignored`.
#[test]
#[ignore = "the website's pictures: run with --ignored"]
fn tour_images() {
    let out = std::env::var("TOUR_OUT").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../website/static/img/tutorials/getting-started/tour")
    });
    std::fs::create_dir_all(&out).unwrap();
    use egui_kittest::kittest::{NodeT, Queryable};
    let marks = Marks::default();
    // (at this width the bar's tools are in its Tools menu)
    let mut h = harness_marked((1280.0, 760.0), 1.0, marks.clone());
    // where a part is: the first widget whose words have these (its rectangle, points); the panes' tabs are not
    // widgets, so they are found from the widgets of their panes
    let rect = |h: &Harness<TerrafluxApp>, words: &str| -> egui::Rect {
        let n = h
            .query_all_by_label_contains("")
            .find(|n| {
                let a = n.accesskit_node();
                a.label().is_some_and(|l| l.contains(words)) || a.value().is_some_and(|v| v.contains(words))
            })
            .unwrap_or_else(|| panic!("{words:?} in the window"));
        let b = n.accesskit_node().bounding_box().expect("a place");
        egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))
    };
    // the window as it opens
    settled(&mut h);
    h.render().expect("the window drawn by wgpu").save(out.join("empty.png")).unwrap();
    // the Templates menu, its group Patterns open (the window's top left corner, as far as the menus reach)
    h.get_by_label("Templates").click();
    h.run_steps(3);
    // (the group's whole name: the template "Patterns and objects" has the word too)
    h.get_by_label_contains("Patterns (GuidosToolbox)").click();
    h.run_steps(3);
    // (the menu's right edge: its rows are a button as wide as its words and, beside one with a tutorial, a link
    // "tutorial" on the same line, so the widest of them, links included)
    let (yours, item) = (rect(&h, "Yours"), rect(&h, "MSPA of a forest"));
    let mut right = item.right();
    for t in terraflux_app::templates::TEMPLATES.iter().filter(|t| t.group == "Patterns (GuidosToolbox)") {
        right = right.max(rect(&h, t.name).right());
    }
    for n in h.query_all_by_label("tutorial") {
        if let Some(b) = n.accesskit_node().bounding_box() {
            if b.x0 as f32 > item.left() && (b.y1 as f32) > item.top() && (b.y0 as f32) < yours.bottom() {
                right = right.max(b.x1 as f32);
            }
        }
    }
    let img = h.render().expect("the window drawn by wgpu");
    let (w, ht) = ((right + 20.0) as u32, (yours.bottom() + 18.0) as u32);
    image::imageops::crop_imm(&img, 0, 0, w.min(img.width()), ht.min(img.height()))
        .to_image()
        .save(out.join("templates-menu.png"))
        .unwrap();
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "MSPA of a forest").expect("the template");
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, t);
    settled(&mut h);
    // the panes as the template opens them (the Viewer and the Document behind the Model's), the Viewer dragged
    // below the Model (the tour's tip says how); the Node pane is not there: the node's settings are on its box
    let tour_dock = |h: &mut Harness<TerrafluxApp>, below: f32| {
        let a = h.state_mut();
        let mut d = DockState::new(vec![Pane::Model, Pane::Document]);
        d.main_surface_mut().split_below(NodeIndex::root(), below, vec![Pane::Viewer]);
        a.dock = Some(d);
        let s = a.session.as_mut().unwrap();
        terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), "plot");
        s.selected = Some("mspa".into());
        s.ask_viewer = false;
    };
    let wait = |h: &mut Harness<TerrafluxApp>| {
        h.run_steps(3);
        h.state_mut().session.as_mut().unwrap().editor.fit();
        settled(h);
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(2) {
            h.step();
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    tour_dock(&mut h, 0.5);
    wait(&mut h);
    // (the pointer, left from the menu, out of the pictures)
    h.event(egui::Event::PointerGone);
    h.run_steps(2);
    let (whole, save, metadata) = (rect(&h, "Whole model"), rect(&h, "Save output"), rect(&h, "Metadata"));
    // the Model pane alone: from its tab (above its bar) to the Viewer's tab, as wide as the Viewer's pane
    let img = h.render().expect("the window drawn by wgpu");
    let (x0, y0) = ((whole.left() - 14.0) as u32, (whole.top() - 36.0) as u32);
    let (x1, y1) = ((metadata.right() + 15.0) as u32, (save.top() - 70.0) as u32);
    image::imageops::crop_imm(&img, x0, y0, x1 - x0, y1 - y0).to_image().save(out.join("model-pane.png")).unwrap();
    // the window, its parts numbered as the tour explains them
    let beside = |r: egui::Rect| egui::pos2(r.right() + 16.0, r.center().y);
    let before = |r: egui::Rect| egui::pos2(r.left() - 16.0, r.center().y);
    let mspa = h.state().session.as_ref().unwrap().editor.drawn_rect_of("mspa").expect("the box of mspa");
    let places = [
        // (the toolbar: its menus, and at its right the Tools menu)
        ("1", before(rect(&h, "Tools"))),
        ("2", beside(rect(&h, "done in"))),
        // (the box of mspa, selected: its settings are its rows, its gear opens the panel beside it)
        ("3", mspa.left_top() + egui::vec2(-8.0, 8.0)),
        // (the drawers' tabs at the window's left edge)
        ("4", egui::pos2(46.0, 305.0)),
        ("5", egui::pos2(whole.left() + 90.0, whole.bottom() + 40.0)),
        ("6", beside(rect(&h, "Classes"))),
        ("7", egui::pos2(whole.left() + 81.0, save.top() - 26.0)),
        ("8", egui::pos2(save.center().x, save.top() - 12.0)),
        ("9", before(rect(&h, "Background")) - egui::vec2(18.0, 0.0)),
        ("10", rect(&h, "add an operator").left_top() + egui::vec2(10.0, -12.0)),
    ];
    *marks.lock().unwrap() = places.iter().map(|(n, at)| (*at, n.to_string())).collect();
    h.run_steps(2);
    let img = h.render().expect("the window drawn by wgpu");
    img.save(out.join("window.png")).unwrap();
    // a node's settings in the panel beside its box (its gear opens it): the Model pane alone, tall enough for
    // the panel, the picture cut to the box and the panel
    marks.lock().unwrap().clear();
    h.state_mut().dock = Some(DockState::new(vec![Pane::Model]));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.editor.chosen = ["mspa".to_string()].into();
        s.editor.panel = Some(s.editor.path.clone());
    }
    wait(&mut h);
    h.event(egui::Event::PointerGone);
    h.run_steps(2);
    let b = h.state().session.as_ref().unwrap().editor.drawn_rect_of("mspa").expect("the box of mspa");
    let img = h.render().expect("the window drawn by wgpu");
    let canvas = h.state().session.as_ref().unwrap().editor.canvas().expect("the canvas");
    // (the panel opens at the box's right, or at its left when the canvas has no room there)
    let (x0, y0) =
        ((b.left() - 380.0).max(canvas.left() - 6.0) as u32, (b.top() - 190.0).max(canvas.top() - 6.0) as u32);
    let (x1, y1) = ((b.right() + 40.0).min(canvas.right() + 6.0) as u32, (canvas.bottom() + 6.0) as u32);
    image::imageops::crop_imm(&img, x0, y0, x1 - x0, y1 - y0).to_image().save(out.join("node-pane.png")).unwrap();
}

/// The tour's picture of the Trace pane comparing two cells (`trace-compare.png`): the template CLUE-S, a cell of
/// `simulation` traced, a second one picked in the Viewer, the two marked A and B on the map and the pane telling
/// where the difference begins.
/// `cargo test --release -p terraflux-app --test readme_images tour_compare -- --ignored`.
#[test]
#[ignore = "the website's pictures: run with --ignored"]
fn tour_compare_image() {
    let out = std::env::var("TOUR_OUT").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../website/static/img/tutorials/getting-started/tour")
    });
    std::fs::create_dir_all(&out).unwrap();
    use egui_kittest::kittest::Queryable;
    let mut h = harness_at((1280.0, 760.0), 1.0);
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "CLUE-S").expect("the template");
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("simulation", "maps").is_some()
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "simulation");
        s.preview = Some(("simulation".to_string(), "maps".to_string()));
    }
    h.state_mut().focus(Pane::Viewer);
    settled(&mut h);
    h.state_mut().session.as_mut().unwrap().preview_view.picked = Some((40, 60));
    h.run_steps(3);
    h.get_by_label("Trace this cell").click();
    h.run_steps(4);
    h.get_by_label("Compare with another cell").click_accesskit();
    h.run_steps(3);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_view.picked = Some((70, 75));
        s.preview_view.clicks += 1;
    }
    h.run_steps(4);
    assert!(h.state().trace.comparison.is_some(), "compared");
    settled(&mut h);
    h.event(egui::Event::PointerGone);
    h.run_steps(2);
    h.render().expect("the window drawn by wgpu").save(out.join("trace-compare.png")).unwrap();
}
