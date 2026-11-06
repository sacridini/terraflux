//! Several models open, in tabs: the one in front is [`TerrafluxApp::session`] (the rest of the application works
//! on it as before), the others parked in [`TerrafluxApp::others`] with what the panes keep of each (the Trace, the
//! Parameters, the Calculator, the Assistant's conversation), and swapped when a tab comes to the front. The
//! models behind go on running; a model opened goes to a tab of its own (the one in front, empty and untouched, is
//! replaced), one open already to its tab. Closing a tab, or Terraflux, asks about each model with edits not saved.

use std::path::Path;

use egui::{RichText, Ui};

use crate::app::{Session, TerrafluxApp, Then};

/// A model behind, with what the panes keep of it.
pub struct Parked {
    pub session: Session,
    pub trace: crate::trace_view::State,
    pub params: crate::params_view::State,
    pub chat: crate::chat::Chat,
    pub calculator: crate::calc_view::Calculator,
}

impl Parked {
    fn new(session: Session) -> Parked {
        Parked {
            session,
            trace: Default::default(),
            params: Default::default(),
            chat: Default::default(),
            calculator: Default::default(),
        }
    }
}

/// What closing a tab does (ctrl+shift+W; the middle button on it).
pub const CLOSE_HELP: &str = "close this model (ctrl+shift+W); its edits not saved asked about first";

impl TerrafluxApp {
    /// The models open (the tabs).
    pub fn tab_count(&self) -> usize {
        self.others.len() + self.session.is_some() as usize
    }

    /// The model of tab `k` (in the order of the tabs).
    pub fn tab(&self, k: usize) -> Option<&Session> {
        let front = self.front.min(self.others.len());
        match k.cmp(&front) {
            std::cmp::Ordering::Less => self.others.get(k).map(|p| &p.session),
            std::cmp::Ordering::Equal => self.session.as_ref(),
            std::cmp::Ordering::Greater => self.others.get(k - 1).map(|p| &p.session),
        }
    }

    /// Every model open, the one in front first.
    pub fn sessions_mut(&mut self) -> Vec<&mut Session> {
        self.session.iter_mut().chain(self.others.iter_mut().map(|p| &mut p.session)).collect()
    }

    /// The model in front parked (in its place among the tabs), the panes' states with it.
    fn park(&mut self) -> Option<Parked> {
        let session = self.session.take()?;
        Some(Parked {
            session,
            trace: std::mem::take(&mut self.trace),
            params: std::mem::take(&mut self.params),
            chat: std::mem::take(&mut self.chat),
            calculator: std::mem::take(&mut self.calculator),
        })
    }

    /// A model parked brought to the front, the panes' states with it.
    fn unpark(&mut self, p: Parked) {
        self.session = Some(p.session);
        self.trace = p.trace;
        self.params = p.params;
        self.chat = p.chat;
        self.calculator = p.calculator;
    }

    /// Tab `k` to the front.
    pub fn switch_to(&mut self, k: usize) {
        if k >= self.tab_count() || k == self.front {
            return;
        }
        if let Some(p) = self.park() {
            self.others.insert(self.front.min(self.others.len()), p);
        }
        let p = self.others.remove(k);
        self.unpark(p);
        self.front = k;
        self.message = None;
    }

    /// The tab after the one in front (`by` 1) or before it (-1), round.
    pub fn next_tab(&mut self, by: isize) {
        let n = self.tab_count() as isize;
        if n > 1 {
            self.switch_to((self.front as isize + by).rem_euclid(n) as usize);
        }
    }

    /// The tab of a model's file, when it is open.
    pub fn tab_of(&self, path: &Path) -> Option<usize> {
        let wanted = std::fs::canonicalize(path).ok()?;
        (0..self.tab_count()).find(|&k| {
            self.tab(k).and_then(|s| s.path.as_ref()).and_then(|p| std::fs::canonicalize(p).ok())
                == Some(wanted.clone())
        })
    }

    /// A model put in front: in a new tab after the one in front, unless that one is an empty model nothing was done
    /// to (replaced then).
    pub fn put_in_front(&mut self, s: Session) {
        let replace = self
            .session
            .as_ref()
            .is_none_or(|f| f.path.is_none() && !f.worth_keeping() && f.source.nodes.is_empty() && !f.running());
        if !replace && let Some(p) = self.park() {
            let at = self.front.min(self.others.len());
            self.others.insert(at, p);
            self.front = at + 1;
            self.unpark(Parked::new(s));
        } else {
            self.session = Some(s);
        }
    }

    /// Close tab `k`: its edits not saved asked about first (it comes to the front then).
    pub fn close_tab(&mut self, k: usize) {
        self.switch_to(k);
        if self.may_leave(Then::Close) {
            self.drop_front();
        }
    }

    /// The model in front closed: the tab beside it to the front (none left: no model).
    pub(crate) fn drop_front(&mut self) {
        if let (Some(k), Some(s)) = (&self.recovery, &mut self.session) {
            k.forget(s);
        }
        // (its view kept, for when it is opened again)
        if let (Some(st), Some(s)) = (self.restore.clone(), &mut self.session)
            && let Some(v) = crate::restore::view(s, None)
        {
            st.keep_views(vec![v]);
        }
        self.session = None;
        if self.others.is_empty() {
            self.front = 0;
            self.trace = Default::default();
            self.params = Default::default();
            self.chat = Default::default();
            self.calculator = Default::default();
        } else {
            let k = self.front.min(self.others.len() - 1);
            let p = self.others.remove(k);
            self.unpark(p);
            self.front = k;
        }
    }

