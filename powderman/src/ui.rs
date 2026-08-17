//! The UI, as a Dioxus liveview component.
//!
//! Server-rendered and reactive over a websocket: the browser holds no state
//! and there is no build step. The component polls the database on a timer
//! and re-renders, which at this size is simpler than pushing invalidations
//! and is indistinguishable to look at.
//!
//! What it shows is deliberately narrow — what runs exist, what each is
//! parked on, and what every step recorded. That last one is not a log: it is
//! the replay data, so what you are reading is literally what a resumed run
//! would return instead of re-executing.

use dioxus::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StepView {
    pub key: String,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RunView {
    pub id: String,
    pub workflow: String,
    pub status: String,
    pub note: Option<String>,
    pub error: Option<String>,
    pub updated_at: i64,
    pub steps: Vec<StepView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorkflowView {
    pub name: String,
    pub description: String,
    pub example: Option<String>,
    pub schedule: Option<String>,
    pub next: Option<String>,
}

/// One thing on this box that fires on a clock, whoever owns it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TimerRow {
    pub source: String,
    pub name: String,
    pub schedule: String,
    pub next: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub workflow: String,
    pub from: i64,
    pub to: i64,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    pub name: String,
    pub params: serde_json::Value,
    pub at: i64,
    pub ok: bool,
    /// Who ran it — "ui" for a click or a chord, "agent" for the MCP server.
    /// The bus is deliberately one path, so the log is the only place the two
    /// are distinguishable, and it is worth being able to tell.
    #[serde(default)]
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct State {
    pub herdr: Option<String>,
    pub log: Vec<LogEntry>,
    pub workflows: Vec<WorkflowView>,
    pub runs: Vec<RunView>,
    pub machine: std::collections::HashMap<String, f64>,
    pub cpu: Vec<(i64, f64)>,
    pub mem: Vec<(i64, f64)>,
    pub annotations: Vec<Annotation>,
    pub fleet: Vec<crate::metrics::FleetAgent>,
    pub timers: Vec<TimerRow>,
    pub window: (i64, i64),
}

pub const CSS: &str = include_str!("ui.css");

pub(crate) fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

pub(crate) fn hhmmss(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_default()
}

pub(crate) fn gib(bytes: f64) -> String {
    format!("{:.1}G", bytes / 1_073_741_824.0)
}

/// A stat tile. The headline number, big, with its context small beneath —
/// not a run of text in a row.
pub(crate) fn tile(k: &str, v: String, of: Option<String>) -> Element {
    rsx! {
        div { class: "tile",
            div { class: "k", "{k}" }
            div { class: "v", "{v}" }
            if let Some(o) = of { div { class: "of", "{o}" } }
        }
    }
}

use crate::menus::{
    edit_menu, favorites_menu_json, file_menu, help_menu, pie_menu_json, repeat_history_menu,
    theme_menu, undo_history_menu, view_menu, window_menu,
};
use immersion::{
    AreaId, Areas, Chrome, ContextMenu, Field, FieldKind, Keymap, KeymapHelp, Layout, LayoutFile,
    Palette, PaletteItem, Panel, Platform, Splash, StatusBar, WorkspaceTabs, default_keymap,
    pretty_chord,
};

/// The registry: what an area's dropdown offers. The ids are what the tree
/// stores, so renaming one is a migration, not a refactor.
/// The palette's entries: the actions searchable by F3. They are the same
/// `(action, params)` pairs the keymap fires — host actions (undo/redo/
/// maximize) and navigational bus commands — with a label to search by and the
/// chord they are bound to, so the palette, a chord, and an agent all reach one
/// router. Workspace switches are generated per tab, so the palette lists the
/// rooms you can jump to by name.
/// A host mutation: changes server truth, but needs state the bus signature
/// cannot reach (the undo stacks, the settings document). The parity contract
/// for these is that each is attributed in the Info log and reachable by an
/// agent — `mcp::parity` counts the tools, so a new variant here fails CI
/// until it has one.
///
/// An enum rather than a list of strings so the router's match is exhaustive:
/// adding a variant does not compile until every surface handles it. That is
/// the difference between a convention and an invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostAction {
    Undo,
    Redo,
    /// Step back to a chosen point in the history, rather than one at a time.
    UndoTo,
    RepeatLast,
    SetSetting,
    FavoriteAdd,
    LoadLayout,
}

impl HostAction {
    const ALL: &'static [HostAction] = &[
        HostAction::Undo,
        HostAction::Redo,
        HostAction::UndoTo,
        HostAction::RepeatLast,
        HostAction::SetSetting,
        HostAction::FavoriteAdd,
        HostAction::LoadLayout,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            HostAction::Undo => "undo",
            HostAction::Redo => "redo",
            HostAction::UndoTo => "undo_to",
            HostAction::RepeatLast => "repeat_last",
            HostAction::SetSetting => "set_setting",
            HostAction::FavoriteAdd => "favorite_add",
            HostAction::LoadLayout => "load_layout",
        }
    }
}

/// Per-client view state — deliberately NOT server commands. Two browsers may
/// maximize different areas; opening the palette in one must not open it in
/// another. An agent has no viewport, so the absence of these from MCP is by
/// design, and `mcp::parity` asserts they stay absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientAction {
    Maximize,
    Fullscreen,
    Palette,
    Preferences,
    ToggleToolbar,
    ToggleSidebar,
    Cheatsheet,
    AdjustLast,
    Pie,
    Favorites,
    /// Reopen the startup splash. Per-client like the palette: one visitor
    /// reading the shortcuts is not a thing to show everyone else.
    Splash,
    /// Save / load the workbench layout as a file. The browser owns the
    /// download and the file dialog, so these end at a document event the
    /// layout-file shim listens for rather than at server state.
    ExportLayout,
    ImportLayout,
    Noop,
}

