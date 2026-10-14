//! Computers that work together, without knowing what a coordinator is: this computer's batches run on its own
//! pool (a coordinator of the application's, with workers of this computer), which other computers may join
//! when it is shared, given its code; and this computer may be lent to another's, by typing the other's code.
//! Everything goes on the local network (the coordinator found by its beacons), encrypted, and only computers
//! given the code take part.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{RichText, Ui};
use terraflux_dist::discover::hostname;
use terraflux_dist::pool::{Lender, Pool};
use terraflux_dist::proto::WorkerState;
use terraflux_dist::secure::Code;

use crate::app::accent_button;
use crate::theme;

/// The cache of the data the workers fetch.
pub fn cache() -> PathBuf {
    std::env::temp_dir().join("terraflux-cache")
}

/// The pool's folder (its jobs, its data, its certificate).
fn pool_dir() -> PathBuf {
    crate::settings::data_dir().join("pool")
}

/// The pool's code, made once and kept in the settings.
fn pool_code() -> Code {
    if let Some(c) = crate::settings::get().share_code.as_deref().and_then(|c| Code::parse(c).ok()) {
        return c;
    }
    let c = Code::generate();
    crate::settings::update(|s| s.share_code = Some(c.to_string()));
    c
}

/// Whether the application starts sharing and lending as its settings say (tests start them by hand, on
/// this computer only).
pub static AUTOSTART: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// The pool and the lending of this computer.
#[derive(Default)]
pub struct Computers {
    pub pool: Option<Pool>,
    /// The pool's folder, when not the user's (tests).
    pub dir: Option<PathBuf>,
    /// Where a shared pool listens and the port of the beacons, when not every network and 7701 (tests keep
    /// to this computer).
    pub network: Option<(std::net::IpAddr, u16)>,
    pub lender: Option<Lender>,
    pub error: Option<String>,
    /// The code being typed to lend this computer.
    typed: String,
    /// The workers of the pool, asked again every second.
    seen: Option<(Instant, Vec<WorkerState>)>,
}

impl Computers {
    /// As the settings say: the pool started when shared (others may be waiting to join it), the computer lent.
    pub fn new() -> Computers {
        let st = crate::settings::get();
        let mut c = Computers { typed: st.lend_code.clone(), ..Default::default() };
        if !AUTOSTART.load(std::sync::atomic::Ordering::Relaxed) {
            return c;
        }
        if st.share
            && let Err(e) = c.pool(0)
        {
            c.error = Some(e);
        }
        if st.lend {
            c.lend();
        }
        c
    }

    /// The pool (started if it is not), with `workers` workers of this computer for now when more are asked.
    pub fn pool(&mut self, workers: usize) -> Result<&mut Pool, String> {
        if self.pool.is_none() {
            let shared = crate::settings::get().share;
            let dir = self.dir.clone().unwrap_or_else(pool_dir);
            let p = match self.network {
                None => Pool::start(dir, cache(), pool_code(), shared, workers).map_err(|e| e.to_string())?,
                Some((ip, port)) => {
                    let mut p = Pool::start(dir, cache(), pool_code(), false, workers)
                        .and_then(|p| p.with_network(ip, 0, port))
                        .map_err(|e| e.to_string())?;
                    p.set_shared(shared).map_err(|e| e.to_string())?;
                    p
                }
            };
            self.pool = Some(p);
        }
        let p = self.pool.as_mut().unwrap();
        if workers > 0 {
            p.set_workers(workers);
        }
        Ok(p)
    }

