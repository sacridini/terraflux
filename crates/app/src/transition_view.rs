//! The transitions of two maps of classes in a window of their own: the matrix of every class of one map to every
//! class of the other (in hectares, km², a share of the landscape or of the class it came from, or cells), each
//! class's gains, losses, net change and swap as bars, and the flows from one map to the other as a Sankey diagram.
//! A cell of the matrix (or a flow) clicked singles that transition out on the map.

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::difference::{Budget, Matrix};

/// How amounts of land are told.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Unit {
    #[default]
    Hectares,
    SquareKm,
    /// A share of everything compared (the landscape).
    Landscape,
    /// A share of the class it came from (the row of the matrix).
    Row,
    Cells,
}

impl Unit {
    pub fn name(self) -> &'static str {
        match self {
            Unit::Hectares => "hectares",
            Unit::SquareKm => "km²",
            Unit::Landscape => "% of the landscape",
            Unit::Row => "% of the class it came from",
            Unit::Cells => "cells",
        }
    }

    /// Whether it is an area (needs a coordinate system).
    pub fn is_area(self) -> bool {
        matches!(self, Unit::Hectares | Unit::SquareKm)
    }

    /// Its short name, after a number.
    pub fn short(self) -> &'static str {
        match self {
            Unit::Hectares => "ha",
            Unit::SquareKm => "km²",
            Unit::Landscape | Unit::Row => "%",
            Unit::Cells => "cells",
        }
    }
}

/// An amount in square metres (or cells for [`Unit::Cells`]) as a number of the unit: `whole` is what a share is of.
pub fn in_unit(v: f64, unit: Unit, whole: f64) -> f64 {
    match unit {
        Unit::Hectares => v / 1e4,
        Unit::SquareKm => v / 1e6,
        Unit::Landscape | Unit::Row => {
            if whole > 0.0 {
                100.0 * v / whole
            } else {
                0.0
            }
        }
        Unit::Cells => v,
    }
}

/// A number of a unit written short: cells whole, the others to two decimals (three below one, for small areas).
pub fn number(v: f64, unit: Unit) -> String {
    if unit == Unit::Cells {
        return format!("{}", v.round() as i64);
    }
    if v != 0.0 && v.abs() < 1.0 { format!("{v:.3}") } else { format!("{v:.2}") }
}

/// What the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Matrix,
    Budget,
    Sankey,
}

/// The window's state: open, its tab and unit.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub open: bool,
    pub tab: Tab,
    pub unit: Unit,
    /// Why the matrix was not saved.
    pub said: Option<String>,
    /// The flows of a series of maps: every step (true) or the first and the last (false); none: by the number of
    /// steps (see [`every`]).
    pub every: Option<bool>,
}

/// The most steps of a series whose flows can be shown one pair after the other.
pub const MAX_CHAIN: usize = 12;

/// Whether the flows of a series of `n` steps are shown step by step: the choice made, else every step when there
/// are three to eight, the first and the last above that.
pub fn every(state: &State, n: usize) -> bool {
    (3..=MAX_CHAIN).contains(&n) && state.every.unwrap_or(n <= 8)
}

/// The steps of a series: their names and the matrix of each consecutive pair (as many as are worked out).
pub struct Chain<'a> {
    pub names: &'a [String],
    pub matrices: Vec<&'a Matrix>,
    /// Pairs still being worked out.
    pub waiting: usize,
    /// Why a pair could not be made.
    pub error: Option<&'a str>,
}

impl Chain<'_> {
    /// Every pair's matrix is there.
    pub fn ready(&self) -> bool {
        self.waiting == 0 && self.error.is_none() && self.matrices.len() + 1 == self.names.len()
    }
}

/// The unit used: cells when the grid has no coordinate system and an area is asked for.
fn usable(m: &Matrix, unit: Unit) -> Unit {
    if unit.is_area() && m.area.is_none() { Unit::Cells } else { unit }
}

/// The amount of the matrix's cell (row i, column j) in a unit.
pub fn cell(m: &Matrix, i: usize, j: usize, unit: Unit) -> f64 {
    let unit = usable(m, unit);
    // (shares of the areas when known: cells of a grid in degrees are not all as large)
    let area = m.area.is_some() && unit != Unit::Cells;
    let a = m.amounts(area);
    let whole = match unit {
        Unit::Row => a[i].iter().sum(),
        _ => m.total(area),
    };
    in_unit(a[i][j], unit, whole)
}

