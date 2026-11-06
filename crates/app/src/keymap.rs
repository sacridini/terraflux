//! The keyboard shortcuts as a table of actions, each with its default shortcuts and the user's own (kept in the
//! settings, `shortcuts`): what reads the keys asks the table for an action by its id, so that a shortcut changed
//! in Settings (Keyboard) is the one every menu, the palette and the sheet of shortcuts (F1) show and the one that
//! works. The keys of the whole application are in it, and those of the canvas, the Viewer, the calculator, the
//! files and the other panes: what reads them asks for them by id ([`take`] for the application's, [`pressed`] and
//! [`consume`] inside an input state). The keys that are part of editing a text or of a widget's own behaviour
//! (enter, escape, the arrows in a list, the digits of the places) are in it too, `fixed`: shown, not changed.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use egui::{Key, Modifiers, RichText, Ui};

/// Where an action works: everywhere (the keys of the application), or over one place (that place takes its key
/// when the pointer is over it, so a shortcut the same as an everywhere one is a warning, not an error: the
/// place's wins there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    Global,
    Canvas,
    Viewer,
    Calculator,
    Files,
    Assistant,
    Table,
    Dialogs,
}

impl Scope {
    pub const ALL: [Scope; 8] = [
        Scope::Global,
        Scope::Canvas,
        Scope::Viewer,
        Scope::Calculator,
        Scope::Files,
        Scope::Assistant,
        Scope::Table,
        Scope::Dialogs,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Scope::Global => "Everywhere",
            Scope::Canvas => "The model's canvas",
            Scope::Viewer => "Maps",
            Scope::Calculator => "The calculator",
            Scope::Files => "Files",
            Scope::Assistant => "The assistant",
            Scope::Table => "Attribute tables",
            Scope::Dialogs => "Lists and boxes (the palette, the searches, the notes)",
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

/// A key of a place that can be changed.
const fn at(id: &'static str, scope: Scope, name: &'static str, default: &'static [Shortcut]) -> Action {
    Action { id, name, scope, default, fixed: false, idle: false }
}

/// A key of a place that is part of editing a text or of a widget's own behaviour: shown, not changed.
const fn part(id: &'static str, scope: Scope, name: &'static str, default: &'static [Shortcut]) -> Action {
    Action { id, name, scope, default, fixed: true, idle: false }
}

/// 1 to 9, and with ctrl: the slots of the places.
const DIGITS: [Shortcut; 9] = [
    key(Key::Num1),
    key(Key::Num2),
    key(Key::Num3),
    key(Key::Num4),
    key(Key::Num5),
    key(Key::Num6),
    key(Key::Num7),
    key(Key::Num8),
    key(Key::Num9),
];
const CMD_DIGITS: [Shortcut; 9] = [
    cmd(Key::Num1),
    cmd(Key::Num2),
    cmd(Key::Num3),
    cmd(Key::Num4),
    cmd(Key::Num5),
    cmd(Key::Num6),
    cmd(Key::Num7),
    cmd(Key::Num8),
    cmd(Key::Num9),
];

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
    // The model's canvas (the pointer over it, or it in focus).
    at("canvas_home", Scope::Canvas, "The whole graph in view", &[key(Key::Home)]),
    at("canvas_frame", Scope::Canvas, "The nodes selected in view (none selected: the whole graph)", &[key(Key::F)]),
    part(
        "canvas_beside",
        Scope::Canvas,
        "Go to the node beside",
        &[key(Key::ArrowLeft), key(Key::ArrowRight), key(Key::ArrowUp), key(Key::ArrowDown)],
    ),
    part("canvas_show", Scope::Canvas, "Show what the node selected gives", &[key(Key::Enter)]),
    at("canvas_run_to", Scope::Canvas, "Run up to the node selected", &[cmd(Key::Enter)]),
    at("canvas_delete", Scope::Canvas, "Delete the nodes selected", &[key(Key::Delete), key(Key::Backspace)]),
    at("canvas_duplicate", Scope::Canvas, "Duplicate the nodes selected", &[cmd(Key::D)]),
    part("canvas_copy", Scope::Canvas, "Copy the nodes selected", &[cmd(Key::C)]),
    part("canvas_paste", Scope::Canvas, "Paste them", &[cmd(Key::V)]),
    at("canvas_find", Scope::Canvas, "Find a node by id, operator, note or file (inside groups too)", &[cmd(Key::F)]),
    at("canvas_rename", Scope::Canvas, "Rename the node selected", &[cmd(Key::R)]),
    at("canvas_arrange", Scope::Canvas, "Arrange the nodes", &[cmd(Key::L)]),
    at("canvas_group", Scope::Canvas, "Group the nodes selected", &[cmd(Key::G)]),
    at("canvas_ungroup", Scope::Canvas, "Ungroup the group selected", &[cmd_shift(Key::G)]),
    at(
        "canvas_bypass",
        Scope::Canvas,
        "Turn the nodes selected off or on again (bypass: their inputs passed on)",
        &[key(Key::B)],
    ),
    at("canvas_select_all", Scope::Canvas, "Select all the nodes", &[cmd(Key::A)]),
    part("canvas_select_none", Scope::Canvas, "Select none", &[key(Key::Escape)]),
    part(
        "canvas_place_keep",
        Scope::Canvas,
        "Keep where the canvas looks in a slot, as a place saved with the model",
        &CMD_DIGITS,
    ),
    part("canvas_place_go", Scope::Canvas, "Go to the place in that slot (the Viewer has its own)", &DIGITS),
    // The maps (the pointer over one).
    at("viewer_satellite", Scope::Viewer, "The satellite under the map", &[key(Key::B)]),
    at("viewer_hide", Scope::Viewer, "Hide the map, or show it again", &[key(Key::Space)]),
    at(
        "viewer_measure",
        Scope::Viewer,
        "Measure: click points for lengths, area and perimeter (esc ends)",
        &[key(Key::M)],
    ),
    at(
        "viewer_goto",
        Scope::Viewer,
        "Go to a coordinate (degrees or the map's units) and mark it (esc lets the mark go)",
        &[cmd(Key::G)],
    ),
    at(
        "viewer_back",
        Scope::Viewer,
        "The view before (the mouse's back button too)",
        &[Shortcut::new(false, false, true, Key::ArrowLeft)],
    ),
    at(
        "viewer_forward",
        Scope::Viewer,
        "The view after (the mouse's forward button too)",
        &[Shortcut::new(false, false, true, Key::ArrowRight)],
    ),
    at("viewer_step_back", Scope::Viewer, "The step before (Simulation)", &[key(Key::ArrowLeft)]),
    at("viewer_step_next", Scope::Viewer, "The step after (Simulation)", &[key(Key::ArrowRight)]),
    part("viewer_finish", Scope::Viewer, "Finish the shape being drawn", &[key(Key::Enter)]),
    part("viewer_undo_point", Scope::Viewer, "Take the last point away (drawing, measuring)", &[key(Key::Backspace)]),
    part("viewer_cancel", Scope::Viewer, "Leave the shape, the line, the selection or the mark", &[key(Key::Escape)]),
    part(
        "viewer_place_keep",
        Scope::Viewer,
        "Keep the view in a slot, as a place saved with the model (Places)",
        &CMD_DIGITS,
    ),
    part("viewer_place_go", Scope::Viewer, "Go to the place in that slot", &DIGITS),
    // The calculator's script and console.
    at("calc_run", Scope::Calculator, "Give the script to the node and run up to it", &[cmd(Key::Enter)]),
    at("calc_complete", Scope::Calculator, "The names that fit (completions)", &[cmd(Key::Space)]),
    part(
        "calc_next",
        Scope::Calculator,
        "The next completion (the console: the line typed after)",
        &[key(Key::ArrowDown)],
    ),
    part(
        "calc_prev",
        Scope::Calculator,
        "The completion before (the console: the line typed before)",
        &[key(Key::ArrowUp)],
    ),
    part(
        "calc_accept",
        Scope::Calculator,
        "Put the completion chosen in (the console: compute the line)",
        &[key(Key::Tab), key(Key::Enter)],
    ),
    part("calc_close", Scope::Calculator, "Close the completions", &[key(Key::Escape)]),
    // The files pane.
    at("files_back", Scope::Files, "Back to the folder before", &[Shortcut::new(false, false, true, Key::ArrowLeft)]),
    at(
        "files_forward",
        Scope::Files,
        "Forward to the folder after",
        &[Shortcut::new(false, false, true, Key::ArrowRight)],
    ),
    at(
        "files_up",
        Scope::Files,
        "Up to the folder that holds this one",
        &[Shortcut::new(false, false, true, Key::ArrowUp)],
    ),
    at("files_location", Scope::Files, "Type a path", &[cmd(Key::L)]),
    part(
        "files_rows",
        Scope::Files,
        "The row above, below (shift: one more chosen)",
        &[key(Key::ArrowUp), key(Key::ArrowDown)],
    ),
    part("files_fold", Scope::Files, "Open, close the folder chosen", &[key(Key::ArrowRight), key(Key::ArrowLeft)]),
    part("files_enter", Scope::Files, "Open the entries chosen (a path typed: go there)", &[key(Key::Enter)]),
    // The assistant.
    part("chat_send", Scope::Assistant, "Send the question", &[key(Key::Enter)]),
    part("chat_stop", Scope::Assistant, "Stop the answer being written", &[key(Key::Escape)]),
    part(
        "chat_recall",
        Scope::Assistant,
        "The questions asked before, and after",
        &[key(Key::ArrowUp), key(Key::ArrowDown)],
    ),
    // The attribute tables.
    at("table_select_all", Scope::Table, "Select the rows shown", &[cmd(Key::A)]),
    part("table_none", Scope::Table, "Let the selection go", &[key(Key::Escape)]),
    // The lists and boxes.
    part("list_down", Scope::Dialogs, "Choose the next in a list", &[key(Key::ArrowDown)]),
    part("list_up", Scope::Dialogs, "Choose the one before in a list", &[key(Key::ArrowUp)]),
    part("box_enter", Scope::Dialogs, "Do the one chosen, confirm a box", &[key(Key::Enter)]),
    part("box_escape", Scope::Dialogs, "Close a list or a box without doing it", &[key(Key::Escape)]),
    part("note_save", Scope::Dialogs, "Save a note", &[cmd(Key::Enter)]),
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
    list_text(&current(id))
}

/// Shortcuts written for the sheets: "ctrl+K, ctrl+shift+P"; digits one after the other as a range, "1 to 9".
fn list_text(list: &[Shortcut]) -> String {
    let mac = cfg!(target_os = "macos");
    let digit = |s: &Shortcut| s.key.name().parse::<u8>().ok().filter(|_| s.key.name().len() == 1);
    let next = |a: &Shortcut, b: &Shortcut| {
        (a.cmd, a.shift, a.alt) == (b.cmd, b.shift, b.alt) && digit(a).zip(digit(b)).is_some_and(|(x, y)| y == x + 1)
    };
    let mut out: Vec<String> = Vec::new();
    let mut k = 0;
    while k < list.len() {
        let mut e = k;
        while e + 1 < list.len() && next(&list[e], &list[e + 1]) {
            e += 1;
        }
        if e >= k + 2 {
            out.push(format!("{} to {}", list[k].show(mac), list[e].show(mac)));
        } else {
            out.extend(list[k..=e].iter().map(|s| s.show(mac)));
        }
        k = e + 1;
    }
    out.join(", ")
}

/// The actions of a scope with their shortcuts, for the sheet of shortcuts (F1): the keys, the action.
pub fn of_scope(scope: Scope) -> Vec<(String, &'static str)> {
    let o = overrides();
    ACTIONS
        .iter()
        .filter(|a| a.scope == scope)
        .map(|a| {
            let keys = list_text(&bindings(&o, a));
            (if keys.is_empty() { "none".to_string() } else { keys }, a.name)
        })
        .collect()
}

/// The actions of the whole application with their shortcuts, for the sheet of shortcuts (F1).
pub fn everywhere() -> Vec<(String, &'static str)> {
    of_scope(Scope::Global)
}

/// Take the keys of `ids` pressed in this frame: those pressed. While the settings record a shortcut, none.
/// A shortcut of an everywhere action that an action of the place the pointer is over has too is left to that
/// one (it reads it over its place).
pub fn take(ctx: &egui::Context, ids: &[&str]) -> Vec<&'static str> {
    take_with(ctx, &overrides(), ids)
}