impl ClientAction {
    const ALL: &'static [ClientAction] = &[
        ClientAction::Maximize,
        ClientAction::Fullscreen,
        ClientAction::Palette,
        ClientAction::Preferences,
        ClientAction::ToggleToolbar,
        ClientAction::ToggleSidebar,
        ClientAction::Cheatsheet,
        ClientAction::AdjustLast,
        ClientAction::Pie,
        ClientAction::Favorites,
        ClientAction::Splash,
        ClientAction::ExportLayout,
        ClientAction::ImportLayout,
        ClientAction::Noop,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            ClientAction::Maximize => "maximize",
            ClientAction::Fullscreen => "fullscreen",
            ClientAction::Palette => "palette",
            ClientAction::Preferences => "preferences",
            ClientAction::ToggleToolbar => "toggle_toolbar",
            ClientAction::ToggleSidebar => "toggle_sidebar",
            ClientAction::Cheatsheet => "cheatsheet",
            ClientAction::AdjustLast => "adjust_last",
            ClientAction::Pie => "pie",
            ClientAction::Favorites => "favorites",
            ClientAction::Splash => "splash",
            ClientAction::ExportLayout => "export_layout",
            ClientAction::ImportLayout => "import_layout",
            ClientAction::Noop => "noop",
        }
    }
}

/// Where an action goes. One resolution function, used by the router that
/// executes actions AND by the test that proves every surface emits something
/// resolvable — so the two cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// A named command on the bus.
    Bus,
    Host(HostAction),
    ClientView(ClientAction),
}

pub(crate) fn route(action: &str) -> Route {
    if let Some(h) = HostAction::ALL.iter().find(|h| h.name() == action) {
        return Route::Host(*h);
    }
    if let Some(c) = ClientAction::ALL.iter().find(|c| c.name() == action) {
        return Route::ClientView(*c);
    }
    Route::Bus
}

/// The names, derived from the enums rather than repeated beside them: a list
/// written twice is a list that drifts. Only the parity tests enumerate them —
/// the running code matches on the enums.
#[cfg(test)]
pub(crate) fn host_actions() -> Vec<&'static str> {
    HostAction::ALL.iter().map(|h| h.name()).collect()
}

#[cfg(test)]
pub(crate) fn client_view_actions() -> Vec<&'static str> {
    ClientAction::ALL.iter().map(|c| c.name()).collect()
}

/// Everything the layout is pointed at: the arg of every leaf that has one.
///
/// A list showing something an area is already open on marks that row, which
/// is the only way to tell — from the list — what you are looking at. The
/// values are whatever an editor's target is, so a run's is its id and a
/// file's is its path; they are compared as-is and never parsed.
fn targets_of(layout: &Layout) -> Vec<String> {
    layout
        .root
        .leaves()
        .into_iter()
        .filter_map(|leaf| layout.target_of(leaf))
        .filter(|t| !t.is_empty())
        .collect()
}

fn palette_items(ws: &immersion::Workspaces) -> Vec<PaletteItem> {
    let mut items = vec![
        PaletteItem::new("undo", "Undo")
            .with_hint("revert the last layout change")
            .with_chord("Mod+Z"),
        PaletteItem::new("redo", "Redo")
            .with_hint("reapply an undone change")
            .with_chord("Mod+Shift+Z"),
        PaletteItem::new("maximize", "Maximize area")
            .with_hint("toggle the focused area full-deck")
            .with_chord("Mod+Shift+Space"),
        PaletteItem::new("workspace.cycle", "Next workspace")
            .with_chord("Alt+PageDown")
            .with_params(serde_json::json!({ "delta": 1 })),
        PaletteItem::new("workspace.cycle", "Previous workspace")
            .with_chord("Alt+PageUp")
            .with_params(serde_json::json!({ "delta": -1 })),
    ];
    for (i, tab) in ws.tabs.iter().enumerate() {
        items.push(
            PaletteItem::new("workspace.switch", &format!("Switch to {}", tab.name))
                .with_hint("go to this workspace")
                .with_params(serde_json::json!({ "index": i })),
        );
    }
    items
}