/// The matrix as tab-separated text: a row per class of the map shown, a column per class of the other, the totals
/// last; then each class's gains and losses (the same unit; shares of the landscape for the row shares).
pub fn tsv(m: &Matrix, unit: Unit, from: &str, to: &str) -> String {
    let unit = usable(m, unit);
    let mut t = format!("{from} \\ {to} ({})", unit.short());
    for (_, n) in &m.to {
        t.push('\t');
        t.push_str(n);
    }
    t.push_str("\ttotal\n");
    for (i, (_, n)) in m.from.iter().enumerate() {
        t.push_str(n);
        let mut sum = 0.0;
        for j in 0..m.to.len() {
            let v = cell(m, i, j, unit);
            sum += v;
            t.push('\t');
            t.push_str(&number(v, unit));
        }
        t.push('\t');
        t.push_str(&number(sum, unit));
        t.push('\n');
    }
    let shares = if unit == Unit::Row { Unit::Landscape } else { unit };
    t.push_str(&format!("\nclass\tbefore\tafter\tgain\tloss\tnet\tswap ({})\n", shares.short()));
    let area = m.area.is_some() && shares != Unit::Cells;
    let whole = m.total(area);
    for b in m.budgets(area) {
        let f = |v: f64| number(in_unit(v, shares, whole), shares);
        let row = [f(b.before), f(b.after), f(b.gain), f(b.loss), f(b.net()), f(b.swap())];
        t.push_str(&format!("{}\t{}\n", b.name, row.join("\t")));
    }
    t
}

/// The unit used for the matrices of a series: cells when any has no area and an area is asked for.
fn usable_all(ms: &[&Matrix], unit: Unit) -> Unit {
    if unit.is_area() && ms.iter().any(|m| m.area.is_none()) { Unit::Cells } else { unit }
}

/// The flows of a series as tab-separated text: a block per pair of steps (a row per class of the first, a column
/// per class of the second, the totals last), each row starting with the two steps (`from_step`, `to_step`).
pub fn chain_tsv(ms: &[&Matrix], names: &[String], unit: Unit) -> String {
    let unit = usable_all(ms, unit);
    let mut t = String::new();
    for (p, m) in ms.iter().enumerate() {
        if p > 0 {
            t.push('\n');
        }
        t.push_str(&format!("from_step\tto_step\tclass ({})", unit.short()));
        for (_, n) in &m.to {
            t.push('\t');
            t.push_str(n);
        }
        t.push_str("\ttotal\n");
        for (i, (_, n)) in m.from.iter().enumerate() {
            t.push_str(&format!("{}\t{}\t{n}", names[p], names[p + 1]));
            let mut sum = 0.0;
            for j in 0..m.to.len() {
                let v = cell(m, i, j, unit);
                sum += v;
                t.push('\t');
                t.push_str(&number(v, unit));
            }
            t.push('\t');
            t.push_str(&number(sum, unit));
            t.push('\n');
        }
    }
    t
}