/// The places the pointer was over in the last frames, told by [`over`].
fn hovered(ctx: &egui::Context) -> Vec<Scope> {
    let now = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<HashMap<Scope, u64>>(egui::Id::new("keymap over")))
        .map(|m| m.into_iter().filter(|(_, at)| at + 2 >= now).map(|(s, _)| s).collect())
        .unwrap_or_default()
}

/// Tell that the pointer is over the place of `scope` (the canvas, a map) in this frame: its keys come before the
/// everywhere ones the same in the next.
pub fn over(ctx: &egui::Context, scope: Scope) {
    let now = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<HashMap<Scope, u64>>(egui::Id::new("keymap over")).insert(scope, now);
    });
}

fn take_with(ctx: &egui::Context, o: &Overrides, ids: &[&str]) -> Vec<&'static str> {
    if recording() {
        return Vec::new();
    }
    let typing = ctx.memory(|m| m.focused().is_some());
    let places = if typing { Vec::new() } else { hovered(ctx) };
    let all = candidates(o, ids)
        .into_iter()
        .filter(|(a, s)| !typing || !(a.idle || s.types_text()))
        .filter(|(a, s)| {
            a.scope != Scope::Global
                || !ACTIONS.iter().any(|b| {
                    !b.fixed && places.contains(&b.scope) && b.scope != Scope::Global && bindings(o, b).contains(s)
                })
        })
        .collect();
    ctx.input_mut(|i| press(i, all))
}

