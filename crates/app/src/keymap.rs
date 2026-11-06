//! The keyboard shortcuts as a table of actions, each with its default shortcuts and the user's own (kept in the
//! settings, `shortcuts`): what reads the keys asks the table for an action by its id, so that a shortcut changed
//! in Settings (Keyboard) is the one every menu, the palette and the sheet of shortcuts (F1) show and the one that
//! works. The keys of the whole application are in it; the canvas', the Viewer's and the panes' own keys are read
//! where they are used still (their scopes are here for them to join).

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use egui::{Key, Modifiers, RichText, Ui};

/// Where an action works: everywhere (the keys of the application), or over one place (that place takes its key
/// when the pointer is over it, so a shortcut the same as an everywhere one is a warning, not an error).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    Global,
    Canvas,
    Viewer,
    Calculator,
    Files,
}

impl Scope {
    pub const ALL: [Scope; 5] = [Scope::Global, Scope::Canvas, Scope::Viewer, Scope::Calculator, Scope::Files];

    pub fn title(self) -> &'static str {
        match self {
            Scope::Global => "Everywhere",
            Scope::Canvas => "The model's canvas",
            Scope::Viewer => "Maps",
            Scope::Calculator => "The calculator",
            Scope::Files => "Files",
        }
    }
}

/// A key with its modifiers. `cmd` is ctrl on Windows and Linux, the command key on macOS (as egui's
/// `Modifiers::COMMAND`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Shortcut {
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: Key,
}

impl Shortcut {
    pub const fn new(cmd: bool, shift: bool, alt: bool, key: Key) -> Shortcut {
        Shortcut { cmd, shift, alt, key }
    }

    pub fn modifiers(&self) -> Modifiers {
        let mut m = Modifiers::NONE;
        if self.cmd {
            m |= Modifiers::COMMAND;
        }
        if self.shift {
            m |= Modifiers::SHIFT;
        }
        if self.alt {
            m |= Modifiers::ALT;
        }
        m
    }

    /// "Ctrl+Shift+S": how it is kept in the settings (`Ctrl` is the command key on macOS), in any case when read.
    pub fn parse(text: &str) -> Option<Shortcut> {
        let text = text.trim();
        let (mods, key) = match text.strip_suffix('+') {
            // (the key is "+" itself: "Ctrl++")
            Some(m) if m.is_empty() || m.ends_with('+') => (m, "+"),
            _ => text.rsplit_once('+').unwrap_or(("", text)),
        };
        let mut s = Shortcut::new(false, false, false, Key::A);
        for m in mods.split('+').map(str::trim).filter(|m| !m.is_empty()) {
            match m.to_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "command" => s.cmd = true,
                "shift" => s.shift = true,
                "alt" | "option" => s.alt = true,
                _ => return None,
            }
        }
        let key = key.trim();
        s.key = Key::ALL
            .iter()
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(key))
            .or_else(|| Key::from_name(key))
            .filter(|k| !is_modifier(*k))?;
        Some(s)
    }

    /// How the menus and the palette write it: "ctrl+shift+S", on macOS "⌘shift+S".
    pub fn show(&self, mac: bool) -> String {
        let mut s = String::new();
        if self.cmd {
            s += if mac { "⌘" } else { "ctrl+" };
        }
        if self.shift {
            s += "shift+";
        }
        if self.alt {
            s += "alt+";
        }
        s + match self.key {
            // (not the arrows' symbols: the default font lacks them)
            Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp => self.key.name(),
            _ => self.key.symbol_or_name(),
        }
    }

    /// Whether pressing it writes a character (a letter, a digit, a sign) when a text is being edited.
    fn types_text(&self) -> bool {
        !self.cmd
            && !self.alt
            && (self.key.name().chars().count() == 1
                || matches!(
                    self.key,
                    Key::Space
                        | Key::Colon
                        | Key::Comma
                        | Key::Minus
                        | Key::Period
                        | Key::Plus
                        | Key::Equals
                        | Key::Semicolon
                        | Key::Backslash
                        | Key::Slash
                        | Key::Pipe
                        | Key::Questionmark
                        | Key::Exclamationmark
                        | Key::OpenBracket
                        | Key::CloseBracket
                        | Key::Backtick
                        | Key::Quote
                ))
    }
}