    /// Lend this computer to the code typed (in the settings).
    pub fn lend(&mut self) {
        let st = crate::settings::get();
        self.lender = None;
        match Code::parse(&st.lend_code) {
            Ok(code) => {
                let threads = st.lend_threads.unwrap_or_else(|| (terraflux_ops::threads::most() / 2).max(1));
                let idle = st.lend_idle.map(|m| Duration::from_secs_f64(m * 60.0));
                let on = match self.network {
                    Some((ip, port)) => std::net::SocketAddr::new(ip, port),
                    None => terraflux_dist::discover::everywhere(),
                };
                match Lender::start(code, cache(), threads, idle, on) {
                    Ok(l) => {
                        self.lender = Some(l);
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// The workers of the pool now (asked at most every second).
    pub fn workers(&mut self) -> Vec<WorkerState> {
        let Some(p) = &self.pool else { return Vec::new() };
        if self.seen.as_ref().is_none_or(|(t, _)| t.elapsed() > Duration::from_secs(1)) {
            self.seen = Some((Instant::now(), p.workers()));
        }
        self.seen.as_ref().map(|(_, w)| w.clone()).unwrap_or_default()
    }

    /// The other computers working for the pool now.
    pub fn helpers(&mut self) -> Vec<WorkerState> {
        let me = format!("{} ", hostname());
        self.workers().into_iter().filter(|w| !w.name.starts_with(&me) || w.name.len() <= me.len()).collect()
    }
}

/// The pane of the computers.
pub fn pane(ui: &mut Ui, c: &mut Computers) {
    egui::ScrollArea::vertical().id_salt("computers pane").show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            share_ui(ui, c);
            ui.add_space(12.0);
            lend_ui(ui, c);
            if let Some(e) = &c.error {
                ui.add_space(6.0);
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
        });
    });
}

fn share_ui(ui: &mut Ui, c: &mut Computers) {
    theme::section(ui, "Other computers help this one");
    ui.label(
        RichText::new(
            "The runs of \"Many runs\" also go to the computers of this network given this computer's code. Each \
             gets the model and its data once; the results come back here.",
        )
        .weak(),
    );
    let mut share = crate::settings::get().share;
    if ui.checkbox(&mut share, "Let other computers help").changed() {
        crate::settings::update(|s| s.share = share);
        let workers = c.pool.as_ref().map_or(0, |p| p.local_workers());
        match c.pool(workers) {
            Ok(p) => {
                if let Err(e) = p.set_shared(share) {
                    c.error = Some(e.to_string());
                }
            }
            Err(e) => c.error = Some(e),
        }
    }
    if !share {
        return;
    }
    let code = pool_code();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("This computer's code:");
        ui.label(RichText::new(code.to_string()).monospace().size(22.0).strong().color(theme::accent()));
        if ui.small_button("Copy").clicked() {
            ui.ctx().copy_text(code.to_string());
        }
        if ui.small_button("New code").on_hover_text("the computers given the old code stop helping").clicked() {
            let new = Code::generate();
            crate::settings::update(|s| s.share_code = Some(new.to_string()));
            if let Some(p) = &mut c.pool
                && let Err(e) = p.set_code(new)
            {
                c.error = Some(e.to_string());
            }
        }
    });
    ui.label(
        RichText::new(format!(
            "On each computer that helps: open Terraflux, Computers, \"Lend this computer\", and type {code}. Or, \
             without the window: terraflux worker --code {code}"
        ))
        .weak()
        .size(11.0),
    );
    if let Some(p) = &c.pool {
        ui.label(RichText::new(format!("listening on the port {}", p.port())).weak().size(11.0));
    }
    let helpers = c.helpers();
    ui.add_space(4.0);
    if helpers.is_empty() {
        ui.label(RichText::new("No other computer yet.").weak());
    } else {
        egui::Grid::new("helpers").striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
            for h in ["computer", "threads", "runs done", ""] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for w in &helpers {
                ui.label(&w.name);
                ui.label(w.threads.to_string());
                ui.label(w.done.to_string());
                ui.label(RichText::new(if w.task.is_empty() { "waiting" } else { "running" }).weak());
                ui.end_row();
            }
        });
    }
}

fn lend_ui(ui: &mut Ui, c: &mut Computers) {
    theme::section(ui, "This computer helps another");
    ui.label(
        RichText::new("Type the code another computer shows (its Computers pane): this one runs some of its runs.")
            .weak(),
    );
    let mut st = crate::settings::get();
    let before = st.clone();
    let mut now = false;
    ui.horizontal(|ui| {
        let label = ui.label("Code");
        let resp = ui
            .add(egui::TextEdit::singleline(&mut c.typed).hint_text("K7QM-2XPA-9DTE").desired_width(160.0))
            .labelled_by(label.id);
        let valid = Code::parse(&c.typed).is_ok();
        if !c.typed.is_empty() && !valid && !resp.has_focus() {
            ui.label(RichText::new("12 letters and digits").color(ui.visuals().warn_fg_color));
        }
        let lending = c.lender.is_some();
        let label = if lending { "Stop lending" } else { "Lend this computer" };
        if ui.add_enabled(valid || lending, accent_button(label, valid && !lending)).clicked() {
            if lending {
                c.lender = None;
                st.lend = false;
            } else {
                st.lend = true;
                st.lend_code = c.typed.trim().to_string();
                now = true;
            }
        }
    });
    let most = terraflux_ops::threads::most();
    ui.horizontal(|ui| {
        ui.label("Threads it uses");
        let mut n = st.lend_threads.unwrap_or((most / 2).max(1)).clamp(1, most);
        if ui.add(egui::Slider::new(&mut n, 1..=most)).changed() {
            st.lend_threads = Some(n);
        }
    });
    let knows = terraflux_dist::idle::seconds().is_some();
    ui.add_enabled_ui(knows, |ui| {
        ui.horizontal(|ui| {
            let mut away = st.lend_idle.is_some();
            ui.checkbox(&mut away, "Only when nobody uses this computer for")
                .on_disabled_hover_text("this system does not tell when its user is away");
            let mut m = st.lend_idle.unwrap_or(10.0);
            ui.add_enabled(away, egui::DragValue::new(&mut m).range(1.0..=240.0).suffix(" min"));
            st.lend_idle = away.then_some(m);
        });
    });
    let restart = now
        || (st.lend
            && (st.lend_code != before.lend_code
                || st.lend_threads != before.lend_threads
                || st.lend_idle != before.lend_idle));
    if st != before {
        crate::settings::update(|s| *s = st.clone());
    }
    if restart {
        c.lend();
    }
    if let Some(l) = &c.lender {
        let s = l.status();
        let paused = l.paused.load(std::sync::atomic::Ordering::Relaxed);
        ui.horizontal(|ui| {
            let text = match (&s.coordinator, &s.note) {
                (_, Some(n)) => n.clone(),
                (Some(at), None) => format!(
                    "working for the computer at {at}: {} runs done{}",
                    s.done,
                    if s.task.is_some() { ", one running" } else { "" }
                ),
                (None, None) => "starting".into(),
            };
            ui.label(RichText::new(text).color(theme::accent()));
            if ui.button(if paused { "Go on" } else { "Pause" }).clicked() {
                l.paused.store(!paused, std::sync::atomic::Ordering::Relaxed);
            }
        });
    }
}
