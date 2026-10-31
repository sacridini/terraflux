//! The look of the application, in two shapes. The classic: flat panels, square corners, compact rows and one
//! accent for what is selected or active (after Ableton Live's). The modern: rounded corners, more room, softer
//! shadows and a font of its own (Inter; the website's Source Sans 3 in Paper). Several themes, each a palette (the mid greys of the default, two light ones,
//! greens, near black, warm golds; the modern Slate, Paper and Canopy) in one of the shapes, chosen in the
//! settings; the text's size and the interface's zoom with them. Every theme's text, weak text included, reads at
//! 4.5:1 at least on its panels and canvas (WCAG's AA).

use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, CornerRadius, FontId, Margin, Shadow, Stroke, Style, TextStyle, Vec2, Visuals};

/// The shapes of a look: the corners, the room between things, the shadows and the font.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    /// Of buttons, fields and the like; of windows and menus; of the boxes of the model builder's canvas.
    pub widget_radius: u8,
    pub window_radius: u8,
    pub node_radius: f32,
    pub item_spacing: Vec2,
    pub button_padding: Vec2,
    /// A row's height (at text size 1).
    pub row: f32,
    pub window_margin: i8,
    pub menu_margin: i8,
    /// A window's shadow: its offset down and its blur.
    pub shadow: (i8, u8),
    /// The text's sizes: small, body, heading, monospace.
    pub sizes: [f32; 4],
}

/// The font of a theme's text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sans {
    /// egui's own.
    Egui,
    Inter,
    /// The website's.
    SourceSans,
}

/// The classic shape: square corners, compact.
pub const CLASSIC: Shape = Shape {
    widget_radius: 2,
    window_radius: 3,
    node_radius: 4.0,
    item_spacing: Vec2::new(6.0, 4.0),
    button_padding: Vec2::new(7.0, 2.0),
    row: 20.0,
    window_margin: 8,
    menu_margin: 4,
    shadow: (4, 12),
    sizes: [9.0, 12.5, 18.0, 12.0],
};

/// The modern shape: rounded, more room, Inter.
pub const MODERN: Shape = Shape {
    widget_radius: 6,
    window_radius: 10,
    node_radius: 10.0,
    item_spacing: Vec2::new(7.0, 5.0),
    button_padding: Vec2::new(8.0, 3.0),
    row: 22.0,
    window_margin: 12,
    menu_margin: 6,
    shadow: (8, 24),
    sizes: [9.5, 12.0, 18.0, 11.5],
};

/// A theme's colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub name: &'static str,
    pub dark: bool,
    /// The modern shape ([`MODERN`]), else the classic.
    pub modern: bool,
    pub sans: Sans,
    /// What is selected, active or running; text on it.
    pub accent: Color32,
    pub on_accent: Color32,
    /// The bars at the top, a shade darker than the panels.
    pub bar: Color32,
    /// The canvas of the model builder.
    pub canvas: Color32,
    /// Behind maps, charts and text fields.
    pub well: Color32,
    pub panel: Color32,
    pub window: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub line: Color32,
    pub text: Color32,
    pub weak: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// The themes: the classic, then the modern.
