//! The text fits at every size the settings offer, in the classic look and the modern: the panes, menus and
//! windows drawn off screen and their widgets looked for in the tree of accessibility (a process of its own: the
//! theme and the size of the text are the application's, for every test running beside it).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_app::wizard::{Picked, Wizard};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// A Dinamica model of Sibuyan written by the wizard.
fn model() -> PathBuf {
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    w.save(&tmp("fit-model.toml")).unwrap()
}

fn shot(h: &mut Harness<TerrafluxApp>, name: &str) {
    match h.render() {
        Ok(img) => img.save(tmp(&format!("ui-{name}.png"))).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
    }
}

/// Settings of its own (not the user's, nor the other tests'); no computer shared or lent.
fn setup() {
    unsafe { std::env::set_var("TERRAFLUX_SETTINGS", tmp("fit-settings.json")) };
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
}

fn new_app(model: Option<PathBuf>) -> TerrafluxApp {
    setup();
    TerrafluxApp::new(model)
}

/// The application rendered off screen by wgpu, the builder's canvas pipelines installed as the application
/// does.
fn harness(app: TerrafluxApp) -> Harness<'static, TerrafluxApp> {
    setup();
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    terraflux_app::canvas_gpu::install(&rs);
    Harness::builder()
        .with_size((1280.0, 820.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app)
}

/// Step the application until its data is read, two minutes at most.
fn loaded(h: &mut Harness<TerrafluxApp>) {
    let t = Instant::now();
    while !h.state().session.as_ref().is_some_and(|s| !s.loading()) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "reading the data: too long");
    }
}

/// A widget of the tree of accessibility: what it is, where, and the path to it from the root.
struct Placed {
    what: String,
    rect: egui::Rect,
    path: Vec<usize>,
    /// The text inside a field: its lines.
    lines: Vec<egui::Rect>,
    floating: bool,
}

fn rect_of(r: Option<egui::accesskit::Rect>) -> Option<egui::Rect> {
    r.map(|r| egui::Rect::from_min_max(egui::pos2(r.x0 as f32, r.y0 as f32), egui::pos2(r.x1 as f32, r.y1 as f32)))
}

fn placed(
    n: egui_kittest::Node<'_>,
    chain: &str,
    seen: egui::Rangef,
    path: &mut Vec<usize>,
    widgets: &mut Vec<Placed>,
    separators: &mut Vec<egui::Rect>,
    bars: &mut Vec<egui::Rect>,
) {
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT;
    let a = n.accesskit_node();
    if a.is_hidden() {
        return;
    }
    match (a.role(), rect_of(a.bounding_box())) {
        (Role::Unknown, Some(r)) if r.width() <= 8.0 && r.height() >= 60.0 => separators.push(r),
        (Role::ScrollBar, Some(r)) if r.width() > r.height() * 3.0 => bars.push(r),
        (
            Role::Button
            | Role::CheckBox
            | Role::RadioButton
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::ComboBox
            | Role::Slider
            | Role::SpinButton
            | Role::Label
            | Role::Link,
            Some(r),
        ) if r.width() > 0.0 && r.height() > 0.0 => {
            // (scrolled out of sight: not drawn; in part: what is seen of it)
            let ys = r.y_range().intersection(seen);
            if ys.span() <= 0.0 {
                // (out of sight: only what makes a scroll bar sideways)
                widgets.push(Placed {
                    what: format!("{:?} {:?}", a.role(), a.label().unwrap_or_default()),
                    rect: r,
                    path: Vec::new(),
                    lines: Vec::new(),
                    floating: true,
                });
                return;
            }
            let cut = ys != r.y_range();
            let r = egui::Rect::from_x_y_ranges(r.x_range(), ys);
            let runs: String = n.children().filter_map(|c| c.accesskit_node().value().map(|v| v.to_string())).collect();
            let text = a.label().map(|l| l.to_string()).or_else(|| a.value().map(|v| v.to_string())).unwrap_or(runs);
            let lines = if !cut && matches!(a.role(), Role::TextInput | Role::MultilineTextInput) {
                n.children().filter_map(|c| rect_of(c.accesskit_node().bounding_box())).collect()
            } else {
                Vec::new()
            };
            let short: String = text.chars().take(40).collect();
            // (in a window, a menu or a popup over the panes: the panes' edges not its)
            let inner = chain.strip_prefix("/Window").unwrap_or(chain);
            let floating =
                inner.starts_with("/Unknown") || ["Window", "Dialog", "Menu"].iter().any(|w| inner.contains(w));
            let what = format!("{:?} {short:?}", a.role());
            widgets.push(Placed { what, rect: r, path: path.clone(), lines, floating });
            return;
        }
        _ => {}
    }
    // (the roles above it: whether it is in a window)
    let chain = format!("{chain}/{:?}", a.role());
    // a scroll bar up and down among the children: the others are scrolled, seen within its height
    let mut seen = seen;
    for c in n.children() {
        let b = c.accesskit_node();
        if b.role() == Role::ScrollBar
            && let Some(r) = rect_of(b.bounding_box())
            && r.height() > r.width()
        {
            seen = seen.intersection(r.y_range());
        }
    }
    for (k, c) in n.children().enumerate() {
        path.push(k);
        placed(c, &chain, seen, path, widgets, separators, bars);
        path.pop();
    }
}