#[component]
pub fn App() -> Element {
    let mut state = use_signal(State::default);
    let mut ws = use_signal(crate::daemon::workspaces);
    // Settings are a serde document the widget editor edits by pointer. Not on
    // the layout bus — a preference is not a layout mutation — but every edit
    // still round-trips through serde and the database like everything else.
    let mut settings = use_signal(crate::daemon::settings);

    // Poll rather than push. One query per second against a WAL database that
    // one process writes is not a cost worth engineering away yet. The
    // workbench rides along so a second browser converges within a tick.
    use_future(move || async move {
        loop {
            if let Ok(s) = fetch_state().await {
                state.set(s);
            }
            ws.set(crate::daemon::workspaces());
            settings.set(crate::daemon::settings());
            let ms = settings.read()["poll_ms"]
                .as_u64()
                .unwrap_or(1000)
                .clamp(200, 10000);
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        }
    });

    let s = state.read().clone();

    // The splash opens on load unless suppressed, and can be reopened from the
    // brand. A signal, because dismissing is transient UI state that need not
    // touch the database — only the "don't show again" preference persists.
    let mut splash_open = use_signal(move || {
        crate::daemon::settings()["splash_on_start"]
            .as_bool()
            .unwrap_or(true)
    });

    // THE write path. Every button, dropdown, gesture and tab emits a named
    // command; this fires it through the daemon (which applies + persists) and
    // updates the local signal immediately, since the poll is for OTHER
    // clients and waiting a tick for your own edit would read as lag.
    // A transient status report (Blender names the operation you just ran in
    // the status bar). Cleared after a few seconds by a token: each report
    // bumps a generation, and the timer only clears if its generation is still
    // current, so a newer report is never wiped by an older timer.
    // The client reports its platform once; chords are written server-side
    // from it, so a rebind updates everywhere without touching the DOM.
    let mut mac = use_signal(|| false);
    let mut help_open = use_signal(|| false);
    // Which binding is waiting for a chord, while the keymap editor captures.
    let mut capturing = use_signal(|| None::<String>);
    // Adjust Last (F9): re-run the last command with edited params.
    let mut adjust_open = use_signal(|| false);
    let mut adjust_name = use_signal(String::new);
    let mut adjust_doc = use_signal(|| serde_json::Value::Null);
    let adj_edit = use_callback(move |(ptr, val): (String, serde_json::Value)| {
        let mut d = adjust_doc();
        immersion::apply_edit(&mut d, &ptr, val);
        adjust_doc.set(d);
    });
    let adj_cancel = use_callback(move |()| adjust_open.set(false));
    let adj_apply = use_callback(move |()| {
        // Replace the last operation with the edited one: revert it, then re-run.
        ws.set(crate::daemon::undo("ui"));
        ws.set(crate::daemon::dispatch(&adjust_name(), adjust_doc()));
        adjust_open.set(false);
    });
    let mut report = use_signal(|| None::<String>);
    let mut report_gen = use_signal(|| 0u64);
    let do_report = use_callback(move |msg: String| {
        report.set(Some(msg));
        let g = report_gen() + 1;
        report_gen.set(g);
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if report_gen() == g {
                report.set(None);
            }
        });
    });

    // The second error surface: a field flags itself locally, and the same
    // message lands here, Blender's Info-line echo.
    let on_editor_error = use_callback(move |e: immersion::EditorError| {
        do_report.call(format!("\u{26a0} {e}"));
    });

    let cmd = use_callback(move |(name, params): (String, serde_json::Value)| {
        let label = report_label(&name, &params);
        // The checked path: a command that fails must say so, not report the
        // success label over a workbench that did not change.
        match crate::daemon::dispatch_checked(&name, params) {
            Ok(w) => {
                ws.set(w);
                do_report.call(label);
            }
            Err(e) => do_report.call(format!("\u{26a0} {label}: {e}")),
        }
    });

    let on_setting = use_callback(move |(pointer, value): (String, serde_json::Value)| {
        settings.set(crate::daemon::set_setting("ui", &pointer, value));
    });

    // The command palette is per-client view state, like maximize — one client
    // searching commands does not open the palette in another.
    let mut palette_open = use_signal(|| false);
    let mut prefs_open = use_signal(|| false);
    // Which area is being retargeted, if any. Per-client like the palette: two
    // people picking targets should not fight over one modal.
    let mut picking_for = use_signal(|| None::<AreaId>);
    // Which area was last clicked. Per-client view state: two people looking
    // at one workbench have their own focus, and the status bar is theirs.
    let mut focused = use_signal(|| None::<AreaId>);

    // Maximize is per-client view state (two browsers may maximize different
    // areas), so it is a local signal, not a command on the shared tree.
    let mut maximized = use_signal(|| None::<AreaId>);
    // Fullscreen is distraction-free mode: hide all chrome (topbar, headers,
    // status bar) and show a single area. Per-client, like maximize.
    let mut fullscreen = use_signal(|| false);

    // Keymap actions. Layout commands go to the bus; undo/redo/maximize are
    // host concerns the bus does not own.
    let on_action = use_callback(move |(action, params): (String, serde_json::Value)| {
        // Exhaustive over the enums: a new host or client action does not
        // compile until it is handled here.
        match route(&action) {
            Route::Host(HostAction::Undo) => ws.set(crate::daemon::undo("ui")),
            Route::Host(HostAction::Redo) => ws.set(crate::daemon::redo("ui")),
            Route::Host(HostAction::UndoTo) => {
                let depth = params.get("depth").and_then(|d| d.as_u64()).unwrap_or(1);
                ws.set(crate::daemon::undo_to("ui", depth as usize));
            }
            Route::Host(HostAction::RepeatLast) => ws.set(crate::daemon::repeat_last("ui")),
            Route::Host(HostAction::SetSetting) => {
                let pointer = params.get("pointer").and_then(|p| p.as_str()).unwrap_or("");
                let value = params
                    .get("value")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                settings.set(crate::daemon::set_setting("ui", pointer, value));
            }
            Route::Host(HostAction::FavoriteAdd) => {
                // The dedup/cap logic lives in the daemon so an agent can
                // curate favourites too; the UI keeps only the report.
                let label = params.get("label").and_then(|l| l.as_str()).unwrap_or("");
                let (doc, added) = crate::daemon::favorite_add("ui", params.clone());
                settings.set(doc);
                if added {
                    do_report.call(format!("Added to favourites: {label}"));
                }
            }
            // The layout-file shim owns this one: it arrives as file text on
            // that component's own channel, not as an action with params.
            Route::Host(HostAction::LoadLayout) => {}
            Route::ClientView(ClientAction::Noop) => {}
            Route::ClientView(ClientAction::Cheatsheet) => help_open.toggle(),
            Route::ClientView(ClientAction::Pie) => {
                let js = format!(
                    "window.__imOpenPie && window.__imOpenPie({});",
                    serde_json::to_string(&pie_menu_json()).unwrap_or_default()
                );
                dioxus::document::eval(&js);
            }
            Route::ClientView(ClientAction::Favorites) => {
                // Hand the list to the menu shim, which raises it at the
                // pointer and sends the pick back over its own channel.
                let items = favorites_menu_json(&settings());
                let js = format!(
                    "window.__imOpenMenu && window.__imOpenMenu({});",
                    serde_json::to_string(&items).unwrap_or_default()
                );
                dioxus::document::eval(&js);
            }
            Route::ClientView(ClientAction::AdjustLast) => {
                if let Some((name, params)) = crate::daemon::last_command() {
                    adjust_name.set(name);
                    adjust_doc.set(params);
                    adjust_open.set(true);
                }
            }
            Route::ClientView(ClientAction::Fullscreen) => fullscreen.toggle(),
            Route::ClientView(ClientAction::Maximize) => {
                // Toggle: maximize the first area if none is, else restore.
                let cur = maximized();
                let first = ws.read().current().layout.root.leaves().first().copied();
                maximized.set(if cur.is_some() { None } else { first });
            }
            Route::ClientView(ClientAction::Palette) => palette_open.set(true),
            Route::ClientView(ClientAction::Splash) => splash_open.set(true),
            // The browser owns the download and the file dialog. The shim
            // already knows how to do both; this only says which.
            Route::ClientView(a @ (ClientAction::ExportLayout | ClientAction::ImportLayout)) => {
                let event = if a == ClientAction::ExportLayout {
                    "im:layout-save"
                } else {
                    "im:layout-load"
                };
                dioxus::document::eval(&format!(
                    "document.dispatchEvent(new CustomEvent('{event}'))"
                ));
            }
            Route::ClientView(ClientAction::Preferences) => prefs_open.set(true),
            // The chord has no area in it, so it acts on the focused one —
            // and says so rather than doing nothing when nothing is focused.
            Route::ClientView(a @ (ClientAction::ToggleToolbar | ClientAction::ToggleSidebar)) => {
                let region = if a == ClientAction::ToggleToolbar {
                    "toolbar"
                } else {
                    "sidebar"
                };
                match focused() {
                    Some(id) => cmd.call((
                        "toggle_region".to_string(),
                        serde_json::json!({ "id": id, "region": region }),
                    )),
                    None => do_report.call("Click an area first".to_string()),
                }
            }
            Route::Bus => cmd.call((action, params)),
        }
    });

    // A menu pick is just an action: a settings edit, a host mutation, a
    // client-view toggle, or a bus command. One resolution decides which, so
    // a menu item can be any of them without a new arm here.
    let on_menu = use_callback(move |(action, params): (String, serde_json::Value)| {
        on_action.call((action, params));
    });

    // Start capturing a chord for a binding: arm the shim, remember which
    // action is waiting. The Keymap component reports the chord back.
    let cap_start = use_callback(move |action: String| {
        capturing.set(Some(action));
        dioxus::document::eval("window.__imCaptureChord && window.__imCaptureChord();");
    });
    let cap_reset = use_callback(move |action: String| {
        let mut km = settings()["keymap"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        km.remove(&action);
        settings.set(crate::daemon::set_setting(
            "ui",
            "/keymap",
            serde_json::Value::Object(km),
        ));
    });

    let on_pick_target = use_callback(move |id: AreaId| picking_for.set(Some(id)));
    let pick_target = use_callback(move |(id, target): (AreaId, String)| {
        // Through the bus like everything else, so an agent retargets an area
        // the same way the picker does.
        cmd.call((
            "set_target".to_string(),
            serde_json::json!({ "id": id, "target": target }),
        ));
        picking_for.set(None);
    });

    let on_template = use_callback(move |i: usize| {
        if let Some(t) = crate::splash::templates().into_iter().nth(i) {
            cmd.call((
                "workspace.add".to_string(),
                serde_json::json!({ "name": t.name, "layout": t.layout }),
            ));
        }
    });
    let on_recent = use_callback(move |run_id: String| {
        // A recent run opens into a fresh workspace showing just its detail.
        let mut l = Layout::single("run");
        l.set_editor_arg(1, "run", &run_id);
        let name = format!("run {}", &run_id[..run_id.len().min(8)]);
        cmd.call((
            "workspace.add".to_string(),
            serde_json::json!({ "name": name, "layout": l }),
        ));
    });
    let on_dismiss = use_callback(move |()| splash_open.set(false));
    // The splash's "don't show" checkbox edits the same setting the Settings
    // editor does — one value, two surfaces.
    let on_dont_show = use_callback(move |off: bool| {
        settings.set(crate::daemon::set_setting(
            "ui",
            "/splash_on_start",
            serde_json::json!(!off),
        ));
    });

    // Every mutation writes through the daemon (which persists) and updates
    // the local signal immediately — the poll is for OTHER clients, and
    // waiting a tick for your own split would read as lag. Layout mutations
    // land on the ACTIVE workspace, the only one the gestures can reach.
    // The pin-beside-the-charts payoff, now a command like everything else.
    let open_run = use_callback(move |(area, run_id): (AreaId, String)| {
        cmd.call((
            "open_run".to_string(),
            serde_json::json!({ "area": area, "run": run_id }),
        ));
    });

    // Add duplicates the current tree — you branch off what you are looking
    // at, not a blank — so it needs the live layout and composes the
    // workspace.add command rather than being a bare command.
    let ws_add = use_callback(move |()| {
        let dup = ws.read().current().layout.clone();
        cmd.call((
            "workspace.add".to_string(),
            serde_json::json!({ "name": "New", "layout": dup }),
        ));
    });

    // Region content (Blender's T toolbar / N sidebar). The toolbar is a
    // vertical strip of area actions; the sidebar is a small properties panel
    // for the focused area. Both route through the same command bus.
    let render_toolbar = use_callback(move |(id, _editor): (AreaId, String)| -> Element {
        rsx! {
            div { class: "area-tools",
                button {
                    class: "area-tool",
                    title: "split horizontal",
                    onclick: move |_| cmd.call(("split".to_string(), serde_json::json!({ "id": id, "dir": "row" }))),
                    "⬒"
                }
                button {
                    class: "area-tool",
                    title: "split vertical",
                    onclick: move |_| cmd.call(("split".to_string(), serde_json::json!({ "id": id, "dir": "col" }))),
                    "◧"
                }
                button {
                    class: "area-tool",
                    title: "duplicate",
                    onclick: move |_| cmd.call(("duplicate_area".to_string(), serde_json::json!({ "id": id }))),
                    "⧉"
                }
            }
        }
    });
    // Both region renderers and the footer need the same bundle an editor is
    // drawn from, so it is built once here rather than three times below.
    let draw_for = move |id: AreaId, editor: String| crate::editors::Draw {
        area: id,
        editor,
        arg: ws.read().current().layout.target_of(id),
        state: state.read().clone(),
        settings: settings.read().clone(),
        targets: targets_of(&ws.read().current().layout),
        mac: mac(),
        capturing: capturing(),
        open_run,
        cap_start,
        cap_reset,
        on_setting,
        on_error: on_editor_error,
    };
    // One line along an area's bottom edge saying what it is showing —
    // "Rust · 148 lines", "7 runs · 1 running". Empty draws no strip, so an
    // editor with nothing to say costs nothing.
    let render_footer = use_callback(move |(id, editor): (AreaId, String)| -> String {
        crate::editors::footer(&draw_for(id, editor))
    });
    let render_sidebar = use_callback(move |(id, editor): (AreaId, String)| -> Element {
        // An editor's sidebar belongs with the editor — the chart's spec
        // editor, the code viewer's statistics, the run list's tally — so the
        // registry is asked first. What is left here is the fallback for an
        // editor that has nothing to say about what it is showing.
        let d = draw_for(id, editor.clone());
        if let Some(el) = crate::editors::sidebar(&d) {
            return el;
        }
        let label = crate::editors::kinds()
            .into_iter()
            .find(|k| k.id == editor)
            .map(|k| k.label)
            .unwrap_or("unknown");
        rsx! {
            div { class: "area-props",
                Panel { title: "Properties",
                    div { class: "area-props-row",
                        span { class: "k", "Editor" }
                        span { "{label}" }
                    }
                    div { class: "area-props-row",
                        span { class: "k", "Area" }
                        span { "{id}" }
                    }
                }
                Panel { title: "Layout", open: false,
                    div { class: "area-props-row",
                        span { class: "k", "Regions" }
                        span { "T · N" }
                    }
                }
            }
        }
    });

    let render = use_callback(
        move |(area, editor, arg): (AreaId, String, Option<String>)| {
            // The library is rendering this leaf and says what it is pointed
            // at, so its arg wins over the one read back from the layout.
            let mut d = draw_for(area, editor);
            d.arg = arg;
            crate::editors::render(d)
        },
    );

    // What the focused area shows, for the status bar's hints.
    let focused_editor = focused().and_then(|id| {
        ws.read()
            .current()
            .layout
            .root
            .find(id)
            .and_then(|a| match a {
                immersion::Area::Leaf { editor, .. } => Some(editor.clone()),
                _ => None,
            })
    });

    rsx! {
        // Blender's Resolution Scale: the library sizes everything in rem, so
        // the whole interface scales from the root font size.
        style { "html {{ font-size: {(settings()[\"ui_scale\"].as_f64().unwrap_or(1.0)).clamp(0.8, 1.6) * 100.0}%; }}" }
        style { "{immersion::CSS}" }
        style { "{immersion::theme_css(settings()[\"theme\"].as_str().unwrap_or(\"Blender Dark\"))}" }
        style { "{CSS}" }
        div {
            class: if fullscreen() { "app im-fullscreen" } else { "app" },
            style: "--im-accent: {settings()[\"accent\"].as_str().unwrap_or(\"#5680c2\")}; --accent-live: {settings()[\"accent\"].as_str().unwrap_or(\"#5680c2\")}; font-size: {(settings()[\"ui_scale\"].as_f64().unwrap_or(1.0)).clamp(0.8, 1.6) * 100.0}%",
            if splash_open() {
                Splash {
                    brand: "powderman",
                    eyebrow: "durable workflows for a herd of agents",
                    version: concat!("v", env!("CARGO_PKG_VERSION")),
                    subtitle: "a herdr workbench — drive it by hand, or hand it to an agent",
                    foot: crate::splash::splash_foot(mac()),
                    templates: crate::splash::templates(),
                    recents: crate::splash::recents(&s),
                    on_template,
                    on_recent,
                    on_dismiss,
                    dont_show: !settings()["splash_on_start"].as_bool().unwrap_or(true),
                    on_dont_show,
                }
            }
            div { class: "topbar",
                span {
                    class: "brand",
                    title: "open the splash",
                    onclick: move |_| splash_open.set(true),
                    "powderman"
                }
                span { class: "menubar",
                    button { class: "im-menubtn", "data-im-menu-click": "{file_menu()}", "File" }
                    button { class: "im-menubtn", "data-im-menu-click": "{edit_menu(mac())}", "Edit" }
                    button { class: "im-menubtn", "data-im-menu-click": "{undo_history_menu(&crate::daemon::undo_history())}", "Undo History" }
                    button { class: "im-menubtn", "data-im-menu-click": "{repeat_history_menu(&crate::daemon::command_log())}", "Repeat History" }
                    button { class: "im-menubtn", "data-im-menu-click": "{view_menu(&settings())}", "View" }
                    button { class: "im-menubtn", "data-im-menu-click": "{window_menu(ws.read().active, mac())}", "Window" }
                    button { class: "im-menubtn", "data-im-menu-click": "{help_menu(mac())}", "Help" }
                }
                // Everything that is not a menu sits to the right, the way
                // the old Immersion had it: the workspaces you switch between,
                // the search, the theme, and what this page is connected to.
                // The gap in the middle is the point — a menu bar and a set of
                // controls read as two things when they are not crowded
                // together.
                span { class: "topbar-right",
                    WorkspaceTabs {
                        names: ws.read().tabs.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
                        active: ws.read().active,
                        on_command: cmd,
                        on_add: ws_add,
                    }
                    // The palette answered only to F3, which is a palette
                    // newcomers never find. Both references put a search in
                    // the topbar; this is that, opening what already exists.
                    button {
                        class: "im-menubtn topbar-search",
                        title: "search commands",
                        "data-tip": "Commands",
                        "data-tip-key": "{pretty_chord(\"F3\", mac())}",
                        onclick: move |_| palette_open.set(true),
                        dangerous_inner_html: "{immersion::icon(\"search\")}",
                    }
                    span { class: "topbar-sep" }
                    button {
                        class: "im-menubtn topbar-theme",
                        title: "theme",
                        "data-im-menu-click": "{theme_menu(&settings())}",
                        span {
                            class: "topbar-theme-icon",
                            dangerous_inner_html: "{immersion::icon(\"palette\")}",
                        }
                        span { class: "topbar-theme-name",
                            {settings()["theme"].as_str().unwrap_or("Blender Dark").to_string()}
                        }
                    }
                    span { class: "topbar-sep" }
                    // Where the old Immersion showed its room. A liveview page
                    // whose socket has died looks exactly like a working one,
                    // so this says which it is. It needs no signal of its own:
                    // the disconnect script already marks the body, and the
                    // two labels are swapped by CSS — which matters, because
                    // by the time it is wrong there is no server left to
                    // re-render it.
                    span { class: "conn", title: "{crate::daemon::public_url()}",
                        span { class: "conn-dot" }
                        span { class: "conn-on", "Connected" }
                        span { class: "conn-off", "Disconnected" }
                        button {
                            class: "im-copy",
                            title: "copy this server's MCP address",
                            "data-im-copy": "{crate::daemon::public_url()}/mcp",
                            dangerous_inner_html: "{immersion::icon(\"copy\")}",
                        }
                    }
                    // Save and Load live in the File menu now. The component
                    // stays for its machinery — the import channel, and the
                    // element the current layout rides on for the menu path.
                    LayoutFile {
                        buttons: false,
                        layout_json: serde_json::to_string(&ws.read().clone()).unwrap_or_default(),
                        on_import: move |json: String| {
                            ws.set(crate::daemon::set_workspaces_from_json("ui", &json));
                        },
                    }
                }
            }
            Platform { on_platform: move |m: bool| mac.set(m) }
            Keymap {
                bindings: effective_keymap(&settings()),
                on_action,
                on_capture: move |chord: String| {
                    if let Some(action) = capturing() {
                        settings.set(crate::daemon::set_setting("ui",
                            &format!("/keymap/{action}"),
                            serde_json::json!(chord),
                        ));
                        capturing.set(None);
                    }
                },
            }
            ContextMenu { on_command: on_menu }
            if help_open() {
                KeymapHelp {
                    bindings: effective_keymap(&settings()),
                    mac: mac(),
                    on_close: move |()| help_open.set(false),
                }
            }
            if prefs_open() {
                crate::panels::PreferencesPanel {
                    doc: settings(),
                    on_edit: on_setting,
                    on_error: on_editor_error,
                    on_close: move |()| prefs_open.set(false),
                }
            }
            if let Some(id) = picking_for() {
                crate::panels::TargetPicker {
                    area: id,
                    current: ws.read().current().layout.target_of(id),
                    editor: ws
                        .read()
                        .current()
                        .layout
                        .root
                        .find(id)
                        .and_then(|a| match a {
                            immersion::Area::Leaf { editor, .. } => Some(editor.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    state: s.clone(),
                    on_pick: pick_target,
                    on_cancel: use_callback(move |()| picking_for.set(None)),
                }
            }
            if adjust_open() {
                crate::panels::AdjustPanel {
                    name: adjust_name(),
                    doc: adjust_doc(),
                    on_edit: adj_edit,
                    on_error: on_editor_error,
                    on_cancel: adj_cancel,
                    on_apply: adj_apply,
                }
            }
            Chrome { tooltips_enabled: settings()["tooltips_on"].as_bool().unwrap_or(true) }
            if fullscreen() {
                button {
                    class: "fullscreen-exit",
                    title: "exit fullscreen",
                    onclick: move |_| fullscreen.set(false),
                    "⤢ exit"
                }
            }
            if palette_open() {
                Palette {
                    items: palette_items(&ws.read()),
                    on_run: on_action,
                    on_close: move |()| palette_open.set(false),
                }
            }
            div { class: "deck",
                Areas {
                    layout: ws.read().current().layout.clone(),
                    kinds: crate::editors::kinds(),
                    render,
                    render_toolbar: Some(render_toolbar),
                    render_sidebar: Some(render_sidebar),
                    render_footer: Some(render_footer),
                    on_command: cmd,
                    on_pick_target,
                    on_focus: move |id: AreaId| focused.set(Some(id)),
                    maximized: maximized().or_else(|| {
                        if fullscreen() {
                            ws.read().current().layout.root.leaves().first().copied()
                        } else {
                            None
                        }
                    }),
                    revision: settings_revision(&settings()),
                    chords: chord_map(&settings(), mac()),
                }
            }
            StatusBar {
                task: crate::status::running_task(&s),
                hints: crate::status::status_hints(mac(), focused_editor.as_deref()),
                message: report(),
                right: format!(
                    "{} · {} runs · v{}",
                    s.herdr.clone().unwrap_or_else(|| "herdr unreachable".into()),
                    s.runs.len(),
                    env!("CARGO_PKG_VERSION")
                ),
                badge: crate::demo::enabled().then(|| "demo".to_string()),
            }
        }
    }
}

/// Turn a command'"'"'s params into an editable form: a field per key, its widget
/// chosen by the value's type. Adjust Last renders this over the last command's
/// params so you can re-run it with tweaks.
pub(crate) fn fields_from_params(params: &serde_json::Value) -> Vec<Field> {
    let mut fields = Vec::new();
    if let Some(obj) = params.as_object() {
        for (k, v) in obj {
            let kind = if v.is_number() {
                FieldKind::Number {
                    min: None,
                    max: None,
                    step: None,
                }
            } else if v.is_boolean() {
                FieldKind::Bool
            } else {
                FieldKind::Text
            };
            fields.push(Field::new(&format!("/{k}"), k, kind));
        }
    }
    fields
}

/// A short human label for a command, for the status-bar report — Blender's
/// "Split Area" line. Falls back to the raw name for anything unmapped.
fn report_label(name: &str, params: &serde_json::Value) -> String {
    match name {
        "split" => {
            let dir = params.get("dir").and_then(|d| d.as_str()).unwrap_or("");
            format!(
                "Split area {}",
                if dir == "col" {
                    "vertical"
                } else {
                    "horizontal"
                }
            )
        }
        "join" | "join_into" => "Close area".to_string(),
        "swap" => "Swap areas".to_string(),
        "ratio" => "Resize".to_string(),
        "set_editor" | "open_editor" => "Change editor".to_string(),
        "open_run" => "Open run".to_string(),
        n if n.starts_with("workspace.") => "Workspace".to_string(),
        other => other.to_string(),
    }
}

/// `action -> chord`, written the platform's way, for the chrome to show in
/// tooltips. Built from the effective keymap so a rebind moves the hint too.
fn chord_map(settings: &serde_json::Value, mac: bool) -> std::collections::HashMap<String, String> {
    effective_keymap(settings)
        .into_iter()
        .map(|b| (b.action, pretty_chord(&b.chord, mac)))
        .collect()
}

/// A cheap version of the settings document, so the deck re-renders when a
/// setting an editor shows has changed (the layout alone does not encode it).
fn settings_revision(settings: &serde_json::Value) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    settings.to_string().hash(&mut h);
    h.finish()
}

/// The keymap as it actually stands: the shipped defaults with the user's
/// rebinds applied. An override is stored per action, so a rebind survives a
/// change to the default and the editor can show what is customised.
pub(crate) fn effective_keymap(settings: &serde_json::Value) -> Vec<immersion::Binding> {
    let overrides = &settings["keymap"];
    default_keymap()
        .into_iter()
        .map(|mut b| {
            if let Some(chord) = overrides.get(&b.action).and_then(|c| c.as_str()) {
                b.chord = chord.to_string();
            }
            b
        })
        .collect()
}

// --- editors --------------------------------------------------------------
// Each one is a body the library mounts under an area header. They read the
// polled snapshot and call the same daemon functions the old page did; the
// workbench changed where they sit, not what they are.

// No server functions: under liveview the component *is* server-side, so it
// reads the database the daemon already owns. The browser holds nothing but a
// websocket and a DOM.
async fn fetch_state() -> anyhow::Result<State> {
    Ok(crate::daemon::snapshot())
}

pub(crate) async fn run_with(name: String, input: String) -> anyhow::Result<()> {
    let value = if input.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&input)?
    };
    crate::daemon::trigger_with(&name, value)
}