impl std::fmt::Display for Shortcut {
    /// "Ctrl+Shift+S".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.cmd {
            f.write_str("Ctrl+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        f.write_str(self.key.name())
    }
}

fn is_modifier(k: Key) -> bool {
    matches!(
        k,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::AltLeft
            | Key::AltRight
            | Key::SuperLeft
            | Key::SuperRight
    )
}

/// One thing the keyboard does.
#[derive(Debug, PartialEq)]
pub struct Action {
    /// What the settings keep it by: never changes.
    pub id: &'static str,
    /// As the palette names it.
    pub name: &'static str,
    pub scope: Scope,
    /// The shortcuts it has until the user changes them (none, one or two).
    pub default: &'static [Shortcut],
    /// Not to be changed: shown in the settings and in F1, read where it is used (editing a text, undo...).
    pub fixed: bool,
    /// Not while a text is being edited (Tab goes to the next field then).
    pub idle: bool,
}

const fn key(k: Key) -> Shortcut {
    Shortcut::new(false, false, false, k)
}
const fn cmd(k: Key) -> Shortcut {
    Shortcut::new(true, false, false, k)
}
const fn cmd_shift(k: Key) -> Shortcut {
    Shortcut::new(true, true, false, k)
}
const fn cmd_alt(k: Key) -> Shortcut {
    Shortcut::new(true, false, true, k)
}

const fn global(id: &'static str, name: &'static str, default: &'static [Shortcut]) -> Action {
    Action { id, name, scope: Scope::Global, default, fixed: false, idle: false }
}

const fn read_elsewhere(id: &'static str, name: &'static str, default: &'static [Shortcut]) -> Action {
    Action { id, name, scope: Scope::Global, default, fixed: true, idle: false }
}

/// Every action, in the order the settings and F1 list them.
pub static ACTIONS: &[Action] = &[
    global("run", "Run the whole model", &[key(Key::F5)]),
    global("save", "Save the model", &[cmd(Key::S)]),
    global("save_as", "Save the model as...", &[cmd_shift(Key::S)]),
    global("version", "Save a version of the model...", &[cmd_alt(Key::S)]),
    global("new", "New model", &[cmd(Key::N)]),
    global("open_model", "Open a model...", &[cmd_shift(Key::O)]),
    global("open_data", "Open a raster or vectors...", &[cmd(Key::O)]),
    global("commands", "The commands: anything, by its name", &[cmd(Key::K), cmd_shift(Key::P)]),
    global("shortcuts", "Keyboard shortcuts", &[key(Key::F1)]),
    Action {
        id: "to_model",
        name: "To the model and to the last map seen",
        scope: Scope::Global,
        default: &[key(Key::Tab)],
        fixed: false,
        idle: true,
    },
    global("close_pane", "Close the pane in front", &[cmd(Key::W)]),
    global("next_tab", "Next model open (its tab)", &[cmd(Key::Tab)]),
    global("prev_tab", "Model open before (its tab)", &[cmd_shift(Key::Tab)]),
    global("close_tab", "Close the model in front (its tab)", &[cmd_shift(Key::W)]),
    // (read where they are used, for now)
    read_elsewhere("undo", "Undo", &[cmd(Key::Z)]),
    read_elsewhere("redo", "Redo", &[cmd_shift(Key::Z)]),
    read_elsewhere("add_operator", "Add an operator, by its name", &[key(Key::Slash)]),
];

/// The actions the application reads in a window, all but the tabs' (read in the main window only).
pub const APP: &[&str] = &[
    "run",
    "save",
    "save_as",
    "version",
    "new",
    "open_model",
    "open_data",
    "commands",
    "shortcuts",
    "to_model",
    "close_pane",
];

/// The actions of the tabs of models.
pub const TABS: &[&str] = &["next_tab", "prev_tab", "close_tab"];