    /// Quitting: the first model with edits worth keeping brought to the front and asked about (false), else true.
    pub(crate) fn may_quit(&mut self) -> bool {
        if !self.may_leave(Then::Quit) {
            return false;
        }
        match (0..self.tab_count()).find(|&k| self.tab(k).is_some_and(Session::worth_keeping)) {
            Some(k) => {
                self.switch_to(k);
                self.may_leave(Then::Quit)
            }
            None => true,
        }
    }

    /// The models behind: their runs, their files followed, their assistants; a run that ended told as the one in
    /// front's is (`ended` taken here).
    pub(crate) fn poll_behind(&mut self, ctx: &egui::Context) -> Vec<(String, crate::app::Ended)> {
        let mut ended = Vec::new();
        for p in &mut self.others {
            let s = &mut p.session;
            s.poll();
            crate::watch::follow(s, ctx);
            s.auto_run(ctx);
            p.chat.poll(s, &self.registry, ctx);
            crate::review::follow(s);
            if s.running() || s.loading() || s.saving() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            if let Some(e) = s.ended.take() {
                ended.push((s.title(), e));
            }
        }
        ended
    }

    /// The keys of the tabs: ctrl+Tab and ctrl+shift+Tab the next and the one before, ctrl+shift+W closes the one
    /// in front.
    pub(crate) fn tab_keys(&mut self, ui: &mut Ui) {
        if self.tab_count() == 0 {
            return;
        }
        let hit = crate::keymap::take(ui.ctx(), crate::keymap::TABS);
        let (back, next, close) = (hit.contains(&"prev_tab"), hit.contains(&"next_tab"), hit.contains(&"close_tab"));
        if back {
            self.next_tab(-1);
        } else if next {
            self.next_tab(1);
        }
        if close {
            self.close_tab(self.front);
        }
    }

    /// The strip of the tabs, under the toolbar, when two models or more are open: each its name, a dot when it has
    /// edits not saved, a spinner while it runs, and its × (the middle button closes it too).
    /// The colour of the model of tab `k` (by its place among the tabs): the tabs' and the toolbar's.
    pub fn tab_colour(&self, k: usize) -> egui::Color32 {
        tint(k)
    }

    pub(crate) fn tabs_strip(&mut self, ui: &mut Ui) {
        let mut go = None;
        let mut close = None;
        let n = self.tab_count();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(RichText::new(format!("{n} models open")).weak().size(12.0)).on_hover_text(format!(
                "each in its colour; {} and {} go from one to the next",
                crate::keymap::text("next_tab"),
                crate::keymap::text("prev_tab")
            ));
            for k in 0..n {
                let Some(s) = self.tab(k) else { continue };
                let front = k == self.front;
                let mut text = s.title();
                if s.dirty {
                    text.push_str(" ●");
                }
                let file = s.path.as_ref().map_or("not saved yet".to_string(), |p| p.display().to_string());
                let running = s.running();
                // each model its colour: the tab washed in it (the one in front more), a band of it at the left
                let colour = tint(k);
                let fill = ui.visuals().panel_fill.lerp_to_gamma(colour, if front { 0.42 } else { 0.16 });
                let frame = egui::Frame::new()
                    .fill(fill)
                    .stroke(if front {
                        egui::Stroke::new(1.5, colour)
                    } else {
                        egui::Stroke::new(1.0, colour.gamma_multiply(0.45))
                    })
                    .corner_radius(crate::theme::rounded(4.0))
                    .inner_margin(egui::Margin { left: 14, right: 10, top: 4, bottom: 4 })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if running {
                                ui.add(egui::Spinner::new().size(11.0)).on_hover_text("running");
                            }
                            let label = RichText::new(&text).size(13.5);
                            let label =
                                if front { label.strong().color(ui.visuals().strong_text_color()) } else { label };
                            let r = ui.add(egui::Button::new(label).frame(false)).on_hover_text(file);
                            if r.clicked() {
                                go = Some(k);
                            }
                            if r.middle_clicked() {
                                close = Some(k);
                            }
                            if ui.add(egui::Button::new("×").frame(false).small()).on_hover_text(CLOSE_HELP).clicked()
                            {
                                close = Some(k);
                            }
                        });
                    });
                let r = frame.response.rect;
                let band = egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + 5.0, r.max.y));
                let radius = crate::theme::rounded(4.0);
                ui.painter().rect_filled(
                    band,
                    egui::CornerRadius { nw: radius as u8, sw: radius as u8, ne: 0, se: 0 },
                    colour,
                );
            }
        });
        if let Some(k) = close {
            self.close_tab(k);
        } else if let Some(k) = go {
            self.switch_to(k);
        }
    }
}

/// The colours of the models open, by their tab: told apart at a glance, on a dark or a light theme.
const TINTS: [egui::Color32; 8] = [
    egui::Color32::from_rgb(86, 156, 214),
    egui::Color32::from_rgb(222, 140, 60),
    egui::Color32::from_rgb(98, 184, 112),
    egui::Color32::from_rgb(196, 104, 186),
    egui::Color32::from_rgb(64, 184, 184),
    egui::Color32::from_rgb(214, 88, 88),
    egui::Color32::from_rgb(184, 164, 76),
    egui::Color32::from_rgb(140, 124, 226),
];

/// The colour of the model of tab `k`.
pub fn tint(k: usize) -> egui::Color32 {
    TINTS[k % TINTS.len()]
}