pub const THEMES: [Palette; 9] = [
    Palette {
        name: "Default",
        dark: true,
        modern: false,
        sans: Sans::Egui,
        accent: rgb(255, 166, 52),
        on_accent: rgb(28, 28, 28),
        bar: rgb(58, 58, 58),
        canvas: rgb(62, 62, 62),
        well: rgb(40, 40, 40),
        panel: rgb(74, 74, 74),
        window: rgb(82, 82, 82),
        raised: rgb(96, 96, 96),
        hover: rgb(112, 112, 112),
        pressed: rgb(132, 132, 132),
        line: rgb(52, 52, 52),
        text: rgb(222, 222, 222),
        weak: rgb(186, 186, 186),
    },
    // light and airy, a cool accent
    Palette {
        name: "Immaterial",
        dark: false,
        modern: false,
        sans: Sans::Egui,
        accent: rgb(0, 118, 170),
        on_accent: rgb(255, 255, 255),
        bar: rgb(206, 206, 206),
        canvas: rgb(234, 234, 234),
        well: rgb(247, 247, 247),
        panel: rgb(224, 224, 224),
        window: rgb(232, 232, 232),
        raised: rgb(198, 198, 198),
        hover: rgb(184, 184, 184),
        pressed: rgb(168, 168, 168),
        line: rgb(176, 176, 176),
        text: rgb(32, 32, 32),
        weak: rgb(86, 86, 86),
    },
    // warm paper and a forest's green: a light theme for daylight, a projector, print
    Palette {
        name: "Field Notes",
        dark: false,
        modern: false,
        sans: Sans::Egui,
        accent: rgb(26, 112, 76),
        on_accent: rgb(255, 255, 255),
        bar: rgb(222, 217, 205),
        canvas: rgb(238, 234, 224),
        well: rgb(251, 249, 244),
        panel: rgb(231, 226, 215),
        window: rgb(239, 235, 226),
        raised: rgb(212, 206, 192),
        hover: rgb(199, 192, 177),
        pressed: rgb(184, 177, 161),
        line: rgb(186, 179, 164),
        text: rgb(33, 31, 27),
        weak: rgb(92, 86, 75),
    },
    // the greens of a river's bank
    Palette {
        name: "Riparian",
        dark: true,
        modern: false,
        sans: Sans::Egui,
        accent: rgb(120, 214, 160),
        on_accent: rgb(18, 34, 26),
        bar: rgb(40, 52, 47),
        canvas: rgb(46, 59, 53),
        well: rgb(27, 36, 32),
        panel: rgb(52, 66, 60),
        window: rgb(58, 74, 67),
        raised: rgb(70, 88, 80),
        hover: rgb(86, 108, 98),
        pressed: rgb(104, 130, 118),
        line: rgb(34, 44, 40),
        text: rgb(220, 232, 224),
        weak: rgb(156, 177, 165),
    },
    // stark: near black, a red accent
    Palette {
        name: "Angst Robot",
        dark: true,
        modern: false,
        sans: Sans::Egui,
        accent: rgb(255, 58, 74),
        on_accent: rgb(16, 16, 16),
        bar: rgb(20, 20, 22),
        canvas: rgb(24, 24, 26),
        well: rgb(10, 10, 12),
        panel: rgb(30, 30, 33),
        window: rgb(36, 36, 40),
        raised: rgb(48, 48, 53),
        hover: rgb(64, 64, 70),
        pressed: rgb(84, 84, 92),
        line: rgb(8, 8, 9),
        text: rgb(236, 236, 236),
        weak: rgb(138, 138, 146),
    },
    // warm browns and gold
    Palette {
        name: "Twenty Four Carat",
        dark: true,
        modern: false,
        sans: Sans::Egui,
        accent: rgb(232, 186, 72),
        on_accent: rgb(30, 24, 14),
        bar: rgb(42, 36, 28),
        canvas: rgb(48, 41, 32),
        well: rgb(28, 24, 18),
        panel: rgb(54, 46, 36),
        window: rgb(62, 53, 41),
        raised: rgb(74, 64, 50),
        hover: rgb(92, 80, 62),
        pressed: rgb(112, 98, 76),
        line: rgb(36, 30, 23),
        text: rgb(240, 228, 206),
        weak: rgb(176, 160, 134),
    },
    // modern: a cool dark slate, a blue accent (as the editors of code and design of today)
    Palette {
        name: "Slate",
        dark: true,
        modern: true,
        sans: Sans::Inter,
        accent: rgb(110, 150, 255),
        on_accent: rgb(12, 16, 28),
        bar: rgb(23, 25, 29),
        canvas: rgb(28, 30, 35),
        well: rgb(19, 21, 24),
        panel: rgb(33, 36, 41),
        window: rgb(40, 43, 50),
        raised: rgb(50, 54, 62),
        hover: rgb(62, 67, 77),
        pressed: rgb(76, 82, 94),
        line: rgb(47, 51, 58),
        text: rgb(230, 232, 236),
        weak: rgb(158, 164, 176),
    },
    // modern: the website's (its paper, white panels on it, its rules, ink and forest green; Source Sans 3)
    Palette {
        name: "Paper",
        dark: false,
        modern: true,
        sans: Sans::SourceSans,
        accent: rgb(51, 128, 79),
        on_accent: rgb(255, 255, 255),
        bar: rgb(244, 246, 243),
        canvas: rgb(244, 246, 243),
        well: rgb(240, 243, 239),
        panel: rgb(255, 255, 255),
        window: rgb(255, 255, 255),
        raised: rgb(238, 242, 237),
        hover: rgb(228, 234, 228),
        pressed: rgb(217, 224, 217),
        line: rgb(217, 224, 217),
        text: rgb(28, 36, 32),
        weak: rgb(93, 104, 98),
    },
    // modern: the deep greens of a forest's canopy, a mint accent
    Palette {
        name: "Canopy",
        dark: true,
        modern: true,
        sans: Sans::Inter,
        accent: rgb(96, 214, 164),
        on_accent: rgb(10, 30, 22),
        bar: rgb(16, 25, 23),
        canvas: rgb(20, 31, 29),
        well: rgb(12, 19, 18),
        panel: rgb(25, 38, 35),
        window: rgb(31, 46, 43),
        raised: rgb(40, 58, 54),
        hover: rgb(52, 74, 69),
        pressed: rgb(66, 92, 86),
        line: rgb(35, 51, 47),
        text: rgb(226, 236, 232),
        weak: rgb(150, 174, 166),
    },
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The theme in use.
pub fn current() -> &'static Palette {
    &THEMES[CURRENT.load(Ordering::Relaxed).min(THEMES.len() - 1)]
}

