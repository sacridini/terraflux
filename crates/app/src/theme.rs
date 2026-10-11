//! The look of the application: mid greys in flat panels, square corners, compact rows and one warm accent
//! for what is selected or active (after Ableton Live's).

use egui::{Color32, CornerRadius, Margin, Shadow, Stroke, Style, Vec2, Visuals};

/// The accent: what is selected, active or running.
pub const ACCENT: Color32 = Color32::from_rgb(255, 166, 52);
/// Text on the accent.
pub const ON_ACCENT: Color32 = Color32::from_rgb(28, 28, 28);
/// The bars at the top, a shade darker than the panels.
pub const BAR: Color32 = Color32::from_rgb(58, 58, 58);
/// The canvas of the model builder, a shade below the panels.
pub const CANVAS: Color32 = Color32::from_rgb(62, 62, 62);
/// Behind maps, charts and text fields.
pub const WELL: Color32 = Color32::from_rgb(40, 40, 40);

const PANEL: Color32 = Color32::from_rgb(74, 74, 74);
const RAISED: Color32 = Color32::from_rgb(96, 96, 96);
const HOVER: Color32 = Color32::from_rgb(112, 112, 112);
const LINE: Color32 = Color32::from_rgb(52, 52, 52);
const TEXT: Color32 = Color32::from_rgb(222, 222, 222);
const WEAK: Color32 = Color32::from_rgb(160, 160, 160);

pub fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    let r2 = CornerRadius::same(2);
    v.panel_fill = PANEL;
    v.window_fill = Color32::from_rgb(82, 82, 82);
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(3);
    v.menu_corner_radius = CornerRadius::same(3);
    v.window_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(90) };
    v.popup_shadow = Shadow { offset: [0, 3], blur: 8, spread: 0, color: Color32::from_black_alpha(80) };
    v.faint_bg_color = Color32::from_rgb(80, 80, 80);
    v.extreme_bg_color = WELL;
    v.text_edit_bg_color = Some(Color32::from_rgb(50, 50, 50));
    v.code_bg_color = Color32::from_rgb(60, 60, 60);
    v.weak_text_color = Some(WEAK);
    v.hyperlink_color = Color32::from_rgb(120, 196, 255);
    v.warn_fg_color = Color32::from_rgb(255, 208, 92);
    v.error_fg_color = Color32::from_rgb(255, 112, 98);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, ON_ACCENT);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = r2;
    w.inactive.bg_fill = RAISED;
    w.inactive.weak_bg_fill = Color32::from_rgb(88, 88, 88);
    w.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(62, 62, 62));
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = r2;
    w.hovered.bg_fill = HOVER;
    w.hovered.weak_bg_fill = HOVER;
    w.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(140, 140, 140));
    w.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    w.hovered.corner_radius = r2;
    w.hovered.expansion = 0.0;
    // pressed: lighter still (strong text takes this colour too); the accent is for what is on or selected
    w.active.bg_fill = Color32::from_rgb(132, 132, 132);
    w.active.weak_bg_fill = Color32::from_rgb(132, 132, 132);
    w.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(160, 160, 160));
    w.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    w.active.corner_radius = r2;
    w.active.expansion = 0.0;
    w.open.bg_fill = HOVER;
    w.open.weak_bg_fill = Color32::from_rgb(88, 88, 88);
    w.open.bg_stroke = Stroke::new(1.0, LINE);
    w.open.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    w.open.corner_radius = r2;
    v
}

/// The style of every frame.
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |s: &mut Style| {
        s.visuals = visuals();
        s.spacing.item_spacing = Vec2::new(6.0, 4.0);
        s.spacing.button_padding = Vec2::new(7.0, 2.0);
        s.spacing.interact_size.y = 20.0;
        s.spacing.window_margin = Margin::same(8);
        s.spacing.menu_margin = Margin::same(4);
        s.spacing.slider_rail_height = 4.0;
        s.interaction.tooltip_delay = 0.3;
    });
}

/// A heading of a side panel: small capitals in the weak colour.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text.to_uppercase()).size(10.5).color(WEAK).strong());
}
