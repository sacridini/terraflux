//! Pictures of the node editor's canvas, to judge how its boxes look (not the README's: they go to
//! `$CARGO_TARGET_TMPDIR/canvas_shots`, never to `docs/images`):
//! `cargo test -j 4 -p terraflux-app --test canvas_shots -- --ignored --nocapture`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;

const SIZE: (f32, f32) = (1440.0, 900.0);

/// One window at a time: two wgpu devices opened together may crash the program.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Where the pictures are written.
fn out() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas_shots")
}

/// A window of `size` points drawn at `ppp` pixels a point, in the theme `theme`.
fn harness(size: (f32, f32), ppp: f32, theme: &str) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    // (a settings file of its own, written once: never the user's)
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::fs::write(dir.join("canvas-shots-settings.json"), r#"{ "theme": "Slate" }"#).unwrap();
        unsafe { std::env::set_var("TERRAFLUX_SETTINGS", dir.join("canvas-shots-settings.json")) };
    });
    terraflux_app::settings::update(|s| s.theme = theme.into());
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

/// Nothing running, the maps shown coloured.
fn settled(h: &mut Harness<TerrafluxApp>) {
    h.run_steps(3);
    let t = Instant::now();
    while !h
        .state()
        .session
        .as_ref()
        .is_none_or(|s| !s.running() && !s.loading() && !s.view.busy() && !s.preview_view.busy())
    {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "the run: too long");
    }
    h.run_steps(3);
}

/// A template opened over a canvas alone, its banner hidden, the graph fitted to the canvas.
fn template(h: &mut Harness<TerrafluxApp>, name: &str) {
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == name).expect("the template");
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, t);
    settled(h);
    let a = h.state_mut();
    a.dock = Some(egui_dock::DockState::new(vec![Pane::Model]));
    let s = a.session.as_mut().unwrap();
    s.my_data.hidden = true;
    s.editor.fit();
    h.run_steps(6);
}

fn save(h: &mut Harness<TerrafluxApp>, name: &str) {
    h.run_steps(4);
    let img = h.render().expect("the window drawn by wgpu");
    std::fs::create_dir_all(out()).unwrap();
    img.save(out().join(name)).unwrap();
}

/// The camera at `zoom`, the header of the node `node` in the middle of the canvas (the canvas's middle taken
/// from the camera that fits the graph).
fn look_at(h: &mut Harness<TerrafluxApp>, node: &str, zoom: f32) {
    let s = h.state_mut().session.as_mut().unwrap();
    let ids: Vec<String> = s.source.nodes.iter().map(|n| n.id.clone()).collect();
    let all = s.editor.rects_of(&ids);
    let bounds = all.iter().fold(egui::Rect::NOTHING, |b, (_, r)| b.union(*r));
    let cam = s.editor.camera().expect("the canvas drawn");
    let middle = cam.pan + bounds.center().to_vec2() * cam.zoom;
    let at = s.editor.rects_of(&[node.to_string()]).first().expect("the node").1.min + egui::vec2(120.0, 40.0);
    s.editor.set_camera(middle - at.to_vec2() * zoom, zoom);
    h.run_steps(6);
}

#[test]
#[ignore = "pictures of the canvas for a look: run with --ignored"]
fn canvas_shots() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let models = [
        ("fragmentation", "Fragmentation of a forest"),
        ("zonation", "Zonation"),
        ("city", "A city that grows (a loop)"),
        ("mspa", "MSPA of a forest"),
    ];
    for (theme, tag) in [("Slate", "dark"), ("Paper", "light")] {
        for (short, name) in models {
            let mut h = harness(SIZE, 1.0, theme);
            template(&mut h, name);
            save(&mut h, &format!("{short}_{tag}.png"));
            if short == "zonation" && tag == "dark" {
                // far: the names alone, their icons beside them
                look_at(&mut h, "zonation", 0.5);
                save(&mut h, "zonation_far_dark.png");
            }
            if short == "zonation" {
                // the legend open
                let s = h.state_mut().session.as_mut().unwrap();
                s.editor.legend = true;
                s.editor.fit();
                h.run_steps(6);
                save(&mut h, &format!("zonation_legend_{tag}.png"));
            }
        }
    }
    // out of date: "update after every edit" off, the first node's seed changed after the run; it and what reads it
    // faded, with a hollow amber light
    for (theme, tag) in [("Slate", "dark"), ("Paper", "light")] {
        let mut h = harness(SIZE, 1.0, theme);
        template(&mut h, "Fragmentation of a forest");
        let s = h.state_mut().session.as_mut().unwrap();
        s.auto = false;
        terraflux_app::editor::set_param(&mut s.source, &[], "landscape", "seed", "8").unwrap();
        s.edited();
        assert!(!s.stale.is_empty());
        save(&mut h, &format!("stale_{tag}.png"));
    }
    // close: a node's header at 1.5 and twice the pixels, to judge the small details
    for (theme, tag) in [("Slate", "dark"), ("Paper", "light")] {
        for node in ["fos", "landscape", "fad"] {
            let mut h = harness((720.0, 450.0), 2.0, theme);
            template(&mut h, "Fragmentation of a forest");
            look_at(&mut h, node, 1.5);
            save(&mut h, &format!("{node}_close_{tag}.png"));
        }
    }
    // the legend of a scope with features, objects and tables (every shape in it), at twice the pixels
    for (theme, tag) in [("Slate", "dark"), ("Paper", "light")] {
        let mut h = harness((720.0, 450.0), 2.0, theme);
        template(&mut h, "Segment an image into polygons");
        h.state_mut().session.as_mut().unwrap().editor.legend = true;
        h.run_steps(6);
        save(&mut h, &format!("objects_legend_{tag}.png"));
    }
    // close: ports of every shape (objects, vector features, tables, numbers) at three zooms
    for (theme, tag) in [("Slate", "dark"), ("Paper", "light")] {
        for (template_name, node) in [
            ("Segment an image into polygons", "features"),
            ("Segment an image into polygons", "polygons"),
            ("Species distribution model", "data"),
            ("Species distribution model", "fit"),
        ] {
            for zoom in [0.7, 1.0, 1.5] {
                let mut h = harness((720.0, 450.0), 2.0, theme);
                template(&mut h, template_name);
                look_at(&mut h, node, zoom);
                save(&mut h, &format!("shapes_{node}_{zoom}_{tag}.png"));
            }
        }
    }
    println!("the pictures are in {}", out().display());
}