/// The shape of the theme in use.
pub fn shape() -> &'static Shape {
    if current().modern { &MODERN } else { &CLASSIC }
}

/// Use a theme, by name (the default for a name unknown); [`apply`] then.
pub fn choose(name: &str) {
    CURRENT.store(THEMES.iter().position(|t| t.name == name).unwrap_or(0), Ordering::Relaxed);
}

pub fn accent() -> Color32 {
    current().accent
}

pub fn on_accent() -> Color32 {
    current().on_accent
}

/// Orange, whatever the theme: a button to be seen at once (the Node pane's Expand). Dark text on it.
pub fn orange() -> Color32 {
    rgb(255, 140, 26)
}

pub fn on_orange() -> Color32 {
    rgb(28, 28, 28)
}

pub fn bar() -> Color32 {
    current().bar
}

pub fn canvas() -> Color32 {
    current().canvas
}

pub fn well() -> Color32 {
    current().well
}

pub fn weak() -> Color32 {
    current().weak
}

pub fn visuals() -> Visuals {
    let p = current();
    let sh = shape();
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    let r2 = CornerRadius::same(sh.widget_radius);
    let shade = match (p.dark, p.modern) {
        (true, _) => 90,
        (false, false) => 50,
        (false, true) => 28,
    };
    v.panel_fill = p.panel;
    v.window_fill = p.window;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = CornerRadius::same(sh.window_radius);
    v.menu_corner_radius = CornerRadius::same(sh.window_radius.min(8));
    let (down, blur) = sh.shadow;
    v.window_shadow = Shadow { offset: [0, down], blur, spread: 0, color: Color32::from_black_alpha(shade) };
    v.popup_shadow =
        Shadow { offset: [0, down - 1], blur: blur * 2 / 3, spread: 0, color: Color32::from_black_alpha(shade - 10) };
    v.faint_bg_color = p.window;
    v.extreme_bg_color = p.well;
    v.text_edit_bg_color = Some(if p.dark { p.well.gamma_multiply(1.25) } else { p.well });
    v.code_bg_color = p.canvas;
    v.weak_text_color = Some(p.weak);
    v.override_text_color = None;
    if !p.dark && p.modern {
        // the website's: its links in forest ink, its ember, its danger
        v.hyperlink_color = rgb(42, 107, 68);
        v.warn_fg_color = rgb(154, 88, 7);
        v.error_fg_color = rgb(180, 55, 47);
    } else if p.dark {
        v.hyperlink_color = rgb(120, 196, 255);
        v.warn_fg_color = rgb(255, 208, 92);
        v.error_fg_color = rgb(255, 112, 98);
    } else {
        // (dark enough to read at 4.5:1 on the light panels)
        v.hyperlink_color = rgb(0, 88, 178);
        v.warn_fg_color = rgb(138, 84, 0);
        v.error_fg_color = rgb(178, 30, 24);
    }
    let strong = if p.dark { Color32::WHITE } else { Color32::BLACK };
    // what is selected in a list, a tab or a text: the accent softened into the panel, its text strong; the
    // accent itself is for the main action (Run) and the node selected on the canvas
    // (light and modern: softer, as the website's current page)
    v.selection.bg_fill = p.accent.lerp_to_gamma(p.panel, if p.modern && !p.dark { 0.84 } else { 0.6 });
    v.selection.stroke = Stroke::new(1.0, strong);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r2;
    w.inactive.bg_fill = p.raised;
    w.inactive.weak_bg_fill = p.raised.lerp_to_gamma(p.panel, 0.5);
    w.inactive.bg_stroke = Stroke::new(1.0, p.line.lerp_to_gamma(p.raised, 0.5));
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r2;
    w.hovered.bg_fill = p.hover;
    w.hovered.weak_bg_fill = p.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.pressed);
    w.hovered.fg_stroke = Stroke::new(1.0, strong);
    w.hovered.corner_radius = r2;
    w.hovered.expansion = 0.0;
    // pressed: further still (strong text takes this colour too); the accent is for what is on or selected
    w.active.bg_fill = p.pressed;
    w.active.weak_bg_fill = p.pressed;
    w.active.bg_stroke = Stroke::new(1.0, p.pressed.lerp_to_gamma(strong, 0.2));
    w.active.fg_stroke = Stroke::new(1.0, strong);
    w.active.corner_radius = r2;
    w.active.expansion = 0.0;
    w.open.bg_fill = p.hover;
    w.open.weak_bg_fill = p.raised.lerp_to_gamma(p.panel, 0.5);
    w.open.bg_stroke = Stroke::new(1.0, p.line);
    w.open.fg_stroke = Stroke::new(1.0, strong);
    w.open.corner_radius = r2;
    if p.modern && !p.dark {
        // the website's fields and buttons: white, edged in its darker rule
        v.text_edit_bg_color = Some(p.panel);
        w.inactive.bg_stroke = Stroke::new(1.0, rgb(197, 206, 198));
    }
    v
}