/// The window of a matrix (`names`: the maps compared, from and to), `colour` a class's colour (by its value).
/// With `focus`, a cell or flow clicked singles that transition out (clicked again, none). With `chain` (the steps of
/// a series), the flows can also be shown step by step, and copied and saved so.
#[allow(clippy::too_many_arguments)]
pub fn window(
    ctx: &egui::Context,
    id: egui::Id,
    m: &Matrix,
    names: (&str, &str),
    colour: &dyn Fn(i64) -> Color32,
    state: &mut State,
    mut focus: Option<&mut Option<(i64, i64)>>,
    chain: Option<&Chain>,
) {
    let mut open = state.open;
    egui::Window::new(format!("Transitions: {} to {}", names.0, names.1))
        .id(id)
        .open(&mut open)
        .default_size([620.0, 420.0])
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.tab, Tab::Matrix, "Matrix");
                ui.selectable_value(&mut state.tab, Tab::Budget, "Gains and losses");
                ui.selectable_value(&mut state.tab, Tab::Sankey, "Flows");
                ui.separator();
                let units: &[Unit] = if m.area.is_some() {
                    &[Unit::Hectares, Unit::SquareKm, Unit::Landscape, Unit::Row, Unit::Cells]
                } else {
                    &[Unit::Landscape, Unit::Row, Unit::Cells]
                };
                let shown = usable(m, state.unit);
                egui::ComboBox::from_id_salt(id.with("unit")).selected_text(shown.name()).show_ui(ui, |ui| {
                    for u in units {
                        ui.selectable_value(&mut state.unit, *u, u.name());
                    }
                });
                // (the flows step by step when they are shown so: a block a pair of steps)
                let steps = chain.filter(|c| state.tab == Tab::Sankey && every(state, c.names.len()) && c.ready());
                let text = |unit: Unit| match steps {
                    Some(c) => chain_tsv(&c.matrices, c.names, unit),
                    None => tsv(m, unit, names.0, names.1),
                };
                if ui.button("Copy").on_hover_text("the matrix and the gains and losses, tab-separated").clicked() {
                    ui.ctx().copy_text(text(state.unit));
                }
                if ui.button("Save CSV...").clicked()
                    && let Some(p) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name("transitions.csv").save_file()
                {
                    let text = text(state.unit).replace('\t', ",");
                    state.said = std::fs::write(&p, text)
                        .err()
                        .map(|e| format!("not written to {}: {e}", p.display()));
                }
            });
            let unit = usable(m, state.unit);
            let total = m.total(m.area.is_some());
            let kept = m.kept(m.area.is_some());
            let said = if m.area.is_some() {
                format!(
                    "{} ha compared, {} ha ({:.1}%) changed class",
                    number(total / 1e4, Unit::Hectares),
                    number((total - kept) / 1e4, Unit::Hectares),
                    if total > 0.0 { 100.0 * (total - kept) / total } else { 0.0 }
                )
            } else {
                format!(
                    "{} cells compared, {:.1}% changed class (no coordinate system: no area)",
                    total.round(),
                    if total > 0.0 { 100.0 * (total - kept) / total } else { 0.0 }
                )
            };
            ui.label(RichText::new(said).weak());
            if let Some(e) = &state.said {
                ui.label(RichText::new(e).color(ui.visuals().error_fg_color));
            }
            ui.separator();
            match state.tab {
                Tab::Matrix => {
                    if focus.is_some() {
                        ui.label(
                            RichText::new("rows: the classes before; columns: after. Click a cell to see only that transition on the map")
                                .weak()
                                .size(11.0),
                        );
                    }
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        matrix_grid(ui, id, m, unit, colour, focus.as_deref_mut());
                    });
                }
                Tab::Budget => {
                    let shares = if unit == Unit::Row { Unit::Landscape } else { unit };
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        budget_bars(ui, m, shares, colour);
                    });
                }
                Tab::Sankey => {
                    let shares = if unit == Unit::Row { Unit::Landscape } else { unit };
                    let Some(c) = chain else {
                        sankey(ui, m, shares, colour, focus);
                        return;
                    };
                    let n = c.names.len();
                    let now = every(state, n);
                    ui.horizontal(|ui| {
                        let mut set = state.every.unwrap_or(now);
                        ui.selectable_value(&mut set, false, "First and last");
                        ui.selectable_value(&mut set, true, "Every step");
                        if set != now || state.every.is_some() {
                            state.every = Some(set);
                        }
                        if n > 8 && !now {
                            ui.label(RichText::new(format!("{n} steps: the first and the last, by default")).weak().size(11.0));
                        }
                    });
                    if !every(state, n) {
                        sankey(ui, m, shares, colour, focus);
                    } else if let Some(e) = c.error {
                        ui.label(RichText::new(e).color(ui.visuals().error_fg_color));
                    } else if !c.ready() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("computing the flows of every step...").weak());
                        });
                    } else {
                        let shares = if usable_all(&c.matrices, shares) != shares { Unit::Cells } else { shares };
                        sankey_chain(ui, &c.matrices, c.names, shares, colour, focus);
                    }
                }
            }
        });
    state.open = open;
}

/// A swatch of a class's colour.
fn swatch(ui: &mut Ui, c: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().rect_filled(r, 2.0, crate::palette::seen(c));
}