/// What does not fit in the frame drawn: widgets over each other (within a pane or a bar), past the window's
/// sides, across the edge of their pane (a separator of the dock), a scroll bar sideways where the text should
/// wrap (unless `sideways` is allowed), the text of a field taller than the field.
fn misfits(h: &Harness<TerrafluxApp>, sideways: bool) -> Vec<String> {
    let width = h.ctx.content_rect().width();
    let (mut widgets, mut separators, mut bars) = (Vec::new(), Vec::new(), Vec::new());
    placed(h.root(), "", egui::Rangef::EVERYTHING, &mut Vec::new(), &mut widgets, &mut separators, &mut bars);
    let at = |r: egui::Rect| format!("[{:.0},{:.0} {:.0}x{:.0}]", r.min.x, r.min.y, r.width(), r.height());
    let mut out = Vec::new();
    let (hidden, widgets): (Vec<Placed>, Vec<Placed>) = widgets.into_iter().partition(|w| w.path.is_empty());
    for (i, a) in widgets.iter().enumerate() {
        if a.rect.max.x > width + 1.0 || a.rect.min.x < -1.0 {
            out.push(format!("{} {} past the window's side", a.what, at(a.rect)));
        }
        for s in separators.iter().filter(|_| !a.floating) {
            let along = a.rect.y_range().intersection(s.y_range()).span() > 2.0;
            if along && a.rect.min.x < s.min.x - 1.0 && a.rect.max.x > s.max.x + 1.0 {
                out.push(format!("{} {} across a pane's edge at x {:.0}", a.what, at(a.rect), s.center().x));
            }
        }
        for l in &a.lines {
            if l.min.y < a.rect.min.y - 1.0 || l.max.y > a.rect.max.y + 1.0 {
                out.push(format!("{} {}: its text {} out of it", a.what, at(a.rect), at(*l)));
            }
        }
        for b in &widgets[i + 1..] {
            // (within the same pane or bar: their paths alike past the layers and the panes' frames)
            let common = a.path.iter().zip(&b.path).take_while(|(x, y)| x == y).count();
            let o = a.rect.intersect(b.rect);
            let nested = a.rect.contains_rect(b.rect) || b.rect.contains_rect(a.rect);
            if common >= 3 && o.width() > 1.0 && o.height() > 1.0 && !nested {
                out.push(format!("{} {} over {} {}", a.what, at(a.rect), b.what, at(b.rect)));
            }
        }
    }
    if !sideways {
        // (the culprits within the dock: below the top of its separators, above the bar)
        let top = separators.iter().map(|s| s.min.y).fold(f32::INFINITY, f32::min) - 40.0;
        for b in &bars {
            let wide = widgets.iter().chain(&hidden).filter(|w| {
                b.x_range().contains(w.rect.min.x)
                    && w.rect.max.x > b.max.x + 1.0
                    && w.rect.min.y > top
                    && w.rect.max.y <= b.min.y + 1.0
            });
            let wide: Vec<String> = wide.map(|w| format!("{} {}", w.what, at(w.rect))).take(3).collect();
            out.push(format!("a scroll bar sideways at {}: too wide for its pane: {}", at(*b), wide.join(", ")));
        }
    }
    out
}