pub fn action(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// The user's changes, parsed (an entry that is not a shortcut is left out).
pub type Overrides = BTreeMap<String, Vec<Shortcut>>;

pub fn parse_map(map: &BTreeMap<String, Vec<String>>) -> Overrides {
    map.iter().map(|(id, l)| (id.clone(), l.iter().filter_map(|s| Shortcut::parse(s)).collect())).collect()
}

/// The shortcuts of an action under the changes `o`.
pub fn bindings(o: &Overrides, a: &Action) -> Vec<Shortcut> {
    match o.get(a.id) {
        Some(l) if !a.fixed => l.clone(),
        _ => a.default.to_vec(),
    }
}

/// The user's changes now, worked out again only when the settings changed.
fn overrides() -> Arc<Overrides> {
    static CACHE: Mutex<Option<(u64, Arc<Overrides>)>> = Mutex::new(None);
    let g = crate::settings::generation();
    let mut c = CACHE.lock().unwrap();
    if let Some((at, o)) = &*c
        && *at == g
    {
        return o.clone();
    }
    let o = Arc::new(parse_map(&crate::settings::get().shortcuts));
    *c = Some((g, o.clone()));
    o
}

/// The shortcuts an action has now.
pub fn current(id: &str) -> Vec<Shortcut> {
    action(id).map_or(Vec::new(), |a| bindings(&overrides(), a))
}

/// The first shortcut of an action as the menus and the palette write it (empty: it has none).
pub fn text(id: &str) -> String {
    current(id).first().map_or(String::new(), |s| s.show(cfg!(target_os = "macos")))
}

/// All its shortcuts, "ctrl+K, ctrl+shift+P" (empty: it has none).
pub fn texts(id: &str) -> String {
    current(id).iter().map(|s| s.show(cfg!(target_os = "macos"))).collect::<Vec<_>>().join(", ")
}

/// The actions of the whole application with their shortcuts, for the sheet of shortcuts (F1).
pub fn everywhere() -> Vec<(String, &'static str)> {
    let o = overrides();
    ACTIONS
        .iter()
        .filter(|a| a.scope == Scope::Global)
        .map(|a| {
            let l = bindings(&o, a);
            let keys = l.iter().map(|s| s.show(cfg!(target_os = "macos"))).collect::<Vec<_>>().join(", ");
            (if keys.is_empty() { "none".to_string() } else { keys }, a.name)
        })
        .collect()
}

/// Take the keys of `ids` pressed in this frame: those pressed. While the settings record a shortcut, none.
pub fn take(ctx: &egui::Context, ids: &[&str]) -> Vec<&'static str> {
    take_with(ctx, &overrides(), ids)
}

fn take_with(ctx: &egui::Context, o: &Overrides, ids: &[&str]) -> Vec<&'static str> {
    if recording() {
        return Vec::new();
    }
    let typing = ctx.memory(|m| m.focused().is_some());
    let mut all: Vec<(&'static Action, Shortcut)> = ACTIONS
        .iter()
        .filter(|a| !a.fixed && ids.contains(&a.id))
        .flat_map(|a| bindings(o, a).into_iter().map(move |s| (a, s)))
        .filter(|(a, s)| !typing || !(a.idle || s.types_text()))
        .collect();
    // (the most specific first: a shortcut without shift or alt takes the key with them too)
    all.sort_by_key(|(_, s)| Reverse(s.shift as u8 + s.alt as u8));
    let mut hit = Vec::new();
    ctx.input_mut(|i| {
        for (a, s) in all {
            if i.consume_key(s.modifiers(), s.key) && !hit.contains(&a.id) {
                hit.push(a.id);
            }
        }
    });
    hit
}

/// Whether the settings are recording a shortcut: the keys of the application wait.
static RECORDING: AtomicBool = AtomicBool::new(false);

fn recording() -> bool {
    RECORDING.load(Ordering::Relaxed)
}

/// A shortcut that is also another's: the pair, and whether it is an error (the same scope) or a warning (an
/// everywhere one and one of a place: the place's wins over it).
#[derive(Debug, Clone, PartialEq)]
pub struct Clash {
    pub shortcut: Shortcut,
    pub a: &'static Action,
    pub b: &'static Action,
    pub error: bool,
}

/// The shortcuts given to two actions that would both answer to them, under the changes `o`.
pub fn clashes(o: &Overrides) -> Vec<Clash> {
    clashes_among(ACTIONS, o)
}

fn clashes_among(actions: &'static [Action], o: &Overrides) -> Vec<Clash> {
    let mut out = Vec::new();
    for (k, a) in actions.iter().enumerate() {
        let sa = bindings(o, a);
        for b in &actions[k + 1..] {
            let same = a.scope == b.scope;
            if !same && a.scope != Scope::Global && b.scope != Scope::Global {
                continue;
            }
            for s in bindings(o, b) {
                if sa.contains(&s) {
                    out.push(Clash { shortcut: s, a, b, error: same });
                }
            }
        }
    }
    out
}