/// The style of every frame, in the theme chosen, its text `text` times egui's sizes.
pub fn apply_with(ctx: &egui::Context, text: f32) {
    let p = current();
    let sh = *shape();
    let theme = if p.dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);
    ctx.set_fonts(fonts(p.sans));
    let text = text.clamp(0.6, 2.0);
    ctx.style_mut_of(theme, |s: &mut Style| {
        s.visuals = visuals();
        s.spacing.item_spacing = sh.item_spacing;
        s.spacing.button_padding = sh.button_padding;
        s.spacing.interact_size.y = sh.row * text.max(1.0);
        s.spacing.window_margin = Margin::same(sh.window_margin);
        s.spacing.menu_margin = Margin::same(sh.menu_margin);
        s.spacing.slider_rail_height = 4.0;
        s.interaction.tooltip_delay = 0.3;
        let size = |base: f32| base * text;
        let [small, body, heading, mono] = sh.sizes;
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(size(small))),
            (TextStyle::Body, FontId::proportional(size(body))),
            (TextStyle::Button, FontId::proportional(size(body))),
            (TextStyle::Heading, FontId::proportional(size(heading))),
            (TextStyle::Monospace, FontId::monospace(size(mono))),
        ]
        .into();
    });
}

/// egui's fonts, the theme's first for the text (egui's for the signs it lacks).
fn fonts(sans: Sans) -> egui::FontDefinitions {
    let mut f = egui::FontDefinitions::default();
    let (name, data) = match sans {
        Sans::Egui => return f,
        Sans::Inter => ("Inter", egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf"))),
        // (its letters smaller than Inter's at a size: scaled to read alike)
        Sans::SourceSans => (
            "Source Sans 3",
            egui::FontData::from_static(include_bytes!("../assets/fonts/SourceSans3-Regular.ttf"))
                .tweak(egui::FontTweak { scale: 1.12, ..Default::default() }),
        ),
    };
    f.font_data.insert(name.into(), std::sync::Arc::new(data));
    f.families.entry(egui::FontFamily::Proportional).or_default().insert(0, name.into());
    f
}

/// The style of every frame, in the theme chosen.
pub fn apply(ctx: &egui::Context) {
    apply_with(ctx, 1.0);
}

/// A bar of how far something is (`fraction`: 0 to 1; none: it cannot say, the bar moves) `width` points wide,
/// and its words beside it in the text's colour, readable whatever the bar shows under them.
pub fn progress(ui: &mut egui::Ui, fraction: Option<f32>, text: &str, width: f32) -> egui::Response {
    ui.horizontal(|ui| {
        let bar =
            egui::ProgressBar::new(fraction.unwrap_or(0.0)).desired_width(width).desired_height(6.0).fill(accent());
        let bar = ui.add(if fraction.is_none() { bar.animate(true) } else { bar });
        ui.add(egui::Label::new(egui::RichText::new(text).size(12.0)).truncate());
        bar
    })
    .inner
    .on_hover_text(text)
}

/// A heading of a side panel: small capitals in the weak colour.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text.to_uppercase()).size(10.5).color(weak()).strong());
}