pub(crate) async fn resume(id: String) -> anyhow::Result<()> {
    crate::daemon::resume(&id)
}

#[cfg(test)]
mod keymap_tests {
    use super::*;

    #[test]
    fn an_override_replaces_the_default_chord() {
        let settings = serde_json::json!({ "keymap": { "favorites": "Mod+Shift+K" } });
        let km = effective_keymap(&settings);
        let fav = km
            .iter()
            .find(|b| b.action == "favorites")
            .expect("favorites binding");
        assert_eq!(fav.chord, "Mod+Shift+K");
        // untouched bindings keep their defaults
        let undo = km
            .iter()
            .find(|b| b.action == "undo")
            .expect("undo binding");
        assert_eq!(undo.chord, "Mod+Z");
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    /// Whether any leaf in a layout ships with its N panel open.
    fn opens_a_sidebar(l: &Layout) -> bool {
        l.root.leaves().iter().any(|id| {
            matches!(
                l.root.find(*id),
                Some(immersion::Area::Leaf { regions, .. }) if regions.sidebar
            )
        })
    }

    /// A list marks the row an area is already open on, which needs the
    /// layout's targets as the editors store them. The bug this guards is the
    /// one the target picker had: a run's target is its id, and a list
    /// comparing ids against JSON pointers marks nothing, forever, silently.
    #[test]
    fn the_targets_are_what_the_editors_actually_store() {
        let mut l = Layout::single("runs");
        let detail = l.split(1, immersion::Dir::Row, 0.5).expect("a second area");
        l.set_editor(detail, "run");
        l.set_target(detail, "aaaa1111");
        let files = l
            .split(detail, immersion::Dir::Col, 0.5)
            .expect("a third area");
        l.set_editor(files, "code");
        l.set_target(files, "src/main.rs");

        let mut targets = targets_of(&l);
        targets.sort();
        assert_eq!(targets, vec!["aaaa1111", "src/main.rs"]);
        // The list area itself points at nothing, and an empty target is not
        // a target — it would match every row with a blank id.
        assert_eq!(targets.len(), 2, "the bare list contributes nothing");
    }

    /// A starter set built by naming templates is a starter set that ships
    /// fewer tabs than it means to the moment a name is renamed — silently,
    /// because a missing template is skipped rather than refused.
    #[test]
    fn every_starter_workspace_names_a_template_that_exists() {
        let names: Vec<String> = crate::splash::templates()
            .iter()
            .map(|t| t.name.clone())
            .collect();
        for want in crate::daemon::STARTER {
            assert!(
                names.iter().any(|n| n == want),
                "the starter set names {want}, which is not a template: {names:?}"
            );
        }
        let ws = crate::daemon::default_workspaces();
        assert_eq!(
            ws.tabs.len(),
            crate::daemon::STARTER.len(),
            "a starter tab went missing"
        );
        assert_eq!(
            ws.active, 0,
            "you land on the first tab, not the last added"
        );
        assert_eq!(ws.tabs[0].name, "Overview");
    }

    /// The sidebar region existed from the start and nothing shipped with it
    /// on, so it was a feature you had to already know about to ever see.
    /// Turning it on is one call per layout, which is exactly the kind of
    /// thing a new arrangement forgets.
    #[test]
    fn every_shipped_arrangement_opens_its_sidebar() {
        for t in crate::splash::templates() {
            assert!(
                opens_a_sidebar(&t.layout),
                "the {} template ships with no N panel open",
                t.name
            );
        }
        let starter = crate::daemon::default_workspaces();
        assert!(
            opens_a_sidebar(&starter.tabs[0].layout),
            "a first visit meets no N panel"
        );
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;

    /// Pull every "action" out of a menu-JSON string. Menus are the JSON the
    /// shim receives, so parsing them exercises exactly what a click emits.
    fn menu_actions(json: &str) -> Vec<String> {
        serde_json::from_str::<Vec<serde_json::Value>>(json)
            .expect("menu JSON parses")
            .into_iter()
            .filter_map(|i| i.get("action")?.as_str().map(str::to_string))
            .collect()
    }

    /// The parity invariant: every action any UI surface can emit — a default
    /// binding, a palette row, a menu item, a pie slice — resolves to a bus
    /// command, a host action, or a declared client-view action. A name in
    /// none of the three is a control that silently does nothing; this is the
    /// test that turns "one write path" from a convention into CI.
    #[test]
    fn every_ui_action_resolves() {
        let commands = crate::workflows::commands();
        let ws = immersion::Workspaces::new("test", Layout::single("runs"));
        // Settings with one favourite, so the favourites surface is exercised
        // rather than skipped on an empty default.
        let settings = serde_json::json!({
            "favorites": [{ "label": "Split", "action": "split",
                            "params": { "id": 1, "dir": "row" } }]
        });

        let mut actions: Vec<(String, String)> = Vec::new(); // (surface, action)
        for b in immersion::default_keymap() {
            actions.push(("keymap".into(), b.action));
        }
        for p in palette_items(&ws) {
            actions.push(("palette".into(), p.action));
        }
        for a in menu_actions(&window_menu(0, false)) {
            actions.push(("window menu".into(), a));
        }
        for a in menu_actions(&file_menu()) {
            actions.push(("file menu".into(), a));
        }
        for a in menu_actions(&view_menu(&crate::daemon::settings_defaults())) {
            actions.push(("view menu (topbar)".into(), a));
        }
        // Both are built from live history, so they are harvested against a
        // synthetic one — the rows are what matters, not where they came from.
        let history = vec![(1, "split".to_string()), (2, "set_editor".to_string())];
        for a in menu_actions(&undo_history_menu(&history)) {
            actions.push(("undo history".into(), a));
        }
        let log = vec![LogEntry {
            name: "split".into(),
            params: serde_json::json!({ "id": 1, "dir": "row" }),
            at: 0,
            ok: true,
            source: "ui".into(),
        }];
        for a in menu_actions(&repeat_history_menu(&log)) {
            actions.push(("repeat history".into(), a));
        }
        for a in menu_actions(&edit_menu(false)) {
            actions.push(("edit menu".into(), a));
        }
        for a in menu_actions(&help_menu(false)) {
            actions.push(("help menu".into(), a));
        }
        for a in menu_actions(&pie_menu_json()) {
            actions.push(("pie".into(), a));
        }
        for a in menu_actions(&favorites_menu_json(&settings)) {
            actions.push(("favorites".into(), a));
        }
        let kinds = crate::editors::kinds();
        for a in menu_actions(&immersion::editor_menu_json(1, &kinds, "runs")) {
            actions.push(("editor menu".into(), a));
        }
        for a in menu_actions(&immersion::view_menu_json(1, true, true, true)) {
            actions.push(("view menu".into(), a));
        }
        for a in menu_actions(&immersion::area_menu_json(1)) {
            actions.push(("area menu".into(), a));
        }

        assert!(
            actions.len() > 30,
            "harvest looks broken: {}",
            actions.len()
        );
        let mut orphans = Vec::new();
        for (surface, action) in &actions {
            let known = commands.get(action).is_some() || !matches!(route(action), Route::Bus);
            if !known {
                orphans.push(format!("{surface}: {action}"));
            }
        }
        assert!(
            orphans.is_empty(),
            "actions with no resolution (bus, host, or client-view):\n  {}",
            orphans.join("\n  ")
        );
    }

    /// Menu params, not just menu names. `every_ui_action_resolves` checks
    /// that a row's action is *known*; a row whose params the command rejects
    /// still does nothing when clicked. File ▸ New workspace carries a whole
    /// serialized layout, which is exactly the kind of payload that rots.
    #[test]
    fn the_file_menus_rows_run_with_the_params_they_carry() {
        let commands = crate::workflows::commands();
        let mut ws = immersion::Workspaces::new("test", Layout::single("runs"));
        let before = ws.tabs.len();
        for item in
            serde_json::from_str::<Vec<serde_json::Value>>(&file_menu()).expect("menu JSON parses")
        {
            let Some(action) = item.get("action").and_then(|a| a.as_str()) else {
                continue;
            };
            if !matches!(route(action), Route::Bus) {
                continue;
            }
            let params = item
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            commands
                .run(&mut ws, action, &params)
                .unwrap_or_else(|e| panic!("File menu row {action} does nothing: {e}"));
        }
        assert!(
            ws.tabs.len() > before,
            "and the rows actually did something"
        );
    }

    /// The View menu writes settings, so each row has to name a pointer the
    /// document has. A typo would write a key nothing reads — a control that
    /// appears to work and does not.
    #[test]
    fn the_view_menu_writes_pointers_the_settings_document_has() {
        let defaults = crate::daemon::settings_defaults();
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(&view_menu(&defaults)).expect("menu JSON parses");
        for row in &rows {
            if row.get("action").and_then(|a| a.as_str()) != Some("set_setting") {
                continue;
            }
            let pointer = row["params"]["pointer"].as_str().expect("a pointer");
            assert!(
                defaults.pointer(pointer).is_some(),
                "the View menu writes {pointer}, which the settings document does not have"
            );
        }
        // Every theme the library ships is offered, so adding one there does
        // not need a row added here.
        for t in immersion::themes() {
            assert!(
                rows.iter()
                    .any(|r| r.get("label").and_then(|l| l.as_str()) == Some(t.name)),
                "the {} theme is not on the View menu",
                t.name
            );
        }
        // And exactly one of them is ticked — the active one.
        let ticked = rows
            .iter()
            .filter(|r| r["params"]["pointer"] == "/theme" && r.get("icon").is_some())
            .count();
        assert_eq!(ticked, 1, "themes ticked: {ticked}");
    }

    /// The complement: a host action that stops being routed is as broken as
    /// an unrouted menu item. The on_action / on_menu matches cannot be
    /// inspected directly, so this pins the contract they implement.
    #[test]
    fn host_and_client_lists_do_not_overlap_or_shadow_the_bus() {
        let commands = crate::workflows::commands();
        for a in host_actions() {
            assert!(
                !client_view_actions().contains(&a),
                "{a} is both a host action and client-view state"
            );
            assert!(
                commands.get(a).is_none(),
                "{a} is both a host action and a bus command — one must win"
            );
        }
        for a in client_view_actions() {
            assert!(
                commands.get(a).is_none(),
                "{a} is both client-view and a bus command — one must win"
            );
        }
    }
}

#[cfg(test)]
mod region_chord_tests {
    use super::*;

    /// The header has always labelled these buttons T and N. For a long time
    /// neither key was bound to anything, so the labels named keys that did
    /// nothing — the kind of wrong that is invisible until someone presses one.
    #[test]
    fn the_letters_on_the_buttons_are_bound() {
        let km = immersion::default_keymap();
        for (chord, action) in [("T", "toggle_toolbar"), ("N", "toggle_sidebar")] {
            let b = km
                .iter()
                .find(|b| b.chord == chord)
                .unwrap_or_else(|| panic!("{chord} is on a button but bound to nothing"));
            assert_eq!(b.action, action);
        }
    }

    /// They act on the focused area, so they are client-view state that the
    /// host turns into a bus command — not commands themselves, which would
    /// need an area id the chord cannot carry.
    #[test]
    fn they_route_through_the_client_view() {
        assert!(matches!(
            route("toggle_toolbar"),
            Route::ClientView(ClientAction::ToggleToolbar)
        ));
        assert!(matches!(
            route("toggle_sidebar"),
            Route::ClientView(ClientAction::ToggleSidebar)
        ));
    }
}