/// The shortcuts of the actions `ids` (those that can be changed), the most specific first: a shortcut without
/// shift or alt takes the key with them too.
fn candidates(o: &Overrides, ids: &[&str]) -> Vec<(&'static Action, Shortcut)> {
    let mut all: Vec<(&'static Action, Shortcut)> = ACTIONS
        .iter()
        .filter(|a| !a.fixed && ids.contains(&a.id))
        .flat_map(|a| bindings(o, a).into_iter().map(move |s| (a, s)))
        .collect();
    all.sort_by_key(|(_, s)| Reverse(s.shift as u8 + s.alt as u8));
    all
}

fn press(i: &mut egui::InputState, all: Vec<(&'static Action, Shortcut)>) -> Vec<&'static str> {
    let mut hit = Vec::new();
    for (a, s) in all {
        if i.consume_key(s.modifiers(), s.key) && !hit.contains(&a.id) {
            hit.push(a.id);
        }
    }
    hit
}

/// Take, from an input state, the keys of `ids` (actions that can be changed) pressed: those pressed. What asks
/// has seen to it that the place wants its keys now (nothing being typed, the pointer over it). While the
/// settings record a shortcut, none.
pub fn consume(i: &mut egui::InputState, ids: &[&str]) -> Vec<&'static str> {
    if recording() {
        return Vec::new();
    }
    press(i, candidates(&overrides(), ids))
}

/// Whether the key of the action `id` was pressed, and taken.
pub fn pressed(i: &mut egui::InputState, id: &str) -> bool {
    !consume(i, &[id]).is_empty()
}

/// Whether the key of the action `id` was pressed, not taken: shift and alt held with it do not matter (a shortcut
/// that has none of the modifiers answers to any).
pub fn peek(i: &egui::InputState, id: &str) -> bool {
    !recording()
        && current(id).iter().any(|s| {
            i.key_pressed(s.key) && (!(s.cmd || s.shift || s.alt) || i.modifiers.matches_logically(s.modifiers()))
        })
}

/// Whether the key of the action `id` was pressed, not taken, with the modifiers it has and no more.
pub fn peek_exact(i: &egui::InputState, id: &str) -> bool {
    !recording() && current(id).iter().any(|s| i.key_pressed(s.key) && i.modifiers.matches_exact(s.modifiers()))
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
            // (an everywhere action that waits while a text is edited, and a key of that editing: never together)
            if (a.idle && b.fixed) || (b.idle && a.fixed) {
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

/// Which of an everywhere action and one of a place has the key when the pointer is over the place, said for the
/// row of `row` (nothing for two actions of one scope): the place's, unless its key cannot be changed (the
/// everywhere one is read first then).
fn wins(c: &Clash, row: &Action) -> String {
    if c.error {
        return String::new();
    }
    let (global, place) = if c.a.scope == Scope::Global { (c.a, c.b) } else { (c.b, c.a) };
    if place.fixed {
        return ": the everywhere one is taken first".to_string();
    }
    if row.scope == Scope::Global {
        format!(": over {} that one wins", place.scope.title())
    } else {
        format!(": over {} this one wins over {}", place.scope.title(), global.name)
    }
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
                            let t = if list.is_empty() { "none".to_string() } else { list_text(&list) };
                            ui.label(RichText::new(t).monospace().color(weak)).on_hover_text(
                                "this one is part of editing a text or of how the widget works: it cannot be changed",
                            );
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
                                (ui.visuals().warn_fg_color, "also")
                            };
                            ui.label(
                                RichText::new(format!(
                                    "{} {what} {} ({}){}",
                                    c.shortcut.show(cfg!(target_os = "macos")),
                                    other.name,
                                    other.scope.title(),
                                    wins(c, a)
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
            // the canvas (builder.rs, editor_view.rs, places.rs)
            ("canvas_home", &["Home"], "Home"),
            ("canvas_frame", &["F"], "F"),
            ("canvas_beside", &["Left", "Right", "Up", "Down"], "Left"),
            ("canvas_show", &["Enter"], "Enter"),
            ("canvas_run_to", &["Ctrl+Enter"], "ctrl+Enter"),
            ("canvas_delete", &["Delete", "Backspace"], "Delete"),
            ("canvas_duplicate", &["Ctrl+D"], "ctrl+D"),
            ("canvas_copy", &["Ctrl+C"], "ctrl+C"),
            ("canvas_paste", &["Ctrl+V"], "ctrl+V"),
            ("canvas_find", &["Ctrl+F"], "ctrl+F"),
            ("canvas_rename", &["Ctrl+R"], "ctrl+R"),
            ("canvas_arrange", &["Ctrl+L"], "ctrl+L"),
            ("canvas_group", &["Ctrl+G"], "ctrl+G"),
            ("canvas_ungroup", &["Ctrl+Shift+G"], "ctrl+shift+G"),
            ("canvas_bypass", &["B"], "B"),
            ("canvas_select_all", &["Ctrl+A"], "ctrl+A"),
            ("canvas_select_none", &["Escape"], "Escape"),
            ("canvas_place_keep", &DIGIT_NAMES_CTRL, "ctrl+1"),
            ("canvas_place_go", &DIGIT_NAMES, "1"),
            // the maps (map_view.rs, app.rs, draw.rs, measure.rs)
            ("viewer_satellite", &["B"], "B"),
            ("viewer_hide", &["Space"], "Space"),
            ("viewer_measure", &["M"], "M"),
            ("viewer_goto", &["Ctrl+G"], "ctrl+G"),
            ("viewer_back", &["Alt+Left"], "alt+Left"),
            ("viewer_forward", &["Alt+Right"], "alt+Right"),
            ("viewer_step_back", &["Left"], "Left"),
            ("viewer_step_next", &["Right"], "Right"),
            ("viewer_finish", &["Enter"], "Enter"),
            ("viewer_undo_point", &["Backspace"], "Backspace"),
            ("viewer_cancel", &["Escape"], "Escape"),
            ("viewer_place_keep", &DIGIT_NAMES_CTRL, "ctrl+1"),
            ("viewer_place_go", &DIGIT_NAMES, "1"),
            // the calculator (calc_view.rs)
            ("calc_run", &["Ctrl+Enter"], "ctrl+Enter"),
            ("calc_complete", &["Ctrl+Space"], "ctrl+Space"),
            ("calc_next", &["Down"], "Down"),
            ("calc_prev", &["Up"], "Up"),
            ("calc_accept", &["Tab", "Enter"], "Tab"),
            ("calc_close", &["Escape"], "Escape"),
            // the files (files_view.rs)
            ("files_back", &["Alt+Left"], "alt+Left"),
            ("files_forward", &["Alt+Right"], "alt+Right"),
            ("files_up", &["Alt+Up"], "alt+Up"),
            ("files_location", &["Ctrl+L"], "ctrl+L"),
            ("files_rows", &["Up", "Down"], "Up"),
            ("files_fold", &["Right", "Left"], "Right"),
            ("files_enter", &["Enter"], "Enter"),
            // the assistant (chat.rs), the tables (attribute_table.rs), the lists and boxes
            ("chat_send", &["Enter"], "Enter"),
            ("chat_stop", &["Escape"], "Escape"),
            ("chat_recall", &["Up", "Down"], "Up"),
            ("table_select_all", &["Ctrl+A"], "ctrl+A"),
            ("table_none", &["Escape"], "Escape"),
            ("list_down", &["Down"], "Down"),
            ("list_up", &["Up"], "Up"),
            ("box_enter", &["Enter"], "Enter"),
            ("box_escape", &["Escape"], "Escape"),
            ("note_save", &["Ctrl+Enter"], "ctrl+Enter"),
        ];
        assert_eq!(before.len(), ACTIONS.len());
        for (id, keys, first) in before {
            let a = action(id).unwrap_or_else(|| panic!("{id}"));
            let got: Vec<String> = a.default.iter().map(|s| s.to_string()).collect();
            assert_eq!(&got, keys, "{id}");
            assert_eq!(a.default[0].show(false), *first, "{id}");
        }
    }

    const DIGIT_NAMES: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
    const DIGIT_NAMES_CTRL: [&str; 9] =
        ["Ctrl+1", "Ctrl+2", "Ctrl+3", "Ctrl+4", "Ctrl+5", "Ctrl+6", "Ctrl+7", "Ctrl+8", "Ctrl+9"];

    /// Which of the keys of the places can be changed, and which cannot: written out.
    #[test]
    fn the_scopes_and_the_fixed_keys() {
        let fixed: Vec<&str> = ACTIONS.iter().filter(|a| a.fixed).map(|a| a.id).collect();
        for id in [
            "undo",
            "redo",
            "add_operator",
            "canvas_beside",
            "canvas_show",
            "canvas_copy",
            "canvas_paste",
            "canvas_select_none",
            "canvas_place_keep",
            "canvas_place_go",
            "viewer_place_keep",
            "viewer_place_go",
            "viewer_finish",
            "viewer_undo_point",
            "viewer_cancel",
            "calc_next",
            "calc_prev",
            "calc_accept",
            "calc_close",
            "files_rows",
            "files_fold",
            "files_enter",
            "chat_send",
            "chat_stop",
            "chat_recall",
            "table_none",
            "list_down",
            "list_up",
            "box_enter",
            "box_escape",
            "note_save",
        ] {
            assert!(fixed.contains(&id), "{id} is fixed");
        }
        assert_eq!(fixed.len(), 31);
        for (id, scope) in [
            ("canvas_frame", Scope::Canvas),
            ("viewer_satellite", Scope::Viewer),
            ("calc_run", Scope::Calculator),
            ("files_location", Scope::Files),
            ("chat_send", Scope::Assistant),
            ("table_select_all", Scope::Table),
            ("box_enter", Scope::Dialogs),
        ] {
            assert_eq!(action(id).unwrap().scope, scope, "{id}");
        }
        // every scope has a row in the sheets, the digits as a range
        for s in Scope::ALL {
            assert!(!of_scope(s).is_empty(), "{s:?}");
        }
        assert!(of_scope(Scope::Canvas).iter().any(|(k, w)| k == "ctrl+1 to ctrl+9" && w.starts_with("Keep where")));
        assert!(of_scope(Scope::Canvas).iter().any(|(k, _)| k == "1 to 9"));
        assert!(of_scope(Scope::Canvas).iter().any(|(k, _)| k == "Delete, Backspace"));
    }

    #[test]
    fn a_range_of_digits_is_written_short() {
        let d = |k: Key| key(k);
        assert_eq!(list_text(&[d(Key::Num1), d(Key::Num2)]), "1, 2");
        assert_eq!(list_text(&[d(Key::Num1), d(Key::Num2), d(Key::Num3)]), "1 to 3");
        assert_eq!(list_text(&[d(Key::Num1), d(Key::Num3)]), "1, 3");
        assert_eq!(list_text(&[d(Key::F)]), "F");
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
        // (some made here: the table's own never clash by default)
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
    fn a_place_wins_over_an_everywhere_shortcut_the_same() {
        let mut map = BTreeMap::new();
        map.insert("run".to_string(), vec!["Ctrl+R".to_string()]);
        let o = parse_map(&map);
        let c = clashes(&o);
        assert_eq!(c.len(), 1);
        assert!(!c[0].error);
        assert_eq!((c[0].a.id, c[0].b.id), ("run", "canvas_rename"));
        assert!(wins(&c[0], c[0].a).contains("that one wins"));
        assert!(wins(&c[0], c[0].b).contains("this one wins"));
        // a key that cannot be changed is not given up: the everywhere one is first
        map.insert("run".to_string(), vec!["Enter".to_string()]);
        let c = clashes(&parse_map(&map));
        assert!(c.iter().all(|c| !c.error));
        assert!(c.iter().any(|c| wins(c, c.a).contains("taken first")));
        // pointer over the canvas (told in the frame before): the canvas has ctrl+R, the model is not run
        let ctx = egui::Context::default();
        let run = |events: Vec<egui::Event>, over_canvas: bool| {
            let mut hit = Vec::new();
            let input = egui::RawInput { events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                hit = take_with(ui.ctx(), &parse_map(&map_run_r()), APP);
                if over_canvas {
                    over(ui.ctx(), Scope::Canvas);
                }
            });
            out.textures_delta.clear();
            hit
        };
        let c = Modifiers::COMMAND;
        assert_eq!(run(vec![press(Key::R, c)], false), vec!["run"]);
        run(vec![], true);
        assert!(run(vec![press(Key::R, c)], true).is_empty());
        // and where the pointer is over a map, ctrl+R is the application's again (the map has no ctrl+R)
        let ctx2 = egui::Context::default();
        let mut hit = Vec::new();
        for events in [vec![], vec![press(Key::R, c)]] {
            let input = egui::RawInput { events, ..Default::default() };
            let mut out = ctx2.run_ui(input, |ui| {
                hit = take_with(ui.ctx(), &parse_map(&map_run_r()), APP);
                over(ui.ctx(), Scope::Viewer);
            });
            out.textures_delta.clear();
        }
        assert_eq!(hit, vec!["run"]);
    }

    fn map_run_r() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([("run".to_string(), vec!["Ctrl+R".to_string()])])
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
