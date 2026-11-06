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
        let shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        let (back, next, close) = ui.input_mut(|i| {
            let back = i.consume_key(shift, egui::Key::Tab);
            let next = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Tab);
            (back, next, i.consume_key(shift, egui::Key::W))
        });
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
    pub(crate) fn tabs_strip(&mut self, ui: &mut Ui) {
        let mut go = None;
        let mut close = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for k in 0..self.tab_count() {
                let Some(s) = self.tab(k) else { continue };
                let front = k == self.front;
                let mut text = s.title();
                if s.dirty {
                    text.push_str(" ●");
                }
                let file = s.path.as_ref().map_or("not saved yet".to_string(), |p| p.display().to_string());
                let running = s.running();
                egui::Frame::new()
                    .fill(if front { ui.visuals().panel_fill } else { ui.visuals().faint_bg_color })
                    .stroke(if front { egui::Stroke::new(1.0, crate::theme::accent()) } else { egui::Stroke::NONE })
                    .corner_radius(crate::theme::rounded(4.0))
                    .inner_margin(egui::Margin::symmetric(10, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if running {
                                ui.add(egui::Spinner::new().size(11.0)).on_hover_text("running");
                            }
                            let label = RichText::new(&text);
                            let r = ui
                                .add(egui::Button::new(if front { label.strong() } else { label }).frame(false))
                                .on_hover_text(file);
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
            }
        });
        if let Some(k) = close {
            self.close_tab(k);
        } else if let Some(k) = go {
            self.switch_to(k);
        }
    }
}
