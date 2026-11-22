//! The families of the operators: the subjects of the library (`builder::SUBJECTS`) gathered into thirteen, each
//! with an icon (Phosphor's) and a colour. A box of the canvas wears its family's icon at the left of its name and
//! a thin band of its colour along its left edge; the library, the search and the command palette write the same
//! icon before the operators and subjects. The colours are muted and of a lightness that reads on the theme in use
//! (lighter on a dark one, darker on a light one), so that they never shout over the ports' and wires' own.

use egui::Color32;
use egui_phosphor::regular as ph;

/// What an operator belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    Data,
    Raster,
    Vector,
    Terrain,
    Landscape,
    Connectivity,
    LandChange,
    Conservation,
    Statistics,
    Synthetic,
    Tables,
    View,
    ModelFlow,
}

/// The icon of the canvas's legend button.
pub const LEGEND_ICON: &str = ph::SHAPES;

/// Every family, in the order of the library.
pub const ALL: [Family; 13] = [
    Family::Data,
    Family::Raster,
    Family::Vector,
    Family::Terrain,
    Family::Landscape,
    Family::Connectivity,
    Family::LandChange,
    Family::Conservation,
    Family::Statistics,
    Family::Synthetic,
    Family::Tables,
    Family::View,
    Family::ModelFlow,
];

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Data => "Data",
            Family::Raster => "Raster",
            Family::Vector => "Vector & objects",
            Family::Terrain => "Terrain & water",
            Family::Landscape => "Landscape patterns",
            Family::Connectivity => "Connectivity",
            Family::LandChange => "Land change",
            Family::Conservation => "Conservation",
            Family::Statistics => "Statistics & ML",
            Family::Synthetic => "Synthetic landscapes",
            Family::Tables => "Tables",
            Family::View => "View",
            Family::ModelFlow => "Model flow",
        }
    }

    /// Its icon, a glyph of the Phosphor font (a fallback of every text).
    pub fn icon(self) -> &'static str {
        match self {
            Family::Data => ph::DATABASE,
            Family::Raster => ph::GRID_FOUR,
            Family::Vector => ph::POLYGON,
            Family::Terrain => ph::MOUNTAINS,
            Family::Landscape => ph::PUZZLE_PIECE,
            Family::Connectivity => ph::SHARE_NETWORK,
            Family::LandChange => ph::CLOCK_COUNTER_CLOCKWISE,
            Family::Conservation => ph::LEAF,
            Family::Statistics => ph::CHART_SCATTER,
            Family::Synthetic => ph::DICE_FIVE,
            Family::Tables => ph::TABLE,
            Family::View => ph::EYE,
            Family::ModelFlow => ph::FLOW_ARROW,
        }
    }

    /// Its colour on a dark or a light theme: a hue of its own, little saturation.
    pub fn colour(self, dark: bool) -> Color32 {
        let (hue, sat) = match self {
            Family::Data => (215.0, 0.24),
            Family::Raster => (197.0, 0.42),
            Family::Vector => (335.0, 0.38),
            Family::Terrain => (24.0, 0.40),
            Family::Landscape => (105.0, 0.34),
            Family::Connectivity => (262.0, 0.38),
            Family::LandChange => (42.0, 0.46),
            Family::Conservation => (148.0, 0.36),
            Family::Statistics => (296.0, 0.30),
            Family::Synthetic => (178.0, 0.36),
            Family::Tables => (62.0, 0.30),
            Family::View => (356.0, 0.40),
            Family::ModelFlow => (210.0, 0.06),
        };
        if dark { hsl(hue, sat, 0.67) } else { hsl(hue, sat + 0.08, 0.40) }
    }

    /// The family of an operator's subject (`None` for a subject that is no family's: "Other").
    pub fn of_subject(subject: &str) -> Option<Family> {
        Some(match subject {
            "IO" | "Data preparation" | "Large data & batch" => Family::Data,
            "Raster analysis" | "Map algebra" => Family::Raster,
            "Vector analysis" | "Objects (OBIA)" => Family::Vector,
            "Terrain hydrology" | "Surface metrics" => Family::Terrain,
            "Habitat patterns" | "Fragmentation" | "Landscape metrics" => Family::Landscape,
            "Habitat networks"
            | "Circuit connectivity"
            | "Grid distances"
            | "Distance & restoration"
            | "Animal movement" => Family::Connectivity,
            "Land change simulation" | "Change detection" => Family::LandChange,
            "Biodiversity analysis"
            | "Spatial prioritization"
            | "Species distributions"
            | "Ecosystem services"
            | "Carbon and REDD+"
            | "Forest Code (Brazil)" => Family::Conservation,
            "Machine learning" | "Map accuracy and area" | "Uncertainty" | "Spatial statistics" => Family::Statistics,
            "Neutral landscape models" => Family::Synthetic,
            "Tables" => Family::Tables,
            "Plot" => Family::View,
            "Model flow" => Family::ModelFlow,
            _ => return None,
        })
    }
}