/// The matrix as a grid: a row per class before, a column per class after, the totals at the end; each cell shaded
/// by its share of everything, the classes kept in bold.
fn matrix_grid(
    ui: &mut Ui,
    id: egui::Id,
    m: &Matrix,
    unit: Unit,
    colour: &dyn Fn(i64) -> Color32,
    mut focus: Option<&mut Option<(i64, i64)>>,
) {
    let area = m.area.is_some() && unit != Unit::Cells;
    let whole = m.total(area);
    let raw = m.amounts(area);
    let accent = crate::theme::accent();
    egui::Grid::new(id.with("grid")).striped(false).spacing([10.0, 4.0]).show(ui, |ui| {
        ui.label(RichText::new(format!("before \\ after ({})", unit.short())).weak().size(11.0));
        for (v, n) in &m.to {
            ui.horizontal(|ui| {
                swatch(ui, colour(*v));
                ui.label(RichText::new(n).strong());
            });
        }
        ui.label(RichText::new("total").weak());
        ui.end_row();
        for (i, (p, n)) in m.from.iter().enumerate() {
            ui.horizontal(|ui| {
                swatch(ui, colour(*p));
                ui.label(RichText::new(n).strong());
            });
            let mut sum = 0.0;
            for (j, (q, _)) in m.to.iter().enumerate() {
                let v = cell(m, i, j, unit);
                sum += v;
                let text = RichText::new(number(v, unit)).monospace();
                let text = if p == q { text.strong() } else { text };
                let share = if whole > 0.0 { (raw[i][j] / whole) as f32 } else { 0.0 };
                let chosen = focus.as_deref().is_some_and(|f| *f == Some((*p, *q)));
                let r =
                    ui.add(egui::Button::selectable(chosen, text).fill(accent.gamma_multiply((share * 3.0).min(0.6))));
                let r = r.on_hover_text(format!("{} -> {}", n, m.to[j].1));
                if r.clicked()
                    && let Some(f) = focus.as_deref_mut()
                {
                    *f = if chosen { None } else { Some((*p, *q)) };
                }
            }
            ui.label(RichText::new(number(sum, unit)).monospace().weak());
            ui.end_row();
        }
        // the totals after (shares of the row have none)
        if unit != Unit::Row {
            ui.label(RichText::new("total").weak());
            let mut all = 0.0;
            for j in 0..m.to.len() {
                let v: f64 = (0..m.from.len()).map(|i| cell(m, i, j, unit)).sum();
                all += v;
                ui.label(RichText::new(number(v, unit)).monospace().weak());
            }
            ui.label(RichText::new(number(all, unit)).monospace().weak());
            ui.end_row();
        }
    });
}

/// Each class's gains (right) and losses (left) as bars about a line, the net change marked, the numbers beside.
fn budget_bars(ui: &mut Ui, m: &Matrix, unit: Unit, colour: &dyn Fn(i64) -> Color32) {
    let area = m.area.is_some() && unit != Unit::Cells;
    let whole = m.total(area);
    let budgets: Vec<Budget> = m.budgets(area);
    let top = budgets.iter().map(|b| b.gain.max(b.loss)).fold(0.0, f64::max).max(f64::MIN_POSITIVE);
    let f = |v: f64| number(in_unit(v, unit, whole), unit);
    let (gain_c, loss_c) = (Color32::from_rgb(70, 160, 90), Color32::from_rgb(205, 85, 70));
    ui.horizontal(|ui| {
        ui.label(RichText::new("■ loss").color(loss_c));
        ui.label(RichText::new("■ gain").color(gain_c));
        ui.label(RichText::new("| net change").weak());
        ui.label(RichText::new(format!("({})", unit.short())).weak());
    });
    for b in &budgets {
        ui.horizontal(|ui| {
            swatch(ui, colour(b.class));
            ui.add_sized([120.0, 16.0], egui::Label::new(RichText::new(&b.name).strong()).truncate());
            let (r, resp) = ui.allocate_exact_size(Vec2::new(220.0, 16.0), Sense::hover());
            let mid = r.center().x;
            let half = r.width() / 2.0 - 2.0;
            let p = ui.painter();
            let lw = half * (b.loss / top) as f32;
            let gw = half * (b.gain / top) as f32;
            p.rect_filled(
                Rect::from_min_max(Pos2::new(mid - lw, r.top() + 2.0), Pos2::new(mid, r.bottom() - 2.0)),
                1.0,
                loss_c,
            );
            p.rect_filled(
                Rect::from_min_max(Pos2::new(mid, r.top() + 2.0), Pos2::new(mid + gw, r.bottom() - 2.0)),
                1.0,
                gain_c,
            );
            let nx = mid + half * (b.net() / top) as f32;
            p.line_segment(
                [Pos2::new(nx, r.top()), Pos2::new(nx, r.bottom())],
                Stroke::new(2.0, ui.visuals().strong_text_color()),
            );
            p.line_segment(
                [Pos2::new(mid, r.top()), Pos2::new(mid, r.bottom())],
                Stroke::new(1.0, ui.visuals().weak_text_color()),
            );
            resp.on_hover_text(format!(
                "{}: before {}, after {}\ngain {}, loss {}\nnet {}, swap {}",
                b.name,
                f(b.before),
                f(b.after),
                f(b.gain),
                f(b.loss),
                f(b.net()),
                f(b.swap())
            ));
            ui.label(
                RichText::new(format!("+{} −{}  net {}  swap {}", f(b.gain), f(b.loss), f(b.net()), f(b.swap())))
                    .monospace()
                    .size(11.0),
            );
        });
    }
}