/// The modern shape in use.
pub fn modern() -> bool {
    current().modern
}

/// In the modern shape, the buttons of `ui` without a frame until the pointer is over them (a bar's).
pub fn quiet(ui: &mut egui::Ui) {
    if modern() {
        let w = &mut ui.visuals_mut().widgets;
        w.inactive.weak_bg_fill = Color32::TRANSPARENT;
        w.inactive.bg_stroke = Stroke::NONE;
    }
}

/// A part of a side panel under its heading: in the modern shape a card (rounded, a shade off the panel, the
/// heading in small capitals), in the classic a line above and the heading in bold. No heading for "".
pub fn card<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    if !modern() {
        ui.separator();
        if !title.is_empty() {
            ui.label(egui::RichText::new(title).strong());
        }
        return add(ui);
    }
    let p = current();
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(p.window)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(8)
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(egui::RichText::new(title.to_uppercase()).size(10.5).color(weak()).strong());
                ui.add_space(2.0);
            }
            add(ui)
        })
        .inner
}

/// What `add` shows in a row, in the modern shape in a pill of the panel's colour (the bar's model and its run).
pub fn pill<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    if !modern() {
        return add(ui);
    }
    egui::Frame::new()
        .fill(current().panel)
        .corner_radius(7)
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| ui.horizontal(add).inner)
        .inner
}

/// The radius of a frame's corners: `classic` in the classic shape, rounder in the modern.
pub fn rounded(classic: f32) -> f32 {
    if modern() { (classic * 2.0).max(6.0) } else { classic }
}

/// A plot in the theme: in the modern shape its grid lighter (its lines further apart).
pub fn plot(p: egui_plot::Plot<'_>) -> egui_plot::Plot<'_> {
    if modern() { p.grid_spacing(egui::Rangef::new(28.0, 300.0)) } else { p }
}

/// A short text in a rounded chip (the modern shape), else as a label.
pub fn chip(ui: &mut egui::Ui, text: egui::RichText) -> egui::Response {
    if !modern() {
        return ui.label(text);
    }
    egui::Frame::new()
        .fill(well())
        .corner_radius(5)
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| ui.label(text))
        .inner
}