/// The clashes now.
pub fn conflicts() -> Vec<Clash> {
    clashes(&overrides())
}

/// The state of the settings' Keyboard section.
#[derive(Debug, Default)]
pub struct Panel {
    pub filter: String,
    /// The action whose shortcut is being recorded and which of its shortcuts (past the last: a new one).
    pub recording: Option<(&'static str, usize)>,
    /// The combination pressed while recording, taken at the start of the frame.
    combo: Option<(Modifiers, Key)>,
}

impl Panel {
    /// At the start of every frame, before anything reads the keys: while a shortcut is recorded, the keys pressed
    /// are taken here (the canvas and the panes would take them otherwise) and the application's wait.
    pub fn capture(&mut self, ctx: &egui::Context) {
        RECORDING.store(self.recording.is_some(), Ordering::Relaxed);
        if self.recording.is_some()
            && let Some(c) = pressed_combo(ctx)
        {
            self.combo = Some(c);
        }
    }
}

/// Keep `list` as the shortcuts of `a` (nothing kept when they are its defaults).
fn set(map: &mut BTreeMap<String, Vec<String>>, a: &Action, list: Vec<Shortcut>) {
    if list == a.default {
        map.remove(a.id);
    } else {
        map.insert(a.id.to_string(), list.iter().map(|s| s.to_string()).collect());
    }
}

/// The key combination pressed in this frame, taken so that nothing else answers to it.
fn pressed_combo(ctx: &egui::Context) -> Option<(Modifiers, Key)> {
    ctx.input_mut(|i| {
        let k = i
            .events
            .iter()
            .position(|e| matches!(e, egui::Event::Key { pressed: true, key, .. } if !is_modifier(*key)))?;
        match i.events.remove(k) {
            egui::Event::Key { key, modifiers, .. } => Some((modifiers, key)),
            _ => None,
        }
    })
}

/// The Keyboard section of the settings: the actions by where they work, a filter, each shortcut a button that
/// records the next combination pressed (esc cancels, backspace clears it), a second one added, back to the
/// default, and the shortcuts two actions share told at once. Changes `map` (the settings' `shortcuts`).
pub fn panel(ui: &mut Ui, p: &mut Panel, map: &mut BTreeMap<String, Vec<String>>) {
    if let Some((id, slot)) = p.recording
        && let Some((m, key)) = p.combo.take()
        && let Some(a) = action(id)
    {
        let mut list = bindings(&parse_map(map), a);
        let plain = !m.command && !m.shift && !m.alt && !m.ctrl;
        p.recording = None;
        if plain && key == Key::Escape {
        } else if plain && key == Key::Backspace {
            if slot < list.len() {
                list.remove(slot);
            }
            set(map, a, list);
        } else {
            let s = Shortcut::new(m.command, m.shift, m.alt, key);
            if slot < list.len() {
                list[slot] = s;
            } else {
                list.push(s);
            }
            // (not twice)
            let mut kept: Vec<Shortcut> = Vec::new();
            for x in list {
                if !kept.contains(&x) {
                    kept.push(x);
                }
            }
            set(map, a, kept);
        }
    }
    let weak = crate::theme::weak();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut p.filter).hint_text("filter the actions").desired_width(200.0));
        if ui
            .add_enabled(!map.is_empty(), egui::Button::new("Reset all"))
            .on_hover_text("every shortcut back to its default")
            .clicked()
        {
            map.clear();
            p.recording = None;
        }
    });
    ui.label(
        RichText::new("click a shortcut, then press the keys (esc cancels, backspace takes it away)")
            .size(11.0)
            .color(weak),
    );
    ui.add_space(4.0);
    let o = parse_map(map);
    let all = clashes(&o);
    let filter = p.filter.trim().to_lowercase();
    egui::ScrollArea::vertical().max_height(400.0).auto_shrink([true, true]).show(ui, |ui| {
        for scope in Scope::ALL {
            let rows: Vec<&'static Action> = ACTIONS
                .iter()
                .filter(|a| a.scope == scope)
                .filter(|a| {
                    filter.is_empty()
                        || a.name.to_lowercase().contains(&filter)
                        || bindings(&o, a).iter().any(|s| s.show(false).to_lowercase().contains(&filter))
                })
                .collect();
            if rows.is_empty() {
                continue;
            }
            crate::theme::section(ui, scope.title());
            egui::Grid::new(("keymap", scope)).num_columns(4).spacing([14.0, 4.0]).show(ui, |ui| {
                for a in rows {
                    ui.label(a.name);
                    let list = bindings(&o, a);
                    ui.horizontal(|ui| {
                        if a.fixed {
                            let t = if list.is_empty() {
                                "none".to_string()
                            } else {
                                list.iter().map(|s| s.show(cfg!(target_os = "macos"))).collect::<Vec<_>>().join(", ")
                            };
                            ui.label(RichText::new(t).monospace().color(weak))
                                .on_hover_text("this one is read where it is used and cannot be changed yet");
                            return;
                        }
                        for (k, s) in list.iter().enumerate() {
                            let on = p.recording == Some((a.id, k));
                            let t =
                                if on { "press the keys...".to_string() } else { s.show(cfg!(target_os = "macos")) };
                            let r = ui
                                .add(egui::Button::selectable(on, RichText::new(t).monospace()))
                                .on_hover_text("click, then press the new shortcut");
                            if r.clicked() {
                                p.recording = if on { None } else { Some((a.id, k)) };
                            }
                        }
                        if list.is_empty() {
                            let on = p.recording == Some((a.id, 0));
                            let t = if on { "press the keys..." } else { "none" };
                            if ui
                                .add(egui::Button::selectable(on, RichText::new(t).monospace().color(weak)))
                                .on_hover_text("click, then press a shortcut")
                                .clicked()
                            {
                                p.recording = if on { None } else { Some((a.id, 0)) };
                            }
                        } else if list.len() < 2 {
                            let on = p.recording == Some((a.id, list.len()));
                            if ui
                                .add(egui::Button::selectable(on, if on { "press the keys..." } else { "+" }))
                                .on_hover_text("a second shortcut")
                                .clicked()
                            {
                                p.recording = if on { None } else { Some((a.id, list.len())) };
                            }
                        }
                    });
                    if map.contains_key(a.id) {
                        if ui.small_button("Reset").on_hover_text("back to the default").clicked() {
                            map.remove(a.id);
                            if p.recording.is_some_and(|(id, _)| id == a.id) {
                                p.recording = None;
                            }
                        }
                    } else {
                        ui.label("");
                    }
                    // the shortcuts shared with another action
                    ui.vertical(|ui| {
                        for c in all.iter().filter(|c| c.a.id == a.id || c.b.id == a.id) {
                            let other = if c.a.id == a.id { c.b } else { c.a };
                            let (colour, what) = if c.error {
                                (ui.visuals().error_fg_color, "also")
                            } else {
                                (ui.visuals().warn_fg_color, "also, over a place:")
                            };
                            ui.label(
                                RichText::new(format!(
                                    "{} {what} {} ({})",
                                    c.shortcut.show(cfg!(target_os = "macos")),
                                    other.name,
                                    other.scope.title()
                                ))
                                .size(11.0)
                                .color(colour),
                            );
                        }
                    });
                    ui.end_row();
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Vec<&'static str> {
        ACTIONS.iter().map(|a| a.id).collect()
    }

    #[test]
    fn the_ids_are_not_repeated() {
        let mut v = ids();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), ACTIONS.len());
        for id in APP.iter().chain(TABS) {
            assert!(action(id).is_some_and(|a| !a.fixed), "{id}");
        }
    }

    #[test]
    fn a_shortcut_is_written_and_read_back() {
        for k in Key::ALL.iter().copied().filter(|k| !is_modifier(*k)) {
            for (cmd, shift, alt) in
                [(false, false, false), (true, false, false), (true, true, true), (false, true, true)]
            {
                let s = Shortcut::new(cmd, shift, alt, k);
                assert_eq!(Shortcut::parse(&s.to_string()), Some(s), "{s}");
            }
        }
        assert_eq!(Shortcut::parse("ctrl+shift+s"), Some(Shortcut::new(true, true, false, Key::S)));
        assert_eq!(Shortcut::parse(" Cmd + Option + F5 "), Some(Shortcut::new(true, false, true, Key::F5)));
        assert_eq!(Shortcut::parse("Ctrl++"), Some(Shortcut::new(true, false, false, Key::Plus)));
        assert_eq!(Shortcut::parse("Ctrl+"), None);
        assert_eq!(Shortcut::parse("Hyper+S"), None);
        assert_eq!(Shortcut::parse("Ctrl+Shift"), None);
        assert_eq!(Shortcut::parse(""), None);
    }

    #[test]
    fn a_shortcut_is_shown_by_the_system() {
        let s = Shortcut::new(true, true, false, Key::S);
        assert_eq!(s.show(false), "ctrl+shift+S");
        assert_eq!(s.show(true), "⌘shift+S");
        assert_eq!(Shortcut::new(false, false, false, Key::F5).show(true), "F5");
        assert_eq!(s.to_string(), "Ctrl+Shift+S");
    }

    /// The keys the code read before the table (app.rs `Keys`, tabs.rs), written out: the defaults are the same.
    #[test]
    fn the_defaults_are_the_keys_of_before() {
        let before: &[(&str, &[&str], &str)] = &[
            ("run", &["F5"], "F5"),
            ("save", &["Ctrl+S"], "ctrl+S"),
            ("save_as", &["Ctrl+Shift+S"], "ctrl+shift+S"),
            ("version", &["Ctrl+Alt+S"], "ctrl+alt+S"),
            ("new", &["Ctrl+N"], "ctrl+N"),
            ("open_model", &["Ctrl+Shift+O"], "ctrl+shift+O"),
            ("open_data", &["Ctrl+O"], "ctrl+O"),
            ("commands", &["Ctrl+K", "Ctrl+Shift+P"], "ctrl+K"),
            ("shortcuts", &["F1"], "F1"),
            ("to_model", &["Tab"], "Tab"),
            ("close_pane", &["Ctrl+W"], "ctrl+W"),
            ("next_tab", &["Ctrl+Tab"], "ctrl+Tab"),
            ("prev_tab", &["Ctrl+Shift+Tab"], "ctrl+shift+Tab"),
            ("close_tab", &["Ctrl+Shift+W"], "ctrl+shift+W"),
            ("undo", &["Ctrl+Z"], "ctrl+Z"),
            ("redo", &["Ctrl+Shift+Z"], "ctrl+shift+Z"),
            ("add_operator", &["Slash"], "/"),
        ];
        assert_eq!(before.len(), ACTIONS.len());
        for (id, keys, first) in before {
            let a = action(id).unwrap_or_else(|| panic!("{id}"));
            let got: Vec<String> = a.default.iter().map(|s| s.to_string()).collect();
            assert_eq!(&got, keys, "{id}");
            assert_eq!(a.default[0].show(false), *first, "{id}");
        }
    }

    #[test]
    fn the_changes_replace_the_defaults() {
        let run = action("run").unwrap();
        let mut map = BTreeMap::new();
        assert_eq!(bindings(&parse_map(&map), run), run.default);
        map.insert("run".to_string(), vec!["Ctrl+R".to_string()]);
        assert_eq!(bindings(&parse_map(&map), run), vec![Shortcut::new(true, false, false, Key::R)]);
        // an empty list: no shortcut; a bad one left out
        map.insert("run".to_string(), vec![]);
        assert!(bindings(&parse_map(&map), run).is_empty());
        map.insert("run".to_string(), vec!["Hyper+R".to_string(), "F6".to_string()]);
        assert_eq!(bindings(&parse_map(&map), run), vec![Shortcut::new(false, false, false, Key::F6)]);
        // a fixed action keeps its own
        let undo = action("undo").unwrap();
        map.insert("undo".to_string(), vec!["F9".to_string()]);
        assert_eq!(bindings(&parse_map(&map), undo), undo.default);
        // back to the defaults: nothing kept
        set(&mut map, run, run.default.to_vec());
        assert!(!map.contains_key("run"));
        set(&mut map, run, vec![]);
        assert_eq!(map["run"], Vec::<String>::new());
    }

    #[test]
    fn the_defaults_do_not_clash() {
        assert!(clashes(&Overrides::new()).is_empty());
    }

    #[test]
    fn two_actions_of_one_scope_clash_and_a_place_only_warns() {
        let mut map = BTreeMap::new();
        map.insert("run".to_string(), vec!["Ctrl+S".to_string()]);
        let c = clashes(&parse_map(&map));
        assert_eq!(c.len(), 1);
        assert!(c[0].error);
        assert_eq!((c[0].a.id, c[0].b.id), ("run", "save"));
        assert_eq!(c[0].shortcut, Shortcut::new(true, false, false, Key::S));
        // a fixed one counts too
        map.insert("run".to_string(), vec!["Ctrl+Z".to_string()]);
        assert!(clashes(&parse_map(&map)).iter().any(|c| c.error && c.b.id == "undo"));
    }

    #[test]
    fn an_everywhere_shortcut_against_a_place_only_warns() {
        // (the scopes of the places have no action in the table yet: some made here)
        static MADE: [Action; 4] = [
            global("g", "g", &[key(Key::F5)]),
            Action { id: "c", name: "c", scope: Scope::Canvas, default: &[key(Key::F5)], fixed: false, idle: false },
            Action { id: "v", name: "v", scope: Scope::Viewer, default: &[key(Key::F5)], fixed: false, idle: false },
            Action { id: "c2", name: "c2", scope: Scope::Canvas, default: &[key(Key::F6)], fixed: false, idle: false },
        ];
        let c = clashes_among(&MADE, &Overrides::new());
        // everywhere with each place: warnings; the canvas with the viewer: none (the pointer is over one)
        assert_eq!(
            c.iter().map(|c| (c.a.id, c.b.id, c.error)).collect::<Vec<_>>(),
            vec![("g", "c", false), ("g", "v", false)]
        );
        let mut o = Overrides::new();
        o.insert("c2".into(), vec![key(Key::F5)]);
        let c = clashes_among(&MADE, &o);
        assert!(c.iter().any(|c| (c.a.id, c.b.id, c.error) == ("c", "c2", true)));
    }

    fn pressed(events: Vec<egui::Event>, o: &Overrides, ids: &[&str]) -> Vec<&'static str> {
        let ctx = egui::Context::default();
        let mut hit = Vec::new();
        let input = egui::RawInput { events, ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| hit = take_with(ui.ctx(), o, ids));
        out.textures_delta.clear();
        hit
    }

    fn press(key: Key, m: Modifiers) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m }
    }

    #[test]
    fn the_most_specific_shortcut_takes_the_key() {
        let none = Overrides::new();
        let c = Modifiers::COMMAND;
        assert_eq!(pressed(vec![press(Key::S, c)], &none, APP), vec!["save"]);
        assert_eq!(pressed(vec![press(Key::S, c | Modifiers::SHIFT)], &none, APP), vec!["save_as"]);
        assert_eq!(pressed(vec![press(Key::S, c | Modifiers::ALT)], &none, APP), vec!["version"]);
        assert_eq!(pressed(vec![press(Key::F5, Modifiers::NONE)], &none, APP), vec!["run"]);
        assert_eq!(pressed(vec![press(Key::P, c | Modifiers::SHIFT)], &none, APP), vec!["commands"]);
        assert_eq!(pressed(vec![press(Key::K, c)], &none, APP), vec!["commands"]);
        assert_eq!(pressed(vec![press(Key::Tab, c | Modifiers::SHIFT)], &none, TABS), vec!["prev_tab"]);
        assert_eq!(pressed(vec![press(Key::Tab, c)], &none, TABS), vec!["next_tab"]);
        assert_eq!(pressed(vec![press(Key::W, c | Modifiers::SHIFT)], &none, TABS), vec!["close_tab"]);
        assert_eq!(pressed(vec![press(Key::W, c)], &none, APP), vec!["close_pane"]);
        // the others are not asked for
        assert!(pressed(vec![press(Key::Tab, c)], &none, APP).is_empty());
        // a changed shortcut works and the old one does not
        let mut map = BTreeMap::new();
        map.insert("run".to_string(), vec!["Ctrl+R".to_string()]);
        let o = parse_map(&map);
        assert_eq!(pressed(vec![press(Key::R, c)], &o, APP), vec!["run"]);
        assert!(pressed(vec![press(Key::F5, Modifiers::NONE)], &o, APP).is_empty());
    }
}