/// A band of the Sankey: `amount` from class `from` at step `pair` to class `to` at the next step.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub pair: usize,
    pub from: usize,
    pub to: usize,
    pub amount: f64,
}

/// The classes, each step's amount of each (the nodes' heights: a class absent at a step has zero) and the bands from
/// each step to the next, of the matrices of consecutive pairs of steps.
#[derive(Debug, Clone, PartialEq)]
pub struct Layers {
    pub classes: Vec<(i64, String)>,
    /// By step, then class.
    pub heights: Vec<Vec<f64>>,
    pub links: Vec<Link>,
}

/// The nodes and bands of the Sankey of `ms` (the matrix of each pair of steps: one more step than matrices), in the
/// amounts of the area when `area` and the matrices have it, else cells. Every class of any step has a row at every
/// step, in the order of its value; a node is as tall as the larger of what flows in and out of it.
pub fn layers(ms: &[&Matrix], area: bool) -> Layers {
    let mut classes: Vec<(i64, String)> = Vec::new();
    for m in ms {
        for (v, n) in m.from.iter().chain(m.to.iter()) {
            if !classes.iter().any(|c| c.0 == *v) {
                classes.push((*v, n.clone()));
            }
        }
    }
    classes.sort_by_key(|c| c.0);
    let at = |v: i64| classes.iter().position(|c| c.0 == v).unwrap_or(0);
    let mut heights = vec![vec![0.0; classes.len()]; ms.len() + 1];
    let mut links = Vec::new();
    for (p, m) in ms.iter().enumerate() {
        let a = m.amounts(area);
        for (i, row) in a.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                if *v <= 0.0 {
                    continue;
                }
                let (f, t) = (at(m.from[i].0), at(m.to[j].0));
                links.push(Link { pair: p, from: f, to: t, amount: *v });
            }
        }
    }
    // (a node: what flows out of it or into it, the larger)
    for (k, col) in heights.iter_mut().enumerate() {
        for (c, h) in col.iter_mut().enumerate() {
            let out: f64 = links.iter().filter(|l| l.pair == k && l.from == c).map(|l| l.amount).sum();
            let inn: f64 =
                if k > 0 { links.iter().filter(|l| l.pair + 1 == k && l.to == c).map(|l| l.amount).sum() } else { 0.0 };
            *h = out.max(inn);
        }
    }
    Layers { classes, heights, links }
}

/// A band as drawn: its link and the top of its band at each side, and its thickness.
struct Flow {
    link: usize,
    y0: f32,
    y1: f32,
    h: f32,
}

/// The ease of a flow's band from one side to the other.
fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// The flows from the classes before (left) to the classes after (right), each as thick as its amount, in the colour
/// of the class it came from; a flow hovered tells its amount, clicked singles it out (with `focus`).
pub fn sankey(
    ui: &mut Ui,
    m: &Matrix,
    unit: Unit,
    colour: &dyn Fn(i64) -> Color32,
    focus: Option<&mut Option<(i64, i64)>>,
) {
    sankey_chain(ui, &[m], &[], unit, colour, focus);
}