/// The panes, menus and windows at every size of the text the settings offer, classic and modern: nothing drawn
/// over something else, past the window or its pane, or out of its field. (`TERRAFLUX_SHOTS` set: each drawn in
/// the tests' folder, `ui-fit-<theme>-<text>-<view>.png`.)
#[test]
fn the_text_fits_at_every_size() {
    let mut found = Vec::new();
    for theme in ["Default", "Slate", "Paper"] {
        for text in [1.0, 1.3, 1.6] {
            terraflux_app::settings::update(|s| {
                s.theme = theme.into();
                s.text = text;
            });
            let mut h = harness(new_app(Some(model())));
            loaded(&mut h);
            let mut look = |h: &mut Harness<TerrafluxApp>, view: &str, sideways: bool| {
                if std::env::var("TERRAFLUX_WIDTHS").is_ok() {
                    h.ctx.all_styles_mut(|s| s.debug.show_expand_width = true);
                }
                h.run_steps(4);
                if std::env::var("TERRAFLUX_SHOTS").is_ok() {
                    shot(h, &format!("fit-{theme}-{text}-{view}"));
                }
                for m in misfits(h, sideways) {
                    found.push(format!("{theme} {text} {view}: {m}"));
                }
            };
            let views = [
                ("model", Pane::Model, Some("markov")),
                ("settings of a node", Pane::Node, Some("woe")),
                ("operators", Pane::Operators, None),
                ("files", Pane::Files, None),
                ("simulation", Pane::Simulation, None),
                ("prepare", Pane::Prepare, Some("$t0")),
                ("many runs", Pane::Batch, None),
                ("problems", Pane::Problems, None),
                ("assistant", Pane::Assistant, None),
                ("parameters", Pane::Parameters, None),
                ("document", Pane::Document, None),
                ("computers", Pane::Computers, None),
                ("areas", Pane::Areas, None),
            ];
            for (view, pane, node) in views {
                h.state_mut().dock = Some(terraflux_app::app::default_layout());
                let s = h.state_mut().session.as_mut().unwrap();
                s.selected = node.map(String::from);
                s.fields.clear();
                h.state_mut().focus(pane);
                look(&mut h, view, false);
            }
            // a table, wider than its pane: sideways
            h.state_mut().dock = Some(terraflux_app::app::default_layout());
            h.state_mut().session.as_mut().unwrap().preview = Some(("woe".into(), "table".into()));
            h.state_mut().focus(Pane::Viewer);
            look(&mut h, "table", true);
            h.state_mut().settings_open = true;
            look(&mut h, "settings", false);
            h.state_mut().settings_open = false;
            h.state_mut().shortcuts_open = true;
            look(&mut h, "shortcuts", false);
            h.state_mut().shortcuts_open = false;
            h.state_mut().commands = Some(Default::default());
            look(&mut h, "commands", false);
            h.state_mut().commands = None;
            for menu in ["File", "Templates", "View"] {
                h.run_steps(2);
                h.get_by_label(menu).click();
                look(&mut h, &format!("menu {menu}"), false);
                h.key_press(egui::Key::Escape);
            }
        }
    }
    terraflux_app::settings::update(|s| {
        s.theme = "Default".into();
        s.text = 1.0;
    });
    let _ = std::fs::write(tmp("misfits.txt"), found.join("\n"));
    let shown = found[..found.len().min(40)].join("\n");
    assert!(found.is_empty(), "{} things do not fit (all in misfits.txt):\n{shown}", found.len());
}