/// The family of an operator: that of its subject in the library; the nodes of the user's and what is in no
/// subject are of the model's flow.
pub fn of(op: &str) -> Family {
    Family::of_subject(crate::builder::subject_of(op)).unwrap_or(Family::ModelFlow)
}

/// The font family of the icons: Phosphor's alone (a text font may have glyphs at the same private-use codepoints).
pub fn icon_family() -> egui::FontFamily {
    egui::FontFamily::Name("phosphor".into())
}

/// The font of an icon `size` points high (the text's until the fonts with the icons are in: they are set at a
/// frame's start and used from the next).
pub fn icon_font(ctx: &egui::Context, size: f32) -> egui::FontId {
    if ctx.fonts(|f| f.definitions().families.contains_key(&icon_family())) {
        egui::FontId::new(size, icon_family())
    } else {
        egui::FontId::proportional(size)
    }
}

/// A line of text with the icon of `family` before it, the icon in its colour (in `colour` at `size` the text).
pub fn titled(
    ctx: &egui::Context,
    family: Family,
    text: &str,
    size: f32,
    colour: Color32,
    dark: bool,
) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    let font = egui::FontId::proportional(size);
    let icon = TextFormat {
        font_id: icon_font(ctx, size * 0.95),
        color: family.colour(dark),
        valign: egui::Align::Center,
        ..Default::default()
    };
    job.append(family.icon(), 0.0, icon);
    job.append(
        text,
        6.0,
        TextFormat { font_id: font, color: colour, valign: egui::Align::Center, ..Default::default() },
    );
    job
}

/// A colour from hue (degrees), saturation and lightness.
fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(byte(r), byte(g), byte(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_operator_has_a_family() {
        let r = terraflux_ops::Registry::standard();
        for g in r.signatures() {
            let subject = crate::builder::subject_of(&g.name);
            assert!(Family::of_subject(subject).is_some(), "{} (subject {subject}) is of no family", g.name);
        }
        assert_eq!(of("control.repeat"), Family::ModelFlow);
        assert_eq!(of("raster.read"), Family::Data);
    }

    #[test]
    fn every_subject_is_in_a_family() {
        for (subject, _) in crate::builder::SUBJECTS.iter() {
            assert!(Family::of_subject(subject).is_some(), "the subject {subject} is of no family");
        }
        // (and every family has a subject, an icon and a colour of its own)
        let used: HashSet<Family> =
            crate::builder::SUBJECTS.iter().filter_map(|(s, _)| Family::of_subject(s)).collect();
        assert_eq!(used.len(), ALL.len());
        let icons: HashSet<&str> = ALL.iter().map(|f| f.icon()).collect();
        let colours: HashSet<Color32> = ALL.iter().map(|f| f.colour(true)).collect();
        assert_eq!((icons.len(), colours.len()), (13, 13));
    }
}