/// The flows over a series of steps: a column of classes per step (`ms`: the matrix of each pair of steps, `steps`
/// their names, written under the columns when there is one each), the bands between consecutive columns.
pub fn sankey_chain(
    ui: &mut Ui,
    ms: &[&Matrix],
    steps: &[String],
    unit: Unit,
    colour: &dyn Fn(i64) -> Color32,
    focus: Option<&mut Option<(i64, i64)>>,
) {
    let area = ms.iter().all(|m| m.area.is_some()) && unit != Unit::Cells;
    let lay = layers(ms, area);
    let n = lay.heights.len();
    let totals: Vec<f64> = lay.heights.iter().map(|h| h.iter().sum()).collect();
    let whole = totals.iter().copied().fold(0.0, f64::max);
    let size = Vec2::new(ui.available_width().max(240.0), ui.available_height().max(220.0));
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    if whole <= 0.0 {
        return;
    }
    let p = ui.painter_at(r);
    let label_w = (r.width() * 0.22).clamp(60.0, 160.0);
    let (xl, xr, nw) = (r.left() + label_w, r.right() - label_w, 10.0);
    let gap = 6.0;
    // (the left side of each column)
    let xs: Vec<f32> = (0..n).map(|k| xl + (xr - xl) * k as f32 / (n - 1).max(1) as f32).collect();
    let foot = if steps.len() == n { 14.0 } else { 0.0 };
    let most = lay.heights.iter().map(|h| h.iter().filter(|v| **v > 0.0).count()).max().unwrap_or(1).max(1);
    let room = (r.height() - 8.0 - foot - gap * (most - 1) as f32).max(20.0);
    let scale = room / whole as f32;
    // the nodes: their tops
    let tops: Vec<Vec<f32>> = lay
        .heights
        .iter()
        .map(|sums| {
            let mut y = r.top() + 4.0;
            sums.iter()
                .map(|s| {
                    let top = y;
                    if *s > 0.0 {
                        y += *s as f32 * scale + gap;
                    }
                    top
                })
                .collect()
        })
        .collect();
    let mut out = tops.clone();
    let mut inn = tops.clone();
    let mut flows = Vec::new();
    for (k, l) in lay.links.iter().enumerate() {
        let h = l.amount as f32 * scale;
        flows.push(Flow { link: k, y0: out[l.pair][l.from], y1: inn[l.pair + 1][l.to], h });
        out[l.pair][l.from] += h;
        inn[l.pair + 1][l.to] += h;
    }
    // the flow under the pointer
    let hovered = resp.hover_pos().and_then(|pos| {
        let pair = (0..n - 1).find(|k| pos.x > xs[*k] + nw && pos.x < xs[k + 1])?;
        let t = ease((pos.x - xs[pair] - nw) / (xs[pair + 1] - xs[pair] - nw));
        flows.iter().position(|f| {
            let top = f.y0 + (f.y1 - f.y0) * t;
            lay.links[f.link].pair == pair && pos.y >= top && pos.y <= top + f.h.max(1.0)
        })
    });
    let chosen = focus.as_deref().copied().flatten();
    for (k, f) in flows.iter().enumerate() {
        let l = &lay.links[f.link];
        let (pv, qv) = (lay.classes[l.from].0, lay.classes[l.to].0);
        let is = chosen == Some((pv, qv)) || hovered == Some(k);
        let dim = chosen.is_some() && !is;
        let c = crate::palette::seen(colour(pv));
        let c = if is {
            c.gamma_multiply(0.9)
        } else if dim {
            c.gamma_multiply(0.15)
        } else {
            c.gamma_multiply(0.45)
        };
        let (x0, x1) = (xs[l.pair] + nw, xs[l.pair + 1]);
        let steps = 24;
        let mut mesh = egui::Mesh::default();
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let y = f.y0 + (f.y1 - f.y0) * ease(t);
            mesh.colored_vertex(Pos2::new(x, y), c);
            mesh.colored_vertex(Pos2::new(x, y + f.h.max(0.5)), c);
            if s > 0 {
                let b = 2 * s as u32;
                mesh.add_triangle(b - 2, b - 1, b);
                mesh.add_triangle(b - 1, b + 1, b);
            }
        }
        p.add(egui::Shape::mesh(mesh));
    }
    // the nodes and their names (at the two ends; the steps' under their columns)
    let font = egui::FontId::proportional(11.0);
    let text = ui.visuals().text_color();
    for (k, col) in lay.heights.iter().enumerate() {
        for (c, (v, name)) in lay.classes.iter().enumerate() {
            if col[c] <= 0.0 {
                continue;
            }
            let h = col[c] as f32 * scale;
            p.rect_filled(
                Rect::from_min_size(Pos2::new(xs[k], tops[k][c]), Vec2::new(nw, h.max(1.0))),
                1.0,
                crate::palette::seen(colour(*v)),
            );
            let said = format!("{name} {}", number(in_unit(col[c], unit, whole), unit));
            if k == 0 {
                p.text(
                    Pos2::new(xs[k] - 4.0, tops[k][c] + h / 2.0),
                    egui::Align2::RIGHT_CENTER,
                    said,
                    font.clone(),
                    text,
                );
            } else if k == n - 1 {
                p.text(
                    Pos2::new(xs[k] + nw + 4.0, tops[k][c] + h / 2.0),
                    egui::Align2::LEFT_CENTER,
                    said,
                    font.clone(),
                    text,
                );
            }
        }
        if foot > 0.0 {
            p.text(
                Pos2::new(xs[k] + nw / 2.0, r.bottom()),
                egui::Align2::CENTER_BOTTOM,
                &steps[k],
                font.clone(),
                ui.visuals().weak_text_color(),
            );
        }
    }
    if let Some(k) = hovered {
        let l = &lay.links[flows[k].link];
        let (from, to) = (&lay.classes[l.from].1, &lay.classes[l.to].1);
        let when =
            if steps.len() == n { format!("{} to {}: ", steps[l.pair], steps[l.pair + 1]) } else { String::new() };
        let said = format!("{when}{from} -> {to}: {} {}", number(in_unit(l.amount, unit, whole), unit), unit.short());
        let resp = resp.clone().on_hover_text(said);
        if resp.clicked()
            && let Some(focus) = focus
        {
            let pair = (lay.classes[l.from].0, lay.classes[l.to].0);
            *focus = if *focus == Some(pair) { None } else { Some(pair) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix() -> Matrix {
        Matrix {
            from: vec![(1, "forest".into()), (2, "pasture".into())],
            to: vec![(1, "forest".into()), (2, "pasture".into())],
            cells: vec![vec![6.0, 2.0], vec![1.0, 1.0]],
            area: Some(vec![vec![60_000.0, 20_000.0], vec![10_000.0, 10_000.0]]),
        }
    }

    #[test]
    fn amounts_in_each_unit() {
        let m = matrix();
        assert_eq!(cell(&m, 0, 1, Unit::Hectares), 2.0);
        assert_eq!(cell(&m, 0, 1, Unit::SquareKm), 0.02);
        assert_eq!(cell(&m, 0, 1, Unit::Landscape), 20.0);
        assert_eq!(cell(&m, 0, 1, Unit::Row), 25.0);
        assert_eq!(cell(&m, 0, 1, Unit::Cells), 2.0);
        // no coordinate system: an area asked for is told in cells
        let plain = Matrix { area: None, ..matrix() };
        assert_eq!(cell(&plain, 0, 1, Unit::Hectares), 2.0);
        assert_eq!(cell(&plain, 1, 0, Unit::Landscape), 10.0);
    }

    /// The second pair of a series: the forest and the pasture of the first pair's second map, urban land appearing.
    fn next() -> Matrix {
        Matrix {
            from: vec![(1, "forest".into()), (2, "pasture".into())],
            to: vec![(1, "forest".into()), (2, "pasture".into()), (3, "urban".into())],
            cells: vec![vec![5.0, 1.0, 1.0], vec![1.0, 2.0, 0.0]],
            area: Some(vec![vec![50_000.0, 10_000.0, 10_000.0], vec![10_000.0, 20_000.0, 0.0]]),
        }
    }

    #[test]
    fn two_steps_are_the_flows_of_their_matrix() {
        let m = matrix();
        let l = layers(&[&m], true);
        assert_eq!(l.classes, m.from);
        assert_eq!(l.heights, vec![vec![80_000.0, 20_000.0], vec![70_000.0, 30_000.0]]);
        let flows: Vec<(usize, usize, f64)> = l.links.iter().map(|k| (k.from, k.to, k.amount)).collect();
        assert_eq!(
            flows,
            vec![(0, 0, 60_000.0), (0, 1, 20_000.0), (1, 0, 10_000.0), (1, 1, 10_000.0)],
            "a flow for each cell of the matrix with something in it"
        );
        // (in cells when asked)
        assert_eq!(layers(&[&m], false).heights[0], vec![8.0, 2.0]);
    }

    #[test]
    fn three_steps_add_up_column_by_column() {
        let (a, b) = (matrix(), next());
        let l = layers(&[&a, &b], true);
        assert_eq!(l.heights.len(), 3);
        for col in &l.heights {
            assert_eq!(col.iter().sum::<f64>(), 100_000.0, "the landscape at every step");
        }
        assert_eq!(l.heights[1], vec![70_000.0, 30_000.0, 0.0]);
        assert_eq!(l.heights[2], vec![60_000.0, 30_000.0, 10_000.0]);
        // each pair's flows are what leaves one column and what enters the next
        for p in 0..2 {
            let of = |f: &dyn Fn(&Link) -> usize, c: usize| -> f64 {
                l.links.iter().filter(|k| k.pair == p && f(k) == c).map(|k| k.amount).sum()
            };
            for c in 0..l.classes.len() {
                assert_eq!(of(&|k| k.from, c), l.heights[p][c], "pair {p}, out of class {c}");
                assert_eq!(of(&|k| k.to, c), l.heights[p + 1][c], "pair {p}, into class {c}");
            }
        }
    }

    #[test]
    fn a_class_absent_at_a_step_keeps_its_row() {
        let (a, b) = (matrix(), next());
        let l = layers(&[&a, &b], true);
        assert_eq!(l.classes.iter().map(|c| c.0).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(l.classes[2].1, "urban");
        assert_eq!((l.heights[0][2], l.heights[1][2]), (0.0, 0.0), "no urban land before the last step, a row still");
        assert!(l.links.iter().all(|k| k.from != 2), "nothing flows out of it");
    }

    #[test]
    fn the_flows_of_every_step_as_text() {
        let (a, b) = (matrix(), next());
        let names: Vec<String> = ["1997", "2005", "2011"].iter().map(|s| s.to_string()).collect();
        let t = chain_tsv(&[&a, &b], &names, Unit::Hectares);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "from_step\tto_step\tclass (ha)\tforest\tpasture\ttotal");
        assert_eq!(lines[1], "1997\t2005\tforest\t6.00\t2.00\t8.00");
        assert_eq!(lines[3], "");
        assert_eq!(lines[4], "from_step\tto_step\tclass (ha)\tforest\tpasture\turban\ttotal");
        assert_eq!(lines[5], "2005\t2011\tforest\t5.00\t1.00\t1.00\t7.00");
    }

    #[test]
    fn every_step_by_the_number_of_steps() {
        let mut s = State::default();
        assert!(!every(&s, 2) && every(&s, 3) && every(&s, 8) && !every(&s, 9), "by default");
        s.every = Some(true);
        assert!(every(&s, 9) && every(&s, MAX_CHAIN) && !every(&s, MAX_CHAIN + 1) && !every(&s, 2));
        s.every = Some(false);
        assert!(!every(&s, 4));
    }

    #[test]
    fn the_matrix_as_text() {
        let t = tsv(&matrix(), Unit::Hectares, "2000", "2020");
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "2000 \\ 2020 (ha)\tforest\tpasture\ttotal");
        assert_eq!(lines[1], "forest\t6.00\t2.00\t8.00");
        assert_eq!(lines[2], "pasture\t1.00\t1.00\t2.00");
        assert_eq!(lines[4], "class\tbefore\tafter\tgain\tloss\tnet\tswap (ha)");
        assert_eq!(lines[5], "forest\t8.00\t7.00\t1.00\t2.00\t-1.00\t2.00");
        assert_eq!(lines[6], "pasture\t2.00\t3.00\t2.00\t1.00\t1.00\t2.00");
    }
}