/// Zoomed far out: the nodes as cards (under 0.6), at 0.5 and 0.3, in both themes.
#[test]
#[ignore = "pictures of the canvas for a look: run with --ignored"]
fn far_shots() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for (theme, tag) in [("Slate", "dark"), ("Paper", "light")] {
        // (the third has groups)
        for (short, name, node) in [
            ("zonation", "Zonation", "zonation"),
            ("city", "A city that grows (a loop)", "city"),
            ("switch", "Choose the method: a switch and its scenarios", ""),
        ] {
            let mut h = harness(SIZE, 1.0, theme);
            template(&mut h, name);
            let first = {
                let s = h.state().session.as_ref().unwrap();
                let pick = s.source.nodes.iter().find(|n| n.id == node).unwrap_or(&s.source.nodes[0]);
                pick.id.clone()
            };
            for zoom in [0.5, 0.3] {
                look_at(&mut h, &first, zoom);
                save(&mut h, &format!("far_{short}_{zoom}_{tag}.png"));
            }
        }
    }
    println!("the pictures are in {}", out().display());
}

/// The pointer resting on a card (0.5): the tooltip with the whole title, the operator and the id.
#[test]
#[ignore = "pictures of the tooltips for a look: run with --ignored"]
fn far_hover_shots() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for (theme, tag) in [("Slate", "dark"), ("Paper", "light")] {
        let mut h = harness(SIZE, 1.0, theme);
        template(&mut h, "Zonation");
        look_at(&mut h, "zonation", 0.5);
        let at = h.state().session.as_ref().unwrap().editor.screen_card_of("zonation").expect("a card").center();
        h.hover_at(at);
        h.run_steps(8);
        save(&mut h, &format!("far_hover_{tag}.png"));
    }
}

/// The tooltips of what flows: the pointer rests on an output port, then on a wire, of a model that was run.
#[test]
#[ignore = "pictures of the tooltips for a look: run with --ignored"]
fn hover_shots() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for (theme, tag) in [("Slate", "dark"), ("Paper", "light")] {
        let mut h = harness(SIZE, 1.0, theme);
        template(&mut h, "Fragmentation of a forest");
        // a connection into the node "fos": its port, and where the value comes from
        let (port, from, out) = {
            let s = h.state().session.as_ref().unwrap();
            let n = s.source.nodes.iter().find(|n| n.id == "fos").expect("the node");
            n.inputs
                .iter()
                .find_map(|(port, src)| match src {
                    terraflux_graph::Source::Ref(r) => match terraflux_graph::Ref::parse(r) {
                        Ok(terraflux_graph::Ref::Port { node, port: out }) => Some((port.clone(), node, out)),
                        _ => None,
                    },
                    _ => None,
                })
                .expect("a connection")
        };
        let at = h.state().session.as_ref().unwrap().editor.screen_of_output(&from, &out).expect("the output");
        h.hover_at(at);
        h.run_steps(8);
        save(&mut h, &format!("hover_port_{tag}.png"));
        // the wire, half way along
        let path = h.state().session.as_ref().unwrap().editor.wire("fos", &port).expect("the wire");
        let mid = path[path.len() / 2];
        let at = h.state().session.as_ref().unwrap().editor.screen_at(mid).expect("on the canvas");
        h.hover_at(at);
        h.run_steps(8);
        save(&mut h, &format!("hover_wire_{tag}.png"));
    }
}

#[test]
#[ignore = "the icons of the families at several sizes, for a look: run with --ignored"]
fn family_icons() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let mut h = Harness::builder()
        .with_size((720.0, 260.0))
        .with_pixels_per_point(1.0)
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui(|ui| {
            terraflux_app::theme::apply(ui.ctx());
            for size in [13.0, 14.0, 18.0, 28.0] {
                ui.horizontal(|ui| {
                    for f in terraflux_app::family::ALL {
                        let text = egui::RichText::new(f.icon())
                            .font(terraflux_app::family::icon_font(ui.ctx(), size))
                            .color(f.colour(true));
                        ui.label(text);
                    }
                });
            }
        });
    h.run_steps(4);
    let img = h.render().expect("the window drawn by wgpu");
    std::fs::create_dir_all(out()).unwrap();
    img.save(out().join("family_icons.png")).unwrap();
}