/// The room a button of this text takes in a row, the spacing before it included (at any size of the text).
pub fn button_room(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let w = ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER).size().x;
    w + 2.0 * ui.spacing().button_padding.x + ui.spacing().item_spacing.x + 1.0
}

/// A collapsing header whose title is cut short ("…") where its pane is narrow, rather than widening the pane:
/// the body's result when open.
pub fn fold<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    title: impl Into<egui::WidgetText>,
    open: bool,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    use egui::collapsing_header::{CollapsingState, paint_default_icon};
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, open);
    let header = ui.horizontal(|ui| {
        let spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing.x = 0.0;
        state.show_toggle_button(ui, paint_default_icon);
        ui.spacing_mut().item_spacing = spacing;
        ui.add(egui::Label::new(title).truncate().sense(egui::Sense::click())).clicked()
    });
    if header.inner {
        state.toggle(ui);
    }
    state.show_body_indented(&header.response, ui, add).map(|r| r.inner)
}

/// A choice on or off: a switch in the modern shape, else a checkbox (`ui.add` it, or `ui.add_enabled`).
pub fn toggle<'a>(on: &'a mut bool, text: &'a str) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| {
        if modern() { switch(ui, on, text) } else { ui.checkbox(on, text) }
    }
}

/// The modern shape's switch.
fn switch(ui: &mut egui::Ui, on: &mut bool, text: &str) -> egui::Response {
    let galley =
        egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, TextStyle::Body);
    let track = Vec2::new(26.0, 14.0);
    let size = Vec2::new(track.x + 6.0 + galley.size().x, ui.spacing().interact_size.y.max(galley.size().y));
    let (rect, mut resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, *on, text));
    if ui.is_rect_visible(rect) {
        let p = current();
        let t = ui.ctx().animate_bool_responsive(resp.id, *on);
        let track_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - track.y / 2.0), track);
        let off = match (p.dark, resp.hovered()) {
            (true, false) => p.hover,
            (true, true) => p.pressed,
            (false, false) => p.pressed,
            (false, true) => p.pressed.lerp_to_gamma(p.text, 0.15),
        };
        let fill = off.lerp_to_gamma(p.accent, t);
        let painter = ui.painter();
        painter.rect_filled(track_rect, track.y / 2.0, fill);
        let r = track.y / 2.0 - 2.0;
        let x = egui::lerp((track_rect.left() + r + 2.0)..=(track_rect.right() - r - 2.0), t);
        let knob = match (*on, p.dark) {
            (true, _) => p.on_accent,
            (false, true) => p.text,
            (false, false) => Color32::WHITE,
        };
        painter.circle_filled(egui::pos2(x, track_rect.center().y), r, knob);
        let at = egui::pos2(track_rect.right() + 6.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, p.text);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG's contrast of two colours (1 to 21).
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_theme_reads_at_aa() {
        for t in &THEMES {
            // (the modern look's cards in the windows' colour)
            let card = if t.modern { t.window } else { t.panel };
            for (what, bg) in [("panel", t.panel), ("canvas", t.canvas), ("bar", t.bar), ("card", card)] {
                assert!(contrast(t.text, bg) > contrast(t.weak, bg), "{}: text over weak text on {what}", t.name);
                assert!(contrast(t.weak, bg) >= 4.5, "{}: weak text on {what}: {}", t.name, contrast(t.weak, bg));
            }
            assert!(contrast(t.on_accent, t.accent) >= 4.5, "{}: text on the accent", t.name);
            // what is selected: the accent softened into the panel, its text strong
            let strong = if t.dark { Color32::WHITE } else { Color32::BLACK };
            let soft = if t.modern && !t.dark { 0.84 } else { 0.6 };
            assert!(contrast(strong, t.accent.lerp_to_gamma(t.panel, soft)) >= 4.5, "{}: the selection", t.name);
        }
    }
}
