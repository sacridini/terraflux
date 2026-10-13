//! The look of the application: flat panels, square corners, compact rows and one accent for what is selected
//! or active (after Ableton Live's). Several themes, each a palette (the mid greys of the default, a light one,
//! greens, near black, warm golds), chosen in the settings; the text's size and the interface's zoom with them.

use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, CornerRadius, FontId, Margin, Shadow, Stroke, Style, TextStyle, Vec2, Visuals};

/// A theme's colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub name: &'static str,
    pub dark: bool,
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

/// The themes.
pub const THEMES: [Palette; 5] = [
    Palette {
        name: "Default",
        dark: true,
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
        weak: rgb(160, 160, 160),
    },
    // light and airy, a cool accent
    Palette {
        name: "Immaterial",
        dark: false,
        accent: rgb(0, 148, 204),
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
        weak: rgb(108, 108, 108),
    },
    // the greens of a river's bank
    Palette {
        name: "Riparian",
        dark: true,
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
        weak: rgb(148, 170, 158),
    },
    // stark: near black, a red accent
    Palette {
        name: "Angst Robot",
        dark: true,
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
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The theme in use.
pub fn current() -> &'static Palette {
    &THEMES[CURRENT.load(Ordering::Relaxed).min(THEMES.len() - 1)]
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
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    let r2 = CornerRadius::same(2);
    let shade = if p.dark { 90 } else { 50 };
    v.panel_fill = p.panel;
    v.window_fill = p.window;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = CornerRadius::same(3);
    v.menu_corner_radius = CornerRadius::same(3);
    v.window_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(shade) };
    v.popup_shadow = Shadow { offset: [0, 3], blur: 8, spread: 0, color: Color32::from_black_alpha(shade - 10) };
    v.faint_bg_color = p.window;
    v.extreme_bg_color = p.well;
    v.text_edit_bg_color = Some(if p.dark { p.well.gamma_multiply(1.25) } else { p.well });
    v.code_bg_color = p.canvas;
    v.weak_text_color = Some(p.weak);
    v.override_text_color = None;
    if p.dark {
        v.hyperlink_color = rgb(120, 196, 255);
        v.warn_fg_color = rgb(255, 208, 92);
        v.error_fg_color = rgb(255, 112, 98);
    } else {
        v.hyperlink_color = rgb(0, 102, 204);
        v.warn_fg_color = rgb(176, 112, 0);
        v.error_fg_color = rgb(200, 40, 30);
    }
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0, p.on_accent);
    let strong = if p.dark { Color32::WHITE } else { Color32::BLACK };

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
    v
}

/// The style of every frame, in the theme chosen, its text `text` times egui's sizes.
pub fn apply_with(ctx: &egui::Context, text: f32) {
    let p = current();
    let theme = if p.dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);
    let text = text.clamp(0.6, 2.0);
    ctx.style_mut_of(theme, |s: &mut Style| {
        s.visuals = visuals();
        s.spacing.item_spacing = Vec2::new(6.0, 4.0);
        s.spacing.button_padding = Vec2::new(7.0, 2.0);
        s.spacing.interact_size.y = 20.0 * text.max(1.0);
        s.spacing.window_margin = Margin::same(8);
        s.spacing.menu_margin = Margin::same(4);
        s.spacing.slider_rail_height = 4.0;
        s.interaction.tooltip_delay = 0.3;
        let size = |base: f32| base * text;
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(size(9.0))),
            (TextStyle::Body, FontId::proportional(size(12.5))),
            (TextStyle::Button, FontId::proportional(size(12.5))),
            (TextStyle::Heading, FontId::proportional(size(18.0))),
            (TextStyle::Monospace, FontId::monospace(size(12.0))),
        ]
        .into();
    });
}

/// The style of every frame, in the theme chosen.
pub fn apply(ctx: &egui::Context) {
    apply_with(ctx, 1.0);
}

/// A heading of a side panel: small capitals in the weak colour.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text.to_uppercase()).size(10.5).color(weak()).strong());
}
